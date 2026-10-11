//! The drift audit: AICD §9, AICD §25.
//!
//! `spec/PRD.md` section 4.3, row D-09, asks for a "drift audit (weekly and on
//! demand): documentation agent compares canonical documents with code, updates
//! verification dates, files divergences; stale documents block readiness", and
//! section 4.13, row K-07, for "freshness tracker and staleness exposure; drift
//! audit integration". AICD §9 gives the rule ("Every document carries a 'last
//! verified against code' date maintained by the drift audit") and AICD §25 the
//! tracker that holds it. `spec/LLD.md` section 2 gives `ori-memory` a
//! `DriftAudit`, and this is it. The tracker is `crate::freshness`; this module
//! is the thing that fills it.
//!
//! ```mermaid
//! flowchart LR
//!   D[canonical documents: path to text] --> A[DriftAudit::run]
//!   V[CodeView: the files the code has] --> A
//!   A -->|no divergence| VER[record_verification: the date moves]
//!   A -->|dangling reference| DIV[record_divergence: the document is stale]
//!   VER --> T[FreshnessTracker]
//!   DIV --> T
//!   T -->|list_stale| S[StaleReport]
//!   S -->|is_ready| RD[readiness, ORI-T-0061]
//! ```
//!
//! # What "compares canonical documents with code" means here
//!
//! The comparison AICD §8 describes, "the documentation agent compares the
//! canonical documents with the actual code and flags every divergence to a
//! human", is semantic, and a semantic comparison is the documentation agent's.
//! What the engine can do mechanically in phase 1, and what this module does, is
//! **reference drift**: a canonical document names a path of this repository in
//! a code span, and the code does not have that path.
//!
//! A *path claim* is an inline code span (a pair of single backticks, outside a
//! fenced block) that:
//!
//! - contains a `/` and no blank, does not start with `/` or `~`, and has none
//!   of `< > * ? { } [ ] ( ) $ | = , ; "` (those are placeholders, patterns and
//!   commands);
//! - starts, after an optional `./`, with a name the code has at its top level
//!   (`crates`, `spec`, `scripts`, whatever the [`crate::drift::CodeView`]
//!   holds), so `ledger/src/entry.rs` in a document about another product is not
//!   a claim about this one;
//! - names no `..` segment and no empty segment.
//!
//! A trailing line suffix (`:12`, `:12-30`), an anchor (`#section`) and a
//! trailing `/` are stripped before the lookup. The path is then looked up in the
//! view: a file, or a directory that holds a file, is *present*; a path under an
//! entry the code's walk did not look inside is *unverifiable*, neither present
//! nor missing; anything else is *missing*, and a missing path is a divergence.
//!
//! The divergence is filed as the document's own, with the paths in its
//! description: `ori readiness` lists the stale document with it
//! (criterion ORI-P1-026). A document with no divergence and no unverifiable path
//! is verified at the audit's time. A document with an unverifiable path is
//! neither: it keeps whatever the tracker already holds for it, so a document
//! that was never verified stays stale, because a document nobody could check is
//! never fresh (`crate::freshness`, "Freshness unknown is never fresh").
//!
//! ## What this does not catch
//!
//! - Behaviour: a document that says the indexer returns ten results when it
//!   returns twenty. The documentation agent's weekly pass is where that is
//!   found; it files it through [`crate::drift::DriftAudit::file_finding`].
//! - Names other than paths: types, functions, flags, events, error codes, table
//!   columns, numbers, thresholds.
//! - A path written outside a single-backtick span, in a fenced block, with a
//!   blank in it (`spec/design/Ori Studio.html` is one), or as a root-level file
//!   name with no `/` (`Cargo.toml`).
//! - A path that exists and is the wrong file, or a line suffix past the end of
//!   its file.
//! - A document that is wrong and names no path: it is verified, and the report
//!   says how many references were checked for it, which may be none.
//! - An empty directory, which is not in a list of files. A cited directory
//!   that holds nothing reads as missing.
//! - A document that describes a path that is planned and not yet built. The
//!   audit cannot tell intent from error, and does not try: it files the
//!   divergence, and a human resolves it, as AICD §24.3's divergence register
//!   does ("the code is right and the intent updates, or the intent is right and
//!   a ticket is created").
//!
//! # How the result reaches readiness
//!
//! Only through the tracker. [`crate::drift::DriftAudit::run`] takes a
//! `&mut FreshnessTracker` and records into it; readiness (ORI-T-0061) calls
//! `FreshnessTracker::list_stale` and then `StaleReport::is_ready`, and nothing
//! else. The [`crate::drift::DriftReport`] this module returns says what was
//! recorded and why, for a human or an event, and deliberately carries no
//! readiness verdict of its own: a second answer here is a second way to be
//! wrong, and the lead has recorded that readiness must never re-derive its
//! answer from the stale list.
//!
//! This module writes no file and appends no event. The tracker is in memory;
//! persisting it, and the audit's report, is the caller's, through the event log.
//!
//! # Refusals
//!
//! An audit that compared nothing must not read as a clean audit
//! ([`crate::drift::DriftError::NothingToAudit`], [`crate::drift::DriftError::EmptyCodeView`]),
//! and a finding with no description would list a stale document with no
//! divergence ([`crate::drift::DriftError::EmptyFinding`]). Every error carries a
//! [`ori_core::error::MethodologyRef`] to AICD §9, held by a test that resolves
//! each one in the real index (criterion ORI-P1-033).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::Timestamp;

