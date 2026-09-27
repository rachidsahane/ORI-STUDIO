//! The code map builder: AICD §8, AICD §25.
//!
//! AICD §8 gives the memory service three jobs, the first of which is to
//! "index layers 1 to 3: full-text search, semantic search, and a structural
//! map of each code repository (modules, dependencies, entry points)". AICD
//! §25 names the component that builds it, "code map builder", and fixes its
//! shape: "modules, their public interfaces, dependency edges, entry points,
//! the tests that cover each module, and the specification sections each
//! module implements". `spec/PRD.md` section 4.13, K-03, is the same
//! requirement read as a product feature: "Code map (tree-sitter): modules,
//! interfaces, dependency edges, entry points, covering tests, spec sections
//! per module". [`build_code_map`] is that builder. Criterion ORI-P1-030 is
//! what a caller measures it against: a 50,000 line, four language
//! repository, with the package it feeds (`memory.context`, ORI-T-0039)
//! staying under a cap that `spec/TESTING.md` section 3 has not recorded yet,
//! because no baseline has been calibrated. This module does not invent one;
//! it reports what it measures (see the `tests::ori_p1_030_the_fifty_thousand_line_measurement`
//! test, `#[ignore]`d, and this ticket's closing report).
//!
//! What this module does not do: assemble a context package (that is
//! ORI-T-0039's `Retrieval`), enforce a scope (that is `ScopeEnforcer`, in
//! `scope.rs`), or touch operational memory. `crates/ori-memory/src/lib.rs`'s
//! "must not: return unsanitized production content in a package" is a rule
//! for the barrier between layer 4 telemetry and layer 3; a code map reads a
//! repository's own source, which is layer 1 canonical knowledge already
//! versioned in git, not production data, so that rule does not reach this
//! module. It has its own rule instead, stated below.
//!
//! # AICD §25's rule this module is judged against
//!
//! "The repositories are the truth; the index is a derived cache and can be
//! rebuilt from scratch at any time." A [`CodeMap`] is exactly that: it holds
//! nothing [`build_code_map`] cannot recompute from the files on disk, and it
//! is never mutated in place, only rebuilt.
//!
//! # Tree-sitter never refuses a file
//!
//! Tree-sitter is error-tolerant by design: a file with a syntax error still
//! parses, into a tree with `ERROR` nodes standing in for what did not parse.
//! A code map that swallowed that fact would look complete while missing half
//! a module. So every [`Module`] here carries [`Module::parsed_with_errors`],
//! set from `tree.root_node().has_error()`, and [`CodeMap::coverage`] reports,
//! for the whole run: how many files were seen, how many parsed clean, how
//! many parsed with at least one `ERROR` node, and how many were not parsed at
//! all, each with a [`SkipReason`]. Nothing here calls a map "complete"; it
//! reports what it saw, and lets the caller decide what "complete" requires.
//!
//! # Threat model
//!
//! In scope: a hostile repository that holds still. Everything in it may be
//! chosen by an attacker: a FIFO, socket or device where a source file
//! should be, symlinks aimed anywhere (out of the root, into `.git`, at a
//! FIFO, at a directory, at themselves), a `spec` directory that is itself a
//! symlink, names that are not UTF-8, and files at or near the size cap
//! shaped to make parsing, extraction, import resolution or the citation
//! scan slow, or large. None of it may make [`build_code_map`] hang, read
//! outside the mapped root or inside `.git`, run a stage without a time
//! bound, hold or return memory beyond the bounds stated under "Memory"
//! below, or report a map as more complete than it is. The bounds in the
//! next section are how.
//!
//! Out of scope: a live writer, a process changing the tree while it is
//! being mapped. The code map reads the product's canonical checkout, and a
//! process able to swap files there already has write access to the host's
//! tree, so racing this module gains it nothing it did not already have
//! (the operator's ruling for this ticket, 2026-09-24). What such a race can
//! still do, stated here rather than claimed closed:
//!
//! - retarget a symlink entry between its re-validation and its open (the
//!   `canonicalize`-then-open gap in `process_file`), so a file outside the
//!   root or inside `.git` is read under the link's own path;
//! - swap a directory for a symlink after the walk classified it, so the
//!   walk lists a directory outside the root, or a later open passes through
//!   one (`O_NOFOLLOW` guards only a path's final component); the `spec`
//!   directory and everything under it, found once and then listed and
//!   opened by path, are open to the same swap, so citations can come from
//!   documents outside the root;
//! - keep growing the tree while it is walked, which lengthens the walk for
//!   as long as the writer keeps writing (the walk has no total bound; see
//!   "What is not bounded");
//! - on a unix platform outside the verified set (see
//!   `open_regular_file_no_follow`), swap a FIFO in between the `stat` that
//!   stands in for `O_NONBLOCK` there and the open, which then waits.
//!
//! One property is kept even against a live writer, on the platforms whose
//! `open(2)` flag values this ticket verified (macOS, where this round's
//! tests also ran; Linux x86_64 and aarch64, whose values were run in round 3
//! and cross-checked against the `libc` crate's source in round 4, with
//! this round's tests not run there): the mapper never hangs. Every open of
//! repository content is non-blocking and decided by
//! the opened handle's own metadata before anything is read (see "File
//! type" below), a directory is listed with `opendir`, which fails at once
//! on a FIFO rather than waiting
//! (`tests::ori_t_0036_listing_a_directory_that_became_a_fifo_fails_rather_than_blocks`),
//! and every stage that processes content runs under a deadline. A race can
//! change what is read; it cannot make an open, a read or a listing wait,
//! so the map always comes back
//! (`tests::ori_t_0036_a_fifo_swapped_in_after_the_walk_never_hangs_the_map`
//! swaps a FIFO in mid-run and waits a bounded time for the result).
//!
//! # Untrusted input, and the bounds this module sets
//!
//! The repository being mapped may be a user's own, so its content is
//! untrusted input in AICD §8's "sanitization barrier" sense, even though the
//! barrier itself (typed records, provenance, length caps) governs layer 4
//! telemetry, not this module. What carries over is the discipline: nothing
//! here executes a file, runs a build script, or shells out. The bounds
//! actually enforced:
//!
//! - **File type.** Every read of repository content, a plain walk entry, a
//!   symlink walk entry and a `spec/` document alike, goes through one
//!   function, `open_regular_file`: an open that never blocks (`O_NONBLOCK`,
//!   plus `O_NOFOLLOW` unless the entry is a symlink the walk validated),
//!   then a decision on the opened handle's own metadata, and only then a
//!   read, through `take(cap + 1)`. `open(2)` on a FIFO with no writer waits
//!   for one; with `O_NONBLOCK` it returns at once, and the handle's `fstat`
//!   refuses it, a socket or a device as [`SkipReason::NotARegularFile`]
//!   before a byte is read, whether it was there when the walk ran or was
//!   swapped in after. The walk still classifies each entry by its directory
//!   entry type (and a symlink's target by a `stat`), which skips a FIFO
//!   already in place without opening it, but the guarantee no longer rests
//!   on that early check: round 3 had left the symlink branch opening with a
//!   blocking, link-following open and checking the type only afterward,
//!   and the review of that round hung the map forever by swapping a FIFO in
//!   behind a symlink after the walk. Windows has no FIFO in a checkout; see
//!   `open_regular_file_no_follow`'s Windows variant for what its open does
//!   with the nearest things. On a unix platform outside the verified set, a
//!   `stat` taken just before the open stands in for `O_NONBLOCK`: it
//!   refuses every FIFO already in place, and not one swapped in between the
//!   two calls (see "Threat model").
//! - **Size.** [`CodeMapOptions::max_file_bytes`] caps how much of any one
//!   file is read, default 8 MiB. The bound is enforced on the bytes
//!   actually read, through [`std::io::Read::take`]`(cap + 1)`, not on a
//!   `stat`ed size trusted in advance: a file that grows, or that lies about
//!   its length (a virtual filesystem entry, for instance), is still cut off
//!   at the cap. A file over the cap is [`SkipReason::TooLarge`], and its
//!   `bytes` field reports what was actually read (at most `cap + 1`), which
//!   is also how a test can tell "read, then cut off" apart from "read in
//!   full, then measured"
//!   (`tests::ori_t_0036_a_file_over_the_size_cap_is_skipped_and_never_fully_read`).
//! - **Time.** One principle, not a bound scoped to whichever stage a review
//!   happened to measure first: every loop over content this module read
//!   from the repository checks a wall-clock deadline, and whatever it
//!   cannot finish before that deadline is a recorded `TimedOut`, never a
//!   hang, and never presented as if it were complete. Concretely, the
//!   stages, each under its own deadline:
//!   - **Per file** (reading, parsing, extraction, and test detection,
//!     together): [`CodeMapOptions::file_timeout`] plus a size-proportional
//!     allowance (`per_file_budget`; see its doc for why a fixed bound alone
//!     is wrong for roughly half the builds that check it). Parsing is
//!     bounded through [`tree_sitter::Parser::parse_with_options`]'s
//!     progress callback, which tree-sitter polls periodically. Extraction
//!     is bounded by this module's own traversal helpers (`for_each_node`,
//!     `for_each_sibling_group`) and every language's top-level scan loop,
//!     each of which checks the same deadline. `use crate::...` resolution
//!     (`RustModuleIndex::advance`) checks it too, periodically during its
//!     own walk, not only between the `use` items around it: a single
//!     pathological `use` path is exactly what escaped the per-file check in
//!     this ticket's third review round, because nothing *inside* the
//!     function that resolved it checked anything. The same holds for the
//!     two other places one statement can hold unbounded work, both found by
//!     the review of round 3 or while fixing it: a grouped `use` is walked
//!     node by node with a check per node (`rust_use_edges`), and a Python
//!     `from ... import` checks before each imported name
//!     (`python_import_from_edges`). Round 6 found, while measuring the edge
//!     cap, more loops over one statement's items that checked nothing, in
//!     8 MiB files that hold still (release builds, a 13-second budget):
//!     the names of a Go `var` or `const` list (a `var A,A,...,A int` ran
//!     4.7 to 12 seconds past it) and its `type` specs
//!     (`go_declaration_interfaces`); Python's imported names, which both
//!     Python paths collected in full before the first check (2.8 and 4.4
//!     seconds past); and, with no overrun measured but the same unchecked
//!     shape, a TypeScript `export let` list (`ts_declaration_interfaces`)
//!     and the siblings of one parent in Rust test detection
//!     (`rust_test_names`), and every gathering of one parent's children
//!     (now `gather_children`, under both traversal helpers and a grouped
//!     `use`'s member list), which gathered them all with no check. Each now
//!     checks per item, and names are walked in place rather than collected
//!     first.
//!   - **The `spec/` citation scan**, once per [`build_code_map`] call, not
//!     once per file (it runs after every file's [`Module`] already
//!     exists): its own deadline, sized by the same `file_timeout`
//!     configuration value, checked across every module and document it
//!     scans by the work done, after every 4096 lines or every 1 MiB of
//!     search, whichever comes first (`CitationScan::scan_document`), once
//!     per entry while documents are listed (`list_markdown`), and once per
//!     document before it is read (`scan_spec_directory`). By lines alone,
//!     one line can be a whole 8 MiB document: the review of round 3 ran the
//!     round-3 scan 5 to 9 times past its budget that way.
//!     [`Coverage::spec_citation_scan`] is where its own incompleteness is
//!     recorded, with the reason, not a demotion of an already-successful
//!     `Module`; see that field's doc for why.
//!
//!   A file whose syntax is small but pathologically nested (deeply
//!   bracketed input is the classic case) is bounded by time even when it is
//!   not bounded by size (see "Quadratic extraction" below for the defect
//!   class this whole principle closes, and what its own third round found
//!   still missing from the second round's version of it: two more stages
//!   doing unbounded work outside any deadline, not a defect in the
//!   per-file bound itself).
//! - **Symlinks.** A directory reached through a symlink is never descended
//!   into, whatever its target, which makes cycle-safety independent of where
//!   the link points (a symlink loop cannot be walked into in the first
//!   place, so no separate loop detector is needed). In a repository that
//!   holds still (see "Threat model"), this rule has no exception: the
//!   `spec/` scan resolves the directory named `spec` by
//!   reading the mapped root's own entries and matching the name exactly,
//!   never by asking the OS whether `root.join("spec")` is a directory,
//!   which would follow a symlink or, on Windows, a junction, there too.
//!   The `spec/` scan follows no symlink at all, and records each one it
//!   declines that could stand for documents (`spec` itself, a link named
//!   `*.md`, a link to a directory) as [`SkipReason::SymlinkNotFollowed`] in
//!   [`Coverage::spec_docs_skipped`], so the scan is then
//!   [`SpecScan::Partial`], not complete. A
//!   symlinked *file* is read only when [`std::fs::canonicalize`] resolves it
//!   to a path inside the repository root and outside any directory named
//!   `.git`; otherwise it is [`SkipReason::SymlinkOutsideRoot`] or
//!   [`SkipReason::GitMetadata`] and is never opened. It is reported under
//!   its own in-root path, never its target's, so a link and its target are
//!   two distinct entries in [`Coverage`], the way two distinct files always
//!   are. A symlink to a directory is never descended into either, and is
//!   recorded as [`SkipReason::SymlinkedDirectory`], not silently absent:
//!   the walk saw it and made a decision about it, unlike an ordinary
//!   subdirectory, which is not a "file" candidate at all.
//! - **Time-of-check to time-of-use.** A symlink, once validated at walk
//!   time (in-root, outside `.git`), is re-validated immediately before its
//!   open, and a plain, non-symlink entry is opened refusing to follow a
//!   symlink at all (`open_regular_file_no_follow`, on the platforms this
//!   ticket verified those flags on), so a path swapped for a symlink after
//!   the walk classified it as a plain file is refused, not read through
//!   (this ticket's third review round reproduced that swap against the
//!   second round's `stat`-then-open). This narrows what a live writer can
//!   do; it does not close it, and closing it is not claimed: see "Threat
//!   model" for what a race can still change, and for the one thing it
//!   cannot, which is make the map hang.
//! - **Traversal.** The directory walk is iterative (an explicit stack of
//!   pending directories), never recursive, so a pathologically deep
//!   directory tree cannot overflow the call stack the way a naive recursive
//!   walker would. A directory literally named `.git` is not descended into
//!   at any depth, by the walk or by the `spec/` citation scan (which
//!   records each one it passes over in [`Coverage::spec_docs_skipped`];
//!   until round 6 it descended them), because it is version-control
//!   metadata, never source; this is one of two filesystem conventions this
//!   module hard-codes (the other is `spec`, above), and both are documented
//!   here because they are real exclusions, not oversights (a repo that
//!   keeps source inside a directory named `.git` is not one this module
//!   claims to map, and none does).
//! - **Every entry accounted for.** A directory this module cannot read
//!   (permissions, a transient I/O error) is recorded as one
//!   [`SkipReason::Unreadable`] entry for the directory itself, not silently
//!   dropped with its contents unlisted; the root directory failing the same
//!   way is a hard [`Error::Root`], never an empty [`CodeMap`] that would
//!   read the same as an empty repository. An entry whose name is not valid
//!   UTF-8 is recorded too, under its lossy rendering, as
//!   [`SkipReason::NonUtf8Name`], rather than silently absent from both
//!   `files_seen` and `files_skipped`; this cannot be exercised on this
//!   module's own development filesystem (APFS rejects such names outright),
//!   so it is proven on Linux instead (see the report for how).
//! - **Memory.** What the `spec/` citation scan holds while it runs, how
//!   much of `spec/` it reads, and what the returned map keeps from it, each
//!   bounded since round 5; what each module keeps from its own file, since
//!   round 6 (the last two items below). Until round 5 none was: the review
//!   of round 3 measured 2.3 GB held for eight newline-only documents of
//!   8 MiB (every line of every document copied into its own `String`, all
//!   documents at once, about 35 times the corpus), and 4.2 GB of heading
//!   text in the returned map for 2000 modules each cited under 500 long
//!   headings (the heading copied into every citation), both from
//!   repositories that hold still.
//!   - *While scanning:* one document at a time, read into one buffer of at
//!     most `max_file_bytes + 1` bytes (8 MiB by default) and dropped before
//!     the next is read, every line searched in place as a slice of that
//!     buffer and never copied (`CitationScan::scan_document`). Besides the
//!     buffer: the list of document paths and the record of entries not
//!     scanned ([`Coverage::spec_docs_skipped`], one entry each; both as
//!     many as `spec/` has entries, the one quantity here bounded only the
//!     way the walk itself is; see "What is not bounded"), per module the
//!     (document, heading) pairs it
//!     has been cited under (at most 500), and the output below, whose
//!     heading text is held twice while the scan runs (the table and its
//!     lookup index) and once after.
//!   - *The whole corpus:* at most [`CodeMapOptions::max_spec_bytes`] read
//!     across every document together, 64 MiB by default, counted on bytes
//!     actually read (at most one byte past the budget, which is how the
//!     scan knows the corpus did not fit). A larger corpus is scanned up to
//!     the first document that does not fit and reported as
//!     [`SpecScan::CorpusOverBudget`], never as complete, with that document
//!     and every one after it in [`Coverage::spec_docs_skipped`] as
//!     [`SkipReason::SpecCorpusOverBudget`]
//!     (`tests::ori_t_0036_a_spec_corpus_over_the_total_budget_is_reported_incomplete`).
//!   - *Nothing passed over silently:* a document over the per-file cap
//!     ([`SkipReason::TooLarge`]), not UTF-8 ([`SkipReason::Binary`]), not a
//!     regular file or not readable ([`SkipReason::NotARegularFile`],
//!     [`SkipReason::Unreadable`]), a directory under `spec/` that cannot
//!     be listed, and a symlink the scan declines are each recorded in
//!     [`Coverage::spec_docs_skipped`], and the scan is then
//!     [`SpecScan::Partial`]. [`SpecScan::Complete`] means every listed
//!     document was read and scanned and that list is empty: the bounds
//!     above never make a scan that passed over something look like one
//!     that did not.
//!   - *Kept in the returned map:* each cited document's path once
//!     ([`CodeMap::spec_docs`]) and each distinct cited heading once
//!     ([`CodeMap::spec_headings`]), a heading longer than 4096 bytes cut to
//!     4096 (back to a UTF-8 character boundary) and marked
//!     [`SpecHeading::truncated`]. So the heading text a map keeps is at
//!     most the smaller of 4096 bytes times the number of distinct cited
//!     headings and the bytes read from `spec/` (each kept heading is a
//!     different piece of a line that was read, kept at most once), which
//!     is at most `max_spec_bytes`, 64 MiB by default, however many modules
//!     cite it. Each citation is two indices, 24 bytes on a 64-bit target,
//!     at most 500 per module ([`Module::spec_sections_truncated`] marks a
//!     module that had more), so at most 12 KB of citations per module
//!     (`tests::ori_t_0036_retained_heading_text_is_bounded_for_many_modules_citing_long_headings`
//!     sums what a returned map keeps, for the review's shape, against this
//!     bound).
//!   - *Kept per module, its dependency edges:* at most 4096, the first
//!     found in source order ([`Module::dependency_edges_truncated`] marks a
//!     module that had more). An edge is a 24-byte record on a 64-bit
//!     target and holds no copy of the importing module's path: the
//!     [`Module`] holding it is the importing module. A resolved target
//!     ([`EdgeTarget`]) is the walk's own allocation of that file's path,
//!     shared by every edge in the map that names it, so it is stored once
//!     for the whole map however many edges name it, and all of them
//!     together are at most the paths the walk found. An unresolved target
//!     is its own allocation: a 16-byte reference-count header and at most
//!     1024 bytes of text ([`DependencyEdge::to_truncated`] marks one cut to
//!     fit). So the edges one module keeps take at most 4096 x (24 + 16 +
//!     1024) = 4,358,144 bytes, about 4.2 MiB, not counting the allocator's
//!     own rounding, whatever its file holds and however long its own path
//!     or its targets' paths are
//!     (`tests::ori_t_0036_retained_edge_bytes_are_bounded_for_many_imports_at_a_long_path`
//!     sums what a returned map keeps, for the review's shape in all four
//!     languages, against this bound). Until round 6, an edge copied its
//!     importing module's path, a resolved edge its target's path as well,
//!     and an entry point its module's path, with no cap on how many there
//!     were: one import name costs about two bytes of source, and the
//!     review of round 5 measured 1.78 GB of such copies kept for one 4 MiB
//!     Python file (`import a,a,...`) at an 850-byte path, each symlink to
//!     the file adding as much again.
//!   - *Kept per module, the rest:* its path, at most 500 citations (above),
//!     and its interfaces, entry points and covering tests, none of which
//!     holds a copy of the module's path. Each of those is a fixed-size
//!     record (40, 32 and 24 bytes on a 64-bit target) plus text that is a
//!     distinct piece of the file's own source (a name, or a shebang entry
//!     point's first line) or one of the fixed names `main` and `__main__`.
//!     Their count is not capped: they grow with the bytes read from that
//!     one file, which [`CodeMapOptions::max_file_bytes`] caps, and with
//!     nothing else. The densest shape is one interface per two bytes of
//!     source (each is a distinct name, and two names need a byte between
//!     them: `var A,A,...,A int` in Go), 41 bytes kept per two read, so at
//!     most about 20.5 times the file's size, about 172 MB for a file of
//!     8 MiB that is nothing but such a list, not counting the allocator's
//!     rounding
//!     (`tests::ori_t_0036_retained_interface_bytes_stay_within_the_stated_factor_of_the_file`
//!     checks the factor on that shape).
//! - **What is not bounded.** There is no cap on the total number of files or
//!   total bytes walked, and no `.gitignore` is honored: a `target/` or
//!   `node_modules/` directory is walked like any other, its files seen,
//!   parsed if their extension matches, and reported. A repository whose
//!   build output dwarfs its source will have that reflected honestly in
//!   [`Coverage`] rather than hidden by a heuristic this module does not
//!   implement. Both are named as gaps in this ticket's closing report, not
//!   silently assumed away. The same holds for the number of entries under
//!   `spec/`, which are listed (paths only) before any is read, and for the
//!   number of interfaces, entry points and covering tests one module keeps,
//!   which only its file's size bounds (see "Memory"). Races with a
//!   live writer are out of scope; see "Threat model".
//!
//! # Quadratic extraction, and what stays fast
//!
//! This ticket's third adversarial-review round found that its own
//! second-round fix ("the whole per-file step is bounded") was true of the
//! stages the second round's own review had measured, and false of two more
//! that process the same untrusted content: `use crate::` resolution (one
//! function, rewritten to a per-prefix search that was itself O(k^2) in a
//! `use` path's segment count, with no deadline check inside it) and the
//! `spec/` citation scan (which does not run per file at all, so it never
//! received a deadline to check in the first place). Both are fixed here;
//! see "Time" above for where each stage's deadline now lives.
//!
//! `use crate::` resolution is also no longer quadratic in the general case,
//! not only bounded: `RustModuleIndex` is a trie built once per
//! [`build_code_map`] call, so a `use` path of `k` segments resolves in
//! O(k), the same complexity the rest of this module's traversals already
//! had. Building and, just as much, *dropping* that trie are both iterative
//! rather than recursive, for the same reason `for_each_node` and
//! `walk_repository` already are: a sufficiently long `use crate::` path
//! (or a sufficiently deep directory tree feeding one) built a trie deep
//! enough to overflow the stack on Rust's own default recursive `Drop`, a
//! second, distinct stack-depth failure this module's own new test for the
//! first one found, not a reviewer.
//!
//! One further shape this round's review measured but did not ask this
//! module to change: many attributes stacked on one item (nested or
//! repeated `#[...]` forms) costs real, non-trivial time to extract (about
//! 1.5 seconds at 600 KB, in the review's own measurement), but that cost
//! stays inside [`CodeMapOptions::file_timeout`]'s budget rather than
//! escaping it the way the two fixed cases did: past the budget, it is
//! [`SkipReason::TimedOut`], not a hang, which is what this module claims
//! for every pathological shape, not linear time for all of them. Left as
//! measured and documented, not changed.
//!
//! A grouped Rust import (`use crate::{foo::Foo, bar::Bar};`, a prefixed one
//! such as `use crate::net::{http::Client, tcp::Stream};`, nested groups, or
//! a top-level `use {crate::foo::Foo, std::fmt};`) gives exactly the edges
//! its ungrouped equivalent would, member by member, resolved or not
//! (`rust_use_edges`, which walks the parser's own tree and resolves each
//! member from the trie position its group's prefix reached, so a shared
//! prefix is walked once). Round 3 cut the argument text at its first `{`
//! and resolved what was left: a top-level group came out as one unresolved
//! edge carrying the grouped text, and a prefixed group as one edge to the
//! prefix's own file (`src/net.rs` for the example above), a
//! complete-looking edge to the wrong module, which the review of round 3
//! found. The text spent spelling out unresolved members is capped (a long
//! prefix over many members would otherwise cost their product); past the
//! cap, the rest are recorded once, together, as the declaration exactly as
//! written.
//!
//! The same shape, in Python: `from <K dots> import <M names>` rebuilt its
//! K-dot prefix once per name, K times M work inside one statement with no
//! deadline check inside it (over two minutes for one 8 MiB statement, in
//! the review of round 3), and kept all K dots in every unresolved edge's
//! text (678 MB from a 169 KB file). The dots are now counted once, spelled
//! as a count past 32, and the names loop checks the deadline
//! (`python_import_from_edges`).
//!
//! # What "covering tests" means here, and what it misses
//!
//! Nothing in K-03 or in AICD §25 defines "the tests that cover each module"
//! precisely, so this is the rule this module implements. It has two parts,
//! and both are read only, never executed:
//!
//! 1. **Inline.** A test recognized by the language's own convention,
//!    anywhere in the file (not only at the top level, since Rust's own
//!    convention nests `#[test]` functions inside `mod tests { ... }`):
//!    Rust, a function whose innermost preceding attribute's last path
//!    segment is exactly `test` (so `#[test]` and `#[tokio::test]` both
//!    count, `#[test_case(1)]` does not, because its last segment is
//!    `test_case`); Python, a function anywhere named with a `test` prefix;
//!    TypeScript, a call to `test(...)`, `it(...)` or `describe(...)` whose
//!    first argument is a string; Go, a function named with a `Test` prefix.
//! 2. **By filename convention**, in the same directory as the module (no
//!    import is resolved to make this link; it is a name match only):
//!    TypeScript, `<stem>.test.ts(x)` or `<stem>.spec.ts(x)`; Python,
//!    `test_<stem>.py` or `<stem>_test.py`, in the module's own directory or
//!    a sibling `tests/` or `test/` directory; Go, `<stem>_test.go`, which is
//!    also the Go toolchain's own rule. Rust has no filename rule: this
//!    module does not follow the import graph from a separate
//!    `tests/` integration test back to the module it exercises through the
//!    crate's public API, so that link is missed entirely.
//!
//! What this misses, stated rather than hidden: a nonstandard test file name
//! is never linked to the module it tests; a conventionally named test file
//! that does not actually exercise the module (a stale or misnamed one) is
//! linked anyway, because the rule is a name match, not a proof of coverage.
//! Neither false negative nor false positive is caught by anything here.
//!
//! # What "spec sections per module" means here, and what it misses
//!
//! AICD §25 says this is derived "from commit and pull request anchors",
//! which needs the git and pull-request history this module does not read
//! (that is `ori-orchestrator` and `ori-store`'s territory). What this module
//! does instead: if the mapped root has a `spec/` directory, every `.md` file
//! under it (each bounded by the same [`CodeMapOptions::max_file_bytes`], all
//! of them together by [`CodeMapOptions::max_spec_bytes`], read best effort:
//! a document that cannot be scanned contributes nothing rather than
//! failing the map, and is recorded in [`Coverage::spec_docs_skipped`] with
//! its reason, the scan then reported as not complete; a corpus over the
//! total budget stops the scan and says so) is scanned for the module's own
//! path exactly as this map
//! records it (root-relative, forward slashes) appearing as a literal
//! substring anywhere in the text, and each match is recorded as a
//! [`SpecCitation`] naming the document and the nearest preceding Markdown
//! heading, if any, by index into tables the map keeps once each (see
//! "Memory"). This is deliberately narrower than AICD §25's rule: a
//! specification that names a module by a different relative form, by name
//! alone, or from a different mapped root, is not found; a document that
//! merely happens to contain the path as a substring inside a code fence or
//! an unrelated sentence is found anyway. It is a stand-in a later ticket
//! (most likely retrieval, ORI-T-0039, or the citation checker) can replace
//! with the commit-anchor rule AICD §25 actually asks for; this module states
//! that replacement is owed rather than claiming to already be it.
//!
//! # Determinism
//!
//! The same repository produces the same [`CodeMap`], field for field,
//! regardless of the order the filesystem's `readdir` happens to return
//! entries in. Every directory's entries are sorted by name before they are
//! queued; every per-module list ([`Module::interfaces`],
//! [`Module::dependency_edges`], [`Module::entry_points`],
//! [`Module::covering_tests`], [`Module::spec_sections`]) is sorted by a
//! stable key before it is returned; [`CodeMap::modules`] is sorted by path;
//! [`Coverage::files_skipped`] is sorted by path. No output is ever built by
//! iterating a [`std::collections::HashSet`] or [`std::collections::HashMap`];
//! those are used only for membership lookups
//! (`tests::ori_t_0036_output_order_does_not_follow_directory_iteration_order`
//! and `tests::ori_p1_030_the_same_repository_gives_byte_identical_output_regardless_of_iteration_order`
//! are what this claim is checked against).

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read as _;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tree_sitter::{Node, ParseOptions, ParseState, Parser};

/// The result type this module returns.
///
/// No methodology section applies: this is the ordinary Rust alias, not a
/// control's decision. A [`build_code_map`] failure is a malformed input (a
/// root that is not a readable directory), never a refusal of an action, so
/// unlike `ori-core`'s `Error` it carries no `MethodologyRef`: nothing here
/// enforces a permission, that is `ScopeEnforcer`'s job elsewhere in this
/// crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Why [`build_code_map`] could not run at all.
///
/// This is distinct from [`SkipReason`]: a [`SkipReason`] is recorded *inside*
/// a successful [`CodeMap`], for one file among many, and the map is still
/// returned. [`Error`] means no map was built at all, because the root itself
/// could not be walked.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The root path could not be resolved (it does not exist, or a
    /// component along it is not readable).
    Root {
        /// The path as given.
        path: String,
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// The root path resolved, but is not a directory.
    NotADirectory {
        /// The path as given.
        path: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root { path, source } => {
                write!(f, "cannot read repository root {path:?}: {source}")
            }
            Self::NotADirectory { path } => {
                write!(f, "repository root {path:?} is not a directory")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Root { source, .. } => Some(source),
            Self::NotADirectory { .. } => None,
        }
    }
}

/// One of the four languages K-03 names. TSX is parsed as TypeScript (see the
/// module doc); K-03 names four languages, not five.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Language {
    /// `.rs`.
    Rust,
    /// `.ts` and `.tsx`.
    TypeScript,
    /// `.py` and `.pyi`.
    Python,
    /// `.go`.
    Go,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rust => "Rust",
            Self::TypeScript => "TypeScript",
            Self::Python => "Python",
            Self::Go => "Go",
        };
        f.write_str(name)
    }
}

/// What kind of public item [`Interface::name`] names.
///
/// Grouped coarsely across four languages that do not share a type system:
/// `Type` stands for a struct, class, enum, trait, interface or type alias;
/// `Constant` for a top-level binding (`const`, `static`, exported
/// `const`/`let` in TypeScript, an uppercase Go `const`/`var`); `Module` for a
/// Rust `pub mod` declaration, which is a namespace, not a value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InterfaceKind {
    /// A free function.
    Function,
    /// A struct, class, enum, trait, interface or type alias.
    Type,
    /// A top-level constant or variable binding.
    Constant,
    /// A Rust `pub mod` declaration.
    Module,
}

/// One public interface a module exposes.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Interface {
    /// Its name, as written.
    pub name: String,
    /// What kind of item it is.
    pub kind: InterfaceKind,
    /// The 1-based line it starts on.
    pub line: usize,
}

/// One dependency edge from a module to another module, or to something
/// outside the mapped repository.
///
/// The importing module is the [`Module`] whose [`Module::dependency_edges`]
/// holds this edge; the edge itself holds no copy of that module's path.
/// Until round 6 of this ticket it did (a `from` field, one copy of the path
/// per edge), which let one file hold gigabytes of copies of its own path;
/// see the module doc's "Memory" bound.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DependencyEdge {
    /// When [`DependencyEdge::external`] is `false`, the path of another
    /// file the walk found in this same repository (a [`Module::path`] when
    /// that file was parsed), shared with every other edge that resolves to
    /// the same file rather than copied per edge; see [`EdgeTarget`].
    /// Otherwise, the import target as written in the source (a crate name,
    /// a bare specifier, a dotted absolute import, a Go import path), because
    /// it could not be resolved to a file in the mapped repository. Three
    /// bounded departures from "as written": a member of a grouped Rust `use`
    /// is written as its own full path (its group's prefix, `::`, the
    /// member), which is what its ungrouped form records; a Python relative
    /// import with more than 32 leading dots writes them as a count (`[N
    /// leading dots]name`); and text longer than 1024 bytes is cut to 1024
    /// (back to a UTF-8 character boundary) and marked
    /// [`DependencyEdge::to_truncated`]. See the module doc's "Quadratic
    /// extraction" and "Memory" for why.
    pub to: EdgeTarget,
    /// Whether `to` names something outside the mapped repository (or
    /// something this module's resolver did not attempt, see the module
    /// doc's per-language notes; Go edges are always external).
    pub external: bool,
    /// Whether `to` is the first 1024 bytes of a longer unresolved target
    /// rather than all of it. Never `true` for a resolved edge, whose `to`
    /// is always a whole path.
    pub to_truncated: bool,
}

/// Where a [`DependencyEdge`] points, as text: read it as a `&str` (it
/// dereferences to one and compares equal to one).
///
/// Shared, not copied: every edge in one [`CodeMap`] that resolves to the
/// same file holds the same single allocation of that file's path, made
/// once per [`build_code_map`] call, so a module importing one file a
/// thousand times keeps one copy of the path, not a thousand. An unresolved
/// target is its own allocation of at most 1024 bytes. See the module doc's
/// "Memory" bound.
#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct EdgeTarget(Arc<str>);

impl EdgeTarget {
    /// The target's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for EdgeTarget {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for EdgeTarget {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Rendered as the text itself, exactly as a `String` field renders, so a
/// map's `Debug` output reads the same as it did before targets were shared.
impl fmt::Debug for EdgeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl fmt::Display for EdgeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for EdgeTarget {
    fn eq(&self, other: &str) -> bool {
        *self.0 == *other
    }
}

impl PartialEq<&str> for EdgeTarget {
    fn eq(&self, other: &&str) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<String> for EdgeTarget {
    fn eq(&self, other: &String) -> bool {
        *self.0 == **other
    }
}

/// One entry point: a place a program starts running. The module it is in
/// is the [`Module`] whose [`Module::entry_points`] holds it; like a
/// [`DependencyEdge`], it holds no copy of that module's path (until round
/// 6 it did, one per entry point, the same defect).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EntryPoint {
    /// `main` (Rust, Go, or a top-level TypeScript function so named),
    /// `__main__` (a Python `if __name__ == "__main__":` guard), or the
    /// literal first line of a shebang (`#!...`), which is checked for every
    /// language uniformly.
    pub name: String,
    /// The 1-based line it starts on.
    pub line: usize,
}

/// One citation of a module's path found in the mapped repository's own
/// `spec/` documents: which document, and under which heading, as indices
/// into [`CodeMap::spec_docs`] and [`CodeMap::spec_headings`], where each
/// cited document's path and each distinct cited heading is stored once for
/// the whole map rather than once per citation. [`CodeMap::spec_doc`] and
/// [`CodeMap::spec_heading`] read them. See the module doc, "What spec
/// sections per module means here, and what it misses", and "Memory" for
/// why a citation holds indices and not text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SpecCitation {
    /// The citing document, an index into [`CodeMap::spec_docs`].
    pub doc_index: usize,
    /// The nearest Markdown heading above the citation, an index into
    /// [`CodeMap::spec_headings`]; `None` when the document has no heading
    /// above it.
    pub heading_index: Option<usize>,
}

/// One distinct heading some [`SpecCitation`] names, stored once in
/// [`CodeMap::spec_headings`] however many modules cite under it.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SpecHeading {
    /// The heading's text, without its leading `#` marks and surrounding
    /// whitespace, and never longer than 4096 bytes.
    pub text: String,
    /// Whether the heading was longer than 4096 bytes, so that `text` is its
    /// first 4096 bytes (cut back to a UTF-8 character boundary), not all of
    /// it.
    pub truncated: bool,
}

/// One parsed source file and what this module found in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// Its path, relative to the mapped root, with forward slashes on every
    /// platform.
    pub path: String,
    /// Which of the four languages it was parsed as.
    pub language: Language,
    /// Whether tree-sitter's tree for this file contained at least one
    /// `ERROR` node. A map is never presented as complete while this is
    /// `true` anywhere in it; see the module doc.
    pub parsed_with_errors: bool,
    /// The module's public interfaces, sorted by (line, name).
    pub interfaces: Vec<Interface>,
    /// The module's outgoing dependency edges, sorted by (external, to), at
    /// most 4096 of them.
    pub dependency_edges: Vec<DependencyEdge>,
    /// Whether the file had more than 4096 dependency edges, so that
    /// `dependency_edges` holds the first 4096 found in source order (then
    /// sorted) and extraction stopped recording more. See the module doc's
    /// "Memory" bound.
    pub dependency_edges_truncated: bool,
    /// The module's entry points, sorted by (line, name).
    pub entry_points: Vec<EntryPoint>,
    /// The tests found to cover this module, by the rule the module doc
    /// states. Sorted and deduplicated.
    pub covering_tests: Vec<String>,
    /// The specification sections found to cite this module's path, at most
    /// 500. Sorted by the texts they index: document path, then heading.
    pub spec_sections: Vec<SpecCitation>,
    /// Whether this module was cited under more than 500 distinct (document,
    /// heading) pairs, so that `spec_sections` holds the first 500 found and
    /// the scan stopped looking for more of this module's citations.
    pub spec_sections_truncated: bool,
}

