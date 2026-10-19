# ORI-T-0187: batch 10 slice S4, the report tool `aicd_report` (Sonnet: the lead decided every rule below; implement it)

| Field | Value |
|---|---|
| Worktree | `<WORKTREE>` |
| Branch | `<BRANCH>`, at `<BASE>` (the session head, where S2 and S3/S8 are integrated) |
| Tier | **2** (`spec/RISK_MAP.md`: `crates/ori-mcp/src/tools/`); the operator merges |
| Declared scope (claim <CLAIM>) | `crates/ori-mcp/src/tools/report.rs` (new), and in `crates/ori-mcp/src/tools/mod.rs` **only** the line `pub mod report;` with a `///` line above it, placed in alphabetical order among the `pub mod` lines. Nothing else. No existing test changes |
| Spec trailer | `Spec: API_SPEC.md#3-agent-facing-mcp-server` |
| Ticket trailer | `Ticket: ORI-T-0187`, **one per commit** |
| Commit types | only `feat`, `fix`, `docs`, `ci`, `chore`. **Never `test(...)`** |

## Why

`spec/API_SPEC.md` section 3 gives `aicd_report(ticket_id, kind, content)` to coder, qa, operations and documentation: "Closing, blocked, QA run and drift audit reports through the sanitization barrier", with the kinds and fields under "Report kinds". It is how an agent files the closing report and the blocked report that `CLAUDE.md` and AICD appendix A.5 require. `ori_memory::barrier::submit_report` is the barrier: it sanitizes every field, writes each raw text to its evidence file, and appends the record. S2 admits the four roles and validates the arguments against the schema. This ticket is the handler between them. It writes **no event of its own** and no text but fixed labels.

Read first, on your base:
- `spec/API_SPEC.md` section 3, all of it, "Report kinds" above all, and `templates/closing-report.md` and `templates/blocked-report.md`;
- `crates/ori-memory/src/barrier.rs`'s module doc and `submit_report`, `NewReport`, `NewReportField`, `FieldName`, `BarrierConfig`, `Submission` (`record`, `evidence`, `truncated_fields`), `RecordKind`, `BarrierError`;
- in `crates/ori-memory/src/scope.rs`: `ticket_filed` and `read_log`, and `attributed_evidence`. The last walks back from a record's event over the evidence run, which is why nothing may be appended between the barrier's rows;
- `ports.rs`: `IdSource`;
- `server.rs`'s module doc and `tools/mod.rs` (`ToolHandler`, `BoundCall`, `HandlerError`, the `Tool::Report` catalogue entry);
- `tools/memory.rs` (S3), all of it, the model for a slice;
- `ops/tickets/batch-10-s4-s5-s9-brief.md`, the S4 section and X1 to X6;
- `ops/tickets/ORI-T-0184-s7-aicd-evidence.md`, "An assumption these constants rest on": S7, which shows a record's evidence, sized its budget on this tool filing **one** `NewReportField` per `content` entry, with `raw_text` unchanged. This ticket honours that.

## Decision 1: one handler

```rust
pub struct ReportHandler { ids: Box<dyn IdSource + Send>, config: BarrierConfig }
impl ReportHandler { pub fn new(ids: Box<dyn IdSource + Send>, config: BarrierConfig) -> Self; }
```

- The engine registers it with `Handlers::register(Tool::Report, ..)` and gives it the product's `BarrierConfig` (`field_cap_chars`, default 4000). A doc example shows it.
- The kinds and fields are constants in this file, in template order, exactly API_SPEC section 3's names:
  - `CLOSING_FIELDS: [&str; 10]`: `modules_touched`, `criteria_covered`, `outcome`, `approaches_tried_and_rejected`, `deviations_from_the_plan`, `escalation_decisions`, `specification_update`, `new_criteria`, `what_the_next_agent_should_know`, `provenance`;
  - `BLOCKED_FIELDS: [&str; 5]`: `budget_consumed`, `approaches_tried`, `hypothesis`, `what_it_would_try_next`, `what_it_needs_from_a_human`.
