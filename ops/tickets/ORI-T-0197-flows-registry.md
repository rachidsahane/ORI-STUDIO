# ORI-T-0197: batch 12 slice R1, document registry and shared ports

| Field | Value |
|---|---|
| Worktree | assigned at dispatch |
| Branch | `feat/ORI-T-0197-flows-registry` |
| Tier | **1.** `spec/RISK_MAP.md:21` tiers `crates/ori-flows` at 1. Seat-holder enforcement lives in this slice; PD-1 will amend the row reason to "approvals checked in core and by `registry::record`" (`ops/batch-12-plan.md`, lead decision). No tier-2 path change in R1 unless the operator adds `registry.rs` via Q12 |
| Declared scope (claim TBD at dispatch) | **New:** `crates/ori-flows/src/registry.rs`, `crates/ori-flows/src/ports.rs`, `crates/ori-flows/src/migrate/mod.rs` (module stub, doc header only), `crates/ori-flows/tests/ori_t_0060_registry.rs`. **Queue:** `crates/ori-flows/Cargo.toml` (add `ori-core`, `serde`, `serde_json` per plan), `crates/ori-flows/src/lib.rs`, `Cargo.lock`. **Nothing else.** No `init.rs`, `readiness.rs`, `documents.rs`, `launch.rs`, `migrate/env.rs`, `migrate/m2.rs`, `migrate/divergence.rs`. **No existing test changes** |
| Spec anchor | `spec/DATA_MODEL.md` §2 Document, §3 transitions; `spec/API_SPEC.md` §2 event kinds (`product.created`, `document.*`), preamble on mutating methods; `spec/LLD.md` §2 ori-flows; AICD §9, §18, §23 G1 (methodology html:880); `spec/SECURITY_NOTES.md` Secrets (:58-61), paths (:53); `spec/ENV_SETUP.md` where init remote rules apply by analogy |
| Spec trailer | `Spec: DATA_MODEL.md#3-document-lifecycle` |
| Ticket trailer | `Ticket: ORI-T-0197` |
| Commit types | `feat`, `fix`, `docs`, `ci`, `chore` only |
| Model | Sonnet 5.5 |

## Why

Batch 12 decision D1–D2, D5, D12–D13 (`ops/batch-12-plan.md`). Every later slice (R2, RD, DOC, E0, L1, M2b) consumes **`DocumentRegistry`**, **`registry::record`**, **`registry::create_product`**, and shared **`ports.rs`**. Without R1, three folds and three payload writers would collide (ORI-T-0063/0065 risks).

**Preconditions:** batches 2 and 3 present at HEAD. ORI-T-0194 fold precedents apply (two-pass JSON read; lockout on first refused event — behaviour matches H1; operator affirms trade in plan Q11).

## Read first

- `ops/batch-12-plan.md` (D1, D2, D3 payloads for kinds R1 writes, D5, D12, D13, FOUNDATION table);
- `ops/tickets/ORI-T-0194-mcp-registry.md` Decision 3 (two-pass payloads) and Decision 4 (verified read / lockout pattern — adapt for documents);
- `crates/ori-core/src/document.rs` (`DocumentEvent`, `Document::apply`, `fires_from`, `as_str`, `disables_launch`);
- `crates/ori-core/src/types.rs` (`Actor`, `Seat`, `Id`, `Timestamp`);
- `crates/ori-core/src/error.rs` (`Error`, `RefusalKind`, `MethodologyRef`);
- `crates/ori-store/src/event_log.rs` (`append`, `read_range`, `verify`);
- `crates/ori-store/src/db.rs` (`ProductDb`);
- `crates/ori-mcp/src/ports.rs` (`IdSource`, `Clock`, `PortError` — copy shapes, not the type);
- `crates/ori-watch/src/hooks.rs` test `ori_t_0054_a_remote_url_and_the_credential_in_it_are_never_recorded` (remote credential precedent).

## Decision 1: module layout

