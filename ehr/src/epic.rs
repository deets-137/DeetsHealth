//! Epic, as a system: SMART Backend Services (integrations/EPIC.md §5, step E-2).
//!
//! `ehr` signs a short JWT with the non-production ES384 key (`dev-certs/epic-nonprod-es384.key`,
//! EPIC.md §2.3) and trades it for an access token at Epic's token endpoint
//! (`client_credentials`, RFC 7523). No person signs in. The public half is served at the JWK
//! Set URL that the registration names.
//!
//! - [`EpicConfig::load`] reads `epic-client.json` (the client ID) and the key.
//! - [`Epic::connect`] reads the server's `smart-configuration` for the token endpoint.
//! - [`Epic::token`] signs the assertion and asks for a token. The token is a secret: it is
//!   held in `Zeroizing`, sent only to Epic, and never printed.
//! - [`Epic::get`] reads one FHIR path with the token.
//!
//! Output and errors name a status code or a fixed string. Never a token, a key or a value
//! from a reply.

use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aws_lc_rs::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, ECDSA_P384_SHA384_FIXED_SIGNING},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{redirect::Policy, Url};
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::{pem::PemObject, PrivatePkcs8KeyDer};
use serde::Deserialize;
use zeroize::Zeroizing;

use crate::transport;

/// Epic's sandbox (EPIC.md §3). With the trailing slash, so paths join under it.
pub const SANDBOX_BASE: &str = "https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4/";
/// The key ID: the RFC 7638 thumbprint of the public key in
/// `ehr/jwks-worker/src/nonprod.jwks.json`. A test checks that the two agree.
pub const NONPROD_KID: &str = "EsN08Bnv-m-gvEvF-gX8wgsfbch9Kz5COViuihyZeA8";
/// Epic refuses an assertion that lives longer than 5 minutes.
const ASSERTION_LIFE: Duration = Duration::from_secs(240);
/// The longest any one request to Epic may take.
const REQUEST_LIMIT: Duration = Duration::from_secs(15);

/// What `ehr` needs to sign in to one Epic server.
pub struct EpicConfig {
    pub client_id: String,
    pub base: Url,
    key: EcdsaKeyPair,
    kid: String,
}

#[derive(Deserialize)]
struct ClientFile {
    nonprod_client_id: String,
}

impl EpicConfig {
    /// The sandbox, with the non-production client ID from `client_file` and the key in
    /// `key_file` (PKCS #8, PEM).
    pub fn sandbox(client_file: &Path, key_file: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(client_file)
            .map_err(|_| format!("cannot read {}", client_file.display()))?;
        let file: ClientFile =
            serde_json::from_str(&text).map_err(|_| format!("{}: not the expected shape", client_file.display()))?;
        if file.nonprod_client_id.trim().is_empty() {
            return Err(format!("{}: nonprod_client_id is empty", client_file.display()));
        }
        let der = PrivatePkcs8KeyDer::from_pem_file(key_file)
            .map_err(|_| format!("{}: not a PKCS #8 PEM key", key_file.display()))?;
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P384_SHA384_FIXED_SIGNING, der.secret_pkcs8_der())
            .map_err(|_| format!("{}: not a P-384 key", key_file.display()))?;
        Ok(Self {
            client_id: file.nonprod_client_id.trim().to_string(),
            base: Url::parse(SANDBOX_BASE).map_err(|_| "bad sandbox URL")?,
            key,
            kid: NONPROD_KID.to_string(),
        })
    }

    /// The public key as a JWK's `x` and `y` (base64url), to check against the published set.
    pub fn public_xy(&self) -> (String, String) {
        use aws_lc_rs::signature::KeyPair;
        // Uncompressed point: 0x04 || X (48 bytes) || Y (48 bytes).
        let point = self.key.public_key().as_ref();
        (URL_SAFE_NO_PAD.encode(&point[1..49]), URL_SAFE_NO_PAD.encode(&point[49..97]))
    }

    /// The signed client assertion (RFC 7523): `iss` and `sub` are the client ID, `aud` the
    /// token endpoint, `jti` 128 random bits, `exp` under Epic's 5-minute limit.
    fn assertion(&self, token_url: &Url, now: u64) -> Result<Zeroizing<String>, String> {
        let header = serde_json::json!({ "alg": "ES384", "typ": "JWT", "kid": self.kid });
        let mut jti = [0u8; 16];
        getrandom::fill(&mut jti).map_err(|_| "no random source")?;
        let claims = serde_json::json!({
            "iss": self.client_id,
            "sub": self.client_id,
            "aud": token_url.as_str(),
            "jti": URL_SAFE_NO_PAD.encode(jti),
            "iat": now,
            "nbf": now,
            "exp": now + ASSERTION_LIFE.as_secs(),
        });
        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let sig = self
            .key
            .sign(&SystemRandom::new(), signing_input.as_bytes())
            .map_err(|_| "cannot sign the assertion")?;
        Ok(Zeroizing::new(format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))))
    }
}

