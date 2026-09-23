//! `products.rebuild` is reachable through the real `ProductDb::open` path,
//! not only through a test harness that builds its own schema: ORI-T-0107,
//! `spec/ARCHITECTURE.md` section 8, "migrations run forward on open".
//!
//! Every test under `crates/ori-store/src/projections/` and
//! `crates/ori-store/src/rebuild.rs` (ORI-T-0025) builds its schema with
//! `include_str!` against an in-memory connection, bypassing
//! `ProductDb::open` entirely. That proved the projectors and `rebuild`
//! itself, over real SQL, but proved nothing about whether a live product
//! ever reaches that SQL: `crate::db::MIGRATIONS` carried one entry until
//! ORI-T-0107 appended a second, and until it did, `ProductDb::open` never
//! created `proj_tickets`, `proj_locks` or `proj_escalations` at all. This
//! file is the integration level `spec/LLD.md` section 4 names for exactly
//! this ("Tests live beside the code (`mod tests`) for units, in
//! `crates/<crate>/tests/` for integration"): it drives the same public API
//! a real caller would, `ProductDb::open`, `ProductDb::connection`,
//! `EventLog::append`, `rebuild`, and nothing else.
//!
//! No methodology section applies to the harness below beyond what the
//! module doc of `crate::rebuild` and `crate::projections` already cite; this
//! file's own citation is `spec/ARCHITECTURE.md` section 8's rule that
//! migrations run forward on open, which is the rule `ProductDb::open`
//! reaching `proj_tickets`/`proj_locks`/`proj_escalations` at all depends on.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Timestamp;
use ori_store::db::MIGRATIONS;
use ori_store::db::ProductDb;
use ori_store::event_log::EventLog;
use ori_store::projections;
use ori_store::rebuild::rebuild;

