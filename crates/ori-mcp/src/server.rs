//! The agent-facing MCP server core: `spec/API_SPEC.md` section 3, AICD §8,
//! AICD §17.
//!
//! One synchronous, transport-agnostic core that decides, records and
//! dispatches every call an agent makes. It holds **no database and no
//! transport**: the engine passes the product's one `ProductDb` writer with
//! every item it hands to [`crate::server::Server::receive`], and reads and
//! writes the connection itself. It builds no tool: each tool's effect is a
//! [`crate::tools::ToolHandler`] a later slice registers
//! ([`crate::tools::Handlers`]).
//!
//! # The binding, never from a message
//!
//! [`crate::server::Binding`] is the session a connection serves, **as the
//! engine registered it**: the session, the agent identity, the role, the
//! product and the ticket, if any. The engine builds it when it registers the
//! session and hands it to [`crate::server::Server::new`]; it is fixed for the
//! server's life. No message, argument or `_meta` field reaches it
//! (`spec/SECURITY_NOTES.md`, "Boundary 2 in phase 1"; E-0013). Every event this
//! server appends has `Actor::Agent(identity)`, never `System` (`spec/DATA_MODEL.md`
//! section 4: "`system` is allowed only for scheduled triggers, watchers, and the
//! engine's recovery after a restart").
//!
//! **What this rests on.** Until condition 6 of E-0013 is met, which is a later
//! `ori-memory` ticket that adds a role lookup to the scope enforcer so that
//! tools other than the memory tools do not rest on the engine's role alone, the
//! role checks of this server (checks 5 and 7 below) rest on the engine's
//! binding alone. A binding the engine got wrong is a role this server believes.
//! Meeting condition 6 is what lifts that dependency.
//!
//! # The connection
//!
//! The lifecycle is [`crate::protocol::ConnectionState`], used strictly. Every
//! received message goes through `ConnectionState::check` first, then
//! `KnownRequest::decode` or `KnownNotification::decode`, and their
//! `ProtocolError`s are answered as W0's answering rule says: an error with an id
//! answers that request; one without an id, raised on input that could not be
//! read as a request, is answered once with an id-less error; one raised on a
//! notification is never answered. **None of these is a call and none is
//! recorded**, and neither is a `tools/call` that W0 refuses this way.
//!
//! | Message | Answer |
//! |---|---|
//! | `initialize` | The negotiated version, the `tools` capability only (`listChanged` false), and the server's info; then the state is `Initialized`. A refused `initialize` leaves it `Fresh`. |
//! | `notifications/initialized` | The state is `Ready`. |
//! | `ping` | An empty result, in every state. |
//! | `tools/list` | The caller's tools and nothing else (below). |
//! | `tools/call` | The pipeline (below). |
//! | `notifications/cancelled` | Ignored, below. |
//! | A response from the client | Ignored: the server sends no request in phase 1, so a response answers nothing and changes nothing. |
//!
//! **Cancellation is ignored.** Requests are processed in arrival order, and each
//! is answered before the next is read, so a cancellation always names a request
//! already answered. A call is never cancelled once it has been read: its effect
//! and its `mcp.tool_called` stand. MCP allows a receiver to ignore a
//! cancellation then.
//!
//! **A `tools/call` that carries `params.task` is processed as a normal call.**
//! MCP 2025-11-25's tasks page: "Receivers that do not declare the task
//! capability for a request type MUST process requests of that type normally,
//! ignoring any task-augmentation metadata if present." The server declares no
//! `tasks` capability, so it reads `task` only to drop it: it is never recorded
//! or echoed, and the reply is a `CallToolResult`, never a task result. The
//! documentation on `CallToolParams.task` in `protocol.rs` says the opposite
//! ("refuse rather than silently ignore it"), which is wrong on this point and is
//! corrected by a separate ticket, because that file is another slice's.
//!
//! # `tools/list` is the caller's tools and nothing else
//!
//! One page, no cursor: the definitions of every tool, in [`crate::scopes::Tool::ALL`]
//! order, for which **all three** hold: the tool is built in phase 1, the role
//! holds it ([`crate::scopes::ToolScopes::permits`]), and a handler is
//! registered. A `cursor` parameter is refused as invalid (-32602), since the
//! server issues none. `tools/list` is not a call and is not recorded.
//!
//! **Proxied tools are slice S6's.** S6 edits this file at two documented
//! points: where `list` appends its tools, and where `call` classifies the
//! requested name, where it adds its branch before `Tool::from_wire_name` (the
//! pipeline's check 3). At the append point S6 must keep two things. The server
//! declares `listChanged: false`, so S6 either snapshots the proxied tools at
//! connection start or switches the capability to `true` and sends
//! `notifications/tools/list_changed`. And S6 drops or refuses a proxied name
//! that breaks MCP's tool-name rules (1 to 128 characters of letters, digits,
//! underscore, hyphen and dot).
//!
//! # The pipeline of a call
//!
//! A **call** is a `tools/call` request that passed W0's `check` and `decode`, so
//! the connection is `Ready`. At entry the engine's clock is read once: that is
//! the call's `at`. Every call ends in exactly one of a refusal before any effect,
//! or `mcp.tool_called` followed by the effect.
//!
//! ```mermaid
//! flowchart TD
//!   entry["call entry: read the clock once, charge the session's call budget"] --> c0{"0: the database is this product's?"}
//!   c0 -- "no" --> r0["product_database_mismatch, AICD §26: not recorded, close"]
//!   c0 -- "yes" --> c1{"1: the budget admits it?"}
//!   c1 -- "no" --> r1["rate_limited, AICD §22: record, close"]
//!   c1 -- "yes" --> c2{"2: the session is active?"}
//!   c2 -- "no, or unreadable" --> r2["session_not_active or session_state_unavailable: record, close"]
//!   c2 -- "yes" --> c3{"3: one of Ori's tools?"}
//!   c3 -- "no" --> r3["unknown_tool: record"]
//!   c3 -- "yes" --> c4{"4: in phase 1 and handled?"}
//!   c4 -- "no" --> r4["tool_not_available: record"]
//!   c4 -- "yes" --> c5{"5: the role holds it?"}
//!   c5 -- "no" --> r5["tool_not_permitted_for_role: record"]
//!   c5 -- "yes" --> c6{"6: within the byte cap and the schema?"}
//!   c6 -- "no" --> r6["invalid_arguments, AICD §27: record, an error result"]
//!   c6 -- "yes" --> c7{"7: the ticket rule?"}
//!   c7 -- "no" --> r7["claim_contradicts_session or ticket_not_assigned: record, an error result"]
//!   c7 -- "yes" --> c8{"8: mcp.tool_called appended?"}
//!   c8 -- "no" --> r8["call_not_recorded, AICD §17: attempt mcp.tool_refused, close"]
//!   c8 -- "yes" --> c9["9: the handler's effect"]
//!   c9 --> ok["the handler's result as it is"]
//!   c9 --> fail["mcp.tool_failed, then an error result"]
//! ```
//!
//! | # | Check | Refusal code | Section | Reply | After it |
//! |---|---|---|---|---|---|
//! | 0 | the database is the bound product's | `product_database_mismatch` | AICD §26 | JSON-RPC -32603 | **not recorded** (never write into another product's log); `unrecorded` set; close |
//! | 1 | the session's [`crate::server::RateWindow`] admits the call | `rate_limited` | AICD §22 | JSON-RPC -32010 | record; close |
//! | 2 | `SessionDesk::is_active` is `Ok(true)` | `session_not_active`; `session_state_unavailable` when it errs (fail closed) | AICD §17 | JSON-RPC -32011; -32603 | record; close |
//! | 3 | the name is one of Ori's tools | `unknown_tool` | AICD §17 | JSON-RPC -32602 | record |
//! | 4 | the tool is in phase 1 and has a handler | `tool_not_available` | AICD §17 | JSON-RPC -32602 | record |
//! | 5 | `ToolScopes::authorize` | `tool_not_permitted_for_role` | AICD §17 | JSON-RPC -32602 | record |
//! | 6 | the serialized arguments fit the cap, then the schema | `invalid_arguments`, with `field` | AICD §27 | an `isError` result | record |
//! | 7 | the ticket rule | `claim_contradicts_session` or `ticket_not_assigned` | AICD §25; AICD §7 | an `isError` result | record |
//! | 8 | append `mcp.tool_called` | `call_not_recorded` | AICD §17 | JSON-RPC -32603 | attempt `mcp.tool_refused`; `unrecorded` set; close |
//!
//! The codes -32010 and -32011 are Ori's own, from the implementation-defined
//! range -32000 to -32099, chosen to avoid -32042, which is MCP's. Checks 1 and 2
//! come first because they reveal nothing and cost no more than one read. Closing
//! on them bounds the log's growth to the window's limit plus one row per
//! connection. The endpoint bounds the connection rate (prerequisite PE1).
//!
//! **Refusals 0 to 5 and 8 are JSON-RPC error responses** with the request's id,
//! the code above, a message that is the refusal's `Display`, and `data` of
//! `{"reason", "methodology_ref"}`. **Refusals 6 and 7 are `CallToolResult`s**
//! with `isError` true: MCP's tools page puts a tool's own failures in `isError`
//! results, and a bad argument is the tool's. Their `structuredContent` is
//! `{"reason", "methodology_ref"}` (and `"field"` for `invalid_arguments`), and
//! their single text block is that `structuredContent` serialized. **No reply to
//! a refusal echoes agent free text**: not the tool name, not an argument key,
//! not a free-text value. A validated, canonical identifier is not free text, so
//! the ticket refusals may name the ticket in canonical form. This covers this
//! module's refusals; W0's decode replies are W0's.
//!
//! # The ticket rule (check 7)
//!
//! For a tool whose schema has a `ticket_id` property, after the schema passed,
//! comparing **parsed** identifiers (so a lower-case spelling of the bound ticket
//! passes). The rule follows `spec/ENV_SETUP.md`'s Tickets column, where only the
//! coder's row is restricted to its own ticket ("Read own; plan and report"):
//!
//! - role `coder` with a bound ticket, and the argument differs from it:
//!   `claim_contradicts_session`;
//! - role `coder` with no bound ticket: `ticket_not_assigned`, because a coder
//!   session is always issued for a ticket;
//! - any other role: a bound ticket does not restrict, and the argument passes
//!   this check. `AgentSession.ticket_id` is nullable for unattended sessions
//!   (`spec/DATA_MODEL.md` section 2), and an attended lead or assistant may be
//!   bound to a ticket without being limited to it.
//!
//! What a non-coder role may do with a ticket belongs to the downstream control:
//!
//! | Tool | The control that decides |
//! |---|---|
//! | `aicd_context` | the memory scope enforcer (slice S3) |
//! | `aicd_plan_submit` | the coder rule above |
//! | `aicd_report` | the report path (slice S4). **It has no ticket check today**: S4 must check that the ticket exists in this product, which is a gap S4 closes |
//! | `aicd_escalate` | the escalation desk (slice S9) |
//! | `aicd_ticket_upgrade` | the ticket desk |
//!
//! # The call budget
//!
//! MCP's tools page requires a server to rate-limit tool invocations. This is the
//! phase 1 answer. The window is fixed at 60 000 ms on the engine's clock and
//! held in the session's shared [`crate::server::RateWindow`], one per session
//! and shared by every connection of that session, so reconnecting does not reset
//! it. A call is charged at entry, before check 1, once per call, refused or not.
//! A new window starts at the first call, when 60 000 ms have passed since the
//! window began, or when the clock steps back. Under the engine's default of 120,
//! the 121st call of a window is refused `rate_limited`, recorded, and the
//! connection closed; a new connection in the same window has its first call
//! refused and closed the same way. A poisoned lock counts as an exhausted
//! window.
//!
//! # What is recorded
//!
//! Only `tools/call` produces events: `initialize`, `ping`, `tools/list`,
//! notifications, responses and unreadable input change nothing and reveal
//! nothing beyond what the binding already permits. The payloads are JSON objects
//! of identifiers, fixed codes and lengths, **never agent free text**
//! (`spec/API_SPEC.md` section 3):
//!
//! - `mcp.tool_called`: `{"session_id", "tool", "arguments_bytes"}`;
//! - `mcp.tool_refused`: `{"session_id", "tool", "name_chars", "reason", "methodology_ref", "field"}`,
//!   where `tool` is the wire name or null when the name is not one of Ori's
//!   tools, `name_chars` is present only when `tool` is null, and `field` only for
//!   `invalid_arguments`;
//! - `mcp.tool_failed`: `{"session_id", "tool", "called_seq", "failure", "reason", "methodology_ref", "field"}`,
//!   the last three only when the failure mapping below gives them a value.
//!
//! All three carry the product, the call's `at` (a failure takes its own),
//! `Actor::Agent(identity)` and a ticket column: for `mcp.tool_called` and
//! `mcp.tool_failed` of a tool with a `ticket_id` argument, the validated
//! canonical argument; otherwise, and for every `mcp.tool_refused`, the binding's
//! ticket or none. A `methodology_ref` is always written as
//! `{"section", "subsection"}` from the reference itself, by one helper used for
//! events, error data and `structuredContent` alike.
//!
//! **When an append fails**, the call is still refused and the reply is the same;
//! `Reply::unrecorded` carries the failure; and the connection closes, because a
//! log that stopped accepting rows ends the session's connection, not only the
//! call. That is both halves of `ori-memory`'s `RefusedUnlogged`: the request is
//! refused, and the logging failure goes to the caller. A call whose
//! `mcp.tool_called` cannot be appended is refused `call_not_recorded`, so no
//! effect happens without its record.
//!
//! # After `mcp.tool_called`
//!
//! A handler that returns an `Ok` result that is not an error is answered as it
//! is. A handler that returns an error, or `Ok` with `isError`, gets `mcp.tool_failed`
//! appended (with the clock read a second time), and the reply is an error
//! **result** whose `structuredContent` is chosen by this table and whose single
//! text block is that `structuredContent` serialized (a handler's own error
//! result is passed through as it is):
//!
//! | The handler returned | `failure` | `reason` | `methodology_ref` |
//! |---|---|---|---|
//! | `HandlerError::Refused { code, methodology_ref }` | `refused` | `code` | always |
//! | `Port(PortError::Refused(e))`, with `e.methodology_ref()` and `e.code()` | `refused` | `e.code()` | always |
//! | `Port(PortError::Refused(e))`, a reference and no code | `refused` | `refused` (a fixed fallback) | the reference |
//! | `Port(PortError::Refused(e))`, no reference | `invalid` | none | none |
//! | `Port(PortError::Invalid { field, .. })` | `invalid` | none | none, and `field` (a constant) is added; `why` is never copied |
//! | `Port(PortError::Unavailable { .. })` | `unavailable` | none | none |
//! | `Ok(result)` with `isError` | `error_result` | none | none |
//!
//! A refusal inside the effect (the memory scope enforcer, the evidence lockout,
//! `E_SCOPE_LOCKED`, `E_UPGRADE_ONLY`) is therefore `mcp.tool_called` then
//! `mcp.tool_failed`, and its reply is an error result, not a JSON-RPC error. A
//! reference that does not resolve is never written: it is an implementation
//! defect and is reported as `invalid`. If the `mcp.tool_failed` append fails, the
//! reply is unchanged, `Reply::unrecorded` carries the failure, and the connection
//! closes.
//!
//! # How the engine drives it
//!
//! The engine holds the one `ProductDb` writer (`crates/ori-store/src/db.rs`'s
//! single-writer note), so the server holds none: it passes the handle to
//! [`crate::server::Server::receive`] with every item. **The engine's rule: its
//! per-connection reader reads a whole line off the writer task, and then takes
//! the writer for one `receive(db, item)`, never across a read.**
//! [`crate::server::Server::serve`] holds the writer across blocking reads, so it
//! is for tests and single-session use only, and it drops `Reply::unrecorded`,
//! which the engine reads from `receive`.
//!
//! The server is `Send` (all its trait objects are), so the engine can build it on
//! one thread and hand it to the product's writer task. It is synchronous: no
//! async runtime is in the workspace.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/server.rs`, "What agents
//! can do".

use std::fmt;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

use ori_core::error::MethodologyRef;
use ori_core::types::{Actor, Id, Role, Timestamp};
use ori_store::db::ProductDb;
use ori_store::event_log::EventLog;
use serde_json::{Value, json};

use crate::ports::{Clock, PortError, SessionDesk};
use crate::protocol::{
    CallToolParams, CallToolResult, ConnectionState, Direction, ErrorCode, ErrorObject,
    ErrorResponse, FrameError, Implementation, InitializeParams, InitializeResult, JsonObject,
    KnownNotification, KnownRequest, LineReader, ListToolsParams, ListToolsResult, Message,
    Notification, ProtocolError, ReceiveError, Request, RequestId, ResultResponse,
    SUPPORTED_PROTOCOL_VERSIONS, ServerCapabilities, ToolsCapability, Transition,
    negotiate_version, write_message,
};
use crate::scopes::{Caller, Tool, ToolRefusal, ToolScopes};
use crate::tools::{
    self, BoundCall, HandlerError, Handlers, TICKET_ARGUMENT, WHY_NOT_AN_IDENTIFIER,
    WHY_NOT_AN_OBJECT,
};

/// The JSON-RPC code of a call refused `rate_limited`: Ori's own, from the
/// implementation-defined range -32000 to -32099, chosen to avoid -32042, which
/// MCP uses.
pub const RATE_LIMITED_CODE: i64 = -32010;

/// The JSON-RPC code of a call refused because its session is not active: Ori's
/// own, from the same range as [`RATE_LIMITED_CODE`].
pub const SESSION_NOT_ACTIVE_CODE: i64 = -32011;

/// The length of the call budget's window: 60 000 ms on the engine's clock.
pub const RATE_WINDOW_MILLIS: i64 = 60_000;

/// The engine's default call budget: 120 calls per session per window.
pub const DEFAULT_MAX_CALLS_PER_MINUTE: u32 = 120;

/// The default of [`ServerConfig::max_arguments_bytes`]: 512 KiB of serialized
/// arguments.
pub const DEFAULT_MAX_ARGUMENTS_BYTES: usize = 524_288;

/// The `why` of an argument object over the byte cap; its `field` is `""`.
pub const WHY_ARGUMENTS_TOO_LARGE: &str = "arguments too large";

/// The kind of the event that records an allowed call, appended before its
/// effect.
pub const EVENT_TOOL_CALLED: &str = "mcp.tool_called";

/// The kind of the event that records a refused call.
pub const EVENT_TOOL_REFUSED: &str = "mcp.tool_refused";

/// The kind of the event that records an allowed call whose effect failed.
pub const EVENT_TOOL_FAILED: &str = "mcp.tool_failed";

/// The reason of a failure that a refusal inside the effect gave no code for.
const FALLBACK_REFUSAL_CODE: &str = "refused";

/// AICD §17, "Autonomy tiers and the permission model".
const PERMISSIONS_SECTION: u8 = 17;
/// AICD §22, "Failure modes and safeguards".
const SAFEGUARDS_SECTION: u8 = 22;
/// AICD §26, "Operating several products with one team".
const PRODUCTS_SECTION: u8 = 26;
/// AICD §27, "Security and compliance in depth".
const SECURITY_SECTION: u8 = 27;

