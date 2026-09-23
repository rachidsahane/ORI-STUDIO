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
//! # Untrusted input, and the bounds this module sets
//!
//! The repository being mapped may be a user's own, so its content is
//! untrusted input in AICD §8's "sanitization barrier" sense, even though the
//! barrier itself (typed records, provenance, length caps) governs layer 4
//! telemetry, not this module. What carries over is the discipline: nothing
//! here executes a file, runs a build script, or shells out. The bounds
//! actually enforced:
//!
//! - **Size.** [`CodeMapOptions::max_file_bytes`] (default 8 MiB) caps how
//!   much of any one file is read before it is parsed; larger files are
//!   [`SkipReason::TooLarge`] and never opened for their full content beyond
//!   the `stat` that measured them.
//! - **Time.** [`CodeMapOptions::parse_timeout`] (default 5 seconds) bounds
//!   how long tree-sitter is given to parse one file, via
//!   [`tree_sitter::Parser::parse_with_options`]'s progress callback, which
//!   tree-sitter polls periodically during parsing. A file whose syntax is
//!   small but pathologically nested (deeply bracketed input is the classic
//!   case) is bounded by time even when it is not bounded by size, and a
//!   parse tree-sitter cannot finish in time is [`SkipReason::TimedOut`], not
//!   a hang.
//! - **Symlinks.** A directory reached through a symlink is never descended
//!   into, whatever its target, which makes cycle-safety independent of where
//!   the link points (a symlink loop cannot be walked into in the first
//!   place, so no separate loop detector is needed). A symlinked *file* is
//!   read only when [`std::fs::canonicalize`] resolves it to a path inside the
//!   repository root; outside, it is [`SkipReason::SymlinkOutsideRoot`] and is
//!   never opened.
//! - **Traversal.** The directory walk is iterative (an explicit stack of
//!   pending directories), never recursive, so a pathologically deep
//!   directory tree cannot overflow the call stack the way a naive recursive
//!   walker would. A directory literally named `.git` is not descended into
//!   at any depth, because it is version-control metadata, never source; this
//!   is the one filesystem convention this module hard-codes, and it is
//!   documented here because it is a real exclusion, not an oversight (a repo
//!   that keeps source inside a directory named `.git` is not one this module
//!   claims to map, and none does).
//! - **What is not bounded.** There is no cap on the total number of files or
//!   total bytes walked, and no `.gitignore` is honored: a `target/` or
//!   `node_modules/` directory is walked like any other, its files seen,
//!   parsed if their extension matches, and reported. A repository whose
//!   build output dwarfs its source will have that reflected honestly in
//!   [`Coverage`] rather than hidden by a heuristic this module does not
//!   implement. Both are named as gaps in this ticket's closing report, not
//!   silently assumed away.
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
//! under it (bounded by the same [`CodeMapOptions::max_file_bytes`], read
//! best effort, an unreadable or oversized document silently contributing
//! nothing rather than failing the map) is scanned for the module's own path
//! exactly as this map records it (root-relative, forward slashes) appearing
//! as a literal substring anywhere in the text, and each match is recorded as
//! a [`SpecCitation`] naming the document and the nearest preceding Markdown
//! heading, if any. This is deliberately narrower than AICD §25's rule: a
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

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
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
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DependencyEdge {
    /// The importing module's path.
    pub from: String,
    /// When [`DependencyEdge::external`] is `false`, another module's
    /// [`Module::path`] in this same [`CodeMap`]. Otherwise, the import
    /// target exactly as written in the source (a crate name, a bare
    /// specifier, a dotted absolute import, a Go import path), because it
    /// could not be resolved to a file in the mapped repository.
    pub to: String,
    /// Whether `to` names something outside the mapped repository (or
    /// something this module's resolver did not attempt, see the module
    /// doc's per-language notes; Go edges are always external).
    pub external: bool,
}

/// One entry point: a place a program starts running.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EntryPoint {
    /// The module it is in.
    pub module: String,
    /// `main` (Rust, Go, or a top-level TypeScript function so named),
    /// `__main__` (a Python `if __name__ == "__main__":` guard), or the
    /// literal first line of a shebang (`#!...`), which is checked for every
    /// language uniformly.
    pub name: String,
    /// The 1-based line it starts on.
    pub line: usize,
}

