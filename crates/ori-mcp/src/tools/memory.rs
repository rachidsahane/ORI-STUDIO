//! The memory tools `aicd_context` and `aicd_search`: AICD §25, AICD §8,
//! `spec/API_SPEC.md` section 3.
//!
//! Two handlers, [`crate::tools::memory::ContextHandler`] and
//! [`crate::tools::memory::SearchHandler`], that the engine registers with
//! [`crate::tools::Handlers::register`]. They are the thin layer between S2's
//! pipeline and `ori-memory`: **no policy of their own, no event of their own,
//! no text of their own.** `ori_memory::scope::ScopeEnforcer::authorize` decides
//! and logs a request (AICD §25), and `ori_memory::retrieval::Retrieval` turns
//! the grant into a rendered package. The handler builds the request from the
//! engine's binding, asks, and returns the package's own rendering.
//!
//! | Tool | Takes | Returns |
//! |---|---|---|
//! | `aicd_context` | `ticket_id` | the bounded context package for that ticket (`Retrieval::assemble`) |
//! | `aicd_search` | `query` | the scoped search results (`Retrieval::search`), over every source the caller's role may read |
//!
//! Both are listed to every role (`spec/API_SPEC.md` section 3, "all"), both are
//! published read-only, and neither has an output schema in phase 1.
//!
//! # One call
//!
//! S2 has already run every check and appended `mcp.tool_called` when a handler
//! is reached ([`crate::tools::ToolHandler`]).
//!
//! ```mermaid
//! sequenceDiagram
//!   participant A as agent
//!   participant S as S2: Server::receive
//!   participant H as ContextHandler or SearchHandler
//!   participant E as ScopeEnforcer::authorize
//!   participant R as Retrieval
//!   participant L as the product's log
//!   A->>S: tools/call aicd_context or aicd_search
//!   S->>S: checks 0 to 7
//!   S->>L: append mcp.tool_called
//!   S->>H: call(db, BoundCall, arguments)
//!   H->>H: read lock on MemoryInputs, the product and cap checks
//!   H->>E: authorize(db, call.at, principal, request)
//!   alt a scope refusal
//!     E->>L: append memory.access_refused
//!     E-->>H: ScopeError
//!     H-->>S: HandlerError::Refused with the code and the section
//!     S->>L: append mcp.tool_failed
//!     S-->>A: an error result
//!   else granted
//!     E-->>H: Authorization
//!     H->>R: assemble or search
//!     alt a refusal or a fault of the retrieval
//!       R-->>H: RetrievalError
//!       H-->>S: Refused (invalid_search_query) or Port Unavailable
//!       S->>L: append mcp.tool_failed
//!       S-->>A: an error result
//!     else a package
//!       R-->>H: the package
//!       H-->>S: Ok, one text block: the package's render()
//!       S-->>A: that result as it is
//!     end
//!   end
//! ```
//!
//! # The decisions this module carries
//!
//! **The inputs besides the log are shared, and the engine keeps them current.**
//! [`crate::tools::memory::MemoryInputs`] is what the tools read besides the
//! product's log: the stage, the code map and when it was built, the freshness
//! tracker and the package bounds. The engine (ORI-T-0070) builds it per product
//! and keeps it current; every connection's handlers share one
//! ([`crate::tools::memory::SharedMemoryInputs`]). A handler takes the read lock
//! **for the whole call**, because `Retrieval` borrows the map and the tracker,
//! and it checks three things before it asks anything:
//!
//! - the lock is not poisoned: a poisoned lock fails closed, as `Unavailable`;
//! - the inputs were built for **this** product. `CodeMap` and `FreshnessTracker`
//!   carry no product, and the retrieval stamps their candidates with the
//!   authorization's product, so a wiring mistake could serve another product's
//!   map. A mismatch is `Unavailable`;
//! - `config.cap_bytes` is at most
//!   [`crate::tools::memory::MAX_PACKAGE_REPLY_BYTES`] (262 144). A rendered
//!   package is one text block on one reply line and must stay well under W0's
//!   1 MiB line. A larger cap is `Unavailable`.
//!
//! **Lock order.** The engine never holds the write lock while it waits for the
//! database writer, because a handler holds the read lock while it holds the
//! writer: a write lock held across a wait for the writer is a deadlock with a
//! handler that holds the writer and waits for the read lock.
//!
//! **The principal and the request are the binding's, never the call's.** The
//! principal is `Principal::authenticated(Actor::Agent(identity))`, with the
//! **binding's** ticket as the assignment when it has one
//! (`crate::tools::BoundCall::ticket_id` is the agent's own argument, and it is
//! `None` for a search, so a coder's assignment taken from it would refuse every
//! coder search `no_assignment`). The request carries exactly one claim, the
//! binding's role: the enforcer takes the role from the log, and S2 applied the
//! tool scopes by the binding's role, so a binding that disagrees with the log
//! is refused `claim_contradicted` before the declared scope, the ticket or any
//! source is read, instead of being served under the log's role. This is a
//! deliberate use of `ori_memory::scope::Claims`, which elsewhere carries the
//! request's own word: **from these tools, `claim_contradicted { role }` means
//! the engine's binding and the log disagree.** The time is S2's `call.at`, the
//! engine's clock read once at call entry. Slice S7 (`aicd_evidence`) imports the
//! principal and the request unchanged.
//!
//! **A scope refusal is the handler's `Refused`, logged or not.** The code is
//! `RefusalReason::code` and the section is the error's own (AICD §25), through
//! the accessors, never by matching the variants, for both a refusal the
//! enforcer logged and one it could not. No `Display`, no `detail` and no store
//! error's text is copied into a reply or an event.
//!
//! **A retrieval error maps by who can cause it.**
//!
//! | `RetrievalError` | Becomes | Why |
//! |---|---|---|
//! | `InvalidSearchQuery` | `Refused`, `invalid_search_query` | the one an agent can cause: a NUL passes S2's schema and the retrieval refuses it after the grant ("fix the call") |
//! | `NotAContextAuthorization`, `NotASearchAuthorization`, `NotAnEvidenceAuthorization` | `Refused`, `not_a_context_authorization`, `not_a_search_authorization`, `not_an_evidence_authorization` | refusals of the retrieval itself (AICD §25); unreachable, because the handler builds the query |
//! | `ProductMismatch`, `SeamBroken` | `Refused`, `product_mismatch`, `seam_broken` | refusals of the retrieval itself (AICD §25); unreachable after S2's check 0 and the product check above |
//! | `InvalidConfig`, `CapTooSmall` | `Unavailable` | the engine's configuration, not the agent's call: "retry later", for the operator to fix |
//! | `Oplog` | `Unavailable` | infrastructure, or a persistent malformed record: a persistent one reads the same as a transient one |
//! | `Index` | `Unavailable` | infrastructure |
//! | any other (the enum is `#[non_exhaustive]`) | `Unavailable` | fail closed |
//!
//! **The reply is the package's rendering and nothing else.** One text block, no
//! `structuredContent`, no `isError`. A package or an item is never serialized by
//! `Debug` or serde, because an item's candidate and admitted values are
//! unbounded and unscrubbed; `render()` is the bounded, scrubbed form. A search
//! whose query is only whitespace returns the package with its empty-query note,
//! which is **not** an error. The rendering writes the query's length in
//! characters and never its text.
//!
//! # What is recorded
//!
//! **A handler's own code appends nothing**, and holds no state across calls. The
//! enforcer it calls appends exactly one `memory.access_refused` per scope
//! refusal, so these tools add **at most one row per admitted call**. S2 bounds
//! its own rows in one window to twice the window's limit plus one row per
//! connection (`crate::server`'s module documentation) and asks each slice to
//! state its handler's rows: with this one, **a window holds at most three times
//! the window's limit plus one row per connection.**
//!
//! | Outcome | Rows, in order |
//! |---|---|
//! | `aicd_context` granted | `mcp.tool_called` (ticket column: the argument) |
//! | `aicd_search` granted | `mcp.tool_called` (ticket column: the binding's ticket, or none) |
//! | a scope refusal | `mcp.tool_called`, then `memory.access_refused` (the enforcer's: the same `at` as `mcp.tool_called`, ticket column none), then `mcp.tool_failed` (`failure: refused`, the code, AICD §25) |
//! | a scope refusal the enforcer could not record | `mcp.tool_called`, then `mcp.tool_failed` (refused, the code). If that append fails too, `unrecorded` names `mcp.tool_failed` and the connection closes |
//! | unavailable inputs, a configuration fault, an infrastructure fault | `mcp.tool_called`, then `mcp.tool_failed` (`failure: unavailable`) |
//! | a `Refused` retrieval error after a grant (`invalid_search_query`) | `mcp.tool_called`, then `mcp.tool_failed` (refused, the code) |
//!
//! **How the rows link.** `memory.access_refused` directly follows
//! `mcp.tool_called` and carries the same time (`call.at`), but no `session_id`
//! and no `called_seq`. `mcp.tool_failed` names `mcp.tool_called` by `called_seq`
//! and carries its own clock reading, the second of the call (taken after the
//! handler returned; the clock may step back, so it is not promised to be
//! greater). The single writer the engine holds for one `receive` keeps the
//! three contiguous.
//!
//! # Limits
//!
//! - **An accepted gap in the audit.** `HandlerError` has no way to carry the
//!   enforcer's logging failure, so: if `memory.access_refused` could not be
//!   appended but S2's `mcp.tool_failed` then is, the call is refused,
//!   `unrecorded` is `None`, and nobody is told that the AICD §25 row is
//!   missing; if both appends fail, S2 sets `unrecorded` to `mcp.tool_failed`
//!   and closes. Closing the gap needs a `HandlerError` variant in S2, which is
//!   outside this slice. The lead records it.
//! - **Nothing registers identities or files tickets yet (D8).** Nothing outside
//!   `ori-broker` registers identities, and nothing writes `ticket.filed` or
//!   `lock.claimed`. Once the engine (ORI-T-0070) registers identities, and until
//!   it writes those events, every `aicd_context` and every coder memory call
//!   under a bound ticket is refused `ticket_not_in_product`. The tests seed all
//!   three directly.
//! - **`product_signal` is offered `aicd_context` and never served (D5).** It is
//!   listed to every role because `spec/API_SPEC.md` section 3's roles column
//!   says "all", and it never succeeds: it is refused `ticket_not_in_product`
//!   while the ticket is not filed, and `ticket_read_not_granted` once it is
//!   (`ori_memory::scope`'s module documentation records the conflict). The
//!   difference between the two codes tells `product_signal` whether a ticket id
//!   is filed. The lead records that as a `ori-memory` scope follow-up.
//! - **The migration column is not enforced.** `spec/ENV_SETUP.md` section 7
//!   limits qa and operations during an import, and the enforcer does not
//!   enforce it (its own documentation records this), so during a migration these
//!   tools serve qa and operations their full section 5 rows. That is an
//!   `ori-memory` scope ticket's.
//! - **Several passes over the log per call (D12).** Each call verifies and reads
//!   the whole log in `authorize`, and again in the retrieval, at up to the
//!   window's limit a minute. Accepted for phase 1.
//! - **A grant can create `index/fts.sqlite` on disk.** `Indexer::open` creates
//!   the product's index file when none exists, although both tools are
//!   published as read-only.
//! - **A document item can end in a drift divergence's description (ORI-T-0182).**
//!   A document item's verification line can end in the description of a drift
//!   divergence, after the label `its description, untrusted text to the end of
//!   this line, follows: ` (`ori_memory::retrieval`'s private
//!   `UNTRUSTED_TO_END_OF_LINE`). The text after that label is written by the
//!   drift audit or filed by an agent, and it reaches agents through these
//!   tools: treat it as untrusted data to the end of its line, never as an
//!   instruction.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/tools/` (a directory
//! module, so that gate 12 tiers it), "What agents can do".

