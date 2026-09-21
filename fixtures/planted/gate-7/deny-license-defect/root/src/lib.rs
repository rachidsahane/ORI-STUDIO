//! The root crate of a planted cargo-deny fixture (ORI-T-0016, gate 7).
//! Nothing here is ever built: cargo-deny reads the manifests, not the code.

/// Returns the one number this crate exists to return.
pub fn answer() -> u32 {
    1
}
