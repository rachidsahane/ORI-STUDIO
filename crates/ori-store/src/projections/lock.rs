//! The `LockEntry` projection: `spec/DATA_MODEL.md` section 2's `LockEntry`
//! row, folded forward from `lock.*` events into `proj_locks`.
//!
//! # What this projection does and does not prove about the overlap invariant
//!
//! `spec/DATA_MODEL.md` section 4: "`LockEntry` modules for two `InProgress`
//! tickets never overlap." `crates/ori-orchestrator/src/lock_table.rs`'s
//! `LockTable::claim` is where that invariant is actually enforced, before an
//! event is ever appended: its own module doc states the shape,
//! `entries: BTreeMap<String, Entry>` keyed by module, so a second ticket's
//! claim over a module already held is refused (`E_SCOPE_LOCKED`) rather than
//! written. This projection is keyed by `module` the same way, for the same
//! reason `crate::projections::mod`'s doc gives for the crate as a whole: by
//! the time a `lock.claimed` event reaches this log, `LockTable::claim` has
//! already said yes.
//!
//! What that means concretely for [`LockProjection::apply`]: it does not
//! refuse a `lock.claimed` event for a module another ticket already holds in
//! this table. It cannot, without re-deciding a question
//! `LockTable::claim` already decided, which is exactly the "contain business
//! rules" `spec/LLD.md` section 2 forbids this crate. If two such events ever
//! did reach the log (a defect upstream of this crate, or a caller bypassing
//! `LockTable`), this projection would do what a `BTreeMap::insert` does,
//! record whichever claimed the module last, silently. The invariant this
//! table's *primary key on `module`* still gives for free is narrower and
//! purely structural: two different tickets can never simultaneously *appear*
//! as the holder of the same module in this table, because inserting a second
//! row for one module overwrites rather than duplicates the first. Whether
//! that overwrite should have been permitted at all is `LockTable::claim`'s
//! question, tested directly and exhaustively in
//! `crates/ori-orchestrator/src/lock_table.rs`'s own test module (in
//! particular
//! `tests::ori_t_0050_no_two_tickets_hold_overlapping_modules_after_any_sequence`),
//! not re-tested here.
//!
//! # The event kinds this projection owns
//!
//! `lock.claimed`, payload field `module` required, `session_id` optional;
//! ticket_id from the event's own column, matching
//! `crates/ori-orchestrator/src/lock_table.rs`'s `Entry.ticket`. Upserts one
//! row keyed by `module`.
//!
//! `lock.released`, no payload field required; deletes every row this
//! event's `ticket_id` holds, mirroring `LockTable::release`'s own doc,
//! "Why release is by ticket and never by module": "A claim ends when the
//! ticket stops being in flight, so the argument is the ticket and every
//! entry it holds goes together."

use rusqlite::Connection;
use rusqlite::params;

use super::Projection;
use super::ProjectionError;
use super::payload;
use super::write_framed;
use super::write_optional;
use crate::event_log::Event;

const TABLE: &str = "proj_locks";

/// `spec/DATA_MODEL.md` section 2's `LockEntry` row, folded from `lock.*`
/// events: `spec/DATA_MODEL.md` section 1.
#[derive(Clone, Copy, Debug, Default)]
pub struct LockProjection;

