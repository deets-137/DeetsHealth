//! Sealing one value (SECURITY.md §4.6 and §8.2, CORE-CRATE.md §5).
//!
//! The platform's [`Cipher`] does the cryptography (Windows CNG, AES-256-GCM). This module
//! owns only the format around it:
//! - **Associated data** binds a value to its place: table, row key and column. A sealed value
//!   copied to another row, or to another column of the same row, fails to open.
//! - **A format byte** leads every sealed value, so a later change of algorithm can tell old
//!   values from new ones.
//! - **Padding** (format 2, the owner, 2026-09-21): the plaintext is padded before it is
//!   sealed, so the sealed length shows only a rough size class. Every value under 64 bytes
//!   (a missing value, a date, a code) seals to the same length; a longer one is padded by
//!   the Padmé rule, at most about 12% extra (docs/architecture/STORE.md §5.1).
//!
//! It is not a pass holder: it takes and returns bytes. The cache writer in [`crate::store`]
//! reveals a [`crate::Shielded`] value with its pass and hands the bytes here.

use crate::platform::Cipher;
use crate::CoreError;
use zeroize::Zeroizing;

/// Format 1: the platform cipher's output (nonce ‖ ciphertext ‖ tag), AES-256-GCM, over the
/// plaintext as it is. Still opened, so a schema 1 file can be migrated; never written.
const FORMAT_V1: u8 = 1;
/// Format 2: as format 1, over the padded plaintext: a 4-byte length (little-endian), the
/// plaintext, then zeros up to [`padded_len`].
const FORMAT_V2: u8 = 2;
/// The smallest padded plaintext.
pub const PAD_FLOOR: usize = 64;

/// The padded length for `n` bytes (length prefix included): at least [`PAD_FLOOR`], then
/// Padmé (Nikitin et al., "Reducing Metadata Leakage from Encrypted Files and Communication
/// with PURBs", 2019): keep the top bits of the length and round the rest up, so the length
/// leaks O(log log n) bits and the overhead stays under about 12%.
pub fn padded_len(n: usize) -> usize {
    if n <= PAD_FLOOR {
        return PAD_FLOOR;
    }
    let e = usize::BITS - 1 - n.leading_zeros(); // floor(log2 n)
    let s = u32::BITS - e.leading_zeros(); // floor(log2 e) + 1
    let mask = (1usize << (e - s)) - 1;
    (n + mask) & !mask
}

fn pad(plaintext: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
    let len = u32::try_from(plaintext.len()).map_err(|_| CoreError::Crypto)?;
    let total = padded_len(4 + plaintext.len());
    let mut out = Zeroizing::new(Vec::with_capacity(total));
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(plaintext);
    out.resize(total, 0);
    Ok(out)
}

fn unpad(mut padded: Zeroizing<Vec<u8>>) -> Result<Zeroizing<Vec<u8>>, CoreError> {
    let len = padded.get(..4).ok_or(CoreError::Crypto)?;
    let len = u32::from_le_bytes(len.try_into().map_err(|_| CoreError::Crypto)?) as usize;
    let end = 4usize.checked_add(len).filter(|e| *e <= padded.len()).ok_or(CoreError::Crypto)?;
    if padded[end..].iter().any(|b| *b != 0) {
        return Err(CoreError::Crypto);
    }
    padded.truncate(end);
    padded.drain(..4);
    Ok(padded)
}
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

/// Seal one value for one place, padded (format 2).
pub fn seal<C: Cipher>(cipher: &C, key: &C::Key, place: &Place, plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
    let body = cipher.seal(key, &place.aad(), &pad(plaintext)?)?;
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(FORMAT_V2);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Open one value from its place (format 2, or format 1 from a schema 1 file). The plaintext
/// is wiped when it drops.
pub fn open<C: Cipher>(cipher: &C, key: &C::Key, place: &Place, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
    match sealed.split_first() {
        Some((&FORMAT_V2, body)) => unpad(cipher.open(key, &place.aad(), body)?),
        Some((&FORMAT_V1, body)) => cipher.open(key, &place.aad(), body),
        _ => Err(CoreError::Crypto),
    }
}

/// Format 1, as schema 1 wrote it: for the migration tests only.
#[cfg(test)]
pub(crate) fn seal_v1<C: Cipher>(cipher: &C, key: &C::Key, place: &Place, plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
    let mut out = vec![FORMAT_V1];
    out.extend(cipher.seal(key, &place.aad(), plaintext)?);
    Ok(out)
}

/// Whether a sealed value is in the current format. The schema 2 migration re-seals the rest.
pub fn is_current(sealed: &[u8]) -> bool {
    sealed.first() == Some(&FORMAT_V2)
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
        assert_eq!(s[0], FORMAT_V2);
        assert_eq!(&*open_text(&Recorder, &(), &HERE, &s).unwrap(), "7.2");
        assert_eq!(&*open_text(&Recorder, &(), &HERE, &seal(&Recorder, &(), &HERE, b"").unwrap()).unwrap(), "");
    }

    /// The owner's rule: every short value seals to one length; a long one to a size class.
    #[test]
    fn padding_hides_short_lengths() {
        let len = |p: &[u8]| seal(&Recorder, &(), &HERE, p).unwrap().len();
        let short = len(b"");
        for p in [&b"x"[..], b"2019-04-02T10:00:00-05:00", &[b'a'; 60]] {
            assert_eq!(len(p), short, "under the floor, one length");
        }
        assert_eq!(padded_len(1), PAD_FLOOR);
        assert_eq!(padded_len(64), 64);
        assert_eq!(padded_len(100), 104);
        assert_eq!(padded_len(1000), 1024);
        for n in [65, 99, 1_000, 4_097, 70_000, 1_000_000] {
            let p = padded_len(n);
            assert!(p >= n && (p - n) * 100 <= n * 12, "Padmé overhead for {n}: {p}");
        }
        // Many lengths, few classes: the 1,000 lengths from 1,000 to 1,999 give 17.
        let classes: std::collections::BTreeSet<_> = (1000..2000).map(padded_len).collect();
        assert_eq!(classes.len(), 17);
    }

    #[test]
    fn bad_padding_is_refused() {
        let mut padded = pad(b"abc").unwrap();
        let last = padded.len() - 1;
        padded[last] = 1;
        assert_eq!(unpad(padded).unwrap_err(), CoreError::Crypto);
        let mut long = pad(b"abc").unwrap();
        long[..4].copy_from_slice(&1000u32.to_le_bytes());
        assert_eq!(unpad(long).unwrap_err(), CoreError::Crypto);
    }

    #[test]
    fn format_1_still_opens() {
        let mut s = vec![FORMAT_V1];
        s.extend(Recorder.seal(&(), &HERE.aad(), b"old").unwrap());
        assert_eq!(&*open(&Recorder, &(), &HERE, &s).unwrap(), b"old");
        assert!(!is_current(&s));
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
