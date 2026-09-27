//! The repository indexer, over SQLite FTS5: AICD §25.
//!
//! `spec/LLD.md` section 2 gives this crate `Indexer`; its section 6 fixes
//! where it lives on disk, `<app data>/ori/products/<product_id>/index/`, a
//! sibling of `product.sqlite`, `sessions/` and `evidence/`
//! (`crates/ori-store/src/db.rs`'s `ProductDb::open` already creates that
//! directory; this module is what fills it, in its own file,
//! `index/fts.sqlite`, never inside `product.sqlite`: see "The index is
//! derived" below). An on-disk [`Indexer`] is opened only from the
//! [`ProductDb`] that owns the product directory, never from a bare path, and
//! borrows it for as long as it lives; "Recovery" below is what that buys.
//! `spec/PRD.md` K-02 is the requirement: "Repository indexer: full-text,
//! document graph (sections, ADRs, criteria, modules), re-index on merge".
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
//! # `spec/design/` is walked like the rest of `spec/`
//!
//! Escalation E-0006 (`ops/escalations/E-0006-the-design-artifact-is-mock-data.md`)
//! found `spec/design/Ori Studio.html` to be mock UI data for a fictional
//! product ("Ledgerline"), with its own fabricated `spec/`-shaped
//! citations. That file is all E-0006 is about, and the walk never reads
//! it: it reads only `.md` files, and records the HTML file in
//! [`RepoWalk::skipped`] with [`SkipReason::NotMarkdown`], like any other
//! file that is not Markdown.
//!
//! Earlier rounds went further and never entered `spec/design/` at all, on
//! a reading of E-0006 that the lead has since corrected as a mistake: what
//! that removed from the index was `spec/design/DESIGN.md`, which
//! `spec/README.md` lists as a Draft specification document (a gate G2
//! artifact) and which ruling R23 says governs the phase 3 screen set, and
//! any `.md` file added under `spec/design/` later. The exclusion is gone:
//! every `.md` file under `spec/design/` is split, indexed and tracked
//! exactly as the same file is anywhere else under `spec/`
//! (`tests::ori_t_0035_every_markdown_file_under_spec_design_is_indexed_like_any_spec_file`).
//!
//! # The index is derived: rebuild and incremental must agree
//!
//! Same discipline `ori-store`'s projections and `rebuild.rs` already carry
//! for the event log: `index/` holds nothing that cannot be reconstructed
//! from the repository. [`Indexer::open`] creates
//! `<product dir>/index/fts.sqlite`, a file of its own, never a table inside
//! `product.sqlite`: deleting it and reopening loses nothing that
//! [`Indexer::full_rebuild`] cannot put back from the repository, whereas a
//! table sharing `product.sqlite` would tie this derived, disposable data to
//! the event log's own file, which `spec/LLD.md` section 6 never asks for and
//! this module's own tests
//! (`tests::ori_t_0035_the_index_lives_in_its_own_file_never_inside_product_sqlite`)
//! check directly. [`Indexer::incremental_sync`] ("re-index on merge") must
//! leave the index in the state a full rebuild from the same target document
//! set would. This module's tests prove it the way `rebuild.rs` proves its
//! own claim: by dumping [`Indexer::all_documents`] (every stored column of
//! every document, sorted by path so the dump does not depend on row
//! insertion order) from both paths and comparing byte for byte, not by
//! inspection, and, for rows this build did not write itself, by comparing
//! the raw stored rows. A document removed from the target set is deleted,
//! not left to linger, which [`Indexer::incremental_sync`]'s own tests check
//! directly.
//!
//! ```mermaid
//! flowchart TB
//!   T[target: &[IndexableDocument]] --> C{full_rebuild or incremental_sync}
//!   C -->|full_rebuild| COUNT[count every stored row: at a target path, or not] --> CLEAR[DROP and CREATE documents] --> ADDALL[INSERT every target document] --> COMMIT[transaction commit]
//!   C -->|incremental_sync| OPTION{an FTS5 option stored that this build never sets?}
//!   OPTION -->|yes: rebuild exactly as full_rebuild, in this transaction| COUNT
//!   OPTION -->|no| CURRENT[every stored row: rowid and every column]
//!   CURRENT --> DIFF{diff against target, by path}
//!   DIFF -->|path not text, or path not in target| DEL[DELETE the rows]
//!   DIFF -->|path new, or any column stored differs, or two rows share it| UPSERT[DELETE WHERE path, then INSERT]
//!   DIFF -->|exactly one row, every column exactly as this build writes it| SKIP[leave alone]
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
//! The path is the file's components joined with `/`, and nothing else is
//! rewritten. A later review found the walk replacing every `\` with `/`
//! on every platform, so on Unix, where `\` is an ordinary character in a
//! file name, a file named with one collided with the real file of the
//! same `/` spelling, and the duplicate then refused indexing the whole
//! repository. `Path::components` already splits on `\` exactly where the
//! platform treats it as a separator (Windows) and nowhere else, so joining
//! the components is the whole normalization.
//!
//! A file's encoding signature is not its content either: a leading
//! byte-order mark is dropped when the file is read, so a file saved with
//! one splits into exactly the documents, identities and titles it does
//! without (`read_capped`'s doc has the review that found otherwise). Nor
//! are its line endings: `\r\n` is folded to `\n` when the file is read,
//! so a checkout made with either splits into the same documents, byte for
//! byte, the text of a file with no heading included (the same doc has the
//! review).
//!
//! # Duplicate paths: skipped by the walk, refused by the writers
//!
//! [`Indexer::full_rebuild`] and [`Indexer::incremental_sync`] both call
//! `refuse_invalid_target` before writing anything: a target set holding
//! two [`IndexableDocument`]s at one `path` is refused with
//! [`IndexerError::DuplicatePath`], not written with whichever one happens
//! to survive. An adversarial review found that, without this, the two
//! methods disagreed on such input (`full_rebuild` kept every row;
//! `incremental_sync` kept exactly one and, because its own diff runs
//! against a `BTreeMap` that already dropped the duplicate, never settled:
//! it deleted and re-inserted a different one of the two documents on every
//! call against the same unchanged target).
//!
//! The repository walk must never hand them such a set, because one odd file
//! must never refuse indexing the whole repository. Fenced code is never
//! read as a heading, and repeated headings get GitHub's disambiguating
//! suffix (`section_documents`'s own doc has the detail); anything that
//! still repeats a path an earlier document already took (a criteria
//! table's genuinely repeated ID, which this module does not silently
//! rename) is left out and reported in [`RepoWalk::skipped`] with
//! [`SkipReason::DuplicateDocumentPath`], naming the file it came from. The
//! walk visits directory entries in name order, so which of two such
//! documents is kept does not depend on the filesystem's own order.
//!
//! # What the repository walk never reads
//!
//! The operator's threat model for this repository's content readers
//! (ruled 2026-09-24 for the code map, ORI-T-0036, and applying here): a
//! static hostile repository is in scope. [`Indexer::walk_repo`] therefore
//! decides every entry by `DirEntry::file_type`, which describes the entry
//! itself and never what a symbolic link points at, and records in
//! [`RepoWalk::skipped`], with a [`SkipReason`], each of these instead of
//! reading it:
//!
//! - a symbolic link, to a file or to a directory, anywhere under `spec/`
//!   ([`SkipReason::Symlink`]): followed, a link inside `spec/` indexed one
//!   file's text a second time under another identity, and a link out of
//!   the repository would have read whatever it pointed at;
//! - an entry that is not a regular file, such as a named pipe, which
//!   would block the walk forever on read ([`SkipReason::NotARegularFile`]);
//! - a regular file whose name does not carry the `.md` extension exactly,
//!   in lower case, `.MD` and `.markdown` included
//!   ([`SkipReason::NotMarkdown`]): the walk reads only `.md` files, the
//!   extension every specification file in this repository has, and a
//!   review found round 6 dropping every other file with no record while
//!   this list said nothing else was left out;
//! - a `.md` file whose path under the repository is not UTF-8
//!   ([`SkipReason::NonUtf8Path`]): a lossy conversion would map two
//!   different names onto one identity;
//! - a file or directory that cannot be read, including a `.md` file whose
//!   bytes are not UTF-8 text ([`SkipReason::Unreadable`]), so one such file
//!   no longer fails the whole walk;
//! - a document whose path an earlier document already took (above);
//! - a file longer than 1 MiB ([`SkipReason::FileTooLarge`]), never read
//!   past that, and a document longer than 64 KiB of title and body
//!   ([`SkipReason::DocumentTooLarge`]), which no writer stores: "Query
//!   cost is bounded by bytes", below;
//! - a `.md` file that splits into more than 1,024 documents
//!   ([`SkipReason::FileDocumentLimit`]), and one that would take the walk
//!   past its budget of documents or of bytes
//!   ([`SkipReason::WalkDocumentLimit`], [`SkipReason::WalkByteLimit`]):
//!   "What one walk costs", below.
//!
//! Nothing else is left out; no directory under `spec/` is skipped by name. Every other line of every file the walk reads
//! is in some document: a criteria file's rows that start with an
//! identifier become one [`DocumentKind::Criterion`] each, and every other
//! line of it (its title, its prose, its table header, a proposed criterion
//! in the acceptance-criterion template's two-column form, a whole
//! criteria file of another shape) is split into sections like any other
//! specification file. A review found all of that dropped with no record.
//! And every file the walk reads yields at least one document: a file with
//! no heading, even an empty one, is one document at its bare path, and a
//! criteria file with no criterion row, an empty or blank placeholder
//! included, is split exactly as the same file is anywhere else. A later
//! review found such a placeholder under `spec/criteria/` yielding no
//! document and no record, while the same empty file under
//! `spec/runbooks/` was one document.
//!
//! A static repository is the scope: an entry swapped for a link between the
//! walk's type check and its read is a race this does not claim to close.
//!
//! `spec/` itself is not an entry the walk can leave out: a walk with no
//! `spec/` directory to walk (the repository root or `spec/` missing, or
//! `spec/` a symbolic link, never followed, or a regular file) is refused
//! with [`IndexerError::NoSpecDirectory`]. A review found round 6 returning
//! an empty walk, `Ok`, with nothing skipped, for a repository root that
//! did not exist and for a `spec/` that was a file; handed to
//! [`Indexer::incremental_sync`], the documented pairing, that walk removed
//! every document and returned `Ok`, and [`Indexer::collect_from_repo`]
//! drops the skip list, so even the round 6 record of a linked `spec/` gave
//! its caller nothing to notice.
//!
//! # What one walk costs
//!
//! A walk's memory is its result, every document it returns with its text,
//! plus what it is reading at the moment. Round 6 bounded one file
//! (`MAX_FILE_BYTES`, 1 MiB) and one document (`MAX_DOCUMENT_BYTES`, 64
//! KiB) but not a walk, and a document costs a few hundred bytes beyond its
//! text: a review measured one 1 MiB file of empty headings, two bytes
//! each, split into 524,288 documents and about 200 MB of memory, five such
//! files into 570 MB, with nothing bounding how many files a repository
//! holds. A walk now splits at most `MAX_WALK_DOCUMENTS` (16,384)
//! documents and reads at most `MAX_WALK_BYTES` (16 MiB) of file text into
//! them, in all. A file that would take it past either is left out whole
//! and recorded ([`SkipReason::WalkDocumentLimit`],
//! [`SkipReason::WalkByteLimit`]); a file whose reported length is past
//! what is left, or any file once no document is left, is left out without
//! being read at all; and a later, smaller file that still fits is read.
//! Files are reached in name order, so which ones are left out does not
//! depend on the filesystem. This repository's own `spec/` is 221
//! documents and about 198 KB today.
//!
//! No one file can spend the walk's budget. One file is read up to
//! `MAX_FILE_BYTES`, a sixteenth of the bytes, and split into at most
//! `MAX_FILE_DOCUMENTS` (1,024), a sixteenth of the documents: a file past
//! that is left out whole, its split stopped as soon as it passes the cap,
//! and recorded for itself ([`SkipReason::FileDocumentLimit`]). A review
//! found round 7 letting one file take everything: a 32 KiB file of 16,384
//! bare `#` lines, named to sort first, spent the whole document budget, so
//! every later file was recorded as past it, and a criteria file of 16,384
//! rows repeating one identifier did the same while storing one document.
//! Every document split is still counted, the ones left out as repeats or
//! as too large included, because each costs the walk a skip-list entry
//! (`MAX_WALK_DOCUMENTS`'s doc has the arithmetic); the cap is what keeps
//! that from being one file's to spend.
//!
//! A walk that runs out anyway, which now takes sixteen files at the cap,
//! is not a set of documents to sync by: a file left out because the walk
//! ran out before it may hold documents the index already has, and
//! [`Indexer::incremental_sync`] removes whatever its target set lacks. The
//! review found [`Indexer::collect_from_repo`] dropping the skip list, so
//! the file that spent the budget silently removed the rest of the index
//! while the sync returned `Ok`. `collect_from_repo` now refuses such a
//! walk ([`IndexerError::WalkBudgetExhausted`], naming every file left
//! out); [`Indexer::walk_repo`] still returns it, with the list, for a
//! caller that decides otherwise.
//!
//! Measured on round 7's build (release, macOS, peak resident memory of a
//! process that only walks; the per-file cap since can only lower what one
//! file costs): one 1 MiB file of empty headings, 185 MB before the walk
//! had a budget and 3 MB with it (recorded, not split past it); five such
//! files, 517 to 593 MB before and 3 MB with it; a tree at both budgets at
//! once, 16,384 documents from sixteen files of about 1 MiB, 38 MB, the
//! most measured within them. Not bounded, and growing with the tree
//! rather than with any one file's contents: the skip list, one entry per
//! entry left out (and one per document left out, which the document
//! budget counts), and the directory listings held at once, one per level
//! of the directory being walked (names and types only: one directory is
//! open at a time, whatever the depth).
//!
//! Nor does a walk's stack grow with the tree: it keeps its place in an
//! explicit list of directory listings on the heap, not in a recursion. A
//! review found round 6's recursive walk, one call frame per directory
//! level, aborting the whole process with a stack overflow on trees the
//! path limit allows (250 one-letter levels on a 2 MiB thread in an
//! unoptimized build; 442 on a 1,600 KiB thread optimized), where a walk
//! must read or record, never crash
//! (`tests::ori_t_0035_a_tree_as_deep_as_the_path_limit_allows_is_walked_on_a_small_stack`).
//!
//! # Text before the first heading is a document too
//!
//! A review found every line above a file's first heading discarded: the
//! six role files under `spec/agents/` are front matter plus prose with no
//! heading at all and indexed whole, and adding one heading anywhere in one
//! removed all of its text above that heading from search while every sync
//! returned `Ok`. `section_documents` now keeps that text as a document of
//! its own at the file's bare path, no anchor, kind
//! [`DocumentKind::Section`], titled by the path: the identity a
//! heading-less file already has, so adding a first heading keeps the old
//! text at the path it was always found at and adds the new section beside
//! it, and a bare path can never collide with a heading's, which always
//! carries a `#`. A preamble of only blank lines is not a document; no
//! non-blank line of a walked section file is ever dropped.
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
//! `rusqlite`/SQLite message forwarded verbatim to the caller.
//!
//! A quoted phrase has exactly one shape FTS5's parser refuses: one holding
//! a `NUL`, because the parser stops scanning at it and reports
//! "unterminated string". [`Indexer::search`] refuses that query itself,
//! before SQLite sees it, and that is the only [`IndexerError::InvalidQuery`]
//! it returns. Every failure SQLite reports while a search runs is therefore
//! never the query's fault, and goes through the same classifier every other
//! read does. A review found round 6 mapping any bare `SQLITE_ERROR` from the
//! `MATCH` to `InvalidQuery` instead: one flipped bit in the tokenizer name
//! the stored schema holds (`unicode61` read back as `tnicode61`) made every
//! search fail with FTS5's "no such tokenizer", and the caller was told its
//! query was at fault.
//!
//! # Query cost is bounded by bytes
//!
//! Quoting stops `query` from being read as FTS5 syntax; it does nothing
//! about `query`'s *cost*. An adversarial review measured one search of a
//! common word repeated to fill 1 MB costing 4.5 seconds and 2.18 GB of
//! resident memory, because FTS5 opens one index iterator per phrase term
//! and does not deduplicate repeated tokens, so cost grows with (repeated
//! terms) x (how many documents contain each). [`Indexer::search`] refuses
//! `query` outright, before it is quoted or bound, when it is longer than
//! `MAX_QUERY_BYTES`: under `unicode61` every term is at least one byte and
//! two terms are separated by at least one more, so 1024 bytes can never
//! hold more than 512 terms, whatever the script.
//!
//! That bounds the number of terms, not what each one costs. A later review
//! found the other factor: each term's cost is its positions in the
//! document being matched, and nothing bounded a document, so one 2 MB spec
//! file of a single repeated term and one 1,023-byte query took about 3
//! seconds and 1.09 GB of SQLite heap. So a document is bounded too:
//! `MAX_DOCUMENT_BYTES`, 64 KiB of `title` and `body` together, which every
//! writer refuses to exceed ([`IndexerError::DocumentTooLarge`], before
//! anything is written) and the repository walk leaves out with a reason.
//! At the cap, the worst search within the query cap is measured at 72.6
//! MB of heap (the constant's doc has the whole measurement), and
//! `tests::ori_t_0035_the_worst_query_within_the_byte_cap_against_a_document_at_the_size_cap_stays_bounded`
//! holds it inside a 96 MiB heap cap in which the same search against a
//! document four times the cap fails.
//!
//! What the two caps do not bound, measured rather than assumed: the
//! number of documents a search matches. Its memory grows slowly with it
//! (the same worst search against 1, 10 and 100 documents each at the cap:
//! 72.7, 73.2 and 87.6 MB of heap), and its time in proportion (0.08, 0.8
//! and 7.5 seconds, release), as any search's time grows with the corpus
//! it searches. A repository that fills itself with documents built to
//! match one query can still make that query slow, and its memory still
//! grows by that slow step per matching document; what it can no longer
//! do is make any one document cost memory without bound. The walk also
//! reads no file past `MAX_FILE_BYTES`, 1 MiB, which bounds what one file
//! costs the walk itself, and no more than its budget in all ("What one
//! walk costs", above).
//!
//! Earlier rounds also capped a token count, computed by this module's own
//! approximation of `unicode61`. Reviews found it wrong in both directions
//! (NFD text overcounted, refusing legitimate queries; combining marks that
//! `unicode61` treats as separators, U+0336 among them, undercounted, so a
//! query FTS5 splits into 341 terms counted as one), and it bounded nothing
//! the byte cap did not already bound, so it was removed rather than
//! refined once more.
//! `tests::ori_t_0035_the_most_expensive_query_within_the_byte_cap_stays_bounded_in_time_and_memory`
//! measures the most expensive query shape found within the cap, a
//! one-letter term repeated 512 times against documents that each repeat it
//! more often than that, so the whole phrase really is matched against
//! every document: in a child process whose SQLite heap is capped, it
//! completes inside the cap in bounded time, while the same term repeated
//! past the byte cap runs out of that same heap.
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
//! does not hide that choice inside an implicit, undocumented wait. Any
//! number of [`Indexer`]s may be open on one product at once, in one process
//! or across threads (an `Indexer` is `Send`); SQLite's own locking orders
//! them.
//!
//! # What is reported as corrupt, and what is not
//!
//! [`IndexerError::Corrupt`] means the file is damaged, and nothing else.
//! A failure SQLite itself reports as `SQLITE_CORRUPT`, or as
//! `SQLITE_NOTADB` (what a destroyed header produces, at open), is `Corrupt`
//! directly. Every other failure that is not a held lock, on every read and
//! every write this module makes, is settled by asking SQLite rather than by
//! guessing from the error code (`classify`): `PRAGMA quick_check`, on the
//! same connection, in a clean read snapshot of the file as committed. It
//! checks every b-tree page and, in the SQLite this workspace bundles, also
//! runs FTS5's own inverted-index check through the virtual table's
//! integrity method. It is a read: it takes no write lock, so a concurrent
//! writer can neither refuse it nor be locked out by it.
//!
//! The snapshot is clean because whatever transaction the failed statement
//! was part of is rolled back first, and the check then runs in a read
//! transaction of its own, ended with a rollback, so the connection is left
//! in no transaction at all. The caller is returning the error, and its own
//! transaction would be rolled back as it returns; nothing is lost by
//! ending it first. A review found round 8 running the check inside that
//! transaction instead: SQLite rolls back only the failed statement, so the
//! write transaction, what its earlier statements wrote and FTS5's
//! in-memory view of it (pending data, a structure record a failed flush
//! left half-updated) were all still there, and the check read them. With
//! two legal FTS5 options another writer had set (`pgsz` 64, `automerge`
//! 0), a genuine out-of-memory error in `incremental_sync` came back
//! `Corrupt` from a healthy index, which sends a caller to
//! [`Indexer::recover`] to quarantine it
//! (`tests::ori_t_0035_the_check_never_reads_the_write_transaction_a_failed_statement_left_open`,
//! `tests::ori_t_0035_a_write_that_fails_part_way_through_its_transaction_never_calls_a_healthy_index_corrupt`).
//! Rolling back discards FTS5's in-memory state along with the pages, and
//! [`Indexer::incremental_sync`] no longer keeps an FTS5 option this build
//! did not set (its doc has why).
//!
//! The check's own read transaction first opens a cursor on the table,
//! because FTS5 refreshes its per-connection view of the index only when a
//! cursor opens and its integrity method reads that view as it stands:
//! measured while building this, a bare `PRAGMA quick_check` on a healthy
//! index another connection had just written reported "checksum mismatch"
//! 133 times in 200, and 0 in 200 opened this way. The rule:
//!
//! - the check reports damage in a page it read, or itself fails with
//!   `SQLITE_CORRUPT` or `SQLITE_NOTADB`: `Corrupt`, carrying the original
//!   error;
//! - the check returns `ok`, reports that it could not read a page, or
//!   fails for any other reason (busy, locked, out of memory, an FTS5
//!   format this build does not read): the original error, unchanged, as
//!   [`IndexerError::Sqlite`] (or [`IndexerError::Locked`]), exactly as it
//!   would have been without the check.
//!
//! "Could not read a page" is its own case because a review found the
//! bundled SQLite reporting a genuine out-of-memory condition inside the
//! check as a *row*, not an error: a page the check could not get for want
//! of memory is written into its report ("unable to get the page. error
//! code=7"), followed by lines that are only consequences of the pages it
//! skipped ("Page 7: never used"). Round 6 read every such row as damage,
//! and a healthy index, with SQLite's heap limit set a little above the
//! heap in use, came back `Corrupt` from `search` and `all_documents`.
//! `report_names_an_unread_page` has the detail;
//! `tests::ori_t_0035_a_genuine_out_of_memory_condition_inside_the_check_itself_is_never_called_corrupt`
//! steps the heap limit down through that window 256 bytes at a time, with
//! and without another connection committing between calls; against round
//! 6's reading it finds 68 and 67 `Corrupt` results.
//!
//! Round 4 asked FTS5's `integrity-check` command instead. That command is
//! an `INSERT`, so it needed the write lock with a zero busy timeout, it
//! allocated more than the read that had just failed, and any failure of
//! it at all was read as corruption: a review measured a genuine
//! out-of-memory condition reported as `Corrupt` in 76 of 76 attempts, a
//! concurrent writer turning a healthy index `Corrupt` in 2989 of 3000, and
//! one failed search holding every writer off for a scan of the whole index.
//!
//! What the rule gives up, stated rather than hidden: damage the check
//! cannot settle is never called `Corrupt`. It is reported as the error
//! SQLite gives, which names the file, and [`Indexer::recover`] repairs
//! every such case, because it never reads the old file at all; that is the
//! route for any failure that persists. This module's tests pin these
//! shapes:
//!
//! - FTS5's structure record rewritten through SQL so that a length in it
//!   decodes to an implausible size, after which every reader, the check
//!   included, fails with `SQLITE_NOMEM`, which nothing distinguishes from a
//!   real out-of-memory condition
//!   (`tests::ori_t_0035_an_out_of_memory_error_the_check_cannot_settle_is_never_called_corrupt_and_recover_still_repairs_it`);
//! - an FTS5 format version this build does not read (a newer build's
//!   index, or a damaged config row), which fails the check with a plain
//!   `SQLITE_ERROR`;
//! - one bit flipped in the tokenizer name the stored schema holds, after
//!   which every search fails with FTS5's "no such tokenizer" (a plain
//!   `SQLITE_ERROR`, never blamed on the query), and one bit flipped in the
//!   header's write version (the file becomes read-only, so a rebuild fails
//!   with `SQLITE_READONLY`) or schema format (open fails with "unsupported
//!   file format")
//!   (`tests::ori_t_0035_damage_the_check_cannot_settle_is_never_blamed_on_the_query_and_recover_repairs_it`).
//!
//! Page-level damage, the kind storage actually produces, is seen: in this
//! module's page sweep the check reports every damaged copy as damaged,
//! including the ones whose damage an ordinary search never touches.
//!
//! The check runs only after a call fails, so the rule classifies damage a
//! call trips over; it never makes a call that succeeds look again. A read
//! that reads through damaged bytes without an error returns what it read,
//! and damage a read never touches is not reported by it at all: a review
//! flipping one bit at a time found, among copies the check calls damaged,
//! `all_documents` returning `Ok` with altered contents and `search`
//! returning `Ok` with a `documents_covered` off by one to nine, and
//! `incremental_sync` returning `Ok` on most of them. Such damage surfaces
//! when a later call fails on it, and [`Indexer::recover`] replaces it
//! whether or not any call has.
//!
//! The check reads the whole file, so it costs time in proportion to the
//! index. It runs only on a failure that is neither a lock nor already
//! `SQLITE_CORRUPT`, never on a success path, and never blocks anyone.
//!
//! # Recovery: [`Indexer::recover`], and why the compiler proves it safe
//!
//! Some damage cannot be repaired from inside the file.
//! [`Indexer::full_rebuild`] drops and recreates the FTS5 table inside one
//! write transaction, which repairs damage done to the shadow tables'
//! *rows* through SQL; but dropping an FTS5 table walks and frees every page
//! of every shadow b-tree, so one damaged *page* makes the `DROP` itself fail
//! with `SQLITE_CORRUPT`, the transaction rolls back, and every retry fails
//! the same way. Round 4 claimed otherwise; a review overwrote each page of a
//! closed index in turn, on a fresh copy each time, with `0x00` and with
//! `0xFF`, and `full_rebuild` repaired 0 of 300. The only repair for that is
//! a new file.
//!
//! Replacing the file is safe only when nothing has it open: round 3
//! deleted it under other open connections, whose later writes then went
//! to an unlinked file and were silently lost. So file recovery is one
//! associated function, [`Indexer::recover`], taking
//! `&mut ProductDb`, and every on-disk [`Indexer`] holds a shared borrow of
//! the `ProductDb` it was opened from for as long as it lives
//! (`Indexer<'db>`). The proof has two halves:
//!
//! 1. **Every `Indexer` in this process.** `&mut ProductDb` cannot exist
//!    while any `&ProductDb` does, so while any `Indexer<'db>` is alive the
//!    compiler refuses a call to `recover` (`recover`'s own doc carries
//!    `compile_fail` examples that pin this). `Indexer` implements [`Drop`],
//!    with an empty body, for exactly this reason: without it the borrow
//!    would end at an `Indexer`'s last *use*, and one merely left in scope
//!    would still have its connection open on the file `recover` moves,
//!    which is exactly what this half of the proof says cannot happen. With
//!    it the borrow lasts until the connection is closed. (How much a
//!    connection left open that way could actually hurt was measured, not
//!    assumed: on this build, on macOS, closing it after the move neither
//!    checkpointed into the moved file nor deleted anything at the live
//!    path, and it can never be used again, since any use would extend the
//!    borrow; on Windows, where SQLite opens files without delete sharing,
//!    the move would be refused instead, which was not measured here. The
//!    proof rests on neither.) An `Indexer` leaked with
//!    `std::mem::forget` holds the moved file open, unused, until the
//!    process ends.
//! 2. **Every other engine process.** A live `ProductDb` holds the
//!    product's OS-level single-writer lock (`crates/ori-store/src/db.rs`'s
//!    module doc), so no other process holds a `ProductDb`, and so an
//!    `Indexer`, for this product while `recover` runs.
//!
//! Both halves speak of *this product's* files, so both rest on one more
//! fact: an `Indexer` opened from a product's `ProductDb` holds that
//! product's file and no other. A review broke that twice. Round 5's
//! [`Indexer::open`] canonicalized `index/`, and so followed a link: with
//! one product's `index/` (or its `fts.sqlite`) a link into another's, the
//! first product's `Indexer`, borrowing only its own `ProductDb`, held the
//! second product's file open; `recover` on the second compiled, moved that
//! file into quarantine, and the first `Indexer`'s later writes returned
//! `Ok` into the quarantined copy, 40 times in 40. And both `open` and
//! `recover` resolved `ProductDb::dir()`, which is whatever path the product
//! was opened with, relative ones included, against the working directory
//! of the moment: after a change of directory, `recover` moved and rebuilt
//! a different, separately locked product's live index, 20 times in 20. So
//! `open` and `recover` both refuse a product directory that is not
//! absolute ([`IndexerError::OpenRefused`],
//! [`IndexerError::RecoveryRefused`]); both canonicalize it once, which
//! resolves a link at or above it exactly as `ProductDb::open` did when it
//! reached its lock file through the same path; both refuse an `index/`
//! that is a link; and `open` refuses an `index/fts.sqlite`, `-wal`,
//! `-journal` or `-shm` that is a link, not a regular file, or a file with
//! a second name, then opens the file with `SQLITE_OPEN_NOFOLLOW`, so
//! SQLite itself refuses a link anywhere in the path should one appear
//! between that check and the open. `recover` does not refuse a linked
//! index file: it is the repair for one, moving the link itself into
//! quarantine and never touching what it points at. It does refuse, with
//! nothing moved, an entry at an index file's name that is neither a file
//! nor a link: a review found round 7 renaming whatever was there, and with
//! another product's own directory at `index/fts.sqlite-wal` (a product id
//! or products root naming that path, which `ProductDb::open` does not
//! check), it moved that product's live lock, event log and index into this
//! product's quarantine while the other product had them open. Neither half
//! of the proof above speaks for that product, whose `Indexer` borrows its
//! own `ProductDb`, not this one's; `recover` now moves only what is this
//! product's to move.
//!
//! The second name is round 7's: a review hard-linked one product's
//! `fts.sqlite`, and then its `-wal`, to another product's, and round 6's
//! `open`, which looked only for symbolic links, accepted both; the two
//! products then read and erased each other's acknowledged writes, and
//! `recover` on the other product ran while this product's `Indexer` still
//! held the file it moved, the same failure the symbolic link above
//! produced. `open` now refuses any index file whose link count is above one
//! ([`IndexerError::OpenRefused`]), and `recover` moves such a name like
//! any other, which moves only this product's name. The other product's
//! file then still has a second name, in this product's quarantine, so its
//! own `open` is refused by the same rule (its writes would change this
//! product's evidence) until its own `recover` moves its name aside too.
//! Two limits, stated: the
//! link count is read on Unix only (on Windows the standard library exposes
//! it only on a nightly toolchain, and this crate takes no platform
//! dependency to read it), and a log whose other name was removed before
//! `open` ran (the other product closed normally after the link was made)
//! has one name, this product's, and is read as this product's own log:
//! from then on nothing shares it, so the proof above holds, but its frames
//! are the other product's content, exactly as if they had been copied
//! into this product's `index/`. Content placed in a product's own `index/`
//! is outside what any check here can tell apart from the product's own;
//! the index is derived, and `recover` replaces it.
//!
//! Under that proof `recover` moves `fts.sqlite` and any `-wal`, `-journal`
//! or `-shm` beside it into `index/quarantine/<n>-<recovered_at>/`, and
//! never deletes them, because they are the evidence of what went wrong: the
//! write-ahead log may hold the damaged index's last committed transactions,
//! which the database file itself does not. They move as one unit: if one
//! move fails, every file already moved is moved back, the directory made
//! for them is left where it is, empty if every file went back, and
//! [`IndexerError::QuarantineIncomplete`] names it and any file that could
//! not be put back; nothing is rebuilt. No quarantine directory is ever
//! removed, so no name is ever made twice: a review found round 8 removing
//! the empty directory of a recovery it had rolled back, and the next
//! recovery, handed the same `recovered_at`, then made exactly that name
//! again and filled it, so the path the failed call's error named held
//! another recovery's evidence. A review found round 5 returning at the
//! first failure, with the log already in quarantine and the database still
//! live: the next open then served the index as it stood before the log's
//! transactions, with no error, and a retry quarantined the database in a
//! second directory, apart from its log. Side files still move first, for
//! the one failure part way no rollback can answer, a process killed
//! between two moves: SQLite discards a write-ahead log or rollback journal
//! it finds beside a new, empty database (measured on this build: a leftover
//! log was gone once the new database's connection closed, a leftover
//! journal once it was first read), so the order leaves the old database
//! without its log, which is the stale-index outcome above and loses
//! nothing, never the log beside a fresh database, which would lose it.
//! That case is not detected. It then creates a fresh index and runs
//! [`Indexer::full_rebuild`] from the caller's target set. It never opens
//! the damaged file, so no damage can make it fail, and damage found while
//! opening (a destroyed header included) is recovered exactly like any
//! other. `tests::ori_t_0035_recover_repairs_every_page_of_a_damaged_index_in_a_deterministic_sweep`
//! overwrites every page in turn, with `0x00` and with `0xFF`: after
//! `recover`, every case opens, passes `PRAGMA integrity_check`, and
//! finds every rebuilt document.
//!
//! ```mermaid
//! flowchart TB
//!   ERR[IndexerError::Corrupt from any call] --> DROP[caller drops every Indexer on the product]
//!   DROP --> MUT[&mut ProductDb: the compiler proves no Indexer is alive; the OS lock proves no other engine process]
//!   MUT --> OWN{product directory absolute, index/ and quarantine/ not links, every entry at an index file's name a file or a link?}
//!   OWN -->|no| REFUSED[RecoveryRefused: nothing moved]
//!   OWN -->|yes| MOVE[move -wal, -journal, -shm, then fts.sqlite into index/quarantine/n-recovered_at/]
//!   MOVE -->|a move fails| BACK[move back every file already moved; QuarantineIncomplete names any that stayed]
//!   MOVE --> FRESH[create a fresh fts.sqlite]
//!   FRESH --> REBUILD[full_rebuild from the caller's target set]
//! ```
//!
//! What the proof does not cover, the same limits `ProductDb`'s own lock
//! states for `product.sqlite`: a process that opens `fts.sqlite` directly,
//! bypassing `ProductDb` (an operator's `sqlite3` shell, say), is outside it.
//! An `index/` or `index/quarantine/` that is a symbolic link is refused
//! ([`IndexerError::RecoveryRefused`]): `recover` moves files only inside a
//! directory the product owns outright, never into or out of one that may
//! be another product's. Nor does it cover a component of the product
//! directory's own path that is replaced by a link, or retargeted, after
//! `ProductDb::open` took its lock: canonicalizing then resolves to a
//! directory other than the locked one, and nothing here can compare the
//! two, since `ProductDb` keeps the path as it was given and no identity of
//! the directory it locked. A static filesystem under the products root is
//! the scope, the same scope the repository walk states for `spec/`.
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
//! [`IndexerError::Locked`]), never split across the two. [`IndexReport::total`]
//! is read the same way, inside the write transaction before it commits;
//! `tests::ori_t_0035_index_report_total_is_read_before_commit_never_after_another_writers_commit`
//! proves it deterministically, by committing a second connection's write
//! at the exact moment after this one's commit returns.
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
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use rusqlite::Connection;
use rusqlite::ErrorCode;
use rusqlite::OpenFlags;
use rusqlite::params;
use rusqlite::types::ValueRef;

/// How long a write waits for a lock another connection holds before
/// [`Indexer::full_rebuild`], [`Indexer::incremental_sync`] or
/// [`Indexer::add_or_replace`] returns [`IndexerError::Locked`]: zero, a
/// deliberate, documented choice ("Concurrency" above), not SQLite's own
/// undocumented default.
const WRITE_BUSY_TIMEOUT: Duration = Duration::from_millis(0);

/// The directory under a product's own directory the index lives in, per
/// `spec/LLD.md` section 6's layout.
const INDEX_DIR: &str = "index";

/// The index's own database file, inside [`INDEX_DIR`].
const INDEX_FILE: &str = "fts.sqlite";

/// Where [`Indexer::recover`] moves a replaced index, inside [`INDEX_DIR`].
const QUARANTINE_DIR: &str = "quarantine";

/// The files SQLite may keep beside [`INDEX_FILE`], in the order
/// [`Indexer::recover`] moves them: every side file before the database
/// itself. A move that fails part way is rolled back, every file already
/// moved put back where it was (the module doc's "Recovery"); the order
/// still matters for a process killed between two moves, which no rollback
/// can answer: that leaves the old database without its side files, never
/// the side files beside a fresh database, where SQLite would discard them,
/// and the evidence with them (the module doc records the measurement).
const INDEX_FILE_SET: [&str; 4] = [
    "fts.sqlite-wal",
    "fts.sqlite-journal",
    "fts.sqlite-shm",
    INDEX_FILE,
];

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

    /// The bytes FTS5 indexes for this document, `title` and `body`
    /// together: what `MAX_DOCUMENT_BYTES` bounds.
    fn indexed_byte_len(&self) -> usize {
        self.title.len() + self.body.len()
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
/// call: every row the call deleted and every document it wrote, counted
/// inside its one write transaction, so that the rows stored before the
/// call, less `removed` and `replaced`, plus `upserted`, is exactly `total`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexReport {
    /// Target documents written: every one of them for
    /// [`Indexer::full_rebuild`]; for [`Indexer::incremental_sync`], each
    /// one new at its path or whose stored row was not exactly what this
    /// build writes for it, or every one of them when it rebuilt an index
    /// holding an FTS5 option this build did not set.
    pub upserted: usize,
    /// Stored rows deleted whose path is not in the target set (or is not
    /// text at all, so no target document could name it).
    pub removed: usize,
    /// Stored rows deleted at a path that is in the target set, to make
    /// room for the target's document there: the old version of every
    /// document rewritten, and any second row sharing its path. For
    /// [`Indexer::full_rebuild`], and an [`Indexer::incremental_sync`] that
    /// rebuilt the table, every stored row at a target path.
    pub replaced: usize,
    /// The index's total live document count after this call, read inside
    /// the same transaction as the writes, before it commits.
    pub total: usize,
}

/// What [`Indexer::recover`] did: AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryReport {
    /// The directory, under `index/quarantine/`, the replaced files were
    /// moved into; `None` when there was no index file to move.
    pub quarantine: Option<PathBuf>,
    /// Every file moved, at its new path inside `quarantine`, in the order
    /// it was moved.
    pub quarantined_files: Vec<PathBuf>,
    /// The full rebuild of the fresh index from the caller's target set.
    pub rebuilt: IndexReport,
}

/// One entry the repository walk did not read, and why: AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedEntry {
    /// The entry, as the walk reached it (under the `repo_root` it was
    /// given).
    pub path: PathBuf,
    /// Why it was not read.
    pub reason: SkipReason,
}

/// Why [`Indexer::walk_repo`] left an entry out: the module doc's "What the
/// repository walk never reads".
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SkipReason {
    /// A symbolic link, to a file or a directory: never followed.
    Symlink,
    /// An entry that is not a regular file, a directory or a symbolic link
    /// (a named pipe, a socket, a device), whatever its name: never opened.
    NotARegularFile,
    /// A regular file whose name does not carry the `.md` extension,
    /// exactly and in lower case: `NOTES.MD`, `notes.markdown`, a file
    /// named `.md` (which has no extension), `openapi.yaml`. The walk reads
    /// only `.md` files, and records every other file rather than dropping
    /// it silently.
    NotMarkdown,
    /// A `.md` file whose path under the repository is not valid UTF-8.
    NonUtf8Path,
    /// A file or directory that could not be read; `error` is the
    /// operating system's description.
    Unreadable {
        /// Why the read failed.
        error: String,
    },
    /// A document whose path an earlier document in the same walk already
    /// took; the earlier one is kept.
    DuplicateDocumentPath {
        /// The document path that was already taken.
        path: String,
    },
    /// A file longer than `MAX_FILE_BYTES` (1 MiB): never read past that
    /// many bytes, and none of it indexed (the module doc's "Query cost is
    /// bounded by bytes").
    FileTooLarge {
        /// The file's length in bytes, as far as the walk found it.
        byte_len: u64,
    },
    /// A document the file produced whose `title` and `body` together are
    /// longer than `MAX_DOCUMENT_BYTES` (64 KiB), which no writer in this
    /// module stores; the file's other documents are kept.
    DocumentTooLarge {
        /// The document's path.
        path: String,
        /// Its `title` and `body` length together, in bytes.
        byte_len: usize,
    },
    /// A `.md` file left out whole because its documents would have taken
    /// the walk past `MAX_WALK_DOCUMENTS` (16,384) documents in all (the
    /// module doc's "What one walk costs"). The file itself may be
    /// ordinary: the walk ran out of budget before it, so
    /// [`Indexer::collect_from_repo`] refuses a walk holding one of these.
    WalkDocumentLimit {
        /// How many documents the walk had left when it reached the file.
        remaining: usize,
    },
    /// A `.md` file left out whole because it splits into more than
    /// `MAX_FILE_DOCUMENTS` (1,024) documents, a sixteenth of the walk's
    /// budget, so that no one file can spend what every other file needs;
    /// its split stopped as soon as it passed that (the module doc's "What
    /// one walk costs").
    FileDocumentLimit {
        /// The most documents one file may split into.
        limit: usize,
    },
    /// A `.md` file left out whole, unsplit, because its text would have
    /// taken the walk past `MAX_WALK_BYTES` (16 MiB) read in all (the
    /// module doc's "What one walk costs"). As with
    /// [`SkipReason::WalkDocumentLimit`], the walk ran out, not the file,
    /// and [`Indexer::collect_from_repo`] refuses a walk holding one.
    WalkByteLimit {
        /// The file's length in bytes.
        byte_len: u64,
        /// How many bytes the walk had left when it reached the file.
        remaining: u64,
    },
}

/// What one [`Indexer::walk_repo`] found: AICD §25.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoWalk {
    /// Every document collected, ready for [`Indexer::full_rebuild`] or
    /// [`Indexer::incremental_sync`], with no two sharing a path.
    pub documents: Vec<IndexableDocument>,
    /// Every entry left out, with the reason, in walk order.
    pub skipped: Vec<SkippedEntry>,
}

/// A refusal from this module: AICD §25.
#[derive(Debug)]
#[non_exhaustive]
pub enum IndexerError {
    /// Creating or resolving the on-disk directory at `path` failed.
    Directory {
        /// The directory this was attempted against.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A filesystem operation failed: reading the repository's `spec/`
    /// directory itself, or moving a file aside during
    /// [`Indexer::recover`].
    Io {
        /// The path the operation was against.
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
    /// `path` is damaged: SQLite reported `SQLITE_CORRUPT` or
    /// `SQLITE_NOTADB`, or `PRAGMA quick_check`, asked after some other
    /// failure, reported damage in a page it read (the module doc's "What
    /// is reported as corrupt, and what is not"; a check that could not run,
    /// or could not read a page, never produces this). Recovery is
    /// [`Indexer::recover`], which needs the
    /// product's `ProductDb` exclusively; `path` is never deleted, only
    /// moved aside into `index/quarantine/` by that call.
    Corrupt {
        /// The database file found damaged.
        path: PathBuf,
        /// The underlying error: the one the failing call itself got, not
        /// the check's.
        source: rusqlite::Error,
    },
    /// A `rusqlite` call failed in a way none of the above names more
    /// specifically, and the integrity check found nothing wrong or could
    /// not run. Damage the check cannot settle is reported this way too
    /// (the module doc's "What is reported as corrupt, and what is not"):
    /// for an on-disk index, a failure that persists is repaired by
    /// [`Indexer::recover`], which never reads the old file.
    Sqlite {
        /// What was being attempted.
        context: String,
        /// The underlying error.
        source: rusqlite::Error,
    },
    /// `query` holds a `NUL`, the one text FTS5's parser refuses even after
    /// it is quoted as a literal phrase (the module doc's "FTS5 query
    /// safety"), refused before SQLite sees it. Carries only the query text
    /// a caller already had, never an SQLite message. Never returned for a
    /// failure SQLite reports while a search runs: reviews found earlier
    /// versions of this module blaming a valid query for a corrupt index,
    /// and then for a tokenizer the damaged schema named and this build
    /// does not have.
    InvalidQuery {
        /// The text that could not be searched safely.
        query: String,
    },
    /// `query`'s byte length exceeds `MAX_QUERY_BYTES`: the module doc's
    /// "Query cost is bounded by bytes". Refused before FTS5 ever sees it,
    /// so the cost this guards against is never paid.
    QueryTooLarge {
        /// `query`'s length in bytes.
        byte_len: usize,
    },
    /// `documents` (the target set given to [`Indexer::full_rebuild`],
    /// [`Indexer::incremental_sync`] or [`Indexer::recover`]) held the same
    /// `path` more than once. Refused rather than silently keeping
    /// whichever of the two happened to be written last: an adversarial
    /// review found that `full_rebuild` and `incremental_sync` disagreed on
    /// such a set (the former kept every row, the latter kept one and never
    /// settled on repeated syncs of the same unchanged target).
    DuplicatePath {
        /// The path that appeared more than once.
        path: String,
    },
    /// [`Indexer::recover`] refused to move anything, because the directory
    /// it would move files out of or into is not one the product owns
    /// outright, the product directory is a relative path, or an entry at
    /// an index file's name is neither a regular file nor a symbolic link,
    /// such as a directory, which may be another product's (the module
    /// doc's "Recovery").
    RecoveryRefused {
        /// The directory or entry refused.
        path: PathBuf,
        /// Why.
        reason: &'static str,
    },
    /// [`Indexer::open`] refused to open the index, because the file it
    /// would open is not certainly the one under this product's own
    /// directory: the product directory is a relative path, or `index/`,
    /// `index/fts.sqlite` or one of the files SQLite keeps beside it is a
    /// symbolic link, not the kind of entry it must be, or (on Unix) a file
    /// with a second name, a hard link (the module doc's "Recovery").
    /// Nothing was opened or changed.
    OpenRefused {
        /// The entry refused.
        path: PathBuf,
        /// Why.
        reason: &'static str,
    },
    /// A document in the target set has a `title` and `body` longer
    /// together than `MAX_DOCUMENT_BYTES` (64 KiB): refused before anything
    /// is written, as [`IndexerError::DuplicatePath`] is, because a
    /// document that size would lift the bound on every later search's
    /// cost (the module doc's "Query cost is bounded by bytes").
    DocumentTooLarge {
        /// The document's path.
        path: String,
        /// Its `title` and `body` length together, in bytes.
        byte_len: usize,
    },
    /// [`Indexer::walk_repo`] found no `spec/` directory to walk under the
    /// repository root it was given: the root or `spec/` does not exist,
    /// or `spec/` is a symbolic link or not a directory. Refused rather
    /// than reported as a repository with no documents, which a sync would
    /// read as every document removed (the module doc's "The vacuity
    /// trap").
    NoSpecDirectory {
        /// The entry that is missing or not a directory.
        path: PathBuf,
        /// Why.
        reason: &'static str,
    },
    /// [`Indexer::collect_from_repo`] refused a walk its budget cut short:
    /// the walk left files out because it had run out of documents or bytes
    /// before reaching them ([`SkipReason::WalkDocumentLimit`],
    /// [`SkipReason::WalkByteLimit`]), not for anything in those files, so
    /// they may hold documents the index already has, and a set without
    /// them, handed to [`Indexer::incremental_sync`] or
    /// [`Indexer::full_rebuild`], would remove those documents and return
    /// `Ok` (the module doc's "What one walk costs").
    /// [`Indexer::walk_repo`] still returns such a walk, with every entry it
    /// left out, for a caller that decides otherwise.
    WalkBudgetExhausted {
        /// Every file the walk left out for want of budget, with its
        /// reason, in walk order; never empty.
        left_out: Vec<SkippedEntry>,
    },
    /// [`Indexer::recover`] could not move every index file into
    /// quarantine, and moved back every file it had already moved that it
    /// could. Nothing was rebuilt. When `stranded` is empty the index files
    /// are exactly where they were before the call, and the quarantine
    /// directory made for them is left in place, empty, so that no later
    /// recovery ever takes its name; otherwise each path in it is a file
    /// that is in `quarantine` and could not be put back, and every other
    /// file is at its live path.
    QuarantineIncomplete {
        /// The file whose move failed, at its live path.
        path: PathBuf,
        /// Why that move failed.
        source: std::io::Error,
        /// The quarantine directory the files were being moved into.
        quarantine: PathBuf,
        /// Files moved into `quarantine` that could not be moved back.
        stranded: Vec<PathBuf>,
    },
}

impl IndexerError {
    /// Wraps a `rusqlite` failure with what was being attempted, for the one
    /// variant that is not any of this module's named refusals; the specific
    /// cases of a write lock already held and of damage are split out by
    /// [`classify`] at each call site, the same split
    /// `crates/ori-store/src/db.rs`'s `DbError::sqlite` and `is_busy` make
    /// for `product.sqlite`'s own lock.
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
            Self::Corrupt { path, source } if path.as_os_str() == IN_MEMORY_LABEL => write!(
                f,
                "{} is corrupt: {source}; an in-memory index is derived data held nowhere \
                 else: drop this Indexer and open a new one",
                path.display()
            ),
            Self::Corrupt { path, source } => write!(
                f,
                "{} is corrupt: {source}; the index is derived data: recover it with \
                 Indexer::recover, which needs the product's ProductDb exclusively (drop every \
                 Indexer on the product first), moves the damaged files aside into \
                 index/quarantine/ and rebuilds the index from the target set it is given",
                path.display()
            ),
            Self::Sqlite { context, source } => write!(f, "{context}: {source}"),
            Self::InvalidQuery { query } => write!(f, "invalid query: {query}"),
            Self::QueryTooLarge { byte_len } => write!(
                f,
                "query is too large: {byte_len} bytes (max {MAX_QUERY_BYTES})"
            ),
            Self::DuplicatePath { path } => write!(
                f,
                "{path} appears more than once in the target set; full_rebuild and \
                 incremental_sync both refuse rather than guess which one should win"
            ),
            Self::RecoveryRefused { path, reason } => write!(
                f,
                "refusing to recover the index through {}: {reason}",
                path.display()
            ),
            Self::OpenRefused { path, reason } => write!(
                f,
                "refusing to open the index through {}: {reason}",
                path.display()
            ),
            Self::DocumentTooLarge { path, byte_len } => write!(
                f,
                "{path} is too large to index: {byte_len} bytes of title and body (max \
                 {MAX_DOCUMENT_BYTES})"
            ),
            Self::NoSpecDirectory { path, reason } => write!(
                f,
                "no spec/ directory to walk at {}: {reason}; refusing to report an empty \
                 repository, which a sync would read as every document removed",
                path.display()
            ),
            Self::WalkBudgetExhausted { left_out } => {
                write!(
                    f,
                    "the repository walk ran out of its budget and left {} file(s) out",
                    left_out.len()
                )?;
                if let Some(first) = left_out.first() {
                    write!(f, ", the first {}", first.path.display())?;
                }
                f.write_str(
                    "; refusing to hand back a document set a sync would read as their documents \
                     removed (Indexer::walk_repo returns the walk with every entry it left out)",
                )
            }
            Self::QuarantineIncomplete {
                path,
                source,
                quarantine,
                stranded,
            } if stranded.is_empty() => write!(
                f,
                "{} could not be moved into quarantine: {source}; every file already moved \
                 was put back, so the index files are where they were and nothing was rebuilt \
                 ({} is left empty, and no later recovery reuses its name)",
                path.display(),
                quarantine.display()
            ),
            Self::QuarantineIncomplete {
                path,
                source,
                quarantine,
                stranded,
            } => {
                write!(
                    f,
                    "{} could not be moved into quarantine: {source}; nothing was rebuilt, and \
                     these files are in {} and could not be moved back:",
                    path.display(),
                    quarantine.display()
                )?;
                for file in stranded {
                    write!(f, " {}", file.display())?;
                }
                f.write_str("; every other index file is at its live path")
            }
        }
    }
}

impl std::error::Error for IndexerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Directory { source, .. }
            | Self::Io { source, .. }
            | Self::QuarantineIncomplete { source, .. } => Some(source),
            Self::Corrupt { source, .. } | Self::Sqlite { source, .. } => Some(source),
            Self::Locked { .. }
            | Self::InvalidQuery { .. }
            | Self::QueryTooLarge { .. }
            | Self::DuplicatePath { .. }
            | Self::RecoveryRefused { .. }
            | Self::OpenRefused { .. }
            | Self::DocumentTooLarge { .. }
            | Self::NoSpecDirectory { .. }
            | Self::WalkBudgetExhausted { .. } => None,
        }
    }
}

/// The label an [`Indexer::open_in_memory`] index is named by in an error,
/// since it has no file.
const IN_MEMORY_LABEL: &str = ":memory:";

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

/// Whether a `rusqlite` failure is SQLite itself reporting the file damaged:
/// `SQLITE_CORRUPT` (`ErrorCode::DatabaseCorrupt`, every extended code of
/// it, FTS5's `SQLITE_CORRUPT_VTAB` included) or `SQLITE_NOTADB`
/// (`ErrorCode::NotADatabase`, what a destroyed header produces: a review
/// found round 4 reporting that one as a generic [`IndexerError::Sqlite`]
/// at open, which left the damage with no recovery route at all).
fn is_corrupt(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == ErrorCode::DatabaseCorrupt || inner.code == ErrorCode::NotADatabase
    )
}

/// What `PRAGMA quick_check` said about a file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Integrity {
    /// It returned exactly `ok`.
    Intact,
    /// It reported damage it read, or failed with
    /// `SQLITE_CORRUPT`/`SQLITE_NOTADB`.
    Damaged,
    /// It could not run to a verdict (busy, locked, out of memory, an FTS5
    /// format this build does not read, a page it could not read, or
    /// anything else).
    Undetermined,
}

/// What one row of `PRAGMA quick_check`'s report says, other than `ok`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReportRow {
    /// Damage the check read.
    Damage,
    /// The check could not read part of the file, so it read nothing there
    /// and every other line of its report may be a consequence of that
    /// (see [`report_names_an_unread_page`]).
    UnreadPage,
    /// FTS5 could not run its own check ("unable to validate"): says
    /// nothing either way.
    NotValidated,
}

/// Reads one row of `PRAGMA quick_check`'s report.
fn report_row(text: &str) -> ReportRow {
    if text.starts_with("unable to validate") {
        ReportRow::NotValidated
    } else if report_names_an_unread_page(text) {
        ReportRow::UnreadPage
    } else {
        ReportRow::Damage
    }
}

/// Whether a row of `PRAGMA quick_check`'s report says the check failed to
/// read a page, rather than that a page it read was damaged: "unable to get
/// the page. error code=N" for any `N` that is not `SQLITE_CORRUPT` or
/// `SQLITE_NOTADB`, "failed to get page N" (a freelist or overflow page),
/// or "Failed to read ptrmap key=N".
///
/// A review found the bundled SQLite (3.53.2) reporting a genuine
/// out-of-memory condition this way rather than as an error: its b-tree
/// check turns a failed page read into an out-of-memory error only when the
/// read fails with `SQLITE_IOERR_NOMEM`, while the page cache fails with
/// plain `SQLITE_NOMEM`, so the check writes "unable to get the page. error
/// code=7" into its report and carries on. Every other line of that report
/// is then a consequence of the pages it never read ("Page 7: never used",
/// "Child page depth differs", "wrong # of entries in index"), so the whole
/// report is read as "could not check", never as damage. Real damage in a
/// page that was read is reported by the other messages, which is what
/// [`Integrity::Damaged`] rests on; a page whose read fails with
/// `SQLITE_CORRUPT` is still damage.
fn report_names_an_unread_page(text: &str) -> bool {
    text.lines().any(|line| {
        line.contains("failed to get page")
            || line.contains("Failed to read ptrmap")
            || line
                .split_once("unable to get the page. error code=")
                .is_some_and(|(_, code)| {
                    code.trim().parse::<i32>().map_or(true, |code| {
                        let primary = code & 0xff;
                        primary != rusqlite::ffi::SQLITE_CORRUPT
                            && primary != rusqlite::ffi::SQLITE_NOTADB
                    })
                })
    })
}

/// Runs `PRAGMA quick_check` on `conn`: a read, never a write, so it needs
/// no write lock and can neither be refused by a concurrent writer nor lock
/// one out (the module doc's "What is reported as corrupt, and what is
/// not"). In the SQLite this workspace bundles it checks every b-tree page
/// and then runs FTS5's own inverted-index check through the virtual
/// table's integrity method; a row naming damage found by either is
/// [`Integrity::Damaged`] even if a later step of the pragma fails.
///
/// It reads a clean snapshot of the file as committed, never a write
/// transaction a failed statement left open (the module doc's "What is
/// reported as corrupt, and what is not" has the review): any transaction
/// `conn` is in is rolled back first, which also discards FTS5's in-memory
/// state for it, and the check runs in a read transaction of its own,
/// ended with a rollback, so `conn` is left in no transaction at all. The
/// rollback is prepared before anything else, so ending the check never
/// needs memory to prepare it; if it cannot be prepared, or the caller's
/// transaction cannot be ended, the verdict is [`Integrity::Undetermined`]
/// and nothing is read. A read statement of the caller's still in progress
/// in no transaction keeps its own snapshot, which the check then shares;
/// that snapshot holds nothing uncommitted. Round 8 ran the check inside a
/// savepoint of whatever transaction `conn` was in, and a review found a
/// damaged file making both ends of that savepoint fail, which left every
/// later call on the connection refused as a transaction within a
/// transaction.
///
/// Within that read transaction it first opens a cursor on the table. That
/// order is load-bearing: FTS5 keeps a per-connection cache of the index's
/// structure and refreshes it only when a cursor opens, and its integrity
/// method reads the cache as it stands. Measured while building this: after
/// another connection's commits, a bare `PRAGMA quick_check` on a perfectly
/// healthy index reported "fts5: checksum mismatch" 133 times in 200;
/// opened this way, 0 in 200
/// (`tests::ori_t_0035_the_check_never_reads_a_stale_view_of_an_index_another_connection_changed`).
///
/// A row that begins "unable to validate" is FTS5 saying it could not run
/// its check, not that it found damage; the bundled version reports that as
/// an error rather than a row, and it is read as saying nothing all the
/// same, so a different SQLite that did report it as a row could never turn
/// "could not check" into "damaged". A row that says the check failed to
/// read a page makes the whole verdict [`Integrity::Undetermined`]
/// ([`report_names_an_unread_page`] has the measurement), so every row is
/// read before a verdict is given.
fn quick_check(conn: &Connection) -> Integrity {
    let Ok(mut rollback) = conn.prepare("ROLLBACK") else {
        return Integrity::Undetermined;
    };
    if !conn.is_autocommit() && (rollback.execute([]).is_err() || !conn.is_autocommit()) {
        return Integrity::Undetermined;
    }
    if conn.execute_batch("BEGIN DEFERRED").is_err() {
        return Integrity::Undetermined;
    }
    let verdict = quick_check_in_one_snapshot(conn);
    for _ in 0..2 {
        if conn.is_autocommit() {
            break;
        }
        let _ = rollback.execute([]);
    }
    verdict
}

/// [`quick_check`]'s body, run inside its own read transaction.
fn quick_check_in_one_snapshot(conn: &Connection) -> Integrity {
    match conn.query_row(
        "SELECT count(*) FROM documents WHERE rowid = 0",
        [],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(_) => {}
        Err(error) if is_corrupt(&error) => return Integrity::Damaged,
        Err(_) => return Integrity::Undetermined,
    }
    let mut statement = match conn.prepare("PRAGMA quick_check") {
        Ok(statement) => statement,
        Err(error) if is_corrupt(&error) => return Integrity::Damaged,
        Err(_) => return Integrity::Undetermined,
    };
    let mut rows = match statement.query([]) {
        Ok(rows) => rows,
        Err(error) if is_corrupt(&error) => return Integrity::Damaged,
        Err(_) => return Integrity::Undetermined,
    };
    let (mut saw_ok, mut damage, mut failed) = (false, false, false);
    loop {
        match rows.next() {
            Ok(Some(row)) => match row.get_ref(0) {
                Ok(ValueRef::Text(b"ok")) => saw_ok = true,
                Ok(ValueRef::Text(text)) => match report_row(&String::from_utf8_lossy(text)) {
                    ReportRow::UnreadPage => return Integrity::Undetermined,
                    ReportRow::NotValidated => {}
                    ReportRow::Damage => damage = true,
                },
                Ok(_) | Err(_) => return Integrity::Undetermined,
            },
            Ok(None) => break,
            Err(error) if is_corrupt(&error) => return Integrity::Damaged,
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    if damage {
        Integrity::Damaged
    } else if saw_ok && !failed {
        Integrity::Intact
    } else {
        Integrity::Undetermined
    }
}

/// Maps a `rusqlite` failure to the refusal it belongs to without asking
/// the file anything: a locked file to [`IndexerError::Locked`], a failure
/// SQLite itself called damage to [`IndexerError::Corrupt`], everything
/// else to [`IndexerError::Sqlite`]. For the two places no connection
/// exists yet to ask, or nothing has run that could have met damage:
/// opening the file, and beginning a transaction.
fn classify_without_check(path: &Path, context: &str, source: rusqlite::Error) -> IndexerError {
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

/// The one classifier every read and every write in this module uses once a
/// connection exists: the module doc's "What is reported as corrupt, and
/// what is not", as code. A lock is [`IndexerError::Locked`] and
/// `SQLITE_CORRUPT`/`SQLITE_NOTADB` is [`IndexerError::Corrupt`], directly;
/// anything else asks [`quick_check`] on `conn` (the same connection, or
/// the caller's transaction, which derefs to one, and which the check
/// rolls back before it reads anything, so every caller returns the error
/// straight away) and is `Corrupt` only when the check reports
/// [`Integrity::Damaged`]. When it reports
/// [`Integrity::Intact`] or [`Integrity::Undetermined`] the original error is
/// returned unchanged as [`IndexerError::Sqlite`]: a check that could not
/// run (out of memory, busy) is never evidence of damage. The original
/// `source`, never the check's own error, is what either variant carries.
///
/// Round 4's version ran FTS5's `integrity-check` command, a write, and
/// read any failure of it as corruption; the module doc records what that
/// cost.
fn classify(
    conn: &Connection,
    path: &Path,
    context: &str,
    source: rusqlite::Error,
) -> IndexerError {
    if is_locked(&source) || is_corrupt(&source) {
        return classify_without_check(path, context, source);
    }
    match quick_check(conn) {
        Integrity::Damaged => IndexerError::Corrupt {
            path: path.to_owned(),
            source,
        },
        Integrity::Intact | Integrity::Undetermined => IndexerError::sqlite(context, source),
    }
}

/// Refuses `documents` if any two elements share a `path` (the identity
/// [`Indexer::full_rebuild`] and [`Indexer::incremental_sync`] both key on)
/// or any one of them is larger than `MAX_DOCUMENT_BYTES`. Checked before
/// either does any write, and before [`Indexer::recover`] moves anything,
/// so a caller sees the refusal before any partial effect.
///
/// # Errors
///
/// [`IndexerError::DocumentTooLarge`] or [`IndexerError::DuplicatePath`]
/// for the first offending document, in `documents`' own order.
fn refuse_invalid_target(documents: &[IndexableDocument]) -> Result<(), IndexerError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for document in documents {
        refuse_oversized(document)?;
        if !seen.insert(document.path.as_str()) {
            return Err(IndexerError::DuplicatePath {
                path: document.path.clone(),
            });
        }
    }
    Ok(())
}

/// Refuses `document` if its `title` and `body` together are longer than
/// `MAX_DOCUMENT_BYTES`: the per-document half of the module doc's "Query
/// cost is bounded by bytes".
///
/// # Errors
///
/// [`IndexerError::DocumentTooLarge`].
fn refuse_oversized(document: &IndexableDocument) -> Result<(), IndexerError> {
    let byte_len = document.indexed_byte_len();
    if byte_len > MAX_DOCUMENT_BYTES {
        return Err(IndexerError::DocumentTooLarge {
            path: document.path.clone(),
            byte_len,
        });
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

/// The largest `query` [`Indexer::search`] accepts, in bytes: half of the
/// bound on its cost, the module doc's "Query cost is bounded by bytes"
/// ([`MAX_DOCUMENT_BYTES`] is the other half). An adversarial review measured 1 MB of a common
/// repeated term costing 4.5 seconds and 2.18 GB of resident memory,
/// because FTS5 opens one index iterator per phrase term and repeated
/// tokens are not deduplicated. 1 KiB is generous for a legitimate search
/// phrase (`spec/API_SPEC.md`'s `aicd_search(query)` is a short free-text
/// query, never a document body) and admits at most 512 FTS5 terms in any
/// script, since every term is at least one byte and every two are
/// separated by at least one more.
const MAX_QUERY_BYTES: usize = 1024;

/// The largest document this index stores, in bytes of `title` and `body`
/// together (the two columns FTS5 indexes): the other half of the module
/// doc's "Query cost is bounded by bytes". [`MAX_QUERY_BYTES`] bounds how
/// many terms a phrase has; this bounds how many positions each term can
/// have in one document, and a phrase's memory is about the product of the
/// two. Measured on this build, release, as SQLite's own heap high-water
/// mark around [`Indexer::search`]'s own statement, the query `a` repeated
/// 512 times (1,023 bytes) against one document of `a` repeated to fill
/// it: 24.6 MB at 16 KiB, 40.6 MB at 32 KiB, 72.6 MB and 0.15 to 0.18
/// seconds at 64 KiB, 136.7 MB at 128 KiB, 264.8 MB at 256 KiB, 521.0 MB
/// at 512 KiB, and 1.03 GB and 6.6 seconds at the review's 2 MB; the same
/// document searched for `a` once costs 1.0 MB at 64 KiB. 64 KiB admits
/// the largest document this repository has, the 20,797-byte second ADR,
/// which is indexed whole as one document, three times over, and holds
/// the worst search within the query cap under 75 MB of heap; a document
/// past it is left out by the walk and refused by every writer, both with
/// a reason.
const MAX_DOCUMENT_BYTES: usize = 64 * 1024;

/// The largest file [`Indexer::walk_repo`] reads, in bytes: 1 MiB. Not a
/// bound on search cost ([`MAX_DOCUMENT_BYTES`] is), but on what one file
/// costs the walk itself, which holds a file's whole text and its split
/// documents in memory at once. The largest file under this repository's
/// `spec/` today is 32,646 bytes, so it is thirty-two times what is needed;
/// a section file this long can still hold many documents under the
/// per-document cap.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// The most documents one [`Indexer::walk_repo`] splits out of the files it
/// reads, in all: the module doc's "What one walk costs". A document costs
/// the walk a few hundred bytes beyond its own text (its path twice, three
/// strings, a set entry), so a file of empty headings, two bytes each,
/// turned a 1 MiB file into 524,288 documents and about 200 MB of memory in
/// a review. 16,384 is about 74 times the 221 documents this repository's
/// own `spec/` produces today. Every document a file splits into is
/// counted, the ones the walk then leaves out (a repeated path, one over
/// [`MAX_DOCUMENT_BYTES`]) included, because each of those costs the walk
/// an entry in its skip list as a kept one costs an entry in its result:
/// counting only the kept ones would leave the skip list bounded by
/// nothing but [`MAX_WALK_BYTES`], about 1.7 million ten-byte criteria rows
/// repeating one identifier. What keeps one file from spending this
/// budget is [`MAX_FILE_DOCUMENTS`].
const MAX_WALK_DOCUMENTS: usize = 16_384;

/// The most documents one file may split into: 1,024, a sixteenth of
/// [`MAX_WALK_DOCUMENTS`], the same share of the walk's documents that
/// [`MAX_FILE_BYTES`] is of [`MAX_WALK_BYTES`]. A file past it is left out
/// whole and recorded ([`SkipReason::FileDocumentLimit`]), and its split
/// stops as soon as it passes the cap. A review found one 32 KiB file of
/// 16,384 bare `#` lines, sorting first, spending the whole walk's budget,
/// so that every later file, this repository's own specification
/// included, was left out, and a sync then removed their documents and
/// returned `Ok`; and a criteria file of 16,384 rows repeating one
/// identifier doing the same while storing one document. No one file can
/// now spend more than its share. The file of this repository's `spec/`
/// with the most documents today has about 45, so this is over twenty
/// times what is needed.
const MAX_FILE_DOCUMENTS: usize = MAX_WALK_DOCUMENTS / 16;

/// The most bytes of file text one [`Indexer::walk_repo`] reads into
/// documents, in all: 16 MiB, the module doc's "What one walk costs". The
/// documents a walk returns hold their text, so this bounds that half of
/// its memory as [`MAX_WALK_DOCUMENTS`] bounds the other. This repository's
/// `spec/` is about 198 KB of Markdown today.
const MAX_WALK_BYTES: u64 = 16 * 1024 * 1024;

/// Commits `tx`. In test builds only, it then runs the after-commit hook a
/// test installed on this thread, which is how
/// `tests::ori_t_0035_index_report_total_is_read_before_commit_never_after_another_writers_commit`
/// commits a second connection's write in the exact window between this
/// commit and whatever its caller does next, deterministically rather than
/// by hoping a race lands there. Compiled out of every other build.
fn commit(tx: rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
    tx.commit()?;
    #[cfg(test)]
    tests::run_after_commit_hook();
    Ok(())
}

/// The repository indexer: a SQLite FTS5 virtual table over
/// [`IndexableDocument`]s, keyed by `path`: AICD §25.
///
/// An on-disk `Indexer<'db>` is opened from, and borrows for its whole life,
/// the [`ProductDb`] that owns the product directory; the module doc's
/// "Recovery" is why. `PhantomData<&'db ()>` rather than a stored
/// `&'db ProductDb` carries that borrow: `ProductDb` holds
/// `rusqlite::Connection`s, which are not `Sync`, so a stored reference
/// would make every `Indexer` impossible to hand to another thread for no
/// gain, while the lifetime alone is what the borrow checker reasons about.
pub struct Indexer<'db> {
    conn: Connection,
    /// The file this connection is open against, `Some` only for
    /// [`Indexer::open`]; `None` for [`Indexer::open_in_memory`], which has
    /// no path to name in [`IndexerError::Locked`] or a diagnostic.
    path: Option<PathBuf>,
    product: PhantomData<&'db ()>,
}

/// Empty, and load-bearing: an explicit `Drop` makes dropping an
/// `Indexer<'db>` a use of `'db`, so the borrow of the [`ProductDb`] it was
/// opened from lasts until its connection is actually closed, not merely
/// until its last method call. Without it, an `Indexer` still in scope but
/// no longer used would let [`Indexer::recover`] compile and run while its
/// connection was still open on the file being moved, which is what the
/// module doc's "Recovery" proves cannot happen. The second `compile_fail`
/// example on [`Indexer::recover`] pins this: remove this impl and that
/// example compiles.
impl Drop for Indexer<'_> {
    fn drop(&mut self) {}
}

impl Indexer<'static> {
    /// An index held only in memory, for tests and for any caller that
    /// wants no on-disk footprint. It borrows no [`ProductDb`], since it has
    /// no file anything could need recovering.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Sqlite`] if the schema cannot be created.
    pub fn open_in_memory() -> Result<Self, IndexerError> {
        let display_path = PathBuf::from(IN_MEMORY_LABEL);
        let conn = Connection::open_in_memory().map_err(|source| {
            classify_without_check(&display_path, "open an in-memory database", source)
        })?;
        Self::configure(conn, None)
    }
}

impl<'db> Indexer<'db> {
    /// Opens the product's on-disk index, `<product dir>/index/fts.sqlite`
    /// (`spec/LLD.md` section 6), creating the directory and the file if
    /// either is absent, and borrows `db` for as long as the returned
    /// `Indexer` lives. Never `product.sqlite`; see the module doc's "The
    /// index is derived". Any number of `Indexer`s may be open on one
    /// product at once.
    ///
    /// The file opened is always the one under this product's own
    /// directory, never one reached through a link: the borrow of `db` is
    /// what proves [`Indexer::recover`] safe, and it proves nothing about a
    /// file that belongs to another product (the module doc's "Recovery").
    /// So `db`'s directory must be absolute, and neither `index/` nor
    /// `index/fts.sqlite` nor any file SQLite keeps beside it may be a
    /// symbolic link, nor (on Unix) a file with a second name; the path is
    /// resolved once, here, and SQLite is told to refuse a link anywhere in
    /// it as well.
    ///
    /// # Errors
    ///
    /// [`IndexerError::OpenRefused`] if `db`'s directory is a relative path,
    /// or `index/` or an index file is a symbolic link or the wrong kind of
    /// entry, or an index file has a second name; [`IndexerError::Directory`] if the product directory cannot be
    /// resolved or `index/` cannot be created;
    /// [`IndexerError::Corrupt`] if the file exists and is damaged in a way
    /// opening it meets (a destroyed header, a damaged schema page, a
    /// truncated file); [`Indexer::recover`] repairs every such case;
    /// [`IndexerError::Sqlite`] if the file cannot be opened or the schema
    /// cannot be created for another reason.
    pub fn open(db: &'db ProductDb) -> Result<Self, IndexerError> {
        let index_dir = owned_index_dir(db, open_refused)?;
        refuse_unowned_index_files(&index_dir)?;
        Self::open_file(index_dir.join(INDEX_FILE))
    }

    /// Opens (creating if absent) the index file at exactly `path`, with
    /// `SQLITE_OPEN_NOFOLLOW`, so SQLite itself refuses a symbolic link
    /// anywhere in `path` (it already opens every file with `O_NOFOLLOW`,
    /// which covers only the last component). Private: every caller
    /// outside this module reaches a file only through a [`ProductDb`],
    /// which is what "Recovery" in the module doc rests on.
    fn open_file(path: PathBuf) -> Result<Self, IndexerError> {
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::default() | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|source| classify_without_check(&path, "open", source))?;
        Self::configure(conn, Some(path))
    }

    /// Sets this connection's pragmas and creates the schema if absent.
    ///
    /// Every failure here goes through [`classify`], not the generic
    /// [`IndexerError::sqlite`]: an adversarial review found `Indexer::open`
    /// reporting a corrupt file (most often surfacing at the `journal_mode`
    /// pragma, the first real statement run against it) as a bare
    /// [`IndexerError::Sqlite`], and a later one found a destroyed header
    /// (`SQLITE_NOTADB`) still reported that way, which left
    /// [`Indexer::recover`]'s caller no signal to call it.
    fn configure(conn: Connection, path: Option<PathBuf>) -> Result<Self, IndexerError> {
        let display_path = path
            .clone()
            .unwrap_or_else(|| PathBuf::from(IN_MEMORY_LABEL));
        // WAL: concurrent readers never block a writer or each other (the
        // same choice `crates/ori-store/src/db.rs`'s `open_connection` makes
        // for `product.sqlite`). A zero busy_timeout is set explicitly,
        // deliberately, rather than left at SQLite's own default: see the
        // module doc's "Concurrency".
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|source| classify(&conn, &display_path, "set journal_mode", source))?;
        conn.busy_timeout(WRITE_BUSY_TIMEOUT)
            .map_err(|source| classify(&conn, &display_path, "set busy_timeout", source))?;
        conn.execute_batch(CREATE_TABLE_SQL).map_err(|source| {
            classify(&conn, &display_path, "create the documents table", source)
        })?;
        Ok(Self {
            conn,
            path,
            product: PhantomData,
        })
    }

    /// The path [`IndexerError::Locked`] and [`IndexerError::Sqlite`] name
    /// for a refusal: the real file for [`Indexer::open`], or a synthetic
    /// label for [`Indexer::open_in_memory`].
    fn display_path(&self) -> PathBuf {
        self.path
            .clone()
            .unwrap_or_else(|| PathBuf::from(IN_MEMORY_LABEL))
    }

    /// Every document presently in the index, in path order, each paired
    /// with its stored checksum.
    ///
    /// The dump this module's rebuild-versus-incremental proof compares:
    /// read fresh from the index itself on every call, never from
    /// bookkeeping held in memory, so it is correct even for an [`Indexer`]
    /// freshly opened on an existing on-disk file whose history this
    /// process never saw. A row whose stored `kind` is not one of the four
    /// [`DocumentKind`]s (a row some other writer stored) is not a document
    /// this build can represent and is left out;
    /// [`Indexer::incremental_sync`] and [`Indexer::full_rebuild`] both
    /// rewrite or remove such a row.
    ///
    /// # Errors
    ///
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the read fails for another reason.
    pub fn all_documents(&self) -> Result<Vec<(IndexableDocument, u64)>, IndexerError> {
        Self::all_documents_on(&self.conn, &self.display_path())
    }

    /// [`Indexer::all_documents`]'s body, taking any `&Connection`.
    fn all_documents_on(
        conn: &Connection,
        display_path: &Path,
    ) -> Result<Vec<(IndexableDocument, u64)>, IndexerError> {
        let mut statement = conn
            .prepare("SELECT path, kind, title, body, checksum FROM documents ORDER BY path")
            .map_err(|source| {
                classify(
                    conn,
                    display_path,
                    "prepare a full read of documents",
                    source,
                )
            })?;
        let rows = statement
            .query_map([], Self::row_to_document)
            .map_err(|source| classify(conn, display_path, "read every document back", source))?;
        let mut out = Vec::new();
        for row in rows {
            let pair = row
                .map_err(|source| classify(conn, display_path, "read one document row", source))?;
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
            .map_err(|source| classify(conn, display_path, "count documents", source))?;
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
    /// [`IndexerError::DocumentTooLarge`] if `document` is larger than the
    /// module doc's per-document cap, checked before anything is written;
    /// [`IndexerError::Locked`] if another writer holds the lock;
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the write fails for another reason.
    pub fn add_or_replace(&mut self, document: &IndexableDocument) -> Result<(), IndexerError> {
        refuse_oversized(document)?;
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| classify_without_check(&path, "begin transaction", source))?;
        Self::delete_path(&tx, &document.path, &path)?;
        Self::insert(&tx, document, &path)?;
        commit(tx).map_err(|source| classify(&self.conn, &path, "commit", source))
    }

    /// Clears the index and indexes exactly `documents`: a full rebuild, the
    /// reconstruction the module doc's "the index is derived" section names.
    ///
    /// Inside one write transaction, this drops the `documents` table (and
    /// with it every FTS5 shadow table) and recreates it before inserting,
    /// so damage done to the shadow tables' rows through SQL is repaired as
    /// a side effect of its ordinary job, and SQLite's own locking
    /// serializes it against every other connection the ordinary way; no
    /// file is ever deleted or replaced here. Damage to a *page* of the file
    /// makes the `DROP` itself fail, every time: that is
    /// [`IndexerError::Corrupt`], and the repair is [`Indexer::recover`],
    /// never a retry of this (the module doc's "Recovery").
    ///
    /// The report counts what the drop discards, read first in the same
    /// transaction: every stored row, as [`IndexReport::replaced`] when a
    /// target document has its path and as [`IndexReport::removed`] when
    /// none does. A review found `removed` always 0 here, even when the
    /// rebuild dropped rows `incremental_sync` would have reported removed.
    /// The rows are read from FTS5's own content table, `documents_content`,
    /// never through the virtual table, so damage to the inverted index,
    /// which the drop repairs, cannot fail the count first.
    ///
    /// # Errors
    ///
    /// [`IndexerError::DuplicatePath`] if `documents` holds one path twice;
    /// [`IndexerError::DocumentTooLarge`] if one of them is larger than the
    /// module doc's per-document cap;
    /// [`IndexerError::Locked`] if another writer holds the lock;
    /// [`IndexerError::Corrupt`] if the file is damaged beyond what
    /// rebuilding the table can repair;
    /// [`IndexerError::Sqlite`] if a write fails for another reason.
    pub fn full_rebuild(
        &mut self,
        documents: &[IndexableDocument],
    ) -> Result<IndexReport, IndexerError> {
        refuse_invalid_target(documents)?;
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| classify_without_check(&path, "begin transaction", source))?;
        let report = Self::rebuild_on(&tx, documents, &path)?;
        commit(tx).map_err(|source| classify(&self.conn, &path, "commit", source))?;
        Ok(report)
    }

    /// [`Indexer::full_rebuild`]'s body, inside the write transaction `tx`
    /// its caller opened and commits: counts what the drop discards, drops
    /// and recreates the table, inserts every document, and reads `total`
    /// before the commit (the module doc's "Reads and writes share one
    /// transaction"). [`Indexer::incremental_sync`] runs it too, for an
    /// index holding an FTS5 option this build did not set.
    fn rebuild_on(
        tx: &Connection,
        documents: &[IndexableDocument],
        path: &Path,
    ) -> Result<IndexReport, IndexerError> {
        let (removed, replaced) = Self::rows_a_rebuild_discards(tx, documents, path)?;
        tx.execute_batch("DROP TABLE documents;")
            .map_err(|source| {
                classify(
                    tx,
                    path,
                    "drop the documents table for a full rebuild",
                    source,
                )
            })?;
        tx.execute_batch(CREATE_TABLE_SQL).map_err(|source| {
            classify(
                tx,
                path,
                "recreate the documents table for a full rebuild",
                source,
            )
        })?;
        for document in documents {
            Self::insert(tx, document, path)?;
        }
        let total = Self::total_indexed_on(tx, path)?;
        Ok(IndexReport {
            upserted: documents.len(),
            removed,
            replaced,
            total,
        })
    }

    /// Whether `documents_config`, the table where FTS5 keeps the options
    /// set on the index, holds any row but the one FTS5 writes itself when
    /// the table is created, its format `version`. This build sets no
    /// option; any other row (`pgsz`, `automerge`, `rank`,
    /// `secure-delete`, or any option a later FTS5 adds) was set by another
    /// writer through SQL. A file with no such table holds no option.
    fn holds_an_option_this_build_did_not_set(
        conn: &Connection,
        display_path: &Path,
    ) -> Result<bool, IndexerError> {
        let has_config_table: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_schema \
                 WHERE type = 'table' AND name = 'documents_config'",
                [],
                |row| row.get(0),
            )
            .map_err(|source| classify(conn, display_path, "look up the config table", source))?;
        if !has_config_table {
            return Ok(false);
        }
        conn.query_row(
            "SELECT count(*) > 0 FROM documents_config WHERE k IS NOT 'version'",
            [],
            |row| row.get(0),
        )
        .map_err(|source| classify(conn, display_path, "read the FTS5 options", source))
    }

    /// Every row a [`Indexer::full_rebuild`] to `documents` is about to
    /// drop, as `(removed, replaced)`: rows whose path no document in
    /// `documents` has (or that have no text path), and rows at a path one
    /// does. Read from `documents_content`, FTS5's own table of the stored
    /// column values, one row per document, where the first declared
    /// column, `path`, is `c0`; see `full_rebuild`'s doc for why not
    /// through the virtual table. A file with no such table stores no rows
    /// this could count, and neither half is counted.
    fn rows_a_rebuild_discards(
        conn: &Connection,
        documents: &[IndexableDocument],
        display_path: &Path,
    ) -> Result<(usize, usize), IndexerError> {
        let has_content_table: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_schema \
                 WHERE type = 'table' AND name = 'documents_content'",
                [],
                |row| row.get(0),
            )
            .map_err(|source| classify(conn, display_path, "look up the content table", source))?;
        if !has_content_table {
            return Ok((0, 0));
        }
        let target_paths: BTreeSet<&str> = documents
            .iter()
            .map(|document| document.path.as_str())
            .collect();
        let mut statement = conn
            .prepare("SELECT c0 FROM documents_content")
            .map_err(|source| {
                classify(conn, display_path, "prepare a count of stored rows", source)
            })?;
        let mut rows = statement
            .query([])
            .map_err(|source| classify(conn, display_path, "count stored rows", source))?;
        let (mut removed, mut replaced) = (0usize, 0usize);
        loop {
            let row = match rows.next() {
                Ok(Some(row)) => row,
                Ok(None) => break,
                Err(source) => {
                    return Err(classify(conn, display_path, "count one stored row", source));
                }
            };
            let at_target_path = match row.get_ref(0) {
                Ok(ValueRef::Text(bytes)) => std::str::from_utf8(bytes)
                    .is_ok_and(|stored_path| target_paths.contains(stored_path)),
                Ok(_) => false,
                Err(source) => {
                    return Err(classify(conn, display_path, "count one stored row", source));
                }
            };
            if at_target_path {
                replaced += 1;
            } else {
                removed += 1;
            }
        }
        Ok((removed, replaced))
    }

    /// Re-indexes to match `documents` exactly ("re-index on merge", PRD
    /// K-02), leaving the stored rows exactly as [`Indexer::full_rebuild`]
    /// of the same target set would: every stored row whose path is not in
    /// `documents` is deleted; every document in `documents` is written
    /// (delete-then-add) unless exactly one row is stored at its path and
    /// every column of it (`kind`, `title` and `body` as text, `checksum`
    /// as an integer) is exactly what this build writes for that document,
    /// in which case it is left alone.
    ///
    /// The diff basis is every stored row's `rowid` and every column, read
    /// without assuming their types and compared against the target as each
    /// row is read (so no stored body is held in memory), never
    /// [`Indexer::all_documents`] (which leaves out a row whose `kind` does
    /// not parse). Reviews found four ways a row some other writer stored
    /// (a manual edit, an older or newer build) outlived every sync while
    /// `full_rebuild` replaced it: a `kind` that does not parse, removed by
    /// round 3 but still skipped when its path *was* in the target and its
    /// checksum happened to match (the checksum hashes the kind this build
    /// would store, not the text actually stored); two rows at one path; a
    /// `path` or `checksum` of the wrong type, which failed every sync
    /// outright; and, once those were fixed, a `title` or `body` changed
    /// with `kind` and `checksum` left alone, which round 5 compared by
    /// checksum only and so never repaired (a `NULL` body made
    /// [`Indexer::all_documents`] fail on every call after). Each is now a
    /// difference like any other: a row that is not exactly what this build
    /// would have written is rewritten, and one whose path is not text at
    /// all is deleted by its `rowid`, since no target document can name it.
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
    /// Nor does it keep an FTS5 option this build did not set. FTS5 stores
    /// options in the index itself (`pgsz`, `automerge`, `rank`,
    /// `secure-delete` and the rest), and another writer can set any of
    /// them through SQL; this build sets none, and [`Indexer::full_rebuild`]
    /// drops them with the table. A review found this method keeping them,
    /// and a pair of them (`pgsz` 64 with `automerge` 0) is the condition
    /// under which it found a genuine out-of-memory error called `Corrupt`
    /// (the module doc's "What is reported as corrupt, and what is not").
    /// So when the index holds any option this build did not set, this
    /// method rebuilds the table exactly as `full_rebuild` does, in the same
    /// transaction, and reports it as `full_rebuild` would: every target
    /// document upserted, every stored row removed or replaced.
    ///
    /// What it trusts, stated rather than left to be found: the rows the
    /// `documents` table returns, when no such option is set. It does not
    /// check that FTS5's inverted index still agrees with those rows (a
    /// writer that edits the shadow tables directly can leave a row that
    /// reads back exactly right and searches wrongly, which
    /// [`IndexerError::Corrupt`] reports once a search trips over it), nor
    /// the table's own definition (a writer that recreates the table with
    /// another tokenizer and the same rows is not noticed).
    /// [`Indexer::full_rebuild`] replaces both, and is the answer to either.
    ///
    /// # Errors
    ///
    /// [`IndexerError::DuplicatePath`] if `documents` holds one path twice;
    /// [`IndexerError::DocumentTooLarge`] if one of them is larger than the
    /// module doc's per-document cap;
    /// [`IndexerError::Locked`] if another writer holds the lock, or holds
    /// it by the time this call's reads try to become writes;
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if reading the current state or a write
    /// fails for another reason.
    pub fn incremental_sync(
        &mut self,
        documents: &[IndexableDocument],
    ) -> Result<IndexReport, IndexerError> {
        refuse_invalid_target(documents)?;
        let path = self.display_path();
        let tx = self
            .conn
            .transaction()
            .map_err(|source| classify_without_check(&path, "begin transaction", source))?;

        // An option this build never sets is a difference like any other,
        // and only a rebuild removes one: see the doc above.
        if Self::holds_an_option_this_build_did_not_set(&tx, &path)? {
            let report = Self::rebuild_on(&tx, documents, &path)?;
            commit(tx).map_err(|source| classify(&self.conn, &path, "commit", source))?;
            return Ok(report);
        }

        // Read inside the transaction just opened, not before it: see the
        // doc above.
        let target: BTreeMap<&str, (&IndexableDocument, i64)> = documents
            .iter()
            .map(|document| {
                (
                    document.path.as_str(),
                    (document, document.checksum() as i64),
                )
            })
            .collect();
        let stored = Self::stored_rows_on(&tx, &target, &path)?;
        let mut by_path: BTreeMap<&str, Vec<&StoredRow>> = BTreeMap::new();
        let mut unaddressable: Vec<i64> = Vec::new();
        for row in &stored {
            match &row.path {
                Some(stored_path) => by_path.entry(stored_path.as_str()).or_default().push(row),
                None => unaddressable.push(row.rowid),
            }
        }

        let mut removed = 0usize;
        for rowid in unaddressable {
            Self::delete_rowid(&tx, rowid, &path)?;
            removed += 1;
        }
        for (stored_path, rows) in &by_path {
            if !target.contains_key(stored_path) {
                Self::delete_path(&tx, stored_path, &path)?;
                removed += rows.len();
            }
        }

        let mut upserted = 0usize;
        let mut replaced = 0usize;
        for document in documents {
            let at_path = by_path
                .get(document.path.as_str())
                .map_or(&[][..], Vec::as_slice);
            if let [only] = at_path
                && only.exact
            {
                continue;
            }
            Self::delete_path(&tx, &document.path, &path)?;
            replaced += at_path.len();
            Self::insert(&tx, document, &path)?;
            upserted += 1;
        }

        // Read inside the transaction, before commit: see full_rebuild's
        // same fix, above.
        let total = Self::total_indexed_on(&tx, &path)?;
        commit(tx).map_err(|source| classify(&self.conn, &path, "commit", source))?;
        Ok(IndexReport {
            upserted,
            removed,
            replaced,
            total,
        })
    }

    /// Every stored row, as [`StoredRow`]s: the diff basis
    /// [`Indexer::incremental_sync`] uses, read without assuming any
    /// column's type, since a row another writer stored may hold anything,
    /// and each compared against `target` as it is read.
    fn stored_rows_on(
        conn: &Connection,
        target: &BTreeMap<&str, (&IndexableDocument, i64)>,
        display_path: &Path,
    ) -> Result<Vec<StoredRow>, IndexerError> {
        let mut statement = conn
            .prepare("SELECT rowid, path, kind, title, body, checksum FROM documents")
            .map_err(|source| {
                classify(conn, display_path, "prepare a read of stored rows", source)
            })?;
        let rows = statement
            .query_map([], |row| {
                let path = text_of(row.get_ref(1)?);
                let exact = match path.as_deref().and_then(|stored| target.get(stored)) {
                    Some((document, checksum)) => {
                        row.get_ref(2)? == ValueRef::Text(document.kind.as_str().as_bytes())
                            && row.get_ref(3)? == ValueRef::Text(document.title.as_bytes())
                            && row.get_ref(4)? == ValueRef::Text(document.body.as_bytes())
                            && row.get_ref(5)? == ValueRef::Integer(*checksum)
                    }
                    None => false,
                };
                Ok(StoredRow {
                    rowid: row.get(0)?,
                    path,
                    exact,
                })
            })
            .map_err(|source| classify(conn, display_path, "read stored rows", source))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(
                row.map_err(|source| classify(conn, display_path, "read one stored row", source))?,
            );
        }
        Ok(out)
    }

    fn delete_path(
        conn: &Connection,
        target_path: &str,
        display_path: &Path,
    ) -> Result<(), IndexerError> {
        conn.execute(
            "DELETE FROM documents WHERE path = ?1",
            params![target_path],
        )
        .map_err(|source| classify(conn, display_path, "delete one document", source))?;
        Ok(())
    }

    fn delete_rowid(
        conn: &Connection,
        rowid: i64,
        display_path: &Path,
    ) -> Result<(), IndexerError> {
        conn.execute("DELETE FROM documents WHERE rowid = ?1", params![rowid])
            .map_err(|source| {
                classify(
                    conn,
                    display_path,
                    "delete one row with no text path",
                    source,
                )
            })?;
        Ok(())
    }

    fn insert(
        conn: &Connection,
        document: &IndexableDocument,
        display_path: &Path,
    ) -> Result<(), IndexerError> {
        conn.execute(
            "INSERT INTO documents (path, kind, title, body, checksum) VALUES (?1,?2,?3,?4,?5)",
            params![
                document.path,
                document.kind.as_str(),
                document.title,
                document.body,
                document.checksum() as i64,
            ],
        )
        .map_err(|source| classify(conn, display_path, "insert one document", source))?;
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
    /// [`IndexerError::QueryTooLarge`] if `query` is longer than
    /// `MAX_QUERY_BYTES` (the module doc's "Query cost is bounded by
    /// bytes"), checked before FTS5 ever sees it;
    /// [`IndexerError::InvalidQuery`] if `query` holds a `NUL`, which FTS5
    /// cannot parse even quoted (the module doc's "FTS5 query safety"),
    /// checked before FTS5 ever sees it;
    /// [`IndexerError::Locked`] if a concurrent write holds the lock;
    /// [`IndexerError::Corrupt`] if the index is damaged;
    /// [`IndexerError::Sqlite`] if the search fails for another reason.
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchReport, IndexerError> {
        if query.len() > MAX_QUERY_BYTES {
            return Err(IndexerError::QueryTooLarge {
                byte_len: query.len(),
            });
        }
        if query.contains('\0') {
            return Err(IndexerError::InvalidQuery {
                query: query.to_owned(),
            });
        }

        let path = self.display_path();
        let tx = self.conn.unchecked_transaction().map_err(|source| {
            classify_without_check(&path, "begin a read transaction for search", source)
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
            .map_err(|source| classify(&tx, &path, "prepare a search", source))?;
        // `query_map` itself only prepares the row-mapping closure; a
        // locked write, a corrupt page and a tokenizer that cannot be
        // loaded all surface lazily, while the returned iterator is stepped
        // below, not here. Neither failure point is ever the query's fault:
        // the one query text FTS5's parser refuses once quoted, an embedded
        // NUL, was refused above, so every failure goes through
        // `classify` (the module doc's "FTS5 query safety").
        let rows = statement
            .query_map(params![quoted, limit], |row| {
                let path: String = row.get(0)?;
                let kind_text: String = row.get(1)?;
                let title: String = row.get(2)?;
                let score: f64 = row.get(3)?;
                Ok((path, kind_text, title, score))
            })
            .map_err(|source| classify(&tx, &path, "search", source))?;

        let mut hits = Vec::new();
        for row in rows {
            let (path_hit, kind_text, title, score) =
                row.map_err(|source| classify(&tx, &path, "search", source))?;
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
}

impl Indexer<'_> {
    /// Replaces the product's on-disk index with a fresh one rebuilt from
    /// `documents`, moving the old file and its side files aside into
    /// `index/quarantine/`, never deleting them: the only file-level
    /// recovery this module has, and the answer to every
    /// [`IndexerError::Corrupt`] (the module doc's "Recovery").
    ///
    /// `&mut ProductDb` is the whole safety argument. Every on-disk
    /// [`Indexer`] borrows its `ProductDb` for as long as it lives, so while
    /// one is alive this does not compile, which is the compiler proving no
    /// `Indexer` in this process has the file open; the `ProductDb`'s own OS
    /// lock proves no other engine process does.
    ///
    /// A call made while an `Indexer` is still used afterwards is refused
    /// at compile time:
    ///
    /// ```compile_fail,E0502
    /// # use ori_core::types::Timestamp;
    /// # use ori_memory::indexer::Indexer;
    /// # use ori_store::db::ProductDb;
    /// # fn recover_while_searching(db: &mut ProductDb) {
    /// let indexer = Indexer::open(db).expect("open");
    /// let _ = Indexer::recover(db, &[], Timestamp::from_millis(0));
    /// let _ = indexer.search("still in use", 10);
    /// # }
    /// ```
    ///
    /// So is one made while an `Indexer` is merely still in scope, never
    /// used again but not yet dropped (its connection would otherwise still
    /// be open on the file this moves; see the `Drop` impl):
    ///
    /// ```compile_fail,E0502
    /// # use ori_core::types::Timestamp;
    /// # use ori_memory::indexer::Indexer;
    /// # use ori_store::db::ProductDb;
    /// # fn recover_while_in_scope(db: &mut ProductDb) {
    /// let indexer = Indexer::open(db).expect("open");
    /// let _ = indexer.search("last use", 10);
    /// let _ = Indexer::recover(db, &[], Timestamp::from_millis(0));
    /// # }
    /// ```
    ///
    /// And the same calls compile once the `Indexer` is dropped first, which
    /// is what shows the two examples above fail for the borrow and for
    /// nothing else:
    ///
    /// ```no_run
    /// # use ori_core::types::Timestamp;
    /// # use ori_memory::indexer::Indexer;
    /// # use ori_store::db::ProductDb;
    /// # fn recover_after_dropping(db: &mut ProductDb) {
    /// let indexer = Indexer::open(db).expect("open");
    /// let _ = indexer.search("last use", 10);
    /// drop(indexer);
    /// let _ = Indexer::recover(db, &[], Timestamp::from_millis(0));
    /// # }
    /// ```
    ///
    /// The quarantine directory is named `<n, 20 digits>-<recovered_at in
    /// milliseconds, 20 digits>`. `n` counts this product's recoveries, one
    /// more than the highest already in `index/quarantine/`, so the names
    /// sort in the order recoveries happened even when the caller's clock
    /// stepped back between two of them, and no name is ever reused;
    /// `recovered_at` records when, and this module reads no clock itself,
    /// for the reason `ProductDb::open`'s `opened_at` gives. `documents` is the
    /// target set the fresh index is rebuilt from, normally
    /// [`Indexer::collect_from_repo`]'s. The damaged file is never opened,
    /// so no damage can make this fail. An `index/fts.sqlite` (or side
    /// file) that is a symbolic link or a file with a second name, which
    /// [`Indexer::open`] refuses, is moved like any other file: the link, or
    /// this product's name for the file, goes into quarantine, and whatever
    /// it pointed at, or the file's other name, is never touched. An entry
    /// at one of those names that is neither a file nor a link, such as a
    /// directory, is refused before anything moves: it may be another
    /// product's. The files move as one unit:
    /// if one cannot be moved, every one already moved is moved back and
    /// nothing is rebuilt. If the rebuild itself fails after the move, the
    /// old files are already safe in quarantine and the product has a
    /// fresh, possibly partial index; calling this again, or
    /// [`Indexer::full_rebuild`], completes it.
    ///
    /// # Errors
    ///
    /// [`IndexerError::DuplicatePath`] or [`IndexerError::DocumentTooLarge`]
    /// if `documents` holds one path twice or a document over the module
    /// doc's per-document cap, checked before anything is moved;
    /// [`IndexerError::RecoveryRefused`] if `db`'s directory is a relative
    /// path, `index/` or `index/quarantine/` is a symbolic link, or an entry
    /// at an index file's name is neither a regular file nor a symbolic
    /// link;
    /// [`IndexerError::QuarantineIncomplete`] if a file cannot be moved,
    /// naming every file that could not be put back;
    /// [`IndexerError::Directory`] or [`IndexerError::Io`] if a directory
    /// cannot be created or read;
    /// any error [`Indexer::open`] or [`Indexer::full_rebuild`] returns,
    /// from creating and rebuilding the fresh index.
    pub fn recover(
        db: &mut ProductDb,
        documents: &[IndexableDocument],
        recovered_at: Timestamp,
    ) -> Result<RecoveryReport, IndexerError> {
        refuse_invalid_target(documents)?;
        let db: &ProductDb = db;
        let index_dir = owned_index_dir(db, recovery_refused)?;

        let (quarantine, quarantined_files) = quarantine_index_files(&index_dir, recovered_at)?;

        // The fresh index: created, rebuilt and closed before this returns,
        // so nothing this call opened is still open when the caller's own
        // Indexers come back.
        let rebuilt = {
            let mut fresh = Indexer::open(db)?;
            fresh.full_rebuild(documents)?
        };
        Ok(RecoveryReport {
            quarantine,
            quarantined_files,
            rebuilt,
        })
    }

    /// Walks `repo_root` for the canonical documents this indexer is
    /// permitted to hold, and returns them ready for [`Indexer::full_rebuild`]
    /// or [`Indexer::incremental_sync`]: [`Indexer::walk_repo`]'s documents,
    /// without its list of what was skipped and why. Call `walk_repo` to see
    /// that list.
    ///
    /// A walk its own budget cut short is refused, never returned: a file
    /// left out because the walk ran out of documents or bytes before
    /// reaching it may hold documents the index already has, and this set
    /// is what a sync removes documents by. A review found round 7
    /// returning such a set here with the skip list dropped, so one added
    /// file that spent the budget silently removed the rest of the index
    /// while the sync returned `Ok`. Every other entry the walk leaves out is
    /// left out for something in that entry alone (a link, a file too large,
    /// a repeated path, a file that splits into too many documents), and
    /// removes only that entry's own documents.
    ///
    /// # Errors
    ///
    /// As [`Indexer::walk_repo`], and [`IndexerError::WalkBudgetExhausted`]
    /// if the walk left any file out for want of budget.
    pub fn collect_from_repo(repo_root: &Path) -> Result<Vec<IndexableDocument>, IndexerError> {
        let walk = Self::walk_repo(repo_root)?;
        let left_out: Vec<SkippedEntry> = walk
            .skipped
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry.reason,
                    SkipReason::WalkDocumentLimit { .. } | SkipReason::WalkByteLimit { .. }
                )
            })
            .collect();
        if !left_out.is_empty() {
            return Err(IndexerError::WalkBudgetExhausted { left_out });
        }
        Ok(walk.documents)
    }

    /// Walks `repo_root/spec` for the canonical documents this indexer is
    /// permitted to hold, and reports every entry it left out, with the
    /// reason.
    ///
    /// Only `spec/adr/*.md` (kind [`DocumentKind::Adr`], one document per
    /// file), `spec/criteria/*.md` (kind [`DocumentKind::Criterion`], one
    /// document per table row whose first cell matches
    /// `ORI-[A-Z0-9]+-[0-9]+`, per `spec/criteria/phase-1.md`'s own format
    /// line, "Identifier `ORI-P1-nnn`", with every other line of the file
    /// split into sections as below), and every other `spec/**/*.md` (kind
    /// [`DocumentKind::Section`], one document for any text above the first
    /// ATX heading and one per heading, flat rather than level-aware: this
    /// module's own splitter, not `ori-gates::spec_refs::heading_slug`'s,
    /// since `ori-memory` does not and must not depend on `ori-gates`, a
    /// sideways crate under `spec/LLD.md` section 2's dependency direction)
    /// are read, `spec/design/*.md` included (see the module doc). Symbolic
    /// links, non-regular files, files not named `.md`, non-UTF-8 paths,
    /// unreadable entries, repeated document paths, files over 1 MiB,
    /// documents over 64 KiB, files of more than 1,024 documents and files
    /// past the walk's own budget (16,384 documents and 16 MiB of text in
    /// all) are left out and listed in [`RepoWalk::skipped`], with the
    /// reason; see the module doc's "What the repository walk never reads"
    /// and "What one walk costs". The walk's stack use does not depend on
    /// the tree's depth. Entries are visited in name order, so
    /// the result does not depend on the filesystem's own directory order.
    ///
    /// A walk that returns `Ok` walked a real `spec/` directory: when there
    /// is none to walk it is refused, never reported as an empty
    /// repository, because an empty walk handed to
    /// [`Indexer::incremental_sync`] removes every document and returns
    /// `Ok` (the module doc's "The vacuity trap").
    ///
    /// # Errors
    ///
    /// [`IndexerError::NoSpecDirectory`] if `repo_root` does not exist, or
    /// `repo_root/spec` does not exist, is a symbolic link (never
    /// followed), or is not a directory; [`IndexerError::Io`] if
    /// `repo_root/spec` cannot be inspected or listed. One unreadable file
    /// or subdirectory below it never fails the walk.
    pub fn walk_repo(repo_root: &Path) -> Result<RepoWalk, IndexerError> {
        let spec_dir = repo_root.join("spec");
        let mut walk = RepoWalk::default();
        match std::fs::symlink_metadata(&spec_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(if std::fs::symlink_metadata(repo_root).is_err() {
                    IndexerError::NoSpecDirectory {
                        path: repo_root.to_owned(),
                        reason: "the repository root does not exist",
                    }
                } else {
                    IndexerError::NoSpecDirectory {
                        path: spec_dir,
                        reason: "it does not exist",
                    }
                });
            }
            Err(source) => {
                return Err(IndexerError::Io {
                    path: spec_dir,
                    source,
                });
            }
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(IndexerError::NoSpecDirectory {
                    path: spec_dir,
                    reason: "it is a symbolic link, which the walk never follows",
                });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(IndexerError::NoSpecDirectory {
                    path: spec_dir,
                    reason: "it is not a directory",
                });
            }
            Ok(_) => {}
        }
        walk_markdown(repo_root, &spec_dir, &mut walk).map_err(|source| IndexerError::Io {
            path: spec_dir.clone(),
            source,
        })?;
        Ok(walk)
    }
}

/// One row as [`Indexer::incremental_sync`] reads it: its `rowid`, its
/// `path` if that is UTF-8 text (`None` for a value of any other type), and
/// whether every column is exactly what this build writes for the target
/// document at that path.
struct StoredRow {
    rowid: i64,
    path: Option<String>,
    exact: bool,
}

/// `value` as a `String` if it is UTF-8 text, else `None`.
fn text_of(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Text(bytes) => std::str::from_utf8(bytes).ok().map(str::to_owned),
        _ => None,
    }
}

/// Why a symbolic link is refused, wherever one is: the file behind it may
/// be another product's, which no borrow of this product's `ProductDb`
/// says anything about (the module doc's "Recovery").
const LINK_REFUSAL: &str = "it is a symbolic link, so the file behind it may be another product's";

/// [`IndexerError::OpenRefused`], as a constructor [`owned_index_dir`] can
/// be handed.
fn open_refused(path: PathBuf, reason: &'static str) -> IndexerError {
    IndexerError::OpenRefused { path, reason }
}

/// [`IndexerError::RecoveryRefused`], as a constructor [`owned_index_dir`]
/// can be handed.
fn recovery_refused(path: PathBuf, reason: &'static str) -> IndexerError {
    IndexerError::RecoveryRefused { path, reason }
}

/// `<db's directory>/index/`, resolved the one way that keeps every
/// [`Indexer`] and [`Indexer::recover`] on this product's own files, and
/// created if absent: `db`'s directory must be absolute (a relative one is
/// resolved against the working directory at the time of the call, which
/// after a change of directory is another directory than the one
/// `ProductDb::open` locked); it is then canonicalized, which resolves any
/// link at or above it, the same way `ProductDb::open` reached its lock
/// file through it; and `index/` itself must be a real directory, never a
/// link. Every refusal is built with `refuse`, so `open` and `recover` each
/// name their own.
fn owned_index_dir(
    db: &ProductDb,
    refuse: fn(PathBuf, &'static str) -> IndexerError,
) -> Result<PathBuf, IndexerError> {
    let product_dir = db.dir();
    if !product_dir.is_absolute() {
        return Err(refuse(
            product_dir.to_owned(),
            "the product directory is a relative path, which resolves against the working \
             directory of the moment, not the one the product's lock was taken in",
        ));
    }
    let product_dir = product_dir
        .canonicalize()
        .map_err(|source| IndexerError::Directory {
            path: product_dir.to_owned(),
            source,
        })?;
    let index_dir = product_dir.join(INDEX_DIR);
    let mut created = false;
    loop {
        match std::fs::symlink_metadata(&index_dir) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(refuse(index_dir, LINK_REFUSAL));
            }
            Ok(metadata) if metadata.is_dir() => return Ok(index_dir),
            Ok(_) => return Err(refuse(index_dir, "it is not a directory")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !created => {
                match std::fs::create_dir(&index_dir) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(source) => {
                        return Err(IndexerError::Directory {
                            path: index_dir,
                            source,
                        });
                    }
                }
                created = true;
            }
            Err(source) => {
                return Err(IndexerError::Directory {
                    path: index_dir,
                    source,
                });
            }
        }
    }
}

/// Why a file with another name is refused: the other name may be another
/// product's (the module doc's "Recovery").
const HARD_LINK_REFUSAL: &str =
    "it has another name (a hard link), so the file may be another product's";

/// Refuses to open the index if any file of [`INDEX_FILE_SET`] in
/// `index_dir` is a symbolic link, not a regular file, or (on Unix) a file
/// with more than one name: [`Indexer::open`] opens only files under its
/// own product's directory, and a hard link is the same file under another
/// directory too. An absent file is fine; SQLite creates it.
///
/// A review found round 6 refusing only symbolic links: with this
/// product's `fts.sqlite` or `-wal` a hard link to another product's, the
/// open was accepted, the two products read and erased each other's
/// acknowledged writes, and `recover` on the other product compiled and
/// ran while this product's `Indexer` still held the file it moved. The
/// link count is read from the same `symlink_metadata` call, so it
/// describes the entry this function decided on; `SQLITE_OPEN_NOFOLLOW`
/// says nothing about hard links.
fn refuse_unowned_index_files(index_dir: &Path) -> Result<(), IndexerError> {
    for name in INDEX_FILE_SET {
        let live = index_dir.join(name);
        match std::fs::symlink_metadata(&live) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(open_refused(live, LINK_REFUSAL));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(open_refused(live, "it is not a regular file"));
            }
            Ok(metadata) if link_count(&metadata).is_some_and(|links| links > 1) => {
                return Err(open_refused(live, HARD_LINK_REFUSAL));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(IndexerError::Io { path: live, source }),
        }
    }
    Ok(())
}

/// How many names (hard links) the file `metadata` describes has, where
/// the standard library can say: on Unix, `st_nlink`.
#[cfg(unix)]
fn link_count(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.nlink())
}

/// Elsewhere, unknown: on Windows the standard library exposes a file's
/// link count only on a nightly toolchain, and this crate takes no platform
/// dependency to read it (the module doc's "Recovery" states the gap).
#[cfg(not(unix))]
fn link_count(_metadata: &std::fs::Metadata) -> Option<u64> {
    None
}

/// Why [`Indexer::recover`] refuses an entry at an index file's name that
/// is neither a regular file nor a symbolic link: `quarantine_index_files`
/// has the review that found a directory there moved whole.
const NOT_A_FILE_REFUSAL: &str = "it is neither a regular file nor a symbolic link, so it may \
                                  be another product's directory, which recover never moves; \
                                  move it aside by hand";

/// Refuses `dir` if it is a symbolic link: [`Indexer::recover`] moves files
/// only inside a directory the product owns outright (the module doc's
/// "Recovery").
fn refuse_symlink(dir: &Path) -> Result<(), IndexerError> {
    let metadata = std::fs::symlink_metadata(dir).map_err(|source| IndexerError::Io {
        path: dir.to_owned(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(recovery_refused(dir.to_owned(), LINK_REFUSAL));
    }
    Ok(())
}

/// Moves every file of [`INDEX_FILE_SET`] present in `index_dir` into a new,
/// uniquely named directory under `index_dir/quarantine/`, side files
/// first, and checks none of them is left at its live path. Returns the
/// directory (`None`, and nothing created, when no file was present) and
/// every moved file's new path. Only [`Indexer::recover`] calls this, under
/// the proof its doc states.
///
/// The files move as one unit. A review found a failed move returning at
/// once, leaving the side files already moved in quarantine: the next open
/// then served the database without its write-ahead log, silently, as it
/// stood before the log's transactions, and a retry quarantined the
/// database in a second directory, apart from its log. Now a failed move
/// moves every file already moved back to its live path, last moved first;
/// [`IndexerError::QuarantineIncomplete`] names the directory and any file
/// that could not be put back. The directory itself is left in place, even
/// when that leaves it empty: its count then stays taken, so
/// [`unique_quarantine_dir`] never makes its name again, and the path the
/// error names is never overwritten with, or merged into by, a later
/// recovery's evidence. A review found round 8 removing it, and the next
/// recovery at the same `recovered_at` making the same name.
///
/// It moves files and links, and nothing else. A review found round 7
/// deciding only that an entry existed and then renaming it whatever it
/// was: with another product's directory at `index/fts.sqlite-wal` (a
/// product id or products root that names that path, which
/// `ProductDb::open` does not check, or a link above the product directory
/// that resolves there), `recover` moved that product's whole live
/// directory, its lock, its event log and its index, into this product's
/// quarantine, while its `ProductDb` and `Indexer` were open. Their later
/// writes returned `Ok` into the moved copy, its lock stopped excluding a
/// second writer, and its next open found an empty event log. Every entry
/// is now checked before anything is moved, and an entry that is neither
/// a regular file nor a symbolic link is refused
/// ([`IndexerError::RecoveryRefused`]), with nothing moved.
fn quarantine_index_files(
    index_dir: &Path,
    recovered_at: Timestamp,
) -> Result<(Option<PathBuf>, Vec<PathBuf>), IndexerError> {
    let mut present = Vec::new();
    for name in INDEX_FILE_SET {
        let live = index_dir.join(name);
        match std::fs::symlink_metadata(&live) {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
                present.push(name);
            }
            Ok(_) => return Err(recovery_refused(live, NOT_A_FILE_REFUSAL)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(IndexerError::Io { path: live, source }),
        }
    }
    if present.is_empty() {
        return Ok((None, Vec::new()));
    }

    let quarantine_root = index_dir.join(QUARANTINE_DIR);
    std::fs::create_dir_all(&quarantine_root).map_err(|source| IndexerError::Directory {
        path: quarantine_root.clone(),
        source,
    })?;
    refuse_symlink(&quarantine_root)?;
    let destination = unique_quarantine_dir(&quarantine_root, recovered_at)?;

    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in present {
        let from = index_dir.join(name);
        let to = destination.join(name);
        if let Err(source) = move_file(&from, &to) {
            let mut stranded = Vec::new();
            for (live, quarantined) in moved.iter().rev() {
                if move_file(quarantined, live).is_err() {
                    stranded.push(quarantined.clone());
                }
            }
            return Err(IndexerError::QuarantineIncomplete {
                path: from,
                source,
                quarantine: destination,
                stranded,
            });
        }
        moved.push((from, to));
    }
    let moved: Vec<PathBuf> = moved.into_iter().map(|(_, to)| to).collect();
    for name in INDEX_FILE_SET {
        let live = index_dir.join(name);
        if std::fs::symlink_metadata(&live).is_ok() {
            return Err(IndexerError::Io {
                path: live,
                source: std::io::Error::other(
                    "still present after being moved into quarantine; refusing to create a \
                     fresh index beside it",
                ),
            });
        }
    }
    Ok((Some(destination), moved))
}

/// Creates and returns `quarantine_root/<n, 20 digits>-<millis, 20
/// digits>`, where `n` is one more than the highest `n` of any entry
/// already in `quarantine_root` with a name of that shape (1 for the
/// first), and `millis` is `recovered_at`: `create_dir`, not
/// `create_dir_all`, so a name already taken is never reused, whatever
/// made it. This module never removes a directory this made, a rolled
/// back recovery's included ([`quarantine_index_files`]), so a count once
/// taken stays taken and no name is ever made twice. A timestamp before
/// the Unix epoch is written as zero rather than with a sign.
///
/// The count comes first so the names sort in the order recoveries
/// happened. A review found round 7 naming the directory
/// `<millis>-<sequence>`, from `recovered_at` alone: `recovered_at` is the
/// caller's wall clock (this module reads none), which can step back, and
/// a recovery handed an earlier time than the last one sorted before it,
/// so the last directory in name order held older evidence than the
/// newest. Only [`Indexer::recover`] calls this, holding the product
/// exclusively, so no second recovery of this product reads the same
/// highest `n`.
fn unique_quarantine_dir(
    quarantine_root: &Path,
    recovered_at: Timestamp,
) -> Result<PathBuf, IndexerError> {
    let millis = u64::try_from(recovered_at.millis()).unwrap_or(0);
    let directory_error = |source: std::io::Error| IndexerError::Directory {
        path: quarantine_root.to_owned(),
        source,
    };
    let mut ordinal = 0u64;
    for entry in std::fs::read_dir(quarantine_root).map_err(directory_error)? {
        let entry = entry.map_err(directory_error)?;
        if let Some(taken) = entry.file_name().to_str().and_then(quarantine_ordinal) {
            ordinal = ordinal.max(taken);
        }
    }
    // Nothing else creates names of this shape while the product is held
    // exclusively, so the first candidate is free; the bound only keeps a
    // filesystem that reports every name taken from looping for ever.
    for _ in 0..1000 {
        ordinal = ordinal.checked_add(1).ok_or_else(|| {
            directory_error(std::io::Error::other(
                "a quarantine directory already carries the highest recovery count there is",
            ))
        })?;
        let candidate = quarantine_root.join(format!("{ordinal:020}-{millis:020}"));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(IndexerError::Directory {
                    path: candidate,
                    source,
                });
            }
        }
    }
    Err(directory_error(std::io::Error::other(
        "a thousand quarantine directory names in a row were already taken",
    )))
}

/// The recovery count `n` a quarantine directory name of
/// [`unique_quarantine_dir`]'s shape, `<n, 20 digits>-<millis, 20
/// digits>`, carries; `None` for any other name. A count too large for a
/// `u64` reads as `u64::MAX`, so no later name can be made to sort before
/// it.
fn quarantine_ordinal(name: &str) -> Option<u64> {
    let (ordinal, millis) = name.split_once('-')?;
    let digits = |part: &str| part.len() == 20 && part.bytes().all(|byte| byte.is_ascii_digit());
    (digits(ordinal) && digits(millis)).then(|| ordinal.parse().unwrap_or(u64::MAX))
}

/// `std::fs::rename(from, to)`, for every move [`quarantine_index_files`]
/// makes, forward and back. In test builds only, it first asks the test
/// running on this thread whether this move must fail, which is how that
/// function's rollback is tested on every platform, including a move back
/// that fails too; `chflags uchg`, the real refusal those tests also use,
/// exists only on macOS and the BSDs. Compiled out of every other build.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if let Some(error) = tests::injected_move_failure(from) {
        return Err(error);
    }
    std::fs::rename(from, to)
}

/// Walks `spec_dir` for `.md` files, depth first, entries in name order,
/// entering every directory under it, `spec/design/` included (the module
/// doc's "`spec/design/` is walked like the rest of `spec/`"), appending
/// every document found to
/// `walk.documents` (and every entry left out to `walk.skipped`), with
/// `path` computed relative to `repo_root`, never to the directory being
/// listed. Returns an error only when `spec_dir` itself cannot be listed,
/// which [`Indexer::walk_repo`] turns into [`IndexerError::Io`]; any
/// directory below it that cannot be listed is recorded as
/// [`SkipReason::Unreadable`].
///
/// It is a loop over an explicit stack of directory listings, not a
/// recursion: a review found round 6's recursive version, one call frame
/// per directory level, aborting the whole process with a stack overflow
/// on a tree whose depth the path limit allows (250 one-letter levels
/// overflowed a 2 MiB thread in an unoptimized build, 442 levels a
/// 1,600 KiB thread in an optimized one), where it should have walked or
/// recorded it. Its stack use no longer depends on the tree's depth; the
/// listings it holds are on the heap, one per directory level open at the
/// time, and hold no directory open ([`entries_in_reverse_name_order`]).
/// The order is the recursion's own: a directory's entries are
/// visited as soon as the directory itself is reached.
///
/// An adversarial review found an earlier version stripped each file's
/// path relative to `dir.parent().parent()`, where `dir` is whichever
/// directory the recursion was currently walking, not `repo_root`. That kept
/// only the file's last three path components, so a top-level `spec/*.md`
/// file carried the checkout directory's own name as a prefix (identical
/// content in two checkouts of the same repository, or in a worktree versus
/// the main checkout, got two different identities), and a file three or
/// more levels under `spec/` lost its leading `spec/` components entirely,
/// so two unrelated files could collide on one path. Stripping every path
/// against `repo_root`, once, fixes this: `relative` is always exactly the
/// path under `repo_root`.
///
/// A second review found two narrower defects this rewrite did not reach.
/// First, `is_adr`/`is_criteria` (below) still read `relative_path`'s
/// components for `adr` or `criteria` *anywhere*, not only in the second
/// position `walk_repo`'s own doc names (`spec/adr/*.md`,
/// `spec/criteria/*.md`): an ordinary prose document nested under a
/// `criteria`-named directory that is not `spec/criteria/` was silently
/// parsed as a criteria table and dropped out of the index entirely when
/// it matched no `ORI-` row (the test fixture below spells the exact
/// path). Classification now
/// checks the exact shape, `["spec", "adr", filename]` or `["spec",
/// "criteria", filename]`, three components with `adr`/`criteria` in the
/// second one, not a components-contains check. Second, the `spec/design`
/// skip of that time matched any directory named `design` at any depth;
/// the skip has since been removed altogether, so no directory is skipped
/// by name at all.
///
/// A third review found the walk following symbolic links (`is_dir` and
/// `read_to_string` both follow them), rewriting `\` to `/` on every
/// platform, and failing the whole walk on one unreadable file. Every
/// entry is now decided by `DirEntry::file_type`, which never follows a
/// link; the path is the components joined with `/`; and anything left out
/// is recorded with a reason: the module doc's "What the repository walk
/// never reads".
fn walk_markdown(repo_root: &Path, spec_dir: &Path, walk: &mut RepoWalk) -> std::io::Result<()> {
    let mut budget = WalkBudget::new();
    let mut listings: Vec<Vec<ListedEntry>> = vec![entries_in_reverse_name_order(spec_dir)?];
    while let Some(listing) = listings.last_mut() {
        let Some((path, file_type)) = listing.pop() else {
            listings.pop();
            continue;
        };
        let file_type = match file_type {
            Ok(file_type) => file_type,
            Err(error) => {
                walk.skipped.push(SkippedEntry {
                    path,
                    reason: SkipReason::Unreadable {
                        error: error.to_string(),
                    },
                });
                continue;
            }
        };
        if file_type.is_symlink() {
            walk.skipped.push(SkippedEntry {
                path,
                reason: SkipReason::Symlink,
            });
            continue;
        }
        if file_type.is_dir() {
            match entries_in_reverse_name_order(&path) {
                Ok(entries) => listings.push(entries),
                Err(error) => walk.skipped.push(SkippedEntry {
                    path,
                    reason: SkipReason::Unreadable {
                        error: error.to_string(),
                    },
                }),
            }
            continue;
        }
        if !file_type.is_file() {
            walk.skipped.push(SkippedEntry {
                path,
                reason: SkipReason::NotARegularFile,
            });
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            walk.skipped.push(SkippedEntry {
                path,
                reason: SkipReason::NotMarkdown,
            });
            continue;
        }
        collect_file(repo_root, path, walk, &mut budget);
    }
    Ok(())
}

/// One entry of a directory listing: its path, and what kind of entry it
/// is (never following a link), as read when the directory was listed.
type ListedEntry = (PathBuf, std::io::Result<std::fs::FileType>);

/// Every entry of the directory `dir`, sorted by name, last name first, so
/// that popping from the end visits them in name order. Fails if the
/// directory cannot be listed, or any entry of it cannot be read.
///
/// It keeps each entry's path and type, never the `DirEntry` itself: on
/// Unix a `DirEntry` holds its directory's open handle, so listings kept
/// for every level of a deep tree would hold one file descriptor per
/// level, and past the process's limit a directory would be recorded as
/// unreadable for no reason of its own. Here the handle is closed before
/// this returns, so a walk holds one open directory at a time.
fn entries_in_reverse_name_order(dir: &Path) -> std::io::Result<Vec<ListedEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        entries.push((entry.file_name(), entry.path(), entry.file_type()));
    }
    entries.sort_by(|(left, ..), (right, ..)| right.cmp(left));
    Ok(entries
        .into_iter()
        .map(|(_, path, file_type)| (path, file_type))
        .collect())
}

/// What one walk has left to spend, and the document paths it has already
/// given out: the module doc's "What one walk costs".
struct WalkBudget {
    /// Every document path already taken, so a repeat is left out.
    seen: BTreeSet<String>,
    /// Documents still to split, of [`MAX_WALK_DOCUMENTS`].
    documents: usize,
    /// Bytes of file text still to read into documents, of
    /// [`MAX_WALK_BYTES`].
    bytes: u64,
}

impl WalkBudget {
    fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
            documents: MAX_WALK_DOCUMENTS,
            bytes: MAX_WALK_BYTES,
        }
    }
}

/// Reads one regular `.md` file the walk reached and appends its documents
/// to `walk`, or records why it was left out: [`walk_markdown`]'s per-file
/// half. A file that splits into more than [`MAX_FILE_DOCUMENTS`] is left
/// out whole, its split stopped as soon as it passes that, so no one file
/// costs more than its share; a file whose text would take the walk past
/// what `budget` has left of [`MAX_WALK_BYTES`], or whose documents would
/// take it past what is left of [`MAX_WALK_DOCUMENTS`], is left out whole
/// too, recorded as the walk's budget running out, which
/// [`Indexer::collect_from_repo`] refuses.
fn collect_file(repo_root: &Path, path: PathBuf, walk: &mut RepoWalk, budget: &mut WalkBudget) {
    // Never defaulted to the absolute path: an identity that silently
    // became absolute the one time stripping failed would reintroduce
    // exactly the checkout-dependent-identity defect `walk_markdown`'s doc
    // describes. Unreachable in practice (every path here was built by
    // joining entries onto a directory under `repo_root`), and recorded
    // rather than failing the walk if it ever is reached.
    let Ok(relative_path) = path.strip_prefix(repo_root) else {
        walk.skipped.push(SkippedEntry {
            reason: SkipReason::Unreadable {
                error: format!(
                    "not under repo_root {}; refusing to synthesize an absolute-path identity",
                    repo_root.display()
                ),
            },
            path,
        });
        return;
    };
    let Some(components) = relative_path
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<&str>>>()
    else {
        walk.skipped.push(SkippedEntry {
            path,
            reason: SkipReason::NonUtf8Path,
        });
        return;
    };
    // `Path::components` splits on `\` only where the platform treats it as
    // a separator, so this join is the whole normalization: on Unix a `\`
    // in a file name stays part of that name. See the module doc's
    // "Document identity".
    let relative = components.join("/");
    // A file that cannot fit what is left of the budget is left out before
    // it is read, so a spent budget costs the rest of the walk no reading;
    // its length is checked again once read, in case it grew.
    if budget.documents == 0 {
        walk.skipped.push(SkippedEntry {
            path,
            reason: SkipReason::WalkDocumentLimit { remaining: 0 },
        });
        return;
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&path)
        && metadata.len() > budget.bytes
        && metadata.len() <= MAX_FILE_BYTES
    {
        walk.skipped.push(SkippedEntry {
            path,
            reason: SkipReason::WalkByteLimit {
                byte_len: metadata.len(),
                remaining: budget.bytes,
            },
        });
        return;
    }
    let text = match read_capped(&path) {
        Ok(Ok(text)) => text,
        Ok(Err(byte_len)) => {
            walk.skipped.push(SkippedEntry {
                path,
                reason: SkipReason::FileTooLarge { byte_len },
            });
            return;
        }
        Err(error) => {
            walk.skipped.push(SkippedEntry {
                path,
                reason: SkipReason::Unreadable {
                    error: error.to_string(),
                },
            });
            return;
        }
    };
    let byte_len = u64::try_from(text.len()).unwrap_or(u64::MAX);
    if byte_len > budget.bytes {
        walk.skipped.push(SkippedEntry {
            path,
            reason: SkipReason::WalkByteLimit {
                byte_len,
                remaining: budget.bytes,
            },
        });
        return;
    }
    let is_adr = components.len() == 3 && components[0] == "spec" && components[1] == "adr";
    let is_criteria =
        components.len() == 3 && components[0] == "spec" && components[1] == "criteria";
    // Every file splits under the same cap, whatever the walk has left, so
    // a file past it is recorded for what it is, and a file past only what
    // the walk has left is recorded as the walk running out.
    let limit = MAX_FILE_DOCUMENTS;
    let documents = if is_adr {
        let title = first_heading(&text).unwrap_or_else(|| relative.clone());
        Some(vec![IndexableDocument::new(
            relative,
            DocumentKind::Adr,
            title,
            text,
        )])
    } else if is_criteria {
        criteria_documents(&relative, &text, limit).and_then(|(mut rows, rest)| {
            // A criteria file with no criterion row in it is split exactly
            // as the same file is anywhere else, so it is never left with
            // no document: a review found an empty or blank-only file here
            // producing none and no record, while the same file under
            // spec/runbooks/ was one document at its bare path.
            if rows.is_empty() {
                return section_documents(&relative, &text, limit);
            }
            if !rest.trim().is_empty() {
                rows.extend(section_documents(&relative, &rest, limit - rows.len())?);
            }
            Some(rows)
        })
    } else {
        section_documents(&relative, &text, limit)
    };
    let Some(documents) = documents else {
        walk.skipped.push(SkippedEntry {
            path,
            reason: SkipReason::FileDocumentLimit { limit },
        });
        return;
    };
    if documents.len() > budget.documents {
        walk.skipped.push(SkippedEntry {
            path,
            reason: SkipReason::WalkDocumentLimit {
                remaining: budget.documents,
            },
        });
        return;
    }
    // Every document split is charged, kept or left out below, for the
    // reason MAX_WALK_DOCUMENTS gives.
    budget.bytes -= byte_len;
    budget.documents -= documents.len();
    let seen = &mut budget.seen;
    for document in documents {
        let byte_len = document.indexed_byte_len();
        if byte_len > MAX_DOCUMENT_BYTES {
            walk.skipped.push(SkippedEntry {
                path: path.clone(),
                reason: SkipReason::DocumentTooLarge {
                    path: document.path,
                    byte_len,
                },
            });
        } else if seen.insert(document.path.clone()) {
            walk.documents.push(document);
        } else {
            walk.skipped.push(SkippedEntry {
                path: path.clone(),
                reason: SkipReason::DuplicateDocumentPath {
                    path: document.path,
                },
            });
        }
    }
}

/// Reads `path` as UTF-8 text, never more than [`MAX_FILE_BYTES`] of it:
/// `Ok(Err(byte_len))` for a file longer than that (its length as the
/// filesystem reports it, or as far as this read got, whichever is more),
/// so a file that grows between the size check and the read is still
/// bounded.
///
/// A leading byte-order mark (U+FEFF, the bytes `EF BB BF` many Windows
/// editors save) is dropped: it says how the file is encoded, and is no
/// part of its text. A review found it kept, so the file's first line,
/// `\u{FEFF}# Title`, was not read as a heading: the first section's
/// identity moved from `path#anchor` to the bare path, its title became
/// the path, and an ADR's title became its first `##` heading, from a
/// change of encoding signature alone. A U+FEFF anywhere else is text and
/// is kept.
///
/// Line endings are folded to `\n` ([`lf_line_endings`]), for the same
/// reason: a checkout's line endings are not the file's content. A review
/// found round 7 keeping `\r\n` in the documents that hold a file's text
/// whole (a file with no heading, an ADR) while every split section was
/// rebuilt line by line with `\n`: the same content checked out with
/// `\r\n` and with `\n` disagreed on 8 of this repository's 213 documents,
/// and adding a first heading to a role file on a `\r\n` checkout rewrote
/// the text above it, unchanged, from `\r\n` to `\n`, which the freshness
/// tracker then reported as changed since its verification. This
/// repository pins `\n` in `.gitattributes`, which says the parser folds
/// `\r\n` itself; the product repositories this engine indexes need not.
fn read_capped(path: &Path) -> std::io::Result<Result<String, u64>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let reported = file.metadata()?.len();
    if reported > MAX_FILE_BYTES {
        return Ok(Err(reported));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_FILE_BYTES {
        return Ok(Err(read.max(reported)));
    }
    let mut text = String::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if text.starts_with(BYTE_ORDER_MARK) {
        text.drain(..BYTE_ORDER_MARK.len_utf8());
    }
    Ok(Ok(lf_line_endings(text)))
}

/// `text` with every carriage return that ends a line, directly before its
/// `\n`, dropped, so `\r\n` becomes `\n` (and so does the rarer `\r\r\n`);
/// a carriage return anywhere else is text and is kept. What is left holds
/// no `\r\n` at all, so `str::lines`, which strips a `\r` only before a
/// `\n`, removes nothing from it: a document holding a file's text whole
/// and one rebuilt from its lines agree byte for byte.
fn lf_line_endings(text: String) -> String {
    if !text.contains("\r\n") {
        return text;
    }
    let mut folded = String::with_capacity(text.len());
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        if lines.peek().is_some() {
            folded.push_str(line.trim_end_matches('\r'));
            folded.push('\n');
        } else {
            folded.push_str(line);
        }
    }
    folded
}

/// U+FEFF, which at the very start of a file is a byte-order mark:
/// [`read_capped`] drops it there.
const BYTE_ORDER_MARK: char = '\u{FEFF}';

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
/// [`Indexer::walk_repo`]'s doc for why this does not attempt `ori-gates`'s
/// level-aware split), plus one for any non-blank text above the first
/// heading, at `relative_path` itself with no anchor, titled by the path. A
/// file with no heading at all becomes that one document, holding the whole
/// text. No non-blank line is ever dropped: the module doc's "Text before
/// the first heading is a document too" records the review that found
/// every line above a first heading discarded. `None`, with the split
/// stopped where it passed, if `text` holds more than `limit` documents:
/// the walk passes `MAX_FILE_DOCUMENTS`, "What one walk costs" in the
/// module doc.
///
/// Two defects an earlier adversarial review found are fixed here too.
/// First, a line starting with `#` *inside a fenced code block* (three or
/// more backticks) is not a heading: a `# install` comment inside a shell
/// snippet no longer splits the document or contributes an anchor. Second,
/// two headings whose text produces the same anchor (an exact repeat, such
/// as two `## Steps` in one file) are disambiguated deterministically,
/// GitHub's own actual convention: keep incrementing a `-N` suffix until an
/// anchor no earlier heading in this file has already claimed. A second
/// review found this module's first attempt at that convention checked only
/// a per-base counter, not the set of anchors already assigned, so a
/// heading whose own text already ends in a numeral collided with the
/// suffix a repeat produced (`Phase 1`, `Phase 1.1`, `Phase 1` gave
/// `phase-1`, `phase-1-1`, `phase-1-1`, a second collision from the very fix
/// meant to remove the first one). `used_anchors` below tracks the whole
/// set, so the third heading here is tried against `phase-1` (taken), then
/// `phase-1-1` (also taken), then `phase-1-2` (free), matching what GitHub's
/// own slugger does. Without either fix, two documents could carry the same
/// `path`, which [`Indexer::walk_repo`] would then have to leave out; see
/// `tests::ori_t_0035_collect_from_repo_and_full_rebuild_succeed_on_this_repositorys_own_spec_tree`
/// for the check that this repository's own `spec/` never hits one.
///
/// A later review found that search quadratic: every repeat of a heading
/// restarted it at `-1`, so the `n`th repeat tried `n` anchors, and 32,768
/// identical headings (a 128 KB file) took 25.6 seconds. `next_suffix`
/// below keeps, for each base anchor, the first suffix not yet tried, and
/// the search resumes there. It skips nothing it should not: every suffix
/// below it was found taken, and an anchor, once taken, stays taken. Each
/// candidate anchor is tried at most once, so a file splits in time
/// linear in its size
/// (`tests::ori_t_0035_a_file_of_one_heading_repeated_to_the_file_cap_splits_within_a_stated_bound`).
fn section_documents(
    relative_path: &str,
    text: &str,
    limit: usize,
) -> Option<Vec<IndexableDocument>> {
    let mut preamble = String::new();
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_body = String::new();
    let mut in_fence = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence && trimmed.starts_with('#') {
            if sections.len() >= limit {
                return None;
            }
            if let Some(title) = current_title.take() {
                sections.push((title, std::mem::take(&mut current_body)));
            }
            current_title = Some(trimmed.trim_start_matches('#').trim().to_owned());
            continue;
        }
        let body = if current_title.is_some() {
            &mut current_body
        } else {
            &mut preamble
        };
        body.push_str(line);
        body.push('\n');
    }
    if let Some(title) = current_title {
        sections.push((title, current_body));
    }

    let preamble_documents = usize::from(sections.is_empty() || !preamble.trim().is_empty());
    if sections.len() + preamble_documents > limit {
        return None;
    }
    if sections.is_empty() {
        return Some(vec![IndexableDocument::new(
            relative_path.to_owned(),
            DocumentKind::Section,
            relative_path.to_owned(),
            text.to_owned(),
        )]);
    }

    let mut out = Vec::with_capacity(sections.len() + 1);
    if !preamble.trim().is_empty() {
        out.push(IndexableDocument::new(
            relative_path.to_owned(),
            DocumentKind::Section,
            relative_path.to_owned(),
            preamble,
        ));
    }
    let mut used_anchors: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut next_suffix: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    out.extend(sections.into_iter().map(|(title, body)| {
        let base = heading_anchor(&title);
        let anchor = if used_anchors.contains(&base) {
            let suffix = next_suffix.entry(base.clone()).or_insert(1);
            loop {
                let candidate = format!("{base}-{suffix}");
                *suffix += 1;
                if !used_anchors.contains(&candidate) {
                    break candidate;
                }
            }
        } else {
            base
        };
        used_anchors.insert(anchor.clone());
        IndexableDocument::new(
            format!("{relative_path}#{anchor}"),
            DocumentKind::Section,
            title,
            body,
        )
    }));
    Some(out)
}

/// The repository file a document path the walk gives belongs to, the
/// `path (under spec/)` of `spec/DATA_MODEL.md` section 2's `Document`: the
/// path itself for a document holding a file's text (a file with no
/// heading, the text above a first heading, an ADR), and the part before
/// the anchor for a section or a criterion, `<file>#<anchor>`. An anchor
/// never holds a `.` or a `/` ([`heading_anchor`] keeps only alphanumerics
/// and `-`, a repeat adds `-<n>`, and a criterion identifier is
/// `ORI-[A-Z0-9]+-[0-9]+`), while a walked file's name always ends in
/// `.md`, so the last `#` of a path starts an anchor exactly when nothing
/// after it holds either, a file whose own name holds a `#` included.
/// `crate::freshness` reads the index's corpus by it.
pub(crate) fn document_file(path: &str) -> &str {
    match path.rsplit_once('#') {
        Some((file, anchor)) if !anchor.contains(['.', '/']) => file,
        _ => path,
    }
}

/// A simple, local heading-to-anchor mapping: lower-cased, non-alphanumeric
/// runs collapsed to one `-`. Deliberately not
/// `ori-gates::spec_refs::heading_slug` (see [`Indexer::walk_repo`]'s
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

/// Splits a criteria file into one [`IndexableDocument`] per criterion,
/// matching `spec/criteria/phase-1.md`'s own format (a markdown table whose
/// first column is the identifier, `ORI-P1-nnn`), and returns every other
/// line, in order, as the second half of the pair, for [`collect_file`] to
/// index as sections like any other specification file. A table row
/// inside a fenced code block is text, not a criterion.
///
/// A review found everything but those rows dropped without a record: a
/// criteria file's title and prose, a proposed criterion appended in the
/// acceptance-criterion template's two-column form (whose first cell is a
/// field name, not an identifier), and a whole criteria file of another
/// shape, such as the personas file the specification schedules for that
/// directory, which produced no document at all while every sync
/// returned `Ok`. None of it is dropped now. `None`, with the split
/// stopped where it passed, if `text` holds more than `limit` criteria
/// rows (the per-file cap, as [`section_documents`] takes it).
fn criteria_documents(
    relative_path: &str,
    text: &str,
    limit: usize,
) -> Option<(Vec<IndexableDocument>, String)> {
    let mut rows = Vec::new();
    let mut rest = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence && let Some(id) = criterion_row_id(trimmed) {
            if rows.len() >= limit {
                return None;
            }
            rows.push(IndexableDocument::new(
                format!("{relative_path}#{id}"),
                DocumentKind::Criterion,
                id.to_owned(),
                trimmed.to_owned(),
            ));
            continue;
        }
        rest.push_str(line);
        rest.push('\n');
    }
    Some((rows, rest))
}

/// The identifier in `row`'s first cell, if `row` is a markdown table row
/// whose first cell is one ([`is_criterion_id`]).
fn criterion_row_id(row: &str) -> Option<&str> {
    if !row.starts_with('|') {
        return None;
    }
    let id = row.trim_matches('|').split('|').next()?.trim();
    is_criterion_id(id).then_some(id)
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
    use std::cell::RefCell;
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

    /// The product id every on-disk test opens its scratch product under.
    const PRODUCT: &str = "PRODUCT-T35";

    /// A scratch product, opened and locked the way the engine opens one:
    /// an on-disk `Indexer` exists only through a `ProductDb`.
    fn product(scratch: &Scratch) -> ProductDb {
        ProductDb::open(&scratch.path, PRODUCT, Timestamp::from_millis(1_000))
            .expect("a scratch product opens")
    }

    /// `<product dir>/index/`, where `Indexer::open` puts the index.
    fn index_dir(db: &ProductDb) -> PathBuf {
        db.dir().join(INDEX_DIR)
    }

    /// `<product dir>/index/fts.sqlite`, the index file itself.
    fn index_file(db: &ProductDb) -> PathBuf {
        index_dir(db).join(INDEX_FILE)
    }

    // -------------------------------------------------------------------
    // The after-commit seam `commit` calls in test builds (see its doc).
    // -------------------------------------------------------------------

    thread_local! {
        static AFTER_COMMIT: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    }

    /// Installs `hook` to run once, on this thread, right after the next
    /// `commit` returns.
    fn set_after_commit_hook(hook: impl FnOnce() + 'static) {
        AFTER_COMMIT.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
    }

    /// Runs and clears this thread's hook, if one is installed: called by
    /// `commit` in test builds only.
    pub(super) fn run_after_commit_hook() {
        let hook = AFTER_COMMIT.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }

    // -------------------------------------------------------------------
    // The move seam `move_file` calls in test builds (see its doc).
    // -------------------------------------------------------------------

    /// Which moves fail: the predicate is asked the source path of every
    /// move `move_file` is about to make on this thread.
    type MoveFailure = Box<dyn Fn(&Path) -> bool>;

    thread_local! {
        static FAIL_MOVE: RefCell<Option<MoveFailure>> = const { RefCell::new(None) };
    }

    /// Makes every move on this thread whose source `fails` accepts fail,
    /// until the returned guard is dropped.
    fn fail_moves(fails: impl Fn(&Path) -> bool + 'static) -> impl Drop {
        struct Clear;
        impl Drop for Clear {
            fn drop(&mut self) {
                FAIL_MOVE.with(|slot| *slot.borrow_mut() = None);
            }
        }
        FAIL_MOVE.with(|slot| *slot.borrow_mut() = Some(Box::new(fails)));
        Clear
    }

    /// The error a move from `from` must fail with, if the test on this
    /// thread asked for one: called by `move_file` in test builds only.
    pub(super) fn injected_move_failure(from: &Path) -> Option<std::io::Error> {
        FAIL_MOVE.with(|slot| {
            slot.borrow().as_ref().and_then(|fails| {
                fails(from).then(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "this move was made to fail by the test",
                    )
                })
            })
        })
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
            IndexerError::WalkBudgetExhausted {
                left_out: vec![SkippedEntry {
                    path: PathBuf::from("spec"),
                    reason: SkipReason::WalkDocumentLimit { remaining: 0 },
                }],
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
    // The repository walk: the design artifact, ADR, criteria and section
    // handling.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0035_collect_from_repo_never_indexes_the_html_design_artifact() {
        // Escalation E-0006 is about this file alone: mock data for a
        // fictional product. The walk reads only .md files, so it is
        // recorded as not Markdown and none of it is indexed.
        let scratch = Scratch::new("collect-design");
        let spec = scratch.path.join("spec");
        let design = spec.join("design");
        fs::create_dir_all(&design).expect("create spec/design");
        let artifact = design.join("Ori Studio.html");
        fs::write(&artifact, "<html>Ledgerline mock data, ORI-DVG-04</html>")
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
            "the HTML design artifact must never contribute a document (escalation E-0006): \
             {documents:?}"
        );
        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert_eq!(
            walk.skipped,
            vec![SkippedEntry {
                path: artifact,
                reason: SkipReason::NotMarkdown,
            }],
            "it is recorded as a file that is not Markdown, like any other"
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
        let sections = section_documents("docs/EXAMPLE.md", text, usize::MAX).expect("no limit");
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
        let db = product(&scratch);
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
                .expect("full rebuild");
        }
        let reopened = Indexer::open(&db).expect("reopen the same directory");
        let dump = reopened.all_documents().expect("read back after reopen");
        assert_eq!(dump.len(), 1);
        assert_eq!(dump[0].0.path, "a.md");
    }

    #[test]
    fn ori_t_0035_incremental_sync_after_a_reopen_still_removes_and_upserts_correctly() {
        let scratch = Scratch::new("on-disk-incremental-reopen");
        let db = product(&scratch);
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer
                .full_rebuild(&[
                    doc("a.md", DocumentKind::Section, "A", "alpha"),
                    doc("b.md", DocumentKind::Section, "B", "beta"),
                ])
                .expect("full rebuild");
        }
        let mut reopened = Indexer::open(&db).expect("reopen the same directory");
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
        let mut db = product(&scratch);
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
                .expect("full rebuild");
        }
        assert!(
            index_file(&db).is_file(),
            "the index must be its own file, index/fts.sqlite"
        );

        // Deleting the index directory's file and rebuilding from the
        // repository (here: the same target set a caller already had)
        // reproduces the identical stored state: the derived-data property
        // the module doc's "The index is derived" names.
        let before = {
            let indexer = Indexer::open(&db).expect("reopen before delete");
            indexer.all_documents().expect("dump before delete")
        };
        fs::remove_file(index_file(&db)).expect("delete the index file");
        let after = {
            let mut indexer = Indexer::open(&db).expect("reopen after delete");
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

        // product.sqlite exists beside it (ProductDb made it) and holds none
        // of the index's tables: read through the ProductDb's own connection,
        // which needs `&mut db`, so every Indexer above is already gone.
        assert!(db.dir().join("product.sqlite").is_file());
        let index_tables: i64 = db
            .connection()
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'documents%'",
                [],
                |row| row.get(0),
            )
            .expect("read product.sqlite's schema");
        assert_eq!(
            index_tables, 0,
            "this module must never write into product.sqlite"
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

        // An embedded NUL is the one text FTS5's parser refuses even quoted
        // (it stops scanning at a NUL inside a literal), so search refuses it
        // before SQLite sees it: a typed refusal, never a panic and never
        // the raw SQLite message.
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
        let db = product(&scratch);
        let mut first = Indexer::open(&db).expect("first writer opens");
        first
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("seed the index");

        let mut second = Indexer::open(&db).expect("second connection opens");

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
        let documents =
            section_documents("docs/runbooks/restore.md", text, usize::MAX).expect("no limit");
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
        let documents =
            section_documents("docs/runbooks/code.md", text, usize::MAX).expect("no limit");
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
    /// [`corrupt_fts5_structure_row_with_an_oversized_length_prefix`] below
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
    /// `documents_data`'s row 10 that makes the bundled SQLite report
    /// `ErrorCode::OutOfMemory` (`SQLITE_NOMEM`) while reading this exact
    /// module's 80-document, single-shared-term corpus (the same shape
    /// `tests::ori_t_0035_a_corrupt_index_is_reported_as_corrupt_not_invalid_query`
    /// seeds), found empirically rather than derived from FTS5's on-disk
    /// format. Row 10 is FTS5's structure record (`FTS5_STRUCTURE_ROWID` in
    /// the bundled `sqlite3.c`; its averages record is row 1, which round 4
    /// misnamed this after): a length in it decodes to an implausible size,
    /// and every reader of the index, `PRAGMA quick_check` included, fails
    /// for want of the memory that size asks for. Not a general-purpose
    /// corruption pattern the way [`corrupt_fts5_shadow_table`] is; it
    /// exists to pin the one shape of damage the module doc's "What is
    /// reported as corrupt, and what is not" says the check cannot settle.
    const STRUCTURE_ROW_OVERSIZED_LENGTH_PREFIX: &str =
        "556f784c090469b211bcb0a1cc943696ddceafb6a695cb1485d7";

    /// Replaces `documents_data`'s row 10 with
    /// [`STRUCTURE_ROW_OVERSIZED_LENGTH_PREFIX`] through a second,
    /// independent connection: see that constant's doc for what it is and
    /// why it is its own helper rather than folded into
    /// [`corrupt_fts5_shadow_table`].
    fn corrupt_fts5_structure_row_with_an_oversized_length_prefix(path: &Path) {
        let raw =
            rusqlite::Connection::open(path).expect("open a second, raw connection to the file");
        let bytes: Vec<u8> = (0..STRUCTURE_ROW_OVERSIZED_LENGTH_PREFIX.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&STRUCTURE_ROW_OVERSIZED_LENGTH_PREFIX[i..i + 2], 16)
                    .expect("the pinned pattern is valid hex")
            })
            .collect();
        let changed = raw
            .execute(
                "UPDATE documents_data SET block = ?1 WHERE rowid = 10",
                params![bytes],
            )
            .expect("replace the structure row directly");
        assert_eq!(
            changed, 1,
            "row 10 must exist and be the one row this replaces, or this test proves nothing \
             (a schema or FTS5 version change may have moved the structure record)"
        );
    }

    #[test]
    fn ori_t_0035_a_corrupt_index_is_reported_as_corrupt_not_invalid_query() {
        let scratch = Scratch::new("corrupt-search");
        let db = product(&scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
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

        corrupt_fts5_shadow_table(&index_file(&db));

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
    fn ori_t_0035_an_out_of_memory_error_the_check_cannot_settle_is_never_called_corrupt_and_recover_still_repairs_it()
     {
        // Round 4 pinned this shape as Corrupt: its check (FTS5's
        // integrity-check command) failed here too, and it read any failure
        // of the check as damage, which is the same rule that called a
        // genuinely out-of-memory healthy index Corrupt 76 times in 76. Round
        // 5's check fails here with SQLITE_NOMEM as well, since it must read
        // the same structure record, and SQLITE_NOMEM is exactly what a real
        // out-of-memory condition looks like, so the rule is the original
        // error, unchanged, never Corrupt. What makes that acceptable is the
        // second half: recover repairs it anyway, because it never reads
        // the damaged file.
        let scratch = Scratch::new("corrupt-out-of-memory");
        let mut db = product(&scratch);
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
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&seed).expect("seed a real corpus");
            corrupt_fts5_structure_row_with_an_oversized_length_prefix(&index_file(&db));

            assert_eq!(
                quick_check(&indexer.conn),
                Integrity::Undetermined,
                "the check itself cannot run to a verdict on this damage; that is the case this \
                 test pins"
            );
            match indexer.search("alpha", 10) {
                Err(IndexerError::Sqlite { source, .. }) => assert!(
                    matches!(
                        source.sqlite_error_code(),
                        Some(rusqlite::ErrorCode::OutOfMemory)
                    ),
                    "the pinned pattern must still make SQLite report OutOfMemory, or this test \
                     is not exercising the case it names: {source:?}"
                ),
                other => panic!(
                    "an out-of-memory failure the check cannot settle must come back as the \
                     original error, never as Corrupt: {other:?}"
                ),
            }
            let others: [(&str, Result<(), IndexerError>); 3] = [
                ("all_documents", indexer.all_documents().map(|_| ())),
                (
                    "add_or_replace",
                    indexer.add_or_replace(&doc("z.md", DocumentKind::Section, "Z", "zulu")),
                ),
                (
                    "incremental_sync",
                    indexer.incremental_sync(&seed).map(|_| ()),
                ),
            ];
            for (call, result) in others {
                assert!(
                    !matches!(result, Err(IndexerError::Corrupt { .. })),
                    "{call} must not call this Corrupt either: {result:?}"
                );
            }
        }

        let report = Indexer::recover(&mut db, &seed, Timestamp::from_millis(2_000))
            .expect("recover never reads the damaged file, so this damage cannot stop it");
        assert_eq!(report.rebuilt.total, seed.len());
        let indexer = Indexer::open(&db).expect("the fresh index opens");
        let hits = indexer
            .search("alpha", 100)
            .expect("search works after recovery");
        assert_eq!(hits.hits.len(), seed.len());
    }

    #[test]
    fn ori_t_0035_full_rebuild_recovers_a_corrupt_on_disk_index() {
        let scratch = Scratch::new("corrupt-recover");
        let db = product(&scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
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

        corrupt_fts5_shadow_table(&index_file(&db));
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
        let db = product(&scratch);
        let mut seeder = Indexer::open(&db).expect("seed connection opens");
        seeder.full_rebuild(&[]).expect("start empty");
        drop(seeder);

        // Each thread gets its own Indexer, opened here from the one
        // ProductDb and moved into a scoped thread: an Indexer is `Send`,
        // and the scope ends before `db` could be borrowed mutably.
        let mut writer = Indexer::open(&db).expect("writer opens");
        let reader = Indexer::open(&db).expect("reader opens");
        let (vacuous, inverse) = std::thread::scope(|scope| {
            let writer = scope.spawn(move || {
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

            let reader = scope.spawn(move || {
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
            reader.join().expect("reader thread must not panic")
        });
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
        let db = product(&scratch);
        let mut seeder = Indexer::open(&db).expect("seed connection opens");
        seeder
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "original")])
            .expect("seed the target document");
        drop(seeder);

        let mut writer = Indexer::open(&db).expect("writer opens");
        let mut syncer = Indexer::open(&db).expect("syncer opens");
        std::thread::scope(|scope| {
            let writer = scope.spawn(move || {
                for _ in 0..300 {
                    let _ = writer.add_or_replace(&doc(
                        "intruder.md",
                        DocumentKind::Section,
                        "I",
                        "an unrelated document",
                    ));
                }
            });

            let syncer = scope.spawn(move || {
                for _ in 0..300 {
                    let _ = syncer.incremental_sync(&[doc(
                        "a.md",
                        DocumentKind::Section,
                        "A",
                        "original",
                    )]);
                }
            });

            writer.join().expect("writer thread must not panic");
            syncer.join().expect("syncer thread must not panic");
        });

        // Contention has stopped; one final, uncontested sync must converge
        // to exactly the target, proving the index survived the race
        // intact rather than corrupted or permanently stuck.
        let mut settle = Indexer::open(&db).expect("settle connection opens");
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
    // Round 9, item 3: the spec/design/ exclusion was a mistake, corrected
    // by the lead. DESIGN.md is a Draft specification document (a gate G2
    // artifact) and is indexed like any spec file; E-0006 concerns only the
    // HTML file, which the walk never reads.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_every_markdown_file_under_spec_design_is_indexed_like_any_spec_file() {
        let scratch = Scratch::new("design-indexed");
        let spec = scratch.path.join("spec");
        let design = spec.join("design");
        fs::create_dir_all(design.join("screens")).expect("create spec/design/screens");
        fs::create_dir_all(spec.join("runbooks").join("design"))
            .expect("create spec/runbooks/design");
        let design_md = ["spec", "design", "DESIGN.md"].join("/");
        let files = [
            (
                design_md.clone(),
                "Draft preamble designword\n\n# Design\n\n## Tokens\n\ntokenword\n",
            ),
            (
                ["spec", "design", "screens", "fleet.md"].join("/"),
                "# Fleet\n\nscreenword\n",
            ),
            (
                ["spec", "runbooks", "design", "R.md"].join("/"),
                "# Runbook design notes\n\nrunbookword\n",
            ),
        ];
        for (relative, text) in &files {
            fs::write(scratch.path.join(relative), text).expect("write a spec file");
        }

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert!(
            walk.skipped.is_empty(),
            "no directory under spec/ is skipped by name: {:?}",
            walk.skipped
        );
        let paths: Vec<&str> = walk.documents.iter().map(|d| d.path.as_str()).collect();
        for expected in [
            design_md.clone(),
            format!("{design_md}#design"),
            format!("{design_md}#tokens"),
            format!("{}#fleet", files[1].0),
            format!("{}#runbook-design-notes", files[2].0),
        ] {
            assert!(
                paths.contains(&expected.as_str()),
                "{expected} is indexed: {paths:?}"
            );
        }
        let design_documents: Vec<&IndexableDocument> = walk
            .documents
            .iter()
            .filter(|d| document_file(&d.path) == design_md)
            .collect();
        let reference =
            section_documents(&design_md, files[0].1, MAX_FILE_DOCUMENTS).expect("split");
        assert_eq!(
            design_documents,
            reference.iter().collect::<Vec<_>>(),
            "DESIGN.md is split exactly as any section file under spec/ is"
        );
        assert_eq!(
            assert_every_entry_is_indexed_or_skipped(&scratch.path, &walk),
            files.len()
        );

        let mut indexer = Indexer::open_in_memory().expect("an in-memory index");
        indexer
            .full_rebuild(&Indexer::collect_from_repo(&scratch.path).expect("collect"))
            .expect("index the walk");
        for word in ["designword", "tokenword", "screenword", "runbookword"] {
            let found = indexer.search(word, 10).expect("search");
            assert_eq!(found.hits.len(), 1, "{word} is searchable: {found:?}");
        }
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
        let db = product(&scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
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

        let fts_path = index_file(&db);
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
        let db = product(&scratch);
        let mut a = Indexer::open(&db).expect("A opens");
        a.full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
            .expect("seed");
        let mut b = Indexer::open(&db).expect("B opens the same directory");

        corrupt_fts5_shadow_table(&index_file(&db));
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
        let fresh = Indexer::open(&db).expect("a fresh connection opens");
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
        let db = product(&scratch);
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer
                .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha content")])
                .expect("seed");
        }
        let fts_path = index_file(&db);
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

        match Indexer::open(&db) {
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

    fn insert_raw_row(file: &Path, path: &str, kind: &str, title: &str, body: &str, checksum: i64) {
        let conn =
            rusqlite::Connection::open(file).expect("open a raw connection to the on-disk index");
        conn.execute(
            "INSERT INTO documents (path, kind, title, body, checksum) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![path, kind, title, body, checksum],
        )
        .expect("insert a raw row with an unparseable kind");
    }

    #[test]
    fn ori_t_0035_incremental_sync_removes_an_unparseable_kind_row_absent_from_target() {
        let scratch = Scratch::new("unparseable-removed");
        let db = product(&scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        indexer
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "alpha")])
            .expect("seed");
        insert_raw_row(&index_file(&db), "x.md", "prompt", "X", "xray", 7);

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
        let db = product(&scratch);
        let indexer_setup = Indexer::open(&db).expect("open on-disk index");
        drop(indexer_setup);
        insert_raw_row(&index_file(&db), "x.md", "prompt", "X", "xray", 7);

        let mut indexer = Indexer::open(&db).expect("reopen");
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
        let documents =
            section_documents("docs/runbooks/release.md", text, usize::MAX).expect("no limit");
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
    // Item 4(c) (LOW): IndexReport.total is read inside the same
    // transaction as the write that produced it, never by a later,
    // separate autocommit statement that can describe another writer's
    // state.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_full_rebuild_total_never_disagrees_with_a_concurrent_writer() {
        let scratch = Scratch::new("total-race");
        let db = product(&scratch);
        let mut seeder = Indexer::open(&db).expect("seed connection opens");
        seeder.full_rebuild(&[]).expect("start empty");
        drop(seeder);

        let mut other = Indexer::open(&db).expect("other writer opens");
        let mut mine = Indexer::open(&db).expect("this writer opens");
        std::thread::scope(|scope| {
            let other = scope.spawn(move || {
                for _ in 0..600 {
                    let _ = other.full_rebuild(&[]);
                }
            });

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
        });
    }

    // =====================================================================
    // Round 5. One test per item of the round-5 brief (recorded in the pull
    // request report), each checked to fail against e76ba23 or under the
    // plant the report names, and to pass after.
    // =====================================================================

    /// Every stored row exactly as SQLite holds it (no column parsed, no row
    /// left out, `rowid` excluded since a rewrite changes it), sorted: what
    /// "stored exactly as `full_rebuild` would store it" is compared by,
    /// since [`Indexer::all_documents`] leaves out a row whose kind does not
    /// parse and so cannot see the difference these tests are about.
    fn raw_rows(file: &Path) -> Vec<String> {
        let raw = Connection::open(file).expect("open a raw connection to the index");
        let mut statement = raw
            .prepare("SELECT path, kind, title, body, checksum FROM documents")
            .expect("prepare the raw dump");
        let mut rows: Vec<String> = statement
            .query_map([], |row| {
                let mut columns = Vec::new();
                for index in 0..5 {
                    columns.push(format!("{:?}", row.get_ref(index)?));
                }
                Ok(columns.join(" | "))
            })
            .expect("run the raw dump")
            .collect::<rusqlite::Result<_>>()
            .expect("read every raw row");
        rows.sort();
        rows
    }

    /// `PRAGMA page_size` and the (b-tree, page type) of every page `dbstat`
    /// knows, for `file`.
    fn page_map(file: &Path) -> (usize, BTreeMap<i64, (String, String)>) {
        let raw = Connection::open(file).expect("open a raw connection to the index");
        let page_size: i64 = raw
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .expect("read the page size");
        let mut statement = raw
            .prepare("SELECT pageno, name, pagetype FROM dbstat")
            .expect("prepare the dbstat read");
        let map = statement
            .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))
            .expect("read dbstat")
            .collect::<rusqlite::Result<_>>()
            .expect("read every dbstat row");
        (
            usize::try_from(page_size).expect("a positive page size"),
            map,
        )
    }

    /// Checkpoints `file`'s write-ahead log into it and truncates the log,
    /// so the file alone holds the whole index: what a cleanly closed index
    /// looks like on disk.
    fn checkpoint(file: &Path) {
        let raw = Connection::open(file).expect("open a raw connection to the index");
        raw.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .expect("checkpoint the index");
    }

    /// A 700-document corpus: large enough that `documents_content`,
    /// `documents_data` and `documents_docsize` each have an interior page
    /// as well as leaves, so the page sweep below damages every kind of
    /// page this index has.
    fn sweep_corpus() -> Vec<IndexableDocument> {
        (0..700)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "T",
                    &format!("needle alpha u{n}"),
                )
            })
            .collect()
    }

    // ---------------------------------------------------------------------
    // Item 1 (HIGH): recovery that works, safe by construction.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_recover_repairs_every_page_of_a_damaged_index_in_a_deterministic_sweep() {
        // The reviewers' method: overwrite each page in turn with a fixed
        // pattern, on a fresh copy each time, for 0x00 and for 0xFF. Round
        // 4's only recovery, full_rebuild, repaired 0 of every such sweep
        // (its DROP TABLE fails on the damaged page). After recover, every
        // case must open, pass PRAGMA integrity_check, and find every
        // rebuilt document; and the damaged file must be in quarantine,
        // byte for byte, never deleted.
        let scratch = Scratch::new("page-sweep");
        let mut db = product(&scratch);
        let corpus = sweep_corpus();
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&corpus).expect("seed the corpus");
        }
        let file = index_file(&db);
        checkpoint(&file);
        let pristine = fs::read(&file).expect("read the pristine index");
        let (page_size, pages_by_number) = page_map(&file);
        let page_count = pristine.len() / page_size;
        assert_eq!(pristine.len() % page_size, 0, "a whole number of pages");

        // Non-vacuity: the sweep reaches leaf and interior pages of every
        // shadow b-tree that has more than one page, and the schema page.
        let kinds: BTreeSet<(&str, &str)> = pages_by_number
            .values()
            .map(|(name, kind)| (name.as_str(), kind.as_str()))
            .collect();
        for needed in [
            ("sqlite_schema", "leaf"),
            ("documents_config", "leaf"),
            ("documents_idx", "leaf"),
            ("documents_content", "internal"),
            ("documents_content", "leaf"),
            ("documents_data", "internal"),
            ("documents_data", "leaf"),
            ("documents_docsize", "internal"),
            ("documents_docsize", "leaf"),
        ] {
            assert!(
                kinds.contains(&needed),
                "the sweep corpus must produce a {needed:?} page, or the sweep does not cover \
                 it: {kinds:?}"
            );
        }

        let mut cases = 0usize;
        for fill in [0x00u8, 0xFF] {
            for page in 1..=page_count {
                let mut damaged = pristine.clone();
                damaged[(page - 1) * page_size..page * page_size].fill(fill);
                for name in INDEX_FILE_SET {
                    let _ = fs::remove_file(index_dir(&db).join(name));
                }
                fs::write(&file, &damaged).expect("plant the damaged copy");

                // The check sees every one of these, including the pages an
                // ordinary search never touches. Through a raw connection,
                // since page 1 damage stops `Indexer::open`; closed before
                // `recover` (a raw connection is exactly what the proof in
                // the module doc's "Recovery" does not cover).
                {
                    let raw =
                        Connection::open(&file).expect("a raw connection to the damaged copy");
                    assert_eq!(
                        quick_check(&raw),
                        Integrity::Damaged,
                        "page {page} filled with {fill:#04x}: the check reports the damage"
                    );
                }

                let at = Timestamp::from_millis(10_000 + i64::try_from(cases).expect("small"));
                let report = Indexer::recover(&mut db, &corpus, at).unwrap_or_else(|error| {
                    panic!("page {page} filled with {fill:#04x}: recover must succeed: {error}")
                });
                let quarantined = report
                    .quarantine
                    .as_ref()
                    .expect("the damaged file was there to move")
                    .join(INDEX_FILE);
                assert_eq!(
                    fs::read(&quarantined).expect("read the quarantined file"),
                    damaged,
                    "page {page} filled with {fill:#04x}: the damaged file must be kept, byte \
                     for byte, as evidence"
                );
                assert_eq!(report.rebuilt.total, corpus.len());

                let indexer = Indexer::open(&db).unwrap_or_else(|error| {
                    panic!("page {page} filled with {fill:#04x}: the fresh index opens: {error}")
                });
                let integrity: String = indexer
                    .conn
                    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                    .expect("run PRAGMA integrity_check");
                assert_eq!(
                    integrity, "ok",
                    "page {page} filled with {fill:#04x}: the fresh index passes integrity_check"
                );
                let found = indexer
                    .search("needle", corpus.len() + 1)
                    .expect("search the fresh index");
                assert_eq!(
                    (found.hits.len(), found.documents_covered),
                    (corpus.len(), corpus.len()),
                    "page {page} filled with {fill:#04x}: every rebuilt document is found"
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 2 * page_count);
        let kept = fs::read_dir(index_dir(&db).join(QUARANTINE_DIR))
            .expect("list the quarantine")
            .count();
        assert_eq!(
            kept, cases,
            "every recovery keeps its own quarantine directory; none is reused or deleted"
        );
    }

    #[test]
    fn ori_t_0035_a_destroyed_header_is_corrupt_at_open_and_recover_repairs_it() {
        // Round 4 reported this (SQLITE_NOTADB at the first pragma) as a
        // generic Sqlite error, and even its Corrupt cases told the caller
        // to call full_rebuild on an Indexer that could not be opened.
        let scratch = Scratch::new("destroyed-header");
        let mut db = product(&scratch);
        let corpus = vec![doc("a.md", DocumentKind::Section, "A", "alpha content")];
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&corpus).expect("seed");
        }
        let file = index_file(&db);
        checkpoint(&file);
        {
            use std::io::Seek;
            use std::io::Write;
            let mut raw = fs::OpenOptions::new()
                .write(true)
                .open(&file)
                .expect("open the raw file for writing");
            raw.seek(std::io::SeekFrom::Start(0)).expect("seek");
            raw.write_all(&[0xFF; 100]).expect("destroy the header");
        }

        let message = match Indexer::open(&db) {
            Err(error @ IndexerError::Corrupt { .. }) => {
                if let IndexerError::Corrupt { source, .. } = &error {
                    assert_eq!(
                        source.sqlite_error_code(),
                        Some(rusqlite::ErrorCode::NotADatabase),
                        "this is the SQLITE_NOTADB case: {source:?}"
                    );
                }
                error.to_string()
            }
            Err(other) => panic!("a destroyed header must be Corrupt, not {other:?}"),
            Ok(_) => panic!("a destroyed header must not open"),
        };
        assert!(
            message.contains("Indexer::recover") && !message.contains("full_rebuild"),
            "the message must name the one operation that can succeed, and never advise one \
             that cannot: {message}"
        );

        Indexer::recover(&mut db, &corpus, Timestamp::from_millis(2_000))
            .expect("recover repairs damage found at open");
        let indexer = Indexer::open(&db).expect("the fresh index opens");
        assert_eq!(
            indexer.search("alpha", 10).expect("search").hits.len(),
            1,
            "the rebuilt document is found"
        );
    }

    #[test]
    fn ori_t_0035_full_rebuild_on_a_damaged_page_is_corrupt_and_says_to_recover_never_to_retry() {
        let scratch = Scratch::new("rebuild-page-damage");
        let mut db = product(&scratch);
        let corpus = sweep_corpus();
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&corpus).expect("seed");
        }
        let file = index_file(&db);
        checkpoint(&file);
        let (page_size, pages_by_number) = page_map(&file);
        let data_leaf = pages_by_number
            .iter()
            .find(|(_, (name, kind))| name == "documents_data" && kind == "leaf")
            .map(|(page, _)| usize::try_from(*page).expect("a positive page number"))
            .expect("a documents_data leaf page");
        let mut damaged = fs::read(&file).expect("read the index");
        damaged[(data_leaf - 1) * page_size..data_leaf * page_size].fill(0xFF);
        fs::write(&file, &damaged).expect("damage one page");

        {
            let mut indexer = Indexer::open(&db).expect("page damage does not stop open");
            for attempt in 0..3 {
                match indexer.full_rebuild(&corpus) {
                    Err(error @ IndexerError::Corrupt { .. }) => {
                        let message = error.to_string();
                        assert!(
                            message.contains("Indexer::recover")
                                && !message.contains("full_rebuild"),
                            "attempt {attempt}: the message must name recover and never advise \
                             the rebuild that just failed: {message}"
                        );
                    }
                    other => panic!(
                        "attempt {attempt}: a DROP that fails on a damaged page is Corrupt, \
                         every time: {other:?}"
                    ),
                }
            }
        }
        Indexer::recover(&mut db, &corpus, Timestamp::from_millis(3_000))
            .expect("recover repairs what full_rebuild cannot");
        let indexer = Indexer::open(&db).expect("the fresh index opens");
        assert_eq!(
            indexer
                .search("needle", corpus.len() + 1)
                .expect("search")
                .hits
                .len(),
            corpus.len()
        );
    }

    #[test]
    fn ori_t_0035_recover_quarantines_the_write_ahead_log_with_its_database_as_evidence() {
        // A write-ahead log holding committed frames the database file does
        // not: part of the evidence. Left at the live path, SQLite would
        // discard it once the fresh index beside it was used (see the module
        // doc's "Recovery"). recover moves it with the database, byte for
        // byte.
        let scratch = Scratch::new("leftover-wal");
        let mut db = product(&scratch);
        let stale: Vec<IndexableDocument> = (0..50)
            .map(|n| {
                doc(
                    &format!("old{n}.md"),
                    DocumentKind::Section,
                    "Old",
                    "stale words",
                )
            })
            .collect();
        let held = scratch.path.join("held");
        fs::create_dir_all(&held).expect("create a holding directory");
        {
            let mut writer = Indexer::open(&db).expect("open on-disk index");
            writer
                .conn
                .execute_batch("PRAGMA wal_autocheckpoint = 0")
                .expect("keep every commit in the log");
            writer.full_rebuild(&stale).expect("write the stale index");
            // Copied while the connection is still open, so the log still
            // holds committed frames the database file does not.
            for name in [INDEX_FILE, "fts.sqlite-wal"] {
                fs::copy(index_dir(&db).join(name), held.join(name)).expect("copy a live file");
            }
        }
        for name in INDEX_FILE_SET {
            let _ = fs::remove_file(index_dir(&db).join(name));
        }
        for name in [INDEX_FILE, "fts.sqlite-wal"] {
            fs::copy(held.join(name), index_dir(&db).join(name)).expect("plant a live file");
        }
        let planted_log = fs::read(held.join("fts.sqlite-wal")).expect("read the planted log");
        assert!(
            planted_log.len() > 32,
            "the planted log must hold frames, not only a header, or this proves nothing"
        );

        let fresh = vec![doc("new.md", DocumentKind::Section, "New", "fresh words")];
        let report = Indexer::recover(&mut db, &fresh, Timestamp::from_millis(4_000))
            .expect("recover succeeds");
        let quarantine = report.quarantine.expect("files were moved");
        assert_eq!(
            fs::read(quarantine.join("fts.sqlite-wal")).expect("the log is in quarantine"),
            planted_log,
            "the log is kept, byte for byte, beside the database it belongs to"
        );
        assert!(quarantine.join(INDEX_FILE).is_file());

        let indexer = Indexer::open(&db).expect("the fresh index opens");
        let integrity: String = indexer
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("run PRAGMA integrity_check");
        assert_eq!(integrity, "ok");
        assert_eq!(indexer.search("stale", 100).expect("search").hits.len(), 0);
        let found = indexer.search("fresh", 100).expect("search");
        assert_eq!((found.hits.len(), found.documents_covered), (1, 1));
    }

    #[test]
    fn ori_t_0035_recover_names_quarantine_directories_in_recovery_order_and_never_reuses_one() {
        // A review found round 7 naming each directory from recovered_at
        // alone, the caller's wall clock: recoveries at 5, 5 and then 4
        // sorted the last one first, and this test asserted that order as
        // correct. The recovery count now comes first in the name.
        let scratch = Scratch::new("quarantine-names");
        let mut db = product(&scratch);
        let corpus = vec![doc("a.md", DocumentKind::Section, "A", "alpha")];

        // A product whose index was never opened has nothing to move.
        let first = Indexer::recover(&mut db, &corpus, Timestamp::from_millis(5))
            .expect("recover with nothing to move");
        assert_eq!(first.quarantine, None);
        assert!(first.quarantined_files.is_empty());
        assert_eq!(first.rebuilt.total, 1);

        let mut names = Vec::new();
        for millis in [5, 5, 4] {
            let report = Indexer::recover(&mut db, &corpus, Timestamp::from_millis(millis))
                .expect("recover");
            let quarantine = report.quarantine.expect("the previous index was moved");
            assert_eq!(
                report.quarantined_files.last(),
                Some(&quarantine.join(INDEX_FILE)),
                "the database itself is moved last, after its side files"
            );
            names.push(
                quarantine
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("a UTF-8 name")
                    .to_owned(),
            );
        }
        assert_eq!(
            names,
            vec![
                "00000000000000000001-00000000000000000005",
                "00000000000000000002-00000000000000000005",
                "00000000000000000003-00000000000000000004",
            ],
            "each name carries its recovery's count, then its recovered_at; two recoveries \
             in one millisecond get two names, and none is reused"
        );
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(
            sorted, names,
            "names sort in the order recoveries happened, the one whose clock stepped back \
             included"
        );
    }

    /// The path of every row of the index file in the quarantine directory
    /// `quarantine`, read from a copy of the directory so the evidence
    /// itself is never opened.
    fn quarantined_paths(scratch: &Scratch, quarantine: &Path) -> Vec<String> {
        let copy = scratch.path.join(format!(
            "copy-of-{}",
            quarantine.file_name().expect("a name").to_string_lossy()
        ));
        fs::create_dir_all(&copy).expect("create the copy");
        for entry in fs::read_dir(quarantine).expect("list the quarantine directory") {
            let entry = entry.expect("an entry");
            fs::copy(entry.path(), copy.join(entry.file_name())).expect("copy a quarantined file");
        }
        let raw = Connection::open(copy.join(INDEX_FILE)).expect("open the copy");
        let mut statement = raw
            .prepare("SELECT path FROM documents ORDER BY path")
            .expect("prepare");
        statement
            .query_map([], |row| row.get(0))
            .expect("run")
            .collect::<rusqlite::Result<_>>()
            .expect("read every path")
    }

    #[test]
    fn ori_t_0035_the_last_quarantine_directory_in_name_order_holds_the_newest_evidence_after_the_clock_steps_back()
     {
        // The review's probe: a first recovery at T moves the index holding
        // "one" and rebuilds it with "two"; the caller's clock then steps
        // back a second; a second recovery moves the index holding "two".
        // Round 7's last directory in name order held "one", the older
        // evidence.
        let scratch = Scratch::new("quarantine-clock");
        let mut db = product(&scratch);
        let body = |word: &str| {
            vec![doc(
                &format!("{word}.md"),
                DocumentKind::Section,
                word,
                word,
            )]
        };
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&body("one")).expect("seed");
        }
        let at = 1_700_000_003_000;
        let older = Indexer::recover(&mut db, &body("two"), Timestamp::from_millis(at))
            .expect("the first recovery")
            .quarantine
            .expect("the first index was moved");
        let newest = Indexer::recover(&mut db, &body("three"), Timestamp::from_millis(at - 1_000))
            .expect("the second recovery, its clock a second behind")
            .quarantine
            .expect("the second index was moved");
        let quarantine_root = index_dir(&db).join(QUARANTINE_DIR);
        let mut listing: Vec<std::ffi::OsString> = fs::read_dir(&quarantine_root)
            .expect("list quarantine/")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        listing.sort();
        let name = |path: &Path| path.file_name().expect("a name").to_owned();
        assert_eq!(listing, vec![name(&older), name(&newest)]);
        assert_eq!(
            quarantined_paths(&scratch, &newest),
            ["two.md"],
            "the last directory in name order holds the newest evidence"
        );
        assert_eq!(quarantined_paths(&scratch, &older), ["one.md"]);

        // A count already taken, by whatever made it, orders the next
        // recovery after it; a name of any other shape is not a count.
        fs::create_dir(quarantine_root.join("00000000000000000041-00000000000000000000"))
            .expect("plant a higher count");
        fs::create_dir(quarantine_root.join("99999999999999999999-x"))
            .expect("plant a name of another shape");
        let next = Indexer::recover(&mut db, &body("four"), Timestamp::from_millis(at - 2_000))
            .expect("the third recovery")
            .quarantine
            .expect("the third index was moved");
        assert_eq!(
            next.file_name().and_then(|name| name.to_str()),
            Some(format!("{:020}-{:020}", 42, at - 2_000).as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_recover_refuses_an_index_directory_that_is_a_symbolic_link() {
        let scratch = Scratch::new("recover-symlink");
        let mut db = product(&scratch);
        let elsewhere = scratch.path.join("another-products-index");
        fs::create_dir_all(&elsewhere).expect("create the other directory");
        fs::write(elsewhere.join(INDEX_FILE), b"another product's index")
            .expect("write the other product's file");
        fs::remove_dir(index_dir(&db)).expect("remove the real index directory");
        std::os::unix::fs::symlink(&elsewhere, index_dir(&db)).expect("link index/ elsewhere");

        match Indexer::recover(&mut db, &[], Timestamp::from_millis(1)) {
            Err(error @ IndexerError::RecoveryRefused { .. }) => {
                assert_eq!(error.methodology_ref().section, 25);
            }
            other => panic!("a symlinked index/ must be refused: {other:?}"),
        }
        assert_eq!(
            fs::read(elsewhere.join(INDEX_FILE)).expect("the other file is still there"),
            b"another product's index",
            "nothing was moved out of a directory this product does not own"
        );
        assert!(!elsewhere.join(QUARANTINE_DIR).exists());
    }

    // ---------------------------------------------------------------------
    // Item 2 (MEDIUM): diagnosis never calls a healthy index Corrupt.
    // ---------------------------------------------------------------------

    /// Runs this test binary again as a child process, running only the
    /// test at `name` with `env` set, and asserts that exactly that one test
    /// ran and passed. For the tests below that lower SQLite's process-wide
    /// heap limit, which must never happen inside the process every other
    /// test in this binary shares: the limit can only be lowered, never
    /// raised back, without an `unsafe` call this crate does not make.
    fn run_in_child_process(name: &str, env: &str, extra_env: &[(&str, &str)]) {
        let exe = std::env::current_exe().expect("the test binary's own path");
        let mut command = std::process::Command::new(exe);
        command
            .args([name, "--exact", "--nocapture", "--test-threads=1"])
            .env(env, "1");
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let output = command.output().expect("the test binary runs as a child");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "the child run of {name} failed:\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("test result: ok. 1 passed"),
            "the child must have run exactly {name}, not zero tests:\n{stdout}"
        );
        // The child's own measurements, visible under `--nocapture`.
        eprint!("{stderr}");
    }

    #[test]
    fn ori_t_0035_a_genuine_out_of_memory_condition_on_a_healthy_index_is_never_called_corrupt() {
        const CHILD: &str = "ORI_T_0035_GENUINE_OOM_CHILD";
        const ROOT: &str = "ORI_T_0035_GENUINE_OOM_ROOT";
        let Some(root) = std::env::var_os(ROOT).filter(|_| std::env::var_os(CHILD).is_some())
        else {
            // The parent: seed a healthy index, hand it to a child that
            // starves SQLite of memory, then check it is still healthy.
            let scratch = Scratch::new("genuine-oom");
            let corpus: Vec<IndexableDocument> = (0..2000)
                .map(|n| {
                    doc(
                        &format!("d{n}.md"),
                        DocumentKind::Section,
                        "D",
                        &format!("needle alpha{n} shared body text for a real doclist"),
                    )
                })
                .collect();
            {
                let db = product(&scratch);
                let mut indexer = Indexer::open(&db).expect("open on-disk index");
                indexer.full_rebuild(&corpus).expect("seed");
            }
            let root = scratch.path.to_str().expect("a UTF-8 scratch path");
            run_in_child_process(
                "indexer::tests::ori_t_0035_a_genuine_out_of_memory_condition_on_a_healthy_index_is_never_called_corrupt",
                CHILD,
                &[(ROOT, root)],
            );
            let db = product(&scratch);
            let indexer = Indexer::open(&db).expect("the index still opens");
            let integrity: String = indexer
                .conn
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .expect("run PRAGMA integrity_check");
            assert_eq!(
                integrity, "ok",
                "the index was healthy all along: nothing the child was told was corruption"
            );
            return;
        };

        // The child: every public call, repeated while SQLite's heap limit
        // steps down from ample to starved. Each outcome is Ok, or a
        // failure for want of memory; never Corrupt.
        let db = ProductDb::open(Path::new(&root), PRODUCT, Timestamp::from_millis(2_000))
            .expect("the child opens the parent's product");
        let corpus: Vec<IndexableDocument> = (0..2000)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    &format!("needle alpha{n} shared body text for a real doclist"),
                )
            })
            .collect();
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        assert_eq!(quick_check(&indexer.conn), Integrity::Intact);
        let (mut ok, mut out_of_memory, mut corrupt, mut other) = (0usize, 0usize, 0usize, 0usize);
        for limit_kib in (32..=8192u32).rev().step_by(32) {
            let _ = indexer.conn.query_row(
                &format!("PRAGMA hard_heap_limit = {}", u64::from(limit_kib) * 1024),
                [],
                |row| row.get::<_, i64>(0),
            );
            let results: [Result<(), IndexerError>; 5] = [
                indexer.search("needle", 50).map(|_| ()),
                indexer.search("alpha123", 50).map(|_| ()),
                indexer.all_documents().map(|_| ()),
                indexer.add_or_replace(&corpus[7]),
                indexer.incremental_sync(&corpus).map(|_| ()),
            ];
            for result in results {
                match result {
                    Ok(()) => ok += 1,
                    Err(IndexerError::Corrupt { source, .. }) => {
                        corrupt += 1;
                        eprintln!("limit {limit_kib} KiB: Corrupt from {source:?}");
                    }
                    Err(IndexerError::Sqlite { source, .. })
                        if source.sqlite_error_code() == Some(rusqlite::ErrorCode::OutOfMemory) =>
                    {
                        out_of_memory += 1;
                    }
                    Err(error) => {
                        other += 1;
                        eprintln!("limit {limit_kib} KiB: {error:?}");
                    }
                }
            }
        }
        eprintln!(
            "genuine out-of-memory sweep: ok {ok}, out of memory {out_of_memory}, corrupt \
             {corrupt}, other {other}"
        );
        assert!(
            ok > 0 && out_of_memory > 0,
            "the sweep must reach both an ample and a starved heap, or it proves nothing: ok \
             {ok}, out of memory {out_of_memory}"
        );
        assert_eq!(
            corrupt, 0,
            "a genuine out-of-memory condition on a healthy index is never Corrupt"
        );
    }

    #[test]
    fn ori_t_0035_a_concurrent_writer_never_turns_a_healthy_index_corrupt() {
        // The review's reproduction: a row another writer stored with a NULL
        // kind makes a search that reaches it fail on a healthy index (a
        // type error, not damage), so the failure goes to the check. Round
        // 4's check was a write: another connection holding the write lock
        // made it fail with SQLITE_BUSY, and a writer committing made it
        // fail with SQLITE_BUSY_SNAPSHOT, and either failure became Corrupt.
        let scratch = Scratch::new("writer-not-corrupt");
        let db = product(&scratch);
        let file = index_file(&db);
        {
            let mut seeder = Indexer::open(&db).expect("open on-disk index");
            let corpus: Vec<IndexableDocument> = (0..200)
                .map(|n| {
                    doc(
                        &format!("d{n}.md"),
                        DocumentKind::Section,
                        "D",
                        "needle body",
                    )
                })
                .collect();
            seeder.full_rebuild(&corpus).expect("seed");
        }
        Connection::open(&file)
            .expect("a raw connection")
            .execute(
                "INSERT INTO documents (path, kind, title, body, checksum) \
                 VALUES ('foreign.md', NULL, 'F', 'foreign words', 1)",
                [],
            )
            .expect("store a row this build would never write");

        let reader = Indexer::open(&db).expect("the reader opens");
        let assert_not_corrupt =
            |result: Result<SearchReport, IndexerError>, when: &str| match result {
                Err(IndexerError::Sqlite { source, .. }) => assert!(
                    matches!(source, rusqlite::Error::InvalidColumnType(..)),
                    "{when}: the original error, unchanged: {source:?}"
                ),
                other => panic!("{when}: a healthy index is never Corrupt: {other:?}"),
            };

        // Deterministic: another connection holds the write lock throughout.
        let holder = Connection::open(&file).expect("a lock-holding connection");
        holder
            .execute_batch("BEGIN IMMEDIATE")
            .expect("take the write lock");
        assert_not_corrupt(
            reader.search("foreign", 10),
            "with the write lock held elsewhere",
        );
        holder.execute_batch("ROLLBACK").expect("release it");

        // A writer committing throughout.
        let mut writer = Indexer::open(&db).expect("the writer opens");
        let stop = std::sync::atomic::AtomicBool::new(false);
        let commits = std::thread::scope(|scope| {
            let stop = &stop;
            let churn = scope.spawn(move || {
                let mut commits = 0usize;
                let mut round = 0usize;
                while !stop.load(Ordering::SeqCst) {
                    round += 1;
                    let body = format!("churn {round}");
                    if writer
                        .add_or_replace(&doc("churn.md", DocumentKind::Section, "C", &body))
                        .is_ok()
                    {
                        commits += 1;
                    }
                }
                commits
            });
            for round in 0..300 {
                assert_not_corrupt(
                    reader.search("foreign", 10),
                    &format!("search {round} with a writer committing"),
                );
            }
            stop.store(true, Ordering::SeqCst);
            churn.join().expect("the writer thread must not panic")
        });
        assert!(
            commits > 0,
            "the writer must actually have committed during the searches"
        );
        assert_eq!(
            quick_check(&reader.conn),
            Integrity::Intact,
            "and the index really is healthy"
        );
    }

    #[test]
    fn ori_t_0035_all_documents_reports_damage_it_trips_over_as_corrupt_not_a_generic_error() {
        // A body replaced through SQL by a blob: all_documents fails reading
        // it as text (a type error, not SQLITE_CORRUPT), and round 4 passed
        // that straight through as a generic Sqlite error. The content no
        // longer matches the inverted index, which the check reports.
        let scratch = Scratch::new("all-documents-routed");
        let mut db = product(&scratch);
        let corpus: Vec<IndexableDocument> = (0..20)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "T",
                    &format!("needle alpha u{n}"),
                )
            })
            .collect();
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&corpus).expect("seed");
            Connection::open(index_file(&db))
                .expect("a raw connection")
                .execute(
                    "UPDATE documents_content SET c3 = x'00ff10' WHERE id = 3",
                    [],
                )
                .expect("damage one stored body");
            match indexer.all_documents() {
                Err(IndexerError::Corrupt { source, .. }) => assert!(
                    matches!(source, rusqlite::Error::InvalidColumnType(..)),
                    "the original error is kept: {source:?}"
                ),
                other => panic!("the check reports this damage, so it is Corrupt: {other:?}"),
            }
        }
        Indexer::recover(&mut db, &corpus, Timestamp::from_millis(6_000)).expect("recover");
        let indexer = Indexer::open(&db).expect("the fresh index opens");
        assert_eq!(
            indexer.all_documents().expect("read back").len(),
            corpus.len()
        );
    }

    #[test]
    fn ori_t_0035_an_fts5_format_this_build_does_not_read_is_reported_as_it_is_and_recover_replaces_it()
     {
        // A newer build's index and a damaged config row look the same from
        // here: FTS5 refuses the version number with a plain SQLITE_ERROR,
        // and so does the check. It is reported as that error, never as
        // Corrupt, and recover replaces it without reading it.
        let scratch = Scratch::new("fts5-format");
        let mut db = product(&scratch);
        let corpus = vec![doc("a.md", DocumentKind::Section, "A", "alpha content")];
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&corpus).expect("seed");
        }
        Connection::open(index_file(&db))
            .expect("a raw connection")
            .execute("UPDATE documents_config SET v = 99 WHERE k = 'version'", [])
            .expect("claim an FTS5 format this build does not read");
        {
            let indexer = Indexer::open(&db).expect("the file itself still opens");
            assert_eq!(quick_check(&indexer.conn), Integrity::Undetermined);
            match indexer.search("alpha", 10) {
                Err(IndexerError::Sqlite { source, .. }) => assert_eq!(
                    source.sqlite_error_code(),
                    Some(rusqlite::ErrorCode::Unknown),
                    "the plain SQLITE_ERROR FTS5 reports: {source:?}"
                ),
                other => panic!("reported as the error it is, never Corrupt: {other:?}"),
            }
        }
        Indexer::recover(&mut db, &corpus, Timestamp::from_millis(7_000))
            .expect("recover replaces a file it never reads");
        let indexer = Indexer::open(&db).expect("the fresh index opens");
        assert_eq!(indexer.search("alpha", 10).expect("search").hits.len(), 1);
    }

    #[test]
    fn ori_t_0035_classify_calls_corrupt_exactly_what_the_check_calls_damaged() {
        // The rule itself, against one synthetic out-of-memory error: the
        // same error is Corrupt on a damaged file and itself on a healthy
        // one.
        let out_of_memory = || {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(7),
                Some("out of memory".to_owned()),
            )
        };
        let healthy = Indexer::open_in_memory().expect("an in-memory index");
        assert!(matches!(
            classify(&healthy.conn, Path::new("h"), "probe", out_of_memory()),
            IndexerError::Sqlite { .. }
        ));

        let scratch = Scratch::new("classify-rule");
        let db = product(&scratch);
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer
                .full_rebuild(&sweep_corpus())
                .expect("seed the corpus");
        }
        let file = index_file(&db);
        checkpoint(&file);
        let (page_size, pages_by_number) = page_map(&file);
        let content_leaf = pages_by_number
            .iter()
            .find(|(_, (name, kind))| name == "documents_content" && kind == "leaf")
            .map(|(page, _)| usize::try_from(*page).expect("a positive page number"))
            .expect("a documents_content leaf page");
        let mut damaged = fs::read(&file).expect("read the index");
        damaged[(content_leaf - 1) * page_size..content_leaf * page_size].fill(0x00);
        fs::write(&file, &damaged).expect("damage one page");
        let damaged_index = Indexer::open(&db).expect("page damage does not stop open");
        assert_eq!(quick_check(&damaged_index.conn), Integrity::Damaged);
        assert!(matches!(
            classify(&damaged_index.conn, &file, "probe", out_of_memory()),
            IndexerError::Corrupt { .. }
        ));
    }

    #[test]
    fn ori_t_0035_the_check_never_reads_a_stale_view_of_an_index_another_connection_changed() {
        // Found while building round 5, not by a review: FTS5 caches the
        // index structure per connection and refreshes it only when a
        // cursor opens, and PRAGMA quick_check's FTS5 step reads the cache
        // as it stands, so a bare quick_check on a connection another
        // connection had written past reported "checksum mismatch" on a
        // healthy index. quick_check opens a cursor first, in the same
        // snapshot.
        let scratch = Scratch::new("stale-check");
        let db = product(&scratch);
        let corpus: Vec<IndexableDocument> = (0..300)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    "needle body",
                )
            })
            .collect();
        let mut writer = Indexer::open(&db).expect("the writer opens");
        writer.full_rebuild(&corpus).expect("seed");
        let checker = Indexer::open(&db).expect("the checker opens");
        assert_eq!(quick_check(&checker.conn), Integrity::Intact);
        for round in 0..60 {
            writer
                .add_or_replace(&doc(
                    &format!("churn{}.md", round % 7),
                    DocumentKind::Section,
                    "C",
                    &format!("churn {round} words"),
                ))
                .expect("a write");
            if round % 3 == 0 {
                writer
                    .full_rebuild(&corpus[..(round % 50) + 1])
                    .expect("a rebuild");
            }
            assert_eq!(
                quick_check(&checker.conn),
                Integrity::Intact,
                "round {round}: a healthy index another connection just wrote is Intact"
            );
        }
    }

    // ---------------------------------------------------------------------
    // Item 3 (MEDIUM): text before a file's first heading is indexed.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_text_above_the_first_heading_stays_searchable_when_a_role_file_gains_its_first_heading()
     {
        // The review's edit: one heading appended to a role file, which is
        // front matter plus prose with no heading. Round 4 dropped every
        // line above that heading from the index, and every sync said Ok.
        let scratch = Scratch::new("preamble");
        let agents = scratch.path.join("spec").join("agents");
        fs::create_dir_all(&agents).expect("create spec/agents");
        let role = agents.join("lead.md");
        let before = "---\nname: lead\ndescription: The lead / reviewer, a different model than \
                      the coders.\n---\n\nYou are the **lead** in the Ori Studio fleet. Review \
                      is a checklist, never a summary.\n";
        fs::write(&role, before).expect("write the role file");
        let bare = concat!("spec/agents/", "lead", ".md");
        let anchored = concat!("spec/agents/", "lead", ".md", "#notes");
        let phrases = [
            "different model than the coders",
            "Review is a checklist, never a summary",
        ];

        let first = Indexer::collect_from_repo(&scratch.path).expect("collect");
        let first_paths: Vec<&str> = first.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(first_paths, vec![bare]);
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer.full_rebuild(&first).expect("full rebuild");
        for phrase in phrases {
            let hits = indexer.search(phrase, 10).expect("search").hits;
            assert_eq!(hits.len(), 1, "before the edit: {phrase:?}");
            assert_eq!(hits[0].path, bare);
        }

        fs::write(&role, format!("{before}\n## Notes\n\nA later note.\n"))
            .expect("append one heading");
        let second = Indexer::collect_from_repo(&scratch.path).expect("collect");
        let second_paths: Vec<&str> = second.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            second_paths,
            vec![bare, anchored],
            "the text above the heading keeps its bare path; the new section sits beside it"
        );
        let report = indexer
            .incremental_sync(&second)
            .expect("incremental sync after the edit");
        assert_eq!(
            report.removed, 0,
            "nothing the file still says may leave the index"
        );
        for phrase in phrases {
            let hits = indexer.search(phrase, 10).expect("search").hits;
            assert_eq!(
                hits.len(),
                1,
                "after the edit: {phrase:?} must still be found"
            );
            assert_eq!(hits[0].path, bare);
        }
        let later = indexer.search("later note", 10).expect("search").hits;
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].path, anchored);

        let mut rebuilt = Indexer::open_in_memory().expect("in-memory index opens");
        rebuilt.full_rebuild(&second).expect("full rebuild");
        assert_eq!(
            rebuilt.all_documents().expect("dump"),
            indexer.all_documents().expect("dump"),
            "and a full rebuild of the edited file agrees"
        );
    }

    /// Every regular `.md` file under `repo_root/spec`, as a repository
    /// relative, `/`-joined path, never through a link: the files
    /// [`Indexer::walk_repo`] reads, found independently of it, so a file
    /// the walk produced nothing for is still checked.
    fn markdown_files_to_index(repo_root: &Path) -> Vec<String> {
        fn visit(repo_root: &Path, dir: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).expect("list a directory") {
                let entry = entry.expect("a directory entry");
                let path = entry.path();
                let file_type = entry.file_type().expect("an entry's type");
                let relative: Vec<String> = path
                    .strip_prefix(repo_root)
                    .expect("under the root")
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy().into_owned())
                    .collect();
                if file_type.is_dir() {
                    visit(repo_root, &path, out);
                } else if file_type.is_file()
                    && path.extension().and_then(|extension| extension.to_str()) == Some("md")
                {
                    out.push(relative.join("/"));
                }
            }
        }
        let mut out = Vec::new();
        visit(repo_root, &repo_root.join("spec"), &mut out);
        out.sort();
        out
    }

    /// Asserts that every non-blank line of every file
    /// [`markdown_files_to_index`] finds, unless `walk` recorded the file as
    /// skipped, is a heading (some document's title) or sits in the body of
    /// some document from that file (a criterion's body is its row,
    /// trimmed). Returns how many files and lines were checked.
    fn assert_every_non_blank_line_is_indexed(repo_root: &Path, walk: &RepoWalk) -> (usize, usize) {
        let skipped: BTreeSet<&Path> = walk
            .skipped
            .iter()
            .map(|entry| entry.path.as_path())
            .collect();
        let (mut files, mut lines) = (0usize, 0usize);
        for file in markdown_files_to_index(repo_root) {
            if skipped.contains(repo_root.join(&file).as_path()) {
                continue;
            }
            let anchored = format!("{file}#");
            let documents: Vec<&IndexableDocument> = walk
                .documents
                .iter()
                .filter(|document| document.path == file || document.path.starts_with(&anchored))
                .collect();
            let text = fs::read_to_string(repo_root.join(&file)).expect("read a walked file");
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let as_title = line.trim_start().trim_start_matches('#').trim();
                let kept = documents.iter().any(|document| {
                    document.body.contains(line)
                        || document.title == as_title
                        || (document.kind == DocumentKind::Criterion
                            && document.body == line.trim())
                });
                assert!(
                    kept,
                    "{file}: this line is in no indexed document: {line:?} (documents from the \
                     file: {})",
                    documents.len()
                );
                lines += 1;
            }
            files += 1;
        }
        (files, lines)
    }

    #[test]
    fn ori_t_0035_no_non_blank_line_of_any_file_this_repositorys_walk_reads_is_left_out_of_the_index()
     {
        // Round 5's version of this check looked at Section documents only,
        // so it could not see that a criteria file kept nothing but its
        // table rows. Every file the walk reads, of every kind, now; a
        // file that produced no document at all fails on its first line.
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crates/ori-memory sits two levels under the repository root")
            .to_owned();
        let walk = Indexer::walk_repo(&repo_root).expect("walk this repository");
        let (files, checked) = assert_every_non_blank_line_is_indexed(&repo_root, &walk);
        // And every entry under spec/ is either read or recorded (round 7:
        // a file not named .md was neither).
        assert!(assert_every_entry_is_indexed_or_skipped(&repo_root, &walk) >= files);
        assert!(
            files > 20,
            "a vacuous walk would pass the check above for the wrong reason: {files} files"
        );
        assert!(
            walk.documents
                .iter()
                .any(|document| document.kind == DocumentKind::Criterion)
                && walk
                    .documents
                    .iter()
                    .any(|document| document.kind == DocumentKind::Adr),
            "the criteria and ADR files were among those checked"
        );
        assert!(checked > 1000, "only {checked} lines checked");
    }

    // ---------------------------------------------------------------------
    // Item 4: correctness residuals.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_incremental_sync_rewrites_any_stored_row_that_is_not_exactly_what_this_build_would_write()
     {
        // Each tamper is applied through a raw connection after a full
        // rebuild of `target`, and leaves the stored checksum alone. Round 4
        // skipped the first one forever (checksum equal, so "unchanged"),
        // failed every sync on the last two, and kept the duplicate.
        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("x.md", DocumentKind::Section, "X", "xray"),
        ];
        let tampers = [
            (
                "an unparseable kind, checksum unchanged",
                "UPDATE documents SET kind = 'prompt' WHERE path = 'x.md'",
            ),
            (
                "a kind in another case, checksum unchanged",
                "UPDATE documents SET kind = 'Section' WHERE path = 'x.md'",
            ),
            (
                "another valid kind, checksum unchanged",
                "UPDATE documents SET kind = 'adr' WHERE path = 'x.md'",
            ),
            (
                "a second row at the same path",
                "INSERT INTO documents (path, kind, title, body, checksum) \
                 VALUES ('x.md', 'prompt', 'X', 'xray', 7)",
            ),
            (
                "a checksum that is not an integer",
                "UPDATE documents SET checksum = 'deadbeef' WHERE path = 'x.md'",
            ),
            (
                "a row with no text path",
                "INSERT INTO documents (path, kind, title, body, checksum) \
                 VALUES (NULL, 'section', 'N', 'nothing', 1)",
            ),
        ];
        for (label, tamper) in tampers {
            let scratch = Scratch::new("tamper");
            let db = product(&scratch);
            let file = index_file(&db);
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&target).expect("full rebuild");
            let expected = raw_rows(&file);
            Connection::open(&file)
                .expect("a raw connection")
                .execute_batch(tamper)
                .expect("apply the tamper");
            assert_ne!(
                raw_rows(&file),
                expected,
                "{label}: the tamper must change what is stored, or this case proves nothing"
            );

            let first = indexer
                .incremental_sync(&target)
                .unwrap_or_else(|error| panic!("{label}: incremental_sync must succeed: {error}"));
            assert_eq!(
                raw_rows(&file),
                expected,
                "{label}: incremental_sync must leave exactly what full_rebuild stores"
            );
            assert!(first.upserted + first.removed >= 1, "{label}: {first:?}");
            assert_eq!(first.total, target.len(), "{label}");
            let second = indexer.incremental_sync(&target).expect("resync");
            assert_eq!(
                (second.upserted, second.removed),
                (0, 0),
                "{label}: and then it settles"
            );
            let hits = indexer.search("xray", 10).expect("search").hits;
            assert_eq!(hits.len(), 1, "{label}: x.md is searchable again");
            assert_eq!(hits[0].kind, DocumentKind::Section, "{label}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_the_walk_never_follows_a_symbolic_link_inside_spec_or_out_of_the_repository() {
        let scratch = Scratch::new("walk-symlinks");
        let spec = scratch.path.join("spec");
        let design = spec.join("design");
        fs::create_dir_all(design.join("screens")).expect("create spec/design/screens");
        fs::write(
            design.join("DESIGN.md"),
            "# Design\n\nLedgerline mock content\n",
        )
        .expect("write a design file");
        fs::write(
            design.join("screens").join("S.md"),
            "# Screen\n\nLedgerline screen mock\n",
        )
        .expect("write a nested design file");
        fs::write(spec.join("PRD.md"), "# PRD\n\nreal content\n").expect("write a real document");
        let outside = scratch.path.join("outside");
        fs::create_dir_all(&outside).expect("create a directory outside spec/");
        fs::write(
            outside.join("secret.md"),
            "# Secret\n\nprivate words outside spec\n",
        )
        .expect("write a file outside spec/");
        fs::create_dir_all(spec.join("runbooks")).expect("create spec/runbooks");

        let links = [
            (PathBuf::from("design"), spec.join("mockups")),
            (outside.clone(), spec.join("elsewhere")),
            (design.join("DESIGN.md"), spec.join("linked.md")),
            (
                PathBuf::from("../design/screens"),
                spec.join("runbooks").join("screens"),
            ),
        ];
        for (target, link) in &links {
            std::os::unix::fs::symlink(target, link).expect("create a symbolic link");
        }

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert!(
            walk.documents
                .iter()
                .any(|document| document.body.contains("real content")),
            "the real document is still collected: {walk:?}"
        );
        assert!(
            walk.documents
                .iter()
                .all(|document| !document.body.contains("private words")),
            "no link may bring anything outside spec/ into the index: {:?}",
            walk.documents
        );
        // spec/design/ is walked like the rest of spec/, so its files are
        // indexed once each, at their own paths, and never a second time
        // through a link to them.
        let design_text: Vec<&str> = walk
            .documents
            .iter()
            .filter(|document| document.body.contains("Ledgerline"))
            .map(|document| document_file(&document.path))
            .collect();
        assert_eq!(
            design_text,
            [
                ["spec", "design", "DESIGN.md"].join("/"),
                ["spec", "design", "screens", "S.md"].join("/"),
            ],
            "each linked-to file is indexed at its real path only: {:?}",
            walk.documents
        );
        let skipped_links: BTreeSet<PathBuf> = walk
            .skipped
            .iter()
            .filter(|entry| entry.reason == SkipReason::Symlink)
            .map(|entry| entry.path.clone())
            .collect();
        let expected: BTreeSet<PathBuf> = links.iter().map(|(_, link)| link.clone()).collect();
        assert_eq!(
            skipped_links, expected,
            "every link is reported as skipped, with its reason"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_a_spec_directory_that_is_itself_a_symbolic_link_is_never_walked() {
        let scratch = Scratch::new("spec-symlink");
        let real = scratch.path.join("somewhere-else");
        fs::create_dir_all(&real).expect("create the link target");
        fs::write(real.join("PRD.md"), "# PRD\n\nwords from outside\n").expect("write a file");
        let repo = scratch.path.join("repo");
        fs::create_dir_all(&repo).expect("create the repository root");
        std::os::unix::fs::symlink(&real, repo.join("spec")).expect("link spec/ elsewhere");

        // Round 6 returned an empty walk recording the link; round 7
        // refuses it, since an empty walk handed to a sync removes every
        // document (and collect_from_repo drops the record).
        match Indexer::walk_repo(&repo) {
            Err(IndexerError::NoSpecDirectory { path, reason }) => {
                assert_eq!(path, repo.join("spec"));
                assert!(reason.contains("symbolic link"), "{reason}");
            }
            other => panic!("a linked spec/ is never walked, and never an empty walk: {other:?}"),
        }
        assert!(matches!(
            Indexer::collect_from_repo(&repo),
            Err(IndexerError::NoSpecDirectory { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_a_backslash_in_a_unix_file_name_is_part_of_the_name_never_a_separator() {
        // Round 4 rewrote every backslash to a slash, so this file took the
        // real runbook's identity and DuplicatePath then refused indexing
        // the whole repository.
        let scratch = Scratch::new("backslash-name");
        let spec = scratch.path.join("spec");
        fs::create_dir_all(spec.join("runbooks")).expect("create spec/runbooks");
        fs::write(
            spec.join("runbooks").join("restore.md"),
            "# Restore\n\nreal steps\n",
        )
        .expect("write the real runbook");
        fs::write(
            spec.join("runbooks\\restore.md"),
            "# Restore\n\nimpostor steps\n",
        )
        .expect("write a file whose name holds a backslash");

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert!(
            walk.skipped.is_empty(),
            "neither file collides with the other: {walk:?}"
        );
        let real = concat!("spec/runbooks/restore", ".md", "#restore");
        let odd = concat!("spec/runbooks\\restore", ".md", "#restore");
        let paths: BTreeSet<&str> = walk.documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, BTreeSet::from([real, odd]));

        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&walk.documents)
            .expect("one odd file name never refuses the whole index");
        let found = indexer.search("real steps", 10).expect("search").hits;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, real);
        let found = indexer.search("impostor", 10).expect("search").hits;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, odd);
    }

    #[test]
    fn ori_t_0035_a_repeated_criterion_id_is_skipped_with_a_reason_and_never_refuses_the_whole_walk()
     {
        let scratch = Scratch::new("repeated-criterion");
        let criteria = scratch.path.join("spec").join("criteria");
        fs::create_dir_all(&criteria).expect("create spec/criteria");
        let table = criteria.join("phase-1.md");
        fs::write(
            &table,
            "| ID | Expected |\n|---|---|\n| ORI-P1-001 | first wording |\n\
             | ORI-P1-001 | second wording |\n| ORI-P1-002 | other |\n",
        )
        .expect("write a table repeating one ID");
        fs::write(
            scratch.path.join("spec").join("PRD.md"),
            "# PRD\n\nreal content\n",
        )
        .expect("write a real document");

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        let repeated = concat!("spec/criteria/phase-1", ".md", "#ORI-P1-001");
        assert_eq!(
            walk.skipped,
            vec![SkippedEntry {
                path: table,
                reason: SkipReason::DuplicateDocumentPath {
                    path: repeated.to_owned(),
                },
            }],
            "the repeat is left out and reported, naming its file"
        );
        let kept = walk
            .documents
            .iter()
            .find(|document| document.path == repeated)
            .expect("the first row is kept");
        assert!(kept.body.contains("first wording"));
        // Four documents: the two distinct rows, the PRD's section, and,
        // since round 6, the table's header and separator rows, which are
        // no criterion's and are indexed as the criteria file's own text
        // rather than dropped (the module doc's "What the repository walk
        // never reads").
        let paths: BTreeSet<&str> = walk
            .documents
            .iter()
            .map(|document| document.path.as_str())
            .collect();
        assert_eq!(
            paths,
            BTreeSet::from([
                repeated,
                concat!("spec/criteria/phase-1", ".md", "#ORI-P1-002"),
                concat!("spec/criteria/phase-1", ".md"),
                concat!("spec/PRD", ".md", "#prd"),
            ])
        );
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        let report = indexer
            .full_rebuild(&walk.documents)
            .expect("the walk's set never fails the rebuild wholesale");
        assert_eq!(report.total, 4);
    }

    #[test]
    fn ori_t_0035_one_unreadable_file_is_skipped_with_a_reason_and_never_fails_the_walk() {
        // Round 4 read every file with `?`, so one file that is not UTF-8
        // text failed the whole walk.
        let scratch = Scratch::new("unreadable-file");
        let spec = scratch.path.join("spec");
        fs::create_dir_all(&spec).expect("create spec/");
        fs::write(spec.join("PRD.md"), "# PRD\n\nreal content\n").expect("write a real document");
        let binary = spec.join("binary.md");
        fs::write(&binary, [0xFFu8, 0xFE, 0x00, 0x80]).expect("write bytes that are not UTF-8");

        let walk = Indexer::walk_repo(&scratch.path).expect("one odd file never fails the walk");
        assert_eq!(walk.documents.len(), 1);
        assert_eq!(walk.skipped.len(), 1);
        assert_eq!(walk.skipped[0].path, binary);
        assert!(matches!(
            walk.skipped[0].reason,
            SkipReason::Unreadable { .. }
        ));
    }

    #[test]
    fn ori_t_0035_the_most_expensive_query_within_the_byte_cap_stays_bounded_in_time_and_memory() {
        // The byte cap is the only bound on a query's cost now (the token
        // cap is gone). The most expensive shape found within it: a
        // one-letter term repeated 512 times (1024 bytes, the most terms
        // the cap admits), against documents that each repeat it 600
        // times, so the phrase matches every document and FTS5 walks every
        // position of every term. Run in a child process, because it caps
        // SQLite's heap for the whole process.
        const CHILD: &str = "ORI_T_0035_BOUNDED_QUERY_CHILD";
        const HEAP_LIMIT: u64 = 32 * 1024 * 1024;
        const TIME_LIMIT: Duration = Duration::from_secs(20);
        if std::env::var_os(CHILD).is_none() {
            run_in_child_process(
                "indexer::tests::ori_t_0035_the_most_expensive_query_within_the_byte_cap_stays_bounded_in_time_and_memory",
                CHILD,
                &[],
            );
            return;
        }

        let scratch = Scratch::new("bounded-query");
        let db = product(&scratch);
        let body = "a ".repeat(600);
        let corpus: Vec<IndexableDocument> = (0..300)
            .map(|n| doc(&format!("d{n}.md"), DocumentKind::Section, "T", &body))
            .collect();
        {
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&corpus).expect("seed");
        }
        let indexer = Indexer::open(&db).expect("reopen");
        let worst = "a ".repeat(MAX_QUERY_BYTES / 2);
        assert_eq!(worst.len(), MAX_QUERY_BYTES);

        let limit: i64 = indexer
            .conn
            .query_row(
                &format!("PRAGMA hard_heap_limit = {HEAP_LIMIT}"),
                [],
                |row| row.get(0),
            )
            .expect("cap SQLite's heap");
        assert_eq!(
            u64::try_from(limit).expect("a positive limit"),
            HEAP_LIMIT,
            "the cap took effect"
        );

        let started = std::time::Instant::now();
        let report = indexer
            .search(&worst, 10)
            .expect("the worst query within the byte cap runs inside the heap cap");
        let elapsed = started.elapsed();
        eprintln!("worst query within the byte cap: {elapsed:?}");
        assert_eq!(
            (report.hits.len(), report.documents_covered),
            (10, corpus.len()),
            "it really matched: the phrase was walked against every document"
        );
        assert!(
            elapsed < TIME_LIMIT,
            "the worst query within the byte cap took {elapsed:?}"
        );

        // Control: the same term 64 times past the cap. search refuses it,
        // so it goes to FTS5 directly, as an uncapped search would, and the
        // same heap cap stops it: the byte cap is what keeps a query inside.
        let over = "a ".repeat(32 * MAX_QUERY_BYTES);
        assert!(matches!(
            indexer.search(&over, 10),
            Err(IndexerError::QueryTooLarge { .. })
        ));
        let uncapped: rusqlite::Result<Vec<String>> = indexer
            .conn
            .prepare("SELECT path FROM documents WHERE documents MATCH ?1 LIMIT 10")
            .and_then(|mut statement| {
                statement
                    .query_map(params![quote_fts5_phrase(&over)], |row| row.get(0))?
                    .collect()
            });
        match uncapped {
            Err(error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::OutOfMemory) => {}
            other => panic!(
                "the control must exhaust the same heap cap, or the cap proves nothing: \
                 {other:?}"
            ),
        }
    }

    #[test]
    fn ori_t_0035_a_query_of_many_short_terms_within_the_byte_cap_is_accepted() {
        // The token cap refused this (512 terms against a cap of 64) and,
        // in the other direction, accepted 341 FTS5 terms spelled with
        // U+0336 as one. Now the byte length alone decides.
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&[doc("a.md", DocumentKind::Section, "A", "a a a")])
            .expect("full rebuild");
        let many = "a ".repeat(MAX_QUERY_BYTES / 2);
        assert_eq!(many.len(), MAX_QUERY_BYTES);
        indexer
            .search(&many, 10)
            .expect("512 short terms within the byte cap are accepted");
        let struck = "a\u{0336}".repeat(341);
        assert!(struck.len() <= MAX_QUERY_BYTES);
        indexer
            .search(&struck, 10)
            .expect("341 struck-through letters within the byte cap are accepted");
        let over = "a\u{0336}".repeat(342);
        assert!(over.len() > MAX_QUERY_BYTES);
        assert!(matches!(
            indexer.search(&over, 10),
            Err(IndexerError::QueryTooLarge { byte_len }) if byte_len == over.len()
        ));
    }

    #[test]
    fn ori_t_0035_index_report_total_is_read_before_commit_never_after_another_writers_commit() {
        // Deterministic: the after-commit seam (see `commit`) commits a
        // second connection's row at the exact moment this call's own
        // commit returns. Read before commit, `total` is the target's size;
        // read after, it would count the intruder too. The round-4 stress
        // test passed 20 of 20 against that mutant.
        let scratch = Scratch::new("total-window");
        let db = product(&scratch);
        let file = index_file(&db);
        let mut mine = Indexer::open(&db).expect("open on-disk index");
        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("b.md", DocumentKind::Section, "B", "beta"),
            doc("c.md", DocumentKind::Section, "C", "gamma"),
        ];
        for method in ["full_rebuild", "incremental_sync"] {
            mine.full_rebuild(&[]).expect("start empty");
            let fired = std::rc::Rc::new(std::cell::Cell::new(false));
            let hook_fired = std::rc::Rc::clone(&fired);
            let intruder_file = file.clone();
            set_after_commit_hook(move || {
                Connection::open(&intruder_file)
                    .expect("the intruder's own connection")
                    .execute(
                        "INSERT INTO documents (path, kind, title, body, checksum) \
                         VALUES ('intruder.md', 'section', 'I', 'intruder', 1)",
                        [],
                    )
                    .expect("the intruder commits");
                hook_fired.set(true);
            });
            let report = if method == "full_rebuild" {
                mine.full_rebuild(&target)
            } else {
                mine.incremental_sync(&target)
            }
            .expect("the call succeeds");
            assert!(
                fired.get(),
                "{method}: the intruder must have committed, or this proves nothing"
            );
            assert_eq!(
                report.total,
                target.len(),
                "{method}: total is read inside the transaction, before commit, never after \
                 another writer's commit"
            );
            assert_eq!(
                mine.all_documents().expect("read back").len(),
                target.len() + 1,
                "{method}: the intruder's commit really did land right after this one"
            );
        }
    }

    // =====================================================================
    // Round 6: seven items an adversarial review confirmed by independent
    // reproduction. Each test below fails against the code before its fix.
    // =====================================================================

    /// Opens a second product beside `scratch`'s own, under the same root.
    fn other_product(scratch: &Scratch, id: &str) -> ProductDb {
        ProductDb::open(&scratch.path, id, Timestamp::from_millis(1_000))
            .expect("a second scratch product opens")
    }

    // ---------------------------------------------------------------------
    // Item 1 (HIGH): a link let one product's Indexer hold another's file.
    // ---------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_open_refuses_an_index_linked_into_another_products_index() {
        // The review's setup: product A's index/ (or its fts.sqlite) is a
        // link to product B's. Round 5's open followed it, so A's Indexer,
        // borrowing only A's ProductDb, held B's file open; recover(&mut B)
        // compiled, moved that file into quarantine, and A's later writes
        // returned Ok into the quarantined file. Now A's open is refused, so
        // no Indexer can hold a file its own borrow says nothing about.
        let bravo = vec![doc("b.md", DocumentKind::Section, "B", "bravo words")];
        for variant in ["index directory", "index file", "write-ahead log"] {
            let scratch = Scratch::new("open-link");
            let alpha = other_product(&scratch, "PRODUCT-T35-A");
            let mut beta = other_product(&scratch, "PRODUCT-T35-B");
            Indexer::open(&beta)
                .expect("B opens its own index")
                .full_rebuild(&bravo)
                .expect("B builds its index");
            checkpoint(&index_file(&beta));
            let beta_bytes = fs::read(index_file(&beta)).expect("read B's index");
            let (link, target) = match variant {
                "index directory" => {
                    fs::remove_dir_all(index_dir(&alpha)).expect("remove A's index/");
                    (index_dir(&alpha), index_dir(&beta))
                }
                "index file" => (index_file(&alpha), index_file(&beta)),
                _ => (
                    index_dir(&alpha).join("fts.sqlite-wal"),
                    index_dir(&beta).join("fts.sqlite-wal"),
                ),
            };
            std::os::unix::fs::symlink(&target, &link).expect("link A's entry into B's index");

            match Indexer::open(&alpha) {
                Err(error @ IndexerError::OpenRefused { .. }) => {
                    assert_eq!(error.methodology_ref().section, 25);
                    let IndexerError::OpenRefused { path, .. } = &error else {
                        unreachable!()
                    };
                    assert_eq!(
                        path.file_name(),
                        link.file_name(),
                        "{variant}: the refusal names the link: {error}"
                    );
                }
                Err(other) => panic!("{variant}: refused for the wrong reason: {other:?}"),
                Ok(_) => panic!("{variant}: A's Indexer must never open through a link"),
            }
            assert_eq!(
                fs::read(index_file(&beta)).expect("read B's index again"),
                beta_bytes,
                "{variant}: B's file is untouched"
            );

            // A link at A's own index file is A's to move: recover moves
            // the link itself into A's quarantine, never what it points at,
            // and A then has a real index of its own. A linked index/ is
            // refused by recover too (its own test, above).
            if variant == "index file" {
                let mut alpha = alpha;
                let report = Indexer::recover(
                    &mut alpha,
                    &[doc("a.md", DocumentKind::Section, "A", "alpha words")],
                    Timestamp::from_millis(9),
                )
                .expect("recover moves A's link aside");
                let moved = report
                    .quarantine
                    .expect("the link was moved")
                    .join(INDEX_FILE);
                assert!(
                    fs::symlink_metadata(&moved)
                        .expect("the moved entry")
                        .file_type()
                        .is_symlink(),
                    "the link itself was moved, not the file behind it"
                );
                let a = Indexer::open(&alpha).expect("A now opens a real index of its own");
                assert_eq!(a.search("alpha", 10).expect("search").hits.len(), 1);
                assert_eq!(a.search("bravo", 10).expect("search").hits.len(), 0);
            }
            assert_eq!(
                fs::read(index_file(&beta)).expect("read B's index again"),
                beta_bytes,
                "{variant}: B's file is still untouched"
            );
            assert!(!index_dir(&beta).join(QUARANTINE_DIR).exists());
            let b = Indexer::open(&beta).expect("B still opens");
            assert_eq!(
                b.search("bravo", 10).expect("search").hits.len(),
                1,
                "{variant}: B's index is intact"
            );
            drop(b);
            Indexer::recover(&mut beta, &bravo, Timestamp::from_millis(10))
                .expect("and B, whose borrow nothing else holds, can still recover its own");
        }
    }

    // ---------------------------------------------------------------------
    // Item 2 (MEDIUM): a relative product directory re-resolved later.
    // ---------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_open_and_recover_refuse_a_product_directory_given_as_a_relative_path() {
        // The review changed the working directory between ProductDb::open
        // and recover, and recover moved and rebuilt a different, separately
        // locked product's live index. The working directory is process-wide,
        // so this test never changes it: it reaches its scratch product
        // through a relative path from wherever the test runs, which is the
        // precondition the review's reproduction needed.
        let scratch = Scratch::new("relative-dir");
        let corpus = vec![doc("a.md", DocumentKind::Section, "A", "alpha words")];
        let file = {
            let db = product(&scratch);
            Indexer::open(&db)
                .expect("open through the absolute path")
                .full_rebuild(&corpus)
                .expect("seed");
            checkpoint(&index_file(&db));
            index_file(&db)
        };
        let before = fs::read(&file).expect("read the index");

        let cwd = std::env::current_dir().expect("the working directory");
        let mut relative = PathBuf::new();
        for _ in cwd.components() {
            relative.push("..");
        }
        relative.push(
            scratch
                .path
                .strip_prefix("/")
                .expect("an absolute scratch path"),
        );
        assert!(relative.is_relative());
        let mut db = ProductDb::open(&relative, PRODUCT, Timestamp::from_millis(2_000))
            .expect("ProductDb accepts a relative root");
        assert!(db.dir().is_relative(), "the precondition: {:?}", db.dir());

        assert!(
            matches!(Indexer::open(&db), Err(IndexerError::OpenRefused { .. })),
            "open refuses a relative product directory"
        );
        match Indexer::recover(&mut db, &corpus, Timestamp::from_millis(3)) {
            Err(error @ IndexerError::RecoveryRefused { .. }) => {
                assert!(error.to_string().contains("relative"), "{error}");
            }
            other => panic!("recover refuses a relative product directory: {other:?}"),
        }
        assert_eq!(
            fs::read(&file).expect("the index is still at its live path"),
            before,
            "nothing was moved or rebuilt"
        );
        assert!(
            !scratch
                .path
                .join(PRODUCT)
                .join(INDEX_DIR)
                .join(QUARANTINE_DIR)
                .exists()
        );
    }

    // ---------------------------------------------------------------------
    // Item 3 (MEDIUM): no per-document cap, so no bound on a query's cost.
    // ---------------------------------------------------------------------

    /// A document whose `title` and `body` are exactly `byte_len` bytes
    /// together, its body the one-letter term `a` as densely as it fits:
    /// the most positions of one term a document of that size can hold.
    fn dense_document(path: &str, byte_len: usize) -> IndexableDocument {
        let mut body = "a ".repeat((byte_len - 1) / 2);
        while body.len() < byte_len - 1 {
            body.push('a');
        }
        let document = doc(path, DocumentKind::Section, "T", &body);
        assert_eq!(document.indexed_byte_len(), byte_len);
        document
    }

    #[test]
    fn ori_t_0035_every_writer_refuses_a_document_over_the_size_cap_and_the_walk_skips_it_with_a_reason()
     {
        let at_cap = dense_document("at-cap.md", MAX_DOCUMENT_BYTES);
        let over = dense_document("over.md", MAX_DOCUMENT_BYTES + 1);
        let refused = |result: Result<(), IndexerError>, call: &str| match result {
            Err(IndexerError::DocumentTooLarge { path, byte_len }) => {
                assert_eq!(
                    (path.as_str(), byte_len),
                    ("over.md", MAX_DOCUMENT_BYTES + 1)
                );
            }
            other => panic!("{call} must refuse a document over the cap: {other:?}"),
        };

        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(std::slice::from_ref(&at_cap))
            .expect("a document exactly at the cap is stored");
        let both = [at_cap.clone(), over.clone()];
        refused(indexer.full_rebuild(&both).map(|_| ()), "full_rebuild");
        refused(
            indexer.incremental_sync(&both).map(|_| ()),
            "incremental_sync",
        );
        refused(indexer.add_or_replace(&over), "add_or_replace");
        assert_eq!(
            indexer.all_documents().expect("dump").len(),
            1,
            "a refusal writes nothing"
        );
        let scratch = Scratch::new("recover-over-cap");
        let mut db = product(&scratch);
        Indexer::open(&db)
            .expect("open")
            .full_rebuild(std::slice::from_ref(&at_cap))
            .expect("seed");
        refused(
            Indexer::recover(&mut db, &both, Timestamp::from_millis(1)).map(|_| ()),
            "recover",
        );
        assert!(
            !index_dir(&db).join(QUARANTINE_DIR).exists(),
            "recover refuses before moving anything"
        );

        // The walk: a file over the file cap is never read; a document over
        // the document cap is left out and its file's other documents kept;
        // a long file of short sections is indexed whole.
        let repo = Scratch::new("walk-caps");
        let spec = repo.path.join("spec");
        fs::create_dir_all(spec.join("adr")).expect("create spec/adr");
        fs::create_dir_all(spec.join("runbooks")).expect("create spec/runbooks");
        let huge = spec.join("huge.md");
        let file_cap = usize::try_from(MAX_FILE_BYTES).expect("fits");
        fs::write(&huge, "a".repeat(file_cap + 1))
            .expect("write a file one byte over the file cap");
        let adr = spec.join("adr").join("ADR-9999-long.md");
        fs::write(
            &adr,
            format!("# ADR-9999\n\n{}\n", "b ".repeat(MAX_DOCUMENT_BYTES / 2)),
        )
        .expect("write an ADR over the document cap");
        let big_section = spec.join("runbooks").join("mixed.md");
        let long_section: String = "c ".repeat(MAX_DOCUMENT_BYTES / 2);
        fs::write(
            &big_section,
            format!("# Short\n\nkept words\n\n# Long\n\n{long_section}\n"),
        )
        .expect("write a file with one section over the document cap");
        let many = spec.join("runbooks").join("many.md");
        let mut many_text = String::new();
        for n in 0..40 {
            many_text.push_str(&format!("# Part {n}\n\n{}\n\n", "d ".repeat(1000)));
        }
        assert!(many_text.len() > MAX_DOCUMENT_BYTES);
        fs::write(&many, &many_text).expect("write a long file of short sections");

        let walk = Indexer::walk_repo(&repo.path).expect("walk");
        let adr_path = concat!("spec/adr/", "ADR-9999-long", ".md");
        let long_path = concat!("spec/runbooks/", "mixed", ".md", "#long");
        let reasons: BTreeMap<String, SkipReason> = walk
            .skipped
            .iter()
            .map(|entry| {
                (
                    entry
                        .path
                        .strip_prefix(&repo.path)
                        .expect("under the repository")
                        .to_string_lossy()
                        .into_owned(),
                    entry.reason.clone(),
                )
            })
            .collect();
        assert_eq!(reasons.len(), 3, "{:?}", walk.skipped);
        assert!(matches!(
            reasons[concat!("spec/huge", ".md")],
            SkipReason::FileTooLarge { byte_len } if byte_len == MAX_FILE_BYTES + 1
        ));
        assert!(matches!(
            &reasons[adr_path],
            SkipReason::DocumentTooLarge { path, byte_len }
                if path == adr_path && *byte_len > MAX_DOCUMENT_BYTES
        ));
        assert!(matches!(
            &reasons[concat!("spec/runbooks/", "mixed", ".md")],
            SkipReason::DocumentTooLarge { path, .. } if path == long_path
        ));
        let paths: BTreeSet<&str> = walk.documents.iter().map(|d| d.path.as_str()).collect();
        assert!(paths.contains(concat!("spec/runbooks/", "mixed", ".md", "#short")));
        assert_eq!(
            paths
                .iter()
                .filter(|path| path.starts_with(concat!("spec/runbooks/", "many", ".md#")))
                .count(),
            40,
            "the cap is per document: a long file of short sections is indexed whole"
        );
        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer
            .full_rebuild(&walk.documents)
            .expect("the walk never hands a writer a document it refuses");
    }

    #[test]
    fn ori_t_0035_the_worst_query_within_the_byte_cap_against_a_document_at_the_size_cap_stays_bounded()
     {
        // The review's case: 512 repetitions of a one-letter term (1,023
        // bytes) against one 2 MB document of that term, about 3 seconds
        // and 1.09 GB of SQLite heap. The document cap is what bounds it:
        // at the cap, the same query is measured (release) at 72.6 MB of
        // heap; here it must finish inside a 96 MiB heap cap and the stated
        // time, while the same query against a document four times the cap
        // (measured at 264.8 MB), which only a raw insert can store,
        // exhausts that same heap cap.
        const CHILD: &str = "ORI_T_0035_DOCUMENT_CAP_QUERY_CHILD";
        const HEAP_LIMIT: u64 = 96 * 1024 * 1024;
        const TIME_LIMIT: Duration = Duration::from_secs(20);
        if std::env::var_os(CHILD).is_none() {
            run_in_child_process(
                "indexer::tests::ori_t_0035_the_worst_query_within_the_byte_cap_against_a_document_at_the_size_cap_stays_bounded",
                CHILD,
                &[],
            );
            return;
        }

        let scratch = Scratch::new("document-cap-query");
        let db = product(&scratch);
        let at_cap = dense_document("at-cap.md", MAX_DOCUMENT_BYTES);
        Indexer::open(&db)
            .expect("open")
            .full_rebuild(std::slice::from_ref(&at_cap))
            .expect("a document at the cap is stored");
        let four_times = dense_document("four-times.md", 4 * MAX_DOCUMENT_BYTES);
        let mut indexer = Indexer::open(&db).expect("reopen");
        assert!(matches!(
            indexer.add_or_replace(&four_times),
            Err(IndexerError::DocumentTooLarge { .. })
        ));
        let worst = "a ".repeat(MAX_QUERY_BYTES / 2);
        let limit: i64 = indexer
            .conn
            .query_row(
                &format!("PRAGMA hard_heap_limit = {HEAP_LIMIT}"),
                [],
                |row| row.get(0),
            )
            .expect("cap SQLite's heap");
        assert_eq!(u64::try_from(limit).expect("positive"), HEAP_LIMIT);

        let started = std::time::Instant::now();
        let report = indexer
            .search(&worst, 10)
            .expect("the worst query within the byte cap runs inside the heap cap");
        let elapsed = started.elapsed();
        eprintln!("worst query against a document at the size cap: {elapsed:?}");
        assert_eq!((report.hits.len(), report.documents_covered), (1, 1));
        assert!(elapsed < TIME_LIMIT, "took {elapsed:?}");

        // Control: past the cap, stored the only way it still can be.
        Connection::open(index_file(&db))
            .expect("a raw connection")
            .execute(
                "INSERT INTO documents (path, kind, title, body, checksum) VALUES (?1, ?2, ?3, ?4, 1)",
                params![
                    four_times.path,
                    four_times.kind.as_str(),
                    four_times.title,
                    four_times.body
                ],
            )
            .expect("store a document four times the cap behind the writers' backs");
        let uncapped: rusqlite::Result<Vec<String>> = indexer
            .conn
            .prepare(
                "SELECT path FROM documents WHERE documents MATCH ?1 AND path = 'four-times.md'",
            )
            .and_then(|mut statement| {
                statement
                    .query_map(params![quote_fts5_phrase(&worst)], |row| row.get(0))?
                    .collect()
            });
        match uncapped {
            Err(error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::OutOfMemory) => {}
            other => panic!(
                "the control must exhaust the same heap cap, or the cap proves nothing: {other:?}"
            ),
        }
    }

    // ---------------------------------------------------------------------
    // Item 4 (MEDIUM): the heading-anchor search was quadratic.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_a_file_of_one_heading_repeated_to_the_file_cap_splits_within_a_stated_bound() {
        // The review: 32,768 identical headings (128 KB) took 25.6 s, and
        // 1 MB about 27 minutes by extrapolation. This is the worst file the
        // walk reads: exactly the file cap, one four-byte heading repeated
        // 262,144 times. The bound, 20 s in an unoptimized build on a loaded
        // machine, is far above what the linear search needs and far below
        // what the quadratic one did; each run is on a thread, so a
        // regression fails at the bound instead of running for minutes.
        // Since round 7 a walk splits at most MAX_WALK_DOCUMENTS documents,
        // and since round 8 one file at most MAX_FILE_DOCUMENTS, so the
        // whole file is split here by the splitter itself, and the walk,
        // whose split stops at the per-file cap, records the file instead,
        // for itself.
        const BOUND: Duration = Duration::from_secs(20);
        let scratch = Scratch::new("repeated-headings");
        let runbooks = scratch.path.join("spec").join("runbooks");
        fs::create_dir_all(&runbooks).expect("create spec/runbooks");
        let file_cap = usize::try_from(MAX_FILE_BYTES).expect("fits");
        let headings = file_cap / 4;
        let text = "# a\n".repeat(headings);
        fs::write(runbooks.join("repeat.md"), &text).expect("write a file of one repeated heading");

        let prefix = concat!("spec/runbooks/", "repeat", ".md");
        let (sender, receiver) = std::sync::mpsc::channel();
        let started = std::time::Instant::now();
        std::thread::spawn(move || {
            let _ = sender.send(section_documents(prefix, &text, usize::MAX));
        });
        let documents = receiver
            .recv_timeout(BOUND)
            .unwrap_or_else(|_| {
                panic!("{headings} repeated headings did not split within {BOUND:?}")
            })
            .expect("no limit");
        eprintln!(
            "{headings} repeated headings split in {:?}",
            started.elapsed()
        );
        assert_eq!(documents.len(), headings);
        assert_eq!(documents[0].path, format!("{prefix}#a"));
        assert_eq!(documents[1].path, format!("{prefix}#a-1"));
        assert_eq!(
            documents[headings - 1].path,
            format!("{prefix}#a-{}", headings - 1)
        );
        let distinct: std::collections::HashSet<&str> =
            documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(distinct.len(), headings, "every anchor is distinct");

        let root = scratch.path.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(Indexer::walk_repo(&root));
        });
        let walk = receiver
            .recv_timeout(BOUND)
            .unwrap_or_else(|_| panic!("the walk did not finish within {BOUND:?}"))
            .expect("walk");
        assert!(walk.documents.is_empty(), "{}", walk.documents.len());
        assert_eq!(
            walk.skipped,
            vec![SkippedEntry {
                path: runbooks.join("repeat.md"),
                reason: SkipReason::FileDocumentLimit {
                    limit: MAX_FILE_DOCUMENTS
                },
            }]
        );
    }

    // ---------------------------------------------------------------------
    // Item 5 (MEDIUM): content left out with no record.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_every_line_of_a_criteria_file_is_indexed_including_prose_a_template_form_criterion_and_a_file_of_another_shape()
     {
        // The review's three cases: a criteria file's title, format line and
        // "proposed criteria" prose; a proposed criterion appended in the
        // acceptance-criterion template's two-column form; and a whole
        // criteria file of another shape (a personas file), which produced
        // no document at all. Round 5 kept only rows starting with an ID.
        let scratch = Scratch::new("criteria-prose");
        let criteria = scratch.path.join("spec").join("criteria");
        fs::create_dir_all(&criteria).expect("create spec/criteria");
        fs::write(
            criteria.join("phase-1.md"),
            "# Acceptance criteria: Phase 1\n\n\
             Format per the template. Identifier `ORI-P1-nnn`, formatmarker.\n\n\
             | ID | Type | Expected result |\n|---|---|---|\n\
             | ORI-P1-001 | F | first result |\n| ORI-P1-002 | F | second result |\n\n\
             Proposed criteria are appended below this line, proposalmarker.\n\n\
             | Field | Value |\n|---|---|\n| ID | ORI-P1-050 |\n| Type | F |\n\
             | Precondition | templatemarker |\n\n\
             ```text\n| ORI-P1-099 | fenced | not a criterion |\n```\n",
        )
        .expect("write a criteria file with prose");
        let personas = format!("{}.md", ["per", "sonas"].concat());
        fs::write(
            criteria.join(&personas),
            "# Personas\n\nWho the product is for, personamarker.\n\n## Solo operator\n\n\
             Runs one product alone, solomarker.\n\n| Persona | Goal |\n|---|---|\n\
             | Operator | tablemarker |\n",
        )
        .expect("write a criteria file of another shape");

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert!(walk.skipped.is_empty(), "{:?}", walk.skipped);
        let (files, lines) = assert_every_non_blank_line_is_indexed(&scratch.path, &walk);
        assert_eq!(files, 2);
        assert!(lines > 20, "only {lines} lines checked");
        let criteria_ids: BTreeSet<&str> = walk
            .documents
            .iter()
            .filter(|d| d.kind == DocumentKind::Criterion)
            .map(|d| d.title.as_str())
            .collect();
        assert_eq!(
            criteria_ids,
            BTreeSet::from(["ORI-P1-001", "ORI-P1-002"]),
            "rows starting with an ID are criteria; the fenced row is text"
        );

        let mut indexer = Indexer::open_in_memory().expect("in-memory index opens");
        indexer.full_rebuild(&walk.documents).expect("rebuild");
        let personas_prefix = format!("spec/criteria/{personas}");
        for (marker, file) in [
            ("formatmarker", concat!("spec/criteria/phase-1", ".md")),
            ("proposalmarker", concat!("spec/criteria/phase-1", ".md")),
            ("templatemarker", concat!("spec/criteria/phase-1", ".md")),
            ("ORI-P1-050", concat!("spec/criteria/phase-1", ".md")),
            ("fenced", concat!("spec/criteria/phase-1", ".md")),
            ("personamarker", personas_prefix.as_str()),
            ("solomarker", personas_prefix.as_str()),
            ("tablemarker", personas_prefix.as_str()),
        ] {
            let hits = indexer.search(marker, 10).expect("search").hits;
            assert_eq!(hits.len(), 1, "{marker}: {hits:?}");
            assert!(hits[0].path.starts_with(file), "{marker}: {hits:?}");
            assert_eq!(hits[0].kind, DocumentKind::Section, "{marker}");
        }
        let resync = indexer
            .incremental_sync(&walk.documents)
            .expect("an immediate resync");
        assert_eq!((resync.upserted, resync.removed), (0, 0));
    }

    // ---------------------------------------------------------------------
    // Item 6 (LOW): a move failing part way split the database from its log.
    // ---------------------------------------------------------------------

    /// Plants, as `db`'s live index, a database file and a write-ahead log
    /// whose committed frames hold one document (`walword`) the database
    /// file alone does not, and returns the planted bytes of each. Checks
    /// both halves of that on copies, never on the planted files, since
    /// closing a connection on them would checkpoint the log away.
    fn plant_an_index_whose_log_holds_a_commit(
        db: &ProductDb,
        scratch: &Scratch,
    ) -> (Vec<u8>, Vec<u8>) {
        let held = scratch.path.join("held");
        fs::create_dir_all(&held).expect("create a holding directory");
        {
            let mut writer = Indexer::open(db).expect("open on-disk index");
            let five: Vec<IndexableDocument> = (0..5)
                .map(|n| doc(&format!("d{n}.md"), DocumentKind::Section, "D", "alpha"))
                .collect();
            writer.full_rebuild(&five).expect("seed");
            writer
                .conn
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint = 0")
                .expect("checkpoint, then keep every later commit in the log");
            writer
                .add_or_replace(&doc("w.md", DocumentKind::Section, "W", "walword"))
                .expect("a commit only the log holds");
            for name in [INDEX_FILE, "fts.sqlite-wal"] {
                fs::copy(index_dir(db).join(name), held.join(name)).expect("copy a live file");
            }
        }
        for name in INDEX_FILE_SET {
            let _ = fs::remove_file(index_dir(db).join(name));
        }
        for name in [INDEX_FILE, "fts.sqlite-wal"] {
            fs::copy(held.join(name), index_dir(db).join(name)).expect("plant a live file");
        }
        let walword_rows = |files: &[&str]| -> i64 {
            let probe = scratch.path.join(format!("probe-{}", files.len()));
            fs::create_dir_all(&probe).expect("create a probe directory");
            for name in files {
                fs::copy(held.join(name), probe.join(name)).expect("copy for the probe");
            }
            Connection::open(probe.join(INDEX_FILE))
                .expect("open the probe")
                .query_row(
                    "SELECT count(*) FROM documents_content WHERE c3 = 'walword'",
                    [],
                    |row| row.get(0),
                )
                .expect("count the rows holding walword")
        };
        assert_eq!(
            walword_rows(&[INDEX_FILE]),
            0,
            "the database alone lacks it"
        );
        assert_eq!(
            walword_rows(&[INDEX_FILE, "fts.sqlite-wal"]),
            1,
            "the database with its log has it"
        );
        (
            fs::read(held.join(INDEX_FILE)).expect("read the planted database"),
            fs::read(held.join("fts.sqlite-wal")).expect("read the planted log"),
        )
    }

    /// Every entry under `db`'s `index/quarantine/`, recursively, as paths
    /// relative to it; empty when the directory is absent.
    fn quarantine_contents(db: &ProductDb) -> Vec<String> {
        let root = index_dir(db).join(QUARANTINE_DIR);
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries {
                let directory = entry.expect("an entry").path();
                out.push(
                    directory
                        .strip_prefix(&root)
                        .expect("under the root")
                        .to_string_lossy()
                        .into_owned(),
                );
                for file in fs::read_dir(&directory).expect("list a quarantine directory") {
                    out.push(
                        file.expect("an entry")
                            .path()
                            .strip_prefix(&root)
                            .expect("under the root")
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        out.sort();
        out
    }

    /// Asserts a failed recover left the planted pair exactly where it was,
    /// and its quarantine directory in place and empty, then recovers again
    /// and asserts both went into one directory, a new one.
    fn assert_rolled_back_then_recovered_together(
        db: &mut ProductDb,
        planted: &(Vec<u8>, Vec<u8>),
        result: Result<RecoveryReport, IndexerError>,
    ) {
        let failed = match &result {
            Err(IndexerError::QuarantineIncomplete {
                path,
                stranded,
                quarantine,
                ..
            }) => {
                assert_eq!(path.file_name(), Some(std::ffi::OsStr::new(INDEX_FILE)));
                assert!(
                    stranded.is_empty(),
                    "every moved file was put back: {stranded:?}"
                );
                quarantine.clone()
            }
            other => panic!("a failed move is reported as such: {other:?}"),
        };
        let message = result.expect_err("checked above").to_string();
        assert!(message.contains("put back"), "{message}");
        assert_eq!(
            fs::read(index_file(db)).expect("the database is at its live path"),
            planted.0
        );
        assert_eq!(
            fs::read(index_dir(db).join("fts.sqlite-wal")).expect("so is its log"),
            planted.1,
            "the log was moved back beside its database, byte for byte"
        );
        let failed_name = failed
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            quarantine_contents(db),
            vec![failed_name.clone()],
            "no quarantine directory is left holding half the evidence: the failed \
             recovery's own is left in place, empty"
        );

        let report = Indexer::recover(db, &[], Timestamp::from_millis(8))
            .expect("once the obstacle is gone, recover succeeds");
        let quarantine = report.quarantine.expect("the pair was moved");
        assert_ne!(quarantine, failed, "into a directory of its own");
        assert_eq!(
            fs::read(quarantine.join(INDEX_FILE)).expect("the database is in quarantine"),
            planted.0
        );
        assert_eq!(
            fs::read(quarantine.join("fts.sqlite-wal")).expect("beside its log"),
            planted.1
        );
        let contents = quarantine_contents(db);
        assert_eq!(contents.len(), 4, "{contents:?}");
        assert!(
            !contents
                .iter()
                .any(|entry| entry.starts_with(&format!("{failed_name}/"))),
            "the failed recovery's directory still holds nothing: {contents:?}"
        );
    }

    #[test]
    fn ori_t_0035_a_rolled_back_recoverys_quarantine_directory_name_is_never_reused() {
        // The review's reproduction: a recovery whose move of the database
        // fails is rolled back, and a second recovery handed the same
        // recovered_at succeeds. Round 8 removed the first one's empty
        // directory, so the second made exactly that name again and filled
        // it: the path the first error named held other evidence.
        let scratch = Scratch::new("quarantine-no-reuse");
        let mut db = product(&scratch);
        let planted = plant_an_index_whose_log_holds_a_commit(&db, &scratch);
        let at = Timestamp::from_millis(7);
        let result = {
            let _failing = fail_moves(|from| {
                from.file_name() == Some(std::ffi::OsStr::new(INDEX_FILE))
                    && from.parent().and_then(Path::file_name)
                        == Some(std::ffi::OsStr::new(INDEX_DIR))
            });
            Indexer::recover(&mut db, &[], at)
        };
        let Err(IndexerError::QuarantineIncomplete {
            quarantine: failed,
            stranded,
            ..
        }) = &result
        else {
            panic!("a failed move is reported as such: {result:?}");
        };
        assert!(stranded.is_empty(), "{stranded:?}");
        assert!(
            failed.is_dir(),
            "the failed recovery's directory is left in place: {}",
            failed.display()
        );
        assert_eq!(
            fs::read_dir(failed).expect("list it").count(),
            0,
            "and it is empty, every file having gone back"
        );

        let report = Indexer::recover(&mut db, &[], at)
            .expect("the same recovery, at the same time, once the obstacle is gone");
        let quarantine = report.quarantine.expect("the pair was moved");
        assert_ne!(
            &quarantine, failed,
            "a name a rolled-back recovery made is never made again"
        );
        assert_eq!(
            fs::read_dir(failed).expect("list it again").count(),
            0,
            "nothing is ever merged into the failed recovery's directory"
        );
        let name = |path: &Path| path.file_name().expect("a name").to_owned();
        assert!(
            name(failed) < name(&quarantine),
            "and the names still sort in the order the recoveries happened"
        );
        assert_eq!(
            fs::read(quarantine.join(INDEX_FILE)).expect("the database is in quarantine"),
            planted.0
        );
        assert_eq!(
            fs::read(quarantine.join("fts.sqlite-wal")).expect("beside its log"),
            planted.1
        );
    }

    #[test]
    fn ori_t_0035_a_move_that_fails_part_way_is_rolled_back_so_the_database_and_its_log_stay_together()
     {
        let scratch = Scratch::new("move-rolled-back");
        let mut db = product(&scratch);
        let planted = plant_an_index_whose_log_holds_a_commit(&db, &scratch);
        let result = {
            let _failing = fail_moves(|from| {
                from.file_name() == Some(std::ffi::OsStr::new(INDEX_FILE))
                    && from.parent().and_then(Path::file_name)
                        == Some(std::ffi::OsStr::new(INDEX_DIR))
            });
            Indexer::recover(&mut db, &[], Timestamp::from_millis(7))
        };
        assert_rolled_back_then_recovered_together(&mut db, &planted, result);
    }

    #[test]
    fn ori_t_0035_a_file_that_cannot_be_moved_back_is_named_exactly_and_nothing_is_rebuilt() {
        let scratch = Scratch::new("move-stranded");
        let mut db = product(&scratch);
        let planted = plant_an_index_whose_log_holds_a_commit(&db, &scratch);
        let result = {
            let _failing = fail_moves(|from| {
                let name = from.file_name().and_then(std::ffi::OsStr::to_str);
                let parent = from.parent().and_then(Path::file_name);
                let grandparent = from
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name);
                (name == Some(INDEX_FILE) && parent == Some(std::ffi::OsStr::new(INDEX_DIR)))
                    || (name == Some("fts.sqlite-wal")
                        && grandparent == Some(std::ffi::OsStr::new(QUARANTINE_DIR)))
            });
            Indexer::recover(&mut db, &[], Timestamp::from_millis(7))
        };
        let Err(IndexerError::QuarantineIncomplete {
            quarantine,
            stranded,
            ..
        }) = &result
        else {
            panic!("a failed move is reported as such: {result:?}");
        };
        assert_eq!(stranded, &vec![quarantine.join("fts.sqlite-wal")]);
        let message = result.as_ref().expect_err("checked above").to_string();
        assert!(
            message.contains(&quarantine.join("fts.sqlite-wal").display().to_string()),
            "the error names the stranded file: {message}"
        );
        assert_eq!(
            fs::read(quarantine.join("fts.sqlite-wal")).expect("the stranded log"),
            planted.1,
            "a stranded file is kept, never deleted"
        );
        assert_eq!(
            fs::read(index_file(&db)).expect("the database never moved"),
            planted.0,
            "and nothing was rebuilt over it"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ori_t_0035_an_index_file_that_cannot_be_renamed_leaves_the_database_and_its_log_together() {
        // The review's exact reproduction, with a real file the operating
        // system refuses to rename (`chflags uchg`), not the test seam.
        struct Unlock(PathBuf);
        impl Drop for Unlock {
            fn drop(&mut self) {
                let _ = std::process::Command::new("chflags")
                    .arg("nouchg")
                    .arg(&self.0)
                    .status();
            }
        }
        let scratch = Scratch::new("move-immutable");
        let mut db = product(&scratch);
        let planted = plant_an_index_whose_log_holds_a_commit(&db, &scratch);
        let file = index_file(&db);
        let status = std::process::Command::new("chflags")
            .arg("uchg")
            .arg(&file)
            .status()
            .expect("chflags runs");
        assert!(status.success(), "the file was made immutable");
        let unlock = Unlock(file.clone());
        assert!(
            fs::rename(&file, scratch.path.join("probe")).is_err(),
            "the precondition: the file really cannot be renamed"
        );
        let result = Indexer::recover(&mut db, &[], Timestamp::from_millis(7));
        drop(unlock);
        assert_rolled_back_then_recovered_together(&mut db, &planted, result);
    }

    // ---------------------------------------------------------------------
    // Item 7: every stored column compared; counters that describe the
    // write.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_incremental_sync_repairs_a_title_or_body_changed_out_of_band_with_kind_and_checksum_untouched()
     {
        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("x.md", DocumentKind::Section, "X", "xray"),
        ];
        let tampers = [
            (
                "another body",
                "UPDATE documents SET body = 'other' WHERE path = 'x.md'",
            ),
            (
                "another title",
                "UPDATE documents SET title = 'Y' WHERE path = 'x.md'",
            ),
            (
                "a NULL body",
                "UPDATE documents SET body = NULL WHERE path = 'x.md'",
            ),
            (
                "a title stored as a blob of the same bytes",
                "UPDATE documents SET title = CAST('X' AS BLOB) WHERE path = 'x.md'",
            ),
            (
                "a body stored as a number",
                "UPDATE documents SET body = 42 WHERE path = 'x.md'",
            ),
        ];
        for (label, tamper) in tampers {
            let scratch = Scratch::new("tamper-columns");
            let db = product(&scratch);
            let file = index_file(&db);
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&target).expect("full rebuild");
            let expected = raw_rows(&file);
            Connection::open(&file)
                .expect("a raw connection")
                .execute_batch(tamper)
                .expect("apply the tamper");
            assert_ne!(
                raw_rows(&file),
                expected,
                "{label}: the tamper changed a row"
            );

            let first = indexer
                .incremental_sync(&target)
                .unwrap_or_else(|error| panic!("{label}: incremental_sync must succeed: {error}"));
            assert_eq!(
                raw_rows(&file),
                expected,
                "{label}: incremental_sync must leave exactly what full_rebuild stores"
            );
            assert_eq!(
                (first.upserted, first.replaced, first.removed),
                (1, 1, 0),
                "{label}"
            );
            let second = indexer.incremental_sync(&target).expect("resync");
            assert_eq!(
                (second.upserted, second.removed),
                (0, 0),
                "{label}: settles"
            );
            assert_eq!(
                indexer
                    .all_documents()
                    .expect("every row reads back again")
                    .len(),
                2,
                "{label}"
            );
            assert_eq!(
                indexer.search("xray", 10).expect("search").hits.len(),
                1,
                "{label}"
            );
        }
    }

    #[test]
    fn ori_t_0035_the_report_counters_describe_every_row_each_write_deleted_and_inserted() {
        // Stored before each write: a.md (unchanged in the target), x.md
        // twice (the target changes it), old.md (not in the target) and a
        // row with no text path. The target adds new.md.
        let seed = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("x.md", DocumentKind::Section, "X", "xray"),
            doc("old.md", DocumentKind::Section, "O", "oscar"),
        ];
        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("x.md", DocumentKind::Section, "X", "xray changed"),
            doc("new.md", DocumentKind::Section, "N", "november"),
        ];
        for method in ["full_rebuild", "incremental_sync"] {
            let scratch = Scratch::new("counters");
            let db = product(&scratch);
            let file = index_file(&db);
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&seed).expect("seed");
            Connection::open(&file)
                .expect("a raw connection")
                .execute_batch(
                    "INSERT INTO documents (path, kind, title, body, checksum) \
                     VALUES ('x.md', 'section', 'X', 'xray', 1); \
                     INSERT INTO documents (path, kind, title, body, checksum) \
                     VALUES (NULL, 'section', 'N', 'nothing', 1);",
                )
                .expect("store a second x.md row and a row with no path");
            let before = raw_rows(&file).len();
            assert_eq!(before, 5);

            let report = if method == "full_rebuild" {
                indexer.full_rebuild(&target)
            } else {
                indexer.incremental_sync(&target)
            }
            .expect("the write succeeds");
            assert_eq!(
                before - report.removed - report.replaced + report.upserted,
                report.total,
                "{method}: the counters account for every row: {report:?}"
            );
            assert_eq!(report.total, raw_rows(&file).len(), "{method}");
            let expected = if method == "full_rebuild" {
                // Every stored row is dropped: old.md and the pathless row
                // are not in the target; a.md and both x.md rows are.
                IndexReport {
                    upserted: 3,
                    removed: 2,
                    replaced: 3,
                    total: 3,
                }
            } else {
                // a.md is left alone; both x.md rows are replaced by one.
                IndexReport {
                    upserted: 2,
                    removed: 2,
                    replaced: 2,
                    total: 3,
                }
            };
            assert_eq!(report, expected, "{method}");
        }
    }

    // =====================================================================
    // Round 7: six items an adversarial review confirmed by independent
    // reproduction. Each test below fails against the code before its fix.
    // =====================================================================

    // ---------------------------------------------------------------------
    // Item 1: a genuine out-of-memory condition inside the check itself.
    // ---------------------------------------------------------------------

    /// SQLite's current process-wide hard heap limit, in bytes.
    fn hard_heap_limit(conn: &Connection) -> u64 {
        let limit: i64 = conn
            .query_row("PRAGMA hard_heap_limit", [], |row| row.get(0))
            .expect("read the hard heap limit");
        u64::try_from(limit).expect("a limit is never negative")
    }

    /// How many bytes SQLite can still allocate in one piece on `conn`
    /// under a hard heap limit of `limit`, found with `randomblob`, which
    /// allocates exactly the blob it returns: the distance between the heap
    /// in use and the limit, measured without an `unsafe` call (the heap in
    /// use has no safe accessor). Doubling from a small size, then
    /// bisecting, so a small headroom costs only small blobs.
    fn heap_headroom(conn: &Connection, limit: u64) -> u64 {
        let fits = |bytes: u64| {
            conn.query_row(
                "SELECT length(randomblob(?1))",
                params![i64::try_from(bytes).expect("fits")],
                |row| row.get::<_, i64>(0),
            )
            .is_ok()
        };
        let (mut fits_below, mut fails_at) = (0u64, 256u64);
        while fails_at < limit && fits(fails_at) {
            fits_below = fails_at;
            fails_at *= 2;
        }
        let mut fails_at = fails_at.min(limit);
        while fails_at - fits_below > 16 {
            let middle = fits_below + (fails_at - fits_below) / 2;
            if fits(middle) {
                fits_below = middle;
            } else {
                fails_at = middle;
            }
        }
        fits_below
    }

    #[test]
    fn ori_t_0035_a_genuine_out_of_memory_condition_inside_the_check_itself_is_never_called_corrupt()
     {
        // The review's reproduction, on a smaller index: a healthy index,
        // SQLite's hard heap limit set to the heap in use plus a margin that
        // steps down from 512 KiB, 4 KiB at a time, then 256 bytes at a
        // time below 64 KiB. Where the margin leaves the check
        // enough memory for its own bookkeeping but not for a page it must
        // read, PRAGMA quick_check does not fail: it returns a report
        // ("unable to get the page. error code=7", "failed to get page 59",
        // "Page 7: never used"), which round 6 read as damage, so a search
        // and all_documents on a healthy index returned Corrupt, and a
        // caller told to recover would have quarantined it. Measured against
        // round 6's reading of the report: 68 Corrupt and 15 damaged
        // verdicts in the first run, 67 and 13 in the second, which has
        // another connection committing between calls, as the review's did.
        // The fine steps span the window found here with room on either
        // side; the sweep must also reach both an ample and a starved heap.
        const CHILD: &str = "ORI_T_0035_CHECK_OOM_CHILD";
        const ROOT: &str = "ORI_T_0035_CHECK_OOM_ROOT";
        const WRITER: &str = "ORI_T_0035_CHECK_OOM_WRITER";
        const COARSE: std::ops::RangeInclusive<u64> = 64 * 1024..=512 * 1024;
        const FINE: std::ops::Range<u64> = 12 * 1024..64 * 1024;
        let corpus: Vec<IndexableDocument> = (0..500)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    &format!("needle alpha{n} shared body text for a real doclist"),
                )
            })
            .collect();
        let Some(root) = std::env::var_os(ROOT).filter(|_| std::env::var_os(CHILD).is_some())
        else {
            for writer in ["0", "1"] {
                let scratch = Scratch::new("check-oom");
                {
                    let db = product(&scratch);
                    let mut indexer = Indexer::open(&db).expect("open on-disk index");
                    indexer.full_rebuild(&corpus).expect("seed");
                }
                let root = scratch.path.to_str().expect("a UTF-8 scratch path");
                run_in_child_process(
                    "indexer::tests::ori_t_0035_a_genuine_out_of_memory_condition_inside_the_check_itself_is_never_called_corrupt",
                    CHILD,
                    &[(ROOT, root), (WRITER, writer)],
                );
                let db = product(&scratch);
                let indexer = Indexer::open(&db).expect("the index still opens");
                let integrity: String = indexer
                    .conn
                    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                    .expect("run PRAGMA integrity_check");
                assert_eq!(integrity, "ok", "the index was healthy all along");
                let stored = indexer.all_documents().expect("read back");
                assert_eq!(
                    stored
                        .iter()
                        .filter(|(document, _)| document.path != "churn.md")
                        .count(),
                    corpus.len(),
                    "every seeded document is still there"
                );
            }
            return;
        };

        let with_writer = std::env::var(WRITER).is_ok_and(|value| value == "1");
        let db = ProductDb::open(Path::new(&root), PRODUCT, Timestamp::from_millis(2_000))
            .expect("the child opens the parent's product");
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        let mut writer = Indexer::open(&db).expect("a second connection, for the writer run");
        let _ = indexer
            .conn
            .query_row("PRAGMA hard_heap_limit = 33554432", [], |row| {
                row.get::<_, i64>(0)
            });
        assert!(indexer.search("needle", 50).is_ok());
        assert_eq!(indexer.all_documents().expect("warm").len(), corpus.len());
        assert_eq!(quick_check(&indexer.conn), Integrity::Intact);

        let (mut ok, mut out_of_memory, mut corrupt, mut damaged, mut other) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        let margins = COARSE.rev().step_by(4096).chain(FINE.rev().step_by(256));
        for margin in margins {
            if with_writer {
                let _ = writer.add_or_replace(&doc(
                    "churn.md",
                    DocumentKind::Section,
                    "C",
                    &format!("churn {margin}"),
                ));
            }
            let limit = hard_heap_limit(&indexer.conn);
            let in_use = limit - heap_headroom(&indexer.conn, limit);
            let target = in_use + margin;
            if target < limit {
                let _ = indexer.conn.query_row(
                    &format!("PRAGMA hard_heap_limit = {target}"),
                    [],
                    |row| row.get::<_, i64>(0),
                );
            }
            let results: [(&str, Result<(), IndexerError>); 4] = [
                ("search", indexer.search("needle", 50).map(|_| ())),
                ("search", indexer.search("alpha123", 50).map(|_| ())),
                ("all_documents", indexer.all_documents().map(|_| ())),
                (
                    "incremental_sync",
                    indexer.incremental_sync(&corpus).map(|_| ()),
                ),
            ];
            for (call, result) in results {
                match result {
                    Ok(()) => ok += 1,
                    Err(IndexerError::Corrupt { source, .. }) => {
                        corrupt += 1;
                        eprintln!("margin {margin}: {call} called Corrupt: {source:?}");
                    }
                    Err(IndexerError::Sqlite { source, .. })
                        if source.sqlite_error_code() == Some(rusqlite::ErrorCode::OutOfMemory) =>
                    {
                        out_of_memory += 1;
                    }
                    Err(error) => {
                        other += 1;
                        eprintln!("margin {margin}: {call}: {error:?}");
                    }
                }
            }
            if quick_check(&indexer.conn) == Integrity::Damaged {
                damaged += 1;
                eprintln!("margin {margin}: the check itself said Damaged");
            }
        }
        eprintln!(
            "check out-of-memory sweep (writer {with_writer}): ok {ok}, out of memory \
             {out_of_memory}, corrupt {corrupt}, check damaged {damaged}, other {other}"
        );
        assert!(
            ok > 0 && out_of_memory > 0,
            "the sweep must reach both an ample and a starved heap, or it proves nothing: ok \
             {ok}, out of memory {out_of_memory}"
        );
        assert_eq!(
            (corrupt, damaged),
            (0, 0),
            "a genuine out-of-memory condition on a healthy index is never Corrupt, and the check \
             never calls it damaged"
        );
    }

    #[test]
    fn ori_t_0035_a_check_report_naming_a_page_it_could_not_read_is_never_read_as_damage() {
        // The row the review traced under a genuine out-of-memory condition,
        // and the shapes around it: a page the check could not get is "could
        // not check", whatever else the report says; a page whose read
        // failed with SQLITE_CORRUPT, and every message about a page that
        // was read, is damage.
        let traced = "*** in database main ***\nFreelist: failed to get page 59\n\
                      Tree 2 page 2: unable to get the page. error code=7\n\
                      Page 7: never used";
        let cases = [
            (traced, ReportRow::UnreadPage),
            (
                "*** in database main ***\nTree 4 page 9: unable to get the page. error code=3082",
                ReportRow::UnreadPage,
            ),
            (
                "*** in database main ***\nTree 4 page 9: unable to get the page. error code=10",
                ReportRow::UnreadPage,
            ),
            (
                "*** in database main ***\nFailed to read ptrmap key=5",
                ReportRow::UnreadPage,
            ),
            (
                "*** in database main ***\nTree 4 page 9: unable to get the page. error code=11",
                ReportRow::Damage,
            ),
            (
                "*** in database main ***\nTree 4 page 9: unable to get the page. error code=267",
                ReportRow::Damage,
            ),
            (
                "*** in database main ***\nPage 7: never used",
                ReportRow::Damage,
            ),
            (
                "*** in database main ***\nTree 2 page 2: btreeInitPage() returns error code 11",
                ReportRow::Damage,
            ),
            (
                "malformed inverted index for FTS5 table main.documents",
                ReportRow::Damage,
            ),
            (
                "unable to validate the inverted index for FTS5 table main.documents: out of memory",
                ReportRow::NotValidated,
            ),
        ];
        for (row, expected) in cases {
            assert_eq!(report_row(row), expected, "{row:?}");
        }
    }

    /// `bytes` with bit 0 of the byte at `offset` flipped.
    fn flip_low_bit(bytes: &[u8], offset: usize) -> Vec<u8> {
        let mut flipped = bytes.to_vec();
        flipped[offset] ^= 0x01;
        flipped
    }

    #[test]
    fn ori_t_0035_damage_the_check_cannot_settle_is_never_blamed_on_the_query_and_recover_repairs_it()
     {
        // The review's case: one bit flipped in the tokenizer name the
        // stored schema holds, `unicode61` read back as `tnicode61`. Every
        // search then fails with FTS5's "no such tokenizer", a plain
        // SQLITE_ERROR, which round 6 mapped to InvalidQuery before any
        // check ran, telling the caller its query was at fault. The check
        // cannot settle this damage, so it is the error SQLite gives, never
        // InvalidQuery; and recover, which never reads the old file,
        // repairs it. Two header bytes the review flipped are pinned beside
        // it the same way: the write version (offset 18), after which the
        // file is read-only, and the schema format (offset 47), after which
        // SQLite refuses the file format.
        let corpus: Vec<IndexableDocument> = (0..40)
            .map(|n| {
                doc(
                    &format!("d{n}.md"),
                    DocumentKind::Section,
                    "D",
                    &format!("alpha words {n}"),
                )
            })
            .collect();
        for damage in ["tokenizer name", "write version", "schema format"] {
            let scratch = Scratch::new("unsettled-damage");
            let mut db = product(&scratch);
            let file = index_file(&db);
            Indexer::open(&db)
                .expect("open on-disk index")
                .full_rebuild(&corpus)
                .expect("seed");
            checkpoint(&file);
            let healthy = fs::read(&file).expect("read the index");
            let offset = match damage {
                "tokenizer name" => {
                    let name = b"unicode61";
                    let found: Vec<usize> = healthy
                        .windows(name.len())
                        .enumerate()
                        .filter(|(_, window)| window == name)
                        .map(|(at, _)| at)
                        .collect();
                    assert_eq!(found.len(), 1, "the tokenizer is named once, in the schema");
                    found[0]
                }
                "write version" => 18,
                _ => 47,
            };
            fs::write(&file, flip_low_bit(&healthy, offset)).expect("flip one bit");

            let outcomes: Vec<(&str, Result<(), IndexerError>)> = match Indexer::open(&db) {
                Err(error) => vec![("open", Err(error))],
                Ok(mut indexer) => {
                    if damage == "tokenizer name" {
                        assert_eq!(
                            quick_check(&indexer.conn),
                            Integrity::Undetermined,
                            "the check cannot settle this damage; that is the case pinned"
                        );
                    }
                    vec![
                        ("search", indexer.search("alpha", 10).map(|_| ())),
                        ("all_documents", indexer.all_documents().map(|_| ())),
                        ("full_rebuild", indexer.full_rebuild(&corpus).map(|_| ())),
                    ]
                }
            };
            eprintln!("{damage}: {outcomes:?}");
            assert!(
                outcomes.iter().any(|(_, outcome)| outcome.is_err()),
                "{damage}: some call must meet the damage, or this case proves nothing"
            );
            for (call, outcome) in &outcomes {
                assert!(
                    !matches!(
                        outcome,
                        Err(IndexerError::InvalidQuery { .. } | IndexerError::Corrupt { .. })
                    ),
                    "{damage}: {call} must report the error SQLite gives, never blame the query \
                     and never call damage the check cannot see Corrupt: {outcome:?}"
                );
            }
            if damage == "tokenizer name" {
                assert!(
                    matches!(
                        &outcomes[0],
                        ("search", Err(IndexerError::Sqlite { source, .. }))
                            if source.sqlite_error_code() == Some(rusqlite::ErrorCode::Unknown)
                    ),
                    "the search fails with the plain SQLITE_ERROR FTS5 gives: {:?}",
                    outcomes[0]
                );
            }

            let report = Indexer::recover(&mut db, &corpus, Timestamp::from_millis(8_000))
                .unwrap_or_else(|error| panic!("{damage}: recover repairs it: {error}"));
            assert_eq!(report.rebuilt.total, corpus.len(), "{damage}");
            let indexer = Indexer::open(&db).expect("the fresh index opens");
            let found = indexer.search("alpha", 100).expect("search after recovery");
            assert_eq!(
                (found.hits.len(), found.documents_covered),
                (corpus.len(), corpus.len()),
                "{damage}"
            );
        }
    }

    // ---------------------------------------------------------------------
    // Item 3 (HIGH): a hard link let one product's Indexer hold another's
    // file.
    // ---------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_open_refuses_an_index_file_hard_linked_to_another_products() {
        // The review's setup: product A's fts.sqlite, or its write-ahead
        // log, is a hard link to product B's. Round 6 refused only symbolic
        // links, so A's open was accepted, A read B's document, A's full
        // rebuild erased B's acknowledged writes, and recover(&mut B) ran
        // while A's Indexer held the file it moved. Now A's open is refused,
        // and recover(&mut A) moves only A's name.
        use std::os::unix::fs::MetadataExt;
        let alpha_words = vec![doc("a.md", DocumentKind::Section, "A", "alpha words")];
        let bravo = vec![doc("b.md", DocumentKind::Section, "B", "bravo words")];
        for variant in ["index file", "write-ahead log"] {
            let scratch = Scratch::new("open-hard-link");
            let mut alpha = other_product(&scratch, "PRODUCT-T35-A");
            let mut beta = other_product(&scratch, "PRODUCT-T35-B");
            Indexer::open(&alpha)
                .expect("A opens its own index")
                .full_rebuild(&alpha_words)
                .expect("A builds its index");
            // For the log, B's writer stays open, so B's write-ahead log
            // exists to link. For the database file, B has closed, as in
            // the review's first probe: its log is checkpointed and gone.
            let mut writer = Some(Indexer::open(&beta).expect("B opens its own index"));
            if let Some(writer) = writer.as_mut() {
                writer.full_rebuild(&bravo).expect("B builds its index");
            }
            if variant == "index file" {
                writer = None;
            }
            let (theirs, ours) = match variant {
                "index file" => (index_file(&beta), index_file(&alpha)),
                _ => (
                    index_dir(&beta).join("fts.sqlite-wal"),
                    index_dir(&alpha).join("fts.sqlite-wal"),
                ),
            };
            assert!(theirs.exists(), "{variant}: B's file exists to be linked");
            let _ = fs::remove_file(&ours);
            fs::hard_link(&theirs, &ours).expect("give B's file a second name in A's index/");
            assert_eq!(fs::metadata(&theirs).expect("B's file").nlink(), 2);
            let before = fs::read(&theirs).expect("read B's file");

            match Indexer::open(&alpha) {
                Err(IndexerError::OpenRefused { path, reason }) => {
                    let ours_resolved = ours
                        .parent()
                        .expect("A's index/")
                        .canonicalize()
                        .expect("resolve A's index/")
                        .join(ours.file_name().expect("a file name"));
                    assert_eq!(
                        path, ours_resolved,
                        "{variant}: the refusal names A's entry"
                    );
                    assert_eq!(reason, HARD_LINK_REFUSAL, "{variant}");
                }
                Err(other) => panic!("{variant}: refused for the wrong reason: {other:?}"),
                Ok(indexer) => panic!(
                    "{variant}: A's Indexer must never open a file another product's name is \
                     on; it reads {:?}",
                    indexer.search("bravo", 10)
                ),
            }
            assert_eq!(
                fs::read(&theirs).expect("read B's file again"),
                before,
                "{variant}: B's file is untouched"
            );

            let report = Indexer::recover(&mut alpha, &alpha_words, Timestamp::from_millis(9))
                .expect("recover moves A's name aside");
            assert!(
                report
                    .quarantined_files
                    .iter()
                    .any(|moved| moved.file_name() == ours.file_name()),
                "{variant}: A's name for the file is in A's quarantine: {report:?}"
            );
            assert_eq!(
                fs::metadata(&theirs).expect("B's file").nlink(),
                2,
                "{variant}: moving A's name kept the file; A's quarantine holds the second name"
            );
            assert_eq!(
                fs::read(&theirs).expect("read B's file once more"),
                before,
                "{variant}: recover never touched B's file"
            );
            let a = Indexer::open(&alpha).expect("A now opens a real index of its own");
            assert_eq!(a.search("alpha", 10).expect("search").hits.len(), 1);
            assert_eq!(a.search("bravo", 10).expect("search").hits.len(), 0);
            drop(a);

            // B's side: closing B's writer checkpoints its log and removes
            // B's name for it, so a linked log leaves B one name per file.
            // A linked database file keeps its second name in A's
            // quarantine, so B's own open is refused by the same rule
            // (B's writes would change A's evidence) until B recovers.
            drop(writer);
            let b_refused = matches!(
                Indexer::open(&beta),
                Err(IndexerError::OpenRefused { reason, .. }) if reason == HARD_LINK_REFUSAL
            );
            if variant == "index file" {
                assert!(b_refused, "{variant}: B's file still has a second name");
                Indexer::recover(&mut beta, &bravo, Timestamp::from_millis(10))
                    .expect("B recovers its own index");
            } else {
                assert!(
                    !b_refused,
                    "{variant}: B's own files have one name each again"
                );
            }
            let b = Indexer::open(&beta).expect("B opens");
            assert_eq!(b.search("bravo", 10).expect("search").hits.len(), 1);
            assert_eq!(b.search("alpha", 10).expect("search").hits.len(), 0);
        }
    }

    // ---------------------------------------------------------------------
    // Item 2: the walk crashed on a deep tree, and nothing bounded a walk.
    // ---------------------------------------------------------------------

    /// As [`run_in_child_process`], with the child's limit on open files
    /// lowered to `limit` first, by `ulimit -n` in a shell that then runs
    /// the child: cargo raises the limit it hands the tests it runs, so a
    /// limit a process may really have is seen only this way.
    #[cfg(unix)]
    fn run_in_child_process_with_open_file_limit(
        name: &str,
        env: &str,
        extra_env: &[(&str, &str)],
        limit: u32,
    ) {
        let exe = std::env::current_exe().expect("the test binary's own path");
        let mut command = std::process::Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("ulimit -n \"$0\" && exec \"$@\"")
            .arg(limit.to_string())
            .arg(exe)
            .args([name, "--exact", "--nocapture", "--test-threads=1"])
            .env(env, "1");
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let output = command.output().expect("the test binary runs as a child");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "the child run of {name} failed:\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("test result: ok. 1 passed"),
            "the child must have run exactly {name}, not zero tests:\n{stdout}"
        );
        eprint!("{stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0035_a_tree_as_deep_as_the_path_limit_allows_is_walked_on_a_small_stack() {
        // The review: round 6's walk recursed once per directory level, and
        // a tree of one-letter directories the path limit allows (250 levels
        // in an unoptimized build on a 2 MiB thread, 442 on a 1,600 KiB
        // thread optimized) aborted the whole process with a stack
        // overflow. Here, 400 levels on a 256 KiB thread, where the
        // recursion needs several MiB; a child process, since an overflow
        // aborts every test in the process it happens in. The child may
        // also open no more than 64 files: a walk that kept every level's
        // directory open would record the deep levels as unreadable.
        const CHILD: &str = "ORI_T_0035_DEEP_WALK_CHILD";
        const ROOT: &str = "ORI_T_0035_DEEP_WALK_ROOT";
        const DEPTH: usize = 400;
        const STACK: usize = 256 * 1024;
        const OPEN_FILES: u32 = 64;
        let Some(root) = std::env::var_os(ROOT).filter(|_| std::env::var_os(CHILD).is_some())
        else {
            let scratch = Scratch::new("deep-walk");
            let mut deepest = scratch.path.join("spec");
            for _ in 0..DEPTH {
                deepest.push("d");
            }
            fs::create_dir_all(&deepest).expect("create a deep tree");
            fs::write(deepest.join("deep.md"), "# Deep\n\ndeepmarker\n").expect("write");
            fs::write(scratch.path.join("spec").join("top.md"), "# Top\n\ntop\n").expect("write");
            let root = scratch.path.to_str().expect("a UTF-8 scratch path");
            run_in_child_process_with_open_file_limit(
                "indexer::tests::ori_t_0035_a_tree_as_deep_as_the_path_limit_allows_is_walked_on_a_small_stack",
                CHILD,
                &[(ROOT, root)],
                OPEN_FILES,
            );
            return;
        };

        let root = PathBuf::from(root);
        let walk = std::thread::Builder::new()
            .stack_size(STACK)
            .spawn(move || Indexer::walk_repo(&root))
            .expect("spawn a thread with a small stack")
            .join()
            .expect("the walk never overflows the stack")
            .expect("walk");
        assert!(walk.skipped.is_empty(), "{:?}", walk.skipped);
        let deep = format!("spec/{}deep.md#deep", "d/".repeat(DEPTH));
        let paths: Vec<&str> = walk.documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            [deep.as_str(), concat!("spec/top", ".md", "#top")],
            "name order, depth first: the directory d sorts before the file top"
        );
    }

    /// Makes `file` unreadable to this process until the returned guard is
    /// dropped, where the platform can (Unix, by its mode), so a test can
    /// tell a file the walk left out unread from one it read.
    fn make_unreadable(file: &Path) -> impl Drop + use<> {
        struct Restore(PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o644));
                }
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(file, fs::Permissions::from_mode(0o000))
                .expect("make a file unreadable");
            assert!(
                fs::read(file).is_err(),
                "this test needs a process the file mode applies to"
            );
        }
        Restore(file.to_owned())
    }

    #[test]
    fn ori_t_0035_a_walk_splits_and_reads_no_more_than_its_budget_and_records_what_it_left_out() {
        // The review: one 1 MiB file of empty headings made 524,288
        // documents and about 200 MB of memory, five such files 570 MB, and
        // nothing bounded the number of files. A walk now splits at most
        // MAX_WALK_DOCUMENTS documents and reads at most MAX_WALK_BYTES of
        // text into them; a file past either is left out whole, recorded,
        // and a later file that still fits is read. Since round 8 no one
        // file may split into more than MAX_FILE_DOCUMENTS, so it takes
        // sixteen files at that cap to spend the documents, and a walk that
        // spent either budget is refused by collect_from_repo.

        // The document budget: sixteen files at the per-file cap but for
        // one document, then a file of two (over by one), then a file of
        // one (exactly the last).
        let documents = Scratch::new("walk-document-budget");
        let spec = documents.path.join("spec");
        fs::create_dir_all(&spec).expect("create spec/");
        let files = MAX_WALK_DOCUMENTS / MAX_FILE_DOCUMENTS;
        assert_eq!(files, 16);
        for n in 0..files {
            let sections = if n + 1 == files {
                MAX_FILE_DOCUMENTS - 1
            } else {
                MAX_FILE_DOCUMENTS
            };
            fs::write(
                spec.join(format!("1-fill-{n:02}.md")),
                "# a\n".repeat(sections),
            )
            .expect("write a file of short sections");
        }
        fs::write(spec.join("2-over.md"), "# b\n\nover\n\n# c\n\nover\n")
            .expect("write a file one document over what is left");
        fs::write(spec.join("3-fits.md"), "# d\n\nfits\n").expect("write a file that fits");
        // Once the budget is spent a file is left out without being read:
        // this one could not be read at all, and is not recorded as
        // unreadable.
        let after = spec.join("4-after.md");
        fs::write(&after, "# e\n\nafter\n").expect("write a file after the budget");
        let unreadable = make_unreadable(&after);
        let walk = Indexer::walk_repo(&documents.path).expect("walk");
        drop(unreadable);
        assert_eq!(
            walk.skipped,
            vec![
                SkippedEntry {
                    path: spec.join("2-over.md"),
                    reason: SkipReason::WalkDocumentLimit { remaining: 1 },
                },
                SkippedEntry {
                    path: after,
                    reason: SkipReason::WalkDocumentLimit { remaining: 0 },
                },
            ]
        );
        assert_eq!(walk.documents.len(), MAX_WALK_DOCUMENTS);
        assert_eq!(
            walk.documents.last().map(|d| d.path.as_str()),
            Some(concat!("spec/3-fits", ".md", "#d"))
        );
        assert_eq!(
            assert_every_entry_is_indexed_or_skipped(&documents.path, &walk),
            files + 3
        );
        match Indexer::collect_from_repo(&documents.path) {
            Err(error @ IndexerError::WalkBudgetExhausted { .. }) => {
                assert_eq!(error.methodology_ref().section, 25);
                let IndexerError::WalkBudgetExhausted { left_out } = &error else {
                    unreachable!()
                };
                assert_eq!(left_out, &walk.skipped, "{error}");
            }
            other => panic!("a walk that ran out of documents is refused: {other:?}"),
        }

        // The byte budget: seventeen files of sixteen near-cap sections
        // each, of which sixteen fit, then a small file that still fits.
        let bytes = Scratch::new("walk-byte-budget");
        let spec = bytes.path.join("spec");
        fs::create_dir_all(&spec).expect("create spec/");
        let section = format!("{}\n", "w ".repeat(31_990));
        let big: String = (0..16).map(|n| format!("# Part {n}\n{section}")).collect();
        let big_len = u64::try_from(big.len()).expect("fits");
        assert!(big_len <= MAX_FILE_BYTES);
        assert!(16 * big_len <= MAX_WALK_BYTES && 17 * big_len > MAX_WALK_BYTES);
        for n in 0..17 {
            fs::write(spec.join(format!("{n:02}.md")), &big).expect("write a large file");
        }
        fs::write(spec.join("99-small.md"), "# Small\n\nsmallmarker\n")
            .expect("write a small file");
        // The file past the byte budget is left out from its length alone:
        // it could not be read, and is not recorded as unreadable.
        let unreadable = make_unreadable(&spec.join("16.md"));
        let walk = Indexer::walk_repo(&bytes.path).expect("walk");
        drop(unreadable);
        assert_eq!(
            walk.skipped,
            vec![SkippedEntry {
                path: spec.join("16.md"),
                reason: SkipReason::WalkByteLimit {
                    byte_len: big_len,
                    remaining: MAX_WALK_BYTES - 16 * big_len,
                },
            }]
        );
        assert_eq!(walk.documents.len(), 16 * 16 + 1);
        assert_eq!(
            walk.documents.last().map(|d| d.path.as_str()),
            Some(concat!("spec/99-small", ".md", "#small"))
        );
        assert_eq!(
            assert_every_entry_is_indexed_or_skipped(&bytes.path, &walk),
            18
        );
        match Indexer::collect_from_repo(&bytes.path) {
            Err(IndexerError::WalkBudgetExhausted { left_out }) => {
                assert_eq!(left_out, walk.skipped);
            }
            other => panic!("a walk that ran out of bytes is refused: {other:?}"),
        }
    }

    // ---------------------------------------------------------------------
    // Item 4 (MEDIUM): files not named .md were dropped with no record.
    // ---------------------------------------------------------------------

    /// Asserts that every entry under `repo_root/spec` other than a
    /// directory, found independently of the walk and never through a link,
    /// is either the file some document in `walk` came from or at or under
    /// an entry `walk` recorded as skipped. Returns how many entries were
    /// checked.
    fn assert_every_entry_is_indexed_or_skipped(repo_root: &Path, walk: &RepoWalk) -> usize {
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).expect("list a directory") {
                let entry = entry.expect("a directory entry");
                if entry.file_type().expect("an entry's type").is_dir() {
                    visit(&entry.path(), out);
                } else {
                    out.push(entry.path());
                }
            }
        }
        let mut entries = Vec::new();
        visit(&repo_root.join("spec"), &mut entries);
        for entry in &entries {
            let recorded = walk
                .skipped
                .iter()
                .any(|skipped| entry.starts_with(&skipped.path));
            let relative: Vec<String> = entry
                .strip_prefix(repo_root)
                .expect("under the root")
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect();
            let relative = relative.join("/");
            let anchored = format!("{relative}#");
            let indexed = walk
                .documents
                .iter()
                .any(|document| document.path == relative || document.path.starts_with(&anchored));
            assert!(
                recorded || indexed,
                "{relative} is neither indexed nor recorded as skipped: {:?}",
                walk.skipped
            );
        }
        entries.len()
    }

    #[test]
    fn ori_t_0035_every_file_the_walk_does_not_read_is_recorded_including_markdown_by_another_name()
    {
        // The review's files: Markdown spelled .MD and .markdown, a file
        // named .md (which has no extension), and a YAML file, each holding
        // a unique word. Round 6 dropped all four with a silent continue:
        // none indexed, none in the skip list, and a sync reported Ok.
        let scratch = Scratch::new("not-markdown");
        let spec = scratch.path.join("spec");
        fs::create_dir_all(spec.join("api")).expect("create spec/api");
        fs::write(spec.join("control.md"), "# Control\n\nwombatcontrol\n")
            .expect("write a control file");
        let others = [
            spec.join("NOTES.MD"),
            spec.join("extra.markdown"),
            spec.join(".md"),
            spec.join("api").join("openapi.yaml"),
        ];
        for (n, other) in others.iter().enumerate() {
            fs::write(other, format!("# Other\n\nwombat{n}\n")).expect("write a file");
        }

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        let recorded: BTreeSet<PathBuf> = walk
            .skipped
            .iter()
            .filter(|entry| entry.reason == SkipReason::NotMarkdown)
            .map(|entry| entry.path.clone())
            .collect();
        assert_eq!(
            recorded,
            others.iter().cloned().collect::<BTreeSet<PathBuf>>(),
            "every file not named .md is recorded, with its reason: {:?}",
            walk.skipped
        );
        assert_eq!(walk.skipped.len(), others.len(), "{:?}", walk.skipped);
        let paths: Vec<&str> = walk.documents.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, [concat!("spec/control", ".md", "#control")]);
        assert_eq!(
            assert_every_entry_is_indexed_or_skipped(&scratch.path, &walk),
            others.len() + 1
        );
    }

    // ---------------------------------------------------------------------
    // Item 6 (LOW): a byte-order mark hid a file's first heading.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_a_byte_order_mark_changes_no_document_identity_title_or_text() {
        // The review prepended EF BB BF, and nothing else, to three files:
        // round 6 moved a section file's first section from path#anchor to
        // the bare path, titled it by the path, and titled an ADR by its
        // first ## heading. The same files with and without the mark now
        // walk to the same documents.
        let files = [
            (
                ["spec", "NOTES.md"].join("/"),
                "# Notes: Ori Studio\n\nLow-level words.\n\n## Crates\n\ncrate words\n",
            ),
            (
                ["spec", "adr", "ADR-9001-mark.md"].join("/"),
                "# ADR-9001: A marked decision\n\nStatus: accepted\n\n## Context\n\nwhy\n",
            ),
            (
                ["spec", "criteria", "phase-9.md"].join("/"),
                "# Criteria\n\n| ID | Expected |\n|---|---|\n| ORI-P9-001 | first |\n",
            ),
        ];
        let plain = Scratch::new("bom-plain");
        let marked = Scratch::new("bom-marked");
        for (relative, text) in &files {
            for (root, prefix) in [(&plain.path, ""), (&marked.path, "\u{FEFF}")] {
                let file = root.join(relative);
                fs::create_dir_all(file.parent().expect("a parent")).expect("create a directory");
                fs::write(&file, format!("{prefix}{text}")).expect("write");
            }
        }
        assert_eq!(
            &fs::read(marked.path.join(&files[0].0)).expect("read")[..3],
            [0xEF, 0xBB, 0xBF],
            "the marked copy starts with the three bytes of a byte-order mark"
        );

        let plain_walk = Indexer::walk_repo(&plain.path).expect("walk the plain copy");
        let marked_walk = Indexer::walk_repo(&marked.path).expect("walk the marked copy");
        assert!(marked_walk.skipped.is_empty(), "{:?}", marked_walk.skipped);
        assert_eq!(
            marked_walk.documents, plain_walk.documents,
            "a byte-order mark changes no path, kind, title or text"
        );
        let by_path: BTreeMap<&str, &IndexableDocument> = marked_walk
            .documents
            .iter()
            .map(|document| (document.path.as_str(), document))
            .collect();
        let notes = format!("{}#notes-ori-studio", files[0].0);
        assert_eq!(by_path[notes.as_str()].title, "Notes: Ori Studio");
        assert!(
            !by_path.contains_key(files[0].0.as_str()),
            "no preamble document"
        );
        assert_eq!(
            by_path[files[1].0.as_str()].title,
            "ADR-9001: A marked decision"
        );
        assert!(
            marked_walk
                .documents
                .iter()
                .all(|document| !document.body.contains('\u{FEFF}')),
            "the mark is in no stored text"
        );
    }

    // ---------------------------------------------------------------------
    // Item 5 (MEDIUM): no spec/ to walk looked like an empty repository.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_a_walk_with_no_spec_directory_is_refused_and_never_empties_the_index() {
        // The review: a repository root that does not exist, and a spec/
        // that is a regular file, each walked as Ok with no document and
        // nothing skipped; incremental_sync with that walk then removed all
        // 213 documents of a real index and returned Ok.
        let scratch = Scratch::new("no-spec");
        let real = scratch.path.join("real");
        fs::create_dir_all(real.join("spec")).expect("create spec/");
        fs::write(real.join("spec").join("PRD.md"), "# PRD\n\nreal content\n")
            .expect("write a real document");
        let documents = Indexer::collect_from_repo(&real).expect("walk the real repository");
        let db = product(&scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        indexer.full_rebuild(&documents).expect("seed");

        let missing_root = scratch.path.join("no-such-repo");
        let without_spec = scratch.path.join("without-spec");
        fs::create_dir_all(&without_spec).expect("create a root with no spec/");
        let spec_is_a_file = scratch.path.join("spec-is-a-file");
        fs::create_dir_all(&spec_is_a_file).expect("create a root");
        fs::write(spec_is_a_file.join("spec"), "not a directory").expect("write spec as a file");
        let cases = [
            ("missing root", missing_root.clone(), missing_root),
            ("no spec/", without_spec.clone(), without_spec.join("spec")),
            (
                "spec/ a regular file",
                spec_is_a_file.clone(),
                spec_is_a_file.join("spec"),
            ),
        ];
        for (label, root, named) in cases {
            match Indexer::walk_repo(&root) {
                Err(error @ IndexerError::NoSpecDirectory { .. }) => {
                    assert_eq!(error.methodology_ref().section, 25);
                    let IndexerError::NoSpecDirectory { path, .. } = &error else {
                        unreachable!()
                    };
                    assert_eq!(path, &named, "{label}: {error}");
                }
                other => panic!("{label}: refused, never an empty walk: {other:?}"),
            }
            assert!(
                matches!(
                    Indexer::collect_from_repo(&root),
                    Err(IndexerError::NoSpecDirectory { .. })
                ),
                "{label}"
            );
            // The caller's documented pairing never reaches the sync.
            if let Ok(walk) = Indexer::walk_repo(&root) {
                indexer
                    .incremental_sync(&walk.documents)
                    .expect("the sync the review ran");
            }
            let found = indexer.search("real", 10).expect("search");
            assert_eq!(
                (found.hits.len(), found.documents_covered),
                (1, 1),
                "{label}: the index still holds the real document"
            );
        }
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

    // ---------------------------------------------------------------------
    // Round 8, item 2 (MEDIUM): recover moved a directory at an index
    // file's name, another product's whole live directory included.
    // ---------------------------------------------------------------------

    /// How many rows `probe_log` holds in the product database at
    /// `product_dir`, read through a connection of its own.
    fn probe_rows(product_dir: &Path) -> i64 {
        Connection::open_with_flags(
            product_dir.join("product.sqlite"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open the product database read-only")
        .query_row("SELECT count(*) FROM probe_log", [], |row| row.get(0))
        .expect("count probe_log")
    }

    #[test]
    fn ori_t_0035_recover_refuses_a_directory_at_an_index_file_name_and_moves_nothing() {
        // The review's layouts: another product whose own product directory
        // is this product's index/fts.sqlite-wal (its product id names that
        // path), another whose products root is index/fts.sqlite-journal,
        // and a plain directory at index/fts.sqlite-shm. Round 7 renamed
        // each whole into quarantine: the other product's live lock, event
        // log and index went with it, its later writes returned Ok into the
        // moved copy, and a second ProductDb for it then opened beside the
        // first.
        let alpha = [doc("a.md", DocumentKind::Section, "A", "alpha")];
        for name in ["fts.sqlite-wal", "fts.sqlite-journal", "fts.sqlite-shm"] {
            let scratch = Scratch::new("recover-directory");
            let mut db = product(&scratch);
            {
                let mut indexer = Indexer::open(&db).expect("open on-disk index");
                indexer.full_rebuild(&alpha).expect("seed");
            }
            let planted = fs::read(index_file(&db)).expect("the live index");
            let at = index_dir(&db).join(name);
            let opened = Timestamp::from_millis(2_000);
            let other = match name {
                "fts.sqlite-wal" => Some((scratch.path.clone(), format!("{PRODUCT}/index/{name}"))),
                "fts.sqlite-journal" => Some((at.clone(), "PRODUCT-OTHER".to_owned())),
                _ => {
                    fs::create_dir_all(&at).expect("create a directory at the name");
                    fs::write(at.join("keep"), b"kept").expect("write a file inside it");
                    None
                }
            };
            let mut other_db = other.as_ref().map(|(root, id)| {
                let mut other_db =
                    ProductDb::open(root, id, opened).expect("the other product opens");
                other_db
                    .connection()
                    .execute_batch(
                        "CREATE TABLE probe_log (n INTEGER); INSERT INTO probe_log VALUES (1);",
                    )
                    .expect("write the other product's event log");
                other_db
            });
            let other_dir = other_db.as_ref().map(|other_db| other_db.dir().to_owned());
            let mut other_indexer = other_db.as_ref().map(|other_db| {
                let mut indexer = Indexer::open(other_db).expect("the other product's index opens");
                indexer
                    .full_rebuild(&[doc("p.md", DocumentKind::Section, "P", "pword")])
                    .expect("seed the other product's index");
                indexer
            });
            if let Some(other_dir) = &other_dir {
                assert!(other_dir.join("lock").is_file(), "{name}: the precondition");
                assert!(
                    other_dir
                        .canonicalize()
                        .expect("resolve")
                        .starts_with(at.canonicalize().expect("resolve")),
                    "{name}: the other product lives at or under this product's index file name"
                );
            }
            assert!(
                matches!(Indexer::open(&db), Err(IndexerError::OpenRefused { .. })),
                "{name}: open refuses a directory at an index file's name"
            );

            let result = Indexer::recover(&mut db, &alpha, Timestamp::from_millis(9));
            match &result {
                Err(error @ IndexerError::RecoveryRefused { path, .. }) => {
                    assert_eq!(error.methodology_ref().section, 25);
                    assert_eq!(
                        path.canonicalize()
                            .expect("the refused entry is still there"),
                        at.canonicalize().expect("resolve"),
                        "{name}: {error}"
                    );
                }
                other => {
                    panic!("{name}: a directory at an index file's name is refused: {other:?}")
                }
            }
            assert!(at.is_dir(), "{name}: the directory was not moved");
            assert!(
                !index_dir(&db).join(QUARANTINE_DIR).exists(),
                "{name}: nothing was moved into quarantine: {:?}",
                quarantine_contents(&db)
            );
            assert_eq!(
                fs::read(index_file(&db)).expect("the database is at its live path"),
                planted,
                "{name}: and nothing was rebuilt over it"
            );

            let (Some((other_root, other_id)), Some(other_dir)) = (other, other_dir) else {
                assert_eq!(fs::read(at.join("keep")).expect("the file inside"), b"kept");
                continue;
            };
            // The other product's live files are where it has them open.
            for file in ["lock", "product.sqlite"] {
                assert!(other_dir.join(file).is_file(), "{name}: {file} stayed put");
            }
            other_indexer
                .as_mut()
                .expect("opened above")
                .add_or_replace(&doc(
                    "later.md",
                    DocumentKind::Section,
                    "Later",
                    "laterword",
                ))
                .expect("the other product's write");
            {
                let second = Indexer::open(other_db.as_ref().expect("opened above"))
                    .expect("a second Indexer on the other product");
                let found = second.search("laterword", 10).expect("search");
                assert_eq!(
                    (found.hits.len(), found.documents_covered),
                    (1, 2),
                    "{name}: the write went into the live index, not a moved copy"
                );
            }
            match ProductDb::open(&other_root, &other_id, opened) {
                Err(ori_store::db::DbError::Locked { .. }) => {}
                other => panic!(
                    "{name}: the other product's lock still excludes a second writer: {:?}",
                    other.map(|_| "opened")
                ),
            }
            drop(other_indexer);
            other_db
                .as_mut()
                .expect("opened above")
                .connection()
                .execute("INSERT INTO probe_log VALUES (2)", [])
                .expect("the other product's second event");
            assert_eq!(probe_rows(&other_dir), 2, "{name}: its event log is live");
        }
    }

    // ---------------------------------------------------------------------
    // Round 8, item 6 (LOW): an empty criteria file yielded no document and
    // no record.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_a_criteria_file_with_no_criterion_row_is_a_document_like_the_same_file_anywhere_else()
     {
        // The review's placeholders: an empty spec/criteria/personas file
        // and a blank-only phase-2 file walked to no document and no skip
        // entry, while the same files under spec/runbooks/ were one
        // document each. A file with no trailing newline is here too: its
        // text is the same document wherever it sits.
        let scratch = Scratch::new("empty-criteria");
        let spec = scratch.path.join("spec");
        let shapes = [
            ("empty", ""),
            ("blank", "\n\n"),
            ("unterminated", "prose with no newline"),
        ];
        for directory in ["criteria", "runbooks"] {
            fs::create_dir_all(spec.join(directory)).expect("create a directory");
            for (name, text) in shapes {
                fs::write(spec.join(directory).join(format!("{name}.md")), text)
                    .expect("write a file");
            }
        }
        fs::write(
            spec.join("criteria").join("rows.md"),
            "| ORI-P9-001 | F | first |\n\n",
        )
        .expect("write a criteria file of rows and blank lines only");

        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert!(walk.skipped.is_empty(), "{:?}", walk.skipped);
        assert_eq!(
            assert_every_entry_is_indexed_or_skipped(&scratch.path, &walk),
            2 * shapes.len() + 1
        );
        let by_path: BTreeMap<&str, &IndexableDocument> = walk
            .documents
            .iter()
            .map(|document| (document.path.as_str(), document))
            .collect();
        for (name, text) in shapes {
            let criteria = format!("spec/criteria/{name}.md");
            let runbooks = format!("spec/runbooks/{name}.md");
            let (Some(criteria_document), Some(runbooks_document)) = (
                by_path.get(criteria.as_str()),
                by_path.get(runbooks.as_str()),
            ) else {
                panic!("{name}: one document each: {by_path:?}");
            };
            assert_eq!(criteria_document.kind, DocumentKind::Section, "{name}");
            assert_eq!(criteria_document.title, criteria, "{name}");
            assert_eq!(criteria_document.body, text, "{name}");
            assert_eq!(
                (&criteria_document.body, criteria_document.kind),
                (&runbooks_document.body, runbooks_document.kind),
                "{name}: the same text is the same document wherever it sits"
            );
        }
        let rows: Vec<&str> = walk
            .documents
            .iter()
            .filter(|document| {
                document
                    .path
                    .starts_with(concat!("spec/criteria/rows", ".md"))
            })
            .map(|document| document.path.as_str())
            .collect();
        assert_eq!(
            rows,
            [concat!("spec/criteria/rows", ".md", "#ORI-P9-001")],
            "a file of rows and blank lines is its rows, with no empty document beside them"
        );
    }

    // ---------------------------------------------------------------------
    // Round 8, item 5 (LOW): documents holding a file whole kept \r\n.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_line_endings_change_no_document_and_a_first_heading_keeps_a_role_files_text_on_a_crlf_checkout()
     {
        // The review: the same spec/ checked out with \r\n and with \n
        // disagreed on the ADRs and the heading-less role files, whose text
        // was stored whole, \r\n included, while split sections were
        // rebuilt with \n; and appending a first heading to a role file on
        // the \r\n checkout rewrote the unchanged text above it.
        let role = concat!("spec/agents/", "coder", ".md");
        let files = [
            (
                role,
                "---\nrole: coder\n---\n\nYou are a coder.\nKeep\rthis carriage return.\n",
            ),
            (
                concat!("spec/adr/", "ADR-9002-endings", ".md"),
                "# ADR-9002: Endings\n\nStatus: accepted\n\n## Context\n\nwhy\n",
            ),
            (
                concat!("spec/", "NOTES", ".md"),
                "Preamble words.\n\n# Notes\n\nnote words\r\r\n\n## More\n\nmore words\n",
            ),
            (
                concat!("spec/criteria/", "phase-9", ".md"),
                "# Criteria\n\n| ID | Expected |\n|---|---|\n| ORI-P9-001 | first |\n",
            ),
        ];
        let crlf = |text: &str| text.replace('\n', "\r\n");
        let lf_root = Scratch::new("endings-lf");
        let crlf_root = Scratch::new("endings-crlf");
        let write_all = |root: &Path, appended: &str| {
            for (relative, text) in &files {
                let file = root.join(relative);
                fs::create_dir_all(file.parent().expect("a parent")).expect("create a directory");
                let text = if *relative == role {
                    format!("{text}{appended}")
                } else {
                    (*text).to_owned()
                };
                let text = if root == crlf_root.path {
                    crlf(&text)
                } else {
                    text
                };
                fs::write(&file, text).expect("write");
            }
        };
        write_all(&lf_root.path, "");
        write_all(&crlf_root.path, "");
        assert!(
            fs::read(crlf_root.path.join(role))
                .expect("read")
                .windows(2)
                .any(|pair| pair == b"\r\n"),
            "the precondition: the second checkout really has \\r\\n endings"
        );

        let lf_walk = Indexer::walk_repo(&lf_root.path).expect("walk the \\n checkout");
        let crlf_walk = Indexer::walk_repo(&crlf_root.path).expect("walk the \\r\\n checkout");
        assert!(crlf_walk.skipped.is_empty(), "{:?}", crlf_walk.skipped);
        assert_eq!(
            crlf_walk.documents, lf_walk.documents,
            "line endings change no path, kind, title or text"
        );
        let bare = |walk: &RepoWalk| -> IndexableDocument {
            walk.documents
                .iter()
                .find(|document| document.path == role)
                .expect("the role file's document")
                .clone()
        };
        assert!(
            bare(&crlf_walk).body.contains("Keep\rthis"),
            "a carriage return inside a line is text, and is kept"
        );
        assert!(
            crlf_walk
                .documents
                .iter()
                .all(|document| !document.body.contains("\r\n")),
            "no stored text holds a \\r\\n"
        );

        // Seed an index from the \r\n checkout, append a first heading to
        // the role file (the review's edit), and sync: only the new section
        // is written.
        let db_scratch = Scratch::new("endings-index");
        let db = product(&db_scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        indexer.full_rebuild(&crlf_walk.documents).expect("seed");
        let before = bare(&crlf_walk);
        write_all(&crlf_root.path, "## Notes\n\nzebrafish\n");
        let after_walk = Indexer::walk_repo(&crlf_root.path).expect("walk again");
        assert_eq!(
            bare(&after_walk),
            before,
            "the text above a new first heading is the document it always was"
        );
        let report = indexer
            .incremental_sync(&after_walk.documents)
            .expect("sync the added heading");
        assert_eq!(
            (report.upserted, report.removed, report.replaced),
            (1, 0, 0),
            "only the new section is written: {report:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Round 8, item 1 (MEDIUM): one odd file spent the walk's budget, and a
    // sync then removed the rest of the index.
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0035_one_odd_file_never_spends_the_walks_budget_and_a_walk_cut_short_never_reaches_a_sync()
     {
        // The review's two files, each added to a repository whose index
        // already held its real documents: spec/!.md, 16,384 bare '#'
        // lines (32 KiB) sorting first; and a criteria file of 16,384 rows
        // repeating one identifier, which stored one document. Each spent
        // the whole document budget, every later file was recorded as past
        // it, and collect_from_repo followed by incremental_sync removed
        // the real documents and returned Ok.
        let scratch = Scratch::new("odd-file-budget");
        let spec = scratch.path.join("spec");
        for directory in ["adr", "criteria", "runbooks"] {
            fs::create_dir_all(spec.join(directory)).expect("create a directory");
        }
        fs::write(
            spec.join(concat!("PROJECT_BRIEF", ".md")),
            "# Brief\n\nbriefword\n",
        )
        .expect("write the brief");
        fs::write(
            spec.join("adr").join("0001-first.md"),
            "# ADR-0001: First\n\nadrword\n",
        )
        .expect("write an ADR");
        fs::write(
            spec.join("runbooks").join("real.md"),
            "# Real\n\nrunbookword\n",
        )
        .expect("write a runbook");
        let real = Indexer::collect_from_repo(&scratch.path).expect("walk the real repository");
        assert_eq!(real.len(), 3);
        let db = product(&scratch);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        indexer.full_rebuild(&real).expect("seed");
        let assert_real_documents_indexed = |indexer: &Indexer<'_>, label: &str| {
            for word in ["briefword", "adrword", "runbookword"] {
                let found = indexer.search(word, 10).expect("search");
                assert_eq!(
                    (found.hits.len(), found.documents_covered),
                    (1, 3),
                    "{label}: {word}"
                );
            }
        };

        let odd_files = [
            (spec.join("!.md"), "#\n".repeat(MAX_WALK_DOCUMENTS)),
            (
                spec.join("criteria").join("0.md"),
                "| ORI-X-1 |\n".repeat(MAX_WALK_DOCUMENTS),
            ),
        ];
        for (odd, text) in &odd_files {
            let label = odd.display().to_string();
            fs::write(odd, text).expect("add the odd file");
            let walk = Indexer::walk_repo(&scratch.path).expect("walk");
            assert_eq!(
                walk.skipped,
                vec![SkippedEntry {
                    path: odd.clone(),
                    reason: SkipReason::FileDocumentLimit {
                        limit: MAX_FILE_DOCUMENTS
                    },
                }],
                "{label}: the odd file is left out, for itself"
            );
            assert_eq!(
                walk.documents, real,
                "{label}: every real file is still walked"
            );
            let documents = Indexer::collect_from_repo(&scratch.path).expect("collect");
            let report = indexer.incremental_sync(&documents).expect("sync");
            assert_eq!(
                (
                    report.upserted,
                    report.removed,
                    report.replaced,
                    report.total
                ),
                (0, 0, 0, 3),
                "{label}"
            );
            assert_real_documents_indexed(&indexer, &label);
            fs::remove_file(odd).expect("remove the odd file");
        }

        // Sixteen files at the cap still spend the documents: the walk
        // records every later file as past the budget, and collect_from_repo
        // refuses rather than hand a sync a set without them.
        let files = MAX_WALK_DOCUMENTS / MAX_FILE_DOCUMENTS;
        for n in 0..files {
            fs::write(
                spec.join(format!("!{n:02}.md")),
                "#\n".repeat(MAX_FILE_DOCUMENTS),
            )
            .expect("write a file at the cap");
        }
        let walk = Indexer::walk_repo(&scratch.path).expect("walk");
        assert_eq!(walk.documents.len(), MAX_WALK_DOCUMENTS);
        let past: Vec<&Path> = walk
            .skipped
            .iter()
            .filter(|entry| entry.reason == SkipReason::WalkDocumentLimit { remaining: 0 })
            .map(|entry| entry.path.as_path())
            .collect();
        assert_eq!(past.len(), 3, "{:?}", walk.skipped);
        match Indexer::collect_from_repo(&scratch.path) {
            Err(error @ IndexerError::WalkBudgetExhausted { .. }) => {
                let IndexerError::WalkBudgetExhausted { left_out } = &error else {
                    unreachable!()
                };
                assert_eq!(left_out.len(), 3, "{error}");
                assert!(error.to_string().contains("3 file(s)"), "{error}");
            }
            other => panic!(
                "a walk cut short is refused: {:?}",
                other.map(|documents| documents.len())
            ),
        }
        assert_real_documents_indexed(&indexer, "after the refusal");
    }

    // ---------------------------------------------------------------------
    // Round 9, item 2: the diagnosis read a write transaction a failed
    // statement had left open, and FTS5's in-memory view of it.
    // ---------------------------------------------------------------------

    /// SQLite's verdict on the committed file at `file`, through a fresh
    /// connection of its own, so with nothing of any other connection's
    /// uncommitted state or cached view: `PRAGMA integrity_check`, a read
    /// that also runs FTS5's inverted-index check, and, when `write` is
    /// set and so no other connection may be holding the write lock,
    /// FTS5's own `integrity-check` command too. An oracle independent of
    /// `quick_check`.
    fn committed_file_is_healthy(file: &Path, write: bool) -> bool {
        let raw = Connection::open(file).expect("a raw connection");
        let integrity: String = raw
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("run PRAGMA integrity_check");
        integrity == "ok"
            && (!write
                || raw
                    .execute(
                        "INSERT INTO documents(documents) VALUES('integrity-check')",
                        [],
                    )
                    .is_ok())
    }

    #[test]
    fn ori_t_0035_the_check_never_reads_the_write_transaction_a_failed_statement_left_open() {
        // What round 8's classify did after a failed statement inside a
        // write transaction: SQLite rolls back only that statement, so the
        // transaction, and whatever earlier statements in it wrote, was
        // still open, and quick_check read it. Here the earlier statement
        // leaves an FTS5 shadow row the index does not account for, the
        // kind of half-written state the review's out-of-memory run left
        // in FTS5's own view; the committed file is healthy throughout.
        let scratch = Scratch::new("check-clean-snapshot");
        let db = product(&scratch);
        let file = index_file(&db);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        indexer
            .full_rebuild(&sweep_corpus()[..40])
            .expect("seed the corpus");
        let out_of_memory = || {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOMEM),
                Some("out of memory".to_owned()),
            )
        };
        let tx = indexer.conn.transaction().expect("begin a write");
        tx.execute(
            "INSERT INTO documents_docsize(id, sz) VALUES (999999, x'00')",
            [],
        )
        .expect("an earlier statement of the transaction writes");
        assert_eq!(
            quick_check_in_one_snapshot(&tx),
            Integrity::Damaged,
            "the precondition: read inside that transaction, the index looks damaged"
        );
        assert!(
            committed_file_is_healthy(&file, false),
            "while the file is not"
        );

        let error = classify(&tx, &file, "delete one document", out_of_memory());
        assert!(
            matches!(&error, IndexerError::Sqlite { source, .. }
                if source.sqlite_error_code() == Some(ErrorCode::OutOfMemory)),
            "a healthy index is never called Corrupt: {error:?}"
        );
        assert!(
            tx.is_autocommit(),
            "the diagnosis ends the failed write and leaves no transaction open"
        );
        drop(tx);
        assert!(committed_file_is_healthy(&file, true));
        assert_eq!(
            indexer
                .search("needle", 100)
                .expect("the Indexer is still usable")
                .documents_covered,
            40
        );
        indexer
            .add_or_replace(&doc("after.md", DocumentKind::Section, "A", "after"))
            .expect("and still writes");
    }

    #[test]
    fn ori_t_0035_a_write_that_fails_part_way_through_its_transaction_never_calls_a_healthy_index_corrupt()
     {
        // The same through the public API: another writer's triggers on
        // FTS5's shadow tables make every document delete also write a
        // stray docsize row, and refuse one insert. incremental_sync
        // deletes first, so by the time the refused insert fails, its
        // transaction holds the stray row; the committed file never does.
        let scratch = Scratch::new("sync-fails-part-way");
        let db = product(&scratch);
        let file = index_file(&db);
        let mut indexer = Indexer::open(&db).expect("open on-disk index");
        let seed = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("b.md", DocumentKind::Section, "B", "beta"),
        ];
        indexer.full_rebuild(&seed).expect("seed");
        Connection::open(&file)
            .expect("another writer")
            .execute_batch(
                "CREATE TRIGGER stray AFTER DELETE ON documents_content BEGIN \
                     INSERT INTO documents_docsize(id, sz) VALUES (old.id + 1000000, x'00'); \
                 END; \
                 CREATE TRIGGER refuse BEFORE INSERT ON documents_content \
                     WHEN new.c0 = 'refused.md' BEGIN \
                     SELECT RAISE(ABORT, 'refused by another writer'); \
                 END;",
            )
            .expect("install the triggers");
        assert!(committed_file_is_healthy(&file, true), "the precondition");

        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("refused.md", DocumentKind::Section, "R", "refused"),
        ];
        match indexer.incremental_sync(&target) {
            Err(IndexerError::Sqlite { source, .. }) => assert_eq!(
                source.sqlite_error_code(),
                Some(ErrorCode::ConstraintViolation),
                "the trigger's refusal, as it is: {source:?}"
            ),
            other => panic!("a healthy index is never called Corrupt: {other:?}"),
        }
        assert!(
            committed_file_is_healthy(&file, true),
            "and it is still healthy"
        );
        let dump: Vec<String> = indexer
            .all_documents()
            .expect("the Indexer is still usable")
            .into_iter()
            .map(|(document, _)| document.path)
            .collect();
        assert_eq!(dump, ["a.md", "b.md"], "the failed sync changed nothing");
    }

    /// Every row of `documents_config`, FTS5's table of the options set on
    /// the index, as text, sorted.
    fn fts5_config(file: &Path) -> Vec<String> {
        let raw = Connection::open(file).expect("a raw connection");
        let mut statement = raw
            .prepare("SELECT k, v FROM documents_config ORDER BY k")
            .expect("prepare");
        statement
            .query_map([], |row| {
                Ok(format!("{:?}={:?}", row.get_ref(0)?, row.get_ref(1)?))
            })
            .expect("run")
            .collect::<rusqlite::Result<_>>()
            .expect("read every config row")
    }

    #[test]
    fn ori_t_0035_incremental_sync_never_keeps_an_fts5_option_this_build_did_not_set() {
        // The review's precondition for the false Corrupt: another writer
        // set two legal FTS5 options through SQL (pgsz 64, automerge 0).
        // full_rebuild dropped them; incremental_sync kept them. Every
        // option FTS5 takes is set the same way, rank and secure-delete
        // included.
        let target = vec![
            doc("a.md", DocumentKind::Section, "A", "alpha"),
            doc("b.md", DocumentKind::Section, "B", "beta"),
        ];
        let reference = Scratch::new("config-reference");
        let reference_db = product(&reference);
        {
            let mut indexer = Indexer::open(&reference_db).expect("open the reference");
            indexer
                .full_rebuild(&target)
                .expect("the reference rebuild");
        }
        let expected_config = fts5_config(&index_file(&reference_db));
        let expected_rows = raw_rows(&index_file(&reference_db));

        for options in [
            &["('pgsz', 64)", "('automerge', 0)"][..],
            &["('rank', 'bm25(10.0, 1.0)')"][..],
            &["('secure-delete', 1)"][..],
            &[
                "('crisismerge', 2)",
                "('usermerge', 2)",
                "('deletemerge', 0)",
            ][..],
        ] {
            let scratch = Scratch::new("config-foreign");
            let db = product(&scratch);
            let file = index_file(&db);
            let mut indexer = Indexer::open(&db).expect("open on-disk index");
            indexer.full_rebuild(&target).expect("seed");
            assert_eq!(fts5_config(&file), expected_config);
            let raw = Connection::open(&file).expect("another writer");
            for option in options {
                raw.execute(
                    &format!("INSERT INTO documents(documents, rank) VALUES {option}"),
                    [],
                )
                .expect("set an FTS5 option");
            }
            drop(raw);
            assert_ne!(
                fts5_config(&file),
                expected_config,
                "{options:?}: the precondition, the option is stored"
            );

            let report = indexer.incremental_sync(&target).expect("sync");
            assert_eq!(
                fts5_config(&file),
                expected_config,
                "{options:?}: after a sync the index holds only what a rebuild writes"
            );
            assert_eq!(raw_rows(&file), expected_rows, "{options:?}");
            assert_eq!(
                target.len() - report.removed - report.replaced + report.upserted,
                report.total,
                "{options:?}: the counters describe the write: {report:?}"
            );
            assert_eq!(report.total, target.len(), "{options:?}");
            let again = indexer.incremental_sync(&target).expect("resync");
            assert_eq!(
                (again.upserted, again.removed, again.replaced),
                (0, 0, 0),
                "{options:?}: and then it settles"
            );
            assert_eq!(indexer.search("alpha", 10).expect("search").hits.len(), 1);
        }
    }
}
