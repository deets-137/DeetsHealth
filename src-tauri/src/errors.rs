// Every failure the user should hear about, as one event (the owner, 2026-09-21: every
// failure and error route has an error the front end can toast). docs/architecture/ERRORS.md
// lists every code and its toast.
//
//   app-error { code, kind?, site?, count? }
//
// - `code`  a fixed word for WHAT failed ("fetch-site-unreachable"). The front end turns it
//           into a toast (src/app-errors.ts).
// - `kind`  the core's error kind (`CoreError::kind`), for diag only; never shown.
// - `site`  the app's own random site key, when a site is involved.
// - `count` a number, when the code needs one ("3 types").
//
// Nothing in the event is protected data: no value, no name, no id from the record, no URL,
// no site name (SECURITY.md §4.3). The codes are ours.
//
// An error before the page listens (at setup) would be lost, so errors wait in a queue until
// the page calls `app_errors_ready`, which returns the queue and switches to live events.

use deetshealth_core::CoreError;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct AppErrors(Mutex<Queue>);

#[derive(Default)]
struct Queue {
    ready: bool,
    waiting: Vec<Value>,
}

/// One error to report. Build it with [`AppError::new`] and the `with_*` calls.
pub struct AppError {
    code: &'static str,
    kind: Option<&'static str>,
    site: Option<String>,
    count: Option<usize>,
}

impl AppError {
    pub fn new(code: &'static str) -> Self {
        Self { code, kind: None, site: None, count: None }
    }
    pub fn kind(mut self, e: CoreError) -> Self {
        self.kind = Some(kind_word(e));
        self
    }
    pub fn site(mut self, site: &str) -> Self {
        self.site = Some(site.to_owned());
        self
    }
    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }
}

/// A fixed word per core error kind.
pub fn kind_word(e: CoreError) -> &'static str {
    match e {
        CoreError::Crypto => "crypto",
        CoreError::KeyStore => "key-store",
        CoreError::Io => "storage",
        CoreError::Network => "network",
        CoreError::Parse => "unreadable-reply",
        CoreError::SignIn => "sign-in",
        CoreError::Browser => "browser",
        CoreError::Version => "newer-version",
        CoreError::Tree(_) => "record-view",
    }
}

/// Send one error to the page, or queue it until the page listens.
pub fn report(app: &AppHandle, e: AppError) {
    let v = json!({ "code": e.code, "kind": e.kind, "site": e.site, "count": e.count });
    let Some(state) = app.try_state::<AppErrors>() else { return };
    let Ok(mut q) = state.0.lock() else { return };
    if q.ready {
        drop(q);
        let _ = app.emit("app-error", v);
    } else {
        q.waiting.push(v);
    }
}

/// The page listens now: it gets every error that came before, and live events after.
#[tauri::command]
pub fn app_errors_ready(app: AppHandle) -> Vec<Value> {
    let Some(state) = app.try_state::<AppErrors>() else { return Vec::new() };
    let Ok(mut q) = state.0.lock() else { return Vec::new() };
    q.ready = true;
    std::mem::take(&mut q.waiting)
}
