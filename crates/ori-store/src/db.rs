//! Schema migrations, and `ProductDb` open and lock: AICD §8, AICD §26.
//!
//! `spec/LLD.md` section 2 gives this crate "migrations, `ProductDb`
//! open/lock". `spec/LLD.md` section 6 fixes the layout `ProductDb::open`
//! establishes: one directory per product under `<app data>/ori/products/`,
//! holding `product.sqlite`, `index/`, `sessions/`, `evidence/` and `lock`.
//! Criterion ORI-P1-036 is the row this module answers part of: "Two projects
//! registered | Open both through the CLI in two daemons | Two databases,
//! two sockets, two lock files; a ticket in one is invisible to the other;
//! `memory.search` in one never returns the other's records". This crate owns
//! the two databases and the two lock files; the sockets are `ori-rpc`'s, the
//! two daemons are `ori-cli`/`ori-engine`'s, and `memory.search` is
//! `ori-memory`'s, so [`crate::db::ProductDb`] proves its share and no more.
//!
//! # What the lock is, and what it does and does not promise
//!
//! `spec/LLD.md` section 6 names a file, `lock`, and says only "single-writer
//! lock". A file that a second process can simply open promises nothing by
//! itself; what makes it a lock is what holding it means, and here that is an
//! operating-system advisory lock, taken by opening `lock` as its own tiny
//! SQLite database (`<https://sqlite.org/pragma.html#pragma_locking_mode>`),
//! setting `PRAGMA locking_mode = EXCLUSIVE`, and beginning an exclusive
//! transaction: SQLite's own file-locking (POSIX `fcntl` advisory record
//! locks on Unix, `LockFileEx` on Windows) then holds an OS-level exclusive
//! lock on that file for as long as the connection stays open, which
//! `ProductDb` never lets go before its own [`Drop`]. `lock` is deliberately
//! its own SQLite file rather than a lock taken on `product.sqlite` itself:
//! holding `product.sqlite` permanently exclusive would fight the WAL mode
//! `open_connection` sets on it for concurrent, lock-free reads
//! (`spec/LLD.md` section 5, "Reads are lock-free snapshots"), and would make
//! an external tool opening `product.sqlite` to inspect it (`sqlite3
//! product.sqlite`, a backup) itself look like a second writer.
//!
//! This was proved empirically in the scratchpad, not only argued: two
//! `rusqlite::Connection`s in one process against the same `lock` path, the
//! second refused with "database is locked" while the first is alive; a
//! third process, spawned as a child, holding the lock and killed with
//! `SIGKILL` (no [`Drop`] runs, nothing this crate's code executes), and the
//! very next attempt against the same path acquiring it immediately with no
//! special handling. That second case is `spec/runbooks/recover-engine.md`'s
//! precondition, "the lock file is stale or absent": under this design there
//! is no separate notion of staleness to detect. A `lock` file's mere
//! presence on disk means nothing at all, by design; only the OS-level lock
//! state means anything, and the OS clears that state itself, synchronously,
//! whenever the holding process's file descriptor closes for any reason,
//! including a crash. `ProductDb::open` therefore never inspects who held the
//! lock before it, never reads a PID, and never asks whether a previous
//! holder is "still running": there is nothing to ask, because the one fact
//! that matters (is this file exclusively held right now) is answered
//! directly by attempting to hold it.
//!
//! What this guarantees: at most one `ProductDb` holds a given product's
//! `lock` at a time, for as long as its process runs, released automatically
//! and immediately on every path out, graceful or not, with no cleanup step
//! and no human action required to make the next open succeed.
//!
//! What this cannot promise, stated plainly rather than left to be found
//! later:
//!
//! - It is advisory and cooperative among users of this crate's `open`. A
//!   process that opens `product.sqlite` directly, bypassing `ProductDb`
//!   entirely, is not stopped by this lock; nothing at the filesystem level
//!   can stop that, only a convention that every writer goes through
//!   `ProductDb::open` can, and this module is that convention's one
//!   implementation, not its enforcement against a process that ignores it.
//! - It has no cross-machine or network-filesystem guarantee. SQLite's own
//!   documentation is explicit that its locking is unreliable over NFS and
//!   similar filesystems; `spec/ARCHITECTURE.md` section 7 places `<app
//!   data>` on the machine the engine runs on, local or the unattended
//!   daemon's own disk, never a network mount, so this is an accepted
//!   constraint of the deployment this project ships, not a gap in the lock.
//! - It says nothing about *what* was running when the lock was held: no
//!   session, no PID, no ticket. That bookkeeping is the event log's, once
//!   `ProductDb::open` returns a connection to it; conflating "who holds the
//!   mutex" with "what work is in flight" is exactly the coupling
//!   `spec/runbooks/recover-engine.md` keeps apart by reading the log for the
//!   second question and the lock for nothing but the first.
//!
//! # Migrations: identity, order, and what a database newer than the binary
//! means
//!
//! Every migration this binary can apply is compiled into it
//! ([`include_str!`] on a file under `crates/ori-store/migrations/`, listed
//! in [`crate::db::MIGRATIONS`]), never read from a directory at runtime: a
//! missing or misplaced install location on a user's machine cannot make the
//! reader find zero migrations, because there is no read to fail. A
//! migration's identity is its `version`, a `u32` starting at 1 with no gap
//! ever allowed, checked by `validate_migrations` against the compiled-in
//! list itself and by the same check against what a database has recorded
//! having run
//! (`_ori_migrations`, one row per applied migration, holding the exact SQL
//! text that ran). Order is exactly ascending version, always starting from
//! whatever the database has already recorded plus one: there is no code path
//! that can apply version 3 before version 2 has a row, because the loop that
//! applies pending migrations only ever looks at
//! `&migrations[recorded.len()..]`.
//!
//! Applying an already-applied database's full migration set again is a
//! no-op rather than a refusal (defect 2 of the ticket's list: "must be
//! refused or be idempotent; say which"): `apply_pending` only executes the
//! slice of [`crate::db::MIGRATIONS`] strictly past what `_ori_migrations`
//! already records, so a second `open` of a database already at the binary's
//! version runs zero SQL and returns the same version, proved by
//! `tests::ori_t_0024_reopening_an_already_migrated_database_is_idempotent`.
//! Idempotent was chosen over refused because "the database is already
//! correct" is not an error condition a caller should have to handle
//! specially; refusing a repeat `open` would make every ordinary restart of
//! the engine an error.
//!
//! A migration may never be edited after it has run anywhere: enforced by
//! storing the exact SQL text in `_ori_migrations.sql` when a migration
//! applies, and comparing it, byte for byte, against the compiled-in text for
//! that version on every later open. A hashing crate was available for this
//! (`ops/escalations/E-0004-external-crates.md`) and deliberately not used:
//! the exact text is at least as precise as any hash, with no collision
//! question to reason about, and it is already resident in the binary
//! ([`crate::db::MIGRATIONS`]) at zero marginal cost, so a hash would only
//! have thrown that precision away in exchange for a shorter column.
//!
//! A database at a schema version the binary does not have (recorded rows in
//! `_ori_migrations` beyond [`crate::db::MIGRATIONS`]'s length) is the case
//! that eats data: an older binary that silently proceeded would read and
//! write through a schema it does not fully understand, using SQL that
//! assumes columns and constraints a newer migration may have added, changed
//! or removed. `apply_pending` checks this before touching anything else and
//! refuses with [`crate::db::DbError::SchemaTooNew`], never falling through
//! to "nothing to apply, must be fine": `spec/ARCHITECTURE.md` section 8
//! states the rule this answers, "migrations run forward on open", which
//! only makes sense read together with its neighbor, that an engine on an
//! older methodology "refuses to open a product on a newer one without an
//! explicit upgrade flow"; the schema version is this crate's instance of
//! that same rule.
//!
//! # The vacuous-truth trap this module was written against
//!
//! An empty [`crate::db::MIGRATIONS`] would make "every migration in the
//! compiled-in list has a row in `_ori_migrations`" true of every database,
//! including a brand new one, for the same reason "all of my zero coins are
//! gold" is true of an empty pocket: the claim quantifies over nothing.
//! `validate_migrations` is called first, before any database is even
//! opened, and refuses with [`crate::db::DbError::NoMigrationsEmbedded`]
//! whenever [`crate::db::MIGRATIONS`] (or, in
//! `tests::ori_t_0024_an_empty_compiled_in_migration_set_is_refused_not_silently_accepted`,
//! an injected empty slice standing in for a broken reader) is empty, rather
//! than letting the rest of `open` run to a vacuous, undeserved success.
//!
//! # Isolation between two products
//!
//! Mechanically, isolation is that `ProductDb::open` addresses a product by a
//! directory, `<products_root>/<product_id>/`, and never by anything an
//! operator could point at the wrong place without also renaming the
//! directory. Two products can never share a `product.sqlite` connection or a
//! `lock` connection simply because two different `product_id` values compute
//! two different paths. The one misconfiguration this cannot prevent by
//! construction alone, an operator or a symlink pointing product B's
//! directory at product A's files, is caught by a stamp rather than by the
//! path: the first `open` of a fresh `product.sqlite` (and, separately, of a
//! fresh `lock`) writes the `product_id` it was opened with into a singleton
//! table, and every later `open` compares its `product_id` argument against
//! that stamp, refusing with [`crate::db::DbError::ProductMismatch`] on a
//! disagreement.
//! `AICD §26`'s "A product's coder cannot see another product's code" is
//! written about the fleet, not the database, but the database-level
//! statement of the same rule is what a coder's own crate rests on being
//! true.
//!
//! Must not: contain business rules (`spec/LLD.md` section 2). Nothing here
//! reads or interprets a payload; `migrations/0001_init.sql` explains why
//! `payload` carries no `json_valid` check either, even though
//! `spec/DATA_MODEL.md` section 2 calls the column "payload (json)": the
//! sibling `event_log.rs` deliberately stores it as opaque, non-empty text,
//! and a `CHECK` here would have silently disagreed with an insert that
//! module's own Rust-level validation already allows.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ori_core::error::MethodologyRef;
use ori_core::types::Timestamp;
use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};

