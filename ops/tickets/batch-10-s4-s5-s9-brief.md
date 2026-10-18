# Batch 10 design brief: S4 (`aicd_report`), S5 (`aicd_ticket_upgrade`, `aicd_spec_propose`), S9 (`aicd_plan_submit`, `aicd_escalate`)

Read at session head 61d435a; S3 read at /home/user/wt/ORI-T-0181 (a576fbe). I read the source myself wherever the maps disagreed or a claim carried weight. Every citation is path:line.

## 0. Where the maps and the records disagree, after reading the source

1. **Who declares `PlanDesk` and `EscalationDesk`: S9 does, not PM2.**
   - `crates/ori-mcp/src/ports.rs:33-38` says "They arrive with slice S9", and `ops/lock-table.md:1122` agrees.
   - `ops/lock-table.md:1529` ("S9 still waits on PM2's ports") is wrong. PM2 delivered only `sanitize_text` and `SanitizedText` (`ops/lock-table.md:1527`; `crates/ori-memory/src/barrier.rs:1897-1915`).
   - S9's declared scope in `ops/batch-10-plan.md:33` lists only `src/tools/orchestration.rs`. It must add `src/ports.rs`.
2. **S4 beyond `closing` and `blocked` is blocked by a template, not by E-0012.** `ops/lock-table.md:1529` says S4 lost its barrier prerequisite. But `spec/API_SPEC.md:142-147` still says `qa_run` and `drift_audit` have "Not yet defined" fields, and `templates/` has no form for either (`ls templates/`: closing-report and blocked-report only).
3. **Event payload line numbers in `server.rs`.** The S4 map is right: `called_payload` is at `crates/ori-mcp/src/server.rs:1602-1609` and `failed_payload` at `:1640-1653`. The S5 map is about 3 lines off. Nothing else differs.
4. **The escalation event's name.** `spec/API_SPEC.md:93` says `escalation.opened`. ori-store projects and replays `escalation.raised` (`crates/ori-store/src/projections/escalation.rs:23-25,156`; `crates/ori-store/src/rebuild.rs:416`).
5. **The engine's dependencies.** The `ports.rs` diagram draws `ENG --> ORCH` (`crates/ori-mcp/src/ports.rs:18`). `crates/ori-engine/Cargo.toml:8-15` has no `ori-orchestrator` line, and `spec/LLD.md:48-55` draws only ENG→FLOWS→ORC. The engine needs that line to implement S9's desks; a downward edge is allowed (`spec/LLD.md:42`).
6. **Who checks a lead's or non-coder's ticket.** All three maps agree, and `server.rs` confirms it:
   - check 7 restricts only the coder (`crates/ori-mcp/src/server.rs:1015-1028`);
   - the downstream owners are S4's report path, S9's escalation desk and S5's ticket desk (`server.rs:189-195`; `crates/ori-mcp/src/tools/mod.rs:1127-1132`).

## X. Decisions that cut across all three slices

**X1. `Send`.**
- `ToolHandler: Send` (`tools/mod.rs:1167`). `IdSource`, `TicketDesk` and `SpecDesk` carry no `Send` bound (`ports.rs:57-60`).
- Recommendation: handlers hold `Box<dyn IdSource + Send>` and `Box<dyn TicketDesk + Send>` (or a generic `D: TicketDesk + Send`). This needs no change to `ports.rs`.
- S9's new traits are declared `: Send`, as `Clock` (`ports.rs:316`) and `SessionDesk` (`ports.rs:334`) are.

**X2. Faults of shape the schema cannot express** (a missing or blank content field, a blank question, a bad module path).
- `HandlerError::Refused` carries a code and a section but no field (`tools/mod.rs:1081-1086`). `PortError::Invalid { field }` carries a field but no section (`server.rs:930-935`).
- ori-core's doctrine is that malformed input is not a refusal and carries no section (`crates/ori-core/src/error.rs:395-401`). S3 already used `PortError::Invalid` for its own check (memory.rs:536-543 in the S3 worktree).
- **Recommendation:** use `PortError::Invalid { field: <a constant JSON pointer>, why: <a constant> }`. The pointer is built only from the slice's own constant names, never an agent key: `/content` for an unexpected key, `/content/<name>` for a missing or blank one.
- Later option: a small S2 follow-up adds `HandlerError::Invalid { field, why }`, mapped to S2's own `invalid_arguments` (AICD §27, `server.rs:140`). That ticket could also close S3's accepted gap (`ops/lock-table.md:1861`).

