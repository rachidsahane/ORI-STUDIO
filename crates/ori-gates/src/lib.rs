//! Verification gates: AICD §14, AICD §15.
//!
//! Owns `GateDef`, the `Runner` trait, the built-in runners (coverage matrix,
//! modified tests, significance, citation, liveness) and `Prover`
//! (`spec/LLD.md` section 2).
//!
//! Must not: report a gate installed without a proof (`spec/LLD.md` section 2).

/// The coverage matrix: criteria to tests, gate 4 of `spec/CI_CD.md` section 1
/// (AICD §14).
pub mod coverage;

/// The `Gate` entity, [`crate::gate::GateDef`], [`crate::gate::GateProof`] and
/// the two-value [`crate::gate::DefinitionState`] a definition alone can
/// honestly report (`spec/DATA_MODEL.md` section 2).
pub mod gate;

/// The methodology's machine-readable section index and its generator (AICD §39).
/// The detector that makes CLAUDE.md's never-modify-a-test rule real (AICD §14).
pub mod modified_tests;

/// The [`crate::runner::Runner`] trait and [`crate::runner::Registry`], which
/// pairs a [`crate::gate::GateDef`] with an optional runner and is the only
/// place [`crate::runner::GateState::Installed`] is produced
/// (`spec/LLD.md` section 2).
pub mod runner;

pub mod sections;

/// The index of this repository's own specification and the reader that
/// resolves prose references to it (AICD §39).
/// Gate 12: the significance labeler (AICD §15).
pub mod significance;

pub mod spec_refs;