- A private `enum ReportKind { Closing, Blocked, QaRun, DriftAudit }` with the four wire names.
- The role table:

  | Role | May write |
  |---|---|
  | coder | `closing`, `blocked` |
  | documentation | `closing`, `drift_audit`, `blocked` |
  | qa | `qa_run`, `blocked` |
  | operations | `blocked` |
  | any other | nothing (S2 refuses it first) |

**Field names and `why` texts.**
- A whole argument is named bare: `ticket_id`, `kind`, `content`, as S3 and S5 name theirs.
- A field inside `content` is a JSON pointer, `/content/<name>`, as S2's own refusals point inside an argument.
- Each `why` is a named private constant, never built from an agent's key:
  - `WHY_TICKET = "the call carries no ticket"`;
  - `WHY_KIND = "the kind argument is missing or is not one of the four"`;
  - `WHY_CONTENT = "the content argument is missing or is not an object"`;
  - `WHY_UNEXPECTED = "content holds a key that is not a field of this kind"`;
  - `WHY_MISSING = "a field of this kind is missing"`;
  - `WHY_BLANK = "a field of this kind is blank"`;
  - `WHY_NOT_TEXT = "a field of this kind is not a string"`.

## Decision 2: the call, step by step, every check before anything is drawn or written

1. **The ticket** is `call.ticket_id`. `None` is `Port(PortError::Invalid { field: "ticket_id", why: WHY_TICKET })`, unreachable after S2.
2. **The kind** is `arguments.remove("kind")` as a string, parsed to `ReportKind`. Anything else is `Port(Invalid { field: "kind", why: WHY_KIND })`, unreachable after S2's enumeration.
3. **The role may write the kind** (the table). If not: `HandlerError::Refused { code: "report_kind_not_permitted_for_role", methodology_ref: AICD §17 }`, the section S2's `tool_not_permitted_for_role` uses.
4. **The kind has fields.** `qa_run` and `drift_audit` have none yet: API_SPEC section 3 says no template defines them. Each is `Refused { code: "report_kind_not_available", methodology_ref: AICD §17 }`, the section S2's `tool_not_available` uses. The module doc says a template ticket must name the fields first.
5. **The content** is `arguments.remove("content")` as an object (`Port(Invalid { field: "content", why: WHY_CONTENT })` otherwise, unreachable). The map is a `BTreeMap`, and its keys are the agent's text. **The method, and so the order of the faults:**
   1. Walk **the kind's constant list in template order**, and `remove` each field from the map. For the first field that is missing, not a string (unreachable after S2), or trims to empty, answer `Port(Invalid { field: "/content/<name>", why: WHY_MISSING | WHY_NOT_TEXT | WHY_BLANK })`, and stop.
   2. Only after the whole list passes: if any key is left in the map, answer `Port(Invalid { field: "content", why: WHY_UNEXPECTED })`.

   So a missing or blank field is reported before an extra key, and the first in template order wins. `field` is a `&'static str`: build the `/content/<name>` pointers as constants from the lists (a `match` or a parallel const array), **never from an agent key**. `none` is ordinary text: every field of the kind is required and non-blank (D2). S2's schema already refuses an eleventh key (`content` holds at most ten).
6. **The ticket is filed in this product.** Read the log as `scope.rs`'s `read_log` does (`EventLog::verify`, then `read_range` to the tip), and look for an event of kind `ticket.filed` whose ticket column is the ticket, as `ticket_filed` does.
   - Not filed: `Refused { code: "ticket_not_in_product", methodology_ref: AICD §25 }`, the code and section the enforcer uses. This holds for every role, the coder included (D1).
   - A log that cannot be read: `Port(PortError::Unavailable { what: "the product's log".to_owned() })`.
