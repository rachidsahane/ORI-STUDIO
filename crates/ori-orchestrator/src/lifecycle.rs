//! The ticket lifecycle as the step that assembles what the machine decides
//! on: AICD §11, AICD §12, AICD §17.
//!
//! `spec/LLD.md` section 2 gives this crate `Lifecycle` (validated
//! transitions). This module is that entry, and like `crate::closing` it is
//! smaller than the name suggests, because the transitions are already
//! validated once and validating them twice is the defect.
//!
//! # Why this is not a second state machine
//!
//! `crates/ori-core/src/ticket.rs` holds the machine: the fourteen edges of
//! `spec/DATA_MODEL.md` section 3's Ticket diagram, the upgrade-only rule of
//! AICD §11, the closing guards of AICD §11 and AICD §16, and the
//! test-modification guard of AICD §12. It passes the criteria it claims. A
//! copy of any of that here would be a copy that can drift, and two
//! enforcements of one rule that disagree are worse than one that is missing,
//! because the second one looks like coverage.
//!
//! What that crate cannot do is read anything. `spec/LLD.md` section 2 forbids
//! it IO and every workspace import, and its own documentation says whose job
//! the inputs are: "The caller reads the log, which this crate may not, and
//! states what it found." Its list of what it does not decide is this module's
//! list of what it does:
//!
//! > The diagram writes "lead assigns, scope locked" on the edge into
//! > `InProgress`. Neither half is checkable here. `Actor` carries an identity
//! > and not a role, so "the lead" is the permission function's question
//! > (ORI-T-0022), and the lock table that makes "scope locked" true is
//! > `LockTable` in `ori-orchestrator`.
//!
//! So three things stand between an RPC of `spec/API_SPEC.md` section 1 and
//! `Ticket::apply`, and no component owns any of them:
//!
//! 1. the role behind the actor, which AICD §17's matrix is indexed by and
//!    which `ori-core` cannot look up because `AgentIdentity` is a row of
//!    `ori-store`;
//! 2. the lock table, which is `crate::lock_table` and which `ori-core` may not
//!    import;
//! 3. what the log holds about a close, which is `crate::closing` and which
//!    `ori-core` may not read.
//!
//! [`Lifecycle::advance`] is the one call that puts all three in front of the
//! machine and then lets the machine answer. It refuses on its own account in
//! exactly one case, the permission matrix, because that is the one rule of the
//! three that is a rule rather than an input.
//!
//! ```mermaid
//! flowchart TB
//!   RPC[a tickets.* call] --> P{AICD §17: may this role<br/>take this verb on tickets?}
//!   P -- refused --> NP[LifecycleError::NotPermitted<br/>carrying the matrix cell verbatim]
//!   P -- granted or not governed --> L{entering or leaving<br/>an in-flight state?}
//!   L -- entering --> C[LockTable::claim the declared scope]
//!   L -- leaving --> R[LockTable::release every module]
//!   L -- neither --> M
//!   C -- overlaps --> SL[E_SCOPE_LOCKED, nothing recorded]
//!   C -- admitted --> M[ori-core Ticket::apply]
//!   R --> M
//!   M -- refused --> RF[the refusal and its MethodologyRef, nothing recorded]
//!   M -- accepted --> A[Advance: the new ticket and the new table, together]
//! ```
//!
//! # The order of the three, and why it is this one
//!
//! `ori-core` had the same problem and its module documentation states the
//! shape of the answer: it puts the entry guards of a destination before the
//! transition table, because criterion ORI-P1-006 describes a ticket in
//! `Merged` while the diagram draws `Deployed --> Closed`, and a machine that
//! consulted the table first would refuse that call "with the right section and
//! the wrong rule". The criteria are the fixed points; the route the ticket
//! took is answered after the rule the criterion names.
//!
//! Criterion ORI-P1-008 in `spec/criteria/phase-1.md` puts the same question
//! here, verbatim: "Two validated tickets with overlapping declared scope |
//! Lead assigns both | Second returns `E_SCOPE_LOCKED`". A ticket in
//! `Validated` is one edge short of `Queued`, which is the state the diagram
//! assigns out of, so a lifecycle that asked the machine first would answer the
//! second assignment with `RefusalKind::TicketTransition` and the criterion
//! would be unreachable through this module while every test still passed. The
//! lock is therefore claimed before the machine is asked, for the reason
//! `ori-core` answers guards before its table.
//!
//! The permission check comes before both. An actor that AICD §17 refuses the
//! verb to has not earned an answer about the contents of the lock table or
//! about which state a ticket is in, and a refusal that named a module would
//! tell it one of those.
//!
//! Nothing is recorded on any refusal. [`LockTable::claim`] and
//! [`LockTable::release`] return new tables rather than mutating, and this
//! module returns the new table only inside [`Advance`], which it builds only
//! after the machine has accepted. A claim made for a transition the machine
//! then refused would be a module held by a ticket that never started, which is
//! a lock nobody can release.
//!
//! # Which states hold locks
//!
//! AICD §12 has the lead keep "a lock table of modules currently claimed by
//! in-flight tickets" and never says which states are in flight, so
//! [`in_flight`] derives it from `spec/DATA_MODEL.md` section 3's diagram, one
//! state at a time, and the function's own documentation carries the argument
//! for each. The three that hold locks are `InProgress`, `Escalated` and
//! `InReview`.
//!
//! # What this module still does not decide
//!
//! Which role an identity holds. [`Lifecycle::advance`] takes the role as an
//! argument because `permits` does, and that function's documentation says why:
//! "The pairing of `actor` and `role` is the caller's to get right: which role
//! an agent identity holds is a field of `AgentIdentity` in
//! `spec/DATA_MODEL.md` section 2, which lives in `ori-store`". This crate does
//! not read `ori-store` either, so the pairing is passed through rather than
//! resolved.
//!
//! Whether a human or the engine may act. `permits` answers
//! `Decision::NotGoverned` for both, and this module treats that as "not
//! refused here" and not as a grant: AICD §18's seats and
//! `spec/SECURITY_NOTES.md` are where a human's authority comes from, and
//! neither is readable from this crate. The consequence is stated rather than
//! hidden: this module refuses an agent that AICD §17 refuses and refuses no
//! human, which is also what criterion ORI-P1-005 requires of the one verb it
//! names.
//!
//! Must not: hold a rule `ori-core` already holds, decide a transition, or read
//! the log itself; `spec/LLD.md` section 2 puts the log in `ori-store` and the
//! decision in `ori-core`.

