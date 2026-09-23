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
//! # Document identity is repository-relative, never checkout-dependent
//!
//! [`IndexableDocument::path`]'s own doc calls it "repository relative", and
//! [`Indexer::incremental_sync`] trusts that literally: two calls to
//! [`Indexer::collect_from_repo`] against the same repository content, from
//! any two checkouts (a worktree, a clone under a different directory
//! name), must produce the same paths, or an unchanged repository looks
//! changed. An adversarial review found `walk_markdown` breaking this by
//! stripping each file's path relative to whichever directory the recursion
//! was currently walking rather than `repo_root`; the fix (`repo_root`
//! threaded through every recursive call, `path.strip_prefix(repo_root)`,
//! refused rather than defaulted to an absolute path on failure) and the
//! full account of the defect are in that function's own doc.
//!
//! # Duplicate paths are refused, not silently resolved
//!
//! [`Indexer::full_rebuild`] and [`Indexer::incremental_sync`] both call
//! `refuse_duplicate_paths` before writing anything: a target set holding
//! two [`IndexableDocument`]s at one `path` is refused with
//! [`IndexerError::DuplicatePath`], not written with whichever one happens
//! to survive. An adversarial review found that, without this, the two
//! methods disagreed on such input (`full_rebuild` kept every row;
//! `incremental_sync` kept exactly one and, because its own diff runs
//! against a `BTreeMap` that already dropped the duplicate, never settled:
//! it deleted and re-inserted a different one of the two documents on every
//! call against the same unchanged target). [`Indexer::collect_from_repo`]'s
//! own walk is also fixed so it does not normally produce this: fenced code
//! is never read as a heading, and repeated headings get GitHub's
//! disambiguating suffix (`section_documents`'s own doc has the detail);
//! the refusal stands as the guard for every other caller, and for a
//! criteria table's genuinely repeated ID, which this module does not
//! silently rename.
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
//! # Unbounded query cost is refused before FTS5 ever sees it
//!
//! Quoting stops `query` from being read as FTS5 syntax; it does nothing
//! about `query`'s *cost*. An adversarial review measured one search of a
//! common word repeated to fill 1 MB costing 4.5 seconds and 2.18 GB of
//! resident memory, because FTS5 opens one index iterator per phrase term
//! and does not deduplicate repeated tokens, so cost grows with (repeated
//! terms) x (how many documents contain each). [`Indexer::search`] now
//! refuses `query` outright, before it is quoted or bound, if it exceeds
//! `MAX_QUERY_BYTES` or `count_tokens` finds more than
//! `MAX_QUERY_TOKENS`; both constants name the review's numbers and why
//! their limits hold.
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
//! # A corrupt index is reported honestly, and `full_rebuild` repairs it
//!
//! An adversarial review damaged an on-disk index directly (writing garbage
//! into its FTS5 segment pages through a second connection) and found every
//! resulting error folded into [`IndexerError::InvalidQuery`], which blamed
//! a perfectly ordinary, valid query for damage that had nothing to do with
//! it, and left `documents_covered` and `all_documents` reporting the old,
//! stale count as if nothing were wrong. `search`, `full_rebuild` and
//! `incremental_sync` now all classify a `rusqlite` failure by its SQLite
//! error code (`classify_error`, `search_error`) before choosing a variant:
//! [`IndexerError::Corrupt`] for `SQLITE_CORRUPT`, [`IndexerError::Locked`]
//! for a busy write lock, and only a bare, query-specific `SQLITE_ERROR`
//! becomes [`IndexerError::InvalidQuery`].
//!
//! A later re-verification workflow found that classification incomplete: a
//! corrupted structural record whose length prefix happens to decode to an
//! implausible size can make SQLite report `ErrorCode::OutOfMemory`
//! (`SQLITE_NOMEM`, the allocator refusing an implausible request, not an
//! actual low-memory condition) for the exact same underlying damage, and
//! neither `classify_error` nor `search_error` recognized it, so it fell
//! into the generic [`IndexerError::Sqlite`] and gave a caller no signal to
//! rebuild -- the same "damage reported as something else" class, one level
//! deeper. Rather than adding `OutOfMemory` to the list of codes read as
//! corruption (a genuine out-of-memory condition is not corruption, and
//! guessing from the error code alone would misreport one as the other),
//! `search`'s and `incremental_sync`'s own reads now settle an error that is
//! neither a clean lock nor an already-recognized `SQLITE_CORRUPT` by asking
//! FTS5 directly, on the same connection: `INSERT INTO documents(documents)
//! VALUES('integrity-check')`, the internal command FTS5 exposes for this
//! exact purpose (`diagnose_ambiguous_read_error`). Only when that check
//! also fails is the original error reported as [`IndexerError::Corrupt`];
//! when it passes, the original error is reported unchanged, so a real
//! out-of-memory condition is never relabeled as corruption it is not.
//!
//! Recovery is [`Indexer::full_rebuild`]'s job, and only its: the index is
//! derived data ("The index is derived" above), so on
//! [`IndexerError::Corrupt`] it drops and recreates the FTS5 virtual table
//! in place, inside its own write transaction, and never deletes
//! `fts.sqlite` itself. An earlier version did (delete the file and its
//! `-wal`/`-shm` side files, recreate at the same path); a re-verification
//! workflow found that deleting the file out from under every *other* open
//! connection to it silently lost that connection's acknowledged writes and
//! defeated this module's own "Concurrency" guarantee between them, so the
//! file is now repaired in place instead. `search` and `incremental_sync` do
//! not self-heal; they report [`IndexerError::Corrupt`] so a caller can
//! choose to call `full_rebuild`, rather than a read or a diff silently
//! triggering a rebuild neither asked for. Corruption found while merely
//! opening the file ([`Indexer::open`]) is classified and reported the same
//! way, but is never self-healed either: destroying a file this module was
//! only handed a path to is `ProductDb`'s decision, not `Indexer`'s.
//!
//! # Reads and writes share one transaction
//!
//! An adversarial review ran a concurrent writer against [`Indexer::search`]
//! and found `documents_covered` (from a `count(*)` statement) and the
//! `MATCH` hits (from a second, later statement) occasionally describing two
//! different committed states of the index: a "400 documents, 0 matched"
//! report when no committed state ever had that shape, exactly the false
//! clean absence the vacuity trap above exists to prevent. It found the
//! matching defect in [`Indexer::incremental_sync`]: its diff was read with
//! a separate `all_documents()` call *before* the write transaction opened,
//! so a writer that committed in that gap was neither deleted nor reported
//! as [`IndexerError::Locked`], just silently missing from the result.
//! `search` now reads its count and its hits inside one
//! `Connection::unchecked_transaction` (a read-only transaction on `&self`,
//! not `&mut self`, so concurrent reads are still unblocked from each
//! other); `incremental_sync` now reads its diff basis as the first
//! statement inside the same write transaction its deletes and upserts run
//! in, so a concurrent writer's commit either lands entirely before that
//! snapshot (seen and diffed against correctly) or entirely after (this
//! transaction's own commit orders before or after it, and SQLite's own
//! `SQLITE_BUSY_SNAPSHOT` refuses the ambiguous case as
//! [`IndexerError::Locked`]), never split across the two.
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

    /// A checksum of `kind`, `title` and `body` together, for change
    /// detection only.
    ///
    /// Every field [`Indexer::incremental_sync`] must notice a change in is
    /// hashed, `kind` included: an adversarial review (recorded in the pull
    /// request report) found that hashing only `title` and `body` let a
    /// document whose `kind` changed at a fixed path keep its old, wrong
    /// `kind` forever under `incremental_sync`, while `full_rebuild` stored
    /// the new one, breaking the one property this module exists to prove.
    /// `path` is deliberately not hashed: it is the row's identity
    /// (`IndexableDocument::path`'s own doc), not one of its changeable
    /// fields, and hashing it would make a rename look like a same-path edit
    /// to no useful effect since callers already diff by `path` directly.
    ///
    /// `std::hash::Hasher`, not a cryptographic digest: nothing here needs a
    /// security property (this is "did the row change", not an integrity
    /// check against a hostile actor), and a hashing crate of its own is
    /// outside this ticket's approved dependency (`rusqlite`, already
    /// pinned by `ori-store`; see `crates/ori-memory/Cargo.toml`'s comment).
    /// Stored as a SQLite `INTEGER` via a bitwise `as i64` reinterpretation
    /// of the `u64` value (exact and lossless both ways; SQLite's own
    /// integer column is signed 64-bit, so this is the same trick
    /// `checksum() as i64` / `.. as u64` uses at every call site below).
    fn checksum(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.kind.as_str().hash(&mut hasher);
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
    /// `path` is damaged: either SQLite itself reported
    /// `ErrorCode::DatabaseCorrupt` directly, or a read got some other
    /// ambiguous error (round 4's own finding: a corrupted length prefix
    /// can make SQLite report `ErrorCode::OutOfMemory` instead) and FTS5's
    /// own `integrity-check` command, asked directly, agreed
    /// (`diagnose_ambiguous_read_error`). [`Indexer::full_rebuild`]
    /// recovers from this by dropping and recreating the FTS5 virtual
    /// table in place, inside its own write transaction (the module doc's
    /// "A corrupt index is reported honestly, and `full_rebuild` repairs
    /// it"); `path` is never deleted. [`Indexer::search`] and
    /// [`Indexer::incremental_sync`] report it rather than silently
    /// widening or narrowing what they return, since neither is the
    /// reconstruction step; corruption found while opening the file at all
    /// ([`Indexer::open`]) is reported the same way but is never
    /// self-healed, since destroying a file the caller handed a path to is
    /// `ProductDb`'s decision, not this module's.
    Corrupt {
        /// The database file SQLite reported corrupt.
        path: PathBuf,
        /// The underlying error.
        source: rusqlite::Error,
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
    /// is echoed back to whoever sent the query. Distinct from
    /// [`IndexerError::Corrupt`] and [`IndexerError::Locked`]: an
    /// adversarial review found an earlier version of this module folding
    /// every failure while a `MATCH` ran into this variant, which blamed a
    /// perfectly valid query for a corrupt index; `search_error` now
    /// checks the SQLite error code before choosing a variant.
    InvalidQuery {
        /// The text that could not be searched safely.
        query: String,
    },
    /// `query`'s byte length exceeds `MAX_QUERY_BYTES`, or it tokenizes to
    /// more than `MAX_QUERY_TOKENS` terms: the module doc's "Unbounded
    /// query cost". Refused before FTS5 ever sees it, so the cost this
    /// guards against is never paid.
    QueryTooLarge {
        /// `query`'s length in bytes.
        byte_len: usize,
        /// How many alphanumeric runs (this module's own coarse token
        /// count, `count_tokens`) `query` has.
        token_count: usize,
    },
    /// `documents` (the target set given to [`Indexer::full_rebuild`] or
    /// [`Indexer::incremental_sync`]) held the same `path` more than once.
    /// Refused rather than silently keeping whichever of the two happened
    /// to be written last: an adversarial review found that
    /// `full_rebuild` and `incremental_sync` disagreed on such a set (the
    /// former kept every row, the latter kept one and never settled on
    /// repeated syncs of the same unchanged target).
    DuplicatePath {
        /// The path that appeared more than once.
        path: String,
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
            Self::Corrupt { path, source } => write!(
                f,
                "{} is corrupt: {source}; the index is derived data, rebuild it with \
                 Indexer::full_rebuild",
                path.display()
            ),
            Self::Sqlite { context, source } => write!(f, "{context}: {source}"),
            Self::InvalidQuery { query } => write!(f, "invalid query: {query}"),
            Self::QueryTooLarge {
                byte_len,
                token_count,
            } => write!(
                f,
                "query is too large: {byte_len} bytes (max {MAX_QUERY_BYTES}), {token_count} \
                 tokens (max {MAX_QUERY_TOKENS})"
            ),
            Self::DuplicatePath { path } => write!(
                f,
                "{path} appears more than once in the target set; full_rebuild and \
                 incremental_sync both refuse rather than guess which one should win"
            ),
        }
    }
}

