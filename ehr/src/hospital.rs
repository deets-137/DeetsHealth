//! The hospital's own setup: Config › Hospital (HOSPITAL-CONFIG.md). Its lists and numbers are
//! the `hospital` service of the settings store (settings.rs); `ehr/fixtures/hospital.json` is
//! the Default and the copy a run without a store uses.
//!
//! - **One hospital** for every EHR source (the owner's 2A, 2026-11-15): the local fake and
//!   Epic's sandbox read the same lists. Many hospitals, with routing by rules, come later.
//! - **Each call keeps its own copy** (`OpenCall`): a save reaches the next call, never a call
//!   in progress.
//! - **The model never sees a code**: only spoken names and the time-of-day names.
//! - **No protected data.** Each row is a fact about the hospital. The errors name a list and a
//!   row number, never a value.
//!
//! Some lists are kept but read by no call yet (the owner's 1B: "what we have left to build"):
//! `specialties`, `reasons`, `hours`, `closed_days`, and a location's id, address and phone.
//!
//! **One Epic list per list** (the owner's A, 2026-11-15): `visit_type_list`,
//! `specialty_list` and `diagnosis_list` each hold one OID, set once; a row holds only its ID
//! in that list. A reason names its code system by name (`REASON_SYSTEMS`). A version saved
//! before A, with an OID on each row, is converted when it is read (`upgrade`).

use std::sync::{Arc, OnceLock};

use deetshealth_core::Shielded;
use jiff::tz::TimeZone;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    visit_about::{names_sensitive, BookNote, VisitAbout},
    visits::{Limits, VisitGroup, VisitGroups},
};

const SOURCE: &str = include_str!("../fixtures/hospital.json");

/// Every setting of the `hospital` service, in the page's order. settings.rs › HOSPITAL is the
/// same list.
pub const KEYS: &[&str] = &[
    "time_zone",
    "visit_type_list",
    "visit_types",
    "visit_groups",
    "diagnosis_list",
    "reasons",
    "times_of_day",
    "hours",
    "closed_days",
    "facilities",
    "locations",
    "specialty_list",
    "specialties",
    "slots_offered",
    "narrow_over",
    "groups_named",
    "past_days",
    "established_years",
    "sooner_with_others",
    "ahead_windows",
    "booking_note_label",
];

/// The longest cell: a name, a code, an address.
pub const CELL_MAX: usize = 120;
/// The most rows in one list.
pub const ROWS_MAX: usize = 200;
/// The most facilities, and the most departments: an import brings every one in (the owner,
/// 2026-11-15), and the analysts remove what the agent does not need.
pub const PLACES_MAX: usize = 2000;
/// The most words (or members) in one cell of words.
pub const WORDS_MAX: usize = 30;
/// The most times of day: each is a word the model may pick.
pub const TIMES_MAX: usize = 12;
/// The most search windows.
pub const WINDOWS_MAX: usize = 5;

/// The code systems a reason may name, by name, and the OID each stands for. "hospital" is the
/// hospital's own diagnosis list (`diagnosis_list`): Epic's `indications` takes "a community
/// diagnosis code and/or a SNOMED code" (open.epic api:840); Epic's sample sends ICD-10-CM too.
pub const REASON_SYSTEMS: &[(&str, &str)] = &[
    ("snomed", "urn:oid:2.16.840.1.113883.6.96"),
    ("icd10cm", "urn:oid:2.16.840.1.113883.6.90"),
    ("hospital", ""),
];

/// The list keys and the rows they serve: a row's ID is in its list's OID.
const LISTS: &[(&str, &str)] = &[("visit_type_list", "visit_types"), ("specialty_list", "specialties")];

/// An OID as a FHIR system: empty, or `urn:oid:` and dotted numbers.
fn oid(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("urn:oid:") else { return text.is_empty() };
    !rest.is_empty() && rest.len() <= CELL_MAX && rest.split('.').all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// One visit type (Epic's `service-type`). `record` is the type's name as the record holds it;
/// a visit or a slot of that name takes the row's spoken name and its Online mark. `code` is
/// the type's ID in the hospital's visit type list (`visit_type_list`). Only a row with an ID
/// reaches Epic's `$find`; the local fake books any row the agent may book.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VisitTypeRow {
    #[serde(default)]
    pub code: String,
    pub record: String,
    #[serde(default)]
    pub spoken: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub book: bool,
    /// Which patients the type is for (BOOKING-RULES.md §3.4). A version saved before the
    /// column reads as "either".
    #[serde(default)]
    pub patients: PatientKind,
}

/// Whether a visit type is for patients new to the department's specialty, for established
/// ones, or for either (BOOKING-RULES.md §3.4; the CPT rule: new = not seen by that specialty
/// in the hospital's window, `established_years`).
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PatientKind {
    #[default]
    Either,
    New,
    Established,
}

impl PatientKind {
    /// Whether a caller who is (or is not) established fits.
    pub fn fits(self, established: bool) -> bool {
        match self {
            PatientKind::Either => true,
            PatientKind::New => !established,
            PatientKind::Established => established,
        }
    }
}

/// A clinician as a record names them: the Practitioner's FHIR id and the display name, the same
/// shape as a place (BOOKING-RULES.md §3.2: the PCP, and who a visit or a slot is with).
pub type ClinicianRef = PlaceRef;

/// A place as a visit's record names it: the Location's FHIR id and the record's name, trimmed
/// and in lower case so it compares without a reveal. Wrapped: on a patient's visit it says
/// where they were seen.
#[derive(Clone, Default, PartialEq)]
pub struct PlaceRef {
    pub id: Option<Shielded<String>>,
    pub name: Option<Shielded<String>>,
}

impl PlaceRef {
    pub fn new(id: Option<&str>, name: Option<&str>) -> Self {
        let keep = |s: Option<&str>, lower: bool| {
            s.map(str::trim).filter(|s| !s.is_empty()).map(|s| Shielded::new(if lower { s.to_lowercase() } else { s.to_string() }))
        };
        Self { id: keep(id, false), name: keep(name, true) }
    }

    /// The same place (or clinician): the same FHIR id when both have one, else the same name.
    pub fn same(&self, other: &PlaceRef) -> bool {
        match (&self.id, &other.id) {
            (Some(a), Some(b)) => a == b,
            _ => self.name.is_some() && self.name == other.name,
        }
    }
}

/// One time of day a caller may name ("morning"): its first hour and its length.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimeOfDayRow {
    pub name: String,
    /// HH:MM, local time at the place.
    pub from: String,
    pub hours: u8,
}

/// One facility (Epic's EAF): a building or campus that holds departments (HOSPITAL-CONFIG.md
/// §8.2, the owner's 3A). `id` is the Location's FHIR id; `code` its EAF ID. Kept; a call says
/// a department's words, never a facility's, yet.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct FacilityRow {
    pub record: String,
    #[serde(default)]
    pub spoken: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub phone: String,
}

