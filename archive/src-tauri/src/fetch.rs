// The fetch, the host half (CORE-CRATE.md §7). The core does the work (core/src/sync.rs);
// this file runs it off the UI thread, one fetch per site at a time, and reports counts.
//
// Events to the front end carry counts and the app's own random site key only, never a value
// (SECURITY.md §4.3):
//   fetch-progress { site, counts: { results, medicines, allergies, conditions, visits },
//                    jobsDone, jobsTotal, calls }                        (payload::FetchProgress)
//   fetch-done     { site, end: "done" | "sign-in-again" | "failed" }   (payload::FetchDone)
//   app-error      every failure, with a fixed code (errors.rs, ERRORS.md)
//
// Before any work on a site file, the file is brought up to date: opening it migrates an
// older schema, and `reindex_if_stale` rebuilds an index made by older rules (the owner,
// 2026-09-21). At app start, `check_sites` does this for every site.

use crate::errors::{self, AppError};
use crate::log;
use crate::signin::SignIn;
use deetshealth_core::payload::{FetchDone, FetchEndWord, FetchProgress, SiteCounts};
use deetshealth_core::platform::{SiteKey, Timer};
use deetshealth_core::store::SiteStore;
use deetshealth_core::sync::{self, FetchEnd, Progress};
use deetshealth_core::CoreError;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Sites with work running now (a fetch, or the start-up check): one piece of work per site.
#[derive(Default)]
pub struct Running(Mutex<HashSet<String>>);

impl Running {
    /// Claim a site for one piece of work. `false`: something else holds it.
    pub(crate) fn claim(app: &AppHandle, key: &str) -> bool {
        app.state::<Running>().0.lock().is_ok_and(|mut r| r.insert(key.to_owned()))
    }
    pub(crate) fn release(app: &AppHandle, key: &str) {
        if let Ok(mut r) = app.state::<Running>().0.lock() {
            r.remove(key);
        }
    }
}

/// Open a site's store and bring it up to date. Every failure is reported; `None` then.
fn open_current(app: &AppHandle, site: &SiteKey, key: &str) -> Option<SiteStore<crate::cng::Cng>> {
    let Some(s) = app.try_state::<SignIn>() else {
        errors::report(app, AppError::new("fetch-unavailable"));
        return None;
    };
    let store = match s.files().open(site) {
        Ok(Some(store)) => store,
        Ok(None) => {
            log::error("site: the key is gone; the store cannot open");
            errors::report(app, AppError::new("site-key-missing").site(key));
            return None;
        }
        Err(e) => {
            log::error(&format!("site: the store would not open: {e}"));
            let code = if e == CoreError::Version { "site-newer" } else { "site-unreadable" };
            errors::report(app, AppError::new(code).kind(e).site(key));
            return None;
        }
    };
    match sync::reindex_if_stale(&store) {
        Ok(None) => {}
        Ok(Some(r)) => log::info(&format!("reindex: {} rows, {} links ({} waiting)", r.rows, r.links, r.waiting)),
        // The rows keep their old index; the next open tries again. The store still works.
        Err(e) => {
            log::warn(&format!("reindex: failed: {e}"));
            errors::report(app, AppError::new("reindex-failed").kind(e).site(key));
        }
    }
    Some(store)
}

/// At app start: open every site once, off the UI thread, so an old file is migrated and a
/// stale index rebuilt before anything reads it.
pub fn check_sites(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(s) = app.try_state::<SignIn>() else { return };
        let sites = s.files().list();
        for site in &sites {
            let key = site.as_str().to_owned();
            if Running::claim(&app, &key) {
                drop(open_current(&app, site, &key));
                Running::release(&app, &key);
            }
        }
        log::info(&format!("sites: {} checked at start", sites.len()));
    });
}

/// The platform's time for the core: the wall clock and tokio's timer.
pub struct HostTimer;

impl Timer for HostTimer {
    fn now(&self) -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }
    fn sleep(&self, seconds: u64) -> impl std::future::Future<Output = ()> {
        tokio::time::sleep(std::time::Duration::from_secs(seconds))
    }
}

fn progress_event(site: &str, p: &Progress) -> FetchProgress {
    FetchProgress {
        site: site.to_owned(),
        counts: SiteCounts::from_types(&p.counts),
        jobs_done: p.jobs_done as u32,
        jobs_total: p.jobs_total as u32,
        calls: p.calls,
    }
}

