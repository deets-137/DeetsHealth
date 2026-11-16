//! The wrapper type for protected data (SECURITY.md §4.2, CORE-CRATE.md §4).
//!
//! `Shielded<T>` has no `Debug`, `Display`, `Serialize` or `ToString`, so a log line, a panic
//! message or a JSON reply that tries to carry one does not compile. The plaintext comes out
//! only through [`Shielded::reveal`], which needs a [`Pass`]. Exactly four pass types exist,
//! and this file names all four: [`crate::store::CachePass`] (the sealed cache writer),
//! [`crate::payload::CardPass`] (the card payload), [`crate::net::SitePass`] (a request
//! to the site the value came from, and the parse of its reply; the owner, 2026-09-21) and
//! [`crate::wire::WirePass`] (the `ehr` service's reply to the agent; 2026-10-21). A fifth,
//! [`crate::text::TextPass`], is the `ehr` service's text message: the code to the number on
//! file (2026-11-12). A sixth, [`crate::ehr_request::EhrPass`], is the `ehr` service's request
//! to an EHR: the caller's name and date of birth in a patient search, the patient's id in a
//! chart read (2026-11-12). The first three are built only inside their own modules; the fourth
//! only in `ehr/src/edge.rs`, the fifth only in `ehr/src/texts.rs` and the sixth only in
//! `ehr/src/epic_ehr.rs`, which tests in `ehr` enforce. The value is wiped from memory when it drops.
//!
//! A log line cannot print one:
//! ```compile_fail
//! let name = deetshealth_core::Shielded::new(String::from("a name"));
//! println!("{:?}", name);
//! ```
//! Nor can a JSON reply carry one:
//! ```compile_fail
//! fn to_front_end<T: serde::Serialize>(_: &T) {}
//! to_front_end(&deetshealth_core::Shielded::new(String::from("a name")));
//! ```
//! Nor can code outside the core make a pass:
//! ```compile_fail
//! let pass = deetshealth_core::store::CachePass { _only_here: () };
//! ```

use std::cmp::Ordering;
use zeroize::Zeroize;

/// A protected value. Wrap it the moment it is parsed; it stays wrapped until the cache
/// writer seals it or a card payload shows it.
pub struct Shielded<T: Zeroize>(T);

impl<T: Zeroize> Shielded<T> {
    /// Wrap a value just parsed from the site or just unsealed from the cache.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// The plaintext, for the two holders of a pass only.
    pub fn reveal<P: Pass>(&self, _pass: &P) -> &T {
        &self.0
    }
}

impl<T: Zeroize> Drop for Shielded<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// A copy stays shielded, and is wiped on its own drop.
impl<T: Zeroize + Clone> Clone for Shielded<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Comparison, so core can filter and sort in Rust (no protected column is plain in SQLite,
// CORE-CRATE.md §5). A comparison answers "same?" or "which first?", never "what is it?".
impl<T: Zeroize + PartialEq> PartialEq for Shielded<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: Zeroize + Eq> Eq for Shielded<T> {}
impl<T: Zeroize + PartialOrd> PartialOrd for Shielded<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<T: Zeroize + Ord> Ord for Shielded<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: Zeroize + AsRef<[u8]>> Shielded<T> {
    /// "Same secret?" in constant time, for a value someone may guess at (the text code). The
    /// time it takes depends on the length only, never on where the first difference is, so
    /// the timing tells a guesser nothing (`subtle`). Use it for every check of a secret; `==`
    /// stays for sorting and matching records.
    pub fn ct_eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        self.0.as_ref().ct_eq(other.0.as_ref()).into()
    }
}

impl Shielded<String> {
    /// "Is this the same name, allowing for a slip?" Like the comparisons above, it answers
    /// yes or no and never reveals either value. Case, spaces and punctuation are ignored;
    /// a name of 5 letters or more may differ by one letter (speech-to-text spells names
    /// loosely; the owner's identification rule, SECURITY.md §4.4). A shorter one must match.
    pub fn same_name(&self, other: &Self) -> bool {
        let norm = |s: &str| -> Vec<char> {
            s.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect()
        };
        let (a, b) = (norm(&self.0), norm(&other.0));
        if a.is_empty() || b.is_empty() {
            return false;
        }
        if a == b {
            return true;
        }
        a.len().min(b.len()) >= 5 && within_one_edit(&a, &b)
    }