use crate::code_map::CodeMap;
use crate::code_map::SkipReason;
use crate::freshness::FreshnessTracker;

/// The most missing paths one divergence description lists; the rest are
/// counted.
const DESCRIBED_PATHS: usize = 12;

/// The reference every refusal of this module carries: AICD §9, whose rule is
/// that the drift audit maintains each document's verification date.
const fn drift_audit_ref() -> MethodologyRef {
    MethodologyRef {
        section: 9,
        subsection: None,
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why an audit was refused: AICD §9.
///
/// Every variant carries [`DriftError::methodology_ref`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DriftError {
    /// No document was given, so nothing was compared and no result can be
    /// reported.
    NothingToAudit,
    /// The code view holds no file, so every path would read as missing, or no
    /// path could be checked.
    EmptyCodeView,
    /// A finding was filed with a blank path or a blank description.
    EmptyFinding {
        /// Which of the two was blank.
        field: &'static str,
    },
}

impl DriftError {
    /// The methodology section this refusal rests on: AICD §9.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        drift_audit_ref()
    }
}

impl fmt::Display for DriftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToAudit => f.write_str(
                "no document was given to the drift audit, so nothing was compared with the code",
            ),
            Self::EmptyCodeView => f.write_str(
                "the code view holds no file, so there is nothing to compare the documents with",
            ),
            Self::EmptyFinding { field } => write!(
                f,
                "a divergence cannot be filed with a blank {field}: the stale document would be \
                 listed with no divergence"
            ),
        }
    }
}

impl std::error::Error for DriftError {}

// ---------------------------------------------------------------------------
// The code view
// ---------------------------------------------------------------------------

/// What a path lookup in the code found: AICD §9.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathStatus {
    /// A file, or a directory holding a file.
    Present,
    /// Not in the code.
    Missing,
    /// Under an entry the walk that built the view did not look inside, so the
    /// view cannot say.
    Unverifiable,
}

/// The files the code has: AICD §9. What documents are compared with.
///
/// Built from paths ([`CodeView::from_paths`]) or from a code map
/// ([`CodeView::from_code_map`]). It holds names only and reads no file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodeView {
    files: BTreeSet<String>,
    unlisted: BTreeSet<String>,
}

impl CodeView {
    /// A view of exactly these repository-relative paths, `/`-separated. The
    /// caller asserts they are every file of the tree.
    #[must_use]
    pub fn from_paths<I, S>(paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            files: paths
                .into_iter()
                .map(|path| normalise(path.as_ref()))
                .filter(|path| !path.is_empty())
                .collect(),
            unlisted: BTreeSet::new(),
        }
    }

    /// Marks `path` as an entry whose contents the walk did not look inside: a
    /// reference under it is unverifiable, not missing.
    #[must_use]
    pub fn with_unlisted(mut self, path: &str) -> Self {
        let path = normalise(path);
        if !path.is_empty() {
            self.files.insert(path.clone());
            self.unlisted.insert(path);
        }
        self
    }

    /// A view of every file the code map saw: each module it parsed and each
    /// file it saw and did not parse (a skipped file still exists).
    ///
    /// A skipped entry the walk may not have looked inside (a directory it
    /// timed out on or would not follow, an unreadable entry, any reason this
    /// module does not know) is marked unlisted, so a path under it is
    /// unverifiable rather than wrongly missing.
    #[must_use]
    pub fn from_code_map(map: &CodeMap) -> Self {
        let mut view = Self::from_paths(map.modules.iter().map(|module| module.path.as_str()));
        for skipped in &map.coverage.files_skipped {
            view = match skipped.reason {
                SkipReason::UnsupportedLanguage
                | SkipReason::TooLarge { .. }
                | SkipReason::Binary
                | SkipReason::NonUtf8Name
                | SkipReason::NotARegularFile
                | SkipReason::SpecCorpusOverBudget { .. }
                | SkipReason::SameFileAs { .. } => {
                    let path = normalise(&skipped.path);
                    if !path.is_empty() {
                        view.files.insert(path);
                    }
                    view
                }
                _ => view.with_unlisted(&skipped.path),
            };
        }
        view
    }

    /// Whether the view holds no file.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// How many files the view holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the code has a top-level entry named `name`.
    fn has_top_level(&self, name: &str) -> bool {
        let directory = format!("{name}/");
        self.files.contains(name)
            || self
                .files
                .range(directory.clone()..)
                .next()
                .is_some_and(|file| file.starts_with(&directory))
    }

    /// Looks up one normalised path.
    #[must_use]
    pub fn status(&self, path: &str) -> PathStatus {
        let path = normalise(path);
        if self.files.contains(&path) {
            return PathStatus::Present;
        }
        let directory = format!("{path}/");
        if self
            .files
            .range(directory.clone()..)
            .next()
            .is_some_and(|file| file.starts_with(&directory))
        {
            return PathStatus::Present;
        }
        let mut enclosing = path.as_str();
        loop {
            if self.unlisted.contains(enclosing) {
                return PathStatus::Unverifiable;
            }
            match enclosing.rsplit_once('/') {
                Some((parent, _)) => enclosing = parent,
                None => return PathStatus::Missing,
            }
        }
    }
}

