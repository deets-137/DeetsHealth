//! Finding one of the caller's visits (EPIC.md §7.10; the owner's V1A, 2026-11-14).
//!
//! An adapter returns the visits that match the day, month or past the caller asked for. This
//! module, shared by every EHR, narrows them: by the clinician the caller named, by a visit
//! group, and when too many are left, by asking which group the caller means. Its numbers, the
//! groups, how far back and the search windows are the call's hospital (Config › Hospital,
//! hospital.rs; FUTURE-SETTINGS.md §13.3). The Default windows are 7, then 30, then 90 days
//! (the owner, 2026-11-15): a small first window keeps the reply small for a patient with many
//! visits (the sandbox's 7 days take 0.5 s, 90 days 1.3 s; EPIC.md §7.10).

use deetshealth_core::Shielded;
use serde::{Deserialize, Serialize};

use crate::{scheduler::Slot, scheduler::VisitStatus, visit_about::names_sensitive};

/// When the narrowing question is asked, and how many groups it names (the hospital's).
#[derive(Clone, Copy)]
pub struct Limits {
    /// More matches than this, and no group asked for: the caller is asked which group.
    pub narrow_over: usize,
    /// The most groups one question names; the rest are "another visit".
    pub groups_named: usize,
}

impl Default for Limits {
    /// The Default version's numbers.
    fn default() -> Self {
        Self { narrow_over: 2, groups_named: 3 }
    }
}

/// One visit group: a name the caller hears ("primary care"), the words a caller may say for it
/// ("my checkup"), and its members by visit type, as the record names them. The hospital's own
/// list (HOSPITAL-CONFIG.md): never protected data.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VisitGroup {
    pub name: String,
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub visit_types: Vec<String>,
}

/// The hospital's groups, without any whose name or words name sensitive care: such a name
/// would say what a visit is for (FUTURE-SETTINGS.md §9).
#[derive(Clone, Default)]
pub struct VisitGroups(Vec<VisitGroup>);

impl VisitGroups {
    pub fn new(list: &[VisitGroup]) -> Self {
        Self(
            list.iter()
                .filter(|g| !g.name.trim().is_empty())
                .filter(|g| !names_sensitive(&g.name) && !g.words.iter().any(|w| names_sensitive(w)))
                .cloned()
                .collect(),
        )
    }

    /// The group of a visit type, from the record's plain words, before they are wrapped. A
    /// private visit gets none: the adapter never asks for it.
    pub fn of(&self, visit_type: Option<&str>) -> Option<Shielded<String>> {
        let t = visit_type?.trim();
        self.0
            .iter()
            .find(|g| g.visit_types.iter().any(|m| m.trim().eq_ignore_ascii_case(t)))
            .map(|g| Shielded::new(g.name.clone()))
    }

