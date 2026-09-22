//! The Ticket state machine as pure functions: AICD §11.
//!
//! `spec/LLD.md` section 2 gives this crate "state machines as pure functions
//! (`Ticket::apply(event) -> Result<Ticket>`)" and forbids it IO and any
//! workspace import. This module is the Ticket half of that row. The transition
//! set is `spec/DATA_MODEL.md` section 3's Ticket diagram, transcribed edge for
//! edge into [`TRANSITIONS`], and the three sentences of the invariants
//! paragraph under that diagram are the three guards in [`Ticket::apply`].
//!
//! # What an event is here, and why it is not `Event`
//!
//! `spec/DATA_MODEL.md` section 2's `Event` entity carries `seq`, `hash_prev`,
//! a `product_id`, an `at` and a JSON `payload`, and `spec/LLD.md` section 2
//! puts it in `ori-store` with the append-only hash-chained log it belongs to.
//! This crate cannot hold that type: it may not import `ori-store`, and it has
//! no business knowing about sequence numbers or hash chains.
//!
//! [`TicketEvent`] is the decoded form: the actor, and the one fact the machine
//! needs to decide. `ori-store` decodes a logged `Event` into one of these and
//! asks this function; nothing here knows the event was logged, and nothing in
//! `ori-store` decides whether it was admissible. That split is why this does
//! not encroach on `ori-store`'s row: a `TicketEvent` cannot be appended to
//! anything and carries no field of the log.
//!
//! The actor sits on [`TicketEvent`] rather than on the variants that happen to
//! need it because `spec/DATA_MODEL.md` section 4 states it as an invariant of
//! every event: "Every `Event` has an actor". A field on the struct is that
//! invariant in the type, where a field on four variants would be a convention.
//!
//! # Why the events are named after the states they enter
//!
//! No two edges of the diagram enter one state for two reasons this machine
//! must tell apart. Three edges enter `InProgress` (a lead assigns, an
//! escalation is answered, a reviewer requests changes) and two enter `Queued`
//! (a validated ticket joins the queue, a blocked one is re-planned); in every
//! case the ticket simply is in that state afterwards, and nothing downstream
//! of this function reads which edge was walked. So one value per reachable
//! state is the whole event vocabulary, and the labels the diagram writes on
//! those edges are recorded in the doc comments rather than in the type. It
//! also makes [`TicketEventKind::target`] a bijection onto the eleven states an
//! event can reach, which is what lets the tests below enumerate the machine
//! rather than sample it.
//!
//! # Order of decision
//!
//! [`Ticket::apply`] answers in three phases, and the order is load-bearing:
//!
//! 1. the entry guards of the destination, which are about the destination and
//!    not about the path taken to it;
//! 2. the transition table, which is about the path;
//! 3. the field rules, which are about the actor and the value it proposes.
//!
//! Criterion ORI-P1-006 in `spec/criteria/phase-1.md` fixes phase 1 before
//! phase 2. It puts a ticket in `Merged`, closes it with no specification
//! update, and requires the refusal to cite AICD §11's closing rule. A `Merged`
//! ticket also fails the table, because the diagram routes it through
//! `Deployed` first, so a machine that consulted the table first would refuse
//! the same call with the right section and the wrong rule. The invariant is
//! worded the same way: a ticket "cannot enter `Closed`" without the update,
//! which is a condition on the destination, not on the edge.
//!
//! The cost is stated rather than hidden. A ticket asked to make a nonsense
//! transition whose destination is also guarded hears about the guard first:
//! `Filed` to `Closed` with nothing recorded is refused for the missing
//! specification update, and the illegal transition goes unmentioned. Both are
//! true, and [`TicketState::may_advance_to`] answers the second question on its
//! own for a caller that wants it separately.
//!
//! # What this module does not decide
//!
//! The diagram writes "lead assigns, scope locked" on the edge into
//! `InProgress`. Neither half is checkable here. [`Actor`] carries an identity
//! and not a role, so "the lead" is the permission function's question
//! (ORI-T-0022), and the lock table that makes "scope locked" true is
//! `LockTable` in `ori-orchestrator` (`spec/LLD.md` section 2). The escalation
//! trigger that the edge into `Escalated` carries is a field of the
//! `Escalation` row in `spec/DATA_MODEL.md` section 2, not of the ticket.
//!
//! Must not: do IO, or import any other workspace crate (`spec/LLD.md` section
//! 2).

use crate::error::Error;
use crate::error::RefusalKind;
use crate::error::Result;
use crate::types::Actor;
use crate::types::Category;
use crate::types::Ticket;
use crate::types::TicketKind;
use crate::types::TicketState;
use crate::types::Tier;

// ---------------------------------------------------------------------------
// The transition table
// ---------------------------------------------------------------------------

/// Every move a ticket may make, as (from, to): AICD §11.
///
/// Derived from AICD §11's "Lifecycle" line, "Filed to Categorized to Validated
/// to Queued to In progress to In review to Merged to Deployed to Closed", and
/// from `spec/DATA_MODEL.md` section 3's Ticket diagram, which draws that line
/// together with the five edges the prose of AICD §11 and AICD §12 describe but
/// the line does not carry. The diagram is the transcription source; the pairs
/// below are in the order it draws them and each carries the label it writes.
///
/// Fourteen pairs out of the 144 that twelve states admit. A pair absent here
/// is refused, including every self-transition: the diagram draws no loop, so a
/// ticket never re-enters the state it is in.
///
/// The table is public because a caller that needs to ask the shape of the
/// lifecycle, such as a queue view offering the moves available from a state,
/// must read this one rather than keep a second copy that can disagree with it.
pub const TRANSITIONS: &[(TicketState, TicketState)] = &[
    // Filed --> Categorized
    (TicketState::Filed, TicketState::Categorized),
    // Categorized --> Validated: auto (Auto, Behavioral) or human (Decisional)
    (TicketState::Categorized, TicketState::Validated),
    // Categorized --> Rejected
    (TicketState::Categorized, TicketState::Rejected),
    // Validated --> Queued
    (TicketState::Validated, TicketState::Queued),
    // Queued --> InProgress: lead assigns, scope locked
    (TicketState::Queued, TicketState::InProgress),
    // InProgress --> Blocked: budget exceeded
    (TicketState::InProgress, TicketState::Blocked),
    // InProgress --> Escalated: trigger
    (TicketState::InProgress, TicketState::Escalated),
    // Blocked --> Queued: re-planned
    (TicketState::Blocked, TicketState::Queued),
    // Escalated --> InProgress: answered
    (TicketState::Escalated, TicketState::InProgress),
    // InProgress --> InReview: PR ready
    (TicketState::InProgress, TicketState::InReview),
    // InReview --> InProgress: changes requested
    (TicketState::InReview, TicketState::InProgress),
    // InReview --> Merged: tier approvals + gates
    (TicketState::InReview, TicketState::Merged),
    // Merged --> Deployed
    (TicketState::Merged, TicketState::Deployed),
    // Deployed --> Closed: spec update recorded + (defect) criteria accepted
    (TicketState::Deployed, TicketState::Closed),
];

impl TicketState {
    /// Where a ticket is before anything has happened to it: AICD §11.
    ///
    /// Derived from the `[*] --> Filed` edge of `spec/DATA_MODEL.md` section
    /// 3's Ticket diagram, which is the only entry the diagram draws. No event
    /// reaches this state, which is why [`TicketEventKind`] has no value for
    /// it: a ticket is filed into it and never returns to it.
    pub const INITIAL: Self = Self::Filed;

