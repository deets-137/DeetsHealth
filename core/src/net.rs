//! Talking to a site (CORE-CRATE.md §4, the third pass: the owner, 2026-09-21).
//!
//! This module holds [`SitePass`], the third and last way to read a [`Shielded`] value. It
//! reveals a value only to put it into a request to the site it came from (the URL, the
//! `Authorization` header, a form field), and it reads each reply only inside
//! [`parse_reply`], where the caller wraps every value it keeps. Nothing here logs, and no
//! error carries a value (`CoreError` is kinds only).
//!
//! The client (desktop): HTTPS only, TLS 1.2 or later through the OS (SChannel on Windows),
//! no redirects (a token never follows a redirect to another host), gzip, a 30 s timeout.
//! On the web the browser's fetch does all of this.

use crate::platform::OpenBrowser;
use crate::{CoreError, Shielded};
use serde_json::Value;

/// The site I/O pass. Its field is private, so only this module can make one.
pub struct SitePass {
    _only_here: (),
}

impl SitePass {
    fn issue() -> Self {
        Self { _only_here: () }
    }
}

/// A protected text value (a URL, a token, a reply body).
pub type Text = Shielded<String>;

pub struct Net {
    client: reqwest::Client,
}

fn net<E>(_: E) -> CoreError {
    CoreError::Network
}

impl Net {
    pub fn new() -> Result<Self, CoreError> {
        let b = reqwest::Client::builder();
        // Without the `native-tls` feature (ehr's build) there is no TLS backend of core's own;
        // `https_only` then refuses every request.
        #[cfg(all(not(target_arch = "wasm32"), feature = "native-tls"))]
        let b = b.min_tls_version(reqwest::tls::Version::TLS_1_2);
        #[cfg(not(target_arch = "wasm32"))]
        let b = b
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(concat!("DeetsHealth/", env!("CARGO_PKG_VERSION")));
        Ok(Self { client: b.build().map_err(net)? })
    }

    /// GET a JSON document. `bearer` is the access token, when the site needs one.
    pub async fn get_json(&self, url: &Text, bearer: Option<&Text>) -> Result<Reply, CoreError> {
        let pass = SitePass::issue();
        let mut req = self.client.get(url.reveal(&pass).as_str()).header("Accept", "application/fhir+json, application/json");
        if let Some(t) = bearer {
            req = req.bearer_auth(t.reveal(&pass));
        }
        Reply::from(req.send().await.map_err(net)?).await
    }

