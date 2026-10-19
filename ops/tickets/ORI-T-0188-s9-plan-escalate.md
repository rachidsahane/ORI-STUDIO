# ORI-T-0188: batch 10 slice S9, `aicd_plan_submit` and `aicd_escalate`, their ports, and `PortError::Denied` (Sonnet: the lead decided every rule below; implement it)

| Field | Value |
|---|---|
| Worktree | `<WORKTREE>` |
| Branch | `<BRANCH>`, at `<BASE>` (the session head, where S2 and S3/S8 are integrated) |
| Tier | **2** (`spec/RISK_MAP.md`: `crates/ori-mcp/src/ports.rs`, `server.rs`, `tools/`); the operator merges |
| Declared scope (claim <CLAIM>) | `crates/ori-mcp/src/tools/orchestration.rs` (new). `crates/ori-mcp/src/ports.rs`: `PortError` (the `Denied` variant, its arms in `methodology_ref`, `Display` and `source`, and the docs of the type, of `Refused` and of `methodology_ref`), the docs of `TicketDesk` and `SpecDesk` only, the two new desks and their value types, the `use` lines they need (`ori_memory::barrier::SanitizedField`, `ori_memory::scope::ModulePath`), the module doc, and new unit tests. `crates/ori-mcp/src/server.rs`: **only** the `Denied` arm in `Failure::of_error` and its row in the module doc's failure table. `crates/ori-mcp/src/lib.rs`: **only** the doc lines on `pub mod ports`. In `crates/ori-mcp/src/tools/mod.rs`, **only** the line `pub mod orchestration;` with a `///` line, in alphabetical order. Nothing else. **No existing test changes** |
| Spec trailer | `Spec: API_SPEC.md#3-agent-facing-mcp-server` |
| Ticket trailer | `Ticket: ORI-T-0188`, **one per commit** |
| Commit types | only `feat`, `fix`, `docs`, `ci`, `chore`. **Never `test(...)`** |

## Why

`spec/API_SPEC.md` section 3 gives two tools:
- `aicd_plan_submit(ticket_id, plan, declared_scope)` for the coder: "Stores the plan; lock check; may return `E_SCOPE_LOCKED`";
- `aicd_escalate(ticket_id, trigger, question, recommendation, evidence)` for the lead and the coder: "Opens an escalation; suspends the session if configured", with the evidence rules under "Escalation evidence".

