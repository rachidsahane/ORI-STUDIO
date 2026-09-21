# Proof: branch protection on `main`

AICD §14: a gate is installed only when it has been seen to fail. This is the proof for the first control this project installed. It is not a CI gate, so it has no entry in CI_CD section 1, but the same rule applies and the same record is owed.

| | |
|---|---|
| Control | GitHub branch protection on `main` |
| Applied | 2026-09-21, by the lead, on the operator's instruction |
| Configuration | Require a pull request; 0 required approvals; dismiss stale approvals; no admin bypass; linear history; conversation resolution; no force pushes; no deletions; no required status checks yet |

## Passes clean

`main` at `ffab3fd`, six pull requests open and all reporting `MERGEABLE`. Protection does not block the normal path.

## Fails on the forbidden action

An empty commit was created on `main` and pushed directly, which is the action CONVENTIONS and AICD §13 forbid to everyone.

```
remote: error: GH006: Protected branch update failed for refs/heads/main.
remote: - Changes must be made through a pull request.
 ! [remote rejected] main -> main (protected branch hook declined)
error: failed to push some refs
```

`git push` exit status 1. Local `main` reset; local and remote identical at `ffab3fd` afterwards.

## The failure is visible where a human would look

The refusal is printed by git at the point of the attempt, names the rule, and returns a non-zero exit status a script can test.

## Why no required status checks yet

Deliberate, and the reasoning is recorded so it is not mistaken for an omission. No CI gate exists yet. A required check that never reports leaves every pull request pending indefinitely, which is the mirror image of the defect this rule exists for: the first form protects nothing, the second blocks everything. Each check name is added on the day its proof file lands, starting with ORI-T-0013. "Require branches up to date" is added at the same time, because with no checks to re-run it costs six branch updates and protects nothing.

## What this proof taught, and it is the valuable part

The first run of this test reported `RESULT: FAIL, the push SUCCEEDED` while the push had in fact been refused. The harness was:

```
if git push origin main 2>&1 | tee /tmp/push.log | tail -6; then
```

A shell pipeline returns the exit status of its **last** command, so the `if` tested `tail`, which always succeeds. The control worked perfectly; the check reporting on it was inverted.

This is "present but reporting nothing" (AICD §39), produced by the lead, while proving a control, in the project built to catch it. It carries a concrete rule for every gate this project writes:

**A gate must capture the exit status of the command it is gating, before that command's output is piped anywhere.** Use a separate capture, or `PIPESTATUS`, or `set -o pipefail`. A gate whose command is piped into a formatter has no exit status of its own and will pass on every input.

This goes to ORI-T-0004 (`scripts/gates.sh`) and to every gate ticket ORI-T-0013 through ORI-T-0017 and ORI-T-0047, ORI-T-0048. It is also the strongest argument yet that the planted-defect requirement is not ceremony: the planted defect is what distinguishes a gate that works from a gate that reports success on everything.
