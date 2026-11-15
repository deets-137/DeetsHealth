//! The edge (EHR-SERVICE.md §5): the only place a wire type becomes a domain type, and back.
//!
//! Protobuf has no required fields: a missing value arrives as "" or 0. So every request is
//! checked here, before any logic runs, and each protected value goes straight into
//! `Shielded`. A reply that must carry a protected value reveals it here, with the one
//! `WirePass` in the service. Errors name the field, never its value.

use deetshealth_core::{wire::WirePass, Shielded};
use tonic::Status;

use crate::{
    proto::v1 as pb,
    scheduler::{DayRange, Slot, SlotWish, VisitAsk, VisitMode, VisitStatus},
    session::{Source, Token},
    verify::Claimed,
    visit_about::BookNote,
};

/// The longest reason for a visit the agent may pass on.
const MAX_REASON: usize = 500;
/// The longest given or family name.
const MAX_NAME: usize = 100;
/// The longest call id (a record's file name): `20261112-143222-test`, with room for a suffix.
const MAX_CALL_ID: usize = 64;

/// One line of a FileCall stream: the call id, checked (letters, digits and `-` only, so it is a
/// safe file and row name), and the line, which must be one JSON object. The line holds the
/// call's words, so it goes no further than the store, which seals it.
pub fn file_call_line(req: pb::FileCallLine) -> Result<(String, String), Status> {
    let id = req.call_id;
    if id.is_empty() || id.len() > MAX_CALL_ID || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(Status::invalid_argument("call_id: not a record id"));
    }
    let line = req.line;
    if !line.trim_start().starts_with('{') {
        return Err(Status::invalid_argument("line: not a JSON event"));
    }
    Ok((id, line))
}

/// The EHR a new call reads. Unspecified is the local fakes.
pub fn source(req: &pb::OpenCallRequest) -> Result<Source, Status> {
    match pb::EhrSource::try_from(req.ehr) {
        Ok(pb::EhrSource::Unspecified | pb::EhrSource::Local) => Ok(Source::Local),
        Ok(pb::EhrSource::EpicSandbox) => Ok(Source::EpicSandbox),
        Err(_) => Err(Status::invalid_argument("ehr: not a known source")),
    }
}

pub fn handle(wire: &str) -> Result<Token, Status> {
    Token::parse(wire).ok_or_else(|| Status::invalid_argument("handle: not a handle"))
}

pub fn identify(mut req: pb::IdentifyRequest) -> Result<(Token, Claimed), Status> {
    let handle = handle(&req.handle)?;
    let given = std::mem::take(&mut req.given_name);
    let family = std::mem::take(&mut req.family_name);
    let birth_date = std::mem::take(&mut req.birth_date);
    for (name, field) in [(&given, "given_name"), (&family, "family_name")] {
        if name.trim().is_empty() || name.chars().count() > MAX_NAME {
            return Err(Status::invalid_argument(match field {
                "given_name" => "given_name: empty or too long",
                _ => "family_name: empty or too long",
            }));
        }
    }
    if !is_day(&birth_date) {
        return Err(Status::invalid_argument("birth_date: not YYYY-MM-DD"));
    }
    // Checked here, before the session counts a try: a slip of the tongue ("2056") must not
    // cost the caller one of their three (2026-11-11 desk test).
    if !is_possible_birth(&birth_date, &utc_day(1)) {
        return Err(Status::invalid_argument("birth_date: not a possible birth date"));
    }
    Ok((
        handle,
        Claimed {
            given: Shielded::new(given),
            family: Shielded::new(family),
            birth_date: Shielded::new(birth_date),
        },
    ))
}

pub fn check_code(mut req: pb::CheckCodeRequest) -> Result<(Token, Shielded<String>), Status> {
    let handle = handle(&req.handle)?;
    let code = std::mem::take(&mut req.code);
    if code.len() != 6 || !code.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Status::invalid_argument("code: not six digits"));
    }
    Ok((handle, Shielded::new(code)))
}

/// A new search, or the next slots of the last one.
pub enum SlotAsk {
    New(SlotWish),
    More,
}