/// One department (Epic's DEP), the place a visit is booked in. `record` is the Location's name
/// as the record holds it ("EMC Family Medicine"); the visit says `spoken` instead. The first
/// row is said when a record names no place. `id` is the FHIR id, `code` the DEP ID, `facility`
/// the FHIR id of its facility (empty: none), `specialty` its ID in `specialty_list`. The id,
/// address, phone, specialty and hours are kept for later (`$find`'s `location-reference`,
/// "where to go").
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct LocationRow {
    pub record: String,
    #[serde(default)]
    pub spoken: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub facility: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub specialty: String,
    #[serde(default)]
    pub hours: Vec<HoursRow>,
}

/// The USPS codes of the states, DC and the territories: what a directory search may name.
pub const US_STATES: &[&str] = &[
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "DC", "FL", "GA", "HI", "ID", "IL", "IN", "IA", "KS", "KY", "LA", "ME", "MD", "MA",
    "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ", "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC", "SD", "TN", "TX",
    "UT", "VT", "VA", "WA", "WV", "WI", "WY", "AS", "GU", "MP", "PR", "VI",
];

/// One directory search (HOSPITAL-CONFIG.md §8.2): a state ("WI"), or a city and its state
/// ("Verona, WI"). The page sends each in this form; anything else is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSearch {
    pub city: Option<String>,
    pub state: String,
}

impl PlaceSearch {
    /// "WI" or "Verona, WI": a known state code, and a city of letters, spaces, `.`, `'` and `-`
    /// (at most 60). None for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (city, state) = match text.rsplit_once(',') {
            Some((city, state)) => (Some(city.trim()), state.trim()),
            None => (None, text),
        };
        if !US_STATES.contains(&state) {
            return None;
        }
        let city = match city {
            None => None,
            Some(c) if !c.is_empty() && c.chars().count() <= 60 && c.chars().all(|ch| ch.is_alphabetic() || " .'-".contains(ch)) => {
                Some(c.to_string())
            }
            Some(_) => return None,
        };
        Some(Self { city, state: state.to_string() })
    }
}

/// What an EHR's directory says of the hospital's places (HOSPITAL-CONFIG.md §8): the rows an
/// import offers the page, before it merges them into the draft. Each EHR's adapter builds it
/// from its own directory (Epic: epic_ehr.rs › places).
#[derive(Debug, Default, PartialEq)]
pub struct Places {
    pub facilities: Vec<FacilityRow>,
    pub locations: Vec<LocationRow>,
    /// The OID of the departments' specialty list; empty when no department named one.
    pub specialty_list: String,
    /// Each specialty the departments name, once: its ID and the directory's own words.
    pub specialties: Vec<SpecialtyRow>,
    /// Locations that are neither a facility nor a department (rooms, billing offices).
    pub others: usize,
    /// Departments whose specialty is in a second list: left without a specialty.
    pub other_specialty_lists: usize,
    /// The reads it took.
    pub reads: usize,
}

/// One specialty (Epic's `specialties`), its ID in `specialty_list`. Kept; no call reads it yet.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpecialtyRow {
    #[serde(default)]
    pub code: String,
    pub spoken: String,
    #[serde(default)]
    pub words: Vec<String>,
    /// Primary care (BOOKING-RULES.md §4): a call that names no specialty searches at the
    /// caller's last department of a primary care specialty when they have no visit with their
    /// PCP. A version saved before the mark reads as not primary care.
    #[serde(default)]
    pub primary_care: bool,
}

/// One reason as a code (Epic's `indications`). Kept; no call reads it yet: booking sends the
/// caller's own words (EPIC.md §5.2).
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReasonRow {
    /// A name from `REASON_SYSTEMS`, or empty.
    #[serde(default)]
    pub system: String,
    #[serde(default)]
    pub code: String,
    pub words: Vec<String>,
    #[serde(default)]
    pub visit_type: String,
    #[serde(default)]
    pub specialty: String,
}

/// The hours on one day of the week. Kept; no call reads it yet.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HoursRow {
    /// mon to sun.
    pub day: String,
    pub opens: String,
    pub closes: String,
}

/// A day the hospital is closed. Kept; no call reads it yet.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClosedDayRow {
    /// YYYY-MM-DD.
    pub date: String,
    #[serde(default)]
    pub name: String,
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
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// The hospital, checked. Built only by [`Hospital::from_pairs`], so every value fits.
pub struct Hospital {
    pub time_zone: String,
    /// The OID of the hospital's visit type list: the system of every visit type's ID.
    pub visit_type_list: String,
    /// The OID of the hospital's own diagnosis list: a reason's "hospital" system.
    pub diagnosis_list: String,
    /// The OID of the hospital's specialty list.
    pub specialty_list: String,
    pub visit_types: Vec<VisitTypeRow>,
    pub visit_groups: Vec<VisitGroup>,
    pub reasons: Vec<ReasonRow>,
    pub times_of_day: Vec<TimeOfDayRow>,
    pub hours: Vec<HoursRow>,
    pub closed_days: Vec<ClosedDayRow>,
    pub facilities: Vec<FacilityRow>,
    pub locations: Vec<LocationRow>,
    pub specialties: Vec<SpecialtyRow>,
    /// How many slots the agent offers at a time.
    pub slots_offered: usize,
    /// More visits than this match, and the caller named something: the narrowing question.
    pub narrow_over: usize,
    /// The most groups one question names; the rest are "another visit".
    pub groups_named: usize,
    /// How far back "when was I last seen?" looks, in days.
    pub past_days: i64,
    /// A caller seen by a specialty within this many years is established in it (the CPT
    /// rule's 3 by default; BOOKING-RULES.md §3.4).
    pub established_years: i64,
    /// The owner's P-A (BOOKING-RULES.md §3.3): with the PCP's times, also offer a time with
    /// another clinician at the same place when it is on an earlier day than the PCP's first.
    pub sooner_with_others: bool,
    /// The windows the search for the visits to come widens through, in days, then no end.
    pub ahead_windows: Vec<i64>,
    /// Put after the caller's words in the note a booking files in Epic (the owner's N2,
    /// 2026-11-15), so staff know the reason is the caller's own. Empty: the words alone.
    pub booking_note_label: String,
    tz: TimeZone,
    groups: VisitGroups,
}

/// One cell of text: one line, at most `CELL_MAX` characters, not empty when `needed`.
fn cell(text: &str, needed: bool) -> Result<(), &'static str> {
    if needed && text.trim().is_empty() {
        return Err("is empty");
    }
    if text.chars().count() > CELL_MAX {
        return Err("is too long");
    }
    if text.chars().any(char::is_control) {
        return Err("holds more than one line");
    }
    Ok(())
}

