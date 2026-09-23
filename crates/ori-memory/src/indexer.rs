//! The repository indexer, over SQLite FTS5: AICD §25.
//!
//! `spec/LLD.md` section 2 gives this crate `Indexer`; its section 6 fixes
//! where it lives on disk, `<app data>/ori/products/<product_id>/index/`, a
//! sibling of `product.sqlite`, `sessions/` and `evidence/`
//! (`crates/ori-store/src/db.rs`'s `ProductDb::open` already creates that
//! directory; this module is what fills it, in its own file,
//! `index/fts.sqlite`, never inside `product.sqlite`: see "The index is
//! derived" below). `spec/PRD.md` K-02 is the requirement: "Repository
//! indexer: full-text, document graph (sections, ADRs, criteria, modules),
//! re-index on merge".
//!
//! # Why SQLite FTS5, not `tantivy`
//!
//! `spec/LLD.md` section 2 originally named `tantivy` for this module, and
//! escalation E-0004 approved it (2026-09-23). Pinning it into this
//! workspace was proved, not assumed, to fail two already-installed checks:
//! `cargo deny` refused 12 duplicate crate-version entries (tantivy's own
//! `fs4` and `uuid` dependencies resolve to `windows-sys`/`windows-targets`
//! and `getrandom`/`r-efi` versions that collide with the ones `keyring` and
//! `proptest` already pin in this workspace), and `cargo audit --deny
//! warnings` denied RUSTSEC-2026-0253 in `lru 0.16.4` (tantivy's own cache
//! dependency), patched only at `lru >= 0.18.2`, outside tantivy 0.26.2's own
//! `^0.16.3` requirement, so no exact pin of tantivy fixed it. Both findings
//! are recorded in the pull request report this ticket's history carries.
//! The operator ruled: replace `tantivy` with SQLite's FTS5 extension, which
//! is already compiled into the SQLite this workspace bundles
//! (`libsqlite3-sys 0.38.2`'s `build.rs` passes `-DSQLITE_ENABLE_FTS5`
//! unconditionally under the `bundled` feature `crates/ori-store/Cargo.toml`
//! already turns on), so this module's `rusqlite` dependency
//! (`crates/ori-memory/Cargo.toml`) adds no new crate to the workspace at
//! all, only a second edge to one already-audited version.
//!
//! # What this module covers of K-02, and what it does not
//!
//! Covered: full-text search over indexed documents; a document graph in the
//! narrow sense of a [`DocumentKind`] every document carries (`Section`,
//! `Adr`, `Criterion`, `Module`), so a caller can filter or facet by kind;
//! incremental re-indexing ("re-index on merge") that agrees with a full
//! rebuild, proved in this module's tests.
//!
//! Not covered, and left for the tickets that own the missing half: edges
//! *between* graph nodes (a section citing an ADR, a criterion naming a
//! module) are not modeled here, only the nodes and their kind; `Module`
//! documents are never produced by this module's own repository walk
//! ([`Indexer::collect_from_repo`] never emits one) because reading source
//! code into a module summary is the code map's job (ORI-T-0036, a sibling
//! ticket this crate's scope forbids touching); this module accepts a
//! `Module`-kind [`IndexableDocument`] from a caller that built one, same as
//! any other kind, but builds none itself.
//!
//! # The seam PRD K-04 needs: what this module can never be handed
//!
//! `spec/PRD.md` K-04 (ORI-T-0037's ticket, binding here too): "raw evidence
//! stored apart and never indexed". `spec/DATA_MODEL.md` section 2's
//! `EvidenceBlob` is `{ id, record_id, content_ref, content_type,
//! untrusted: true }`, a reference to a file under `evidence/`, never a body
//! of text. [`IndexableDocument`], the only shape [`Indexer::full_rebuild`],
//! [`Indexer::incremental_sync`] and [`Indexer::add_or_replace`] accept, has
//! no `content_ref`, no `content_type`, and no `untrusted` flag: there is no
//! field of it an `EvidenceBlob` could be coerced into, and this module
//! defines no conversion from one to the other.
//!
//! ORI-T-0037 has since landed on `main` (not on this branch, which does not
//! rebase onto it; the lead integrates) and defines the real seam:
//! `ori_memory::barrier::Indexable`, a sealed trait implemented only for its
//! `SanitizedField`, with `EvidenceBlob` exposing no text at all (only
//! `content_ref() -> &Path`). The intended contract, recorded here rather
//! than guessed at in code this branch cannot compile against: the
//! record-ingesting path (a sanitized `MemoryRecord`, as opposed to a
//! canonical `spec/` document) accepts `&impl
//! ori_memory::barrier::Indexable`, never anything carrying a `content_ref`,
//! wired at integration. `IndexableDocument` remains, unchanged, this
//! module's own type for canonical documents walked out of `spec/`, which
//! `ori_memory::barrier::Indexable` has no reason to cover.
//!
//! # `spec/design/` is excluded, and this is a decision, not an oversight
//!
//! Escalation E-0006 (`ops/escalations/E-0006-the-design-artifact-is-mock-data.md`)
//! found `spec/design/Ori Studio.html` to be 342 KB of mock UI data for a
//! fictional product ("Ledgerline"), with its own fabricated
//! `spec/`-shaped citations. [`Indexer::collect_from_repo`] never descends
//! into `spec/design/`: indexing that file's prose as this product's
//! canonical documents would put a different, invented product's sample
//! tickets and sample criteria into a full-text index and a freshness
//! tracker that both exist to be trusted, exactly the "look like this
//! product's truth" failure E-0006 raised about the machinery built around
//! that file before it. `spec/design/` governs the phase 3 screen set
//! (ruling R23) and is real specification content for a different purpose
//! (the UI's own design system, not mocked); this indexer takes no side on
//! that file's worth, only on whether it belongs in *this* corpus, and it
//! does not.
//!
//! # The index is derived: rebuild and incremental must agree
//!
//! Same discipline `ori-store`'s projections and `rebuild.rs` already carry
//! for the event log: `index/` holds nothing that cannot be reconstructed
//! from the repository. [`Indexer::open`] creates `<dir>/fts.sqlite`, a file
//! of its own, never a table inside `product.sqlite`: deleting `index/` and
//! reopening loses nothing that [`Indexer::full_rebuild`] cannot put back
//! from the repository, whereas a table sharing `product.sqlite` would tie
//! this derived, disposable data to the event log's own file, which
//! `spec/LLD.md` section 6 never asks for and this module's own tests
//! (`tests::ori_t_0035_the_index_lives_in_its_own_file_never_inside_product_sqlite`)
//! check directly. [`Indexer::incremental_sync`] ("re-index on merge") must
//! leave the index in the state a full rebuild from the same target document
//! set would. This module's tests prove it the way `rebuild.rs` proves its
//! own claim: by dumping [`Indexer::all_documents`] (every stored column of
//! every document, sorted by path so the dump does not depend on row
//! insertion order) from both paths and comparing byte for byte, not by
//! inspection. A document removed from the target set is deleted (`DELETE
//! FROM documents WHERE path = ?1`), not left to linger, which
//! [`Indexer::incremental_sync`]'s own tests check directly.
//!
//! ```mermaid
//! flowchart TB
//!   T[target: &[IndexableDocument]] --> C{full_rebuild or incremental_sync}
//!   C -->|full_rebuild| CLEAR[DELETE FROM documents] --> ADDALL[INSERT every target document] --> COMMIT[transaction commit]
//!   C -->|incremental_sync| CURRENT[all_documents: what is on disk now]
//!   CURRENT --> DIFF{diff against target, by path and checksum}
//!   DIFF -->|path in current, not in target| DEL[DELETE WHERE path]
//!   DIFF -->|path new, or checksum changed| UPSERT[DELETE WHERE path, then INSERT]
//!   DIFF -->|path present, checksum unchanged| SKIP[leave alone]
//!   DEL --> COMMIT
//!   UPSERT --> COMMIT
//! ```
//!
//! # FTS5 query safety: quoting is not optional
//!
//! Binding `query` as a parameter (never string-formatted into the SQL text)
//! stops ordinary SQL injection, but is not enough on its own: FTS5 parses
//! **its own query language** inside the bound `MATCH` argument itself
//! (column filters like `path:`, boolean `AND`/`OR`/`NOT`, `NEAR`, a
//! trailing `*` prefix wildcard, bare double quotes). A caller's search text
//! is never that language; it is content to search for. [`Indexer::search`]
//! therefore always wraps `query` as one FTS5 string literal (`"..."`,
//! doubling any embedded `"`, exactly SQLite's own escaping rule for a
//! quoted string), so `path:secret OR *` becomes a literal three-token
//! phrase search, never a column filter or a widened boolean query. This is
//! tested directly against a hostile corpus
//! (`tests::ori_t_0035_hostile_query_text_is_never_interpreted_as_fts5_syntax`):
//! unbalanced quotes, `NOT`, a column filter, a bare `*`, an embedded `NUL`
//! and an empty string each either return the (possibly empty) matching set
//! or [`IndexerError::InvalidQuery`], never a widened search and never a
//! `rusqlite`/SQLite message forwarded verbatim to the caller (the embedded
//! `NUL` case does reach SQLite's own parser, which reports "unterminated
//! string" because FTS5's parser stops scanning at a `NUL`; that raw report
//! is discarded and mapped to [`IndexerError::InvalidQuery`], not returned).
//!
//! # Tokenizer
//!
//! `unicode61`, FTS5's own general-purpose Unicode tokenizer, the built-in
//! default: this indexer has no language-specific stemming need PRD K-02
//! states, and `unicode61` already folds diacritics (a search for `resume`
//! matches stored `résumé`, proved in
//! `tests::ori_t_0035_search_matches_non_ascii_terms_and_is_diacritic_insensitive`),
//! which is the right default for a specification written in mixed
//! technical and prose English with occasional accented names and no
//! per-language configuration surface this ticket has any reason to expose.
//!
//! # Concurrency
//!
//! A second writer against the same on-disk `index/fts.sqlite` must fail
//! cleanly or wait, never corrupt the index (SQLite makes both testable,
//! which `tantivy`'s own file locking did not for this ticket to prove).
//! This module chooses **fail cleanly**: `journal_mode = WAL` (concurrent
//! readers never block a writer or each other) and an explicit
//! `busy_timeout` of zero (a second writer's transaction returns
//! [`IndexerError::Locked`] immediately rather than blocking the caller for
//! an unbounded or arbitrary wait), deterministic and cheap to test
//! (`tests::ori_t_0035_a_second_writer_against_the_same_on_disk_index_fails_cleanly_not_corrupting_it`)
//! without a timeout-dependent, potentially flaky test. A caller that wants
//! to wait instead retries on [`IndexerError::Locked`] itself; this module
//! does not hide that choice inside an implicit, undocumented wait.
//!
//! # The vacuity trap
//!
//! An index nobody put anything into still answers every query, with zero
//! hits, which reads exactly like "searched, found nothing" from the
//! outside. [`SearchReport`] and [`IndexReport`] both carry
//! `documents_covered` / `total`, the index's live document count at the
//! moment of the call, independent of how many hits a particular query got,
//! so a test (and a caller such as `ori-flows`'s readiness check) can assert
//! the index was not empty before trusting an absence.
//!
//! Must not: return unsanitized production content in a package (`spec/LLD.md`
//! section 2); this module never reads `evidence/`, and the type it accepts
//! cannot represent an entry from it.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Connection;
use rusqlite::ErrorCode;
use rusqlite::params;

