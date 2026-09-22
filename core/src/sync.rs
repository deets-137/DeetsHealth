//! The fetch (CORE-CRATE.md §7): everything the site serves for this patient, into the
//! sealed store.
//!
//! 1. Refresh the access token if it runs out soon (`auth::needs_refresh`).
//! 2. Read `/metadata` (no token): which types the site serves and how to search each one.
//! 3. Read the patient: name, birth date and sex only (`_elements` asks for no more).
//! 4. One job per type the site serves and the scopes allow; `Observation` once per category
//!    when the site supports `category`. Each job: `?<patient param>=<id>&_count=100`, plus
//!    the `_include`s the site lists from [`WANTED_INCLUDES`], then every `next` page.
//! 5. Three jobs run at once ([`PACE`]). Pages inside one job run in order.
//! 6. Each page is written in one transaction: a resource replaces its earlier row (same FHIR
//!    type and id) or gets a new one. Warnings go to `site_warning`.
//!
//! Stage 5 stores each resource's type, id, a first `kind` and the raw resource. The index
//! columns and links come in stage 6, from the raw resource, with no new fetch.

use crate::auth::{self, Refresh};
use crate::fhir::{self, Issue, Searchable, OBSERVATION_CATEGORIES};
use crate::net::{self, Field, Net, Reply, Text};
use crate::platform::Timer;
use crate::store::{ResourceRow, RowId, SiteRow, Store, Table, WarningRow};
use crate::{CoreError, Shielded};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet, VecDeque};

/// Jobs at once, per site (fork 5a): a stewardship choice. Pages inside one type run in
/// order, so the longest page chain sets the floor and more at once gains little.
pub const PACE: usize = 3;
/// Resources per page.
pub const PAGE_SIZE: &str = "100";
/// Tries per request on a network error, 429, 500, 502, 503 or 504.
const TRIES: u32 = 4;
/// The longest wait we honour from `Retry-After`.
const MAX_WAIT_S: u64 = 60;

/// References the cards show, fetched in the same call when the site lists them in its
/// `searchInclude` (LESSONS.md §3.3: never one read per row).
pub const WANTED_INCLUDES: [&str; 8] = [
    "MedicationRequest:medication",
    "MedicationRequest:requester",
    "MedicationDispense:medication",
    "MedicationAdministration:medication",
    "MedicationStatement:medication",
    "Encounter:participant",
    "Encounter:service-provider",
    "Encounter:location",
];

/// Counts only: safe for a log line and for the Sites row (SECURITY.md §4.3).
#[derive(Clone, Default)]
pub struct Progress {
    /// Resources stored so far, by FHIR type.
    pub counts: BTreeMap<String, u64>,
    pub jobs_done: usize,
    pub jobs_total: usize,
    pub calls: u32,
}

/// How a full fetch ended.
pub enum FetchEnd {
    Done(Summary),
    /// The token could not be renewed: the Sites row says "Sign in again".
    SignInAgain,
}

/// Counts only.
pub struct Summary {
    pub progress: Progress,
    /// Issues stored in `site_warning`, including skipped jobs.
    pub warnings: u32,
    /// Jobs the site refused (a 4xx other than 401), by FHIR type.
    pub skipped: Vec<String>,
    pub seconds: u64,
    /// Time spent reading row keys to match resources to rows (CORE-CRATE.md §12, stage 3).
    pub key_read_ms: u128,
}

struct Job {
    fhir_type: String,
    table: Table,
    kind: String,
    url: Text,
}

/// What the three workers share. They run on one task, so a RefCell is enough; no borrow is
/// held across an `.await`.
struct Shared {
    jobs: VecDeque<Job>,
    progress: Progress,
    token: Text,
    token_generation: u32,
    sign_in_again: bool,
    warnings: u32,
    skipped: Vec<String>,
    /// (FHIR type, FHIR id) → row, per table once read.
    keys: BTreeMap<(String, Text), RowId>,
    keys_read: HashSet<Table>,
    key_read_ms: u128,
}

/// A full fetch for the site in `store`. `on_progress` gets counts after every page.
pub async fn full_fetch<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    on_progress: impl FnMut(&Progress),
) -> Result<FetchEnd, CoreError> {
    full_fetch_paged(net, store, timer, PAGE_SIZE, on_progress).await
}