fn words(list: &[String]) -> Result<(), &'static str> {
    if list.len() > WORDS_MAX {
        return Err("has too many words");
    }
    for w in list {
        cell(w, true).map_err(|_| "has a word that is empty, too long or more than one line")?;
    }
    Ok(())
}

fn rows<T>(key: &str, list: &[T], each: impl Fn(&T) -> Result<(), String>) -> Result<(), String> {
    rows_up_to(key, list, ROWS_MAX, each)
}

fn rows_up_to<T>(key: &str, list: &[T], max: usize, each: impl Fn(&T) -> Result<(), String>) -> Result<(), String> {
    if list.len() > max {
        return Err(format!("{key}: at most {max} rows"));
    }
    for (i, row) in list.iter().enumerate() {
        each(row).map_err(|e| format!("{key}: row {}: {e}", i + 1))?;
    }
    Ok(())
}

/// The first row number whose `name` repeats an earlier one, in any case.
fn twice<T>(list: &[T], name: impl Fn(&T) -> &str) -> Option<usize> {
    let mut seen: Vec<String> = Vec::new();
    for (i, row) in list.iter().enumerate() {
        let n = name(row).trim().to_lowercase();
        if n.is_empty() {
            continue;
        }
        if seen.contains(&n) {
            return Some(i + 1);
        }
        seen.push(n);
    }
    None
}

/// The first row number that names a place an earlier row names: by the FHIR id when the row
/// has one (exactly), else by the record's name (in any case). Two departments of one name in
/// two facilities are two places when Epic gives each its id.
fn twice_place<'a, T>(list: &'a [T], id_and_name: impl Fn(&'a T) -> (&'a str, &'a str)) -> Option<usize> {
    let mut seen: Vec<String> = Vec::new();
    for (i, row) in list.iter().enumerate() {
        let (id, name) = id_and_name(row);
        let key = if id.trim().is_empty() { format!("name:{}", name.trim().to_lowercase()) } else { format!("id:{}", id.trim()) };
        if key == "name:" {
            continue;
        }
        if seen.contains(&key) {
            return Some(i + 1);
        }
        seen.push(key);
    }
    None
}

/// One day's hours: mon to sun, HH:MM, opening before closing.
fn hours_row(h: &HoursRow) -> Result<(), String> {
    if !["mon", "tue", "wed", "thu", "fri", "sat", "sun"].contains(&h.day.as_str()) {
        return Err("the day is not mon to sun".into());
    }
    let (Some(o), Some(c)) = (minute_of(&h.opens), minute_of(&h.closes)) else {
        return Err("a time is not HH:MM".into());
    };
    if o >= c {
        return Err("it closes before it opens".into());
    }
    Ok(())
}

fn field<T: DeserializeOwned>(key: &str, value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|_| format!("{key}: not the list's shape"))
}

fn whole(key: &str, value: &Value, min: i64, max: i64) -> Result<i64, String> {
    value.as_i64().filter(|n| (min..=max).contains(n)).ok_or_else(|| format!("{key}: a whole number from {min} to {max}"))
}

