//! Domain types, state machines and invariants: AICD §7, AICD §9, AICD §11, AICD §17.
//!
//! Owns `Product`, `Ticket`, `Document`, the state machines expressed as pure
//! functions (`Ticket::apply(event) -> Result<Ticket>`), `Category`, `Tier`,
//! `Role`, `Scope`, and the error enum whose refusals carry a methodology-section
//! reason (`spec/LLD.md` section 2).
//!
//! This crate is not listed in the `spec/ARCHITECTURE.md` section 2 component
//! table, so its sections are derived from what `spec/LLD.md` section 2 says it
//! owns: `Role` and the separation of duties from AICD §7; `Document` and the
//! specification system from AICD §9; `Ticket` and `Category` from AICD §11;
//! `Tier` and `Scope` from the autonomy tiers and permission model in AICD §17.
//!
//! Must not: do IO, or import any other workspace crate (`spec/LLD.md` section 2).

pub mod visibility_probe;