**X3. Desk refusals that have no ori-core `RefusalKind`.**
- `PortError::Refused` must hold an `ori_core::error::Error` (`ports.rs:114-117`). Three kinds of refusal cannot ride it:
  - `LifecycleError::NotPermitted` (`crates/ori-orchestrator/src/lifecycle.rs:253-271`);
  - `EscalationError` (`crates/ori-orchestrator/src/escalation.rs:436-469`, AICD §12);
  - a spec-desk refusal (no `RefusalKind` fits: `error.rs:226-276`).
- **Recommendation:** add `PortError::Denied { code: &'static str, methodology_ref: MethodologyRef }` in `ports.rs`, with one arm in `Failure::of_error` (`server.rs:917-938`) and a row in its doc table (`server.rs:267-275`).
  - `server.rs:923-936` is the only exhaustive match on `PortError` in the crate, and no test matches it exhaustively, so this needs no test change.
  - Land it with S9, which edits `ports.rs` anyway, adding `server.rs` (one arm) to S9's scope.
  - S5 needs nothing from it, because it propagates `PortError` with `?`.

**X4. Row bounds.** S2 asks each slice to state what its handler adds (`server.rs:148-154`). S3 states 3 × the limit (memory.rs:130-138). The figures are given per slice below.

**X5. Queue and shared test code.**
- Each slice adds one `pub mod …;` line where S3 adds `pub mod memory;` (S3 worktree `tools/mod.rs:62-63`). Module lines are a queue (`ops/lock-table.md:1478`).
- Cut S4, S5 and S9 after S3 integrates, so they can reuse `tools::memory::test_support` (`World`, `Setup`, `file_ticket`, `claim`, `rows_since`; memory.rs:628-1033).

**X6. Every handler appends no event of its own.** This follows S3's "no policy of their own, no event of their own" (memory.rs:7, 132). Effect rows come only from the barrier or the desks.

---

## S4: `aicd_report` (`src/tools/report.rs`, tier 2)

### (1) Readiness
- **Exists:**
  - the barrier: `submit_report`, `NewReport`, `FieldName`, `BarrierConfig` (`barrier.rs:535-554`, `:671-677`, `:1404-1601`);
  - `IdSource` (`ports.rs:170-178`);
  - the catalogue entry (`tools/mod.rs:899-921`);
  - role admission for coder, qa, operations and documentation (`scopes.rs:888-941`);
  - the `evidence/` directory, which `ProductDb` creates (`crates/ori-store/src/db.rs:477`).
- **Missing:**
  - a public "is this ticket filed" read. `scope.rs`'s `ticket_filed` is private (`crates/ori-memory/src/scope.rs:3053-3057`). S4 can read the log itself, so this blocks nothing.
  - field names for `qa_run` and `drift_audit` (`API_SPEC.md:142-147`): a templates/documentation ticket, tier 1.
  - any writer of `ticket.filed` (`scope.rs:232-239`): ORI-T-0070 (ori-engine, tier 1). Until then every report is refused once the existence check is in, which is the same limit S3 accepts (memory.rs:166-171).
  - an `IdSource` implementation: ORI-T-0070.
- **Verdict:** dispatchable now for `closing` and `blocked`, with `qa_run` and `drift_audit` refused by a fixed code. It needs the decisions in (6).

