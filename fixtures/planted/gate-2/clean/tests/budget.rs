//! The integration level of the control input (ORI-T-0014).
//!
//! `cargo test` builds every file under `tests/` as its own binary, linked
//! against the library as an external crate. This file is here so that the
//! clean input exercises the same three target kinds the failing inputs do.

use gate_2_clean::remaining_budget;

#[test]
fn a_full_budget_is_the_limit() {
    assert_eq!(remaining_budget(0, 25), 25);
}

#[test]
fn an_exhausted_budget_is_zero() {
    assert_eq!(remaining_budget(25, 25), 0);
}
