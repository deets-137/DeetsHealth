//! The pass for the `ehr` service's wire edge (docs/architecture/EHR-SERVICE.md §5).
//!
//! A reply to the agent that carries a protected value must reveal it, because a gRPC message
//! holds plain strings. That happens in one function file, `ehr/src/edge.rs`, and nowhere
//! else. The pass lives here, in core, because only `shielded.rs` decides who holds one.
//! `ehr` cannot be a module of core, so the guard is a test in `ehr`: it fails if any file
//! other than `edge.rs` names `WirePass`.

/// Proof that the caller is the wire edge of the `ehr` service.
pub struct WirePass {
    _only_edge: (),
}

impl WirePass {
    /// For `ehr/src/edge.rs` only (a test in `ehr` enforces it).
    pub fn issue() -> Self {
        Self { _only_edge: () }
    }
}
