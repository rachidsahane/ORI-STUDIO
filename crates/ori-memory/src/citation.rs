//! The citation checker: AICD §39, AICD §14.
//!
//! `spec/PRD.md` section 4.20, row Z-03, asks for a "citation and reference
//! integrity gate over specification, operational memory and instruction files,
//! against the methodology's machine-readable section index; drafting-error
//! rate recorded". AICD §39 records the failure that earned it ("four drafting
//! agents produced eleven fabricated section references, one of which would have
//! removed a required check") and the rule adopted from it: "References are
//! checked mechanically." AICD §14 adds the second half this module checks, that
//! a gate is not cited as protection until it has been seen to fail on a planted
//! defect. `spec/LLD.md` section 2 gives `ori-memory` a `CitationChecker`, and
//! this is it. It is the engine's own checker: the gate that runs it on every
//! pull request is a later ticket in `ori-gates` and is not this module.
//!
//! ```mermaid
//! flowchart LR
//!   J[methodology/sections.json] -->|SectionIndex::from_json| X[SectionIndex]
//!   C[Corpus: specification, memory records, instruction files] --> K[CitationChecker::check]
//!   X --> K
//!   G[GateStandings, from the caller] --> K
//!   E[Exclusions: the E-0006 design mockup] --> K
//!   K --> R[CitationReport: unresolved, unproven gates, drafting-error rate]
//! ```
//!
//! # What is shared with `ori-gates` and what is restated, and why
//!
//! `ori-memory` may not depend on `ori-gates` (`spec/LLD.md` section 2: the
//! orchestrator depends on both, and `ori-memory` depends only on `ori-store` and
//! `ori-core`). Nothing is therefore imported from the `sections` or `spec_refs`
//! modules of `ori-gates`. What is shared is data:
//! `methodology/sections.json`, read by [`crate::citation::SectionIndex`] as a
//! file, which is exactly what the committed index is for. The index itself is
//! tied to a fresh parse of the methodology by tests in `ori-gates`, so a stale
//! index is that crate's failure and not this one's.
//!
//! What is restated is the citation grammar, because the scanner in `ori-gates`
//! is a test-module item that no other crate can call: `AICD`, blanks, then the
//! section sign (or `&sect;`) and a number with an optional `.n` part, or then
//! the word `appendix` and a letter with an optional `.n` part. A bare section
//! sign is not a citation of the methodology, because this repository writes it
//! for its own documents' sections. The restatement is held to the same fixture
//! the original is held to (`tests::ori_t_0040_the_scanner_collects_the_citation_form_conventions_fixes_and_no_other`),
//! and the repository sweep (`tests::ori_t_0040_this_repositorys_specification_resolves_with_only_the_mockup_excluded`)
//! runs it over every file under `spec/`, where the original's verdict is that
//! everything resolves outside the design mockup. A divergence between the two
//! scanners would show there first.
//!
//! The other grammar `ori-gates` has, references to this repository's own `spec/`
//! documents (`crates/ori-gates/src/spec_refs.rs`), is not Z-03's. Z-03 is
//! "against the methodology's machine-readable section index", so this checker
//! resolves methodology citations only.
//!
//! # What is checked
//!
//! 1. Every methodology citation of every judged document resolves in the
//!    section index, or is reported with its file and line.
//! 2. The drafting-error rate: unresolved over total, both counts
//!    ([`crate::citation::DraftingErrorRate`]). A corpus with no citation has no
//!    rate, which is not a rate of zero (the type says so). This module computes
//!    and reports the rate; appending it to the metrics or the event log is the
//!    caller's, because this module appends no event.
//! 3. Criterion ORI-P1-012: a gate cited as protection whose standing is not
//!    proven is flagged ([`crate::citation::UnprovenGateCitation`]).
//!
//! # Gate standings come from the caller
//!
//! `ori-gates` owns what a gate's standing is (`spec/DATA_MODEL.md` section 2:
//! "state (defined, proven, installed, inert)"), and `ori-memory` cannot read its
//! registry. The caller therefore hands in [`crate::citation::GateStandings`]. This
//! module keeps no second source of truth: it holds what it is given, for the
//! length of one check, and a gate the caller names no standing for is reported
//! as unknown rather than read as fine. A gate whose proof exists but which is
//! inert is also flagged, under its own finding: AICD §39 names a check that is
//! "present but reporting nothing" as the failure, and an inert gate protects
//! nothing.
//!
//! # What "cites a gate as protection" means here, and what it misses
//!
//! Prose cannot be parsed for intent, so the rule is mechanical and small. A
//! sentence (split at a full stop, question mark or exclamation mark followed by a
//! blank, or at the end of its line) cites gate `n` as protection when it
//! contains the words `gate n` (a blank between, so `gate-7` in a path is not
//! one), carries a protection word (a word that starts `protect`, `guard`,
//! `catch`, `caught`, `prevent`, `enforc`, `refus`, `block` or `cover`), and carries no
//! hedge word (`not`, `never`, `cannot`, `until`, `unless`, `once`, `will`,
//! `would`, `if`, `whether`, `yet`, `without`, `before`, `should`, `could`,
//! `may`, `might`, or a word ending `n't`). A hedged sentence says the gate is
//! not protecting yet, and flagging it would flag the sentences that tell the
//! truth.
//!
//! It does not catch: a gate named by something other than `gate n` (its name, a
//! range such as "gates 7 and 9", a number in a list), a protection claim in a
//! word this list lacks, a sentence whose hedge word is incidental (so a
//! false claim can hide behind an unrelated "not"), a claim spread over two
//! sentences, or a claim in a fenced code block that is read as the prose it is
//! (a fenced block is not special here). It flags a description of what a gate
//! is going to do when the sentence is not hedged. These are the limits of a
//! phase 1 reader; the gate that wraps this checker can tighten the lists, and
//! the lists are small on purpose so that a tightening is a visible diff.
//!
//! # The design mockup is excluded by name, and the exclusion cannot go stale
//!
//! The operator answered escalation E-0006 on 2026-09-27: the check covers
//! everything under `spec/`, HTML included, and excludes the design mockup
//! `spec/design/Ori Studio.html` by name. The file is a 342 KB blob of sample
//! data about a fictional product, Ledgerline, with citations that are props, one
//! of them a picture of this very check refusing a bad citation. The exclusion
//! is one [`crate::citation::Exclusion`] carrying the file, the reason, and a
//! marker that must still be in the file: the fictional product's name. **On
//! every run**, before anything is judged, the checker looks for the marker in
//! the excluded document and refuses the run if it is gone
//! ([`crate::citation::CitationError::ExclusionMarkerGone`]), so the exemption
//! cannot outlive its subject. For this repository the exclusion is also
//! required to exist ([`crate::citation::Exclusion::required`]); a product that
//! has no such file passes [`crate::citation::Exclusions::none`] and is judged
//! over everything. An empty marker would assert nothing and is refused when the
//! checker is built.
//!
//! The excluded file's citations are neither resolved nor counted in the
//! drafting-error rate; the report names the file, the reason, and how many
//! citations it left unjudged ([`crate::citation::ExcludedDocument`]), so the
//! size of the blind spot is never a guess.
//!
//! # Reading the corpus
//!
//! The checker judges bytes it is given ([`crate::citation::Corpus`]); the
//! readers here ([`crate::citation::Corpus::read_specification`] and
//! [`crate::citation::Corpus::read_files`]) only read, never write, and never
//! follow a symbolic link. They read every file under `spec/` whatever its
//! extension, because a walk narrowed to Markdown reports a clean sweep of this
//! repository while every citation that fails is in an HTML file. A file they
//! cannot read, or will not (too large, a link, not a regular file, a name that is
//! not UTF-8), is recorded in the corpus as unread and carried into the report,
//! which is not clean while one exists: a file skipped quietly is a file whose
//! citations all appear to resolve. A tree beyond the file or byte limit is
//! refused outright. Like the indexer, a read race against a live writer is out of
//! scope: the walk reads the canonical checkout.
//!
//! Operational memory records and instruction files are the corpus's other two
//! kinds. Records come from the memory service as text the caller already holds
//! ([`crate::citation::Corpus::push`]); instruction files are named by the caller
//! ([`crate::citation::Corpus::read_files`]).
//!
//! # Refusals
//!
//! Every error here ([`crate::citation::CitationError`]) carries a
//! [`ori_core::error::MethodologyRef`] to AICD §39, the section whose rule the
//! check implements, and criterion ORI-P1-033 is held by a test that resolves
//! each one in the real index.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;

use ori_core::error::MethodologyRef;
use serde::Deserialize;

/// The methodology's section index, relative to the repository root:
/// AICD §39.
pub const SECTION_INDEX_PATH: &str = "methodology/sections.json";

/// The directory under the repository root that holds the specification:
/// `spec/LLD.md` section 1.
pub const SPECIFICATION_DIR: &str = "spec";

