// Sign-in, the host half (CORE-CRATE.md §6). The core does the SMART work (core/src/auth.rs);
// this file holds the one waiting sign-in, opens the browser, takes the link back, and seals
// the new site.
//
// The link comes back to a loopback listener (loopback.rs), one per sign-in, on a port Windows
// picks: `http://127.0.0.1:<port>/callback?code=…&state=…` (the owner, 2026-10-13; it replaced
// the `deetshealth://` scheme, which Epic's sign-in page cannot hand off to). The link holds
// the code, so it is shielded on arrival and never logged: a log line says only what happened.
//
// Events to the front end carry no protected data: `sign-in` with a fixed status word
// (`payload::SignInStatus`), `sites-changed` with no payload, and `app-error` with a fixed
// code on every failure (errors.rs).
//
// Two ways in: **Connect** (a site from the menu, saved as a new site) and a row's **Sign in**
// (a saved site whose tokens are dead). The second renews the saved site in place when the
// same patient comes back; another patient is saved as a new site (`auth::renew_site`).

use crate::browser::SystemBrowser;
use crate::cng::Cng;
use crate::errors::{self, AppError};
use crate::log;
use crate::loopback;
use crate::sites::SiteFiles;
use deetshealth_core::auth::{self, LinkMatch, PendingSignIn, Renewal, SiteEntry};
use deetshealth_core::net::{self, Net, Text};
use deetshealth_core::payload::SignInStatus;
use deetshealth_core::platform::SiteKey;
use deetshealth_core::store::SiteRow;
use deetshealth_core::CoreError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub struct SignIn {
    net: Net,
    files: SiteFiles,
    pending: Mutex<Option<Waiting>>,
}

/// The one sign-in waiting for the browser.
struct Waiting {
    pending: PendingSignIn,
    /// The saved site a row's **Sign in** renews; `None` for Connect.
    renew: Option<SiteKey>,
    /// Closes this sign-in's return listener when the wait ends in any way.
    _stop: StopOnDrop,
}

/// Sets the listener's stop flag when it drops: on Cancel, on a new sign-in that replaces this
/// one, when the link is taken, and when the wait runs out.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// What became of a link that reached the return listener. The listener shows a page for it.
pub enum LinkOutcome {
    /// Ours: the token trade has started.
    Accepted,
    /// Ours, but the site did not grant access.
    Refused,
    /// No sign-in waits (cancelled, replaced, or the wait ran out).
    NotWaiting,
    /// Another `state`: the waiting sign-in goes on.
    NotOurs,
}

impl SignIn {
    pub fn new(files: SiteFiles) -> Result<Self, deetshealth_core::CoreError> {
        Ok(Self { net: Net::new()?, files, pending: Mutex::new(None) })
    }

    /// The HTTP client, shared with the fetch.
    pub fn net(&self) -> &Net {
        &self.net
    }

