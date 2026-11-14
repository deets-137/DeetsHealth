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
//!     ehr epic-sandbox
//!
//! - Prints the Chat page's Epic list as JSON (epic_ehr.rs `sandbox_list`): Epic's public test
//!   patients and their visits ahead. Dev only; the dev server runs it.
//!
//! Output names ports, paths and fixed error strings. Never a request or a value.

use std::{net::IpAddr, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use deetshealth_ehr::{
    epic::{Epic, EpicConfig},
    epic_ehr::{self, EpicEhr},
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

/// `ehr epic-find`: probes Epic's sandbox `Appointment/$find` (STU3) for step E-4 (EPIC.md §5.2).
/// Reads one booked visit of a public test patient for a real visit type and department, then
/// sends `$find` a few ways, each adding one input. Prints, per try: the HTTP status, each
/// OperationOutcome issue (Epic's own words), and for a Bundle the count and shape of what came
/// back (field names, statuses, visit type words). Never an id, a token or a patient's value.
/// About thirteen requests (nine `$find`s); `$find` books nothing.
async fn epic_find() -> Result<(), String> {
    use serde_json::{json, Value};
    let config = EpicConfig::sandbox(&PathBuf::from(EPIC_CLIENT), &PathBuf::from(EPIC_KEY))?;
    let epic = Epic::connect(config).await.map_err(|e| e.to_string())?;
    // Camila Lopez (EPIC.md §5.2): a rolling set of booked visits gives a real visit type and
    // department. A public test patient.
    let patient = "erXuFYUfucBZaryVksYEcMg3";
    let mut url = epic.base().join("Appointment").map_err(|_| "bad URL")?;
    url.query_pairs_mut()
        .append_pair("patient", patient)
        .append_pair("service-category", "appointment")
        .append_pair("_count", "5");
    let bundle = epic.read(url).await.map_err(|e| e.to_string())?;
    let visit = bundle["entry"]
        .as_array()
        .and_then(|e| e.iter().map(|x| &x["resource"]).find(|r| r["resourceType"] == "Appointment"))
        .cloned()
        .ok_or("no Appointment to learn a visit type from")?;
    let coding = visit["serviceType"][0]["coding"][0].clone();
    let location = visit["participant"]
        .as_array()
        .and_then(|p| p.iter().find_map(|x| x["actor"]["reference"].as_str().filter(|r| r.starts_with("Location/"))))
        .map(str::to_string);
    println!(
        "a booked visit: serviceType system {}, display {:?}; a Location participant: {}",
        coding["system"].as_str().unwrap_or("-"),
        coding["display"].as_str().unwrap_or("-"),
        if location.is_some() { "yes" } else { "no" }
    );

    // Epic's spec (api:840, EPIC.md §5.2): `patient` is a Patient resource that Epic matches by
    // demographics. Read the test patient and keep the fields the spec lists.
    let record = epic.read(epic.base().join(&format!("Patient/{patient}")).map_err(|_| "bad URL")?).await.map_err(|e| e.to_string())?;
    let mut demographics = json!({"resourceType": "Patient"});
    let mut kept = Vec::new();
    for field in ["identifier", "name", "birthDate", "gender", "telecom", "address"] {
        if !record[field].is_null() {
            demographics[field] = record[field].clone();
            kept.push(field);
        }
    }
    println!("the test patient's demographics: {kept:?}");

    let stu3 = reqwest::Url::parse(&epic.base().as_str().replace("/R4/", "/STU3/")).map_err(|_| "bad STU3 URL")?;
    let find = stu3.join("Appointment/$find").map_err(|_| "bad $find URL")?;
    // The window: the next 14 days, in the sandbox's zone (America/Chicago, -06:00 in November).
    let start = jiff::Zoned::now().date().checked_add(jiff::Span::new().days(1)).map_err(|_| "date")?;
    let end = start.checked_add(jiff::Span::new().days(14)).map_err(|_| "date")?;
    let p = |name: &str, v: Value| {
        let mut o = v;
        o["name"] = json!(name);
        o
    };
    let as_resource = p("patient", json!({"resource": {"resourceType": "Patient", "id": patient}}));
    let as_reference = p("patient", json!({"valueReference": {"reference": format!("Patient/{patient}")}}));
    let times = vec![
        p("startTime", json!({"valueDateTime": format!("{start}T08:00:00-06:00")})),
        p("endTime", json!({"valueDateTime": format!("{end}T17:00:00-06:00")})),
    ];
    let service = p("serviceType", json!({"valueCodeableConcept": {"coding": [coding.clone()]}}));
    let place = location.map(|l| p("location-reference", json!({"valueReference": {"reference": l}})));
    let mut tries: Vec<(&str, Vec<Value>)> = vec![
        ("patient as a resource, times", [vec![as_resource.clone()], times.clone()].concat()),
        ("patient as a reference, times", [vec![as_reference.clone()], times.clone()].concat()),
        ("patient as a resource, times, serviceType", [vec![as_resource.clone()], times.clone(), vec![service.clone()]].concat()),
        ("patient as a reference, times, serviceType", [vec![as_reference.clone()], times.clone(), vec![service.clone()]].concat()),
    ];
    // Tries from Epic's spec (2026-11-14): the patient's demographics; the table's spelling
    // `service-type`; Epic's own sample visit type, indications and window.
    let as_demographics = p("patient", json!({"resource": demographics}));
    let service_dashed = p("service-type", json!({"valueCodeableConcept": {"coding": [coding.clone()]}}));
    let sample_service = p(
        "serviceType",
        json!({"valueCodeableConcept": {"coding": [{"system": "urn:oid:1.2.840.114350.1.13.0.1.7.3.808267.11", "code": "95014", "display": "Office Visit"}]}}),
    );
    let sample_indications = p(
        "indications",
        json!({"valueCodeableConcept": {"coding": [
            {"system": "urn:oid:2.16.840.1.113883.6.96", "code": "46866001", "display": "Fracture of lower limb (disorder)"},
            {"system": "urn:oid:2.16.840.1.113883.6.90", "code": "S82.90XA", "display": "Broken leg"}
        ], "text": "Broken leg"}}),
    );
    let sample_times = vec![
        p("startTime", json!({"valueDateTime": format!("{start}T13:00:00Z")})),
        p("endTime", json!({"valueDateTime": format!("{end}T22:00:00Z")})),
    ];
    let place_or_none: Vec<Value> = place.iter().cloned().collect();
    if let Some(place) = place.clone() {
        tries.push(("patient as a resource, times, serviceType, location", [vec![as_resource], times.clone(), vec![service.clone(), place.clone()]].concat()));
        tries.push(("patient as a reference, times, serviceType, location", [vec![as_reference], times.clone(), vec![service.clone(), place]].concat()));
    }
    tries.push(("patient's demographics, times, serviceType, location", [vec![as_demographics.clone()], times.clone(), vec![service], place_or_none.clone()].concat()));
    tries.push(("patient's demographics, times, service-type, location", [vec![as_demographics.clone()], times, vec![service_dashed], place_or_none.clone()].concat()));
    tries.push(("Epic's sample: demographics, visit type 95014, indications, location", [vec![as_demographics], sample_times, vec![sample_service, sample_indications], place_or_none].concat()));
    for (label, parameter) in tries {
        let body = json!({"resourceType": "Parameters", "parameter": parameter});
        let (status, reply) = epic.post(find.clone(), &body).await.map_err(|e| e.to_string())?;
        println!("\n$find, {label}: HTTP {status}");
        let Some(reply) = reply else { continue };
        let outcomes: Vec<&Value> = match reply["resourceType"].as_str() {
            Some("OperationOutcome") => vec![&reply],
            _ => reply["entry"].as_array().map(|e| e.iter().map(|x| &x["resource"]).filter(|r| r["resourceType"] == "OperationOutcome").collect()).unwrap_or_default(),
        };
        for o in outcomes {
            for issue in o["issue"].as_array().into_iter().flatten() {
                let words = issue["details"]["text"].as_str().or(issue["diagnostics"].as_str()).unwrap_or("-");
                let code = issue["details"]["coding"][0]["code"].as_str().unwrap_or("-");
                println!("  issue: {} {} [{}] {}", issue["severity"].as_str().unwrap_or("-"), issue["code"].as_str().unwrap_or("-"), code, words.chars().take(200).collect::<String>());
            }
        }
        let found: Vec<&Value> = reply["entry"].as_array().map(|e| e.iter().map(|x| &x["resource"]).filter(|r| r["resourceType"] == "Appointment").collect()).unwrap_or_default();
        if reply["resourceType"] == "Bundle" {
            println!("  Bundle: {} proposed appointment(s)", found.len());
        }
        if let Some(a) = found.first() {
            let keys: Vec<&String> = a.as_object().map(|o| o.keys().collect()).unwrap_or_default();
            println!("  first: status {}, fields {keys:?}", a["status"].as_str().unwrap_or("-"));
            println!("  its serviceType: {:?}", a["serviceType"][0]["coding"][0]["display"].as_str().or(a["serviceType"][0]["text"].as_str()).unwrap_or("-"));
            let slot = a["contained"].as_array().and_then(|c| c.iter().find(|x| x["resourceType"] == "Slot"));
            println!("  contained Slot: {}", slot.map(|s| s["status"].as_str().unwrap_or("-")).unwrap_or("none"));
            let actors: Vec<&str> = a["participant"].as_array().into_iter().flatten().filter_map(|x| x["actor"]["reference"].as_str()).map(|r| r.split('/').next().unwrap_or("-")).collect();
            println!("  participants: {actors:?}");
        }
    }
    Ok(())
}

async fn run() -> Result<(), String> {
    let mut first = std::env::args().skip(1).peekable();
    if first.peek().map(String::as_str) == Some("epic-find") {
        return epic_find().await;
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
/// When the sign-in fails, ehr serves the local fakes only and says so once on stderr: a call
/// that asks for Epic is refused.
async fn open_epic() -> Option<Arc<EpicEhr>> {
    if std::env::var("DEETS_EPIC_SANDBOX").as_deref() != Ok("1") {
        return None;
    }
    // The dev server passes both paths, absolute: ehr runs from agent/.
    let path = |var: &str, default: &str| PathBuf::from(std::env::var(var).unwrap_or_else(|_| default.into()));
    let client = path("DEETS_EPIC_CLIENT_FILE", EPIC_CLIENT);
    let key = path("DEETS_EPIC_KEY_FILE", EPIC_KEY);
    match epic_ehr::connect_sandbox(&client, &key).await {
        Ok(epic) => Some(Arc::new(epic)),
        Err(e) => {
            eprintln!("ehr: no Epic sandbox: {e}");
            None
        }
    }
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
    epic: Option<Arc<EpicEhr>>,
) -> Result<(), String> {
    let (health, health_service) = tonic_health::server::health_reporter();
    health.set_serving::<EhrServer<EhrService<FakeScheduler, FakePatients, C>>>().await;
    let mut service = EhrService::new(FakeScheduler::default(), FakePatients, codes);
    if let Some(store) = store {
        service = service.with_store(store);
    }
    if let Some(epic) = epic {
        service = service.with_epic(epic);
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
