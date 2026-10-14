//! The fetch (CORE-CRATE.md §7): everything the site serves for this patient, into the
//! sealed store.
//!
//! 1. Refresh the access token if it runs out soon (`auth::needs_refresh`).
//! 2. Read `/metadata` (no token): which types the site serves and how to search each one.
//! 3. Read the patient: name, birth date and sex only (`_elements` asks for no more).
//! 4. One job per type the site serves and the scopes allow. Each job:
//!    `?<patient param>=<id>&_count=100`, plus the `_include`s the site lists from
//!    [`WANTED_INCLUDES`], then every `next` page. `Observation` and `CarePlan` are searched
//!    plain first; when the site supports `category` and refuses the plain search (Epic does),
//!    they are searched once per category instead (`fhir::category_fallback`; the owner,
//!    2026-09-21 for Observation, 2026-10-13 for CarePlan).
//! 5. Three jobs run at once ([`PACE`]). Pages inside one job run in order.
//! 6. Each page is written in one transaction: a resource replaces its earlier row (same FHIR
//!    type and id) or gets a new one, with its index columns and its links (index.rs).
//!    Warnings go to `site_warning`.
//! 7. Links whose target came later are resolved once every job is done.

use crate::auth::{self, Refresh};
use crate::fhir::{self, Issue, Searchable};
use crate::net::{self, Field, Net, Reply, Text};
use crate::platform::Timer;
use crate::index::{Index, Ref};
use crate::store::{LinkRow, ResourceRow, RowId, SiteRow, Store, Table, WarningRow};
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
    /// Every FHIR type with a job that stopped before its last page, for any reason (the
    /// refused ones included): the records of these types may be incomplete.
    pub stopped: Vec<String>,
    pub seconds: u64,
    /// Time spent reading row keys to match resources to rows (CORE-CRATE.md §12, stage 3).
    pub key_read_ms: u128,
    /// Links written (DATA-MODEL.md §4).
    pub links: u32,
    /// Links whose target the site never sent: a reference to a resource not fetched.
    pub links_waiting: u32,
    /// The waiting links by the FHIR type they point at (`other` for a word that is not a FHIR
    /// type), so a gap in the includes shows (FETCH.md §5.3).
    pub waiting_by_type: BTreeMap<String, u32>,
    /// The stored warnings by `<FHIR type> <word>`: our own codes (`http-400`), or the site's
    /// issues as [`fhir::issue_word`] gives them. Never an issue's text.
    pub warnings_by_code: BTreeMap<String, u32>,
}

/// One HTTP call of a fetch, for the call ledger in the log (FETCH.md §9). Our own words and
/// numbers only: never a URL, an id, a token or a value from a reply (SECURITY.md §4.5).
pub struct CallNote {
    /// `metadata`, `Patient`, or the job: its FHIR type and our own extra parameters
    /// (`Observation?category=laboratory`).
    pub what: String,
    /// The page within the job, from 1.
    pub page: u32,
    /// `None`: no reply after every try.
    pub status: Option<u16>,
    /// Attempts, retries included.
    pub tries: u32,
    pub ms: u128,
    pub bytes: usize,
    /// Resources the search matched, and resources brought by `_include`.
    pub entries: u32,
    pub included: u32,
    /// The site's issues on this page, as [`fhir::issue_word`] gives them.
    pub issues: Vec<String>,
    /// What the job did next: `next-page`, `last-page`, `fallback`, `category-not-served`,
    /// `refused`, `token-renew`,
    /// `network`, `unreadable`, `store-failed`, `next-link-off-site`.
    pub outcome: &'static str,
}

