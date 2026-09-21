# Planted inputs for gate 2 (`cargo test --workspace --locked`)

Ticket: ORI-T-0014. Spec anchor: `spec/CI_CD.md` section 1 gate 2, `spec/TESTING.md` sections 1 and 4, `spec/runbooks/prove-gate.md`.
Proof: `ops/gates/gate-2.md`. Rule: AICD §14, "a gate is installed only when it has been seen to fail".

## Criterion

`ORI-P1-013` (`spec/criteria/phase-1.md`): *precondition* gate installed, its planted defect present; *action* `gates.prove`; *expected result* passes clean, fails dirty, proof stored with evidence refs, gate state Installed.

That criterion is written against the `gates.prove` tool in `crates/ori-gates/src/prover.rs`, which does not exist yet; `ops/phase-1-backlog.md` assigns ORI-T-0014 no criterion of its own for that reason. `fixtures/planted/gate-2/prove.sh` is what `gates.prove` will drive for this gate, and it is the input the criterion's "its planted defect present" clause refers to. When the tool lands, the test that names `ORI-P1-013` runs against this directory.

## Why this fixture is shaped differently from `fixtures/planted/gate-1/`

Gate 1 checks a property of the code: the tree is formatted, the tree is lint clean. **Gate 2 checks a property of the tests, and a test suite can be green and prove nothing.**

That is not a worry about the gate. It is a property of the tool the gate is built on, and it is reproducible in four ways:

- `cargo test` exits 0 on a package with no tests at all. It prints `running 0 tests`, prints `test result: ok`, and succeeds.
- It exits 0 on tests that assert nothing.
- It exits 0 when every test carries `#[ignore]`, printing `0 passed; 0 failed; 3 ignored` and `test result: ok`.
- `--workspace` is exactly as wide as the root manifest's `members` list. This repository's root `Cargo.toml` names its seventeen crates by hand and uses no globs, so a crate added under `crates/` and left out of that list is a crate whose failing tests never run.

In all four the gate reports success and the thing it is supposed to protect is absent. That is AICD §39's "present but reporting nothing", reproduced inside the tool rather than in a workflow file.

So this fixture plants both directions, and `prove.sh` **records both**. It does not add a check that would make gate 2 catch the last four. Gate 2 is `cargo test --workspace --locked` and nothing else, and what those four inputs establish is what gate 2 does not establish. `ops/gates/gate-2.md` states it, and `prove.sh` prints it in its verdict on every run so that the reader of a CI log is told as plainly as the reader of the proof file.

## What is here

| Path | Gate 2 owes it | Class | What is planted in it |
|---|---|---|---|
| `clean/` | pass | control | Nothing. A correct `remaining_budget`, tested at all three levels `cargo test` runs: a unit test, an integration test under `tests/`, and a documentation example. |
| `failing-unit/` | **fail** | catches | The library ignores the spend; the unit test beside it asserts the right answer. `cargo test` exits 101. |
| `failing-integration/` | **fail** | catches | The same defect, with the test that catches it under `tests/`, which cargo builds as its own binary. No unit test and no doc example here, so the integration binary is the only thing that can fail. |
| `failing-doctest/` | **fail** | catches | The same defect, with the test that catches it written as a documentation example. Doc tests run last, under their own heading, from the library target. |
| `no-tests/` | pass | blind | The same defective library and no test of any kind: no `#[cfg(test)]` module, no `tests/` directory, no doc example. `running 0 tests`, `test result: ok`, exit 0. |
| `vacuous-tests/` | pass | blind | The same defective library and three tests that run and assert nothing about what it returned. `3 passed; 0 failed`, exit 0. |
| `ignored-tests/` | pass | blind | The same defective library and three tests that would catch it, every one carrying `#[ignore]`. `0 passed; 0 failed; 3 ignored`, exit 0. |
| `unlisted-member/` | pass | blind | A workspace whose `members` names `listed` only. `unlisted/` sits beside it, is a package, has a failing test, and is not a member and not excluded. `1 passed; 0 failed`, exit 0, and cargo never mentions `unlisted` at all. |
| `prove.sh` | n/a | | The harness. Runs the gate command against each package, compares the eight verdicts against the eight this table owes, checks that each answer is attributable to the mechanism planted for it, checks that every plant is still planted, checks that `.github/workflows/ci.yml` still runs the gate command somewhere a failure would fail the run, checks that the `gate-2-proof` job which runs the harness is itself one whose failure fails the run, and checks that its own comparator can tell agreement from disagreement. |
| `workflow-facts.awk` | n/a | | Reads a GitHub Actions workflow file and prints its structure. `prove.sh` asks its questions of that structure rather than of the file's text. |
| `workflow-samples/` | n/a | | Sixteen whole workflow files: twelve for the liveness check (one live, ten dead in ten different ways with the command string left byte-identical, one the parser refuses to read) and four named `self-*` for the check that the harness's own job can fail the run. |

