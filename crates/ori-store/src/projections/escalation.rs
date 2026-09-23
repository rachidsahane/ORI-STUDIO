//! The `Escalation` projection: `spec/DATA_MODEL.md` section 2's `Escalation`
//! row, folded forward from `escalation.*` events into `proj_escalations`.
//!
//! `crates/ori-orchestrator/src/escalation.rs`'s `Escalation::raise` already
//! refuses a trigger with no finding behind it, a blank question, and a blank
//! recommendation, before anything is ever appended. This projection assumes
//! all three already held, the same "must not contain business rules"
//! discipline `crate::projections::ticket` and `crate::projections::lock`
//! both state in their own module docs.
//!
//! # Why the row is keyed by `escalation_id` and not by `ticket_id`
//!
//! `spec/DATA_MODEL.md` section 1 draws `Ticket ||--o{ Escalation : raises`:
//! one ticket may raise more than one escalation over its lifetime (an
//! `adr_area` question answered, then later a `security` one on the same
//! ticket). The event's own `ticket_id` column cannot key a table that must
//! hold more than one row per ticket, so `escalation_id` is read out of the
//! payload instead, the same way `crate::projections::lock` reads `module`
//! for the same structural reason.
//!
//! # The event kinds this projection owns
//!
//! `escalation.raised`, payload fields `escalation_id`, `trigger`,
//! `question`, `recommendation`, all required; ticket_id from the event's own
//! column. Inserts a new row, state `open`.
//!
//! `escalation.answered`, payload fields `escalation_id`, `answered_by`,
//! `answer`, all required. Updates the row to state `answered`. If no row
//! with that `escalation_id` exists yet (the `escalation.raised` event was
//! skipped, the same malformed-sequencing shape
//! `crate::projections::ticket`'s module doc names for `ticket.filed`), a row
//! is still created, with `trigger`, `question` and `recommendation` left
//! `NULL`: honest about what was actually replayed, and what makes skipping
//! an earlier event observable in the dump rather than silently absorbed.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rusqlite::params;

use super::Projection;
use super::ProjectionError;
use super::payload;
use super::write_framed;
use super::write_optional;
use crate::event_log::Event;

const TABLE: &str = "proj_escalations";

fn required_field<'a>(event: &'a Event, key: &'static str) -> Result<&'a str, ProjectionError> {
    payload::field(event.payload(), key).ok_or(ProjectionError::MalformedPayload {
        seq: event.seq(),
        field: key,
    })
}

struct Row {
    escalation_id: String,
    product_id: String,
    ticket_id: String,
    trigger: Option<String>,
    question: Option<String>,
    recommendation: Option<String>,
    state: String,
    answered_by: Option<String>,
    answer: Option<String>,
    last_seq: u64,
    updated_at: i64,
}

