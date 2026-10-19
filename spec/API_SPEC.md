# API_SPEC: Ori Studio

Four interfaces. All are versioned; changing any is a decisional ticket.

1. **Client API**: JSON-RPC 2.0 between clients (desktop UI, CLI, remote) and the engine.
2. **Event stream**: subscriptions over the same transport.
3. **Agent-facing MCP server**: what agents may call, under scopes.
4. **Adapter traits**: the internal contracts for integration slots and agent runtimes.

Naming: `domain.verb`. Every request carries `product_id` unless global. Every mutating method returns the `Event.seq` it produced. Errors use JSON-RPC error codes plus a `reason` string that maps to a methodology section when a control refused the action (PRD A-10).

## 1. Client API (JSON-RPC 2.0)

### app (application level, shared by all windows)
- `app.config.get()`, `app.config.set({host_connection?, default_models?, notification_defaults?, container_runtime?, org_repo_path?})`
- `app.providers.list()`, `app.providers.set({provider, secret, roles?})` (secret to keychain), `app.providers.remove({provider})`
- `app.windows.list()`, `app.windows.open({product_id})` (focuses the existing window if the project is already open)

### products
- `products.list()`, `products.create({name, repo_path, remote?, seats})`, `products.import({repo_path, remote?})` → origin detection result and migration plan
- `products.open({id})`, `products.close({id})`, `products.rebuild({id})` (rebuild projections from event log and repository)

### project configuration
- `project.providers.override({product_id, provider, secret, roles?})`, `project.providers.clearOverride({product_id, provider})`
- `project.mcp.add({product_id, name, transport, config, secret?, exposed_to_roles, acknowledged_reach?})` (`name` matches `[a-z][a-z0-9-]{0,31}` and does not begin with `aicd`; `transport` is `stdio` only in phase 1; a server added without `acknowledged_reach: true` is exposed to no role; all three as under section 3, "Proxied tools"), `project.mcp.list({product_id})`, `project.mcp.remove({product_id, id})`, `project.mcp.test({product_id, id})`
- `project.integrations.set({product_id, slot, adapter, config, secret?})`, `project.integrations.health({product_id})`
- `project.seats.set({product_id, seat, holder})`, `project.classification.set({product_id, rules})`

### flows
- `flows.framing.start({product_id})`, `flows.framing.confirm({product_id, framing_record})`
- `flows.documents.generate({product_id, set: "brief" | "foundation" | "phase", phase_id?})` → list of Document ids in `Draft`
- `flows.documents.requestChanges({document_id, instructions})` → new `Draft`
- `flows.documents.approve({document_id, seat})`
- `flows.readiness({product_id})` → `{ready: bool, missing: [{item, reason, link}]}`
- `flows.launch({product_id})` → refused with `missing` unless readiness is true
- `flows.migration.start({product_id})`, `flows.migration.phase.complete({product_id, phase_key})` → refused unless exit criteria met, with the list
- `flows.phase.start({phase_id})`, `flows.phase.close({phase_id})`

### tickets
- `tickets.create({product_id, title, category?, kind?, spec_anchor, declared_scope?, budget?, criteria?})`
- `tickets.list({product_id, view: "review" | "coder" | "blocked" | "decisional" | "signals" | "all", filter?})`
- `tickets.get({id})`, `tickets.validate({id})`, `tickets.reject({id, reason})`
- `tickets.setCategory({id, category})` (downgrade requires human actor; agents receive `E_UPGRADE_ONLY`)
- `tickets.close({id})` (refused without spec update and, for defects, accepted criteria)

### escalations
- `escalations.list({product_id, state?})`, `escalations.answer({id, answer, decision})`

### review
- `review.queue({product_id})` → PRs with tier, evidence summary, lead result
- `review.evidence({pr_id})` → coverage matrix, gate runs, mutation delta, adversarial checklist result, modified tests
- `review.approve({pr_id})`, `review.requestChanges({pr_id, comments})`
- `review.diff({pr_id})` → file list and hunks for the diff viewer

### fleet
- `fleet.state({product_id})` → identities with state, ticket, budget used, worktree, container, model
- `fleet.session.transcript({session_id, offset?})`, `fleet.session.kill({session_id, reason})`
- `fleet.run({product_id, role, trigger})` (manual dispatch of an unattended agent)

### spec
- `spec.documents({product_id})` → documents with state, owner seat, verification date
- `spec.read({document_id})`, `spec.write({document_id, content})` (human edit; opens a specification PR; runs citation check; refused outside `spec/`)
- `spec.citations.check({product_id})`

