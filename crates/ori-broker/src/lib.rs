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
//! [`identity`] is this ticket's declared scope so far: `AgentIdentity`
//! (`spec/DATA_MODEL.md` section 2) and its creation path. `keychain.rs` (the
//! `Keychain` trait, its real and fake implementations, and the `Secret`
//! newtype) is the rest of this same ticket's scope and follows in the next
//! commit. `issuance.rs` (`CredentialIssuance`, full scoping and revocation on
//! session end) is ORI-T-0027; `family.rs` (cross-model refusal) is
//! ORI-T-0028; `forbidden.rs` (the forbidden-action test harness) is
//! ORI-T-0029. None of the three exist yet, and nothing here reaches into
//! them.

pub mod identity;
