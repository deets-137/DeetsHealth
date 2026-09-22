//! Reading FHIR: which table each resource type lives in (DATA-MODEL.md §5, §7), what a site
//! says it serves (its `CapabilityStatement`), what the granted scopes allow, and one page of
//! a search. Every reply is read through `net::parse_reply`; every value kept is wrapped.
//!
//! Resource type names ("Observation") and search parameter names are FHIR's own words, not
//! record data, so they stay plain; logs may name them (SECURITY.md §4.5).

use crate::net::{self, Text};
use crate::store::Table;
use crate::{CoreError, Shielded};
use serde_json::Value;

/// The table for each FHIR type (DATA-MODEL.md §5 and §7.1). `None`: not stored, or not a
/// type a patient search returns. `Patient` has its own trimmed row (FHIR-SHAPES.md §5).
pub fn table_for(fhir_type: &str) -> Option<Table> {
    use Table::*;
    Some(match fhir_type {
        "Encounter" | "Appointment" | "EpisodeOfCare" => Encounter,
        "Observation" | "DiagnosticReport" | "Specimen" | "ImagingStudy" | "Media" | "MolecularSequence"
        | "RiskAssessment" | "ClinicalImpression" | "DetectedIssue" => Result,
        "MedicationRequest" | "MedicationDispense" | "Immunization" | "MedicationStatement"
        | "MedicationAdministration" | "ImmunizationRecommendation" | "ImmunizationEvaluation" => Medication,
        "DocumentReference" | "QuestionnaireResponse" | "Composition" | "DocumentManifest" | "Communication" => Document,
        "ServiceRequest" | "DeviceRequest" | "NutritionOrder" | "VisionPrescription" | "CommunicationRequest"
        | "RequestGroup" | "Task" | "SupplyRequest" => Order,
        "Condition" | "AllergyIntolerance" | "Procedure" | "Goal" | "CarePlan" | "Device" | "FamilyMemberHistory"
        | "AdverseEvent" | "Flag" | "DeviceUseStatement" | "BodyStructure" | "ResearchSubject" => ChartItem,
        "Practitioner" | "PractitionerRole" | "RelatedPerson" | "Organization" | "Location" | "CareTeam"
        | "HealthcareService" | "OrganizationAffiliation" | "Endpoint" | "Person" => Party,
        "Coverage" | "ExplanationOfBenefit" | "Claim" | "ClaimResponse" | "Account" | "ChargeItem" | "Invoice"
        | "Contract" | "InsurancePlan" | "CoverageEligibilityRequest" | "CoverageEligibilityResponse"
        | "EnrollmentRequest" | "EnrollmentResponse" | "PaymentNotice" | "PaymentReconciliation" => Billing,
        "Medication" | "Substance" | "MedicationKnowledge" => Drug,
        // Provenance is link data (DATA-MODEL.md §5). Its raw resource waits here until the
        // index turns it into links (stage 6).
        "MeasureReport" | "Consent" | "List" | "Group" | "AuditEvent" | "Linkage" | "Basic" | "GuidanceResponse"
        | "BiologicallyDerivedProduct" | "DeviceMetric" | "SupplyDelivery" | "AppointmentResponse" | "Provenance" => Other,
        "Questionnaire" | "Measure" | "ResearchStudy" | "ValueSet" | "ConceptMap" | "PlanDefinition"
        | "ActivityDefinition" | "ObservationDefinition" => Definition,
        _ => return None,
    })
}

/// The search parameters that tie a resource to its patient, in the order we prefer them.
const PATIENT_PARAMS: [&str; 4] = ["patient", "subject", "beneficiary", "member"];

/// FHIR's own Observation categories. A site that needs `category` (Epic does) is searched
/// once per category; an Observation in none of them is missed there (FHIR-SHAPES.md).
pub const OBSERVATION_CATEGORIES: [&str; 9] =
    ["laboratory", "vital-signs", "social-history", "survey", "exam", "imaging", "procedure", "therapy", "activity"];

/// One type a site lets us search by patient.
#[derive(Clone)]
pub struct Searchable {
    pub fhir_type: String,
    pub table: Table,
    /// `patient`, `subject`, … whichever the site lists first in [`PATIENT_PARAMS`] order.
    pub patient_param: &'static str,
    pub has_category: bool,
}

/// What a site's `CapabilityStatement` says it serves.
pub struct Capability {
    pub searchable: Vec<Searchable>,
    pub software_version: Option<Text>,
}

