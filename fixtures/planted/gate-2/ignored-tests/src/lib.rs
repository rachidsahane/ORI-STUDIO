//! A defective library whose tests are all ignored (ORI-T-0014, AICD §14).
//!
//! The implementation is the defective one from `failing-unit/src/lib.rs` and
//! the three tests below are the ones that catch it. Every one carries
//! `#[ignore]`, so `cargo test` compiles them, lists them as ignored, and runs
//! none of them: "0 passed; 0 failed; 3 ignored", "test result: ok", exit 0.
//!
//! This is the third state gate 2 reports as success and the cheapest of the
//! three to produce from a working suite: one attribute per test, which leaves
//! the test bodies in the diff for a reviewer to read and approve.
//!
//! `--include-ignored` would run them and `cargo test` would then fail. Gate 2
//! as spec/CI_CD.md section 1 item 2 defines it does not pass that flag, and
//! this fixture does not add it: the fixture's job is to record what the gate
//! does, not to repair it.

/// How much of a budget is left after `spent` against a `limit`.
///
/// PLANTED DEFECT: the spend is ignored. See `clean/src/lib.rs` for the
/// correct form.
pub fn remaining_budget(_spent: u32, limit: u32) -> u32 {
    limit
}

#[cfg(test)]
mod tests {
    use super::remaining_budget;

    #[test]
    #[ignore = "planted for the gate 2 proof: this test catches the defect and never runs"]
    fn remaining_budget_subtracts_the_spend() {
        assert_eq!(remaining_budget(3, 10), 7);
    }

    #[test]
    #[ignore = "planted for the gate 2 proof: this test catches the defect and never runs"]
    fn remaining_budget_saturates_at_zero() {
        assert_eq!(remaining_budget(12, 10), 0);
    }

    #[test]
    #[ignore = "planted for the gate 2 proof: this test catches the defect and never runs"]
    fn an_exhausted_budget_is_zero() {
        assert_eq!(remaining_budget(25, 25), 0);
    }
}
