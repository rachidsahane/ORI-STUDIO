# R37. Two `test_modified` escalations, answered by the operator

| | |
|---|---|
| Ruling | R37 |
| Tier | 2: both changed existing tests inside tier 2 pull requests |
| Made by | The operator, in session, on or before 2026-09-23. Recorded by the lead on 2026-09-27 |

`CLAUDE.md` absolute rule 3 forbids an agent to modify an existing test; it escalates with trigger `test_modified` and stops. Both coders below did. Both answers are the operator's.

## ORI-T-0107: four `db.rs` tests (pull request 55, carried by pull request 59)

Registering migration 2 in `crates/ori-store/src/db.rs` broke four ORI-T-0024 tests that assumed the binary carries exactly one migration. Three asserted `schema_version()` against a literal 1 (two unit tests and one property test). The fourth simulated a future migration at a hard-coded version 2, which the real migration 2 now occupied:

- `ori_t_0024_a_fresh_product_directory_opens_and_lays_out_the_whole_contract`
- `ori_t_0024_reopening_an_already_migrated_database_is_idempotent`
- `ori_t_0024_any_number_of_sequential_opens_stays_idempotent`
- `ori_t_0024_a_database_newer_than_the_binary_is_refused_not_silently_used`

The coder made the change, saw them fail, reverted, and escalated before touching them.

**The operator authorized the four edits.** Each now reads `MIGRATIONS.len()` instead of a literal, and the future version is `MIGRATIONS.len() + 1`, so a third migration does not reopen this. That condition is recorded as the lead's addition in pull request 55's body and as the operator's in commit a146dcb's message; it is what shipped either way. A fifth, new test pins the count itself. The lead reported the assertion count rising from 32 to 37, with no hard-coded `schema_version` literal left.

## ORI-T-0108: twelve `AgentIdentity::create` call sites (pull request 58, carried by pull request 60)

The operator's ruling on the backlog's escalation 6 put the model family on `AgentIdentity`. Making `family` a required argument of `AgentIdentity::create` changed its twelve existing test call sites under `crates/ori-broker/src/`: nine in `identity.rs`, two in `keychain.rs`, one in `issuance.rs`. The coder stopped with the exact diff instead of applying it.

**The operator authorized the edits, with the design they served**: `register_identity` reads every `identity.created` event for the product from the event log and refuses there, so no caller can hand it an empty list of existing identities (commit 79bacb0: "Authorized by the operator, continuing ORI-T-0108"). The condition: each call site gains the family argument and nothing else. The lead's audit, in pull requests 58 and 60: 12 arguments added, 0 assertion lines removed, and every other removed line in those three files is doc-comment prose describing the old design.

## Recorded elsewhere, and not ruled again here

The model family on `AgentIdentity`: `spec/adr/ADR-0001-stack.md` (pull request 57). `serde` and `serde_json`: [[E-0008]] (pull request 61). `tantivy` approved and then withdrawn for SQLite FTS5: [[E-0004]] (pull requests 65 and 67) and `spec/adr/ADR-0003-full-text-index.md` (pull request 68).