fn read_row(conn: &Connection, escalation_id: &str) -> Result<Option<Row>, ProjectionError> {
    conn.query_row(
        &format!(
            "SELECT escalation_id, product_id, ticket_id, trigger, question, recommendation, \
             state, answered_by, answer, last_seq, updated_at FROM {TABLE} WHERE escalation_id = ?1"
        ),
        params![escalation_id],
        |row| {
            let last_seq: i64 = row.get(9)?;
            Ok(Row {
                escalation_id: row.get(0)?,
                product_id: row.get(1)?,
                ticket_id: row.get(2)?,
                trigger: row.get(3)?,
                question: row.get(4)?,
                recommendation: row.get(5)?,
                state: row.get(6)?,
                answered_by: row.get(7)?,
                answer: row.get(8)?,
                last_seq: u64::try_from(last_seq).unwrap_or(0),
                updated_at: row.get(10)?,
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
             (escalation_id, product_id, ticket_id, trigger, question, recommendation, state, \
              answered_by, answer, last_seq, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(escalation_id) DO UPDATE SET \
             product_id = excluded.product_id, ticket_id = excluded.ticket_id, \
             trigger = excluded.trigger, question = excluded.question, \
             recommendation = excluded.recommendation, state = excluded.state, \
             answered_by = excluded.answered_by, answer = excluded.answer, \
             last_seq = excluded.last_seq, updated_at = excluded.updated_at"
        ),
        params![
            row.escalation_id,
            row.product_id,
            row.ticket_id,
            row.trigger,
            row.question,
            row.recommendation,
            row.state,
            row.answered_by,
            row.answer,
            i64::try_from(row.last_seq).unwrap_or(i64::MAX),
            row.updated_at,
        ],
    )?;
    Ok(())
}

/// `spec/DATA_MODEL.md` section 2's `Escalation` row, folded from
/// `escalation.*` events: `spec/DATA_MODEL.md` section 1.
#[derive(Clone, Copy, Debug, Default)]
pub struct EscalationProjection;

impl Projection for EscalationProjection {
    fn name(&self) -> &'static str {
        "escalation"
    }

    fn reset(&self, conn: &Connection) -> Result<(), ProjectionError> {
        conn.execute(&format!("DELETE FROM {TABLE}"), [])?;
        Ok(())
    }

    fn apply(&self, conn: &Connection, event: &Event) -> Result<(), ProjectionError> {
        let Some(suffix) = event.kind().strip_prefix("escalation.") else {
            return Ok(());
        };
        let Some(ticket_id) = event.ticket_id() else {
            return Err(ProjectionError::MissingTicketId {
                seq: event.seq(),
                kind: event.kind().to_owned(),
            });
        };

        match suffix {
            "raised" => {
                let escalation_id = required_field(event, "escalation_id")?.to_owned();
                let trigger = required_field(event, "trigger")?.to_owned();
                let question = required_field(event, "question")?.to_owned();
                let recommendation = required_field(event, "recommendation")?.to_owned();
                let row = Row {
                    escalation_id,
                    product_id: event.product_id().as_str().to_owned(),
                    ticket_id: ticket_id.as_str().to_owned(),
                    trigger: Some(trigger),
                    question: Some(question),
                    recommendation: Some(recommendation),
                    state: "open".to_owned(),
                    answered_by: None,
                    answer: None,
                    last_seq: event.seq(),
                    updated_at: event.at().millis(),
                };
                write_row(conn, &row)
            }
            "answered" => {
                let escalation_id = required_field(event, "escalation_id")?.to_owned();
                let answered_by = required_field(event, "answered_by")?.to_owned();
                let answer = required_field(event, "answer")?.to_owned();
                let mut row = read_row(conn, &escalation_id)?.unwrap_or(Row {
                    escalation_id: escalation_id.clone(),
                    product_id: event.product_id().as_str().to_owned(),
                    ticket_id: ticket_id.as_str().to_owned(),
                    trigger: None,
                    question: None,
                    recommendation: None,
                    state: String::new(),
                    answered_by: None,
                    answer: None,
                    last_seq: 0,
                    updated_at: 0,
                });
                row.product_id = event.product_id().as_str().to_owned();
                row.ticket_id = ticket_id.as_str().to_owned();
                row.state = "answered".to_owned();
                row.answered_by = Some(answered_by);
                row.answer = Some(answer);
                row.last_seq = event.seq();
                row.updated_at = event.at().millis();
                write_row(conn, &row)
            }
            _ => Ok(()),
        }
    }

    fn dump(&self, conn: &Connection) -> Result<Vec<u8>, ProjectionError> {
        let mut statement = conn.prepare(&format!(
            "SELECT escalation_id, product_id, ticket_id, trigger, question, recommendation, \
             state, answered_by, answer, updated_at FROM {TABLE} ORDER BY escalation_id ASC"
        ))?;
        let rows = statement.query_map([], |row| {
            let escalation_id: String = row.get(0)?;
            let product_id: String = row.get(1)?;
            let ticket_id: String = row.get(2)?;
            let trigger: Option<String> = row.get(3)?;
            let question: Option<String> = row.get(4)?;
            let recommendation: Option<String> = row.get(5)?;
            let state: String = row.get(6)?;
            let answered_by: Option<String> = row.get(7)?;
            let answer: Option<String> = row.get(8)?;
            let updated_at: i64 = row.get(9)?;
            Ok((
                escalation_id,
                product_id,
                ticket_id,
                trigger,
                question,
                recommendation,
                state,
                answered_by,
                answer,
                updated_at,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (
                escalation_id,
                product_id,
                ticket_id,
                trigger,
                question,
                recommendation,
                state,
                answered_by,
                answer,
                updated_at,
            ) = row?;
            write_framed(&mut out, escalation_id.as_bytes());
            write_framed(&mut out, product_id.as_bytes());
            write_framed(&mut out, ticket_id.as_bytes());
            write_optional(&mut out, trigger.as_deref());
            write_optional(&mut out, question.as_deref());
            write_optional(&mut out, recommendation.as_deref());
            write_framed(&mut out, state.as_bytes());
            write_optional(&mut out, answered_by.as_deref());
            write_optional(&mut out, answer.as_deref());
            out.extend_from_slice(&updated_at.to_be_bytes());
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

    use super::EscalationProjection;
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

    fn human() -> Actor {
        Actor::Human(id("HUMAN"))
    }

    fn ts(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn ori_t_0025_raised_creates_an_open_row() {
        let mut conn = connection();
        let projection = EscalationProjection;
        let ticket = id("TICKET1");
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "escalation.raised",
            Some(ticket.clone()),
            r#"{"escalation_id":"E1","trigger":"adr_area","question":"may this proceed?","recommendation":"stop"}"#,
        )
        .expect("append");
        projection.apply(&conn, &event).expect("apply");

        let row: (String, String, String) = conn
            .query_row(
                "SELECT state, trigger, ticket_id FROM proj_escalations WHERE escalation_id = 'E1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("row exists");
        assert_eq!(row.0, "open");
        assert_eq!(row.1, "adr_area");
        assert_eq!(row.2, ticket.as_str());
    }

    #[test]
    fn ori_t_0025_answered_updates_state_and_leaves_the_original_fields() {
        let mut conn = connection();
        let projection = EscalationProjection;
        let ticket = id("TICKET1");
        for (actor, kind, payload) in [
            (
                agent(),
                "escalation.raised",
                r#"{"escalation_id":"E1","trigger":"security","question":"ok?","recommendation":"no"}"#
                    .to_owned(),
            ),
            (
                human(),
                "escalation.answered",
                r#"{"escalation_id":"E1","answered_by":"HUMAN","answer":"approved"}"#.to_owned(),
            ),
        ] {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(1),
                actor,
                kind,
                Some(ticket.clone()),
                payload,
            )
            .expect("append");
            projection.apply(&conn, &event).expect("apply");
        }

        let row: (String, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT state, answered_by, answer, trigger FROM proj_escalations WHERE escalation_id = 'E1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("row exists");
        assert_eq!(row.0, "answered");
        assert_eq!(row.1, Some("HUMAN".to_owned()));
        assert_eq!(row.2, Some("approved".to_owned()));
        assert_eq!(
            row.3,
            Some("security".to_owned()),
            "trigger was set by escalation.raised and must survive being answered"
        );
    }

    #[test]
    fn ori_t_0025_an_escalation_domain_event_with_no_ticket_id_is_refused() {
        let mut conn = connection();
        let projection = EscalationProjection;
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "escalation.raised",
            None,
            r#"{"escalation_id":"E1","trigger":"security","question":"ok?","recommendation":"no"}"#,
        )
        .expect("append");
        let error = projection
            .apply(&conn, &event)
            .expect_err("an escalation.* event with no ticket_id is malformed");
        assert!(matches!(
            error,
            ProjectionError::MissingTicketId { seq: 1, .. }
        ));
    }

    #[test]
    fn ori_t_0025_raised_with_a_missing_field_is_refused() {
        let mut conn = connection();
        let projection = EscalationProjection;
        let ticket = id("TICKET1");
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "escalation.raised",
            Some(ticket),
            r#"{"escalation_id":"E1","trigger":"security"}"#,
        )
        .expect("append");
        let error = projection
            .apply(&conn, &event)
            .expect_err("question and recommendation are required");
        assert!(matches!(
            error,
            ProjectionError::MalformedPayload {
                field: "question",
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0025_an_event_outside_the_escalation_namespace_is_left_alone() {
        let mut conn = connection();
        let projection = EscalationProjection;
        let ticket = id("TICKET1");
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
        projection
            .apply(&conn, &event)
            .expect("not this projection's concern");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proj_escalations", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, 0);
    }

    #[test]
    fn ori_t_0025_dump_is_ordered_by_escalation_id() {
        let mut conn = connection();
        let projection = EscalationProjection;
        let ticket = id("TICKET1");
        for escalation_id in ["E2", "E1"] {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(1),
                agent(),
                "escalation.raised",
                Some(ticket.clone()),
                format!(
                    r#"{{"escalation_id":"{escalation_id}","trigger":"security","question":"ok?","recommendation":"no"}}"#
                ),
            )
            .expect("append");
            projection.apply(&conn, &event).expect("apply");
        }
        let dump = projection.dump(&conn).expect("dump");
        let e1_pos = dump
            .windows(2)
            .position(|window| window == b"E1")
            .expect("E1 present");
        let e2_pos = dump
            .windows(2)
            .position(|window| window == b"E2")
            .expect("E2 present");
        assert!(e1_pos < e2_pos, "escalation_id order is ascending");
    }
}