/// How long a write waits for a lock another connection holds before
/// [`Indexer::full_rebuild`], [`Indexer::incremental_sync`] or
/// [`Indexer::add_or_replace`] returns [`IndexerError::Locked`]: zero, a
/// deliberate, documented choice ("Concurrency" above), not SQLite's own
/// undocumented default.
const WRITE_BUSY_TIMEOUT: Duration = Duration::from_millis(0);

/// The virtual table's schema: `path`, `kind` and `checksum` carry
/// `UNINDEXED` (stored and retrievable, never matched by `MATCH`, the same
/// role `STORED`-without-`TEXT` played in this module's `tantivy` draft);
/// `title` and `body` are the only full-text-searched columns.
const CREATE_TABLE_SQL: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS documents USING fts5( \
    path UNINDEXED, \
    kind UNINDEXED, \
    title, \
    body, \
    checksum UNINDEXED, \
    tokenize = 'unicode61' \
);";

/// Which of PRD K-02's document graph kinds an [`IndexableDocument`] is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentKind {
    /// A section of a canonical specification document.
    Section,
    /// One architecture decision record.
    Adr,
    /// One acceptance criterion.
    Criterion,
    /// A source module, as the code map (ORI-T-0036) describes it. This
    /// module's own repository walk never produces one; see the module doc.
    Module,
}

impl DocumentKind {
    /// The stable text this kind is stored and matched as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Adr => "adr",
            Self::Criterion => "criterion",
            Self::Module => "module",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "section" => Some(Self::Section),
            "adr" => Some(Self::Adr),
            "criterion" => Some(Self::Criterion),
            "module" => Some(Self::Module),
            _ => None,
        }
    }
}

impl fmt::Display for DocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One canonical document this indexer may hold: AICD §25.
///
/// See the module doc's "The seam PRD K-04 needs" for why this is
/// deliberately not, and cannot be mistaken for, an `EvidenceBlob`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexableDocument {
    /// Where it sits, repository relative, `/` separated, and (within one
    /// [`Indexer`]) unique: this is the identity [`Indexer::incremental_sync`]
    /// diffs and [`Indexer::add_or_replace`] replaces on.
    pub path: String,
    /// Which document graph kind it is.
    pub kind: DocumentKind,
    /// Its title (a heading, an ADR's name, a criterion's id).
    pub title: String,
    /// Its full text, indexed and stored.
    pub body: String,
}

impl IndexableDocument {
    /// Builds one document.
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        kind: DocumentKind,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            title: title.into(),
            body: body.into(),
        }
    }

    /// A checksum of `title` and `body` together, for change detection only.
    ///
    /// `std::hash::Hasher`, not a cryptographic digest: nothing here needs a
    /// security property (this is "did the text change", not an integrity
    /// check against a hostile actor), and a hashing crate of its own is
    /// outside this ticket's approved dependency (`rusqlite`, already
    /// pinned by `ori-store`; see `crates/ori-memory/Cargo.toml`'s comment).
    /// Stored as a SQLite `INTEGER` via a bitwise `as i64` reinterpretation
    /// of the `u64` value (exact and lossless both ways; SQLite's own
    /// integer column is signed 64-bit, so this is the same trick
    /// `checksum() as i64` / `.. as u64` uses at every call site below).
    fn checksum(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.title.hash(&mut hasher);
        self.body.hash(&mut hasher);
        hasher.finish()
    }
}

