//! `ToolScopes`: which role may call which of Ori's own tools, and the one door
//! to an operator's server: AICD §17, AICD §25, AICD §26, AICD §27.
//!
//! # What this module owes
//!
//! `spec/API_SPEC.md` section 3 lists the eleven tools Ori exposes to an agent
//! session, and the roles that hold each. AICD §17 makes the permission matrix
//! the source of what a role may do, and AICD §25 puts the enforcement at the
//! edge of the service rather than in the agent's good behaviour. This module
//! is that enforcement for the tool surface: [`crate::scopes::ToolScopes`] answers
//! "may this role call this tool" from one constant table, and a call outside
//! the table is refused with a [`crate::scopes::ToolRefusal`] that carries the
//! methodology section it was refused under (`CLAUDE.md` rule 9, criterion
//! ORI-P1-033).
//!
//! The same section lets the operator expose tools of the operator's own MCP
//! servers to roles ("Proxied tools" in `spec/API_SPEC.md` section 3). Whether
//! one role in one project may reach one such server is decided here too, and
//! the decision is a value: a [`crate::scopes::UserToolGrant`], which nothing
//! outside this module can build. The host that speaks to the operator's server
//! (tier 1, a later slice) takes a grant by reference, so no call reaches an
//! operator's server without this module's decision (`ops/batch-10-plan.md`,
//! decision D3).
//!
//! ```mermaid
//! flowchart TD
//!   call["a tools/call from a session"] --> own{"Tool::from_wire_name recognises the name?"}
//!   own -- "yes: one of Ori's eleven" --> auth["ToolScopes::authorize(role, tool)"]
//!   auth -- "false in the table" --> r1["ToolRefusal::ToolNotPermittedForRole, AICD §17"]
//!   auth -- "true" --> run["the server core runs the tool"]
//!   own -- "no" --> split{"split_proxied: server__tool?"}
//!   split -- "no" --> unknown["unknown tool, not this module's refusal"]
//!   split -- "yes" --> door["ToolScopes::authorize_user_tool(caller, server)"]
//!   door -- "1. another project" --> r2["ServerNotInProject, AICD §26"]
//!   door -- "2. reach not acknowledged" --> r3["ServerReachNotAcknowledged, AICD §17"]
//!   door -- "3. role not listed" --> r4["ServerNotExposedToRole, AICD §17"]
//!   door -- "4. otherwise" --> grant["UserToolGrant"]
//!   grant --> host["the host calls the operator's server"]
//! ```
//!
//! # The roles column is the table
//!
//! [`crate::scopes::ToolScopes::permits`] is one `match` on the role with an
//! exhaustive `match` on the tool inside each arm, and no wildcard arm anywhere.
//! A new role or a new tool fails to compile here until someone decides who
//! holds it, instead of inheriting a grant, which is how `ori-memory`'s
//! `scope.rs` writes its own table. The test module reads `spec/API_SPEC.md`
//! section 3's first table at test time and requires every tool, every role and
//! the Phase 1 column to agree with the code, so the table in the specification
//! and the table in this file cannot drift apart unnoticed.
//!
//! | Tool | Roles |
//! |---|---|
//! | `aicd_context`, `aicd_search` | all seven |
//! | `aicd_evidence` | lead, qa, operations |
//! | `aicd_plan_submit` | coder |
//! | `aicd_report` | coder, qa, operations, documentation |
//! | `aicd_escalate` | lead, coder |
//! | `aicd_ticket_create` | qa, operations, product_signal |
//! | `aicd_ticket_upgrade` | lead |
//! | `aicd_criterion_propose` | qa |
//! | `aicd_spec_propose` | documentation |
//! | `aicd_gate_result` | qa |
//!
//! The specification's roles cell for `aicd_gate_result` reads "qa (test env),
//! system". `system` is an [`ori_core::types::Actor`], not an
//! [`ori_core::types::Role`], and the engine's own writes do not pass through
//! tool scopes, so the table here holds `qa` alone. Gate runs are written by
//! `ori-gates` and the CI slot (decision D8).
//!
//! # What this table does not decide
//!
//! - Whether a tool is built. Phase 1 builds eight of the eleven
//!   ([`crate::scopes::Tool::in_phase_1`]); the table answers who would hold a
//!   tool, and the server core checks the phase before it dispatches.
//! - Whether the call's arguments are allowed. Which ticket a coder may name,
//!   what a report may hold and what an escalation must carry are the tools'
//!   own checks; the refusals for the ones that belong to the session are
//!   declared on [`crate::scopes::ToolRefusal`] so that slice S2 does not
//!   reopen this file.
//! - Anything about the operator's exposure of a server beyond the four checks
//!   of [`crate::scopes::ToolScopes::authorize_user_tool`]. The exposure is
//!   data the operator recorded (`mcp.server_added`), not a row of this table.
//!
//! # The three promises
//!
//! "Agents never receive file write tools to `spec/` or `ops/`, never receive
//! the merge operation, and never receive credentials through MCP"
//! (`spec/API_SPEC.md` section 3). They bind Ori's own tools, and the way this
//! module keeps the first part is that no tool is named for any of the three; a
//! test refuses a wire name containing `merge`, `credential`, `secret` or
//! `write`. They do not bind a proxied tool, whose reach is the operator's
//! decision (R45 point 1), which is why a server whose reach the operator has
//! not acknowledged is exposed to no role.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/scopes.rs`, "What agents
//! can do". A change here is a change to what an agent may do.
//!
//! Must not: expose a tool not in `spec/API_SPEC.md` section 3
//! (`spec/LLD.md` section 2), and hold no policy outside this file and the
//! operator's recorded exposure.

use core::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::{Id, Role};

/// AICD §7, "The agent team and separation of duties": a ticket belongs to one
/// agent.
const ROLES_SECTION: u8 = 7;

/// AICD §17, "Autonomy tiers and the permission model": what each role may do.
const PERMISSIONS_SECTION: u8 = 17;

/// AICD §25, "Memory service: implementation model": the scope enforcer maps an
/// identity to a role and refuses what is outside it.
const MEMORY_SERVICE_SECTION: u8 = 25;

/// AICD §26, "Operating several products with one team": one fleet per product,
/// and fleets never share context.
const PRODUCTS_SECTION: u8 = 26;

/// AICD §27, "Security and compliance in depth": the threat model a name that
/// imitates another is a threat under.
const SECURITY_SECTION: u8 = 27;

/// A reference to a whole methodology section, built without a check because
/// every caller in this module passes one of the constants above.
const fn whole_section(section: u8) -> MethodologyRef {
    MethodologyRef {
        section,
        subsection: None,
    }
}

// ---------------------------------------------------------------------------
// The tools
// ---------------------------------------------------------------------------

/// One of Ori's own eleven tools, in the order `spec/API_SPEC.md` section 3's
/// table lists them: AICD §17.
///
/// The table is the whole of them (`spec/API_SPEC.md` section 3, "Proxied
/// tools"). A tool of an operator's server is not one of these: it is named
/// `<server>__<tool>` and reaches the session through
/// [`ToolScopes::authorize_user_tool`] and [`split_proxied`]. Tool names are
/// stable (API_SPEC section 5), so no variant is ever removed, and a variant
/// that is not built yet ([`Tool::in_phase_1`]) stays.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tool {
    /// `aicd_context`: the bounded context package.
    Context,
    /// `aicd_search`: scoped search over allowed layers.
    Search,
    /// `aicd_evidence`: raw evidence, returned labeled untrusted and logged.
    Evidence,
    /// `aicd_plan_submit`: stores the plan and runs the lock check.
    PlanSubmit,
    /// `aicd_report`: closing, blocked, QA run and drift audit reports.
    Report,
    /// `aicd_escalate`: opens an escalation.
    Escalate,
    /// `aicd_ticket_create`: files a ticket. Not built in phase 1.
    TicketCreate,
    /// `aicd_ticket_upgrade`: raises a ticket's category, never lowers it.
    TicketUpgrade,
    /// `aicd_criterion_propose`: enters a criterion in the `proposed` state
    /// only. Not built in phase 1.
    CriterionPropose,
    /// `aicd_spec_propose`: opens a specification pull request.
    SpecPropose,
    /// `aicd_gate_result`: records a gate run. Not built in phase 1.
    GateResult,
}