/// One setting of the `hospital` service, checked on its own (settings.rs › check). The error
/// names the setting and the row, never a value.
pub fn check_key(key: &str, value: &Value) -> Result<(), String> {
    match key {
        "time_zone" => {
            let zone = value.as_str().filter(|z| !z.is_empty() && z.len() <= 64).ok_or("time_zone: an IANA name, such as America/Chicago")?;
            TimeZone::get(zone).map(|_| ()).map_err(|_| "time_zone: not a known IANA name".to_string())
        }
        "visit_type_list" | "specialty_list" | "diagnosis_list" => {
            let text = value.as_str().ok_or_else(|| format!("{key}: an OID as text"))?;
            if oid(text) {
                Ok(())
            } else {
                Err(format!("{key}: an OID such as urn:oid:1.2.840.114350…, or empty"))
            }
        }
        "visit_types" => {
            let list: Vec<VisitTypeRow> = field(key, value)?;
            rows(key, &list, |r| {
                cell(&r.record, true).map_err(|e| format!("the record's name {e}"))?;
                cell(&r.spoken, false).map_err(|e| format!("the spoken name {e}"))?;
                cell(&r.code, false).map_err(|e| format!("the ID {e}"))?;
                Ok(())
            })?;
            match twice(&list, |r| &r.record) {
                Some(n) => Err(format!("{key}: row {n}: the record's name is there twice")),
                None => Ok(()),
            }
        }
        "visit_groups" => {
            let list: Vec<VisitGroup> = field(key, value)?;
            rows(key, &list, |g| {
                cell(&g.name, true).map_err(|e| format!("the name {e}"))?;
                words(&g.words).map_err(|e| format!("the caller's words {e}"))?;
                words(&g.visit_types).map_err(|e| format!("the visit types {e}"))?;
                Ok(())
            })?;
            match twice(&list, |g| &g.name) {
                Some(n) => Err(format!("{key}: row {n}: the name is there twice")),
                None => Ok(()),
            }
        }
        "reasons" => {
            let list: Vec<ReasonRow> = field(key, value)?;
            rows(key, &list, |r| {
                if r.words.is_empty() {
                    return Err("needs the caller's words".into());
                }
                words(&r.words).map_err(|e| format!("the caller's words {e}"))?;
                if !r.system.is_empty() && !REASON_SYSTEMS.iter().any(|(n, _)| *n == r.system) {
                    return Err("the code system is not SNOMED CT, ICD-10-CM or the hospital's list".into());
                }
                if !r.code.trim().is_empty() && r.system.is_empty() {
                    return Err("a code needs its code system".into());
                }
                for (what, text) in [("the code", &r.code), ("the visit type", &r.visit_type), ("the specialty", &r.specialty)] {
                    cell(text, false).map_err(|e| format!("{what} {e}"))?;
                }
                Ok(())
            })
        }
        "times_of_day" => {
            let list: Vec<TimeOfDayRow> = field(key, value)?;
            if list.len() > TIMES_MAX {
                return Err(format!("{key}: at most {TIMES_MAX} rows"));
            }
            rows(key, &list, |t| {
                cell(&t.name, true).map_err(|e| format!("the name {e}"))?;
                if t.name.chars().count() > 40 || !t.name.chars().all(|c| c.is_ascii_lowercase() || c == ' ' || c == '-') {
                    return Err("the name holds only a to z, spaces and -, at most 40".into());
                }
                let from = minute_of(&t.from).ok_or("the start is not HH:MM")?;
                if t.hours == 0 || u16::from(t.hours) * 60 + from > 24 * 60 {
                    return Err("the length ends after midnight or is 0".into());
                }
                Ok(())
            })?;
            match twice(&list, |t| &t.name) {
                Some(n) => Err(format!("{key}: row {n}: the name is there twice")),
                None => Ok(()),
            }
        }
        "hours" => {
            let list: Vec<HoursRow> = field(key, value)?;
            rows(key, &list, hours_row)
        }
        "closed_days" => {
            let list: Vec<ClosedDayRow> = field(key, value)?;
            rows(key, &list, |d| {
                if d.date.len() != 10 || d.date.parse::<jiff::civil::Date>().is_err() {
                    return Err("the date is not YYYY-MM-DD".into());
                }
                cell(&d.name, false).map_err(|e| format!("the name {e}"))?;
                Ok(())
            })
        }
        "facilities" => {
            let list: Vec<FacilityRow> = field(key, value)?;
            rows_up_to(key, &list, PLACES_MAX, |f| {
                for (what, text, needed) in [
                    ("the record's name", &f.record, true),
                    ("the spoken name", &f.spoken, false),
                    ("the FHIR id", &f.id, false),
                    ("the EAF ID", &f.code, false),
                    ("the address", &f.address, false),
                    ("the phone", &f.phone, false),
                ] {
                    cell(text, needed).map_err(|e| format!("{what} {e}"))?;
                }
                Ok(())
            })?;
            match twice_place(&list, |f| (&f.id, &f.record)) {
                Some(n) => Err(format!("{key}: row {n}: the facility is there twice")),
                None => Ok(()),
            }
        }
        "locations" => {
            let list: Vec<LocationRow> = field(key, value)?;
            rows_up_to(key, &list, PLACES_MAX, |l| {
                for (what, text, needed) in [
                    ("the record's name", &l.record, true),
                    ("the spoken name", &l.spoken, false),
                    ("the FHIR id", &l.id, false),
                    ("the address", &l.address, false),
                    ("the phone", &l.phone, false),
                    ("the facility", &l.facility, false),
                    ("the DEP ID", &l.code, false),
                    ("the specialty", &l.specialty, false),
                ] {
                    cell(text, needed).map_err(|e| format!("{what} {e}"))?;
                }
                if l.hours.len() > 14 {
                    return Err("the hours: at most 14 rows".into());
                }
                l.hours.iter().try_for_each(hours_row).map_err(|e| format!("the hours: {e}"))
            })?;
            match twice_place(&list, |l| (&l.id, &l.record)) {
                Some(n) => Err(format!("{key}: row {n}: the department is there twice")),
                None => Ok(()),
            }
        }
        "specialties" => {
            let list: Vec<SpecialtyRow> = field(key, value)?;
            rows(key, &list, |s| {
                cell(&s.spoken, true).map_err(|e| format!("the spoken name {e}"))?;
                cell(&s.code, false).map_err(|e| format!("the ID {e}"))?;
                words(&s.words).map_err(|e| format!("the caller's words {e}"))?;
                Ok(())
            })
        }
        "booking_note_label" => {
            let text = value.as_str().ok_or_else(|| format!("{key}: text"))?;
            cell(text, false).map_err(|e| format!("{key}: the label {e}"))
        }
        "slots_offered" => whole(key, value, 1, 5).map(|_| ()),
        "narrow_over" => whole(key, value, 1, 10).map(|_| ()),
        "groups_named" => whole(key, value, 1, 5).map(|_| ()),
        "past_days" => whole(key, value, 1, 3650).map(|_| ()),
        "established_years" => whole(key, value, 1, 10).map(|_| ()),
        "sooner_with_others" => value.as_bool().map(|_| ()).ok_or_else(|| format!("{key}: on or off")),
        "ahead_windows" => {
            let list: Vec<i64> = field(key, value)?;
            if list.is_empty() || list.len() > WINDOWS_MAX {
                return Err(format!("{key}: 1 to {WINDOWS_MAX} windows"));
            }
            if list.iter().any(|d| !(1..=365).contains(d)) {
                return Err(format!("{key}: each window is 1 to 365 days"));
            }
            if list.windows(2).any(|w| w[0] >= w[1]) {
                return Err(format!("{key}: each window is longer than the one before"));
            }
            Ok(())
        }
        _ => Err("a setting that is not the hospital's".into()),
    }
}