### files
- `files.tree({product_id, path?})`, `files.read({product_id, path})` (read-only outside spec/)
- `files.changes({product_id})` → unattributed changes with state
- `files.changes.resolve({change_id, resolution: "exception" | "revert", ticket_id?})`

### memory
- `memory.search({product_id, query, layers?})` (operator scope)
- `memory.context({product_id, ticket_id})` → the package an agent would receive, for inspection
- `memory.reindex({product_id})`, `memory.drift.run({product_id})`

### broker
- `broker.identities({product_id})`, `broker.identity.create({product_id, role, model, runtime})`
- `broker.forbiddenActionTest({product_id})` → per identity: action attempted, refused (bool), evidence
- `broker.credentials.set({slot | provider, secret})` (secret goes to keychain; never returned)

### gates
- `gates.list({product_id})`, `gates.prove({gate_id})` → GateProof or failure, `gates.runs({pr_id})`

### dashboard, map, calibration, notify, terminal
- `dashboard.product({product_id})`, `dashboard.portfolio()`
- `map.evolution({product_id})` → phases with progress, criteria covered, tickets closed, versions, significant modifications
- `calibration.records({product_id})`, `calibration.run({product_id, model})`
- `notify.rules({product_id})`, `notify.rules.set({...})`, `notify.ack({id})`
- `terminal.open({product_id})` → session id; I/O over the event stream; every command logged
- `chat.send({product_id, message})`, `chat.history({product_id, offset?})`; cards are events (below)

## 2. Event stream

`events.subscribe({product_id?, kinds?})` streams `Event` rows as they are appended. Clients render from events; there is no polling API for state that events carry. Kinds include: `ticket.*`, `escalation.opened`, `escalation.answered`, `report.*`, `session.*`, `mcp.server_added`, `mcp.server_removed`, `mcp.tool_called`, `mcp.tool_refused`, `mcp.tool_failed`, `gate.run`, `gate.inert`, `gate.proven`, `pr.*`, `change.unattributed`, `document.*`, `phase.*`, `notification.*`, `card.*` (confirmation and report cards addressed to the chat), `terminal.io`.

Cards: `card.confirmation` carries `{card_id, kind, question, recommendation, evidence_refs, actions: [{id, label, rpc}]}`; answering a card is calling the referenced RPC, which produces `card.answered`.

## 3. Agent-facing MCP server

Exposed to each agent session under its identity's scopes. Every call is recorded as `Event`s with the identity as actor (section 2 lists the kinds). Each call is either allowed or refused, and records exactly one of `mcp.tool_called` (allowed; appended before the effect) and `mcp.tool_refused` (refused; nothing takes effect). `mcp.tool_failed` follows `mcp.tool_called` when the allowed effect fails. A call whose `mcp.tool_called` cannot be appended is refused, so no effect happens without its record. The `mcp.*` events carry identifiers and lengths, never agent free text: the content of a call is recorded through the sanitization barrier (SECURITY_NOTES, "Input validation"), not in the event. Tools:

| Tool | Roles | Effect | Phase 1 |
|---|---|---|---|
| `aicd_context(ticket_id)` | all | The bounded context package (methodology 25) | Yes |
| `aicd_search(query)` | all | Scoped search over allowed layers | Yes |
| `aicd_evidence(record_id)` | lead, qa, operations | Raw evidence, returned labeled untrusted, logged | Yes |
| `aicd_plan_submit(ticket_id, plan, declared_scope)` | coder | Stores the plan; lock check; may return `E_SCOPE_LOCKED` | Yes |
| `aicd_report(ticket_id, kind, content)` | coder, qa, operations, documentation | Closing, blocked, QA run and drift audit reports through the sanitization barrier; `kind` and `content` as under "Report kinds" below | Yes |
| `aicd_escalate(ticket_id, trigger, question, recommendation, evidence)` | lead, coder | Opens an escalation; suspends the session if configured; `evidence` as under "Escalation evidence" below | Yes |
| `aicd_ticket_create(title, category, kind, spec_anchor, evidence)` | qa, operations, product_signal | Files a ticket; category and kind proposals only | No |
| `aicd_ticket_upgrade(ticket_id, category)` | lead | Upgrade only | Yes |
| `aicd_criterion_propose(...)` | qa | Enters `proposed` state only | No |
| `aicd_spec_propose(document_id, content)` | documentation | Opens a specification PR | Yes |
| `aicd_gate_result(gate_id, pr_id, status, output)` | qa (test env), system | Records a run | No |

