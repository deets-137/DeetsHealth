//! The hospital's own setup (`ehr/fixtures/hospital.json`): the values a hospital will set on
//! Config › Hospital (FUTURE-SETTINGS.md §13). Until the settings store holds them, the dev
//! hospital's drafts are built into the binary. No protected data.

use std::sync::OnceLock;

use serde::Deserialize;

const SOURCE: &str = include_str!("../fixtures/hospital.json");

#[derive(Deserialize)]
pub struct Hospital {
    pub times_of_day: Vec<TimeOfDayRow>,
    /// How many slots the agent offers at a time (FUTURE-SETTINGS.md §13.2).
    pub slots_offered: usize,
}

/// One time of day a caller may name ("morning"): its first hour and its length.
#[derive(Deserialize)]
pub struct TimeOfDayRow {
    pub name: String,
    /// HH:MM, local time at the place.
    pub from: String,
    pub hours: u8,
}

/// A time of day, as minutes after midnight and a length in minutes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimeOfDay {
    pub from_minute: u16,
    pub minutes: u16,
}

impl TimeOfDay {
    /// Whether a local "HH:MM" falls inside it.
    pub fn holds(&self, hh_mm: &str) -> bool {
        let Some(m) = minute_of(hh_mm) else { return false };
        m >= self.from_minute && m < self.from_minute + self.minutes
    }

    /// "08:00:00", for Epic's `Timing.repeat.timeOfDay`.
    pub fn start(&self) -> String {
        format!("{:02}:{:02}:00", self.from_minute / 60, self.from_minute % 60)
    }
}

fn minute_of(hh_mm: &str) -> Option<u16> {
    let (h, m) = hh_mm.split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.get(..2)?.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl Hospital {
    /// The time of day the caller named, by its name, in any case.
    pub fn time_of_day(&self, name: &str) -> Option<TimeOfDay> {
        let row = self.times_of_day.iter().find(|t| t.name.eq_ignore_ascii_case(name.trim()))?;
        Some(TimeOfDay { from_minute: minute_of(&row.from)?, minutes: u16::from(row.hours) * 60 })
    }
}

/// The hospital. A bad file fails the tests, so it never reaches a run.
pub fn hospital() -> &'static Hospital {
    static PARSED: OnceLock<Hospital> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(SOURCE).expect("ehr/fixtures/hospital.json does not parse"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_parses_and_each_time_of_day_is_whole() {
        let h = hospital();
        assert!((1..=5).contains(&h.slots_offered));
        for row in &h.times_of_day {
            let t = h.time_of_day(&row.name).expect("a time of day");
            assert!(t.minutes > 0 && t.from_minute + t.minutes <= 24 * 60);
        }
        let morning = h.time_of_day("Morning").unwrap();
        assert_eq!(morning.start(), "08:00:00");
        assert!(morning.holds("09:30") && !morning.holds("12:00"));
        assert!(h.time_of_day("midnight snack").is_none());
    }
}
