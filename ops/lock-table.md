# Lock table

Held by the lead until `ori-orchestrator::LockTable` owns it (ORI-T-0050). Every claim and release is recorded here, in order. AICD §12, conflict handling at scale.

| # | Ticket | Modules claimed | Claimed | Released |
|---|---|---|---|---|
| 1 | ORI-T-0006 | `ops/methodology-anchor-defects.md` | batch 1 open | |
| 2 | ORI-T-0007 | `spec/agents/CLAUDE.md`, `CLAUDE.md` | batch 1 open | |
| 2b | ORI-T-0007 | extended: `spec/agents/coder.md`, `spec/README.md` | granted on re-declaration, ruling R7; neither claimed | |
| 3 | ORI-T-0008 | `.claude/agents/**` | batch 1 open | |
| 4 | ORI-T-0009 | `templates/**` | batch 1 open | |
| 5 | ORI-T-0011 | `spec/adr/ADR-0002-single-operator.md` | batch 1 open | |

| 7 | ORI-T-0003 | `rust-toolchain.toml`, `.gitignore`, `scripts/setup-dev.sh` | batch 1 wave 2 | |
| 8 | ORI-T-0004 | `scripts/gates.sh` | batch 1 wave 2 | |
| 9 | ORI-T-0005 | `methodology/sections.json`, `crates/ori-gates/src/sections.rs`, the `mod` line in `crates/ori-gates/src/lib.rs` | batch 1 wave 2 | |
| 10 | ORI-T-0002 | `apps/desktop/**`, root `Cargo.toml` members entry, `Cargo.lock` (ruling R21) | batch 1 wave 2 | |

| 11 | ORI-T-0013 | `.github/workflows/ci.yml`, `fixtures/planted/gate-1/**`, `ops/gates/gate-1.md` | batch 1 wave 4 | |

Claims 11 and 12 released on merge (ORI-T-0013, ORI-T-0014).

Claim 13, ORI-T-0016. Granted `.github/workflows/ci.yml`, `fixtures/planted/gate-7/**`. **Extended mid-ticket** under ruling R28 to `deny.toml`, `scripts/secret-scan.sh` and `scripts/gates.sh`, because a dependency policy the whole repository is judged by and a scanner a gate invokes do not belong in a fixture directory. **The lead granted that extension in a prompt and failed to record it here, so a reviewer correctly reported `scripts/gates.sh` as an unrecorded scope violation.** Same failure as rulings R15 to R24: a grant exists when it is written here, not when the lead states it. Recorded now.

Claim 14, ORI-T-0084. Granted `fixtures/planted/gate-1/workflow-samples/**`. The claim also named `ops/gates/gate-1.md` and `ops/gates/gate-2.md`, which was an error: ruling R25 makes operational records the lead's, so the ticket could not have used them. Released to the lead. **Extended to `fixtures/planted/gate-2/README.md`**, held by the lead rather than the coder, to correct a statement the ticket's own repair made false; nothing else claims it.

ORI-T-0017 remains queued behind claim 13. All four claim `.github/workflows/ci.yml`, so the lock table refuses a parallel start and they run serially, each branched from the previous. Splitting the planted defects into parallel worktrees and serializing only the YAML edit was considered and rejected: it is the lead routing around its own control to save wall clock, in the project whose product is the control.

Claim 15, ORI-T-0018. **Granted nothing, because it was never claimed.** The ticket ran, branched, committed and opened a pull request with no row in this table, and `ops/phase-1-backlog.md` declares its scope as `crates/ori-core/src/lib.rs` while the change landed in `crates/ori-cli/src/main.rs`. Nothing collided, because nothing else was in flight, which is the only reason this cost nothing. Recorded now as held and released together: `crates/ori-cli/src/main.rs`, plus `ops/gates/gate-2.md`, `ops/gates/gate-13.md`, `ops/rulings.md` and this file, all lead records under ruling R25.

This is the fourth occurrence of one failure: a grant or a record that exists in the lead's intent and not in a file. See CR-005.

No two claims overlap, verified mechanically before the claims were granted. ORI-T-0001 through 0005, 0010 and 0012 through 0018 are unclaimed: 0010 is held on a missing precondition (the organizational repository), the rest are held on the Rust toolchain.

---

## Batch 2

Claim 16, **ORI-T-0019**, granted at batch 2 open, **before the coder was launched**, which is the repair CR-005 asks for. Modules: `crates/ori-core/src/types.rs`, `crates/ori-core/src/error.rs`, and the `mod` lines in `crates/ori-core/src/lib.rs`. `crates/ori-core/Cargo.toml` is **not** granted: ORI-T-0019 adds no dependency, and escalation E-0004 is open on the only one batch 2 needs.

