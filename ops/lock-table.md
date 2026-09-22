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

---

## Round 3, after the defect batch merged

Claims recorded before dispatch, as CR-005 requires. Both tickets below are corrections of records that outlived their subject, which is the defect class ORI-T-0086 was raised about and which its own coder then found twice more.

Claim 21, **ORI-T-0089**, documentation role. Modules: `spec/TESTING.md` and `spec/README.md`.

`spec/TESTING.md` section 5 says "the fixtures **are** AICD products" and names four directories that do not exist. That is the upstream of the claim ORI-T-0087 just corrected in `CLAUDE.md`: batch 0 added `inert-gate` to this sentence and changed `CLAUDE.md`'s count to match, so `CLAUDE.md` was a correct restatement of a specification that was itself in the wrong tense. Anyone re-deriving the instruction file from the specification would reintroduce the defect ORI-T-0087 removed.

`spec/README.md` is in the same claim for a second reason: it fixes the root `CLAUDE.md` as "the same bytes, with a header naming the source" and **never says where the header ends**. ORI-T-0087's coder had to choose a definition to run its byte check. Until the specification pins it, any test comparing the pair encodes a convention no document states, so this blocks the pair-comparison ticket rather than merely annoying it.

Claim 22, **ORI-T-0090**, coder role. Modules: `crates/ori-gates/src/sections.rs`, `scripts/gates.sh`, `fixtures/planted/gate-13/README.md`.

Three records that report something other than the truth:

1. `crates/ori-gates/src/sections.rs` carries a register reason saying `crates/ori-core/src/error.rs` "arrives with pull request 27, ORI-T-0019, which is open and unmerged, so this test is red until that merges". Pull request 27 merged at `0974dd1`, before the commit carrying that sentence reached `main`. It is the text a reader is handed when the register fires.
2. `scripts/gates.sh` tells every run that gate 9 "stays defined and not installed until the index is generated". `methodology/sections.json` exists and is 11,961 bytes, and a test ties it to a fresh parse of the methodology. The real reason is recorded in three other places and is the operator's ruling pending the anchor fix. A reader following the runner would go generate an index that already exists.
3. `fixtures/planted/gate-13/` has no `README.md` while `gate-1`, `gate-2` and `gate-7` each do. The harness the commit-trailer gate depends on is the one with no explanation of what it holds.

`ops/gates/gate-13.md` is excluded from the claim: under ruling R25 the coder produces evidence and the lead records it.

Claims 21 and 22 are disjoint: `spec/` against `crates/`, `scripts/` and `fixtures/`. Nothing else is in flight.

**Not claimed, and queued behind claim 22 because both want `crates/ori-gates/src/sections.rs`:**

| Ticket | Work | Waits on |
|---|---|---|
| ORI-T-0091 | Resolve backticked function-name citations in doc comments against the test names actually declared, so a doc naming a test cannot outlive it | claim 22 releasing |
| ORI-T-0092 | The `CLAUDE.md` pair byte-comparison test, as a tier 1 test riding gate 2 rather than a fifteenth gate | claim 21 releasing, because it needs `spec/README.md` to define the header first |
| ORI-T-0088 | Gate 13 resolves the `Spec:` anchor | tier 2, with the operator |

---

## Round 4

Claim 23, **ORI-T-0091**, coder role, recorded before dispatch. Modules: `crates/ori-gates/src/sections.rs`.

Two pieces of one problem, which is why they are one ticket rather than two.

**The repository has four broken intra-doc links on `main` right now**, and every one of the eighteen pull request checks is green:

```
$ git status --porcelain            (clean)
$ RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps   exit 101
error: unresolved link to `AnchorState`
error: unresolved link to `REGENERATE_COMMAND`
error: public documentation for `source_bytes` links to private item `lf_line_endings`
error: public documentation for `parse` links to private item `lf_line_endings`
$ cargo doc --workspace --no-deps                              exit 0
```

All four are in this one file. ORI-T-0090's coder root-caused the first two empirically rather than guessing: `AnchorState` and `REGENERATE_COMMAND` are both `pub` in that same module, and rustdoc still cannot resolve them, because a module's inner `//!` docs resolve in **crate root** scope. It tested the repair instead of asserting it: `crate::sections::AnchorState` resolves, `self::AnchorState` does not.

**The second piece is what lets them exist.** ORI-T-0086's remedy left `crates/ori-core/src/error.rs` naming two tests in `crates/ori-gates` in prose, with nothing tying the names to the tests. Rename either and that doc is false again with every test green, which is the exact failure mode the named test was written to stop. ORI-T-0086's coder reported it as the highest-value follow-up it found and named the machinery: the sweep in this file already walks every file and already resolves `AICD §<n>` citations.

So the ticket fixes the four links and builds the check that makes the class visible, in the file that owns both.

**Not in scope, and deliberately:** a documentation gate. `spec/CI_CD.md` section 1 lists fourteen gates and none builds docs, so nothing in CI runs the strict form. That is `.github/workflows`, tier 2 under `spec/RISK_MAP.md`, and it needs a planted-defect proof under AICD §14. It goes to the operator alongside ORI-T-0088.

**Also running, and read only:** a repository-wide audit for the defect class this project keeps finding two or three at a time. It writes nothing and claims nothing. If it reports anything in `crates/ori-gates/src/sections.rs`, that report is against the tree as it was when the audit started, and this claim wins.
