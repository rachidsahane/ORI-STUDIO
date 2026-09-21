//! The machine-readable index of the methodology's numbered headings: AICD §39.
//!
//! AICD §39 records the failure that earned this module ("four drafting agents
//! produced eleven fabricated section references, one of which would have
//! removed a required check") and the rule adopted from it: "References are
//! checked mechanically. The methodology's section index is machine-readable, a
//! checker validates every reference in specification, operational memory and
//! instruction files." This module builds that index. The citation gate
//! (`spec/LLD.md` section 2, installed under AICD §14) resolves every `AICD §<n>`
//! reference in the repository against it.
//!
//! What the index contains is fixed by the lead's ruling R1 in `ops/rulings.md`:
//! the 40 numbered sections, the 3 appendices, the 8 numbered subsections of
//! section 24 and the 5 numbered templates of appendix A. Those are every
//! numbered heading the document carries.
//!
//! # The numbers are the index, not the anchors
//!
//! Entries are derived from the displayed heading numbers and titles, never from
//! `id` attributes. `ops/methodology-anchor-defects.md` records why: the document
//! has no `s23`, sections 23 to 30 carry `s24` to `s31`, and `s31` sits on two
//! headings. An index derived from anchors would be wrong for nine of the forty
//! sections and would look right, which is the defect class AICD §39 names
//! "present but reporting nothing".
//!
//! Each entry still carries the anchor as the document has it, with
//! [`AnchorState`] marking the anchors that are absent or duplicated, so the
//! ticket that repairs them can work from this index rather than deriving the
//! same facts again.
//!
//! # Regenerating `methodology/sections.json`
//!
//! [`REGENERATE_COMMAND`] rewrites the committed index from the document. The
//! test `committed_sections_json_matches_a_fresh_parse_of_the_methodology` fails
//! whenever the committed file and the document disagree, so a stale index is a
//! test failure rather than a silent wrong answer. Regeneration itself also
//! fails that test: it is a write, and a write must not report itself as a
//! check.
//!
//! # What the index says about the citations already in the repository
//!
//! The index is only half of what this ticket owes. The other half is the
//! answer it gives, and the answer is not "everything resolves". Fourteen
//! occurrences of twelve distinct numbers do not, all of them in
//! `spec/design/Ori Studio.html`. They are open escalation 4 in
//! `ops/phase-1-backlog.md`, whose question is whether gate 9 checks markdown
//! only or everything under `spec/`.
//!
//! The lead's ruling R19 (`ops/rulings.md`) leaves that question open with the
//! operator and directs this ticket to report the twelve and correct nothing
//! until it is answered. R19 settles what this ticket does with them; it does
//! not exempt the file, and it does not decide what gate 9's scope will be.
//!
//! The test `citations_resolve_everywhere_but_the_recorded_design_artifact`
//! holds that answer mechanically. It reads every file in the repository as
//! bytes, with no extension filter, because a scan narrowed to `*.md` and
//! `*.rs` reports a clean sweep of this repository and every citation that
//! fails is in an HTML file. Run
//!
//! ```text
//! cargo test -p ori-gates citations -- --nocapture
//! ```
//!
//! to print the current inventory: distinct citations, occurrences, files
//! scanned, what was excluded and every unresolved citation with its line.
//!
//! Must not: add a dependency (`CLAUDE.md` absolute rule 6). The parser is
//! written against this document's actual structure and uses nothing outside
//! `std`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// The methodology document the index is generated from, relative to the
/// repository root.
pub const SOURCE_PATH: &str = "methodology/AICD_Methodology_v0.3.html";

/// The generated index, relative to the repository root.
pub const OUTPUT_PATH: &str = "methodology/sections.json";

/// The command that rewrites [`OUTPUT_PATH`] from [`SOURCE_PATH`].
pub const REGENERATE_COMMAND: &str = "ORI_SECTIONS_REGENERATE=1 cargo test -p ori-gates sections";

/// The environment variable that turns the staleness test into a regeneration.
pub const REGENERATE_ENV: &str = "ORI_SECTIONS_REGENERATE";

/// What a heading is: AICD §39 (the index is of numbered headings).
///
/// The three kinds are the three citation granularities ruling R1 admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum EntryKind {
    /// A numbered section, cited `AICD §7`.
    Section,
    /// An appendix, cited `AICD appendix A`.
    Appendix,
    /// A numbered subsection of a section or appendix, cited `AICD §24.2` or
    /// `AICD appendix A.5`.
    Subsection,
}

impl EntryKind {
    /// The name this kind carries in `methodology/sections.json`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Appendix => "appendix",
            Self::Subsection => "subsection",
        }
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The state of a heading's HTML anchor: AICD §39.
///
/// Recorded, not acted on. The index resolves citations by number, so a
/// defective anchor never reaches a citation check; it is carried here so the
/// repair ticket has the inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum AnchorState {
    /// The heading carries an `id` that no other element in the document
    /// carries.
    Unique,
    /// The heading carries an `id` that at least one other element also
    /// carries. A browser resolves such a fragment to the first occurrence in
    /// document order, so at most one of the headings is reachable.
    Duplicated,
    /// The heading carries no `id`, so nothing can link to it.
    Absent,
}

impl AnchorState {
    /// The name this state carries in `methodology/sections.json`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unique => "unique",
            Self::Duplicated => "duplicated",
            Self::Absent => "absent",
        }
    }
}

impl fmt::Display for AnchorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One numbered heading of the methodology: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// The number as a citation spells it: `7`, `24.2`, `A`, `A.5`.
    pub number: String,
    /// Which of the three granularities this entry is.
    pub kind: EntryKind,
    /// The number of the section or appendix that encloses a subsection;
    /// `None` for a section or an appendix.
    pub parent: Option<String>,
    /// The heading text with the number removed, markup stripped and whitespace
    /// collapsed.
    pub title: String,
    /// The heading's `id` attribute as the document has it, defects included.
    pub anchor: Option<String>,
    /// Whether that anchor is usable.
    pub anchor_state: AnchorState,
    /// The 1-based line of the document the heading opens on.
    pub line: usize,
}

/// The index of every numbered heading in the methodology: AICD §39.
///
/// Built by [`Index::parse`], written by [`Index::to_json`], read back by the
/// citation gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Index {
    /// The document the index was built from, as a repository-relative path.
    pub source: String,
    /// The document's length in bytes once its line endings are LF, so a human
    /// can tell a stale index from a current one with `wc -c`.
    ///
    /// Normalized rather than raw. A CRLF checkout of the document is the same
    /// document and must produce the same index, so this counts the bytes the
    /// repository stores, not the bytes a particular checkout wrote to disk.
    /// See [`lf_line_endings`].
    pub source_bytes: usize,
    /// The document's length in lines, so a human can tell a stale index from a
    /// current one with `wc -l`.
    pub source_lines: usize,
    /// Every numbered heading, in document order.
    pub entries: Vec<Entry>,
}

