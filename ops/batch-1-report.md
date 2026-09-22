# Batch 1 report: workspace, toolchain, CI skeleton

Written by the lead at batch 1 close. Every number here was read from the repository or the version control host at the time of writing, not recalled. That distinction is the subject of CR-005 in `ops/calibration.md`.

## Position

| | |
|---|---|
| Tickets merged | **16** of 18 planned, plus 5 raised during the batch |
| Tickets not merged | 2: ORI-T-0010 and ORI-T-0015, both blocked |
| | *Corrected. The first row read 15, and 15 plus 2 is 17 of 18. ORI-T-0018 fell in neither row, which this same report states as merged twice elsewhere. The smoke ticket was left out of the arithmetic of the batch it closed.* |
| Escalations opened | 4, all open with the operator |
| Gates installed | 3 of 5 in this batch's scope, 1 partial, 1 with no subject yet |
| Deliberate red runs | 3, on branches closed unmerged and deleted. No planted defect entered `main` |

## What merged

| Pull request | Ticket | Tier |
|---|---|---|
| 1 | ORI-T-0000, specification corrections | 2 |
| 2 | ORI-T-0011, ADR-0002 the single-operator exception | 2 |
| 3 | ORI-T-0006, methodology anchor defect report | 0 |
| 4 | ORI-T-0007, `spec/agents/CLAUDE.md` as the canonical product base | 1 |
| 5 | ORI-T-0009, seed `templates/` from AICD appendix A | 1 |
| 6 | ORI-T-0008, populate `.claude/agents/` | 1 |
| 7 | ORI-T-0001, Cargo workspace and sixteen crate skeletons | 2 |
| 8 | ORI-T-0003, toolchain pin, gitignore, `setup-dev.sh` | 1 |
| 9 | ORI-T-0004, `scripts/gates.sh`, the local gate set | 1 |
| 10 | ORI-T-0005, methodology section index and generator | 1 |
| 11 | ORI-T-0002, scaffold `apps/desktop` | 1 |
| 12 | ORI-T-0012, CI workflow and the three-platform build | 2 |
| 16 | ORI-T-0013, prove CI gate 1 on a planted defect | 2 |
| 19 | ORI-T-0014, prove CI gate 2, and record what it cannot see | 2 |
| 21 | ORI-T-0016, gate 7, two checks of three | 2 |
| 23 | ORI-T-0017, build and prove gate 13, the commit-trailer gate | 2 |

Raised during the batch and merged: 13 (ORI-T-0080, root README), 14 (ORI-T-0081, LICENSE and NOTICE), 15 (ORI-T-0082, line-ending independence in the methodology parser, found by CI on Windows), 18 (ORI-T-0083, design reference on the front page), 22 (ORI-T-0084, repair defective evidence in installed gate 1).

Closed unmerged on purpose: 17, 20 and 24, the three visibility demonstrations. Each carried a planted defect, was observed red, and was deleted. AICD §14's third condition is satisfied by observation, and `main`'s history never contained the defect.

## Gate state

| Gate | Ticket | State | Evidence |
|---|---|---|---|
| 1, `fmt` and `clippy` | ORI-T-0013 | **Installed** | green run, plus run on pull request 17 observed red |
| 2, `cargo test` workspace-wide | ORI-T-0014 | **Installed** | run 35598542173 green, run 35609765474 observed red on three platforms |
| 3, contract tests | ORI-T-0015 | **Not started, and cannot be** | there is no contract to test until `ori-rpc` exists |
| 7, audit, deny, secret scan | ORI-T-0016 | **Partially installed, two checks of three** | advisories and licences proven on planted defects; the secret scan is escalation E-0003 |
| 13, commit trailers | ORI-T-0017 | **Installed** | run 35654836919 green, run 35656176844 observed red |

Eighteen checks now run on every pull request: `build`, `clippy` and `test` on ubuntu, macos and windows; `fmt`; `gate-1-proof`; `gate-2-proof`; `gate-7`; `gate-7-proof`; `gate-13`; `gate-13-proof`; `ci`; and `GitGuardian Security Checks`, which nobody in this project installed.

## The exit criterion, read literally

The backlog says: "every gate below proven on a planted defect, the smoke ticket merged, `cargo build --workspace` green on macOS, Windows and Linux in CI."

| Clause | State |
|---|---|
| `cargo build --workspace` green on three platforms | **met**, and on every pull request since 12 |
| the smoke ticket merged | **met**. Pull request 25 merged at `bf05ef9` |
| every gate proven | **not met, and two of the gaps are not closable inside batch 1** |

**Gate 3 has no subject.** It proves contract tests against an interface that `ori-rpc` does not define yet. `ori-rpc` is batch 8. The ticket was planned into batch 1 because the batch is titled "CI skeleton", and that was a planning error: a gate cannot be proven on a planted defect when nothing exists to plant a defect in. ORI-T-0015 moves to the batch that creates its subject.

**Gate 7's third check is escalation E-0003**, open with the operator, and deliberately not spent a third attempt on.