/// One citation of a module's path found in the mapped repository's own
/// `spec/` documents. See the module doc, "What spec sections per module
/// means here, and what it misses".
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SpecCitation {
    /// The document's path, relative to the mapped root.
    pub doc: String,
    /// The nearest Markdown heading before the citation, if the document has
    /// one above it.
    pub heading: Option<String>,
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
    /// The module's outgoing dependency edges, sorted by (external, to).
    pub dependency_edges: Vec<DependencyEdge>,
    /// The module's entry points, sorted by (line, name).
    pub entry_points: Vec<EntryPoint>,
    /// The tests found to cover this module, by the rule the module doc
    /// states. Sorted and deduplicated.
    pub covering_tests: Vec<String>,
    /// The specification sections found to cite this module's path. Sorted
    /// by (doc, heading).
    pub spec_sections: Vec<SpecCitation>,
}

/// Why one candidate file was not parsed into a [`Module`].
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SkipReason {
    /// Its extension is not one this module maps to a language.
    UnsupportedLanguage,
    /// It is larger than [`CodeMapOptions::max_file_bytes`].
    TooLarge {
        /// Its size, in bytes.
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
    /// Tree-sitter did not finish parsing it within
    /// [`CodeMapOptions::parse_timeout`]; see the module doc's "Time" bound.
    TimedOut,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLanguage => f.write_str("unsupported language"),
            Self::TooLarge { bytes, cap } => write!(f, "too large ({bytes} bytes > {cap} cap)"),
            Self::Unreadable(reason) => write!(f, "unreadable: {reason}"),
            Self::Binary => f.write_str("binary (or not valid UTF-8)"),
            Self::SymlinkOutsideRoot => f.write_str("symlink outside the repository root"),
            Self::TimedOut => f.write_str("parse timed out"),
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
/// checks it). A map is never presented as complete while
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
}

/// The result of [`build_code_map`]: every module found, and what the walk
/// covered.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeMap {
    /// Every module found, sorted by path.
    pub modules: Vec<Module>,
    /// What the walk saw, parsed or not.
    pub coverage: Coverage,
}

/// The bounds [`build_code_map_with_options`] enforces. See the module doc's
/// "Untrusted input" section for why each exists.
#[derive(Clone, Copy, Debug)]
pub struct CodeMapOptions {
    /// The largest file this module will read fully and parse. Default 8
    /// MiB.
    pub max_file_bytes: u64,
    /// The longest tree-sitter is given to parse one file. Default 5
    /// seconds.
    pub parse_timeout: Duration,
}

