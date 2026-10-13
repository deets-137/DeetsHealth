//! SMART sign-in (CORE-CRATE.md §6): standalone patient launch, public client, PKCE (S256).
//!
//! 1. [`begin`] reads the site's `smart-configuration`, makes the PKCE verifier and `state`,
//!    and builds the authorize URL. The host opens it in the system browser.
//! 2. The browser comes back through the `deetshealth(-dev)://callback` link. [`check_link`]
//!    tells the host whether the link belongs to the waiting sign-in.
//! 3. [`exchange`] trades the code for tokens and returns the site's row, ready to seal.
//! 4. [`refresh`] renews the access token; when the site refuses, the user signs in again.
//!
//! The core never reads the clock (it has none on the web): the caller passes `now`, in
//! seconds since 1970.

use crate::net::{self, Field, Net, Text};
use crate::platform::Cipher;
use crate::store::SiteRow;
use crate::{CoreError, Shielded};
use serde_json::Value;
use zeroize::Zeroizing;

/// Every patient read scope, plus identity and a refresh token (CORE-CRATE.md §6), in SMART
/// v1 words.
pub const SCOPES: &str = "openid fhirUser launch/patient offline_access patient/*.read";
/// The same, in SMART v2 words (`.rs`: read and search). For a site registered with v2 scopes.
pub const SCOPES_V2: &str = "openid fhirUser launch/patient offline_access patient/*.rs";
/// Epic's sandbox (integrations/EPIC.md §3). No trailing slash: it is also the `aud`.
pub const EPIC_SANDBOX: &str = "https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4";
/// A waiting sign-in older than this is dropped.
pub const WAIT_LIMIT_S: u64 = 10 * 60;
/// Refresh this long before the access token runs out (clock-skew margin).
pub const REFRESH_MARGIN_S: u64 = 120;

/// A site the Connect menu offers. Sandbox entries are public fakes, fixed in the app.
pub struct SiteEntry {
    pub name: &'static str,
    pub base_url: String,
    pub client_id: &'static str,
    /// The scope string this site's registration reads: [`SCOPES`] or [`SCOPES_V2`].
    pub scopes: &'static str,
}

/// SMART Health IT's launcher, set up as a patient standalone launch with its patient picker,
/// a public client, and PKCE always required. The launcher reads its setup from a
/// base64url JSON array in the URL: [launch type 3 = patient-standalone, patient, provider,
/// encounter, skip login, skip auth, simulated EHR, scope, redirect URIs, client id, client
/// secret, simulated error, JWKS URL, JWKS, client type 0 = public, PKCE 2 = always].
pub const SMART_LAUNCH: &str = r#"[3,"","","",0,0,0,"","","","","","","",0,2]"#;

pub fn sandbox_sites() -> Vec<SiteEntry> {
    let mut sites = vec![SiteEntry {
        name: "SMART Health IT sandbox",
        base_url: format!("https://launch.smarthealthit.org/v/r4/sim/{}/fhir", b64url(SMART_LAUNCH.as_bytes())),
        client_id: "deetshealth",
        scopes: SCOPES,
    }];
    // Epic's sandbox, only in a build made with `epic-client.json` at the repo root (build.rs;
    // EPIC.md §2). Its index follows SMART's, so a saved SMART site keeps its menu entry.
    if let Some(id) = option_env!("DEETSHEALTH_EPIC_CLIENT_ID").filter(|s| !s.is_empty()) {
        let v2 = option_env!("DEETSHEALTH_EPIC_SCOPES") == Some("v2");
        sites.push(SiteEntry {
            name: "Epic sandbox",
            base_url: EPIC_SANDBOX.to_owned(),
            client_id: id,
            scopes: if v2 { SCOPES_V2 } else { SCOPES },
        });
    }
    sites
}

