//! Migration flows (M0 through M5): AICD §24.
//!
//! Submodules for environment scanning, M0 inventory, M2 sequence, and
//! divergence registration are added in later batch-12 slices. This module
//! exists so `lib.rs` can declare the tree before those files land.
