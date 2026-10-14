//! The index (CORE-CRATE.md §8, DATA-MODEL.md §3–§4): from one raw resource, the columns a
//! row is found and ordered by, and every reference it makes to another row.
//!
//! It runs on a parsed resource inside `net::parse_reply` (at the fetch, and again when the
//! cache is re-indexed from the raw resources), and wraps every value it keeps. It builds no
//! card model: those wait for each card's design (the owner, 2026-09-21).
//!
//! The words it keeps plain are FHIR's and ours, never the record's: a resource type, a
//! field path such as `Observation.basedOn`, and a link role such as `ordered`.

use crate::fhir;
use crate::net::Text;
use crate::store::Table;
use crate::Shielded;
use serde_json::Value;

/// The version of the rules in this file and in `fhir::table_for`. Raise it after any change
/// to them: a site file with an older number is re-indexed from its raw rows when it is next
/// opened, with no fetch (the owner, 2026-09-21; INDEX-AND-LINKS.md §10).
pub const INDEX_VERSION: i64 = 1;

/// The index columns of one resource. Every value is shielded.
#[derive(Default)]
pub struct Index {
    pub kind: Option<Text>,
    pub code_system: Option<Text>,
    pub code: Option<Text>,
    pub code_display: Option<Text>,
    pub status: Option<Text>,
    pub start: Option<Text>,
    pub end: Option<Text>,
    pub recorded: Option<Text>,
    pub last_updated: Option<Text>,
    pub canonical: Option<Text>,
    pub version: Option<Text>,
}

/// One reference from this resource to another resource.
pub struct Ref {
    /// The FHIR field it came from, e.g. `Observation.basedOn` (plain: FHIR's words).
    pub source_field: String,
    /// Why the rows are tied (plain: our words; DATA-MODEL.md §4).
    pub role: &'static str,
    /// The target's FHIR type (plain).
    pub target_type: String,
    pub target_table: Table,
    pub target_id: Text,
    /// The reference as the site sent it.
    pub target_ref: Text,
    /// The reference's `display`: all the record view knows of a target the site did not
    /// send (a stub, GLOBALS.md §5.5). Not stored in the link table.
    pub display: Option<Text>,
}

fn text(v: Option<&Value>) -> Option<Text> {
    v.and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| Shielded::new(s.to_owned()))
}

fn at<'a>(r: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(r, |v, k| v.get(k))
}

/// The first of `paths` that holds a value.
fn first(r: &Value, paths: &[&str]) -> Option<Text> {
    paths.iter().find_map(|p| text(at(r, p)))
}

/// When a resource happened, began or ended, and when it was written down. One list for all
/// types: each type has only some of these fields, and the first present one wins. Dates stay
/// as the site wrote them, so "2019-04" keeps its precision (CORE-CRATE.md §8).
const START: [&str; 22] = [
    "effectiveDateTime", "effectiveInstant", "effectivePeriod.start", "occurrenceDateTime", "occurrencePeriod.start",
    "performedDateTime", "performedPeriod.start", "onsetDateTime", "onsetPeriod.start", "period.start", "start",
    "started", "whenHandedOver", "whenPrepared", "collection.collectedDateTime", "collection.collectedPeriod.start",
    "context.period.start", "dispenseRequest.validityPeriod.start", "billablePeriod.start", "sent", "startDate",
    "createdDateTime",
];
const END: [&str; 11] = [
    "effectivePeriod.end", "occurrencePeriod.end", "performedPeriod.end", "abatementDateTime", "abatementPeriod.end",
    "period.end", "end", "collection.collectedPeriod.end", "context.period.end", "dispenseRequest.validityPeriod.end",
    "billablePeriod.end",
];
const RECORDED: [&str; 10] =
    ["issued", "recordedDate", "recorded", "authoredOn", "authored", "dateAsserted", "date", "created", "received", "dateWritten"];

/// The concept that names a resource, per type; `code` for every type not listed.
pub(crate) fn code_field(t: &str) -> &'static [&'static str] {
    match t {
        "MedicationRequest" | "MedicationDispense" | "MedicationAdministration" | "MedicationStatement" => {
            &["medicationCodeableConcept"]
        }
        "Immunization" | "ImmunizationEvaluation" => &["vaccineCode", "targetDisease"],
        "Encounter" | "EpisodeOfCare" | "Organization" | "Location" | "DocumentReference" | "Composition" | "Coverage"
        | "Claim" | "ExplanationOfBenefit" | "Account" | "Device" | "Consent" | "Flag" => &["type", "code"],
        "Appointment" => &["serviceType", "appointmentType"],
        "Goal" => &["description"],
        "CarePlan" | "CareTeam" | "Communication" => &["category"],
        "AdverseEvent" => &["event"],
        "RelatedPerson" | "PractitionerRole" => &["relationship", "code"],
        _ => &["code"],
    }
}

