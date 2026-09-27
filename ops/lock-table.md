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
