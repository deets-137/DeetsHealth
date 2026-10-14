//! The date key (GLOBALS.md §5.4): one sortable key per record.
//!
//! A FHIR date is text with its own precision (`2019`, `2019-04`, a full time with an offset).
//! Text order is not time order when offsets differ, and the engine indexes a canonical number
//! (`2019`) as a number, which sorts before every string. So every key is a full UTC instant,
//! `YYYY-MM-DDTHH:MM:SSZ`, and the precision is kept beside it.

/// The key of a record with no date at all: first in time, so a hub that lists newest first
/// shows it last.
pub(crate) const NO_DATE: &str = "0000-01-01T00:00:00Z";

/// A date as a key, and what the site sent: `year`, `month`, `day` or `time`.
pub(crate) struct DateKey {
    pub key: String,
    pub precision: &'static str,
}

/// The key for a FHIR `date`, `dateTime` or `instant`. A partial date takes the start of its
/// period. A time with an offset is turned into UTC; a date with no time is not shifted.
/// `None` for text that is not a FHIR date.
pub(crate) fn key(s: &str) -> Option<DateKey> {
    let s = s.trim();
    let b = s.as_bytes();
    let num = |from: usize, to: usize| -> Option<i64> {
        let t = s.get(from..to)?;
        (!t.is_empty() && t.bytes().all(|c| c.is_ascii_digit())).then(|| t.parse().ok()).flatten()
    };
    let year = num(0, 4)?;
    if year == 0 {
        return None;
    }
    if s.len() == 4 {
        return Some(make(year, 1, 1, 0, 0, 0, "year"));
    }
    let month = (b.get(4) == Some(&b'-')).then(|| num(5, 7)).flatten().filter(|m| (1..=12).contains(m))?;
    if s.len() == 7 {
        return Some(make(year, month, 1, 0, 0, 0, "month"));
    }
    let day = (b.get(7) == Some(&b'-')).then(|| num(8, 10)).flatten().filter(|d| (1..=31).contains(d))?;
    if s.len() == 10 {
        return Some(make(year, month, day, 0, 0, 0, "day"));
    }
    if b.get(10) != Some(&b'T') || b.get(13) != Some(&b':') {
        return None;
    }
    let hour = num(11, 13).filter(|h| *h <= 23)?;
    let min = num(14, 16).filter(|m| *m <= 59)?;
    let mut i = 16;
    let mut sec = 0;
    if b.get(16) == Some(&b':') {
        sec = num(17, 19).filter(|s| *s <= 60)?.min(59);
        i = 19;
        if b.get(19) == Some(&b'.') {
            i = 20;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
    }
    let offset = match b.get(i) {
        // FHIR requires a zone with a time; read a time without one as UTC.
        None => 0,
        Some(b'Z') if i + 1 == s.len() => 0,
        Some(&sign @ (b'+' | b'-')) if s.len() == i + 6 && b[i + 3] == b':' => {
            let o = num(i + 1, i + 3)? * 60 + num(i + 4, i + 6)?;
            if sign == b'+' {
                o
            } else {
                -o
            }
        }
        _ => return None,
    };
    let t = days_from_civil(year, month, day) * 86_400 + hour * 3600 + min * 60 + sec - offset * 60;
    let (y, m, d) = civil_from_days(t.div_euclid(86_400));
    let r = t.rem_euclid(86_400);
    Some(make(y, m, d, r / 3600, r % 3600 / 60, r % 60, "time"))
}

fn make(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, precision: &'static str) -> DateKey {
    DateKey { key: format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z"), precision }
}

/// Days since 1970-01-01 (H. Hinnant, "chrono-Compatible Low-Level Date Algorithms").
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(s: &str) -> Option<(String, &'static str)> {
        key(s).map(|d| (d.key, d.precision))
    }

    #[test]
    fn partial_dates_take_the_start_of_their_period_and_keep_their_precision() {
        assert_eq!(k("2019"), Some(("2019-01-01T00:00:00Z".into(), "year")));
        assert_eq!(k("2019-04"), Some(("2019-04-01T00:00:00Z".into(), "month")));
        assert_eq!(k("2019-04-05"), Some(("2019-04-05T00:00:00Z".into(), "day")));
    }

    #[test]
    fn offsets_become_utc_across_a_day_month_and_year() {
        assert_eq!(k("2024-03-05T09:30:00-05:00").unwrap().0, "2024-03-05T14:30:00Z");
        assert_eq!(k("2023-12-31T22:00:00-03:00").unwrap().0, "2024-01-01T01:00:00Z");
        assert_eq!(k("2024-03-01T01:00:00+02:00").unwrap().0, "2024-02-29T23:00:00Z", "a leap day");
        assert_eq!(k("2024-03-05T14:30:00.123Z").unwrap().0, "2024-03-05T14:30:00Z");
        assert_eq!(k("2024-03-05T14:30Z").unwrap().0, "2024-03-05T14:30:00Z");
    }

    #[test]
    fn keys_sort_in_time_order_and_never_as_numbers() {
        let mut keys = [k("2019").unwrap().0, k("2018-12-31T23:00:00-05:00").unwrap().0, NO_DATE.to_string()];
        keys.sort();
        assert_eq!(keys, [NO_DATE, "2019-01-01T00:00:00Z", "2019-01-01T04:00:00Z"]);
        assert!(!almirah_engine::key::is_canonical_number(&keys[1]));
    }

    #[test]
    fn text_that_is_not_a_date_has_no_key() {
        for bad in ["", "yesterday", "2019-13", "2019-04-05T25:00:00Z", "2019-04-05 10:00", "0000", "2019-04-05T10:00:00+0500"] {
            assert!(key(bad).is_none(), "{bad}");
        }
    }
}
