//! The real server over real mTLS (EHR-SERVICE.md §6): fresh dev certificates in a temp
//! folder, a free port, the Phase 1 fakes. Proves who can connect, not what the calls do
//! (service.rs tests that).

#[path = "../examples/dev_certs.rs"]
mod dev_certs;

use std::{path::PathBuf, sync::Arc, time::Duration};

use deetshealth_ehr::{
    proto::v1::{ehr_client::EhrClient, ehr_server::EhrServer, OpenCallRequest},
    scheduler::FakeScheduler,
    service::EhrService,
    transport,
    verify::{FakeCodes, FakePatients},
};
use tokio::net::TcpListener;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity, Server};

struct Running {
    certs: PathBuf,
    port: u16,
}

async fn start() -> Running {
    let certs = std::env::temp_dir().join(format!("deets-ehr-test-{}", std::process::id()));
    dev_certs::write_dev_certs(&certs).unwrap();
    let config = transport::server_config(&certs).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let svc = EhrService::new(FakeScheduler::default(), FakePatients, FakeCodes);
    tokio::spawn(
        Server::builder()
            .add_service(EhrServer::new(svc))
            .serve_with_incoming(transport::incoming(listener, Arc::new(config))),
    );
    Running { certs, port }
}

async fn connect(r: &Running, identity: Option<&str>) -> Result<EhrClient<Channel>, ()> {
    let read = |f: &str| std::fs::read(r.certs.join(f)).unwrap();
    let mut tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(read("ca.pem")))
        .domain_name("ehr");
    if let Some(name) = identity {
        tls = tls.identity(Identity::from_pem(read(&format!("{name}.pem")), read(&format!("{name}.key"))));
    }
    let channel = Channel::from_shared(format!("https://127.0.0.1:{}", r.port))
        .unwrap()
        .tls_config(tls)
        .unwrap()
        .connect_timeout(Duration::from_secs(5))
        .connect()
        .await
        .map_err(|_| ())?;
    Ok(EhrClient::new(channel))
}

async fn can_open_a_call(r: &Running, identity: Option<&str>) -> bool {
    let Ok(mut client) = connect(r, identity).await else { return false };
    client.open_call(OpenCallRequest::default()).await.is_ok()
}

#[tokio::test]
async fn who_can_connect() {
    let r = start().await;
    assert!(can_open_a_call(&r, Some("agent")).await, "the agent's certificate gets in");
    assert!(!can_open_a_call(&r, None).await, "no certificate: refused");
    assert!(!can_open_a_call(&r, Some("ehr")).await, "ehr's certificate cannot act as a client");
    let _ = std::fs::remove_dir_all(&r.certs);
}