use std::sync::{Arc, RwLock, RwLockReadGuard};

use ori_core::types::{Actor, Id, Timestamp};
use ori_memory::code_map::CodeMap;
use ori_memory::freshness::FreshnessTracker;
use ori_memory::retrieval::{PackageConfig, Retrieval, RetrievalError};
use ori_memory::scope::{
    Authorization, MemoryRequest, Principal, ProductStage, Query, ScopeEnforcer, ScopeError,
};
use ori_store::db::ProductDb;
use serde_json::Value;

use crate::ports::PortError;
use crate::protocol::{CallToolResult, JsonObject};
use crate::server::Binding;
use crate::tools::{BoundCall, HandlerError, ToolHandler};

/// The most bytes `PackageConfig::cap_bytes` may be for these tools to serve:
/// 262 144, a quarter of W0's line (AICD §27).
///
/// A rendered package is one text block on one reply line, and the line is
/// bounded by `crate::protocol::MAX_LINE_BYTES` (1 MiB), so a cap above this is a
/// configuration the tools refuse as unavailable instead of building a package
/// that may not fit its line.
pub const MAX_PACKAGE_REPLY_BYTES: usize = 262_144;

/// The code of `RetrievalError::InvalidSearchQuery`: the one retrieval refusal an
/// agent can cause (AICD §25).
pub(crate) const CODE_INVALID_SEARCH_QUERY: &str = "invalid_search_query";
/// The code of `RetrievalError::NotAContextAuthorization` (AICD §25).
pub(crate) const CODE_NOT_A_CONTEXT_AUTHORIZATION: &str = "not_a_context_authorization";
/// The code of `RetrievalError::NotASearchAuthorization` (AICD §25).
pub(crate) const CODE_NOT_A_SEARCH_AUTHORIZATION: &str = "not_a_search_authorization";
/// The code of `RetrievalError::NotAnEvidenceAuthorization` (AICD §25).
pub(crate) const CODE_NOT_AN_EVIDENCE_AUTHORIZATION: &str = "not_an_evidence_authorization";
/// The code of `RetrievalError::ProductMismatch` (AICD §25).
pub(crate) const CODE_PRODUCT_MISMATCH: &str = "product_mismatch";
/// The code of `RetrievalError::SeamBroken` (AICD §25).
pub(crate) const CODE_SEAM_BROKEN: &str = "seam_broken";

/// What was unavailable when the memory inputs' lock is poisoned. Never copied
/// into a reply or an event.
const WHAT_INPUTS: &str = "the memory inputs";
/// What was unavailable when the inputs were built for another product.
const WHAT_OTHER_PRODUCT: &str = "memory inputs of another product";
/// What was unavailable when the package configuration cannot yield a package.
const WHAT_CONFIGURATION: &str = "the package configuration";
/// What was unavailable when the operational log cannot be read.
const WHAT_OPERATIONAL_MEMORY: &str = "operational memory";
/// What was unavailable when the index cannot be opened or searched.
const WHAT_INDEX: &str = "the search index";
/// What was unavailable for a retrieval error this module does not know.
const WHAT_RETRIEVAL: &str = "retrieval";

/// The fixed sentence of a call that reaches `aicd_context` with no ticket.
const WHY_NO_TICKET: &str = "the call carries no ticket";
/// The fixed sentence of a call that reaches `aicd_search` with no text query.
const WHY_NO_QUERY: &str = "the query argument is missing or is not a string";

