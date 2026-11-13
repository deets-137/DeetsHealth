//! The text message: the verification code, sent to the number on file through Twilio's
//! Messages API (SECURITY.md §4.4; integrations/TWILIO.md; CHAT.md §8).
//!
//! The same request in every place it runs. In dev it goes to the fake Twilio in the agent's
//! dev server (on 127.0.0.1), which shows the text on the Chat page. From Phase 5 it goes to
//! Twilio itself: only the settings change (BUILD-ORDER.md).
//!
//! This is the one file that holds a `TextPass` (core/src/text.rs): the number and the code are
//! revealed here, for the request body only. Errors are fixed strings; a reply from the text
//! service is never read beyond its status.
//!
//! Settings, from the environment (never a flag: a flag shows in the process list):
//! - `DEETS_TWILIO_ACCOUNT_SID` — `AC` and 32 hex digits. Unset: no texts, the fake code.
//! - `DEETS_TWILIO_AUTH_TOKEN` — the account's auth token.
//! - `DEETS_TWILIO_MESSAGING_SERVICE` — `MG` and 32 hex digits: the sender.
//! - `DEETS_TWILIO_API` — the API's base URL. Default `https://api.twilio.com`. Plain `http`
//!   is refused unless the host is this machine (the fake Twilio).

use std::time::Duration;

use deetshealth_core::{text::TextPass, Shielded};
use reqwest::{redirect::Policy, Url};
use rustls::{ClientConfig, RootCertStore};
use zeroize::Zeroizing;

use crate::{
    transport,
    verify::{Codes, Down},
};

/// The prompt file: the text's words live with every other word the caller reads or hears, so
/// the owner edits them in one place (agent/prompts/AGENT-PROMPT.md › `text.code`).
const PROMPT_FILE: &str = include_str!("../../agent/prompts/AGENT-PROMPT.md");
const SECTION: &str = "text.code";
const DEFAULT_API: &str = "https://api.twilio.com";
/// The longest the caller waits for the text service. Over this, the agent says it could not
/// send the code.
const SEND_LIMIT: Duration = Duration::from_secs(5);

/// A code made from the OS's random source: six digits, each value equally likely.
pub fn random_code() -> Shielded<String> {
    // 4_294_000_000 is the largest multiple of 1_000_000 that fits in a u32. A draw at or above
    // it is drawn again, so `% 1_000_000` favors no code.
    loop {
        let mut b = [0u8; 4];
        getrandom::fill(&mut b).expect("the OS random source");
        let n = u32::from_le_bytes(b);
        if n < 4_294_000_000 {
            return Shielded::new(format!("{:06}", n % 1_000_000));
        }
    }
}

/// The text's words, with `{code}` where the code goes. Lines next to each other join with a
/// space and a blank line starts a paragraph, as in prompts.py.
pub fn template() -> Result<String, String> {
    let mut lines = PROMPT_FILE.lines().map(|l| l.trim_end_matches('\r'));
    let heading = format!("## {SECTION}");
    if !lines.any(|l| l.trim_end() == heading) {
        return Err(format!("AGENT-PROMPT.md has no section '{heading}'"));
    }
    let mut paragraphs: Vec<String> = vec![];
    let mut current = String::new();
    for line in lines.take_while(|l| !l.starts_with("## ")) {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }
    let text = paragraphs.join("\n\n");
    if text.matches("{code}").count() != 1 || text.replace("{code}", "").contains(['{', '}']) {
        return Err(format!("AGENT-PROMPT.md › {SECTION} must hold {{code}} once and no other placeholder"));
    }
    Ok(text)
}

/// Where and as whom to send.
pub struct TwilioConfig {
    api: Url,
    account_sid: String,
    auth_token: Zeroizing<String>,
    messaging_service: String,
}

impl TwilioConfig {
    /// From the environment. `None` when no account is set: `ehr` then uses the fake code.
    pub fn from_env() -> Result<Option<Self>, String> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let Some(account_sid) = var("DEETS_TWILIO_ACCOUNT_SID") else {
            return Ok(None);
        };
        let auth_token = Zeroizing::new(var("DEETS_TWILIO_AUTH_TOKEN").ok_or("DEETS_TWILIO_AUTH_TOKEN is not set")?);
        let messaging_service =
            var("DEETS_TWILIO_MESSAGING_SERVICE").ok_or("DEETS_TWILIO_MESSAGING_SERVICE is not set")?;
        let api = var("DEETS_TWILIO_API").unwrap_or_else(|| DEFAULT_API.into());
        Self::new(&api, account_sid, auth_token, messaging_service).map(Some)
    }

    pub fn new(
        api: &str,
        account_sid: String,
        auth_token: Zeroizing<String>,
        messaging_service: String,
    ) -> Result<Self, String> {
        let api = Url::parse(api).map_err(|_| "DEETS_TWILIO_API is not a URL")?;
        let local = matches!(api.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if api.scheme() != "https" && !(api.scheme() == "http" && local) {
            return Err("DEETS_TWILIO_API must be https (plain http only to this machine)".into());
        }
        if !is_sid(&account_sid, "AC") {
            return Err("DEETS_TWILIO_ACCOUNT_SID is not AC and 32 hex digits".into());
        }
        if !is_sid(&messaging_service, "MG") {
            return Err("DEETS_TWILIO_MESSAGING_SERVICE is not MG and 32 hex digits".into());
        }
        if auth_token.is_empty() {
            return Err("DEETS_TWILIO_AUTH_TOKEN is empty".into());
        }
        Ok(Self { api, account_sid, auth_token, messaging_service })
    }

    /// For the start-up line: where texts go, never a secret.
    pub fn api(&self) -> &str {
        self.api.as_str()
    }
}

