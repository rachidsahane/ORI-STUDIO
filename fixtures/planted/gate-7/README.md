# Planted defects for CI gate 7

Ticket: ORI-T-0016. Spec: `spec/CI_CD.md` section 1 gate 7, `spec/ENV_SETUP.md`
section 4, `spec/CONVENTIONS.md` "Dependencies", `spec/TESTING.md` sections 1, 4
and 5, `spec/runbooks/prove-gate.md`. Proof: `ops/gates/gate-7.md`. Rule:
AICD §14, a gate is installed only when it has been seen to fail.

## Gate 7 is PARTIALLY installed: two checks of three

| Check | Tool | State |
|---|---|---|
| advisories | `cargo-audit` | **installed**, seen to fail on a planted lockfile |
| licences and bans | `cargo-deny` | **installed**, each half seen to fail separately |
| secret scan | `scripts/secret-scan.sh` | **NOT INSTALLED**. Escalation E-0003. |

The secret scan is advisory and nothing may cite it as a gate. It matches six
byte patterns against printable-ASCII runs, so a credential in any other
encoding is invisible to it: an AWS key written UTF-16 is committed, scanned,
and comes back exit 0 with the key in the file. base64, gzip, UTF-32 and a key
split across a chunk boundary are the same hole, and closing one opens the
next, which is why E-0003 asks the operator what implements this check rather
than a third attempt being made at the scanner. Separately, this repository
already runs GitGuardian on every pull request, and has since before gate 7
was written; whether that is the answer to E-0003 is the operator's call.
`ops/gates/gate-7.md` carries the reproduction and the real output.

The scanner is kept, and the secret rows below are kept, because it catches the
common case and its fixtures are reusable by whatever the operator chooses.
What those rows establish is what the scanner **does**, never that gate 7's
third check **exists**.

Three checks in three tools, so three sets of planted inputs, each run
separately and attributably.

Run the proof: `bash fixtures/planted/gate-7/prove.sh`. It exits 0 only from its
final verdict. 1 means gate 7 stopped catching something still planted here; 2
means the proof could not be made; 3 means a tool was missing and nothing ran.

## What is NOT here, and where it went

`deny.toml` is at the repository root and the scanner is `scripts/secret-scan.sh`.
Both were under this directory in the first attempt and both were wrong to be.
Ruling R28: a fixture directory holds planted defects, which are inputs a gate
is run against; a policy the whole repository is judged by, and a scanner the
gate invokes, are neither. `prove.sh` asserts on every run that neither has
come back here as a second copy, because a gate whose implementation exists
twice can be repaired in the copy CI does not run.

## What is here

| Path | What it is | Gate 7 owes it |
|---|---|---|
| `advisory-clean/Cargo.lock` | a lockfile naming `cfg-if 1.0.0`, which has no advisory | pass |
| `advisory-defect/Cargo.lock` | a lockfile naming `time 0.1.44`, which carries RUSTSEC-2020-0071 | **fail** |
| `deny-clean/` | two path crates, both Apache-2.0, no wildcard | pass |
| `deny-license-defect/` | the same with one crate under GPL-3.0-only | **fail the licenses check, pass bans** |
| `deny-bans-defect/` | the same with a wildcard version requirement | **fail the bans check, pass licenses** |
| `secret-samples/declared-fake.env` | six secret-shaped values, every one carrying the declared-fake marker | pass, having exempted all six |
| `workflow-samples/*.yml` | sixteen whole workflows, one live, ten dead in ten ways, one unreadable, four differing in how much history their checkout asks for | inputs for the harness's own checks |

The nine planted repositories the secret rows are run against are **built at run
time** by `prove.sh`, in a temporary directory, from `declared-fake.env`: one
clean, one holding the declared fake, one holding the same values with the
marker replaced so that nothing is declared, and one where that file was
committed and then deleted so the secret survives only in the history. A fifth
is a depth-1 clone of the fourth, which the scanner must refuse.

The last four are about content a scanner is most tempted to skip, and they
exist because attempt 1 proved ten rows that were all about text while the
scanner matched with `grep -I`, which reports no match for a file holding a NUL
byte without reading it. Every such file was skipped, counted as scanned, and
reported clean, in the tree and in the history both. The four are: the declared
sample inside a file with NUL bytes, which must be found and exempted; the
derived sample inside the same shape of file, in the tree; the same committed
and then deleted, so only the blob is left; and a 16 MiB binary with the values
baked into it, which is the shape of a compiled artifact somebody committed.
Each row pins `1 as binary` in the scanner's own counts and `run` rather than
`line` in the finding, so a return to skipping is a disagreement here and not a
quieter pass.

