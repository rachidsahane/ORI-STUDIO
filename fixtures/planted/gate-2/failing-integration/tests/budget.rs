//! The failing integration test (ORI-T-0014).
//!
//! It asserts what `clean/tests/budget.rs` asserts. The library it is linked
//! against ignores the spend, so this binary reports a failure and
//! `cargo test --workspace --locked` exits 101.

use gate_2_failing_integration::remaining_budget;

#[test]
fn an_exhausted_budget_is_zero() {
    assert_eq!(remaining_budget(25, 25), 0);
}
