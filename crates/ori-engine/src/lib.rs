//! Composition root of the engine: AICD §6, AICD §26.
//!
//! Owns `Engine::open(product)`, `Engine::call(method, params)` and
//! `Engine::subscribe()` (`spec/LLD.md` section 2); it wires the crates and
//! exposes `Engine`.
//!
//! This crate is not listed in the `spec/ARCHITECTURE.md` section 2 component
//! table, so its sections are derived from what `spec/LLD.md` section 2 says it
//! owns: as the composition root it assembles the components that implement the
//! five layers of AICD §6, and `Engine::open(product)` is the one-instance-per-
//! product rule of AICD §26 ("one fleet instance per product", never sharing
//! credentials or context).
