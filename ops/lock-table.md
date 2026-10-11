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

> **Claim 12 was never entered.** This line releases a claim that does not exist above it, and the numbered rows run 1, 2, 2b, 3, 4, 5, 7, 8, 9, 10, 11 and stop; claim 6 is missing too. So **ORI-T-0014 holds no recorded claim anywhere**, and it is a tier 2 ticket that edited `.github/workflows/ci.yml`, created `fixtures/planted/gate-2/**` and produced `ops/gates/gate-2.md`. It ran against the same workflow file as three other tickets and the table that was supposed to serialise them never recorded it. Nothing collided, because the lead ran them serially by hand. Recorded now as claim 12, granted and released together, rather than invented as though it had been written at the time. Claim 6 remains an allocation gap; see [[R31]].

Claim 13, ORI-T-0016. Granted `.github/workflows/ci.yml`, `fixtures/planted/gate-7/**`. **Extended mid-ticket** under ruling R28 to `deny.toml`, `scripts/secret-scan.sh` and `scripts/gates.sh`, because a dependency policy the whole repository is judged by and a scanner a gate invokes do not belong in a fixture directory. **The lead granted that extension in a prompt and failed to record it here, so a reviewer correctly reported `scripts/gates.sh` as an unrecorded scope violation.** Same failure as rulings R15 to R24: a grant exists when it is written here, not when the lead states it. Recorded now.

Claim 14, ORI-T-0084. Granted `fixtures/planted/gate-1/workflow-samples/**`. The claim also named `ops/gates/gate-1.md` and `ops/gates/gate-2.md`, which was an error: ruling R25 makes operational records the lead's, so the ticket could not have used them. Released to the lead. **Extended to `fixtures/planted/gate-2/README.md`**, held by the lead rather than the coder, to correct a statement the ticket's own repair made false; nothing else claims it.

ORI-T-0017 remains queued behind claim 13. All four claim `.github/workflows/ci.yml`, so the lock table refuses a parallel start and they run serially, each branched from the previous. Splitting the planted defects into parallel worktrees and serializing only the YAML edit was considered and rejected: it is the lead routing around its own control to save wall clock, in the project whose product is the control.

Claim 15, ORI-T-0018. **Granted nothing, because it was never claimed.** The ticket ran, branched, committed and opened a pull request with no row in this table, and `ops/phase-1-backlog.md` declares its scope as `crates/ori-core/src/lib.rs` while the change landed in `crates/ori-cli/src/main.rs`. Nothing collided, because nothing else was in flight, which is the only reason this cost nothing. Recorded now as held and released together: `crates/ori-cli/src/main.rs`, plus `ops/gates/gate-2.md`, `ops/gates/gate-13.md`, `ops/rulings.md` and this file, all lead records under ruling R25.

This is the fourth occurrence of one failure: a grant or a record that exists in the lead's intent and not in a file. See CR-005.

No two claims overlap, verified mechanically before the claims were granted.

> **This paragraph's second sentence was deleted, and what it said is recorded here rather than silently dropped.** It read: "ORI-T-0001 through 0005, 0010 and 0012 through 0018 are unclaimed: 0010 is held on a missing precondition (the organizational repository), the rest are held on the Rust toolchain." It was true when batch 1 opened and false within hours, because rows seven to eleven of this same table, eighteen lines above it, grant claims to five of the tickets it calls unclaimed, and claims 13 and 15 grant two more. Every ticket it names except ORI-T-0010 and ORI-T-0015 has since merged. A standing sentence about a moving set, written below the rows that move it. Found by the round 4 audit ([[CR-007]]).

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
| **ORI-T-0095** | The `CLAUDE.md` pair byte-comparison test, as a tier 1 test riding gate 2 rather than a fifteenth gate. **Renumbered from ORI-T-0092 by [[R31]]**, which the lead allocated twice. Claim 21 is released by R31, and `spec/README.md` now defines the header boundary, so this is unblocked | nothing |
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

---

## Round 5: working the audit's queue

Thirty of [[CR-007]]'s thirty-two findings remain. They are dispatched by owner and by tier, not in one pull request. Every claim below was recorded before its agent was launched.

Claim 24, **ORI-T-0092**, documentation role. Modules: `spec/CONVENTIONS.md`, `spec/LLD.md`, `spec/TESTING.md`, `spec/README.md`. Tier 1; none of these four is in `spec/RISK_MAP.md`'s tier 2 list.

Three records that outlived their subject, all of them the same shape round 4 corrected elsewhere. `spec/CONVENTIONS.md` and `spec/LLD.md` still describe `fixtures/` as holding AICD products, which is the exact claim ORI-T-0087 and ORI-T-0089 removed from `CLAUDE.md` and `spec/TESTING.md` section 5, so the specification now contradicts itself in two directions. `spec/TESTING.md` section 3 sends phase 1's thresholds to `ops/calibration.md`, a file ruling [[R30]] closed to new entries. `spec/README.md`'s registry still reports `methodology/sections.json` as Missing; it is committed and three other records treat it as present.

Claim 25, **ORI-T-0093**, coder role. Modules: `crates/ori-core/src/error.rs`, `crates/ori-core/src/types.rs`. Tier 1: `spec/RISK_MAP.md` tiers this crate by content, and neither the state machines nor the permission function exists yet.

Two vacuous checks and one wrong citation. The first is the one the lead endorsed in public and should not have: `ori_p1_033_every_refusal_this_crate_can_make_is_covered_by_these_tests` asserts only that its own list has no duplicates. The exhaustive match lives in `refusal_tag`, so adding a variant breaks the build until an arm is added there, and nothing then ties the variant to `every_refusal()`. The second: three of the ten `wire_enum!` value lists, `ProductOrigin::ALL`, `TicketKind::ALL` and `DocumentSet::ALL`, are outside the count test, and the only other test that touches them loops over `ALL`, so emptying one passes everything. The third: `E_UPGRADE_ONLY` is cited to API_SPEC section 2; it is in section 1, and section 2 is the event stream.

Claim 26, **ORI-T-0094**, coder role. Modules: `README.md`, `crates/ori-gates/src/sections.rs`. **Tier 2**, inherited from `sections.rs`, which ORI-T-0091's coder raised to 2 as gate substrate and which `spec/RISK_MAP.md` still does not tier at all.

Four self-descriptions that are wrong. The root README says every test in the repository is in `ori-gates` and covers the section index generator; `ori-core` holds 27 that do not. Its Status table undercounts installed gates and local runners. `sections.rs` opens by saying the citation gate resolves every `AICD §<n>` reference in the repository, and **there is no citation gate**: gate 9 is Defined, `citation_gate.rs` does not exist, and `ops/gates/gate-9.md` does not exist. The same file says three times that there are three `ori_p1_033_reader_*` tests; there are four.

The three claims are disjoint: `spec/`, `crates/ori-core/`, and `README.md` with `crates/ori-gates/`.

**Held for the operator, because they are tier 2 or they are the lead's own records:**

| Finding | Where | Why it waits |
|---|---|---|
| ADR-0002 undercounts batch 1's tier 2 tickets and claims a merge order that did not happen | `spec/adr/ADR-0002-single-operator.md` | ADRs are tier 2 in `spec/RISK_MAP.md`, and this is the ADR that governs every tier 2 merge |
| `spec/RISK_MAP.md` claims to tier every module and does not | `spec/RISK_MAP.md` | tier 2 by its own table |
| `ci.yml`'s header is stale in four places and contradicts `ops/gates/gate-2.md` | `.github/workflows/ci.yml` | tier 2, supply chain |
| Gate 13's pull-request cross-check is skipped when the payload carries no commit count | `scripts/gates.sh` | gate integrity, and it changes a gate's verdict |
| Three `prove.sh` self-descriptions disagree with their own tables | `fixtures/planted/gate-1`, `gate-7` | gate integrity |
| Eight defects in `ops/` records | `ops/` | ruling R25 makes these the lead's, and the lead is writing them |


---

## Releases

Ruling [[R31]] writes the releases this table always said it recorded and never did: twenty-three claims and four partial releases. The full table is in `ops/rulings/R31-identifiers-and-releases.md` rather than duplicated here, because two copies of one record is the defect [[CR-007]] spent six lenses looking for.

As of R31 the only claims still **held** are 20 (ORI-T-0088, tier 2 with the operator), 24, 25 and 26. Everything numbered 1 to 23 is released.

---

## Round 6 corrections and releases

**Claim 27's recorded scope was wrong when it was written, and the error is the lead's.** It read "`crates/ori-gates/` (any file under it, including a new module and the `mod` line in `lib.rs`)". Claim 26 was **held** on `crates/ori-gates/src/sections.rs` at that moment, for ORI-T-0094. So the lead wrote a claim that overlapped a claim the lead had recorded itself, one round earlier, in this file.

`aicd_plan_submit` is what returns `E_SCOPE_LOCKED`. It does not exist ([[E-0005]]), so nothing checked it, and [[R31]] exists because this class keeps recurring. The coder found the overlap by reading this table, narrowed its own footprint below it without being asked, never read-modified `sections.rs`, and delivered one new file plus four added lines in `lib.rs`, which claim 26 does not hold.

**Claim 27 is narrowed to what was actually worked:** `crates/ori-gates/src/spec_refs.rs`, and the `mod` line in `crates/ori-gates/src/lib.rs`. The `mod` line follows the precedent this table already records: a file every parallel ticket must edit is not a lock, it is a queue.

### Releases owed since R31

| Claim | Ticket | Released |
|---|---|---|
| 24 | ORI-T-0092 | merged, pull request 41 |
| 25 | ORI-T-0093 | merged, pull request 43 |
| 26 | ORI-T-0094 | merged, pull request 42 |
| 28 | ORI-T-0098 | **held**, pull request 46 open |
| 27 | ORI-T-0097 | **held**, pull request 47 open |

Claim 20 (ORI-T-0088) is still held and still the operator's.

### Allocated by this round, none of them started

| Ticket | Work | Why it is not running |
|---|---|---|
| ORI-T-0099 | `RefusalKind::code` calls `E_SCOPE_LOCKED` a code the client API returns; `spec/API_SPEC.md` puts it on the agent-facing MCP server | queued behind claim 28 |
| ORI-T-0100 | Globs are an undocumented blind spot of ORI-T-0091's citation check | queued behind claim 27 |
| **ORI-T-0101** | **The credential-rotation runbook does not exist and is cited three times in the text a human reads after the secret scan finds a credential.** The path is `spec/runbooks/` plus `rotate-credentials.md`, written in two pieces here because this record is scanned by the reader ORI-T-0097 built and a whole one is an unresolvable reference. `scripts/gates.sh` twice and `scripts/secret-scan.sh` once. Either write the runbook or change the three sentences | ready; `scripts/` is not tiered by `spec/RISK_MAP.md` except as `scripts/release*`, so the operator sets the tier |
| ORI-T-0102 | Pin `TicketState` and `DocumentState` identifiers against `spec/DATA_MODEL.md` section 3's diagrams. Today only their counts, 12 and 5, are pinned | queued behind claim 28 |
| ORI-T-0103 | Hold `crates/ori-core/src/types.rs`'s restatement of the value spellings against `spec/DATA_MODEL.md` the way `RESTATEMENTS` holds `error.rs` against the methodology. ORI-T-0098 proved the gap and left the register entry ready | queued behind claim 27 |
| ORI-T-0104 | `spec/CONVENTIONS.md` fixes the `Spec:` trailer as `<document>#<section>` and the repository uses both `#1` and `#1-pipeline-on-every-pull-request`. One of the two should be named | documentation role |
| ORI-T-0105 | A coder scoped to one file cannot write a cross-reference to a test: any backticked `#[test]` name in a doc comment under `crates/` fails ORI-T-0091's check unless `CITED_TESTS` in another crate registers it. Worth a line in `spec/CONVENTIONS.md` | documentation role |

---

## Batches 2 to 6: releases, claims and allocations owed

Written on 2026-09-27, in one pass, from the merge history: `gh pr view` on pull requests 45 to 71 (title, body, files, merge time, merge commit), their GitHub timelines, and `git log origin/main` with its `Ticket:` trailers. Nothing here is written from memory and nothing is backdated. A date inside a row is the date of the event it records; every row was written on 2026-09-27, and each table says so in its own column.

**What was owed.** The last claim or release this table recorded is pull request 48's, in "Round 6 corrections and releases" above. The only later change to this file is one line in commit 5adaea7, carried by pull request 50, which repaired a reference. Since then eighteen pull requests merged (50 to 53, 57, and 59 to 71), carrying 31 tickets, and the lead allocated ORI-T-0106 to ORI-T-0111 in pull request bodies and commit trailers. None of it was written here. Under [[R31]] rule 2 a claim with no release is held whatever the tree looks like, and under rule 1 an allocation is made here and nowhere else, so from 2026-09-22 until this section this table showed claims 27 and 28 as held after they merged, and showed none of the 31.

