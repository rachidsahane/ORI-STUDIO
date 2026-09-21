//! Guided procedures: AICD §23, AICD §24.
//!
//! Owns `NewProductFlow` (G0..G7), `MigrationFlow` (M0..M5), `DocumentGenerator`,
//! `Readiness` and `PhaseControl` (`spec/LLD.md` section 2).
//!
//! Must not: skip an approval (`spec/LLD.md` section 2).