/// The detail of an unrecorded refusal of a database that is not the bound
/// product's: nothing was written, by design.
const DETAIL_FOREIGN_DATABASE: &str = "the refusal was not recorded: the database is not the session's product's, and nothing is \
     written to another product's log";

/// A reference to a whole methodology section.
const fn whole_section(section: u8) -> MethodologyRef {
    MethodologyRef {
        section,
        subsection: None,
    }
}

/// A methodology reference as every event, error and `structuredContent` writes
/// it: `{"section", "subsection"}`. The one place it is written (AICD §17).
fn methodology_ref_json(reference: &MethodologyRef) -> Value {
    json!({
        "section": reference.section,
        "subsection": reference.subsection,
    })
}

// ---------------------------------------------------------------------------
// The binding
// ---------------------------------------------------------------------------

/// The session a connection serves, as the engine registered it: AICD §17.
///
/// The engine builds it when it registers the session and hands it to
/// [`Server::new`]; it is fixed for the server's life. **No message, argument or
/// `_meta` field reaches it** (`spec/SECURITY_NOTES.md`, "Boundary 2 in phase 1";
/// E-0013). `identity` is the agent identity, and every event the server appends
/// has `Actor::Agent(identity)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    session_id: Id,
    identity: Id,
    role: Role,
    product_id: Id,
    ticket_id: Option<Id>,
}

impl Binding {
    /// A binding of this session, agent identity, role, product and ticket.
    ///
    /// `ticket_id` is `None` for a session with no ticket, such as an
    /// unattended one (`spec/DATA_MODEL.md` section 2).
    #[must_use]
    pub const fn new(
        session_id: Id,
        identity: Id,
        role: Role,
        product_id: Id,
        ticket_id: Option<Id>,
    ) -> Self {
        Self {
            session_id,
            identity,
            role,
            product_id,
            ticket_id,
        }
    }

    /// The session the connection serves.
    #[must_use]
    pub const fn session_id(&self) -> &Id {
        &self.session_id
    }

    /// The agent identity of the session.
    #[must_use]
    pub const fn identity(&self) -> &Id {
        &self.identity
    }

    /// The role of the session.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The product the session belongs to.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The ticket the session was issued for, if it has one.
    #[must_use]
    pub const fn ticket_id(&self) -> Option<&Id> {
        self.ticket_id.as_ref()
    }

    /// The [`Caller`] this binding makes, for the desks and for slice S6:
    /// the identity, role and product, and nothing an agent sent.
    #[must_use]
    pub fn caller(&self) -> Caller {
        Caller::new(self.identity.clone(), self.role, self.product_id.clone())
    }
}

// ---------------------------------------------------------------------------
// Configuration and the call budget
// ---------------------------------------------------------------------------

/// The server's limits: AICD §27.
///
/// `max_arguments_bytes` is the most serialized bytes of `arguments` a call may
/// carry. It must stay below [`crate::protocol::MAX_LINE_BYTES`] (1 MiB) minus the
/// request envelope, because a line over that cap ends the connection before the
/// server sees it. Every maximum in the catalogue was chosen so that the largest
/// schema-valid argument object of every tool fits under the **default**, 512 KiB
/// (524 288 bytes), counted at 6 bytes per character; a smaller cap refuses
/// schema-valid calls, and a larger one is above what any schema allows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    /// The most serialized bytes of arguments a call may carry.
    pub max_arguments_bytes: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_arguments_bytes: DEFAULT_MAX_ARGUMENTS_BYTES,
        }
    }
}

/// One session's call budget, shared by every connection of that session:
/// AICD §22.
///
/// The engine creates one when it registers the session (its default is
/// [`DEFAULT_MAX_CALLS_PER_MINUTE`]) and passes a clone of the `Arc` to every
/// connection's [`Server`], so that reconnecting does not reset the window. A
/// fixed window of [`RATE_WINDOW_MILLIS`] on the engine's clock.
#[derive(Debug)]
pub struct RateWindow {
    max_calls_per_minute: u32,
    window_start: Option<Timestamp>,
    calls_in_window: u32,
}

impl RateWindow {
    /// A budget of this many calls per window, with no window started.
    #[must_use]
    pub const fn new(max_calls_per_minute: u32) -> Self {
        Self {
            max_calls_per_minute,
            window_start: None,
            calls_in_window: 0,
        }
    }

    /// Charges one call made at `now`, and says whether the budget admits it.
    ///
    /// A new window starts at the first call, when `now` is at or past the
    /// window's start plus [`RATE_WINDOW_MILLIS`], or when `now` is before the
    /// window's start (a clock that stepped back). Signed, saturating
    /// arithmetic throughout. A refused call is charged too, so a session that
    /// keeps calling stays refused until the window rolls.
    fn charge(&mut self, now: Timestamp) -> bool {
        let starts_new = match self.window_start {
            None => true,
            Some(start) => {
                let started = start.millis();
                now.millis() < started || now.millis() >= started.saturating_add(RATE_WINDOW_MILLIS)
            }
        };
        if starts_new {
            self.window_start = Some(now);
            self.calls_in_window = 0;
        }
        self.calls_in_window = self.calls_in_window.saturating_add(1);
        self.calls_in_window <= self.max_calls_per_minute
    }
}

// ---------------------------------------------------------------------------
// The refusals this module raises
// ---------------------------------------------------------------------------

/// A refusal of a call: AICD §17.
///
/// Every variant carries a methodology section through
/// [`CallRefusal::methodology_ref`], and the type has no variant without one
/// (`CLAUDE.md` rule 9, criterion ORI-P1-033). The refusals of
/// [`crate::scopes::ToolRefusal`] ride in [`CallRefusal::Tool`]. No variant
/// carries agent free text: `field` is a pointer built only from schema property
/// names and array indices.
///
/// The `Display` is written by hand, as ruling R20 allows until `thiserror` is
/// adopted; converting it is mechanical, each arm becoming an `#[error]`
/// attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallRefusal {
    /// A refusal of `scopes.rs`: the role, a ticket, or the session's state.
    Tool(ToolRefusal),
    /// The database passed in is not the bound product's. AICD §26.
    ProductDatabaseMismatch,
    /// The session's state could not be read, so the call is refused. AICD §17.
    SessionStateUnavailable,
    /// The session's call budget for the window is spent. AICD §22.
    RateLimited,
    /// The name is none of Ori's tools. AICD §17.
    UnknownTool,
    /// The tool is not built in phase 1, or has no handler. AICD §17.
    ToolNotAvailable,
    /// The arguments are over the byte cap or break the tool's schema. AICD §27.
    InvalidArguments {
        /// An RFC 6901 pointer built only from schema property names and array
        /// indices; `""` is the root.
        field: String,
        /// A fixed description of what is wrong at `field`.
        why: &'static str,
    },
    /// `mcp.tool_called` could not be appended, so the call is not made.
    /// AICD §17.
    CallNotRecorded,
}

/// How a refusal reaches the agent.
enum Surface {
    /// A JSON-RPC error response with this code.
    Rpc(i64),
    /// A `CallToolResult` with `isError` true.
    Result,
}

impl CallRefusal {
    /// A stable code naming the refusal, for the `mcp.tool_refused` event and
    /// the reply: AICD §17.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Tool(refusal) => refusal.code(),
            Self::ProductDatabaseMismatch => "product_database_mismatch",
            Self::SessionStateUnavailable => "session_state_unavailable",
            Self::RateLimited => "rate_limited",
            Self::UnknownTool => "unknown_tool",
            Self::ToolNotAvailable => "tool_not_available",
            Self::InvalidArguments { .. } => "invalid_arguments",
            Self::CallNotRecorded => "call_not_recorded",
        }
    }

    /// The methodology section this refusal is made under, which every variant
    /// carries: criterion ORI-P1-033.
    ///
    /// AICD §26 for another product's database; AICD §22 for a spent budget;
    /// AICD §27 for arguments that break the schema or the cap; AICD §17 for a
    /// session that cannot be read, a name or phase the server does not serve,
    /// and a call that could not be recorded; and for a [`ToolRefusal`], its own.
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        match self {
            Self::Tool(refusal) => refusal.methodology_ref(),
            Self::ProductDatabaseMismatch => whole_section(PRODUCTS_SECTION),
            Self::SessionStateUnavailable => whole_section(PERMISSIONS_SECTION),
            Self::RateLimited => whole_section(SAFEGUARDS_SECTION),
            Self::UnknownTool => whole_section(PERMISSIONS_SECTION),
            Self::ToolNotAvailable => whole_section(PERMISSIONS_SECTION),
            Self::InvalidArguments { .. } => whole_section(SECURITY_SECTION),
            Self::CallNotRecorded => whole_section(PERMISSIONS_SECTION),
        }
    }

    /// Whether the connection closes after this refusal.
    const fn closes_connection(&self) -> bool {
        match self {
            Self::ProductDatabaseMismatch
            | Self::RateLimited
            | Self::SessionStateUnavailable
            | Self::CallNotRecorded => true,
            Self::Tool(refusal) => matches!(refusal, ToolRefusal::SessionNotActive),
            Self::UnknownTool | Self::ToolNotAvailable | Self::InvalidArguments { .. } => false,
        }
    }

    /// How the refusal reaches the agent: refusals 6 and 7 are error results,
    /// every other is a JSON-RPC error.
    const fn surface(&self) -> Surface {
        match self {
            Self::Tool(refusal) => match refusal {
                ToolRefusal::SessionNotActive => Surface::Rpc(SESSION_NOT_ACTIVE_CODE),
                ToolRefusal::TicketNotAssigned { .. }
                | ToolRefusal::ClaimContradictsSession { .. } => Surface::Result,
                ToolRefusal::ToolNotPermittedForRole { .. }
                | ToolRefusal::ServerNotInProject { .. }
                | ToolRefusal::ServerReachNotAcknowledged { .. }
                | ToolRefusal::ServerNotExposedToRole { .. } => {
                    Surface::Rpc(crate::protocol::INVALID_PARAMS)
                }
            },
            Self::RateLimited => Surface::Rpc(RATE_LIMITED_CODE),
            Self::UnknownTool | Self::ToolNotAvailable => {
                Surface::Rpc(crate::protocol::INVALID_PARAMS)
            }
            Self::ProductDatabaseMismatch
            | Self::SessionStateUnavailable
            | Self::CallNotRecorded => Surface::Rpc(crate::protocol::INTERNAL_ERROR),
            Self::InvalidArguments { .. } => Surface::Result,
        }
    }

    /// The `{"reason", "methodology_ref"}` object of a reply, and `"field"` for
    /// `invalid_arguments`.
    fn structured(&self) -> JsonObject {
        let mut object = JsonObject::new();
        object.insert("reason".to_owned(), Value::from(self.code()));
        object.insert(
            "methodology_ref".to_owned(),
            methodology_ref_json(&self.methodology_ref()),
        );
        if let Self::InvalidArguments { field, .. } = self {
            object.insert("field".to_owned(), Value::from(field.as_str()));
        }
        object
    }
}

impl fmt::Display for CallRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Tool(refusal) = self {
            return write!(f, "{refusal}");
        }
        write!(f, "refused under {}: ", self.methodology_ref())?;
        match self {
            Self::Tool(_) => Ok(()),
            Self::ProductDatabaseMismatch => {
                f.write_str("the database is not this session's product's")
            }
            Self::SessionStateUnavailable => {
                f.write_str("the session's state could not be read, so the call is refused")
            }
            Self::RateLimited => f.write_str("the session's call budget for this window is spent"),
            Self::UnknownTool => f.write_str("the tool is not one of this server's tools"),
            Self::ToolNotAvailable => f.write_str("the tool is not available in this phase"),
            Self::InvalidArguments { field, why } => {
                write!(f, "the arguments are not valid at {field:?}: {why}")
            }
            Self::CallNotRecorded => {
                f.write_str("the call could not be recorded, so it was not made")
            }
        }
    }
}

impl std::error::Error for CallRefusal {}

// ---------------------------------------------------------------------------
// What a received item produces
// ---------------------------------------------------------------------------

/// Which event could not be recorded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AuditEvent {
    /// `mcp.tool_refused`, or a refusal that is never recorded (a database that
    /// is not the bound product's).
    Refused,
    /// `mcp.tool_called`.
    Called,
    /// `mcp.tool_failed`.
    Failed,
}

impl AuditEvent {
    /// The kind of the event.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Refused => EVENT_TOOL_REFUSED,
            Self::Called => EVENT_TOOL_CALLED,
            Self::Failed => EVENT_TOOL_FAILED,
        }
    }
}

/// An event the server could not append: AICD §17.
///
/// The call's reply is unchanged by it, and the connection closes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditFailure {
    /// Which event could not be written.
    pub event: AuditEvent,
    /// The store error's own text, never agent text.
    pub detail: String,
}

impl fmt::Display for AuditFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} could not be recorded: {}",
            self.event.kind(),
            self.detail
        )
    }
}

/// What one received item produces: at most one message, whether to close the
/// connection, and any event that could not be recorded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reply {
    /// The message to write back, if the item is answered.
    pub message: Option<Message>,
    /// Whether the connection closes once `message` has been written.
    pub close: bool,
    /// An event the server could not append. The engine reports it; the reply is
    /// not changed by it.
    pub unrecorded: Option<AuditFailure>,
}

impl Reply {
    fn answer(message: Message) -> Self {
        Self {
            message: Some(message),
            close: false,
            unrecorded: None,
        }
    }

    const fn silent() -> Self {
        Self {
            message: None,
            close: false,
            unrecorded: None,
        }
    }
}

/// An error response for `error`: with its id when it has one.
fn error_message(error: &ProtocolError) -> Message {
    Message::ErrorResponse(error.to_error_response())
}

/// A successful response carrying `result`, or the error to answer with when it
/// does not serialize to an object.
fn result_message(id: RequestId, result: &CallToolResult) -> Message {
    match ResultResponse::with_result(id.clone(), result) {
        Ok(response) => Message::ResultResponse(response),
        Err(error) => error_message(&error.with_id(Some(id))),
    }
}

/// The reply a refusal gets on the wire: a JSON-RPC error for refusals 0 to 5
/// and 8, an error result for 6 and 7, and never agent text.
fn refusal_message(id: RequestId, refusal: &CallRefusal) -> Message {
    match refusal.surface() {
        Surface::Rpc(code) => {
            let mut data = JsonObject::new();
            data.insert("reason".to_owned(), Value::from(refusal.code()));
            data.insert(
                "methodology_ref".to_owned(),
                methodology_ref_json(&refusal.methodology_ref()),
            );
            Message::ErrorResponse(ErrorResponse {
                id: Some(id),
                error: ErrorObject {
                    code,
                    message: refusal.to_string(),
                    data: Some(Value::Object(data)),
                },
            })
        }
        Surface::Result => result_message(id, &error_result(refusal.structured())),
    }
}

/// An error result whose `structuredContent` is `structured` and whose single
/// text block is that object serialized.
fn error_result(structured: JsonObject) -> CallToolResult {
    let text = Value::Object(structured.clone()).to_string();
    CallToolResult {
        content: vec![crate::protocol::ContentBlock::text(text)],
        structured_content: Some(structured),
        is_error: Some(true),
    }
}

// ---------------------------------------------------------------------------
// Failures after `mcp.tool_called`
// ---------------------------------------------------------------------------

/// How an allowed call's effect failed, as the event and the reply record it.
struct Failure {
    failure: &'static str,
    reason: Option<&'static str>,
    methodology_ref: Option<MethodologyRef>,
    field: Option<&'static str>,
}

impl Failure {
    const fn bare(failure: &'static str) -> Self {
        Self {
            failure,
            reason: None,
            methodology_ref: None,
            field: None,
        }
    }

    /// A refusal inside the effect, only when its reference resolves: a
    /// reference that does not is an implementation defect, never written.
    fn refused(code: &'static str, reference: MethodologyRef) -> Self {
        if reference.resolves() {
            Self {
                failure: "refused",
                reason: Some(code),
                methodology_ref: Some(reference),
                field: None,
            }
        } else {
            Self::bare("invalid")
        }
    }

    /// Classifies a handler's error by the failure mapping.
    fn of_error(error: &HandlerError) -> Self {
        match error {
            HandlerError::Refused {
                code,
                methodology_ref,
            } => Self::refused(code, methodology_ref.clone()),
            HandlerError::Port(PortError::Refused(inner)) => {
                match (inner.methodology_ref(), inner.code()) {
                    (Some(reference), Some(code)) => Self::refused(code, reference),
                    (Some(reference), None) => Self::refused(FALLBACK_REFUSAL_CODE, reference),
                    (None, _) => Self::bare("invalid"),
                }
            }
            HandlerError::Port(PortError::Invalid { field, .. }) => Self {
                failure: "invalid",
                reason: None,
                methodology_ref: None,
                field: Some(*field),
            },
            HandlerError::Port(PortError::Unavailable { .. }) => Self::bare("unavailable"),
        }
    }

    /// The `{"reason", "methodology_ref", "field"}` object, each key only when
    /// the mapping gives it a value.
    fn structured(&self) -> JsonObject {
        let mut object = JsonObject::new();
        if let Some(reason) = self.reason {
            object.insert("reason".to_owned(), Value::from(reason));
        }
        if let Some(reference) = &self.methodology_ref {
            object.insert(
                "methodology_ref".to_owned(),
                methodology_ref_json(reference),
            );
        }
        if let Some(field) = self.field {
            object.insert("field".to_owned(), Value::from(field));
        }
        object
    }
}

/// Counts the bytes a value serializes to without keeping them.
struct ByteCounter(usize);

impl Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(buf.len());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The serialized length of `value` in bytes; `usize::MAX` if it cannot be
/// serialized, which fails the byte cap closed.
fn serialized_len(value: &Value) -> usize {
    let mut counter = ByteCounter(0);
    match serde_json::to_writer(&mut counter, value) {
        Ok(()) => counter.0,
        Err(_) => usize::MAX,
    }
}

/// A call that passed checks 0 to 7.
struct Accepted {
    tool: Tool,
    arguments: JsonObject,
    arguments_bytes: usize,
    ticket: Option<Id>,
}

/// A call whose `mcp.tool_called` is appended: what a later `mcp.tool_failed`
/// needs to know about it.
struct Dispatched {
    tool: Tool,
    called_seq: u64,
    ticket_column: Option<Id>,
}

/// The ticket rule (check 7): AICD §7, AICD §25.
///
/// Only the coder's row of `spec/ENV_SETUP.md`'s Tickets column is restricted to
/// its own ticket. Parsed identifiers are compared.
fn ticket_rule(binding: &Binding, ticket: &Id) -> Result<(), ToolRefusal> {
    if binding.role() != Role::Coder {
        return Ok(());
    }
    match binding.ticket_id() {
        Some(bound) if bound == ticket => Ok(()),
        Some(_) => Err(ToolRefusal::ClaimContradictsSession {
            claim: TICKET_ARGUMENT,
        }),
        None => Err(ToolRefusal::TicketNotAssigned {
            ticket: ticket.clone(),
        }),
    }
}

// ---------------------------------------------------------------------------
// The server
// ---------------------------------------------------------------------------

