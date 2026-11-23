//! A plan's billing cycle (VOICE-VENDORS.md §9, §10): the day it began and the day it renews,
//! when the plan renews on a day of the month. Shared by the voice and the model.

/// A day of the calendar, `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

impl Day {
    pub fn parse(s: &str) -> Option<Day> {
        let mut it = s.get(..10)?.split('-');
        let (y, m, d) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
        ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some(Day { y, m, d })
    }

    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

/// The first day of the cycle that holds `today`, when the plan renews on `day` (1 to 28, so
/// every month has it).
pub fn cycle_start(today: Day, day: u32) -> Day {
    let day = day.clamp(1, 28);
    if today.d >= day {
        Day { d: day, ..today }
    } else if today.m == 1 {
        Day { y: today.y - 1, m: 12, d: day }
    } else {
        Day { m: today.m - 1, d: day, ..today }
    }
}

/// The day the cycle that began on `start` renews.
pub fn next_cycle(start: Day) -> Day {
    if start.m == 12 {
        Day { y: start.y + 1, m: 1, ..start }
    } else {
        Day { m: start.m + 1, ..start }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn the_cycle() {
        for (today, day, start, renews) in [
            ("2026-11-13", 1, "2026-11-01", "2026-12-01"),
            ("2026-11-13", 20, "2026-10-20", "2026-11-20"),
            ("2026-01-05", 20, "2025-12-20", "2026-01-20"),
            ("2026-12-20", 20, "2026-12-20", "2027-01-20"),
        ] {
            let s = cycle_start(d(today), day);
            assert_eq!((s.iso(), next_cycle(s).iso()), (start.to_string(), renews.to_string()));
        }
    }
}
