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

async fn run() -> Result<(), String> {
    let mut first = std::env::args().skip(1).peekable();
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