**What the merge history shows about overlap.** Nothing was checked against this table before those coders ran. Checked now, file by file from the commits on `main`: outside the queue files (a crate's `lib.rs` module lines, its `Cargo.toml` dependency list, and `Cargo.lock`) and the lead's own records, every file two tickets wrote was written by the second after the first, either on a branch stacked on it or after it merged. ORI-T-0108 wrote four files of ORI-T-0026, ORI-T-0027 and ORI-T-0028 (stacked) and ORI-T-0098's `crates/ori-core/src/types.rs` (merged); ORI-T-0107 wrote ORI-T-0024's `db.rs` and ORI-T-0025's migration 2 (stacked); ORI-T-0111 wrote ORI-T-0032's `injector.rs` (merged); pull request 68 edited `spec/adr/ADR-0001-stack.md` after ORI-T-0109 had (merged). The queue files conflicted when pull requests 63, 70 and 71 were rebuilt on `main`, and each pull request says how it was resolved by hand. Nothing collided. That is the history's answer after the fact, not a check that ran before dispatch.

### a. Releases owed on claims still shown as held

| Claim | Ticket | Modules | Released | Recorded |
|---|---|---|---|---|
| 27 | ORI-T-0097 | `crates/ori-gates/src/spec_refs.rs`, the `mod` line in `crates/ori-gates/src/lib.rs`, as narrowed above | merged, pull request 47, 2026-09-22 | 2026-09-27, from the merge history |
| 28 | ORI-T-0098 | `crates/ori-core/src/types.rs`, the only file pull request 46 changed. Claim 28's grant row was on pull request 45, which closed unmerged, so this table never held its modules until now | merged, pull request 46, 2026-09-22 | 2026-09-27, from the merge history |

**Claim 20, ORI-T-0088, is still held, and has still never been granted.** Searched for the identifier in every commit message on every ref, every branch name, and the title, body and comments of all 71 pull requests. It appears only in lead records describing it as not started, the latest being commit 924c993 on the branch of pull request 45, which closed unmerged. No branch, commit trailer or pull request carries it, and `scripts/gates.sh` still prints at gate 13 "NOT ESTABLISHED: that the Spec: anchor resolves". It is tier 2 and stays with the operator.

### b. Claims for the tickets pull requests 50 to 71 merged

Numbered from 29, in merge order, and within one pull request in the order of its commits on `main`. Each is granted and released together, because each was worked without a claim and has merged. Modules are the files the pull request changed for that ticket, grouped by crate. "(queue)" marks a file every parallel ticket in that crate must edit, which the "Round 6 corrections" precedent treats as a queue and not a lock. Tier is as the pull request's title states it.

| # | Ticket | Modules | Tier (title) | PR | Merged | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 29 | ORI-T-0020 | `crates/ori-core/src/`: `ticket.rs`, `lib.rs` (queue) | 2 | 50 | 2026-09-22 | merged | 2026-09-27, merge history |
| 30 | ORI-T-0021 | `crates/ori-core/src/`: `document.rs`, `phase.rs`, `lib.rs` (queue) | 2 | 50 | 2026-09-22 | merged | 2026-09-27, merge history |
| 31 | ORI-T-0022 | `crates/ori-core/src/`: `permission.rs`, `lib.rs` (queue) | 2 | 50 | 2026-09-22 | merged | 2026-09-27, merge history |
| 32 | ORI-T-0050 | `crates/ori-orchestrator/`: `src/lock_table.rs`, `Cargo.toml` (the `ori-core` edge), `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 51 | 2026-09-22 | merged | 2026-09-27, merge history |
| 33 | ORI-T-0051 | `crates/ori-orchestrator/src/`: `escalation.rs`, `budgets.rs`, `lib.rs` (queue) | 2 | 51 | 2026-09-22 | merged | 2026-09-27, merge history |
| 34 | ORI-T-0052 | `crates/ori-orchestrator/src/`: `closing.rs`, `lib.rs` (queue) | 2 | 51 | 2026-09-22 | merged | 2026-09-27, merge history |
| 35 | ORI-T-0042 | `crates/ori-gates/src/`: `coverage.rs`, `lib.rs` (queue) | 2 | 51 | 2026-09-22 | merged | 2026-09-27, merge history |
| 36 | ORI-T-0043 | `crates/ori-gates/src/`: `modified_tests.rs`, `lib.rs` (queue) | 2 | 51 | 2026-09-22 | merged | 2026-09-27, merge history |
| 37 | ORI-T-0030 | `crates/ori-runtime/`: `src/session.rs`, `src/worktree.rs`, `Cargo.toml`, `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 51 | 2026-09-22 | merged | 2026-09-27, merge history |
| 38 | ORI-T-0049 | `crates/ori-orchestrator/src/`: `lifecycle.rs`, `lib.rs` (queue) | 2 | 52 | 2026-09-22 | merged | 2026-09-27, merge history |
| 39 | ORI-T-0053 | `crates/ori-orchestrator/src/`: `merge_queue.rs`, `lib.rs` (queue) | 2 | 52 | 2026-09-22 | merged | 2026-09-27, merge history |
| 40 | ORI-T-0041 | `crates/ori-gates/src/`: `gate.rs`, `runner.rs`, `lib.rs` (queue) | 2 | 52 | 2026-09-22 | merged | 2026-09-27, merge history |
| 41 | ORI-T-0034 | `crates/ori-runtime/src/`: `budget.rs`, `transcript.rs`, `recovery.rs`, `lib.rs` (queue) | 2 | 52 | 2026-09-22 | merged | 2026-09-27, merge history |
| 42 | ORI-T-0044 | `crates/ori-gates/`: `src/significance.rs`, `Cargo.toml`, `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 52 | 2026-09-22 | merged | 2026-09-27, merge history |
| 43 | ORI-T-0023 | `crates/ori-store/`: `src/event_log.rs`, `Cargo.toml`, `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 53 | 2026-09-23 | merged | 2026-09-27, merge history |
| 44 | ORI-T-0024 | `crates/ori-store/`: `src/db.rs`, `migrations/0001_init.sql`, `Cargo.toml`, `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 53 | 2026-09-23 | merged | 2026-09-27, merge history |
| 45 | ORI-T-0106 | `crates/ori-store/tests/store_seam.rs` | 2 | 53 | 2026-09-23 | merged | 2026-09-27, merge history |
| 46 | ORI-T-0109 | `spec/adr/ADR-0001-stack.md` (the "Model family" decision row), `spec/DATA_MODEL.md` (the `AgentIdentity` field list) | not stated; see below | 57 | 2026-09-23 | merged | 2026-09-27, merge history |
| 47 | ORI-T-0025 | `crates/ori-store/`: `src/projections/` (`mod.rs`, `ticket.rs`, `lock.rs`, `escalation.rs`, `payload.rs`), `src/rebuild.rs`, `migrations/0002_projections.sql`, `src/lib.rs` (queue) | 2 | 59 | 2026-09-23 | merged | 2026-09-27, merge history |
| 48 | ORI-T-0107 | `crates/ori-store/`: `migrations/0002_projections.sql` (two indexes, added before registration), `src/db.rs` (the `MIGRATIONS` entry, and four tests under [[R37]]), `tests/projections_reachable.rs` | 2 | 59 | 2026-09-23 | merged | 2026-09-27, merge history |
| 49 | ORI-T-0026 | `crates/ori-broker/`: `src/identity.rs`, `src/keychain.rs`, `Cargo.toml`, `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 60 | 2026-09-23 | merged | 2026-09-27, merge history |
| 50 | ORI-T-0028 | `crates/ori-broker/src/`: `family.rs`, `lib.rs` (queue) | 2 | 60 | 2026-09-23 | merged | 2026-09-27, merge history |
| 51 | ORI-T-0027 | `crates/ori-broker/src/`: `issuance.rs`, `lib.rs` (queue) | 2 | 60 | 2026-09-23 | merged | 2026-09-27, merge history |
| 52 | ORI-T-0108 | `crates/ori-broker/src/`: `registration.rs`, `family.rs`, `identity.rs`, `keychain.rs`, `issuance.rs` (twelve test call sites across the last three, under [[R37]]), `lib.rs` (queue); `crates/ori-core/src/`: `types.rs` (`ModelFamily`), `lib.rs` (queue) | 2 | 60 | 2026-09-23 | merged | 2026-09-27, merge history |
| 53 | ORI-T-0031 | `crates/ori-runtime/src/`: `container.rs`, `lib.rs` (queue) | 2 | 62 | 2026-09-23 | merged | 2026-09-27, merge history |
| 54 | ORI-T-0032 | `crates/ori-runtime/src/`: `injector.rs`, `lib.rs` (queue) | 2 | 63 | 2026-09-23 | merged | 2026-09-27, merge history |
| 55 | ORI-T-0033 | `crates/ori-runtime/`: `src/acp.rs`, `src/headless.rs`, `Cargo.toml` (`serde`, `serde_json`), `src/lib.rs` (queue); `Cargo.lock` (queue) | not stated; body: 1 | 64 | 2026-09-23 | merged | 2026-09-27, merge history |
| 56 | ORI-T-0037 | `crates/ori-memory/`: `src/barrier.rs`, `Cargo.toml`, `src/lib.rs` (queue); `Cargo.lock` (queue) | 2 | 66 | 2026-09-23 | merged | 2026-09-27, merge history |
| 57 | ORI-T-0111 | `crates/ori-runtime/src/injector.rs` | 2 | 69 | 2026-09-23 | merged | 2026-09-27, merge history |
| 58 | ORI-T-0036 | `crates/ori-memory/`: `src/code_map.rs`, `Cargo.toml` (`tree-sitter` and the four grammars), `src/lib.rs` (queue); `Cargo.lock` (queue) | not stated; body: 1 | 70 | 2026-09-27 | merged | 2026-09-27, merge history |
| 59 | ORI-T-0035 | `crates/ori-memory/`: `src/indexer.rs`, `src/freshness.rs`, `Cargo.toml` (`rusqlite`), `src/lib.rs` (queue); `Cargo.lock` (queue) | not stated; body: 1 | 71 | 2026-09-27 | merged | 2026-09-27, merge history |

**How the several-ticket pull requests were split.** Pull requests 53, 59 and 60 carry one or more commits per ticket, each with that ticket's own `Ticket:` trailer, so their rows follow the commits. Three commits carry several tickets under one trailer: `fd3afd4` (pull request 50) carries ORI-T-0020, ORI-T-0021 and ORI-T-0022 under `Ticket: ORI-T-0022`; `54d7122` (pull request 51) carries ORI-T-0050, ORI-T-0051, ORI-T-0052 and ORI-T-0042 under `Ticket: ORI-T-0051`; `bd7800b` (pull request 52) carries ORI-T-0049, ORI-T-0053 and ORI-T-0041 under `Ticket: ORI-T-0041`. Their rows are split by `ops/phase-1-backlog.md`'s declared scope, and each commit's body describes its tickets file by file in the same way, so the split is honest. The one file there the backlog does not assign, `crates/ori-orchestrator/Cargo.toml`, is ORI-T-0050's by commit `54d7122`'s own body. What the trailers alone would tell a reader is narrower: **ORI-T-0020, ORI-T-0021, ORI-T-0042, ORI-T-0049, ORI-T-0050, ORI-T-0052 and ORI-T-0053 are named in no `Ticket:` trailer on `main`.** Anything that traces tickets through trailers (AICD §13) will not find those seven.

**Tiers.** Four titles state no tier: pull requests 57, 64, 70 and 71. Their bodies say tier 1. For ORI-T-0033, ORI-T-0036 and ORI-T-0035 that agrees with `spec/RISK_MAP.md`, except that the map does not name `freshness.rs`. For ORI-T-0109 it does not: pull request 57 changed an ADR, which `spec/RISK_MAP.md` tiers 2, and the backlog's own rule makes a ticket tier 2 when its scope touches a path tiered 2, whatever the ticket says.

**Not claims.** Pull request 49 closed unmerged; its content is commit 5adaea7, the first commit of pull request 50. Pull requests 54, 55, 56 and 58 closed as superseded by 59 and 60, which carry their commits unchanged; their tickets' claims are the rows above. Lead records under ruling R25 rode inside code pull requests: `ops/escalations/E-0007-may-the-lead-merge.md` (pull requests 50 and 52), `ops/incidents/INC-0003-two-coders-wrote-to-the-primary-checkout.md` (pull request 52), and commit 5adaea7's repairs to E-0006 and this file (pull request 50).

**Records pull requests in the same window.** Not claims on code, listed so the window is complete:

| PR | Merged | Title's tier | Files | `Ticket:` trailer | Recorded |
|---|---|---|---|---|---|
| 61 | 2026-09-23 | 0 | `ops/escalations/E-0008-json-serialization.md` (new), `ops/escalations/E-0004-external-crates.md` (its State line) | ORI-T-0033 | 2026-09-27, merge history |
| 65 | 2026-09-23 | 0 | `ops/escalations/E-0004-external-crates.md` (`tantivy` and `tree-sitter` approved) | ORI-T-0035 | 2026-09-27, merge history |
| 67 | 2026-09-23 | 0 | `ops/escalations/E-0004-external-crates.md` (`tantivy` withdrawn for SQLite FTS5) | ORI-T-0035 | 2026-09-27, merge history |
| 68 | 2026-09-23 | not stated | `spec/adr/ADR-0003-full-text-index.md` (new), `spec/adr/ADR-0001-stack.md` (one decision row), `spec/LLD.md`, `spec/ROADMAP.md` | ORI-T-0035 | 2026-09-27, merge history |

Pull request 68 is not an `ops/` record. It is a documentation-role change to four files under `spec/`, two of them ADRs, which `spec/RISK_MAP.md` tiers 2. It is listed here rather than as a claim because it carried no ticket of its own: its commit names ORI-T-0035, whose claim is 59.

### c. Claims held as of 2026-09-27

Both dispatched by the lead on 2026-09-27 and recorded at dispatch. Both branches exist, cut from `main` at 423fc55.

| # | Ticket | Modules | Tier | Branch | Claimed | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 60 | ORI-T-0038, the memory scope enforcer | `crates/ori-memory/src/scope.rs` | 2, `spec/RISK_MAP.md` (`barrier.rs`, `scope.rs`) | `feat/ORI-T-0038-memory` | 2026-09-27 | **held** | 2026-09-27, at dispatch |
| 61 | ORI-T-0039, the operational log and the retrieval package | `crates/ori-memory/src/oplog.rs`, `crates/ori-memory/src/retrieval.rs` | 1, `ops/phase-1-backlog.md`; `spec/RISK_MAP.md` tiers retrieval 1 and does not name the operational log | `feat/ORI-T-0039-memory` | 2026-09-27 | **held** | 2026-09-27, at dispatch |

**`retrieval.rs` is built only after ORI-T-0038 merges.** The retrieval package must go through the scope enforcer, so building it first would mean either calling an enforcer that does not exist or assembling a package without one. `oplog.rs` may proceed now.

The `pub mod` lines in `crates/ori-memory/src/lib.rs` follow the precedent in "Round 6 corrections" above: a file every parallel ticket must edit is a queue, not a lock. Neither claim holds it.

Checked against every claim this table shows as held once this section is read: claim 20 (`.github/workflows/ci.yml`, `scripts/gates.sh`, `fixtures/planted/gate-13/**`) is the only other one, and it is disjoint from both. Claims 60 and 61 are disjoint from each other. ORI-T-0040 (`citation.rs`, `drift.rs`) follows both, in the backlog's order.

### d. Allocations

**Every identifier already allocated, searched rather than remembered**, because the lead has collided twice (ORI-T-0025 and ORI-T-0102 were already taken when it tried to reuse them). Searched: the working tree, including `ops/`, `spec/`, `crates/`, `scripts/`, `templates/`, `.github/`, `fixtures/`, `README.md` and `CLAUDE.md`; the tree of every one of the repository's 141 refs; `git log --all` message bodies; every branch name; the title, body and head branch of all 71 pull requests, and every pull request comment. **The highest identifier found is ORI-T-0111**: pull request 69, commit 5afbf10, and the tests naming it in `crates/ori-runtime/src/injector.rs`. `ORI-T-9999` also appears, in `fixtures/planted/gate-13/messages/ticket-duplicate.txt`; it is a planted defect for gate 13, not an allocation. The next free identifier was therefore ORI-T-0112.

The search also found identifiers allocated outside this table, which [[R31]] rule 1 names as the only place an allocation happens. Recorded here, with where each was made, so the next search starts from one file:

| Identifier | Work | Allocated in | Recorded |
|---|---|---|---|
| ORI-T-0000 | Batch 0, specification corrections | pull request 1 | 2026-09-27 |
| ORI-T-0080 to ORI-T-0083 | Root README; LICENSE and NOTICE; a line-ending independent methodology parser; the design reference on the front page | pull requests 13, 14, 15 and 18 | 2026-09-27 |
| ORI-T-0096 | The lead's repair of eight record defects in `ops/` | pull request 44's body, commit 67cec99 | 2026-09-27 |
| ORI-T-0106 | The store seam test | pull request 53 (claim 45) | 2026-09-27 |
| ORI-T-0107 | Registering migration 2 | pull request 55, carried by 59 (claim 48) | 2026-09-27 |
| ORI-T-0108 | The model family on `AgentIdentity`, enforced at registration | pull request 57's body, then 58, carried by 60 (claim 52) | 2026-09-27 |
| ORI-T-0109 | ADR-0001 records the model family on `AgentIdentity` | pull request 57 (claim 46) | 2026-09-27 |
| ORI-T-0110 | Provider-only container egress, below | pull request 62's body | 2026-09-27 |
| ORI-T-0111 | An injected session never inherits the engine's stdio | pull request 69 (claim 57) | 2026-09-27 |

**One identifier is carried by work that is not its own.** Commit 5adaea7 (pull request 50) carries `Ticket: ORI-T-0101`, the credential-rotation runbook ticket allocated in round 6, which has not started. The commit repaired three references in the lead's records, one of them in ORI-T-0101's own row. The runbook keeps the identifier. The trailer is published and is not rewritten (CLAUDE.md rule 2), so a reader tracing ORI-T-0101 through `git log` will meet that commit first.

**ORI-T-0110 recorded.** Provider-only container egress: an engine-side proxy that restricts a container's outbound network to the operator's configured provider endpoints. `crates/ori-runtime`, next to `container.rs`, which `spec/RISK_MAP.md` tiers 2; tier 2. Allocated in pull request 62's body on 2026-09-23 and named in `crates/ori-runtime/src/container.rs`'s module doc, never in this table until now. Not started; ready. Ruling [[R36]] is why it exists.

**ORI-T-0029**, the forbidden-action test harness (`crates/ori-broker/src/forbidden.rs`, tier 2), remains blocked on [[E-0007]], open with the operator.

**Follow-ups allocated now**, from the lead's review of pull requests 70 and 71 under ruling [[R35]]. Each is stated as the defect, not the fix. None is claimed: each claim is written when its ticket is dispatched.

| Ticket | Defect | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0112 | The code map does not list exported Go methods as interfaces | `crates/ori-memory/src/code_map.rs` | 1, `spec/RISK_MAP.md` (code map) | ready | 2026-09-27 |
| ORI-T-0113 | The code map does not list re-exports as interfaces: Rust `pub use`, TypeScript `export ... from` | `code_map.rs` | 1 | ready | 2026-09-27 |
| ORI-T-0114 | The code map reports an import that resolves to a crate root (`lib.rs`, `main.rs`) as `NotFound` | `code_map.rs` | 1 | ready | 2026-09-27 |
| ORI-T-0115 | `use {self as me}` yields `NotAttempted` where the ungrouped form yields `Resolved` | `code_map.rs` | 1 | ready | 2026-09-27 |
| ORI-T-0116 | An escaped Go import specifier is not resolved like its unescaped form | `code_map.rs` | 1 | ready | 2026-09-27 |
| ORI-T-0117 | The per-module link budget and the per-link depth count are unmeasured at scale, against performance criterion ORI-P1-030. Pull request 70 records that past about 400,000 links legitimate links time out under the shared budget, and that the per-link lookup bound counts the checkout's own depth | `code_map.rs` | 1 | ready to measure; ORI-P1-030's pass line is the baseline ORI-T-0078 records | 2026-09-27 |
| ORI-T-0118 | Drift history does not follow a rename, including a case-only rename (`DATA_MODEL.md` to `DATA_MODEL.MD`, `spec/runbooks` to `spec/RUNBOOKS`): the old records drop as a removal | `crates/ori-memory/src/indexer.rs`, `freshness.rs` | 1 for `indexer.rs`; `spec/RISK_MAP.md` does not name `freshness.rs`, so the operator sets it, and the lead recommends 1, the tier ORI-T-0035 carried it at | **waiting on the operator**: should drift history follow a rename? | 2026-09-27 |
| ORI-T-0119 | A file named with an upper-case `.MD` extension is not recognised as Markdown | `indexer.rs` | 1 | ready | 2026-09-27 |
| ORI-T-0120 | A genuine single allocation failure during `incremental_sync` can still make the diagnosis call a healthy index `Corrupt`. The index is derived data and `recover` rebuilds it, so what is lost is a rebuild | `indexer.rs` | 1 | ready | 2026-09-27 |
| ORI-T-0121 | Adding a heading below unchanged text may mark the unchanged text changed: false staleness | `freshness.rs` | operator sets; the lead recommends 1 | **reproduce on `main` before starting**: it was observed before later fixes to freshness landed | 2026-09-27 |
| ORI-T-0122 | `ProductDb::open` keeps the directory path as given, possibly relative, and does not keep the identity of the directory it locked, so a later chdir or a swapped directory can redirect an operation. It should canonicalize once and hold that identity | `crates/ori-store/src/db.rs` | `spec/RISK_MAP.md` does not name `db.rs`, so the operator sets it; the lead recommends 2, because `db.rs` holds the write lock over the event log and runs the migrations the map tiers 2 | ready; meanwhile `indexer.rs` refuses a relative directory (pull request 71) | 2026-09-27 |

ORI-T-0112 to ORI-T-0117 all edit `code_map.rs`, and ORI-T-0118 to ORI-T-0121 all edit `indexer.rs` or `freshness.rs`, so each group runs one ticket at a time. None of the eleven overlaps claim 60 or 61.

**A constraint, not a ticket.** The readiness check `ori-flows` will build must call `freshness::StaleReport::is_ready()` and never re-derive readiness from the stale list: a file whose specification text the walk left out is freshness unknown, may carry no record at all, and still makes the product not ready (pull request 71). The backlog already holds that work, so no identifier is allocated for it. It is recorded against **ORI-T-0061**, readiness computation, `crates/ori-flows/src/readiness.rs`, batch 12, criteria ORI-P1-002 and ORI-P1-026 (ORI-P1-026 is the criterion whose action is `ori readiness`). ORI-T-0071, which builds the `ori` commands, exposes that computation and must not compute readiness itself.

**State after this section:** the claims held are 20, 60 and 61. The next free identifier is ORI-T-0123.

---

## Claim 62, and claim 61 extended

Written by the lead at dispatch, 2026-09-27, under [[R31]] rule 2.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 62 | ORI-T-0040, the citation checker and the drift audit | `crates/ori-memory/src/citation.rs`, `crates/ori-memory/src/drift.rs`, `crates/ori-memory/src/lib.rs` (queue); `crates/ori-memory/src/freshness.rs` only additively, and only if persisting the freshness tracker needs it | 1, `spec/RISK_MAP.md` (drift, citation) | `feat/ORI-T-0040-memory` | 2026-09-27 | **held** | 2026-09-27, at dispatch |

The citation checker's rule set, which `ops/phase-1-backlog.md` batch 6 listed as undecided, was decided by the operator on 2026-09-27 in [[E-0006]]: everything under `spec/`, HTML included, with the design mockup excluded by name and its marker asserted on every run.

**Claim 61 extended.** ORI-T-0039's targeted repair round writes `crates/ori-memory/src/barrier.rs`, additively: the barrier records which fields its cap cut in the `memory.record_created` payload, so the operational log can say a claim field was cut instead of parsing half an entry as a path. `spec/RISK_MAP.md` tiers `barrier.rs` 2, so that change is tier 2 whatever the rest of ORI-T-0039 is. No other claim holds `barrier.rs`: ORI-T-0037's claim was released when pull request 66 merged. No existing test of `barrier.rs` may change; a change that would need one stops as `test_modified`.

**State after this section:** the claims held are 20, 60, 61 and 62. The next free identifier is ORI-T-0123.

---

## ORI-T-0123 allocated

Written by the lead on 2026-09-27 under [[R31]] rule 1.

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0123 | Amend ADR-0002 to state [[R38]]: the lead session merges tier 0 and tier 1 once every check passes, and the operator merges every tier 2 change personally | `spec/adr/ADR-0002-single-operator.md`, and any other statement in `spec/` the decision makes false | 2, `spec/RISK_MAP.md` (ADRs) | dispatched to the documentation role, branch `docs/adr-0002-lead-merges-tier-0-and-1` | 2026-09-27 |

**State after this section:** the claims held are 20, 60, 61 and 62. The next free identifier is ORI-T-0124.

---

## ORI-T-0124 and ORI-T-0125 allocated

Written by the lead on 2026-09-27 under [[R31]] rule 1.

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0124 | The paired regeneration `spec/README.md` requires after ORI-T-0123 changes `spec/agents/CLAUDE.md`, and the copy `.claude/agents/README.md` requires after it changes `spec/agents/lead.md`: the root `CLAUDE.md` body and `.claude/agents/lead.md` made byte-identical to their sources again | `CLAUDE.md`, `.claude/agents/lead.md` | 2, carried with ORI-T-0123 on one branch so the pairs never diverge across a merge | dispatched to the coder role, on `docs/adr-0002-lead-merges-tier-0-and-1` | 2026-09-27 |
| ORI-T-0125 | Three code map deadline tests time out under machine load and fail `cargo test --workspace` locally, on `main` as well as on every branch: `ori_t_0036_retained_heading_text_is_bounded_for_many_modules_citing_long_headings`, `ori_t_0036_a_spec_document_the_deadline_interrupted_is_recorded`, `ori_t_0036_the_rust_declaration_walk_checks_its_deadline`. Each passes alone in about a second; CI has passed them. Reported by the ORI-T-0038 coder and the ORI-T-0123 documentation agent on 2026-09-27 | `crates/ori-memory/src/code_map.rs` (its tests) | 1, `spec/RISK_MAP.md` (code map) | **waiting on the operator**: the fix changes existing tests, which is trigger `test_modified` | 2026-09-27 |

**State after this section:** the claims held are 20, 60, 61 and 62. The next free identifier is ORI-T-0126.

---

## Session of 2026-10-11: claim 60 released, claims 61 and 62 re-dispatched, claim 63

Written by the lead at dispatch, 2026-10-11, under [[R31]] rules 2 and 3. Coders in this session run on Sonnet 5.5 ([[R39]]).

**Claim 60 released.** ORI-T-0038 merged as pull request 77 on 2026-09-27; its commits are on `main` (1f5328f to 9c661e1). Verified from the merge history, not from memory.

**Claims 61 and 62 are still held, and their work is not in the repository.** Both were dispatched on 2026-09-27 to `feat/ORI-T-0039-memory` and `feat/ORI-T-0040-memory`. Neither branch exists on the remote today, no pull request names either ticket, and the only remote commits whose `Ticket:` trailer names either one are two lead records under `ops/` (27b8833 and e12e31e, both `ORI-T-0040`), neither of which changes a file under `crates/`. Whatever was built under them, including the targeted repair round that extended claim 61 to `barrier.rs`, was never pushed. Both tickets are dispatched again from `main` at 7bed207, under the same claims and the same modules. Claim 61's extension to `barrier.rs` stands, additively only: a coder that needs it says so in its report and that change is tier 2 ([[R38]] point 3).

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 63 | ORI-T-0045, the prover: the planted-defect demonstration | `crates/ori-gates/src/prover.rs`, `crates/ori-gates/src/lib.rs` (queue) | 2, `ops/phase-1-backlog.md` batch 7 | `feat/ORI-T-0045-prover` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Checked against every claim held once this section is read: 20 (`.github/workflows/ci.yml`, `scripts/gates.sh`, `fixtures/planted/gate-13/**`), 61 (`oplog.rs`, `retrieval.rs`, `barrier.rs` additively) and 62 (`citation.rs`, `drift.rs`, `freshness.rs` additively). Claim 63 is in `crates/ori-gates/` and is disjoint from all three. Claims 61 and 62 share only the queue file `crates/ori-memory/src/lib.rs`, which neither holds.

**Where the work lands.** This session runs in a cloud container whose standing instruction is to develop on one branch, `claude/sweet-maxwell-jkaiie`, and to open no pull request unless the operator asks. Each ticket is still built on its own branch, in its own worktree, by its own coder; those branches stay local. The lead reviews each one and brings it onto `claude/sweet-maxwell-jkaiie` as rebased commits that keep the coder's `Ticket:` and `Spec:` trailers. Nothing reaches `main` from this session without a pull request the operator asks for, so a tier 2 change on that branch is merged by the operator, as [[R38]] requires.

**State after this section:** the claims held are 20, 61, 62 and 63. The next free identifier is ORI-T-0126.

---

## Claim 64, and ORI-T-0126 allocated

Written by the lead at dispatch, 2026-10-11, under [[R31]].

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 64 | ORI-T-0054, the tree watcher and the git hooks installer | `crates/ori-watch/src/watcher.rs`, `crates/ori-watch/src/hooks.rs`, `crates/ori-watch/Cargo.toml` (`notify`, approved in [[E-0009]], and the workspace crates the two modules need), `crates/ori-watch/src/lib.rs` (queue), `Cargo.lock` (queue) | 1, `spec/RISK_MAP.md` (`ori-watch` watcher, hooks) | `feat/ORI-T-0054-watch` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Disjoint from claims 20, 61, 62 and 63: nothing else has claimed a path under `crates/ori-watch/`. `Cargo.lock` is a queue file, as in claims 32 to 59.

| Ticket | Defect | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0126 | Five tests fail whenever the suite runs as root, because each makes a path mode 000 and asserts the process can no longer read it: `code_map::tests::ori_t_0036_the_files_seen_invariant_holds_with_every_pre_skip_reason_nested`, and in `indexer::tests` `ori_p1_026_a_divergence_on_a_file_the_walk_leaves_out_stays_listed_with_the_reason`, `ori_p1_026_entries_that_share_a_key_list_every_file_and_every_reason`, `ori_p1_026_with_no_record_an_unreadable_file_is_freshness_unknown_and_not_ready`, `ori_t_0035_a_walk_splits_and_reads_no_more_than_its_budget_and_records_what_it_left_out`. Seen by the lead on 2026-10-11 in a container that runs as root; CI does not run as root and passed all five on `main` at 7bed207. A root-run suite is red for a reason that is not a defect in the code under test, and the cure is a test that recognises it cannot exercise the unreadable case and says so | `crates/ori-memory/src/code_map.rs`, `crates/ori-memory/src/indexer.rs` (their tests) | 1, `spec/RISK_MAP.md` (code map, indexer) | **waiting on the operator**: the fix changes existing tests, trigger `test_modified` | 2026-10-11 |

**State after this section:** the claims held are 20, 61, 62, 63 and 64. The next free identifier is ORI-T-0127.

---

## Session of 2026-10-11: the operator's answers, and ORI-T-0127 to ORI-T-0129 allocated

Written by the lead on 2026-10-11 under [[R31]] rule 1. The operator answered four open questions in session the same day, each with the option the lead recommended:

| Question | Answer | Recorded in |
|---|---|---|
| [[E-0002]], where the adapter traits live | Option A: in `ori-core` | E-0002's State line |
| [[E-0007]], what the lead's "merge tier 0" means | Reading 1: authority; the lead holds no merge credential | E-0007's State line |
| `test_modified` for ORI-T-0125 and ORI-T-0126 | Both authorized, on a condition | [[R40]] |
| ORI-P1-031 asks for a ticket move the machine refuses (recorded under E-0007) | Add the edge `InProgress --> Queued` to `spec/DATA_MODEL.md` section 3 | E-0007's State line |

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0127 | State E-0002 option A in the specification: the adapter traits of `spec/API_SPEC.md` section 4 are declared in `ori-core` and implemented in `ori-integrations`, and `ori-orchestrator` names `dyn VcsHost` without a new edge | `spec/LLD.md` section 2, `spec/API_SPEC.md` section 4 | 1, `spec/RISK_MAP.md` (`spec/`) | ready; documentation role | 2026-10-11 |
| ORI-T-0128 | Draw `InProgress --> Queued` in the Ticket state machine, labelled for engine recovery, so ORI-P1-031's "ticket Queued" is a move the machine allows | `spec/DATA_MODEL.md` section 3 | 1, `spec/RISK_MAP.md` (`spec/`) | ready; documentation role | 2026-10-11 |
| ORI-T-0129 | Add that edge to `crates/ori-core/src/ticket.rs`'s transition table, after ORI-T-0128 merges | `crates/ori-core/src/ticket.rs` | 2, `spec/RISK_MAP.md` (`ori-core` state machines) | waits on ORI-T-0128 | 2026-10-11 |

**Unblocked by these answers:** ORI-T-0029, the forbidden-action test harness (`crates/ori-broker/src/forbidden.rs`, tier 2), no longer waits on E-0007. ORI-T-0058, the adapter traits, now builds in `ori-core` rather than at `crates/ori-integrations/src/traits.rs` as `ops/phase-1-backlog.md` batch 11 declares, and waits on ORI-T-0127 so that the code follows the specification rather than the other way round. ORI-T-0125 and ORI-T-0126 no longer wait on the operator and run one at a time against `code_map.rs` and `indexer.rs` with the other follow-ups there.

**State after this section:** the claims held are 20, 61, 62, 63 and 64. The next free identifier is ORI-T-0130.

---

## Claims 65 and 66

Written by the lead at dispatch, 2026-10-11, under [[R31]]. Both are documentation-role tickets and write only under `spec/`.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 65 | ORI-T-0127, E-0002 option A in the specification | `spec/LLD.md`, `spec/API_SPEC.md` | 1, `spec/RISK_MAP.md` (`spec/`) | `docs/ORI-T-0127-adapter-traits-in-core` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 66 | ORI-T-0128, the recovery edge `InProgress --> Queued` | `spec/DATA_MODEL.md` | 1, `spec/RISK_MAP.md` (`spec/`) | `docs/ORI-T-0128-recovery-edge` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Disjoint from each other and from claims 20 and 61 to 64, none of which holds a path under `spec/`.

**State after this section:** the claims held are 20 and 61 to 66. The next free identifier is ORI-T-0130.

---

## Claim 66 released; claims 67 and 68; what ORI-T-0129 inherits

Written by the lead on 2026-10-11 under [[R31]] rules 2 and 3.

**Claim 66 released.** ORI-T-0128 is on `claude/sweet-maxwell-jkaiie` as 3298433, reviewed by the lead: three lines added to `spec/DATA_MODEL.md` section 3, the edge and one paragraph saying it is the engine's recovery move only, in the order of `spec/runbooks/recover-engine.md` step 1. `cargo test -p ori-gates` passed (138), so every reference it adds resolves. It is not on `main`.

**What ORI-T-0129 inherits from ORI-T-0128's report**, so its ticket is written from this rather than rediscovered:

1. `crates/ori-core/src/ticket.rs`'s test module hard-codes the diagram: fourteen lines in `DIAGRAM`, `DIAGRAM_EDGES = 14`, and `ALL_PAIRS - permitted == 130`. The new edge makes them fifteen and 129. **That is a change to an existing test**, so ORI-T-0129 needs the operator's `test_modified` answer before it starts, the shape of [[R37]] and [[R40]].
2. "The engine's move and no one else's" has no code behind it yet. `TicketEventKind::Queued` serves both existing edges into `Queued`, and `crates/ori-orchestrator/src/lifecycle.rs` gates no verb on the new edge, so nothing distinguishes a recovery requeue from any other `Queued` event. If distinguishing it needs a new event kind, that is `contract_change`.
3. Which `Event.actor` the recovery requeue carries is unspecified: `spec/DATA_MODEL.md` section 4 allows `system` "only for scheduled triggers and watchers", and engine recovery is neither. A specification gap, not decided here.
4. Out of ORI-T-0129's file but in its path: `crates/ori-runtime/src/recovery.rs` describes the edge as missing (its module doc, and a test fixture string), and says `Queued --> InProgress` is the only edge into `InProgress`, which is false: three edges enter it.

ORI-T-0129 stays waiting: on the operator for item 1, and on a decision for items 2 and 3.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 67 | ORI-T-0126, the five root-only tests | the `#[cfg(test)]` modules of `crates/ori-memory/src/code_map.rs` and `crates/ori-memory/src/indexer.rs`: the five tests [[R40]] names and test-only helpers | 1 | `fix/ORI-T-0126-0125-tests` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 68 | ORI-T-0125, the three load-sensitive deadline tests | the `#[cfg(test)]` module of `crates/ori-memory/src/code_map.rs`: the three tests [[R40]] names and test-only helpers | 1 | the same branch, after claim 67's commits | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Claims 67 and 68 share a file with each other, so one coder runs them in order on one branch, each commit carrying only its own `Ticket:` trailer. Neither overlaps claim 61 or 62: those hold `oplog.rs`, `retrieval.rs`, `citation.rs`, `drift.rs`, and `barrier.rs` and `freshness.rs` additively, never `code_map.rs` or `indexer.rs`.

**State after this section:** the claims held are 20, 61 to 65, 67 and 68. The next free identifier is ORI-T-0130.

---

## Session of 2026-10-11: claim 65 released, three more answers, ORI-T-0058 redefined, claims 69 to 76

Written by the lead at dispatch, 2026-10-11, under [[R31]].

**Claim 65 released.** ORI-T-0127 is on `claude/sweet-maxwell-jkaiie` as 7f421ee (cherry-picked from 5926f63), reviewed by the lead: `spec/LLD.md` and `spec/API_SPEC.md` say the seven adapter traits and the types in their signatures are declared in `ori-core`, the five slot traits implemented in `ori-integrations` and `AgentRuntime` in `ori-runtime`. `cargo test -p ori-gates` passed (138). Its agent found [[E-0002]]'s premise false, and the operator, asked again, chose all seven with their types (E-0002's State line). The text it wrote already says that, so it stands. What it reported and did not change, each now a follow-up for the lead to ticket: `spec/ARCHITECTURE.md`'s `integrations` row and `spec/ROADMAP.md` (lines 9 and 72) still put the traits in `ori-integrations`; `spec/RISK_MAP.md` (tier 2) has no row for the `ori-core` file that will declare `VcsHost::merge`; nothing mechanical enforces "only `merge_queue` calls `merge`" now that every crate can name the trait; and [[R8]]'s `INT --> STORE` edge is still drawn though `ori-integrations` must not read the store.

