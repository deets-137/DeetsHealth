//! Epic's sandbox behind ehr's adapter traits (EPIC.md §5.2, step E-3; the owner's fork 2A,
//! 2026-11-12). Dev only: a call that opens with `EHR_SOURCE_EPIC_SANDBOX` reads here.
//!
//! - [`EpicPatients`]: `Patient?family&given&birthdate`. Epic's sandbox finds nothing for a
//!   name alone or a date alone; with all three it forgives a slip in the name ("Lopes" finds
//!   Lopez). Our own [`crate::verify::one_match`] then decides, as for every EHR.
//! - [`EpicScheduler`]: the caller's visits from `Appointment?patient&service-category=appointment
//!   &status&date`: a day or a month the caller named, the past, or the visits to come in a
//!   90-day window, then 400 (EPIC.md §7.10).
//!   Slots from STU3 `$find`, bookings by `$book` (E-4, EPIC.md §7.10): each booking writes to
//!   Epic's shared sandbox.
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
    hospital::{
        ClinicianRef, FacilityRow, Hospital, HoursRow, LocationRow, PlaceRef, PlaceSearch, Places, SpecialtyRow, VisitTypeRow, PLACES_MAX,
    },
    scheduler::{BookResult, FoundSlots, FoundVisits, Scheduler, SchedulerDown, Slot, SlotId, SlotWish, VisitAsk, VisitStatus},
    session::PatientKey,
    verify::{Candidate, Claimed, Down, Patients},
    visit_about::BookNote,
};

/// The list of Epic's public test patients, built into the binary.
const LIST: &str = include_str!("../fixtures/epic-sandbox.json");

/// The statuses each search asks Epic for (EPIC.md §7.10): the visits to come, the past, and
/// every status a caller may hear of, for a day or a month they named.
const UPCOMING: &str = "booked,pending,arrived,checked-in";
const PAST: &str = "fulfilled,noshow,cancelled,arrived,checked-in";
const ANY: &str = "booked,pending,arrived,checked-in,fulfilled,noshow,cancelled";
/// The rail shows this many upcoming visits per patient, and a count of the rest.
const RAIL_VISITS: usize = 10;
/// How far ahead the rail looks for visits, in days.
const RAIL_DAYS: i64 = 60;

/// The rail's list of Epic's test patients. The hospital's setup (the time zone, the visit
/// types `$find` searches, the groups) is Config › Hospital (hospital.rs).
#[derive(Deserialize)]
pub struct SandboxList {
    /// The rail's words for a visit that names no place.
    pub place: String,
    pub patients: Vec<ListedPatient>,
}

/// The longest `appointmentNote` (EPIC.md §7.2: Epic's `comment` holds 500 characters).
const MAX_NOTE: usize = 500;

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
    pub patients: EpicPatients,
    pub scheduler: EpicScheduler,
}

