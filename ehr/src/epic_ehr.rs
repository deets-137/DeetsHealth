//! Epic's sandbox behind ehr's adapter traits (EPIC.md §5.2, step E-3; the owner's fork 2A,
//! 2026-11-12). Dev only: a call that opens with `EHR_SOURCE_EPIC_SANDBOX` reads here.
//!
//! - [`EpicPatients`]: `Patient?family&given&birthdate`. Epic's sandbox finds nothing for a
//!   name alone or a date alone; with all three it forgives a slip in the name ("Lopes" finds
//!   Lopez). Our own [`crate::verify::one_match`] then decides, as for every EHR.
//! - [`EpicScheduler`]: the next visit from `Appointment?patient&service-category=appointment
//!   &date`, in widening windows so a patient with hundreds of visits costs one small read.
//!   Finding and booking slots use the fake's slots moved to the next weekdays, until the
//!   booking through STU3 `$find` and `$book` is built (E-4, EPIC.md §5.2).
//! - [`EpicEhr::sandbox_list`]: the rail's list (`ehr epic-sandbox`).
//!
//! Every value from Epic goes straight into `Shielded`, except in `sandbox_list`, which is for
//! the dev page and reads only the public test patients. Errors name a status, never a value.

use std::{path::Path, sync::Arc};

use deetshealth_core::{ehr_request::EhrPass, Shielded};
use jiff::{tz::TimeZone, SignedDuration, Timestamp};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    epic::Epic,
    scheduler::{BookResult, FakeScheduler, Scheduler, SchedulerDown, Slot, SlotId, SlotWish},
    session::PatientKey,
    verify::{Candidate, Claimed, Down, Patients},
    visit_about::VisitAbout,
};

/// The list of Epic's public test patients, built into the binary.
const LIST: &str = include_str!("../fixtures/epic-sandbox.json");

/// The windows the next-visit search widens through, in days from now.
const VISIT_WINDOWS: [i64; 3] = [7, 90, 400];
/// The rail shows this many upcoming visits per patient, and a count of the rest.
const RAIL_VISITS: usize = 10;
/// How far ahead the rail looks for visits, in days.
const RAIL_DAYS: i64 = 60;

#[derive(Deserialize)]
pub struct SandboxList {
    pub time_zone: String,
    pub place: String,
    pub patients: Vec<ListedPatient>,
}

#[derive(Deserialize)]
pub struct ListedPatient {
    pub id: String,
}

pub fn sandbox_list_file() -> SandboxList {
    serde_json::from_str(LIST).expect("ehr/fixtures/epic-sandbox.json does not parse")
}

/// Epic's sandbox, as ehr's adapters.
pub struct EpicEhr {
    epic: Arc<Epic>,
    tz: TimeZone,
    pub patients: EpicPatients,
    pub scheduler: EpicScheduler,
}

impl EpicEhr {
    pub fn new(epic: Epic) -> Result<Self, String> {
        let list = sandbox_list_file();
        let tz = TimeZone::get(&list.time_zone).map_err(|_| "epic-sandbox.json: unknown time_zone")?;
        let epic = Arc::new(epic);
        let today = Timestamp::now().to_zoned(tz.clone()).date();
        Ok(Self {
            patients: EpicPatients { epic: Arc::clone(&epic) },
            scheduler: EpicScheduler {
                epic: Arc::clone(&epic),
                tz: tz.clone(),
                place: list.place.clone(),
                stand_in: FakeScheduler::from_weekdays_after(today, list.place),
            },
            epic,
            tz,
        })
    }

