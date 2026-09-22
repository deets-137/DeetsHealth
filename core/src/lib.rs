//! DeetsHealth's web-ready core (docs/architecture/CORE-CRATE.md).
//!
//! It signs in, fetches, seals, stores and normalizes, with no Tauri dependency, so it also
//! builds for `wasm32-unknown-unknown`. What differs per platform comes in through the traits
//! in [`platform`]; the Tauri host (`src-tauri`) fills them on Windows.
//!
//! Every protected value (PHI and PII, SECURITY.md §2) lives in [`Shielded`] from the moment
//! it is parsed. Only three modules can read one: [`store`] (the cache writer), [`payload`]
//! (the card payload) and [`net`] (a request to the value's own site). Another reader is a
//! change to `shielded.rs`, on purpose.
//!
//! Build stages (CORE-CRATE.md §2): 1 skeleton ✓ · 2 seal ✓ · 3 store ✓ · 4 auth ✓ ·
//! 5 fhir + sync · 6 index + normalize · 7 payloads + the Sites section · 8 refresh and erase.

pub mod auth;
pub mod error;
pub mod fhir;
pub mod index;
pub mod net;
pub mod payload;
pub mod platform;
pub mod seal;
pub mod shielded;
pub mod store;
pub mod sync;

pub use error::CoreError;
pub use shielded::{Pass, Shielded};