impl Default for CodeMapOptions {
    fn default() -> Self {
        Self {
            max_file_bytes: 8 * 1024 * 1024,
            parse_timeout: Duration::from_secs(5),
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

    let walk = walk_repository(&root_canon);
    let files_seen = walk.files.len() + walk.pre_skipped.len();
    let known: HashSet<String> = walk.files.iter().map(|file| file.rel.clone()).collect();

    let mut modules: Vec<Module> = Vec::new();
    let mut skipped: Vec<SkippedFile> = walk.pre_skipped;
    let mut files_parsed_clean = 0usize;
    let mut files_parsed_with_errors = 0usize;

    for file in &walk.files {
        match process_file(file, &known, options) {
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
    attach_spec_citations(&root_canon, &mut modules, options);

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
        module
            .spec_sections
            .sort_by(|a, b| (&a.doc, &a.heading).cmp(&(&b.doc, &b.heading)));
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
        },
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
        let Ok(read_dir) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<fs::DirEntry> = read_dir.filter_map(std::result::Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);

        for entry in entries {
            let name = entry.file_name();
            let abs = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_symlink() {
                match fs::metadata(&abs) {
                    Ok(target_meta) if target_meta.is_dir() => {
                        // Never descended: see the module doc's cycle-safety note.
                    }
                    Ok(_) => match fs::canonicalize(&abs) {
                        Ok(resolved) if resolved.starts_with(root_canon) => {
                            if let Some(rel) = rel_path_string(root_canon, &resolved) {
                                files.push(CandidateFile { abs, rel });
                            }
                        }
                        _ => {
                            if let Some(rel) = rel_path_string(root_canon, &abs) {
                                pre_skipped.push(SkippedFile {
                                    path: rel,
                                    reason: SkipReason::SymlinkOutsideRoot,
                                });
                            }
                        }
                    },
                    Err(err) => {
                        if let Some(rel) = rel_path_string(root_canon, &abs) {
                            pre_skipped.push(SkippedFile {
                                path: rel,
                                reason: SkipReason::Unreadable(err.to_string()),
                            });
                        }
                    }
                }
            } else if file_type.is_dir() {
                if name == ".git" {
                    continue;
                }
                pending.push(abs);
            } else if file_type.is_file() {
                if let Some(rel) = rel_path_string(root_canon, &abs) {
                    files.push(CandidateFile { abs, rel });
                }
            } else if let Some(rel) = rel_path_string(root_canon, &abs) {
                pre_skipped.push(SkippedFile {
                    path: rel,
                    reason: SkipReason::Unreadable("not a regular file".to_owned()),
                });
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

// ---------------------------------------------------------------------------
// Per-file processing
// ---------------------------------------------------------------------------

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

/// Turns one candidate file into a [`Module`], or the [`SkipReason`] it was
/// skipped for.
fn process_file(
    file: &CandidateFile,
    known: &HashSet<String>,
    options: &CodeMapOptions,
) -> std::result::Result<Module, SkipReason> {
    let language = language_of(&file.rel).ok_or(SkipReason::UnsupportedLanguage)?;

    let meta = fs::metadata(&file.abs).map_err(|err| SkipReason::Unreadable(err.to_string()))?;
    if meta.len() > options.max_file_bytes {
        return Err(SkipReason::TooLarge {
            bytes: meta.len(),
            cap: options.max_file_bytes,
        });
    }

    let bytes = fs::read(&file.abs).map_err(|err| SkipReason::Unreadable(err.to_string()))?;
    if bytes.contains(&0u8) {
        return Err(SkipReason::Binary);
    }
    let Ok(source) = std::str::from_utf8(&bytes) else {
        return Err(SkipReason::Binary);
    };

    let tsx = file.rel.ends_with(".tsx");
    let tree =
        parse_bounded(language, tsx, source, options.parse_timeout).ok_or(SkipReason::TimedOut)?;
    let root = tree.root_node();
    let parsed_with_errors = root.has_error();

    let mut extracted = match language {
        Language::Rust => extract_rust(root, source.as_bytes(), &file.rel, known),
        Language::TypeScript => extract_typescript(root, source.as_bytes(), &file.rel, known),
        Language::Python => extract_python(root, source.as_bytes(), &file.rel, known),
        Language::Go => extract_go(root, source.as_bytes(), &file.rel),
    };

    if let Some(shebang) = shebang_entry_point(&file.rel, source) {
        extracted.entry_points.push(shebang);
    }

    let covering_tests = match language {
        Language::Rust => rust_test_names(root, source.as_bytes()),
        Language::TypeScript => typescript_test_names(root, source.as_bytes()),
        Language::Python => python_test_names(root, source.as_bytes()),
        Language::Go => go_test_names(root, source.as_bytes()),
    };

    Ok(Module {
        path: file.rel.clone(),
        language,
        parsed_with_errors,
        interfaces: extracted.interfaces,
        dependency_edges: extracted.edges,
        entry_points: extracted.entry_points,
        covering_tests,
        spec_sections: Vec::new(),
    })
}

/// Parses `source` with a wall-clock deadline, returning `None` when
/// tree-sitter's own progress callback cancels the parse before it finishes.
/// See the module doc's "Time" bound.
fn parse_bounded(
    language: Language,
    tsx: bool,
    source: &str,
    timeout: Duration,
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
    let deadline = Instant::now() + timeout;
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
fn shebang_entry_point(rel: &str, source: &str) -> Option<EntryPoint> {
    if !source.starts_with("#!") {
        return None;
    }
    let line = source.lines().next().unwrap_or("#!").to_owned();
    Some(EntryPoint {
        module: rel.to_owned(),
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
fn for_each_node<'a>(root: Node<'a>, mut visit: impl FnMut(Node<'a>)) {
    let mut stack: Vec<Node<'a>> = vec![root];
    while let Some(node) = stack.pop() {
        visit(node);
        let mut cursor = node.walk();
        let mut children: Vec<Node<'a>> = node.children(&mut cursor).collect();
        children.reverse();
        stack.extend(children);
    }
}

/// What one language's top-level scan produced.
struct Extracted {
    interfaces: Vec<Interface>,
    edges: Vec<DependencyEdge>,
    entry_points: Vec<EntryPoint>,
}

impl Extracted {
    fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            edges: Vec::new(),
            entry_points: Vec::new(),
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

fn resolve_rust_mod(rel: &str, name: &str, known: &HashSet<String>) -> Option<String> {
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
    let as_file = format!("{base}.rs");
    if known.contains(&as_file) {
        return Some(as_file);
    }
    let as_mod = format!("{base}/mod.rs");
    if known.contains(&as_mod) {
        return Some(as_mod);
    }
    None
}

fn resolve_rust_crate_path(segments: &[&str], known: &HashSet<String>) -> Option<String> {
    for take in (1..=segments.len()).rev() {
        let base = segments[..take].join("/");
        let as_file = format!("{base}.rs");
        if known.contains(&as_file) {
            return Some(as_file);
        }
        let as_mod = format!("{base}/mod.rs");
        if known.contains(&as_mod) {
            return Some(as_mod);
        }
    }
    None
}

fn extract_rust(root: Node, source: &[u8], rel: &str, known: &HashSet<String>) -> Extracted {
    let mut out = Extracted::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "mod_item" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = text(name_node, source);
                let is_declaration = child.child_by_field_name("body").is_none();
                if is_declaration {
                    let resolved = resolve_rust_mod(rel, name, known);
                    out.edges.push(DependencyEdge {
                        from: rel.to_owned(),
                        to: resolved.clone().unwrap_or_else(|| format!("mod {name}")),
                        external: resolved.is_none(),
                    });
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
                let raw = text(argument, source);
                let head = raw
                    .split(['{', '*'])
                    .next()
                    .unwrap_or(raw)
                    .trim_end_matches("::");
                if let Some(after_crate) =
                    head.strip_prefix("crate::")
                        .or(if head == "crate" { Some("") } else { None })
                {
                    let segments: Vec<&str> =
                        after_crate.split("::").filter(|s| !s.is_empty()).collect();
                    let resolved = resolve_rust_crate_path(&segments, known);
                    out.edges.push(DependencyEdge {
                        from: rel.to_owned(),
                        to: resolved.clone().unwrap_or_else(|| raw.to_owned()),
                        external: resolved.is_none(),
                    });
                } else {
                    out.edges.push(DependencyEdge {
                        from: rel.to_owned(),
                        to: raw.to_owned(),
                        external: true,
                    });
                }
            }
            "function_item" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = text(name_node, source);
                if name == "main" {
                    out.entry_points.push(EntryPoint {
                        module: rel.to_owned(),
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
    out
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

fn rust_marked_test(item: Node, source: &[u8]) -> bool {
    let mut candidate = item.prev_sibling();
    while let Some(node) = candidate {
        if node.kind() != "attribute_item" {
            break;
        }
        if rust_attribute_last_segment(node, source).as_deref() == Some("test") {
            return true;
        }
        candidate = node.prev_sibling();
    }
    false
}

fn rust_test_names(root: Node, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for_each_node(root, |node| {
        if node.kind() == "function_item"
            && rust_marked_test(node, source)
            && let Some(name_node) = node.child_by_field_name("name")
        {
            names.push(text(name_node, source).to_owned());
        }
    });
    names.sort();
    names.dedup();
    names
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

fn resolve_ts_relative(rel: &str, specifier: &str, known: &HashSet<String>) -> Option<String> {
    let base = dir_components(rel);
    let joined = normalize_join(&base, specifier)?;
    [
        format!("{joined}.ts"),
        format!("{joined}.tsx"),
        format!("{joined}/index.ts"),
        format!("{joined}/index.tsx"),
    ]
    .into_iter()
    .find(|candidate| known.contains(candidate))
}

fn ts_import_edge(rel: &str, specifier: &str, known: &HashSet<String>) -> DependencyEdge {
    if specifier.starts_with("./") || specifier.starts_with("../") {
        let resolved = resolve_ts_relative(rel, specifier, known);
        DependencyEdge {
            from: rel.to_owned(),
            to: resolved.clone().unwrap_or_else(|| specifier.to_owned()),
            external: resolved.is_none(),
        }
    } else {
        DependencyEdge {
            from: rel.to_owned(),
            to: specifier.to_owned(),
            external: true,
        }
    }
}

fn ts_declaration_interfaces(declaration: Node, source: &[u8]) -> Vec<Interface> {
    let mut items = Vec::new();
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
    items
}

fn extract_typescript(root: Node, source: &[u8], rel: &str, known: &HashSet<String>) -> Extracted {
    let mut out = Extracted::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                if let Some(source_node) = child.child_by_field_name("source") {
                    let specifier = string_literal_text(source_node, source);
                    out.edges.push(ts_import_edge(rel, specifier, known));
                }
            }
            "export_statement" => {
                if let Some(source_node) = child.child_by_field_name("source") {
                    let specifier = string_literal_text(source_node, source);
                    out.edges.push(ts_import_edge(rel, specifier, known));
                }
                if let Some(declaration) = child.child_by_field_name("declaration") {
                    out.interfaces
                        .extend(ts_declaration_interfaces(declaration, source));
                    if declaration.kind() == "function_declaration"
                        && let Some(name) = declaration.child_by_field_name("name")
                        && text(name, source) == "main"
                    {
                        out.entry_points.push(EntryPoint {
                            module: rel.to_owned(),
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
                        module: rel.to_owned(),
                        name: "main".to_owned(),
                        line: line_of(child),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn typescript_test_names(root: Node, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for_each_node(root, |node| {
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
    names
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

fn python_import_from_edges(
    node: Node,
    source: &[u8],
    rel: &str,
    known: &HashSet<String>,
) -> Vec<DependencyEdge> {
    let mut edges = Vec::new();
    let Some(module_name) = node.child_by_field_name("module_name") else {
        return edges;
    };

    if module_name.kind() == "relative_import" {
        let mut cursor = module_name.walk();
        let dots = module_name
            .children(&mut cursor)
            .find(|child| child.kind() == "import_prefix")
            .map(|prefix| text(prefix, source).chars().count())
            .unwrap_or(1);
        let dotted = {
            let mut cursor = module_name.walk();
            module_name
                .children(&mut cursor)
                .find(|child| child.kind() == "dotted_name")
                .map(|dotted_name| text(dotted_name, source).to_owned())
        };

        let mut base = dir_components(rel);
        for _ in 0..dots.saturating_sub(1) {
            base.pop();
        }

        if let Some(dotted) = dotted {
            let mut target_dir = base;
            target_dir.extend(dotted.split('.').map(str::to_owned));
            let joined = target_dir.join("/");
            let resolved = [format!("{joined}.py"), format!("{joined}/__init__.py")]
                .into_iter()
                .find(|candidate| known.contains(candidate));
            edges.push(DependencyEdge {
                from: rel.to_owned(),
                to: resolved.clone().unwrap_or_else(|| format!(".{dotted}")),
                external: resolved.is_none(),
            });
        } else {
            let names: Vec<Node> = node
                .children_by_field_name("name", &mut node.walk())
                .collect();
            for name_node in names {
                let name_text = if name_node.kind() == "aliased_import" {
                    name_node
                        .child_by_field_name("name")
                        .map(|n| text(n, source).to_owned())
                } else {
                    Some(text(name_node, source).to_owned())
                };
                let Some(name_text) = name_text else { continue };
                let mut target_dir = base.clone();
                target_dir.push(name_text.clone());
                let joined = target_dir.join("/");
                let resolved = [format!("{joined}.py"), format!("{joined}/__init__.py")]
                    .into_iter()
                    .find(|candidate| known.contains(candidate));
                let dots_text = ".".repeat(dots);
                edges.push(DependencyEdge {
                    from: rel.to_owned(),
                    to: resolved
                        .clone()
                        .unwrap_or_else(|| format!("{dots_text}{name_text}")),
                    external: resolved.is_none(),
                });
            }
        }
    } else {
        edges.push(DependencyEdge {
            from: rel.to_owned(),
            to: text(module_name, source).to_owned(),
            external: true,
        });
    }

    edges
}

fn extract_python(root: Node, source: &[u8], rel: &str, known: &HashSet<String>) -> Extracted {
    let mut out = Extracted::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                let names: Vec<Node> = child
                    .children_by_field_name("name", &mut child.walk())
                    .collect();
                for name_node in names {
                    let name_text = if name_node.kind() == "aliased_import" {
                        name_node
                            .child_by_field_name("name")
                            .map(|n| text(n, source).to_owned())
                    } else {
                        Some(text(name_node, source).to_owned())
                    };
                    if let Some(name_text) = name_text {
                        out.edges.push(DependencyEdge {
                            from: rel.to_owned(),
                            to: name_text,
                            external: true,
                        });
                    }
                }
            }
            "import_from_statement" => {
                out.edges
                    .extend(python_import_from_edges(child, source, rel, known));
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
                            module: rel.to_owned(),
                            name: "__main__".to_owned(),
                            line: line_of(child),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn python_test_names(root: Node, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for_each_node(root, |node| {
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
    names
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

fn go_import_spec_edges(import_declaration: Node, source: &[u8], rel: &str) -> Vec<DependencyEdge> {
    let mut edges = Vec::new();
    for_each_node(import_declaration, |node| {
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
                || text(path_node, source).trim_matches('"').to_owned(),
                |content| text(content, source).to_owned(),
            );
        edges.push(DependencyEdge {
            from: rel.to_owned(),
            to: content,
            external: true,
        });
    });
    edges
}

fn go_exported(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn extract_go(root: Node, source: &[u8], rel: &str) -> Extracted {
    let mut out = Extracted::new();
    let mut package_name = String::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
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
                out.edges.extend(go_import_spec_edges(child, source, rel));
            }
            "function_declaration" => {
                let Some(name_node) = child.child_by_field_name("name") else {
                    continue;
                };
                let name = text(name_node, source);
                if name == "main" && package_name == "main" {
                    out.entry_points.push(EntryPoint {
                        module: rel.to_owned(),
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
            "type_declaration" => {
                let mut inner_cursor = child.walk();
                for spec in child
                    .children(&mut inner_cursor)
                    .filter(|n| n.kind() == "type_spec")
                {
                    if let Some(name_node) = spec.child_by_field_name("name") {
                        let name = text(name_node, source);
                        if go_exported(name) {
                            out.interfaces.push(Interface {
                                name: name.to_owned(),
                                kind: InterfaceKind::Type,
                                line: line_of(spec),
                            });
                        }
                    }
                }
            }
            "const_declaration" | "var_declaration" => {
                let mut inner_cursor = child.walk();
                for spec in child
                    .children(&mut inner_cursor)
                    .filter(|n| n.kind() == "const_spec" || n.kind() == "var_spec")
                {
                    let names: Vec<Node> = spec
                        .children_by_field_name("name", &mut spec.walk())
                        .collect();
                    for name_node in names {
                        let name = text(name_node, source);
                        if go_exported(name) {
                            out.interfaces.push(Interface {
                                name: name.to_owned(),
                                kind: InterfaceKind::Constant,
                                line: line_of(spec),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn go_test_names(root: Node, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for_each_node(root, |node| {
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
    names
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

/// Scans `<root>/spec/**/*.md`, best effort, for each module's path as a
/// literal substring. See the module doc, "What spec sections per module
/// means here".
fn attach_spec_citations(root: &Path, modules: &mut [Module], options: &CodeMapOptions) {
    let spec_dir = root.join("spec");
    if !spec_dir.is_dir() {
        return;
    }

    let docs = collect_markdown(&spec_dir, root, options);
    for module in modules.iter_mut() {
        for (doc_rel, content) in &docs {
            let mut heading: Option<String> = None;
            for line in content.lines() {
                let trimmed = line.trim_start();
                if let Some(stripped) = trimmed.strip_prefix('#') {
                    let stripped = stripped.trim_start_matches('#').trim();
                    if !stripped.is_empty() {
                        heading = Some(stripped.to_owned());
                    }
                }
                if line.contains(module.path.as_str()) {
                    module.spec_sections.push(SpecCitation {
                        doc: doc_rel.clone(),
                        heading: heading.clone(),
                    });
                }
            }
        }
    }
}

/// Walks `dir` (inside `root`) for `.md` files, following no symlinked
/// directory, silently skipping what cannot be read within
/// [`CodeMapOptions::max_file_bytes`]: this is a best-effort secondary scan,
/// not part of [`Coverage`].
fn collect_markdown(dir: &Path, root: &Path, options: &CodeMapOptions) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(read_dir) = fs::read_dir(&current) else {
            continue;
        };
        let mut entries: Vec<fs::DirEntry> = read_dir.filter_map(std::result::Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            if meta.len() > options.max_file_bytes {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(rel) = rel_path_string(root, &path) {
                out.push((rel, content));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
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
            foo.spec_sections.iter().any(|c| c.doc == "spec/LLD.md"
                && c.heading.as_deref() == Some("2. Crate responsibilities")),
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
}