/// [`full_fetch`] with another page size. The app always uses [`PAGE_SIZE`]; a test uses a
/// small one so the `next` links get followed on a small sandbox record.
pub async fn full_fetch_paged<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    page_size: &str,
    mut on_progress: impl FnMut(&Progress),
) -> Result<FetchEnd, CoreError> {
    let started = timer.now();
    let mut site = store.site()?.ok_or(CoreError::Io)?;
    if auth::needs_refresh(&site, started) {
        match auth::refresh(net, &mut site, started).await? {
            Refresh::SignInAgain => return Ok(FetchEnd::SignInAgain),
            Refresh::Renewed => store.put_site(&site)?,
        }
    }
    let base = site.base_url.clone().ok_or(CoreError::Io)?;
    let patient = site.patient_fhir_id.clone().ok_or(CoreError::Io)?;
    let token = site.access_token.clone().ok_or(CoreError::Io)?;
    let mut calls = 0u32;

    // What the site serves.
    let meta = get(net, timer, &net::join(&base, "metadata"), None, &mut calls).await?;
    if !meta.ok() {
        return Err(CoreError::Network);
    }
    let cap = fhir::read_capability(&meta.body)?;
    let includes = read_includes(&meta.body);
    site.capability = Some(meta.body);
    site.software_version = cap.software_version;

    // The patient, trimmed at the parse.
    let url = net::url_with(
        &net::join(&base, "Patient"),
        &[("_id", Field::Shielded(&patient)), ("_elements", Field::Plain("name,birthDate,gender"))],
    )?;
    let reply = get(net, timer, &url, Some(&token), &mut calls).await?;
    if reply.ok() {
        let page = fhir::read_page(&reply.body)?;
        if let Some(p) = page.found.iter().find(|f| f.fhir_type == "Patient") {
            store.put_patient(&fhir::read_patient(&p.raw)?)?;
        }
    }

    // The jobs.
    let scopes = site.scopes.as_ref().map(net::read_scopes);
    let mut jobs = VecDeque::new();
    for s in cap.searchable.iter().filter(|s| fhir::scope_allows(scopes.as_deref(), &s.fhir_type)) {
        if s.fhir_type == "Observation" && s.has_category {
            for c in OBSERVATION_CATEGORIES {
                jobs.push_back(job(&base, &patient, s, c, &[("category", c)], &includes, page_size)?);
            }
        } else {
            jobs.push_back(job(&base, &patient, s, &s.fhir_type.to_lowercase(), &[], &includes, page_size)?);
        }
    }

    store.clear_warnings()?;
    let jobs_total = jobs.len();
    let shared = RefCell::new(Shared {
        jobs,
        progress: Progress { jobs_total, calls, ..Default::default() },
        token,
        token_generation: 0,
        sign_in_again: false,
        warnings: 0,
        skipped: Vec::new(),
        keys: BTreeMap::new(),
        keys_read: HashSet::new(),
        key_read_ms: 0,
    });
    let site_cell = RefCell::new(site);
    let progress_cb = RefCell::new(&mut on_progress);
    let workers = (0..PACE).map(|_| worker(net, store, timer, &base, &shared, &site_cell, &progress_cb));
    futures_util::future::join_all(workers).await;

    let shared = shared.into_inner();
    let mut site = site_cell.into_inner();
    if shared.sign_in_again {
        return Ok(FetchEnd::SignInAgain);
    }
    let now = Shielded::new(timer.now().to_string());
    site.last_fetch = Some(now.clone());
    site.last_full_fetch = Some(now);
    store.put_site(&site)?;
    Ok(FetchEnd::Done(Summary {
        progress: shared.progress,
        warnings: shared.warnings,
        skipped: shared.skipped,
        seconds: timer.now().saturating_sub(started),
        key_read_ms: shared.key_read_ms,
    }))
}

#[allow(clippy::too_many_arguments)]
fn job(
    base: &Text,
    patient: &Text,
    s: &Searchable,
    kind: &str,
    extra: &[(&str, &str)],
    includes: &[String],
    page_size: &str,
) -> Result<Job, CoreError> {
    let mut q = vec![(s.patient_param, Field::Shielded(patient)), ("_count", Field::Plain(page_size))];
    q.extend(extra.iter().map(|(k, v)| (*k, Field::Plain(v))));
    let prefix = format!("{}:", s.fhir_type);
    q.extend(includes.iter().filter(|i| i.starts_with(&prefix)).map(|i| ("_include", Field::Plain(i.as_str()))));
    Ok(Job { fhir_type: s.fhir_type.clone(), table: s.table, kind: kind.to_owned(), url: net::url_with(&net::join(base, &s.fhir_type), &q)? })
}