`remaining_budget` is the same function in all eight packages, and the defective copies differ from `clean/` by exactly one thing: they return the limit whatever has been spent. Keeping one subject is what makes a verdict attributable to the change.

## Why `workflow-facts.awk` is a copy and not a shared file

It is a byte-identical copy of `fixtures/planted/gate-1/workflow-facts.awk` below the header comment, and the copy is deliberate. ORI-T-0014 was asked to factor what generalises unless factoring would couple the two gates so that breaking one breaks the other. It would, in both directions: gate 1 is an installed gate, so a shared parser makes every edit to it an edit to gate 1's proof and stops that proof being reproducible from gate 1's own directory; and a parser bug would make both provers refuse on the same commit, turning one repairable failure into two red gates with no second opinion. The shared location would also be a new path outside this ticket's declared scope, and gate 1's copy is outside it too.

What a copy costs is that a defect found in one is not fixed in the other. That cost is paid deliberately: each copy has its own `workflow-samples/` table, and each prover exercises its own copy over its own table on every run before it believes anything the parser says. A divergence shows up as a sample disagreeing in the prover that owns the copy, not as silence.

## Running it

```
bash fixtures/planted/gate-2/prove.sh
```

Exit 0 the proof holds; 1 gate 2 stopped answering an input the way `ops/gates/gate-2.md` records; 2 the harness could not prove what it claims, which covers seven states worth telling apart in the output (it could not tell agreement from disagreement, every case disagreed so a comparison is inverted somewhere and the run cannot say where, the workflow no longer runs the command under proof where its failure would fail the run, the `gate-2-proof` job that runs the harness is no longer one whose failure fails the run or can no longer be found in the workflow at all, it could not read a fixture's code apart from the fixture's prose, a planted input no longer contains what it was planted to contain, or an input answered for a reason other than the planted one); 3 the toolchain made gate 2 unrunnable, so nothing was checked. `.github/workflows/ci.yml` runs it in the `gate-2-proof` job on every pull request.

Exit 0 is a statement about the run, not about a merge. `ci` is not a required status check on `main`, so a red run here blocks nothing yet; `ops/gates/branch-protection.md` records why and when that changes.

## Why these packages do not turn the repository red

Each top-level directory here is its own workspace root, through an empty `[workspace]` table or, for `unlisted-member/`, through a `[workspace]` of its own. The repository's root `Cargo.toml` lists its members explicitly and uses no globs, so `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --all` and `cargo clippy --workspace` run from the repository root never reach them. The `test` job in `.github/workflows/ci.yml` is the continuous check on that: if a planted package with a failing test ever became visible to the root workspace, `test` would go red, and the `ci` aggregate waits on it.

## The five questions, because "the gate answered correctly" is five claims

1. **Is this a command CI runs?** A command in a job the required aggregate does not wait on, in a job or step carrying `if:` or `continue-on-error:`, in a workflow no pull request fires, or sitting as text inside another step's shell script, is a command whose failure reaches nobody. Every one of those leaves the command string byte-identical in the file, so the question cannot be answered by grepping for it. `workflow-facts.awk` parses the file and `prove.sh` asks the structure.
2. **Does gate 2 give each input the verdict it owes?** Eight cases, one comparator, run twice over one set of observations with every expectation flipped the second time.
3. **Is the answer the answer to the planted question?** An exit status is not evidence on its own: a package that no longer compiles, or a `Cargo.lock` that `--locked` rejects, exits non-zero and neither is a failing test. Each row names the signatures its planted mechanism leaves in the output, including signatures that must be **absent**, and an answer without them proves nothing.
4. **Is the job that asks the first three itself live?** `continue-on-error: true` on `gate-2-proof` leaves the harness running, refusing and annotating, and GitHub records the job as a success. The same six conditions are asked of that job, which the harness finds by the command that invokes it and by name, refusing when the two disagree.
5. **Is the plant still planted?** This one gate 1's fixture did not need. Its planted defects announce themselves in the gate's own output: a formatting diff, a named lint. Four of the eight inputs here are planted to be **passed**, and a pass looks identical whether the plant is there or not. Repair `no-tests/src/lib.rs` by giving it a test and the row still agrees while the thing this fixture exists to demonstrate has quietly gone. So the plants are asserted against the sources, and a missing plant is a refusal.

