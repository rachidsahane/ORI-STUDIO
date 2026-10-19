//! The ports: what the server reaches through the engine, declared here and
//! implemented by `ori-engine`: AICD §17.
//!
//! # Why traits here
//!
//! The server core (slice S2) and the handlers of its tools must reach a ticket's
//! category, a specification pull request, a source of identifiers, the operator's
//! MCP servers, the time, the state of the session it serves, the plans agents
//! submit and the escalations they open, and `spec/LLD.md` section 2 draws no
//! edge from `ori-mcp` to `ori-orchestrator`, `ori-flows` or `ori-broker`, and
//! none is added (`ops/batch-10-plan.md`, decision D1). The edge the graph does draw is `ENG --> MCP`: `ori-engine`
//! depends on this crate, so a trait declared here and implemented there adds no
//! edge. These are not the adapter traits of `spec/API_SPEC.md` section 4, and
//! nothing is added to `ori-core` for them.
//!
//! ```mermaid
//! flowchart LR
//!   ENG["ori-engine"] -- "depends on" --> MCP["ori-mcp: Server, ToolScopes, eight ports"]
//!   ENG -- "depends on" --> ORCH["ori-orchestrator"]
//!   ENG -- "depends on" --> BRK["ori-broker"]
//!   ENG -. "implements IdSource, TicketDesk, SpecDesk, UserToolHost, Clock, SessionDesk, PlanDesk and EscalationDesk with what it holds" .-> MCP
//!   MCP -. "never" .-> ORCH
//!   MCP -. "never" .-> BRK
//! ```
//!
//! # What is declared
//!
//! Eight ports: [`crate::ports::IdSource`], [`crate::ports::TicketDesk`],
//! [`crate::ports::SpecDesk`], [`crate::ports::UserToolHost`],
//! [`crate::ports::Clock`], [`crate::ports::SessionDesk`],
//! [`crate::ports::PlanDesk`] and [`crate::ports::EscalationDesk`]. The first
//! four are slice S1's; the next two are slice S2's, and the server core calls
//! them itself: [`crate::ports::Clock`] stamps every event the server appends, and
//! [`crate::ports::SessionDesk`] says whether the session the connection serves
//! may still act. The last two are slice S9's, and the handlers of
//! `aicd_plan_submit` and `aicd_escalate` call them, not the server core:
//! [`crate::ports::PlanDesk`] stores a plan after the lock check, and
//! [`crate::ports::EscalationDesk`] opens an escalation. Both desks take agent
//! prose as `ori_memory::barrier::SanitizedField`, the sanitization barrier's
//! sealed type (escalation E-0012's public entry, plan prerequisite PM2), which
//! `sanitize_text` makes for text outside a record, `submit_report` makes for a
//! report's fields, and reading a stored record back makes. `PlanDesk` also takes
//! the declared paths as `ori_memory::scope::ModulePath`. So neither takes a raw
//! `String` where the barrier's output belongs. That is a statement about these
//! two ports, not about every port: [`crate::ports::SpecDesk::propose`]'s content
//! and [`crate::ports::UserToolHost::call_tool`]'s arguments are not barrier text,
//! by decision.
//!
//! # What every port promises
//!
//! - **Every method that writes takes `&mut ProductDb`**, so the single writer
//!   stays the caller's: the engine's writer task holds the one
//!   [`ori_store::db::ProductDb`] of a product (`spec/LLD.md` section 5), and
//!   an implementation writes through the handle it is given and opens none of
//!   its own.
//! - **No port carries a credential value.** A credential is a name the engine
//!   resolves through the broker; an operator's server's own secret stays with
//!   the engine, which connects to the server itself (`spec/API_SPEC.md`
//!   section 3, "Proxied tools", condition 4).
//! - **[`crate::ports::UserToolHost`] takes a
//!   [`crate::scopes::UserToolGrant`] by value**, which only
//!   [`crate::scopes::ToolScopes::authorize_user_tool`] can make and which each
//!   host call consumes, so no call reaches an operator's server without a
//!   decision of `scopes.rs` made for that call.
//!
//! [`crate::ports::Clock`], [`crate::ports::SessionDesk`],
//! [`crate::ports::PlanDesk`] and [`crate::ports::EscalationDesk`] are `Send`;
//! [`crate::ports::IdSource`], [`crate::ports::TicketDesk`],
//! [`crate::ports::SpecDesk`] and [`crate::ports::UserToolHost`] are not
//! required to be. A holder that must be `Send`, such as a
//! [`crate::tools::ToolHandler`], asks for `dyn Trait + Send`. The server or its
//! handlers hold the four `Send` ports as trait objects and the server is itself
//! `Send`, so the engine can build it on one thread and hand it to the product's
//! writer task.
//!
//! The desks of the effect, [`crate::ports::TicketDesk`] and
//! [`crate::ports::SpecDesk`], which slice S5's handlers call, receive the
//! [`crate::scopes::Caller`] that the server core built from the session binding,
//! never an identity read from a tool argument. The other ports take something
//! else: [`crate::ports::IdSource`] and [`crate::ports::Clock`] take nothing,
//! [`crate::ports::UserToolHost`] takes a [`crate::scopes::UserToolGrant`], and
//! [`crate::ports::SessionDesk`], [`crate::ports::PlanDesk`] and
//! [`crate::ports::EscalationDesk`] take the [`crate::server::Binding`] itself,
//! each for its own reason:
//!
//! - [`crate::ports::SessionDesk`] answers for the session;
//! - [`crate::ports::PlanDesk`] must record the session in each lock claim
//!   (`lock.claimed { module, session_id }`), and a [`crate::scopes::Caller`]
//!   carries none;
//! - [`crate::ports::EscalationDesk`] holds the session it may suspend.
//!
//! The binding, like the caller, carries the agent identity of the session, and a
//! port that writes records its write under it. A desk never assumes that the
//! server core's scope check ran: it makes the checks of the component it routes
//! the change through, and the two S9 desks, which take the binding itself, check
//! the binding as well. Each method below says which checks it makes.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/ports.rs`, "What agents
//! can do". Every method here is something an agent's call can cause.

