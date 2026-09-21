//! The `ori` command-line client: AICD §28.
//!
//! `spec/LLD.md` section 1 gives this crate the `ori` binary; `spec/ROADMAP.md`
//! phase 1 gives it commands mirroring the Client API of `spec/API_SPEC.md`
//! section 1. Neither `spec/ARCHITECTURE.md` section 2's component table nor
//! `spec/LLD.md` section 2's ownership table lists it, so its section is derived
//! from what the crate does.
//!
//! AICD §28 is the methodology's only account of the human supervision surface,
//! and this crate is that surface: `spec/ARCHITECTURE.md` section 9 makes the UI
//! and the CLI capability-equal, and `spec/ROADMAP.md` phase 1 ships the CLI with
//! no UI at all. The section therefore binds here in both directions. What it
//! shows each human function, `ori` must be able to print; what it deliberately
//! withholds (agent working memory and intermediate reasoning, raw production
//! telemetry, vanity metrics) `ori` must withhold too, because anything the CLI
//! can do, the supervision surface can do.
//!
//! Its opening sentence, "Humans supervise through a dashboard, not through
//! terminals", is not a refusal of this crate: read against the rest of AICD §28
//! it contrasts a designed surface with watching an agent think, which that
//! section hides from every surface, and it does not reserve supervision to one
//! renderer. The citation is kept on that reading, settling the `ori-cli` half of
//! ruling R14 (`ops/rulings.md`).

/// Entry point of the `ori` binary.
fn main() {}