**Three more answers from the operator, 2026-10-11:**

| Question | Answer | Recorded in |
|---|---|---|
| `test_modified` for ORI-T-0129 | Authorized, on a condition | [[R41]] |
| Which `Event.actor` the recovery requeue carries | Widen `system` in `spec/DATA_MODEL.md` section 4 to the engine's restart recovery | ORI-T-0130 below |
| `ops/phase-1-backlog.md` open escalation 2: move the fixtures forward | Yes | that row of the backlog; claims 72 to 75 |

**ORI-T-0058 redefined.** `ops/phase-1-backlog.md` batch 11 declares it as `crates/ori-integrations/src/traits.rs`, tier 1. Under the operator's answer it declares the seven traits of `spec/API_SPEC.md` section 4 in `ori-core` and moves into `ori-core` the types their signatures name, `AgentIdentity` from `ori-broker` and `SessionSpec`, `SessionHandle` and `RuntimeCaps` from `ori-runtime` among them, with the old paths kept as re-exports so that no caller and no existing test changes. Moving `AgentIdentity` changes `crates/ori-broker/src/identity.rs`, which `spec/RISK_MAP.md` tiers 2, so ORI-T-0058 is **tier 2**.

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0130 | `spec/DATA_MODEL.md` section 4: `system` is allowed for scheduled triggers, watchers, and the engine's restart recovery (`spec/runbooks/recover-engine.md`), which ORI-T-0128's recovery edge needs an actor for | `spec/DATA_MODEL.md` | 1 | dispatched, claim 71 | 2026-10-11 |

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 69 | ORI-T-0058, the adapter traits and their types in `ori-core` | `crates/ori-core/src/adapters.rs` (new), `crates/ori-core/src/lib.rs` (queue); the declarations it moves, in `crates/ori-broker/src/identity.rs` and `crates/ori-runtime/src/acp.rs`, each replaced by a re-export; nothing else in either crate | 2 | `feat/ORI-T-0058-adapter-traits` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 70 | ORI-T-0129, the recovery edge in the ticket machine | `crates/ori-core/src/ticket.rs` (its table, its docs, and its test module under [[R41]]); `crates/ori-runtime/src/recovery.rs` (its module doc and its docs only, where they call the edge missing) | 2 | `feat/ORI-T-0129-recovery-edge` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 71 | ORI-T-0130, `system` as the recovery actor | `spec/DATA_MODEL.md` | 1 | `docs/ORI-T-0130-recovery-actor` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 72 | ORI-T-0073, `fixtures/new-product` | `fixtures/new-product/**` | 1 | `feat/ORI-T-0073-fixture-new-product` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 73 | ORI-T-0074, `fixtures/migrated-with-drift` | `fixtures/migrated-with-drift/**` | 1 | `feat/ORI-T-0074-fixture-migrated-with-drift` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 74 | ORI-T-0075, `fixtures/inert-gate` | `fixtures/inert-gate/**` | 1 | `feat/ORI-T-0075-fixture-inert-gate` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 75 | ORI-T-0076, `fixtures/looping-agent` | `fixtures/looping-agent/**` | 1 | `feat/ORI-T-0076-fixture-looping-agent` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 76 | ORI-T-0046, the liveness definition | `crates/ori-gates/src/liveness.rs` (new), `crates/ori-gates/src/lib.rs` (queue) | 2, `spec/RISK_MAP.md` (`prover.rs, liveness.rs`) | `feat/ORI-T-0046-liveness` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Checked against every claim held: 20, 61 to 64, 67 and 68. Claims 69 and 70 share only `crates/ori-core/src/lib.rs`, a queue file neither holds; 69's `acp.rs` and 70's `recovery.rs` are different files of `ori-runtime`, and nothing else holds either. Claim 71 is the only claim under `spec/`. Claims 72 to 75 are four disjoint new directories, and none is under `fixtures/planted/`. Claim 76 shares only the queue file `crates/ori-gates/src/lib.rs` with claim 63.

