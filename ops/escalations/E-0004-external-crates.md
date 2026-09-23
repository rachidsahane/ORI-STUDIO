# Escalation E-0004: the external crates phase 1 cannot avoid

| | |
|---|---|
| Trigger | `new_dependency` |
| Raised by | Lead, at batch 2 planning, before any ticket that needs one is started |
| Blocks | ORI-T-0020, ORI-T-0021 and ORI-T-0022 (property tests). ORI-T-0019 is unaffected and starts now |
| State | Answered by the operator in session on 2026-09-22: `proptest`, `rusqlite`, a hashing crate (`sha2` was taken) and `keyring` approved, each pinned to an exact version with `Cargo.lock` committed. `tantivy` and `tree-sitter` were not part of that answer and remain open for batch 6. Recorded here on 2026-09-23, after ORI-T-0026's coder found this line still read "Open" while four crates had been added on the answer |

## Why this is raised at all

CLAUDE.md rule 6 is unqualified: never add a crate without escalating with trigger `new_dependency`. The specification naming a crate does not discharge that, because the rule is about who decides what enters the trust boundary, not about who first wrote the name down. So this is raised before the first line that would need one is written.

`ops/phase-1-backlog.md` recommended bundling these into one decision at batch 2 planning rather than four separate interruptions. That recommendation stands, and this is it.

## The five, and the important difference between them

| Crate | Needed at | Named in the specification | What it is for |
|---|---|---|---|
| `proptest` | batch 2 | **Yes**, `spec/TESTING.md` section 1, by name | Property-based tests over generated event sequences |
| `keyring` | batch 4 | **Yes**, `spec/LLD.md` section 2 (`ori-broker`) and ADR-0001 | The OS keychain, the only place `ori-broker` may persist a secret |
| `tantivy` | batch 6 | **Yes**, `spec/LLD.md` section 2 (`ori-memory`) and section 6 | Full-text index for operational memory |
| `tree-sitter` | batch 6 | **Yes**, `spec/PROJECT_BRIEF.md` principle on embedded storage, `spec/PRD.md` K-03, `spec/LLD.md` section 2 | The code map |
| `rusqlite` | batch 3 | **No. Named nowhere in `spec/`** | The Rust binding to SQLite |

**Four of the five are already decisions the specification made.** Approving them is confirming a choice, and the only real question is the terms on which they enter.

**`rusqlite` is not one of them.** `spec/PROJECT_BRIEF.md` and ADR-0001 choose *SQLite*; neither names a Rust binding, and the two serious candidates differ in a way that matters here. This is a real open choice and it arrives at batch 3, not batch 2, so it does not need answering today.

## What the lead recommends

**Approve the four the specification names, on these terms**, which are the terms ORI-T-0016 already established for `cargo-audit` and `cargo-deny`:

1. Pinned to an exact version in `Cargo.toml`, with `Cargo.lock` committed, which it already is.
2. Covered by gate 7's `cargo-deny` licence and bans checks, which run today and are proven on planted defects.
3. `proptest` enters as a **dev-dependency only**. It is a test-time tool and must never appear in a shipped binary's dependency graph. That is mechanically checkable and the lead will check it on the first pull request that adds it.

**Defer `rusqlite`** to its own escalation at batch 3 planning, where the choice can be stated against what `ori-store` actually needs, rather than answered early and cheaply here.

## What this does not ask for

It does not ask to relax CLAUDE.md rule 6. Every crate after these still escalates. What it asks is that the four the specification already chose be approved once, together, rather than interrupting batches 2, 4 and 6 to re-read a decision that is already written down.

## If the answer is no, or is delayed

ORI-T-0019 (domain types and the error enum) needs none of them and starts now. ORI-T-0020, ORI-T-0021 and ORI-T-0022 are the ones that stop, and they stop on the property tests specifically: their state machines could be written and unit-tested without `proptest`, but `spec/ROADMAP.md` phase 1 item 2 says "state machines with property tests" and shipping them without would be the ticket deciding to do less than the specification asks. The lead will not do that silently.