pub fn find_slots(mut req: pb::FindSlotsRequest) -> Result<(Token, SlotAsk), Status> {
    let handle = handle(&req.handle)?;
    if req.more {
        return Ok((handle, SlotAsk::More));
    }
    let reason = std::mem::take(&mut req.reason);
    if reason.trim().is_empty() || reason.chars().count() > MAX_REASON {
        return Err(Status::invalid_argument("reason: empty or too long"));
    }
    let day = |s: String, what: &str| -> Result<Option<Shielded<String>>, Status> {
        if s.is_empty() {
            Ok(None)
        } else if is_day(&s) {
            Ok(Some(Shielded::new(s)))
        } else {
            Err(Status::invalid_argument(format!("{what}: not YYYY-MM-DD")))
        }
    };
    let earliest_day = day(std::mem::take(&mut req.earliest_day), "earliest_day")?;
    let latest_day = day(std::mem::take(&mut req.latest_day), "latest_day")?;
    let time = std::mem::take(&mut req.time_of_day);
    let time_of_day = if time.trim().is_empty() {
        None
    } else {
        // Only a name on the hospital's list: the model picks, the hospital decides what it means.
        Some(crate::hospital::hospital().time_of_day(&time).ok_or_else(|| Status::invalid_argument("time_of_day: not on the hospital's list"))?)
    };
    let mut days = Vec::new();
    for d in &req.days {
        use jiff::civil::Weekday::*;
        let w = match d.trim().to_ascii_lowercase().as_str() {
            "mon" => Monday,
            "tue" => Tuesday,
            "wed" => Wednesday,
            "thu" => Thursday,
            "fri" => Friday,
            "sat" => Saturday,
            "sun" => Sunday,
            _ => return Err(Status::invalid_argument("days: mon to sun")),
        };
        if !days.contains(&w) {
            days.push(w);
        }
    }
    let who = std::mem::take(&mut req.clinician).trim().to_string();
    if who.chars().count() > MAX_NAME {
        return Err(Status::invalid_argument("clinician: too long"));
    }
    let clinician = (!who.is_empty()).then(|| Shielded::new(who));
    let mode = match pb::VisitMode::try_from(req.visit_mode) {
        Ok(pb::VisitMode::Unspecified) => VisitMode::Either,
        Ok(pb::VisitMode::InPerson) => VisitMode::InPerson,
        Ok(pb::VisitMode::Online) => VisitMode::Online,
        Err(_) => return Err(Status::invalid_argument("visit_mode: not a known value")),
    };
    // Screened here, as plain words, before they are wrapped (visit_about.rs).
    let note = BookNote::screen(reason.trim().to_string());
    Ok((
        handle,
        SlotAsk::New(SlotWish { note, earliest_day, latest_day, mode, time_of_day, days, clinician, who: None }),
    ))
}

pub fn book(req: &pb::BookRequest) -> Result<(Token, Token, String), Status> {
    let handle = handle(&req.handle)?;
    let slot = Token::parse(&req.slot_token)
        .ok_or_else(|| Status::invalid_argument("slot_token: not a token"))?;
    let key = &req.idempotency_key;
    let ok_key = (16..=128).contains(&key.len())
        && key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
    if !ok_key {
        return Err(Status::invalid_argument("idempotency_key: 16 to 128 of [A-Za-z0-9_-]"));
    }
    Ok((handle, slot, key.clone()))
}

/// The one reveal for the wire: a slot, under the token the agent knows it by.
pub fn slot(token: &Token, slot: &Slot) -> pb::Slot {
    let pass = WirePass::issue();
    pb::Slot {
        token: token.to_hex(),
        starts_at: slot.starts_at.reveal(&pass).clone(),
        minutes: slot.minutes,
        clinician: slot.clinician.reveal(&pass).clone(),
        place: slot.place.reveal(&pass).clone(),
        online: slot.about.online,
    }
}

/// What the caller asked about a visit (EPIC.md §7.10). The clinician's and the group's words
/// stay wrapped; the day or month becomes the range of days it covers.
pub struct VisitQuestion {
    pub ask: VisitAsk,
    pub clinician: Option<Shielded<String>>,
    pub group: Option<Shielded<String>>,
}

pub fn find_visit(mut req: pb::FindVisitRequest) -> Result<(Token, VisitQuestion), Status> {
    let handle = handle(&req.handle)?;
    let when = std::mem::take(&mut req.when);
    let range = if when.is_empty() {
        None
    } else {
        Some(day_range(when.trim()).ok_or_else(|| Status::invalid_argument("when: not YYYY-MM-DD or YYYY-MM"))?)
    };
    let words = |s: String, what: &str| -> Result<Option<Shielded<String>>, Status> {
        let s = s.trim().to_string();
        if s.chars().count() > MAX_NAME {
            return Err(Status::invalid_argument(format!("{what}: too long")));
        }
        Ok((!s.is_empty()).then(|| Shielded::new(s)))
    };
    let clinician = words(std::mem::take(&mut req.clinician), "clinician")?;
    let group = words(std::mem::take(&mut req.group), "group")?;
    Ok((handle, VisitQuestion { ask: VisitAsk { range, past: req.past }, clinician, group }))
}