use core::fmt;

use ori_core::error::{Error, MethodologyRef};
use ori_core::types::{Category, Id, Timestamp};
use ori_memory::barrier::SanitizedField;
use ori_memory::scope::ModulePath;
use ori_store::db::ProductDb;

use crate::protocol::{CallToolResult, JsonObject, Tool};
use crate::scopes::{Caller, UserToolGrant};
use crate::server::Binding;

/// Why a port could not do what it was asked: AICD §17.
///
/// Four kinds, and the server core answers three ways. The two refusal kinds,
/// [`PortError::Refused`] and [`PortError::Denied`], are answered alike: a
/// refusal by the component behind the port is passed on with its own code and
/// methodology section, whether `ori-core` has a `RefusalKind` for it (`Refused`)
/// or has none (`Denied`). An argument the port could not accept
/// ([`PortError::Invalid`]) is the caller's mistake, and an unavailable component
/// ([`PortError::Unavailable`]) is the engine's.
///
/// The free text in [`PortError::Invalid`]'s `why` and in
/// [`PortError::Unavailable`]'s `what` may hold implementation detail, such as
/// a server's command or its stderr. It reaches neither an `mcp.*` event, which
/// holds identifiers and lengths and never text (`spec/API_SPEC.md` section 3),
/// nor an agent's reply: the server core replies with the failure's kind and
/// code only.
///
/// The `Display` is written by hand, as ruling R20 allows until `thiserror` is
/// adopted; converting it is mechanical, each arm becoming an `#[error]`
/// attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PortError {
    /// The component behind the port refused. The error keeps its
    /// [`MethodologyRef`] and its `E_` code, such as `E_UPGRADE_ONLY` for a
    /// category that would be lowered (`spec/API_SPEC.md` section 1,
    /// `tickets.setCategory`).
    ///
    /// It must hold an [`Error`] that is a refusal, one with a
    /// [`MethodologyRef`]. The server core treats a `Refused` without one as an
    /// implementation defect and reports the failure as `invalid`. A refusal
    /// that has no `ori-core` `RefusalKind` is [`PortError::Denied`], not a
    /// `Refused` that holds an error without a reference.
    Refused(Error),
    /// The component behind the port refused under a rule that `ori-core`'s
    /// `RefusalKind` has no variant for (a lifecycle permission, an escalation
    /// rule, a specification rule), with its own fixed code and section.
    ///
    /// The server core answers it as it answers [`PortError::Refused`]:
    /// `failure: refused`, with the code and the section, always. **The code is
    /// an MCP `reason` value**, lower case like `source_not_in_scope`, and not an
    /// `E_` code: a new `E_` code would change `spec/API_SPEC.md`'s contract.
    ///
    /// A desk's codes are its own fixed constants, never built from agent text.
    /// A `&'static str` does not prove that, because `String::leak` makes one.
    Denied {
        /// The refusal's fixed code, an MCP `reason` value.
        code: &'static str,
        /// The methodology section it was refused under. The server core
        /// records and answers a reference that does not resolve as `invalid`,
        /// as for every refusal.
        methodology_ref: MethodologyRef,
    },
    /// An argument was not acceptable to the component behind the port.
    Invalid {
        /// The argument, by the name the tool gives it.
        field: &'static str,
        /// Why it was not acceptable.
        why: String,
    },
    /// The component behind the port could not be reached or could not answer.
    Unavailable {
        /// What was unavailable.
        what: String,
    },
}

impl PortError {
    /// The methodology section of the refusal this error carries: AICD §17.
    ///
    /// For [`PortError::Refused`], the section of the held error's refusal, or
    /// `None` if the held error carries none (a `Malformed` error is not a
    /// refusal). Always `Some` for [`PortError::Denied`]. `None` for
    /// [`PortError::Invalid`] and [`PortError::Unavailable`], which are not
    /// control refusals (the shape of
    /// [`ori_core::error::Error::methodology_ref`], for the reason that module
    /// gives).
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Refused(error) => error.methodology_ref(),
            Self::Denied {
                methodology_ref, ..
            } => Some(methodology_ref.clone()),
            Self::Invalid { .. } | Self::Unavailable { .. } => None,
        }
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) => write!(f, "{error}"),
            Self::Denied {
                code,
                methodology_ref,
            } => write!(f, "refused under {methodology_ref}: {code}"),
            Self::Invalid { field, why } => write!(f, "{field} is not acceptable: {why}"),
            Self::Unavailable { what } => write!(f, "{what} is unavailable"),
        }
    }
}

impl std::error::Error for PortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(error) => Some(error),
            Self::Denied { .. } | Self::Invalid { .. } | Self::Unavailable { .. } => None,
        }
    }
}

/// A source of identifiers: `Id` never generates one (`ori-core`), because a
/// ULID needs a clock and a source of randomness, which the engine holds.
///
/// Takes no `ProductDb` and writes nothing: the identifier is a value, and the
/// caller records it. No credential passes through this port.
pub trait IdSource {
    /// The next identifier, never one returned before.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the engine's clock or random source
    /// cannot answer.
    fn next_id(&mut self) -> Result<Id, PortError>;
}