impl EpicEhr {
    pub fn new(epic: Epic) -> Result<Self, String> {
        let epic = Arc::new(epic);
        Ok(Self {
            patients: EpicPatients { epic: Arc::clone(&epic) },
            scheduler: EpicScheduler { epic: Arc::clone(&epic) },
            epic,
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
            ahead.retain(|a| a.1.status == VisitStatus::Upcoming);
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

    /// The hospital's places from Epic's directory (HOSPITAL-CONFIG.md §8; EPIC.md §6):
    /// `Location.Search (Organizational Directory) (R4)`, one search per entry of `searches`,
    /// each search's `next` pages followed. A state searches `address=<state>`; a city and its
    /// state search `address=<city>` and `address-state=<state>` (Epic's search takes `address`;
    /// it filters by `address-state` after, EPIC.md §2.2). Facilities (EAF) and departments (DEP)
    /// with their specialties and hours; anything else is counted, not kept. A location is a fact
    /// about the hospital, never a patient's. Errors name a status or a count, never a value.
    pub async fn places(&self, searches: &[PlaceSearch]) -> Result<Places, String> {
        let mut found: Vec<Value> = Vec::new();
        let mut reads = 0usize;
        for search in searches {
            let mut url = self.epic.base().join("Location").map_err(|_| "bad URL")?;
            match &search.city {
                Some(city) => {
                    url.query_pairs_mut().append_pair("address", city).append_pair("address-state", &search.state);
                }
                None => {
                    url.query_pairs_mut().append_pair("address", &search.state);
                }
            }
            let mut next = Some(url);
            while let Some(page) = next.take() {
                if reads >= PLACES_PAGES {
                    return Err(format!("Location search: more than {PLACES_PAGES} pages"));
                }
                reads += 1;
                let bundle = self.epic.read(page).await.map_err(|e| e.to_string())?;
                for l in entries(&bundle, "Location") {
                    let id = l["id"].as_str().unwrap_or_default();
                    if !id.is_empty() && !found.iter().any(|f| f["id"].as_str() == Some(id)) {
                        found.push(l.clone());
                    }
                }
                next = next_link(&bundle).and_then(|u| reqwest::Url::parse(&u).ok());
            }
        }
        let mut places = places_from(&found)?;
        places.reads = reads;
        Ok(places)
    }

    /// `ehr epic-book`: one booking for a public test patient, by the call's own code. Prints
    /// what came back (shapes, outcomes, counts), never a value. A write to Epic's sandbox.
    pub async fn probe_book(&self, patient: &str) -> Result<Vec<String>, String> {
        let record = self
            .epic
            .read(self.epic.base().join(&format!("Patient/{patient}")).map_err(|_| "bad URL")?)
            .await
            .map_err(|e| e.to_string())?;
        let key = PatientKey(Shielded::new(patient.to_string()));
        let wish = SlotWish {
            note: BookNote::screen("DeetsHealth test booking, synthetic caller".into()),
            earliest_day: None,
            latest_day: None,
            mode: crate::scheduler::VisitMode::Online,
            time_of_day: None,
            days: Vec::new(),
            clinician: None,
            who: Some(Shielded::new(demographics(&record))),
            seen: Vec::new(),
            place: None,
        };
        let mut out = Vec::new();
        let h = Hospital::file();
        let found = self.scheduler.find(&h, &key, &wish).await.map_err(|_| "$find: no answer")?;
        out.push(format!("$find: {} slot(s)", found.slots.len()));
        let Some(slot) = found.slots.first() else { return Ok(out) };
        let before = self.scheduler.visits(&h, &key, &VisitAsk { range: None, past: false }).await.map_err(|_| "search: no answer")?.visits.len();
        let result = self.scheduler.book(&h, &key, &slot.id, &wish.note).await.map_err(|_| "$book: no answer")?;
        match result {
            BookResult::Booked(v) => {
                out.push("$book: booked".into());
                out.push(format!(
                    "  CSN: {}; instructions: {}; type: {}; online: {}; the note as reason: {}",
                    if v.csn.is_some() { "yes" } else { "no" },
                    if v.about.instructions.is_some() { "yes" } else { "none" },
                    if v.about.visit_type.is_some() { "yes" } else { "no" },
                    v.about.online,
                    if v.about.reason.is_some() { "yes" } else { "no" },
                ));
                out.push(format!("  same start as the slot: {}", v.starts_at == slot.starts_at));
            }
            BookResult::Taken => out.push("$book: refused (taken)".into()),
            BookResult::NotThrough => out.push("$book: no answer, and no booking found".into()),
        }
        let after = self.scheduler.visits(&h, &key, &VisitAsk { range: None, past: false }).await.map_err(|_| "search: no answer")?.visits.len();
        out.push(format!("visits to come in the first window: {before} before, {after} after"));
        Ok(out)
    }

    /// `ehr epic-find`'s last step: the real slot search for a public test patient, as a call
    /// would run it. Counts only, never a value. Read only.
    pub async fn probe_find(&self, patient: &str, record: &Value) -> Vec<String> {
        let key = PatientKey(Shielded::new(patient.to_string()));
        let who = Shielded::new(demographics(record));
        let mut out = Vec::new();
        // The visit search by window, and with no end at all (date=ge only): how many come back,
        // how fast, and whether Epic pages them. FindVisit has 3 s (ehr_client.py).
        for days in [Some(7), Some(30), Some(90), None] {
            let now = Timestamp::now();
            let mut url = match self.epic.base().join("Appointment") {
                Ok(u) => u,
                Err(_) => continue,
            };
            url.query_pairs_mut()
                .append_pair("patient", patient)
                .append_pair("service-category", "appointment")
                .append_pair("status", UPCOMING)
                .append_pair("date", &format!("ge{}", now.round(jiff::Unit::Second).unwrap_or(now)));
            if let Some(d) = days {
                let to = now + SignedDuration::from_hours(24 * d);
                url.query_pairs_mut().append_pair("date", &format!("lt{}", to.round(jiff::Unit::Second).unwrap_or(to)));
            }
            let started = std::time::Instant::now();
            let label = days.map_or("no end".to_string(), |d| format!("{d} days"));
            out.push(match self.epic.read(url).await {
                Ok(b) => {
                    let links: Vec<&str> = b["link"].as_array().into_iter().flatten().filter_map(|l| l["relation"].as_str()).collect();
                    let last = entries(&b, "Appointment").filter_map(|a| a["start"].as_str()).max().map(|s| s.chars().take(10).collect::<String>());
                    format!(
                        "visit search, {label}: {} visit(s), total {}, links {links:?}, the latest on {}, in {} ms",
                        entries(&b, "Appointment").count(),
                        b["total"],
                        last.unwrap_or_default(),
                        started.elapsed().as_millis()
                    )
                }
                Err(e) => format!("visit search, {label}: {e} in {} ms", started.elapsed().as_millis()),
            });
        }
        for (label, mode) in [("online", crate::scheduler::VisitMode::Online), ("either", crate::scheduler::VisitMode::Either)] {
            let wish = SlotWish {
                note: BookNote::screen("a probe".into()),
                earliest_day: None,
                latest_day: None,
                mode,
                time_of_day: None,
                days: Vec::new(),
                clinician: None,
                who: Some(who.clone()),
                seen: Vec::new(),
                place: None,
            };
            match self.scheduler.find(&Hospital::file(), &key, &wish).await {
                Ok(found) => {
                    let online = found.slots.iter().filter(|s| s.about.online).count();
                    out.push(format!("EpicScheduler::find, {label}: {} slot(s), {online} online, {} next link(s)", found.slots.len(), found.next.len()));
                    if !found.next.is_empty() {
                        out.push(match self.scheduler.find_more(&Hospital::file(), &key, &found.next).await {
                            Ok(more) => format!("  find_more: {} slot(s), {} next link(s)", more.slots.len(), more.next.len()),
                            Err(_) => "  find_more: no answer".into(),
                        });
                    }
                }
                Err(_) => out.push(format!("EpicScheduler::find, {label}: no answer")),
            }
        }
        out
    }

    /// The rail's visits, in the file hospital's time zone: the rail is not a call, and
    /// `ehr epic-sandbox` runs with no store.
    async fn appointments(&self, patient: &str, from: Timestamp, to: Timestamp) -> Result<Vec<(Timestamp, Visit)>, ()> {
        appointments(&self.epic, Hospital::file().tz(), patient, from, to, UPCOMING).await
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
            .filter_map(|r| Some((patient_fields(r, Timestamp::now())?, demographics(r), pcps(r))))
            .map(|(p, facts, pcp)| Candidate {
                patient: PatientKey(Shielded::new(p.id)),
                given: Shielded::new(p.given),
                family: Shielded::new(p.family),
                birth_date: Shielded::new(p.birth_date),
                phone: p.phone.map(Shielded::new),
                facts: Some(Shielded::new(facts)),
                pcp,
            })
            .collect())
    }
}

/// The patient's PCPs (`generalPractitioner`): each Practitioner reference, by its FHIR id and
/// display (step 0, EPIC.md §7.12: relative `Practitioner/{id}` references with a display; one
/// test patient has two). A PractitionerRole or Organization reference is not a clinician.
fn pcps(r: &Value) -> Vec<ClinicianRef> {
    r["generalPractitioner"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let reference = g["reference"].as_str()?;
            if !(reference.starts_with("Practitioner/") || reference.contains("/Practitioner/")) {
                return None;
            }
            let id = reference.rsplit('/').next().filter(|i| !i.is_empty());
            Some(ClinicianRef::new(id, g["display"].as_str()))
        })
        .collect()
}

/// Visits in Epic's sandbox; slots from `$find`, bookings by `$book` (E-4, EPIC.md §7.10). Each
/// booking writes to Epic's shared sandbox.
/// The visit types, time zone, places and groups are the call's hospital (hospital.rs).
pub struct EpicScheduler {
    epic: Arc<Epic>,
}

impl EpicScheduler {
    fn stu3(&self, path: &str) -> Result<reqwest::Url, SchedulerDown> {
        let base = reqwest::Url::parse(&self.epic.base().as_str().replace("/R4/", "/STU3/")).map_err(|_| SchedulerDown)?;
        base.join(path).map_err(|_| SchedulerDown)
    }

