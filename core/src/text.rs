//! The pass for the `ehr` service's text message: the verification code, sent to the number on
//! file (SECURITY.md §4.4; CHAT.md §8).
//!
//! The text service's request must carry the number and the code in plain form. That happens
//! in one file, `ehr/src/texts.rs`, and nowhere else. As with `WirePass`, the pass lives here
//! because only `shielded.rs` decides who holds one, and the guard is a test in `ehr`: it fails
//! if any file other than `texts.rs` names `TextPass`.

/// Proof that the caller is the `ehr` service's text sender.
pub struct TextPass {
    _only_texts: (),
}

impl TextPass {
    /// For `ehr/src/texts.rs` only (a test in `ehr` enforces it).
    pub fn issue() -> Self {
        Self { _only_texts: () }
    }
}