**State after this section:** the claims held are 20, 61 to 64 and 67 to 76. The next free identifier is ORI-T-0131.

---

## Claim 71 released; follow-ups ORI-T-0131 to ORI-T-0139 allocated

Written by the lead on 2026-10-11 under [[R31]].

**Claim 71 released.** ORI-T-0130 is on `claude/sweet-maxwell-jkaiie` as 8f8a4cd, reviewed by the lead: `spec/DATA_MODEL.md` section 4 admits `system` for "the engine's recovery after a restart (`spec/runbooks/recover-engine.md`)", and section 3's recovery paragraph records the requeue and the events around it with actor `system`. `cargo test -p ori-gates` passed (138). This closes item 3 of "what ORI-T-0129 inherits".

**Follow-ups**, each stated as the defect, from the reports and reviews of ORI-T-0045, ORI-T-0127 and ORI-T-0130. None is claimed; each claim is written at dispatch.

| Ticket | Defect | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0131 | A `GateProof` is not sealed: `GateProof::new` and `GateDef::with_proof` are public and a proof carries no gate number, so code that never ran the prover can make `Registry::list` report `Installed`. `runner.rs`'s own tests do so. "Only `Registry` produces `Installed`" holds; "`Installed` implies a demonstration ran" is a convention. Found by ORI-T-0045's coder, confirmed by its reviewer | `crates/ori-gates/src/gate.rs` | `spec/RISK_MAP.md` does not name `gate.rs`; the lead recommends 2, because it decides what may be called installed; the operator sets it | ready | 2026-10-11 |
| ORI-T-0132 | ORI-P1-013's "proof stored with evidence refs" has no owner: no ticket in `ops/phase-1-backlog.md` builds the gate projection, the `gates.prove` method's wiring, or a durable proof and revocation event, and `Registry` has no replace or remove for the runbook's rollback. ORI-P1-013 is not satisfied by ORI-T-0045 alone | backlog gap: `ori-store` projections, `ori-rpc`, `ori-gates` | to be set when planned | **needs planning** | 2026-10-11 |
| ORI-T-0133 | `gate.proven` is named in `spec/LLD.md` section 4 and `spec/design/DESIGN.md`, and by ORI-T-0045's constant, but not in `spec/API_SPEC.md` section 2's event list; no event kind exists for revoking a proof | `spec/API_SPEC.md` | 1 | ready; documentation role | 2026-10-11 |
| ORI-T-0134 | `crates/ori-gates/src/lib.rs` has misattached `///` docs: `modified_tests` carries two stacked docs, one describing `sections`; `significance` carries the `spec_refs` doc; `sections` and `spec_refs` carry none | `crates/ori-gates/src/lib.rs` | 1 | ready, after claims 63 and 76 release the queue file | 2026-10-11 |
| ORI-T-0135 | Two more `code_map` timing tests fail under machine load and pass alone: `ori_t_0036_a_spec_document_of_long_lines_cannot_overrun_the_scan_deadline`, `ori_t_0036_python_relative_import_work_is_linear_in_dots_plus_names`. Seen by ORI-T-0045's coder at load average about 5 | `crates/ori-memory/src/code_map.rs` (its tests) | 1 | **waiting on the operator**: `test_modified`, not covered by [[R40]] | 2026-10-11 |
| ORI-T-0136 | After [[E-0002]], `spec/ARCHITECTURE.md`'s `integrations` row and `spec/ROADMAP.md` (lines 9 and 72) still put the adapter traits in `ori-integrations` | `spec/ARCHITECTURE.md`, `spec/ROADMAP.md` | 1 | ready; documentation role | 2026-10-11 |
| ORI-T-0137 | After [[E-0002]], `spec/RISK_MAP.md` has no row for the `ori-core` file that declares `VcsHost::merge`, so the merge signature moved from a tier 2 path to an untiered one; `spec/SECURITY_NOTES.md` and `spec/adr/ADR-0002-single-operator.md` name only `ori-integrations::*::merge` | `spec/RISK_MAP.md`, `spec/SECURITY_NOTES.md`, `spec/adr/ADR-0002-single-operator.md` | 2 | ready; documentation role; the operator merges | 2026-10-11 |
| ORI-T-0138 | Nothing mechanical enforces `CLAUDE.md`'s "Only `ori-orchestrator::merge_queue` calls `VcsHost::merge`", and since [[E-0002]] every crate can name the trait | a test or gate, location to be decided | to be set; the lead recommends 2 (merge path) | **needs planning** | 2026-10-11 |
| ORI-T-0139 | Code docs still quote the old rule "`system` only for scheduled triggers and watchers", some arguing from it: `crates/ori-core/src/types.rs`, `crates/ori-core/src/ticket.rs` (`by_human`), `crates/ori-store/src/event_log.rs`, `crates/ori-memory/src/barrier.rs`, `crates/ori-memory/src/scope.rs`. No behaviour changes; each conclusion was checked by ORI-T-0130's agent and still holds | those five files, docs only | 2: four of the five are tier 2 paths | ready, after claims 61 and 70 release `barrier.rs` and `ticket.rs` | 2026-10-11 |

