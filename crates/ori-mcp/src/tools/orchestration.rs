//! The orchestration tools `aicd_plan_submit` and `aicd_escalate`: AICD §12,
//! AICD §17, `spec/API_SPEC.md` section 3.
//!
//! Two handlers, [`crate::tools::orchestration::PlanSubmitHandler`] and
//! [`crate::tools::orchestration::EscalateHandler`], that the engine registers
//! with [`crate::tools::Handlers::register`]. The effects live in
//! `ori-orchestrator`, which this crate never depends on, so each handler reaches
//! them through a port the engine implements (ORI-T-0070):
//! [`crate::ports::PlanDesk`] and [`crate::ports::EscalationDesk`]. A handler is
//! the thin layer between S2's pipeline and its desk: it reads the arguments,
//! passes every agent text through the sanitization barrier, reads the declared
//! scope strictly, hands the desk typed values, and writes the reply. **No event
//! of its own, and no text of its own but fixed labels.**
//!
//! | Tool | Roles | API_SPEC's words | Takes | Returns |
//! |---|---|---|---|---|
//! | `aicd_plan_submit` | coder | "Stores the plan; lock check; may return `E_SCOPE_LOCKED`" | `ticket_id`, `plan`, `declared_scope` | the plan's id and how many modules it newly claimed |
//! | `aicd_escalate` | lead, coder | "Opens an escalation; suspends the session if configured" | `ticket_id`, `trigger`, `question`, `recommendation`, `evidence` | the escalation's id and whether the session was suspended |
//!
//! Neither is read-only, neither is open-world, and both are built in phase 1
//! (`spec/API_SPEC.md` section 3). The tool's input schema, its bounds included,
//! is the catalogue's ([`crate::tools::catalogue`]).
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
//!   participant H as PlanSubmitHandler or EscalateHandler
//!   participant B as sanitize_text and ModulePath::parse
//!   participant D as PlanDesk or EscalationDesk, the engine's
//!   participant L as the product's log
//!   A->>S: tools/call aicd_plan_submit or aicd_escalate
//!   S->>S: checks 0 to 7
//!   S->>L: append mcp.tool_called
//!   S->>H: call(db, BoundCall, arguments)
//!   H->>B: every text, and the declared scope
//!   alt an argument the handler rejects
//!     B-->>H: a blank text, a path that is not a module path, or a quote
//!     H-->>S: Port Invalid, with the field
//!   else a text over the schema's bound
//!     B-->>H: truncated
//!     H-->>S: Port Unavailable, fail closed
//!   else typed values
//!     B-->>H: SanitizedField and ModulePath
//!     H->>D: submit or open(db, binding, ticket, the values)
//!     alt a refusal, E_SCOPE_LOCKED or Denied or Invalid
//!       D-->>H: the PortError, made before the desk's first append
//!       H-->>S: that error as it is
//!     else stored or opened
//!       D->>L: the desk's own rows
//!       D-->>H: PlanStored or EscalationOpened
//!       H-->>S: Ok, one text block of identifiers and counts
//!     end
//!   end
//!   S->>L: append mcp.tool_failed, on every error
//!   S-->>A: the reply
//! ```
//!
//! # The decisions this module carries
//!
//! **The ticket is the call's `ticket_id` argument.** For a coder, S2's check 7
//! makes it the bound ticket. For a lead nobody has checked it, and the desk does:
//! a ticket the product does not hold is `Invalid { field: "ticket_id" }`
//! ([`crate::ports::PlanDesk`]'s and [`crate::ports::EscalationDesk`]'s
//! contracts). A call that reaches a handler with no ticket is `Invalid` too, and
//! S2's checks make that unreachable.
//!
//! **Every agent text passes the barrier before a desk sees it, and a desk takes
//! nothing else.** The plan, the question, the recommendation and every evidence
//! text go through `ori_memory::barrier::sanitize_text` at the schema's own bound,
//! and the desks take `ori_memory::barrier::SanitizedField`, which only that
//! function makes, so the compiler checks that no raw `String` reaches them. The
//! text is **not trimmed**: a control character becomes U+FFFD, and nothing else
//! changes. Three things follow:
//!
//! - *A text over the bound is `Unavailable`, and nothing is stored.* S2's schema
//!   counts characters as `sanitize_text` does, so a schema-valid call is never
//!   over a bound. A direct call can be, and a handler that stored what
//!   `sanitize_text` kept would store less than the agent sent. It fails closed,
//!   with a fixed `what` (a named constant) that S2 never copies into a reply or
//!   an event.
//! - *A text that is blank once sanitized is `Invalid`, with its field.*
//!   `Escalation::raise` and `Finding::new` refuse blanks, so the handler says so
//!   first, and says which argument. Judged on the sanitized text, because that is
//!   what the desk's component judges. An empty string never passes S2's schema,
//!   so this is spaces only, in practice.
//! - *The `field` of every `Invalid` is the argument's bare name* (`ticket_id`,
//!   `plan`, `declared_scope`, `trigger`, `question`, `recommendation`,
//!   `evidence`), never an index and never the agent's text, and the `why` is a
//!   fixed sentence, a named constant. S2's reply carries the field and never the
//!   `why`.
//!
//! | Argument | Bound (characters) | Over the bound | Blank | Not the right shape |
//! |---|---|---|---|---|
//! | `plan` | 65 536 | `Unavailable` | `Invalid` | `Invalid` |
//! | `question` | 8 192 | `Unavailable` | `Invalid` | `Invalid` |
//! | `recommendation` | 8 192 | `Unavailable` | `Invalid` | `Invalid` |
//! | `evidence[].what` | 2 048 | `Unavailable` | `Invalid` (`evidence`) | `Invalid` (`evidence`) |
//! | `evidence[].found_at` | 1 024 | `Unavailable` | `Invalid` (`evidence`) | `Invalid` (`evidence`) |
//!
//! **The declared scope is read strictly, and a quote is refused (D4).** Each
//! entry goes through `ori_memory::scope::ModulePath::parse`, which refuses an
//! empty path, an absolute one, a backslash, a glob, an empty or `.` or `..`
//! component, surrounding whitespace and control characters. The handler also
//! refuses a `"`, which the parse allows: `ori-store`'s projection reader cuts a
//! value at its first `"`, so a quoted path would be read two ways by the memory
//! scope and would refuse every memory call of that coder. Any such entry is
//! `Invalid { field: "declared_scope" }`, with no index and none of the entry's
//! text. The entries keep their order, and a duplicate goes through: the lock
//! table decides what it means. Whether the path names a directory of the
//! product is the desk's component's job, not the handler's.
//!
//! **The trigger is the catalogue's own constant.** It is matched against
//! [`crate::tools::ESCALATION_TRIGGERS`] and handed to the desk as that array's
//! `&'static str`, never the agent's string. Anything else is `Invalid`, which
//! S2's schema makes unreachable. [`crate::tools::ESCALATION_TRIGGERS`] and
//! `ori-orchestrator`'s `Trigger` are not pinned to each other here:
//! `ori-engine` is the one crate that sees both, and pins them (ORI-T-0070).
//!
//! **A refusal of the desk passes through unchanged.** The handler maps no
//! `PortError`: `?` hands it to S2, which records and answers it by its failure
//! table (`crate::server`'s module documentation). A lock conflict is
//! `E_SCOPE_LOCKED` under AICD §12, and the module it names stays inside the
//! error: S2 replies with the code and the section only, so another ticket's
//! claim is not disclosed. A refusal that `ori-core` has no refusal kind for
//! (`ticket_not_in_progress`, `escalation_refused`, `product_database_mismatch`) is
//! [`crate::ports::PortError::Denied`], with its own lower case code and section.
//!
//! **The reply is identifiers and counts, as one text block, and nothing else.**
//! Neither the plan, nor a path, nor the question, nor a piece of evidence is
//! echoed.
//!
//! | Tool | The reply, exactly |
//! |---|---|
//! | `aicd_plan_submit` | `plan <plan_id> stored for ticket <ticket_id>; <n> module(s) newly claimed` and a newline |
//! | `aicd_escalate` | `escalation <escalation_id> opened for ticket <ticket_id>; session suspended: <true or false>` and a newline |
//!
//! After a suspension, S2 refuses the connection's next call `session_not_active`
//! and closes the connection (S2's check 2): the agent that is told
//! `session suspended: true` is told its last answer.
//!
//! # What is recorded
//!
//! **A handler's own code appends nothing**, and holds no state across calls.
//! Every row besides S2's is the desk's, under the desk's contract: it makes every
//! refusal and every `Invalid` before its first append, so **a refused call stores
//! nothing**, and only an `Unavailable`, which is an append that failed, can leave
//! rows behind it. Each row of the desk carries the ticket in its ticket column.
//!
//! | Outcome | Rows, in order |
//! |---|---|
//! | a plan stored | `mcp.tool_called` (ticket column: the argument), then the desk's plan row, then one `lock.claimed` per new claim |
//! | an escalation opened | `mcp.tool_called`, then `escalation.raised`, then `ticket.escalated` when the desk's rule applies (a coder's escalation on its own bound ticket, while it is in progress), then a session row when the session is suspended |
//! | `E_SCOPE_LOCKED`, or any desk refusal (`Refused`, `Denied`) | `mcp.tool_called`, then `mcp.tool_failed` (`failure: refused`, the code, the section). Nothing is stored: both desks refuse before their first append |
//! | a ticket not in the product, or an argument the handler rejects | `mcp.tool_called`, then `mcp.tool_failed` (`failure: invalid`, the field) |
//! | a text over the schema's bound (a direct call only) | `mcp.tool_called`, then `mcp.tool_failed` (`failure: unavailable`) |
//! | a desk that could not answer | `mcp.tool_called`, then whatever rows the desk appended before it failed, then `mcp.tool_failed` (`failure: unavailable`) |
//!
//! **Bounds.** With the desks' contracts, `aicd_plan_submit` leaves at most 67
//! rows a call: S2's two (`mcp.tool_called`, and `mcp.tool_failed` on a failure),
//! plus the desk's plan row and up to 64 claims, which is the schema's own bound
//! on the declared scope (the handler does not count the entries again). So calls
//! to it leave **at most 67 times the window's limit plus one row per
//! connection**. `aicd_escalate` leaves at most five rows a call (S2's two and the
//! desk's three), so **five times the limit plus one row per connection**. Both
//! are safe upper bounds, counted the same way as S2's and S3's. Under the desks'
//! contract only an `Unavailable` follows an append, so the tight maxima are 66 and
//! 4. The rate window is per session and covers every tool, and other tools'
//! handlers append rows of their own.
//!
//! # Limits
//!
//! - **D5, the event's name.** `spec/API_SPEC.md` section 2 names
//!   `escalation.opened`, while `ori-store` projects `escalation.raised`. The desk
//!   writes `escalation.raised`, and PD-2 corrects API_SPEC: renaming in
//!   `ori-store` would change its tests.
//! - **`ori-store`'s projection reader is not a JSON parser.** With the payload
//!   order the desk's contract fixes (`escalation_id`, `trigger`, `question`,
//!   `recommendation`, then `evidence`), no value can be read as another column.
//!   But the reader undoes no JSON escape, so in the human queue a `"` in a
//!   question, a recommendation or an evidence text cuts the text, after a
//!   trailing `\`, and every `\` shows doubled. The lead records the `ori-store`
//!   follow-up ORI-T-0189 to make the reader parse JSON. The handlers do not
//!   refuse quotes in prose: only a path, where a quote changes what is claimed
//!   (D4).
//! - **The tool does not carry the template's "Options rejected" and "What is
//!   blocked" fields**, nor the Escalation row's `context_package_ref`
//!   (`spec/DATA_MODEL.md` section 2). PD-2 decides.
//! - **Suspension is not configurable in phase 1 (D6).** The port reports whether
//!   the session was suspended, and the desk answers `false` until a configuration
//!   key exists, and always `false` for a lead's session. S2's `SessionDesk`
//!   fixture always answers active, so no test here shows the effect of a
//!   suspension on the next call.
//! - **What the engine needs to implement the desks (ORI-T-0070).** In its
//!   manifest, `ori-memory` (`SanitizedField`, `ModulePath`), `ori-orchestrator`,
//!   and `serde` with `serde_json` (E-0008, on the same pins) for the escalation
//!   payload's struct. The contracts are written on the traits
//!   ([`crate::ports::PlanDesk`], [`crate::ports::EscalationDesk`]); this module's
//!   tests run against fake desks, so the real ones, including the payload order of
//!   `escalation.raised`, are tested there.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/tools/` (a directory
//! module, so that gate 12 tiers it), "What agents can do".