    /// The site files, shared with the fetch.
    pub fn files(&self) -> &SiteFiles {
        &self.files
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// What the Sites section shows while a sign-in runs. Fixed words only.
fn status(app: &AppHandle, word: SignInStatus) {
    let _ = app.emit("sign-in", word);
}

/// Report a failed start (errors.rs) and hand the same code back to the caller. The caller
/// does not toast it: the `app-error` event already does.
fn start_failed(app: &AppHandle, e: AppError, code: &'static str) -> String {
    errors::report(app, e);
    code.to_owned()
}

/// Start a sign-in to one of the Connect menu's sites. A new start replaces a waiting one.
/// On failure it returns the `app-error` code.
#[tauri::command]
pub async fn sign_in_start(app: AppHandle, site: usize) -> Result<(), String> {
    let entry = auth::sandbox_sites().into_iter().nth(site);
    let Some(entry) = entry else { return Err(start_failed(&app, AppError::new("sign-in-no-site"), "sign-in-no-site")) };
    begin(&app, &entry, None).await
}

/// A saved site's **Sign in** half: its tokens are dead. `site` is the app's own site key.
/// On failure it returns the `app-error` code.
#[tauri::command]
pub async fn site_sign_in(app: AppHandle, site: String) -> Result<(), String> {
    let fail = |code: &'static str| start_failed(&app, AppError::new(code).site(&site), code);
    let key = SiteKey::from_stored(site.clone()).map_err(|_| fail("site-unknown"))?;
    // The store is file I/O: off the UI thread.
    let app2 = app.clone();
    let key2 = key.clone();
    let row = tauri::async_runtime::spawn_blocking(move || -> Result<Option<SiteRow>, &'static str> {
        let s = app2.try_state::<SignIn>().ok_or("sign-in-unavailable")?;
        let store = s.files.open(&key2).map_err(|_| "site-unreadable")?.ok_or("site-key-missing")?;
        store.site().map_err(|_| "site-unreadable")
    })
    .await
    .map_err(|_| fail("sign-in-failed"))?
    .map_err(fail)?;
    let entry = row.as_ref().and_then(auth::entry_for).ok_or_else(|| fail("sign-in-no-site"))?;
    begin(&app, &entry, Some(key)).await
}

/// Start a sign-in and open the browser. A new start replaces a waiting one.
async fn begin(app: &AppHandle, entry: &SiteEntry, renew: Option<SiteKey>) -> Result<(), String> {
    let fail = |code: &'static str| start_failed(app, AppError::new(code), code);
    let s = app.try_state::<SignIn>().ok_or_else(|| fail("sign-in-unavailable"))?;
    // The return listener first: the redirect URI names its port.
    let bound = match loopback::Bound::open() {
        Ok(b) => b,
        Err(e) => {
            log::warn(&format!("sign-in: could not open a local port for the return: {}", e.kind()));
            return Err(fail("sign-in-port"));
        }
    };
    let redirect_uri = bound.redirect_uri();
    let (pending, url) = match auth::begin(&s.net, &Cng, entry, &redirect_uri, now()).await {
        Ok(p) => p,
        Err(e) => {
            let code = match e {
                CoreError::Network => "sign-in-site-unreachable",
                CoreError::Parse => "sign-in-site-unreadable",
                _ => "sign-in-failed",
            };
            log::warn(&format!("sign-in: could not start: {e}"));
            return Err(start_failed(app, AppError::new(code).kind(e), code));
        }
    };
    let renewing = renew.is_some();
    let stop = Arc::new(AtomicBool::new(false));
    // Replacing an older waiting sign-in drops it, which closes its listener.
    *s.pending.lock().map_err(|_| fail("sign-in-failed"))? = Some(Waiting { pending, renew, _stop: StopOnDrop(stop.clone()) });
    bound.serve(app.clone(), stop, Duration::from_secs(auth::WAIT_LIMIT_S));
    if let Err(e) = net::open_in_browser(&SystemBrowser, &url) {
        log::warn(&format!("sign-in: {e}"));
        return Err(start_failed(app, AppError::new("sign-in-browser").kind(e), "sign-in-browser"));
    }
    log::info(if renewing { "sign-in: started again for a saved site; waiting for the browser" } else { "sign-in: started; waiting for the browser" });
    status(app, SignInStatus::Waiting);
    Ok(())
}

/// Cancel the waiting sign-in (the Sites row's Cancel half).
#[tauri::command]
pub fn sign_in_cancel(app: AppHandle) {
    if let Some(s) = app.try_state::<SignIn>() {
        if let Ok(mut p) = s.pending.lock() {
            if p.take().is_some() {
                log::info("sign-in: cancelled");
            }
        }
    }
    status(&app, SignInStatus::Cancelled);
}

/// A link reached the return listener (loopback.rs). It arrives shielded.
pub fn on_link(app: &AppHandle, link: Text) -> LinkOutcome {
    let Some(s) = app.try_state::<SignIn>() else { return LinkOutcome::NotWaiting };
    let taken = {
        let Ok(mut slot) = s.pending.lock() else { return LinkOutcome::NotWaiting };
        match slot.as_ref() {
            None => {
                log::warn("sign-in: a link arrived, but no sign-in was waiting; ignored");
                errors::report(app, AppError::new("sign-in-stray-link"));
                return LinkOutcome::NotWaiting;
            }
            Some(w) if w.pending.expired(now()) => {
                *slot = None;
                log::warn("sign-in: a link arrived after the wait ran out; ignored");
                status(app, SignInStatus::Expired);
                errors::report(app, AppError::new("sign-in-expired"));
                return LinkOutcome::NotWaiting;
            }
            Some(w) => match auth::check_link(&w.pending, &link) {
                LinkMatch::NotOurs => {
                    log::warn("sign-in: a link with the wrong state arrived; ignored");
                    errors::report(app, AppError::new("sign-in-wrong-link"));
                    return LinkOutcome::NotOurs;
                }
                LinkMatch::Refused => {
                    *slot = None;
                    log::info("sign-in: the site did not grant access");
                    status(app, SignInStatus::Refused);
                    errors::report(app, AppError::new("sign-in-refused"));
                    return LinkOutcome::Refused;
                }
                LinkMatch::Ours(code) => (slot.take(), code),
            },
        }
    };
    // Its `_stop` drops here, which tells the listener the wait is over.
    let (Some(Waiting { pending, renew, .. }), code) = taken else { return LinkOutcome::NotWaiting };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let s = app.state::<SignIn>();
        let row = match auth::exchange(&s.net, pending, code, now()).await {
            Ok(row) => row,
            Err(e) => {
                log::warn(&format!("sign-in: the token trade failed: {e}"));
                status(&app, SignInStatus::Failed);
                let code = if e == CoreError::SignIn { "sign-in-incomplete" } else { "sign-in-trade-failed" };
                errors::report(&app, AppError::new(code).kind(e));
                return;
            }
        };
        let app2 = app.clone();
        let sealed = tauri::async_runtime::spawn_blocking(move || save(&app2.state::<SignIn>().files, row, renew)).await;
        match sealed {
            Ok(Ok(site)) => {
                log::info("sign-in: connected; the site is sealed");
                status(&app, SignInStatus::Connected);
                let _ = app.emit("sites-changed", ());
                crate::fetch::start(&app, site);
            }
            Ok(Err(e)) => {
                log::error(&format!("sign-in: could not seal the new site: {e}"));
                status(&app, SignInStatus::Failed);
                errors::report(&app, AppError::new("site-seal-failed").kind(e));
            }
            Err(_) => {
                log::error("sign-in: could not seal the new site: the task stopped");
                status(&app, SignInStatus::Failed);
                errors::report(&app, AppError::new("site-seal-failed"));
            }
        }
    });
    LinkOutcome::Accepted
}