/// Ticket operations the server reaches through the engine: AICD §11, AICD §17.
///
/// The implementation holds the rules of `ori-orchestrator`; this crate holds
/// none of them. A method that writes takes `&mut ProductDb`, so the single
/// writer stays the caller's. No credential passes through this port.
pub trait TicketDesk {
    /// Raises a ticket's category, which AICD §11 allows in one direction only.
    ///
    /// Called for `aicd_ticket_upgrade`. The implementation never assumes the
    /// server core's scope check ran, so it:
    ///
    /// - records its write with `Actor::Agent`, naming `caller.identity()` as
    ///   the agent;
    /// - refuses a caller whose product is not `db`'s product, as
    ///   `Denied { code: "product_database_mismatch", methodology_ref: AICD §26 }`
    ///   ([`PortError::Denied`]), the code the server core's own first check uses
    ///   for that fact and the answer [`crate::ports::PlanDesk`] and
    ///   [`crate::ports::EscalationDesk`] give for it too;
    /// - routes the change through `ori-orchestrator`'s `Lifecycle::advance`,
    ///   which evaluates AICD §17's matrix itself.
    ///
    /// It refuses what `ori-core`'s `Ticket::apply` refuses as a category
    /// downgrade, with an error that keeps the code `E_UPGRADE_ONLY`
    /// ([`PortError::Refused`]). Besides the checks this contract lists (the
    /// product), the desk applies no business rule of its own beyond what the
    /// component it routes through refuses. It may answer [`PortError::Denied`]
    /// for a refusal that `ori-core`'s `RefusalKind` has no variant for, such as a
    /// lifecycle permission; any `Denied` code of this desk besides
    /// `product_database_mismatch` is left to slice S5 and ORI-T-0070.
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal by the component behind the port that
    /// `ori-core` has a refusal kind for, [`PortError::Denied`] for one it has
    /// none for, [`PortError::Invalid`] for a ticket the caller's product does
    /// not hold, [`PortError::Unavailable`] when it could not answer.
    fn upgrade(
        &mut self,
        db: &mut ProductDb,
        caller: &Caller,
        ticket: &Id,
        to: Category,
    ) -> Result<(), PortError>;
}

/// Specification operations the server reaches through the engine: AICD §9,
/// AICD §17.
///
/// A method that writes takes `&mut ProductDb`, so the single writer stays the
/// caller's. No credential passes through this port: opening the pull request
/// uses a credential the engine resolves through the broker, by name, and the
/// reply carries only the pull request's reference.
pub trait SpecDesk {
    /// Opens a specification pull request; returns its reference for the reply.
    ///
    /// Called for `aicd_spec_propose`. The agent never writes `spec/` itself:
    /// the content becomes a proposal a human reviews (`spec/API_SPEC.md`
    /// section 3, the first promise).
    ///
    /// The implementation never assumes the server core's scope check ran, so
    /// it:
    ///
    /// - records its write with `Actor::Agent`, naming `caller.identity()` as
    ///   the agent;
    /// - refuses a caller whose product is not `db`'s product, as
    ///   `Denied { code: "product_database_mismatch", methodology_ref: AICD §26 }`
    ///   ([`PortError::Denied`]), as [`crate::ports::TicketDesk`],
    ///   [`crate::ports::PlanDesk`] and [`crate::ports::EscalationDesk`] do;
    /// - routes the proposal through the owning component's own checks, which
    ///   evaluate AICD §17's matrix for the specification themselves.
    ///
    /// A specification rule that `ori-core`'s `RefusalKind` has no variant for is
    /// answered [`PortError::Denied`] too; any `Denied` code of this desk besides
    /// `product_database_mismatch` is left to slice S5 and ORI-T-0070.
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal by the component behind the port that
    /// `ori-core` has a refusal kind for, [`PortError::Denied`] for one it has
    /// none for, [`PortError::Invalid`] for a document that is not one of the
    /// specification's, [`PortError::Unavailable`] when it could not answer.
    fn propose(
        &mut self,
        db: &mut ProductDb,
        caller: &Caller,
        document: &Id,
        content: &str,
    ) -> Result<String, PortError>;
}

/// The host toward the operator's MCP servers, as the server core sees it:
/// AICD §17, AICD §26.
///
/// **Every method takes a [`UserToolGrant`] by value**, which only
/// [`crate::scopes::ToolScopes::authorize_user_tool`] can make, and each host
/// call consumes it. The grant cannot be copied, so one decision covers one host
/// call; the caller (slices S2 and S6) calls `authorize_user_tool` for every
/// call, from the registry's state at that moment. No call reaches an
/// operator's server without `scopes.rs`'s decision, and the host that
/// implements this holds no policy. The grant names the product, the server and
/// the role. It names the server by name, and the implementation resolves that
/// name in that product when the call is made and reaches no other server.
///
/// The server's own secret stays with the engine, which connects to the server
/// itself, and no credential passes through this port.
///
/// **What the server returns is untrusted data, and the host is not trusted to
/// filter it** (`spec/API_SPEC.md` section 3, "Proxied tools", condition 3). The
/// implementation returns what the operator's server returned. The server
/// core's proxy (slice S6, `tools/proxy.rs`, tier 2) enforces condition 3 on
/// whatever the host returns: it keeps text blocks only, drops every other
/// block and any `structured_content`, and labels every block it forwards
/// untrusted.
pub trait UserToolHost {
    /// The tools of the granted server, as the server named them: untrusted
    /// data, names and descriptions included. The grant is consumed.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the server cannot be reached.
    fn list_tools(&mut self, grant: UserToolGrant) -> Result<Vec<Tool>, PortError>;

