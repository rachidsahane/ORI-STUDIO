# inert-gate: a repository whose workflow is present and never fires

This directory is a **test fixture**: an AICD product the end-to-end suite and the liveness gate run against. It is not an example and not scratch. Do not tidy it.

Ticket: ORI-T-0075. Spec anchor: `spec/TESTING.md` section 5 ("a repository whose workflow is present but never fires, for the liveness gate and the 'present but reporting nothing' defect class"). Related: `spec/CI_CD.md` section 1 (the liveness gate "fails loudly if any required workflow has not executed inside its window"), `spec/PRD.md` Z-02 and O-06, AICD §14 and AICD §39.

Criteria it serves: none in phase 1. The liveness gate is built by ORI-T-0046 (`crates/ori-gates/src/liveness.rs`) and asserted in phase 4. This is the input that gate is proven against: a gate installed without having been seen to fail on a planted defect is not installed (AICD §14).

## The defect, in one paragraph

`.github/workflows/ci.yml` is a valid GitHub Actions workflow with a job that runs a check. Its triggers are `push` to `master` and `pull_request` into `master`. This repository has one branch, `main`, and never had `master`. Neither event can occur, so the workflow has never run and cannot run. Every document that cites it as the pipeline is citing something that protects nothing, and nothing in the repository says so except the absence of runs. The liveness gate has to find that absence.

The cause is a branch filter naming a branch that does not exist, which is an ordinary way to leave a workflow that never runs and one a reader cannot see from the file alone. The manual-trigger-only form is the one AICD §39 records; this repository's own `.github/workflows/ci.yml` carries a warning about it. The fixture avoids that form on purpose: a `workflow_dispatch` trigger offers a manual run, and "cannot fire" would then be an overstatement. Here there is no trigger of any kind that this repository can raise.

**How this differs from `fixtures/migrated-with-drift`.** That fixture's inert workflow is there for `ori migrate` to detect while it reads the repository. This one is there for the liveness gate to fail on while it compares a declaration with a record of runs. What distinguishes the two cases is therefore not the workflow file, which any reader could be argued into believing: it is the run history.

## What is here

| Path | What it is | Serves |
|---|---|---|
| `README.md` | This file: the fixture's contract. | the reading rules below |
| `.github/workflows/ci.yml` | The inert workflow. Valid, registered if pushed, never run. Its header names the triggers that fire it, per PRD Z-02. | the "present" half of "present but reporting nothing" |
| `check.sh` | The one command the workflow's job runs. It passes. | gives the workflow something real to not run |
| `VERSION` | `0.1.0`. What `check.sh` checks. | the smallest product that has a check |
| `liveness/expected-runs.txt` | The declaration: this workflow is required, and must run inside every 604800-second window. | what the liveness gate compares against |
| `liveness/run-history.txt` | The observation: a complete history of runs for the thirty days to `as_of`, containing no run. | what the liveness gate reads as the run history |

There is no `spec/`, no `.env`, no second workflow and no application code. Anything more would be something the liveness gate does not read.

## What a gate reads, exactly

The two files under `liveness/`. The workflow file is read only to confirm it is present. Nothing else in the repository is read.

**Grammar.** UTF-8, LF line endings. One record per line. Blank lines and lines whose first character is `#` are ignored; there are no trailing comments. A record is a keyword, then `key=value` fields separated by single spaces. A value contains no space. Integers are decimal digits. Times are Unix seconds, UTC.

| File | Record | Fields |
|---|---|---|
| `expected-runs.txt` | `expected` | `workflow` (path from the repository root), `required` (`true` or `false`), `window_seconds` (integer) |
| `run-history.txt` | `history` | `as_of`, `covers_from` (times), `complete` (`true` or `false`) |
| `run-history.txt` | `run` | `workflow` (path), `started_at` (time), `conclusion` (one word) |

The file has exactly one `history` record and any number of `run` records. `expected-runs.txt` has one or more `expected` records and no other kind.

**Values in this fixture.**

| Field | Value | Means |
|---|---|---|
| `workflow` | `.github/workflows/ci.yml` | the only workflow |
| `required` | `true` | it is required |
| `window_seconds` | `604800` | seven days |
| `as_of` | `1791676800` | 2026-10-11T00:00:00Z, the instant the history was taken |
| `covers_from` | `1789084800` | 2026-09-11T00:00:00Z, thirty days earlier |
| `complete` | `true` | the `run` records are every run in `[covers_from, as_of]` |
| `run` records | none | nothing ran |

**How to read them.** In this order:

