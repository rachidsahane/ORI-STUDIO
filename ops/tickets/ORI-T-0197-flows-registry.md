# ORI-T-0197: batch 12 slice R1, document registry and shared ports

| Field | Value |
|---|---|
| Worktree | assigned at dispatch |
| Branch | `feat/ORI-T-0197-flows-registry` |
| Tier | **1.** `spec/RISK_MAP.md:21` tiers `crates/ori-flows` at 1. Seat-holder enforcement lives in this slice; batch 12 PD-1 amends the row *reason* to include `registry::record`'s holder check (`ops/batch-12-plan.md`). No tier-2 path in R1 unless Q12 adds `registry.rs` |
| Declared scope (claim 122) | **New:** `crates/ori-flows/src/registry.rs`, `crates/ori-flows/src/ports.rs`, `crates/ori-flows/src/migrate/mod.rs` (module stub, doc header only), `crates/ori-flows/tests/ori_t_0197_registry.rs`. **Queue:** `crates/ori-flows/Cargo.toml` (add `ori-core`, `serde`, `serde_json` per plan), `crates/ori-flows/src/lib.rs`, `Cargo.lock`. **Nothing else.** **No existing test changes** |
| Spec anchor | `spec/DATA_MODEL.md` §2 Document, §3 Document state machine (`### Document`); `spec/criteria/phase-1.md` ORI-P1-001 (`product.created` at init); `spec/API_SPEC.md` §2 `document.*` kinds and mutating-method preamble; PD-2 for `product.created` kind documentation; `spec/LLD.md` §2 ori-flows; AICD §9, §18, §23 (methodology html:837, G1 exit html:885); `spec/SECURITY_NOTES.md` Secrets (:58-61), paths (:53) |
| Spec trailer | `Spec: DATA_MODEL.md#document` |
| Ticket trailer | `Ticket: ORI-T-0197` |
| Commit types | `feat`, `fix`, `docs`, `ci`, `chore` only |
| Model | Sonnet 5.5 |

## Why

Batch 12 D1–D2, D5, D12–D13 (`ops/batch-12-plan.md`). Every later slice consumes **`DocumentRegistry`**, **`registry::record`**, **`registry::create_product`**, and shared **`ports.rs`**.

**Preconditions:** batches 2 and 3 at HEAD. ORI-T-0194 fold precedents (two-pass JSON; lockout — Q11 / [[R45]] point 3).

## Read first

- `ops/batch-12-plan.md` (D1, D2, D12, D13, FOUNDATION table);
- `ops/tickets/ORI-T-0194-mcp-registry.md` Decision 3–4 (payload read, fold lockout);
- `crates/ori-core/src/document.rs`, `types.rs`, `error.rs`;
- `crates/ori-store/src/event_log.rs`, `db.rs`;
- `crates/ori-mcp/src/ports.rs` (shapes only);
- `crates/ori-watch/src/hooks.rs` `ori_t_0054_a_remote_url_and_the_credential_in_it_are_never_recorded`.

## Decision 1: module layout

- `registry.rs` — registry, fold, payloads, `create_product`, `record`, `FOUNDATION`, kinds.
- `ports.rs` — shared ports (Decision 6).
- `migrate/mod.rs` — doc header only; submodules added later.
- `lib.rs` — `pub mod migrate;`, `pub mod ports;`, `pub mod registry;`.
- Integration tests: **`tests/ori_t_0197_registry.rs`** (ticket id; backlog ORI-T-0060 is satisfied by the same tests).

## Decision 2: FOUNDATION and owner derivation (D12)

Public **`FOUNDATION`**: eleven rows exactly as `ops/batch-12-plan.md` FOUNDATION table. **`owner_for_kind(kind) -> Seat`** from plan D12 (lead reading in module doc).

**No `owner` in event payloads.** Fold derives owner for `Document::apply`.

## Decision 3: event kinds and payloads

Kinds written only through **`registry::create_product`** and **`registry::record`**:

