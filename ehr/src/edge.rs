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
    scheduler::{Slot, SlotWish},
    session::Token,
    verify::Claimed,
};

/// The longest reason for a visit the agent may pass on.
const MAX_REASON: usize = 500;
/// The longest given or family name.
const MAX_NAME: usize = 100;

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

pub fn find_slots(mut req: pb::FindSlotsRequest) -> Result<(Token, SlotWish), Status> {
    let handle = handle(&req.handle)?;
    let reason = std::mem::take(&mut req.reason);
    if reason.trim().is_empty() || reason.chars().count() > MAX_REASON {
        return Err(Status::invalid_argument("reason: empty or too long"));
    }
    let day = std::mem::take(&mut req.earliest_day);
    let earliest_day = if day.is_empty() {
        None
    } else if is_day(&day) {
        Some(Shielded::new(day))
    } else {
        return Err(Status::invalid_argument("earliest_day: not YYYY-MM-DD"));
    };
    Ok((handle, SlotWish { reason: Shielded::new(reason), earliest_day }))
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
    }
}

/// The one reveal for a visit the caller already has.
pub fn visit(slot: &Slot) -> pb::Visit {
    let pass = WirePass::issue();
    pb::Visit {
        starts_at: slot.starts_at.reveal(&pass).clone(),
        minutes: slot.minutes,
        clinician: slot.clinician.reveal(&pass).clone(),
        place: slot.place.reveal(&pass).clone(),
    }
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
        };
        let err = find_slots(req).err().unwrap();
        assert!(!err.message().contains("CANARY"));
    }
}
