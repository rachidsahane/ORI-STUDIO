# Planted defects for gate 1 (`fmt`, `clippy -D warnings`)

Ticket: ORI-T-0013. Spec anchor: `spec/CI_CD.md` section 1 gate 1, `spec/TESTING.md` section 4, `spec/runbooks/prove-gate.md`.
Proof: `ops/gates/gate-1.md`. Rule: AICD §14, "a gate is installed only when it has been seen to fail".

## Criterion

`ORI-P1-013` (`spec/criteria/phase-1.md`): *precondition* gate installed, its planted defect present; *action* `gates.prove`; *expected result* passes clean, fails dirty, proof stored with evidence refs, gate state Installed.

That criterion is written against the `gates.prove` tool in `crates/ori-gates/src/prover.rs`, which does not exist yet; `ops/phase-1-backlog.md` assigns ORI-T-0013 no criterion of its own for that reason. `fixtures/planted/gate-1/prove.sh` is what `gates.prove` will drive for this gate, and it is the input the criterion's "its planted defect present" clause refers to. When the tool lands, the test that names `ORI-P1-013` runs against this directory.

## What is here

| Path | Gate 1 owes it | What is wrong with it |
|---|---|---|
| `clean/` | pass on both halves | nothing. It is the control. |
| `fmt-defect/` | fail `cargo fmt --all --check`, pass clippy | `src/lib.rs` declares `pub fn sum(a:i32,b:i32)->i32{a+b}`: no space after either parameter colon, none around `->`, and the body on the declaration line. `rustfmt` rewrites it. It compiles and no lint fires on it. |
| `clippy-defect/` | pass fmt, fail `cargo clippy ... -D warnings` | `src/lib.rs` writes `values.len() == 0` where `clippy::len_zero` wants `values.is_empty()`. The lint is warn by default, so `-D warnings` makes it an error. It is `rustfmt` clean and `rustc` compiles it without a word. |
| `prove.sh` | n/a | The harness. Runs each half of gate 1 against each package, compares the six verdicts against the six this table owes, checks that each failure is the failure planted for it, checks that `.github/workflows/ci.yml` still runs those commands somewhere a failure would fail the run, checks that the `gate-1-proof` job which runs the harness is itself one whose failure fails the run, and checks that its own comparator can tell agreement from disagreement. |
| `workflow-facts.awk` | n/a | Reads a GitHub Actions workflow file and prints its structure: triggers, jobs, job keys, `needs:`, steps, step keys and single-line `run:` values. `prove.sh` asks its questions of that structure rather than of the file's text. |
| `workflow-samples/` | n/a | Sixteen whole workflow files. Twelve are the planted defects for the liveness check: one where gate 1's command is live, ten where it is switched off in ten different ways with the command string left byte-identical, and one whose shape the parser refuses to read. Four more, named `self-*`, are the planted defects for the check that the harness's own job can fail the run: one live, one with `continue-on-error:` on that job, one where the job is renamed, and one where two jobs run the harness. `prove.sh` classifies all of them on every run before it says anything about the real workflow. |

Each defect breaks exactly one half of gate 1, so a failure is attributable to the half it is aimed at. A file that broke both would prove neither.

## Why these packages do not turn the repository red

Each `Cargo.toml` here carries an empty `[workspace]` table, which makes its directory a workspace root of its own. The repository's root `Cargo.toml` lists its members explicitly and uses no globs, so `cargo build --workspace`, `cargo fmt --all` and `cargo clippy --workspace` run from the repository root never reach these packages. The `fmt` and `clippy` jobs in `.github/workflows/ci.yml` are the continuous check on that: if a planted package ever became visible to the root workspace, both would go red, and the `ci` aggregate waits on both of them. `ci` is not yet a required status check on `main`; `ops/gates/branch-protection.md` records why and when it becomes one.

## Running it

```
bash fixtures/planted/gate-1/prove.sh
```

Exit 0 the proof holds; 1 gate 1 stopped behaving the way the proof claims; 2 the harness could not prove what it claims, which covers four states worth telling apart in the output (it could not tell agreement from disagreement, the workflow no longer runs the command under proof where its failure would fail the run, the `gate-1-proof` job that runs the harness is no longer one whose failure fails the run or can no longer be found in the workflow at all, or a planted input failed for a reason other than the defect planted in it); 3 the toolchain made gate 1 unrunnable, so nothing was checked. `.github/workflows/ci.yml` runs it in the `gate-1-proof` job on every pull request.

Exit 0 is a statement about the run, not about a merge. `ci` is not a required status check on `main`, so a red run here blocks nothing yet; the verdict `prove.sh` prints separates what the run established from what it did not, and `ops/gates/branch-protection.md` records why there are no required checks and when that changes.

## The four questions, because "the gate caught it" is four claims

1. **Is this a command CI runs?** A command in a job the required aggregate does not wait on, in a job or step carrying `if:` or `continue-on-error:`, in a workflow no pull request fires, or sitting as text inside another step's shell script, is a command whose failure reaches nobody. Every one of those leaves the command string byte-identical in the file, so the question cannot be answered by grepping for it. `workflow-facts.awk` parses the file and `prove.sh` asks the structure.
2. **Does gate 1 give each input the verdict it owes?** Six cases, one comparator, run twice over one set of observations with every expectation flipped the second time.
3. **Is the failure the one this proof planted?** A non-zero exit is not evidence on its own: an unparsable `rustfmt.toml`, a syntax error or a broken manifest all produce one. Each row names the signature its planted mechanism leaves in the output, and a failure without that signature proves nothing and is reported as such.
4. **Is the job that asks the first three itself live?** Question 1 was asked about the `fmt` and `clippy` jobs and not about `gate-1-proof`, the job that runs the harness. `continue-on-error: true` on that one line leaves the harness running, refusing and annotating, and GitHub records the job as a success: the whole demonstration switched off by an edit the harness used to accept in silence. The same six conditions are now asked of that job, which the harness finds by the command that invokes it and by name, refusing when the two disagree rather than quietly ceasing to check itself.

The first and the third were added in the second attempt at this ticket, after review broke the first version of the harness in both places: gate 1's fmt half was switched off entirely while `prove.sh` reported that the proof held, and both planted packages were replaced with defects gate 1 does not check while `prove.sh` reported that it had caught them. The fourth was added in the third attempt, after review switched off the job that runs the harness and the harness reported that the proof held.

## Three things about this fixture that are load bearing and look like details

1. **The function in all three packages is named `holds_nothing` and not `is_empty`.** `clippy::len_zero` suppresses itself inside an item named `is_empty`, because its own suggestion would be the recursive call there. The first version of `clippy-defect` did name it `is_empty`, and `cargo clippy -- -D warnings` exited 0 on a package planted to fail it. `ops/gates/gate-1.md` records that run.
2. **There is no `#[rustfmt::skip]` anywhere in `fmt-defect`.** It would make `rustfmt` leave the planted line alone and gate 1 would report a pass on a defect that was never presented to it.
3. **The files under `workflow-samples/` are inputs, not workflows.** They live under `fixtures/` and not under `.github/workflows/`, so GitHub never registers or runs them. They exist to be read by `workflow-facts.awk`. Deleting one, or changing what it plants without changing the table in `prove.sh` that says what each is owed, turns the liveness check into a check nobody has seen answer both ways.

All three are ways this fixture could go quiet while still looking correct, which is the defect class of AICD §14. Anyone editing these files should re-run `prove.sh` and read the verdict, not the diff.
