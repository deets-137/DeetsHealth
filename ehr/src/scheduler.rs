//! Booking, behind one trait (VOICE-AGENT.md §8.4): each EHR gets an adapter. Phase 1 has the
//! in-memory fake only. Phase 2 adds Epic behind the same trait.
//!
//! The fake is deterministic: fixed slots on fixed days, no clock. A tool result that changed
//! from run to run would change the prompt and miss the replay cache (EHR-SERVICE.md §7).

use std::{collections::HashSet, future::Future, sync::Mutex};

use deetshealth_core::Shielded;

use crate::{
    sandbox::sandbox,
    session::PatientKey,
    visit_about::{BookNote, VisitAbout},
};

/// The EHR's own id for a slot. It never leaves this service: the agent gets a token.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SlotId(pub String);

/// One open slot. Once booked for a patient it is part of their chart, so its words are
/// shielded from the start.
#[derive(Clone)]
pub struct Slot {
    pub id: SlotId,
    /// YYYY-MM-DDTHH:MM, local time at the place.
    pub starts_at: Shielded<String>,
    pub minutes: u32,
    pub clinician: Shielded<String>,
    pub place: Shielded<String>,
    /// Its type and reason, after the sensitive screen (visit_about.rs). Empty for an open
    /// slot.
    pub about: VisitAbout,
}

/// How the caller wants to be seen (the owner's 1A, 2026-11-13). At Epic, online or in person
/// is a visit type, so this picks among the hospital's visit types (EPIC.md §5.2).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum VisitMode {
    #[default]
    Either,
    InPerson,
    Online,
}

impl VisitMode {
    /// Whether a slot that is (or is not) online fits.
    pub fn fits(self, online: bool) -> bool {
        match self {
            VisitMode::Either => true,
            VisitMode::InPerson => !online,
            VisitMode::Online => online,
        }
    }
}

/// What the caller asked for.
pub struct SlotWish {
    /// The caller's reason, screened: the visit's note when they book (Book).
    pub note: BookNote,
    /// YYYY-MM-DD; none means from the next open day.
    pub earliest_day: Option<Shielded<String>>,
    pub mode: VisitMode,
}

#[derive(Clone)]
pub enum BookResult {
    Booked(Slot),
    /// Someone else took it first.
    Taken,
}

/// The EHR did not answer. Never carries text from the EHR.
pub struct SchedulerDown;

/// The most slots one FindSlots returns: enough to choose from, short enough to say aloud.
pub const MAX_OFFERED: usize = 3;

pub trait Scheduler: Send + Sync + 'static {
    fn find(
        &self,
        patient: &PatientKey,
        wish: &SlotWish,
    ) -> impl Future<Output = Result<Vec<Slot>, SchedulerDown>> + Send;

    /// Books `slot` with `note`, the caller's reason, as the visit's note for the scheduler
    /// (Epic's `appointmentNote`). The booked slot's `about` holds its type and the reason.
    fn book(
        &self,
        patient: &PatientKey,
        slot: &SlotId,
        note: &BookNote,
    ) -> impl Future<Output = Result<BookResult, SchedulerDown>> + Send;

    /// The patient's soonest visit still to come, if any (DESIGN.md S5).
    fn next_visit(
        &self,
        patient: &PatientKey,
    ) -> impl Future<Output = Result<Option<Slot>, SchedulerDown>> + Send;
}

/// The fake's fixed slots: synthetic clinicians at a synthetic clinic, each with a visit type
/// as Epic names them (2026-11-14). The first three stay in person, so a list for "either"
/// reads as before; two are online, for `VisitMode::Online`.
const FAKE_SLOTS: &[(&str, &str, &str, u32, &str, &str)] = &[
    // (id, day, time, minutes, clinician, visit type)
    ("slot-01", "2026-11-02", "09:00", 20, "Dr. Avery Lin", "Office Visit"),
    ("slot-02", "2026-11-02", "14:30", 20, "Dr. Avery Lin", "Office Visit"),
    ("slot-03", "2026-11-03", "10:00", 30, "Dr. Sam Okafor", "Office Visit"),
    ("slot-04", "2026-11-04", "08:30", 20, "Dr. Avery Lin", "Video Visit"),
    ("slot-05", "2026-11-05", "11:00", 30, "Dr. Sam Okafor", "Office Visit"),
    ("slot-06", "2026-11-06", "15:00", 20, "Dr. Avery Lin", "Video Visit"),
];

/// The in-memory scheduler for tests and the text harness (VOICE-AGENT.md §9).
pub struct FakeScheduler {
    /// Each fake slot's day (YYYY-MM-DD), by its place in `FAKE_SLOTS`.
    days: Vec<String>,
    place: String,
    taken: Mutex<HashSet<String>>,
    /// Bookings made on this run, by patient.
    booked: Mutex<Vec<(PatientKey, Slot)>>,
}