/// Start a full fetch for one site, unless one is running. Returns at once.
pub fn start(app: &AppHandle, site: SiteKey) {
    let key = site.as_str().to_owned();
    if !Running::claim(app, &key) {
        log::info("fetch: already running for this site; not started again");
        errors::report(app, AppError::new("fetch-busy").site(&key));
        return;
    }
    let app = app.clone();
    // SQLite and the sealing are blocking work, so the whole fetch runs on a blocking thread,
    // driving the async HTTP from there (CLAUDE.md › Conventions).
    tauri::async_runtime::spawn_blocking(move || {
        let end = tauri::async_runtime::block_on(run(&app, &site, &key));
        Running::release(&app, &key);
        let _ = app.emit("fetch-done", FetchDone { site: key, end });
    });
}

async fn run(app: &AppHandle, site: &SiteKey, key: &str) -> FetchEndWord {
    let Some(store) = open_current(app, site, key) else { return FetchEndWord::Failed };
    let s = app.state::<SignIn>();
    log::info("fetch: started");
    let result = sync::full_fetch(
        s.net(),
        &store,
        &HostTimer,
        |p| {
            let _ = app.emit("fetch-progress", progress_event(key, p));
        },
        |c| log::info(&call_line(c)),
    )
    .await;
    match result {
        Ok(FetchEnd::Done(sum)) => {
            let n: u64 = sum.progress.counts.values().sum();
            log::info(&format!(
                "fetch: done: {n} resources, {} types, {} links ({} waiting), {} calls, {} s, {} warnings, key reads {} ms",
                sum.progress.counts.len(),
                sum.links,
                sum.links_waiting,
                sum.progress.calls,
                sum.seconds,
                sum.warnings,
                sum.key_read_ms
            ));
            log::info(&format!("fetch: types: {}", tally(sum.progress.counts.iter().map(|(k, v)| (k, *v)))));
            if !sum.waiting_by_type.is_empty() {
                log::info(&format!("fetch: waiting links point at: {}", tally(sum.waiting_by_type.iter().map(|(k, v)| (k, *v as u64)))));
            }
            if !sum.warnings_by_code.is_empty() {
                log::info(&format!("fetch: warnings: {}", tally(sum.warnings_by_code.iter().map(|(k, v)| (k, *v as u64)))));
            }
            if !sum.skipped.is_empty() {
                log::info(&format!("fetch: the site refused: {}", sum.skipped.join(", ")));
            }
            if !sum.stopped.is_empty() {
                log::info(&format!("fetch: incomplete: {}", sum.stopped.join(", ")));
                errors::report(app, AppError::new("fetch-partial").site(key).count(sum.stopped.len()));
            }
            FetchEndWord::Done
        }
        Ok(FetchEnd::SignInAgain) => {
            log::info("fetch: the site refused the refresh token; sign in again");
            forget_tokens(&store);
            errors::report(app, AppError::new("fetch-sign-in-again").site(key));
            FetchEndWord::SignInAgain
        }
        Err(e) => {
            log::warn(&format!("fetch: failed: {e}"));
            let code = match e {
                CoreError::Network => "fetch-site-unreachable",
                CoreError::Parse => "fetch-site-unreadable",
                CoreError::Crypto | CoreError::Io | CoreError::KeyStore => "fetch-store-failed",
                CoreError::Version => "site-newer",
                _ => "fetch-failed",
            };
            errors::report(app, AppError::new(code).kind(e).site(key));
            FetchEndWord::Failed
        }
    }
}

/// One call-ledger line (FETCH.md §9): our words and numbers only (`sync::CallNote`).
/// `fetch: call Observation?category=laboratory p1 · 200 · 34 + 5 included · 12.3 KB · 412 ms · next-page`
fn call_line(c: &sync::CallNote) -> String {
    let status = c.status.map_or("no reply".to_owned(), |s| s.to_string());
    let mut line = format!("fetch: call {} p{} · {status}", c.what, c.page);
    if c.entries > 0 || c.included > 0 {
        line.push_str(&format!(" · {}", c.entries));
        if c.included > 0 {
            line.push_str(&format!(" + {} included", c.included));
        }
    }
    line.push_str(&format!(" · {:.1} KB · {} ms", c.bytes as f64 / 1024.0, c.ms));
    if c.tries > 1 {
        line.push_str(&format!(" · {} tries", c.tries));
    }
    if !c.issues.is_empty() {
        line.push_str(&format!(" · issues {}", c.issues.join(" ")));
    }
    line.push_str(&format!(" · {}", c.outcome));
    line
}

