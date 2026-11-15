//! The EHR ledger (docs/ops/DEBUGGING.md §3; the owner, 2026-11-15): one entry for each request
//! ehr sends to an EHR while it serves one gRPC call: what it was, its HTTP status, how long the
//! EHR took, how many results came back, and the reply's size. Names and counts only: the
//! request is named by its resource, its operation and its parameter names, never a value, an
//! id or a patient.
//!
//! The entries of one gRPC call are gathered in a task-local list ([`kept`]); the service adds
//! them to the call's session, and CloseCall hands the whole ledger to the agent for the dev
//! call record. A request made outside a gathered call (the rail's list, a probe) is not kept.

use std::{cell::RefCell, future::Future, time::Instant};

use reqwest::Url;

/// One request to the EHR.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// "GET Appointment?date&patient&service-category&status", "POST STU3 Appointment/$find".
    pub what: String,
    /// The HTTP status; 0 when no answer came.
    pub status: u16,
    pub ms: u32,
    /// The resources in a Bundle, else 1 for a resource, 0 for none.
    pub results: u32,
    pub bytes: u32,
    /// When it started.
    pub at: Instant,
}

tokio::task_local! {
    static LEDGER: RefCell<Vec<Entry>>;
}

/// Runs `f` with its own ledger, and gives back what it sent.
pub async fn kept<F: Future>(f: F) -> (F::Output, Vec<Entry>) {
    LEDGER
        .scope(RefCell::new(Vec::new()), async {
            let out = f.await;
            let entries = LEDGER.with(|l| l.take());
            (out, entries)
        })
        .await
}

/// Notes one request, if a gathered call is running.
pub fn note(method: &str, url: &Url, status: u16, started: Instant, body: Option<&str>) {
    let (results, bytes) = match body {
        Some(b) => (results_in(b), u32::try_from(b.len()).unwrap_or(u32::MAX)),
        None => (0, 0),
    };
    let entry = Entry {
        what: what(method, url),
        status,
        ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
        results,
        bytes,
        at: started,
    };
    let _ = LEDGER.try_with(|l| l.borrow_mut().push(entry));
}

/// The request's name: the method, the FHIR version when it is not R4, the resource, `{id}` for
/// an id, an `$operation` by name, and the query's parameter names, sorted, without values.
pub fn what(method: &str, url: &Url) -> String {
    let segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
    let fhir = segments.iter().position(|s| *s == "FHIR");
    let mut name = vec![method.to_string()];
    let rest: &[&str] = match fhir {
        Some(i) if i + 1 < segments.len() => {
            if segments[i + 1] != "R4" {
                name.push(segments[i + 1].to_string());
            }
            &segments[i + 2..]
        }
        _ => &segments[segments.len().saturating_sub(1)..],
    };
    let path: Vec<String> = rest
        .iter()
        .enumerate()
        .map(|(i, s)| if i == 0 || s.starts_with('$') { (*s).to_string() } else { "{id}".to_string() })
        .collect();
    let mut keys: Vec<String> = url.query_pairs().map(|(k, _)| k.into_owned()).collect();
    keys.sort();
    keys.dedup();
    let query = if keys.is_empty() { String::new() } else { format!("?{}", keys.join("&")) };
    format!("{} {}{query}", name.join(" "), path.join("/"))
}

fn results_in(body: &str) -> u32 {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return 0 };
    if v["resourceType"] == "Bundle" {
        let n = v["entry"]
            .as_array()
            .map_or(0, |e| e.iter().filter(|x| x["resource"]["resourceType"] != "OperationOutcome").count());
        return u32::try_from(n).unwrap_or(u32::MAX);
    }
    u32::from(v["resourceType"].is_string() && v["resourceType"] != "OperationOutcome")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_named_without_any_value() {
        let u = |s: &str| Url::parse(s).unwrap();
        let search = u("https://x/api/FHIR/R4/Appointment?patient=eABC&status=booked&date=ge2026&date=lt2027");
        assert_eq!(what("GET", &search), "GET Appointment?date&patient&status");
        assert_eq!(what("GET", &u("https://x/api/FHIR/R4/Patient/eABC123")), "GET Patient/{id}");
        assert_eq!(what("POST", &u("https://x/api/FHIR/STU3/Appointment/$find")), "POST STU3 Appointment/$find");
        assert_eq!(what("POST", &u("https://x/api/FHIR/STU3/Appointment/$find?sessionID=s1")), "POST STU3 Appointment/$find?sessionID");
        assert!(!what("GET", &search).contains("eABC"));
    }

    #[test]
    fn results_count_a_bundles_resources_but_not_its_outcomes() {
        let b = r#"{"resourceType":"Bundle","entry":[{"resource":{"resourceType":"Appointment"}},{"resource":{"resourceType":"OperationOutcome"}}]}"#;
        assert_eq!(results_in(b), 1);
        assert_eq!(results_in(r#"{"resourceType":"Patient"}"#), 1);
        assert_eq!(results_in(r#"{"resourceType":"OperationOutcome"}"#), 0);
    }

    #[tokio::test]
    async fn a_gathered_call_keeps_its_own_entries_only() {
        let url = Url::parse("https://x/api/FHIR/R4/Patient").unwrap();
        note("GET", &url, 200, Instant::now(), None);
        let ((), entries) = kept(async { note("GET", &url, 200, Instant::now(), Some("{}")) }).await;
        assert_eq!(entries.len(), 1, "the note outside a gathered call is dropped");
        assert_eq!(entries[0].what, "GET Patient");
    }
}