    /// Whether [`TRANSITIONS`] connects this state to `to`.
    ///
    /// Derived from AICD §11's "Lifecycle" by way of [`TRANSITIONS`]. This is
    /// the table and nothing else: it knows no actor, no ticket and no
    /// invariant, so a `true` here means only that the diagram draws the edge.
    /// [`Ticket::apply`] is what adds the guards.
    #[must_use]
    pub fn may_advance_to(self, to: Self) -> bool {
        TRANSITIONS
            .iter()
            .any(|(from, into)| *from == self && *into == to)
    }

    /// Whether the lifecycle leaves this state with nowhere to go.
    ///
    /// Derived from `spec/DATA_MODEL.md` section 3's Ticket diagram, which
    /// gives `Rejected` and `Closed` no outgoing edge. It draws `Closed --> [*]`
    /// and draws nothing out of `Rejected`, which is the same fact told twice:
    /// a ticket in either state is done, and the difference is only that one
    /// was worked. Computed from [`TRANSITIONS`] rather than listed, so the two
    /// cannot drift.
    #[must_use]
    pub fn is_final(self) -> bool {
        !TRANSITIONS.iter().any(|(from, _)| *from == self)
    }
}

// ---------------------------------------------------------------------------
// The facts a closing event carries
// ---------------------------------------------------------------------------

/// What the documentation agent recorded for a ticket: AICD §11.
///
/// Derived from AICD §11's closing rule, "'Closed' requires the documentation
/// agent's specification update (or its explicit 'no change needed')", which
/// names two acceptable answers and makes the absence of both the third case.
/// `spec/DATA_MODEL.md` section 3 restates it as an invariant of the Ticket
/// machine: a ticket "cannot enter `Closed` without a `spec_update` event (or
/// `no_change_needed`)".
///
/// This is a report about the event log, not a part of the ticket row. The
/// caller reads the log, which this crate may not, and states what it found.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpecUpdate {
    /// A `spec_update` event is recorded for the ticket.
    Recorded,
    /// The documentation agent recorded its explicit "no change needed".
    NoChangeNeeded,
    /// Neither is recorded.
    Absent,
}

impl SpecUpdate {
    /// Whether this answer satisfies AICD §11's closing rule.
    ///
    /// Both of the section's two answers satisfy it and nothing else does,
    /// which is the whole of the rule as it applies to this half.
    #[must_use]
    pub const fn satisfies_closing_rule(self) -> bool {
        matches!(self, Self::Recorded | Self::NoChangeNeeded)
    }
}

/// Whether an accepted criterion covers a defect: AICD §16.
///
/// Derived from AICD §16's loop, "Signal to Classification to Ticket to Fix to
/// Specification update to New acceptance criteria", and the sentence under it:
/// "a fix without new acceptance criteria produces a bug that can return. Both
/// steps are required for a ticket to close." AICD §11 states the same
/// condition for defects alone, and `spec/DATA_MODEL.md` section 3 makes it an
/// invariant of this machine.
///
/// [`CriterionCoverage::Proposed`] is a value of its own because a proposed
/// criterion is not an accepted one. `spec/DATA_MODEL.md` section 2 gives
/// `Criterion` the states "proposed, accepted, rejected, superseded" and notes
/// that "Proposed criteria never enter the coverage matrix";
/// `spec/TESTING.md` section 2 says the same. Folding it into
/// [`CriterionCoverage::Absent`] would refuse it correctly and explain it
/// wrongly, by telling an author that no criterion references the ticket when
/// one does.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CriterionCoverage {
    /// An accepted criterion references the ticket.
    Accepted,
    /// A criterion references the ticket and has not been accepted yet.
    Proposed,
    /// No criterion references the ticket.
    Absent,
}

impl CriterionCoverage {
    /// Whether this answer satisfies AICD §16's second step.
    ///
    /// Only an accepted criterion does. A proposed one is the QA agent's
    /// proposal and a human has not yet agreed it covers the case.
    #[must_use]
    pub const fn satisfies_closing_rule(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

/// What a pull request did to the tests that already existed: AICD §12.
///
/// Derived from AICD §12's escalation triggers, one of which is "a test had to
/// be modified or removed for the build to pass", and from
/// `spec/DATA_MODEL.md` section 3's third Ticket invariant: "a ticket whose PR
/// modified an existing test has an open `Escalation` before `InReview` can
/// proceed". Criterion ORI-P1-010 in `spec/criteria/phase-1.md` states the
/// consequence this machine owns, "PR cannot reach InReview".
///
/// Three named values rather than two booleans: a pair of booleans next to each
/// other can be passed in the wrong order and still compile, and only three of
/// its four combinations mean anything.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TestModification {
    /// The pull request modified no test that already existed.
    None,
    /// It modified one, and an escalation is open for it.
    EscalationOpen,
    /// It modified one, and no escalation is open.
    EscalationMissing,
}

impl TestModification {
    /// Whether the pull request may enter review.
    ///
    /// Modifying nothing and modifying something under an open escalation both
    /// may. AICD §12 makes the modification a trigger, not a prohibition; what
    /// is refused is proceeding without having asked.
    #[must_use]
    pub const fn may_enter_review(self) -> bool {
        matches!(self, Self::None | Self::EscalationOpen)
    }
}

// ---------------------------------------------------------------------------
// The event
// ---------------------------------------------------------------------------

/// What happened to a ticket, in the form this machine decides on: AICD §11.
///
/// One value per state an event can reach, plus the two changes that move no
/// state. Each variant's doc comment names the edges of
/// `spec/DATA_MODEL.md` section 3's Ticket diagram it stands for and the label
/// the diagram writes on them.
///
/// The names are past tense because this is the fact the log will hold, and the
/// payloads are the fields `spec/DATA_MODEL.md` section 2 marks as set at that
/// point in the life of a ticket.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TicketEventKind {
    /// `Filed --> Categorized`, carrying the category the ticket is filed
    /// under or moved to.
    ///
    /// AICD §11: "The QA agent proposes the category when it files a ticket."
    /// The upgrade-only rule applies to the category named here exactly as it
    /// applies to [`TicketEventKind::CategorySet`], because it is the same lead
    /// agent making the same decision one step earlier.
    Categorized {
        /// The category the ticket carries out of this event.
        category: Category,
    },
    /// `Categorized --> Rejected`. The ticket will not be worked.
    Rejected,
    /// `Categorized --> Validated`, labelled "auto (Auto, Behavioral) or human
    /// (Decisional)".
    ///
    /// AICD §11: "'Validated' means the ticket is authorized to be worked:
    /// automatic for Auto and Behavioral, human-approved for Decisional." The
    /// label is a guard, and [`Ticket::apply`] enforces it against
    /// [`TicketEvent::actor`].
    Validated,
    /// `Validated --> Queued`, and `Blocked --> Queued` labelled "re-planned".
    Queued,
    /// `Queued --> InProgress` labelled "lead assigns, scope locked",
    /// `Escalated --> InProgress` labelled "answered", and
    /// `InReview --> InProgress` labelled "changes requested".
    InProgress,
    /// `InProgress --> Blocked`, labelled "budget exceeded".
    ///
    /// AICD §12: "When any limit is exceeded, the coder stops, writes a blocked
    /// report and hands off."
    Blocked,
    /// `InProgress --> Escalated`, labelled "trigger".
    ///
    /// Which of AICD §12's triggers fired is a field of the `Escalation` row in
    /// `spec/DATA_MODEL.md` section 2 and is not carried here: this machine
    /// treats every trigger alike.
    Escalated,
    /// `InProgress --> InReview`, labelled "PR ready".
    InReview {
        /// What the pull request did to the tests that already existed.
        tests: TestModification,
    },
    /// `InReview --> Merged`, labelled "tier approvals + gates".
    Merged {
        /// Whether the change was a significant modification.
        ///
        /// `spec/DATA_MODEL.md` section 2 marks `Ticket.significance` "set at
        /// merge", which is this event and no other.
        significant: bool,
    },
    /// `Merged --> Deployed`. Released from a tag.
    Deployed,
    /// `Deployed --> Closed`, labelled "spec update recorded + (defect)
    /// criteria accepted".
    ///
    /// The two payloads are that label. They are reports about the event log,
    /// which the caller reads and this crate may not.
    Closed {
        /// What the documentation agent recorded.
        spec_update: SpecUpdate,
        /// Whether an accepted criterion covers the case.
        criterion: CriterionCoverage,
    },
    /// The category changed, with no change of state: AICD §11.
    ///
    /// `spec/API_SPEC.md` section 1 exposes this as `tickets.setCategory`.
    /// Criterion ORI-P1-005 is the rule it carries.
    CategorySet {
        /// The category asked for.
        category: Category,
    },
    /// The risk tier changed, with no change of state: AICD §13.
    ///
    /// `spec/API_SPEC.md` section 1 names no method for this; ruling R11 in
    /// `ops/rulings.md` is what puts the tier on the same upgrade-only ladder
    /// as the category, and `RefusalKind::TierDowngrade` is the refusal already
    /// written for it.
    TierSet {
        /// The tier asked for.
        tier: Tier,
    },
}

