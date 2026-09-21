//! Card payloads (CORE-CRATE.md §9). Stage 7 fills it: one payload struct per card view, with
//! plain values, built from the model here and nowhere else. Payloads derive `TS` so their
//! TypeScript types are generated into `src/model/`.
//!
//! This module is one of the two places that may read a [`crate::Shielded`] value.

/// The card payload's pass. Its field is private, so only this module can make one.
pub struct CardPass {
    _only_here: (),
}

impl CardPass {
    /// Made while one payload is built, then dropped. Unused until stage 7.
    #[allow(dead_code)]
    fn issue() -> Self {
        Self { _only_here: () }
    }
}