## Six things about this fixture that are load bearing and look like details

1. **The plant assertions read the code, not the file, and not one line at a time.** These packages explain themselves at length, so `no-tests/src/lib.rs` says in its documentation that it carries no `#[cfg(test)]` module, and `ignored-tests/src/lib.rs` says that every one of its tests carries `#[ignore]`. The first version of stage 3b grepped the whole file and found those words in the prose: it reported that `no-tests` had acquired a test, and counted four `#[ignore]` against three `#[test]` in a file whose three tests are all ignored. Both refusals were correct about the bytes and wrong about the fixture. `code_of` strips line comments first, and refuses outright on a file containing a block comment rather than reading one as code.

   The second version read the stripped code one line at a time, which is what `grep` does, and `vacuous-tests` is the one input whose plant is the **absence** of something. An assertion wrapped the way rustfmt wraps a long one,

   ```
   assert_eq!(
       remaining_budget(3, 10),
       7
   );
   ```

   matched nothing. The input stopped being vacuous, the plant check reported it still planted, and the refusal that followed came from the comparator two stages later and blamed gate 2 for a fixture that had been repaired. `joined_code_of` removes the newlines before the match, and the two shapes are a self-check of their own: stage 3b runs the reader over a file whose only assertion is in a comment and a file whose assertion is spread over four lines, owed opposite answers, before it reads any fixture.

2. **No signature names `Compiling`.** The obvious signature for `unlisted-member` was `Compiling gate-2-listed`, which cargo prints on a cold build and not on a warm one, so the attribution passed on a fresh checkout and refused on the second local run. A signature that answers differently depending on a build cache is not a signature of the planted mechanism. What is the planted mechanism there is that cargo never mentions `gate-2-unlisted` in any state, which is a signature only a negation can express.

3. **`ignored-tests` is not run with `--include-ignored`, and `unlisted-member` has no `exclude` entry.** Either would repair the input. The fixture's job is to record what the gate does, not to fix it.

4. **The files under `workflow-samples/` are inputs, not workflows.** They live under `fixtures/` and not under `.github/workflows/`, so GitHub never registers or runs them. Deleting one, or changing what it plants without changing the table in `prove.sh` that says what each is owed, turns a self-check into a check nobody has seen answer both ways.

5. **Each sample differs from `live.yml` in exactly one way, and each one is dead for that one reason.** `prove.sh` compares the classification and not the reason, so a sample that was dead twice over would agree with its row while testing nothing about the mechanism it is named for. What keeps that honest is the minimality, which is a property of the files and is checked by reading `diff live.yml <sample>`, plus the reason `classify_command` prints, which was read once against all twelve.

6. **Every sample is valid YAML, and one of them was not.** A sample is a whole workflow file, so a sample GitHub would reject is a sample whose construction could never arise. `workflow-facts.awk` does not enforce YAML and never noticed: `dead-command-is-block-scalar-text.yml` carried `run: echo 'gate 2: disabled'`, which is a parse error, because a plain YAML scalar may not contain a colon followed by a space. It was classified `dead` for the right reason, over a file no runner could have loaded. `fixtures/planted/gate-1/workflow-samples/` carried the same defect in its own copy; ORI-T-0084 repaired it. Nothing in `prove.sh` checks this: it would need a YAML parser, and adding one is a dependency this ticket was not given. It is checked by hand and recorded here.

All six are ways this fixture could go quiet while still looking correct, which is the defect class of AICD §14. Anyone editing these files should re-run `prove.sh` and read the verdict, not the diff.