/// A path as the view keeps it: `/`-separated, no leading `./` or `/`, no
/// trailing `/`.
fn normalise(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.trim_start_matches("./").trim_start_matches('/');
    path.trim_end_matches('/').to_owned()
}

// ---------------------------------------------------------------------------
// Path claims
// ---------------------------------------------------------------------------

/// Every path claim of `text`, in order of first appearance, each once: see the
/// module doc for the rule. A claim is the normalised path, ready for
/// [`CodeView::status`].
#[must_use]
pub fn path_claims(text: &str, code: &CodeView) -> Vec<String> {
    let mut claims: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for span in single_backtick_spans(line) {
            if let Some(claim) = claim_of(span, code)
                && !claims.contains(&claim)
            {
                claims.push(claim);
            }
        }
    }
    claims
}

/// The contents of the single-backtick spans of one line. A run of two or more
/// backticks is not a span here; it is skipped.
fn single_backtick_spans(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] != b'`' {
            at += 1;
            continue;
        }
        let mut run_end = at;
        while bytes.get(run_end) == Some(&b'`') {
            run_end += 1;
        }
        if run_end - at != 1 {
            at = run_end;
            continue;
        }
        let Some(close) = line[run_end..].find('`') else {
            break;
        };
        let end = run_end + close;
        // A closing run of two or more is the end of some other kind of span.
        if bytes.get(end + 1) == Some(&b'`') {
            let mut skip = end;
            while bytes.get(skip) == Some(&b'`') {
                skip += 1;
            }
            at = skip;
            continue;
        }
        spans.push(&line[run_end..end]);
        at = end + 1;
    }
    spans
}

/// The normalised path a span claims, if it claims one.
fn claim_of(span: &str, code: &CodeView) -> Option<String> {
    let span = span.trim();
    if span.is_empty()
        || span.contains(char::is_whitespace)
        || span.contains("://")
        || span.starts_with(['/', '~'])
        || span.contains(|c: char| "<>*?{}[]()$|=,;\"'".contains(c))
        || !span.contains('/')
    {
        return None;
    }
    let mut path = span;
    if let Some((before, _)) = path.split_once('#') {
        path = before;
    }
    path = strip_line_suffix(path);
    let path = normalise(path);
    if path.is_empty()
        || path
            .split('/')
            .any(|part| part.is_empty() || part == ".." || part == ".")
    {
        return None;
    }
    let first = path.split('/').next()?;
    // A path with a single segment after normalising was `name/`: still a claim
    // about a top-level directory, when the code has one by that name.
    code.has_top_level(first).then_some(path)
}

/// Removes a trailing `:12` or `:12-30`.
fn strip_line_suffix(path: &str) -> &str {
    let Some((head, tail)) = path.rsplit_once(':') else {
        return path;
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let is_range = match tail.split_once('-') {
        Some((from, to)) => digits(from) && digits(to),
        None => digits(tail),
    };
    if is_range { head } else { path }
}

// ---------------------------------------------------------------------------
// The audit
// ---------------------------------------------------------------------------

/// What the audit concluded about one document: AICD §9.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditOutcome {
    /// Every path it names is in the code: verified at the audit's time.
    Verified,
    /// A path it names is not in the code: a divergence was filed.
    Diverged,
    /// A path it names is under an entry nobody looked inside, and none is
    /// missing: nothing was recorded either way.
    Unverifiable,
}