/// One embedded schema migration.
///
/// No methodology section applies: this is the value a migration is, not a
/// rule about what one may do. `spec/LLD.md` section 2 names "migrations" as
/// something this crate owns without fixing their shape; `version`, `name`
/// and `sql` are what [`ProductDb::open`]'s ordering, recording and
/// edited-after-run checks need, and no more.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    /// Starts at 1, one higher than the previous entry in [`MIGRATIONS`],
    /// with no gap ever admitted.
    pub version: u32,
    /// A short label, recorded alongside the SQL so `_ori_migrations` reads
    /// as more than a list of numbers.
    pub name: &'static str,
    /// The exact SQL this migration runs, embedded at compile time.
    pub sql: &'static str,
}

/// Every migration this binary knows how to apply, in the only order any of
/// them may ever run.
///
/// Adding a migration means appending one entry here whose `version` is one
/// higher than the last, and a new file under `crates/ori-store/migrations/`
/// that this entry's `sql` reads with [`include_str!`]. Removing or
/// reordering an entry here is editing a migration that may already have run
/// somewhere, which `apply_pending` exists to refuse.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "projections",
        sql: include_str!("../migrations/0002_projections.sql"),
    },
];

/// A refusal from this module: `ProductDb::open` failing to become a
/// [`ProductDb`], or a step inside it.
///
/// `spec/CONVENTIONS.md` names `thiserror` as the house error convention;
/// ruling R20 in `ops/rulings.md` defers it and states what stands until
/// then, "a hand-written `Display` and `std::error::Error` implementation
/// [...], with a comment naming the conversion", which is what this is:
/// each [`Display`](fmt::Display) arm below becomes a `#[error("...")]`
/// attribute on its variant when that lands.
#[derive(Debug)]
#[non_exhaustive]
pub enum DbError {
    /// A filesystem operation failed while establishing the product
    /// directory's layout.
    Io {
        /// The path the operation was against.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// A second `ProductDb` tried to open a `lock` another live `ProductDb`
    /// already holds (planted-defect 1 of the ticket's list).
    Locked {
        /// The `lock` file that was already held.
        path: PathBuf,
    },
    /// The `product_id` an `open` was called with disagrees with the one
    /// stamped into the file it opened (planted-defect 5).
    ProductMismatch {
        /// The file whose stamp disagreed: `product.sqlite` or `lock`.
        path: PathBuf,
        /// The `product_id` this `open` was called with.
        expected: String,
        /// The `product_id` already stamped there.
        found: String,
    },
    /// [`MIGRATIONS`] (or an injected list standing in for it in a test) is
    /// empty, which would make "every migration has run" vacuously true of
    /// any database (planted-defect 8, the one the ticket names as the one a
    /// careless implementation gets wrong).
    NoMigrationsEmbedded,
    /// The versions recorded in `_ori_migrations` are not the contiguous run
    /// `1..=N` a correct history always is: either a gap (an earlier version
    /// never ran) or [`MIGRATIONS`] itself is malformed (planted-defect 3).
    MigrationGap {
        /// The version a contiguous history would have here.
        expected_next: u32,
        /// The version that was actually next.
        found: u32,
    },
    /// A migration already recorded as applied no longer matches the SQL
    /// text this binary embeds for that version: it was edited after it ran
    /// somewhere, which is never allowed.
    MigrationEdited {
        /// The version whose recorded text disagreed.
        version: u32,
        /// Its name, for a human reading the refusal.
        name: &'static str,
    },
    /// `_ori_migrations` records a version higher than [`MIGRATIONS`]
    /// carries: this database was written by a newer binary (planted-defect
    /// 4, "the one that eats data").
    SchemaTooNew {
        /// The highest version the database has recorded.
        database_version: u32,
        /// The highest version this binary carries.
        binary_version: u32,
    },
    /// A `rusqlite` call failed in a way none of the above names more
    /// specifically.
    Sqlite {
        /// What was being attempted.
        context: String,
        /// The underlying error.
        source: rusqlite::Error,
    },
}

impl DbError {
    /// Wraps a `rusqlite` failure with what was being attempted, for the one
    /// variant that is not any of this module's named refusals.
    fn sqlite(context: impl Into<String>, source: rusqlite::Error) -> Self {
        Self::Sqlite {
            context: context.into(),
            source,
        }
    }