impl TicketEventKind {
    /// The state this event puts the ticket in, or `None` when it moves none.
    ///
    /// Derived from `spec/DATA_MODEL.md` section 3's Ticket diagram: every
    /// event names its destination, and the two that name none are the field
    /// changes AICD §11's upgrade-only rule governs. Nothing maps to
    /// [`TicketState::INITIAL`], which is reached by filing and by nothing
    /// else.
    #[must_use]
    pub const fn target(&self) -> Option<TicketState> {
        match self {
            Self::Categorized { .. } => Some(TicketState::Categorized),
            Self::Rejected => Some(TicketState::Rejected),
            Self::Validated => Some(TicketState::Validated),
            Self::Queued => Some(TicketState::Queued),
            Self::InProgress => Some(TicketState::InProgress),
            Self::Blocked => Some(TicketState::Blocked),
            Self::Escalated => Some(TicketState::Escalated),
            Self::InReview { .. } => Some(TicketState::InReview),
            Self::Merged { .. } => Some(TicketState::Merged),
            Self::Deployed => Some(TicketState::Deployed),
            Self::Closed { .. } => Some(TicketState::Closed),
            Self::CategorySet { .. } | Self::TierSet { .. } => None,
        }
    }
}

/// One thing that happened to a ticket, and who did it: AICD §13.
///
/// Derived from `spec/DATA_MODEL.md` section 4's first invariant, "Every
/// `Event` has an actor", and from AICD §13's audit trail, in which every
/// action is traceable to the identity that took it. The actor is not
/// decoration: AICD §11's upgrade-only rule and the human-approval half of the
/// `Categorized` to `Validated` edge are both decided from it.
///
/// The fields are public because the shape is what `ori-store` builds when it
/// decodes a logged `Event`, and [`TicketEvent::new`] exists for the readable
/// spelling rather than as a gate: there is nothing here a constructor could
/// refuse that [`Ticket::apply`] does not decide with the ticket in hand.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TicketEvent {
    /// Who performed it.
    pub actor: Actor,
    /// What happened.
    pub kind: TicketEventKind,
}

impl TicketEvent {
    /// An event by `actor`.
    ///
    /// No methodology section applies beyond the type's own: this is the
    /// constructor, not a rule.
    #[must_use]
    pub const fn new(actor: Actor, kind: TicketEventKind) -> Self {
        Self { actor, kind }
    }

    /// Whether a human performed it.
    ///
    /// Derived from AICD §11's "Only a human can downgrade", which is a
    /// question about the actor and not about the identity behind it.
    /// [`Actor::System`] is not a human: `spec/DATA_MODEL.md` section 4 admits
    /// it "only for scheduled triggers and watchers", neither of which is a
    /// person deciding.
    #[must_use]
    pub const fn by_human(&self) -> bool {
        matches!(self.actor, Actor::Human(_))
    }
}

// ---------------------------------------------------------------------------
// The machine
// ---------------------------------------------------------------------------

/// Where a category sits on AICD §11's ladder, or `None` when it is not on it.
///
/// AICD §11's rule names the ladder in full: "Auto to Behavioral, Behavioral to
/// Decisional". `Category::ProductSignal` is not a rung of it. The same table
/// calls it "Not a defect: an insight about usage or value" that "Never enters
/// the coder queue directly", so it is not a degree of the thing the other
/// three are degrees of, and the doc comment on `Category` in
/// `crates/ori-core/src/types.rs` is why that type refuses to derive `Ord`.
const fn rung(category: Category) -> Option<u8> {
    match category {
        Category::Auto => Some(0),
        Category::Behavioral => Some(1),
        Category::Decisional => Some(2),
        Category::ProductSignal => None,
    }
}

/// Whether moving from `from` to `to` is a downgrade in AICD §11's sense.
///
/// A move involving `Category::ProductSignal` is not one. The rule is about the
/// ladder, and `spec/API_SPEC.md` section 1 restricts exactly what the rule
/// does and no more: `tickets.setCategory` is annotated "downgrade requires
/// human actor". A machine that also refused an agent the off-ladder moves
/// would be inventing a rule and, worse, would have to explain it with a
/// refusal whose sentence reads "and product_signal is below decisional",
/// which is not true of anything.
const fn is_category_downgrade(from: Category, to: Category) -> bool {
    match (rung(from), rung(to)) {
        (Some(held), Some(asked)) => asked < held,
        _ => false,
    }
}

