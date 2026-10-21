//! Booking, behind one trait (VOICE-AGENT.md §8.4): each EHR gets an adapter. Phase 1 has the
//! in-memory fake only. Phase 2 adds Epic behind the same trait.
//!
//! The fake is deterministic: fixed slots on fixed days, no clock. A tool result that changed
//! from run to run would change the prompt and miss the replay cache (EHR-SERVICE.md §7).

use std::{collections::HashSet, future::Future, sync::Mutex};

use deetshealth_core::Shielded;

use crate::session::PatientKey;

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
}

/// What the caller asked for.
pub struct SlotWish {
    pub reason: Shielded<String>,
    /// YYYY-MM-DD; none means from the next open day.
    pub earliest_day: Option<Shielded<String>>,
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

    fn book(
        &self,
        patient: &PatientKey,
        slot: &SlotId,
    ) -> impl Future<Output = Result<BookResult, SchedulerDown>> + Send;
}

/// The fake's fixed slots: synthetic clinicians at a synthetic clinic.
const FAKE_SLOTS: &[(&str, &str, &str, u32, &str)] = &[
    // (id, day, time, minutes, clinician)
    ("slot-01", "2026-11-02", "09:00", 20, "Dr. Avery Lin"),
    ("slot-02", "2026-11-02", "14:30", 20, "Dr. Avery Lin"),
    ("slot-03", "2026-11-03", "10:00", 30, "Dr. Sam Okafor"),
    ("slot-04", "2026-11-04", "08:30", 20, "Dr. Avery Lin"),
    ("slot-05", "2026-11-05", "11:00", 30, "Dr. Sam Okafor"),
    ("slot-06", "2026-11-06", "15:00", 20, "Dr. Avery Lin"),
];
const FAKE_PLACE: &str = "Main Street Clinic";

/// The in-memory scheduler for tests and the text harness (VOICE-AGENT.md §9).
#[derive(Default)]
pub struct FakeScheduler {
    taken: Mutex<HashSet<String>>,
}

impl FakeScheduler {
    fn slot(&self, i: usize) -> Slot {
        let (id, day, time, minutes, clinician) = FAKE_SLOTS[i];
        Slot {
            id: SlotId(id.to_string()),
            starts_at: Shielded::new(format!("{day}T{time}")),
            minutes,
            clinician: Shielded::new(clinician.to_string()),
            place: Shielded::new(FAKE_PLACE.to_string()),
        }
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
                    .is_none_or(|from| Shielded::new(FAKE_SLOTS[i].1.to_string()) >= *from)
            })
            .take(MAX_OFFERED)
            .map(|i| self.slot(i))
            .collect())
    }

    async fn book(&self, _: &PatientKey, slot: &SlotId) -> Result<BookResult, SchedulerDown> {
        let Some(i) = FAKE_SLOTS.iter().position(|s| s.0 == slot.0) else {
            return Ok(BookResult::Taken);
        };
        if !self.taken.lock().unwrap().insert(slot.0.clone()) {
            return Ok(BookResult::Taken);
        }
        Ok(BookResult::Booked(self.slot(i)))
    }
}