/// An access token. The string never leaves this module except as a bearer header to Epic.
#[derive(Clone)]
pub struct Token {
    secret: Zeroizing<String>,
    /// Seconds the token lives, as Epic says.
    pub expires_in: u64,
    /// How many scopes Epic granted (the names are not printed).
    pub scope_count: usize,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    scope: String,
}

#[derive(Deserialize)]
struct SmartConfig {
    token_endpoint: String,
}

/// A failed step: what was asked, and Epic's status or a fixed reason. Never a body.
#[derive(Debug)]
pub struct EpicError(pub String);

impl std::fmt::Display for EpicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One Epic server, signed in as our backend app.
pub struct Epic {
    config: EpicConfig,
    client: reqwest::Client,
    pub token_url: Url,
    /// The token in use and when it stops being used (a minute before Epic's expiry).
    current: tokio::sync::Mutex<Option<(Token, Instant)>>,
}

/// A token is renewed this long before Epic says it expires.
const TOKEN_MARGIN: Duration = Duration::from_secs(60);

impl Epic {
    /// Builds the HTTP client (AWS-LC, Mozilla's roots, no redirects) and reads the server's
    /// `.well-known/smart-configuration` for the token endpoint.
    pub async fn connect(config: EpicConfig) -> Result<Self, EpicError> {
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = ClientConfig::builder_with_provider(transport::provider())
            .with_safe_default_protocol_versions()
            .map_err(|_| EpicError("the TLS provider refused its own versions".into()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .timeout(REQUEST_LIMIT)
            .redirect(Policy::none())
            .build()
            .map_err(|_| EpicError("cannot build the HTTP client".into()))?;
        let url = config
            .base
            .join(".well-known/smart-configuration")
            .map_err(|_| EpicError("bad smart-configuration URL".into()))?;
        let reply = client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| EpicError("smart-configuration: no answer".into()))?;
        let status = reply.status();
        if !status.is_success() {
            return Err(EpicError(format!("smart-configuration: HTTP {}", status.as_u16())));
        }
        let smart: SmartConfig = reply
            .json()
            .await
            .map_err(|_| EpicError("smart-configuration: not the expected shape".into()))?;
        let token_url =
            Url::parse(&smart.token_endpoint).map_err(|_| EpicError("smart-configuration: bad token_endpoint".into()))?;
        if token_url.scheme() != "https" {
            return Err(EpicError("smart-configuration: token_endpoint is not https".into()));
        }
        Ok(Self { config, client, token_url, current: tokio::sync::Mutex::new(None) })
    }

    /// The base URL, to build a search URL on (with `query_pairs_mut`, so every value is
    /// encoded).
    pub fn base(&self) -> &Url {
        &self.config.base
    }

    /// A token that is still good: the one in use, or a new one.
    async fn bearer(&self, fresh: bool) -> Result<Token, EpicError> {
        let mut current = self.current.lock().await;
        if !fresh {
            if let Some((token, until)) = current.as_ref() {
                if Instant::now() < *until {
                    return Ok(token.clone());
                }
            }
        }
        let token = self.token().await?;
        let life = Duration::from_secs(token.expires_in).saturating_sub(TOKEN_MARGIN);
        *current = Some((token.clone(), Instant::now() + life));
        Ok(token)
    }

    /// Reads one FHIR URL under the base with the token in use; a 401 gets one new token and
    /// one more try. Any status but 200 is an error that names the status only.
    pub async fn read(&self, url: Url) -> Result<serde_json::Value, EpicError> {
        if !url.as_str().starts_with(self.config.base.as_str()) {
            return Err(EpicError("FHIR read: not under the base URL".into()));
        }
        for fresh in [false, true] {
            let token = self.bearer(fresh).await?;
            let reply = self
                .client
                .get(url.clone())
                .bearer_auth(token.secret.as_str())
                .header("Accept", "application/fhir+json")
                .send()
                .await
                .map_err(|_| EpicError("FHIR read: no answer".into()))?;
            let status = reply.status().as_u16();
            if status == 401 && !fresh {
                continue;
            }
            if status != 200 {
                return Err(EpicError(format!("FHIR read: HTTP {status}")));
            }
            return reply.json().await.map_err(|_| EpicError("FHIR read: not JSON".into()));
        }
        Err(EpicError("FHIR read: HTTP 401".into()))
    }

    /// Signs the assertion and asks for an access token.
    pub async fn token(&self) -> Result<Token, EpicError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| EpicError("the clock is before 1970".into()))?
            .as_secs();
        let assertion = self.config.assertion(&self.token_url, now).map_err(EpicError)?;
        let form = [
            ("grant_type", "client_credentials"),
            ("client_assertion_type", "urn:ietf:params:oauth:client-assertion-type:jwt-bearer"),
            ("client_assertion", assertion.as_str()),
        ];
        let reply = self
            .client
            .post(self.token_url.clone())
            .header("Accept", "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|_| EpicError("token: no answer".into()))?;
        let status = reply.status();
        if !status.is_success() {
            // Epic's OAuth error code ("invalid_client", ...) is a fixed word, not a value.
            let code = reply
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(oauth_word))
                .unwrap_or("no error code");
            return Err(EpicError(format!("token: HTTP {} ({code})", status.as_u16())));
        }
        let body: TokenReply =
            reply.json().await.map_err(|_| EpicError("token: not the expected shape".into()))?;
        Ok(Token {
            secret: Zeroizing::new(body.access_token),
            expires_in: body.expires_in,
            scope_count: body.scope.split_whitespace().count(),
        })
    }

    /// Whether the key on disk is the one the JWK Set URL publishes (by `kid`, `x` and `y`).
    pub async fn key_is_published(&self, jwks_url: &str) -> Result<bool, EpicError> {
        let reply = self
            .client
            .get(jwks_url)
            .send()
            .await
            .map_err(|_| EpicError("key set: no answer".into()))?;
        if !reply.status().is_success() {
            return Err(EpicError(format!("key set: HTTP {}", reply.status().as_u16())));
        }
        let set: serde_json::Value =
            reply.json().await.map_err(|_| EpicError("key set: not JSON".into()))?;
        let (x, y) = self.config.public_xy();
        Ok(set["keys"].as_array().into_iter().flatten().any(|k| {
            k["kid"] == self.config.kid.as_str() && k["x"] == x.as_str() && k["y"] == y.as_str()
        }))
    }

    /// Reads one FHIR path (relative to the base, query included) with the token. Returns the
    /// status and, on success, the JSON. The caller decides what of it may be printed.
    pub async fn get(&self, token: &Token, path: &str) -> Result<(u16, Option<serde_json::Value>), EpicError> {
        let url = self.config.base.join(path).map_err(|_| EpicError("bad FHIR path".into()))?;
        let reply = self
            .client
            .get(url)
            .bearer_auth(token.secret.as_str())
            .header("Accept", "application/fhir+json")
            .send()
            .await
            .map_err(|_| EpicError("FHIR read: no answer".into()))?;
        let status = reply.status().as_u16();
        if !reply.status().is_success() {
            return Ok((status, None));
        }
        let json = reply.json().await.map_err(|_| EpicError("FHIR read: not JSON".into()))?;
        Ok((status, Some(json)))
    }
}

