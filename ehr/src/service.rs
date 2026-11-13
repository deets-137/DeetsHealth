//! The gRPC service (EHR-SERVICE.md §1). Each call: the edge checks the request, the session
//! decides whether the caller may ask, the adapters answer, the edge builds the reply.
//!
//! Every error message is a fixed string. None is built from a request or the EHR's reply.

use std::{sync::Arc, time::Duration};

use tonic::{Request, Response, Status};

use crate::{
    edge,
    epic_ehr::EpicEhr,
    proto::v1::{self as pb, ehr_server::Ehr},
    scheduler::{BookResult, Scheduler, SchedulerDown, Slot, SlotId, SlotWish},
    session::{Pending, PatientKey, Refusal, Sessions, Source, Stage, Token},
    settings,
    store::Store,
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
    async fn find(&self, patient: &PatientKey, wish: &SlotWish) -> Result<Vec<Slot>, SchedulerDown> {
        match self {
            Sched::Local(s) => s.find(patient, wish).await,
            Sched::Epic(e) => e.scheduler.find(patient, wish).await,
        }
    }

    async fn book(&self, patient: &PatientKey, slot: &SlotId) -> Result<BookResult, SchedulerDown> {
        match self {
            Sched::Local(s) => s.book(patient, slot).await,
            Sched::Epic(e) => e.scheduler.book(patient, slot).await,
        }
    }

    async fn next_visit(&self, patient: &PatientKey) -> Result<Option<Slot>, SchedulerDown> {
        match self {
            Sched::Local(s) => s.next_visit(patient).await,
            Sched::Epic(e) => e.scheduler.next_visit(patient).await,
        }
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
    /// `EHR_SOURCE_EPIC_SANDBOX` finds the caller and their visits there (dev only).
    epic: Option<Arc<EpicEhr>>,
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
    pub fn with_epic(mut self, epic: Arc<EpicEhr>) -> Self {
        self.epic = Some(epic);
        self
    }

    fn settings_store(&self) -> Result<&Arc<Store>, Status> {
        self.store.as_ref().ok_or_else(|| Status::failed_precondition("ehr runs without a store"))
    }

    fn epic(&self) -> Result<&Arc<EpicEhr>, Status> {
        self.epic.as_ref().ok_or_else(|| Status::failed_precondition("ehr does not serve Epic's sandbox"))
    }

    fn sched(&self, source: Source) -> Result<Sched<S>, Status> {
        Ok(match source {
            Source::Local => Sched::Local(Arc::clone(&self.scheduler)),
            Source::EpicSandbox => Sched::Epic(Arc::clone(self.epic()?)),
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
        if source == Source::EpicSandbox {
            self.epic()?;
        }
        let handle = self.sessions.open(source).await.map_err(refused)?;
        Ok(Response::new(pb::OpenCallReply {
            handle: handle.to_hex(),
            identifies_left: MAX_IDENTIFIES,
            resends_left: MAX_RESENDS,
            code_tries_left: MAX_CODE_FAILURES,
        }))
    }

    async fn close_call(
        &self,
        req: Request<pb::CloseCallRequest>,
    ) -> Result<Response<pb::CloseCallReply>, Status> {
        let handle = edge::handle(&req.get_ref().handle)?;
        self.sessions.close(&handle).await;
        Ok(Response::new(pb::CloseCallReply {}))
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
        let found = match session.source {
            Source::Local => self.patients.candidates(&claimed).await,
            Source::EpicSandbox => self.epic()?.patients.candidates(&claimed).await,
        }
        .map_err(down)?;
        // One match with a number that takes a text: a code goes out. With no such number on
        // file, nothing is sent, the same as no match.
        if let Some(Candidate { patient, phone: Some(phone), .. }) = one_match(&claimed, found) {
            let code = self.codes.issue();
            self.codes.send(&phone, &code).await.map_err(down)?;
            session.stage = Stage::Unverified(Some(Pending {
                patient,
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
        if let Stage::Unverified(Some(p)) = &mut session.stage {
            p.code = self.codes.issue();
            p.issued = std::time::Instant::now();
            self.codes.send(&p.phone, &p.code).await.map_err(down)?;
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
                (p.code.ct_eq(&code) && p.issued.elapsed() < CODE_LIFETIME).then(|| p.patient.clone())
            }
        };
        if let Some(patient) = right {
            session.stage = Stage::Verified(patient);
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
        let (handle, wish) = edge::find_slots(req.into_inner())?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let mut session = session.lock().await;
        let patient = session.patient().map_err(refused)?;
        let found = self.sched(session.source)?.find(&patient, &wish).await.map_err(scheduler_down)?;

        // New tokens each time: a token from an older FindSlots no longer books.
        session.offered.clear();
        let mut slots = Vec::with_capacity(found.len());
        for slot in &found {
            let token = Token::random();
            slots.push(edge::slot(&token, slot));
            session.offered.insert(token, slot.id.clone());
        }
        Ok(Response::new(pb::FindSlotsReply { slots }))
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
                    let result = scheduler.book(&patient, &slot_id).await.map_err(scheduler_down)?;
                    session.booked.insert(key, (slot_token.clone(), result.clone()));
                    result
                }
            };
            Ok(match result {
                BookResult::Booked(slot) => pb::BookReply {
                    outcome: pb::BookOutcome::Booked.into(),
                    booked: Some(edge::slot(&slot_token, &slot)),
                },
                BookResult::Taken => {
                    pb::BookReply { outcome: pb::BookOutcome::SlotTaken.into(), booked: None }
                }
            })
        });
        let reply = task.await.map_err(|_| Status::internal("the booking stopped"))??;
        Ok(Response::new(reply))
    }

    async fn next_visit(
        &self,
        req: Request<pb::NextVisitRequest>,
    ) -> Result<Response<pb::NextVisitReply>, Status> {
        let handle = edge::handle(&req.get_ref().handle)?;
        let session = self.sessions.get(&handle).await.map_err(refused)?;
        let (patient, source) = {
            let session = session.lock().await;
            (session.patient().map_err(refused)?, session.source)
        };
        let visit = self.sched(source)?.next_visit(&patient).await.map_err(scheduler_down)?;
        Ok(Response::new(pb::NextVisitReply { visit: visit.as_ref().map(edge::visit) }))
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
        let store = self.store.as_ref().ok_or_else(|| Status::failed_precondition("ehr runs without a store"))?;
        let filed = store.file_record(&id, &record).await.map_err(|_| Status::internal("the store could not file the call"))?;
        Ok(Response::new(pb::FileCallReply { filed }))
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
        let v = settings::save(store.pool(), name, table, &values, by, req.seed).await.map_err(|e| match e {
            settings::SaveError::DefaultExists => Status::already_exists("the Default version exists"),
            settings::SaveError::Db(_) => Status::internal("the store could not save the settings"),
        })?;
        Ok(Response::new(pb::SaveSettingsReply { version: Some(setting_version(v)) }))
    }
}

/// The service a settings request names, with its list and table (settings.rs).
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
        Request::new(pb::FindSlotsRequest {
            handle: handle.into(),
            reason: "a check-up".into(),
            earliest_day: day.into(),
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

    // ---- the next visit ----

    async fn next(s: &Svc, h: &str) -> Result<Option<pb::Visit>, Status> {
        s.next_visit(Request::new(pb::NextVisitRequest { handle: h.into() }))
            .await
            .map(|r| r.into_inner().visit)
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
        let epic = pb::OpenCallRequest { ehr: pb::EhrSource::EpicSandbox.into() };
        assert_eq!(s.open_call(Request::new(epic)).await.unwrap_err().code(), Code::FailedPrecondition);
        let local = pb::OpenCallRequest { ehr: pb::EhrSource::Local.into() };
        assert!(s.open_call(Request::new(local)).await.is_ok());
        let unknown = pb::OpenCallRequest { ehr: 99 };
        assert_eq!(s.open_call(Request::new(unknown)).await.unwrap_err().code(), Code::InvalidArgument);
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
                }])
            }
        }
        struct Counted(AtomicUsize);
        impl Codes for Counted {
            fn issue(&self) -> deetshealth_core::Shielded<String> {
                deetshealth_core::Shielded::new(FAKE_CODE.into())
            }
            async fn send(&self, _: &deetshealth_core::Shielded<String>, _: &deetshealth_core::Shielded<String>) -> Result<(), Down> {
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
            ("Slot", "token"),
            ("Slot", "starts_at"),
            ("Slot", "clinician"),
            ("Slot", "place"),
            ("BookRequest", "handle"),
            ("BookRequest", "slot_token"),
            ("NextVisitRequest", "handle"),
            ("Visit", "starts_at"),
            ("Visit", "clinician"),
            ("Visit", "place"),
            ("Visit", "visit_type"),
            ("Visit", "reason"),
        ];
        assert_eq!(pb::MARKED_FIELDS, want);
    }
}