/// The agent-facing MCP server core for one connection of one session: AICD §8,
/// AICD §17.
///
/// Synchronous, `Send`, and holding no database: the engine passes the product's
/// one `ProductDb` writer to [`Server::receive`] with every item, and takes it
/// for one `receive` only, never across a read. The module documentation states
/// the lifecycle, the pipeline and what is recorded.
pub struct Server {
    binding: Binding,
    config: ServerConfig,
    state: ConnectionState,
    clock: Box<dyn Clock>,
    sessions: Box<dyn SessionDesk>,
    handlers: Handlers,
    budget: Arc<Mutex<RateWindow>>,
}

impl fmt::Debug for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Server")
            .field("binding", &self.binding)
            .field("state", &self.state)
            .field("handlers", &self.handlers)
            .finish_non_exhaustive()
    }
}

impl Server {
    /// A server for one connection of the session `binding` names.
    ///
    /// `budget` is the session's, shared by every connection of that session: the
    /// engine creates it when it registers the session and passes a clone of the
    /// `Arc` to every connection's server, so reconnecting does not reset the
    /// window. `clock`, `sessions` and `handlers` are the engine's and the
    /// slices'. The connection starts `Fresh`.
    #[must_use]
    pub fn new(
        binding: Binding,
        config: ServerConfig,
        budget: Arc<Mutex<RateWindow>>,
        clock: Box<dyn Clock>,
        sessions: Box<dyn SessionDesk>,
        handlers: Handlers,
    ) -> Self {
        Self {
            binding,
            config,
            state: ConnectionState::Fresh,
            clock,
            sessions,
            handlers,
            budget,
        }
    }

    /// One received item in, at most one message out.
    ///
    /// `received` is what `LineReader::read_message` returned for one line (not
    /// the end of input). The reply says what to write, whether to close the
    /// connection after writing it, and which event, if any, could not be
    /// recorded. The engine takes the database writer for this call only.
    pub fn receive(
        &mut self,
        db: &mut ProductDb,
        received: Result<Message, ReceiveError>,
    ) -> Reply {
        match received {
            // The stream is out of step: answer once, with no id, and close.
            Err(ReceiveError::Frame(error)) => Reply {
                message: Some(error_message(&error.to_protocol_error())),
                close: true,
                unrecorded: None,
            },
            // One bad line: an error with an id answers that request, one
            // without is answered once with no id, and reading goes on.
            Err(ReceiveError::Message(error)) => Reply::answer(error_message(&error)),
            Ok(message) => self.receive_message(db, message),
        }
    }

    /// Drives one connection to its end, for tests and single-session use only.
    ///
    /// Reads with `LineReader::read_message`, calls [`Server::receive`], and
    /// writes the reply with `write_message`. Returns `Ok(())` when the reader is
    /// at the end of input, or once a reply with `close` set has been written. A
    /// [`ReceiveError::Frame`] is stream-fatal: it is answered once and the frame
    /// error is returned, even if the answer cannot be written. A write failure
    /// is returned as its frame error.
    ///
    /// **This is not the engine's per-connection loop.** It holds the writer
    /// across blocking reads, and it drops `Reply::unrecorded`. The engine reads a
    /// whole line off the writer task, then takes the writer for one
    /// `receive(db, item)`, never across a read.
    ///
    /// # Errors
    ///
    /// The frame error that ended the stream, or the one that stopped a write.
    pub fn serve<R: BufRead, W: Write>(
        &mut self,
        db: &mut ProductDb,
        reader: &mut LineReader<R>,
        writer: &mut W,
    ) -> Result<(), FrameError> {
        loop {
            let received = match reader.read_message() {
                Ok(None) => return Ok(()),
                Ok(Some(message)) => Ok(message),
                Err(error) => Err(error),
            };
            let fatal = match &received {
                Err(ReceiveError::Frame(error)) => Some(error.clone()),
                Err(ReceiveError::Message(_)) | Ok(_) => None,
            };
            let reply = self.receive(db, received);
            let written = match &reply.message {
                Some(message) => write_message(writer, message),
                None => Ok(()),
            };
            if let Some(error) = fatal {
                return Err(error);
            }
            written?;
            if reply.close {
                return Ok(());
            }
        }
    }

    fn receive_message(&mut self, db: &mut ProductDb, message: Message) -> Reply {
        if let Err(error) = self.state.check(Direction::ClientToServer, &message) {
            // A request is answered with its id; a notification is dropped.
            return match message {
                Message::Request(_) => Reply::answer(error_message(&error)),
                Message::Notification(_)
                | Message::ResultResponse(_)
                | Message::ErrorResponse(_) => Reply::silent(),
            };
        }
        match message {
            Message::Request(request) => self.receive_request(db, request),
            Message::Notification(notification) => {
                self.receive_notification(notification);
                Reply::silent()
            }
            // The server sends no request in phase 1: a response answers nothing.
            Message::ResultResponse(_) | Message::ErrorResponse(_) => Reply::silent(),
        }
    }

    fn receive_request(&mut self, db: &mut ProductDb, request: Request) -> Reply {
        let (id, known) = match KnownRequest::decode(Direction::ClientToServer, request) {
            Ok(decoded) => decoded,
            Err(error) => return Reply::answer(error_message(&error)),
        };
        match known {
            KnownRequest::Initialize(params) => self.initialize(id, &params),
            KnownRequest::Ping => Reply::answer(Message::ResultResponse(ResultResponse::empty(id))),
            KnownRequest::ToolsList(params) => self.list(id, &params),
            KnownRequest::ToolsCall(params) => self.call(db, id, params),
        }
    }

    fn receive_notification(&mut self, notification: Notification) {
        match KnownNotification::decode(Direction::ClientToServer, notification) {
            Ok(Some(KnownNotification::Initialized)) => {
                if let Ok(next) = self.state.advance(Transition::InitializedNotified) {
                    self.state = next;
                }
            }
            // Cancellation is ignored (module documentation); a notification's
            // errors, and the ones this direction never carries, are never
            // answered.
            Ok(Some(KnownNotification::Cancelled(_) | KnownNotification::ToolsListChanged))
            | Ok(None)
            | Err(_) => {}
        }
    }

    fn initialize(&mut self, id: RequestId, params: &InitializeParams) -> Reply {
        let version = match negotiate_version(&params.protocol_version, SUPPORTED_PROTOCOL_VERSIONS)
        {
            Ok(version) => version,
            Err(error) => return Reply::answer(error_message(&error.with_id(Some(id)))),
        };
        let result = InitializeResult {
            protocol_version: version.to_owned(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..ServerCapabilities::default()
            },
            server_info: Implementation {
                name: "ori-studio".to_owned(),
                title: Some("Ori Studio".to_owned()),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                description: None,
                website_url: None,
            },
            instructions: None,
        };
        match ResultResponse::with_result(id.clone(), &result) {
            Ok(response) => {
                if let Ok(next) = self.state.advance(Transition::InitializeAnswered) {
                    self.state = next;
                }
                Reply::answer(Message::ResultResponse(response))
            }
            Err(error) => Reply::answer(error_message(&error.with_id(Some(id)))),
        }
    }

    /// `tools/list`: the caller's tools and nothing else, in one page.
    fn list(&self, id: RequestId, params: &ListToolsParams) -> Reply {
        if params.cursor.is_some() {
            // The server issues no cursor, so every cursor is invalid. The
            // cursor is not echoed.
            return Reply::answer(error_message(
                &ProtocolError::new(ErrorCode::InvalidParams, "invalid cursor").with_id(Some(id)),
            ));
        }
        let role = self.binding.role();
        let listed: Vec<_> = Tool::ALL
            .into_iter()
            .filter(|tool| {
                tool.in_phase_1()
                    && ToolScopes::permits(role, *tool)
                    && self.handlers.is_registered(*tool)
            })
            .map(|tool| tools::spec(tool).definition(self.config.max_arguments_bytes))
            .collect();
        // S6 POINT 1: slice S6 appends the operator's proxied tools to `listed`
        // here. It must keep two things. The server declares `listChanged:
        // false`, so S6 either snapshots the proxied tools at connection start
        // or switches `list_changed` to true and sends the notification. And S6
        // drops or refuses a proxied name that breaks MCP's tool-name rules.
        let result = ListToolsResult {
            tools: listed,
            next_cursor: None,
        };
        match ResultResponse::with_result(id.clone(), &result) {
            Ok(response) => Reply::answer(Message::ResultResponse(response)),
            Err(error) => Reply::answer(error_message(&error.with_id(Some(id)))),
        }
    }

    /// Charges the session's budget for the call made at `now`. A poisoned lock
    /// counts as an exhausted window.
    fn budget_admits(&self, now: Timestamp) -> bool {
        match self.budget.lock() {
            Ok(mut window) => window.charge(now),
            Err(_) => false,
        }
    }

    /// `tools/call`: the pipeline of the module documentation.
    fn call(&mut self, db: &mut ProductDb, id: RequestId, params: CallToolParams) -> Reply {
        let now = self.clock.now();
        let within_budget = self.budget_admits(now);
        // S6 POINT 2: slice S6 adds its branch for a proxied name before this
        // classification, which is check 3.
        let named = Tool::from_wire_name(&params.name);
        let name_chars = params.name.chars().count();
        // `task` and `_meta` are dropped here: never recorded and never echoed.
        let CallToolParams { arguments, .. } = params;

        let accepted = match self.admit(db, within_budget, named, arguments) {
            Ok(accepted) => accepted,
            Err(refusal) => return self.refuse(db, now, id, named, name_chars, &refusal),
        };
        let ticket_column = accepted
            .ticket
            .clone()
            .or_else(|| self.binding.ticket_id().cloned());
        let called = self.append(
            db,
            now,
            EVENT_TOOL_CALLED,
            ticket_column.clone(),
            self.called_payload(accepted.tool, accepted.arguments_bytes),
        );
        let called_seq = match called {
            Ok(seq) => seq,
            Err(detail) => {
                // No effect without its record: refuse, and attempt to record the
                // refusal. The failure that matters is the first append's.
                let mut reply = self.refuse(
                    db,
                    now,
                    id,
                    named,
                    name_chars,
                    &CallRefusal::CallNotRecorded,
                );
                reply.unrecorded = Some(AuditFailure {
                    event: AuditEvent::Called,
                    detail,
                });
                reply.close = true;
                return reply;
            }
        };

        let Accepted {
            tool,
            arguments,
            ticket,
            ..
        } = accepted;
        let outcome = {
            let call = BoundCall {
                binding: &self.binding,
                tool,
                called_seq,
                at: now,
                ticket_id: ticket.as_ref(),
            };
            match self.handlers.handler_mut(tool) {
                Some(handler) => handler.call(db, &call, arguments),
                None => Err(HandlerError::Port(PortError::Unavailable {
                    what: "the tool's handler".to_owned(),
                })),
            }
        };
        let dispatched = Dispatched {
            tool,
            called_seq,
            ticket_column,
        };
        match outcome {
            Ok(result) if !result.is_error() => Reply::answer(result_message(id, &result)),
            Ok(result) => self.fail(db, id, &dispatched, &Failure::bare("error_result"), &result),
            Err(error) => {
                let failure = Failure::of_error(&error);
                let result = error_result(failure.structured());
                self.fail(db, id, &dispatched, &failure, &result)
            }
        }
    }

    /// Checks 0 to 7. Returns the call to record and dispatch, or the first
    /// refusal.
    fn admit(
        &mut self,
        db: &mut ProductDb,
        within_budget: bool,
        named: Option<Tool>,
        arguments: Option<JsonObject>,
    ) -> Result<Accepted, CallRefusal> {
        // 0: the database is the bound product's. Never write into another
        // product's log.
        if db.product_id() != self.binding.product_id().as_str() {
            return Err(CallRefusal::ProductDatabaseMismatch);
        }
        // 1: the session's call budget.
        if !within_budget {
            return Err(CallRefusal::RateLimited);
        }
        // 2: the session may still act. An unreadable state fails closed.
        match self.sessions.is_active(db, &self.binding) {
            Ok(true) => {}
            Ok(false) => return Err(CallRefusal::Tool(ToolRefusal::SessionNotActive)),
            Err(_) => return Err(CallRefusal::SessionStateUnavailable),
        }
        // 3: one of Ori's tools.
        let Some(tool) = named else {
            return Err(CallRefusal::UnknownTool);
        };
        // 4: built in phase 1, and a handler is registered.
        if !tool.in_phase_1() || !self.handlers.is_registered(tool) {
            return Err(CallRefusal::ToolNotAvailable);
        }
        // 5: the role holds the tool.
        ToolScopes::authorize(self.binding.role(), tool).map_err(CallRefusal::Tool)?;
        // 6: the byte cap, then the schema. Absent arguments are the empty
        // object.
        let spec = tools::spec(tool);
        let mut value = Value::Object(arguments.unwrap_or_default());
        let arguments_bytes = serialized_len(&value);
        if arguments_bytes > self.config.max_arguments_bytes {
            return Err(CallRefusal::InvalidArguments {
                field: String::new(),
                why: WHY_ARGUMENTS_TOO_LARGE,
            });
        }
        spec.schema()
            .validate(&mut value)
            .map_err(|violation| CallRefusal::InvalidArguments {
                field: violation.field,
                why: violation.why,
            })?;
        let Value::Object(arguments) = value else {
            return Err(CallRefusal::InvalidArguments {
                field: String::new(),
                why: WHY_NOT_AN_OBJECT,
            });
        };
        // 7: the ticket rule, on the validated, canonical argument.
        let ticket = match (spec.takes_ticket(), arguments.get(TICKET_ARGUMENT)) {
            (true, Some(Value::String(text))) => match Id::parse(text) {
                Ok(ticket) => Some(ticket),
                Err(_) => {
                    return Err(CallRefusal::InvalidArguments {
                        field: format!("/{TICKET_ARGUMENT}"),
                        why: WHY_NOT_AN_IDENTIFIER,
                    });
                }
            },
            _ => None,
        };
        if let Some(ticket) = &ticket {
            ticket_rule(&self.binding, ticket).map_err(CallRefusal::Tool)?;
        }
        Ok(Accepted {
            tool,
            arguments,
            arguments_bytes,
            ticket,
        })
    }

    /// Records a refusal and builds its reply. A refusal that cannot be recorded
    /// is still refused, and the connection closes. Refusal 0 is never recorded.
    fn refuse(
        &self,
        db: &mut ProductDb,
        now: Timestamp,
        id: RequestId,
        named: Option<Tool>,
        name_chars: usize,
        refusal: &CallRefusal,
    ) -> Reply {
        let message = refusal_message(id, refusal);
        if matches!(refusal, CallRefusal::ProductDatabaseMismatch) {
            return Reply {
                message: Some(message),
                close: true,
                unrecorded: Some(AuditFailure {
                    event: AuditEvent::Refused,
                    detail: DETAIL_FOREIGN_DATABASE.to_owned(),
                }),
            };
        }
        let appended = self.append(
            db,
            now,
            EVENT_TOOL_REFUSED,
            self.binding.ticket_id().cloned(),
            self.refused_payload(named, name_chars, refusal),
        );
        let unrecorded = appended.err().map(|detail| AuditFailure {
            event: AuditEvent::Refused,
            detail,
        });
        Reply {
            message: Some(message),
            close: refusal.closes_connection() || unrecorded.is_some(),
            unrecorded,
        }
    }

    /// Records `mcp.tool_failed` and builds the reply for an effect that failed.
    fn fail(
        &mut self,
        db: &mut ProductDb,
        id: RequestId,
        dispatched: &Dispatched,
        failure: &Failure,
        result: &CallToolResult,
    ) -> Reply {
        let at = self.clock.now();
        let appended = self.append(
            db,
            at,
            EVENT_TOOL_FAILED,
            dispatched.ticket_column.clone(),
            self.failed_payload(dispatched.tool, dispatched.called_seq, failure),
        );
        let unrecorded = appended.err().map(|detail| AuditFailure {
            event: AuditEvent::Failed,
            detail,
        });
        Reply {
            message: Some(result_message(id, result)),
            close: unrecorded.is_some(),
            unrecorded,
        }
    }

    /// Appends one event of this session's identity. `Err` holds the store
    /// error's own text.
    fn append(
        &self,
        db: &mut ProductDb,
        at: Timestamp,
        kind: &'static str,
        ticket: Option<Id>,
        payload: String,
    ) -> Result<u64, String> {
        EventLog::append(
            db.connection(),
            self.binding.product_id().clone(),
            at,
            Actor::Agent(self.binding.identity().clone()),
            kind,
            ticket,
            payload,
        )
        .map(|event| event.seq())
        .map_err(|error| error.to_string())
    }

    fn called_payload(&self, tool: Tool, arguments_bytes: usize) -> String {
        json!({
            "session_id": self.binding.session_id().as_str(),
            "tool": tool.wire_name(),
            "arguments_bytes": arguments_bytes,
        })
        .to_string()
    }

    fn refused_payload(
        &self,
        named: Option<Tool>,
        name_chars: usize,
        refusal: &CallRefusal,
    ) -> String {
        let mut payload = JsonObject::new();
        payload.insert(
            "session_id".to_owned(),
            Value::from(self.binding.session_id().as_str()),
        );
        payload.insert(
            "tool".to_owned(),
            named.map_or(Value::Null, |tool| Value::from(tool.wire_name())),
        );
        if named.is_none() {
            payload.insert("name_chars".to_owned(), Value::from(name_chars));
        }
        payload.insert("reason".to_owned(), Value::from(refusal.code()));
        payload.insert(
            "methodology_ref".to_owned(),
            methodology_ref_json(&refusal.methodology_ref()),
        );
        if let CallRefusal::InvalidArguments { field, .. } = refusal {
            payload.insert("field".to_owned(), Value::from(field.as_str()));
        }
        Value::Object(payload).to_string()
    }

    fn failed_payload(&self, tool: Tool, called_seq: u64, failure: &Failure) -> String {
        let mut payload = JsonObject::new();
        payload.insert(
            "session_id".to_owned(),
            Value::from(self.binding.session_id().as_str()),
        );
        payload.insert("tool".to_owned(), Value::from(tool.wire_name()));
        payload.insert("called_seq".to_owned(), Value::from(called_seq));
        payload.insert("failure".to_owned(), Value::from(failure.failure));
        for (key, value) in failure.structured() {
            payload.insert(key, value);
        }
        Value::Object(payload).to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicI64, AtomicU32, AtomicUsize, Ordering};

    use ori_core::error::{Error, RefusalKind};
    use ori_core::types::{Category, TicketState};
    use ori_store::event_log::Event;

    use super::*;
    use crate::protocol::parse_message;
    use crate::tools::ToolHandler;