/// One code system, one spelling (FHIR-SHAPES.md §4, rule 2).
pub fn canonical_system(s: &str) -> &str {
    match s.trim_end_matches('/') {
        "https://loinc.org" | "http://loinc.org" | "urn:oid:2.16.840.1.113883.6.1" => "http://loinc.org",
        "https://snomed.info/sct" | "http://snomed.info/sct" | "urn:oid:2.16.840.1.113883.6.96" => "http://snomed.info/sct",
        "https://www.nlm.nih.gov/research/umls/rxnorm" | "urn:oid:2.16.840.1.113883.6.88" => {
            "http://www.nlm.nih.gov/research/umls/rxnorm"
        }
        "urn:oid:2.16.840.1.113883.12.292" => "http://hl7.org/fhir/sid/cvx",
        "http://hl7.org/fhir/observation-category" => "http://terminology.hl7.org/CodeSystem/observation-category",
        "urn:oid:2.16.840.1.113883.6.90" => "http://hl7.org/fhir/sid/icd-10-cm",
        other => other,
    }
}

/// A CodeableConcept (or the first of a list of them): its first coding, and its text.
fn concept(v: &Value) -> (Option<Text>, Option<Text>, Option<Text>) {
    let cc = v.as_array().and_then(|a| a.first()).unwrap_or(v);
    let coding = cc.get("coding").and_then(Value::as_array).and_then(|a| a.first());
    let system = coding
        .and_then(|c| c.get("system"))
        .and_then(Value::as_str)
        .map(|s| Shielded::new(canonical_system(s).to_owned()));
    let code = coding.and_then(|c| text(c.get("code")));
    let display = text(cc.get("text")).or_else(|| coding.and_then(|c| text(c.get("display"))));
    (system, code, display)
}

/// The first code of a CodeableConcept list field, e.g. `category[0].coding[0].code`.
fn first_code(r: &Value, field: &str) -> Option<String> {
    let cc = r.get(field)?;
    let cc = cc.as_array().and_then(|a| a.first()).unwrap_or(cc);
    cc.get("coding")?.as_array()?.first()?.get("code")?.as_str().map(str::to_owned)
}

/// What a row is, in our words, so a card can pick its rows (DATA-MODEL.md §5).
fn kind(t: &str, r: &Value) -> String {
    match t {
        "Observation" => first_code(r, "category").unwrap_or_else(|| "observation".into()),
        "DiagnosticReport" => if r.get("result").is_some() { "panel" } else { "report" }.into(),
        "Condition" => first_code(r, "category").unwrap_or_else(|| "condition".into()),
        "Immunization" => "vaccine".into(),
        "ImmunizationRecommendation" => "vaccine-due".into(),
        "MedicationRequest" => "ordered".into(),
        "MedicationDispense" => "filled".into(),
        "MedicationStatement" => "reported".into(),
        "MedicationAdministration" => "given".into(),
        "Encounter" => "visit".into(),
        "Appointment" => "planned".into(),
        "EpisodeOfCare" => "episode".into(),
        "DocumentReference" => "note".into(),
        "QuestionnaireResponse" => "form".into(),
        "Communication" => "message".into(),
        "AllergyIntolerance" => "allergy".into(),
        other => other.to_lowercase(),
    }
}

/// The index columns of one resource.
pub fn extract(t: &str, r: &Value) -> Index {
    let status = text(r.get("status")).or_else(|| {
        r.pointer("/clinicalStatus/coding/0/code").and_then(Value::as_str).map(|s| Shielded::new(s.to_owned()))
    });
    let (code_system, code, code_display) = code_field(t)
        .iter()
        .find_map(|f| r.get(*f).map(concept))
        .unwrap_or((None, None, None));
    Index {
        kind: Some(Shielded::new(kind(t, r))),
        code_system,
        code,
        code_display,
        status,
        start: first(r, &START),
        end: first(r, &END),
        recorded: first(r, &RECORDED),
        last_updated: text(r.pointer("/meta/lastUpdated")),
        canonical: if fhir::table_for(t) == Some(Table::Definition) { text(r.get("url")) } else { None },
        version: if fhir::table_for(t) == Some(Table::Definition) { text(r.get("version")) } else { None },
    }
}

