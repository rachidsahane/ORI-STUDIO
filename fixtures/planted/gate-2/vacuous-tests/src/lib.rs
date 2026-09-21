//! A defective library whose tests assert nothing (ORI-T-0014, AICD §14).
//!
//! The implementation is the defective one from `failing-unit/src/lib.rs`.
//! The three tests below call it and throw the answer away. They run, they
//! pass, cargo prints "3 passed; 0 failed", and the defect is untouched.
//!
//! This is the second state gate 2 reports as success, and it is the one that
//! survives a reader glancing at a test count.

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

    /// Calls the function under test and discards what it returned. This is
    /// the commonest vacuous shape: it exercises the code, so it appears in a
    /// line coverage report, and it can only fail by panicking.
    #[test]
    fn remaining_budget_runs() {
        let _ = remaining_budget(3, 10);
    }

    /// Asserts a tautology. It cannot fail whatever the library does.
    #[test]
    fn remaining_budget_is_a_number() {
        let _ = remaining_budget(12, 10);
        assert!(true);
    }

    /// Asserts a property of its own arguments rather than of the answer.
    #[test]
    fn remaining_budget_accepts_a_limit() {
        let limit = 10u32;
        let _ = remaining_budget(3, limit);
        assert_eq!(limit, 10);
    }
}
