//! The `Ticket` projection: `spec/DATA_MODEL.md` section 2's `Ticket` row,
//! folded forward from `ticket.*` events into `proj_tickets`.
//!
//! `ori_core::ticket::Ticket::apply` already decides, in `crates/ori-core`,
//! whether a ticket may move from one state to another, whether a category
//! may be lowered, whether a ticket may close. This module does none of that:
//! it assumes every event it is given already passed those guards before it
//! was appended (`crates/ori-store/src/projections/mod.rs`'s module doc,
//! "must not contain business rules"), and folds the one fact each event
//! kind carries into the row for the ticket it names.
//!
//! # The event kinds this projection owns
//!
//! `spec/LLD.md` section 4's naming convention, `domain.verb_past`, with the
//! `ticket.` domain. Every one of these fourteen is this ticket's own
//! invention (see the parent module's doc, "What a payload convention this
//! ticket invents means for \"identically\""), chosen to mirror
//! `ori_core::ticket::TicketEventKind`'s own eleven state-reaching variants
//! plus the two field-only changes, so that a future writer wiring the real
//! event log to `Ticket::apply` has an obvious event to emit for each one:
//!
//! | Event kind | Required payload field | Effect |
//! |---|---|---|
//! | `ticket.filed` | `category`, `kind` | creates the row, state `Filed` |
//! | `ticket.categorized` | `category` | state `Categorized` |
//! | `ticket.rejected` | | state `Rejected` |
//! | `ticket.validated` | | state `Validated` |
//! | `ticket.queued` | | state `Queued` |
//! | `ticket.in_progress` | | state `InProgress` |
//! | `ticket.blocked` | | state `Blocked` |
//! | `ticket.escalated` | | state `Escalated` |
//! | `ticket.in_review` | | state `InReview` |
//! | `ticket.merged` | `significant` (bool) | state `Merged` |
//! | `ticket.deployed` | | state `Deployed` |
//! | `ticket.closed` | | state `Closed` |
//! | `ticket.category_set` | `category` | category only, no state change |
//! | `ticket.tier_set` | `tier` | tier only, no state change |
//!
//! An event whose `kind` does not begin `ticket.` is not this projection's
//! concern and [`TicketProjection::apply`] returns `Ok(())` without touching
//! anything. An event that does begin `ticket.` but is none of the fourteen
//! above is likewise left alone (`Ok(())`): a future event kind this
//! projection does not yet know is not this crate's vocabulary to police
//! (`spec/LLD.md` section 2, "must not contain business rules" reads the same
//! way for "what kinds may exist" as it does for "which transitions are
//! legal").
//!
//! # Why a missing row is created rather than refused
//!
//! A `ticket.categorized` event for a `ticket_id` this table has never seen
//! (the row `ticket.filed` would have created) still writes a row, with
//! `kind` left `NULL` because only `ticket.filed`'s payload carries it. This
//! is deliberate, for two reasons. First, honesty about what replay actually
//! did: a projector that instead refused would make
//! `crate::rebuild::rebuild` fail on any log where seq 1 is not that ticket's
//! `ticket.filed` (an ordinary case once more than one ticket shares a log),
//! which is a much louder and less specific failure than the row simply
//! missing the field nothing set. Second, it is exactly what makes plant 4
//! in the pull request report's table ("make rebuild replay from seq 2
//! instead of 1") observable: skipping the `ticket.filed` event leaves `kind`
//! `NULL` in the rebuilt row where the incrementally-built one carries the
//! real value, a byte-for-byte difference [`crate::rebuild`]'s identity test
//! catches directly.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rusqlite::params;

use super::Projection;
use super::ProjectionError;
use super::payload;
use super::write_framed;
use super::write_optional;
use crate::event_log::Event;

const TABLE: &str = "proj_tickets";

/// One row of `proj_tickets`, read, mutated in Rust, and written back: a
/// deliberately simple read-modify-write rather than a single `SQL` upsert
/// with a `COALESCE` per column, because half of the fourteen event kinds
/// touch only `state` and must leave every other column exactly as they found
/// it, which is easier to get right, and to read, as ordinary field
/// assignment than as a growing `ON CONFLICT` clause.
struct Row {
    ticket_id: String,
    product_id: String,
    state: String,
    category: Option<String>,
    kind: Option<String>,
    tier: Option<String>,
    significant: Option<bool>,
    last_seq: u64,
    updated_at: i64,
}