**A correction.** The lead previously told the operator that gate 9, the citation gate, was one of the two gaps holding batch 1's exit. That was wrong. Gate 9 is not in batch 1's table and the exit clause is "every gate below", meaning gates 1, 2, 3, 7 and 13. Gate 9 is ORI-T-0047, it is held by the operator's ruling pending the methodology anchor fix, and it holds nothing here. The gaps are gate 3 and gate 7's third check.

## Escalations open with the operator

| | Trigger | Question |
|---|---|---|
| E-0001 | `spec_conflict` | What a tier 2 change carries while three of AICD §38's five substitutes do not exist |
| E-0002 | `contract_change` | Where the adapter traits live. Binds at batch 8 |
| E-0003 | `spec_conflict` | What implements gate 7's secret scan. Lead recommends `gitleaks`, pinned by SHA-256 |
| E-0004 | `new_dependency` | The four external crates the specification already names. Blocks three of batch 2's four tickets |

Also owed, and not an escalation: **`ci` is not a required status check on `main`.** Five gate proofs now record this. Every gate in this repository can be red while a merge goes through, because nothing has been configured to stop it. This is the single highest-value setting change available and it costs one API call.

## What batch 1 taught, beyond its tickets

Four findings carried into `ops/calibration.md`:

- **CR-001**, cross-model review: 130 findings raised, 42 survived refutation. A review lens that refutes above 90 per cent is asking too broad a question and should be split.
- **CR-003**, the recurring class: a precondition stated as fact. Four of five recorded human-origin defects. The operator's own rule, that verification does not depend on who states the precondition, is the highest-yield control in the project so far.
- **CR-004**, a gate can report the opposite of the truth, and did. `if cmd | tee log | tail -6; then` reads `tail`'s exit status. A refused push was reported as success.
- **CR-005**, the lead's records are the least gated artifact in the project. Eighteen checks judge `crates/`. Zero judge `ops/`, which is where the evidence for those eighteen lives.

And one incident: **INC-0002**, a coder acting outside its declared scope. `ori-watch`, which AICD §12 specifies to catch unattributed change, is batch 9.

> **Corrected, and the correction matters more than the identifier.** This sentence cited **INC-0001**, which exists in no branch of this repository: `ops/incidents/` holds exactly one file, `INC-0002-scope-violation.md`. Worse, the narrative attached to it contradicted the real record in every particular. It said "an unattributed change undetected for 100 minutes, found by accident"; INC-0002 records the actor as known, self-reported, and **"Detected: by the lead, reading the coder's own report, before any commit."** So the lead invented an incident identifier, attached a worse story to it than the truth, and cited it in two records. Found by the round 4 audit ([[CR-007]]).

## What the operator owes before batch 2 can run in full

2. Answer E-0004. Three of batch 2's four tickets stop on it; ORI-T-0019 has started without it.
3. Decide whether tier 0 auto-merge turns on now that gates 1, 2 and 13 are installed, or stays off. The batch 1 ruling disabled it "on unproven gates" and that condition has changed.
4. Optionally: make `ci` a required status check, which is what turns eighteen green checks into eighteen enforced ones.

## Position at the time this record merges, which is later than the rest of it

The body above was written when batch 1 closed. Four pull requests then merged in the order this record asked for, and the facts have moved. Updated rather than left to disagree with `main`, which is the defect ORI-T-0018 found in two gate records.

| Pull request | Ticket | Landed at | State |
|---|---|---|---|
| 25 | ORI-T-0018, the G4 trivial-ticket test | `bf05ef9` | merged |
| 27 | ORI-T-0019, domain types and the error enum | `0974dd1` | merged |
| 28 | ORI-T-0085, tie the index restatement to the index | `478173e` | merged |
| 26 | this record, superseded | | closed, could not be rebased |

**`main` is green at `478173e`**, run 35664113834. That matters more than it reads: pull request 28 was deliberately red on its own branch, because the test it adds fails when `crates/ori-core/src/error.rs` is absent and that file arrived with 27. The merge order held, the subject arrived, and the test went green on `main` without anyone editing it. A check that was red for a stated reason and turned green when the reason was removed is a check that was measuring something.

So gate 2 and gate 13 are **Installed** without qualification, and batch 2 has its first two tickets merged rather than open.

### What this changes in the list below

Item 1, merging pull request 25, is done and is struck from the list. This record itself moved branches on the way here: pull request 26 merged cleanly but could not be **rebased**, and this repository merges by rebase only. [[R30]] records the mechanism and why the history could not simply be rewritten. Items 2, 3 and 4 stand: escalation E-0004 is still unanswered and still holds ORI-T-0020, ORI-T-0021 and ORI-T-0022; the tier 0 auto-merge condition is still the operator's to resolve; and `ci` is still not a required status check.

That last one acquired a demonstration while this record sat open. Pull request 28 carried four red checks, `test` on three platforms and `ci`, and **nothing in the repository would have prevented it being merged in that state.** It was merged deliberately, in the right order, by a human who had read why. The point is that the eighteen checks played no part in that decision, because none of them can.
