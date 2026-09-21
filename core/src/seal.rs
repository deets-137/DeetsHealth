//! Sealing one value (SECURITY.md §4.6 and §8.2, CORE-CRATE.md §5).
//!
//! The platform's [`Cipher`] does the cryptography (Windows CNG, AES-256-GCM). This module
//! owns only the format around it:
//! - **Associated data** binds a value to its place: table, row key and column. A sealed value
//!   copied to another row, or to another column of the same row, fails to open.
//! - **A format byte** leads every sealed value, so a later change of algorithm can tell old
//!   values from new ones.
//!
//! It is not a pass holder: it takes and returns bytes. The cache writer in [`crate::store`]
//! reveals a [`crate::Shielded`] value with its pass and hands the bytes here.

use crate::platform::Cipher;
use crate::CoreError;
use zeroize::Zeroizing;

/// Format 1: the platform cipher's output (nonce ‖ ciphertext ‖ tag), AES-256-GCM.
const FORMAT_V1: u8 = 1;
/// Leads the associated data, so these bytes can never pass for another use of the same key.
const AAD_DOMAIN: &[u8] = b"deetshealth.cache.v1";

/// Where a value lives. Every part goes into the associated data.
pub struct Place<'a> {
    pub table: &'a str,
    pub row: &'a str,
    pub column: &'a str,
}

impl Place<'_> {
    /// Each part is length-prefixed, so no two places can make the same bytes.
    fn aad(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(AAD_DOMAIN.len() + 16 + self.table.len() + self.row.len() + self.column.len());
        for part in [AAD_DOMAIN, self.table.as_bytes(), self.row.as_bytes(), self.column.as_bytes()] {
            out.extend_from_slice(&(part.len() as u32).to_le_bytes());
            out.extend_from_slice(part);
        }
        out
    }
}

/// Seal one value for one place.
pub fn seal<C: Cipher>(cipher: &C, key: &C::Key, place: &Place, plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
    let body = cipher.seal(key, &place.aad(), plaintext)?;
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(FORMAT_V1);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Open one value from its place. The plaintext is wiped when it drops.
pub fn open<C: Cipher>(cipher: &C, key: &C::Key, place: &Place, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
    match sealed.split_first() {
        Some((&FORMAT_V1, body)) => cipher.open(key, &place.aad(), body),
        _ => Err(CoreError::Crypto),
    }
}

/// Open a value that holds text. Bytes that are not UTF-8 are wiped and refused.
pub fn open_text<C: Cipher>(cipher: &C, key: &C::Key, place: &Place, sealed: &[u8]) -> Result<Zeroizing<String>, CoreError> {
    let mut bytes = open(cipher, key, place, sealed)?;
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(s) => Ok(Zeroizing::new(s)),
        Err(e) => {
            drop(Zeroizing::new(e.into_bytes()));
            Err(CoreError::Crypto)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Not a cipher: a stand-in that records the associated data, so these tests check this
    /// module's format only. The real AES-GCM is tested in the host (src-tauri/src/cng.rs).
    struct Recorder;
    impl Cipher for Recorder {
        type Key = ();
        fn seal(&self, _: &(), aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
            let mut v = (aad.len() as u32).to_le_bytes().to_vec();
            v.extend_from_slice(aad);
            v.extend_from_slice(plaintext);
            Ok(v)
        }
        fn open(&self, _: &(), aad: &[u8], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
            let n = u32::from_le_bytes(sealed.get(..4).ok_or(CoreError::Crypto)?.try_into().unwrap()) as usize;
            let (a, rest) = sealed[4..].split_at(n);
            if a != aad {
                return Err(CoreError::Crypto);
            }
            Ok(Zeroizing::new(rest.to_vec()))
        }
        fn random(&self, out: &mut [u8]) -> Result<(), CoreError> {
            out.fill(0);
            Ok(())
        }
        fn sha256(&self, _: &[u8]) -> Result<[u8; 32], CoreError> {
            Ok([0; 32])
        }
    }

    const HERE: Place = Place { table: "result", row: "r1", column: "value" };

    #[test]
    fn round_trip_text() {
        let s = seal(&Recorder, &(), &HERE, "7.2".as_bytes()).unwrap();
        assert_eq!(s[0], FORMAT_V1);
        assert_eq!(&*open_text(&Recorder, &(), &HERE, &s).unwrap(), "7.2");
    }

    #[test]
    fn another_row_or_column_does_not_open() {
        let s = seal(&Recorder, &(), &HERE, b"x").unwrap();
        let other_row = Place { row: "r2", ..HERE };
        let other_col = Place { column: "unit", ..HERE };
        assert_eq!(open(&Recorder, &(), &other_row, &s).unwrap_err(), CoreError::Crypto);
        assert_eq!(open(&Recorder, &(), &other_col, &s).unwrap_err(), CoreError::Crypto);
    }

    #[test]
    fn places_never_collide() {
        // Without length prefixes, ("ab", "c") and ("a", "bc") would give the same bytes.
        let a = Place { table: "t", row: "ab", column: "c" }.aad();
        let b = Place { table: "t", row: "a", column: "bc" }.aad();
        assert_ne!(a, b);
    }

    #[test]
    fn unknown_format_is_refused() {
        let mut s = seal(&Recorder, &(), &HERE, b"x").unwrap();
        s[0] = 9;
        assert_eq!(open(&Recorder, &(), &HERE, &s).unwrap_err(), CoreError::Crypto);
        assert_eq!(open(&Recorder, &(), &HERE, &[]).unwrap_err(), CoreError::Crypto);
    }
}
