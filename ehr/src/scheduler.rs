//! Booking, behind one trait (VOICE-AGENT.md §8.4): each EHR gets an adapter. Phase 1 has the
//! in-memory fake only. Phase 2 adds Epic behind the same trait.
//!
//! The fake is deterministic: fixed slots on fixed days, no clock. A tool result that changed
//! from run to run would change the prompt and miss the replay cache (EHR-SERVICE.md §7).

use std::{collections::HashSet, future::Future, sync::Mutex};

use deetshealth_core::Shielded;

use crate::{
    hospital::{Hospital, TimeOfDay},
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
    /// The visit's number at the hospital (Epic's CSN), once booked. For staff only: it goes in
    /// a flag's notes, never to the model (FLAGS.md §3.3).
    pub csn: Option<Shielded<String>>,
    pub status: VisitStatus,
    /// The hospital's visit group (visits.rs), from the visit type. None for a private visit.
    pub group: Option<Shielded<String>>,
}

/// Where a visit stands, as the agent may say it (EPIC.md §7.10).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum VisitStatus {
    /// Booked and still to come (also an open slot).
    #[default]
    Upcoming,
    Seen,
    Missed,
    Cancelled,
}

impl VisitStatus {
    /// From Epic's status words. A visit still `booked` after its time has no known outcome,
    /// so a caller hears nothing of it: None. `past` says whether the visit's time has gone.
    pub fn from_epic(status: &str, past: bool) -> Option<Self> {
        match (status, past) {
            ("booked" | "pending" | "arrived" | "checked-in", false) => Some(Self::Upcoming),
            ("arrived" | "checked-in" | "fulfilled", true) => Some(Self::Seen),
            ("noshow", _) => Some(Self::Missed),
            ("cancelled", _) => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// The days a visit search covers: from `from` (YYYY-MM-DD) up to, not including, `to`.
#[derive(Clone)]
pub struct DayRange {
    pub from: Shielded<String>,
    pub to: Shielded<String>,
}

/// The visits a search found, and the window they were found in (days ahead; None: no
/// limit), so the agent says "in the next 7 days" truthfully.
#[derive(Default)]
pub struct FoundVisits {
    pub visits: Vec<Slot>,
    pub within_days: Option<u32>,
}

/// Which of the caller's visits to search (EPIC.md §7.10). None and not past: the visits to
/// come. The clinician and the group are narrowed after, in visits.rs, the same for every EHR.
pub struct VisitAsk {
    /// A day or a month the caller named: every status in it.
    pub range: Option<DayRange>,
    /// The caller asked about a visit that is over ("when was I last seen?").
    pub past: bool,
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

/// What the caller asked for (EPIC.md §7.10, build 2).
pub struct SlotWish {
    /// The caller's reason, screened: the visit's note when they book (Book).
    pub note: BookNote,
    /// YYYY-MM-DD; none means from the next open day.
    pub earliest_day: Option<Shielded<String>>,
    /// YYYY-MM-DD, the last day the caller can come; none means no end ($find's `endTime`).
    pub latest_day: Option<Shielded<String>>,
    pub mode: VisitMode,
    /// A time of day from the hospital's list ("morning"; $find's `time-of-day`).
    pub time_of_day: Option<TimeOfDay>,
    /// The days of the week the caller can come; empty means any.
    pub days: Vec<jiff::civil::Weekday>,
    /// The caller's words for a clinician: the slots are narrowed to them after the search
    /// ($find has no clinician input).
    pub clinician: Option<Shielded<String>>,
    /// The verified patient's demographics for Epic's `$find` (a Patient resource's
    /// `identifier`, `name` and `birthDate`, as JSON), from the search that found them. The
    /// service sets it; none for the fake.
    pub who: Option<Shielded<String>>,
}

/// One search's slots, all of them, in order (the service offers a few at a time), and the
/// EHR's links to more pages (Epic's `next`).
#[derive(Default)]
pub struct FoundSlots {
    pub slots: Vec<Slot>,
    pub next: Vec<String>,
}

// One answer per call, never stored in bulk: the size of the visit does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum BookResult {
    Booked(Slot),
    /// Someone else took it first.
    Taken,
    /// The EHR's reply was lost, and a search found no such booking (the owner's V2A): the caller
    /// is asked whether to try again. Never kept as the key's answer, so a retry books.
    NotThrough,
}

/// The EHR did not answer. Never carries text from the EHR.
pub struct SchedulerDown;

/// Each method takes the call's hospital (HOSPITAL-CONFIG.md): its visit types, time zone,
/// places, groups and search numbers, as they were when the call opened.
pub trait Scheduler: Send + Sync + 'static {
    /// Every slot that fits `wish`, soonest first.
    fn find(
        &self,
        h: &Hospital,
        patient: &PatientKey,
        wish: &SlotWish,
    ) -> impl Future<Output = Result<FoundSlots, SchedulerDown>> + Send;

    /// The next page of a search: the first of `next`, which `find` gave.
    fn find_more(
        &self,
        h: &Hospital,
        patient: &PatientKey,
        next: &[String],
    ) -> impl Future<Output = Result<FoundSlots, SchedulerDown>> + Send;

    /// Books `slot` with `note`, the caller's reason, as the visit's note for the scheduler
    /// (Epic's `appointmentNote`). The booked slot's `about` holds its type and the reason.
    fn book(
        &self,
        h: &Hospital,
        patient: &PatientKey,
        slot: &SlotId,
        note: &BookNote,
    ) -> impl Future<Output = Result<BookResult, SchedulerDown>> + Send;

    /// The patient's visits that `ask` covers, in the order they are said: the soonest first,
    /// or the newest first for a past one (DESIGN.md S5; EPIC.md §7.10).
    fn visits(
        &self,
        h: &Hospital,
        patient: &PatientKey,
        ask: &VisitAsk,
    ) -> impl Future<Output = Result<FoundVisits, SchedulerDown>> + Send;
}

/// The fake's fixed slots: synthetic clinicians at a synthetic clinic. Each slot is in person
/// or online; its visit type is the hospital's first type of that kind that the agent may book
/// (`Hospital::fake_type`; the owner's 2A, 2026-11-15). The first three stay in person, so a
/// list for "either" reads as before; two are online, for `VisitMode::Online`.
const FAKE_SLOTS: &[(&str, &str, &str, u32, &str, bool)] = &[
    // (id, day, time, minutes, clinician, online)
    ("slot-01", "2026-11-02", "09:00", 20, "Dr. Avery Lin", false),
    ("slot-02", "2026-11-02", "14:30", 20, "Dr. Avery Lin", false),
    ("slot-03", "2026-11-03", "10:00", 30, "Dr. Sam Okafor", false),
    ("slot-04", "2026-11-04", "08:30", 20, "Dr. Avery Lin", true),
    ("slot-05", "2026-11-05", "11:00", 30, "Dr. Sam Okafor", false),
    ("slot-06", "2026-11-06", "15:00", 20, "Dr. Avery Lin", true),
];

/// The in-memory scheduler for tests and the text harness (VOICE-AGENT.md §9).
pub struct FakeScheduler {
    /// Each fake slot's day (YYYY-MM-DD), by its place in `FAKE_SLOTS`.
    days: Vec<String>,
    taken: Mutex<HashSet<String>>,
    /// Bookings made on this run, by patient.
    booked: Mutex<Vec<(PatientKey, Slot)>>,
}

impl Default for FakeScheduler {
    /// The fixed days.
    fn default() -> Self {
        Self::with_days(FAKE_SLOTS.iter().map(|s| s.1.to_string()).collect())
    }
}

impl FakeScheduler {
    fn with_days(days: Vec<String>) -> Self {
        Self { days, taken: Mutex::default(), booked: Mutex::default() }
    }

    /// The record's name of slot `i`'s visit type, or None when the hospital has no type of
    /// that kind to book.
    fn visit_type(h: &Hospital, i: usize) -> Option<String> {
        h.fake_type(FAKE_SLOTS[i].5).map(|t| t.record.clone())
    }

    /// An open slot: its type is known (and so whether it is online), its reason is not yet.
    /// Its place is the sandbox's, in the hospital's words for it.
    fn slot(&self, h: &Hospital, i: usize, visit_type: &str) -> Slot {
        let (id, _, time, minutes, clinician, _) = FAKE_SLOTS[i];
        let about = h.about(Some(visit_type), None);
        Slot {
            id: SlotId(id.to_string()),
            starts_at: Shielded::new(format!("{}T{time}", self.days[i])),
            minutes,
            clinician: Shielded::new(clinician.to_string()),
            place: Shielded::new(h.place(Some(&sandbox().place))),
            group: h.group_of(&about, Some(visit_type)),
            about,
            csn: None,
            status: VisitStatus::Upcoming,
        }
    }

    /// The visits booked on this run for `patient`.
    pub fn booked_for(&self, patient: &PatientKey) -> Vec<Slot> {
        self.booked.lock().unwrap().iter().filter(|(k, _)| k == patient).map(|(_, s)| s.clone()).collect()
    }
}

impl Scheduler for FakeScheduler {
    async fn find(&self, h: &Hospital, _: &PatientKey, wish: &SlotWish) -> Result<FoundSlots, SchedulerDown> {
        use jiff::civil::Date;
        let taken = self.taken.lock().unwrap();
        let slots = (0..FAKE_SLOTS.len())
            .filter(|&i| !taken.contains(FAKE_SLOTS[i].0))
            .filter(|&i| wish.mode.fits(FAKE_SLOTS[i].5))
            // Shielded compares without revealing. The fake's own days and times are not
            // protected: they are the fake's, until a patient books one.
            .filter(|&i| wish.earliest_day.as_ref().is_none_or(|from| Shielded::new(self.days[i].clone()) >= *from))
            .filter(|&i| wish.latest_day.as_ref().is_none_or(|to| Shielded::new(self.days[i].clone()) <= *to))
            .filter(|&i| wish.time_of_day.is_none_or(|t| t.holds(FAKE_SLOTS[i].2)))
            .filter(|&i| {
                wish.days.is_empty()
                    || self.days[i].parse::<Date>().is_ok_and(|d| wish.days.contains(&d.weekday()))
            })
            .filter_map(|i| Some(self.slot(h, i, &Self::visit_type(h, i)?)))
            .collect();
        Ok(FoundSlots { slots, next: Vec::new() })
    }

    async fn find_more(&self, _: &Hospital, _: &PatientKey, _: &[String]) -> Result<FoundSlots, SchedulerDown> {
        // The fake's search is one page.
        Ok(FoundSlots::default())
    }

    async fn book(&self, h: &Hospital, patient: &PatientKey, slot: &SlotId, note: &BookNote) -> Result<BookResult, SchedulerDown> {
        let Some(i) = FAKE_SLOTS.iter().position(|s| s.0 == slot.0) else {
            return Ok(BookResult::Taken);
        };
        // A save between the search and the booking cannot reach this call: its hospital is
        // the one it opened with. None only if the call's own list has no such type.
        let Some(visit_type) = Self::visit_type(h, i) else {
            return Ok(BookResult::Taken);
        };
        if !self.taken.lock().unwrap().insert(slot.0.clone()) {
            return Ok(BookResult::Taken);
        }
        // The visit keeps the caller's reason as its note, as Epic's `$book` will
        // (`appointmentNote`); NextVisit reads it back, through the sensitive screen.
        let mut booked = self.slot(h, i, &visit_type);
        booked.about = h.booked(Some(&visit_type), note);
        // A synthetic CSN, the same for the same slot on every run (replay cache).
        booked.csn = Some(Shielded::new(format!("900000{:02}", i + 1)));
        if booked.about.private {
            booked.group = None;
        }
        self.booked.lock().unwrap().push((patient.clone(), booked.clone()));
        Ok(BookResult::Booked(booked))
    }

    async fn visits(&self, h: &Hospital, patient: &PatientKey, ask: &VisitAsk) -> Result<FoundVisits, SchedulerDown> {
        // The visits the sandbox patients already have (`ehr/fixtures/sandbox.json`). The fake
        // has no clock: a visit's status says whether it is over.
        let sandbox = sandbox();
        let fixed = sandbox.visits.iter().filter_map(|v| {
            let status = VisitStatus::from_epic(v.status.as_deref().unwrap_or("booked"), false)
                .or_else(|| VisitStatus::from_epic(v.status.as_deref()?, true))?;
            let key = PatientKey(Shielded::new(v.patient.clone()));
            let about = h.about(v.visit_type.as_deref(), v.reason.as_deref()).with_instructions(v.instructions.as_deref());
            let group = h.group_of(&about, v.visit_type.as_deref());
            let visit = Slot {
                id: SlotId(format!("visit-{}-{}", v.patient, v.starts_at)),
                starts_at: Shielded::new(v.starts_at.clone()),
                minutes: v.minutes,
                clinician: Shielded::new(v.clinician.clone()),
                place: Shielded::new(h.place(Some(&sandbox.place))),
                about,
                csn: v.csn.clone().map(Shielded::new),
                status,
                group,
            };
            Some((key, visit))
        });
        let booked = self.booked.lock().unwrap().clone();
        // Shielded compares without revealing: the patient, the day, and which start comes first.
        let mut found: Vec<Slot> = fixed
            .chain(booked)
            .filter(|(key, _)| key == patient)
            .map(|(_, visit)| visit)
            .filter(|v| match &ask.range {
                Some(r) => v.starts_at >= r.from && v.starts_at < r.to,
                None if ask.past => v.status != VisitStatus::Upcoming,
                None => v.status == VisitStatus::Upcoming,
            })
            .collect();
        found.sort_by(|a, b| a.starts_at.cmp(&b.starts_at));
        if ask.past && ask.range.is_none() {
            found.reverse();
        }
        // The fake has no clock, so no window: every visit to come counts.
        Ok(FoundVisits { visits: found, within_days: None })
    }
}