impl Index {
    /// Builds the index from the document's text: AICD §39.
    ///
    /// `source` is recorded in the output verbatim, so callers pass the
    /// repository-relative path whatever path they actually read.
    ///
    /// The text's line endings are folded to LF before anything is read from
    /// it, so one document produces one index whatever platform it arrived on.
    /// [`lf_line_endings`] says why that fold lives here and nowhere else.
    ///
    /// # Errors
    ///
    /// Refuses any document whose numbered headings it cannot account for: an
    /// unclosed heading, a section number out of sequence, a subsection outside
    /// the section that numbers it, a repeated number or an empty title. A
    /// partial index would resolve some citations and silently fail others,
    /// which is the defect class AICD §39 names.
    pub fn parse(source: &str, html: &str) -> Result<Self, SectionsError> {
        // The single point at which line endings are decided. Everything below
        // reads `html` and nothing reads the argument again, so no later step
        // can disagree about what a line ending is.
        let document = lf_line_endings(html);
        let html: &str = &document;

        let census = id_census(html);
        let headings = headings(html)?;

        let mut entries: Vec<Entry> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut top: Option<String> = None;
        let mut next_section: u32 = 1;
        let mut next_appendix: u32 = 0;
        let mut children: BTreeMap<String, u32> = BTreeMap::new();

        for heading in &headings {
            let (number, title, kind, parent) = if heading.level == 1 {
                let Some((number, title)) = numbered_section(heading.inner) else {
                    // A part title, the table of contents heading or the
                    // colophon. None of them carries a number, so none of them
                    // can be cited.
                    continue;
                };
                let kind = classify_top(&number, heading.line, next_section, next_appendix)?;
                match kind {
                    EntryKind::Appendix => next_appendix += 1,
                    _ => next_section += 1,
                }
                top = Some(number.clone());
                (number, title, kind, None)
            } else {
                let Some((number, title)) = numbered_subsection(&text_of(heading.inner)) else {
                    continue;
                };
                let (parent, child) = match split_number(&number) {
                    Some((parent, child)) => (parent.to_owned(), child),
                    None => {
                        return Err(SectionsError::OrphanSubsection {
                            number,
                            line: heading.line,
                        });
                    }
                };
                let Some(enclosing) = top.clone() else {
                    return Err(SectionsError::OrphanSubsection {
                        number,
                        line: heading.line,
                    });
                };
                if parent != enclosing {
                    return Err(SectionsError::SubsectionOutsideParent {
                        number,
                        enclosing,
                        line: heading.line,
                    });
                }
                let counter = children.entry(parent.clone()).or_insert(0);
                *counter += 1;
                if child != *counter {
                    return Err(SectionsError::SubsectionOutOfSequence {
                        expected: format!("{parent}.{counter}"),
                        found: number,
                        line: heading.line,
                    });
                }
                (number, title, EntryKind::Subsection, Some(parent))
            };

            if title.is_empty() {
                return Err(SectionsError::EmptyTitle {
                    number,
                    line: heading.line,
                });
            }
            if !seen.insert(number.clone()) {
                return Err(SectionsError::RepeatedNumber {
                    number,
                    line: heading.line,
                });
            }

            let anchor = heading.id.map(str::to_owned);
            let anchor_state = match anchor.as_deref() {
                None => AnchorState::Absent,
                Some(id) if census.get(id).copied().unwrap_or(0) > 1 => AnchorState::Duplicated,
                Some(_) => AnchorState::Unique,
            };

            entries.push(Entry {
                number,
                kind,
                parent,
                title,
                anchor,
                anchor_state,
                line: heading.line,
            });
        }

        if next_section == 1 {
            return Err(SectionsError::NoSections);
        }

        Ok(Self {
            source: source.to_owned(),
            source_bytes: html.len(),
            source_lines: html.lines().count(),
            entries,
        })
    }

    /// Reads a document from disk and builds the index: AICD §39.
    ///
    /// `source` is the repository-relative path recorded in the output, which is
    /// stable wherever `path` happens to point.
    ///
    /// # Errors
    ///
    /// [`SectionsError::Io`] if the document cannot be read, or any error
    /// [`Index::parse`] returns.
    pub fn from_file(path: &Path, source: &str) -> Result<Self, SectionsError> {
        let html = fs::read_to_string(path).map_err(|error| SectionsError::Io {
            path: path.display().to_string(),
            error,
        })?;
        Self::parse(source, &html)
    }

    /// The entry a citation's number names, if the methodology has one.
    ///
    /// `number` is the bare number a citation carries: `7`, `24.2`, `A`, `A.5`.
    /// Parsing the surrounding citation text belongs to the citation gate, not
    /// here.
    pub fn get(&self, number: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.number == number)
    }

    /// Whether the methodology has a heading numbered `number`.
    pub fn contains(&self, number: &str) -> bool {
        self.get(number).is_some()
    }

    /// How many entries of one kind the index holds.
    pub fn count(&self, kind: EntryKind) -> usize {
        self.entries.iter().filter(|e| e.kind == kind).count()
    }

    /// The numbered subsections of one section or appendix, in document order.
    pub fn children_of(&self, parent: &str) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(move |entry| entry.parent.as_deref() == Some(parent))
    }

    /// Serializes the index as `methodology/sections.json`: AICD §39.
    ///
    /// Deterministic: the same document produces the same bytes, so the
    /// committed file can be compared for staleness rather than reparsed.
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(32 * 1024);
        out.push_str("{\n");
        out.push_str("  \"generator\": \"crates/ori-gates/src/sections.rs\",\n");
        push_pair(
            &mut out,
            "  ",
            "generated_under",
            "AICD \u{a7}39, ruling R1 (ops/rulings.md)",
        );
        push_pair(&mut out, "  ", "regenerate", REGENERATE_COMMAND);
        push_pair(&mut out, "  ", "source", &self.source);
        out.push_str(&format!("  \"source_bytes\": {},\n", self.source_bytes));
        out.push_str(&format!("  \"source_lines\": {},\n", self.source_lines));

        out.push_str("  \"counts\": {\n");
        out.push_str(&format!(
            "    \"section\": {},\n",
            self.count(EntryKind::Section)
        ));
        out.push_str(&format!(
            "    \"appendix\": {},\n",
            self.count(EntryKind::Appendix)
        ));
        out.push_str(&format!(
            "    \"subsection\": {},\n",
            self.count(EntryKind::Subsection)
        ));
        out.push_str(&format!("    \"total\": {}\n", self.entries.len()));
        out.push_str("  },\n");

        out.push_str("  \"anchor_states\": {\n");
        for (index, state) in [
            AnchorState::Unique,
            AnchorState::Duplicated,
            AnchorState::Absent,
        ]
        .iter()
        .enumerate()
        {
            let count = self
                .entries
                .iter()
                .filter(|entry| entry.anchor_state == *state)
                .count();
            let comma = if index == 2 { "" } else { "," };
            out.push_str(&format!("    \"{state}\": {count}{comma}\n"));
        }
        out.push_str("  },\n");

        out.push_str("  \"entries\": [\n");
        for (index, entry) in self.entries.iter().enumerate() {
            out.push_str("    {\n");
            push_pair(&mut out, "      ", "number", &entry.number);
            push_pair(&mut out, "      ", "kind", entry.kind.as_str());
            match entry.parent.as_deref() {
                Some(parent) => push_pair(&mut out, "      ", "parent", parent),
                None => out.push_str("      \"parent\": null,\n"),
            }
            push_pair(&mut out, "      ", "title", &entry.title);
            match entry.anchor.as_deref() {
                Some(anchor) => push_pair(&mut out, "      ", "anchor", anchor),
                None => out.push_str("      \"anchor\": null,\n"),
            }
            push_pair(
                &mut out,
                "      ",
                "anchor_state",
                entry.anchor_state.as_str(),
            );
            out.push_str(&format!("      \"line\": {}\n", entry.line));
            let comma = if index + 1 == self.entries.len() {
                ""
            } else {
                ","
            };
            out.push_str(&format!("    }}{comma}\n"));
        }
        out.push_str("  ]\n");
        out.push_str("}\n");
        out
    }
}

