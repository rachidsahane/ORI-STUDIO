//! The host: the half of `ori-mcp` that faces the operator's own MCP servers, as
//! [`crate::server`] faces the agents: AICD §17, AICD §26.
//!
//! `spec/LLD.md` section 2 gives `ori-mcp` two halves, a `Host`, the client to
//! the operator's servers, and a `Server`, the tools of `spec/API_SPEC.md`
//! section 3. This module is the first, built in slices. Only the registry
//! exists so far ([`crate::host::registry`], ORI-T-0194, slice H1): the record
//! of which servers the operator added to a product, the two events that record
//! it, and the rules an addition must satisfy. The client that connects to a
//! server and launches it is a later slice (H2), and nothing here does either.
//!
//! # Tier
//!
//! `spec/RISK_MAP.md` tiers `crates/ori-mcp (host)` at 1. The host holds no
//! policy: who may reach a server is decided in [`crate::scopes`], which is
//! tier 2, on facts this module supplies. A defect in the registry's fold would
//! widen what [`crate::scopes::ToolScopes::authorize_user_tool`] grants, so
//! ORI-T-0194 was raised to tier 2 for that reason, as the backlog lets an agent
//! raise a tier. This ticket changes neither that row of `spec/RISK_MAP.md` nor
//! gate 12's list of tier 2 paths.
//!
//! # Where the registry sits
//!
//! ```mermaid
//! flowchart LR
//!   op["operator: project.mcp.add"] --> eng["ori-engine (not yet built)"]
//!   eng -- "Registry::add" --> reg["Registry"]
//!   reg -- "appends mcp.server_added" --> log[("the product's event log")]
//!   log -- "folded by load and refresh" --> reg
//!   reg -- "Registry::exposed" --> s6["S6, the proxy in server.rs (not yet built)"]
//!   s6 -- "ExposedServer" --> door["ToolScopes::authorize_user_tool"]
//!   door -- "UserToolGrant" --> uth["UserToolHost"]
//! ```

/// The registry of the servers the operator added to a product: the types of an
/// addition, the two events that record it, the fold of the log into the
/// registry, and `add` and `remove` (AICD §17, AICD §26, AICD §27). Tier 2 by
/// the raise stated in [`crate::host`].
pub mod registry;