**What those four rows cover, corrected.** An earlier version of this file said
they covered "a keystore, a .p12, a compiled binary with a token baked in, a
SQLite file or an `.env` saved with a byte-order mark". All five were then built
and run against the repaired scanner, and three of the five are still silently
clean:

| Built and run | Result |
|---|---|
| a compiled binary with the key baked in as ASCII | exit 1, found |
| a SQLite file with the key in a text column | exit 1, found, `run 8` |
| an `.env` with a UTF-8 byte-order mark | exit 1, found, and never missed: no NUL byte |
| an `.env` with a UTF-16 byte-order mark | **exit 0, not found** |
| a PKCS#12 (`.p12`) holding a private key | **exit 0, not found** |
| a Java keystore (JKS) holding a private key | **exit 0, not found** |

These four rows cover one shape: printable ASCII with non-printable bytes
around it, which is what a compiled object and a SQLite file have. A real
`.p12` and a real JKS store the key DER-encoded and encrypted; UTF-16 stores it
one byte apart from itself. In all three the credential is in the file and the
string is not, and every byte count the scanner prints on them is honest. The
file this fixture manufactures used to be called `keystore.p12` and was neither
a keystore nor a `.p12`: it is ASCII between NUL bytes behind a JKS magic
number. It is now called `ascii-in-nul-bytes.bin` and built by `nul_wrapped()`,
because a name outlives a comment and that one was making the claim by itself.
The real output of all six runs is in `ops/gates/gate-7.md`.

A fixture that imitates the easy half of a file format, described as the format,
is a claim of coverage nobody re-derived. That is the defect class this
directory exists to catch, written by the thing meant to catch it, which is why
the correction is recorded here rather than quietly applied.

They are manufactured rather than shipped because a tracked file holding an
undeclared secret-shaped string is exactly what this gate exists to reject. A
fixture that made the repository fail its own gate would be a fixture nobody
could merge.

## Why nothing here is a dependency

This workspace has zero third-party dependencies and adding one is an
escalation trigger (`spec/CONVENTIONS.md`, AICD §12). So:

- the advisory fixtures are lockfiles **with no manifest**. cargo-audit reads
  `Cargo.lock` and compares names and versions against the RustSec database; it
  does not build and does not download. Nothing can resolve these directories.
- the cargo-deny fixtures are **path-only workspaces**, each its own workspace
  root through a `[workspace]` table, which cargo-deny resolves from
  `cargo metadata` with no network and no registry.

The root `Cargo.toml` lists its members explicitly and names nothing under
`fixtures/`, so `cargo build --workspace` never reaches any of this. `prove.sh`
asserts that on every run.

## The declared fake, and what telling it from a secret costs

`spec/TESTING.md` section 5 gives `fixtures/migrated-with-drift` "a tracked env
file with fake keys" and `.gitignore` re-includes `/fixtures/**/.env` so it can
be tracked. `spec/ENV_SETUP.md` section 4 forbids secrets in fixtures. Both are
true at once only if the gate can tell a declared fake from a credential.

**The rule is about the value, not the path.** A match is exempt when the
matched text itself carries the marker (`FIXTURE` followed by `FAKE`). No
directory is exempt and there is no allow list of files, because a path allow
list makes every byte at that path invisible, which is a hole exactly where the
specification says secrets must not be. A value cannot carry the marker and
still authenticate: to carry it, the value has to be edited.

This convention is not in any specification. Nothing says a fixture's fake
secret is exempt only when the marker is inside the value: `spec/TESTING.md`
section 5 says a fixture has fake keys and `spec/ENV_SETUP.md` section 4 says
fixtures hold no secret, and neither says how a gate tells one from the other.
It is a convention this gate imposes because it needed one, and it is an open
question for the operator, recorded in `ops/gates/gate-7.md`.

What it costs is written out at the top of `scripts/secret-scan.sh` and in
`ops/gates/gate-7.md`. In short: every fixture's fake key has to carry the
marker inside the value, `fixtures/migrated-with-drift/.env` included when
ORI-T-0074 builds it; a fixture needing a byte-exact realistic credential
cannot have one; and somebody who can edit a value can add the marker, which
leaves the last link a human reading the diff.

## What proving this gate needs that the others did not

`prove.sh` asks the four questions gate 1's harness arrived at and the fifth
gate 2's added, and one more of its own: **is the history actually on the
runner?** `actions/checkout` clones with `fetch-depth: 1` by default, and in
that clone every object a ref reaches belongs to the tree, so "scan the
history" reads the tree and reports the history clean. The scanner refuses a
shallow repository rather than reporting it clean, and the harness reads
`fetch-depth` out of `.github/workflows/ci.yml` so that the reason is named
rather than met as a surprise refusal in another job's log.