/// The reference every refusal of this module carries: AICD §39.
const fn citation_rule_ref() -> MethodologyRef {
    MethodologyRef {
        section: 39,
        subsection: None,
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a citation check could not be made, or was refused: AICD §39.
///
/// Every variant carries [`CitationError::methodology_ref`], the section whose
/// rule ("References are checked mechanically") the check implements.
#[derive(Debug)]
#[non_exhaustive]
pub enum CitationError {
    /// The section index could not be read as one.
    IndexMalformed {
        /// What was wrong with it.
        reason: String,
    },
    /// The section index resolved nothing. An index with no entry would
    /// report every citation as a drafting error, or, read the other way, would
    /// look like a check while checking nothing.
    IndexEmpty,
    /// A file or directory could not be read.
    Io {
        /// The path, as the caller named it.
        path: String,
        /// The operating system's account.
        source: io::Error,
    },
    /// The corpus held no document. A check over nothing must not read as a
    /// clean check.
    EmptyCorpus,
    /// An exclusion's marker is empty, so asserting it present asserts nothing.
    ExclusionMarkerEmpty {
        /// The excluded document's path.
        path: String,
    },
    /// An excluded document no longer carries the marker that is the evidence
    /// for excluding it, so the exclusion has outlived its subject and the run
    /// is refused.
    ExclusionMarkerGone {
        /// The excluded document's path.
        path: String,
        /// The marker that is no longer in it.
        marker: String,
        /// Why the document was excluded.
        reason: String,
    },
    /// A document the exclusion list requires to exist is not in the corpus,
    /// so the exclusion stands for nothing and its reason cannot be checked.
    ExcludedDocumentMissing {
        /// The excluded document's path.
        path: String,
        /// Why the document was excluded.
        reason: String,
    },
    /// The tree to read is larger than the limits allow, and reading part of it
    /// would report a partial check as a whole one.
    CorpusTooLarge {
        /// Which limit was crossed.
        limit: CorpusLimit,
    },
}

/// Which limit a tree crossed: AICD §39.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorpusLimit {
    /// More entries than [`ReadLimits::max_files`].
    Files(usize),
    /// More bytes read in total than [`ReadLimits::max_total_bytes`].
    TotalBytes(u64),
}

impl CitationError {
    /// The methodology section this refusal rests on: AICD §39, the rule that
    /// references are checked mechanically.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        citation_rule_ref()
    }
}

impl fmt::Display for CitationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexMalformed { reason } => {
                write!(f, "the methodology section index is malformed: {reason}")
            }
            Self::IndexEmpty => f.write_str(
                "the methodology section index holds no section, so it resolves nothing",
            ),
            Self::Io { path, source } => write!(f, "{path}: {source}"),
            Self::EmptyCorpus => f.write_str(
                "the corpus holds no document, so nothing was checked and no result can be reported",
            ),
            Self::ExclusionMarkerEmpty { path } => write!(
                f,
                "the exclusion of {path} has an empty marker, which asserts nothing about the file"
            ),
            Self::ExclusionMarkerGone {
                path,
                marker,
                reason,
            } => write!(
                f,
                "{path} no longer contains {marker:?}, the evidence for not judging it ({reason}); \
                 the exclusion has outlived its subject and the run is refused until someone reads \
                 the file again"
            ),
            Self::ExcludedDocumentMissing { path, reason } => write!(
                f,
                "{path} is excluded because it is {reason} and it is not in the corpus, so the \
                 exclusion stands for nothing and its reason cannot be checked"
            ),
            Self::CorpusTooLarge {
                limit: CorpusLimit::Files(limit),
            } => write!(
                f,
                "the specification tree holds more than {limit} entries; refusing a partial check"
            ),
            Self::CorpusTooLarge {
                limit: CorpusLimit::TotalBytes(limit),
            } => write!(
                f,
                "the specification tree holds more than {limit} bytes; refusing a partial check"
            ),
        }
    }
}

impl std::error::Error for CitationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// The section index
// ---------------------------------------------------------------------------

/// The shape of `methodology/sections.json` this module reads. Unknown fields
/// are ignored, so the generator may add to the file.
#[derive(Deserialize)]
struct IndexWire {
    counts: Option<CountsWire>,
    entries: Vec<EntryWire>,
}

#[derive(Deserialize)]
struct CountsWire {
    total: Option<usize>,
}

#[derive(Deserialize)]
struct EntryWire {
    number: String,
    kind: String,
}

/// What the methodology numbers, read from `methodology/sections.json`:
/// AICD §39.
///
/// Holds the number of every section, appendix and numbered subsection in the
/// form a citation spells it (`7`, `24.2`, `A`, `A.5`). It is read as data, never
/// restated in code, so it cannot drift from the file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionIndex {
    numbers: BTreeSet<String>,
    sections: usize,
}

impl SectionIndex {
    /// Reads the index from the text of `methodology/sections.json`.
    ///
    /// # Errors
    ///
    /// [`CitationError::IndexMalformed`] when the text is not the index, a number
    /// is blank or repeated, or the file's own total disagrees with its entries;
    /// [`CitationError::IndexEmpty`] when it lists no numbered section.
    pub fn from_json(json: &str) -> Result<Self, CitationError> {
        let wire: IndexWire =
            serde_json::from_str(json).map_err(|error| CitationError::IndexMalformed {
                reason: error.to_string(),
            })?;
        if let Some(total) = wire.counts.and_then(|counts| counts.total)
            && total != wire.entries.len()
        {
            return Err(CitationError::IndexMalformed {
                reason: format!(
                    "the file counts {total} entries and lists {}",
                    wire.entries.len()
                ),
            });
        }
        let mut numbers = BTreeSet::new();
        let mut sections = 0usize;
        for entry in wire.entries {
            if entry.number.trim().is_empty() {
                return Err(CitationError::IndexMalformed {
                    reason: "an entry has a blank number".to_owned(),
                });
            }
            if entry.kind == "section" {
                sections += 1;
            }
            let number = entry.number;
            if !numbers.insert(number.clone()) {
                return Err(CitationError::IndexMalformed {
                    reason: format!("the number {number} is listed twice"),
                });
            }
        }
        if sections == 0 {
            return Err(CitationError::IndexEmpty);
        }
        Ok(Self { numbers, sections })
    }

    /// Reads the index from a file.
    ///
    /// # Errors
    ///
    /// [`CitationError::Io`] when the file cannot be read, otherwise as
    /// [`SectionIndex::from_json`].
    pub fn from_file(path: &Path) -> Result<Self, CitationError> {
        let json = fs::read_to_string(path).map_err(|source| CitationError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&json)
    }

    /// Whether the methodology has a heading numbered `number`.
    #[must_use]
    pub fn contains(&self, number: &str) -> bool {
        self.numbers.contains(number)
    }

    /// How many numbered sections (not appendices or subsections) the index
    /// lists.
    #[must_use]
    pub const fn section_count(&self) -> usize {
        self.sections
    }

    /// How many headings the index lists in all.
    #[must_use]
    pub fn len(&self) -> usize {
        self.numbers.len()
    }

    /// Whether the index lists no heading. Never true of one built by this
    /// module, which refuses an empty index.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Citations
// ---------------------------------------------------------------------------

/// One methodology citation as a document spells it: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Citation {
    /// The number in the form [`SectionIndex::contains`] takes: `7`, `24.2`,
    /// `A`, `A.5`.
    pub number: String,
    /// The 1-based line it is written on.
    pub line: usize,
}

impl Citation {
    /// The citation spelled the way a document writes it.
    #[must_use]
    pub fn spelled(&self) -> String {
        spell(&self.number)
    }
}

/// A number spelled back as a citation. Written with escapes so this file does
/// not carry the citation it describes.
fn spell(number: &str) -> String {
    if number.starts_with(|c: char| c.is_ascii_alphabetic()) {
        format!("AICD appendix {number}")
    } else {
        format!("AICD \u{a7}{number}")
    }
}

/// Every methodology citation in `bytes`, in order: AICD §39.
///
/// Bytes, not text: a file that is not UTF-8 would otherwise be skipped, and a
/// skipped file reads as a file with no bad citation.
#[must_use]
pub fn find_citations(bytes: &[u8]) -> Vec<Citation> {
    const WORD: &[u8] = b"AICD";
    let mut found = Vec::new();
    let mut at = 0usize;
    let mut line = 1usize;
    let mut counted_to = 0usize;
    while at + WORD.len() <= bytes.len() {
        if &bytes[at..at + WORD.len()] != WORD {
            at += 1;
            continue;
        }
        let after_word = at + WORD.len();
        let marker = skip_blanks(bytes, after_word);
        if marker == after_word {
            // `AICD` glued to what follows is a word, not a citation.
            at = after_word;
            continue;
        }
        let Some((number, end)) =
            section_number(bytes, marker).or_else(|| appendix_number(bytes, marker))
        else {
            at = after_word;
            continue;
        };
        line += bytes[counted_to..at]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
        counted_to = at;
        found.push(Citation { number, line });
        at = end;
    }
    found
}

/// The section sign (or its entity) and a number at `at`.
fn section_number(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    const SIGN: &[u8] = "\u{a7}".as_bytes();
    const ENTITY: &[u8] = b"&sect;";
    let rest = bytes.get(at..)?;
    let mut cursor = if rest.starts_with(SIGN) {
        at + SIGN.len()
    } else if rest.starts_with(ENTITY) {
        at + ENTITY.len()
    } else {
        return None;
    };
    cursor = skip_blanks(bytes, cursor);
    let start = cursor;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    if cursor == start {
        return None;
    }
    let mut number = String::from_utf8_lossy(&bytes[start..cursor]).into_owned();
    if let Some((fraction, end)) = fraction(bytes, cursor) {
        number.push('.');
        number.push_str(&fraction);
        cursor = end;
    }
    Some((number, cursor))
}

