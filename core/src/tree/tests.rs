//! The record view on a small record shaped like the Epic sandbox's (GLOBALS.md §2): a visit
//! the site did not send, blood pressure as components, a panel with a member that did not
//! arrive, an order, a medicine named by reference, a problem, an allergy.

use super::*;
use crate::store::RowId;
use serde_json::json;

#[derive(Default)]
struct Source {
    rows: Vec<(Table, ResourceRow)>,
    warnings: Vec<WarningRow>,
}

fn t(s: &str) -> Text {
    Shielded::new(s.to_owned())
}

fn row_id(n: usize) -> RowId {
    RowId::from_stored(format!("{n:032x}")).unwrap()
}

impl Source {
    fn add(&mut self, table: Table, raw: serde_json::Value) -> String {
        let id = row_id(self.rows.len() + 1);
        let key = id.as_str().to_owned();
        self.rows.push((
            table,
            ResourceRow {
                id,
                fhir_type: t(raw["resourceType"].as_str().unwrap()),
                fhir_id: t(raw["id"].as_str().unwrap()),
                kind: t("unused"),
                code_system: None,
                code: None,
                code_display: None,
                status: None,
                start: None,
                end: None,
                recorded: None,
                last_updated: None,
                canonical: None,
                version: None,
                raw: t(&raw.to_string()),
            },
        ));
        key
    }
}

