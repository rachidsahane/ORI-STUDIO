//! Projections: derived SQLite state folded from the event log, and the
//! registry that makes an empty projection set unrepresentable as a pass.
//!
//! `spec/DATA_MODEL.md` section 1: "The store is event-sourced: every change
//! is an `Event`; the tables below are projections rebuilt from the log."
//! Section 4's last invariant is this ticket's criterion, ORI-P1-028's second
//! clause: "The SQLite file plus the repository fully reconstruct every
//! projection; a `rebuild` command proves it in CI." This module is the first
//! half, "every projection"; `crate::rebuild` is the `rebuild` command.
//!
//! CLAUDE.md's load-bearing facts: "No code path updates or deletes an event.
//! Projections are derived; if a projection looks wrong, the fix is in the
//! projector, never in the log." Every [`Projection`] here reads the log
//! forward, never writes to `events`, and never once asks whether a
//! transition it is folding was itself valid: `spec/LLD.md` section 2 puts
//! "ori-store | ... | Contain business rules" in this crate's must-not
//! column. The state machines that decide whether a ticket may move, a module
//! may be claimed, or an escalation may fire already exist and already
//! refuse: `ori_core::ticket::Ticket::apply` in `crates/ori-core`,
//! `crates/ori-orchestrator/src/lock_table.rs`'s `LockTable::claim`, and
//! `crates/ori-orchestrator/src/escalation.rs`'s `Escalation::raise`. By the
//! time an event reaches this crate's log, whatever wrote it has already
//! asked one of those and been told yes. A projector here only ever folds
//! what is already true into a row; it never re-decides it.
//!
//! # The three projectors, and why these three
//!
//! `Ticket`, `LockEntry` and `Escalation`, in [`crate::projections::ticket`],
//! [`crate::projections::lock`] and [`crate::projections::escalation`].
//! CLAUDE.md names these three as the natural choice: `crates/ori-orchestrator/`
//! already implements the ticket lifecycle, the lock table and escalations as
//! pure functions, and `spec/DATA_MODEL.md` section 4 states an invariant a
//! lock projection can at least carry the shape of, "`LockEntry` modules for
//! two `InProgress` tickets never overlap" (see
//! [`crate::projections::lock`]'s module doc for exactly how much of that
//! invariant this projection carries and how much stays where it is already
//! enforced, in `LockTable::claim`). No other entity in `spec/DATA_MODEL.md`
//! section 2 has a driving crate on this branch: `Gate` and `GateProof` are
//! `ori-gates`, which this ticket's scope does not touch; `Document`, `Phase`,
//! `AgentSession`, `MemoryRecord` and the rest have no state machine anywhere
//! in the workspace yet for a projection to be honest about deriving from.
//!
//! # What a payload convention this ticket invents means for "identically"
//!
//! Nothing in the workspace calls [`crate::event_log::EventLog::append`] with
//! a `ticket.*`, `lock.*` or `escalation.*` event yet: `ori_core::ticket`,
//! `ori_orchestrator::lock_table` and `ori_orchestrator::escalation` are pure
//! functions with no writer wired to this crate's log. So the event kind
//! vocabulary and the flat-field payload shape
//! `crate::projections::payload` reads are this ticket's own design, not a
//! contract already relied on elsewhere; the pull request report says this
//! plainly under "Whether this payload convention is load-bearing elsewhere"
//! and names the follow-up (wiring the real writer to emit it) as a finding,
//! not silently assumed. It does not weaken the identity proof:
//! [`crate::rebuild::rebuild`] and the ordinary incremental path both call the
//! exact same [`Projection::apply`] over the exact same stored rows, so
//! whatever the convention says, both paths agree on what it says.
//!
//! # Why `Registry` cannot be empty and still claim to prove anything
//!
//! `crates/ori-gates/src/gate.rs` and `crates/ori-gates/src/runner.rs` solved
//! this for gates: `DefinitionState` has two variants and only
//! `runner::Registry::list` can produce the third, `Installed`, so a gate
//! reported installed without a runner is a value that cannot be built rather
//! than a case nobody wrote a test for. `runner::Registry::audit` additionally
//! refuses an empty registry outright, because "no entry violates the rule" is
//! vacuously true of zero entries.
//!
//! [`Registry`] follows the second half of that pattern, not the first: a
//! `Projection` is a trait object behind `Box<dyn Projection>`, so there is no
//! third state to make unrepresentable by construction the way
//! `DefinitionState` does. What is unrepresentable instead is a *rebuild* over
//! nothing succeeding: [`crate::rebuild::rebuild`] calls [`Registry::is_empty`]
//! before it opens a transaction, before it resets a single table, before it
//! reads a single event, and refuses with a [`ori_core::error::MethodologyRef`]
//! rather than proceeding to do nothing and report success. That refusal is
//! this ticket's `runner::AuditError::Empty`.
//!
//! The second guard is [`PROJECTION_COUNT`], a constant written independently
//! of [`standard`]'s body: a test asserts `standard().len() ==
//! PROJECTION_COUNT` and that `PROJECTION_COUNT == 3`. Deleting one
//! `registry.register(...)` line from [`standard`] drops its length to two
//! while the independently-declared constant stays three, so the suite fails
//! on the missing projector rather than quietly proving less. This is the same
//! shape as `crate::event_log::EventLog::verify`'s independent `SELECT
//! COUNT(*)` cross-check against what a read actually returned
//! (`crate::event_log`'s own module doc, "what defect 8 caught"): an
//! expectation stated once, away from the code whose completeness it checks,
//! so the code cannot silently agree with a smaller version of itself.
//!
//! ```mermaid
//! flowchart TB
//!   E[Event, read from the log in seq order] --> D{registry.projections}
//!   D --> T[TicketProjection::apply]
//!   D --> L[LockProjection::apply]
//!   D --> X[EscalationProjection::apply]
//!   T --> PT[(proj_tickets)]
//!   L --> PL[(proj_locks)]
//!   X --> PX[(proj_escalations)]
//!   PT & PL & PX --> DA[dump_all: one framed byte string]
//!   R[Registry::is_empty] -->|true| REFUSE[rebuild refuses, MethodologyRef]
//!   R -->|false| REBUILD[rebuild: reset_all, replay seq 1.., dump_all]
//! ```
//!
//! Must not: contain business rules, or update or delete a row of `events`
//! (`spec/LLD.md` section 2; CLAUDE.md's load-bearing facts).

