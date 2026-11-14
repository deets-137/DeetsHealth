//! The store's sealing (DATABASE.md §1; SECURITY.md §4.6).
//!
//! [`AwsLc`] fills core's [`Cipher`] with AES-256-GCM, SHA-256 and random bytes from AWS-LC, the
//! provider the wire already uses (its FIPS module under the `fips` feature). core's `seal.rs`
//! owns the format around it: the padding, the format byte, and the associated data that binds a
//! value to its table, row and column.
//!
//! The key: in dev, 32 random bytes in a file (`DEETS_SEAL_KEY_FILE`, made by
//! scripts/postgres.mjs in the git-ignored `.pg/`). In the cloud, a data key from the hospital's
//! KMS (not built). A deploy (`DEETS_REQUIRE_FIPS=1`) refuses the dev key file.

use std::path::Path;

use aws_lc_rs::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN},
    digest, hkdf, hmac,
    rand::{SecureRandom, SystemRandom},
};
use deetshealth_core::{platform::Cipher, CoreError};
use zeroize::Zeroizing;

/// AES-256-GCM and friends from AWS-LC.
pub struct AwsLc;

/// One data key, and the lookup key made from it. Neither leaves this type; it has no `Debug`.
pub struct SealKey(LessSafeKey, hmac::Key);

/// The HKDF salt and info that make the lookup key from the data key: a key of its own for each
/// use, so a lookup hash never reuses the sealing key (docs/ops/DEMO.md §3.5).
const LOOKUP_SALT: &[u8] = b"deetshealth lookup v1";
const LOOKUP_INFO: &[u8] = b"users.email_hash";

impl SealKey {
    /// A key from its 32 bytes. The bytes are wiped by the caller (`Zeroizing`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        let unbound = UnboundKey::new(&AES_256_GCM, bytes).map_err(|_| CoreError::Crypto)?;
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, LOOKUP_SALT).extract(bytes);
        let okm = prk.expand(&[LOOKUP_INFO], hmac::HMAC_SHA256).map_err(|_| CoreError::Crypto)?;
        Ok(Self(LessSafeKey::new(unbound), hmac::Key::from(okm)))
    }

    /// A keyed hash (HMAC-SHA256) of `value` under the lookup key: equal values give equal
    /// hashes, so a sealed column can still be found by its value, and the hash reveals
    /// nothing without the key. `what` names the column, so two columns never share a hash.
    pub fn lookup(&self, what: &str, value: &str) -> [u8; 32] {
        let mut ctx = hmac::Context::with_key(&self.1);
        ctx.update(what.as_bytes());
        ctx.update(&[0]);
        ctx.update(value.as_bytes());
        let tag = ctx.sign();
        let mut out = [0u8; 32];
        out.copy_from_slice(tag.as_ref());
        out
    }

    /// The dev key from its file. Refused in a deploy, which takes its key from the KMS.
    pub fn from_dev_file(path: &Path) -> Result<Self, String> {
        if std::env::var("DEETS_REQUIRE_FIPS").as_deref() == Ok("1") {
            return Err("a deploy takes its sealing key from the KMS, never a dev key file".into());
        }
        let bytes = Zeroizing::new(std::fs::read(path).map_err(|_| "cannot read the dev sealing key file")?);
        if bytes.len() != 32 {
            return Err("the dev sealing key file must hold 32 bytes".into());
        }
        Self::from_bytes(&bytes).map_err(|_| "the dev sealing key is not a key".into())
    }
}

impl Cipher for AwsLc {
    type Key = SealKey;

    fn seal(&self, key: &SealKey, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut nonce = [0u8; NONCE_LEN];
        self.random(&mut nonce)?;
        let mut body = plaintext.to_vec();
        let sealed = key
            .0
            .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(aad), &mut body);
        let mut out = Vec::with_capacity(NONCE_LEN + body.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&body);
        // `body` held the plaintext before it was sealed in place; it is ciphertext now.
        sealed.map(|_| out).map_err(|_| CoreError::Crypto)
    }

    fn open(&self, key: &SealKey, aad: &[u8], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
        if sealed.len() < NONCE_LEN {
            return Err(CoreError::Crypto);
        }
        let (nonce, body) = sealed.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| CoreError::Crypto)?;
        let mut buf = Zeroizing::new(body.to_vec());
        let plain = key.0.open_in_place(nonce, Aad::from(aad), &mut buf).map_err(|_| CoreError::Crypto)?;
        Ok(Zeroizing::new(plain.to_vec()))
    }

    fn random(&self, out: &mut [u8]) -> Result<(), CoreError> {
        SystemRandom::new().fill(out).map_err(|_| CoreError::Crypto)
    }

    fn sha256(&self, data: &[u8]) -> Result<[u8; 32], CoreError> {
        let d = digest::digest(&digest::SHA256, data);
        d.as_ref().try_into().map_err(|_| CoreError::Crypto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deetshealth_core::seal::{open_text, seal, Place};

    fn key() -> SealKey {
        SealKey::from_bytes(&[7u8; 32]).unwrap()
    }

    #[test]
    fn a_sealed_value_opens_only_in_its_place() {
        let at = Place { table: "transcript_lines", row: "call-1/3", column: "words" };
        let sealed = seal(&AwsLc, &key(), &at, b"Jordan Ellis").unwrap();
        assert!(!sealed.windows(6).any(|w| w == b"Jordan"), "no plaintext in the sealed bytes");
        assert_eq!(&*open_text(&AwsLc, &key(), &at, &sealed).unwrap(), "Jordan Ellis");
        let elsewhere = Place { table: "transcript_lines", row: "call-1/4", column: "words" };
        assert!(open_text(&AwsLc, &key(), &elsewhere, &sealed).is_err(), "another row cannot open it");
        let other_key = SealKey::from_bytes(&[8u8; 32]).unwrap();
        assert!(open_text(&AwsLc, &other_key, &at, &sealed).is_err(), "another key cannot open it");
    }
}