    /// The rail's list (CHAT.md §13): each listed test patient as Epic has them now, and their
    /// visits in the next 60 days, in the shape of `ehr/fixtures/sandbox.json`. Public fakes
    /// only. About two reads per patient.
    pub async fn sandbox_list(&self) -> Result<Value, String> {
        let list = sandbox_list_file();
        let now = Timestamp::now();
        let mut patients = Vec::new();
        let mut visits = Vec::new();
        let mut more = serde_json::Map::new();
        for listed in &list.patients {
            let url = self.epic.base().join(&format!("Patient/{}", listed.id)).map_err(|_| "bad patient id")?;
            let resource = self.epic.read(url).await.map_err(|e| e.to_string())?;
            let Some(p) = patient_fields(&resource, now) else { continue };
            patients.push(json!({
                "id": p.id,
                "given": p.given,
                "family": p.family,
                "birth_date": p.birth_date,
                "phone": p.phone.unwrap_or_default(),
            }));
            let mut ahead = self
                .appointments(&p.id, now, now + SignedDuration::from_hours(24 * RAIL_DAYS))
                .await
                .map_err(|_| "Appointment search failed")?;
            ahead.sort_by_key(|a| a.0);
            if ahead.len() > RAIL_VISITS {
                more.insert(p.id.clone(), json!(ahead.len() - RAIL_VISITS));
            }
            for (_, v) in ahead.into_iter().take(RAIL_VISITS) {
                visits.push(json!({
                    "patient": p.id,
                    "starts_at": v.starts_at,
                    "minutes": v.minutes,
                    "clinician": v.clinician,
                    "place": v.place,
                    "visit_type": v.visit_type,
                    "reason": v.reason,
                }));
            }
        }
        Ok(json!({ "place": list.place, "patients": patients, "visits": visits, "more": more }))
    }

    async fn appointments(&self, patient: &str, from: Timestamp, to: Timestamp) -> Result<Vec<(Timestamp, Visit)>, ()> {
        appointments(&self.epic, &self.tz, &self.scheduler.place, patient, from, to).await
    }
}

/// Finds the caller in Epic's sandbox.
pub struct EpicPatients {
    epic: Arc<Epic>,
}

impl Patients for EpicPatients {
    async fn candidates(&self, claimed: &Claimed) -> Result<Vec<Candidate>, Down> {
        let mut url = self.epic.base().join("Patient").map_err(|_| Down)?;
        {
            // The values go to Epic only, encoded, and nowhere else.
            let pass = EhrPass::issue();
            url.query_pairs_mut()
                .append_pair("family", claimed.family.reveal(&pass))
                .append_pair("given", claimed.given.reveal(&pass))
                .append_pair("birthdate", claimed.birth_date.reveal(&pass));
        }
        let bundle = self.epic.read(url).await.map_err(|_| Down)?;
        Ok(entries(&bundle, "Patient")
            .filter_map(|r| patient_fields(r, Timestamp::now()))
            .map(|p| Candidate {
                patient: PatientKey(Shielded::new(p.id)),
                given: Shielded::new(p.given),
                family: Shielded::new(p.family),
                birth_date: Shielded::new(p.birth_date),
                phone: p.phone.map(Shielded::new),
            })
            .collect())
    }
}

/// Visits in Epic's sandbox; slots from the stand-in.
pub struct EpicScheduler {
    epic: Arc<Epic>,
    tz: TimeZone,
    place: String,
    stand_in: FakeScheduler,
}

impl Scheduler for EpicScheduler {
    async fn find(&self, patient: &PatientKey, wish: &SlotWish) -> Result<Vec<Slot>, SchedulerDown> {
        self.stand_in.find(patient, wish).await
    }

    async fn book(&self, patient: &PatientKey, slot: &SlotId) -> Result<BookResult, SchedulerDown> {
        self.stand_in.book(patient, slot).await
    }

    async fn next_visit(&self, patient: &PatientKey) -> Result<Option<Slot>, SchedulerDown> {
        let now = Timestamp::now();
        let pass = EhrPass::issue();
        let id = patient.0.reveal(&pass).clone();
        let mut found = None;
        for days in VISIT_WINDOWS {
            let to = now + SignedDuration::from_hours(24 * days);
            let ahead = appointments(&self.epic, &self.tz, &self.place, &id, now, to)
                .await
                .map_err(|_| SchedulerDown)?;
            if let Some((_, v)) = ahead.into_iter().min_by(|a, b| a.0.cmp(&b.0)) {
                found = Some(v.into_slot());
                break;
            }
        }
        // A visit booked on this call (the stand-in) counts too.
        let booked = self.stand_in.booked_for(patient);
        Ok(found.into_iter().chain(booked).min_by(|a, b| a.starts_at.cmp(&b.starts_at)))
    }
}

/// One visit, read from an Appointment, in the place's local time.
struct Visit {
    id: String,
    starts_at: String,
    minutes: u32,
    clinician: String,
    place: String,
    /// `serviceType`: Epic's visit type ("MyChart Video Visit").
    visit_type: Option<String>,
    /// `reasonCode`: the reason for the visit, when the scheduler entered one.
    reason: Option<String>,
}