The effects live in `ori-orchestrator`, which `ori-mcp` must never depend on (`ports.rs`'s module doc). So, as for S5, the server reaches them through ports the engine implements (ORI-T-0070). `ports.rs` says `PlanDesk` and `EscalationDesk` "arrive with slice S9": both take agent text that must first pass the sanitization barrier, and PM2 (ORI-T-0176) now gives that text a sealed type, `ori_memory::barrier::SanitizedField`, made by `barrier::sanitize_text`.

This ticket declares the two ports, adds the refusal variant the desks need, and writes the two handlers. The handlers write no event of their own and no text but fixed labels.

Read first, on your base:
- `ports.rs`, all of it;
- `server.rs`'s module doc (the check order, the ticket rule, "After `mcp.tool_called`", the failure table, the log bound) and `Failure::of_error`;
- `tools/mod.rs`: `ToolHandler`, `BoundCall`, `HandlerError`, `ESCALATION_TRIGGERS`, and the catalogue entries `Tool::PlanSubmit` and `Tool::Escalate`;
- `tools/memory.rs` (S3), all of it, and its `test_support`;
- in `ori-memory`:
  - `barrier.rs`: `sanitize_text`, `SanitizedText`, `SanitizedField`;
  - `scope.rs`: `ModulePath::parse`, `PathRejection`, and the module doc's "Module paths";
- in `ori-core`, `error.rs`: `RefusalKind::ScopeLocked` (`E_SCOPE_LOCKED`, AICD §12) and `MethodologyRef`;
- in `ori-orchestrator`: `lock_table.rs` (`claim`), `escalation.rs` (`Escalation::raise`, `Finding`, `Trigger`) and `lifecycle.rs`. You read these to write the desks' contracts; this crate never depends on them;
- `crates/ori-store/src/projections/payload.rs`'s module doc: a value containing `"` cannot be read back;
- `ops/tickets/batch-10-s4-s5-s9-brief.md`, the S9 section and X1 to X6.

## Decision 1: `PortError::Denied`

```rust
/// The component behind the port refused under a rule that `ori-core`'s
/// `RefusalKind` has no variant for (a lifecycle permission, an escalation
/// rule, a specification rule), with its own fixed code and section.
Denied { code: &'static str, methodology_ref: MethodologyRef },
```

- `PortError::methodology_ref()` returns `Some` for it. Its `Display` is `refused under <section>: <code>`. `source()` is `None`.
- `server.rs`: one arm in `Failure::of_error`, `HandlerError::Port(PortError::Denied { code, methodology_ref }) => Self::refused(code, methodology_ref.clone())`, and one row in the module doc's failure table: `failure: refused`, `reason` the code, `methodology_ref` always.
- `ports.rs`'s doc on `Refused` points to `Denied` for refusals with no `RefusalKind`.
- S5's desks may return it as well. Say so on `TicketDesk` and `SpecDesk`: a doc change, no signature change. Add there that a foreign product is answered `Denied { code: "product_database_mismatch", methodology_ref: AICD §26 }`, as the new desks answer it, and that "nothing else" in `TicketDesk`'s doc means no business rule beyond ori-core's. Any other `Denied` code of theirs is left to S5 and ORI-T-0070.
- `PortError`'s doc says four kinds, the two refusal kinds answered alike, and lists `Denied` beside `Refused`. `methodology_ref`'s doc says: the refusal's section for `Refused` (`None` if the held error carries none), always `Some` for `Denied`, and `None` for `Invalid` and `Unavailable`.
- **The codes `Denied` carries are MCP `reason` values**, lower case like S2's and S3's (`source_not_in_scope`), not `E_` codes. A new `E_` code would be a `contract_change`.

## Decision 2: the two ports

```rust
/// Plan operations the server reaches through the engine: AICD §12, AICD §17.
pub trait PlanDesk: Send {
    fn submit(
        &mut self,
        db: &mut ProductDb,
        binding: &Binding,
        ticket: &Id,
        plan: &SanitizedField,
        declared_scope: &[ModulePath],
    ) -> Result<PlanStored, PortError>;
}

/// What a stored plan answers with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanStored { pub plan_id: Id, pub modules_claimed: usize }

/// Escalation operations the server reaches through the engine: AICD §12.
pub trait EscalationDesk: Send {
    fn open(
        &mut self,
        db: &mut ProductDb,
        binding: &Binding,
        ticket: &Id,
        escalation: &NewEscalation,
    ) -> Result<EscalationOpened, PortError>;
}

/// An escalation as the handler hands it to the desk: every text sanitized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewEscalation {
    pub trigger: &'static str,
    pub question: SanitizedField,
    pub recommendation: SanitizedField,
    pub evidence: Vec<EscalationEvidence>,
}

/// One finding behind an escalation: what was found, and where.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationEvidence { pub what: SanitizedField, pub found_at: SanitizedField }

/// What an opened escalation answers with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationOpened { pub escalation_id: Id, pub suspended: bool }
```

- The ports take the **`&Binding`**, as `SessionDesk` does, not a `Caller`: a lock claim records the session (`lock.claimed { module, session_id }`), and `Caller` has none.
- Both are `Send`, as `Clock` and `SessionDesk` are. The desks mint their own ids, so the handlers need no `IdSource`.

**The contracts, written on each trait.** The implementation never assumes the server core's checks ran, so it:
- records its writes with `Actor::Agent`, naming `binding.identity()`;
- refuses a binding whose product is not `db`'s as `Denied { code: "product_database_mismatch", methodology_ref: AICD §26 }`, S2's own code for that fact;
- refuses a ticket the product does not hold, as `PortError::Invalid { field: "ticket_id" }`. S2 checks a coder's ticket against the binding, and nobody checks a lead's;
- answers every refusal with a code and a section: `Refused` for an `ori-core` refusal kind, `Denied` otherwise. A `Malformed` error is never wrapped in `Refused`;
- **makes every refusal, and every `Invalid`, before its first append.** A refused call stores nothing. Only an `Unavailable` (an append that failed) can leave rows behind it;
- writes both its events with the ticket in their ticket column.

`PlanDesk::submit` also:
- **D2, check, then store.** It runs the lock check (`LockTable::claim`) before it stores anything. On a conflict it passes `LockTable::claim`'s error on unchanged, as `Refused(..)` with `RefusalKind::ScopeLocked` (code `E_SCOPE_LOCKED`, AICD §12), and stores nothing. The module stays in the error and never reaches the agent: S2 replies with the code and the section only.
- **D3, what the check claims.** The plan's declared scope extends the ticket's own claim, for an `InProgress` ticket only. This is the lead's choice: a plan is submitted while the ticket is being worked on, and a ticket that is escalated or in review is re-planned only through the lead. Any other state is `Denied { code: "ticket_not_in_progress", methodology_ref: AICD §11 }`.
- **Its rows.** It appends one plan row (its kind is named by PD-2), then one `lock.claimed` per newly claimed module, and nothing else. A module is newly claimed if the lock table did not hold it before the call. `PlanStored::modules_claimed` counts the new claims.

`EscalationDesk::open` also:
- routes the escalation through `ori-orchestrator`'s `Escalation::raise`, which raises a trigger only when a finding stands behind it. An `EscalationError` it returns (unreachable after the handler's checks) is `Denied { code: "escalation_refused", methodology_ref: AICD §12 }`;
- appends `escalation.raised`, the kind ori-store projects (D5). **The payload's keys are in this order: `escalation_id`, `trigger`, `question`, `recommendation`, then `evidence` last.** Use a struct serialized by serde, which keeps its fields' declaration order; a `serde_json::Value` map sorts its keys and must not be used. ori-store's projection reader takes the first `"key"` it finds anywhere in the payload, values included. With this order, the column a human triages on (`trigger`) is read before any agent text, so no question, recommendation or evidence can set it;
- **D6, the transition and the suspension.**
  - The ticket moves to `Escalated` only for a **coder's** escalation on its own bound ticket, when that ticket is `InProgress`. The desk asks `Lifecycle::advance`, which is pure, for the move, as `TicketDesk` does, before its first append. It appends `ticket.escalated` once `advance` accepts. A lead's escalation never moves the ticket: the coder working it would otherwise be stopped by a session it does not hold.
  - The session row, when there is one, carries the ticket in its ticket column.
  - `EscalationOpened::suspended` says whether the session was suspended. It is `false` in phase 1 until a configuration key exists, and always `false` for a lead's session.