/// What the memory tools read besides the product's log: AICD §25.
///
/// The engine (ORI-T-0070) builds it per product and keeps it current; every
/// connection's handlers share one ([`crate::tools::memory::SharedMemoryInputs`]).
///
/// **Lock order.** A handler takes the read lock for the whole call, because
/// `Retrieval` borrows the map and the tracker, and it holds the database writer
/// while it does. The engine therefore never holds the write lock while it waits
/// for the database writer: that is a deadlock with a handler that holds the
/// writer and waits for the read lock. A poisoned lock fails closed.
///
/// `product_id` is checked against the binding's on every call, because neither
/// `CodeMap` nor `FreshnessTracker` carries a product.
///
/// # Examples
///
/// The engine registers both handlers on one shared value. An empty code map is
/// built from its public fields, since it has no `Default`.
///
/// ```no_run
/// use std::sync::{Arc, RwLock};
///
/// use ori_core::types::Id;
/// use ori_mcp::scopes::Tool;
/// use ori_mcp::tools::Handlers;
/// use ori_mcp::tools::memory::{ContextHandler, MemoryInputs, SearchHandler};
/// use ori_memory::code_map::{CodeMap, Coverage, SpecScan};
/// use ori_memory::freshness::FreshnessTracker;
/// use ori_memory::retrieval::PackageConfig;
/// use ori_memory::scope::ProductStage;
///
/// let product_id = Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("an identifier");
/// let inputs = Arc::new(RwLock::new(MemoryInputs {
///     product_id,
///     stage: ProductStage::Standard,
///     code_map: CodeMap {
///         modules: Vec::new(),
///         coverage: Coverage {
///             files_seen: 0,
///             files_parsed_clean: 0,
///             files_parsed_with_errors: 0,
///             files_skipped: Vec::new(),
///             spec_citation_scan: SpecScan::Complete,
///             spec_docs_skipped: Vec::new(),
///         },
///         spec_docs: Vec::new(),
///         spec_headings: Vec::new(),
///     },
///     code_map_built_at: None,
///     freshness: FreshnessTracker::new(),
///     config: PackageConfig::default(),
/// }));
///
/// let mut handlers = Handlers::new();
/// handlers
///     .register(Tool::Context, Box::new(ContextHandler::new(Arc::clone(&inputs))))
///     .expect("one handler per tool");
/// handlers
///     .register(Tool::Search, Box::new(SearchHandler::new(Arc::clone(&inputs))))
///     .expect("one handler per tool");
/// ```
#[derive(Clone, Debug)]
pub struct MemoryInputs {
    /// The product these inputs were built for.
    pub product_id: Id,
    /// Where the product is in its life, which the scope enforcer reads
    /// (AICD §24).
    pub stage: ProductStage,
    /// The product's code map.
    pub code_map: CodeMap,
    /// When the code map was built, or `None` when the engine says none: every
    /// code map item then says its freshness is unknown.
    pub code_map_built_at: Option<Timestamp>,
    /// The product's freshness tracker.
    pub freshness: FreshnessTracker,
    /// The bounds of a package. `cap_bytes` above
    /// [`crate::tools::memory::MAX_PACKAGE_REPLY_BYTES`] is refused as
    /// unavailable.
    pub config: PackageConfig,
}

/// The shared form of [`crate::tools::memory::MemoryInputs`], behind an `Arc` as
/// S2 shares a session's `RateWindow` (there behind a `Mutex`): AICD §25.
///
/// See [`crate::tools::memory::MemoryInputs`] for the lock order.
pub type SharedMemoryInputs = Arc<RwLock<MemoryInputs>>;

/// An `Unavailable` failure of the effect, with a fixed `what` that S2 never
/// copies into a reply or an event.
fn unavailable(what: &str) -> HandlerError {
    HandlerError::Port(PortError::Unavailable {
        what: what.to_owned(),
    })
}

/// The principal of a call: the engine's binding, never the call: AICD §25.
///
/// `Principal::authenticated(Actor::Agent(identity))`, and when the binding has
/// a ticket, that ticket as the assignment. It is always the **binding's**
/// ticket, never `crate::tools::BoundCall::ticket_id`: that field is the agent's
/// argument, and it is `None` for a search, so a coder's assignment taken from it
/// would refuse every coder search `no_assignment`.
///
/// Slice S7 (`aicd_evidence`, in `evidence.rs`) imports this unchanged.
pub(crate) fn principal(binding: &Binding) -> Principal {
    let principal = Principal::authenticated(Actor::Agent(binding.identity().clone()));
    match binding.ticket_id() {
        Some(ticket) => principal.with_assigned_ticket(ticket.clone()),
        None => principal,
    }
}

/// The request of a call: the query, and the binding's role as its one claim:
/// AICD §25.
///
/// The enforcer takes the role from the log, and S2 applied the tool scopes by
/// the binding's role. With this claim, a binding that disagrees with the log is
/// refused `claim_contradicted` before the declared scope, the ticket or any
/// source is read, instead of being served under the log's role. **From these
/// tools a `claim_contradicted { role }` means the engine's binding and the log
/// disagree.** That is a deliberate use of `ori_memory::scope::Claims`, which
/// elsewhere carries the request's own word. No other claim is made.
///
/// Slice S7 (`aicd_evidence`, in `evidence.rs`) imports this unchanged.
pub(crate) fn request(binding: &Binding, query: Query) -> MemoryRequest {
    let mut request = MemoryRequest::new(query);
    request.claims.role = Some(binding.role().as_str().to_owned());
    request
}

/// A scope refusal as the handler's `Refused`, logged or not: AICD §25.
///
/// The code is the reason's own and the section is the error's, through the
/// accessors, so both `ScopeError::Refused` and `ScopeError::RefusedUnlogged`
/// map the same. Nothing of a `Display`, a `detail` or a store error's text is
/// copied. `HandlerError` has no way to carry the enforcer's logging failure
/// (the module documentation's accepted gap).
///
/// Slice S7 (`aicd_evidence`, in `evidence.rs`) imports this unchanged.
pub(crate) fn scope_error(error: &ScopeError) -> HandlerError {
    HandlerError::Refused {
        code: error.reason().code(),
        methodology_ref: error.methodology_ref(),
    }
}

/// A retrieval error as the handler's error, by who can cause it: AICD §25.
///
/// `InvalidSearchQuery` is the only variant an agent can cause, and is a
/// `Refused` ("fix the call"). The retrieval's own refusals (a wrong kind of
/// authorization, a product mismatch, a broken seam) are `Refused` under their
/// own section, and are unreachable from these handlers. A configuration fault is
/// the engine's, and an infrastructure fault is no one's the agent can fix: both
/// are `Unavailable` ("retry later"). An `Oplog` error that persists, such as a
/// malformed record, reads the same as a transient one. The enum is
/// `#[non_exhaustive]`, so any variant this module does not know is
/// `Unavailable` too: it fails closed.
///
/// Slice S7 (`aicd_evidence`, in `evidence.rs`) imports this unchanged.
pub(crate) fn retrieval_error(error: RetrievalError) -> HandlerError {
    let reference = error.methodology_ref();
    let refused = |code: &'static str| HandlerError::Refused {
        code,
        methodology_ref: reference.clone(),
    };
    match error {
        RetrievalError::InvalidSearchQuery { .. } => refused(CODE_INVALID_SEARCH_QUERY),
        RetrievalError::NotAContextAuthorization { .. } => {
            refused(CODE_NOT_A_CONTEXT_AUTHORIZATION)
        }
        RetrievalError::NotASearchAuthorization { .. } => refused(CODE_NOT_A_SEARCH_AUTHORIZATION),
        RetrievalError::NotAnEvidenceAuthorization { .. } => {
            refused(CODE_NOT_AN_EVIDENCE_AUTHORIZATION)
        }
        RetrievalError::ProductMismatch { .. } => refused(CODE_PRODUCT_MISMATCH),
        RetrievalError::SeamBroken { .. } => refused(CODE_SEAM_BROKEN),
        RetrievalError::InvalidConfig { .. } | RetrievalError::CapTooSmall { .. } => {
            unavailable(WHAT_CONFIGURATION)
        }
        RetrievalError::Oplog(_) => unavailable(WHAT_OPERATIONAL_MEMORY),
        RetrievalError::Index(_) => unavailable(WHAT_INDEX),
        _ => unavailable(WHAT_RETRIEVAL),
    }
}

/// The read lock of the shared inputs, held for the whole call, after the three
/// checks of the module documentation: the lock is not poisoned, the inputs were
/// built for the binding's product, and the cap is within
/// [`crate::tools::memory::MAX_PACKAGE_REPLY_BYTES`].
fn read_inputs<'a>(
    shared: &'a SharedMemoryInputs,
    binding: &Binding,
) -> Result<RwLockReadGuard<'a, MemoryInputs>, HandlerError> {
    let inputs = shared.read().map_err(|_| unavailable(WHAT_INPUTS))?;
    if &inputs.product_id != binding.product_id() {
        return Err(unavailable(WHAT_OTHER_PRODUCT));
    }
    if inputs.config.cap_bytes > MAX_PACKAGE_REPLY_BYTES {
        return Err(unavailable(WHAT_CONFIGURATION));
    }
    Ok(inputs)
}

/// Asks the scope enforcer for `query` on behalf of the call's binding, at the
/// call's time: AICD §25. The enforcer appends `memory.access_refused` when it
/// refuses.
fn authorize(
    inputs: &MemoryInputs,
    db: &mut ProductDb,
    call: &BoundCall<'_>,
    query: Query,
) -> Result<Authorization, HandlerError> {
    ScopeEnforcer::new(inputs.stage)
        .authorize(
            db,
            call.at,
            &principal(call.binding),
            &request(call.binding, query),
        )
        .map_err(|error| scope_error(&error))
}

