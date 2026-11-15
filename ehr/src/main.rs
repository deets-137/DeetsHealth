//! The `ehr` service (EHR-SERVICE.md).
//!
//!     ehr [--bind ADDR] [--port N] [--certs DIR]
//!
//! - `--bind` defaults to 127.0.0.1. `--port` defaults to 0: the OS picks a free port, and the
//!   first line on stdout names it (`ehr listening on 127.0.0.1:PORT`). Never a fixed port.
//! - `--certs` defaults to `dev-certs` (made by `cargo run -p deetshealth-ehr --example
//!   dev_certs`).
//! - The code by text goes through Twilio's Messages API when `DEETS_TWILIO_ACCOUNT_SID` and
//!   its partners are set (texts.rs; in dev, the fake Twilio). Otherwise the fake code, 246810.
//! - Phase 1 runs the fake EHR only. A deploy (`DEETS_REQUIRE_FIPS=1`) refuses to start until
//!   the real adapters exist, and refuses TLS that is not in FIPS mode.
//!
//!     ehr ingest [--calls DIR]
//!
//! - Files the dev call records and annotations in DIR (default `agent/calls`) into the call
//!   store (store.rs; DATABASE.md): `DEETS_DATABASE_URL` and the dev sealing key file
//!   `DEETS_SEAL_KEY_FILE`, both set by scripts/postgres.mjs. Idempotent.
//!
//!     ehr epic-check
//!
//! - Step E-2 against Epic's sandbox (epic.rs; EPIC.md §5): signs in as our backend app with
//!   `epic-client.json` and `dev-certs/epic-nonprod-es384.key`, reads `metadata`, searches for
//!   one public test patient. Prints status codes and counts.
//! - With `DEETS_EPIC_SANDBOX=1`, `ehr` also serves Epic's sandbox: a call that opens with
//!   `EHR_SOURCE_EPIC_SANDBOX` finds the caller and their visits there (epic_ehr.rs).
//!
//!     ehr epic-find
//!
//! - Step E-4's probe of Epic's sandbox `Appointment/$find` (STU3; EPIC.md §5.2): learns a real
//!   visit type and department from a public test patient's visit, then sends `$find` a few
//!   ways. Prints statuses, Epic's own error words and the replies' shape. Books nothing.
//!
//!     ehr epic-locations
//!
//! - Probes the sandbox's Location APIs for Config › Hospital › Places (EPIC.md §2.2): the
//!   departments on a public test patient's visits by `Location.Read`, then `Location.Search`
//!   tries. Prints statuses, counts, systems and the hospital's department and specialty names.
//!
//!     ehr epic-sandbox
//!
//! - Prints the Chat page's Epic list as JSON (epic_ehr.rs `sandbox_list`): Epic's public test
//!   patients and their visits ahead. Dev only; the dev server runs it.
//!
//! Output names ports, paths and fixed error strings. Never a request or a value.

