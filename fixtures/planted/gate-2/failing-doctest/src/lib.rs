//! A library whose documentation test fails (ORI-T-0014, AICD §14).
//!
//! The same planted defect again, with the test that catches it written as a
//! documentation example. `cargo test` compiles and runs those from the doc
//! comments of the library target and reports them under their own "Doc-tests"
//! heading, after the unit and integration binaries have finished. There is no
//! unit test and no tests/ directory here, so the doc test is the only thing
//! in this package that can fail.

/// How much of a budget is left after `spent` against a `limit`.
///
/// PLANTED DEFECT: the spend is ignored, and the example below asserts the
/// answer `clean/src/lib.rs` gives.
///
/// ```
/// use gate_2_failing_doctest::remaining_budget;
/// assert_eq!(remaining_budget(3, 10), 7);
/// ```
pub fn remaining_budget(_spent: u32, limit: u32) -> u32 {
    limit
}
