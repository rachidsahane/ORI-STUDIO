# SECURITY_NOTES: Ori Studio

Who may do what, where input is validated, how failure is handled. Names the tier 2 modules. Applies the methodology's threat model (AICD §27) to this product.

## Trust boundaries

1. **Operator ↔ engine.** Local clients authenticate to the engine with a per-installation token stored in the keychain; remote (phase 4) with a token the operator issues. Every RPC call is an event with a human actor.
2. **Engine ↔ agent sessions.** Sessions are separate processes in worktrees and containers. They talk to the engine only through the MCP server and ACP. They receive credentials at spawn, scoped and expiring; they never receive the operator's tokens.
3. **Engine ↔ integrations and providers.** Outbound only, to user-configured endpoints, with credentials from the broker per call. Inbound webhooks are verified by signature and treated as untrusted data.
4. **Engine ↔ repository.** The engine reads the whole tree; humans write `spec/` only; agents write their worktree only, which holds for Ori's own tools: a proxied tool the operator exposed may reach further (API_SPEC section 3, "Proxied tools"); the merge queue alone merges, on Ori's own merge path: it alone calls `VcsHost::merge`, with Ori's credentials, and an operator-exposed server acting with its own credentials is outside that guarantee.

### Boundary 2 in phase 1: binding an MCP connection to a session

A session's MCP identity is the session that was handed the connection's endpoint. The engine opens a local endpoint for one session alone and delivers it to that session, so the connection is bound to the session by construction and no field of any message can change the binding. There is no secret behind the endpoint: delivering it is the binding. The residual risk is this: in worktree-only mode, a hostile process running as the same user that learns another session's endpoint could speak as that session. The operator accepted that risk for phase 1 on 2026-10-11, on six conditions:

1. The endpoint sits in a per-session directory of mode 0700.
2. No secret is in its path.
3. Tier 2 tickets refuse to start in worktree-only mode. This is the rule of "Failure handling" below ("Container runtime unavailable"; ORI-P1-032), not a new one. The condition is that the rule is enforced where a session is launched, and until it is, this condition is a dependency and not a fact.
4. Container mode mounts only that session's endpoint (phase 2).
5. Once the engine has a cryptographic random source, which it needs for identifiers anyway, the endpoint's name becomes unguessable.
6. A later `ori-memory` ticket adds a role lookup to the scope enforcer, so that tools other than the memory tools do not rest on the engine's role alone.

## Auto mode and where control lives

Agents are launched in their runtime's non-interactive mode and never see a permission prompt. Control does not disappear; it moves from dialogs to structure:

| Instead of a prompt asking... | The control is |
|---|---|
| "may this agent run this command?" | The container and worktree: the command runs, and can only affect what the identity can reach (for Ori's own tools; a proxied tool's server is not bound by them) |
| "may this agent access this file?" | Mounts and the memory scope enforcer (for Ori's own tools; not a proxied tool's server) |
| "may this agent use this credential?" | The broker issued only the credentials the role needs, expiring with the session |
| "may this agent merge, push or deploy?" | It cannot: it never held that credential; the merge queue alone merges, on Ori's own merge path, where it alone calls `VcsHost::merge` with Ori's credentials. An operator-exposed server acting with its own credentials is outside this row (API_SPEC section 3, "Proxied tools") |
| "is this output correct?" | Gates proven on planted defects, the lead's adversarial review, tiers, the operator's decision at the control points |

The container, worktree, mounts and memory scope bound what Ori's own tools can affect, and the broker issues Ori's own credentials only; none of these bounds a proxied tool's server. Its reach is the operator's decision, taken when the operator acknowledged it on exposing the server and recorded in `mcp.server_added` (API_SPEC section 3, "Proxied tools"). That acknowledgement is not one of the controls in the table, and the forbidden-action test does not exercise a proxied tool's reach (ENV_SETUP section 6). Where the operator has acknowledged a server whose tools can merge, push or return a secret, the instruction that an agent never merges and never handles a credential (`CLAUDE.md`, absolute rules 2 and 4) is the only barrier left against an agent calling such a tool. That is the reach the acknowledgement accepts, and it is why "No permission is expressed only in an instruction file" ("Authorization model" below) holds for Ori's own tools and not for an acknowledged proxied one.

Ori Studio itself holds full rights on the project's repository (through the app-scoped host identity) and on the local machine; it is the trusted component, and everything it delegates is scoped.