impl Visit {
    fn into_slot(self) -> Slot {
        let about = VisitAbout::from_record(self.visit_type.as_deref(), self.reason.as_deref());
        Slot {
            id: SlotId(format!("epic-appointment-{}", self.id)),
            starts_at: Shielded::new(self.starts_at),
            minutes: self.minutes,
            clinician: Shielded::new(self.clinician),
            place: Shielded::new(self.place),
            about,
        }
    }
}

/// The patient's visits still to come that start in [from, to), with their start instants.
async fn appointments(
    epic: &Epic,
    tz: &TimeZone,
    default_place: &str,
    patient: &str,
    from: Timestamp,
    to: Timestamp,
) -> Result<Vec<(Timestamp, Visit)>, ()> {
    let mut url = epic.base().join("Appointment").map_err(|_| ())?;
    url.query_pairs_mut()
        .append_pair("patient", patient)
        .append_pair("service-category", "appointment")
        .append_pair("date", &format!("ge{}", from.round(jiff::Unit::Second).map_err(|_| ())?))
        .append_pair("date", &format!("lt{}", to.round(jiff::Unit::Second).map_err(|_| ())?));
    let bundle = epic.read(url).await.map_err(|_| ())?;
    Ok(entries(&bundle, "Appointment")
        .filter_map(|a| appointment(a, tz, default_place))
        .filter(|(start, _)| *start >= from && *start < to)
        .collect())
}

/// The resources of one type in a search Bundle (Epic adds an OperationOutcome when it finds
/// nothing).
fn entries<'a>(bundle: &'a Value, kind: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    bundle["entry"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| &e["resource"])
        .filter(move |r| r["resourceType"] == kind)
}

struct PatientFields {
    id: String,
    given: String,
    family: String,
    birth_date: String,
    phone: Option<String>,
}

/// The fields verification needs. The official name, else the first; its first given name
/// (Epic lists the middle name after it). The mobile number only: a code never goes to a work
/// or home line. Which numbers count becomes a hospital setting (FUTURE-SETTINGS.md §8).
fn patient_fields(r: &Value, now: Timestamp) -> Option<PatientFields> {
    let names = r["name"].as_array()?;
    let name = names.iter().find(|n| n["use"] == "official").or(names.first())?;
    Some(PatientFields {
        id: r["id"].as_str()?.to_string(),
        given: name["given"][0].as_str()?.to_string(),
        family: name["family"].as_str()?.to_string(),
        birth_date: r["birthDate"].as_str()?.to_string(),
        phone: code_number(r, now),
    })
}

/// The number the code may go to (FUTURE-SETTINGS.md §8.1): a `mobile` phone that has not
/// ended, the patient's first choice by `rank` (1 is first; no rank comes after every rank),
/// then the first in the chart's list.
fn code_number(r: &Value, now: Timestamp) -> Option<String> {
    r["telecom"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["system"] == "phone" && t["use"] == "mobile")
        .filter(|t| !ended(&t["period"]["end"], now))
        .filter_map(|t| Some((t["rank"].as_u64().unwrap_or(u64::MAX), e164(t["value"].as_str()?)?)))
        // min_by_key keeps the first of equal ranks, so the chart's own order breaks a tie.
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, number)| number)
}

/// Whether a ContactPoint's `period.end` is in the past. No end: still in use. An end that
/// does not parse counts as ended: a code never goes to a number we cannot read as current.
/// A date with no time ends at the close of that day (UTC).
fn ended(end: &Value, now: Timestamp) -> bool {
    let Some(end) = end.as_str() else {
        return !end.is_null();
    };
    if let Ok(t) = end.parse::<Timestamp>() {
        return t <= now;
    }
    match end.parse::<jiff::civil::Date>() {
        Ok(day) => day < now.to_zoned(TimeZone::UTC).date(),
        Err(_) => true,
    }
}

/// A US number as +E.164 ("+1 469-469-4321" → "+14694694321"), or none.
fn e164(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    match digits.len() {
        10 => Some(format!("+1{digits}")),
        11 if digits.starts_with('1') => Some(format!("+{digits}")),
        _ => None,
    }
}

