//! The generated code for `proto/deets/ehr/v1/ehr.proto` (EHR-SERVICE.md §3). Never edit the
//! output; edit the contract. `redacted.rs` comes from `build.rs`: the `Debug` of every
//! message with a marked field, and the list of marked fields.

pub mod v1 {
    #![allow(clippy::all)]
    tonic::include_proto!("deets.ehr.v1");
    include!(concat!(env!("OUT_DIR"), "/redacted.rs"));
}