use std::{net::IpAddr, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use deetshealth_ehr::{
    epic::{Epic, EpicConfig},
    epic_ehr::{self, EpicSandbox},
    proto::v1::ehr_server::EhrServer,
    scheduler::FakeScheduler,
    seal_key::SealKey,
    service::EhrService,
    store::Store,
    texts::{TwilioCodes, TwilioConfig},
    transport,
    verify::{Codes, FakeCodes, FakePatients},
};
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tonic::transport::Server;

/// The Epic sandbox's dev files (EPIC.md §2.3), relative to the repo root.
const EPIC_CLIENT: &str = "epic-client.json";
const EPIC_KEY: &str = "dev-certs/epic-nonprod-es384.key";

/// The longest a request may run when the agent sends no deadline of its own.
const REQUEST_LIMIT: Duration = Duration::from_secs(10);
/// The largest message, either way. Every message in the contract is small.
const MESSAGE_LIMIT: usize = 64 * 1024;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ehr: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn ingest(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut calls = PathBuf::from("agent/calls");
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--calls" => calls = value.into(),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let url = std::env::var("DEETS_DATABASE_URL").map_err(|_| "DEETS_DATABASE_URL is not set (npm run db:ingest sets it)")?;
    let key_file = std::env::var("DEETS_SEAL_KEY_FILE").map_err(|_| "DEETS_SEAL_KEY_FILE is not set (npm run db:ingest sets it)")?;
    let key = SealKey::from_dev_file(&PathBuf::from(key_file))?;
    let store = Store::connect(&url, key).await?;
    let filed = store.file_dir(&calls).await?;
    println!(
        "ehr ingest: {} calls filed, {} unchanged, {} new annotation lines",
        filed.calls, filed.unchanged, filed.annotations
    );
    Ok(())
}

/// `ehr epic-check`: step E-2 against Epic's sandbox (EPIC.md §5). About five requests: the
/// key set, `smart-configuration`, the token, `metadata`, one `Patient` search for a public
/// test patient. Prints status codes and counts only, never a value.
async fn epic_check() -> Result<(), String> {
    const JWKS_URL: &str = "https://health-keys.deets.solutions/nonprod/jwks.json";
    let config = EpicConfig::sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY))?;
    let epic = Epic::connect(config).await.map_err(|e| e.to_string())?;
    println!("smart-configuration: ok");
    let published = epic.key_is_published(JWKS_URL).await.map_err(|e| e.to_string())?;
    println!("key set: {}", if published { "the key on disk is published" } else { "the key on disk is NOT in the set" });
    let token = epic.token().await.map_err(|e| e.to_string())?;
    println!("token: ok, lives {} s, {} scopes", token.expires_in, token.scope_count);
    let (status, body) = epic.get(&token, "metadata").await.map_err(|e| e.to_string())?;
    let version = body.as_ref().and_then(|b| b["fhirVersion"].as_str()).unwrap_or("-");
    println!("metadata: HTTP {status}, FHIR {version}");
    // Camila Lopez: a public test patient on fhir.epic.com › Test Data (EPIC.md §3).
    let (status, body) = epic
        .get(&token, "Patient?family=Lopez&given=Camila&birthdate=1987-09-12")
        .await
        .map_err(|e| e.to_string())?;
    let matches = body
        .as_ref()
        .and_then(|b| b["entry"].as_array())
        .map(|e| e.iter().filter(|x| x["resource"]["resourceType"] == "Patient").count());
    match matches {
        Some(n) => println!("Patient search: HTTP {status}, {n} match(es)"),
        None => println!("Patient search: HTTP {status}"),
    }
    Ok(())
}

/// `ehr epic-find`: probes Epic's sandbox `Appointment/$find` (STU3) for step E-4 (EPIC.md §5.2,
/// §7.10). The shape that works (2026-11-15): the patient's demographics, a start and an end,
/// `service-type` (with a dash) and a `location-reference`. Each try below changes one input.
/// Prints, per try: the HTTP status, each OperationOutcome issue (Epic's own words), and for a
/// Bundle the count and shape of what came back (field names, link relations, extension URLs,
/// the form of participant references). Also the visit types on the test patient's visits:
/// codes and words are the hospital's setup, not a patient's. Never an id, a token or a
/// patient's value. About ten requests; `$find` books nothing.
/// `ehr epic-book`: ONE real booking in Epic's shared sandbox, for a public test patient, by
/// the code a call uses (`$find`, then `$book`, then the read after; EPIC.md §7.10). A write:
/// run it only with the owner's go-ahead (given 2026-11-15 for one booking). Prints shapes and
/// outcomes, never a value.
async fn epic_book() -> Result<(), String> {
    let ehr = epic_ehr::connect_sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY)).await?;
    for line in ehr.probe_book("erXuFYUfucBZaryVksYEcMg3").await? {
        println!("{line}");
    }
    Ok(())
}