use core::fmt;

use ori_memory::barrier::{SanitizedField, sanitize_text};
use ori_memory::scope::ModulePath;
use ori_store::db::ProductDb;
use serde_json::Value;

use crate::ports::{EscalationDesk, EscalationEvidence, NewEscalation, PlanDesk, PortError};
use crate::protocol::{CallToolResult, JsonObject};
use crate::tools::{BoundCall, ESCALATION_TRIGGERS, HandlerError, ToolHandler};

/// The most characters a plan may hold: the catalogue's own bound on `plan`.
const PLAN_MAX_CHARS: usize = 65_536;
/// The most characters a question or a recommendation may hold: the catalogue's
/// own bound.
const TEXT_MAX_CHARS: usize = 8_192;
/// The most characters an evidence entry's `what` may hold: the catalogue's own
/// bound.
const WHAT_MAX_CHARS: usize = 2_048;
/// The most characters an evidence entry's `found_at` may hold: the catalogue's
/// own bound.
const FOUND_AT_MAX_CHARS: usize = 1_024;
/// The fewest entries `evidence` may hold: the catalogue's own bound.
const EVIDENCE_MIN: usize = 1;
/// The most entries `evidence` may hold: the catalogue's own bound.
const EVIDENCE_MAX: usize = 16;

/// The fixed sentence of a call that reaches a handler with no ticket.
const WHY_TICKET: &str = "the call carries no ticket";
/// The fixed sentence of a plan that is missing, is not a string or is blank.
const WHY_PLAN: &str = "the plan is missing, is not a string or is blank";
/// The fixed sentence of a declared scope that is not a list of module paths.
const WHY_SCOPE: &str = "the declared scope is missing, is not a list of strings or holds a \
                         path that is not a module path";
/// The fixed sentence of a trigger that is not one of the escalation triggers.
const WHY_TRIGGER: &str = "the trigger is missing or is not one of the escalation triggers";
/// The fixed sentence of a question that is missing, is not a string or is blank.
const WHY_QUESTION: &str = "the question is missing, is not a string or is blank";
/// The fixed sentence of a recommendation that is missing, is not a string or is
/// blank.
const WHY_RECOMMENDATION: &str = "the recommendation is missing, is not a string or is blank";
/// The fixed sentence of an evidence list that is not 1 to 16 findings of two
/// non-blank strings.
const WHY_EVIDENCE: &str = "the evidence is missing, is not a list of 1 to 16 findings with a \
                            what and a found_at, or holds a blank text";