/// Seal a signed-in site: into the saved site it renews when the same patient came back,
/// else as a new site. A saved site that will not open (erased meanwhile) gets a new one too.
fn save(files: &SiteFiles, row: SiteRow, renew: Option<SiteKey>) -> Result<SiteKey, CoreError> {
    let old = renew.and_then(|key| {
        let store = files.open(&key).ok()??;
        let old = store.site().ok()??;
        Some((key, store, old))
    });
    let row = match old {
        Some((key, store, old)) => match auth::renew_site(old, row) {
            Renewal::Same(merged) => {
                store.put_site(&merged)?;
                log::info("sign-in: the saved site is renewed");
                return Ok(key);
            }
            Renewal::Other(row) => {
                log::info("sign-in: another patient came back; saved as a new site");
                row
            }
        },
        None => row,
    };
    let (site, store) = files.create()?;
    store.put_site(&row)?;
    Ok(site)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use deetshealth_core::auth::{b64url, SiteEntry};
    use deetshealth_core::Shielded;
    use deetshealth_core::platform::OpenBrowser;
    use deetshealth_core::store::SiteRow;

    /// Catches the authorize URL instead of opening a browser.
    struct Catch(std::cell::RefCell<String>);
    impl OpenBrowser for Catch {
        fn open_url(&self, url: &str) -> Result<(), deetshealth_core::CoreError> {
            *self.0.borrow_mut() = url.to_owned();
            Ok(())
        }
    }

    /// Sign in to the live SMART sandbox with no browser: the launcher is set to pick a
    /// patient and skip its login, so the authorize step answers with the code at once. Real
    /// CNG (the launcher requires PKCE). About five calls. Checks the `state` rule on the way.
    pub(crate) async fn headless_sign_in(net: &Net) -> SiteRow {
        // A sandbox patient id: a public fake, read from the open server.
        let list: serde_json::Value = reqwest::get("https://r4.smarthealthit.org/Patient?_count=1&_elements=id")
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let pid = list["entry"][0]["resource"]["id"].as_str().unwrap().to_owned();
        let launch = format!(r#"[3,"{pid}","","",1,1,0,"","","","","","","",0,2]"#);
        let entry = SiteEntry {
            name: "SMART Health IT sandbox",
            base_url: format!("https://launch.smarthealthit.org/v/r4/sim/{}/fhir", b64url(launch.as_bytes())),
            client_id: "deetshealth",
            scopes: deetshealth_core::auth::SCOPES,
        };

        let (pending, url) = auth::begin(net, &Cng, &entry, "http://127.0.0.1:1/callback", now()).await.unwrap();
        let catch = Catch(Default::default());
        net::open_in_browser(&catch, &url).unwrap();
        let authorize = catch.0.borrow().clone();
        assert!(authorize.contains("code_challenge_method=S256"));

        let no_redirect = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
        let r = no_redirect.get(&authorize).send().await.unwrap();
        let link = r.headers()["location"].to_str().unwrap().to_owned();
        assert!(link.starts_with("http://127.0.0.1:1/callback"));

        let wrong = Shielded::new(link.replace("state=", "state=x"));
        assert!(matches!(auth::check_link(&pending, &wrong), LinkMatch::NotOurs));
        let LinkMatch::Ours(code) = auth::check_link(&pending, &Shielded::new(link)) else { panic!("our link") };
        auth::exchange(net, pending, code, now()).await.unwrap()
    }

    /// The whole sign-in against the live SMART sandbox, then a refresh and a refused
    /// refresh. Run with `cargo test -p deetshealth --lib signin -- --ignored`: about 7 calls.
    #[test]
    #[ignore]
    fn sign_in_and_refresh_against_the_smart_sandbox() {
        tauri::async_runtime::block_on(async {
            let net = Net::new().unwrap();
            let mut row = headless_sign_in(&net).await;
            assert!(row.access_token.is_some() && row.patient_fhir_id.is_some() && row.base_url.is_some());
            assert!(row.client_id.is_some() && row.smart_config.is_some());
            assert!(row.refresh_token.is_some(), "offline_access gives a refresh token");

            assert!(matches!(auth::refresh(&net, &mut row, now()).await.unwrap(), auth::Refresh::Renewed));
            row.refresh_token = Some(Shielded::new("not-a-token".into()));
            assert!(matches!(auth::refresh(&net, &mut row, now()).await.unwrap(), auth::Refresh::SignInAgain));
        });
    }
}