Agents never receive file write tools to `spec/` or `ops/`, never receive the merge operation, and never receive credentials through MCP. These three promises bind Ori's own tools and Ori's own credentials; "Proxied tools" below says what they do not bind.

### Phase 1

Phase 1 implements eight of the eleven tools. The three marked No are not built in phase 1, and no row is removed, because tool names are stable (section 5). `aicd_ticket_create` is held only by roles that run unattended, which arrive in phase 4 (PRD F-10). `aicd_criterion_propose` is qa's, and proposed criteria arrive in phase 2 (PRD Q-07). `aicd_gate_result` is not built because one of its two roles, `system`, is an actor and not a role; gate runs are written by `ori-gates` and the CI slot.

### Escalation evidence

`aicd_escalate` carries `evidence`, a list of 1 to 16 entries, each `{what, found_at}`: what was found, and where it was found. An escalation with no entry is refused. `ori-orchestrator`'s `Escalation` raises a trigger only when a finding stands behind it, and `question` and `recommendation` are the agent's own words, which are not a finding, so the server builds none from them. An entry follows the Context field of `templates/escalation.md`: what was found, where, with the evidence a human needs to judge it.

### Report kinds

`aicd_report` takes `kind`, one of `closing`, `blocked`, `qa_run` and `drift_audit`, and `content`, an object of named string fields. `incident` and `post_mortem`, which `Report.kind` in DATA_MODEL section 2 also lists, are not kinds of this tool. Each role may write only the kinds in this table:

| Role | Kinds it may write |
|---|---|
| coder | `closing`, `blocked` |
| documentation | `closing`, `drift_audit`, `blocked` |
| qa | `qa_run`, `blocked` |
| operations | `blocked`, until phase 4 |

Each kind is stored as one operational-memory record kind (`MemoryRecord.kind`, DATA_MODEL section 2):

| Kind | Stored as | Fields of `content` |
|---|---|---|
| `closing` | `closing_report` | `modules_touched`, `criteria_covered`, `outcome`, `approaches_tried_and_rejected`, `deviations_from_the_plan`, `escalation_decisions`, `specification_update`, `new_criteria`, `what_the_next_agent_should_know`, `provenance` |
| `blocked` | `blocked_report` | `budget_consumed`, `approaches_tried`, `hypothesis`, `what_it_would_try_next`, `what_it_needs_from_a_human` |
| `qa_run` | `finding` | Not yet defined |
| `drift_audit` | `finding` | Not yet defined |

The names of `closing` and `blocked` are the field names of `templates/closing-report.md` and `templates/blocked-report.md`, lower case with underscores. Each template's Fields table says which fields are required and where `none` is a valid value; a field the template draws as a table is one string here. The template's `Agent identity`, `Ticket`, `Product` and `Timestamp` (for the blocked report, `Ticket`) are not fields of `content`. Every operational-memory record carries the agent identity, the ticket, the product and the timestamp (AICD §25), and the server sets them from the session and from `ticket_id`, so an agent does not assert them. Every field of `content` passes the sanitization barrier before storage (SECURITY_NOTES, "Input validation").

The field names of `qa_run` and `drift_audit` are not defined anywhere yet. No file in `templates/` is a form for either, and AICD appendix A has no entry for either, so they are to be defined by a template; changing `templates/` follows the rule in `templates/README.md`. Until a template defines them, the server has no list of names to hold the `content` of these two kinds to.

### Proxied tools

The eleven tools above are Ori's own, and the table is the whole of them. The operator may also expose tools of the operator's own MCP servers to roles, and the server proxies them to the session. The conditions of this section bound this:

1. Only servers the operator added to the project (`project.mcp.add`) are proxied, each only to the roles in its `exposed_to_roles`, and only if the operator acknowledged the reach described below (condition 5). A call from a role that is not listed, or from a session of another project, is refused (ORI-P1-038).
2. A proxied tool is named `<server>__<tool>`: `<server>` is the `name` given when the server was added, and `<tool>` is the name the server gives the tool. A server's `name` matches `[a-z][a-z0-9-]{0,31}` and does not begin with `aicd`, and `project.mcp.add` accepts no other. A name has no underscore, so the first `__` in a proxied tool's name always splits the server from the tool, whatever underscores the tool's own name holds, and a proxied name has one reading. A name does not begin with `aicd`, so a proxied tool's name never begins as the name of one of Ori's own tools does.
3. A proxied tool returns text blocks only, and every block is untrusted (SECURITY_NOTES, "Injection"). The `<tool>` part of its name and its description come from the operator's server, and are untrusted data too, like the blocks it returns.
4. A proxied tool never carries a credential of Ori's. A server's own secret (`MCP_<SERVER>_*`, ENV_SETUP section 4) stays with the engine, which connects to the server itself, and an agent is never given a server's configuration.
5. Exposing a server to a role requires the operator's explicit acknowledgement of the reach described below, recorded as `acknowledged_reach`, true, in the server's `mcp.server_added` event. A server added without it is exposed to no role.