**State after this section:** the claims held are 20, 61 to 64, 67 to 70 and 72 to 76. The next free identifier is ORI-T-0140.

---

## Claims 63 and 74: integrated on the session branch

Written by the lead on 2026-10-11 under [[R31]] rule 3. "Integrated" means on `claude/sweet-maxwell-jkaiie`, reviewed and verified there; it is not on `main`. A claim is released here when its work is integrated, because nothing else in this session will touch its modules; it is released on `main` only when the operator merges.

| # | Ticket | Integrated as | Review | Verified by the lead on the session branch | Tier, and who merges to `main` |
|---|---|---|---|---|---|
| 63 | ORI-T-0045, the prover | 4639bc3, 31274fa (cherry-picked from 4fe2554, 8e0ee60) | Independent review on Opus 5.5: nothing blocking, two should-fix (a stale proof could be attached to a changed gate; a false claim in `revoke`'s doc) and four notes, all repaired in attempt 2 and re-checked by the lead | `cargo test -p ori-gates`: 161 passed; `cargo clippy -p ori-gates --all-targets -D warnings`: clean | **2**: the operator |
| 74 | ORI-T-0075, `fixtures/inert-gate` | 9be53c7 (from e6aa6c5) | The lead: a valid workflow whose only triggers name a branch the fixture never has, and a liveness record documented in its README | `cargo test -p ori-gates`: 138 passed before the prover landed; `scripts/secret-scan.sh`: no finding, 378 files, 670 history blobs | 1 |

**Two lead errors the ORI-T-0075 coder found, recorded because they reached four tickets.** The fixture tickets' standing facts offered the commit type `test(fixtures):`, which gate 13 refuses (this repository's types are `feat`, `fix`, `docs`, `ci` and `chore`); and they told the fixture and documentation agents to share one cargo target directory, `/home/user/wt/shared-target`. `ori-gates`' repository-wide tests bake `CARGO_MANIFEST_DIR` in at compile time and cargo reused a test binary built in another worktree, so a test run there scans the other worktree's tree and can report green on a tree that does not contain the files under review. The coder saw 32 failures pointing at a removed worktree, which is how it surfaced. The three fixture coders still running were corrected in flight. ORI-T-0130's result was re-established by the lead from its own target directory on the session branch (138 passed). The repository clone was also shallow, so `scripts/secret-scan.sh` refused in every worktree; the lead unshallowed it.

**ORI-P1-013 is not satisfied by ORI-T-0045.** "Proof stored with evidence refs" has no owner: ORI-T-0132.

**State after this section:** the claims held are 20, 61, 62, 64, 67 to 70, 72, 73, 75 and 76. The next free identifier is ORI-T-0140.

---

## Claims 72 and 75 integrated; ORI-T-0140 allocated and claimed (77)

Written by the lead on 2026-10-11 under [[R31]].

| # | Ticket | Integrated as | Verified by the lead on the session branch |
|---|---|---|---|
| 72 | ORI-T-0073, `fixtures/new-product` | cherry-picked from 3a9cf82 | `cargo test -p ori-gates`: 161 passed; `scripts/secret-scan.sh`: no finding |
| 75 | ORI-T-0076, `fixtures/looping-agent` | cherry-picked from bb5be9c | `cargo test -p ori-gates`: 161 passed; `scripts/secret-scan.sh`: no finding; the fixture's own `selftest.sh`: 41 passed |

**A defect in merged code, found by ORI-T-0076's coder and confirmed by the lead by reading.** `crates/ori-runtime/src/headless.rs`, `send`: `session.stdout.read_to_end(&mut output)` (line 276) runs before `wait_with_timeout(&mut session.child, RUN_TIMEOUT)` (line 277). `read_to_end` returns only when every holder of the pipe's write end has closed it, so a child that keeps its stdout open, or leaves a descendant holding it, blocks `send` indefinitely and the 20 second kill never starts. The coder measured it with a scratch crate against the real adapter: `sh -c 'sleep 23'` returned `Ok(EndTurn)` after 23.0 s with no `Timeout` and no defect recorded. The module doc promises the bound. A looping or hung agent is exactly what budgets and the liveness of the engine exist for, so this is a defect in a control, not a performance note.

| Ticket | Defect | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0140 | The headless run's `RUN_TIMEOUT` does not bound a child that holds its stdout open: `send` reads to end before it starts the clock | `crates/ori-runtime/src/headless.rs` | 1, `spec/RISK_MAP.md` (`ori-runtime` headless) | dispatched, claim 77 | 2026-10-11 |

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 77 | ORI-T-0140 | `crates/ori-runtime/src/headless.rs` | 1 | `fix/ORI-T-0140-headless-timeout` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Disjoint from claim 69 (`acp.rs`, `identity.rs`, `ori-core`) and claim 70 (`ticket.rs`, `recovery.rs`). If the same pattern exists in `crates/ori-runtime/src/acp.rs`, claim 69 holds that file, so ORI-T-0140 reports it and does not change it.

**State after this section:** the claims held are 20, 61, 62, 64, 67 to 70, 73, 76 and 77. The next free identifier is ORI-T-0141.

