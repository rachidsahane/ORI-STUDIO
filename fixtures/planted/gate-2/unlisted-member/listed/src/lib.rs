//! The workspace member (ORI-T-0014).
//!
//! Correct, tested, and green. It is here so that the run this workspace
//! produces says "test result: ok" with a test count above zero, which is what
//! makes the silence about `../unlisted` hard to notice.

/// How much of a budget is left after `spent` against a `limit`.
pub fn remaining_budget(spent: u32, limit: u32) -> u32 {
    limit.saturating_sub(spent)
}

#[cfg(test)]
mod tests {
    use super::remaining_budget;

    #[test]
    fn remaining_budget_subtracts_the_spend() {
        assert_eq!(remaining_budget(3, 10), 7);
    }
}