    /// The methodology section this refusal rests on.
    ///
    /// Every variant cites `AICD §8`, this crate's own section
    /// (`crates/ori-store/src/lib.rs`'s module doc, "AICD §8, the memory
    /// layer 3 store"), because every one of them is this store refusing to
    /// stand in for durable state it cannot vouch for.
    /// [`DbError::ProductMismatch`] additionally rests on `AICD §26`,
    /// "Operating several products with one team": "A product's coder cannot
    /// see another product's code" is the fleet-level statement of the
    /// isolation this variant enforces at the file level.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        MethodologyRef {
            section: 8,
            subsection: None,
        }
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Self::Locked { path } => {
                write!(
                    f,
                    "{} is held by another writer; refusing a second one",
                    path.display()
                )
            }
            Self::ProductMismatch {
                path,
                expected,
                found,
            } => write!(
                f,
                "{} was stamped for product '{found}', not '{expected}'; refusing to open it as the wrong product",
                path.display()
            ),
            Self::NoMigrationsEmbedded => write!(
                f,
                "no migrations are embedded in this binary; refusing to treat that as \"every migration has run\""
            ),
            Self::MigrationGap {
                expected_next,
                found,
            } => write!(
                f,
                "expected migration version {expected_next} next, found {found}; the migration history is not contiguous"
            ),
            Self::MigrationEdited { version, name } => write!(
                f,
                "migration {version} ('{name}') no longer matches the text that ran; migrations may never be edited after they run"
            ),
            Self::SchemaTooNew {
                database_version,
                binary_version,
            } => write!(
                f,
                "database schema version {database_version} is newer than this binary's {binary_version}; refusing to open it"
            ),
            Self::Sqlite { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::Locked { .. }
            | Self::ProductMismatch { .. }
            | Self::NoMigrationsEmbedded
            | Self::MigrationGap { .. }
            | Self::MigrationEdited { .. }
            | Self::SchemaTooNew { .. } => None,
        }
    }
}

/// Whether a `rusqlite` failure is SQLite reporting the file as already
/// locked by another connection, the specific case [`DbError::Locked`]
/// names rather than falling back to [`DbError::Sqlite`].
fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == ErrorCode::DatabaseBusy || inner.code == ErrorCode::DatabaseLocked
    )
}

/// The directory one product's on-disk state lives under: `<products_root>/
/// <product_id>/`, per `spec/LLD.md` section 6.
///
/// No methodology section applies. The rule is `spec/LLD.md` section 6's
/// layout diagram; this function states the one line of it that is a path
/// computation rather than a set of sibling paths.
#[must_use]
pub fn product_dir(products_root: &Path, product_id: &str) -> PathBuf {
    products_root.join(product_id)
}

/// An open, locked, migrated connection to one product's `product.sqlite`,
/// per `spec/LLD.md` section 6: AICD §8.
///
/// Holds the `product.sqlite` connection and the `lock` connection that
/// keeps this the only writer for as long as it lives; both close, and the
/// lock releases, on [`Drop`]. `spec/LLD.md` section 5 gives the event log a
/// single writer task; that task is meant to hold exactly one `ProductDb` for
/// as long as it runs.
#[derive(Debug)]
pub struct ProductDb {
    conn: Connection,
    // Held only for its `Drop`: closing this connection is what releases the
    // OS-level exclusive lock `acquire_lock` took. Never read past
    // construction, which is why it carries the leading underscore rather
    // than a `#[allow(dead_code)]`; the module doc above states what closing
    // it, gracefully or by a crash, actually guarantees.
    _lock: Connection,
    product_id: String,
    dir: PathBuf,
}