    /// One `$find` (EPIC.md §7.10, the probe's shape of 2026-11-15): the patient's
    /// demographics, the start and the end, one visit type, the time of day and the days.
    async fn find_one(&self, h: &Hospital, who: &Value, kind: &VisitTypeRow, wish: &SlotWish, pass: &EhrPass) -> Result<FoundSlots, SchedulerDown> {
        let at = |day: &Shielded<String>, plus: i64| -> Result<Timestamp, SchedulerDown> {
            let d: jiff::civil::Date = day.reveal(pass).parse().map_err(|_| SchedulerDown)?;
            let d = d.checked_add(jiff::Span::new().days(plus)).map_err(|_| SchedulerDown)?;
            Ok(d.to_zoned_datetime(h.tz().clone()).map_err(|_| SchedulerDown)?.timestamp())
        };
        let now = Timestamp::now().round(jiff::Unit::Second).map_err(|_| SchedulerDown)?;
        let start = match &wish.earliest_day {
            Some(d) => at(d, 0)?.max(now),
            None => now,
        };
        let mut parameter = vec![
            json!({"name": "patient", "resource": who}),
            json!({"name": "startTime", "valueDateTime": start.to_string()}),
            json!({"name": "service-type", "valueCodeableConcept": {"coding": [
                {"system": h.visit_type_list, "code": kind.code, "display": kind.record}
            ]}}),
        ];
        if let Some(d) = &wish.latest_day {
            parameter.push(json!({"name": "endTime", "valueDateTime": at(d, 1)?.to_string()}));
        }
        // The caller's usual place (BOOKING-RULES.md §3.2): a Location reference, as Epic's sample
        // sends it (the probe of 2026-11-15 took it). A place with no FHIR id sends nothing; the
        // service keeps only that place's slots either way.
        if let Some(id) = wish.place.as_ref().and_then(|p| p.id.as_ref()) {
            parameter.push(json!({"name": "location-reference", "valueReference": {"reference": format!("Location/{}", id.reveal(pass))}}));
        }
        if wish.time_of_day.is_some() || !wish.days.is_empty() {
            let mut repeat = json!({});
            if let Some(t) = wish.time_of_day {
                repeat["timeOfDay"] = json!([t.start()]);
                repeat["duration"] = json!(t.minutes);
                repeat["durationUnit"] = json!("min");
            }
            if !wish.days.is_empty() {
                repeat["dayOfWeek"] = json!(wish.days.iter().map(|d| day_code(*d)).collect::<Vec<_>>());
            }
            parameter.push(json!({"name": "time-of-day", "valueTiming": {"repeat": repeat}}));
        }
        let body = json!({"resourceType": "Parameters", "parameter": parameter});
        let (status, reply) = self.epic.post(self.stu3("Appointment/$find")?, &body).await.map_err(|_| SchedulerDown)?;
        if !(200..300).contains(&status) {
            return Err(SchedulerDown);
        }
        Ok(page(&reply.unwrap_or_default(), h))
    }

    /// After a lost `$book` reply: the patient's booked visit at that start, if Epic has one.
    async fn booked_at(&self, h: &Hospital, patient: &str, start: Timestamp) -> Result<Option<Slot>, SchedulerDown> {
        let found = appointments(&self.epic, h.tz(), patient, start, start + SignedDuration::from_mins(1), UPCOMING)
            .await
            .map_err(|_| SchedulerDown)?;
        Ok(found.into_iter().next().map(|(_, v)| v.into_slot(h)))
    }
}

impl Scheduler for EpicScheduler {
    async fn find(&self, h: &Hospital, _: &PatientKey, wish: &SlotWish) -> Result<FoundSlots, SchedulerDown> {
        let pass = EhrPass::issue();
        let who: Value = wish.who.as_ref().and_then(|w| serde_json::from_str(w.reveal(&pass)).ok()).ok_or(SchedulerDown)?;
        // One `$find` per coded visit type that fits: in person, online, or both (2 reads with
        // the Default list). A type with no code is the local fake's only.
        let mut found = FoundSlots::default();
        for kind in h.coded_bookable().filter(|t| wish.mode.fits(t.online)) {
            let page = self.find_one(h, &who, kind, wish, &pass).await?;
            found.slots.extend(page.slots);
            found.next.extend(page.next);
        }
        found.slots.sort_by(|a, b| a.starts_at.cmp(&b.starts_at));
        Ok(found)
    }

    /// Epic's next page, from the link it gave: a POST with no parameters to that link (a GET
    /// answers 403; the probe, 2026-11-15). `post` holds it to Epic's own host. An expired page
    /// (error 4113) or any refusal gives none: the caller hears there are no more, and a new
    /// search starts over.
    async fn find_more(&self, h: &Hospital, _: &PatientKey, next: &[String]) -> Result<FoundSlots, SchedulerDown> {
        let Some(first) = next.first() else { return Ok(FoundSlots::default()) };
        let url = reqwest::Url::parse(first).map_err(|_| SchedulerDown)?;
        let bundle = match self.epic.post(url, &json!({"resourceType": "Parameters"})).await {
            Ok((200..=299, Some(b))) => b,
            Ok(_) => return Ok(FoundSlots::default()),
            Err(_) => return Err(SchedulerDown),
        };
        let mut page = page(&bundle, h);
        page.next.extend(next[1..].iter().cloned());
        Ok(page)
    }

