//! The gRPC service (EHR-SERVICE.md §1). Each call: the edge checks the request, the session
//! decides whether the caller may ask, the adapters answer, the edge builds the reply.
//!
//! Every error message is a fixed string. None is built from a request or the EHR's reply.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use tonic::{Request, Response, Status};

use crate::{
    edge,
    epic_ehr::{EpicEhr, EpicSandbox},
    hospital::{Hospital, PlaceSearch},
    proto::v1::{self as pb, ehr_server::Ehr},
    scheduler::{BookResult, FoundSlots, FoundVisits, Scheduler, SchedulerDown, SlotId, SlotWish, VisitAsk},
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
        if let Some(Candidate { patient, phone: Some(phone), facts, .. }) = one_match(&claimed, found) {
            let code = self.codes.issue();
            self.codes.send(&phone, &code, session.text_template.as_deref()).await.map_err(down)?;
            session.stage = Stage::Unverified(Some(Pending {
                patient,
                facts,
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
                (p.code.ct_eq(&code) && p.issued.elapsed() < CODE_LIFETIME).then(|| (p.patient.clone(), p.facts.clone()))
            }
        };
        if let Some((patient, facts)) = right {
            session.stage = Stage::Verified(patient);
            session.facts = facts;
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

        // The EHR's work runs in its own task (`ledgered`), with the session unlocked, so its
        // ledger is kept even when the agent stops waiting.
        let (mut session, mut found) = match ask {
            // A new search. New tokens: a token from an older search no longer books. The
            // reason asked with these slots is the one Book sends.
            edge::SlotAsk::New(mut wish, time) => {
                wish.who = facts;
                // Only a name on the call's own list: the model picks, the hospital decides
                // what it means.
                if !time.is_empty() {
                    let t = h.time_of_day(&time).ok_or_else(|| Status::invalid_argument("time_of_day: not on the hospital's list"))?;
                    wish.time_of_day = Some(t);
                }
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
                    (session, page.slots)
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
            slots.push(edge::slot(&token, slot));
            session.offered.insert(token, slot.id.clone());
        }
        let more = !session.rest.is_empty() || !session.next_pages.is_empty();
        Ok(Response::new(pb::FindSlotsReply { slots, more }))
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
                    booked: Some(edge::slot(&slot_token, &slot)),
                    visit: Some(edge::visit(&slot)),
                },
                BookResult::Taken => pb::BookReply {
                    outcome: pb::BookOutcome::SlotTaken.into(),
                    booked: None,
                    visit: None,
                },
                BookResult::NotThrough => pb::BookReply {
                    outcome: pb::BookOutcome::NotThrough.into(),
                    booked: None,
                    visit: None,
                },
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
        let by_day = question.ask.range.is_some();
        let ask = question.ask;
        let hh = Arc::clone(&h);
        let found = ledgered(&session, "FindVisit", async move { sched.visits(&hh, &patient, &ask).await }).await?;
        let FoundVisits { visits: found, within_days } = found.map_err(scheduler_down)?;
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
        // Checked first, so a bad run is refused the same way with or without a store.
        let run = edge::test_run(req.into_inner())?;
        let store = self.settings_store()?;
        let filed = store.synthetic().file_test_run(&run).await.map_err(|_| Status::internal("the store could not file the run"))?;
        Ok(Response::new(pb::FileTestRunReply {
            run_id: filed.run_id,
            needed: filed.needed,
            pruned: i32::try_from(filed.pruned).unwrap_or(i32::MAX),
        }))
    }

    async fn add_annotation(&self, req: Request<pb::Annotation>) -> Result<Response<pb::AddAnnotationReply>, Status> {
        let a = edge::annotation(req.into_inner())?;
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

    async fn list_annotations(&self, _: Request<pb::ListAnnotationsRequest>) -> Result<Response<pb::ListAnnotationsReply>, Status> {
        let store = self.settings_store()?;
        let found = store.synthetic().list_annotations().await.map_err(|_| Status::internal("the store could not read the annotations"))?;
        Ok(Response::new(pb::ListAnnotationsReply { annotations: found.into_iter().map(edge::annotation_out).collect() }))
    }

    async fn list_setting_versions(
        &self,
        req: Request<pb::ListSettingVersionsRequest>,
    ) -> Result<Response<pb::ListSettingVersionsReply>, Status> {
        let (name, _, _) = setting_service(&req.get_ref().service)?;
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
        let v = settings::save(store.pool(), name, table, &values, by, req.seed).await.map_err(|e| match e {
            settings::SaveError::DefaultExists => Status::already_exists("the Default version exists"),
            settings::SaveError::Db(_) => Status::internal("the store could not save the settings"),
        })?;
        Ok(Response::new(pb::SaveSettingsReply { version: Some(setting_version(v)) }))
    }

    async fn list_users(&self, _: Request<pb::ListUsersRequest>) -> Result<Response<pb::ListUsersReply>, Status> {
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
            })
            .collect();
        Ok(Response::new(pb::ListUsersReply { users }))
    }

    async fn add_user(&self, req: Request<pb::AddUserRequest>) -> Result<Response<pb::AddUserReply>, Status> {
        let req = req.into_inner();
        // Checked before the store, so a bad request is refused the same way with or without one.
        users::normal_new(&req.email, &req.name).map_err(users_status)?;
        let store = self.settings_store()?;
        let id = users::add(store, &req.email, &req.name, &req.by).await.map_err(users_status)?;
        Ok(Response::new(pb::AddUserReply { id }))
    }

    async fn remove_user(&self, req: Request<pb::RemoveUserRequest>) -> Result<Response<pb::RemoveUserReply>, Status> {
        let store = self.settings_store()?;
        let email = users::remove(store, &req.get_ref().id).await.map_err(users_status)?;
        Ok(Response::new(pb::RemoveUserReply { email: email.to_string() }))
    }

    async fn admit_user(&self, req: Request<pb::AdmitUserRequest>) -> Result<Response<pb::AdmitUserReply>, Status> {
        let store = self.settings_store()?;
        let active = users::admit(store, &req.get_ref().email).await.map_err(users_status)?;
        Ok(Response::new(pb::AdmitUserReply { active }))
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
        let req = req.into_inner();
        let store = self.settings_store()?;
        let profile = users::Profile {
            first_name: &req.first_name,
            last_name: &req.last_name,
            phone: &req.phone,
            email: &req.email,
        };
        let saved = users::update(store, &req.id, &profile).await.map_err(users_status)?;
        let old_email = if saved.email_changed { saved.old_email.to_string() } else { String::new() };
        Ok(Response::new(pb::UpdateUserReply { old_email, email_changed: saved.email_changed }))
    }

    async fn get_user_photo(&self, req: Request<pb::GetUserPhotoRequest>) -> Result<Response<pb::UserPhoto>, Status> {
        let store = self.settings_store()?;
        let bytes = users::photo(store, &req.get_ref().id).await.map_err(users_status)?;
        let photo = bytes.map(|b| B64.encode(b.as_slice())).unwrap_or_default();
        Ok(Response::new(pb::UserPhoto { photo }))
    }

    async fn set_user_photo(&self, req: Request<pb::SetUserPhotoRequest>) -> Result<Response<pb::SetUserPhotoReply>, Status> {
        let req = req.into_inner();
        // Base64 of 256 KB is about 350 KB: anything far longer is refused before it is decoded.
        if req.photo.len() > users::MAX_PHOTO * 4 / 3 + 8 {
            return Err(Status::invalid_argument("photo: at most 256 KB"));
        }
        let store = self.settings_store()?;
        if req.photo.is_empty() {
            users::clear_photo(store, &req.id).await.map_err(users_status)?;
        } else {
            let bytes = Zeroizing::new(
                B64.decode(req.photo.as_bytes()).map_err(|_| Status::invalid_argument("photo: not base64"))?,
            );
            users::set_photo(store, &req.id, &bytes).await.map_err(users_status)?;
        }
        Ok(Response::new(pb::SetUserPhotoReply {}))
    }

    async fn seen_owner(&self, _: Request<pb::SeenOwnerRequest>) -> Result<Response<pb::SeenOwnerReply>, Status> {
        let store = self.settings_store()?;
        users::seen_owner(store).await.map_err(users_status)?;
        Ok(Response::new(pb::SeenOwnerReply {}))
    }

    async fn import_places(&self, req: Request<pb::ImportPlacesRequest>) -> Result<Response<pb::ImportPlacesReply>, Status> {
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
        users::UsersError::Owner => Status::permission_denied("the owner cannot be removed"),
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
        pb::FindVisitRequest { handle: h.into(), when: when.into(), clinician: clinician.into(), past, group: group.into() }
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
            ("Visit", "starts_at"),
            ("Visit", "clinician"),
            ("Visit", "place"),
            ("Visit", "visit_type"),
            ("Visit", "reason"),
            ("Visit", "instructions"),
            ("Visit", "csn"),
            ("FindVisitReply", "groups"),
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
}