/// A sign-in waiting for the browser to come back. Held in memory only, one at a time.
pub struct PendingSignIn {
    display_name: Text,
    base_url: Text,
    client_id: Text,
    redirect_uri: String,
    smart_config: Text,
    token_endpoint: Text,
    verifier: Text,
    state: Text,
    pub started_at: u64,
}

impl PendingSignIn {
    pub fn expired(&self, now: u64) -> bool {
        now.saturating_sub(self.started_at) > WAIT_LIMIT_S
    }
}

/// Read the site's SMART configuration, make PKCE and `state`, and build the authorize URL.
pub async fn begin<C: Cipher>(
    net: &Net,
    cipher: &C,
    entry: &SiteEntry,
    redirect_uri: &str,
    now: u64,
) -> Result<(PendingSignIn, Text), CoreError> {
    let base = Shielded::new(entry.base_url.clone());
    let reply = net.get_json(&net::join(&base, ".well-known/smart-configuration"), None).await?;
    if !reply.ok() {
        return Err(CoreError::Network);
    }
    let (authorize, token_endpoint) = net::parse_reply(&reply.body, |v| {
        let https = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .filter(|s| s.starts_with("https://"))
                .map(|s| Shielded::new(s.to_owned()))
                .ok_or(CoreError::Parse)
        };
        Ok((https("authorization_endpoint")?, https("token_endpoint")?))
    })?;

    // The verifier is made here, so its plaintext exists only in this block, until wrapped.
    let (verifier, challenge) = {
        let v = Zeroizing::new(b64url(&random_bytes(cipher, 32)?));
        let c = b64url(&cipher.sha256(v.as_bytes())?);
        (Shielded::new(v.to_string()), Shielded::new(c))
    };
    let state = Shielded::new(b64url(&random_bytes(cipher, 16)?));
    let client_id = Shielded::new(entry.client_id.to_owned());

    let url = net::url_with(
        &authorize,
        &[
            ("response_type", Field::Plain("code")),
            ("client_id", Field::Shielded(&client_id)),
            ("redirect_uri", Field::Plain(redirect_uri)),
            ("scope", Field::Plain(entry.scopes)),
            ("state", Field::Shielded(&state)),
            ("aud", Field::Shielded(&base)),
            ("code_challenge", Field::Shielded(&challenge)),
            ("code_challenge_method", Field::Plain("S256")),
        ],
    )?;
    let pending = PendingSignIn {
        display_name: Shielded::new(entry.name.to_owned()),
        base_url: base,
        client_id,
        redirect_uri: redirect_uri.to_owned(),
        smart_config: reply.body,
        token_endpoint,
        verifier,
        state,
        started_at: now,
    };
    Ok((pending, url))
}

/// What a returning link means for the waiting sign-in.
pub enum LinkMatch {
    /// Our `state`, with a code: go on to [`exchange`].
    Ours(Text),
    /// Our `state`, but the site refused (the user said no, or an error). The sign-in ends.
    Refused,
    /// Not our `state`, or not a sign-in link at all: drop the link, keep waiting.
    NotOurs,
}

/// Match a returning link to the waiting sign-in. The link is shielded by the host the moment
/// it arrives; it holds the code.
pub fn check_link(pending: &PendingSignIn, link: &Text) -> LinkMatch {
    let parsed = net::parse_link(link, |url| {
        let get = |k: &str| url.query_pairs().find(|(n, _)| n == k).map(|(_, v)| Shielded::new(v.into_owned()));
        Ok((get("state"), get("code"), url.query_pairs().any(|(n, _)| n == "error")))
    });
    match parsed {
        Ok((Some(state), code, error)) if state == pending.state => match (code, error) {
            (Some(code), false) => LinkMatch::Ours(code),
            _ => LinkMatch::Refused,
        },
        _ => LinkMatch::NotOurs,
    }
}