impl Row {
    fn fresh(ticket_id: &str, product_id: &str) -> Self {
        Self {
            ticket_id: ticket_id.to_owned(),
            product_id: product_id.to_owned(),
            state: String::new(),
            category: None,
            kind: None,
            tier: None,
            significant: None,
            last_seq: 0,
            updated_at: 0,
        }
    }
}

fn read_row(conn: &Connection, ticket_id: &str) -> Result<Option<Row>, ProjectionError> {
    conn.query_row(
        &format!(
            "SELECT ticket_id, product_id, state, category, kind, tier, significant, last_seq, \
             updated_at FROM {TABLE} WHERE ticket_id = ?1"
        ),
        params![ticket_id],
        |row| {
            let last_seq: i64 = row.get(7)?;
            let significant: Option<String> = row.get(6)?;
            Ok(Row {
                ticket_id: row.get(0)?,
                product_id: row.get(1)?,
                state: row.get(2)?,
                category: row.get(3)?,
                kind: row.get(4)?,
                tier: row.get(5)?,
                significant: significant.map(|value| value == "true"),
                last_seq: u64::try_from(last_seq).unwrap_or(0),
                updated_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(ProjectionError::from)
}

fn write_row(conn: &Connection, row: &Row) -> Result<(), ProjectionError> {
    conn.execute(
        &format!(
            "INSERT INTO {TABLE} \
             (ticket_id, product_id, state, category, kind, tier, significant, last_seq, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(ticket_id) DO UPDATE SET \
             product_id = excluded.product_id, state = excluded.state, category = excluded.category, \
             kind = excluded.kind, tier = excluded.tier, significant = excluded.significant, \
             last_seq = excluded.last_seq, updated_at = excluded.updated_at"
        ),
        params![
            row.ticket_id,
            row.product_id,
            row.state,
            row.category,
            row.kind,
            row.tier,
            row.significant.map(|value| if value { "true" } else { "false" }),
            i64::try_from(row.last_seq).unwrap_or(i64::MAX),
            row.updated_at,
        ],
    )?;
    Ok(())
}

fn required_field<'a>(event: &'a Event, key: &'static str) -> Result<&'a str, ProjectionError> {
    payload::field(event.payload(), key).ok_or(ProjectionError::MalformedPayload {
        seq: event.seq(),
        field: key,
    })
}

fn required_bool_field(event: &Event, key: &'static str) -> Result<bool, ProjectionError> {
    payload::bool_field(event.payload(), key).ok_or(ProjectionError::MalformedPayload {
        seq: event.seq(),
        field: key,
    })
}

/// `spec/DATA_MODEL.md` section 2's `Ticket` row, folded from `ticket.*`
/// events: `spec/DATA_MODEL.md` section 1.
#[derive(Clone, Copy, Debug, Default)]
pub struct TicketProjection;

impl Projection for TicketProjection {
    fn name(&self) -> &'static str {
        "ticket"
    }

    fn reset(&self, conn: &Connection) -> Result<(), ProjectionError> {
        conn.execute(&format!("DELETE FROM {TABLE}"), [])?;
        Ok(())
    }

    fn apply(&self, conn: &Connection, event: &Event) -> Result<(), ProjectionError> {
        let Some(suffix) = event.kind().strip_prefix("ticket.") else {
            return Ok(());
        };
        let Some(ticket_id) = event.ticket_id() else {
            return Err(ProjectionError::MissingTicketId {
                seq: event.seq(),
                kind: event.kind().to_owned(),
            });
        };

        let mut row = read_row(conn, ticket_id.as_str())?
            .unwrap_or_else(|| Row::fresh(ticket_id.as_str(), event.product_id().as_str()));

        match suffix {
            "filed" => {
                row.state = "Filed".to_owned();
                row.category = Some(required_field(event, "category")?.to_owned());
                row.kind = Some(required_field(event, "kind")?.to_owned());
            }
            "categorized" => {
                row.state = "Categorized".to_owned();
                row.category = Some(required_field(event, "category")?.to_owned());
            }
            "rejected" => row.state = "Rejected".to_owned(),
            "validated" => row.state = "Validated".to_owned(),
            "queued" => row.state = "Queued".to_owned(),
            "in_progress" => row.state = "InProgress".to_owned(),
            "blocked" => row.state = "Blocked".to_owned(),
            "escalated" => row.state = "Escalated".to_owned(),
            "in_review" => row.state = "InReview".to_owned(),
            "merged" => {
                row.state = "Merged".to_owned();
                row.significant = Some(required_bool_field(event, "significant")?);
            }
            "deployed" => row.state = "Deployed".to_owned(),
            "closed" => row.state = "Closed".to_owned(),
            "category_set" => {
                row.category = Some(required_field(event, "category")?.to_owned());
            }
            "tier_set" => {
                row.tier = Some(required_field(event, "tier")?.to_owned());
            }
            _ => return Ok(()),
        }

        row.product_id = event.product_id().as_str().to_owned();
        row.last_seq = event.seq();
        row.updated_at = event.at().millis();
        write_row(conn, &row)
    }

    fn dump(&self, conn: &Connection) -> Result<Vec<u8>, ProjectionError> {
        let mut statement = conn.prepare(&format!(
            "SELECT ticket_id, product_id, state, category, kind, tier, significant, last_seq, \
             updated_at FROM {TABLE} ORDER BY ticket_id ASC"
        ))?;
        let rows = statement.query_map([], |row| {
            let last_seq: i64 = row.get(7)?;
            let significant: Option<String> = row.get(6)?;
            Ok(Row {
                ticket_id: row.get(0)?,
                product_id: row.get(1)?,
                state: row.get(2)?,
                category: row.get(3)?,
                kind: row.get(4)?,
                tier: row.get(5)?,
                significant: significant.map(|value| value == "true"),
                last_seq: u64::try_from(last_seq).unwrap_or(0),
                updated_at: row.get(8)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            let row = row?;
            write_framed(&mut out, row.ticket_id.as_bytes());
            write_framed(&mut out, row.product_id.as_bytes());
            write_framed(&mut out, row.state.as_bytes());
            write_optional(&mut out, row.category.as_deref());
            write_optional(&mut out, row.kind.as_deref());
            write_optional(&mut out, row.tier.as_deref());
            write_optional(
                &mut out,
                row.significant
                    .map(|value| if value { "true" } else { "false" }),
            );
            out.extend_from_slice(&row.last_seq.to_be_bytes());
            out.extend_from_slice(&row.updated_at.to_be_bytes());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use ori_core::types::Actor;
    use ori_core::types::Id;
    use ori_core::types::Timestamp;
    use rusqlite::Connection;

    use super::TicketProjection;
    use crate::event_log::EventLog;
    use crate::projections::Projection;
    use crate::projections::ProjectionError;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite always opens");
        conn.execute_batch(include_str!("../../migrations/0001_init.sql"))
            .expect("migration 0001 is valid SQL");
        conn.execute_batch(include_str!("../../migrations/0002_projections.sql"))
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

    fn ts(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn ori_t_0025_filed_creates_a_row_and_later_events_update_it_without_erasing_earlier_fields() {
        let mut conn = connection();
        let projection = TicketProjection;
        let ticket = id("TICKET1");

        let filed = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "ticket.filed",
            Some(ticket.clone()),
            r#"{"category":"auto","kind":"defect"}"#,
        )
        .expect("append");
        projection.apply(&conn, &filed).expect("apply filed");

        let categorized = EventLog::append(
            &mut conn,
            product(),
            ts(2),
            agent(),
            "ticket.categorized",
            Some(ticket.clone()),
            r#"{"category":"behavioral"}"#,
        )
        .expect("append");
        projection
            .apply(&conn, &categorized)
            .expect("apply categorized");

        let row: (String, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT state, category, kind, tier FROM proj_tickets WHERE ticket_id = ?1",
                rusqlite::params![ticket.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("row exists");
        assert_eq!(row.0, "Categorized");
        assert_eq!(row.1, Some("behavioral".to_owned()));
        assert_eq!(
            row.2,
            Some("defect".to_owned()),
            "kind was set by ticket.filed and must survive an event that never touches it"
        );
        assert_eq!(row.3, None, "tier was never set");
    }

    #[test]
    fn ori_t_0025_merged_sets_significant_and_the_state() {
        let mut conn = connection();
        let projection = TicketProjection;
        let ticket = id("TICKET2");

        for (kind, payload) in [
            ("ticket.filed", r#"{"category":"auto","kind":"feature"}"#),
            ("ticket.merged", r#"{"significant":true}"#),
        ] {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(1),
                agent(),
                kind,
                Some(ticket.clone()),
                payload,
            )
            .expect("append");
            projection.apply(&conn, &event).expect("apply");
        }

        let row: (String, Option<String>) = conn
            .query_row(
                "SELECT state, significant FROM proj_tickets WHERE ticket_id = ?1",
                rusqlite::params![ticket.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("row exists");
        assert_eq!(row.0, "Merged");
        assert_eq!(row.1, Some("true".to_owned()));
    }

    #[test]
    fn ori_t_0025_an_event_outside_the_ticket_namespace_is_left_alone() {
        let mut conn = connection();
        let projection = TicketProjection;
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "gate.proven",
            None,
            "{}",
        )
        .expect("append");
        projection
            .apply(&conn, &event)
            .expect("not this projection's concern");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proj_tickets", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0);
    }

    #[test]
    fn ori_t_0025_a_ticket_domain_event_with_no_ticket_id_is_refused_not_silently_skipped() {
        // Plant 3 in the pull request report's table: "make one projector
        // skip every event whose ticket_id is NULL. Must FAIL." This is the
        // test that must fail if that defect is planted: the honest code
        // below returns Err; a projector that silently skipped would return
        // Ok, and this assertion would fail on it.
        let mut conn = connection();
        let projection = TicketProjection;
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.filed",
            None,
            r#"{"category":"auto","kind":"defect"}"#,
        )
        .expect("EventLog::append does not itself require ticket_id for any kind");

        let error = projection
            .apply(&conn, &event)
            .expect_err("a ticket.* event with no ticket_id is malformed, not skippable");
        assert!(matches!(
            error,
            ProjectionError::MissingTicketId { seq: 1, .. }
        ));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proj_tickets", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0, "the refused event must not have written a row");
    }

    #[test]
    fn ori_t_0025_a_missing_required_payload_field_is_refused() {
        let mut conn = connection();
        let projection = TicketProjection;
        let ticket = id("TICKET3");
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "ticket.filed",
            Some(ticket),
            r#"{"category":"auto"}"#,
        )
        .expect("append");
        let error = projection
            .apply(&conn, &event)
            .expect_err("kind is required by ticket.filed and absent here");
        assert!(matches!(
            error,
            ProjectionError::MalformedPayload { field: "kind", .. }
        ));
    }

    #[test]
    fn ori_t_0025_reset_clears_every_row() {
        let mut conn = connection();
        let projection = TicketProjection;
        let ticket = id("TICKET4");
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "ticket.filed",
            Some(ticket),
            r#"{"category":"auto","kind":"chore"}"#,
        )
        .expect("append");
        projection.apply(&conn, &event).expect("apply");
        projection.reset(&conn).expect("reset");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proj_tickets", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0);
    }

    #[test]
    fn ori_t_0025_dump_is_ordered_by_ticket_id_and_reflects_every_field() {
        let mut conn = connection();
        let projection = TicketProjection;
        let first = id("TICKETA");
        let second = id("TICKETB");
        for (ticket, category) in [(&second, "decisional"), (&first, "auto")] {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(1),
                agent(),
                "ticket.filed",
                Some(ticket.clone()),
                format!(r#"{{"category":"{category}","kind":"feature"}}"#),
            )
            .expect("append");
            projection.apply(&conn, &event).expect("apply");
        }
        let dump = projection.dump(&conn).expect("dump");
        // ticket_id order is ascending regardless of the insertion order
        // above (second was inserted first).
        let first_pos = dump
            .windows(first.as_str().len())
            .position(|window| window == first.as_str().as_bytes())
            .expect("first ticket appears");
        let second_pos = dump
            .windows(second.as_str().len())
            .position(|window| window == second.as_str().as_bytes())
            .expect("second ticket appears");
        assert!(
            first_pos < second_pos,
            "dump is ordered by ticket_id ascending"
        );
    }
}
