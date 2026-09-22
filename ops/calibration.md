# Calibration records: Ori Studio

> **This file is closed to new entries.** Ruling [[R30]] moved serially numbered records to one file each. CR-001 to CR-005 stay here, unmoved. **CR-006 onward live in `ops/calibration/`, one file per record.**


Measurements taken under AICD §21 (performance-relative thresholds) and §30 (calibration procedures). Every threshold in this product is expressed relative to a measurement recorded here, never as an absolute number.

## CR-001: cross-model review of the specification set, 2026-09-20

The first measurement this product has. It is a review-quality measurement, not a budget measurement: no ticket has been worked yet, so no token or attempt medians exist.

**What was measured.** The phase 1 specification set (20 documents, the six agent role files, the criteria file, three runbooks and the design document) was written by one model and audited by another, through eight independent lenses, each lens blind to the others. Every finding was then put to two adversarial refuters with distinct instructions: a textualist checking the quote against the file, and an architect asking whether the disagreement was real or was deliberate layering between a catalog, a schedule and a criterion. A finding survived only if neither refuter rejected it.

| Quantity | Value |
|---|---|
| Agents | 25 (8 finders, 16 refuters, 1 completeness critic) |
| Findings raised | 130 |
| Findings surviving both refuters | 42 |
| Refutation rate | 68 percent |
| Additional findings from the completeness critic | 13 |
| Distilled to, in the operator-facing report | 6 contradiction groups, 8 unmappable criteria, 9 registry defects |

### Raised against survived, by lens

| Lens | Raised | Survived | Refuted |
|---|---|---|---|
| citations | 15 | 11 | 27 percent |
| methodology-conformance | 14 | 7 | 50 percent |
| phases | 21 | 7 | 67 percent |
| criteria-map | 20 | 6 | 70 percent |
| interfaces | 14 | 4 | 71 percent |
| permissions | 12 | 4 | 67 percent |
| layout | 18 | 2 | 89 percent |
| gates | 16 | 1 | 94 percent |

### What the numbers say

- **A single-pass model audit would have been wrong two times in three.** 88 of 130 findings did not survive contact with the text or with the document set's deliberate layering. Reporting them unfiltered would have cost the operator more attention than the defects did. The adversarial pass is not a formality; it is what makes the audit usable.
- **Refutation rate varies by an order of magnitude across lenses, and it is diagnostic.** The citation lens is mechanical: a reference either resolves against the section index or it does not, and 73 percent of its findings survived. The gates and layout lenses are interpretive, and 89 to 94 percent of their findings were the reviewer mistaking layering for conflict.

 
Calibration note, general to AICD and not specific to this project
 
**A lens refuting above 90 percent is asking too broad a question and should be split, not given more agents.** The failure is not reviewer quality; it is that one lens was made to carry two kinds of question at once. A mechanical question has a decidable answer from the text alone: does this reference resolve, does this path exist, is this identifier in that list. An interpretive question does not: is this disagreement real, or is it a specification set doing its job, where a catalog lists everything, a schedule commits a subset, a criterion tests one slice and an instruction file narrows a base. Mixing the two floods the mechanical findings with interpretive noise, and the refuters then spend their budget rejecting the noise.

