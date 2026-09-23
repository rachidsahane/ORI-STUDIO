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
//! `ModelFamily` (`types.rs`) is not in that list either; ORI-T-0108 moves it
//! here from `ori-broker`'s `family.rs` because it is a validated value type
//! with no IO, per the operator's ruling on escalation 6 in
//! `ops/phase-1-backlog.md`: `AgentIdentity` (`ori-broker`) stores it, and a
//! runtime adapter (`ori-runtime`) declares it, so neither crate that needs it
//! can own it without the other depending on it.
//!
//! Must not: do IO, or import any other workspace crate (`spec/LLD.md` section 2).

pub mod document;
pub mod error;
pub mod permission;
pub mod phase;
pub mod ticket;
pub mod types;
