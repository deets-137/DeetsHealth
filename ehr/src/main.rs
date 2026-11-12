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
//! Output names ports, paths and fixed error strings. Never a request or a value.

use std::{net::IpAddr, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use deetshealth_ehr::{
    proto::v1::ehr_server::EhrServer,
    scheduler::FakeScheduler,
    service::EhrService,
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

async fn run() -> Result<(), String> {
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
    match texts {
        Some(codes) => serve(listener, config, codes).await,
        None => serve(listener, config, FakeCodes).await,
    }
}

async fn serve<C: Codes>(listener: TcpListener, config: Arc<ServerConfig>, codes: C) -> Result<(), String> {
    let (health, health_service) = tonic_health::server::health_reporter();
    health.set_serving::<EhrServer<EhrService<FakeScheduler, FakePatients, C>>>().await;
    let ehr = EhrServer::new(EhrService::new(FakeScheduler::default(), FakePatients, codes))
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
