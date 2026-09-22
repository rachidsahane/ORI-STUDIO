# Planted inputs for gate 13 (the commit-trailer gate)

Ticket: ORI-T-0017. Spec anchor: `spec/CI_CD.md` section 1 item 13, `spec/CONVENTIONS.md` "Git", `spec/PRD.md` G-02, `spec/TESTING.md` sections 1 and 4, `spec/runbooks/prove-gate.md`.
Proof: `ops/gates/gate-13.md`. Rule: AICD §14, "a gate is installed only when it has been seen to fail".

## Criterion

`ORI-P1-013` (`spec/criteria/phase-1.md`): *precondition* gate installed, its planted defect present; *action* `gates.prove`; *expected result* passes clean, fails dirty, proof stored with evidence refs, gate state Installed.

That criterion is written against the `gates.prove` tool in `crates/ori-gates/src/prover.rs`, which does not exist yet; `ops/phase-1-backlog.md` assigns ORI-T-0017 no criterion of its own for that reason. `fixtures/planted/gate-13/prove.sh` is what `gates.prove` will drive for this gate, and it is the input the criterion's "its planted defect present" clause refers to. When the tool lands, the test that names `ORI-P1-013` runs against this directory.

## What gate 13 is, and why it is two claims and not one

`spec/CI_CD.md` section 1 item 13: "every commit on the PR: Conventional Commits, `Ticket: <id>` and `Spec: <document>#<section>` trailers, per CONVENTIONS; PRD G-02".

That sentence contains two claims a proof has to establish separately, and this directory is shaped by the split.

**The first is what a conforming commit message looks like**, and the trap in it is that git does not read trailers the way a reader does. Git parses trailers out of the **last paragraph** of a message and out of nothing else. A message carrying `Ticket:` and `Spec:` correctly spelled, at the start of their lines, in a paragraph that is not the last one, is accepted by any human and by any regex, and `git interpret-trailers --parse` returns nothing for those two keys. Everything downstream that reads trailers, the traceability chain of AICD §13 among them, sees nothing. That message is not hypothetical: the lead shipped exactly that shape in the first commit this project made under CONVENTIONS, which is why `messages/trap-trailers-in-earlier-paragraph.txt` is here and why stage 7 of `prove.sh` runs a regex and the gate over the same message and prints both answers side by side.

**The second is that "every commit on the PR" names a set**, and a gate that computes the empty set and reports success has checked nothing while looking exactly like a gate that checked everything and found nothing wrong. That is AICD §39's "present but reporting nothing". The set is different under each of the four events `.github/workflows/ci.yml` runs under, and different again at a coder's desk, so the enumeration rows below plant nine ways for it to come out empty or undeterminable and two ways for there to be legitimately nothing at a desk.

Because of the second claim, the gate answers **four** questions and not two, and the case table's verdict column has four values:

| Verdict | Exit | Meaning |
|---|---|---|
| `pass` | 0 | every commit in the set conforms, and the set was not empty |
| `fail` | 1 | at least one commit does not conform |
| `refuse` | 2 | the set could not be determined, so no commit was checked |
| `nothing` | 3 | there was legitimately nothing to enumerate, which happens only off a GitHub event. Nothing was checked, and this says so rather than reporting a pass |

Giving `pass` and `nothing` one name is this gate's own defect class, which is why they are two states here and why all thirty-five rows the gate must not pass carry `!^  PASSED\.` as a required **negative** signature. On the eleven `refuse` and `nothing` rows, where no commit was checked at all, stage 3 **enforces** that negative and refuses to run without it. Those rows would otherwise rest on an exit status alone, and one weakened line in `scripts/gates.sh` could change the exit status of a run that checked nothing.

## What is here

| Path | What it is | Count |
|---|---|---|
| `prove.sh` | The harness, in eleven numbered stages with lettered self-checks between them. Builds every planted repository at run time, runs the gate once per row, records exit status and output, and judges only what was recorded. | 46 rows |
| `messages/*.txt` | One commit message per `message` row, and every `message` row has one: the four clean controls the gate must accept, the malformed subjects and trailers it must reject, and the three trap messages whose `Ticket:` and `Spec:` lines are correctly spelled and sit where git's trailer parser will not read them (glued to the subject, after prose in the last paragraph, and in a paragraph that is not the last). | 22 files |
| `workflow-samples/*.yml` | Whole workflow files, read by `workflow-facts.awk` as inputs to the harness's own self-checks. Twelve for the liveness check (one live, ten dead in ten different ways with the command string left byte-identical, one the parser refuses to read), six for the self-identification check (four `self-*`, plus `live.yml` and the unreadable one reused), four `depth-*` for the checkout-depth check. | 20 files |
| `workflow-facts.awk` | Reads a GitHub Actions workflow file and prints its structure: triggers, jobs, job keys, `needs:`, steps, step keys and single-line `run:` values. `prove.sh` asks its questions of that structure rather than of the file's text. | 1 file |

