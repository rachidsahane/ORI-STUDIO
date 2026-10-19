# fixtures/migrated-with-drift

An existing product that was never built under the method: it has code, tests,
documents that have drifted from the code, a CI workflow that cannot start, and
an environment file with credentials in it that is tracked by git. The
end-to-end suite runs `ori migrate` against it. Specification anchor:
`spec/TESTING.md` section 5, "documents that disagree with code, an inert
workflow, a tracked env file with fake keys, for J-02 and Z-01"; migration
itself is `spec/PRD.md` section 4.5.

| Criterion or journey | What this fixture gives it |
|---|---|
| ORI-P1-024, `ori migrate` | An M0 inventory has something to find in every category it lists; the inert workflow is detected and listed; the tracked env file is reported by key names and shape only; unattributed-change detection is turned on over a real repository |
| ORI-P1-025, M2 documents and the divergence register | Ten planted disagreements between documents and code, each listed below, so the register can be checked entry by entry, plus statements that agree with the code and must not become entries |
| ORI-P1-026, `ori readiness` | Four stale documents, each with the divergence that makes it stale, and one document that is not stale |
| J-02 and Z-01 | The product above as the existing product; a recipe for an unattributed change at the end of this file |

## Read this first

This directory is the product plus two harness files, `README.md` (this file,
which is the answer key) and `make-repo.sh`. Do not point `ori migrate` at the
directory itself: the answer key would be inside the repository the migration
reads, and the directory has no history. Build a repository from it:

```sh
bash fixtures/migrated-with-drift/make-repo.sh "$workdir/parceltrack"
```

The script prints the path it built. It refuses a destination that is not empty
or that sits inside another git repository.

## A repository of its own

`spec/TESTING.md` section 5 wants each fixture to be a repository of its own.
A `.git` directory cannot be committed inside another repository, a submodule
would put a second object database under a repository whose secret scan and
history checks read one, and a git bundle is a binary the scans cannot read.
None of the three is used. The fixture is plain files, and `make-repo.sh` turns
a copy of them into a repository, the same one every time:

- Three commits on `main`: the sources, then `docs/`, then `.github/` and
  `.env`. The environment file is therefore in the tree and in history.
- One branch, `feature/carrier-webhooks`, one commit ahead of `main` and never
  merged: the "branch with unmerged work" an M0 inventory lists.
- No tag. The baseline tag is created by the migration, and its existence is
  something the migration verifies.
- A fixed author, `Pat Okafor <pat@parceltrack.example>`, and fixed dates, so
  two runs give the same commits. The script ignores the machine's git
  configuration.
- The harness files are left out of the copy.

A test that needs only files, not history, can copy the directory instead and
remove `README.md` from the copy.

## Files

| Path | What it is | Serves |
|---|---|---|
| `README.md` | This file: the contract the suite asserts against | all |
| `make-repo.sh` | Builds the repository described above | ORI-P1-024, Z-01 |
| `Cargo.toml`, `Cargo.lock` | The product's manifest (no dependencies) and lock file; the manifest declares its own workspace so it stays out of this repository's | M-01 import: stack |
| `src/` | The product: five modules, a library root and a command line | ORI-P1-025, ORI-P1-026 (the code the documents are compared with), Z-01 |
| `tests/store_flow.rs`, tests inside `src/` | Ten tests, all passing | M-01 import: tests present |
| `docs/overview.md`, `architecture.md`, `api.md`, `runbook.md` | Documents that disagree with the code | ORI-P1-025, ORI-P1-026 |
| `docs/cli.md` | A document that agrees with the code in every statement | ORI-P1-025, ORI-P1-026 (no false positive) |
| `.github/workflows/ci.yml` | The inert workflow | ORI-P1-024 |
| `.github/CODEOWNERS` | The humans with access, as far as the repository shows them | ORI-P1-024 (inventory) |
| `.env` | The tracked environment file with fake keys | ORI-P1-024 (env report) |
| `.gitignore` | The product's own, which does not list `.env`: that is why the file is tracked | ORI-P1-024 |

## Planted disagreements

Ten. Each is a statement in exactly one document that the code does not bear
out. The register should hold one entry for each and no other entry.