/// The word `appendix`, a letter and an optional `.n` at `at`. The letter is
/// upper-cased for the lookup: a lower-case letter is a spelling slip, and
/// reporting it as unresolved would report a fabrication that is not there.
fn appendix_number(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    const WORD: &[u8] = b"appendix";
    if !bytes.get(at..at + WORD.len())?.eq_ignore_ascii_case(WORD) {
        return None;
    }
    let after_word = at + WORD.len();
    let mut cursor = skip_blanks(bytes, after_word);
    if cursor == after_word {
        return None;
    }
    let letter = *bytes.get(cursor)?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    cursor += 1;
    // `appendix Alpha` names no appendix.
    if bytes.get(cursor).is_some_and(u8::is_ascii_alphanumeric) {
        return None;
    }
    let mut number = String::new();
    number.push(letter.to_ascii_uppercase() as char);
    if let Some((fraction, end)) = fraction(bytes, cursor) {
        number.push('.');
        number.push_str(&fraction);
        cursor = end;
    }
    Some((number, cursor))
}

/// `.5` at `at`, as the digits and the offset past them.
fn fraction(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    if bytes.get(at) != Some(&b'.') {
        return None;
    }
    let start = at + 1;
    let mut cursor = start;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    if cursor == start {
        return None;
    }
    Some((
        String::from_utf8_lossy(&bytes[start..cursor]).into_owned(),
        cursor,
    ))
}

/// The offset of the first byte at or after `at` that is not a blank. A
/// non-breaking space counts as one, which the HTML sources carry.
fn skip_blanks(bytes: &[u8], mut at: usize) -> usize {
    loop {
        match bytes.get(at) {
            Some(byte) if byte.is_ascii_whitespace() => at += 1,
            Some(0xc2) if bytes.get(at + 1) == Some(&0xa0) => at += 2,
            _ => return at,
        }
    }
}

// ---------------------------------------------------------------------------
// Gate citations (ORI-P1-012)
// ---------------------------------------------------------------------------

/// Where a gate stands, as the caller reports it: AICD §14.
///
/// The four values are `spec/DATA_MODEL.md` section 2's gate states. This module
/// does not decide them: it holds what the caller read from the gate registry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GateStanding {
    /// Defined, and no proof exists.
    Defined,
    /// A proof exists: the gate failed on its planted defect.
    Proven,
    /// Proven and wired into the pipeline.
    Installed,
    /// Installed, and its runs have gone missing, so it protects nothing.
    Inert,
}

/// The standing of each gate, supplied by the caller: AICD §14.
///
/// `ori-gates` owns gate state and `ori-memory` cannot read its registry, so
/// this is input and not a registry. A gate with no entry is unknown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GateStandings {
    standings: BTreeMap<u8, GateStanding>,
}

impl GateStandings {
    /// No standing for any gate, so every gate cited as protection is unknown.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the standing of gate `number`, replacing any earlier one.
    pub fn set(&mut self, number: u8, standing: GateStanding) {
        self.standings.insert(number, standing);
    }

    /// The standing of gate `number`, if the caller gave one.
    #[must_use]
    pub fn get(&self, number: u8) -> Option<GateStanding> {
        self.standings.get(&number).copied()
    }
}

/// Why a gate cited as protection was flagged: AICD §14.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateFinding {
    /// The gate is defined and no proof exists, so it is unproven: criterion
    /// ORI-P1-012.
    Unproven,
    /// The gate was proven and installed and has since gone inert.
    Inert,
    /// The caller reported no standing for this gate, so it cannot be called
    /// proven.
    Unknown,
}

impl fmt::Display for GateFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unproven => "defined, with no proof that it fails on a planted defect",
            Self::Inert => "inert: its runs are missing, so it protects nothing",
            Self::Unknown => "no standing was reported for it, so it cannot be called proven",
        })
    }
}

/// A gate cited as protection that is not proven: criterion ORI-P1-012.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnprovenGateCitation {
    /// The document that cites it.
    pub path: String,
    /// What kind of document that is.
    pub kind: CorpusKind,
    /// The 1-based line of the sentence.
    pub line: usize,
    /// The gate's number.
    pub gate: u8,
    /// Why it is flagged.
    pub finding: GateFinding,
}

impl fmt::Display for UnprovenGateCitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: gate {} is cited as protection and is {}",
            self.path, self.line, self.gate, self.finding
        )
    }
}

/// Words that begin a protection claim, matched at a word's start.
const PROTECTION_WORDS: [&str; 9] = [
    "protect", "guard", "catch", "caught", "prevent", "enforc", "refus", "block", "cover",
];

/// Words that say a gate is not protecting (yet), whole-word.
const HEDGE_WORDS: [&str; 17] = [
    "not", "never", "cannot", "until", "unless", "once", "will", "would", "if", "whether", "yet",
    "without", "before", "should", "could", "may", "might",
];

/// Every gate a document cites as protection, with the line: criterion
/// ORI-P1-012. See the module doc for the rule and what it misses.
#[must_use]
pub fn find_gate_citations(text: &str) -> Vec<(usize, u8)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for sentence in sentences(line) {
            let lower = sentence.to_lowercase();
            let words: Vec<&str> = lower
                .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '\u{2019}'))
                .filter(|word| !word.is_empty())
                .collect();
            if words.iter().any(|word| is_hedge(word)) {
                continue;
            }
            if !words
                .iter()
                .any(|word| PROTECTION_WORDS.iter().any(|stem| word.starts_with(stem)))
            {
                continue;
            }
            for gate in gate_numbers(&lower) {
                if !found.contains(&(index + 1, gate)) {
                    found.push((index + 1, gate));
                }
            }
        }
    }
    found
}

fn is_hedge(word: &str) -> bool {
    HEDGE_WORDS.contains(&word) || word.ends_with("n't") || word.ends_with("n\u{2019}t")
}

/// The sentences of one line: cut after `.`, `?` or `!` followed by a blank.
fn sentences(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut chars = line.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if matches!(c, '.' | '?' | '!')
            && chars.peek().is_some_and(|(_, next)| next.is_whitespace())
        {
            parts.push(&line[start..at + c.len_utf8()]);
            start = at + c.len_utf8();
        }
    }
    parts.push(&line[start..]);
    parts
}

/// The numbers `n` of every `gate n` in `lower`: the word `gate` not glued to a
/// path or a name, a blank, one to three digits, and no further letter, digit or
/// dotted number after them.
fn gate_numbers(lower: &str) -> Vec<u8> {
    const WORD: &str = "gate";
    let bytes = lower.as_bytes();
    let mut numbers = Vec::new();
    let mut from = 0usize;
    while let Some(offset) = lower[from..].find(WORD) {
        let at = from + offset;
        from = at + WORD.len();
        let before_ok = at == 0 || !is_name_byte(bytes[at - 1]);
        if !before_ok {
            continue;
        }
        let mut cursor = at + WORD.len();
        let blanks_from = cursor;
        while bytes.get(cursor).is_some_and(|b| *b == b' ' || *b == b'\t') {
            cursor += 1;
        }
        if cursor == blanks_from {
            continue;
        }
        let digits_from = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if cursor == digits_from || cursor - digits_from > 3 {
            continue;
        }
        // A full stop after the number ends the sentence; a letter, a hyphen or
        // a slash after it makes it part of some other name.
        let glued = bytes
            .get(cursor)
            .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_' | b'/'));
        let dotted = bytes.get(cursor) == Some(&b'.')
            && bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit);
        if glued || dotted {
            continue;
        }
        if let Ok(number) = lower[digits_from..cursor].parse::<u8>() {
            numbers.push(number);
        }
    }
    numbers
}

/// A byte that continues a name or a path, so `gate` beside it is not the word.
const fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/' | b'.')
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// What kind of document a corpus member is: `spec/PRD.md` section 4.20, row
/// Z-03 names three.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CorpusKind {
    /// A file under `spec/`.
    Specification,
    /// An operational memory record.
    OperationalMemory,
    /// An instruction file, such as a role or base instruction file.
    Instruction,
}

impl fmt::Display for CorpusKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Specification => "specification",
            Self::OperationalMemory => "operational memory",
            Self::Instruction => "instruction file",
        })
    }
}

/// One document to check: its name, its kind, its bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusDocument {
    /// The name the report cites: a repository-relative path with `/`
    /// separators, or a record identifier for operational memory.
    pub path: String,
    /// What kind of document it is.
    pub kind: CorpusKind,
    /// Its content, as bytes.
    pub bytes: Vec<u8>,
}

/// Why a file the reader met is not in the corpus: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnreadReason {
    /// A symbolic link; never followed.
    Symlink,
    /// Not a regular file, a directory or a link: a pipe, a socket, a device.
    NotARegularFile,
    /// Larger than [`ReadLimits::max_file_bytes`].
    TooLarge {
        /// How many bytes were read before the cap cut the read off.
        bytes: u64,
        /// The cap.
        cap: u64,
    },
    /// The operating system refused, in its words.
    Unreadable(String),
    /// A name that is not valid UTF-8, which a report cannot spell.
    NonUtf8Name,
    /// A path outside the repository root, or one that climbs out of it.
    OutsideRoot,
}

impl fmt::Display for UnreadReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symlink => f.write_str("a symbolic link, never followed"),
            Self::NotARegularFile => f.write_str("not a regular file"),
            Self::TooLarge { bytes, cap } => {
                write!(f, "too large ({bytes} bytes read, cap {cap})")
            }
            Self::Unreadable(why) => write!(f, "unreadable: {why}"),
            Self::NonUtf8Name => f.write_str("its name is not valid UTF-8"),
            Self::OutsideRoot => f.write_str("outside the repository root"),
        }
    }
}