impl Ticket {
    /// Applies one event, returning the ticket that results: AICD §11.
    ///
    /// Derived from `spec/LLD.md` section 2, which gives this crate the state
    /// machines "as pure functions (`Ticket::apply(event) -> Result<Ticket>`)".
    /// The transitions are `spec/DATA_MODEL.md` section 3's Ticket diagram by
    /// way of [`TRANSITIONS`]; the guards are the three sentences of the
    /// invariants paragraph printed under it. The module doc above states the
    /// order the three phases run in and why criterion ORI-P1-006 fixes it.
    ///
    /// # Why it borrows and returns a new ticket
    ///
    /// Taking `self` by value would make the state a caller has just moved out
    /// of unusable, which is the tidier shape for the path where the event is
    /// accepted. It is the wrong shape for this machine, because refusing is
    /// most of what this machine does: each of the three criteria this function
    /// carries names a refusal. On the error path the ticket would be
    /// gone, so every caller that wants to report what it refused would have to
    /// clone before calling, and the clone would then happen on the accepted
    /// path too. Borrowing moves the copy inside, where it happens once and
    /// only when the event is accepted.
    ///
    /// The receiver is left untouched either way, which is what makes this a
    /// pure function and what lets a caller hold the state the log is still at
    /// while it decides what to do with the one it would move to.
    ///
    /// # Errors
    ///
    /// Every refusal is `Error::Refused` and carries the `MethodologyRef` its
    /// `RefusalKind` names, per criterion ORI-P1-033:
    ///
    /// - `RefusalKind::SpecUpdateMissing` when the ticket is asked to enter
    ///   `Closed` with neither a specification update nor an explicit "no
    ///   change needed" recorded (AICD §11, criterion ORI-P1-006).
    /// - `RefusalKind::CriterionMissing` when a `defect` is asked to enter
    ///   `Closed` with no accepted criterion covering it (AICD §16, criterion
    ///   ORI-P1-007).
    /// - `RefusalKind::TestModifiedWithoutEscalation` when a pull request that
    ///   modified an existing test is asked to enter `InReview` with no
    ///   escalation open (AICD §12).
    /// - `RefusalKind::TicketTransition` when the diagram draws no such edge,
    ///   and when a `Decisional` ticket is validated by anything but a human.
    /// - `RefusalKind::CategoryDowngrade` when an actor that is not human lowers
    ///   the category (AICD §11, criterion ORI-P1-005). Its code is
    ///   `E_UPGRADE_ONLY`.
    /// - `RefusalKind::TierDowngrade` when an actor that is not human lowers the
    ///   risk tier (AICD §11 by ruling R11 in `ops/rulings.md`).
    pub fn apply(&self, event: &TicketEvent) -> Result<Self> {
        let Some(to) = event.kind.target() else {
            return self.apply_field_change(event);
        };

        self.check_entry(to, event)?;

        if !self.state.may_advance_to(to) {
            return Err(Error::refused_with(
                RefusalKind::TicketTransition {
                    from: self.state,
                    to,
                },
                format!("ticket {}", self.id),
            ));
        }

        let mut next = self.clone();
        next.state = to;
        match event.kind {
            TicketEventKind::Categorized { category } => {
                next.category = self.checked_category(category, event)?;
            }
            TicketEventKind::Merged { significant } => {
                next.significant = Some(significant);
            }
            _ => {}
        }
        Ok(next)
    }

    /// Applies a sequence of events in order, stopping at the first refusal.
    ///
    /// Derived from `spec/DATA_MODEL.md`'s opening sentence, "The store is
    /// event-sourced: every change is an `Event`; the tables below are
    /// projections rebuilt from the log", together with CLAUDE.md's rule that a
    /// wrong projection is fixed in the projector. This is the fold a projector
    /// runs, held here so that the projector in `ori-store` rebuilds a ticket
    /// through the same guards the live path uses rather than by assigning
    /// states out of the log.
    ///
    /// # Errors
    ///
    /// The first refusal, exactly as [`Ticket::apply`] would return it. Nothing
    /// partial is returned: a sequence that is refused halfway leaves the
    /// receiver as it was and yields no ticket.
    pub fn replay<'a, I>(&self, events: I) -> Result<Self>
    where
        I: IntoIterator<Item = &'a TicketEvent>,
    {
        let mut ticket = self.clone();
        for event in events {
            ticket = ticket.apply(event)?;
        }
        Ok(ticket)
    }

    /// The guards on entering a state, which are the invariants paragraph of
    /// `spec/DATA_MODEL.md` section 3.
    fn check_entry(&self, to: TicketState, event: &TicketEvent) -> Result<()> {
        match (to, &event.kind) {
            (TicketState::Validated, _) => self.check_validation(event),
            (TicketState::InReview, TicketEventKind::InReview { tests }) => {
                Self::check_tests(*tests)
            }
            (
                TicketState::Closed,
                TicketEventKind::Closed {
                    spec_update,
                    criterion,
                },
            ) => self.check_closing(*spec_update, *criterion),
            _ => Ok(()),
        }
    }

    /// AICD §11: "automatic for Auto and Behavioral, human-approved for
    /// Decisional", which `spec/DATA_MODEL.md` section 3 writes on the edge as
    /// "auto (Auto, Behavioral) or human (Decisional)".
    ///
    /// The refusal is `RefusalKind::TicketTransition`, which is the same
    /// refusal the table would make and cites the same section. What this adds
    /// is the detail, because the pair alone does not explain a refusal that
    /// depends on who asked.
    fn check_validation(&self, event: &TicketEvent) -> Result<()> {
        if self.category == Category::Decisional && !event.by_human() {
            return Err(Error::refused_with(
                RefusalKind::TicketTransition {
                    from: self.state,
                    to: TicketState::Validated,
                },
                format!(
                    "a decisional ticket is validated by a human and {} is not one",
                    event.actor
                ),
            ));
        }
        Ok(())
    }

    /// `spec/DATA_MODEL.md` section 3: "a ticket whose PR modified an existing
    /// test has an open `Escalation` before `InReview` can proceed".
    fn check_tests(tests: TestModification) -> Result<()> {
        if tests.may_enter_review() {
            return Ok(());
        }
        Err(Error::refused_with(
            RefusalKind::TestModifiedWithoutEscalation,
            "open an escalation with trigger test_modified before the pull request is reviewed",
        ))
    }

    /// `spec/DATA_MODEL.md` section 3: a ticket "cannot enter `Closed` without a
    /// `spec_update` event (or `no_change_needed`) and, if `kind` is `defect`,
    /// an accepted criterion referencing it".
    ///
    /// The two halves are asked in that order because criterion ORI-P1-007
    /// describes a ticket that has the first and lacks the second, so the
    /// second can only be the answer once the first is satisfied.
    fn check_closing(&self, spec_update: SpecUpdate, criterion: CriterionCoverage) -> Result<()> {
        if !spec_update.satisfies_closing_rule() {
            return Err(Error::refused_with(
                RefusalKind::SpecUpdateMissing,
                format!(
                    "ticket {} is {} and nothing is recorded",
                    self.id, self.state
                ),
            ));
        }
        if self.kind == TicketKind::Defect && !criterion.satisfies_closing_rule() {
            let detail = match criterion {
                CriterionCoverage::Proposed => {
                    "a criterion references it and is still proposed; a proposed criterion is not \
                     in the coverage matrix (spec/TESTING.md section 2)"
                }
                _ => "no criterion references it",
            };
            return Err(Error::refused_with(RefusalKind::CriterionMissing, detail));
        }
        Ok(())
    }

    /// The two events that change a field and no state.
    fn apply_field_change(&self, event: &TicketEvent) -> Result<Self> {
        let mut next = self.clone();
        match event.kind {
            TicketEventKind::CategorySet { category } => {
                next.category = self.checked_category(category, event)?;
            }
            TicketEventKind::TierSet { tier } => {
                next.tier = self.checked_tier(tier, event)?;
            }
            // Every other value names a target state and was handled before
            // this function was reached.
            _ => {}
        }
        Ok(next)
    }