pub mod escalation;
pub mod lock;
pub(crate) mod payload;
pub mod ticket;

use core::fmt;

use rusqlite::Connection;

use crate::event_log::Event;

/// How many projections [`standard`] installs.
///
/// Declared independently of [`standard`]'s body, so a test can assert
/// `standard().len() == PROJECTION_COUNT` and catch a registration line
/// deleted from [`standard`] rather than only a projector whose logic
/// regressed. See the module doc, "Why `Registry` cannot be empty and still
/// claim to prove anything".
pub const PROJECTION_COUNT: usize = 3;

/// One derived table, folded forward from the event log: `spec/DATA_MODEL.md`
/// section 1.
///
/// Every method takes `&Connection` rather than `&mut Connection`: a
/// projector's own state lives entirely in the SQLite tables it reads and
/// writes through the connection it is given, never in a Rust-level field, so
/// two calls through the same connection always see what the one before it
/// left, and nothing about a [`Projection`] value itself needs to change to
/// process the next event. [`crate::rebuild::rebuild`] and the ordinary
/// incremental path both call these same three methods; neither gets a
/// different implementation.
pub trait Projection {
    /// The name this projection is registered and reported under, for
    /// [`dump_all`]'s framing and a human reading a report.
    fn name(&self) -> &'static str;