    /// Calls `tool` of the granted server with `arguments`, and returns its
    /// result as the server returned it. The grant is consumed.
    ///
    /// `tool` is the server's own name for the tool, the part after the first
    /// `__` of the name the agent called ([`crate::scopes::split_proxied`]),
    /// once the server core has matched it against the server's tool list.
    ///
    /// # Errors
    ///
    /// [`PortError::Invalid`] for a tool the server does not have,
    /// [`PortError::Unavailable`] when the server cannot be reached or does not
    /// answer.
    fn call_tool(
        &mut self,
        grant: UserToolGrant,
        tool: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, PortError>;
}

/// The engine's clock: AICD §17.
///
/// Every event the server appends takes its `at` from here, **never from a
/// message**: an agent cannot choose the time of its own audit trail, and the
/// call budget's window runs on this clock too. `ori-core` has no clock
/// (`Timestamp` is built from a value the caller has), so the engine, which has
/// one, implements this.
///
/// Takes no `ProductDb` and writes nothing; no credential passes through this
/// port. The server reads it once at the entry of every call, which is the
/// call's own time, and once more when it records a failure. `Send`, because the
/// server that holds it is handed to the product's writer task.
pub trait Clock: Send {
    /// The current time. It may step backwards (a clock set back by an
    /// operator): the call budget starts a new window when it does.
    fn now(&mut self) -> Timestamp;
}

/// Whether the session a connection serves may still act: AICD §17.
///
/// A connection grants what the session behind it holds, and nothing once that
/// session is not active (suspended for an escalation, ended, revoked). The
/// server asks before it looks at what a call names, and closes the connection
/// on a refusal, so a session that is over cannot probe the tool surface.
///
/// The caller holds the single `ProductDb` writer ([`crate::ports`]'s "What
/// every port promises"): an implementation reads the session's state through
/// the handle it is given and opens none of its own. No credential passes
/// through this port. `Send`, because the server that holds it is handed to the
/// product's writer task.
pub trait SessionDesk: Send {
    /// Whether the session `binding` names is active.
    ///
    /// `binding` is the engine's own record of the session the connection
    /// serves; the implementation answers for that session and for no other.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the session's state could not be read.
    /// The server refuses the call and closes the connection: it fails closed.
    fn is_active(&mut self, db: &mut ProductDb, binding: &Binding) -> Result<bool, PortError>;
}

/// Plan operations the server reaches through the engine: AICD §12, AICD §17.
///
/// Called by the handler of `aicd_plan_submit` (slice S9), never by the server
/// core. AICD §12 has every implementation plan declare the modules it will
/// touch, and the lead refuse a ticket whose declared scope overlaps one already
/// claimed: this port is where the plan is stored and that check is made. The
/// plan arrives as a [`SanitizedField`], the sanitization barrier's own output,
/// and the declared scope as [`ModulePath`]s the handler has already read
/// strictly. The desk must mint its own identifiers, so the handler needs none
/// from an [`IdSource`]. No credential passes through this port. `Send`, because
/// the server that holds it is handed to the product's writer task.
///
/// The port takes the [`Binding`], as [`SessionDesk`] does, and not a
/// [`Caller`]: the desk must record the session in each lock claim
/// (`lock.claimed { module, session_id }`), and a `Caller` carries none. The tool
/// is coder-only, so no lead reaches this port.
pub trait PlanDesk: Send {
    /// Stores a ticket's plan and claims the modules it declares: AICD §12.
    ///
    /// `plan` is the sanitized text, untrimmed. `declared_scope` is the agent's
    /// list in its own order, duplicates included: the handler passes both on
    /// unchanged. `LockTable::claim` takes an `ori_core::types::Scope`, and
    /// `Scope::new` trims each path, strips trailing `/` characters, collapses
    /// duplicates and sorts. So the agent's order and duplicates never reach the
    /// table.
    ///
    /// The implementation never assumes the server core's checks ran, so it
    /// must:
    ///
    /// - record its writes with `Actor::Agent`, naming `binding.identity()` as
    ///   the agent;
    /// - **check the binding itself, before its first append, in this order:**
    ///   - refuse a binding whose role is not `coder`, as
    ///     `Denied { code: "tool_not_permitted_for_role", methodology_ref: AICD §17 }`
    ///     ([`PortError::Denied`]), S2's check 5;
    ///   - refuse a coder binding with no bound ticket, as
    ///     `Denied { code: "ticket_not_assigned", methodology_ref: AICD §7 }`;
    ///   - refuse a coder binding bound to a ticket other than the call's, as
    ///     `Denied { code: "claim_contradicts_session", methodology_ref: AICD §25 }`.
    ///
    ///   The last two are the two codes of S2's check 7, for the same two facts;
    /// - then refuse a binding whose product is not `db`'s product, as
    ///   `Denied { code: "product_database_mismatch", methodology_ref: AICD §26 }`,
    ///   the server core's own code for that fact;
    /// - then refuse a ticket the product does not hold, as
    ///   `PortError::Invalid { field: "ticket_id" }`;
    /// - answer every refusal with a code and a section: [`PortError::Refused`]
    ///   for a refusal kind of `ori-core`, [`PortError::Denied`] otherwise. A
    ///   desk's codes are its own fixed constants, never built from agent text;
    /// - **make every refusal, and every `Invalid`, before its first append.** A
    ///   refused call stores nothing. Only an [`PortError::Unavailable`] can leave
    ///   rows behind it (see "Faults and retries" below);
    /// - write every event it appends with the ticket in its ticket column, and
    ///   read the time of its rows from the engine's [`Clock`]: the port passes
    ///   none.
    ///
    /// It must also:
    ///
    /// - **check, then store.** Run the lock check, `ori-orchestrator`'s
    ///   `LockTable::claim`, before it stores anything. On a conflict, pass the
    ///   error of `LockTable::claim` on unchanged, as [`PortError::Refused`]
    ///   holding `RefusalKind::ScopeLocked` (code `E_SCOPE_LOCKED`, AICD §12), and
    ///   store nothing. The module stays inside the error and never reaches the
    ///   agent: the server core replies with the code and the section only;
    /// - **claim for an in-progress ticket only.** The plan's declared scope
    ///   extends the ticket's own claim, and only while the ticket is
    ///   `InProgress`: a plan is submitted while the ticket is being worked on,
    ///   and a ticket that is escalated or in review is re-planned only through
    ///   the lead. Any other state is
    ///   `Denied { code: "ticket_not_in_progress", methodology_ref: AICD §11 }`;
    /// - **append** one plan row, then one `lock.claimed` per newly claimed
    ///   module, and nothing else.
    ///
    /// # The plan row
    ///
    /// `spec/DATA_MODEL.md` section 2's Plan row has seven fields: `id`,
    /// `ticket_id`, `session_id`, `content`, `declared_scope`, `approved_by` and
    /// `approved_at` ("Immutable; a new plan supersedes"). At submission the desk
    /// knows the first five. The approval fields belong to an approval this port
    /// does not perform. **ORI-T-0192** settles three things:
    ///
    /// - the row's event kind;
    /// - the order of its payload's keys;
    /// - how a submitted plan's row carries the approval fields.
    ///
    /// ORI-T-0192 is the specification ticket that settles batch 10's event names
    /// and rows, and its record in `ops/lock-table.md` lists these items.
    /// Whatever it names, no key a projection of the row reads may come after the
    /// plan or the paths in the payload: `ori-store`'s reader takes the first
    /// `"key"` it finds anywhere in a payload (ORI-T-0189), so a key that follows
    /// the agent's text could be set by it.
    ///
    /// # Newly claimed
    ///
    /// A module is newly claimed when `LockTable::claim` inserts a key it did not
    /// hold for this ticket before the call. A path under a module the ticket
    /// already holds is a new key: it gets its `lock.claimed` row and is counted.
    /// `claim` does not report which keys are new, so the desk compares the
    /// table's entries before and after.
    ///
    /// # Faults and retries
    ///
    /// A `Malformed` error is [`PortError::Unavailable`], whether it comes from
    /// `LockTable::claim`, from `Scope::new` or from a stored identifier: it is
    /// not a refusal, and a `Refused` without a section is recorded as `invalid`.
    /// `Unavailable` also answers a read that failed before any append. Each
    /// `EventLog::append` commits on its own, so an `Unavailable` after a first
    /// append leaves the rows before it. A retry then appends a second plan row.
    /// The claims already recorded are held for this ticket, so they are not
    /// newly claimed and are not appended again. That holds if the desk builds
    /// its lock table from the log, which is ORI-T-0070's choice.
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal that `ori-core` has a refusal kind
    /// for, `E_SCOPE_LOCKED` among them; [`PortError::Denied`] for one it has
    /// none for, the binding's refusals among them; [`PortError::Invalid`] for a
    /// ticket the product does not hold; [`PortError::Unavailable`] when it could
    /// not answer, which can follow appended rows.
    fn submit(
        &mut self,
        db: &mut ProductDb,
        binding: &Binding,
        ticket: &Id,
        plan: &SanitizedField,
        declared_scope: &[ModulePath],
    ) -> Result<PlanStored, PortError>;
}

/// What a stored plan answers with: AICD §12.
///
/// Identifiers and a count, never the plan or a path: the reply to an agent
/// echoes neither.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanStored {
    /// The identifier of the stored plan.
    pub plan_id: Id,
    /// How many modules this call newly claimed: the keys
    /// `LockTable::claim` inserted that it did not hold for this ticket before
    /// the call. A path under a module the ticket already holds is a new key and
    /// is counted.
    pub modules_claimed: usize,
}