impl Hospital {
    /// The hospital from its settings. A key missing from `pairs` (a version saved before the
    /// key existed) takes the file's value. Each value is checked.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a Value)>) -> Result<Self, String> {
        let mut map = file_map().clone();
        for (k, v) in pairs {
            if KEYS.contains(&k) {
                map.insert(k.to_string(), v.clone());
            }
        }
        for k in KEYS {
            let v = map.get(*k).ok_or_else(|| format!("{k}: missing"))?;
            check_key(k, v)?;
        }
        let get = |k: &str| &map[k];
        let n = |k: &str| get(k).as_i64().unwrap_or(0);
        let time_zone: String = field("time_zone", get("time_zone"))?;
        let tz = TimeZone::get(&time_zone).map_err(|_| "time_zone: not a known IANA name".to_string())?;
        let visit_groups: Vec<VisitGroup> = field("visit_groups", get("visit_groups"))?;
        let visit_type_list: String = field("visit_type_list", get("visit_type_list"))?;
        let diagnosis_list: String = field("diagnosis_list", get("diagnosis_list"))?;
        let specialty_list: String = field("specialty_list", get("specialty_list"))?;
        let visit_types: Vec<VisitTypeRow> = field("visit_types", get("visit_types"))?;
        let reasons: Vec<ReasonRow> = field("reasons", get("reasons"))?;
        let specialties: Vec<SpecialtyRow> = field("specialties", get("specialties"))?;
        // An ID means nothing without its list: the row names which list must be set.
        if visit_type_list.is_empty() {
            if let Some(i) = visit_types.iter().position(|r| !r.code.trim().is_empty()) {
                return Err(format!("visit_types: row {}: an ID needs the visit type list's OID", i + 1));
            }
        }
        if specialty_list.is_empty() {
            if let Some(i) = specialties.iter().position(|r| !r.code.trim().is_empty()) {
                return Err(format!("specialties: row {}: an ID needs the specialty list's OID", i + 1));
            }
        }
        if diagnosis_list.is_empty() {
            if let Some(i) = reasons.iter().position(|r| r.system == "hospital") {
                return Err(format!("reasons: row {}: the hospital's list needs its OID", i + 1));
            }
        }
        let facilities: Vec<FacilityRow> = field("facilities", get("facilities"))?;
        let locations: Vec<LocationRow> = field("locations", get("locations"))?;
        // A department's facility and specialty name rows elsewhere: each must be there.
        for (i, l) in locations.iter().enumerate() {
            let f = l.facility.trim();
            if !f.is_empty() && !facilities.iter().any(|x| x.id.trim() == f) {
                return Err(format!("locations: row {}: its facility is not on the facility list", i + 1));
            }
            if !l.specialty.trim().is_empty() && specialty_list.is_empty() {
                return Err(format!("locations: row {}: a specialty ID needs the specialty list's OID", i + 1));
            }
        }
        Ok(Self {
            groups: VisitGroups::new(&visit_groups),
            visit_groups,
            tz,
            time_zone,
            visit_type_list,
            diagnosis_list,
            specialty_list,
            visit_types,
            reasons,
            times_of_day: field("times_of_day", get("times_of_day"))?,
            hours: field("hours", get("hours"))?,
            closed_days: field("closed_days", get("closed_days"))?,
            facilities,
            locations,
            specialties,
            slots_offered: n("slots_offered") as usize,
            narrow_over: n("narrow_over") as usize,
            groups_named: n("groups_named") as usize,
            past_days: n("past_days"),
            established_years: n("established_years"),
            sooner_with_others: get("sooner_with_others").as_bool().unwrap_or(true),
            ahead_windows: field("ahead_windows", get("ahead_windows"))?,
            booking_note_label: field("booking_note_label", get("booking_note_label"))?,
        })
    }

    /// The file's hospital: the Default, and the copy for a run without a store.
    pub fn file() -> Arc<Hospital> {
        static PARSED: OnceLock<Arc<Hospital>> = OnceLock::new();
        Arc::clone(PARSED.get_or_init(|| {
            Arc::new(Hospital::from_pairs(std::iter::empty()).expect("ehr/fixtures/hospital.json does not fit"))
        }))
    }

    pub fn tz(&self) -> &TimeZone {
        &self.tz
    }

    /// The visit groups, without any whose name or words name sensitive care.
    pub fn groups(&self) -> &VisitGroups {
        &self.groups
    }

    /// When the narrowing question is asked, and how many groups it names.
    pub fn limits(&self) -> Limits {
        Limits { narrow_over: self.narrow_over, groups_named: self.groups_named }
    }

    /// The time of day the caller named, by its name, in any case.
    pub fn time_of_day(&self, name: &str) -> Option<TimeOfDay> {
        let row = self.times_of_day.iter().find(|t| t.name.eq_ignore_ascii_case(name.trim()))?;
        Some(TimeOfDay { from_minute: minute_of(&row.from)?, minutes: u16::from(row.hours) * 60 })
    }

    /// The names the model may pick for `find_slots`'s time of day.
    pub fn time_names(&self) -> Vec<String> {
        self.times_of_day.iter().map(|t| t.name.clone()).collect()
    }

    /// Whether the call can book both an in-person and an online visit: Epic searches the coded
    /// types, the local fake books any type the agent may book.
    pub fn books_both_modes(&self, epic: bool) -> bool {
        let has = |online: bool| {
            if epic {
                self.coded_bookable().any(|t| t.online == online)
            } else {
                self.fake_type(online, true).is_some() || self.fake_type(online, false).is_some()
            }
        };
        has(true) && has(false)
    }

    /// The note `$book` files with a visit: the caller's words, then the hospital's label
    /// (`booking_note_label`), within Epic's `limit` characters. The label is kept whole; the
    /// caller's words are cut when the two do not fit.
    pub fn booking_note(&self, words: &str, limit: usize) -> String {
        let label = self.booking_note_label.trim();
        if label.is_empty() {
            return words.chars().take(limit).collect();
        }
        let room = limit.saturating_sub(label.chars().count() + 1);
        let words: String = words.trim().chars().take(room).collect();
        format!("{words} {label}")
    }

    /// The visit types Epic's `$find` searches: the agent may book them, and each has a code.
    pub fn coded_bookable(&self) -> impl Iterator<Item = &VisitTypeRow> {
        self.visit_types.iter().filter(|t| t.book && !t.code.trim().is_empty())
    }

    /// The type the local fake gives a slot that is online or not, for a caller who is (or is
    /// not) established: the first the agent may book that fits. None: the fake offers no such
    /// slot.
    pub fn fake_type(&self, online: bool, established: bool) -> Option<&VisitTypeRow> {
        self.visit_types.iter().find(|t| t.book && t.online == online && t.patients.fits(established))
    }

    /// Whether any type the agent may book is for new or for established patients only: then a
    /// search needs the caller's past visits (BOOKING-RULES.md §3.4). With none, no read.
    pub fn sorts_patients(&self) -> bool {
        self.visit_types.iter().any(|t| t.book && t.patients != PatientKind::Either)
    }

    /// Which patients a slot's type is for, by the record's name: "either" for a type not on
    /// the list.
    pub fn patients_of(&self, visit_type: Option<&str>) -> PatientKind {
        visit_type.and_then(|t| self.visit_type(t)).map(|r| r.patients).unwrap_or_default()
    }

    /// The department a place names: by its FHIR id first, then by the record's name.
    fn department(&self, at: &PlaceRef) -> Option<&LocationRow> {
        if let Some(id) = &at.id {
            if let Some(row) = self.locations.iter().find(|l| !l.id.trim().is_empty() && Shielded::new(l.id.trim().to_string()) == *id) {
                return Some(row);
            }
        }
        let name = at.name.as_ref()?;
        self.locations.iter().find(|l| Shielded::new(l.record.trim().to_lowercase()) == *name)
    }

    /// Whether a place is a department of a primary care specialty on the lists.
    pub fn primary_care_at(&self, at: &PlaceRef) -> bool {
        let Some(code) = self.department(at).map(|l| l.specialty.trim()).filter(|s| !s.is_empty()) else { return false };
        self.specialties.iter().any(|s| s.primary_care && s.code.trim() == code)
    }

    /// Whether a caller seen at `seen` (their seen visits in the window) is established at
    /// `at` (BOOKING-RULES.md §3.4): seen at a department of the same specialty, by the Places
    /// list; when `at` has no specialty on the list, seen at that same department. Answers yes
    /// or no and never reveals a place.
    pub fn established(&self, seen: &[PlaceRef], at: &PlaceRef) -> bool {
        let specialty = self.department(at).map(|l| l.specialty.trim()).filter(|s| !s.is_empty());
        seen.iter().any(|v| match specialty {
            Some(s) => self.department(v).is_some_and(|l| l.specialty.trim() == s),
            None => v.same(at),
        })
    }

    /// The row of a visit type, by the record's name, in any case.
    pub fn visit_type(&self, record: &str) -> Option<&VisitTypeRow> {
        let r = record.trim();
        self.visit_types.iter().find(|t| t.record.trim().eq_ignore_ascii_case(r))
    }

    /// What a visit is, from the record's plain words, through the sensitive screen
    /// (visit_about.rs). A type on the list says its spoken name and takes its Online mark; a
    /// spoken name that names sensitive care makes the visit private too. A type not on the
    /// list keeps the record's words, and the word list decides whether it is online.
    pub fn about(&self, visit_type: Option<&str>, reason: Option<&str>) -> VisitAbout {
        let mut about = VisitAbout::from_record(visit_type, reason);
        let Some(row) = visit_type.and_then(|t| self.visit_type(t)) else { return about };
        about.online = row.online;
        let spoken = row.spoken.trim();
        if about.private || spoken.is_empty() {
            return about;
        }
        if names_sensitive(spoken) {
            return VisitAbout { online: row.online, private: true, ..VisitAbout::default() };
        }
        about.visit_type = Some(Shielded::new(spoken.to_string()));
        about
    }

    /// A visit this call booked: the slot's own type, and the caller's reason from the note,
    /// as `VisitAbout::booked` reads them, with the list's words.
    pub fn booked(&self, visit_type: Option<&str>, note: &BookNote) -> VisitAbout {
        let about = self.about(visit_type, None);
        if about.private || note.private {
            return VisitAbout { online: about.online, private: true, ..VisitAbout::default() };
        }
        VisitAbout { reason: Some(note.words.clone()), ..about }
    }

    /// The visit group of a visit type, by the record's name. None for a private visit.
    pub fn group_of(&self, about: &VisitAbout, visit_type: Option<&str>) -> Option<Shielded<String>> {
        if about.private {
            return None;
        }
        self.groups.of(visit_type)
    }

    /// The words for a visit's place: the row's spoken name for the record's Location (by its
    /// FHIR id first, exactly; then by its name, in any case), the record's own words for a
    /// place not on the list, and the first row's when the record names none.
    pub fn place(&self, id: Option<&str>, record: Option<&str>) -> String {
        let spoken = |l: &LocationRow| if l.spoken.trim().is_empty() { l.record.clone() } else { l.spoken.clone() };
        let id = id.map(str::trim).filter(|i| !i.is_empty());
        if let Some(row) = id.and_then(|i| self.locations.iter().find(|l| l.id.trim() == i)) {
            return spoken(row);
        }
        match record.map(str::trim).filter(|r| !r.is_empty()) {
            Some(r) => self.locations.iter().find(|l| l.record.trim().eq_ignore_ascii_case(r)).map(spoken).unwrap_or_else(|| r.to_string()),
            None => self.locations.first().map(spoken).unwrap_or_default(),
        }
    }
}