impl Tool {
    /// Every tool, in the order of `spec/API_SPEC.md` section 3's table.
    pub const ALL: [Tool; 11] = [
        Tool::Context,
        Tool::Search,
        Tool::Evidence,
        Tool::PlanSubmit,
        Tool::Report,
        Tool::Escalate,
        Tool::TicketCreate,
        Tool::TicketUpgrade,
        Tool::CriterionPropose,
        Tool::SpecPropose,
        Tool::GateResult,
    ];

    /// The name the tool carries on the wire, which is the name in
    /// `spec/API_SPEC.md` section 3 up to its argument list.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Context => "aicd_context",
            Self::Search => "aicd_search",
            Self::Evidence => "aicd_evidence",
            Self::PlanSubmit => "aicd_plan_submit",
            Self::Report => "aicd_report",
            Self::Escalate => "aicd_escalate",
            Self::TicketCreate => "aicd_ticket_create",
            Self::TicketUpgrade => "aicd_ticket_upgrade",
            Self::CriterionPropose => "aicd_criterion_propose",
            Self::SpecPropose => "aicd_spec_propose",
            Self::GateResult => "aicd_gate_result",
        }
    }

    /// The tool a wire name names, or `None` when the name is none of Ori's
    /// eleven.
    ///
    /// Read from [`Tool::ALL`] and [`Tool::wire_name`], so the spelling lives in
    /// one place. The comparison is exact: no case folding and no trimming, so
    /// a name has one reading.
    #[must_use]
    pub fn from_wire_name(name: &str) -> Option<Tool> {
        Self::ALL.into_iter().find(|tool| tool.wire_name() == name)
    }

    /// Whether phase 1 builds the tool: decision D8 of `ops/batch-10-plan.md`
    /// and `spec/API_SPEC.md` section 3's Phase 1 column.
    ///
    /// False for `aicd_ticket_create` (held only by roles that run unattended,
    /// which arrive in phase 4), `aicd_criterion_propose` (qa's, and proposed
    /// criteria arrive in phase 2) and `aicd_gate_result` (one of its two roles
    /// is an actor, and gate runs are written by `ori-gates` and the CI slot).
    #[must_use]
    pub const fn in_phase_1(self) -> bool {
        match self {
            Self::Context
            | Self::Search
            | Self::Evidence
            | Self::PlanSubmit
            | Self::Report
            | Self::Escalate
            | Self::TicketUpgrade
            | Self::SpecPropose => true,
            Self::TicketCreate | Self::CriterionPropose | Self::GateResult => false,
        }
    }
}

impl fmt::Display for Tool {
    /// Writes the wire name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_name())
    }
}

// ---------------------------------------------------------------------------
// The operator's server names and the proxied-name rule
// ---------------------------------------------------------------------------

/// The longest a server name may be, in bytes: the `{0,31}` of
/// `[a-z][a-z0-9-]{0,31}` plus its first character.
const SERVER_NAME_MAX: usize = 32;

/// What a server name must not begin with, so a proxied tool's name never
/// begins as one of Ori's own does.
const SERVER_NAME_RESERVED_PREFIX: &str = "aicd";

/// The separator between a proxied tool's server and tool parts.
const PROXY_SEPARATOR: &str = "__";

/// The name the operator gave an MCP server when adding it to a project:
/// AICD §27.
///
/// `spec/API_SPEC.md` section 3, "Proxied tools", condition 2: a server's name
/// matches `[a-z][a-z0-9-]{0,31}` and does not begin with `aicd`, and
/// `project.mcp.add` accepts no other. The rule exists so that a proxied tool's
/// name has one reading and never imitates one of Ori's own tools. A name has
/// no underscore, so the first `__` in `<server>__<tool>` always splits the
/// server from the tool whatever underscores the tool's own name holds
/// ([`split_proxied`]), and a name does not begin with `aicd`, so a proxied
/// tool's name never begins as the name of one of Ori's tools does.
///
/// The only constructor is [`ServerName::parse`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ServerName(String);

impl ServerName {
    /// Reads a server name, refusing anything outside the rule above.
    ///
    /// The checks run in this order: empty, longer than 32 bytes, a first
    /// character outside `a` to `z`, a later character outside `a` to `z`,
    /// `0` to `9` and `-`, and last the reserved prefix. The length is in bytes,
    /// which is the length in characters for every name that could be valid,
    /// because every valid name is ASCII; the check costs nothing on a long
    /// hostile input. The reported `index` is a byte offset, and every character
    /// before the offending one passed, so it is ASCII and the offset is also
    /// the character's position.
    ///
    /// # Errors
    ///
    /// A [`ServerNameError`] naming the first rule the text breaks.
    pub fn parse(text: &str) -> Result<ServerName, ServerNameError> {
        if text.is_empty() {
            return Err(ServerNameError::Empty);
        }
        if text.len() > SERVER_NAME_MAX {
            return Err(ServerNameError::TooLong { length: text.len() });
        }
        for (index, character) in text.char_indices() {
            if index == 0 {
                if !character.is_ascii_lowercase() {
                    return Err(ServerNameError::FirstCharacter);
                }
            } else if !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-')
            {
                return Err(ServerNameError::Character { index });
            }
        }
        if text.starts_with(SERVER_NAME_RESERVED_PREFIX) {
            return Err(ServerNameError::ReservedPrefix);
        }
        Ok(ServerName(text.to_owned()))
    }

    /// The name as the operator wrote it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerName {
    /// Writes the name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a text is not a server name: AICD §27.
///
/// None of the variants echoes the text that was refused.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ServerNameError {
    /// The text is empty.
    Empty,
    /// The text is longer than 32 bytes.
    TooLong {
        /// The length of the text in bytes.
        length: usize,
    },
    /// The first character is not `a` to `z`.
    FirstCharacter,
    /// A character after the first is not `a` to `z`, `0` to `9` or `-`.
    Character {
        /// The byte offset of the first such character.
        index: usize,
    },
    /// The name begins with `aicd`, which every one of Ori's own tool names
    /// does, so a proxied tool could imitate one.
    ReservedPrefix,
}

impl ServerNameError {
    /// The section every refusal of a server name is made under: AICD §27.
    ///
    /// The rule exists so a proxied tool's name has one reading and never
    /// imitates one of Ori's own tools, which is a threat of the kind AICD §27
    /// describes.
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        whole_section(SECURITY_SECTION)
    }
}

impl fmt::Display for ServerNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a server name is empty"),
            Self::TooLong { length } => write!(
                f,
                "a server name is {length} bytes long and at most {SERVER_NAME_MAX} are allowed"
            ),
            Self::FirstCharacter => {
                f.write_str("a server name must begin with a character from a to z")
            }
            Self::Character { index } => write!(
                f,
                "a server name may hold only a to z, 0 to 9 and a hyphen, and the character at \
                 offset {index} is none of them"
            ),
            Self::ReservedPrefix => write!(
                f,
                "a server name must not begin with {SERVER_NAME_RESERVED_PREFIX}"
            ),
        }
    }
}