    /// Clears every row this projection owns, leaving its tables as empty as
    /// a fresh migration would. Does not drop or recreate a table: the
    /// tables are `migrations/0002_projections.sql`'s to own, and clearing
    /// rows is sufficient for state that carries no autoincrement counter or
    /// index a `DELETE` would not also reset correctly.
    ///
    /// # Errors
    ///
    /// [`ProjectionError::Sql`] if the underlying statement fails.
    fn reset(&self, conn: &Connection) -> Result<(), ProjectionError>;

    /// Folds one event into this projection's tables, or does nothing when
    /// `event` is not this projection's concern (its `kind` does not carry
    /// this projection's namespace prefix).
    ///
    /// # Errors
    ///
    /// [`ProjectionError::MissingTicketId`] when `event.kind()` is this
    /// projection's concern and `event.ticket_id()` is absent: every entity
    /// these three projections derive is a per-ticket entity
    /// (`spec/DATA_MODEL.md` section 1's `Ticket ||--o{ ... }` edges), so an
    /// event that names itself as one of theirs and carries no ticket is
    /// malformed data, refused rather than silently dropped (this is the
    /// guard against the "skip every event whose `ticket_id` is `NULL`"
    /// defect the pull request report's plant table names).
    /// [`ProjectionError::MalformedPayload`] when a required field is absent
    /// from `event.payload()`. [`ProjectionError::Sql`] on an underlying
    /// statement failure.
    fn apply(&self, conn: &Connection, event: &Event) -> Result<(), ProjectionError>;

    /// A deterministic byte encoding of every row this projection currently
    /// holds, ordered by primary key, for the identity comparison
    /// [`crate::rebuild`]'s tests run.
    ///
    /// "Deterministic" is the whole job: two calls against two connections
    /// holding the same logical rows must return the same bytes regardless of
    /// physical insertion order, which is why every implementation issues an
    /// explicit `ORDER BY` rather than relying on a `SELECT`'s natural row
    /// order (the pull request report's plant table, item 6, is the defect
    /// this guards).
    ///
    /// # Errors
    ///
    /// [`ProjectionError::Sql`] if the underlying statement fails.
    fn dump(&self, conn: &Connection) -> Result<Vec<u8>, ProjectionError>;
}

/// Something a [`Projection`] could not read from or write to its own
/// tables, or the malformed input that made it refuse.
///
/// The split mirrors `crate::event_log::EventLogError`: a refusal
/// ([`ProjectionError::is_refusal`] true) carries the
/// [`ori_core::error::MethodologyRef`] a control's refusal owes; a value that
/// failed to reach the database carries none, because it is not a control
/// refusing an action.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProjectionError {
    /// An event whose `kind` this projection owns carried no `ticket_id`,
    /// though every entity this projection derives is a per-ticket entity.
    /// AICD §13: an event that cannot be traced to the ticket it is about is
    /// a link the audit trail cannot be walked through.
    MissingTicketId {
        /// The `seq` of the offending event.
        seq: u64,
        /// The event's own `kind`, for the human reading the refusal.
        kind: String,
    },
    /// A payload this projection needed a field from did not carry it, or the
    /// field was not the shape the projection reads it as.
    MalformedPayload {
        /// The `seq` of the offending event.
        seq: u64,
        /// The field name that was required and absent.
        field: &'static str,
    },
    /// `rusqlite` reported a failure this module does not classify further.
    Sql {
        /// `rusqlite::Error`'s own message.
        message: String,
    },
}

impl ProjectionError {
    /// The methodology section a refusal is made under, for the refusals and
    /// for nothing else.
    #[must_use]
    pub const fn methodology_ref(&self) -> Option<ori_core::error::MethodologyRef> {
        match self {
            // AICD §13: "This is the audit trail: any line of code can be
            // traced to a commit, to a ticket, to a paragraph of
            // specification, to a human decision." An event this projection
            // owns and that carries no ticket_id is a link the trail cannot
            // be walked through; `crate::event_log::EventLogError::MissingActor`
            // cites the same section for the same reason, one field over.
            Self::MissingTicketId { .. } => Some(ori_core::error::MethodologyRef {
                section: 13,
                subsection: None,
            }),
            // AICD §8: this crate's own memory-layer citation
            // (`crates/ori-store/src/lib.rs`), reused here because a payload
            // this projection cannot read is this store failing to stand in
            // for state it cannot vouch for, the same ground `db.rs`'s
            // `DbError` cites throughout.
            Self::MalformedPayload { .. } => Some(ori_core::error::MethodologyRef {
                section: 8,
                subsection: None,
            }),
            Self::Sql { .. } => None,
        }
    }