/// A file the reader met and did not read, with the reason: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnreadFile {
    /// Its path, as far as it can be spelled.
    pub path: String,
    /// Why it was not read.
    pub reason: UnreadReason,
}

/// The bounds the readers enforce: AICD §39. The corpus may be a user's own
/// repository, so none of it is trusted to be small.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadLimits {
    /// The largest file read; a larger one is recorded as unread.
    pub max_file_bytes: u64,
    /// The most entries a tree may hold; a larger tree is refused.
    pub max_files: usize,
    /// The most bytes read in all; a larger tree is refused.
    pub max_total_bytes: u64,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            max_files: 10_000,
            max_total_bytes: 256 * 1024 * 1024,
        }
    }
}

/// The documents to check, and what the reader left out: AICD §39.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Corpus {
    documents: Vec<CorpusDocument>,
    unread: Vec<UnreadFile>,
}

impl Corpus {
    /// An empty corpus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one document the caller already holds, such as an operational
    /// memory record.
    pub fn push(&mut self, path: impl Into<String>, kind: CorpusKind, bytes: impl Into<Vec<u8>>) {
        self.documents.push(CorpusDocument {
            path: path.into(),
            kind,
            bytes: bytes.into(),
        });
    }

    /// The documents, in the order they were added or read.
    #[must_use]
    pub fn documents(&self) -> &[CorpusDocument] {
        &self.documents
    }

    /// The files a reader met and did not read.
    #[must_use]
    pub fn unread(&self) -> &[UnreadFile] {
        &self.unread
    }

    /// Reads every file under `<root>/spec`, whatever its extension, as the
    /// specification: E-0006 decided that HTML is checked too.
    ///
    /// Never writes, never follows a link. Documents come back in path order.
    ///
    /// # Errors
    ///
    /// [`CitationError::Io`] when `spec` itself cannot be listed,
    /// [`CitationError::CorpusTooLarge`] when the tree crosses a limit.
    pub fn read_specification(root: &Path, limits: &ReadLimits) -> Result<Self, CitationError> {
        let mut corpus = Self::new();
        let mut total = 0u64;
        let mut pending = vec![(root.join(SPECIFICATION_DIR), SPECIFICATION_DIR.to_owned())];
        let mut first = true;
        while let Some((directory, relative)) = pending.pop() {
            let listing = match fs::read_dir(&directory) {
                Ok(listing) => listing,
                Err(source) if first => {
                    return Err(CitationError::Io {
                        path: directory.display().to_string(),
                        source,
                    });
                }
                Err(source) => {
                    corpus.unread.push(UnreadFile {
                        path: relative,
                        reason: UnreadReason::Unreadable(source.to_string()),
                    });
                    continue;
                }
            };
            first = false;
            let mut entries = Vec::new();
            for entry in listing {
                match entry {
                    Ok(entry) => match entry.file_type() {
                        Ok(kind) => entries.push((entry.file_name(), entry.path(), kind)),
                        Err(source) => corpus.unread.push(UnreadFile {
                            path: format!("{relative}/{}", entry.file_name().to_string_lossy()),
                            reason: UnreadReason::Unreadable(source.to_string()),
                        }),
                    },
                    Err(source) => corpus.unread.push(UnreadFile {
                        path: relative.clone(),
                        reason: UnreadReason::Unreadable(source.to_string()),
                    }),
                }
            }
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (name, path, kind) in entries {
                let Some(name) = name.to_str() else {
                    corpus.unread.push(UnreadFile {
                        path: format!("{relative}/{}", name.to_string_lossy()),
                        reason: UnreadReason::NonUtf8Name,
                    });
                    continue;
                };
                let child = format!("{relative}/{name}");
                if corpus.documents.len() + corpus.unread.len() >= limits.max_files {
                    return Err(CitationError::CorpusTooLarge {
                        limit: CorpusLimit::Files(limits.max_files),
                    });
                }
                if kind.is_dir() {
                    pending.push((path, child));
                    continue;
                }
                match read_regular_file(&path, limits.max_file_bytes) {
                    Ok(bytes) => {
                        total += u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                        if total > limits.max_total_bytes {
                            return Err(CitationError::CorpusTooLarge {
                                limit: CorpusLimit::TotalBytes(limits.max_total_bytes),
                            });
                        }
                        corpus.documents.push(CorpusDocument {
                            path: child,
                            kind: CorpusKind::Specification,
                            bytes,
                        });
                    }
                    Err(reason) => corpus.unread.push(UnreadFile {
                        path: child,
                        reason,
                    }),
                }
            }
        }
        corpus.documents.sort_by(|a, b| a.path.cmp(&b.path));
        corpus.unread.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(corpus)
    }

    /// Reads the named files, relative to `root`, as documents of `kind`: the
    /// instruction files a caller names (`CLAUDE.md`, the role files).
    ///
    /// A path that is absolute or climbs out of `root` is recorded as unread
    /// ([`UnreadReason::OutsideRoot`]); so is any file the reader will not or
    /// cannot read. Never writes, never follows a link.
    #[must_use]
    pub fn read_files(
        root: &Path,
        relative_paths: &[&str],
        kind: CorpusKind,
        limits: &ReadLimits,
    ) -> Self {
        let mut corpus = Self::new();
        for relative in relative_paths {
            let escapes = Path::new(relative).is_absolute()
                || Path::new(relative)
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir));
            if escapes {
                corpus.unread.push(UnreadFile {
                    path: (*relative).to_owned(),
                    reason: UnreadReason::OutsideRoot,
                });
                continue;
            }
            match read_regular_file(&root.join(relative), limits.max_file_bytes) {
                Ok(bytes) => corpus.documents.push(CorpusDocument {
                    path: (*relative).to_owned(),
                    kind,
                    bytes,
                }),
                Err(reason) => corpus.unread.push(UnreadFile {
                    path: (*relative).to_owned(),
                    reason,
                }),
            }
        }
        corpus
    }

    /// Adds everything `other` holds, documents and unread files.
    pub fn extend(&mut self, other: Self) {
        self.documents.extend(other.documents);
        self.unread.extend(other.unread);
    }
}

/// Reads one regular file, at most `cap` bytes. Decides on the entry's own
/// metadata first, so a link, a pipe or a device is never opened.
fn read_regular_file(path: &Path, cap: u64) -> Result<Vec<u8>, UnreadReason> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| UnreadReason::Unreadable(error.to_string()))?;
    let kind = metadata.file_type();
    if kind.is_symlink() {
        return Err(UnreadReason::Symlink);
    }
    if !kind.is_file() {
        return Err(UnreadReason::NotARegularFile);
    }
    let file = fs::File::open(path).map_err(|error| UnreadReason::Unreadable(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| UnreadReason::Unreadable(error.to_string()))?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > cap {
        return Err(UnreadReason::TooLarge { bytes: read, cap });
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Exclusions
// ---------------------------------------------------------------------------

/// One document the check reads and does not judge, with the evidence for not
/// judging it: AICD §39, escalation E-0006.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exclusion {
    /// The document's path, matched exactly, relative to the repository root.
    pub path: &'static str,
    /// Text that must still be in the document. If it is gone, the reason for the
    /// exclusion has gone with it and the run is refused.
    pub marker: &'static str,
    /// Why the document is not judged.
    pub reason: &'static str,
    /// Whether the document must be in the corpus. True for this repository's
    /// own mockup, so the exclusion cannot quietly stand for nothing.
    pub required: bool,
}

/// The design mockup: escalation E-0006, option 2, answered 2026-09-27.
///
/// `spec/design/Ori Studio.html` is mock data about a fictional product. Its
/// marker is that product's name.
pub const DESIGN_MOCKUP: Exclusion = Exclusion {
    path: "spec/design/Ori Studio.html",
    marker: "Ledgerline",
    reason: "a design mockup whose citations are sample data about a fictional product, \
             one of them a picture of this check refusing a bad citation",
    required: true,
};

/// The documents a check reads and does not judge: AICD §39.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Exclusions {
    list: Vec<Exclusion>,
}

impl Exclusions {
    /// Nothing excluded: every document is judged. What a product that has no
    /// such mockup passes.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// This repository's list: the design mockup, by name, per E-0006.
    #[must_use]
    pub fn ori_studio() -> Self {
        Self {
            list: vec![DESIGN_MOCKUP],
        }
    }

    /// Adds one exclusion.
    #[must_use]
    pub fn with(mut self, exclusion: Exclusion) -> Self {
        self.list.push(exclusion);
        self
    }

    /// The exclusions, in order.
    pub fn iter(&self) -> std::slice::Iter<'_, Exclusion> {
        self.list.iter()
    }
}

// ---------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------

/// A citation that resolves in no heading of the methodology: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnresolvedCitation {
    /// The document it is written in.
    pub path: String,
    /// What kind of document that is.
    pub kind: CorpusKind,
    /// The 1-based line.
    pub line: usize,
    /// The number it names, as [`SectionIndex::contains`] takes it.
    pub number: String,
}

impl fmt::Display for UnresolvedCitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} is not a heading the methodology has",
            self.path,
            self.line,
            spell(&self.number)
        )
    }
}

/// The drafting-error rate: unresolved citations over all citations, with both
/// counts. `spec/PRD.md` section 4.20, row Z-03.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DraftingErrorRate {
    /// How many citations did not resolve.
    pub unresolved: usize,
    /// How many citations there were.
    pub total: usize,
}

