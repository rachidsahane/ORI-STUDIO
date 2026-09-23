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
//! [`identity`] and [`keychain`] are this ticket's declared scope:
//! `AgentIdentity` (`spec/DATA_MODEL.md` section 2), and the `Keychain` trait
//! with its real and fake implementations plus the `Secret` newtype
//! (`spec/SECURITY_NOTES.md` "Secrets"). `issuance.rs` (`CredentialIssuance`,
//! full scoping and revocation on session end) is ORI-T-0027; `family.rs`
//! (cross-model refusal) is ORI-T-0028; `forbidden.rs` (the forbidden-action
//! test harness) is ORI-T-0029. None of the three exist yet, and nothing here
//! reaches into them.

pub mod identity;
pub mod keychain;