/// One Appointment as a visit, if it is still on (booked, pending, arrived or checked in).
fn appointment(a: &Value, tz: &TimeZone, default_place: &str) -> Option<(Timestamp, Visit)> {
    let status = a["status"].as_str()?;
    if !matches!(status, "booked" | "pending" | "arrived" | "checked-in") {
        return None;
    }
    let start: Timestamp = a["start"].as_str()?.parse().ok()?;
    let minutes = a["minutesDuration"].as_u64().map(|m| m as u32).or_else(|| {
        let end: Timestamp = a["end"].as_str()?.parse().ok()?;
        u32::try_from(end.duration_since(start).as_mins()).ok()
    })?;
    let actor = |kind: &str| {
        a["participant"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| &p["actor"])
            .find(|actor| actor["reference"].as_str().is_some_and(|r| r.starts_with(kind)))
            .and_then(|actor| actor["display"].as_str())
            .map(str::to_string)
    };
    let local = start.to_zoned(tz.clone());
    Some((
        start,
        Visit {
            id: a["id"].as_str()?.to_string(),
            starts_at: local.strftime("%Y-%m-%dT%H:%M").to_string(),
            minutes,
            clinician: actor("Practitioner/").unwrap_or_default(),
            place: actor("Location/").unwrap_or_else(|| default_place.to_string()),
            visit_type: concept_words(&a["serviceType"][0]),
            reason: concept_words(&a["reasonCode"][0]),
        },
    ))
}

/// A CodeableConcept's words: its text, else its first coding's display. Never `comment`
/// (visit_about.rs).
fn concept_words(c: &Value) -> Option<String> {
    c["text"]
        .as_str()
        .or_else(|| c["coding"].as_array()?.iter().find_map(|x| x["display"].as_str()))
        .map(str::to_string)
}