impl DraftingErrorRate {
    /// The rate as a fraction, or `None` when there was no citation at all: no
    /// citations is not a rate of zero.
    #[must_use]
    pub fn fraction(&self) -> Option<f64> {
        if self.total == 0 {
            None
        } else {
            Some(self.unresolved as f64 / self.total as f64)
        }
    }

    /// The rate in basis points (hundredths of a percent), rounded down, for a
    /// caller that records integers; `None` when there was no citation.
    #[must_use]
    pub fn basis_points(&self) -> Option<u64> {
        if self.total == 0 {
            return None;
        }
        let points = (self.unresolved as u128).saturating_mul(10_000) / self.total as u128;
        u64::try_from(points).ok()
    }
}

impl fmt::Display for DraftingErrorRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.fraction() {
            Some(fraction) => write!(
                f,
                "{} of {} citations do not resolve ({:.2}%)",
                self.unresolved,
                self.total,
                fraction * 100.0
            ),
            None => f.write_str("no citation found: the drafting-error rate is undefined"),
        }
    }
}

/// A document read and not judged, with why and how much was left unjudged:
/// AICD §39, escalation E-0006.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExcludedDocument {
    /// The document's path.
    pub path: String,
    /// Why it is not judged.
    pub reason: String,
    /// The marker that was found in it on this run.
    pub marker: String,
    /// How many citations it holds that were neither resolved nor counted.
    pub citations_not_judged: usize,
}

/// What one check found: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CitationReport {
    /// How many documents were judged (excluded ones are not).
    pub documents_checked: usize,
    /// The drafting-error rate over the judged documents.
    pub rate: DraftingErrorRate,
    /// Every citation that did not resolve, in corpus order then line order.
    pub unresolved: Vec<UnresolvedCitation>,
    /// How many gate citations were found, so that "no gate flagged" and "no gate
    /// cited" are never the same answer.
    pub gate_citations: usize,
    /// Every gate cited as protection that is not proven: criterion ORI-P1-012.
    pub unproven_gates: Vec<UnprovenGateCitation>,
    /// Every document read and not judged.
    pub excluded: Vec<ExcludedDocument>,
    /// Every file the reader left out; the check is not clean while any exists.
    pub unread: Vec<UnreadFile>,
}

impl CitationReport {
    /// Whether the check found nothing wrong and nothing was left out: no
    /// unresolved citation, no unproven gate cited, no unread file.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.unresolved.is_empty() && self.unproven_gates.is_empty() && self.unread.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The checker
// ---------------------------------------------------------------------------

/// The citation checker: AICD §39, AICD §14, `spec/PRD.md` section 4.20 row Z-03.
#[derive(Clone, Debug)]
pub struct CitationChecker {
    index: SectionIndex,
    gates: GateStandings,
    exclusions: Exclusions,
}

impl CitationChecker {
    /// A checker over `index`, with the gate standings the caller reports and the
    /// exclusions it applies.
    ///
    /// # Errors
    ///
    /// [`CitationError::ExclusionMarkerEmpty`] when an exclusion's marker is
    /// empty.
    pub fn new(
        index: SectionIndex,
        gates: GateStandings,
        exclusions: Exclusions,
    ) -> Result<Self, CitationError> {
        if let Some(blank) = exclusions
            .iter()
            .find(|exclusion| exclusion.marker.is_empty())
        {
            return Err(CitationError::ExclusionMarkerEmpty {
                path: blank.path.to_owned(),
            });
        }
        Ok(Self {
            index,
            gates,
            exclusions,
        })
    }