### (2) Call sequence
1. Entry: `fn call(&mut self, db: &mut ProductDb, call: &BoundCall<'_>, arguments: JsonObject) -> Result<CallToolResult, HandlerError>` (`tools/mod.rs:1175-1180`).
2. Ticket: `pub ticket_id: Option<&'a Id>` (`tools/mod.rs:1146`). `None` gives `PortError::Invalid { field: "ticket_id", .. }`, which is unreachable after `server.rs:1476-1502` (S3's precedent).
3. Kind: `arguments["kind"]`, one of `REPORT_KINDS` (`tools/mod.rs:811`), mapped to S4's own enum.
4. Role check against API_SPEC's table (`API_SPEC.md:129-134`), using `pub const fn role(&self) -> Role` (`server.rs:453`).
5. Kind defined: `qa_run` and `drift_audit` are refused here.
6. Content against S4's constant list for the kind (`API_SPEC.md:140-141`), in template order:
   - an unexpected key;
   - a missing required name;
   - a value that trims to empty.

   The map is a BTreeMap (no `preserve_order`, `crates/ori-mcp/Cargo.toml` comment), so iterate S4's list, never the map.
7. Ticket filed, reading the log:
   - `pub fn connection(&mut self) -> &mut Connection` (`db.rs:498`);
   - `pub fn verify(conn: &Connection) -> Result<VerifyReport, EventLogError>` (`crates/ori-store/src/event_log.rs:795`);
   - `pub fn read_range(conn: &Connection, from: u64, to: u64) -> Result<Vec<Event>, EventLogError>` (`:763`);
   - filter `kind == "ticket.filed" && ticket_id == Some(t)`, as `scope.rs:3053-3057` does, using the read pattern of `barrier.rs:2348-2354`.
8. Identifiers, drawn only after every check: `fn next_id(&mut self) -> Result<Id, PortError>;` (`ports.rs:177`), once for the record and once per field.
9. Build the request:
   - `pub fn parse(text: &str) -> Result<Self, BarrierError>` (`barrier.rs:671`), applied to S4's constant names, never the agent's keys;
   - `NewReportField { name, raw_text, evidence_id }` (`barrier.rs:1404-1412`);
   - `NewReport { record_id, kind: RecordKind::ClosingReport | BlockedReport, ticket_id: Some(t.clone()), fields }` (`barrier.rs:1419-1428`).
10. Submit: `pub fn submit_report(db: &mut ProductDb, at: Timestamp, source: Actor, request: NewReport, config: &BarrierConfig) -> Result<Submission, BarrierError>` (`barrier.rs:1518-1524`), with:
    - `at = call.at`;
    - `source = Actor::Agent(call.binding.identity().clone())` (`server.rs:447`);
    - `config` = the engine's `BarrierConfig { pub field_cap_chars: usize }` (`barrier.rs:541-544`; default 4000 at `:535`).
11. Read back: `pub const fn record(&self) -> &MemoryRecord` (`barrier.rs:1447`), then `.id()` (`:811`); `pub const fn truncated_fields(&self) -> &BTreeSet<FieldName>` (`:1460`).
12. Reply: `pub fn text(text: impl Into<String>) -> Self` (`crates/ori-mcp/src/protocol.rs:1687`).

### (3) Refusals and errors

| Source | Becomes | Failure and section |
|---|---|---|
| S2 checks 5, 6, 7 and 8 | S2's own `tool_not_permitted_for_role`, `invalid_arguments`, `claim_contradicts_session` / `ticket_not_assigned`, `call_not_recorded` | `mcp.tool_refused`; §17 / §27 / §25, §7 / §17 (`server.rs:139-142`) |
| `call.ticket_id` is `None` (unreachable) | `Port(Invalid { field: "ticket_id" })` | invalid |
| Kind not permitted for the role | `Refused { code: "report_kind_not_permitted_for_role", §17 }` (recommended) | refused |
| `qa_run` or `drift_audit` | `Refused { code: "report_kind_not_available", §17 }` (recommended; mirrors `tool_not_available`, `server.rs:138`) | refused |
| Unexpected content key | `Port(Invalid { field: "/content" })` (X2) | invalid |
| Missing or blank field | `Port(Invalid { field: "/content/<const>" })` (X2) | invalid |
| Ticket not filed in this product | `Refused { code: "ticket_not_in_product", §25 }`, the code and section `scope.rs:1494,1661-1663` use | refused |
| `IdSource` | `Port(Unavailable)` | unavailable |
| Any `BarrierError` from `submit_report`: `EvidenceWriteRefused` (IdAlreadyRecorded, FileAlreadyExists, EvidenceDirIsLink), `Io`, `Malformed`, `Serialize`, `EventLog` | `Port(Unavailable { what: const })`. Once field names are S4's constants, none of these is the agent's (`barrier.rs:1488-1517`). Never copy the error's `Display`, which carries paths | unavailable |

### (4) Events
- S2 appends `mcp.tool_called` first (`server.rs:1342-1348`); its ticket column is the argument.
- `submit_report` then appends, after writing every file first:
  - N × `memory.evidence_stored`, whose ticket column is the **record id** (`barrier.rs:1582`);
  - 1 × `memory.record_created`, whose ticket column is the ticket (`barrier.rs:1592`).

  All carry `call.at` and the agent actor.
- **No ordering conflict with S2's rule.** The barrier's rows follow `mcp.tool_called`. `scope.rs` attributes evidence by walking back from the record while the actor and `at` match, and stops at `mcp.tool_called` (`scope.rs:2836-2850`). The handler must append nothing between the barrier's rows.
- A pass-2 failure leaves orphan evidence rows and then `mcp.tool_failed`. `scope.rs` never hands those rows out.
- **Bound:** at most 12 rows per admitted call (closing: 1 + 10 + 1; the failure path is 1 + ≤10 + 1). A window holds **at most 12 × the limit plus 1 row per connection**.

### (5) Reply
- One text block holding the canonical record id, plus, when the cap cut something, the truncated fields' **constant** names.
- No `structuredContent`, as in S3 (memory.rs:122-128). A canonical id is not free text (`server.rs:165-169`).
- No content key, value or error `Display` reaches the reply. `mcp.*` payloads carry only ids and lengths (`server.rs:1602-1609`).

### (6) Contradictions and decisions
- **D1, ticket existence.** Recommended: read the log in the handler, for every role including the coder (fail closed, as `scope.rs` does). Adding a `TicketDesk` method would break `impl TicketDesk for Scripted` (`ports.rs:430-440`) and need `test_modified`. Cost: a second whole-log verify per call, since the barrier already does one (`barrier.rs:1568`); accept it, as S3 D12 did.
- **D2, required fields.** Recommended: every listed field of the kind is required and non-blank (closing 10, blocked 5), and `none` is accepted as ordinary text. Do not read the ticket's kind for `new_criteria` or `provenance` (`templates/closing-report.md:22,24`).
- **D3, the cap.**
  - The schema accepts 8192 characters (`tools/mod.rs:913`); the barrier cuts at 4000 (`barrier.rs:535`).
  - Recommended: keep the engine's `BarrierConfig`, and name the cut fields in the reply. The raw text stays whole in the evidence file (`barrier.rs:1476-1480`).
  - Do not change the catalogue: its bounds are pinned by `tools/mod.rs:1590` and the tests that follow.
- **D4, a blocked report has no side effect in S4.** The API row gives it none (`API_SPEC.md:107`). The engine reacts to `memory.record_created` of kind `blocked_report`. PD-2 reconciles `report.*` (`API_SPEC.md:93`) and the Report row's `session_id` (`spec/DATA_MODEL.md:52`), which the barrier's record does not carry.
- **D5, `budget_consumed`** is stored as given. The runtime meter is the authority; the engine compares.
- **Recorded, not decided here:**
  - ticketless qa and operations sessions must still name a ticket (`ops/batch-10-plan.md:57`);
  - `drift_audit` would be stored as `Finding`, which `scope.rs` puts in `operational_defect` (`scope.rs:2402-2410`), a source documentation's memory scope cannot read (`spec/ENV_SETUP.md:63`);
  - S4's role rests on the binding alone until E-0013 condition 6 (`server.rs:24-29`).

---

## S5: `aicd_ticket_upgrade`, `aicd_spec_propose` (`src/tools/desk.rs`, tier 2)

### (1) Readiness
- **Exists:**
  - `TicketDesk::upgrade` (`ports.rs:206-212`) and `SpecDesk::propose` (`ports.rs:243-249`);
  - the catalogue entries (`tools/mod.rs:971-983`, `:994-1009`);
  - admission: lead only for the upgrade (`scopes.rs:899-905`), documentation only for the proposal (`scopes.rs:937-946`).
- **Missing, all on the implementer side (ori-engine, ORI-T-0070, tier 1):**
  - both desk implementations (`crates/ori-engine/src/lib.rs` is a 12-line doc comment);
  - a document registry or projection (`crates/ori-store/src/projections/` holds only escalation, lock and ticket);
  - an event kind for a proposal (`API_SPEC.md:93` lists only `pr.*` and `document.*`);
  - a path that commits content to a branch (`crates/ori-core/src/adapters.rs:545-552`'s `open_pr` needs an existing head branch, a title and a body).
- **Verdict:** dispatchable now against fake desks (pattern: `ports.rs:412-452`). It has the fewest decisions of the three.

### (2) Call sequence
**Upgrade:**
1. `call.ticket_id`; `None` gives `Port(Invalid { field: "ticket_id" })`.
2. `arguments["category"]` parsed with `fn from_str(s: &str) -> Result<Self>` (`crates/ori-core/src/types.rs:77-80`); a failure gives `Invalid { field: "category" }` (unreachable).
3. `pub fn caller(&self) -> Caller` (`server.rs:472-474`).
4. `self.desk.upgrade(db, &caller, ticket, to)?`. The `?` passes the `PortError` through via `From` (`tools/mod.rs:1091-1095`), so `E_UPGRADE_ONLY` and §11 survive (`server.rs:270`).
5. `CallToolResult::text(..)`.

**Propose:**
1. `arguments["document_id"]` parsed with `pub fn parse(text: &str) -> Result<Self>` (`types.rs:120`); a failure gives `Invalid { field: "document_id" }`.
2. `arguments["content"]` as `&str`. It is not sanitized (`spec/SECURITY_NOTES.md:51`).
3. `self.desk.propose(db, &caller, &document, content)?` returns `String`.
4. Reply.

### (3) Refusals and errors

| Source | Becomes | Failure and section |
|---|---|---|
| S2 check 5 (another role) | `tool_not_permitted_for_role` | refused, §17 (`server.rs:139`) |
| S2 check 6 (category not in the enum, document id not a ULID, content of 0 or more than 65 536 characters, an extra key) | `invalid_arguments` | §27 |
| Check 7 | passes every lead (`server.rs:1016-1018`) | none |
| Desk: agent downgrade on the ladder | `Port(Refused(CategoryDowngrade))`, code `E_UPGRADE_ONLY` | refused, §11 (`error.rs:293,339`; `ticket.rs:820-826`) |
| Desk: ticket or document not in the caller's product | `Port(Invalid { field: "ticket_id" \| "document_id" })` | invalid (`ports.rs:204,241`) |
| Desk: §17 matrix refusal, or a spec refusal | needs X3's `Denied`; today only Invalid or Unavailable (section lost) | see X3 |
| Desk: store, host or broker down | `Port(Unavailable)` | unavailable |
| Desk returns `Refused(Error::Malformed)` | recorded as `invalid`, with no code | `server.rs:927` |

### (4) Events
- `mcp.tool_called`. Its ticket column is the argument for the upgrade, and the binding's ticket or none for the proposal (`server.rs:1338-1341`).
- The upgrade desk appends `ticket.category_set {category}` (`crates/ori-store/src/projections/ticket.rs:36`) and only on success, because `Lifecycle::advance` is pure (`lifecycle.rs:484-488`).
- The proposal desk's row is undefined.
- **Ordering:**
  - Within one `receive` the rows are contiguous (`server.rs:288-292`).
  - `propose` holds the product's single writer across a network call, because the port takes `&mut ProductDb`. A slow host stalls every connection of the product.
  - A PR opened before a failed append exists unrecorded. `mcp.tool_called` still precedes it, so S2's rule holds.
- **Bound:** 3 × the limit plus 1 per connection, **only if** the port docs require at most one desk row per call. Recommend S5 states that as a requirement on the implementer in its module doc.

### (5) Reply
- **Upgrade:** a text block made only of the canonical ticket id and the enum value. The port returns `()`, so there is no seq, although the preamble says "Every mutating method returns the `Event.seq`" (`API_SPEC.md:10`).
- **Propose:** the reference is host text, opaque and unbounded (`adapters.rs:300-341`), so it is integration data (CLAUDE.md rule 7). Recommended:
  - pass it through `sanitize_text(ref, MAX_PR_REF_CHARS)` (`barrier.rs:1897`) for neutralization;
  - if `truncated` is set, fail closed as `Unavailable`;
  - label the reference in the text.

  Never echo `content`.

### (6) Contradictions and decisions
- **D1, "Upgrade only" versus ori-core.** ori-core lets an agent make a no-op set and any move to or from `product_signal` (`crates/ori-core/src/ticket.rs:548-571`), and lets a Closed ticket's category be raised (`ticket.rs:800-813`). The port refuses exactly what `Ticket::apply` refuses "and nothing else" (`ports.rs:197-199`). Recommended: no extra rule in the handler, because a second enforcement is what `lifecycle.rs:14-17` warns against. Raise the question for PD-2.
- **D2, `spec/<ticket>`, title, body and session.**
  - `SpecDesk::propose` receives none of them (`ports.rs:243-249`; `Caller` at `scopes.rs:461-465`). The role file requires `spec/<ticket>` branches (`spec/agents/documentation.md:11`), and `open_pr` requires a title and a body.
  - Widening the port changes `impl SpecDesk for Scripted` (`ports.rs:442-452`), which is test code, so it needs `test_modified`.
  - Recommended: no port change. The engine resolves the session and ticket from the identity's live binding (PW, `ops/batch-10-plan.md:44`). Record this as a limit.
- **D3, a test of ORI-P1-005** at the handler, against a fake desk (`spec/criteria/phase-1.md:11`).
- **D4, the credential.** `spec/ENV_SETUP.md:63` gives documentation "Installation token scoped to spec branches". Whose token opens the PR is the engine's decision (`ports.rs:219-221`).

---

## S9: `aicd_plan_submit`, `aicd_escalate` (`src/tools/orchestration.rs` plus `src/ports.rs`, tier 2)

### (1) Readiness
- **Exists:**
  - `sanitize_text` and `SanitizedField` (`barrier.rs:727-745`, `:1897-1915`);
  - `ModulePath::parse` (`scope.rs:1052-1084`), reachable because ori-mcp depends on ori-memory;
  - the catalogue entries (`tools/mod.rs:876-898`, `:922-944`);
  - admission: coder for both, lead for `aicd_escalate` (`scopes.rs:888-905`);
  - `LockTable::claim`, `Escalation::raise` and `Lifecycle::advance` (ori-orchestrator; lock table and escalation tier 1, lifecycle tier 2, `spec/RISK_MAP.md:19-20`).
- **Missing:**
  - the **`PlanDesk` and `EscalationDesk` traits** (S9, ori-mcp, tier 2);
  - the X3 variant (S9, `ports.rs` plus one arm in `server.rs`, tier 2);
  - a `plan.*` event kind (PD-2, tier 1);
  - the `escalation.raised`/`opened` name (PD-2; renaming the store's kind would modify tests);
  - evidence and context-package columns (`crates/ori-store/migrations/0002_projections.sql:80-92`): migration 0003, tier 2;
  - any writer of `lock.claimed`, `escalation.raised`, `ticket.escalated` or `session.*`;
  - a fold of the lock table and a reconstruction of `Ticket` (`proj_tickets` omits `declared_scope`, `0002_projections.sql:28-32`);
  - the engine's `ori-orchestrator` manifest line: ORI-T-0070, tier 1;
  - any suspension mechanism or configuration key (only `API_SPEC.md:108` mentions it; `spec/DATA_MODEL.md:116` has no Suspended state).
- **Verdict:** the handlers and ports can be built against fakes, but not before the lead decides D1 to D6 below. Dispatch S9 last.

### (2) Call sequence (proposed shapes)

**Ports.** These are proposals, not existing code. They mirror `SessionDesk`, which takes `&Binding` (`ports.rs:344`), because `Caller` has no session (`scopes.rs:461-465`):
- `fn submit(&mut self, db: &mut ProductDb, binding: &Binding, ticket: &Id, plan: &SanitizedField, declared_scope: &[ModulePath]) -> Result<Id, PortError>`
- `fn open(&mut self, db: &mut ProductDb, binding: &Binding, ticket: &Id, escalation: &NewEscalation) -> Result<Opened, PortError>`, where:
  - `NewEscalation` holds `trigger: &'static str` (taken from `ESCALATION_TRIGGERS`, `tools/mod.rs:798-807`), the question and recommendation as `SanitizedField`, and the evidence as `Vec<(SanitizedField, SanitizedField)>`;
  - `Opened` holds the escalation id and `suspended: bool`.

  The desks mint their own ids, so the handler needs no `IdSource`.

**Plan:**
1. Take `call.ticket_id`.
2. Sanitize the plan: `sanitize_text(plan, 65_536)`, at the schema maximum (`tools/mod.rs:884`). A `truncated` result is a defect and becomes `Unavailable`.
3. Validate each scope entry with `pub fn parse(text: &str) -> Result<Self, PathRejection>` (`scope.rs:1052`), and also refuse `"` (see D4).
4. Call `desk.submit(..)?`.
5. Reply.

**Escalate:**
1. Take the trigger as its `&'static str` constant.
2. Sanitize the question and recommendation (8192), each `what` (2048) and each `found_at` (1024), at the schema maxima (`tools/mod.rs:826-938`).
3. Refuse any value that is blank after trimming (X2), because `Escalation::raise` and `Finding::new` refuse blanks (`escalation.rs:253-259,385-395`).
4. Call `desk.open(..)?`.
5. Reply.

### (3) Refusals and errors

| Source | Becomes | Failure and section |
|---|---|---|
| S2 checks 4 to 7 | `tool_not_available` / `tool_not_permitted_for_role` / `invalid_arguments` / coder ticket rule | §17 / §17 / §27 / §25, §7 |
| Scope entry fails `ModulePath::parse`, or holds `"` | `Port(Invalid { field: "/declared_scope" })` (X2; an index cannot go in a `&'static str`) | invalid |
| Blank text after sanitizing | `Port(Invalid { field: "/question" \| "/recommendation" \| "/evidence" })` | invalid |
| `LockTable::claim` conflict, via the desk | `Port(Refused(ScopeLocked))`, code `E_SCOPE_LOCKED` | refused, §12 (`error.rs:307,340`; `server.rs:270`) |
| Ticket not in the product (a lead's ticket is unchecked, `server.rs:194`) | `Port(Invalid { field: "ticket_id" })` | invalid |
| `TicketTransition` (escalating a ticket that is not InProgress) | `Port(Refused(..))` with no code | refused, with the fallback reason `refused`, §11 (`server.rs:271`) |
| `EscalationError` or `NotPermitted` inside the desk (unreachable after the handler's checks) | X3's `Denied` (§12 / §17) | refused |
| `Scope::new` gives `Malformed` | must not be wrapped as `Refused`, or it is recorded `invalid` with no code (`ports.rs:114-116`) | invalid |
| An append fails inside the desk | `Port(Unavailable)`; earlier rows stay (`event_log.rs:725-751`) | unavailable |

### (4) Events
- **Plan, on success:**
  - `mcp.tool_called`;
  - a plan row (kind to be named);
  - one `lock.claimed {module, session_id}` per **newly** claimed module (`crates/ori-store/src/projections/lock.rs:38-41`).

  That is at most 66 rows per call (the schema allows 64 entries, `tools/mod.rs:891`), so a window holds at most **66 × the limit plus 1 per connection**.
- **`E_SCOPE_LOCKED`:** only `mcp.tool_called` and `mcp.tool_failed`, **if the desk checks before it stores** (D2). Otherwise a plan row sits inside a "failed" call.
- **Escalate:**
  - `mcp.tool_called`;
  - `escalation.raised {escalation_id, trigger, question, recommendation, evidence}` (the projection ignores evidence);
  - `ticket.escalated` when it applies;
  - `session.*` and teardown rows if the session is suspended.
- **Two payload hazards** come from the projection reader, which cuts a value at the first `"` (`crates/ori-store/src/projections/payload.rs:20-22,38-47`):
  - a `"` in a module path makes the memory scope read the claim two ways and refuse every memory call of that coder (`scope.rs:229-231`);
  - a quote in a question shows up cut in the human queue.

### (5) Reply
- Ids and counts only:
  - plan: the plan id and the number of modules claimed;
  - escalate: the escalation id and whether the session was suspended.
- Neither the plan, the question nor the evidence is echoed.
- After a suspension, the next call is refused `session_not_active` and the connection closes (`server.rs:136`).

### (6) Contradictions and decisions
- **D1, port shapes as above.** S9's scope gains `ports.rs`. Correct `ops/lock-table.md:1529`. Update the "Six ports" text and diagram (`ports.rs:15-38`).
- **D2, plan order.** Recommended: check, then store; nothing is stored on `E_SCOPE_LOCKED`. `API_SPEC.md:106` fixes no order, and `templates/plan.md:3` ties the plan's declared scope to the lock table.
- **D3, what the lock check claims.** Recommended:
  - an extension of the ticket's own claim (`crates/ori-orchestrator/src/lock_table.rs:95-112`);
  - only for an InProgress ticket, because claims made outside an in-flight state are never released (`lifecycle.rs:538-547`).
- **D4, path rules.** Recommended: `ModulePath::parse` plus refusing `"`. `Scope::new` alone is too loose (`types.rs:368-383`). Checking against the product root (`spec/SECURITY_NOTES.md:53`) is the desk's job.
- **D5, the event name.** Recommended: keep `escalation.raised`, and have PD-2 change `API_SPEC.md:93`. Renaming in ori-store would modify the tests at `rebuild.rs:416` and `projections/escalation.rs:329ff`.
- **D6, suspension.** Recommended:
  - the port returns `suspended`;
  - it is false in phase 1 until a configuration key exists;
  - a lead's session is never suspended;
  - the ticket moves to Escalated only when it is InProgress and is the caller's bound ticket; otherwise the escalation opens without a transition.
- **Also:**
  - The `E_SCOPE_LOCKED` reply does not name the module (`server.rs:917-929`). Keep it that way, so another ticket's claim is not disclosed.
  - The template's "Options rejected" and "What is blocked" fields (`templates/escalation.md:17-18`) and the `context_package_ref` column (`DATA_MODEL.md:51`) are not carried by the tool. PD-2 decides, possibly through a `contract_change` escalation.
  - Pin `ESCALATION_TRIGGERS` to `Trigger::ALL` with a test in ori-engine, the one crate that sees both (`escalation.rs:128-158`).

---

## Dispatch order
1. **Integrate S3 (ORI-T-0181) first.** It settles the `tools/mod.rs` module-line queue and supplies `test_support`.
2. **S5 and S4 in parallel next.** Their files are disjoint, and they share only the queued module line.
   - S5 is the smallest, needs no port change, and proves the `Send` and pass-through desk pattern S9 reuses.
   - S4 is the most valuable: CLAUDE.md rule 8 and R25's expiry (`ops/rulings.md:34,177-181`) wait on it. Give S4's package D1 to D5 and the X2 rule.
3. **S9 last.** Dispatch it once the lead has fixed X3 and D1 to D6, with scope `orchestration.rs`, `ports.rs` and one arm in `server.rs`. If suspension stays open, D6's flag lets S9 proceed anyway. Splitting S9 into a plan half and an escalate half is optional.

Files cited most: `/home/user/ORI-STUDIO/crates/ori-mcp/src/server.rs`, `/home/user/ORI-STUDIO/crates/ori-mcp/src/tools/mod.rs`, `/home/user/ORI-STUDIO/crates/ori-mcp/src/ports.rs`, `/home/user/ORI-STUDIO/crates/ori-memory/src/barrier.rs`, `/home/user/ORI-STUDIO/crates/ori-memory/src/scope.rs`, `/home/user/ORI-STUDIO/spec/API_SPEC.md`, `/home/user/ORI-STUDIO/ops/lock-table.md`, `/home/user/wt/ORI-T-0181/crates/ori-mcp/src/tools/memory.rs`.