impl ProductDb {
    /// Opens (creating if absent) the product directory at
    /// `<products_root>/<product_id>/`, acquires its `lock`, and migrates
    /// `product.sqlite` to this binary's current schema version.
    ///
    /// `opened_at` is the time to stamp `_ori_product`, `_ori_lock_meta` and
    /// any migration this call applies with: this crate does IO but this
    /// call does not read a clock itself, so that migrating and re-opening a
    /// database in a test never depends on when the test happened to run.
    ///
    /// # Errors
    ///
    /// See [`DbError`]'s variants; the module doc above states which of the
    /// ticket's nine planted defects each one is for.
    pub fn open(
        products_root: &Path,
        product_id: &str,
        opened_at: Timestamp,
    ) -> Result<Self, DbError> {
        Self::open_with_migrations(products_root, product_id, opened_at, MIGRATIONS)
    }

    /// [`ProductDb::open`]'s body, parameterized over the migration list so a
    /// test can inject an empty one (defect 8) without editing [`MIGRATIONS`]
    /// itself, which every other product this binary ever opens must keep
    /// seeing whole.
    fn open_with_migrations(
        products_root: &Path,
        product_id: &str,
        opened_at: Timestamp,
        migrations: &[Migration],
    ) -> Result<Self, DbError> {
        validate_migrations(migrations)?;

        let dir = product_dir(products_root, product_id);
        ensure_dir(&dir)?;
        ensure_dir(&dir.join("index"))?;
        ensure_dir(&dir.join("sessions"))?;
        ensure_dir(&dir.join("evidence"))?;

        let lock_path = dir.join("lock");
        let lock = acquire_lock(&lock_path, product_id, opened_at)?;

        let db_path = dir.join("product.sqlite");
        let mut conn = open_connection(&db_path)?;
        apply_pending(&mut conn, migrations, opened_at)?;
        verify_or_stamp_product(&conn, &db_path, product_id, opened_at)?;

        Ok(Self {
            conn,
            _lock: lock,
            product_id: product_id.to_owned(),
            dir,
        })
    }

    /// The `product.sqlite` connection, for the single writer task to run
    /// its own statements against (`event_log.rs`'s append, and the
    /// projections built from it).
    pub fn connection(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// The `product_id` this database was opened for and has stamped.
    #[must_use]
    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    /// The product directory this database was opened from.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The highest migration version applied, read back from
    /// `_ori_migrations` rather than cached, so it always reflects what is
    /// actually on disk.
    ///
    /// # Errors
    ///
    /// Only on a `rusqlite` failure reading the table; a database this call
    /// can be made against has already passed [`ProductDb::open`]'s checks.
    pub fn schema_version(&self) -> Result<u32, DbError> {
        recorded_max_version(&self.conn)
    }
}

/// Refuses [`DbError::NoMigrationsEmbedded`] for an empty list (the
/// vacuous-truth trap the module doc names), and [`DbError::MigrationGap`]
/// for a list whose versions are not exactly `1..=len`, ascending, with no
/// gap: the shape [`MIGRATIONS`] itself must always have.
fn validate_migrations(migrations: &[Migration]) -> Result<(), DbError> {
    if migrations.is_empty() {
        return Err(DbError::NoMigrationsEmbedded);
    }
    for (index, migration) in migrations.iter().enumerate() {
        let expected = (index + 1) as u32;
        if migration.version != expected {
            return Err(DbError::MigrationGap {
                expected_next: expected,
                found: migration.version,
            });
        }
    }
    Ok(())
}

/// Creates `path` and any missing parent, tolerating "already exists".
fn ensure_dir(path: &Path) -> Result<(), DbError> {
    match fs::create_dir_all(path) {
        Ok(()) => Ok(()),
        Err(source) => Err(DbError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Opens `path` as its own SQLite file, sets `locking_mode = EXCLUSIVE`, and
/// begins (and commits) an exclusive transaction that stamps or verifies the
/// product this lock belongs to, so that the OS-level exclusive lock SQLite
/// takes for that transaction is never released again before this
/// connection's own [`Drop`]. The module doc above states what this does and
/// does not guarantee.
fn acquire_lock(
    path: &Path,
    product_id: &str,
    opened_at: Timestamp,
) -> Result<Connection, DbError> {
    let mut conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(source) => return Err(DbError::sqlite(format!("open {}", path.display()), source)),
    };
    conn.busy_timeout(Duration::from_millis(0))
        .map_err(|source| DbError::sqlite("set busy_timeout on lock", source))?;
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")
        .map_err(|source| DbError::sqlite("set locking_mode on lock", source))?;

    let tx = match conn.transaction_with_behavior(TransactionBehavior::Exclusive) {
        Ok(tx) => tx,
        Err(source) if is_busy(&source) => {
            return Err(DbError::Locked {
                path: path.to_owned(),
            });
        }
        Err(source) => {
            return Err(DbError::sqlite(
                format!("acquire exclusive lock on {}", path.display()),
                source,
            ));
        }
    };

    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS _ori_lock_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            product_id TEXT NOT NULL,
            opened_at INTEGER NOT NULL
        );",
    )
    .map_err(|source| DbError::sqlite("create _ori_lock_meta", source))?;

    let existing: Option<String> = tx
        .query_row(
            "SELECT product_id FROM _ori_lock_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| DbError::sqlite("read _ori_lock_meta", source))?;

    match existing {
        Some(found) if found != product_id => {
            return Err(DbError::ProductMismatch {
                path: path.to_owned(),
                expected: product_id.to_owned(),
                found,
            });
        }
        Some(_) => {
            tx.execute(
                "UPDATE _ori_lock_meta SET opened_at = ?1 WHERE id = 1",
                params![opened_at.millis()],
            )
            .map_err(|source| DbError::sqlite("update _ori_lock_meta", source))?;
        }
        None => {
            tx.execute(
                "INSERT INTO _ori_lock_meta (id, product_id, opened_at) VALUES (1, ?1, ?2)",
                params![product_id, opened_at.millis()],
            )
            .map_err(|source| DbError::sqlite("insert _ori_lock_meta", source))?;
        }
    }

    tx.commit()
        .map_err(|source| DbError::sqlite("commit lock stamp", source))?;

    Ok(conn)
}

/// Opens `path` as `product.sqlite`, in WAL mode (`spec/LLD.md` section 5,
/// "Reads are lock-free snapshots") with foreign keys enforced.
fn open_connection(path: &Path) -> Result<Connection, DbError> {
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(source) => return Err(DbError::sqlite(format!("open {}", path.display()), source)),
    };
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| DbError::sqlite("set journal_mode", source))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| DbError::sqlite("set foreign_keys", source))?;
    Ok(conn)
}

