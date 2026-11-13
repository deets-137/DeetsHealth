//! The sandbox: the synthetic patients and visits the fakes serve (`ehr/fixtures/sandbox.json`).
//!
//! One file, read by three parts: the fake patient search (verify.rs), the fake scheduler's
//! visits (scheduler.rs) and the agent's dev server, which sends it to the console's Chat page
//! (CHAT.md §8). So the list the tester reads is always the list the fakes use.

use std::sync::OnceLock;

use serde::Deserialize;

/// The fixture, built into the binary.
const SOURCE: &str = include_str!("../fixtures/sandbox.json");

#[derive(Deserialize)]
pub struct Sandbox {
    pub place: String,
    pub patients: Vec<Patient>,
    pub visits: Vec<Visit>,
}

#[derive(Deserialize)]
pub struct Patient {
    pub id: String,
    pub given: String,
    pub family: String,
    /// YYYY-MM-DD.
    pub birth_date: String,
    /// The number on file that takes a text.
    pub phone: String,
}

#[derive(Deserialize)]
pub struct Visit {
    pub patient: String,
    /// YYYY-MM-DDTHH:MM, local time at the place.
    pub starts_at: String,
    pub minutes: u32,
    pub clinician: String,
    /// The visit type, as a record names it ("Office Visit"). Optional.
    #[serde(default)]
    pub visit_type: Option<String>,
    /// The reason for the visit. Optional.
    #[serde(default)]
    pub reason: Option<String>,
}

/// The sandbox. A bad fixture fails the tests, so it never reaches a run.
pub fn sandbox() -> &'static Sandbox {
    static PARSED: OnceLock<Sandbox> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(SOURCE).expect("ehr/fixtures/sandbox.json does not parse"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_parses_and_every_visit_has_its_patient() {
        let s = sandbox();
        assert!(!s.patients.is_empty());
        for v in &s.visits {
            assert!(s.patients.iter().any(|p| p.id == v.patient), "a visit for a patient not in the list");
        }
    }

    #[test]
    fn every_number_is_one_no_real_phone_uses() {
        for p in &sandbox().patients {
            let local = p.phone.strip_prefix("+1555555").expect("a +1 555-555 number");
            let n: u32 = local.parse().expect("four digits");
            assert!((100..=199).contains(&n), "555-0100 to 555-0199 only");
        }
    }
}