    /// AICD §11, "Rule: upgrade only": the lead agent "may upgrade a category
    /// (Auto to Behavioral, Behavioral to Decisional) but never downgrade it.
    /// Only a human can downgrade."
    fn checked_category(&self, to: Category, event: &TicketEvent) -> Result<Category> {
        if is_category_downgrade(self.category, to) && !event.by_human() {
            return Err(Error::refused_with(
                RefusalKind::CategoryDowngrade {
                    from: self.category,
                    to,
                },
                format!("{} may raise it and only a human may lower it", event.actor),
            ));
        }
        Ok(to)
    }

    /// AICD §11's upgrade-only rule applied to the risk tier by ruling R11 in
    /// `ops/rulings.md`: "only a human may lower it".
    fn checked_tier(&self, to: Tier, event: &TicketEvent) -> Result<Tier> {
        if to < self.tier && !event.by_human() {
            return Err(Error::refused_with(
                RefusalKind::TierDowngrade {
                    from: self.tier,
                    to,
                },
                format!("{} may raise it and only a human may lower it", event.actor),
            ));
        }
        Ok(to)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::types::Budget;
    use crate::types::Id;
    use crate::types::Scope;
    use crate::types::SpecAnchor;

    // ---------------------------------------------------------------------
    // The diagram, transcribed a second time and independently
    // ---------------------------------------------------------------------

    /// `spec/DATA_MODEL.md` section 3's Ticket diagram, read off the document
    /// again rather than off `TRANSITIONS`.
    ///
    /// This is the point of the enumeration below and the reason it is not
    /// circular. A test that asked `TRANSITIONS` what the transitions are would
    /// pass whatever `TRANSITIONS` said, including nothing and everything.
    /// These fourteen lines are the diagram's own, in its order, each written
    /// next to the line it came from.
    const DIAGRAM: &[(TicketState, TicketState)] = &[
        // Filed --> Categorized
        (TicketState::Filed, TicketState::Categorized),
        // Categorized --> Validated: auto (Auto, Behavioral) or human (Decisional)
        (TicketState::Categorized, TicketState::Validated),
        // Categorized --> Rejected
        (TicketState::Categorized, TicketState::Rejected),
        // Validated --> Queued
        (TicketState::Validated, TicketState::Queued),
        // Queued --> InProgress: lead assigns, scope locked
        (TicketState::Queued, TicketState::InProgress),
        // InProgress --> Blocked: budget exceeded
        (TicketState::InProgress, TicketState::Blocked),
        // InProgress --> Escalated: trigger
        (TicketState::InProgress, TicketState::Escalated),
        // Blocked --> Queued: re-planned
        (TicketState::Blocked, TicketState::Queued),
        // Escalated --> InProgress: answered
        (TicketState::Escalated, TicketState::InProgress),
        // InProgress --> InReview: PR ready
        (TicketState::InProgress, TicketState::InReview),
        // InReview --> InProgress: changes requested
        (TicketState::InReview, TicketState::InProgress),
        // InReview --> Merged: tier approvals + gates
        (TicketState::InReview, TicketState::Merged),
        // Merged --> Deployed
        (TicketState::Merged, TicketState::Deployed),
        // Deployed --> Closed: spec update recorded + (defect) criteria accepted
        (TicketState::Deployed, TicketState::Closed),
    ];

    /// How many edges the diagram draws between two named states.
    ///
    /// Written as a number so that a transcription that lost or gained a line
    /// fails here, where the count is checked against the list, before it can
    /// agree with a production table that lost or gained the same line.
    const DIAGRAM_EDGES: usize = 14;

    /// How many ordered pairs twelve states admit.
    const ALL_PAIRS: usize = 144;

    fn id(text: &str) -> Id {
        Id::parse(text).expect("a canonical ULID")
    }

    fn human() -> Actor {
        Actor::Human(id("01ARZ3NDEKTSV4RRFFQ69G5FAV"))
    }

    fn agent() -> Actor {
        Actor::Agent(id("01BX5ZZKBKACTAV9WEVGEMMVRZ"))
    }

    /// A ticket in `state`, with everything the machine reads named and
    /// everything else filled with a value the machine never looks at.
    fn ticket(state: TicketState, kind: TicketKind, category: Category) -> Ticket {
        Ticket {
            id: id("01D78XYFJ1PRM1WPBCBT3VHMNV"),
            product_id: id("01F8MECHZX3TBDSZ7XR8H8JHAF"),
            title: "The Ticket state machine".to_owned(),
            category,
            kind,
            tier: Tier::Two,
            state,
            spec_anchor: SpecAnchor::parse("DATA_MODEL.md#3-state-machines").expect("an anchor"),
            declared_scope: Scope::new(["crates/ori-core/src/ticket.rs"]).expect("a scope"),
            budget: Budget {
                attempts: 3,
                wall_clock_s: 2700,
                tokens: 0,
            },
            filed_by: agent(),
            phase_id: id("01G65Z755AFWAKHE12NY0CQ9FH"),
            significant: None,
            incident_id: None,
        }
    }

    /// A plain feature ticket in `state`.
    fn feature(state: TicketState) -> Ticket {
        ticket(state, TicketKind::Feature, Category::Auto)
    }

    /// The one event that reaches `state`, with every guard on it satisfied.
    ///
    /// Used by the enumeration, where the question is the transition and not
    /// the guards; the guards have their own tests.
    fn event_reaching(state: TicketState) -> TicketEvent {
        let kind = match state {
            TicketState::Filed => panic!("no event reaches the initial state"),
            TicketState::Categorized => TicketEventKind::Categorized {
                category: Category::Auto,
            },
            TicketState::Rejected => TicketEventKind::Rejected,
            TicketState::Validated => TicketEventKind::Validated,
            TicketState::Queued => TicketEventKind::Queued,
            TicketState::InProgress => TicketEventKind::InProgress,
            TicketState::Blocked => TicketEventKind::Blocked,
            TicketState::Escalated => TicketEventKind::Escalated,
            TicketState::InReview => TicketEventKind::InReview {
                tests: TestModification::None,
            },
            TicketState::Merged => TicketEventKind::Merged { significant: false },
            TicketState::Deployed => TicketEventKind::Deployed,
            TicketState::Closed => TicketEventKind::Closed {
                spec_update: SpecUpdate::Recorded,
                criterion: CriterionCoverage::Accepted,
            },
        };
        TicketEvent::new(human(), kind)
    }

    fn refusal(error: &Error) -> RefusalKind {
        match error {
            Error::Refused { kind, .. } => kind.clone(),
            Error::Malformed { .. } => panic!("expected a refusal, got {error}"),
        }
    }

    // ---------------------------------------------------------------------
    // The table, enumerated over all 144 pairs
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0020_the_transition_table_is_the_diagram_and_nothing_else() {
        let table: HashSet<_> = TRANSITIONS.iter().copied().collect();
        let diagram: HashSet<_> = DIAGRAM.iter().copied().collect();

        assert_eq!(
            diagram.len(),
            DIAGRAM_EDGES,
            "the transcription of the diagram in this test lost or gained a line"
        );
        assert_eq!(
            table.len(),
            TRANSITIONS.len(),
            "TRANSITIONS lists an edge twice"
        );

        // Walked in the order each list is written, so that a failure reads in
        // the order of the diagram rather than in a hash order.
        let missing: Vec<_> = DIAGRAM
            .iter()
            .filter(|edge| !table.contains(*edge))
            .collect();
        let extra: Vec<_> = TRANSITIONS
            .iter()
            .filter(|edge| !diagram.contains(*edge))
            .collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "TRANSITIONS and the diagram disagree. In the diagram and refused by the table: \
             {missing:?}. Permitted by the table and not in the diagram: {extra:?}"
        );
    }