/// Keeps only the OAuth error codes RFC 6749 names, so nothing else from a reply is printed.
fn oauth_word(s: &str) -> &'static str {
    match s {
        "invalid_request" => "invalid_request",
        "invalid_client" => "invalid_client",
        "invalid_grant" => "invalid_grant",
        "unauthorized_client" => "unauthorized_client",
        "unsupported_grant_type" => "unsupported_grant_type",
        "invalid_scope" => "invalid_scope",
        _ => "another error code",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kid in the code is the one in the published set.
    #[test]
    fn the_kid_matches_the_published_set() {
        let set: serde_json::Value =
            serde_json::from_str(include_str!("../jwks-worker/src/nonprod.jwks.json")).unwrap();
        assert_eq!(set["keys"][0]["kid"], NONPROD_KID);
    }

    /// An assertion signed with a fresh key verifies, and carries the claims Epic checks.
    #[test]
    fn the_assertion_is_a_valid_es384_jwt() {
        use aws_lc_rs::signature::{KeyPair, UnparsedPublicKey, ECDSA_P384_SHA384_FIXED};
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P384_SHA384_FIXED_SIGNING, &SystemRandom::new()).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P384_SHA384_FIXED_SIGNING, pkcs8.as_ref()).unwrap();
        let public = key.public_key().as_ref().to_vec();
        let config = EpicConfig {
            client_id: "test-client".into(),
            base: Url::parse(SANDBOX_BASE).unwrap(),
            key,
            kid: "test-kid".into(),
        };
        let token_url = Url::parse("https://example.test/oauth2/token").unwrap();
        let jwt = config.assertion(&token_url, 1_000_000).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        UnparsedPublicKey::new(&ECDSA_P384_SHA384_FIXED, &public)
            .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig)
            .unwrap();
        let header: serde_json::Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(header["alg"], "ES384");
        assert_eq!(header["kid"], "test-kid");
        assert_eq!(claims["iss"], "test-client");
        assert_eq!(claims["sub"], "test-client");
        assert_eq!(claims["aud"], "https://example.test/oauth2/token");
        assert!(claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap() <= 300);
        assert_eq!(URL_SAFE_NO_PAD.decode(claims["jti"].as_str().unwrap()).unwrap().len(), 16);
    }
}
