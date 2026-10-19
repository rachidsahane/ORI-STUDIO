# Batch 10 plan: `ori-mcp` (ORI-T-0056, the MCP host; ORI-T-0057, the MCP server and tool scopes)

The "before each batch" control point of `ops/phase-1-backlog.md`. A coder (Sonnet 5.5, [[R39]]) wrote the plan on 2026-10-11 by reading only, at f3da68e. The lead checked the load-bearing citations and records here the decisions it takes and the questions it puts to the operator.

## Decisions taken by the lead

| # | Decision |
|---|---|
| D1 | `Server` reaches the lock table, escalations, ticket and spec operations through **port traits declared in `ori-mcp`** (`src/ports.rs`). `ori-engine` (ORI-T-0070) implements them; `LLD` section 2 draws `ENG --> MCP`. No new crate edge, and nothing is added to `ori-core`: the ports are not the adapter traits of API_SPEC section 4. `ori-mcp` adds `ori-core` to its manifest, which `spec/LLD.md` section 2 allows; it never adds `ori-broker`. An `IdSource` port supplies identifiers, since `Id` never generates one |
| D2 | The wire is MCP as JSON-RPC 2.0, one value per line, with `serde`/`serde_json` only ([[E-0008]]); no MCP SDK crate. Phase 1 needs `initialize`, `notifications/initialized`, `ping`, `tools/list` and `tools/call`; everything else is answered "method not found", and batches are refused. The `Server` core is transport-agnostic and tested over in-memory channels. The real transport is the endpoint and bridge of PE1, subject to [[E-0013]] |
| D3 | `ToolScopes` is an exhaustive constant table in `scopes.rs` (tier 2), with no wildcard arm. Tests pin it to `spec/API_SPEC.md` section 3's roles column. The operator's exposure of user servers to roles is separate data (`McpServer.exposed_to_roles`). A user server is reached only through a sealed `UserToolGrant` that `scopes.rs` alone can create, so the host (tier 1) holds no policy |
| D4 | Every call is an event, with the identity as actor: `mcp.tool_called` before the effect, `mcp.tool_refused`, and `mcp.tool_failed`. A payload carries ids and lengths, never agent free text. A failed append refuses the call. The `mcp.*` kinds are added to `spec/API_SPEC.md` section 2 by a documentation ticket |
| D8 | Phase 1 implements 8 of the 11 tools. **Not in phase 1:** `aicd_ticket_create` (only unattended roles hold it, phase 4); `aicd_criterion_propose` (qa only, PRD Q-07, phase 2); `aicd_gate_result` (its roles name `system`, which is an actor, not a role; gate runs are written by `ori-gates` and the CI slot). No document defined "the phase 1 tool subset"; this is the lead's decision, recorded |
| D9 | The two backlog tickets become the slices below. The backlog row's "parallel: disjoint files" is wrong: both need `Cargo.toml`, `lib.rs` and the same wire types |

## Slices

Identifiers are allocated when each slice is claimed. "Queue" files (`Cargo.toml` dependency lines, `lib.rs` module lines, `Cargo.lock`) are declared by every slice that touches them.

| Slice | From | Declared scope | Tier | Order and preconditions |
|---|---|---|---|---|
| W0, the shared wire | new | `crates/ori-mcp/Cargo.toml`, `src/lib.rs`, `src/protocol.rs` | 1 | First. Needs the MCP specification text, supplied in the ticket |
| S1, scopes and ports | 0057 | `src/scopes.rs`, `src/ports.rs` | 2 | After W0 and the RISK_MAP rows below |
| H1, the host registry | 0056 | `src/host.rs` (registry, events, validation) | 1 | After S1; parallel with S2 |
| H2, the host client | 0056 | `src/host.rs` (client, `Channel`, `Launcher`) | 1 | After H1 |
| S2, the server core | 0057 | `src/server.rs`, `src/tools/mod.rs` | 2 | After S1 |
| S3, `aicd_context` | 0057 | `src/tools/memory.rs` | 2 | After S2 |
| S4, `aicd_report` | 0057 | `src/tools/report.rs` | 2 | After S2; kinds beyond closing and blocked wait on [[E-0012]] |
| S5, `aicd_ticket_upgrade`, `aicd_spec_propose` | 0057 | `src/tools/desk.rs` | 2 | After S2 |
| S6, the user-tool proxy and ORI-P1-038 | 0057 | `src/tools/proxy.rs`, `tests/ori_p1_038_mcp_exposure.rs` | 2 | After H2, S1, S2, and [[E-0010]] |
| S7, `aicd_evidence` | 0057 | `src/tools/evidence.rs` | 2 | After S2 and ORI-T-0144 |
| S8, `aicd_search` | 0057 | `src/tools/memory.rs` | 2 | After S3 and PM1 |
| S9, `aicd_plan_submit`, `aicd_escalate` | 0057 | `src/tools/orchestration.rs` | 2 | After S2, PM2, [[E-0011]] and [[E-0012]] |