impl std::error::Error for ServerNameError {}

/// Splits a proxied tool's name at its first `__` into the server's name and the
/// tool's own name: AICD §27.
///
/// `spec/API_SPEC.md` section 3, "Proxied tools", condition 2. The answer is
/// `None` when:
///
/// - there is no `__` in the name;
/// - the part before the first `__` is not a server name
///   ([`ServerName::parse`]), which includes an empty part and one with an
///   underscore, so `a_b__c` is not a tool of a server named `a`; or
/// - the part after it is empty.
///
/// The tool part may itself hold `__`: `github__a__b` is the tool `a__b` of the
/// server `github`. The tool part is otherwise not examined; it comes from the
/// operator's server and is untrusted data (condition 3).
///
/// A name that [`Tool::from_wire_name`] recognises is never a proxied tool.
/// None of Ori's names holds `__`, so this function returns `None` for each of
/// them, but the server core still asks [`Tool::from_wire_name`] first, so that
/// the order is explicit rather than a consequence of how the names happen to
/// be spelled.
#[must_use]
pub fn split_proxied(name: &str) -> Option<(ServerName, &str)> {
    let (server, tool) = name.split_once(PROXY_SEPARATOR)?;
    if tool.is_empty() {
        return None;
    }
    let server = ServerName::parse(server).ok()?;
    Some((server, tool))
}

// ---------------------------------------------------------------------------
// The caller and the server
// ---------------------------------------------------------------------------

/// Who is calling: the role and the product of the session the engine bound the
/// connection to.
///
/// Slice S2 builds it from the session binding the engine registered, **never
/// from a tool argument**: `spec/SECURITY_NOTES.md`, trust boundary 2, makes the
/// connection's identity the session that was handed the endpoint, and "no field
/// of any message can change the binding". A value built from anything the agent
/// sent would let the agent choose its own role (AICD §17) or its own product
/// (AICD §26).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Caller {
    role: Role,
    product_id: Id,
}

impl Caller {
    /// A caller of this role in this product.
    #[must_use]
    pub fn new(role: Role, product_id: Id) -> Self {
        Self { role, product_id }
    }

    /// The role the engine registered for the session.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The product the session belongs to.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }
}

/// A server the operator added to a product and what the operator decided about
/// exposing it: AICD §26, AICD §17.
///
/// Slice H1's registry builds it from the product's `mcp.server_added` event:
/// the server's name, the roles it is exposed to (`exposed_to_roles`) and
/// whether the operator acknowledged the reach (`acknowledged_reach`).
/// `spec/API_SPEC.md` section 3, "Proxied tools", conditions 1 and 5. It holds
/// no transport, command or secret: a server's own secret stays with the engine
/// (condition 4).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExposedServer {
    product_id: Id,
    name: ServerName,
    exposed_to_roles: Vec<Role>,
    acknowledged_reach: bool,
}

impl ExposedServer {
    /// A server of this product, exposed to these roles, with the operator's
    /// acknowledgement recorded as given.
    #[must_use]
    pub fn new(
        product_id: Id,
        name: ServerName,
        exposed_to_roles: Vec<Role>,
        acknowledged_reach: bool,
    ) -> Self {
        Self {
            product_id,
            name,
            exposed_to_roles,
            acknowledged_reach,
        }
    }

    /// The product the operator added the server to.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The name the operator gave the server.
    #[must_use]
    pub const fn name(&self) -> &ServerName {
        &self.name
    }

    /// The roles the operator exposed the server to.
    #[must_use]
    pub fn exposed_to_roles(&self) -> &[Role] {
        &self.exposed_to_roles
    }

    /// Whether the operator acknowledged the server's reach when exposing it.
    #[must_use]
    pub const fn acknowledged_reach(&self) -> bool {
        self.acknowledged_reach
    }
}

// ---------------------------------------------------------------------------
// The refusals
// ---------------------------------------------------------------------------

/// A refusal this crate's tool surface makes: AICD §17.
///
/// Every variant carries a methodology section through
/// [`ToolRefusal::methodology_ref`], and the type has no variant without one.
/// The variants are all declared here, though slice S2 raises most of them, so
/// that the server core does not reopen this tier 2 file to add a refusal. No
/// variant carries agent free text: `mcp.*` events hold identifiers and
/// lengths, never what an agent wrote (`spec/API_SPEC.md` section 3).
///
/// The `Display` is written by hand, as ruling R20 allows until `thiserror` is
/// adopted; converting it is mechanical, each arm becoming an `#[error]`
/// attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ToolRefusal {
    /// The role does not hold the tool in the table of
    /// `spec/API_SPEC.md` section 3. AICD §17.
    ///
    /// Raised by [`ToolScopes::authorize`], which the server core calls before it
    /// runs any of Ori's own tools.
    ToolNotPermittedForRole {
        /// The role of the session that called.
        role: Role,
        /// The tool it called.
        tool: Tool,
    },
    /// The server belongs to another project than the caller's. AICD §26.
    ///
    /// Raised by [`ToolScopes::authorize_user_tool`] when the products differ,
    /// and by the server core when the registry holds no server of that name in
    /// the caller's product, which is what a session of another project
    /// reaching for it comes to (ORI-P1-038). Checked before everything else
    /// about the server, so a session of another project learns nothing of the
    /// server's roles or acknowledgement.
    ServerNotInProject {
        /// The server's name as the call spelled it.
        server: ServerName,
    },
    /// The operator did not acknowledge the server's reach, so it is exposed to
    /// no role. AICD §17.
    ///
    /// Raised by [`ToolScopes::authorize_user_tool`] whatever `exposed_to_roles`
    /// says: R45 point 1 and `spec/API_SPEC.md` section 3, "Proxied tools",
    /// condition 5, "A server added without it is exposed to no role".
    ServerReachNotAcknowledged {
        /// The server's name.
        server: ServerName,
    },
    /// The operator did not expose the server to the caller's role. AICD §17.
    ///
    /// Raised by [`ToolScopes::authorize_user_tool`] when the caller's role is
    /// not in the server's `exposed_to_roles` (ORI-P1-038, "Coder in A refused
    /// (role)").
    ServerNotExposedToRole {
        /// The role of the session that called.
        role: Role,
        /// The server's name.
        server: ServerName,
    },
    /// The call names a ticket the calling session is not assigned. AICD §7.
    ///
    /// Raised by the server core when a tool that acts on one ticket is given a
    /// ticket other than the one the session's identity holds: a ticket belongs
    /// to one agent (AICD §7), and the assignment is the engine's record, not
    /// the caller's word.
    TicketNotAssigned {
        /// The ticket the call named.
        ticket: Id,
    },
    /// A claim in the call contradicts the session the engine registered.
    /// AICD §25.
    ///
    /// Raised by the server core when an argument asserts something about the
    /// caller (its role, its product, its identity) that differs from the
    /// session binding. The binding wins, and the call is refused rather than
    /// quietly corrected, as the scope enforcer of AICD §25 refuses a request
    /// outside its scope. `claim` names the field, a constant of the server
    /// core, never the agent's value.
    ClaimContradictsSession {
        /// Which claim, as a constant name chosen by the server core.
        claim: &'static str,
    },
    /// The session that holds the connection is not active. AICD §17.
    ///
    /// Raised by the server core when the session has ended or was suspended,
    /// so a connection that outlived its session grants nothing.
    SessionNotActive,
}

