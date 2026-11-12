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
//! Output names ports, paths and fixed error strings. Never a request or a value.

use std::{net::IpAddr, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use deetshealth_ehr::{
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

async fn run() -> Result<(), String> {
    let mut first = std::env::args().skip(1).peekable();
    if first.peek().map(String::as_str) == Some("ingest") {
        first.next();
        return ingest(first).await;
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
    match texts {
        Some(codes) => serve(listener, config, codes, store).await,
        None => serve(listener, config, FakeCodes, store).await,
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
) -> Result<(), String> {
    let (health, health_service) = tonic_health::server::health_reporter();
    health.set_serving::<EhrServer<EhrService<FakeScheduler, FakePatients, C>>>().await;
    let mut service = EhrService::new(FakeScheduler::default(), FakePatients, codes);
    if let Some(store) = store {
        service = service.with_store(store);
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