- `registry.rs` — registry, fold, payloads, `create_product`, `record`, constants `FOUNDATION`, kinds.
- `ports.rs` — shared ports only (Decision 5).
- `migrate/mod.rs` — `//! Migration procedures (M0..M5).` only; submodules added by later slices.
- `lib.rs` — `pub mod ports;`, `pub mod registry;`, `pub mod migrate;` (alphabetical: migrate, ports, registry — match existing style when adding).
- Integration test crate: `tests/ori_t_0060_registry.rs` calling public API.

## Decision 2: FOUNDATION and owner derivation (D12)

Public constant **`FOUNDATION`**: eleven rows `(DocumentKind or kind str, path)` exactly as `ops/batch-12-plan.md` FOUNDATION table. Parallel **`owner_for_kind(kind) -> Seat`** from plan D12 interim mapping (documented as lead reading in module doc).

**No `owner` field in any event payload.** The fold derives the owning seat from kind when applying `document.*` via `Document::apply`.

## Decision 3: event kinds and payloads (R1 writer scope)

Kinds written only through **`registry::create_product`** and **`registry::record`**:

| Kind | Payload fields (JSON keys) |
|---|---|
| `product.created` | `name`, `repo_path`, `repo_remote` (optional null), `aicd_version`, `origin` (`"new"` \| `"migrated"`), `seats`: `[{seat, holder}]`, `documents`: `[{id, kind, path, set}]` |
| `document.registered` | `document_id`, `kind`, `path`, `set` |
| `document.created` | `document_id`, `pr`, `content_len`, `citations_unresolved` |
| `document.revised` | same shape as created |
| `document.changes_requested` | `document_id`, `instructions` |
| `document.approved` | `document_id`, `seat` |

**Not appended by R1:** `document.submitted`, `document.drifted` — if seen in log, fold refuses with defect until Q7/P-F define payloads.

**Write:** `serde_json::to_string` from private `Serialize` structs, `#[serde(deny_unknown_fields)]`. **Read:** two-pass per ORI-T-0194 (Value must be object; then `from_str` on same text).

**`create_product` validation before append:**
- Actor must be `Actor::Human`.
- **`seats`:** exactly the four PRD S-01 seats, each once, each holder parses as human identity.
- **`documents`:** for `origin: "new"`, must match FOUNDATION ids/paths/sets; for `origin: "migrated"`, may be empty (as-built registered later).
- **`repo_remote`:** refuse password userinfo (`https://user:pass@`, etc.); never echo secret in error (AICD §27). Accept `git@host:path`, `ssh://user@host/path` without password.
- **`repo_path`:** canonical absolute path in payload (caller supplies; registry canonicalizes and refuses non-absolute).

## Decision 4: `registry::record`

Single entry for all `document.*` appends in ori-flows. Checks before append (order documented in code):
1. Registry not **blocked** (Decision 5).
2. Actor rules: `document.approved` and `document.changes_requested` → human only.
3. For `document.approved`: human **holds** the signing seat in folded `product.created.seats` (D13).
4. Target document exists; `Document::apply` accepts transition with derived owner seat; `fires_from` true.
5. Payload struct serializes.

Returns appended seq. **`pub`** so integration tests in other crates may build state (L1, DOC tests use it — plan D13).

## Decision 5: fold, load, lockout

**`DocumentRegistry::read(db) -> Result<DocumentRegistry, RegistryError>`** verifies log (`EventLog::verify`), reads new range, folds `product.created`, `document.registered`, and supported `document.*` transitions into document map + seat map.

**Fold is authoritative** (D1): re-validates seat holder on every `document.approved` in log even if appended elsewhere.

**Lockout:** first `EventDefect` sets `blocked: Option<(seq, defect)>`. While blocked: `read`/`record`/`create_product` refuse with same error; document state from last good fold still readable for operator diagnostics (mirror ORI-T-0194 pattern). Module doc states append-only log implication and Q11 operator trade.

**Refused kinds:** `document.submitted`, `document.drifted` → defect until defined.

