//! Integration tests proving the seam between `ori_store::db::ProductDb`'s
//! migrated schema and `ori_store::event_log::EventLog`: AICD §8 (the
//! append-only, hash-chained event log), AICD §14 ("a gate is installed only
//! after it has been seen to fail on a planted defect").
//!
//! `crates/ori-store/src/event_log.rs` and `crates/ori-store/src/db.rs` were
//! written in parallel against two independent ideas of the `events` table.
//! Every existing unit test in each module proves its own half correct
//! against its own schema: `event_log.rs`'s tests hand-write
//! `CREATE TABLE events (...)` themselves, and `db.rs`'s tests never call
//! `EventLog`. Nothing before this file proved the two halves agree.
//!
//! This lives under `tests/`, a Cargo integration test, on purpose: it can
//! only reach `ori_store`'s public API, so it is structurally unable to
//! hand-write its own schema and structurally forced to exercise
//! `EventLog::append`, `EventLog::read_range` and `EventLog::verify` against
//! the exact table `ProductDb::open` creates by actually running
//! `crates/ori-store/migrations/0001_init.sql`, the way any real caller
//! outside this crate would.
//!
//! Every test here was run, by hand, against five defects planted one at a
//! time in a scratch copy of `migrations/0001_init.sql` (never in this
//! committed file): a `CHECK (json_valid(payload))` added to `payload`,
//! `hash_prev` renamed to `prev_hash`, the `events_no_update` trigger
//! dropped, the `events_no_delete` trigger dropped, and the `actor_id` CHECK
//! widened to require `actor_id IS NOT NULL` unconditionally. Each plant
//! failed this suite before being reverted; the pull request report carries
//! the exact exit codes and failure messages, because a comment in this file
//! is not itself the evidence AICD §14 asks for.
//!
//! # Whether this is a seam test or a coincidence
//!
//! The one test below that a hand-written `CREATE TABLE` (in place of
//! `ProductDb::open`) cannot satisfy is
//! [`ori_t_0106_the_migrations_append_blocking_triggers_are_installed_by_open_alone`]:
//! it inspects `sqlite_master` immediately after `ProductDb::open`, before
//! any `EventLog::append` call has had a chance to install its own,
//! independent, idempotent copy of the same two triggers
//! (`event_log.rs`'s `install_guards`, called at the start of every append).
//! Every other test here calls `EventLog::append` at least once before
//! checking anything, and `install_guards` alone is already enough to make
//! `UPDATE`/`DELETE` refusal and read/verify round-tripping hold over *any*
//! correctly-columned `events` table, migrated or hand-rolled. That is not a
//! flaw hiding in this file: it is `event_log.rs`'s own documented
//! defense-in-depth ("What enforces append-only", point 2) doing exactly
//! what it says, and the pull request report answers the ticket's
//! substitution question in full against that fact rather than around it.

use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