    /// Checks `corpus`.
    ///
    /// Before anything is judged, every exclusion is checked against the corpus:
    /// a required document must be present, and a present one must still carry its
    /// marker. Then every judged document's citations are resolved and every gate
    /// it cites as protection is looked up in the standings the caller gave.
    ///
    /// # Errors
    ///
    /// [`CitationError::EmptyCorpus`] when the corpus has no document;
    /// [`CitationError::ExclusionMarkerGone`] and
    /// [`CitationError::ExcludedDocumentMissing`] when an exclusion no longer
    /// holds.
    pub fn check(&self, corpus: &Corpus) -> Result<CitationReport, CitationError> {
        if corpus.documents.is_empty() {
            return Err(CitationError::EmptyCorpus);
        }
        let mut excluded_paths: BTreeSet<&str> = BTreeSet::new();
        for exclusion in self.exclusions.iter() {
            let found = corpus
                .documents
                .iter()
                .find(|document| document.path == exclusion.path);
            match found {
                Some(document) => {
                    if !String::from_utf8_lossy(&document.bytes).contains(exclusion.marker) {
                        return Err(CitationError::ExclusionMarkerGone {
                            path: exclusion.path.to_owned(),
                            marker: exclusion.marker.to_owned(),
                            reason: exclusion.reason.to_owned(),
                        });
                    }
                    excluded_paths.insert(exclusion.path);
                }
                None if exclusion.required => {
                    return Err(CitationError::ExcludedDocumentMissing {
                        path: exclusion.path.to_owned(),
                        reason: exclusion.reason.to_owned(),
                    });
                }
                None => {}
            }
        }

        let mut report = CitationReport {
            documents_checked: 0,
            rate: DraftingErrorRate {
                unresolved: 0,
                total: 0,
            },
            unresolved: Vec::new(),
            gate_citations: 0,
            unproven_gates: Vec::new(),
            excluded: Vec::new(),
            unread: corpus.unread.clone(),
        };
        for document in &corpus.documents {
            if excluded_paths.contains(document.path.as_str()) {
                if let Some(exclusion) = self
                    .exclusions
                    .iter()
                    .find(|exclusion| exclusion.path == document.path)
                {
                    report.excluded.push(ExcludedDocument {
                        path: document.path.clone(),
                        reason: exclusion.reason.to_owned(),
                        marker: exclusion.marker.to_owned(),
                        citations_not_judged: find_citations(&document.bytes).len(),
                    });
                }
                continue;
            }
            report.documents_checked += 1;
            for citation in find_citations(&document.bytes) {
                report.rate.total += 1;
                if !self.index.contains(&citation.number) {
                    report.rate.unresolved += 1;
                    report.unresolved.push(UnresolvedCitation {
                        path: document.path.clone(),
                        kind: document.kind,
                        line: citation.line,
                        number: citation.number,
                    });
                }
            }
            let text = String::from_utf8_lossy(&document.bytes);
            for (line, gate) in find_gate_citations(&text) {
                report.gate_citations += 1;
                let finding = match self.gates.get(gate) {
                    Some(GateStanding::Proven | GateStanding::Installed) => continue,
                    Some(GateStanding::Defined) => GateFinding::Unproven,
                    Some(GateStanding::Inert) => GateFinding::Inert,
                    None => GateFinding::Unknown,
                };
                report.unproven_gates.push(UnprovenGateCitation {
                    path: document.path.clone(),
                    kind: document.kind,
                    line,
                    gate,
                    finding,
                });
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use super::*;

    // -----------------------------------------------------------------
    // Fixtures. Every citation a test needs to be unresolvable is built here
    // at run time from parts, never written whole in this file: the
    // repository's own citation test reads this file like every other, and
    // a bad citation written whole is a bad citation in the tree (E-0006,
    // "The lead committed this defect while documenting it").
    // -----------------------------------------------------------------

    /// `AICD` and the section sign and `number`, joined at run time.
    fn cite(number: &str) -> String {
        format!("AICD {}{number}", '\u{a7}')
    }

    /// `AICD appendix` and `letter`, joined at run time.
    fn cite_appendix(letter: &str) -> String {
        format!("AICD appendix {letter}")
    }

    /// A citation of a section the methodology does not have.
    fn unresolvable() -> String {
        cite("99")
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .canonicalize()
            .expect("the repository root exists")
    }

    fn real_index() -> SectionIndex {
        SectionIndex::from_file(&repo_root().join(SECTION_INDEX_PATH))
            .expect("the committed methodology index reads")
    }

    fn standings(entries: &[(u8, GateStanding)]) -> GateStandings {
        let mut gates = GateStandings::new();
        for (number, standing) in entries {
            gates.set(*number, *standing);
        }
        gates
    }

    fn checker(gates: GateStandings, exclusions: Exclusions) -> CitationChecker {
        CitationChecker::new(real_index(), gates, exclusions).expect("the checker builds")
    }

    fn spec_doc(path: &str, text: &str) -> Corpus {
        let mut corpus = Corpus::new();
        corpus.push(path, CorpusKind::Specification, text);
        corpus
    }

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0040-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a scratch directory can be made");
            Self { path }
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let target = self.path.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("parent directories can be made");
            }
            fs::write(target, bytes).expect("a scratch file can be written");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // -----------------------------------------------------------------
    // The section index, read as data
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_the_index_reads_the_committed_methodology_index() {
        let index = real_index();
        assert_eq!(index.section_count(), 40);
        assert_eq!(index.len(), 56, "40 sections, 3 appendices, 13 subsections");
        assert!(!index.is_empty());
        for number in ["1", "7", "14", "24.2", "39", "40", "A", "A.5", "C"] {
            assert!(index.contains(number), "{number} is a heading");
        }
        for number in ["0", "41", "99", "24.9", "25.3", "D", "A.6", ""] {
            assert!(!index.contains(number), "{number:?} is not a heading");
        }
    }

    #[test]
    fn ori_t_0040_a_malformed_index_is_refused_not_read_as_empty() {
        let refused = |json: &str| SectionIndex::from_json(json).expect_err("refused");
        assert!(matches!(
            refused("not json"),
            CitationError::IndexMalformed { .. }
        ));
        assert!(matches!(
            refused(r#"{"entries": 3}"#),
            CitationError::IndexMalformed { .. }
        ));
        assert!(matches!(
            refused(r#"{"entries": []}"#),
            CitationError::IndexEmpty
        ));
        assert!(
            matches!(
                refused(r#"{"entries": [{"number": "A", "kind": "appendix"}]}"#),
                CitationError::IndexEmpty
            ),
            "an index with no numbered section resolves nothing a citation names"
        );
        assert!(matches!(
            refused(r#"{"counts": {"total": 2}, "entries": [{"number": "1", "kind": "section"}]}"#),
            CitationError::IndexMalformed { .. }
        ));
        assert!(matches!(
            refused(
                r#"{"entries": [{"number": "1", "kind": "section"}, {"number": "1", "kind": "section"}]}"#
            ),
            CitationError::IndexMalformed { .. }
        ));
        assert!(matches!(
            refused(r#"{"entries": [{"number": " ", "kind": "section"}]}"#),
            CitationError::IndexMalformed { .. }
        ));
        let missing = SectionIndex::from_file(Path::new("/nonexistent/ori-t-0040/sections.json"))
            .expect_err("a missing file is an error, not an empty index");
        assert!(matches!(missing, CitationError::Io { .. }));
    }

    // -----------------------------------------------------------------
    // The scanner (the grammar, restated)
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_the_scanner_collects_the_citation_form_conventions_fixes_and_no_other() {
        let sign = '\u{a7}';
        let source = format!(
            "AICD {sign}7 and AICD {sign}24.2 and AICD appendix A and AICD appendix A.5\n\
             AICD &sect;39 written as an entity, AICD Appendix c in lower case\n\
             a bare {sign}4.2 belongs to another document, AICDLike is a word\n\
             AICD appendix Alpha names no appendix, AICD {sign}x has no number\n"
        );
        let found = find_citations(source.as_bytes());
        let numbers: Vec<&str> = found.iter().map(|c| c.number.as_str()).collect();
        assert_eq!(numbers, ["7", "24.2", "A", "A.5", "39", "C"]);
        assert_eq!(found[0].line, 1, "first line");
        assert_eq!(found[4].line, 2, "the entity form is on line 2");
        assert_eq!(found[5].line, 2, "the lower-case appendix is on line 2");
    }

    #[test]
    fn ori_t_0040_a_citation_split_across_code_spans_is_not_one() {
        // The convention E-0006 records for quoting a bad citation.
        let quoted = format!("`AICD` `{}{}`", '\u{a7}', "99");
        assert!(find_citations(quoted.as_bytes()).is_empty());
    }

    #[test]
    fn ori_t_0040_a_citation_in_undecodable_bytes_is_still_found() {
        let mut bytes = vec![0xff, 0xfe, 0x00];
        bytes.extend_from_slice(unresolvable().as_bytes());
        bytes.push(0x80);
        let found = find_citations(&bytes);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].number, "99");
        assert!(!real_index().contains("99"));
    }

    #[test]
    fn ori_t_0040_a_citation_at_the_end_of_a_sentence_keeps_no_fraction() {
        let text = format!(
            "See {}. Then {}.2 and {}.",
            cite("14"),
            cite("24"),
            cite("39")
        );
        let numbers: Vec<String> = find_citations(text.as_bytes())
            .into_iter()
            .map(|c| c.number)
            .collect();
        assert_eq!(numbers, ["14", "24.2", "39"]);
    }

    // -----------------------------------------------------------------
    // Z-03: resolve every citation, report each unresolved one
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_a_resolving_and_a_non_resolving_citation_are_told_apart_with_file_and_line() {
        let text = format!(
            "# A document\n\nIt rests on {} and {}.\nIt also rests on {}.\n",
            cite("7"),
            cite("24.2"),
            unresolvable()
        );
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&spec_doc("spec/PRD.md", &text))
            .expect("checks");
        assert_eq!(report.documents_checked, 1);
        assert_eq!(
            report.unresolved,
            [UnresolvedCitation {
                path: "spec/PRD.md".to_owned(),
                kind: CorpusKind::Specification,
                line: 4,
                number: "99".to_owned(),
            }]
        );
        assert_eq!(
            report.unresolved[0].to_string(),
            format!(
                "spec/PRD.md:4: {} is not a heading the methodology has",
                unresolvable()
            )
        );
        assert!(!report.is_clean());
    }

    #[test]
    fn ori_t_0040_a_subsection_and_an_appendix_the_methodology_lacks_do_not_resolve() {
        let text = format!(
            "{} {} {} {}",
            cite("24.9"),
            cite("25.3"),
            cite_appendix("Z"),
            cite_appendix("A.6")
        );
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&spec_doc("spec/PRD.md", &text))
            .expect("checks");
        let numbers: Vec<&str> = report
            .unresolved
            .iter()
            .map(|c| c.number.as_str())
            .collect();
        assert_eq!(numbers, ["24.9", "25.3", "Z", "A.6"]);
    }

    #[test]
    fn ori_t_0040_every_kind_of_document_is_judged_and_reports_its_kind() {
        let bad = format!("{}\n", unresolvable());
        let mut corpus = Corpus::new();
        corpus.push("spec/PRD.md", CorpusKind::Specification, bad.as_str());
        corpus.push("memory:REC-1", CorpusKind::OperationalMemory, bad.as_str());
        corpus.push("CLAUDE.md", CorpusKind::Instruction, bad.as_str());
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&corpus)
            .expect("checks");
        let kinds: Vec<CorpusKind> = report.unresolved.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [
                CorpusKind::Specification,
                CorpusKind::OperationalMemory,
                CorpusKind::Instruction
            ]
        );
    }

    #[test]
    fn ori_t_0040_a_check_over_no_document_is_refused_not_reported_clean() {
        let refused = checker(GateStandings::new(), Exclusions::none())
            .check(&Corpus::new())
            .expect_err("an empty corpus is refused");
        assert!(matches!(refused, CitationError::EmptyCorpus));
    }

    // -----------------------------------------------------------------
    // Z-03: the drafting-error rate, unresolved over total, both counts
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_the_drafting_error_rate_counts_unresolved_over_total() {
        let mut corpus = Corpus::new();
        corpus.push(
            "spec/LLD.md",
            CorpusKind::Specification,
            format!("{} {} {}", cite("7"), cite("14"), unresolvable()),
        );
        corpus.push("memory:REC-2", CorpusKind::OperationalMemory, cite("39"));
        corpus.push(
            "CLAUDE.md",
            CorpusKind::Instruction,
            format!("{} {}", unresolvable(), cite("98")),
        );
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&corpus)
            .expect("checks");
        assert_eq!(report.rate.total, 6);
        assert_eq!(report.rate.unresolved, 3);
        assert_eq!(report.rate.fraction(), Some(0.5));
        assert_eq!(report.rate.basis_points(), Some(5_000));
        assert_eq!(report.unresolved.len(), report.rate.unresolved);
        assert_eq!(
            report.rate.to_string(),
            "3 of 6 citations do not resolve (50.00%)"
        );
    }

    #[test]
    fn ori_t_0040_a_corpus_with_no_citation_has_no_rate_and_not_a_rate_of_zero() {
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&spec_doc("spec/PRD.md", "No citation here.\n"))
            .expect("checks");
        assert_eq!(report.rate.total, 0);
        assert_eq!(report.rate.fraction(), None);
        assert_eq!(report.rate.basis_points(), None);
        assert!(report.rate.to_string().contains("undefined"));
        assert_eq!(report.documents_checked, 1, "the document was still judged");
    }

    #[test]
    fn ori_t_0040_the_rate_rounds_down_and_never_overflows() {
        let rate = DraftingErrorRate {
            unresolved: 1,
            total: 3,
        };
        assert_eq!(rate.basis_points(), Some(3_333));
        let huge = DraftingErrorRate {
            unresolved: usize::MAX,
            total: usize::MAX,
        };
        assert_eq!(huge.basis_points(), Some(10_000));
    }

    // -----------------------------------------------------------------
    // E-0006: the design mockup, excluded by name, its marker asserted
    // -----------------------------------------------------------------

    fn mockup_text(with_marker: bool) -> String {
        let marker = if with_marker { "Ledgerline" } else { "Gone" };
        format!(
            "<html>{marker} rows: {} and {} and {}</html>",
            unresolvable(),
            cite("17.9"),
            cite("7")
        )
    }

    #[test]
    fn ori_t_0040_the_design_mockup_exclusion_is_the_one_the_operator_decided() {
        let exclusions = Exclusions::ori_studio();
        let listed: Vec<&Exclusion> = exclusions.iter().collect();
        assert_eq!(listed.len(), 1, "this repository excludes exactly one file");
        let mockup = listed[0];
        assert_eq!(mockup.path, "spec/design/Ori Studio.html");
        assert_eq!(mockup.marker, "Ledgerline");
        assert!(mockup.required);
        assert!(!mockup.reason.is_empty());
    }