/// Why one candidate file was not parsed into a [`Module`].
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SkipReason {
    /// Its extension is not one this module maps to a language.
    UnsupportedLanguage,
    /// It is larger than [`CodeMapOptions::max_file_bytes`]. `bytes` is what
    /// was actually read (at most `cap + 1`), not a `stat`ed size taken on
    /// trust; see the module doc's "Size" bound.
    TooLarge {
        /// How many bytes were actually read before the cap cut it off.
        bytes: u64,
        /// The cap it exceeded.
        cap: u64,
    },
    /// It could not be read: an I/O error opening or reading it, described
    /// here as text since [`io::Error`] is not [`Clone`].
    Unreadable(String),
    /// It contains a NUL byte, or is not valid UTF-8; tree-sitter's Rust
    /// binding parses UTF-8 text, and a source file in these four languages
    /// is UTF-8 text.
    Binary,
    /// It is a symlink whose target resolves outside the mapped repository
    /// root, or whose target could not be resolved at all.
    SymlinkOutsideRoot,
    /// It, or a symlink's target, is not a regular file: a FIFO, socket,
    /// device or similar. Decided at walk time from the directory entry's
    /// type (or a `stat` of a symlink's target), neither of which opens
    /// anything, or, when it became one after the walk, from the opened
    /// handle's own metadata, after an open that does not block and before
    /// anything is read; see the module doc's "File type" bound.
    NotARegularFile,
    /// A symlink resolves inside a directory named `.git`, or, in
    /// [`Coverage::spec_docs_skipped`], a directory named `.git` under
    /// `spec/` that the citation scan did not descend. This module never
    /// descends `.git` directly; in [`Coverage::files_skipped`] this reason
    /// is what stops a symlink from reaching the same content by a side
    /// door.
    GitMetadata,
    /// Its name, or an ancestor directory's name, is not valid UTF-8. The
    /// path recorded here is a lossy rendering (invalid bytes replaced), for
    /// display only; it is not a path this module can open.
    NonUtf8Name,
    /// This module did not finish parsing and extracting it within its
    /// budget; see the module doc's "Time" bound.
    TimedOut,
    /// It is a symlink whose target is a directory. Never descended (see the
    /// module doc's cycle-safety note), and, unlike a plain directory,
    /// recorded here rather than silently absent from both
    /// [`Coverage::files_seen`] and [`Coverage::files_skipped`]: a directory
    /// reached by the ordinary (non-symlink) walk is not itself a "file" and
    /// so is never a candidate in the first place, but a symlink *is* one
    /// candidate entry this module saw and made a decision about, so that
    /// decision is recorded like any other.
    SymlinkedDirectory,
    /// A symlink under `spec/` (or `spec` itself) that the `spec/` citation
    /// scan does not follow, wherever it points: the scan reads no document
    /// through a link. Recorded only in [`Coverage::spec_docs_skipped`], and
    /// only for a link that could stand for documents: one named `*.md`, or
    /// one whose target is a directory.
    SymlinkNotFollowed,
    /// A `spec/` document the citation scan did not scan because the whole
    /// corpus reached [`CodeMapOptions::max_spec_bytes`] first: the
    /// document that did not fit, and every one after it. Recorded only in
    /// [`Coverage::spec_docs_skipped`].
    SpecCorpusOverBudget {
        /// The budget that ran out, in bytes.
        budget: u64,
    },
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLanguage => f.write_str("unsupported language"),
            Self::TooLarge { bytes, cap } => write!(f, "too large ({bytes} bytes > {cap} cap)"),
            Self::Unreadable(reason) => write!(f, "unreadable: {reason}"),
            Self::Binary => f.write_str("binary (or not valid UTF-8)"),
            Self::SymlinkOutsideRoot => f.write_str("symlink outside the repository root"),
            Self::NotARegularFile => f.write_str("not a regular file"),
            Self::GitMetadata => f.write_str("inside .git, version-control metadata, never source"),
            Self::NonUtf8Name => f.write_str("name is not valid UTF-8"),
            Self::TimedOut => f.write_str("timed out"),
            Self::SymlinkedDirectory => f.write_str("symlink to a directory, never descended"),
            Self::SymlinkNotFollowed => f.write_str("symlink the spec/ scan does not follow"),
            Self::SpecCorpusOverBudget { budget } => {
                write!(
                    f,
                    "not scanned: the spec/ corpus used up its {budget} byte budget first"
                )
            }
        }
    }
}

/// One file that was seen but not parsed, and why.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SkippedFile {
    /// Its path, relative to the mapped root.
    pub path: String,
    /// Why it was skipped.
    pub reason: SkipReason,
}

/// What the walk saw, whether or not it could parse what it saw.
///
/// Every file seen is accounted for exactly once: `files_seen ==
/// files_parsed_clean + files_parsed_with_errors + files_skipped.len()`
/// always holds (`tests::ori_p1_030_coverage_accounts_for_every_file_seen_exactly_once`
/// checks it, and
/// `tests::ori_t_0036_the_files_seen_invariant_holds_with_every_pre_skip_reason_nested`
/// checks it with every reason the walk itself records, below the root).
/// A map is never presented as complete while
/// `files_parsed_with_errors` or `files_skipped` is nonzero without that
/// being visible here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coverage {
    /// Every file the walk visited, parsed or not.
    pub files_seen: usize,
    /// Files that parsed with no `ERROR` node in their tree.
    pub files_parsed_clean: usize,
    /// Files that parsed but whose tree contained at least one `ERROR` node.
    pub files_parsed_with_errors: usize,
    /// Files seen but not parsed, with the reason for each, sorted by path.
    pub files_skipped: Vec<SkippedFile>,
    /// How the `spec/` citation scan ended: complete, or incomplete and why.
    /// The scan is a separate stage from parsing and extraction (it runs
    /// once, after every file's `Module` already exists), so its own
    /// incompleteness is recorded here rather than by turning an
    /// already-successfully-parsed module into a [`SkippedFile`]: a module's
    /// interfaces, edges, entry points and tests are unaffected by the
    /// citation scan stopping early, and are not retroactively called
    /// incomplete for a reason that has nothing to do with them. What *is*
    /// incomplete, honestly, when this is not [`SpecScan::Complete`]: some
    /// modules' [`Module::spec_sections`] may be missing citations a
    /// completed scan would have found. See the module doc's "Time" and
    /// "Memory" bounds.
    pub spec_citation_scan: SpecScan,
    /// Every entry under `spec/` the citation scan saw and did not scan, with
    /// the reason for each, sorted by path: a document that could not be
    /// opened as a regular file or read, was over
    /// [`CodeMapOptions::max_file_bytes`], was not UTF-8, or was not reached
    /// before the deadline or the corpus budget ran out; a directory that
    /// could not be listed; a directory named `.git`, which the scan never
    /// descends ([`SkipReason::GitMetadata`]); a symlink the scan does not
    /// follow. Empty exactly
    /// when [`Coverage::spec_citation_scan`] is [`SpecScan::Complete`], and
    /// never empty when it is [`SpecScan::Partial`].
    pub spec_docs_skipped: Vec<SkippedFile>,
}

/// How the `spec/` citation scan ended. Anything but [`SpecScan::Complete`]
/// means some citations may be missing; the citations found before the scan
/// stopped are kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SpecScan {
    /// Every document under `spec/` was read and scanned and nothing under
    /// it was skipped ([`Coverage::spec_docs_skipped`] is empty), or the
    /// mapped root has no `spec/` directory at all.
    Complete,
    /// The scan went through every document it listed, but did not scan at
    /// least one entry: each is in [`Coverage::spec_docs_skipped`] with its
    /// reason.
    Partial,
    /// The scan's deadline passed before it finished.
    TimedOut,
    /// The documents under `spec/` add up to more than
    /// [`CodeMapOptions::max_spec_bytes`]: the scan stopped at the first
    /// document that did not fit in what was left of the budget, and scanned
    /// neither it nor anything after it.
    CorpusOverBudget {
        /// The budget the corpus exceeded, in bytes.
        budget: u64,
    },
}

impl SpecScan {
    /// Whether the scan finished with nothing skipped: every document read
    /// and scanned.
    #[must_use]
    pub fn is_complete(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// The result of [`build_code_map`]: every module found, and what the walk
/// covered.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeMap {
    /// Every module found, sorted by path.
    pub modules: Vec<Module>,
    /// What the walk saw, parsed or not.
    pub coverage: Coverage,
    /// Every `spec/` document some [`SpecCitation`] names, each stored once,
    /// its path relative to the mapped root. [`SpecCitation::doc_index`]
    /// indexes this.
    pub spec_docs: Vec<String>,
    /// Every distinct heading some [`SpecCitation`] names, each stored once
    /// for the whole map, however many modules cite under it.
    /// [`SpecCitation::heading_index`] indexes this. See the module doc's
    /// "Memory" bound for how large it can get.
    pub spec_headings: Vec<SpecHeading>,
}

impl CodeMap {
    /// The document `citation` names, or `None` when `citation` did not come
    /// from this map.
    #[must_use]
    pub fn spec_doc(&self, citation: &SpecCitation) -> Option<&str> {
        self.spec_docs.get(citation.doc_index).map(String::as_str)
    }

    /// The heading `citation` names: `None` when the citation has no heading
    /// above it, or did not come from this map.
    #[must_use]
    pub fn spec_heading(&self, citation: &SpecCitation) -> Option<&SpecHeading> {
        citation
            .heading_index
            .and_then(|index| self.spec_headings.get(index))
    }
}

/// The bounds [`build_code_map_with_options`] enforces. See the module doc's
/// "Untrusted input" section for why each exists.
#[derive(Clone, Copy, Debug)]
pub struct CodeMapOptions {
    /// The largest file this module will read. Default 8 MiB. Enforced on
    /// bytes actually read, not a `stat`ed size taken on trust.
    pub max_file_bytes: u64,
    /// The longest this module gives itself for one file's *whole* step,
    /// parsing and extraction together. Default 5 seconds. Renamed from
    /// `parse_timeout`: extraction did not used to share this bound, which
    /// was itself a defect (see the module doc's "Quadratic extraction").
    pub file_timeout: Duration,
    /// The most bytes the whole `spec/` citation scan reads, across every
    /// document together. Default 64 MiB (eight documents at the default
    /// `max_file_bytes`). Enforced on bytes actually read: the scan reads at
    /// most one byte past it, and a corpus larger than this is scanned up to
    /// the first document that does not fit and reported as
    /// [`SpecScan::CorpusOverBudget`], never as complete.
    pub max_spec_bytes: u64,
}

impl Default for CodeMapOptions {
    fn default() -> Self {
        Self {
            max_file_bytes: 8 * 1024 * 1024,
            file_timeout: Duration::from_secs(5),
            max_spec_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Builds a [`CodeMap`] for the repository at `root`, with
/// [`CodeMapOptions::default`].
///
/// AICD §25: "code map builder ... updated on every merge". This module
/// builds one; when it is called is `ori-watch` or `ori-orchestrator`'s
/// decision (this crate has no IO trigger of its own), and re-reading the
/// module doc's "the index is a derived cache" is the reason a caller may
/// simply call this again rather than trying to update a map in place.
pub fn build_code_map(root: &Path) -> Result<CodeMap> {
    build_code_map_with_options(root, &CodeMapOptions::default())
}

/// [`build_code_map`], with explicit bounds.
pub fn build_code_map_with_options(root: &Path, options: &CodeMapOptions) -> Result<CodeMap> {
    let root_canon = fs::canonicalize(root).map_err(|source| Error::Root {
        path: root.display().to_string(),
        source,
    })?;
    let root_meta = fs::metadata(&root_canon).map_err(|source| Error::Root {
        path: root.display().to_string(),
        source,
    })?;
    if !root_meta.is_dir() {
        return Err(Error::NotADirectory {
            path: root.display().to_string(),
        });
    }
    // The root itself must be readable, checked explicitly and up front: an
    // unreadable *subdirectory* is a recorded skip (the map is still built,
    // see `walk_repository`), but an unreadable root would otherwise make
    // `walk_repository`'s own first `read_dir` fail the same way, silently,
    // leaving `files` and `pre_skipped` both empty and returning `Ok` with a
    // map that reads exactly like an empty repository. That contradicts this
    // type's own contract (see [`Error`]'s doc), so it is refused here
    // instead.
    fs::read_dir(&root_canon).map_err(|source| Error::Root {
        path: root.display().to_string(),
        source,
    })?;

    let walk = walk_repository(&root_canon);
    let files_seen = walk.files.len() + walk.pre_skipped.len();
    // Each path allocated once, here: a resolved edge's target is a clone
    // of one of these, never a fresh copy (see `EdgeTarget`).
    let known: KnownPaths = walk
        .files
        .iter()
        .map(|file| Arc::from(file.rel.as_str()))
        .collect();
    // Built once, shared by every file: see `RustModuleIndex`'s doc for why
    // building it per `use crate::...` resolution (the shape this ticket's
    // third review round found) is the defect, not a detail.
    let rust_index = RustModuleIndex::build(&known);

    let mut modules: Vec<Module> = Vec::new();
    let mut skipped: Vec<SkippedFile> = walk.pre_skipped;
    let mut files_parsed_clean = 0usize;
    let mut files_parsed_with_errors = 0usize;

    for file in &walk.files {
        match process_file(file, &known, &rust_index, options, &root_canon) {
            Ok(module) => {
                if module.parsed_with_errors {
                    files_parsed_with_errors += 1;
                } else {
                    files_parsed_clean += 1;
                }
                modules.push(module);
            }
            Err(reason) => skipped.push(SkippedFile {
                path: file.rel.clone(),
                reason,
            }),
        }
    }

    link_sibling_test_files(&mut modules);
    // Its own budget, not a reused per-file deadline: this stage runs once,
    // after every file's `Module` already exists, not per file, so it gets
    // one fresh deadline sized by the same `file_timeout` configuration
    // value rather than inheriting an `Instant` computed for (and mostly
    // spent by) something else. See `attach_spec_citations`'s doc and the
    // module doc's "Time" bound.
    let spec_deadline = Instant::now() + options.file_timeout;
    let spec = attach_spec_citations(&root_canon, &mut modules, options, spec_deadline);

    for module in &mut modules {
        module
            .interfaces
            .sort_by(|a, b| (a.line, &a.name).cmp(&(b.line, &b.name)));
        module
            .dependency_edges
            .sort_by(|a, b| (a.external, &a.to).cmp(&(b.external, &b.to)));
        module
            .entry_points
            .sort_by(|a, b| (a.line, &a.name).cmp(&(b.line, &b.name)));
        module.covering_tests.sort();
        module.covering_tests.dedup();
        // By the texts the indices name, not the indices themselves, which
        // are in first-cited order.
        module.spec_sections.sort_by(|a, b| {
            spec_citation_sort_key(&spec, a).cmp(&spec_citation_sort_key(&spec, b))
        });
        module.spec_sections.dedup();
    }
    modules.sort_by(|a, b| a.path.cmp(&b.path));
    skipped.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(CodeMap {
        modules,
        coverage: Coverage {
            files_seen,
            files_parsed_clean,
            files_parsed_with_errors,
            files_skipped: skipped,
            spec_citation_scan: spec.status,
            spec_docs_skipped: spec.skipped,
        },
        spec_docs: spec.docs,
        spec_headings: spec.headings,
    })
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// One candidate file the walk found: a file (or a symlink resolving to one
/// inside the root) with no verdict yet about whether it can be parsed.
struct CandidateFile {
    /// Absolute path, safe to open.
    abs: PathBuf,
    /// Root-relative, forward-slash path, used as the module's identity.
    rel: String,
    /// Whether this entry was a symlink when the walk classified it. Read
    /// again in `process_file`, which opens this path differently depending
    /// on the answer: a direct entry is opened refusing any symlink the path
    /// might name by the time `process_file` runs (nothing here was ever
    /// meant to follow one), while a symlink entry is re-validated
    /// immediately before its own open, narrowing (not eliminating) the
    /// window between this classification and that read. Both opens are
    /// non-blocking and decided by the handle. See the module doc's
    /// "Threat model".
    was_symlink: bool,
}

/// What [`walk_repository`] found.
struct Walk {
    /// Every candidate file, sorted by `rel`.
    files: Vec<CandidateFile>,
    /// Files decided during the walk itself (symlinks outside the root),
    /// before language classification.
    pre_skipped: Vec<SkippedFile>,
}

/// Walks `root_canon` (already canonicalized) for every file, following no
/// symlinked directory anywhere, descending no directory named `.git`. See
/// the module doc's "Untrusted input" bounds.
fn walk_repository(root_canon: &Path) -> Walk {
    let mut files: Vec<CandidateFile> = Vec::new();
    let mut pre_skipped: Vec<SkippedFile> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![root_canon.to_path_buf()];

    while let Some(dir) = pending.pop() {
        let read_dir = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                // The directory itself was seen (its parent's `read_dir`
                // named it); what it contains cannot be enumerated, so that
                // is recorded, not guessed at or silently dropped along with
                // everything under it. The root directory's own
                // unreadability is caught earlier, in
                // `build_code_map_with_options`, as a hard error rather than
                // a skip; this branch is for a subdirectory.
                admit_skip(
                    root_canon,
                    &dir,
                    SkipReason::Unreadable(err.to_string()),
                    &mut pre_skipped,
                );
                continue;
            }
        };
        let mut entries: Vec<fs::DirEntry> = Vec::new();
        for entry in read_dir {
            match entry {
                Ok(entry) => entries.push(entry),
                Err(err) => {
                    // One entry in an otherwise-readable directory could not
                    // be read (a race, a permissions edge case): recorded
                    // against the directory, since the entry's own name is
                    // exactly what failed to come back.
                    admit_skip(
                        root_canon,
                        &dir,
                        SkipReason::Unreadable(err.to_string()),
                        &mut pre_skipped,
                    );
                }
            }
        }
        entries.sort_by_key(std::fs::DirEntry::file_name);

        for entry in entries {
            let name = entry.file_name();
            let abs = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    admit_skip(
                        root_canon,
                        &abs,
                        SkipReason::Unreadable(err.to_string()),
                        &mut pre_skipped,
                    );
                    continue;
                }
            };

            if file_type.is_symlink() {
                // `stat`, never `open`: deciding what a symlink's target is
                // must never itself open it (a FIFO's `open` can block with
                // no writer present). `fs::metadata` follows the link but is
                // a `stat`-family call.
                match fs::metadata(&abs) {
                    Ok(target_meta) if target_meta.is_dir() => {
                        // Never descended: see the module doc's cycle-safety
                        // note. Recorded, not silently absent: this entry is
                        // one the walk saw and decided about, unlike an
                        // ordinary subdirectory, which is not a "file"
                        // candidate at all.
                        admit_skip(
                            root_canon,
                            &abs,
                            SkipReason::SymlinkedDirectory,
                            &mut pre_skipped,
                        );
                    }
                    Ok(target_meta) if target_meta.is_file() => match fs::canonicalize(&abs) {
                        Ok(resolved) if resolved.starts_with(root_canon) => {
                            if resolved_path_enters_git(root_canon, &resolved) {
                                admit_skip(
                                    root_canon,
                                    &abs,
                                    SkipReason::GitMetadata,
                                    &mut pre_skipped,
                                );
                            } else {
                                // Reported under the link's own path, not the
                                // target's: a link and its target are two
                                // distinct files, and only one of them is
                                // this entry.
                                admit_file(root_canon, &abs, true, &mut files, &mut pre_skipped);
                            }
                        }
                        _ => admit_skip(
                            root_canon,
                            &abs,
                            SkipReason::SymlinkOutsideRoot,
                            &mut pre_skipped,
                        ),
                    },
                    Ok(_) => {
                        // Not a directory, not a regular file: a FIFO, a
                        // socket, a device. Never opened.
                        admit_skip(
                            root_canon,
                            &abs,
                            SkipReason::NotARegularFile,
                            &mut pre_skipped,
                        );
                    }
                    Err(err) => {
                        admit_skip(
                            root_canon,
                            &abs,
                            SkipReason::Unreadable(err.to_string()),
                            &mut pre_skipped,
                        );
                    }
                }
            } else if file_type.is_dir() {
                if is_git_directory_name(&name) {
                    continue;
                }
                pending.push(abs);
            } else if file_type.is_file() {
                admit_file(root_canon, &abs, false, &mut files, &mut pre_skipped);
            } else {
                admit_skip(
                    root_canon,
                    &abs,
                    SkipReason::NotARegularFile,
                    &mut pre_skipped,
                );
            }
        }
    }

    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    pre_skipped.sort_by(|a, b| a.path.cmp(&b.path));
    Walk { files, pre_skipped }
}

/// `abs`, relative to `root`, with forward slashes. `None` when `abs` is not
/// under `root` or is not valid UTF-8 (both would mean this module cannot
/// name the file deterministically, so it is treated as unreachable rather
/// than guessed at).
fn rel_path_string(root: &Path, abs: &Path) -> Option<String> {
    let rel = abs.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        parts.push(component.as_os_str().to_str()?.to_owned());
    }
    Some(parts.join("/"))
}

/// [`rel_path_string`], but never `None`: invalid bytes are replaced (Rust's
/// usual lossy rendering), for display in a [`SkippedFile::path`] only. A
/// path built this way is never used to open a file.
fn lossy_rel_path_string(root: &Path, abs: &Path) -> String {
    let rel = abs.strip_prefix(root).unwrap_or(abs);
    let parts: Vec<String> = rel
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join("/")
}

/// Whether a directory entry's name is literally `.git`: the one rule for
/// version-control metadata, which neither the walk ([`walk_repository`])
/// nor the `spec/` scan ([`list_markdown`]) ever descends. Exact and
/// case-sensitive, like the `spec` match, on every platform.
fn is_git_directory_name(name: &std::ffi::OsStr) -> bool {
    name == ".git"
}

/// Whether `resolved` (already confirmed inside `root_canon`) has a path
/// component literally named `.git`. Only meaningful for a symlink target: a
/// direct (non-symlink) entry under `.git` is never reached at all, because
/// [`walk_repository`] never descends a directory named `.git`.
fn resolved_path_enters_git(root_canon: &Path, resolved: &Path) -> bool {
    resolved
        .strip_prefix(root_canon)
        .map(|rel| rel.components().any(|c| c.as_os_str() == ".git"))
        .unwrap_or(false)
}

/// Admits `abs` as a candidate file under its own path, or, when its path is
/// not valid UTF-8, records it as [`SkipReason::NonUtf8Name`] instead of
/// dropping it with neither outcome.
fn admit_file(
    root_canon: &Path,
    abs: &Path,
    was_symlink: bool,
    files: &mut Vec<CandidateFile>,
    pre_skipped: &mut Vec<SkippedFile>,
) {
    match rel_path_string(root_canon, abs) {
        Some(rel) => files.push(CandidateFile {
            abs: abs.to_path_buf(),
            rel,
            was_symlink,
        }),
        None => pre_skipped.push(SkippedFile {
            path: lossy_rel_path_string(root_canon, abs),
            reason: SkipReason::NonUtf8Name,
        }),
    }
}

/// Records `abs` as skipped for `reason`. Its exact path is used when valid
/// UTF-8, its lossy rendering otherwise, so the record itself is never lost
/// to the same encoding problem a different reason might be reporting.
fn admit_skip(
    root_canon: &Path,
    abs: &Path,
    reason: SkipReason,
    pre_skipped: &mut Vec<SkippedFile>,
) {
    let path =
        rel_path_string(root_canon, abs).unwrap_or_else(|| lossy_rel_path_string(root_canon, abs));
    pre_skipped.push(SkippedFile { path, reason });
}

// ---------------------------------------------------------------------------
// Per-file processing
// ---------------------------------------------------------------------------

/// The raw `open(2)` flag values this module passes through
/// [`std::os::unix::fs::OpenOptionsExt::custom_flags`], on the platforms they
/// were verified for. `libc` is not a dependency of this crate (`ori-memory`'s
/// `Cargo.toml` does not list it, and CLAUDE.md rule 6 makes adding one an
/// escalation this ticket does not make on its own), so these are the
/// platform's own header constants, not `libc`'s named ones.
///
/// Verified, by platform: macOS (`O_NOFOLLOW` 0x0100, `O_NONBLOCK` 0x0004),
/// compiled and run on this ticket's own development machine (arm64), and
/// read again from the SDK's own `sys/fcntl.h` in round 4; Linux x86_64
/// (`O_NOFOLLOW` 0o400000, `O_NONBLOCK` 0o4000) and Linux aarch64
/// (`O_NOFOLLOW` 0o100000, the same `O_NONBLOCK`), each compiled and run in
/// round 3 inside a container for that platform, printing the macros from its
/// own `<fcntl.h>`, and cross-checked in round 4 against the `libc` crate's
/// source for both the glibc and musl targets (0x20000, 0x8000 and 2048,
/// the same three numbers). Linux's `O_NOFOLLOW` is not
/// architecture-uniform: `arch/arm64/include/uapi/asm/fcntl.h` overrides the
/// `asm-generic` value x86_64 and most other architectures use, which is
/// exactly why this is two Linux constants, not one. Every other unix takes
/// the fallback in [`open_regular_file_no_follow`] and
/// [`open_regular_file_following`]; see their docs for what it keeps and what
/// it cannot.
#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
))]
mod raw_open_flags {
    #[cfg(target_os = "macos")]
    pub(super) const O_NOFOLLOW: i32 = 0x0100;
    #[cfg(target_os = "macos")]
    pub(super) const O_NONBLOCK: i32 = 0x0004;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(super) const O_NOFOLLOW: i32 = 0o400_000;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(super) const O_NONBLOCK: i32 = 0o4_000;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub(super) const O_NOFOLLOW: i32 = 0o100_000;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub(super) const O_NONBLOCK: i32 = 0o4_000;
}

/// Opens `path` for reading, never blocking in the open, and refusing to
/// follow a symlink the path's final component names: the open for a plain,
/// non-symlink entry the walk already classified, and for every `spec/`
/// document. The entry is meant to stay a plain file through to the read
/// that follows, and `O_NOFOLLOW` is what stops the open from silently
/// reading through a symlink that replaced the path afterward (this ticket's
/// third review round reproduced exactly that swap). `O_NONBLOCK` is what
/// stops the open itself from waiting: `open(2)` on a FIFO with no writer
/// blocks until one appears, and a path that was a regular file when the walk
/// classified it can be a FIFO by the time it is opened. With `O_NONBLOCK`
/// the open of a FIFO returns at once, and [`open_regular_file`] then refuses
/// the handle by its own metadata before anything reads from it. On a regular
/// file `O_NONBLOCK` changes nothing about the read that follows.
///
/// On a unix platform outside the verified set (see [`raw_open_flags`]), no
/// flag value is trusted, so this takes a `stat`-family call on the path
/// first (`symlink_metadata`, which does not follow a final symlink and never
/// opens anything) and refuses anything that is not a regular file before a
/// plain, blocking open. That keeps a FIFO or a symlink that is already in
/// place when this runs from ever being opened, which is every static case,
/// and is what the review of this ticket's third round found missing from its
/// fallback (a FIFO named `*.md` under `spec/` hung it with no race at all).
/// It does not keep a FIFO swapped in between that `stat` and the open from
/// blocking the open: on those platforms, "never hangs, even under a race"
/// is not claimed. See the module doc's threat model.
#[cfg(unix)]
fn open_regular_file_no_follow(path: &Path) -> io::Result<fs::File> {
    #[cfg(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    ))]
    {
        use raw_open_flags::{O_NOFOLLOW, O_NONBLOCK};
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(O_NOFOLLOW | O_NONBLOCK)
            .open(path)
    }
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() {
            return Err(io::Error::other(
                "refusing to open a symlink (checked by a stat before the open: no verified \
                 O_NOFOLLOW on this platform)",
            ));
        }
        if !meta.is_file() {
            return Err(io::Error::other(
                "not a regular file (checked by a stat before the open: no verified O_NONBLOCK \
                 on this platform)",
            ));
        }
        fs::File::open(path)
    }
}

/// Opens `path` for reading, never blocking in the open, following a symlink
/// the path's final component names: the open for an entry the walk
/// classified as a symlink, which [`process_file`] has just re-validated as
/// resolving inside the root and outside `.git`. This is the open this
/// ticket's round-3 fix left blocking (a plain `fs::File::open`, with the
/// type check moved to after it), so a symlink whose in-root target was
/// replaced by a FIFO after the walk hung [`build_code_map`] forever in
/// `open(2)`; the review of this ticket's third round reproduced it three
/// times out of three.
/// `O_NONBLOCK` alone, without `O_NOFOLLOW`, is the fix: the link is still
/// followed, as it must be, but the open of whatever it now names returns at
/// once, and [`open_regular_file`] decides by the handle.
///
/// Outside the verified set (see [`raw_open_flags`]), the same fallback as
/// [`open_regular_file_no_follow`], through `fs::metadata` (which follows the
/// link, and is still a `stat`, never an open): every static case is refused
/// before the open; a swap between that `stat` and the open is not.
#[cfg(unix)]
fn open_regular_file_following(path: &Path) -> io::Result<fs::File> {
    #[cfg(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    ))]
    {
        use raw_open_flags::O_NONBLOCK;
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(O_NONBLOCK)
            .open(path)
    }
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        let meta = fs::metadata(path)?;
        if !meta.is_file() {
            return Err(io::Error::other(
                "not a regular file (checked by a stat before the open: no verified O_NONBLOCK \
                 on this platform)",
            ));
        }
        fs::File::open(path)
    }
}

/// [`open_regular_file_no_follow`], for Windows: opens the reparse point
/// itself (`FILE_FLAG_OPEN_REPARSE_POINT`) rather than transparently
/// following it, so a path that became a symlink or junction after the walk
/// classified it as a plain file yields a handle whose own metadata this
/// module can recognise as not a regular file
/// ([`SkipReason::NotARegularFile`]), instead of silently opening whatever
/// the reparse point now names. Checked for both `is_file()` and
/// `is_symlink()` in [`open_regular_file`], not `is_file()` alone, as a
/// second, independent guard: this constant's effect on the metadata Rust's
/// `std` reports for a reparse-point handle is documented behaviour this
/// module could not run and observe on an actual Windows machine.
///
/// What this open does with a FIFO-like target: Windows has no FIFO in the
/// file system's namespace, so no directory entry in a checkout can be one.
/// The nearest things are a named pipe, which lives in the separate
/// `\\.\pipe\` device namespace, and a device (a console, a serial port).
/// A path reaches either only by naming it, through a symlink target or a
/// reserved device name, and neither blocks this open waiting for a peer
/// the way a unix FIFO's `open(2)` does: `CreateFileW` on a named pipe
/// with no free instance fails at once (`ERROR_PIPE_BUSY`), since waiting for
/// one is `WaitNamedPipe`'s job, which this module never calls. What could
/// wait is a read from such a handle, which is why nothing is read before
/// [`open_regular_file`] has seen `is_file()` on the handle's own metadata.
/// Whether Rust's `std` reports `is_file() == false` for a pipe or console
/// handle (it derives the file type from attributes and the reparse tag, not
/// from `GetFileType`) is the one link in that chain this ticket could only
/// compile, never run.
#[cfg(windows)]
fn open_regular_file_no_follow(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

/// [`open_regular_file_following`], for Windows: a plain open, which follows
/// a symlink the way the unix variant does. What it does with a FIFO-like
/// target is what [`open_regular_file_no_follow`]'s Windows doc says: the
/// open does not wait for a peer, and nothing is read until the handle's own
/// metadata says `is_file()`.
#[cfg(windows)]
fn open_regular_file_following(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}

/// Opens one piece of repository content for reading and decides, by the
/// opened handle's own metadata, whether it is a regular file: the one
/// function every read of repository content goes through (a plain walk
/// entry, a symlink walk entry, and a `spec/` document alike). The open never
/// blocks on the verified platforms (see [`open_regular_file_no_follow`] and
/// [`open_regular_file_following`]), and the decision is made on the handle,
/// not on a path that could name something else by the time it is read, so no
/// FIFO, device or swapped-in file can make a caller wait in the open or in
/// the read that follows: a caller only ever reads a handle this has already
/// seen to be a regular file, through [`read_capped`].
///
/// `follow_final_symlink` is `true` only for an entry the walk classified as
/// a symlink and the caller has just re-validated; everything else is opened
/// refusing one.
fn open_regular_file(
    path: &Path,
    follow_final_symlink: bool,
) -> std::result::Result<(fs::File, fs::Metadata), SkipReason> {
    // Outside the verified set, the open functions below refuse anything
    // that is not a regular file by a `stat` before their open (see their
    // docs), but can only say so through an `io::Error`. The same `stat`,
    // taken here first, is what lets that refusal come back as
    // `NotARegularFile` on those platforms too, the reason the verified
    // platforms give from the handle, rather than as `Unreadable`.
    #[cfg(all(
        unix,
        not(any(
            target_os = "macos",
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
        ))
    ))]
    {
        let before = if follow_final_symlink {
            fs::metadata(path)
        } else {
            fs::symlink_metadata(path)
        }
        .map_err(|err| SkipReason::Unreadable(err.to_string()))?;
        if !before.is_file() {
            return Err(SkipReason::NotARegularFile);
        }
    }
    let opened = if follow_final_symlink {
        open_regular_file_following(path)
    } else {
        open_regular_file_no_follow(path)
    }
    .map_err(|err| SkipReason::Unreadable(err.to_string()))?;
    let meta = opened
        .metadata()
        .map_err(|err| SkipReason::Unreadable(err.to_string()))?;
    // Both conditions: on Windows, a handle opened with
    // `FILE_FLAG_OPEN_REPARSE_POINT` onto a path that became a symlink or
    // junction is expected to report `is_symlink()` on its own metadata,
    // which `is_file()` alone might or might not also catch (behaviour this
    // module could not observe on an actual Windows machine; see
    // `open_regular_file_no_follow`'s doc).
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(SkipReason::NotARegularFile);
    }
    Ok((opened, meta))
}

