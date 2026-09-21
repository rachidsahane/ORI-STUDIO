//! Integration adapter slots: AICD §16.
//!
//! Owns the traits in `spec/API_SPEC.md` section 4, the `reference/` adapters
//! (one per slot) and `Webhooks` (`spec/LLD.md` section 2).
//!
//! Must not: read the store (`spec/LLD.md` section 2). `spec/API_SPEC.md`
//! section 4 states that adapters have no access to the store, so this crate
//! depends on `ori-core` only, per the lead's ruling R8 on `spec/LLD.md`
//! section 2's graph.
