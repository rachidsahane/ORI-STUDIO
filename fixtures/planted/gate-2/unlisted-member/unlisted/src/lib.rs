//! The package the workspace does not list (ORI-T-0014, AICD §39).
//!
//! It carries the planted defect from `failing-unit/src/lib.rs` and the unit
//! test that catches it. Both are invisible to `cargo test --workspace` run in
//! the parent directory, because `members` there names only `listed`.

/// How much of a budget is left after `spent` against a `limit`.
///
/// PLANTED DEFECT: the spend is ignored. See `../../clean/src/lib.rs` for the
/// correct form.
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
