//! The planted lint defect for gate 1 (ORI-T-0013, AICD §14).
//!
//! Identical to `../../clean/src/lib.rs` except for the body of
//! `holds_nothing`.
//!
//! What is wrong with it: `values.len() == 0` fires `clippy::len_zero`, which
//! is warn by default, so `-D warnings` turns it into an error and
//! `cargo clippy` exits 101. The lint is a clippy lint and not a rustc lint,
//! which is the point: `cargo build` and `cargo test` compile this file
//! without a word, so a run that catches it caught it with clippy and not with
//! the compiler clippy wraps.
//!
//! This file is `rustfmt` clean on purpose. A file that broke both halves of
//! gate 1 would prove neither: the run would fail and the reason would be
//! ambiguous.
//!
//! The function is not named `is_empty`, and that is load bearing. See
//! `../../clean/src/lib.rs`.

/// Adds two integers, formatted the way `rustfmt` formats it.
pub fn sum(a: i32, b: i32) -> i32 {
    a + b
}

/// Reports whether the slice holds nothing. THE PLANTED DEFECT IS THE LAST
/// LINE OF THE BODY: `len() == 0` where the lint wants `is_empty()`.
pub fn holds_nothing(values: &[i32]) -> bool {
    values.len() == 0
}