    /// Whether a control refused, as opposed to the database reporting an
    /// unrelated failure.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(
            self,
            Self::MissingTicketId { .. } | Self::MalformedPayload { .. }
        )
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTicketId { seq, kind } => write!(
                f,
                "refused: event at seq {seq} has kind {kind:?}, which this projection owns, and \
                 carries no ticket_id"
            ),
            Self::MalformedPayload { seq, field } => write!(
                f,
                "refused: event at seq {seq}'s payload carries no readable {field:?} field"
            ),
            Self::Sql { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ProjectionError {}

impl From<rusqlite::Error> for ProjectionError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sql {
            message: err.to_string(),
        }
    }
}

/// Writes a variable-length byte string's length, as eight big-endian bytes,
/// before the bytes themselves: the same framing
/// `crate::event_log::update_framed` uses over a hasher, reused here over a
/// growing `Vec<u8>` so that two adjacent fields of a [`Projection::dump`]
/// encoding, or two adjacent projections in [`dump_all`]'s output, can never
/// be split at a different point than the one they were written at.
pub(crate) fn write_framed(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Writes an optional text field: a one-byte presence tag, then the framed
/// text (empty when absent), so that `None` and `Some("")` never encode to
/// the same bytes.
pub(crate) fn write_optional(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(text) => {
            out.push(1);
            write_framed(out, text.as_bytes());
        }
        None => {
            out.push(0);
            write_framed(out, b"");
        }
    }
}

/// The projections this product knows about: `spec/DATA_MODEL.md` section 1.
///
/// Holds `Box<dyn Projection>` rather than a fixed struct of three fields, the
/// same choice `crates/ori-gates/src/runner.rs`'s own `Registry` makes for
/// `Box<dyn Runner>` and for the reason its module doc gives: a caller that
/// needs to iterate "every projection" (`crate::rebuild::rebuild`) is written
/// once against the trait, and a fourth projector is one more
/// [`Registry::register`] call rather than a new field everywhere that loop
/// appears.
#[derive(Default)]
pub struct Registry {
    projections: Vec<Box<dyn Projection>>,
}

impl Registry {
    /// An empty registry. [`crate::rebuild::rebuild`] refuses to run over one;
    /// see the module doc.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one projection.
    pub fn register(&mut self, projection: Box<dyn Projection>) {
        self.projections.push(projection);
    }

    /// Whether this registry holds no projection at all: the case
    /// [`crate::rebuild::rebuild`] refuses rather than treats as a rebuild
    /// over nothing that trivially succeeds.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.projections.is_empty()
    }

    /// How many projections this registry holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.projections.len()
    }

    /// Every registered projection, in registration order (the order
    /// [`dump_all`] frames them in, and the order [`crate::rebuild::rebuild`]
    /// resets and applies them in).
    pub fn projections(&self) -> impl Iterator<Item = &dyn Projection> {
        self.projections.iter().map(Box::as_ref)
    }
}

/// The registry this product actually runs: [`ticket::TicketProjection`],
/// [`lock::LockProjection`] and [`escalation::EscalationProjection`], in that
/// order. See the module doc, "The three projectors, and why these three",
/// and [`PROJECTION_COUNT`] for the independent check that this list cannot
/// silently shrink.
#[must_use]
pub fn standard() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(ticket::TicketProjection));
    registry.register(Box::new(lock::LockProjection));
    registry.register(Box::new(escalation::EscalationProjection));
    registry
}