7. **The identifiers**, drawn only now: `self.ids.next_id()?` once for the record, then once per field in template order. The `?` keeps `PortError::Unavailable`.
8. **The request.** `NewReport { record_id, kind: RecordKind::ClosingReport | RecordKind::BlockedReport, ticket_id: Some(ticket.clone()), fields }`. Each field is `NewReportField { name: FieldName::parse(<the constant>), raw_text: <the value, unchanged>, evidence_id }`, one per field of the list, in template order. `FieldName::parse` of a constant cannot fail; map a failure to `Unavailable` rather than panic.
9. **Submit.** `barrier::submit_report(db, call.at, Actor::Agent(call.binding.identity().clone()), request, &self.config)`.
   - Any `BarrierError` is `Port(PortError::Unavailable { what: "the report store".to_owned() })`, `WHAT_STORE`. Never copy its `Display`: it carries paths.
   - The agent's words cannot cause one any more: the field names are the handler's constants, and the ids are the engine's.
   - This drops the barrier's own §8 refusals (`EvidenceWriteRefused`) to `unavailable`. That is deliberate, since none is the agent's to fix. Say so under the limits.
10. **The reply**, one text block with no `structured_content` and no `is_error`:
    - `report recorded: record <record_id>\n`;
    - and, when `truncated_fields()` is not empty, a second line: `cut at the barrier's cap of <self.config.field_cap_chars> characters, the evidence keeps the whole text: <name>, <name>\n`. **The names come from walking the constant list in template order** and keeping those in `truncated_fields()`. They never come from walking the set, which is a `BTreeSet` in alphabetical order.
    - No agent text: no key, no value, no error's `Display`.

**D1, the ticket check is the handler's.** S2's check 7 ties only a coder to its bound ticket. Any other role's ticket passes S2 unchecked, and S2's doc and `BoundCall`'s say "S4 must check that the ticket exists in this product" (S2-F12).
- Reading the log costs a second verify per call, since the barrier makes one too; it is accepted, as S3's D12 was.
- **Until the engine writes `ticket.filed` (ORI-T-0070), every report is refused `ticket_not_in_product`**, the limit S3 already states. Say so under the limits.

**D2, required fields.** Every field of the kind is required and must not be blank. The templates mark the blocked report's five as required. The closing report's `new_criteria` and `provenance` are required only for some tickets. The handler does not interpret the ticket's kind, although it could read it from the `ticket.filed` payload, so all ten are required: an agent writes `none` where nothing applies. Say under the limits:
- `none` is accepted in every field, including those whose template does not allow it;
- `new_criteria` is required on every ticket;
- a defect closed with `new_criteria: none` is recorded as it is, for the lead's review.

**D3, the cap.** S2's schema accepts 8 192 characters per field, and the barrier keeps at most `field_cap_chars` (4 000 by default) of the sanitized text in the record. The raw text stays whole in the evidence file. The reply names the cut fields, so an agent is told. The catalogue's bounds stay as S2 pinned them.

**D4, a blocked report has no side effect here.** API_SPEC gives the tool none. The engine reacts to the record (kind `blocked_report`). `report.*` events and the Report row's `session_id` (DATA_MODEL section 2) are PD-2's to reconcile.

**D5.** `budget_consumed` is stored as the agent wrote it. The runtime meter is the authority, and the engine compares.

## Decision 3: the module doc

Write it in the style of `memory.rs`'s: what the tool does, in API_SPEC's words; the role and kind tables; a Mermaid sequence diagram; the events, the row bound and the limits.

**Events.** The handler's own code appends nothing.