/// `A 3, B 12`: kinds and counts, in key order.
fn tally<'a>(items: impl Iterator<Item = (&'a String, u64)>) -> String {
    items.map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
}

/// The site refused the refresh token, so both tokens are dead. Drop them, so the Sites row
/// says "Sign in again" also after a restart (`payload::site_view`). A failure here only
/// costs that: the next Refresh finds out again.
fn forget_tokens(store: &SiteStore<crate::cng::Cng>) {
    let Ok(Some(mut row)) = store.site() else { return };
    row.access_token = None;
    row.refresh_token = None;
    row.token_expiry = None;
    if let Err(e) = store.put_site(&row) {
        log::warn(&format!("fetch: could not drop the dead tokens: {e}"));
    }
}

/// The Sites row's Refresh half. `site` is the app's own random site key. On
/// failure it returns the `app-error` code.
#[tauri::command]
pub fn site_refresh(app: AppHandle, site: String) -> Result<(), String> {
    // A key in the right form with no site behind it fails later, at the open
    // (`site-key-missing`); no file I/O here, on the UI thread.
    let Ok(site) = SiteKey::from_stored(site) else {
        errors::report(&app, AppError::new("site-unknown"));
        return Err("site-unknown".into());
    };
    start(&app, site);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cng::Cng;
    use crate::sites::SiteFiles;
    use deetshealth_core::net::Net;
    use deetshealth_core::store::{Store, Table};

    /// A full fetch against the live SMART sandbox, into a temporary sealed store: sign in
    /// without a browser (signin.rs), then `full_fetch`. Prints counts only. Run with
    /// `cargo test -p deetshealth --lib fetch -- --ignored --nocapture`: about 5 calls to sign
    /// in, then one call per type and page (the log of the run says how many).
    #[test]
    #[ignore]
    fn full_fetch_against_the_smart_sandbox() {
        tauri::async_runtime::block_on(async {
            let net = Net::new().unwrap();
            let row = crate::signin::tests::headless_sign_in(&net).await;
            let dir = std::env::temp_dir().join(format!("deetshealth-fetch-{}", SiteKey::generate(&Cng).unwrap().as_str()));
            let files = SiteFiles::new(&dir);
            let (_site, store) = files.create().unwrap();
            store.put_site(&row).unwrap();

            // Pages of 20, so the `next` links are followed even on a small sandbox record.
            let mut pages = 0;
            let end = sync::full_fetch_paged(&net, &store, &HostTimer, "20", |_| pages += 1, |_| {}).await.unwrap();
            let FetchEnd::Done(sum) = end else { panic!("sign in again") };
            let first_pages = sum.progress.calls as usize - 2; // less /metadata and the patient
            assert!(first_pages > sum.progress.jobs_total, "some job had a second page");
            assert_eq!(sum.warnings, 0, "every next link stayed on the site");
            let total: u64 = sum.progress.counts.values().sum();
            eprintln!(
                "fetch: {total} resources in {} types; {} links, {} waiting; {} jobs; {} calls; {} s; {} warnings; key reads {} ms; refused: {:?}",
                sum.progress.counts.len(),
                sum.links,
                sum.links_waiting,
                sum.progress.jobs_total,
                sum.progress.calls,
                sum.seconds,
                sum.warnings,
                sum.key_read_ms,
                sum.skipped
            );
            for (t, n) in &sum.progress.counts {
                eprintln!("  {t}: {n}");
            }
            assert!(total > 0, "the sandbox patient has a record");
            assert!(sum.links > 0, "the record is woven: references became links");
            assert!(store.patient().unwrap().is_some(), "the patient row is kept");
            let site = Store::site(&store).unwrap().unwrap();
            assert!(site.capability.is_some() && site.last_full_fetch.is_some());

            // A second fetch, at the app's page size, replaces rows; it does not add them.
            let before: Vec<u64> = Table::ALL.iter().map(|t| store.count(*t).unwrap()).collect();
            let FetchEnd::Done(second) = sync::full_fetch(&net, &store, &HostTimer, |_| {}, |_| {}).await.unwrap() else { panic!() };
            let after: Vec<u64> = Table::ALL.iter().map(|t| store.count(*t).unwrap()).collect();
            assert_eq!(before, after);
            eprintln!("second fetch at 100 a page: {} calls, {} s, key reads {} ms", second.progress.calls, second.seconds, second.key_read_ms);
            drop(store);
            let _ = std::fs::remove_dir_all(&dir);
        });
    }
}