/// Read a `/metadata` reply.
pub fn read_capability(body: &Text) -> Result<Capability, CoreError> {
    net::parse_reply(body, |v| {
        if v.get("resourceType").and_then(Value::as_str) != Some("CapabilityStatement") {
            return Err(CoreError::Parse);
        }
        let mut searchable = Vec::new();
        let resources = v.pointer("/rest/0/resource").and_then(Value::as_array).ok_or(CoreError::Parse)?;
        for r in resources {
            let Some(t) = r.get("type").and_then(Value::as_str) else { continue };
            let Some(table) = table_for(t) else { continue };
            let can_search = r
                .get("interaction")
                .and_then(Value::as_array)
                .is_none_or(|i| i.iter().any(|x| x.get("code").and_then(Value::as_str) == Some("search-type")));
            let params: Vec<&str> = r
                .get("searchParam")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|p| p.get("name").and_then(Value::as_str)).collect())
                .unwrap_or_default();
            let Some(patient_param) = PATIENT_PARAMS.into_iter().find(|p| params.contains(p)) else { continue };
            if can_search {
                searchable.push(Searchable {
                    fhir_type: t.to_owned(),
                    table,
                    patient_param,
                    has_category: params.contains(&"category"),
                });
            }
        }
        let software_version = v.pointer("/software/version").and_then(Value::as_str).map(|s| Shielded::new(s.to_owned()));
        Ok(Capability { searchable, software_version })
    })
}

/// Whether the granted scopes let us read a type. SMART v1 (`patient/Observation.read`,
/// `patient/*.read`, `.*`) and v2 (`patient/Observation.rs`). No scope list: allowed.
pub fn scope_allows(scopes: Option<&str>, fhir_type: &str) -> bool {
    let Some(scopes) = scopes else { return true };
    scopes.split_whitespace().any(|s| {
        let Some(rest) = s.strip_prefix("patient/") else { return false };
        let Some((t, access)) = rest.split_once('.') else { return false };
        (t == "*" || t == fhir_type) && (access == "read" || access == "*" || (access.contains('r') && access.contains('s')))
    })
}

/// One resource from a search page.
pub struct Found {
    pub fhir_type: String,
    pub fhir_id: Text,
    /// The resource as JSON text.
    pub raw: Text,
    /// Came in through `_include`, not as a match.
    pub included: bool,
}

/// One issue from an `OperationOutcome` (in a search page, or as an error reply).
pub struct Issue {
    pub severity: Option<Text>,
    pub code: Option<Text>,
    pub text: Option<Text>,
}

/// One page of a search.
pub struct Page {
    pub found: Vec<Found>,
    pub issues: Vec<Issue>,
    /// The site's `next` link, when there are more pages.
    pub next: Option<Text>,
    /// `Bundle.total`, when the site sends it.
    pub total: Option<u64>,
}