/// `ehr epic-locations`: probes Epic's sandbox Location APIs for Config › Hospital › Places
/// (EPIC.md §2.2; HOSPITAL-CONFIG.md §2.7, §2.8). `Location.Read` of the departments on a public
/// test patient's visits, then a few `Location.Search` tries, each changing one input. Prints
/// HTTP statuses, Epic's own issue words, counts, the fields present, the identifier and
/// specialty systems, and department and specialty names: the hospital's setup, never a
/// patient's value. About ten requests; reads only.
async fn epic_locations() -> Result<(), String> {
    use serde_json::Value;
    let config = EpicConfig::sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY))?;
    let epic = Epic::connect(config).await.map_err(|e| e.to_string())?;
    let token = epic.token().await.map_err(|e| e.to_string())?;

    // What one Location holds: its fields, identifier systems, specialty, parent.
    fn describe(l: &Value) -> String {
        let fields: Vec<&str> = l.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
        let systems: Vec<&str> =
            l["identifier"].as_array().map(|a| a.iter().filter_map(|i| i["system"].as_str()).collect()).unwrap_or_default();
        let specialty: Vec<String> = l["extension"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|e| e["url"].as_str().is_some_and(|u| u.ends_with("/specialty")))
                    .flat_map(|e| e["valueCodeableConcept"]["coding"].as_array().cloned().unwrap_or_default())
                    .map(|c| format!("{} {} \"{}\"", c["system"].as_str().unwrap_or("-"), c["code"].as_str().unwrap_or("-"), c["display"].as_str().unwrap_or("-")))
                    .collect()
            })
            .unwrap_or_default();
        format!(
            "\"{}\" | fields {:?} | identifier systems {:?} | specialty {:?} | partOf {}",
            l["name"].as_str().unwrap_or("-"),
            fields,
            systems,
            specialty,
            l["partOf"]["display"].as_str().map(|d| format!("\"{d}\"")).unwrap_or_else(|| "none".into()),
        )
    }
    fn issues(b: &Value) -> Vec<String> {
        let outcomes = std::iter::once(b).chain(b["entry"].as_array().into_iter().flatten().map(|e| &e["resource"]));
        outcomes
            .filter(|r| r["resourceType"] == "OperationOutcome")
            .flat_map(|r| r["issue"].as_array().cloned().unwrap_or_default())
            .map(|i| format!("{}: {}", i["severity"].as_str().unwrap_or("-"), i["details"]["text"].as_str().or(i["diagnostics"].as_str()).unwrap_or("-")))
            .collect()
    }
    let locations = |b: &Value| -> Vec<Value> {
        b["entry"].as_array().map(|e| e.iter().map(|x| x["resource"].clone()).filter(|r| r["resourceType"] == "Location").collect()).unwrap_or_default()
    };

    // 1. The departments on Camila Lopez's visits (a public test patient, EPIC.md §5.2).
    let (status, bundle) = epic
        .get(&token, "Appointment?patient=erXuFYUfucBZaryVksYEcMg3&service-category=appointment")
        .await
        .map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = Vec::new();
    for a in bundle.as_ref().and_then(|b| b["entry"].as_array()).into_iter().flatten() {
        for p in a["resource"]["participant"].as_array().into_iter().flatten() {
            if let Some(r) = p["actor"]["reference"].as_str().filter(|r| r.contains("Location/")) {
                let id = r.rsplit('/').next().unwrap_or("").to_string();
                if !id.is_empty() && !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    println!("Appointment.Search: HTTP {status}, {} distinct Location references", ids.len());
    for id in &ids {
        let (status, body) = epic.get(&token, &format!("Location/{id}")).await.map_err(|e| e.to_string())?;
        match body.as_ref().filter(|b| b["resourceType"] == "Location") {
            Some(l) => println!("Location.Read: HTTP {status} | {}", describe(l)),
            None => println!("Location.Read: HTTP {status} | {:?}", body.as_ref().map(issues).unwrap_or_default()),
        }
    }

    // 2. Location.Search: each try changes one input.
    for query in ["status=active", "address=WI", "address=Verona", "address=WI&_count=100", "name=Family", "address-state=WI"] {
        let (status, body) = epic.get(&token, &format!("Location?{query}")).await.map_err(|e| e.to_string())?;
        let Some(b) = body else {
            println!("Location?{query}: HTTP {status}, no body");
            continue;
        };
        let found = locations(&b);
        let links: Vec<&str> = b["link"].as_array().map(|l| l.iter().filter_map(|x| x["relation"].as_str()).collect()).unwrap_or_default();
        println!(
            "Location?{query}: HTTP {status}, {} Locations, total {}, links {:?}, issues {:?}",
            found.len(),
            b["total"].as_u64().map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
            links,
            issues(&b)
        );
        let with_specialty = found.iter().filter(|l| l["extension"].as_array().is_some_and(|a| a.iter().any(|e| e["url"].as_str().is_some_and(|u| u.ends_with("/specialty"))))).count();
        let with_parent = found.iter().filter(|l| !l["partOf"].is_null()).count();
        if !found.is_empty() {
            println!("  {with_specialty} with a specialty, {with_parent} with a parent location");
            for l in found.iter().take(8) {
                println!("  {}", describe(l));
            }
        }
    }
    Ok(())
}

async fn epic_find() -> Result<(), String> {
    use serde_json::{json, Value};
    let config = EpicConfig::sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY))?;
    let epic = Epic::connect(config).await.map_err(|e| e.to_string())?;
    // Camila Lopez (EPIC.md §5.2): a rolling set of booked visits gives real visit types and a
    // department. A public test patient.
    let patient = "erXuFYUfucBZaryVksYEcMg3";
    let mut url = epic.base().join("Appointment").map_err(|_| "bad URL")?;
    url.query_pairs_mut().append_pair("patient", patient).append_pair("service-category", "appointment");
    let bundle = epic.read(url).await.map_err(|e| e.to_string())?;
    let visits: Vec<&Value> = bundle["entry"]
        .as_array()
        .map(|e| e.iter().map(|x| &x["resource"]).filter(|r| r["resourceType"] == "Appointment").collect())
        .unwrap_or_default();
    let mut types: Vec<(String, String, String)> = Vec::new();
    for v in &visits {
        let c = &v["serviceType"][0]["coding"][0];
        let t = (
            c["system"].as_str().unwrap_or("-").to_string(),
            c["code"].as_str().unwrap_or("-").to_string(),
            c["display"].as_str().unwrap_or("-").to_string(),
        );
        if !types.contains(&t) {
            types.push(t);
        }
    }
    println!("visit types on the test patient's {} visits:", visits.len());
    for (system, code, display) in &types {
        println!("  {system} | {code} | {display}");
    }
    let visit = visits.first().ok_or("no Appointment to learn a visit type from")?;
    let location = visit["participant"]
        .as_array()
        .and_then(|p| p.iter().find_map(|x| x["actor"]["reference"].as_str().filter(|r| r.contains("Location/"))))
        .map(str::to_string)
        .ok_or("no Location participant")?;
    let record = epic.read(epic.base().join(&format!("Patient/{patient}")).map_err(|_| "bad URL")?).await.map_err(|e| e.to_string())?;
    let demographics = |fields: &[&str]| {
        let mut d = json!({"resourceType": "Patient"});
        for f in fields {
            if !record[*f].is_null() {
                d[*f] = record[*f].clone();
            }
        }
        d
    };
    let stu3 = reqwest::Url::parse(&epic.base().as_str().replace("/R4/", "/STU3/")).map_err(|_| "bad STU3 URL")?;
    let find = stu3.join("Appointment/$find").map_err(|_| "bad $find URL")?;
    let start = jiff::Zoned::now().date().checked_add(jiff::Span::new().days(1)).map_err(|_| "date")?;
    let end = start.checked_add(jiff::Span::new().days(14)).map_err(|_| "date")?;
    let p = |name: &str, v: Value| {
        let mut o = v;
        o["name"] = json!(name);
        o
    };
    let all = ["identifier", "name", "birthDate", "gender", "telecom", "address"];
    let few = ["identifier", "name", "birthDate"];
    let times = vec![
        p("startTime", json!({"valueDateTime": format!("{start}T08:00:00-06:00")})),
        p("endTime", json!({"valueDateTime": format!("{end}T17:00:00-06:00")})),
    ];
    let only_start = vec![p("startTime", json!({"valueDateTime": format!("{start}T08:00:00-06:00")}))];
    let service = |c: &Value| p("service-type", json!({"valueCodeableConcept": {"coding": [c.clone()]}}));
    let video = visit["serviceType"][0]["coding"][0].clone();
    let other = types
        .iter()
        .find(|t| !t.2.to_lowercase().contains("video"))
        .map(|t| json!({"system": t.0, "code": t.1, "display": t.2}));
    let place = p("location-reference", json!({"valueReference": {"reference": location}}));
    let mornings = p(
        "time-of-day",
        json!({"valueTiming": {"repeat": {"timeOfDay": ["08:00:00"], "duration": 4, "durationUnit": "h"}}}),
    );
    let tuesdays = p("time-of-day", json!({"valueTiming": {"repeat": {"dayOfWeek": ["tue", "thu"]}}}));
    let mut tries: Vec<(String, Vec<Value>)> = vec![
        ("all demographics, times, video type, location".into(), [vec![p("patient", json!({"resource": demographics(&all)}))], times.clone(), vec![service(&video), place.clone()]].concat()),
        ("identifier, name, birthDate only".into(), [vec![p("patient", json!({"resource": demographics(&few)}))], times.clone(), vec![service(&video), place.clone()]].concat()),
        ("no location".into(), [vec![p("patient", json!({"resource": demographics(&all)}))], times.clone(), vec![service(&video)]].concat()),
        ("no endTime".into(), [vec![p("patient", json!({"resource": demographics(&all)}))], only_start, vec![service(&video), place.clone()]].concat()),
        ("time-of-day: mornings".into(), [vec![p("patient", json!({"resource": demographics(&all)}))], times.clone(), vec![service(&video), place.clone(), mornings]].concat()),
        ("time-of-day: Tuesdays and Thursdays".into(), [vec![p("patient", json!({"resource": demographics(&all)}))], times.clone(), vec![service(&video), place.clone(), tuesdays]].concat()),
    ];
    if let Some(o) = &other {
        tries.push((format!("an in-person type ({})", o["display"].as_str().unwrap_or("-")), [vec![p("patient", json!({"resource": demographics(&all)}))], times, vec![service(o), place]].concat()));
    }
    for (label, parameter) in tries {
        let body = json!({"resourceType": "Parameters", "parameter": parameter});
        let (status, reply) = epic.post(find.clone(), &body).await.map_err(|e| e.to_string())?;
        println!("\n$find, {label}: HTTP {status}");
        let Some(reply) = reply else { continue };
        let outcomes: Vec<&Value> = match reply["resourceType"].as_str() {
            Some("OperationOutcome") => vec![&reply],
            _ => reply["entry"].as_array().map(|e| e.iter().map(|x| &x["resource"]).filter(|r| r["resourceType"] == "OperationOutcome").collect()).unwrap_or_default(),
        };
        let mut seen: Vec<String> = Vec::new();
        for o in outcomes {
            for issue in o["issue"].as_array().into_iter().flatten() {
                let words = issue["details"]["text"].as_str().or(issue["diagnostics"].as_str()).unwrap_or("-");
                let code = issue["details"]["coding"][0]["code"].as_str().unwrap_or("-");
                let line = format!("{} {} [{}] {}", issue["severity"].as_str().unwrap_or("-"), issue["code"].as_str().unwrap_or("-"), code, words.chars().take(200).collect::<String>());
                let at: Vec<&str> = issue["expression"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                let line = if at.is_empty() { line } else { format!("{line} at {at:?}") };
                if !seen.contains(&line) {
                    println!("  issue: {line}");
                    seen.push(line);
                }
            }
        }
        let found: Vec<&Value> = reply["entry"].as_array().map(|e| e.iter().map(|x| &x["resource"]).filter(|r| r["resourceType"] == "Appointment").collect()).unwrap_or_default();
        if reply["resourceType"] == "Bundle" {
            let links: Vec<&str> = reply["link"].as_array().into_iter().flatten().filter_map(|l| l["relation"].as_str()).collect();
            println!("  Bundle: {} proposed appointment(s); total {}; links {links:?}", found.len(), reply["total"]);
        }
        if let Some(a) = found.first() {
            let keys: Vec<&String> = a.as_object().map(|o| o.keys().collect()).unwrap_or_default();
            println!("  first: status {}, fields {keys:?}", a["status"].as_str().unwrap_or("-"));
            let ext: Vec<&str> = a["extension"].as_array().into_iter().flatten().filter_map(|x| x["url"].as_str()).collect();
            println!("  extensions: {ext:?}");
            let starts: Vec<String> = found.iter().filter_map(|x| x["start"].as_str()).map(|s| s.chars().skip(11).take(5).collect()).collect();
            println!("  start times (UTC, hh:mm): {starts:?}");
            let forms: Vec<String> = a["participant"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x["actor"]["reference"].as_str())
                .map(|r| {
                    let kind = ["Practitioner/", "Location/", "Patient/", "Device/", "HealthcareService/"].iter().find(|k| r.contains(*k)).copied().unwrap_or("other");
                    format!("{}{kind}", if r.starts_with("http") { "absolute " } else { "relative " })
                })
                .collect();
            println!("  participants: {forms:?}; each has a display: {}", a["participant"].as_array().into_iter().flatten().all(|x| x["actor"]["display"].is_string()));
        }
    }

    // The real path (EPIC.md §7.10, build 2): `EpicScheduler::find` and `find_more` with the
    // as a call would run them. Read only: nothing is booked.
    let ehr = epic_ehr::connect_sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY)).await?;
    for line in ehr.probe_find(patient, &record).await {
        println!("{line}");
    }
    Ok(())
}