    #[test]
    fn ori_t_0040_the_design_mockup_is_excluded_and_its_marker_is_asserted() {
        let mut corpus = Corpus::new();
        corpus.push(
            DESIGN_MOCKUP.path,
            CorpusKind::Specification,
            mockup_text(true),
        );
        corpus.push("spec/PRD.md", CorpusKind::Specification, cite("7"));
        let report = checker(GateStandings::new(), Exclusions::ori_studio())
            .check(&corpus)
            .expect("checks with the marker present");
        assert!(report.unresolved.is_empty(), "the mockup is not judged");
        assert_eq!(report.documents_checked, 1);
        assert_eq!(
            report.rate,
            DraftingErrorRate {
                unresolved: 0,
                total: 1
            },
            "the mockup's citations are not in the rate"
        );
        assert_eq!(report.excluded.len(), 1);
        assert_eq!(report.excluded[0].path, DESIGN_MOCKUP.path);
        assert_eq!(report.excluded[0].marker, "Ledgerline");
        assert_eq!(report.excluded[0].citations_not_judged, 3);
        assert!(report.is_clean());
    }

    #[test]
    fn ori_t_0040_the_run_fails_when_the_mockup_marker_is_gone() {
        let mut corpus = Corpus::new();
        corpus.push(
            DESIGN_MOCKUP.path,
            CorpusKind::Specification,
            mockup_text(false),
        );
        corpus.push("spec/PRD.md", CorpusKind::Specification, cite("7"));
        let refused = checker(GateStandings::new(), Exclusions::ori_studio())
            .check(&corpus)
            .expect_err("an exclusion that outlived its subject refuses the run");
        match refused {
            CitationError::ExclusionMarkerGone { path, marker, .. } => {
                assert_eq!(path, DESIGN_MOCKUP.path);
                assert_eq!(marker, "Ledgerline");
            }
            other => panic!("expected ExclusionMarkerGone, got {other}"),
        }
    }

    #[test]
    fn ori_t_0040_a_required_exclusion_whose_file_is_not_in_the_corpus_refuses_the_run() {
        let refused = checker(GateStandings::new(), Exclusions::ori_studio())
            .check(&spec_doc("spec/PRD.md", &cite("7")))
            .expect_err("the mockup is required here");
        assert!(matches!(
            refused,
            CitationError::ExcludedDocumentMissing { .. }
        ));
        // A product that has no such file passes no exclusion and is judged
        // over everything.
        checker(GateStandings::new(), Exclusions::none())
            .check(&spec_doc("spec/PRD.md", &cite("7")))
            .expect("no exclusion, no requirement");
    }

