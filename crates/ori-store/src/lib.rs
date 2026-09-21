//! Durable per-product state: AICD §8, the memory layer 3 store.
//!
//! Owns `EventLog` (append, hash chain, read range), projections, migrations and
//! `ProductDb` open/lock (`spec/LLD.md` section 2). The event log is append-only
//! and hash-chained: no code path updates or deletes an event, and every
//! projection is derived from the log.
//!
//! Must not: contain business rules (`spec/LLD.md` section 2).