The 46 rows, by the verdict each is owed and the family it belongs to:

| | `pass` | `fail` | `refuse` | `nothing` | total |
|---|---|---|---|---|---|
| `message` | 4 | 18 | 0 | 0 | 22 |
| `range` | 3 | 2 | 0 | 0 | 5 |
| `enumeration` | 4 | 4 | 9 | 2 | 19 |
| **total** | **11** | **24** | **9** | **2** | **46** |

**The family column is not decoration.** `message` rows present one commit and ask what the gate says about its message. `range` rows ask what it does with a set of more than one commit and with a merge commit in it. `enumeration` rows ask what it does when the set is empty, unnameable, or produced by an event other than `pull_request`: `push`, `merge_group`, `workflow_dispatch`, a shallow clone, a payload with no SHAs, a count GitHub and the gate disagree on, and a local branch at a desk. A table missing a family would report on a gate whose other half ran unwatched.

## What is NOT here, and where it went

**The gate's implementation is `scripts/gates.sh`, not a file in this directory.** Ruling R28: a fixture directory holds planted defects, which are inputs a gate is run against; the checker a gate invokes is not one. `.github/workflows/ci.yml` runs `bash scripts/gates.sh --commit-trailers` in the `gate-13` job, a coder at a desk runs `bash scripts/gates.sh`, and this harness runs the same `ct_run()` over the same implementation. There is exactly one implementation and all three callers reach it. A second copy inline in the workflow would be the other half of the same defect: a gate that can be repaired in the copy CI does not run.

The planted repositories are **built at run time** by `prove.sh` in a temporary directory, from the messages in `messages/`. They are not committed, because a tracked repository carrying commits that fail this repository's own commit gate is a thing nobody could merge.

## Running it

```
bash fixtures/planted/gate-13/prove.sh
```