impl std::error::Error for IndexerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Directory { source, .. } | Self::Io { source, .. } => Some(source),
            Self::Corrupt { source, .. } | Self::Sqlite { source, .. } => Some(source),
            Self::Locked { .. }
            | Self::InvalidQuery { .. }
            | Self::QueryTooLarge { .. }
            | Self::DuplicatePath { .. } => None,
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

/// Whether a `rusqlite` failure is SQLite reporting `ErrorCode::DatabaseCorrupt`
/// (`SQLITE_CORRUPT`): the check that routes a damaged index to
/// [`IndexerError::Corrupt`] instead of [`IndexerError::Sqlite`] or (worse,
/// the defect an adversarial review found) [`IndexerError::InvalidQuery`].
fn is_corrupt(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _) if inner.code == ErrorCode::DatabaseCorrupt
    )
}

/// Whether a `rusqlite` failure is SQLite reporting a bare `SQLITE_ERROR`
/// (`ErrorCode::Unknown`, primary result code 1: rusqlite's own `ErrorCode`
/// maps every primary code it does not otherwise name to this one variant,
/// and `SQLITE_ERROR` is one of those). [`Indexer::search`] runs exactly one
/// fixed, already-tested SQL statement whose only caller-influenced part is
/// the `MATCH` argument, so a bare `SQLITE_ERROR` surfacing from that one
/// statement is FTS5's own query-syntax parser refusing the (quoted, but
/// still occasionally invalid, the embedded-`NUL` case) phrase, not a schema
/// or connection problem; those come back with a more specific `ErrorCode`
/// and are handled before this check runs (see `search_error`).
fn is_query_syntax_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _) if inner.code == ErrorCode::Unknown
    )
}

/// Maps any `rusqlite` failure to the named refusal it belongs to: a locked
/// file to [`IndexerError::Locked`], a damaged one to
/// [`IndexerError::Corrupt`], everything else to [`IndexerError::Sqlite`].
/// Used for every read and every write this module makes, so the same
/// failure is classified the same way wherever it happens: an adversarial
/// review found the previous version reaching this decision differently in
/// different call sites (`search` folded corruption into
/// [`IndexerError::InvalidQuery`], `total_indexed` did not check for a lock
/// at all).
fn classify_error(path: &Path, context: &str, source: rusqlite::Error) -> IndexerError {
    if is_locked(&source) {
        IndexerError::Locked {
            path: path.to_owned(),
        }
    } else if is_corrupt(&source) {
        IndexerError::Corrupt {
            path: path.to_owned(),
            source,
        }
    } else {
        IndexerError::sqlite(context, source)
    }
}

/// [`Indexer::search`]'s own classifier: the same three cases
/// [`classify_error`] names, plus [`IndexerError::InvalidQuery`] for a bare
/// `SQLITE_ERROR` ([`is_query_syntax_error`]), which `classify_error` alone
/// would fold into [`IndexerError::Sqlite`]. Checked in this order because a
/// lock or a corrupt page can itself present as a generic `SQLITE_ERROR` in
/// some SQLite versions, and a real lock or real corruption must never be
/// misreported as the caller's query being unsafe. Falls back to
/// [`diagnose_ambiguous_read_error`], not directly to [`IndexerError::Sqlite`],
/// for the same reason `total_indexed_on` and `current_paths_and_checksums_on`
/// do: see that function's doc.
fn search_error(
    conn: &Connection,
    path: &Path,
    query: &str,
    source: rusqlite::Error,
) -> IndexerError {
    if is_locked(&source) {
        return IndexerError::Locked {
            path: path.to_owned(),
        };
    }
    if is_corrupt(&source) {
        return IndexerError::Corrupt {
            path: path.to_owned(),
            source,
        };
    }
    if is_query_syntax_error(&source) {
        return IndexerError::InvalidQuery {
            query: query.to_owned(),
        };
    }
    diagnose_ambiguous_read_error(conn, path, "search", source)
}

/// Runs FTS5's own consistency check (`INSERT INTO documents(documents)
/// VALUES('integrity-check')`, the internal command FTS5 exposes for
/// exactly this) to settle a read failure that is neither a lock
/// ([`is_locked`]) nor something [`is_corrupt`] already recognized, rather
/// than guessing from the error code alone: a re-verification workflow
/// found that a structural record whose length prefix happens to decode to
/// an implausible size can make SQLite report `ErrorCode::OutOfMemory`
/// (`SQLITE_NOMEM`, the allocator refusing an implausible request, not an
/// actual low-memory condition) instead of `ErrorCode::DatabaseCorrupt` for
/// the exact same underlying damage; `classify_error` and `search_error`
/// alone would fold that into [`IndexerError::Sqlite`] and give a caller no
/// signal to rebuild, the "damage reported as something else" class an
/// earlier adversarial review already found and fixed once for
/// [`IndexerError::InvalidQuery`] (round 2, finding 5).
///
/// This never infers corruption from the error code alone: a genuine
/// out-of-memory condition is not corruption and must not be treated as
/// one, so this asks FTS5 directly, on the same connection (or the same
/// transaction, which derefs to one) the failing read itself used, and
/// reports [`IndexerError::Corrupt`] only when FTS5's own check also
/// fails, keeping the original `source` -- not the integrity check's own
/// error -- as the diagnostic either way. Checks [`is_locked`] and
/// [`is_corrupt`] first, the same as [`classify_error`], so this is a
/// complete drop-in classifier for a read, not just the ambiguous-error
/// tail of one; [`Indexer::total_indexed_on`] (search's first read) and
/// [`Indexer::current_paths_and_checksums_on`] (`incremental_sync`'s diff
/// read) both call this directly, and `search_error` above falls back to
/// it only after its own query-syntax check, so the two checks this
/// repeats there are redundant but harmless.
fn diagnose_ambiguous_read_error(
    conn: &Connection,
    path: &Path,
    context: &str,
    source: rusqlite::Error,
) -> IndexerError {
    if is_locked(&source) {
        return IndexerError::Locked {
            path: path.to_owned(),
        };
    }
    if is_corrupt(&source) {
        return IndexerError::Corrupt {
            path: path.to_owned(),
            source,
        };
    }
    match conn.execute_batch("INSERT INTO documents(documents) VALUES('integrity-check');") {
        Ok(()) => IndexerError::sqlite(context, source),
        Err(_) => IndexerError::Corrupt {
            path: path.to_owned(),
            source,
        },
    }
}

/// Refuses `documents` if any two elements share a `path`: the identity
/// [`Indexer::full_rebuild`] and [`Indexer::incremental_sync`] both key on.
/// Checked before either does any write, so a caller sees the refusal
/// before any partial effect.
///
/// # Errors
///
/// [`IndexerError::DuplicatePath`] naming the first path seen twice, in
/// `documents`' own order.
fn refuse_duplicate_paths(documents: &[IndexableDocument]) -> Result<(), IndexerError> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for document in documents {
        if !seen.insert(document.path.as_str()) {
            return Err(IndexerError::DuplicatePath {
                path: document.path.clone(),
            });
        }
    }
    Ok(())
}

