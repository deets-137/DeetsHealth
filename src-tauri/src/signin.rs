// Sign-in, the host half (CORE-CRATE.md §6). The core does the SMART work (core/src/auth.rs);
// this file holds the one waiting sign-in, opens the browser, takes the link back, and seals
// the new site.
//
// The link arrives as plain text in the process arguments. It holds the code, so it is
// shielded on arrival and never logged: a log line says only what happened.
//
// Events to the front end carry no protected data: `sign-in` with a fixed status word,
// `sites-changed` with no payload, and `app-error` with a fixed code on every failure
// (errors.rs).

use crate::browser::SystemBrowser;
use crate::cng::Cng;
use crate::errors::{self, AppError};
use crate::log;
use crate::sites::SiteFiles;
use deetshealth_core::auth::{self, LinkMatch, PendingSignIn};
use deetshealth_core::net::{self, Net};
use deetshealth_core::{CoreError, Shielded};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub struct SignIn {
    net: Net,
    files: SiteFiles,
    redirect_uri: String,
    pending: Mutex<Option<PendingSignIn>>,
}

impl SignIn {
    pub fn new(files: SiteFiles, scheme: &str) -> Result<Self, deetshealth_core::CoreError> {
        Ok(Self { net: Net::new()?, files, redirect_uri: format!("{scheme}://callback"), pending: Mutex::new(None) })
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
fn status(app: &AppHandle, word: &'static str) {
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
    let fail = |code: &'static str| start_failed(&app, AppError::new(code), code);
    let s = app.try_state::<SignIn>().ok_or_else(|| fail("sign-in-unavailable"))?;
    let entries = auth::sandbox_sites();
    let entry = entries.get(site).ok_or_else(|| fail("sign-in-no-site"))?;
    let (pending, url) = match auth::begin(&s.net, &Cng, entry, &s.redirect_uri, now()).await {
        Ok(p) => p,
        Err(e) => {
            let code = match e {
                CoreError::Network => "sign-in-site-unreachable",
                CoreError::Parse => "sign-in-site-unreadable",
                _ => "sign-in-failed",
            };
            log::warn(&format!("sign-in: could not start: {e}"));
            return Err(start_failed(&app, AppError::new(code).kind(e), code));
        }
    };
    *s.pending.lock().map_err(|_| fail("sign-in-failed"))? = Some(pending);
    if let Err(e) = net::open_in_browser(&SystemBrowser, &url) {
        log::warn(&format!("sign-in: {e}"));
        return Err(start_failed(&app, AppError::new("sign-in-browser").kind(e), "sign-in-browser"));
    }
    log::info("sign-in: started; waiting for the browser");
    status(&app, "waiting");
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
    status(&app, "cancelled");
}

/// A `deetshealth(-dev)://` link reached the app (a second launch, or the first one).
pub fn on_link(app: &AppHandle, link: String) {
    let link = Shielded::new(link);
    let Some(s) = app.try_state::<SignIn>() else { return };
    let taken = {
        let Ok(mut slot) = s.pending.lock() else { return };
        match slot.as_ref() {
            None => {
                log::warn("sign-in: a link arrived, but no sign-in was waiting; ignored");
                errors::report(app, AppError::new("sign-in-stray-link"));
                return;
            }
            Some(p) if p.expired(now()) => {
                *slot = None;
                log::warn("sign-in: a link arrived after the wait ran out; ignored");
                status(app, "expired");
                errors::report(app, AppError::new("sign-in-expired"));
                return;
            }
            Some(p) => match auth::check_link(p, &link) {
                LinkMatch::NotOurs => {
                    log::warn("sign-in: a link with the wrong state arrived; ignored");
                    errors::report(app, AppError::new("sign-in-wrong-link"));
                    return;
                }
                LinkMatch::Refused => {
                    *slot = None;
                    log::info("sign-in: the site did not grant access");
                    status(app, "refused");
                    errors::report(app, AppError::new("sign-in-refused"));
                    return;
                }
                LinkMatch::Ours(code) => (slot.take(), code),
            },
        }
    };
    let (Some(pending), code) = taken else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let s = app.state::<SignIn>();
        let row = match auth::exchange(&s.net, pending, code, now()).await {
            Ok(row) => row,
            Err(e) => {
                log::warn(&format!("sign-in: the token trade failed: {e}"));
                status(&app, "failed");
                let code = if e == CoreError::SignIn { "sign-in-incomplete" } else { "sign-in-trade-failed" };
                errors::report(&app, AppError::new(code).kind(e));
                return;
            }
        };
        let app2 = app.clone();
        let sealed = tauri::async_runtime::spawn_blocking(move || {
            let files = &app2.state::<SignIn>().files;
            let (site, store) = files.create()?;
            store.put_site(&row)?;
            Ok::<_, deetshealth_core::CoreError>(site)
        })
        .await;
        match sealed {
            Ok(Ok(site)) => {
                log::info("sign-in: connected; the site is sealed");
                status(&app, "connected");
                let _ = app.emit("sites-changed", ());
                crate::fetch::start(&app, site);
            }
            Ok(Err(e)) => {
                log::error(&format!("sign-in: could not seal the new site: {e}"));
                status(&app, "failed");
                errors::report(&app, AppError::new("site-seal-failed").kind(e));
            }
            Err(_) => {
                log::error("sign-in: could not seal the new site: the task stopped");
                status(&app, "failed");
                errors::report(&app, AppError::new("site-seal-failed"));
            }
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use deetshealth_core::auth::{b64url, SiteEntry};
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
        };

        let (pending, url) = auth::begin(net, &Cng, &entry, "deetshealth-dev://callback", now()).await.unwrap();
        let catch = Catch(Default::default());
        net::open_in_browser(&catch, &url).unwrap();
        let authorize = catch.0.borrow().clone();
        assert!(authorize.contains("code_challenge_method=S256"));

        let no_redirect = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
        let r = no_redirect.get(&authorize).send().await.unwrap();
        let link = r.headers()["location"].to_str().unwrap().to_owned();
        assert!(link.starts_with("deetshealth-dev://callback"));

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