    #[test]
    fn ori_t_0020_every_one_of_the_144_state_pairs_is_answered_by_the_table() {
        let diagram: HashSet<_> = DIAGRAM.iter().copied().collect();
        let mut pairs = 0_usize;
        let mut permitted = 0_usize;
        let mut wrong = Vec::new();

        for from in TicketState::ALL.iter().copied() {
            for to in TicketState::ALL.iter().copied() {
                pairs += 1;
                let expected = diagram.contains(&(from, to));
                if expected {
                    permitted += 1;
                }
                if from.may_advance_to(to) != expected {
                    wrong.push((from, to, expected));
                }
            }
        }

        assert_eq!(pairs, ALL_PAIRS, "twelve states make 144 ordered pairs");
        assert_eq!(permitted, DIAGRAM_EDGES, "the diagram draws fourteen edges");
        assert_eq!(
            ALL_PAIRS - permitted,
            130,
            "the other 130 pairs are refused"
        );
        assert!(
            wrong.is_empty(),
            "the table answers {} of the 144 pairs against the diagram; each is \
             (from, to, what the diagram says): {wrong:?}",
            wrong.len()
        );
    }

    #[test]
    fn ori_t_0020_no_pair_of_the_table_is_a_self_transition() {
        for (from, to) in TRANSITIONS {
            assert_ne!(
                from, to,
                "the diagram draws no loop, so a ticket never re-enters {from}"
            );
        }
    }

    #[test]
    fn ori_t_0020_the_states_with_no_way_out_are_rejected_and_closed() {
        let finals: Vec<_> = TicketState::ALL
            .iter()
            .copied()
            .filter(|state| state.is_final())
            .collect();
        assert_eq!(finals, vec![TicketState::Rejected, TicketState::Closed]);
        assert_eq!(TicketState::INITIAL, TicketState::Filed);
    }

    // ---------------------------------------------------------------------
    // The function, enumerated over every state and every event
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0020_every_state_but_the_initial_one_is_reached_by_exactly_one_event() {
        let mut reached = HashSet::new();
        for state in TicketState::ALL.iter().copied() {
            if state == TicketState::INITIAL {
                continue;
            }
            let event = event_reaching(state);
            assert_eq!(
                event.kind.target(),
                Some(state),
                "the event this test pairs with {state} does not name it"
            );
            assert!(reached.insert(state), "{state} is named by two events");
        }
        assert_eq!(reached.len(), TicketState::ALL.len() - 1);

        for kind in [
            TicketEventKind::CategorySet {
                category: Category::Auto,
            },
            TicketEventKind::TierSet { tier: Tier::Zero },
        ] {
            assert_eq!(kind.target(), None, "{kind:?} moves no state");
        }
    }

    #[test]
    fn ori_t_0020_no_event_reaches_the_initial_state() {
        // The remaining twelve of the 144 pairs: nothing an event can express
        // enters Filed, from any state, including Filed itself.
        for state in TicketState::ALL.iter().copied() {
            if state == TicketState::INITIAL {
                continue;
            }
            assert_ne!(
                event_reaching(state).kind.target(),
                Some(TicketState::INITIAL)
            );
        }
        for (_, to) in TRANSITIONS {
            assert_ne!(
                *to,
                TicketState::INITIAL,
                "the diagram draws no edge into it"
            );
        }
    }

    #[test]
    fn ori_t_0020_apply_accepts_exactly_the_pairs_the_diagram_draws() {
        let diagram: HashSet<_> = DIAGRAM.iter().copied().collect();
        let mut checked = 0_usize;
        let mut accepted = 0_usize;
        let mut wrong = Vec::new();

        for from in TicketState::ALL.iter().copied() {
            for to in TicketState::ALL.iter().copied() {
                if to == TicketState::INITIAL {
                    continue;
                }
                checked += 1;
                let expected = diagram.contains(&(from, to));
                let outcome = feature(from).apply(&event_reaching(to));
                match (&outcome, expected) {
                    (Ok(next), true) => {
                        accepted += 1;
                        // Every field the event does not carry is the field the
                        // ticket already held. The canonical events name the
                        // category this ticket already has, so the only field
                        // any of them writes is the significance label.
                        let mut expected = feature(from);
                        expected.state = to;
                        if to == TicketState::Merged {
                            expected.significant = Some(false);
                        }
                        assert_eq!(
                            *next, expected,
                            "{from} to {to} landed elsewhere or rewrote a field"
                        );
                    }
                    (Err(error), false) => {
                        assert_eq!(
                            refusal(error),
                            RefusalKind::TicketTransition { from, to },
                            "{from} to {to} was refused, and not as a transition"
                        );
                    }
                    _ => wrong.push((from, to, expected)),
                }
            }
        }

        assert_eq!(checked, 132, "twelve states by the eleven an event reaches");
        assert_eq!(accepted, DIAGRAM_EDGES);
        assert!(
            wrong.is_empty(),
            "apply answers {} pairs against the diagram; each is \
             (from, to, what the diagram says): {wrong:?}",
            wrong.len()
        );
    }