/// What was unavailable when the plan is over the bound. Never copied into a
/// reply or an event.
const WHAT_PLAN: &str = "the plan's sanitization";
/// What was unavailable when the question is over the bound.
const WHAT_QUESTION: &str = "the question's sanitization";
/// What was unavailable when the recommendation is over the bound.
const WHAT_RECOMMENDATION: &str = "the recommendation's sanitization";
/// What was unavailable when an evidence text is over the bound, for both `what`
/// and `found_at`.
const WHAT_EVIDENCE: &str = "the evidence's sanitization";

/// An `Invalid` failure with a constant field and a constant sentence: S2 copies
/// the field into the reply and never the sentence (AICD §27).
fn invalid(field: &'static str, why: &'static str) -> HandlerError {
    HandlerError::Port(PortError::Invalid {
        field,
        why: why.to_owned(),
    })
}

/// A text over its bound, as an `Unavailable` failure with a fixed `what`: it
/// fails closed rather than store less than the agent sent (AICD §8).
fn over_bound(what: &'static str) -> HandlerError {
    HandlerError::Port(PortError::Unavailable {
        what: what.to_owned(),
    })
}

/// One agent text through the sanitization barrier, at `cap_chars`: AICD §8.
///
/// The text is not trimmed. A result the cap cut short is `over_bound(what)`, so
/// nothing less than what was sent is ever handed on.
fn sanitized(
    raw: &str,
    cap_chars: usize,
    what: &'static str,
) -> Result<SanitizedField, HandlerError> {
    let text = sanitize_text(raw, cap_chars);
    if text.truncated {
        return Err(over_bound(what));
    }
    Ok(text.field)
}

/// Whether a sanitized text is nothing but whitespace.
fn is_blank(text: &SanitizedField) -> bool {
    text.text().trim().is_empty()
}

/// The string argument `key`, sanitized at `cap_chars`, which must not be blank.
///
/// A missing key, a value that is not a string, and a blank text are
/// `Invalid { field: key }` with `why`; a text over the bound is
/// `Unavailable { what }`.
fn required_text(
    arguments: &mut JsonObject,
    key: &'static str,
    cap_chars: usize,
    why: &'static str,
    what: &'static str,
) -> Result<SanitizedField, HandlerError> {
    let Some(Value::String(raw)) = arguments.remove(key) else {
        return Err(invalid(key, why));
    };
    let text = sanitized(&raw, cap_chars, what)?;
    if is_blank(&text) {
        return Err(invalid(key, why));
    }
    Ok(text)
}

/// The declared scope: a list of strings, each a module path, none with a quote
/// (D4). Order and duplicates are kept.
fn declared_scope(value: Option<Value>) -> Result<Vec<ModulePath>, HandlerError> {
    let Some(Value::Array(entries)) = value else {
        return Err(invalid("declared_scope", WHY_SCOPE));
    };
    let mut paths = Vec::with_capacity(entries.len());
    for entry in &entries {
        let Value::String(text) = entry else {
            return Err(invalid("declared_scope", WHY_SCOPE));
        };
        // `ModulePath::parse` allows a quote, which `ori-store`'s projection
        // reader cannot read back: it cuts a value at its first one.
        if text.contains('"') {
            return Err(invalid("declared_scope", WHY_SCOPE));
        }
        match ModulePath::parse(text) {
            Ok(path) => paths.push(path),
            Err(_) => return Err(invalid("declared_scope", WHY_SCOPE)),
        }
    }
    Ok(paths)
}

/// The trigger, as the catalogue's own `&'static str`, never the agent's string.
fn trigger(value: Option<Value>) -> Result<&'static str, HandlerError> {
    let Some(Value::String(text)) = value else {
        return Err(invalid("trigger", WHY_TRIGGER));
    };
    ESCALATION_TRIGGERS
        .iter()
        .copied()
        .find(|known| *known == text)
        .ok_or_else(|| invalid("trigger", WHY_TRIGGER))
}

/// The evidence: 1 to 16 objects, each with a `what` and a `found_at` that are
/// strings, sanitized and not blank. Order is kept.
fn evidence(value: Option<Value>) -> Result<Vec<EscalationEvidence>, HandlerError> {
    let Some(Value::Array(entries)) = value else {
        return Err(invalid("evidence", WHY_EVIDENCE));
    };
    if !(EVIDENCE_MIN..=EVIDENCE_MAX).contains(&entries.len()) {
        return Err(invalid("evidence", WHY_EVIDENCE));
    }
    let mut findings = Vec::with_capacity(entries.len());
    for entry in &entries {
        let Value::Object(object) = entry else {
            return Err(invalid("evidence", WHY_EVIDENCE));
        };
        let (Some(Value::String(what)), Some(Value::String(found_at))) =
            (object.get("what"), object.get("found_at"))
        else {
            return Err(invalid("evidence", WHY_EVIDENCE));
        };
        let what = sanitized(what, WHAT_MAX_CHARS, WHAT_EVIDENCE)?;
        let found_at = sanitized(found_at, FOUND_AT_MAX_CHARS, WHAT_EVIDENCE)?;
        if is_blank(&what) || is_blank(&found_at) {
            return Err(invalid("evidence", WHY_EVIDENCE));
        }
        findings.push(EscalationEvidence { what, found_at });
    }
    Ok(findings)
}

/// The handler of `aicd_plan_submit(ticket_id, plan, declared_scope)`: stores a
/// ticket's plan and runs the scope lock check, through the engine's
/// [`crate::ports::PlanDesk`] (AICD §12).
///
/// The ticket is the call's `ticket_id` argument, which S2's check 7 makes the
/// bound ticket for a coder. The plan is sanitized at the schema's bound and the
/// declared scope is read as module paths, both as the module documentation says;
/// the desk then checks the lock table before it stores anything. The reply is
/// `plan <id> stored for ticket <ticket>; <n> module(s) newly claimed`, and echoes
/// neither the plan nor a path. Appends no event of its own and holds no state
/// across calls. See the module documentation for the rows a call leaves and the
/// limits.
///
/// # Examples
///
/// The engine registers it with its own desk.
///
/// ```no_run
/// use ori_mcp::ports::PlanDesk;
/// use ori_mcp::scopes::Tool;
/// use ori_mcp::tools::Handlers;
/// use ori_mcp::tools::orchestration::PlanSubmitHandler;
///
/// fn register(handlers: &mut Handlers, desk: Box<dyn PlanDesk>) {
///     handlers
///         .register(Tool::PlanSubmit, Box::new(PlanSubmitHandler::new(desk)))
///         .expect("one handler per tool");
/// }
/// ```
pub struct PlanSubmitHandler {
    desk: Box<dyn PlanDesk>,
}

impl PlanSubmitHandler {
    /// A handler over the engine's plan desk: AICD §12.
    #[must_use]
    pub fn new(desk: Box<dyn PlanDesk>) -> Self {
        Self { desk }
    }
}

impl fmt::Debug for PlanSubmitHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlanSubmitHandler").finish_non_exhaustive()
    }
}

