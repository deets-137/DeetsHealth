//! What differs per platform (DESIGN.md §6, CORE-CRATE.md §3). The Tauri host fills these on
//! Windows; a web version fills them later with WebCrypto, non-extractable keys and IndexedDB.
//!
//! A key is an associated type, never bytes the core can see: on the web it will be a
//! non-extractable `CryptoKey`, and on Windows it stays inside the host's CNG glue.
//!
//! The sign-in return is not a trait: the platform pushes the link to the core (stage 4).

use crate::CoreError;
use zeroize::Zeroizing;

/// The crypto provider (SECURITY.md §4.6, fork 10a: Windows CNG on the desktop).
pub trait Cipher {
    type Key;

    /// AES-256-GCM. The output is nonce ‖ ciphertext ‖ tag, with a fresh random 96-bit nonce.
    /// `aad` binds the value to its table and row (CORE-CRATE.md §5).
    fn seal(&self, key: &Self::Key, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CoreError>;

    /// The reverse of [`Cipher::seal`]. Fails with [`CoreError::Crypto`] on a wrong key, a
    /// wrong `aad` or any change to the sealed bytes. The plaintext is wiped when it drops.
    fn open(&self, key: &Self::Key, aad: &[u8], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError>;

    /// Bytes from the platform's validated random source.
    fn random(&self, out: &mut [u8]) -> Result<(), CoreError>;

    /// SHA-256, for the PKCE challenge.
    fn sha256(&self, data: &[u8]) -> Result<[u8; 32], CoreError>;
}

/// One data key per site (SECURITY.md §4.6, fork 4a), wrapped for this user only.
pub trait KeyStore {
    type Key;

    /// Make and store a new key for a site. Called once, at the site's first sign-in.
    fn create(&self, site: &SiteKey) -> Result<Self::Key, CoreError>;

    /// The site's key, or `None` if it has none.
    fn open(&self, site: &SiteKey) -> Result<Option<Self::Key>, CoreError>;

    /// Destroy the site's key: the cryptographic erase at sign-out (SECURITY.md §8.3).
    fn destroy(&self, site: &SiteKey) -> Result<(), CoreError>;
}

/// Time, from the platform: the core has no clock of its own (none exists on the web).
pub trait Timer {
    /// Seconds since 1970.
    fn now(&self) -> u64;
    /// Wait. The fetch uses it for `Retry-After` and back-off only.
    fn sleep(&self, seconds: u64) -> impl std::future::Future<Output = ()>;
}

/// Opens the system browser for sign-in. Never a WebView2 popup (SECURITY.md §4.4).
pub trait OpenBrowser {
    fn open_url(&self, url: &str) -> Result<(), CoreError>;
}

/// The app's own opaque name for one connected site: random, made at the first sign-in. It
/// names a file and a key, never the site, so the site's real name stays sealed. It still
/// has no `Debug` or `Display`: logs carry no id of any kind (SECURITY.md §4.5).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SiteKey(String);

impl SiteKey {
    /// A new name for a site at its first sign-in: 128 random bits as lowercase hex.
    pub fn generate<C: Cipher>(cipher: &C) -> Result<Self, CoreError> {
        random_hex(cipher).map(Self)
    }

    /// Wrap a key already made (read back from the store). Refuses anything but 32 lowercase
    /// hex digits, so a key can always be used as a file name.
    pub fn from_stored(s: String) -> Result<Self, CoreError> {
        if is_hex_key(&s) {
            Ok(Self(s))
        } else {
            Err(CoreError::KeyStore)
        }
    }

    /// The text form, for a file name or a store row.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 128 random bits as 32 lowercase hex digits: the form of every opaque key the app makes.
pub(crate) fn random_hex<C: Cipher>(cipher: &C) -> Result<String, CoreError> {
    let mut bytes = [0u8; 16];
    cipher.random(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub(crate) fn is_hex_key(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