/// Why the index could not be built: AICD §39.
///
/// Every variant is a refusal to publish an index that would resolve some
/// citations and silently fail others. [`SectionsError::methodology_ref`] names
/// the section each refusal rests on.
///
/// The [`fmt::Display`] and [`Error`] implementations below are written by hand
/// rather than derived. `spec/CONVENTIONS.md` names `thiserror` as the house
/// error convention, so adopting it would implement the specification rather
/// than add an unsanctioned dependency. The lead's ruling R20 (`ops/rulings.md`)
/// defers it to batch 2 all the same: the first external crate this project
/// takes on should land alongside the dependency audit that watches it,
/// `spec/CI_CD.md` gate 7 (`cargo-audit`, `cargo-deny`, secret scan), which
/// arrives with ORI-T-0016 and does not exist yet.
///
/// R20 requires the hand-written implementation to carry a comment naming the
/// conversion, and this is that comment: when `thiserror` lands, every arm of
/// the [`fmt::Display`] match below becomes one `#[error(...)]` attribute on
/// the variant it prints, the `error` field of [`SectionsError::Io`] takes
/// `#[source]`, and both hand-written implementations are deleted. No message
/// and no call site changes.
#[derive(Debug)]
pub enum SectionsError {
    /// The document could not be read.
    Io {
        /// The path that was attempted.
        path: String,
        /// What the filesystem said.
        error: io::Error,
    },
    /// A heading tag opens and never closes.
    UnclosedHeading {
        /// The heading level, 1 to 3.
        level: u8,
        /// The line the heading opens on.
        line: usize,
    },
    /// A numbered heading's number is neither a section number nor an appendix
    /// letter.
    UnreadableNumber {
        /// The number as the document spells it.
        found: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// The numbered sections do not run consecutively from 1.
    SectionOutOfSequence {
        /// The number the sequence requires next.
        expected: String,
        /// The number the document carries.
        found: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// The appendices do not run consecutively from A.
    AppendixOutOfSequence {
        /// The letter the sequence requires next.
        expected: String,
        /// The letter the document carries.
        found: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// A numbered section appears after an appendix has begun.
    SectionAfterAppendix {
        /// The section number found in the appendices.
        found: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// A numbered subsection appears before any section.
    OrphanSubsection {
        /// The subsection number.
        number: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// A numbered subsection sits inside a section other than the one its
    /// number names.
    SubsectionOutsideParent {
        /// The subsection number.
        number: String,
        /// The section that actually encloses it.
        enclosing: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// The subsections of one parent do not run consecutively from 1.
    SubsectionOutOfSequence {
        /// The number the sequence requires next.
        expected: String,
        /// The number the document carries.
        found: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// Two numbered headings carry the same number, so a citation to it is
    /// ambiguous.
    RepeatedNumber {
        /// The number carried twice.
        number: String,
        /// The line of the second heading.
        line: usize,
    },
    /// A numbered heading has a number and no title.
    EmptyTitle {
        /// The number whose title is missing.
        number: String,
        /// The line the heading opens on.
        line: usize,
    },
    /// The document contains no numbered section at all.
    NoSections,
}

impl SectionsError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute rule
    /// 9).
    ///
    /// Returned as text rather than as `ori_core::MethodologyRef`: `ori-gates`
    /// does not depend on `ori-core`, and adding the edge is outside this
    /// ticket's declared scope.
    pub fn methodology_ref(&self) -> &'static str {
        match self {
            // A gate that cannot read its input reports nothing, which AICD §14
            // requires it to do loudly rather than by passing.
            Self::Io { .. } => "AICD §14",
            // Every other variant is a document the mechanical reference check
            // of AICD §39 cannot be trusted on.
            _ => "AICD §39",
        }
    }
}

impl fmt::Display for SectionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "cannot read {path}: {error}"),
            Self::UnclosedHeading { level, line } => {
                write!(f, "line {line}: <h{level}> is never closed")
            }
            Self::UnreadableNumber { found, line } => write!(
                f,
                "line {line}: heading number {found:?} is neither a section number nor an appendix letter"
            ),
            Self::SectionOutOfSequence {
                expected,
                found,
                line,
            } => write!(
                f,
                "line {line}: expected section {expected}, found section {found}"
            ),
            Self::AppendixOutOfSequence {
                expected,
                found,
                line,
            } => write!(
                f,
                "line {line}: expected appendix {expected}, found appendix {found}"
            ),
            Self::SectionAfterAppendix { found, line } => write!(
                f,
                "line {line}: section {found} appears after the appendices begin"
            ),
            Self::OrphanSubsection { number, line } => {
                write!(f, "line {line}: subsection {number} precedes every section")
            }
            Self::SubsectionOutsideParent {
                number,
                enclosing,
                line,
            } => write!(
                f,
                "line {line}: subsection {number} sits inside section {enclosing}"
            ),
            Self::SubsectionOutOfSequence {
                expected,
                found,
                line,
            } => write!(
                f,
                "line {line}: expected subsection {expected}, found subsection {found}"
            ),
            Self::RepeatedNumber { number, line } => {
                write!(f, "line {line}: number {number} is used by two headings")
            }
            Self::EmptyTitle { number, line } => {
                write!(f, "line {line}: heading {number} has no title")
            }
            Self::NoSections => f.write_str("the document contains no numbered section"),
        }
    }
}

impl Error for SectionsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing. The document's structure is regular and is documented in
// `ops/methodology-anchor-defects.md`:
//
//   <h1 id="s7"><span class="num">7</span>The agent team ...</h1>
//   <h1 id="a1"><span class="num">A</span>Templates</h1>
//   <h2>24.2 Phase M1: Access attribution</h2>
//   <h2>A.5 Blocked report</h2>
//
// Headings never nest and never carry a nested heading, which is what makes a
// scan for the opening tag and its matching close sufficient.
// ---------------------------------------------------------------------------

/// The document with its CRLF line endings folded to LF: AICD §39.
///
/// The one place this module handles line endings. Everything [`Index::parse`]
/// derives is derived from the value returned here, so the index a document
/// produces does not depend on the platform the document was checked out on.
///
/// # Why here and not at read time
///
/// [`Index::parse`] is public and takes text, so the fold has to be inside it
/// to cover a caller that did not get the document from [`Index::from_file`]:
/// an editor buffer, a fixture, a document fetched from elsewhere. Folding in
/// [`Index::from_file`] would leave the parser itself platform-sensitive and
/// would make the regression test write a file to prove anything. Folding in
/// the line handling instead would be worse still: this parser scans bytes
/// rather than lines, so it has no single line handler, and the field that
/// actually differed across platforms was [`Index::source_bytes`], which no
/// amount of care inside the scan would have fixed.
///
/// # Why only CRLF
///
/// `core.autocrlf` rewrites LF to CRLF on checkout and back on commit, and
/// leaves a lone CR untouched. Folding `\r\n` is therefore the exact inverse of
/// what the checkout did and nothing more. A lone CR is the same byte on every
/// platform, so it is content rather than a line ending, and rewriting it would
/// change a title the document really carries.
///
/// Borrows when there is nothing to fold, which is every run on a repository
/// checked out with LF.
fn lf_line_endings(html: &str) -> Cow<'_, str> {
    if html.contains('\r') {
        Cow::Owned(html.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(html)
    }
}

/// One heading tag as the document carries it.
struct Heading<'a> {
    level: u8,
    id: Option<&'a str>,
    inner: &'a str,
    line: usize,
}

/// Every `<h1>`, `<h2>` and `<h3>` in the document, in order.
fn headings(html: &str) -> Result<Vec<Heading<'_>>, SectionsError> {
    let bytes = html.as_bytes();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        let Some(level) = heading_level(bytes, i) else {
            i += 1;
            continue;
        };
        let Some(open_end) = find_byte(bytes, i, b'>') else {
            return Err(SectionsError::UnclosedHeading { level, line });
        };
        let close = format!("</h{level}>");
        let Some(offset) = html[open_end + 1..].find(&close) else {
            return Err(SectionsError::UnclosedHeading { level, line });
        };
        out.push(Heading {
            level,
            id: attribute(&html[i + 3..open_end], "id"),
            inner: &html[open_end + 1..open_end + 1 + offset],
            line,
        });
        // Advance past the opening tag only, never past the content, so that
        // newlines inside the heading keep the line counter honest.
        i = open_end + 1;
    }

    Ok(out)
}