/// Escalation operations the server reaches through the engine: AICD §12,
/// AICD §17.
///
/// Called by the handler of `aicd_escalate` (slice S9), never by the server
/// core. AICD §12 makes each escalation a question addressed to a human, with a
/// recommendation attached and evidence behind the trigger: this port is where
/// that is opened. Every text arrives as a [`SanitizedField`], the sanitization
/// barrier's own output. The desk must mint its own identifiers, so the handler
/// needs none from an [`IdSource`]. No credential passes through this port.
/// `Send`, because the server that holds it is handed to the product's writer
/// task.
///
/// The port takes the [`Binding`], as [`SessionDesk`] does, and not a
/// [`Caller`], because the desk holds the session it may suspend, which is the
/// binding's. A lead and a coder both reach this port.
pub trait EscalationDesk: Send {
    /// Opens an escalation on a ticket, and suspends the session if the
    /// configuration says so: AICD §12.
    ///
    /// The implementation never assumes the server core's checks ran, so it
    /// must:
    ///
    /// - record its writes with `Actor::Agent`, naming `binding.identity()` as
    ///   the agent;
    /// - **check the binding itself, before its first append, in this order:**
    ///   - refuse a binding whose role is neither `lead` nor `coder`, as
    ///     `Denied { code: "tool_not_permitted_for_role", methodology_ref: AICD §17 }`
    ///     ([`PortError::Denied`]), with S2's check 5 code;
    ///   - for a coder, refuse a binding with no bound ticket, as
    ///     `Denied { code: "ticket_not_assigned", methodology_ref: AICD §7 }`,
    ///     and a binding bound to a ticket other than the call's, as
    ///     `Denied { code: "claim_contradicts_session", methodology_ref: AICD §25 }`:
    ///     the two codes of S2's check 7, for the same two facts.
    ///
    ///   A lead may name any ticket the product holds, and the desk checks that
    ///   the product holds it;
    /// - then refuse a binding whose product is not `db`'s product, as
    ///   `Denied { code: "product_database_mismatch", methodology_ref: AICD §26 }`,
    ///   the server core's own code for that fact;
    /// - then refuse a ticket the product does not hold, as
    ///   `PortError::Invalid { field: "ticket_id" }`;
    /// - answer every refusal with a code and a section: [`PortError::Refused`]
    ///   for a refusal kind of `ori-core`, [`PortError::Denied`] otherwise. A
    ///   desk's codes are its own fixed constants, never built from agent text;
    /// - **make every refusal, and every `Invalid`, before its first append.** A
    ///   refused call stores nothing. Only an [`PortError::Unavailable`] can leave
    ///   rows behind it (see "Faults and retries" below);
    /// - write every event it appends with the ticket in its ticket column, and
    ///   read the time of its rows from the engine's [`Clock`]: the port passes
    ///   none.
    ///
    /// It must also:
    ///
    /// - **route the escalation through** `ori-orchestrator`'s
    ///   `Escalation::raise`, which raises a trigger only when a finding stands
    ///   behind it. For that the desk must:
    ///   - pass each text's `text()` unaltered;
    ///   - record every evidence entry as a `Finding` under the call's trigger in
    ///     the situation it hands `Escalation::raise`;
    ///   - map the trigger by its spelling to `ori-orchestrator`'s `Trigger`,
    ///     refusing any spelling it does not know.
    ///
    ///   An `EscalationError` it still meets is
    ///   `Denied { code: "escalation_refused", methodology_ref: AICD §12 }`.
    ///   `Escalation::raise` and `Finding::new` trim the texts they keep, so an
    ///   escalation built by the component holds trimmed text. The handler hands
    ///   the desk untrimmed text;
    /// - **append `escalation.raised`**, the kind `ori-store` projects. The
    ///   payload's keys are in this order: `escalation_id`, `trigger`,
    ///   `question`, `recommendation`, and `evidence` last. The payload is a
    ///   struct serialized by serde, which keeps its fields' declaration order;
    ///   a `serde_json::Value` map sorts its keys and must not be used.
    ///   `ori-store`'s projection reader takes the first `"key"` it finds
    ///   anywhere in the payload, values included, so with this order the
    ///   column a human triages on, `trigger`, is read before any agent text, and
    ///   no question, recommendation or evidence can set it;
    /// - **move the ticket to `Escalated` only for a coder's escalation**, which
    ///   the checks above have made an escalation on its own bound ticket, and
    ///   only when that ticket is `InProgress`. Ask `ori-orchestrator`'s
    ///   `Lifecycle::advance`, which is pure, for the move, as [`TicketDesk`]
    ///   does for its own, before the first append, and append
    ///   `ticket.escalated` once `advance` accepts. A lead's escalation never
    ///   moves the ticket: the coder working it would otherwise be stopped by a
    ///   session it does not hold;
    /// - **append one session row only when it suspends the session**,
    ///   carrying the ticket in its ticket column. Its event kind, and the
    ///   session's target state, are ORI-T-0192's to settle (its record in
    ///   `ops/lock-table.md` lists them), because `spec/DATA_MODEL.md`'s
    ///   `AgentSession` has no suspended state. `AgentIdentity` has `suspended`,
    ///   and `AgentSession`'s `Escalated` is terminal and revokes the session's
    ///   credentials. `EscalationOpened::suspended` says whether the session was
    ///   suspended. In phase 1 the desk never suspends, so it appends no session
    ///   row, and `suspended` is `false` until a configuration key exists, and
    ///   always `false` for a lead's session.
    ///
    /// It appends at most three rows: `escalation.raised`, `ticket.escalated` and
    /// one session row.
    ///
    /// # After a suspension
    ///
    /// If the engine's [`SessionDesk`] then reports the session inactive, S2
    /// refuses the session's next `tools/call` `session_not_active` and closes
    /// the connection. A call that S2's check 0 or check 1 refuses first is
    /// answered `product_database_mismatch` or `rate_limited`, and `ping` and
    /// `tools/list` do not consult `SessionDesk`.
    ///
    /// # Faults and retries
    ///
    /// A `Malformed` error is [`PortError::Unavailable`], whether it comes from a
    /// stored identifier or from `ori-orchestrator`: it is not a refusal, and a
    /// `Refused` without a section is recorded as `invalid`. `Unavailable` also
    /// answers a read that failed before any append. Each `EventLog::append`
    /// commits on its own, so an `Unavailable` after a first append leaves the
    /// rows before it. A retry then opens a second escalation, a second
    /// `escalation.raised`.
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal that `ori-core` has a refusal kind
    /// for; [`PortError::Denied`] for one it has none for, the binding's refusals
    /// and `escalation_refused` among them; [`PortError::Invalid`] for a ticket
    /// the product does not hold; [`PortError::Unavailable`] when it could not
    /// answer, which can follow appended rows.
    fn open(
        &mut self,
        db: &mut ProductDb,
        binding: &Binding,
        ticket: &Id,
        escalation: &NewEscalation,
    ) -> Result<EscalationOpened, PortError>;
}