// ---------------------------------------------------------------------
// Scratch layout, the same pattern `crates/ori-store/src/db.rs`'s own
// tests use for a fixture path that is absolute on every platform CI
// builds for, copied here because a `tests/` integration test cannot
// reach a `#[cfg(test)]` module inside the crate to reuse it.
// ---------------------------------------------------------------------

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir();
        assert!(
            root.is_absolute(),
            "the temporary directory is absolute on every platform this runs on: {}",
            root.display()
        );
        let path = root.join(format!(
            "ori-t-0106-{label}-{}-{unique}",
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

/// A valid ULID-shaped id, distinguished by the label so two calls with
/// different labels never collide. Mirrors
/// `crate::event_log::tests::id` (private to that module, unreachable from
/// here), not a copy of a test this ticket may not modify: nothing under
/// `#[cfg(test)]` in `event_log.rs` is touched by this file.
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

fn ts(millis: i64) -> Timestamp {
    Timestamp::from_millis(millis)
}

/// Appends a run of five events, through [`EventLog::append`] alone, that
/// together cover every case the ticket names: a `Human` actor, an `Agent`
/// actor, a `System` actor (whose `actor_id` the migration's `CHECK`
/// requires `NULL`), an event with `ticket_id` present and one with it
/// absent, and two payloads that are not valid JSON, including one carried
/// by the `System` actor event, which is the exact combination the planted
/// `CHECK (json_valid(payload))` broke.
fn append_run(conn: &mut Connection, product_id: &Id) -> Vec<Event> {
    let first = EventLog::append(
        conn,
        product_id.clone(),
        ts(1_000),
        Actor::Human(id("HUMANONE")),
        "ticket.filed",
        Some(id("TICKET001")),
        r#"{"n":1}"#,
    )
    .expect("event 1: Human actor, ticket present, JSON payload");
    let second = EventLog::append(
        conn,
        product_id.clone(),
        ts(2_000),
        Actor::Agent(id("AGENTONE")),
        "ticket.validated",
        None,
        r#"{"n":2}"#,
    )
    .expect("event 2: Agent actor, ticket absent, JSON payload");
    let third = EventLog::append(
        conn,
        product_id.clone(),
        ts(3_000),
        Actor::System,
        "gate.proven",
        Some(id("TICKET002")),
        "not json at all, deliberately",
    )
    .expect(
        "event 3: System actor (actor_id NULL per the migration's CHECK), ticket present, a \
         non-JSON payload: the exact case the planted json_valid(payload) CHECK broke",
    );
    let fourth = EventLog::append(
        conn,
        product_id.clone(),
        ts(4_000),
        Actor::Human(id("HUMANTWO")),
        "note.recorded",
        None,
        "still plain text, still not json",
    )
    .expect("event 4: Human actor, ticket absent, a second non-JSON payload");
    let fifth = EventLog::append(
        conn,
        product_id.clone(),
        ts(5_000),
        Actor::Agent(id("AGENTTWO")),
        "ticket.merged",
        Some(id("TICKET003")),
        r#"{"n":5}"#,
    )
    .expect("event 5: Agent actor, ticket present, JSON payload");

    vec![first, second, third, fourth, fifth]
}

/// Acceptance criteria 1 and 2: append a run of events against the schema
/// `ProductDb::open` actually migrated, then read it back with
/// [`EventLog::read_range`] and get exactly what was appended, in order.
#[test]
fn ori_t_0106_append_and_read_range_round_trip_over_the_migrated_schema() {
    let scratch = Scratch::new("append-read-range");
    let product_id = id("PRODUCT");
    let mut db = ProductDb::open(&scratch.path, product_id.as_str(), ts(500))
        .expect("a fresh product opens against the real migration");

    let appended = append_run(db.connection(), &product_id);
    assert_eq!(appended.len(), 5, "append_run always appends five events");

    let read = EventLog::read_range(db.connection(), 1, 5)
        .expect("read_range over exactly what was appended verifies its own linkage");
    assert_eq!(
        read, appended,
        "read_range must reproduce exactly what EventLog::append returned, in order, over the \
         table the migration actually created"
    );
}

/// Acceptance criterion 3: [`EventLog::verify`] reports a clean chain over
/// the migrated table.
#[test]
fn ori_t_0106_verify_reports_a_clean_chain_over_the_migrated_table() {
    let scratch = Scratch::new("verify-clean-chain");
    let product_id = id("PRODUCT");
    let mut db = ProductDb::open(&scratch.path, product_id.as_str(), ts(500))
        .expect("a fresh product opens against the real migration");

    let appended = append_run(db.connection(), &product_id);
    let last = appended
        .last()
        .expect("append_run always appends five events");

    let report = EventLog::verify(db.connection()).expect(
        "a chain written entirely through EventLog::append over the migrated schema verifies clean",
    );
    assert_eq!(report.events_checked, 5);
    assert_eq!(report.tip_seq, Some(5));
    assert_eq!(report.tip_digest, Some(last.digest()));
}

/// Acceptance criterion 4: close the `ProductDb`, reopen it, and confirm the
/// chain still verifies and `append` continues correctly onto the reopened
/// tip. Catches a migration that re-runs, or a schema that silently resets
/// state, on a second open.
#[test]
fn ori_t_0106_reopening_the_product_keeps_the_chain_and_append_continues_onto_its_tip() {
    let scratch = Scratch::new("reopen-continues-tip");
    let product_id = id("PRODUCT");

    let (before_report, tip_digest) = {
        let mut db = ProductDb::open(&scratch.path, product_id.as_str(), ts(500))
            .expect("first open against the real migration");
        let appended = append_run(db.connection(), &product_id);
        let report = EventLog::verify(db.connection()).expect("clean chain before close");
        let tip_digest = appended.last().expect("five events").digest();
        (report, tip_digest)
    }; // db dropped here: connection closed, lock released.

    let mut reopened = ProductDb::open(&scratch.path, product_id.as_str(), ts(9_000))
        .expect("reopen after a clean close");

    let after_report = EventLog::verify(reopened.connection()).expect(
        "the chain still verifies after a close and reopen: the migration did not re-run or \
         reset state",
    );
    assert_eq!(
        after_report, before_report,
        "reopening must not change what verify reports over the same, unappended-to log"
    );
    assert_eq!(after_report.tip_digest, Some(tip_digest));

    let sixth = EventLog::append(
        reopened.connection(),
        product_id.clone(),
        ts(10_000),
        Actor::Agent(id("AGENTTHREE")),
        "ticket.merged",
        Some(id("TICKET004")),
        r#"{"n":6}"#,
    )
    .expect("append continues correctly onto the reopened tip");
    assert_eq!(sixth.seq(), 6);
    assert_eq!(
        sixth.hash_prev(),
        tip_digest,
        "the sixth event must chain onto the tip the closed connection left behind, not onto a \
         reset genesis"
    );

    let read = EventLog::read_range(reopened.connection(), 1, 6)
        .expect("read_range over the whole log after reopening and one more append");
    assert_eq!(read.len(), 6);
    assert_eq!(read.last().expect("six events").seq(), 6);
}

/// Acceptance criterion 5, isolated to the migration's own contribution.
///
/// Checked against `sqlite_master` immediately after [`ProductDb::open`],
/// before any [`EventLog::append`] call: `event_log.rs`'s `install_guards`
/// installs the identically-named `events_no_update` / `events_no_delete`
/// triggers idempotently on every append, so a check made *after* an append
/// cannot tell "the migration installed this" apart from "event_log.rs's own
/// defense-in-depth installed this instead". Checking before the first
/// append is the only way to attribute the triggers to the migration
/// specifically, which is what this test is for.
#[test]
fn ori_t_0106_the_migrations_append_blocking_triggers_are_installed_by_open_alone() {
    let scratch = Scratch::new("triggers-from-open-alone");
    let product_id = id("PRODUCT");
    let mut db = ProductDb::open(&scratch.path, product_id.as_str(), ts(500))
        .expect("a fresh product opens against the real migration");

    for trigger in ["events_no_update", "events_no_delete"] {
        let sql = format!(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = '{trigger}'"
        );
        let count: i64 = db
            .connection()
            .query_row(&sql, [], |row| row.get(0))
            .unwrap_or_else(|e| panic!("query sqlite_master for trigger {trigger}: {e}"));
        assert_eq!(
            count, 1,
            "the migration itself must install the {trigger} trigger on ProductDb::open alone, \
             before any EventLog::append call has a chance to install its own idempotent copy"
        );
    }
}

/// Acceptance criterion 5, end to end: through the same path any real
/// caller uses (open, append, then attempt a raw mutation), the
/// append-blocking triggers refuse an `UPDATE` and a `DELETE` against the
/// table `ProductDb` handed back, and the chain is unaffected by the refused
/// attempts.
#[test]
fn ori_t_0106_append_only_is_enforced_end_to_end_through_productdb_and_eventlog() {
    let scratch = Scratch::new("append-only-end-to-end");
    let product_id = id("PRODUCT");
    let mut db = ProductDb::open(&scratch.path, product_id.as_str(), ts(500))
        .expect("a fresh product opens against the real migration");

    let appended = append_run(db.connection(), &product_id);
    assert_eq!(appended.len(), 5);

    let update = db
        .connection()
        .execute("UPDATE events SET kind = 'tampered' WHERE seq = 1", []);
    assert!(
        update.is_err(),
        "an UPDATE against the real migrated table, reached through ProductDb, must be refused"
    );

    let delete = db
        .connection()
        .execute("DELETE FROM events WHERE seq = 1", []);
    assert!(
        delete.is_err(),
        "a DELETE against the real migrated table, reached through ProductDb, must be refused"
    );

    let report = EventLog::verify(db.connection())
        .expect("the refused UPDATE and DELETE must not have changed anything the chain checks");
    assert_eq!(report.events_checked, 5);
    assert_eq!(report.tip_seq, Some(5));
}