Registration rules on fold: path under `spec/` (DATA_MODEL:44), unique id/path, kind valid, migration-set rules for as-built kinds (defer detailed as-built rules to M2; fold accepts `set` values `foundation`, `phase`, `migration`).

## Decision 6: `ports.rs`

- **`IdSource`**: same contract as `ori_mcp::ports::IdSource` (doc cites ori-core Id generation rule).
- **`Clock`**: same as `ori_mcp::ports::Clock` (`Send`, `now() -> Timestamp`).
- **`PortError`**: variants `Refused(Error)`, `Denied { code, methodology_ref }`, `Invalid { field, why }`, `Unavailable { what }`; `methodology_ref()` like ori-mcp.

No other traits in R1.

## Decision 7: errors and MethodologyRef

Every refusal that is a control refusal carries **`MethodologyRef`** (CLAUDE.md rule 9). Map defects to sections (§8 malformed payload, §13 log, §18 human/seat, §23 foundation, §27 secrets). **`PortError::Invalid`** for malformed caller input without MethodologyRef when appropriate (D14 repo_path symlink).

## Tests (`tests/ori_t_0060_registry.rs` + unit tests in `registry.rs`)

**22 tests**, each named `ori_t_0197_<snake_case>` (ticket id) or `ori_t_0060_<snake_case>` (backlog id) — **use `ori_t_0197_*` prefix** for new file. Real `ProductDb` in temp dirs; plant bad events with `EventLog::append` where needed.

1. **`ori_t_0197_foundation_rows_match_plan`** — FOUNDATION len 11; paths match plan table.
2. **`ori_t_0197_create_product_new_origin_writes_one_event`** — human actor; payload byte-stable; seats four; eleven documents.
3. **`ori_t_0197_create_product_refuses_remote_with_password`** — `https://u:tok@h/r.git`; no event; error lacks `tok`.
4. **`ori_t_0197_create_product_accepts_git_at_host`** — scp-style remote accepted.
5. **`ori_t_0197_create_product_refuses_non_human_actor`**
6. **`ori_t_0197_create_product_refuses_duplicate_seat`**
7. **`ori_t_0197_record_created_from_missing`** — `document.created`; state Draft.
8. **`ori_t_0197_record_refuses_agent_approved`**
9. **`ori_t_0197_record_refuses_approver_not_holding_seat`**
10. **`ori_t_0197_record_approver_holding_seat_succeeds`**
11. **`ori_t_0197_record_refuses_changes_requested_by_agent`**
12. **`ori_t_0197_payload_refuses_json_array`** — two-pass shape check.
13. **`ori_t_0197_payload_refuses_duplicate_key`**
14. **`ori_t_0197_payload_refuses_trailing_text`**
15. **`ori_t_0197_fold_refuses_document_submitted_kind`**
16. **`ori_t_0197_fold_refuses_document_drifted_kind`**
17. **`ori_t_0197_lockout_blocks_record_after_bad_event`**
18. **`ori_t_0197_lockout_survives_reload`**
19. **`ori_t_0197_owner_derived_matches_kind_for_apply`**
20. **`ori_t_0197_document_registered_birth_edge`**
21. **`ori_t_0197_ports_port_error_denied_carries_methodology_ref`**
22. **`ori_t_0197_migrated_origin_empty_documents`**

Mutations: at least 5 re-run after feat commit (fold skips seat check; create_product skips remote scan; array payload accepted — kill each).

## Checks

- `cargo fmt --all --check`
- `cargo clippy -p ori-flows --all-targets --locked -- -D warnings`
- `CARGO_TARGET_DIR=target/win cargo clippy -p ori-flows --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ori-flows --no-deps --locked`
- `cargo test -p ori-flows --locked`
- Gate 13 on commits

## Commits

Conventional Commits; one `Ticket: ORI-T-0197`, one `Spec:` trailer per commit. Suggested: `feat(flows): add document registry and shared ports`.

## Deviations

If spec and plan conflict, stop and report. Narrowest reading only with listed deviation.

## What you hand back

Commit hashes, check exit codes, test counts, mutation table, deviations, not-tested list, self-assessed tier 1.
