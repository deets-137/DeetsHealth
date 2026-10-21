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

/// YYYY-MM-DD with a month 01 to 12 and a day 01 to 31.
fn is_day(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) || !digits(5..7) || !digits(8..10) {
        return false;
    }
    let month: u8 = s[5..7].parse().unwrap_or(0);
    let day: u8 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
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
        for bad in ["2026-13-01", "2026-11-00", "26-11-02", "2026/11/02", "2026-11-2x"] {
            assert!(!is_day(bad), "{bad}");
        }
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