/// Reads at most `cap + 1` bytes from `opened`, through
/// [`std::io::Read::take`], so a file that grows, or that misreports its
/// length, is still cut off: whether it was over the cap is the caller's
/// check on the returned length, never on a `stat`ed size. See the module
/// doc's "Size" bound.
///
/// `stated_len`, the handle's own reported length, only sizes the buffer up
/// front, and never past `cap + 1`: without it, `read_to_end` grows the
/// buffer by doubling, so a read of just over 8 MiB could hold a 16 MiB
/// allocation, and briefly both it and the 8 MiB one it replaced. A length
/// that lies costs at most one `cap + 1` allocation; it is never what
/// decides anything.
fn read_capped(opened: fs::File, cap: u64, stated_len: u64) -> io::Result<Vec<u8>> {
    let limit = cap.saturating_add(1);
    let mut bytes = Vec::with_capacity(usize::try_from(stated_len.min(limit)).unwrap_or(0));
    opened.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn language_of(rel: &str) -> Option<Language> {
    let ext = rel.rsplit('.').next()?;
    match ext {
        "rs" => Some(Language::Rust),
        "ts" | "tsx" => Some(Language::TypeScript),
        "py" | "pyi" => Some(Language::Python),
        "go" => Some(Language::Go),
        _ => None,
    }
}

/// The wall-clock budget one file's whole step (open, read, parse,
/// extraction and test detection together) gets:
/// [`CodeMapOptions::file_timeout`] plus a size-proportional allowance for a
/// file over 256 KiB (one second per MiB above that, computed
/// proportionally, not a whole extra second added to every file regardless
/// of size); nothing beyond `file_timeout` itself for a file at or under
/// that threshold, which this module's own small fixtures, and any
/// ordinarily-sized source file, always are.
///
/// Why an allowance beyond the fixed bound: a fixed bound checked once does
/// not distinguish an ordinary large file from a pathological one, and this
/// ticket's third review round measured that an *unoptimized* build's parse
/// and extraction together can need several times as long as a release
/// build's for the identical input, several seconds longer for a file
/// already within the default 5 second budget. Without an allowance, such a
/// file becomes `TimedOut` only in a debug build (`cargo test`,
/// `scripts/gates.sh` and an unreleased `cargo build` all use one) while the
/// identical file maps clean in release, which is not a bound working as
/// documented, it is the bound being wrong for half the builds that use it.
///
/// The allowance is deliberately generous and is not a throughput promise;
/// it exists so a legitimate large file is not mistaken for a pathological
/// one, not so an actually pathological one passes. It does not undermine
/// the other bound it sits next to: this ticket's quadratic-shape
/// reproductions (nested Rust items, `use crate::` paths with huge segment
/// counts) cost far more time than one second per MiB of the *small* inputs
/// that trigger them, so a fixed floor plus a modest per-MiB allowance is
/// nowhere near enough budget for either to pass.
fn per_file_budget(options: &CodeMapOptions, file_bytes: u64) -> Duration {
    /// Below this, the allowance is exactly zero, not a rounded-up sliver of
    /// one: an ordinary small file (this module's own fixtures included)
    /// gets exactly `options.file_timeout`, unchanged, which is also what
    /// keeps a test-injected `Duration::ZERO` an exact, deterministic "the
    /// deadline is already past" for every file this size or smaller,
    /// rather than a few proportional microseconds that would make such a
    /// test's outcome depend on real timing.
    const THRESHOLD_BYTES: u64 = 256 * 1024;
    /// One second of extra budget per MiB above the threshold, computed
    /// proportionally (integer microseconds, not a whole MiB rounded up),
    /// so a file just over the threshold gets a correspondingly small
    /// allowance rather than jumping straight to a full extra second.
    const PER_MIB_ALLOWANCE_MICROS: u64 = 1_000_000;
    const MIB_BYTES: u64 = 1024 * 1024;

    if file_bytes <= THRESHOLD_BYTES {
        return options.file_timeout;
    }
    let allowance_micros = file_bytes.saturating_mul(PER_MIB_ALLOWANCE_MICROS) / MIB_BYTES;
    options
        .file_timeout
        .saturating_add(Duration::from_micros(allowance_micros))
}

/// Every root-relative path the walk admitted as a candidate file, each
/// allocated once per [`build_code_map`] call. Import resolution looks a
/// path up here and, on a match, clones the one shared allocation into the
/// edge's [`EdgeTarget`], so a resolved edge never copies a path.
type KnownPaths = HashSet<Arc<str>>;

/// Turns one candidate file into a [`Module`], or the [`SkipReason`] it was
/// skipped for.
fn process_file(
    file: &CandidateFile,
    known: &KnownPaths,
    rust_index: &RustModuleIndex,
    options: &CodeMapOptions,
    root_canon: &Path,
) -> std::result::Result<Module, SkipReason> {
    let language = language_of(&file.rel).ok_or(SkipReason::UnsupportedLanguage)?;

    // Open first, decide by the opened handle, not by a path-based `stat`
    // followed by a separate, later `open`: this ticket's third review round
    // reproduced a swap timed into exactly that gap (a plain file replaced
    // by a symlink out of the root, or into `.git`, after the walk but
    // before this call), which the two-syscall form cannot see. Both
    // branches open without blocking and decide by the handle
    // (`open_regular_file`), so a FIFO or device swapped in after the walk,
    // under either kind of entry, is refused rather than waited on; the
    // review of the third round found the symlink branch still opening
    // blocking, and hung it forever that way. What each branch does beyond
    // that:
    let (opened, meta) = if file.was_symlink {
        // This entry was already a symlink when the walk validated it
        // in-root and outside `.git`. Re-validating immediately before this
        // open narrows that window from "the whole walk plus every earlier
        // file" down to "between this canonicalize and this open"; it does
        // not close it (a swap timed into that narrower gap is not
        // detected, and is outside this module's threat model: see the
        // module doc).
        let resolved =
            fs::canonicalize(&file.abs).map_err(|err| SkipReason::Unreadable(err.to_string()))?;
        if !resolved.starts_with(root_canon) {
            return Err(SkipReason::SymlinkOutsideRoot);
        }
        if resolved_path_enters_git(root_canon, &resolved) {
            return Err(SkipReason::GitMetadata);
        }
        open_regular_file(&file.abs, true)?
    } else {
        // This entry was a plain, non-symlink file when the walk saw it: it
        // was never meant to follow a symlink at all, so if the path now
        // names one (swapped in after the walk), the open itself refuses it
        // on every platform this was verified on; see
        // `open_regular_file_no_follow`'s doc for which those are.
        open_regular_file(&file.abs, false)?
    };

    let deadline = Instant::now() + per_file_budget(options, meta.len());

    // The cap is enforced on bytes actually read (`read_capped`), not on
    // `meta.len()` taken on trust: a file that grows after this `fstat`, or
    // that misreports its length, is still cut off at `cap + 1` bytes.
    let cap = options.max_file_bytes;
    let bytes = read_capped(opened, cap, meta.len())
        .map_err(|err| SkipReason::Unreadable(err.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(SkipReason::TooLarge {
            bytes: bytes.len() as u64,
            cap,
        });
    }

    if bytes.contains(&0u8) {
        return Err(SkipReason::Binary);
    }
    let Ok(source) = std::str::from_utf8(&bytes) else {
        return Err(SkipReason::Binary);
    };

    let tsx = file.rel.ends_with(".tsx");
    let tree = parse_bounded(language, tsx, source, deadline).ok_or(SkipReason::TimedOut)?;
    let root = tree.root_node();
    let parsed_with_errors = root.has_error();

    let (mut extracted, extraction_completed) = match language {
        Language::Rust => extract_rust(
            root,
            source.as_bytes(),
            &file.rel,
            known,
            rust_index,
            deadline,
        ),
        Language::TypeScript => {
            extract_typescript(root, source.as_bytes(), &file.rel, known, deadline)
        }
        Language::Python => extract_python(root, source.as_bytes(), &file.rel, known, deadline),
        Language::Go => extract_go(root, source.as_bytes(), deadline),
    };

    if let Some(shebang) = shebang_entry_point(source) {
        extracted.entry_points.push(shebang);
    }

    let (covering_tests, tests_completed) = match language {
        Language::Rust => rust_test_names(root, source.as_bytes(), deadline),
        Language::TypeScript => typescript_test_names(root, source.as_bytes(), deadline),
        Language::Python => python_test_names(root, source.as_bytes(), deadline),
        Language::Go => go_test_names(root, source.as_bytes(), deadline),
    };

    // The deadline covers this whole function, not only the parse: if either
    // extraction stage stopped early, or time simply ran out while this
    // function did its own bookkeeping around them, the file is `TimedOut`,
    // never reported as a `Module` whose interfaces, edges or tests are
    // silently incomplete.
    if !extraction_completed || !tests_completed || Instant::now() >= deadline {
        return Err(SkipReason::TimedOut);
    }

    Ok(Module {
        path: file.rel.clone(),
        language,
        parsed_with_errors,
        interfaces: extracted.interfaces,
        dependency_edges: extracted.edges.edges,
        dependency_edges_truncated: extracted.edges.truncated,
        entry_points: extracted.entry_points,
        covering_tests,
        spec_sections: Vec::new(),
        spec_sections_truncated: false,
    })
}

/// Parses `source` with a wall-clock `deadline`, returning `None` when
/// tree-sitter's own progress callback cancels the parse before it finishes.
/// See the module doc's "Time" bound.
fn parse_bounded(
    language: Language,
    tsx: bool,
    source: &str,
    deadline: Instant,
) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    let grammar = match (language, tsx) {
        (Language::Rust, _) => tree_sitter_rust::LANGUAGE.into(),
        (Language::TypeScript, true) => tree_sitter_typescript::LANGUAGE_TSX.into(),
        (Language::TypeScript, false) => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        (Language::Python, _) => tree_sitter_python::LANGUAGE.into(),
        (Language::Go, _) => tree_sitter_go::LANGUAGE.into(),
    };
    if parser.set_language(&grammar).is_err() {
        return None;
    }

    let bytes = source.as_bytes();
    let mut cancel = move |_state: &ParseState| Instant::now() >= deadline;
    let parse_options = ParseOptions::new().progress_callback(&mut cancel);
    parser.parse_with_options(
        &mut |offset, _point| bytes.get(offset..).unwrap_or(&[]),
        None,
        Some(parse_options),
    )
}

/// A shebang line is checked the same way for every language: the file's
/// first two bytes are `#!`.
fn shebang_entry_point(source: &str) -> Option<EntryPoint> {
    if !source.starts_with("#!") {
        return None;
    }
    let line = source.lines().next().unwrap_or("#!").to_owned();
    Some(EntryPoint {
        name: line,
        line: 1,
    })
}

fn text<'a>(node: Node<'a>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

fn line_of(node: Node) -> usize {
    node.start_position().row + 1
}

fn dir_components(rel: &str) -> Vec<String> {
    let mut parts: Vec<String> = rel.split('/').map(str::to_owned).collect();
    parts.pop();
    parts
}

/// Joins `base` (directory components) with a `/`-separated relative
/// specifier, resolving `.` and `..`. `None` when a `..` climbs above `base`.
fn normalize_join(base: &[String], relative: &str) -> Option<String> {
    let mut stack: Vec<String> = base.to_vec();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            other => stack.push(other.to_owned()),
        }
    }
    Some(stack.join("/"))
}

/// A generic, iterative (never recursive) pre-order walk of every node under
/// `root`, inclusive. Used only for cross-cutting scans (inline test
/// detection) that must not be limited to top-level items.
///
/// Every step here is O(1) amortized: `node.walk()` seeds a
/// [`tree_sitter::TreeCursor`] directly at `node` (`ts_tree_cursor_init`
/// pushes one stack entry; it does not walk down from the tree's root), and
/// `node.children(&mut cursor)` advances that cursor one sibling at a time.
/// Nothing here calls [`Node::prev_sibling`], [`Node::next_sibling`] or
/// [`Node::parent`], which is deliberate: in tree-sitter 0.25.10 each of
/// those starts from `ts_node_parent`, which walks down from the tree's
/// root, so a call from a node at depth D costs O(D), and D of them (one per
/// node at that depth) cost O(D^2). `rust_marked_test`'s use of
/// [`Node::prev_sibling`] was exactly that, before this ticket's adversarial
/// review measured it (see the report): a 320 KB file of nested `fn` items
/// took two to three minutes to extract, parsed clean, outside the parse
/// timeout, because nothing bounded the extraction step that followed.
/// [`for_each_sibling_group`] is the replacement, for the one caller that
/// needed sibling order.
///
/// Returns `false` when `deadline` passed before every node was visited, so
/// a caller can tell an aborted scan apart from a complete one; see the
/// module doc's "Time" bound. This is now the *second* line of defense
/// against a slow extraction, not the only one: the cost is O(1) amortized
/// per node regardless. The deadline is checked before each node is
/// visited and before each child is gathered, since one node can have
/// millions of children (see [`gather_children`]).
fn for_each_node<'a>(root: Node<'a>, deadline: Instant, mut visit: impl FnMut(Node<'a>)) -> bool {
    let mut stack: Vec<Node<'a>> = vec![root];
    while let Some(node) = stack.pop() {
        if Instant::now() >= deadline {
            return false;
        }
        visit(node);
        let Some(mut children) = gather_children(node, deadline) else {
            return false;
        };
        children.reverse();
        stack.extend(children);
    }
    true
}

/// `node`'s children in sibling order, or `None` when `deadline` passed
/// while gathering them, checked before each one. Gathering is one linear
/// pass, but one parent can have millions of children (a file of nothing
/// but `#[a]` lines has one per line), and until round 6 each traversal
/// gathered them all with no check, as one step between two checks.
fn gather_children<'a>(node: Node<'a>, deadline: Instant) -> Option<Vec<Node<'a>>> {
    let mut cursor = node.walk();
    let mut children = Vec::new();
    for child in node.children(&mut cursor) {
        if Instant::now() >= deadline {
            return None;
        }
        children.push(child);
    }
    Some(children)
}

/// Like [`for_each_node`], but the callback receives one parent's children in
/// their sibling order, all at once, so it can track "the previous sibling"
/// itself in O(1) per step instead of through [`Node::prev_sibling`] (see
/// [`for_each_node`]'s doc for why that matters). Every node is still visited
/// exactly once (as a member of its parent's children), and every parent is
/// still descended into, nested ones included, because every child is pushed
/// onto the same traversal stack. Returns `false` when `deadline` passed
/// before every parent's children were visited, checked once per parent
/// and once per child gathered ([`gather_children`]); a caller whose
/// per-child work is not constant checks it again inside `visit` (as
/// [`rust_test_names`] does).
fn for_each_sibling_group<'a>(
    root: Node<'a>,
    deadline: Instant,
    mut visit: impl FnMut(&[Node<'a>]),
) -> bool {
    let mut stack: Vec<Node<'a>> = vec![root];
    while let Some(node) = stack.pop() {
        if Instant::now() >= deadline {
            return false;
        }
        let Some(children) = gather_children(node, deadline) else {
            return false;
        };
        visit(&children);
        let mut reversed = children;
        reversed.reverse();
        stack.extend(reversed);
    }
    true
}

/// What one language's top-level scan produced.
struct Extracted {
    interfaces: Vec<Interface>,
    edges: EdgeSink,
    entry_points: Vec<EntryPoint>,
}

impl Extracted {
    fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            edges: EdgeSink::new(),
            entry_points: Vec::new(),
        }
    }
}

/// The most dependency edges one module keeps. Past it, extraction stops
/// recording edges for that module and sets
/// [`Module::dependency_edges_truncated`]: one import name costs about two
/// bytes of source (`import a,a,...` in Python, `use {a,a,...};` in Rust),
/// so a file under the size cap could otherwise hold millions of edges. Far
/// above what hand-written code imports; a generated file past it is
/// reported truncated, not silently cut. See the module doc's "Memory".
const MAX_DEPENDENCY_EDGES_PER_MODULE: usize = 4096;

/// The longest unresolved edge target text one edge keeps. Longer text is
/// cut to this many bytes, back to a UTF-8 character boundary, and marked
/// [`DependencyEdge::to_truncated`]. A resolved target is never cut: it is
/// a whole path, shared rather than copied (see [`EdgeTarget`]).
const MAX_EDGE_TARGET_BYTES: usize = 1024;

/// Where one module's dependency edges are collected while its file is
/// extracted: every edge-producing site in every language goes through
/// [`EdgeSink::push_resolved`] or [`EdgeSink::push_external`], which are
/// what hold the edge cap and the target text cap, so no site can keep more
/// than they allow, and none holds a copy of the importing file's path.
struct EdgeSink {
    edges: Vec<DependencyEdge>,
    /// Set the first time an edge that exists is refused because `limit`
    /// edges are already held, never merely because `limit` were reached.
    truncated: bool,
    /// [`MAX_DEPENDENCY_EDGES_PER_MODULE`] for every module this crate
    /// maps. A field, not the constant read in place, only so that a test
    /// can drive one statement's extraction past the cap and time it.
    limit: usize,
}

impl EdgeSink {
    fn new() -> Self {
        Self::with_limit(MAX_DEPENDENCY_EDGES_PER_MODULE)
    }

    fn with_limit(limit: usize) -> Self {
        Self {
            edges: Vec::new(),
            truncated: false,
            limit,
        }
    }

    /// Whether one more edge fits. `false` marks the module truncated: the
    /// caller had an edge to record and could not.
    fn has_room(&mut self) -> bool {
        if self.edges.len() < self.limit {
            true
        } else {
            self.truncated = true;
            false
        }
    }

    /// Records an edge to `target`, one of the walk's own paths, by cloning
    /// its shared allocation, never copying its text. `false` when the sink
    /// is full, and the caller should stop producing edges.
    fn push_resolved(&mut self, target: &Arc<str>) -> bool {
        if !self.has_room() {
            return false;
        }
        self.edges.push(DependencyEdge {
            to: EdgeTarget(Arc::clone(target)),
            external: false,
            to_truncated: false,
        });
        true
    }

    /// Records an unresolved edge whose target is `text`, keeping at most
    /// [`MAX_EDGE_TARGET_BYTES`] of it. `false` when the sink is full, and
    /// the caller should stop producing edges.
    fn push_external(&mut self, text: &str) -> bool {
        if !self.has_room() {
            return false;
        }
        let kept = truncate_at_char_boundary(text, MAX_EDGE_TARGET_BYTES);
        self.edges.push(DependencyEdge {
            to: EdgeTarget(Arc::from(kept)),
            external: true,
            to_truncated: kept.len() < text.len(),
        });
        true
    }

    /// [`EdgeSink::push_resolved`] when `resolved` names a file, otherwise
    /// [`EdgeSink::push_external`] of `text`.
    fn push(&mut self, resolved: Option<&Arc<str>>, text: &str) -> bool {
        match resolved {
            Some(target) => self.push_resolved(target),
            None => self.push_external(text),
        }
    }
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn rust_has_pub(node: Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| child.kind() == "visibility_modifier")
}

fn resolve_rust_mod<'k>(rel: &str, name: &str, known: &'k KnownPaths) -> Option<&'k Arc<str>> {
    let mut dir = dir_components(rel);
    let stem = rel
        .rsplit('/')
        .next()
        .unwrap_or(rel)
        .trim_end_matches(".rs");
    if !matches!(stem, "mod" | "lib" | "main") {
        dir.push(stem.to_owned());
    }
    dir.push(name.to_owned());
    let base = dir.join("/");
    known
        .get(format!("{base}.rs").as_str())
        .or_else(|| known.get(format!("{base}/mod.rs").as_str()))
}

/// The directory `use crate::...` paths in `rel` resolve against: the
/// nearest ancestor of `rel` (walking up towards the mapped root, `rel`'s own
/// directory included) that itself directly contains a `lib.rs` or
/// `main.rs`, empty string when that ancestor is the mapped root itself.
///
/// `None` when no such ancestor exists among the files this map saw, which
/// happens when the mapped root is neither a crate's own `src/` directory
/// nor an ancestor of one (an arbitrary subdirectory, say); `use crate::...`
/// is then always recorded as external text rather than resolved against a
/// guess. Searching from the *importing file's own path* upward, rather than
/// from a single fixed location, is what makes this correct both when the
/// mapped root is one crate's `src/` directory directly and when it is a
/// whole repository (or workspace) with `src/` one or more levels down: each
/// file's `crate::` paths resolve against its own nearest crate root, not a
/// single global guess, which also gives each crate in a mapped workspace
/// its own correct answer.
fn rust_crate_root(rel: &str, known: &KnownPaths) -> Option<String> {
    let mut dir = dir_components(rel);
    loop {
        let prefix = dir.join("/");
        let lib = if prefix.is_empty() {
            "lib.rs".to_owned()
        } else {
            format!("{prefix}/lib.rs")
        };
        let main = if prefix.is_empty() {
            "main.rs".to_owned()
        } else {
            format!("{prefix}/main.rs")
        };
        if known.contains(lib.as_str()) || known.contains(main.as_str()) {
            return Some(prefix);
        }
        if dir.is_empty() {
            return None;
        }
        dir.pop();
    }
}

/// An index over every path in `known` that looks like a Rust source file,
/// built once per [`build_code_map_with_options`] call and reused for every
/// `use crate::...` resolution in every file mapped, so resolving one `use`
/// path of `k` `::`-separated segments costs O(k), not O(k^2).
///
/// Before this index existed, resolution rebuilt and re-hashed a fresh
/// candidate string for each of up to `k` progressively shorter prefixes.
/// This ticket's third adversarial-review round measured exactly the
/// quadratic shape that produces: 18 seconds for a 240 KB `use crate::`
/// path at 80,000 segments, and still running past 120 seconds at 330,000
/// segments (990 KB, 12% of the size cap), entirely outside the per-file
/// deadline, because nothing inside the old function checked it. A trie
/// walked once, segment by segment, from the crate root down, replaces the
/// "try every prefix length" search with the same longest-prefix answer
/// computed in one pass.
#[derive(Default)]
struct RustModuleIndex {
    children: HashMap<String, RustModuleIndex>,
    /// The resolved path when this node's own segment sequence names a real
    /// `<segments>.rs` file. Preferred over `as_mod_dir` when both exist
    /// (matching the original search order, which checked the file form
    /// before the `mod.rs` form at each length). The walk's own allocation
    /// of the path ([`KnownPaths`]), shared, never a copy.
    as_file: Option<Arc<str>>,
    /// The resolved path when this node's own segment sequence names a real
    /// `<segments>/mod.rs` file, shared the same way.
    as_mod_dir: Option<Arc<str>>,
}

/// Iterative, not the derived recursive drop glue: a deep, mostly
/// single-child chain (the shape a long `use crate::` path, or a deep
/// directory tree, builds) would otherwise overflow the stack when the
/// index is dropped, one frame per level, the same failure class `insert`
/// itself was rewritten to avoid. Found here by this module's own
/// 50,000-segment test overflowing on drop, after `insert` was already
/// iterative: fixing one recursive traversal is not fixing the shape, and
/// this module's `Drop` is exactly the shape it names.
impl Drop for RustModuleIndex {
    fn drop(&mut self) {
        let mut stack: Vec<RustModuleIndex> =
            self.children.drain().map(|(_, child)| child).collect();
        while let Some(mut node) = stack.pop() {
            stack.extend(node.children.drain().map(|(_, child)| child));
            // `node`'s own `children` is now empty, so its `Drop::drop`
            // (this same implementation) recurses no further when it runs
            // at the end of this iteration.
        }
    }
}

impl RustModuleIndex {
    fn build(known: &KnownPaths) -> Self {
        let mut root = Self::default();
        for path in known {
            if let Some(stem) = path.strip_suffix("/mod.rs") {
                root.insert(stem.split('/').filter(|s| !s.is_empty()), path, false);
            } else if let Some(stem) = path.strip_suffix(".rs") {
                if stem.is_empty() || stem == "mod" {
                    // A bare top-level `mod.rs` or `.rs` names the crate
                    // root itself, not a segment any `use crate::...` path
                    // could name (Rust's grammar does not allow `mod` as a
                    // path segment; harmless either way, excluded for
                    // clarity of intent).
                    continue;
                }
                root.insert(stem.split('/').filter(|s| !s.is_empty()), path, true);
            }
        }
        root
    }

    /// Iterative, not recursive: a path with as many `::`-separated segments
    /// as this ticket's own reproduction (tens of thousands) would overflow
    /// the stack one frame per segment otherwise, which is exactly the kind
    /// of pathological-depth failure the rest of this module (`for_each_node`,
    /// `walk_repository`) is already iterative to avoid; found by this
    /// ticket's own new stack-overflowing test before the fix, not by a
    /// reviewer.
    fn insert<'a>(
        &mut self,
        segments: impl Iterator<Item = &'a str>,
        resolved: &Arc<str>,
        is_file: bool,
    ) {
        let mut node = self;
        for seg in segments {
            node = node.children.entry(seg.to_owned()).or_default();
        }
        if is_file {
            node.as_file = Some(Arc::clone(resolved));
        } else {
            node.as_mod_dir = Some(Arc::clone(resolved));
        }
    }

    fn resolved(&self) -> Option<&Arc<str>> {
        self.as_file.as_ref().or(self.as_mod_dir.as_ref())
    }

    /// The trie node standing for `crate_root` (from [`rust_crate_root`]),
    /// where every `use crate::...` path's walk starts. `None` when no Rust
    /// file lives under it at all.
    fn crate_root_node(&self, crate_root: &str) -> Option<&Self> {
        let mut node = self;
        for seg in crate_root.split('/').filter(|s| !s.is_empty()) {
            node = node.children.get(seg)?;
        }
        Some(node)
    }

    /// Advances `cursor` one trie hop per segment, remembering the deepest
    /// node that names a real file: O(segment count), whatever the path.
    /// Returns the advanced cursor and whether it finished before `deadline`,
    /// which it checks every 256 hops so that one pathological path with an
    /// enormous segment count cannot run past its file's budget. A walk cut
    /// short keeps whatever it had found, and the `false` it returns is what
    /// turns the whole file into [`SkipReason::TimedOut`], so an
    /// approximation is never presented as final.
    ///
    /// A cursor is a position, not a path: a grouped import's prefix is
    /// walked once, and every member under it continues from the cursor the
    /// prefix left, rather than walking the shared prefix again per member.
    fn advance<'i, 's>(
        cursor: TrieCursor<'i>,
        segments: impl Iterator<Item = &'s str>,
        deadline: Instant,
    ) -> (TrieCursor<'i>, bool) {
        let mut cursor = cursor;
        for (step, seg) in segments.enumerate() {
            if step.is_multiple_of(256) && Instant::now() >= deadline {
                return (cursor, false);
            }
            let Some(node) = cursor.node else {
                break;
            };
            match node.children.get(seg) {
                Some(next) => {
                    cursor.node = Some(next);
                    if let Some(resolved) = next.resolved() {
                        cursor.best = Some(resolved);
                    }
                }
                None => {
                    cursor.node = None;
                    break;
                }
            }
        }
        (cursor, true)
    }

    /// Longest-prefix match of `crate_root` (from [`rust_crate_root`]) then
    /// `segments`, one hop per segment: O(total segment count), not O(count
    /// squared). `crate_root` alone, with no further segment, is never a
    /// resolution by itself, matching the original search's `take` range
    /// (which never tried zero segments either). Checks `deadline` the way
    /// [`RustModuleIndex::advance`] does, and returns whatever was found
    /// before it passed; see that function for why that is never a wrong
    /// answer presented as final.
    ///
    /// Test-only since round 4: resolution itself goes through
    /// `extend_use_root`, which composes the same two steps
    /// ([`RustModuleIndex::crate_root_node`], then
    /// [`RustModuleIndex::advance`]) but keeps the cursor, so a grouped
    /// import's members can continue from their prefix. This is that
    /// composition for a single path, which the index's own tests call.
    #[cfg(test)]
    fn longest_prefix<'a>(
        &self,
        crate_root: &str,
        segments: impl Iterator<Item = &'a str>,
        deadline: Instant,
    ) -> Option<String> {
        let start = self.crate_root_node(crate_root)?;
        let (cursor, _) = Self::advance(
            TrieCursor {
                node: Some(start),
                best: None,
            },
            segments,
            deadline,
        );
        cursor.best.map(|best| best.to_string())
    }
}

/// Where a `use crate::...` path's walk through [`RustModuleIndex`] stands.
#[derive(Clone, Copy)]
struct TrieCursor<'i> {
    /// The trie node the segments so far lead to; `None` once one had no
    /// child (no longer path can then resolve any deeper than `best`), or
    /// when the importing file has no crate root to start from.
    node: Option<&'i RustModuleIndex>,
    /// The deepest file found along the walk so far.
    best: Option<&'i Arc<str>>,
}

/// What a `use` path's leading segments have decided so far.
#[derive(Clone, Copy)]
enum UseRoot<'i> {
    /// No segment seen yet: a top-level group, `use {a, b};`, whose members
    /// each decide for themselves.
    Undecided,
    /// The path starts with something other than `crate` (a crate name,
    /// `std`, `self`, `super`): never resolved, recorded as written.
    NotCrate,
    /// The path starts with `crate`, and its walk has reached this far.
    Crate(TrieCursor<'i>),
}

/// Extends `root` with `segments`: the first segment of an undecided path
/// decides whether it is `crate`-rooted, and a `crate`-rooted path advances
/// its trie walk (see [`RustModuleIndex::advance`]). The `bool` is `false`
/// when `deadline` cut the walk short.
fn extend_use_root<'i, 's>(
    root: UseRoot<'i>,
    segments: impl IntoIterator<Item = &'s str>,
    crate_start: Option<&'i RustModuleIndex>,
    deadline: Instant,
) -> (UseRoot<'i>, bool) {
    let mut segments = segments.into_iter();
    let root = match root {
        UseRoot::Undecided => match segments.next() {
            None => return (UseRoot::Undecided, true),
            Some("crate") => UseRoot::Crate(TrieCursor {
                node: crate_start,
                best: None,
            }),
            Some(_) => return (UseRoot::NotCrate, true),
        },
        decided => decided,
    };
    match root {
        UseRoot::Crate(cursor) => {
            let (cursor, completed) = RustModuleIndex::advance(cursor, segments, deadline);
            (UseRoot::Crate(cursor), completed)
        }
        other => (other, true),
    }
}

/// The `::`-separated segments of a path node as the parser gives them
/// (`scoped_identifier`, `identifier`, `crate`, `self`, `super`), read from
/// the tree rather than split out of the text, so whitespace or a comment
/// between segments changes nothing. Iterative: a `scoped_identifier` nests
/// one level per segment, and a path can have hundreds of thousands.
fn rust_path_segments<'s>(path: Node<'s>, source: &'s [u8]) -> Vec<&'s str> {
    let mut reversed = Vec::new();
    let mut current = Some(path);
    while let Some(node) = current {
        if node.kind() == "scoped_identifier" {
            if let Some(name) = node.child_by_field_name("name") {
                reversed.push(text(name, source));
            }
            current = node.child_by_field_name("path");
        } else {
            reversed.push(text(node, source));
            current = None;
        }
    }
    reversed.reverse();
    reversed
}

/// The path segments one `use` leaf names: an `a::b as c` names `a::b`, an
/// `a::b::*` names `a::b` (so `use crate::foo::*;` resolves to the file for
/// `foo`), and anything else is a path itself.
fn rust_use_leaf_segments<'s>(leaf: Node<'s>, source: &'s [u8]) -> Vec<&'s str> {
    match leaf.kind() {
        "use_as_clause" => leaf
            .child_by_field_name("path")
            .map(|path| rust_path_segments(path, source))
            .unwrap_or_default(),
        "use_wildcard" => {
            let mut cursor = leaf.walk();
            let path = leaf
                .named_children(&mut cursor)
                .find(|child| !child.is_extra());
            path.map(|path| rust_path_segments(path, source))
                .unwrap_or_default()
        }
        _ => rust_path_segments(leaf, source),
    }
}

/// How much text, as a multiple of the whole declaration's own length, one
/// grouped `use` may spend spelling out its unresolved members (plus
/// [`USE_GROUP_TEXT_SLACK`]). A member's text repeats its group's prefix, so
/// a long prefix over many members would otherwise cost prefix length times
/// member count: `use crate::a::...::a::{b, b, ...}` inside one 8 MiB file
/// could ask for hundreds of GB. Ordinary code spends two or three times its
/// own length at most, far inside this.
const USE_GROUP_TEXT_FACTOR: usize = 16;

/// See [`USE_GROUP_TEXT_FACTOR`].
const USE_GROUP_TEXT_SLACK: usize = 4096;

/// One `path::` level of a grouped `use`, shared by every member under it,
/// so a member's text is built by walking up these rather than by copying
/// its prefix into every member.
struct UsePrefix<'s> {
    parent: Option<usize>,
    /// This level's own path, exactly as written.
    text: &'s str,
    /// The length of the whole prefix chain rendered with `::` joins.
    rendered_len: usize,
}

/// The text of an unresolved grouped member: its group's prefix chain and
/// its own text, joined with `::`; a `self` member is its prefix itself.
fn render_use_member(
    prefixes: &[UsePrefix<'_>],
    prefix: Option<usize>,
    member: &str,
    is_self: bool,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut current = prefix;
    while let Some(index) = current {
        parts.push(prefixes[index].text);
        current = prefixes[index].parent;
    }
    parts.reverse();
    if !(is_self && !parts.is_empty()) {
        parts.push(member);
    }
    parts.join("::")
}

/// Records into `sink` the dependency edges of one Rust `use` declaration,
/// given its `argument` node, and returns whether it finished before
/// `deadline`. A full `sink` ends the declaration early and is not a
/// deadline miss: the sink itself records the truncation.
///
/// A declaration without a group is one path: an edge to the file its
/// longest `crate::` prefix names, or, when it does not start with `crate`
/// or no prefix names a file, one external edge carrying the argument
/// exactly as written.
///
/// A grouped declaration (`use crate::{a::A, b::B};`,
/// `use crate::net::{http::Client, tcp::Stream};`, nested groups, a
/// top-level `use {crate::a::A, std::fmt};`) gives exactly the edges its
/// ungrouped equivalent would: the parser's own tree is walked, each member
/// is resolved on its own, from the trie position its group's prefix
/// reached (so a shared prefix is walked once, not once per member), and an
/// unresolved member is recorded as its own external edge, its text the
/// member's path with its group's prefix (`crate::net::Missing`), which is
/// what the ungrouped `use` would have recorded. A `self` member stands for
/// its group's prefix, as in Rust. No member is ever represented by its
/// group's prefix alone: the round-3 version cut the text at the first `{`
/// and resolved what was left, so `use crate::net::{http::Client,
/// tcp::Stream};` came out as one edge to `src/net.rs`, a complete-looking
/// edge to the wrong file.
///
/// Bounded: every tree node visited checks `deadline`, and the text spent on
/// unresolved members is capped at [`USE_GROUP_TEXT_FACTOR`] times the
/// declaration's own length plus [`USE_GROUP_TEXT_SLACK`]. Past that cap, the
/// members not yet spelled out are recorded once, together, as one external
/// edge carrying the whole argument as written (itself cut to
/// [`MAX_EDGE_TARGET_BYTES`] by the sink, like every unresolved target):
/// honest about what was not resolved, and never a partial edge that looks
/// complete.
fn rust_use_edges(
    argument: Node,
    source: &[u8],
    crate_start: Option<&RustModuleIndex>,
    sink: &mut EdgeSink,
    deadline: Instant,
) -> bool {
    let raw = text(argument, source);

    if !matches!(argument.kind(), "use_list" | "scoped_use_list") {
        let (root, completed) = extend_use_root(
            UseRoot::Undecided,
            rust_use_leaf_segments(argument, source),
            crate_start,
            deadline,
        );
        let resolved = match root {
            UseRoot::Crate(cursor) => cursor.best,
            _ => None,
        };
        sink.push(resolved, raw);
        return completed;
    }

    let mut prefixes: Vec<UsePrefix<'_>> = Vec::new();
    let mut text_budget = raw
        .len()
        .saturating_mul(USE_GROUP_TEXT_FACTOR)
        .saturating_add(USE_GROUP_TEXT_SLACK);
    let mut over_budget = false;
    let mut stack: Vec<(Node, UseRoot<'_>, Option<usize>)> =
        vec![(argument, UseRoot::Undecided, None)];
    while let Some((node, root, prefix)) = stack.pop() {
        if Instant::now() >= deadline {
            return false;
        }
        match node.kind() {
            "use_list" => {
                // Gathered with a check per child (`gather_children`): one
                // group can hold millions of members.
                let Some(children) = gather_children(node, deadline) else {
                    return false;
                };
                // Reversed onto the stack, so members come off it in source
                // order, the order `EdgeSink` keeps the first 4096 in (edges
                // are sorted afterwards).
                stack.extend(
                    children
                        .into_iter()
                        .filter(|member| member.is_named() && !member.is_extra())
                        .rev()
                        .map(|member| (member, root, prefix)),
                );
            }
            "scoped_use_list" => {
                let Some(list) = node.child_by_field_name("list") else {
                    continue;
                };
                let Some(path) = node.child_by_field_name("path") else {
                    // `use ::{a, b};`: no prefix of its own.
                    stack.push((list, root, prefix));
                    continue;
                };
                let (root, completed) = extend_use_root(
                    root,
                    rust_path_segments(path, source),
                    crate_start,
                    deadline,
                );
                if !completed {
                    return false;
                }
                let own = text(path, source);
                let rendered_len =
                    prefix.map_or(0, |index| prefixes[index].rendered_len + 2) + own.len();
                prefixes.push(UsePrefix {
                    parent: prefix,
                    text: own,
                    rendered_len,
                });
                stack.push((list, root, Some(prefixes.len() - 1)));
            }
            _ => {
                let (root, completed) = extend_use_root(
                    root,
                    rust_use_leaf_segments(node, source),
                    crate_start,
                    deadline,
                );
                if !completed {
                    return false;
                }
                if let UseRoot::Crate(TrieCursor {
                    best: Some(resolved),
                    ..
                }) = root
                {
                    if !sink.push_resolved(resolved) {
                        return true;
                    }
                    continue;
                }
                if over_budget {
                    continue;
                }
                let member = text(node, source);
                let is_self = node.kind() == "self";
                let prefix_len = prefix.map_or(0, |index| prefixes[index].rendered_len);
                let rendered_len = match prefix {
                    Some(_) if is_self => prefix_len,
                    Some(_) => prefix_len + 2 + member.len(),
                    None => member.len(),
                };
                if rendered_len > text_budget {
                    over_budget = true;
                    continue;
                }
                text_budget -= rendered_len;
                if !sink.push_external(&render_use_member(&prefixes, prefix, member, is_self)) {
                    return true;
                }
            }
        }
    }
    if over_budget {
        sink.push_external(raw);
    }
    true
}

fn extract_rust(
    root: Node,
    source: &[u8],
    rel: &str,
    known: &KnownPaths,
    rust_index: &RustModuleIndex,
    deadline: Instant,
) -> (Extracted, bool) {
    let mut out = Extracted::new();
    let crate_start = rust_crate_root(rel, known)
        .as_deref()
        .and_then(|crate_root| rust_index.crate_root_node(crate_root));
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if Instant::now() >= deadline {
            return (out, false);
        }
        match child.kind() {
            "mod_item" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = text(name_node, source);
                let is_declaration = child.child_by_field_name("body").is_none();
                if is_declaration {
                    match resolve_rust_mod(rel, name, known) {
                        Some(resolved) => out.edges.push_resolved(resolved),
                        None => out.edges.push_external(&format!("mod {name}")),
                    };
                }
                if rust_has_pub(child) {
                    out.interfaces.push(Interface {
                        name: name.to_owned(),
                        kind: InterfaceKind::Module,
                        line: line_of(child),
                    });
                }
            }
            "use_declaration" => {
                let Some(argument) = child.child_by_field_name("argument") else {
                    continue;
                };
                if !rust_use_edges(argument, source, crate_start, &mut out.edges, deadline) {
                    return (out, false);
                }
            }
            "function_item" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = text(name_node, source);
                if name == "main" {
                    out.entry_points.push(EntryPoint {
                        name: "main".to_owned(),
                        line: line_of(child),
                    });
                }
                if rust_has_pub(child) {
                    out.interfaces.push(Interface {
                        name: name.to_owned(),
                        kind: InterfaceKind::Function,
                        line: line_of(child),
                    });
                }
            }
            "struct_item" | "enum_item" | "trait_item" | "type_item" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                if rust_has_pub(child) {
                    out.interfaces.push(Interface {
                        name: text(name_node, source).to_owned(),
                        kind: InterfaceKind::Type,
                        line: line_of(child),
                    });
                }
            }
            "const_item" | "static_item" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                if rust_has_pub(child) {
                    out.interfaces.push(Interface {
                        name: text(name_node, source).to_owned(),
                        kind: InterfaceKind::Constant,
                        line: line_of(child),
                    });
                }
            }
            _ => {}
        }
    }
    (out, true)
}

fn rust_attribute_last_segment(attribute_item: Node, source: &[u8]) -> Option<String> {
    let mut cursor = attribute_item.walk();
    let attribute = attribute_item
        .children(&mut cursor)
        .find(|child| child.kind() == "attribute")?;
    let raw = text(attribute, source);
    let before_args = raw.split('(').next().unwrap_or(raw).trim();
    before_args.rsplit("::").next().map(str::to_owned)
}

/// Every Rust test name, found by [`for_each_sibling_group`] rather than by
/// walking backward from each `function_item` with [`Node::prev_sibling`]
/// (see [`for_each_node`]'s doc for the defect that was). Each parent's
/// children are scanned once, left to right, tracking "does a run of
/// attributes immediately before this item mark it as a test" locally, which
/// is the same rule `rust_marked_test` used to compute by walking backward,
/// computed forward instead.
///
/// [`for_each_sibling_group`] checks `deadline` once per parent and once per
/// child it gathers; [`rust_tests_in_siblings`] checks it once per sibling
/// too, since one parent can have millions of children (a file of nothing
/// but `#[a]` lines has one child per line) and reading an attribute is not
/// constant work.
fn rust_test_names(root: Node, source: &[u8], deadline: Instant) -> (Vec<String>, bool) {
    let mut names = Vec::new();
    let mut timed_out = false;
    let completed = for_each_sibling_group(root, deadline, |siblings| {
        if !timed_out && !rust_tests_in_siblings(siblings, source, &mut names, deadline) {
            timed_out = true;
        }
    });
    names.sort();
    names.dedup();
    (names, completed && !timed_out)
}

/// Pushes onto `names` every function among one parent's `siblings` (in
/// sibling order) that a run of attributes immediately before it marks as a
/// test, and returns whether it finished before `deadline`, checked before
/// every sibling.
fn rust_tests_in_siblings(
    siblings: &[Node],
    source: &[u8],
    names: &mut Vec<String>,
    deadline: Instant,
) -> bool {
    let mut pending_test = false;
    for &node in siblings {
        if Instant::now() >= deadline {
            return false;
        }
        if node.kind() == "attribute_item" {
            if rust_attribute_last_segment(node, source).as_deref() == Some("test") {
                pending_test = true;
            }
            continue;
        }
        if node.kind() == "function_item"
            && pending_test
            && let Some(name_node) = node.child_by_field_name("name")
        {
            names.push(text(name_node, source).to_owned());
        }
        pending_test = false;
    }
    true
}

// ---------------------------------------------------------------------------
// TypeScript (and TSX)
// ---------------------------------------------------------------------------

fn string_literal_text<'a>(node: Node<'a>, source: &'a [u8]) -> &'a str {
    let mut cursor = node.walk();
    if let Some(fragment) = node
        .children(&mut cursor)
        .find(|child| child.kind() == "string_fragment")
    {
        return text(fragment, source);
    }
    text(node, source).trim_matches(|c| c == '"' || c == '\'' || c == '`')
}

fn resolve_ts_relative<'k>(
    rel: &str,
    specifier: &str,
    known: &'k KnownPaths,
) -> Option<&'k Arc<str>> {
    let base = dir_components(rel);
    let joined = normalize_join(&base, specifier)?;
    [
        format!("{joined}.ts"),
        format!("{joined}.tsx"),
        format!("{joined}/index.ts"),
        format!("{joined}/index.tsx"),
    ]
    .into_iter()
    .find_map(|candidate| known.get(candidate.as_str()))
}