impl TreeSource for Source {
    fn resources(&self, table: Table) -> Result<Vec<ResourceRow>, CoreError> {
        Ok(self
            .rows
            .iter()
            .filter(|(tb, _)| *tb == table)
            .map(|(_, r)| ResourceRow {
                id: RowId::from_stored(r.id.as_str().to_owned()).unwrap(),
                fhir_type: r.fhir_type.clone(),
                fhir_id: r.fhir_id.clone(),
                kind: r.kind.clone(),
                code_system: None,
                code: None,
                code_display: None,
                status: None,
                start: None,
                end: None,
                recorded: None,
                last_updated: None,
                canonical: None,
                version: None,
                raw: r.raw.clone(),
            })
            .collect())
    }
    fn type_counts(&self, table: Table) -> Result<BTreeMap<String, u64>, CoreError> {
        let mut n = BTreeMap::new();
        for (tb, r) in &self.rows {
            if *tb == table {
                *n.entry(r.fhir_type.reveal(&TreePass::for_tests()).clone()).or_insert(0) += 1;
            }
        }
        Ok(n)
    }
    fn patient(&self) -> Result<Option<PatientRow>, CoreError> {
        Ok(Some(PatientRow {
            fhir_id: t("p1"),
            name: Some(t(r#"[{"use":"official","given":["Sandbox"],"family":"Patient"}]"#)),
            birth_date: Some(t("1987-09-12")),
            sex: Some(t("female")),
        }))
    }
    fn site(&self) -> Result<Option<SiteRow>, CoreError> {
        Ok(Some(SiteRow { display_name: Some(t("Sandbox Health")), ..Default::default() }))
    }
    fn warnings(&self) -> Result<Vec<WarningRow>, CoreError> {
        Ok(self
            .warnings
            .iter()
            .map(|w| WarningRow {
                id: RowId::from_stored(w.id.as_str().to_owned()).unwrap(),
                fhir_type: w.fhir_type.clone(),
                severity: None,
                code: w.code.clone(),
                text: None,
                at: w.at.clone(),
            })
            .collect())
    }
}

fn camila_like() -> (Source, BTreeMap<&'static str, String>) {
    let mut s = Source::default();
    let mut rows = BTreeMap::new();
    let vitals = json!({"coding":[{"system":"http://terminology.hl7.org/CodeSystem/observation-category","code":"vital-signs"}]});
    let lab = json!({"coding":[{"system":"http://terminology.hl7.org/CodeSystem/observation-category","code":"laboratory"}]});
    rows.insert("visit", s.add(Table::Encounter, json!({"resourceType":"Encounter","id":"e1","status":"finished",
        "class":{"code":"AMB"},"period":{"start":"2024-03-05T09:00:00-05:00"},
        "diagnosis":[{"condition":{"reference":"Condition/c1"}}],
        "participant":[{"individual":{"reference":"Practitioner/dr1","display":"Dr. Sandbox"}}]})));
    // Three blood pressures on a visit the site did not send.
    for (i, day) in ["2024-01-02", "2024-02-02", "2024-03-02"].iter().enumerate() {
        s.add(Table::Result, json!({"resourceType":"Observation","id":format!("bp{i}"),"status":"final","category":[vitals],
            "code":{"coding":[{"system":"http://loinc.org","code":"85354-9"}],"text":"Blood pressure"},
            "effectiveDateTime":day,"encounter":{"reference":"Encounter/missing","display":"Inpatient stay"},
            "component":[
                {"code":{"coding":[{"system":"http://loinc.org","code":"8480-6"}],"text":"Systolic"},"valueQuantity":{"value":120,"unit":"mm[Hg]"}},
                {"code":{"coding":[{"system":"http://loinc.org","code":"8462-4"}],"text":"Diastolic"},"valueQuantity":{"value":80,"unit":"mm[Hg]"}}]}));
    }
    rows.insert("order", s.add(Table::Order, json!({"resourceType":"ServiceRequest","id":"sr1","status":"completed",
        "intent":"order","authoredOn":"2024-03-05","encounter":{"reference":"Encounter/e1"},
        "identifier":[{"type":{"coding":[{"code":"PLAC"}]},"value":"778899"}],
        "code":{"coding":[{"system":"http://loinc.org","code":"24323-8"}]}})));
    rows.insert("panel", s.add(Table::Result, json!({"resourceType":"DiagnosticReport","id":"dr1","status":"final",
        "code":{"coding":[{"system":"http://loinc.org","code":"24323-8"}],"text":"CMP"},"effectiveDateTime":"2024-03-05",
        "encounter":{"reference":"Encounter/e1"},"basedOn":[{"reference":"ServiceRequest/sr1"}],
        "result":[{"reference":"Observation/na"},{"reference":"Observation/k"},{"reference":"Observation/gone","display":"Chloride"}]})));
    for (id, code, v) in [("na", "2951-2", "140"), ("k", "2823-3", "4.10")] {
        s.add(Table::Result, serde_json::from_str(&format!(
            r#"{{"resourceType":"Observation","id":"{id}","status":"final","category":[{lab}],
            "code":{{"coding":[{{"system":"http://loinc.org","code":"{code}"}}]}},"effectiveDateTime":"2024-03-05",
            "encounter":{{"reference":"Encounter/e1"}},"valueQuantity":{{"value":{v},"unit":"mmol/L"}},
            "note":[{{"text":"first note"}},{{"text":"second note"}}]}}"#)).unwrap());
    }
    rows.insert("problem", s.add(Table::ChartItem, json!({"resourceType":"Condition","id":"c1",
        "category":[{"coding":[{"code":"problem-list-item"}]}],"clinicalStatus":{"coding":[{"code":"active"}]},
        "verificationStatus":{"coding":[{"code":"confirmed"}]},
        "code":{"coding":[{"system":"http://hl7.org/fhir/sid/icd-10-cm","code":"E11.9"},{"system":"http://snomed.info/sct","code":"44054006"}]},
        "onsetDateTime":"2019"})));
    s.add(Table::ChartItem, json!({"resourceType":"AllergyIntolerance","id":"a1","category":["medication"],
        "code":{"text":"Penicillin"},"reaction":[{"manifestation":[{"text":"Hives"}],"severity":"mild"}]}));
    rows.insert("drug", s.add(Table::Drug, json!({"resourceType":"Medication","id":"m1",
        "code":{"coding":[{"system":"http://www.nlm.nih.gov/research/umls/rxnorm","code":"860975"}],"text":"Metformin 500 mg"}})));
    for (id, day) in [("mr1", "2023-01-01"), ("mr2", "2024-03-05")] {
        s.add(Table::Medication, json!({"resourceType":"MedicationRequest","id":id,"status":"active","intent":"order",
            "authoredOn":day,"medicationReference":{"reference":"Medication/m1"},"reasonReference":[{"reference":"Condition/c1"}],
            "requester":{"reference":"Practitioner/dr1","display":"Dr. Sandbox"}}));
    }
    s.warnings.push(WarningRow {
        id: row_id(900),
        fhir_type: t("Observation"),
        severity: None,
        code: Some(t("suppressed")),
        text: None,
        at: t("2024-03-06T00:00:00Z"),
    });
    (s, rows)
}

fn build() -> (Tree, BTreeMap<&'static str, String>) {
    let (s, rows) = camila_like();
    (Tree::build(&[&s], || 0).unwrap(), rows)
}

fn show(v: Option<Text>) -> String {
    v.map(|v| v.reveal(&TreePass::for_tests()).clone()).unwrap_or_default()
}

fn shows(v: Vec<Text>) -> Vec<String> {
    v.into_iter().map(|v| v.reveal(&TreePass::for_tests()).clone()).collect()
}

fn val(s: &str) -> Key {
    Key::Value(t(s))
}

#[test]
fn every_row_becomes_a_record_and_missing_targets_become_stubs() {
    let (tree, _) = build();
    let n = |g: &str| tree.counts().iter().find(|c| c.global == g).cloned().unwrap();
    assert_eq!(n("ENC").records, 2, "the visit and the stub visit");
    assert_eq!(n("ENC").stubs, 1);
    // 3 blood pressures + 6 components + the report + 2 values + the member that did not arrive
    assert_eq!(n("RES").records, 13);
    assert_eq!(n("RES").components, 6);
    assert_eq!(n("RES").stubs, 1);
    assert_eq!(n("PARTY").stubs, 1, "one stub for the practitioner named three times");
    assert_eq!(n("MED").records, 3);
}

#[test]
fn a_stub_visit_holds_its_values_and_its_display() {
    let (tree, _) = build();
    let stub = tree.walk(Global::Enc, "by-kind", &[val("visit")], false).unwrap();
    let stub = stub.into_iter().find(|ien| show(tree.field(Global::Enc, *ien, "stub").unwrap()) == "1").unwrap();
    assert_eq!(show(tree.field(Global::Enc, stub, "display").unwrap()), "Inpatient stay");
    // Every blood pressure and every component points at it.
    assert_eq!(tree.walk(Global::Res, "by-enc", &[Key::Ien(stub)], false).unwrap().len(), 9);
}

#[test]
fn components_are_children_in_order_and_trend_by_their_own_code() {
    let (tree, _) = build();
    let systolic = tree.walk(Global::Res, "by-trend", &[val("vital-signs"), val("loinc:8480-6")], true).unwrap();
    assert_eq!(systolic.len(), 3, "one systolic per blood pressure");
    let newest = systolic[0];
    let parent = tree.pointer(Global::Res, newest, "parent").unwrap().unwrap();
    assert_eq!(show(tree.field(Global::Res, parent, "date").unwrap()), "2024-03-02T00:00:00Z", "newest first");
    let children = tree.walk(Global::Res, "by-parent", &[Key::Ien(parent)], false).unwrap();
    assert_eq!(children[0], newest, "systolic is first, as sent");
    assert_eq!(show(tree.field(Global::Res, children[1], "concept").unwrap()), "loinc:8462-4");
    assert_eq!(show(tree.field(Global::Res, newest, "row").unwrap()), "", "a component has no row of its own");
}

#[test]
fn a_panel_keeps_its_order_and_a_member_that_did_not_arrive() {
    let (tree, rows) = build();
    let panel = tree.by_row(Global::Res, 1, &rows["panel"]).unwrap().unwrap();
    assert_eq!(show(tree.field(Global::Res, panel, "kind").unwrap()), "panel");
    let members = tree.walk(Global::Res, "by-parent", &[Key::Ien(panel)], false).unwrap();
    assert_eq!(members.len(), 3);
    let concepts: Vec<String> = members.iter().map(|m| show(tree.field(Global::Res, *m, "concept").unwrap())).collect();
    assert_eq!(concepts[..2], ["loinc:2951-2", "loinc:2823-3"], "the lab's order, not by name");
    assert_eq!(show(tree.field(Global::Res, members[2], "stub").unwrap()), "1");
    assert_eq!(show(tree.field(Global::Res, members[2], "display").unwrap()), "Chloride");
    assert_eq!(show(tree.field(Global::Res, members[1], "value").unwrap()), "4.10", "a trailing zero is kept");
    for m in &members {
        assert_eq!(tree.pointer(Global::Res, *m, "root").unwrap(), Some(panel));
    }
    assert_eq!(tree.walk(Global::Res, "by-root", &[Key::Ien(panel)], false).unwrap().len(), 4);
    assert_eq!(shows(tree.notes(members[0]).unwrap()), ["first note", "second note"]);
    let order = tree.pointer(Global::Res, panel, "order").unwrap().unwrap();
    assert_eq!(show(tree.field(Global::Ord, order, "number").unwrap()), "778899");
}

#[test]
fn a_visit_reaches_what_happened_at_it() {
    let (tree, rows) = build();
    let visit = tree.by_row(Global::Enc, 1, &rows["visit"]).unwrap().unwrap();
    assert_eq!(show(tree.field(Global::Enc, visit, "date").unwrap()), "2024-03-05T14:00:00Z", "in UTC");
    assert_eq!(tree.walk(Global::Res, "by-enc", &[Key::Ien(visit)], false).unwrap().len(), 3);
    assert_eq!(tree.walk(Global::Ord, "by-enc", &[Key::Ien(visit)], false).unwrap().len(), 1);
    let problem = tree.by_row(Global::Chart, 1, &rows["problem"]).unwrap().unwrap();
    assert_eq!(tree.walk(Global::Enc, "by-diagnosis", &[Key::Ien(problem)], false).unwrap(), [visit]);
    let who = tree.list(Global::Enc, visit, "participants").unwrap();
    let dr: u64 = show(who.into_iter().next()).parse().unwrap();
    assert_eq!(show(tree.field(Global::Party, dr, "display").unwrap()), "Dr. Sandbox");
}

#[test]
fn medicines_group_by_their_drug() {
    let (tree, rows) = build();
    let drug = tree.by_row(Global::Med, 1, &rows["drug"]).unwrap().unwrap();
    let orders = tree.walk(Global::Med, "by-medicine", &[val("rxnorm:860975")], true).unwrap();
    assert_eq!(orders.len(), 3, "the drug record and both orders");
    assert!(orders.contains(&drug));
    let ordered = tree.walk(Global::Med, "by-kind", &[val("ordered")], true).unwrap();
    assert_eq!(ordered.len(), 2);
    assert_eq!(show(tree.field(Global::Med, ordered[0], "concept_text").unwrap()), "Metformin 500 mg");
    let problem = tree.by_row(Global::Chart, 1, &rows["problem"]).unwrap().unwrap();
    assert_eq!(tree.walk(Global::Med, "by-reason", &[Key::Ien(problem)], false).unwrap().len(), 2);
}

#[test]
fn chart_items_keep_their_status_and_their_standard_code() {
    let (tree, rows) = build();
    let problem = tree.by_row(Global::Chart, 1, &rows["problem"]).unwrap().unwrap();
    assert_eq!(show(tree.field(Global::Chart, problem, "concept").unwrap()), "snomed:44054006", "SNOMED before ICD-10");
    assert_eq!(show(tree.field(Global::Chart, problem, "verification").unwrap()), "confirmed");
    assert_eq!(show(tree.field(Global::Chart, problem, "precision").unwrap()), "year");
    let allergies = tree.walk(Global::Chart, "by-kind", &[val("allergy")], false).unwrap();
    assert_eq!(shows(tree.list(Global::Chart, allergies[0], "reactions.manifestation").unwrap()), ["Hives"]);
    assert_eq!(show(tree.field(Global::Chart, allergies[0], "mark").unwrap()), "none", "no date at all");
}

#[test]
fn the_patient_the_site_and_what_the_fetch_saw() {
    let (tree, _) = build();
    assert_eq!(show(tree.site_field(SiteNode::Patient, 1, "name").unwrap()), "Sandbox Patient");
    assert_eq!(show(tree.site_field(SiteNode::Site, 1, "name").unwrap()), "Sandbox Health");
    let (rows, warnings) = tree.fetch(1, "Observation").unwrap().unwrap();
    assert_eq!(rows, 5, "3 blood pressures and 2 lab values");
    assert_eq!(shows(warnings), ["suppressed"]);
}

#[test]
fn links_hold_the_roles_no_pointer_takes() {
    let (tree, _) = build();
    let n = tree.counts().iter().find(|c| c.global == "LINK").unwrap().records;
    // The two orders' requesters are pointers; nothing in this record is left over.
    assert_eq!(n, 0);
}

#[test]
fn two_sites_never_share_a_record_or_a_local_code() {
    let (a, _) = camila_like();
    let (b, _) = camila_like();
    let tree = Tree::build(&[&a, &b], || 0).unwrap();
    let res = tree.counts().iter().find(|c| c.global == "RES").unwrap();
    assert_eq!(res.records, 26, "the same FHIR ids at two sites are two records");
    let k = tree.walk(Global::Res, "by-trend", &[val("laboratory"), val("loinc:2823-3")], false).unwrap();
    assert_eq!(k.len(), 2, "a LOINC code is one trend across sites");
}
