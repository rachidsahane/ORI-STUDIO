//! Model Context Protocol host and server: AICD §8, AICD §25.
//!
//! Owns `Host` (client to the user's servers), `Server` (the tools in
//! `spec/API_SPEC.md` section 3), `ToolScopes` and the ports
//! (`spec/LLD.md` section 2).
//!
//! The methodology names the shape, not the protocol. AICD §8 puts a memory
//! service on top of the four layers and exposes it to the agents as a tool
//! connection, and it makes enforcing scopes the service's own job rather than
//! something an agent is asked to respect. AICD §25 fixes the components behind
//! that connection: the scope enforcer that maps an agent identity to a role and
//! its readable sources at query time, refusing and logging anything outside it,
//! and the retrieval API that answers a task-oriented question with a bounded
//! context package carrying provenance. `ToolScopes` is that enforcer at this
//! crate's edge, and the memory tools `Server` exposes are that retrieval API
//! reached through the connection.
//!
//! MCP itself is not in the methodology and does not need to be. AICD §5 makes
//! how the memory service is implemented an extension point, and ADR-0001 chose
//! MCP as the protocol that realizes the tool connection AICD §8 requires. The
//! methodology binds the shape and the scopes; the ADR binds the wire format.
//!
//! Must not (`spec/LLD.md` section 2): expose an Ori tool beyond the eleven of
//! `spec/API_SPEC.md` section 3, or expose a proxied tool except on the
//! conditions of that section's "Proxied tools".

/// The host toward the operator's MCP servers: the registry of the servers the
/// operator added to the product, the events that record them, and the rules
/// an addition must satisfy (AICD §17, AICD §26).
pub mod host;

/// What the server reaches through the engine: the traits `ori-engine`
/// implements for identifiers, ticket upgrades, specification proposals, the
/// operator's MCP servers, the time, a session's state, plans and escalations,
/// and the error they share (AICD §17). Tier 2.
pub mod ports;

/// The MCP wire both halves of this crate share: JSON-RPC 2.0 framing and
/// messages, the phase 1 MCP messages, version negotiation and the connection
/// state machine, on MCP 2025-11-25 (AICD §8). It holds no scope, tool or
/// policy.
pub mod protocol;

/// Who may call which of Ori's eleven tools, and the sealed grant that is the
/// only way to an operator's server: `ToolScopes`, the refusals, the proxied
/// tool name rule (AICD §17, AICD §25, AICD §26, AICD §27). Tier 2.
pub mod scopes;

/// The transport-agnostic core of the agent-facing MCP server: the connection
/// lifecycle, the per-call pipeline that decides, records and dispatches every
/// `tools/call`, and the session binding (AICD §8, AICD §17). Tier 2.
pub mod server;

/// The tool catalogue with its input schemas, the validator generated from
/// them, and the handler seam each tool plugs into (AICD §17, AICD §27). Tier 2.
pub mod tools;
