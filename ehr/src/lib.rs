//! DeetsHealth's `ehr` service (docs/architecture/EHR-SERVICE.md).
//!
//! The agent (Python) asks; this service decides. It holds each call's session behind an
//! opaque handle: who the caller has proved to be, the slots offered, the bookings made. Every
//! chart and booking call takes only the handle, so the model can never name a patient
//! (SECURITY.md §4.9).
//!
//! - [`proto`]: the generated gRPC code, with the `(phi)` and `(secret)` fields redacted from
//!   every debug print.
//! - `edge`: the only place wire types turn into domain types, and back (§5).
//! - [`session`]: the sessions and their stages.
//! - [`scheduler`]: the `Scheduler` trait and the in-memory fake (Phase 1).
//! - [`service`]: the gRPC service.
//!
//! Build state (BUILD-ORDER.md, Phase 1): the contract, the sessions, the fake scheduler ✓ ·
//! verification (waits on the identification fork, §8) · the transport (waits on its fork, §6).

mod edge;
pub mod proto;
pub mod scheduler;
pub mod service;
pub mod session;
pub mod transport;
pub mod verify;