The three promises under the tool table above (no file write tools to `spec/` or `ops/`, never the merge operation, never credentials through MCP) bind Ori's own tools and Ori's own credentials. They cannot bind a proxied tool, because Ori cannot tell what a tool of the operator's server does, and what a server says about its own tools (that one is read-only, say) is a hint from an untrusted party that Ori does not rely on. An operator's server may hold its own credentials, and tools that act with them outside Ori's merge queue, and Ori cannot tell which tools those are. A proxied tool's reach is therefore the operator's decision. Exposing a server to a role requires the operator's explicit acknowledgement of exactly that: that the server may hold its own credentials and tools that act with them outside Ori's merge queue, and that Ori cannot tell which tools those are. The acknowledgement is given as `acknowledged_reach` in `project.mcp.add` and recorded in the `mcp.server_added` event, which carries the server's name, transport, exposed roles and `acknowledged_reach`. ENV_SETUP section 5 states what that means for the permission manifest, and ENV_SETUP section 6 what it means for the forbidden-action test.

In phase 1 the host connects to stdio servers only, and `project.mcp.add` accepts `stdio` as its `transport` and no other. Remote (HTTP) servers are a later decision (ROADMAP, "Later (not scheduled)").

## 4. Adapter traits (Rust)

Sketches; exact signatures live in LLD and code. Every adapter is a crate implementing one trait and registered by name.

All seven traits are declared in `ori-core`, together with the types in their signatures: a trait declared there can name only types `ori-core` declares, since it imports no other workspace crate. The declaration is a contract and performs no IO (`spec/LLD.md` section 2). The traits are implemented outside `ori-core`: the five slot traits (`VcsHost`, `ErrorTracking`, `Analytics`, `Notification`, `Ci`) in `ori-integrations`, which keeps the reference adapters and the webhook receiver, and `AgentRuntime` in `ori-runtime`. A crate that holds an adapter as a trait object names the trait through `ori-core`, so `ori-orchestrator` names `dyn VcsHost` without depending on `ori-integrations`.

```rust
trait VcsHost { fn identity_token(&self, identity: &AgentIdentity) -> ScopedToken; fn create_branch(...); fn open_pr(...); fn pr_status(...); fn merge(...) /* callable by merge queue only */; fn set_check(...); fn webhooks(&self) -> WebhookSpec; }
trait ErrorTracking { fn recent_issues(...); fn issue_events(...); }
trait Analytics { fn segments(...); fn events(...); }
trait Notification { fn send(&self, route: Route, payload: Payload); }
trait Ci { fn trigger(...); fn status(...); fn artifacts(...); }
trait AgentRuntime { fn spawn(&self, session: SessionSpec) -> SessionHandle; fn send(...); fn recv(...); fn kill(...); fn capabilities(&self) -> RuntimeCaps; }
trait ModelProvider { fn complete(...); fn embed(...); }
```

Rules: adapters receive credentials from the broker per call or per session, never from configuration; adapters have no access to the store; the `merge` method exists only on the `VcsHost` trait object held by the merge queue. The rule concerns the trait object the merge queue holds, not where the trait is declared: `merge` is called from `ori-orchestrator::merge_queue` only, on the adapter `ori-engine` gives it, which is the call the carve-out in `spec/LLD.md` section 2 allows.

## 5. Versioning and compatibility

The Client API carries `api_version` in the handshake; the engine serves the current and previous minor. MCP tool names are stable; new tools are additive. Adapter traits are versioned by the crate that declares them, `ori-core`; a breaking change is a decisional ticket.

### API schema artifact

The Client API's machine-readable contract is a JSON Schema per method, generated by `ori-rpc` from its method registry. It is a build artifact, never hand-written and never checked in under `spec/`, so it cannot drift from the registry. Each schema carries the `api_version` of the handshake, so a client can tell which minor it describes.

| Property | Rule |
|---|---|
| Produced by | `ori-rpc`, from the method registry, at build time |
| Shape | One JSON Schema per Client API method, over its params and its result |
| Emitted to | The workspace build output, generated and never checked in; published with the release as an asset |
| Checked by | ORI-P1-034 (every `--json` CLI output validates against the schema for its method) and the contract tests of TESTING section 1 |
