# Proof: CI gate 2, `cargo test`, workspace-wide

| | |
|---|---|
| Gate | CI_CD section 1, item 2 |
| Command | `cargo test --workspace --locked` |
| Ticket | ORI-T-0014 |
| **State** | **Installed** |
| Written by | The lead, from the coder's verified evidence (ruling R25) |

Gate 2's `test` job already ran on three platforms and had only ever been seen passing, so under AICD §14 it was not installed. This records the demonstration. What is owed for Installed is at the end.

## What a green gate 2 establishes, and it is less than it looks

This is the finding of the ticket and it is not an inference. It is the observed behaviour of the gate command on four inputs planted for it, each run once, exit status captured without a pipeline.

| Planted input | What is wrong with it | Gate 2 |
|---|---|---|
| `no-tests` | a defective library with no unit test, no `tests/`, no doc example | **exit 0**, `running 0 tests` |
| `vacuous-tests` | the same defective library, three tests that call the function and assert nothing about what it returned | **exit 0**, 3 passed |
| `ignored-tests` | the same defective library, three tests that would catch the defect, every one `#[ignore]`d | **exit 0**, 0 passed 0 failed 3 ignored |
| `unlisted-member` | a workspace whose `members` names one package, with a sibling package that has a failing test and is neither a member nor excluded | **exit 0**, and cargo never names the package in any state |

In all four the gate reports success and the thing it is supposed to protect is absent. That is AICD §39's "present but reporting nothing", reproduced inside the tool the gate is built on rather than in a workflow file.

**So a green gate 2 establishes two things and no more:** every test the root manifest's `members` list reaches, that is not `#[ignore]`d, and that asserts something, passed on that runner; and the workspace compiles under `cfg(test)` on that runner. It establishes nothing about correctness or coverage, because it cannot distinguish a suite that proves something from no suite at all.

`unlisted-member` is the one with teeth here. **"Workspace-wide" in CI_CD's definition of gate 2 is a claim about the `members` list, not about the tree.** A crate added without a members entry is silently untested and nothing says so. This repository is not exposed today, 17 members against 17 packages on disk and no orphans, verified by the lead, but it is one forgotten entry away and no control watches for it.

## What would make gate 2 mean more, and it does not exist yet

Elsewhere in CI_CD section 1, and both unavailable:

- **Gate 4, the coverage matrix**, answers "does a test exist" by requiring every criterion to have a test and every test to name a criterion. `scripts/gates.sh` reports it unavailable for want of `crates/ori-gates/src/coverage_matrix.rs`.
- **Gate 6, the mutation score threshold**, answers "does the test assert anything that matters". Unavailable for want of `cargo-mutants`.

Until they exist, **no document may cite a green gate 2 for more than the two lines above.** This is a finding about the gate set, not a failure of the gate: gate 2 does what its definition says, and its definition is narrow.

## Passes clean, fails on the planted defect

Gate 2 caught every input carrying a real failing test, at each of the three levels `cargo test` runs:

| Planted input | Gate 2 |
|---|---|
| `clean`, correct and tested at all three levels | exit 0 |
| `failing-unit` | **exit 101** |
| `failing-integration`, under `tests/` | **exit 101** |
| `failing-doctest` | **exit 101** |

Eight packages in total, each its own workspace root so the repository build never reaches them. The repository's own build stays green with all eight present.

## The proof is a job

`gate-2-proof` runs `fixtures/planted/gate-2/prove.sh` on every pull request. It reuses the shape ORI-T-0013 paid four defects and three attempts to arrive at: observation separated from judgement, one comparator run twice over one observation set with every expectation flipped, the workflow parsed rather than grepped, attribution of each failure to its planted mechanism, and the harness asserting **its own job** is live by the same six structural conditions it applies to the job it proves.

`workflow-facts.awk` is a deliberate copy of gate 1's rather than a shared file. Factoring it would couple the two gates so that breaking one breaks the other, and a proof that fails when an unrelated gate is edited is a proof nobody will trust.

## The failure is visible where a human would look

AICD §14's third condition, established by observation.

**Run 35608800130**, this ticket's own pull request: thirteen jobs green including `gate-2-proof`.

> **Corrected.** This line first cited run **35598542173**, which is ORI-T-0013's pull request: twelve jobs, `gate-1-proof`, and **no `gate-2-proof` at all**. `ops/gates/gate-1.md` cites that run correctly; the lead copied its identifier into this file and paired it with this gate's own, accurate description. So the description was written from a real observation and the citation pointed at a run that contradicts it, and gate 2 was moved to **Installed** on that pairing.
> The evidence itself was never in doubt: run 35608800130 is green with `gate-2-proof` among thirteen jobs, and the red run below is correct. Both were re-verified against the version control host before this correction was written. The state line stands; the citation did not.
> Found by the round 4 audit ([[CR-007]]). It is the sharpest instance of the class yet recorded, because a reader who followed the citation would have found a run that disproves the sentence citing it.

**Run 35609765474**, a throwaway branch carrying one deliberately failing test in `crates/ori-gates`, opened as a pull request and closed without merging: **`test` failed on ubuntu, macos and windows, and the `ci` aggregate failed.** What a human sees on the pull request is three red `test` checks and a red `ci`, each linking to the assertion that failed. The branch was deleted; the run persists.

## What is not established

1. **That a red `ci` blocks anything.** `main` still has no required status checks. Gate 1's proof already records this as owed to the operator; gate 2 makes it owed twice.
2. **That the four blind inputs are the only ones.** They are the four the ticket looked for. `cargo test` may be blind in ways nobody has planted for yet.

## Rollback

`git revert`. Gate 2 returns to `Defined`: the `test` job keeps running on three platforms, and nothing may cite it as protection.
