//! The gRPC service (EHR-SERVICE.md §1). Each call: the edge checks the request, the session
//! decides whether the caller may ask, the adapters answer, the edge builds the reply.
//!
//! Every error message is a fixed string. None is built from a request or the EHR's reply.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use tonic::{Request, Response, Status};

use crate::{
    access::{self, Actor, Level, Lever, Refused},
    audit::{self, Entry},
    edge,
    epic_ehr::{EpicEhr, EpicSandbox},
    hospital::{ClinicianRef, Hospital, Pick, PickOutcome, PlaceAnswer, PlaceRef, PlaceSearch, RuleOutcome},
    proto::v1::{self as pb, ehr_server::Ehr},
    scheduler::{
        BookResult, DayRange, FoundSlots, FoundVisits, Scheduler, SchedulerDown, SeenVisit, Slot, SlotId, SlotWish, VisitAsk, VisitStatus,
    },
    visit_about::BookNote,
    visits::{narrow, Found, Narrow},
    session::{Pending, PatientKey, Refusal, Sessions, Source, Stage, Token},
    settings,
    store::{self, Store},
    users,
    verify::{one_match, Candidate, Codes, Down, Patients},
};
use zeroize::Zeroizing;

/// The scheduler a call uses, by the source it opened with. Owned, so a booking's own task can
/// hold it (`book`).
enum Sched<S> {
    Local(Arc<S>),
    Epic(Arc<EpicEhr>),
}

impl<S: Scheduler> Sched<S> {
    async fn find(&self, h: &Hospital, patient: &PatientKey, wish: &SlotWish) -> Result<FoundSlots, SchedulerDown> {
        match self {
            Sched::Local(s) => s.find(h, patient, wish).await,
            Sched::Epic(e) => e.scheduler.find(h, patient, wish).await,
        }
    }

    async fn find_more(&self, h: &Hospital, patient: &PatientKey, next: &[String]) -> Result<FoundSlots, SchedulerDown> {
        match self {
            Sched::Local(s) => s.find_more(h, patient, next).await,
            Sched::Epic(e) => e.scheduler.find_more(h, patient, next).await,
        }
    }

    async fn book(&self, h: &Hospital, patient: &PatientKey, slot: &SlotId, note: &BookNote) -> Result<BookResult, SchedulerDown> {
        match self {
            Sched::Local(s) => s.book(h, patient, slot, note).await,
            Sched::Epic(e) => e.scheduler.book(h, patient, slot, note).await,
        }
    }

    async fn visits(&self, h: &Hospital, patient: &PatientKey, ask: &VisitAsk) -> Result<FoundVisits, SchedulerDown> {
        match self {
            Sched::Local(s) => s.visits(h, patient, ask).await,
            Sched::Epic(e) => e.scheduler.visits(h, patient, ask).await,
        }
    }
}

/// Runs `work`, the EHR's part of one gRPC call, in its own task with its own EHR ledger
/// (ledger.rs). The ledger goes into the session when the work ends, even if the agent stopped
/// waiting: the Camila call of 2026-11-15 timed out, and what Epic did was lost.
async fn ledgered<T: Send + 'static>(
    session: &Arc<tokio::sync::Mutex<crate::session::Session>>,
    rpc: &'static str,
    work: impl std::future::Future<Output = T> + Send + 'static,
) -> Result<T, Status> {
    let session = Arc::clone(session);
    tokio::spawn(async move {
        let (out, entries) = crate::ledger::kept(work).await;
        session.lock().await.ledger.extend(entries.into_iter().map(|e| (rpc, e)));
        out
    })
    .await
    .map_err(|_| Status::internal("the EHR request stopped"))
}

/// What an RPC needs from its caller (docs/architecture/ACCESS-ROLES.md §5, §8.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Need {
    /// The call loop: no person. A call is bound to its caller by its handle (session.rs).
    Call,
    /// The agent on its own, no person: a sign-in check, the start-up seed, the owner's row.
    System,
    /// A settings service, the lever by service (settings.rs › `lever`). With a person: View to
    /// read, Edit to save. With none: the agent itself (a call reading its settings, the prompt
    /// file synced, a Default seed), allowed and audited as the system.
    Settings(Level),
    /// Any person on the list.
    Person,
    /// A person holding this lever at this level.
    Lever(Lever, Level),
    /// The same, or the agent on its own when it says so (`deets-actor: system`): the reads it
    /// makes at start (the People list for Cloudflare's group, the annotations for its catalog).
    LeverOrSystem(Lever, Level),
}

/// Every RPC of `ehr.proto` and what it needs. A test fails when an RPC is missing here.
pub const RPC_LEVERS: &[(&str, Need)] = &[
    ("OpenCall", Need::Call),
    ("CloseCall", Need::Call),
    ("Identify", Need::Call),
    ("ResendCode", Need::Call),
    ("CheckCode", Need::Call),
    ("FindSlots", Need::Call),
    ("Book", Need::Call),
    ("FindVisit", Need::Call),
    ("PlaceInfo", Need::Call),
    ("FileCall", Need::Call),
    ("FileTestRun", Need::Lever(Lever::Testing, Level::Edit)),
    ("AddAnnotation", Need::Lever(Lever::Testing, Level::Edit)),
    ("ListAnnotations", Need::LeverOrSystem(Lever::Testing, Level::View)),
    ("ListSettingVersions", Need::Settings(Level::View)),
    ("GetSettings", Need::Settings(Level::View)),
    ("SaveSettings", Need::Settings(Level::Edit)),
    ("ListUsers", Need::LeverOrSystem(Lever::People, Level::View)),
    ("AddUser", Need::Lever(Lever::People, Level::Edit)),
    ("RemoveUser", Need::Lever(Lever::People, Level::Edit)),
    ("AdmitUser", Need::System),
    ("SeedUsers", Need::System),
    ("UpdateUser", Need::Lever(Lever::People, Level::Edit)),
    ("GetUserPhoto", Need::Lever(Lever::People, Level::View)),
    ("SetUserPhoto", Need::Lever(Lever::People, Level::Edit)),
    ("SeenOwner", Need::System),
    ("Levels", Need::Person),
    ("GrantRole", Need::Lever(Lever::Roles, Level::Edit)),
    ("RevokeRole", Need::Lever(Lever::Roles, Level::Edit)),
    ("ReadAudit", Need::Lever(Lever::AuditLogs, Level::View)),
    ("VerifyAudit", Need::Lever(Lever::AuditLogs, Level::View)),
    ("LogAccess", Need::Person),
    ("ImportPlaces", Need::Lever(Lever::Rules, Level::Edit)),
];

/// The gRPC metadata that names the person behind a staff request: their user id, or `system`
/// when the agent acts on its own.
pub const ACTOR: &str = "deets-actor";
/// The `deets-actor` value of the agent acting on its own (`Need::Settings`, `LeverOrSystem`).
pub const SYSTEM: &str = "system";

/// What `rpc` needs. An RPC missing from the table needs the most: Roles Edit (and the coverage
/// test fails).
pub fn need_of(rpc: &str) -> Need {
    RPC_LEVERS.iter().find(|(name, _)| *name == rpc).map_or(Need::Lever(Lever::Roles, Level::Edit), |(_, n)| *n)
}

