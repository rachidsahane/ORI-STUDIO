//! `products.rebuild`: drop, replay, and prove identical.
//!
//! Criterion ORI-P1-028 in `spec/criteria/phase-1.md`, second clause:
//! "`products.rebuild` reproduces every projection identically."
//! `spec/DATA_MODEL.md` section 4's last invariant: "The SQLite file plus the
//! repository fully reconstruct every projection; a `rebuild` command proves
//! it in CI." [`rebuild`] is that command: it clears every registered
//! projection's tables, replays the whole event log from `seq` 1 through
//! [`crate::projections::apply_all`], and leaves the tables exactly where the
//! ordinary incremental path (the single writer task, `spec/LLD.md` section 5,
//! calling [`crate::projections::apply_all`] once per event as it is
//! appended) would have left them.
//!
//! # "Identically" is proved by dumping and comparing, not by inspection
//!
//! The ticket that opened this module quotes AICD §39's "present but
//! reporting nothing" and the prior art of the same trap in this repository,
//! an empty criteria list making "every criterion is covered" vacuously true,
//! an empty gate registry making "every gate has been proven" vacuously true.
//! [`rebuild`]'s own first line answers the same trap for this ticket: an
//! empty [`crate::projections::Registry`] refuses (below), rather than
//! reporting a rebuild of nothing as success.
//!
//! What "identically" is checked against, in this module's own tests: a
//! deterministic byte dump of every row of every registered projection's
//! table, in primary-key order
//! ([`crate::projections::dump_all`], each [`crate::projections::Projection::dump`]),
//! compared byte for byte between the state incremental application produced
//! and the state a full rebuild produced from a deliberately cleared start
//! (`tests::ori_p1_028_rebuild_from_scratch_reproduces_incremental_state_identically`,
//! below). Every column every `CREATE TABLE` in
//! `migrations/0002_projections.sql` declares is in that dump; none is
//! skipped, because a comparison that ignores a column is a comparison that
//! will not catch a projector that corrupts it (the pull request report says
//! this in the same words the ticket does, and states what is not compared
//! and why: nothing that IS a column of these three tables is left out; what
//! is out of scope is the five `Ticket` fields `proj_tickets` does not carry
//! at all, `title`, `spec_anchor`, `declared_scope`, `budget`, `phase_id`,
//! named and justified in `crates/ori-store/src/projections/ticket.rs`'s own
//! module doc).
//!
//! ```mermaid
//! flowchart TB
//!   S{registry.is_empty} -->|true| REFUSE["Err EmptyRegistry, AICD §39"]
//!   S -->|false| TX[begin transaction]
//!   TX --> RESET[reset_all: DELETE every projection's rows]
//!   RESET --> READ["EventLog::read_range from seq 1"]
//!   READ --> FOLD[apply_all, once per event, in seq order]
//!   FOLD --> COMMIT[commit]
//! ```
//!
//! Must not: contain business rules, or read from `seq` other than 1
//! (`spec/DATA_MODEL.md` section 4's "fully reconstruct" means from the
//! start, not from wherever a caller happens to ask).

use core::fmt;

use rusqlite::Connection;

use crate::event_log::EventLog;
use crate::event_log::EventLogError;
use crate::projections::ProjectionError;
use crate::projections::Registry;
use crate::projections::apply_all;
use crate::projections::reset_all;

/// What one call to [`rebuild`] did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RebuildReport {
    /// How many events were replayed, from `seq` 1 through the log's current
    /// tip.
    pub events_replayed: u64,
    /// How many projections were reset and replayed into.
    pub projections: usize,
}

/// Why [`rebuild`] refused, or could not finish.
#[derive(Debug)]
#[non_exhaustive]
pub enum RebuildError {
    /// `registry` held no projection at all. Refused rather than treated as a
    /// rebuild of nothing that trivially succeeds: see the module doc and
    /// `crate::projections`'s, "Why `Registry` cannot be empty and still
    /// claim to prove anything".
    EmptyRegistry,
    /// A projection's [`crate::projections::Projection::reset`] or
    /// [`crate::projections::Projection::apply`] refused or failed.
    Projection(ProjectionError),
    /// Reading the log back failed or found it broken.
    EventLog(EventLogError),
    /// The transaction itself could not be opened or committed.
    Sql(rusqlite::Error),
}

