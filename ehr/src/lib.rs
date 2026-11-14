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
//! - [`sandbox`]: the synthetic patients and visits the fakes serve.
//! - [`texts`]: the code by text, through Twilio's Messages API (the fake Twilio in dev).
//! - [`epic`]: the Epic sign-in, SMART Backend Services (EPIC.md §5, E-2).
//! - [`store`]: the call store in Postgres, sealed by [`seal_key`] (DATABASE.md).
//! - [`settings`]: the settings store: each service's settings, versioned, insert-only
//!   (CONSOLE.md §7.3).
//!
//! Build state (BUILD-ORDER.md, Phase 1): the contract, the sessions, the fake scheduler ✓ ·
//! verification (waits on the identification fork, §8) · the transport (waits on its fork, §6).

mod edge;
pub mod epic;
pub mod epic_ehr;
pub mod proto;
pub mod sandbox;
pub mod scheduler;
pub mod seal_key;
pub mod service;
pub mod session;
pub mod settings;
pub mod store;
pub mod texts;
pub mod transport;
pub mod users;
pub mod verify;
pub mod visit_about;