/// The role of a link, from the source type and the field (DATA-MODEL.md §4).
fn role(t: &str, path: &str) -> &'static str {
    let last = path.rsplit('.').next().unwrap_or(path);
    match (t, path) {
        ("Encounter", "diagnosis.condition") => "diagnosed",
        ("Encounter", "partOf") => "parent",
        ("Encounter", "episodeOfCare") => "episode",
        ("Encounter", "appointment") => "booking",
        ("Encounter", "participant.individual") => "participant",
        ("Encounter", "location.location") => "location",
        ("Encounter", "serviceProvider") => "service-provider",
        ("DocumentReference", "context.encounter") | ("Composition", "encounter") => "documented",
        ("Claim", "item.encounter") | ("ExplanationOfBenefit", "item.encounter") | ("ChargeItem", "context") => "billed",
        ("MedicationDispense", "context") => "dispensed",
        ("MedicationAdministration", "context") => "given",
        (_, "encounter") | (_, "context") => match t {
            "Observation" | "DiagnosticReport" | "Specimen" => "collected",
            "Procedure" | "ImagingStudy" => "performed",
            "Immunization" => "given",
            "Condition" => "diagnosed",
            "MedicationRequest" | "ServiceRequest" | "DeviceRequest" | "NutritionOrder" | "VisionPrescription"
            | "CommunicationRequest" | "RequestGroup" | "Task" | "SupplyRequest" => "ordered",
            _ => "recorded",
        },
        (_, "basedOn") => "fulfils",
        (_, "result") | (_, "hasMember") => "contains",
        (_, "partOf") => "part-of",
        (_, "derivedFrom") => "derived-from",
        (_, "reasonReference") => "reason",
        (_, "medicationReference") => "drug",
        (_, "specimen") => "specimen",
        ("Provenance", "target") => "provenance-of",
        _ => match last {
            "requester" => "requester",
            "performer" | "actor" | "who" | "individual" | "member" => "performer",
            "recorder" | "asserter" | "author" | "enterer" => "author",
            "location" => "location",
            "organization" | "serviceProvider" | "custodian" | "managingOrganization" | "insurer" | "provider" => "organization",
            _ => "reference",
        },
    }
}

/// Split a FHIR reference into its type and id: `Type/id`, `Type/id/_history/n`, or an absolute
/// URL ending in `Type/id`. `None` for a contained (`#…`) or bundle (`urn:…`) reference.
pub fn split_reference(s: &str) -> Option<(&str, &str)> {
    if s.starts_with('#') || s.starts_with("urn:") {
        return None;
    }
    let s = s.split("/_history/").next()?;
    let mut parts = s.trim_end_matches('/').rsplit('/');
    let id = parts.next()?;
    let t = parts.next()?;
    (t.chars().next()?.is_ascii_uppercase() && !id.is_empty()).then_some((t, id))
}

/// Every reference in a resource to a row we keep, with its field and role. References to the
/// patient (always this one patient) and inside `contained`, `extension`, `meta` and `text`
/// are left out.
pub fn references(t: &str, r: &Value) -> Vec<Ref> {
    let mut out = Vec::new();
    walk(t, r, "", &mut out);
    out
}

