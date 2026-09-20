# API_SPEC: Ori Studio

Four interfaces. All are versioned; changing any is a decisional ticket.

1. **Client API**: JSON-RPC 2.0 between clients (desktop UI, CLI, remote) and the engine.
2. **Event stream**: subscriptions over the same transport.
3. **Agent-facing MCP server**: what agents may call, under scopes.
4. **Adapter traits**: the internal contracts for integration slots and agent runtimes.

Naming: `domain.verb`. Every request carries `product_id` unless global. Every mutating method returns the `Event.seq` it produced. Errors use JSON-RPC error codes plus a `reason` string that maps to a methodology section when a control refused the action (PRD F-43).

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
- `project.mcp.add({product_id, name, transport, config, secret?, exposed_to_roles})`, `project.mcp.list({product_id})`, `project.mcp.remove({product_id, id})`, `project.mcp.test({product_id, id})`
- `project.integrations.set({product_id, slot, adapter, config, secret?})`, `project.integrations.health({product_id})`
- `project.seats.set({product_id, seat, holder})`, `project.classification.set({product_id, rules})`

### flows
- `flows.framing.start({product_id})`, `flows.framing.confirm({product_id, framing_record})`
- `flows.documents.generate({product_id, set: "brief" | "foundation" | "phase", phase_id?})` → list of Document ids in `Draft`
- `flows.documents.requestChanges({document_id, instructions})` → new `Draft`
- `flows.documents.signOff({document_id, seat})`
- `flows.readiness({product_id})` → `{ready: bool, missing: [{item, reason, link}]}`
- `flows.launch({product_id})` → refused with `missing` unless readiness is true
- `flows.migration.start({product_id})`, `flows.migration.phase.complete({product_id, phase_key})` → refused unless exit criteria met, with the list
- `flows.phase.start({phase_id})`, `flows.phase.close({phase_id})`

### tickets
- `tickets.create({product_id, title, category?, spec_anchor, declared_scope?, budget?, criteria?})`
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

`events.subscribe({product_id?, kinds?})` streams `Event` rows as they are appended. Clients render from events; there is no polling API for state that events carry. Kinds include: `ticket.*`, `escalation.opened`, `escalation.answered`, `report.*`, `session.*`, `gate.run`, `gate.inert`, `pr.*`, `change.unattributed`, `document.*`, `phase.*`, `notification.*`, `card.*` (confirmation and report cards addressed to the chat), `terminal.io`.

Cards: `card.confirmation` carries `{card_id, kind, question, recommendation, evidence_refs, actions: [{id, label, rpc}]}`; answering a card is calling the referenced RPC, which produces `card.answered`.

## 3. Agent-facing MCP server

Exposed to each agent session under its identity's scopes. Every call is an `Event` with the identity as actor. Tools:

| Tool | Roles | Effect |
|---|---|---|
| `aicd_context(ticket_id)` | all | The bounded context package (methodology 25) |
| `aicd_search(query)` | all | Scoped search over allowed layers |
| `aicd_evidence(record_id)` | lead, qa, operations | Raw evidence, returned labeled untrusted, logged |
| `aicd_plan_submit(ticket_id, plan, declared_scope)` | coder | Stores the plan; lock check; may return `E_SCOPE_LOCKED` |
| `aicd_report(ticket_id, kind, content)` | coder, qa, operations, documentation | Closing, blocked, run and audit reports through the sanitization barrier |
| `aicd_escalate(ticket_id, trigger, question, recommendation)` | lead, coder | Opens an escalation; suspends the session if configured |
| `aicd_ticket_create(title, category, spec_anchor, evidence)` | qa, operations, product_signal | Files a ticket; category proposals only |
| `aicd_ticket_upgrade(ticket_id, category)` | lead | Upgrade only |
| `aicd_criterion_propose(...)` | qa | Enters `proposed` state only |
| `aicd_spec_propose(document_id, content)` | documentation | Opens a specification PR |
| `aicd_gate_result(gate_id, pr_id, status, output)` | qa (test env), system | Records a run |

Agents never receive file write tools to `spec/` or `ops/`, never receive the merge operation, and never receive credentials through MCP.

## 4. Adapter traits (Rust)

Sketches; exact signatures live in LLD and code. Every adapter is a crate implementing one trait and registered by name.

```rust
trait VcsHost { fn identity_token(&self, identity: &AgentIdentity) -> ScopedToken; fn create_branch(...); fn open_pr(...); fn pr_status(...); fn merge(...) /* callable by merge queue only */; fn set_check(...); fn webhooks(&self) -> WebhookSpec; }
trait ErrorTracking { fn recent_issues(...); fn issue_events(...); }
trait Analytics { fn segments(...); fn events(...); }
trait Notification { fn send(&self, route: Route, payload: Payload); }
trait Ci { fn trigger(...); fn status(...); fn artifacts(...); }
trait AgentRuntime { fn spawn(&self, session: SessionSpec) -> SessionHandle; fn send(...); fn recv(...); fn kill(...); fn capabilities(&self) -> RuntimeCaps; }
trait ModelProvider { fn complete(...); fn embed(...); }
```

Rules: adapters receive credentials from the broker per call or per session, never from configuration; adapters have no access to the store; the `merge` method exists only on the `VcsHost` trait object held by the merge queue.

## 5. Versioning and compatibility

The Client API carries `api_version` in the handshake; the engine serves the current and previous minor. MCP tool names are stable; new tools are additive. Adapter traits are versioned by crate; a breaking change is a decisional ticket.
