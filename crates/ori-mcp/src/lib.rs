//! Model Context Protocol host and server: AICD §8, AICD §25.
//!
//! Owns `Host` (client to the user's servers), `Server` (the tools in
//! `spec/API_SPEC.md` section 3) and `ToolScopes` (`spec/LLD.md` section 2).
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
//! Must not: expose a tool not in `spec/API_SPEC.md` (`spec/LLD.md` section 2).

/// What the server reaches through the engine: the traits `ori-engine`
/// implements for identifiers, ticket upgrades, specification proposals and the
/// operator's MCP servers, and the error they share (AICD §17). Tier 2.
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
