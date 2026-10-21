//! Who the caller is (SECURITY.md §4.4; the owner, 2026-10-21): a name and a date of birth,
//! then a code to the number on file. The model never checks an answer; this module does.
//!
//! Two traits, so each EHR and each text service gets its own adapter:
//! - [`Patients`] finds the candidates for a name and date of birth. The match itself is ours,
//!   in [`one_match`], the same for every EHR.
//! - [`Codes`] makes a code and sends it. Phase 1 has the fake only; the text message comes
//!   with the real phone network (BUILD-ORDER.md, Phase 5).

use std::future::Future;

use deetshealth_core::Shielded;

use crate::session::PatientKey;

/// What the caller said.
pub struct Claimed {
    pub given: Shielded<String>,
    pub family: Shielded<String>,
    /// YYYY-MM-DD.
    pub birth_date: Shielded<String>,
}

/// One patient the EHR returned for a search.
pub struct Candidate {
    pub patient: PatientKey,
    pub given: Shielded<String>,
    pub family: Shielded<String>,
    pub birth_date: Shielded<String>,
    /// The number on file that takes a text.
    pub phone: Shielded<String>,
}

/// The EHR or the text service did not answer. Never carries text from them.
pub struct Down;

pub trait Patients: Send + Sync + 'static {
    /// The patients the EHR finds for this claim. It may return too many; [`one_match`]
    /// decides.
    fn candidates(
        &self,
        claimed: &Claimed,
    ) -> impl Future<Output = Result<Vec<Candidate>, Down>> + Send;
}

pub trait Codes: Send + Sync + 'static {
    /// A new code: six digits.
    fn issue(&self) -> Shielded<String>;
    fn send(
        &self,
        phone: &Shielded<String>,
        code: &Shielded<String>,
    ) -> impl Future<Output = Result<(), Down>> + Send;
}

/// The one candidate that matches the claim, or none. Two or more that match is none: a code
/// can go to one number only, and guessing between patients is never safe.
pub fn one_match(claimed: &Claimed, found: Vec<Candidate>) -> Option<Candidate> {
    let mut hits = found.into_iter().filter(|c| {
        c.birth_date == claimed.birth_date
            && c.given.same_name(&claimed.given)
            && c.family.same_name(&claimed.family)
    });
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}

/// Synthetic patients for tests and the text harness. The numbers are in 555-0100 to 555-0199,
/// which no real phone uses.
const FAKE_PATIENTS: &[(&str, &str, &str, &str, &str)] = &[
    // (id, given, family, birth date, phone)
    ("fake-patient-1", "Jordan", "Ellis", "1984-03-12", "+15555550101"),
    ("fake-patient-2", "Priya", "Raman", "1971-11-30", "+15555550102"),
    // Two near-identical names on one date: the match must refuse both.
    ("fake-patient-3", "Casey", "Morgan", "1990-06-05", "+15555550103"),
    ("fake-patient-4", "Casey", "Morgen", "1990-06-05", "+15555550104"),
];

/// The fake EHR: every patient with the claimed date of birth, as a real search by date would.
#[derive(Default)]
pub struct FakePatients;

impl Patients for FakePatients {
    async fn candidates(&self, claimed: &Claimed) -> Result<Vec<Candidate>, Down> {
        Ok(FAKE_PATIENTS
            .iter()
            .map(|&(id, given, family, dob, phone)| Candidate {
                patient: PatientKey(Shielded::new(id.to_string())),
                given: Shielded::new(given.to_string()),
                family: Shielded::new(family.to_string()),
                birth_date: Shielded::new(dob.to_string()),
                phone: Shielded::new(phone.to_string()),
            })
            .filter(|c| c.birth_date == claimed.birth_date)
            .collect())
    }
}

/// The fake code (BUILD-ORDER.md, Phase 1: "C3's second factor is a fake code"). Always the
/// same, so a typed call can use it and the replay cache stays valid. Nothing is sent.
pub const FAKE_CODE: &str = "246810";

#[derive(Default)]
pub struct FakeCodes;

impl Codes for FakeCodes {
    fn issue(&self) -> Shielded<String> {
        Shielded::new(FAKE_CODE.to_string())
    }

    async fn send(&self, _: &Shielded<String>, _: &Shielded<String>) -> Result<(), Down> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(given: &str, family: &str, dob: &str) -> Claimed {
        Claimed {
            given: Shielded::new(given.into()),
            family: Shielded::new(family.into()),
            birth_date: Shielded::new(dob.into()),
        }
    }

    async fn matched(c: &Claimed) -> bool {
        one_match(c, FakePatients.candidates(c).await.ok().unwrap()).is_some()
    }

    #[tokio::test]
    async fn a_name_and_birth_date_find_one_patient() {
        assert!(matched(&claim("jordan", "ellis", "1984-03-12")).await);
        assert!(matched(&claim("Priya", "Ramann", "1971-11-30")).await, "one slip in a long name");
    }

    #[tokio::test]
    async fn a_wrong_date_or_name_finds_no_one() {
        assert!(!matched(&claim("Jordan", "Ellis", "1984-03-13")).await);
        assert!(!matched(&claim("Jordan", "Smith", "1984-03-12")).await);
    }

    #[tokio::test]
    async fn two_close_names_on_one_date_find_no_one() {
        assert!(!matched(&claim("Casey", "Morgan", "1990-06-05")).await);
    }
}
