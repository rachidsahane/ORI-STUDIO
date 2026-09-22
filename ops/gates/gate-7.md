# Proof: CI gate 7, the supply chain gate

| | |
|---|---|
| Gate | CI_CD section 1, item 7: `cargo-audit`, `cargo-deny` (advisories, licenses), secret scan (tree and history) |
| Ticket | ORI-T-0016 |
| **State** | **Partially installed. Two checks of three.** |
| Written by | The lead, from the coder's verified evidence (ruling R25) |

Gate 7 is three checks. Two are installed. **The secret scan is not**, and no document may cite it as protection. What implements it is escalation E-0003, open with the operator.

## Installed: advisories, `cargo-audit`

| Planted input | Result |
|---|---|
| `advisory-clean`, a lockfile naming `cfg-if 1.0.0` | exit 0 |
| `advisory-defect`, a lockfile naming `time 0.1.44` | **exit 1**, RUSTSEC-2020-0071 |

The interesting part is how a vulnerability was planted in a workspace with **zero third-party dependencies**, where adding one is an escalation trigger. The fixtures are lockfiles with **no manifest**. `cargo-audit` compares names and versions against the RustSec database without building or resolving anything, so a real published advisory is presented to the real tool and no crate enters the workspace. Verified: the root manifest still names 0 third-party packages and 17 path members, and the harness asserts on every run that no workspace member lives under `fixtures/`.

## Installed: licences and bans, `cargo-deny`

| Planted input | Result |
|---|---|
| `deny-clean`, two Apache-2.0 path crates | exit 0 |
| `deny-license-defect`, a GPL-3.0-only path crate | **exit 4**, "bans ok, licenses FAILED" |
| `deny-bans-defect`, a wildcard version requirement | **exit 2**, "bans FAILED, licenses ok" |

Each half is attributable by **negated signature**: the licence fixture must leave bans clean and the bans fixture must leave licences clean. A fixture that broke both would prove neither.

`deny.toml` is at the repository root, where `cargo-deny` looks by default and where a reader expects a dependency policy. It was written under `fixtures/planted/gate-7/` first, which ruling R28 corrected: a fixture directory holds inputs a gate is run against, not the policy the whole repository is judged by.

## NOT installed: the secret scan

`scripts/secret-scan.sh` exists, runs on every pull request, and catches the common case. **It is not a gate.** Its own header says so, its step in `ci.yml` is named "NOT A GATE", `scripts/gates.sh` reports that sub-check **blocked** rather than passed, and its exit-0 line no longer contains the word "clean".

### Why, measured rather than argued

Two attempts, each finding the same hole one layer deeper.

**Attempt 1** used `grep -I`, which treats any file containing a NUL byte as binary and reports no match. Skipped, counted as scanned, run reported clean. Verified: `grep -I` exit 1 and `grep -a` exit 0 on the same file.

**Attempt 2** read every byte and matched printable-ASCII runs. The lead planted an AWS key encoded UTF-16 and committed it:

```
the file holds the key in UTF-16 : yes
scanner exit                     : 0
findings                         : none
```

Attempt 3 measured the four claims attempt 2 had shipped, against real artefacts rather than approximations, and **three of four were still false**:

| Real artefact | Scanner |
|---|---|
| ASCII text, control | exit 1, found |
| compiled binary, key baked in as ASCII | exit 1, found |
| SQLite with the key in a text column | exit 1, found |
| `.env` with a UTF-8 BOM | exit 1, found, and was never missed |
| `.env` with a **UTF-16** BOM | **exit 0, not found** |
| a real **`.p12`** built with `openssl pkcs12 -export` | **exit 0, not found** |
| a real **Java keystore** holding an encrypted private key | **exit 0, not found** |

A fixture named `keystore.p12` was renamed `ascii-in-nul-bytes.bin`, because it was never a keystore: it was ASCII between NUL bytes behind a JKS magic number, and **a filename makes a claim to readers who never reach the comment written to excuse it**.

UTF-16 is one encoding. Base64, gzip, UTF-32 and a key split across a chunk boundary are the same hole. A pattern matcher over bytes loses this race by construction, and each round produces a gate reporting clean on a credential that is really there, which is worse than no gate because documents cite it.

### What the operator needs to decide

Escalation **E-0003**. CI_CD says "secret scan (tree and history)" and names no tool, which is why a coder wrote one. GitGuardian already runs here on every pull request, but the ticket measured two things that bear on it: **it is not a required check**, and its own summary reports scanning **one commit**, not a history.

## What is not established

1. **The visibility clause of AICD §14.** No local run watches the pull request check it writes to, and **this gate has no cited visibility run of either kind.**
   This line previously read "Established by the run of this pull request, recorded in a follow-up commit." **No such commit was ever made**, and no run identifier for gate 7 appears anywhere under `ops/`. The sentence claimed evidence in a place that does not exist, inside the list of things the gate has *not* established, which is the one list where a false entry is most expensive.
   What is true: `gate-7` and `gate-7-proof` have been seen green on real runs, for example 35654836919. What has never been observed is `gate-7` **red** on a planted defect in a real run, which is what AICD §14's third condition asks for. Gates 1, 2 and 13 each have such a run recorded; gate 7 does not. Until one exists this clause stays open on its own terms, and the gate stays at two checks of three for the separate reason recorded in escalation E-0003.
   Found by the round 4 audit ([[CR-007]]).
2. **That a red `ci` blocks anything.** `main` still has no required status checks. Three proofs now record this as owed.
3. **That the two installed checks are exhaustive of their own domains.** `cargo-audit` is as good as the RustSec database on the day it runs; `cargo-deny` enforces the policy written in `deny.toml` and no more. Neither is a claim about dependencies nobody has published an advisory for.

## Rollback

`git revert`. Gate 7 returns to undefined: no advisory check, no licence policy, no scanner.
