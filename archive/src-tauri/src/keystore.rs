// One data key per site (SECURITY.md §4.6, fork 4a), wrapped by DPAPI for this Windows user.
//
// A site's key is 32 random bytes from CNG. On disk it is only a DPAPI blob in
// `<dir>/<site key>.key` (the `*.key` pattern is git-ignored). DPAPI ties the blob to this
// Windows user; the app's own entropy ties it to this app. The blob's description is left
// empty, because DPAPI stores the description in the clear. Destroying the file is the
// cryptographic erase at sign-out (SECURITY.md §8.3).

use crate::cng::{Cng, CngKey, KEY_LEN};
use deetshealth_core::platform::{Cipher, KeyStore, SiteKey};
use deetshealth_core::CoreError;
use std::io::Write;
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use zeroize::{Zeroize, Zeroizing};

/// Mixed into every blob, so another program running as this user cannot open it by
/// accident with a plain CryptUnprotectData call.
const ENTROPY: &[u8] = b"deetshealth.sitekey.v1";

pub struct DpapiKeyStore {
    dir: PathBuf,
}

impl DpapiKeyStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, site: &SiteKey) -> PathBuf {
        // SiteKey is 32 hex digits (core/src/platform.rs), so this is always a plain file name.
        self.dir.join(format!("{}.key", site.as_str()))
    }
}

fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    // DPAPI only reads an input blob, so the `*mut` cast is never written through.
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
}

/// Copy a blob that DPAPI allocated, then wipe and free DPAPI's copy.
///
/// SAFETY: `out` must come from a successful CryptProtectData / CryptUnprotectData call.
unsafe fn take_blob(out: CRYPT_INTEGER_BLOB) -> Zeroizing<Vec<u8>> {
    let theirs = std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize);
    let ours = Zeroizing::new(theirs.to_vec());
    theirs.zeroize();
    let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
    ours
}

fn protect(secret: &[u8]) -> Result<Vec<u8>, CoreError> {
    let (input, entropy) = (blob(secret), blob(ENTROPY));
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: both input blobs point at live slices; `out` is freed by take_blob.
    unsafe {
        CryptProtectData(&input, PCWSTR::null(), Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            .map_err(|_| CoreError::KeyStore)?;
        // The wrapped blob is not secret; it still goes through the same copy-and-free path.
        Ok(take_blob(out).to_vec())
    }
}

fn unprotect(wrapped: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
    let (input, entropy) = (blob(wrapped), blob(ENTROPY));
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as in `protect`. The plaintext key is wiped in DPAPI's buffer before it is freed.
    unsafe {
        CryptUnprotectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            .map_err(|_| CoreError::KeyStore)?;
        Ok(take_blob(out))
    }
}

/// Write a new file whole, or not at all: a temp file, flushed to disk, then renamed. Never
/// replaces an existing key (that would orphan the site's sealed data).
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    if path.exists() {
        return Err(CoreError::KeyStore);
    }
    let tmp = path.with_extension("key.tmp");
    let mut f = std::fs::File::create(&tmp).map_err(|_| CoreError::Io)?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|_| CoreError::Io)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|_| CoreError::Io)
}

impl KeyStore for DpapiKeyStore {
    type Key = CngKey;

    fn create(&self, site: &SiteKey) -> Result<CngKey, CoreError> {
        std::fs::create_dir_all(&self.dir).map_err(|_| CoreError::Io)?;
        let mut raw = Zeroizing::new([0u8; KEY_LEN]);
        Cng.random(&mut raw[..])?;
        let key = CngKey::import(&raw)?;
        write_new(&self.path(site), &protect(&raw[..])?)?;
        Ok(key)
    }

    fn open(&self, site: &SiteKey) -> Result<Option<CngKey>, CoreError> {
        let wrapped = match std::fs::read(self.path(site)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(CoreError::Io),
        };
        let raw = unprotect(&wrapped)?;
        let raw: Zeroizing<[u8; KEY_LEN]> =
            Zeroizing::new(raw.as_slice().try_into().map_err(|_| CoreError::KeyStore)?);
        CngKey::import(&raw).map(Some)
    }

    fn destroy(&self, site: &SiteKey) -> Result<(), CoreError> {
        let path = self.path(site);
        match std::fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(CoreError::Io),
            Ok(m) => {
                // Overwrite before delete. On an SSD this is not a guarantee (SECURITY.md §8.7);
                // the blob still needs this Windows user to open.
                if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(&path) {
                    let _ = f.write_all(&vec![0u8; m.len() as usize]).and_then(|_| f.sync_all());
                }
            }
        }
        std::fs::remove_file(&path).map_err(|_| CoreError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temp folder, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let site = SiteKey::generate(&Cng).unwrap();
            let p = std::env::temp_dir().join(format!("deetshealth-test-{}", site.as_str()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn create_open_destroy() {
        let dir = TempDir::new();
        let store = DpapiKeyStore::new(dir.0.clone());
        let site = SiteKey::generate(&Cng).unwrap();
        assert!(store.open(&site).unwrap().is_none());

        let made = store.create(&site).unwrap();
        let sealed = Cng.seal(&made, b"place", b"value").unwrap();
        let reopened = store.open(&site).unwrap().expect("key");
        assert_eq!(&**Cng.open(&reopened, b"place", &sealed).unwrap(), b"value");

        // The file on disk is a DPAPI blob, never the raw key.
        let on_disk = std::fs::read(store.path(&site)).unwrap();
        assert!(on_disk.len() > KEY_LEN);

        store.destroy(&site).unwrap();
        assert!(store.open(&site).unwrap().is_none());
        store.destroy(&site).unwrap(); // a second erase is fine
    }

    #[test]
    fn never_replaces_a_key() {
        let dir = TempDir::new();
        let store = DpapiKeyStore::new(dir.0.clone());
        let site = SiteKey::generate(&Cng).unwrap();
        store.create(&site).unwrap();
        assert!(matches!(store.create(&site), Err(CoreError::KeyStore)));
    }

    #[test]
    fn a_changed_blob_does_not_open() {
        let dir = TempDir::new();
        let store = DpapiKeyStore::new(dir.0.clone());
        let site = SiteKey::generate(&Cng).unwrap();
        store.create(&site).unwrap();
        let mut b = std::fs::read(store.path(&site)).unwrap();
        let mid = b.len() / 2;
        b[mid] ^= 1;
        std::fs::write(store.path(&site), &b).unwrap();
        assert!(store.open(&site).is_err());
    }
}