/// Trade the code for tokens. Returns the site's row: everything sealed on its first write.
pub async fn exchange(net: &Net, pending: PendingSignIn, code: Text, now: u64) -> Result<SiteRow, CoreError> {
    let reply = net
        .post_form(
            &pending.token_endpoint,
            &[
                ("grant_type", Field::Plain("authorization_code")),
                ("code", Field::Shielded(&code)),
                ("redirect_uri", Field::Plain(&pending.redirect_uri)),
                ("client_id", Field::Shielded(&pending.client_id)),
                ("code_verifier", Field::Shielded(&pending.verifier)),
            ],
        )
        .await?;
    if !reply.ok() {
        return Err(CoreError::SignIn);
    }
    let t = read_tokens(&reply.body, now)?;
    let patient = t.patient.ok_or(CoreError::SignIn)?;
    Ok(SiteRow {
        display_name: Some(pending.display_name),
        base_url: Some(pending.base_url),
        client_id: Some(pending.client_id),
        patient_fhir_id: Some(patient),
        smart_config: Some(pending.smart_config),
        scopes: t.scope,
        access_token: Some(t.access_token),
        refresh_token: t.refresh_token,
        token_expiry: Some(t.expiry),
        ..Default::default()
    })
}

/// The Connect menu's entry that a saved site came from, found by its base URL. `None`: the
/// entry is no longer in the menu. Compared shielded; nothing is revealed.
pub fn entry_for(site: &SiteRow) -> Option<SiteEntry> {
    let base = site.base_url.as_ref()?;
    sandbox_sites().into_iter().find(|e| Shielded::new(e.base_url.clone()) == *base)
}

/// A new sign-in to a site that is already saved (the row's **Sign in**). If it opened the
/// same patient, the old row takes the new tokens and keeps what the fetch learned
/// ([`Renewal::Same`]). A different patient is [`Renewal::Other`]: the caller saves it as a
/// new site, so two records never mix in one file.
pub fn renew_site(old: SiteRow, new: SiteRow) -> Renewal {
    if old.patient_fhir_id.is_none() || old.patient_fhir_id != new.patient_fhir_id {
        return Renewal::Other(new);
    }
    Renewal::Same(SiteRow {
        capability: old.capability,
        software_version: old.software_version,
        last_fetch: old.last_fetch,
        last_full_fetch: old.last_full_fetch,
        ..new
    })
}

/// What [`renew_site`] found.
pub enum Renewal {
    /// The same patient: the saved row with the new tokens.
    Same(SiteRow),
    /// Another patient: the new row, to save as a new site.
    Other(SiteRow),
}

/// True when the access token is missing or runs out within the margin.
pub fn needs_refresh(site: &SiteRow, now: u64) -> bool {
    match site.token_expiry.as_ref().and_then(net::read_u64) {
        None => true,
        Some(at) => now + REFRESH_MARGIN_S >= at,
    }
}

/// What a refresh ended in.
pub enum Refresh {
    /// New tokens are in the row; seal it.
    Renewed,
    /// No refresh token, or the site refused it: the Sites row says "Sign in again".
    SignInAgain,
}

/// Renew the access token in `site`. A network failure is an error (try again later); a
/// refusal is [`Refresh::SignInAgain`].
pub async fn refresh(net: &Net, site: &mut SiteRow, now: u64) -> Result<Refresh, CoreError> {
    let (Some(refresh_token), Some(client_id), Some(config)) = (&site.refresh_token, &site.client_id, &site.smart_config) else {
        return Ok(Refresh::SignInAgain);
    };
    let token_endpoint = net::parse_reply(config, |v| {
        v.get("token_endpoint")
            .and_then(Value::as_str)
            .filter(|s| s.starts_with("https://"))
            .map(|s| Shielded::new(s.to_owned()))
            .ok_or(CoreError::Parse)
    })?;
    let reply = net
        .post_form(
            &token_endpoint,
            &[
                ("grant_type", Field::Plain("refresh_token")),
                ("refresh_token", Field::Shielded(refresh_token)),
                ("client_id", Field::Shielded(client_id)),
            ],
        )
        .await?;
    if matches!(reply.status, 400 | 401) {
        return Ok(Refresh::SignInAgain);
    }
    if !reply.ok() {
        return Err(CoreError::Network);
    }
    let t = read_tokens(&reply.body, now)?;
    site.access_token = Some(t.access_token);
    site.token_expiry = Some(t.expiry);
    if t.refresh_token.is_some() {
        site.refresh_token = t.refresh_token;
    }
    if t.scope.is_some() {
        site.scopes = t.scope;
    }
    Ok(Refresh::Renewed)
}