/// An RPC's name as an audit action: `SaveSettings` → `rpc.save_settings`.
fn rpc_action(rpc: &str) -> String {
    let mut out = String::from("rpc.");
    for (i, c) in rpc.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A refusal from access.rs as a status: fixed words, never a name.
fn refused_status(r: Refused) -> Status {
    match r {
        Refused::Needs(lever, level) => Status::permission_denied(format!("needs {} {}", lever.word(), level.word())),
        Refused::Rule(why) => Status::permission_denied(why),
        Refused::NotFound(why) => Status::not_found(why),
        Refused::Store(_) => Status::internal("the store could not check the access"),
    }
}

/// How many times a caller may give a name and date of birth on one call.
pub const MAX_IDENTIFIES: u32 = 3;
/// How many times a code may be sent again on one call.
pub const MAX_RESENDS: u32 = 2;
/// Wrong codes before the chart path closes (SECURITY.md §4.4: three).
pub const MAX_CODE_FAILURES: u32 = 3;
/// How long a code works.
pub const CODE_LIFETIME: Duration = Duration::from_secs(10 * 60);

pub struct EhrService<S, P, C> {
    sessions: Sessions,
    scheduler: Arc<S>,
    patients: P,
    codes: C,
    /// The call store (DATABASE.md), when ehr has one: `FileCall` files each ended call.
    store: Option<Arc<Store>>,
    /// Epic's sandbox (epic_ehr.rs), when ehr serves it: a call that opens with
    /// `EHR_SOURCE_EPIC_SANDBOX` finds the caller and their visits there (dev only). It may be
    /// not signed in yet while Epic is down (`EpicSandbox::start`).
    epic: Option<Arc<EpicSandbox>>,
}

impl<S: Scheduler, P: Patients, C: Codes> EhrService<S, P, C> {
    pub fn new(scheduler: S, patients: P, codes: C) -> Self {
        Self {
            sessions: Sessions::default(),
            scheduler: Arc::new(scheduler),
            patients,
            codes,
            store: None,
            epic: None,
        }
    }

    /// Files each ended call into `store` (FileCall).
    pub fn with_store(mut self, store: Arc<Store>) -> Self {
        self.store = Some(store);
        self
    }

    /// Serves Epic's sandbox as a second source (OpenCall's `ehr`).
    pub fn with_epic(self, epic: Arc<EpicEhr>) -> Self {
        self.with_epic_sandbox(EpicSandbox::ready(epic))
    }

    /// The same, signed in now or later (main.rs).
    pub fn with_epic_sandbox(mut self, epic: Arc<EpicSandbox>) -> Self {
        self.epic = Some(epic);
        self
    }

    fn settings_store(&self) -> Result<&Arc<Store>, Status> {
        self.store.as_ref().ok_or_else(|| Status::failed_precondition("ehr runs without a store"))
    }

    /// Writes a refusal's audit row on its own and returns `status`. A failed write is noted
    /// (no value) and the request is still refused.
    async fn refuse(&self, entry: Entry, status: Status) -> Status {
        if let Some(store) = self.store.as_ref() {
            if audit::write_alone(store.pool(), &entry.refused()).await.is_err() {
                eprintln!("ehr: a refusal's audit row could not be written");
            }
        }
        status
    }

    /// The gate every RPC passes first (docs/architecture/ACCESS-ROLES.md §5): who the request names
    /// (`deets-actor`) and whether they hold what `rpc` needs (`RPC_LEVERS`). Some(person) for a
    /// person; None where no person is needed or the agent acts on its own. ehr reads the
    /// person's levels from the store itself, never from the request. A refusal is audited.
    /// `service`: the settings service a settings RPC names.
    async fn gate(&self, rpc: &'static str, meta: &tonic::metadata::MetadataMap, service: Option<&str>) -> Result<Option<Actor>, Status> {
        let need = need_of(rpc);
        if matches!(need, Need::Call | Need::System) {
            return Ok(None);
        }
        let store = self.settings_store()?;
        let action = rpc_action(rpc);
        let Some(named) = meta.get(ACTOR) else {
            if matches!(need, Need::Settings(_)) {
                return Ok(None);
            }
            let entry = Entry { unknown: true, ..Entry::by(None, &action) };
            return Err(self.refuse(entry, Status::permission_denied("no person named")).await);
        };
        let id = named.to_str().unwrap_or("");
        if id == SYSTEM {
            if matches!(need, Need::Settings(_) | Need::LeverOrSystem(..)) {
                return Ok(None);
            }
            let entry = Entry { unknown: true, ..Entry::by(None, &action) };
            return Err(self.refuse(entry, Status::permission_denied("needs a person")).await);
        }
        let found = if access::uuid_ok(id) {
            access::actor_in(store.pool(), id).await.map_err(|_| Status::internal("the store could not read the access"))?
        } else {
            None
        };
        let Some(actor) = found else {
            let entry = Entry { unknown: true, ..Entry::by(None, &action) };
            return Err(self.refuse(entry, Status::permission_denied("not a person on the list")).await);
        };
        let (lever, level) = match need {
            Need::Settings(level) => (settings::lever(service.unwrap_or("")), level),
            Need::Lever(lever, level) | Need::LeverOrSystem(lever, level) => (lever, level),
            Need::Person | Need::Call | Need::System => return Ok(Some(actor)),
        };
        if !actor.levels.may(lever, level) {
            let entry = Entry::by(Some(&actor), &action).lever(lever, level);
            return Err(self.refuse(entry, refused_status(Refused::Needs(lever, level))).await);
        }
        Ok(Some(actor))
    }

    /// The rules of a change to another person (access.rs): `remove` for a removal (and the last
    /// manager), else a card or picture edit. A refusal is audited.
    async fn may_change_person(&self, who: &Actor, rpc: &'static str, user_id: &str, remove: bool) -> Result<(), Status> {
        let store = self.settings_store()?;
        let mut conn = store.pool().acquire().await.map_err(|_| Status::internal("the store could not check the access"))?;
        let checked = if remove {
            access::may_remove(&mut conn, who, user_id).await
        } else {
            access::may_edit_card(&mut conn, who, user_id).await
        };
        if let Err(r) = checked {
            let entry = Entry::by(Some(who), &rpc_action(rpc)).lever(Lever::People, Level::Edit).object("person", user_id);
            return Err(self.refuse(entry, refused_status(r)).await);
        }
        Ok(())
    }

    /// A grant or a revoke, with its audit row in the same transaction; a refusal audited alone.
    async fn change_role(&self, who: &Actor, user_id: &str, role_id: &str, grant: bool) -> Result<(), Status> {
        let store = self.settings_store()?;
        if !access::uuid_ok(user_id) || !access::role_id_ok(role_id) {
            return Err(Status::invalid_argument("user_id and role_id: a person's id and a role's id"));
        }
        let mut tx = store.pool().begin().await.map_err(|_| Status::internal("the store could not change the role"))?;
        let done = if grant {
            access::grant(&mut tx, who, user_id, role_id).await
        } else {
            access::revoke(&mut tx, who, user_id, role_id).await
        };
        let entry = Entry::by(Some(who), if grant { "role.grant" } else { "role.revoke" })
            .lever(Lever::Roles, Level::Edit)
            .object("person", user_id)
            .change(vec![format!("role:{role_id}")], None, None);
        if let Err(r) = done {
            drop(tx);
            return Err(self.refuse(entry, refused_status(r)).await);
        }
        audit::write(&mut tx, &entry).await.map_err(|_| Status::internal("the store could not write the audit row"))?;
        tx.commit().await.map_err(|_| Status::internal("the store could not change the role"))?;
        Ok(())
    }

    /// `text.code` of one saved prompt version: the words were checked on save ({code} and no
    /// other placeholder). NOT_FOUND for a version that does not exist.
    async fn prompt_text_code(&self, id: i64) -> Result<String, Status> {
        let store = self.settings_store()?;
        let (_, _, table) = setting_service("prompt")?;
        let found = settings::get(store.pool(), "prompt", table, id)
            .await
            .map_err(|_| Status::internal("the store could not read the prompt"))?;
        let Some((_, values)) = found else {
            return Err(Status::not_found("prompt_version: no such version"));
        };
        values
            .into_iter()
            .find(|(k, _)| k == "text.code")
            .and_then(|(_, v)| serde_json::from_str::<String>(&v).ok())
            .ok_or_else(|| Status::internal("the prompt version has no text.code"))
    }

    /// The System variables of one saved version: the active one when `id` is 0, and the file's
    /// (compiled in) when the store has none yet. NOT_FOUND for a version that does not exist.
    async fn system_variables(&self, id: i64) -> Result<settings::Variables, Status> {
        let store = self.settings_store()?;
        let (_, _, table) = setting_service("system")?;
        let found = settings::get(store.pool(), "system", table, id)
            .await
            .map_err(|_| Status::internal("the store could not read the System variables"))?;
        let Some((_, values)) = found else {
            if id != 0 {
                return Err(Status::not_found("system_version: no such version"));
            }
            return settings::file_variables().map_err(|_| Status::internal("VARIABLES.md does not fit"));
        };
        let raw = values
            .into_iter()
            .find(|(k, _)| k == "variables")
            .and_then(|(_, v)| serde_json::from_str::<serde_json::Value>(&v).ok())
            .ok_or_else(|| Status::internal("the System version has no variables"))?;
        settings::variables(&raw).map_err(|_| Status::internal("the System version does not fit"))
    }

    /// The hospital a new call runs with (HOSPITAL-CONFIG.md), and its version's label: the
    /// active version, as the agent labels it ("MM-DD | n", or "Default"); the file, labelled
    /// "file", when ehr has no store, the store has no version yet, or it cannot be read. A
    /// call never fails for want of its hospital.
    async fn call_hospital(&self) -> (Arc<Hospital>, String) {
        let file = || (Hospital::file(), "file".to_string());
        let Some(store) = self.store.as_ref() else { return file() };
        let Ok((_, _, table)) = setting_service("hospital") else { return file() };
        let found = match settings::get(store.pool(), "hospital", table, 0).await {
            Ok(Some(found)) => found,
            Ok(None) => return file(),
            Err(_) => {
                eprintln!("ehr: the hospital version could not be read; this call uses the file");
                return file();
            }
        };
        let (version, values) = found;
        let parsed: Vec<(String, serde_json::Value)> =
            values.into_iter().filter_map(|(k, v)| serde_json::from_str(&v).ok().map(|v| (k, v))).collect();
        match Hospital::from_pairs(parsed.iter().map(|(k, v)| (k.as_str(), v))) {
            Ok(h) => {
                let label = if version.seq == 0 { "Default".to_string() } else { format!("{} | {}", version.day.get(5..).unwrap_or(""), version.seq) };
                (Arc::new(h), label)
            }
            Err(_) => {
                eprintln!("ehr: the active hospital version does not fit; this call uses the file");
                file()
            }
        }
    }

    fn epic(&self) -> Result<Arc<EpicEhr>, Status> {
        let sandbox =
            self.epic.as_ref().ok_or_else(|| Status::failed_precondition("ehr does not serve Epic's sandbox"))?;
        sandbox.get().ok_or_else(|| Status::unavailable("Epic's sandbox is not answering"))
    }

    fn sched(&self, source: Source) -> Result<Sched<S>, Status> {
        Ok(match source {
            Source::Local => Sched::Local(Arc::clone(&self.scheduler)),
            Source::EpicSandbox => Sched::Epic(self.epic()?),
        })
    }

    /// Where the verified caller was seen in the hospital's window (`established_years`): the
    /// places of their seen visits (BOOKING-RULES.md §3.4). Read once a call, at the first
    /// search that needs it (one visit search at Epic, in FindSlots's ledger), then kept in the
    /// session for the call.
    async fn seen(
        &self,
        call: &Arc<tokio::sync::Mutex<crate::session::Session>>,
        h: &Arc<Hospital>,
        patient: &PatientKey,
        source: Source,
    ) -> Result<Vec<SeenVisit>, Status> {
        if let Some(seen) = call.lock().await.seen.clone() {
            return Ok(seen);
        }
        let today = jiff::Timestamp::now().to_zoned_datetime(h.tz().clone()).date();
        let from = today.checked_sub(jiff::Span::new().years(h.established_years)).map_err(|_| Status::internal("the window"))?;
        let to = today.tomorrow().map_err(|_| Status::internal("the window"))?;
        let ask = VisitAsk {
            range: Some(DayRange { from: shielded_day(from), to: shielded_day(to) }),
            past: false,
        };
        let (sched, hh, who) = (self.sched(source)?, Arc::clone(h), patient.clone());
        let found = ledgered(call, "FindSlots", async move { sched.visits(&hh, &who, &ask).await })
            .await?
            .map_err(scheduler_down)?;
        let mut seen: Vec<SeenVisit> = found
            .visits
            .into_iter()
            .filter(|v| v.status == VisitStatus::Seen)
            .map(|v| SeenVisit { place: v.place_ref, place_words: v.place, clinician: v.clinician_ref, starts_at: v.starts_at })
            .collect();
        // Newest first: the last visit decides the usual place.
        seen.sort_by(|a, b| b.starts_at.cmp(&a.starts_at));
        call.lock().await.seen = Some(seen.clone());
        Ok(seen)
    }
}

/// The caller's usual place for this search (BOOKING-RULES.md §3.2), and the PCP seen there. For
/// primary care: the last seen visit with a PCP on file (when there are two, the one seen last:
/// case 4b), else the last at a department of a primary care specialty. For a specialty: the
/// last at one of its departments. None: no such visit (cases 2 and 5).
fn usual_place(h: &Hospital, seen: &[SeenVisit], pcp: &[ClinicianRef], pick: &Pick) -> Option<(SeenVisit, Option<ClinicianRef>)> {
    if !pick.primary_care {
        return seen.iter().find(|v| Hospital::among(&pick.departments, &v.place)).map(|v| (v.clone(), None));
    }
    if let Some((v, p)) = seen.iter().find_map(|v| pcp.iter().find(|p| v.clinician.same(p)).map(|p| (v, p))) {
        return Some((v.clone(), Some(p.clone())));
    }
    seen.iter().find(|v| h.primary_care_at(&v.place)).map(|v| (v.clone(), None))
}

/// The Booking rules before a search (BOOKING-RULES.md §3.5): the outcome when every place the
/// search would look at holds the visit back for this caller (Referral needed, Hand off), known
/// from the place alone. None: some place may offer a time, a rule depends on each time, or there
/// is no rule or no place to check.
fn held_back_everywhere(h: &Hospital, places: &[PlaceRef], seen: &[PlaceRef]) -> Option<RuleOutcome> {
    if h.booking_rules.is_empty() || places.is_empty() {
        return None;
    }
    let mut first = None;
    for p in places {
        let o = h.rule_before_search(p, h.established(seen, p))?;
        if !o.holds_back() {
            return None;
        }
        first.get_or_insert(o);
    }
    first
}

/// The Booking rules per time (§3.5): drops each time its rule holds back, and each time with
/// another clinician under "Book with the PCP only" (when the caller has a PCP on file); marks
/// each time whose rule says Book and flag. Returns the first held-back outcome, for when no
/// time is left.
fn keep_by_rules(
    h: &Hospital,
    seen: &[PlaceRef],
    pcp: Option<&ClinicianRef>,
    slots: &mut Vec<Slot>,
    flag_on_book: &mut std::collections::HashSet<SlotId>,
) -> Option<RuleOutcome> {
    if h.booking_rules.is_empty() {
        return None;
    }
    let mut held = None;
    slots.retain(|s| {
        let o = h.rule_for(s, h.established(seen, &s.place_ref));
        if o.holds_back() {
            held.get_or_insert(o);
            return false;
        }
        if o == RuleOutcome::PcpOnly && pcp.is_some_and(|p| !s.clinician_ref.same(p)) {
            return false;
        }
        if o == RuleOutcome::BookAndFlag {
            flag_on_book.insert(s.id.clone());
        }
        true
    });
    held
}

fn rule_ask(o: RuleOutcome) -> pb::RuleAsk {
    match o {
        RuleOutcome::ReferralNeeded => pb::RuleAsk::ReferralNeeded,
        RuleOutcome::HandOff => pb::RuleAsk::HandOff,
        _ => pb::RuleAsk::None,
    }
}

/// A reply that asks about the place or the kind of care, and searched nothing (§3.1): the
/// places to name, at most the hospital's "groups one question names".
fn place_question(h: &Hospital, ask: pb::PlaceAsk, names: &[String]) -> pb::FindSlotsReply {
    let named = if ask == pb::PlaceAsk::Which { names.iter().take(h.groups_named).cloned().collect() } else { Vec::new() };
    pb::FindSlotsReply {
        place_ask: ask.into(),
        other_places: ask == pb::PlaceAsk::Which && names.len() > h.groups_named,
        places: named,
        ..Default::default()
    }
}

/// The PCP's times first (BOOKING-RULES.md §3.3), in their own order, then the others. With the
/// hospital's P-A setting on, the soonest time with another clinician on an earlier day than
/// the PCP's first goes in the first offer, as its last option; its id comes back, so the reply
/// marks it. Slots with no PCP among them keep their order.
fn pcp_first(h: &Hospital, pcp: Option<&ClinicianRef>, slots: &mut Vec<Slot>, offered_at_once: usize) -> Option<SlotId> {
    let pcp = pcp?;
    let (mut mine, others): (Vec<Slot>, Vec<Slot>) = std::mem::take(slots).into_iter().partition(|s| s.clinician_ref.same(pcp));
    if mine.is_empty() {
        *slots = others;
        return None;
    }
    let mut others = others;
    let sooner = if h.sooner_with_others {
        let first = mine[0].starts_at.clone();
        others.iter().position(|s| s.starts_at.earlier_day(&first)).map(|i| others.remove(i))
    } else {
        None
    };
    let id = sooner.as_ref().map(|s| s.id.clone());
    if let Some(s) = sooner {
        let at = mine.len().min(offered_at_once.saturating_sub(1));
        mine.insert(at, s);
    }
    mine.extend(others);
    *slots = mine;
    id
}

/// A day, wrapped: a window's edge is a search input like the caller's own days.
fn shielded_day(d: jiff::civil::Date) -> deetshealth_core::Shielded<String> {
    deetshealth_core::Shielded::new(d.to_string())
}

/// Keeps the slots whose visit type fits the caller at the slot's place: a type for new
/// patients only to a caller new to that specialty, one for established patients only to a
/// caller seen by it in the window (BOOKING-RULES.md §3.4). Every slot fits when no type the
/// agent may book is for one kind. The same for every EHR.
fn keep_fitting(h: &Hospital, seen: &[PlaceRef], slots: &mut Vec<Slot>) {
    if h.sorts_patients() {
        slots.retain(|s| s.kind.fits(h.established(seen, &s.place_ref)));
    }
}

/// The longest record FileCall takes, in lines and in bytes. A half-hour call is far below both.
pub const MAX_RECORD_LINES: usize = 20_000;
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

fn refused(r: Refusal) -> Status {
    match r {
        // A wrong handle, a closed call and an expired call look the same.
        Refusal::NoSession => Status::unauthenticated("no such call"),
        Refusal::NotVerified => Status::permission_denied("the caller is not verified"),
        Refusal::Full => Status::resource_exhausted("too many open calls"),
    }
}

fn scheduler_down(_: SchedulerDown) -> Status {
    Status::unavailable("the scheduler did not answer")
}

fn down(_: Down) -> Status {
    Status::unavailable("the EHR or the text service did not answer")
}

#[tonic::async_trait]
impl<S: Scheduler, P: Patients, C: Codes> Ehr for EhrService<S, P, C> {
    async fn open_call(
        &self,
        req: Request<pb::OpenCallRequest>,
    ) -> Result<Response<pb::OpenCallReply>, Status> {
        let source = edge::source(req.get_ref())?;
        // A synthetic call reaches only a sandbox; a real one only a production EHR.
        edge::data_class(req.get_ref(), source)?;
        if source == Source::EpicSandbox {
            self.epic()?;
        }
        // The text message's words from the call's prompt version (CONSOLE.md §9), read once:
        // a save during the call does not change them.
        // Its System variables are filled now, from the call's System version (SYSTEM-PAGE.md
        // §3.4); `{code}` is filled when the text goes out.
        let system = req.get_ref().system_version;
        if system < 0 {
            return Err(Status::invalid_argument("system_version: not a version"));
        }
        let template = match req.get_ref().prompt_version {
            0 => None,
            id if id < 0 => return Err(Status::invalid_argument("prompt_version: not a version")),
            id => {
                let words = self.prompt_text_code(id).await?;
                let vars = self.system_variables(system).await?;
                Some(settings::fill(&words, &|n| vars.get(n).map(|(_, v)| v.clone()), false))
            }
        };
        // The call's hospital, read once: the same for every EHR source (the owner's 2A).
        let (hospital, hospital_version) = self.call_hospital().await;
        let times_of_day = hospital.time_names();
        let books_both_modes = hospital.books_both_modes(source == Source::EpicSandbox);
        let (local_now, zone) = crate::sandbox::local_now(hospital.time_zone.clone());
        let handle = self.sessions.open(source, hospital).await.map_err(refused)?;
        if template.is_some() {
            let session = self.sessions.get(&handle).await.map_err(refused)?;
            session.lock().await.text_template = template;
        }
        Ok(Response::new(pb::OpenCallReply {
            handle: handle.to_hex(),
            identifies_left: MAX_IDENTIFIES,
            resends_left: MAX_RESENDS,
            code_tries_left: MAX_CODE_FAILURES,
            local_now,
            zone,
            times_of_day,
            hospital_version,
            books_both_modes,
        }))
    }

    async fn close_call(
        &self,
        req: Request<pb::CloseCallRequest>,
    ) -> Result<Response<pb::CloseCallReply>, Status> {
        let handle = edge::handle(&req.get_ref().handle)?;
        // The call's EHR ledger goes back with the close, for the dev call record
        // (DEBUGGING.md §3): names and counts only.
        let ledger = match self.sessions.close(&handle).await {
            Some(session) => {
                let session = session.lock().await;
                session
                    .ledger
                    .iter()
                    .map(|(rpc, e)| pb::EhrRequest {
                        rpc: (*rpc).to_string(),
                        what: e.what.clone(),
                        status: u32::from(e.status),
                        ms: e.ms,
                        results: e.results,
                        bytes: e.bytes,
                        at_ms: session.ms_since_open(e.at),
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        Ok(Response::new(pb::CloseCallReply { ledger }))
    }

    async fn identify(
        &self,
        req: Request<pb::IdentifyRequest>,
    ) -> Result<Response<pb::IdentifyReply>, Status> {
        let (handle, claimed) = edge::identify(req.into_inner())?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let mut session = session.lock().await;
        match session.stage {
            Stage::Unverified(_) => {}
            Stage::Verified(_) => return Err(Status::failed_precondition("already verified")),
            Stage::Locked => return Err(refused(Refusal::NotVerified)),
        }
        if session.identifies >= MAX_IDENTIFIES {
            return Err(Status::failed_precondition("no tries left"));
        }
        session.identifies += 1;

        // A new claim replaces the old one and its code.
        session.stage = Stage::Unverified(None);
        let (found, entries) = match session.source {
            Source::Local => crate::ledger::kept(self.patients.candidates(&claimed)).await,
            Source::EpicSandbox => {
                let epic = self.epic()?;
                let claimed = &claimed;
                crate::ledger::kept(async move { epic.patients.candidates(claimed).await }).await
            }
        };
        session.ledger.extend(entries.into_iter().map(|e| ("Identify", e)));
        let found = found.map_err(down)?;
        // One match with a number that takes a text: a code goes out. With no such number on
        // file, nothing is sent, the same as no match.
        if let Some(Candidate { patient, phone: Some(phone), facts, pcp, .. }) = one_match(&claimed, found) {
            let code = self.codes.issue();
            self.codes.send(&phone, &code, session.text_template.as_deref()).await.map_err(down)?;
            session.stage = Stage::Unverified(Some(Pending {
                patient,
                facts,
                pcp,
                phone,
                code,
                issued: std::time::Instant::now(),
            }));
        }
        // The same reply whether or not one matched (SECURITY.md §4.4).
        Ok(Response::new(pb::IdentifyReply { tries_left: MAX_IDENTIFIES - session.identifies }))
    }

    async fn resend_code(
        &self,
        req: Request<pb::ResendCodeRequest>,
    ) -> Result<Response<pb::ResendCodeReply>, Status> {
        let handle = edge::handle(&req.get_ref().handle)?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let mut session = session.lock().await;
        if !matches!(session.stage, Stage::Unverified(_)) {
            return Err(Status::failed_precondition("no code to send"));
        }
        if session.resends >= MAX_RESENDS {
            return Err(Status::failed_precondition("no resends left"));
        }
        session.resends += 1;
        let template = session.text_template.clone();
        if let Stage::Unverified(Some(p)) = &mut session.stage {
            p.code = self.codes.issue();
            p.issued = std::time::Instant::now();
            self.codes.send(&p.phone, &p.code, template.as_deref()).await.map_err(down)?;
        }
        // The same reply whether or not a code went out.
        Ok(Response::new(pb::ResendCodeReply { resends_left: MAX_RESENDS - session.resends }))
    }

    async fn check_code(
        &self,
        req: Request<pb::CheckCodeRequest>,
    ) -> Result<Response<pb::CheckCodeReply>, Status> {
        let (handle, code) = edge::check_code(req.into_inner())?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let mut session = session.lock().await;
        let reply = |outcome: pb::CheckOutcome, tries_left| {
            Ok(Response::new(pb::CheckCodeReply { outcome: outcome.into(), tries_left }))
        };

        let right = match &session.stage {
            Stage::Verified(_) => return reply(pb::CheckOutcome::Pass, 0),
            Stage::Locked => return reply(pb::CheckOutcome::Locked, 0),
            Stage::Unverified(None) => None,
            Stage::Unverified(Some(p)) => {
                // Constant time: the reply's timing never says how many digits were right.
                (p.code.ct_eq(&code) && p.issued.elapsed() < CODE_LIFETIME).then(|| (p.patient.clone(), p.facts.clone(), p.pcp.clone()))
            }
        };
        if let Some((patient, facts, pcp)) = right {
            session.stage = Stage::Verified(patient);
            session.facts = facts;
            session.pcp = pcp;
            return reply(pb::CheckOutcome::Pass, 0);
        }
        session.code_failures += 1;
        if session.code_failures >= MAX_CODE_FAILURES {
            // Wipes the pending patient and code with the old stage.
            session.stage = Stage::Locked;
            return reply(pb::CheckOutcome::Locked, 0);
        }
        reply(pb::CheckOutcome::Fail, MAX_CODE_FAILURES - session.code_failures)
    }

    async fn find_slots(
        &self,
        req: Request<pb::FindSlotsRequest>,
    ) -> Result<Response<pb::FindSlotsReply>, Status> {
        let (handle, ask) = edge::find_slots(req.into_inner())?;
        let call = self.sessions.get(&handle).await.map_err(refused)?;
        let (patient, source, facts, h) = {
            let session = call.lock().await;
            (session.patient().map_err(refused)?, session.source, session.facts.clone(), Arc::clone(&session.hospital))
        };
        let sched = self.sched(source)?;
        let offered_at_once = h.slots_offered;
        let new_search = matches!(ask, edge::SlotAsk::New(..));

        // The EHR's work runs in its own task (`ledgered`), with the session unlocked, so its
        // ledger is kept even when the agent stops waiting.
        let (mut session, mut found) = match ask {
            // A new search. New tokens: a token from an older search no longer books. The
            // reason asked with these slots is the one Book sends.
            edge::SlotAsk::New(mut wish, ask) => {
                wish.who = facts;
                // Only a name on the call's own list: the model picks, the hospital decides
                // what it means.
                if !ask.time.is_empty() {
                    let t = h.time_of_day(&ask.time).ok_or_else(|| Status::invalid_argument("time_of_day: not on the hospital's list"))?;
                    wish.time_of_day = Some(t);
                }
                // The place and the kind of care, matched in Rust against the lists (§3.1). Words
                // that name nothing on them, or several places, are a question, and no search runs.
                let pick = h.pick(ask.place.as_ref(), ask.specialty.as_ref());
                let question = match pick.outcome {
                    PickOutcome::Search => None,
                    PickOutcome::WhichPlace => Some(pb::PlaceAsk::Which),
                    PickOutcome::UnknownPlace => Some(pb::PlaceAsk::UnknownPlace),
                    PickOutcome::UnknownSpecialty => Some(pb::PlaceAsk::UnknownSpecialty),
                    PickOutcome::NotAtPlace => Some(pb::PlaceAsk::NotAtPlace),
                };
                if let Some(q) = question {
                    return Ok(Response::new(place_question(&h, q, &pick.names)));
                }
                // The caller's seen visits in the hospital's window, read once a call, only when
                // they can matter: new or established (BOOKING-RULES.md §3.4), or a usual place
                // (§3.2: for primary care, the caller has a PCP or the hospital marks a primary
                // care specialty; for a specialty, their last visit at one of its departments).
                let pcp = call.lock().await.pcp.clone();
                let wants_usual = !ask.any_place
                    && !pick.place_named
                    && (pick.named_specialty || !pcp.is_empty() || h.marks_primary_care());
                let seen = if h.sorts_patients() || h.rules_sort_patients() || wants_usual {
                    self.seen(&call, &h, &patient, source).await?
                } else {
                    Vec::new()
                };
                wish.seen = seen.iter().map(|v| v.place.clone()).collect();
                // The usual place and the PCP (§3.2): for primary care, the last visit with a PCP
                // on file, else the last at a primary care department; for a specialty, the last
                // at one of its departments. A caller who asked for anywhere gets none.
                let usual = if wants_usual { usual_place(&h, &seen, &pcp, &pick) } else { None };
                let places = match &usual {
                    Some(u) => vec![u.0.place.clone()],
                    None => pick.departments.clone(),
                };
                // The Booking rules before any search (§3.5): when every place the search would
                // look at holds the visit back (Referral needed, Hand off) by its specialty,
                // department and new or established, nothing is searched and the rule answers.
                if let Some(rule) = held_back_everywhere(&h, &places, &wish.seen) {
                    return Ok(Response::new(pb::FindSlotsReply { rule: rule_ask(rule).into(), ..Default::default() }));
                }
                // No place named and no usual place, and the kind of care is at several places:
                // the caller is asked where (1B), naming them when one question can.
                if usual.is_none() && !ask.any_place && !pick.place_named && pick.departments.len() > 1 && pick.names.len() > 1 {
                    let q = if pick.names.len() <= h.groups_named { pb::PlaceAsk::Which } else { pb::PlaceAsk::Where };
                    return Ok(Response::new(place_question(&h, q, &pick.names)));
                }
                let place_words = usual.as_ref().map(|u| u.0.place_words.clone());
                if ask.plan_only {
                    // Where the search would look, and nothing searched.
                    return Ok(Response::new(pb::FindSlotsReply { place_default: edge::place_words(place_words.as_ref()), ..Default::default() }));
                }
                // The PCP's times come first, for primary care only.
                let search_pcp =
                    if pick.primary_care { usual.as_ref().and_then(|u| u.1.clone()).or_else(|| pcp.first().cloned()) } else { None };
                let strict = usual.is_some() || pick.strict;
                wish.place = (places.len() == 1).then(|| places[0].clone());
                wish.specialty = pick.specialty_code.clone();
                let hh = Arc::clone(&h);
                let (found, wish) = ledgered(&call, "FindSlots", async move {
                    let found = sched.find(&hh, &patient, &wish).await;
                    (found, wish)
                })
                .await?;
                let found = found.map_err(scheduler_down)?;
                let mut session = call.lock().await;
                session.note = Some(wish.note.clone());
                session.offered.clear();
                session.next_pages = found.next;
                let mut slots = found.slots;
                // $find has no clinician input: the clinician the caller named narrows here,
                // the same for every EHR (EPIC.md §7.1).
                if let Some(who) = &wish.clinician {
                    slots.retain(|s| who.names_clinician(&s.clinician));
                }
                // In person or online, as the caller asked: an EHR may return a type other than
                // the one searched (Epic's sandbox returns every time as a video visit; the
                // call of 2026-11-15, 22:36, offered five online times to an in-person ask).
                session.mode = wish.mode;
                slots.retain(|s| wish.mode.fits(s.about.online));
                keep_fitting(&h, &wish.seen, &mut slots);
                // The search's places only, whether or not the EHR took `location-reference` or
                // `specialties` (step 0: the sandbox ignores `specialties`).
                slots.retain(|s| h.keeps(&places, strict, &s.place_ref));
                // The Booking rules per time (§3.5): its visit type and provider are known now.
                let any = !slots.is_empty();
                session.flag_on_book.clear();
                let held = keep_by_rules(&h, &wish.seen, search_pcp.as_ref(), &mut slots, &mut session.flag_on_book);
                if any && slots.is_empty() {
                    if let Some(rule) = held {
                        session.search_places = places;
                        session.search_strict = strict;
                        session.rest.clear();
                        session.next_pages.clear();
                        return Ok(Response::new(pb::FindSlotsReply { rule: rule_ask(rule).into(), ..Default::default() }));
                    }
                }
                session.search_places = places;
                session.search_strict = strict;
                session.sooner = pcp_first(&h, search_pcp.as_ref(), &mut slots, offered_at_once);
                session.search_pcp = search_pcp;
                session.place_words = place_words;
                (session, slots)
            }
            // The next ones of the same search (the owner, 2026-11-14: keep Epic's page of 10):
            // the rest of this page, then the EHR's next page. The tokens already offered stay.
            edge::SlotAsk::More => {
                let mut session = call.lock().await;
                let rest = std::mem::take(&mut session.rest);
                if rest.is_empty() && !session.next_pages.is_empty() {
                    let next = session.next_pages.clone();
                    drop(session);
                    let hh = Arc::clone(&h);
                    let page = ledgered(&call, "FindSlots", async move { sched.find_more(&hh, &patient, &next).await })
                        .await?
                        .map_err(scheduler_down)?;
                    let mut session = call.lock().await;
                    session.next_pages = page.next;
                    let mut slots = page.slots;
                    let mode = session.mode;
                    slots.retain(|s| mode.fits(s.about.online));
                    let places: Vec<PlaceRef> = session.seen.iter().flatten().map(|v| v.place.clone()).collect();
                    keep_fitting(&h, &places, &mut slots);
                    slots.retain(|s| h.keeps(&session.search_places, session.search_strict, &s.place_ref));
                    let pcp = session.search_pcp.clone();
                    let mut flags = std::mem::take(&mut session.flag_on_book);
                    keep_by_rules(&h, &places, pcp.as_ref(), &mut slots, &mut flags);
                    session.flag_on_book = flags;
                    (session, slots)
                } else {
                    (session, rest)
                }
            }
        };
        let later = found.split_off(found.len().min(offered_at_once));
        session.rest = later;
        let mut slots = Vec::with_capacity(found.len());
        for slot in &found {
            let token = Token::random();
            // With the caller's PCP, and the one time offered as sooner with another clinician
            // (BOOKING-RULES.md §3.3).
            let with_pcp = session.search_pcp.as_ref().is_some_and(|p| slot.clinician_ref.same(p));
            let sooner = session.sooner.as_ref() == Some(&slot.id);
            slots.push(edge::slot(&token, slot, with_pcp, sooner));
            session.offered.insert(token, slot.id.clone());
        }
        let more = !session.rest.is_empty() || !session.next_pages.is_empty();
        // The usual place is named with a new search's times only (§3.2).
        let place_default = if new_search { edge::place_words(session.place_words.as_ref()) } else { String::new() };
        Ok(Response::new(pb::FindSlotsReply { slots, more, place_default, ..Default::default() }))
    }

    async fn book(
        &self,
        req: Request<pb::BookRequest>,
    ) -> Result<Response<pb::BookReply>, Status> {
        let (handle, slot_token, key) = edge::book(req.get_ref())?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let source = session.lock().await.source;
        let scheduler = self.sched(source)?;

        // A booking runs to its end in its own task, even when the caller interrupts or the
        // deadline passes and this request is dropped. The answer is kept under the key, so a
        // retry gets it instead of booking twice (EHR-SERVICE.md §2).
        let task = tokio::spawn(async move {
            let mut session = session.lock_owned().await;
            let patient = session.patient().map_err(refused)?;
            // Book and flag (BOOKING-RULES.md §3.5): the rule was checked for this time when it
            // was offered, with the call's own hospital, which no save changes.
            let check_referral = session.offered.get(&slot_token).is_some_and(|id| session.flag_on_book.contains(id));
            let result = match session.booked.get(&key) {
                Some((first_token, _)) if *first_token != slot_token => {
                    return Err(Status::failed_precondition(
                        "idempotency_key: used for another slot",
                    ));
                }
                Some((_, result)) => result.clone(),
                None => {
                    let slot_id = session.offered.get(&slot_token).cloned().ok_or_else(|| {
                        Status::failed_precondition("slot_token: not offered on this call")
                    })?;
                    // A token exists only after a FindSlots, which set the note.
                    let note = session
                        .note
                        .clone()
                        .ok_or_else(|| Status::failed_precondition("slot_token: not offered on this call"))?;
                    // In this task already, so the ledger is kept even if the agent stops waiting.
                    let h = Arc::clone(&session.hospital);
                    let (result, entries) = crate::ledger::kept(scheduler.book(&h, &patient, &slot_id, &note)).await;
                    session.ledger.extend(entries.into_iter().map(|e| ("Book", e)));
                    let result = result.map_err(scheduler_down)?;
                    // A lost reply is never the key's answer: the caller's yes retries it (V2A).
                    if !matches!(result, BookResult::NotThrough) {
                        session.booked.insert(key, (slot_token.clone(), result.clone()));
                    }
                    result
                }
            };
            Ok(match result {
                BookResult::Booked(slot) => pb::BookReply {
                    outcome: pb::BookOutcome::Booked.into(),
                    booked: Some(edge::slot(&slot_token, &slot, false, false)),
                    visit: Some(edge::visit(&slot)),
                    check_referral,
                },
                BookResult::Taken => pb::BookReply { outcome: pb::BookOutcome::SlotTaken.into(), ..Default::default() },
                BookResult::NotThrough => pb::BookReply { outcome: pb::BookOutcome::NotThrough.into(), ..Default::default() },
            })
        });
        let reply = task.await.map_err(|_| Status::internal("the booking stopped"))??;
        Ok(Response::new(reply))
    }

    async fn find_visit(
        &self,
        req: Request<pb::FindVisitRequest>,
    ) -> Result<Response<pb::FindVisitReply>, Status> {
        let (handle, question) = edge::find_visit(req.into_inner())?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let (patient, source, h) = {
            let session = session.lock().await;
            (session.patient().map_err(refused)?, session.source, Arc::clone(&session.hospital))
        };
        let sched = self.sched(source)?;
        // A place the caller named (BOOKING-RULES.md §3.1): its departments, matched in Rust. A
        // place on no list is said so, and no visit is looked for.
        // Every place the words name counts: the visit says which one it is at.
        let at = question.place.as_ref().filter(|w| w.names_anything()).map(|words| h.places_named(words));
        if at.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Response::new(pb::FindVisitReply { unknown_place: true, ..Default::default() }));
        }
        let by_day = question.ask.range.is_some();
        let ask = question.ask;
        let hh = Arc::clone(&h);
        let found = ledgered(&session, "FindVisit", async move { sched.visits(&hh, &patient, &ask).await }).await?;
        let FoundVisits { visits: mut found, within_days } = found.map_err(scheduler_down)?;
        if let Some(at) = &at {
            found.retain(|v| Hospital::among(at, &v.place_ref));
        }
        let groups = h.groups();
        let group = question.group.as_ref().and_then(|g| groups.named_by(g));
        let ask = Narrow {
            clinician: question.clinician.as_ref(),
            unknown_group: question.group.is_some() && group.is_none(),
            group,
            by_day,
        };
        let reply = match narrow(found, &ask, groups, h.limits()) {
            Found::None => pb::FindVisitReply::default(),
            Found::Visit { visit, more } => pb::FindVisitReply {
                visit: Some(edge::visit(&visit)),
                more,
                more_within_days: within_days.unwrap_or(0),
                ..Default::default()
            },
            Found::Which { groups, other } => {
                pb::FindVisitReply { groups: edge::groups(&groups), other_visits: other, ..Default::default() }
            }
        };
        Ok(Response::new(reply))
    }

    async fn place_info(&self, req: Request<pb::PlaceInfoRequest>) -> Result<Response<pb::PlaceInfoReply>, Status> {
        let (handle, place) = edge::place_info(req.into_inner())?;
        // An open call is enough: the lists hold no protected data (BOOKING-RULES.md §3.7).
        let call = self.sessions.get(&handle).await.map_err(refused)?;
        let h = Arc::clone(&call.lock().await.hospital);
        let today = jiff::Timestamp::now().to_zoned_datetime(h.tz().clone()).date().to_string();
        Ok(Response::new(match h.place_info(&place, &today) {
            PlaceAnswer::Unknown => pb::PlaceInfoReply { outcome: pb::PlaceInfoOutcome::Unknown.into(), ..Default::default() },
            PlaceAnswer::Which(names) => pb::PlaceInfoReply {
                outcome: pb::PlaceInfoOutcome::Which.into(),
                other_places: names.len() > h.groups_named,
                places: names.into_iter().take(h.groups_named).collect(),
                ..Default::default()
            },
            PlaceAnswer::Found(f) => pb::PlaceInfoReply {
                outcome: pb::PlaceInfoOutcome::Found.into(),
                name: f.name,
                address: f.address,
                phone: f.phone,
                hours: f.hours.into_iter().map(|r| pb::OpenHours { day: r.day, opens: r.opens, closes: r.closes }).collect(),
                closed_days: f.closed_days.into_iter().map(|d| pb::ClosedDay { date: d.date, name: d.name }).collect(),
                ..Default::default()
            },
        }))
    }

    async fn file_call(
        &self,
        req: Request<tonic::Streaming<pb::FileCallLine>>,
    ) -> Result<Response<pb::FileCallReply>, Status> {
        // The stream is checked first, so a bad request is refused the same way with or without
        // a store.
        let mut stream = req.into_inner();
        let mut id: Option<String> = None;
        // The record holds the call's words: wiped when it drops.
        let mut record = Zeroizing::new(Vec::new());
        let mut lines = 0usize;
        while let Some(message) = stream.message().await? {
            let (call_id, line) = edge::file_call_line(message)?;
            match &id {
                None => id = Some(call_id),
                Some(first) if *first != call_id => return Err(Status::invalid_argument("call_id: changed within one record")),
                Some(_) => {}
            }
            lines += 1;
            if lines > MAX_RECORD_LINES || record.len() + line.len() + 1 > MAX_RECORD_BYTES {
                return Err(Status::resource_exhausted("the record is too long"));
            }
            record.extend_from_slice(line.as_bytes());
            record.push(b'\n');
        }
        let id = id.ok_or_else(|| Status::invalid_argument("an empty record"))?;
        if store::origin_of(&id).is_none() {
            return Err(Status::invalid_argument("call_id: does not start with its origin"));
        }
        let class = store::record_class(&record).map_err(|_| Status::invalid_argument("the record's data_class"))?;
        let store = self.store.as_ref().ok_or_else(|| Status::failed_precondition("ehr runs without a store"))?;
        let store = store.class(class).ok_or_else(|| Status::failed_precondition("no real store on this ehr"))?;
        let filed = store.file_record(&id, &record).await.map_err(|_| Status::internal("the store could not file the call"))?;
        Ok(Response::new(pb::FileCallReply { filed }))
    }

    async fn file_test_run(&self, req: Request<pb::FileTestRunRequest>) -> Result<Response<pb::FileTestRunReply>, Status> {
        let meta = req.metadata().clone();
        // Checked first, so a bad run is refused the same way with or without a store.
        let run = edge::test_run(req.into_inner())?;
        self.gate("FileTestRun", &meta, None).await?;
        let store = self.settings_store()?;
        let filed = store.synthetic().file_test_run(&run).await.map_err(|_| Status::internal("the store could not file the run"))?;
        Ok(Response::new(pb::FileTestRunReply {
            run_id: filed.run_id,
            needed: filed.needed,
            pruned: i32::try_from(filed.pruned).unwrap_or(i32::MAX),
        }))
    }

    async fn add_annotation(&self, req: Request<pb::Annotation>) -> Result<Response<pb::AddAnnotationReply>, Status> {
        let meta = req.metadata().clone();
        let a = edge::annotation(req.into_inner())?;
        self.gate("AddAnnotation", &meta, None).await?;
        let store = self.settings_store()?;
        // The class of the call it names: a real call's note is sealed with real rows.
        let class = match store.class_of_call(&a.call_id).await.map_err(|_| Status::internal("the store could not find the call"))? {
            Some(class) => class,
            None if store::origin_of(&a.call_id) == Some("live") && store.class(store::DataClass::Real).is_none() => {
                return Err(Status::failed_precondition("no real store on this ehr"));
            }
            None => return Err(Status::not_found("call_id: no such call")),
        };
        let filed = class.add_annotation(&a).await.map_err(|_| Status::internal("the store could not file the annotation"))?;
        Ok(Response::new(pb::AddAnnotationReply { filed }))
    }

    async fn list_annotations(&self, req: Request<pb::ListAnnotationsRequest>) -> Result<Response<pb::ListAnnotationsReply>, Status> {
        self.gate("ListAnnotations", req.metadata(), None).await?;
        let store = self.settings_store()?;
        let found = store.synthetic().list_annotations().await.map_err(|_| Status::internal("the store could not read the annotations"))?;
        Ok(Response::new(pb::ListAnnotationsReply { annotations: found.into_iter().map(edge::annotation_out).collect() }))
    }

    async fn list_setting_versions(
        &self,
        req: Request<pb::ListSettingVersionsRequest>,
    ) -> Result<Response<pb::ListSettingVersionsReply>, Status> {
        let (name, _, _) = setting_service(&req.get_ref().service)?;
        self.gate("ListSettingVersions", req.metadata(), Some(name)).await?;
        let store = self.settings_store()?;
        let versions = settings::versions(store.pool(), name).await.map_err(|_| Status::internal("the store could not list the settings"))?;
        Ok(Response::new(pb::ListSettingVersionsReply { versions: versions.into_iter().map(setting_version).collect() }))
    }

    async fn get_settings(&self, req: Request<pb::GetSettingsRequest>) -> Result<Response<pb::GetSettingsReply>, Status> {
        let (name, _, table) = setting_service(&req.get_ref().service)?;
        let id = req.get_ref().version_id;
        if id < 0 {
            return Err(Status::invalid_argument("version_id: not a version"));
        }
        self.gate("GetSettings", req.metadata(), Some(name)).await?;
        let store = self.settings_store()?;
        let found = settings::get(store.pool(), name, table, id).await.map_err(|_| Status::internal("the store could not read the settings"))?;
        let reply = match found {
            None if id != 0 => return Err(Status::not_found("no such version")),
            None => pb::GetSettingsReply { version: None, values: vec![] },
            Some((v, values)) => pb::GetSettingsReply {
                version: Some(setting_version(v)),
                values: values.into_iter().map(|(key, value)| pb::SettingValue { key, value }).collect(),
            },
        };
        Ok(Response::new(reply))
    }

    async fn save_settings(&self, req: Request<pb::SaveSettingsRequest>) -> Result<Response<pb::SaveSettingsReply>, Status> {
        let meta = req.metadata().clone();
        let req = req.into_inner();
        let (name, list, table) = setting_service(&req.service)?;
        let by = req.by.trim();
        if by.is_empty() || by.len() > 64 {
            return Err(Status::invalid_argument("by: who saves, 1 to 64 characters"));
        }
        let given: Vec<(String, String)> = req.values.into_iter().map(|v| (v.key, v.value)).collect();
        // Checked before the store, so a bad set is refused the same way with or without one.
        let values = settings::check(list, &given).map_err(Status::invalid_argument)?;
        settings::coherent(name, &values).map_err(Status::invalid_argument)?;
        let who = self.gate("SaveSettings", &meta, Some(name)).await?;
        let store = self.settings_store()?;
        // The words and the System variables agree (SYSTEM-PAGE.md §3.3): a section holds only
        // its own code values and System variables of the active System version; a System save
        // keeps every variable the active words use. A Default seed is the files, which the
        // agent checked together at start-up.
        if !req.seed {
            match name {
                "prompt" => {
                    let names = self.system_variables(0).await?.into_keys().collect();
                    settings::prompt_variables(&values, &names).map_err(Status::invalid_argument)?;
                }
                "system" => {
                    let names: BTreeSet<String> = values
                        .iter()
                        .find(|(k, _)| *k == "variables")
                        .map(|(_, v)| settings::variables(v))
                        .transpose()
                        .map_err(Status::invalid_argument)?
                        .unwrap_or_default()
                        .into_keys()
                        .collect();
                    let (_, _, prompt_table) = setting_service("prompt")?;
                    let prompt = settings::get(store.pool(), "prompt", prompt_table, 0)
                        .await
                        .map_err(|_| Status::internal("the store could not read the prompt"))?;
                    if let Some((_, words)) = prompt {
                        let words: Vec<(&str, serde_json::Value)> = words
                            .iter()
                            .filter_map(|(k, v)| Some((k.as_str(), serde_json::from_str(v).ok()?)))
                            .collect();
                        settings::prompt_variables(&words, &names).map_err(|e| {
                            // "line.x: {name} is not a System variable" → which variable, and where.
                            let (section, rest) = e.split_once(": ").unwrap_or(("a section", &e));
                            let var = rest.split_whitespace().next().unwrap_or("a variable");
                            Status::invalid_argument(format!("variables: {var} is still used in {section}"))
                        })?;
                    }
                }
                _ => {}
            }
        }
        let v = settings::save(store.pool(), name, table, &values, by, req.seed, who.as_ref()).await.map_err(|e| match e {
            settings::SaveError::DefaultExists => Status::already_exists("the Default version exists"),
            settings::SaveError::Refused(why) => Status::failed_precondition(why),
            settings::SaveError::Db(_) => Status::internal("the store could not save the settings"),
        })?;
        Ok(Response::new(pb::SaveSettingsReply { version: Some(setting_version(v)) }))
    }

    async fn list_users(&self, req: Request<pb::ListUsersRequest>) -> Result<Response<pb::ListUsersReply>, Status> {
        self.gate("ListUsers", req.metadata(), None).await?;
        let store = self.settings_store()?;
        let people = users::list(store).await.map_err(users_status)?;
        let users = people
            .into_iter()
            .map(|u| pb::User {
                id: u.id,
                email: u.email.to_string(),
                name: u.name.to_string(),
                owner: u.owner,
                added_by: u.added_by.to_string(),
                added_at: u.added_at,
                last_seen_at: u.last_seen_at.unwrap_or_default(),
                first_name: u.first_name.to_string(),
                last_name: u.last_name.to_string(),
                phone: u.phone.to_string(),
                phone_verified: u.phone_verified,
                source: u.source,
                photo_at: u.photo_at.unwrap_or_default(),
                role_ids: u.role_ids,
            })
            .collect();
        Ok(Response::new(pb::ListUsersReply { users }))
    }

    async fn add_user(&self, req: Request<pb::AddUserRequest>) -> Result<Response<pb::AddUserReply>, Status> {
        let meta = req.metadata().clone();
        let req = req.into_inner();
        // Checked before the store, so a bad request is refused the same way with or without one.
        users::normal_new(&req.email, &req.name).map_err(users_status)?;
        let who = self.gate("AddUser", &meta, None).await?;
        let store = self.settings_store()?;
        let id = users::add(store, &req.email, &req.name, who.as_ref()).await.map_err(users_status)?;
        Ok(Response::new(pb::AddUserReply { id }))
    }

    async fn remove_user(&self, req: Request<pb::RemoveUserRequest>) -> Result<Response<pb::RemoveUserReply>, Status> {
        let who = self.gate("RemoveUser", req.metadata(), None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        let id = &req.get_ref().id;
        self.may_change_person(&who, "RemoveUser", id, true).await?;
        let store = self.settings_store()?;
        let email = users::remove(store, id, Some(&who)).await.map_err(users_status)?;
        Ok(Response::new(pb::RemoveUserReply { email: email.to_string() }))
    }

    async fn admit_user(&self, req: Request<pb::AdmitUserRequest>) -> Result<Response<pb::AdmitUserReply>, Status> {
        let store = self.settings_store()?;
        let id = users::admit(store, &req.get_ref().email).await.map_err(users_status)?;
        Ok(Response::new(pb::AdmitUserReply { active: id.is_some(), user_id: id.unwrap_or_default() }))
    }

    async fn seed_users(&self, req: Request<pb::SeedUsersRequest>) -> Result<Response<pb::SeedUsersReply>, Status> {
        if req.get_ref().emails.len() > 500 {
            return Err(Status::invalid_argument("emails: at most 500"));
        }
        let store = self.settings_store()?;
        let emails: Vec<String> = req.into_inner().emails.into_iter().map(|e| e.email).collect();
        let added = users::seed(store, &emails).await.map_err(users_status)?;
        Ok(Response::new(pb::SeedUsersReply { added: added as i32 }))
    }

    async fn update_user(&self, req: Request<pb::UpdateUserRequest>) -> Result<Response<pb::UpdateUserReply>, Status> {
        let who = self.gate("UpdateUser", req.metadata(), None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        let req = req.into_inner();
        self.may_change_person(&who, "UpdateUser", &req.id, false).await?;
        let store = self.settings_store()?;
        let profile = users::Profile {
            first_name: &req.first_name,
            last_name: &req.last_name,
            phone: &req.phone,
            email: &req.email,
        };
        let saved = users::update(store, &req.id, &profile, Some(&who)).await.map_err(users_status)?;
        let old_email = if saved.email_changed { saved.old_email.to_string() } else { String::new() };
        Ok(Response::new(pb::UpdateUserReply { old_email, email_changed: saved.email_changed }))
    }

    async fn get_user_photo(&self, req: Request<pb::GetUserPhotoRequest>) -> Result<Response<pb::UserPhoto>, Status> {
        self.gate("GetUserPhoto", req.metadata(), None).await?;
        let store = self.settings_store()?;
        let bytes = users::photo(store, &req.get_ref().id).await.map_err(users_status)?;
        let photo = bytes.map(|b| B64.encode(b.as_slice())).unwrap_or_default();
        Ok(Response::new(pb::UserPhoto { photo }))
    }

    async fn set_user_photo(&self, req: Request<pb::SetUserPhotoRequest>) -> Result<Response<pb::SetUserPhotoReply>, Status> {
        let meta = req.metadata().clone();
        let req = req.into_inner();
        // Base64 of 256 KB is about 350 KB: anything far longer is refused before it is decoded.
        if req.photo.len() > users::MAX_PHOTO * 4 / 3 + 8 {
            return Err(Status::invalid_argument("photo: at most 256 KB"));
        }
        let who = self.gate("SetUserPhoto", &meta, None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        self.may_change_person(&who, "SetUserPhoto", &req.id, false).await?;
        let store = self.settings_store()?;
        if req.photo.is_empty() {
            users::clear_photo(store, &req.id, Some(&who)).await.map_err(users_status)?;
        } else {
            let bytes = Zeroizing::new(
                B64.decode(req.photo.as_bytes()).map_err(|_| Status::invalid_argument("photo: not base64"))?,
            );
            users::set_photo(store, &req.id, &bytes, Some(&who)).await.map_err(users_status)?;
        }
        Ok(Response::new(pb::SetUserPhotoReply {}))
    }

    async fn seen_owner(&self, _: Request<pb::SeenOwnerRequest>) -> Result<Response<pb::SeenOwnerReply>, Status> {
        let store = self.settings_store()?;
        let id = users::owner_id(store).await.map_err(users_status)?;
        if let Some(id) = &id {
            users::seen(store, id).await.map_err(users_status)?;
        }
        Ok(Response::new(pb::SeenOwnerReply { user_id: id.unwrap_or_default() }))
    }

    async fn levels(&self, req: Request<pb::LevelsRequest>) -> Result<Response<pb::LevelsReply>, Status> {
        let who = self.gate("Levels", req.metadata(), None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        let levels = who.levels.words().into_iter().map(|(lever, level)| pb::LeverLevel { lever: lever.into(), level: level.into() }).collect();
        Ok(Response::new(pb::LevelsReply { user_id: who.id, role_ids: who.role_ids, roles_version: who.roles_version, levels }))
    }

    async fn grant_role(&self, req: Request<pb::GrantRoleRequest>) -> Result<Response<pb::GrantRoleReply>, Status> {
        let who = self.gate("GrantRole", req.metadata(), None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        let r = req.get_ref();
        self.change_role(&who, &r.user_id, &r.role_id, true).await?;
        Ok(Response::new(pb::GrantRoleReply {}))
    }

    async fn revoke_role(&self, req: Request<pb::RevokeRoleRequest>) -> Result<Response<pb::RevokeRoleReply>, Status> {
        let who = self.gate("RevokeRole", req.metadata(), None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        let r = req.get_ref();
        self.change_role(&who, &r.user_id, &r.role_id, false).await?;
        Ok(Response::new(pb::RevokeRoleReply {}))
    }

    async fn read_audit(&self, req: Request<pb::ReadAuditRequest>) -> Result<Response<pb::ReadAuditReply>, Status> {
        self.gate("ReadAudit", req.metadata(), None).await?;
        let r = req.get_ref();
        let store = self.settings_store()?;
        let rows = audit::read(store.pool(), r.before, i64::from(r.limit), r.changes)
            .await
            .map_err(|_| Status::internal("the store could not read the audit trail"))?;
        let rows = rows
            .into_iter()
            .map(|s| pb::AuditRow {
                seq: s.seq,
                at: s.at,
                actor: s.actor.unwrap_or_default(),
                source: s.source,
                roles: s.roles,
                roles_version: s.roles_version.unwrap_or_default(),
                lever: s.lever.unwrap_or_default(),
                level: s.level.unwrap_or_default(),
                action: s.action,
                object_kind: s.object_kind.unwrap_or_default(),
                object_id: s.object_id.unwrap_or_default(),
                data_class: s.data_class.unwrap_or_default(),
                fields: s.fields,
                before_version: s.before_version.unwrap_or_default(),
                after_version: s.after_version.unwrap_or_default(),
                allowed: s.allowed,
            })
            .collect();
        Ok(Response::new(pb::ReadAuditReply { rows }))
    }

    async fn verify_audit(&self, req: Request<pb::VerifyAuditRequest>) -> Result<Response<pb::VerifyAuditReply>, Status> {
        self.gate("VerifyAudit", req.metadata(), None).await?;
        let store = self.settings_store()?;
        let reply = match audit::verify(store.pool()).await {
            Ok(rows) => pb::VerifyAuditReply { ok: true, rows: i64::try_from(rows).unwrap_or(i64::MAX), first_bad: 0 },
            Err(-1) => return Err(Status::internal("the store could not read the audit trail")),
            Err(seq) => pb::VerifyAuditReply { ok: false, rows: 0, first_bad: seq },
        };
        Ok(Response::new(reply))
    }

    async fn log_access(&self, req: Request<pb::LogAccessRequest>) -> Result<Response<pb::LogAccessReply>, Status> {
        let who = self.gate("LogAccess", req.metadata(), None).await?.ok_or_else(|| Status::permission_denied("no person named"))?;
        let r = req.into_inner();
        let lever = Lever::from_word(&r.lever).ok_or_else(|| Status::invalid_argument("lever: not a lever"))?;
        let level = Level::from_word(&r.level).ok_or_else(|| Status::invalid_argument("level: none, view or edit"))?;
        let action_ok = !r.action.is_empty()
            && r.action.len() <= 64
            && r.action.starts_with(|c: char| c.is_ascii_lowercase())
            && r.action.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.');
        if !action_ok {
            return Err(Status::invalid_argument("action: a lower-case word with dots"));
        }
        // The kinds a look may name: a fixed list, so no request text becomes a kind.
        let kind: Option<&'static str> = match r.object_kind.as_str() {
            "" => None,
            "page" => Some("page"),
            "call" => Some("call"),
            "recording" => Some("recording"),
            "test_run" => Some("test_run"),
            "export" => Some("export"),
            // A dev-server route its wall refused; the id is the route's pattern, never a path
            // a request filled in (`/agent/calls/{call_id}`).
            "route" => Some("route"),
            _ => return Err(Status::invalid_argument("object_kind: page, call, recording, test_run, export or route")),
        };
        let braces_ok = kind == Some("route");
        let id_ok = r.object_id.len() <= 64
            && r.object_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/') || (braces_ok && matches!(c, '{' | '}')));
        if !id_ok {
            return Err(Status::invalid_argument("object_id: an id of at most 64 characters"));
        }
        let data_class: Option<&'static str> = match r.data_class.as_str() {
            "" => None,
            "synthetic" => Some("synthetic"),
            "real" => Some("real"),
            _ => return Err(Status::invalid_argument("data_class: synthetic or real")),
        };
        let allowed = who.levels.may(lever, level);
        let mut entry = Entry::by(Some(&who), &r.action).lever(lever, level);
        entry.object_kind = kind;
        entry.object_id = (!r.object_id.is_empty()).then_some(r.object_id);
        entry.data_class = data_class;
        entry.allowed = allowed;
        let store = self.settings_store()?;
        audit::write_alone(store.pool(), &entry).await.map_err(|_| Status::internal("the store could not write the audit row"))?;
        Ok(Response::new(pb::LogAccessReply { allowed }))
    }

    async fn import_places(&self, req: Request<pb::ImportPlacesRequest>) -> Result<Response<pb::ImportPlacesReply>, Status> {
        self.gate("ImportPlaces", req.metadata(), None).await?;
        let given = req.into_inner().addresses;
        let searches: Option<Vec<PlaceSearch>> = given.iter().map(|a| PlaceSearch::parse(a)).collect();
        let searches = searches.filter(|s| (1..=5).contains(&s.len())).ok_or_else(|| {
            Status::invalid_argument("addresses: 1 to 5, each a state (WI) or a city and its state (Verona, WI)")
        })?;
        // Epic's sandbox is the one directory ehr serves today; each EHR's adapter gets its own.
        let epic = self.epic()?;
        let places = epic.places(&searches).await.map_err(|e| {
            if e.contains("more than") {
                Status::resource_exhausted("Epic's directory returned more than an import holds")
            } else {
                Status::unavailable("Epic's directory search failed")
            }
        })?;
        let json = |v: serde_json::Value| v.to_string();
        let values = [
            ("facilities", json(serde_json::json!(places.facilities))),
            ("locations", json(serde_json::json!(places.locations))),
            ("specialty_list", json(serde_json::json!(places.specialty_list))),
            ("specialties", json(serde_json::json!(places.specialties))),
        ];
        let count = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
        Ok(Response::new(pb::ImportPlacesReply {
            values: values.into_iter().map(|(key, value)| pb::SettingValue { key: key.to_string(), value }).collect(),
            others: count(places.others),
            other_specialty_lists: count(places.other_specialty_lists),
            reads: count(places.reads),
        }))
    }
}

/// The service a settings request names, with its list and table (settings.rs).
/// The People list's errors (users.rs) as statuses: fixed words, never an email or a name.
fn users_status(e: users::UsersError) -> Status {
    match e {
        users::UsersError::Bad(why) => Status::invalid_argument(why),
        users::UsersError::Exists => Status::already_exists("that email is already on the list"),
        users::UsersError::NotFound => Status::not_found("no such person on the list"),
        users::UsersError::Taken => Status::already_exists("that email is already a removed person's: add them back instead"),
        users::UsersError::Store(_) => Status::internal("the store could not read or write the list"),
    }
}

fn setting_service(name: &str) -> Result<(&'static str, &'static [settings::Setting], &'static str), Status> {
    settings::service(name).ok_or_else(|| Status::invalid_argument("service: not a settings service"))
}

fn setting_version(v: settings::Version) -> pb::SettingVersion {
    pb::SettingVersion {
        id: v.id,
        day: v.day,
        seq: v.seq,
        created_at: v.created_at,
        created_by: v.created_by,
        active: v.active,
    }
}

#[cfg(test)]
mod tests {
    use tonic::Code;

    use super::*;
    use crate::{
        scheduler::FakeScheduler,
        verify::{FakeCodes, FakePatients, FAKE_CODE},
    };

    type Svc = EhrService<FakeScheduler, FakePatients, FakeCodes>;

    fn svc() -> Svc {
        EhrService::new(FakeScheduler::default(), FakePatients, FakeCodes)
    }

    async fn open(s: &Svc) -> String {
        s.open_call(Request::new(pb::OpenCallRequest::default())).await.unwrap().into_inner().handle
    }

    async fn identify(s: &Svc, h: &str, given: &str, family: &str, dob: &str) -> pb::IdentifyReply {
        s.identify(Request::new(pb::IdentifyRequest {
            handle: h.into(),
            given_name: given.into(),
            family_name: family.into(),
            birth_date: dob.into(),
        }))
        .await
        .unwrap()
        .into_inner()
    }

    async fn check(s: &Svc, h: &str, code: &str) -> pb::CheckCodeReply {
        s.check_code(Request::new(pb::CheckCodeRequest { handle: h.into(), code: code.into() }))
            .await
            .unwrap()
            .into_inner()
    }

    /// The real path: a fake patient's name and date of birth, then the fake code.
    async fn verified(s: &Svc) -> String {
        let h = open(s).await;
        identify(s, &h, "Jordan", "Ellis", "1984-03-12").await;
        assert_eq!(check(s, &h, FAKE_CODE).await.outcome, pb::CheckOutcome::Pass as i32);
        h
    }

    fn find_req(handle: &str, day: &str) -> Request<pb::FindSlotsRequest> {
        find_req_with(handle, day, "a check-up", pb::VisitMode::Unspecified)
    }

    fn find_req_with(handle: &str, day: &str, reason: &str, mode: pb::VisitMode) -> Request<pb::FindSlotsRequest> {
        Request::new(pb::FindSlotsRequest {
            handle: handle.into(),
            reason: reason.into(),
            earliest_day: day.into(),
            visit_mode: mode.into(),
            ..Default::default()
        })
    }

    fn book_req(handle: &str, token: &str, key: &str) -> Request<pb::BookRequest> {
        Request::new(pb::BookRequest {
            handle: handle.into(),
            slot_token: token.into(),
            idempotency_key: key.into(),
        })
    }

    async fn find(s: &Svc, handle: &str, day: &str) -> Vec<pb::Slot> {
        s.find_slots(find_req(handle, day)).await.unwrap().into_inner().slots
    }

    // ---- verification ----

    #[tokio::test]
    async fn a_match_and_the_right_code_verify() {
        let s = svc();
        let h = verified(&s).await;
        assert!(!find(&s, &h, "").await.is_empty());
    }

    #[tokio::test]
    async fn a_match_and_a_mismatch_get_the_same_identify_reply() {
        let s = svc();
        let (a, b) = (open(&s).await, open(&s).await);
        let hit = identify(&s, &a, "Jordan", "Ellis", "1984-03-12").await;
        let miss = identify(&s, &b, "Nobody", "Here", "2000-01-01").await;
        assert_eq!(hit, miss);
    }

    #[tokio::test]
    async fn without_a_match_even_the_fake_code_fails() {
        let s = svc();
        let h = open(&s).await;
        identify(&s, &h, "Nobody", "Here", "2000-01-01").await;
        assert_eq!(check(&s, &h, FAKE_CODE).await.outcome, pb::CheckOutcome::Fail as i32);
    }

    #[tokio::test]
    async fn two_close_names_on_one_date_never_verify() {
        let s = svc();
        let h = open(&s).await;
        identify(&s, &h, "Casey", "Morgan", "1990-06-05").await;
        assert_eq!(check(&s, &h, FAKE_CODE).await.outcome, pb::CheckOutcome::Fail as i32);
    }

    #[tokio::test]
    async fn three_wrong_codes_lock_the_chart_path() {
        let s = svc();
        let h = open(&s).await;
        identify(&s, &h, "Jordan", "Ellis", "1984-03-12").await;
        assert_eq!(check(&s, &h, "000000").await.tries_left, 2);
        assert_eq!(check(&s, &h, "000000").await.tries_left, 1);
        assert_eq!(check(&s, &h, "000000").await.outcome, pb::CheckOutcome::Locked as i32);
        // Locked stays locked, even with the right code.
        assert_eq!(check(&s, &h, FAKE_CODE).await.outcome, pb::CheckOutcome::Locked as i32);
        let err = s.find_slots(find_req(&h, "")).await.unwrap_err();
        assert_eq!(err.code(), Code::PermissionDenied);
    }

    #[tokio::test]
    async fn identify_has_three_tries() {
        let s = svc();
        let h = open(&s).await;
        for left in [2, 1, 0] {
            assert_eq!(identify(&s, &h, "Nobody", "Here", "2000-01-01").await.tries_left, left);
        }
        let err = s
            .identify(Request::new(pb::IdentifyRequest {
                handle: h,
                given_name: "Jordan".into(),
                family_name: "Ellis".into(),
                birth_date: "1984-03-12".into(),
            }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn a_resend_looks_the_same_with_or_without_a_match() {
        let s = svc();
        let (a, b) = (open(&s).await, open(&s).await);
        identify(&s, &a, "Jordan", "Ellis", "1984-03-12").await;
        identify(&s, &b, "Nobody", "Here", "2000-01-01").await;
        let resend = |h: String| s.resend_code(Request::new(pb::ResendCodeRequest { handle: h }));
        let (ra, rb) = (resend(a.clone()).await.unwrap(), resend(b).await.unwrap());
        assert_eq!(ra.into_inner(), rb.into_inner());
        assert_eq!(check(&s, &a, FAKE_CODE).await.outcome, pb::CheckOutcome::Pass as i32);
    }

    // ---- the gate ----

    #[tokio::test]
    async fn no_slots_before_verification() {
        let s = svc();
        let h = open(&s).await;
        let err = s.find_slots(find_req(&h, "")).await.unwrap_err();
        assert_eq!(err.code(), Code::PermissionDenied);
        // Identified but no code yet: still refused.
        identify(&s, &h, "Jordan", "Ellis", "1984-03-12").await;
        let err = s.find_slots(find_req(&h, "")).await.unwrap_err();
        assert_eq!(err.code(), Code::PermissionDenied);
    }

    #[tokio::test]
    async fn no_booking_before_verification() {
        let s = svc();
        let h = open(&s).await;
        let err = s
            .book(book_req(&h, &Token::random().to_hex(), "key-0000000000000001"))
            .await
            .unwrap_err();
        assert_eq!(err.code(), Code::PermissionDenied);
    }

    #[tokio::test]
    async fn a_wrong_handle_and_a_closed_call_look_the_same() {
        let s = svc();
        let h = verified(&s).await;
        s.close_call(Request::new(pb::CloseCallRequest { handle: h.clone() })).await.unwrap();
        s.close_call(Request::new(pb::CloseCallRequest { handle: h.clone() })).await.unwrap();
        let closed = s.find_slots(find_req(&h, "")).await.unwrap_err();
        let wrong = s.find_slots(find_req(&Token::random().to_hex(), "")).await.unwrap_err();
        assert_eq!(closed.code(), Code::Unauthenticated);
        assert_eq!((closed.code(), closed.message()), (wrong.code(), wrong.message()));
    }

    // ---- slots and booking ----

    #[tokio::test]
    async fn find_offers_at_most_three_from_the_day_asked() {
        let s = svc();
        let h = verified(&s).await;
        let slots = find(&s, &h, "").await;
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].starts_at, "2026-11-02T09:00");
        let later = find(&s, &h, "2026-11-04").await;
        assert!(later.iter().all(|x| x.starts_at.as_str() >= "2026-11-04"));
    }

    #[tokio::test]
    async fn a_retry_with_the_same_key_gets_the_same_answer() {
        let s = svc();
        let h = verified(&s).await;
        let slot = find(&s, &h, "").await.remove(0);
        let key = "key-0000000000000001";
        let first = s.book(book_req(&h, &slot.token, key)).await.unwrap().into_inner();
        let again = s.book(book_req(&h, &slot.token, key)).await.unwrap().into_inner();
        assert_eq!(first.outcome, pb::BookOutcome::Booked as i32);
        assert_eq!(first, again);
    }

    #[tokio::test]
    async fn a_key_cannot_be_reused_for_another_slot() {
        let s = svc();
        let h = verified(&s).await;
        let slots = find(&s, &h, "").await;
        let key = "key-0000000000000001";
        s.book(book_req(&h, &slots[0].token, key)).await.unwrap();
        let err = s.book(book_req(&h, &slots[1].token, key)).await.unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn a_slot_booked_on_one_call_is_taken_for_another() {
        let s = svc();
        let (a, b) = (verified(&s).await, verified(&s).await);
        let slot_a = find(&s, &a, "").await.remove(0);
        let slot_b = find(&s, &b, "").await.remove(0);
        s.book(book_req(&a, &slot_a.token, "key-aaaaaaaaaaaaaaaa")).await.unwrap();
        let r = s.book(book_req(&b, &slot_b.token, "key-bbbbbbbbbbbbbbbb")).await.unwrap();
        assert_eq!(r.into_inner().outcome, pb::BookOutcome::SlotTaken as i32);
    }

    #[tokio::test]
    async fn a_slot_token_works_only_on_its_own_call() {
        let s = svc();
        let (a, b) = (verified(&s).await, verified(&s).await);
        let slot_a = find(&s, &a, "").await.remove(0);
        let err = s.book(book_req(&b, &slot_a.token, "key-0000000000000001")).await.unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn an_old_token_stops_working_after_a_new_find() {
        let s = svc();
        let h = verified(&s).await;
        let old = find(&s, &h, "").await.remove(0);
        find(&s, &h, "").await;
        let err = s.book(book_req(&h, &old.token, "key-0000000000000001")).await.unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
    }

    // ---- the caller's visits ----

    async fn next(s: &Svc, h: &str) -> Result<Option<pb::Visit>, Status> {
        ask_visit(s, pb::FindVisitRequest { handle: h.into(), ..Default::default() }).await.map(|r| r.visit)
    }

    async fn ask_visit(s: &Svc, req: pb::FindVisitRequest) -> Result<pb::FindVisitReply, Status> {
        s.find_visit(Request::new(req)).await.map(|r| r.into_inner())
    }

    #[tokio::test]
    async fn no_next_visit_before_verification() {
        let s = svc();
        let h = open(&s).await;
        assert_eq!(next(&s, &h).await.unwrap_err().code(), Code::PermissionDenied);
    }

    #[tokio::test]
    async fn the_next_visit_is_the_soonest_including_a_new_booking() {
        let s = svc();
        let h = verified(&s).await;
        assert_eq!(next(&s, &h).await.unwrap().unwrap().starts_at, "2026-11-10T10:00");
        let slot = find(&s, &h, "").await.remove(0);
        s.book(book_req(&h, &slot.token, "key-0000000000000001")).await.unwrap();
        assert_eq!(next(&s, &h).await.unwrap().unwrap().starts_at, "2026-11-02T09:00");
    }

    #[tokio::test]
    async fn more_offers_the_rest_and_keeps_the_old_tokens() {
        let s = svc();
        let h = verified(&s).await;
        let first = s.find_slots(find_req(&h, "")).await.unwrap().into_inner();
        assert_eq!(first.slots.len(), 3);
        assert!(first.more, "six fake slots: three wait");
        let more = pb::FindSlotsRequest { handle: h.clone(), more: true, ..Default::default() };
        let next = s.find_slots(Request::new(more.clone())).await.unwrap().into_inner();
        assert_eq!(next.slots.len(), 3);
        assert!(!next.more);
        assert_ne!(first.slots[0].starts_at, next.slots[0].starts_at);
        // A slot offered first still books after More.
        let r = s.book(book_req(&h, &first.slots[0].token, "key-0000000000000001")).await.unwrap();
        assert_eq!(r.into_inner().outcome, pb::BookOutcome::Booked as i32);
        let none = s.find_slots(Request::new(more)).await.unwrap().into_inner();
        assert!(none.slots.is_empty() && !none.more);
    }

    #[tokio::test]
    async fn a_time_of_day_days_a_last_day_and_a_clinician_narrow_the_slots() {
        let s = svc();
        let h = verified(&s).await;
        let ask = |f: fn(&mut pb::FindSlotsRequest)| {
            let mut r = pb::FindSlotsRequest { handle: h.clone(), reason: "a check-up".into(), ..Default::default() };
            f(&mut r);
            r
        };
        let got = |r: pb::FindSlotsReply| r.slots.iter().map(|x| x.starts_at.clone()).collect::<Vec<_>>();
        let afternoons = s.find_slots(Request::new(ask(|r| r.time_of_day = "afternoon".into()))).await.unwrap().into_inner();
        assert_eq!(got(afternoons), ["2026-11-02T14:30", "2026-11-06T15:00"]);
        let tuesdays = s.find_slots(Request::new(ask(|r| r.days = vec!["tue".into()]))).await.unwrap().into_inner();
        assert_eq!(got(tuesdays), ["2026-11-03T10:00"]);
        let by_tuesday = s.find_slots(Request::new(ask(|r| r.latest_day = "2026-11-03".into()))).await.unwrap().into_inner();
        assert_eq!(got(by_tuesday), ["2026-11-02T09:00", "2026-11-02T14:30", "2026-11-03T10:00"]);
        let okafor = s.find_slots(Request::new(ask(|r| r.clinician = "Dr. Okafor".into()))).await.unwrap().into_inner();
        assert_eq!(got(okafor), ["2026-11-03T10:00", "2026-11-05T11:00"]);
    }

    fn visit_req(h: &str, when: &str, clinician: &str, past: bool, group: &str) -> pb::FindVisitRequest {
        pb::FindVisitRequest { handle: h.into(), when: when.into(), clinician: clinician.into(), past, group: group.into(), ..Default::default() }
    }

    #[tokio::test]
    async fn the_next_visit_gives_its_instructions_csn_and_a_count() {
        let s = svc();
        let h = verified(&s).await;
        let r = ask_visit(&s, visit_req(&h, "", "", false, "")).await.unwrap();
        let v = r.visit.unwrap();
        assert_eq!(v.instructions, "Bring a list of the medicines you take.");
        assert_eq!(v.csn, "90000101");
        assert_eq!(v.status, pb::VisitStatus::Upcoming as i32);
        assert_eq!(r.more, 2, "the X-ray and the December visit");
        assert!(r.groups.is_empty(), "no question for the next visit");
    }

    #[tokio::test]
    async fn a_past_visit_is_the_newest_with_its_status() {
        let s = svc();
        let h = verified(&s).await;
        let v = ask_visit(&s, visit_req(&h, "", "", true, "")).await.unwrap().visit.unwrap();
        assert_eq!((v.starts_at.as_str(), v.status), ("2026-10-20T14:00", pb::VisitStatus::Missed as i32));
        let v = ask_visit(&s, visit_req(&h, "2026-10-06", "", false, "")).await.unwrap().visit.unwrap();
        assert_eq!(v.status, pb::VisitStatus::Seen as i32);
    }

    #[tokio::test]
    async fn a_month_with_several_visits_asks_which_group_and_a_group_answers_it() {
        let s = svc();
        let h = verified(&s).await;
        // November holds two visits: no question. Dr. Lin has two, one in each group: the group
        // picks one. A group the hospital does not have asks which.
        let r = ask_visit(&s, visit_req(&h, "2026-11", "", false, "")).await.unwrap();
        assert!(r.visit.is_some() && r.more == 1, "two visits: no question");
        let r = ask_visit(&s, visit_req(&h, "", "Dr. Lin", false, "my scan")).await.unwrap();
        assert_eq!(r.visit.unwrap().starts_at, "2026-11-20T08:15");
        let r = ask_visit(&s, visit_req(&h, "", "Dr. Lin", false, "")).await.unwrap();
        assert_eq!(r.visit.unwrap().starts_at, "2026-11-20T08:15", "two: the soonest and a count");
        let r = ask_visit(&s, visit_req(&h, "", "", false, "my dentist")).await.unwrap();
        assert_eq!(r.groups, ["primary care", "imaging"], "an unknown group asks which");
        assert!(!r.other_visits);
    }

    #[tokio::test]
    async fn a_call_opens_with_its_hospital_s_names_and_label() {
        let s = svc();
        let reply = s.open_call(Request::new(pb::OpenCallRequest::default())).await.unwrap().into_inner();
        // No store: the file, labelled so in the call record.
        assert_eq!(reply.hospital_version, "file");
        assert_eq!(reply.times_of_day, Hospital::file().time_names());
        assert_eq!(reply.local_now.len(), 16);
        // A name the call's list does not hold is refused, by a fixed message.
        let h = verified(&s).await;
        let mut req = find_req_with(&h, "", "a check-up", pb::VisitMode::Unspecified);
        req.get_mut().time_of_day = "brunch".into();
        let err = s.find_slots(req).await.unwrap_err();
        assert_eq!(err.code(), Code::InvalidArgument);
        assert!(!err.message().contains("brunch"));
    }

    #[tokio::test]
    async fn a_bad_when_is_refused_and_no_match_is_empty() {
        let s = svc();
        let h = verified(&s).await;
        let err = ask_visit(&s, visit_req(&h, "next week", "", false, "")).await.unwrap_err();
        assert_eq!(err.code(), Code::InvalidArgument);
        let r = ask_visit(&s, visit_req(&h, "2027-03", "", false, "")).await.unwrap();
        assert!(r.visit.is_none() && r.groups.is_empty());
    }

    /// The Priya Raman call (2026-11-13): the reason given with FindSlots was lost at Book, so
    /// NextVisit said "none on record" about the visit just booked.
    #[tokio::test]
    async fn a_booking_keeps_the_callers_reason_and_its_type() {
        let s = svc();
        let h = verified(&s).await;
        let reason = "strawberry water distillation";
        let req = find_req_with(&h, "", reason, pb::VisitMode::Unspecified);
        let slot = s.find_slots(req).await.unwrap().into_inner().slots.remove(0);
        assert!(!slot.online);
        let reply = s.book(book_req(&h, &slot.token, "key-0000000000000001")).await.unwrap().into_inner();
        let visit = reply.visit.unwrap();
        // The hospital's spoken name for the fake's "Office Visit" (Config › Hospital's Default).
        assert_eq!((visit.reason.as_str(), visit.visit_type.as_str()), (reason, "an office visit"));
        assert!(!visit.online && !visit.reason_private);
        let next_visit = next(&s, &h).await.unwrap().unwrap();
        assert_eq!(next_visit.reason, reason);
    }

    #[test]
    fn the_usual_place_is_the_last_with_a_pcp_else_the_last_primary_care_place() {
        use deetshealth_core::Shielded;
        let list = serde_json::json!("urn:oid:1.2.840.114350.1.13.0.1.7.10.686980.110");
        let specialties = serde_json::json!([{"code": "9", "spoken": "family medicine", "primary_care": true}, {"code": "4", "spoken": "cardiology"}]);
        let locations = serde_json::json!([
            {"record": "North Family", "id": "d1", "specialty": "9"},
            {"record": "South Family", "id": "d2", "specialty": "9"},
            {"record": "Heart Clinic", "id": "d3", "specialty": "4"}
        ]);
        let h = Hospital::from_pairs([("specialty_list", &list), ("specialties", &specialties), ("locations", &locations)]).unwrap();
        let primary = h.pick(None, None);
        let visit = |place: &str, who: &str, at: &str| SeenVisit {
            place: PlaceRef::new(Some(place), None),
            place_words: Shielded::new(place.to_string()),
            clinician: ClinicianRef::new(Some(who), None),
            starts_at: Shielded::new(at.to_string()),
        };
        // Newest first, as `seen` keeps them.
        let seen = [visit("d3", "cardio", "2026-10-01T09:00"), visit("d2", "pcp-b", "2026-06-01T09:00"), visit("d1", "pcp-a", "2026-01-01T09:00")];
        let pcp = |ids: &[&str]| ids.iter().map(|i| ClinicianRef::new(Some(i), None)).collect::<Vec<_>>();
        let place_of = |u: Option<(SeenVisit, Option<ClinicianRef>)>| u.map(|(v, p)| (v.place, p));
        // Case 1: the last visit with the PCP, not the later cardiology visit.
        assert!(place_of(usual_place(&h, &seen, &pcp(&["pcp-a"]), &primary)) == Some((PlaceRef::new(Some("d1"), None), Some(ClinicianRef::new(Some("pcp-a"), None)))));
        // Case 4b: two PCPs, the one seen last.
        assert!(place_of(usual_place(&h, &seen, &pcp(&["pcp-a", "pcp-b"]), &primary)).is_some_and(|(p, _)| p == PlaceRef::new(Some("d2"), None)));
        // No PCP visit: the last primary care place; cardiology is not one.
        assert!(place_of(usual_place(&h, &seen, &pcp(&["nobody"]), &primary)) == Some((PlaceRef::new(Some("d2"), None), None)));
        assert!(usual_place(&h, &seen[..1], &[], &primary).is_none(), "cardiology only: no usual place");
    }

    /// Priya Raman, verified: PCP Dr. Okafor, never seen.
    async fn priya(s: &Svc) -> String {
        let h = open(s).await;
        identify(s, &h, "Priya", "Raman", "1971-11-30").await;
        assert_eq!(check(s, &h, FAKE_CODE).await.outcome, pb::CheckOutcome::Pass as i32);
        h
    }

    /// An in-person search with the caller's words for a place and a kind of care.
    fn find_at(handle: &str, place: &str, specialty: &str) -> Request<pb::FindSlotsRequest> {
        let mut r = find_req_with(handle, "", "a check-up", pb::VisitMode::InPerson).into_inner();
        r.place = place.into();
        r.specialty = specialty.into();
        Request::new(r)
    }

    /// BOOKING-RULES.md §3.3: Priya Raman's PCP is Dr. Okafor (the fake's file), whom she has not
    /// seen yet, so no usual place; at Main Street Clinic his times come first, and Dr. Lin's on
    /// an earlier day is the one sooner time, the last of the first offer (P-A). Jordan Ellis has
    /// no PCP on file: his times keep their order.
    #[tokio::test]
    async fn the_pcp_s_times_come_first_with_one_sooner_time_from_another_clinician() {
        let s = svc();
        let priya = priya(&s).await;
        let reply = s.find_slots(find_at(&priya, "Main Street", "")).await.unwrap().into_inner();
        let got: Vec<(&str, &str, bool, bool)> =
            reply.slots.iter().map(|x| (x.starts_at.as_str(), x.clinician.as_str(), x.with_pcp, x.sooner)).collect();
        assert_eq!(
            got,
            [
                ("2026-11-03T10:00", "Dr. Sam Okafor", true, false),
                ("2026-11-05T11:00", "Dr. Sam Okafor", true, false),
                ("2026-11-02T09:00", "Dr. Avery Lin", false, true),
            ]
        );
        assert!(reply.place_default.is_empty(), "never seen by her PCP: no usual place");
        // The rest, on "more": the other clinician's times, none marked sooner.
        let more = pb::FindSlotsRequest { handle: priya.clone(), more: true, ..Default::default() };
        let rest = s.find_slots(Request::new(more)).await.unwrap().into_inner();
        assert_eq!(rest.slots.iter().map(|x| (x.starts_at.as_str(), x.sooner)).collect::<Vec<_>>(), [("2026-11-02T14:30", false)]);

        let jordan = verified(&s).await;
        let reply = s.find_slots(find_req_with(&jordan, "", "a check-up", pb::VisitMode::InPerson)).await.unwrap().into_inner();
        let starts: Vec<&str> = reply.slots.iter().map(|x| x.starts_at.as_str()).collect();
        assert_eq!(starts, ["2026-11-02T09:00", "2026-11-02T14:30", "2026-11-03T10:00"]);
        assert!(reply.slots.iter().all(|x| !x.with_pcp && !x.sooner));
    }

    /// BOOKING-RULES.md §3.1, §3.2 (1B): Priya has no usual place, so with nothing named she is
    /// asked where, and the two primary care places are named; a plan says the same and searches
    /// nothing. Jordan's usual place is Main Street Clinic: a plan names it.
    #[tokio::test]
    async fn with_no_usual_place_the_caller_is_asked_where_and_a_plan_searches_nothing() {
        let s = svc();
        let priya = priya(&s).await;
        let reply = s.find_slots(find_at(&priya, "", "")).await.unwrap().into_inner();
        assert_eq!(reply.place_ask, pb::PlaceAsk::Which as i32);
        assert_eq!(reply.places, ["Main Street Clinic", "the family medicine clinic"]);
        assert!(reply.slots.is_empty() && !reply.other_places);
        // Anywhere: both places, Dr. Okafor first; Dr. Moreno is at the family medicine clinic.
        let mut any = find_at(&priya, "", "").into_inner();
        any.any_place = true;
        let reply = s.find_slots(Request::new(any)).await.unwrap().into_inner();
        assert_eq!(reply.place_ask, pb::PlaceAsk::None as i32);
        assert!(reply.more, "three of five offered");
        let more = s.find_slots(Request::new(pb::FindSlotsRequest { handle: priya.clone(), more: true, ..Default::default() })).await;
        let all: Vec<String> = more.unwrap().into_inner().slots.iter().map(|x| x.clinician.clone()).collect();
        assert!(all.iter().any(|c| c == "Dr. Lee Moreno"), "{all:?}");

        let mut plan = find_at(&priya, "", "").into_inner();
        plan.plan_only = true;
        assert_eq!(s.find_slots(Request::new(plan)).await.unwrap().into_inner().place_ask, pb::PlaceAsk::Which as i32);
        let jordan = verified(&s).await;
        let mut plan = find_at(&jordan, "", "").into_inner();
        plan.plan_only = true;
        let reply = s.find_slots(Request::new(plan)).await.unwrap().into_inner();
        assert_eq!((reply.place_ask, reply.place_default.as_str(), reply.slots.len()), (pb::PlaceAsk::None as i32, "Main Street Clinic", 0));
    }

    /// BOOKING-RULES.md §3.1: a place and a specialty in the caller's words, matched in Rust.
    #[tokio::test]
    async fn a_place_and_a_specialty_narrow_the_search_or_ask() {
        let s = svc();
        let h = verified(&s).await;
        let ask = |reply: &pb::FindSlotsReply| (reply.place_ask, reply.places.clone());
        let who = |reply: &pb::FindSlotsReply| reply.slots.iter().map(|x| x.clinician.clone()).collect::<Vec<_>>();
        // Cardiology: two departments, no visit at either: which one?
        let r = s.find_slots(find_at(&h, "", "a heart doctor")).await.unwrap().into_inner();
        assert_eq!(ask(&r), (pb::PlaceAsk::Which as i32, vec!["EMC Cardiology".to_string(), "EMC Cardiology Procedures".to_string()]));
        // The place and the specialty: Dr. Patel's times only, at EMC Cardiology (a full match).
        let r = s.find_slots(find_at(&h, "EMC cardiology", "cardiology")).await.unwrap().into_inner();
        assert_eq!(who(&r), ["Dr. Rosa Patel", "Dr. Rosa Patel"]);
        assert!(r.slots.iter().all(|x| x.place == "EMC Cardiology"));
        // The facility, no specialty: its primary care department only.
        let r = s.find_slots(find_at(&h, "the medical center", "")).await.unwrap().into_inner();
        assert_eq!(who(&r), ["Dr. Lee Moreno"]);
        // A town: the department there.
        let r = s.find_slots(find_at(&h, "Verona", "")).await.unwrap().into_inner();
        assert!(!r.slots.is_empty() && r.slots.iter().all(|x| x.place == "Main Street Clinic"));
        // Words the lists do not hold, and a place without that kind of care.
        assert_eq!(s.find_slots(find_at(&h, "Chicago", "")).await.unwrap().into_inner().place_ask, pb::PlaceAsk::UnknownPlace as i32);
        assert_eq!(s.find_slots(find_at(&h, "", "a dentist")).await.unwrap().into_inner().place_ask, pb::PlaceAsk::UnknownSpecialty as i32);
        assert_eq!(s.find_slots(find_at(&h, "", "a skin doctor")).await.unwrap().into_inner().place_ask, pb::PlaceAsk::UnknownSpecialty as i32, "no department has it");
        assert_eq!(s.find_slots(find_at(&h, "Main Street", "cardiology")).await.unwrap().into_inner().place_ask, pb::PlaceAsk::NotAtPlace as i32);
        // Words that name nothing are left out: Jordan's usual place.
        let r = s.find_slots(find_at(&h, "the clinic", "a doctor")).await.unwrap().into_inner();
        assert_eq!(r.place_default, "Main Street Clinic");
    }

    /// BOOKING-RULES.md §3.7: where a place is, before verification; its own hours, else the
    /// hospital's; the facility for a building that holds several; a place with no phone.
    #[tokio::test]
    async fn place_info_answers_from_the_lists_before_verification() {
        let s = svc();
        let h = open(&s).await;
        let ask = |words: &str| Request::new(pb::PlaceInfoRequest { handle: h.clone(), place: words.into() });
        let r = s.place_info(ask("Main Street")).await.unwrap().into_inner();
        assert_eq!(r.outcome, pb::PlaceInfoOutcome::Found as i32);
        assert_eq!((r.name.as_str(), r.address.as_str(), r.phone.as_str()), ("Main Street Clinic", "100 Main Street, Verona, WI 53593", "608-555-0100"));
        assert_eq!(r.hours.len(), 5, "the hospital's hours: Monday to Friday");
        let x = s.place_info(ask("the x-ray place")).await.unwrap().into_inner();
        assert_eq!(x.hours.last().map(|d| d.day.as_str()), Some("sat"), "its own hours");
        let c = s.place_info(ask("the medical center")).await.unwrap().into_inner();
        assert_eq!((c.name.as_str(), c.address.as_str()), ("the medical center", "200 Elm Avenue, Madison, WI 53703"));
        // EMC Cardiology Procedures has no phone and no address: the facility's.
        let p = s.place_info(ask("cardiology procedures")).await.unwrap().into_inner();
        assert_eq!((p.phone.as_str(), p.address.as_str()), ("608-555-0120", "200 Elm Avenue, Madison, WI 53703"));
        let w = s.place_info(ask("cardiology")).await.unwrap().into_inner();
        assert_eq!((w.outcome, w.places.len()), (pb::PlaceInfoOutcome::Which as i32, 2));
        assert_eq!(s.place_info(ask("Chicago")).await.unwrap().into_inner().outcome, pb::PlaceInfoOutcome::Unknown as i32);
        assert_eq!(s.place_info(Request::new(pb::PlaceInfoRequest { handle: "no".into(), place: "x".into() })).await.unwrap_err().code(), Code::InvalidArgument);
    }

    /// The Default hospital with these Booking rules, given to an open call.
    async fn with_rules(s: &Svc, handle: &str, rules: serde_json::Value) {
        let h = Hospital::from_pairs([("booking_rules", &rules)]).unwrap();
        let call = s.sessions.get(&Token::parse(handle).unwrap()).await.ok().unwrap();
        call.lock().await.hospital = Arc::new(h);
    }

    /// BOOKING-RULES.md §3.5: the first rule that matches wins; one known from the place alone
    /// answers before any search, one on a provider per time.
    #[tokio::test]
    async fn booking_rules_hold_back_hand_off_refer_flag_and_keep_to_the_pcp() {
        let s = svc();
        let h = verified(&s).await;
        // A new cardiology patient is handed off; Jordan has no cardiology visit.
        with_rules(&s, &h, serde_json::json!([{"specialty": "4", "patients": "new", "outcome": "hand_off"}])).await;
        let r = s.find_slots(find_at(&h, "EMC Cardiology", "cardiology")).await.unwrap().into_inner();
        assert_eq!((r.rule, r.slots.len()), (pb::RuleAsk::HandOff as i32, 0));
        // Primary care is not touched by it.
        assert!(!s.find_slots(find_at(&h, "", "")).await.unwrap().into_inner().slots.is_empty());

        // Per time: Dr. Patel's visits need a referral; the rule before it does not match.
        let rules = serde_json::json!([
            {"specialty": "9", "outcome": "hand_off", "patients": "new"},
            {"provider": "Dr. Rosa Patel", "outcome": "referral_needed"}
        ]);
        with_rules(&s, &h, rules).await;
        let r = s.find_slots(find_at(&h, "EMC Cardiology", "")).await.unwrap().into_inner();
        assert_eq!((r.rule, r.slots.len()), (pb::RuleAsk::ReferralNeeded as i32, 0));

        // Book and flag: the booking says so; another time does not.
        with_rules(&s, &h, serde_json::json!([{"visit_type": "Office Visit", "outcome": "book_and_flag"}])).await;
        let slots = s.find_slots(find_at(&h, "", "")).await.unwrap().into_inner().slots;
        let booked = s.book(book_req(&h, &slots[0].token, "key-0000000000000021")).await.unwrap().into_inner();
        assert!(booked.check_referral);
        let online = s.find_slots(find_req_with(&h, "", "a check-up", pb::VisitMode::Online)).await.unwrap().into_inner().slots;
        let booked = s.book(book_req(&h, &online[0].token, "key-0000000000000022")).await.unwrap().into_inner();
        assert!(!booked.check_referral, "a video visit is not an office visit");

        // Book with the PCP only: Priya hears Dr. Okafor's times and no sooner one.
        let p = priya(&s).await;
        with_rules(&s, &p, serde_json::json!([{"department": "Main Street Clinic", "outcome": "pcp_only"}])).await;
        let r = s.find_slots(find_at(&p, "Main Street", "")).await.unwrap().into_inner();
        assert!(!r.slots.is_empty() && r.slots.iter().all(|x| x.clinician == "Dr. Sam Okafor" && !x.sooner));
    }

    /// A visit asked about by its place (BOOKING-RULES.md §3.1).
    #[tokio::test]
    async fn a_visit_is_found_by_its_place_and_an_unknown_place_says_so() {
        let s = svc();
        let h = verified(&s).await;
        let mut req = visit_req(&h, "", "", false, "");
        req.place = "Main Street".into();
        let reply = ask_visit(&s, req.clone()).await.unwrap();
        assert!(reply.visit.is_some_and(|v| v.place == "Main Street Clinic"));
        req.place = "Chicago".into();
        let reply = ask_visit(&s, req).await.unwrap();
        assert!(reply.unknown_place && reply.visit.is_none());
    }

    /// BOOKING-RULES.md §3.4: Jordan Ellis was seen at Main Street Clinic in October, so his
    /// in-person visit is the established type; Priya Raman has no visit there, so hers is the
    /// new-patient type. One past-visit read a call, kept in the session.
    #[tokio::test]
    async fn a_new_patient_books_the_new_patient_type_and_an_established_one_does_not() {
        async fn book_in_person(s: &Svc, h: &str, key: &str) -> String {
            let slot = s.find_slots(find_at(h, "Main Street Clinic", "")).await.unwrap().into_inner().slots.remove(0);
            s.book(book_req(h, &slot.token, key)).await.unwrap().into_inner().visit.unwrap().visit_type
        }
        let s = svc();
        let jordan = verified(&s).await;
        assert_eq!(book_in_person(&s, &jordan, "key-0000000000000011").await, "an office visit");
        let call = s.sessions.get(&Token::parse(&jordan).unwrap()).await.ok().unwrap();
        assert!(call.lock().await.seen.as_ref().is_some_and(|v| v.len() == 1), "one seen visit, read once");

        let priya = open(&s).await;
        identify(&s, &priya, "Priya", "Raman", "1971-11-30").await;
        assert_eq!(check(&s, &priya, FAKE_CODE).await.outcome, pb::CheckOutcome::Pass as i32);
        assert_eq!(book_in_person(&s, &priya, "key-0000000000000012").await, "a new patient visit");
    }

    #[tokio::test]
    async fn an_online_wish_offers_only_online_slots() {
        let s = svc();
        let h = verified(&s).await;
        let req = find_req_with(&h, "", "a follow-up", pb::VisitMode::Online);
        let slots = s.find_slots(req).await.unwrap().into_inner().slots;
        assert_eq!(slots.len(), 2);
        assert!(slots.iter().all(|x| x.online));
        let req = find_req_with(&h, "", "a follow-up", pb::VisitMode::InPerson);
        let slots = s.find_slots(req).await.unwrap().into_inner().slots;
        assert!(slots.iter().all(|x| !x.online));
    }

    /// The note still goes to the scheduler; only the read-back is held back (FUTURE-SETTINGS.md
    /// §9).
    #[tokio::test]
    async fn a_sensitive_reason_makes_the_booked_visit_private() {
        let s = svc();
        let h = verified(&s).await;
        let req = find_req_with(&h, "", "a prenatal check", pb::VisitMode::Unspecified);
        let slot = s.find_slots(req).await.unwrap().into_inner().slots.remove(0);
        let reply = s.book(book_req(&h, &slot.token, "key-0000000000000001")).await.unwrap().into_inner();
        let visit = reply.visit.unwrap();
        assert!(visit.reason_private && visit.reason.is_empty() && visit.visit_type.is_empty());
    }

    #[tokio::test]
    async fn another_patient_has_no_visit() {
        let s = svc();
        let h = open(&s).await;
        identify(&s, &h, "Priya", "Raman", "1971-11-30").await;
        check(&s, &h, FAKE_CODE).await;
        assert!(next(&s, &h).await.unwrap().is_none());
    }

    /// Epic's sandbox is a second source only when ehr was given it (main.rs `open_epic`).
    #[tokio::test]
    async fn an_epic_call_is_refused_when_ehr_does_not_serve_epic() {
        let s = svc();
        let epic = pb::OpenCallRequest { ehr: pb::EhrSource::EpicSandbox.into(), ..Default::default() };
        assert_eq!(s.open_call(Request::new(epic)).await.unwrap_err().code(), Code::FailedPrecondition);
        let local = pb::OpenCallRequest { ehr: pb::EhrSource::Local.into(), ..Default::default() };
        assert!(s.open_call(Request::new(local)).await.is_ok());
        let unknown = pb::OpenCallRequest { ehr: 99, ..Default::default() };
        assert_eq!(s.open_call(Request::new(unknown)).await.unwrap_err().code(), Code::InvalidArgument);
    }

    /// Epic served but not signed in yet (Epic's sandbox down at start, 2026-11-14): an Epic
    /// call is UNAVAILABLE, so the agent can say Epic is not answering; Local still works.
    #[tokio::test]
    async fn an_epic_call_is_unavailable_while_the_sandbox_is_not_signed_in() {
        let s = svc().with_epic_sandbox(Arc::new(EpicSandbox::default()));
        let epic = pb::OpenCallRequest { ehr: pb::EhrSource::EpicSandbox.into(), ..Default::default() };
        let err = s.open_call(Request::new(epic)).await.unwrap_err();
        assert_eq!(err.code(), Code::Unavailable);
        assert_eq!(err.message(), "Epic's sandbox is not answering");
        let local = pb::OpenCallRequest { ehr: pb::EhrSource::Local.into(), ..Default::default() };
        assert!(s.open_call(Request::new(local)).await.is_ok());
    }

    /// A chart with no mobile number: the patient matches, but no code goes out, so the code
    /// check fails as after no match (Epic's sandbox has three such patients, EPIC.md §5.2).
    #[tokio::test]
    async fn a_match_with_no_number_for_a_text_sends_no_code() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct NoPhone;
        impl Patients for NoPhone {
            async fn candidates(&self, c: &crate::verify::Claimed) -> Result<Vec<Candidate>, Down> {
                Ok(vec![Candidate {
                    patient: PatientKey(deetshealth_core::Shielded::new("p1".into())),
                    given: c.given.clone(),
                    family: c.family.clone(),
                    birth_date: c.birth_date.clone(),
                    phone: None,
                    facts: None,
                    pcp: Vec::new(),
                }])
            }
        }
        struct Counted(AtomicUsize);
        impl Codes for Counted {
            fn issue(&self) -> deetshealth_core::Shielded<String> {
                deetshealth_core::Shielded::new(FAKE_CODE.into())
            }
            async fn send(&self, _: &deetshealth_core::Shielded<String>, _: &deetshealth_core::Shielded<String>, _: Option<&str>) -> Result<(), Down> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let s = EhrService::new(FakeScheduler::default(), NoPhone, Counted(AtomicUsize::new(0)));
        let h = s
            .open_call(Request::new(pb::OpenCallRequest::default()))
            .await
            .unwrap()
            .into_inner()
            .handle;
        let req = |h: &str| pb::IdentifyRequest {
            handle: h.into(),
            given_name: "Elijah".into(),
            family_name: "Davis".into(),
            birth_date: "1993-08-18".into(),
        };
        s.identify(Request::new(req(&h))).await.unwrap();
        assert_eq!(s.codes.0.load(Ordering::SeqCst), 0, "no code sent");
        let reply = s
            .check_code(Request::new(pb::CheckCodeRequest { handle: h.clone(), code: FAKE_CODE.into() }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(reply.outcome, pb::CheckOutcome::Fail as i32);
    }

    #[test]
    fn every_marked_field_is_listed() {
        // The contract marks these; a change to ehr.proto changes this list on purpose.
        let want = [
            ("FileCallLine", "line"),
            // A kept-script draw's failure line and an annotation's words (DATABASE.md §8, §9).
            ("TestRunResult", "failure"),
            ("Annotation", "words"),
            ("OpenCallReply", "handle"),
            ("CloseCallRequest", "handle"),
            ("IdentifyRequest", "handle"),
            ("IdentifyRequest", "given_name"),
            ("IdentifyRequest", "family_name"),
            ("IdentifyRequest", "birth_date"),
            ("ResendCodeRequest", "handle"),
            ("CheckCodeRequest", "handle"),
            ("CheckCodeRequest", "code"),
            ("FindSlotsRequest", "handle"),
            ("FindSlotsRequest", "reason"),
            ("FindSlotsRequest", "earliest_day"),
            ("FindSlotsRequest", "latest_day"),
            ("FindSlotsRequest", "time_of_day"),
            ("FindSlotsRequest", "clinician"),
            ("FindSlotsRequest", "place"),
            ("FindSlotsRequest", "specialty"),
            ("FindSlotsReply", "place_default"),
            ("FindSlotsReply", "places"),
            ("Slot", "token"),
            ("Slot", "starts_at"),
            ("Slot", "clinician"),
            ("Slot", "place"),
            ("BookRequest", "handle"),
            ("BookRequest", "slot_token"),
            ("FindVisitRequest", "handle"),
            ("FindVisitRequest", "when"),
            ("FindVisitRequest", "clinician"),
            ("FindVisitRequest", "group"),
            ("FindVisitRequest", "place"),
            ("Visit", "starts_at"),
            ("Visit", "clinician"),
            ("Visit", "place"),
            ("Visit", "visit_type"),
            ("Visit", "reason"),
            ("Visit", "instructions"),
            ("Visit", "csn"),
            ("FindVisitReply", "groups"),
            ("PlaceInfoRequest", "handle"),
            ("PlaceInfoRequest", "place"),
            ("PlaceInfoReply", "name"),
            ("PlaceInfoReply", "address"),
            ("PlaceInfoReply", "phone"),
            ("PlaceInfoReply", "places"),
        ];
        // The People list's messages come first in the file (ehr.proto, after the settings).
        let people = [
            ("User", "email"),
            ("User", "name"),
            ("User", "added_by"),
            ("User", "first_name"),
            ("User", "last_name"),
            ("User", "phone"),
            ("UpdateUserRequest", "first_name"),
            ("UpdateUserRequest", "last_name"),
            ("UpdateUserRequest", "phone"),
            ("UpdateUserRequest", "email"),
            ("UpdateUserReply", "old_email"),
            ("UserPhoto", "photo"),
            ("SetUserPhotoRequest", "photo"),
            ("AddUserRequest", "email"),
            ("AddUserRequest", "name"),
            ("AddUserRequest", "by"),
            ("RemoveUserReply", "email"),
            ("AdmitUserRequest", "email"),
            ("SeedEmail", "email"),
        ];
        let want: Vec<_> = people.into_iter().chain(want).collect();
        assert_eq!(pb::MARKED_FIELDS, want);
    }

    /// Every `rpc` in ehr.proto has its row in RPC_LEVERS, and every row names an RPC that
    /// exists (docs/architecture/ACCESS-ROLES.md §8.1).
    #[test]
    fn every_rpc_names_what_it_needs() {
        let proto = include_str!("../proto/deets/ehr/v1/ehr.proto");
        let rpcs: Vec<&str> = proto
            .lines()
            .filter_map(|l| l.trim().strip_prefix("rpc "))
            .filter_map(|l| l.split('(').next())
            .map(str::trim)
            .collect();
        assert!(rpcs.len() > 30, "the proto's RPCs were read");
        for rpc in &rpcs {
            assert!(RPC_LEVERS.iter().any(|(name, _)| name == rpc), "{rpc} has no row in RPC_LEVERS");
        }
        for (name, _) in RPC_LEVERS {
            assert!(rpcs.contains(name), "RPC_LEVERS names {name}, which ehr.proto does not have");
        }
        assert_eq!(need_of("NoSuchRpc"), Need::Lever(Lever::Roles, Level::Edit), "unknown: the most");
        assert_eq!(rpc_action("SaveSettings"), "rpc.save_settings");
    }

    /// Without a store, a staff RPC is refused before it reads anything, and a call RPC is not
    /// gated at all.
    #[tokio::test]
    async fn a_staff_rpc_needs_a_store_and_a_call_rpc_no_person() {
        let s = svc();
        let err = s.list_users(Request::new(pb::ListUsersRequest {})).await.unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
        let err = s.levels(Request::new(pb::LevelsRequest {})).await.unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
        assert!(s.gate("OpenCall", &tonic::metadata::MetadataMap::new(), None).await.unwrap().is_none());
    }
}
