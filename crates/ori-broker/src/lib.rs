//! The credential broker: AICD §17, AICD §27.
//!
//! Owns `Identity`, `Issuance`, `Keychain` and `ForbiddenActionTest`
//! (`spec/LLD.md` section 2). The broker issues a credential; `ori-runtime`
//! injects it at spawn and holds it no longer than the session.
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md` section 2).
//!
//! # What is here and what is not (ORI-T-0026)
//!
//! [`identity`], [`keychain`] and [`family`] are declared scope so far:
//! `AgentIdentity` (`spec/DATA_MODEL.md` section 2), the `Keychain` trait
//! with its real and fake implementations plus the `Secret` newtype
//! (`spec/SECURITY_NOTES.md` "Secrets"), and the cross-model refusal at
//! identity creation (ADR-0001 "Model family", AICD §7, criterion
//! ORI-P1-035). `issuance.rs` (`CredentialIssuance`, full scoping and
//! revocation on session end) is ORI-T-0027; `forbidden.rs` (the
//! forbidden-action test harness) is ORI-T-0029. Neither exists yet, and
//! nothing here reaches into them.

pub mod family;
pub mod identity;
pub mod issuance;
pub mod keychain;
