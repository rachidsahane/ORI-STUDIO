//! The control input for the gate 2 proof (ORI-T-0014, AICD §14).
//!
//! Every other package under `fixtures/planted/gate-2/` is this library with
//! one thing changed: the implementation broken, or the tests weakened in one
//! of the three ways `cargo test` reports as success. Keeping the subject the
//! same in all of them is what makes a verdict attributable to the change.
//!
//! Gate 2 must report a pass here, and that half of the proof is not
//! decoration. AICD §14 names the defect class the planted-defect rule exists
//! for, and its first example is "a checker that exits successfully on every
//! input". The mirror of it, a checker that exits non-zero on every input,
//! catches every planted defect and blocks every clean tree. Only a run that
//! shows both answers, from one comparator, on one code path, has shown that
//! the gate can tell the two states apart.

/// How much of a budget is left after `spent` against a `limit`.
///
/// A spend at or over the limit leaves nothing, and never wraps.
///
/// The subject is a stand-in, chosen so that a wrong answer is obvious to a
/// reader: AICD §12 gives every ticket a budget of attempts, wall clock and
/// tokens, and CLAUDE.md rule 8 restates it as "budgets are real", so "how
/// much is left" is a quantity this project has an opinion about. AICD §12
/// says nothing about how to compute it, and this function does not claim
/// otherwise; the defective copies in the sibling packages differ from this
/// one by returning the limit whatever has been spent.
///
/// ```
/// use gate_2_clean::remaining_budget;
/// assert_eq!(remaining_budget(3, 10), 7);
/// assert_eq!(remaining_budget(10, 10), 0);
/// assert_eq!(remaining_budget(12, 10), 0);
/// ```
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

    #[test]
    fn remaining_budget_saturates_at_zero() {
        assert_eq!(remaining_budget(12, 10), 0);
    }
}