/// The level of the heading opening at `at`, if one opens there.
fn heading_level(bytes: &[u8], at: usize) -> Option<u8> {
    if bytes.get(at) != Some(&b'<') || bytes.get(at + 1) != Some(&b'h') {
        return None;
    }
    let digit = *bytes.get(at + 2)?;
    if !(b'1'..=b'3').contains(&digit) {
        return None;
    }
    let after = *bytes.get(at + 3)?;
    if after == b'>' || after.is_ascii_whitespace() {
        Some(digit - b'0')
    } else {
        None
    }
}

/// The value of one attribute of an opening tag.
fn attribute<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let mut from = 0usize;
    while let Some(offset) = attrs[from..].find(&needle) {
        let at = from + offset;
        let standalone = at == 0
            || attrs.as_bytes()[at - 1].is_ascii_whitespace()
            || attrs.as_bytes()[at - 1] == b'<';
        let value_start = at + needle.len();
        if standalone {
            let end = attrs[value_start..].find('"')?;
            return Some(&attrs[value_start..value_start + end]);
        }
        from = value_start;
    }
    None
}

/// How many times each `id` value appears anywhere in the document.
///
/// Counted over the whole file, not only over headings, because an anchor is
/// unusable as soon as any other element shares it.
fn id_census(html: &str) -> BTreeMap<&str, usize> {
    let mut census: BTreeMap<&str, usize> = BTreeMap::new();
    let needle = "id=\"";
    let mut from = 0usize;
    while let Some(offset) = html[from..].find(needle) {
        let at = from + offset;
        let value_start = at + needle.len();
        let preceded_by_space = at > 0 && html.as_bytes()[at - 1].is_ascii_whitespace();
        let value_end = html[value_start..].find('"');
        if let (true, Some(end)) = (preceded_by_space, value_end) {
            *census
                .entry(&html[value_start..value_start + end])
                .or_insert(0) += 1;
        }
        from = value_start;
    }
    census
}

/// The number and title of a numbered `<h1>`, if it carries a number.
///
/// Every numbered heading spells its number in a `<span class="num">`; the part
/// titles, the table of contents heading and the colophon carry none, and are
/// not citable.
fn numbered_section(inner: &str) -> Option<(String, String)> {
    const OPEN: &str = "<span class=\"num\">";
    const CLOSE: &str = "</span>";
    let start = inner.find(OPEN)?;
    let number_start = start + OPEN.len();
    let offset = inner[number_start..].find(CLOSE)?;
    let number = text_of(&inner[number_start..number_start + offset]);
    let title = text_of(&inner[number_start + offset + CLOSE.len()..]);
    Some((number, title))
}

/// Which granularity a numbered `<h1>` is, checking its place in the sequence.
fn classify_top(
    number: &str,
    line: usize,
    next_section: u32,
    next_appendix: u32,
) -> Result<EntryKind, SectionsError> {
    if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) {
        if next_appendix > 0 {
            return Err(SectionsError::SectionAfterAppendix {
                found: number.to_owned(),
                line,
            });
        }
        if number != next_section.to_string() {
            return Err(SectionsError::SectionOutOfSequence {
                expected: next_section.to_string(),
                found: number.to_owned(),
                line,
            });
        }
        return Ok(EntryKind::Section);
    }

    let letter = number.as_bytes();
    if letter.len() == 1 && letter[0].is_ascii_uppercase() {
        // Saturating, not wrapping: a document with more appendices than the
        // alphabet has letters must refuse, never overflow.
        let offset = u8::try_from(next_appendix).unwrap_or(u8::MAX);
        let expected = char::from(b'A'.saturating_add(offset));
        if number != expected.to_string() {
            return Err(SectionsError::AppendixOutOfSequence {
                expected: expected.to_string(),
                found: number.to_owned(),
                line,
            });
        }
        return Ok(EntryKind::Appendix);
    }

    Err(SectionsError::UnreadableNumber {
        found: number.to_owned(),
        line,
    })
}

/// The number and title of a numbered `<h2>` or `<h3>`, if it carries a number.
///
/// A numbered subheading opens with its number and a space: `24.2 Phase M1:
/// Access attribution`. Every other subheading in the document is unnumbered
/// and therefore not citable.
fn numbered_subsection(text: &str) -> Option<(String, String)> {
    let (head, rest) = text.split_once(' ')?;
    let (parent, child) = head.split_once('.')?;
    let parent_ok = !parent.is_empty()
        && (parent.bytes().all(|b| b.is_ascii_digit())
            || (parent.len() == 1 && parent.as_bytes()[0].is_ascii_uppercase()));
    let child_ok = !child.is_empty() && child.bytes().all(|b| b.is_ascii_digit());
    if !parent_ok || !child_ok {
        return None;
    }
    let title = rest.trim().to_owned();
    Some((head.to_owned(), title))
}

/// A subsection number split into its parent and its ordinal.
fn split_number(number: &str) -> Option<(&str, u32)> {
    let (parent, child) = number.split_once('.')?;
    let child = child.parse::<u32>().ok()?;
    Some((parent, child))
}

/// The readable text of an HTML fragment: tags dropped, entities decoded,
/// whitespace collapsed.
fn text_of(fragment: &str) -> String {
    let bytes = fragment.as_bytes();
    let mut raw = String::with_capacity(fragment.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            match find_byte(bytes, i, b'>') {
                Some(end) => i = end + 1,
                None => break,
            }
        } else {
            let end = find_byte(bytes, i, b'<').unwrap_or(bytes.len());
            raw.push_str(&fragment[i..end]);
            i = end;
        }
    }
    let decoded = decode_entities(&raw);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decodes the named entities this document uses, in one left-to-right pass so
/// that `&amp;lt;` does not decode twice.
fn decode_entities(text: &str) -> String {
    const ENTITIES: [(&str, &str); 7] = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&#39;", "'"),
        ("&nbsp;", " "),
    ];
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        match ENTITIES
            .iter()
            .find(|(entity, _)| tail.starts_with(*entity))
        {
            Some((entity, replacement)) => {
                out.push_str(replacement);
                rest = &tail[entity.len()..];
            }
            // An entity this parser does not know is left verbatim rather than
            // guessed at: a wrong title is worse than a literal one.
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The index of the next `target` at or after `from`.
fn find_byte(bytes: &[u8], from: usize, target: u8) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|b| *b == target)
        .map(|p| from + p)
}

/// Appends one `"name": "value",` line of JSON.
fn push_pair(out: &mut String, indent: &str, name: &str, value: &str) {
    out.push_str(indent);
    out.push('"');
    out.push_str(name);
    out.push_str("\": ");
    push_json_string(out, value);
    out.push_str(",\n");
}

