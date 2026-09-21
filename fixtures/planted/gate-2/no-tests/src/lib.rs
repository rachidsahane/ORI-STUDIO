//! A defective library that nothing tests (ORI-T-0014, AICD §14, AICD §39).
//!
//! The implementation is byte for byte the defective one from
//! `failing-unit/src/lib.rs`. What is missing is every test of it: no
//! `#[cfg(test)]` module, no `tests/` directory, no documentation example.
//!
//! `cargo test --workspace --locked` compiles this, prints "running 0 tests"
//! for the library's unit test binary and again for its doc tests, prints
//! "test result: ok" under both, and exits 0. Gate 2 reports success, and what
//! it has established is that this package compiles under `cfg(test)`.
//!
//! This is the first of the three states AICD §39 calls "present but reporting
//! nothing", reproduced in the tool gate 2 is built on rather than in a
//! workflow file. `scripts/gates.sh` already says this in its own words for
//! the repository's workspace: "A clean run of no tests is evidence that the
//! workspace compiles under cfg(test), nothing more."

/// How much of a budget is left after `spent` against a `limit`.
///
/// PLANTED DEFECT: the spend is ignored. See `clean/src/lib.rs` for the
/// correct form. Nothing in this package asserts anything about it.
pub fn remaining_budget(_spent: u32, limit: u32) -> u32 {
    limit
}