---

## Claim 73 integrated; claim 69 extended; ORI-T-0141 and ORI-T-0142 allocated

Written by the lead on 2026-10-11 under [[R31]].

**Claim 73 integrated.** ORI-T-0074, `fixtures/migrated-with-drift`, cherry-picked from 300dce5. Verified on the session branch: not a member of this workspace (`cargo metadata`: 17 packages, no `parceltrack`), `cargo build --workspace --locked` passes, `cargo test -p ori-gates` 161 passed, `scripts/secret-scan.sh` no finding with its 16 new declared fakes. The lead did not read the fixture's `.env` (`CLAUDE.md` rule 4); the scanner did.

**Claim 69 extended**, under [[R42]]: `crates/ori-memory/src/scope.rs`, the one line of `ori_t_0038_identity_payload_layout_and_rules_match_ori_broker_registration` that names the file it reads. Nothing else in that file. No other claim holds `scope.rs`: claim 60 was released when pull request 77 merged.

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0141 | Move `AgentRuntime` and the types its signatures need (`SessionSpec`, `SessionHandle`, `RuntimeCaps`, `StopReason`, `LaunchConfig`, `StdioMode`, `RuntimeError`, `transcript::Entry`, `TranscriptError`) into `ori-core`, re-exported at their old paths, completing [[E-0002]]'s answer. ORI-T-0058's report section 1 lists what a re-export cannot preserve | `crates/ori-core/src/adapters.rs`, `crates/ori-runtime/src/acp.rs`, `transcript.rs`, `headless.rs` | 1 by `spec/RISK_MAP.md` (`ori-core` other types; `ori-runtime` acp, headless, transcript) | waits on claim 77 (ORI-T-0140) releasing `headless.rs`, and on claim 69 releasing `adapters.rs`; its two test changes are authorized by [[R42]] | 2026-10-11 |
| ORI-T-0142 | `spec/DATA_MODEL.md` says `InProgress --> Queued` is "the engine's move and no one else's", and nothing enforces it: `Ticket::apply` accepts it from any actor, and `crates/ori-orchestrator/src/lifecycle.rs`'s `verb()` gates no AICD §17 verb on it. ORI-T-0129's coder proposes a guard on `Actor::System` in `Ticket::apply` (no new event kind, which the `ori-store` projector would ignore), at the cost of changing `ori_t_0020_apply_accepts_exactly_the_pairs_the_diagram_draws`; `system` is also the actor of triggers and watchers, so the guard is wider than the sentence | `crates/ori-core/src/ticket.rs`; possibly `lifecycle.rs` | 2 | **waiting on the operator**: `test_modified` | 2026-10-11 |

**State after this section:** the claims held are 20, 61, 62, 64, 67 to 70, 76 and 77. The next free identifier is ORI-T-0143.

---

## R43: ORI-T-0142 folded into ORI-T-0129

Written by the lead on 2026-10-11. Under [[R43]] the actor guard ORI-T-0142 described is built in ORI-T-0129, under claim 70, which already holds `crates/ori-core/src/ticket.rs`. ORI-T-0142 is closed into ORI-T-0129 and keeps its identifier as a record. What stays open from its row: a role or verb check for the move in `crates/ori-orchestrator/src/lifecycle.rs` (tier 2), allocated when it is planned.

**State after this section:** unchanged. The claims held are 20, 61, 62, 64, 67 to 70, 76 and 77. The next free identifier is ORI-T-0143.

---

## Claim 69 narrowed; ORI-T-0143 allocated

Written by the lead on 2026-10-11 under [[R31]].

**Claim 69 narrowed.** ORI-T-0058 moved no declaration out of `crates/ori-runtime/src/acp.rs` ([[R42]] staged that as ORI-T-0141), and its branch does not change the file (`git diff --stat e095d4f..HEAD` on `feat/ORI-T-0058-adapter-traits` lists five files, none in `ori-runtime`). `acp.rs` is released from claim 69.

**A defect in merged code, found by ORI-T-0140's coder by reading and confirmed by the lead by reading.** `crates/ori-runtime/src/acp.rs`, `AcpClient::call`: the loop checks `Instant::now() > deadline` only between frames, then calls `read_line`, which does `BufReader<ChildStdout>::read_line` and blocks until a newline or end of file. An agent process that stays alive and silent, or writes a partial line, blocks `call` indefinitely, and `IO_TIMEOUT` never fires. It is the same class as ORI-T-0140, in the ACP path rather than the headless one.

| Ticket | Defect | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0143 | `AcpClient::call`'s `IO_TIMEOUT` does not bound a child that stays silent or holds a partial line: the deadline is checked only between frames, around a blocking `read_line` | `crates/ori-runtime/src/acp.rs` | 1, `spec/RISK_MAP.md` (`ori-runtime` acp) | ready; dispatched after ORI-T-0140's review settles the shape of the fix; ORI-T-0141 waits on it, since both change `acp.rs` | 2026-10-11 |

**State after this section:** the claims held are 20, 61, 62, 64, 67 to 70 (69 narrowed), 76 and 77. The next free identifier is ORI-T-0144.

---

## Claims 61 and 76 integrated on the session branch

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`.

| # | Ticket | Integrated as | Review | Verified by the lead on the session branch | Tier, who merges to `main` |
|---|---|---|---|---|---|
| 61 | ORI-T-0039, the operational log and the retrieval package | d517576 to 0c8a6ff, seven commits (from b3b8055 ... 26a6f63) | Independent review on Opus 5.5: changes requested, one blocking (the "cited by" line of a module item read spec document paths and heading text from the code map beside the admitted candidate, around `Authorization::filter`, contradicting the module's own doc) and four should-fix (a record's ordinal among all records disclosed out-of-scope counts; a divergence filed against a file did not reach its sections; the freshness corpus was read before the search; a code-map-incomplete note counted files the map never parses). All repaired in attempt 2; the lead confirmed the blocking one by reading: `module_lines` now takes only the admitted module | `cargo test -p ori-memory -- oplog retrieval`: 40 passed; `cargo clippy -p ori-memory --all-targets -D warnings`: clean; `cargo test -p ori-gates`: 161 passed; strict `cargo doc`: clean | 1: the lead may merge under [[R38]]; it rides with the session branch |
| 76 | ORI-T-0046, the liveness definition | three commits (from 4f1b19d, 3d627a5, 9660b61) | Independent review on Opus 5.5: nothing blocking; one should-fix (no test of an installed gate that ran and went quiet, `Inert(TooOld)`, or one with only future-dated runs), repaired in attempt 2 with the refactor the review described shown to fail exactly those two tests | `cargo test -p ori-gates`: 190 passed; clippy clean | **2**: the operator |

`barrier.rs` was not touched by ORI-T-0039, so claim 61's extension to it is released unused. The review's note N10, that `barrier.rs` parses an evidence blob's `content_ref` from the log unvalidated and `EvidenceReply` hands it to a caller to read, is a defect in `barrier.rs`, not in this ticket:

| Ticket | Defect | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0144 | An evidence blob's `content_ref` is parsed from the `memory.evidence_stored` event and handed out unvalidated, so a planted event names any absolute path, and the first caller that reads evidence by reference reads that file. It should be resolved under the product's evidence directory and refused otherwise | `crates/ori-memory/src/barrier.rs` | 2, `spec/RISK_MAP.md` (`barrier.rs`) | ready | 2026-10-11 |

**State after this section:** the claims held are 20, 62, 64, 67 to 70 and 77. The next free identifier is ORI-T-0145.

---

## ORI-T-0137 widened and claimed (78)

Written by the lead at dispatch, 2026-10-11, under [[R31]].

ORI-T-0058's independent review found a governance defect the ticket created: `AgentIdentity` and its creation path, `ScopedToken` and `VcsHost::merge` now live in `crates/ori-core/src/adapters.rs` and `crates/ori-core/src/adapters/identity.rs`, which `spec/RISK_MAP.md` tiers 1 ("crates/ori-core (other types)"), while they came from `crates/ori-broker`, tier 2 ("Credentials"), and `spec/SECURITY_NOTES.md` "Tier 2 modules" lists `ori-broker` (all) for "Credentials and identities". `spec/adr/ADR-0002-single-operator.md` says no module is moved out of tier 2 to avoid it. ORI-T-0058 does not merge before this is answered.

ORI-T-0137 already held the half of this about `VcsHost::merge`. It is widened to the whole: tier both `ori-core` adapter files at 2 in `spec/RISK_MAP.md` and list them in `spec/SECURITY_NOTES.md`'s tier 2 table, and correct any statement in `spec/adr/ADR-0002-single-operator.md` the move makes false. All three are tier 2 documents, so the operator merges it.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 78 | ORI-T-0137, the adapter files tiered 2 | `spec/RISK_MAP.md`, `spec/SECURITY_NOTES.md`, `spec/adr/ADR-0002-single-operator.md` | 2 | `docs/ORI-T-0137-adapters-tier-2` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

No other claim holds a path under `spec/`. Claim 69 is noted to include `crates/ori-core/src/adapters/identity.rs`, which ORI-T-0058's split clause allowed and the review asked to be recorded (its note N8).

**State after this section:** the claims held are 20, 62, 64, 67 to 70, 77 and 78. The next free identifier is ORI-T-0145.

---

## Claim 79: ORI-T-0143 dispatched

Written by the lead at dispatch, 2026-10-11, under [[R31]]. ORI-T-0140's independent review approved its shape (a reader thread feeding a channel, one deadline over the whole run) and confirmed the ACP defect by reading: `AcpClient::call` checks its deadline only between lines around a blocking `read_line`, and `write_frame` can also block on a full stdin pipe.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 79 | ORI-T-0143, ACP read and write bounded by `IO_TIMEOUT` | `crates/ori-runtime/src/acp.rs` | 1 | `fix/ORI-T-0143-acp-timeout` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Disjoint from claim 77 (`headless.rs`). Claim 69 no longer holds `acp.rs` (narrowed above). ORI-T-0141, which also changes `acp.rs`, waits on this claim's release.

**State after this section:** the claims held are 20, 62, 64, 67 to 69, 77, 78 and 79. The next free identifier is ORI-T-0145.

---

## ORI-T-0145 allocated and claimed (80), paired with ORI-T-0137

Written by the lead at dispatch, 2026-10-11, under [[R31]].

ORI-T-0137's documentation agent found that its new `spec/RISK_MAP.md` row turns `cargo test -p ori-gates` red: `crates/ori-gates/src/significance.rs` (gate 12) restates RISK_MAP's tier 2 rows in `RISK_MAP_TIER_TWO`, and `significance::tests::ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file` compares the restatement with the file. The two must land together, so the code half is ORI-T-0145, built on ORI-T-0137's branch, the shape of ORI-T-0123 and ORI-T-0124. It adds a row to a constant, not to a test, so no `test_modified` arises. Until it lands, gate 12 would not count the adapter files as tier 2.

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0145 | Restate the new tier 2 row (`crates/ori-core/src/adapters.rs, adapters/identity.rs`) in `RISK_MAP_TIER_TWO` | `crates/ori-gates/src/significance.rs` | 1 by `spec/RISK_MAP.md` (`ori-gates` runners); carried with ORI-T-0137, so the pull request is tier 2 | dispatched, claim 80 | 2026-10-11 |

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 80 | ORI-T-0145 | `crates/ori-gates/src/significance.rs` | 1 | `docs/ORI-T-0137-adapters-tier-2` (with claim 78) | 2026-10-11 | **held** | 2026-10-11, at dispatch |

No other claim holds `significance.rs`; claim 42 (ORI-T-0044) released when pull request 52 merged.

Two records corrected now, both found by ORI-T-0137's agent: `ops/phase-1-backlog.md` batch 11's row for ORI-T-0058 still declared `crates/ori-integrations/src/traits.rs` at tier 1 (corrected in place, struck through, to the paths and tier E-0002's answer gave it); and ORI-T-0141, recorded above as tier 1, becomes **tier 2** once ORI-T-0137 merges, because it changes `crates/ori-core/src/adapters.rs`.

**State after this section:** the claims held are 20, 62, 64, 67 to 69, 77 to 80. The next free identifier is ORI-T-0146.

---

## Claims 62, 67, 68 and 77 integrated on the session branch

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`; tier 1 unless stated.