1. **Refuse malformed input; do not guess.** An unknown keyword, an unknown, missing or repeated field, a value that is not what its field requires, no `history` record or two, or no `expected` record: refuse. A refusal is not a pass and it is not a verdict about the workflow.
2. **Take `as_of` as the present, not the clock.** The window for an `expected` record is `[as_of - window_seconds, as_of]`. Here that is `[1791072000, 1791676800]`. The wall clock would put the window past the end of the history, where nothing has been recorded, and every later reading would silently become a different question.
3. **Refuse if the history does not cover the window.** `complete=false`, or `covers_from` later than `as_of - window_seconds`, means "no run recorded" says nothing about "no run happened", and the answer is "cannot decide" and not "fail". This fixture covers its window with room to spare: thirty days against seven.
4. **Skip records with `required=false`.**
5. **Present.** The file at `workflow` must exist. If it does not, that is a different failure, a required workflow that is missing, and not this fixture's.
6. **Count.** The `run` records for this `workflow` with `started_at` inside the window. How a gate treats each `conclusion` is the gate's definition and this fixture needs none of it: it has no `run` record.

**The verdict a correct gate owes on this fixture: fail**, and the finding is that `.github/workflows/ci.yml` is present, required, and ran zero times in `[1791072000, 1791676800]`. A gate that passes it, refuses it, or reports it as missing has not found the defect. This is the state `spec/DATA_MODEL.md` section 3 calls Inert, reached because an installed gate recorded no run.

**The control, for the pass half of a proof.** The gate must also pass a case that is alive. Derive it in a copy and never in this directory: append one line to `liveness/run-history.txt`, and the verdict flips to pass.

```
run workflow=.github/workflows/ci.yml started_at=1791590400 conclusion=success
```

(`1791590400` is 2026-10-10T00:00:00Z, one day before `as_of`, inside the window.) A gate that fails both this and the fixture reports nothing of its own.

Two more mutations of a copy are worth keeping: set `complete=false` (the gate must answer "cannot decide", not "fail"), and change the `workflow` on the `run` line to some other path (the run is another workflow's, so the verdict stays fail).

**What the format is not.** It is this fixture's own, chosen to be read with `std` alone and by eye. ORI-T-0046 was built in parallel without sight of it. If that gate's input differs, adapt a thin reader to this format; do not change the fixture's facts (one required workflow, a seven-day window, a complete thirty-day history, no run).

## How it becomes a repository of its own

`spec/TESTING.md` section 5 says each fixture is "a repository of its own". A git repository cannot be committed inside another one. A directory holding a `.git` is not recorded by the outer repository as its files, a submodule would make the fixture depend on a second repository and on a network fetch, and a bundle is an opaque binary no reviewer can read. So **this directory is committed as plain files, and the repository is made at run time** from a copy. The suite does this, and a test that needs the fixture does the same:

```
work=$(mktemp -d)
cp -R fixtures/inert-gate/. "$work"
cd "$work"
git init -q -b main
git add -A
GIT_AUTHOR_NAME=fixture GIT_AUTHOR_EMAIL=fixture@example.invalid \
GIT_COMMITTER_NAME=fixture GIT_COMMITTER_EMAIL=fixture@example.invalid \
GIT_AUTHOR_DATE=2026-10-10T00:00:00Z GIT_COMMITTER_DATE=2026-10-10T00:00:00Z \
git commit -q -m "chore: snapshot of the inert-gate fixture"
```

Three things in that recipe are not details. `-b main` is what makes the defect: with a default branch of `master` the workflow's filter would match the repository's own branch, and the fixture would no longer be inert. The fixed identity and dates make the commit identical on every run, so a test can name its hash. And it is a copy: `git init` in `fixtures/inert-gate` itself would plant a `.git` in the tree the outer repository tracks.

No nested `.git`, submodule or bundle is committed here.

## Why GitHub does not run this file here

GitHub reads workflow files only from `.github/workflows/` at the root of a repository. This one is at `fixtures/inert-gate/.github/workflows/ci.yml` in the Ori Studio repository, so it is not registered and no event can start it. The root `.github/workflows/ci.yml` is the only workflow that repository runs.

That makes two independent reasons it cannot fire there: the location, and the `master` filter, which would stop it even if the file were moved to the root. **Not verified by a run.** Nobody can observe GitHub from this worktree. The basis is GitHub's documented rule and the filter, and the check that closes it is to list the workflows GitHub has registered for the repository after this lands and see that the only path is `.github/workflows/ci.yml`.

## Not covered here

- That a liveness gate exists. It is ORI-T-0046, and this fixture is an input and not a test of it.
- That the end-to-end suite uses it. `spec/TESTING.md` section 5 says the suite requires this fixture and ORI-T-0077 builds the suite; until that lands nothing reads these files.
- Whether the repository was quiet. The record does not say how many pull requests were opened in the window. The declaration is a policy ("at least once per window"), and the fixture is inert under any amount of activity.
- A workflow that ran and failed, or ran and was skipped. That is a different defect (a gate that reports, and reports nothing useful) and a different fixture.
- A manual-trigger-only workflow. See above for why it was not chosen.