use std::fmt;

use ori_core::error::Error;
use ori_core::error::MethodologyRef;
use ori_core::permission;
use ori_core::permission::Action;
use ori_core::permission::Resource;
use ori_core::ticket::CriterionCoverage;
use ori_core::ticket::SpecUpdate;
use ori_core::ticket::TicketEvent;
use ori_core::ticket::TicketEventKind;
use ori_core::types::Actor;
use ori_core::types::Role;
use ori_core::types::Ticket;
use ori_core::types::TicketState;

use crate::closing::ClosingEvidence;
use crate::closing::CriterionState;
use crate::closing::DocumentationOutcome;
use crate::lock_table::LockTable;

// ---------------------------------------------------------------------------
// Which states hold locks
// ---------------------------------------------------------------------------

/// Whether a ticket in this state is in flight in AICD §12's sense: AICD §12.
///
/// Derived from AICD §12's "lock table of modules currently claimed by
/// in-flight tickets", which is the only place the phrase appears and which
/// does not enumerate the states. The enumeration is read off
/// `spec/DATA_MODEL.md` section 3's Ticket diagram instead, one state at a
/// time, and the reasoning is written here because a list with no argument is a
/// list a later reader cannot check.
///
/// True for three states:
///
/// - `InProgress`, which the data model's own field documentation defines as
///   "Assigned, with its declared scope locked". This is the state the lock
///   exists for.
/// - `Escalated`, because the diagram draws `InProgress --> Escalated: trigger`
///   and `Escalated --> InProgress: answered`. The coder stopped and a human
///   was asked; the worktree and the branch are still the ticket's, and a
///   second coder admitted to those modules meanwhile would be editing files
///   the first will come back to.
/// - `InReview`, because the diagram draws `InReview --> InProgress: changes
///   requested`. The pull request is open and can be sent back, so the modules
///   are not free.
///
/// False for everything else, and the two that are worth naming:
///
/// - `Blocked` releases. `spec/PRD.md` section 4's F-08 orders the kill path
///   "revokes credentials first, terminates second, releases locks third", and
///   criterion ORI-P1-009 ends that session and returns the ticket "to Queued
///   on re-plan", which is a fresh claim through [`LockTable::claim`].
/// - `Merged` releases, which is what criterion ORI-P1-008's "after the first
///   closes, the second starts" needs. The diagram draws no edge from `Merged`
///   back to `InProgress`, so nothing can send the change back to a coder and
///   the modules are free from that point.
#[must_use]
pub const fn in_flight(state: TicketState) -> bool {
    matches!(
        state,
        TicketState::InProgress | TicketState::Escalated | TicketState::InReview
    )
}

// ---------------------------------------------------------------------------
// Which verb of the permission matrix an event asks for
// ---------------------------------------------------------------------------