impl RebuildError {
    /// The methodology section a refusal is made under, for the refusal this
    /// module itself makes and for nothing else: a wrapped
    /// [`ProjectionError`] or [`EventLogError`] carries its own, read through
    /// [`RebuildError::methodology_ref`] rather than duplicated here.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<ori_core::error::MethodologyRef> {
        match self {
            // AICD §39: "present but reporting nothing" is the named defect
            // class this refusal exists against; a rebuild over zero
            // projections would be exactly that, reported as success.
            Self::EmptyRegistry => Some(ori_core::error::MethodologyRef {
                section: 39,
                subsection: None,
            }),
            Self::Projection(error) => error.methodology_ref(),
            Self::EventLog(error) => error.methodology_ref(),
            Self::Sql(_) => None,
        }
    }

    /// Whether this is a control refusing to proceed, as opposed to an
    /// underlying failure.
    #[must_use]
    pub fn is_refusal(&self) -> bool {
        match self {
            Self::EmptyRegistry => true,
            Self::Projection(error) => error.is_refusal(),
            Self::EventLog(error) => error.is_refusal(),
            Self::Sql(_) => false,
        }
    }
}

impl fmt::Display for RebuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRegistry => f.write_str(
                "refused: this registry holds no projection, so \"every projection rebuilt \
                 identically\" would be vacuously true of it; rebuild is not reported as \
                 succeeding over nothing (AICD §39)",
            ),
            Self::Projection(error) => write!(f, "{error}"),
            Self::EventLog(error) => write!(f, "{error}"),
            Self::Sql(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RebuildError {}

impl From<ProjectionError> for RebuildError {
    fn from(error: ProjectionError) -> Self {
        Self::Projection(error)
    }
}

impl From<EventLogError> for RebuildError {
    fn from(error: EventLogError) -> Self {
        Self::EventLog(error)
    }
}

impl From<rusqlite::Error> for RebuildError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

/// Drops and recreates all projection state, replays the entire log from
/// `seq` 1, and leaves projections in a state identical to the one
/// incremental application produced: criterion ORI-P1-028,
/// `spec/DATA_MODEL.md` section 4.
///
/// Runs inside one transaction: every registered projection is reset, then
/// every event from `seq` 1 through the log's current tip is folded into
/// every registered projection, in `seq` order, then the transaction
/// commits. A refusal partway through leaves the database exactly as it was
/// before this call, the same all-or-nothing shape
/// `crate::event_log::EventLog::append` gives one row.
///
/// # Errors
///
/// [`RebuildError::EmptyRegistry`] when `registry` holds no projection,
/// checked before a transaction is even opened. [`RebuildError::EventLog`] if
/// the log itself does not read back consistently.
/// [`RebuildError::Projection`] if any registered projection refuses an
/// event. [`RebuildError::Sql`] on a transaction failure.
pub fn rebuild(conn: &mut Connection, registry: &Registry) -> Result<RebuildReport, RebuildError> {
    if registry.is_empty() {
        return Err(RebuildError::EmptyRegistry);
    }

    let tx = conn.transaction()?;
    reset_all(&tx, registry)?;

    let events = EventLog::read_range(&tx, 1, u64::MAX)?;
    for event in &events {
        apply_all(&tx, registry, event)?;
    }
    let events_replayed = events.len() as u64;

    tx.commit()?;
    Ok(RebuildReport {
        events_replayed,
        projections: registry.len(),
    })
}

#[cfg(test)]
mod tests {
    use ori_core::types::Actor;
    use ori_core::types::Id;
    use ori_core::types::Timestamp;
    use rusqlite::Connection;

    use super::RebuildError;
    use super::rebuild;
    use crate::event_log::EventLog;
    use crate::projections;
    use crate::projections::Registry;
    use crate::projections::apply_all;
    use crate::projections::dump_all;
    use crate::projections::reset_all;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite always opens");
        conn.execute_batch(include_str!("../migrations/0001_init.sql"))
            .expect("migration 0001 is valid SQL");
        conn.execute_batch(include_str!("../migrations/0002_projections.sql"))
            .expect("migration 0002 is valid SQL");
        conn
    }

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
        Id::parse(&format!("0{safe:0>25}")).expect("a sanitized, zero-padded ULID always parses")
    }

    fn product() -> Id {
        id("PRODUCT")
    }

    fn agent() -> Actor {
        Actor::Agent(id("AGENT"))
    }

    fn human() -> Actor {
        Actor::Human(id("HUMAN"))
    }

    fn ts(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    // -----------------------------------------------------------------
    // ORI-P1-028's second clause, and the vacuous-registry refusal
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_028_rebuild_refuses_an_empty_registry_rather_than_reporting_success() {
        let mut conn = connection();
        // A log with a real event in it, so a defect that ignored the
        // registry check would otherwise have something to "succeed" over.
        EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "gate.proven",
            None,
            "{}",
        )
        .expect("append");

        let empty = Registry::new();
        let error = rebuild(&mut conn, &empty)
            .expect_err("rebuild over zero projections must refuse, not succeed vacuously");
        assert!(matches!(error, RebuildError::EmptyRegistry));
        assert!(error.is_refusal());
        assert_eq!(
            error.methodology_ref().map(|reference| reference.section),
            Some(39)
        );
        assert!(error.to_string().contains("AICD §39"));
    }

    #[test]
    fn ori_t_0025_registry_standard_pins_the_count_this_crate_actually_implements() {
        // The independent check the pull request report names: deleting one
        // registration line from projections::standard() drops len() to 2
        // while PROJECTION_COUNT, declared separately, stays 3, and this
        // fails rather than the claim silently shrinking.
        let registry = projections::standard();
        assert_eq!(registry.len(), projections::PROJECTION_COUNT);
        assert_eq!(projections::PROJECTION_COUNT, 3);
    }

    // -----------------------------------------------------------------
    // The identity proof
    // -----------------------------------------------------------------

    /// Appends one event and immediately folds it into every registered
    /// projection, the way the single writer task does
    /// (`spec/LLD.md` section 5: "Projections update from the log in the
    /// same task").
    #[allow(clippy::too_many_arguments)]
    fn append_and_apply(
        conn: &mut Connection,
        registry: &Registry,
        product_id: Id,
        at: Timestamp,
        actor: Actor,
        kind: &str,
        ticket_id: Option<Id>,
        payload: &str,
    ) {
        let event = EventLog::append(conn, product_id, at, actor, kind, ticket_id, payload)
            .expect("append succeeds for a well-formed, non-empty kind and payload");
        apply_all(conn, registry, &event).expect("the harness never feeds a malformed event");
    }

    #[test]
    fn ori_p1_028_rebuild_from_scratch_reproduces_incremental_state_identically() {
        let mut conn = connection();
        let registry = projections::standard();

        let first_ticket = id("TICKET1");
        let second_ticket = id("TICKET2");

        // A real, non-trivial log: two tickets through several lifecycle
        // events, a lock claimed and released, an escalation raised and
        // answered, and one event outside every projection's namespace.
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(1),
            agent(),
            "ticket.filed",
            Some(first_ticket.clone()),
            r#"{"category":"auto","kind":"defect"}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(2),
            agent(),
            "ticket.categorized",
            Some(first_ticket.clone()),
            r#"{"category":"behavioral"}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(3),
            human(),
            "ticket.validated",
            Some(first_ticket.clone()),
            "{}",
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(4),
            agent(),
            "ticket.queued",
            Some(first_ticket.clone()),
            "{}",
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(5),
            agent(),
            "ticket.in_progress",
            Some(first_ticket.clone()),
            "{}",
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(6),
            agent(),
            "lock.claimed",
            Some(first_ticket.clone()),
            r#"{"module":"crates/ori-core","session_id":"S1"}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(7),
            Actor::System,
            "gate.proven",
            None,
            "{}",
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(8),
            agent(),
            "escalation.raised",
            Some(first_ticket.clone()),
            r#"{"escalation_id":"E1","trigger":"adr_area","question":"may this proceed?","recommendation":"stop"}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(9),
            human(),
            "escalation.answered",
            Some(first_ticket.clone()),
            r#"{"escalation_id":"E1","answered_by":"HUMAN","answer":"proceed"}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(10),
            agent(),
            "ticket.in_review",
            Some(first_ticket.clone()),
            "{}",
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(11),
            human(),
            "ticket.merged",
            Some(first_ticket.clone()),
            r#"{"significant":true}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(12),
            agent(),
            "lock.released",
            Some(first_ticket.clone()),
            "{}",
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(13),
            agent(),
            "ticket.filed",
            Some(second_ticket.clone()),
            r#"{"category":"decisional","kind":"feature"}"#,
        );
        append_and_apply(
            &mut conn,
            &registry,
            product(),
            ts(14),
            agent(),
            "ticket.tier_set",
            Some(second_ticket),
            r#"{"tier":"2"}"#,
        );

        let incremental_dump = dump_all(&conn, &registry).expect("dump incremental state");
        assert!(
            !incremental_dump.is_empty(),
            "a real log was built; an empty dump would prove nothing"
        );

        // Prove rebuild reconstructs this from the log alone, not merely
        // leaves pre-existing projection state untouched: clear it first.
        // This is what defeats a rebuild that is a no-op returning success
        // (the pull request report's plant 5): a no-op would leave the
        // tables empty here, and the final assertion below would fail.
        reset_all(&conn, &registry).expect("clear projection state directly");
        let cleared_dump = dump_all(&conn, &registry).expect("dump cleared state");
        assert_ne!(
            cleared_dump, incremental_dump,
            "clearing must actually have changed something, or this test proves nothing"
        );

        let report = rebuild(&mut conn, &registry).expect("rebuild over a valid, well-formed log");
        assert_eq!(report.events_replayed, 14);
        assert_eq!(report.projections, projections::PROJECTION_COUNT);

        let rebuilt_dump = dump_all(&conn, &registry).expect("dump rebuilt state");
        assert_eq!(
            rebuilt_dump, incremental_dump,
            "rebuild must reproduce the incrementally-built state byte for byte"
        );
    }

    #[test]
    fn ori_p1_028_rebuild_on_an_empty_but_non_vacuous_registry_over_an_empty_log_succeeds() {
        // A registry that is genuinely non-empty (real projections are
        // registered) over a log with nothing in it yet: zero events is not
        // the vacuous case this module refuses; zero *projections* is.
        let mut conn = connection();
        let registry = projections::standard();
        let report = rebuild(&mut conn, &registry).expect("an empty log is not refused");
        assert_eq!(report.events_replayed, 0);
        assert_eq!(report.projections, projections::PROJECTION_COUNT);
        let dump = dump_all(&conn, &registry).expect("dump");
        // Every projection's own rows are empty, but the registration
        // framing itself is not, so the dump is not literally empty bytes;
        // what matters is that this path did not refuse.
        assert!(!dump.is_empty());
    }

    #[test]
    fn ori_p1_028_a_ticket_domain_event_with_no_ticket_id_makes_rebuild_refuse_and_commit_nothing()
    {
        let mut conn = connection();
        let registry = projections::standard();
        // Appended directly, bypassing append_and_apply's incremental fold,
        // to plant a malformed row in the log itself (EventLog::append does
        // not require ticket_id for any particular kind; see
        // crate::projections::ticket's module doc).
        EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.filed",
            None,
            r#"{"category":"auto","kind":"defect"}"#,
        )
        .expect("append succeeds; ticket_id is a business rule this crate does not enforce");

        let error = rebuild(&mut conn, &registry)
            .expect_err("a malformed ticket.* event must make rebuild refuse, not skip it");
        assert!(matches!(error, RebuildError::Projection(_)));

        // The refused rebuild must not have left any partial projection
        // state committed: the transaction rolled back with the connection,
        // so every projection's own table is exactly as reset() left it.
        for projection in registry.projections() {
            let rows = projection.dump(&conn).expect("dump one projection");
            assert!(
                rows.is_empty(),
                "{} must hold no rows after a refused rebuild",
                projection.name()
            );
        }
    }

    // -----------------------------------------------------------------
    // Property-based (spec/TESTING.md section 1): the general claim, over an
    // arbitrary generated sequence of events, that incremental application
    // and rebuild agree.
    // -----------------------------------------------------------------

    mod proptests {
        use ori_core::types::Actor;
        use ori_core::types::Timestamp;
        use proptest::prelude::*;

        use super::append_and_apply;
        use super::connection;
        use super::id;
        use super::product;
        use super::rebuild;
        use crate::projections;
        use crate::projections::dump_all;
        use crate::projections::reset_all;

        /// A fixed, small vocabulary this property draws from: three tickets,
        /// three lock modules, three escalation ids. Small on purpose, so
        /// that a generated sequence of `0..40` actions revisits the same
        /// entities many times, which is where an incremental-vs-rebuild
        /// divergence over repeated upserts would show up, rather than a
        /// long tail of entities touched once each.
        const TICKETS: [&str; 3] = ["TICKET1", "TICKET2", "TICKET3"];
        const MODULES: [&str; 3] = ["crates/ori-core", "crates/ori-orchestrator", "docs"];
        const ESCALATIONS: [&str; 3] = ["E1", "E2", "E3"];

        /// One generated action, turned into a `(kind, payload)` pair: every
        /// event this generates carries a `ticket_id` (the entity index maps
        /// to one of [`TICKETS`] regardless of domain), so every event is a
        /// legal, well-formed append that every projector accepts without
        /// refusing; the refusal paths (`MissingTicketId`,
        /// `MalformedPayload`) are covered by the handwritten tests in
        /// `crate::projections::ticket`, `crate::projections::lock` and
        /// `crate::projections::escalation`, not repeated here.
        fn interpret(domain: u8, action: u8) -> (&'static str, String) {
            match domain % 3 {
                0 => {
                    let kinds: [(&str, &str); 14] = [
                        ("ticket.filed", r#"{"category":"auto","kind":"defect"}"#),
                        ("ticket.categorized", r#"{"category":"behavioral"}"#),
                        ("ticket.rejected", "{}"),
                        ("ticket.validated", "{}"),
                        ("ticket.queued", "{}"),
                        ("ticket.in_progress", "{}"),
                        ("ticket.blocked", "{}"),
                        ("ticket.escalated", "{}"),
                        ("ticket.in_review", "{}"),
                        ("ticket.merged", r#"{"significant":true}"#),
                        ("ticket.deployed", "{}"),
                        ("ticket.closed", "{}"),
                        ("ticket.category_set", r#"{"category":"decisional"}"#),
                        ("ticket.tier_set", r#"{"tier":"1"}"#),
                    ];
                    let (kind, payload) = kinds[usize::from(action) % kinds.len()];
                    (kind, payload.to_owned())
                }
                1 => {
                    let module = MODULES[usize::from(action) % MODULES.len()];
                    if action.is_multiple_of(2) {
                        ("lock.claimed", format!(r#"{{"module":"{module}"}}"#))
                    } else {
                        ("lock.released", "{}".to_owned())
                    }
                }
                _ => {
                    let escalation_id = ESCALATIONS[usize::from(action) % ESCALATIONS.len()];
                    if action.is_multiple_of(2) {
                        (
                            "escalation.raised",
                            format!(
                                r#"{{"escalation_id":"{escalation_id}","trigger":"adr_area","question":"q","recommendation":"r"}}"#
                            ),
                        )
                    } else {
                        (
                            "escalation.answered",
                            format!(
                                r#"{{"escalation_id":"{escalation_id}","answered_by":"HUMAN","answer":"a"}}"#
                            ),
                        )
                    }
                }
            }
        }

        fn actor(seed: u8) -> Actor {
            if seed.is_multiple_of(3) {
                Actor::System
            } else if seed % 3 == 1 {
                Actor::Human(id("HUMAN"))
            } else {
                Actor::Agent(id("AGENT"))
            }
        }

        proptest! {
            /// For an arbitrary generated sequence of events over a small,
            /// fixed vocabulary, incremental application (fold as each event
            /// is appended) and a full rebuild from a cleared start agree,
            /// byte for byte, on every registered projection.
            ///
            /// `spec/TESTING.md` section 1 names `proptest` for exactly this,
            /// "Invariants hold for generated event sequences"; this is that
            /// invariant read for `crate::rebuild::rebuild`.
            #[test]
            fn ori_p1_028_property_incremental_and_rebuild_agree_over_any_generated_sequence(
                actions in prop::collection::vec((0u8..3, 0u8..3, 0u8..14), 0..40)
            ) {
                let mut conn = connection();
                let registry = projections::standard();
                let product_id = product();

                for (index, (domain, entity, action)) in actions.iter().copied().enumerate() {
                    let (kind, payload) = interpret(domain, action);
                    let ticket = id(TICKETS[usize::from(entity) % TICKETS.len()]);
                    let at = Timestamp::from_millis(i64::from(index as u32) + 1);
                    append_and_apply(
                        &mut conn,
                        &registry,
                        product_id.clone(),
                        at,
                        actor(action),
                        kind,
                        Some(ticket),
                        &payload,
                    );
                }

                let incremental_dump = dump_all(&conn, &registry).expect("dump incremental state");

                reset_all(&conn, &registry).expect("clear projection state");

                let report = rebuild(&mut conn, &registry).expect("rebuild over a well-formed log");
                prop_assert_eq!(report.events_replayed, actions.len() as u64);

                let rebuilt_dump = dump_all(&conn, &registry).expect("dump rebuilt state");
                prop_assert_eq!(rebuilt_dump, incremental_dump);
            }
        }
    }
}
