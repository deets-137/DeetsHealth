//! Makes the dev CA and the two certificates for mTLS between the agent and `ehr`
//! (EHR-SERVICE.md §6). Dev only: a deploy gets its certificates from the deploy's own CA.
//!
//!     cargo run -p deetshealth-ehr --example dev_certs [DIR]
//!
//! DIR defaults to `dev-certs` (git-ignored). It writes `ca.pem`, `ehr.pem`, `ehr.key`,
//! `agent.pem` and `agent.key`. The CA's own key is never written: no new certificate can be
//! made from this CA once the command ends. Run it again to start over.
//!
//! ECDSA P-256 with SHA-256: a FIPS-approved choice. `ehr`'s certificate can only serve
//! (serverAuth); the agent's can only connect (clientAuth). So neither can stand in for the
//! other.

use std::{fs, path::Path, path::PathBuf};

use rcgen::{
    date_time_ymd, BasicConstraints, CertificateParams, CertifiedIssuer, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};

#[allow(dead_code)] // tests/transport.rs includes this file for `write_dev_certs` only.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "dev-certs".into()));
    write_dev_certs(&dir)?;
    println!("wrote the dev CA and the ehr and agent certificates to {}", dir.display());
    Ok(())
}

pub fn write_dev_certs(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dir)?;

    let mut ca = CertificateParams::new(Vec::<String>::new())?;
    ca.distinguished_name.push(DnType::CommonName, "DeetsHealth dev CA");
    ca.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    ca.not_before = date_time_ymd(2026, 1, 1);
    ca.not_after = date_time_ymd(2028, 1, 1);
    let ca = CertifiedIssuer::self_signed(ca, KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?)?;
    fs::write(dir.join("ca.pem"), ca.pem())?;

    for (name, sans, purpose) in [
        ("ehr", vec!["ehr", "localhost", "127.0.0.1"], ExtendedKeyUsagePurpose::ServerAuth),
        ("agent", vec!["agent"], ExtendedKeyUsagePurpose::ClientAuth),
    ] {
        let mut p = CertificateParams::new(sans.into_iter().map(String::from).collect::<Vec<_>>())?;
        p.distinguished_name.push(DnType::CommonName, name);
        p.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        p.extended_key_usages = vec![purpose];
        p.not_before = date_time_ymd(2026, 1, 1);
        p.not_after = date_time_ymd(2028, 1, 1);
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let cert = p.signed_by(&key, &ca)?;
        fs::write(dir.join(format!("{name}.pem")), cert.pem())?;
        fs::write(dir.join(format!("{name}.key")), key.serialize_pem())?;
    }
    Ok(())
}