    /// `$book` (EPIC.md §7.4): the patient and the proposed visit, each a `valueIdentifier`
    /// holding its FHIR id, and the caller's reason as `appointmentNote`, revealed here with the
    /// `EhrPass`. Then one `Appointment.Read` of the new visit for its instructions. A lost
    /// reply is never sent again blind: a search at that time decides (the owner's V2A).
    async fn book(&self, h: &Hospital, patient: &PatientKey, slot: &SlotId, note: &BookNote) -> Result<BookResult, SchedulerDown> {
        let Some((proposed, start)) = slot.0.strip_prefix("epic-proposed-").and_then(|s| s.split_once('@')) else {
            return Ok(BookResult::Taken);
        };
        let start: Timestamp = start.parse().map_err(|_| SchedulerDown)?;
        let pass = EhrPass::issue();
        let id = patient.0.reveal(&pass).clone();
        let words = h.booking_note(note.words.reveal(&pass), MAX_NOTE);
        let body = json!({"resourceType": "Parameters", "parameter": [
            {"name": "patient", "valueIdentifier": {"value": id}},
            {"name": "appointment", "valueIdentifier": {"value": proposed}},
            {"name": "appointmentNote", "valueString": words},
        ]});
        let sent = self.epic.post(self.stu3("Appointment/$book")?, &body).await;
        let now = Timestamp::now();
        let booked = match sent {
            Ok((200..=299, Some(reply))) => {
                let a = if reply["resourceType"] == "Bundle" { entries(&reply, "Appointment").next().cloned() } else { Some(reply) };
                a.filter(|a| a["status"] == "booked").and_then(|a| appointment(&a, h.tz(), now).map(|(_, v)| (a, v)))
            }
            // Epic said no: the slot is gone, or the booking is refused.
            Ok((400..=499, _)) => return Ok(BookResult::Taken),
            _ => None,
        };
        let Some((resource, visit)) = booked else {
            return Ok(match self.booked_at(h, &id, start).await? {
                Some(found) => BookResult::Booked(found),
                None => BookResult::NotThrough,
            });
        };
        // The new visit's instructions: the `$book` reply has none (EPIC.md §7.4). One read.
        let instructions = match resource["id"].as_str().and_then(|i| self.epic.base().join(&format!("Appointment/{i}")).ok()) {
            Some(url) => self.epic.read(url).await.ok().and_then(|r| r["patientInstruction"].as_str().map(str::to_string)),
            None => None,
        };
        let visit_type = visit.visit_type.clone();
        let mut slot = visit.into_slot(h);
        slot.about = h.booked(visit_type.as_deref(), note).with_instructions(instructions.as_deref());
        if slot.about.private {
            slot.group = None;
        }
        Ok(BookResult::Booked(slot))
    }

    async fn visits(&self, h: &Hospital, patient: &PatientKey, ask: &VisitAsk) -> Result<FoundVisits, SchedulerDown> {
        let now = Timestamp::now();
        let pass = EhrPass::issue();
        let id = patient.0.reveal(&pass).clone();
        let search = |from: Timestamp, to: Timestamp, statuses: &'static str| {
            appointments(&self.epic, h.tz(), &id, from, to, statuses)
        };
        let days = |n: i64| SignedDuration::from_hours(24 * n);
        let mut within_days = None;
        let mut found = match &ask.range {
            // A day or a month the caller named, in the place's time zone: one read.
            Some(r) => {
                let at = |day: &Shielded<String>| -> Result<Timestamp, SchedulerDown> {
                    let d: jiff::civil::Date = day.reveal(&pass).parse().map_err(|_| SchedulerDown)?;
                    Ok(d.to_zoned_datetime(h.tz().clone()).map_err(|_| SchedulerDown)?.timestamp())
                };
                search(at(&r.from)?, at(&r.to)?, ANY).await.map_err(|_| SchedulerDown)?
            }
            None if ask.past => {
                let mut v = search(now - days(h.past_days), now, PAST).await.map_err(|_| SchedulerDown)?;
                v.retain(|a| a.1.status != VisitStatus::Upcoming);
                v
            }
            // The visits to come: the hospital's windows (7, 30, 90 days by default), then no
            // end (one read per window, and one more; most patients with a visit soon cost one
            // small read).
            None => {
                let mut v = Vec::new();
                for &d in &h.ahead_windows {
                    v = search(now, now + days(d), UPCOMING).await.map_err(|_| SchedulerDown)?;
                    if !v.is_empty() {
                        within_days = Some(d as u32);
                        break;
                    }
                }
                if v.is_empty() {
                    // No end: a search with no upper date returns them all (the probe,
                    // 2026-11-15: 269 visits, not paged, as fast as 90 days).
                    v = search(now, Timestamp::MAX, UPCOMING).await.map_err(|_| SchedulerDown)?;
                }
                v
            }
        };
        found.sort_by_key(|a| a.0);
        let mut visits: Vec<Slot> = found.into_iter().map(|(_, v)| v.into_slot(h)).collect();
        if ask.past && ask.range.is_none() {
            visits.reverse();
        }
        Ok(FoundVisits { visits, within_days })
    }
}

/// One visit, read from an Appointment, in the place's local time.
struct Visit {
    id: String,
    starts_at: String,
    minutes: u32,
    clinician: String,
    /// The Practitioner's FHIR id: the PCP is matched by it (BOOKING-RULES.md §3.2).
    clinician_id: Option<String>,
    /// The Location's name, as the record holds it; none when it names no place.
    place: Option<String>,
    /// The Location's FHIR id: the hospital's list matches it first.
    place_id: Option<String>,
    /// `serviceType`: Epic's visit type ("MyChart Video Visit").
    visit_type: Option<String>,
    /// `reasonCode`: the reason for the visit, when the scheduler entered one.
    reason: Option<String>,
    /// `patientInstruction`: how to prepare (EPIC.md §7.8).
    instructions: Option<String>,
    /// The CSN, from `identifier` (EPIC.md §7.3).
    csn: Option<String>,
    status: VisitStatus,
}

impl Visit {
    /// The visit in the hospital's words: its type's spoken name and Online mark, its place's
    /// spoken name, its group.
    fn into_slot(self, h: &Hospital) -> Slot {
        let about = h.about(self.visit_type.as_deref(), self.reason.as_deref()).with_instructions(self.instructions.as_deref());
        let group = h.group_of(&about, self.visit_type.as_deref());
        let clinician_ref = ClinicianRef::new(self.clinician_id.as_deref(), Some(&self.clinician));
        Slot {
            id: SlotId(format!("epic-appointment-{}", self.id)),
            starts_at: Shielded::new(self.starts_at),
            minutes: self.minutes,
            clinician: Shielded::new(self.clinician),
            clinician_ref,
            place: Shielded::new(h.place(self.place_id.as_deref(), self.place.as_deref())),
            about,
            csn: self.csn.map(Shielded::new),
            status: self.status,
            group,
            place_ref: PlaceRef::new(self.place_id.as_deref(), self.place.as_deref()),
            kind: h.patients_of(self.visit_type.as_deref()),
        }
    }
}