/// Clears every table every registered projection owns.
///
/// # Errors
///
/// The first [`ProjectionError`] any projection's [`Projection::reset`]
/// returns.
pub fn reset_all(conn: &Connection, registry: &Registry) -> Result<(), ProjectionError> {
    for projection in registry.projections() {
        projection.reset(conn)?;
    }
    Ok(())
}

/// Folds one event into every registered projection, in registration order.
///
/// # Errors
///
/// The first [`ProjectionError`] any projection's [`Projection::apply`]
/// returns; later projections in the registry are not applied to this event
/// when an earlier one refuses.
pub fn apply_all(
    conn: &Connection,
    registry: &Registry,
    event: &Event,
) -> Result<(), ProjectionError> {
    for projection in registry.projections() {
        projection.apply(conn, event)?;
    }
    Ok(())
}

/// A deterministic byte encoding of every row every registered projection
/// currently holds, framed by name so that one projection's bytes can never
/// bleed into its neighbour's: the identity comparison
/// `crate::rebuild`'s tests run, "compare them the strictest way you can
/// defend: dump every projection table in a deterministic order and compare
/// the bytes" (the pull request report quotes this instruction verbatim).
///
/// # Errors
///
/// The first [`ProjectionError`] any projection's [`Projection::dump`]
/// returns.
pub fn dump_all(conn: &Connection, registry: &Registry) -> Result<Vec<u8>, ProjectionError> {
    let mut out = Vec::new();
    for projection in registry.projections() {
        write_framed(&mut out, projection.name().as_bytes());
        let rows = projection.dump(conn)?;
        write_framed(&mut out, &rows);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ori_t_0025_standard_registers_exactly_the_projections_this_crate_implements() {
        // The anti-vacuity test the pull request report names: PROJECTION_COUNT
        // is declared independently of standard()'s body, so deleting one
        // registration line drops len() to 2 while this constant stays 3, and
        // the assertion below fails rather than the claim quietly shrinking.
        let registry = standard();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), PROJECTION_COUNT);
        assert_eq!(PROJECTION_COUNT, 3);
        let names: Vec<&str> = registry.projections().map(Projection::name).collect();
        assert_eq!(names, vec!["ticket", "lock", "escalation"]);
    }

    #[test]
    fn ori_t_0025_a_fresh_registry_is_empty() {
        let registry = Registry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert_eq!(registry.projections().count(), 0);
    }

    #[test]
    fn ori_t_0025_write_framed_never_lets_two_fields_of_different_lengths_collide() {
        // The same ambiguity crate::event_log::update_framed's own doc names:
        // without a length prefix, "ab"+"c" and "a"+"bc" would concatenate to
        // the same bytes. With one, they must not.
        let mut one = Vec::new();
        write_framed(&mut one, b"ab");
        write_framed(&mut one, b"c");
        let mut other = Vec::new();
        write_framed(&mut other, b"a");
        write_framed(&mut other, b"bc");
        assert_ne!(one, other);
    }

    #[test]
    fn ori_t_0025_write_optional_distinguishes_none_from_some_empty() {
        let mut none = Vec::new();
        write_optional(&mut none, None);
        let mut some_empty = Vec::new();
        write_optional(&mut some_empty, Some(""));
        assert_ne!(none, some_empty);
    }

    #[test]
    fn ori_t_0025_projection_error_display_names_are_readable() {
        let missing = ProjectionError::MissingTicketId {
            seq: 3,
            kind: "ticket.filed".to_owned(),
        };
        assert!(missing.is_refusal());
        assert_eq!(
            missing.methodology_ref().map(|reference| reference.section),
            Some(13)
        );
        assert!(missing.to_string().contains("seq 3"));

        let malformed = ProjectionError::MalformedPayload {
            seq: 5,
            field: "category",
        };
        assert!(malformed.is_refusal());
        assert_eq!(
            malformed
                .methodology_ref()
                .map(|reference| reference.section),
            Some(8)
        );

        let sql = ProjectionError::Sql {
            message: "disk I/O error".to_owned(),
        };
        assert!(!sql.is_refusal());
        assert_eq!(sql.methodology_ref(), None);
    }
}