    /// "Do the caller's words name this clinician?" (`self` is the caller's words, `display` the
    /// record's name, such as "Avery Lin, MD"). Titles ("Dr.", "doctor") are left out; every other
    /// word the caller said must match a word of the name, with [`Shielded::same_name`]'s slip.
    /// Answers yes or no and never reveals either value (EPIC.md §7.10).
    pub fn names_clinician(&self, display: &Self) -> bool {
        const TITLES: &[&str] = &["dr", "doctor", "md", "do", "np", "pa", "nurse", "practitioner"];
        let words = |s: &str| -> Vec<String> {
            s.split(|c: char| !c.is_alphabetic())
                .map(str::to_lowercase)
                .filter(|w| !w.is_empty() && !TITLES.contains(&w.as_str()))
                .collect()
        };
        let (asked, name) = (words(&self.0), words(&display.0));
        !asked.is_empty()
            && asked.iter().all(|a| {
                name.iter().any(|n| Shielded::new(a.clone()).same_name(&Shielded::new(n.clone())))
            })
    }

    /// "Is this time on an earlier day than that one?" Both are local times that start with the
    /// day, YYYY-MM-DD (a slot's start). Answers yes or no and never reveals either value
    /// (BOOKING-RULES.md §3.3: a sooner time with another clinician).
    pub fn earlier_day(&self, other: &Self) -> bool {
        match (self.0.get(..10), other.0.get(..10)) {
            (Some(a), Some(b)) => a < b,
            _ => false,
        }
    }

    /// The first of `terms` that the caller's words name, by whole words in any case ("my surgery
    /// visit" names "surgery"; "primary care" names "primary care"). Used for a visit group,
    /// whose names are the hospital's, not protected data. Never reveals the caller's words.
    pub fn first_named(&self, terms: &[&str]) -> Option<usize> {
        let norm = |s: &str| -> String {
            let words: Vec<String> =
                s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
            format!(" {} ", words.join(" "))
        };
        let said = norm(&self.0);
        terms.iter().position(|t| {
            let t = norm(t);
            t.trim() != "" && said.contains(&t)
        })
    }

    /// Every one of `terms` that the caller's words name, for a place or a specialty on the
    /// hospital's lists (BOOKING-RULES.md §3.1). The words that name no one place are left out
    /// ("the", "clinic", "doctor"). A term whose every word the caller said ("my heart doctor"
    /// names "heart doctor") is a full match; when there is one, only full matches count. Else a
    /// term that holds every word the caller said ("north" names "EMC North Family Medicine").
    /// The terms are the hospital's, not protected data. Answers with positions only and never
    /// reveals the caller's words.
    pub fn names_terms(&self, terms: &[&str]) -> Vec<usize> {
        const GENERIC: &[&str] = &[
            "a", "an", "the", "my", "our", "your", "at", "in", "on", "of", "to", "for", "near", "by", "with", "and", "or", "one",
            "please", "clinic", "clinics", "office", "center", "centre", "hospital", "department", "location", "building", "campus",
            "doctor", "doctors", "dr", "specialist", "physician", "provider", "care", "visit", "appointment", "place", "site", "s",
        ];
        let words = |s: &str| -> Vec<String> {
            s.split(|c: char| !c.is_alphanumeric())
                .map(str::to_lowercase)
                .filter(|w| !w.is_empty() && !GENERIC.contains(&w.as_str()))
                .collect()
        };
        let said = words(&self.0);
        if said.is_empty() {
            return Vec::new();
        }
        let terms: Vec<Vec<String>> = terms.iter().map(|t| words(t)).collect();
        let pick = |fits: &dyn Fn(&[String]) -> bool| -> Vec<usize> {
            terms.iter().enumerate().filter(|(_, t)| !t.is_empty() && fits(t)).map(|(i, _)| i).collect()
        };
        let full = pick(&|t| t.iter().all(|w| said.contains(w)));
        if !full.is_empty() {
            return full;
        }
        pick(&|t| said.iter().all(|w| t.contains(w)))
    }

    /// Whether the caller's words hold any word that could name a place or a specialty (not only
    /// "the clinic" or "a doctor"): with none, the words name nothing and are left out.
    pub fn names_anything(&self) -> bool {
        !self.names_terms(&[self.0.as_str()]).is_empty()
    }
}

/// True when one insertion, deletion or substitution turns `a` into `b`.
fn within_one_edit(a: &[char], b: &[char]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if long.len() - short.len() > 1 {
        return false;
    }
    let first_diff = short.iter().zip(long).position(|(x, y)| x != y).unwrap_or(short.len());
    if short.len() == long.len() {
        short[first_diff + 1..] == long[first_diff + 1..]
    } else {
        short[first_diff..] == long[first_diff + 1..]
    }
}

mod sealed {
    /// Only this file can implement it, so only this file decides who holds a pass.
    pub trait Sealed {}
}

/// Proof that the caller may read plaintext. Implemented for exactly six types, below, and a
/// sixth, `dev::DevPass`, only in a build with the `dev` feature (never a release).
pub trait Pass: sealed::Sealed {}

