//! The planted formatting defect for gate 1 (ORI-T-0013, AICD §14).
//!
//! Identical to `../../clean/src/lib.rs` except for the line that declares
//! `sum`.
//!
//! What is wrong with it: `pub fn sum(a:i32,b:i32)->i32{a+b}` is missing the
//! space after each parameter colon, the spaces around `->`, and the line
//! breaks around the body, so `rustfmt` rewrites it, `cargo fmt --check` exits
//! non-zero and prints the diff it would have applied. It is valid Rust, it
//! compiles, and no lint fires on it, which is the point: only the formatting
//! half of gate 1 can see this defect, so a failure here is attributable to
//! that half and to nothing else.
//!
//! There is deliberately no `#[rustfmt::skip]` anywhere in this file. That
//! attribute would make `rustfmt` leave the line alone, gate 1 would report a
//! pass, and the proof would record that a gate caught a defect that was never
//! presented to it.

/// Adds two integers. THE PLANTED DEFECT IS THE NEXT LINE: it is unformatted.
pub fn sum(a:i32,b:i32)->i32{a+b}

/// Reports whether the slice holds nothing, written the way
/// `clippy::len_zero` wants it written. See `../../clean/src/lib.rs` for why
/// this function is not named `is_empty`.
pub fn holds_nothing(values: &[i32]) -> bool {
    values.is_empty()
}
