//! Core errors. Each is a kind with fixed text and carries no value, so any error may be
//! logged as it is (SECURITY.md §4.5: logs carry error kinds only).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    /// The cipher failed, or a sealed value would not open (wrong key, wrong row, tampered).
    Crypto,
    /// The platform key store could not make, open or destroy a site's key.
    KeyStore,
    /// A file or database operation failed.
    Io,
    /// The site could not be reached, or it answered with an error status.
    Network,
    /// A reply from the site was not what FHIR or SMART promises.
    Parse,
    /// Sign-in failed: a wrong `state`, a refused grant, or no sign-in waiting.
    SignIn,
    /// The platform could not open the system browser.
    Browser,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CoreError::Crypto => "crypto error",
            CoreError::KeyStore => "key store error",
            CoreError::Io => "storage error",
            CoreError::Network => "network error",
            CoreError::Parse => "unexpected reply",
            CoreError::SignIn => "sign-in error",
            CoreError::Browser => "could not open the browser",
        })
    }
}

impl std::error::Error for CoreError {}