    /// POST a form (the token endpoint). Values may be protected (a code, a refresh token)
    /// or fixed app text.
    pub async fn post_form(&self, url: &Text, form: &[(&str, Field<'_>)]) -> Result<Reply, CoreError> {
        let pass = SitePass::issue();
        let pairs: Vec<(&str, &str)> = form
            .iter()
            .map(|(k, v)| {
                (*k, match v {
                    Field::Plain(s) => *s,
                    Field::Shielded(t) => t.reveal(&pass).as_str(),
                })
            })
            .collect();
        let req = self.client.post(url.reveal(&pass).as_str()).header("Accept", "application/json").form(&pairs);
        Reply::from(req.send().await.map_err(net)?).await
    }
}

/// One form field for [`Net::post_form`].
pub enum Field<'a> {
    Plain(&'a str),
    Shielded(&'a Text),
}

/// A site's reply: its status and its body, the body shielded from the moment it arrives.
pub struct Reply {
    pub status: u16,
    /// `Retry-After` in seconds, when the site sent one (429, 503).
    pub retry_after: Option<u64>,
    pub body: Text,
}

impl Reply {
    async fn from(r: reqwest::Response) -> Result<Self, CoreError> {
        let status = r.status().as_u16();
        let retry_after = r.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|s| s.trim().parse().ok());
        let body = Shielded::new(r.text().await.map_err(net)?);
        Ok(Self { status, retry_after, body })
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Read a reply's JSON. `f` gets the parsed document for the length of the call and must wrap
/// in [`Shielded`] every value it keeps: this is "the moment it is parsed" (SECURITY.md §4.2).
/// Only core code can call it; the host never sees a reply.
pub(crate) fn parse_reply<R>(body: &Text, f: impl FnOnce(&Value) -> Result<R, CoreError>) -> Result<R, CoreError> {
    let v: Value = serde_json::from_str(body.reveal(&SitePass::issue())).map_err(|_| CoreError::Parse)?;
    f(&v)
}

/// Read the query of a link the site sent back (the sign-in return), as for [`parse_reply`].
pub(crate) fn parse_link<R>(link: &Text, f: impl FnOnce(&reqwest::Url) -> Result<R, CoreError>) -> Result<R, CoreError> {
    let url = reqwest::Url::parse(link.reveal(&SitePass::issue())).map_err(|_| CoreError::SignIn)?;
    f(&url)
}

/// A reply's size in bytes, for the call ledger (sync.rs). A length, never the content.
pub(crate) fn byte_len(t: &Text) -> usize {
    t.reveal(&SitePass::issue()).len()
}

/// The value when it is only ASCII digits, at most `max` of them: a site's message number
/// (fhir::issue_word). Anything else gives `None`, so no text gets out.
pub(crate) fn digits(t: &Text, max: usize) -> Option<String> {
    let s = t.reveal(&SitePass::issue());
    (!s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.clone())
}

/// The granted scope string, to decide which types to fetch. OAuth scopes are the site's
/// words for what we may read, not record data.
pub(crate) fn read_scopes(t: &Text) -> String {
    t.reveal(&SitePass::issue()).clone()
}

/// Whether `url` is on the site at `base`: a `next` link must be, or the token would go
/// elsewhere. The same scheme, host and port, and a path at or under the base path, split at
/// a `/` (so `/fhir2` is not under `/fhir`). A text prefix is not enough: with a base of
/// `https://a.org`, `https://a.org.other.net` would pass it.
pub(crate) fn on_site(url: &Text, base: &Text) -> bool {
    let p = SitePass::issue();
    let (Ok(u), Ok(b)) = (reqwest::Url::parse(url.reveal(&p)), reqwest::Url::parse(base.reveal(&p))) else {
        return false;
    };
    let base_path = b.path().trim_end_matches('/');
    u.scheme() == b.scheme()
        && u.host_str().is_some()
        && u.host_str() == b.host_str()
        && u.port_or_known_default() == b.port_or_known_default()
        && u.username().is_empty()
        && (u.path().trim_end_matches('/') == base_path || u.path().starts_with(&format!("{base_path}/")))
}

/// A stored reference (`Type/id`, as the site sent it) split into its plain FHIR type and its
/// shielded id, to find the row it points to.
pub(crate) fn split_ref(t: &Text) -> Option<(String, Text)> {
    let s = t.reveal(&SitePass::issue());
    crate::index::split_reference(s).map(|(ty, id)| (ty.to_owned(), Shielded::new(id.to_owned())))
}

/// A number kept from a site's reply (the token expiry, as decimal text).
pub(crate) fn read_u64(t: &Text) -> Option<u64> {
    t.reveal(&SitePass::issue()).parse().ok()
}

/// Build a URL from a protected base and plain or protected query pairs.
pub(crate) fn url_with(base: &Text, query: &[(&str, Field<'_>)]) -> Result<Text, CoreError> {
    let pass = SitePass::issue();
    let mut url = reqwest::Url::parse(base.reveal(&pass)).map_err(|_| CoreError::Parse)?;
    {
        let mut q = url.query_pairs_mut();
        for (k, v) in query {
            match v {
                Field::Plain(s) => q.append_pair(k, s),
                Field::Shielded(t) => q.append_pair(k, t.reveal(&pass)),
            };
        }
    }
    Ok(Shielded::new(String::from(url)))
}

/// A protected base URL with a path added, e.g. `.well-known/smart-configuration`.
pub(crate) fn join(base: &Text, path: &str) -> Text {
    let b = base.reveal(&SitePass::issue());
    Shielded::new(format!("{}/{}", b.trim_end_matches('/'), path.trim_start_matches('/')))
}

/// Hand the sign-in page to the system browser. The URL names the site, so it is revealed
/// only here, for the platform to open; the host must not keep or log it.
pub fn open_in_browser(browser: &impl OpenBrowser, url: &Text) -> Result<(), CoreError> {
    let u = url.reveal(&SitePass::issue());
    if !u.starts_with("https://") {
        return Err(CoreError::Browser);
    }
    browser.open_url(u)
}

#[cfg(test)]
impl SitePass {
    pub(crate) fn for_tests() -> Self {
        Self::issue()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(url: &str, base: &str) -> bool {
        on_site(&Shielded::new(url.into()), &Shielded::new(base.into()))
    }

    #[test]
    fn a_next_link_must_stay_on_the_site() {
        let base = "https://launch.example.org/v/r4/sim/abc/fhir";
        assert!(on("https://launch.example.org/v/r4/sim/abc/fhir?_getpages=x", base), "HAPI: the base itself");
        assert!(on("https://launch.example.org/v/r4/sim/abc/fhir/Observation?page=2", base));
        assert!(on("https://launch.example.org:443/v/r4/sim/abc/fhir/Observation", base), "the default port");
        assert!(!on("https://launch.example.org/v/r4/sim/abc/fhir2/Observation", base), "a path boundary");
        assert!(!on("https://launch.example.org/other/Observation", base));
        assert!(!on("http://launch.example.org/v/r4/sim/abc/fhir/Observation", base), "the scheme");
        assert!(!on("https://launch.example.org:8443/v/r4/sim/abc/fhir/Observation", base), "the port");
        assert!(!on("https://user@launch.example.org/v/r4/sim/abc/fhir/Observation", base));
        // The gap the text prefix had: a base URL with no path.
        assert!(on("https://fhir.example.org/Observation?page=2", "https://fhir.example.org"));
        assert!(!on("https://fhir.example.org.other.net/Observation", "https://fhir.example.org"));
        assert!(!on("not a url", base));
    }
}
