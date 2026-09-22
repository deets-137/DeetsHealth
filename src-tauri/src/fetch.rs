// The fetch, the host half (CORE-CRATE.md §7). The core does the work (core/src/sync.rs);
// this file runs it off the UI thread, one fetch per site at a time, and reports counts.
//
// Events to the front end carry counts and the app's own random site key only, never a value
// (SECURITY.md §4.3):
//   fetch-progress { site, counts: { <FHIR type>: n }, jobsDone, jobsTotal, calls }
//   fetch-done     { site, end: "done" | "sign-in-again" | "failed" }

use crate::log;
use crate::signin::SignIn;
use deetshealth_core::platform::{SiteKey, Timer};
use deetshealth_core::sync::{self, FetchEnd, Progress};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Sites with a fetch running now.
#[derive(Default)]
pub struct Running(Mutex<HashSet<String>>);

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

fn progress_event(site: &str, p: &Progress) -> serde_json::Value {
    json!({ "site": site, "counts": p.counts, "jobsDone": p.jobs_done, "jobsTotal": p.jobs_total, "calls": p.calls })
}

/// Start a full fetch for one site, unless one is running. Returns at once.
pub fn start(app: &AppHandle, site: SiteKey) {
    let key = site.as_str().to_owned();
    {
        let running = app.state::<Running>();
        let Ok(mut r) = running.0.lock() else { return };
        if !r.insert(key.clone()) {
            log::info("fetch: already running for this site; not started again");
            return;
        }
    }
    let app = app.clone();
    // SQLite and the sealing are blocking work, so the whole fetch runs on a blocking thread,
    // driving the async HTTP from there (CLAUDE.md › Conventions).
    tauri::async_runtime::spawn_blocking(move || {
        let end = tauri::async_runtime::block_on(run(&app, &site, &key));
        if let Ok(mut r) = app.state::<Running>().0.lock() {
            r.remove(&key);
        }
        let _ = app.emit("fetch-done", json!({ "site": key, "end": end }));
    });
}

async fn run(app: &AppHandle, site: &SiteKey, key: &str) -> &'static str {
    let Some(s) = app.try_state::<SignIn>() else { return "failed" };
    let store = match s.files().open(site) {
        Ok(Some(store)) => store,
        _ => {
            log::error("fetch: the site's store would not open");
            return "failed";
        }
    };
    log::info("fetch: started");
    let result = sync::full_fetch(s.net(), &store, &HostTimer, |p| {
        let _ = app.emit("fetch-progress", progress_event(key, p));
    })
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
            if !sum.skipped.is_empty() {
                log::info(&format!("fetch: the site refused: {}", sum.skipped.join(", ")));
            }
            "done"
        }
        Ok(FetchEnd::SignInAgain) => {
            log::info("fetch: the site refused the refresh token; sign in again");
            "sign-in-again"
        }
        Err(e) => {
            log::warn(&format!("fetch: failed: {e}"));
            "failed"
        }
    }
}

/// The Sites row's Refresh half (stage 7). `site` is the app's own random site key.
#[tauri::command]
pub fn site_refresh(app: AppHandle, site: String) -> Result<(), String> {
    let site = SiteKey::from_stored(site).map_err(|e| e.to_string())?;
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
            let end = sync::full_fetch_paged(&net, &store, &HostTimer, "20", |_| pages += 1).await.unwrap();
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
            let FetchEnd::Done(second) = sync::full_fetch(&net, &store, &HostTimer, |_| {}).await.unwrap() else { panic!() };
            let after: Vec<u64> = Table::ALL.iter().map(|t| store.count(*t).unwrap()).collect();
            assert_eq!(before, after);
            eprintln!("second fetch at 100 a page: {} calls, {} s, key reads {} ms", second.progress.calls, second.seconds, second.key_read_ms);
            drop(store);
            let _ = std::fs::remove_dir_all(&dir);
        });
    }
}