| Kind | Payload fields | Validation (create_product, record, and fold) |
|---|---|---|
| `product.created` | `name`, `repo_path`, `repo_remote?`, `aicd_version`, `origin`, `seats`, `documents` | Human actor on append. **Remote:** refuse password userinfo (`https://user:pass@`, `http(s)://…@` with secret); never echo secret (AICD §27). **repo_path:** canonical **absolute** path stored. **seats:** four PRD S-01 seats, each once, human holders. **documents:** for `origin: "new"`, must match FOUNDATION; for `origin: "migrated"`, may be empty. |
| `document.registered` | `document_id`, `kind`, `path`, `set` | Registration rules (Decision 5). |
| `document.created` / `document.revised` | `document_id`, `pr`, `content_len`, `citations_unresolved` | **All required.** **`pr`:** validated reference (numeric id or URL **without userinfo secrets**). |
| `document.changes_requested` | `document_id`, `instructions` | **Human actor only.** **Non-blank**; **control characters refused**; **max 8192 bytes** UTF-8. |
| `document.approved` | `document_id`, `seat` | Human actor; holder check (Decision 4). |

**Not appended by R1:** `document.submitted`, `document.drifted` — fold **refuses** until Q7 / P-F.

**Write:** `serde_json::to_string`, `#[serde(deny_unknown_fields)]`. **Read:** two-pass (Value object check, then `from_str`) per ORI-T-0194.

## Decision 4: `registry::record`

Checks before append (document in code):
1. Registry not **blocked** (Decision 5).
2. **`document.approved` / `document.changes_requested`:** actor **`Actor::Human`** only.
3. **`document.changes_requested`:** instructions non-blank, no unsafe control chars, ≤8192 bytes.
4. **`document.created` / `document.revised`:** all fields present; `pr` passes same remote/userinfo rules as `product.created` (no password in URL).
5. **`document.approved`:** human **holds** signing seat in folded `product.created.seats` (D13, AICD §18).
6. **Migrated product, phase 1 (D13):** if folded `product.created.origin == "migrated"` and **`seats` is empty**, **`document.approved` for `set == "migration"` documents is refused** (no as-built approval without seat holders; full M2 approval waits ORI-T-0198 / import).
7. Target document exists; `Document::apply` + derived owner; `fires_from` true.
8. Payload serializes.

Returns seq. **`pub`** for L1/DOC integration tests.

## Decision 5: fold, load, lockout

**`DocumentRegistry::read(db)`** — verified read, fold in order.

**On every `product.created` (whoever appended it), the fold re-checks:**
- human actor;
- remote userinfo rule (no password);
- **`repo_path` canonical absolute**;
- seat list (four seats, once each, human holders);
- for `origin: "new"`, documents match FOUNDATION;
- for `origin: "migrated"`, documents may be empty.

Defect → **`EventDefect`**, sets **lockout** (same seq recorded).

**On every `document.*` and `document.registered`, the fold re-checks:**
- payload two-pass validity and field rules from Decision 3;
- **human actor** on `document.approved` and `document.changes_requested`;
- **seat holder** on every `document.approved` (even if appended outside `record`);
- **`Document::apply`** with derived owner seat;
- **`fires_from`**;
- **registration rules:** path under `spec/` (DATA_MODEL:44); unique id and path; kind valid; `set` in `foundation` | `phase` | `migration`.

**Lockout:** first defect sets `blocked`. While blocked: `read`, `record`, **`create_product`** refuse with the same error; last good state readable for diagnostics.

**Refused kinds:** `document.submitted`, `document.drifted`.

## Decision 6: `ports.rs`

`IdSource`, `Clock`, `PortError` (`Refused`, `Denied`, `Invalid`, `Unavailable`) — shapes match `ori_mcp::ports.rs`.

## Decision 7: errors and MethodologyRef

Control refusals carry **`MethodologyRef`** (§8 payload, §13 log, §18 human/seat, §23 foundation, §27 secrets).

## Tests

**34 tests total: 32 in `tests/ori_t_0197_registry.rs`, 2 in `registry.rs` `mod tests`** (payload/`pr`/instruction bounds without a full `ProductDb`).

Each integration test is named **`ori_t_0197_*`**. Plant bad events with `EventLog::append` where noted.