## Prerequisites outside `ori-mcp`

| Id | Work | Where | Tier |
|---|---|---|---|
| PD-1 | `spec/RISK_MAP.md` rows for the new files: `ports.rs`, `server.rs` and `tools/**` at 2 with `scopes.rs`; `protocol.rs` and `host.rs` at 1. Restated in gate 12's `RISK_MAP_TIER_TWO`, as ORI-T-0145 did. **Before S1** | `spec/RISK_MAP.md`, `crates/ori-gates/src/significance.rs` | 2 |
| PM1 | A search entry that turns a search `Authorization` into admitted results. `Retrieval::assemble` refuses non-context authorizations, and `Indexer::search` returns raw hits | `crates/ori-memory/src/retrieval.rs` | 1 |
| PM2 | A public barrier entry returning the sealed sanitized field ([[E-0012]]) | `crates/ori-memory/src/barrier.rs` | 2 |
| PR1 | An `ori-runtime` spawn path for the operator's stdio MCP servers, with environment injection, and a broker event for resolving an MCP secret. The secret never enters `ori-mcp` | `ori-runtime` (`injector.rs`), `ori-broker` | 2 |
| PE1 | The per-session endpoint, the relay binary, and `mcp_servers` threaded through `SessionSpec` and `acp.rs` ([[E-0013]]); after ORI-T-0141 and ORI-T-0149 | `ori-mcp`, `ori-core` adapters, `ori-runtime` | 2 |
| PW | `ori-engine` implements every port and registers session bindings; folded into ORI-T-0070 | `ori-engine` | 1 |
| PD-2 | Documentation: API_SPEC section 3 after [[E-0010]] to [[E-0012]]; the `ori-mcp` row of LLD ("ports"); `mcp.*` in API_SPEC section 2 | `spec/` | 1 |

## Questions put to the operator

[[E-0010]] (proxying the operator's servers' tools), [[E-0011]] (evidence on `aicd_escalate`), [[E-0012]] (agent text through the barrier; report kinds), [[E-0013]] (binding a connection by endpoint). And one scope question: **the host connects to stdio servers only in phase 1**, because the workspace has no HTTP or TLS client crate. That narrows PRD I-03's "connect any MCP server", and HTTP would be a `new_dependency` decision for later. A Windows named-pipe endpoint is the same crate question `ori-rpc` (ORI-T-0068) must answer, so it is deferred to batch 13 planning and asked once.

## Recorded, not blocking

- `spec/API_SPEC.md` section 3 lists `system` among the roles of `aicd_gate_result`, but `system` is an actor, not a role.
- `spec/SECURITY_NOTES.md` and ADR-0002 say `ori-mcp::tool_scopes`; LLD and the backlog say `scopes.rs`.
- `spec/CONVENTIONS.md` says async with Tokio, but no async runtime is in `Cargo.lock`. The server is synchronous.
- `spec/PRD.md` and AICD §17 want input and output in the audit trail, while SECURITY_NOTES forbids agent free text in events. The server follows the shipped precedent: ids and lengths only.
- `aicd_report` for a role with no ticket has no `ticket_id` to give.
- **Not verified by the planner, which had no web access:** the MCP specification's `protocolVersion` strings, and the exact ACP `mcpServers` entry shape. W0's ticket must supply both.