impl ToolRefusal {
    /// A stable code naming the refusal, for the `mcp.tool_refused` event and
    /// the reply: AICD §17.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ToolNotPermittedForRole { .. } => "tool_not_permitted_for_role",
            Self::ServerNotInProject { .. } => "server_not_in_project",
            Self::ServerReachNotAcknowledged { .. } => "server_reach_not_acknowledged",
            Self::ServerNotExposedToRole { .. } => "server_not_exposed_to_role",
            Self::TicketNotAssigned { .. } => "ticket_not_assigned",
            Self::ClaimContradictsSession { .. } => "claim_contradicts_session",
            Self::SessionNotActive => "session_not_active",
        }
    }

    /// The methodology section this refusal is made under, which every variant
    /// carries: criterion ORI-P1-033.
    ///
    /// AICD §17 for a role, a reach or a session; AICD §26 for another
    /// project's server; AICD §7 for another agent's ticket; AICD §25 for a
    /// claim the scope enforcer would not accept.
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        let section = match self {
            Self::ToolNotPermittedForRole { .. } => PERMISSIONS_SECTION,
            Self::ServerNotInProject { .. } => PRODUCTS_SECTION,
            Self::ServerReachNotAcknowledged { .. } => PERMISSIONS_SECTION,
            Self::ServerNotExposedToRole { .. } => PERMISSIONS_SECTION,
            Self::TicketNotAssigned { .. } => ROLES_SECTION,
            Self::ClaimContradictsSession { .. } => MEMORY_SERVICE_SECTION,
            Self::SessionNotActive => PERMISSIONS_SECTION,
        };
        whole_section(section)
    }
}

impl fmt::Display for ToolRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refused under {}: ", self.methodology_ref())?;
        match self {
            Self::ToolNotPermittedForRole { role, tool } => {
                write!(f, "the {role} role may not call {tool}")
            }
            Self::ServerNotInProject { server } => {
                write!(
                    f,
                    "the server {server} is not one of this product's servers"
                )
            }
            Self::ServerReachNotAcknowledged { server } => write!(
                f,
                "the operator has not acknowledged the reach of the server {server}, so it is \
                 exposed to no role"
            ),
            Self::ServerNotExposedToRole { role, server } => {
                write!(f, "the server {server} is not exposed to the {role} role")
            }
            Self::TicketNotAssigned { ticket } => {
                write!(f, "the ticket {ticket} is not assigned to this session")
            }
            Self::ClaimContradictsSession { claim } => write!(
                f,
                "the call's {claim} claim contradicts the session the engine registered"
            ),
            Self::SessionNotActive => f.write_str("the session is not active"),
        }
    }
}

impl std::error::Error for ToolRefusal {}

// ---------------------------------------------------------------------------
// The grant and the table
// ---------------------------------------------------------------------------

/// Permission to reach one operator's server as one role in one product:
/// AICD §17, AICD §26.
///
/// **Sealed.** The fields are private and there is no public constructor, no
/// `Default` and no `Clone`. The only way to obtain one is
/// [`ToolScopes::authorize_user_tool`], so a function that takes a
/// `&UserToolGrant` can only have been reached by a call that passed its four
/// checks; the host that speaks to the operator's server therefore holds no
/// policy (decision D3). A grant is for the call it was made for: a caller that
/// wants a second call makes a second decision.
///
/// ```
/// use ori_core::types::{Id, Role};
/// use ori_mcp::scopes::{Caller, ExposedServer, ServerName, ToolScopes};
///
/// let product = Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
/// let caller = Caller::new(Role::Qa, product.clone());
/// let server = ExposedServer::new(
///     product,
///     ServerName::parse("github").unwrap(),
///     vec![Role::Qa],
///     true,
/// );
/// let grant = ToolScopes::authorize_user_tool(&caller, &server).unwrap();
/// assert_eq!(grant.role(), Role::Qa);
/// assert_eq!(grant.server().as_str(), "github");
/// ```
///
/// The fields cannot be named outside this module, so a grant cannot be built
/// by hand:
///
/// ```compile_fail,E0451
/// use ori_core::types::{Id, Role};
/// use ori_mcp::scopes::{ServerName, UserToolGrant};
///
/// let _grant = UserToolGrant {
///     product_id: Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
///     server: ServerName::parse("github").unwrap(),
///     role: Role::Qa,
/// };
/// ```
///
/// It has no `Default`:
///
/// ```compile_fail,E0599
/// use ori_mcp::scopes::UserToolGrant;
///
/// let _grant = UserToolGrant::default();
/// ```
///
/// And it cannot be copied, so a grant cannot be kept and spent again:
///
/// ```compile_fail,E0599
/// use ori_core::types::{Id, Role};
/// use ori_mcp::scopes::{Caller, ExposedServer, ServerName, ToolScopes};
///
/// let product = Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
/// let caller = Caller::new(Role::Qa, product.clone());
/// let server = ExposedServer::new(
///     product,
///     ServerName::parse("github").unwrap(),
///     vec![Role::Qa],
///     true,
/// );
/// let grant = ToolScopes::authorize_user_tool(&caller, &server).unwrap();
/// let _again = grant.clone();
/// ```
#[derive(Debug)]
pub struct UserToolGrant {
    product_id: Id,
    server: ServerName,
    role: Role,
}

impl UserToolGrant {
    /// The product the grant is in.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The server the grant reaches.
    #[must_use]
    pub const fn server(&self) -> &ServerName {
        &self.server
    }

    /// The role the grant was made to.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// The decisions of the tool surface, as functions of a role and a tool or of a
/// caller and a server: AICD §17, AICD §25.
///
/// A namespace: it holds no state.
pub struct ToolScopes;

impl ToolScopes {
    /// Whether this role holds this tool in `spec/API_SPEC.md` section 3's roles
    /// column: AICD §17.
    ///
    /// One `match` on the role and, in each arm, an exhaustive `match` on the
    /// tool, with no wildcard anywhere. The column's "all" is all seven roles.
    /// The column's "qa (test env), system" for `aicd_gate_result` names
    /// `system`, which is an [`ori_core::types::Actor`] and not a [`Role`], so
    /// only `qa` holds it here; the engine's own writes do not pass through tool
    /// scopes.
    ///
    /// The answer is the roles column alone. Whether phase 1 builds the tool is
    /// [`Tool::in_phase_1`], which the server core checks as well.
    #[must_use]
    pub const fn permits(role: Role, tool: Tool) -> bool {
        match role {
            Role::Coder => match tool {
                Tool::Context | Tool::Search | Tool::PlanSubmit | Tool::Report | Tool::Escalate => {
                    true
                }
                Tool::Evidence
                | Tool::TicketCreate
                | Tool::TicketUpgrade
                | Tool::CriterionPropose
                | Tool::SpecPropose
                | Tool::GateResult => false,
            },
            Role::Lead => match tool {
                Tool::Context
                | Tool::Search
                | Tool::Evidence
                | Tool::Escalate
                | Tool::TicketUpgrade => true,
                Tool::PlanSubmit
                | Tool::Report
                | Tool::TicketCreate
                | Tool::CriterionPropose
                | Tool::SpecPropose
                | Tool::GateResult => false,
            },
            Role::Qa => match tool {
                Tool::Context
                | Tool::Search
                | Tool::Evidence
                | Tool::Report
                | Tool::TicketCreate
                | Tool::CriterionPropose
                | Tool::GateResult => true,
                Tool::PlanSubmit | Tool::Escalate | Tool::TicketUpgrade | Tool::SpecPropose => {
                    false
                }
            },
            Role::Operations => match tool {
                Tool::Context
                | Tool::Search
                | Tool::Evidence
                | Tool::Report
                | Tool::TicketCreate => true,
                Tool::PlanSubmit
                | Tool::Escalate
                | Tool::TicketUpgrade
                | Tool::CriterionPropose
                | Tool::SpecPropose
                | Tool::GateResult => false,
            },
            Role::Documentation => match tool {
                Tool::Context | Tool::Search | Tool::Report | Tool::SpecPropose => true,
                Tool::Evidence
                | Tool::PlanSubmit
                | Tool::Escalate
                | Tool::TicketCreate
                | Tool::TicketUpgrade
                | Tool::CriterionPropose
                | Tool::GateResult => false,
            },
            Role::ProductSignal => match tool {
                Tool::Context | Tool::Search | Tool::TicketCreate => true,
                Tool::Evidence
                | Tool::PlanSubmit
                | Tool::Report
                | Tool::Escalate
                | Tool::TicketUpgrade
                | Tool::CriterionPropose
                | Tool::SpecPropose
                | Tool::GateResult => false,
            },
            Role::Assistant => match tool {
                Tool::Context | Tool::Search => true,
                Tool::Evidence
                | Tool::PlanSubmit
                | Tool::Report
                | Tool::Escalate
                | Tool::TicketCreate
                | Tool::TicketUpgrade
                | Tool::CriterionPropose
                | Tool::SpecPropose
                | Tool::GateResult => false,
            },
        }
    }

