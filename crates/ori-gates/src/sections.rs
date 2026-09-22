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
//! [`crate::sections::AnchorState`] marking the anchors that are absent or
//! duplicated, so the
//! ticket that repairs them can work from this index rather than deriving the
//! same facts again.
//!
//! # Regenerating `methodology/sections.json`
//!
//! [`crate::sections::REGENERATE_COMMAND`] rewrites the committed index from
//! the document. The
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
//! # Why the links above are written `crate::sections::`
//!
//! `lib.rs` carries a `///` doc comment on `pub mod sections;`. When a module
//! is documented from both its declaration site and its own `//!` block,
//! rustdoc resolves the merged result in one scope, and that scope is the
//! declaring module, here the crate root. So an unqualified link to
//! `AnchorState` does not resolve from inside this file even though the item
//! is declared in it, and a `self::`-qualified one does not either. Both were
//! broken on `main` under
//! `RUSTDOCFLAGS="-D warnings"` while the default `cargo doc` stayed green,
//! which is the "present but reporting nothing" class AICD §39 names, applied
//! to the documentation build.
//!
//! A `sections::`-qualified link would also resolve today, and is shorter,
//! and is the wrong choice: it depends on the crate-root scope that the `///` in
//! `lib.rs` happens to impose, so deleting that `///` (the narrowest real fix,
//! and out of ORI-T-0091's declared scope) would break it. The
//! `crate::`-absolute form resolves under both scopes and is what is written
//! here. Do not shorten it.
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
    /// [`Index::parse`] folds CRLF to LF once, before anything here is counted.
    /// The private `lf_line_endings` performs that fold and carries the full
    /// reasoning; `cargo doc --document-private-items` renders it.
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
    /// The fold lives here rather than in [`Index::from_file`] because this
    /// function is public and takes text: a caller that never touched the
    /// filesystem, such as an editor buffer or a fixture, must get the same
    /// index as one that did. The private `lf_line_endings` performs it and
    /// carries the rest of the reasoning, including why a lone CR is content
    /// rather than a line ending; `cargo doc --document-private-items` renders
    /// it.
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
    //
    // ORI-T-0085 adds the exception, and only because the exception is real.
    // The tests named `ori_p1_033_*` at the end of this module exist for
    // criterion ORI-P1-033 and for nothing else: one compares the repository's
    // restatements of the index with the index, and three prove the reader that
    // one depends on. Naming the criterion there is a report of what they cover,
    // not an invention. The three that prove the reader prove the instrument
    // rather than the criterion, and say so.

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

    // -----------------------------------------------------------------------
    // ORI-T-0085: the tie between a restatement of the index and the index.
    //
    // Criterion ORI-P1-033 reads "Carries `MethodologyRef` with a section that
    // resolves in the methodology index". The crate that builds that reference
    // is `ori-core`, and `spec/LLD.md` section 2 forbids it IO and forbids it
    // importing any workspace crate, so it cannot read the index. It restates
    // it instead, in two constants, and checks its own refusals against the
    // restatement. Nothing compared the restatement with the index, because no
    // crate could read both.
    //
    // The gap is not theoretical. On branch `feat/ORI-T-0019-domain-types`,
    // setting `SECTION_COUNT` to 44 leaves all 27 of that crate's tests
    // passing, exit 0. Every refusal would then be free to cite four sections
    // the methodology does not have, and the criterion would be false with no
    // test to say so. That is the defect class AICD §39 names, "present but
    // reporting nothing".
    //
    // This module can read both: it already parses the methodology and already
    // walks every file in the repository. So the comparison lives here, and it
    // reads the restatement out of Rust source as text.
    //
    // The comparison is made against the index parsed from the methodology
    // document, not against the bytes of `methodology/sections.json`. The two
    // are already tied by
    // `committed_sections_json_matches_a_fresh_parse_of_the_methodology`, so
    // going through the JSON would add a second parser and a second way to be
    // wrong without adding a fact.
    // -----------------------------------------------------------------------

    /// The name of the constant that restates how many sections there are.
    const SECTION_COUNT_NAME: &str = "SECTION_COUNT";

    /// The name of the constant that restates which subsections there are.
    const SUBSECTIONS_NAME: &str = "NUMBERED_SUBSECTIONS";

    /// Both names, for the sweep that looks for a restatement nobody registered.
    const RESTATED_CONSTANTS: [&str; 2] = [SECTION_COUNT_NAME, SUBSECTIONS_NAME];

    /// Every file that restates the index in Rust source, with why it must.
    ///
    /// A file listed here and absent from the tree is a failure, not a skip.
    /// A check that passes when its subject is missing reports nothing, and a
    /// check that reports nothing is what this test exists to remove; it would
    /// also go on reporting nothing after the subject came back under a name
    /// the list no longer matched.
    ///
    /// A reason here names the standing constraint that forces the
    /// restatement, and nothing else. It does not name a pull request, a
    /// branch or a ticket whose state can change after this line is written.
    /// The reason is handed to a human as the explanation of a live failure,
    /// so a clause that has since stopped being true does not merely age: it
    /// misdirects the one reader who is already looking at something broken.
    /// The previous text here said pull request 27 was "open and unmerged"; it
    /// had merged before that sentence reached `main`, so it was false on
    /// arrival and stayed false. Merge state is knowable from the history at
    /// any time and belongs to whoever is asking, never to a constant.
    const RESTATEMENTS: [(&str, &str); 1] = [(
        "crates/ori-core/src/error.rs",
        "`ori-core` may not do IO and may not import a workspace crate \
         (`spec/LLD.md` section 2, and CLAUDE.md's load-bearing facts), so it \
         restates the index rather than reading it. If the file moved rather \
         than went away, the repair is to correct this entry and not to drop \
         it: a restatement nobody registered is a restatement nobody checks",
    )];

    /// Files that carry the text of a restatement without being one.
    ///
    /// Checked, not trusted: the test below asserts each of these still carries
    /// the text it is exempted for, so an exemption that has outlived its
    /// reason is a failure rather than a permanent blind spot.
    const NOT_A_RESTATEMENT: [(&str, &str); 1] = [(
        "crates/ori-gates/src/sections.rs",
        "this file, whose fixtures quote a restatement in order to prove the \
         reader that finds one",
    )];

    /// What one file's Rust source says the methodology index holds.
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Restated {
        /// The type [`SECTION_COUNT_NAME`] is declared as, for the message.
        declared: String,
        /// The value of [`SECTION_COUNT_NAME`].
        sections: u64,
        /// The pairs of [`SUBSECTIONS_NAME`], in the order they are written.
        subsections: Vec<(u64, String)>,
    }

    /// The first offset at or after `at` that is neither whitespace nor comment.
    ///
    /// Nested block comments are not handled: `/* /* */ */` is taken to end at
    /// the first `*/`. The reader below refuses what it cannot parse, so the
    /// consequence of that simplification is a loud failure and never a wrong
    /// answer.
    fn noise_end(source: &str, mut at: usize) -> usize {
        let bytes = source.as_bytes();
        loop {
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            let tail = &source[at..];
            if let Some(rest) = tail.strip_prefix("//") {
                at += 2 + rest.find('\n').map_or(rest.len(), |end| end + 1);
                continue;
            }
            if let Some(rest) = tail.strip_prefix("/*") {
                at += 2 + rest.find("*/").map_or(rest.len(), |end| end + 2);
                continue;
            }
            return at;
        }
    }

    /// Whether a byte can sit inside a Rust identifier.
    fn is_ident_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    /// A position in Rust source that reads tokens and skips comments.
    ///
    /// Deliberately not a Rust lexer. It knows the one grammar the two
    /// constants are written in and refuses everything else, because a reader
    /// that guesses at a form it was not given would report agreement it never
    /// checked.
    struct Cursor<'a> {
        source: &'a str,
        at: usize,
    }

    impl<'a> Cursor<'a> {
        fn new(source: &'a str, at: usize) -> Self {
            Cursor { source, at }
        }

        /// Moves past whitespace and comments.
        fn skip(&mut self) {
            self.at = noise_end(self.source, self.at);
        }

        /// Consumes `token` if it is next, and says whether it was.
        fn eat(&mut self, token: &str) -> bool {
            self.skip();
            if self.source[self.at..].starts_with(token) {
                self.at += token.len();
                true
            } else {
                false
            }
        }

        /// Moves past the next `token`, or says the source ran out first.
        ///
        /// Used only to cross a type annotation, which carries no string and no
        /// `=`, so it does not need to know either.
        fn seek(&mut self, token: &str) -> bool {
            loop {
                self.skip();
                if self.at >= self.source.len() {
                    return false;
                }
                if self.eat(token) {
                    return true;
                }
                let step = self.source[self.at..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
                self.at += step;
            }
        }

        /// A Rust identifier, such as the type a constant is declared as.
        fn identifier(&mut self) -> Option<String> {
            self.skip();
            let tail = &self.source[self.at..];
            let read = tail
                .char_indices()
                .find(|(_, character)| !(character.is_ascii_alphanumeric() || *character == '_'))
                .map_or(tail.len(), |(offset, _)| offset);
            if read == 0 {
                return None;
            }
            self.at += read;
            Some(tail[..read].to_string())
        }

        /// A decimal integer literal, with `_` separators and any type suffix.
        ///
        /// `40`, `40u8` and `4_0` are the same literal, and all three are read,
        /// so a change of integer form is not a change of meaning here.
        fn integer(&mut self) -> Option<u64> {
            self.skip();
            let tail = &self.source[self.at..];
            let mut digits = String::new();
            let mut read = 0usize;
            for character in tail.chars() {
                if character.is_ascii_digit() {
                    digits.push(character);
                } else if character != '_' {
                    break;
                }
                read += character.len_utf8();
            }
            if digits.is_empty() {
                return None;
            }
            for character in tail[read..].chars() {
                if character.is_ascii_alphanumeric() || character == '_' {
                    read += character.len_utf8();
                } else {
                    break;
                }
            }
            self.at += read;
            digits.parse().ok()
        }

        /// A double-quoted string literal.
        ///
        /// Raw strings and escapes other than the six below are refused rather
        /// than guessed at, for the reason the type carries: a guess is a report
        /// of something the source does not say.
        fn string(&mut self) -> Option<String> {
            self.skip();
            let bytes = self.source.as_bytes();
            if bytes.get(self.at) != Some(&b'"') {
                return None;
            }
            let mut at = self.at + 1;
            let mut value = String::new();
            while at < bytes.len() {
                match bytes[at] {
                    b'"' => {
                        self.at = at + 1;
                        return Some(value);
                    }
                    b'\\' => {
                        value.push(match *bytes.get(at + 1)? {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            b'0' => '\0',
                            _ => return None,
                        });
                        at += 2;
                    }
                    _ => {
                        let character = self.source[at..].chars().next()?;
                        value.push(character);
                        at += character.len_utf8();
                    }
                }
            }
            None
        }

        /// The next forty characters, quoted, for a message that has to say
        /// what was found instead of what was looked for.
        fn rest(&self) -> String {
            let at = noise_end(self.source, self.at);
            let tail = &self.source[at..];
            let cut = tail
                .char_indices()
                .nth(40)
                .map_or(tail.len(), |(offset, _)| offset);
            format!("{:?}", &tail[..cut])
        }
    }

    /// A cursor just past the one `const <name>` in `source`.
    ///
    /// None and several are both refused. None is the reformat or the rename
    /// that would otherwise turn this whole test into a green run that compared
    /// nothing, which AICD §39 names as the defect worth more than the one it
    /// hides. Several is ambiguity, and picking one would be a guess.
    fn declaration<'a>(source: &'a str, name: &str) -> Result<Cursor<'a>, String> {
        let needle = format!("const {name}");
        let bytes = source.as_bytes();
        let mut ends: Vec<usize> = Vec::new();
        let mut at = 0usize;
        while let Some(offset) = source[at..].find(&needle) {
            let start = at + offset;
            let end = start + needle.len();
            let before = start == 0 || !is_ident_byte(bytes[start - 1]);
            let after = match bytes.get(end) {
                None => true,
                Some(byte) => !is_ident_byte(*byte),
            };
            if before && after {
                ends.push(end);
            }
            at = end;
        }
        match ends.len() {
            1 => Ok(Cursor::new(source, ends[0])),
            0 => Err(format!(
                "there is no `const {name}` here. It was renamed, removed, or written in a form \
                 this reader does not recognise, so nothing at all was compared against \
                 {OUTPUT_PATH}. That is why this is a failure rather than a pass: the reader \
                 reports what it checked, and it checked nothing."
            )),
            many => Err(format!(
                "there are {many} declarations of `const {name}` here, so which one restates \
                 {OUTPUT_PATH} is ambiguous and none was used."
            )),
        }
    }

    /// The type and value of `const <name>: <type> = <integer>;`.
    fn integer_constant(source: &str, name: &str) -> Result<(String, u64), String> {
        let mut cursor = declaration(source, name)?;
        if !cursor.eat(":") {
            return Err(format!(
                "`const {name}` is not followed by `: <type>` but by {}",
                cursor.rest()
            ));
        }
        let Some(declared) = cursor.identifier() else {
            return Err(format!(
                "`const {name}` is not declared with a plain type name but with {}",
                cursor.rest()
            ));
        };
        if !cursor.eat("=") {
            return Err(format!(
                "`const {name}: {declared}` is not followed by `=` but by {}",
                cursor.rest()
            ));
        }
        let Some(value) = cursor.integer() else {
            return Err(format!(
                "`const {name}: {declared}` is not given an integer literal but {}",
                cursor.rest()
            ));
        };
        if !cursor.eat(";") {
            return Err(format!(
                "`const {name}: {declared} = {value}` is not closed by `;` but by {}",
                cursor.rest()
            ));
        }
        Ok((declared, value))
    }

    /// The pairs of `const <name>: &[(<integer>, &str)] = &[(n, "s"), ...];`.
    fn pair_slice_constant(source: &str, name: &str) -> Result<Vec<(u64, String)>, String> {
        let mut cursor = declaration(source, name)?;
        if !cursor.eat(":") {
            return Err(format!(
                "`const {name}` is not followed by `: <type>` but by {}",
                cursor.rest()
            ));
        }
        if !cursor.seek("=") {
            return Err(format!(
                "`const {name}` is declared with no `=` and so no value"
            ));
        }
        if !cursor.eat("&") || !cursor.eat("[") {
            return Err(format!(
                "`const {name}` is not given a `&[...]` slice literal but {}",
                cursor.rest()
            ));
        }
        let mut pairs: Vec<(u64, String)> = Vec::new();
        loop {
            if cursor.eat("]") {
                break;
            }
            let nth = pairs.len() + 1;
            if !cursor.eat("(") {
                return Err(format!(
                    "`const {name}` entry {nth} does not open with `(` but with {}",
                    cursor.rest()
                ));
            }
            let Some(section) = cursor.integer() else {
                return Err(format!(
                    "`const {name}` entry {nth} has no section number but {}",
                    cursor.rest()
                ));
            };
            if !cursor.eat(",") {
                return Err(format!(
                    "`const {name}` entry {nth} has no `,` after {section} but {}",
                    cursor.rest()
                ));
            }
            let Some(subsection) = cursor.string() else {
                return Err(format!(
                    "`const {name}` entry {nth} has no quoted subsection but {}",
                    cursor.rest()
                ));
            };
            cursor.eat(",");
            if !cursor.eat(")") {
                return Err(format!(
                    "`const {name}` entry {nth} does not close with `)` but with {}",
                    cursor.rest()
                ));
            }
            pairs.push((section, subsection));
            if cursor.eat(",") {
                continue;
            }
            if cursor.eat("]") {
                break;
            }
            return Err(format!(
                "`const {name}` has neither `,` nor `]` after entry {nth} but {}",
                cursor.rest()
            ));
        }
        if !cursor.eat(";") {
            return Err(format!(
                "`const {name}` is not closed by `;` but by {}",
                cursor.rest()
            ));
        }
        if pairs.is_empty() {
            return Err(format!(
                "`const {name}` is an empty slice, so nothing was compared against {OUTPUT_PATH}"
            ));
        }
        Ok(pairs)
    }

    /// What one file's source says the index holds, or why it could not be read.
    fn read_restatement(source: &str) -> Result<Restated, String> {
        let (declared, sections) = integer_constant(source, SECTION_COUNT_NAME)?;
        let subsections = pair_slice_constant(source, SUBSECTIONS_NAME)?;
        Ok(Restated {
            declared,
            sections,
            subsections,
        })
    }

    /// A list for a message, cut short before it stops being readable.
    fn at_most(items: &[String], cap: usize) -> String {
        if items.len() <= cap {
            return items.join(", ");
        }
        format!(
            "{}, and {} more",
            items[..cap].join(", "),
            items.len() - cap
        )
    }

    /// Every way a restatement disagrees with the index, in the words a human
    /// needs in order to fix it.
    ///
    /// Returns the disagreements rather than asserting, so the same comparison
    /// serves the repository and the fixtures the planted defects are applied
    /// to, and so one run names every defect instead of the first.
    fn restatement_defects(restated: &Restated, index: &Index) -> Vec<String> {
        let mut defects: Vec<String> = Vec::new();

        let carried = index.count(EntryKind::Section) as u64;
        let stated = restated.sections;
        if stated != carried {
            let declared = &restated.declared;
            let consequence = if stated > carried {
                let fabricated: Vec<String> = ((carried + 1)..=stated)
                    .map(|n| spell(&n.to_string()))
                    .collect();
                format!(
                    "{} would be built without complaint and resolve against no heading",
                    at_most(&fabricated, 8)
                )
            } else {
                let lost: Vec<String> = ((stated + 1)..=carried)
                    .map(|n| spell(&n.to_string()))
                    .collect();
                format!(
                    "{} are headings of the methodology that no refusal can cite",
                    at_most(&lost, 8)
                )
            };
            defects.push(format!(
                "`{SECTION_COUNT_NAME}: {declared} = {stated}` restates {} as carrying {stated} \
                 numbered sections; it carries {carried}, so {consequence}",
                index.source
            ));
        }

        // `spec/LLD.md` section 4 types `MethodologyRef::section` as `u8`, so a
        // subsection of an appendix (`A.1`) cannot be written as one. Those are
        // not expected in the restatement and their absence is not a defect.
        let mut expressible: BTreeSet<(u64, String)> = BTreeSet::new();
        for entry in index
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Subsection)
        {
            let Some((section, subsection)) = entry.number.split_once('.') else {
                defects.push(format!(
                    "the index carries a subsection numbered {} with no `.` in it, which this \
                     comparison cannot place, so it is reported rather than passed over",
                    entry.number
                ));
                continue;
            };
            if let Ok(number) = section.parse::<u64>() {
                expressible.insert((number, subsection.to_string()));
            }
        }

        let listed: BTreeSet<(u64, String)> = restated.subsections.iter().cloned().collect();
        if listed.len() != restated.subsections.len() {
            defects.push(format!(
                "`{SUBSECTIONS_NAME}` lists {} pairs of which only {} are distinct",
                restated.subsections.len(),
                listed.len()
            ));
        }
        for (section, subsection) in expressible.difference(&listed) {
            defects.push(format!(
                "the methodology has a heading {} and `{SUBSECTIONS_NAME}` does not carry \
                 ({section}, {subsection:?}), so no refusal can cite it",
                spell(&format!("{section}.{subsection}"))
            ));
        }
        for (section, subsection) in listed.difference(&expressible) {
            defects.push(format!(
                "`{SUBSECTIONS_NAME}` carries ({section}, {subsection:?}) and the methodology has \
                 no heading {}, so a refusal citing it would resolve against nothing",
                spell(&format!("{section}.{subsection}"))
            ));
        }

        defects
    }

    /// Whether `bytes` declares `const <name>` at a token boundary.
    ///
    /// Bytes, not text, for the reason [`citations_in`] gives: a file that is
    /// not valid UTF-8 must not become a file the sweep passes over in silence.
    fn declares(bytes: &[u8], name: &str) -> bool {
        let needle = format!("const {name}");
        let needle = needle.as_bytes();
        bytes.windows(needle.len()).enumerate().any(|(at, window)| {
            window == needle
                && (at == 0 || !is_ident_byte(bytes[at - 1]))
                && match bytes.get(at + needle.len()) {
                    None => true,
                    Some(byte) => !is_ident_byte(*byte),
                }
        })
    }

    /// ORI-P1-033: every restatement of the methodology index in this
    /// repository says what the index says.
    ///
    /// Derived from criterion ORI-P1-033 in `spec/criteria/phase-1.md`, "Carries
    /// `MethodologyRef` with a section that resolves in the methodology index",
    /// and from AICD §39, which is where the rule that references are checked
    /// mechanically comes from. The criterion's own tests live in `ori-core` and
    /// check refusals against that crate's restatement of the index; this test
    /// is the missing half, the one that checks the restatement against the
    /// index, without which the criterion can be false with every test green.
    ///
    /// The three other tests named `ori_p1_033_reader_*` prove the reader this
    /// one depends on, on fixtures carrying planted defects (AICD §14). They
    /// prove the instrument, not the criterion.
    #[test]
    fn ori_p1_033_every_restatement_of_the_index_agrees_with_the_index() {
        let index = methodology();
        let root = repo_root();
        let mut defects: Vec<String> = Vec::new();

        for (path, why) in RESTATEMENTS {
            match fs::read_to_string(root.join(path)) {
                Ok(source) => match read_restatement(&source) {
                    Ok(restated) => defects.extend(
                        restatement_defects(&restated, &index)
                            .into_iter()
                            .map(|defect| format!("{path}: {defect}")),
                    ),
                    Err(reason) => defects.push(format!("{path}: {reason}")),
                },
                Err(error) => defects.push(format!(
                    "{path}: cannot be read ({error}). It is registered as a restatement of \
                     {OUTPUT_PATH} because {why}. A registered restatement that is absent is a \
                     failure and not a skip, because a check that passes when its subject is \
                     missing is a check that reports nothing."
                )),
            }
        }

        // The sweep: a restatement nobody registered is a restatement nobody
        // checks, and the register is a hand-kept list, which is the thing this
        // repository keeps finding to have gone quietly stale.
        for (path, why) in NOT_A_RESTATEMENT {
            let bytes = fs::read(root.join(path))
                .unwrap_or_else(|error| panic!("cannot read the exempt {path}: {error}"));
            assert!(
                RESTATED_CONSTANTS.iter().any(|name| declares(&bytes, name)),
                "{path} is exempt from the restatement sweep because it is {why}, and it now \
                 declares neither {SECTION_COUNT_NAME} nor {SUBSECTIONS_NAME}. The exemption has \
                 outlived its reason and is a blind spot until it is removed."
            );
        }

        let (files, _) = scan_repository();
        assert!(
            !files.is_empty(),
            "the scan visited no file under {}, so the sweep below stands for nothing",
            root.display()
        );
        let registered: BTreeSet<&str> = RESTATEMENTS
            .iter()
            .chain(NOT_A_RESTATEMENT.iter())
            .map(|(path, _)| *path)
            .collect();
        for file in &files {
            if registered.contains(file.as_str()) {
                continue;
            }
            let bytes = fs::read(root.join(file))
                .unwrap_or_else(|error| panic!("cannot read {file}: {error}"));
            for name in RESTATED_CONSTANTS {
                if declares(&bytes, name) {
                    defects.push(format!(
                        "{file} declares `const {name}` and is in neither RESTATEMENTS nor \
                         NOT_A_RESTATEMENT, so a second restatement of {OUTPUT_PATH} exists that \
                         nothing compares with it"
                    ));
                }
            }
        }

        assert!(
            defects.is_empty(),
            "{} between the repository's restatements of {OUTPUT_PATH} and {}:\n  {}",
            count(defects.len(), "disagreement"),
            index.source,
            defects.join("\n  ")
        );
    }

    // ---- the reader, on fixtures written for the purpose (AICD §14) ----

    /// A restatement in the form `crates/ori-core/src/error.rs` writes it on
    /// branch `feat/ORI-T-0019-domain-types`, trimmed to what is read.
    ///
    /// The fixture proves the reader; the test above proves the fixture is the
    /// form the repository actually uses, because it runs the same reader over
    /// the real file and refuses what it cannot parse.
    const RESTATEMENT_FIXTURE: &str = concat!(
        "//! [`SECTION_COUNT`] and [`NUMBERED_SUBSECTIONS`] restate the index.\n",
        "\n",
        "/// How many numbered sections the methodology carries.\n",
        "pub const SECTION_COUNT: u8 = 40;\n",
        "\n",
        "/// Every subsection the methodology numbers, as (section, subsection).\n",
        "pub const NUMBERED_SUBSECTIONS: &[(u8, &str)] = &[\n",
        "    (24, \"1\"),\n",
        "    (24, \"2\"),\n",
        "    (24, \"3\"),\n",
        "    (24, \"4\"),\n",
        "    (24, \"5\"),\n",
        "    (24, \"6\"),\n",
        "    (24, \"7\"),\n",
        "    (24, \"8\"),\n",
        "];\n",
        "\n",
        "fn resolves(reference: &MethodologyRef) -> bool {\n",
        "    reference.section >= 1 && reference.section <= SECTION_COUNT\n",
        "}\n",
    );

    /// The fixture with one substitution, which must actually substitute.
    ///
    /// A planted defect that failed to land leaves the check passing on an
    /// unaltered fixture and that pass being recorded as a proof, which is the
    /// inverted instrument of AICD §14 and of `ops/calibration.md` CR-004.
    fn planted(from: &str, to: &str) -> String {
        assert!(
            RESTATEMENT_FIXTURE.contains(from),
            "the fixture does not contain {from:?}, so this defect was never planted and \
             whatever the check says next is about the unaltered fixture"
        );
        let altered = RESTATEMENT_FIXTURE.replace(from, to);
        assert_ne!(
            altered, RESTATEMENT_FIXTURE,
            "the substitution left the fixture unchanged"
        );
        altered
    }

    /// The defects the reader and the comparison find in one fixture.
    fn fixture_defects(source: &str) -> Vec<String> {
        match read_restatement(source) {
            Ok(restated) => restatement_defects(&restated, &methodology()),
            Err(reason) => vec![reason],
        }
    }

    #[test]
    fn ori_p1_033_reader_reads_the_form_ori_core_writes_and_finds_it_agrees() {
        let restated = read_restatement(RESTATEMENT_FIXTURE).expect("the fixture is readable");
        assert_eq!(restated.declared, "u8", "the declared type");
        assert_eq!(restated.sections, 40, "the restated section count");
        assert_eq!(
            restated.subsections,
            (1..=8)
                .map(|n| (24u64, n.to_string()))
                .collect::<Vec<(u64, String)>>(),
            "the restated subsections"
        );
        assert!(
            restatement_defects(&restated, &methodology()).is_empty(),
            "the unaltered fixture disagrees with the methodology"
        );
    }

    #[test]
    fn ori_p1_033_reader_reads_the_constants_however_they_are_spaced() {
        // An innocent reformat must not be a failure, or the check becomes a
        // thing people route around. Four forms of the same two constants:
        // one line, a suffixed literal, an interleaved comment, no trailing
        // comma.
        let reformatted = concat!(
            "pub const SECTION_COUNT : u8=4_0u8 ;\n",
            "pub const NUMBERED_SUBSECTIONS: &[(u8, &str)] = &[(24,\"1\"),(24,\"2\"),\n",
            "  /* a comment mid-slice */ (24,\"3\",),(24,\"4\"),(24,\"5\"),(24,\"6\"),\n",
            "  (24,\"7\"), // and a line comment\n",
            "  (24,\"8\")];\n",
        );
        let restated = read_restatement(reformatted).expect("the reformatted source is readable");
        assert_eq!(restated.sections, 40);
        assert_eq!(restated.subsections.len(), 8);
        assert!(restatement_defects(&restated, &methodology()).is_empty());
    }

    #[test]
    fn ori_p1_033_reader_catches_a_restatement_that_disagrees_with_the_index() {
        let wrong_count = fixture_defects(&planted(
            "SECTION_COUNT: u8 = 40;",
            "SECTION_COUNT: u8 = 44;",
        ));
        assert_eq!(wrong_count.len(), 1, "one defect, found: {wrong_count:?}");
        assert!(
            wrong_count[0].contains("carrying 44") && wrong_count[0].contains("carries 40"),
            "the message does not say what disagrees: {}",
            wrong_count[0]
        );

        let removed = fixture_defects(&planted("    (24, \"8\"),\n", ""));
        assert_eq!(removed.len(), 1, "one defect, found: {removed:?}");
        assert!(
            removed[0].contains("24.8") && removed[0].contains("does not carry"),
            "the message does not name the missing subsection: {}",
            removed[0]
        );

        let invented = fixture_defects(&planted(
            "    (24, \"8\"),\n",
            "    (24, \"8\"),\n    (24, \"9\"),\n",
        ));
        assert_eq!(invented.len(), 1, "one defect, found: {invented:?}");
        assert!(
            invented[0].contains("24.9") && invented[0].contains("no heading"),
            "the message does not name the invented subsection: {}",
            invented[0]
        );
    }

    #[test]
    fn ori_p1_033_reader_refuses_a_constant_it_can_no_longer_find() {
        // The defect that matters most. A rename or a rewrite that the reader
        // cannot parse must be a failure, because the alternative is a reader
        // that finds nothing, compares nothing and reports success forever
        // (AICD §39, "present but reporting nothing").
        for (what, source) in [
            (
                "renamed",
                planted("const SECTION_COUNT:", "const SECTION_TOTAL:"),
            ),
            (
                "rewritten as a function",
                planted(
                    "pub const SECTION_COUNT: u8 = 40;",
                    "pub const fn section_count() -> u8 { 40 }",
                ),
            ),
            (
                "rewritten as a macro invocation",
                planted(
                    "pub const NUMBERED_SUBSECTIONS: &[(u8, &str)] = &[",
                    "pub const NUMBERED_SUBSECTIONS: &[(u8, &str)] = subsections![",
                ),
            ),
            (
                "given an entry in a shape the reader was not taught",
                planted(
                    "    (24, \"8\"),\n",
                    "    Pair { section: 24, subsection: \"8\" },\n",
                ),
            ),
            (
                "emptied",
                planted(
                    concat!(
                        "    (24, \"1\"),\n    (24, \"2\"),\n    (24, \"3\"),\n    (24, \"4\"),\n",
                        "    (24, \"5\"),\n    (24, \"6\"),\n    (24, \"7\"),\n    (24, \"8\"),\n"
                    ),
                    "",
                ),
            ),
            (
                "declared twice",
                planted(
                    "pub const SECTION_COUNT: u8 = 40;",
                    "pub const SECTION_COUNT: u8 = 40;\npub const SECTION_COUNT: u8 = 40;",
                ),
            ),
        ] {
            let reason = read_restatement(&source)
                .err()
                .unwrap_or_else(|| panic!("a constant {what} was read as if nothing had changed"));
            assert!(
                reason.contains(SECTION_COUNT_NAME) || reason.contains(SUBSECTIONS_NAME),
                "the refusal for a constant {what} does not name the constant: {reason}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // ORI-T-0091: doc comments that name a test function in prose.
    //
    // `crates/ori-core/src/error.rs` explains where the check on its own
    // restatement lives by naming test functions of this file, in prose, in
    // backticks. Nothing tied those names to the tests. Renaming or deleting
    // any of them left that doc comment false with every test in the workspace
    // green, which is the "present but reporting nothing" class AICD §39 names,
    // and it is the class the first of those tests was written to remove.
    // ORI-T-0086 corrected a doc comment in that file and the correction
    // created this instance of the defect it was correcting.
    //
    // Rustdoc resolves an intra-doc link and refuses to build when it cannot.
    // It does not look inside a bare backticked span. The bare span is
    // therefore the whole of the gap, and it is what this check reads.
    //
    // # Which backticked strings are claims, and which are prose
    //
    // A doc comment is full of backticks. Paths, commands, types, attributes,
    // crate names and ordinary emphasis all wear them. A checker that tried to
    // resolve all of them would drown in false positives and be deleted; one
    // that resolved too few would be the defect it was built to remove.
    //
    // The line below was measured, not guessed. Across every Rust file in the
    // workspace there are exactly seven distinct bare backticked spans shaped
    // like a snake_case identifier: three name test functions, one names a
    // method, and three name things that are not Rust at all. A population
    // that size can be registered by hand and argued with, so the rule is:
    //
    //   A bare backticked span in a `///` or `//!` comment is a claim about
    //   this repository's Rust source when its text matches
    //   `[a-z][a-z0-9]*(_[a-z0-9]+)+`, and only then.
    //
    // Deliberately outside it, each for a reason that would otherwise produce
    // noise no reader could act on:
    //
    //   - Intra-doc links. Rustdoc already resolves them, and a second
    //     resolver would be a second way to be wrong. Four of them were broken
    //     on `main` when this ticket opened and rustdoc is what found them.
    //   - Any span carrying `/`, `.`, `::`, `-`, a space or an upper-case
    //     letter: paths, commands, types, constants, qualified names. None is
    //     a bare function name and each class would need its own resolver.
    //   - A lower-case word with no underscore, such as `std` or `id`. Not
    //     distinguishable from English or from a crate name, so the false
    //     positive rate would be total.
    //   - Anything inside a fenced code block. Samples are illustrations, not
    //     claims about what this repository declares.
    //
    // # Why a register and a sweep, and not either alone
    //
    // The register is what fails when a cited test is renamed or deleted: the
    // registered name stops resolving. The sweep is what fails when a citation
    // appears that nobody registered. A register alone goes stale, which is the
    // failure this repository keeps finding in its own hand-kept lists. A sweep
    // alone can never fail, because a name that resolves to nothing is
    // indistinguishable from prose until a human has said it is a citation.
    // -----------------------------------------------------------------------

    /// The roots whose Rust source this check reads, each with why it is in.
    ///
    /// `fixtures/` is deliberately out. The crates under it carry planted
    /// defects and exist to be compiled and refused by the gate harness, so a
    /// cited test name that resolved only against a fixture would be a false
    /// pass on a file that is an input to a check rather than part of this
    /// repository's own suite. The test below asserts each root was actually
    /// reached, so an exclusion cannot quietly become a scan of nothing.
    const DOC_SCAN_ROOTS: [(&str, &str); 2] = [
        ("crates/", "the workspace's own crates"),
        ("apps/", "the desktop shell, which is workspace source too"),
    ];

    /// Every doc comment in the workspace that names a test function in prose.
    ///
    /// The four fields are the file whose doc comment says it, the name it
    /// says, the file the test is declared in, and why the name appears as
    /// prose rather than as an intra-doc link that rustdoc would resolve.
    ///
    /// A registered name that no `#[test] fn` answers to is a failure and not
    /// a skip, for the reason [`RESTATEMENTS`] gives: a check that passes when
    /// its subject is missing reports nothing, and would go on reporting
    /// nothing after the subject came back under a name the register no longer
    /// matched. Renaming a cited test is therefore a three-part edit, the
    /// test, the prose and this register, and the failure message says so.
    const CITED_TESTS: [(&str, &str, &str, &str); 5] = [
        (
            "crates/ori-core/src/error.rs",
            "ori_p1_033_every_restatement_of_the_index_agrees_with_the_index",
            "crates/ori-gates/src/sections.rs",
            "`ori-core` may not import a workspace crate (`spec/LLD.md` section 2), and the test \
             is a `#[cfg(test)]` item in another crate, so no intra-doc link written from here \
             could resolve however it were spelled",
        ),
        (
            "crates/ori-core/src/error.rs",
            "committed_sections_json_matches_a_fresh_parse_of_the_methodology",
            "crates/ori-gates/src/sections.rs",
            "the same bar: a `#[cfg(test)]` item in a crate `ori-core` may not import",
        ),
        (
            "crates/ori-core/src/error.rs",
            "citations_resolve_everywhere_but_the_recorded_design_artifact",
            "crates/ori-gates/src/sections.rs",
            "the same bar again. ORI-T-0091's ticket named two citations in this file and there \
             are three, which is why the sweep below exists and the register is not trusted to \
             be complete on its own",
        ),
        (
            "crates/ori-gates/src/sections.rs",
            "committed_sections_json_matches_a_fresh_parse_of_the_methodology",
            "crates/ori-gates/src/sections.rs",
            "rustdoc does not document `#[cfg(test)]` items, so a link to this test cannot \
             resolve even from the file that declares it",
        ),
        (
            "crates/ori-gates/src/sections.rs",
            "citations_resolve_everywhere_but_the_recorded_design_artifact",
            "crates/ori-gates/src/sections.rs",
            "the same bar: a `#[cfg(test)]` item rustdoc will not document",
        ),
    ];

    /// Bare snake_case spans in a citing file that name no Rust function.
    ///
    /// Checked, not trusted, on the pattern [`NOT_A_RESTATEMENT`] sets: the
    /// test asserts each of these is still written in the file it exempts, so
    /// an exemption that has outlived its reason is a failure rather than a
    /// permanent blind spot.
    const NOT_AN_IDENTIFIER: [(&str, &str, &str); 2] = [
        (
            "crates/ori-core/src/error.rs",
            "aicd_plan_submit",
            "a tool of this product's own MCP surface, specified in `spec/API_SPEC.md` and not \
             built by phase 1. Escalation E-0005 records that it does not exist. It names a tool \
             a human or agent calls, never a Rust function",
        ),
        (
            "crates/ori-core/src/error.rs",
            "contract_change",
            "an escalation trigger from CLAUDE.md's list, a string passed to a tool, never a \
             Rust function",
        ),
    ];

    /// One bare backticked span from a doc comment, and where it is written.
    #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct DocSpan {
        /// The file, relative to the repository root, with `/` separators.
        file: String,
        /// The 1-based line the span sits on.
        line: usize,
        /// The text between the backticks.
        text: String,
    }

    /// Whether `text` is a plain snake_case identifier.
    ///
    /// The whole of the rule stated above, in one place: lower-case ASCII and
    /// digits, at least one interior underscore, no leading or trailing
    /// underscore and no doubled one. Everything a doc comment backticks that
    /// is not this shape is prose as far as this check is concerned.
    fn is_snake_case_identifier(text: &str) -> bool {
        let bytes = text.as_bytes();
        if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
            return false;
        }
        let mut underscores = 0usize;
        let mut after_underscore = false;
        for &byte in &bytes[1..] {
            if byte == b'_' {
                if after_underscore {
                    return false;
                }
                underscores += 1;
                after_underscore = true;
            } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
                after_underscore = false;
            } else {
                return false;
            }
        }
        underscores >= 1 && !after_underscore
    }

    /// Every bare backticked snake_case span in one file's doc comments.
    ///
    /// Intra-doc links are stepped over rather than collected: rustdoc
    /// resolves those and this check must not become a second answer to a
    /// question already answered. Fenced blocks are stepped over because a
    /// sample is an illustration and not a claim.
    fn doc_spans(source: &str, file: &str) -> Vec<DocSpan> {
        let mut found = Vec::new();
        let mut fenced = false;

        for (offset, raw) in source.lines().enumerate() {
            let trimmed = raw.trim_start();
            let body = match trimmed
                .strip_prefix("///")
                .or_else(|| trimmed.strip_prefix("//!"))
            {
                Some(body) => body.trim_start(),
                None => continue,
            };
            if body.starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }

            let bytes = body.as_bytes();
            let mut at = 0usize;
            while at < bytes.len() {
                if bytes[at] == b'[' && bytes.get(at + 1) == Some(&b'`') {
                    match body[at + 2..].find("`]") {
                        Some(end) => at += 2 + end + 2,
                        None => at += 1,
                    }
                    continue;
                }
                if bytes[at] == b'`' {
                    let Some(end) = body[at + 1..].find('`') else {
                        break;
                    };
                    let text = &body[at + 1..at + 1 + end];
                    if is_snake_case_identifier(text) {
                        found.push(DocSpan {
                            file: file.to_string(),
                            line: offset + 1,
                            text: text.to_string(),
                        });
                    }
                    at += 1 + end + 1;
                    continue;
                }
                at += 1;
            }
        }

        found
    }

    /// The words Rust allows between the start of a declaration line and `fn`.
    ///
    /// The reader below admits a line as a declaration only when everything
    /// before `fn` is drawn from this set. That is what keeps it from reading
    /// the `fn` in a sentence or the `fn` inside a quoted source fixture, both
    /// of which this file is full of: the fixtures that prove these readers
    /// are themselves Rust source written as string literals.
    const DECLARATION_PREFIX: [&str; 8] = [
        "pub",
        "pub(crate)",
        "pub(super)",
        "const",
        "async",
        "unsafe",
        "extern",
        "\"C\"",
    ];

    /// Every `fn` one file declares, as (1-based line, name).
    ///
    /// A text scan, for the reason [`read_restatement`] gives: this crate
    /// cannot import the crates it reads, so it reads their source as text.
    ///
    /// Line-based and prefix-checked rather than a bare search for the token
    /// `fn`. A search found `in` in the doc comment "mentioning fn in prose"
    /// and `thing` in a quoted fixture, and every phantom it adds is a name
    /// that would let a citation of something that does not exist resolve.
    fn function_declarations(source: &str) -> Vec<(usize, &str)> {
        let mut out = Vec::new();

        for (offset, raw) in source.lines().enumerate() {
            let line = raw.trim();
            let mut at = 0usize;
            let bytes = line.as_bytes();
            while at + 2 <= bytes.len() {
                let boundary_before = at == 0 || !is_ident_byte(bytes[at - 1]);
                let boundary_after = match bytes.get(at + 2) {
                    None => true,
                    Some(byte) => !is_ident_byte(*byte),
                };
                if &bytes[at..at + 2] == b"fn" && boundary_before && boundary_after {
                    break;
                }
                at += 1;
            }
            if at + 2 > bytes.len() {
                continue;
            }
            if !line[..at]
                .split_whitespace()
                .all(|word| DECLARATION_PREFIX.contains(&word))
            {
                continue;
            }

            let mut cursor = at + 2;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            let start = cursor;
            while cursor < bytes.len() && is_ident_byte(bytes[cursor]) {
                cursor += 1;
            }
            // A declaration's name is followed by its parameters or its
            // generics. Anything else is a word that happened to follow `fn`.
            if cursor > start && matches!(bytes.get(cursor), Some(b'(') | Some(b'<')) {
                out.push((offset + 1, &line[start..cursor]));
            }
        }

        out
    }

    /// The names of the `#[test]` functions one file declares.
    ///
    /// A test function is the first `fn` declared after a line that is exactly
    /// `#[test]`, which is what lets the reader step over any attribute
    /// written between the two without knowing what those attributes are.
    ///
    /// Exactly, so that the `"#[test]\n"` of a quoted fixture is not read as
    /// an attribute of this file. A phantom test here would mask the deletion
    /// of a real one that happened to share its name.
    fn test_function_names(source: &str) -> Vec<&str> {
        let declarations = function_declarations(source);
        let mut names = Vec::new();

        for (offset, raw) in source.lines().enumerate() {
            if raw.trim() != "#[test]" {
                continue;
            }
            if let Some((_, name)) = declarations.iter().find(|(line, _)| *line > offset + 1) {
                names.push(*name);
            }
        }

        names
    }

    /// Every doc comment that names a test function names one that exists.
    ///
    /// No acceptance criterion in `spec/criteria/` covers this check, for the
    /// reason the note at the top of this module gives, so the name is
    /// descriptive and no criterion identifier is invented for it.
    ///
    /// Derived from AICD §39, "references are checked mechanically", and from
    /// AICD §14, which is why the failure paths are planted and proved rather
    /// than assumed. The references AICD §39 is written about are references to
    /// the methodology; this applies the same rule to a reference one file
    /// makes to another file's test, which is the form the defect took here.
    #[test]
    fn a_doc_comment_naming_a_test_names_a_test_that_exists() {
        let root = repo_root();
        let (files, _) = scan_repository();
        assert!(
            !files.is_empty(),
            "the scan visited no file under {}, so nothing below stands for anything",
            root.display()
        );

        let sources: Vec<&String> = files
            .iter()
            .filter(|file| file.ends_with(".rs"))
            .filter(|file| {
                DOC_SCAN_ROOTS
                    .iter()
                    .any(|(prefix, _)| file.starts_with(prefix))
            })
            .collect();

        // The floors. Every one of these is a way for this check to find
        // nothing and report a pass, which AICD §14 and AICD §39 both forbid,
        // so each is a failure with the reason named instead.
        for (prefix, why) in DOC_SCAN_ROOTS {
            assert!(
                sources.iter().any(|file| file.starts_with(prefix)),
                "no Rust source was found under {prefix}, which this check reads because it is \
                 {why}. Either the tree moved or the scan is broken, and in both cases the \
                 checks below would pass over the files they exist to read."
            );
        }

        let mut spans: Vec<DocSpan> = Vec::new();
        let mut tests: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut functions: BTreeSet<&str> = BTreeSet::new();
        let mut texts: Vec<String> = Vec::with_capacity(sources.len());

        for file in &sources {
            texts.push(
                fs::read_to_string(root.join(file))
                    .unwrap_or_else(|error| panic!("cannot read {file}: {error}")),
            );
        }
        for (file, text) in sources.iter().zip(texts.iter()) {
            spans.extend(doc_spans(text, file));
            for name in test_function_names(text) {
                tests.entry(name).or_default().insert(file.as_str());
            }
            functions.extend(
                function_declarations(text)
                    .into_iter()
                    .map(|(_, name)| name),
            );
        }

        assert!(
            !tests.is_empty(),
            "the reader found no `#[test]` function in {}, which means it is broken rather than \
             the workspace untested. Every check below resolves a cited name against this set, \
             so an empty one would pass everything.",
            count(sources.len(), "Rust file")
        );
        assert!(
            tests.contains_key("a_doc_comment_naming_a_test_names_a_test_that_exists"),
            "the reader did not find this test's own name among the {} it collected, so it is \
             not reading `#[test]` functions the way they are actually written and every \
             resolution below is worthless.",
            count(tests.len(), "test")
        );
        assert!(
            !spans.is_empty(),
            "the reader found no bare backticked span in any doc comment under {}, so the doc \
             comment parse is broken. A citation it cannot see is a citation it reports nothing \
             about.",
            root.display()
        );
        assert!(
            functions.contains("lf_line_endings"),
            "the reader did not find `lf_line_endings`, a function this very file declares, so \
             it is not reading declarations correctly."
        );

        let mut defects: Vec<String> = Vec::new();
        let registered: BTreeSet<(&str, &str)> = CITED_TESTS
            .iter()
            .map(|(citing, name, _, _)| (*citing, *name))
            .collect();

        // The register. A cited name that no test answers to, or that the
        // citing file no longer writes, is the drift this check exists for.
        for (citing, name, home, why) in CITED_TESTS {
            match tests.get(name) {
                None => defects.push(format!(
                    "{citing} names `{name}` in prose as a test of this repository, and no \
                     `#[test] fn {name}` exists anywhere under {}. If the test was renamed, the \
                     repair is three edits and not one: the test, the prose in {citing}, and \
                     this register. It is registered because {why}.",
                    DOC_SCAN_ROOTS.map(|(prefix, _)| prefix).join(" or ")
                )),
                Some(declared) if !declared.contains(home) => defects.push(format!(
                    "{citing} names `{name}` and says it is declared in {home}. It is declared \
                     in {} instead, so the prose sends a reader to the wrong file.",
                    at_most(
                        &declared
                            .iter()
                            .map(|f| (*f).to_string())
                            .collect::<Vec<_>>(),
                        3
                    )
                )),
                Some(_) => {}
            }
            if !spans
                .iter()
                .any(|span| span.file == citing && span.text == name)
            {
                defects.push(format!(
                    "`{name}` is registered as cited by {citing} and {citing} no longer writes \
                     it in a doc comment. Either the citation went away, in which case drop this \
                     entry, or it was reworded past what the reader recognises, in which case \
                     the reader is now blind to it."
                ));
            }
        }

        // The sweep. A citation nobody registered is a citation nobody checks,
        // and the register is hand-kept, which is the thing this repository
        // keeps finding to have gone quietly stale. ORI-T-0091 was briefed with
        // two of the three citations in `error.rs`; this is what would have
        // caught the third.
        for span in &spans {
            if tests.contains_key(span.text.as_str())
                && !registered.contains(&(span.file.as_str(), span.text.as_str()))
            {
                defects.push(format!(
                    "{}:{} names `{}` in a doc comment, which is a `#[test]` function, and \
                     CITED_TESTS does not register it. An unregistered citation is one nothing \
                     ties to the test, which is the defect this check removes.",
                    span.file, span.line, span.text
                ));
            }
        }

        // Inside a file already known to cite tests, every bare snake_case span
        // must be something. This is what catches a citation of a test that
        // never existed, which neither the register nor the sweep can see: the
        // register does not list it and the sweep cannot resolve it.
        let citing_files: BTreeSet<&str> = CITED_TESTS.iter().map(|(citing, ..)| *citing).collect();
        let exempt: BTreeSet<(&str, &str)> = NOT_AN_IDENTIFIER
            .iter()
            .map(|(file, text, _)| (*file, *text))
            .collect();
        for span in &spans {
            let file = span.file.as_str();
            let text = span.text.as_str();
            if !citing_files.contains(file)
                || registered.contains(&(file, text))
                || functions.contains(text)
                || exempt.contains(&(file, text))
            {
                continue;
            }
            defects.push(format!(
                "{}:{} names `{text}` in a doc comment. {file} is a file that cites tests, and \
                 `{text}` is neither a function declared anywhere in this workspace nor listed \
                 in NOT_AN_IDENTIFIER. Either it is a citation of something that does not exist, \
                 or it is prose that needs registering as prose.",
                span.file, span.line
            ));
        }

        // The exemptions, checked rather than trusted.
        for (file, text, why) in NOT_AN_IDENTIFIER {
            if !spans
                .iter()
                .any(|span| span.file == file && span.text == text)
            {
                defects.push(format!(
                    "`{text}` is exempt in {file} because it is {why}, and {file} no longer \
                     writes it. The exemption has outlived its reason and is a blind spot until \
                     it is removed."
                ));
            }
        }

        assert!(
            defects.is_empty(),
            "{} between the doc comments of this workspace and the tests they name:\n  {}",
            count(defects.len(), "disagreement"),
            defects.join("\n  ")
        );
    }

    // ---- the readers, on fixtures written for the purpose (AICD §14) ----

    /// A doc comment carrying one span of every kind the rule has to separate.
    const DOC_COMMENT_FIXTURE: &str = concat!(
        "//! A module. [`linked_name`] is rustdoc's to resolve, not ours.\n",
        "//!\n",
        "//! Bare `a_cited_test` is ours. So is `two_words`.\n",
        "//! Not ours: `std`, `id`, `Cow`, `SECTION_COUNT`, `spec/LLD.md`,\n",
        "//! `cargo fmt`, `core.autocrlf`, `self::thing`, `kebab-case`, `_lead`,\n",
        "//! `trail_`, `double__bar`.\n",
        "//!\n",
        "//! ```text\n",
        "//! `fenced_sample` is an illustration\n",
        "//! ```\n",
        "\n",
        "/// An item, citing `another_test`.\n",
        "pub fn thing() {}\n",
        "\n",
        "// a plain comment naming `not_a_doc_comment`\n",
    );

    #[test]
    fn the_doc_reader_takes_bare_identifiers_and_leaves_everything_else() {
        let spans = doc_spans(DOC_COMMENT_FIXTURE, "fixture");
        let found: Vec<&str> = spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(
            found,
            ["a_cited_test", "two_words", "another_test"],
            "the reader took a span it should have left, or left one it should have taken"
        );
    }

    #[test]
    fn the_doc_reader_reports_the_line_a_citation_is_written_on() {
        let spans = doc_spans(DOC_COMMENT_FIXTURE, "fixture");
        assert_eq!(spans[0].line, 3, "`a_cited_test` is on line 3");
        assert_eq!(spans[2].line, 12, "`another_test` is on line 12");
        assert_eq!(spans[0].file, "fixture", "the file is carried through");
    }

    #[test]
    fn the_function_reader_separates_test_functions_from_plain_ones() {
        let source = concat!(
            "fn plain() {}\n",
            "#[test]\n",
            "fn a_test() {}\n",
            "#[test]\n",
            "#[should_panic]\n",
            "fn a_test_with_another_attribute() {}\n",
            "/// A doc comment mentioning fn in prose.\n",
            "pub fn documented() {}\n",
            "const NOT_FN: &str = \"confn fnord\";\n",
            "    \"#[test]\\n\",\n",
            "    \"pub fn quoted_fixture() {}\\n\",\n",
        );
        assert_eq!(
            function_declarations(source)
                .into_iter()
                .map(|(_, name)| name)
                .collect::<Vec<_>>(),
            [
                "plain",
                "a_test",
                "a_test_with_another_attribute",
                "documented"
            ],
            "the reader took a `fn` from prose, from a quoted fixture, or from `confn fnord`"
        );
        assert_eq!(
            test_function_names(source),
            ["a_test", "a_test_with_another_attribute"],
            "an attribute between `#[test]` and the function hid it, or a quoted `#[test]` \
             counted as one"
        );
    }
}