| # | Test name | Decisions | Criteria |
|---|---|---|---|
| 1 | `ori_t_0197_foundation_rows_match_plan` | D2 | — |
| 2 | `ori_t_0197_create_product_new_origin_writes_one_event` | D3 | ORI-P1-001 (payload half) |
| 3 | `ori_t_0197_create_product_refuses_remote_with_password` | D3 | ORI-P1-001 / secrets |
| 4 | `ori_t_0197_create_product_accepts_git_at_host` | D3 | — |
| 5 | `ori_t_0197_create_product_refuses_non_human_actor` | D3 | ORI-P1-001 |
| 6 | `ori_t_0197_create_product_refuses_duplicate_seat` | D3 | D13 |
| 7 | `ori_t_0197_create_product_refuses_non_absolute_repo_path` | D3, D5 | — |
| 8 | `ori_t_0197_create_product_refuses_when_registry_locked` | D5 | — |
| 9 | `ori_t_0197_record_created_from_missing` | D4 | ORI-P1-016 (state half, later slice) |
| 10 | `ori_t_0197_record_refuses_agent_approved` | D4 | ORI-P1-040 |
| 11 | `ori_t_0197_record_refuses_approver_not_holding_seat` | D4, D13 | ORI-P1-040 |
| 12 | `ori_t_0197_record_approver_holding_seat_succeeds` | D4, D13 | ORI-P1-040 |
| 13 | `ori_t_0197_record_refuses_changes_requested_by_agent` | D4 | ORI-P1-040 |
| 14 | `ori_t_0197_record_refuses_overlong_instructions` | D3, D4 | — |
| 15 | `ori_t_0197_record_refuses_pr_with_password_userinfo` | D3, D4 | — |
| 16 | `ori_t_0197_migrated_origin_empty_documents` | D3 | — |
| 17 | `ori_t_0197_migrated_origin_refuses_as_built_approval_without_seats` | D4, D13 | ORI-P1-040 (phase 1 limit) |
| 18 | `ori_t_0197_payload_refuses_json_array` | D3 | — |
| 19 | `ori_t_0197_payload_refuses_duplicate_key` | D3 | — |
| 20 | `ori_t_0197_payload_refuses_trailing_text` | D3 | — |
| 21 | `ori_t_0197_fold_refuses_document_submitted_kind` | D5 | — |
| 22 | `ori_t_0197_fold_refuses_document_drifted_kind` | D5 | — |
| 23 | `ori_t_0197_fold_lockout_on_planted_product_created_bad_remote` | D5 | ORI-P1-001 |
| 24 | `ori_t_0197_fold_lockout_on_planted_product_created_non_absolute_path` | D5 | — |
| 25 | `ori_t_0197_fold_refuses_planted_approval_without_seat_holder` | D5, D13 | ORI-P1-040 |
| 26 | `ori_t_0197_fold_refuses_planted_changes_requested_control_char` | D5 | — |
| 27 | `ori_t_0197_lockout_blocks_record_after_bad_event` | D5 | — |
| 28 | `ori_t_0197_lockout_blocks_create_product_after_bad_event` | D5 | — |
| 29 | `ori_t_0197_lockout_survives_reload` | D5 | — |
| 30 | `ori_t_0197_owner_derived_matches_kind_for_apply` | D2 | ORI-P1-002 (owner half, later RD) |
| 31 | `ori_t_0197_document_registered_birth_edge` | D5 | — |
| 32 | `ori_t_0197_ports_port_error_denied_carries_methodology_ref` | D6 | — |
| 33 | `unit_payload_pr_rejects_userinfo` | D3 | mod tests |
| 34 | `unit_instructions_cap_and_control` | D3 | mod tests |

**Mutations** (re-run after feat commit; each must fail at least one test):

1. fold skips `product.created` remote re-check;
2. fold skips seat-holder re-check on planted `document.approved`;
3. `create_product` skips non-absolute `repo_path` check;
4. reader skips Value object check (array payload);
5. lockout not enforced on `create_product`;
6. `record` skips instruction length cap.

## Checks

fmt, clippy (native + Windows), strict doc, `cargo test -p ori-flows --locked`, gate 13.

## Commits

`feat(flows): add document registry and shared ports` with `Ticket: ORI-T-0197`, `Spec: DATA_MODEL.md#document`.

## What you hand back

Commit hashes, check exit codes, **34** test results, mutation table, deviations, not-tested list, tier 1.
