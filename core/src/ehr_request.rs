//! The pass for the `ehr` service's requests to an EHR: the caller's name and date of birth in a
//! patient search, and the patient's id in a chart read (SECURITY.md §4.9; EPIC.md §5.2).
//!
//! A FHIR search must carry these values in plain form, encoded into the URL. That happens in
//! one file, `ehr/src/epic_ehr.rs` (each EHR adapter, as it is added), and nowhere else. As with
//! `TextPass`, the pass lives here because only `shielded.rs` decides who holds one, and the
//! guard is a test in `ehr`: it fails if any other file names `EhrPass`.

/// Proof that the caller is one of the `ehr` service's EHR adapters.
pub struct EhrPass {
    _only_adapters: (),
}

impl EhrPass {
    /// For `ehr/src/epic_ehr.rs` only (a test in `ehr` enforces it).
    pub fn issue() -> Self {
        Self { _only_adapters: () }
    }
}