| # | Ticket | Review, and what was repaired | Verified by the lead on the session branch |
|---|---|---|---|
| 62 | ORI-T-0040, the citation checker and the drift audit | Changes requested, one blocking, **reproduced** by the reviewer: the audit's routine verification erased a divergence filed by someone else, and a path divergence overwrote it, so readiness turned true without a human seeing the finding. Repaired by giving every divergence an origin (audit, filed, unattributed) in `freshness.rs` (additively, claim 62 widened), the audit clearing only its own, and only a human resolving a filed finding. Should-fix: vacuous verification of documents with no path claim (now `NothingChecked`, freshness left as it was); readers that followed links outside the root; a check over only excluded documents read as clean; gate-reader evasions (a non-breaking space, a trigger word); directories not counted against the walk limit. All repaired; the lead confirmed the blocking repair by reading `record_audit_verification` and the audit's calls | `cargo test -p ori-memory`: 466 passed; `cargo test -p ori-gates`: 190 passed; workspace clippy, strict doc, fmt: clean |
| 67, 68 | ORI-T-0126 then ORI-T-0125, eight tests under [[R40]] | R40's condition holds for all eight on an ordinary non-root runner (the reviewer emulated one with `setpriv` and, for the capability case, ran as uid 65534 with `CAP_DAC_OVERRIDE`). Two should-fix, repaired: the code-map test took its "mode does not apply" branch without checking for root; a regression would have failed under a message blaming load. **Deviation from the ticket, accepted by the lead and recorded here:** `ori_t_0036_retained_heading_text_is_bounded_for_many_modules_citing_long_headings` now runs with a 3600 s scan budget. The ticket said to stop if the only fix was a wider bound; the review established the test never asserted a deadline (its five seconds were an incidental precondition of a complete scan, and the deadline is proven by other tests), so R40's "never widen a bound until it bounds nothing" is not engaged | `cargo test -p ori-memory` **as root: 466 passed, 0 failed**, the five root-only failures gone |
| 77 | ORI-T-0140, the headless deadline | Approved, no blocking. One should-fix repaired: the abandoned reader drained a continuously writing descendant at full speed after the deadline; now a shared flag stops it and the writer gets EPIPE. The two waits share one deadline | `cargo test -p ori-runtime`: 163 passed |

ORI-T-0140's coder found that `acp.rs`'s `drain_stderr` has the same unbounded drain for standard error. It is added to ORI-T-0143 (claim 79), which holds `acp.rs`.

**State after this section:** the claims held are 20, 64, 69, 78, 79 and 80. The next free identifier is ORI-T-0146.

---