#[derive(Clone)]
struct Job {
    fhir_type: String,
    /// The type and our own extra parameters, for the call ledger. No patient id.
    label: String,
    table: Table,
    url: Text,
    /// Jobs to run instead when the site refuses this one's first page. The plain
    /// `Observation` search carries the 9 category searches here (FETCH.md §5.2).
    fallback: Vec<Job>,
    /// This job is one category of a fallback. A site may not know every category (Epic's
    /// sandbox refuses 8 of the 10 CarePlan ones it names), so a refusal here means "not
    /// served": no warning, and the type is not marked incomplete.
    is_fallback: bool,
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
    stopped: std::collections::BTreeSet<String>,
    keys: Keys,
    /// Links written.
    links: u32,
    warnings_by_code: BTreeMap<String, u32>,
}

/// A full fetch for the site in `store`. `on_progress` gets counts after every page;
/// `on_call` gets one [`CallNote`] per HTTP call.
pub async fn full_fetch<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    on_progress: impl FnMut(&Progress),
    on_call: impl FnMut(&CallNote),
) -> Result<FetchEnd, CoreError> {
    full_fetch_paged(net, store, timer, PAGE_SIZE, on_progress, on_call).await
}

/// [`full_fetch`] with another page size. The app always uses [`PAGE_SIZE`]; a test uses a
/// small one so the `next` links get followed on a small sandbox record.
pub async fn full_fetch_paged<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    page_size: &str,
    mut on_progress: impl FnMut(&Progress),
    mut on_call: impl FnMut(&CallNote),
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
    let t0 = Stopwatch::start();
    let meta = get(net, timer, &net::join(&base, "metadata"), None, &mut calls).await;
    on_call(&simple_note("metadata", calls, t0.ms(), meta.as_ref().ok()));
    let meta = meta?;
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
    let (before, t0) = (calls, Stopwatch::start());
    let reply = get(net, timer, &url, Some(&token), &mut calls).await;
    on_call(&simple_note("Patient", calls - before, t0.ms(), reply.as_ref().ok()));
    let reply = reply?;
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
        let mut j = job(&base, &patient, s, &[], &includes, page_size)?;
        if s.has_category {
            for c in fhir::category_fallback(&s.fhir_type) {
                j.fallback.push(job(&base, &patient, s, &[("category", c)], &includes, page_size)?);
            }
        }
        jobs.push_back(j);
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
        stopped: Default::default(),
        keys: Keys::default(),
        links: 0,
        warnings_by_code: BTreeMap::new(),
    });
    let site_cell = RefCell::new(site);
    let progress_cb = RefCell::new(&mut on_progress);
    let call_cb = RefCell::new(&mut on_call);
    let workers = (0..PACE).map(|_| worker(net, store, timer, &base, &shared, &site_cell, &progress_cb, &call_cb));
    futures_util::future::join_all(workers).await;

    let mut shared = shared.into_inner();
    let mut site = site_cell.into_inner();
    if shared.sign_in_again {
        return Ok(FetchEnd::SignInAgain);
    }
    // Links whose target arrived on a later page, or in another job.
    let (_, links_waiting, waiting_by_type) = store.in_transaction(|st| resolve_links(st, &mut shared.keys))?;
    let now = Shielded::new(timer.now().to_string());
    site.last_fetch = Some(now.clone());
    site.last_full_fetch = Some(now);
    store.put_site(&site)?;
    Ok(FetchEnd::Done(Summary {
        progress: shared.progress,
        warnings: shared.warnings,
        skipped: shared.skipped,
        stopped: shared.stopped.into_iter().collect(),
        seconds: timer.now().saturating_sub(started),
        key_read_ms: shared.keys.ms,
        links: shared.links,
        links_waiting,
        waiting_by_type,
        warnings_by_code: shared.warnings_by_code,
    }))
}

/// The ledger note for a call outside the jobs (`metadata`, `Patient`).
fn simple_note(what: &str, tries: u32, ms: u128, reply: Option<&Reply>) -> CallNote {
    CallNote {
        what: what.to_owned(),
        page: 1,
        status: reply.map(|r| r.status),
        tries,
        ms,
        bytes: reply.map_or(0, |r| net::byte_len(&r.body)),
        entries: 0,
        included: 0,
        issues: Vec::new(),
        outcome: if reply.is_some() { "last-page" } else { "network" },
    }
}