    /// The group the caller's words name, by its name or one of its words.
    pub fn named_by(&self, said: &Shielded<String>) -> Option<Shielded<String>> {
        let terms: Vec<(&str, usize)> = self
            .0
            .iter()
            .enumerate()
            .flat_map(|(i, g)| std::iter::once((g.name.as_str(), i)).chain(g.words.iter().map(move |w| (w.as_str(), i))))
            .collect();
        let words: Vec<&str> = terms.iter().map(|t| t.0).collect();
        said.first_named(&words).map(|k| Shielded::new(self.0[terms[k].1].name.clone()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What the caller asked about, past what the adapter already searched.
pub struct Narrow<'a> {
    pub clinician: Option<&'a Shielded<String>>,
    /// The caller named a group, and it is one of the hospital's.
    pub group: Option<Shielded<String>>,
    /// The caller named a group that is not one of the hospital's.
    pub unknown_group: bool,
    /// The caller asked for a day or a month: a cancelled visit counts only when no other does.
    pub by_day: bool,
}

impl Narrow<'_> {
    /// The caller named something (a day, a month, a clinician, a group we do not have). With
    /// nothing named, "my next visit" means the soonest, never a question.
    fn named(&self) -> bool {
        self.by_day || self.clinician.is_some() || self.unknown_group
    }
}

// One answer per call, never stored in bulk: the size of the visit does not matter.
#[allow(clippy::large_enum_variant)]
pub enum Found {
    None,
    /// The visit, and how many others matched.
    Visit { visit: Slot, more: u32 },
    /// Too many matched: the groups to name, in order, and whether others are left out.
    Which { groups: Vec<Shielded<String>>, other: bool },
}

/// Narrows the adapter's visits, which come in the order they are said (soonest first; newest
/// first for the past).
pub fn narrow(mut found: Vec<Slot>, ask: &Narrow, groups: &VisitGroups, limits: Limits) -> Found {
    let Limits { narrow_over, groups_named } = limits;
    if let Some(who) = ask.clinician {
        found.retain(|v| who.names_clinician(&v.clinician));
    }
    if let Some(g) = &ask.group {
        found.retain(|v| v.group.as_ref() == Some(g));
    }
    if ask.by_day && found.iter().any(|v| v.status != VisitStatus::Cancelled) {
        found.retain(|v| v.status != VisitStatus::Cancelled);
    }
    if found.is_empty() {
        return Found::None;
    }
    if ask.group.is_none() && ask.named() && !groups.is_empty() && (found.len() > narrow_over || ask.unknown_group) {
        let mut named: Vec<Shielded<String>> = Vec::new();
        for g in found.iter().filter_map(|v| v.group.as_ref()) {
            if !named.contains(g) {
                named.push(g.clone());
            }
        }
        let other = found.iter().any(|v| v.group.as_ref().is_none_or(|g| !named[..named.len().min(groups_named)].contains(g)));
        named.truncate(groups_named);
        // One group only, with nothing else: the question would have one answer.
        if named.len() >= 2 || (named.len() == 1 && other) {
            return Found::Which { groups: named, other };
        }
    }
    let more = (found.len() - 1) as u32;
    // The first is the one said; the order of the rest no longer matters, only their count.
    Found::Visit { visit: found.swap_remove(0), more }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scheduler::SlotId,
        visit_about::VisitAbout,
    };

    fn visit(n: u32, clinician: &str, group: Option<&str>, status: VisitStatus) -> Slot {
        Slot {
            id: SlotId(format!("v{n}")),
            starts_at: Shielded::new(format!("2026-11-{:02}T09:00", n)),
            minutes: 20,
            clinician: Shielded::new(clinician.to_string()),
            place: Shielded::new("Main Street Clinic".to_string()),
            about: VisitAbout::default(),
            csn: None,
            status,
            group: group.map(|g| Shielded::new(g.to_string())),
            place_ref: Default::default(),
            kind: Default::default(),
            visit_type_ref: None,
            clinician_ref: Default::default(),
        }
    }

    fn groups() -> VisitGroups {
        VisitGroups::new(&[
            VisitGroup { name: "primary care".into(), words: vec!["checkup".into()], visit_types: vec!["Office Visit".into()] },
            VisitGroup { name: "imaging".into(), words: vec!["x-ray".into()], visit_types: vec!["X-Ray Exam".into()] },
            VisitGroup { name: "behavioral health".into(), words: vec![], visit_types: vec!["Therapy".into()] },
        ])
    }

    fn ask() -> Narrow<'static> {
        Narrow { clinician: None, group: None, unknown_group: false, by_day: false }
    }

    #[test]
    fn a_sensitive_group_is_dropped_from_the_list() {
        let g = groups();
        assert!(g.of(Some("Therapy")).is_none());
        assert!(g.of(Some("office visit")).is_some());
        assert!(g.named_by(&Shielded::new("my checkup".into())).is_some());
        assert!(g.named_by(&Shielded::new("my therapy".into())).is_none());
    }