    /// Authorizes one call of one of Ori's own tools: AICD §17.
    ///
    /// # Errors
    ///
    /// [`ToolRefusal::ToolNotPermittedForRole`] when [`ToolScopes::permits`] is
    /// false for the pair.
    pub fn authorize(role: Role, tool: Tool) -> Result<(), ToolRefusal> {
        if Self::permits(role, tool) {
            Ok(())
        } else {
            Err(ToolRefusal::ToolNotPermittedForRole { role, tool })
        }
    }

    /// Authorizes a caller to reach an operator's server, and returns the only
    /// value that lets the host do it: AICD §17, AICD §26.
    ///
    /// The checks run in this order, and the order is part of the contract.
    /// ORI-P1-038's "qa in B refused (project)" must come out as the project
    /// refusal whatever the server's roles say, so the project is checked first
    /// and nothing about a server of another project is revealed.
    ///
    /// 1. The caller's product is the server's product, or
    ///    [`ToolRefusal::ServerNotInProject`].
    /// 2. The operator acknowledged the server's reach, or
    ///    [`ToolRefusal::ServerReachNotAcknowledged`]. A server added without
    ///    the acknowledgement is exposed to no role (R45 point 1), so this
    ///    holds for a role the server lists as well.
    /// 3. The caller's role is in the server's `exposed_to_roles`, or
    ///    [`ToolRefusal::ServerNotExposedToRole`].
    /// 4. Otherwise a [`UserToolGrant`].
    ///
    /// # Errors
    ///
    /// The first failing check's [`ToolRefusal`].
    pub fn authorize_user_tool(
        caller: &Caller,
        server: &ExposedServer,
    ) -> Result<UserToolGrant, ToolRefusal> {
        if caller.product_id != server.product_id {
            return Err(ToolRefusal::ServerNotInProject {
                server: server.name.clone(),
            });
        }
        if !server.acknowledged_reach {
            return Err(ToolRefusal::ServerReachNotAcknowledged {
                server: server.name.clone(),
            });
        }
        if !server.exposed_to_roles.contains(&caller.role) {
            return Err(ToolRefusal::ServerNotExposedToRole {
                role: caller.role,
                server: server.name.clone(),
            });
        }
        Ok(UserToolGrant {
            product_id: caller.product_id.clone(),
            server: server.name.clone(),
            role: caller.role,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;
    use std::str::FromStr;

    use ori_core::permission::{self, Action, Resource};
    use ori_core::types::Actor;

    use super::*;

    /// Two distinct, valid product identifiers.
    const PRODUCT_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const PRODUCT_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRY";

    fn id(text: &str) -> Id {
        Id::parse(text).expect("a valid ULID")
    }

    fn name(text: &str) -> ServerName {
        ServerName::parse(text).expect("a valid server name")
    }

    fn caller(role: Role, product: &str) -> Caller {
        Caller::new(role, id(product))
    }

    fn server(product: &str, exposed_to: &[Role], acknowledged: bool) -> ExposedServer {
        ExposedServer::new(
            id(product),
            name("github"),
            exposed_to.to_vec(),
            acknowledged,
        )
    }

    // -------------------------------------------------------------------
    // Reading the specification's tool table
    // -------------------------------------------------------------------

    /// One row of `spec/API_SPEC.md` section 3's first table.
    struct SpecRow {
        wire_name: String,
        roles: HashSet<Role>,
        names_system: bool,
        phase_1: bool,
    }

    /// Reads the first table under the heading that begins `## 3.`.
    ///
    /// The header row is checked, so a reordered column fails loudly rather than
    /// being read as another.
    fn parse_tool_table(text: &str) -> Vec<SpecRow> {
        let mut table: Vec<&str> = Vec::new();
        for line in text
            .lines()
            .skip_while(|line| !line.starts_with("## 3."))
            .skip(1)
        {
            if line.starts_with('#') {
                break;
            }
            let trimmed = line.trim();
            if trimmed.starts_with('|') {
                table.push(trimmed);
            } else if !table.is_empty() {
                break;
            }
        }
        assert!(
            table.len() > 2,
            "no tool table was found under the heading that begins `## 3.`"
        );
        let cells = |row: &str| -> Vec<String> {
            row.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        };
        let header = cells(table[0]);
        assert_eq!(
            header,
            ["Tool", "Roles", "Effect", "Phase 1"],
            "the tool table's columns are no longer the ones this reader knows"
        );
        table[2..]
            .iter()
            .map(|row| {
                let cells = cells(row);
                assert_eq!(cells.len(), 4, "a tool table row of another width: {row}");
                let wire_name = cells[0]
                    .trim_matches('`')
                    .split('(')
                    .next()
                    .expect("split yields one item")
                    .to_owned();
                let mut roles = HashSet::new();
                let mut names_system = false;
                if cells[1] == "all" {
                    roles.extend(Role::ALL.iter().copied());
                } else {
                    for item in cells[1].split(',') {
                        let item = item
                            .split('(')
                            .next()
                            .expect("split yields one item")
                            .trim();
                        if item == "system" {
                            names_system = true;
                        } else {
                            let role = Role::from_str(item)
                                .unwrap_or_else(|_| panic!("{item:?} is not a role"));
                            roles.insert(role);
                        }
                    }
                }
                let phase_1 = match cells[3].as_str() {
                    "Yes" => true,
                    "No" => false,
                    other => panic!("a Phase 1 cell that is neither Yes nor No: {other:?}"),
                };
                SpecRow {
                    wire_name,
                    roles,
                    names_system,
                    phase_1,
                }
            })
            .collect()
    }

    fn spec_rows() -> Vec<SpecRow> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../../spec/API_SPEC.md");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        parse_tool_table(&text)
    }

    #[test]
    fn ori_t_0166_the_table_reader_reads_a_synthetic_table() {
        let text = "\
# Title

## 2. Earlier

| Tool | Roles | Effect | Phase 1 |
|---|---|---|---|
| `earlier(x)` | coder | not this table | Yes |

## 3. Agent-facing MCP server

Some prose.

| Tool | Roles | Effect | Phase 1 |
|---|---|---|---|
| `aicd_one(a, b)` | all | first | Yes |
| `aicd_two(a)` | lead, qa (test env), system | second | No |

| Tool | Roles | Effect | Phase 1 |
|---|---|---|---|
| `aicd_other` | coder | the second table is not read | Yes |
";
        let rows = parse_tool_table(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].wire_name, "aicd_one");
        assert_eq!(rows[0].roles.len(), 7);
        assert!(!rows[0].names_system);
        assert!(rows[0].phase_1);
        assert_eq!(rows[1].wire_name, "aicd_two");
        assert_eq!(rows[1].roles, HashSet::from([Role::Lead, Role::Qa]));
        assert!(rows[1].names_system);
        assert!(!rows[1].phase_1);
    }

    #[test]
    fn ori_t_0166_tool_all_is_the_spec_tables_names_in_order() {
        let from_spec: Vec<String> = spec_rows().into_iter().map(|row| row.wire_name).collect();
        let from_code: Vec<&str> = Tool::ALL.iter().map(|tool| tool.wire_name()).collect();
        assert_eq!(from_spec, from_code);
        assert_eq!(from_spec.len(), 11);
    }

    /// An exhaustive, wildcard-free position for every variant, so a variant
    /// added to the enum fails to compile until it is given a place in
    /// [`Tool::ALL`]'s order.
    fn position(tool: Tool) -> usize {
        match tool {
            Tool::Context => 0,
            Tool::Search => 1,
            Tool::Evidence => 2,
            Tool::PlanSubmit => 3,
            Tool::Report => 4,
            Tool::Escalate => 5,
            Tool::TicketCreate => 6,
            Tool::TicketUpgrade => 7,
            Tool::CriterionPropose => 8,
            Tool::SpecPropose => 9,
            Tool::GateResult => 10,
        }
    }

    #[test]
    fn ori_t_0166_tool_all_lists_every_variant_at_its_position() {
        for (index, tool) in Tool::ALL.iter().enumerate() {
            assert_eq!(
                position(*tool),
                index,
                "{tool} is not at its position in Tool::ALL"
            );
        }
        let distinct: HashSet<Tool> = Tool::ALL.iter().copied().collect();
        assert_eq!(distinct.len(), Tool::ALL.len());
    }

    #[test]
    fn ori_t_0166_permits_equals_the_spec_roles_column_for_every_tool_and_role() {
        let mut checked = 0;
        for row in spec_rows() {
            let tool = Tool::from_wire_name(&row.wire_name)
                .unwrap_or_else(|| panic!("{} is in the spec and not in Tool", row.wire_name));
            for role in Role::ALL {
                assert_eq!(
                    ToolScopes::permits(*role, tool),
                    row.roles.contains(role),
                    "{role} and {tool}: the code and spec/API_SPEC.md section 3 disagree"
                );
                checked += 1;
            }
        }
        assert_eq!(
            checked,
            11 * 7,
            "every tool against every role was compared"
        );
    }

    #[test]
    fn ori_t_0166_in_phase_1_equals_the_spec_phase_1_column() {
        let rows = spec_rows();
        assert_eq!(
            rows.iter().filter(|row| row.phase_1).count(),
            8,
            "phase 1 builds eight tools"
        );
        for row in rows {
            let tool = Tool::from_wire_name(&row.wire_name).expect("a tool of the table");
            assert_eq!(tool.in_phase_1(), row.phase_1, "{tool}");
        }
    }

    #[test]
    fn ori_t_0166_system_is_named_only_in_the_gate_result_row() {
        let rows = spec_rows();
        let naming: Vec<&str> = rows
            .iter()
            .filter(|row| row.names_system)
            .map(|row| row.wire_name.as_str())
            .collect();
        assert_eq!(naming, [Tool::GateResult.wire_name()]);
        // The engine's own writes are an actor, not a role: no role holds the
        // tool but qa.
        for role in Role::ALL {
            assert_eq!(
                ToolScopes::permits(*role, Tool::GateResult),
                *role == Role::Qa
            );
        }
    }

    #[test]
    fn ori_t_0166_wire_names_round_trip_and_nothing_else_is_a_tool() {
        for tool in Tool::ALL {
            assert_eq!(Tool::from_wire_name(tool.wire_name()), Some(tool));
            assert_eq!(tool.to_string(), tool.wire_name());
        }
        let names: HashSet<&str> = Tool::ALL.iter().map(|tool| tool.wire_name()).collect();
        assert_eq!(names.len(), 11, "no two tools share a name");
        for other in [
            "",
            "aicd",
            "aicd_",
            "AICD_CONTEXT",
            " aicd_context",
            "aicd_context ",
            "aicd_context(ticket_id)",
            "aicd_merge",
            "github__create_issue",
        ] {
            assert_eq!(Tool::from_wire_name(other), None, "{other:?}");
        }
    }

    #[test]
    fn ori_t_0166_no_tool_is_named_for_the_three_promises() {
        // API_SPEC section 3: no file write tools, no merge operation, no
        // credentials through MCP.
        for tool in Tool::ALL {
            for forbidden in ["merge", "credential", "secret", "write"] {
                assert!(
                    !tool.wire_name().contains(forbidden),
                    "{tool} is named for {forbidden}"
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // Never wider than the permission matrix
    // -------------------------------------------------------------------

    /// The four tools whose effect AICD §17's matrix also governs, and the cell
    /// each is read against.
    ///
    /// Reported by this run (`--nocapture` shows it): the matrix allows
    /// `Propose` on the specification to qa (criteria only) and to documentation
    /// (via pull request), so `CriterionPropose` is narrower than the matrix for
    /// documentation, and `SpecPropose` is narrower for qa. Narrower is allowed;
    /// wider would be a defect.
    #[test]
    fn ori_t_0166_a_tool_is_never_wider_than_the_permission_matrix() {
        let cells = [
            (Tool::TicketCreate, Resource::Tickets, Action::Create),
            (
                Tool::TicketUpgrade,
                Resource::Tickets,
                Action::UpgradeCategory,
            ),
            (
                Tool::CriterionPropose,
                Resource::Specification,
                Action::Propose,
            ),
            (Tool::SpecPropose, Resource::Specification, Action::Propose),
        ];
        let actor = Actor::Agent(id(PRODUCT_A));
        let mut narrower = Vec::new();
        for (tool, resource, action) in cells {
            let mut held = 0;
            for role in Role::ALL {
                let allowed = permission::permits(&actor, *role, resource, action).is_allowed();
                if ToolScopes::permits(*role, tool) {
                    assert!(
                        allowed,
                        "{tool} is held by {role}, which AICD §17's matrix does not allow to \
                         {action} on {resource}"
                    );
                    held += 1;
                } else if allowed {
                    narrower.push(format!("{tool}: {role}"));
                }
            }
            assert!(
                held > 0,
                "{tool} is held by no role, so nothing was compared"
            );
        }
        eprintln!("roles the matrix allows and the tool table does not: {narrower:?}");
    }

    // -------------------------------------------------------------------
    // authorize
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0166_authorize_refuses_exactly_what_permits_refuses() {
        for role in Role::ALL {
            for tool in Tool::ALL {
                let outcome = ToolScopes::authorize(*role, tool);
                if ToolScopes::permits(*role, tool) {
                    assert_eq!(outcome, Ok(()));
                } else {
                    let refusal = outcome.expect_err("not permitted");
                    assert_eq!(
                        refusal,
                        ToolRefusal::ToolNotPermittedForRole { role: *role, tool }
                    );
                    assert_eq!(refusal.methodology_ref().section, 17);
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // ORI-P1-038, the authorization half
    // -------------------------------------------------------------------

    #[test]
    fn ori_p1_038_a_role_outside_exposed_to_roles_is_refused_under_section_17() {
        let refusal = ToolScopes::authorize_user_tool(
            &caller(Role::Coder, PRODUCT_A),
            &server(PRODUCT_A, &[Role::Qa], true),
        )
        .expect_err("a coder is not in the server's roles");
        assert_eq!(
            refusal,
            ToolRefusal::ServerNotExposedToRole {
                role: Role::Coder,
                server: name("github")
            }
        );
        assert_eq!(refusal.methodology_ref().section, 17);
    }

    #[test]
    fn ori_p1_038_a_server_of_another_project_is_refused_under_section_26() {
        let exposed_to_qa = server(PRODUCT_A, &[Role::Qa], true);
        let refusal = ToolScopes::authorize_user_tool(&caller(Role::Qa, PRODUCT_B), &exposed_to_qa)
            .expect_err("qa in B reaches a server of A");
        assert_eq!(
            refusal,
            ToolRefusal::ServerNotInProject {
                server: name("github")
            }
        );
        assert_eq!(refusal.methodology_ref().section, 26);

        // The project refusal wins when the role would have been refused too:
        // a coder of B reaching a server of A that lists qa only.
        let refusal =
            ToolScopes::authorize_user_tool(&caller(Role::Coder, PRODUCT_B), &exposed_to_qa)
                .expect_err("a coder of B reaches a server of A");
        assert_eq!(
            refusal,
            ToolRefusal::ServerNotInProject {
                server: name("github")
            }
        );
    }

    #[test]
    fn ori_p1_038_a_listed_role_in_the_servers_project_gets_a_grant() {
        let grant = ToolScopes::authorize_user_tool(
            &caller(Role::Qa, PRODUCT_A),
            &server(PRODUCT_A, &[Role::Lead, Role::Qa], true),
        )
        .expect("qa is listed, in the right project, with the reach acknowledged");
        assert_eq!(grant.product_id(), &id(PRODUCT_A));
        assert_eq!(grant.server(), &name("github"));
        assert_eq!(grant.role(), Role::Qa);
    }

    #[test]
    fn ori_t_0166_the_checks_run_project_then_reach_then_role() {
        // Another project, reach not acknowledged, role not listed: the project.
        let refusal = ToolScopes::authorize_user_tool(
            &caller(Role::Coder, PRODUCT_B),
            &server(PRODUCT_A, &[Role::Qa], false),
        )
        .expect_err("refused");
        assert_eq!(refusal.code(), "server_not_in_project");

        // Right project, reach not acknowledged, role not listed: the reach.
        let refusal = ToolScopes::authorize_user_tool(
            &caller(Role::Coder, PRODUCT_A),
            &server(PRODUCT_A, &[Role::Qa], false),
        )
        .expect_err("refused");
        assert_eq!(refusal.code(), "server_reach_not_acknowledged");

        // Right project, reach acknowledged, role not listed: the role.
        let refusal = ToolScopes::authorize_user_tool(
            &caller(Role::Coder, PRODUCT_A),
            &server(PRODUCT_A, &[Role::Qa], true),
        )
        .expect_err("refused");
        assert_eq!(refusal.code(), "server_not_exposed_to_role");

        // A server exposed to nobody refuses everybody, under the role check.
        let refusal = ToolScopes::authorize_user_tool(
            &caller(Role::Qa, PRODUCT_A),
            &server(PRODUCT_A, &[], true),
        )
        .expect_err("refused");
        assert_eq!(refusal.code(), "server_not_exposed_to_role");
    }

    #[test]
    fn ori_t_0166_a_server_without_acknowledged_reach_is_exposed_to_no_role() {
        // R45 point 1, and API_SPEC section 3, "Proxied tools", condition 5.
        let everyone = Role::ALL;
        for role in Role::ALL {
            // The control: with the acknowledgement the same data grants.
            ToolScopes::authorize_user_tool(
                &caller(*role, PRODUCT_A),
                &server(PRODUCT_A, everyone, true),
            )
            .unwrap_or_else(|refusal| {
                panic!("{role} should be granted with the reach acknowledged: {refusal}")
            });

            let refusal = ToolScopes::authorize_user_tool(
                &caller(*role, PRODUCT_A),
                &server(PRODUCT_A, everyone, false),
            )
            .expect_err("a server without the acknowledgement is exposed to no role");
            assert_eq!(
                refusal,
                ToolRefusal::ServerReachNotAcknowledged {
                    server: name("github")
                }
            );
            assert_eq!(refusal.methodology_ref().section, 17);
        }
    }

    #[test]
    fn ori_t_0166_the_caller_and_the_server_hold_what_they_were_given() {
        let caller = caller(Role::Documentation, PRODUCT_A);
        assert_eq!(caller.role(), Role::Documentation);
        assert_eq!(caller.product_id(), &id(PRODUCT_A));
        let exposed = server(PRODUCT_B, &[Role::Lead, Role::Qa], true);
        assert_eq!(exposed.product_id(), &id(PRODUCT_B));
        assert_eq!(exposed.name(), &name("github"));
        assert_eq!(exposed.exposed_to_roles(), [Role::Lead, Role::Qa]);
        assert!(exposed.acknowledged_reach());
        assert!(!server(PRODUCT_B, &[], false).acknowledged_reach());
    }

    // -------------------------------------------------------------------
    // ORI-P1-033, every refusal carries a reference that resolves
    // -------------------------------------------------------------------

    /// Declares, for one enum, the list of one sample per variant and the
    /// expectation per variant from the same lines, so that a variant with no
    /// line here stops the build, and a line whose sample belongs to another
    /// variant shows up as a duplicate.
    ///
    /// The generated `match` has no wildcard and the arm grammar admits none.
    macro_rules! variant_table {
        (
            $enum:ident, $samples:ident, $expected:ident, $out:ty;
            $( $variant:ident $({ $($pattern:tt)* })? => $sample:expr, $expect:expr; )+
        ) => {
            fn $samples() -> Vec<$enum> {
                vec![ $( $sample ),+ ]
            }

            fn $expected(value: &$enum) -> $out {
                match value {
                    $( $enum::$variant $({ $($pattern)* })? => $expect, )+
                }
            }
        };
    }

    variant_table! {
        ToolRefusal, every_refusal, refusal_expectation, (&'static str, u8);
        ToolNotPermittedForRole { .. } => ToolRefusal::ToolNotPermittedForRole {
            role: Role::Coder,
            tool: Tool::Evidence,
        }, ("tool_not_permitted_for_role", 17);
        ServerNotInProject { .. } => ToolRefusal::ServerNotInProject { server: name("github") },
            ("server_not_in_project", 26);
        ServerReachNotAcknowledged { .. } => ToolRefusal::ServerReachNotAcknowledged {
            server: name("github"),
        }, ("server_reach_not_acknowledged", 17);
        ServerNotExposedToRole { .. } => ToolRefusal::ServerNotExposedToRole {
            role: Role::Coder,
            server: name("github"),
        }, ("server_not_exposed_to_role", 17);
        TicketNotAssigned { .. } => ToolRefusal::TicketNotAssigned { ticket: id(PRODUCT_A) },
            ("ticket_not_assigned", 7);
        ClaimContradictsSession { .. } => ToolRefusal::ClaimContradictsSession { claim: "role" },
            ("claim_contradicts_session", 25);
        SessionNotActive => ToolRefusal::SessionNotActive, ("session_not_active", 17);
    }

    variant_table! {
        ServerNameError, every_name_error, name_error_expectation, u8;
        Empty => ServerNameError::Empty, 27;
        TooLong { .. } => ServerNameError::TooLong { length: 33 }, 27;
        FirstCharacter => ServerNameError::FirstCharacter, 27;
        Character { .. } => ServerNameError::Character { index: 1 }, 27;
        ReservedPrefix => ServerNameError::ReservedPrefix, 27;
    }

    #[test]
    fn ori_p1_033_every_tool_refusal_carries_a_methodology_ref_that_resolves() {
        let refusals = every_refusal();
        assert_eq!(
            refusals.len(),
            7,
            "the seven refusals the server core needs"
        );
        let mut codes = HashSet::new();
        for refusal in &refusals {
            let (code, section) = refusal_expectation(refusal);
            assert_eq!(refusal.code(), code);
            let reference = refusal.methodology_ref();
            assert!(
                reference.resolves(),
                "{refusal:?} cites a section that does not resolve"
            );
            assert_eq!(reference.section, section, "{refusal:?}");
            assert_eq!(reference.subsection, None);
            assert!(
                refusal
                    .to_string()
                    .starts_with(&format!("refused under {reference}: ")),
                "{refusal}"
            );
            codes.insert(refusal.code());
        }
        assert_eq!(
            codes.len(),
            refusals.len(),
            "two variants share a code, or one is listed twice"
        );
    }

    #[test]
    fn ori_p1_033_every_server_name_error_carries_a_methodology_ref_that_resolves() {
        let errors = every_name_error();
        assert_eq!(errors.len(), 5);
        let distinct: HashSet<ServerNameError> = errors.iter().copied().collect();
        assert_eq!(distinct.len(), errors.len(), "one variant is listed twice");
        for error in &errors {
            let reference = error.methodology_ref();
            assert!(
                reference.resolves(),
                "{error:?} cites a section that does not resolve"
            );
            assert_eq!(reference.section, name_error_expectation(error));
            assert_eq!(reference.subsection, None);
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn ori_t_0166_refusal_texts_name_what_was_refused() {
        let text = ToolRefusal::ToolNotPermittedForRole {
            role: Role::Coder,
            tool: Tool::Evidence,
        }
        .to_string();
        assert!(
            text.contains("coder") && text.contains("aicd_evidence"),
            "{text}"
        );
        let text = ToolRefusal::ServerNotExposedToRole {
            role: Role::Qa,
            server: name("github"),
        }
        .to_string();
        assert!(text.contains("qa") && text.contains("github"), "{text}");
        let text = ToolRefusal::TicketNotAssigned {
            ticket: id(PRODUCT_B),
        }
        .to_string();
        assert!(text.contains(PRODUCT_B), "{text}");
        // A server-name error never echoes the text it refused.
        let refused = "x\u{202e}evil";
        let error = ServerName::parse(refused).expect_err("not a name");
        assert!(!error.to_string().contains("evil"));
    }

    // -------------------------------------------------------------------
    // Server names and the proxied-name rule
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0166_valid_server_names_are_accepted() {
        let longest = "a".repeat(32);
        for text in [
            "a",
            "github",
            "my-server-2",
            "a-",
            "a--b",
            "aic",
            "z9",
            longest.as_str(),
        ] {
            let parsed =
                ServerName::parse(text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
            assert_eq!(parsed.as_str(), text);
            assert_eq!(parsed.to_string(), text);
        }
    }

    #[test]
    fn ori_t_0166_each_server_name_error_has_a_name_that_causes_it() {
        let too_long = "a".repeat(33);
        let cases: [(&str, ServerNameError); 7] = [
            ("", ServerNameError::Empty),
            (too_long.as_str(), ServerNameError::TooLong { length: 33 }),
            ("2fa", ServerNameError::FirstCharacter),
            ("My", ServerNameError::FirstCharacter),
            ("a_b", ServerNameError::Character { index: 1 }),
            ("aicd", ServerNameError::ReservedPrefix),
            ("aicdx", ServerNameError::ReservedPrefix),
        ];
        let mut seen = HashSet::new();
        for (text, expected) in cases {
            let error = ServerName::parse(text).expect_err(text);
            assert_eq!(error, expected, "{text:?}");
            seen.insert(std::mem::discriminant(&error));
        }
        assert_eq!(
            seen.len(),
            every_name_error().len(),
            "every variant has a case"
        );
    }

    #[test]
    fn ori_t_0166_the_server_name_checks_run_in_the_documented_order() {
        let cases: [(&str, ServerNameError); 14] = [
            // Length before anything else, so a long hostile input is refused
            // without being read.
            (
                "2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ServerNameError::TooLong { length: 41 },
            ),
            // First character before the others.
            ("-a", ServerNameError::FirstCharacter),
            ("Aicd", ServerNameError::FirstCharacter),
            ("\u{e9}", ServerNameError::FirstCharacter),
            // The other characters, by offset.
            ("aMy", ServerNameError::Character { index: 1 }),
            ("ab_", ServerNameError::Character { index: 2 }),
            ("a b", ServerNameError::Character { index: 1 }),
            ("a.b", ServerNameError::Character { index: 1 }),
            ("a\n", ServerNameError::Character { index: 1 }),
            ("a\0b", ServerNameError::Character { index: 1 }),
            ("a\u{e9}", ServerNameError::Character { index: 1 }),
            ("a__b", ServerNameError::Character { index: 1 }),
            // The character check before the prefix check.
            ("aicd_x", ServerNameError::Character { index: 4 }),
            // The prefix, last.
            ("aicd-x", ServerNameError::ReservedPrefix),
        ];
        for (text, expected) in cases {
            assert_eq!(ServerName::parse(text), Err(expected), "{text:?}");
        }
    }

    #[test]
    fn ori_t_0166_split_proxied_splits_at_the_first_double_underscore() {
        let (server, tool) = split_proxied("github__create_issue").expect("a proxied name");
        assert_eq!((server.as_str(), tool), ("github", "create_issue"));

        // The tool part may hold `__` itself.
        let (server, tool) = split_proxied("github__a__b").expect("a proxied name");
        assert_eq!((server.as_str(), tool), ("github", "a__b"));

        // Three underscores: the first `__` splits, and the tool keeps the rest.
        let (server, tool) = split_proxied("a___b").expect("a proxied name");
        assert_eq!((server.as_str(), tool), ("a", "_b"));

        // The tool part is the operator's server's own name for it, untrusted,
        // and is never read as one of Ori's tools.
        let (server, tool) = split_proxied("github__aicd_context").expect("a proxied name");
        assert_eq!((server.as_str(), tool), ("github", "aicd_context"));
    }

    #[test]
    fn ori_t_0166_split_proxied_refuses_what_is_not_a_proxied_name() {
        for text in [
            "aicd_context",
            "github_",
            "github__",
            "__x",
            "Git__x",
            "a_b__c",
            "aicd__x",
            "",
            "__",
            "github",
            "9a__x",
        ] {
            assert!(split_proxied(text).is_none(), "{text:?}");
        }
        // None of Ori's own names is a proxied name.
        for tool in Tool::ALL {
            assert!(split_proxied(tool.wire_name()).is_none(), "{tool}");
        }
    }

    #[test]
    fn ori_t_0166_a_proxied_name_has_one_reading() {
        // No server name holds an underscore, so `a__b` and `a_b__c` cannot both
        // read as the server `a`: the second has no server at all.
        let (server, tool) = split_proxied("a__b").expect("a proxied name");
        assert_eq!((server.as_str(), tool), ("a", "b"));
        assert!(split_proxied("a_b__c").is_none());
        assert!(ServerName::parse("a_b").is_err());
    }
}