| ID | Document and heading | The document says | The code does | Kind |
|---|---|---|---|---|
| D-01 | `docs/overview.md`, Quick start | The server listens on port 8080 by default | `DEFAULT_PORT` in `src/config.rs` is 7070 | Value |
| D-02 | `docs/architecture.md`, Storage | Parcels live in PostgreSQL, reached through `DATABASE_URL` and a pool of four connections, schema created on first start | `src/store.rs` reads and writes one tab-separated file named by `PARCEL_DATA_FILE`; no source file mentions PostgreSQL or `DATABASE_URL` | Subsystem |
| D-03 | `docs/api.md`, Statuses | A status is `created`, `shipped`, `delivered` or `lost` | `Status` in `src/model.rs` is `registered`, `in_transit`, `out_for_delivery`, `delivered` or `returned` | Vocabulary |
| D-04 | `docs/architecture.md`, Notifications | A failed notification is retried up to three times | `MAX_NOTIFY_ATTEMPTS` in `src/notify.rs` is 5 | Value |
| D-05 | `docs/api.md`, Endpoints | The lookup is `GET /v1/parcels/{id}` | `route` in `src/server.rs` serves `GET /parcels/<id>`; a path under `/v1` is answered 404 | Path |
| D-06 | `docs/architecture.md`, Audit trail | `src/audit.rs` writes every status change to an append-only audit log with the time and the operator | There is no `src/audit.rs`, no audit log and no notion of an operator | Documented, not built |
| D-07 | `docs/api.md`, Endpoints | Nothing: the document lists one endpoint | `route` also answers `GET /health` with 200 and `ok` | Built, not documented |
| D-08 | `docs/overview.md`, Continuous integration | Every pull request to `main` runs the full test suite in CI | `.github/workflows/ci.yml` runs on pushes to `master` and pull requests to `develop`; see the next section | Process claim |
| D-09 | `docs/runbook.md`, Backups | A cron job runs `scripts/backup.sh` | The product has no `scripts/` directory and no such file | Missing file |
| D-10 | `docs/api.md`, Endpoints | The lookup returns JSON, `{"id": ..., "status": ...}` | It returns plain text: the id, a tab, the status, a newline | Format |

Four documents are stale and one is not, which is what `ori readiness` should
say for ORI-P1-026:

| Document | Stale | Divergences |
|---|---|---|
| `docs/overview.md` | yes | D-01, D-08 |
| `docs/architecture.md` | yes | D-02, D-04, D-06 |
| `docs/api.md` | yes | D-03, D-05, D-07, D-10 |
| `docs/runbook.md` | yes | D-09 |
| `docs/cli.md` | no | none |

### Statements that agree with the code (controls)

None of these is a divergence. A register entry for one is a false positive.

- Every statement of `docs/cli.md`: the commands, their output, the refusals,
  the exit codes, and the defaults of `PARCEL_PORT` (7070) and
  `PARCEL_DATA_FILE` (`parcels.tsv`).
- `docs/overview.md`: a parcel can be returned until it is delivered, and once
  delivered or returned it does not change; the four documents it links to
  exist.
- `docs/architecture.md`: the layout table (all six files exist and hold what
  is said); announcements go through the `Sender` trait; the listener is read
  only and reads the store afresh on every request.
- `docs/api.md`: an unknown id is answered 404 with the body `not found`; a
  method other than `GET` is answered 405.
- `docs/runbook.md`: the listener binds the loopback interface only; local
  settings live in `.env` in the repository root; `.github/CODEOWNERS` exists.

### Statements the code can neither confirm nor deny

`docs/runbook.md` names two environments and their addresses, a certificate
renewed by certbot every 60 days, a 02:00 schedule and a Monday check of the
job's mail. They are facts about hosts the repository does not describe, so
they belong in an M0 inventory and in no divergence entry. (That
`scripts/backup.sh` does not exist is D-09; the schedule around it is not
checked.)

## The inert workflow

`.github/workflows/ci.yml` is a valid workflow with one job that would run the
tests. It can never start, because every route into it is closed:

- `push` is filtered to branches named `master`, and the repository has no
  branch of that name: its default branch is `main` and the only other branch
  is `feature/carrier-webhooks`.
- `pull_request` is filtered to base branches named `develop`, and there is no
  such branch either.
- There is no `schedule`, no `workflow_dispatch`, and no other event, so nothing
  else can start it.

The story it stands for is a repository whose branches were renamed after the
workflow was written. It is detectable from the repository alone, without a
history of runs: compare the branch filters with the branches that exist. The
document that says CI guards `main` (D-08) is wrong for the same reason.

In this repository the file is additionally inert by location, because GitHub
reads workflows only from the root `.github/workflows` directory. That is a
fact about where the fixture is stored and not what migration is asked to
detect; in a repository built by `make-repo.sh` the file sits at the root.