/// One hit from [`Indexer::search`].
#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    /// The matching document's path.
    pub path: String,
    /// Its kind.
    pub kind: DocumentKind,
    /// Its title.
    pub title: String,
    /// FTS5's `bm25()` relevance score against the query: smaller (more
    /// negative) is a better match, SQLite's own convention, which is why
    /// [`Indexer::search`] orders hits by this value ascending.
    pub score: f64,
}

/// The result of one [`Indexer::search`] call.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchReport {
    /// The matching documents, best score first.
    pub hits: Vec<SearchHit>,
    /// The index's total live document count at the moment of this call,
    /// independent of `hits.len()`. The module doc's vacuity trap: this is
    /// what lets a caller tell "searched an index of 400 documents, 0
    /// matched this query" apart from "searched an empty index".
    pub documents_covered: usize,
}

/// The result of one [`Indexer::full_rebuild`] or [`Indexer::incremental_sync`]
/// call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexReport {
    /// Documents written (added new, or replaced because their checksum
    /// changed).
    pub upserted: usize,
    /// Documents deleted because they were no longer in the target set.
    pub removed: usize,
    /// The index's total live document count after this call.
    pub total: usize,
}

/// A refusal from this module: AICD §25.
#[derive(Debug)]
#[non_exhaustive]
pub enum IndexerError {
    /// Creating the on-disk directory at `path` failed.
    Directory {
        /// The directory this was attempted against.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A repository walk failed reading a file.
    Io {
        /// The path the read was against.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A second writer already held `index/fts.sqlite`'s write lock when
    /// this call needed it (`WRITE_BUSY_TIMEOUT` is zero; see the module
    /// doc's "Concurrency"). The index itself is untouched; retrying later
    /// is safe.
    Locked {
        /// The database file that was already locked.
        path: PathBuf,
    },
    /// A `rusqlite` call failed in a way none of the above names more
    /// specifically.
    Sqlite {
        /// What was being attempted.
        context: String,
        /// The underlying error.
        source: rusqlite::Error,
    },
    /// `query` could not be run safely as an FTS5 `MATCH` expression even
    /// after being quoted as a literal phrase (the module doc's "FTS5 query
    /// safety"; an embedded `NUL` is the one case this module's own tests
    /// reach). Carries only the query text a caller already had, never the
    /// underlying SQLite message, so nothing from inside the query engine
    /// is echoed back to whoever sent the query.
    InvalidQuery {
        /// The text that could not be searched safely.
        query: String,
    },
}

impl IndexerError {
    /// Wraps a `rusqlite` failure with what was being attempted, for the one
    /// variant that is not any of this module's named refusals; the specific
    /// case of a write lock already held is split out as
    /// [`IndexerError::Locked`] by [`is_locked`] at each write call site,
    /// the same split `crates/ori-store/src/db.rs`'s `DbError::sqlite` and
    /// `is_busy` make for `product.sqlite`'s own lock.
    fn sqlite(context: impl Into<String>, source: rusqlite::Error) -> Self {
        Self::Sqlite {
            context: context.into(),
            source,
        }
    }

    /// The methodology section this refusal rests on: AICD §25, this
    /// module's own section (`crates/ori-memory/src/lib.rs`'s module doc,
    /// "AICD §8, AICD §25"), the same shape `crates/ori-store/src/db.rs`'s
    /// `DbError::methodology_ref` uses for the reason its own doc gives:
    /// every variant here, mechanical as some of them read, is this indexer
    /// refusing to stand in for a corpus or a query result it cannot vouch
    /// for. `spec/LLD.md` section 4 fixes `MethodologyRef { section: u8,
    /// subsection: Option<String> }` as "a contract other crates and the
    /// client API read" (`crates/ori-core/src/error.rs`), so this reuses
    /// that type rather than a locally shaped copy.
    #[must_use]
    pub const fn methodology_ref(&self) -> ori_core::error::MethodologyRef {
        ori_core::error::MethodologyRef {
            section: 25,
            subsection: None,
        }
    }
}

impl fmt::Display for IndexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Locked { path } => write!(
                f,
                "{} is held by another writer; refusing to wait (retry is safe)",
                path.display()
            ),
            Self::Sqlite { context, source } => write!(f, "{context}: {source}"),
            Self::InvalidQuery { query } => write!(f, "invalid query: {query}"),
        }
    }
}

impl std::error::Error for IndexerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Directory { source, .. } | Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::Locked { .. } | Self::InvalidQuery { .. } => None,
        }
    }
}

/// Whether a `rusqlite` failure is SQLite reporting a write conflict rather
/// than any other error: the same check
/// `crates/ori-store/src/db.rs`'s `is_busy` makes for `product.sqlite`'s own
/// lock file.
fn is_locked(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == ErrorCode::DatabaseBusy || inner.code == ErrorCode::DatabaseLocked
    )
}