impl ToolHandler for PlanSubmitHandler {
    /// `aicd_plan_submit`: sanitize the plan, read the declared scope, hand both
    /// to the desk, and answer with the plan's id and the count of modules newly
    /// claimed: AICD §12.
    ///
    /// # Errors
    ///
    /// The desk's [`crate::ports::PortError`] as it is: `Refused` for
    /// `E_SCOPE_LOCKED` (AICD §12) or another refusal kind of `ori-core`, `Denied`
    /// for a rule it has no kind for, `Invalid` for a ticket the product does not
    /// hold, `Unavailable` when it could not answer. The handler's own:
    /// `Port(Invalid)` for a call with no ticket, a plan that is missing, not a
    /// string or blank, or a declared scope that is not a list of module paths
    /// without a quote (all unreachable through S2's schema and checks except the
    /// blank plan and the paths the schema cannot judge), and `Port(Unavailable)`
    /// for a plan over the schema's bound, which only a direct call can reach.
    fn call(
        &mut self,
        db: &mut ProductDb,
        call: &BoundCall<'_>,
        mut arguments: JsonObject,
    ) -> Result<CallToolResult, HandlerError> {
        let Some(ticket) = call.ticket_id else {
            return Err(invalid("ticket_id", WHY_TICKET));
        };
        let plan = required_text(&mut arguments, "plan", PLAN_MAX_CHARS, WHY_PLAN, WHAT_PLAN)?;
        let paths = declared_scope(arguments.remove("declared_scope"))?;
        let stored = self.desk.submit(db, call.binding, ticket, &plan, &paths)?;
        Ok(CallToolResult::text(format!(
            "plan {} stored for ticket {}; {} module(s) newly claimed\n",
            stored.plan_id, ticket, stored.modules_claimed
        )))
    }
}

/// The handler of `aicd_escalate(ticket_id, trigger, question, recommendation,
/// evidence)`: opens an escalation with its evidence, and suspends the session if
/// configured, through the engine's [`crate::ports::EscalationDesk`] (AICD §12).
///
/// The ticket is the call's `ticket_id` argument: for a coder S2's check 7 makes
/// it the bound ticket, and for a lead the desk checks it. The trigger is the
/// catalogue's own constant, the question, the recommendation and every evidence
/// text are sanitized at the schema's bounds, and the desk raises the escalation
/// only if a finding stands behind the trigger. The reply is `escalation <id>
/// opened for ticket <ticket>; session suspended: <true or false>`. **After a
/// suspension S2 refuses the connection's next call `session_not_active` and
/// closes it.** Appends no event of its own and holds no state across calls. See
/// the module documentation for the rows a call leaves and the limits.
///
/// # Examples
///
/// The engine registers it with its own desk.
///
/// ```no_run
/// use ori_mcp::ports::EscalationDesk;
/// use ori_mcp::scopes::Tool;
/// use ori_mcp::tools::Handlers;
/// use ori_mcp::tools::orchestration::EscalateHandler;
///
/// fn register(handlers: &mut Handlers, desk: Box<dyn EscalationDesk>) {
///     handlers
///         .register(Tool::Escalate, Box::new(EscalateHandler::new(desk)))
///         .expect("one handler per tool");
/// }
/// ```
pub struct EscalateHandler {
    desk: Box<dyn EscalationDesk>,
}

impl EscalateHandler {
    /// A handler over the engine's escalation desk: AICD §12.
    #[must_use]
    pub fn new(desk: Box<dyn EscalationDesk>) -> Self {
        Self { desk }
    }
}

impl fmt::Debug for EscalateHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EscalateHandler").finish_non_exhaustive()
    }
}