    // -------------------------------------------------------------------
    // Scratch layout and identifiers: the self-cleaning pattern
    // `ori-memory`'s `scope.rs` tests use, with no dev-dependency.
    // -------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0169-{label}-{}-{unique}",
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
    fn id(label: &str) -> Id {
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

    fn product_a() -> Id {
        id("PRODUCTA")
    }

    fn product_b() -> Id {
        id("PRODUCTB")
    }

    fn session() -> Id {
        id("SESSION")
    }

    fn identity() -> Id {
        id("AGENT")
    }

    fn ticket_1() -> Id {
        id("TICKET1")
    }

    fn ticket_2() -> Id {
        id("TICKET2")
    }

    const START: i64 = 1_000_000;

    // Markers planted in everything an agent chooses.
    const MARKER_NAME: &str = "zz-marker-name-7f3a";
    const MARKER_TEXT: &str = "zz-marker-text-7f3a";
    const MARKER_KEY: &str = "zz-marker-key-7f3a";
    const MARKER_ID: &str = "zz-marker-id-7f3a";
    const MARKER_META: &str = "zz-marker-meta-7f3a";
    const MARKER_TASK: &str = "zz-marker-task-7f3a";
    const MARKER_NUMBER: i64 = 77_003_377;

    fn markers() -> Vec<String> {
        vec![
            MARKER_NAME.to_owned(),
            MARKER_TEXT.to_owned(),
            MARKER_KEY.to_owned(),
            MARKER_ID.to_owned(),
            MARKER_META.to_owned(),
            MARKER_TASK.to_owned(),
            MARKER_NUMBER.to_string(),
        ]
    }

    // -------------------------------------------------------------------
    // The fakes: a clock, a session desk, and handlers
    // -------------------------------------------------------------------

    struct FakeClock {
        now: Arc<AtomicI64>,
        step: i64,
        reads: Arc<AtomicUsize>,
    }

    impl Clock for FakeClock {
        fn now(&mut self) -> Timestamp {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Timestamp::from_millis(self.now.fetch_add(self.step, Ordering::SeqCst))
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum DeskMode {
        Active,
        Inactive,
        Unreadable,
    }

    struct FakeDesk {
        mode: Arc<Mutex<DeskMode>>,
        asked: Arc<AtomicUsize>,
    }

    impl SessionDesk for FakeDesk {
        fn is_active(&mut self, _db: &mut ProductDb, binding: &Binding) -> Result<bool, PortError> {
            assert_eq!(binding.session_id(), &session(), "the engine's own binding");
            self.asked.fetch_add(1, Ordering::SeqCst);
            match *self.mode.lock().expect("the desk mode") {
                DeskMode::Active => Ok(true),
                DeskMode::Inactive => Ok(false),
                DeskMode::Unreadable => Err(PortError::Unavailable {
                    what: format!("the session registry {MARKER_TEXT}"),
                }),
            }
        }
    }

    struct FnHandler<F>(F);

    impl<F> ToolHandler for FnHandler<F>
    where
        F: FnMut(
                &mut ProductDb,
                &BoundCall<'_>,
                JsonObject,
            ) -> Result<CallToolResult, HandlerError>
            + Send,
    {
        fn call(
            &mut self,
            db: &mut ProductDb,
            call: &BoundCall<'_>,
            arguments: JsonObject,
        ) -> Result<CallToolResult, HandlerError> {
            (self.0)(db, call, arguments)
        }
    }

    fn handler<F>(function: F) -> Box<dyn ToolHandler>
    where
        F: FnMut(
                &mut ProductDb,
                &BoundCall<'_>,
                JsonObject,
            ) -> Result<CallToolResult, HandlerError>
            + Send
            + 'static,
    {
        Box::new(FnHandler(function))
    }

    /// A handler that counts its calls and answers "done".
    fn counting_handler(ran: &Arc<AtomicUsize>) -> Box<dyn ToolHandler> {
        let ran = Arc::clone(ran);
        handler(move |_, _, _| {
            ran.fetch_add(1, Ordering::SeqCst);
            Ok(CallToolResult::text("done"))
        })
    }

    fn all_phase_1_handlers(ran: &Arc<AtomicUsize>) -> Handlers {
        let mut handlers = Handlers::new();
        for tool in Tool::ALL {
            if tool.in_phase_1() {
                handlers
                    .register(tool, counting_handler(ran))
                    .expect("one handler per tool");
            }
        }
        handlers
    }

    /// A handler that answers every call with `outcome`.
    fn answering(
        outcome: Result<CallToolResult, HandlerError>,
        ran: &Arc<AtomicUsize>,
    ) -> Box<dyn ToolHandler> {
        let ran = Arc::clone(ran);
        handler(move |_, _, _| {
            ran.fetch_add(1, Ordering::SeqCst);
            outcome.clone()
        })
    }

    // -------------------------------------------------------------------
    // The world: a real `ProductDb` in a temporary directory and a server
    // -------------------------------------------------------------------

    struct World {
        db: ProductDb,
        server: Server,
        binding: Binding,
        clock: Arc<AtomicI64>,
        reads: Arc<AtomicUsize>,
        desk: Arc<Mutex<DeskMode>>,
        desk_asked: Arc<AtomicUsize>,
        budget: Arc<Mutex<RateWindow>>,
        ran: Arc<AtomicUsize>,
        next_id: i64,
        _scratch: Scratch,
    }

    /// Builds a world's handlers from its counter of handler runs.
    type HandlerBuilder = Box<dyn FnOnce(&Arc<AtomicUsize>) -> Handlers>;

    struct Setup {
        role: Role,
        ticket: Option<Id>,
        db_product: Id,
        max_calls: u32,
        config: ServerConfig,
        step: i64,
        desk: DeskMode,
        handlers: Option<HandlerBuilder>,
    }

    impl Setup {
        fn new() -> Self {
            Self {
                role: Role::Coder,
                ticket: Some(ticket_1()),
                db_product: product_a(),
                max_calls: 120,
                config: ServerConfig::default(),
                step: 0,
                desk: DeskMode::Active,
                handlers: None,
            }
        }

        fn role(mut self, role: Role) -> Self {
            self.role = role;
            self
        }

        fn ticket(mut self, ticket: Option<Id>) -> Self {
            self.ticket = ticket;
            self
        }

        fn db_product(mut self, product: Id) -> Self {
            self.db_product = product;
            self
        }

        fn max_calls(mut self, max: u32) -> Self {
            self.max_calls = max;
            self
        }

        fn max_arguments_bytes(mut self, bytes: usize) -> Self {
            self.config = ServerConfig {
                max_arguments_bytes: bytes,
            };
            self
        }

        fn step(mut self, step: i64) -> Self {
            self.step = step;
            self
        }

        fn desk(mut self, mode: DeskMode) -> Self {
            self.desk = mode;
            self
        }

        fn handlers(mut self, build: impl FnOnce(&Arc<AtomicUsize>) -> Handlers + 'static) -> Self {
            self.handlers = Some(Box::new(build));
            self
        }

        fn build(self, label: &str) -> World {
            let scratch = Scratch::new(label);
            let db = ProductDb::open(
                &scratch.path,
                self.db_product.as_str(),
                Timestamp::from_millis(1),
            )
            .expect("a fresh product database opens");
            let binding = Binding::new(session(), identity(), self.role, product_a(), self.ticket);
            let clock = Arc::new(AtomicI64::new(START));
            let reads = Arc::new(AtomicUsize::new(0));
            let desk = Arc::new(Mutex::new(self.desk));
            let desk_asked = Arc::new(AtomicUsize::new(0));
            let budget = Arc::new(Mutex::new(RateWindow::new(self.max_calls)));
            let ran = Arc::new(AtomicUsize::new(0));
            let handlers = match self.handlers {
                Some(build) => build(&ran),
                None => all_phase_1_handlers(&ran),
            };
            let server = Server::new(
                binding.clone(),
                self.config,
                Arc::clone(&budget),
                Box::new(FakeClock {
                    now: Arc::clone(&clock),
                    step: self.step,
                    reads: Arc::clone(&reads),
                }),
                Box::new(FakeDesk {
                    mode: Arc::clone(&desk),
                    asked: Arc::clone(&desk_asked),
                }),
                handlers,
            );
            World {
                db,
                server,
                binding,
                clock,
                reads,
                desk,
                desk_asked,
                budget,
                ran,
                next_id: 1,
                _scratch: scratch,
            }
        }
    }

    impl World {
        fn ready(label: &str) -> Self {
            let mut world = Setup::new().build(label);
            world.handshake();
            world
        }

        fn send(&mut self, message: Message) -> Reply {
            self.server.receive(&mut self.db, Ok(message))
        }

        fn handshake(&mut self) {
            let reply = self.send(initialize_request(0, "2025-11-25"));
            assert!(reply.message.is_some());
            let reply = self.send(initialized_notification());
            assert_eq!(reply, Reply::silent());
        }

        fn next_request_id(&mut self) -> RequestId {
            self.next_id += 1;
            RequestId::Number(self.next_id)
        }

        fn call(&mut self, name: &str, arguments: Value) -> Reply {
            let id = self.next_request_id();
            self.send(call_request(
                id,
                json!({"name": name, "arguments": arguments}),
            ))
        }

        fn events(&mut self) -> Vec<Event> {
            EventLog::read_range(self.db.connection(), 1, u64::MAX).expect("the log reads")
        }

        fn mcp_events(&mut self) -> Vec<Event> {
            self.events()
                .into_iter()
                .filter(|event| event.kind().starts_with("mcp."))
                .collect()
        }

        fn kinds(&mut self) -> Vec<String> {
            self.mcp_events()
                .iter()
                .map(|event| event.kind().to_owned())
                .collect()
        }

        fn read_only(&mut self) {
            self.db
                .connection()
                .execute_batch("PRAGMA query_only = ON;")
                .expect("the connection goes read-only");
        }

        /// Makes every append of `kind` fail, and no other.
        fn forbid(&mut self, kind: &str) {
            self.db
                .connection()
                .execute_batch(&format!(
                    "CREATE TRIGGER forbid_{0} BEFORE INSERT ON events WHEN NEW.kind = '{1}' \
                     BEGIN SELECT RAISE(ABORT, 'a test forbids this kind'); END;",
                    kind.replace('.', "_"),
                    kind
                ))
                .expect("the trigger is created");
        }
    }

    // -------------------------------------------------------------------
    // Messages and what a reply looks like
    // -------------------------------------------------------------------

    fn initialize_request(id: i64, version: &str) -> Message {
        Message::Request(Request {
            id: RequestId::Number(id),
            method: "initialize".to_owned(),
            params: Some(json!({
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {"name": "test-agent", "version": "1"},
            })),
        })
    }

    fn initialized_notification() -> Message {
        Message::Notification(Notification::without_params("notifications/initialized"))
    }

    fn request(id: i64, method: &str, params: Option<Value>) -> Message {
        Message::Request(Request {
            id: RequestId::Number(id),
            method: method.to_owned(),
            params,
        })
    }

    fn call_request(id: RequestId, params: Value) -> Message {
        Message::Request(Request {
            id,
            method: "tools/call".to_owned(),
            params: Some(params),
        })
    }

    /// The reply's message as the JSON it goes out as.
    fn wire(reply: &Reply) -> Value {
        serde_json::to_value(reply.message.as_ref().expect("a reply message"))
            .expect("a message serializes")
    }

    fn methodology(section: u64) -> Value {
        json!({"section": section, "subsection": null})
    }

    /// Asserts a JSON-RPC error reply of one of refusals 0 to 5 and 8: the id, the
    /// code, the `Display` message and the `data`. Returns the wire message.
    fn assert_rpc_refusal(reply: &Reply, id: i64, code: i64, reason: &str, section: u64) -> Value {
        let message = wire(reply);
        assert_eq!(message["jsonrpc"], "2.0");
        assert_eq!(message["id"], id);
        assert!(message.get("result").is_none(), "{message}");
        assert_eq!(message["error"]["code"], code, "{message}");
        assert_eq!(
            message["error"]["data"],
            json!({"reason": reason, "methodology_ref": methodology(section)}),
            "{message}"
        );
        let text = message["error"]["message"].as_str().expect("a message");
        assert!(
            text.starts_with(&format!("refused under AICD §{section}: ")),
            "{text}"
        );
        message
    }

    /// Asserts an error result of refusal 6 or 7: `structuredContent` is the
    /// reason and the reference (and the field), and the single text block is that
    /// object serialized.
    fn assert_result_refusal(
        reply: &Reply,
        id: i64,
        reason: &str,
        section: u64,
        field: Option<&str>,
    ) -> Value {
        let message = wire(reply);
        assert_eq!(message["id"], id);
        assert!(message.get("error").is_none(), "{message}");
        let result = &message["result"];
        assert_eq!(result["isError"], true, "{message}");
        let mut expected = json!({"reason": reason, "methodology_ref": methodology(section)});
        if let Some(field) = field {
            expected["field"] = json!(field);
        }
        assert_eq!(result["structuredContent"], expected, "{message}");
        let blocks = result["content"].as_array().expect("content");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        let text: Value =
            serde_json::from_str(blocks[0]["text"].as_str().expect("text")).expect("JSON text");
        assert_eq!(text, expected, "the text block is the structured content");
        message
    }

    fn payload(event: &Event) -> Value {
        serde_json::from_str(event.payload()).expect("a JSON payload")
    }

    fn agent() -> Actor {
        Actor::Agent(identity())
    }

    fn plan_arguments(ticket: &Id) -> Value {
        json!({
            "ticket_id": ticket.as_str(),
            "plan": "a plan",
            "declared_scope": ["crates/ori-mcp"],
        })
    }

    /// Asserts exactly one `mcp.tool_refused` event, and nothing else of `mcp.`,
    /// with this payload, written by the agent identity for this product.
    fn assert_one_refusal_event(world: &mut World, expected: Value) -> Event {
        let events = world.mcp_events();
        assert_eq!(events.len(), 1, "exactly one mcp event: {events:?}");
        let event = events.into_iter().next().expect("one event");
        assert_eq!(event.kind(), "mcp.tool_refused");
        assert_eq!(payload(&event), expected);
        assert_eq!(event.actor(), &agent());
        assert_eq!(event.product_id(), &product_a());
        event
    }

    fn refused_payload_for(tool: Option<&str>, reason: &str, section: u64, extra: Value) -> Value {
        let mut expected = json!({
            "session_id": session().as_str(),
            "tool": tool,
            "reason": reason,
            "methodology_ref": methodology(section),
        });
        if let Value::Object(extra) = extra {
            for (key, value) in extra {
                expected[key] = value;
            }
        }
        expected
    }

    fn assert_no_marker(text: &str, what: &str) {
        for marker in markers() {
            assert!(
                !text.contains(&marker),
                "{what} echoes the marker {marker}: {text}"
            );
        }
    }

    // -------------------------------------------------------------------
    // 1. The handshake
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_initialize_is_answered_with_the_negotiated_version_the_tools_capability_and_the_server_info()
     {
        let mut world = Setup::new().build("initialize");
        let reply = world.send(initialize_request(7, "2025-11-25"));
        assert!(!reply.close && reply.unrecorded.is_none());
        let message = wire(&reply);
        assert_eq!(message["jsonrpc"], "2.0");
        assert_eq!(message["id"], 7);
        assert_eq!(
            message["result"],
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {
                    "name": "ori-studio",
                    "title": "Ori Studio",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
            "the tools capability only, and no instructions"
        );
        // Initializing is not a call: nothing is recorded, and the clock is not read.
        assert!(world.events().is_empty());
        assert_eq!(world.reads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_a_version_the_server_does_not_speak_is_answered_with_the_one_it_does() {
        let mut world = Setup::new().build("older-version");
        let reply = world.send(initialize_request(1, "2024-11-05"));
        assert_eq!(wire(&reply)["result"]["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn ori_t_0169_a_refused_initialize_leaves_the_connection_fresh() {
        let mut world = Setup::new().build("refused-initialize");
        let reply = world.send(initialize_request(3, "latest"));
        let message = wire(&reply);
        assert_eq!(message["id"], 3);
        assert_eq!(message["error"]["code"], -32602);
        assert!(!reply.close);
        // Still Fresh: tools/list is out of order, a good initialize works.
        let reply = world.send(request(4, "tools/list", None));
        assert_eq!(wire(&reply)["error"]["code"], -32600);
        let reply = world.send(initialize_request(5, "2025-11-25"));
        assert!(wire(&reply).get("result").is_some());
        assert!(world.events().is_empty());
    }

    #[test]
    fn ori_t_0169_tools_list_and_tools_call_before_initialized_are_refused_by_the_connection_state()
    {
        let mut world = Setup::new().build("before-initialized");
        for state in ["fresh", "initialized"] {
            if state == "initialized" {
                let reply = world.send(initialize_request(1, "2025-11-25"));
                assert!(wire(&reply).get("result").is_some());
            }
            let reply = world.send(request(10, "tools/list", None));
            let message = wire(&reply);
            assert_eq!(message["id"], 10);
            assert_eq!(message["error"]["code"], -32600, "{state}: {message}");
            assert_eq!(message["error"]["data"]["state"], state);
            // A tools/call that the lifecycle refuses is not a call: not
            // recorded, no handler, no clock read.
            let reply = world.call("aicd_search", json!({"query": "q"}));
            let message = wire(&reply);
            assert_eq!(message["error"]["code"], -32600, "{state}: {message}");
            assert!(!reply.close && reply.unrecorded.is_none());
        }
        assert!(world.events().is_empty());
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
        assert_eq!(world.reads.load(Ordering::SeqCst), 0);
        assert_eq!(world.budget.lock().expect("the budget").calls_in_window, 0);
        // And after notifications/initialized the same request is served.
        let reply = world.send(initialized_notification());
        assert_eq!(reply, Reply::silent());
        let reply = world.send(request(11, "tools/list", None));
        assert!(wire(&reply).get("result").is_some());
    }

    #[test]
    fn ori_t_0169_ping_is_answered_in_every_state() {
        let mut world = Setup::new().build("ping");
        let ping = |world: &mut World, id: i64| {
            let reply = world.send(request(id, "ping", None));
            let message = wire(&reply);
            assert_eq!(message["id"], id);
            assert_eq!(message["result"], json!({}));
            assert!(!reply.close && reply.unrecorded.is_none());
        };
        ping(&mut world, 1);
        world.send(initialize_request(2, "2025-11-25"));
        ping(&mut world, 3);
        world.send(initialized_notification());
        ping(&mut world, 4);
        assert!(world.events().is_empty(), "a ping is not a call");
    }

    #[test]
    fn ori_t_0169_a_second_initialize_is_refused() {
        let mut world = Setup::new().build("second-initialize");
        world.send(initialize_request(1, "2025-11-25"));
        let reply = world.send(initialize_request(2, "2025-11-25"));
        let message = wire(&reply);
        assert_eq!(message["id"], 2);
        assert_eq!(message["error"]["code"], -32600);
        world.send(initialized_notification());
        let reply = world.send(initialize_request(3, "2025-11-25"));
        assert_eq!(wire(&reply)["error"]["code"], -32600);
        // The connection is undisturbed.
        let reply = world.send(request(4, "tools/list", None));
        assert!(wire(&reply).get("result").is_some());
    }

    #[test]
    fn ori_t_0169_unknown_methods_responses_and_notifications_are_answered_by_the_rule() {
        let mut world = World::ready("unknown-items");
        // An unknown request is "method not found", with its id.
        let reply = world.send(request(20, "resources/list", None));
        let message = wire(&reply);
        assert_eq!(message["id"], 20);
        assert_eq!(message["error"]["code"], -32601);
        // A request whose parameters do not fit is -32602 with its id, and is not
        // a call.
        let reply = world.send(request(21, "tools/call", Some(json!({"arguments": {}}))));
        let message = wire(&reply);
        assert_eq!(message["id"], 21);
        assert_eq!(message["error"]["code"], -32602);
        // A cancellation is ignored, as is one that does not decode, and a
        // notification that is out of state or unknown is never answered.
        for notification in [
            Notification::with_params(
                "notifications/cancelled",
                &json!({"requestId": 1, "reason": MARKER_TEXT}),
            )
            .expect("params"),
            Notification::with_params("notifications/cancelled", &json!({"requestId": [1]}))
                .expect("params"),
            Notification::without_params("notifications/initialized"),
            Notification::without_params("notifications/roots/list_changed"),
            Notification::without_params("notifications/tools/list_changed"),
        ] {
            assert_eq!(
                world.send(Message::Notification(notification)),
                Reply::silent()
            );
        }
        // A response from the client answers nothing.
        for response in [
            Message::ResultResponse(ResultResponse::empty(RequestId::Number(5))),
            Message::ErrorResponse(ErrorResponse {
                id: None,
                error: ErrorObject {
                    code: -1,
                    message: MARKER_TEXT.to_owned(),
                    data: None,
                },
            }),
        ] {
            assert_eq!(world.send(response), Reply::silent());
        }
        assert!(world.events().is_empty(), "none of these is a call");
    }

    #[test]
    fn ori_t_0169_unreadable_input_is_answered_without_an_id_and_a_frame_error_closes() {
        let mut world = World::ready("unreadable");
        // One bad line: answered once with no id, and reading goes on.
        let error = parse_message(b"{not json").expect_err("not JSON");
        let reply = world
            .server
            .receive(&mut world.db, Err(ReceiveError::Message(error)));
        let message = wire(&reply);
        assert!(message.get("id").is_none(), "{message}");
        assert_eq!(message["error"]["code"], -32700);
        assert!(!reply.close);
        // A readable request with a bad shape carries its id.
        let error = parse_message(br#"{"jsonrpc":"2.0","id":9,"method":5}"#).expect_err("a shape");
        let reply = world
            .server
            .receive(&mut world.db, Err(ReceiveError::Message(error)));
        assert_eq!(wire(&reply)["id"], 9);
        // A frame error is stream-fatal: answered once, no id, and closed.
        let reply = world.server.receive(
            &mut world.db,
            Err(ReceiveError::Frame(FrameError::TooLong { limit: 64 })),
        );
        let message = wire(&reply);
        assert!(message.get("id").is_none(), "{message}");
        assert_eq!(message["error"]["code"], -32600);
        assert!(reply.close);
        assert!(world.events().is_empty());
    }

    // -------------------------------------------------------------------
    // 2. tools/list
    // -------------------------------------------------------------------

    fn listed_names(world: &mut World) -> Vec<String> {
        let id = world.next_request_id();
        let reply = world.send(Message::Request(Request {
            id,
            method: "tools/list".to_owned(),
            params: None,
        }));
        let message = wire(&reply);
        assert!(message["result"].get("nextCursor").is_none(), "{message}");
        message["result"]["tools"]
            .as_array()
            .expect("a tools array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("a name").to_owned())
            .collect()
    }

    #[test]
    fn ori_t_0169_tools_list_is_exactly_the_phase_1_permitted_registered_tools_for_all_seven_roles()
    {
        for role in Role::ALL {
            // Every tool registered, even the three that are not built.
            let mut world = Setup::new()
                .role(*role)
                .handlers(|ran| {
                    let mut handlers = Handlers::new();
                    for tool in Tool::ALL {
                        handlers
                            .register(tool, counting_handler(ran))
                            .expect("one handler per tool");
                    }
                    handlers
                })
                .build("list-roles");
            world.handshake();
            let expected: Vec<String> = Tool::ALL
                .into_iter()
                .filter(|tool| tool.in_phase_1() && ToolScopes::permits(*role, *tool))
                .map(|tool| tool.wire_name().to_owned())
                .collect();
            assert_eq!(listed_names(&mut world), expected, "{role}");
            assert!(!expected.is_empty(), "{role} holds at least a memory tool");
            assert!(world.events().is_empty(), "tools/list is not recorded");
            assert_eq!(world.reads.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn ori_t_0169_tools_list_omits_a_tool_with_no_handler() {
        let mut world = Setup::new()
            .role(Role::Coder)
            .handlers(|ran| {
                let mut handlers = Handlers::new();
                handlers
                    .register(Tool::Report, counting_handler(ran))
                    .expect("registers");
                handlers
                    .register(Tool::Context, counting_handler(ran))
                    .expect("registers");
                handlers
            })
            .build("list-registered");
        world.handshake();
        // `Tool::ALL` order, not registration order.
        assert_eq!(listed_names(&mut world), ["aicd_context", "aicd_report"]);
    }

    #[test]
    fn ori_t_0169_tools_list_returns_the_catalogue_definitions_in_one_page() {
        let mut world = Setup::new().role(Role::Lead).build("list-definitions");
        world.handshake();
        let reply = world.send(request(40, "tools/list", None));
        let message = wire(&reply);
        let tools = message["result"]["tools"].as_array().expect("tools");
        let lead_tools = [
            Tool::Context,
            Tool::Search,
            Tool::Evidence,
            Tool::Escalate,
            Tool::TicketUpgrade,
        ];
        assert_eq!(tools.len(), lead_tools.len());
        for (listed, tool) in tools.iter().zip(lead_tools) {
            let definition = tools::spec(tool).definition(DEFAULT_MAX_ARGUMENTS_BYTES);
            assert_eq!(
                listed,
                &serde_json::to_value(&definition).expect("serializes")
            );
            assert!(
                listed["description"]
                    .as_str()
                    .expect("a description")
                    .ends_with("Arguments over 512 KiB serialized are refused.")
            );
        }
    }

    #[test]
    fn ori_t_0169_tools_list_refuses_a_cursor() {
        let mut world = World::ready("list-cursor");
        let reply = world.send(request(
            50,
            "tools/list",
            Some(json!({"cursor": MARKER_TEXT})),
        ));
        let message = wire(&reply);
        assert_eq!(message["id"], 50);
        assert_eq!(message["error"]["code"], -32602);
        assert_eq!(message["error"]["message"], "invalid cursor");
        assert_no_marker(&message.to_string(), "the cursor refusal");
        assert!(!reply.close);
        assert!(world.events().is_empty());
    }

    // -------------------------------------------------------------------
    // 3. Each refusal, alone
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_refusal_0_a_database_of_another_product_is_refused_unrecorded_and_closes() {
        let mut world = Setup::new().db_product(product_b()).build("refusal-0");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        let message = assert_rpc_refusal(&reply, 2, -32603, "product_database_mismatch", 26);
        assert_no_marker(&message.to_string(), "the reply");
        assert!(reply.close, "the connection closes");
        assert_eq!(
            reply.unrecorded,
            Some(AuditFailure {
                event: AuditEvent::Refused,
                detail: DETAIL_FOREIGN_DATABASE.to_owned(),
            })
        );
        // Nothing is written into another product's log, not even the refusal,
        // and nothing ran.
        assert!(world.events().is_empty(), "the log is untouched");
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
        assert_eq!(world.desk_asked.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_refusal_1_a_spent_budget_is_refused_recorded_and_closes() {
        let mut world = Setup::new().max_calls(0).build("refusal-1");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_rpc_refusal(&reply, 2, RATE_LIMITED_CODE, "rate_limited", 22);
        assert!(reply.close);
        assert_eq!(reply.unrecorded, None);
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(Some("aicd_plan_submit"), "rate_limited", 22, json!({})),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
        // Checks 1 and 2 reveal nothing and cost one read: the desk was not asked.
        assert_eq!(world.desk_asked.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_refusal_2_a_session_that_is_not_active_is_refused_recorded_and_closes() {
        let mut world = Setup::new().desk(DeskMode::Inactive).build("refusal-2");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_rpc_refusal(&reply, 2, SESSION_NOT_ACTIVE_CODE, "session_not_active", 17);
        assert!(reply.close);
        assert_eq!(reply.unrecorded, None);
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_plan_submit"),
                "session_not_active",
                17,
                json!({}),
            ),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_refusal_2_a_session_state_that_cannot_be_read_is_refused_closed() {
        let mut world = Setup::new().desk(DeskMode::Unreadable).build("refusal-2b");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        let message = assert_rpc_refusal(&reply, 2, -32603, "session_state_unavailable", 17);
        assert_no_marker(&message.to_string(), "the reply: the desk's own text");
        assert!(reply.close, "it fails closed");
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_plan_submit"),
                "session_state_unavailable",
                17,
                json!({}),
            ),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_refusal_3_a_name_that_is_not_one_of_oris_tools_is_refused_unknown() {
        let mut world = World::ready("refusal-3");
        let reply = world.call(MARKER_NAME, json!({"query": MARKER_TEXT}));
        let message = assert_rpc_refusal(&reply, 2, -32602, "unknown_tool", 17);
        assert_no_marker(&message.to_string(), "the reply");
        assert!(
            !reply.close,
            "an unknown name does not close the connection"
        );
        assert_eq!(reply.unrecorded, None);
        // `tool` is null and the name is recorded by its length only.
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                None,
                "unknown_tool",
                17,
                json!({"name_chars": MARKER_NAME.chars().count()}),
            ),
        );
        // A name of Ori's own spelled another way is unknown too: no case
        // folding, no trimming.
        let reply = world.call("AICD_SEARCH", json!({"query": "q"}));
        assert_rpc_refusal(&reply, 3, -32602, "unknown_tool", 17);
        let reply = world.call(" aicd_search", json!({"query": "q"}));
        assert_rpc_refusal(&reply, 4, -32602, "unknown_tool", 17);
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_refusal_3_a_name_is_recorded_by_its_length_in_characters() {
        let mut world = World::ready("refusal-3-chars");
        let name = "é".repeat(7);
        world.call(&name, json!({}));
        let events = world.mcp_events();
        assert_eq!(payload(&events[0])["name_chars"], 7);
        assert_eq!(payload(&events[0])["tool"], Value::Null);
    }

    #[test]
    fn ori_t_0169_refusal_4_a_tool_that_is_not_in_phase_1_is_refused_not_available() {
        // `aicd_gate_result` is qa's, not built in phase 1, and registered: a later
        // phase registers it, and dispatch still refuses it.
        let mut world = Setup::new()
            .role(Role::Qa)
            .ticket(None)
            .handlers(|ran| {
                let mut handlers = all_phase_1_handlers(ran);
                handlers
                    .register(Tool::GateResult, counting_handler(ran))
                    .expect("a tool outside phase 1 may be registered");
                handlers
            })
            .build("refusal-4a");
        world.handshake();
        let reply = world.call("aicd_gate_result", json!({}));
        let message = assert_rpc_refusal(&reply, 2, -32602, "tool_not_available", 17);
        assert_no_marker(&message.to_string(), "the reply");
        assert!(!reply.close);
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_gate_result"),
                "tool_not_available",
                17,
                json!({}),
            )
            .as_object()
            .map(|object| {
                let mut object = object.clone();
                object.insert("session_id".to_owned(), json!(session().as_str()));
                Value::Object(object)
            })
            .expect("an object"),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 0, "never dispatched");
    }

    #[test]
    fn ori_t_0169_refusal_4_a_phase_1_tool_with_no_handler_is_refused_not_available() {
        let mut world = Setup::new()
            .handlers(|_| Handlers::new())
            .build("refusal-4b");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_rpc_refusal(&reply, 2, -32602, "tool_not_available", 17);
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_plan_submit"),
                "tool_not_available",
                17,
                json!({}),
            ),
        );
    }

    #[test]
    fn ori_t_0169_refusal_5_a_role_that_does_not_hold_the_tool_is_refused() {
        let mut world = Setup::new().role(Role::Lead).build("refusal-5");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        let message = assert_rpc_refusal(&reply, 2, -32602, "tool_not_permitted_for_role", 17);
        assert_eq!(
            message["error"]["message"],
            "refused under AICD §17: the lead role may not call aicd_plan_submit"
        );
        assert!(!reply.close);
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_plan_submit"),
                "tool_not_permitted_for_role",
                17,
                json!({}),
            ),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_refusal_6_arguments_that_break_the_schema_are_an_error_result_with_the_field() {
        let mut world = World::ready("refusal-6");
        // A map value that is a number, an extra key, and a free-text value are
        // all marked: none may come back.
        let reply = world.call(
            "aicd_report",
            json!({
                "ticket_id": ticket_1().as_str(),
                "kind": "closing",
                "content": {MARKER_KEY: MARKER_NUMBER},
            }),
        );
        let message = assert_result_refusal(&reply, 2, "invalid_arguments", 27, Some("/content"));
        assert_no_marker(&message.to_string(), "the reply");
        assert!(!reply.close);
        assert_eq!(reply.unrecorded, None);
        let event = assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_report"),
                "invalid_arguments",
                27,
                json!({"field": "/content"}),
            ),
        );
        assert_no_marker(event.payload(), "the event");
        // An extra key reports the object that holds it.
        let reply = world.call(
            "aicd_search",
            json!({"query": "q", MARKER_KEY: MARKER_TEXT}),
        );
        let message = assert_result_refusal(&reply, 3, "invalid_arguments", 27, Some(""));
        assert_no_marker(&message.to_string(), "the reply");
        // A missing property names the property.
        let reply = world.call("aicd_search", json!({}));
        assert_result_refusal(&reply, 4, "invalid_arguments", 27, Some("/query"));
        // Arguments that are absent are the empty object.
        let id = world.next_request_id();
        let reply = world.send(call_request(id, json!({"name": "aicd_search"})));
        assert_result_refusal(&reply, 5, "invalid_arguments", 27, Some("/query"));
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
        assert_eq!(world.kinds(), ["mcp.tool_refused"; 4]);
    }

    #[test]
    fn ori_t_0169_refusal_6_the_byte_cap_is_tested_before_the_schema_and_at_its_boundary() {
        // `{"query":"aaaa"}` is 16 bytes serialized.
        let call = |cap: usize| {
            let mut world = Setup::new().max_arguments_bytes(cap).build("refusal-6-cap");
            world.handshake();
            let reply = world.call("aicd_search", json!({"query": "aaaa"}));
            (reply, world)
        };
        let (reply, mut world) = call(16);
        assert_eq!(wire(&reply)["result"]["content"][0]["text"], "done");
        assert_eq!(world.kinds(), ["mcp.tool_called"]);
        assert_eq!(payload(&world.mcp_events()[0])["arguments_bytes"], 16);
        let (reply, mut world) = call(15);
        assert_result_refusal(&reply, 2, "invalid_arguments", 27, Some(""));
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
        let events = world.mcp_events();
        assert_eq!(payload(&events[0])["field"], "");
        // The cap comes before the schema: an object that is both too large and
        // not valid reports the root and the size, not a property.
        let mut world = Setup::new().max_arguments_bytes(4).build("refusal-6-order");
        world.handshake();
        let reply = world.call("aicd_search", json!({"nope": 1}));
        assert_result_refusal(&reply, 2, "invalid_arguments", 27, Some(""));
    }

    #[test]
    fn ori_t_0169_refusal_6_the_display_of_a_refusal_names_the_fixed_why() {
        let refusal = CallRefusal::InvalidArguments {
            field: "/content".to_owned(),
            why: WHY_ARGUMENTS_TOO_LARGE,
        };
        assert_eq!(
            refusal.to_string(),
            "refused under AICD §27: the arguments are not valid at \"/content\": arguments too large"
        );
        assert_eq!(WHY_ARGUMENTS_TOO_LARGE, "arguments too large");
    }

    #[test]
    fn ori_t_0169_refusal_7_a_coder_naming_another_ticket_contradicts_its_session() {
        let mut world = World::ready("refusal-7a");
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_2()));
        let message = assert_result_refusal(&reply, 2, "claim_contradicts_session", 25, None);
        assert!(!reply.close);
        assert_eq!(reply.unrecorded, None);
        assert_no_marker(&message.to_string(), "the reply");
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_plan_submit"),
                "claim_contradicts_session",
                25,
                json!({}),
            ),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
        // The same holds for every tool that takes a ticket.
        for (name, arguments) in [
            ("aicd_context", json!({"ticket_id": ticket_2().as_str()})),
            (
                "aicd_report",
                json!({"ticket_id": ticket_2().as_str(), "kind": "closing", "content": {}}),
            ),
        ] {
            let reply = world.call(name, arguments);
            assert_result_refusal(&reply, world.next_id, "claim_contradicts_session", 25, None);
        }
    }

    #[test]
    fn ori_t_0169_refusal_7_a_coder_with_no_ticket_is_not_assigned_one() {
        let mut world = Setup::new().ticket(None).build("refusal-7b");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_2()));
        assert_result_refusal(&reply, 2, "ticket_not_assigned", 7, None);
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(
                Some("aicd_plan_submit"),
                "ticket_not_assigned",
                7,
                json!({}),
            ),
        );
        // A tool with no ticket argument is not subject to the rule.
        let reply = world.call("aicd_search", json!({"query": "q"}));
        assert_eq!(wire(&reply)["result"]["content"][0]["text"], "done");
    }

    #[test]
    fn ori_t_0169_refusal_7_only_the_coder_is_held_to_its_own_ticket() {
        // An attended lead bound to a ticket, naming another: the argument passes
        // this check, and the downstream control decides what a lead may do.
        for (role, bound) in [
            (Role::Lead, Some(ticket_1())),
            (Role::Lead, None),
            (Role::Assistant, Some(ticket_1())),
            (Role::Qa, Some(ticket_1())),
            (Role::Documentation, None),
        ] {
            let tool = if ToolScopes::permits(role, Tool::Report) {
                "aicd_report"
            } else {
                "aicd_context"
            };
            let arguments = if tool == "aicd_report" {
                json!({"ticket_id": ticket_2().as_str(), "kind": "blocked", "content": {}})
            } else {
                json!({"ticket_id": ticket_2().as_str()})
            };
            let mut world = Setup::new().role(role).ticket(bound).build("refusal-7c");
            world.handshake();
            let reply = world.call(tool, arguments);
            assert_eq!(
                wire(&reply)["result"]["content"][0]["text"],
                "done",
                "{role} is not limited to its bound ticket"
            );
            assert_eq!(world.ran.load(Ordering::SeqCst), 1);
            // The ticket column is the argument, not the binding's.
            let events = world.mcp_events();
            assert_eq!(events[0].ticket_id(), Some(&ticket_2()));
        }
    }

    #[test]
    fn ori_t_0169_refusal_8_a_call_that_cannot_be_recorded_is_not_made_and_the_refusal_is_attempted()
     {
        // `mcp.tool_called` cannot be appended, and `mcp.tool_refused` can.
        let mut world = World::ready("refusal-8a");
        world.forbid("mcp.tool_called");
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        let message = assert_rpc_refusal(&reply, 2, -32603, "call_not_recorded", 17);
        assert_no_marker(&message.to_string(), "the reply");
        assert!(reply.close, "the connection closes");
        let failure = reply.unrecorded.expect("the append failure is reported");
        assert_eq!(failure.event, AuditEvent::Called);
        assert!(
            failure.detail.contains("a test forbids this kind"),
            "the store error's own text: {}",
            failure.detail
        );
        assert_one_refusal_event(
            &mut world,
            refused_payload_for(Some("aicd_plan_submit"), "call_not_recorded", 17, json!({})),
        );
        assert_eq!(
            world.ran.load(Ordering::SeqCst),
            0,
            "no effect without its record"
        );
    }

    #[test]
    fn ori_t_0169_refusal_8_a_log_that_accepts_nothing_leaves_no_event_and_no_effect() {
        let mut world = World::ready("refusal-8b");
        world.read_only();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_rpc_refusal(&reply, 2, -32603, "call_not_recorded", 17);
        assert!(reply.close);
        let failure = reply.unrecorded.expect("reported");
        assert_eq!(failure.event, AuditEvent::Called);
        assert!(world.events().is_empty());
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    // -------------------------------------------------------------------
    // 4. The order of the checks
    // -------------------------------------------------------------------

    /// The refusal a call got, as its reason.
    fn reason_of(reply: &Reply) -> String {
        let message = wire(reply);
        message["error"]["data"]["reason"]
            .as_str()
            .or_else(|| message["result"]["structuredContent"]["reason"].as_str())
            .expect("a refusal reason")
            .to_owned()
    }

    #[test]
    fn ori_t_0169_check_order_0_before_1() {
        let mut world = Setup::new()
            .db_product(product_b())
            .max_calls(0)
            .build("order-0-1");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_eq!(reason_of(&reply), "product_database_mismatch");
    }

    #[test]
    fn ori_t_0169_check_order_1_before_2() {
        let mut world = Setup::new()
            .max_calls(0)
            .desk(DeskMode::Inactive)
            .build("order-1-2");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_eq!(reason_of(&reply), "rate_limited");
    }

    #[test]
    fn ori_t_0169_check_order_2_before_3() {
        let mut world = Setup::new().desk(DeskMode::Inactive).build("order-2-3");
        world.handshake();
        let reply = world.call(MARKER_NAME, json!({}));
        assert_eq!(reason_of(&reply), "session_not_active");
    }

    // The pair (3, 4) cannot be built: a name that is not one of Ori's tools has
    // no phase and no handler to be unavailable by, so no input fails both, and
    // check 4 is only reachable for a name check 3 accepted.

    #[test]
    fn ori_t_0169_check_order_4_before_5() {
        // A lead may not call aicd_plan_submit, and it has no handler either.
        let mut world = Setup::new()
            .role(Role::Lead)
            .handlers(|_| Handlers::new())
            .build("order-4-5");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        assert_eq!(reason_of(&reply), "tool_not_available");
    }

    #[test]
    fn ori_t_0169_check_order_5_before_6() {
        let mut world = Setup::new().role(Role::Lead).build("order-5-6");
        world.handshake();
        let reply = world.call("aicd_plan_submit", json!({}));
        assert_eq!(reason_of(&reply), "tool_not_permitted_for_role");
    }

    #[test]
    fn ori_t_0169_check_order_6_before_7() {
        // The ticket differs from the bound one and a property is missing.
        let mut world = World::ready("order-6-7");
        let reply = world.call(
            "aicd_plan_submit",
            json!({"ticket_id": ticket_2().as_str(), "plan": "p"}),
        );
        assert_eq!(reason_of(&reply), "invalid_arguments");
    }

    #[test]
    fn ori_t_0169_check_order_7_before_8() {
        // The ticket rule refuses before `mcp.tool_called` is appended, so a log
        // that accepts nothing still records nothing and reports the refusal's own
        // append failure.
        let mut world = World::ready("order-7-8");
        world.read_only();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_2()));
        assert_eq!(reason_of(&reply), "claim_contradicts_session");
        assert_eq!(
            reply.unrecorded.map(|failure| failure.event),
            Some(AuditEvent::Refused),
            "the refusal could not be recorded, and no tool_called was attempted"
        );
    }

    // -------------------------------------------------------------------
    // 5. A successful call
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_a_call_is_recorded_before_its_effect_and_the_handler_gets_its_binding_and_seq() {
        use std::sync::mpsc;

        struct Seen {
            kinds_at_call: Vec<String>,
            last_payload_seq: u64,
            called_seq: u64,
            at: Timestamp,
            ticket: Option<Id>,
            arguments: JsonObject,
            tool: Tool,
            session: Id,
            role: Role,
        }

        let (sender, receiver) = mpsc::channel::<Seen>();
        let mut world = Setup::new()
            .handlers(move |_| {
                let mut handlers = Handlers::new();
                handlers
                    .register(
                        Tool::PlanSubmit,
                        handler(move |db, call, arguments| {
                            let events =
                                EventLog::read_range(db.connection(), 1, u64::MAX).expect("reads");
                            sender
                                .send(Seen {
                                    kinds_at_call: events
                                        .iter()
                                        .map(|event| event.kind().to_owned())
                                        .collect(),
                                    last_payload_seq: events.last().map_or(0, Event::seq),
                                    called_seq: call.called_seq,
                                    at: call.at,
                                    ticket: call.ticket_id.cloned(),
                                    arguments,
                                    tool: call.tool,
                                    session: call.binding.session_id().clone(),
                                    role: call.binding.role(),
                                })
                                .expect("the test is listening");
                            let mut result = CallToolResult::text("the plan is stored");
                            result.structured_content =
                                Some(serde_json::from_value(json!({"stored": true})).expect("map"));
                            Ok(result)
                        }),
                    )
                    .expect("registers");
                handlers
            })
            .build("success");
        world.handshake();
        // The bound ticket, spelled in lower case.
        let lower = ticket_1().as_str().to_ascii_lowercase();
        let reply = world.call(
            "aicd_plan_submit",
            json!({"ticket_id": lower, "plan": "a plan", "declared_scope": ["crates/ori-mcp"]}),
        );
        // The reply is the handler's result, as it is.
        let message = wire(&reply);
        assert_eq!(message["id"], 2);
        assert_eq!(
            message["result"],
            json!({
                "content": [{"type": "text", "text": "the plan is stored"}],
                "structuredContent": {"stored": true},
            })
        );
        assert!(!reply.close && reply.unrecorded.is_none());

        let seen = receiver.try_recv().expect("the handler ran");
        // The record was in the log before the effect ran.
        assert_eq!(seen.kinds_at_call, ["mcp.tool_called"]);
        assert_eq!(seen.last_payload_seq, seen.called_seq);
        assert_eq!(seen.called_seq, 1);
        assert_eq!(seen.at, Timestamp::from_millis(START));
        assert_eq!(seen.tool, Tool::PlanSubmit);
        assert_eq!(seen.session, session());
        assert_eq!(seen.role, Role::Coder);
        // The ticket reaches the handler canonical, in the call and in the
        // arguments.
        assert_eq!(seen.ticket, Some(ticket_1()));
        assert_eq!(seen.arguments["ticket_id"], ticket_1().as_str());
        assert_eq!(seen.arguments["plan"], "a plan");

        // The only event is the call's, by the agent, with the ticket.
        let events = world.mcp_events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.kind(), "mcp.tool_called");
        assert_eq!(event.actor(), &agent());
        assert_eq!(event.product_id(), &product_a());
        assert_eq!(event.at(), Timestamp::from_millis(START));
        assert_eq!(event.ticket_id(), Some(&ticket_1()));
        let serialized = serde_json::to_vec(&json!({
            "ticket_id": lower,
            "plan": "a plan",
            "declared_scope": ["crates/ori-mcp"],
        }))
        .expect("serializes")
        .len();
        assert_eq!(
            payload(event),
            json!({
                "session_id": session().as_str(),
                "tool": "aicd_plan_submit",
                "arguments_bytes": serialized,
            })
        );
    }

    #[test]
    fn ori_t_0169_a_call_of_a_tool_with_no_ticket_argument_carries_the_bindings_ticket() {
        let mut world = World::ready("ticket-column");
        world.call("aicd_search", json!({"query": "q"}));
        assert_eq!(world.mcp_events()[0].ticket_id(), Some(&ticket_1()));
        // A binding with no ticket records none.
        let mut world = Setup::new().ticket(None).build("ticket-column-none");
        world.handshake();
        world.call("aicd_search", json!({"query": "q"}));
        assert_eq!(world.mcp_events()[0].ticket_id(), None);
        // A refusal records the binding's ticket, never the argument's.
        let mut world = Setup::new().role(Role::Lead).build("ticket-column-refusal");
        world.handshake();
        world.call(
            "aicd_plan_submit",
            json!({"ticket_id": ticket_2().as_str(), "plan": "p", "declared_scope": ["a"]}),
        );
        assert_eq!(world.mcp_events()[0].kind(), "mcp.tool_refused");
        assert_eq!(world.mcp_events()[0].ticket_id(), Some(&ticket_1()));
    }

    #[test]
    fn ori_t_0169_the_clock_is_read_once_per_call_and_once_more_for_a_failure() {
        let mut world = Setup::new().step(10).build("clock-reads");
        world.handshake();
        assert_eq!(world.reads.load(Ordering::SeqCst), 0);
        // A refusal: one read.
        world.call(MARKER_NAME, json!({}));
        assert_eq!(world.reads.load(Ordering::SeqCst), 1);
        // A success: one read.
        world.call("aicd_search", json!({"query": "q"}));
        assert_eq!(world.reads.load(Ordering::SeqCst), 2);
        let events = world.mcp_events();
        assert_eq!(events[0].at(), Timestamp::from_millis(START));
        assert_eq!(events[1].at(), Timestamp::from_millis(START + 10));

        // A failure: a second read, for `mcp.tool_failed`.
        let mut world = Setup::new()
            .step(10)
            .handlers(|ran| {
                let mut handlers = Handlers::new();
                handlers
                    .register(
                        Tool::Search,
                        answering(
                            Err(HandlerError::Port(PortError::Unavailable {
                                what: "x".to_owned(),
                            })),
                            ran,
                        ),
                    )
                    .expect("registers");
                handlers
            })
            .build("clock-reads-failure");
        world.handshake();
        world.call("aicd_search", json!({"query": "q"}));
        assert_eq!(world.reads.load(Ordering::SeqCst), 2);
        let events = world.mcp_events();
        assert_eq!(events[0].kind(), "mcp.tool_called");
        assert_eq!(events[0].at(), Timestamp::from_millis(START));
        assert_eq!(events[1].kind(), "mcp.tool_failed");
        assert_eq!(events[1].at(), Timestamp::from_millis(START + 10));
    }

    // -------------------------------------------------------------------
    // 6. A failing call
    // -------------------------------------------------------------------

    /// A world whose `aicd_plan_submit` handler answers `outcome`, after one call
    /// by the bound coder.
    fn failing_world(outcome: Result<CallToolResult, HandlerError>, label: &str) -> (World, Reply) {
        let mut world = Setup::new()
            .step(10)
            .handlers(move |ran| {
                let mut handlers = Handlers::new();
                handlers
                    .register(Tool::PlanSubmit, answering(outcome, ran))
                    .expect("registers");
                handlers
            })
            .build(label);
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        (world, reply)
    }

    /// Asserts `mcp.tool_called` then `mcp.tool_failed` with this failure and
    /// these optional keys, and an error result whose structured content is the
    /// keys and whose single text block is that object serialized.
    fn assert_failure(world: &mut World, reply: &Reply, failure: &str, structured: &Value) {
        let events = world.mcp_events();
        let kinds: Vec<&str> = events.iter().map(Event::kind).collect();
        assert_eq!(kinds, ["mcp.tool_called", "mcp.tool_failed"]);
        let mut expected = json!({
            "session_id": session().as_str(),
            "tool": "aicd_plan_submit",
            "called_seq": events[0].seq(),
            "failure": failure,
        });
        for (key, value) in structured.as_object().expect("an object") {
            expected[key] = value.clone();
        }
        assert_eq!(payload(&events[1]), expected);
        assert_eq!(events[1].actor(), &agent());
        assert_eq!(events[1].ticket_id(), Some(&ticket_1()));
        assert_eq!(events[1].at(), Timestamp::from_millis(START + 10));

        let message = wire(reply);
        assert_eq!(message["id"], 2);
        assert!(
            message.get("error").is_none(),
            "an error result, not a JSON-RPC error"
        );
        assert_eq!(message["result"]["isError"], true);
        assert_eq!(message["result"]["structuredContent"], *structured);
        let blocks = message["result"]["content"].as_array().expect("content");
        assert_eq!(blocks.len(), 1);
        let text: Value =
            serde_json::from_str(blocks[0]["text"].as_str().expect("text")).expect("JSON text");
        assert_eq!(&text, structured);
        assert!(
            !reply.close,
            "a failed effect does not close the connection"
        );
        assert_eq!(reply.unrecorded, None);
    }

    fn reference(section: u8) -> MethodologyRef {
        MethodologyRef::at(section).expect("a section the methodology carries")
    }

    #[test]
    fn ori_t_0169_a_refusal_inside_the_effect_is_called_then_failed_with_its_own_code_and_section()
    {
        // A memory scope refusal (AICD §25).
        let (mut world, reply) = failing_world(
            Err(HandlerError::Refused {
                code: "source_not_in_scope",
                methodology_ref: reference(25),
            }),
            "failure-memory",
        );
        assert_failure(
            &mut world,
            &reply,
            "refused",
            &json!({"reason": "source_not_in_scope", "methodology_ref": methodology(25)}),
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ori_t_0169_a_port_refusal_with_a_code_and_a_reference_carries_both() {
        let upgrade_only = Error::refused(RefusalKind::CategoryDowngrade {
            from: Category::Decisional,
            to: Category::Auto,
        });
        let (mut world, reply) = failing_world(
            Err(HandlerError::Port(PortError::Refused(upgrade_only))),
            "failure-upgrade-only",
        );
        assert_failure(
            &mut world,
            &reply,
            "refused",
            &json!({"reason": "E_UPGRADE_ONLY", "methodology_ref": methodology(11)}),
        );
        // `E_SCOPE_LOCKED`, whose detail is free text that is never copied.
        let locked = Error::refused_with(
            RefusalKind::ScopeLocked {
                module: MARKER_TEXT.to_owned(),
            },
            MARKER_TEXT,
        );
        let (mut world, reply) = failing_world(
            Err(HandlerError::Port(PortError::Refused(locked))),
            "failure-scope-locked",
        );
        assert_failure(
            &mut world,
            &reply,
            "refused",
            &json!({"reason": "E_SCOPE_LOCKED", "methodology_ref": methodology(12)}),
        );
        assert_no_marker(&wire(&reply).to_string(), "the reply");
        for event in world.events() {
            assert_no_marker(event.payload(), "an event");
        }
    }

    #[test]
    fn ori_t_0169_a_port_refusal_with_a_reference_and_no_code_uses_the_fixed_fallback() {
        let transition = Error::refused(RefusalKind::TicketTransition {
            from: TicketState::Filed,
            to: TicketState::Merged,
        });
        let (mut world, reply) = failing_world(
            Err(HandlerError::Port(PortError::Refused(transition))),
            "failure-transition",
        );
        assert_failure(
            &mut world,
            &reply,
            "refused",
            &json!({"reason": "refused", "methodology_ref": methodology(11)}),
        );
    }

    #[test]
    fn ori_t_0169_a_port_refusal_with_no_reference_is_invalid() {
        let malformed = Error::malformed("Id", MARKER_TEXT);
        let (mut world, reply) = failing_world(
            Err(HandlerError::Port(PortError::Refused(malformed))),
            "failure-malformed",
        );
        assert_failure(&mut world, &reply, "invalid", &json!({}));
        assert_no_marker(&wire(&reply).to_string(), "the reply");
    }

    #[test]
    fn ori_t_0169_an_invalid_port_error_names_its_constant_field_and_never_copies_its_why() {
        let (mut world, reply) = failing_world(
            Err(HandlerError::Port(PortError::Invalid {
                field: "ticket_id",
                why: format!("no such ticket {MARKER_TEXT}"),
            })),
            "failure-invalid",
        );
        assert_failure(
            &mut world,
            &reply,
            "invalid",
            &json!({"field": "ticket_id"}),
        );
        assert_no_marker(&wire(&reply).to_string(), "the reply");
        for event in world.events() {
            assert_no_marker(event.payload(), "an event");
        }
    }

    #[test]
    fn ori_t_0169_an_unavailable_port_error_is_unavailable_with_nothing_else() {
        let (mut world, reply) = failing_world(
            Err(HandlerError::Port(PortError::Unavailable {
                what: format!("the desk {MARKER_TEXT}"),
            })),
            "failure-unavailable",
        );
        assert_failure(&mut world, &reply, "unavailable", &json!({}));
        assert_eq!(
            wire(&reply)["result"]["content"][0]["text"],
            "{}",
            "the text is the structured content serialized"
        );
        assert_no_marker(&wire(&reply).to_string(), "the reply");
    }

    #[test]
    fn ori_t_0169_a_handlers_own_error_result_is_passed_through_and_recorded_as_a_failure() {
        let mut own = CallToolResult::error_text("the handler's own words");
        own.structured_content = Some(serde_json::from_value(json!({"detail": 1})).expect("map"));
        let (mut world, reply) = failing_world(Ok(own.clone()), "failure-error-result");
        let events = world.mcp_events();
        let kinds: Vec<&str> = events.iter().map(Event::kind).collect();
        assert_eq!(kinds, ["mcp.tool_called", "mcp.tool_failed"]);
        assert_eq!(
            payload(&events[1]),
            json!({
                "session_id": session().as_str(),
                "tool": "aicd_plan_submit",
                "called_seq": events[0].seq(),
                "failure": "error_result",
            })
        );
        assert_eq!(
            wire(&reply)["result"],
            serde_json::to_value(&own).expect("serializes"),
            "the result as it is"
        );
        assert!(!reply.close && reply.unrecorded.is_none());
    }

    #[test]
    fn ori_t_0169_a_reference_that_does_not_resolve_is_never_written() {
        let unresolvable = MethodologyRef {
            section: 99,
            subsection: None,
        };
        assert!(!unresolvable.resolves());
        let (mut world, reply) = failing_world(
            Err(HandlerError::Refused {
                code: "made_up",
                methodology_ref: unresolvable.clone(),
            }),
            "failure-unresolvable",
        );
        assert_failure(&mut world, &reply, "invalid", &json!({}));
        // The same for a port error that holds one.
        assert_eq!(
            Failure::of_error(&HandlerError::Refused {
                code: "made_up",
                methodology_ref: unresolvable
            })
            .methodology_ref,
            None
        );
    }

    #[test]
    fn ori_t_0169_a_subsection_reference_from_a_handler_reaches_the_event_and_the_reply() {
        let subsection = MethodologyRef::within(24, "3").expect("AICD §24.3 resolves");
        let (mut world, reply) = failing_world(
            Err(HandlerError::Refused {
                code: "as_built_refused",
                methodology_ref: subsection,
            }),
            "failure-subsection",
        );
        assert_failure(
            &mut world,
            &reply,
            "refused",
            &json!({
                "reason": "as_built_refused",
                "methodology_ref": {"section": 24, "subsection": "3"},
            }),
        );
    }

    #[test]
    fn ori_t_0169_every_refusal_surfaced_through_the_seam_carries_its_reference() {
        for (code, section) in [
            ("source_not_in_scope", 25_u8),
            ("evidence_lockout", 25),
            ("E_SCOPE_LOCKED", 12),
            ("E_UPGRADE_ONLY", 11),
            ("not_the_owner", 9),
        ] {
            let (mut world, reply) = failing_world(
                Err(HandlerError::Refused {
                    code,
                    methodology_ref: reference(section),
                }),
                "failure-seam",
            );
            assert_failure(
                &mut world,
                &reply,
                "refused",
                &json!({"reason": code, "methodology_ref": methodology(u64::from(section))}),
            );
        }
    }

    #[test]
    fn ori_t_0169_a_failed_effect_whose_record_cannot_be_appended_is_still_answered_and_closes() {
        // 12 (b): the handler turns the log read-only and fails.
        let mut world = Setup::new()
            .handlers(|_| {
                let mut handlers = Handlers::new();
                handlers
                    .register(
                        Tool::PlanSubmit,
                        handler(|db, _, _| {
                            db.connection()
                                .execute_batch("PRAGMA query_only = ON;")
                                .expect("read-only");
                            Err(HandlerError::Port(PortError::Unavailable {
                                what: "the desk".to_owned(),
                            }))
                        }),
                    )
                    .expect("registers");
                handlers
            })
            .build("fail-closed-b");
        world.handshake();
        let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
        // The reply is still the error result.
        let message = wire(&reply);
        assert_eq!(message["result"]["isError"], true);
        assert_eq!(message["result"]["structuredContent"], json!({}));
        // The only event is the call's.
        assert_eq!(world.kinds(), ["mcp.tool_called"]);
        // The failure is reported, and the connection closes.
        let failure = reply.unrecorded.expect("reported");
        assert_eq!(failure.event, AuditEvent::Failed);
        assert!(reply.close);
    }

    // -------------------------------------------------------------------
    // 7. Payloads hold no agent text
    // -------------------------------------------------------------------

    fn marked_params(name: &str, arguments: Value) -> Value {
        json!({
            "name": name,
            "arguments": arguments,
            "_meta": {"progressToken": MARKER_META, MARKER_KEY: MARKER_META},
            "task": {"ttl": 1, MARKER_KEY: MARKER_TASK},
        })
    }

    /// Sends one call with a marker in the name, the arguments, `_meta`, `task`
    /// and a string request id, and checks nothing of it was recorded or echoed.
    /// `events` is how many events the call must have written, so a scenario that
    /// records nothing cannot pass for the wrong reason.
    fn assert_marked_call_is_clean(
        mut world: World,
        name: &str,
        arguments: Value,
        events: usize,
        what: &str,
    ) {
        world.handshake();
        let reply = world.send(call_request(
            RequestId::String(MARKER_ID.to_owned()),
            marked_params(name, arguments),
        ));
        let all = world.events();
        assert_eq!(all.len(), events, "{what}: the events written");
        for event in &all {
            assert_no_marker(event.payload(), &format!("{what}: an event payload"));
            assert_no_marker(event.kind(), &format!("{what}: an event kind"));
        }
        let mut message = wire(&reply);
        // The id is echoed: JSON-RPC requires it. Nothing else may carry a marker.
        assert_eq!(message["id"], MARKER_ID, "{what}");
        message.as_object_mut().expect("an object").remove("id");
        assert_no_marker(&message.to_string(), &format!("{what}: the reply"));
    }

    fn marked_plan() -> Value {
        json!({
            "ticket_id": ticket_1().as_str(),
            "plan": MARKER_TEXT,
            "declared_scope": [MARKER_TEXT],
            MARKER_KEY: MARKER_TEXT,
        })
    }

    #[test]
    fn ori_t_0169_no_refusal_and_no_event_holds_agent_text() {
        let valid_plan = json!({
            "ticket_id": ticket_1().as_str(),
            "plan": MARKER_TEXT,
            "declared_scope": [MARKER_TEXT],
        });
        // 0: another product's database.
        assert_marked_call_is_clean(
            Setup::new().db_product(product_b()).build("marker-0"),
            "aicd_plan_submit",
            valid_plan.clone(),
            0,
            "refusal 0",
        );
        // 1: a spent budget.
        assert_marked_call_is_clean(
            Setup::new().max_calls(0).build("marker-1"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 1",
        );
        // 2: a session that is not active, and one that cannot be read.
        assert_marked_call_is_clean(
            Setup::new().desk(DeskMode::Inactive).build("marker-2"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 2",
        );
        assert_marked_call_is_clean(
            Setup::new().desk(DeskMode::Unreadable).build("marker-2b"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 2, unreadable",
        );
        // 3: a name that is not one of Ori's tools, with markers in the arguments.
        assert_marked_call_is_clean(
            Setup::new().build("marker-3"),
            MARKER_NAME,
            marked_plan(),
            1,
            "refusal 3",
        );
        // 4: no handler.
        assert_marked_call_is_clean(
            Setup::new().handlers(|_| Handlers::new()).build("marker-4"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 4",
        );
        // 5: a role that does not hold the tool.
        assert_marked_call_is_clean(
            Setup::new().role(Role::Lead).build("marker-5"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 5",
        );
        // 6: a free-text value, an extra key and a number where a string belongs.
        assert_marked_call_is_clean(
            Setup::new().build("marker-6"),
            "aicd_report",
            json!({
                "ticket_id": ticket_1().as_str(),
                "kind": "closing",
                "content": {MARKER_KEY: MARKER_NUMBER},
                MARKER_TEXT: MARKER_TEXT,
            }),
            1,
            "refusal 6, an extra key",
        );
        assert_marked_call_is_clean(
            Setup::new().build("marker-6b"),
            "aicd_report",
            json!({
                "ticket_id": ticket_1().as_str(),
                "kind": "closing",
                "content": {MARKER_KEY: MARKER_NUMBER},
            }),
            1,
            "refusal 6, a map value",
        );
        assert_marked_call_is_clean(
            Setup::new().build("marker-6c"),
            "aicd_search",
            json!({"query": MARKER_TEXT.repeat(20)}),
            1,
            "refusal 6, a long string",
        );
        // 7: another ticket, and no ticket.
        assert_marked_call_is_clean(
            Setup::new().build("marker-7"),
            "aicd_plan_submit",
            json!({
                "ticket_id": ticket_2().as_str(),
                "plan": MARKER_TEXT,
                "declared_scope": [MARKER_TEXT],
            }),
            1,
            "refusal 7",
        );
        assert_marked_call_is_clean(
            Setup::new().ticket(None).build("marker-7b"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 7, no ticket",
        );
        // 8: a call that cannot be recorded, with its refusal recorded.
        let mut world = Setup::new().build("marker-8");
        world.forbid("mcp.tool_called");
        assert_marked_call_is_clean(
            world,
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "refusal 8",
        );
        // A success.
        assert_marked_call_is_clean(
            Setup::new().build("marker-ok"),
            "aicd_plan_submit",
            valid_plan.clone(),
            1,
            "a success",
        );
        // Every failure of an effect.
        for (index, outcome) in [
            Err(HandlerError::Port(PortError::Invalid {
                field: "ticket_id",
                why: MARKER_TEXT.to_owned(),
            })),
            Err(HandlerError::Port(PortError::Unavailable {
                what: MARKER_TEXT.to_owned(),
            })),
            Err(HandlerError::Port(PortError::Refused(Error::refused_with(
                RefusalKind::ScopeLocked {
                    module: MARKER_TEXT.to_owned(),
                },
                MARKER_TEXT,
            )))),
            Err(HandlerError::Port(PortError::Refused(Error::malformed(
                "Id",
                MARKER_TEXT,
            )))),
        ]
        .into_iter()
        .enumerate()
        {
            let world = Setup::new()
                .handlers(move |ran| {
                    let mut handlers = Handlers::new();
                    handlers
                        .register(Tool::PlanSubmit, answering(outcome, ran))
                        .expect("registers");
                    handlers
                })
                .build("marker-failure");
            assert_marked_call_is_clean(
                world,
                "aicd_plan_submit",
                valid_plan.clone(),
                2,
                &format!("failure {index}"),
            );
        }
    }

    #[test]
    fn ori_t_0169_a_call_carrying_task_is_processed_as_a_normal_call_and_nothing_of_it_is_kept() {
        let mut world = Setup::new()
            .handlers(|_| {
                let mut handlers = Handlers::new();
                handlers
                    .register(
                        Tool::Search,
                        handler(|_, _, arguments| {
                            assert_eq!(arguments.len(), 1, "only the arguments reach the handler");
                            Ok(CallToolResult::text("a normal result"))
                        }),
                    )
                    .expect("registers");
                handlers
            })
            .build("task");
        world.handshake();
        let reply = world.send(call_request(
            RequestId::Number(9),
            json!({
                "name": "aicd_search",
                "arguments": {"query": "q"},
                "task": {"ttl": 60_000, MARKER_KEY: MARKER_TASK},
            }),
        ));
        // A CallToolResult, never a task result.
        let message = wire(&reply);
        assert_eq!(
            message["result"],
            json!({"content": [{"type": "text", "text": "a normal result"}]})
        );
        assert!(message["result"].get("task").is_none());
        assert_eq!(world.kinds(), ["mcp.tool_called"]);
        for event in world.events() {
            assert_no_marker(event.payload(), "an event");
        }
    }

    // -------------------------------------------------------------------
    // 9. The call budget
    // -------------------------------------------------------------------

    /// A second connection of the same session: another server on the same
    /// database, the same budget and the same clock.
    fn another_connection(world: &World) -> Server {
        Server::new(
            world.binding.clone(),
            ServerConfig::default(),
            Arc::clone(&world.budget),
            Box::new(FakeClock {
                now: Arc::clone(&world.clock),
                step: 0,
                reads: Arc::clone(&world.reads),
            }),
            Box::new(FakeDesk {
                mode: Arc::clone(&world.desk),
                asked: Arc::clone(&world.desk_asked),
            }),
            all_phase_1_handlers(&world.ran),
        )
    }

    fn search(world: &mut World) -> Reply {
        world.call("aicd_search", json!({"query": "q"}))
    }

    fn is_done(reply: &Reply) -> bool {
        wire(reply)["result"]["content"][0]["text"] == "done"
    }

    #[test]
    fn ori_t_0169_120_calls_pass_the_121st_is_refused_and_closes_and_the_window_resets() {
        let mut world = World::ready("rate-120");
        for _ in 0..120 {
            let reply = search(&mut world);
            assert!(is_done(&reply));
            assert!(!reply.close);
        }
        let reply = search(&mut world);
        assert_rpc_refusal(&reply, world.next_id, RATE_LIMITED_CODE, "rate_limited", 22);
        assert!(reply.close, "the 121st call closes the connection");
        assert_eq!(reply.unrecorded, None);
        let kinds = world.kinds();
        assert_eq!(kinds.len(), 121);
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| *kind == "mcp.tool_called")
                .count(),
            120
        );
        assert_eq!(kinds.last().map(String::as_str), Some("mcp.tool_refused"));
        assert_eq!(world.ran.load(Ordering::SeqCst), 120);
        // Still refused one millisecond before the window ends, and passes at its
        // end: a fixed window of 60 000 ms from its first call.
        world.clock.store(START + 59_999, Ordering::SeqCst);
        assert!(!is_done(&search(&mut world)));
        world.clock.store(START + 60_000, Ordering::SeqCst);
        assert!(is_done(&search(&mut world)));
        // The new window began at its first call: 120 calls from there.
        for _ in 0..119 {
            assert!(is_done(&search(&mut world)));
        }
        assert!(!is_done(&search(&mut world)));
    }

    #[test]
    fn ori_t_0169_a_refused_call_is_charged_too_so_a_session_that_keeps_calling_stays_refused() {
        let mut world = Setup::new().max_calls(1).build("rate-charged");
        world.handshake();
        assert!(is_done(&search(&mut world)));
        for _ in 0..5 {
            assert!(!is_done(&search(&mut world)));
        }
        assert_eq!(
            world.budget.lock().expect("the budget").calls_in_window,
            6,
            "six calls charged, refused or not"
        );
    }

    #[test]
    fn ori_t_0169_a_clock_that_steps_back_starts_a_new_window() {
        let mut world = World::ready("rate-step-back");
        for _ in 0..120 {
            assert!(is_done(&search(&mut world)));
        }
        assert!(!is_done(&search(&mut world)), "the window is spent");
        // The clock steps back before the window's start: a new window begins.
        world.clock.store(START - 1, Ordering::SeqCst);
        assert!(is_done(&search(&mut world)));
        // And that window runs from the earlier time.
        world.clock.store(START - 1 + 59_999, Ordering::SeqCst);
        for _ in 0..119 {
            assert!(is_done(&search(&mut world)));
        }
        assert!(!is_done(&search(&mut world)));
    }

    #[test]
    fn ori_t_0169_the_window_arithmetic_is_signed_and_saturating() {
        let mut window = RateWindow::new(2);
        let at = Timestamp::from_millis;
        // A window that began at the end of time: no overflow, and a clock at that
        // instant is not "past 60 000 ms later" in a way that panics.
        window.window_start = Some(at(i64::MAX - 10));
        assert!(window.charge(at(i64::MAX)));
        // A window that began at the beginning of time, and a clock far later.
        window.window_start = Some(at(i64::MIN));
        window.calls_in_window = 2;
        assert!(
            window.charge(at(i64::MAX)),
            "a far later call starts a new window"
        );
        assert_eq!(window.calls_in_window, 1);
        // A clock at the very beginning of time, before a window that began later.
        window.window_start = Some(at(0));
        window.calls_in_window = 2;
        assert!(
            window.charge(at(i64::MIN)),
            "stepping back starts a new window"
        );
        assert_eq!(window.window_start, Some(at(i64::MIN)));
        // A budget of zero admits nothing; the count saturates rather than wraps.
        let mut none = RateWindow::new(0);
        assert!(!none.charge(at(0)));
        none.calls_in_window = u32::MAX;
        assert!(!none.charge(at(1)));
        assert_eq!(none.calls_in_window, u32::MAX);
        // An exact boundary.
        let mut edge = RateWindow::new(1);
        assert!(edge.charge(at(100)));
        assert!(!edge.charge(at(100 + RATE_WINDOW_MILLIS - 1)));
        assert!(edge.charge(at(100 + RATE_WINDOW_MILLIS)));
    }

    #[test]
    fn ori_t_0169_two_servers_on_one_budget_share_one_window() {
        let mut world = Setup::new().max_calls(3).build("rate-shared");
        world.handshake();
        assert!(is_done(&search(&mut world)));
        assert!(is_done(&search(&mut world)));
        // A new connection of the same session: reconnecting does not reset it.
        let mut second = another_connection(&world);
        second.receive(&mut world.db, Ok(initialize_request(0, "2025-11-25")));
        second.receive(&mut world.db, Ok(initialized_notification()));
        let params = json!({"name": "aicd_search", "arguments": {"query": "q"}});
        let reply = second.receive(
            &mut world.db,
            Ok(call_request(RequestId::Number(1), params.clone())),
        );
        assert!(is_done(&reply), "the third call of the window");
        let reply = second.receive(
            &mut world.db,
            Ok(call_request(RequestId::Number(2), params.clone())),
        );
        assert_rpc_refusal(&reply, 2, RATE_LIMITED_CODE, "rate_limited", 22);
        assert!(reply.close);
        // The first connection shares the same spent window.
        let reply = search(&mut world);
        assert_eq!(reason_of(&reply), "rate_limited");
        // A third connection has its first call refused and closed the same way.
        let mut third = another_connection(&world);
        third.receive(&mut world.db, Ok(initialize_request(0, "2025-11-25")));
        third.receive(&mut world.db, Ok(initialized_notification()));
        let reply = third.receive(
            &mut world.db,
            Ok(call_request(RequestId::Number(1), params)),
        );
        assert_eq!(reason_of(&reply), "rate_limited");
        assert!(reply.close);
    }

    #[test]
    fn ori_t_0169_calls_over_successive_connections_append_at_most_the_limit_plus_one_row_each() {
        let mut world = Setup::new().build("rate-flood");
        let params = json!({"name": "aicd_search", "arguments": {"query": "q"}});
        let (mut sent, mut connections) = (0_usize, 0_usize);
        while sent < 10_000 {
            let mut server = another_connection(&world);
            connections += 1;
            server.receive(&mut world.db, Ok(initialize_request(0, "2025-11-25")));
            server.receive(&mut world.db, Ok(initialized_notification()));
            while sent < 10_000 {
                sent += 1;
                let id = RequestId::Number(i64::try_from(sent).expect("small"));
                let reply = server.receive(&mut world.db, Ok(call_request(id, params.clone())));
                if reply.close {
                    break;
                }
            }
        }
        let rows = world.mcp_events().len();
        assert!(
            rows <= 120 + connections,
            "{rows} rows for {connections} connections"
        );
        assert_eq!(world.ran.load(Ordering::SeqCst), 120, "only the limit ran");
        assert!(connections > 1, "the refusals closed the connections");
        assert_eq!(sent, 10_000);
    }

    #[test]
    fn ori_t_0169_a_poisoned_budget_counts_as_an_exhausted_window() {
        let mut world = World::ready("rate-poisoned");
        let budget = Arc::clone(&world.budget);
        let poisoner = std::thread::spawn(move || {
            let _held = budget.lock().expect("the budget");
            panic!("poisoning the budget on purpose");
        });
        assert!(poisoner.join().is_err());
        assert!(world.budget.lock().is_err(), "the lock is poisoned");
        let reply = search(&mut world);
        assert_rpc_refusal(&reply, world.next_id, RATE_LIMITED_CODE, "rate_limited", 22);
        assert!(reply.close, "fail closed");
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    // -------------------------------------------------------------------
    // 10. ORI-P1-033: every refusal carries a reference that resolves
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
        CallRefusal, every_call_refusal, call_refusal_expectation, (&'static str, u8);
        Tool { .. } => CallRefusal::Tool(ToolRefusal::SessionNotActive),
            ("session_not_active", 17);
        ProductDatabaseMismatch => CallRefusal::ProductDatabaseMismatch,
            ("product_database_mismatch", 26);
        SessionStateUnavailable => CallRefusal::SessionStateUnavailable,
            ("session_state_unavailable", 17);
        RateLimited => CallRefusal::RateLimited, ("rate_limited", 22);
        UnknownTool => CallRefusal::UnknownTool, ("unknown_tool", 17);
        ToolNotAvailable => CallRefusal::ToolNotAvailable, ("tool_not_available", 17);
        InvalidArguments { .. } => CallRefusal::InvalidArguments {
            field: "/content".to_owned(),
            why: WHY_ARGUMENTS_TOO_LARGE,
        }, ("invalid_arguments", 27);
        CallNotRecorded => CallRefusal::CallNotRecorded, ("call_not_recorded", 17);
    }

    /// One sample of each of `scopes.rs`'s refusals, which ride in
    /// `CallRefusal::Tool`.
    fn every_tool_refusal() -> Vec<ToolRefusal> {
        use crate::scopes::ServerName;
        let server = ServerName::parse("github").expect("a server name");
        vec![
            ToolRefusal::ToolNotPermittedForRole {
                role: Role::Coder,
                tool: Tool::Evidence,
            },
            ToolRefusal::ServerNotInProject {
                server: server.clone(),
            },
            ToolRefusal::ServerReachNotAcknowledged {
                server: server.clone(),
            },
            ToolRefusal::ServerNotExposedToRole {
                role: Role::Coder,
                server,
            },
            ToolRefusal::TicketNotAssigned { ticket: ticket_1() },
            ToolRefusal::ClaimContradictsSession {
                claim: TICKET_ARGUMENT,
            },
            ToolRefusal::SessionNotActive,
        ]
    }

    #[test]
    fn ori_p1_033_every_call_refusal_carries_a_methodology_ref_that_resolves() {
        let refusals = every_call_refusal();
        assert_eq!(refusals.len(), 8, "the eight variants of CallRefusal");
        let mut codes = std::collections::HashSet::new();
        for refusal in &refusals {
            let (code, section) = call_refusal_expectation(refusal);
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
    fn ori_p1_033_every_refusal_that_rides_in_call_refusal_tool_resolves_and_keeps_its_code() {
        let mut codes = std::collections::HashSet::new();
        for inner in every_tool_refusal() {
            let refusal = CallRefusal::Tool(inner.clone());
            assert_eq!(refusal.code(), inner.code());
            assert_eq!(refusal.methodology_ref(), inner.methodology_ref());
            assert!(refusal.methodology_ref().resolves(), "{inner:?}");
            assert_eq!(refusal.to_string(), inner.to_string());
            // Each reaches the agent with its reference, one way or the other.
            let message = serde_json::to_value(refusal_message(RequestId::Number(1), &refusal))
                .expect("serializes");
            let carried = message["error"]["data"]["methodology_ref"]
                .as_object()
                .or_else(|| message["result"]["structuredContent"]["methodology_ref"].as_object())
                .expect("a reference");
            assert_eq!(carried["section"], inner.methodology_ref().section);
            codes.insert(inner.code());
        }
        assert_eq!(codes.len(), 7);
    }

    #[test]
    fn ori_t_0169_the_refusals_reach_the_wire_on_the_surface_the_decisions_name() {
        let surface = |refusal: CallRefusal| match refusal.surface() {
            Surface::Rpc(code) => Some(code),
            Surface::Result => None,
        };
        assert_eq!(surface(CallRefusal::ProductDatabaseMismatch), Some(-32603));
        assert_eq!(surface(CallRefusal::RateLimited), Some(-32010));
        assert_eq!(
            surface(CallRefusal::Tool(ToolRefusal::SessionNotActive)),
            Some(-32011)
        );
        assert_eq!(surface(CallRefusal::SessionStateUnavailable), Some(-32603));
        assert_eq!(surface(CallRefusal::UnknownTool), Some(-32602));
        assert_eq!(surface(CallRefusal::ToolNotAvailable), Some(-32602));
        assert_eq!(
            surface(CallRefusal::Tool(ToolRefusal::ToolNotPermittedForRole {
                role: Role::Coder,
                tool: Tool::Evidence
            })),
            Some(-32602)
        );
        assert_eq!(
            surface(CallRefusal::InvalidArguments {
                field: String::new(),
                why: WHY_ARGUMENTS_TOO_LARGE
            }),
            None
        );
        assert_eq!(
            surface(CallRefusal::Tool(ToolRefusal::ClaimContradictsSession {
                claim: TICKET_ARGUMENT
            })),
            None
        );
        assert_eq!(
            surface(CallRefusal::Tool(ToolRefusal::TicketNotAssigned {
                ticket: ticket_1()
            })),
            None
        );
        assert_eq!(surface(CallRefusal::CallNotRecorded), Some(-32603));
        // -32042 is MCP's, and Ori's own codes avoid it and stay in the
        // implementation-defined range.
        for code in [RATE_LIMITED_CODE, SESSION_NOT_ACTIVE_CODE] {
            assert!((-32099..=-32000).contains(&code));
            assert_ne!(code, -32042);
        }
        assert_ne!(RATE_LIMITED_CODE, SESSION_NOT_ACTIVE_CODE);
    }

    #[test]
    fn ori_t_0169_the_closing_refusals_are_the_decided_ones() {
        let closes: Vec<bool> = every_call_refusal()
            .iter()
            .map(CallRefusal::closes_connection)
            .collect();
        // Tool(SessionNotActive), ProductDatabaseMismatch, SessionStateUnavailable,
        // RateLimited, UnknownTool, ToolNotAvailable, InvalidArguments,
        // CallNotRecorded, in the order of the table above.
        assert_eq!(closes, [true, true, true, true, false, false, false, true]);
        assert!(
            !CallRefusal::Tool(ToolRefusal::ToolNotPermittedForRole {
                role: Role::Coder,
                tool: Tool::Evidence
            })
            .closes_connection()
        );
    }

    // -------------------------------------------------------------------
    // 11. serve
    // -------------------------------------------------------------------

    fn lines(items: &[Value]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for item in items {
            bytes.extend_from_slice(item.to_string().as_bytes());
            bytes.push(b'\n');
        }
        bytes
    }

    fn rpc(id: i64, method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    fn run_serve(
        world: &mut World,
        input: Vec<u8>,
        cap: usize,
    ) -> (Result<(), FrameError>, Vec<Value>) {
        let mut reader = LineReader::with_max_line_bytes(Cursor::new(input), cap);
        let mut output: Vec<u8> = Vec::new();
        let result = world.server.serve(&mut world.db, &mut reader, &mut output);
        let replies = String::from_utf8(output)
            .expect("UTF-8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect();
        (result, replies)
    }

    #[test]
    fn ori_t_0169_serve_runs_a_whole_session_to_end_of_input() {
        let mut world = Setup::new().build("serve-session");
        let input = lines(&[
            rpc(
                1,
                "initialize",
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name": "t", "version": "1"},
                }),
            ),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            rpc(2, "tools/list", json!({})),
            rpc(
                3,
                "tools/call",
                json!({"name": "aicd_search", "arguments": {"query": "q"}}),
            ),
            rpc(
                4,
                "tools/call",
                json!({"name": "aicd_nothing", "arguments": {}}),
            ),
        ]);
        let (result, replies) = run_serve(&mut world, input, crate::protocol::MAX_LINE_BYTES);
        assert_eq!(result, Ok(()));
        assert_eq!(
            replies.len(),
            4,
            "the notification has no reply: {replies:?}"
        );
        assert_eq!(replies[0]["id"], 1);
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(replies[1]["id"], 2);
        assert_eq!(
            replies[1]["result"]["tools"]
                .as_array()
                .expect("tools")
                .len(),
            5,
            "a coder holds five phase 1 tools"
        );
        assert_eq!(replies[2]["id"], 3);
        assert_eq!(replies[2]["result"]["content"][0]["text"], "done");
        assert_eq!(replies[3]["id"], 4);
        assert_eq!(replies[3]["error"]["data"]["reason"], "unknown_tool");
        assert_eq!(world.kinds(), ["mcp.tool_called", "mcp.tool_refused"]);
    }

    #[test]
    fn ori_t_0169_serve_answers_a_too_long_line_once_without_an_id_and_returns_the_frame_error() {
        let mut world = Setup::new().build("serve-too-long");
        let mut input = lines(&[rpc(1, "ping", json!({}))]);
        input.extend(std::iter::repeat_n(b'x', 200));
        input.push(b'\n');
        input.extend(lines(&[rpc(2, "ping", json!({}))]));
        let (result, replies) = run_serve(&mut world, input, 80);
        assert_eq!(result, Err(FrameError::TooLong { limit: 80 }));
        assert_eq!(replies.len(), 2, "the ping, then one answer: {replies:?}");
        assert_eq!(replies[0]["id"], 1);
        assert!(replies[1].get("id").is_none(), "no id: {replies:?}");
        assert_eq!(replies[1]["error"]["code"], -32600);
        // The line after it was never read: the reader stays refused.
    }

    #[test]
    fn ori_t_0169_serve_answers_a_bad_line_without_an_id_and_reads_on() {
        let mut world = World::ready("serve-bad-line");
        let mut input = b"{not json\n".to_vec();
        input.extend(lines(&[rpc(5, "ping", json!({}))]));
        let (result, replies) = run_serve(&mut world, input, crate::protocol::MAX_LINE_BYTES);
        assert_eq!(result, Ok(()));
        assert_eq!(replies.len(), 2);
        assert!(replies[0].get("id").is_none());
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[1]["id"], 5);
    }

    #[test]
    fn ori_t_0169_serve_returns_once_a_reply_that_closes_has_been_written() {
        let mut world = Setup::new().max_calls(0).build("serve-close");
        world.handshake();
        let input = lines(&[
            rpc(
                1,
                "tools/call",
                json!({"name": "aicd_search", "arguments": {"query": "q"}}),
            ),
            rpc(2, "ping", json!({})),
        ]);
        let (result, replies) = run_serve(&mut world, input, crate::protocol::MAX_LINE_BYTES);
        assert_eq!(result, Ok(()));
        assert_eq!(replies.len(), 1, "the line after the close is not read");
        assert_eq!(replies[0]["error"]["data"]["reason"], "rate_limited");
    }

    #[test]
    fn ori_t_0169_serve_returns_a_write_failure_as_its_frame_error() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "the peer is gone",
                ))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut world = Setup::new().build("serve-write");
        let mut reader = LineReader::new(Cursor::new(lines(&[rpc(1, "ping", json!({}))])));
        let result = world.server.serve(&mut world.db, &mut reader, &mut Broken);
        assert!(
            matches!(
                result,
                Err(FrameError::Io {
                    kind: io::ErrorKind::BrokenPipe,
                    ..
                })
            ),
            "{result:?}"
        );
        // A frame error that ends the stream is returned even when its answer
        // cannot be written.
        let mut reader = LineReader::with_max_line_bytes(Cursor::new(vec![b'x'; 100]), 10);
        let result = world.server.serve(&mut world.db, &mut reader, &mut Broken);
        assert_eq!(result, Err(FrameError::TooLong { limit: 10 }));
    }

    // -------------------------------------------------------------------
    // 12. The fail-closed append paths
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_a_refusal_whose_record_cannot_be_appended_is_still_refused_the_same_way() {
        // (a) A log that accepts nothing, and a call that fails check 5.
        let call = |read_only: bool| {
            let mut world = Setup::new().role(Role::Lead).build("fail-closed-a");
            world.handshake();
            if read_only {
                world.read_only();
            }
            let reply = world.call("aicd_plan_submit", plan_arguments(&ticket_1()));
            (reply, world)
        };
        let (writable, mut writable_world) = call(false);
        let (read_only, mut read_only_world) = call(true);
        // The same reply.
        assert_eq!(wire(&read_only), wire(&writable));
        assert_rpc_refusal(&read_only, 2, -32602, "tool_not_permitted_for_role", 17);
        // No handler ran, and no event was added.
        assert_eq!(read_only_world.ran.load(Ordering::SeqCst), 0);
        assert!(read_only_world.events().is_empty());
        assert_eq!(writable_world.kinds(), ["mcp.tool_refused"]);
        // The failure is reported, and the connection closes: the other does not.
        let failure = read_only.unrecorded.expect("reported");
        assert_eq!(failure.event, AuditEvent::Refused);
        assert!(!failure.detail.is_empty());
        assert!(read_only.close);
        assert!(!writable.close);
        assert_eq!(writable.unrecorded, None);
    }

    #[test]
    fn ori_t_0169_a_failed_refusal_append_does_not_let_the_call_go_on() {
        // Every refusal check, with a log that accepts nothing: none reaches a
        // handler, and each closes.
        for (label, setup, tool, arguments) in [
            (
                "rate",
                Setup::new().max_calls(0),
                "aicd_plan_submit",
                plan_arguments(&ticket_1()),
            ),
            (
                "session",
                Setup::new().desk(DeskMode::Inactive),
                "aicd_plan_submit",
                plan_arguments(&ticket_1()),
            ),
            ("unknown", Setup::new(), "aicd_nothing", json!({})),
            (
                "role",
                Setup::new().role(Role::Lead),
                "aicd_plan_submit",
                plan_arguments(&ticket_1()),
            ),
            ("arguments", Setup::new(), "aicd_plan_submit", json!({})),
            (
                "ticket",
                Setup::new(),
                "aicd_plan_submit",
                plan_arguments(&ticket_2()),
            ),
        ] {
            let mut world = setup.build(label);
            world.handshake();
            world.read_only();
            let reply = world.call(tool, arguments);
            assert_eq!(
                reply.unrecorded.as_ref().map(|failure| failure.event),
                Some(AuditEvent::Refused),
                "{label}"
            );
            assert!(reply.close, "{label}");
            assert_eq!(world.ran.load(Ordering::SeqCst), 0, "{label}: no effect");
            assert!(world.events().is_empty(), "{label}");
        }
    }

    // -------------------------------------------------------------------
    // 13 to 15, and the rest of the surface
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_a_database_of_another_product_gets_nothing_appended_on_any_call() {
        // 13: whatever the call is, and wherever it would have failed, nothing
        // reaches another product's log.
        let mut world = Setup::new().db_product(product_b()).build("other-product");
        world.handshake();
        for (name, arguments) in [
            ("aicd_search", json!({"query": "q"})),
            ("aicd_nothing", json!({})),
            ("aicd_plan_submit", json!({})),
        ] {
            let reply = world.call(name, arguments);
            assert_eq!(reason_of(&reply), "product_database_mismatch");
            assert!(reply.close);
            assert!(reply.unrecorded.is_some());
        }
        assert!(world.events().is_empty());
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_server_is_send_and_the_ports_are_objects() {
        fn assert_send<T: Send>() {}
        assert_send::<Server>();
        assert_send::<Handlers>();
        assert_send::<Box<dyn Clock>>();
        assert_send::<Box<dyn SessionDesk>>();
        assert_send::<Box<dyn ToolHandler>>();
        // Both new ports are usable as trait objects.
        let _: Option<&mut dyn Clock> = None;
        let _: Option<&mut dyn SessionDesk> = None;
    }

    #[test]
    fn ori_t_0169_a_second_registration_of_a_tool_never_replaces_the_first() {
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let mut handlers = Handlers::new();
        assert!(
            handlers
                .register(Tool::Search, counting_handler(&first))
                .is_ok()
        );
        assert_eq!(
            handlers.register(Tool::Search, counting_handler(&second)),
            Err(Tool::Search),
            "the duplicate is refused and names the tool"
        );
        assert!(handlers.is_registered(Tool::Search));
        assert!(!handlers.is_registered(Tool::Context));
        // A tool outside phase 1 may be registered, because a later phase does.
        assert!(
            handlers
                .register(Tool::GateResult, counting_handler(&first))
                .is_ok()
        );

        let mut world = Setup::new()
            .handlers(move |_| handlers)
            .build("register-twice");
        world.handshake();
        assert!(is_done(&search(&mut world)));
        assert_eq!(first.load(Ordering::SeqCst), 1, "the first handler served");
        assert_eq!(second.load(Ordering::SeqCst), 0, "the second never did");
        // Registered, outside phase 1: not listed, not dispatched.
        assert_eq!(listed_names(&mut world), ["aicd_search"]);
    }

    #[test]
    fn ori_t_0169_the_binding_is_the_engines_and_makes_the_caller() {
        let binding = Binding::new(
            session(),
            identity(),
            Role::Documentation,
            product_a(),
            Some(ticket_1()),
        );
        assert_eq!(binding.session_id(), &session());
        assert_eq!(binding.identity(), &identity());
        assert_eq!(binding.role(), Role::Documentation);
        assert_eq!(binding.product_id(), &product_a());
        assert_eq!(binding.ticket_id(), Some(&ticket_1()));
        assert_eq!(
            binding.caller(),
            Caller::new(identity(), Role::Documentation, product_a())
        );
        let unattended = Binding::new(session(), identity(), Role::Qa, product_a(), None);
        assert_eq!(unattended.ticket_id(), None);
    }

    #[test]
    fn ori_t_0169_every_event_is_written_by_the_agent_identity_never_the_system() {
        let mut world = World::ready("actor");
        search(&mut world);
        world.call(MARKER_NAME, json!({}));
        world.call("aicd_plan_submit", json!({}));
        let events = world.mcp_events();
        assert_eq!(events.len(), 3);
        for event in &events {
            assert_eq!(event.actor(), &Actor::Agent(identity()));
            assert!(!event.actor().is_system());
            assert_eq!(event.product_id(), &product_a());
        }
    }

    #[test]
    fn ori_t_0169_a_tool_the_agent_cannot_reach_by_name_is_not_reached_by_a_case_or_a_prefix() {
        // The proxied-name shape is not an alias for one of Ori's tools.
        let mut world = World::ready("alias");
        for name in [
            "aicd_search ",
            "aicd_search\n",
            "aicd__search",
            "github__search",
            "",
        ] {
            let reply = world.call(name, json!({"query": "q"}));
            assert_eq!(reason_of(&reply), "unknown_tool", "{name:?}");
        }
        assert_eq!(world.ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ori_t_0169_the_largest_valid_arguments_of_every_phase_1_tool_pass_check_6() {
        let bound = Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("an id");
        for tool in Tool::ALL.into_iter().filter(|tool| tool.in_phase_1()) {
            let schema = tools::spec(tool).schema();
            let arguments = tools::largest_valid_arguments(schema);
            let role = Role::ALL
                .iter()
                .copied()
                .find(|role| ToolScopes::permits(*role, tool))
                .expect("a role holds the tool");
            let mut world = Setup::new()
                .role(role)
                .ticket(Some(bound.clone()))
                .build("largest");
            world.handshake();
            let reply = world.call(tool.wire_name(), arguments);
            assert!(is_done(&reply), "{tool}: refused its own largest object");
            assert_eq!(world.ran.load(Ordering::SeqCst), 1, "{tool}");
            let events = world.mcp_events();
            assert_eq!(
                payload(&events[0])["arguments_bytes"],
                schema.worst_case_bytes(),
                "{tool}"
            );
            assert!(schema.worst_case_bytes() <= DEFAULT_MAX_ARGUMENTS_BYTES);
        }
    }

    #[test]
    fn ori_t_0169_the_audit_failure_and_the_handler_error_say_what_they_are() {
        let failure = AuditFailure {
            event: AuditEvent::Failed,
            detail: "disk full".to_owned(),
        };
        assert_eq!(
            failure.to_string(),
            "mcp.tool_failed could not be recorded: disk full"
        );
        assert_eq!(AuditEvent::Refused.kind(), "mcp.tool_refused");
        assert_eq!(AuditEvent::Called.kind(), "mcp.tool_called");
        let from_port: HandlerError = PortError::Unavailable {
            what: "the desk".to_owned(),
        }
        .into();
        assert!(matches!(from_port, HandlerError::Port(_)));
        assert!(std::error::Error::source(&from_port).is_some());
        let refused = HandlerError::Refused {
            code: "source_not_in_scope",
            methodology_ref: reference(25),
        };
        assert_eq!(
            refused.to_string(),
            "refused under AICD §25: source_not_in_scope"
        );
        assert!(std::error::Error::source(&refused).is_none());
        assert_eq!(ServerConfig::default().max_arguments_bytes, 524_288);
    }
}