    #[test]
    fn ori_t_0020_every_refusal_this_machine_makes_carries_a_reason_that_resolves() {
        let refusals = [
            feature(TicketState::Filed).apply(&event_reaching(TicketState::Closed)),
            feature(TicketState::Deployed).apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Absent,
                    criterion: CriterionCoverage::Accepted,
                },
            )),
            ticket(TicketState::Deployed, TicketKind::Defect, Category::Auto).apply(
                &TicketEvent::new(
                    human(),
                    TicketEventKind::Closed {
                        spec_update: SpecUpdate::Recorded,
                        criterion: CriterionCoverage::Absent,
                    },
                ),
            ),
            feature(TicketState::InProgress).apply(&TicketEvent::new(
                agent(),
                TicketEventKind::InReview {
                    tests: TestModification::EscalationMissing,
                },
            )),
            ticket(
                TicketState::Categorized,
                TicketKind::Feature,
                Category::Decisional,
            )
            .apply(&TicketEvent::new(agent(), TicketEventKind::Validated)),
            ticket(
                TicketState::Queued,
                TicketKind::Feature,
                Category::Decisional,
            )
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::CategorySet {
                    category: Category::Auto,
                },
            )),
            feature(TicketState::Queued).apply(&TicketEvent::new(
                agent(),
                TicketEventKind::TierSet { tier: Tier::Zero },
            )),
        ];

        for outcome in &refusals {
            let error = outcome.as_ref().expect_err("a refusal");
            assert!(error.is_refusal(), "{error} is not a refusal");
            let reference = error.methodology_ref().expect("a methodology reference");
            assert!(reference.resolves(), "{reference} resolves against nothing");
        }
    }

    #[test]
    fn ori_t_0020_applying_an_event_leaves_the_ticket_it_was_applied_to_alone() {
        let before = feature(TicketState::Filed);
        let after = before
            .apply(&event_reaching(TicketState::Categorized))
            .expect("filed to categorized");

        assert_eq!(before.state, TicketState::Filed);
        assert_eq!(after.state, TicketState::Categorized);
        assert_eq!(before, feature(TicketState::Filed));

        let refused = before.apply(&event_reaching(TicketState::Merged));
        assert!(refused.is_err());
        assert_eq!(before, feature(TicketState::Filed));
    }

    #[test]
    fn ori_t_0020_a_replay_walks_the_lifecycle_and_stops_at_the_first_refusal() {
        let events = vec![
            TicketEvent::new(
                agent(),
                TicketEventKind::Categorized {
                    category: Category::Behavioral,
                },
            ),
            TicketEvent::new(agent(), TicketEventKind::Validated),
            TicketEvent::new(agent(), TicketEventKind::Queued),
            TicketEvent::new(agent(), TicketEventKind::InProgress),
            TicketEvent::new(
                agent(),
                TicketEventKind::InReview {
                    tests: TestModification::None,
                },
            ),
            TicketEvent::new(human(), TicketEventKind::Merged { significant: true }),
            TicketEvent::new(agent(), TicketEventKind::Deployed),
            TicketEvent::new(
                agent(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Recorded,
                    criterion: CriterionCoverage::Accepted,
                },
            ),
        ];

        let closed = feature(TicketState::Filed)
            .replay(&events)
            .expect("the whole lifecycle");
        assert_eq!(closed.state, TicketState::Closed);
        assert_eq!(closed.category, Category::Behavioral);
        assert_eq!(closed.significant, Some(true));

        let cut_short = feature(TicketState::Filed)
            .replay(&events[..3])
            .expect("part");
        assert_eq!(cut_short.state, TicketState::Queued);

        let skipped = feature(TicketState::Filed).replay(&events[1..]);
        assert_eq!(
            refusal(skipped.as_ref().expect_err("a refusal")),
            RefusalKind::TicketTransition {
                from: TicketState::Filed,
                to: TicketState::Validated,
            }
        );
    }

    // ---------------------------------------------------------------------
    // ORI-P1-005: the category ladder
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_005_an_agent_raises_the_category_of_a_ticket_it_filed_as_auto() {
        let filed = ticket(TicketState::Queued, TicketKind::Defect, Category::Auto);
        assert!(matches!(filed.filed_by, Actor::Agent(_)));

        let raised = filed
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::CategorySet {
                    category: Category::Decisional,
                },
            ))
            .expect("an agent may upgrade");

        assert_eq!(raised.category, Category::Decisional);
        assert_eq!(raised.state, TicketState::Queued, "no state moved");
    }

    #[test]
    fn ori_p1_005_an_agent_lowering_the_category_back_receives_e_upgrade_only() {
        let raised = ticket(
            TicketState::Queued,
            TicketKind::Defect,
            Category::Decisional,
        );

        let error = raised
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::CategorySet {
                    category: Category::Auto,
                },
            ))
            .expect_err("an agent may not downgrade");

        assert_eq!(
            refusal(&error),
            RefusalKind::CategoryDowngrade {
                from: Category::Decisional,
                to: Category::Auto,
            }
        );
        assert_eq!(error.code(), Some("E_UPGRADE_ONLY"));
        assert_eq!(
            error.methodology_ref().expect("a reason").section,
            11,
            "AICD §11 carries the upgrade-only rule"
        );
    }

    #[test]
    fn ori_p1_005_a_human_lowering_the_category_back_is_allowed() {
        let raised = ticket(
            TicketState::Queued,
            TicketKind::Defect,
            Category::Decisional,
        );

        let lowered = raised
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::CategorySet {
                    category: Category::Auto,
                },
            ))
            .expect("only a human can downgrade, and this is one");

        assert_eq!(lowered.category, Category::Auto);
    }

    #[test]
    fn ori_p1_005_the_system_actor_is_not_a_human_and_may_not_lower_a_category() {
        let raised = ticket(
            TicketState::Queued,
            TicketKind::Defect,
            Category::Decisional,
        );
        let error = raised
            .apply(&TicketEvent::new(
                Actor::System,
                TicketEventKind::CategorySet {
                    category: Category::Behavioral,
                },
            ))
            .expect_err("the system is not a person deciding");
        assert_eq!(error.code(), Some("E_UPGRADE_ONLY"));
    }

    #[test]
    fn ori_p1_005_the_ladder_is_the_three_categories_the_rule_names() {
        // Every ordered pair of the four categories, answered by the rule.
        for from in Category::ALL.iter().copied() {
            for to in Category::ALL.iter().copied() {
                let held = ticket(TicketState::Queued, TicketKind::Chore, from);
                let by_agent = held.apply(&TicketEvent::new(
                    agent(),
                    TicketEventKind::CategorySet { category: to },
                ));
                let expected_refusal = matches!(
                    (from, to),
                    (Category::Behavioral | Category::Decisional, Category::Auto)
                        | (Category::Decisional, Category::Behavioral)
                );
                assert_eq!(
                    by_agent.is_err(),
                    expected_refusal,
                    "an agent moving {from} to {to}"
                );
                assert!(
                    held.apply(&TicketEvent::new(
                        human(),
                        TicketEventKind::CategorySet { category: to },
                    ))
                    .is_ok(),
                    "a human moving {from} to {to}"
                );
            }
        }
    }

    #[test]
    fn ori_p1_005_the_upgrade_only_rule_applies_when_the_category_is_first_set() {
        let filed = ticket(TicketState::Filed, TicketKind::Defect, Category::Decisional);
        let error = filed
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::Categorized {
                    category: Category::Auto,
                },
            ))
            .expect_err("categorizing is the same decision one step earlier");
        assert_eq!(error.code(), Some("E_UPGRADE_ONLY"));

        let raised = filed
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::Categorized {
                    category: Category::Decisional,
                },
            ))
            .expect("holding it where it is");
        assert_eq!(raised.state, TicketState::Categorized);
    }

    #[test]
    fn ori_t_0020_an_agent_may_raise_a_tier_and_only_a_human_may_lower_one() {
        let held = ticket(TicketState::Queued, TicketKind::Chore, Category::Auto);
        assert_eq!(held.tier, Tier::Two);

        let error = held
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::TierSet { tier: Tier::One },
            ))
            .expect_err("ruling R11 puts the tier on the same ladder");
        assert_eq!(
            refusal(&error),
            RefusalKind::TierDowngrade {
                from: Tier::Two,
                to: Tier::One,
            }
        );
        assert_eq!(error.methodology_ref().expect("a reason").section, 11);

        assert!(
            held.apply(&TicketEvent::new(
                human(),
                TicketEventKind::TierSet { tier: Tier::One },
            ))
            .is_ok()
        );
        let mut lower = held.clone();
        lower.tier = Tier::Zero;
        assert!(
            lower
                .apply(&TicketEvent::new(
                    agent(),
                    TicketEventKind::TierSet { tier: Tier::Two },
                ))
                .is_ok(),
            "raising is always allowed"
        );
    }

    // ---------------------------------------------------------------------
    // ORI-P1-006: closing without a specification update
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_006_a_merged_ticket_with_no_spec_update_event_is_refused_the_close() {
        let merged = feature(TicketState::Merged);

        let error = merged
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Absent,
                    criterion: CriterionCoverage::Accepted,
                },
            ))
            .expect_err("nothing is recorded");

        assert_eq!(refusal(&error), RefusalKind::SpecUpdateMissing);
        assert_eq!(
            error.methodology_ref().expect("a reason").section,
            11,
            "AICD §11 carries the closing rule"
        );
        assert!(
            error.to_string().contains("AICD §11"),
            "the reason is on the sentence a human reads: {error}"
        );
    }

    #[test]
    fn ori_p1_006_the_closing_rule_is_answered_before_the_route_the_ticket_took() {
        // Both facts are true of a Merged ticket asked to close, and the
        // criterion fixes which one is reported. The diagram routes a ticket
        // through Deployed, so the pair is refused as well.
        assert!(!TicketState::Merged.may_advance_to(TicketState::Closed));
        assert!(TicketState::Deployed.may_advance_to(TicketState::Closed));

        let complete = TicketEvent::new(
            human(),
            TicketEventKind::Closed {
                spec_update: SpecUpdate::Recorded,
                criterion: CriterionCoverage::Accepted,
            },
        );
        let error = feature(TicketState::Merged)
            .apply(&complete)
            .expect_err("merged is not deployed");
        assert_eq!(
            refusal(&error),
            RefusalKind::TicketTransition {
                from: TicketState::Merged,
                to: TicketState::Closed,
            },
            "with the closing rule satisfied, what is left is the route"
        );
    }

    #[test]
    fn ori_p1_006_an_explicit_no_change_needed_satisfies_the_closing_rule() {
        let closed = feature(TicketState::Deployed)
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::NoChangeNeeded,
                    criterion: CriterionCoverage::Absent,
                },
            ))
            .expect("the documentation agent said none was needed");
        assert_eq!(closed.state, TicketState::Closed);
    }

    #[test]
    fn ori_p1_006_a_deployed_ticket_with_nothing_recorded_is_refused_the_same_way() {
        let error = feature(TicketState::Deployed)
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Absent,
                    criterion: CriterionCoverage::Accepted,
                },
            ))
            .expect_err("the rule is about entering Closed, from anywhere");
        assert_eq!(refusal(&error), RefusalKind::SpecUpdateMissing);
    }

    // ---------------------------------------------------------------------
    // ORI-P1-007: closing a defect with no accepted criterion
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_007_a_merged_defect_with_a_spec_update_and_no_criterion_is_refused_the_close() {
        let defect = ticket(TicketState::Merged, TicketKind::Defect, Category::Auto);

        let error = defect
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Recorded,
                    criterion: CriterionCoverage::Absent,
                },
            ))
            .expect_err("a fix without new acceptance criteria");

        assert_eq!(refusal(&error), RefusalKind::CriterionMissing);
        assert_eq!(
            error.methodology_ref().expect("a reason").section,
            16,
            "AICD §16 is where both steps are required for a ticket to close"
        );
        assert!(error.to_string().contains("AICD §16"), "{error}");
    }

    #[test]
    fn ori_p1_007_a_proposed_criterion_does_not_close_a_defect() {
        let defect = ticket(TicketState::Deployed, TicketKind::Defect, Category::Auto);

        let error = defect
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Recorded,
                    criterion: CriterionCoverage::Proposed,
                },
            ))
            .expect_err("proposed criteria do not count until accepted");

        assert_eq!(refusal(&error), RefusalKind::CriterionMissing);
        assert!(
            error.to_string().contains("proposed"),
            "the detail tells the author which of the two cases this is: {error}"
        );
    }

    #[test]
    fn ori_p1_007_the_criterion_is_required_of_defects_and_of_nothing_else() {
        for kind in [TicketKind::Defect, TicketKind::Feature, TicketKind::Chore] {
            for criterion in [
                CriterionCoverage::Accepted,
                CriterionCoverage::Proposed,
                CriterionCoverage::Absent,
            ] {
                let outcome =
                    ticket(TicketState::Deployed, kind, Category::Auto).apply(&TicketEvent::new(
                        human(),
                        TicketEventKind::Closed {
                            spec_update: SpecUpdate::Recorded,
                            criterion,
                        },
                    ));
                let expected_ok =
                    kind != TicketKind::Defect || criterion == CriterionCoverage::Accepted;
                assert_eq!(
                    outcome.is_ok(),
                    expected_ok,
                    "{kind} closing on {criterion:?}"
                );
            }
        }
    }

    #[test]
    fn ori_p1_007_a_defect_closes_when_both_steps_of_the_loop_are_recorded() {
        let closed = ticket(TicketState::Deployed, TicketKind::Defect, Category::Auto)
            .apply(&TicketEvent::new(
                human(),
                TicketEventKind::Closed {
                    spec_update: SpecUpdate::Recorded,
                    criterion: CriterionCoverage::Accepted,
                },
            ))
            .expect("both steps");
        assert_eq!(closed.state, TicketState::Closed);
        assert!(closed.state.is_final());
    }

    // ---------------------------------------------------------------------
    // The third invariant, and the guard on validation
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0020_a_pull_request_that_modified_a_test_reaches_review_only_under_an_escalation() {
        let in_progress = feature(TicketState::InProgress);

        let error = in_progress
            .apply(&TicketEvent::new(
                agent(),
                TicketEventKind::InReview {
                    tests: TestModification::EscalationMissing,
                },
            ))
            .expect_err("an existing test was modified and nothing was asked");
        assert_eq!(refusal(&error), RefusalKind::TestModifiedWithoutEscalation);
        assert_eq!(error.methodology_ref().expect("a reason").section, 12);

        for tests in [TestModification::None, TestModification::EscalationOpen] {
            let reviewed = in_progress
                .apply(&TicketEvent::new(
                    agent(),
                    TicketEventKind::InReview { tests },
                ))
                .expect("review may proceed");
            assert_eq!(reviewed.state, TicketState::InReview);
        }
    }

    #[test]
    fn ori_t_0020_a_decisional_ticket_is_validated_by_a_human_and_the_others_automatically() {
        for category in Category::ALL.iter().copied() {
            let categorized = ticket(TicketState::Categorized, TicketKind::Feature, category);

            let by_agent =
                categorized.apply(&TicketEvent::new(agent(), TicketEventKind::Validated));
            if category == Category::Decisional {
                let error = by_agent.expect_err("a human approves a decisional ticket");
                assert_eq!(
                    refusal(&error),
                    RefusalKind::TicketTransition {
                        from: TicketState::Categorized,
                        to: TicketState::Validated,
                    }
                );
                assert!(error.to_string().contains("decisional"), "{error}");
            } else {
                assert_eq!(
                    by_agent.expect("automatic").state,
                    TicketState::Validated,
                    "{category} is validated without a human"
                );
            }

            assert_eq!(
                categorized
                    .apply(&TicketEvent::new(human(), TicketEventKind::Validated))
                    .expect("a human may always validate")
                    .state,
                TicketState::Validated
            );
        }
    }

    #[test]
    fn ori_t_0020_the_merge_event_is_what_sets_the_significance_label() {
        let in_review = feature(TicketState::InReview);
        assert_eq!(in_review.significant, None, "unset until merge");

        for significant in [true, false] {
            let merged = in_review
                .apply(&TicketEvent::new(
                    human(),
                    TicketEventKind::Merged { significant },
                ))
                .expect("tier approvals and gates");
            assert_eq!(merged.significant, Some(significant));
        }
    }
}