## Configuration scopes and trust

Application-level secrets (host connection, provider keys) are held once in the OS keychain and used by every project's engine through the broker; project-level overrides are separate keychain entries. MCP server credentials are project-level and exposed only to that project's engine. No project can read another project's overrides or MCP sessions.

## Authorization model

Permissions are a function of (actor, role, resource, action) evaluated in `ori-core` and enforced by the component that owns the resource (broker for credentials, orchestrator for tickets and merges, memory for retrieval, watch for the tree). No permission is expressed only in an instruction file. The permission matrix of AICD §17 is the source; `ENV_SETUP.md` section 5 records the manifest per identity.

## Input validation

- All RPC parameters validated against the API schema before dispatch.
- All MCP tool inputs validated and length-capped; free text from agents passes the sanitization barrier before storage.
- Agent free text that `ori-mcp` stores through a port (a plan's content; an escalation's question, recommendation and evidence) reaches the port as the barrier's sealed sanitized type, which only `barrier.rs` produces, so the compiler checks passage through the barrier at that port. Report content goes through `barrier::submit_report`. Identifiers, triggers, declared-scope paths and specification content (`aicd_spec_propose`, which goes to a human pull request) are validated and length-capped, not sanitized. The ports do not constrain what `ori-mcp` appends to its own `mcp.*` events: that rule is API_SPEC section 3's, which keeps agent free text out of them.
- All integration payloads (webhooks, polled data) parsed into typed structures; unparseable fields are dropped and counted, never stored raw except as evidence blobs.
- Paths from any caller are canonicalized and checked against the product root; no path escapes.
- Terminal input is passed to the PTY unmodified but logged; the terminal is a human surface, not an agent one.

## Secrets

- Only the broker touches the keychain. Secrets are never logged, never serialized into events, never returned by any RPC.
- Credential issuance is recorded without the secret. Expiry is enforced by the issuing system where possible (installation tokens) and by revocation on session end otherwise.
- The forbidden-action test is part of `flows.launch`, of every permission change, and of the release pipeline.
- A secret appearing in any log, event, ticket or report is an incident: rotate first, investigate second.

## Injection

- Everything from integrations, from the web, from dependencies and from agent free text is data. The context package marks provenance and `untrusted` on every item; the UI renders untrusted content quoted and labeled.
- The sanitization barrier is code with tests that include planted injection strings; it is a tier 2 module.

## Tier 2 modules (changes require two approvals, or the single-operator profile substitutes)

| Module | Why |
|---|---|
| `ori-core` (state machines, the permission function) | Control structure and the authorization decision itself |
| `ori-core::adapters` (`adapters.rs`, `adapters/identity.rs`) | Identities and how they are created, the scoped credential type (`ScopedToken`) an adapter receives per call, and the adapter traits, among them the contract of `VcsHost::merge` |
| `ori-broker` (all) | Credentials and identities |
| `ori-orchestrator::merge_queue` | Ori's own merge path: the only caller of `VcsHost::merge` |
| `ori-orchestrator::lifecycle` (tier and category rules) | Control structure |
| `ori-mcp::scopes`, `ori-mcp::ports`, `ori-mcp::server`, `ori-mcp::tools` | What agents can do |
| `ori-memory::barrier`, `ori-memory::scope` | Injection and scope boundaries |
| `ori-runtime::container`, `ori-runtime::injector` | Isolation and credential injection |
| `ori-watch::attribution` | Detection of unattributed changes |
| `ori-gates::prover`, `ori-gates::liveness` | Gate integrity |
| `ori-store::event_log` | The audit trail and hash chain |
| `ori-rpc::auth`, transports | Client authentication |
| `ori-integrations::*::merge`, webhook verification | Merge exposure and inbound trust |
| Release pipeline and signing | Supply chain of the product itself |
| Schema migrations | Data integrity |

## Failure handling

- Engine crash: the event log is durable; on restart, sessions found running are killed, their credentials revoked, their locks released, and their tickets returned to `Queued` with a `session.recovered` event.
- Container runtime unavailable: coders run worktree-only with a visible downgrade banner and an event; tier 2 tickets refuse to start in that mode.
- Integration unavailable: the affected slot reports degraded; nothing in the engine blocks except merges that require a CI status from that slot.
- Provider unavailable: sessions end as `blocked` with the reason; nothing is retried silently.