/// Appends a JSON string literal.
fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // These tests carry no criterion identifier. `spec/CONVENTIONS.md` requires
    // every test name to embed the criterion it covers, and no acceptance
    // criterion in `spec/criteria/` covers this module: it is tooling for the
    // citation gate rather than product behaviour. Inventing an identifier
    // would put a fabricated reference in the coverage matrix, which is the
    // defect class AICD §39 exists to stop, so the names are descriptive and
    // the gap is reported instead.

    /// The repository root, two levels above `crates/ori-gates`.
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("a crate lives two levels below the repository root")
            .to_path_buf()
    }

    fn methodology() -> Index {
        Index::from_file(&repo_root().join(SOURCE_PATH), SOURCE_PATH)
            .expect("the shipped methodology parses")
    }

    #[test]
    fn parses_the_granularity_ruling_r1_fixes() {
        let index = methodology();
        assert_eq!(index.count(EntryKind::Section), 40, "numbered sections");
        assert_eq!(index.count(EntryKind::Appendix), 3, "appendices");
        assert_eq!(index.count(EntryKind::Subsection), 13, "subsections");
        assert_eq!(index.entries.len(), 56, "total entries");
        assert_eq!(index.children_of("24").count(), 8, "subsections of 24");
        assert_eq!(index.children_of("A").count(), 5, "templates of appendix A");
    }

    #[test]
    fn numbered_sections_run_from_one_to_forty_in_document_order() {
        let index = methodology();
        let numbers: Vec<&str> = index
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Section)
            .map(|entry| entry.number.as_str())
            .collect();
        let expected: Vec<String> = (1..=40).map(|n| n.to_string()).collect();
        assert_eq!(numbers, expected);
    }

    #[test]
    fn appendices_are_a_b_and_c() {
        let index = methodology();
        let letters: Vec<&str> = index
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Appendix)
            .map(|entry| entry.number.as_str())
            .collect();
        assert_eq!(letters, vec!["A", "B", "C"]);
    }

    #[test]
    fn every_subsection_belongs_to_section_24_or_appendix_a() {
        let index = methodology();
        for entry in index
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::Subsection)
        {
            let parent = entry.parent.as_deref().unwrap_or_default();
            assert!(
                parent == "24" || parent == "A",
                "unexpected parent {parent} for {}",
                entry.number
            );
            assert!(index.contains(parent), "parent {parent} is itself indexed");
        }
    }

    #[test]
    fn titles_match_the_document_for_a_hand_checked_sample() {
        let index = methodology();
        // Checked by hand against the headings of
        // methodology/AICD_Methodology_v0.3.html.
        let sample = [
            ("1", "Why a new methodology"),
            ("7", "The agent team and separation of duties"),
            (
                "14",
                "Verification: tests written in language, built by AI, checked by humans",
            ),
            ("23", "Starting a new product under AICD"),
            ("24", "Migrating an existing product into AICD"),
            ("24.1", "Phase M0: Freeze and snapshot"),
            ("24.2", "Phase M1: Access attribution"),
            ("24.8", "Special cases"),
            ("31", "Simulation personas and finding quality"),
            ("32", "Agent instruction design"),
            ("39", "Lessons from the first application"),
            ("40", "Open questions and next explorations"),
            ("A", "Templates"),
            ("A.1", "Acceptance criterion"),
            ("A.5", "Blocked report"),
            ("B", "Glossary"),
            ("C", "Document history"),
        ];
        for (number, title) in sample {
            let entry = index
                .get(number)
                .unwrap_or_else(|| panic!("{number} is indexed"));
            assert_eq!(entry.title, title, "title of {number}");
        }
    }

    #[test]
    fn resolves_the_citation_granularities_the_repository_uses_and_no_others() {
        let index = methodology();
        for number in ["7", "24", "24.2", "39", "A", "A.5", "C"] {
            assert!(index.contains(number), "{number} resolves");
        }
        // Finer than the methodology numbers, or outside them entirely.
        for number in ["0", "41", "D", "23.1", "39.1", "A.6", "24.9", "s24"] {
            assert!(!index.contains(number), "{number} does not resolve");
        }
    }

    #[test]
    fn anchors_are_read_from_the_document_not_derived_from_the_number() {
        let index = methodology();
        let one = index.get("1").expect("section 1 is indexed");
        assert_eq!(one.anchor.as_deref(), Some("s1"));
        assert_eq!(one.anchor_state, AnchorState::Unique);
    }

    #[test]
    fn line_numbers_increase_with_document_order() {
        let index = methodology();
        let mut previous = 0usize;
        for entry in &index.entries {
            assert!(
                entry.line > previous,
                "{} at line {} follows line {previous}",
                entry.number,
                entry.line
            );
            previous = entry.line;
        }
    }

    #[test]
    fn json_is_deterministic_and_reports_the_source_length() {
        let index = methodology();
        assert_eq!(index.to_json(), index.to_json());
        let json = index.to_json();
        assert!(json.contains("\"source_bytes\": "), "records byte length");
        assert!(json.contains("\"source_lines\": "), "records line count");
        assert!(json.ends_with("}\n"), "ends with one newline");
        assert!(json.contains("\"number\": \"24.2\""), "carries subsections");
    }

    /// Fails when `methodology/sections.json` and the document disagree, and
    /// rewrites the file when [`REGENERATE_ENV`] is set. This is the whole
    /// staleness control: a stale index is a red test, not a silent wrong
    /// answer (AICD §39, "present but reporting nothing").
    #[test]
    fn committed_sections_json_matches_a_fresh_parse_of_the_methodology() {
        let root = repo_root();
        let fresh = methodology().to_json();
        let committed_path = root.join(OUTPUT_PATH);

        // Regeneration is a write, not a check. It must never be a green test:
        // a green run would mean `cargo test` reports the index verified when
        // the only thing that happened is that the index was overwritten, and
        // one exported variable would make that permanent. That is the defect
        // class AICD §39 names, "present but reporting nothing", so the write
        // is followed by a failure that says what was written.
        if std::env::var_os(REGENERATE_ENV).is_some() {
            fs::write(&committed_path, &fresh).expect("sections.json is writable");
            panic!(
                "regenerated {OUTPUT_PATH} from {SOURCE_PATH} because {REGENERATE_ENV} is set; \
                 nothing was checked. Re-run without {REGENERATE_ENV} to check the file that was \
                 just written."
            );
        }

        let committed = fs::read_to_string(&committed_path).unwrap_or_else(|error| {
            panic!("{OUTPUT_PATH} cannot be read ({error}); regenerate: {REGENERATE_COMMAND}")
        });
        assert_eq!(
            committed, fresh,
            "{OUTPUT_PATH} is stale against {SOURCE_PATH}; regenerate: {REGENERATE_COMMAND}"
        );
    }

    // ---- the parser itself, on fragments written for the purpose ----

    const SYNTHETIC: &str = concat!(
        "<div id=\"part1\">\n",
        "<h1 class=\"parttitle\">A part with no number</h1>\n",
        "<h1 id=\"s1\"><span class=\"num\">1</span>First &amp; foremost</h1>\n",
        "<h2>An unnumbered subheading</h2>\n",
        "<h1 id=\"dup\"><span class=\"num\">2</span>Second <em>with</em> markup</h1>\n",
        "<h2>2.1 A numbered subheading</h2>\n",
        "<h2>2.2 Another one</h2>\n",
        "<h1><span class=\"num\">3</span>Third, with no anchor</h1>\n",
        "<h1 id=\"a1\"><span class=\"num\">A</span>Templates</h1>\n",
        "<h2>A.1 Acceptance criterion</h2>\n",
        "<p id=\"dup\">an element sharing an anchor with a heading</p>\n",
        "</div>\n",
    );

    fn synthetic() -> Index {
        Index::parse("synthetic", SYNTHETIC).expect("the fragment parses")
    }

    #[test]
    fn unnumbered_headings_are_not_indexed() {
        let index = synthetic();
        assert_eq!(index.entries.len(), 7);
        assert!(!index.entries.iter().any(|e| e.title.contains("A part")));
        assert!(
            !index
                .entries
                .iter()
                .any(|e| e.title == "An unnumbered subheading")
        );
    }

    #[test]
    fn titles_are_stripped_of_markup_and_decoded() {
        let index = synthetic();
        assert_eq!(
            index.get("1").map(|e| e.title.as_str()),
            Some("First & foremost")
        );
        assert_eq!(
            index.get("2").map(|e| e.title.as_str()),
            Some("Second with markup")
        );
    }

    #[test]
    fn an_anchor_shared_with_another_element_is_marked_duplicated() {
        let index = synthetic();
        let two = index.get("2").expect("section 2 is indexed");
        assert_eq!(two.anchor.as_deref(), Some("dup"));
        assert_eq!(two.anchor_state, AnchorState::Duplicated);
    }

    #[test]
    fn a_heading_with_no_id_is_marked_absent() {
        let index = synthetic();
        let three = index.get("3").expect("section 3 is indexed");
        assert_eq!(three.anchor, None);
        assert_eq!(three.anchor_state, AnchorState::Absent);
        let sub = index.get("2.1").expect("subsection 2.1 is indexed");
        assert_eq!(sub.anchor_state, AnchorState::Absent);
    }

    #[test]
    fn subsections_carry_the_parent_that_encloses_them() {
        let index = synthetic();
        assert_eq!(
            index.get("2.1").and_then(|e| e.parent.as_deref()),
            Some("2")
        );
        assert_eq!(
            index.get("A.1").and_then(|e| e.parent.as_deref()),
            Some("A")
        );
    }

    #[test]
    fn a_skipped_section_number_is_refused() {
        let html =
            "<h1><span class=\"num\">1</span>One</h1>\n<h1><span class=\"num\">3</span>Three</h1>";
        let error = Index::parse("synthetic", html).expect_err("the gap is refused");
        assert!(
            matches!(error, SectionsError::SectionOutOfSequence { ref expected, ref found, line: 2 } if expected == "2" && found == "3"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_subsection_outside_the_section_that_numbers_it_is_refused() {
        let html = "<h1><span class=\"num\">1</span>One</h1>\n<h2>24.1 Belongs elsewhere</h2>";
        let error = Index::parse("synthetic", html).expect_err("the stray subsection is refused");
        assert!(
            matches!(error, SectionsError::SubsectionOutsideParent { ref number, ref enclosing, .. } if number == "24.1" && enclosing == "1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_skipped_subsection_number_is_refused() {
        let html =
            "<h1><span class=\"num\">1</span>One</h1>\n<h2>1.1 First</h2>\n<h2>1.3 Third</h2>";
        let error = Index::parse("synthetic", html).expect_err("the gap is refused");
        assert!(
            matches!(error, SectionsError::SubsectionOutOfSequence { ref expected, .. } if expected == "1.2"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn an_appendix_out_of_order_is_refused() {
        let html =
            "<h1><span class=\"num\">1</span>One</h1>\n<h1><span class=\"num\">C</span>Third</h1>";
        let error = Index::parse("synthetic", html).expect_err("the jump is refused");
        assert!(
            matches!(error, SectionsError::AppendixOutOfSequence { ref expected, .. } if expected == "A"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_section_after_an_appendix_is_refused() {
        let html = "<h1><span class=\"num\">1</span>One</h1>\n<h1><span class=\"num\">A</span>Templates</h1>\n<h1><span class=\"num\">2</span>Two</h1>";
        let error = Index::parse("synthetic", html).expect_err("the stray section is refused");
        assert!(
            matches!(error, SectionsError::SectionAfterAppendix { ref found, .. } if found == "2"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn an_unclosed_heading_is_refused() {
        let html = "<h1><span class=\"num\">1</span>One";
        let error = Index::parse("synthetic", html).expect_err("the unclosed tag is refused");
        assert!(
            matches!(error, SectionsError::UnclosedHeading { level: 1, line: 1 }),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_numbered_heading_with_no_title_is_refused() {
        let html = "<h1><span class=\"num\">1</span></h1>";
        let error = Index::parse("synthetic", html).expect_err("the empty title is refused");
        assert!(
            matches!(error, SectionsError::EmptyTitle { ref number, .. } if number == "1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_document_with_no_numbered_section_is_refused() {
        let html = "<h1>Contents</h1>\n<h2>Nothing numbered here</h2>";
        let error = Index::parse("synthetic", html).expect_err("the empty index is refused");
        assert!(
            matches!(error, SectionsError::NoSections),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_missing_document_is_refused_by_io_not_by_an_empty_index() {
        let error = Index::from_file(Path::new("/nonexistent/methodology.html"), SOURCE_PATH)
            .expect_err("a missing file is refused");
        assert!(matches!(error, SectionsError::Io { .. }), "{error}");
        assert_eq!(error.methodology_ref(), "AICD §14");
    }

    #[test]
    fn every_refusal_names_the_methodology_section_it_rests_on() {
        let refusals = [
            SectionsError::NoSections,
            SectionsError::UnclosedHeading { level: 2, line: 9 },
            SectionsError::EmptyTitle {
                number: "1".into(),
                line: 9,
            },
            SectionsError::RepeatedNumber {
                number: "1".into(),
                line: 9,
            },
        ];
        for refusal in refusals {
            assert!(
                refusal.methodology_ref().starts_with("AICD §"),
                "{refusal} carries no methodology reference"
            );
            assert!(!refusal.to_string().is_empty());
        }
    }

    #[test]
    fn a_line_number_points_at_the_line_the_heading_opens_on() {
        let index = synthetic();
        // Line 3 of SYNTHETIC is the <h1> for section 1.
        assert_eq!(index.get("1").map(|e| e.line), Some(3));
    }

    #[test]
    fn json_escapes_quotes_and_backslashes() {
        let mut out = String::new();
        push_json_string(&mut out, "a \"quoted\" \\ path\n");
        assert_eq!(out, "\"a \\\"quoted\\\" \\\\ path\\n\"");
    }

    #[test]
    fn entities_decode_once_and_unknown_ones_survive() {
        assert_eq!(decode_entities("&amp;lt;"), "&lt;");
        assert_eq!(decode_entities("&unknown; stays"), "&unknown; stays");
        assert_eq!(decode_entities("no entity"), "no entity");
    }

    /// The same document indexes identically whether it arrives with LF or with
    /// CRLF line endings: AICD §39.
    ///
    /// The regression test for ORI-T-0082. GitHub's Windows runners check out
    /// with `core.autocrlf=true`, so before the fix the methodology arrived
    /// there with 1519 carriage returns the committed bytes do not have,
    /// [`Index::source_bytes`] counted them, and the same commit produced one
    /// index on Linux and a different one on Windows. An index that varies by
    /// platform is a citation gate that passes on one runner and fails on
    /// another for the same code, which is how the fabricated references AICD
    /// §39 records get back in.
    ///
    /// Both documents are built here in memory, from a base whose line endings
    /// are folded first, so the test fails on every platform rather than only
    /// on the one that has the problem, and it does not care how the checkout
    /// that is running it wrote the file to disk.
    #[test]
    fn a_crlf_document_indexes_identically_to_the_same_document_with_lf() {
        let on_disk = fs::read_to_string(repo_root().join(SOURCE_PATH))
            .expect("the shipped methodology reads");
        let lf = on_disk.replace("\r\n", "\n");
        let crlf = lf.replace('\n', "\r\n");
        assert!(
            crlf.len() > lf.len(),
            "the CRLF document is not actually CRLF, so this test would check nothing"
        );

        let from_lf = Index::parse(SOURCE_PATH, &lf).expect("the LF document parses");
        let from_crlf = Index::parse(SOURCE_PATH, &crlf).expect("the CRLF document parses");

        // Named separately because this is the field that differed: every
        // other field of the index was already carriage-return tolerant, and an
        // equality failure alone would not say so.
        assert_eq!(
            from_lf.source_bytes, from_crlf.source_bytes,
            "source_bytes counts the carriage returns the checkout added"
        );
        assert_eq!(from_lf, from_crlf, "the two indexes are not the same index");
        assert_eq!(
            from_lf.to_json(),
            from_crlf.to_json(),
            "the two indexes do not serialize to the same bytes"
        );

        // And on the fragment the rest of the parser tests use, so a heading,
        // a title and an anchor read across a folded line ending are checked
        // too rather than only the whole-document totals.
        let fragment = SYNTHETIC.replace('\n', "\r\n");
        assert_eq!(
            Index::parse("synthetic", &fragment).expect("the CRLF fragment parses"),
            synthetic(),
            "the synthetic fragment indexes differently with CRLF"
        );
    }
    // -----------------------------------------------------------------------
    // The ticket's central check: does every citation in the repository
    // resolve against the index this module generates?
    //
    // The answer is not "yes", and an index shipped with the claim that it is
    // would be the overstatement AICD §39 names. Twelve distinct numbers do
    // not resolve. They are recorded in `RECORDED_UNRESOLVED` below, they are
    // all in one design artifact, and the lead's ruling R19 (`ops/rulings.md`)
    // is that this ticket reports them and corrects nothing while the question
    // behind them, open escalation 4, is open with the operator.
    //
    // `cargo test -p ori-gates citations -- --nocapture` prints the current
    // inventory: distinct citations, occurrences, files and the unresolved
    // list. That printed block is what goes in the pull request report.
    // -----------------------------------------------------------------------

    /// Directory names the repository scan does not descend into, each with the
    /// reason it is not repository content.
    ///
    /// Nothing else is excluded. No extension filter, no text/binary filter: a
    /// scan restricted to `*.md` and `*.rs` reports a clean sweep of this
    /// repository, because every citation that fails to resolve lives in an
    /// HTML file. That is precisely the false "everything is fine" this test
    /// exists to prevent.
    const SCAN_EXCLUSIONS: [(&str, &str); 2] = [
        (".git", "object database, not repository content"),
        ("target", "build output, not repository content"),
    ];

    /// The one file whose citations are known not to resolve.
    ///
    /// The lead's ruling R19 (`ops/rulings.md`): the twelve unresolvable
    /// citations are all in this design artifact, whether gate 9's scope covers
    /// non-markdown files under `spec/` is an open question with the operator,
    /// recorded as open escalation 4 in `ops/phase-1-backlog.md`, and the index
    /// reports them while nothing is corrected until that is answered. They are
    /// written out in section 6 of `ops/methodology-anchor-defects.md`.
    ///
    /// R19 exempts no file and no format. It records where this repository's
    /// unresolvable citations sit today and what this ticket does about them,
    /// which is why the test below treats the recorded set as a ceiling that
    /// may shrink rather than as a standing pass for HTML or for this file.
    const DESIGN_ARTIFACT: &str = "spec/design/Ori Studio.html";

    /// Every citation number in the repository that the index does not
    /// resolve, as of this ticket.
    ///
    /// Under ruling R1 the methodology numbers subsections in exactly two
    /// places, `§24.1` to `§24.8` and `A.1` to `A.5`, so every one of these
    /// names a subsection heading the document does not have.
    ///
    /// The test treats this as a ceiling, not an equality. A number leaving the
    /// set (the repair landing) keeps the test green; a number entering it does
    /// not.
    const RECORDED_UNRESOLVED: [&str; 12] = [
        "11.7", "12.3", "17.3", "17.4", "17.9", "23.1", "23.4", "23.5", "26.1", "27.2", "39.1",
        "39.3",
    ];

    /// One citation as the repository spells it, and where it is.
    #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Citation {
        /// The bare number, in the form [`Index::contains`] takes: `7`, `24.2`,
        /// `A`, `A.5`.
        number: String,
        /// The file, relative to the repository root, with `/` separators.
        file: String,
        /// The 1-based line the citation sits on.
        line: usize,
    }

    /// Every file under the repository root and every citation in them.
    ///
    /// Panics rather than skipping: a file the scan cannot read must not become
    /// a file the scan silently passes over, because the result would be a
    /// clean report of an unchecked repository.
    fn scan_repository() -> (Vec<String>, Vec<Citation>) {
        let root = repo_root();
        let mut files: Vec<String> = Vec::new();
        let mut citations: Vec<Citation> = Vec::new();
        let mut pending = vec![root.clone()];

        while let Some(directory) = pending.pop() {
            let listing = fs::read_dir(&directory)
                .unwrap_or_else(|e| panic!("cannot list {}: {e}", directory.display()));
            for entry in listing {
                let entry = entry.unwrap_or_else(|e| {
                    panic!("cannot read an entry of {}: {e}", directory.display())
                });
                let path = entry.path();
                let relative = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let kind = entry
                    .file_type()
                    .unwrap_or_else(|e| panic!("cannot type {relative}: {e}"));

                if kind.is_dir() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if SCAN_EXCLUSIONS
                        .iter()
                        .any(|(excluded, _)| *excluded == name)
                    {
                        continue;
                    }
                    pending.push(path);
                    continue;
                }
                assert!(
                    kind.is_file(),
                    "{relative} is neither a file nor a directory, so the scan would pass over it \
                     without reading it"
                );

                let bytes =
                    fs::read(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
                citations.extend(citations_in(&bytes, &relative));
                files.push(relative);
            }
        }

        files.sort();
        citations.sort();
        (files, citations)
    }

    /// Every `AICD §<n>` and `AICD appendix <X>` citation in one file.
    ///
    /// Bytes, not text: a file that is not valid UTF-8 would otherwise be
    /// skipped, and a skipped file reads as a file with no bad citations.
    /// `spec/CONVENTIONS.md` fixes the form as `AICD §<n>`, so a bare `§4.2` is
    /// not a citation of the methodology and is not collected. The repository
    /// uses bare `§n` for its own documents' sections in several places, and
    /// collecting those would report fabrications that are not there.
    fn citations_in(bytes: &[u8], file: &str) -> Vec<Citation> {
        let mut found = Vec::new();
        let mut at = 0usize;
        let mut line = 1usize;
        let mut counted_to = 0usize;

        while at + 4 <= bytes.len() {
            if &bytes[at..at + 4] != b"AICD" {
                at += 1;
                continue;
            }
            let after_word = at + 4;
            let marker = skip_blanks(bytes, after_word);
            if marker == after_word {
                // `AICD` glued to whatever follows it, which is a word, not a
                // citation.
                at = after_word;
                continue;
            }
            let Some((number, end)) =
                section_citation(bytes, marker).or_else(|| appendix_citation(bytes, marker))
            else {
                at = after_word;
                continue;
            };

            line += bytes[counted_to..at]
                .iter()
                .filter(|b| **b == b'\n')
                .count();
            counted_to = at;
            found.push(Citation {
                number,
                file: file.to_owned(),
                line,
            });
            at = end;
        }
        found
    }

    /// `§ 24.2` at `at`, as the number and the offset just past it.
    fn section_citation(bytes: &[u8], at: usize) -> Option<(String, usize)> {
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

    /// `appendix A.5` at `at`, as the number and the offset just past it.
    ///
    /// The letter is upper-cased for the lookup. A lower-case letter is a
    /// spelling slip, not a reference to a heading that does not exist, and
    /// reporting it as unresolved would put a fabrication in the report.
    fn appendix_citation(bytes: &[u8], at: usize) -> Option<(String, usize)> {
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

    /// `.5` at `at`, as the digits and the offset just past them.
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

    /// The offset of the first byte at or after `at` that is not a space.
    fn skip_blanks(bytes: &[u8], mut at: usize) -> usize {
        loop {
            match bytes.get(at) {
                Some(byte) if byte.is_ascii_whitespace() => at += 1,
                // A non-breaking space, which the HTML sources carry.
                Some(0xc2) if bytes.get(at + 1) == Some(&0xa0) => at += 2,
                _ => return at,
            }
        }
    }

    /// A citation number spelled back the way the repository writes it.
    ///
    /// An appendix number printed as `\u{a7}D` would be a report of something
    /// the file does not say, which is the defect class this whole test is
    /// against.
    fn spell(number: &str) -> String {
        if number.starts_with(|c: char| c.is_ascii_alphabetic()) {
            format!("AICD appendix {number}")
        } else {
            format!("AICD \u{a7}{number}")
        }
    }

    /// `1 file`, `2 files`: a count that reads as English in the report.
    fn count(many: usize, noun: &str) -> String {
        if many == 1 {
            format!("{many} {noun}")
        } else {
            format!("{many} {noun}s")
        }
    }

    /// The citation report this ticket owes, as a block for the pull request.
    ///
    /// Distinct citations, occurrences, the files they sit in, what the scan
    /// excluded, and every citation the index does not resolve with its number
    /// and its line.
    fn citation_report(
        files: &[String],
        citations: &[Citation],
        unresolved: &[&Citation],
    ) -> String {
        let mut distinct: Vec<&str> = citations.iter().map(|c| c.number.as_str()).collect();
        distinct.sort_unstable();
        distinct.dedup();

        let mut report = String::new();
        report.push_str(&format!(
            "citations: {} distinct, {} occurrences, over {} files\n",
            distinct.len(),
            citations.len(),
            files.len()
        ));
        let mut by_area: BTreeMap<&str, (BTreeSet<&str>, usize)> = BTreeMap::new();
        for citation in citations {
            let area = match citation.file.split_once('/') {
                Some((directory, _)) => directory,
                None => "(repository root)",
            };
            let entry = by_area.entry(area).or_default();
            entry.0.insert(citation.file.as_str());
            entry.1 += 1;
        }
        let carrying: usize = by_area
            .values()
            .map(|(area_files, _)| area_files.len())
            .sum();
        report.push_str(&format!(
            "files carrying citations: {} of {} scanned\n",
            carrying,
            files.len()
        ));
        for (area, (area_files, occurrences)) in &by_area {
            report.push_str(&format!(
                "  {}: {}, {}\n",
                area,
                count(area_files.len(), "file"),
                count(*occurrences, "occurrence")
            ));
        }

        report.push_str(&format!(
            "excluded: {}\n",
            SCAN_EXCLUSIONS
                .iter()
                .map(|(name, why)| format!("{name}/ ({why})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));

        let mut unresolved_numbers: Vec<&str> =
            unresolved.iter().map(|c| c.number.as_str()).collect();
        unresolved_numbers.sort_unstable();
        unresolved_numbers.dedup();
        report.push_str(&format!(
            "unresolved: {} distinct, {} occurrences\n",
            unresolved_numbers.len(),
            unresolved.len()
        ));
        for citation in unresolved {
            report.push_str(&format!(
                "  {}:{} {}\n",
                citation.file,
                citation.line,
                spell(&citation.number)
            ));
        }
        report
    }

    /// Every citation in the repository is resolved against the index, and the
    /// ones that do not resolve are named: AICD §39.
    ///
    /// This is not gate 9. Gate 9 refuses a build on any unresolved citation
    /// and is ticketed as ORI-T-0047, held until the methodology's anchors are
    /// repaired (`ops/phase-1-backlog.md`). What this test holds is the part of
    /// the answer that must not drift: nothing unresolved outside the one file
    /// ruling R19 records, and no new unresolved number inside it either. The
    /// file is not exempt. R19 leaves its twelve citations uncorrected until the
    /// operator answers open escalation 4, and until then this test is what
    /// keeps them from growing.
    #[test]
    fn citations_resolve_everywhere_but_the_recorded_design_artifact() {
        let index = methodology();

        // The recorded set must itself be unresolvable, or the list below is
        // quietly excusing citations that were fine all along.
        for number in RECORDED_UNRESOLVED {
            assert!(
                !index.contains(number),
                "{number} is recorded as unresolvable but the index resolves it"
            );
        }

        let (files, citations) = scan_repository();

        // The floor. A run in which nothing could be checked must not pass.
        assert!(
            !files.is_empty(),
            "the scan visited no file under {}",
            repo_root().display()
        );
        assert!(
            !citations.is_empty(),
            "the scan read {} files and found no citation at all, which means the scanner is \
             broken rather than the repository clean",
            files.len()
        );
        assert!(
            files.iter().any(|file| file == DESIGN_ARTIFACT),
            "{DESIGN_ARTIFACT} was not among the {} files scanned, so the checks below stand \
             for nothing",
            files.len()
        );

        let unresolved: Vec<&Citation> = citations
            .iter()
            .filter(|citation| !index.contains(&citation.number))
            .collect();
        let report = citation_report(&files, &citations, &unresolved);
        println!("{report}");

        let elsewhere: Vec<String> = unresolved
            .iter()
            .filter(|citation| citation.file != DESIGN_ARTIFACT)
            .map(|citation| {
                format!(
                    "{}:{} {}",
                    citation.file,
                    citation.line,
                    spell(&citation.number)
                )
            })
            .collect();
        assert!(
            elsewhere.is_empty(),
            "citations outside {DESIGN_ARTIFACT} that the methodology has no heading for:\n  {}\n\n{report}",
            elsewhere.join("\n  ")
        );

        let mut arrivals: Vec<&str> = unresolved
            .iter()
            .map(|citation| citation.number.as_str())
            .filter(|number| !RECORDED_UNRESOLVED.contains(number))
            .collect();
        arrivals.sort_unstable();
        arrivals.dedup();
        assert!(
            arrivals.is_empty(),
            "unresolved citations in {DESIGN_ARTIFACT} that RECORDED_UNRESOLVED does not list: \
             {arrivals:?}\n\n{report}"
        );
    }

    #[test]
    fn the_scanner_collects_the_citation_form_conventions_fixes_and_no_other() {
        let source = concat!(
            "AICD \u{a7}7 and AICD \u{a7}24.2 and AICD appendix A and AICD appendix A.5\n",
            "AICD &sect;39 written as an entity, AICD Appendix c in lower case\n",
            "a bare \u{a7}4.2 belongs to another document, AICDLike is a word\n",
            "AICD appendix Alpha names no appendix, AICD \u{a7}x has no number\n",
        );
        let found = citations_in(source.as_bytes(), "fixture");
        let numbers: Vec<&str> = found.iter().map(|c| c.number.as_str()).collect();
        assert_eq!(numbers, ["7", "24.2", "A", "A.5", "39", "C"]);
        assert_eq!(found[0].line, 1, "first line");
        assert_eq!(found[4].line, 2, "the entity form is on line 2");
        assert_eq!(found[5].line, 2, "the lower-case appendix is on line 2");
    }

    #[test]
    fn the_scanner_reads_bytes_so_a_file_that_is_not_utf_8_is_still_checked() {
        let mut bytes = vec![0xff, 0xfe, 0x00];
        bytes.extend_from_slice("AICD \u{a7}99".as_bytes());
        bytes.push(0x80);
        let found = citations_in(&bytes, "fixture");
        let numbers: Vec<&str> = found.iter().map(|c| c.number.as_str()).collect();
        assert_eq!(numbers, ["99"], "a citation in undecodable bytes is found");
        assert!(!methodology().contains("99"), "and does not resolve");
    }
}

#[cfg(test)]
mod visibility_probe {
    /// Deliberate failure. AICD 14 visibility demonstration for gate 2.
    /// Throwaway branch, never merged.
    #[test]
    fn gate_2_must_go_red_and_a_human_must_see_it() {
        assert_eq!(1, 2, "deliberate: proving gate 2's failure is visible");
    }
}
