# ORI-T-0199: Q7 — `document.submitted`, remove R1 approve bridge, `flows.documents.submit`

| Field | Value |
|---|---|
| Worktree | assigned at dispatch |
| Branch | `feat/ORI-T-0199-document-submitted` |
| Tier | **1** (`spec/RISK_MAP.md:21`, whole `crates/ori-flows`) |
| Declared scope (claim **124**) | **`crates/ori-flows/src/registry.rs`** — fix `document.submitted` payload (`{document_id}`), two-pass read, `serde_json::to_string`, `apply_document_submitted`, fold + `record` support; **remove** the interim Draft→Submitted in-memory block in `apply_document_approved` ([[R49]]); refuse `document.approved` when state is not `UnderReview` (fold and **pre-append** check in `record` for approved/changes_requested so a bad transition does not lock the product). **`crates/ori-flows/src/documents.rs`** (new) — **`submit(document_id)`** entry that appends `document.submitted` through `registry::record` only (batch 12 D1/D9). **`crates/ori-flows/tests/ori_t_0199_document_submitted.rs`** (new). **Modify** existing tests listed under **Escalation (operator, `test_modified`)** — no other test file changes. **Queue:** `crates/ori-flows/src/lib.rs`, `Cargo.toml` / `Cargo.lock` only if needed. **Nothing else.** |
| Spec anchor | `spec/DATA_MODEL.md` §3 Document state machine; `spec/API_SPEC.md` §1 (add/use `flows.documents.submit` — PD-3 may document later); `spec/criteria/phase-1.md` ORI-P1-040; `ops/batch-12-plan.md` D2, D9, Q7 ([[R49]]); AICD §9 |
| Spec trailer | `Spec: DATA_MODEL.md#document` |
| Ticket trailer | `Ticket: ORI-T-0199` |
| Commit types | `feat`, `fix`, `docs`, `chore` only |
| Model | Sonnet 5.5 |
| Precondition | **Dispatch** only after [PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85) (ORI-T-0197 / R1) **merges into `claude/sweet-maxwell-jkaiie`** ([[R50]]). Until then: designed, not dispatched. **Test edits:** exactly the two ORI-T-0197 tests listed under [[R50]]; no other existing test may change. |

## Why

[[R49]]: operator answered Q7 **yes** and accepted R1's bridge **only until this ticket lands**. DATA_MODEL §3 requires `document.submitted` on the log for Draft→UnderReview; `DocumentEvent::Approved` fires only from `UnderReview`.

## Read first

- `ops/rulings/R49-operator-answers-of-2026-10-19-r1-deviation-q7.md`;
- `ops/rulings/R50-test-modifications-for-0199.md`;
- `ops/batch-12-plan.md` D1, D2, D9;
- PR 85 `crates/ori-flows/src/registry.rs` (`apply_document_approved` bridge at Draft, `DOCUMENT_SUBMITTED_KIND` refused in `fold_event`);
- `crates/ori-core/src/document.rs` (`DocumentEvent::Submitted`, `fires_from`);
- `ops/tickets/ORI-T-0194-mcp-registry.md` (two-pass payload, lockout).

## Decision 1: `document.submitted` payload

- Kind: `document.submitted` (constant already in `registry.rs` on PR 85).
- Payload: **`{"document_id":"<id>"}`** only — `#[serde(deny_unknown_fields)]`, read via existing `read_object_payload`, write via `serde_json::to_string`.
- Fold: `apply_document_submitted` uses `apply_with_document_event` with `DocumentEvent::Submitted`; derived owner via `owner_for_kind` as other transitions.
- **`record`:** allow append of `document.submitted` when registry not blocked; validate payload shape before append (same as other kinds).

## Decision 2: Remove R1 bridge

Delete the block in `apply_document_approved` that, when `working.state == DocumentState::Draft`, calls `DocumentEvent::Submitted` in memory. **`fires_from`** for `Approved` then requires **`UnderReview`**.

**`record` before append:** for `document.approved` and `document.changes_requested`, after seat/migrated checks, ensure `DocumentEvent` built from the kind **`fires_from`** the folded document state (mirror fold), returning `EventDefect::Transition` as `RegistryError::Refused` **without** appending — same pattern as avoiding lockout on other pre-fold refusals.

## Decision 3: `documents.rs` submit entry

- Public **`Documents::submit(&mut self, db, at, actor, document_id) -> Result<u64, …>`** (name module/type to match batch 12 DOC slice conventions).
- Refuses when registry blocked; calls **`registry::record`** with `DOCUMENT_SUBMITTED_KIND` and serialized payload.
- **Actor:** human or agent per API_SPEC when PD-3 lands; until then follow **`DocumentEvent::Submitted`** (no extra human-only rule in core). Do not add network or new crates.

## Decision 4: Existing logs / migration

**Phase 1 has no production event logs.** All `ProductDb` data is local dev, tests, and fixtures. Logs that contain only `document.created` → `document.approved` (possible under [[R49]] bridge) **will fail fold** after this change. **No migration code:** rebuild or discard databases; state that in the closing report "not tested" if no long-lived fixture relies on the bridge (none expected).