/// What the audit found in one document: AICD §9.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentAudit {
    /// The document's path, the key the tracker records under.
    pub path: String,
    /// What was concluded.
    pub outcome: AuditOutcome,
    /// How many path claims the document makes, and so were checked. May be zero.
    pub references_checked: usize,
    /// The paths the code does not have.
    pub missing: Vec<String>,
    /// The paths under an entry nobody looked inside.
    pub unverifiable: Vec<String>,
}

/// What one audit recorded: AICD §9.
///
/// A record of the run, for a human or an event. It has no readiness verdict on
/// purpose: readiness reads the tracker (the module doc, "How the result reaches
/// readiness").
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriftReport {
    /// When the audit ran: the date verifications were recorded at.
    pub at: Timestamp,
    /// One entry per document audited, in path order.
    pub documents: Vec<DocumentAudit>,
}

impl DriftReport {
    /// The documents the audit found diverging from the code.
    pub fn divergences(&self) -> impl Iterator<Item = &DocumentAudit> {
        self.documents
            .iter()
            .filter(|audit| audit.outcome == AuditOutcome::Diverged)
    }

    /// How many path claims were checked across all documents. Zero means the
    /// audit compared nothing, however many documents it verified.
    #[must_use]
    pub fn references_checked(&self) -> usize {
        self.documents
            .iter()
            .map(|audit| audit.references_checked)
            .sum()
    }
}

/// The drift audit: AICD §9, `spec/PRD.md` section 4.3 row D-09 and section 4.13
/// row K-07.
#[derive(Clone, Debug)]
pub struct DriftAudit {
    code: CodeView,
}

impl DriftAudit {
    /// An audit against `code`.
    #[must_use]
    pub const fn new(code: CodeView) -> Self {
        Self { code }
    }

    /// Audits `documents` (path to the text a verification covers, which for a
    /// document of the index is
    /// [`crate::indexer::IndexableDocument::freshness_text`], as
    /// [`crate::indexer::Indexer::freshness_corpus`] gives it) and records the
    /// result in `tracker` at `at`.
    ///
    /// A document that names a missing path gets a divergence on record
    /// (`FreshnessTracker::record_divergence`); one that names none is verified
    /// at `at` with the text given (`FreshnessTracker::record_verification`),
    /// which also clears an earlier divergence, so a document that was repaired,
    /// or a code tree that gained the file, reads fresh at the next run. A
    /// document with only unverifiable paths is left as the tracker has it.
    ///
    /// # Errors
    ///
    /// [`DriftError::NothingToAudit`] when `documents` is empty and
    /// [`DriftError::EmptyCodeView`] when the view holds no file; the tracker is
    /// untouched in either case.
    pub fn run(
        &self,
        tracker: &mut FreshnessTracker,
        documents: &BTreeMap<String, String>,
        at: Timestamp,
    ) -> Result<DriftReport, DriftError> {
        if documents.is_empty() {
            return Err(DriftError::NothingToAudit);
        }
        if self.code.is_empty() {
            return Err(DriftError::EmptyCodeView);
        }
        let mut audited = Vec::with_capacity(documents.len());
        for (path, text) in documents {
            let claims = path_claims(text, &self.code);
            let mut missing = Vec::new();
            let mut unverifiable = Vec::new();
            for claim in &claims {
                match self.code.status(claim) {
                    PathStatus::Present => {}
                    PathStatus::Missing => missing.push(claim.clone()),
                    PathStatus::Unverifiable => unverifiable.push(claim.clone()),
                }
            }
            let outcome = if !missing.is_empty() {
                tracker.record_divergence(path.clone(), describe(&missing), at);
                AuditOutcome::Diverged
            } else if !unverifiable.is_empty() {
                AuditOutcome::Unverifiable
            } else {
                tracker.record_verification(path.clone(), at, text.clone());
                AuditOutcome::Verified
            };
            audited.push(DocumentAudit {
                path: path.clone(),
                outcome,
                references_checked: claims.len(),
                missing,
                unverifiable,
            });
        }
        Ok(DriftReport {
            at,
            documents: audited,
        })
    }

    /// Files a divergence found by something other than this module's
    /// mechanical comparison, such as the documentation agent's weekly read:
    /// D-09's "files divergences". It lands in `tracker` exactly as a mechanical
    /// one does.
    ///
    /// # Errors
    ///
    /// [`DriftError::EmptyFinding`] when `path` or `description` is blank.
    pub fn file_finding(
        tracker: &mut FreshnessTracker,
        path: &str,
        description: &str,
        at: Timestamp,
    ) -> Result<(), DriftError> {
        if path.trim().is_empty() {
            return Err(DriftError::EmptyFinding { field: "path" });
        }
        if description.trim().is_empty() {
            return Err(DriftError::EmptyFinding {
                field: "description",
            });
        }
        tracker.record_divergence(path, description, at);
        Ok(())
    }
}

