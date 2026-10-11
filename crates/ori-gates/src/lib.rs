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

/// The liveness definition: whether an expected workflow or agent run happened
/// inside its window, and when a missing run makes an installed gate inert
/// (AICD §14, `spec/PRD.md` section 4.11, O-06).
pub mod liveness;

/// The methodology's machine-readable section index and its generator (AICD §39).
/// The detector that makes CLAUDE.md's never-modify-a-test rule real (AICD §14).
pub mod modified_tests;

/// The prover: a gate is proven only by a clean pass and a failure on its
/// planted defect, and every other pair of outcomes is a refusal (AICD §14).
pub mod prover;

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