/// An escalation as the handler hands it to the desk, every text sanitized:
/// AICD §12.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewEscalation {
    /// Which of the eight triggers applies, AICD §12's seven and this product's
    /// `precondition_missing`: always one of the
    /// `crate::tools::ESCALATION_TRIGGERS`, never the agent's own text.
    pub trigger: &'static str,
    /// The question for the human.
    pub question: SanitizedField,
    /// What the agent recommends.
    pub recommendation: SanitizedField,
    /// What stands behind the trigger: one to sixteen findings, in the agent's
    /// order.
    pub evidence: Vec<EscalationEvidence>,
}

/// One finding behind an escalation: what was found, and where: AICD §12.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationEvidence {
    /// What was found.
    pub what: SanitizedField,
    /// Where it was found.
    pub found_at: SanitizedField,
}

/// What an opened escalation answers with: AICD §12.
///
/// An identifier and a flag, never the question or the evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationOpened {
    /// The identifier of the escalation.
    pub escalation_id: Id,
    /// Whether the session was suspended. `false` in phase 1 until a
    /// configuration key exists, and always `false` for a lead's session.
    pub suspended: bool,
}

#[cfg(test)]
mod tests {
    use ori_core::error::RefusalKind;
    use ori_core::types::Role;
    use ori_memory::barrier::sanitize_text;

