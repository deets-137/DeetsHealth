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
        #[cfg(not(target_arch = "wasm32"))]
        let b = b
            .https_only(true)
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
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