/// Creates `_ori_migrations` if this is a fresh database.
fn ensure_migrations_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _ori_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sql TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )
    .map_err(|source| DbError::sqlite("create _ori_migrations", source))
}

/// Every row of `_ori_migrations`, ordered by version ascending.
fn recorded_migrations(conn: &Connection) -> Result<Vec<(u32, String, String)>, DbError> {
    let mut statement = conn
        .prepare("SELECT version, name, sql FROM _ori_migrations ORDER BY version ASC")
        .map_err(|source| DbError::sqlite("prepare read of _ori_migrations", source))?;
    let rows = statement
        .query_map([], |row| {
            let version: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let sql: String = row.get(2)?;
            Ok((version as u32, name, sql))
        })
        .map_err(|source| DbError::sqlite("read _ori_migrations", source))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|source| DbError::sqlite("read a row of _ori_migrations", source))?);
    }
    Ok(out)
}

/// The highest version recorded in `_ori_migrations`, 0 for a fresh
/// database.
fn recorded_max_version(conn: &Connection) -> Result<u32, DbError> {
    Ok(recorded_migrations(conn)?.len() as u32)
}

/// Verifies the migration history already on `conn` is contiguous and
/// unedited, refuses a database newer than `migrations` knows about, and
/// applies whatever is left pending. Returns the schema version after this
/// call, which is `migrations.len()` on success.
fn apply_pending(
    conn: &mut Connection,
    migrations: &[Migration],
    applied_at: Timestamp,
) -> Result<u32, DbError> {
    ensure_migrations_table(conn)?;
    let recorded = recorded_migrations(conn)?;

    for (index, (version, _, _)) in recorded.iter().enumerate() {
        let expected = (index + 1) as u32;
        if *version != expected {
            return Err(DbError::MigrationGap {
                expected_next: expected,
                found: *version,
            });
        }
    }

    let database_version = recorded.len() as u32;
    let binary_version = migrations.len() as u32;
    if database_version > binary_version {
        return Err(DbError::SchemaTooNew {
            database_version,
            binary_version,
        });
    }

    for (version, name, sql) in &recorded {
        let embedded = &migrations[(*version - 1) as usize];
        if embedded.sql != sql.as_str() || embedded.name != name.as_str() {
            return Err(DbError::MigrationEdited {
                version: *version,
                name: embedded.name,
            });
        }
    }

    for migration in &migrations[database_version as usize..] {
        let tx = conn.transaction().map_err(|source| {
            DbError::sqlite(format!("begin migration {}", migration.version), source)
        })?;
        tx.execute_batch(migration.sql).map_err(|source| {
            DbError::sqlite(format!("apply migration {}", migration.version), source)
        })?;
        tx.execute(
            "INSERT INTO _ori_migrations (version, name, sql, applied_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                migration.version,
                migration.name,
                migration.sql,
                applied_at.millis()
            ],
        )
        .map_err(|source| {
            DbError::sqlite(format!("record migration {}", migration.version), source)
        })?;
        tx.commit().map_err(|source| {
            DbError::sqlite(format!("commit migration {}", migration.version), source)
        })?;
    }

    Ok(binary_version)
}