- **Its rows:** at most three, `escalation.raised`, `ticket.escalated` and one session row.

**`ports.rs`'s module doc**, every passage the new desks make false:
- "Six ports" becomes "eight ports", in the text and in the diagram's node;
- the diagram's "implements …" label names the two new desks;
- the paragraph that says `PlanDesk` and `EscalationDesk` "are not here" now says they are: they take `SanitizedField`, PM2's sealed type, so neither takes a raw `String` where the barrier's output belongs. Do not write that no port takes raw agent text: `SpecDesk::propose`'s content and `UserToolHost::call_tool`'s arguments are not barrier text, by decision;
- the opening list of what the server reaches (lines 6 to 8) adds plans and escalations;
- "The first four are slice S1's; the last two are slice S2's, and the server core calls them itself" names the two new desks as slice S9's, called by its handlers, not by the server core;
- "`TicketDesk` and `SpecDesk`, which slices S3 to S9 call" names who calls them: slice S5's handlers;
- the paragraph saying the desks of the effect take a `Caller` and only `SessionDesk` takes the `Binding` names the two new desks beside `SessionDesk`, with the reason: a lock claim records the session;
- the paragraph saying only `Clock` and `SessionDesk` are `Send` adds the two new desks.

`lib.rs`'s doc on `pub mod ports` adds plans and escalations to what the ports reach.

**Recorded for ORI-T-0070, not here:** to implement the new desks, ori-engine needs in its manifest `ori-memory` (`SanitizedField`, `ModulePath`), `ori-orchestrator`, and `serde` with `serde_json` (E-0008, on the same pins) for D2's payload struct.

**Field names.** Each `Invalid`'s `field` is the argument's bare name: `ticket_id`, `plan`, `declared_scope`, `trigger`, `question`, `recommendation`, `evidence`. This overrides the brief's pointer form. Each `why` is a named private constant (`WHY_PLAN`, `WHY_SCOPE`, `WHY_TRIGGER`, `WHY_QUESTION`, `WHY_RECOMMENDATION`, `WHY_EVIDENCE`, `WHY_TICKET`). Each truncation's `Unavailable { what }` is a named private constant too:
- `WHAT_PLAN = "the plan's sanitization"`;
- `WHAT_QUESTION = "the question's sanitization"`;
- `WHAT_RECOMMENDATION = "the recommendation's sanitization"`;
- `WHAT_EVIDENCE = "the evidence's sanitization"`, for both `what` and `found_at`.

## Decision 3: `aicd_plan_submit`, step by step (`PlanSubmitHandler { desk: Box<dyn PlanDesk> }`, `pub fn new`)