struct Tokens {
    access_token: Text,
    refresh_token: Option<Text>,
    scope: Option<Text>,
    patient: Option<Text>,
    /// Seconds since 1970, as decimal text.
    expiry: Text,
}

/// Read a token reply (SMART App Launch §token response).
fn read_tokens(body: &Text, now: u64) -> Result<Tokens, CoreError> {
    net::parse_reply(body, |v| {
        let s = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| Shielded::new(s.to_owned()));
        let bearer = v.get("token_type").and_then(Value::as_str).is_some_and(|t| t.eq_ignore_ascii_case("bearer"));
        if !bearer {
            return Err(CoreError::SignIn);
        }
        // No expires_in: assume the SMART default of one hour, and refresh early.
        let expires_in = v.get("expires_in").and_then(Value::as_u64).unwrap_or(3600);
        Ok(Tokens {
            access_token: s("access_token").ok_or(CoreError::SignIn)?,
            refresh_token: s("refresh_token"),
            scope: s("scope"),
            patient: s("patient"),
            expiry: Shielded::new((now + expires_in).to_string()),
        })
    })
}

/// `n` random bytes, wiped on drop: the PKCE verifier (32 bytes → 43 base64url characters,
/// within RFC 7636's 43–128) and `state` (16 bytes).
fn random_bytes<C: Cipher>(cipher: &C, n: usize) -> Result<Zeroizing<Vec<u8>>, CoreError> {
    let mut b = Zeroizing::new(vec![0u8; n]);
    cipher.random(&mut b)?;
    Ok(b)
}

