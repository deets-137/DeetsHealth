// Windows CNG: the app's only crypto (SECURITY.md §4.6, fork 10a). AES-256-GCM, random bytes
// and SHA-256 all come from the OS's FIPS 140-validated module; this file only calls it.
// Every `unsafe` block in the app's crypto lives here or in keystore.rs.
//
// Sealed layout from `seal`: nonce (12) ‖ ciphertext ‖ tag (16). The core adds its format byte
// in front (core/src/seal.rs).

use deetshealth_core::{platform::Cipher, CoreError};
use std::ffi::c_void;
use std::sync::Mutex;
use windows::Win32::Security::Cryptography::{
    BCryptDecrypt, BCryptDestroyKey, BCryptEncrypt, BCryptGenRandom, BCryptGenerateSymmetricKey, BCryptHash,
    BCRYPT_AES_GCM_ALG_HANDLE, BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO, BCRYPT_FLAGS, BCRYPT_KEY_HANDLE,
    BCRYPT_SHA256_ALG_HANDLE, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};
use zeroize::Zeroizing;

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
pub const KEY_LEN: usize = 32;
/// BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION in bcrypt.h; the crate does not export it.
const AUTH_INFO_VERSION: u32 = 1;

/// One site's AES-256 key, held inside CNG. The raw bytes are wiped right after import; the
/// app keeps only this handle, destroyed on drop.
pub struct CngKey(Mutex<BCRYPT_KEY_HANDLE>);

// The handle is a raw pointer, so Rust cannot mark it thread-safe itself. The Mutex lets one
// call use it at a time, and CNG itself is safe to call from any thread.
unsafe impl Send for CngKey {}
unsafe impl Sync for CngKey {}

impl CngKey {
    /// Import a raw key into CNG. The caller wipes `secret`.
    pub fn import(secret: &[u8; KEY_LEN]) -> Result<Self, CoreError> {
        let mut handle = BCRYPT_KEY_HANDLE::default();
        // SAFETY: the AES-GCM pseudo-handle needs no provider; CNG allocates the key object.
        unsafe { BCryptGenerateSymmetricKey(BCRYPT_AES_GCM_ALG_HANDLE, &mut handle, None, secret, 0) }
            .ok()
            .map_err(|_| CoreError::Crypto)?;
        Ok(Self(Mutex::new(handle)))
    }
}

impl Drop for CngKey {
    fn drop(&mut self) {
        let handle = *self.0.get_mut().unwrap_or_else(|p| p.into_inner());
        // SAFETY: made by BCryptGenerateSymmetricKey above and never used after this.
        unsafe {
            let _ = BCryptDestroyKey(handle);
        }
    }
}

/// The Windows crypto provider.
pub struct Cng;

/// The GCM parameters for one call. CNG reads the nonce and the associated data and writes
/// only the tag, so the `*mut` casts on the first two are never written through.
fn auth_info(nonce: &[u8; NONCE_LEN], aad: &[u8], tag: &mut [u8; TAG_LEN]) -> BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: std::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: AUTH_INFO_VERSION,
        pbNonce: nonce.as_ptr() as *mut u8,
        cbNonce: NONCE_LEN as u32,
        pbAuthData: if aad.is_empty() { std::ptr::null_mut() } else { aad.as_ptr() as *mut u8 },
        cbAuthData: aad.len() as u32,
        pbTag: tag.as_mut_ptr(),
        cbTag: TAG_LEN as u32,
        ..Default::default()
    }
}

impl Cipher for Cng {
    type Key = CngKey;

    fn seal(&self, key: &CngKey, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut nonce = [0u8; NONCE_LEN];
        self.random(&mut nonce)?;
        let mut tag = [0u8; TAG_LEN];
        let info = auth_info(&nonce, aad, &mut tag);
        let len = plaintext.len();
        let mut out = vec![0u8; NONCE_LEN + len + TAG_LEN];
        let mut written = 0u32;
        let handle = key.0.lock().map_err(|_| CoreError::Crypto)?;
        // SAFETY: `info` points into `nonce`, `aad` and `tag`, which all outlive the call; the
        // output slice is exactly the plaintext's length, as GCM needs.
        unsafe {
            BCryptEncrypt(
                *handle,
                Some(plaintext),
                Some(&info as *const _ as *const c_void),
                None,
                Some(&mut out[NONCE_LEN..NONCE_LEN + len]),
                &mut written,
                BCRYPT_FLAGS(0),
            )
        }
        .ok()
        .map_err(|_| CoreError::Crypto)?;
        drop(handle);
        if written as usize != len {
            return Err(CoreError::Crypto);
        }
        out[..NONCE_LEN].copy_from_slice(&nonce);
        out[NONCE_LEN + len..].copy_from_slice(&tag);
        Ok(out)
    }

