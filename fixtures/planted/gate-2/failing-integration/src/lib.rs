//! A library whose integration test fails (ORI-T-0014, AICD §14).
//!
//! The same planted defect as `failing-unit/`, with the test that catches it
//! moved out to `tests/budget.rs`, which cargo builds as a separate binary.
//! There is no unit test and no documentation example here, so the only thing
//! that can fail in this package is the integration binary.

/// How much of a budget is left after `spent` against a `limit`.
///
/// PLANTED DEFECT: the spend is ignored. See `clean/src/lib.rs` for the
/// correct form.
pub fn remaining_budget(_spent: u32, limit: u32) -> u32 {
    limit
}
