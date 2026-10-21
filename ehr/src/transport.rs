//! mTLS for the wire (EHR-SERVICE.md §6; the owner, 2026-10-21): TLS 1.3 only, on AWS-LC,
//! with its FIPS module in the deploy build. tonic serves the TLS streams made here; it builds
//! no TLS config of its own, so every setting is in this file.

use std::{io, path::Path, sync::Arc, time::Duration};

use rustls::{crypto::CryptoProvider, server::WebPkiClientVerifier, RootCertStore, ServerConfig};
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tokio_stream::wrappers::ReceiverStream;

/// A handshake that takes longer than this is dropped.
const HANDSHAKE_LIMIT: Duration = Duration::from_secs(5);

/// The crypto: AWS-LC's FIPS module in the deploy build, the same library without it on dev.
pub fn provider() -> Arc<CryptoProvider> {
    #[cfg(feature = "fips")]
    let p = rustls::crypto::default_fips_provider();
    #[cfg(not(feature = "fips"))]
    let p = rustls::crypto::aws_lc_rs::default_provider();
    Arc::new(p)
}

/// The server side: `ehr`'s certificate, and only clients the dev CA (or the deploy's CA)
/// signed for client use. Every error is a fixed string naming the file, never its content.
pub fn server_config(certs: &Path) -> Result<ServerConfig, String> {
    let read_certs = |name: &str| -> Result<Vec<CertificateDer<'static>>, String> {
        CertificateDer::pem_file_iter(certs.join(name))
            .and_then(|it| it.collect::<Result<Vec<_>, _>>())
            .map_err(|_| format!("cannot read {name} in {}", certs.display()))
    };
    let mut roots = RootCertStore::empty();
    for ca in read_certs("ca.pem")? {
        roots.add(ca).map_err(|_| "ca.pem: not a usable CA certificate".to_string())?;
    }
    let chain = read_certs("ehr.pem")?;
    let key = PrivateKeyDer::from_pem_file(certs.join("ehr.key"))
        .map_err(|_| format!("cannot read ehr.key in {}", certs.display()))?;

    let verifier = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider())
        .build()
        .map_err(|_| "cannot build the client check".to_string())?;
    let mut config = ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| "TLS 1.3 is not available".to_string())?
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain, key)
        .map_err(|_| "ehr.pem and ehr.key do not match".to_string())?;
    config.alpn_protocols = vec![b"h2".to_vec()];
    Ok(config)
}

/// A deploy sets `DEETS_REQUIRE_FIPS=1`. Then `ehr` will not start unless its TLS is in FIPS
/// mode. On dev the variable is unset.
pub fn fips_required() -> bool {
    std::env::var("DEETS_REQUIRE_FIPS").is_ok_and(|v| v == "1")
}

/// Each accepted TCP connection, after its TLS handshake, as a stream tonic can serve. A
/// failed or slow handshake is dropped; it never stops the listener.
pub fn incoming(
    listener: TcpListener,
    config: Arc<ServerConfig>,
) -> ReceiverStream<Result<TlsStream<tokio::net::TcpStream>, io::Error>> {
    let acceptor = TlsAcceptor::from(config);
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            };
            let (acceptor, tx) = (acceptor.clone(), tx.clone());
            tokio::spawn(async move {
                if let Ok(Ok(tls)) = tokio::time::timeout(HANDSHAKE_LIMIT, acceptor.accept(tcp)).await {
                    let _ = tx.send(Ok(tls)).await;
                }
            });
        }
    });
    ReceiverStream::new(rx)
}