    fn open(&self, key: &CngKey, aad: &[u8], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
        if sealed.len() < NONCE_LEN + TAG_LEN {
            return Err(CoreError::Crypto);
        }
        let (nonce, rest) = sealed.split_at(NONCE_LEN);
        let (body, tag) = rest.split_at(rest.len() - TAG_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CoreError::Crypto)?;
        let mut tag: [u8; TAG_LEN] = tag.try_into().map_err(|_| CoreError::Crypto)?;
        let info = auth_info(&nonce, aad, &mut tag);
        let mut out = Zeroizing::new(vec![0u8; body.len()]);
        let mut written = 0u32;
        let handle = key.0.lock().map_err(|_| CoreError::Crypto)?;
        // SAFETY: as in `seal`. A wrong key, wrong `aad` or changed byte fails the tag check
        // (STATUS_AUTH_TAG_MISMATCH), and `out` is wiped on drop.
        unsafe {
            BCryptDecrypt(
                *handle,
                Some(body),
                Some(&info as *const _ as *const c_void),
                None,
                Some(&mut out[..]),
                &mut written,
                BCRYPT_FLAGS(0),
            )
        }
        .ok()
        .map_err(|_| CoreError::Crypto)?;
        if written as usize != body.len() {
            return Err(CoreError::Crypto);
        }
        Ok(out)
    }

    fn random(&self, out: &mut [u8]) -> Result<(), CoreError> {
        // SAFETY: the system-preferred RNG needs no algorithm handle.
        unsafe { BCryptGenRandom(None, out, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
            .ok()
            .map_err(|_| CoreError::Crypto)
    }

    fn sha256(&self, data: &[u8]) -> Result<[u8; 32], CoreError> {
        let mut out = [0u8; 32];
        // SAFETY: the SHA-256 pseudo-handle needs no provider; `out` is the digest's size.
        unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out) }
            .ok()
            .map_err(|_| CoreError::Crypto)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> CngKey {
        let mut raw = Zeroizing::new([0u8; KEY_LEN]);
        Cng.random(&mut raw[..]).unwrap();
        CngKey::import(&raw).unwrap()
    }

    #[test]
    fn round_trip() {
        let k = key();
        let s = Cng.seal(&k, b"place", b"sandbox value").unwrap();
        assert_eq!(s.len(), NONCE_LEN + 13 + TAG_LEN);
        assert_eq!(&**Cng.open(&k, b"place", &s).unwrap(), b"sandbox value");
    }

    #[test]
    fn empty_value_and_empty_aad() {
        let k = key();
        let s = Cng.seal(&k, b"", b"").unwrap();
        assert!(Cng.open(&k, b"", &s).unwrap().is_empty());
    }

    #[test]
    fn wrong_key_wrong_place_or_one_changed_byte_fails() {
        let (k, other) = (key(), key());
        let s = Cng.seal(&k, b"place", b"value").unwrap();
        assert!(Cng.open(&other, b"place", &s).is_err());
        assert!(Cng.open(&k, b"other place", &s).is_err());
        for i in 0..s.len() {
            let mut t = s.clone();
            t[i] ^= 1;
            assert!(Cng.open(&k, b"place", &t).is_err(), "byte {i}");
        }
        assert!(Cng.open(&k, b"place", &s[..NONCE_LEN + TAG_LEN - 1]).is_err());
    }

    #[test]
    fn every_seal_has_a_fresh_nonce() {
        let k = key();
        let a = Cng.seal(&k, b"p", b"same").unwrap();
        let b = Cng.seal(&k, b"p", b"same").unwrap();
        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
        assert_ne!(a, b);
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn opens_the_standard_gcm_vectors() {
        // The GCM specification (McGrew & Viega), test cases 13 and 14: a 256-bit zero key,
        // a 96-bit zero nonce, no associated data. Proves CNG does real AES-256-GCM.
        let k = CngKey::import(&[0u8; KEY_LEN]).unwrap();
        let tc13 = unhex(&("00".repeat(12) + "530f8afbc74536b9a963b4f1c4cb738b"));
        assert!(Cng.open(&k, b"", &tc13).unwrap().is_empty());
        let tc14 = unhex(&("00".repeat(12) + "cea7403d4d606b6e074ec5d3baf39d18" + "d0d1c8a799996bf0265b98b5d48ab919"));
        assert_eq!(&**Cng.open(&k, b"", &tc14).unwrap(), &[0u8; 16]);
    }

    #[test]
    fn sha256_matches_the_standard_vector() {
        // FIPS 180-2 example: SHA-256("abc").
        let d = Cng.sha256(b"abc").unwrap();
        let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