    use super::*;
    use crate::scopes::{ExposedServer, ServerName, ToolScopes};

    const PRODUCT: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    /// A valid identifier for the agent identity of a session.
    const AGENT: &str = "01D39ZY06FGSCTVN4T2V9PKHFZ";

    fn id(text: &str) -> Id {
        Id::parse(text).expect("a valid ULID")
    }

    /// A refusal from the component behind a port: AICD §11's upgrade-only rule.
    fn upgrade_only() -> Error {
        Error::refused(RefusalKind::CategoryDowngrade {
            from: Category::Decisional,
            to: Category::Auto,
        })
    }

    #[test]
    fn ori_t_0166_a_refused_port_error_keeps_its_methodology_ref_and_its_code() {
        let error = PortError::Refused(upgrade_only());
        let reference = error
            .methodology_ref()
            .expect("a refusal carries a reference");
        assert!(reference.resolves());
        assert_eq!(reference.section, 11);
        match &error {
            PortError::Refused(inner) => assert_eq!(inner.code(), Some("E_UPGRADE_ONLY")),
            other => panic!("not the refusal that was built: {other:?}"),
        }
        let text = error.to_string();
        assert!(
            text.contains("refused") && text.contains("AICD §11"),
            "{text}"
        );
        let source = std::error::Error::source(&error).expect("a refusal has a source");
        assert_eq!(source.to_string(), text);
    }

    #[test]
    fn ori_t_0166_an_invalid_or_unavailable_port_error_carries_no_methodology_ref() {
        let invalid = PortError::Invalid {
            field: "ticket_id",
            why: "no such ticket".to_owned(),
        };
        let unavailable = PortError::Unavailable {
            what: "the id source".to_owned(),
        };
        assert_eq!(invalid.methodology_ref(), None);
        assert_eq!(unavailable.methodology_ref(), None);
        assert_eq!(
            invalid.to_string(),
            "ticket_id is not acceptable: no such ticket"
        );
        assert_eq!(unavailable.to_string(), "the id source is unavailable");
        assert!(std::error::Error::source(&invalid).is_none());
        assert!(std::error::Error::source(&unavailable).is_none());
    }

    /// A refusal under a rule `ori-core`'s `RefusalKind` has no variant for: a
    /// ticket that is not in progress, AICD §11.
    fn not_in_progress() -> PortError {
        PortError::Denied {
            code: "ticket_not_in_progress",
            methodology_ref: MethodologyRef::at(11).expect("a section the methodology carries"),
        }
    }

    #[test]
    fn ori_t_0188_a_denied_port_error_keeps_its_code_and_its_methodology_ref() {
        let error = not_in_progress();
        let reference = error
            .methodology_ref()
            .expect("a denial always carries a reference");
        assert!(reference.resolves());
        assert_eq!(
            reference,
            MethodologyRef {
                section: 11,
                subsection: None
            }
        );
        match &error {
            PortError::Denied {
                code,
                methodology_ref,
            } => {
                assert_eq!(*code, "ticket_not_in_progress");
                assert_eq!(methodology_ref, &reference);
            }
            other => panic!("not the denial that was built: {other:?}"),
        }
        assert_eq!(
            error.to_string(),
            "refused under AICD §11: ticket_not_in_progress"
        );
        assert!(std::error::Error::source(&error).is_none());
        assert_ne!(error, PortError::Refused(upgrade_only()));
    }

    #[test]
    fn ori_t_0188_a_refused_error_that_is_malformed_carries_no_methodology_ref() {
        // `Refused` holds the section of the refusal it holds, and `None` for an
        // error that is not a refusal: that is what `Denied` exists to avoid.
        let malformed = PortError::Refused(Error::malformed("a module path", "x"));
        assert_eq!(malformed.methodology_ref(), None);
        let refused = PortError::Refused(upgrade_only());
        assert_eq!(
            refused.methodology_ref(),
            Some(MethodologyRef {
                section: 11,
                subsection: None
            })
        );
    }

