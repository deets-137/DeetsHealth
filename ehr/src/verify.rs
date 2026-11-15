//! Who the caller is (SECURITY.md §4.4; the owner, 2026-10-21): a name and a date of birth,
//! then a code to the number on file. The model never checks an answer; this module does.
//!
//! Two traits, so each EHR and each text service gets its own adapter:
//! - [`Patients`] finds the candidates for a name and date of birth. The match itself is ours,
//!   in [`one_match`], the same for every EHR.
//! - [`Codes`] makes a code and sends it. [`FakeCodes`] (always 246810, nothing sent) serves the
//!   typed harness and the tests; `texts::TwilioCodes` sends a real Twilio request, to the fake
//!   Twilio in dev and to Twilio itself from Phase 5 (BUILD-ORDER.md).

use std::future::Future;

use deetshealth_core::Shielded;

use crate::{sandbox::sandbox, session::PatientKey};

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
    /// The number on file that takes a text, as +E.164. None when the chart has no such number:
    /// the patient still counts in the match (so a near-twin is never skipped), but no code
    /// goes out, and the caller is handed off as after any failed match.
    pub phone: Option<Shielded<String>>,
    /// The patient's demographics for a slot search (Epic's `$find`, EPIC.md §7.10): their
    /// `identifier`, `name` and `birthDate` as the EHR sent them, as JSON. None for the fake.
    /// Kept for the call only, in the session.
    pub facts: Option<Shielded<String>>,
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
    /// `template`: the call's text words with `{code}` (its prompt version); None for the
    /// sender's own, from the prompt file.
    fn send(
        &self,
        phone: &Shielded<String>,
        code: &Shielded<String>,
        template: Option<&str>,
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

/// The fake EHR: every sandbox patient with the claimed date of birth, as a real search by
/// date would (`ehr/fixtures/sandbox.json`). Casey Morgan and Casey Morgen share a date: the
/// match must refuse both.
#[derive(Default)]
pub struct FakePatients;

impl Patients for FakePatients {
    async fn candidates(&self, claimed: &Claimed) -> Result<Vec<Candidate>, Down> {
        Ok(sandbox()
            .patients
            .iter()
            .map(|p| Candidate {
                patient: PatientKey(Shielded::new(p.id.clone())),
                given: Shielded::new(p.given.clone()),
                family: Shielded::new(p.family.clone()),
                birth_date: Shielded::new(p.birth_date.clone()),
                phone: Some(Shielded::new(p.phone.clone())),
                facts: None,
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

    async fn send(&self, _: &Shielded<String>, _: &Shielded<String>, _: Option<&str>) -> Result<(), Down> {
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
