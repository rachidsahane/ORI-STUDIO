# Proof: CI gate 1, `fmt` and `clippy -D warnings`

| | |
|---|---|
| Gate | CI_CD section 1, item 1 |
| Commands | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Ticket | ORI-T-0013 |
| **State** | **Installed** |
| Written by | The lead, from the coder's verified evidence (ruling R25) |

`spec/DATA_MODEL.md` gives a gate the states `Defined -> Proven (GateProof present) -> Installed`. All three of AICD §14's conditions are now met, on observed CI runs, and they are recorded below. Gate 1 may be cited as protection.

## What AICD §14 asks for, and which parts this establishes

The rule: a gate enters service only after a demonstration on a planted defect, in which it passes on a clean tree, fails on the planted defect, and **the failure is visible where a human would look**.

All three are established. The first two by the demonstration below; the third by an observed run, because no local run can watch the pull request check it writes to.

## The planted defects

Three cargo packages under `fixtures/planted/gate-1/`, each its own workspace root through an empty `[workspace]` table, so the repository workspace never reaches them. They were put there rather than in an `exclude` entry of the root manifest because the root manifest is lock claim 10 and outside this ticket's scope.

| Package | What is planted | Gate 1 owes it |
|---|---|---|
| `clean/` | nothing | pass on both halves |
| `fmt-defect/` | `pub fn sum(a:i32,b:i32)->i32{a+b}` | fail `fmt`, pass `clippy` |
| `clippy-defect/` | `values.len() == 0`, a `clippy::len_zero` violation | pass `fmt`, fail `clippy` |

The clean package is not decoration. Without an input the gate must pass, a run cannot distinguish a working gate from one that fails on everything.

## Passes clean, fails on the planted defect

Verified by the lead in the worktree, each exit status captured directly with no pipeline:

| Command | Exit |
|---|---|
| `cargo fmt --all --check` in `clean/` | 0 |
| `cargo fmt --all --check` in `fmt-defect/` | **1** |
| `cargo clippy --all-targets --locked -- -D warnings` in `clippy-defect/` | **101** |

And the repository's own build stays green with all three packages present: `cargo build --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all --check`, `cargo test --workspace --locked`, `bash scripts/gates.sh` and `bash fixtures/planted/gate-1/prove.sh` all exit 0.

## The proof is a job, not only a record

`fixtures/planted/gate-1/prove.sh` runs on every pull request as the `gate-1-proof` job. A proof file alone records that somebody once saw the gate fail, and stays reassuring on the day the gate is weakened, which is §14's own defect class applied to the proof. The job re-presents the same defects against the commit under review.

That job's success condition is inverted relative to every other job in the file: it passes when the gate commands fail. Getting an inversion wrong produces a job that passes on everything, so the harness separates observation from judgement, runs the six gate commands once, then judges those observations twice through one comparator, once against the real expectations and once against every expectation flipped, requiring per-row complementarity. A comparator that always agrees agrees in both passes and is caught.

## Four defects found in this ticket, all the same class

Every one was found by constructing the bad state and running it. None was found by reading.

| Found | Defect |
|---|---|
| Review, attempt 1 | The prover treated any non-zero exit as "gate 1 caught it". A fixture with a syntax error, a missing `src/lib.rs` or a malformed manifest produced a successful proof. The gate would have been certified on evidence that never involved the gate. |
| Review, attempt 1 | The precondition was a whole-file grep for the command strings. A `run:` line matches while belonging to a job absent from the aggregate's `needs:`, gated by a false `if:`, or unreachable. **Gate 1's `fmt` half could be switched off entirely and the prover still reported the proof holds.** |
| Review, attempt 2 | `ci.yml` stated "gate 1, INSTALLED" while this file said installation was incomplete. Two files in one diff disagreeing about whether a control exists. |
| Review, attempt 2 | **The prover rejected `continue-on-error:` on every job except its own.** One line on `gate-1-proof` turned every refusal it could issue into a green check: the whole demonstration switched off while every check on the pull request stayed green. |

The last is the same defect class at one more level of recursion: the check that proves the gate cannot be weakened was itself weakenable by exactly the mechanism it rejected in others.

## What the prover now establishes about the workflow

It parses `.github/workflows/ci.yml` rather than grepping it, and requires of each gate command, and of its own job: the command is the `run:` of a step of a named job; neither job nor step carries `if:` or `continue-on-error:`; the job is in the `ci` aggregate's `needs:`; the aggregate carries no `continue-on-error:`; and the workflow fires on an unfiltered `pull_request`. Any shape the parser cannot read is a refusal, not a pass.

Verified by the lead against the shipped harness, exit captured without a pipeline:

| Planted weakening of `ci.yml` | Exit |
|---|---|
| none, control | 0 |
| `continue-on-error: true` on job `gate-1-proof` | **2** |
| job `gate-1-proof` renamed | **2** |

## The failure is visible where a human would look

§14's third condition, established by observation rather than argument.

**Run 35598542173**, the `pull_request` trigger on this ticket's own pull request: twelve jobs, all green, `gate-1-proof` among them. Gate 1 passes on a clean tree and the proof job reports its verdict in the check log.

**Run 35598700908**, on a throwaway branch carrying one deliberate formatting violation in `crates/ori-core`, opened as a pull request and then closed without merging:

| Job | Result |
|---|---|
| `fmt` | **failure** |
| `ci` (aggregate) | **failure** |
| everything else, including `gate-1-proof` | success |

What a human sees on the pull request is `fmt fail` and `ci fail`, each linking to the job log naming the file and the line. `gate-1-proof` stayed green, correctly: its planted fixtures were untouched, and its job is to verify the gate catches those, not to catch this one.

The branch was deleted and the pull request closed. The runs persist as the evidence.

## What this proof still does not establish

1. **That a red `ci` blocks anything.** See below. Visibility and blocking are different claims and only the first is §14's.

2. **That a failing `ci` blocks a merge.** It does not, today. `main` has no required status checks at all: `ops/gates/branch-protection.md` records that as deliberate, because a required check the branch cannot produce leaves every pull request pending forever, and each name is added on the day its proof lands. Until `ci` is required, a red run colours the pull request and stops nothing. **Adding `ci` to the required checks is the operator's action and is owed now that this proof exists.**

3. **That the recursion terminates.** A `continue-on-error:` added to `gate-1-proof` in a later commit makes the harness refuse, but GitHub reports a job carrying that key as success to `needs`, so `ci` still passes. The refusal becomes an annotation, and the last link is a human reading the diff of a tier 2 file. Closing this needs the `ci` aggregate to assert that no job or step in the workflow carries `continue-on-error:`. That is one edit to an existing step and it is a follow-up ticket, not this one.

4. **That gate 1 has ever failed on this repository's own workspace.** The demonstration runs inside the planted packages. The workspace halves are the `fmt` and `clippy` jobs, which have only been seen passing.

5. **That the awk parser behaves identically under every awk.** It was run under one, `one-true-awk 20200816` on macOS. It is written to POSIX awk with no GNU extensions. Linux and Windows runners will exercise it when this lands.

## Rollback

`git revert` of the merge commit. Gate 1 returns to `Defined`: the `fmt` and `clippy` jobs keep running, and nothing may cite them as protection.