#[allow(clippy::too_many_arguments)]
fn job(
    base: &Text,
    patient: &Text,
    s: &Searchable,
    extra: &[(&str, &str)],
    includes: &[String],
    page_size: &str,
) -> Result<Job, CoreError> {
    let mut q = vec![(s.patient_param, Field::Shielded(patient)), ("_count", Field::Plain(page_size))];
    q.extend(extra.iter().map(|(k, v)| (*k, Field::Plain(v))));
    let prefix = format!("{}:", s.fhir_type);
    q.extend(includes.iter().filter(|i| i.starts_with(&prefix)).map(|i| ("_include", Field::Plain(i.as_str()))));
    let mut label = s.fhir_type.clone();
    if !extra.is_empty() {
        label.push('?');
        label.push_str(&extra.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"));
    }
    Ok(Job {
        label,
        fhir_type: s.fhir_type.clone(),
        table: s.table,
        url: net::url_with(&net::join(base, &s.fhir_type), &q)?,
        fallback: Vec::new(),
        is_fallback: !extra.is_empty(),
    })
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
#[allow(clippy::too_many_arguments)]
async fn worker<S: Store, T: Timer>(
    net: &Net,
    store: &S,
    timer: &T,
    base: &Text,
    shared: &RefCell<Shared>,
    site: &RefCell<SiteRow>,
    on_progress: &RefCell<&mut impl FnMut(&Progress)>,
    on_call: &RefCell<&mut impl FnMut(&CallNote)>,
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
        run_job(net, store, timer, base, shared, site, &job, on_progress, on_call).await;
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
    on_call: &RefCell<&mut impl FnMut(&CallNote)>,
) {
    let mut next = Some(job.url.clone());
    let mut refreshed = false;
    let mut first = true;
    let mut page_no = 0;
    let emit = |note: CallNote| (on_call.borrow_mut())(&note);
    while let Some(url) = next.take() {
        let (token, generation) = {
            let s = shared.borrow();
            (s.token.clone(), s.token_generation)
        };
        let mut calls = 0;
        let t0 = Stopwatch::start();
        let reply = get(net, timer, &url, Some(&token), &mut calls).await;
        shared.borrow_mut().progress.calls += calls;
        page_no += 1;
        let mut note = CallNote {
            what: job.label.clone(),
            page: page_no,
            status: None,
            tries: calls,
            ms: t0.ms(),
            bytes: 0,
            entries: 0,
            included: 0,
            issues: Vec::new(),
            outcome: "network",
        };
        let reply = match reply {
            Ok(r) => r,
            Err(_) => {
                emit(note);
                warn(store, shared, &job.fhir_type, "network", timer.now());
                return;
            }
        };
        note.status = Some(reply.status);
        note.bytes = net::byte_len(&reply.body);
        match reply.status {
            200..=299 => {}
            401 if !refreshed => {
                refreshed = true;
                note.outcome = "token-renew";
                emit(note);
                if !renew(net, store, timer, shared, site, generation).await {
                    // "Sign in again" ends the whole fetch and says so; any other failure
                    // stops only this job, and must still leave a warning.
                    if !shared.borrow().sign_in_again {
                        warn(store, shared, &job.fhir_type, "token-refresh", timer.now());
                    }
                    return;
                }
                next = Some(url);
                page_no -= 1;
                continue;
            }
            status if first && !job.fallback.is_empty() && is_refusal(status) => {
                note.issues = fhir::read_error(&reply.body).iter().map(fhir::issue_word).collect();
                note.outcome = "fallback";
                emit(note);
                use_fallback(shared, job);
                return;
            }
            status if first && job.is_fallback && is_refusal(status) => {
                note.issues = fhir::read_error(&reply.body).iter().map(fhir::issue_word).collect();
                note.outcome = "category-not-served";
                emit(note);
                return;
            }
            status => {
                let issues = fhir::read_error(&reply.body);
                note.issues = issues.iter().map(fhir::issue_word).collect();
                note.outcome = "refused";
                emit(note);
                let now = timer.now();
                {
                    let mut s = shared.borrow_mut();
                    if !s.skipped.contains(&job.fhir_type) {
                        s.skipped.push(job.fhir_type.clone());
                    }
                }
                warn(store, shared, &job.fhir_type, &format!("http-{status}"), now);
                store_issues(store, shared, &job.fhir_type, issues, now);
                return;
            }
        }
        let page = match fhir::read_page(&reply.body) {
            Ok(p) => p,
            Err(_) => {
                note.outcome = "unreadable";
                emit(note);
                warn(store, shared, &job.fhir_type, "unreadable-page", timer.now());
                return;
            }
        };
        note.included = page.found.iter().filter(|f| f.included).count() as u32;
        note.entries = page.found.len() as u32 - note.included;
        note.issues = page.issues.iter().map(fhir::issue_word).collect();
        // A site may refuse with an empty page and an error issue instead of a 4xx.
        if first && !job.fallback.is_empty() && page.found.is_empty() && has_error_issue(&page.issues) {
            note.outcome = "fallback";
            emit(note);
            use_fallback(shared, job);
            return;
        }
        first = false;
        if let Err(e) = write_page(store, shared, job, &page.found) {
            note.outcome = "store-failed";
            emit(note);
            warn(store, shared, &job.fhir_type, if e == CoreError::Crypto { "seal" } else { "store" }, timer.now());
            return;
        }
        // A `next` link must stay on this site: the token never goes to another host.
        let off_site = page.next.as_ref().is_some_and(|n| !net::on_site(n, base));
        note.outcome = match (&page.next, off_site) {
            (None, _) => "last-page",
            (Some(_), false) => "next-page",
            (Some(_), true) => "next-link-off-site",
        };
        emit(note);
        store_issues(store, shared, &job.fhir_type, page.issues, timer.now());
        let snapshot = shared.borrow().progress.clone();
        (on_progress.borrow_mut())(&snapshot);
        next = match page.next {
            Some(n) if !off_site => Some(n),
            Some(_) => {
                warn(store, shared, &job.fhir_type, "next-link-off-site", timer.now());
                None
            }
            None => None,
        };
    }
}

/// A reply that means "not like this": any 4xx but 401 (the token) and 429 (the pace).
fn is_refusal(status: u16) -> bool {
    (400..500).contains(&status) && status != 401 && status != 429
}

fn has_error_issue(issues: &[Issue]) -> bool {
    let (error, fatal) = (Shielded::new("error".to_owned()), Shielded::new("fatal".to_owned()));
    issues.iter().any(|i| i.severity.as_ref().is_some_and(|s| *s == error || *s == fatal))
}

/// The site refused a plain search: queue its fallback jobs instead. No warning: the refusal
/// is expected on such a site, and the fallback fetches the same records.
fn use_fallback(shared: &RefCell<Shared>, job: &Job) {
    let mut s = shared.borrow_mut();
    s.progress.jobs_total += job.fallback.len();
    s.jobs.extend(job.fallback.iter().cloned());
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

/// Write one page's resources, with their index columns and links, in one transaction.
fn write_page<S: Store>(store: &S, shared: &RefCell<Shared>, job: &Job, found: &[fhir::Found]) -> Result<(), CoreError> {
    store.in_transaction(|st| {
        for f in found {
            let table = if f.included || f.fhir_type != job.fhir_type {
                match fhir::table_for(&f.fhir_type) {
                    Some(t) => t,
                    None => continue,
                }
            } else {
                job.table
            };
            let mut s = shared.borrow_mut();
            let id = s.keys.row_for(st, table, &f.fhir_type, &f.fhir_id)?;
            let n = write_resource(st, &mut s.keys, table, id, &f.fhir_type, &f.fhir_id, &f.index, &f.raw, &f.refs)?;
            s.links += n;
            *s.progress.counts.entry(f.fhir_type.clone()).or_default() += 1;
        }
        Ok(())
    })
}

/// Row keys by (FHIR type, FHIR id), read per table the first time a table is needed.
#[derive(Default)]
pub(crate) struct Keys {
    map: BTreeMap<(String, Text), RowId>,
    read: HashSet<Table>,
    /// Time spent reading them (CORE-CRATE.md §12, stage 3 left this to measure).
    ms: u128,
}

impl Keys {
    fn load<S: Store>(&mut self, store: &S, table: Table) -> Result<(), CoreError> {
        if self.read.insert(table) {
            let t0 = Stopwatch::start();
            for (row, t, id) in store.keys(table)? {
                self.map.insert((t, id), row);
            }
            self.ms += t0.ms();
        }
        Ok(())
    }

    /// The row a resource has, if it is stored.
    fn find<S: Store>(&mut self, store: &S, table: Table, fhir_type: &str, fhir_id: &Text) -> Result<Option<RowId>, CoreError> {
        self.load(store, table)?;
        Ok(self.map.get(&(fhir_type.to_owned(), fhir_id.clone())).cloned())
    }

    /// The row for a resource: the one it had, or a new one.
    fn row_for<S: Store>(&mut self, store: &S, table: Table, fhir_type: &str, fhir_id: &Text) -> Result<RowId, CoreError> {
        if let Some(row) = self.find(store, table, fhir_type, fhir_id)? {
            return Ok(row);
        }
        let row = store.new_row_id()?;
        self.map.insert((fhir_type.to_owned(), fhir_id.clone()), row.clone());
        Ok(row)
    }
}

/// One resource and its links: the row replaces the old one, and its old links go. A link
/// whose target is stored already points at it; the rest wait for [`resolve_links`].
/// Returns the number of links written.
#[allow(clippy::too_many_arguments)]
fn write_resource<S: Store>(
    st: &S,
    keys: &mut Keys,
    table: Table,
    id: RowId,
    fhir_type: &str,
    fhir_id: &Text,
    index: &Index,
    raw: &Text,
    refs: &[Ref],
) -> Result<u32, CoreError> {
    st.put_resource(
        table,
        &ResourceRow {
            id: id.clone(),
            fhir_type: Shielded::new(fhir_type.to_owned()),
            fhir_id: fhir_id.clone(),
            kind: index.kind.clone().unwrap_or_else(|| Shielded::new(fhir_type.to_lowercase())),
            code_system: index.code_system.clone(),
            code: index.code.clone(),
            code_display: index.code_display.clone(),
            status: index.status.clone(),
            start: index.start.clone(),
            end: index.end.clone(),
            recorded: index.recorded.clone(),
            last_updated: index.last_updated.clone(),
            canonical: index.canonical.clone(),
            version: index.version.clone(),
            raw: raw.clone(),
        },
    )?;
    st.delete_links_from(&id)?;
    for r in refs {
        let to_id = keys.find(st, r.target_table, &r.target_type, &r.target_id)?;
        st.put_link(&LinkRow {
            id: st.new_row_id()?,
            from_table: table,
            from_id: id.clone(),
            to_table: r.target_table,
            to_id,
            role: Shielded::new(r.role.to_owned()),
            source_field: Shielded::new(r.source_field.clone()),
            target_ref: r.target_ref.clone(),
        })?;
    }
    Ok(refs.len() as u32)
}

/// Point every waiting link at its target, now that more rows are stored. Returns the links
/// resolved and the links still waiting (their target was never fetched). Call inside a
/// transaction.
pub(crate) fn resolve_links<S: Store>(store: &S, keys: &mut Keys) -> Result<(u32, u32, BTreeMap<String, u32>), CoreError> {
    let (mut resolved, mut waiting) = (0, 0);
    let mut by_type = BTreeMap::new();
    for l in store.unresolved_links()? {
        let parts = net::split_ref(&l.target_ref);
        let target = match &parts {
            Some((t, id)) => keys.find(store, l.to_table, t, id)?,
            None => None,
        };
        match target {
            Some(row) => {
                store.set_link_target(&l.id, &row)?;
                resolved += 1;
            }
            None => {
                waiting += 1;
                // Only a FHIR type name is let out; anything else reads `other`.
                let word = match &parts {
                    Some((t, _)) if fhir::table_for(t).is_some() => t.clone(),
                    Some(_) => "other".to_owned(),
                    None => "not-a-reference".to_owned(),
                };
                *by_type.entry(word).or_default() += 1;
            }
        }
    }
    Ok((resolved, waiting, by_type))
}

/// Counts only.
pub struct Reindexed {
    pub rows: u32,
    pub links: u32,
    pub waiting: u32,
}

/// Re-index the store if its rows were built with older index rules
/// ([`crate::index::INDEX_VERSION`]). `None`: already current. The host calls it whenever it
/// opens a site file for work (the owner, 2026-09-21: automatic, by version).
pub fn reindex_if_stale<S: Store>(store: &S) -> Result<Option<Reindexed>, CoreError> {
    if store.index_version()? >= crate::index::INDEX_VERSION {
        return Ok(None);
    }
    reindex(store).map(Some)
}

/// Re-index every stored row from its raw resource: new index columns and new links, with no
/// fetch (DATA-MODEL.md §6, decision 2). The store is then marked current, in the same
/// transaction.
pub fn reindex<S: Store>(store: &S) -> Result<Reindexed, CoreError> {
    let mut keys = Keys::default();
    store.in_transaction(|st| {
        let (mut rows, mut links) = (0, 0);
        for table in Table::ALL {
            for row in st.resources(table)? {
                let (t, index, refs) = net::parse_reply(&row.raw, |v| {
                    let t = v.get("resourceType").and_then(|x| x.as_str()).ok_or(CoreError::Parse)?;
                    Ok((t.to_owned(), crate::index::extract(t, v), crate::index::references(t, v)))
                })?;
                links += write_resource(st, &mut keys, table, row.id.clone(), &t, &row.fhir_id, &index, &row.raw, &refs)?;
                rows += 1;
            }
        }
        let (_, waiting, _) = resolve_links(st, &mut keys)?;
        st.set_index_version(crate::index::INDEX_VERSION)?;
        Ok(Reindexed { rows, links, waiting })
    })
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

/// A warning of our own: what went wrong with a job, as a fixed code. Every call stops a job,
/// so the type is marked as stopped too.
fn warn<S: Store>(store: &S, shared: &RefCell<Shared>, fhir_type: &str, code: &str, now: u64) {
    shared.borrow_mut().stopped.insert(fhir_type.to_owned());
    let issue = Issue { severity: Some(Shielded::new("error".into())), code: Some(Shielded::new(code.into())), text: None, detail: None };
    // Our own code is a fixed word, so it is tallied as it is.
    write_issues(store, shared, fhir_type, vec![(issue, code.to_owned())], now);
}

/// The site's issues: tallied as [`fhir::issue_word`] gives them, stored sealed.
fn store_issues<S: Store>(store: &S, shared: &RefCell<Shared>, fhir_type: &str, issues: Vec<Issue>, now: u64) {
    let worded = issues.into_iter().map(|i| {
        let w = fhir::issue_word(&i);
        (i, w)
    });
    write_issues(store, shared, fhir_type, worded.collect(), now);
}

fn write_issues<S: Store>(store: &S, shared: &RefCell<Shared>, fhir_type: &str, issues: Vec<(Issue, String)>, now: u64) {
    for (i, word) in issues {
        *shared.borrow_mut().warnings_by_code.entry(format!("{fhir_type} {word}")).or_default() += 1;
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