/// "2026-11-10" -> that day; "2026-11" -> that month. Each as [from, to).
fn day_range(s: &str) -> Option<DayRange> {
    let (from, to) = if is_day(s) {
        let d: jiff::civil::Date = s.parse().ok()?;
        (d, d.tomorrow().ok()?)
    } else {
        let b = s.as_bytes();
        if b.len() != 7 || b[4] != b'-' || !b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit) {
            return None;
        }
        let year: i16 = s[..4].parse().ok()?;
        let month: i8 = s[5..].parse().ok()?;
        let first = jiff::civil::Date::new(year, month, 1).ok()?;
        (first, first.checked_add(jiff::Span::new().months(1)).ok()?)
    };
    Some(DayRange { from: Shielded::new(from.to_string()), to: Shielded::new(to.to_string()) })
}

/// The one reveal for a visit the caller already has, or has just booked.
pub fn visit(slot: &Slot) -> pb::Visit {
    let pass = WirePass::issue();
    let status = match slot.status {
        VisitStatus::Upcoming => pb::VisitStatus::Upcoming,
        VisitStatus::Seen => pb::VisitStatus::Seen,
        VisitStatus::Missed => pb::VisitStatus::Missed,
        VisitStatus::Cancelled => pb::VisitStatus::Cancelled,
    };
    pb::Visit {
        starts_at: slot.starts_at.reveal(&pass).clone(),
        minutes: slot.minutes,
        clinician: slot.clinician.reveal(&pass).clone(),
        place: slot.place.reveal(&pass).clone(),
        visit_type: slot.about.visit_type.as_ref().map(|s| s.reveal(&pass).clone()).unwrap_or_default(),
        reason: slot.about.reason.as_ref().map(|s| s.reveal(&pass).clone()).unwrap_or_default(),
        online: slot.about.online,
        reason_private: slot.about.private,
        instructions: slot.about.instructions.as_ref().map(|s| s.reveal(&pass).clone()).unwrap_or_default(),
        instructions_private: slot.about.instructions_private,
        csn: slot.csn.as_ref().map(|s| s.reveal(&pass).clone()).unwrap_or_default(),
        status: status.into(),
    }
}

/// The one reveal for the groups the caller is asked about.
pub fn groups(names: &[Shielded<String>]) -> Vec<String> {
    let pass = WirePass::issue();
    names.iter().map(|n| n.reveal(&pass).clone()).collect()
}

/// YYYY-MM-DD with a month 01 to 12 and a day 01 to 31.
fn is_day(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) || !digits(5..7) || !digits(8..10) {
        return false;
    }
    let year: u32 = s[0..4].parse().unwrap_or(0);
    let month: u8 = s[5..7].parse().unwrap_or(0);
    let day: u8 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

fn days_in_month(year: u32, month: u8) -> u8 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A birth date no later than `latest` and no earlier than 1900. Both are `is_day` strings, so
/// the order of the text is the order of the days.
fn is_possible_birth(birth: &str, latest: &str) -> bool {
    birth >= "1900-01-01" && birth <= latest
}

/// The UTC day `ahead` days from now, as YYYY-MM-DD. One day ahead covers every time zone's
/// "today", so a baby born this evening in Hawaii still passes.
fn utc_day(ahead: i64) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_day(secs.div_euclid(86_400) + ahead)
}