fn file_map() -> &'static Map<String, Value> {
    static MAP: OnceLock<Map<String, Value>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: Map<String, Value> = serde_json::from_str(SOURCE).expect("ehr/fixtures/hospital.json does not parse");
        map.remove("about");
        map
    })
}

/// A version saved before the owner's A (2026-11-15) held an OID on each visit type and
/// specialty row, and an OID as a reason's system. Read it in today's shape: when every row of
/// a list names the same OID and the version has no list key, that OID becomes the list's; a
/// reason's SNOMED or ICD-10-CM OID becomes its name, and another OID becomes "hospital" with
/// that OID as the diagnosis list. Rows that name two OIDs are left as they are, so the
/// version fails its check and the call uses the file, as for any version that does not fit.
/// `values` are (key, JSON text), as the store holds them.
pub fn upgrade(values: &mut Vec<(String, String)>) {
    let has = |values: &[(String, String)], key: &str| values.iter().any(|(k, _)| k == key);
    let mut lists: Vec<(String, String)> = Vec::new();
    for (list_key, rows_key) in LISTS {
        let Some(slot) = values.iter_mut().find(|(k, _)| k == rows_key) else { continue };
        let Ok(Value::Array(mut rows)) = serde_json::from_str::<Value>(&slot.1) else { continue };
        let named: Vec<String> = rows.iter().filter_map(|r| r.get("system")?.as_str().map(str::to_string)).filter(|s| !s.is_empty()).collect();
        let one = named.first().cloned();
        if named.iter().any(|s| Some(s) != one.as_ref()) {
            continue;
        }
        for r in &mut rows {
            if let Some(o) = r.as_object_mut() {
                o.remove("system");
            }
        }
        slot.1 = Value::Array(rows).to_string();
        if let Some(oid) = one {
            lists.push((list_key.to_string(), oid));
        }
    }
    if let Some(slot) = values.iter_mut().find(|(k, _)| k == "reasons") {
        if let Ok(Value::Array(mut rows)) = serde_json::from_str::<Value>(&slot.1) {
            let mut own: Option<String> = None;
            let mut two = false;
            for r in &mut rows {
                let Some(s) = r.get("system").and_then(Value::as_str).map(str::to_string) else { continue };
                if s.is_empty() || REASON_SYSTEMS.iter().any(|(n, _)| *n == s) {
                    continue;
                }
                let name = match REASON_SYSTEMS.iter().find(|(_, o)| !o.is_empty() && *o == s) {
                    Some((n, _)) => n.to_string(),
                    None => {
                        two |= own.as_ref().is_some_and(|o| *o != s);
                        own.get_or_insert(s);
                        "hospital".to_string()
                    }
                };
                r["system"] = Value::String(name);
            }
            if !two {
                slot.1 = Value::Array(rows).to_string();
                if let Some(oid) = own {
                    lists.push(("diagnosis_list".to_string(), oid));
                }
            }
        }
    }
    for (key, oid) in lists {
        if !has(values, &key) {
            values.push((key, Value::String(oid).to_string()));
        }
    }
}

