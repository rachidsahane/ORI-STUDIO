# Calibration records: Ori Studio

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

## CR-006: two defects the lead committed while instructing a coder not to

Both found by ORI-T-0019's coder, in the prompt the lead wrote it. Recorded because the shape matters more than either instance.

### 1. The lead used the unsafe idiom in the same breath as forbidding it

The ticket told the coder, at length, not to read `$?` through a pipe, citing CR-004. It then offered two safe alternatives, and **one of them does not work in this shell**:

```
$ echo "shell: $0; version: $ZSH_VERSION"
shell: /bin/zsh; version: 5.9
$ (true | true; echo "PIPESTATUS[0]='${PIPESTATUS[0]}'  pipestatus[1]='${pipestatus[1]}'")
PIPESTATUS[0]=''  pipestatus[1]='0'
```

`PIPESTATUS` is bash. In zsh the array is `pipestatus` and it is 1-indexed, so `${PIPESTATUS[0]}` expands to the empty string and prints nothing where a status should be.

**The lead then used it.** Three commands in the same session ended `echo "push exit: ${PIPESTATUS[0]}"`, each printed `push exit:` with nothing after it, and the lead read them as passing. The pushes did succeed, so nothing was lost, but **the check reported nothing and was counted as a check**. That is CR-004 exactly, and it is the project's dominant defect class, committed by the lead inside the instruction warning about it.

The coder used the other alternative throughout and reported the problem rather than working around it silently.

**Fix.** The instruction now reads: redirect to a file and capture `$?` on the next line. Nothing else. One idiom that works in both shells beats two where one is a trap.

### 2. The lead cited records the coder could not see

The ticket referred to escalation `E-0004` and to `CR-005` as existing. The coder grepped the repository and reported, correctly, that neither string appears anywhere in it.

Both were real, written and pushed, in pull requests 25 and 26. **Neither was in `main`, which is what the ticket's worktree was cut from.** So the coder was working from a tree in which the lead's evidence did not exist, and had no way to tell an unmerged record from an imagined one.

This is **not** the class of rulings R15 to R24, where the record was never written at all, and the coder's diagnosis that it was is the one thing in its report that is wrong. It is a new class: **a ticket dispatched from `main` while the lead reasons from branches.** The failure mode is worse than the old one, because the record exists and the lead can point at it, while the agent doing the work correctly reports it absent.

**Fix.** A ticket that cites an operational record states where the record lives: merged, or pull request N. If the ticket depends on the record rather than merely mentioning it, the ticket waits for the merge.

### What the two have in common

Both are the lead treating its own knowledge as though it were the repository's. CR-005 measured that `ops/` has no gate on it; these two are the same fact in motion, one turn later. The count of this class is now six.

### A third finding, kept because it is cheap and recurring

`ops/escalations/` numbers its files `E-000n`. `ops/phase-1-backlog.md` numbers its open escalations 1 to 9 with no prefix. **The two are unrelated and nothing says so.** Backlog row 3 is the dependency question; `E-0003` is the secret scan. Backlog row 4 is gate 9's scope. A reader who assumes the obvious mapping is wrong every time, and that is very likely how defect 2 above began. The backlog rows want `E-` identifiers or an explicit note that they are not the same numbering.