/// A Twilio id: its two-letter prefix, then 32 hex digits. Checked, so an id can never change
/// the request's path.
fn is_sid(s: &str, prefix: &str) -> bool {
    s.len() == 34 && s.starts_with(prefix) && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// The real code sender.
pub struct TwilioCodes {
    client: reqwest::Client,
    messages: Url,
    account_sid: String,
    auth_token: Zeroizing<String>,
    messaging_service: String,
    template: String,
}

impl TwilioCodes {
    pub fn new(config: TwilioConfig) -> Result<Self, String> {
        let template = template()?;
        let messages = config
            .api
            .join(&format!("2010-04-01/Accounts/{}/Messages.json", config.account_sid))
            .map_err(|_| "cannot build the Messages URL")?;
        // The same crypto as the wire (AWS-LC, FIPS in the deploy build), and Mozilla's roots.
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = ClientConfig::builder_with_provider(transport::provider())
            .with_safe_default_protocol_versions()
            .map_err(|_| "the TLS provider refused its own versions")?
            .with_root_certificates(roots)
            .with_no_client_auth();
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .timeout(SEND_LIMIT)
            .redirect(Policy::none())
            .build()
            .map_err(|_| "cannot build the HTTP client")?;
        Ok(Self {
            client,
            messages,
            account_sid: config.account_sid,
            auth_token: config.auth_token,
            messaging_service: config.messaging_service,
            template,
        })
    }
}

impl Codes for TwilioCodes {
    fn issue(&self) -> Shielded<String> {
        random_code()
    }

    async fn send(&self, phone: &Shielded<String>, code: &Shielded<String>, template: Option<&str>) -> Result<(), Down> {
        let pass = TextPass::issue();
        let words = template.unwrap_or(&self.template);
        let body = Zeroizing::new(words.replace("{code}", code.reveal(&pass)));
        let form = [
            ("To", phone.reveal(&pass).as_str()),
            ("MessagingServiceSid", self.messaging_service.as_str()),
            ("Body", body.as_str()),
        ];
        let reply = self
            .client
            .post(self.messages.clone())
            .basic_auth(&self.account_sid, Some(self.auth_token.as_str()))
            .form(&form)
            .send()
            .await
            .map_err(|_| Down)?;
        // 201 Created: Twilio has queued the text. Its body is never read.
        if reply.status().is_success() {
            Ok(())
        } else {
            Err(Down)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_six_digits_and_they_vary() {
        let pass = TextPass::issue();
        let codes: Vec<String> = (0..50).map(|_| random_code().reveal(&pass).clone()).collect();
        assert!(codes.iter().all(|c| c.len() == 6 && c.bytes().all(|b| b.is_ascii_digit())));
        let distinct: std::collections::HashSet<_> = codes.iter().collect();
        assert!(distinct.len() > 40, "50 draws gave {} codes", distinct.len());
    }

    /// One text segment: 160 characters at most, plain ASCII (GSM-7). A longer text costs two.
    #[test]
    fn the_words_fit_one_text() {
        let t = template().unwrap().replace("{code}", "123456");
        assert!(t.len() <= 160, "{} characters", t.len());
        assert!(t.is_ascii(), "a character outside GSM-7 sends the text as UCS-2, 70 to a segment");
    }

    #[test]
    fn settings_are_checked() {
        let sid = format!("AC{}", "0".repeat(32));
        let mg = format!("MG{}", "0".repeat(32));
        let token = || Zeroizing::new("t".to_string());
        assert!(TwilioConfig::new("https://api.twilio.com", sid.clone(), token(), mg.clone()).is_ok());
        assert!(TwilioConfig::new("http://127.0.0.1:9000", sid.clone(), token(), mg.clone()).is_ok());
        assert!(TwilioConfig::new("http://api.twilio.com", sid.clone(), token(), mg.clone()).is_err(), "plain http away");
        assert!(TwilioConfig::new("https://x", "AC/../../x".into(), token(), mg.clone()).is_err(), "a sid that bends the path");
        assert!(TwilioConfig::new("https://x", sid, token(), "MG1".into()).is_err());
    }

    /// The text pass is issued in this file only (core/src/text.rs).
    #[test]
    fn only_this_file_names_the_text_pass() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let needle = ["Text", "Pass"].concat();
        for entry in std::fs::read_dir(src).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == "texts.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains(&needle), "{} names the text pass", path.display());
        }
    }
}