impl Default for FakeScheduler {
    /// The fixed days, at the sandbox's place.
    fn default() -> Self {
        Self::with_days(FAKE_SLOTS.iter().map(|s| s.1.to_string()).collect(), sandbox().place.clone())
    }
}

impl FakeScheduler {
    fn with_days(days: Vec<String>, place: String) -> Self {
        Self { days, place, taken: Mutex::default(), booked: Mutex::default() }
    }

    /// The same slots, moved to real days: the fake's first day becomes the first weekday after
    /// `today`, and each later fake day the next weekday after that. For Epic mode
    /// (epic_ehr.rs), where the call's "today" is the real date, until
    /// booking through STU3 `$find` and `$book` is built (E-4, EPIC.md §5.2).
    pub fn from_weekdays_after(today: jiff::civil::Date, place: String) -> Self {
        use jiff::civil::Weekday;
        let mut fake_days: Vec<&str> = FAKE_SLOTS.iter().map(|s| s.1).collect();
        fake_days.dedup();
        let mut real = Vec::with_capacity(fake_days.len());
        let mut day = today;
        while real.len() < fake_days.len() {
            day = day.tomorrow().expect("a date before 9999");
            if !matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday) {
                real.push(day.to_string());
            }
        }
        let days = FAKE_SLOTS
            .iter()
            .map(|s| real[fake_days.iter().position(|d| *d == s.1).unwrap()].clone())
            .collect();
        Self::with_days(days, place)
    }

    /// An open slot: its type is known (and so whether it is online), its reason is not yet.
    fn slot(&self, i: usize) -> Slot {
        let (id, _, time, minutes, clinician, visit_type) = FAKE_SLOTS[i];
        Slot {
            id: SlotId(id.to_string()),
            starts_at: Shielded::new(format!("{}T{time}", self.days[i])),
            minutes,
            clinician: Shielded::new(clinician.to_string()),
            place: Shielded::new(self.place.clone()),
            about: VisitAbout::from_record(Some(visit_type), None),
        }
    }

    /// The visits booked on this run for `patient`.
    pub fn booked_for(&self, patient: &PatientKey) -> Vec<Slot> {
        self.booked.lock().unwrap().iter().filter(|(k, _)| k == patient).map(|(_, s)| s.clone()).collect()
    }
}

impl Scheduler for FakeScheduler {
    async fn find(&self, _: &PatientKey, wish: &SlotWish) -> Result<Vec<Slot>, SchedulerDown> {
        let taken = self.taken.lock().unwrap();
        Ok((0..FAKE_SLOTS.len())
            .filter(|&i| !taken.contains(FAKE_SLOTS[i].0))
            // Shielded compares without revealing.
            .filter(|&i| {
                wish.earliest_day
                    .as_ref()
                    .is_none_or(|from| Shielded::new(self.days[i].clone()) >= *from)
            })
            .map(|i| self.slot(i))
            .filter(|slot| wish.mode.fits(slot.about.online))
            .take(MAX_OFFERED)
            .collect())
    }

    async fn book(&self, patient: &PatientKey, slot: &SlotId, note: &BookNote) -> Result<BookResult, SchedulerDown> {
        let Some(i) = FAKE_SLOTS.iter().position(|s| s.0 == slot.0) else {
            return Ok(BookResult::Taken);
        };
        if !self.taken.lock().unwrap().insert(slot.0.clone()) {
            return Ok(BookResult::Taken);
        }
        // The visit keeps the caller's reason as its note, as Epic's `$book` will
        // (`appointmentNote`); NextVisit reads it back, through the sensitive screen.
        let mut booked = self.slot(i);
        booked.about = VisitAbout::booked(Some(FAKE_SLOTS[i].5), note);
        self.booked.lock().unwrap().push((patient.clone(), booked.clone()));
        Ok(BookResult::Booked(booked))
    }

    async fn next_visit(&self, patient: &PatientKey) -> Result<Option<Slot>, SchedulerDown> {
        // The visits the sandbox patients already have (`ehr/fixtures/sandbox.json`).
        let sandbox = sandbox();
        let fixed = sandbox.visits.iter().map(|v| {
            let key = PatientKey(Shielded::new(v.patient.clone()));
            let visit = Slot {
                id: SlotId(format!("visit-{}", v.patient)),
                starts_at: Shielded::new(v.starts_at.clone()),
                minutes: v.minutes,
                clinician: Shielded::new(v.clinician.clone()),
                place: Shielded::new(sandbox.place.clone()),
                about: VisitAbout::from_record(v.visit_type.as_deref(), v.reason.as_deref()),
            };
            (key, visit)
        });
        let booked = self.booked.lock().unwrap().clone();
        // Shielded compares without revealing: the patient, and which start comes first.
        Ok(fixed
            .chain(booked)
            .filter(|(key, _)| key == patient)
            .map(|(_, visit)| visit)
            .min_by(|a, b| a.starts_at.cmp(&b.starts_at)))
    }
}