| Outcome | Rows, in order |
|---|---|
| recorded | `mcp.tool_called` (ticket column: the argument), then one `memory.evidence_stored` per field, in template order (the barrier's; ticket column: **the record id**, as the barrier writes it), then `memory.record_created` (ticket column: the ticket). All carry `call.at` and the agent as actor |
| a refusal before the barrier: a kind not permitted or not available, a ticket not filed | `mcp.tool_called`, then `mcp.tool_failed` (`failure: refused`, the code, the section) |
| a content fault | `mcp.tool_called`, then `mcp.tool_failed` (`failure: invalid`, the constant field) |
| the identifiers, the log or the barrier failed | `mcp.tool_called`, then any evidence rows the barrier appended before it failed, then `mcp.tool_failed` (`failure: unavailable`) |

**Nothing comes between the barrier's rows.** The enforcer attributes a record's evidence by walking back from the record's event over evidence rows of the same actor and time (`scope.rs`'s `attributed_evidence`). The barrier appends them all in one call, with nothing between. The handler appends nothing before or after it. A barrier failure part-way leaves evidence rows that no record claims; the enforcer never hands those out.

**The row bound.** An admitted call adds at most 11 rows of the barrier's: 10 evidence rows and the record. With S2's own rows, a call leaves at most 12. So calls to this tool leave at most twelve times the window's limit plus one row per connection.

**Limits to state:**
- D1's limit: until the engine writes `ticket.filed`, every report is refused;
- D2 and D3;
- `qa_run` and `drift_audit` wait on a template;
- `drift_audit` will be stored as `finding`, which the enforcer files under `operational_defect`, a source documentation's memory scope cannot read (`spec/ENV_SETUP.md` section 5). That is recorded for the template ticket;
- the role is the binding's, until E-0013's condition 6. The handler makes no identity check of its own;
- a qa or operations session must still name a ticket (the schema requires one);
- the barrier's §8 refusals become `unavailable` (step 9);
- **a record id's uniqueness rests on the `IdSource` alone.** The barrier checks evidence ids against the log, never record ids. A repeated record id is stored, and both records then become unreadable as evidence. The engine's `IdSource` (ORI-T-0070) is the guard;
- the records this tool files are readable, as records, by the roles the enforcer's table gives them: `drift_audit`'s routing to `operational_defect` is one case.

## Tests

New only, in `report.rs`'s test module, on `memory.rs`'s `test_support` fixtures:
- build worlds with `Setup::new(role, ticket).build_with(label, |inputs| ..)`, registering `ReportHandler` with a **scripted `IdSource`** written in the test module. It hands out ids from a list (`test_support::id(..)`), counts how many it drew in a shared counter, and can be told to fail;
- file the ticket with `test_support::file_ticket` unless a test says not;
- assert the **exact** rows each call appended;
- write the harness closure as `build_with(label, move |_| ..)`, since an unused `inputs` is a clippy error;
- compare every `structuredContent` as a `serde_json::Value` built with `json!`.

1. `ori_t_0187_a_coder_files_a_closing_report_through_the_barrier`. A coder bound to T, with T filed, sends all ten fields, each carrying distinct text.
   - The rows are exactly `[mcp.tool_called]`, then ten `memory.evidence_stored`, then `memory.record_created`.
   - **Provenance:** the record's `provenance().actor()` is `Actor::Agent(identity())`. Each of the eleven barrier rows has that actor, and the same `at` as `mcp.tool_called` (S2's `call.at`).
   - `barrier::read_records` holds one record: kind `closing_report`, ticket T, and its `structured` keys exactly the ten names.
   - `barrier::read_evidence` gives ten blobs in template order. Blob `i`'s content (`read_evidence_content`) is field `i`'s text byte for byte.
   - The reply is exactly `report recorded: record <id>\n`, and the id source drew exactly 11 ids.
2. `ori_t_0187_qa_and_operations_file_blocked_reports`. qa, then operations, each naming a filed ticket, file a `blocked` report with its five fields. Each is stored as `blocked_report`, with the rows `[mcp.tool_called]`, five evidence rows, then the record.
3. `ori_t_0187_a_role_writes_only_its_kinds`. Each case is refused with `structuredContent` exactly `{"failure":"refused","reason":"report_kind_not_permitted_for_role","methodology_ref":{"section":17,"subsection":null}}`, the rows `[mcp.tool_called, mcp.tool_failed]`, and **no id drawn**:
   - a coder writing `qa_run`;
   - qa writing `closing`;
   - operations writing `closing`;
   - documentation writing `qa_run`.