/// The verb AICD §17's matrix names for this move, when it names one:
/// AICD §17.
///
/// Derived from AICD §17's tickets column, which grants the lead "Read, assign,
/// upgrade category", and from the labels `spec/DATA_MODEL.md` section 3 writes
/// on the edges. Two moves out of the whole lifecycle carry a verb of that
/// column, and this function returns `None` for every other move rather than
/// inventing one: AICD §17's principle is that the matrix "grants nothing by
/// implication", and a control that refused a move no cell describes would be
/// denying by implication, which is the same fault mirrored.
///
/// - `Queued --> InProgress` is [`Action::Assign`]. The diagram labels that one
///   edge "lead assigns"; the two other edges into `InProgress` are labelled
///   "answered" and "changes requested" and are not assignments, so they carry
///   no verb.
/// - `TicketEventKind::CategorySet` is [`Action::UpgradeCategory`].
///   `spec/API_SPEC.md` section 1 exposes it as `tickets.setCategory`, which is
///   the call the matrix's "upgrade category" describes.
///
/// `TicketEventKind::Categorized` deliberately carries no verb even though it
/// also sets the category. AICD §11 says "The QA agent proposes the category
/// when it files a ticket", and AICD §17 grants the QA role "Create, read" on
/// tickets and not "upgrade category". Requiring the upgrade verb at filing
/// would refuse the QA agent the one thing AICD §11 names it for. The
/// upgrade-only rule still applies to that event, in `ori-core`, where it
/// applies to both.
#[must_use]
pub const fn verb(from: TicketState, kind: &TicketEventKind) -> Option<Action> {
    match kind {
        TicketEventKind::InProgress if matches!(from, TicketState::Queued) => Some(Action::Assign),
        TicketEventKind::CategorySet { .. } => Some(Action::UpgradeCategory),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The refusal
// ---------------------------------------------------------------------------

/// Why a lifecycle call was refused: AICD §17.
///
/// Two variants, because there are two places a refusal can come from and they
/// are not the same kind of fact. [`LifecycleError::Refused`] is `ori-core`
/// speaking, through the lock table or through the ticket machine, and is
/// passed along unchanged so that its `RefusalKind`, its section and its code
/// reach the caller exactly as that crate wrote them.
/// [`LifecycleError::NotPermitted`] is this module's own and its only one.
///
/// The variant exists because `Decision` is not an `Error`: `permits` returns
/// a decision rather than a result, and its own documentation says why, which
/// is that "expressing a refusal as `crate::error::Error` needs a
/// `crate::error::RefusalKind` variant that does not exist". Adding that
/// variant is a change to `ori-core`'s public error enum and therefore an
/// AICD §12 escalation with trigger `contract_change`; it is not taken here.
/// Nothing is lost by the shape, because [`LifecycleError::reason`] carries the
/// `MethodologyRef` that CLAUDE.md rule 9 and criterion ORI-P1-033 require, in
/// the same spelling `crate::escalation::EscalationError` uses in this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    /// AICD §17's matrix does not grant this role this verb on the ticket
    /// queue.
    NotPermitted {
        /// The role the caller resolved for the actor.
        role: Role,
        /// The verb the move asked for.
        action: Action,
        /// The section the refusal is made under, from `permission::refusal_for`.
        reason: MethodologyRef,
        /// The cell of AICD §17's matrix the decision came from, verbatim.
        ///
        /// This is what PRD A-10's "explain" shows a human beside a refusal:
        /// not a paraphrase of the rule but the row of the table it came from.
        cell: &'static str,
    },
    /// `ori-core` refused, through the lock table or through the ticket
    /// machine.
    Refused(Error),
}

impl LifecycleError {
    /// The methodology section this refusal was made under.
    ///
    /// Every refusal carries one, which is CLAUDE.md rule 9 and criterion
    /// ORI-P1-033. `Option` rather than a bare reference because
    /// `Error::methodology_ref` is an `Option`: that enum also carries
    /// `Error::Malformed`, which is a value that did not parse and not a
    /// control refusing anything. No lifecycle call produces that variant
    /// today, and answering `None` if one ever did is honest where inventing a
    /// section would not be.
    #[must_use]
    pub fn reason(&self) -> Option<MethodologyRef> {
        match self {
            Self::NotPermitted { reason, .. } => Some(reason.clone()),
            Self::Refused(error) => error.methodology_ref(),
        }
    }

    /// The error code `spec/API_SPEC.md` names for this refusal, where it names
    /// one.
    ///
    /// Delegated whole to `Error::code`, which returns `E_UPGRADE_ONLY` and
    /// `E_SCOPE_LOCKED` and nothing else. A permission refusal has no code:
    /// `spec/API_SPEC.md` names exactly two and inventing a third would add a
    /// value to the client API, which CLAUDE.md makes an escalation with
    /// trigger `contract_change`.
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::NotPermitted { .. } => None,
            Self::Refused(error) => error.code(),
        }
    }

    /// The refusal `ori-core` made, when the refusal came from there.
    ///
    /// No methodology section applies: this is the accessor, not a rule.
    #[must_use]
    pub const fn refusal(&self) -> Option<&Error> {
        match self {
            Self::NotPermitted { .. } => None,
            Self::Refused(error) => Some(error),
        }
    }
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPermitted {
                role,
                action,
                reason,
                cell,
            } => write!(
                f,
                "refused: the {role} role may not {action} on the ticket queue ({reason}); \
                 AICD §17 grants it \"{cell}\""
            ),
            Self::Refused(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LifecycleError {}

impl From<Error> for LifecycleError {
    fn from(error: Error) -> Self {
        Self::Refused(error)
    }
}

// ---------------------------------------------------------------------------
// What an accepted move produces
// ---------------------------------------------------------------------------

/// The ticket and the lock table after one accepted move: AICD §11, AICD §12.
///
/// The two travel together because they move together or not at all. A ticket
/// that entered `InProgress` and a table that did not record its modules would
/// be a coder working files nothing claims, and a table that recorded them
/// beside a ticket that did not move would be a lock nobody can release. One
/// value makes the pair the only thing a caller can be handed.
///
/// The fields are private and [`Lifecycle::advance`] is the only constructor,
/// for the reason `crate::closing::ClosingEvidence` gives: a caller that could
/// set them separately could state a pairing that never happened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Advance {
    ticket: Ticket,
    lifecycle: Lifecycle,
}

impl Advance {
    /// The ticket as the accepted event left it.
    ///
    /// No methodology section applies beyond the type's own: this is the
    /// accessor.
    #[must_use]
    pub const fn ticket(&self) -> &Ticket {
        &self.ticket
    }

    /// The lifecycle, carrying the lock table as the accepted event left it.
    ///
    /// No methodology section applies beyond the type's own: this is the
    /// accessor.
    #[must_use]
    pub const fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }

    /// The pair, for a caller that keeps both.
    ///
    /// No methodology section applies beyond the type's own: this is the
    /// accessor.
    #[must_use]
    pub fn into_parts(self) -> (Ticket, Lifecycle) {
        (self.ticket, self.lifecycle)
    }
}

// ---------------------------------------------------------------------------
// The lifecycle
// ---------------------------------------------------------------------------

/// The validated ticket transitions of `spec/LLD.md` section 2: AICD §11,
/// AICD §12, AICD §17.
///
/// Holds the lock table, because AICD §12 gives the lead agent exactly one
/// thing to maintain across tickets and this is the component the lead runs.
/// Everything else a decision needs is passed in per call, because everything
/// else is a fact about the one ticket in hand.
///
/// A value with pure transitions, for the three reasons
/// `crate::lock_table::LockTable` states at length: a refused move must record
/// nothing, the table is derived state folded out of the append-only log, and
/// every state machine in this workspace already has this shape.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Lifecycle {
    locks: LockTable,
}