    /// A port that answers from a script, to show the traits can be implemented
    /// and used as objects.
    struct Scripted {
        next: u32,
    }

    impl IdSource for Scripted {
        fn next_id(&mut self) -> Result<Id, PortError> {
            self.next += 1;
            if self.next > 1 {
                return Err(PortError::Unavailable {
                    what: "the id source".to_owned(),
                });
            }
            Ok(id(PRODUCT))
        }
    }

    impl TicketDesk for Scripted {
        fn upgrade(
            &mut self,
            _db: &mut ProductDb,
            _caller: &Caller,
            _ticket: &Id,
            _to: Category,
        ) -> Result<(), PortError> {
            Err(PortError::Refused(upgrade_only()))
        }
    }

    impl SpecDesk for Scripted {
        fn propose(
            &mut self,
            _db: &mut ProductDb,
            _caller: &Caller,
            _document: &Id,
            _content: &str,
        ) -> Result<String, PortError> {
            Ok("pull/1".to_owned())
        }
    }

    impl UserToolHost for Scripted {
        fn list_tools(&mut self, grant: UserToolGrant) -> Result<Vec<Tool>, PortError> {
            Ok(vec![Tool {
                name: format!("{}-tool", grant.server()),
                title: None,
                description: None,
                input_schema: JsonObject::new(),
                output_schema: None,
                annotations: None,
                execution: None,
            }])
        }

        fn call_tool(
            &mut self,
            grant: UserToolGrant,
            tool: &str,
            _arguments: Option<JsonObject>,
        ) -> Result<CallToolResult, PortError> {
            Ok(CallToolResult::text(format!("{}:{tool}", grant.server())))
        }
    }

    #[test]
    fn ori_t_0166_the_ports_are_object_safe_and_a_host_is_reached_only_with_a_grant() {
        let mut scripted = Scripted { next: 0 };
        // Each port is usable as a trait object, which is how the engine hands
        // them to the server core.
        let ids: &mut dyn IdSource = &mut scripted;
        assert_eq!(ids.next_id(), Ok(id(PRODUCT)));
        assert!(matches!(ids.next_id(), Err(PortError::Unavailable { .. })));
        let _: Option<&mut dyn TicketDesk> = None;
        let _: Option<&mut dyn SpecDesk> = None;

        // The only way to a grant is the decision in scopes.rs, and each host
        // call consumes the grant it was made for: two calls are two decisions.
        let product = id(PRODUCT);
        let caller = Caller::new(id(AGENT), Role::Qa, product.clone());
        let exposed = ExposedServer::new(
            product,
            ServerName::parse("github").expect("a server name"),
            vec![Role::Qa],
            true,
        );
        let host: &mut dyn UserToolHost = &mut scripted;
        let grant = ToolScopes::authorize_user_tool(&caller, &exposed).expect("a grant");
        let tools = host.list_tools(grant).expect("a list");
        assert_eq!(tools[0].name, "github-tool");
        let grant = ToolScopes::authorize_user_tool(&caller, &exposed).expect("a second grant");
        let result = host
            .call_tool(grant, "create_issue", None)
            .expect("a result");
        assert_eq!(result.content[0].as_text(), Some("github:create_issue"));
    }

    /// The engine's side of slice S9's two ports, answering from nothing.
    struct Desks;

    impl PlanDesk for Desks {
        fn submit(
            &mut self,
            _db: &mut ProductDb,
            _binding: &Binding,
            ticket: &Id,
            _plan: &SanitizedField,
            declared_scope: &[ModulePath],
        ) -> Result<PlanStored, PortError> {
            Ok(PlanStored {
                plan_id: ticket.clone(),
                modules_claimed: declared_scope.len(),
            })
        }
    }

    impl EscalationDesk for Desks {
        fn open(
            &mut self,
            _db: &mut ProductDb,
            _binding: &Binding,
            ticket: &Id,
            _escalation: &NewEscalation,
        ) -> Result<EscalationOpened, PortError> {
            Ok(EscalationOpened {
                escalation_id: ticket.clone(),
                suspended: false,
            })
        }
    }

    #[test]
    fn ori_t_0188_the_new_desks_are_send_trait_objects_over_sanitized_values() {
        fn assert_send<T: Send + ?Sized>() {}
        assert_send::<dyn PlanDesk>();
        assert_send::<dyn EscalationDesk>();
        let _: Option<Box<dyn PlanDesk>> = Some(Box::new(Desks));
        let _: Option<Box<dyn EscalationDesk>> = Some(Box::new(Desks));

        // The values the desks take are the barrier's output, and they compare by
        // value, which the handler's tests rely on.
        let finding = |what: &str, found_at: &str| EscalationEvidence {
            what: sanitize_text(what, 100).field,
            found_at: sanitize_text(found_at, 100).field,
        };
        let escalation = |question: &str| NewEscalation {
            trigger: "scope_conflict",
            question: sanitize_text(question, 100).field,
            recommendation: sanitize_text("go on", 100).field,
            evidence: vec![finding("a", "b")],
        };
        assert_eq!(escalation("q"), escalation("q"));
        assert_ne!(escalation("q"), escalation("r"));
        assert_ne!(
            PlanStored {
                plan_id: id(PRODUCT),
                modules_claimed: 1
            },
            PlanStored {
                plan_id: id(PRODUCT),
                modules_claimed: 2
            }
        );
    }
}
