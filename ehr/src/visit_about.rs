//! What a visit is: its type and its reason, as the record holds them, after the screen for
//! sensitive visits (FUTURE-SETTINGS.md §9; the owner, 2026-11-13).
//!
//! Each adapter calls [`VisitAbout::from_record`] with the plain words from its EHR, before any
//! of them is wrapped. A type or reason that names a sensitive kind of care is dropped there, so
//! it never reaches the wire, the agent or the model; the visit says only that its reason is
//! private. Its time, clinician and place are still said (an open question, §9.3).
//!
//! The scheduler's own notes (Epic's `comment`) are never read: they are for staff and can hold
//! anything (EPIC.md §5.2).

use deetshealth_core::Shielded;

/// The words that make a visit private: matched as whole words or phrases, in any case, in the
/// visit type or the reason. A draft default, the owner's to edit; a hospital adds to it, never
/// removes from it (FUTURE-SETTINGS.md §9.2).
pub const SENSITIVE: &[&str] = &[
    "behavioral health",
    "mental health",
    "psychiatry",
    "psychiatric",
    "psychology",
    "psychotherapy",
    "counseling",
    "substance",
    "addiction",
    "detox",
    "opioid",
    "methadone",
    "suboxone",
    // Not "aids" (hearing aids) or "prep" (bowel prep): too many ordinary visits hold them.
    "hiv",
    "pre-exposure",
    "sexual health",
    "std",
    "sti",
    "reproductive",
    "obstetric",
    "obstetrics",
    "ob",
    "obgyn",
    "ob/gyn",
    "prenatal",
    "pregnancy",
    "abortion",
    "family planning",
    "fertility",
    "ivf",
    "genetic",
    "genetics",
    "gender",
    "transgender",
];

/// Words in a visit type that make it an online visit (a video or phone visit, no room).
const ONLINE: &[&str] = &["video", "virtual", "telehealth", "telemedicine", "online", "e-visit"];

/// What a visit is.
#[derive(Clone, Default)]
pub struct VisitAbout {
    /// The visit type as the record names it ("Office Visit", "MyChart Video Visit").
    pub visit_type: Option<Shielded<String>>,
    /// The reason for the visit, when the record holds one.
    pub reason: Option<Shielded<String>>,
    /// An online visit: the agent says so, with the place the record names.
    pub online: bool,
    /// The type or reason named sensitive care: both are dropped, and this is set.
    pub private: bool,
}

/// The caller's reason for a visit they book: the note the scheduler gets with the booking
/// (Epic's `appointmentNote`, EPIC.md §5.2) and the reason the read-back may say. Screened once,
/// as plain words, before they are wrapped (edge.rs). The words go to the scheduler even when
/// private: the screen governs only what the agent says back on the phone.
#[derive(Clone)]
pub struct BookNote {
    pub words: Shielded<String>,
    pub private: bool,
}

impl BookNote {
    pub fn screen(words: String) -> Self {
        let private = names_any(&words, SENSITIVE);
        Self { words: Shielded::new(words), private }
    }
}

impl VisitAbout {
    /// A visit this call booked: the slot's own type, and the caller's reason from the note.
    /// A sensitive type or note makes the whole visit private, as for a visit read from the
    /// record.
    pub fn booked(visit_type: Option<&str>, note: &BookNote) -> Self {
        let about = Self::from_record(visit_type, None);
        if about.private || note.private {
            return Self { visit_type: None, reason: None, online: about.online, private: true };
        }
        Self { reason: Some(note.words.clone()), ..about }
    }

    /// From the record's plain words. Empty words count as none.
    pub fn from_record(visit_type: Option<&str>, reason: Option<&str>) -> Self {
        let clean = |s: Option<&str>| s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let (visit_type, reason) = (clean(visit_type), clean(reason));
        let online = visit_type.as_deref().is_some_and(|t| names_any(t, ONLINE));
        let private = [&visit_type, &reason].iter().any(|s| s.as_deref().is_some_and(|s| names_any(s, SENSITIVE)));
        if private {
            return Self { visit_type: None, reason: None, online, private };
        }
        Self { visit_type: visit_type.map(Shielded::new), reason: reason.map(Shielded::new), online, private }
    }
}

/// Whether `text` holds any of `terms` as whole words, in any case. "ob" matches "OB visit" and
/// "OB/GYN", never "job".
fn names_any(text: &str, terms: &[&str]) -> bool {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '/' || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    // Each word, and each word split again at "/" and "-", so "OB/GYN" holds "ob" and "ob/gyn".
    let mut parts: Vec<String> = Vec::new();
    for w in &words {
        parts.push(w.clone());
        parts.extend(w.split(['/', '-']).filter(|p| !p.is_empty() && *p != w).map(str::to_string));
    }
    let joined = format!(" {} ", words.join(" "));
    terms.iter().any(|term| {
        if term.contains(' ') {
            joined.contains(&format!(" {term} "))
        } else {
            parts.iter().any(|p| p == term)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sensitive_type_or_reason_is_dropped_and_marked_private() {
        let a = VisitAbout::from_record(Some("Behavioral Health Follow Up"), Some("anxiety"));
        assert!(a.private && a.visit_type.is_none() && a.reason.is_none());
        let b = VisitAbout::from_record(Some("Office Visit"), Some("Prenatal check"));
        assert!(b.private && b.visit_type.is_none());
        let c = VisitAbout::from_record(Some("OB/GYN Annual"), None);
        assert!(c.private);
    }

    #[test]
    fn an_ordinary_visit_keeps_its_words() {
        let a = VisitAbout::from_record(Some("New Patient Appointment"), Some("Annual physical"));
        assert!(!a.private && !a.online);
        assert!(a.visit_type.is_some() && a.reason.is_some());
        // Whole words only: "job" holds no "ob", "Stomach" no "std".
        assert!(!VisitAbout::from_record(Some("Job physical"), Some("Stomach pain")).private);
    }

    #[test]
    fn a_video_visit_is_online_and_keeps_its_type() {
        let a = VisitAbout::from_record(Some("MyChart Video Visit"), None);
        assert!(a.online && !a.private && a.visit_type.is_some());
        assert!(!VisitAbout::from_record(Some("X-Ray Exam"), None).online);
    }

    #[test]
    fn empty_words_count_as_none() {
        let a = VisitAbout::from_record(Some("  "), Some(""));
        assert!(a.visit_type.is_none() && a.reason.is_none() && !a.private);
    }
}