impl Lifecycle {
    /// A lifecycle holding nothing.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            locks: LockTable::new(),
        }
    }

    /// A lifecycle over a lock table that already holds claims.
    ///
    /// No methodology section applies: this is the constructor a projector uses
    /// when it has folded the log into a table and needs the lifecycle that
    /// carries it.
    #[must_use]
    pub const fn over(locks: LockTable) -> Self {
        Self { locks }
    }

    /// The lock table this lifecycle carries.
    ///
    /// Borrowed rather than copied so that a caller reading it cannot hold a
    /// table that a later accepted move has already replaced.
    #[must_use]
    pub const fn locks(&self) -> &LockTable {
        &self.locks
    }

    /// Applies one event to one ticket, with everything `ori-core` cannot see:
    /// AICD §11, AICD §12, AICD §17.
    ///
    /// `role` is the role the caller resolved for `event.actor` from
    /// `AgentIdentity`; see the module documentation for why it is passed
    /// rather than looked up. The three steps run in the order the module
    /// documentation argues for, and each is a call into the component that
    /// owns the rule:
    ///
    /// 1. `permission::permits` for the verb [`verb`] names, when it names one.
    /// 2. [`LockTable::claim`] of the ticket's own `declared_scope` when the
    ///    move enters an [`in_flight`] state from one that is not, and
    ///    [`LockTable::release`] when it leaves one. A move between two
    ///    in-flight states changes nothing, which is what keeps a ticket's
    ///    modules held while it is escalated or in review.
    /// 3. `Ticket::apply`, which is the whole of the decision about the
    ///    transition itself.
    ///
    /// The state a ticket is in is never screened before those steps.
    /// `crate::closing` records why as a duty on exactly this caller: criteria
    /// ORI-P1-006 and ORI-P1-007 both describe a ticket in `Merged` being
    /// closed, and "a handler that refused `tickets.close` on a `Merged` ticket
    /// for its state alone would make both criteria unreachable while every
    /// test below still passed". This function reads the state to choose a verb
    /// and to move locks, and refuses on it never; the only component that
    /// refuses a transition is the machine, which answers the closing guard
    /// first.
    ///
    /// # Errors
    ///
    /// [`LifecycleError::NotPermitted`] when AICD §17's matrix does not grant
    /// the role the verb the move asks for.
    ///
    /// [`LifecycleError::Refused`] carrying `ori-core`'s own refusal: the lock
    /// table's `RefusalKind::ScopeLocked`, whose code is `E_SCOPE_LOCKED`
    /// (criterion ORI-P1-008), or any refusal `Ticket::apply` makes, including
    /// `RefusalKind::CategoryDowngrade`, whose code is `E_UPGRADE_ONLY`
    /// (criterion ORI-P1-005).
    ///
    /// On every one of those, nothing is recorded: no lock is taken, no lock is
    /// released, and the ticket and the lifecycle the caller holds are
    /// untouched.
    pub fn advance(
        &self,
        ticket: &Ticket,
        event: &TicketEvent,
        role: Role,
    ) -> Result<Advance, LifecycleError> {
        self.check_permission(ticket.state, event, role)?;
        let locks = self.moved_locks(ticket, event)?;
        let ticket = ticket.apply(event)?;
        Ok(Advance {
            ticket,
            lifecycle: Self { locks },
        })
    }

    /// AICD §17's matrix, asked about the verb this move carries.
    ///
    /// A move that carries no verb is not checked, and a `Decision` other than
    /// `Decision::Refused` passes. `Decision::NotGoverned` is deliberately not
    /// treated as a grant and is also not a refusal here; the module
    /// documentation states what that costs and where a human's authority is
    /// decided instead.
    fn check_permission(
        &self,
        from: TicketState,
        event: &TicketEvent,
        role: Role,
    ) -> Result<(), LifecycleError> {
        let Some(action) = verb(from, &event.kind) else {
            return Ok(());
        };
        let decision = permission::permits(&event.actor, role, Resource::Tickets, action);
        if let Some(reason) = decision.methodology_ref() {
            return Err(LifecycleError::NotPermitted {
                role,
                action,
                reason,
                cell: permission::cell(role, Resource::Tickets),
            });
        }
        Ok(())
    }

    /// The lock table this move would leave behind, claimed or released.
    ///
    /// AICD §12's table holds the modules of in-flight tickets, so the table
    /// changes exactly when the move crosses the [`in_flight`] boundary. An
    /// event that moves no state cannot cross it and leaves the table alone.
    fn moved_locks(&self, ticket: &Ticket, event: &TicketEvent) -> Result<LockTable, Error> {
        let Some(to) = event.kind.target() else {
            return Ok(self.locks.clone());
        };
        match (in_flight(ticket.state), in_flight(to)) {
            (false, true) => self.locks.claim(&ticket.id, &ticket.declared_scope),
            (true, false) => Ok(self.locks.release(&ticket.id)),
            (false, false) | (true, true) => Ok(self.locks.clone()),
        }
    }
}

// ---------------------------------------------------------------------------
// The evidence a close is decided on
// ---------------------------------------------------------------------------

/// Turns what the log holds into the two facts the machine closes on:
/// AICD §11, AICD §16.
///
/// This is the step `crate::closing` names and could not take. That module
/// reads the records and the criteria and produces witnesses; `ori-core` takes
/// two verdicts and decides; and the sentence between them was unwritable while
/// this crate's manifest had no `ori-core` edge, which it now has. Nothing
/// about the rule is decided here. Each answer is a restatement of a witness,
/// and `Ticket::apply` is what says whether the pair closes the ticket.
///
/// The two mappings, each with the reason it is not the obvious one:
///
/// - A documentation record becomes `SpecUpdate::Recorded` or
///   `SpecUpdate::NoChangeNeeded` by its own outcome, and the absence of a
///   record becomes `SpecUpdate::Absent`. AICD §11 accepts both answers alike,
///   so the distinction survives only because it is what a human reading the
///   refusal or the acceptance wants to see.
/// - An accepted criterion becomes `CriterionCoverage::Accepted`. Where there
///   is none, a criterion that references the ticket and is still `proposed`
///   becomes `CriterionCoverage::Proposed`, and anything else becomes
///   `CriterionCoverage::Absent`. The middle case is not folded into the last
///   one because `ori-core` gives it a value of its own and says why: folding
///   it "would refuse it correctly and explain it wrongly, by telling an author
///   that no criterion references the ticket when one does". A `rejected` or
///   `superseded` criterion is not a proposal either, so it is `Absent`: it
///   references the ticket and is not on its way to accepting it.
#[must_use]
pub fn closing_event(actor: Actor, evidence: &ClosingEvidence) -> TicketEvent {
    let spec_update = match evidence.documentation() {
        Some(record) => match record.outcome {
            DocumentationOutcome::SpecUpdate => SpecUpdate::Recorded,
            DocumentationOutcome::NoChangeNeeded => SpecUpdate::NoChangeNeeded,
        },
        None => SpecUpdate::Absent,
    };
    let criterion = if evidence.accepted_criterion().is_some() {
        CriterionCoverage::Accepted
    } else if evidence
        .unaccepted_criteria()
        .iter()
        .any(|reference| matches!(reference.state, CriterionState::Proposed))
    {
        CriterionCoverage::Proposed
    } else {
        CriterionCoverage::Absent
    };
    TicketEvent::new(
        actor,
        TicketEventKind::Closed {
            spec_update,
            criterion,
        },
    )
}