## Claims 69, 78 and 80 integrated on the session branch

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`. All three are tier 2 by their paths and go to the operator with the pull request.

| # | Ticket | Review, and what was repaired | Verified by the lead on the session branch |
|---|---|---|---|
| 78 | ORI-T-0137, the `ori-core` adapter files tiered 2 | The lead read the diff: one `spec/RISK_MAP.md` row, one `spec/SECURITY_NOTES.md` row, two rows of `spec/adr/ADR-0002-single-operator.md`'s surface table brought up to date. It adds tier 2 coverage and removes none. Its agent corrected the lead's ticket: only `AgentIdentity` came from `ori-broker`; `ScopedToken` and the six traits are new declarations | with claim 80 below |
| 80 | ORI-T-0145, gate 12 restates the new row | The lead read it: a thirteenth `RiskMapRow` with the Path cell verbatim, the count in the doc corrected, one new test that the identity adapter is significant and a tier 1 core file is not; the failing test reproduced first, the new test shown to fail on a wrong path | `cargo test -p ori-gates`: 191 passed |
| 69 | ORI-T-0058, six adapter traits and `AgentIdentity` in `ori-core` | Three attempts. Attempt 1 stopped correctly on two escalations ([[R42]]). The independent review found no code defect; its blocker was governance (the moved files fell off every tier 2 path), answered by claims 78 and 80, which are integrated first so the files are never untiered on this branch. Its four should-fix were repaired in attempt 3: `merge` and `set_check` are pinned to the head the gates passed on; status is fail-closed and all-or-error; a token's scope is a type of its own so it cannot be swapped with the secret; `identity_token` says an identity is not proof of registration. Integrated after claims 78 and 80. Commit ee92886 alone is red on the [[R42]] test, which 6cd1cdd fixes; a squash merge keeps `main` bisectable | `cargo test -p ori-core`: 118 passed; `-p ori-broker`: 66; `-p ori-memory ori_t_0038`: passed; workspace clippy, strict doc, fmt: clean |

`AgentRuntime` stays staged as ORI-T-0141, now **tier 2** by the new row (it changes `crates/ori-core/src/adapters.rs`), and waits on claim 79 (ORI-T-0143) releasing `acp.rs`.

**State after this section:** the claims held are 20, 64 and 79. The next free identifier is ORI-T-0146.

---

## Claims 81 to 84 dispatched; batch 10 planning started

Written by the lead at dispatch, 2026-10-11, under [[R31]]. Every branch below is cut from a962bd9.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 81 | ORI-T-0144, an evidence blob's `content_ref` resolved under the product's evidence directory | `crates/ori-memory/src/barrier.rs` | 2 | `fix/ORI-T-0144-evidence-ref` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 82 | ORI-T-0134, `ori-gates`' module docs attached to the right modules | `crates/ori-gates/src/lib.rs` (doc comments only) | 1 | `fix/ORI-T-0134-gates-lib-docs` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 83 | ORI-T-0133, `gate.proven` named in the event stream | `spec/API_SPEC.md` | 1 | `docs/ORI-T-0133-gate-events` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 84 | ORI-T-0136, the adapter traits' home stated in the architecture and the roadmap | `spec/ARCHITECTURE.md`, `spec/ROADMAP.md` | 1 | `docs/ORI-T-0136-adapter-home` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Disjoint from each other and from every held claim. Claim 20 holds `.github/workflows/ci.yml`, `scripts/gates.sh` and `fixtures/planted/gate-13/**`; claim 64 holds `ori-watch`; claim 79 holds `acp.rs`. ORI-T-0139, which also changes `barrier.rs` (docs only), waits on claim 81's release. ORI-T-0134 was ready once claims 63 and 76 released `lib.rs`, and both have.

**Batch 10 (`ori-mcp`) is being planned, and nothing is claimed for it.** ORI-T-0056 and ORI-T-0057 share design questions the backlog row does not settle:
- how `Server` reaches the lock table and escalations without a sideways dependency (`spec/LLD.md` section 2);
- what carries MCP to an agent session, and how a connection is bound to one identity;
- how the host reaches a user's server when only `ori-runtime` spawns processes;
- which tools make up the phase 1 subset (`spec/ROADMAP.md`).

One coder writes the plan for both, reading only, with no worktree and no branch. The lead reviews it and takes any spec conflict to the operator before claims are granted. This is the "before each batch" control point of `ops/phase-1-backlog.md`.

**State after this section:** the claims held are 20, 64, 79 and 81 to 84. The next free identifier is ORI-T-0146.

---

## Claims 83 and 84 integrated; ORI-T-0146 to ORI-T-0148 allocated

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`; both tier 1.

| # | Ticket | Review | Verified by the lead on the session branch |
|---|---|---|---|
| 83 | ORI-T-0133, `gate.proven` in `spec/API_SPEC.md` section 2 | The lead read the diff: one bare kind added after `gate.inert`, in the list's own form, with no payload, projection or revocation kind invented. Its agent confirmed the three places that already named the kind | its agent: `cargo test -p ori-gates` 191 passed, gate 13 clean; the lead re-ran gate 13 on the integrated commits |
| 84 | ORI-T-0136, the adapter traits' home in `spec/ARCHITECTURE.md` and `spec/ROADMAP.md` | The lead read the diff: the `integrations` row and two roadmap lines now place the declarations in `ori-core`. The diagram label "adapter slots" was rightly left as it is (PROJECT_BRIEF principle 12 names the integration categories that way). Its agent corrected the lead's ticket: the specification now says all seven traits are in `ori-core`, but at a962bd9 `AgentRuntime` is still declared in `crates/ori-runtime/src/acp.rs`; ORI-T-0141 closes that, and until it lands a drift audit would flag it | as for 83 |

The documentation agent's findings, recorded:

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0146 | `crates/ori-gates/src/prover.rs`'s module doc (around lines 103 to 106) says `spec/API_SPEC.md` section 2 does not list `gate.proven`, and calls that a gap. Claim 83 closed the gap, so the sentence is now false. Docs only | `crates/ori-gates/src/prover.rs` | 2 (`spec/RISK_MAP.md`: `prover.rs`) | ready | 2026-10-11 |
| ORI-T-0147 | `spec/ARCHITECTURE.md` section 1's diagram never says what its arrows mean. Read as crate dependencies, it contradicts `spec/LLD.md` section 2 on four edges (`RPC --> ORC`, `RPC --> FLOW`, `WATCH --> ORC`, `RT --> MCP`); read as runtime interaction, which is what the document says it describes, it agrees. Add a legend that points to LLD section 2 for dependencies | `spec/ARCHITECTURE.md` | 1 | ready; documentation role | 2026-10-11 |
| ORI-T-0148 | `ModelProvider` is declared in `ori-core` by the specification, but no document names the crate that implements it. `spec/API_SPEC.md` section 4 and `spec/LLD.md` section 2 name implementers only for the five slot traits and `AgentRuntime`. A model provider makes network calls, which `spec/ARCHITECTURE.md` section 9 allows only in integrations, runtime and mcp | `spec/API_SPEC.md`, `spec/LLD.md` | 1 | **needs planning**: the implementing crate is a decision | 2026-10-11 |

Added to ORI-T-0132 (needs planning), from the same report: `spec/criteria/phase-1.md` ORI-P1-013 ends with "gate state Installed" after `gates.prove`. `spec/DATA_MODEL.md` section 3 draws `Defined → Proven → Installed` as one prose line, with no edge back to `Defined` and none out of `Inert`. Nothing in `spec/` says what moves a gate from `Proven` to `Installed`. The runbook's revocation ("state back to Defined") has no edge and no event kind.

**State after this section:** the claims held are 20, 64, 79, 81 and 82. The next free identifier is ORI-T-0149.

---

## Claim 85: ORI-T-0147 dispatched

Written by the lead at dispatch, 2026-10-11, under [[R31]].

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 85 | ORI-T-0147, a legend for ARCHITECTURE's overview diagram | `spec/ARCHITECTURE.md` | 1 | `docs/ORI-T-0147-architecture-legend` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Claim 84 (ORI-T-0136), which also changed `spec/ARCHITECTURE.md`, is integrated, and the branch is cut after it, from 7bf0508. No other claim holds a path under `spec/`.

**State after this section:** the claims held are 20, 64, 79, 81, 82 and 85. The next free identifier is ORI-T-0149.

---

## Claim 79 integrated; ORI-T-0149 to ORI-T-0151 allocated

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`; tier 1.

| # | Ticket | Review, and what was decided | Verified by the lead on the session branch |
|---|---|---|---|
| 79 | ORI-T-0143, the ACP client's reads and writes bounded by `IO_TIMEOUT` | Independent review (Opus): nothing blocking. Every wait on a pipe is a `recv_timeout` on the remaining time. On a timeout the child is killed and reaped and the session is spent. No existing test changed: the test module at base is a byte-identical prefix of the new one. The reviewer overlaid ORI-T-0140's `headless.rs` and ran `ori-runtime` green. Two should-fix findings are carried into ORI-T-0149 below, because nothing in the engine calls `AcpClient::prompt` yet (`grep` over `crates/`: the only other user of the trait is `headless.rs`'s own implementation). Decisions the lead accepts and records: a prompt that times out kills the child and spends the session, and no launch defect is recorded for an ACP timeout. On that path the process ends before the supervisor revokes the session's credentials. `spec/LLD.md` section 5's order (revoke, then terminate, then release locks) is the supervisor's kill sequence, and the supervisor must still run it on a `Timeout`; that is a constraint on whichever ticket builds the session supervisor. Integrated as d2a9b0b | `cargo test -p ori-runtime`: 183 passed, 8 ignored; workspace clippy and fmt: clean |

The review's findings and the coder's, recorded. All three tickets change files that ORI-T-0141 also changes (`acp.rs`, `headless.rs`, `transcript.rs`), so all three wait on ORI-T-0141:

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0149 | Make `AcpClient` fit to drive a real agent. **This must land before any production caller drives `AcpClient`.** Four parts. (1) `session/prompt` runs under `IO_TIMEOUT` (20 s) for the whole turn, and an agent that thinks or runs a tool silently for longer is killed. In ACP the response to `session/prompt` arrives only when the turn ends. Bound the turn by the session's budget (`budget.rs`) and an inactivity bound instead, and keep `IO_TIMEOUT` for the handshake round trips. (2) `MAX_LINE_BYTES` (1 MiB) refuses, and spends the session on, any longer frame. A tool-call `diff` carrying a large file's old and new text, or an image block, can exceed it. Check the cap against the ACP specification; raise it, or bound the queue by bytes rather than by line count, or record with a source why 1 MiB is enough. (3) A timeout drops up to 16 queued lines, which can include a `session/request_permission` that would otherwise be recorded as a launch defect (ORI-P1-039). Drain the queue before retiring. (4) `retire` calls `kill_child`, whose `wait()` after the kill has no bound, and `let _ =` discards a kill or wait failure while the docs say "killed and reaped" | `crates/ori-runtime/src/acp.rs` | 1 (`spec/RISK_MAP.md`: `ori-runtime` acp) | ready after ORI-T-0141 | 2026-10-11 |
| ORI-T-0150 | `headless.rs` drains standard error with `drain_stderr`, which stops only at end of file. ORI-T-0143 added `drain_stderr_until(stderr, stop)`; switch the headless adapter to it so a descendant holding standard error open is not drained at full speed after the deadline | `crates/ori-runtime/src/headless.rs` | 1 | ready after ORI-T-0141 | 2026-10-11 |
| ORI-T-0151 | `Transcript::record` clones the whole entry list on every append (`transcript.rs`, around line 242), so each pass of `AcpClient::call` costs time proportional to the transcript, and a chatty agent grows the transcript without limit over a session. Cap it, and say what a capped transcript records about what it dropped | `crates/ori-runtime/src/transcript.rs` | 1 | ready after ORI-T-0141 | 2026-10-11 |

**State after this section:** the claims held are 20, 64, 81, 82 and 85. The next free identifier is ORI-T-0152.

---

## Claim 86: ORI-T-0141 dispatched

Written by the lead at dispatch, 2026-10-11, under [[R31]]. Claim 79 is released (ORI-T-0143 integrated), and claim 77 released earlier (ORI-T-0140). The branch is cut from 56dc155, which carries both.

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 86 | ORI-T-0141, `AgentRuntime` and the types in its signatures moved into `ori-core` | `crates/ori-core/src/adapters.rs`, `crates/ori-runtime/src/acp.rs`, `transcript.rs`, `headless.rs` | 2 (`spec/RISK_MAP.md`: `crates/ori-core/src/adapters.rs`) | `feat/ORI-T-0141-agent-runtime-core` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Disjoint from every held claim (20, 64, 81, 82, 85). Its test changes are the two [[R42]] authorizes, and no others. The ticket forbids a new file under `crates/ori-core/src/adapters/`: `spec/RISK_MAP.md` tiers only `adapters.rs` and `adapters/identity.rs`, and ORI-T-0058's review found that kind of move landing types on an untiered path. ORI-T-0149, ORI-T-0150 and ORI-T-0151 wait on this claim.

**State after this section:** the claims held are 20, 64, 81, 82, 85 and 86. The next free identifier is ORI-T-0152.

---

## Claim 85 integrated; ORI-T-0152 and ORI-T-0153 allocated

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`; tier 1.

| # | Ticket | Review | Verified |
|---|---|---|---|
| 85 | ORI-T-0147, a legend for ARCHITECTURE's overview diagram | The lead read the diff: one paragraph under the diagram, saying its arrows are runtime and data-flow interactions and pointing to `spec/LLD.md` section 2 for crate dependencies. No edge changed. Its agent checked every edge against the manifests and confirmed the ticket's claims, with one nuance: `acp.rs` sends `mcpServers` in `session/new`, but the list is empty, a named seam until `ori-mcp` exists | its agent: `cargo test -p ori-gates` 191 passed, gate 13 clean |

From the same report:

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0152 | `spec/LLD.md` section 2's graph draws `INT` in the `--> STORE` fan-in, but `crates/ori-integrations/Cargo.toml` depends only on `ori-core`, and the same table says `ori-integrations` must not read the store. The graph is wrong about `ori-integrations`; every other drawn edge agrees with the manifests. First noted under claim 65 | `spec/LLD.md` | 1 | ready; documentation role | 2026-10-11 |
| ORI-T-0153 | `crates/ori-integrations/Cargo.toml`'s `description` still says "Adapter traits, reference adapters ...", while the traits are declared in `ori-core` | `crates/ori-integrations/Cargo.toml` | 1 | ready; best carried by ORI-T-0059, the next ticket in that crate | 2026-10-11 |

For batch 10's plan: the runtime does not call the MCP server. It tells the agent where the server is (ACP `session/new`, `mcpServers`), and the agent calls it. As built, that list is empty.

**State after this section:** the claims held are 20, 64, 81, 82 and 86. The next free identifier is ORI-T-0154.

---

## Claim 82 integrated; ORI-T-0154 allocated

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`; tier 1.

| # | Ticket | Review | Verified |
|---|---|---|---|
| 82 | ORI-T-0134, `ori-gates`' module docs on the right modules | The lead read the diff: four `pub mod` lines, each with exactly one doc. One deviation, accepted: the coder replaced rather than moved `modified_tests`' sentence. "The detector that makes CLAUDE.md's never-modify-a-test rule real" claimed enforcement that does not exist: gate 5 is defined and not installed, and `scripts/gates.sh` reports it not available. The new sentence says what the module does and names gate 5 without saying it runs | its coder: workspace test, clippy, strict doc, fmt all exit 0; gate 13 clean |

The coder's finding, recorded:

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0154 | Nothing in CI or `scripts/gates.sh` runs `RUSTDOCFLAGS="-D warnings" cargo doc`, and no crate sets `missing_docs`. `CLAUDE.md` rule 9 requires a doc comment on every public item, and INC-0003's second half broke rustdoc on a branch. Today the check exists only as a standing instruction in each ticket. Two modules here carried no doc at all, and every gate was green | `.github/workflows/ci.yml`, `scripts/gates.sh`, possibly the workspace `Cargo.toml` lints | 2 (`.github/workflows`, supply chain) | **needs planning**: a change to the gate set; the operator decides, and claim 20 holds `ci.yml` and `scripts/gates.sh` | 2026-10-11 |

**State after this section:** the claims held are 20, 64, 81 and 86. The next free identifier is ORI-T-0155.

---

## Claim 64 integrated; ORI-T-0155 allocated

Written by the lead on 2026-10-11 under [[R31]] rule 3. Not on `main`; tier 1.

| # | Ticket | Review, and what was repaired | Verified by the lead on the session branch |
|---|---|---|---|
| 64 | ORI-T-0054, the tree watcher and the git hooks installer, on `notify` 8.2.0 exactly pinned ([[E-0009]]) | Three review passes, all independent (Opus). **Pass 1** found blocking defects; repaired in 679eb47 and 1000df3: a dead backend reported as a quiet tree; refs read through links; a preserved hook overwritten. **Pass 2** found two defects that fail only on CI's other legs: the dead-backend test panicked inside notify's `extern "C"` and `extern "system"` callbacks, which aborts the test binary on macOS and Windows; and three test helpers were dead code on Windows, which fails `clippy -D warnings` there. Both repaired in eae39cc, along with five should-fix items: open-flag tests (mutation-checked), `.git` re-checked on every poll and install, a hook linked to a managed hook refused, the third-tool window documented, and an absolute-link test. Pass 2 also found a secret-shaped test string. The scanner reads every blob any ref reaches, so the coder folded the fix into the commit that introduced it, on its own unpushed branch, and the string never reached this branch. **Pass 3** found nothing blocking. It checked the cfg gating line by line against notify 8.2.0 and libc 0.2.189, and simulated the Windows and macOS clippy runs in copies; both were clean. Its should-fix items go to ORI-T-0155 | workspace `cargo test`: **1342 passed, 0 failed**, as root; workspace clippy, strict doc, fmt: clean; secret scan: no finding in 784 history blobs |

Gate 7 judges `notify`'s dependency graph in CI on the push that carries this section.

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0155 | The hook installer's link refusal does only one `read_link` with an exact name match, and only for the four managed hooks. Pass 3 reproduced two misses in a copy. (a) A chain, `pre-push -> relay -> post-commit`, installs, and the user's pre-push veto is silently lost. (b) An unmanaged hook linked to a managed one, `post-checkout -> post-merge`, makes every checkout record a false `post-merge` event into the spool the engine reads. Compare canonical paths, and scan every entry of the hooks directory for a link that resolves to a managed hook. Also, all from pass 3: `RefPoller::poll`'s `# Errors` omits `GitDirNotADirectory` and `UnsupportedRefStorage`; the hooks module doc does not state that a `.git` swapped during an install is still written through; two doc sentences are stale (hooks.rs around 66 to 73; the `Drop` doc's "this workspace does not set it", which the new `compile_error!` makes moot); a case-insensitive file system misses `Post-Commit`. **Must land before anything installs the hooks or reads the spool** (ORI-T-0055 and the engine) | `crates/ori-watch/src/hooks.rs`, `watcher.rs` | 1 | ready | 2026-10-11 |

**State after this section:** the claims held are 20, 81 and 86. The next free identifier is ORI-T-0156.

---

## Batch 10 planned; E-0010 to E-0013 raised

Written by the lead on 2026-10-11. The plan is `ops/batch-10-plan.md`: the lead's decisions D1 to D4, D8 and D9, twelve slices, and seven prerequisites outside `ori-mcp`. Nothing is claimed yet. The first slice, W0 (the shared wire), waits on the MCP specification text, which its ticket must supply. Slice S1 waits on PD-1 (RISK_MAP rows for the new files) because `ports.rs`, `server.rs` and `tools/**` have no tier until then, and ORI-T-0058's review showed what landing code on an untiered path costs. The four questions are with the operator: [[E-0010]], [[E-0011]], [[E-0012]], [[E-0013]].

**State after this section:** unchanged. The claims held are 20, 81 and 86. The next free identifier is ORI-T-0156.

---

## E-0010 to E-0013 answered; ORI-T-0156 to ORI-T-0158 allocated; claims 87 and 89

Written by the lead on 2026-10-11 under [[R31]]. The operator answered all four in session, each as recommended (State lines in the escalation files).

| Ticket | Work | Module | Tier | State | Recorded |
|---|---|---|---|---|---|
| ORI-T-0156 | Write the four answers and the batch 10 decisions into the specification: API_SPEC sections 2 and 3, LLD's `ori-mcp` row, ENV_SETUP section 5, SECURITY_NOTES, and a **new** RISK_MAP row for `crates/ori-mcp/src/scopes.rs, ports.rs, server.rs, tools/` at 2 plus `protocol.rs` at 1. This is plan prerequisites PD-1 and PD-2 together. The existing abstract row `crates/ori-mcp (server tool scopes)` stays as it is: gate 12's test `ori_t_0044_three_risk_map_tier_two_rows_are_recorded_as_unresolved_and_not_silently` counts it as one of exactly three unresolved rows, and changing it would be a `test_modified` | `spec/API_SPEC.md`, `spec/LLD.md`, `spec/ENV_SETUP.md`, `spec/SECURITY_NOTES.md`, `spec/RISK_MAP.md` | 2 | dispatched, claim 87; documentation role | 2026-10-11 |
| ORI-T-0157 | Restate ORI-T-0156's new tier 2 row in gate 12's `RISK_MAP_TIER_TWO`, on ORI-T-0156's branch: the pattern of ORI-T-0137 and ORI-T-0145 | `crates/ori-gates/src/significance.rs` | 1 by path; carried with ORI-T-0156, so tier 2 | after ORI-T-0156; claim 88 reserved | 2026-10-11 |
| ORI-T-0158 | Batch 10 slice W0: the shared MCP wire (`protocol.rs`), on MCP 2025-11-25, which the lead fetched from the specification's repository because the specification's site is outside this container's network policy | `crates/ori-mcp/Cargo.toml`, `src/lib.rs`, `src/protocol.rs`, `Cargo.lock` | 1 | dispatched, claim 89 | 2026-10-11 |

| # | Ticket | Modules | Tier | Branch | Granted | Released | Recorded |
|---|---|---|---|---|---|---|---|
| 87 | ORI-T-0156 | as above | 2 | `docs/ORI-T-0156-mcp-spec` | 2026-10-11 | **held** | 2026-10-11, at dispatch |
| 89 | ORI-T-0158 | as above | 1 | `feat/ORI-T-0158-mcp-wire` | 2026-10-11 | **held** | 2026-10-11, at dispatch |

Claim 87 and claim 89 are disjoint, and both are disjoint from every held claim (20, 81, 86). No other claim holds `Cargo.lock` or any path under `crates/ori-mcp/`.

**State after this section:** the claims held are 20, 81, 86, 87 and 89; claim 88 is reserved for ORI-T-0157. The next free identifier is ORI-T-0159.

---

## Claim 64 withdrawn from the session branch and held again

Written by the lead on 2026-10-11. CI on 3e53283, the first run to carry ORI-T-0054, failed two checks. The lead reverted its seven commits in one commit (6d406e4: a revert, not a history rewrite), so the session branch's code is again identical to f3da68e, which CI passed in full.

| Check | What failed | Whose |
|---|---|---|
| gate-7 (`cargo deny check bans`) | `windows-sys` locked at both 0.60.2 (through `keyring` 3.6.3, `notify` 8.2.0 itself, and `winapi-util`) and 0.61.2 (through `mio` 1.2.4 under `notify`). [[E-0009]] predicted this and set its terms: if gate 7 refuses, the question returns to the operator, and `deny.toml` is not relaxed to admit it. The lead read the manifests. `notify` pulls `mio` only on Linux, Android, macOS and the BSDs, and `mio` pulls `windows-sys` only on Windows, so the two versions never build together on any one target. `deny.toml` has no `[graph] targets` and judges the union of every target. Every `notify` release has the same pair (8.x on 0.60, its `mio` on 0.61; 9.0 release candidates on 0.61 against `keyring`'s 0.60) | this ticket's dependency; the operator decides |
| test (macos-latest) | Four watcher tests fail under FSEvents: three wait for a modification and see `Created`, because FSEvents reports a later write to a recently created file as a creation; one sees the temporary of a rename-over reported beside its target, because FSEvents does not pair the two halves of the rename as inotify does | this ticket's code; repaired on its branch |

**Claim 64 is held again** for the repair on `feat/ORI-T-0054-watch`. ORI-T-0155 (the follow-up from pass 3) is folded into the same repair, since it changes the same two files and nothing of ORI-T-0054 is integrated to follow up on.

**State after this section:** the claims held are 20, 64, 81, 86, 87 and 89; claim 88 is reserved. The next free identifier is ORI-T-0159.