Exit 0 the proof holds; 1 gate 13 did not behave the way `ops/gates/gate-13.md` records, meaning a check has been weakened or a planted input no longer contains what it was planted to contain, and gate 13 may not be cited until this is 0 again; 2 the harness could not prove what it claims, which covers eight states worth telling apart in the output (it could not tell agreement from disagreement, every case disagreed so a comparison is inverted somewhere and the run cannot say where, the workflow no longer runs the gate command where its failure would fail the run, it no longer runs this script there, this script could not find itself in the workflow, the `gate-13` job's checkout no longer asks for the whole history, a planted input is no longer planted, or an answer was given for a reason that is not the planted one); 3 a prerequisite is missing so no gate command ran at all, which covers git, awk, jq or `scripts/gates.sh` being unusable here and the planted repositories failing to build. `.github/workflows/ci.yml` runs it in the `gate-13-proof` job on every pull request.

Exit 0 is a statement about the run, not about a merge. `ci` is not a required status check on `main`, so a red run here blocks nothing yet; `ops/gates/branch-protection.md` records why there are no required checks and when that changes.

## The five questions, because "the gate caught it" is five claims

ORI-T-0013, ORI-T-0014 and ORI-T-0016 found defects between them in the harnesses for gates 1, 2 and 7, every one a code path reporting success for a state that is not success. Their questions are asked here, in their shape.

1. **Is this a command CI runs?** Stage 4. A command in a job the required aggregate does not wait on, in a job or step carrying `if:` or `continue-on-error:`, in a workflow no pull request fires, or sitting as text inside another step's shell script, is a command whose failure reaches nobody. Every one of those leaves the command string byte-identical, so the question cannot be answered by grepping for it. That is the same discipline one layer up from the gate itself: a `run:` line that is really a here-document reads the same to a grep as a `Ticket:` line that is really body text.
2. **Does gate 13 give each input the verdict it owes?** Stages 8 and 9. One planted input per row, judged by one comparator run twice over one set of observations with every expectation flipped, requiring per-row complementarity.
3. **Is the answer the answer to the planted question?** Stage 6 records and stage 8b judges. A non-zero exit is not evidence on its own: gate 13 exits 1 when a commit does not conform and 2 when it could not name the commits at all, and a row that accepted any non-zero answer would count the second as the first. Each row names the reason codes and the sentences its planted mechanism leaves in the output, including signatures that must be **absent**.
4. **Is the job that asks the first three itself live?** Stage 4b. `continue-on-error: true` on `gate-13-proof` turns every refusal below into a green check: the harness still runs, still refuses, still writes its annotation, and GitHub records the job as a success. The same six structural conditions are asked of this script's own job, which it finds by the command that invokes it **and** by name, with both required to agree.
5. **Is the plant still planted?** Stage 3b. Eleven rows are planted to be passed, and a pass looks the same whether the plant is there or not. The trap message in particular is a pass for a regex and a refusal for the gate only while its `Ticket:` and `Spec:` lines sit in a paragraph that is not the last one. Every planted message is asserted against its source file before any gate runs.

Stage 4c adds a sixth this gate shares with gate 7: **is the history actually on the runner?** `actions/checkout` defaults to `fetch-depth: 1`, and in that clone the base SHA a `pull_request` payload names is simply not an object the repository holds. A gate that enumerated "whatever is present" would check a shorter range and report on it as though it were the whole pull request. Gate 13 refuses instead, and stage 4c reads `fetch-depth` out of the `gate-13` job so the reason is named here rather than met as a surprise refusal on somebody's pull request.

## Five things about this fixture that are load bearing and look like details

1. **The inversion is the dangerous part.** This job's success condition is the opposite of the job it proves: most rows pass when the gate command fails or refuses. Getting that backwards produces a job that passes on every input, which is the exact defect AICD §14 names. Four things guard it and none is a comment: the table is not all failures and not all passes (eleven rows disagree with a harness that reported failure for everything); the comparator runs twice with every expectation flipped and must reach opposite conclusions **per row**, not by count, because `ops/gates/gate-1.md` records a real fixture defect that made a count-based check report the harness itself broken; every checker is exercised over its own sample table before it is trusted; and the plants are asserted rather than assumed.

2. **Observation is separated from judgement.** The gate runs once per row in stage 6, its exit status and output are recorded, and every judgement after that is a pure function of what was recorded. A harness that re-ran the gate inside its comparator could not be shown to be judging the same observation twice.

3. **There is no pipeline anywhere in `run_gate`.** `ops/gates/branch-protection.md` records a failure of this project's own making: a proof harness wrote `if git push ... | tee log | tail -6; then`, and a shell pipeline returns the status of its **last** command, so the test read `tail`, which always succeeds, and reported that a refused push had succeeded. The gate command's output is redirected to a file and `$?` is read on the next line. `set -e` is deliberately not used, because most rows expect a non-zero status and an errexit shell that leaves before the verdict is a gate that exits 0 without checking anything; the `EXIT` trap turns any such early departure into exit 2.

4. **The planted runs have `GITHUB_ACTIONS` unset, and this harness's own annotations are not suppressed.** `scripts/gates.sh` emits `::error::` annotations addressed to the author of the commit under review. The commits under review here exist only inside this run, in a temporary directory, so their annotations would land on the pull request as thirty-odd lines reading "gate 13 failed" about commits nobody wrote. The annotations `emit` writes are the ones a human is meant to see and they stay.

5. **`workflow-facts.awk` is a copy of gate 7's, which is a copy of gate 1's, and the copy is deliberate.** Gate 1 is an installed gate, so a shared parser would make every edit to it an edit to gate 1's proof and stop that proof being reproducible from gate 1's own directory; and a parser bug would make all four provers refuse on the same commit, turning one repairable failure into four red gates with no second opinion. What a copy costs is that a defect found in one is not fixed in the others, and that cost is paid deliberately: each copy is exercised over its own `workflow-samples/` table on every run, so a divergence shows up as a sample disagreeing in the prover that owns the copy, not as silence. This copy already carries one repair the others needed: `dead-command-is-block-scalar-text.yml` writes `run: echo 'gate 13 is disabled'` with no colon inside the plain scalar, because a plain YAML scalar may not contain a colon followed by a space and gate 1's and gate 2's copies of that sample were for a time files no runner could have loaded (`fixtures/planted/gate-2/README.md`, note 6; ORI-T-0084 repaired gate 1's).

All five are ways this fixture could go quiet while still looking correct, which is the defect class of AICD §14. Anyone editing these files should re-run `prove.sh` and read the verdict, not the diff. Deleting this directory disarms gate 13's proof with nothing failing to say so (`CLAUDE.md`, "`fixtures/planted/` has one directory per gate that has been proven").