    #[test]
    fn few_visits_give_the_soonest_and_a_count() {
        let found = vec![visit(1, "Dr. Lin", Some("primary care"), VisitStatus::Upcoming), visit(2, "Dr. Okafor", Some("imaging"), VisitStatus::Upcoming)];
        let Found::Visit { visit: v, more } = narrow(found, &ask(), &groups(), Limits::default()) else { panic!("a visit") };
        assert_eq!(v.id.0, "v1");
        assert_eq!(more, 1);
    }

    #[test]
    fn many_visits_in_two_groups_ask_which_when_the_caller_named_a_month() {
        let month = Narrow { by_day: true, ..ask() };
        let found = || vec![
            visit(1, "Dr. Lin", Some("primary care"), VisitStatus::Upcoming),
            visit(2, "Dr. Lin", Some("primary care"), VisitStatus::Upcoming),
            visit(3, "Dr. Okafor", Some("imaging"), VisitStatus::Upcoming),
        ];
        // "My next visit": the soonest, never a question.
        assert!(matches!(narrow(found(), &ask(), &groups(), Limits::default()), Found::Visit { more: 2, .. }));
        let Found::Which { groups: named, other } = narrow(found(), &month, &groups(), Limits::default()) else { panic!("a question") };
        assert_eq!(named.len(), 2);
        assert!(named[0] == Shielded::new("primary care".to_string()));
        assert!(!other);
    }

    #[test]
    fn a_private_visit_has_no_group_and_is_another_visit() {
        let found = vec![
            visit(1, "Dr. Lin", None, VisitStatus::Upcoming),
            visit(2, "Dr. Lin", Some("primary care"), VisitStatus::Upcoming),
            visit(3, "Dr. Lin", Some("primary care"), VisitStatus::Upcoming),
        ];
        let month = Narrow { by_day: true, ..ask() };
        let Found::Which { groups: named, other } = narrow(found, &month, &groups(), Limits::default()) else { panic!("a question") };
        assert_eq!(named.len(), 1);
        assert!(other);
    }

    #[test]
    fn a_group_or_a_clinician_narrows_to_one() {
        let found = || {
            vec![
                visit(1, "Dr. Avery Lin", Some("primary care"), VisitStatus::Upcoming),
                visit(2, "Dr. Avery Lin", Some("primary care"), VisitStatus::Upcoming),
                visit(3, "Dr. Sam Okafor", Some("imaging"), VisitStatus::Upcoming),
            ]
        };
        let imaging = Narrow { group: Some(Shielded::new("imaging".into())), ..ask() };
        let Found::Visit { visit: v, more } = narrow(found(), &imaging, &groups(), Limits::default()) else { panic!("a visit") };
        assert_eq!((v.id.0.as_str(), more), ("v3", 0));
        let who = Shielded::new("Dr. Okafor".to_string());
        let by_name = Narrow { clinician: Some(&who), ..ask() };
        let Found::Visit { visit: v, .. } = narrow(found(), &by_name, &groups(), Limits::default()) else { panic!("a visit") };
        assert_eq!(v.id.0, "v3");
    }

    #[test]
    fn no_groups_means_no_question() {
        let found = (1..=5).map(|n| visit(n, "Dr. Lin", None, VisitStatus::Upcoming)).collect();
        assert!(matches!(narrow(found, &ask(), &VisitGroups::default(), Limits::default()), Found::Visit { more: 4, .. }));
    }

    #[test]
    fn on_a_day_a_cancelled_visit_counts_only_alone() {
        let day = Narrow { by_day: true, ..ask() };
        let found = vec![visit(1, "Dr. Lin", None, VisitStatus::Cancelled), visit(2, "Dr. Lin", None, VisitStatus::Seen)];
        let Found::Visit { visit: v, more } = narrow(found, &day, &VisitGroups::default(), Limits::default()) else { panic!("a visit") };
        assert_eq!((v.id.0.as_str(), more), ("v2", 0));
        let alone = vec![visit(1, "Dr. Lin", None, VisitStatus::Cancelled)];
        assert!(matches!(narrow(alone, &day, &VisitGroups::default(), Limits::default()), Found::Visit { .. }));
    }
}
