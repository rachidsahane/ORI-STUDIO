# R31. A ticket identifier is allocated once, and a claim is not closed until its release is written

| | |
|---|---|
| Ruling | R31 |
| Tier | 1 |
| Made by | The lead, at ORI-T-0092 review |
| Follows | [[R30]], which fixed the container. This fixes the bookkeeping inside it |

## What went wrong, twice, in one ticket

ORI-T-0092's agent reported two defects in `ops/lock-table.md` that no gate could have caught, because no gate reads `ops/` ([[CR-007]]).

**The identifier was already taken.** `ops/lock-table.md` penciled `ORI-T-0092` into a queue row for the `CLAUDE.md` pair byte-comparison test. The lead then allocated the same identifier to a documentation ticket against four `spec/` documents. Two different pieces of work now share one identifier, in a project whose merge history, commit trailers and coverage matrix are all read by identifier.

**No claim has ever been released.** The table's own header says "Every claim and release is recorded here, in order. AICD §12". Claims 16 through 23 have no release recorded, and claims 1 through 10 have none either. Claim 21 holds `spec/TESTING.md` and `spec/README.md`; ORI-T-0089 merged, and the table still says they are held. ORI-T-0092 touched both.

Under the mandated procedure `aicd_plan_submit` would have answered `E_SCOPE_LOCKED` on two of four files. Nothing checked, because that tool does not exist ([[E-0005]]). **The agent proceeded on its own reading of a stale record and said so, which is the only reason the lead knows.** A queue row in the same table literally waits on "claim 21 releasing", so the table contained both the stale hold and a dependency on its release.

## Why it is structural and not carelessness

R30 recorded that the previous diagnosis of this family, the lead forgetting to write things down, was wrong. The same correction applies here.

A claim is written when a ticket is dispatched, which is a moment the lead is already stopped and thinking about that ticket. A release falls due when a pull request merges, which is a moment **the operator acts and the lead is doing something else**. The two halves of the same record have different authors and different triggers, and only one of them was ever going to get written. Twenty-three claims and four partial releases is what that looks like.

The same asymmetry explains the identifier: a queue row is written while planning a future batch, and an allocation is made while dispatching a ticket now. Nothing connected the two moments.

## The ruling

1. **A ticket identifier is allocated once and never reused.** The allocation happens in `ops/lock-table.md` and nowhere else, and a queue row that names an identifier **is** an allocation. Penciling a ticket into a queue reserves its number. The next free identifier is the highest allocated plus one, and both a claim and a queue row count as allocated.
2. **A claim is not closed until its release is written**, with the merge that released it. A claim with no release is held, whatever the tree looks like, and the next ticket that wants those modules must either wait or record why the hold is stale before proceeding.
3. **The lead writes the release when it verifies the merge**, which is the moment it already returns to the repository and checks `main`. That is the only recurring moment where the lead is looking at a merge and at the table at once.
4. **The releases owed as of this ruling are written below, in one pass**, from the merge history rather than from memory. Nothing is backdated and nothing is rewritten; the table gains the column it always said it had.

## The duplicate identifier, resolved

`ORI-T-0092` stays with the documentation ticket, which holds the only recorded **claim** on it (claim 24) and has three commits and a pull request carrying it in their trailers. The queue row was a plan and not a claim, and it has no branch, no commit and no trailer.

The `CLAUDE.md` pair byte-comparison test is reallocated to **ORI-T-0095**.

This breaks clause 1's first-written-wins reading, on purpose, because clause 1 did not exist when the collision happened and the cost is asymmetric: renumbering a queue row costs one line, and renumbering merged trailers costs a history rewrite that CLAUDE.md rule 2 forbids the lead from performing. **From this ruling forward, first allocated wins and the cheap side loses.**

## Releases owed, written from the merge history

| Claim | Ticket | Modules | Released |
|---|---|---|---|
| 1 | ORI-T-0006 | `ops/methodology-anchor-defects.md` | merged, pull request 3 |
| 2, 2b | ORI-T-0007 | `spec/agents/CLAUDE.md`, `CLAUDE.md`, `spec/agents/coder.md`, `spec/README.md` | merged, pull request 4 |
| 3 | ORI-T-0008 | `.claude/agents/**` | merged, pull request 6 |
| 4 | ORI-T-0009 | `templates/**` | merged, pull request 5 |
| 5 | ORI-T-0011 | `spec/adr/ADR-0002-single-operator.md` | merged, pull request 2 |
| 7 | ORI-T-0003 | `rust-toolchain.toml`, `.gitignore`, `scripts/setup-dev.sh` | merged, pull request 8 |
| 8 | ORI-T-0004 | `scripts/gates.sh` | merged, pull request 9 |
| 9 | ORI-T-0005 | `methodology/sections.json`, `crates/ori-gates/src/sections.rs`, the `mod` line | merged, pull request 10 |
| 10 | ORI-T-0002 | `apps/desktop/**`, root `Cargo.toml`, `Cargo.lock` | merged, pull request 11 |
| 11 | ORI-T-0013 | `ci.yml` gate 1, `fixtures/planted/gate-1/**`, `ops/gates/gate-1.md` | already recorded released |
| 12 | ORI-T-0014 | recorded as released without ever having been entered | see the note at claim 18's finding; the release stands, the claim was never written |
| 13 | ORI-T-0016 | `ci.yml` gate 7, `fixtures/planted/gate-7/**`, `deny.toml`, `scripts/secret-scan.sh`, `scripts/gates.sh` | merged, pull request 21 |
| 14 | ORI-T-0084 | `fixtures/planted/gate-1/workflow-samples/**`, `fixtures/planted/gate-2/README.md` | merged, pull request 22 |
| 15 | ORI-T-0018 | `crates/ori-cli/src/main.rs` and lead records | already recorded released |
| 16 | ORI-T-0019 | `crates/ori-core/src/types.rs`, `error.rs`, `lib.rs` `mod` lines | merged, pull request 27 |
| 17 | ORI-T-0085 | `crates/ori-gates/src/sections.rs` | merged, pull request 28 |
| 18 | ORI-T-0086 | `crates/ori-core/src/error.rs` | merged, pull request 31 |
| 19 | ORI-T-0087 | `spec/agents/CLAUDE.md`, `CLAUDE.md` | merged, pull request 32 |
| 20 | ORI-T-0088 | never granted; tier 2, still with the operator | **held** |
| 21 | ORI-T-0089 | `spec/TESTING.md`, `spec/README.md` | merged, pull request 34 |
| 22 | ORI-T-0090 | `crates/ori-gates/src/sections.rs`, `scripts/gates.sh`, `fixtures/planted/gate-13/README.md` | merged, pull request 36 |
| 23 | ORI-T-0091 | `crates/ori-gates/src/sections.rs` | merged, pull request 38 |
| 24 | ORI-T-0092 | `spec/CONVENTIONS.md`, `spec/LLD.md`, `spec/TESTING.md`, `spec/README.md` | **held**, pull request 41 open |
| 25 | ORI-T-0093 | `crates/ori-core/src/error.rs`, `types.rs` | **held**, in flight |
| 26 | ORI-T-0094 | `README.md`, `crates/ori-gates/src/sections.rs` | **held**, in flight |

Claim 6 was never entered. It is recorded here as an allocation gap rather than invented, because the table is a log.

## What this does not fix

It does not give `ops/` a gate, and this ruling is itself unchecked by anything. A release row that is wrong, or an identifier allocated twice after today, would be found the same way these two were: by an agent reading the table and saying so. CR-005 and CR-007 both propose the mechanical version to the methodology and it is still owed.