async fn run() -> Result<(), String> {
    let mut first = std::env::args().skip(1).peekable();
    if first.peek().map(String::as_str) == Some("epic-find") {
        return epic_find().await;
    }
    if first.peek().map(String::as_str) == Some("epic-book") {
        return epic_book().await;
    }
    if first.peek().map(String::as_str) == Some("epic-locations") {
        return epic_locations().await;
    }
    if first.peek().map(String::as_str) == Some("ingest") {
        first.next();
        return ingest(first).await;
    }
    if first.peek().map(String::as_str) == Some("epic-check") {
        return epic_check().await;
    }
    if first.peek().map(String::as_str) == Some("epic-sandbox") {
        let epic = epic_ehr::connect_sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY)).await?;
        println!("{}", epic.sandbox_list().await?);
        return Ok(());
    }
    let mut bind: IpAddr = [127, 0, 0, 1].into();
    let mut port: u16 = 0;
    let mut certs = PathBuf::from("dev-certs");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--bind" => bind = value.parse().map_err(|_| "--bind: not an IP address")?,
            "--port" => port = value.parse().map_err(|_| "--port: not a port")?,
            "--certs" => certs = value.into(),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }

    let config = transport::server_config(&certs)?;
    if transport::fips_required() {
        if !config.fips() {
            return Err("DEETS_REQUIRE_FIPS=1, but TLS is not in FIPS mode (build with --features fips)".into());
        }
        return Err("DEETS_REQUIRE_FIPS=1, but this build has the fake adapters only".into());
    }

    // The code by text: Twilio's Messages API when its settings are set (the fake Twilio in
    // dev), else the fake code. Read before the port is bound, so a bad setting stops ehr
    // before its first line.
    let texts = TwilioConfig::from_env()?.map(TwilioCodes::new).transpose()?;

    let listener = TcpListener::bind((bind, port)).await.map_err(|_| "cannot bind the port")?;
    let addr = listener.local_addr().map_err(|_| "cannot read the bound port")?;
    println!("ehr listening on {addr}");

    let config = Arc::new(config);
    let store = open_store().await;
    let epic = open_epic().await;
    match texts {
        Some(codes) => serve(listener, config, codes, store, epic).await,
        None => serve(listener, config, FakeCodes, store, epic).await,
    }
}