/// The file's values, by key: the Default version the agent seeds (dev_server.py).
pub fn file_values() -> Vec<(&'static str, Value)> {
    KEYS.iter().map(|k| (*k, file_map()[*k].clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_file_fits_and_holds_every_key() {
        let map = file_map();
        for k in KEYS {
            assert!(map.contains_key(*k), "{k} is not in hospital.json");
        }
        assert_eq!(map.len(), KEYS.len(), "hospital.json holds a key that is not a setting");
        let h = Hospital::file();
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

    #[test]
    fn the_fake_and_epic_each_have_a_type_to_book() {
        let h = Hospital::file();
        for established in [true, false] {
            assert!(h.fake_type(false, established).is_some() && h.fake_type(true, established).is_some());
        }
        assert!(h.coded_bookable().any(|t| t.online) && h.coded_bookable().any(|t| !t.online));
    }

    #[test]
    fn the_default_gives_new_and_established_callers_their_own_in_person_type() {
        let h = Hospital::file();
        assert!(h.sorts_patients());
        assert_eq!(h.fake_type(false, true).unwrap().record, "Office Visit");
        assert_eq!(h.fake_type(false, false).unwrap().record, "New Patient Appointment");
        // Online is for either.
        assert_eq!(h.fake_type(true, true), h.fake_type(true, false));
        assert_eq!(h.established_years, 3);
    }

    #[test]
    fn established_by_the_specialty_else_by_the_same_department() {
        let list = json!("urn:oid:1.2.840.114350.1.13.0.1.7.10.686980.110");
        let specialties = json!([{"code": "9", "spoken": "family medicine"}, {"code": "4", "spoken": "cardiology"}]);
        let locations = json!([
            {"record": "North Family Medicine", "id": "d1", "specialty": "9"},
            {"record": "South Family Medicine", "id": "d2", "specialty": "9"},
            {"record": "EMC Cardiology", "id": "d3", "specialty": "4"},
            {"record": "Main Street Clinic"}
        ]);
        let h = Hospital::from_pairs([("specialty_list", &list), ("specialties", &specialties), ("locations", &locations)]).unwrap();
        let at = |id: Option<&str>, name: &str| PlaceRef::new(id, Some(name));
        let seen_north = [at(Some("d1"), "North Family Medicine")];
        // Seen by family medicine at another department: established in family medicine.
        assert!(h.established(&seen_north, &at(Some("d2"), "South Family Medicine")));
        // Not seen by cardiology: new there.
        assert!(!h.established(&seen_north, &at(Some("d3"), "EMC Cardiology")));
        // A place with no specialty on the list: the same department only, by name in any case.
        let seen_main = [at(None, "  main street clinic ")];
        assert!(h.established(&seen_main, &at(None, "Main Street Clinic")));
        assert!(!h.established(&seen_main, &at(None, "Lab Annex")));
        // Nothing seen: new everywhere.
        assert!(!h.established(&[], &at(Some("d1"), "North Family Medicine")));
        assert!(PatientKind::New.fits(false) && !PatientKind::New.fits(true));
        assert!(PatientKind::Established.fits(true) && !PatientKind::Established.fits(false));
        assert!(PatientKind::Either.fits(true) && PatientKind::Either.fits(false));
    }

    #[test]
    fn a_version_from_before_the_column_reads_as_either_and_needs_no_read() {
        let old = json!([{"code": "", "record": "Office Visit", "spoken": "", "online": false, "book": true}]);
        let h = Hospital::from_pairs([("visit_types", &old)]).expect("an old version");
        assert_eq!(h.visit_types[0].patients, PatientKind::Either);
        assert!(!h.sorts_patients(), "no type for one kind: no past-visit read");
        assert!(check_key("visit_types", &json!([{"record": "A", "patients": "returning"}])).is_err());
        assert!(check_key("established_years", &json!(0)).is_err());
        assert!(check_key("established_years", &json!(11)).is_err());
    }

    #[test]
    fn a_type_on_the_list_says_its_spoken_name_and_its_online_mark() {
        let h = Hospital::from_pairs([(
            "visit_types",
            &json!([
                {"record": "MyChart Video Visit", "spoken": "a video visit", "online": true},
                {"record": "Clinic Visit", "spoken": "a clinic visit", "online": false},
                {"record": "Virtual Check", "spoken": "", "online": false},
                {"record": "Quiet Room", "spoken": "a counseling visit"}
            ]),
        )])
        .unwrap();
        // Shielded compares without revealing.
        let is = |v: &Option<Shielded<String>>, want: &str| v.as_ref() == Some(&Shielded::new(want.to_string()));
        let a = h.about(Some("mychart video visit"), Some("Cough"));
        assert!(a.online && !a.private);
        assert!(is(&a.visit_type, "a video visit"));
        // The row's mark wins over the word list, both ways.
        assert!(!h.about(Some("Virtual Check"), None).online);
        assert!(!h.about(Some("Clinic Visit"), None).online);
        // A type not on the list keeps the record's words and the word list.
        let b = h.about(Some("Telehealth Follow Up"), None);
        assert!(b.online && is(&b.visit_type, "Telehealth Follow Up"));
        // A spoken name that names sensitive care makes the visit private.
        assert!(h.about(Some("Quiet Room"), None).private);
    }

    #[test]
    fn a_place_says_its_spoken_name_or_the_record_s_words() {
        let h = Hospital::from_pairs([(
            "locations",
            &json!([
                {"record": "Main Street Clinic"},
                {"record": "EMC Family Medicine", "spoken": "the family medicine clinic"}
            ]),
        )])
        .unwrap();
        assert_eq!(h.place(None, Some("emc family medicine")), "the family medicine clinic");
        assert_eq!(h.place(None, Some("North Lab")), "North Lab");
        assert_eq!(h.place(None, None), "Main Street Clinic");
        assert_eq!(h.place(None, Some("  ")), "Main Street Clinic");
    }

    #[test]
    fn a_place_is_found_by_its_fhir_id_first() {
        let facilities = json!([{"record": "EHS Hospital", "id": "f1"}, {"record": "EHS Clinic", "id": "f2"}]);
        let locations = json!([
            {"record": "Family Medicine", "id": "d1", "facility": "f1", "spoken": "the hospital's family clinic"},
            {"record": "Family Medicine", "id": "d2", "facility": "f2", "spoken": "the north family clinic"}
        ]);
        let h = Hospital::from_pairs([("facilities", &facilities), ("locations", &locations)]).expect("two of one name, two ids");
        assert_eq!(h.place(Some("d2"), Some("Family Medicine")), "the north family clinic");
        // An id not on the list: the name decides.
        assert_eq!(h.place(Some("d9"), Some("family medicine")), "the hospital's family clinic");
    }

    #[test]
    fn a_department_names_a_facility_on_the_list() {
        let facilities = json!([{"record": "EHS Hospital", "id": "f1"}]);
        let stray = json!([{"record": "Cardiology", "id": "d1", "facility": "f9"}]);
        let e = Hospital::from_pairs([("facilities", &facilities), ("locations", &stray)]).err().unwrap();
        assert_eq!(e, "locations: row 1: its facility is not on the facility list");
        let none = json!("");
        let coded = json!([{"record": "Cardiology", "specialty": "4"}]);
        let no_specialties = json!([]);
        let e = Hospital::from_pairs([("specialty_list", &none), ("specialties", &no_specialties), ("locations", &coded)]).err().unwrap();
        assert!(e.starts_with("locations: row 1"), "{e}");
        // The same id twice is one place twice.
        let twice = json!([{"record": "A", "id": "d1"}, {"record": "B", "id": "d1"}]);
        assert!(check_key("locations", &twice).is_err());
        // Hours are checked as the hospital's are.
        let late = json!([{"record": "A", "hours": [{"day": "mon", "opens": "17:00", "closes": "08:00"}]}]);
        assert!(check_key("locations", &late).unwrap_err().starts_with("locations: row 1: the hours"));
    }

    #[test]
    fn a_directory_search_is_a_state_or_a_city_and_state() {
        assert_eq!(PlaceSearch::parse(" WI "), Some(PlaceSearch { city: None, state: "WI".into() }));
        assert_eq!(PlaceSearch::parse("Verona, WI"), Some(PlaceSearch { city: Some("Verona".into()), state: "WI".into() }));
        assert_eq!(PlaceSearch::parse("St. Mary's, PA").unwrap().city.as_deref(), Some("St. Mary's"));
        for bad in ["", "wi", "Wisconsin", "Verona", "Verona, ZZ", ", WI", "Ver0na, WI", "a&b, WI"] {
            assert_eq!(PlaceSearch::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_version_from_before_the_facilities_still_fits() {
        // Its locations have none of the new fields, and it has no facilities key.
        let old = json!([{"record": "EMC Cardiology", "spoken": "", "id": "", "address": "", "phone": ""}]);
        let h = Hospital::from_pairs([("locations", &old)]).expect("an old version");
        assert!(h.locations[0].facility.is_empty() && h.locations[0].hours.is_empty());
    }

    #[test]
    fn a_bad_list_is_refused_by_its_row_never_its_value() {
        let bad = [
            ("time_zone", json!("Mars/Olympus")),
            ("visit_type_list", json!("1.2.840.114350")),
            ("visit_type_list", json!("urn:oid:1.2..3")),
            ("diagnosis_list", json!(7)),
            ("visit_types", json!([{"record": "A", "system": "urn:oid:1.2"}])),
            ("reasons", json!([{"words": ["a"], "system": "urn:oid:2.16.840.1.113883.6.96"}])),
            ("reasons", json!([{"words": ["a"], "code": "1"}])),
            ("visit_types", json!([{"record": "A"}, {"record": "a"}])),
            ("visit_types", json!([{"record": "A", "extra": 1}])),
            ("times_of_day", json!([{"name": "Late Night", "from": "22:00", "hours": 1}])),
            ("times_of_day", json!([{"name": "late", "from": "22:00", "hours": 3}])),
            ("hours", json!([{"day": "monday", "opens": "08:00", "closes": "17:00"}])),
            ("hours", json!([{"day": "mon", "opens": "17:00", "closes": "08:00"}])),
            ("closed_days", json!([{"date": "2026-02-30"}])),
            ("reasons", json!([{"words": []}])),
            ("ahead_windows", json!([30, 7])),
            ("slots_offered", json!(9)),
            ("locations", json!([{"record": "Line\none"}])),
        ];
        for (key, value) in bad {
            let e = check_key(key, &value).expect_err(key);
            assert!(e.starts_with(key), "{e}");
            assert!(!e.contains("Mars") && !e.contains("Line"), "an error holds a value: {e}");
        }
    }

    #[test]
    fn the_note_keeps_the_label_whole_and_cuts_the_words() {
        let h = Hospital::file();
        assert_eq!(h.booking_note("headache", 500), "headache (automated transcription from patient)");
        let long = "a".repeat(600);
        let note = h.booking_note(&long, 500);
        assert_eq!(note.chars().count(), 500);
        assert!(note.ends_with(" (automated transcription from patient)"));
        let none = json!("");
        let plain = Hospital::from_pairs([("booking_note_label", &none)]).unwrap();
        assert_eq!(plain.booking_note("headache", 500), "headache");
    }

    #[test]
    fn both_modes_only_when_both_can_be_booked() {
        let h = Hospital::file();
        assert!(h.books_both_modes(false) && h.books_both_modes(true));
        let one = json!([
            {"record": "Office Visit", "book": true},
            {"code": "1400000001", "record": "MyChart Video Visit", "online": true, "book": true}
        ]);
        let h = Hospital::from_pairs([("visit_types", &one)]).unwrap();
        assert!(h.books_both_modes(false), "the fake books both");
        assert!(!h.books_both_modes(true), "Epic has only the online type with an ID");
    }

    #[test]
    fn an_id_needs_its_list() {
        let none = json!("");
        let typed = json!([{"record": "A", "code": "1003"}]);
        let e = Hospital::from_pairs([("visit_type_list", &none), ("visit_types", &typed)]).err().unwrap();
        assert_eq!(e, "visit_types: row 1: an ID needs the visit type list's OID");
        let own = json!([{"words": ["a"], "system": "hospital", "code": "9"}]);
        let e = Hospital::from_pairs([("diagnosis_list", &none), ("reasons", &own)]).err().unwrap();
        assert!(e.starts_with("reasons: row 1"), "{e}");
        // With no ID, no list is needed: the local fake's own types.
        let plain = json!([{"record": "Office Visit", "book": true}]);
        assert!(Hospital::from_pairs([("visit_type_list", &none), ("visit_types", &plain)]).is_ok());
    }

    /// The store's (key, JSON text) pairs, as `settings::get` returns them.
    fn stored(pairs: &[(&str, Value)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn parsed(values: &[(String, String)]) -> Vec<(String, Value)> {
        values.iter().map(|(k, v)| (k.clone(), serde_json::from_str(v).unwrap())).collect()
    }

    #[test]
    fn a_version_from_before_a_reads_in_today_s_shape() {
        let prc = "urn:oid:1.2.840.114350.1.13.0.1.7.2.808267";
        let edg = "urn:oid:1.2.840.114350.1.13.861.1.7.2.696871";
        let mut values = stored(&[
            ("visit_types", json!([
                {"system": "", "code": "", "record": "Office Visit"},
                {"system": prc, "code": "1003", "record": "New Patient Appointment"}
            ])),
            ("reasons", json!([
                {"system": "urn:oid:2.16.840.1.113883.6.96", "code": "46866001", "words": ["broken leg"]},
                {"system": edg, "code": "121346631", "words": ["leg"]}
            ])),
            ("specialties", json!([{"system": "", "code": "", "spoken": "dermatology"}])),
        ]);
        upgrade(&mut values);
        let p = parsed(&values);
        let h = Hospital::from_pairs(p.iter().map(|(k, v)| (k.as_str(), v))).expect("it fits");
        assert_eq!(h.visit_type_list, prc);
        assert_eq!(h.diagnosis_list, edg);
        assert_eq!(h.visit_types[1].code, "1003");
        assert_eq!(h.reasons[0].system, "snomed");
        assert_eq!(h.reasons[1].system, "hospital");
        // No row named a specialty list: the file's value stays.
        assert_eq!(h.specialty_list, Hospital::file().specialty_list);
    }

    #[test]
    fn rows_that_name_two_lists_are_not_merged() {
        let mut values = stored(&[(
            "visit_types",
            json!([
                {"system": "urn:oid:1.2.3", "code": "1", "record": "A"},
                {"system": "urn:oid:1.2.4", "code": "2", "record": "B"}
            ]),
        )]);
        upgrade(&mut values);
        let p = parsed(&values);
        assert!(Hospital::from_pairs(p.iter().map(|(k, v)| (k.as_str(), v))).is_err(), "the version must not fit");
        assert!(!values.iter().any(|(k, _)| k == "visit_type_list"));
    }
}
