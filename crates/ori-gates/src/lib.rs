//! Verification gates: AICD §14, AICD §15.
//!
//! Owns `GateDef`, the `Runner` trait, the built-in runners (coverage matrix,
//! modified tests, significance, citation, liveness) and `Prover`
//! (`spec/LLD.md` section 2).
//!
//! Must not: report a gate installed without a proof (`spec/LLD.md` section 2).

/// The methodology's machine-readable section index and its generator (AICD §39).
pub mod sections;

/// The index of this repository's own specification and the reader that
/// resolves prose references to it (AICD §39).
pub mod spec_refs;
