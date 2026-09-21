//! A library whose unit test fails (ORI-T-0014, AICD §14).
//!
//! The planted defect is in the implementation and not in the test: the test
//! asserts what `clean/` asserts, and the implementation ignores the spend.
//! That is the shape of a real regression, and it is the input gate 2 must
//! catch. `cargo test --workspace --locked` exits 101 here.

/// How much of a budget is left after `spent` against a `limit`.
///
/// PLANTED DEFECT: the spend is ignored, so this always reports the whole
/// limit as available. `clean/src/lib.rs` carries the correct form,
/// `limit.saturating_sub(spent)`.
///
/// No documentation example here, deliberately. An example asserting the
/// correct answer would fail as a doc test as well, and this package's failure
/// would then be attributable to two mechanisms rather than one.
pub fn remaining_budget(_spent: u32, limit: u32) -> u32 {
    limit
}

#[cfg(test)]
mod tests {
    use super::remaining_budget;

    #[test]
    fn remaining_budget_subtracts_the_spend() {
        assert_eq!(remaining_budget(3, 10), 7);
    }
}