#[cfg(test)]
mod tests {
    use ori_core::error::RefusalKind;
    use ori_core::types::Budget;
    use ori_core::types::Category;
    use ori_core::types::Id;
    use ori_core::types::Scope;
    use ori_core::types::SpecAnchor;
    use ori_core::types::TicketKind;
    use ori_core::types::Tier;

    use super::*;
    use crate::closing::CriterionRef;
    use crate::closing::DocumentationRecord;

    fn id(text: &str) -> Id {
        Id::parse(text).expect("a canonical ULID")
    }

    fn human() -> Actor {
        Actor::Human(id("01ARZ3NDEKTSV4RRFFQ69G5FAV"))
    }

    fn agent() -> Actor {
        Actor::Agent(id("01BX5ZZKBKACTAV9WEVGEMMVRZ"))
    }

    /// A ticket in `state` whose declared scope is `modules`.
    fn ticket_with(
        ticket_id: &str,
        state: TicketState,
        kind: TicketKind,
        category: Category,
        modules: &[&str],
    ) -> Ticket {
        Ticket {
            id: id(ticket_id),
            product_id: id("01F8MECHZX3TBDSZ7XR8H8JHAF"),
            title: "Lifecycle: validated transitions, categories, tiers".to_owned(),
            category,
            kind,
            tier: Tier::Two,
            state,
            spec_anchor: SpecAnchor::parse("DATA_MODEL.md#3-state-machines").expect("an anchor"),
            declared_scope: Scope::new(modules).expect("a scope"),
            budget: Budget {
                attempts: 3,
                wall_clock_s: 2700,
                tokens: 0,
            },
            filed_by: agent(),
            phase_id: id("01F8MECHZX3TBDSZ7XR8H8JHAG"),
            significant: None,
            incident_id: None,
        }
    }