/// Epic's sandbox as a second source, when `DEETS_EPIC_SANDBOX=1` (the dev server sets it,
/// with `DEETS_EPIC_CLIENT_FILE` and `DEETS_EPIC_KEY_FILE`).
/// When the sign-in fails (Epic's sandbox goes down at times), ehr says so on stderr, refuses
/// Epic calls as UNAVAILABLE, and signs in again in the background (`EpicSandbox::start`).
async fn open_epic() -> Option<Arc<EpicSandbox>> {
    if std::env::var("DEETS_EPIC_SANDBOX").as_deref() != Ok("1") {
        return None;
    }
    // The dev server passes both paths, absolute: ehr runs from agent/.
    let path = |var: &str, default: &str| PathBuf::from(std::env::var(var).unwrap_or_else(|_| default.into()));
    let client = path("DEETS_EPIC_CLIENT_FILE", EPIC_CLIENT);
    let key = path("DEETS_EPIC_KEY_FILE", EPIC_KEY);
    Some(EpicSandbox::start(client, key).await)
}

/// The call store, when its settings are set (DATABASE.md). Without them, or when Postgres is
/// down, ehr serves calls without a store and says so once on stderr: FileCall is refused and
/// the next `ehr ingest` files the records it missed.
async fn open_store() -> Option<Arc<Store>> {
    let (Ok(url), Ok(key_file)) = (std::env::var("DEETS_DATABASE_URL"), std::env::var("DEETS_SEAL_KEY_FILE")) else {
        return None;
    };
    let key = match SealKey::from_dev_file(&PathBuf::from(key_file)) {
        Ok(key) => key,
        Err(e) => {
            eprintln!("ehr: no call store: {e}");
            return None;
        }
    };
    match Store::connect(&url, key).await {
        Ok(store) => Some(Arc::new(store)),
        Err(e) => {
            eprintln!("ehr: no call store: {e}");
            None
        }
    }
}

async fn serve<C: Codes>(
    listener: TcpListener,
    config: Arc<ServerConfig>,
    codes: C,
    store: Option<Arc<Store>>,
    epic: Option<Arc<EpicSandbox>>,
) -> Result<(), String> {
    let (health, health_service) = tonic_health::server::health_reporter();
    health.set_serving::<EhrServer<EhrService<FakeScheduler, FakePatients, C>>>().await;
    let mut service = EhrService::new(FakeScheduler::default(), FakePatients, codes);
    if let Some(store) = store {
        service = service.with_store(store);
    }
    if let Some(epic) = epic {
        service = service.with_epic_sandbox(epic);
    }
    let ehr = EhrServer::new(service)
        .max_decoding_message_size(MESSAGE_LIMIT)
        .max_encoding_message_size(MESSAGE_LIMIT);

    Server::builder()
        .timeout(REQUEST_LIMIT)
        .concurrency_limit_per_connection(64)
        .add_service(health_service)
        .add_service(ehr)
        .serve_with_incoming(transport::incoming(listener, config))
        .await
        .map_err(|_| "the server stopped".into())
}