1. **The ticket** is `call.ticket_id`. For a coder, S2's check 7 makes it the bound ticket. `None` is `Port(Invalid { field: "ticket_id", .. })`, unreachable.
2. **The plan** is a string argument, `Invalid { field: "plan" }` otherwise.
   - Pass it through `sanitize_text(plan, PLAN_MAX_CHARS)`, with `PLAN_MAX_CHARS = 65_536`, the schema's bound.
   - A `truncated` result cannot happen through S2's schema, which counts characters as `sanitize_text` does. A direct call can reach it, and it is `Port(Unavailable { what: WHAT_PLAN })`: fail closed, never store less than was sent. Test 11 pins it.
   - A plan whose sanitized text trims to empty is `Invalid { field: "plan" }`.
3. **The declared scope** is an array of strings.
   - Each entry goes through `ModulePath::parse`.
   - An entry that fails, or that contains `"` (**D4**: ori-store's projection reader cuts a value at its first `"`, so a quoted path would be read two ways and refuse every memory call of that coder), is `Invalid { field: "declared_scope", .. }`.
   - Never put an index or the agent's text in `field` or `why`: both are constants.
   - Keep the entries' order. Duplicates go through: the lock table decides.
4. `let stored = self.desk.submit(db, call.binding, ticket, &plan.field, &paths)?;`
5. **The reply**, one text block: exactly `plan <plan_id> stored for ticket <ticket_id>; <n> module(s) newly claimed\n`. Neither the plan nor any path is echoed.

## Decision 4: `aicd_escalate`, step by step (`EscalateHandler { desk: Box<dyn EscalationDesk> }`, `pub fn new`)

1. **The ticket** is `call.ticket_id`, as above.
2. **The trigger** is a string argument, matched against `ESCALATION_TRIGGERS` and kept as **that array's `&'static str`**. Anything else is `Invalid { field: "trigger" }`, unreachable.
3. **The question and the recommendation** go through `sanitize_text(.., 8_192)`. **Each evidence entry's** `what` goes through `sanitize_text(.., 2_048)` and its `found_at` through `sanitize_text(.., 1_024)`. These are the schema's bounds.
   - `truncated` is `Unavailable`, as for the plan.
   - A text that trims to empty is `Invalid`, with field `question`, `recommendation` or `evidence`. `Escalation::raise` and `Finding::new` refuse blanks, so the handler says so first, with the field.
   - An evidence list that is not 1 to 16 objects, or an entry missing `what` or `found_at` or holding a non-string, is `Invalid { field: "evidence" }`, unreachable after the schema. A non-string question or recommendation is `Invalid` with its own field.
4. `let opened = self.desk.open(db, call.binding, ticket, &NewEscalation { .. })?;`
5. **The reply**, one text block: exactly `escalation <escalation_id> opened for ticket <ticket_id>; session suspended: <true|false>\n`. After a suspension, S2 refuses the next call `session_not_active` and closes; say so in the doc.

## Decision 5: the module doc of `orchestration.rs`

Write it in the style of `memory.rs`'s:
- the two tools in API_SPEC's words;
- a Mermaid sequence diagram;
- the events, the bounds and the limits.

**Events.** The handlers' own code appends nothing.

| Outcome | Rows, in order |
|---|---|
| a plan stored | `mcp.tool_called` (ticket column: the argument), then the desk's plan row, then one `lock.claimed` per new claim |
| an escalation opened | `mcp.tool_called`, then `escalation.raised`, then `ticket.escalated` when D6 applies, then a session row when suspended |
| `E_SCOPE_LOCKED`, or any desk refusal (`Refused`, `Denied`) | `mcp.tool_called`, then `mcp.tool_failed` (`failure: refused`, the code, the section). Nothing is stored: both desks refuse before their first append |
| a ticket not in the product, or an argument the handler rejects | `mcp.tool_called`, then `mcp.tool_failed` (`failure: invalid`, the field) |
| a desk that could not answer | `mcp.tool_called`, then whatever rows the desk appended before it failed, then `mcp.tool_failed` (`failure: unavailable`) |

**Bounds.** With the desks' contracts:
- `aicd_plan_submit` leaves at most 67 rows a call: S2's two (`mcp.tool_called`, and `mcp.tool_failed` on a failure), plus the desk's plan row and up to 64 claims. So calls to it leave at most 67 times the window's limit plus one row per connection;
- `aicd_escalate` leaves at most five rows a call (S2's two and the desk's three), so five times the limit plus one row per connection.

Both are safe upper bounds, counted the same way. Under the desks' contract only an `Unavailable` follows an append, so the tight maxima are 66 and 4. This overrides the brief's 66.

**Limits to state:**
- **D5, the event's name.** `spec/API_SPEC.md` section 2 names `escalation.opened`, while ori-store projects `escalation.raised`. The desk writes `escalation.raised`, and PD-2 corrects API_SPEC: renaming in ori-store would change its tests.
- **ori-store's projection reader is not a JSON parser.** With D2's key order, no value can be read as another column. But the reader undoes no JSON escape, so in the human queue:
  - a `"` in a question, a recommendation or an evidence text cuts the text, after a trailing `\`;
  - every `\` shows doubled.

  The lead records the ori-store follow-up ORI-T-0189 to make the reader parse JSON. The handlers do not refuse quotes in prose.
- The tool does not carry the template's "Options rejected" and "What is blocked" fields, nor the Escalation row's `context_package_ref` (DATA_MODEL section 2). PD-2 decides.
- `ESCALATION_TRIGGERS` and `ori-orchestrator`'s `Trigger` are not pinned to each other here. ori-engine is the one crate that sees both, and pins them in ORI-T-0070.
- Suspension is not configurable in phase 1 (D6).

## Tests

New only. **Unit tests in `ports.rs`** for `Denied`: its `methodology_ref`, its `Display`, and that `source()` is `None`.

**In `orchestration.rs`'s test module**, on `memory.rs`'s `test_support`:
- build worlds with `Setup::new(role, ticket).build_with(..)`, registering both handlers with **fake desks**. A fake records every call in an `Arc<Mutex<Vec<..>>>`: the binding's session, identity and role, the ticket, each sanitized text, the paths, the trigger. It answers from a script, and can append rows through the `db` it is handed with `test_support::append`;
- assert the **exact** rows each call appended.

1. `ori_t_0188_a_coder_submits_a_plan_through_its_desk`.
   - The plan has leading and trailing spaces and carries a NUL and a bell character. The scope is `["crates/z", "crates/a", "crates/z"]`.
   - The desk received:
     - the binding's session and identity, and the bound ticket;
     - the plan untrimmed, with each control character replaced by U+FFFD;
     - **three** parsed paths, in that order, the duplicate included.
   - The fake appends a `plan.submitted` fixture row and two `lock.claimed` rows, and answers `modules_claimed: 2`.
   - The reply is exactly `plan <id> stored for ticket <t>; 2 module(s) newly claimed\n`.
   - The rows are `[mcp.tool_called, plan.submitted, lock.claimed, lock.claimed]`.
2. `ori_t_0188_a_locked_scope_is_e_scope_locked_and_names_no_module`. The desk answers `Refused(Error::refused(RefusalKind::ScopeLocked { module: MARKER.to_owned() }))`.
   - `structuredContent` is exactly `{"failure":"refused","reason":"E_SCOPE_LOCKED","methodology_ref":{"section":12,"subsection":null}}`.
   - The marker appears in neither the reply nor any row.
   - The rows are `[mcp.tool_called, mcp.tool_failed]`.
3. `ori_t_0188_a_denied_refusal_keeps_its_code_and_section`. The desk answers `Denied { code: "ticket_not_in_progress", methodology_ref: AICD §11 }`.
   - `structuredContent` is exactly `{"failure":"refused","reason":"ticket_not_in_progress","methodology_ref":{"section":11,"subsection":null}}`, and the `mcp.tool_failed` row records the same.
   - This pins the new `server.rs` arm.
4. `ori_t_0188_a_declared_scope_is_held_to_module_paths`. Each case gives `structuredContent` exactly `{"failure":"invalid","field":"declared_scope"}`, the rows `[mcp.tool_called, mcp.tool_failed]`, and no desk call. The marker appears in no row. The cases:
   - `/MARKER/x` (absolute);
   - `crates/../MARKER` (a parent step);
   - `crates//MARKER` (an empty segment);
   - `crates/MARKER"x`, valid but for the quote, the one only the quote check catches. The marker stays whole.
5. `ori_t_0188_blank_text_is_invalid_with_its_field`. Each blank is spaces only (`"   "`; an empty string never passes S2's schema). Each gives `structuredContent` exactly `{"failure":"invalid","field":<f>}` and the rows `[mcp.tool_called, mcp.tool_failed]`, and the desk is never called:
   - a plan of spaces gives `plan`;
   - a blank question gives `question`;
   - a blank recommendation gives `recommendation`;
   - an evidence entry whose `found_at` is spaces gives `evidence`.
6. `ori_t_0188_the_lead_opens_an_escalation_through_its_desk`. A lead with no bound ticket names T, with trigger `scope_conflict` (not the array's first entry). The question, the recommendation and two evidence entries with distinct texts each carry a NUL and leading and trailing spaces. The desk receives them with U+FFFD in place, the spaces kept, and the two entries in order.
   - The desk received the trigger equal to `"scope_conflict"` and contained in `ESCALATION_TRIGGERS`, the sanitized question and recommendation, and every evidence entry in order. Compare by value: two uses of a `const` share no address, so a pointer comparison would fail a correct handler.
   - The fake appends `escalation.raised`.
   - The reply is exactly `escalation <id> opened for ticket <t>; session suspended: false\n`.
   - The rows are `[mcp.tool_called, escalation.raised]`.
7. `ori_t_0188_a_coders_escalation_reaches_its_desk_and_reports_a_suspension`. A coder bound to T escalates on T. The desk received the bound ticket and the binding's session and `Role::Coder`. It answers `suspended: true`, and the reply ends `session suspended: true\n`.
8. `ori_t_0188_each_tool_reaches_its_desk_for_its_roles_only`. S2 refuses before the desk with a JSON-RPC error: assert `error.code == -32602` and `error.data.reason == "tool_not_permitted_for_role"` on the wire, and the rows `[mcp.tool_refused]`. The desk records nothing. The cases:
   - lead, qa, documentation, operations, product_signal and assistant calling `aicd_plan_submit`;
   - qa, documentation, operations, product_signal and assistant calling `aicd_escalate`.
9. `ori_t_0188_no_agent_text_reaches_the_reply_or_the_calls_rows`. A marker in the plan, the paths, the question, the recommendation and the evidence. The reply and every `mcp.*` row hold none of them.
10. `ori_t_0188_the_handlers_fallbacks_never_panic`. Call each handler's `ToolHandler::call` directly, as S3 does. Each gives `Port(PortError::Invalid { field, why })` with its constant field, and `why` equal to its named `WHY_*` constant:
    - `call.ticket_id` `None`;
    - a non-string plan;
    - a `declared_scope` that is not an array, or has a non-string entry;
    - an unknown trigger;
    - an `evidence` that is not an array of objects, or is empty.
11. `ori_t_0188_text_over_the_schemas_bound_fails_closed`. Called directly, past S2's schema, the desk records no call, and each gives `Port(PortError::Unavailable { what })` with its fixed `what`:
    - a plan of 65 537 characters;
    - a question of 8 193;
    - a recommendation of 8 193;
    - an evidence `what` of 2 049;
    - an evidence `found_at` of 1 025.

    Each `what` equals its named `WHAT_*` constant. Through `World::call`, every cap is pinned at the schema's bound:
    - a plan of exactly 65 536 characters is stored;
    - an escalation with a question and a recommendation of exactly 8 192 characters, a `what` of 2 048 and a `found_at` of 1 024 reaches the desk with each text whole.

## Mutation proofs

Run each in a `git archive` copy under your worktree's `target/`, with its own `CARGO_TARGET_DIR`. Delete the copy when done.

| # | Mutation | Must fail |
|---|---|---|
| m1 | sanitize `plan.replace(['\u{0}', '\u{7}'], "")` instead of the plan | test 1 (U+FFFD expected) |
| m2 | drop the `"` check | test 4 |
| m3 | delete the `Denied` arm's code (map it to `Self::bare("invalid")`) | test 3 |
| m4 | accept a blank question | test 5 |
| m5 | append the question to the reply | test 9 |
| m6 | pass `ESCALATION_TRIGGERS[0]` for every trigger | test 6 |
| m7 | ignore `truncated` (store what `sanitize_text` returned) | test 11 |
| m8 | trim the plan before sanitizing it | test 1 |
| m9 | sanitize the question at 8 191 | test 11 |

## Checks

- The standing gate list, the Windows compile check included.
- `cargo test -p ori-mcp --locked`, run twice.
- `cargo test -p ori-mcp --doc --locked`.
- Gate 13 against `<BASE>`.

Report every exit status as a number. List what is not tested and why: the desks' real implementations and contracts (ORI-T-0070), including the payload order of `escalation.raised`, and the suspension's effect on the next call (S2's `SessionDesk` fixture always answers active). Confirm that no existing test changed, and say whether adding `Denied` required changing any `match` other than the ones named here.
