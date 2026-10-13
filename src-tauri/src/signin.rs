// Sign-in, the host half (CORE-CRATE.md §6). The core does the SMART work (core/src/auth.rs);
// this file holds the one waiting sign-in, opens the browser, takes the link back, and seals
// the new site.
//
// The link arrives as plain text in the process arguments. It holds the code, so it is
// shielded on arrival and never logged: a log line says only what happened.
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
use crate::sites::SiteFiles;
use deetshealth_core::auth::{self, LinkMatch, PendingSignIn, Renewal, SiteEntry};
use deetshealth_core::net::{self, Net};
use deetshealth_core::payload::SignInStatus;
use deetshealth_core::platform::SiteKey;
use deetshealth_core::store::SiteRow;
use deetshealth_core::{CoreError, Shielded};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub struct SignIn {
    net: Net,
    files: SiteFiles,
    redirect_uri: String,
    pending: Mutex<Option<Waiting>>,
}

/// The one sign-in waiting for the browser.
struct Waiting {
    pending: PendingSignIn,
    /// The saved site a row's **Sign in** renews; `None` for Connect.
    renew: Option<SiteKey>,
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
    let (pending, url) = match auth::begin(&s.net, &Cng, entry, &s.redirect_uri, now()).await {
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
    *s.pending.lock().map_err(|_| fail("sign-in-failed"))? = Some(Waiting { pending, renew });
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
            Some(w) if w.pending.expired(now()) => {
                *slot = None;
                log::warn("sign-in: a link arrived after the wait ran out; ignored");
                status(app, SignInStatus::Expired);
                errors::report(app, AppError::new("sign-in-expired"));
                return;
            }
            Some(w) => match auth::check_link(&w.pending, &link) {
                LinkMatch::NotOurs => {
                    log::warn("sign-in: a link with the wrong state arrived; ignored");
                    errors::report(app, AppError::new("sign-in-wrong-link"));
                    return;
                }
                LinkMatch::Refused => {
                    *slot = None;
                    log::info("sign-in: the site did not grant access");
                    status(app, SignInStatus::Refused);
                    errors::report(app, AppError::new("sign-in-refused"));
                    return;
                }
                LinkMatch::Ours(code) => (slot.take(), code),
            },
        }
    };
    let (Some(Waiting { pending, renew }), code) = taken else { return };
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