4. `ori_t_0187_a_kind_with_no_template_is_not_available`. qa writing `qa_run`, and documentation writing `drift_audit`, are each refused `report_kind_not_available` at §17, with the same rows and no id drawn.
5. `ori_t_0187_content_is_held_to_the_kinds_fields`. Each case gives `structuredContent` exactly `{"failure":"invalid","field":<f>}`, the rows `[mcp.tool_called, mcp.tool_failed]`, and no id drawn. The cases, each on a **blocked** report (five fields, so an extra key stays within S2's ten):
   - the five fields and an extra key named with a marker gives `content`. The marker appears in neither the reply nor any row's payload;
   - `hypothesis` missing gives `/content/hypothesis`;
   - `budget_consumed` and `approaches_tried` both missing gives `/content/budget_consumed`, the first in template order and the last alphabetically;
   - `budget_consumed` of only spaces gives `/content/budget_consumed`;
   - **precedence:** `approaches_tried` blank, `what_it_needs_from_a_human` missing, and an extra key gives `/content/approaches_tried`;
   - the field `""` (empty) gives `/content/<that name>`.
6. `ori_t_0187_a_ticket_not_filed_is_refused`. Two cases, each refused `ticket_not_in_product` at §25, with the rows `[mcp.tool_called, mcp.tool_failed]`, no id drawn, and no file in the product's evidence directory:
   - a qa binding naming an unfiled ticket, with a blocked report;
   - a coder bound to `ticket_t()`, unfiled, with a full closing report.
7. `ori_t_0187_an_id_source_that_cannot_answer_stores_nothing`. The id source fails on its first draw. The result is `{"failure":"unavailable"}`, the rows `[mcp.tool_called, mcp.tool_failed]`, and no `memory.*` row.
8. `ori_t_0187_a_barrier_refusal_is_unavailable_and_shows_no_path`. The id source is scripted outright:
   - draws 1 to 11 for a first closing report (its record, then its ten evidence ids);
   - draw 12 is a fresh record id;
   - draw 13 repeats the first report's first evidence id.

   The second report is a blocked report. The barrier refuses it before writing anything (`EvidenceWriteRefused`, `FileAlreadyExists`, since the first report's file stands at the path).
   - The result is `{"failure":"unavailable"}`, and the rows appended after the tip taken just before the second call are `[mcp.tool_called, mcp.tool_failed]`.
   - Neither the reply nor those rows contain `db.dir()` as a string.
   - A direct `ToolHandler::call` with the same scripted reuse gives `Err(HandlerError::Port(PortError::Unavailable { what }))` with `what == WHAT_STORE`.
9. `ori_t_0187_a_field_cut_at_the_cap_is_named_in_the_reply`. Two cases:
   - **The default cap.** `BarrierConfig::default()`, one field of 5 000 characters. The reply's second line names that field and the cap 4000. The record's structured text for it is 4 000 characters, and its evidence file holds all 5 000.
   - **The handler's own config, in template order.** `BarrierConfig { field_cap_chars: 50 }`, a closing report whose `modules_touched` and `approaches_tried_and_rejected` are 60 characters and the rest short. The reply is exactly `report recorded: record <id>\ncut at the barrier's cap of 50 characters, the evidence keeps the whole text: modules_touched, approaches_tried_and_rejected\n`: template order, the reverse of alphabetical.
10. `ori_t_0187_no_agent_text_reaches_the_reply_or_the_calls_rows`. Every field carries a marker. The test makes three calls of its own:
    - a recorded report;
    - an extra-key call (blocked, five fields plus a marker key);
    - an unfiled-ticket call.

    Neither any reply, nor any `mcp.*` row, nor any `memory.evidence_stored` payload holds the marker; those payloads carry only ids, a path and a content type. The marker is expected in `memory.record_created`'s payload (the structured text) and in each evidence file (through `read_evidence_content`).
11. `ori_t_0187_the_field_lists_are_api_specs`. Read the spec with `include_str!("../../../../spec/API_SPEC.md")`, the path from `crates/ori-mcp/src/tools/report.rs`.
    - Find the table row that starts with `` | `closing` | `` and the one that starts with `` | `blocked` | ``, and split each on `|`.
    - Cell 2 equals `` `closing_report` `` and `` `blocked_report` ``, built from `RecordKind::as_str()`.
    - Cell 3's backticked names, in order, equal `CLOSING_FIELDS` and `BLOCKED_FIELDS`.
12. `ori_t_0187_a_reports_evidence_is_attributable`. After test 1's shape (a fresh world), register a lead identity (`test_support::register_in`). `ScopeEnforcer::new(ProductStage::Standard).authorize(..)` with `Query::Evidence { record_id }` for that lead is granted with ten blobs, those of the report. This is a cross-crate pin: the record this tool files is attributable end to end.
13. `ori_t_0187_the_handlers_fallbacks_never_panic`. Call `ToolHandler::call` directly, as S3 does. Each case gives `Port(PortError::Invalid { field, why })` with its constant field, and `why` equal to its named constant:
    - `call.ticket_id` `None`;
    - a missing, non-string or unknown `kind`;
    - a `content` that is not an object;
    - a blocked report whose `hypothesis` is the number 7, which gives `/content/hypothesis` and `WHY_NOT_TEXT`;
    - a blocked report with its five fields and an extra key, which gives `content` and `WHY_UNEXPECTED`, with `why` holding no part of the key.
14. `ori_t_0187_every_role_and_kind_is_decided`. A matrix over the four roles and the four kinds, each call with `content: {}` (S2 accepts an empty map) and a filed ticket, and no id drawn in any cell:
    - permitted and available gives `invalid` at `/content/<first field of the kind>`;
    - permitted but not available gives `report_kind_not_available`;
    - not permitted gives `report_kind_not_permitted_for_role`.

## Mutation proofs

Run each in a `git archive` copy under your worktree's `target/`, with its own `CARGO_TARGET_DIR`. Delete the copy when done.

| # | Mutation | Must fail |
|---|---|---|
| m1 | draw the identifiers before the content check | test 5 (an id drawn) |
| m2 | build the fields from the map's keys, in the map's order | test 1 (blob order) |
| m3 | skip the ticket-filed check | test 6 |
| m4 | let a coder write `qa_run` | test 3 |
| m5 | accept a blank value | test 5 |
| m6 | omit the cut line | test 9 |
| m7 | **in the archive copy only, outside the declared scope and never committed:** in `crates/ori-memory/src/barrier.rs`, one `EventLog::append` of a non-evidence kind between `submit_report`'s evidence loop and its record append | tests 1 and 12 |
| m8 | take the cut names from `truncated_fields()`'s iteration | test 9 |
| m9 | check for an extra key before the constant list | test 5 (precedence) |
| m10 | submit with `BarrierConfig::default()` instead of `self.config` | test 9 |
| m11 | deny a coder `blocked`, or permit qa `drift_audit` | test 14 |
| m12 | skip the ticket-filed check for a coder | test 6 |

## Checks

- The standing gate list, the Windows compile check included.
- `cargo test -p ori-mcp --locked`, run twice.
- `cargo test -p ori-mcp --doc --locked`.
- Gate 13 against `<BASE>`.

Report every exit status as a number. List what is not tested and why: `qa_run` and `drift_audit` content (no template), the engine's `IdSource` and a repeated record id (ORI-T-0070), and the barrier's other refusals (only `FileAlreadyExists` is staged). Confirm that no existing test changed.