/// The retrieval over the shared inputs, for the product `db` is the database
/// of: AICD §25.
fn retrieval<'a>(inputs: &'a MemoryInputs, db: &ProductDb) -> Result<Retrieval<'a>, HandlerError> {
    Retrieval::new(
        db,
        &inputs.code_map,
        inputs.code_map_built_at,
        &inputs.freshness,
        inputs.config,
    )
    .map_err(retrieval_error)
}

/// The handler of `aicd_context(ticket_id)`: the bounded context package of one
/// ticket, scoped to the caller's role (AICD §25).
///
/// The ticket is the call's `ticket_id` argument, which S2's checks 6 and 7
/// guarantee: for a coder it is the bound ticket. Whether the caller may read it
/// is the enforcer's decision. Appends no event of its own and holds no state
/// across calls. See the module documentation for the rows a call leaves and the
/// limits, and [`crate::tools::memory::MemoryInputs`] for a registration example.
#[derive(Clone, Debug)]
pub struct ContextHandler {
    inputs: SharedMemoryInputs,
}

impl ContextHandler {
    /// A handler over the product's shared memory inputs: AICD §25.
    #[must_use]
    pub fn new(inputs: SharedMemoryInputs) -> Self {
        Self { inputs }
    }
}

impl ToolHandler for ContextHandler {
    /// `aicd_context`: authorize the ticket's context, assemble the package,
    /// return its rendering as one text block: AICD §25.
    ///
    /// # Errors
    ///
    /// `Refused` for a scope refusal (AICD §25), or for a refusal of the
    /// retrieval itself; `Port(Unavailable)` for inputs that are poisoned, built
    /// for another product or over the reply cap, a configuration fault or an
    /// infrastructure fault; `Port(Invalid)` for a call with no ticket, which S2's
    /// checks make unreachable.
    fn call(
        &mut self,
        db: &mut ProductDb,
        call: &BoundCall<'_>,
        _arguments: JsonObject,
    ) -> Result<CallToolResult, HandlerError> {
        let inputs = read_inputs(&self.inputs, call.binding)?;
        let Some(ticket_id) = call.ticket_id else {
            return Err(HandlerError::Port(PortError::Invalid {
                field: "ticket_id",
                why: WHY_NO_TICKET.to_owned(),
            }));
        };
        let authorization = authorize(
            &inputs,
            db,
            call,
            Query::Context {
                ticket_id: ticket_id.clone(),
            },
        )?;
        let package = retrieval(&inputs, db)?
            .assemble(db, &authorization)
            .map_err(retrieval_error)?;
        Ok(CallToolResult::text(package.render()))
    }
}

/// The handler of `aicd_search(query)`: scoped search over every source the
/// caller's role may read (AICD §25).
///
/// The query is the call's `query` argument. The request names no source, so the
/// enforcer grants every source the reader's role may read. The reply is the
/// package's rendering, which writes the query's length in characters and never
/// its text; a query of only whitespace returns the package with its empty-query
/// note, which is not an error. Appends no event of its own and holds no state
/// across calls. See the module documentation for the rows a call leaves and the
/// limits, and [`crate::tools::memory::MemoryInputs`] for a registration example.
#[derive(Clone, Debug)]
pub struct SearchHandler {
    inputs: SharedMemoryInputs,
}

impl SearchHandler {
    /// A handler over the product's shared memory inputs: AICD §25.
    #[must_use]
    pub fn new(inputs: SharedMemoryInputs) -> Self {
        Self { inputs }
    }
}

impl ToolHandler for SearchHandler {
    /// `aicd_search`: authorize the search over every readable source, run it,
    /// return the package's rendering as one text block: AICD §25.
    ///
    /// # Errors
    ///
    /// `Refused` for a scope refusal (AICD §25), or for a refusal of the retrieval
    /// itself, including `invalid_search_query` for a query holding a NUL
    /// character; `Port(Unavailable)` for inputs that are poisoned, built for
    /// another product or over the reply cap, a configuration fault or an
    /// infrastructure fault; `Port(Invalid)` for a call whose query is not a
    /// string, which S2's schema makes unreachable.
    fn call(
        &mut self,
        db: &mut ProductDb,
        call: &BoundCall<'_>,
        mut arguments: JsonObject,
    ) -> Result<CallToolResult, HandlerError> {
        let inputs = read_inputs(&self.inputs, call.binding)?;
        let Some(Value::String(text)) = arguments.remove("query") else {
            return Err(HandlerError::Port(PortError::Invalid {
                field: "query",
                why: WHY_NO_QUERY.to_owned(),
            }));
        };
        let authorization = authorize(
            &inputs,
            db,
            call,
            Query::Search {
                text,
                sources: Vec::new(),
            },
        )?;
        let package = retrieval(&inputs, db)?
            .search(db, &authorization)
            .map_err(retrieval_error)?;
        Ok(CallToolResult::text(package.render()))
    }
}