/// Builds the Epic adapters from the dev files, or says why not.
pub async fn connect_sandbox(client_file: &Path, key_file: &Path) -> Result<EpicEhr, String> {
    let config = crate::epic::EpicConfig::sandbox(client_file, key_file)?;
    let epic = Epic::connect(config).await.map_err(|e| e.to_string())?;
    EpicEhr::new(epic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_parses_and_its_zone_is_known() {
        let list = sandbox_list_file();
        assert!(!list.patients.is_empty());
        TimeZone::get(&list.time_zone).unwrap();
    }

    /// The shapes Epic's sandbox sent on 2026-11-12 (EPIC.md §5.2), cut to the fields read.
    #[test]
    fn a_patient_gives_the_official_first_given_name_and_the_mobile_number() {
        let r = json!({
            "resourceType": "Patient", "id": "p1", "birthDate": "1987-09-12",
            "name": [{"use": "usual", "family": "X", "given": ["Y"]},
                     {"use": "official", "family": "Lopez", "given": ["Camila", "Maria"]}],
            "telecom": [{"system": "phone", "value": "+1 469-888-8888", "use": "work"},
                        {"system": "phone", "value": "+1 469-469-4321", "use": "mobile"},
                        {"system": "email", "value": "a@b.c"}]
        });
        let p = patient_fields(&r, Timestamp::now()).unwrap();
        assert_eq!((p.given.as_str(), p.family.as_str()), ("Camila", "Lopez"));
        assert_eq!(p.phone.as_deref(), Some("+14694694321"));
    }

    #[test]
    fn a_patient_with_only_a_work_number_has_no_number_for_a_code() {
        let r = json!({
            "resourceType": "Patient", "id": "p1", "birthDate": "1987-09-12",
            "name": [{"family": "Lopez", "given": ["Camila"]}],
            "telecom": [{"system": "phone", "value": "+1 469-888-8888", "use": "work"}]
        });
        assert!(patient_fields(&r, Timestamp::now()).unwrap().phone.is_none());
    }

    #[test]
    fn an_appointment_is_read_in_the_place_s_local_time() {
        let tz = TimeZone::get("America/Chicago").unwrap();
        let a = json!({
            "resourceType": "Appointment", "id": "a1", "status": "booked",
            "start": "2026-11-12T14:00:00Z", "end": "2026-11-12T14:30:00Z", "minutesDuration": 30,
            "serviceType": [{"coding": [{"code": "1400000001", "display": "MyChart Video Visit"}]}],
            "comment": "Synthetic sandbox booking latency test\n",
            "participant": [
                {"actor": {"reference": "Patient/p1", "display": "Lopez, Camila Maria"}},
                {"actor": {"reference": "Practitioner/d1", "display": "Physician Family Medicine, MD"}},
                {"actor": {"reference": "Location/l1", "display": "EMC Family Medicine"}}
            ]
        });
        let (_, v) = appointment(&a, &tz, "Epic sandbox").unwrap();
        assert_eq!(v.starts_at, "2026-11-12T08:00");
        assert_eq!(v.minutes, 30);
        assert_eq!(v.clinician, "Physician Family Medicine, MD");
        assert_eq!(v.place, "EMC Family Medicine");
        // The visit type from serviceType's coding; no reasonCode, so no reason; the
        // scheduler's comment is never read.
        assert_eq!(v.visit_type.as_deref(), Some("MyChart Video Visit"));
        assert_eq!(v.reason, None);
        let cancelled = json!({"id": "a2", "status": "cancelled", "start": "2026-11-12T14:00:00Z", "minutesDuration": 30});
        assert!(appointment(&cancelled, &tz, "Epic sandbox").is_none());
    }

    fn with_phones(telecom: Value) -> Value {
        json!({
            "resourceType": "Patient", "id": "p1", "birthDate": "1987-09-12",
            "name": [{"family": "Lopez", "given": ["Camila"]}],
            "telecom": telecom
        })
    }

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    /// FUTURE-SETTINGS.md §8.1: a number whose period has ended never gets the code.
    #[test]
    fn an_ended_mobile_number_gets_no_code() {
        let now = at("2026-11-13T12:00:00Z");
        let old = |end: &str| json!({"system": "phone", "use": "mobile", "value": "+1 469-469-0001", "period": {"end": end}});
        let current = json!({"system": "phone", "use": "mobile", "value": "+1 469-469-4321"});
        // An old number first, then the current one: the current one wins.
        let r = with_phones(json!([old("2024-03-01T00:00:00Z"), current]));
        assert_eq!(code_number(&r, now).as_deref(), Some("+14694694321"));
        // Only an old number: no number at all. A date-only end, yesterday, counts as ended.
        assert_eq!(code_number(&with_phones(json!([old("2026-11-12")])), now), None);
        // An end that cannot be read counts as ended.
        assert_eq!(code_number(&with_phones(json!([old("soon")])), now), None);
        // An end later today, or in the future, is still in use.
        let today = with_phones(json!([old("2026-11-13")]));
        assert_eq!(code_number(&today, now).as_deref(), Some("+14694690001"));
        let later = with_phones(json!([old("2027-01-01T00:00:00Z")]));
        assert_eq!(code_number(&later, now).as_deref(), Some("+14694690001"));
    }

    /// FUTURE-SETTINGS.md §8.1: the patient's first choice by rank, then the chart's order.
    #[test]
    fn the_code_goes_to_the_best_ranked_mobile_number() {
        let now = at("2026-11-13T12:00:00Z");
        let phone = |value: &str, rank: Option<u64>| match rank {
            Some(r) => json!({"system": "phone", "use": "mobile", "value": value, "rank": r}),
            None => json!({"system": "phone", "use": "mobile", "value": value}),
        };
        let r = with_phones(json!([phone("4690000003", Some(3)), phone("4690000001", Some(1)), phone("4690000009", None)]));
        assert_eq!(code_number(&r, now).as_deref(), Some("+14690000001"));
        // No rank comes after every rank.
        let r = with_phones(json!([phone("4690000009", None), phone("4690000002", Some(2))]));
        assert_eq!(code_number(&r, now).as_deref(), Some("+14690000002"));
        // Equal ranks, or none: the first in the chart's list.
        let r = with_phones(json!([phone("4690000005", None), phone("4690000006", None)]));
        assert_eq!(code_number(&r, now).as_deref(), Some("+14690000005"));
        // A better-ranked work number never counts.
        let work = json!({"system": "phone", "use": "work", "value": "4690000000", "rank": 1});
        let r = with_phones(json!([work, phone("4690000004", Some(4))]));
        assert_eq!(code_number(&r, now).as_deref(), Some("+14690000004"));
    }

    /// The EHR request pass is issued in this file only (core/src/ehr_request.rs).
    #[test]
    fn only_this_file_names_the_ehr_pass() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let needle = ["Ehr", "Pass"].concat();
        for entry in std::fs::read_dir(src).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == "epic_ehr.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains(&needle), "{} names the EHR pass", path.display());
        }
    }

    #[test]
    fn numbers_become_e164_or_nothing() {
        assert_eq!(e164("(785) 785-4321").as_deref(), Some("+17857854321"));
        assert_eq!(e164("1-785-785-4321").as_deref(), Some("+17857854321"));
        assert_eq!(e164("4321"), None);
    }
}