/// Stamps `_ori_product` with `product_id` on a fresh database, or refuses
/// with [`DbError::ProductMismatch`] if a different `product_id` is already
/// stamped there. Called after [`apply_pending`], since `_ori_product` is
/// created by migration 1.
fn verify_or_stamp_product(
    conn: &Connection,
    path: &Path,
    product_id: &str,
    opened_at: Timestamp,
) -> Result<(), DbError> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT product_id FROM _ori_product WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| DbError::sqlite("read _ori_product", source))?;

    match existing {
        Some(found) if found != product_id => Err(DbError::ProductMismatch {
            path: path.to_owned(),
            expected: product_id.to_owned(),
            found,
        }),
        Some(_) => Ok(()),
        None => {
            conn.execute(
                "INSERT INTO _ori_product (id, product_id, created_at) VALUES (1, ?1, ?2)",
                params![product_id, opened_at.millis()],
            )
            .map_err(|source| DbError::sqlite("insert _ori_product", source))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // -------------------------------------------------------------------
    // Scratch layout: `crates/ori-runtime/src/session.rs`'s `absent` and
    // `crates/ori-runtime/src/worktree.rs`'s `Scratch` guard, copied for
    // this crate's own temporary product roots, rooted at
    // `std::env::temp_dir()` rather than `/`, so a fixture path is absolute
    // on every platform CI builds for.
    // -------------------------------------------------------------------

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
                "ori-t-0024-{label}-{}-{unique}",
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

    // -------------------------------------------------------------------
    // ORI-P1-036 (this module's share of it: two databases, two lock files)
    // -------------------------------------------------------------------

    /// "Two databases": opening two different products under the same
    /// `products_root` produces two independent `product.sqlite` files,
    /// neither able to see the other's rows, because they are simply
    /// different files.
    #[test]
    fn ori_p1_036_two_products_open_as_two_independent_databases() {
        let scratch = Scratch::new("two-products");
        let mut a =
            ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("product A opens");
        let mut b =
            ProductDb::open(&scratch.path, "PRODUCT-B", at(1_000)).expect("product B opens");

        a.connection()
            .execute(
                "INSERT INTO events (product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev) \
                 VALUES ('PRODUCT-A', 1000, 'system', NULL, 'ticket.filed', 'T-1', '{}', 'GENESIS')",
                [],
            )
            .expect("an insert into A's events");

        let count_in_b: i64 = b
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .expect("count B's events");
        assert_eq!(
            count_in_b, 0,
            "a ticket recorded in A's database must be invisible to B's, because they are different files"
        );
    }

    /// "Two lock files": each product directory gets its own `lock`, so
    /// holding product A's lock never contends with opening product B.
    #[test]
    fn ori_p1_036_two_products_have_two_independent_lock_files() {
        let scratch = Scratch::new("two-locks");
        let a = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
            .expect("product A opens and locks");
        let b = ProductDb::open(&scratch.path, "PRODUCT-B", at(1_000));
        assert!(
            b.is_ok(),
            "product B's lock is a different file from product A's, so holding A's must never block B: {:?}",
            b.err()
        );
        assert_ne!(
            a.dir(),
            b.expect("checked above").dir(),
            "two products must resolve to two different directories, and so two different lock files"
        );
    }

    // -------------------------------------------------------------------
    // ORI-T-0107: the registry's own size is pinned, not only implied
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0107_migrations_pins_exactly_two_entries_versions_one_and_two() {
        // Every other test in this file that used to hardcode `1` now reads
        // `MIGRATIONS.len()` instead, which makes each of them agree with
        // MIGRATIONS automatically but makes none of them assert what the
        // count itself should be: a MIGRATIONS entry silently added or
        // removed would still leave every one of those tests passing,
        // agreeing with a registry that has quietly grown or shrunk. This
        // test is the independent check, the same role
        // `crate::projections::PROJECTION_COUNT` plays for the projection
        // registry: declared here, away from MIGRATIONS's own body, so
        // deleting or duplicating an entry fails this test rather than only
        // being implied by tests that read the count dynamically.
        assert_eq!(MIGRATIONS.len(), 2);
        assert_eq!(MIGRATIONS[0].version, 1);
        assert_eq!(MIGRATIONS[1].version, 2);

        let scratch = Scratch::new("migrations-pinned");
        let mut db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
            .expect("a fresh product opens and applies every migration");
        let recorded = recorded_migrations(db.connection()).expect("read _ori_migrations back");
        let versions: Vec<u32> = recorded.iter().map(|(version, _, _)| *version).collect();
        assert_eq!(
            versions,
            vec![1, 2],
            "a fresh product's _ori_migrations must record exactly the versions MIGRATIONS carries"
        );
    }

    // -------------------------------------------------------------------
    // ORI-T-0024: the happy path (planted-defect 9, "unchanged, must pass",
    // read as "the ordinary case must still work")
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_a_fresh_product_directory_opens_and_lays_out_the_whole_contract() {
        let scratch = Scratch::new("fresh-open");
        let db =
            ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("a fresh product opens");

        assert_eq!(db.product_id(), "PRODUCT-A");
        assert_eq!(
            db.schema_version().expect("read schema version"),
            MIGRATIONS.len() as u32
        );
        for sub in ["index", "sessions", "evidence"] {
            assert!(
                db.dir().join(sub).is_dir(),
                "spec/LLD.md section 6 names {sub}/ in the layout every open establishes"
            );
        }
        assert!(db.dir().join("product.sqlite").is_file());
        assert!(db.dir().join("lock").is_file());
    }

    #[test]
    fn ori_t_0024_reopening_after_a_clean_close_succeeds_and_keeps_the_data() {
        let scratch = Scratch::new("reopen-clean");
        {
            let mut db =
                ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("first open");
            db.connection()
                .execute(
                    "INSERT INTO events (product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev) \
                     VALUES ('PRODUCT-A', 1000, 'system', NULL, 'ticket.filed', 'T-1', '{}', 'GENESIS')",
                    [],
                )
                .expect("an insert");
        } // dropped: lock released

        let mut reopened = ProductDb::open(&scratch.path, "PRODUCT-A", at(2_000))
            .expect("reopen after a clean close");
        let count: i64 = reopened
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .expect("count events");
        assert_eq!(
            count, 1,
            "product.sqlite is durable across an open/close cycle"
        );
    }

    // -------------------------------------------------------------------
    // Planted defect 1: a second writer refused
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_a_second_writer_is_refused_while_the_first_is_open() {
        let scratch = Scratch::new("second-writer");
        let _first = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
            .expect("first open holds the lock");

        let second = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000));
        match second {
            Err(DbError::Locked { .. }) => {}
            other => {
                panic!("expected DbError::Locked while the first writer is open, got {other:?}")
            }
        }
    }

    #[test]
    fn ori_t_0024_the_lock_releases_the_moment_the_first_writer_drops() {
        let scratch = Scratch::new("lock-release");
        let first = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("first open");
        drop(first);

        let second = ProductDb::open(&scratch.path, "PRODUCT-A", at(2_000));
        assert!(
            second.is_ok(),
            "dropping the first ProductDb must release the lock immediately: {:?}",
            second.err()
        );
    }

    // -------------------------------------------------------------------
    // Planted defect 2: a migration applied twice is idempotent
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_reopening_an_already_migrated_database_is_idempotent() {
        let scratch = Scratch::new("idempotent-reopen");
        for round in 0..5 {
            let db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000 + round))
                .unwrap_or_else(|e| panic!("open round {round} must succeed: {e}"));
            assert_eq!(
                db.schema_version().expect("schema version"),
                MIGRATIONS.len() as u32
            );
            drop(db);
        }
    }

    #[test]
    fn ori_t_0024_apply_pending_run_twice_against_the_same_connection_does_not_reapply() {
        let scratch = Scratch::new("apply-pending-twice");
        let db_path = scratch.path.join("direct.sqlite");
        let mut conn = open_connection(&db_path).expect("open a bare connection");
        let first = apply_pending(&mut conn, MIGRATIONS, at(1_000)).expect("first apply");
        let second = apply_pending(&mut conn, MIGRATIONS, at(2_000))
            .expect("second apply is a no-op, not an error");
        assert_eq!(first, second);
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM _ori_migrations", [], |row| row.get(0))
            .expect("count _ori_migrations");
        assert_eq!(
            applied,
            MIGRATIONS.len() as i64,
            "each migration must have exactly one row, not one per apply_pending call"
        );
    }

    // -------------------------------------------------------------------
    // Planted defect 3: migrations applied (recorded) out of order
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_a_gap_in_recorded_migrations_is_refused() {
        let scratch = Scratch::new("migration-gap");
        let db_path = scratch.path.join("direct.sqlite");
        let mut conn = open_connection(&db_path).expect("open a bare connection");
        ensure_migrations_table(&conn).expect("bootstrap _ori_migrations");
        // Insert a row for version 2 without version 1 ever having run: the
        // out-of-order case, simulated directly since MIGRATIONS today has
        // only one real entry to reorder.
        conn.execute(
            "INSERT INTO _ori_migrations (version, name, sql, applied_at) VALUES (2, 'phantom', '-- x', 1000)",
            [],
        )
        .expect("insert the corrupt row");

        match apply_pending(&mut conn, MIGRATIONS, at(2_000)) {
            Err(DbError::MigrationGap {
                expected_next,
                found,
            }) => {
                assert_eq!(expected_next, 1);
                assert_eq!(found, 2);
            }
            other => panic!("expected DbError::MigrationGap, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------
    // Planted defect 4: a database newer than the binary
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_a_database_newer_than_the_binary_is_refused_not_silently_used() {
        let scratch = Scratch::new("schema-too-new");
        {
            let db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
                .expect("open at today's version");
            drop(db);
        }

        // Simulate a future binary having run one migration beyond what this
        // binary's MIGRATIONS carries: the version is derived from
        // MIGRATIONS.len(), never hardcoded, so a third migration does not
        // reopen this test the way it reopened the other three that used to
        // hardcode the count (ORI-T-0107).
        let future_version = MIGRATIONS.len() as u32 + 1;
        let db_path = scratch.path.join("PRODUCT-A").join("product.sqlite");
        let conn = open_connection(&db_path).expect("open the underlying file directly");
        conn.execute(
            "INSERT INTO _ori_migrations (version, name, sql, applied_at) VALUES (?1, 'from_the_future', '-- x', 1500)",
            params![future_version],
        )
        .expect("insert the future row");
        drop(conn);

        match ProductDb::open(&scratch.path, "PRODUCT-A", at(2_000)) {
            Err(DbError::SchemaTooNew {
                database_version,
                binary_version,
            }) => {
                assert_eq!(database_version, future_version);
                assert_eq!(binary_version, MIGRATIONS.len() as u32);
            }
            other => panic!("expected DbError::SchemaTooNew, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------
    // A migration edited after it ran
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_a_migration_edited_after_it_ran_is_refused() {
        let scratch = Scratch::new("migration-edited");
        {
            let db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000))
                .expect("open at today's version");
            drop(db);
        }

        let db_path = scratch.path.join("PRODUCT-A").join("product.sqlite");
        let conn = open_connection(&db_path).expect("open the underlying file directly");
        conn.execute(
            "UPDATE _ori_migrations SET sql = '-- tampered' WHERE version = 1",
            [],
        )
        .expect("tamper with the recorded text");
        drop(conn);

        match ProductDb::open(&scratch.path, "PRODUCT-A", at(2_000)) {
            Err(DbError::MigrationEdited { version, .. }) => assert_eq!(version, 1),
            other => panic!("expected DbError::MigrationEdited, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------
    // Planted defect 5: two products sharing one file, or one lock
    // -------------------------------------------------------------------

    // `ProductDb::open`'s own path computation (`product_dir`, `products_root`
    // joined with `product_id`) already makes this unreachable through the
    // public API by construction: two different `product_id` arguments always
    // compute two different directories, so there is no sequence of `open`
    // calls that points two different product ids at one directory. That is
    // the mechanical isolation the module doc's "Isolation between two
    // products" section describes; what these two tests prove is the second
    // layer behind it, for the one misconfiguration construction alone cannot
    // rule out (an operator's registry, or a symlink, pointing two ids at one
    // physical file): the stamp each file carries, exercised directly against
    // the two functions that read and write it, the same way
    // `tests::ori_t_0024_a_gap_in_recorded_migrations_is_refused` exercises
    // `apply_pending` directly rather than only through `ProductDb::open`.

    #[test]
    fn ori_t_0024_two_products_cannot_share_one_product_sqlite_file() {
        let scratch = Scratch::new("shared-file");
        let db_path = scratch.path.join("shared.sqlite");
        let mut conn = open_connection(&db_path).expect("open a bare connection");
        apply_pending(&mut conn, MIGRATIONS, at(1_000)).expect("migrate");
        verify_or_stamp_product(&conn, &db_path, "product-A", at(1_000)).expect("first stamp");

        match verify_or_stamp_product(&conn, &db_path, "product-B", at(2_000)) {
            Err(DbError::ProductMismatch {
                expected, found, ..
            }) => {
                assert_eq!(expected, "product-B");
                assert_eq!(found, "product-A");
            }
            other => panic!("expected DbError::ProductMismatch, got {other:?}"),
        }
    }

    #[test]
    fn ori_t_0024_two_products_cannot_share_one_lock_file() {
        let scratch = Scratch::new("shared-lock");
        let lock_path = scratch.path.join("shared-lock-file");
        let first = acquire_lock(&lock_path, "product-A", at(1_000)).expect("first stamp and lock");
        drop(first); // released, so this isolates the stamp check from defect 1's lock check

        match acquire_lock(&lock_path, "product-B", at(2_000)) {
            Err(DbError::ProductMismatch {
                expected, found, ..
            }) => {
                assert_eq!(expected, "product-B");
                assert_eq!(found, "product-A");
            }
            other => panic!("expected DbError::ProductMismatch, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------
    // Planted defect 6 and 7: the suite itself is not vacuous either way
    // -------------------------------------------------------------------
    //
    // These two are proved in the scratchpad against a copy of this crate
    // with `ProductDb::open_with_migrations` mutated to always `Ok` (defect
    // 6) or always `Err` (defect 7), per the ticket's instruction to plant
    // in a copy rather than the repository. What makes each mutation
    // catchable, stated as the tests that catch it:
    //
    // - Always `Ok` is caught by every refusal test above
    //   (`tests::ori_t_0024_a_second_writer_is_refused_while_the_first_is_open`,
    //   `tests::ori_t_0024_a_gap_in_recorded_migrations_is_refused`,
    //   `tests::ori_t_0024_a_database_newer_than_the_binary_is_refused_not_silently_used`,
    //   `tests::ori_t_0024_two_products_cannot_share_one_product_sqlite_file`,
    //   and
    //   others), each of which asserts a specific `Err` variant and fails
    //   loudly on an `Ok` it did not expect.
    // - Always `Err` is caught by every happy-path test above
    //   (`tests::ori_t_0024_a_fresh_product_directory_opens_and_lays_out_the_whole_contract`,
    //   `tests::ori_t_0024_reopening_after_a_clean_close_succeeds_and_keeps_the_data`,
    //   `tests::ori_p1_036_two_products_open_as_two_independent_databases`),
    //   each of which calls `.expect(...)` on an `open` that must succeed.
    //
    // Neither direction is a hole: a suite of only-refusal tests would miss
    // defect 7, and a suite of only-happy-path tests would miss defect 6.
    // This module has both.

    // -------------------------------------------------------------------
    // Planted defect 8: an empty migration set must fail, not vacuously pass
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_an_empty_compiled_in_migration_set_is_refused_not_silently_accepted() {
        let scratch = Scratch::new("empty-migrations");
        let empty: &[Migration] = &[];
        match ProductDb::open_with_migrations(&scratch.path, "PRODUCT-A", at(1_000), empty) {
            Err(DbError::NoMigrationsEmbedded) => {}
            other => panic!(
                "an empty migration list must be refused, not treated as \"every migration has run\": {other:?}"
            ),
        }
    }

    #[test]
    fn ori_t_0024_validate_migrations_refuses_a_malformed_compiled_in_list() {
        let malformed: &[Migration] = &[
            Migration {
                version: 1,
                name: "one",
                sql: "-- x",
            },
            Migration {
                version: 3,
                name: "three",
                sql: "-- y",
            },
        ];
        match validate_migrations(malformed) {
            Err(DbError::MigrationGap {
                expected_next,
                found,
            }) => {
                assert_eq!(expected_next, 2);
                assert_eq!(found, 3);
            }
            other => panic!("expected DbError::MigrationGap, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------
    // The append-only guarantee the schema itself enforces
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0024_the_schema_itself_refuses_updating_an_event() {
        let scratch = Scratch::new("no-update");
        let mut db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("open");
        db.connection()
            .execute(
                "INSERT INTO events (product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev) \
                 VALUES ('PRODUCT-A', 1000, 'system', NULL, 'ticket.filed', 'T-1', '{}', 'GENESIS')",
                [],
            )
            .expect("an insert");
        let result = db.connection().execute(
            "UPDATE events SET kind = 'ticket.validated' WHERE seq = 1",
            [],
        );
        assert!(
            result.is_err(),
            "the events_no_update trigger must refuse this"
        );
    }

    #[test]
    fn ori_t_0024_the_schema_itself_refuses_deleting_an_event() {
        let scratch = Scratch::new("no-delete");
        let mut db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("open");
        db.connection()
            .execute(
                "INSERT INTO events (product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev) \
                 VALUES ('PRODUCT-A', 1000, 'system', NULL, 'ticket.filed', 'T-1', '{}', 'GENESIS')",
                [],
            )
            .expect("an insert");
        let result = db
            .connection()
            .execute("DELETE FROM events WHERE seq = 1", []);
        assert!(
            result.is_err(),
            "the events_no_delete trigger must refuse this"
        );
    }

    // -------------------------------------------------------------------
    // Property-based (spec/TESTING.md section 1): invariants across a
    // generated sequence of opens, rather than the one sequence a unit test
    // enumerates by hand.
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Any number of sequential opens (each closing before the next)
            /// against one product directory always lands on schema version
            /// 1 and never errors: idempotency generalized from "twice" to
            /// "N times", `spec/TESTING.md` section 1's "invariants hold for
            /// generated event sequences" read as a generated sequence of
            /// opens rather than of domain events.
            #[test]
            fn ori_t_0024_any_number_of_sequential_opens_stays_idempotent(rounds in 1u32..12) {
                let scratch = Scratch::new("proptest-idempotent");
                for round in 0..rounds {
                    let db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000 + i64::from(round)))
                        .unwrap_or_else(|e| panic!("round {round} must open: {e}"));
                    prop_assert_eq!(
                        db.schema_version().expect("schema version"),
                        MIGRATIONS.len() as u32
                    );
                }
            }

            /// Any recorded schema version above what the binary carries is
            /// refused as [`DbError::SchemaTooNew`], for any excess amount,
            /// not only the one value a handwritten test picks.
            #[test]
            fn ori_t_0024_any_schema_version_above_the_binary_is_refused(excess in 1u32..200) {
                let scratch = Scratch::new("proptest-too-new");
                {
                    let db = ProductDb::open(&scratch.path, "PRODUCT-A", at(1_000)).expect("open at today's version");
                    drop(db);
                }
                let db_path = scratch.path.join("PRODUCT-A").join("product.sqlite");
                let conn = open_connection(&db_path).expect("open directly");
                let binary_version = MIGRATIONS.len() as u32;
                let future_version = binary_version + excess;
                // A contiguous run from binary_version + 1 up to future_version,
                // the shape a genuinely newer binary would actually have left
                // behind (it too applies migrations one version at a time, with
                // no gap): a single row at future_version alone, with nothing
                // recorded in between, is not a state any binary following this
                // module's own rules could produce, and correctly trips the gap
                // check instead, as `tests::ori_t_0024_a_gap_in_recorded_migrations_is_refused`
                // proves for that separate case.
                for version in (binary_version + 1)..=future_version {
                    conn.execute(
                        "INSERT INTO _ori_migrations (version, name, sql, applied_at) VALUES (?1, 'future', '-- x', 1500)",
                        params![version],
                    )
                    .expect("insert a future row");
                }
                drop(conn);

                let result = ProductDb::open(&scratch.path, "PRODUCT-A", at(2_000));
                match result {
                    Err(DbError::SchemaTooNew { database_version, binary_version: reported_binary_version }) => {
                        prop_assert_eq!(database_version, future_version);
                        prop_assert_eq!(reported_binary_version, binary_version);
                    }
                    other => prop_assert!(false, "expected SchemaTooNew for excess {excess}, got {other:?}"),
                }
            }
        }
    }
}