/// The patient's visits with one of `statuses` that start in [from, to), with their start
/// instants.
async fn appointments(
    epic: &Epic,
    tz: &TimeZone,
    patient: &str,
    from: Timestamp,
    to: Timestamp,
    statuses: &str,
) -> Result<Vec<(Timestamp, Visit)>, ()> {
    let mut url = epic.base().join("Appointment").map_err(|_| ())?;
    let now = Timestamp::now();
    url.query_pairs_mut()
        .append_pair("patient", patient)
        .append_pair("service-category", "appointment")
        .append_pair("status", statuses)
        .append_pair("date", &format!("ge{}", from.round(jiff::Unit::Second).map_err(|_| ())?));
    // `Timestamp::MAX`: no end, so no upper date at all.
    if to != Timestamp::MAX {
        url.query_pairs_mut().append_pair("date", &format!("lt{}", to.round(jiff::Unit::Second).map_err(|_| ())?));
    }
    let bundle = epic.read(url).await.map_err(|_| ())?;
    Ok(entries(&bundle, "Appointment")
        .filter_map(|a| appointment(a, tz, now))
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
        Ok(day) => day < now.to_zoned_datetime(TimeZone::UTC).date(),
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

/// One Appointment as a visit, if its status is one a caller may hear (`VisitStatus::from_epic`:
/// to come, seen, missed, cancelled). `now` decides whether its time has gone.
fn appointment(a: &Value, tz: &TimeZone, now: Timestamp) -> Option<(Timestamp, Visit)> {
    let start: Timestamp = a["start"].as_str()?.parse().ok()?;
    let status = VisitStatus::from_epic(a["status"].as_str()?, start < now)?;
    let minutes = a["minutesDuration"].as_u64().map(|m| m as u32).or_else(|| {
        let end: Timestamp = a["end"].as_str()?.parse().ok()?;
        u32::try_from(end.duration_since(start).as_mins()).ok()
    })?;
    let local = start.to_zoned_datetime(tz.clone());
    Some((
        start,
        Visit {
            id: a["id"].as_str()?.to_string(),
            starts_at: local.strftime("%Y-%m-%dT%H:%M").to_string(),
            minutes,
            clinician: actor(a, "Practitioner/").unwrap_or_default(),
            clinician_id: actor_id(a, "Practitioner/"),
            place: actor(a, "Location/"),
            place_id: actor_id(a, "Location/"),
            // The visit type's words, else the visit's short subject line (EPIC.md §7.3).
            visit_type: concept_words(&a["serviceType"][0])
                .or_else(|| a["description"].as_str().map(str::to_string)),
            reason: concept_words(&a["reasonCode"][0]),
            instructions: a["patientInstruction"].as_str().map(str::to_string),
            csn: csn(a),
            status,
        },
    ))
}

/// One page of proposed visits from `$find`, and its `next` link.
fn page(bundle: &Value, h: &Hospital) -> FoundSlots {
    let slots = entries(bundle, "Appointment").filter_map(|a| proposed(a, h)).collect();
    let next = bundle["link"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["relation"] == "next")
        .filter_map(|l| l["url"].as_str())
        .map(str::to_string)
        .collect();
    FoundSlots { slots, next }
}

/// A proposed visit from `$find`, in its own department's time zone (the timezone
/// extension, EPIC.md §7.2), else the hospital's. Its id carries the start, so a lost
/// `$book` reply can be checked by a search at that time (V2A).
fn proposed(a: &Value, h: &Hospital) -> Option<Slot> {
    let tz = h.tz();
    if a["status"] != "proposed" {
        return None;
    }
    let start: Timestamp = a["start"].as_str()?.parse().ok()?;
    let zone = a["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|x| x["url"] == "http://hl7.org/fhir/StructureDefinition/timezone")
        .and_then(|x| x["valueCode"].as_str())
        .and_then(|z| TimeZone::get(z).ok())
        .unwrap_or_else(|| tz.clone());
    let minutes = a["minutesDuration"].as_u64().map(|m| m as u32).or_else(|| {
        let end: Timestamp = a["end"].as_str()?.parse().ok()?;
        u32::try_from(end.duration_since(start).as_mins()).ok()
    })?;
    let visit_type = concept_words(&a["serviceType"][0]);
    let about = h.about(visit_type.as_deref(), None);
    let group = h.group_of(&about, visit_type.as_deref());
    let (place_id, place) = (actor_id(a, "Location/"), actor(a, "Location/"));
    let clinician = actor(a, "Practitioner/");
    Some(Slot {
        id: SlotId(format!("epic-proposed-{}@{}", a["id"].as_str()?, start)),
        starts_at: Shielded::new(start.to_zoned_datetime(zone).strftime("%Y-%m-%dT%H:%M").to_string()),
        minutes,
        clinician_ref: ClinicianRef::new(actor_id(a, "Practitioner/").as_deref(), clinician.as_deref()),
        clinician: Shielded::new(clinician.unwrap_or_default()),
        place: Shielded::new(h.place(place_id.as_deref(), place.as_deref())),
        about,
        csn: None,
        status: VisitStatus::Upcoming,
        group,
        place_ref: PlaceRef::new(place_id.as_deref(), place.as_deref()),
        kind: h.patients_of(visit_type.as_deref()),
    })
}

/// A participant's display name, by the kind of its reference ("Practitioner/"). `$find` sends
/// absolute references (`https://…/Practitioner/…`), a search relative ones. A scheduled
/// resource, such as a machine, is never a Practitioner, so it is never read as the clinician.
fn actor(a: &Value, kind: &str) -> Option<String> {
    a["participant"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| &p["actor"])
        .find(|actor| {
            actor["reference"].as_str().is_some_and(|r| r.starts_with(kind) || r.contains(&format!("/{kind}")))
        })
        .and_then(|actor| actor["display"].as_str())
        .map(str::to_string)
}

/// The most pages one import reads: the sandbox's 435 locations came in one.
const PLACES_PAGES: usize = 50;
/// A department's master file (DEP: 68 69 80) and a facility's (EAF: 69 65 70), as the end of
/// an identifier's system (EPIC.md §2.2; HOSPITAL-CONFIG.md §2.10).
const DEP_SYSTEM: &str = ".7.2.686980";
const EAF_SYSTEM: &str = ".7.2.696570";

/// A search Bundle's `next` link.
fn next_link(bundle: &Value) -> Option<String> {
    bundle["link"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|l| l["relation"] == "next")
        .and_then(|l| l["url"].as_str())
        .map(str::to_string)
}

/// Directory text in one cell: one line, at most the hospital's cell length.
fn cell_text(s: &str) -> String {
    let one: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    one.trim().chars().take(crate::hospital::CELL_MAX).collect()
}

/// The ID of the identifier whose system ends with `file`.
fn record_id(l: &Value, file: &str) -> Option<String> {
    l["identifier"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| i["system"].as_str().is_some_and(|s| s.ends_with(file)))
        .and_then(|i| i["value"].as_str())
        .map(cell_text)
}

/// "1 Main St, Verona, WI 53593": the lines, the city, the state and the postal code.
fn address_text(a: &Value) -> String {
    let mut parts: Vec<String> = a["line"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    if let Some(city) = a["city"].as_str() {
        parts.push(city.to_string());
    }
    let state_zip = [a["state"].as_str(), a["postalCode"].as_str()].into_iter().flatten().collect::<Vec<_>>().join(" ");
    if !state_zip.is_empty() {
        parts.push(state_zip);
    }
    cell_text(&parts.join(", "))
}

/// The first phone number in `telecom`.
fn phone_text(l: &Value) -> String {
    l["telecom"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["system"] == "phone")
        .and_then(|t| t["value"].as_str())
        .map(cell_text)
        .unwrap_or_default()
}

/// `hoursOfOperation` as the hospital's hours rows: one per day, "08:00:00" read as "08:00"; all
/// day is 00:00 to 23:59. A row that does not fit is left out. At most 14.
fn hours_rows(l: &Value) -> Vec<HoursRow> {
    let hhmm = |t: &Value| t.as_str().and_then(|s| s.get(..5)).map(str::to_string);
    let mut out = Vec::new();
    for h in l["hoursOfOperation"].as_array().into_iter().flatten() {
        let (opens, closes) = if h["allDay"] == true {
            (Some("00:00".to_string()), Some("23:59".to_string()))
        } else {
            (hhmm(&h["openingTime"]), hhmm(&h["closingTime"]))
        };
        let (Some(opens), Some(closes)) = (opens, closes) else { continue };
        for day in h["daysOfWeek"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let row = HoursRow { day: day.to_string(), opens: opens.clone(), closes: closes.clone() };
            if crate::hospital::check_key("hours", &json!([row])).is_ok() && out.len() < 14 {
                out.push(row);
            }
        }
    }
    out
}

/// The department's specialty: its system and code, and the directory's words.
fn specialty_of(l: &Value) -> Option<(String, String, String)> {
    l["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["url"].as_str().is_some_and(|u| u.ends_with("/specialty")))
        .flat_map(|e| e["valueCodeableConcept"]["coding"].as_array().cloned().unwrap_or_default())
        .find_map(|c| {
            let system = c["system"].as_str().filter(|s| s.starts_with("urn:oid:"))?;
            let code = c["code"].as_str().filter(|s| !s.is_empty())?;
            Some((system.to_string(), cell_text(code), cell_text(c["display"].as_str().unwrap_or(code))))
        })
}

/// The import's rows from the Locations a search returned: a facility has an EAF identifier, a
/// department a DEP identifier and, mostly, `partOf` its facility. Facilities sorted by name;
/// departments by their facility's name, then their own.
fn places_from(found: &[Value]) -> Result<Places, String> {
    let mut p = Places::default();
    for l in found {
        let id = cell_text(l["id"].as_str().unwrap_or_default());
        let record = cell_text(l["name"].as_str().unwrap_or_default());
        if id.is_empty() || record.is_empty() {
            p.others += 1;
            continue;
        }
        if let Some(code) = record_id(l, DEP_SYSTEM) {
            let facility = l["partOf"]["reference"].as_str().and_then(|r| r.rsplit('/').next()).map(cell_text).unwrap_or_default();
            let mut specialty = String::new();
            if let Some((system, code, words)) = specialty_of(l) {
                if p.specialty_list.is_empty() {
                    p.specialty_list = system.clone();
                }
                if system == p.specialty_list {
                    if !p.specialties.iter().any(|s| s.code == code) {
                        p.specialties.push(SpecialtyRow { code: code.clone(), spoken: words.to_lowercase(), words: vec![], primary_care: false });
                    }
                    specialty = code;
                } else {
                    p.other_specialty_lists += 1;
                }
            }
            p.locations.push(LocationRow {
                record,
                id,
                code,
                facility,
                specialty,
                address: address_text(&l["address"]),
                phone: phone_text(l),
                hours: hours_rows(l),
                ..LocationRow::default()
            });
        } else if let Some(code) = record_id(l, EAF_SYSTEM) {
            p.facilities.push(FacilityRow { record, id, code, address: address_text(&l["address"]), phone: phone_text(l), ..FacilityRow::default() });
        } else {
            p.others += 1;
        }
    }
    if p.facilities.len() > PLACES_MAX || p.locations.len() > PLACES_MAX {
        return Err(format!("Location search: more than {PLACES_MAX} facilities or departments"));
    }
    // A department whose facility the search did not return keeps none.
    for l in &mut p.locations {
        if !l.facility.is_empty() && !p.facilities.iter().any(|f| f.id == l.facility) {
            l.facility.clear();
        }
    }
    let lower = |s: &str| s.to_lowercase();
    p.facilities.sort_by_key(|f| lower(&f.record));
    let facility_name = |id: &str, fs: &[FacilityRow]| fs.iter().find(|f| f.id == id).map(|f| lower(&f.record)).unwrap_or_else(|| "\u{10ffff}".into());
    let fs = p.facilities.clone();
    p.locations.sort_by_key(|l| (facility_name(&l.facility, &fs), lower(&l.record)));
    p.specialties.sort_by_key(|s| lower(&s.spoken));
    Ok(p)
}

/// A participant's FHIR id, by the kind of its reference ("Location/"): the last part of the
/// reference, relative or absolute.
fn actor_id(a: &Value, kind: &str) -> Option<String> {
    a["participant"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["actor"]["reference"].as_str())
        .find(|r| r.starts_with(kind) || r.contains(&format!("/{kind}")))
        .and_then(|r| r.rsplit('/').next())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// What `$find` needs to know the patient (the probe, 2026-11-15): their `identifier`, `name`
/// and `birthDate`, as the search sent them. JSON, wrapped at once by the caller.
fn demographics(r: &Value) -> String {
    let mut d = json!({"resourceType": "Patient"});
    for field in ["identifier", "name", "birthDate"] {
        if !r[field].is_null() {
            d[field] = r[field].clone();
        }
    }
    d.to_string()
}

/// Epic's `Timing.repeat.dayOfWeek` code.
fn day_code(d: jiff::civil::Weekday) -> &'static str {
    use jiff::civil::Weekday::*;
    match d {
        Monday => "mon",
        Tuesday => "tue",
        Wednesday => "wed",
        Thursday => "thu",
        Friday => "fri",
        Saturday => "sat",
        Sunday => "sun",
    }
}

/// The visit's CSN: the `identifier` whose type or system names a CSN, else the first one.
fn csn(a: &Value) -> Option<String> {
    let ids = a["identifier"].as_array()?;
    let named = |i: &&Value| {
        let words = format!("{} {}", i["system"].as_str().unwrap_or(""), i["type"]["text"].as_str().unwrap_or(""));
        words.to_ascii_lowercase().contains("csn")
    };
    ids.iter().find(named).or(ids.first())?["value"].as_str().map(str::to_string)
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

/// How long ehr waits between sign-in tries while Epic's sandbox is down.
pub const SANDBOX_RETRY: std::time::Duration = std::time::Duration::from_secs(30);

/// Epic's sandbox as ehr serves it: signed in, or not yet. Epic's sandbox goes down at times
/// (2026-11-14: every FHIR request answered 503 "OltpServerUnavailable"). A sign-in that fails
/// at start is tried again in the background, so Epic comes back without a restart of ehr.
#[derive(Default)]
pub struct EpicSandbox {
    ehr: std::sync::RwLock<Option<Arc<EpicEhr>>>,
}

impl EpicSandbox {
    /// Already signed in (the tests, and a start where Epic answered).
    pub fn ready(ehr: Arc<EpicEhr>) -> Arc<Self> {
        Arc::new(Self { ehr: std::sync::RwLock::new(Some(ehr)) })
    }

    /// The signed-in sandbox, or None while Epic does not answer.
    pub fn get(&self) -> Option<Arc<EpicEhr>> {
        self.ehr.read().ok()?.clone()
    }

    /// Signs in now; on a failure, says so once on stderr and tries again every
    /// [`SANDBOX_RETRY`] in the background, saying when it is back. Errors name a status only.
    pub async fn start(client: std::path::PathBuf, key: std::path::PathBuf) -> Arc<Self> {
        let me = Arc::new(Self::default());
        match connect_sandbox(&client, &key).await {
            Ok(ehr) => me.set(ehr),
            Err(e) => {
                eprintln!(
                    "ehr: no Epic sandbox yet ({e}); Epic calls are refused, trying again every {} s",
                    SANDBOX_RETRY.as_secs()
                );
                let slot = Arc::clone(&me);
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(SANDBOX_RETRY).await;
                        if let Ok(ehr) = connect_sandbox(&client, &key).await {
                            slot.set(ehr);
                            eprintln!("ehr: Epic sandbox signed in; Epic calls work again");
                            return;
                        }
                    }
                });
            }
        }
        me
    }

    fn set(&self, ehr: EpicEhr) {
        if let Ok(mut slot) = self.ehr.write() {
            *slot = Some(Arc::new(ehr));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_parses() {
        let list = sandbox_list_file();
        assert!(!list.patients.is_empty());
    }

    /// The shapes the sandbox returned on 2026-11-15 (EPIC.md §2.2), with made-up values.
    #[test]
    fn an_import_splits_facilities_from_departments() {
        let eaf = "urn:oid:1.2.840.114350.1.13.0.1.7.2.696570";
        let dep = "urn:oid:1.2.840.114350.1.13.0.1.7.2.686980";
        let spec = "urn:oid:1.2.840.114350.1.13.0.1.7.10.686980.110";
        let specialty = |code: &str, words: &str| {
            json!([{"url": "http://open.epic.com/FHIR/StructureDefinition/extension/specialty",
                    "valueCodeableConcept": {"coding": [{"system": spec, "code": code, "display": words}]}}])
        };
        let found = vec![
            json!({"resourceType": "Location", "id": "f1", "name": "EHS Hospital",
                   "identifier": [{"system": eaf, "value": "101"}, {"system": "urn:oid:2.16.840.1.113883.4.4", "value": "x"}],
                   "address": {"line": ["1 Main St"], "city": "Verona", "state": "WI", "postalCode": "53593"},
                   "telecom": [{"system": "phone", "value": "608-555-0100"}]}),
            json!({"resourceType": "Location", "id": "d2", "name": "EMC Family Medicine",
                   "identifier": [{"system": dep, "value": "1003"}], "partOf": {"reference": "Location/f1"},
                   "extension": specialty("9", "Family Medicine"),
                   "hoursOfOperation": [{"daysOfWeek": ["mon", "tue"], "openingTime": "08:00:00", "closingTime": "17:00:00"}]}),
            json!({"resourceType": "Location", "id": "d1", "name": "EMC Cardiology",
                   "identifier": [{"system": dep, "value": "1002"}], "partOf": {"reference": "Location/f1"},
                   "extension": specialty("4", "Cardiology")}),
            json!({"resourceType": "Location", "id": "d3", "name": "Lab Annex",
                   "identifier": [{"system": dep, "value": "1004"}], "partOf": {"reference": "Location/f9"}}),
            json!({"resourceType": "Location", "id": "r1", "name": "Room 4"}),
        ];
        let p = places_from(&found).unwrap();
        assert_eq!(p.facilities.len(), 1);
        assert_eq!(p.facilities[0].code, "101");
        assert_eq!(p.facilities[0].address, "1 Main St, Verona, WI 53593");
        assert_eq!(p.facilities[0].phone, "608-555-0100");
        let names: Vec<&str> = p.locations.iter().map(|l| l.record.as_str()).collect();
        assert_eq!(names, ["EMC Cardiology", "EMC Family Medicine", "Lab Annex"], "by facility, then name; no facility last");
        assert_eq!(p.locations[1].facility, "f1");
        assert_eq!(p.locations[1].specialty, "9");
        assert_eq!(p.locations[1].hours.len(), 2);
        assert_eq!(p.locations[1].hours[0].opens, "08:00");
        assert!(p.locations[2].facility.is_empty(), "a facility the search did not return is cleared");
        assert_eq!(p.specialty_list, spec);
        let spoken: Vec<&str> = p.specialties.iter().map(|s| s.spoken.as_str()).collect();
        assert_eq!(spoken, ["cardiology", "family medicine"]);
        assert_eq!(p.others, 1);
        // What it builds fits the hospital's checks.
        for (key, value) in [("facilities", json!(p.facilities)), ("locations", json!(p.locations))] {
            crate::hospital::check_key(key, &value).unwrap();
        }
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
        let before = at("2026-11-01T00:00:00Z");
        let (_, v) = appointment(&a, &tz, before).unwrap();
        assert_eq!(v.starts_at, "2026-11-12T08:00");
        assert_eq!(v.minutes, 30);
        assert_eq!(v.clinician, "Physician Family Medicine, MD");
        assert_eq!(v.place.as_deref(), Some("EMC Family Medicine"));
        // The visit type from serviceType's coding; no reasonCode, so no reason; the
        // scheduler's comment is never read.
        assert_eq!(v.visit_type.as_deref(), Some("MyChart Video Visit"));
        assert_eq!(v.reason, None);
        assert_eq!(v.status, VisitStatus::Upcoming);
        assert_eq!(v.instructions, None);
        let cancelled = json!({"id": "a2", "status": "cancelled", "start": "2026-11-12T14:00:00Z", "minutesDuration": 30});
        assert_eq!(appointment(&cancelled, &tz, before).unwrap().1.status, VisitStatus::Cancelled);
        let unknown = json!({"id": "a3", "status": "entered-in-error", "start": "2026-11-12T14:00:00Z", "minutesDuration": 30});
        assert!(appointment(&unknown, &tz, before).is_none());
    }

    /// The shape of the sandbox's `$find` reply (the probe, 2026-11-15): absolute participant
    /// references with displays, the timezone extension, a `next` link.
    #[test]
    fn a_find_page_reads_each_proposed_visit_in_its_own_zone() {
        let bundle = json!({
            "resourceType": "Bundle", "total": 2,
            "link": [{"relation": "self", "url": "https://x/self"}, {"relation": "next", "url": "https://x/next"}],
            "entry": [
                {"resource": {"resourceType": "Appointment", "id": "p1", "status": "proposed",
                  "extension": [{"url": "http://hl7.org/fhir/StructureDefinition/timezone", "valueCode": "America/New_York"}],
                  "serviceType": [{"coding": [{"code": "1400000001", "display": "MYCHART VIDEO VISIT"}]}],
                  "start": "2026-11-16T22:00:00Z", "end": "2026-11-16T22:15:00Z", "minutesDuration": 15,
                  "participant": [
                    {"actor": {"reference": "https://fhir.example/api/FHIR/STU3/Patient/x", "display": "Lopez, Camila"}},
                    {"actor": {"reference": "https://fhir.example/api/FHIR/STU3/Practitioner/d", "display": "Physician Family Medicine, MD"}},
                    {"actor": {"reference": "https://fhir.example/api/FHIR/STU3/Location/l", "display": "EMC Family Medicine"}}
                  ]}},
                {"resource": {"resourceType": "Appointment", "id": "b1", "status": "booked", "start": "2026-11-16T22:00:00Z"}},
                {"resource": {"resourceType": "OperationOutcome"}}
            ]
        });
        // The Default hospital: Chicago, and "EMC Family Medicine" said as its spoken name.
        let page = page(&bundle, &Hospital::file());
        assert_eq!(page.next, ["https://x/next"]);
        assert_eq!(page.slots.len(), 1, "only proposed visits");
        let s = &page.slots[0];
        let is = |v: &Shielded<String>, want: &str| *v == Shielded::new(want.to_string());
        assert!(is(&s.starts_at, "2026-11-16T17:00"), "in the department's zone");
        assert!(is(&s.clinician, "Physician Family Medicine, MD"));
        assert!(is(&s.place, "the family medicine clinic"));
        // The type on the list in another case: its Online mark and its spoken name.
        assert!(s.about.online && s.minutes == 15);
        assert!(s.about.visit_type.as_ref().is_some_and(|t| is(t, "a video visit")));
        assert_eq!(s.id.0, "epic-proposed-p1@2026-11-16T22:00:00Z", "the id carries the start for a lost reply");
    }

    #[test]
    fn the_demographics_hold_only_what_find_needs() {
        let r = json!({"resourceType": "Patient", "id": "p", "identifier": [{"value": "1"}], "name": [{"family": "F"}],
                       "birthDate": "1987-09-12", "telecom": [{"value": "555"}], "address": [{"city": "C"}], "gender": "female"});
        let d: Value = serde_json::from_str(&demographics(&r)).unwrap();
        let mut keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["birthDate", "identifier", "name", "resourceType"]);
    }

    #[test]
    fn a_weekday_is_epic_s_day_code() {
        assert_eq!(day_code(jiff::civil::Weekday::Thursday), "thu");
    }

    #[test]
    fn an_appointment_gives_its_instructions_csn_and_past_status() {
        let tz = TimeZone::get("America/Chicago").unwrap();
        let a = json!({
            "resourceType": "Appointment", "id": "a1", "status": "fulfilled",
            "start": "2026-11-12T14:00:00Z", "minutesDuration": 30,
            "description": "Visit for routine care",
            "patientInstruction": "Arrive 15 minutes early.",
            "identifier": [{"system": "urn:oid:1.2.840.114350.1.13.0.1.7.3.698084.8", "value": "x1"},
                           {"type": {"text": "CSN"}, "value": "4000123"}],
        });
        let after = at("2026-11-13T00:00:00Z");
        let (_, v) = appointment(&a, &tz, after).unwrap();
        assert_eq!(v.status, VisitStatus::Seen);
        assert_eq!(v.visit_type.as_deref(), Some("Visit for routine care"), "description when no serviceType");
        assert_eq!(v.instructions.as_deref(), Some("Arrive 15 minutes early."));
        assert_eq!(v.csn.as_deref(), Some("4000123"));
        // Still booked after its time: no known outcome, so nothing to say.
        let stale = json!({"id": "a2", "status": "booked", "start": "2026-11-12T14:00:00Z", "minutesDuration": 30});
        assert!(appointment(&stale, &tz, after).is_none());
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