The two lenses to split, and how:

 
 - **gates**, 94 percent refuted. It was asked to check gate definitions, test levels, CI wiring, runbooks and thresholds together. Split into a set-membership lens (is every gate in the pipeline definition also in the data model's gate enumeration, in the delivery plan, and in the test strategy, and the reverse) which is mechanical and decidable, and a gate-lifecycle lens (is any gate cited as protection before its proof exists, and can the gates named for a given stage actually run at that stage) which is interpretive and needs the reviewer to reason about ordering.
 
 - **layout**, 89 percent refuted. It was asked to check module ownership, dependency direction and file paths together. Split into a path-existence lens (every path any document names either exists, or is scheduled to be created by a named work item) which is mechanical, and an ownership lens (does any two documents assign the same responsibility to different modules) which is interpretive.

The counterpart holds too: a lens refuting below roughly 30 percent, as the citation lens did here, is a lens whose work a deterministic checker should be doing instead of a model. Its high survival rate is not a sign the model is good at it, but a sign that the question was never one that needed a model.
- **The defect classes AICD §39 names were the ones actually found.** Fabricated section references (a citation to methodology section 55, which does not exist), preconditions named as if they existed, and gates cited as protection before any proof existed. All three were present in a specification set written with care, which is the case §39 makes: the failures were human beliefs written into artifacts as facts, not agent inventions.

### Thresholds this sets

None yet. This is a baseline, not a threshold. Two numbers become calibration targets once the engine exists:

- **Drafting-error rate** (AICD §39, PRD C-07): fabricated or unresolvable references per document produced. Measured here as 1 unresolvable methodology reference in 20 documents, plus 3 dangling PRD function identifiers and 4 wrong internal section numbers. The citation gate (CI_CD gate 9) drives this to zero mechanically once it is installed.
- **Review precision**: the share of a reviewing model's findings that survive adversarial verification. 32 percent here. Re-measure on any change to the reviewing model, and treat a sharp rise with suspicion: it more likely means the refuters got weaker than that the reviewer got better.

**Re-run condition.** Whenever the model running the lead role changes, whenever the agent instruction files change materially, and at the end of phase 1 against the then-current specification.


## CR-002: human-origin defects, running record

AICD §39 records that the failures found on the first application were not agent inventions: each was a human belief written into an artifact as a fact, or a control that existed only on paper. This record continues that count for this project, because the rate and the class are what make the rule worth its cost.

| # | What was stated | What was true | Class |
|---|---|---|---|
| 1 | A directory was described in a ticket as a dead legacy backend, to be deleted | It owned the live database's migration history and built two of five production images | A belief stated as a conclusion (AICD §39) |
| 2 | A rollback point was named in a backlog as existing | It did not exist | A precondition stated as fact (AICD §39) |
| 3 | Four preconditions for phase 1 were reported as met: toolchain, remote, initial commit, branch protection | Two were met. The Rust toolchain was absent, and `main` had neither branch protection nor a ruleset | A precondition stated as fact |
| 4 | On three further occasions: that the Rust toolchain was installed, that six pull requests had been merged, and that branch protection was on | The toolchain was absent on the first, and was installed by the fleet. The pull requests were open on all three occasions. Protection was absent on all three and was applied by the fleet | A precondition stated as fact, repeated |
| 5 | A control was verified and the verification reported the opposite of the truth | Branch protection refused a direct push to `main`, exactly as required. The lead's test harness piped `git push` into `tee` and `tail`, so it read the pipeline's last exit status and reported the refusal as a success | Present but reporting nothing (AICD §39), produced while proving a control |

**Defect 3, in detail.** On 2026-09-20 the operator reported four preconditions met and instructed the fleet to proceed without further confirmation. The lead verified each against its own evidence before starting: `cargo`, `rustc` and `rustup` absent from every standard path with no `~/.cargo` or `~/.rustup`; `branches/main/protection` returning 404, `rulesets` returning an empty list, and the effective rules for `main` empty. Two of four were absent. No work started.

**Why it matters more than the first two.** Defects 1 and 2 were found by a gate reading an artifact. This one was found by an agent declining to accept a direct, explicit statement from the human who holds every seat, on a project where that human is the sole authority. A methodology in which the operator's word overrides verification would have proceeded, built a workspace with no compiler, and discovered it at the first `cargo build`. The cost of the check was two API calls and one path search.

**The rule the operator set after it.** Verification is not conditional on who states the precondition. When a precondition is absent, stop the ticket that depends on it and continue everything that does not, rather than stopping the session. This is a strictly better failure mode than the original instruction, which stopped all work on any absence: it keeps the cost of a wrong precondition proportional to what actually depended on it.

**Measurement to carry forward.** Preconditions stated as fact is now the highest-frequency human-origin defect class in the record, at two of three. It is also the cheapest to check. Any ticket template that names a precondition should carry the command that verifies it, so the check is mechanical rather than remembered.


## CR-003: the defect that keeps recurring, and what it costs

Five human-origin defects are now recorded and **three of the five are the same class**: a precondition stated as fact. It is the cheapest class to check and the most frequent to occur.

| | |
|---|---|
| Occurrences | 4 of 5 recorded defects involve a precondition stated as fact |
| Cost of checking | Two API calls and a path search, under ten seconds |
| Cost when unchecked | A workspace built with no compiler; gate tickets landing against an unprotected branch, proving the gates exist and nothing about whether they are enforced |
| Detection | Every occurrence was caught by the fleet verifying rather than accepting |

The rule the operator set after occurrence 3, that verification does not depend on who states the precondition, has now caught three more. It is the highest-yield control in this project so far, measured in defects caught per unit of effort, and it costs less than any gate.

**The mechanical fix, recommended for the methodology.** A ticket that names a precondition should carry the command that verifies it, so the check is executed rather than remembered. `scripts/gates.sh` (ORI-T-0004) is the natural home for the local ones. This is proposed as a calibration note to AICD §30 alongside the lens-split note of CR-001.

## CR-004: a gate can report the opposite of the truth, and did

Defect 5 above is worth separating, because it is not a human-origin defect and not an agent-origin one. It is a defect in a **check**, and it appeared the first time this project verified a control.

Branch protection was applied and then tested by attempting the forbidden action, a direct push to `main`. The push was refused correctly. The test harness reported that it had succeeded, because:

```
if git push origin main 2>&1 | tee /tmp/push.log | tail -6; then
```

returns the exit status of `tail`, not of `git push`. The control was sound; the instrument was inverted.

**Measurement.** One of one controls verified so far produced a false reading on first attempt, from the harness rather than the control. The sample is one, so the rate means nothing yet; the failure mode means a great deal.

**What it changes.** Every gate this project writes must capture the exit status of the command it gates before piping that command's output anywhere: a separate capture, `PIPESTATUS`, or `set -o pipefail`. A gate whose command is piped into a formatter has no exit status of its own and passes on every input, which is indistinguishable from working until a planted defect is put in front of it.

This is the strongest evidence yet for AICD §14's planted-defect requirement. Without a deliberately failing input, the inverted harness above would have been recorded as a successful proof, and branch protection would have been cited as installed on the strength of a check that could not fail.

## CR-005: the lead's records are the least gated artifact in the project

Measured on ORI-T-0018, the G4 trivial-ticket test. The ticket's change was one doc comment. Its journey found seven defects, and **five of the seven are in records the lead wrote**, none of which any gate reads.

| # | Defect | Where | Would any gate have caught it |
|---|---|---|---|
| 1 | list item deleted without renumbering (`1. 3.`) | `ops/gates/gate-2.md` | no |
| 2 | same, and a placeholder run id `Run 35655...` where every other record carries a real one | `ops/gates/gate-13.md` | no |
| 3 | worktree handed to the coder dirty with the lead's uncommitted `ops/` edits | working tree | no |
| 4 | the ticket never claimed anything in `ops/lock-table.md`, and `ops/phase-1-backlog.md` declares a scope path the change did not touch | `ops/lock-table.md`, `ops/phase-1-backlog.md` | no |
| 5 | the pull request's exact check names written from memory: `clippy` as one job when it is three, `gate-2-proof` and `GitGuardian Security Checks` missing | pull request body | no |

The two that are not the lead's are a bare `§28` that gate 9 does not collect, and AICD §28's opening sentence, which is a methodology defect.

### What the numbers say

**Eighteen checks judge `crates/`. Zero judge `ops/`.** Every line of Rust in this repository is read by `fmt`, `clippy`, `test` on three platforms, two supply-chain checks and a secret scanner. Every line of the operational record that those gates' own evidence lives in is read by nobody, and the only reason the project knows about these five is that a coder was asked to report friction and did.

That inverts the project's own claim. `ops/gates/*.md` is where a reader goes to find out whether a gate is real. A placeholder run id in that file is not a typo; it is the evidence for AICD §14's visibility clause being absent while the file says **Installed**. Defect 2 shipped a gate record whose state line and whose evidence disagreed, and it was the record, not the gate, that was wrong.

### The class, and its count

Defect 4 is the fourth occurrence of a single failure: **a grant, a ruling or a claim that exists in the lead's intent and not in a file.**

| # | Occurrence | Cost |
|---|---|---|
| 1 | rulings R15 to R24 issued in prompts, never recorded | six dangling `R16` and `R19` references shipped in code |
| 2 | the same rulings written to a working tree and never committed | ORI-T-0005's reviewer failed the ticket a second time, correctly |
| 3 | ORI-T-0016's scope extension granted in a prompt (ruling R28) | a reviewer reported `scripts/gates.sh` as an unrecorded scope violation, correctly |
| 4 | ORI-T-0018 never entered in the lock table, against a backlog path it did not touch | nothing, because nothing else was in flight |

Occurrence 4 cost nothing only because the fleet was serial. The lock table's whole purpose is parallel work; the first batch that runs two coders at once is the batch where this class stops being free.

### What it recommends to the methodology

AICD §14 says a gate is installed when it has been seen to fail on a planted defect. It is silent on the record that asserts this, and that record is load-bearing: it is what a human reads instead of re-running the proof. Two proposals for AICD §30, alongside the lens-split note of CR-001 and the executable-precondition note of CR-003:

1. **The evidence record is part of the gate.** A gate is installed when the proof has been seen to fail *and* its record names the runs that were observed. A record citing a run id that does not resolve is an uninstalled gate, not an untidy file, and that is mechanically checkable.
2. **Fleet bookkeeping needs a gate of its own.** The lock table, the rulings file and the backlog are the fleet's control surface under §12, and in this project they are the only artifacts with no check on them. `ori-watch` (§12) is specified to detect unattributed change and does not exist yet.

**Measured cost of the control that found these.** One instruction to the coder: report what the path did wrong, not only what the ticket did. Five lead defects, none of which any gate in the repository can see. It is the same shape as CR-003's finding: the highest-yield controls so far are the ones that cost a sentence.
