//! The `ori` command-line client: AICD §28.
//!
//! `spec/LLD.md` section 1 gives this crate the `ori` binary. `ori-cli` is not
//! listed in the `spec/ARCHITECTURE.md` section 2 component table, so its section
//! is derived: `spec/ARCHITECTURE.md` section 9 states that the UI has no
//! capability the CLI lacks and that both go through `ori-rpc`, whose component
//! row in `spec/ARCHITECTURE.md` section 2 cites AICD §28. The CLI therefore
//! exposes the supervision surface of AICD §28 and adds nothing of its own.

/// Entry point of the `ori` binary.
fn main() {}