fn issues_of(oo: &Value) -> Vec<Issue> {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).map(|s| Shielded::new(s.to_owned()));
    oo.get("issue")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|i| Issue {
                    severity: s(i.get("severity")),
                    code: s(i.get("code")),
                    text: s(i.get("diagnostics")).or_else(|| s(i.pointer("/details/text"))),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Read one search page (a `searchset` Bundle).
pub fn read_page(body: &Text) -> Result<Page, CoreError> {
    net::parse_reply(body, |v| {
        if v.get("resourceType").and_then(Value::as_str) != Some("Bundle") {
            return Err(CoreError::Parse);
        }
        let mut found = Vec::new();
        let mut issues = Vec::new();
        for e in v.get("entry").and_then(Value::as_array).into_iter().flatten() {
            let Some(r) = e.get("resource") else { continue };
            let t = r.get("resourceType").and_then(Value::as_str).unwrap_or_default();
            let mode = e.pointer("/search/mode").and_then(Value::as_str);
            if t == "OperationOutcome" || mode == Some("outcome") {
                issues.extend(issues_of(r));
                continue;
            }
            let Some(id) = r.get("id").and_then(Value::as_str) else { continue };
            found.push(Found {
                fhir_type: t.to_owned(),
                fhir_id: Shielded::new(id.to_owned()),
                raw: Shielded::new(serde_json::to_string(r).map_err(|_| CoreError::Parse)?),
                included: mode == Some("include"),
            });
        }
        let next = v
            .get("link")
            .and_then(Value::as_array)
            .and_then(|l| l.iter().find(|x| x.get("relation").and_then(Value::as_str) == Some("next")))
            .and_then(|x| x.get("url").and_then(Value::as_str))
            .map(|u| Shielded::new(u.to_owned()));
        let total = v.get("total").and_then(Value::as_u64);
        Ok(Page { found, issues, next, total })
    })
}

/// The issues in an error reply, when it is an `OperationOutcome`.
pub fn read_error(body: &Text) -> Vec<Issue> {
    net::parse_reply(body, |v| Ok(if v.get("resourceType").and_then(Value::as_str) == Some("OperationOutcome") { issues_of(v) } else { Vec::new() }))
        .unwrap_or_default()
}

/// The patient: name, birth date and sex only; the rest is dropped here, at the parse
/// (FHIR-SHAPES.md §5, fork 1).
pub fn read_patient(body: &Text) -> Result<crate::store::PatientRow, CoreError> {
    net::parse_reply(body, |v| {
        if v.get("resourceType").and_then(Value::as_str) != Some("Patient") {
            return Err(CoreError::Parse);
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(|s| Shielded::new(s.to_owned()));
        Ok(crate::store::PatientRow {
            fhir_id: s("id").ok_or(CoreError::Parse)?,
            name: v.get("name").map(|n| Shielded::new(n.to_string())),
            birth_date: s("birthDate"),
            sex: s("gender"),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::SitePass;

    fn t(s: &str) -> Text {
        Shielded::new(s.to_owned())
    }
    fn show(v: &Text) -> String {
        v.reveal(&SitePass::for_tests()).clone()
    }

    #[test]
    fn every_type_in_the_model_has_a_table() {
        assert!(table_for("Observation") == Some(Table::Result));
        assert!(table_for("Immunization") == Some(Table::Medication));
        assert!(table_for("Provenance") == Some(Table::Other));
        assert!(table_for("Patient").is_none());
        assert!(table_for("MedicinalProductIndication").is_none());
    }

    #[test]
    fn capability_picks_the_patient_param() {
        let cap = read_capability(&t(r#"{"resourceType":"CapabilityStatement","software":{"version":"9"},"rest":[{"resource":[
            {"type":"Observation","interaction":[{"code":"search-type"}],"searchParam":[{"name":"category"},{"name":"patient"},{"name":"subject"}]},
            {"type":"AdverseEvent","searchParam":[{"name":"subject"}]},
            {"type":"Group","searchParam":[{"name":"member"}]},
            {"type":"Practitioner","searchParam":[{"name":"name"}]},
            {"type":"Condition","interaction":[{"code":"read"}],"searchParam":[{"name":"patient"}]},
            {"type":"MedicinalProductIndication","searchParam":[{"name":"subject"}]}]}]}"#))
        .unwrap();
        let got: Vec<(&str, &str, bool)> =
            cap.searchable.iter().map(|s| (s.fhir_type.as_str(), s.patient_param, s.has_category)).collect();
        assert_eq!(got, [("Observation", "patient", true), ("AdverseEvent", "subject", false), ("Group", "member", false)]);
        assert_eq!(show(cap.software_version.as_ref().unwrap()), "9");
        assert!(read_capability(&t(r#"{"resourceType":"Bundle"}"#)).is_err());
    }

    #[test]
    fn scopes() {
        assert!(scope_allows(None, "Observation"));
        assert!(scope_allows(Some("openid patient/*.read"), "Observation"));
        assert!(scope_allows(Some("patient/Observation.rs"), "Observation"));
        assert!(scope_allows(Some("patient/*.*"), "Condition"));
        assert!(!scope_allows(Some("patient/Condition.read"), "Observation"));
        assert!(!scope_allows(Some("user/*.read launch/patient"), "Observation"));
        assert!(!scope_allows(Some("patient/Observation.c"), "Observation"));
    }

    #[test]
    fn a_page_with_matches_includes_and_an_outcome() {
        let p = read_page(&t(r#"{"resourceType":"Bundle","total":2,"link":[{"relation":"self","url":"https://s/a"},{"relation":"next","url":"https://s/b"}],"entry":[
            {"resource":{"resourceType":"MedicationRequest","id":"m1"},"search":{"mode":"match"}},
            {"resource":{"resourceType":"Practitioner","id":"p1"},"search":{"mode":"include"}},
            {"resource":{"resourceType":"OperationOutcome","issue":[{"severity":"warning","code":"suppressed","diagnostics":"some withheld"}]},"search":{"mode":"outcome"}},
            {"resource":{"resourceType":"Observation"}}]}"#))
        .unwrap();
        assert_eq!(p.found.len(), 2, "a resource with no id is skipped");
        assert!(!p.found[0].included && p.found[1].included);
        assert_eq!(show(&p.found[0].raw), r#"{"resourceType":"MedicationRequest","id":"m1"}"#, "the site's key order");
        assert_eq!(show(p.issues[0].code.as_ref().unwrap()), "suppressed");
        assert_eq!(show(p.next.as_ref().unwrap()), "https://s/b");
        assert_eq!(p.total, Some(2));
    }

    #[test]
    fn the_patient_keeps_three_fields() {
        let p = read_patient(&t(r#"{"resourceType":"Patient","id":"p","name":[{"family":"F"}],"birthDate":"1970","gender":"female",
            "address":[{"city":"C"}],"telecom":[{"value":"555"}],"identifier":[{"value":"123"}]}"#))
        .unwrap();
        assert_eq!(show(&p.fhir_id), "p");
        assert_eq!(show(p.name.as_ref().unwrap()), r#"[{"family":"F"}]"#);
        assert_eq!(show(p.birth_date.as_ref().unwrap()), "1970");
    }
}