Checked against every other claim in this table: nothing else has ever claimed a path under `crates/ori-core/`. ORI-T-0018 touched `crates/ori-cli/src/main.rs` (claim 15) and is a different crate.

ORI-T-0020, ORI-T-0021 and ORI-T-0022 are **not claimed**: all three are held on escalation E-0004. They are recorded here as intended claims so that a later ticket cannot take their paths without seeing them.

| Ticket | Intended modules | Held on |
|---|---|---|
| ORI-T-0020 | `crates/ori-core/src/ticket.rs` | E-0004 |
| ORI-T-0021 | `crates/ori-core/src/document.rs`, `crates/ori-core/src/phase.rs` | E-0004 |
| ORI-T-0022 | `crates/ori-core/src/permission.rs` | E-0004 |

All four are disjoint from each other and from claim 16, except for the `mod` lines in `crates/ori-core/src/lib.rs`, which claim 16 holds. Each later ticket's `mod` line is a one-line edit the lead applies on merge rather than a shared claim, because a file every parallel ticket must edit is not a lock, it is a queue.

Claim 17, **ORI-T-0085**, granted at ORI-T-0019 review, before the coder was launched. Module: `crates/ori-gates/src/sections.rs`. Checked: claim 9 (ORI-T-0005) held that file and released on merge of pull request 10; nothing else has claimed it since. ORI-T-0019 does not touch `crates/ori-gates/`, so the two run in parallel without overlap.

---

## Defect batch, opened after batch 1 closed

Three defects that batch 1's own tickets found in `main`, none of them blocked by escalation E-0004. Every claim below was recorded before its coder was launched, which is what CR-005 asked for and what ORI-T-0019's dispatch failed to do.

Claim 18, **ORI-T-0086**. Module: `crates/ori-core/src/error.rs`. Its module doc says "What has no mechanical check today is the pair of constants below against the index itself, because no code may read both." Pull request 28 merged and that check now exists in `main` as `ori_p1_033_every_restatement_of_the_index_agrees_with_the_index`. A record claiming a gap that is closed is the same defect class as one claiming a check that does not exist, which is why this is a ticket and not a tidy-up. Found by ORI-T-0085's coder, which could not fix it because the file was outside its scope.

Claim 19, **ORI-T-0087**. Modules: `spec/agents/CLAUDE.md` and the generated root `CLAUDE.md`. Its load-bearing facts section states "The four fixtures under `fixtures/` are AICD products used by the end-to-end suite", and `fixtures/` contains only `planted/gate-1`, `gate-2`, `gate-7` and `gate-13`, which are planted-defect harnesses and not products. `scripts/gates.sh` independently reports `fixture not present: fixtures/new-product`. The count coincidence makes the sentence look satisfied by `ls`, and an agent told not to clean them up would protect the wrong four. Found by ORI-T-0085's coder.

This claim is a **pair**, per `spec/README.md`: `spec/agents/CLAUDE.md` is canonical and changes through the documentation role; the root `CLAUDE.md` is generated from it and is written by the coder role, because the documentation role's writes are refused outside `spec/`. The two are ordered together so the files do not diverge across a merge, and they ship as one pull request for that reason.

Claim 20, **ORI-T-0088**, **not granted yet**. Intended modules: `.github/workflows/ci.yml` (gate 13 job), `scripts/gates.sh` (gate 13 runner), `fixtures/planted/gate-13/**`. `ops/gates/gate-13.md` is deliberately excluded: under ruling R25 the coder produces evidence and the lead records it. This is tier 2 under `spec/RISK_MAP.md` (`.github/workflows` is tier 2, supply chain), so it is planned and brought to the operator rather than started alongside the other two.

Gate 13 does not resolve the `Spec:` trailer's anchor, and says so itself in `scripts/gates.sh`: "NOT ESTABLISHED: that the Spec: anchor resolves. The document half is printed above as an observation and is never judged". The lead put a fabricated anchor into ORI-T-0085's ticket, `TESTING.md#1-test-types`, which resolves against nothing; the coder caught it by reading the file, and gate 13 would have passed it. A ticket about fabricated references would have committed a fabricated reference through a gate documented not to look.

No two claims overlap: `crates/ori-core/src/error.rs`, the two instruction files, and the gate 13 paths are disjoint, and nothing else is in flight.
