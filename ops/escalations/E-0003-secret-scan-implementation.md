# Escalation E-0003: what implements gate 7's secret scan

| | |
|---|---|
| Trigger | `spec_conflict` |
| Raised by | Lead, on ORI-T-0016 attempt 2, after the coder exhausted two attempts on it |
| Blocks | One third of gate 7. The advisory and licence halves are unaffected |
| State | Open, with the operator |

## What happened

`spec/CI_CD.md` section 1 gate 7 is three checks: `cargo-audit`, `cargo-deny`, and "secret scan (tree and history)". The first two name real tools and both are proven. The third names no tool, so ORI-T-0016 wrote one: a shell script matching credential patterns with `grep`.

Two attempts, and each found the same class of hole one layer deeper.

**Attempt 1** used `grep -I`, which treats any file containing a NUL byte as binary and reports no match. A secret in a keystore, a `.p12`, a SQLite file or a compiled binary was skipped, counted as scanned, and the run reported clean. Verified by the lead: `grep -I` exit 1, `grep -a` exit 0 on the same file, scanner verdict "0 in the tree".

**Attempt 2** repaired that by reading every byte and matching printable-ASCII runs. The lead then planted an AWS key encoded UTF-16 and committed it. **The scanner exits 0 and reports clean.** The key is in the file; `A\0K\0I\0A` is not an ASCII run.

There is no third attempt worth spending. UTF-16 is one encoding. Base64, gzip, UTF-32, a key split across a chunk boundary, and a dozen more are the same hole. **A pattern matcher over bytes loses this race by construction**, and every round of it produces a gate that reports clean on a credential that is really there, which is worse than no gate because documents cite it.

## The thing that makes this decidable

**This repository already has a secret scanner running on every pull request.** GitGuardian reports a check on every PR and has passed on all of them. Nobody asked it to; it is installed at the account or organisation level.

So the fleet spent two attempts hand-rolling a worse version of a tool that was already in the pipeline.

## The question

What implements gate 7's secret scan? CI_CD says "secret scan (tree and history)" and names no tool, which is why a coder wrote one.

## The lead's recommendation

Name a real scanner in CI_CD, and prove that instead. Two candidates:

1. **GitGuardian**, already present and already running. Costs nothing to adopt. The gate becomes "the GitGuardian check is green", and the planted-defect proof is a commit carrying a real test credential that it must catch. The dependency is on a third-party service, which `spec/PROJECT_BRIEF.md` principle 3 would normally make a slot rather than a fixed choice.
2. **`gitleaks`**, pinned by SHA-256 like `cargo-audit` and `cargo-deny` already are in this ticket. Runs locally too, so `scripts/gates.sh` can run the same check a coder runs before opening a pull request, which the GitGuardian route cannot.

The lead prefers the second, because ENV_SETUP already establishes the pattern of pinned local tooling and because a gate a coder cannot run at a desk is half a gate. But this changes CI_CD, which is specification, so it is not the lead's to decide.

## What ORI-T-0016 ships meanwhile

Two of gate 7's three checks, proven on planted defects with the real tools:

- **advisories**, `cargo-audit`: a lockfile naming `time 0.1.44`, which carries RUSTSEC-2020-0071. No dependency added to the workspace: the fixture has no manifest and is never built.
- **licences and bans**, `cargo-deny` against a `deny.toml` grounded in Apache 2.0 compatibility: a GPL-3.0-only path crate fails licences while bans stays clean, and a wildcard version fails bans while licences stays clean, so each half is attributable.

The secret scan half is **not installed**, and `ops/gates/gate-7.md` says so. The scanner stays in the tree, runs, and catches the common cases, but no document may cite it as protection and `scripts/gates.sh` reports that half as blocked rather than passed.

## Two facts about GitGuardian the ticket established, and they matter to your answer

The coder checked rather than repeating the lead's observation, and found two things:

- **It is not a required status check.** `GET /repos/:owner/:repo/branches/main/protection/required_status_checks` returns 404, "Required status checks not enabled". So it has been green on all nineteen pull requests and has blocked nothing.
- **Its own summary says it scanned one commit.** "**1** commit was scanned without uncovering any secrets." That is a scan of the pull request's head, not of tree and history. CI_CD gate 7 asks for both.

So adopting GitGuardian as-is would not satisfy gate 7's own wording without changing how it is configured, and it would still be a check nobody can run at a desk.

## A second, smaller question inside the first

The scanner invented a rule no specification states: a fixture's fake secret is exempt only when a marker appears **inside the value**, not merely because the file lives under `fixtures/`. It needed one, because `spec/TESTING.md` section 5 requires `fixtures/migrated-with-drift/.env` to be tracked with fake keys and `.gitignore` re-includes it for exactly that reason. Whatever implements the scan inherits this question, and if it is a third-party tool the answer is its allow-list format rather than ours.
