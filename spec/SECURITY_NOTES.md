# SECURITY_NOTES: Ori Studio

Who may do what, where input is validated, how failure is handled. Names the tier 2 modules. Applies the methodology's threat model (AICD §27) to this product.

## Trust boundaries

1. **Operator ↔ engine.** Local clients authenticate to the engine with a per-installation token stored in the keychain; remote (phase 4) with a token the operator issues. Every RPC call is an event with a human actor.
2. **Engine ↔ agent sessions.** Sessions are separate processes in worktrees and containers. They talk to the engine only through the MCP server and ACP. They receive credentials at spawn, scoped and expiring; they never receive the operator's tokens.
3. **Engine ↔ integrations and providers.** Outbound only, to user-configured endpoints, with credentials from the broker per call. Inbound webhooks are verified by signature and treated as untrusted data.
4. **Engine ↔ repository.** The engine reads the whole tree; humans write `spec/` only; agents write their worktree only; the merge queue alone merges.

## Auto mode and where control lives

Agents are launched in their runtime's non-interactive mode and never see a permission prompt. Control does not disappear; it moves from dialogs to structure:

| Instead of a prompt asking... | The control is |
|---|---|
| "may this agent run this command?" | The container and worktree: the command runs, and can only affect what the identity can reach |
| "may this agent access this file?" | Mounts and the memory scope enforcer |
| "may this agent use this credential?" | The broker issued only the credentials the role needs, expiring with the session |
| "may this agent merge, push or deploy?" | It cannot: it never held that credential; the merge queue alone merges |
| "is this output correct?" | Gates proven on planted defects, the lead's adversarial review, tiers, the operator's decision at the control points |

Ori Studio itself holds full rights on the project's repository (through the app-scoped host identity) and on the local machine; it is the trusted component, and everything it delegates is scoped.

## Configuration scopes and trust

Application-level secrets (host connection, provider keys) are held once in the OS keychain and used by every project's engine through the broker; project-level overrides are separate keychain entries. MCP server credentials are project-level and exposed only to that project's engine. No project can read another project's overrides or MCP sessions.

## Authorization model

Permissions are a function of (actor, role, resource, action) evaluated in `ori-core` and enforced by the component that owns the resource (broker for credentials, orchestrator for tickets and merges, memory for retrieval, watch for the tree). No permission is expressed only in an instruction file. The permission matrix of AICD §17 is the source; `ENV_SETUP.md` section 5 records the manifest per identity.

## Input validation

- All RPC parameters validated against the API schema before dispatch.
- All MCP tool inputs validated and length-capped; free text from agents passes the sanitization barrier before storage.
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
| `ori-broker` (all) | Credentials and identities |
| `ori-orchestrator::merge_queue` | The only merge path |
| `ori-orchestrator::lifecycle` (tier and category rules) | Control structure |
| `ori-mcp::tool_scopes` (server tool scopes) | What agents can do |
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
