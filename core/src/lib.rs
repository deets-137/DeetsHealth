//! DeetsHealth's web-ready core (docs/architecture/CORE-CRATE.md).
//!
//! It signs in, fetches, seals, stores and normalizes, with no Tauri dependency, so it also
//! builds for `wasm32-unknown-unknown`. What differs per platform comes in through the traits
//! in [`platform`]; the Tauri host (`src-tauri`) fills them on Windows.
//!
//! Every protected value (PHI and PII, SECURITY.md §2) lives in [`Shielded`] from the moment
//! it is parsed. Only four places can read one: [`store`] (the cache writer), [`payload`]
//! (the card payload), [`net`] (a request to the value's own site) and the `ehr` service's
//! wire edge, through [`wire::WirePass`]. Another reader is a change to `shielded.rs`, on
//! purpose.
//!
//! Build stages (CORE-CRATE.md §2): 1 skeleton ✓ · 2 seal ✓ · 3 store ✓ · 4 auth ✓ ·
//! 5 fhir + sync ✓ · 6 index and links ✓ · 7 the Sites payloads and events ✓ · 8 refresh.
//! The normalizer and each card's payload come card by card.

pub mod auth;
/// The dev inspector: feature `dev` only, never in a release (docs/ops/DEBUGGING.md).
#[cfg(feature = "dev")]
pub mod dev;
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
pub mod wire;

pub use error::CoreError;
pub use shielded::{Pass, Shielded};