/// Unpadded base64url (RFC 4648 §5).
pub fn b64url(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::SitePass;

    fn t(s: &str) -> Text {
        Shielded::new(s.to_owned())
    }

    /// SMART is always first. Epic follows only in a build that read `epic-client.json`, with
    /// the scope words of its registration.
    #[test]
    fn epic_entry_follows_the_build() {
        let sites = sandbox_sites();
        assert_eq!(sites[0].name, "SMART Health IT sandbox");
        match option_env!("DEETSHEALTH_EPIC_CLIENT_ID").filter(|s| !s.is_empty()) {
            None => assert_eq!(sites.len(), 1),
            Some(id) => {
                assert_eq!(sites.len(), 2);
                assert_eq!(sites[1].client_id, id);
                assert_eq!(sites[1].base_url, EPIC_SANDBOX);
                let v2 = option_env!("DEETSHEALTH_EPIC_SCOPES") == Some("v2");
                assert_eq!(sites[1].scopes, if v2 { SCOPES_V2 } else { SCOPES });
            }
        }
    }
    fn show(v: &Text) -> String {
        v.reveal(&SitePass::for_tests()).clone()
    }
    fn pending(state: &str) -> PendingSignIn {
        PendingSignIn {
            display_name: t("SMART Health IT sandbox"),
            base_url: t("https://sandbox.example/fhir"),
            client_id: t("deetshealth"),
            redirect_uri: "deetshealth-dev://callback".into(),
            smart_config: t("{}"),
            token_endpoint: t("https://sandbox.example/token"),
            verifier: t("v"),
            state: t(state),
            started_at: 1000,
        }
    }

    #[test]
    fn base64url_matches_rfc_4648() {
        for (plain, enc) in [("", ""), ("f", "Zg"), ("fo", "Zm8"), ("foo", "Zm9v"), ("foob", "Zm9vYg"), ("fooba", "Zm9vYmE"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(b64url(plain.as_bytes()), enc);
        }
        assert_eq!(b64url(&[0xfb, 0xff]), "-_8", "the URL-safe alphabet");
    }

    #[test]
    fn the_sandbox_entry_is_a_patient_standalone_launch() {
        let s = &sandbox_sites()[0];
        assert!(s.base_url.starts_with("https://launch.smarthealthit.org/v/r4/sim/"));
        assert!(s.base_url.ends_with("/fhir"));
    }

    #[test]
    fn links_are_matched_by_state() {
        let p = pending("s1");
        let ours = |q: &str| check_link(&p, &t(&format!("deetshealth-dev://callback/?{q}")));
        match ours("code=c1&state=s1") {
            LinkMatch::Ours(code) => assert_eq!(show(&code), "c1"),
            _ => panic!("our link"),
        }
        assert!(matches!(ours("code=c1&state=other"), LinkMatch::NotOurs));
        assert!(matches!(ours("code=c1"), LinkMatch::NotOurs));
        assert!(matches!(ours("error=access_denied&state=s1"), LinkMatch::Refused));
        assert!(matches!(ours("state=s1"), LinkMatch::Refused));
        assert!(matches!(check_link(&p, &t("not a link")), LinkMatch::NotOurs));
    }

    #[test]
    fn a_new_sign_in_renews_only_the_same_patient() {
        let row = |pid: &str, fetched: Option<&str>| SiteRow {
            patient_fhir_id: Some(t(pid)),
            access_token: Some(t(&format!("token-{pid}"))),
            last_fetch: fetched.map(t),
            ..Default::default()
        };
        let Renewal::Same(merged) = renew_site(row("p1", Some("100")), row("p1", None)) else { panic!("same patient") };
        assert_eq!(show(merged.last_fetch.as_ref().unwrap()), "100");
        assert_eq!(show(merged.access_token.as_ref().unwrap()), "token-p1");
        assert!(matches!(renew_site(row("p1", Some("100")), row("p2", None)), Renewal::Other(_)));

        let saved = SiteRow { base_url: Some(t(&sandbox_sites()[0].base_url)), ..Default::default() };
        assert!(entry_for(&saved).is_some());
        assert!(entry_for(&SiteRow { base_url: Some(t("https://elsewhere.example")), ..Default::default() }).is_none());
    }

    #[test]
    fn a_waiting_sign_in_expires() {
        let p = pending("s");
        assert!(!p.expired(1000 + WAIT_LIMIT_S));
        assert!(p.expired(1001 + WAIT_LIMIT_S));
    }

    #[test]
    fn token_replies() {
        let ok = t(r#"{"access_token":"a","token_type":"Bearer","expires_in":300,"refresh_token":"r","scope":"x","patient":"p"}"#);
        let tk = read_tokens(&ok, 1000).unwrap();
        assert_eq!(show(&tk.access_token), "a");
        assert_eq!(show(&tk.expiry), "1300");
        assert_eq!(show(tk.patient.as_ref().unwrap()), "p");
        assert!(read_tokens(&t(r#"{"access_token":"a","token_type":"mac"}"#), 0).is_err());
        assert!(read_tokens(&t(r#"{"token_type":"bearer"}"#), 0).is_err());
        assert!(read_tokens(&t("not json"), 0).is_err());
        let no_expiry = read_tokens(&t(r#"{"access_token":"a","token_type":"bearer"}"#), 0).unwrap();
        assert_eq!(show(&no_expiry.expiry), "3600");
    }

    #[test]
    fn refresh_comes_early() {
        let site = SiteRow { token_expiry: Some(t("10000")), ..Default::default() };
        assert!(!needs_refresh(&site, 10000 - REFRESH_MARGIN_S - 1));
        assert!(needs_refresh(&site, 10000 - REFRESH_MARGIN_S));
        assert!(needs_refresh(&SiteRow::default(), 0));
    }
}