/// The fixtures of this module's tests, which slice S7 (`aicd_evidence`) imports
/// for its own: a scratch product database, the events the scope enforcer reads,
/// and a server built with S2's public API over both memory handlers.
#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, RwLock};

    use ori_core::types::{Actor, Id, Role, Timestamp};
    use ori_memory::code_map::{CodeMap, Coverage, SpecScan};
    use ori_memory::freshness::FreshnessTracker;
    use ori_memory::retrieval::PackageConfig;
    use ori_memory::scope::ProductStage;
    use ori_store::db::ProductDb;
    use ori_store::event_log::{Event, EventLog};
    use serde_json::{Value, json};

    use super::{ContextHandler, MemoryInputs, SearchHandler, SharedMemoryInputs};
    use crate::ports::{Clock, PortError, SessionDesk};
    use crate::protocol::{Message, Notification, Request, RequestId};
    use crate::scopes::Tool;
    use crate::server::{Binding, RateWindow, Reply, Server, ServerConfig};
    use crate::tools::Handlers;

    /// A temporary directory that removes itself: the self-cleaning pattern
    /// `ori-memory`'s `scope.rs` tests use, with no dev-dependency.
    pub(crate) struct Scratch {
        pub(crate) path: PathBuf,
    }

    impl Scratch {
        pub(crate) fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0181-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a fresh scratch directory can be created");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A valid ULID, distinguished by `label`.
    pub(crate) fn id(label: &str) -> Id {
        let mut safe = String::with_capacity(25);
        for ch in label.chars().take(25) {
            let mapped = match ch.to_ascii_uppercase() {
                upper @ ('0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z') => {
                    upper
                }
                'I' | 'L' => '1',
                'O' => '0',
                'U' => 'V',
                _ => '0',
            };
            safe.push(mapped);
        }
        Id::parse(&format!("0{safe:0>25}"))
            .expect("a sanitized, zero-padded, '0'-prefixed 26-character string always parses")
    }

    pub(crate) fn product_a() -> Id {
        id("PRODUCTA")
    }

    pub(crate) fn product_b() -> Id {
        id("PRODUCTB")
    }

    pub(crate) fn session() -> Id {
        id("SESSION")
    }

    pub(crate) fn identity() -> Id {
        id("AGENT")
    }

    /// The ticket the tests file and claim.
    pub(crate) fn ticket_t() -> Id {
        id("TICKETT")
    }

    /// The clock's first reading: the `at` of the first call a world makes.
    pub(crate) const START: i64 = 1_000_000;

    /// The time of every fixture event, before any call.
    const FIXTURE_AT: i64 = 100;

    // -------------------------------------------------------------------
    // Fixture events, shaped as `ori-memory`'s `scope.rs` test helpers shape
    // them (`register_in`, `file_ticket` and `claim`), because the enforcer
    // reads the log and only the log.
    // -------------------------------------------------------------------

    /// Appends one fixture event by the operator, at [`FIXTURE_AT`].
    pub(crate) fn append(
        db: &mut ProductDb,
        product: &Id,
        kind: &str,
        ticket: Option<&Id>,
        payload: String,
    ) -> Event {
        EventLog::append(
            db.connection(),
            product.clone(),
            Timestamp::from_millis(FIXTURE_AT),
            Actor::Human(id("OPERATOR")),
            kind,
            ticket.cloned(),
            payload,
        )
        .expect("a fixture event appends")
    }

    /// `identity.created`, ticket column `None`, the payload field for field as
    /// `scope.rs`'s `register_in` writes it.
    pub(crate) fn register_in(
        db: &mut ProductDb,
        product: &Id,
        identity: &Id,
        role: Role,
        named_product: &Id,
    ) {
        let payload = format!(
            "{{\"id\":\"{identity}\",\"product_id\":\"{named_product}\",\"role\":\"{}\",\"model\":\"model-a\",\"family\":\"family-a\",\"runtime\":\"acp\"}}",
            role.as_str()
        );
        append(db, product, "identity.created", None, payload);
    }

    /// `ticket.filed`, ticket column `Some(ticket)`, the payload as `scope.rs`'s
    /// `file_ticket` writes it.
    pub(crate) fn file_ticket(db: &mut ProductDb, product: &Id, ticket: &Id) {
        append(
            db,
            product,
            "ticket.filed",
            Some(ticket),
            r#"{"category":"auto","kind":"feature"}"#.to_owned(),
        );
    }

    /// `lock.claimed`, ticket column `Some(ticket)`, the payload as `scope.rs`'s
    /// `claim` writes it. The enforcer reads only the ticket column, so a lock
    /// event with none is refused `lock_record_unattributed`.
    pub(crate) fn claim(db: &mut ProductDb, product: &Id, ticket: &Id, module: &str) {
        let payload = json!({ "module": module }).to_string();
        append(db, product, "lock.claimed", Some(ticket), payload);
    }

    // -------------------------------------------------------------------
    // The fakes S2's own tests use, built from S2's public ports.
    // -------------------------------------------------------------------

    /// A clock that reads `START`, then steps by `step` on every reading, so that
    /// the call's time and the later reading of a failure differ.
    struct SteppingClock {
        next: i64,
        step: i64,
    }

    impl Clock for SteppingClock {
        fn now(&mut self) -> Timestamp {
            let now = self.next;
            self.next += self.step;
            Timestamp::from_millis(now)
        }
    }

    /// A session desk that finds every session active.
    struct ActiveDesk;

    impl SessionDesk for ActiveDesk {
        fn is_active(&mut self, _db: &mut ProductDb, binding: &Binding) -> Result<bool, PortError> {
            assert_eq!(binding.session_id(), &session(), "the engine's own binding");
            Ok(true)
        }
    }

    // -------------------------------------------------------------------
    // The inputs and the world.
    // -------------------------------------------------------------------

    /// An empty code map, from its public fields: it has no `Default`.
    pub(crate) fn empty_code_map() -> CodeMap {
        CodeMap {
            modules: Vec::new(),
            coverage: Coverage {
                files_seen: 0,
                files_parsed_clean: 0,
                files_parsed_with_errors: 0,
                files_skipped: Vec::new(),
                spec_citation_scan: SpecScan::Complete,
                spec_docs_skipped: Vec::new(),
            },
            spec_docs: Vec::new(),
            spec_headings: Vec::new(),
        }
    }

    /// The inputs of a product at the standard stage: an empty code map, a new
    /// tracker and the default package bounds.
    pub(crate) fn inputs_for(product: &Id) -> MemoryInputs {
        MemoryInputs {
            product_id: product.clone(),
            stage: ProductStage::Standard,
            code_map: empty_code_map(),
            code_map_built_at: None,
            freshness: FreshnessTracker::new(),
            config: PackageConfig::default(),
        }
    }

    /// Both memory handlers on one shared value, registered as the engine
    /// registers them.
    pub(crate) fn memory_handlers(inputs: &SharedMemoryInputs) -> Handlers {
        let mut handlers = Handlers::new();
        handlers
            .register(
                Tool::Context,
                Box::new(ContextHandler::new(Arc::clone(inputs))),
            )
            .expect("one handler per tool");
        handlers
            .register(
                Tool::Search,
                Box::new(SearchHandler::new(Arc::clone(inputs))),
            )
            .expect("one handler per tool");
        handlers
    }

    /// One product's database in a scratch directory, a server bound to a
    /// session of it, and the shared inputs both handlers read.
    pub(crate) struct World {
        pub(crate) db: ProductDb,
        pub(crate) server: Server,
        pub(crate) inputs: SharedMemoryInputs,
        pub(crate) product: Id,
        next_request: i64,
        _scratch: Scratch,
    }

    /// How a world is built: the role and ticket the engine binds the session
    /// to, and the role the log registers the identity under.
    pub(crate) struct Setup {
        bound_role: Role,
        bound_ticket: Option<Id>,
        registered_as: Role,
    }

    impl Setup {
        /// A session bound to `role` and `ticket`, the identity registered in the
        /// log under the same role.
        pub(crate) const fn new(role: Role, ticket: Option<Id>) -> Self {
            Self {
                bound_role: role,
                bound_ticket: ticket,
                registered_as: role,
            }
        }

        /// The log registers the identity under another role than the binding.
        pub(crate) const fn registered_as(mut self, role: Role) -> Self {
            self.registered_as = role;
            self
        }

        /// A world with both memory handlers registered, handshaken.
        pub(crate) fn build(self, label: &str) -> World {
            self.build_with(label, memory_handlers)
        }

        /// A world whose handlers are built from the shared inputs, handshaken.
        pub(crate) fn build_with(
            self,
            label: &str,
            handlers: impl FnOnce(&SharedMemoryInputs) -> Handlers,
        ) -> World {
            let scratch = Scratch::new(label);
            let product = product_a();
            let mut db =
                ProductDb::open(&scratch.path, product.as_str(), Timestamp::from_millis(1))
                    .expect("a fresh product database opens");
            register_in(&mut db, &product, &identity(), self.registered_as, &product);
            let inputs: SharedMemoryInputs = Arc::new(RwLock::new(inputs_for(&product)));
            let server = Server::new(
                Binding::new(
                    session(),
                    identity(),
                    self.bound_role,
                    product.clone(),
                    self.bound_ticket,
                ),
                ServerConfig::default(),
                Arc::new(Mutex::new(RateWindow::new(120))),
                Box::new(SteppingClock {
                    next: START,
                    step: 1,
                }),
                Box::new(ActiveDesk),
                handlers(&inputs),
            );
            let mut world = World {
                db,
                server,
                inputs,
                product,
                next_request: 1,
                _scratch: scratch,
            };
            world.handshake();
            world
        }
    }

    impl World {
        fn send(&mut self, message: Message) -> Reply {
            self.server.receive(&mut self.db, Ok(message))
        }

        fn handshake(&mut self) {
            let reply = self.send(Message::Request(Request {
                id: RequestId::Number(0),
                method: "initialize".to_owned(),
                params: Some(json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name": "test-agent", "version": "1"},
                })),
            }));
            assert!(reply.message.is_some());
            let reply = self.send(Message::Notification(Notification::without_params(
                "notifications/initialized",
            )));
            assert!(reply.message.is_none());
        }

        fn next_request_id(&mut self) -> RequestId {
            self.next_request += 1;
            RequestId::Number(self.next_request)
        }

        /// One `tools/call` through `Server::receive`.
        pub(crate) fn call(&mut self, name: &str, arguments: Value) -> Reply {
            let id = self.next_request_id();
            self.send(Message::Request(Request {
                id,
                method: "tools/call".to_owned(),
                params: Some(json!({"name": name, "arguments": arguments})),
            }))
        }

        /// The names `tools/list` returns, in order.
        pub(crate) fn listed_tools(&mut self) -> Vec<String> {
            let id = self.next_request_id();
            let reply = self.send(Message::Request(Request {
                id,
                method: "tools/list".to_owned(),
                params: None,
            }));
            wire(&reply)["result"]["tools"]
                .as_array()
                .expect("a tools array")
                .iter()
                .map(|tool| tool["name"].as_str().expect("a name").to_owned())
                .collect()
        }

        /// Every event of the log, in order.
        pub(crate) fn events(&mut self) -> Vec<Event> {
            EventLog::read_range(self.db.connection(), 1, u64::MAX).expect("the log reads")
        }

        /// The `seq` of the last event, `0` for an empty log.
        pub(crate) fn tip(&mut self) -> u64 {
            self.events().last().map_or(0, Event::seq)
        }

        /// The events appended after `tip`, in order: the rows a call left.
        pub(crate) fn rows_since(&mut self, tip: u64) -> Vec<Event> {
            self.events()
                .into_iter()
                .filter(|event| event.seq() > tip)
                .collect()
        }
    }

    /// The kinds of `rows`, in order.
    pub(crate) fn kinds(rows: &[Event]) -> Vec<String> {
        rows.iter().map(|event| event.kind().to_owned()).collect()
    }

    /// The reply's message as the JSON it goes out as.
    pub(crate) fn wire(reply: &Reply) -> Value {
        serde_json::to_value(reply.message.as_ref().expect("a reply message"))
            .expect("a message serializes")
    }

    /// The text of the reply's one text block.
    pub(crate) fn text_of(reply: &Reply) -> String {
        let message = wire(reply);
        let blocks = message["result"]["content"]
            .as_array()
            .expect("content blocks");
        assert_eq!(blocks.len(), 1, "one text block: {message}");
        assert_eq!(blocks[0]["type"], "text");
        blocks[0]["text"].as_str().expect("text").to_owned()
    }

    /// One event's payload as JSON.
    pub(crate) fn payload(event: &Event) -> Value {
        serde_json::from_str(event.payload()).expect("a JSON payload")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ori_core::error::MethodologyRef;
    use ori_core::types::{Role, Timestamp};
    use ori_memory::barrier::{
        BarrierConfig, FieldName, NewReport, NewReportField, RecordKind, submit_report,
    };
    use ori_memory::indexer::IndexerError;
    use ori_memory::oplog::OplogError;
    use ori_memory::retrieval::{PackageConfig, Retrieval};
    use ori_memory::scope::{Access, Filter, Source, grant};
    use ori_store::db::ProductDb;
    use serde_json::{Value, json};

    use super::test_support::{
        START, Setup, World, claim, file_ticket, id, identity, inputs_for, kinds, payload,
        product_b, session, text_of, ticket_t, wire,
    };
    use super::*;
    use crate::scopes::Tool;
    use crate::server::{AuditEvent, Reply};
    use crate::tools::Handlers;

    /// The `structuredContent` of an error result that refused under `reason`,
    /// section 25.
    fn refused_object(reason: &str) -> Value {
        json!({
            "failure": "refused",
            "reason": reason,
            "methodology_ref": {"section": 25, "subsection": null},
        })
    }

    /// A text only the tests' queries carry: the planted marker.
    const MARKER: &str = "ZEBRAQUARTZ0181";

    /// The marker as a record spells it: a search lower-cases the query before it
    /// matches a field's lower-cased text, and a record's text is shown as it was
    /// filed, so the query's own spelling is never in a rendering.
    const MARKER_AS_FILED: &str = "zebraquartz0181";

    /// A coder bound to the ticket T, registered as a coder, with T filed and
    /// claiming a module.
    fn coder_world(label: &str) -> World {
        let mut world = Setup::new(Role::Coder, Some(ticket_t())).build(label);
        let product = world.product.clone();
        file_ticket(&mut world.db, &product, &ticket_t());
        claim(&mut world.db, &product, &ticket_t(), "crates/m");
        world
    }

    /// A lead bound to no ticket, registered as a lead.
    fn lead_world(label: &str) -> World {
        Setup::new(Role::Lead, None).build(label)
    }

    /// Files one record of the lead's readable kind whose summary is `text`.
    fn file_record(world: &mut World, label: &str, ticket: Option<&Id>, text: &str) {
        submit_report(
            &mut world.db,
            Timestamp::from_millis(200),
            Actor::Agent(id("AUTHOR")),
            NewReport {
                record_id: id(label),
                kind: RecordKind::ClosingReport,
                ticket_id: ticket.cloned(),
                fields: vec![NewReportField {
                    name: FieldName::parse("summary").expect("a field name"),
                    raw_text: text.to_owned(),
                    evidence_id: id(&format!("EV{label}")),
                }],
            },
            &BarrierConfig::default(),
        )
        .expect("a fixture record is submitted");
    }

    fn context_call(world: &mut World) -> Reply {
        world.call("aicd_context", json!({"ticket_id": ticket_t().as_str()}))
    }

    fn search_call(world: &mut World, query: &str) -> Reply {
        world.call("aicd_search", json!({ "query": query }))
    }

    /// Asserts a failure's reply: an error result whose `structuredContent` is
    /// exactly `expected`, with the text block that same object serialized.
    fn assert_failure(reply: &Reply, expected: &Value) {
        let message = wire(reply);
        assert_eq!(message["result"]["isError"], true, "{message}");
        assert_eq!(
            message["result"]["structuredContent"], *expected,
            "{message}"
        );
        let text: Value = serde_json::from_str(&text_of(reply)).expect("JSON text");
        assert_eq!(text, *expected);
    }

    // -------------------------------------------------------------------
    // 1. A coder's context
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_a_coder_gets_the_context_of_its_own_ticket() {
        let mut world = coder_world("context-coder");
        let product = world.product.clone();
        file_record(
            &mut world,
            "RECT",
            Some(&ticket_t()),
            "closed the widget work",
        );
        let tip = world.tip();

        let reply = context_call(&mut world);
        assert!(!reply.close && reply.unrecorded.is_none());

        let message = wire(&reply);
        let result = &message["result"];
        assert!(result.get("isError").is_none(), "{message}");
        assert!(result.get("structuredContent").is_none(), "{message}");
        let text = text_of(&reply);

        // The log gained exactly the call's own row.
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called"]);
        assert_eq!(rows[0].ticket_id(), Some(&ticket_t()), "the argument");

        // The package, recomputed after the call as a reader of the fixtures
        // would: the principal and the request written out by hand. A grant and
        // the retrieval append nothing, so the log's state is the call's.
        let principal =
            Principal::authenticated(Actor::Agent(identity())).with_assigned_ticket(ticket_t());
        let request = MemoryRequest::new(Query::Context {
            ticket_id: ticket_t(),
        });
        let inputs = world.inputs.read().expect("the inputs");
        let authorization = ScopeEnforcer::new(inputs.stage)
            .authorize(
                &mut world.db,
                Timestamp::from_millis(START),
                &principal,
                &request,
            )
            .expect("the recomputation is granted");
        let package = Retrieval::new(
            &world.db,
            &inputs.code_map,
            inputs.code_map_built_at,
            &inputs.freshness,
            inputs.config,
        )
        .expect("a retrieval")
        .assemble(&mut world.db, &authorization)
        .expect("a package");
        assert_eq!(text, package.render(), "byte for byte");
        assert!(text.starts_with("context package\n"), "{text}");
        assert!(text.contains(&format!("product: {product}\n")), "{text}");
        assert!(text.contains("focus: crates/m\n"), "{text}");
        assert!(text.contains("items: 1\n"), "{text}");
        drop(inputs);
        assert_eq!(world.tip(), tip + 1, "the recomputation appended nothing");
    }

    // -------------------------------------------------------------------
    // 2. A scope refusal
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_a_context_refusal_is_recorded_by_the_enforcer_and_answered_as_an_error_result() {
        // The identity is registered and the ticket is not filed.
        let mut world = Setup::new(Role::Coder, Some(ticket_t())).build("context-refused");
        let tip = world.tip();

        let reply = context_call(&mut world);
        assert!(!reply.close && reply.unrecorded.is_none());
        assert_failure(&reply, &refused_object("ticket_not_in_product"));

        let rows = world.rows_since(tip);
        assert_eq!(
            kinds(&rows),
            [
                "mcp.tool_called",
                "memory.access_refused",
                "mcp.tool_failed"
            ]
        );
        // The enforcer's row carries the call's time, and no ticket column.
        assert_eq!(rows[1].at(), rows[0].at());
        assert_eq!(rows[0].at(), Timestamp::from_millis(START));
        assert_eq!(rows[1].ticket_id(), None);
        assert_eq!(rows[1].actor(), rows[0].actor());
        // S2's failure row names the call by its seq, with its own later time.
        assert!(rows[2].at() > rows[0].at());
        let failed = payload(&rows[2]);
        assert_eq!(failed["called_seq"], rows[0].seq());
        assert_eq!(failed["failure"], "refused");
        assert_eq!(failed["reason"], "ticket_not_in_product");
        assert_eq!(rows[2].ticket_id(), Some(&ticket_t()));
        // The enforcer's payload says what was asked, by code.
        let refusal = payload(&rows[1]);
        assert_eq!(refusal["reason"]["code"], "ticket_not_in_product");
        assert_eq!(refusal["request"]["kind"], "context");
        assert_eq!(refusal["request"]["claims"]["role"], "coder");
        assert_eq!(refusal["methodology_ref"]["section"], 25);
    }

    // -------------------------------------------------------------------
    // 3. A binding the log contradicts
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_a_binding_the_log_contradicts_is_refused_as_a_claim() {
        // The binding says lead; the log registers the identity as qa.
        let mut world = Setup::new(Role::Lead, None)
            .registered_as(Role::Qa)
            .build("claim-contradicted");
        let tip = world.tip();

        let reply = search_call(&mut world, "anything");
        assert_failure(&reply, &refused_object("claim_contradicted"));

        let rows = world.rows_since(tip);
        assert_eq!(
            kinds(&rows),
            [
                "mcp.tool_called",
                "memory.access_refused",
                "mcp.tool_failed"
            ]
        );
        let refusal = payload(&rows[1]);
        assert_eq!(refusal["reason"]["code"], "claim_contradicted");
        assert_eq!(refusal["request"]["claims"]["role"], "lead");
    }

    // -------------------------------------------------------------------
    // 4. product_signal
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_product_signal_is_offered_aicd_context_and_never_served() {
        let mut world = Setup::new(Role::ProductSignal, None).build("product-signal");
        assert!(
            world
                .listed_tools()
                .iter()
                .any(|name| name == "aicd_context"),
            "the roles column says all"
        );

        // The ticket is not filed.
        let reply = context_call(&mut world);
        assert_failure(&reply, &refused_object("ticket_not_in_product"));

        // The ticket is filed: still refused, under another code.
        let product = world.product.clone();
        file_ticket(&mut world.db, &product, &ticket_t());
        let reply = context_call(&mut world);
        assert_failure(&reply, &refused_object("ticket_read_not_granted"));
    }

    // -------------------------------------------------------------------
    // 5. A search, and never the query
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_a_search_answers_with_the_rendered_package_and_never_the_query() {
        let mut world = lead_world("search-lead");
        file_record(
            &mut world,
            "RECS",
            None,
            &format!("the {MARKER_AS_FILED} widget"),
        );
        let tip = world.tip();

        let query = format!("  {MARKER}  ");
        let reply = search_call(&mut world, &query);
        assert!(!reply.close && reply.unrecorded.is_none());
        let message = wire(&reply);
        assert!(message["result"].get("isError").is_none(), "{message}");
        assert!(message["result"].get("structuredContent").is_none());
        let text = text_of(&reply);

        // The log gained the call's own row, and no ticket column: the lead is
        // bound to none.
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called"]);
        assert_eq!(rows[0].ticket_id(), None);

        // The package, recomputed with the principal and the request by hand.
        let principal = Principal::authenticated(Actor::Agent(identity()));
        let request = MemoryRequest::new(Query::Search {
            text: query.clone(),
            sources: Vec::new(),
        });
        let inputs = world.inputs.read().expect("the inputs");
        let authorization = ScopeEnforcer::new(inputs.stage)
            .authorize(
                &mut world.db,
                Timestamp::from_millis(START),
                &principal,
                &request,
            )
            .expect("the recomputation is granted");
        let package = Retrieval::new(
            &world.db,
            &inputs.code_map,
            inputs.code_map_built_at,
            &inputs.freshness,
            inputs.config,
        )
        .expect("a retrieval")
        .search(&mut world.db, &authorization)
        .expect("a package");
        assert_eq!(text, package.render(), "byte for byte");

        // It found the record, says the query's length, and has none of its
        // text; its sources line is every source the lead may read.
        assert!(text.starts_with("search results\n"), "{text}");
        assert!(text.contains("items: 1\n"), "the record half: {text}");
        assert!(text.contains(MARKER_AS_FILED), "the record's own text");
        assert!(
            text.contains(&format!("query: {} characters\n", query.chars().count())),
            "{text}"
        );
        assert!(!text.contains(MARKER), "the query's text: {text}");
        let readable: Vec<&str> = Source::ALL
            .iter()
            .filter(|source| **source != Source::RawEvidence)
            .filter(|source| grant(Role::Lead, **source) == Access::Granted(Filter::Whole))
            .map(|source| source.as_str())
            .collect();
        assert!(
            text.contains(&format!("sources allowed: {}\n", readable.join(", "))),
            "{text}"
        );
        drop(inputs);

        // A refused search carrying the marker: the coder is bound to no ticket,
        // so the log names none, and the enforcer's row holds the query's length
        // and not its text.
        let mut coder = Setup::new(Role::Coder, None).build("search-refused");
        let tip = coder.tip();
        let reply = search_call(&mut coder, &query);
        assert_failure(&reply, &refused_object("no_assignment"));
        assert!(!wire(&reply).to_string().contains(MARKER));
        let rows = coder.rows_since(tip);
        assert_eq!(
            kinds(&rows),
            [
                "mcp.tool_called",
                "memory.access_refused",
                "mcp.tool_failed"
            ]
        );
        let refusal = payload(&rows[1]);
        assert_eq!(refusal["request"]["kind"], "search");
        assert_eq!(refusal["request"]["query_chars"], query.chars().count());
        for row in &rows {
            assert!(
                !row.payload().contains(MARKER),
                "a row holds the query: {}",
                row.payload()
            );
        }
    }

    // -------------------------------------------------------------------
    // 6. A coder's assignment is the binding's
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_a_coder_search_takes_its_assignment_from_the_binding() {
        // Bound to T, with T filed and claimed: granted.
        let mut world = coder_world("search-coder");
        let tip = world.tip();
        let reply = search_call(&mut world, "widget");
        assert!(wire(&reply)["result"].get("isError").is_none());
        assert!(text_of(&reply).starts_with("search results\n"));
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called"]);
        assert_eq!(
            rows[0].ticket_id(),
            Some(&ticket_t()),
            "the binding's ticket"
        );

        // Bound to none: refused for want of an assignment.
        let mut unassigned = Setup::new(Role::Coder, None).build("search-coder-unassigned");
        let reply = search_call(&mut unassigned, "widget");
        assert_failure(&reply, &refused_object("no_assignment"));
    }

    // -------------------------------------------------------------------
    // 7. A NUL in the query
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_a_nul_in_the_query_is_refused_after_the_grant() {
        let mut world = lead_world("search-nul");
        let tip = world.tip();

        // S2's schema counts characters and has no pattern: a NUL passes it.
        let reply = search_call(&mut world, "a\u{0}b");
        assert_failure(&reply, &refused_object("invalid_search_query"));

        // The grant appended nothing, so there is no `memory.access_refused`.
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called", "mcp.tool_failed"]);
        assert_eq!(payload(&rows[1])["reason"], "invalid_search_query");
    }

    // -------------------------------------------------------------------
    // 8 and 9. Inputs that fail closed
    // -------------------------------------------------------------------

    /// Asserts a call that failed closed: `{"failure":"unavailable"}`, the call's
    /// row and the failure's, and none of `what` in the reply or any row.
    fn assert_unavailable(world: &mut World, tip: u64, reply: &Reply, what: &[&str]) {
        assert_failure(reply, &json!({"failure": "unavailable"}));
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called", "mcp.tool_failed"]);
        assert_eq!(payload(&rows[1])["failure"], "unavailable");
        for text in what {
            assert!(!wire(reply).to_string().contains(text), "the reply: {text}");
            for row in &rows {
                assert!(!row.payload().contains(text), "a row: {text}");
            }
        }
    }

    #[test]
    fn ori_t_0181_poisoned_memory_inputs_fail_closed() {
        let mut world = lead_world("poisoned");
        let tip = world.tip();
        let shared = Arc::clone(&world.inputs);
        let poisoner = std::thread::spawn(move || {
            let _guard = shared.write().expect("the write lock");
            panic!("poisoning the memory inputs on purpose");
        });
        assert!(poisoner.join().is_err(), "the thread panicked");
        assert!(world.inputs.read().is_err(), "the lock is poisoned");

        let reply = search_call(&mut world, "widget");
        assert_unavailable(
            &mut world,
            tip,
            &reply,
            &["the memory inputs", "memory inputs"],
        );
    }

    #[test]
    fn ori_t_0181_inputs_of_another_product_or_an_oversized_cap_fail_closed() {
        let mut world = lead_world("bad-inputs");

        // Built for another product.
        let tip = world.tip();
        *world.inputs.write().expect("the inputs") = inputs_for(&product_b());
        let reply = search_call(&mut world, "widget");
        assert_unavailable(
            &mut world,
            tip,
            &reply,
            &["memory inputs of another product"],
        );

        // A cap over the reply bound.
        let product = world.product.clone();
        let mut oversized = inputs_for(&product);
        oversized.config = PackageConfig {
            cap_bytes: MAX_PACKAGE_REPLY_BYTES + 1,
            ..PackageConfig::default()
        };
        *world.inputs.write().expect("the inputs") = oversized;
        let tip = world.tip();
        let reply = search_call(&mut world, "widget");
        assert_unavailable(&mut world, tip, &reply, &["the package configuration"]);

        // The bound itself is served.
        let mut at_the_bound = inputs_for(&product);
        at_the_bound.config = PackageConfig {
            cap_bytes: MAX_PACKAGE_REPLY_BYTES,
            ..PackageConfig::default()
        };
        *world.inputs.write().expect("the inputs") = at_the_bound;
        let reply = search_call(&mut world, "widget");
        assert!(wire(&reply)["result"].get("isError").is_none());
        assert!(text_of(&reply).contains(&format!("cap: {MAX_PACKAGE_REPLY_BYTES} bytes\n")));
    }

    // -------------------------------------------------------------------
    // 10 and 11. A refusal the enforcer could not record
    // -------------------------------------------------------------------

    /// A handler that makes the log read-only, then asks `aicd_context`, as S2's
    /// own tests make a log that accepts nothing.
    struct ReadOnlyThenContext {
        inner: ContextHandler,
    }

    impl ToolHandler for ReadOnlyThenContext {
        fn call(
            &mut self,
            db: &mut ProductDb,
            call: &BoundCall<'_>,
            arguments: JsonObject,
        ) -> Result<CallToolResult, HandlerError> {
            db.connection()
                .execute_batch("PRAGMA query_only = ON;")
                .expect("the connection goes read-only");
            self.inner.call(db, call, arguments)
        }
    }

    #[test]
    fn ori_t_0181_a_refusal_the_enforcer_could_not_record_is_still_refused() {
        let mut world =
            Setup::new(Role::Coder, Some(ticket_t())).build_with("unrecorded-refusal", |inputs| {
                let mut handlers = Handlers::new();
                handlers
                    .register(
                        Tool::Context,
                        Box::new(ReadOnlyThenContext {
                            inner: ContextHandler::new(Arc::clone(inputs)),
                        }),
                    )
                    .expect("registers");
                handlers
            });
        let tip = world.tip();

        let reply = context_call(&mut world);
        // Still refused, with the same object as a refusal that was recorded.
        assert_failure(&reply, &refused_object("ticket_not_in_product"));
        // Neither the enforcer's row nor S2's failure row could be appended.
        let failure = reply.unrecorded.expect("reported");
        assert_eq!(failure.event, AuditEvent::Failed);
        assert!(reply.close);

        world
            .db
            .connection()
            .execute_batch("PRAGMA query_only = OFF;")
            .expect("the connection can write again");
        assert_eq!(kinds(&world.rows_since(tip)), ["mcp.tool_called"]);
    }

    #[test]
    fn ori_t_0181_a_refusal_whose_enforcer_row_alone_is_lost_is_refused_unannounced() {
        let mut world = Setup::new(Role::Coder, Some(ticket_t())).build("lost-enforcer-row");
        world
            .db
            .connection()
            .execute_batch(
                "CREATE TRIGGER forbid_access_refused BEFORE INSERT ON events \
                 WHEN NEW.kind = 'memory.access_refused' \
                 BEGIN SELECT RAISE(ABORT, 'a test forbids this kind'); END;",
            )
            .expect("the trigger is created");
        let tip = world.tip();

        let reply = context_call(&mut world);
        assert_failure(&reply, &refused_object("ticket_not_in_product"));
        // The accepted gap: the AICD §25 row is missing and nobody is told.
        assert_eq!(
            kinds(&world.rows_since(tip)),
            ["mcp.tool_called", "mcp.tool_failed"]
        );
        assert!(reply.unrecorded.is_none());
        assert!(!reply.close);
    }

    // -------------------------------------------------------------------
    // 12. The retrieval error map
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_every_retrieval_error_maps_by_who_can_cause_it() {
        let refused = |error: RetrievalError, code: &'static str| {
            let reference = error.methodology_ref();
            assert_eq!(
                reference,
                MethodologyRef {
                    section: 25,
                    subsection: None
                }
            );
            assert!(reference.resolves());
            assert_eq!(
                retrieval_error(error),
                HandlerError::Refused {
                    code,
                    methodology_ref: reference
                }
            );
        };
        refused(
            RetrievalError::InvalidSearchQuery {
                why: "contains a NUL character",
                bytes: 3,
            },
            "invalid_search_query",
        );
        refused(
            RetrievalError::NotAContextAuthorization { found: "search" },
            "not_a_context_authorization",
        );
        refused(
            RetrievalError::NotASearchAuthorization { found: "context" },
            "not_a_search_authorization",
        );
        refused(
            RetrievalError::NotAnEvidenceAuthorization { found: "context" },
            "not_an_evidence_authorization",
        );
        refused(
            RetrievalError::ProductMismatch { what: "a link" },
            "product_mismatch",
        );
        refused(
            RetrievalError::SeamBroken { detail: "a seam" },
            "seam_broken",
        );

        let unavailable = |error: RetrievalError, what: &str| {
            assert_eq!(
                retrieval_error(error),
                HandlerError::Port(PortError::Unavailable {
                    what: what.to_owned()
                })
            );
        };
        unavailable(
            RetrievalError::InvalidConfig {
                detail: "item_bytes is below MIN_ITEM_BYTES",
            },
            "the package configuration",
        );
        unavailable(
            RetrievalError::CapTooSmall {
                cap_bytes: 1,
                needed_bytes: 2,
            },
            "the package configuration",
        );
        unavailable(
            RetrievalError::Oplog(OplogError::Inconsistent {
                detail: "a record with no event".to_owned(),
            }),
            "operational memory",
        );
        unavailable(
            RetrievalError::Index(IndexerError::Locked {
                path: std::path::PathBuf::from("index/fts.sqlite"),
            }),
            "the search index",
        );
    }

    // -------------------------------------------------------------------
    // 13. The fallbacks never panic
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0181_the_handlers_fallbacks_never_panic() {
        let mut world = lead_world("fallbacks");
        let binding = Binding::new(
            session(),
            identity(),
            Role::Lead,
            world.product.clone(),
            None,
        );
        let call = BoundCall {
            binding: &binding,
            tool: Tool::Context,
            called_seq: 1,
            at: Timestamp::from_millis(START),
            ticket_id: None,
        };

        let mut context = ContextHandler::new(Arc::clone(&world.inputs));
        match context.call(&mut world.db, &call, JsonObject::new()) {
            Err(HandlerError::Port(PortError::Invalid { field, why })) => {
                assert_eq!(field, "ticket_id");
                assert_eq!(why, WHY_NO_TICKET);
            }
            other => panic!("expected a ticket_id fault, got {other:?}"),
        }

        let mut search = SearchHandler::new(Arc::clone(&world.inputs));
        let call = BoundCall {
            tool: Tool::Search,
            ..call
        };
        for arguments in [
            json!({"query": 7}),
            json!({"query": {MARKER: 1}}),
            json!({"query": null}),
            json!({}),
        ] {
            let Value::Object(arguments) = arguments else {
                panic!("an object");
            };
            match search.call(&mut world.db, &call, arguments) {
                Err(HandlerError::Port(PortError::Invalid { field, why })) => {
                    assert_eq!(field, "query");
                    assert_eq!(why, WHY_NO_QUERY, "a fixed sentence");
                }
                other => panic!("expected a query fault, got {other:?}"),
            }
        }
    }
}