/// Records one TypeScript import or re-export's edge into `sink`: resolved
/// when it is a relative specifier naming a file in the map, otherwise the
/// specifier as written.
fn ts_import_edge(rel: &str, specifier: &str, known: &KnownPaths, sink: &mut EdgeSink) {
    let resolved = if specifier.starts_with("./") || specifier.starts_with("../") {
        resolve_ts_relative(rel, specifier, known)
    } else {
        None
    };
    sink.push(resolved, specifier);
}

/// Pushes onto `items` the interfaces one exported TypeScript declaration
/// declares, and returns whether it finished before `deadline`, which it
/// checks before every declarator: one `export let a, a, ..., a;` can hold
/// millions of them, two bytes each, and until round 6 this loop had no
/// check (see [`go_declaration_interfaces`] for what the Go equivalent of
/// that cost).
fn ts_declaration_interfaces(
    declaration: Node,
    source: &[u8],
    items: &mut Vec<Interface>,
    deadline: Instant,
) -> bool {
    match declaration.kind() {
        "function_declaration" => {
            if let Some(name) = declaration.child_by_field_name("name") {
                items.push(Interface {
                    name: text(name, source).to_owned(),
                    kind: InterfaceKind::Function,
                    line: line_of(declaration),
                });
            }
        }
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "type_alias_declaration" => {
            if let Some(name) = declaration.child_by_field_name("name") {
                items.push(Interface {
                    name: text(name, source).to_owned(),
                    kind: InterfaceKind::Type,
                    line: line_of(declaration),
                });
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            let mut cursor = declaration.walk();
            for declarator in declaration
                .children(&mut cursor)
                .filter(|child| child.kind() == "variable_declarator")
            {
                if Instant::now() >= deadline {
                    return false;
                }
                if let Some(name) = declarator.child_by_field_name("name") {
                    items.push(Interface {
                        name: text(name, source).to_owned(),
                        kind: InterfaceKind::Constant,
                        line: line_of(declarator),
                    });
                }
            }
        }
        _ => {}
    }
    true
}

fn extract_typescript(
    root: Node,
    source: &[u8],
    rel: &str,
    known: &KnownPaths,
    deadline: Instant,
) -> (Extracted, bool) {
    let mut out = Extracted::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if Instant::now() >= deadline {
            return (out, false);
        }
        match child.kind() {
            "import_statement" => {
                if let Some(source_node) = child.child_by_field_name("source") {
                    let specifier = string_literal_text(source_node, source);
                    ts_import_edge(rel, specifier, known, &mut out.edges);
                }
            }
            "export_statement" => {
                if let Some(source_node) = child.child_by_field_name("source") {
                    let specifier = string_literal_text(source_node, source);
                    ts_import_edge(rel, specifier, known, &mut out.edges);
                }
                if let Some(declaration) = child.child_by_field_name("declaration") {
                    if !ts_declaration_interfaces(
                        declaration,
                        source,
                        &mut out.interfaces,
                        deadline,
                    ) {
                        return (out, false);
                    }
                    if declaration.kind() == "function_declaration"
                        && let Some(name) = declaration.child_by_field_name("name")
                        && text(name, source) == "main"
                    {
                        out.entry_points.push(EntryPoint {
                            name: "main".to_owned(),
                            line: line_of(declaration),
                        });
                    }
                }
            }
            "function_declaration" => {
                if let Some(name) = child.child_by_field_name("name")
                    && text(name, source) == "main"
                {
                    out.entry_points.push(EntryPoint {
                        name: "main".to_owned(),
                        line: line_of(child),
                    });
                }
            }
            _ => {}
        }
    }
    (out, true)
}

fn typescript_test_names(root: Node, source: &[u8], deadline: Instant) -> (Vec<String>, bool) {
    let mut names = Vec::new();
    let completed = for_each_node(root, deadline, |node| {
        if node.kind() != "call_expression" {
            return;
        }
        let Some(function) = node.child_by_field_name("function") else {
            return;
        };
        if function.kind() != "identifier" {
            return;
        }
        if !matches!(text(function, source), "test" | "it" | "describe") {
            return;
        }
        let Some(arguments) = node.child_by_field_name("arguments") else {
            return;
        };
        let mut cursor = arguments.walk();
        if let Some(first) = arguments.named_children(&mut cursor).next()
            && first.kind() == "string"
        {
            names.push(string_literal_text(first, source).to_owned());
        }
    });
    names.sort();
    names.dedup();
    (names, completed)
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

/// The most leading dots a Python relative import's unresolved edge spells
/// out literally in [`DependencyEdge::to`]. Past this, the prefix is written
/// as a count (`"[N leading dots]"`) instead, so no edge's text grows with a
/// dot count an attacker chose. No real import comes near it: a relative
/// import climbs one directory per dot beyond the first, and one that
/// climbs past the mapped root (which any count above the file's own depth
/// does) is never resolved here anyway.
const MAX_SPELLED_RELATIVE_IMPORT_DOTS: usize = 32;

/// Records into `sink` the dependency edges of one Python `from ... import
/// ...` statement, and returns whether it finished before `deadline`. A full
/// `sink` ends the statement early and is not a deadline miss: the sink
/// itself records the truncation.
///
/// A relative import (`from . import a`, `from ..pkg import b`) resolves
/// against the importing file's own directory, climbing one level per dot
/// beyond the first. What makes this bounded in the size of the statement,
/// which the round-3 version was not (the review of that round measured one
/// 8 MiB statement of `K` dots and `M` names costing `K x M`, over two
/// minutes, because the dot prefix was rebuilt for every name and nothing
/// inside the loop read the clock):
///
/// - The dots are counted once, and the climb is one `truncate`, not a loop
///   of `K` pops.
/// - A climb past the mapped root is never resolved: the target is outside
///   this map, so it cannot name a file in it. (The round-3 version clamped
///   such a climb to the root, resolving `from .. import b` in a top-level
///   file to the root's own `b.py`, which Python itself refuses.)
/// - The unresolved text's prefix is built once per statement, and spelled
///   as a count past [`MAX_SPELLED_RELATIVE_IMPORT_DOTS`], so each edge costs
///   the length of its own name, not `K` more (and never more than
///   [`MAX_EDGE_TARGET_BYTES`], the cap every unresolved target has; the
///   importing file's path is not in the edge at all since round 6).
/// - The loop over names checks `deadline` before every name, returning
///   `false` when it passes, and stops when `sink` is full. The names are
///   walked in place, not collected before the loop: the collection alone
///   ran eleven to fifteen seconds, unchecked, for one 8 MiB statement
///   (release builds, round 6's measurement), past a 13-second file budget.
fn python_import_from_edges(
    node: Node,
    source: &[u8],
    rel: &str,
    known: &KnownPaths,
    sink: &mut EdgeSink,
    deadline: Instant,
) -> bool {
    let Some(module_name) = node.child_by_field_name("module_name") else {
        return true;
    };

    if module_name.kind() != "relative_import" {
        sink.push_external(text(module_name, source));
        return true;
    }

    let (dots, dotted) = {
        let mut cursor = module_name.walk();
        let mut dots = 0usize;
        let mut dotted = None;
        for child in module_name.children(&mut cursor) {
            match child.kind() {
                "import_prefix" => {
                    dots = text(child, source).bytes().filter(|b| *b == b'.').count();
                }
                "dotted_name" => dotted = Some(text(child, source)),
                _ => {}
            }
        }
        (dots.max(1), dotted)
    };

    // `None` when the climb leaves the mapped root: nothing in this map can
    // be the target, so nothing is looked up.
    let base: Option<Vec<String>> = {
        let mut base = dir_components(rel);
        let climb = dots - 1;
        (climb <= base.len()).then(|| {
            base.truncate(base.len() - climb);
            base
        })
    };
    let prefix = if dots <= MAX_SPELLED_RELATIVE_IMPORT_DOTS {
        ".".repeat(dots)
    } else {
        format!("[{dots} leading dots]")
    };
    let resolve = |segments: &[&str]| -> Option<&Arc<str>> {
        let mut target_dir = base.clone()?;
        target_dir.extend(segments.iter().map(|segment| (*segment).to_owned()));
        let joined = target_dir.join("/");
        [format!("{joined}.py"), format!("{joined}/__init__.py")]
            .into_iter()
            .find_map(|candidate| known.get(candidate.as_str()))
    };

    if let Some(dotted) = dotted {
        let segments: Vec<&str> = dotted.split('.').collect();
        match resolve(&segments) {
            Some(resolved) => sink.push_resolved(resolved),
            None => sink.push_external(&format!("{prefix}{dotted}")),
        };
        return true;
    }

    // Walked in place, never collected first: collecting every name before
    // the loop's first deadline check was itself unbounded work (eleven to
    // fifteen seconds for one 8 MiB statement, tree-sitter finding each named
    // child by its field), which round 4's per-name check never reached.
    let mut names = node.walk();
    for name_node in node.children_by_field_name("name", &mut names) {
        if Instant::now() >= deadline {
            return false;
        }
        let name_text = if name_node.kind() == "aliased_import" {
            name_node
                .child_by_field_name("name")
                .map(|n| text(n, source))
        } else {
            Some(text(name_node, source))
        };
        let Some(name_text) = name_text else { continue };
        let recorded = match resolve(&[name_text]) {
            Some(resolved) => sink.push_resolved(resolved),
            None => sink.push_external(&format!("{prefix}{name_text}")),
        };
        if !recorded {
            return true;
        }
    }
    true
}

/// Records into `sink` the edges of one Python `import a, b.c as d`
/// statement (each external: an absolute import is never resolved here),
/// and returns whether it finished before `deadline`, checked before every
/// name. The names are walked in place, never collected first; see
/// [`python_import_from_edges`] for why. A full `sink` ends the statement
/// early and is not a deadline miss.
fn python_import_edges(
    statement: Node,
    source: &[u8],
    sink: &mut EdgeSink,
    deadline: Instant,
) -> bool {
    let mut names = statement.walk();
    for name_node in statement.children_by_field_name("name", &mut names) {
        if Instant::now() >= deadline {
            return false;
        }
        let name_text = if name_node.kind() == "aliased_import" {
            name_node
                .child_by_field_name("name")
                .map(|n| text(n, source))
        } else {
            Some(text(name_node, source))
        };
        if let Some(name_text) = name_text
            && !sink.push_external(name_text)
        {
            return true;
        }
    }
    true
}

fn extract_python(
    root: Node,
    source: &[u8],
    rel: &str,
    known: &KnownPaths,
    deadline: Instant,
) -> (Extracted, bool) {
    let mut out = Extracted::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if Instant::now() >= deadline {
            return (out, false);
        }
        match child.kind() {
            "import_statement" => {
                if !python_import_edges(child, source, &mut out.edges, deadline) {
                    return (out, false);
                }
            }
            "import_from_statement" => {
                if !python_import_from_edges(child, source, rel, known, &mut out.edges, deadline) {
                    return (out, false);
                }
            }
            "function_definition" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let name_text = text(name, source);
                    if !name_text.starts_with('_') {
                        out.interfaces.push(Interface {
                            name: name_text.to_owned(),
                            kind: InterfaceKind::Function,
                            line: line_of(child),
                        });
                    }
                }
            }
            "class_definition" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let name_text = text(name, source);
                    if !name_text.starts_with('_') {
                        out.interfaces.push(Interface {
                            name: name_text.to_owned(),
                            kind: InterfaceKind::Type,
                            line: line_of(child),
                        });
                    }
                }
            }
            "if_statement" => {
                if let Some(condition) = child.child_by_field_name("condition") {
                    let condition_text = text(condition, source);
                    if condition_text.contains("__name__") && condition_text.contains("__main__") {
                        out.entry_points.push(EntryPoint {
                            name: "__main__".to_owned(),
                            line: line_of(child),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    (out, true)
}

fn python_test_names(root: Node, source: &[u8], deadline: Instant) -> (Vec<String>, bool) {
    let mut names = Vec::new();
    let completed = for_each_node(root, deadline, |node| {
        if node.kind() == "function_definition"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name_text = text(name, source);
            if name_text.starts_with("test") {
                names.push(name_text.to_owned());
            }
        }
    });
    names.sort();
    names.dedup();
    (names, completed)
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

/// Records into `sink` one edge per `import_spec` under `import_declaration`,
/// each external (see the module doc). Once `sink` is full, the rest of the
/// declaration is still visited (under `deadline`) but records nothing.
fn go_import_spec_edges(
    import_declaration: Node,
    source: &[u8],
    sink: &mut EdgeSink,
    deadline: Instant,
) {
    for_each_node(import_declaration, deadline, |node| {
        if node.kind() != "import_spec" {
            return;
        }
        let Some(path_node) = node.child_by_field_name("path") else {
            return;
        };
        let mut cursor = path_node.walk();
        let content = path_node
            .children(&mut cursor)
            .find(|child| child.kind() == "interpreted_string_literal_content")
            .map_or_else(
                || text(path_node, source).trim_matches('"'),
                |content| text(content, source),
            );
        sink.push_external(content);
    });
}

fn go_exported(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

/// Pushes onto `interfaces` the exported names one Go `type`, `const` or
/// `var` declaration declares, and returns whether it finished before
/// `deadline`, which it checks before every spec and every name. One
/// declaration can hold millions of names (`var A, A, ..., A int`, two
/// bytes each), and until round 6 they were collected and walked with no
/// check at all: the file's other stages each checked the deadline, and
/// this loop alone ran on past it, 4.7 to 12 seconds past a 13-second
/// budget for one 8 MiB declaration (release builds, this ticket's machine).
fn go_declaration_interfaces(
    declaration: Node,
    source: &[u8],
    interfaces: &mut Vec<Interface>,
    deadline: Instant,
) -> bool {
    let mut cursor = declaration.walk();
    for spec in declaration.children(&mut cursor) {
        if Instant::now() >= deadline {
            return false;
        }
        let kind = match spec.kind() {
            "type_spec" => InterfaceKind::Type,
            "const_spec" | "var_spec" => InterfaceKind::Constant,
            _ => continue,
        };
        let mut names = spec.walk();
        for name_node in spec.children_by_field_name("name", &mut names) {
            if Instant::now() >= deadline {
                return false;
            }
            let name = text(name_node, source);
            if go_exported(name) {
                interfaces.push(Interface {
                    name: name.to_owned(),
                    kind,
                    line: line_of(spec),
                });
            }
        }
    }
    true
}

fn extract_go(root: Node, source: &[u8], deadline: Instant) -> (Extracted, bool) {
    let mut out = Extracted::new();
    let mut package_name = String::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if Instant::now() >= deadline {
            return (out, false);
        }
        match child.kind() {
            "package_clause" => {
                let mut inner_cursor = child.walk();
                if let Some(identifier) = child
                    .children(&mut inner_cursor)
                    .find(|node| node.kind() == "package_identifier")
                {
                    package_name = text(identifier, source).to_owned();
                }
            }
            "import_declaration" => {
                go_import_spec_edges(child, source, &mut out.edges, deadline);
            }
            "function_declaration" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = text(name_node, source);
                if name == "main" && package_name == "main" {
                    out.entry_points.push(EntryPoint {
                        name: "main".to_owned(),
                        line: line_of(child),
                    });
                }
                if go_exported(name) {
                    out.interfaces.push(Interface {
                        name: name.to_owned(),
                        kind: InterfaceKind::Function,
                        line: line_of(child),
                    });
                }
            }
            "type_declaration" | "const_declaration" | "var_declaration" => {
                let completed =
                    go_declaration_interfaces(child, source, &mut out.interfaces, deadline);
                if !completed {
                    return (out, false);
                }
            }
            _ => {}
        }
    }
    (out, true)
}

fn go_test_names(root: Node, source: &[u8], deadline: Instant) -> (Vec<String>, bool) {
    let mut names = Vec::new();
    let completed = for_each_node(root, deadline, |node| {
        if node.kind() == "function_declaration"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name_text = text(name, source);
            if name_text.starts_with("Test") {
                names.push(name_text.to_owned());
            }
        }
    });
    names.sort();
    names.dedup();
    (names, completed)
}

// ---------------------------------------------------------------------------
// Covering tests: filename convention
// ---------------------------------------------------------------------------

/// For every module, looks for a sibling file matching that language's
/// test-filename convention (see the module doc) and, if found in the map,
/// adds it to [`Module::covering_tests`].
fn link_sibling_test_files(modules: &mut [Module]) {
    let known_paths: HashSet<String> = modules.iter().map(|module| module.path.clone()).collect();
    let mut additions: Vec<(usize, String)> = Vec::new();

    for (index, module) in modules.iter().enumerate() {
        let dir = dir_components(&module.path);
        let file_name = module.path.rsplit('/').next().unwrap_or(&module.path);
        let candidates: Vec<String> = match module.language {
            Language::TypeScript => {
                let stem = file_name.trim_end_matches(".tsx").trim_end_matches(".ts");
                let mut dir_str = dir.clone();
                let dir_prefix = if dir_str.is_empty() {
                    String::new()
                } else {
                    dir_str.push(String::new());
                    dir_str.join("/")
                };
                ["test.ts", "test.tsx", "spec.ts", "spec.tsx"]
                    .iter()
                    .map(|suffix| format!("{dir_prefix}{stem}.{suffix}"))
                    .collect()
            }
            Language::Python => {
                let stem = file_name.trim_end_matches(".py").trim_end_matches(".pyi");
                let dir_str = dir.join("/");
                let mut out = Vec::new();
                for candidate_dir in [
                    dir_str.clone(),
                    join_dir(&dir_str, "tests"),
                    join_dir(&dir_str, "test"),
                ] {
                    let prefix = if candidate_dir.is_empty() {
                        String::new()
                    } else {
                        format!("{candidate_dir}/")
                    };
                    out.push(format!("{prefix}test_{stem}.py"));
                    out.push(format!("{prefix}{stem}_test.py"));
                }
                out
            }
            Language::Go => {
                let stem = file_name.trim_end_matches(".go");
                let dir_str = dir.join("/");
                let prefix = if dir_str.is_empty() {
                    String::new()
                } else {
                    format!("{dir_str}/")
                };
                vec![format!("{prefix}{stem}_test.go")]
            }
            Language::Rust => Vec::new(),
        };

        for candidate in candidates {
            if candidate != module.path && known_paths.contains(&candidate) {
                additions.push((index, candidate));
            }
        }
    }

    for (index, addition) in additions {
        modules[index].covering_tests.push(addition);
    }
}

fn join_dir(dir: &str, sub: &str) -> String {
    if dir.is_empty() {
        sub.to_owned()
    } else {
        format!("{dir}/{sub}")
    }
}

// ---------------------------------------------------------------------------
// Spec citations
// ---------------------------------------------------------------------------

/// The most citations one module keeps from the `spec/` scan. Past it, the
/// scan stops looking for that module and sets
/// [`Module::spec_sections_truncated`], so a document of many distinct
/// headings each citing the same module cannot grow that module's list
/// without bound. See the module doc's "Memory" bound.
const MAX_SPEC_CITATIONS_PER_MODULE: usize = 500;

/// The longest heading text the map keeps. A longer heading is cut to this
/// many bytes, back to a UTF-8 character boundary, and marked
/// [`SpecHeading::truncated`]. This, with each distinct heading being kept
/// once for the whole map ([`CitationScan::intern_heading`]), is what bounds
/// the heading text a [`CodeMap`] retains; see the module doc's "Memory".
const MAX_HEADING_BYTES: usize = 4096;

/// How many lines [`CitationScan::scan_document`] searches, across every
/// module and document together, before it reads the clock again, unless
/// [`SPEC_SCAN_DEADLINE_CHECK_EVERY_BYTES`] comes first. Checking every single
/// line would itself cost real time at the scale a `spec/` scan can reach
/// (modules times documents times lines), so this amortizes that cost on the
/// ordinary shape, many short lines.
const SPEC_SCAN_DEADLINE_CHECK_EVERY_LINES: u64 = 4096;

/// How many bytes of work [`CitationScan::scan_document`] does before it
/// reads the clock again, unless [`SPEC_SCAN_DEADLINE_CHECK_EVERY_LINES`]
/// comes first: each line is charged its own length plus the module path
/// searched for in it, which is what one `str::contains` call costs, linear
/// in both.
///
/// Why bytes and not lines alone: a line can be as long as a whole document,
/// up to [`CodeMapOptions::max_file_bytes`], so a line count says nothing
/// about how much work was done between two checks. The review of this
/// ticket's third round built exactly that document (256 modules, 17
/// documents of one 8 MiB line each, with module paths chosen to make each
/// search slow) and measured the round-3 scan, which checked every 4096
/// lines only, running 27 to 45 seconds against its 5 second budget,
/// because the first check came at line 4096 and the whole scan was 4352
/// line searches. Charged by bytes, the scan now reads the clock after every
/// line at least 1 MiB long, so it overruns its deadline by at most one
/// line's search.
const SPEC_SCAN_DEADLINE_CHECK_EVERY_BYTES: u64 = 1024 * 1024;

fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// A Markdown heading's text: the line with its leading whitespace, its run
/// of `#` marks and its surrounding whitespace removed, or `None` when the
/// line is not a heading or the heading is empty. A slice of `line`, never a
/// copy.
fn markdown_heading(line: &str) -> Option<&str> {
    let text = line
        .trim_start()
        .strip_prefix('#')?
        .trim_start_matches('#')
        .trim();
    (!text.is_empty()).then_some(text)
}

/// What the `spec/` scan hands back besides each module's own citations:
/// the tables the citations index into, and how the scan ended.
struct SpecScanOutput {
    docs: Vec<String>,
    headings: Vec<SpecHeading>,
    status: SpecScan,
    /// Every entry under `spec/` seen and not scanned, sorted by path.
    skipped: Vec<SkippedFile>,
}

/// The key a module's citations are sorted by: the document's path, then
/// the heading's text (no heading first), the texts the indices name rather
/// than the indices, so the order is the same whatever order the headings
/// were first cited in.
fn spec_citation_sort_key<'s>(
    spec: &'s SpecScanOutput,
    citation: &SpecCitation,
) -> (&'s str, Option<(&'s str, bool)>) {
    (
        spec.docs.get(citation.doc_index).map_or("", String::as_str),
        citation
            .heading_index
            .and_then(|index| spec.headings.get(index))
            .map(|heading| (heading.text.as_str(), heading.truncated)),
    )
}

/// The citation scan's state, carried from one document to the next: the
/// output tables, and per module what it has already been cited under.
///
/// Documents are scanned one at a time, each from one owned buffer that is
/// dropped before the next is read, and every line is searched in place as
/// a slice of that buffer. The round-3 version held every document at once,
/// each line copied into its own `String` with a heading index beside it,
/// about 35 times the corpus's size in memory; the review of that round
/// measured 2.3 GB for eight newline-only documents of 8 MiB each. See the
/// module doc's "Memory" bound for what is held now.
struct CitationScan {
    /// Every cited document's path, first-cited order.
    docs: Vec<String>,
    /// Every distinct cited heading, first-cited order.
    headings: Vec<SpecHeading>,
    /// `headings`' lookup index, one map per value of
    /// [`SpecHeading::truncated`], so a lookup by `&str` allocates nothing.
    /// A second copy of each heading's text, held only while the scan runs.
    heading_ids: [HashMap<String, usize>; 2],
    /// Per module, the (document, heading) pairs it has been cited under: at
    /// most [`MAX_SPEC_CITATIONS_PER_MODULE`] each.
    seen: Vec<HashSet<(usize, Option<usize>)>>,
    /// Per module, whether it reached [`MAX_SPEC_CITATIONS_PER_MODULE`] and
    /// is no longer searched for.
    done: Vec<bool>,
    lines_since_check: u64,
    bytes_since_check: u64,
}

impl CitationScan {
    fn new(modules: usize) -> Self {
        Self {
            docs: Vec::new(),
            headings: Vec::new(),
            heading_ids: [HashMap::new(), HashMap::new()],
            seen: vec![HashSet::new(); modules],
            done: vec![false; modules],
            lines_since_check: 0,
            bytes_since_check: 0,
        }
    }

    /// The index of `text` in `headings`, adding it if this is its first
    /// citation: each distinct heading is stored once for the whole map,
    /// however many modules and documents cite under it.
    fn intern_heading(&mut self, text: &str, truncated: bool) -> usize {
        let ids = &mut self.heading_ids[usize::from(truncated)];
        if let Some(&id) = ids.get(text) {
            return id;
        }
        let id = self.headings.len();
        self.headings.push(SpecHeading {
            text: text.to_owned(),
            truncated,
        });
        ids.insert(text.to_owned(), id);
        id
    }

    /// Searches `content`, the whole of one document, for every module's
    /// path, line by line, in place: each line is a slice of `content`, and
    /// the heading above it is tracked as a slice too, so nothing is copied
    /// per line. A heading's text is copied only when a citation under it is
    /// recorded for the first time, and a document's path only when it is
    /// first cited. Returns `false` when `deadline` passed first.
    ///
    /// The deadline is checked by work done, not by lines alone: after
    /// [`SPEC_SCAN_DEADLINE_CHECK_EVERY_LINES`] lines or
    /// [`SPEC_SCAN_DEADLINE_CHECK_EVERY_BYTES`] bytes of search, whichever
    /// comes first. The check comes after a line's search, never before the
    /// first one, so an already-expired deadline still lets exactly one
    /// bounded amount of work run; what it never allows is an unbounded
    /// amount.
    fn scan_document(
        &mut self,
        rel: &str,
        content: &str,
        modules: &mut [Module],
        deadline: Instant,
    ) -> bool {
        let mut doc_index: Option<usize> = None;
        for (m, module) in modules.iter_mut().enumerate() {
            if self.done[m] {
                continue;
            }
            // The heading above the current line, and whether it was cut.
            let mut heading: Option<(&str, bool)> = None;
            // Its index in `headings`, once looked up: `Some(None)` when it
            // has not been cited yet, `None` when not looked up since the
            // heading last changed.
            let mut heading_id: Option<Option<usize>> = None;
            for line in content.lines() {
                if let Some(full) = markdown_heading(line) {
                    let text = truncate_at_char_boundary(full, MAX_HEADING_BYTES);
                    heading = Some((text, text.len() < full.len()));
                    heading_id = None;
                }
                if line.contains(module.path.as_str()) {
                    // The pair's key, if both halves have been cited before;
                    // `None` means this pair is certainly new.
                    let heading_key: Option<Option<usize>> = match heading {
                        None => Some(None),
                        Some((text, truncated)) => (*heading_id.get_or_insert_with(|| {
                            self.heading_ids[usize::from(truncated)].get(text).copied()
                        }))
                        .map(Some),
                    };
                    let is_new = match (doc_index, heading_key) {
                        (Some(doc), Some(key)) => !self.seen[m].contains(&(doc, key)),
                        _ => true,
                    };
                    if is_new {
                        if module.spec_sections.len() >= MAX_SPEC_CITATIONS_PER_MODULE {
                            module.spec_sections_truncated = true;
                            self.done[m] = true;
                            break;
                        }
                        let doc = *doc_index.get_or_insert_with(|| {
                            self.docs.push(rel.to_owned());
                            self.docs.len() - 1
                        });
                        let key = heading.map(|(text, truncated)| {
                            let id = self.intern_heading(text, truncated);
                            heading_id = Some(Some(id));
                            id
                        });
                        self.seen[m].insert((doc, key));
                        module.spec_sections.push(SpecCitation {
                            doc_index: doc,
                            heading_index: key,
                        });
                    }
                }
                self.lines_since_check += 1;
                self.bytes_since_check = self
                    .bytes_since_check
                    .saturating_add(line.len() as u64)
                    .saturating_add(module.path.len() as u64);
                if self.lines_since_check >= SPEC_SCAN_DEADLINE_CHECK_EVERY_LINES
                    || self.bytes_since_check >= SPEC_SCAN_DEADLINE_CHECK_EVERY_BYTES
                {
                    self.lines_since_check = 0;
                    self.bytes_since_check = 0;
                    if Instant::now() >= deadline {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// Scans `<root>/spec/**/*.md`, best effort, for each module's path as a
/// literal substring, recording each citation into its module. See the
/// module doc, "What spec sections per module means here". Returns the
/// tables the citations index into and how the scan ended; see
/// [`Coverage::spec_citation_scan`] for why an incomplete scan does not turn
/// an already-successful [`Module`] into a [`SkippedFile`].
///
/// `spec` is found by reading `root`'s own entries and matching a name
/// exactly, never by joining `"spec"` onto `root` and asking the OS whether
/// that is a directory: the join-and-ask form follows a symlink (and, per
/// Rust's own `std` docs, a Windows junction: `FileType::is_symlink` reports
/// `IO_REPARSE_TAG_SYMLINK` and `IO_REPARSE_TAG_MOUNT_POINT` alike, which
/// covers a junction; this module could not verify that on an actual Windows
/// machine, and says so rather than assuming it), and it asks the OS to
/// decide "spec" case-insensitively on macOS and Windows, giving a different
/// map on Linux for the identical repository. Matching by exact `OsStr`
/// equality on `root`'s own directory entries is exact and case-sensitive on
/// every platform alike, and reading `symlink_metadata` (not `metadata`) is
/// what refuses a symlinked or junctioned `spec` rather than following it.
fn attach_spec_citations(
    root: &Path,
    modules: &mut [Module],
    options: &CodeMapOptions,
    deadline: Instant,
) -> SpecScanOutput {
    let mut scan = CitationScan::new(modules.len());
    let mut skipped = Vec::new();
    let status = scan_spec_directory(root, modules, options, deadline, &mut scan, &mut skipped);
    skipped.sort_by(|a, b| a.path.cmp(&b.path));
    SpecScanOutput {
        docs: scan.docs,
        headings: scan.headings,
        status,
        skipped,
    }
}

/// [`attach_spec_citations`]'s body: finds `spec`, lists its documents, then
/// reads and scans them one at a time, in path order, charging every byte
/// read to [`CodeMapOptions::max_spec_bytes`], and records in `skipped`
/// every entry it saw and did not scan, with the reason. The result is
/// [`SpecScan::Complete`] only when `skipped` is empty: a scan that passed
/// over anything says so, rather than looking as complete as one that did
/// not.
///
/// Each document is read with a limit of the smaller of
/// [`CodeMapOptions::max_file_bytes`] and what is left of the budget,
/// through `take(limit + 1)`: more than what is left means the corpus does
/// not fit, and the scan stops there as [`SpecScan::CorpusOverBudget`],
/// having read at most one byte past the budget, with that document and
/// every one after it recorded as [`SkipReason::SpecCorpusOverBudget`];
/// more than the per-file cap means this one document is skipped as
/// [`SkipReason::TooLarge`], its bytes still charged, since they were read.
/// A document that cannot be opened as a regular file or read is skipped
/// with the reason [`open_regular_file`] or the read gave, and one that is
/// not UTF-8 as [`SkipReason::Binary`]. A deadline that passes records the
/// document it interrupted and every one after it as
/// [`SkipReason::TimedOut`].
fn scan_spec_directory(
    root: &Path,
    modules: &mut [Module],
    options: &CodeMapOptions,
    deadline: Instant,
    scan: &mut CitationScan,
    skipped: &mut Vec<SkippedFile>,
) -> SpecScan {
    let read_dir = match fs::read_dir(root) {
        Ok(read_dir) => read_dir,
        Err(err) => {
            // Listed moments ago by the walk; if it cannot be listed now,
            // whether there is a `spec/` is unknown, which is not complete.
            skipped.push(SkippedFile {
                path: "spec".to_owned(),
                reason: SkipReason::Unreadable(format!(
                    "the mapped root could not be listed again to look for spec/: {err}"
                )),
            });
            return SpecScan::Partial;
        }
    };
    let Some(spec_entry) = read_dir
        .filter_map(std::result::Result::ok)
        .find(|entry| entry.file_name() == std::ffi::OsStr::new("spec"))
    else {
        return SpecScan::Complete;
    };
    let spec_dir = spec_entry.path();
    let file_type = match fs::symlink_metadata(&spec_dir) {
        Ok(meta) => meta.file_type(),
        Err(err) => {
            admit_skip(
                root,
                &spec_dir,
                SkipReason::Unreadable(err.to_string()),
                skipped,
            );
            return SpecScan::Partial;
        }
    };
    // `is_symlink()` checked first and on its own, not folded into
    // `is_dir()`: a Windows junction is a directory-shaped reparse point, so
    // if a platform ever reported `is_dir() == true` for one under
    // `symlink_metadata` (unverified here, no Windows machine to check on),
    // this is the independent guard against treating it as a real
    // directory. A `spec` link to a directory is recorded, since it stands
    // for documents the scan will not read; one to anything else is not a
    // `spec/` directory at all.
    if file_type.is_symlink() {
        if fs::metadata(&spec_dir).is_ok_and(|meta| meta.is_dir()) {
            admit_skip(root, &spec_dir, SkipReason::SymlinkNotFollowed, skipped);
            return SpecScan::Partial;
        }
        return SpecScan::Complete;
    }
    if !file_type.is_dir() {
        return SpecScan::Complete;
    }

    let listing = list_markdown(&spec_dir, root, deadline);
    skipped.extend(listing.skipped);
    let mut documents = listing.documents.into_iter();
    if !listing.completed {
        skipped.extend(documents.map(|(path, _)| SkippedFile {
            path,
            reason: SkipReason::TimedOut,
        }));
        return SpecScan::TimedOut;
    }
    let mut remaining = options.max_spec_bytes;
    while let Some((rel, path)) = documents.next() {
        if Instant::now() >= deadline {
            skipped.extend(
                std::iter::once((rel, path))
                    .chain(documents)
                    .map(|(path, _)| SkippedFile {
                        path,
                        reason: SkipReason::TimedOut,
                    }),
            );
            return SpecScan::TimedOut;
        }
        let limit = options.max_file_bytes.min(remaining);
        let bytes = match read_markdown(&path, limit) {
            Ok(bytes) => bytes,
            Err(reason) => {
                skipped.push(SkippedFile { path: rel, reason });
                continue;
            }
        };
        let read = bytes.len() as u64;
        if read > remaining {
            let reason = SkipReason::SpecCorpusOverBudget {
                budget: options.max_spec_bytes,
            };
            skipped.extend(
                std::iter::once((rel, path))
                    .chain(documents)
                    .map(|(path, _)| SkippedFile {
                        path,
                        reason: reason.clone(),
                    }),
            );
            return SpecScan::CorpusOverBudget {
                budget: options.max_spec_bytes,
            };
        }
        remaining -= read;
        if read > options.max_file_bytes {
            skipped.push(SkippedFile {
                path: rel,
                reason: SkipReason::TooLarge {
                    bytes: read,
                    cap: options.max_file_bytes,
                },
            });
            continue;
        }
        // Validated in place: `from_utf8` takes the buffer, it does not copy
        // it.
        let Ok(content) = String::from_utf8(bytes) else {
            skipped.push(SkippedFile {
                path: rel,
                reason: SkipReason::Binary,
            });
            continue;
        };
        if !scan.scan_document(&rel, &content, modules, deadline) {
            skipped.extend(
                std::iter::once((rel, path))
                    .chain(documents)
                    .map(|(path, _)| SkippedFile {
                        path,
                        reason: SkipReason::TimedOut,
                    }),
            );
            return SpecScan::TimedOut;
        }
        // `content`, the only copy of this document, is dropped here, before
        // the next one is read.
    }
    if skipped.is_empty() {
        SpecScan::Complete
    } else {
        SpecScan::Partial
    }
}

/// What [`list_markdown`] found under `spec/`.
struct SpecListing {
    /// Every document to read: root-relative path, path to open. Sorted by
    /// the former.
    documents: Vec<(String, PathBuf)>,
    /// Every entry seen and not listed as a document, with the reason.
    skipped: Vec<SkippedFile>,
    /// `false` when the deadline passed before the listing finished.
    completed: bool,
}

/// Lists every `.md` file under `dir` (inside `root`), following no
/// symlink: only paths, never contents, since [`scan_spec_directory`] reads
/// each document in turn. What it passes over is recorded, the way the main
/// walk records it (see [`walk_repository`]): a directory it cannot list,
/// or an entry it cannot read the type of, as [`SkipReason::Unreadable`]; a
/// document whose path is not UTF-8 as [`SkipReason::NonUtf8Name`]; a
/// symlink that could stand for documents (named `*.md`, or whose target a
/// `stat`, never an open, says is a directory) as
/// [`SkipReason::SymlinkNotFollowed`]; a directory named `.git`, never
/// descended, by the same rule the main walk applies
/// ([`is_git_directory_name`]), as [`SkipReason::GitMetadata`]. Round 5's
/// version descended it, so a nested clone at `spec/` had git's own files
/// read and cited (a branch named `*.md` is enough to make one) while the
/// scan reported itself complete. Checks `deadline` once per entry: a
/// `spec/` tree could itself hold enough entries that listing them is not
/// free.
fn list_markdown(dir: &Path, root: &Path, deadline: Instant) -> SpecListing {
    let mut documents = Vec::new();
    let mut skipped = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let read_dir = match fs::read_dir(&current) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                admit_skip(
                    root,
                    &current,
                    SkipReason::Unreadable(err.to_string()),
                    &mut skipped,
                );
                continue;
            }
        };
        let mut entries: Vec<fs::DirEntry> = Vec::new();
        for entry in read_dir {
            match entry {
                Ok(entry) => entries.push(entry),
                Err(err) => admit_skip(
                    root,
                    &current,
                    SkipReason::Unreadable(err.to_string()),
                    &mut skipped,
                ),
            }
        }
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            if Instant::now() >= deadline {
                documents.sort_by(|a: &(String, PathBuf), b| a.0.cmp(&b.0));
                return SpecListing {
                    documents,
                    skipped,
                    completed: false,
                };
            }
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    admit_skip(
                        root,
                        &path,
                        SkipReason::Unreadable(err.to_string()),
                        &mut skipped,
                    );
                    continue;
                }
            };
            let is_markdown = path.extension().and_then(|ext| ext.to_str()) == Some("md");
            if file_type.is_symlink() {
                if is_markdown || fs::metadata(&path).is_ok_and(|meta| meta.is_dir()) {
                    admit_skip(root, &path, SkipReason::SymlinkNotFollowed, &mut skipped);
                }
                continue;
            }
            if file_type.is_dir() {
                if is_git_directory_name(&entry.file_name()) {
                    admit_skip(root, &path, SkipReason::GitMetadata, &mut skipped);
                } else {
                    pending.push(path);
                }
                continue;
            }
            if !is_markdown {
                continue;
            }
            match rel_path_string(root, &path) {
                Some(rel) => documents.push((rel, path)),
                None => skipped.push(SkippedFile {
                    path: lossy_rel_path_string(root, &path),
                    reason: SkipReason::NonUtf8Name,
                }),
            }
        }
    }
    documents.sort_by(|a, b| a.0.cmp(&b.0));
    SpecListing {
        documents,
        skipped,
        completed: true,
    }
}