## The tracked env file

`.env` holds eight keys. Names are listed here, values are not. The shapes and
lengths come from the script that wrote the file, which no one has read the
file back through.

| Key name | Shape | Value length |
|---|---|---|
| `DATABASE_URL` | Connection URL with a user and password inside it | 67 |
| `PARCEL_API_KEY` | Opaque letters and digits | 35 |
| `CARRIER_API_TOKEN` | `sk-` prefix, then letters and digits | 34 |
| `AWS_ACCESS_KEY_ID` | `AKIA` prefix, then 16 upper-case letters and digits | 20 |
| `AWS_SECRET_ACCESS_KEY` | Base64 alphabet, including `+` and `/` | 40 |
| `NOTIFY_SLACK_TOKEN` | `xoxb-` prefix, then hyphen-separated groups | 38 |
| `SESSION_SECRET` | Base64 alphabet | 51 |
| `SENTRY_DSN` | URL with a key where a user name would be | 63 |

- **Tracked.** The repository root `.gitignore` ignores `.env` everywhere except
  under `fixtures/`, where its exception `!/fixtures/**/.env` brings the file
  back; the product's own `.gitignore` does not list it. `git status` shows it
  as untracked, not ignored, until it is added, and `make-repo.sh` commits it
  in the third commit, so it is in history as well as in the tree.
- **Fake in the value.** Every value contains the declared-fake marker
  `scripts/secret-scan.sh` looks for: the word FIXTURE immediately followed by
  the word FAKE, with nothing between them, inside the value. That is the only
  thing that makes the scanner pass the file, and a value edited without it
  fails the scan.
- **A cheap leak check.** Because every value carries the marker, a test that
  asserts "no value in any output" can search the output for it, building the
  marker from its two halves so the test's own source does not contain it. A
  test does not have to read the file, and should report a match as a leak of
  the value it found.
- **What the code reads.** No source file reads any of these keys. They stand
  for integrations earlier versions used: a database, an object store, a chat
  hook, an error tracker, a carrier API. The code reads only `PARCEL_PORT` and
  `PARCEL_DATA_FILE`, and neither is in the file.

## M0 inventory: what the fixture shows for each category

The inventory should find the following in a repository built by
`make-repo.sh`. The right-hand column says where.

| Category | Where the fixture shows it |
|---|---|
| Repositories | The one repository |
| Branches with unmerged work | `feature/carrier-webhooks`, one commit not in `main` |
| Environments | `docs/runbook.md`: staging and production |
| Services | The `parceltrack serve` listener |
| Data stores | `parcels.tsv` (the code); PostgreSQL (the documents, D-02) |
| Queues | None |
| Jobs | The 02:00 backup cron job in `docs/runbook.md` |
| Third parties | Implied by the env key names: AWS, Slack, Sentry, a carrier API, PostgreSQL |
| Domains | `staging.parceltrack.example` and `api.parceltrack.example` in `docs/runbook.md` |
| Certificates | The Let's Encrypt certificate in `docs/runbook.md` |
| Credential locations | `.env`; `docs/runbook.md`, Settings |
| Humans with access | `.github/CODEOWNERS` (two addresses) and the commit author |

## An unattributed change (Z-01)

After `ori migrate` has turned detection on over a repository built by
`make-repo.sh`, either of these is a change outside any session on a ticket:

```sh
# a disk edit, nothing committed
printf '\n// edited by hand\n' >> src/store.rs

# a commit on main
printf '\n// edited by hand\n' >> src/store.rs && git commit -am "hand edit"
```

Each should raise an unattributed-change event for `src/store.rs`, and a merge
from the branch should then be blocked until the incident is resolved.

## The product, and keeping it out of this repository's workspace

The product depends on nothing but the Rust standard library, so it builds
without a network. In a repository built by `make-repo.sh`, `cargo test
--locked` runs its ten tests.

It does not belong to this repository's Cargo workspace. The root `Cargo.toml`
lists its members explicitly and has no `exclude`, so the product is not a
member; and the empty `[workspace]` table in the product's own `Cargo.toml`
makes it a workspace root of its own, so cargo does not mistake it for an
unlisted member of the enclosing one. `cargo build --workspace` at the root
neither builds it nor fails on it.

## Keeping this file true

The tables above are the contract. A change to anything under `src/`, `docs/`,
`.github/` or `.env` that makes one of them untrue is a change to this fixture's
tests, so update the tables in the same commit, and update the commit list in
`make-repo.sh` when a file is added.