/// The divergence text for a document naming paths the code does not have.
fn describe(missing: &[String]) -> String {
    let listed: Vec<String> = missing
        .iter()
        .take(DESCRIBED_PATHS)
        .map(|path| format!("`{path}`"))
        .collect();
    let more = missing.len().saturating_sub(DESCRIBED_PATHS);
    let noun = if missing.len() == 1 { "path" } else { "paths" };
    let tail = if more == 0 {
        String::new()
    } else {
        format!(", and {more} more")
    };
    format!(
        "names {} repository {noun} the code does not have: {}{tail}",
        missing.len(),
        listed.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::citation::SectionIndex;
    use crate::code_map::build_code_map;
    use crate::freshness::Freshness;
    use crate::freshness::StaleReason;
    use crate::indexer::NotIndexed;

    const DOC: &str = "spec/LLD.md#2-crate-responsibilities-and-dependency-direction";

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    /// The files the code has, in the shape of this repository.
    fn code() -> CodeView {
        CodeView::from_paths([
            "crates/ori-memory/src/lib.rs",
            "crates/ori-memory/src/drift.rs",
            "crates/ori-memory/src/citation.rs",
            "spec/LLD.md",
            "scripts/gates.sh",
            "Cargo.toml",
        ])
    }

    fn corpus(text: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(DOC.to_owned(), text.to_owned())])
    }

    fn stale_report(
        tracker: &FreshnessTracker,
        documents: &BTreeMap<String, String>,
    ) -> crate::freshness::StaleReport {
        tracker.list_stale(documents, &NotIndexed::default())
    }

    // -----------------------------------------------------------------
    // ORI-P1-026: a divergence the audit found makes the product not ready,
    // and the stale document is listed with the divergence
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_a_divergence_found_by_the_audit_makes_the_stale_report_not_ready_and_lists_it() {
        let documents = corpus(
            "The audit lives in `crates/ori-memory/src/drift.rs` and in \
             `crates/ori-memory/src/ghost.rs:40`.\n",
        );
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(code())
            .run(&mut tracker, &documents, at(1_000))
            .expect("audits");

        let found: Vec<&DocumentAudit> = report.divergences().collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, DOC);
        assert_eq!(found[0].missing, ["crates/ori-memory/src/ghost.rs"]);
        assert_eq!(found[0].references_checked, 2);

        let stale = stale_report(&tracker, &documents);
        assert_eq!(
            stale.documents_covered, 1,
            "the count must be non-zero before the answer means anything"
        );
        assert!(
            !stale.is_ready(),
            "a divergence on record is not ready, by the tracker's one readiness answer"
        );
        assert_eq!(stale.stale.len(), 1);
        assert_eq!(stale.stale[0].path, DOC);
        assert!(matches!(
            stale.stale[0].reason,
            StaleReason::Divergence { .. }
        ));
        let line = stale.stale[0].reason.divergence();
        assert!(
            line.contains("crates/ori-memory/src/ghost.rs"),
            "the stale document is listed with the divergence: {line}"
        );
        assert!(
            !line.contains("src/drift.rs"),
            "and only with the path that is missing: {line}"
        );
    }

    #[test]
    fn ori_p1_026_the_same_document_without_the_dangling_path_is_verified_and_ready() {
        // The control for the test above: the audit is not reporting stale for
        // every document it meets.
        let documents = corpus("The audit lives in `crates/ori-memory/src/drift.rs`.\n");
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(code())
            .run(&mut tracker, &documents, at(1_000))
            .expect("audits");
        assert_eq!(report.documents[0].outcome, AuditOutcome::Verified);
        assert_eq!(report.documents[0].references_checked, 1);
        let stale = stale_report(&tracker, &documents);
        assert_eq!(stale.documents_covered, 1);
        assert!(stale.stale.is_empty());
        assert!(stale.is_ready());
    }

    #[test]
    fn ori_p1_026_repairing_the_document_or_adding_the_file_clears_the_divergence_on_the_next_audit()
     {
        let broken = corpus("See `crates/ori-memory/src/ghost.rs`.\n");
        let mut tracker = FreshnessTracker::new();
        DriftAudit::new(code())
            .run(&mut tracker, &broken, at(1_000))
            .expect("audits");
        assert!(!stale_report(&tracker, &broken).is_ready());

        // The document is repaired: the verification of the new text clears it.
        let repaired = corpus("See `crates/ori-memory/src/lib.rs`.\n");
        DriftAudit::new(code())
            .run(&mut tracker, &repaired, at(2_000))
            .expect("audits");
        assert!(stale_report(&tracker, &repaired).is_ready());

        // Or the code gains the file the unchanged document names.
        let mut other = FreshnessTracker::new();
        DriftAudit::new(code())
            .run(&mut other, &broken, at(1_000))
            .expect("audits");
        assert!(!stale_report(&other, &broken).is_ready());
        let grown = CodeView::from_paths([
            "crates/ori-memory/src/lib.rs",
            "crates/ori-memory/src/ghost.rs",
        ]);
        DriftAudit::new(grown)
            .run(&mut other, &broken, at(3_000))
            .expect("audits");
        assert!(stale_report(&other, &broken).is_ready());
    }

    #[test]
    fn ori_p1_026_a_verification_moves_the_date_and_a_later_edit_makes_the_document_stale_again() {
        let text = "See `scripts/gates.sh`.\n";
        let documents = corpus(text);
        let mut tracker = FreshnessTracker::new();
        DriftAudit::new(code())
            .run(&mut tracker, &documents, at(5_000))
            .expect("audits");
        assert_eq!(tracker.check(DOC, text), Freshness::Fresh);
        assert_eq!(
            tracker.check(DOC, "See `scripts/gates.sh`, edited.\n"),
            Freshness::Stale(StaleReason::ChangedSinceVerification {
                verified_at: at(5_000)
            })
        );
    }

    #[test]
    fn ori_p1_026_one_diverging_document_among_fresh_ones_still_refuses_readiness() {
        let documents = BTreeMap::from([
            (
                "spec/CI_CD.md".to_owned(),
                "See `scripts/gates.sh`.".to_owned(),
            ),
            (
                "spec/LLD.md".to_owned(),
                "See `Cargo.toml`, no path.".to_owned(),
            ),
            (
                "spec/PRD.md".to_owned(),
                "See `crates/nope/src/x.rs`.".to_owned(),
            ),
        ]);
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(code())
            .run(&mut tracker, &documents, at(1_000))
            .expect("audits");
        let outcomes: Vec<(&str, AuditOutcome)> = report
            .documents
            .iter()
            .map(|audit| (audit.path.as_str(), audit.outcome))
            .collect();
        assert_eq!(
            outcomes,
            [
                ("spec/CI_CD.md", AuditOutcome::Verified),
                ("spec/LLD.md", AuditOutcome::Verified),
                ("spec/PRD.md", AuditOutcome::Diverged),
            ]
        );
        let stale = stale_report(&tracker, &documents);
        assert_eq!(stale.documents_covered, 3);
        assert!(!stale.is_ready());
        assert_eq!(
            stale
                .stale
                .iter()
                .map(|d| d.path.as_str())
                .collect::<Vec<_>>(),
            ["spec/PRD.md"]
        );
        assert_eq!(
            report.documents[1].references_checked, 0,
            "a document that names no path is verified, and says it was compared with nothing"
        );
        assert_eq!(report.references_checked(), 2);
    }

    // -----------------------------------------------------------------
    // D-09: the documentation agent's own findings reach the same place
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_a_finding_filed_by_the_documentation_agent_lands_in_the_tracker_and_refuses_readiness()
     {
        let documents = corpus("See `scripts/gates.sh`.\n");
        let mut tracker = FreshnessTracker::new();
        DriftAudit::new(code())
            .run(&mut tracker, &documents, at(1_000))
            .expect("audits");
        assert!(stale_report(&tracker, &documents).is_ready());

        DriftAudit::file_finding(
            &mut tracker,
            DOC,
            "the retry limit is 3 in this section and 5 in the code",
            at(2_000),
        )
        .expect("files");
        let stale = stale_report(&tracker, &documents);
        assert!(!stale.is_ready());
        assert!(
            stale.stale[0]
                .reason
                .divergence()
                .contains("the retry limit is 3 in this section and 5 in the code")
        );
    }

    #[test]
    fn ori_t_0040_a_finding_with_a_blank_path_or_description_is_refused_and_files_nothing() {
        let mut tracker = FreshnessTracker::new();
        assert_eq!(
            DriftAudit::file_finding(&mut tracker, " ", "d", at(1)),
            Err(DriftError::EmptyFinding { field: "path" })
        );
        assert_eq!(
            DriftAudit::file_finding(&mut tracker, "spec/CI_CD.md", "\n", at(1)),
            Err(DriftError::EmptyFinding {
                field: "description"
            })
        );
        assert_eq!(tracker.tracked_count(), 0);
    }

    // -----------------------------------------------------------------
    // The refusals: an audit that compared nothing is not a clean audit
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_an_audit_of_no_document_or_against_no_code_is_refused_and_records_nothing() {
        let mut tracker = FreshnessTracker::new();
        assert_eq!(
            DriftAudit::new(code()).run(&mut tracker, &BTreeMap::new(), at(1)),
            Err(DriftError::NothingToAudit)
        );
        assert_eq!(
            DriftAudit::new(CodeView::default()).run(&mut tracker, &corpus("x"), at(1)),
            Err(DriftError::EmptyCodeView)
        );
        assert_eq!(tracker.tracked_count(), 0);
    }

    // -----------------------------------------------------------------
    // What a path claim is
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_a_path_claim_is_a_single_backtick_span_naming_a_path_the_code_could_have() {
        let view = code();
        let cases: [(&str, &[&str]); 18] = [
            (
                "`crates/ori-memory/src/lib.rs`",
                &["crates/ori-memory/src/lib.rs"],
            ),
            ("`./scripts/gates.sh:12-30`", &["scripts/gates.sh"]),
            ("`scripts/gates.sh:7`", &["scripts/gates.sh"]),
            (
                "`spec/LLD.md#2-crate-responsibilities-and-dependency-direction`",
                &["spec/LLD.md"],
            ),
            ("`crates/ori-memory/`", &["crates/ori-memory"]),
            (
                "`crates/a.rs`, then `crates/b.rs`",
                &["crates/a.rs", "crates/b.rs"],
            ),
            ("`crates/a.rs` and again `crates/a.rs`", &["crates/a.rs"]),
            ("`ledger/src/entry.rs` is another product's", &[]),
            ("`crates/<name>/src/lib.rs`", &[]),
            ("`crates/*/Cargo.toml`", &[]),
            ("`cargo test -p ori-memory`", &[]),
            ("`Cargo.toml` has no slash", &[]),
            ("`https://example.com/crates/x`", &[]),
            ("`/crates/absolute.rs` and `~/crates/home.rs`", &[]),
            ("`crates/../etc/passwd`", &[]),
            ("``crates/double.rs``", &[]),
            (
                "```text\nsee `crates/fenced.rs`\n```\nthen `crates/after.rs`",
                &["crates/after.rs"],
            ),
            ("crates/bare.rs is not in a span", &[]),
        ];
        for (text, expected) in cases {
            assert_eq!(path_claims(text, &view), expected, "for {text:?}");
        }
    }

    #[test]
    fn ori_t_0040_a_directory_is_present_when_it_holds_a_file_and_a_missing_one_is_missing() {
        let view = code();
        assert_eq!(view.status("crates/ori-memory/src"), PathStatus::Present);
        assert_eq!(
            view.status("crates/ori-memory/src/lib.rs"),
            PathStatus::Present
        );
        assert_eq!(
            view.status("crates/ori-mem"),
            PathStatus::Missing,
            "a prefix is not a directory"
        );
        assert_eq!(
            view.status("crates/ori-memory/src/gone.rs"),
            PathStatus::Missing
        );
        assert_eq!(view.len(), 6);
    }

    #[test]
    fn ori_t_0040_a_path_under_an_entry_nobody_looked_inside_is_unverifiable_and_never_verified() {
        let view = code().with_unlisted("crates/vendor");
        assert_eq!(view.status("crates/vendor"), PathStatus::Present);
        assert_eq!(
            view.status("crates/vendor/dep/lib.rs"),
            PathStatus::Unverifiable
        );
        assert_eq!(view.status("crates/other/lib.rs"), PathStatus::Missing);

        let documents = corpus("See `crates/vendor/dep/lib.rs`.\n");
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(view)
            .run(&mut tracker, &documents, at(1_000))
            .expect("audits");
        assert_eq!(report.documents[0].outcome, AuditOutcome::Unverifiable);
        assert_eq!(
            report.documents[0].unverifiable,
            ["crates/vendor/dep/lib.rs"]
        );
        assert!(report.divergences().next().is_none());
        let stale = stale_report(&tracker, &documents);
        assert!(
            !stale.is_ready(),
            "a document nobody could check is never fresh"
        );
        assert!(matches!(stale.stale[0].reason, StaleReason::NeverVerified));
    }

    #[test]
    fn ori_t_0040_a_missing_path_wins_over_an_unverifiable_one() {
        let view = code().with_unlisted("crates/vendor");
        let documents = corpus("`crates/vendor/dep/lib.rs` and `crates/ghost/x.rs`");
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(view)
            .run(&mut tracker, &documents, at(1))
            .expect("audits");
        assert_eq!(report.documents[0].outcome, AuditOutcome::Diverged);
    }

    #[test]
    fn ori_t_0040_a_long_list_of_missing_paths_is_cut_in_the_description_and_counted() {
        let text: String = (0..15)
            .map(|n| format!("`crates/ghost/f{n}.rs` "))
            .collect();
        let documents = corpus(&text);
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(code())
            .run(&mut tracker, &documents, at(1))
            .expect("audits");
        assert_eq!(
            report.documents[0].missing.len(),
            15,
            "the report keeps all of them"
        );
        let line = stale_report(&tracker, &documents).stale[0]
            .reason
            .divergence();
        assert!(line.contains("names 15 repository paths"), "{line}");
        assert!(line.contains("and 3 more"), "{line}");
        assert!(
            line.contains("f11.rs") && !line.contains("f12.rs"),
            "{line}"
        );
    }

    // -----------------------------------------------------------------
    // The code view from a code map
    // -----------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0040-drift-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a scratch directory can be made");
            Self { path }
        }

        fn write(&self, relative: &str, text: &str) {
            let target = self.path.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("parent directories can be made");
            }
            fs::write(target, text).expect("a scratch file can be written");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn ori_t_0040_a_view_from_a_code_map_holds_parsed_modules_and_every_file_it_saw() {
        let scratch = Scratch::new("code-map");
        scratch.write("crates/a/src/lib.rs", "pub fn a() {}\n");
        scratch.write("docs/guide.md", "# Guide\n");
        scratch.write("scripts/run.sh", "echo hi\n");
        scratch.write("Cargo.toml", "[package]\n");
        let map = build_code_map(&scratch.path).expect("maps");
        let view = CodeView::from_code_map(&map);
        assert_eq!(
            view.status("crates/a/src/lib.rs"),
            PathStatus::Present,
            "a parsed module"
        );
        assert_eq!(
            view.status("docs/guide.md"),
            PathStatus::Present,
            "a file seen and not parsed"
        );
        assert_eq!(view.status("scripts/run.sh"), PathStatus::Present);
        assert_eq!(view.status("docs/missing.md"), PathStatus::Missing);
        assert!(view.len() >= 4);

        let documents = corpus("See `docs/guide.md` and `docs/missing.md`.");
        let mut tracker = FreshnessTracker::new();
        let report = DriftAudit::new(view)
            .run(&mut tracker, &documents, at(1))
            .expect("audits");
        assert_eq!(report.documents[0].missing, ["docs/missing.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0040_a_directory_the_code_map_would_not_follow_is_unverifiable_not_missing() {
        let scratch = Scratch::new("symlinked-directory");
        scratch.write("real/file.txt", "x");
        scratch.write("crates/a/src/lib.rs", "pub fn a() {}\n");
        std::os::unix::fs::symlink(scratch.path.join("real"), scratch.path.join("linked"))
            .expect("a link can be made");
        let map = build_code_map(&scratch.path).expect("maps");
        let view = CodeView::from_code_map(&map);
        assert_eq!(view.status("linked/file.txt"), PathStatus::Unverifiable);
        assert_eq!(view.status("real/file.txt"), PathStatus::Present);
    }

    // -----------------------------------------------------------------
    // ORI-P1-033: every refusal carries a MethodologyRef that resolves
    // -----------------------------------------------------------------

    /// One name per variant, matched without a wildcard so that a variant
    /// added later stops the build until a sample is written for it.
    fn variant_name(error: &DriftError) -> &'static str {
        match error {
            DriftError::NothingToAudit => "NothingToAudit",
            DriftError::EmptyCodeView => "EmptyCodeView",
            DriftError::EmptyFinding { .. } => "EmptyFinding",
        }
    }

    #[test]
    fn ori_p1_033_every_drift_error_carries_a_methodology_ref_that_resolves() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let index = SectionIndex::from_file(&repo.join(crate::citation::SECTION_INDEX_PATH))
            .expect("the committed methodology index reads");
        let samples = [
            DriftError::NothingToAudit,
            DriftError::EmptyCodeView,
            DriftError::EmptyFinding { field: "path" },
        ];
        let names: std::collections::BTreeSet<&str> = samples.iter().map(variant_name).collect();
        assert_eq!(names.len(), 3, "every variant has a sample: {names:?}");
        for error in samples {
            let reference = error.methodology_ref();
            assert!(
                reference.resolves(),
                "{}: {reference}",
                variant_name(&error)
            );
            assert_eq!(reference.subsection, None);
            assert!(
                index.contains(&reference.section.to_string()),
                "{}: section {} is not in methodology/sections.json",
                variant_name(&error),
                reference.section
            );
            assert!(!error.to_string().is_empty());
        }
    }
}