/// The wanted includes the site lists in its `searchInclude`.
fn read_includes(capability: &Text) -> Vec<String> {
    net::parse_reply(capability, |v| {
        let mut out = Vec::new();
        for r in v.pointer("/rest/0/resource").and_then(|x| x.as_array()).into_iter().flatten() {
            for i in r.get("searchInclude").and_then(|x| x.as_array()).into_iter().flatten().filter_map(|x| x.as_str()) {
                if WANTED_INCLUDES.contains(&i) {
                    out.push(i.to_owned());
                }
            }
        }
        Ok(out)
    })
    .unwrap_or_default()
}

/// One of the [`PACE`] workers: takes jobs until none are left.
async fn worker<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    base: &Text,
    shared: &RefCell<Shared>,
    site: &RefCell<SiteRow>,
    on_progress: &RefCell<&mut impl FnMut(&Progress)>,
) {
    loop {
        let job = {
            let mut s = shared.borrow_mut();
            if s.sign_in_again {
                return;
            }
            match s.jobs.pop_front() {
                Some(j) => j,
                None => return,
            }
        };
        run_job(net, store, timer, base, shared, site, &job, on_progress).await;
        let snapshot = {
            let mut s = shared.borrow_mut();
            s.progress.jobs_done += 1;
            s.progress.clone()
        };
        (on_progress.borrow_mut())(&snapshot);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_job<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    base: &Text,
    shared: &RefCell<Shared>,
    site: &RefCell<SiteRow>,
    job: &Job,
    on_progress: &RefCell<&mut impl FnMut(&Progress)>,
) {
    let mut next = Some(job.url.clone());
    let mut refreshed = false;
    while let Some(url) = next.take() {
        let (token, generation) = {
            let s = shared.borrow();
            (s.token.clone(), s.token_generation)
        };
        let mut calls = 0;
        let reply = get(net, timer, &url, Some(&token), &mut calls).await;
        shared.borrow_mut().progress.calls += calls;
        let reply = match reply {
            Ok(r) => r,
            Err(_) => {
                warn(store, shared, &job.fhir_type, "network", timer.now());
                return;
            }
        };
        match reply.status {
            200..=299 => {}
            401 if !refreshed => {
                refreshed = true;
                if !renew(net, store, timer, shared, site, generation).await {
                    return;
                }
                next = Some(url);
                continue;
            }
            status => {
                let issues = fhir::read_error(&reply.body);
                let now = timer.now();
                {
                    let mut s = shared.borrow_mut();
                    s.skipped.push(job.fhir_type.clone());
                }
                warn(store, shared, &job.fhir_type, &format!("http-{status}"), now);
                store_issues(store, shared, &job.fhir_type, issues, now);
                return;
            }
        }
        let page = match fhir::read_page(&reply.body) {
            Ok(p) => p,
            Err(_) => {
                warn(store, shared, &job.fhir_type, "unreadable-page", timer.now());
                return;
            }
        };
        if let Err(e) = write_page(store, shared, job, &page.found) {
            warn(store, shared, &job.fhir_type, if e == CoreError::Crypto { "seal" } else { "store" }, timer.now());
            return;
        }
        store_issues(store, shared, &job.fhir_type, page.issues, timer.now());
        let snapshot = shared.borrow().progress.clone();
        (on_progress.borrow_mut())(&snapshot);
        // A `next` link must stay on this site: the token never goes to another host.
        next = match page.next {
            Some(n) if net::starts_with(&n, base) => Some(n),
            Some(_) => {
                warn(store, shared, &job.fhir_type, "next-link-off-site", timer.now());
                None
            }
            None => None,
        };
    }
}

/// Renew the token after a 401, once per token generation (another worker may have done it).
async fn renew<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    shared: &RefCell<Shared>,
    site: &RefCell<SiteRow>,
    seen_generation: u32,
) -> bool {
    if shared.borrow().token_generation != seen_generation {
        return true;
    }
    let mut row = std::mem::take(&mut *site.borrow_mut());
    let result = auth::refresh(net, &mut row, timer.now()).await;
    let ok = match result {
        Ok(Refresh::Renewed) => {
            let mut s = shared.borrow_mut();
            if let Some(t) = row.access_token.clone() {
                s.token = t;
            }
            s.token_generation += 1;
            store.put_site(&row).is_ok()
        }
        Ok(Refresh::SignInAgain) => {
            shared.borrow_mut().sign_in_again = true;
            false
        }
        Err(_) => false,
    };
    *site.borrow_mut() = row;
    ok
}