## Escalation (operator, `test_modified`) — answered: approved, [[R50]]

Per `templates/escalation.md`. Raised on package design for [[R49]]; operator answered 2026-10-19.

| Field | Value |
|---|---|
| Trigger | `test_modified` |
| Ticket and anchor | ORI-T-0199, `spec/DATA_MODEL.md` §3; tests introduced by ORI-T-0197 ([PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85)) |
| Raised by | Lead, 2026-10-19, on package design for [[R49]] |
| Question | May ORI-T-0199 change the two ORI-T-0197 integration tests below so approval paths include a logged `document.submitted`, matching DATA_MODEL §3 after the R1 bridge is removed? |
| Recommendation | **Yes.** Replace the tests as described; add new `ori_t_0199_*` tests for submit refusal/success rather than weakening assertions. |
| Answer | **Approved**, [[R50]] — exactly the two tests in the replacement table; no weakening; no other existing test may change |
| Dispatch | **After** [PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85) merges into `claude/sweet-maxwell-jkaiie` ([[R50]]) |

### Context

[PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85) (`683b320`..`a742233`) implements ORI-T-0197 with an interim bridge in `apply_document_approved`: when the folded document is `Draft`, the fold applies `DocumentEvent::Submitted` in memory before `Approved` (`crates/ori-flows/src/registry.rs`, comments at the bridge). **`document.submitted` is refused in `fold_event`** (`KindReserved`). Integration test `ori_t_0197_record_approver_holding_seat_succeeds` appends `document.created` then `document.approved` with no submit event. Integration test `ori_t_0197_fold_refuses_document_submitted_kind` plants `document.submitted` and expects fold lockout. [[R49]] accepts the bridge until this ticket and answers Q7 **yes**. Removing the bridge makes **`document.approved` from `Draft`** invalid per `DocumentEvent::fires_from` in `crates/ori-core/src/document.rs`. **No `registry.rs` unit tests** exercise the bridge; the three unit tests (`unit_payload_pr_rejects_userinfo`, `unit_read_object_payload_rejects_non_object`, `unit_instructions_cap_and_control`) are unchanged.

### Options rejected

| Option | Why not |
|---|---|
| Keep tests unchanged and retain the bridge | Operator rejected in [[R49]] point 1 |
| Delete the success test without replacement | Loses ORI-P1-040 / seat-holder coverage |

### Affected tests and replacements

| Existing test (`ori_t_0197_registry.rs`) | Change | Replacement behavior |
|---|---|---|
| `ori_t_0197_record_approver_holding_seat_succeeds` | **Modify** | After `document.created`, **`record` `document.submitted`**, then `document.approved` with holder; same final approved state |
| `ori_t_0197_fold_refuses_document_submitted_kind` | **Modify** (rename acceptable) | With valid `{document_id}` payload after `document.created`, **fold succeeds** and document is **`UnderReview`**; keep separate **`ori_t_0199_*`** tests for malformed submitted payloads |

**Not modified (verified against PR 85 `683b320..a742233`):** seat-holder and agent refusals on approve run before transition; planted fold test `ori_t_0197_fold_refuses_planted_approval_without_seat_holder` still expects `SeatHolder`.

## Tests

**Target: 10 new tests** in `ori_t_0199_document_submitted.rs` plus **2 modified** ORI-T-0197 integration tests above (**45** total for `cargo test -p ori-flows`: 35 from ORI-T-0197 on PR 85 + 10 new; **0** unit tests modified).

| # | Test name | Covers |
|---|---|---|
| 1 | `ori_t_0199_submit_from_draft_appends_submitted` | Decision 3 |
| 2 | `ori_t_0199_submit_refuses_unknown_document` | Decision 1 |
| 3 | `ori_t_0199_submit_payload_refuses_array` | Two-pass |
| 4 | `ori_t_0199_submit_payload_refuses_unknown_field` | `deny_unknown_fields` |
| 5 | `ori_t_0199_fold_submitted_after_created` | Decision 1–2 |
| 6 | `ori_t_0199_record_refuses_approved_from_draft_without_submit` | Decision 2 |
| 7 | `ori_t_0199_approve_after_submit_succeeds` | End-to-end with seat holder |
| 8 | `ori_t_0199_submit_refuses_when_registry_locked` | Lockout |
| 9 | `ori_t_0199_fold_refuses_approved_from_draft_planted` | Fold without bridge |
| 10 | `ori_t_0199_documents_submit_delegates_to_record` | Decision 3 |

Re-run the full `ori-flows` suite after the two test edits; expect **45** passing tests (32→42 integration + 3 unit unchanged).

## Checks

fmt, clippy (native + Windows), strict doc, `cargo test -p ori-flows --locked`, gate 13.

## Commits

`feat(flows): add document.submitted and remove approve bridge` with `Ticket: ORI-T-0199`, `Spec: DATA_MODEL.md#document`.

## What you hand back

Commit hashes, check exit codes, test counts, deviation removed ([[R49]] condition met), escalation reference if operator pre-approved, tier 1.