/// Days since 1970-01-01 to YYYY-MM-DD (Howard Hinnant's `civil_from_days`).
fn civil_day(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_call_line_needs_a_record_id_and_a_json_event() {
        let line = |id: &str, line: &str| file_call_line(pb::FileCallLine { call_id: id.into(), line: line.into() });
        assert!(line("20261112-143222-test", r#"{"t_ms": 0}"#).is_ok());
        for bad_id in ["", "../../etc/passwd", "a b", "x".repeat(65).as_str()] {
            assert!(line(bad_id, "{}").is_err(), "a call id must be letters, digits and - only");
        }
        assert!(line("20261112-143222-test", "not json").is_err());
    }

    /// The wire pass is issued in this file only (core/src/wire.rs).
    #[test]
    fn only_the_edge_names_the_wire_pass() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let needle = ["Wire", "Pass"].concat();
        for entry in std::fs::read_dir(src).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == "edge.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains(&needle), "{} names the wire pass", path.display());
        }
    }

    #[test]
    fn an_empty_field_is_refused_not_defaulted() {
        assert!(handle("").is_err());
        let req = pb::FindSlotsRequest { handle: Token::random().to_hex(), ..Default::default() };
        assert!(find_slots(req).is_err(), "an empty reason must be refused");
        let req = pb::BookRequest { handle: Token::random().to_hex(), ..Default::default() };
        assert!(book(&req).is_err(), "an empty slot token must be refused");
    }

    #[test]
    fn days_are_checked() {
        assert!(is_day("2026-11-02"));
        assert!(is_day("2024-02-29"), "a leap day");
        for bad in [
            "2026-13-01", "2026-11-00", "26-11-02", "2026/11/02", "2026-11-2x", "2026-02-29", "2026-04-31",
            "1900-02-29",
        ] {
            assert!(!is_day(bad), "{bad}");
        }
    }

    #[test]
    fn a_birth_date_must_be_possible() {
        assert!(is_possible_birth("1984-03-12", "2026-11-12"));
        assert!(is_possible_birth("2026-11-12", "2026-11-12"), "born today, somewhere");
        assert!(!is_possible_birth("2056-02-27", "2026-11-12"), "the 2026-11-11 desk test");
        assert!(!is_possible_birth("1899-12-31", "2026-11-12"));
    }

    #[test]
    fn the_clock_turns_days_into_dates() {
        assert_eq!(civil_day(0), "1970-01-01");
        assert_eq!(civil_day(19_723), "2024-01-01");
        assert_eq!(civil_day(20_403), "2025-11-11");
        assert!(utc_day(1).as_str() > "2026-01-01");
    }

    #[test]
    fn a_future_birth_date_is_refused_before_a_try_is_counted() {
        let req = pb::IdentifyRequest {
            handle: Token::random().to_hex(),
            given_name: "Billy".into(),
            family_name: "Bob".into(),
            birth_date: "2056-02-27".into(),
        };
        let err = identify(req).err().unwrap();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(!err.message().contains("2056"), "a refusal never carries the value");
    }

    #[test]
    fn a_refusal_never_carries_the_value() {
        let req = pb::FindSlotsRequest {
            handle: Token::random().to_hex(),
            reason: "CANARY".into(),
            earliest_day: "CANARY-DAY".into(),
            ..Default::default()
        };
        let err = find_slots(req).err().unwrap();
        assert!(!err.message().contains("CANARY"));
        let req = pb::FindSlotsRequest {
            handle: Token::random().to_hex(),
            reason: "a check-up".into(),
            time_of_day: "CANARY".into(),
            ..Default::default()
        };
        assert!(!find_slots(req).err().unwrap().message().contains("CANARY"));
    }

    fn wish(ask: SlotAsk) -> SlotWish {
        match ask {
            SlotAsk::New(w) => w,
            SlotAsk::More => panic!("a new search"),
        }
    }

    #[test]
    fn a_last_day_a_time_of_day_and_days_are_read() {
        let req = pb::FindSlotsRequest {
            handle: Token::random().to_hex(),
            reason: "a check-up".into(),
            latest_day: "2026-11-20".into(),
            time_of_day: "Morning".into(),
            days: vec!["tue".into(), "THU".into(), "tue".into()],
            clinician: " Dr. Lin ".into(),
            ..Default::default()
        };
        let w = wish(find_slots(req).ok().unwrap().1);
        assert!(w.latest_day.is_some() && w.clinician.is_some() && w.who.is_none());
        assert_eq!(w.time_of_day.unwrap().start(), "08:00:00");
        assert_eq!(w.days, [jiff::civil::Weekday::Tuesday, jiff::civil::Weekday::Thursday]);
        let bad = |f: fn(&mut pb::FindSlotsRequest)| {
            let mut r = pb::FindSlotsRequest { handle: Token::random().to_hex(), reason: "x".into(), ..Default::default() };
            f(&mut r);
            find_slots(r).is_err()
        };
        assert!(bad(|r| r.latest_day = "Friday".into()));
        assert!(bad(|r| r.time_of_day = "midnight".into()));
        assert!(bad(|r| r.days = vec!["weekend".into()]));
        // More: no reason needed, nothing else read.
        let more = pb::FindSlotsRequest { handle: Token::random().to_hex(), more: true, ..Default::default() };
        assert!(matches!(find_slots(more).ok().unwrap().1, SlotAsk::More));
    }

    #[test]
    fn a_visit_mode_is_read_and_an_unknown_one_refused() {
        let req = |mode: i32| pb::FindSlotsRequest {
            handle: Token::random().to_hex(),
            reason: "a check-up".into(),
            visit_mode: mode,
            ..Default::default()
        };
        assert_eq!(wish(find_slots(req(0)).ok().unwrap().1).mode, VisitMode::Either);
        assert_eq!(wish(find_slots(req(2)).ok().unwrap().1).mode, VisitMode::Online);
        assert!(find_slots(req(9)).is_err());
    }

    #[test]
    fn a_sensitive_reason_is_marked_private_at_the_edge() {
        let req = pb::FindSlotsRequest {
            handle: Token::random().to_hex(),
            reason: "  a prenatal check  ".into(),
            ..Default::default()
        };
        let (_, ask) = find_slots(req).ok().unwrap();
        assert!(wish(ask).note.private);
    }
}
