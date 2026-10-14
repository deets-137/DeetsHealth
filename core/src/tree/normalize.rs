//! The normalizer for the record view (GLOBALS.md §5.2): from one raw FHIR resource, the
//! fields of its global. Pointers are not here; the load fills them from the references
//! (`load.rs`). Every value is shielded the moment it is read.
//!
//! It never guesses. A field the site did not send stays empty, and the card says so.

use crate::index::canonical_system;
use crate::store::Text;
use crate::Shielded;
use serde_json::Value;

pub(super) type Fields = Vec<(&'static str, Text)>;

pub(super) fn sh(s: impl Into<String>) -> Text {
    Shielded::new(s.into())
}

/// A string, or a number as the site wrote it (`5.10` stays `5.10`).
fn scalar(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn put(out: &mut Fields, name: &'static str, v: Option<String>) {
    if let Some(v) = v {
        out.push((name, sh(v)));
    }
}

/// A CodeableConcept's words: its `text`, else its first coding's `display`, else its first
/// coding's `code`. For a list of them, the first.
pub(super) fn words(cc: &Value) -> Option<String> {
    let cc = cc.as_array().and_then(|a| a.first()).unwrap_or(cc);
    scalar(cc.get("text"))
        .or_else(|| codings(cc).find_map(|c| scalar(c.get("display"))))
        .or_else(|| codings(cc).find_map(|c| scalar(c.get("code"))))
}

fn codings(cc: &Value) -> impl Iterator<Item = &Value> {
    cc.get("coding").and_then(Value::as_array).into_iter().flatten()
}

/// The standard code systems, by the short name a concept key uses.
const STANDARD: [(&str, &str); 5] = [
    ("http://loinc.org", "loinc"),
    ("http://snomed.info/sct", "snomed"),
    ("http://www.nlm.nih.gov/research/umls/rxnorm", "rxnorm"),
    ("http://hl7.org/fhir/sid/cvx", "cvx"),
    ("http://hl7.org/fhir/sid/icd-10-cm", "icd10cm"),
];

/// The concept of a record (GLOBALS.md §5.3).
pub(super) struct Concept {
    /// `loinc:2951-2` when a coding is in one of the global's standard systems (in the order
    /// given); else the first coding, prefixed by the site, `s1:<system>|<code>`.
    pub key: Option<String>,
    pub text: Option<String>,
    /// Every coding, `<system>|<code>`, in the canonical spelling of its system.
    pub codes: Vec<String>,
}

pub(super) fn concept(site: u32, standard: &[&str], cc: &Value) -> Concept {
    let cc = cc.as_array().and_then(|a| a.first()).unwrap_or(cc);
    let pairs: Vec<(String, String)> = codings(cc)
        .filter_map(|c| {
            let code = scalar(c.get("code"))?;
            let system = c.get("system").and_then(Value::as_str).map(canonical_system).unwrap_or("");
            Some((system.to_owned(), code))
        })
        .collect();
    let standard_key = standard.iter().find_map(|short| {
        let system = STANDARD.iter().find(|(_, s)| s == short)?.0;
        pairs.iter().find(|(sys, _)| sys == system).map(|(_, code)| format!("{short}:{code}"))
    });
    let key = standard_key.or_else(|| pairs.first().map(|(sys, code)| format!("s{site}:{sys}|{code}")));
    Concept { key, text: words(cc), codes: pairs.iter().map(|(s, c)| format!("{s}|{c}")).collect() }
}

/// The value of an `Observation` or of one of its components (GLOBALS.md §4.2, "Value kinds").
pub(super) fn value(v: &Value) -> Fields {
    let mut out = Fields::new();
    let quantity = |q: &Value, out: &mut Fields| {
        put(out, "value", scalar(q.get("value")));
        put(out, "comparator", scalar(q.get("comparator")));
        put(out, "unit", scalar(q.get("unit")).or_else(|| scalar(q.get("code"))));
        if q.get("system").and_then(Value::as_str) == Some("http://unitsofmeasure.org") {
            put(out, "ucum", scalar(q.get("code")));
        }
    };
    let both = |a: Option<String>, b: Option<String>, sep: &str| match (a, b) {
        (None, None) => None,
        (a, b) => Some(format!("{}{sep}{}", a.unwrap_or_default(), b.unwrap_or_default())),
    };
    let kind = if let Some(q) = v.get("valueQuantity") {
        quantity(q, &mut out);
        "quantity"
    } else if let Some(cc) = v.get("valueCodeableConcept") {
        put(&mut out, "value", words(cc));
        "coded"
    } else if let Some(s) = v.get("valueString") {
        put(&mut out, "value", scalar(Some(s)));
        "text"
    } else if let Some(n) = v.get("valueInteger") {
        put(&mut out, "value", scalar(Some(n)));
        "quantity"
    } else if let Some(b) = v.get("valueBoolean") {
        put(&mut out, "value", scalar(Some(b)));
        "boolean"
    } else if let Some(r) = v.get("valueRange") {
        let low = r.pointer("/low/value").and_then(|x| scalar(Some(x)));
        let high = r.pointer("/high/value").and_then(|x| scalar(Some(x)));
        put(&mut out, "value", both(low, high, " – "));
        put(&mut out, "unit", r.pointer("/low/unit").or_else(|| r.pointer("/high/unit")).and_then(|x| scalar(Some(x))));
        "range"
    } else if let Some(r) = v.get("valueRatio") {
        let n = r.pointer("/numerator/value").and_then(|x| scalar(Some(x)));
        let d = r.pointer("/denominator/value").and_then(|x| scalar(Some(x)));
        put(&mut out, "value", both(n, d, ":"));
        "ratio"
    } else if let Some(d) = v.get("valueDateTime").or_else(|| v.get("valueTime")) {
        put(&mut out, "value", scalar(Some(d)));
        "date"
    } else if let Some(p) = v.get("valuePeriod") {
        put(&mut out, "value", both(scalar(p.get("start")), scalar(p.get("end")), " – "));
        "date"
    } else if v.get("valueSampledData").is_some() || v.get("valueAttachment").is_some() {
        "other"
    } else if let Some(r) = v.get("dataAbsentReason") {
        put(&mut out, "value", words(r));
        "absent"
    } else {
        "none"
    };
    out.push(("value_kind", sh(kind)));
    if let Some(range) = v.get("referenceRange").and_then(Value::as_array).and_then(|a| a.first()) {
        put(&mut out, "low", range.pointer("/low/value").and_then(|x| scalar(Some(x))));
        put(&mut out, "high", range.pointer("/high/value").and_then(|x| scalar(Some(x))));
        put(&mut out, "range", scalar(range.get("text")));
    }
    if let Some(i) = v.get("interpretation") {
        let i = i.as_array().and_then(|a| a.first()).unwrap_or(i);
        put(&mut out, "flag", codings(i).find_map(|c| scalar(c.get("code"))).or_else(|| scalar(i.get("text"))));
    }
    out
}

/// The `text` of every `note`, in order. Free text: a subfile node each, never packed.
pub(super) fn notes(r: &Value) -> Vec<Text> {
    r.get("note").and_then(Value::as_array).into_iter().flatten().filter_map(|n| scalar(n.get("text"))).map(sh).collect()
}

/// A person's name for a card: the `official` (else `usual`, else first) `HumanName`, as its
/// `text`, else given names then family. Takes one name or a list.
pub(super) fn human_name(v: &Value) -> Option<String> {
    let names: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        one => vec![one],
    };
    let use_is = |n: &&Value, u: &str| n.get("use").and_then(Value::as_str) == Some(u);
    let n = names.iter().find(|n| use_is(n, "official")).or_else(|| names.iter().find(|n| use_is(n, "usual"))).or(names.first())?;
    scalar(n.get("text")).or_else(|| {
        let mut parts: Vec<String> = n.get("given").and_then(Value::as_array).into_iter().flatten().filter_map(|g| scalar(Some(g))).collect();
        parts.extend(scalar(n.get("family")));
        (!parts.is_empty()).then(|| parts.join(" "))
    })
}

/// `^ENC`'s own fields, pointers aside.
pub(super) fn encounter(r: &Value) -> Fields {
    let mut out = Fields::new();
    let class = r.get("class").map(|c| c.as_array().and_then(|a| a.first()).unwrap_or(c));
    put(&mut out, "class", class.and_then(|c| scalar(c.get("code")).or_else(|| scalar(c.get("display")))));
    out
}

/// `^ORD`'s own fields, pointers aside.
pub(super) fn order(r: &Value) -> Fields {
    let mut out = Fields::new();
    put(&mut out, "intent", scalar(r.get("intent")));
    put(&mut out, "priority", scalar(r.get("priority")));
    put(&mut out, "ordered", scalar(r.get("authoredOn")));
    put(&mut out, "occurrence", scalar(r.get("occurrenceDateTime")).or_else(|| r.pointer("/occurrencePeriod/start").and_then(|x| scalar(Some(x)))));
    // The placer order number: the identifier typed PLAC (HL7 v2-0203).
    let placer = r.get("identifier").and_then(Value::as_array).into_iter().flatten().find(|id| {
        id.get("type").is_some_and(|t| codings(t).any(|c| c.get("code").and_then(Value::as_str) == Some("PLAC")))
    });
    put(&mut out, "number", placer.and_then(|id| scalar(id.get("value"))));
    out
}

/// `^MED`'s own fields, pointers aside: the dose in its parts (HANDOFF.md › Decided 8.3a
/// builds the text on the card), the supply, and a vaccine's dose number and lot.
pub(super) fn medicine(r: &Value) -> Fields {
    let mut out = Fields::new();
    let first = |field: &str| r.get(field).map(|d| d.as_array().and_then(|a| a.first()).unwrap_or(d));
    let dosage = first("dosageInstruction").or_else(|| first("dosage"));
    if let Some(d) = dosage {
        put(&mut out, "sig", scalar(d.get("text")));
        let dose = d.pointer("/doseAndRate/0/doseQuantity").or_else(|| d.get("dose"));
        put(&mut out, "dose", dose.and_then(|q| scalar(q.get("value"))));
        put(&mut out, "dose_unit", dose.and_then(|q| scalar(q.get("unit")).or_else(|| scalar(q.get("code")))));
        put(&mut out, "route", d.get("route").and_then(words));
        let timing = d.get("timing");
        let repeat = timing.and_then(|t| t.get("repeat"));
        let frequency = timing.and_then(|t| t.get("code")).and_then(words).or_else(|| {
            let f = scalar(repeat?.get("frequency"))?;
            let p = scalar(repeat?.get("period"))?;
            let u = scalar(repeat?.get("periodUnit")).unwrap_or_default();
            Some(format!("{f}/{p}{u}"))
        });
        put(&mut out, "frequency", frequency);
        let as_needed = match d.get("asNeededBoolean") {
            Some(b) => scalar(Some(b)),
            None => d.get("asNeededCodeableConcept").map(|_| "true".to_owned()),
        };
        put(&mut out, "as_needed", as_needed);
    }
    let supply = r.get("dispenseRequest");
    let quantity = supply.and_then(|s| s.get("quantity")).or_else(|| r.get("quantity"));
    put(&mut out, "quantity", quantity.and_then(|q| {
        let v = scalar(q.get("value"))?;
        Some(match scalar(q.get("unit")) {
            Some(u) => format!("{v} {u}"),
            None => v,
        })
    }));
    put(&mut out, "refills", supply.and_then(|s| scalar(s.get("numberOfRepeatsAllowed"))));
    put(&mut out, "supply_days", supply.and_then(|s| s.pointer("/expectedSupplyDuration/value")).or_else(|| r.pointer("/daysSupply/value")).and_then(|x| scalar(Some(x))));
    let protocol = first("protocolApplied");
    put(&mut out, "dose_number", protocol.and_then(|p| scalar(p.get("doseNumberPositiveInt")).or_else(|| scalar(p.get("doseNumberString")))));
    put(&mut out, "lot", scalar(r.get("lotNumber")));
    out
}

/// `^CHART`'s own fields, pointers aside.
pub(super) fn chart(r: &Value) -> Fields {
    let mut out = Fields::new();
    let code = |field: &str| r.get(field).and_then(|cc| codings(cc).find_map(|c| scalar(c.get("code"))));
    put(&mut out, "clinical", code("clinicalStatus"));
    put(&mut out, "verification", code("verificationStatus"));
    put(&mut out, "category", r.get("category").and_then(|c| match c {
        Value::Array(a) => a.first().and_then(|x| scalar(Some(x)).or_else(|| words(x))),
        other => words(other),
    }));
    put(&mut out, "criticality", scalar(r.get("criticality")));
    for reaction in r.get("reaction").and_then(Value::as_array).into_iter().flatten() {
        let substance = reaction.get("substance").and_then(words).unwrap_or_default();
        let manifestation = reaction.get("manifestation").and_then(words).unwrap_or_default();
        let severity = scalar(reaction.get("severity")).unwrap_or_default();
        out.push(("reactions.substance", sh(substance)));
        out.push(("reactions.manifestation", sh(manifestation)));
        out.push(("reactions.severity", sh(severity)));
    }
    out
}

/// `^DOC`'s own fields, pointers aside. The note's text is not here: it is fetched when the
/// patient opens the note (DATA-MODEL.md §5).
pub(super) fn document(r: &Value) -> Fields {
    let mut out = Fields::new();
    put(&mut out, "description", scalar(r.get("description")).or_else(|| scalar(r.get("title"))));
    for c in r.get("content").and_then(Value::as_array).into_iter().flatten() {
        let a = c.get("attachment");
        out.push(("contents.type", sh(a.and_then(|a| scalar(a.get("contentType"))).unwrap_or_default())));
        out.push(("contents.binary", sh(a.and_then(|a| scalar(a.get("url"))).unwrap_or_default())));
    }
    out
}

/// `^PARTY`'s own fields: the name a card shows.
pub(super) fn party(t: &str, r: &Value) -> Fields {
    let mut out = Fields::new();
    let name = match t {
        "Practitioner" | "RelatedPerson" | "Person" => r.get("name").and_then(human_name),
        _ => scalar(r.get("name")),
    };
    put(&mut out, "name", name);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreePass;
    use serde_json::json;

    fn get(f: &Fields, name: &str) -> Option<String> {
        f.iter().find(|(n, _)| *n == name).map(|(_, v)| v.reveal(&TreePass::for_tests()).clone())
    }

    #[test]
    fn a_standard_code_is_the_key_wherever_it_sits() {
        let cc = json!({"coding":[{"system":"urn:oid:1.2.840.114350.1.13.0.1.7.5.737384.24","code":"1234"},
            {"system":"https://loinc.org","code":"2345-7","display":"Glucose"}],"text":"Glucose"});
        let c = concept(1, &["loinc"], &cc);
        assert_eq!(c.key.as_deref(), Some("loinc:2345-7"), "LOINC second still wins");
        assert_eq!(c.text.as_deref(), Some("Glucose"));
        assert_eq!(c.codes.len(), 2);
        let local = concept(2, &["loinc"], &json!({"coding":[{"system":"urn:x","code":"9"}]}));
        assert_eq!(local.key.as_deref(), Some("s2:urn:x|9"), "a local code is kept per site");
    }

    #[test]
    fn values_keep_what_the_site_wrote() {
        // Parsed from text, as a reply is: `json!` would make 5.10 a float first.
        let q = value(
            &serde_json::from_str(
                r#"{"valueQuantity":{"value":5.10,"comparator":"<","unit":"mg/dL",
                "system":"http://unitsofmeasure.org","code":"mg/dL"},
                "referenceRange":[{"low":{"value":4.0},"high":{"value":6},"text":"4-6"}],
                "interpretation":[{"coding":[{"code":"H"}]}]}"#,
            )
            .unwrap(),
        );
        assert_eq!(get(&q, "value").as_deref(), Some("5.10"), "a trailing zero is kept");
        assert_eq!(get(&q, "comparator").as_deref(), Some("<"));
        assert_eq!(get(&q, "ucum").as_deref(), Some("mg/dL"));
        assert_eq!(get(&q, "low").as_deref(), Some("4.0"));
        assert_eq!(get(&q, "range").as_deref(), Some("4-6"));
        assert_eq!(get(&q, "flag").as_deref(), Some("H"));
        let coded = value(&json!({"valueCodeableConcept":{"coding":[{"code":"S","display":"Susceptible"}]}}));
        assert_eq!(get(&coded, "value_kind").as_deref(), Some("coded"));
        assert_eq!(get(&coded, "value").as_deref(), Some("Susceptible"));
        let titer = value(&json!({"valueRatio":{"numerator":{"value":1},"denominator":{"value":80}}}));
        assert_eq!(get(&titer, "value").as_deref(), Some("1:80"));
        let absent = value(&json!({"dataAbsentReason":{"text":"Not performed"}}));
        assert_eq!(get(&absent, "value_kind").as_deref(), Some("absent"));
        assert_eq!(get(&value(&json!({})), "value_kind").as_deref(), Some("none"));
    }

    #[test]
    fn names_prefer_official_then_text() {
        let n = json!([{"use":"usual","given":["Cam"]},{"use":"official","given":["Ana","Maria"],"family":"Test"}]);
        assert_eq!(human_name(&n).as_deref(), Some("Ana Maria Test"));
        assert_eq!(human_name(&json!({"text":"Dr. Sandbox"})).as_deref(), Some("Dr. Sandbox"));
    }

    #[test]
    fn a_dose_in_its_parts() {
        let m = medicine(&json!({"dosageInstruction":[{"text":"1 tab daily","route":{"text":"Oral"},
            "timing":{"repeat":{"frequency":1,"period":1,"periodUnit":"d"}},
            "doseAndRate":[{"doseQuantity":{"value":1,"unit":"tablet"}}]}],
            "dispenseRequest":{"quantity":{"value":30,"unit":"tablet"},"numberOfRepeatsAllowed":2}}));
        assert_eq!(get(&m, "frequency").as_deref(), Some("1/1d"));
        assert_eq!(get(&m, "quantity").as_deref(), Some("30 tablet"));
        assert_eq!(get(&m, "refills").as_deref(), Some("2"));
        assert_eq!(get(&m, "route").as_deref(), Some("Oral"));
    }
}