/// Write one page's resources in one transaction.
fn write_page<S: Store>(store: &S, shared: &RefCell<Shared>, job: &Job, found: &[fhir::Found]) -> Result<(), CoreError> {
    store.in_transaction(|st| {
        for f in found {
            let (table, kind) = if f.included || f.fhir_type != job.fhir_type {
                match fhir::table_for(&f.fhir_type) {
                    Some(t) => (t, f.fhir_type.to_lowercase()),
                    None => continue,
                }
            } else {
                (job.table, job.kind.clone())
            };
            let id = row_for(st, shared, table, &f.fhir_type, &f.fhir_id)?;
            st.put_resource(
                table,
                &ResourceRow {
                    id,
                    fhir_type: Shielded::new(f.fhir_type.clone()),
                    fhir_id: f.fhir_id.clone(),
                    kind: Shielded::new(kind),
                    code_system: None,
                    code: None,
                    code_display: None,
                    status: None,
                    start: None,
                    end: None,
                    recorded: None,
                    last_updated: None,
                    canonical: None,
                    version: None,
                    raw: f.raw.clone(),
                },
            )?;
            *shared.borrow_mut().progress.counts.entry(f.fhir_type.clone()).or_default() += 1;
        }
        Ok(())
    })
}

/// The row for a resource: the one it had, or a new one.
fn row_for<S: Store>(store: &S, shared: &RefCell<Shared>, table: Table, fhir_type: &str, fhir_id: &Text) -> Result<RowId, CoreError> {
    let mut s = shared.borrow_mut();
    if !s.keys_read.contains(&table) {
        let t0 = Stopwatch::start();
        for (row, t, id) in store.keys(table)? {
            s.keys.insert((t, id), row);
        }
        s.key_read_ms += t0.ms();
        s.keys_read.insert(table);
    }
    let key = (fhir_type.to_owned(), fhir_id.clone());
    if let Some(row) = s.keys.get(&key) {
        return Ok(row.clone());
    }
    let row = store.new_row_id()?;
    s.keys.insert(key, row.clone());
    Ok(row)
}

/// Milliseconds for the key-read measurement. The web has no `Instant`; it reports 0 there.
struct Stopwatch(#[cfg(not(target_arch = "wasm32"))] std::time::Instant);
impl Stopwatch {
    fn start() -> Self {
        Self(
            #[cfg(not(target_arch = "wasm32"))]
            std::time::Instant::now(),
        )
    }
    fn ms(&self) -> u128 {
        #[cfg(not(target_arch = "wasm32"))]
        return self.0.elapsed().as_millis();
        #[cfg(target_arch = "wasm32")]
        0
    }
}

/// A warning of our own: what went wrong with a job, as a fixed code.
fn warn<S: Store>(store: &S, shared: &RefCell<Shared>, fhir_type: &str, code: &str, now: u64) {
    let issue = Issue { severity: Some(Shielded::new("error".into())), code: Some(Shielded::new(code.into())), text: None };
    store_issues(store, shared, fhir_type, vec![issue], now);
}

fn store_issues<S: Store>(store: &S, shared: &RefCell<Shared>, fhir_type: &str, issues: Vec<Issue>, now: u64) {
    for i in issues {
        let Ok(id) = store.new_row_id() else { continue };
        let w = WarningRow {
            id,
            fhir_type: Shielded::new(fhir_type.to_owned()),
            severity: i.severity,
            code: i.code,
            text: i.text,
            at: Shielded::new(now.to_string()),
        };
        if store.add_warning(&w).is_ok() {
            shared.borrow_mut().warnings += 1;
        }
    }
}

/// GET with retries: a network error, 429, 500, 502, 503 or 504 waits and tries again, up to
/// [`TRIES`] times. The wait is `Retry-After` (at most [`MAX_WAIT_S`]) or 1, 2, 4 s.
async fn get<T: Timer>(net: &Net, timer: &T, url: &Text, token: Option<&Text>, calls: &mut u32) -> Result<Reply, CoreError> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        *calls += 1;
        let result = net.get_json(url, token).await;
        let wait = match &result {
            Ok(r) if matches!(r.status, 429 | 500 | 502 | 503 | 504) => r.retry_after.unwrap_or(1 << (attempt - 1)).min(MAX_WAIT_S),
            Ok(_) => return result,
            Err(_) => 1 << (attempt - 1),
        };
        if attempt >= TRIES {
            return result;
        }
        timer.sleep(wait).await;
    }
}