/// Reads one `spec/` document, at most `limit + 1` bytes of it, or the
/// reason it could not be. Every entry reaching here was confirmed not a
/// symlink when it was listed, so it was never meant to become one
/// afterward either; [`open_regular_file`], refusing a final symlink, is
/// what refuses it if it did, and what opens a FIFO or device without
/// waiting and refuses it by its own handle: the same rule [`process_file`]
/// applies to a plain (non-symlink) entry, for the same reason.
fn read_markdown(path: &Path, limit: u64) -> std::result::Result<Vec<u8>, SkipReason> {
    let (opened, meta) = open_regular_file(path, false)?;
    read_capped(opened, limit, meta.len()).map_err(|err| SkipReason::Unreadable(err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    // -----------------------------------------------------------------
    // A small, four-language fixture repository
    // -----------------------------------------------------------------

    /// Every file this fixture writes, deliberately in an order that is
    /// neither alphabetical nor directory-grouped, so that a builder which
    /// forgot to sort would produce an order that follows this one instead
    /// of `path`. `tests::ori_t_0036_output_order_does_not_follow_directory_iteration_order`
    /// is what that matters for.
    fn write_fixture_repo(root: &Path) {
        write(root, "rust/src/foo.rs", RUST_FOO);
        write(root, "go/pkg/main.go", GO_MAIN);
        write(root, "py/pkg/util.py", PY_UTIL);
        write(
            root,
            "assets/logo.png",
            "not really a png, just unsupported",
        );
        write(root, "rust/src/lib.rs", RUST_LIB);
        write(root, "web/util.test.ts", TS_UTIL_TEST);
        write(root, "py/pkg/core.py", PY_CORE);
        write(root, "rust/src/main.rs", RUST_MAIN);
        write(root, "web/index.ts", TS_INDEX);
        write(root, "go/pkg/main_test.go", GO_MAIN_TEST);
        write(root, "py/pkg/test_core.py", PY_TEST_CORE);
        write(root, "web/util.ts", TS_UTIL);
        write(
            root,
            "spec/LLD.md",
            "# 2. Crate responsibilities\n\nThe module at rust/src/foo.rs is Foo.\n",
        );
    }

    const RUST_LIB: &str = "pub mod foo;\nuse crate::foo::Foo;\nuse std::collections::HashMap;\n\npub fn top(x: i32) -> i32 {\n    x\n}\n";
    const RUST_FOO: &str = "pub struct Foo;\n\n#[test]\nfn it_works() {\n    assert!(true);\n}\n";
    const RUST_MAIN: &str = "fn main() {\n    println!(\"hi\");\n}\n";
    const TS_INDEX: &str = "import { helper } from \"./util\";\n\nexport function run(): number {\n    return helper(1);\n}\n\nfunction main() {}\n";
    const TS_UTIL: &str = "export function helper(x: number): number {\n    return x + 1;\n}\n";
    const TS_UTIL_TEST: &str = "test(\"helper adds one\", () => {\n    helper(1);\n});\n";
    const PY_CORE: &str =
        "def compute(x):\n    return x + 1\n\n\nif __name__ == \"__main__\":\n    compute(1)\n";
    const PY_UTIL: &str = "from . import core\n\n\ndef helper():\n    return core.compute(2)\n";
    const PY_TEST_CORE: &str = "def test_compute():\n    assert True\n";
    const GO_MAIN: &str = "package main\n\nimport \"fmt\"\n\nfunc Hello() string {\n    return \"hi\"\n}\n\nfunc main() {\n    fmt.Println(Hello())\n}\n";
    const GO_MAIN_TEST: &str =
        "package main\n\nimport \"testing\"\n\nfunc TestHello(t *testing.T) {\n}\n";

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture directory");
        }
        fs::write(path, content).expect("write fixture file");
    }

    /// A [`KnownPaths`] holding exactly `paths`, as the walk would build it.
    fn known_paths(paths: &[&str]) -> KnownPaths {
        paths.iter().map(|path| Arc::from(*path)).collect()
    }

    fn temp_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let unique = format!(
            "ori-t-0036-{label}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
        );
        dir.push(unique);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    struct DropGuard(PathBuf);
    impl Drop for DropGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn built_fixture() -> (DropGuard, CodeMap) {
        let dir = temp_dir("fixture");
        let guard = DropGuard(dir.clone());
        write_fixture_repo(&dir);
        let map = build_code_map(&dir).expect("fixture repository maps");
        (guard, map)
    }

    // -----------------------------------------------------------------
    // ORI-P1-030: repository of 50,000 lines, 4 languages | memory.context
    // -----------------------------------------------------------------

    /// Trap 2, and Plant 4: a map over a real repository, before anything
    /// about it is asserted absent, has nonzero modules, interfaces and
    /// edges.
    #[test]
    fn ori_p1_030_a_four_language_repository_produces_a_non_vacuous_map() {
        let (_guard, map) = built_fixture();

        assert!(!map.modules.is_empty(), "no modules found at all");
        let total_interfaces: usize = map.modules.iter().map(|m| m.interfaces.len()).sum();
        assert!(total_interfaces > 0, "no interfaces found in any module");
        let total_edges: usize = map.modules.iter().map(|m| m.dependency_edges.len()).sum();
        assert!(total_edges > 0, "no dependency edges found in any module");
        let total_entry_points: usize = map.modules.iter().map(|m| m.entry_points.len()).sum();
        assert!(
            total_entry_points > 0,
            "no entry points found in any module"
        );
        let total_tests: usize = map.modules.iter().map(|m| m.covering_tests.len()).sum();
        assert!(total_tests > 0, "no covering tests found for any module");

        // Only after the positive counts are asserted does an absence claim
        // about a single, specific module make sense.
        let logo = map
            .coverage
            .files_skipped
            .iter()
            .find(|f| f.path == "assets/logo.png");
        assert!(
            logo.is_some(),
            "the unsupported file should be skipped, not absent"
        );
    }

    /// The four languages are all represented, each with at least one
    /// module, so "4 languages" in the criterion's own text is checked
    /// directly rather than assumed from the fixture's file list.
    #[test]
    fn ori_p1_030_all_four_named_languages_are_represented() {
        let (_guard, map) = built_fixture();
        for language in [
            Language::Rust,
            Language::TypeScript,
            Language::Python,
            Language::Go,
        ] {
            assert!(
                map.modules.iter().any(|m| m.language == language),
                "no module found for {language}"
            );
        }
    }

    /// `files_seen` accounts for every file exactly once, including the
    /// unsupported one: Plant 2, "an unsupported file silently dropped".
    #[test]
    fn ori_p1_030_coverage_accounts_for_every_file_seen_exactly_once() {
        let (_guard, map) = built_fixture();
        let c = &map.coverage;
        assert_eq!(
            c.files_seen,
            c.files_parsed_clean + c.files_parsed_with_errors + c.files_skipped.len()
        );
        assert!(c.files_seen > 0);
    }

    /// The same repository, walked twice from two independently created
    /// copies (files written in a different order each time), gives a
    /// byte-identical `Debug` rendering. Plant 3 and Plant 5's positive case.
    #[test]
    fn ori_p1_030_the_same_repository_gives_byte_identical_output_regardless_of_iteration_order() {
        let dir_a = temp_dir("order-a");
        let guard_a = DropGuard(dir_a.clone());
        write(&dir_a, "z_last.rs", "pub fn z() {}\n");
        write(&dir_a, "a_first.rs", "pub fn a() {}\n");
        write(&dir_a, "m_middle.rs", "pub fn m() {}\n");

        let dir_b = temp_dir("order-b");
        let guard_b = DropGuard(dir_b.clone());
        write(&dir_b, "a_first.rs", "pub fn a() {}\n");
        write(&dir_b, "m_middle.rs", "pub fn m() {}\n");
        write(&dir_b, "z_last.rs", "pub fn z() {}\n");

        let map_a = build_code_map(&dir_a).expect("maps");
        let map_b = build_code_map(&dir_b).expect("maps");

        assert_eq!(format!("{map_a:?}"), format!("{map_b:?}"));
        drop(guard_a);
        drop(guard_b);
    }

    #[test]
    fn ori_p1_030_the_report_never_calls_a_map_with_parse_errors_complete() {
        let dir = temp_dir("errors");
        let guard = DropGuard(dir.clone());
        write(&dir, "broken.rs", "fn broken( { let x = ; }\n");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.files_parsed_with_errors, 1);
        assert_eq!(map.coverage.files_parsed_clean, 0);
        let module = map
            .modules
            .iter()
            .find(|m| m.path == "broken.rs")
            .expect("module present");
        assert!(module.parsed_with_errors);
        drop(guard);
    }

    /// The manual, long-running measurement this ticket's report cites. Not
    /// part of the default suite (Standing trap 8): run with
    /// `cargo test -p ori-memory --release -- --ignored ori_p1_030_the_fifty_thousand_line_measurement -- --nocapture`.
    #[test]
    #[ignore = "long-running measurement over a generated 50,000 line, four language repository; run manually"]
    fn ori_p1_030_the_fifty_thousand_line_measurement() {
        let dir = temp_dir("fifty-thousand");
        let guard = DropGuard(dir.clone());
        let total_lines = generate_large_repo(&dir, 50_000);

        let start = Instant::now();
        let map = build_code_map(&dir).expect("large repository maps");
        let elapsed = start.elapsed();

        let debug_bytes = format!("{map:?}").len();
        eprintln!(
            "ORI-P1-030 measurement: generated_lines={total_lines} files_seen={} modules={} \
             interfaces={} edges={} entry_points={} elapsed={elapsed:?} debug_rendering_bytes={debug_bytes}",
            map.coverage.files_seen,
            map.modules.len(),
            map.modules
                .iter()
                .map(|m| m.interfaces.len())
                .sum::<usize>(),
            map.modules
                .iter()
                .map(|m| m.dependency_edges.len())
                .sum::<usize>(),
            map.modules
                .iter()
                .map(|m| m.entry_points.len())
                .sum::<usize>(),
        );
        assert!(!map.modules.is_empty());
        drop(guard);
    }

    /// Writes a four-language repository whose total source line count is at
    /// least `target_lines`, and returns the actual total. Every file is
    /// valid, parseable source (real functions), not padding, so the
    /// measurement reflects real parsing and extraction work.
    fn generate_large_repo(root: &Path, target_lines: usize) -> usize {
        let mut total = 0usize;
        let per_language = target_lines / 4;
        for (extension, body_fn) in [
            ("rs", rust_body as fn(usize) -> String),
            ("ts", ts_body as fn(usize) -> String),
            ("py", py_body as fn(usize) -> String),
            ("go", go_body as fn(usize) -> String),
        ] {
            // Sized from one real file's own line count rather than a guess,
            // so `total` reliably reaches `target_lines` regardless of how
            // many lines each language's template happens to produce.
            let lines_per_file = body_fn(0).lines().count().max(1);
            let files_needed = per_language.div_ceil(lines_per_file).max(1);
            for file_index in 0..files_needed {
                let content = body_fn(file_index);
                total += content.lines().count();
                write(
                    root,
                    &format!("gen/{extension}/file_{file_index}.{extension}"),
                    &content,
                );
            }
        }

        // One small, hand-written file per language, on top of the bulk
        // above, so the measurement also exercises edge resolution and
        // entry-point detection at this scale rather than reporting zero for
        // both: the bulk files above are deliberately flat (no imports, no
        // `main`), to keep the generator simple and its volume predictable.
        write(
            root,
            "gen/rs/lib.rs",
            "mod file_0;\n\nfn main() {\n    let _ = file_0::f_0_0(1);\n}\n",
        );
        write(
            root,
            "gen/ts/index.ts",
            "import { f_0_0 } from \"./file_0\";\n\nfunction main() {\n    f_0_0(1);\n}\n",
        );
        write(
            root,
            "gen/py/__main__.py",
            "from . import file_0\n\n\ndef run():\n    return file_0.f_0_0(1)\n\n\nif __name__ == \"__main__\":\n    run()\n",
        );
        write(
            root,
            "gen/go/entry/main.go",
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n    fmt.Println(\"done\")\n}\n",
        );

        total
    }

    fn rust_body(file_index: usize) -> String {
        let mut out = String::new();
        for n in 0..40 {
            out.push_str(&format!(
                "pub fn f_{file_index}_{n}(x: i32) -> i32 {{\n    x + {n}\n}}\n\n"
            ));
        }
        out
    }

    fn ts_body(file_index: usize) -> String {
        let mut out = String::new();
        for n in 0..40 {
            out.push_str(&format!(
                "export function f_{file_index}_{n}(x: number): number {{\n    return x + {n};\n}}\n\n"
            ));
        }
        out
    }

    fn py_body(file_index: usize) -> String {
        let mut out = String::new();
        for n in 0..40 {
            out.push_str(&format!(
                "def f_{file_index}_{n}(x):\n    return x + {n}\n\n\n"
            ));
        }
        out
    }

    fn go_body(file_index: usize) -> String {
        let mut out = String::new();
        out.push_str("package generated\n\n");
        for n in 0..38 {
            out.push_str(&format!(
                "func F_{file_index}_{n}(x int) int {{\n    return x + {n}\n}}\n\n"
            ));
        }
        out
    }

    // -----------------------------------------------------------------
    // Trap 1 / Plant 1: parse errors are never counted as clean
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_files_with_parse_errors_are_never_counted_as_clean() {
        let dir = temp_dir("plant-1");
        let guard = DropGuard(dir.clone());
        write(&dir, "ok.rs", "pub fn ok() -> i32 { 1 }\n");
        write(&dir, "broken.rs", "fn broken( { let x = ; }\n");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.files_parsed_clean, 1);
        assert_eq!(map.coverage.files_parsed_with_errors, 1);
        let broken = map
            .modules
            .iter()
            .find(|m| m.path == "broken.rs")
            .expect("present");
        assert!(broken.parsed_with_errors);
        let ok = map
            .modules
            .iter()
            .find(|m| m.path == "ok.rs")
            .expect("present");
        assert!(!ok.parsed_with_errors);
        drop(guard);
    }

    // -----------------------------------------------------------------
    // Trap 2 / Plant 2: an unsupported file is seen and skipped, not dropped
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_an_unsupported_file_is_seen_and_skipped_with_a_reason() {
        let dir = temp_dir("plant-2");
        let guard = DropGuard(dir.clone());
        write(&dir, "notes.txt", "just some notes, not source\n");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.files_seen, 1);
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert_eq!(map.coverage.files_skipped[0].path, "notes.txt");
        assert_eq!(
            map.coverage.files_skipped[0].reason,
            SkipReason::UnsupportedLanguage
        );
        drop(guard);
    }

    // -----------------------------------------------------------------
    // Trap 3 / Plant 3: output order does not follow directory iteration
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_output_order_does_not_follow_directory_iteration_order() {
        let (_guard, map) = built_fixture();
        let paths: Vec<&str> = map.modules.iter().map(|m| m.path.as_str()).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        assert_eq!(
            paths, sorted,
            "modules must be sorted by path, not creation order"
        );
    }

    #[test]
    fn ori_t_0036_per_module_lists_are_sorted_deterministically() {
        let (_guard, map) = built_fixture();
        for module in &map.modules {
            let mut edges_sorted = module.dependency_edges.clone();
            edges_sorted.sort_by(|a, b| (a.external, &a.to).cmp(&(b.external, &b.to)));
            assert_eq!(
                module.dependency_edges, edges_sorted,
                "{} edges not sorted",
                module.path
            );

            let mut tests_sorted = module.covering_tests.clone();
            tests_sorted.sort();
            assert_eq!(
                module.covering_tests, tests_sorted,
                "{} tests not sorted",
                module.path
            );
        }
    }

    // -----------------------------------------------------------------
    // Bounds: size, time, symlinks, traversal
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_a_file_over_the_size_cap_is_skipped_and_never_fully_read() {
        let dir = temp_dir("too-large");
        let guard = DropGuard(dir.clone());
        write(&dir, "big.rs", "pub fn big() -> i32 { 1 }\n");
        let options = CodeMapOptions {
            max_file_bytes: 4,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        match &map.coverage.files_skipped[0].reason {
            SkipReason::TooLarge { cap, .. } => assert_eq!(*cap, 4),
            other => panic!("expected TooLarge, got {other:?}"),
        }
        drop(guard);
    }

    /// Defect 1 and 8 (the `take(cap + 1)` bound) from this ticket's
    /// adversarial review: the test above checks the returned `SkipReason`,
    /// not what was actually read, so a mutant that replaced the bounded
    /// `Read::take` with a `stat`-then-`fs::read` (reporting the file's real
    /// size in `bytes`) survives it undetected. A file far larger than the
    /// cap is used here, and `bytes` is asserted close to the cap, never
    /// close to the file's real size, which only a bounded read can report.
    #[test]
    fn ori_t_0036_the_size_cap_bounds_bytes_actually_read_not_a_stated_size() {
        let dir = temp_dir("too-large-bounded");
        let guard = DropGuard(dir.clone());
        let big = vec![b'a'; 10_000_000];
        fs::write(dir.join("big.rs"), &big).expect("write big fixture file");
        let options = CodeMapOptions {
            max_file_bytes: 100,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        match &map.coverage.files_skipped[0].reason {
            SkipReason::TooLarge { cap, bytes } => {
                assert_eq!(*cap, 100);
                assert!(
                    *bytes <= 101,
                    "bytes reported ({bytes}) must be bounded near the cap (101), not the \
                     file's real size ({}), which is what a bounded read means",
                    big.len()
                );
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
        drop(guard);
    }

    #[test]
    fn ori_t_0036_a_binary_file_with_a_supported_extension_is_skipped_as_binary() {
        let dir = temp_dir("binary");
        let guard = DropGuard(dir.clone());
        fs::write(dir.join("weird.py"), [0u8, 159, 146, 150]).expect("write binary");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert_eq!(map.coverage.files_skipped[0].reason, SkipReason::Binary);
        drop(guard);
    }

    #[test]
    fn ori_t_0036_a_root_that_does_not_exist_is_refused_not_reported_as_empty() {
        let dir = temp_dir("missing-parent");
        let missing = dir.join("does-not-exist");
        let result = build_code_map(&missing);
        assert!(
            result.is_err(),
            "a missing root must error, not silently map to nothing"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ori_t_0036_a_root_that_is_a_file_is_refused() {
        let dir = temp_dir("root-is-file");
        let guard = DropGuard(dir.clone());
        let file_path = dir.join("not-a-directory");
        fs::write(&file_path, "x").expect("write file");
        let result = build_code_map(&file_path);
        assert!(matches!(result, Err(Error::NotADirectory { .. })));
        drop(guard);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlink_outside_the_root_is_skipped_and_never_opened() {
        use std::os::unix::fs::symlink;
        let dir = temp_dir("symlink-outside");
        let guard = DropGuard(dir.clone());
        let outside_dir = temp_dir("symlink-outside-target");
        let outside_guard = DropGuard(outside_dir.clone());
        let outside_file = outside_dir.join("secret.rs");
        fs::write(&outside_file, "pub fn secret() {}\n").expect("write outside file");
        symlink(&outside_file, dir.join("link.rs")).expect("create symlink");

        let map = build_code_map(&dir).expect("maps");
        assert_eq!(
            map.modules.len(),
            0,
            "the symlink target must never be parsed"
        );
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert_eq!(
            map.coverage.files_skipped[0].reason,
            SkipReason::SymlinkOutsideRoot
        );
        drop(guard);
        drop(outside_guard);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_broken_symlink_is_unreadable_not_a_crash() {
        use std::os::unix::fs::symlink;
        let dir = temp_dir("symlink-broken");
        let guard = DropGuard(dir.clone());
        symlink(dir.join("nowhere.rs"), dir.join("broken_link.rs"))
            .expect("create dangling symlink");
        let map = build_code_map(&dir).expect("maps, does not crash");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert!(matches!(
            map.coverage.files_skipped[0].reason,
            SkipReason::Unreadable(_)
        ));
        drop(guard);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlinked_directory_cycle_does_not_hang_or_double_count() {
        use std::os::unix::fs::symlink;
        let dir = temp_dir("symlink-cycle");
        let guard = DropGuard(dir.clone());
        write(&dir, "real.rs", "pub fn real_fn() {}\n");
        symlink(&dir, dir.join("self_loop")).expect("create self-referential symlink");

        let map = build_code_map(&dir).expect("maps without hanging");
        assert_eq!(
            map.modules.len(),
            1,
            "the symlinked directory must not be descended into"
        );
        drop(guard);
    }

    // -----------------------------------------------------------------
    // Language-specific extraction
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_rust_mod_declaration_resolves_to_the_file_it_declares() {
        let (_guard, map) = built_fixture();
        let lib = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/lib.rs")
            .expect("present");
        let mod_edge = lib
            .dependency_edges
            .iter()
            .find(|e| e.to == "rust/src/foo.rs")
            .expect("mod foo; resolves to rust/src/foo.rs");
        assert!(!mod_edge.external);
    }

    #[test]
    fn ori_t_0036_rust_use_crate_path_resolves_when_the_target_exists() {
        let (_guard, map) = built_fixture();
        let lib = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/lib.rs")
            .expect("present");
        let use_edge = lib
            .dependency_edges
            .iter()
            .find(|e| e.to == "rust/src/foo.rs" && !e.external);
        assert!(
            use_edge.is_some(),
            "use crate::foo::Foo; should resolve to rust/src/foo.rs"
        );
        let external = lib
            .dependency_edges
            .iter()
            .any(|e| e.external && e.to.contains("HashMap"));
        assert!(
            external,
            "use std::collections::HashMap; should be external"
        );
    }

    /// Defect 5 from this ticket's adversarial review: the test above,
    /// `tests::ori_t_0036_rust_use_crate_path_resolves_when_the_target_exists`,
    /// passes even when `use crate::foo::Foo;` never resolves, because its
    /// `.find(|e| e.to == "rust/src/foo.rs" && !e.external)` also matches
    /// the unrelated `mod foo;` edge to the same target. CLAUDE.md's
    /// absolute rule 3 ("never modify or delete an existing test") is why
    /// that test's body is untouched here rather than tightened in place;
    /// this is a new, narrower test added alongside it. The old resolver
    /// (`resolve_rust_crate_path` before this fix) tried candidates
    /// relative to the mapped root directly (`foo.rs`, `foo/mod.rs`), which
    /// never matched `rust/src/foo.rs` in this crate's real `src/` layout,
    /// so `use crate::foo::Foo;` was always external before the fix in
    /// `rust_crate_root`; asserting the resolved-edge count is 2, not 1,
    /// is what distinguishes "both `mod foo;` and `use crate::foo::Foo;`
    /// resolved" from "only the `mod` edge happens to share the target".
    #[test]
    fn ori_t_0036_rust_use_crate_path_resolves_to_the_specific_edge_named() {
        let (_guard, map) = built_fixture();
        let lib = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/lib.rs")
            .expect("present");
        let resolved_to_foo = lib
            .dependency_edges
            .iter()
            .filter(|e| e.to == "rust/src/foo.rs" && !e.external)
            .count();
        assert_eq!(
            resolved_to_foo, 2,
            "expected both `mod foo;` and `use crate::foo::Foo;` to resolve to \
             rust/src/foo.rs, got: {:?}",
            lib.dependency_edges
        );
        let still_unresolved_text = lib
            .dependency_edges
            .iter()
            .any(|e| e.to.contains("crate::foo::Foo"));
        assert!(
            !still_unresolved_text,
            "use crate::foo::Foo; must not still be recorded as unresolved external text: {:?}",
            lib.dependency_edges
        );
    }

    /// The crate-root discovery `use crate::` resolution now depends on:
    /// `rust_crate_root` walks up from the importing file's own path to find
    /// the nearest ancestor directly containing `lib.rs` or `main.rs`. This
    /// asserts that search directly, independent of any particular fixture
    /// module's edges.
    #[test]
    fn ori_t_0036_rust_crate_root_is_found_by_walking_up_from_the_importing_file() {
        let known = known_paths(&[
            "crates/x/src/lib.rs",
            "crates/x/src/foo.rs",
            "crates/x/src/nested/bar.rs",
        ]);
        assert_eq!(
            rust_crate_root("crates/x/src/foo.rs", &known),
            Some("crates/x/src".to_owned())
        );
        assert_eq!(
            rust_crate_root("crates/x/src/nested/bar.rs", &known),
            Some("crates/x/src".to_owned()),
            "a file nested under src/ must still find src/ itself, not its own directory"
        );
        let no_root = known_paths(&["somewhere/deep/file.rs"]);
        assert_eq!(
            rust_crate_root("somewhere/deep/file.rs", &no_root),
            None,
            "no lib.rs or main.rs anywhere above the file means no discoverable crate root"
        );
    }

    #[test]
    fn ori_t_0036_rust_pub_items_are_interfaces_and_main_is_an_entry_point() {
        let (_guard, map) = built_fixture();
        let lib = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/lib.rs")
            .expect("present");
        assert!(
            lib.interfaces
                .iter()
                .any(|i| i.name == "top" && i.kind == InterfaceKind::Function)
        );
        assert!(
            lib.interfaces
                .iter()
                .any(|i| i.name == "foo" && i.kind == InterfaceKind::Module)
        );

        let foo = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/foo.rs")
            .expect("present");
        assert!(
            foo.interfaces
                .iter()
                .any(|i| i.name == "Foo" && i.kind == InterfaceKind::Type)
        );
        assert!(foo.covering_tests.contains(&"it_works".to_owned()));

        let main = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/main.rs")
            .expect("present");
        assert!(main.entry_points.iter().any(|e| e.name == "main"));
    }

    #[test]
    fn ori_t_0036_typescript_relative_import_resolves_and_export_is_an_interface() {
        let (_guard, map) = built_fixture();
        let index = map
            .modules
            .iter()
            .find(|m| m.path == "web/index.ts")
            .expect("present");
        let edge = index
            .dependency_edges
            .iter()
            .find(|e| e.to == "web/util.ts")
            .expect("resolved");
        assert!(!edge.external);
        assert!(
            index
                .interfaces
                .iter()
                .any(|i| i.name == "run" && i.kind == InterfaceKind::Function)
        );
        assert!(index.entry_points.iter().any(|e| e.name == "main"));

        let util = map
            .modules
            .iter()
            .find(|m| m.path == "web/util.ts")
            .expect("present");
        assert!(util.covering_tests.contains(&"web/util.test.ts".to_owned()));

        let util_test = map
            .modules
            .iter()
            .find(|m| m.path == "web/util.test.ts")
            .expect("present");
        assert!(
            util_test
                .covering_tests
                .contains(&"helper adds one".to_owned())
        );
    }

    #[test]
    fn ori_t_0036_python_relative_import_resolves_and_name_guard_is_an_entry_point() {
        let (_guard, map) = built_fixture();
        let util = map
            .modules
            .iter()
            .find(|m| m.path == "py/pkg/util.py")
            .expect("present");
        let edge = util
            .dependency_edges
            .iter()
            .find(|e| e.to == "py/pkg/core.py")
            .expect("from . import core; resolves");
        assert!(!edge.external);

        let core = map
            .modules
            .iter()
            .find(|m| m.path == "py/pkg/core.py")
            .expect("present");
        assert!(core.entry_points.iter().any(|e| e.name == "__main__"));
        assert!(core.interfaces.iter().any(|i| i.name == "compute"));
        assert!(
            core.covering_tests
                .contains(&"py/pkg/test_core.py".to_owned())
        );

        let test_core = map
            .modules
            .iter()
            .find(|m| m.path == "py/pkg/test_core.py")
            .expect("present");
        assert!(
            test_core
                .covering_tests
                .contains(&"test_compute".to_owned())
        );
    }

    #[test]
    fn ori_t_0036_go_exported_identifiers_are_interfaces_and_package_main_gates_the_entry_point() {
        let (_guard, map) = built_fixture();
        let main = map
            .modules
            .iter()
            .find(|m| m.path == "go/pkg/main.go")
            .expect("present");
        assert!(
            main.interfaces
                .iter()
                .any(|i| i.name == "Hello" && i.kind == InterfaceKind::Function)
        );
        assert!(main.entry_points.iter().any(|e| e.name == "main"));
        assert!(
            main.covering_tests
                .contains(&"go/pkg/main_test.go".to_owned())
        );

        let import = main.dependency_edges.iter().find(|e| e.to == "fmt");
        assert!(
            import.is_some(),
            "Go edges are always external; see the module doc"
        );
        assert!(import.unwrap().external);

        let test_file = map
            .modules
            .iter()
            .find(|m| m.path == "go/pkg/main_test.go")
            .expect("present");
        assert!(test_file.covering_tests.contains(&"TestHello".to_owned()));
    }

    /// Defect 10 (M5) from this ticket's adversarial review: nothing in the
    /// main fixture has a `func main()` outside `package main`, so the test
    /// above cannot tell "gated on package main" apart from "any func main
    /// is an entry point"; a mutant that deleted the `package_name == "main"`
    /// half of the check survived. Added alongside that test, per CLAUDE.md
    /// rule 3, rather than edited into it.
    #[test]
    fn ori_t_0036_go_func_main_outside_package_main_is_not_an_entry_point() {
        let dir = temp_dir("go-not-main");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "helper/helper.go",
            "package helper\n\nfunc main() {\n}\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let helper = map
            .modules
            .iter()
            .find(|m| m.path == "helper/helper.go")
            .expect("present");
        assert!(
            helper.entry_points.is_empty(),
            "func main() in package helper (not package main) must not be an entry point: {:?}",
            helper.entry_points
        );
        drop(guard);
    }

    // -----------------------------------------------------------------
    // Spec citations
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_a_module_cited_by_path_in_spec_is_linked_to_its_heading() {
        let (_guard, map) = built_fixture();
        let foo = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/foo.rs")
            .expect("present");
        assert!(
            foo.spec_sections
                .iter()
                .any(|c| map.spec_doc(c) == Some("spec/LLD.md")
                    && map.spec_heading(c).map(|h| h.text.as_str())
                        == Some("2. Crate responsibilities")),
            "{:?}",
            foo.spec_sections
        );
    }

    #[test]
    fn ori_t_0036_a_module_never_cited_has_no_spec_sections() {
        let (_guard, map) = built_fixture();
        let main = map
            .modules
            .iter()
            .find(|m| m.path == "rust/src/main.rs")
            .expect("present");
        assert!(main.spec_sections.is_empty());
    }

    // -----------------------------------------------------------------
    // Coverage error type
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0036_the_error_display_names_the_path() {
        let dir = temp_dir("display");
        let missing = dir.join("nope");
        let err = build_code_map(&missing).expect_err("missing root errors");
        let rendered = err.to_string();
        assert!(rendered.contains("nope"), "{rendered}");
        let _ = fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------
    // ORI-T-0036 adversarial review (2026-09-23): one test per defect that
    // failed before the fix in this section and passes after. The plant
    // table in the report records which; CLAUDE.md rule 3 ("never modify or
    // delete an existing test") is why every one of these is a new test
    // added here rather than a change to an existing test's body, even
    // where the review's own suggested fix was to strengthen one in place.
    // -----------------------------------------------------------------

    /// Defect 1 (HIGH): a symlink to a FIFO used to be accepted as a
    /// candidate file (`walk_repository` only checked "not a directory"),
    /// and `process_file` then called `fs::read` on it, which blocks
    /// forever with no writer present. Bounded with a channel and a 10 s
    /// `recv_timeout` rather than run unbounded in the default suite: if the
    /// fix regresses, this fails loudly instead of hanging `cargo test`
    /// itself.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlink_to_a_fifo_is_skipped_not_a_hang() {
        let dir = temp_dir("fifo-symlink");
        let guard = DropGuard(dir.clone());
        write(&dir, "ok.rs", "pub fn ok() {}\n");
        let pipe = dir.join("pipe.rs");
        let mkfifo_ok = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !mkfifo_ok {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            return;
        }
        std::os::unix::fs::symlink(&pipe, dir.join("link.rs")).expect("create symlink to fifo");

        let (tx, rx) = std::sync::mpsc::channel();
        let dir_clone = dir.clone();
        std::thread::spawn(move || {
            let result = build_code_map(&dir_clone);
            let _ = tx.send(result);
        });
        let result = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("build_code_map must return within 10s; a symlink to a FIFO must never block");
        let map = result.expect("maps despite the FIFO");
        assert!(
            map.modules
                .iter()
                .all(|m| m.path != "link.rs" && m.path != "pipe.rs"),
            "neither the FIFO nor a symlink to it may become a module: {:?}",
            map.modules.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
        drop(guard);
    }

    /// Defect 1 (HIGH), the other half: a FIFO named `*.md` directly under
    /// `spec/` used to hang `collect_markdown`'s `read_to_string`, since
    /// that function never checked the entry was a regular file at all.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_fifo_under_spec_is_skipped_not_a_hang() {
        let dir = temp_dir("fifo-spec");
        let guard = DropGuard(dir.clone());
        write(&dir, "src/foo.rs", "pub fn foo() {}\n");
        fs::create_dir_all(dir.join("spec")).expect("create spec dir");
        let pipe = dir.join("spec").join("notes.md");
        let mkfifo_ok = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !mkfifo_ok {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let dir_clone = dir.clone();
        std::thread::spawn(move || {
            let result = build_code_map(&dir_clone);
            let _ = tx.send(result);
        });
        let result = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("build_code_map must return within 10s; a FIFO under spec/ must never block");
        result.expect("maps despite the FIFO under spec/");
        drop(guard);
    }

    /// Defect 2 (HIGH): `attach_spec_citations` used to resolve `spec` with
    /// `root.join("spec").is_dir()`, which follows a symlink, so a
    /// repository whose `spec` entry is a symlink to anywhere had that
    /// target's whole tree read, and heading text from outside the mapped
    /// root leaked into the map.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_spec_symlink_is_never_followed_outside_the_root() {
        let dir = temp_dir("spec-symlink");
        let guard = DropGuard(dir.clone());
        let outside = temp_dir("spec-symlink-outside");
        let outside_guard = DropGuard(outside.clone());
        write(
            &outside,
            "private/notes.md",
            "# PRIVATE: do not leak\n\nsee src/foo.rs\n",
        );
        write(&dir, "src/foo.rs", "pub fn foo() {}\n");
        std::os::unix::fs::symlink(&outside, dir.join("spec")).expect("create spec symlink");

        let map = build_code_map(&dir).expect("maps");
        let foo = map
            .modules
            .iter()
            .find(|m| m.path == "src/foo.rs")
            .expect("present");
        assert!(
            foo.spec_sections.is_empty(),
            "a symlinked spec/ must never be read: {:?}",
            foo.spec_sections
        );
        drop(guard);
        drop(outside_guard);
    }

    /// Defect 11 (LOW): `spec` used to be resolved with `root.join("spec")`,
    /// which a case-insensitive filesystem (macOS APFS, Windows NTFS by
    /// default) matches against a directory actually named `Spec` or
    /// `SPEC`, giving a different map on Linux for the identical repository.
    /// This assertion is the same on every platform: `Spec` (capital S)
    /// must never be read as `spec/`, which the exact `OsStr` comparison in
    /// `attach_spec_citations` now guarantees regardless of the underlying
    /// filesystem's own lookup rules.
    #[test]
    fn ori_t_0036_the_spec_directory_is_matched_case_sensitively_on_every_platform() {
        let dir = temp_dir("spec-case");
        let guard = DropGuard(dir.clone());
        write(&dir, "src/foo.rs", "pub fn foo() {}\n");
        write(&dir, "Spec/LLD.md", "# Heading\n\nsrc/foo.rs\n");
        let map = build_code_map(&dir).expect("maps");
        let foo = map
            .modules
            .iter()
            .find(|m| m.path == "src/foo.rs")
            .expect("present");
        assert!(
            foo.spec_sections.is_empty(),
            "a directory named Spec (capital S) must never be treated as spec/: {:?}",
            foo.spec_sections
        );
        let skip = map
            .coverage
            .files_skipped
            .iter()
            .any(|f| f.path == "Spec/LLD.md");
        assert!(
            skip,
            "Spec/LLD.md must still be seen and skipped as an unsupported file, not vanish either"
        );
        drop(guard);
    }

    /// Defect 3 (HIGH): `rust_marked_test` used to call `Node::prev_sibling`
    /// once per `function_item` in the whole file. In tree-sitter 0.25.10
    /// that method walks down from the tree's root every time
    /// (`ts_node_parent`), so a file of `D` nested functions cost O(D^2) to
    /// extract; the review measured 178s at D=40000 (320 KB, 4% of the
    /// default size cap), parsed clean, entirely outside the parse timeout,
    /// because nothing bounded the extraction step that followed it. `D`
    /// here is smaller (so this test stays fast) but the shape is the same;
    /// the wall-clock bound is far under what the old code would have taken
    /// at this depth (about 27s at D=20000, by the review's own
    /// measurement).
    #[test]
    fn ori_t_0036_nested_rust_functions_extract_in_linear_not_quadratic_time() {
        let dir = temp_dir("quadratic");
        let guard = DropGuard(dir.clone());
        let depth = 20_000;
        let mut content = String::with_capacity(depth * 8);
        for _ in 0..depth {
            content.push_str("fn a(){");
        }
        for _ in 0..depth {
            content.push('}');
        }
        content.push('\n');
        write(&dir, "nested.rs", &content);

        let start = Instant::now();
        let map = build_code_map(&dir).expect("maps");
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(3),
            "extraction over {depth} nested functions took {elapsed:?}; the O(depth) \
             Node::prev_sibling regression took about 27s at this depth"
        );
        let module = map
            .modules
            .iter()
            .find(|m| m.path == "nested.rs")
            .expect("present");
        assert!(
            !module.parsed_with_errors,
            "this input is legal Rust and must parse clean"
        );
        drop(guard);
    }

    /// Defect 3 (HIGH), the other half: the deadline used to cover only the
    /// parse; this proves extraction has its own check, isolated from
    /// tree-sitter's own progress-callback polling (which, for a small
    /// input, may never poll before finishing at all, verified empirically
    /// while building this module). A deadline already in the past, given
    /// directly to `extract_rust` after a real, already-successful parse,
    /// must stop extraction before it does any work.
    #[test]
    fn ori_t_0036_the_whole_file_deadline_bounds_extraction_not_only_parsing() {
        let source = "pub fn a() {}\npub fn b() {}\npub fn c() {}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("set language");
        let tree = parser.parse(source, None).expect("parses");
        let root = tree.root_node();

        let already_past = Instant::now() - Duration::from_secs(1);
        let empty_known = HashSet::new();
        let empty_index = RustModuleIndex::build(&empty_known);
        let (extracted, completed) = extract_rust(
            root,
            source.as_bytes(),
            "x.rs",
            &empty_known,
            &empty_index,
            already_past,
        );
        assert!(
            !completed,
            "extraction must report incomplete once the deadline has already passed"
        );
        assert!(
            extracted.interfaces.is_empty(),
            "nothing should have been extracted before the first deadline check: {:?}",
            extracted.interfaces
        );
    }

    /// Defect 3 (HIGH), end to end: with the whole-file deadline already
    /// expired, a small file (whose parse tree-sitter is expected to finish
    /// before ever polling its cancellation callback) must still come back
    /// `TimedOut`, proving the extraction-side check is what catches it, not
    /// only tree-sitter's own.
    #[test]
    fn ori_t_0036_an_expired_deadline_yields_timed_out_even_when_parsing_alone_would_succeed() {
        let dir = temp_dir("extraction-timeout");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "a.rs",
            "pub fn a() {}\npub fn b() {}\npub fn c() {}\n",
        );
        let options = CodeMapOptions {
            file_timeout: Duration::ZERO,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert_eq!(map.coverage.files_skipped[0].reason, SkipReason::TimedOut);
        drop(guard);
    }

    /// Defect 4 (HIGH): every matching line used to push a fresh
    /// `heading.clone()`, so a spec document within the size cap could still
    /// request unbounded memory (a long heading, many matching lines). 5000
    /// lines citing the same module under the same heading must now
    /// collapse to the one citation the dedup key (doc, heading) allows.
    #[test]
    fn ori_t_0036_spec_citations_are_deduplicated_not_one_allocation_per_matching_line() {
        let dir = temp_dir("spec-dedup");
        let guard = DropGuard(dir.clone());
        write(&dir, "a.rs", "pub fn a() {}\n");
        let mut doc = String::from("# Heading\n\n");
        for _ in 0..5000 {
            doc.push_str("a.rs\n");
        }
        // Assembled, not written whole: the joined path would otherwise read
        // as a citation of a specification document that does not exist,
        // which the citation gate (crates/ori-gates/src/spec_refs.rs)
        // rightly refuses; this is a fixture path, not a citation.
        write(&dir, &format!("spec/{}.md", "x"), &doc);
        let map = build_code_map(&dir).expect("maps");
        let a = map
            .modules
            .iter()
            .find(|m| m.path == "a.rs")
            .expect("present");
        assert_eq!(
            a.spec_sections.len(),
            1,
            "5000 lines citing a.rs under the same heading must collapse to one citation"
        );
        drop(guard);
    }

    /// Defect 4 (HIGH), the cap: many *distinct* headings each citing the
    /// same module are not deduplicated away (each is a real, distinct
    /// citation), so a separate, explicit cap is what bounds that case, with
    /// a recorded truncation marker rather than a silent cutoff (since round
    /// 5, the module's own `spec_sections_truncated` flag rather than an
    /// extra citation whose heading is made-up marker text).
    #[test]
    fn ori_t_0036_spec_citations_are_capped_per_module_with_a_truncation_marker() {
        let dir = temp_dir("spec-cap");
        let guard = DropGuard(dir.clone());
        write(&dir, "a.rs", "pub fn a() {}\n");
        let mut doc = String::new();
        for i in 0..(MAX_SPEC_CITATIONS_PER_MODULE + 50) {
            doc.push_str(&format!("# Heading {i}\n\na.rs\n\n"));
        }
        // Assembled, not written whole: see the comment on the equivalent
        // line in the test above.
        write(&dir, &format!("spec/{}.md", "many"), &doc);
        let map = build_code_map(&dir).expect("maps");
        let a = map
            .modules
            .iter()
            .find(|m| m.path == "a.rs")
            .expect("present");
        // Round 5: the cap is marked by a flag on the module, not by a
        // citation whose heading is a made-up marker text (which would have
        // been interned into the map's heading table as if it were a heading).
        assert_eq!(
            a.spec_sections.len(),
            MAX_SPEC_CITATIONS_PER_MODULE,
            "citations must be capped at exactly the cap"
        );
        assert!(
            a.spec_sections_truncated,
            "the module must be marked truncated when the cap is hit"
        );
        drop(guard);
    }

    // Defect 5 (HIGH): see
    // `ori_t_0036_rust_use_crate_path_resolves_to_the_specific_edge_named`
    // and `ori_t_0036_rust_crate_root_is_found_by_walking_up_from_the_importing_file`,
    // added next to `ori_t_0036_rust_use_crate_path_resolves_when_the_target_exists`
    // above, under "Language-specific extraction".

    /// Defect 6 (MEDIUM): an unreadable subdirectory used to be dropped
    /// with `continue`, taking its entire contents down with it and
    /// recording nothing; the map could look complete while a whole subtree
    /// was invisible. Runs only where `chmod` actually restricts access (not
    /// as root).
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_an_unreadable_subdirectory_is_recorded_not_silently_dropped() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("unreadable-dir");
        let guard = DropGuard(dir.clone());
        write(&dir, "visible.rs", "pub fn visible() {}\n");
        let hidden_dir = dir.join("hidden");
        fs::create_dir_all(&hidden_dir).expect("create hidden dir");
        fs::write(hidden_dir.join("broken.rs"), "pub fn b() {}\n").expect("write file");
        let mut perms = fs::metadata(&hidden_dir).expect("stat").permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&hidden_dir, perms).expect("chmod");

        let still_readable = fs::read_dir(&hidden_dir).is_ok();
        let map = if still_readable {
            None
        } else {
            Some(build_code_map(&dir))
        };

        let mut restore = fs::metadata(&hidden_dir).expect("stat").permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(&hidden_dir, restore);

        let Some(map) = map else {
            eprintln!(
                "running with elevated privileges; chmod 000 did not restrict access, skipping"
            );
            drop(guard);
            return;
        };
        let map = map.expect("maps despite the unreadable subdirectory");
        assert!(map.modules.iter().any(|m| m.path == "visible.rs"));
        assert!(
            map.modules.iter().all(|m| m.path != "hidden/broken.rs"),
            "an unreadable directory's contents cannot be enumerated, so cannot be modules"
        );
        let recorded = map
            .coverage
            .files_skipped
            .iter()
            .any(|f| f.path == "hidden" && matches!(f.reason, SkipReason::Unreadable(_)));
        assert!(
            recorded,
            "the unreadable directory itself must be recorded: {:?}",
            map.coverage.files_skipped
        );
        drop(guard);
    }

    /// Defect 6 (MEDIUM), the root case: the same silent-drop pattern at the
    /// top of the walk used to make an unreadable root indistinguishable
    /// from an empty repository (`Ok`, empty `CodeMap`), contradicting this
    /// module's own documented contract that an unreadable root is a
    /// failure.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_an_unreadable_root_is_a_hard_error_not_an_empty_map() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("unreadable-root");
        let mut perms = fs::metadata(&dir).expect("stat").permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&dir, perms).expect("chmod");

        let still_readable = fs::read_dir(&dir).is_ok();
        let result = if still_readable {
            None
        } else {
            Some(build_code_map(&dir))
        };

        let mut restore = fs::metadata(&dir).expect("stat").permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(&dir, restore);
        let _ = fs::remove_dir_all(&dir);

        let Some(result) = result else {
            eprintln!(
                "running with elevated privileges; chmod 000 did not restrict access, skipping"
            );
            return;
        };
        assert!(
            result.is_err(),
            "an unreadable root must be a hard error, not Ok(an empty-looking map)"
        );
    }

    /// Defect 6 (MEDIUM), non-UTF-8 names: `rel_path_string` returning
    /// `None` used to drop the file with neither a module nor a skip
    /// record. APFS refuses to create such a name at all (confirmed by this
    /// test's own fallback), which is exactly why the review proved this on
    /// Linux instead; this test does the same and no-ops where the
    /// filesystem itself refuses the fixture.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_non_utf8_name_is_recorded_not_silently_dropped() {
        use std::os::unix::ffi::OsStrExt;
        let dir = temp_dir("non-utf8-name");
        let guard = DropGuard(dir.clone());
        write(&dir, "visible.rs", "pub fn visible() {}\n");
        let bad_name = std::ffi::OsStr::from_bytes(b"bad_\xffname.rs");
        let bad_path = dir.join(bad_name);
        if fs::write(&bad_path, "pub fn hidden() {}\n").is_err() {
            eprintln!("this filesystem refuses non-UTF-8 names (expected on macOS/APFS); skipping");
            drop(guard);
            return;
        }
        let map = build_code_map(&dir).expect("maps");
        assert!(map.modules.iter().any(|m| m.path == "visible.rs"));
        let recorded = map
            .coverage
            .files_skipped
            .iter()
            .any(|f| matches!(f.reason, SkipReason::NonUtf8Name));
        assert!(
            recorded,
            "a non-UTF-8 named file must be recorded as skipped, not silently dropped: {:?}",
            map.coverage.files_skipped
        );
        assert_eq!(
            map.coverage.files_seen,
            map.coverage.files_parsed_clean
                + map.coverage.files_parsed_with_errors
                + map.coverage.files_skipped.len()
        );
        drop(guard);
    }

    /// Defect 7 (MEDIUM): a symlinked file used to be reported under its
    /// resolved target's path, not its own, so a link and its target became
    /// two `Module`s at the same path (and, separately, could turn `.git`
    /// content into a module; see the next test).
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlinked_file_is_reported_under_its_own_path_not_its_targets() {
        let dir = temp_dir("symlink-own-path");
        let guard = DropGuard(dir.clone());
        write(&dir, "real.rs", "pub fn real_fn() {}\n");
        std::os::unix::fs::symlink(dir.join("real.rs"), dir.join("alias.rs"))
            .expect("create symlink");
        let map = build_code_map(&dir).expect("maps");
        assert!(map.modules.iter().any(|m| m.path == "real.rs"));
        assert!(
            map.modules.iter().any(|m| m.path == "alias.rs"),
            "the symlink's own path must appear as a module: {:?}",
            map.modules.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
        let real_count = map.modules.iter().filter(|m| m.path == "real.rs").count();
        assert_eq!(real_count, 1, "real.rs must not be duplicated");
        drop(guard);
    }

    // -----------------------------------------------------------------
    // ORI-T-0036 adversarial review, round 3 (2026-09-23): "the fixes
    // addressed instances, not the class." One test per item below that
    // failed before its fix and passes after; the report's plant table
    // says which.
    // -----------------------------------------------------------------

    /// Item 3 (MEDIUM), the mechanism directly: `open_regular_file_no_follow`
    /// must refuse an actual symlink (deterministic, no race needed to prove
    /// the open itself refuses one) and must still open an ordinary file
    /// normally.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_open_regular_file_no_follow_refuses_a_symlink_but_opens_a_plain_file() {
        let dir = temp_dir("no-follow-open");
        let guard = DropGuard(dir.clone());
        write(&dir, "plain.rs", "pub fn plain() {}\n");
        std::os::unix::fs::symlink(dir.join("plain.rs"), dir.join("link.rs"))
            .expect("create symlink");

        let plain_result = open_regular_file_no_follow(&dir.join("plain.rs"));
        assert!(
            plain_result.is_ok(),
            "an ordinary file must still open: {plain_result:?}"
        );

        let link_result = open_regular_file_no_follow(&dir.join("link.rs"));
        assert!(
            link_result.is_err(),
            "a symlink must be refused, not followed"
        );
        drop(guard);
    }

    /// Item 3 (MEDIUM), end to end: a path the walk would have classified
    /// `was_symlink: false` (an ordinary file at walk time) that becomes a
    /// symlink out of the root before `process_file` runs on it (what the
    /// whole-walk race this ticket's report reproduces converges to, for one
    /// file) must be refused, not read through.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_process_file_refuses_a_path_that_became_a_symlink_after_the_walk() {
        let dir = temp_dir("toctou-process-file");
        let guard = DropGuard(dir.clone());
        let outside = temp_dir("toctou-process-file-outside");
        let outside_guard = DropGuard(outside.clone());
        let outside_file = outside.join("secret.rs");
        fs::write(&outside_file, "pub fn outside_leak() {}\n").expect("write outside file");
        let victim = dir.join("victim.rs");
        std::os::unix::fs::symlink(&outside_file, &victim).expect("create symlink");

        let candidate = CandidateFile {
            abs: victim.clone(),
            rel: "victim.rs".to_owned(),
            was_symlink: false,
        };
        let known = KnownPaths::new();
        let index = RustModuleIndex::build(&known);
        let result = process_file(&candidate, &known, &index, &CodeMapOptions::default(), &dir);
        assert!(
            result.is_err(),
            "a path that became a symlink after the walk classified it as a plain file must be \
             refused, not read: {result:?}"
        );
        drop(guard);
        drop(outside_guard);
    }

    /// Defect 7 (MEDIUM), the `.git` half: a symlink living outside `.git`
    /// but resolving inside it used to bypass the "never descend `.git`"
    /// rule entirely, reporting version-control metadata as a module.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlink_resolving_into_git_is_skipped_entirely() {
        let dir = temp_dir("symlink-into-git");
        let guard = DropGuard(dir.clone());
        write(&dir, ".git/secret.rs", "pub fn secret() {}\n");
        std::os::unix::fs::symlink(dir.join(".git/secret.rs"), dir.join("z.rs"))
            .expect("create symlink into .git");
        let map = build_code_map(&dir).expect("maps");
        assert!(
            map.modules.iter().all(|m| !m.path.contains(".git")),
            "no module may come from inside .git: {:?}",
            map.modules.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
        assert!(
            map.modules.iter().all(|m| m.path != "z.rs"),
            "a symlink resolving into .git must be skipped entirely"
        );
        let skip = map.coverage.files_skipped.iter().find(|f| f.path == "z.rs");
        assert_eq!(
            skip.map(|s| &s.reason),
            Some(&SkipReason::GitMetadata),
            "got: {:?}",
            map.coverage.files_skipped
        );
        drop(guard);
    }

    /// Defect 8 (MEDIUM), re-planted: each bound named in the report is
    /// removed in a scratch copy, and the suite is shown to fail; the tests
    /// here are the ones that catch each removal (see the report for the
    /// scratch-copy runs and exit codes). This one is the direct,
    /// injectable-deadline check for the parse timeout specifically: with
    /// `file_timeout` at zero and a file large enough that tree-sitter is
    /// expected to poll its cancellation callback at least once, the parse
    /// itself must be cancelled.
    #[test]
    fn ori_t_0036_an_expired_deadline_can_cancel_parsing_itself() {
        let dir = temp_dir("parse-timeout");
        let guard = DropGuard(dir.clone());
        let depth = 20_000;
        let mut content = String::with_capacity(depth * 8);
        for _ in 0..depth {
            content.push_str("fn a(){");
        }
        for _ in 0..depth {
            content.push('}');
        }
        content.push('\n');
        write(&dir, "big.rs", &content);
        let options = CodeMapOptions {
            file_timeout: Duration::ZERO,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert_eq!(map.coverage.files_skipped[0].reason, SkipReason::TimedOut);
        drop(guard);
    }

    /// Defect 8 (MEDIUM), the `.git` exclusion: nothing previously asserted
    /// that descending `.git` is actually refused, only that it is
    /// documented as refused.
    #[test]
    fn ori_t_0036_git_directory_contents_are_never_seen_at_all() {
        let dir = temp_dir("git-exclusion");
        let guard = DropGuard(dir.clone());
        write(&dir, ".git/hooks/pre-commit.py", "def run():\n    pass\n");
        write(&dir, "sub/.git/objects/x.rs", "pub fn x() {}\n");
        write(&dir, "visible.rs", "pub fn visible() {}\n");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(
            map.coverage.files_seen, 1,
            "only visible.rs may be seen at all: {:?}",
            map
        );
        assert!(map.modules.iter().any(|m| m.path == "visible.rs"));
        drop(guard);
    }

    /// Defect 9 (MEDIUM): the `files_seen` invariant used to be checked only
    /// on a fixture with no pre-skipped entries (`walk.pre_skipped` always
    /// empty there); this exercises several skip reasons in the same build.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_the_files_seen_invariant_holds_across_every_skip_reason_at_once() {
        let dir = temp_dir("invariant-all-reasons");
        let guard = DropGuard(dir.clone());
        write(&dir, "clean.rs", "pub fn clean() {}\n");
        write(&dir, "broken.rs", "fn broken( {\n");
        write(&dir, "notes.txt", "not source\n");
        fs::write(dir.join("weird.py"), [0u8, 159, 146, 150]).expect("write binary");
        write(&dir, "toolarge.rs", &"x".repeat(2000));
        let outside = temp_dir("invariant-outside");
        let outside_guard = DropGuard(outside.clone());
        let outside_file = outside.join("secret.rs");
        fs::write(&outside_file, "pub fn secret() {}\n").expect("write outside file");
        std::os::unix::fs::symlink(&outside_file, dir.join("escapes.rs")).expect("symlink outside");
        std::os::unix::fs::symlink(dir.join("nowhere.rs"), dir.join("dangling.rs"))
            .expect("dangling symlink");
        let pipe = dir.join("pipe.rs");
        let mkfifo_ok = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);

        let options = CodeMapOptions {
            max_file_bytes: 1000,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        let c = &map.coverage;
        assert_eq!(
            c.files_seen,
            c.files_parsed_clean + c.files_parsed_with_errors + c.files_skipped.len(),
            "the invariant must hold with every skip reason represented at once: {c:?}"
        );
        let reasons: HashSet<&SkipReason> = c.files_skipped.iter().map(|f| &f.reason).collect();
        assert!(
            reasons.len() >= 4,
            "expected several distinct skip reasons, got: {c:?}"
        );
        if mkfifo_ok {
            assert!(
                reasons
                    .iter()
                    .any(|r| matches!(r, SkipReason::NotARegularFile))
            );
        }
        drop(guard);
        drop(outside_guard);
    }

    /// Defect 10 (LOW), M6: the existing binary test's fixture bytes are
    /// also invalid UTF-8, so deleting the NUL check alone still leaves the
    /// file classified `Binary` by the separate UTF-8 check, and that
    /// mutant survives undetected. A NUL byte is valid UTF-8 (U+0000), so
    /// this isolates the NUL check specifically.
    #[test]
    fn ori_t_0036_a_nul_byte_in_otherwise_valid_utf8_is_binary() {
        let dir = temp_dir("nul-byte");
        let guard = DropGuard(dir.clone());
        let mut bytes = b"pub fn ok() {}\n".to_vec();
        bytes.push(0u8);
        bytes.extend_from_slice(b"trailing\n");
        assert!(
            std::str::from_utf8(&bytes).is_ok(),
            "the fixture must be valid UTF-8 despite the embedded NUL, or this does not isolate \
             the NUL check"
        );
        fs::write(dir.join("has_nul.rs"), &bytes).expect("write");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.files_skipped.len(), 1);
        assert_eq!(map.coverage.files_skipped[0].reason, SkipReason::Binary);
        drop(guard);
    }

    /// Defect 10 (LOW), M8: nothing previously asserted that a non-`pub`
    /// Rust item is excluded from interfaces.
    #[test]
    fn ori_t_0036_a_private_rust_item_is_not_an_interface() {
        let dir = temp_dir("rust-private");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "m.rs",
            "fn private_helper() -> i32 {\n    1\n}\n\npub fn public_one() -> i32 {\n    2\n}\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let m = map
            .modules
            .iter()
            .find(|mm| mm.path == "m.rs")
            .expect("present");
        assert!(m.interfaces.iter().any(|i| i.name == "public_one"));
        assert!(
            m.interfaces.iter().all(|i| i.name != "private_helper"),
            "a function with no pub must not be an interface: {:?}",
            m.interfaces
        );
        drop(guard);
    }

    /// Defect 10 (LOW), M11: nothing previously asserted that an
    /// underscore-prefixed Python function is excluded from interfaces.
    #[test]
    fn ori_t_0036_a_python_underscore_prefixed_function_is_not_an_interface() {
        let dir = temp_dir("py-private");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "m.py",
            "def _private_helper():\n    pass\n\n\ndef public_one():\n    pass\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let m = map
            .modules
            .iter()
            .find(|mm| mm.path == "m.py")
            .expect("present");
        assert!(m.interfaces.iter().any(|i| i.name == "public_one"));
        assert!(
            m.interfaces.iter().all(|i| i.name != "_private_helper"),
            "an underscore-prefixed function must not be an interface: {:?}",
            m.interfaces
        );
        drop(guard);
    }

    /// Defect 10 (LOW), M12: nothing previously asserted that
    /// `#[test_case(...)]` (last path segment `test_case`, not `test`) is
    /// excluded from covering tests.
    #[test]
    fn ori_t_0036_an_attribute_whose_last_segment_is_not_test_is_not_a_covering_test() {
        let dir = temp_dir("test-case-attr");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "m.rs",
            "#[test_case(1)]\nfn not_really_a_test(x: i32) {\n    let _ = x;\n}\n\n#[test]\nfn really_a_test() {\n    assert!(true);\n}\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let m = map
            .modules
            .iter()
            .find(|mm| mm.path == "m.rs")
            .expect("present");
        assert!(m.covering_tests.contains(&"really_a_test".to_owned()));
        assert!(
            !m.covering_tests.contains(&"not_really_a_test".to_owned()),
            "#[test_case(...)] must not be recognized as #[test]: {:?}",
            m.covering_tests
        );
        drop(guard);
    }

    /// Defect 10 (LOW), M9/M13/M14: the main fixture happens to already
    /// emit every list in sorted order, so deleting the sort calls entirely
    /// still left every assertion in
    /// `tests::ori_t_0036_per_module_lists_are_sorted_deterministically` passing,
    /// because "compare to a freshly re-sorted clone" is tautological when
    /// the input was already sorted. This fixture is built so natural
    /// (pre-sort) order differs from sorted order in all three of the
    /// fields the outer sort touches, and the assertions compare to an
    /// exact, hard-coded expected sequence rather than to a re-sorted clone
    /// of the same data.
    #[test]
    fn ori_t_0036_sorting_is_not_a_coincidence_of_fixture_order() {
        let dir = temp_dir("sortedness");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "unsorted.rs",
            "use zzz_pkg::Thing;\nuse aaa_pkg::Thing;\nuse mmm_pkg::Thing;\n",
        );
        write(&dir, "mm.py", "def test_zzz():\n    pass\n");
        write(&dir, "test_mm.py", "def test_local():\n    pass\n");
        write(
            &dir,
            "guarded.py",
            "#!/usr/bin/env python3\nimport os\n\n\nif __name__ == \"__main__\":\n    pass\n",
        );
        let map = build_code_map(&dir).expect("maps");

        let unsorted_rs = map
            .modules
            .iter()
            .find(|m| m.path == "unsorted.rs")
            .expect("present");
        let edge_targets: Vec<&str> = unsorted_rs
            .dependency_edges
            .iter()
            .map(|e| e.to.as_str())
            .collect();
        assert_eq!(
            edge_targets,
            vec!["aaa_pkg::Thing", "mmm_pkg::Thing", "zzz_pkg::Thing"],
            "dependency edges must be sorted by (external, to), not source order"
        );

        let mm = map
            .modules
            .iter()
            .find(|m| m.path == "mm.py")
            .expect("present");
        assert_eq!(
            mm.covering_tests,
            vec!["test_mm.py".to_owned(), "test_zzz".to_owned()],
            "covering tests must be sorted after the sibling-file convention adds to them"
        );

        let guarded = map
            .modules
            .iter()
            .find(|m| m.path == "guarded.py")
            .expect("present");
        let entry_lines: Vec<usize> = guarded.entry_points.iter().map(|e| e.line).collect();
        assert_eq!(entry_lines.len(), 2, "{:?}", guarded.entry_points);
        assert_eq!(
            entry_lines[0], 1,
            "the shebang entry point (line 1) must sort before the __name__ guard entry point, \
             even though it is discovered second: {:?}",
            guarded.entry_points
        );
        assert!(entry_lines[1] > 1);

        drop(guard);
    }

    /// Item 1 (HIGH): the spec-citation scan used to clone the active
    /// heading twice per matching line before ever checking the dedup key,
    /// which is O(heading length) work repeated per matching line, entirely
    /// outside any deadline. A long heading followed by many matching lines
    /// must now stay fast.
    #[test]
    fn ori_t_0036_the_spec_citation_scan_stays_fast_on_a_pathological_document() {
        let dir = temp_dir("spec-scan-fast");
        let guard = DropGuard(dir.clone());
        write(&dir, "a.rs", "pub fn a() {}\n");
        let mut doc = String::from("# ");
        doc.push_str(&"H".repeat(256 * 1024));
        doc.push('\n');
        for _ in 0..8000 {
            doc.push_str("a.rs\n");
        }
        // Assembled, not written whole: see the comment on the equivalent
        // line in `ori_t_0036_spec_citations_are_deduplicated_not_one_allocation_per_matching_line`.
        write(&dir, &format!("spec/{}.md", "big"), &doc);

        let start = Instant::now();
        let map = build_code_map(&dir).expect("maps");
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(5),
            "the spec-citation scan over one long heading and 8000 matching lines took \
             {elapsed:?}; the pre-fix cost was quadratic in heading length"
        );
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Complete);
        let a = map
            .modules
            .iter()
            .find(|m| m.path == "a.rs")
            .expect("present");
        assert_eq!(a.spec_sections.len(), 1);
        drop(guard);
    }

    /// Item 1 (HIGH), the deadline itself: `attach_spec_citations` never
    /// received `options` far enough to check one before this fix. An
    /// already-past deadline must stop the scan and be recorded as
    /// incomplete, tested directly since the function is private and this
    /// is the same pattern used for the per-file extraction deadline above.
    #[test]
    fn ori_t_0036_an_expired_deadline_stops_the_spec_citation_scan_and_is_recorded() {
        let dir = temp_dir("spec-scan-timeout");
        let guard = DropGuard(dir.clone());
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(&dir, &format!("spec/{}.md", "x"), "# Heading\n\na.rs\n");
        let options = CodeMapOptions {
            file_timeout: Duration::ZERO,
            ..CodeMapOptions::default()
        };
        let mut modules = vec![Module {
            path: "a.rs".to_owned(),
            language: Language::Rust,
            parsed_with_errors: false,
            interfaces: Vec::new(),
            dependency_edges: Vec::new(),
            dependency_edges_truncated: false,
            entry_points: Vec::new(),
            covering_tests: Vec::new(),
            spec_sections: Vec::new(),
            spec_sections_truncated: false,
        }];
        let already_past = Instant::now() - Duration::from_secs(1);
        let output = attach_spec_citations(&dir, &mut modules, &options, already_past);
        assert_eq!(
            output.status,
            SpecScan::TimedOut,
            "a deadline already past must stop the scan and report it timed out"
        );
        drop(guard);
    }

    /// Item 2 (HIGH): `resolve_rust_crate_path`'s per-prefix rebuild-and-rehash
    /// was O(k^2) in the number of `::`-separated segments in one
    /// `use crate::` path, entirely outside the whole-file deadline (nothing
    /// inside the old function checked one). `RustModuleIndex::longest_prefix`
    /// replaces it with a single O(k) walk. A path with tens of thousands of
    /// segments, which the pre-fix code took double-digit seconds or more to
    /// resolve at this depth (this ticket's report cites 8 to 18 seconds at
    /// 80,000 segments), must now resolve quickly.
    #[test]
    fn ori_t_0036_use_crate_path_resolution_is_linear_not_quadratic() {
        let dir = temp_dir("use-crate-quadratic");
        let guard = DropGuard(dir.clone());
        write(&dir, "src/lib.rs", "pub fn f() {}\n");
        let depth = 40_000;
        let mut content = String::from("use crate::");
        for _ in 0..depth {
            content.push_str("a::");
        }
        content.push_str("b;\npub fn f2() {}\n");
        write(&dir, "src/big.rs", &content);

        let start = Instant::now();
        let map = build_code_map(&dir).expect("maps");
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(5),
            "resolving a {depth}-segment use crate:: path took {elapsed:?}; the O(k^2) \
             regression took double-digit seconds at half this depth"
        );
        let big = map
            .modules
            .iter()
            .find(|m| m.path == "src/big.rs")
            .expect("present");
        assert!(!big.parsed_with_errors);
        drop(guard);
    }

    /// Item 2 (HIGH), the index directly: a chain where every prefix along
    /// the walk actually resolves (not the fast-reject case the test above
    /// happens to hit first), proving the O(k) claim on the case that
    /// matters most: a long walk that does real work at every step, not one
    /// that stops after the first mismatch.
    #[test]
    fn ori_t_0036_rust_module_index_longest_prefix_is_linear_on_a_long_matching_chain() {
        let depth = 50_000;
        let mut path = String::from("src");
        for _ in 0..depth {
            path.push_str("/a");
        }
        path.push_str(".rs");
        let known = known_paths(&[path.as_str(), "src/lib.rs"]);
        let index = RustModuleIndex::build(&known);

        let segments: Vec<String> = (0..depth).map(|_| "a".to_owned()).collect();
        let segment_refs: Vec<&str> = segments.iter().map(String::as_str).collect();

        let start = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(30);
        let resolved = index.longest_prefix("src", segment_refs.into_iter(), deadline);
        let elapsed = start.elapsed();

        assert_eq!(resolved.as_deref(), Some(path.as_str()));
        assert!(
            elapsed < Duration::from_secs(2),
            "a {depth}-segment fully-matching walk took {elapsed:?}, expected linear time"
        );
    }

    /// Item 2 (HIGH), the internal deadline check directly, not by timing: on
    /// this chain, only the single leaf node at the very end of the walk is
    /// marked resolved, every ancestor along the way is not, so a walk that
    /// stops early (deadline already past) can only return `None`, never the
    /// correct answer. This is a functional, non-flaky way to prove
    /// `longest_prefix`'s own periodic deadline check actually stops the
    /// walk, independent of how fast the trie itself is at any given depth.
    #[test]
    fn ori_t_0036_rust_module_index_longest_prefix_respects_an_expired_deadline() {
        let depth = 10_000;
        let mut path = String::from("src");
        for _ in 0..depth {
            path.push_str("/a");
        }
        path.push_str(".rs");
        let known = known_paths(&[path.as_str(), "src/lib.rs"]);
        let index = RustModuleIndex::build(&known);

        let segments: Vec<String> = (0..depth).map(|_| "a".to_owned()).collect();
        let segment_refs: Vec<&str> = segments.iter().map(String::as_str).collect();

        let already_past = Instant::now() - Duration::from_secs(1);
        let resolved = index.longest_prefix("src", segment_refs.into_iter(), already_past);
        assert_ne!(
            resolved.as_deref(),
            Some(path.as_str()),
            "an already-expired deadline must stop the walk before it reaches the one node \
             (at the very end of this chain) that would resolve correctly"
        );
    }

    /// Item 4 (mutant survival): removing the `interfaces` sort passes every
    /// existing test because every existing fixture's pub items are already
    /// in ascending line order (tree-sitter visits source in document
    /// order), which is the only order the sort's primary key could ever
    /// disagree with; two items on the *same* line, where only the name
    /// tiebreak can put them back in order, is what actually exercises it.
    #[test]
    fn ori_t_0036_interfaces_are_sorted_by_line_then_name_not_discovery_order() {
        let dir = temp_dir("interfaces-sort");
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub struct Zebra; pub struct Apple;\n");
        let map = build_code_map(&dir).expect("maps");
        let m = map
            .modules
            .iter()
            .find(|mm| mm.path == "m.rs")
            .expect("present");
        let names: Vec<&str> = m.interfaces.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Apple", "Zebra"],
            "same-line interfaces must be name-sorted as a tiebreak, not left in source order: \
             {:?}",
            m.interfaces
        );
        drop(guard);
    }

    /// Item 4 (mutant survival): removing the `spec_sections` sort passes
    /// every existing test for the same shape of reason; two headings in one
    /// document, cited in an order that disagrees with their alphabetical
    /// order, is what actually exercises it.
    #[test]
    fn ori_t_0036_spec_sections_are_sorted_by_doc_then_heading_not_discovery_order() {
        let dir = temp_dir("specsections-sort");
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub fn m() {}\n");
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(
            &dir,
            &format!("spec/{}.md", "x"),
            "# Zeta section\n\nm.rs\n\n# Alpha section\n\nm.rs\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let m = map
            .modules
            .iter()
            .find(|mm| mm.path == "m.rs")
            .expect("present");
        let headings: Vec<Option<&str>> = m
            .spec_sections
            .iter()
            .map(|c| map.spec_heading(c).map(|h| h.text.as_str()))
            .collect();
        assert_eq!(
            headings,
            vec![Some("Alpha section"), Some("Zeta section")],
            "citations under the same doc must be heading-sorted, not left in the order the \
             headings were encountered: {:?}",
            m.spec_sections
        );
        drop(guard);
    }

    /// Item 4 (mutant survival): the combined invariant test added in round
    /// 2 does not include a `GitMetadata` skip or an unreadable subdirectory
    /// in the same run, so a mutant deleting either recording path at
    /// `walk_repository` survives it.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_the_files_seen_invariant_holds_with_git_metadata_and_unreadable_subdirectory_too()
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("invariant-git-and-unreadable");
        let guard = DropGuard(dir.clone());
        write(&dir, "clean.rs", "pub fn clean() {}\n");
        write(&dir, ".git/secret.rs", "pub fn secret() {}\n");
        std::os::unix::fs::symlink(dir.join(".git/secret.rs"), dir.join("z.rs"))
            .expect("create symlink into .git");
        let hidden_dir = dir.join("hidden");
        fs::create_dir_all(&hidden_dir).expect("create hidden dir");
        fs::write(hidden_dir.join("inner.rs"), "pub fn inner() {}\n").expect("write file");
        let mut perms = fs::metadata(&hidden_dir).expect("stat").permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&hidden_dir, perms).expect("chmod");
        let still_readable = fs::read_dir(&hidden_dir).is_ok();

        let map = build_code_map(&dir);

        let mut restore = fs::metadata(&hidden_dir).expect("stat").permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(&hidden_dir, restore);

        let map = map.expect("maps despite the unreadable subdirectory and the git symlink");
        let c = &map.coverage;
        assert_eq!(
            c.files_seen,
            c.files_parsed_clean + c.files_parsed_with_errors + c.files_skipped.len(),
            "the invariant must hold with a GitMetadata skip and an unreadable-subdirectory \
             skip both present: {c:?}"
        );
        if still_readable {
            eprintln!(
                "running with elevated privileges; chmod 000 did not restrict access, the \
                 unreadable-subdirectory half of this test did not exercise anything"
            );
        } else {
            assert!(
                c.files_skipped
                    .iter()
                    .any(|f| f.path == "hidden" && matches!(f.reason, SkipReason::Unreadable(_))),
                "expected the unreadable subdirectory recorded: {c:?}"
            );
        }
        assert!(
            c.files_skipped
                .iter()
                .any(|f| f.reason == SkipReason::GitMetadata),
            "expected a GitMetadata skip: {c:?}"
        );
        drop(guard);
    }

    /// Item 5 (LOW) of round 3, rule replaced in round 4: round 3 pinned the
    /// grouped form as one external edge carrying the grouped text, never
    /// resolved. Round 4's ruling is that a grouped import is either
    /// resolved member by member or honestly absent, and this module now
    /// resolves each member, so this test (the ticket's own, never on
    /// `main`) asserts the new rule on the same fixture: `use
    /// crate::{foo::Foo, bar::Bar};` gives one resolved edge per member, the
    /// same two the ungrouped form gives, and no edge carrying grouped text.
    #[test]
    fn ori_t_0036_a_grouped_use_crate_import_resolves_each_member_it_names() {
        let dir = temp_dir("grouped-use");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "src/lib.rs",
            "pub mod foo;\npub mod bar;\nuse crate::{foo::Foo, bar::Bar};\n\npub fn f() {}\n",
        );
        write(&dir, "src/foo.rs", "pub struct Foo;\n");
        write(&dir, "src/bar.rs", "pub struct Bar;\n");
        let map = build_code_map(&dir).expect("maps");
        let lib = map
            .modules
            .iter()
            .find(|m| m.path == "src/lib.rs")
            .expect("present");
        let edges: Vec<(&str, bool)> = lib
            .dependency_edges
            .iter()
            .map(|e| (e.to.as_str(), e.external))
            .collect();
        // Two from `mod foo; mod bar;`, two from the grouped `use`.
        assert_eq!(
            edges,
            vec![
                ("src/bar.rs", false),
                ("src/bar.rs", false),
                ("src/foo.rs", false),
                ("src/foo.rs", false),
            ],
            "each member of the group must resolve on its own, and nothing may carry the \
             grouped text"
        );
        drop(guard);
    }

    /// Item 6 (LOW): a symlink to a directory used to be never descended
    /// into (correctly) but also never recorded at all, so it vanished from
    /// `Coverage` exactly like the "silently dropped" defect class this
    /// module otherwise refuses.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlinked_directory_is_recorded_not_silently_absent() {
        let dir = temp_dir("symlinked-dir-recorded");
        let guard = DropGuard(dir.clone());
        write(&dir, "real.rs", "pub fn real_fn() {}\n");
        let target_dir = temp_dir("symlinked-dir-target");
        let target_guard = DropGuard(target_dir.clone());
        write(&target_dir, "inside.rs", "pub fn inside() {}\n");
        std::os::unix::fs::symlink(&target_dir, dir.join("linked_dir"))
            .expect("create symlink to directory");

        let map = build_code_map(&dir).expect("maps");
        assert!(
            map.modules
                .iter()
                .all(|m| !m.path.starts_with("linked_dir")),
            "a symlinked directory must never be descended into: {:?}",
            map.modules.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
        let skip = map
            .coverage
            .files_skipped
            .iter()
            .find(|f| f.path == "linked_dir");
        assert_eq!(
            skip.map(|s| &s.reason),
            Some(&SkipReason::SymlinkedDirectory),
            "a symlink to a directory must be recorded, not silently absent from coverage: {:?}",
            map.coverage.files_skipped
        );
        drop(guard);
        drop(target_guard);
    }

    /// Item 7 (LOW): the shared per-file deadline, checked directly against
    /// the formula rather than by timing a real multi-second debug-build
    /// run: a small file gets no allowance beyond `file_timeout`, and a
    /// file over the threshold gets a meaningfully larger budget.
    #[test]
    fn ori_t_0036_per_file_budget_scales_with_size_above_the_threshold_not_below() {
        let options = CodeMapOptions {
            file_timeout: Duration::from_secs(5),
            ..CodeMapOptions::default()
        };
        assert_eq!(
            per_file_budget(&options, 1000),
            Duration::from_secs(5),
            "a small file gets exactly file_timeout, no allowance added"
        );
        assert_eq!(
            per_file_budget(&options, 256 * 1024),
            Duration::from_secs(5),
            "exactly at the threshold, still no allowance"
        );
        let big = per_file_budget(&options, 8 * 1024 * 1024);
        assert!(
            big > Duration::from_secs(5) + Duration::from_secs(7),
            "an 8 MiB file must get a meaningfully larger budget than the base 5s, so a debug \
             build's slower parse and extraction is not mistaken for a hang; got {big:?}"
        );
    }

    // -----------------------------------------------------------------
    // ORI-T-0036, round 4 (2026-09-24), from the review of round 3. One test (at
    // least) per required item, each failing before its fix and passing
    // after; the report's plant table says which test catches which plant.
    // -----------------------------------------------------------------

    /// Creates a FIFO at `path` with the system `mkfifo`, the way the
    /// round-2 FIFO tests above do. `false` when this machine has no
    /// `mkfifo` (the caller then skips, loudly).
    #[cfg(unix)]
    fn make_fifo(path: &Path) -> bool {
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Runs `work` on its own thread and waits at most `limit` for it, so a
    /// regression that blocks in `open(2)` fails the test in bounded time
    /// instead of hanging the whole suite. A thread left blocked by such a
    /// regression is torn down when the test binary exits.
    fn within<T: Send + 'static>(limit: Duration, work: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(work());
        });
        rx.recv_timeout(limit).unwrap_or_else(|_| {
            panic!(
                "did not return within {limit:?}: something blocked (an open or a read of a \
                 FIFO, most likely)"
            )
        })
    }

    /// Item 1 (HIGH), the symlink branch directly: an entry the walk
    /// classified as a symlink to an in-root regular file, whose target is a
    /// FIFO by the time `process_file` opens it (the state the reviewers'
    /// swap converges to, reached here without a race). Round 3 opened this
    /// branch with a blocking, link-following `fs::File::open` and checked
    /// the type only afterward, so this hung forever.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_process_file_never_blocks_on_a_symlink_entry_whose_target_became_a_fifo() {
        let dir = temp_dir("fifo-symlink-branch");
        let guard = DropGuard(dir.clone());
        let fifo = dir.join("t.rs");
        if !make_fifo(&fifo) {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            return;
        }
        std::os::unix::fs::symlink(&fifo, dir.join("z.rs")).expect("create symlink to fifo");
        let candidate = CandidateFile {
            abs: dir.join("z.rs"),
            rel: "z.rs".to_owned(),
            was_symlink: true,
        };
        let root = fs::canonicalize(&dir).expect("canonicalize root");
        let result = within(Duration::from_secs(10), move || {
            let known = KnownPaths::new();
            let index = RustModuleIndex::build(&known);
            process_file(
                &candidate,
                &known,
                &index,
                &CodeMapOptions::default(),
                &root,
            )
        });
        assert_eq!(result, Err(SkipReason::NotARegularFile));
        drop(guard);
    }

    /// Item 1 (HIGH), the plain branch directly: an entry the walk
    /// classified as a plain regular file that is a FIFO by the time
    /// `process_file` opens it. `O_NONBLOCK` on this open is what keeps it
    /// from waiting for a writer.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_process_file_never_blocks_on_a_plain_entry_that_became_a_fifo() {
        let dir = temp_dir("fifo-plain-branch");
        let guard = DropGuard(dir.clone());
        let fifo = dir.join("t.rs");
        if !make_fifo(&fifo) {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            return;
        }
        let candidate = CandidateFile {
            abs: fifo,
            rel: "t.rs".to_owned(),
            was_symlink: false,
        };
        let root = fs::canonicalize(&dir).expect("canonicalize root");
        let result = within(Duration::from_secs(10), move || {
            let known = KnownPaths::new();
            let index = RustModuleIndex::build(&known);
            process_file(
                &candidate,
                &known,
                &index,
                &CodeMapOptions::default(),
                &root,
            )
        });
        assert_eq!(result, Err(SkipReason::NotARegularFile));
        drop(guard);
    }

    /// Item 1 (HIGH), the `spec/` open directly: the scan given a `spec/`
    /// directory holding a FIFO named `*.md` next to a real document must
    /// return, keep the real document's citation, and never read the FIFO.
    /// The round-2 test `tests::ori_t_0036_a_fifo_under_spec_is_skipped_not_a_hang`
    /// covers the same open end to end. Rewritten in round 5, when listing
    /// documents and reading them became two steps (`list_markdown`, then
    /// `read_markdown` per document, one at a time): it used to call the
    /// round-4 `collect_markdown`, which did both and held every document.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_reading_spec_documents_never_blocks_on_a_fifo_document() {
        let dir = temp_dir("fifo-spec-direct");
        let guard = DropGuard(dir.clone());
        let spec = dir.join("spec");
        fs::create_dir_all(&spec).expect("create spec dir");
        fs::write(spec.join("a.md"), "# H\n\nsrc/a.rs\n").expect("write doc");
        if !make_fifo(&spec.join("b.md")) {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            return;
        }
        let fifo = spec.join("b.md");
        let fifo_read = within(Duration::from_secs(10), move || read_markdown(&fifo, 1024));
        assert_eq!(
            fifo_read,
            Err(SkipReason::NotARegularFile),
            "a FIFO must never be read as a document"
        );

        let root = dir.clone();
        let (output, modules) = within(Duration::from_secs(10), move || {
            let mut modules = vec![bare_module("src/a.rs")];
            let output = attach_spec_citations(
                &root,
                &mut modules,
                &CodeMapOptions::default(),
                Instant::now() + Duration::from_secs(60),
            );
            (output, modules)
        });
        // Round 5 fix: a listed document the scan could not read makes the
        // scan partial, with the document and its reason recorded.
        assert_eq!(output.status, SpecScan::Partial);
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        assert_eq!(
            output.skipped,
            vec![SkippedFile {
                path: format!("spec/{}.md", "b"),
                reason: SkipReason::NotARegularFile,
            }]
        );
        assert_eq!(
            output.docs,
            vec![format!("spec/{}.md", "a")],
            "only the regular document"
        );
        assert_eq!(modules[0].spec_sections.len(), 1);
        drop(guard);
    }

    /// Item 1 (HIGH), end to end, the swap the review of round 3 used: a
    /// slow file sorted first (`a_slow.rs`), a plain `t.rs`, and `z.rs`, a
    /// symlink to `t.rs`. After the walk has classified all three and while
    /// `a_slow.rs` is still being parsed, `t.rs` is atomically replaced by a
    /// FIFO (created elsewhere, then renamed over it) at a fixed delay. Both
    /// later opens then meet a FIFO: `t.rs` through the plain branch, `z.rs`
    /// through the symlink branch, which is the one round 3 left blocking
    /// (the review reproduced a hang there three times out of three).
    ///
    /// The delay (250 ms after the mapping thread starts) is far longer than
    /// the walk of three files and far shorter than parsing `a_slow.rs`
    /// (about 1.8 s in this ticket's debug build). If a much faster machine
    /// ever parses it before the swap lands, the assertions below fail
    /// saying so, rather than passing without having exercised anything.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_fifo_swapped_in_after_the_walk_never_hangs_the_map() {
        let dir = temp_dir("fifo-swap-race");
        let guard = DropGuard(dir.clone());
        let staging = temp_dir("fifo-swap-race-staging");
        let staging_guard = DropGuard(staging.clone());
        let staged_fifo = staging.join("fifo");
        if !make_fifo(&staged_fifo) {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            drop(staging_guard);
            return;
        }
        let mut slow = String::with_capacity(150_000 * 20);
        for i in 0..150_000 {
            slow.push_str(&format!("pub fn f{i:06}() {{}}\n"));
        }
        write(&dir, "a_slow.rs", &slow);
        write(&dir, "t.rs", "pub fn t() {}\n");
        std::os::unix::fs::symlink(dir.join("t.rs"), dir.join("z.rs")).expect("create symlink");

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let root = dir.clone();
        std::thread::spawn(move || {
            let _ = started_tx.send(());
            let _ = done_tx.send(build_code_map(&root));
        });
        started_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the mapping thread starts");
        std::thread::sleep(Duration::from_millis(250));
        fs::rename(&staged_fifo, dir.join("t.rs")).expect("swap the FIFO in over t.rs");

        let map = done_rx
            .recv_timeout(Duration::from_secs(60))
            .expect(
                "build_code_map must return within 60s; a FIFO swapped in after the walk must \
                 never block an open",
            )
            .expect("maps");
        let reason_of = |path: &str| {
            map.coverage
                .files_skipped
                .iter()
                .find(|f| f.path == path)
                .map(|f| f.reason.clone())
        };
        assert_eq!(
            reason_of("t.rs"),
            Some(SkipReason::NotARegularFile),
            "t.rs must be refused as the FIFO it became; if it mapped instead, the swap landed \
             after it was read and this run exercised nothing: {:?}",
            map.coverage
        );
        assert_eq!(
            reason_of("z.rs"),
            Some(SkipReason::NotARegularFile),
            "z.rs, the symlink whose target became a FIFO, must be refused: {:?}",
            map.coverage
        );
        drop(guard);
        drop(staging_guard);
    }

    /// Item 1 (HIGH), an assumption the "never hangs" claim rests on: the
    /// walk lists directories with `fs::read_dir`, and a directory swapped
    /// for a FIFO after the walk queued it is then listed through a path
    /// that names a FIFO. `read_dir` (`opendir` on unix) must fail at once
    /// on it, not wait for a writer.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_listing_a_directory_that_became_a_fifo_fails_rather_than_blocks() {
        let dir = temp_dir("fifo-read-dir");
        let guard = DropGuard(dir.clone());
        let fifo = dir.join("was_a_dir");
        if !make_fifo(&fifo) {
            eprintln!("mkfifo not available on this machine; skipping");
            drop(guard);
            return;
        }
        let listed = within(Duration::from_secs(10), move || fs::read_dir(&fifo).is_ok());
        assert!(!listed, "a FIFO must not list as a directory");
        drop(guard);
    }

    /// A bare [`Module`] at `path`, for tests that drive the spec scan
    /// directly rather than through a whole build.
    fn bare_module(path: &str) -> Module {
        Module {
            path: path.to_owned(),
            language: Language::Rust,
            parsed_with_errors: false,
            interfaces: Vec::new(),
            dependency_edges: Vec::new(),
            dependency_edges_truncated: false,
            entry_points: Vec::new(),
            covering_tests: Vec::new(),
            spec_sections: Vec::new(),
            spec_sections_truncated: false,
        }
    }

    /// Item 2 (HIGH), the check itself, deterministically: four lines of
    /// just over 1 MiB each is far fewer than the 4096 lines the round-3
    /// scan waited for before its first check, so with an already-expired
    /// deadline that scan read the clock zero times and reported itself
    /// complete. Charged by bytes, the scan must check after the first
    /// line and stop.
    #[test]
    fn ori_t_0036_the_spec_scan_checks_its_deadline_by_bytes_scanned_not_only_by_lines() {
        // Round 5: driven through `CitationScan::scan_document`, the
        // per-document scan that replaced round 4's all-documents-at-once
        // `scan_spec_citations`; the check itself is the same code.
        let long_line = "x".repeat(1024 * 1024 + 1);
        let content = format!("{long_line}\n{long_line}\n{long_line}\n{long_line}\n");
        let rel = format!("spec/{}.md", "long");
        let mut modules = vec![bare_module("a.rs")];
        let already_past = Instant::now() - Duration::from_secs(1);
        assert!(
            !CitationScan::new(1).scan_document(&rel, &content, &mut modules, already_past),
            "four 1 MiB lines past an expired deadline must stop the scan, not complete it"
        );

        // The line count still bounds the ordinary shape, many short lines.
        let short_lines = "a.rs\n".repeat(5000);
        let mut modules = vec![bare_module("a.rs")];
        assert!(
            !CitationScan::new(1).scan_document(&rel, &short_lines, &mut modules, already_past),
            "5000 short lines past an expired deadline must stop the scan too"
        );

        // And a scan with time left completes.
        let mut modules = vec![bare_module("a.rs")];
        assert!(CitationScan::new(1).scan_document(
            &rel,
            &content,
            &mut modules,
            Instant::now() + Duration::from_secs(60)
        ));
    }

    /// Item 2 (HIGH), end to end, the long-line document the review of round
    /// 3 built, scaled down to fit a unit test: 192 modules named
    /// `("as" x 20) + "aNNNN.rs"` and one `spec/` document, a single line of
    /// `"as"` repeated to just under the 8 MiB cap, so every search is slow
    /// and there are only 192 line searches in all (the round-3 scan's first
    /// check came at the 4096th, so it never checked). The round-3 scan ran
    /// every search, about 8 s in this ticket's debug build (2 s in
    /// release), and reported itself complete; with a 250 ms budget the scan
    /// must stop and say so. One document, not several (round 4 used three
    /// of 64 modules each): since round 5 the scan also reads the clock
    /// before each document, which would cap a lines-only check's overrun
    /// at one document's worth of searches and hide it here; with all the
    /// work in one document, only the check inside the scan can stop it.
    #[test]
    fn ori_t_0036_a_spec_document_of_long_lines_cannot_overrun_the_scan_deadline() {
        let dir = temp_dir("spec-long-lines");
        let guard = DropGuard(dir.clone());
        let prefix = "as".repeat(20);
        for n in 0..192 {
            write(&dir, &format!("{prefix}a{n:04}.rs"), "pub fn f() {}\n");
        }
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(
            &dir,
            &format!("spec/{}.md", "long"),
            &"as".repeat(4_194_300),
        );
        let options = CodeMapOptions {
            file_timeout: Duration::from_millis(250),
            ..CodeMapOptions::default()
        };
        let start = Instant::now();
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        let elapsed = start.elapsed();
        assert!(
            map.modules.len() >= 96,
            "the modules themselves must map, or there is nothing to scan: {:?}",
            map.coverage
        );
        assert!(
            map.coverage.spec_citation_scan == SpecScan::TimedOut,
            "192 slow 8 MiB line searches cannot finish in 250 ms; the scan must stop at its \
             deadline and say so (took {elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_secs(4),
            "the whole map must come back near the scan's 250 ms budget, not after every search; \
             took {elapsed:?}"
        );
        drop(guard);
    }

    /// `from <dots> import a, a, ..., a`, the statement shape the review of
    /// round 3 used against `python_import_from_edges`.
    fn python_relative_import(dots: usize, names: usize) -> String {
        let mut content = String::with_capacity(dots + 3 * names + 16);
        content.push_str("from ");
        content.push_str(&".".repeat(dots));
        content.push_str(" import ");
        for i in 0..names {
            if i > 0 {
                content.push_str(", ");
            }
            content.push('a');
        }
        content.push('\n');
        content
    }

    /// The first `import_from_statement` in `source`'s Python parse tree.
    fn first_import_from(tree: &tree_sitter::Tree) -> Node<'_> {
        let root = tree.root_node();
        let mut cursor = root.walk();
        root.children(&mut cursor)
            .find(|child| child.kind() == "import_from_statement")
            .expect("an import_from_statement")
    }

    /// Item 3 (HIGH), the deadline inside the loop: a statement's names are
    /// iterated with a check before each one, so an expired deadline stops
    /// the loop at once and says so. The round-3 loop had no check at all:
    /// it would have produced every edge and reported itself complete.
    #[test]
    fn ori_t_0036_python_import_from_checks_the_deadline_inside_the_names_loop() {
        let source = python_relative_import(2, 1000);
        let tree = parse_bounded(
            Language::Python,
            false,
            &source,
            Instant::now() + Duration::from_secs(60),
        )
        .expect("parses");
        let statement = first_import_from(&tree);
        let already_past = Instant::now() - Duration::from_secs(1);
        let mut sink = EdgeSink::new();
        let completed = python_import_from_edges(
            statement,
            source.as_bytes(),
            "pkg/m.py",
            &KnownPaths::new(),
            &mut sink,
            already_past,
        );
        assert!(!completed, "an expired deadline must stop the names loop");
        assert!(
            sink.edges.is_empty(),
            "nothing is produced after the deadline: {} edges",
            sink.edges.len()
        );

        let mut sink = EdgeSink::new();
        let completed = python_import_from_edges(
            statement,
            source.as_bytes(),
            "pkg/m.py",
            &KnownPaths::new(),
            &mut sink,
            Instant::now() + Duration::from_secs(60),
        );
        assert!(completed);
        assert_eq!(
            sink.edges.len(),
            1000,
            "with time left, every name is an edge"
        );
    }

    /// Item 3 (HIGH), the memory form, end to end: `K` dots and `M` names
    /// that do not resolve. The round-3 code built each unresolved edge's
    /// text as `K` dots plus the name, so memory was `K x M` (the review
    /// measured 678 MB from a 169 KB file); here it is 40 MB with the
    /// round-3 code and a few hundred KB now. Every edge's text must stay
    /// short, whatever `K` is, and still say how many dots there were.
    #[test]
    fn ori_t_0036_python_relative_import_edges_do_not_grow_with_the_dot_count() {
        let dir = temp_dir("py-dots-memory");
        let guard = DropGuard(dir.clone());
        let dots = 20_000;
        let mut content = String::from("from ");
        content.push_str(&".".repeat(dots));
        content.push_str(" import ");
        let names: Vec<String> = (0..2000).map(|i| format!("n{i}")).collect();
        content.push_str(&names.join(", "));
        content.push('\n');
        write(&dir, "pkg/m.py", &content);
        let map = build_code_map(&dir).expect("maps");
        let m = map
            .modules
            .iter()
            .find(|m| m.path == "pkg/m.py")
            .expect("present");
        assert_eq!(m.dependency_edges.len(), 2000, "one edge per name");
        let longest = m
            .dependency_edges
            .iter()
            .map(|e| e.to.len())
            .max()
            .unwrap_or(0);
        assert!(
            longest <= 64,
            "an edge's text must not grow with the dot count ({dots} dots); longest is \
             {longest} bytes"
        );
        assert!(
            m.dependency_edges
                .iter()
                .all(|e| e.external && e.to.starts_with("[20000 leading dots]")),
            "an import climbing past the root is unresolved, and its count is still stated: {:?}",
            &m.dependency_edges[..3]
        );
        drop(guard);
    }

    /// Item 3 (HIGH), the time form, with large `K` and `M`: two million dots
    /// and two hundred thousand names in one statement (2.6 MB, under the
    /// 8 MiB cap). The round-3 code rebuilt the `K`-dot prefix for every
    /// name, `K x M` = 4e11 bytes of copying, several seconds on this
    /// ticket's machine and outside any deadline check; counted once, the
    /// same statement is linear. Timed on the extraction step alone (the
    /// parse, itself linear, is done first, outside the timing).
    #[test]
    fn ori_t_0036_python_relative_import_work_is_linear_in_dots_plus_names() {
        let (dots, names) = (2_000_000, 200_000);
        let source = python_relative_import(dots, names);
        let tree = parse_bounded(
            Language::Python,
            false,
            &source,
            Instant::now() + Duration::from_secs(120),
        )
        .expect("parses");
        let statement = first_import_from(&tree);
        // No edge cap here: this measures the names loop itself, over every
        // one of its names, not the cap that stops a real module's loop at
        // `MAX_DEPENDENCY_EDGES_PER_MODULE`.
        let mut sink = EdgeSink::with_limit(usize::MAX);
        let start = Instant::now();
        let completed = python_import_from_edges(
            statement,
            source.as_bytes(),
            "m.py",
            &KnownPaths::new(),
            &mut sink,
            Instant::now() + Duration::from_secs(120),
        );
        let elapsed = start.elapsed();
        assert!(completed);
        assert_eq!(sink.edges.len(), names);
        assert!(
            elapsed < Duration::from_secs(3),
            "{dots} dots and {names} names took {elapsed:?}; the prefix must be built once, not \
             once per name"
        );
    }

    /// Item 3 (HIGH), what bounding the dots must not break: a relative
    /// import that stays inside the map still resolves, one that climbs past
    /// the mapped root never resolves to a file inside it (the round-3 code
    /// clamped the climb at the root and did), and an unresolved dotted form
    /// keeps its own dot count in its text.
    #[test]
    fn ori_t_0036_python_relative_imports_resolve_inside_the_map_and_never_above_it() {
        let dir = temp_dir("py-relative-bounds");
        let guard = DropGuard(dir.clone());
        write(&dir, "b.py", "x = 1\n");
        write(&dir, "pkg/b.py", "x = 1\n");
        write(
            &dir,
            "pkg/sub/m.py",
            "from .. import b\nfrom ...gone import y\n",
        );
        write(&dir, "top.py", "from .. import b\n");
        let map = build_code_map(&dir).expect("maps");
        let edges_of = |path: &str| -> Vec<(String, bool)> {
            map.modules
                .iter()
                .find(|m| m.path == path)
                .expect("present")
                .dependency_edges
                .iter()
                .map(|e| (e.to.to_string(), e.external))
                .collect()
        };
        assert_eq!(
            edges_of("pkg/sub/m.py"),
            vec![("pkg/b.py".to_owned(), false), ("...gone".to_owned(), true)],
        );
        assert_eq!(
            edges_of("top.py"),
            vec![("..b".to_owned(), true)],
            "from .. in a top-level file climbs out of the map; it must not resolve to the \
             root's own b.py"
        );
        drop(guard);
    }
    /// A small crate for the grouped-import tests: `src/lib.rs` (the crate
    /// root), `src/foo.rs`, `src/bar.rs`, `src/net.rs`, `src/net/http.rs`
    /// and `src/net/tcp.rs`.
    fn write_grouped_import_crate(dir: &Path) {
        write(
            dir,
            "src/lib.rs",
            "pub mod foo;\npub mod bar;\npub mod net;\n",
        );
        write(dir, "src/foo.rs", "pub struct Foo;\npub struct Baz;\n");
        write(dir, "src/bar.rs", "pub struct Bar;\n");
        write(dir, "src/net.rs", "pub mod http;\npub mod tcp;\n");
        write(dir, "src/net/http.rs", "pub struct Client;\n");
        write(dir, "src/net/tcp.rs", "pub struct Stream;\n");
    }

    /// `module`'s dependency edges as sorted `(to, external)` pairs.
    fn edge_pairs(map: &CodeMap, module: &str) -> Vec<(String, bool)> {
        let mut pairs: Vec<(String, bool)> = map
            .modules
            .iter()
            .find(|m| m.path == module)
            .unwrap_or_else(|| panic!("{module} present"))
            .dependency_edges
            .iter()
            .map(|e| (e.to.to_string(), e.external))
            .collect();
        pairs.sort();
        pairs
    }

    /// Item 5 (LOW), the rule itself: a grouped import gives exactly the
    /// edges its ungrouped equivalent gives, member for member, resolved or
    /// not. Covers a top-level group, a group under a prefix, groups nested
    /// in groups, a group with no prefix at all mixing `crate` and `std`,
    /// members that do not resolve, `self`, a wildcard, an alias, and an
    /// all-external group. The round-3 code resolved none of the grouped
    /// forms and turned the prefixed one into an edge to `src/net.rs`.
    #[test]
    fn ori_t_0036_a_grouped_use_gives_exactly_the_edges_of_its_ungrouped_equivalent() {
        let dir = temp_dir("grouped-equivalence");
        let guard = DropGuard(dir.clone());
        write_grouped_import_crate(&dir);
        write(
            &dir,
            "src/grouped.rs",
            "use crate::{foo::Foo, bar::Bar};\n\
             use crate::net::{http::Client, tcp::Stream};\n\
             use crate::{foo::{Foo, Baz}, net::{self, http::{self, Client}, tcp::Stream}};\n\
             use {crate::foo::Foo, std::fmt};\n\
             use crate::{Error, Result, net::Missing};\n\
             use crate::{foo::*, bar::Bar as B};\n\
             use std::{fmt, io};\n",
        );
        write(
            &dir,
            "src/ungrouped.rs",
            "use crate::foo::Foo;\nuse crate::bar::Bar;\n\
             use crate::net::http::Client;\nuse crate::net::tcp::Stream;\n\
             use crate::foo::Foo;\nuse crate::foo::Baz;\nuse crate::net;\n\
             use crate::net::http;\nuse crate::net::http::Client;\nuse crate::net::tcp::Stream;\n\
             use crate::foo::Foo;\nuse std::fmt;\n\
             use crate::Error;\nuse crate::Result;\nuse crate::net::Missing;\n\
             use crate::foo::*;\nuse crate::bar::Bar as B;\n\
             use std::fmt;\nuse std::io;\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let grouped = edge_pairs(&map, "src/grouped.rs");
        let ungrouped = edge_pairs(&map, "src/ungrouped.rs");
        assert_eq!(grouped, ungrouped);
        assert!(
            grouped
                .iter()
                .any(|(to, ext)| to == "src/net/http.rs" && !ext)
                && grouped
                    .iter()
                    .any(|(to, ext)| to == "src/net/tcp.rs" && !ext),
            "the prefixed and nested members must resolve past their prefix: {grouped:?}"
        );
        assert!(
            grouped.iter().all(|(to, _)| !to.contains('{')),
            "no edge may carry grouped text: {grouped:?}"
        );
        drop(guard);
    }

    /// Item 5 (LOW), the exact case the review of round 3 found: `use
    /// crate::net::{http::Client, tcp::Stream};` with `src/net.rs`,
    /// `src/net/http.rs` and `src/net/tcp.rs` all present gave one edge, to
    /// `src/net.rs`: a complete-looking edge to the wrong file. It must give
    /// the two member files and nothing else.
    #[test]
    fn ori_t_0036_a_prefixed_grouped_use_never_resolves_to_its_prefix_alone() {
        let dir = temp_dir("grouped-prefixed");
        let guard = DropGuard(dir.clone());
        write_grouped_import_crate(&dir);
        write(
            &dir,
            "src/user.rs",
            "use crate::net::{http::Client, tcp::Stream};\n",
        );
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(
            edge_pairs(&map, "src/user.rs"),
            vec![
                ("src/net/http.rs".to_owned(), false),
                ("src/net/tcp.rs".to_owned(), false),
            ]
        );
        drop(guard);
    }

    /// Item 5 (LOW), nesting, exactly: groups inside groups inside a
    /// prefix, with a `self` member and an unresolved member at the inner
    /// level, each member placed by its own full path.
    #[test]
    fn ori_t_0036_a_nested_grouped_use_resolves_every_member_at_every_level() {
        let dir = temp_dir("grouped-nested");
        let guard = DropGuard(dir.clone());
        write_grouped_import_crate(&dir);
        write(
            &dir,
            "src/user.rs",
            "use crate::{net::{http::{self, Client}, tcp::{Stream, Gone}}, foo::Foo};\n",
        );
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(
            edge_pairs(&map, "src/user.rs"),
            vec![
                ("src/foo.rs".to_owned(), false),
                ("src/net/http.rs".to_owned(), false),
                ("src/net/http.rs".to_owned(), false),
                ("src/net/tcp.rs".to_owned(), false),
                ("src/net/tcp.rs".to_owned(), false),
            ],
            "Gone is under tcp, so, like its ungrouped form, it resolves to tcp.rs by its \
             longest prefix"
        );
        drop(guard);
    }

    /// Item 5 (LOW), the text bound: a long prefix that resolves nowhere,
    /// over many members, would cost prefix length times member count to
    /// spell out member by member (here 2000 members under a 905-byte
    /// prefix, about 1.8 MB). Past the budget, the members not yet spelled
    /// out are recorded once, as the declaration as written, and the total
    /// text stays near the declaration's own size.
    ///
    /// Rewritten in round 6, when every unresolved target became capped at
    /// `MAX_EDGE_TARGET_BYTES`: round 4's version used a 6 KB prefix, so
    /// every member it spelled out is now cut to the same first 1024 bytes
    /// as the declaration itself, and "recorded once, as the declaration"
    /// could no longer be told apart from a member. The prefix is now under
    /// the cap, so each member spelled out is whole and exact, and the one
    /// edge standing for the rest is the declaration cut to the cap and
    /// marked. The budget still binds (asserted, not assumed).
    #[test]
    fn ori_t_0036_a_grouped_use_with_a_long_unresolved_prefix_stays_bounded() {
        let dir = temp_dir("grouped-budget");
        let guard = DropGuard(dir.clone());
        write(&dir, "src/lib.rs", "pub fn f() {}\n");
        let prefix = format!("crate{}", "::q".repeat(300));
        let mut argument = format!("{prefix}::{{");
        let members: Vec<String> = (0..2000).map(|i| format!("x{i}")).collect();
        argument.push_str(&members.join(", "));
        argument.push('}');
        assert!(prefix.len() + 2 + "x1999".len() < MAX_EDGE_TARGET_BYTES);
        assert!(argument.len() > MAX_EDGE_TARGET_BYTES);
        write(&dir, "src/user.rs", &format!("use {argument};\n"));
        let map = build_code_map(&dir).expect("maps");
        let user = map
            .modules
            .iter()
            .find(|m| m.path == "src/user.rs")
            .expect("present");
        let total: usize = user.dependency_edges.iter().map(|e| e.to.len()).sum();
        let budget = argument.len() * USE_GROUP_TEXT_FACTOR + USE_GROUP_TEXT_SLACK;
        assert!(
            total <= budget + MAX_EDGE_TARGET_BYTES,
            "{total} bytes of edge text for a {} byte declaration; the cap is {budget} plus \
             one capped target",
            argument.len()
        );
        let (rest, spelled): (Vec<&DependencyEdge>, Vec<&DependencyEdge>) = user
            .dependency_edges
            .iter()
            .partition(|e| e.to.contains('{'));
        assert_eq!(
            rest.len(),
            1,
            "the members past the budget must be recorded, once: {rest:?}"
        );
        assert!(
            rest[0].external
                && rest[0].to_truncated
                && rest[0].to.len() == MAX_EDGE_TARGET_BYTES
                && argument.starts_with(rest[0].to.as_str()),
            "that one edge is the declaration as written, cut to the cap and marked: {:?}",
            rest[0]
        );
        assert!(
            !spelled.is_empty() && spelled.len() < members.len(),
            "the budget must bind: {} of {} members spelled out",
            spelled.len(),
            members.len()
        );
        let expected: HashSet<String> = members
            .iter()
            .map(|member| format!("{prefix}::{member}"))
            .collect();
        assert!(
            spelled
                .iter()
                .all(|e| e.external && !e.to_truncated && expected.contains(e.to.as_str())),
            "every member spelled out before the budget ran out is its own full path, whole"
        );
        drop(guard);
    }

    /// Item 5 (LOW), the walk's own deadline: a grouped import is walked
    /// node by node, and an expired deadline must stop it and say so, the
    /// same as every other stage.
    #[test]
    fn ori_t_0036_the_grouped_use_walk_checks_its_deadline() {
        let source = "use crate::{a::A, b::B, c::C};\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("set language");
        let tree = parser.parse(source, None).expect("parses");
        let root = tree.root_node();
        let mut cursor = root.walk();
        let declaration = root
            .children(&mut cursor)
            .find(|child| child.kind() == "use_declaration")
            .expect("a use declaration");
        let argument = declaration
            .child_by_field_name("argument")
            .expect("an argument");
        let already_past = Instant::now() - Duration::from_secs(1);
        let mut sink = EdgeSink::new();
        let completed = rust_use_edges(argument, source.as_bytes(), None, &mut sink, already_past);
        assert!(!completed, "an expired deadline must stop the walk");
        assert!(sink.edges.is_empty(), "{:?}", sink.edges);
        let mut sink = EdgeSink::new();
        let completed = rust_use_edges(
            argument,
            source.as_bytes(),
            None,
            &mut sink,
            Instant::now() + Duration::from_secs(60),
        );
        assert!(completed);
        assert_eq!(sink.edges.len(), 3, "{:?}", sink.edges);
    }
    /// Item 4 (mutant survival), V3 of the review of round 3: `files_seen`
    /// counting only the pre-skips whose path has no `/` survived every
    /// test, because every invariant fixture put its pre-skips at the top
    /// level. Here every reason the walk itself can record sits two
    /// directories down (the unreadable directory one down, since its
    /// contents are what cannot be listed), next to one clean top-level
    /// file and one clean nested one, and the count is asserted exactly, not
    /// only through the invariant.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_the_files_seen_invariant_holds_with_every_pre_skip_reason_nested() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("invariant-nested");
        let guard = DropGuard(dir.clone());
        let outside = temp_dir("invariant-nested-outside");
        let outside_guard = DropGuard(outside.clone());
        fs::write(outside.join("secret.rs"), "pub fn secret() {}\n").expect("write outside");
        write(&dir, "clean.rs", "pub fn clean() {}\n");
        write(&dir, "deep/nested/ok.rs", "pub fn ok() {}\n");
        write(&dir, ".git/secret.rs", "pub fn git_secret() {}\n");
        write(&dir, "deep/target_dir/inside.rs", "pub fn inside() {}\n");
        let nested = dir.join("deep/nested");
        let link = |target: &Path, name: &str| {
            std::os::unix::fs::symlink(target, nested.join(name)).expect("create symlink");
        };
        link(&dir.join("deep/nowhere.rs"), "dangling.rs");
        link(&dir.join("deep/target_dir"), "linked_dir");
        link(&outside.join("secret.rs"), "escapes.rs");
        link(&dir.join(".git/secret.rs"), "z.rs");
        let fifo_ok = make_fifo(&nested.join("pipe.rs"));
        if fifo_ok {
            link(&nested.join("pipe.rs"), "pipe_link.rs");
        }
        let locked = dir.join("deep/locked");
        fs::create_dir_all(&locked).expect("create locked dir");
        fs::write(locked.join("hidden.rs"), "pub fn hidden() {}\n").expect("write hidden");
        let mut perms = fs::metadata(&locked).expect("stat").permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&locked, perms).expect("chmod");
        let locked_is_unreadable = fs::read_dir(&locked).is_err();

        let map = build_code_map(&dir);

        let mut restore = fs::metadata(&locked).expect("stat").permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(&locked, restore);

        let map = map.expect("maps");
        let c = &map.coverage;
        assert_eq!(
            c.files_seen,
            c.files_parsed_clean + c.files_parsed_with_errors + c.files_skipped.len(),
            "the invariant must hold with every pre-skip nested: {c:?}"
        );
        let mut expected: Vec<(&str, SkipReason)> = vec![
            (
                "deep/nested/dangling.rs",
                SkipReason::Unreadable(String::new()),
            ),
            ("deep/nested/escapes.rs", SkipReason::SymlinkOutsideRoot),
            ("deep/nested/linked_dir", SkipReason::SymlinkedDirectory),
            ("deep/nested/z.rs", SkipReason::GitMetadata),
        ];
        if fifo_ok {
            expected.push(("deep/nested/pipe.rs", SkipReason::NotARegularFile));
            expected.push(("deep/nested/pipe_link.rs", SkipReason::NotARegularFile));
        }
        if locked_is_unreadable {
            expected.push(("deep/locked", SkipReason::Unreadable(String::new())));
        } else {
            eprintln!(
                "chmod 000 did not restrict access here; the unreadable half is not exercised"
            );
        }
        for (path, reason) in &expected {
            let found = c.files_skipped.iter().find(|f| f.path == *path);
            let matches = match (found.map(|f| &f.reason), reason) {
                (Some(SkipReason::Unreadable(_)), SkipReason::Unreadable(_)) => true,
                (Some(actual), wanted) => actual == wanted,
                (None, _) => false,
            };
            assert!(matches, "{path} must be recorded as {reason:?}: {c:?}");
        }
        // Two clean modules (`clean.rs`, `deep/nested/ok.rs`) and
        // `deep/target_dir/inside.rs`, a third, reached directly; plus every
        // nested pre-skip above. `.git` is never walked.
        assert_eq!(c.files_seen, 3 + expected.len(), "{c:?}");
        drop(guard);
        drop(outside_guard);
    }

    /// Item 4 (mutant survival), the second `files_seen` mutant the review
    /// of round 3 listed: dropping `SymlinkedDirectory` from the count
    /// survived, since the test that checks the record never checks the
    /// count, and no invariant fixture had a symlinked directory.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlinked_directory_counts_in_files_seen() {
        let dir = temp_dir("symlinked-dir-counted");
        let guard = DropGuard(dir.clone());
        let target = temp_dir("symlinked-dir-counted-target");
        let target_guard = DropGuard(target.clone());
        write(&dir, "real.rs", "pub fn real_fn() {}\n");
        std::os::unix::fs::symlink(&target, dir.join("linked_dir")).expect("symlink");
        let map = build_code_map(&dir).expect("maps");
        let c = &map.coverage;
        assert_eq!(c.files_seen, 2, "real.rs and linked_dir: {c:?}");
        assert_eq!(
            c.files_seen,
            c.files_parsed_clean + c.files_parsed_with_errors + c.files_skipped.len()
        );
        drop(guard);
        drop(target_guard);
    }

    /// Item 4 (mutant survival): the `interfaces` sort's *primary* key.
    /// The round-3 test put both items on one line, which exercises only
    /// the name tiebreak, so sorting by name alone survived. Two lines in
    /// reverse alphabetical order must stay in line order.
    #[test]
    fn ori_t_0036_interfaces_are_sorted_by_line_first_not_by_name() {
        let dir = temp_dir("interfaces-line-first");
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub struct Zed;\npub struct Abe;\n");
        let map = build_code_map(&dir).expect("maps");
        let names: Vec<&str> = map.modules[0]
            .interfaces
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        assert_eq!(names, vec!["Zed", "Abe"]);
        drop(guard);
    }

    /// Item 4 (mutant survival): the `spec_sections` sort's *primary* key.
    /// The round-3 test put both headings in one document, which exercises
    /// only the heading tiebreak, so sorting by heading alone survived. Two
    /// documents whose headings sort the other way must stay in document
    /// order.
    #[test]
    fn ori_t_0036_spec_sections_are_sorted_by_doc_first_not_by_heading() {
        let dir = temp_dir("specsections-doc-first");
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub fn m() {}\n");
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(&dir, &format!("spec/{}.md", "a"), "# Zeta\n\nm.rs\n");
        write(&dir, &format!("spec/{}.md", "b"), "# Alpha\n\nm.rs\n");
        let map = build_code_map(&dir).expect("maps");
        let cited: Vec<(String, Option<String>)> = map.modules[0]
            .spec_sections
            .iter()
            .map(|c| {
                (
                    map.spec_doc(c).unwrap_or_default().to_owned(),
                    map.spec_heading(c).map(|h| h.text.clone()),
                )
            })
            .collect();
        assert_eq!(
            cited,
            vec![
                (format!("spec/{}.md", "a"), Some("Zeta".to_owned())),
                (format!("spec/{}.md", "b"), Some("Alpha".to_owned())),
            ]
        );
        drop(guard);
    }

    /// Item 4 (mutant survival): removing `covering_tests.dedup()` survived.
    /// A Python module named `test.py` has two filename-convention
    /// candidates that are the same file, `test_test.py`
    /// (`test_<stem>.py` and `<stem>_test.py`), so it is found twice and
    /// must be listed once.
    #[test]
    fn ori_t_0036_a_covering_test_found_by_two_conventions_is_listed_once() {
        let dir = temp_dir("covering-dedup");
        let guard = DropGuard(dir.clone());
        write(&dir, "test.py", "x = 1\n");
        write(&dir, "test_test.py", "def check():\n    pass\n");
        let map = build_code_map(&dir).expect("maps");
        let module = map
            .modules
            .iter()
            .find(|m| m.path == "test.py")
            .expect("present");
        assert_eq!(module.covering_tests, vec!["test_test.py".to_owned()]);
        drop(guard);
    }

    /// Item 4 (mutant survival): `go_exported` always returning `true`
    /// survived. Only an identifier starting with an uppercase letter is
    /// exported in Go, so `hidden` and `main` are not interfaces.
    #[test]
    fn ori_t_0036_an_unexported_go_identifier_is_not_an_interface() {
        let dir = temp_dir("go-unexported");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "main.go",
            "package main\n\nfunc Hello() {}\n\nfunc hidden() {}\n\nfunc main() {}\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let names: Vec<&str> = map.modules[0]
            .interfaces
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        assert_eq!(names, vec!["Hello"]);
        drop(guard);
    }

    /// Item 4 (mutant survival): weakening the Python entry-point guard to
    /// "the condition mentions `__name__`" survived. A guard comparing
    /// `__name__` to anything other than `"__main__"` is not an entry point.
    #[test]
    fn ori_t_0036_a_python_name_guard_not_naming_main_is_not_an_entry_point() {
        let dir = temp_dir("py-not-main-guard");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "m.py",
            "def f():\n    pass\n\n\nif __name__ == \"not_main\":\n    f()\n",
        );
        let map = build_code_map(&dir).expect("maps");
        assert!(
            map.modules[0].entry_points.is_empty(),
            "{:?}",
            map.modules[0].entry_points
        );
        drop(guard);
    }

    /// Item 4 (mutant survival): removing the underscore filter on Python
    /// *classes* survived (the round-2 test covers functions only).
    #[test]
    fn ori_t_0036_a_python_underscore_prefixed_class_is_not_an_interface() {
        let dir = temp_dir("py-private-class");
        let guard = DropGuard(dir.clone());
        write(
            &dir,
            "m.py",
            "class _Hidden:\n    pass\n\n\nclass Shown:\n    pass\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let names: Vec<&str> = map.modules[0]
            .interfaces
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        assert_eq!(names, vec!["Shown"]);
        drop(guard);
    }
    // -----------------------------------------------------------------
    // ORI-T-0036, round 5 (2026-09-24): the spec scan's memory, from the
    // review of round 3's residual and the coordinator's ruling that it is
    // inside the threat model.
    // -----------------------------------------------------------------

    /// Round 5, item 2, measured: the review of round 3's shape, many
    /// modules each cited under many long, distinct headings. Here 64
    /// modules (`a.rs`, `aa.rs`, ... 64 `a`s, so one line of 64 `a`s cites
    /// every one of them) and one document of 500 sections, each a distinct
    /// heading of about 4.2 KB followed by that line. Stored per citation,
    /// as round 4 did, that is 64 x 500 headings of 4096 bytes, 131 MB kept
    /// in the returned map (the review measured 2.1 GB at 1000 modules).
    /// Stored once each and capped, it is 500 x 4096 bytes, and this test
    /// sums what the returned `CodeMap` actually holds and checks it against
    /// the module doc's stated bound: at most `MAX_HEADING_BYTES` per
    /// distinct cited heading, and never more than the corpus read.
    #[test]
    fn ori_t_0036_retained_heading_text_is_bounded_for_many_modules_citing_long_headings() {
        let dir = temp_dir("spec-retained-headings");
        let guard = DropGuard(dir.clone());
        let modules = 64;
        for k in 1..=modules {
            write(&dir, &format!("{}.rs", "a".repeat(k)), "pub fn f() {}\n");
        }
        let citing_line = format!("{}.rs", "a".repeat(modules));
        let sections = MAX_SPEC_CITATIONS_PER_MODULE;
        let mut doc = String::new();
        for i in 0..sections {
            doc.push_str(&format!(
                "# {i:04} {}\n\n{citing_line}\n\n",
                "h".repeat(4200)
            ));
        }
        let corpus_bytes = doc.len();
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(&dir, &format!("spec/{}.md", "x"), &doc);
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Complete);
        assert_eq!(map.modules.len(), modules);
        assert!(
            map.modules
                .iter()
                .all(|m| m.spec_sections.len() == sections && !m.spec_sections_truncated),
            "every module is cited under every one of the {sections} headings"
        );

        let retained: usize = map.spec_headings.iter().map(|h| h.text.len()).sum();
        let bound = (sections * MAX_HEADING_BYTES).min(corpus_bytes);
        assert!(
            map.spec_headings.len() <= sections,
            "each distinct heading is stored once for the whole map, not once per citing \
             module: {} stored for {sections} distinct headings",
            map.spec_headings.len()
        );
        assert!(
            retained <= bound,
            "retained heading text is {retained} bytes; the stated bound for {sections} distinct \
             headings over a {corpus_bytes} byte corpus is {bound}"
        );
        assert!(
            map.spec_headings
                .iter()
                .all(|h| h.truncated && h.text.len() <= MAX_HEADING_BYTES),
            "every one of these headings is longer than the cap, so each is cut and marked"
        );
        assert!(
            std::mem::size_of::<SpecCitation>() <= 24,
            "a citation is two indices, the per-citation cost the module doc states"
        );
        drop(guard);
    }

    /// Round 5, item 2, the semantics of storing once: the same heading text
    /// cited by two modules in two documents is one entry, which every such
    /// citation indexes; a heading at or under the cap is kept whole and not
    /// marked; a longer one is cut to `MAX_HEADING_BYTES` and marked.
    #[test]
    fn ori_t_0036_a_heading_is_stored_once_and_marked_only_when_cut() {
        let dir = temp_dir("spec-heading-once");
        let guard = DropGuard(dir.clone());
        write(&dir, "m1.rs", "pub fn f() {}\n");
        write(&dir, "m2.rs", "pub fn f() {}\n");
        let long = "x".repeat(5000);
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(
            &dir,
            &format!("spec/{}.md", "a"),
            &format!("# Shared\n\nm1.rs m2.rs\n\n# {long}\n\nm1.rs\n"),
        );
        write(&dir, &format!("spec/{}.md", "b"), "# Shared\n\nm2.rs\n");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.spec_headings.len(), 2, "{:?}", map.spec_headings.len());
        let shared: Vec<&SpecHeading> = map
            .spec_headings
            .iter()
            .filter(|h| h.text == "Shared")
            .collect();
        assert_eq!(shared.len(), 1, "one entry for the shared heading");
        assert!(
            !shared[0].truncated,
            "a short heading is kept whole, unmarked"
        );
        let cut = map
            .spec_headings
            .iter()
            .find(|h| h.text != "Shared")
            .expect("the long heading");
        assert!(cut.truncated && cut.text.len() == MAX_HEADING_BYTES);
        let shared_index = map.spec_headings.iter().position(|h| h.text == "Shared");
        let citing_shared = map
            .modules
            .iter()
            .flat_map(|m| m.spec_sections.iter())
            .filter(|c| c.heading_index == shared_index)
            .count();
        assert_eq!(
            citing_shared, 3,
            "m1 in a.md, m2 in a.md and m2 in b.md all index the one shared entry"
        );
        drop(guard);
    }

    /// Round 5, item 3: a `spec/` corpus over the total budget is scanned up
    /// to the first document that does not fit and reported
    /// `CorpusOverBudget`, never complete, and the citations found before
    /// that are kept. The same corpus at a budget it exactly fits is
    /// complete, so the budget is a bound, not an alarm that always trips.
    #[test]
    fn ori_t_0036_a_spec_corpus_over_the_total_budget_is_reported_incomplete() {
        assert_eq!(
            CodeMapOptions::default().max_spec_bytes,
            64 * 1024 * 1024,
            "the default the module doc states"
        );
        let dir = temp_dir("spec-corpus-budget");
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub fn f() {}\n");
        let padding = "p".repeat(580);
        let first = format!("# First\n\nm.rs\n{padding}\n");
        let second = format!("# Second\n\nm.rs\n{padding}\n");
        let total = (first.len() + second.len()) as u64;
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(&dir, &format!("spec/{}.md", "a"), &first);
        write(&dir, &format!("spec/{}.md", "b"), &second);

        let over = CodeMapOptions {
            max_spec_bytes: total - 1,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &over).expect("maps");
        assert_eq!(
            map.coverage.spec_citation_scan,
            SpecScan::CorpusOverBudget { budget: total - 1 },
            "one byte over the budget must be reported, not scanned as if complete"
        );
        assert!(!map.coverage.spec_citation_scan.is_complete());
        let cited: Vec<Option<&str>> = map.modules[0]
            .spec_sections
            .iter()
            .map(|c| map.spec_heading(c).map(|h| h.text.as_str()))
            .collect();
        assert_eq!(
            cited,
            vec![Some("First")],
            "what was scanned before the budget ran out is kept; the document that did not \
             fit is not scanned"
        );
        assert_eq!(
            map.coverage.spec_docs_skipped,
            vec![SkippedFile {
                path: format!("spec/{}.md", "b"),
                reason: SkipReason::SpecCorpusOverBudget { budget: total - 1 },
            }],
            "the document that did not fit is recorded, with the reason"
        );

        let exact = CodeMapOptions {
            max_spec_bytes: total,
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &exact).expect("maps");
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Complete);
        assert!(map.coverage.spec_docs_skipped.is_empty());
        assert_eq!(map.modules[0].spec_sections.len(), 2);
        drop(guard);
    }
    // -----------------------------------------------------------------
    // ORI-T-0036, round 5 follow-up: a spec document the scan passes over
    // is recorded with its reason, and the scan is not called complete.
    // -----------------------------------------------------------------

    /// A repository with one module, `m.rs`, one document citing it (`good`
    /// under `spec/`), and whatever `extra` adds, mapped with `options`.
    fn map_with_spec_extra(
        label: &str,
        options: &CodeMapOptions,
        extra: impl FnOnce(&Path),
    ) -> (DropGuard, CodeMap) {
        let dir = temp_dir(label);
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub fn f() {}\n");
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        write(&dir, &format!("spec/{}.md", "good"), "# Good\n\nm.rs\n");
        extra(&dir);
        let map = build_code_map_with_options(&dir, options).expect("maps");
        (guard, map)
    }

    /// The scan is partial, `path` is the one entry recorded as skipped,
    /// with a reason `expected` accepts, and the good document was still
    /// scanned: the skip stopped nothing else.
    fn assert_spec_skip(map: &CodeMap, path: &str, expected: impl Fn(&SkipReason) -> bool) {
        assert_eq!(
            map.coverage.spec_citation_scan,
            SpecScan::Partial,
            "a scan that passed over a document is not complete: {:?}",
            map.coverage
        );
        assert_eq!(
            map.coverage.spec_docs_skipped.len(),
            1,
            "{:?}",
            map.coverage
        );
        let recorded = &map.coverage.spec_docs_skipped[0];
        assert_eq!(recorded.path, path);
        assert!(expected(&recorded.reason), "{recorded:?}");
        assert_eq!(
            map.modules[0].spec_sections.len(),
            1,
            "the good document is still scanned"
        );
    }

    /// Reason one of three: a document that cannot be read.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_an_unreadable_spec_document_makes_the_scan_partial_and_is_recorded() {
        use std::os::unix::fs::PermissionsExt;
        let bad = format!("spec/{}.md", "bad");
        let mut locked: Option<PathBuf> = None;
        let (guard, map) =
            map_with_spec_extra("spec-skip-unreadable", &CodeMapOptions::default(), |dir| {
                write(dir, &bad, "# Bad\n\nm.rs\n");
                let path = dir.join(&bad);
                let mut perms = fs::metadata(&path).expect("stat").permissions();
                perms.set_mode(0o000);
                fs::set_permissions(&path, perms).expect("chmod");
                locked = Some(path);
            });
        let path = locked.expect("locked");
        let still_readable = fs::read(&path).is_ok();
        let mut restore = fs::metadata(&path).expect("stat").permissions();
        restore.set_mode(0o644);
        let _ = fs::set_permissions(&path, restore);
        if still_readable {
            eprintln!(
                "running with elevated privileges; chmod 000 did not restrict access, skipping"
            );
            drop(guard);
            return;
        }
        assert_spec_skip(&map, &bad, |reason| {
            matches!(reason, SkipReason::Unreadable(_))
        });
        drop(guard);
    }

    /// Reason two of three: a document that is not UTF-8.
    #[test]
    fn ori_t_0036_a_non_utf8_spec_document_makes_the_scan_partial_and_is_recorded() {
        let bad = format!("spec/{}.md", "bad");
        let (guard, map) =
            map_with_spec_extra("spec-skip-binary", &CodeMapOptions::default(), |dir| {
                fs::write(
                    dir.join(&bad),
                    [b'#', b' ', 0xff, 0xfe, b'\n', b'm', b'.', b'r', b's'],
                )
                .expect("write");
            });
        assert_spec_skip(&map, &bad, |reason| *reason == SkipReason::Binary);
        drop(guard);
    }

    /// Reason three of three: a document over the per-file cap, recorded
    /// with what was actually read (at most the cap plus one byte).
    #[test]
    fn ori_t_0036_a_spec_document_over_the_per_file_cap_makes_the_scan_partial_and_is_recorded() {
        let bad = format!("spec/{}.md", "bad");
        let options = CodeMapOptions {
            max_file_bytes: 200,
            ..CodeMapOptions::default()
        };
        let (guard, map) = map_with_spec_extra("spec-skip-too-large", &options, |dir| {
            write(dir, &bad, &format!("# Big\n\nm.rs\n{}\n", "x".repeat(300)));
        });
        assert_spec_skip(&map, &bad, |reason| {
            *reason
                == SkipReason::TooLarge {
                    bytes: 201,
                    cap: 200,
                }
        });
        drop(guard);
    }

    /// The other entries the scan passes over are recorded the same way: a
    /// symlink named `*.md`, a symlink to a directory, an unreadable
    /// subdirectory. A symlink that stands for no document (here, one named
    /// `*.txt` to a file) is not a document and is not recorded.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_spec_symlinks_and_unreadable_directories_are_recorded_not_passed_over() {
        use std::os::unix::fs::PermissionsExt;
        let mut locked: Option<PathBuf> = None;
        let (guard, map) =
            map_with_spec_extra("spec-skip-links", &CodeMapOptions::default(), |dir| {
                write(dir, "notes/real.md", "# Real\n\nm.rs\n");
                write(dir, "notes/real.txt", "m.rs\n");
                let spec = dir.join("spec");
                std::os::unix::fs::symlink(dir.join("notes/real.md"), spec.join("linked.md"))
                    .expect("symlink");
                std::os::unix::fs::symlink(dir.join("notes"), spec.join("linked_dir"))
                    .expect("symlink");
                std::os::unix::fs::symlink(dir.join("notes/real.txt"), spec.join("other.txt"))
                    .expect("symlink");
                let sub = spec.join("locked");
                fs::create_dir_all(&sub).expect("mkdir");
                fs::write(sub.join("inner.md"), "# Inner\n\nm.rs\n").expect("write");
                let mut perms = fs::metadata(&sub).expect("stat").permissions();
                perms.set_mode(0o000);
                fs::set_permissions(&sub, perms).expect("chmod");
                locked = Some(sub);
            });
        let sub = locked.expect("locked");
        let still_readable = fs::read_dir(&sub).is_ok();
        let mut restore = fs::metadata(&sub).expect("stat").permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(&sub, restore);

        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Partial);
        let recorded: Vec<(&str, &SkipReason)> = map
            .coverage
            .spec_docs_skipped
            .iter()
            .map(|f| (f.path.as_str(), &f.reason))
            .collect();
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        let linked_md = format!("spec/{}.md", "linked");
        assert!(recorded.contains(&(linked_md.as_str(), &SkipReason::SymlinkNotFollowed)));
        assert!(recorded.contains(&("spec/linked_dir", &SkipReason::SymlinkNotFollowed)));
        assert!(
            recorded.iter().all(|(path, _)| *path != "spec/other.txt"),
            "a link standing for no document is not recorded: {recorded:?}"
        );
        if still_readable {
            eprintln!("running with elevated privileges; the unreadable half is not exercised");
        } else {
            assert!(
                recorded.iter().any(|(path, reason)| *path == "spec/locked"
                    && matches!(reason, SkipReason::Unreadable(_))),
                "{recorded:?}"
            );
        }
        drop(guard);
    }

    /// A `spec` entry that is itself a symlink to a directory is recorded
    /// too: the documents it stands for are not read, so the scan is not
    /// complete.
    #[cfg(unix)]
    #[test]
    fn ori_t_0036_a_symlinked_spec_directory_makes_the_scan_partial() {
        let dir = temp_dir("spec-root-link");
        let guard = DropGuard(dir.clone());
        write(&dir, "m.rs", "pub fn f() {}\n");
        write(&dir, "docs/x.md", "# X\n\nm.rs\n");
        std::os::unix::fs::symlink(dir.join("docs"), dir.join("spec")).expect("symlink");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Partial);
        assert_eq!(
            map.coverage.spec_docs_skipped,
            vec![SkippedFile {
                path: "spec".to_owned(),
                reason: SkipReason::SymlinkNotFollowed,
            }]
        );
        assert!(map.modules[0].spec_sections.is_empty());
        drop(guard);
    }

    /// A scan the deadline cut short records the document it was in, not
    /// only its own status.
    #[test]
    fn ori_t_0036_a_spec_document_the_deadline_interrupted_is_recorded() {
        let dir = temp_dir("spec-skip-deadline");
        let guard = DropGuard(dir.clone());
        let prefix = "as".repeat(20);
        for n in 0..192 {
            write(&dir, &format!("{prefix}a{n:04}.rs"), "pub fn f() {}\n");
        }
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        let long = format!("spec/{}.md", "long");
        write(&dir, &long, &"as".repeat(4_194_300));
        let options = CodeMapOptions {
            file_timeout: Duration::from_millis(250),
            ..CodeMapOptions::default()
        };
        let map = build_code_map_with_options(&dir, &options).expect("maps");
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::TimedOut);
        assert_eq!(
            map.coverage.spec_docs_skipped,
            vec![SkippedFile {
                path: long,
                reason: SkipReason::TimedOut,
            }]
        );
        drop(guard);
    }

    /// The other half of the rule: a scan that read and scanned every
    /// document is complete and recorded nothing skipped.
    #[test]
    fn ori_t_0036_a_complete_spec_scan_skipped_nothing() {
        let (_guard, map) = built_fixture();
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Complete);
        assert!(map.coverage.spec_docs_skipped.is_empty());
    }

    // -----------------------------------------------------------------
    // ORI-T-0036, round 6 (2026-09-27), from the review of round 5: what
    // one module keeps from its own file, and `.git` under `spec/`.
    // -----------------------------------------------------------------

    /// A directory path of three 200-byte components, so that a module
    /// under it has a path of over 600 bytes: long enough that a copy of it
    /// per edge or per entry point is unmistakable, short enough (with the
    /// temporary directory in front) for every platform's path limit.
    fn long_directory() -> String {
        ["d", "e", "f"]
            .iter()
            .map(|letter| letter.repeat(200))
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Item 1 (HIGH), the defect itself: every `DependencyEdge` held a copy
    /// of its importing module's path (`from`), and every `EntryPoint` one
    /// too (`module`), so one file could hold its own path millions of
    /// times. Neither may hold it at all now: the `Module` that holds an
    /// edge or an entry point is where it comes from. Checked through each
    /// record's `Debug` rendering, which shows every field it has, whatever
    /// the fields are named, in all four languages and for resolved,
    /// unresolved and shebang records alike. The round-5 code fails this.
    #[test]
    fn ori_t_0036_no_edge_or_entry_point_holds_a_copy_of_its_module_path() {
        let dir = temp_dir("no-path-copies");
        let guard = DropGuard(dir.clone());
        let long = long_directory();
        write(
            &dir,
            &format!("{long}/m.py"),
            "import os\nfrom . import b\n\nif __name__ == \"__main__\":\n    pass\n",
        );
        write(&dir, &format!("{long}/b.py"), "x = 1\n");
        write(
            &dir,
            &format!("{long}/s.py"),
            "#!/usr/bin/env python3\nimport os\n",
        );
        write(
            &dir,
            &format!("{long}/m.rs"),
            "mod gone;\nuse std::fmt;\n\nfn main() {}\n",
        );
        write(
            &dir,
            &format!("{long}/m.ts"),
            "import \"./b\";\nimport \"y\";\n\nfunction main() {}\n",
        );
        write(&dir, &format!("{long}/b.ts"), "export const b = 1;\n");
        write(
            &dir,
            &format!("{long}/m.go"),
            "package main\n\nimport \"fmt\"\n\nfunc main() {}\n",
        );
        let map = build_code_map(&dir).expect("maps");
        let checked: Vec<&Module> = map
            .modules
            .iter()
            .filter(|m| m.path.ends_with("/m.py") || m.path.ends_with("/m.rs"))
            .chain(
                map.modules
                    .iter()
                    .filter(|m| m.path.ends_with("/m.ts") || m.path.ends_with("/m.go")),
            )
            .chain(map.modules.iter().filter(|m| m.path.ends_with("/s.py")))
            .collect();
        assert_eq!(checked.len(), 5, "{:?}", map.coverage);
        for module in checked {
            assert!(module.path.len() > 600, "{}", module.path);
            assert!(
                !module.dependency_edges.is_empty() && !module.entry_points.is_empty(),
                "the fixture must give {} both edges and entry points",
                module.path
            );
            for edge in &module.dependency_edges {
                let rendered = format!("{edge:?}");
                assert!(
                    !rendered.contains(module.path.as_str())
                        && rendered.len() <= edge.to.len() + 96,
                    "an edge must hold its target and nothing sized by its own module's path: \
                     {rendered}"
                );
            }
            for entry in &module.entry_points {
                let rendered = format!("{entry:?}");
                assert!(
                    !rendered.contains(module.path.as_str())
                        && rendered.len() <= entry.name.len() + 64,
                    "an entry point must hold its name and line and nothing sized by its \
                     module's path: {rendered}"
                );
            }
        }
        drop(guard);
    }

    /// The bytes one module's edges keep, counted the way the module doc's
    /// "Memory" bound states them: each edge's record, plus, for an
    /// unresolved target (its own allocation), its reference-count header
    /// and its text. A resolved target is shared across the whole map and
    /// counted there, not per edge (see
    /// `tests::ori_t_0036_retained_edge_bytes_are_bounded_for_many_imports_at_a_long_path`,
    /// which checks that it is in fact shared).
    fn retained_edge_bytes(module: &Module) -> usize {
        let header = 2 * std::mem::size_of::<usize>();
        module
            .dependency_edges
            .iter()
            .map(|edge| {
                std::mem::size_of::<DependencyEdge>()
                    + if edge.external {
                        header + edge.to.len()
                    } else {
                        0
                    }
            })
            .sum()
    }

    /// Item 1 (HIGH), the bound: the review's shape, one import name per
    /// two bytes or so of source, past the edge cap, at a path of over 600
    /// bytes, in each of the four languages, with one unresolved target
    /// longer than the target text cap and a thousand edges resolving to
    /// one file. Each module keeps exactly `MAX_DEPENDENCY_EDGES_PER_MODULE`
    /// edges and says it was cut; what those edges keep is summed and must
    /// sit within the stated per-module bound (4,358,144 bytes on a 64-bit
    /// target, the figure the module doc states); the long target is cut to
    /// the cap and marked; and every edge to the one resolved file shares a
    /// single allocation of its path rather than a copy each. The round-5
    /// code kept every edge (4160 here) with a copy of the module's path in
    /// each, plus the resolved target's path copied per edge.
    #[test]
    fn ori_t_0036_retained_edge_bytes_are_bounded_for_many_imports_at_a_long_path() {
        let dir = temp_dir("edge-bytes");
        let guard = DropGuard(dir.clone());
        let long = long_directory();
        let cap = MAX_DEPENDENCY_EDGES_PER_MODULE;
        let flood = cap + 64 - 1001;
        let long_name = "x".repeat(2 * MAX_EDGE_TARGET_BYTES);

        let mut py = format!("from . import {}\n", vec!["b"; 1000].join(", "));
        py.push_str(&format!("import {long_name}{}\n", ", a".repeat(flood)));
        write(&dir, &format!("{long}/m.py"), &py);
        write(&dir, &format!("{long}/b.py"), "x = 1\n");

        let mut rs = format!("use crate::{{{}}};\n", vec!["b"; 1000].join(", "));
        rs.push_str(&format!("use {{{long_name}{}}};\n", ", a".repeat(flood)));
        write(&dir, &format!("{long}/m.rs"), &rs);
        write(&dir, &format!("{long}/lib.rs"), "pub mod b;\n");
        write(&dir, &format!("{long}/b.rs"), "pub struct B;\n");

        let mut ts = "import \"./b\";\n".repeat(1000);
        ts.push_str(&format!("import \"{long_name}\";\n"));
        ts.push_str(&"import \"a\";\n".repeat(flood));
        write(&dir, &format!("{long}/m.ts"), &ts);
        write(&dir, &format!("{long}/b.ts"), "export const b = 1;\n");

        let mut go = format!("package m\n\nimport (\n\"{long_name}\"\n");
        go.push_str(&"\"a\"\n".repeat(cap + 63));
        go.push_str(")\n");
        write(&dir, &format!("{long}/m.go"), &go);

        let map = build_code_map(&dir).expect("maps");
        let record = std::mem::size_of::<DependencyEdge>();
        let header = 2 * std::mem::size_of::<usize>();
        let bound = cap * (record + header + MAX_EDGE_TARGET_BYTES);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            (record, bound),
            (24, 4_358_144),
            "the module doc states a 24-byte edge record and this bound, for a 64-bit target"
        );

        for (file, resolved_target) in [
            ("m.py", Some("b.py")),
            ("m.rs", Some("b.rs")),
            ("m.ts", Some("b.ts")),
            ("m.go", None),
        ] {
            let path = format!("{long}/{file}");
            let module = map
                .modules
                .iter()
                .find(|m| m.path == path)
                .unwrap_or_else(|| panic!("{file} present: {:?}", map.coverage));
            assert!(!module.parsed_with_errors, "{file} parses clean");
            assert_eq!(module.dependency_edges.len(), cap, "{file} keeps the cap");
            assert!(
                module.dependency_edges_truncated,
                "{file} had more edges than it keeps, and must say so"
            );
            let retained = retained_edge_bytes(module);
            assert!(
                retained <= bound,
                "{file}: {retained} bytes of edges kept, over the {bound}-byte bound (the \
                 round-5 shape also copied the {}-byte module path into every edge)",
                path.len()
            );
            let cut: Vec<&DependencyEdge> = module
                .dependency_edges
                .iter()
                .filter(|e| e.to_truncated)
                .collect();
            assert_eq!(cut.len(), 1, "{file}: {cut:?}");
            assert!(
                cut[0].external
                    && cut[0].to.len() == MAX_EDGE_TARGET_BYTES
                    && long_name.starts_with(cut[0].to.as_str()),
                "{file}: the long target is its first {MAX_EDGE_TARGET_BYTES} bytes, marked"
            );
            let resolved: Vec<&DependencyEdge> = module
                .dependency_edges
                .iter()
                .filter(|e| !e.external)
                .collect();
            match resolved_target {
                Some(target) => {
                    assert_eq!(resolved.len(), 1000, "{file}");
                    let expected = format!("{long}/{target}");
                    assert!(
                        resolved
                            .iter()
                            .all(|e| e.to == expected && Arc::ptr_eq(&e.to.0, &resolved[0].to.0)),
                        "{file}: every edge to {target} must share one allocation of its path"
                    );
                }
                None => assert!(resolved.is_empty(), "Go edges are always external"),
            }
        }
        drop(guard);
    }

    /// Item 1 (HIGH), the truncation flag's exact meaning: a module with
    /// exactly `MAX_DEPENDENCY_EDGES_PER_MODULE` edges keeps them all and is
    /// not marked; one more, and it keeps the cap and is marked. The flag
    /// says an edge that exists was dropped, never merely that the cap was
    /// reached.
    #[test]
    fn ori_t_0036_the_edge_cap_marks_a_module_only_when_an_edge_was_dropped() {
        let dir = temp_dir("edge-cap-boundary");
        let guard = DropGuard(dir.clone());
        let cap = MAX_DEPENDENCY_EDGES_PER_MODULE;
        let imports = |count: usize| format!("import {}\n", vec!["a"; count].join(", "));
        write(&dir, "exact.py", &imports(cap));
        write(&dir, "over.py", &imports(cap + 1));
        let map = build_code_map(&dir).expect("maps");
        let find = |path: &str| {
            map.modules
                .iter()
                .find(|m| m.path == path)
                .expect("present")
        };
        let exact = find("exact.py");
        assert_eq!(exact.dependency_edges.len(), cap);
        assert!(!exact.dependency_edges_truncated, "nothing was dropped");
        let over = find("over.py");
        assert_eq!(over.dependency_edges.len(), cap);
        assert!(over.dependency_edges_truncated, "one edge was dropped");
        drop(guard);
    }

    /// Item 1 (HIGH), the rest of what a module keeps: interfaces, entry
    /// points and covering tests are not capped in count, and the module
    /// doc's "Memory" bound states what they can reach instead: a fixed
    /// record each (40, 32 and 24 bytes on a 64-bit target) plus text that
    /// is a piece of the file's own source, at most about 20.5 bytes kept
    /// per byte read, reached by one interface per two bytes. Checked here
    /// on that densest shape in Go and in TypeScript, by summing what the
    /// returned map keeps.
    #[test]
    fn ori_t_0036_retained_interface_bytes_stay_within_the_stated_factor_of_the_file() {
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            (
                std::mem::size_of::<Interface>(),
                std::mem::size_of::<EntryPoint>(),
                std::mem::size_of::<String>(),
            ),
            (40, 32, 24),
            "the record sizes the module doc states, for a 64-bit target"
        );
        let dir = temp_dir("interface-bytes");
        let guard = DropGuard(dir.clone());
        let names = 32 * 1024;
        let go = format!("package m\nvar A{} int\n", ",A".repeat(names - 1));
        let ts = format!("export let a{};\n", ",a".repeat(names - 1));
        write(&dir, "m.go", &go);
        write(&dir, "m.ts", &ts);
        let map = build_code_map(&dir).expect("maps");
        for (path, file_bytes) in [("m.go", go.len()), ("m.ts", ts.len())] {
            let module = map
                .modules
                .iter()
                .find(|m| m.path == path)
                .expect("present");
            assert_eq!(module.interfaces.len(), names, "{path}: one per name");
            let kept: usize = module
                .interfaces
                .iter()
                .map(|i| std::mem::size_of::<Interface>() + i.name.len())
                .sum();
            assert!(
                kept * 2 <= file_bytes * 41 + 82,
                "{path}: {kept} bytes of interfaces kept for a {file_bytes}-byte file, over \
                 20.5 times its size"
            );
        }
        drop(guard);
    }

    /// The first node of `kind` in `tree`, at any depth.
    fn first_of_kind<'t>(tree: &'t tree_sitter::Tree, kind: &str) -> Node<'t> {
        let mut found = None;
        for_each_node(
            tree.root_node(),
            Instant::now() + Duration::from_secs(600),
            |node| {
                if found.is_none() && node.kind() == kind {
                    found = Some(node);
                }
            },
        );
        found.unwrap_or_else(|| panic!("a {kind} node"))
    }

    /// Found while measuring item 1 (HIGH): loops over one statement's
    /// items (or one parent's children, for Rust test detection) that
    /// checked no deadline, or collected every item before their first
    /// check, so one 8 MiB statement ran seconds past its file's budget in
    /// a release build. Each such loop, given one statement of 600,000
    /// items of two to five bytes: with an expired deadline, must extract
    /// nothing and stop at once (within 100 ms, far less than collecting
    /// every item first takes); with a deadline 20 ms away, must stop
    /// within 250 ms and say it did not finish. Timed on the loop alone,
    /// after the parse.
    #[test]
    fn ori_t_0036_one_statement_of_many_items_is_extracted_under_the_deadline() {
        let items = 600_000;
        let go_var = format!("package m\nvar A{} int\n", ",A".repeat(items - 1));
        let go_type = format!("package m\ntype (A int{})\n", ";A int".repeat(items / 3));
        let ts_let = format!("export let a{};\n", ",a".repeat(items - 1));
        let py_import = format!("import a{}\n", ",a".repeat(items - 1));
        let py_from = format!("from . import a{}\n", ",a".repeat(items - 1));
        let rs_attributes = "#[a]\n".repeat(items);
        let cases: [(&str, Language, &str, &str); 6] = [
            ("Go var", Language::Go, &go_var, "var_declaration"),
            ("Go type", Language::Go, &go_type, "type_declaration"),
            (
                "TypeScript let",
                Language::TypeScript,
                &ts_let,
                "lexical_declaration",
            ),
            (
                "Python import",
                Language::Python,
                &py_import,
                "import_statement",
            ),
            (
                "Python from",
                Language::Python,
                &py_from,
                "import_from_statement",
            ),
            ("Rust tests", Language::Rust, &rs_attributes, "source_file"),
        ];
        for (label, language, source, kind) in cases {
            let tree = parse_bounded(
                language,
                false,
                source,
                Instant::now() + Duration::from_secs(600),
            )
            .expect("parses");
            let node = first_of_kind(&tree, kind);
            let run = |deadline: Instant| -> (bool, usize) {
                let mut interfaces = Vec::new();
                let mut sink = EdgeSink::with_limit(usize::MAX);
                let completed = match label {
                    "Go var" | "Go type" => go_declaration_interfaces(
                        node,
                        source.as_bytes(),
                        &mut interfaces,
                        deadline,
                    ),
                    "TypeScript let" => ts_declaration_interfaces(
                        node,
                        source.as_bytes(),
                        &mut interfaces,
                        deadline,
                    ),
                    "Python import" => {
                        python_import_edges(node, source.as_bytes(), &mut sink, deadline)
                    }
                    "Rust tests" => {
                        let (names, completed) = rust_test_names(node, source.as_bytes(), deadline);
                        interfaces.extend(names.into_iter().map(|name| Interface {
                            name,
                            kind: InterfaceKind::Function,
                            line: 0,
                        }));
                        completed
                    }
                    _ => python_import_from_edges(
                        node,
                        source.as_bytes(),
                        "m.py",
                        &KnownPaths::new(),
                        &mut sink,
                        deadline,
                    ),
                };
                (completed, interfaces.len() + sink.edges.len())
            };
            let already_past = Instant::now() - Duration::from_secs(1);
            let start = Instant::now();
            assert_eq!(
                run(already_past),
                (false, 0),
                "{label}: an expired deadline extracts nothing"
            );
            let elapsed = start.elapsed();
            assert!(
                elapsed < Duration::from_millis(100),
                "{label}: with the deadline already past, the loop must stop at its first \
                 item, not after collecting all {items}: it took {elapsed:?}"
            );
            let start = Instant::now();
            let (completed, _) = run(start + Duration::from_millis(20));
            let elapsed = start.elapsed();
            assert!(
                !completed && elapsed < Duration::from_millis(250),
                "{label}: a deadline 20 ms away must stop the loop, but it ran {elapsed:?} \
                 (completed: {completed})"
            );
        }
    }

    /// Found with the test above: the two per-child checks it cannot tell
    /// apart by timing, checked directly with an expired deadline, which
    /// must stop each before its first child. `gather_children` is what
    /// both traversal helpers gather a parent's children with, and
    /// `rust_tests_in_siblings` is Rust test detection's work on one
    /// parent's children once gathered.
    #[test]
    fn ori_t_0036_gathering_and_scanning_children_check_the_deadline_per_child() {
        let source = "#[test]\nfn a() {}\n#[test]\nfn b() {}\n";
        let tree = parse_bounded(
            Language::Rust,
            false,
            source,
            Instant::now() + Duration::from_secs(60),
        )
        .expect("parses");
        let root = tree.root_node();
        let already_past = Instant::now() - Duration::from_secs(1);
        let later = Instant::now() + Duration::from_secs(60);
        assert!(gather_children(root, already_past).is_none());
        let children = gather_children(root, later).expect("time left");
        assert_eq!(children.len(), 4);

        let mut names = Vec::new();
        assert!(!rust_tests_in_siblings(
            &children,
            source.as_bytes(),
            &mut names,
            already_past
        ));
        assert!(names.is_empty(), "{names:?}");
        assert!(rust_tests_in_siblings(
            &children,
            source.as_bytes(),
            &mut names,
            later
        ));
        assert_eq!(names, vec!["a".to_owned(), "b".to_owned()]);
    }

    /// Item 2 (MEDIUM): the `spec/` scan descended directories named `.git`,
    /// which the main walk never does, so a nested clone at `spec/` had
    /// git's own files read, their headings kept and their text cited, and
    /// the scan still reported itself complete. The scan now applies the
    /// walk's rule, records each such directory as `GitMetadata` (making the
    /// scan `Partial`, since something under `spec/` was passed over), and
    /// reads everything else as before. A submodule's `.git` *file*, which
    /// holds no documents, is neither read nor recorded. The round-5 code
    /// fails this.
    #[test]
    fn ori_t_0036_the_spec_scan_never_descends_a_git_directory() {
        let dir = temp_dir("spec-git");
        let guard = DropGuard(dir.clone());
        write(&dir, "a.rs", "pub fn a() {}\n");
        // Assembled, not written whole: see the fixture-path comment earlier
        // in this file.
        let readme = format!("spec/{}.md", "readme");
        let kept = format!("spec/deep/{}.md", "kept");
        write(&dir, &readme, "# Readme\n\nsee a.rs\n");
        write(&dir, &kept, "# Kept\na.rs\n");
        write(
            &dir,
            &format!("spec/.git/info/{}.md", "notes"),
            "# Inside nested .git metadata\nsee a.rs\n",
        );
        write(
            &dir,
            &format!("spec/deep/.git/{}.md", "x"),
            "# Also inside a .git\na.rs\n",
        );
        write(&dir, "spec/sub/.git", "gitdir: ../../.git/modules/sub\n");
        let map = build_code_map(&dir).expect("maps");
        assert_eq!(map.spec_docs, vec![kept.clone(), readme.clone()]);
        let headings: Vec<&str> = map.spec_headings.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(headings, vec!["Kept", "Readme"]);
        let a = map
            .modules
            .iter()
            .find(|m| m.path == "a.rs")
            .expect("present");
        let cited: Vec<&str> = a
            .spec_sections
            .iter()
            .filter_map(|citation| map.spec_doc(citation))
            .collect();
        assert_eq!(cited, vec![kept.as_str(), readme.as_str()]);
        assert_eq!(
            map.coverage.spec_docs_skipped,
            vec![
                SkippedFile {
                    path: "spec/.git".to_owned(),
                    reason: SkipReason::GitMetadata,
                },
                SkippedFile {
                    path: "spec/deep/.git".to_owned(),
                    reason: SkipReason::GitMetadata,
                },
            ]
        );
        assert_eq!(map.coverage.spec_citation_scan, SpecScan::Partial);
        drop(guard);
    }
}