    fn ticket(state: TicketState) -> Ticket {
        ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            state,
            TicketKind::Feature,
            Category::Auto,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        )
    }

    fn other_ticket(state: TicketState) -> Ticket {
        ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNW",
            state,
            TicketKind::Feature,
            Category::Auto,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        )
    }

    fn assign(actor: Actor) -> TicketEvent {
        TicketEvent::new(actor, TicketEventKind::InProgress)
    }

    fn set_category(actor: Actor, category: Category) -> TicketEvent {
        TicketEvent::new(actor, TicketEventKind::CategorySet { category })
    }

    // -----------------------------------------------------------------------
    // ORI-P1-005: the upgrade-only rule, end to end through this seam.
    //
    // `ori-core`'s ticket machine also claims this criterion, with tests named
    // ori_p1_005_*. It claims the decision: the ladder, the downgrade and the
    // E_UPGRADE_ONLY code. These claim the half of the criterion's sentence
    // that crate cannot reach, which is the word "Lead". `Actor` carries an
    // identity and not a role, so the criterion's actor cannot be checked
    // there; here the role is an argument and AICD §17's matrix decides it.
    // The closing report names the double claim.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_005_the_lead_upgrades_a_ticket_an_agent_filed_as_auto() {
        let filed = ticket(TicketState::Queued);
        assert_eq!(filed.category, Category::Auto, "the precondition");

        let upgraded = Lifecycle::new()
            .advance(
                &filed,
                &set_category(agent(), Category::Decisional),
                Role::Lead,
            )
            .expect("AICD §17 grants the lead the upgrade category verb");

        assert_eq!(
            upgraded.ticket().category,
            Category::Decisional,
            "the criterion's expected result: the category is Decisional"
        );
    }

    #[test]
    fn ori_p1_005_an_agent_lowering_the_category_back_receives_e_upgrade_only() {
        let decisional = ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            TicketState::Queued,
            TicketKind::Feature,
            Category::Decisional,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        );

        let refusal = Lifecycle::new()
            .advance(
                &decisional,
                &set_category(agent(), Category::Auto),
                Role::Lead,
            )
            .expect_err("an agent may not lower a category");

        assert_eq!(
            refusal.code(),
            Some("E_UPGRADE_ONLY"),
            "the criterion names the code the client API returns"
        );
        assert_eq!(
            refusal.reason().map(|reference| reference.section),
            Some(11),
            "the refusal cites AICD §11's upgrade-only rule"
        );
    }

    #[test]
    fn ori_p1_005_a_human_lowering_the_category_back_is_allowed() {
        let decisional = ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            TicketState::Queued,
            TicketKind::Feature,
            Category::Decisional,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        );

        let lowered = Lifecycle::new()
            .advance(
                &decisional,
                &set_category(human(), Category::Auto),
                Role::Lead,
            )
            .expect("the criterion's last clause: a human can");

        assert_eq!(lowered.ticket().category, Category::Auto);
    }

    #[test]
    fn ori_p1_005_a_role_the_matrix_does_not_grant_the_verb_to_is_refused_here() {
        let filed = ticket(TicketState::Queued);

        for role in [Role::Coder, Role::Qa, Role::Documentation, Role::Assistant] {
            let refusal = Lifecycle::new()
                .advance(&filed, &set_category(agent(), Category::Decisional), role)
                .expect_err("AICD §17 grants upgrade category to the lead alone");

            assert!(
                matches!(refusal, LifecycleError::NotPermitted { .. }),
                "the refusal is the matrix's and not the machine's, for {role}"
            );
            assert_eq!(
                refusal.reason().map(|reference| reference.section),
                Some(17),
                "a permission refusal cites AICD §17"
            );
        }
    }

    #[test]
    fn ori_p1_005_the_lead_is_the_only_role_the_matrix_grants_the_verb_to() {
        let granted: Vec<Role> = Role::ALL
            .iter()
            .copied()
            .filter(|role| {
                permission::permits(&agent(), *role, Resource::Tickets, Action::UpgradeCategory)
                    .is_allowed()
            })
            .collect();

        assert_eq!(
            granted,
            vec![Role::Lead],
            "the criterion says Lead, and AICD §17's tickets column says so too"
        );
    }

    // -----------------------------------------------------------------------
    // The verb table: which moves AICD §17 names, and which it does not.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0049_the_only_moves_carrying_a_permission_verb_are_assignment_and_set_category() {
        let named = [
            (
                TicketState::Queued,
                TicketEventKind::InProgress,
                Action::Assign,
            ),
            (
                TicketState::Queued,
                TicketEventKind::CategorySet {
                    category: Category::Decisional,
                },
                Action::UpgradeCategory,
            ),
        ];
        for (from, kind, action) in named {
            assert_eq!(verb(from, &kind), Some(action), "{from} to {kind:?}");
        }

        let unnamed = [
            (TicketState::Escalated, TicketEventKind::InProgress),
            (TicketState::InReview, TicketEventKind::InProgress),
            (
                TicketState::Filed,
                TicketEventKind::Categorized {
                    category: Category::Decisional,
                },
            ),
            (TicketState::Categorized, TicketEventKind::Validated),
            (TicketState::Categorized, TicketEventKind::Rejected),
            (TicketState::Validated, TicketEventKind::Queued),
            (TicketState::InProgress, TicketEventKind::Blocked),
            (TicketState::InProgress, TicketEventKind::Escalated),
            (
                TicketState::InReview,
                TicketEventKind::Merged { significant: true },
            ),
            (TicketState::Merged, TicketEventKind::Deployed),
            (
                TicketState::Queued,
                TicketEventKind::TierSet { tier: Tier::Two },
            ),
        ];
        for (from, kind) in unnamed {
            assert_eq!(
                verb(from, &kind),
                None,
                "AICD §17 names no verb for {from} to {kind:?}, and none is invented"
            );
        }
    }

    #[test]
    fn ori_t_0049_the_qa_agent_may_carry_a_category_into_a_ticket_it_files() {
        let filed = ticket(TicketState::Filed);

        let categorized = Lifecycle::new()
            .advance(
                &filed,
                &TicketEvent::new(
                    agent(),
                    TicketEventKind::Categorized {
                        category: Category::Behavioral,
                    },
                ),
                Role::Qa,
            )
            .expect("AICD §11: the QA agent proposes the category when it files a ticket");

        assert_eq!(categorized.ticket().category, Category::Behavioral);
    }

    // -----------------------------------------------------------------------
    // The assignment edge: "lead assigns, scope locked".
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0049_an_accepted_assignment_moves_the_ticket_and_claims_its_modules_together() {
        let queued = ticket(TicketState::Queued);

        let advance = Lifecycle::new()
            .advance(&queued, &assign(agent()), Role::Lead)
            .expect("the lead may assign a queued ticket");

        assert_eq!(advance.ticket().state, TicketState::InProgress);
        assert_eq!(
            advance
                .lifecycle()
                .locks()
                .holder_of("crates/ori-orchestrator/src/lifecycle.rs"),
            Some(&queued.id),
            "spec/DATA_MODEL.md section 3 labels the edge \"lead assigns, scope locked\""
        );
    }

    #[test]
    fn ori_t_0049_a_role_that_may_not_assign_is_refused_before_the_table_is_consulted() {
        let queued = ticket(TicketState::Queued);

        let refusal = Lifecycle::new()
            .advance(&queued, &assign(agent()), Role::Coder)
            .expect_err("AICD §17 grants the coder read own and comment, not assign");

        match refusal {
            LifecycleError::NotPermitted {
                role, action, cell, ..
            } => {
                assert_eq!(role, Role::Coder);
                assert_eq!(action, Action::Assign);
                assert_eq!(
                    cell, "Read own; comment",
                    "the refusal shows the matrix cell it came from, verbatim"
                );
            }
            LifecycleError::Refused(error) => {
                panic!("the matrix answers first, not the table or the machine: {error}")
            }
        }
    }

    // -----------------------------------------------------------------------
    // ORI-P1-008: the scope lock, end to end through this seam.
    //
    // `crate::lock_table` already claims this criterion in full, with tests
    // named ori_p1_008_*: the refusal, the one entry per module, and "after
    // the first closes, the second starts". The test below claims the same
    // identifier for the reason `ori_p1_005_the_lead_upgrades_...` above does:
    // it is not a second proof of the table, it is the proof that this module
    // asks the table before it asks the machine, over the criterion's own
    // precondition of a `Validated` ticket, which `LockTable::claim` alone
    // cannot exercise because it never sees a `TicketState`. The closing
    // report names this double claim as well.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_008_a_second_assignment_over_the_same_module_is_refused_e_scope_locked() {
        let first = ticket(TicketState::Queued);
        let second = other_ticket(TicketState::Validated);

        let after_first = Lifecycle::new()
            .advance(&first, &assign(agent()), Role::Lead)
            .expect("the first assignment is admitted");

        let refusal = after_first
            .lifecycle()
            .advance(&second, &assign(agent()), Role::Lead)
            .expect_err("the declared scopes overlap");

        assert_eq!(
            refusal.code(),
            Some("E_SCOPE_LOCKED"),
            "criterion ORI-P1-008's expected result, and the criterion's precondition is a \
             validated ticket, which is why the table is asked before the machine"
        );
    }

    #[test]
    fn ori_t_0049_a_refused_move_records_nothing_in_the_table() {
        let first = ticket(TicketState::Queued);
        let second = other_ticket(TicketState::Validated);
        let start = Lifecycle::new();

        let after_first = start
            .advance(&first, &assign(agent()), Role::Lead)
            .expect("the first assignment is admitted")
            .lifecycle()
            .clone();

        let _ = after_first
            .advance(&second, &assign(agent()), Role::Lead)
            .expect_err("refused");

        assert_eq!(
            after_first.locks().len(),
            1,
            "the lifecycle the caller holds is untouched by a refused move"
        );
        assert!(
            start.locks().is_empty(),
            "and so is the one the first move was made from"
        );
    }

    #[test]
    fn ori_t_0049_a_transition_the_diagram_does_not_draw_leaves_the_table_alone() {
        let filed = ticket(TicketState::Filed);

        let refusal = Lifecycle::new()
            .advance(&filed, &assign(agent()), Role::Lead)
            .expect_err("the diagram draws no Filed to InProgress edge");

        assert!(
            matches!(
                refusal.refusal(),
                Some(Error::Refused {
                    kind: RefusalKind::TicketTransition { .. },
                    ..
                })
            ),
            "the machine refuses the transition, and this module holds no copy of the table"
        );
    }

    // -----------------------------------------------------------------------
    // Which states hold locks.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0049_exactly_three_states_hold_locks() {
        let held: Vec<TicketState> = TicketState::ALL
            .iter()
            .copied()
            .filter(|state| in_flight(*state))
            .collect();

        assert_eq!(
            held,
            vec![
                TicketState::InProgress,
                TicketState::Escalated,
                TicketState::InReview
            ],
            "every other state of spec/DATA_MODEL.md section 3 leaves the modules free"
        );
    }

    #[test]
    fn ori_t_0049_an_escalation_and_a_review_keep_the_modules_the_coder_holds() {
        let queued = ticket(TicketState::Queued);
        let mut lifecycle = Lifecycle::new()
            .advance(&queued, &assign(agent()), Role::Lead)
            .expect("assigned")
            .lifecycle()
            .clone();
        let mut held = ticket(TicketState::InProgress);

        for kind in [
            TicketEventKind::Escalated,
            TicketEventKind::InProgress,
            TicketEventKind::InReview {
                tests: ori_core::ticket::TestModification::None,
            },
            TicketEventKind::InProgress,
        ] {
            let advance = lifecycle
                .advance(&held, &TicketEvent::new(agent(), kind), Role::Lead)
                .expect("a move between two in-flight states");
            assert!(
                advance.lifecycle().locks().holds(&held.id),
                "the ticket keeps its modules while it is in flight, in {}",
                advance.ticket().state
            );
            held = advance.ticket().clone();
            lifecycle = advance.lifecycle().clone();
        }
    }

    #[test]
    fn ori_t_0049_leaving_the_in_flight_states_releases_every_module() {
        let queued = ticket(TicketState::Queued);
        let assigned = Lifecycle::new()
            .advance(&queued, &assign(agent()), Role::Lead)
            .expect("assigned");
        let (in_progress, lifecycle) = assigned.into_parts();

        let blocked = lifecycle
            .advance(
                &in_progress,
                &TicketEvent::new(agent(), TicketEventKind::Blocked),
                Role::Lead,
            )
            .expect("a budget that ran out");

        assert!(
            blocked.lifecycle().locks().is_empty(),
            "spec/PRD.md section 4's F-08 releases the locks when the session ends"
        );
    }

    #[test]
    fn ori_t_0049_a_merged_ticket_frees_its_modules_for_the_next_one() {
        let first = ticket(TicketState::InReview);
        let second = other_ticket(TicketState::Queued);
        let lifecycle = Lifecycle::over(
            LockTable::new()
                .claim(&first.id, &first.declared_scope)
                .expect("a fresh table admits any claim"),
        );

        let merged = lifecycle
            .advance(
                &first,
                &TicketEvent::new(agent(), TicketEventKind::Merged { significant: true }),
                Role::Lead,
            )
            .expect("merged");

        let started = merged
            .lifecycle()
            .advance(&second, &assign(agent()), Role::Lead)
            .expect("criterion ORI-P1-008: after the first closes, the second starts");

        assert_eq!(started.ticket().state, TicketState::InProgress);
    }

    #[test]
    fn ori_t_0049_an_event_that_moves_no_state_leaves_the_table_exactly_as_it_was() {
        let in_progress = ticket(TicketState::InProgress);
        let lifecycle = Lifecycle::over(
            LockTable::new()
                .claim(&in_progress.id, &in_progress.declared_scope)
                .expect("a fresh table admits any claim"),
        );

        let after = lifecycle
            .advance(
                &in_progress,
                &TicketEvent::new(human(), TicketEventKind::TierSet { tier: Tier::One }),
                Role::Lead,
            )
            .expect("a human may lower a tier");

        assert_eq!(
            after.lifecycle().locks(),
            lifecycle.locks(),
            "a field change crosses no in-flight boundary"
        );
    }

    // -----------------------------------------------------------------------
    // The closing evidence, turned into the two facts the machine closes on.
    //
    // Criteria ORI-P1-006 and ORI-P1-007 are claimed by `ori-core`, which
    // makes the refusal, and by `crate::closing`, which reads the witnesses.
    // The tests here are named ori_t_0049_* because what they cover is the
    // sentence between those two and not a third claim on either criterion.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0049_every_shape_of_evidence_becomes_the_answer_the_machine_expects() {
        let cases = [
            (
                Vec::new(),
                Vec::new(),
                SpecUpdate::Absent,
                CriterionCoverage::Absent,
            ),
            (
                vec![DocumentationRecord::new(
                    4,
                    DocumentationOutcome::SpecUpdate,
                )],
                Vec::new(),
                SpecUpdate::Recorded,
                CriterionCoverage::Absent,
            ),
            (
                vec![DocumentationRecord::new(
                    4,
                    DocumentationOutcome::NoChangeNeeded,
                )],
                Vec::new(),
                SpecUpdate::NoChangeNeeded,
                CriterionCoverage::Absent,
            ),
            (
                Vec::new(),
                vec![CriterionRef::new("ORI-P1-049", CriterionState::Accepted)],
                SpecUpdate::Absent,
                CriterionCoverage::Accepted,
            ),
            (
                Vec::new(),
                vec![CriterionRef::new("ORI-P1-049", CriterionState::Proposed)],
                SpecUpdate::Absent,
                CriterionCoverage::Proposed,
            ),
            (
                Vec::new(),
                vec![
                    CriterionRef::new("ORI-P1-049", CriterionState::Rejected),
                    CriterionRef::new("ORI-P1-050", CriterionState::Superseded),
                ],
                SpecUpdate::Absent,
                CriterionCoverage::Absent,
            ),
        ];

        for (records, criteria, expected_update, expected_coverage) in cases {
            let evidence = ClosingEvidence::read(&records, &criteria);
            let event = closing_event(human(), &evidence);

            assert_eq!(
                event.kind,
                TicketEventKind::Closed {
                    spec_update: expected_update,
                    criterion: expected_coverage,
                },
                "records {records:?} and criteria {criteria:?}"
            );
        }
    }

    #[test]
    fn ori_t_0049_a_rejected_criterion_is_absent_and_not_a_proposal() {
        let evidence = ClosingEvidence::read(
            &[],
            &[CriterionRef::new("ORI-P1-049", CriterionState::Rejected)],
        );

        assert_eq!(
            closing_event(human(), &evidence).kind,
            TicketEventKind::Closed {
                spec_update: SpecUpdate::Absent,
                criterion: CriterionCoverage::Absent,
            },
            "a rejected criterion is not on its way to accepting anything"
        );
    }

    #[test]
    fn ori_t_0049_a_complete_set_of_evidence_closes_a_deployed_defect() {
        let defect = ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            TicketState::Deployed,
            TicketKind::Defect,
            Category::Auto,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        );
        let evidence = ClosingEvidence::read(
            &[DocumentationRecord::new(
                9,
                DocumentationOutcome::SpecUpdate,
            )],
            &[CriterionRef::new("ORI-P1-049", CriterionState::Accepted)],
        );

        let closed = Lifecycle::new()
            .advance(&defect, &closing_event(human(), &evidence), Role::Lead)
            .expect("both steps of AICD §16's loop are recorded");

        assert_eq!(closed.ticket().state, TicketState::Closed);
    }

    #[test]
    fn ori_t_0049_a_merged_ticket_asked_to_close_hears_the_closing_rule_and_not_its_state() {
        let merged = ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            TicketState::Merged,
            TicketKind::Feature,
            Category::Auto,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        );
        let evidence = ClosingEvidence::read(&[], &[]);

        let refusal = Lifecycle::new()
            .advance(&merged, &closing_event(human(), &evidence), Role::Lead)
            .expect_err("nothing is recorded");

        assert!(
            matches!(
                refusal.refusal(),
                Some(Error::Refused {
                    kind: RefusalKind::SpecUpdateMissing,
                    ..
                })
            ),
            "criteria ORI-P1-006 and ORI-P1-007 put the ticket in Merged; this module must not \
             screen the state and make them unreachable"
        );
        assert_eq!(
            refusal.reason().map(|reference| reference.section),
            Some(11),
            "the criterion requires the reason to cite AICD §11's closing rule"
        );
    }

    #[test]
    fn ori_t_0049_a_defect_with_the_update_and_no_accepted_criterion_hears_aicd_16() {
        let defect = ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            TicketState::Merged,
            TicketKind::Defect,
            Category::Auto,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        );
        let evidence = ClosingEvidence::read(
            &[DocumentationRecord::new(
                9,
                DocumentationOutcome::SpecUpdate,
            )],
            &[CriterionRef::new("ORI-P1-049", CriterionState::Proposed)],
        );

        let refusal = Lifecycle::new()
            .advance(&defect, &closing_event(human(), &evidence), Role::Lead)
            .expect_err("a proposed criterion is not an accepted one");

        assert_eq!(
            refusal.reason().map(|reference| reference.section),
            Some(16),
            "criterion ORI-P1-007's expected result"
        );
    }

    // -----------------------------------------------------------------------
    // The refusal itself.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0049_every_refusal_this_module_makes_carries_a_section_that_resolves() {
        let queued = ticket(TicketState::Queued);
        let decisional = ticket_with(
            "01D78XYFJ1PRM1WPBCBT3VHMNV",
            TicketState::Queued,
            TicketKind::Feature,
            Category::Decisional,
            &["crates/ori-orchestrator/src/lifecycle.rs"],
        );
        let second = other_ticket(TicketState::Queued);
        let held = Lifecycle::new()
            .advance(&queued, &assign(agent()), Role::Lead)
            .expect("assigned")
            .lifecycle()
            .clone();

        let refusals = [
            Lifecycle::new()
                .advance(&queued, &assign(agent()), Role::Coder)
                .expect_err("not permitted"),
            held.advance(&second, &assign(agent()), Role::Lead)
                .expect_err("scope locked"),
            Lifecycle::new()
                .advance(&ticket(TicketState::Filed), &assign(agent()), Role::Lead)
                .expect_err("no such transition"),
            Lifecycle::new()
                .advance(
                    &decisional,
                    &set_category(agent(), Category::Auto),
                    Role::Lead,
                )
                .expect_err("upgrade only"),
        ];

        for refusal in refusals {
            let reference = refusal.reason().expect("every refusal names its section");
            assert!(
                reference.resolves(),
                "{reference} does not resolve in the methodology index, from: {refusal}"
            );
            assert!(
                !refusal.to_string().is_empty(),
                "a refusal a human cannot read is not a refusal"
            );
        }
    }

    #[test]
    fn ori_t_0049_a_permission_refusal_carries_no_client_api_code() {
        let refusal = Lifecycle::new()
            .advance(&ticket(TicketState::Queued), &assign(agent()), Role::Coder)
            .expect_err("not permitted");

        assert_eq!(
            refusal.code(),
            None,
            "spec/API_SPEC.md names two codes and neither is this one"
        );
        assert!(
            refusal.to_string().contains("Read own; comment"),
            "the sentence shows the cell of AICD §17 the decision came from"
        );
    }
}