    #[test]
    fn ori_t_0040_the_exclusion_is_by_exact_name_and_nothing_else_is_exempt() {
        let mut corpus = Corpus::new();
        corpus.push(
            DESIGN_MOCKUP.path,
            CorpusKind::Specification,
            mockup_text(true),
        );
        // Same format, same marker, different name: judged.
        corpus.push(
            "spec/design/Other.html",
            CorpusKind::Specification,
            mockup_text(true),
        );
        // Same name, different case: judged.
        corpus.push(
            "spec/design/ori studio.html",
            CorpusKind::Specification,
            mockup_text(true),
        );
        let report = checker(GateStandings::new(), Exclusions::ori_studio())
            .check(&corpus)
            .expect("checks");
        let judged: BTreeSet<&str> = report.unresolved.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            judged,
            BTreeSet::from(["spec/design/Other.html", "spec/design/ori studio.html"])
        );
        assert_eq!(report.excluded.len(), 1);
    }

    #[test]
    fn ori_t_0040_an_exclusion_with_an_empty_marker_is_refused_when_the_checker_is_built() {
        let blank = Exclusion {
            path: "spec/design/Mock.html",
            marker: "",
            reason: "a mockup",
            required: false,
        };
        let refused = CitationChecker::new(
            real_index(),
            GateStandings::new(),
            Exclusions::none().with(blank),
        )
        .expect_err("an empty marker asserts nothing");
        assert!(matches!(
            refused,
            CitationError::ExclusionMarkerEmpty { .. }
        ));
    }

    // -----------------------------------------------------------------
    // ORI-P1-012: a gate cited as protection that is not proven is flagged
    // -----------------------------------------------------------------

    const GATE_DOC: &str = "The merge path is protected by gate 9.\n\
                            Gate 13 catches a missing trailer.\n\
                            Gate 9 is not installed yet, so it protects nothing.\n\
                            See fixtures/planted/gate-9 for the guard.\n\
                            Gate 6 guards the build, and gate 12 blocks a release.\n";

    #[test]
    fn ori_p1_012_a_document_citing_an_unproven_gate_as_protection_is_flagged() {
        let gates = standings(&[
            (9, GateStanding::Defined),
            (13, GateStanding::Installed),
            (6, GateStanding::Inert),
        ]);
        let report = checker(gates, Exclusions::none())
            .check(&spec_doc("spec/CI_CD.md", GATE_DOC))
            .expect("checks");
        let flagged: Vec<(usize, u8, GateFinding)> = report
            .unproven_gates
            .iter()
            .map(|c| (c.line, c.gate, c.finding))
            .collect();
        assert_eq!(
            flagged,
            [
                (1, 9, GateFinding::Unproven),
                (5, 6, GateFinding::Inert),
                (5, 12, GateFinding::Unknown),
            ]
        );
        assert_eq!(
            report.gate_citations, 4,
            "four gate citations were found: the proven one is counted, the hedged and the path are not"
        );
        assert!(!report.is_clean());
        assert_eq!(
            report.unproven_gates[0].to_string(),
            "spec/CI_CD.md:1: gate 9 is cited as protection and is defined, with no proof that it \
             fails on a planted defect"
        );
    }

    #[test]
    fn ori_p1_012_the_gate_state_is_the_callers_and_a_proof_clears_the_flag() {
        let text = "The merge path is protected by gate 9.\n";
        let flagged = |standing: Option<GateStanding>| {
            let mut gates = GateStandings::new();
            if let Some(standing) = standing {
                gates.set(9, standing);
            }
            checker(gates, Exclusions::none())
                .check(&spec_doc("spec/CI_CD.md", text))
                .expect("checks")
                .unproven_gates
                .len()
        };
        assert_eq!(flagged(Some(GateStanding::Defined)), 1);
        assert_eq!(flagged(Some(GateStanding::Inert)), 1);
        assert_eq!(
            flagged(None),
            1,
            "no standing reported is never read as proven"
        );
        assert_eq!(flagged(Some(GateStanding::Proven)), 0);
        assert_eq!(flagged(Some(GateStanding::Installed)), 0);
    }

    #[test]
    fn ori_p1_012_the_gate_reader_takes_the_protection_sentences_and_leaves_the_rest() {
        let cases: [(&str, &[u8]); 12] = [
            ("Gate 7 protects the dependency graph.", &[7]),
            ("The build is guarded by Gate 11.", &[11]),
            ("It was caught by gate 2 and gate 13 as well.", &[2, 13]),
            ("Gate 9 isn't protecting anything.", &[]),
            ("Gate 9 will protect every merge.", &[]),
            ("fixtures/planted/gate-7 guards the proof.", &[]),
            ("Gate 7.2 protects nothing named here.", &[]),
            ("Gate 7a protects.", &[]),
            ("The word gateway protects.", &[]),
            ("Gate 9 is the plan. It protects nothing yet.", &[]),
            ("Gate 3 is the third gate. Gate 4 enforces the rule.", &[4]),
            ("This sentence guards but names no gate.", &[]),
        ];
        for (text, expected) in cases {
            let found: Vec<u8> = find_gate_citations(text)
                .into_iter()
                .map(|(_, gate)| gate)
                .collect();
            assert_eq!(found, expected, "for {text:?}");
        }
    }

    // -----------------------------------------------------------------
    // The readers: whole of spec/, HTML included; nothing skipped quietly
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_the_specification_reader_reads_every_extension_not_only_markdown() {
        let scratch = Scratch::new("every-extension");
        scratch.write("spec/PRD.md", b"markdown");
        scratch.write("spec/design/Ori Studio.html", b"<html>html</html>");
        scratch.write("spec/notes.txt", b"text");
        scratch.write("spec/deep/er/data.bin", &[0xff, 0x00, 0xfe]);
        scratch.write("elsewhere/other.md", b"outside spec/");
        let corpus =
            Corpus::read_specification(&scratch.path, &ReadLimits::default()).expect("reads");
        let paths: Vec<&str> = corpus.documents().iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "spec/PRD.md",
                "spec/deep/er/data.bin",
                "spec/design/Ori Studio.html",
                "spec/notes.txt",
            ]
        );
        assert!(
            corpus
                .documents()
                .iter()
                .all(|d| d.kind == CorpusKind::Specification)
        );
        assert!(corpus.unread().is_empty());
    }

    #[test]
    fn ori_t_0040_a_citation_in_an_html_file_under_spec_is_found() {
        let scratch = Scratch::new("html-citation");
        scratch.write(
            "spec/design/Page.html",
            format!("<p>{}</p>", unresolvable()).as_bytes(),
        );
        let corpus =
            Corpus::read_specification(&scratch.path, &ReadLimits::default()).expect("reads");
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&corpus)
            .expect("checks");
        assert_eq!(report.unresolved.len(), 1);
        assert_eq!(report.unresolved[0].path, "spec/design/Page.html");
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0040_a_symbolic_link_is_never_followed_and_is_reported_unread() {
        let scratch = Scratch::new("symlink");
        scratch.write("spec/PRD.md", b"real");
        std::os::unix::fs::symlink(
            scratch.path.join("spec/PRD.md"),
            scratch.path.join("spec/LLD.md"),
        )
        .expect("a link can be made");
        let corpus =
            Corpus::read_specification(&scratch.path, &ReadLimits::default()).expect("reads");
        assert_eq!(corpus.documents().len(), 1);
        assert_eq!(
            corpus.unread(),
            [UnreadFile {
                path: "spec/LLD.md".to_owned(),
                reason: UnreadReason::Symlink,
            }]
        );
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&corpus)
            .expect("checks");
        assert!(
            !report.is_clean(),
            "a file left unread is never a clean check"
        );
        assert_eq!(report.unread.len(), 1);
    }

    #[test]
    fn ori_t_0040_a_file_over_the_cap_is_reported_unread_and_a_tree_over_a_limit_is_refused() {
        let scratch = Scratch::new("limits");
        scratch.write("spec/PRD.md", b"ok");
        scratch.write("spec/LLD.md", &[b'x'; 64]);
        let limits = ReadLimits {
            max_file_bytes: 16,
            ..ReadLimits::default()
        };
        let corpus = Corpus::read_specification(&scratch.path, &limits).expect("reads");
        assert_eq!(corpus.documents().len(), 1);
        assert_eq!(corpus.unread().len(), 1);
        assert_eq!(corpus.unread()[0].path, "spec/LLD.md");
        assert!(matches!(
            corpus.unread()[0].reason,
            UnreadReason::TooLarge { cap: 16, .. }
        ));

        let few = ReadLimits {
            max_files: 1,
            ..ReadLimits::default()
        };
        assert!(matches!(
            Corpus::read_specification(&scratch.path, &few),
            Err(CitationError::CorpusTooLarge {
                limit: CorpusLimit::Files(1)
            })
        ));
        let light = ReadLimits {
            max_total_bytes: 8,
            max_file_bytes: 1_000,
            ..ReadLimits::default()
        };
        assert!(matches!(
            Corpus::read_specification(&scratch.path, &light),
            Err(CitationError::CorpusTooLarge {
                limit: CorpusLimit::TotalBytes(8)
            })
        ));
    }

    #[test]
    fn ori_t_0040_a_missing_specification_directory_is_an_error_not_an_empty_corpus() {
        let scratch = Scratch::new("no-spec");
        let refused = Corpus::read_specification(&scratch.path, &ReadLimits::default())
            .expect_err("no spec/ here");
        assert!(matches!(refused, CitationError::Io { .. }));
    }

    #[test]
    fn ori_t_0040_instruction_files_are_read_by_name_and_escapes_are_refused() {
        let scratch = Scratch::new("instructions");
        scratch.write("CLAUDE.md", b"base");
        scratch.write("spec/agents/coder.md", b"coder");
        let corpus = Corpus::read_files(
            &scratch.path,
            &[
                "CLAUDE.md",
                "spec/agents/coder.md",
                "../outside.md",
                "/etc/passwd",
                "absent.md",
            ],
            CorpusKind::Instruction,
            &ReadLimits::default(),
        );
        let read: Vec<&str> = corpus.documents().iter().map(|d| d.path.as_str()).collect();
        assert_eq!(read, ["CLAUDE.md", "spec/agents/coder.md"]);
        assert!(
            corpus
                .documents()
                .iter()
                .all(|d| d.kind == CorpusKind::Instruction)
        );
        let reasons: Vec<(&str, bool)> = corpus
            .unread()
            .iter()
            .map(|u| {
                (
                    u.path.as_str(),
                    matches!(u.reason, UnreadReason::OutsideRoot),
                )
            })
            .collect();
        assert_eq!(
            reasons,
            [
                ("../outside.md", true),
                ("/etc/passwd", true),
                ("absent.md", false)
            ]
        );
    }

    #[test]
    fn ori_t_0040_reading_and_checking_write_nothing_to_the_working_tree() {
        fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
            let mut files = BTreeMap::new();
            let mut pending = vec![root.to_path_buf()];
            while let Some(directory) = pending.pop() {
                for entry in fs::read_dir(&directory).expect("lists") {
                    let entry = entry.expect("entry");
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                    } else {
                        files.insert(
                            path.strip_prefix(root)
                                .expect("under root")
                                .display()
                                .to_string(),
                            fs::read(&path).expect("reads"),
                        );
                    }
                }
            }
            files
        }
        let scratch = Scratch::new("no-writes");
        scratch.write("spec/PRD.md", unresolvable().as_bytes());
        scratch.write("CLAUDE.md", b"base");
        let before = snapshot(&scratch.path);
        let mut corpus =
            Corpus::read_specification(&scratch.path, &ReadLimits::default()).expect("reads");
        corpus.extend(Corpus::read_files(
            &scratch.path,
            &["CLAUDE.md"],
            CorpusKind::Instruction,
            &ReadLimits::default(),
        ));
        checker(GateStandings::new(), Exclusions::none())
            .check(&corpus)
            .expect("checks");
        assert_eq!(snapshot(&scratch.path), before);
    }

    // -----------------------------------------------------------------
    // This repository
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0040_this_repositorys_specification_resolves_with_only_the_mockup_excluded() {
        let corpus = Corpus::read_specification(&repo_root(), &ReadLimits::default())
            .expect("this repository's spec/ reads");
        let report = checker(GateStandings::new(), Exclusions::ori_studio())
            .check(&corpus)
            .expect("the mockup is in the tree and still carries its marker");
        // The floors: a sweep that read nothing must not pass.
        assert!(
            report.documents_checked >= 20,
            "the sweep judged only {} documents",
            report.documents_checked
        );
        assert!(
            report.rate.total >= 50,
            "the sweep found only {} citations, which means the scanner is broken or the tree is",
            report.rate.total
        );
        assert!(
            report.unresolved.is_empty(),
            "unresolved citations under spec/:\n  {}",
            report
                .unresolved
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n  ")
        );
        assert!(report.unread.is_empty(), "unread: {:?}", report.unread);
        assert_eq!(report.excluded.len(), 1);
        assert_eq!(report.excluded[0].path, DESIGN_MOCKUP.path);
        assert!(
            report.excluded[0].citations_not_judged > 0,
            "the HTML was read and its citations were found, and left unjudged"
        );
        println!(
            "spec/: {} documents, {}; {} citations left unjudged in {}",
            report.documents_checked,
            report.rate,
            report.excluded[0].citations_not_judged,
            report.excluded[0].path
        );
    }

    #[test]
    fn ori_t_0040_without_the_exclusion_every_unresolved_citation_under_spec_is_in_the_mockup() {
        let corpus = Corpus::read_specification(&repo_root(), &ReadLimits::default())
            .expect("this repository's spec/ reads");
        let report = checker(GateStandings::new(), Exclusions::none())
            .check(&corpus)
            .expect("checks");
        assert!(
            report
                .unresolved
                .iter()
                .all(|citation| citation.path == DESIGN_MOCKUP.path),
            "outside the mockup: {:?}",
            report
                .unresolved
                .iter()
                .filter(|citation| citation.path != DESIGN_MOCKUP.path)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
    }

    // -----------------------------------------------------------------
    // ORI-P1-033: every refusal carries a MethodologyRef that resolves
    // -----------------------------------------------------------------

    /// One name per variant, matched without a wildcard so that a variant
    /// added later stops the build until a sample is written for it.
    fn variant_name(error: &CitationError) -> &'static str {
        match error {
            CitationError::IndexMalformed { .. } => "IndexMalformed",
            CitationError::IndexEmpty => "IndexEmpty",
            CitationError::Io { .. } => "Io",
            CitationError::EmptyCorpus => "EmptyCorpus",
            CitationError::ExclusionMarkerEmpty { .. } => "ExclusionMarkerEmpty",
            CitationError::ExclusionMarkerGone { .. } => "ExclusionMarkerGone",
            CitationError::ExcludedDocumentMissing { .. } => "ExcludedDocumentMissing",
            CitationError::CorpusTooLarge { .. } => "CorpusTooLarge",
        }
    }

    #[test]
    fn ori_p1_033_every_citation_error_carries_a_methodology_ref_that_resolves() {
        let index = real_index();
        let samples = vec![
            CitationError::IndexMalformed {
                reason: "r".to_owned(),
            },
            CitationError::IndexEmpty,
            CitationError::Io {
                path: "p".to_owned(),
                source: io::Error::other("boom"),
            },
            CitationError::EmptyCorpus,
            CitationError::ExclusionMarkerEmpty {
                path: "p".to_owned(),
            },
            CitationError::ExclusionMarkerGone {
                path: "p".to_owned(),
                marker: "m".to_owned(),
                reason: "r".to_owned(),
            },
            CitationError::ExcludedDocumentMissing {
                path: "p".to_owned(),
                reason: "r".to_owned(),
            },
            CitationError::CorpusTooLarge {
                limit: CorpusLimit::Files(1),
            },
            CitationError::CorpusTooLarge {
                limit: CorpusLimit::TotalBytes(1),
            },
        ];
        let names: BTreeSet<&str> = samples.iter().map(variant_name).collect();
        assert_eq!(names.len(), 8, "every variant has a sample: {names:?}");
        for error in samples {
            let reference = error.methodology_ref();
            assert!(
                reference.resolves(),
                "{}: {reference} does not resolve in ori-core's restatement",
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

    #[test]
    fn ori_p1_033_the_errors_the_checker_actually_returns_resolve_too() {
        let index = real_index();
        let returned = [
            SectionIndex::from_json("{").expect_err("malformed"),
            checker(GateStandings::new(), Exclusions::none())
                .check(&Corpus::new())
                .expect_err("empty"),
            Corpus::read_specification(
                Path::new("/nonexistent/ori-t-0040"),
                &ReadLimits::default(),
            )
            .expect_err("io"),
        ];
        for error in returned {
            assert!(index.contains(&error.methodology_ref().section.to_string()));
        }
    }
}
