//! The control input for the gate 1 proof (ORI-T-0013, AICD §14).
//!
//! This file is what `../../fmt-defect/src/lib.rs` and
//! `../../clippy-defect/src/lib.rs` look like when they are correct. Each of
//! those two differs from this file by exactly one item, so a failure reported
//! against one of them cannot be blamed on anything but its planted defect.
//!
//! Gate 1 must report a pass on both halves here, and that half of the proof
//! is not decoration. AICD §14 names the defect class the planted-defect rule
//! exists for, and its first example is "a checker that exits successfully on
//! every input". The mirror of it, a checker that exits non-zero on every
//! input, catches every planted defect and blocks every clean tree. Only a run
//! that shows both answers, from one comparator, on one code path, has shown
//! that the gate can tell the two states apart.

/// Adds two integers, formatted the way `rustfmt` formats it.
///
/// `fmt-defect` carries this same function with the whitespace removed.
pub fn sum(a: i32, b: i32) -> i32 {
    a + b
}

/// Reports whether the slice holds nothing, written the way
/// `clippy::len_zero` wants it written.
///
/// `clippy-defect` carries this same function written as `values.len() == 0`.
///
/// NOT NAMED `is_empty`, AND THE NAME IS LOAD BEARING. `clippy::len_zero`
/// suppresses itself inside an item named `is_empty`, because there its own
/// suggestion would be the recursive call. Named `is_empty`, the defect
/// package below compiles without a single lint and gate 1 reports a pass on
/// a tree that was planted to fail. That is not a hypothesis: it is what the
/// first version of this fixture did, and ops/gates/gate-1.md records the run.
pub fn holds_nothing(values: &[i32]) -> bool {
    values.is_empty()
}