/// Quotes `text` as one FTS5 string literal, doubling any embedded `"`
/// (SQLite's own escaping rule for a quoted string): the module doc's "FTS5
/// query safety". Turns arbitrary caller text into a single literal phrase
/// token, never FTS5's own query syntax.
fn quote_fts5_phrase(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The repository indexer: a SQLite FTS5 virtual table over
/// [`IndexableDocument`]s, keyed by `path`: AICD §25.
pub struct Indexer {
    conn: Connection,
    /// The file this connection is open against, `Some` only for
    /// [`Indexer::open`]; `None` for [`Indexer::open_in_memory`], which has
    /// no path to name in [`IndexerError::Locked`] or a diagnostic.
    path: Option<PathBuf>,
}

impl Indexer {
    /// Opens the on-disk index at `<dir>/fts.sqlite` (`<product>/index/` per
    /// `spec/LLD.md` section 6), creating `dir` and the file if either is
    /// absent. Never `<dir>/../product.sqlite`; see the module doc's "The
    /// index is derived".
    ///
    /// # Errors
    ///
    /// [`IndexerError::Directory`] if `dir` cannot be created;
    /// [`IndexerError::Sqlite`] if the file cannot be opened or the schema
    /// cannot be created.
    pub fn open(dir: &Path) -> Result<Self, IndexerError> {
        std::fs::create_dir_all(dir).map_err(|source| IndexerError::Directory {
            path: dir.to_owned(),
            source,
        })?;
        let path = dir.join("fts.sqlite");
        let conn = Connection::open(&path)
            .map_err(|source| IndexerError::sqlite(format!("open {}", path.display()), source))?;
        Self::configure(conn, Some(path))
    }

    /// An index held only in memory, for tests and for any caller that
    /// wants no on-disk footprint.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Sqlite`] if the schema cannot be created.
    pub fn open_in_memory() -> Result<Self, IndexerError> {
        let conn = Connection::open_in_memory()
            .map_err(|source| IndexerError::sqlite("open an in-memory database", source))?;
        Self::configure(conn, None)
    }

    fn configure(conn: Connection, path: Option<PathBuf>) -> Result<Self, IndexerError> {
        // WAL: concurrent readers never block a writer or each other (the
        // same choice `crates/ori-store/src/db.rs`'s `open_connection` makes
        // for `product.sqlite`). A zero busy_timeout is set explicitly,
        // deliberately, rather than left at SQLite's own default: see the
        // module doc's "Concurrency".
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|source| IndexerError::sqlite("set journal_mode", source))?;
        conn.busy_timeout(WRITE_BUSY_TIMEOUT)
            .map_err(|source| IndexerError::sqlite("set busy_timeout", source))?;
        conn.execute_batch(CREATE_TABLE_SQL)
            .map_err(|source| IndexerError::sqlite("create the documents table", source))?;
        Ok(Self { conn, path })
    }

    /// The path [`IndexerError::Locked`] and [`IndexerError::Sqlite`] name
    /// for a write refusal: the real file for [`Indexer::open`], or a
    /// synthetic label for [`Indexer::open_in_memory`], which this module's
    /// own concurrency test never needs (there is only ever one connection
    /// to a given in-memory database in this process).
    fn display_path(&self) -> PathBuf {
        self.path
            .clone()
            .unwrap_or_else(|| PathBuf::from(":memory:"))
    }

    /// Every document presently in the index, in path order, each paired
    /// with its stored checksum.
    ///
    /// The source of truth [`Indexer::incremental_sync`] diffs against and
    /// this module's rebuild-versus-incremental proof dumps: read fresh from
    /// the index itself on every call, never from bookkeeping held in
    /// memory, so it is correct even for an [`Indexer`] freshly opened on an
    /// existing on-disk file whose history this process never saw.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Sqlite`] if the read fails.
    pub fn all_documents(&self) -> Result<Vec<(IndexableDocument, u64)>, IndexerError> {
        let mut statement = self
            .conn
            .prepare("SELECT path, kind, title, body, checksum FROM documents ORDER BY path")
            .map_err(|source| IndexerError::sqlite("prepare a full read of documents", source))?;
        let rows = statement
            .query_map([], Self::row_to_document)
            .map_err(|source| IndexerError::sqlite("read every document back", source))?;
        let mut out = Vec::new();
        for row in rows {
            let pair =
                row.map_err(|source| IndexerError::sqlite("read one document row", source))?;
            if let Some(pair) = pair {
                out.push(pair);
            }
        }
        Ok(out)
    }

    fn row_to_document(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<Option<(IndexableDocument, u64)>> {
        let path: String = row.get(0)?;
        let kind_text: String = row.get(1)?;
        let title: String = row.get(2)?;
        let body: String = row.get(3)?;
        let checksum: i64 = row.get(4)?;
        Ok(DocumentKind::parse(&kind_text).map(|kind| {
            (
                IndexableDocument::new(path, kind, title, body),
                checksum as u64,
            )
        }))
    }

    /// The index's live document count, without reading any document back:
    /// the cheap half of [`Indexer::all_documents`], for a caller that only
    /// needs the anti-vacuity count ([`SearchReport::documents_covered`],
    /// [`IndexReport::total`]).
    ///
    /// # Errors
    ///
    /// [`IndexerError::Sqlite`] if the read fails.
    fn total_indexed(&self) -> Result<usize, IndexerError> {
        let total: i64 = self
            .conn
            .query_row("SELECT count(*) FROM documents", [], |row| row.get(0))
            .map_err(|source| IndexerError::sqlite("count documents", source))?;
        Ok(total as usize)
    }

    /// Adds `document`, first deleting any existing document at the same
    /// `path` (an FTS5 virtual table has no in-place update; this is the
    /// same delete-then-add shape SQLite's own documentation gives for
    /// FTS5). Runs inside one transaction, so a caller never observes the
    /// old and new rows both present or both absent.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Locked`] if another writer holds the lock;
    /// [`IndexerError::Sqlite`] if the write fails.
    pub fn add_or_replace(&mut self, document: &IndexableDocument) -> Result<(), IndexerError> {
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| Self::write_error(&path, "begin transaction", source))?;
        Self::delete_path(&tx, &document.path, &path)?;
        Self::insert(&tx, document, &path)?;
        tx.commit()
            .map_err(|source| Self::write_error(&path, "commit", source))
    }

    /// Clears the index and indexes exactly `documents`: a full rebuild, the
    /// reconstruction the module doc's "the index is derived" section names.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Locked`] if another writer holds the lock;
    /// [`IndexerError::Sqlite`] if a write fails.
    pub fn full_rebuild(
        &mut self,
        documents: &[IndexableDocument],
    ) -> Result<IndexReport, IndexerError> {
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| Self::write_error(&path, "begin transaction", source))?;
        tx.execute("DELETE FROM documents", [])
            .map_err(|source| Self::write_error(&path, "delete_all for a full rebuild", source))?;
        for document in documents {
            Self::insert(&tx, document, &path)?;
        }
        tx.commit()
            .map_err(|source| Self::write_error(&path, "commit", source))?;
        Ok(IndexReport {
            upserted: documents.len(),
            removed: 0,
            total: self.total_indexed()?,
        })
    }

    /// Re-indexes to match `documents` exactly ("re-index on merge", PRD
    /// K-02): every path in the index but not in `documents` is deleted;
    /// every path in `documents` that is new, or whose checksum differs from
    /// what is on disk, is written (delete-then-add); every path present and
    /// unchanged is left alone. Runs inside one transaction.
    ///
    /// Reads the current index with [`Indexer::all_documents`] rather than
    /// trusting any state held in memory, so this is correct even as the
    /// first call after opening an existing on-disk index in a fresh
    /// process: see that method's doc.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Locked`] if another writer holds the lock;
    /// [`IndexerError::Sqlite`] if reading the current state or a write
    /// fails.
    pub fn incremental_sync(
        &mut self,
        documents: &[IndexableDocument],
    ) -> Result<IndexReport, IndexerError> {
        let current = self.all_documents()?;
        let current_checksums: BTreeMap<&str, u64> = current
            .iter()
            .map(|(document, checksum)| (document.path.as_str(), *checksum))
            .collect();
        let target_paths: BTreeMap<&str, &IndexableDocument> = documents
            .iter()
            .map(|document| (document.path.as_str(), document))
            .collect();

        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| Self::write_error(&path, "begin transaction", source))?;

        let mut removed = 0usize;
        for existing_path in current_checksums.keys() {
            if !target_paths.contains_key(existing_path) {
                Self::delete_path(&tx, existing_path, &path)?;
                removed += 1;
            }
        }

        let mut upserted = 0usize;
        for document in documents {
            let unchanged = current_checksums
                .get(document.path.as_str())
                .is_some_and(|existing| *existing == document.checksum());
            if unchanged {
                continue;
            }
            Self::delete_path(&tx, &document.path, &path)?;
            Self::insert(&tx, document, &path)?;
            upserted += 1;
        }

        tx.commit()
            .map_err(|source| Self::write_error(&path, "commit", source))?;
        Ok(IndexReport {
            upserted,
            removed,
            total: self.total_indexed()?,
        })
    }

    fn delete_path(
        tx: &rusqlite::Transaction<'_>,
        target_path: &str,
        display_path: &Path,
    ) -> Result<(), IndexerError> {
        tx.execute(
            "DELETE FROM documents WHERE path = ?1",
            params![target_path],
        )
        .map_err(|source| Self::write_error(display_path, "delete one document", source))?;
        Ok(())
    }

    fn insert(
        tx: &rusqlite::Transaction<'_>,
        document: &IndexableDocument,
        display_path: &Path,
    ) -> Result<(), IndexerError> {
        tx.execute(
            "INSERT INTO documents (path, kind, title, body, checksum) VALUES (?1,?2,?3,?4,?5)",
            params![
                document.path,
                document.kind.as_str(),
                document.title,
                document.body,
                document.checksum() as i64,
            ],
        )
        .map_err(|source| Self::write_error(display_path, "insert one document", source))?;
        Ok(())
    }

    /// Maps a write failure to [`IndexerError::Locked`] when it is SQLite
    /// reporting the file already held by another writer, and to
    /// [`IndexerError::Sqlite`] otherwise.
    fn write_error(path: &Path, context: &str, source: rusqlite::Error) -> IndexerError {
        if is_locked(&source) {
            IndexerError::Locked {
                path: path.to_owned(),
            }
        } else {
            IndexerError::sqlite(context, source)
        }
    }

    /// Full-text search over `title` and `body`, at most `limit` hits,
    /// best-scored (lowest `bm25()`) first, ties broken by `path` ascending
    /// so the same query gives the same order every time. `query` is always
    /// quoted as one FTS5 literal phrase before being bound; see the module
    /// doc's "FTS5 query safety".
    ///
    /// An empty `query` (after trimming) returns every stored document's
    /// zero matches directly, without asking FTS5 to parse a degenerate
    /// phrase: `documents_covered` is still the index's real size.
    ///
    /// # Errors
    ///
    /// [`IndexerError::InvalidQuery`] if `query`, even quoted, cannot be
    /// searched safely (the module doc's embedded-`NUL` case);
    /// [`IndexerError::Sqlite`] if the search fails for another reason.
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchReport, IndexerError> {
        let total = self.total_indexed()?;
        if query.trim().is_empty() {
            return Ok(SearchReport {
                hits: Vec::new(),
                documents_covered: total,
            });
        }

        let quoted = quote_fts5_phrase(query);
        let limit = i64::try_from(limit.max(1)).unwrap_or(i64::MAX);
        let mut statement = self
            .conn
            .prepare(
                "SELECT path, kind, title, bm25(documents) AS rank \
                 FROM documents WHERE documents MATCH ?1 \
                 ORDER BY rank ASC, path ASC LIMIT ?2",
            )
            .map_err(|source| IndexerError::sqlite("prepare a search", source))?;
        // `query_map` itself only prepares the row-mapping closure; FTS5's
        // own query-syntax errors (the embedded-NUL case this module's
        // tests reach) surface lazily, while the returned iterator is
        // stepped below, not here. Either failure point is mapped the same
        // way: a typed InvalidQuery, never the raw SQLite message.
        let rows = statement
            .query_map(params![quoted, limit], |row| {
                let path: String = row.get(0)?;
                let kind_text: String = row.get(1)?;
                let title: String = row.get(2)?;
                let score: f64 = row.get(3)?;
                Ok((path, kind_text, title, score))
            })
            .map_err(|_source| IndexerError::InvalidQuery {
                query: query.to_owned(),
            })?;

        let mut hits = Vec::new();
        for row in rows {
            let (path, kind_text, title, score) =
                row.map_err(|_source| IndexerError::InvalidQuery {
                    query: query.to_owned(),
                })?;
            if let Some(kind) = DocumentKind::parse(&kind_text) {
                hits.push(SearchHit {
                    path,
                    kind,
                    title,
                    score,
                });
            }
        }
        Ok(SearchReport {
            hits,
            documents_covered: total,
        })
    }

    /// Walks `repo_root` for the canonical documents this indexer is
    /// permitted to hold, and returns them ready for [`Indexer::full_rebuild`]
    /// or [`Indexer::incremental_sync`].
    ///
    /// Only `spec/adr/*.md` (kind [`DocumentKind::Adr`], one document per
    /// file), `spec/criteria/*.md` (kind [`DocumentKind::Criterion`], one
    /// document per table row whose first cell matches
    /// `ORI-[A-Z0-9]+-[0-9]+`, per `spec/criteria/phase-1.md`'s own format
    /// line, "Identifier `ORI-P1-nnn`"), and every other `spec/**/*.md` (kind
    /// [`DocumentKind::Section`], one document per ATX heading, flat rather
    /// than level-aware: this module's own splitter, not
    /// `ori-gates::spec_refs::heading_slug`'s, since `ori-memory` does not
    /// and must not depend on `ori-gates`, a sideways crate under
    /// `spec/LLD.md` section 2's dependency direction) are walked.
    /// `spec/design/` is skipped entirely; see the module doc.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Io`] if a directory or file cannot be read.
    pub fn collect_from_repo(repo_root: &Path) -> Result<Vec<IndexableDocument>, IndexerError> {
        let spec_dir = repo_root.join("spec");
        let mut out = Vec::new();
        walk_markdown(&spec_dir, &mut out)?;
        Ok(out)
    }
}