/// Quotes `text` as one FTS5 string literal, doubling any embedded `"`
/// (SQLite's own escaping rule for a quoted string): the module doc's "FTS5
/// query safety". Turns arbitrary caller text into a single literal phrase
/// token, never FTS5's own query syntax.
fn quote_fts5_phrase(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The largest `query` [`Indexer::search`] accepts, in bytes: an adversarial
/// review measured 1 MB of a common repeated term costing 4.5 seconds and
/// 2.18 GB of resident memory, because FTS5 opens one index iterator per
/// phrase term and repeated tokens are not deduplicated. 1 KiB is generous
/// for a legitimate search phrase (`spec/API_SPEC.md`'s `aicd_search(query)`
/// is a short free-text query, never a document body) and keeps the worst
/// case, even for the cheapest-per-byte common term the review measured
/// (roughly 8.7 KB of resident memory per repeated `"the "`), in the tens of
/// megabytes rather than gigabytes.
const MAX_QUERY_BYTES: usize = 1024;

/// The largest number of tokens [`count_tokens`] may count in `query`: a
/// second bound, additional to [`MAX_QUERY_BYTES`] and never the one this
/// module trusts to be exact.
///
/// [`MAX_QUERY_BYTES`] alone already bounds `search`'s worst case (an
/// adversarial review measured it, at this cap, in the tens of milliseconds
/// and single-digit megabytes even for the specific inputs built to defeat
/// `count_tokens`, below), so this second cap is belt-and-braces, not the
/// property this module depends on for safety. A second review found
/// `count_tokens`'s approximation of `unicode61` wrong in both directions,
/// for example NFD-normalized Latin text (`"résumé"`, common from macOS
/// input) over-counting by treating each combining accent as its own token
/// boundary, and refusing some legitimate queries below the stated cap as
/// a result. [`is_combining_mark`] closes that specific, common case
/// (`char::is_alphanumeric` already agrees with `unicode61` on plain ASCII,
/// which is why the existing byte-repetition tests below are unaffected).
/// It does not close every case: FTS5's own token/separator boundary for
/// vowel-sign marks in scripts such as Thai and Devanagari, and for a few
/// symbol categories such as circled Latin letters, still disagrees with
/// `char::is_alphanumeric` in the other direction (undercounting, letting
/// more real FTS5 terms through than this cap's name promises). Closing
/// that fully would mean either running `unicode61` itself inside `search`
/// (which the module's own "Reads and writes share one transaction"
/// section would then have to account for, since tokenizing through a
/// scratch FTS5 table is a write, not a read) or hand-carrying the Unicode
/// standard's exact General_Category tables with no crate approved to
/// supply them (`crates/ori-memory/Cargo.toml`'s own comment scopes this
/// ticket's dependency to `rusqlite`). Given [`MAX_QUERY_BYTES`] already
/// bounds the cost this cap exists to bound, that residual gap is accepted
/// and stated here rather than hidden.
const MAX_QUERY_TOKENS: usize = 64;

/// Whether `character` is a Unicode combining mark in one of the blocks
/// dedicated to them (Combining Diacritical Marks and its three
/// supplements, and Combining Half Marks): not the Unicode standard's full
/// `Mn`/`Mc` General_Category (which also reaches into many scripts' own
/// blocks, the residual [`MAX_QUERY_TOKENS`] names), only enough to keep
/// `count_tokens` from splitting the tokens NFD normalization most often
/// produces, one base letter followed by one or more combining accents.
#[rustfmt::skip]
const fn is_combining_mark(character: char) -> bool {
    matches!(character as u32,
        0x0300..=0x036F // Combining Diacritical Marks
        | 0x1AB0..=0x1AFF // Combining Diacritical Marks Extended
        | 0x1DC0..=0x1DFF // Combining Diacritical Marks Supplement
        | 0x20D0..=0x20FF // Combining Diacritical Marks for Symbols
        | 0xFE20..=0xFE2F // Combining Half Marks
    )
}

/// This module's own coarse token count: the number of maximal runs of
/// `char::is_alphanumeric` characters in `text`, with a run continuing
/// (never restarting) across a combining mark ([`is_combining_mark`]) so a
/// base letter plus its accents still counts as the one token `unicode61`
/// would tokenize it as. Not FTS5's own tokenizer, and not claimed to be
/// exact in every case; see [`MAX_QUERY_TOKENS`]'s own doc for what this
/// still gets wrong and why that is accepted.
fn count_tokens(text: &str) -> usize {
    let mut count = 0usize;
    let mut in_token = false;
    for character in text.chars() {
        if character.is_alphanumeric() {
            if !in_token {
                count += 1;
                in_token = true;
            }
        } else if !is_combining_mark(character) {
            in_token = false;
        }
    }
    count
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
    /// [`IndexerError::Directory`] if `dir` cannot be created or resolved to
    /// an absolute, canonical path;
    /// [`IndexerError::Corrupt`] if the file exists and SQLite reports it
    /// damaged while opening it (a corrupt header, a corrupt
    /// `sqlite_schema` page, or similar); this is not self-healed, see the
    /// module doc's "Corruption found while opening";
    /// [`IndexerError::Sqlite`] if the file cannot be opened or the schema
    /// cannot be created for another reason.
    pub fn open(dir: &Path) -> Result<Self, IndexerError> {
        std::fs::create_dir_all(dir).map_err(|source| IndexerError::Directory {
            path: dir.to_owned(),
            source,
        })?;
        // Canonical and absolute, resolved once, here: never a caller's
        // possibly-relative `dir` re-resolved later against whatever the
        // process's current working directory happens to be by then. See
        // the module doc's "Corruption found while opening" for why this
        // matters even though this module no longer deletes files by path.
        let canonical_dir = dir
            .canonicalize()
            .map_err(|source| IndexerError::Directory {
                path: dir.to_owned(),
                source,
            })?;
        let path = canonical_dir.join("fts.sqlite");
        let conn =
            Connection::open(&path).map_err(|source| classify_error(&path, "open", source))?;
        Self::configure(conn, Some(path))
    }

    /// An index held only in memory, for tests and for any caller that
    /// wants no on-disk footprint.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Sqlite`] if the schema cannot be created.
    pub fn open_in_memory() -> Result<Self, IndexerError> {
        let display_path = PathBuf::from(":memory:");
        let conn = Connection::open_in_memory().map_err(|source| {
            classify_error(&display_path, "open an in-memory database", source)
        })?;
        Self::configure(conn, None)
    }

    /// Sets this connection's pragmas and creates the schema if absent.
    ///
    /// Every failure here is classified with [`classify_error`], not the
    /// generic [`IndexerError::sqlite`]: an adversarial review found
    /// `Indexer::open` reporting a corrupt file (`SQLITE_CORRUPT`, most
    /// often surfacing at the `journal_mode` pragma, the first real
    /// statement run against it) as a bare [`IndexerError::Sqlite`], which
    /// made `full_rebuild`'s recovery unreachable for exactly the damage
    /// shapes (a corrupt header or `sqlite_schema` page, a truncated file)
    /// that keep the file from opening at all. Classifying it as
    /// [`IndexerError::Corrupt`] here does not, by itself, repair anything;
    /// see "Corruption found while opening" in the module doc for what
    /// this module does and does not do about it.
    fn configure(conn: Connection, path: Option<PathBuf>) -> Result<Self, IndexerError> {
        let display_path = path.clone().unwrap_or_else(|| PathBuf::from(":memory:"));
        // WAL: concurrent readers never block a writer or each other (the
        // same choice `crates/ori-store/src/db.rs`'s `open_connection` makes
        // for `product.sqlite`). A zero busy_timeout is set explicitly,
        // deliberately, rather than left at SQLite's own default: see the
        // module doc's "Concurrency".
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|source| classify_error(&display_path, "set journal_mode", source))?;
        conn.busy_timeout(WRITE_BUSY_TIMEOUT)
            .map_err(|source| classify_error(&display_path, "set busy_timeout", source))?;
        conn.execute_batch(CREATE_TABLE_SQL).map_err(|source| {
            classify_error(&display_path, "create the documents table", source)
        })?;
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
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the read fails for another reason.
    pub fn all_documents(&self) -> Result<Vec<(IndexableDocument, u64)>, IndexerError> {
        Self::all_documents_on(&self.conn, &self.display_path())
    }

    /// [`Indexer::all_documents`]'s body, taking any `&Connection` (an
    /// ordinary connection or a `&rusqlite::Transaction`, which derefs to
    /// one), so [`Indexer::incremental_sync`] can read the diff basis inside
    /// its own write transaction: the module doc's "Reads and writes share
    /// one transaction".
    fn all_documents_on(
        conn: &Connection,
        display_path: &Path,
    ) -> Result<Vec<(IndexableDocument, u64)>, IndexerError> {
        let mut statement = conn
            .prepare("SELECT path, kind, title, body, checksum FROM documents ORDER BY path")
            .map_err(|source| {
                classify_error(display_path, "prepare a full read of documents", source)
            })?;
        let rows = statement
            .query_map([], Self::row_to_document)
            .map_err(|source| classify_error(display_path, "read every document back", source))?;
        let mut out = Vec::new();
        for row in rows {
            let pair = row
                .map_err(|source| classify_error(display_path, "read one document row", source))?;
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
    /// [`IndexReport::total`]). Takes any `&Connection` (an ordinary
    /// connection, for [`Indexer::search`]'s own read transaction, or a
    /// `&rusqlite::Transaction`, which derefs to one, for
    /// [`Indexer::full_rebuild`] and [`Indexer::incremental_sync`] to read
    /// `total` inside their own write transaction, before commit: the
    /// module doc's "Reads and writes share one transaction").
    ///
    /// # Errors
    ///
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the read fails for another reason.
    fn total_indexed_on(conn: &Connection, display_path: &Path) -> Result<usize, IndexerError> {
        let total: i64 = conn
            .query_row("SELECT count(*) FROM documents", [], |row| row.get(0))
            .map_err(|source| {
                diagnose_ambiguous_read_error(conn, display_path, "count documents", source)
            })?;
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
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the write fails for another reason.
    pub fn add_or_replace(&mut self, document: &IndexableDocument) -> Result<(), IndexerError> {
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| classify_error(&path, "begin transaction", source))?;
        Self::delete_path(&tx, &document.path, &path)?;
        Self::insert(&tx, document, &path)?;
        tx.commit()
            .map_err(|source| classify_error(&path, "commit", source))
    }

    /// Clears the index and indexes exactly `documents`: a full rebuild, the
    /// reconstruction the module doc's "the index is derived" section names.
    ///
    /// Repairs shadow-table corruption unconditionally, as a side effect of
    /// doing its own ordinary job, never by deleting a file: inside one
    /// write transaction, this drops the `documents` table (which drops
    /// every FTS5 shadow table with it, `documents_data` included, whatever
    /// state they were in) and recreates it before inserting. SQLite's own
    /// locking on the transaction serializes this against every other
    /// connection the ordinary way, so nobody else ever sees, writes into,
    /// or gets orphaned by a deleted or replaced file, because no file is
    /// ever deleted or replaced: only its own already-open, already-locked
    /// database's own tables change. An adversarial review found the
    /// previous version of this recovery (deleting and recreating the file
    /// on disk) both failed to repair the exact damage it was built for
    /// (a write only reads a segment it happens to touch, so most pages'
    /// damage survived a rebuild that returned `Ok`) and, far more
    /// seriously, orphaned every other open connection to the file it
    /// deleted, silently losing their acknowledged writes and defeating the
    /// module's own "Concurrency" guarantee between them; see the module
    /// doc's "A corrupt index is repaired in place, never by deleting a
    /// file" for the full account and why `DROP`/`CREATE` replaces it
    /// rather than refines it.
    ///
    /// # Errors
    ///
    /// [`IndexerError::DuplicatePath`] if `documents` holds one path twice;
    /// [`IndexerError::Locked`] if another writer holds the lock;
    /// [`IndexerError::Sqlite`] if a write fails for another reason.
    pub fn full_rebuild(
        &mut self,
        documents: &[IndexableDocument],
    ) -> Result<IndexReport, IndexerError> {
        refuse_duplicate_paths(documents)?;
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| classify_error(&path, "begin transaction", source))?;
        tx.execute_batch("DROP TABLE documents;")
            .map_err(|source| {
                classify_error(&path, "drop the documents table for a full rebuild", source)
            })?;
        tx.execute_batch(CREATE_TABLE_SQL).map_err(|source| {
            classify_error(
                &path,
                "recreate the documents table for a full rebuild",
                source,
            )
        })?;
        for document in documents {
            Self::insert(&tx, document, &path)?;
        }
        // Read inside the transaction, before commit: see the module doc's
        // "Reads and writes share one transaction" and the incremental_sync
        // doc below for the same fix to the same shape of bug.
        let total = Self::total_indexed_on(&tx, &path)?;
        tx.commit()
            .map_err(|source| classify_error(&path, "commit", source))?;
        Ok(IndexReport {
            upserted: documents.len(),
            removed: 0,
            total,
        })
    }

    /// Re-indexes to match `documents` exactly ("re-index on merge", PRD
    /// K-02): every path in the index but not in `documents` is deleted;
    /// every path in `documents` that is new, or whose checksum differs from
    /// what is on disk, is written (delete-then-add); every path present and
    /// unchanged is left alone.
    ///
    /// The diff basis is every stored `path` and `checksum`, decided by
    /// `path` alone, never by whether the stored `kind` happens to parse:
    /// an adversarial review found the previous version building the diff
    /// from `all_documents_on`, which silently drops a row whose
    /// `kind` text is not one of the four this build knows (a newer build's
    /// fifth kind, or a stray case change written by something other than
    /// this API). A dropped row is invisible to the removal loop below, so
    /// it survived every sync forever while [`Indexer::full_rebuild`]
    /// correctly dropped it. `current_paths_and_checksums_on`
    /// reads only `path` and `checksum`, neither of which needs `kind` to
    /// parse, so such a row is removed exactly like any other absent from
    /// `documents`, and rewritten (its checksum will not match a real
    /// target document's) exactly like any other present in it.
    ///
    /// Reads the current index inside the same write transaction the deletes
    /// and upserts run in, not before it: an adversarial review found that
    /// reading the diff basis with a separate, earlier `all_documents()`
    /// call let a concurrent writer's commit land in the gap between that
    /// read and this transaction's start, silently lost rather than either
    /// applied or refused. Also correct as the first call after opening an
    /// existing on-disk index in a fresh process, for the same reason
    /// [`Indexer::all_documents`]'s doc gives: nothing here is read from
    /// bookkeeping held in memory.
    ///
    /// # Errors
    ///
    /// [`IndexerError::DuplicatePath`] if `documents` holds one path twice;
    /// [`IndexerError::Locked`] if another writer holds the lock, or holds
    /// it by the time this call's reads try to become writes;
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if reading the current state or a write
    /// fails for another reason.
    pub fn incremental_sync(
        &mut self,
        documents: &[IndexableDocument],
    ) -> Result<IndexReport, IndexerError> {
        refuse_duplicate_paths(documents)?;
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| classify_error(&path, "begin transaction", source))?;

        // Read inside the transaction just opened, not before it: see the
        // doc above. Keyed by path and checksum only, not by parsed kind:
        // see the doc above for why.
        let current_checksums = Self::current_paths_and_checksums_on(&tx, &path)?;
        let target_paths: BTreeMap<&str, &IndexableDocument> = documents
            .iter()
            .map(|document| (document.path.as_str(), document))
            .collect();

        let mut removed = 0usize;
        for existing_path in current_checksums.keys() {
            if !target_paths.contains_key(existing_path.as_str()) {
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

        // Read inside the transaction, before commit: see full_rebuild's
        // same fix, above.
        let total = Self::total_indexed_on(&tx, &path)?;
        tx.commit()
            .map_err(|source| classify_error(&path, "commit", source))?;
        Ok(IndexReport {
            upserted,
            removed,
            total,
        })
    }

    /// Every stored `path` paired with its `checksum`, regardless of
    /// whether the row's stored `kind` parses: the diff basis
    /// [`Indexer::incremental_sync`] uses, deliberately not
    /// [`Indexer::all_documents_on`] (which silently drops a row with an
    /// unparseable `kind`; see `incremental_sync`'s own doc for why that
    /// makes it the wrong source for a removal decision).
    fn current_paths_and_checksums_on(
        conn: &Connection,
        display_path: &Path,
    ) -> Result<BTreeMap<String, u64>, IndexerError> {
        let mut statement = conn
            .prepare("SELECT path, checksum FROM documents")
            .map_err(|source| {
                diagnose_ambiguous_read_error(
                    conn,
                    display_path,
                    "prepare a read of paths and checksums",
                    source,
                )
            })?;
        let rows = statement
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let checksum: i64 = row.get(1)?;
                Ok((path, checksum as u64))
            })
            .map_err(|source| {
                diagnose_ambiguous_read_error(
                    conn,
                    display_path,
                    "read paths and checksums",
                    source,
                )
            })?;
        let mut out = BTreeMap::new();
        for row in rows {
            let (path, checksum) = row.map_err(|source| {
                diagnose_ambiguous_read_error(
                    conn,
                    display_path,
                    "read one path/checksum row",
                    source,
                )
            })?;
            out.insert(path, checksum);
        }
        Ok(out)
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
        .map_err(|source| classify_error(display_path, "delete one document", source))?;
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
        .map_err(|source| classify_error(display_path, "insert one document", source))?;
        Ok(())
    }

    /// Full-text search over `title` and `body`, at most `limit` hits
    /// (`limit` is honoured exactly, including zero), best-scored (lowest
    /// `bm25()`) first, ties broken by `path` ascending so the same query
    /// gives the same order every time. `query` is always quoted as one
    /// FTS5 literal phrase before being bound; see the module doc's "FTS5
    /// query safety".
    ///
    /// An empty `query` (after trimming) returns every stored document's
    /// zero matches directly, without asking FTS5 to parse a degenerate
    /// phrase. `documents_covered` and the `MATCH` itself are read inside
    /// one read transaction, so they always describe the same snapshot: the
    /// module doc's "Reads and writes share one transaction" names why a
    /// caller must never see a count that another connection's commit has
    /// already invalidated by the time the hits come back.
    ///
    /// # Errors
    ///
    /// [`IndexerError::QueryTooLarge`] if `query` exceeds
    /// `MAX_QUERY_BYTES` or `MAX_QUERY_TOKENS` (the module doc's
    /// "Unbounded query cost"), checked before FTS5 ever sees it;
    /// [`IndexerError::InvalidQuery`] if `query`, even quoted, cannot be
    /// searched safely (the module doc's embedded-`NUL` case);
    /// [`IndexerError::Locked`] if a concurrent write holds the lock;
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the search fails for another reason.
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchReport, IndexerError> {
        if query.len() > MAX_QUERY_BYTES {
            return Err(IndexerError::QueryTooLarge {
                byte_len: query.len(),
                token_count: count_tokens(query),
            });
        }
        let token_count = count_tokens(query);
        if token_count > MAX_QUERY_TOKENS {
            return Err(IndexerError::QueryTooLarge {
                byte_len: query.len(),
                token_count,
            });
        }

        let path = self.display_path();
        let tx = self.conn.unchecked_transaction().map_err(|source| {
            classify_error(&path, "begin a read transaction for search", source)
        })?;
        let total = Self::total_indexed_on(&tx, &path)?;
        if query.trim().is_empty() {
            return Ok(SearchReport {
                hits: Vec::new(),
                documents_covered: total,
            });
        }

        let quoted = quote_fts5_phrase(query);
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = tx
            .prepare(
                "SELECT path, kind, title, bm25(documents) AS rank \
                 FROM documents WHERE documents MATCH ?1 \
                 ORDER BY rank ASC, path ASC LIMIT ?2",
            )
            .map_err(|source| classify_error(&path, "prepare a search", source))?;
        // `query_map` itself only prepares the row-mapping closure; FTS5's
        // own query-syntax errors (the embedded-NUL case this module's
        // tests reach), a locked write and a corrupt page all surface
        // lazily, while the returned iterator is stepped below, not here.
        // Either failure point goes through `search_error`, which tells
        // them apart rather than blaming every one of them on the query.
        let rows = statement
            .query_map(params![quoted, limit], |row| {
                let path: String = row.get(0)?;
                let kind_text: String = row.get(1)?;
                let title: String = row.get(2)?;
                let score: f64 = row.get(3)?;
                Ok((path, kind_text, title, score))
            })
            .map_err(|source| search_error(&tx, &path, query, source))?;

        let mut hits = Vec::new();
        for row in rows {
            let (path_hit, kind_text, title, score) =
                row.map_err(|source| search_error(&tx, &path, query, source))?;
            if let Some(kind) = DocumentKind::parse(&kind_text) {
                hits.push(SearchHit {
                    path: path_hit,
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
    /// [`IndexerError::Io`] if a directory or file cannot be read, or if a
    /// file this walk finds is not actually under `repo_root` (a symlink
    /// escaping it, most plausibly; the module never synthesizes an
    /// absolute-path identity as a fallback, see `walk_markdown`'s doc).
    pub fn collect_from_repo(repo_root: &Path) -> Result<Vec<IndexableDocument>, IndexerError> {
        let spec_dir = repo_root.join("spec");
        let mut out = Vec::new();
        walk_markdown(repo_root, &spec_dir, &mut out)?;
        Ok(out)
    }
}

/// Recursively walks `dir` for `.md` files, skipping the whole `spec/design`
/// subtree by its exact repository-relative path, and appends every
/// document found to `out`, with `path` computed relative to `repo_root`,
/// never to `dir` itself.
///
/// An adversarial review found the previous version stripped each file's
/// path relative to `dir.parent().parent()`, where `dir` is whichever
/// directory the recursion is currently walking, not `repo_root`. That kept
/// only the file's last three path components, so a top-level `spec/*.md`
/// file carried the checkout directory's own name as a prefix (identical
/// content in two checkouts of the same repository, or in a worktree versus
/// the main checkout, got two different identities), and a file three or
/// more levels under `spec/` lost its leading `spec/` components entirely,
/// so two unrelated files could collide on one path. Passing `repo_root`
/// down through every recursive call and stripping against it, once, fixes
/// this: `relative` is always exactly the path under `repo_root`.
///
/// A second review found two narrower defects this rewrite did not reach.
/// First, `is_adr`/`is_criteria` (below) still read `relative_path`'s
/// components for `adr` or `criteria` *anywhere*, not only in the second
/// position `collect_from_repo`'s own doc names (`spec/adr/*.md`,
/// `spec/criteria/*.md`): an ordinary prose document nested under a
/// `criteria`-named directory that is not `spec/criteria/` was silently
/// parsed as a criteria table and dropped out of the index entirely when
/// it matched no `ORI-` row (the test fixture below spells the exact
/// path). Classification now
/// checks the exact shape, `["spec", "adr", filename]` or `["spec",
/// "criteria", filename]`, three components with `adr`/`criteria` in the
/// second one, not a components-contains check. Second, the `spec/design`
/// skip matched any directory named `design` at any depth (excluding, for
/// example, a hypothetical `spec/runbooks/design/`, which the module doc
/// never asked it to touch, and which is not what "`spec/design/` is
/// skipped entirely" means), and it was checked directory by directory
/// during the recursion rather than against the exact relative path,
/// which is a narrower rule than "the whole `spec/design` subtree, and
/// nothing else". It now compares the directory's own repository-relative
/// path to `spec/design` exactly, still skipping the whole subtree (this
/// function never recurses past a match), and now the test
/// `tests::ori_t_0035_collect_from_repo_excludes_a_nested_markdown_file_under_spec_design`
/// plants a file two levels deep to prove that directly, not only at the
/// top level.
fn walk_markdown(
    repo_root: &Path,
    dir: &Path,
    out: &mut Vec<IndexableDocument>,
) -> Result<(), IndexerError> {
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
            let relative_dir = path.strip_prefix(repo_root).unwrap_or(&path);
            if relative_dir == Path::new("spec").join("design") {
                // E-0006: spec/design/ is mock data for a fictional product.
                // See the module doc, "spec/design/ is excluded". The exact
                // repository-relative path, not merely the directory's own
                // name, so a same-named directory elsewhere in the tree is
                // untouched.
                continue;
            }
            walk_markdown(repo_root, &path, out)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|source| IndexerError::Io {
            path: path.clone(),
            source,
        })?;
        // Refused, never defaulted to the absolute path: an identity that
        // silently became absolute the one time stripping failed would
        // reintroduce exactly the checkout-dependent-identity defect this
        // rewrite exists to remove.
        let relative_path = path
            .strip_prefix(repo_root)
            .map_err(|_source| IndexerError::Io {
                path: path.clone(),
                source: std::io::Error::other(format!(
                    "{} is not under repo_root {}; refusing to synthesize an absolute-path \
                     identity for it",
                    path.display(),
                    repo_root.display()
                )),
            })?;
        let relative = relative_path.to_string_lossy().replace('\\', "/");
        let components: Vec<&str> = relative_path
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect();
        let is_adr = components.len() == 3 && components[0] == "spec" && components[1] == "adr";
        let is_criteria =
            components.len() == 3 && components[0] == "spec" && components[1] == "criteria";
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
///
/// Two defects an adversarial review found are fixed here. First, a line
/// starting with `#` *inside a fenced code block* (three or more backticks)
/// is not a heading: a `# install` comment inside a shell snippet no longer
/// splits the document or contributes an anchor. Second, two headings whose
/// text produces the same anchor (an exact repeat, such as two `## Steps`
/// in one file) are disambiguated deterministically, GitHub's own actual
/// convention: keep incrementing a `-N` suffix until an anchor no earlier
/// heading in this file has already claimed. A second review found this
/// module's first attempt at that convention checked only a per-base
/// counter, not the set of anchors already assigned, so a heading whose
/// own text already ends in a numeral collided with the suffix a repeat
/// produced (`Phase 1`, `Phase 1.1`, `Phase 1` gave `phase-1`, `phase-1-1`,
/// `phase-1-1`, a second collision from the very fix meant to remove the
/// first one). `used_anchors` below tracks the whole set, so the third
/// heading here is tried against `phase-1` (taken), then `phase-1-1`
/// (also taken), then `phase-1-2` (free), matching what GitHub's own
/// slugger does. Without either fix, two documents could carry the same
/// `path`, which [`Indexer::full_rebuild`] and [`Indexer::incremental_sync`]
/// now refuse outright ([`refuse_duplicate_paths`]) rather than silently
/// keep only one of, so a collision this scheme still produced would fail
/// indexing the whole target set, not only the one colliding document; see
/// `ori_t_0035_full_rebuild_and_incremental_sync_agree_on_this_repositorys_own_spec_tree`
/// for the check that this module's own `spec/` never hits one.
fn section_documents(relative_path: &str, text: &str) -> Vec<IndexableDocument> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_body = String::new();
    let mut in_fence = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            if current_title.is_some() {
                current_body.push_str(line);
                current_body.push('\n');
            }
            continue;
        }
        if !in_fence && trimmed.starts_with('#') {
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

    let mut used_anchors: std::collections::HashSet<String> = std::collections::HashSet::new();
    sections
        .into_iter()
        .map(|(title, body)| {
            let base = heading_anchor(&title);
            let mut anchor = base.clone();
            let mut suffix = 1usize;
            while used_anchors.contains(&anchor) {
                anchor = format!("{base}-{suffix}");
                suffix += 1;
            }
            used_anchors.insert(anchor.clone());
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

    // =====================================================================
    // Adversarial review, ORI-T-0035 (recorded in the pull request report).
    // One test per finding, each failing against the code before its fix
    // and passing after; plants for the new guards follow in the pull
    // request's own plant procedure, not here.
    // =====================================================================

    // ---------------------------------------------------------------------
    // Finding 1 (HIGH): document identity computed against the wrong base.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_collect_from_repo_paths_are_exactly_repository_relative_at_every_depth() {
        let scratch = Scratch::new("paths-depth");
        let spec = scratch.path.join("spec");
        fs::create_dir_all(spec.join("adr")).expect("create spec/adr");
        fs::create_dir_all(spec.join("runbooks").join("deep")).expect("create a depth-3 dir");
        fs::write(spec.join("PRD.md"), "# Top\n\ndepth one\n").expect("write depth-1 file");
        fs::write(
            spec.join("adr").join("ADR-0001.md"),
            "# ADR-0001\n\ndepth two\n",
        )
        .expect("write depth-2 file");
        fs::write(
            spec.join("runbooks").join("deep").join("note.md"),
            "# Deep\n\ndepth three\n",
        )
        .expect("write depth-3 file");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
        let paths: Vec<&str> = documents.iter().map(|d| d.path.as_str()).collect();

        // Built with `concat!` rather than one literal: this repository's
        // own citation gate (crates/ori-gates/src/spec_refs.rs) reads any
        // contiguous "spec/" ... ".md" token in the tree as a citation to check
        // against this repository's real spec/, and these are test
        // fixtures in a throwaway scratch directory, never a citation.
        assert!(
            paths.contains(&concat!("spec/", "PRD.md", "#top")),
            "a depth-1 file must keep its full repository-relative path, not the checkout \
             directory's own name: {paths:?}"
        );
        assert!(
            paths.contains(&concat!("spec/adr/", "ADR-0001.md")),
            "a depth-2 file (an ADR, one document per file) must be repository-relative: \
             {paths:?}"
        );
        assert!(
            paths.contains(&concat!("spec/runbooks/deep/", "note.md", "#deep")),
            "a depth-3 file must keep its full leading path, not lose spec/runbooks/: {paths:?}"
        );
        assert!(
            !paths.iter().any(|path| path.starts_with(
                scratch
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
            )),
            "no path may carry the checkout directory's own name as a prefix: {paths:?}"
        );
    }

    #[test]
    fn ori_t_0035_collect_from_repo_gives_identical_paths_under_two_different_checkout_names() {
        let base = std::env::temp_dir().join(format!(
            "ori-t-0035-checkout-identity-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let checkout_a = base.join("main-checkout");
        let checkout_b = base.join("worktree-x");
        for root in [&checkout_a, &checkout_b] {
            let spec = root.join("spec");
            fs::create_dir_all(spec.join("adr")).expect("create spec/adr");
            fs::write(spec.join("PRD.md"), "# Top\n\nsame content\n")
                .expect("write the same top-level file in both checkouts");
            fs::write(
                spec.join("adr").join("ADR-0001.md"),
                "# ADR-0001\n\nsame content\n",
            )
            .expect("write the same ADR in both checkouts");
        }

        let a = Indexer::collect_from_repo(&checkout_a).expect("collect from checkout A");
        let b = Indexer::collect_from_repo(&checkout_b).expect("collect from checkout B");
        let mut a_paths: Vec<&str> = a.iter().map(|d| d.path.as_str()).collect();
        let mut b_paths: Vec<&str> = b.iter().map(|d| d.path.as_str()).collect();
        a_paths.sort_unstable();
        b_paths.sort_unstable();

        assert_eq!(
            a_paths, b_paths,
            "identical content checked out under two different directory names must produce \
             identical document identities, or an unchanged repository looks changed to \
             incremental_sync and to a FreshnessTracker keyed on these paths"
        );

        let _ = fs::remove_dir_all(&base);
    }

    // ---------------------------------------------------------------------
    // Finding 2 (HIGH): duplicate paths.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_full_rebuild_refuses_a_target_set_with_a_duplicate_path() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let result = indexer.full_rebuild(&[
            doc(
                "x.md#notes",
                DocumentKind::Section,
                "Notes",
                "first notes body",
            ),
            doc(
                "x.md#notes",
                DocumentKind::Section,
                "Notes",
                "second notes body",
            ),
        ]);
        match result {
            Err(IndexerError::DuplicatePath { path }) => assert_eq!(path, "x.md#notes"),
            other => panic!("a duplicate path must be refused, got {other:?}"),
        }
        assert_eq!(
            indexer.all_documents().expect("read back").len(),
            0,
            "a refused full_rebuild must not have written anything"
        );
    }

    #[test]
    fn ori_t_0035_incremental_sync_refuses_a_target_set_with_a_duplicate_path() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("seed");
        let result = indexer.incremental_sync(&[
            doc("x.md#notes", DocumentKind::Section, "Notes", "first"),
            doc("x.md#notes", DocumentKind::Section, "Notes", "second"),
        ]);
        assert!(
            matches!(result, Err(IndexerError::DuplicatePath { .. })),
            "a duplicate path must be refused: {result:?}"
        );
        let dump = indexer.all_documents().expect("read back");
        assert_eq!(
            dump.len(),
            1,
            "a refused incremental_sync must leave the existing index untouched"
        );
        assert_eq!(dump[0].0.path, "a.md");
    }

    #[test]
    fn ori_t_0035_section_documents_disambiguates_repeated_headings_deterministically() {
        // `docs/`, not `spec/`: this repository's own citation gate reads
        // any "spec/" ... ".md" token as a citation to check, and `path` here is
        // an arbitrary caller-supplied prefix as far as `section_documents`
        // is concerned (the real `spec/` prefix `collect_from_repo` always
        // uses is covered by
        // `ori_t_0035_collect_from_repo_paths_are_exactly_repository_relative_at_every_depth`,
        // above).
        let text =
            "# Restore\n\n## Steps\n\nstop the engine\n\n# Verify\n\n## Steps\n\ncheck the chain\n";
        let documents = section_documents("docs/runbooks/restore.md", text);
        let paths: Vec<&str> = documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "docs/runbooks/restore.md#restore",
                "docs/runbooks/restore.md#steps",
                "docs/runbooks/restore.md#verify",
                "docs/runbooks/restore.md#steps-1",
            ],
            "the second '## Steps' must get GitHub's disambiguating suffix, not the same anchor \
             as the first: {paths:?}"
        );
        assert!(paths.iter().all(|path| {
            let mut seen = std::collections::HashSet::new();
            seen.insert(*path)
        }));
    }

    #[test]
    fn ori_t_0035_section_documents_does_not_read_a_hash_comment_inside_a_fenced_code_block_as_a_heading()
     {
        let text = "# Setup\n\n```sh\n# install\nmake\n```\n\n# Install\n\nrun make install\n";
        let documents = section_documents("docs/runbooks/code.md", text);
        let paths: Vec<&str> = documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "docs/runbooks/code.md#setup",
                "docs/runbooks/code.md#install"
            ],
            "a '#' line inside a fenced code block must never split a new section or duplicate \
             an anchor: {paths:?}"
        );
        let setup = documents
            .iter()
            .find(|d| d.path == "docs/runbooks/code.md#setup")
            .expect("the Setup section exists");
        assert!(
            setup.body.contains("# install"),
            "the fenced '# install' line must stay in the Setup section's body, as text: {:?}",
            setup.body
        );
    }

    // ---------------------------------------------------------------------
    // Finding 3 (MEDIUM): kind was not in the checksum.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_incremental_sync_detects_a_kind_only_change() {
        let mut rebuilt = Indexer::open_in_memory().expect("in-memory index opens");
        rebuilt
            .full_rebuild(&[doc("m.md", DocumentKind::Module, "T", "B")])
            .expect("full rebuild with the new kind");

        let mut synced = Indexer::open_in_memory().expect("in-memory index opens");
        synced
            .full_rebuild(&[doc("m.md", DocumentKind::Section, "T", "B")])
            .expect("seed with the old kind");
        let report = synced
            .incremental_sync(&[doc("m.md", DocumentKind::Module, "T", "B")])
            .expect("incremental sync with only the kind changed");

        assert_eq!(
            report.upserted, 1,
            "a kind-only change must be counted as an upsert, not skipped as unchanged"
        );
        assert_eq!(
            rebuilt.all_documents().expect("dump"),
            synced.all_documents().expect("dump"),
            "full_rebuild and incremental_sync must agree on kind, not only on title and body"
        );
    }

    // ---------------------------------------------------------------------
    // Finding 4 (MEDIUM): unbounded query cost.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_search_refuses_a_query_over_the_byte_limit() {
        let indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let huge = "a".repeat(MAX_QUERY_BYTES + 1);
        match indexer.search(&huge, 10) {
            Err(IndexerError::QueryTooLarge { byte_len, .. }) => {
                assert_eq!(byte_len, MAX_QUERY_BYTES + 1);
            }
            other => panic!("a query over the byte limit must be refused, got {other:?}"),
        }
    }

    #[test]
    fn ori_t_0035_search_refuses_a_query_over_the_token_limit() {
        let indexer = Indexer::open_in_memory().expect("in-memory index opens");
        // Short enough in bytes to pass the byte cap, but with more tokens
        // than MAX_QUERY_TOKENS allows.
        let query = "a ".repeat(MAX_QUERY_TOKENS + 1);
        assert!(
            query.len() <= MAX_QUERY_BYTES,
            "the byte cap must not be what refuses this"
        );
        match indexer.search(&query, 10) {
            Err(IndexerError::QueryTooLarge { token_count, .. }) => {
                assert_eq!(token_count, MAX_QUERY_TOKENS + 1);
            }
            other => panic!("a query over the token limit must be refused, got {other:?}"),
        }
    }

    #[test]
    fn ori_t_0035_search_within_both_limits_still_runs() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
            .expect("full rebuild");
        let report = indexer
            .search("alpha", 10)
            .expect("an ordinary query must still run");
        assert_eq!(report.hits.len(), 1);
    }

    // ---------------------------------------------------------------------
    // Finding 5 (MEDIUM): every MATCH failure became InvalidQuery.
    // ---------------------------------------------------------------------

    /// Damages every row of the FTS5 shadow table `documents_data` through a
    /// second, independent connection to the same on-disk file: the
    /// reviewers' own technique, reproduced here, with one change from the
    /// original round-2 version. That version filled each row with
    /// `randomblob(length(block))`, fresh random bytes on every call; a
    /// re-verification workflow measured this as reporting
    /// `ErrorCode::OutOfMemory` (`SQLITE_NOMEM`) instead of
    /// `ErrorCode::DatabaseCorrupt` roughly 1.3% of the time (random bytes
    /// occasionally decode as a record claiming an implausible size, which
    /// SQLite's allocator refuses rather than a plain "this is corrupt"), so
    /// every test built on this helper was flaky at that same rate. Each
    /// byte is now fixed at `0xFF` instead, the same length as the row it
    /// replaces: empirically, and confirmed over hundreds of fresh runs
    /// during this fix, this always yields `ErrorCode::DatabaseCorrupt`, on
    /// every corpus shape this module's tests seed, never `OutOfMemory`.
    /// [`corrupt_fts5_averages_row_with_an_oversized_length_prefix`] below
    /// is the companion helper for the `OutOfMemory` case specifically:
    /// this one is deliberately not it.
    fn corrupt_fts5_shadow_table(path: &Path) {
        let raw =
            rusqlite::Connection::open(path).expect("open a second, raw connection to the file");
        let changed = raw
            .execute(
                "UPDATE documents_data SET block = \
                 unhex(replace(hex(zeroblob(length(block))), '00', 'ff')) \
                 WHERE block IS NOT NULL",
                [],
            )
            .expect("corrupt the FTS5 shadow table directly");
        assert!(
            changed > 0,
            "the corruption update must actually touch at least one row, or this test proves \
             nothing"
        );
    }

    /// The pinned 26-byte replacement this module's own probing found for
    /// `documents_data`'s row 10 (FTS5's averages record) that makes the
    /// bundled SQLite report `ErrorCode::OutOfMemory` (`SQLITE_NOMEM`)
    /// while reading this exact module's 80-document, single-shared-term
    /// corpus (the same shape
    /// `tests::ori_t_0035_a_corrupt_index_is_reported_as_corrupt_not_invalid_query`
    /// seeds): confirmed deterministic over 20 independent fresh runs
    /// before being pinned here, and stable for the reason
    /// `diagnose_ambiguous_read_error`'s doc gives (a length prefix
    /// decoding to an implausible size), not a property of any particular
    /// corpus content beyond its exact shape. Not a general-purpose
    /// corruption pattern the way [`corrupt_fts5_shadow_table`] is: it is
    /// specific to row 10 at this exact length, found empirically rather
    /// than derived from FTS5's on-disk format, and exists only to prove
    /// [`diagnose_ambiguous_read_error`] catches the `OutOfMemory` case
    /// deterministically, not to stand in for corruption generally.
    const AVERAGES_ROW_OVERSIZED_LENGTH_PREFIX: &str =
        "556f784c090469b211bcb0a1cc943696ddceafb6a695cb1485d7";

    /// Replaces `documents_data`'s row 10 with
    /// [`AVERAGES_ROW_OVERSIZED_LENGTH_PREFIX`] through a second,
    /// independent connection: see that constant's doc for what it is and
    /// why, and the module doc's "A corrupt index is reported honestly" for
    /// why this exists as its own helper rather than folded into
    /// [`corrupt_fts5_shadow_table`].
    fn corrupt_fts5_averages_row_with_an_oversized_length_prefix(path: &Path) {
        let raw =
            rusqlite::Connection::open(path).expect("open a second, raw connection to the file");
        let bytes: Vec<u8> = (0..AVERAGES_ROW_OVERSIZED_LENGTH_PREFIX.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&AVERAGES_ROW_OVERSIZED_LENGTH_PREFIX[i..i + 2], 16)
                    .expect("the pinned pattern is valid hex")
            })
            .collect();
        let changed = raw
            .execute(
                "UPDATE documents_data SET block = ?1 WHERE rowid = 10",
                params![bytes],
            )
            .expect("replace the averages row directly");
        assert_eq!(
            changed, 1,
            "row 10 must exist and be the one row this replaces, or this test proves nothing \
             (a schema or FTS5 version change may have moved the averages record)"
        );
    }

    #[test]
    fn ori_t_0035_a_corrupt_index_is_reported_as_corrupt_not_invalid_query() {
        let scratch = Scratch::new("corrupt-search");
        let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
        let seed: Vec<IndexableDocument> = (0..80)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    "alpha content shared by every document so the term has a real doclist",
                )
            })
            .collect();
        indexer.full_rebuild(&seed).expect("seed a real corpus");
        let before = indexer
            .search("alpha", 10)
            .expect("search before corruption");
        assert!(
            !before.hits.is_empty(),
            "the query must genuinely match before corruption"
        );

        corrupt_fts5_shadow_table(&scratch.path.join("fts.sqlite"));

        match indexer.search("alpha", 10) {
            Err(IndexerError::Corrupt { source, .. }) => {
                assert!(
                    matches!(
                        &source,
                        rusqlite::Error::SqliteFailure(inner, _)
                            if inner.code == rusqlite::ErrorCode::DatabaseCorrupt
                    ),
                    "corrupt_fts5_shadow_table's fixed pattern must deterministically report \
                     DatabaseCorrupt, not {source:?}"
                );
            }
            other => panic!(
                "a corrupt index must be reported as IndexerError::Corrupt, with the query \
                 blameless, not {other:?}"
            ),
        }
    }

    #[test]
    fn ori_t_0035_a_corrupt_index_is_reported_as_corrupt_even_when_sqlite_calls_it_out_of_memory() {
        // The exact gap a re-verification workflow found: classify_error and
        // search_error alone treated ErrorCode::OutOfMemory as an ordinary
        // IndexerError::Sqlite, so a corrupted index that happened to
        // surface that particular SQLite error code gave a caller no signal
        // to rebuild. diagnose_ambiguous_read_error closes it by asking
        // FTS5's own integrity-check rather than guessing from the error
        // code.
        let scratch = Scratch::new("corrupt-out-of-memory");
        let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
        let seed: Vec<IndexableDocument> = (0..80)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    "alpha content shared by every document so the term has a real doclist",
                )
            })
            .collect();
        indexer.full_rebuild(&seed).expect("seed a real corpus");

        corrupt_fts5_averages_row_with_an_oversized_length_prefix(&scratch.path.join("fts.sqlite"));

        match indexer.search("alpha", 10) {
            Err(IndexerError::Corrupt { source, .. }) => {
                assert!(
                    matches!(
                        &source,
                        rusqlite::Error::SqliteFailure(inner, _)
                            if inner.code == rusqlite::ErrorCode::OutOfMemory
                    ),
                    "this test's own pinned pattern must deterministically make SQLite report \
                     OutOfMemory, or it is not exercising diagnose_ambiguous_read_error's \
                     reason for existing; got {source:?}"
                );
            }
            other => panic!(
                "a corrupted index that SQLite itself reports as OutOfMemory must still be \
                 IndexerError::Corrupt, with the query blameless, not {other:?}"
            ),
        }
    }

    #[test]
    fn ori_t_0035_full_rebuild_recovers_a_corrupt_on_disk_index() {
        let scratch = Scratch::new("corrupt-recover");
        let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
        let seed: Vec<IndexableDocument> = (0..80)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    "alpha content",
                )
            })
            .collect();
        indexer.full_rebuild(&seed).expect("seed a real corpus");

        corrupt_fts5_shadow_table(&scratch.path.join("fts.sqlite"));
        assert!(
            matches!(
                indexer.search("alpha", 10),
                Err(IndexerError::Corrupt { .. })
            ),
            "the corruption must be real and detected before recovery is tested"
        );

        let report = indexer
            .full_rebuild(&[doc("b.md", DocumentKind::Section, "B", "beta content")])
            .expect("full_rebuild must recover from corruption rather than fail or stay corrupt");
        assert_eq!(report.total, 1);

        let hits = indexer
            .search("beta", 10)
            .expect("search after recovery must work normally")
            .hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "b.md");
    }

    // ---------------------------------------------------------------------
    // Finding 6 (MEDIUM): reads and dependent operations were not in one
    // transaction. Stress tests: each round is cheap, and the invariant
    // must hold on every one of them, not just on average.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_search_documents_covered_never_disagrees_with_a_concurrent_writer() {
        let scratch = Scratch::new("race-search");
        let mut seeder = Indexer::open(&scratch.path).expect("seed connection opens");
        seeder.full_rebuild(&[]).expect("start empty");
        drop(seeder);

        let writer_path = scratch.path.clone();
        let writer = std::thread::spawn(move || {
            let mut writer = Indexer::open(&writer_path).expect("writer opens");
            for round in 0..250 {
                let docs: Vec<IndexableDocument> = if round % 2 == 0 {
                    (0..40)
                        .map(|n| {
                            doc(
                                &format!("w{n}.md"),
                                DocumentKind::Section,
                                "W",
                                "needle content",
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let _ = writer.full_rebuild(&docs);
            }
        });

        let reader_path = scratch.path.clone();
        let reader = std::thread::spawn(move || {
            let reader = Indexer::open(&reader_path).expect("reader opens");
            let mut vacuous = 0usize;
            let mut inverse = 0usize;
            for _ in 0..2000 {
                if let Ok(report) = reader.search("needle", 1000) {
                    if report.documents_covered > 0 && report.hits.is_empty() {
                        vacuous += 1;
                    }
                    if report.documents_covered == 0 && !report.hits.is_empty() {
                        inverse += 1;
                    }
                }
            }
            (vacuous, inverse)
        });

        writer.join().expect("writer thread must not panic");
        let (vacuous, inverse) = reader.join().expect("reader thread must not panic");
        assert_eq!(
            (vacuous, inverse),
            (0, 0),
            "documents_covered and hits must always describe the same committed snapshot: a \
             writer committing between two separate reads used to produce exactly this \
             mismatch"
        );
    }

    #[test]
    fn ori_t_0035_incremental_sync_survives_heavy_concurrent_writing_without_corruption() {
        // A precise reproduction of the original defect's exact race window
        // (a handful of CPU instructions between one SQL statement and the
        // next, inside one function call, with no test hook to pause
        // there) is not practical to force deterministically from outside
        // the function: whichever of a concurrent writer's commit and this
        // call's own transaction happens to land first is a legitimate,
        // unordered choice this test cannot and should not referee round by
        // round (both "the writer's document is swept up and deleted
        // because it already existed when the sync's transaction opened"
        // and "the writer's document survives because it was added only
        // after that sync's transaction had already committed" are correct
        // outcomes, and nothing on the outside can tell them apart without
        // reaching into the method's own internals). What this test checks
        // instead, and can check honestly: heavy concurrent writing never
        // corrupts the index or leaves it in a state incremental_sync
        // cannot still converge correctly from, once the contention stops.
        // `ori_t_0035_search_documents_covered_never_disagrees_with_a_concurrent_writer`,
        // above, is this same finding's deterministic, precisely-targeted
        // half for `search`.
        let scratch = Scratch::new("race-sync");
        let mut seeder = Indexer::open(&scratch.path).expect("seed connection opens");
        seeder
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "original")])
            .expect("seed the target document");
        drop(seeder);

        let writer_path = scratch.path.clone();
        let writer = std::thread::spawn(move || {
            let mut writer = Indexer::open(&writer_path).expect("writer opens");
            for _ in 0..300 {
                let _ = writer.add_or_replace(&doc(
                    "intruder.md",
                    DocumentKind::Section,
                    "I",
                    "an unrelated document",
                ));
            }
        });

        let syncer_path = scratch.path.clone();
        let syncer = std::thread::spawn(move || {
            let mut syncer = Indexer::open(&syncer_path).expect("syncer opens");
            for _ in 0..300 {
                let _ =
                    syncer.incremental_sync(&[doc("a.md", DocumentKind::Section, "A", "original")]);
            }
        });

        writer.join().expect("writer thread must not panic");
        syncer.join().expect("syncer thread must not panic");

        // Contention has stopped; one final, uncontested sync must converge
        // to exactly the target, proving the index survived the race
        // intact rather than corrupted or permanently stuck.
        let mut settle = Indexer::open(&scratch.path).expect("settle connection opens");
        settle
            .incremental_sync(&[doc("a.md", DocumentKind::Section, "A", "original")])
            .expect("a final, uncontested sync must succeed");
        let dump = settle.all_documents().expect("read back the settled state");
        assert_eq!(
            dump.len(),
            1,
            "after contention stops, the index must settle to exactly the target set, not stay \
             corrupted or stuck with an extra document: {dump:?}"
        );
        assert_eq!(dump[0].0.path, "a.md");
    }

    // ---------------------------------------------------------------------
    // Finding 7 (MEDIUM): the spec/design/ exclusion test passed vacuously.
    // A new test, added beside the existing one rather than editing it
    // (CLAUDE.md: an existing test is never modified).
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_collect_from_repo_excludes_a_real_markdown_file_under_spec_design() {
        let scratch = Scratch::new("collect-design-md");
        let spec = scratch.path.join("spec");
        let design = spec.join("design");
        fs::create_dir_all(&design).expect("create spec/design");
        fs::write(
            design.join("DESIGN.md"),
            "# Design Ori Studio\n\nLedgerline mock content that must never be indexed\n",
        )
        .expect("write a real markdown file under spec/design/");
        fs::write(spec.join("PRD.md"), "# PRD\n\nreal content\n").expect("write a real document");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
        assert!(
            !documents.is_empty(),
            "the real document outside spec/design/ must still be collected"
        );
        assert!(
            documents
                .iter()
                .all(|d| !d.path.starts_with("spec/design/")),
            "a real .md file under spec/design/ must never be collected, unlike the .html file \
             the existing test plants (which the extension filter alone already drops, so it \
             cannot tell the directory skip apart from its absence): {documents:?}"
        );
    }

    #[test]
    fn ori_t_0035_collect_from_repo_treats_a_differently_cased_design_directory_as_a_different_name()
     {
        // The exclusion matches the literal directory name "design"
        // (lower case), the name escalation E-0006 and this repository's
        // real spec/design/ both use; it does not fold case. A directory a
        // caller actually named "Design" is therefore a different name as
        // far as this module is concerned, not an evasion of the exclusion:
        // documenting that choice here, rather than leaving it implicit.
        let scratch = Scratch::new("collect-design-case");
        let spec = scratch.path.join("spec");
        let differently_cased = spec.join("Design");
        fs::create_dir_all(&differently_cased).expect("create spec/Design");
        fs::write(differently_cased.join("NOTES.md"), "# Notes\n\ncontent\n")
            .expect("write a markdown file under the differently-cased directory");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
        assert!(
            documents.iter().any(|d| d.path.starts_with("spec/Design/")),
            "a directory literally named Design is not the same name as design, and this \
             module's exclusion is a literal name match, not a case-folded one: {documents:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Finding 8 (LOW): search(query, 0) returned one hit.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_search_with_limit_zero_returns_no_hits() {
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[
                doc("a.md", DocumentKind::Section, "Same", "tie words"),
                doc("b.md", DocumentKind::Section, "Same", "tie words"),
            ])
            .expect("full rebuild");

        let report = indexer.search("tie", 0).expect("limit zero must not error");
        assert_eq!(
            report.hits.len(),
            0,
            "\"at most limit hits\" with limit 0 must return zero hits, not one"
        );
        assert_eq!(
            report.documents_covered, 2,
            "documents_covered must still report the real index size at limit 0"
        );

        let one = indexer.search("tie", 1).expect("limit one must still work");
        assert_eq!(one.hits.len(), 1);
    }

    // =====================================================================
    // Round 3: a re-verification workflow re-ran the round-2 fixes against
    // c8cfc30. Seven held; #5 (corruption recovery) did not, and the fix
    // itself introduced a new high-severity defect (orphaning every other
    // open connection to the file it deleted). Recorded in the pull
    // request report. One test per item, checked to fail before its fix
    // and pass after.
    // =====================================================================

    // ---------------------------------------------------------------------
    // Item 1 (HIGH): corruption recovery redesigned. full_rebuild now
    // repairs the table in place (DROP + CREATE inside its own write
    // transaction), never deletes a file; Indexer::open classifies
    // open-time corruption as Corrupt without self-healing.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_full_rebuild_repairs_shadow_table_corruption_unconditionally() {
        let scratch = Scratch::new("corrupt-repair-unconditional");
        let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
        let seed: Vec<IndexableDocument> = (0..80)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    "alpha content shared by every document",
                )
            })
            .collect();
        indexer.full_rebuild(&seed).expect("seed a real corpus");

        let fts_path = scratch.path.join("fts.sqlite");
        let inode_before = file_inode(&fts_path);
        corrupt_fts5_shadow_table(&fts_path);
        assert!(
            matches!(
                indexer.search("alpha", 10),
                Err(IndexerError::Corrupt { .. })
            ),
            "the corruption must be real before recovery is tested"
        );

        let report = indexer.full_rebuild(&seed).expect(
            "full_rebuild must repair the corruption unconditionally, not only when a \
                      write happens to touch the damaged page",
        );
        assert_eq!(report.total, 80);

        // The fix repairs the table, never the file: the inode must be the
        // one Indexer::open first created, not a new one.
        assert_eq!(
            inode_before,
            file_inode(&fts_path),
            "full_rebuild must never delete or replace fts.sqlite; the same file, same inode, \
             is repaired in place"
        );

        let hits = indexer
            .search("alpha", 100)
            .expect("search after repair must succeed")
            .hits;
        assert_eq!(hits.len(), 80);
        let integrity = indexer.conn.execute(
            "INSERT INTO documents(documents) VALUES('integrity-check')",
            [],
        );
        assert!(
            integrity.is_ok(),
            "FTS5's own integrity-check must pass after repair: {integrity:?}"
        );
    }

    #[test]
    fn ori_t_0035_full_rebuild_recovery_never_orphans_a_second_connection() {
        // The exact defect a second review found in round 2's file-deletion
        // recovery: it repaired the corrupt connection's own view while
        // deleting the file every other open connection still held,
        // silently losing that connection's acknowledged writes and
        // defeating the module's own concurrency guarantee between them.
        let scratch = Scratch::new("corrupt-no-orphan");
        let mut a = Indexer::open(&scratch.path).expect("A opens");
        a.full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
            .expect("seed");
        let mut b = Indexer::open(&scratch.path).expect("B opens the same directory");

        corrupt_fts5_shadow_table(&scratch.path.join("fts.sqlite"));
        assert!(matches!(
            a.search("alpha", 10),
            Err(IndexerError::Corrupt { .. })
        ));
        assert!(matches!(
            b.search("alpha", 10),
            Err(IndexerError::Corrupt { .. })
        ));

        // A repairs it.
        a.full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
            .expect("A's full_rebuild repairs the shared file");

        // B, on the SAME file (never orphaned, because nothing was ever
        // deleted), must now also see a healthy index and be able to write
        // to it, landing in the one file both connections still share.
        b.add_or_replace(&doc("b.md", DocumentKind::Section, "B", "beta content"))
            .expect("B must still be writing to the same live file A just repaired");

        // A fresh connection sees both writes: A's repair and B's add, in
        // one shared file, not two.
        let fresh = Indexer::open(&scratch.path).expect("a fresh connection opens");
        let dump = fresh.all_documents().expect("read back");
        let paths: std::collections::BTreeSet<&str> = dump
            .iter()
            .map(|(document, _)| document.path.as_str())
            .collect();
        assert_eq!(
            paths,
            std::collections::BTreeSet::from(["a.md", "b.md"]),
            "both connections' writes must survive in the one shared file: {dump:?}"
        );
    }

    #[test]
    fn ori_t_0035_open_reports_corrupt_for_a_damaged_header_not_a_generic_sqlite_error() {
        let scratch = Scratch::new("open-time-corrupt");
        {
            let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
                .expect("seed");
        }
        let fts_path = scratch.path.join("fts.sqlite");
        // Damage the part of page 1 that holds sqlite_schema, not just the
        // FTS5 shadow data: this is corruption Indexer::open itself must
        // fail on, before any Indexer exists to call full_rebuild.
        {
            use std::io::Seek;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(&fts_path)
                .expect("open the raw file for writing");
            file.seek(std::io::SeekFrom::Start(100)).expect("seek");
            file.write_all(&[0xFFu8; 400])
                .expect("damage sqlite_schema");
        }

        match Indexer::open(&scratch.path) {
            Err(IndexerError::Corrupt { .. }) => {}
            Err(other) => panic!(
                "a damaged header/schema must be reported as IndexerError::Corrupt, not {other:?}"
            ),
            Ok(_) => panic!("a damaged header/schema must not open successfully"),
        }
    }

    // ---------------------------------------------------------------------
    // Item 2 (MEDIUM): a row whose stored kind does not parse is now
    // removed or rewritten by incremental_sync's path-based diff, never
    // skipped because the row failed to parse.
    // ---------------------------------------------------------------------

    fn insert_raw_row(dir: &Path, path: &str, kind: &str, title: &str, body: &str, checksum: i64) {
        let conn = rusqlite::Connection::open(dir.join("fts.sqlite"))
            .expect("open a raw connection to the on-disk index");
        conn.execute(
            "INSERT INTO documents (path, kind, title, body, checksum) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![path, kind, title, body, checksum],
        )
        .expect("insert a raw row with an unparseable kind");
    }

    #[test]
    fn ori_t_0035_incremental_sync_removes_an_unparseable_kind_row_absent_from_target() {
        let scratch = Scratch::new("unparseable-removed");
        let mut indexer = Indexer::open(&scratch.path).expect("open on-disk index");
        indexer
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("seed");
        insert_raw_row(&scratch.path, "x.md", "prompt", "X", "xray", 7);

        let report = indexer
            .incremental_sync(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("sync with the unparseable row absent from target");
        assert_eq!(
            report.removed, 1,
            "the unparseable-kind row must be counted as removed, not silently kept forever"
        );
        assert_eq!(report.total, 1);
        let dump = indexer.all_documents().expect("read back");
        assert_eq!(dump.len(), 1);
        assert_eq!(dump[0].0.path, "a.md");
    }

    #[test]
    fn ori_t_0035_incremental_sync_rewrites_an_unparseable_kind_row_present_in_target() {
        let scratch = Scratch::new("unparseable-rewritten");
        let indexer_setup = Indexer::open(&scratch.path).expect("open on-disk index");
        drop(indexer_setup);
        insert_raw_row(&scratch.path, "x.md", "prompt", "X", "xray", 7);

        let mut indexer = Indexer::open(&scratch.path).expect("reopen");
        let report = indexer
            .incremental_sync(&[doc("x.md", DocumentKind::Module, "X", "xray")])
            .expect("sync with the unparseable row's path present in target");
        assert_eq!(
            report.upserted, 1,
            "a row whose kind does not parse must be rewritten when its path is in the target, \
             not skipped as though it were already correct"
        );
        let dump = indexer.all_documents().expect("read back");
        assert_eq!(dump.len(), 1);
        assert_eq!(dump[0].0.kind, DocumentKind::Module);
    }

    // ---------------------------------------------------------------------
    // Item 3 (MEDIUM): the -N anchor suffix now keeps incrementing until it
    // finds an anchor no earlier heading already claimed, so it can no
    // longer collide with a heading whose own anchor already ends in -N.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_section_documents_disambiguates_a_heading_that_collides_with_a_suffix() {
        let text = "# Release\n\n## Phase 1\n\nprepare\n\n### Checks\n\nx\n\n## Phase 1.1\n\n\
                     hotfix\n\n## Phase 1\n\nrepeat\n";
        let documents = section_documents("docs/runbooks/release.md", text);
        let paths: Vec<&str> = documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "docs/runbooks/release.md#release",
                "docs/runbooks/release.md#phase-1",
                "docs/runbooks/release.md#checks",
                "docs/runbooks/release.md#phase-1-1",
                "docs/runbooks/release.md#phase-1-2",
            ],
            "the third heading must not collide with the second's own anchor: {paths:?}"
        );
        let mut seen = std::collections::HashSet::new();
        for path in &paths {
            assert!(seen.insert(*path), "no anchor may repeat: {paths:?}");
        }
        // Never a DuplicatePath refusal for this input.
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&documents)
            .expect("this heading shape must never refuse the whole target set");
    }

    #[test]
    fn ori_t_0035_collect_from_repo_and_full_rebuild_succeed_on_this_repositorys_own_spec_tree() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crates/ori-memory sits two levels under the repository root")
            .to_owned();
        let documents = Indexer::collect_from_repo(&repo_root)
            .expect("this repository's own spec/ tree must always collect");
        assert!(
            documents.len() > 100,
            "a vacuous collection would pass every assertion below for the wrong reason: \
             collected {}",
            documents.len()
        );
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let report = indexer.full_rebuild(&documents).expect(
            "this repository's real spec/ tree must never refuse with DuplicatePath: a \
             collision anywhere in it would fail indexing the whole repository",
        );
        assert_eq!(report.total, documents.len());
        let resync = indexer
            .incremental_sync(&documents)
            .expect("an immediate re-sync of the same tree must also succeed");
        assert_eq!(resync.upserted, 0);
        assert_eq!(resync.removed, 0);
    }

    // ---------------------------------------------------------------------
    // Item 4(a) (LOW): classification matches exactly spec/adr/*.md and
    // spec/criteria/*.md, not an adr or criteria component at any depth.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_classification_matches_only_the_exact_spec_adr_and_spec_criteria_shape() {
        let scratch = Scratch::new("classification-exact");
        let spec = scratch.path.join("spec");
        let nested_criteria = spec.join("agents").join("criteria");
        fs::create_dir_all(&nested_criteria).expect("create spec/agents/criteria");
        fs::write(
            nested_criteria.join("checklist.md"),
            "# Review checklist\n\n## Tier 2\n\nreviewer checks apply here\n",
        )
        .expect("write a prose document nested under a criteria-named directory");

        let nested_adr = spec.join("runbooks").join("adr");
        fs::create_dir_all(&nested_adr).expect("create spec/runbooks/adr");
        fs::write(
            nested_adr.join("how-we-write-adrs.md"),
            "# How we write ADRs\n\n## Style\n\nkeep them short\n",
        )
        .expect("write a two-heading document nested under an adr-named directory");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");

        let checklist_docs: Vec<&IndexableDocument> = documents
            .iter()
            .filter(|d| {
                d.path
                    .starts_with(concat!("spec/agents/criteria/", "checklist.md"))
            })
            .collect();
        assert!(
            !checklist_docs.is_empty(),
            "a prose document nested under a criteria-named directory that is not spec/criteria/ \
             must still be collected as Section content, not silently parsed as an (empty) \
             criteria table and dropped: {documents:?}"
        );
        assert!(
            checklist_docs
                .iter()
                .all(|d| d.kind == DocumentKind::Section),
            "it must be Section, not Criterion: {checklist_docs:?}"
        );
        assert!(
            checklist_docs
                .iter()
                .any(|d| d.body.contains("reviewer checks")),
            "its real content must be searchable: {checklist_docs:?}"
        );

        let adr_docs: Vec<&IndexableDocument> = documents
            .iter()
            .filter(|d| {
                d.path
                    .starts_with(concat!("spec/runbooks/adr/", "how-we-write-adrs.md"))
            })
            .collect();
        assert_eq!(
            adr_docs.len(),
            2,
            "a document nested under an adr-named directory that is not spec/adr/ must be split \
             into Section documents by heading, like any other document, not collapsed into one \
             Adr document: {adr_docs:?}"
        );
        assert!(adr_docs.iter().all(|d| d.kind == DocumentKind::Section));
    }

    // ---------------------------------------------------------------------
    // Item 4(b) (LOW): spec/design/ is excluded by its exact
    // repository-relative path; a nested file is excluded too, and a
    // same-named directory elsewhere is not.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_collect_from_repo_excludes_a_nested_markdown_file_under_spec_design() {
        let scratch = Scratch::new("design-nested");
        let nested = scratch.path.join("spec").join("design").join("screens");
        fs::create_dir_all(&nested).expect("create spec/design/screens");
        fs::write(
            nested.join("S.md"),
            "# Nested\n\nLedgerline nested mock content\n",
        )
        .expect("write a nested file under spec/design/");
        fs::write(
            scratch.path.join("spec").join("PRD.md"),
            "# PRD\n\nreal content\n",
        )
        .expect("write a real document");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
        assert!(
            !documents.is_empty(),
            "the real document must still be collected"
        );
        assert!(
            documents
                .iter()
                .all(|d| !d.path.starts_with("spec/design/")),
            "a file nested two levels under spec/design/ must never be collected: {documents:?}"
        );
    }

    #[test]
    fn ori_t_0035_collect_from_repo_does_not_exclude_a_differently_placed_design_directory() {
        let scratch = Scratch::new("design-elsewhere");
        let elsewhere = scratch.path.join("spec").join("runbooks").join("design");
        fs::create_dir_all(&elsewhere).expect("create spec/runbooks/design");
        fs::write(
            elsewhere.join("R.md"),
            "# Runbook design notes\n\nreal content\n",
        )
        .expect("write a file under a design-named directory that is not spec/design");

        let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
        assert!(
            documents
                .iter()
                .any(|d| d.path.starts_with(concat!("spec/runbooks/design/", "R.md"))),
            "the exclusion must match only the exact path spec/design, not any directory named \
             design anywhere under spec/: {documents:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Item 4(c) (LOW): IndexReport.total is read inside the same
    // transaction as the write that produced it, never by a later,
    // separate autocommit statement that can describe another writer's
    // state.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_full_rebuild_total_never_disagrees_with_a_concurrent_writer() {
        let scratch = Scratch::new("total-race");
        let mut seeder = Indexer::open(&scratch.path).expect("seed connection opens");
        seeder.full_rebuild(&[]).expect("start empty");
        drop(seeder);

        let other_path = scratch.path.clone();
        let other = std::thread::spawn(move || {
            let mut other = Indexer::open(&other_path).expect("other writer opens");
            for _ in 0..600 {
                let _ = other.full_rebuild(&[]);
            }
        });

        let mut mine = Indexer::open(&scratch.path).expect("this writer opens");
        let docs: Vec<IndexableDocument> = (0..400)
            .map(|n| doc(&format!("w{n}.md"), DocumentKind::Section, "W", "body"))
            .collect();
        let mut mismatches = 0usize;
        for _ in 0..600 {
            if let Ok(report) = mine.full_rebuild(&docs)
                && report.total != docs.len()
            {
                mismatches += 1;
            }
        }

        other.join().expect("other thread must not panic");
        assert_eq!(
            mismatches, 0,
            "full_rebuild(docs).total must always equal docs.len() when it returns Ok, never a \
             concurrent writer's own count read after this call's own commit"
        );
    }

    // ---------------------------------------------------------------------
    // Item 4(d) (LOW): count_tokens no longer splits a base letter and its
    // combining accent into two tokens, so common NFD-normalized text is
    // not refused below the stated cap.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_search_accepts_an_nfd_normalized_query_under_the_token_cap() {
        // "e" + COMBINING ACUTE ACCENT (U+0301), the NFD spelling of "é",
        // repeated so the query is well under MAX_QUERY_BYTES but would
        // have counted as 2 * MAX_QUERY_TOKENS under the old, byte-blind
        // count.
        let word = "e\u{0301}"; // NFD "é"
        let query = format!("{word} ").repeat(MAX_QUERY_TOKENS);
        assert!(query.len() <= MAX_QUERY_BYTES);
        assert_eq!(
            count_tokens(&query),
            MAX_QUERY_TOKENS,
            "a base letter plus one combining accent must count as one token, matching \
             unicode61, not two"
        );

        let indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .search(&query, 10)
            .expect("an NFD query at exactly the token cap must not be refused");
    }

    /// The inode (Unix) a path currently names, for asserting that a file
    /// was repaired in place rather than deleted and recreated. Returns 0
    /// on a platform or filesystem where this cannot be read, which would
    /// make the assertion using it vacuously pass rather than fail, so
    /// every caller also asserts the file exists and is searchable, never
    /// relying on the inode check alone.
    #[cfg(unix)]
    fn file_inode(path: &Path) -> u64 {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path)
            .map(|metadata| metadata.ino())
            .unwrap_or(0)
    }

    #[cfg(not(unix))]
    fn file_inode(_path: &Path) -> u64 {
        0
    }
}