impl sealed::Sealed for crate::store::CachePass {}
impl Pass for crate::store::CachePass {}
impl sealed::Sealed for crate::payload::CardPass {}
impl Pass for crate::payload::CardPass {}
impl sealed::Sealed for crate::net::SitePass {}
impl Pass for crate::net::SitePass {}
// The fourth: the `ehr` service's wire edge (the owner, 2026-10-21; EHR-SERVICE.md §5).
impl sealed::Sealed for crate::wire::WirePass {}
impl Pass for crate::wire::WirePass {}
// The `ehr` service's text message: the code to the number on file (2026-11-12; CHAT.md §8).
impl sealed::Sealed for crate::text::TextPass {}
impl Pass for crate::text::TextPass {}
// The `ehr` service's request to an EHR: a patient search, a chart read (2026-11-12; EPIC.md §5.2).
impl sealed::Sealed for crate::ehr_request::EhrPass {}
impl Pass for crate::ehr_request::EhrPass {}
#[cfg(feature = "dev")]
impl sealed::Sealed for crate::dev::DevPass {}
#[cfg(feature = "dev")]
impl Pass for crate::dev::DevPass {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::CachePass;

    #[test]
    fn reveal_needs_a_pass_and_gives_the_value() {
        let v = Shielded::new(String::from("sandbox value"));
        assert_eq!(v.reveal(&CachePass::for_tests()), "sandbox value");
    }

    #[test]
    fn sorts_without_revealing() {
        let mut v = [Shielded::new(3u32), Shielded::new(1), Shielded::new(2)];
        v.sort();
        let pass = CachePass::for_tests();
        let order: Vec<u32> = v.iter().map(|s| *s.reveal(&pass)).collect();
        assert_eq!(order, [1, 2, 3]);
    }

    #[test]
    fn same_name_allows_one_slip_in_a_long_name_only() {
        let n = |s: &str| Shielded::new(s.to_string());
        assert!(n("Jordan").same_name(&n("jordan")));
        assert!(n("O'Brien").same_name(&n("obrien")));
        assert!(n("Jordan").same_name(&n("Jordon")));
        assert!(n("Raman").same_name(&n("Ramann")));
        assert!(!n("Jordan").same_name(&n("Jardon")), "two slips");
        assert!(!n("Lee").same_name(&n("Lea")), "short names must match");
        assert!(!n("").same_name(&n("")), "empty never matches");
    }

    #[test]
    fn a_clinician_is_named_by_their_own_words_without_titles() {
        let n = |s: &str| Shielded::new(s.to_string());
        assert!(n("Dr. Lin").names_clinician(&n("Avery Lin, MD")));
        assert!(n("doctor avery lin").names_clinician(&n("Dr. Avery Lin")));
        assert!(n("Okafur").names_clinician(&n("Dr. Sam Okafor")), "one slip in a long name");
        assert!(!n("Dr. Lin").names_clinician(&n("Dr. Sam Okafor")));
        assert!(!n("the doctor").names_clinician(&n("Dr. Sam Okafor")), "no name word left");
    }

    #[test]
    fn first_named_finds_a_term_by_whole_words() {
        let n = |s: &str| Shielded::new(s.to_string());
        let terms = ["primary care", "imaging", "x-ray"];
        assert_eq!(n("my primary care visit").first_named(&terms), Some(0));
        assert_eq!(n("the X-ray one").first_named(&terms), Some(2));
        assert_eq!(n("primary").first_named(&terms), None);
        assert_eq!(n("imagingx").first_named(&terms), None);
    }

    #[test]
    fn names_terms_prefers_a_full_match_and_skips_words_that_name_nothing() {
        let n = |s: &str| Shielded::new(s.to_string());
        let places = ["EMC Cardiology", "EMC Cardiology Procedures", "Main Street Clinic"];
        assert_eq!(n("EMC cardiology").names_terms(&places), [0], "a full match only");
        assert_eq!(n("the cardiology clinic").names_terms(&places), [0, 1], "a word in both");
        assert_eq!(n("main street").names_terms(&places), [2]);
        assert_eq!(n("my heart doctor").names_terms(&["heart doctor", "skin doctor"]), [0]);
        assert!(n("the clinic").names_terms(&places).is_empty(), "no word left");
        assert!(!n("a doctor").names_anything());
        assert!(n("Verona").names_anything());
    }

    #[test]
    fn ct_eq_answers_same_or_not() {
        let c = |s: &str| Shielded::new(s.to_string());
        assert!(c("246810").ct_eq(&c("246810")));
        assert!(!c("246810").ct_eq(&c("246811")), "last digit differs");
        assert!(!c("246810").ct_eq(&c("146810")), "first digit differs");
        assert!(!c("246810").ct_eq(&c("24681")), "lengths differ");
    }

    #[test]
    fn a_clone_is_equal_and_separate() {
        let a = Shielded::new(String::from("x"));
        let b = a.clone();
        drop(a);
        assert_eq!(b.reveal(&CachePass::for_tests()), "x");
    }
}