/// Recursively walks `dir` for `.md` files, skipping `spec/design/` by name,
/// and appends every document found to `out`.
fn walk_markdown(dir: &Path, out: &mut Vec<IndexableDocument>) -> Result<(), IndexerError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|source| IndexerError::Io {
        path: dir.to_owned(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| IndexerError::Io {
            path: dir.to_owned(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("design") {
                // E-0006: spec/design/ is mock data for a fictional product.
                // See the module doc, "spec/design/ is excluded".
                continue;
            }
            walk_markdown(&path, out)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|source| IndexerError::Io {
            path: path.clone(),
            source,
        })?;
        let relative = path
            .strip_prefix(dir.parent().and_then(Path::parent).unwrap_or(dir))
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let is_adr = path
            .components()
            .any(|component| component.as_os_str().to_str() == Some("adr"));
        let is_criteria = path
            .components()
            .any(|component| component.as_os_str().to_str() == Some("criteria"));
        if is_adr {
            let title = first_heading(&text).unwrap_or_else(|| relative.clone());
            out.push(IndexableDocument::new(
                relative,
                DocumentKind::Adr,
                title,
                text,
            ));
        } else if is_criteria {
            out.extend(criteria_documents(&relative, &text));
        } else {
            out.extend(section_documents(&relative, &text));
        }
    }
    Ok(())
}

/// The text of the first ATX heading in `text`, with leading `#`s and
/// whitespace stripped.
fn first_heading(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let trimmed = line.trim_start();
        trimmed
            .starts_with('#')
            .then(|| trimmed.trim_start_matches('#').trim().to_owned())
    })
}

/// Splits `text` into one [`IndexableDocument`] per ATX heading, flat (every
/// heading line starts a new section regardless of its level; see
/// [`Indexer::collect_from_repo`]'s doc for why this does not attempt
/// `ori-gates`'s level-aware split). Text before the first heading, if any,
/// is not a document of its own; a file with no heading at all becomes one
/// document titled by its path.
fn section_documents(relative_path: &str, text: &str) -> Vec<IndexableDocument> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_body = String::new();

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            if let Some(title) = current_title.take() {
                sections.push((title, std::mem::take(&mut current_body)));
            }
            current_title = Some(trimmed.trim_start_matches('#').trim().to_owned());
        } else if current_title.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if let Some(title) = current_title {
        sections.push((title, current_body));
    }

    if sections.is_empty() {
        return vec![IndexableDocument::new(
            relative_path.to_owned(),
            DocumentKind::Section,
            relative_path.to_owned(),
            text.to_owned(),
        )];
    }

    sections
        .into_iter()
        .map(|(title, body)| {
            let anchor = heading_anchor(&title);
            IndexableDocument::new(
                format!("{relative_path}#{anchor}"),
                DocumentKind::Section,
                title,
                body,
            )
        })
        .collect()
}