/// A fresh scratch product-root directory, unique per test: the same pattern
/// `crates/ori-store/src/db.rs`'s own test module uses, copied rather than
/// shared because that one is private to its own `#[cfg(test)]` module.
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
            "the temp dir is absolute on every platform this runs on"
        );
        let path = root.join(format!(
            "ori-t-0107-{label}-{}-{unique}",
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

fn at(millis: i64) -> Timestamp {
    Timestamp::from_millis(millis)
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

/// Every table `sqlite_master` names, for asserting a set of names is a
/// subset of what a real `ProductDb::open` actually created.
fn table_names(conn: &rusqlite::Connection) -> Vec<String> {
    let mut statement = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .expect("prepare sqlite_master read");
    statement
        .query_map([], |row| row.get(0))
        .expect("query sqlite_master")
        .collect::<Result<_, _>>()
        .expect("read every table name")
}

/// The versions `_ori_migrations` records, in ascending order: the
/// observable half of "migrations run forward on open"
/// (`spec/ARCHITECTURE.md` section 8).
fn recorded_versions(conn: &rusqlite::Connection) -> Vec<u32> {
    let mut statement = conn
        .prepare("SELECT version FROM _ori_migrations ORDER BY version ASC")
        .expect("prepare _ori_migrations read");
    statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("query _ori_migrations")
        .map(|version| version.expect("read one version") as u32)
        .collect()
}

#[test]
fn ori_p1_028_a_fresh_product_db_open_creates_all_three_projection_tables_and_records_versions_one_and_two()
 {
    let scratch = Scratch::new("fresh-open");
    let mut db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
        .expect("a fresh product opens and applies every migration MIGRATIONS carries");

    assert_eq!(
        db.schema_version().expect("read schema version"),
        MIGRATIONS.len() as u32,
        "a fresh open lands at the binary's full migration count"
    );

    let names = table_names(db.connection());
    for table in ["proj_tickets", "proj_locks", "proj_escalations"] {
        assert!(
            names.iter().any(|name| name == table),
            "ProductDb::open must have created {table}; sqlite_master carries: {names:?}"
        );
    }

    let versions = recorded_versions(db.connection());
    assert_eq!(
        versions,
        vec![1, 2],
        "_ori_migrations must record exactly the two versions MIGRATIONS carries, in order"
    );
}

/// Appends a small, real log through `ProductDb::connection`, folding each
/// event into the standard registry immediately (the single writer task's
/// own shape, `spec/LLD.md` section 5), and returns the product handle and
/// the registry so the caller can dump, clear, or rebuild against the same
/// real connection.
fn seed(db: &mut ProductDb) -> projections::Registry {
    let registry = projections::standard();
    let product_id = product();

    let events: [(&str, &str); 5] = [
        ("ticket.filed", r#"{"category":"auto","kind":"defect"}"#),
        ("ticket.categorized", r#"{"category":"behavioral"}"#),
        ("ticket.validated", "{}"),
        (
            "lock.claimed",
            r#"{"module":"crates/ori-core","session_id":"S1"}"#,
        ),
        (
            "escalation.raised",
            r#"{"escalation_id":"E1","trigger":"adr_area","question":"q","recommendation":"r"}"#,
        ),
    ];
    let ticket = id("TICKET1");

    for (index, (kind, payload)) in events.iter().enumerate() {
        let event = EventLog::append(
            db.connection(),
            product_id.clone(),
            at(i64::try_from(index).expect("small test index") + 1),
            agent(),
            *kind,
            Some(ticket.clone()),
            *payload,
        )
        .expect("a well-formed event against a freshly migrated, real product database");
        projections::apply_all(db.connection(), &registry, &event)
            .expect("the seeded events are all well-formed for their own projections");
    }

    registry
}

#[test]
fn ori_p1_028_rebuild_through_the_real_connection_reproduces_incremental_state_identically() {
    let scratch = Scratch::new("rebuild-real");
    let mut db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
        .expect("a fresh product opens through the real path");

    let registry = seed(&mut db);

    let incremental_dump =
        projections::dump_all(db.connection(), &registry).expect("dump incremental state");
    assert!(
        !incremental_dump.is_empty(),
        "a real log was built through the real connection; an empty dump would prove nothing"
    );

    // The same cleared-start discipline ORI-T-0025's own identity test uses,
    // now over the tables ProductDb::open itself created: clear first, so
    // rebuild is proven to reconstruct state, not merely leave it alone.
    projections::reset_all(db.connection(), &registry)
        .expect("clear projection state through the real connection");
    let cleared_dump =
        projections::dump_all(db.connection(), &registry).expect("dump cleared state");
    assert_ne!(
        cleared_dump, incremental_dump,
        "clearing must actually have changed something, or this test proves nothing"
    );

    let report = rebuild(db.connection(), &registry).expect("rebuild over a real, well-formed log");
    assert_eq!(report.events_replayed, 5);
    assert_eq!(report.projections, projections::PROJECTION_COUNT);

    let rebuilt_dump =
        projections::dump_all(db.connection(), &registry).expect("dump rebuilt state");
    assert_eq!(
        rebuilt_dump, incremental_dump,
        "rebuild through the real ProductDb connection must reproduce the incrementally-built \
         state byte for byte, exactly as it does over the in-memory harness"
    );
}

#[test]
fn ori_p1_028_reopening_a_seeded_product_does_not_reapply_migration_two_and_keeps_the_projection_state()
 {
    let scratch = Scratch::new("reopen-survives");
    let incremental_dump;
    {
        let mut db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
            .expect("first open, applies both migrations");
        let registry = seed(&mut db);
        incremental_dump =
            projections::dump_all(db.connection(), &registry).expect("dump before close");
    } // dropped: lock released, connection closed

    let mut reopened = ProductDb::open(&scratch.path, "PRODUCT-A", at(2_000))
        .expect("reopening an already-migrated product succeeds");

    assert_eq!(
        reopened.schema_version().expect("schema version"),
        MIGRATIONS.len() as u32,
        "reopening must not change the recorded schema version"
    );
    let versions = recorded_versions(reopened.connection());
    assert_eq!(
        versions,
        vec![1, 2],
        "migration 2 must be recorded exactly once, not reapplied on reopen"
    );

    let registry = projections::standard();
    let survived_dump =
        projections::dump_all(reopened.connection(), &registry).expect("dump after reopen");
    assert_eq!(
        survived_dump, incremental_dump,
        "the projection state written before close must survive an ordinary reopen unchanged"
    );
}