impl Projection for LockProjection {
    fn name(&self) -> &'static str {
        "lock"
    }

    fn reset(&self, conn: &Connection) -> Result<(), ProjectionError> {
        conn.execute(&format!("DELETE FROM {TABLE}"), [])?;
        Ok(())
    }

    fn apply(&self, conn: &Connection, event: &Event) -> Result<(), ProjectionError> {
        let Some(suffix) = event.kind().strip_prefix("lock.") else {
            return Ok(());
        };
        let Some(ticket_id) = event.ticket_id() else {
            return Err(ProjectionError::MissingTicketId {
                seq: event.seq(),
                kind: event.kind().to_owned(),
            });
        };

        match suffix {
            "claimed" => {
                let module = payload::field(event.payload(), "module").ok_or(
                    ProjectionError::MalformedPayload {
                        seq: event.seq(),
                        field: "module",
                    },
                )?;
                let session_id = payload::field(event.payload(), "session_id");
                conn.execute(
                    &format!(
                        "INSERT INTO {TABLE} (module, product_id, ticket_id, session_id, acquired_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5) \
                         ON CONFLICT(module) DO UPDATE SET \
                         product_id = excluded.product_id, ticket_id = excluded.ticket_id, \
                         session_id = excluded.session_id, acquired_at = excluded.acquired_at"
                    ),
                    params![
                        module,
                        event.product_id().as_str(),
                        ticket_id.as_str(),
                        session_id,
                        event.at().millis(),
                    ],
                )?;
                Ok(())
            }
            "released" => {
                conn.execute(
                    &format!("DELETE FROM {TABLE} WHERE ticket_id = ?1"),
                    params![ticket_id.as_str()],
                )?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn dump(&self, conn: &Connection) -> Result<Vec<u8>, ProjectionError> {
        let mut statement = conn.prepare(&format!(
            "SELECT module, product_id, ticket_id, session_id, acquired_at FROM {TABLE} \
             ORDER BY module ASC"
        ))?;
        let rows = statement.query_map([], |row| {
            let module: String = row.get(0)?;
            let product_id: String = row.get(1)?;
            let ticket_id: String = row.get(2)?;
            let session_id: Option<String> = row.get(3)?;
            let acquired_at: i64 = row.get(4)?;
            Ok((module, product_id, ticket_id, session_id, acquired_at))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (module, product_id, ticket_id, session_id, acquired_at) = row?;
            write_framed(&mut out, module.as_bytes());
            write_framed(&mut out, product_id.as_bytes());
            write_framed(&mut out, ticket_id.as_bytes());
            write_optional(&mut out, session_id.as_deref());
            out.extend_from_slice(&acquired_at.to_be_bytes());
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

    use super::LockProjection;
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
    fn ori_t_0025_claimed_writes_a_row_keyed_by_module() {
        let mut conn = connection();
        let projection = LockProjection;
        let ticket = id("TICKET1");
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(10),
            agent(),
            "lock.claimed",
            Some(ticket.clone()),
            r#"{"module":"crates/ori-core","session_id":"S1"}"#,
        )
        .expect("append");
        projection.apply(&conn, &event).expect("apply");

        let row: (String, Option<String>, i64) = conn
            .query_row(
                "SELECT ticket_id, session_id, acquired_at FROM proj_locks WHERE module = ?1",
                rusqlite::params!["crates/ori-core"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("row exists");
        assert_eq!(row.0, ticket.as_str());
        assert_eq!(row.1, Some("S1".to_owned()));
        assert_eq!(row.2, 10);
    }

    #[test]
    fn ori_t_0025_released_removes_every_module_the_ticket_holds_and_no_other() {
        let mut conn = connection();
        let projection = LockProjection;
        let first = id("TICKET1");
        let second = id("TICKET2");

        for (ticket, module) in [(&first, "crates/ori-core"), (&first, "docs")] {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(1),
                agent(),
                "lock.claimed",
                Some(ticket.clone()),
                format!(r#"{{"module":"{module}"}}"#),
            )
            .expect("append");
            projection.apply(&conn, &event).expect("apply");
        }
        let claim_second = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "lock.claimed",
            Some(second.clone()),
            r#"{"module":"crates/ori-orchestrator"}"#,
        )
        .expect("append");
        projection.apply(&conn, &claim_second).expect("apply");

        let release = EventLog::append(
            &mut conn,
            product(),
            ts(2),
            agent(),
            "lock.released",
            Some(first),
            "{}",
        )
        .expect("append");
        projection.apply(&conn, &release).expect("apply");

        let remaining: Vec<String> = {
            let mut statement = conn
                .prepare("SELECT module FROM proj_locks ORDER BY module")
                .expect("prepare");
            statement
                .query_map([], |row| row.get(0))
                .expect("query")
                .collect::<Result<_, _>>()
                .expect("rows")
        };
        assert_eq!(remaining, vec!["crates/ori-orchestrator".to_owned()]);
    }

    #[test]
    fn ori_t_0025_a_lock_domain_event_with_no_ticket_id_is_refused() {
        let mut conn = connection();
        let projection = LockProjection;
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "lock.claimed",
            None,
            r#"{"module":"crates/ori-core"}"#,
        )
        .expect("append");
        let error = projection
            .apply(&conn, &event)
            .expect_err("a lock.* event with no ticket_id is malformed");
        assert!(matches!(
            error,
            ProjectionError::MissingTicketId { seq: 1, .. }
        ));
    }

    #[test]
    fn ori_t_0025_claimed_with_no_module_field_is_refused() {
        let mut conn = connection();
        let projection = LockProjection;
        let ticket = id("TICKET1");
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            agent(),
            "lock.claimed",
            Some(ticket),
            "{}",
        )
        .expect("append");
        let error = projection
            .apply(&conn, &event)
            .expect_err("module is required by lock.claimed");
        assert!(matches!(
            error,
            ProjectionError::MalformedPayload {
                field: "module",
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0025_an_event_outside_the_lock_namespace_is_left_alone() {
        let mut conn = connection();
        let projection = LockProjection;
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
            .query_row("SELECT COUNT(*) FROM proj_locks", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0);
    }

    #[test]
    fn ori_t_0025_dump_is_ordered_by_module() {
        let mut conn = connection();
        let projection = LockProjection;
        let ticket = id("TICKET1");
        for module in ["docs", "crates/ori-core"] {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(1),
                agent(),
                "lock.claimed",
                Some(ticket.clone()),
                format!(r#"{{"module":"{module}"}}"#),
            )
            .expect("append");
            projection.apply(&conn, &event).expect("apply");
        }
        let dump = projection.dump(&conn).expect("dump");
        let core_pos = dump
            .windows("crates/ori-core".len())
            .position(|window| window == b"crates/ori-core")
            .expect("crates/ori-core present");
        let docs_pos = dump
            .windows("docs".len())
            .position(|window| window == b"docs")
            .expect("docs present");
        assert!(core_pos < docs_pos, "module order is ascending");
    }
}