/// A simple, local heading-to-anchor mapping: lower-cased, non-alphanumeric
/// runs collapsed to one `-`. Deliberately not
/// `ori-gates::spec_refs::heading_slug` (see [`Indexer::collect_from_repo`]'s
/// doc); this indexer's anchors are its own document identities, never
/// compared against the citation gate's.
fn heading_anchor(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut pending_dash = false;
    for character in title.chars() {
        if character.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.extend(character.to_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out
}

/// Splits a criteria table's rows into one [`IndexableDocument`] per
/// criterion, matching `spec/criteria/phase-1.md`'s own format: a markdown
/// table whose first column is the identifier (`ORI-P1-nnn`).
fn criteria_documents(relative_path: &str, text: &str) -> Vec<IndexableDocument> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some(id) = cells.first() else { continue };
        if !is_criterion_id(id) {
            continue;
        }
        out.push(IndexableDocument::new(
            format!("{relative_path}#{id}"),
            DocumentKind::Criterion,
            (*id).to_owned(),
            trimmed.to_owned(),
        ));
    }
    out
}

/// Whether `text` matches `ORI-[A-Z0-9]+-[0-9]+`, the criterion identifier
/// shape `spec/DATA_MODEL.md` section 2 states ("id (human-readable, e.g.
/// ORI-P1-014)") and `spec/criteria/phase-1.md` uses throughout.
fn is_criterion_id(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("ORI-") else {
        return false;
    };
    let Some(dash) = rest.rfind('-') else {
        return false;
    };
    let (middle, last) = rest.split_at(dash);
    let last = &last[1..];
    !middle.is_empty()
        && middle.chars().all(|c| c.is_ascii_alphanumeric())
        && !last.is_empty()
        && last.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    // -------------------------------------------------------------------
    // Scratch layout, the same shape `crates/ori-store/src/db.rs` uses for
    // its own tests.
    // -------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0035-{label}-{}-{unique}",
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

    fn doc(path: &str, kind: DocumentKind, title: &str, body: &str) -> IndexableDocument {
        IndexableDocument::new(path, kind, title, body)
    }

    // -------------------------------------------------------------------
    // Every refusal carries a MethodologyRef (CLAUDE.md rule 9).
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_every_indexer_error_variant_carries_the_same_methodology_ref() {
        let errors: Vec<IndexerError> = vec![
            IndexerError::InvalidQuery {
                query: "(".to_owned(),
            },
            IndexerError::Io {
                path: PathBuf::from("nowhere"),
                source: std::io::Error::other("boom"),
            },
            IndexerError::Locked {
                path: PathBuf::from("index/fts.sqlite"),
            },
        ];
        for error in errors {
            let reference = error.methodology_ref();
            assert_eq!(reference.section, 25);
            assert_eq!(reference.subsection, None);
        }
    }

    // -------------------------------------------------------------------
    // The vacuity trap (plant 1: "the indexer indexes nothing, queries
    // report success")
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_search_reports_documents_covered_even_when_a_query_matches_nothing() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let report = indexer
            .full_rebuild(&[
                doc("a.md", DocumentKind::Section, "Alpha", "alpha content"),
                doc("b.md", DocumentKind::Section, "Beta", "beta content"),
            ])
            .expect("full rebuild");
        assert_eq!(report.total, 2, "full_rebuild must report a non-zero total");

        let search = indexer
            .search("zzz_no_such_term_zzz", 10)
            .expect("search runs");
        assert_eq!(search.hits.len(), 0, "the query genuinely matches nothing");
        assert_eq!(
            search.documents_covered, 2,
            "documents_covered must report the index's real size, not just this query's hit \
             count, so an empty result and an empty index are never indistinguishable"
        );
    }

    #[test]
    fn ori_t_0035_an_empty_index_reports_zero_documents_covered_not_success_by_omission() {
        let indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let search = indexer.search("anything", 10).expect("search runs");
        assert_eq!(search.hits.len(), 0);
        assert_eq!(
            search.documents_covered, 0,
            "an index that was never given anything must say so as zero, not merely return no \
             hits and let that read as \"searched and found nothing interesting\""
        );
    }

    // -------------------------------------------------------------------
    // Full rebuild: happy path
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_full_rebuild_indexes_every_document_and_search_finds_them() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let report = indexer
            .full_rebuild(&[
                doc(
                    "docs/PRD-notes.md#k-02",
                    DocumentKind::Section,
                    "Memory",
                    "Repository indexer full-text document graph",
                ),
                doc(
                    "spec/adr/ADR-0001-stack.md",
                    DocumentKind::Adr,
                    "ADR-0001",
                    "SQLite is chosen for embedded storage",
                ),
            ])
            .expect("full rebuild");
        assert_eq!(report.upserted, 2);
        assert_eq!(report.removed, 0);
        assert_eq!(report.total, 2);

        let hits = indexer.search("indexer", 10).expect("search runs").hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "docs/PRD-notes.md#k-02");
        assert_eq!(hits[0].kind, DocumentKind::Section);
    }

    #[test]
    fn ori_t_0035_add_or_replace_upserts_a_single_document_by_path() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .add_or_replace(&doc("a.md", DocumentKind::Section, "A", "first text"))
            .expect("first add");
        assert_eq!(indexer.all_documents().expect("read back").len(), 1);

        indexer
            .add_or_replace(&doc("a.md", DocumentKind::Section, "A", "replaced text"))
            .expect("replace");
        let dump = indexer.all_documents().expect("read back");
        assert_eq!(
            dump.len(),
            1,
            "add_or_replace deletes the old document at the same path before adding the new \
             one, so two calls for the same path must never leave two documents behind"
        );
        assert_eq!(dump[0].0.body, "replaced text");

        let hits = indexer.search("first", 10).expect("search runs").hits;
        assert!(
            hits.is_empty(),
            "the old text must be gone, not merely shadowed by the new document"
        );
    }

    // -------------------------------------------------------------------
    // Plant 2: "incremental re-index skips removed documents" must FAIL,
    // i.e. a correct implementation must remove them.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_incremental_sync_removes_a_document_dropped_from_the_target_set() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[
                doc("a.md", DocumentKind::Section, "A", "alpha"),
                doc("b.md", DocumentKind::Section, "B", "beta"),
            ])
            .expect("full rebuild");
        assert_eq!(indexer.all_documents().expect("read back").len(), 2);

        let report = indexer
            .incremental_sync(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("incremental sync");

        assert_eq!(report.removed, 1, "b.md must be counted as removed");
        assert_eq!(
            report.upserted, 0,
            "a.md is unchanged, so nothing is upserted"
        );
        assert_eq!(report.total, 1);

        let remaining = indexer.all_documents().expect("read back");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0.path, "a.md");

        let hits = indexer.search("beta", 10).expect("search runs").hits;
        assert!(
            hits.is_empty(),
            "a document removed from the repository must disappear from the index on re-index, \
             not linger and still be findable"
        );
    }

    #[test]
    fn ori_t_0035_incremental_sync_upserts_new_and_changed_documents_and_skips_unchanged() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[
                doc("a.md", DocumentKind::Section, "A", "alpha original"),
                doc("b.md", DocumentKind::Section, "B", "beta unchanged"),
            ])
            .expect("full rebuild");

        let report = indexer
            .incremental_sync(&[
                doc("a.md", DocumentKind::Section, "A", "alpha edited"),
                doc("b.md", DocumentKind::Section, "B", "beta unchanged"),
                doc("c.md", DocumentKind::Section, "C", "gamma new"),
            ])
            .expect("incremental sync");

        assert_eq!(report.removed, 0);
        assert_eq!(
            report.upserted, 2,
            "a.md changed and c.md is new; b.md is byte-identical and must not be counted"
        );
        assert_eq!(report.total, 3);

        let hits = indexer.search("edited", 10).expect("search runs").hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "a.md");
    }

    // -------------------------------------------------------------------
    // Plant 4 (this ticket's own defense): full rebuild and incremental
    // must produce byte-identical dumps for the same target set.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_full_rebuild_and_incremental_sync_agree_on_the_same_target_set() {
        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha content here"),
            doc(
                "spec/adr/ADR-0001-stack.md",
                DocumentKind::Adr,
                "ADR-0001",
                "stack decision",
            ),
            doc(
                "docs/phase-1-criteria.md#ORI-P1-026",
                DocumentKind::Criterion,
                "ORI-P1-026",
                "stale document criterion text",
            ),
        ];

        let mut rebuilt = Indexer::open_in_memory().expect("in-memory index opens");
        rebuilt.full_rebuild(&target).expect("full rebuild");
        let rebuilt_dump = rebuilt.all_documents().expect("dump after full rebuild");

        let mut synced = Indexer::open_in_memory().expect("in-memory index opens");
        // Starts from a different, unrelated document, so incremental_sync
        // must both remove it and add every target document, not merely
        // append on top of an already-empty index.
        synced
            .full_rebuild(&[doc(
                "stale.md",
                DocumentKind::Section,
                "Stale",
                "old content",
            )])
            .expect("seed with an unrelated document");
        synced.incremental_sync(&target).expect("incremental sync");
        let synced_dump = synced.all_documents().expect("dump after incremental sync");

        assert_eq!(
            rebuilt_dump, synced_dump,
            "a full rebuild and an incremental sync to the same target set must produce the \
             identical stored state (path, kind, title, body and checksum for every document), \
             compared byte for byte via the Vec<(IndexableDocument, u64)> dump, the strictest \
             equality this module has"
        );
        assert_eq!(
            rebuilt_dump.len(),
            3,
            "the dump itself must not be vacuously empty"
        );
    }

    // -------------------------------------------------------------------
    // The repository walk: spec/design/ exclusion, ADR, criteria and
    // section handling.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_collect_from_repo_excludes_spec_design() {
        let scratch = Scratch::new("collect-design");
        let spec = scratch.path.join("spec");
        let design = spec.join("design");
        fs::create_dir_all(&design).expect("create spec/design");
        fs::write(
            design.join("Ori Studio.html"),
            "<html>Ledgerline mock data, ORI-DVG-04</html>",
        )
        .expect("write the mock design artifact");
        fs::write(spec.join("PRD.md"), "# PRD\n\nreal content\n").expect("write a real document");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
        assert!(
            !documents.is_empty(),
            "the real document must still be collected"
        );
        assert!(
            documents
                .iter()
                .all(|document| !document.body.contains("Ledgerline")),
            "spec/design/ must never contribute a document (escalation E-0006): {documents:?}"
        );
    }

    #[test]
    fn ori_t_0035_collect_from_repo_splits_criteria_rows_and_tags_adrs() {
        let scratch = Scratch::new("collect-criteria-adr");
        let criteria_dir = scratch.path.join("spec").join("criteria");
        fs::create_dir_all(&criteria_dir).expect("create spec/criteria");
        fs::write(
            criteria_dir.join("phase-1.md"),
            "# Acceptance criteria\n\n\
             | ID | Type | Tier | Precondition | Action | Expected result |\n\
             |---|---|---|---|---|---|\n\
             | ORI-P1-026 | F | 1 | Product with stale document | `ori readiness` | Not ready |\n",
        )
        .expect("write criteria table");

        let adr_dir = scratch.path.join("spec").join("adr");
        fs::create_dir_all(&adr_dir).expect("create spec/adr");
        fs::write(
            adr_dir.join("ADR-0001-stack.md"),
            "# ADR-0001: Stack\n\nSQLite, Rust, Tauri.\n",
        )
        .expect("write an ADR");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");

        let criterion = documents
            .iter()
            .find(|document| document.kind == DocumentKind::Criterion)
            .expect("one criterion document");
        assert_eq!(criterion.title, "ORI-P1-026");
        assert!(criterion.body.contains("Not ready"));

        let adr = documents
            .iter()
            .find(|document| document.kind == DocumentKind::Adr)
            .expect("one ADR document");
        assert!(adr.title.contains("ADR-0001"));
        assert!(adr.body.contains("SQLite"));
    }

    #[test]
    fn ori_t_0035_section_documents_splits_flat_by_every_heading() {
        let text = "# Title\n\nintro\n\n## One\n\nfirst body\n\n## Two\n\nsecond body\n";
        let sections = section_documents("docs/EXAMPLE.md", text);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].title, "Title");
        assert!(sections[0].body.contains("intro"));
        assert_eq!(sections[1].title, "One");
        assert!(sections[1].body.contains("first body"));
        assert_eq!(sections[2].title, "Two");
        assert!(sections[2].body.contains("second body"));
    }

    #[test]
    fn ori_t_0035_is_criterion_id_matches_the_documented_shape_only() {
        assert!(is_criterion_id("ORI-P1-026"));
        assert!(is_criterion_id("ORI-M2-014"));
        assert!(!is_criterion_id("not-an-id"));
        assert!(!is_criterion_id("ORI-P1-"));
        assert!(!is_criterion_id("ORI-P1"));
    }

    // -------------------------------------------------------------------
    // A persistent, on-disk index: opens, writes, and re-opens across a
    // fresh process boundary (simulated by dropping and reopening the
    // `Indexer`, which closes and reopens the SQLite connection).
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_an_on_disk_index_survives_being_reopened() {
        let scratch = Scratch::new("on-disk-reopen");
        {
            let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
                .expect("full rebuild");
        }
        let reopened = Indexer::open(&scratch.path).expect("reopen the same directory");
        let dump = reopened.all_documents().expect("read back after reopen");
        assert_eq!(dump.len(), 1);
        assert_eq!(dump[0].0.path, "a.md");
    }

    #[test]
    fn ori_t_0035_incremental_sync_after_a_reopen_still_removes_and_upserts_correctly() {
        let scratch = Scratch::new("on-disk-incremental-reopen");
        {
            let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
            indexer
                .full_rebuild(&[
                    doc("a.md", DocumentKind::Section, "A", "alpha"),
                    doc("b.md", DocumentKind::Section, "B", "beta"),
                ])
                .expect("full rebuild");
        }
        let mut reopened = Indexer::open(&scratch.path).expect("reopen the same directory");
        let report = reopened
            .incremental_sync(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("incremental sync after reopen, with no in-memory history");
        assert_eq!(
            report.removed, 1,
            "the diff must be computed from what is actually on disk, not from bookkeeping this \
             process never had"
        );
        assert_eq!(report.total, 1);
    }

    // -------------------------------------------------------------------
    // Plant B (this ticket's own defense): the index lives in its own file,
    // never inside product.sqlite.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_the_index_lives_in_its_own_file_never_inside_product_sqlite() {
        let scratch = Scratch::new("own-file");
        {
            let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
                .expect("full rebuild");
        }
        assert!(
            scratch.path.join("fts.sqlite").is_file(),
            "the index must be its own file, index/fts.sqlite"
        );
        assert!(
            !scratch.path.join("product.sqlite").exists(),
            "this module must never write into product.sqlite"
        );

        // Deleting the index directory's file and rebuilding from the
        // repository (here: the same target set a caller already had)
        // reproduces the identical stored state: the derived-data property
        // the module doc's "The index is derived" names.
        let before = {
            let indexer = Indexer::open(&scratch.path).expect("reopen before delete");
            indexer.all_documents().expect("dump before delete")
        };
        fs::remove_file(scratch.path.join("fts.sqlite")).expect("delete the index file");
        let after = {
            let mut indexer = Indexer::open(&scratch.path).expect("reopen after delete");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
                .expect("rebuild from the repository");
            indexer.all_documents().expect("dump after rebuild")
        };
        assert_eq!(
            before, after,
            "deleting index/fts.sqlite and rebuilding from the repository must reproduce the \
             identical state"
        );
    }

    // -------------------------------------------------------------------
    // Plant A / FTS5 query safety: hostile query text is never interpreted
    // as FTS5 syntax, and never leaks a raw SQLite message.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_hostile_query_text_is_never_interpreted_as_fts5_syntax() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[
                doc(
                    "secret.md",
                    DocumentKind::Section,
                    "Secret",
                    "this document must never surface through a broadened search",
                ),
                doc(
                    "public.md",
                    DocumentKind::Section,
                    "Public",
                    "ordinary public content",
                ),
            ])
            .expect("full rebuild");

        let hostile = [
            "\"foo",       // unbalanced quote
            "NOT secret",  // boolean operator, unquoted would exclude
            "path:secret", // column filter, unquoted would target `path`
            "*",           // bare prefix wildcard
            "",            // empty string
        ];
        for query in hostile {
            let report = indexer
                .search(query, 10)
                .unwrap_or_else(|error| panic!("query {query:?} must not error: {error}"));
            assert!(
                report.hits.iter().all(|hit| hit.path != "secret.md"),
                "query {query:?} must never broaden into matching secret.md: {:?}",
                report.hits
            );
            assert_eq!(
                report.documents_covered, 2,
                "documents_covered must still report the real index size for query {query:?}"
            );
        }

        // An embedded NUL reaches SQLite's own parser (FTS5 stops scanning
        // at a NUL even inside a quoted literal) and must come back as a
        // typed refusal, never a panic and never the raw SQLite message.
        let nul_query = "foo\u{0}bar";
        match indexer.search(nul_query, 10) {
            Err(IndexerError::InvalidQuery { query }) => assert_eq!(query, nul_query),
            other => {
                panic!("an embedded NUL must refuse as IndexerError::InvalidQuery, got {other:?}")
            }
        }
    }

    /// The single clearest falsifiable case for the module doc's "FTS5 query
    /// safety": `OR` is FTS5's boolean operator when unquoted, so an
    /// unquoted `"alpha OR beta"` would match every document containing
    /// either word. Quoted as one literal three-token phrase, it must match
    /// only a document that actually contains that exact adjacent phrase,
    /// which neither of these two does, so it must match neither.
    #[test]
    fn ori_t_0035_boolean_looking_query_text_is_a_literal_phrase_not_an_operator() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[
                doc("a.md", DocumentKind::Section, "A", "alpha content"),
                doc("b.md", DocumentKind::Section, "B", "beta content"),
            ])
            .expect("full rebuild");

        let hits = indexer
            .search("alpha OR beta", 10)
            .expect("search runs")
            .hits;
        assert!(
            hits.is_empty(),
            "quoting must stop OR from being read as FTS5's boolean operator, which would \
             otherwise match both documents: {hits:?}"
        );
    }

    #[test]
    fn ori_t_0035_search_ranks_more_relevant_documents_first_deterministically() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[
                doc(
                    "a.md",
                    DocumentKind::Section,
                    "A",
                    "indexer indexer indexer mentioned three times",
                ),
                doc("b.md", DocumentKind::Section, "B", "indexer mentioned once"),
            ])
            .expect("full rebuild");
        let hits = indexer.search("indexer", 10).expect("search runs").hits;
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].path, "a.md",
            "the document with more matches must rank first"
        );
        assert!(
            hits[0].score <= hits[1].score,
            "bm25 orders ascending (smaller is a better match): {hits:?}"
        );
    }

    #[test]
    fn ori_t_0035_search_matches_non_ascii_terms_and_is_diacritic_insensitive() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[doc(
                "a.md",
                DocumentKind::Section,
                "Café notes",
                "discusses café culture and résumé formatting",
            )])
            .expect("full rebuild");

        let accented = indexer.search("café", 10).expect("search runs").hits;
        assert_eq!(accented.len(), 1, "the exact accented term must match");

        let unaccented = indexer.search("resume", 10).expect("search runs").hits;
        assert_eq!(
            unaccented.len(),
            1,
            "unicode61 folds diacritics, so the unaccented spelling must match résumé too"
        );
    }

    // -------------------------------------------------------------------
    // Plant C / concurrency: a second writer fails cleanly, never
    // corrupting the index.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_a_second_writer_against_the_same_on_disk_index_fails_cleanly_not_corrupting_it() {
        let scratch = Scratch::new("concurrency");
        let mut first = Indexer::open(&scratch.path).expect("first writer opens");
        first
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("seed the index");

        let mut second = Indexer::open(&scratch.path).expect("second connection opens");

        // Hold a write transaction open on `first` so `second` contends for
        // the lock.
        let tx = first
            .conn
            .transaction()
            .expect("first writer begins a transaction");
        tx.execute(
            "INSERT INTO documents (path, kind, title, body, checksum) VALUES ('b.md','section','B','beta',2)",
            [],
        )
        .expect("first writer's own insert, inside its own open transaction");

        let result = second.full_rebuild(&[doc("c.md", DocumentKind::Section, "C", "gamma")]);
        assert!(
            matches!(result, Err(IndexerError::Locked { .. })),
            "a second writer must fail cleanly with IndexerError::Locked while the first holds \
             the write lock, not hang, panic, or corrupt anything: {result:?}"
        );

        tx.commit().expect("first writer commits");
        drop(first);

        // After the first writer releases the lock, the index is intact
        // (not corrupted) and the second writer can now proceed.
        let recovered = second
            .full_rebuild(&[doc("c.md", DocumentKind::Section, "C", "gamma")])
            .expect("the second writer succeeds once the lock is free");
        assert_eq!(recovered.total, 1);
        let dump = second.all_documents().expect("read back");
        assert_eq!(dump.len(), 1);
        assert_eq!(dump[0].0.path, "c.md");
    }
}