fn walk(t: &str, v: &Value, path: &str, out: &mut Vec<Ref>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| walk(t, x, path, out)),
        Value::Object(o) => {
            if let Some(s) = o.get("reference").and_then(Value::as_str) {
                if let Some((target, id)) = split_reference(s) {
                    if let Some(table) = fhir::table_for(target) {
                        out.push(Ref {
                            source_field: format!("{t}.{path}"),
                            role: role(t, path),
                            target_type: target.to_owned(),
                            target_table: table,
                            target_id: Shielded::new(id.to_owned()),
                            target_ref: Shielded::new(s.to_owned()),
                            display: text(o.get("display")),
                        });
                    }
                }
                return;
            }
            for (k, x) in o {
                if matches!(k.as_str(), "contained" | "extension" | "modifierExtension" | "meta" | "text") {
                    continue;
                }
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                walk(t, x, &p, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::SitePass;
    use serde_json::json;

    fn show(v: &Option<Text>) -> Option<String> {
        v.as_ref().map(|t| t.reveal(&SitePass::for_tests()).clone())
    }

    #[test]
    fn a_lab_result() {
        let r = json!({"resourceType":"Observation","status":"final",
            "category":[{"coding":[{"system":"http://hl7.org/fhir/observation-category","code":"laboratory"}]}],
            "code":{"coding":[{"system":"https://loinc.org","code":"2345-7","display":"Glucose"}],"text":"Glucose SerPl-mCnc"},
            "effectiveDateTime":"2019-04","issued":"2019-04-02T10:00:00Z","meta":{"lastUpdated":"2020-01-01"}});
        let i = extract("Observation", &r);
        assert_eq!(show(&i.kind).as_deref(), Some("laboratory"));
        assert_eq!(show(&i.code_system).as_deref(), Some("http://loinc.org"), "one spelling of LOINC");
        assert_eq!(show(&i.code).as_deref(), Some("2345-7"));
        assert_eq!(show(&i.code_display).as_deref(), Some("Glucose SerPl-mCnc"), "text before display");
        assert_eq!(show(&i.start).as_deref(), Some("2019-04"), "a partial date keeps its precision");
        assert_eq!(show(&i.recorded).as_deref(), Some("2019-04-02T10:00:00Z"));
        assert!(i.end.is_none() && i.canonical.is_none());
    }

    #[test]
    fn per_type_fields() {
        let med = extract("MedicationRequest", &json!({"status":"active","authoredOn":"2020",
            "medicationCodeableConcept":{"coding":[{"system":"http://www.nlm.nih.gov/research/umls/rxnorm","code":"123"}]}}));
        assert_eq!(show(&med.code).as_deref(), Some("123"));
        assert_eq!(show(&med.kind).as_deref(), Some("ordered"));
        assert_eq!(show(&med.recorded).as_deref(), Some("2020"));
        let cond = extract("Condition", &json!({"clinicalStatus":{"coding":[{"code":"active"}]},
            "category":[{"coding":[{"code":"encounter-diagnosis"}]}],"onsetDateTime":"2001","abatementDateTime":"2002"}));
        assert_eq!(show(&cond.status).as_deref(), Some("active"));
        assert_eq!(show(&cond.kind).as_deref(), Some("encounter-diagnosis"));
        assert_eq!((show(&cond.start).as_deref(), show(&cond.end).as_deref()), (Some("2001"), Some("2002")));
        let enc = extract("Encounter", &json!({"type":[{"text":"Office visit"}],"period":{"start":"a","end":"b"}}));
        assert_eq!(show(&enc.code_display).as_deref(), Some("Office visit"));
        assert_eq!(show(&enc.kind).as_deref(), Some("visit"));
        assert_eq!(show(&extract("DiagnosticReport", &json!({"result":[]})).kind).as_deref(), Some("panel"));
        let q = extract("Questionnaire", &json!({"url":"http://q","version":"2"}));
        assert_eq!((show(&q.canonical).as_deref(), show(&q.version).as_deref()), (Some("http://q"), Some("2")));
    }

    #[test]
    fn references_and_roles() {
        let r = json!({"resourceType":"Observation",
            "subject":{"reference":"Patient/p1"},
            "encounter":{"reference":"Encounter/e1"},
            "basedOn":[{"reference":"ServiceRequest/s1"}],
            "performer":[{"reference":"https://site.example/fhir/Practitioner/d1/_history/3"},{"display":"no reference"}],
            "hasMember":[{"reference":"#contained1"}],
            "contained":[{"resourceType":"Practitioner","id":"contained1","organization":{"reference":"Organization/o9"}}],
            "extension":[{"valueReference":{"reference":"Location/x"}}]});
        let refs = references("Observation", &r);
        let got: Vec<(String, &str, String)> = refs.iter().map(|x| (x.source_field.clone(), x.role, x.target_type.clone())).collect();
        assert_eq!(
            got,
            [
                ("Observation.encounter".into(), "collected", "Encounter".into()),
                ("Observation.basedOn".into(), "fulfils", "ServiceRequest".into()),
                ("Observation.performer".into(), "performer", "Practitioner".into()),
            ]
        );
        assert_eq!(refs[2].target_id.reveal(&SitePass::for_tests()), "d1");
    }

    #[test]
    fn encounter_roles_follow_the_data_model() {
        let e = |t: &str, r: Value| references(t, &r).into_iter().map(|x| x.role).collect::<Vec<_>>();
        assert_eq!(e("MedicationRequest", json!({"encounter":{"reference":"Encounter/1"}})), ["ordered"]);
        assert_eq!(e("MedicationDispense", json!({"context":{"reference":"Encounter/1"}})), ["dispensed"]);
        assert_eq!(e("DocumentReference", json!({"context":{"encounter":[{"reference":"Encounter/1"}]}})), ["documented"]);
        assert_eq!(e("Encounter", json!({"diagnosis":[{"condition":{"reference":"Condition/1"}}],"partOf":{"reference":"Encounter/0"}})), ["diagnosed", "parent"]);
        assert_eq!(e("DiagnosticReport", json!({"result":[{"reference":"Observation/1"}]})), ["contains"]);
        assert_eq!(e("ExplanationOfBenefit", json!({"item":[{"encounter":[{"reference":"Encounter/1"}]}]})), ["billed"]);
        assert_eq!(e("AllergyIntolerance", json!({"encounter":{"reference":"Encounter/1"}})), ["recorded"]);
    }

    #[test]
    fn reference_forms() {
        assert_eq!(split_reference("Encounter/e1"), Some(("Encounter", "e1")));
        assert_eq!(split_reference("https://x/fhir/Encounter/e1/_history/2"), Some(("Encounter", "e1")));
        assert_eq!(split_reference("#c1"), None);
        assert_eq!(split_reference("urn:uuid:1"), None);
        assert_eq!(split_reference("e1"), None);
    }
}
