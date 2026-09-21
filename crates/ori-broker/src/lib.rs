//! The credential broker: AICD §17, AICD §27.
//!
//! Owns `Identity`, `Issuance`, `Keychain` and `ForbiddenActionTest`
//! (`spec/LLD.md` section 2). The broker issues a credential; `ori-runtime`
//! injects it at spawn and holds it no longer than the session.
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md` section 2).