impl ToolHandler for EscalateHandler {
    /// `aicd_escalate`: match the trigger, sanitize the question, the
    /// recommendation and the evidence, hand them to the desk, and answer with the
    /// escalation's id and whether the session was suspended: AICD §12.
    ///
    /// # Errors
    ///
    /// The desk's [`crate::ports::PortError`] as it is: `Refused` for a refusal
    /// kind of `ori-core`, `Denied` for a rule it has no kind for
    /// (`escalation_refused`, AICD §12), `Invalid` for a ticket the product does
    /// not hold, `Unavailable` when it could not answer. The handler's own:
    /// `Port(Invalid)` for a call with no ticket, a trigger that is not one of the
    /// eight, a question or a recommendation that is missing, not a string or
    /// blank, or evidence that is not 1 to 16 findings of two non-blank strings
    /// (all unreachable through S2's schema except the blanks), and
    /// `Port(Unavailable)` for a text over the schema's bound, which only a direct
    /// call can reach.
    fn call(
        &mut self,
        db: &mut ProductDb,
        call: &BoundCall<'_>,
        mut arguments: JsonObject,
    ) -> Result<CallToolResult, HandlerError> {
        let Some(ticket) = call.ticket_id else {
            return Err(invalid("ticket_id", WHY_TICKET));
        };
        let trigger = trigger(arguments.remove("trigger"))?;
        let question = required_text(
            &mut arguments,
            "question",
            TEXT_MAX_CHARS,
            WHY_QUESTION,
            WHAT_QUESTION,
        )?;
        let recommendation = required_text(
            &mut arguments,
            "recommendation",
            TEXT_MAX_CHARS,
            WHY_RECOMMENDATION,
            WHAT_RECOMMENDATION,
        )?;
        let evidence = evidence(arguments.remove("evidence"))?;
        let opened = self.desk.open(
            db,
            call.binding,
            ticket,
            &NewEscalation {
                trigger,
                question,
                recommendation,
                evidence,
            },
        )?;
        Ok(CallToolResult::text(format!(
            "escalation {} opened for ticket {}; session suspended: {}\n",
            opened.escalation_id, ticket, opened.suspended
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ori_core::error::{Error, MethodologyRef, RefusalKind};
    use ori_core::types::{Id, Role, Timestamp};
    use serde_json::json;

    use super::*;
    use crate::ports::{EscalationOpened, PlanStored};
    use crate::scopes::Tool;
    use crate::server::{Binding, Reply};
    use crate::tools::Handlers;
    use crate::tools::memory::test_support::{
        START, Setup, World, append, id, identity, kinds, payload, session, text_of, ticket_t, wire,
    };

    /// A text only the tests carry: the planted marker.
    const MARKER: &str = "ZEBRAQUARTZ0188";

    /// What a fake plan desk saw of one call.
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct SeenPlan {
        session: Id,
        identity: Id,
        role: Role,
        ticket: Id,
        plan: String,
        paths: Vec<String>,
    }

    /// What a fake escalation desk saw of one call.
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct SeenEscalation {
        session: Id,
        identity: Id,
        role: Role,
        ticket: Id,
        trigger: &'static str,
        question: String,
        recommendation: String,
        evidence: Vec<(String, String)>,
    }

    type PlanScript =
        Box<dyn FnMut(&mut ProductDb, &Binding, &Id) -> Result<PlanStored, PortError> + Send>;
    type EscalationScript =
        Box<dyn FnMut(&mut ProductDb, &Binding, &Id) -> Result<EscalationOpened, PortError> + Send>;

    /// A plan desk that records every call and answers from its script, which
    /// can append rows through the handle it is given.
    struct FakePlanDesk {
        seen: Arc<Mutex<Vec<SeenPlan>>>,
        script: PlanScript,
    }

    impl PlanDesk for FakePlanDesk {
        fn submit(
            &mut self,
            db: &mut ProductDb,
            binding: &Binding,
            ticket: &Id,
            plan: &SanitizedField,
            declared_scope: &[ModulePath],
        ) -> Result<PlanStored, PortError> {
            self.seen.lock().expect("the record").push(SeenPlan {
                session: binding.session_id().clone(),
                identity: binding.identity().clone(),
                role: binding.role(),
                ticket: ticket.clone(),
                plan: plan.text().to_owned(),
                paths: declared_scope
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect(),
            });
            (self.script)(db, binding, ticket)
        }
    }

    /// An escalation desk that records every call and answers from its script.
    struct FakeEscalationDesk {
        seen: Arc<Mutex<Vec<SeenEscalation>>>,
        script: EscalationScript,
    }

    impl EscalationDesk for FakeEscalationDesk {
        fn open(
            &mut self,
            db: &mut ProductDb,
            binding: &Binding,
            ticket: &Id,
            escalation: &NewEscalation,
        ) -> Result<EscalationOpened, PortError> {
            self.seen.lock().expect("the record").push(SeenEscalation {
                session: binding.session_id().clone(),
                identity: binding.identity().clone(),
                role: binding.role(),
                ticket: ticket.clone(),
                trigger: escalation.trigger,
                question: escalation.question.text().to_owned(),
                recommendation: escalation.recommendation.text().to_owned(),
                evidence: escalation
                    .evidence
                    .iter()
                    .map(|finding| {
                        (
                            finding.what.text().to_owned(),
                            finding.found_at.text().to_owned(),
                        )
                    })
                    .collect(),
            });
            (self.script)(db, binding, ticket)
        }
    }

    /// The fakes' records, shared with the test that built them.
    struct Seen {
        plans: Arc<Mutex<Vec<SeenPlan>>>,
        escalations: Arc<Mutex<Vec<SeenEscalation>>>,
    }

    impl Seen {
        fn plans(&self) -> Vec<SeenPlan> {
            self.plans.lock().expect("the record").clone()
        }

        fn escalations(&self) -> Vec<SeenEscalation> {
            self.escalations.lock().expect("the record").clone()
        }

        /// Whether neither desk was reached.
        fn untouched(&self) -> bool {
            self.plans().is_empty() && self.escalations().is_empty()
        }
    }

    /// A plan answer that appends nothing and claims `claimed` modules.
    fn plan_answer(claimed: usize) -> PlanScript {
        Box::new(move |_, _, _| {
            Ok(PlanStored {
                plan_id: id("PLANONE"),
                modules_claimed: claimed,
            })
        })
    }

    /// An escalation answer that appends nothing.
    fn escalation_answer(suspended: bool) -> EscalationScript {
        Box::new(move |_, _, _| {
            Ok(EscalationOpened {
                escalation_id: id("ESCALATIONONE"),
                suspended,
            })
        })
    }

    /// A world of `role` and `ticket`, both handlers registered over fakes that
    /// answer from the given scripts.
    fn world_with(
        label: &str,
        role: Role,
        ticket: Option<Id>,
        plan: PlanScript,
        escalation: EscalationScript,
    ) -> (World, Seen) {
        let seen = Seen {
            plans: Arc::new(Mutex::new(Vec::new())),
            escalations: Arc::new(Mutex::new(Vec::new())),
        };
        let plan_desk = FakePlanDesk {
            seen: Arc::clone(&seen.plans),
            script: plan,
        };
        let escalation_desk = FakeEscalationDesk {
            seen: Arc::clone(&seen.escalations),
            script: escalation,
        };
        let world = Setup::new(role, ticket).build_with(label, move |_| {
            let mut handlers = Handlers::new();
            handlers
                .register(
                    Tool::PlanSubmit,
                    Box::new(PlanSubmitHandler::new(Box::new(plan_desk))),
                )
                .expect("one handler per tool");
            handlers
                .register(
                    Tool::Escalate,
                    Box::new(EscalateHandler::new(Box::new(escalation_desk))),
                )
                .expect("one handler per tool");
            handlers
        });
        (world, seen)
    }

    /// A world whose desks answer success and append nothing.
    fn world_ok(label: &str, role: Role, ticket: Option<Id>) -> (World, Seen) {
        world_with(
            label,
            role,
            ticket,
            plan_answer(0),
            escalation_answer(false),
        )
    }

    /// The arguments of a valid `aicd_plan_submit` on the ticket T.
    fn plan_arguments(plan: &str, scope: &Value) -> Value {
        json!({
            "ticket_id": ticket_t().as_str(),
            "plan": plan,
            "declared_scope": scope,
        })
    }

    /// The arguments of a valid `aicd_escalate` on the ticket T.
    fn escalate_arguments(
        trigger: &str,
        question: &str,
        recommendation: &str,
        evidence: &Value,
    ) -> Value {
        json!({
            "ticket_id": ticket_t().as_str(),
            "trigger": trigger,
            "question": question,
            "recommendation": recommendation,
            "evidence": evidence,
        })
    }

    /// One finding, as the arguments spell it.
    fn finding(what: &str, found_at: &str) -> Value {
        json!({"what": what, "found_at": found_at})
    }

    /// The `structuredContent` of an error result that was refused under `reason`
    /// and `section`.
    fn refused_object(reason: &str, section: u64) -> Value {
        json!({
            "failure": "refused",
            "reason": reason,
            "methodology_ref": {"section": section, "subsection": null},
        })
    }

    /// The `structuredContent` of an error result that was invalid in `field`.
    fn invalid_object(field: &str) -> Value {
        json!({"failure": "invalid", "field": field})
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

    /// Asserts that `reply` is the call's success: no error, one text block, and
    /// the connection still open.
    fn assert_success(reply: &Reply) -> String {
        assert!(!reply.close && reply.unrecorded.is_none());
        let message = wire(reply);
        assert!(message["result"].get("isError").is_none(), "{message}");
        assert!(
            message["result"].get("structuredContent").is_none(),
            "{message}"
        );
        text_of(reply)
    }

    /// Asserts that no row holds `needle` anywhere in its payload.
    fn assert_no_row_holds(rows: &[ori_store::event_log::Event], needle: &str) {
        for row in rows {
            assert!(
                !row.payload().contains(needle),
                "{} holds the marker: {}",
                row.kind(),
                row.payload()
            );
        }
    }

    /// The call's own binding, for a direct call to a handler.
    fn binding(role: Role, ticket: Option<Id>, product: &Id) -> Binding {
        Binding::new(session(), identity(), role, product.clone(), ticket)
    }

    /// A direct call to `handler` with `arguments`, past S2's schema.
    fn direct(
        handler: &mut dyn ToolHandler,
        world: &mut World,
        tool: Tool,
        ticket: Option<&Id>,
        arguments: &Value,
    ) -> Result<CallToolResult, HandlerError> {
        let binding = binding(Role::Coder, Some(ticket_t()), &world.product);
        let call = BoundCall {
            binding: &binding,
            tool,
            called_seq: 1,
            at: Timestamp::from_millis(START),
            ticket_id: ticket,
        };
        let Value::Object(arguments) = arguments.clone() else {
            panic!("arguments are an object");
        };
        handler.call(&mut world.db, &call, arguments)
    }

    /// A handler pair over desks that record and answer success.
    fn handlers() -> (PlanSubmitHandler, EscalateHandler, Seen) {
        let seen = Seen {
            plans: Arc::new(Mutex::new(Vec::new())),
            escalations: Arc::new(Mutex::new(Vec::new())),
        };
        let plan = PlanSubmitHandler::new(Box::new(FakePlanDesk {
            seen: Arc::clone(&seen.plans),
            script: plan_answer(0),
        }));
        let escalate = EscalateHandler::new(Box::new(FakeEscalationDesk {
            seen: Arc::clone(&seen.escalations),
            script: escalation_answer(false),
        }));
        (plan, escalate, seen)
    }

    // -------------------------------------------------------------------
    // 1. A coder's plan
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_a_coder_submits_a_plan_through_its_desk() {
        let plan_id = id("PLANONE");
        let script: PlanScript = Box::new(move |db, binding, ticket| {
            let product = binding.product_id();
            append(db, product, "plan.submitted", Some(ticket), "{}".to_owned());
            append(
                db,
                product,
                "lock.claimed",
                Some(ticket),
                r#"{"module":"crates/z"}"#.to_owned(),
            );
            append(
                db,
                product,
                "lock.claimed",
                Some(ticket),
                r#"{"module":"crates/a"}"#.to_owned(),
            );
            Ok(PlanStored {
                plan_id: plan_id.clone(),
                modules_claimed: 2,
            })
        });
        let (mut world, seen) = world_with(
            "plan-ok",
            Role::Coder,
            Some(ticket_t()),
            script,
            escalation_answer(false),
        );
        let tip = world.tip();

        let reply = world.call(
            "aicd_plan_submit",
            plan_arguments(
                "  step one\u{0}step two\u{7}  ",
                &json!(["crates/z", "crates/a", "crates/z"]),
            ),
        );
        let text = assert_success(&reply);
        assert_eq!(
            text,
            format!(
                "plan {} stored for ticket {}; 2 module(s) newly claimed\n",
                id("PLANONE"),
                ticket_t()
            )
        );

        // The desk received the binding's session and identity, the bound
        // ticket, the plan untrimmed with each control character replaced, and
        // all three paths in the agent's order, the duplicate included.
        assert_eq!(
            seen.plans(),
            [SeenPlan {
                session: session(),
                identity: identity(),
                role: Role::Coder,
                ticket: ticket_t(),
                plan: "  step one\u{fffd}step two\u{fffd}  ".to_owned(),
                paths: vec![
                    "crates/z".to_owned(),
                    "crates/a".to_owned(),
                    "crates/z".to_owned()
                ],
            }]
        );
        assert!(seen.escalations().is_empty());

        // The log gained S2's row, then exactly the desk's: the handler
        // appended nothing.
        let rows = world.rows_since(tip);
        assert_eq!(
            kinds(&rows),
            [
                "mcp.tool_called",
                "plan.submitted",
                "lock.claimed",
                "lock.claimed"
            ]
        );
        assert_eq!(rows[0].ticket_id(), Some(&ticket_t()), "the argument");
    }

    // -------------------------------------------------------------------
    // 2. A locked scope
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_a_locked_scope_is_e_scope_locked_and_names_no_module() {
        let script: PlanScript = Box::new(|_, _, _| {
            Err(PortError::Refused(Error::refused(
                RefusalKind::ScopeLocked {
                    module: MARKER.to_owned(),
                },
            )))
        });
        let (mut world, seen) = world_with(
            "plan-locked",
            Role::Coder,
            Some(ticket_t()),
            script,
            escalation_answer(false),
        );
        let tip = world.tip();

        let reply = world.call(
            "aicd_plan_submit",
            plan_arguments("a plan", &json!(["crates/a"])),
        );
        assert!(!reply.close && reply.unrecorded.is_none());
        let expected = refused_object("E_SCOPE_LOCKED", 12);
        assert_failure(&reply, &expected);
        assert_eq!(seen.plans().len(), 1, "the desk was asked, and refused");

        // The module the lock table named is in neither the reply nor any row.
        assert!(
            !wire(&reply).to_string().contains(MARKER),
            "the reply names the module"
        );
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called", "mcp.tool_failed"]);
        assert_no_row_holds(&rows, MARKER);
        let failed = payload(&rows[1]);
        assert_eq!(failed["failure"], "refused");
        assert_eq!(failed["reason"], "E_SCOPE_LOCKED");
        assert_eq!(failed["methodology_ref"]["section"], 12);
    }

    // -------------------------------------------------------------------
    // 3. A refusal with no ori-core kind
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_a_denied_refusal_keeps_its_code_and_section() {
        let script: PlanScript = Box::new(|_, _, _| {
            Err(PortError::Denied {
                code: "ticket_not_in_progress",
                methodology_ref: MethodologyRef::at(11).expect("a section the methodology has"),
            })
        });
        let (mut world, _seen) = world_with(
            "plan-denied",
            Role::Coder,
            Some(ticket_t()),
            script,
            escalation_answer(false),
        );
        let tip = world.tip();

        let reply = world.call(
            "aicd_plan_submit",
            plan_arguments("a plan", &json!(["crates/a"])),
        );
        assert!(!reply.close && reply.unrecorded.is_none());
        assert_failure(&reply, &refused_object("ticket_not_in_progress", 11));

        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called", "mcp.tool_failed"]);
        let failed = payload(&rows[1]);
        assert_eq!(failed["failure"], "refused");
        assert_eq!(failed["reason"], "ticket_not_in_progress");
        assert_eq!(
            failed["methodology_ref"],
            json!({"section": 11, "subsection": null})
        );
    }

    // -------------------------------------------------------------------
    // 4. The declared scope
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_a_declared_scope_is_held_to_module_paths() {
        let quoted = format!("crates/{MARKER}\"x");
        let cases = [
            format!("/{MARKER}/x"),
            format!("crates/../{MARKER}"),
            format!("crates//{MARKER}"),
            quoted,
        ];
        for (index, path) in cases.iter().enumerate() {
            let (mut world, seen) =
                world_ok(&format!("scope-{index}"), Role::Coder, Some(ticket_t()));
            let tip = world.tip();

            let reply = world.call(
                "aicd_plan_submit",
                plan_arguments("a plan", &json!(["crates/ok", path])),
            );
            assert!(!reply.close && reply.unrecorded.is_none());
            assert_failure(&reply, &invalid_object("declared_scope"));
            assert!(
                !wire(&reply).to_string().contains(MARKER),
                "the reply echoes the path: {path}"
            );
            assert!(seen.untouched(), "the desk was reached for {path}");

            let rows = world.rows_since(tip);
            assert_eq!(
                kinds(&rows),
                ["mcp.tool_called", "mcp.tool_failed"],
                "{path}"
            );
            assert_no_row_holds(&rows, MARKER);
        }
    }

    // -------------------------------------------------------------------
    // 5. Blank texts
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_blank_text_is_invalid_with_its_field() {
        let blank = "   ";
        let one_finding = json!([finding("what", "where")]);
        // The coder submits plans and the lead escalates: each case runs as the
        // role that holds its tool.
        let cases: [(Role, Option<Id>, &str, &str, Value); 4] = [
            (
                Role::Coder,
                Some(ticket_t()),
                "aicd_plan_submit",
                "plan",
                plan_arguments(blank, &json!(["crates/a"])),
            ),
            (
                Role::Lead,
                None,
                "aicd_escalate",
                "question",
                escalate_arguments("security", blank, "do this", &one_finding),
            ),
            (
                Role::Lead,
                None,
                "aicd_escalate",
                "recommendation",
                escalate_arguments("security", "what now", blank, &one_finding),
            ),
            (
                Role::Lead,
                None,
                "aicd_escalate",
                "evidence",
                escalate_arguments(
                    "security",
                    "what now",
                    "do this",
                    &json!([finding("what", "where"), finding("what", blank)]),
                ),
            ),
        ];
        for (index, (role, bound, tool, field, arguments)) in cases.iter().enumerate() {
            let (mut world, seen) = world_ok(&format!("blank-{index}"), *role, bound.clone());
            let tip = world.tip();

            let reply = world.call(tool, arguments.clone());
            assert!(!reply.close && reply.unrecorded.is_none());
            assert_failure(&reply, &invalid_object(field));
            assert_eq!(
                kinds(&world.rows_since(tip)),
                ["mcp.tool_called", "mcp.tool_failed"],
                "{field}"
            );
            assert!(seen.untouched(), "the desk was reached for a blank {field}");
        }
    }

    // -------------------------------------------------------------------
    // 6. A lead's escalation
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_the_lead_opens_an_escalation_through_its_desk() {
        let script: EscalationScript = Box::new(|db, binding, ticket| {
            append(
                db,
                binding.product_id(),
                "escalation.raised",
                Some(ticket),
                "{}".to_owned(),
            );
            Ok(EscalationOpened {
                escalation_id: id("ESCALATIONONE"),
                suspended: false,
            })
        });
        let (mut world, seen) =
            world_with("escalate-lead", Role::Lead, None, plan_answer(0), script);
        let tip = world.tip();

        let reply = world.call(
            "aicd_escalate",
            escalate_arguments(
                "scope_conflict",
                " why\u{0}now ",
                " do\u{0}this ",
                &json!([
                    finding(" first\u{0}what ", " first\u{0}where "),
                    finding(" second\u{0}what ", " second\u{0}where ")
                ]),
            ),
        );
        let text = assert_success(&reply);
        assert_eq!(
            text,
            format!(
                "escalation {} opened for ticket {}; session suspended: false\n",
                id("ESCALATIONONE"),
                ticket_t()
            )
        );

        // By value: two uses of a `const` share no address, so a pointer
        // comparison would fail a correct handler.
        let received = seen.escalations();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].trigger, "scope_conflict");
        assert!(ESCALATION_TRIGGERS.contains(&received[0].trigger));
        assert_eq!(
            received,
            [SeenEscalation {
                session: session(),
                identity: identity(),
                role: Role::Lead,
                ticket: ticket_t(),
                trigger: "scope_conflict",
                question: " why\u{fffd}now ".to_owned(),
                recommendation: " do\u{fffd}this ".to_owned(),
                evidence: vec![
                    (
                        " first\u{fffd}what ".to_owned(),
                        " first\u{fffd}where ".to_owned()
                    ),
                    (
                        " second\u{fffd}what ".to_owned(),
                        " second\u{fffd}where ".to_owned()
                    ),
                ],
            }]
        );
        assert!(seen.plans().is_empty());

        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called", "escalation.raised"]);
        assert_eq!(rows[0].ticket_id(), Some(&ticket_t()), "the argument");
    }

    // -------------------------------------------------------------------
    // 7. A coder's escalation
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_a_coders_escalation_reaches_its_desk_and_reports_a_suspension() {
        let (mut world, seen) = world_with(
            "escalate-coder",
            Role::Coder,
            Some(ticket_t()),
            plan_answer(0),
            escalation_answer(true),
        );
        let tip = world.tip();

        let reply = world.call(
            "aicd_escalate",
            escalate_arguments(
                "test_modified",
                "may the test change",
                "yes, the spec moved",
                &json!([finding(
                    "a test asserts the old rule",
                    "crates/x/tests/t.rs"
                )]),
            ),
        );
        let text = assert_success(&reply);
        assert_eq!(
            text,
            format!(
                "escalation {} opened for ticket {}; session suspended: true\n",
                id("ESCALATIONONE"),
                ticket_t()
            )
        );
        assert!(text.ends_with("session suspended: true\n"));

        let received = seen.escalations();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].ticket, ticket_t(), "the bound ticket");
        assert_eq!(received[0].session, session());
        assert_eq!(received[0].identity, identity());
        assert_eq!(received[0].role, Role::Coder);
        assert_eq!(received[0].trigger, "test_modified");
        assert_eq!(kinds(&world.rows_since(tip)), ["mcp.tool_called"]);
    }

    // -------------------------------------------------------------------
    // 8. Roles
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_each_tool_reaches_its_desk_for_its_roles_only() {
        let refused_plan = [
            Role::Lead,
            Role::Qa,
            Role::Documentation,
            Role::Operations,
            Role::ProductSignal,
            Role::Assistant,
        ];
        let refused_escalate = [
            Role::Qa,
            Role::Documentation,
            Role::Operations,
            Role::ProductSignal,
            Role::Assistant,
        ];
        let plan = plan_arguments("a plan", &json!(["crates/a"]));
        let escalate = escalate_arguments(
            "security",
            "a question",
            "a recommendation",
            &json!([finding("what", "where")]),
        );
        let cases = [
            ("aicd_plan_submit", &refused_plan[..], &plan),
            ("aicd_escalate", &refused_escalate[..], &escalate),
        ];
        for (tool, roles, arguments) in cases {
            for role in roles {
                let label = format!("roles-{tool}-{}", role.as_str());
                let (mut world, seen) = world_ok(&label, *role, None);
                let tip = world.tip();

                let reply = world.call(tool, arguments.clone());
                let message = wire(&reply);
                assert_eq!(
                    message["error"]["code"], -32602,
                    "{tool} {role:?}: {message}"
                );
                assert_eq!(
                    message["error"]["data"]["reason"], "tool_not_permitted_for_role",
                    "{tool} {role:?}: {message}"
                );
                assert_eq!(
                    kinds(&world.rows_since(tip)),
                    ["mcp.tool_refused"],
                    "{tool} {role:?}"
                );
                assert!(seen.untouched(), "{tool} reached a desk for {role:?}");
            }
        }
    }

    // -------------------------------------------------------------------
    // 9. No agent text in the reply or in the call's rows
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_no_agent_text_reaches_the_reply_or_the_calls_rows() {
        // A plan: the marker is in the plan and in a path.
        let (mut world, seen) = world_ok("marker-plan", Role::Coder, Some(ticket_t()));
        let tip = world.tip();
        let reply = world.call(
            "aicd_plan_submit",
            plan_arguments(
                &format!("the plan {MARKER}"),
                &json!([format!("crates/{MARKER}")]),
            ),
        );
        let text = assert_success(&reply);
        assert!(!text.contains(MARKER), "the reply echoes the plan: {text}");
        assert!(!wire(&reply).to_string().contains(MARKER));
        // The desk did receive the marker, so the check above is not vacuous.
        let received = seen.plans();
        assert!(received[0].plan.contains(MARKER));
        assert!(received[0].paths[0].contains(MARKER));
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called"]);
        assert_no_row_holds(&rows, MARKER);

        // An escalation: the marker is in the question, the recommendation and
        // both texts of the evidence.
        let (mut world, seen) = world_ok("marker-escalation", Role::Lead, None);
        let tip = world.tip();
        let reply = world.call(
            "aicd_escalate",
            escalate_arguments(
                "adr_area",
                &format!("the question {MARKER}"),
                &format!("the recommendation {MARKER}"),
                &json!([finding(&format!("found {MARKER}"), &format!("at {MARKER}"))]),
            ),
        );
        let text = assert_success(&reply);
        assert!(!text.contains(MARKER), "the reply echoes a text: {text}");
        assert!(!wire(&reply).to_string().contains(MARKER));
        let received = seen.escalations();
        assert!(received[0].question.contains(MARKER));
        assert!(received[0].recommendation.contains(MARKER));
        assert!(received[0].evidence[0].0.contains(MARKER));
        assert!(received[0].evidence[0].1.contains(MARKER));
        let rows = world.rows_since(tip);
        assert_eq!(kinds(&rows), ["mcp.tool_called"]);
        assert_no_row_holds(&rows, MARKER);
    }

    // -------------------------------------------------------------------
    // 10. The fallbacks never panic
    // -------------------------------------------------------------------

    /// Asserts a handler's `Port(Invalid)` with its constant field and sentence.
    fn assert_invalid(result: &Result<CallToolResult, HandlerError>, field: &str, why: &str) {
        match result {
            Err(HandlerError::Port(PortError::Invalid {
                field: got_field,
                why: got_why,
            })) => {
                assert_eq!(*got_field, field);
                assert_eq!(got_why, why, "a fixed sentence");
            }
            other => panic!("expected an invalid {field}, got {other:?}"),
        }
    }

    /// Asserts a handler's `Port(Unavailable)` with its fixed `what`.
    fn assert_unavailable(result: &Result<CallToolResult, HandlerError>, what: &str) {
        match result {
            Err(HandlerError::Port(PortError::Unavailable { what: got })) => {
                assert_eq!(got, what, "a fixed text");
            }
            other => panic!("expected an unavailable {what}, got {other:?}"),
        }
    }

    #[test]
    fn ori_t_0188_the_handlers_fallbacks_never_panic() {
        let (mut plan, mut escalate, seen) = handlers();
        let (mut world, _unused) = world_ok("fallbacks", Role::Coder, Some(ticket_t()));
        let ticket = ticket_t();
        assert_eq!(format!("{plan:?}"), "PlanSubmitHandler { .. }");
        assert_eq!(format!("{escalate:?}"), "EscalateHandler { .. }");

        // The plan handler.
        let scope = json!(["crates/a"]);
        let valid = plan_arguments("a plan", &scope);
        let result = direct(&mut plan, &mut world, Tool::PlanSubmit, None, &valid);
        assert_invalid(&result, "ticket_id", WHY_TICKET);
        for bad in [
            json!({"declared_scope": ["crates/a"]}),
            json!({"plan": 7, "declared_scope": ["crates/a"]}),
            json!({"plan": null, "declared_scope": ["crates/a"]}),
            json!({"plan": {MARKER: 1}, "declared_scope": ["crates/a"]}),
        ] {
            let result = direct(&mut plan, &mut world, Tool::PlanSubmit, Some(&ticket), &bad);
            assert_invalid(&result, "plan", WHY_PLAN);
        }
        for bad in [
            json!({"plan": "p"}),
            json!({"plan": "p", "declared_scope": "crates/a"}),
            json!({"plan": "p", "declared_scope": null}),
            json!({"plan": "p", "declared_scope": {"a": 1}}),
            json!({"plan": "p", "declared_scope": [7]}),
            json!({"plan": "p", "declared_scope": ["crates/a", null]}),
        ] {
            let result = direct(&mut plan, &mut world, Tool::PlanSubmit, Some(&ticket), &bad);
            assert_invalid(&result, "declared_scope", WHY_SCOPE);
        }

        // The escalation handler.
        let finding_list = json!([finding("what", "where")]);
        let valid = escalate_arguments("security", "q", "r", &finding_list);
        let result = direct(&mut escalate, &mut world, Tool::Escalate, None, &valid);
        assert_invalid(&result, "ticket_id", WHY_TICKET);
        for trigger in [
            json!("nope"),
            json!("ADR_AREA"),
            json!(" adr_area"),
            json!(7),
            json!(null),
            json!(MARKER),
        ] {
            let mut bad = valid.clone();
            bad["trigger"] = trigger;
            let result = direct(
                &mut escalate,
                &mut world,
                Tool::Escalate,
                Some(&ticket),
                &bad,
            );
            assert_invalid(&result, "trigger", WHY_TRIGGER);
        }
        let mut without_trigger = valid.clone();
        without_trigger
            .as_object_mut()
            .expect("an object")
            .remove("trigger");
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &without_trigger,
        );
        assert_invalid(&result, "trigger", WHY_TRIGGER);

        let mut bad = valid.clone();
        bad["question"] = json!(7);
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &bad,
        );
        assert_invalid(&result, "question", WHY_QUESTION);
        let mut bad = valid.clone();
        bad["recommendation"] = json!({"a": 1});
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &bad,
        );
        assert_invalid(&result, "recommendation", WHY_RECOMMENDATION);

        let seventeen: Vec<Value> = (0..17).map(|_| finding("what", "where")).collect();
        for evidence in [
            json!("x"),
            json!(null),
            json!({}),
            json!([]),
            json!([7]),
            json!([{"what": "w"}]),
            json!([{"found_at": "f"}]),
            json!([{"what": 7, "found_at": "f"}]),
            json!([{"what": "w", "found_at": null}]),
            json!([finding("what", "where"), "x"]),
            Value::Array(seventeen),
        ] {
            let mut bad = valid.clone();
            bad["evidence"] = evidence;
            let result = direct(
                &mut escalate,
                &mut world,
                Tool::Escalate,
                Some(&ticket),
                &bad,
            );
            assert_invalid(&result, "evidence", WHY_EVIDENCE);
        }
        let mut without_evidence = valid.clone();
        without_evidence
            .as_object_mut()
            .expect("an object")
            .remove("evidence");
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &without_evidence,
        );
        assert_invalid(&result, "evidence", WHY_EVIDENCE);

        assert!(seen.untouched(), "no fallback reached a desk");
    }

    // -------------------------------------------------------------------
    // 11. The bounds
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0188_text_over_the_schemas_bound_fails_closed() {
        let (mut plan, mut escalate, seen) = handlers();
        let (mut world, _unused) = world_ok("over-bound", Role::Coder, Some(ticket_t()));
        let ticket = ticket_t();

        // Called directly, past S2's schema, each text one character over.
        let scope = json!(["crates/a"]);
        let result = direct(
            &mut plan,
            &mut world,
            Tool::PlanSubmit,
            Some(&ticket),
            &plan_arguments(&"a".repeat(65_537), &scope),
        );
        assert_unavailable(&result, WHAT_PLAN);

        let finding_list = json!([finding("what", "where")]);
        let valid = escalate_arguments("security", "q", "r", &finding_list);
        let mut over = valid.clone();
        over["question"] = json!("q".repeat(8_193));
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &over,
        );
        assert_unavailable(&result, WHAT_QUESTION);

        let mut over = valid.clone();
        over["recommendation"] = json!("r".repeat(8_193));
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &over,
        );
        assert_unavailable(&result, WHAT_RECOMMENDATION);

        let mut over = valid.clone();
        over["evidence"] = json!([finding(&"w".repeat(2_049), "where")]);
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &over,
        );
        assert_unavailable(&result, WHAT_EVIDENCE);

        let mut over = valid;
        over["evidence"] = json!([finding("what", &"f".repeat(1_025))]);
        let result = direct(
            &mut escalate,
            &mut world,
            Tool::Escalate,
            Some(&ticket),
            &over,
        );
        assert_unavailable(&result, WHAT_EVIDENCE);

        assert!(seen.untouched(), "an over-bound text reached a desk");

        // Through `World::call`, each text of exactly the schema's bound reaches
        // the desk whole. Characters, not bytes: the plan's are two bytes each.
        let (mut world, seen) = world_ok("at-bound-plan", Role::Coder, Some(ticket_t()));
        let tip = world.tip();
        let plan = "\u{e9}".repeat(65_536);
        let reply = world.call(
            "aicd_plan_submit",
            plan_arguments(&plan, &json!(["crates/a"])),
        );
        assert_success(&reply);
        let received = seen.plans();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].plan, plan, "the plan whole");
        assert_eq!(kinds(&world.rows_since(tip)), ["mcp.tool_called"]);

        let (mut world, seen) = world_ok("at-bound-escalation", Role::Lead, None);
        let question = "q".repeat(8_192);
        let recommendation = "r".repeat(8_192);
        let what = "w".repeat(2_048);
        let found_at = "f".repeat(1_024);
        let reply = world.call(
            "aicd_escalate",
            escalate_arguments(
                "security",
                &question,
                &recommendation,
                &json!([finding(&what, &found_at)]),
            ),
        );
        assert_success(&reply);
        let received = seen.escalations();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].question, question);
        assert_eq!(received[0].recommendation, recommendation);
        assert_eq!(received[0].evidence, [(what, found_at)]);
    }
}
