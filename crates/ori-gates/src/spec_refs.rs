//! The index of this repository's own specification and the reader that
//! resolves prose references to it: AICD §39.
//!
//! AICD §39 adopts one rule from the failure that earned it: "References are
//! checked mechanically." [`crate::sections`] applies that rule to the
//! methodology, whose references are spelled `AICD §<n>` and resolve against
//! `methodology/sections.json`. This module applies the same rule to the other
//! corpus a file in this repository cites, which is this repository's own
//! specification under `spec/`, and which until now was resolved by nobody.
//!
//! # Why a separate module and not more of [`crate::sections`]
//!
//! Three reasons, in descending order of weight.
//!
//! The corpus is different. [`crate::sections`] indexes one HTML document whose
//! numbers live in a `<span class="num">` and whose index is generated to a
//! committed JSON file. This indexes twenty-nine markdown documents whose
//! numbers live in the heading text and whose index is derived on every run. No
//! line of the parser is shared and no output is shared.
//!
//! The machinery that could have been shared is not reachable. The repository
//! walk, the citation reader and their helpers are `#[cfg(test)]` items private
//! to [`crate::sections`]'s test module. Widening them would be an edit to an
//! existing test module, which an agent may not make (`spec/CONVENTIONS.md`,
//! "Tests"). What is duplicated instead is the directory walk, about thirty
//! lines, and the sentence-count helper; everything else here is new. The two
//! walks also differ in contract: that one panics because it is a test fixture,
//! this one returns [`crate::spec_refs::SpecRefError`] because it is not.
//!
//! `crates/ori-gates/src/sections.rs` was claimed by another ticket in flight
//! when this one opened (claim 26 in `ops/lock-table.md`). A new file and one
//! `mod` line collide with nothing.
//!
//! # This module is not a gate
//!
//! `spec/CI_CD.md` section 1 lists fourteen gates and none of them is this. The
//! citation gate it lists, item 9, is specified as "every `AICD §n` resolves",
//! which is [`crate::sections`]'s subject and not this one, and it is not built:
//! the note at the head of that module is the fuller account and this one must
//! not contradict it. What runs here is one test under gate 2. It refuses no
//! build of its own, and no document may cite it as protection, because
//! `spec/LLD.md` section 2 puts "report a gate installed without a proof" in
//! this crate's must-not column.
//!
//! # The grammar, and how it was chosen
//!
//! The population was measured before the grammar was written, not guessed. In
//! the tree this module landed on, 551 references over 129 of its 311 files
//! match the grammar. 539 resolve, 7 are reported as not checked because what
//! would settle them is an item ordinal, and 5 do not resolve. The forms:
//!
//! 1. A path-qualified section reference: a path under `spec/` ending `.md`,
//!    then optional closing punctuation, then `section` and a number. 318
//!    occurrences, the dominant form, and the one a renumbering breaks in bulk.
//! 2. A bare-name section reference: the upper-case stem of a specification
//!    document, then `section` and a number or the section sign and a number.
//!    198 occurrences. `.gitignore`, `scripts/` and `ops/` prefer it.
//! 3. An anchor reference: a document name ending `.md`, then `#` and a
//!    fragment. 32 occurrences. This is the form `spec/CONVENTIONS.md` fixes
//!    for the commit trailer, `Spec: <document>#<section>`, which gate 13
//!    checks the shape of and explicitly does not resolve.
//!
//! A fourth thing is resolved: whether a path under `spec/` ending `.md` names
//! a document that exists, whether or not a section or anchor claim follows it.
//! 3 occurrences, all of them defects, and the rule was kept because of them.
//! A path that does name a document and claims no location inside it is not
//! collected: there is nothing about it this module could be wrong about.
//!
//! Deliberately outside the grammar, each because resolving it would cost more
//! false positives than the defects it would find:
//!
//! - Item ordinals. `CI_CD gate 7` and `spec/CI_CD.md` section 1 item 9 name an
//!   entry of a numbered list inside a section, not a heading. About forty
//!   occurrences. Which list an ordinal indexes is not recoverable from the
//!   reference, and a resolver that guessed would be wrong silently. Seven of
//!   them are written as a dotted number against a document that numbers no
//!   subsection; there this module resolves the section, reports the ordinal
//!   by name as not checked, and pretends nothing either way.
//! - Quoted heading titles, as in the form `<DOCUMENT> "<heading>"`. Eighteen
//!   occurrences, and the two that do not match exactly are a heading named by
//!   its first three words and a bullet mistaken for a heading. Neither is a
//!   fabrication, so an exact-match resolver would report two false positives
//!   out of eighteen, which is the rate that gets a checker deleted.
//! - Identifier references: `PRD <letter>-<nn>` and the acceptance criteria
//!   identifiers. 220 occurrences and, measured, all 220 resolve. They are a
//!   different index and a different claim, they have never carried a defect
//!   here, and each would need its own reader. Named as the first extension.
//! - References into `ops/` by line number. The form is fragile and this
//!   repository has been bitten by it, but a line number always exists, so
//!   what broke was the claim about the line and not the line. Nothing
//!   mechanical can check that.
//! - Whether a reference to a section that exists says something true about it.
//!   The first defect this ticket was briefed with was of that kind. It is not
//!   mechanically checkable and this module does not pretend otherwise.
//!
//! # This module is its own first customer
//!
//! The three forms are written into this doc comment on purpose, one of each,
//! and the check asserts it finds all three here. A reader that stops
//! recognising a form stops seeing its own example, which is a failure rather
//! than a quiet fall in the count. The three are: `spec/LLD.md` section 2, the
//! path form; CI_CD section 1, the bare-name form; and CONVENTIONS.md#git, the
//! anchor form, which is also the anchor every commit on this branch carries.
//!
//! Being its own customer cost one correction, on the first run that read the
//! whole tree. The doc comment two hundred lines below this one illustrated
//! the point that a bare file name and a full path reach the same document,
//! and it illustrated it with an anchor that reaches no heading. The check
//! reported it, correctly, against this file. It is a real anchor now.
//!
//! The same trap is why the record of unresolved references below spells each
//! one in parts and why every test fixture carrying a deliberately broken
//! reference is assembled at run time: this file is scanned like every other,
//! so a bad reference written whole here is a bad reference in the tree.
//!
//! # What it found
//!
//! Five of the 551 do not resolve, and each is written out in the record this
//! module carries beside the check. Two are an anchor that two `ops/` records
//! quote in the course of saying that it resolves against nothing; quoting a
//! defect is not committing one, and both are recorded as quotations. Three
//! are a credential-rotation runbook under `spec/runbooks/`
//! that two scripts send a human to in an error path and that is not in the
//! tree. That one is a live defect, it is outside this ticket's declared scope,
//! and the record says so.
//!
//! # Why the links here are written `crate::spec_refs::`
//!
//! `lib.rs` carries a `///` on this module's declaration, so rustdoc resolves
//! this file's links in the crate root's scope rather than this module's. The
//! head of `crates/ori-gates/src/sections.rs` records the same trap and why the
//! absolute form is the one that survives deleting that `///`. Do not shorten
//! these.
//!
//! Must not: add a dependency (`CLAUDE.md` absolute rule 6). Nothing outside
//! `std` is used.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// The directory under the repository root that holds the specification:
/// AICD §39.
///
/// AICD §39 requires references to specification to be checked mechanically,
/// which first requires knowing where the specification is. `spec/LLD.md`
/// section 1 fixes this directory as that place.
pub const SPECIFICATION_ROOT: &str = "spec";

/// One markdown heading, as the document writes it: AICD §39.
///
/// Derived from AICD §39's requirement that the index be of what the document
/// actually says, so the number and the slug are both read from the heading
/// text rather than assumed from its position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Heading {
    /// How many `#` opened it, 1 to 6.
    pub level: u8,
    /// The 1-based line it sits on.
    pub line: usize,
    /// The heading text with the `#` run and surrounding space removed.
    pub text: String,
    /// The number it opens with, if it opens with one: `2`, `4.10`.
    pub number: Option<String>,
    /// The fragment identifier a markdown renderer derives from the text.
    pub slug: String,
}

/// One specification document and every heading in it: AICD §39.
///
/// The unit a reference resolves against. AICD §39 names the failure this
/// guards as a reference that is "present but reporting nothing", so a document
/// that numbers no heading is a document against which every numbered reference
/// is false, and [`crate::spec_refs::Document::numbers_nothing`] says so
/// rather than leaving the caller to infer it from an empty list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    /// The path relative to the repository root, with `/` separators.
    pub path: String,
    /// Every heading, in document order.
    pub headings: Vec<Heading>,
}

impl Document {
    /// Every number this document gives a heading, in document order.
    pub fn numbers(&self) -> Vec<&str> {
        self.headings
            .iter()
            .filter_map(|heading| heading.number.as_deref())
            .collect()
    }

    /// Whether some heading of this document carries exactly `number`.
    pub fn has_number(&self, number: &str) -> bool {
        self.headings
            .iter()
            .any(|heading| heading.number.as_deref() == Some(number))
    }

    /// Whether some heading of this document slugs to `fragment`.
    pub fn has_slug(&self, fragment: &str) -> bool {
        self.headings.iter().any(|heading| heading.slug == fragment)
    }

    /// Whether this document numbers no heading at all.
    ///
    /// True of `spec/CONVENTIONS.md`, `spec/SECURITY_NOTES.md` and five others,
    /// whose headings are titles. Every numbered reference to one of them is
    /// unresolvable whatever the number, and this is what lets the message say
    /// that rather than print an empty list of alternatives.
    pub fn numbers_nothing(&self) -> bool {
        self.headings.iter().all(|heading| heading.number.is_none())
    }

    /// Whether this document numbers any subheading, as `4.10` rather than `4`.
    ///
    /// The one fact that separates a dotted reference naming a subsection from
    /// a dotted reference naming an item inside a section. `spec/PRD.md` and
    /// `spec/ARCHITECTURE.md` number subsections; `spec/CI_CD.md` does not, and
    /// the seven dotted references to it in `ops/` name gates in its list.
    pub fn numbers_subsections(&self) -> bool {
        self.headings
            .iter()
            .filter_map(|heading| heading.number.as_deref())
            .any(|number| number.contains('.'))
    }
}

/// Every specification document of this repository, indexed: AICD §39.
///
/// Built by [`crate::spec_refs::DocumentIndex::from_root`] on every run rather
/// than committed. AICD §39's rule is that references are checked against what
/// the documents say now; a committed index of markdown would be one more thing
/// that can go stale, and unlike the methodology's index there is no expensive
/// parse to amortise.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DocumentIndex {
    documents: BTreeMap<String, Document>,
    by_bare_name: BTreeMap<String, String>,
    by_file_name: BTreeMap<String, Vec<String>>,
}

impl DocumentIndex {
    /// Reads every markdown document under `root`'s specification directory.
    ///
    /// # Errors
    ///
    /// [`crate::spec_refs::SpecRefError::Io`] when a directory or document
    /// cannot be read, because a document skipped is a document whose
    /// references all appear to resolve. [`crate::spec_refs::SpecRefError::NoDocuments`]
    /// and [`crate::spec_refs::SpecRefError::NoNumberedHeadings`] when the
    /// result would be an index that resolves nothing or numbers nothing, which
    /// AICD §39 names as reporting nothing while looking correct.
    pub fn from_root(root: &Path) -> Result<Self, SpecRefError> {
        let specification = root.join(SPECIFICATION_ROOT);
        let mut documents = BTreeMap::new();
        let mut pending = vec![specification.clone()];

        while let Some(directory) = pending.pop() {
            let listing = fs::read_dir(&directory).map_err(|error| SpecRefError::Io {
                path: display_path(&directory),
                error,
            })?;
            for entry in listing {
                let entry = entry.map_err(|error| SpecRefError::Io {
                    path: display_path(&directory),
                    error,
                })?;
                let path = entry.path();
                let kind = entry.file_type().map_err(|error| SpecRefError::Io {
                    path: display_path(&path),
                    error,
                })?;
                if kind.is_dir() {
                    pending.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let text = fs::read_to_string(&path).map_err(|error| SpecRefError::Io {
                    path: display_path(&path),
                    error,
                })?;
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let document = Document {
                    headings: headings_of(&text),
                    path: relative.clone(),
                };
                documents.insert(relative, document);
            }
        }

        if documents.is_empty() {
            return Err(SpecRefError::NoDocuments {
                root: display_path(&specification),
            });
        }
        if documents.values().all(Document::numbers_nothing) {
            return Err(SpecRefError::NoNumberedHeadings {
                root: display_path(&specification),
                documents: documents.len(),
            });
        }

        let mut by_bare_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut by_file_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for path in documents.keys() {
            let file_name = path.rsplit('/').next().unwrap_or(path).to_owned();
            let stem = file_name.trim_end_matches(".md").to_owned();
            by_file_name
                .entry(file_name)
                .or_default()
                .push(path.clone());
            if is_bare_document_name(&stem) {
                by_bare_name.entry(stem).or_default().push(path.clone());
            }
        }

        Ok(Self {
            documents,
            // A name two documents answer to resolves nothing: a reference that
            // spells it is ambiguous rather than wrong, and reporting it as
            // wrong would be a fabrication of this module's own.
            by_bare_name: by_bare_name
                .into_iter()
                .filter(|(_, paths)| paths.len() == 1)
                .filter_map(|(name, paths)| paths.into_iter().next().map(|path| (name, path)))
                .collect(),
            by_file_name,
        })
    }

    /// How many documents the index holds.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Whether the index holds no document. Never true of a built index.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Every document path, in sorted order.
    pub fn paths(&self) -> Vec<&str> {
        self.documents.keys().map(String::as_str).collect()
    }

    /// Every bare name a reference may spell, in sorted order.
    pub fn bare_names(&self) -> Vec<&str> {
        self.by_bare_name.keys().map(String::as_str).collect()
    }

    /// The document at `path`, if the index holds one.
    pub fn document(&self, path: &str) -> Option<&Document> {
        self.documents.get(path)
    }

    /// The path a reference's document token names, if it names exactly one.
    ///
    /// A full path is taken as written. A bare file name is taken when exactly
    /// one document answers to it, which is how TESTING.md#3-thresholds and
    /// `spec/TESTING.md` section 3 reach the same heading of the same document,
    /// and how `README.md`, which two documents answer to, reaches neither.
    pub fn path_of_token(&self, token: &str) -> Option<&str> {
        if let Some((path, _)) = self.documents.get_key_value(token) {
            return Some(path.as_str());
        }
        let file_name = token.rsplit('/').next().unwrap_or(token);
        match self.by_file_name.get(file_name) {
            Some(paths) if paths.len() == 1 => paths.first().map(String::as_str),
            _ => None,
        }
    }

    /// Every reference to the specification in one file's bytes: AICD §39.
    ///
    /// Bytes rather than text, for the reason [`crate::sections`] gives: a file
    /// that is not valid UTF-8 would otherwise be skipped, and a skipped file
    /// reads exactly like a file with nothing wrong in it. `.gitignore`,
    /// `Cargo.toml`, `prove.sh` and `ci.yml` all carry references here, so no
    /// extension filter is applied either.
    pub fn references_in(&self, bytes: &[u8], file: &str) -> Vec<Reference> {
        let mut found = Vec::new();
        let mut at = 0usize;
        let mut line = 1usize;
        let mut counted_to = 0usize;

        while at < bytes.len() {
            let parsed = self
                .bare_name_reference(bytes, at)
                .or_else(|| self.file_name_reference(bytes, at));
            let Some((start, document, target, end)) = parsed else {
                at += 1;
                continue;
            };
            // The file-name form is recognised at its `.md` and starts before
            // it, so the span reported is the whole reference and the line is
            // the line it opens on.
            let start = start.max(counted_to);
            line += bytes[counted_to..start]
                .iter()
                .filter(|b| **b == b'\n')
                .count();
            counted_to = start;
            found.push(Reference {
                file: file.to_owned(),
                line,
                written_as: String::from_utf8_lossy(&bytes[start..end]).into_owned(),
                document,
                target,
            });
            at = end;
        }
        found
    }

    /// What the index makes of one reference: AICD §39.
    ///
    /// The three verdicts are the three honest answers. Resolved. Unresolved,
    /// with what the document actually carries, because a message that says
    /// only "does not resolve" leaves the reader to do the work again. And not
    /// checked, for the part of a reference this module deliberately does not
    /// resolve, because a checker that silently counts an unchecked claim as a
    /// pass is the failure AICD §39 is written about.
    pub fn resolve(&self, reference: &Reference) -> Resolution {
        let Some(document) = self.documents.get(&reference.document) else {
            return Resolution::Unresolved {
                reason: format!(
                    "{} is not a document of this repository's specification; {}",
                    reference.document,
                    self.nearest(&reference.document)
                ),
            };
        };

        match &reference.target {
            // The reader produces this only for a document that is not there,
            // which the arm above has already answered. Reached only by a
            // reference a caller built by hand, where the whole claim is that
            // the document exists and the lookup has just shown that it does.
            Target::Existence => Resolution::Resolved,
            Target::Anchor(fragment) => {
                if document.has_slug(fragment) {
                    return Resolution::Resolved;
                }
                // `spec/CONVENTIONS.md` fixes the trailer as
                // `Spec: <document>#<section>`, and four places in the tree read
                // that literally and write the section number after the `#`.
                // Tried second, so a heading that slugs to a bare number wins.
                if fragment.chars().all(|c| c.is_ascii_digit()) && document.has_number(fragment) {
                    return Resolution::Resolved;
                }
                Resolution::Unresolved {
                    reason: format!(
                        "no heading of {} slugs to that fragment; it has {}",
                        document.path,
                        joined(
                            &document
                                .headings
                                .iter()
                                .map(|h| h.slug.clone())
                                .collect::<Vec<_>>(),
                            6
                        )
                    ),
                }
            }
            Target::Section(number) => {
                if document.numbers_nothing() {
                    return Resolution::Unresolved {
                        reason: format!(
                            "{} numbers no heading at all, so no section number resolves in it; \
                             its headings are {}",
                            document.path,
                            joined(
                                &document
                                    .headings
                                    .iter()
                                    .filter(|h| h.level == 2)
                                    .map(|h| h.text.clone())
                                    .collect::<Vec<_>>(),
                                6
                            )
                        ),
                    };
                }
                if number.contains('.') && !document.numbers_subsections() {
                    let parent = number.split('.').next().unwrap_or(number).to_owned();
                    if document.has_number(&parent) {
                        return Resolution::ItemOrdinal { section: parent };
                    }
                    return Resolution::Unresolved {
                        reason: format!(
                            "{} numbers no subsection, so that reads as section {parent} item \
                             {}, and it has no section {parent}; it has sections {}",
                            document.path,
                            number.split_once('.').map_or("", |(_, item)| item),
                            joined(
                                &document
                                    .numbers()
                                    .iter()
                                    .map(|n| (*n).to_owned())
                                    .collect::<Vec<_>>(),
                                12
                            )
                        ),
                    };
                }
                if document.has_number(number) {
                    return Resolution::Resolved;
                }
                Resolution::Unresolved {
                    reason: format!(
                        "{} has no section {number}; it has sections {}",
                        document.path,
                        joined(
                            &document
                                .numbers()
                                .iter()
                                .map(|n| (*n).to_owned())
                                .collect::<Vec<_>>(),
                            12
                        )
                    ),
                }
            }
        }
    }

    /// A sentence naming the documents whose file name is closest to `token`.
    fn nearest(&self, token: &str) -> String {
        let file_name = token.rsplit('/').next().unwrap_or(token);
        match self.by_file_name.get(file_name) {
            Some(paths) if paths.len() > 1 => format!(
                "that file name is carried by {}, so it names none of them",
                joined(paths, 4)
            ),
            _ => format!(
                "the specification holds {}",
                joined(&self.documents.keys().cloned().collect::<Vec<_>>(), 8)
            ),
        }
    }

    /// A bare document name and the claim after it, at `at`.
    fn bare_name_reference(
        &self,
        bytes: &[u8],
        at: usize,
    ) -> Option<(usize, String, Target, usize)> {
        if !bytes.get(at)?.is_ascii_uppercase() {
            return None;
        }
        if at > 0 && is_token_byte(bytes[at - 1]) {
            return None;
        }
        let mut end = at;
        while end < bytes.len() && matches!(bytes[end], b'A'..=b'Z' | b'0'..=b'9' | b'_') {
            end += 1;
        }
        // A name glued to a lower-case word, a digit or a dot is part of
        // something longer: `TESTINGs`, `LLD.md`, `CI_CD_RUNNER`.
        if matches!(bytes.get(end), Some(byte) if byte.is_ascii_alphanumeric() || *byte == b'.') {
            return None;
        }
        let name = std::str::from_utf8(bytes.get(at..end)?).ok()?;
        let document = self.by_bare_name.get(name)?.clone();
        let (number, end) = section_claim(bytes, end)?;
        Some((at, document, Target::Section(number), end))
    }

    /// A document file name and the claim after it, ending at `at`'s `.md`.
    fn file_name_reference(
        &self,
        bytes: &[u8],
        at: usize,
    ) -> Option<(usize, String, Target, usize)> {
        if bytes.get(at..at + 3)? != b".md" {
            return None;
        }
        let after = at + 3;
        // `.mdx`, `.md.bak`: a longer extension is a different file.
        if matches!(bytes.get(after), Some(byte) if byte.is_ascii_alphanumeric() || *byte == b'.') {
            return None;
        }
        let mut start = at;
        while start > 0 && is_token_byte(bytes[start - 1]) {
            start -= 1;
        }
        let token = std::str::from_utf8(bytes.get(start..after)?).ok()?;
        let Some(path) = self.path_of_token(token) else {
            // A path into the specification that names no document is a claim
            // this module answers. Anything else ending `.md` is a reference to
            // some other part of the tree and is not this module's subject.
            if token.starts_with("spec/") {
                let (target, end) = existence_or_claim(bytes, after);
                return Some((start, token.to_owned(), target, end));
            }
            return None;
        };
        let path = path.to_owned();

        if bytes.get(after) == Some(&b'#') {
            let mut end = after + 1;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'-' | b'_'))
            {
                end += 1;
            }
            if end == after + 1 {
                return None;
            }
            let fragment = std::str::from_utf8(bytes.get(after + 1..end)?)
                .ok()?
                .to_owned();
            return Some((start, path, Target::Anchor(fragment), end));
        }

        // A document that exists, named with no location, claims only that it
        // exists, and it does. Not collected: hundreds of mentions carry no
        // claim this module can be wrong about, and counting them would bury
        // the ones that do.
        let (number, end) = section_claim(bytes, after)?;
        Some((start, path, Target::Section(number), end))
    }

    /// File names that more than one document answers to: AICD §39.
    ///
    /// A known blind spot, reported rather than hidden. A reference spelling
    /// one of these names no document, so this module collects nothing for it
    /// and resolves nothing. There were no such references when this was
    /// written, and the inventory prints the list so that stays visible.
    pub fn ambiguous_file_names(&self) -> Vec<&str> {
        self.by_file_name
            .iter()
            .filter(|(_, paths)| paths.len() > 1)
            .map(|(name, _)| name.as_str())
            .collect()
    }
}

/// What part of a document a reference names: AICD §39.
///
/// The three granularities the repository actually writes, measured before this
/// was written rather than assumed.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Target {
    /// A numbered heading: `section 2`, `§4.10`.
    Section(String),
    /// A fragment identifier: the part after `#`.
    Anchor(String),
    /// The document itself, with no location named inside it.
    ///
    /// Produced only for a path under `spec/` that names no document, where the
    /// claim that fails is that the document is there at all. A path that does
    /// name a document and claims no location inside it is not collected: there
    /// is nothing about it to be wrong.
    Existence,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Section(number) => write!(f, "section {number}"),
            Self::Anchor(fragment) => write!(f, "#{fragment}"),
            Self::Existence => write!(f, "the document itself"),
        }
    }
}

/// One reference to the specification as some file writes it: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Reference {
    /// The file that writes it, relative to the repository root.
    pub file: String,
    /// The 1-based line it sits on.
    pub line: usize,
    /// The reference exactly as written, for the failure message.
    pub written_as: String,
    /// The document path it names, resolved from whatever token it spelled.
    pub document: String,
    /// The location inside that document it names.
    pub target: Target,
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} {}", self.file, self.line, self.written_as)
    }
}

/// What the index makes of one reference: AICD §39.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolution {
    /// The document exists and carries the location named.
    Resolved,
    /// The section resolves and the ordinal after it is an item in a list,
    /// which this module does not resolve and does not count as resolved.
    ItemOrdinal {
        /// The section number that did resolve.
        section: String,
    },
    /// The reference names something the specification does not carry.
    Unresolved {
        /// What is wrong, and what the document actually has.
        reason: String,
    },
}

/// Why an index of the specification could not be built: AICD §39.
///
/// Every variant refuses to publish an index that would resolve some references
/// and quietly pass the rest, which is AICD §39's "present but reporting
/// nothing". [`crate::spec_refs::SpecRefError::methodology_ref`] names the
/// section each refusal rests on.
///
/// Written by hand rather than with `thiserror` for the reason the same
/// decision in `crates/ori-gates/src/sections.rs` records at length under the
/// lead's ruling R20 (`ops/rulings.md`): the first external crate this project
/// takes arrives with the dependency audit that watches it. When it lands, each
/// arm of the [`fmt::Display`] match becomes one attribute on the variant it
/// prints, the `error` field of [`crate::spec_refs::SpecRefError::Io`] takes
/// `#[source]`, and both hand-written implementations are deleted with no
/// message and no call site changed.
#[derive(Debug)]
pub enum SpecRefError {
    /// A directory or document could not be read.
    Io {
        /// What could not be read.
        path: String,
        /// Why not.
        error: io::Error,
    },
    /// The specification directory held no markdown document.
    NoDocuments {
        /// Where it looked.
        root: String,
    },
    /// Documents were read and not one heading in any of them carried a number.
    NoNumberedHeadings {
        /// Where it looked.
        root: String,
        /// How many documents it read.
        documents: usize,
    },
}

impl SpecRefError {
    /// The methodology section this refusal rests on: AICD §39.
    ///
    /// `spec/CONVENTIONS.md` requires every refusal to carry one.
    pub fn methodology_ref(&self) -> &'static str {
        match self {
            // A document that cannot be read is a document whose references are
            // all unchecked, and AICD §14 forbids a check that reports a pass
            // over what it could not read.
            Self::Io { .. } => "AICD §14",
            Self::NoDocuments { .. } | Self::NoNumberedHeadings { .. } => "AICD §39",
        }
    }
}

impl fmt::Display for SpecRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(
                f,
                "cannot read {path}: {error}. A document skipped is a document whose references \
                 all appear to resolve"
            ),
            Self::NoDocuments { root } => write!(
                f,
                "no markdown document under {root}, so an index built here would resolve nothing \
                 and every reference in the repository would pass"
            ),
            Self::NoNumberedHeadings { root, documents } => write!(
                f,
                "{documents} document(s) under {root} and not one numbered heading among them, \
                 so the heading reader is broken rather than the specification unnumbered"
            ),
        }
    }
}

impl Error for SpecRefError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::NoDocuments { .. } | Self::NoNumberedHeadings { .. } => None,
        }
    }
}

/// The fragment identifier a markdown renderer derives from a heading:
/// AICD §39.
///
/// Lower-cased, everything that is not alphanumeric or a space or a hyphen or
/// an underscore dropped, spaces folded to hyphens. That is the rule the
/// renderer this repository's anchors were written against applies, and it is
/// what makes `## 4.8 Git and release (AICD §13)` reachable as
/// `#48-git-and-release-aicd-13`.
pub fn heading_slug(heading: &str) -> String {
    let mut out = String::with_capacity(heading.len());
    let mut pending_space = false;
    for character in heading.chars() {
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if !(character.is_alphanumeric() || character == '-' || character == '_') {
            continue;
        }
        if pending_space {
            out.push('-');
            pending_space = false;
        }
        out.extend(character.to_lowercase());
    }
    out
}

/// Every ATX heading of one markdown document, in order.
fn headings_of(text: &str) -> Vec<Heading> {
    let mut found = Vec::new();
    let mut fenced = false;

    for (offset, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
        if hashes == 0 || hashes > 6 {
            continue;
        }
        let rest = &line[hashes..];
        if !rest.starts_with(' ') {
            continue;
        }
        let heading_text = rest.trim();
        if heading_text.is_empty() {
            continue;
        }
        found.push(Heading {
            level: hashes as u8,
            line: offset + 1,
            number: leading_number(heading_text),
            slug: heading_slug(heading_text),
            text: heading_text.to_owned(),
        });
    }

    found
}

/// The number a heading opens with, if it opens with one followed by a space.
///
/// `2. Entities` gives `2`; `4.10 Continuous test environment` gives `4.10`;
/// `Rust` and `5.1` alone give nothing, the second because a number with no
/// title is not a heading this index can describe.
fn leading_number(heading: &str) -> Option<String> {
    let mut end = 0usize;
    let bytes = heading.as_bytes();
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    let mut number = &heading[..end];
    if number.ends_with('.') {
        number = &number[..number.len() - 1];
    }
    if number.is_empty() || number.contains("..") || number.starts_with('.') {
        return None;
    }
    let rest = &heading[end..];
    if !rest.starts_with(' ') || rest.trim().is_empty() {
        return None;
    }
    Some(number.to_owned())
}

/// `, section 4` or `` ` §4.2 `` at `at`, as the number and the offset past it.
///
/// The closing punctuation run is bounded at four because a reference is
/// written inside at most a backtick, a quote and a bracket; letting it run
/// would let a whole clause sit between a document and a number that does not
/// belong to it.
fn section_claim(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    const SIGN: &[u8] = "\u{a7}".as_bytes();
    let mut cursor = at;
    let mut punctuation = 0usize;
    while punctuation < 4
        && matches!(
            bytes.get(cursor),
            Some(b'`' | b'"' | b'\'' | b')' | b']' | b',')
        )
    {
        cursor += 1;
        punctuation += 1;
    }
    let before_space = cursor;
    while matches!(bytes.get(cursor), Some(b' ')) {
        cursor += 1;
    }
    let spaced = cursor > before_space;

    let rest = bytes.get(cursor..)?;
    if rest.starts_with(b"section ") || rest.starts_with(b"Section ") {
        if !spaced && punctuation == 0 {
            return None;
        }
        cursor += "section ".len();
        while matches!(bytes.get(cursor), Some(b' ')) {
            cursor += 1;
        }
    } else if rest.starts_with(SIGN) {
        cursor += SIGN.len();
        if matches!(bytes.get(cursor), Some(b' ')) {
            cursor += 1;
        }
    } else {
        return None;
    }

    let (number, end) = dotted_number(bytes, cursor)?;
    Some((number, end))
}

/// A `.md` path under `spec/` naming no document, with whatever claim follows.
///
/// The claim is read so the message can repeat it, but the verdict is already
/// settled: a document that is not there carries no section and no anchor.
fn existence_or_claim(bytes: &[u8], after: usize) -> (Target, usize) {
    if bytes.get(after) == Some(&b'#') {
        let mut end = after + 1;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'-' | b'_'))
        {
            end += 1;
        }
        if end > after + 1 {
            let fragment = String::from_utf8_lossy(&bytes[after + 1..end]).into_owned();
            return (Target::Anchor(fragment), end);
        }
    }
    match section_claim(bytes, after) {
        Some((number, end)) => (Target::Section(number), end),
        None => (Target::Existence, after),
    }
}

/// `4` or `4.10` at `at`, as the number and the offset just past it.
fn dotted_number(bytes: &[u8], at: usize) -> Option<(String, usize)> {
    let mut cursor = at;
    while matches!(bytes.get(cursor), Some(byte) if byte.is_ascii_digit()) {
        cursor += 1;
    }
    if cursor == at {
        return None;
    }
    // A dot is part of the number only when a digit follows it, so the full
    // stop ending "section 1." is left where it is.
    while let Some(b'.') = bytes.get(cursor) {
        let mut ahead = cursor + 1;
        while matches!(bytes.get(ahead), Some(byte) if byte.is_ascii_digit()) {
            ahead += 1;
        }
        if ahead == cursor + 1 {
            break;
        }
        cursor = ahead;
    }
    Some((
        String::from_utf8_lossy(&bytes[at..cursor]).into_owned(),
        cursor,
    ))
}

/// Whether `byte` may sit inside a path or a name without ending it.
fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/')
}

/// Whether a document stem is the upper-case name references spell bare.
///
/// `LLD`, `CI_CD`, `PROJECT_BRIEF`. Three characters at least, because two
/// upper-case letters are as likely to be a word in prose as a document.
fn is_bare_document_name(stem: &str) -> bool {
    stem.len() >= 3
        && stem.starts_with(|c: char| c.is_ascii_uppercase())
        && stem
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// `a, b and 4 more`, so a message names examples without printing a corpus.
fn joined(items: &[String], cap: usize) -> String {
    if items.is_empty() {
        return "nothing".to_owned();
    }
    if items.len() <= cap {
        return items.join(", ");
    }
    format!("{} and {} more", items[..cap].join(", "), items.len() - cap)
}

/// A path as a message should print it.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    // These tests carry no criterion identifier, for the reason the same note
    // at the head of crates/ori-gates/src/sections.rs sets out: no acceptance
    // criterion in spec/criteria/ covers reference integrity in this
    // repository's own documents, and inventing an identifier would put a
    // fabricated reference in the coverage matrix, which is the defect class
    // this module exists to remove. The names are descriptive and the gap is
    // reported instead.
    //
    // Nothing in this module's doc comments backticks a test name. The check
    // ORI-T-0091 built in sections.rs sweeps every doc comment under crates/
    // and apps/ for a backticked span that is the name of a #[test] function
    // and fails on any it does not have registered, so a doc comment here that
    // named one of these tests would fail that check and not this one. Tests
    // are named in plain comments like this one instead.

    /// The repository root, two levels above `crates/ori-gates`.
    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("a crate lives two levels below the repository root")
            .to_path_buf()
    }

    fn index() -> DocumentIndex {
        DocumentIndex::from_root(&repository_root()).expect("the specification indexes")
    }

    // -----------------------------------------------------------------------
    // The scan.
    //
    //   cargo test -p ori-gates spec_refs -- --nocapture
    //
    // prints the inventory: references by form, by area, what resolved, what
    // was not checked and why, and every unresolved reference with its line.
    // That printed block is what goes in the pull request report.
    // -----------------------------------------------------------------------

    /// Directory names the scan does not descend into, each with the reason it
    /// is not repository content.
    ///
    /// Nothing else is excluded and no extension filter is applied. A scan
    /// narrowed to markdown and Rust would pass over `.gitignore`,
    /// `rust-toolchain.toml`, `ci.yml` and every `prove.sh`, which between them
    /// carry more than a hundred references, and would report a clean sweep of
    /// a repository it had barely read.
    ///
    /// `target/` matters more here than it looks: a documentation build writes
    /// this module's own doc comments into HTML under it, so a scan that
    /// descended would read every reference twice and attribute the second copy
    /// to a generated file.
    const SCAN_EXCLUSIONS: [(&str, &str); 2] = [
        (".git", "object database, not repository content"),
        ("target", "build output, not repository content"),
    ];

    /// The two files the scan reads and whose references it does not judge,
    /// each with a marker that must still be in it and the reason it is out.
    ///
    /// Neither is an exemption for a format or a directory. Together they cost
    /// this check 26 references, 25 in the prototype and 1 in the methodology,
    /// of which 14 would not resolve. Both files are named in the inventory on
    /// every run, so the size of the blind spot is never a guess.
    ///
    /// The design prototype is a mock. Its references are sample rows in a
    /// JavaScript array and they name the documents of a fictional product
    /// called Ledgerline alongside files like `engine/src/retry.rs` that this
    /// repository does not have. One of them is a picture of a citation gate
    /// refusing a bad reference, so there the bad reference is the point.
    /// Judging any of them would report thirteen defects that are not there,
    /// against seven documents of a product that does not exist. Its
    /// marker is the fictional product's name: if that leaves the file, the
    /// reason for the exclusion has gone with it.
    ///
    /// The methodology is upstream. It is the one document in this tree that
    /// this repository does not author, and the `spec/` path in it is the
    /// layout the method prescribes to every AICD product rather than a claim
    /// about this one. Its reason is structural rather than textual, so its
    /// marker is empty and the tie asserted instead is that the path is the one
    /// [`crate::sections::SOURCE_PATH`] names. A text marker would fail every
    /// time AICD §29 revised a sentence, which is a false alarm rather than a
    /// check.
    const EXCLUDED_FILES: [(&str, &str, &str); 2] = [
        (
            "spec/design/Ori Studio.html",
            "Ledgerline",
            "a design prototype whose references are sample data about a fictional product",
        ),
        (
            "methodology/AICD_Methodology_v0.3.html",
            "",
            "the upstream method document, which this repository does not author",
        ),
    ];

    /// Every reference in this repository that does not resolve, as of this
    /// ticket, with what is to be done about each.
    ///
    /// Spelled in parts, as (the file that writes it, the directory, the file
    /// name, the target, why). Joining them at run time is not decoration: this
    /// module is scanned like every other file, so writing one of these
    /// references whole here would plant it in the tree and this check would
    /// report its own record as a defect. It caught exactly that on the first
    /// run.
    ///
    /// Each entry must still be written where it says it is. A record that has
    /// outlived its reason is a blind spot, so a repair landing fails this test
    /// with a message saying to delete the entry, which is a one-line change in
    /// the same commit as the repair.
    const RECORDED_UNRESOLVED: [(&str, &str, &str, &str, &str); 5] = [
        (
            "ops/lock-table.md",
            "spec/",
            "TESTING.md",
            "#1-test-types",
            "a quotation. The sentence that carries it says the lead put this anchor in a ticket \
             and that it resolves against nothing. Recording a defect is not committing one, and \
             ops/ is an append-only record that must keep saying what was said",
        ),
        (
            "ops/calibration/CR-006-the-lead-reasons-from-branches.md",
            "spec/",
            "TESTING.md",
            "#1-test-types",
            "the same quotation, in a calibration table whose columns are what was asserted and \
             what was true. The anchor is the subject of the row, not a claim the row makes",
        ),
        (
            "scripts/secret-scan.sh",
            "spec/runbooks/",
            "rotate-credentials.md",
            "",
            "A LIVE DEFECT, not a quotation. The error a human is shown when the scanner finds a \
             secret sends them to this runbook first, and the runbook is not in the tree; \
             spec/runbooks/ holds three others. Outside this ticket's declared scope, which is \
             crates/ori-gates/ only, so it is recorded here and reported rather than repaired",
        ),
        (
            "scripts/gates.sh",
            "spec/runbooks/",
            "rotate-credentials.md",
            "",
            "the same missing runbook, cited twice in gate 7's blocked-verdict path, where it is \
             the first instruction a human gets. Same scope bar",
        ),
        (
            "scripts/gates.sh",
            "spec/runbooks/",
            "rotate-credentials.md",
            "",
            "the second of the two citations in that file",
        ),
    ];

    /// Facts about the index that the index must keep agreeing with.
    ///
    /// The floor under the heading reader. Every check below resolves a
    /// reference against numbers this reader derived, so a reader that derived
    /// none, or derived them from the wrong place, would pass everything.
    /// Counting them here is what fails instead.
    ///
    /// The zero is the load-bearing one. `spec/CONVENTIONS.md` numbers no
    /// heading, which is what makes a numbered reference to it false whatever
    /// the number, and a reader that started inventing numbers would show up
    /// here first.
    const INDEX_RESTATEMENT: [(&str, usize); 6] = [
        ("spec/CI_CD.md", 6),
        ("spec/LLD.md", 6),
        ("spec/TESTING.md", 5),
        ("spec/PRD.md", 28),
        ("spec/ARCHITECTURE.md", 13),
        ("spec/CONVENTIONS.md", 0),
    ];

    /// Areas that carry references today and must go on carrying them.
    ///
    /// An area falling to zero means the scan stopped reaching it, and a scan
    /// that reaches nothing reports a clean tree. Each is well under what the
    /// area actually carries, so ordinary editing does not trip it.
    const AREA_FLOORS: [(&str, usize); 6] = [
        ("crates/", 80),
        ("apps/", 50),
        ("ops/", 80),
        ("scripts/", 35),
        ("fixtures/", 50),
        ("spec/", 20),
    ];

    /// The fewest references a run may find before the reader is the suspect.
    ///
    /// 551 were found on the tree this landed on, over 130 of its 311 files.
    /// A reader that has broken finds nearly none, not two thirds, so a floor
    /// here fires on the failure it is for and not on ordinary editing.
    const MINIMUM_REFERENCES: usize = 350;

    /// One file's bytes, and the file, as the scan collects them.
    struct Scanned {
        file: String,
        bytes: Vec<u8>,
    }

    /// Every file under the repository root, read whole.
    ///
    /// Panics rather than skipping: a file the scan cannot read must not become
    /// a file the scan silently passes over, because the result is a clean
    /// report of an unchecked repository.
    fn scan_repository() -> Vec<Scanned> {
        let root = repository_root();
        let mut files = Vec::new();
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
                files.push(Scanned {
                    file: relative,
                    bytes,
                });
            }
        }

        files.sort_by(|a, b| a.file.cmp(&b.file));
        files
    }

    fn plural(many: usize, noun: &str) -> String {
        if many == 1 {
            format!("1 {noun}")
        } else {
            format!("{many} {noun}s")
        }
    }

    /// The inventory, printed on every run so the answer is never only a pass.
    fn inventory(
        files: usize,
        references: &[Reference],
        unresolved: &[(&Reference, String)],
        not_checked: &BTreeMap<&'static str, usize>,
    ) -> String {
        let mut report = String::new();
        let mut by_form: BTreeMap<&str, usize> = BTreeMap::new();
        let mut by_area: BTreeMap<&str, (BTreeSet<&str>, usize)> = BTreeMap::new();
        let mut documents: BTreeMap<&str, usize> = BTreeMap::new();

        for reference in references {
            let form = match &reference.target {
                Target::Section(_) if reference.written_as.contains(".md") => "path and section",
                Target::Section(_) => "bare name and section",
                Target::Anchor(_) => "anchor",
                Target::Existence => "document, no location named",
            };
            *by_form.entry(form).or_default() += 1;
            let area = match reference.file.split_once('/') {
                Some((directory, _)) => directory,
                None => "(repository root)",
            };
            let entry = by_area.entry(area).or_default();
            entry.0.insert(reference.file.as_str());
            entry.1 += 1;
            *documents.entry(reference.document.as_str()).or_default() += 1;
        }

        report.push_str(&format!(
            "specification references: {} over {} files, of {} files scanned\n",
            references.len(),
            by_area
                .values()
                .map(|(area_files, _)| area_files.len())
                .sum::<usize>(),
            files
        ));
        for (form, count) in &by_form {
            report.push_str(&format!("  form {form}: {count}\n"));
        }
        report.push_str("by area:\n");
        for (area, (area_files, occurrences)) in &by_area {
            report.push_str(&format!(
                "  {area}: {}, {}\n",
                plural(area_files.len(), "file"),
                plural(*occurrences, "occurrence")
            ));
        }
        report.push_str("most cited documents:\n");
        let mut ranked: Vec<(&str, usize)> = documents.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        for (path, count) in ranked.iter().take(6) {
            report.push_str(&format!("  {path}: {count}\n"));
        }
        report.push_str(&format!(
            "excluded directories: {}\n",
            SCAN_EXCLUSIONS
                .iter()
                .map(|(name, why)| format!("{name}/ ({why})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        report.push_str("excluded files, read and not judged:\n");
        for (path, _, why) in EXCLUDED_FILES {
            report.push_str(&format!("  {path}: {why}\n"));
        }
        report.push_str("not checked, by design:\n");
        for (what, count) in not_checked {
            report.push_str(&format!("  {what}: {count}\n"));
        }
        report.push_str(&format!("unresolved: {}\n", unresolved.len()));
        for (reference, reason) in unresolved {
            report.push_str(&format!("  {reference}\n      {reason}\n"));
        }
        report
    }

    // The check itself. A reference to this repository's own specification
    // resolves, or it is one of the five this ticket found and recorded.
    //
    // Derived from AICD §39, "references are checked mechanically", whose rule
    // is written about references to the methodology and is applied here to the
    // other corpus this repository cites. Derived from AICD §14 for the way the
    // failure paths are proved rather than assumed: each is planted in a copy
    // of the tree, run, and the exit status and message recorded.
    #[test]
    fn every_reference_to_this_repositorys_specification_resolves() {
        let index = index();
        let root = repository_root();

        // The floors, first, because every one of them is a way for this check
        // to find nothing and report a pass. AICD §14 and AICD §39 both forbid
        // that, so each is a failure naming its own reason instead.
        for (path, numbered) in INDEX_RESTATEMENT {
            let document = index
                .document(path)
                .unwrap_or_else(|| panic!("{path} is not in the index of {}", root.display()));
            assert_eq!(
                document.numbers().len(),
                numbered,
                "{path} numbers {} heading(s) and this module was written against {numbered}. \
                 Either the document was renumbered, in which case every reference to it below \
                 is being resolved against a different set than the one this record describes, \
                 or the heading reader is broken. Its headings are {}",
                document.numbers().len(),
                joined(
                    &document
                        .headings
                        .iter()
                        .map(|h| h.text.clone())
                        .collect::<Vec<_>>(),
                    8
                )
            );
        }

        let files = scan_repository();
        assert!(
            !files.is_empty(),
            "the scan visited no file under {}",
            root.display()
        );
        // The exclusions, checked rather than trusted. An excluded file that is
        // no longer there, or no longer carries the evidence its reason rests
        // on, is a blind spot dressed as a decision.
        assert_eq!(
            EXCLUDED_FILES[1].0,
            crate::sections::SOURCE_PATH,
            "the upstream methodology is excluded because it is upstream, and this is the \
             constant that says which file that is. They have drifted apart, so the exclusion \
             now names some other file"
        );
        for (path, marker, why) in EXCLUDED_FILES {
            let Some(excluded) = files.iter().find(|f| f.file == path) else {
                panic!(
                    "{path} is excluded because it is {why} and it was not among the {} files \
                     scanned, so the exclusion stands for nothing and its reason cannot be \
                     checked",
                    files.len()
                );
            };
            if marker.is_empty() {
                continue;
            }
            assert!(
                String::from_utf8_lossy(&excluded.bytes).contains(marker),
                "{path} no longer contains {marker:?}, which is the evidence for not judging \
                 its references: it is {why}. The exclusion has outlived its evidence and is a \
                 blind spot until somebody reads the file again"
            );
        }

        let mut references: Vec<Reference> = Vec::new();
        for scanned in &files {
            if EXCLUDED_FILES
                .iter()
                .any(|(path, _, _)| *path == scanned.file)
            {
                continue;
            }
            references.extend(index.references_in(&scanned.bytes, &scanned.file));
        }
        references.sort();

        assert!(
            references.len() >= MINIMUM_REFERENCES,
            "the scan read {} and found {}, under the floor of {MINIMUM_REFERENCES}. A reader \
             that finds nothing reports a clean repository, so this is a failure of the reader \
             until somebody shows otherwise",
            plural(files.len(), "file"),
            plural(references.len(), "reference")
        );

        // This file writes a reference in each of the three forms in its own
        // doc comments, so a reader that stopped recognising a form stops
        // seeing it here. The self-referential floor ORI-T-0091 built, applied
        // to a grammar with three shapes instead of one.
        let own = "crates/ori-gates/src/spec_refs.rs";
        let mine: Vec<&Reference> = references.iter().filter(|r| r.file == own).collect();
        for (form, found) in [
            (
                "a path and a section number",
                mine.iter().any(|r| {
                    matches!(r.target, Target::Section(_)) && r.written_as.contains(".md")
                }),
            ),
            (
                "a bare document name and a section number",
                mine.iter().any(|r| {
                    matches!(r.target, Target::Section(_)) && !r.written_as.contains(".md")
                }),
            ),
            (
                "an anchor",
                mine.iter().any(|r| matches!(r.target, Target::Anchor(_))),
            ),
        ] {
            assert!(
                found,
                "the reader did not find {form} in {own}, a file whose own doc comments write \
                 one deliberately so that this cannot pass while the reader is blind to that \
                 form. It found {} reference(s) in this file.",
                mine.len()
            );
        }

        let mut by_area: BTreeMap<&str, usize> = BTreeMap::new();
        for reference in &references {
            if let Some((area, _)) = reference.file.split_once('/') {
                *by_area.entry(area).or_default() += 1;
            }
        }
        for (prefix, floor) in AREA_FLOORS {
            let area = prefix.trim_end_matches('/');
            let found = by_area.get(area).copied().unwrap_or(0);
            assert!(
                found >= floor,
                "{prefix} carries {found} reference(s) and this check was written against at \
                 least {floor}. Either the tree moved or the scan stopped descending, and in \
                 both cases the references there are now checked by nobody"
            );
        }

        let mut unresolved: Vec<(&Reference, String)> = Vec::new();
        let mut not_checked: BTreeMap<&'static str, usize> = BTreeMap::new();
        not_checked.insert("item ordinal inside a section", 0);
        for reference in &references {
            match index.resolve(reference) {
                Resolution::Resolved => {}
                Resolution::ItemOrdinal { .. } => {
                    *not_checked
                        .entry("item ordinal inside a section")
                        .or_default() += 1;
                }
                Resolution::Unresolved { reason } => unresolved.push((reference, reason)),
            }
        }

        let report = inventory(files.len(), &references, &unresolved, &not_checked);
        println!("{report}");

        // The record, checked rather than trusted, in both directions.
        let recorded: BTreeSet<(String, String, String)> = RECORDED_UNRESOLVED
            .iter()
            .map(|(file, directory, name, target, _)| {
                (
                    (*file).to_owned(),
                    format!("{directory}{name}"),
                    (*target).to_owned(),
                )
            })
            .collect();

        let mut defects: Vec<String> = Vec::new();
        for (reference, reason) in &unresolved {
            let target = match &reference.target {
                Target::Anchor(fragment) => format!("#{fragment}"),
                Target::Section(number) => format!("section {number}"),
                Target::Existence => String::new(),
            };
            if recorded.contains(&(reference.file.clone(), reference.document.clone(), target)) {
                continue;
            }
            defects.push(format!("{reference}\n      {reason}"));
        }

        for (file, directory, name, target, why) in RECORDED_UNRESOLVED {
            let document = format!("{directory}{name}");
            let still_there = unresolved.iter().any(|(reference, _)| {
                reference.file == file
                    && reference.document == document
                    && match &reference.target {
                        Target::Anchor(fragment) => format!("#{fragment}") == target,
                        Target::Section(number) => format!("section {number}") == target,
                        Target::Existence => target.is_empty(),
                    }
            });
            if !still_there {
                defects.push(format!(
                    "{file} is recorded as writing an unresolvable reference to {document}, and \
                     it no longer does. Either the repair landed, in which case delete this \
                     entry of RECORDED_UNRESOLVED in the same commit, or the reader stopped \
                     recognising the form, in which case it is now blind to it. The record says \
                     this one is {why}"
                ));
            }
        }

        assert!(
            defects.is_empty(),
            "{} in this repository that the specification does not carry:\n  {}\n\n{report}",
            plural(defects.len(), "reference"),
            defects.join("\n  ")
        );
    }

    // ---- the readers, on fixtures written for the purpose (AICD §14) ----
    //
    // Every fixture that carries an unresolvable reference is assembled from
    // parts at run time rather than written as a literal. The check above reads
    // this file like every other, so a literal bad reference here would be a
    // bad reference in the tree, and the check would report it. That is not a
    // hypothetical: it is what happened on the first run of this module.

    fn planted_path(document: &str, number: &str) -> String {
        format!("see spec/{document}.md section {number} for this")
    }

    fn planted_anchor(document: &str, fragment: &str) -> String {
        format!("Spec: {document}.md#{fragment}")
    }

    #[test]
    fn a_heading_slug_follows_the_renderer_the_anchors_were_written_against() {
        for (heading, expected) in [
            ("Git", "git"),
            (
                "1. Levels and what each proves",
                "1-levels-and-what-each-proves",
            ),
            (
                "4.8 Git and release (AICD \u{a7}13)",
                "48-git-and-release-aicd-13",
            ),
            (
                "2. Crate responsibilities and dependency direction",
                "2-crate-responsibilities-and-dependency-direction",
            ),
            (
                "Tier 2 modules (changes require two approvals)",
                "tier-2-modules-changes-require-two-approvals",
            ),
            ("  spaced  out  ", "spaced-out"),
            ("`code` in a heading", "code-in-a-heading"),
        ] {
            assert_eq!(heading_slug(heading), expected, "slug of {heading:?}");
        }
    }

    #[test]
    fn a_heading_number_is_read_only_when_a_title_follows_it() {
        let headings = headings_of(concat!(
            "# TESTING: Ori Studio\n",
            "## 1. Levels and what each proves\n",
            "## Git\n",
            "### 4.10 Continuous test environment\n",
            "## 5.1\n",
            "##NoSpace\n",
            "```\n",
            "## 9. Fenced, and not a heading\n",
            "```\n",
            "####### too deep\n",
        ));
        let numbered: Vec<Option<&str>> = headings
            .iter()
            .map(|heading| heading.number.as_deref())
            .collect();
        assert_eq!(
            numbered,
            [None, Some("1"), None, Some("4.10"), None],
            "the reader took a number from a fenced block, from a bare number with no title, \
             from a heading with no space after the hashes, or from a seven-hash line"
        );
        assert_eq!(headings[1].line, 2, "the line a heading opens on");
        assert_eq!(headings[3].level, 3, "the level it opens at");
    }

    #[test]
    fn the_reader_takes_the_three_forms_and_leaves_everything_else() {
        let index = index();
        let source = concat!(
            "a path form: `spec/LLD.md` section 2, and spec/TESTING.md section 5.\n",
            "a bare form: LLD section 2, and CI_CD \u{a7}1, and DATA_MODEL \u{a7}3.\n",
            "an anchor form: CONVENTIONS.md#git and spec/PRD.md#6-screen-inventory.\n",
            "left alone: LLDsection 2, ops/phase-1-backlog.md section 1, section 4 on its own,\n",
            "AICD \u{a7}39, PRD G-02, CI_CD gate 7, README.md#anything, a bare spec/LLD.md,\n",
            "LLD, the word TESTING in a sentence, and CI_CDX \u{a7}1.\n",
        );
        let found = index.references_in(source.as_bytes(), "fixture");
        let written: Vec<&str> = found.iter().map(|r| r.written_as.as_str()).collect();
        assert_eq!(
            written,
            [
                "spec/LLD.md` section 2",
                "spec/TESTING.md section 5",
                "LLD section 2",
                "CI_CD \u{a7}1",
                "DATA_MODEL \u{a7}3",
                "CONVENTIONS.md#git",
                "spec/PRD.md#6-screen-inventory",
            ],
            "the reader took a form it should have left, or left one it should have taken"
        );
        assert_eq!(found[0].line, 1, "the first is on line 1");
        assert_eq!(found[2].line, 2, "the bare form is on line 2");
        assert_eq!(found[5].line, 3, "the anchor is on line 3");
        for reference in &found {
            assert_eq!(
                index.resolve(reference),
                Resolution::Resolved,
                "{reference} is a correct reference and must resolve"
            );
        }
    }

    #[test]
    fn the_reader_reports_the_document_a_token_names_however_it_is_spelled() {
        let index = index();
        for spelling in [
            "spec/TESTING.md section 5",
            "TESTING.md#5-ori-studio-tests-itself",
            "TESTING section 5",
        ] {
            let found = index.references_in(spelling.as_bytes(), "fixture");
            assert_eq!(found.len(), 1, "one reference in {spelling:?}");
            assert_eq!(
                found[0].document, "spec/TESTING.md",
                "{spelling:?} names the document"
            );
            assert_eq!(
                index.resolve(&found[0]),
                Resolution::Resolved,
                "{spelling:?} resolves"
            );
        }
    }

    #[test]
    fn a_section_the_document_does_not_have_is_refused_with_what_it_does_have() {
        let index = index();
        let source = planted_path("LLD", "9");
        let found = index.references_in(source.as_bytes(), "fixture");
        assert_eq!(found.len(), 1, "one reference in {source:?}");
        let Resolution::Unresolved { reason } = index.resolve(&found[0]) else {
            panic!("a section that is not there resolved: {source}");
        };
        assert!(
            reason.contains("spec/LLD.md") && reason.contains("no section 9"),
            "the message does not name the document and the number: {reason}"
        );
        assert!(
            reason.contains("1, 2, 3, 4, 5, 6"),
            "the message does not say what the document actually has: {reason}"
        );
    }

    #[test]
    fn a_numbered_reference_to_an_unnumbered_document_is_refused() {
        let index = index();
        let source = planted_path("CONVENTIONS", "2");
        let found = index.references_in(source.as_bytes(), "fixture");
        let Resolution::Unresolved { reason } = index.resolve(&found[0]) else {
            panic!("a section number resolved against a document that numbers none");
        };
        assert!(
            reason.contains("numbers no heading at all") && reason.contains("Rust"),
            "the message should say the document numbers nothing and name its headings: {reason}"
        );
    }

    #[test]
    fn a_document_that_does_not_exist_is_refused() {
        let index = index();
        // With a section claimed, and with none, because the live defect this
        // rule found is a runbook cited in an error message with no section.
        for source in [
            planted_path("NOT_A_DOCUMENT", "1"),
            // Assembled, not written whole, for the reason at the head of this
            // block: a literal here is a defect planted in the real tree.
            format!("rotate first (spec/runbooks/{}.md), then repair", "absent"),
        ] {
            let found = index.references_in(source.as_bytes(), "fixture");
            assert_eq!(found.len(), 1, "the reference is collected, not skipped");
            let Resolution::Unresolved { reason } = index.resolve(&found[0]) else {
                panic!("a reference to a document that is not there resolved: {source}");
            };
            assert!(
                reason.contains("is not a document of this repository's specification"),
                "the message does not say the document is missing: {reason}"
            );
        }
    }

    #[test]
    fn a_document_outside_the_specification_is_not_this_modules_subject() {
        let index = index();
        for source in [
            "ops/phase-1-backlog.md section 3",
            "methodology/sections.md section 1",
            "README.md#anything",
            "apps/desktop/ui/src/screens/README.md section 2",
            "a bare mention of spec/LLD.md with no location claimed",
        ] {
            assert!(
                index.references_in(source.as_bytes(), "fixture").is_empty(),
                "{source:?} is not a claim about a location in this repository's specification \
                 and must not be collected"
            );
        }
        assert!(
            index.ambiguous_file_names().contains(&"README.md"),
            "two documents answer to README.md, so it must be reported as ambiguous"
        );
    }

    #[test]
    fn an_anchor_that_slugs_to_no_heading_is_refused_and_a_section_number_is_not() {
        let index = index();
        let bad = planted_anchor("TESTING", "1-test-types");
        let found = index.references_in(bad.as_bytes(), "fixture");
        let Resolution::Unresolved { reason } = index.resolve(&found[0]) else {
            panic!("an anchor that slugs to no heading resolved");
        };
        assert!(
            reason.contains("1-levels-and-what-each-proves"),
            "the message does not say what the document's anchors are: {reason}"
        );

        // `spec/CONVENTIONS.md` fixes the trailer as `<document>#<section>`, and
        // the pull request workflow reads that literally.
        let numeric = planted_anchor("CI_CD", "1");
        let found = index.references_in(numeric.as_bytes(), "fixture");
        assert_eq!(
            index.resolve(&found[0]),
            Resolution::Resolved,
            "a section number after the hash is the form spec/CONVENTIONS.md fixes"
        );
    }

    #[test]
    fn a_dotted_number_is_a_subsection_or_an_item_depending_on_the_document() {
        let index = index();

        // spec/ARCHITECTURE.md numbers subsections, so a dotted number there is
        // a heading claim and is resolved as one.
        let real = planted_path("ARCHITECTURE", "5.2");
        let found = index.references_in(real.as_bytes(), "fixture");
        assert_eq!(found[0].target, Target::Section("5.2".to_owned()));
        assert_eq!(index.resolve(&found[0]), Resolution::Resolved);

        let missing = planted_path("ARCHITECTURE", "7.1");
        let found = index.references_in(missing.as_bytes(), "fixture");
        assert!(
            matches!(index.resolve(&found[0]), Resolution::Unresolved { .. }),
            "a subsection that document does not have must not resolve"
        );

        // spec/CI_CD.md numbers none, so the ordinal is an item in its list and
        // is reported as not checked rather than as resolved or as a defect.
        let item = planted_path("CI_CD", "1.13");
        let found = index.references_in(item.as_bytes(), "fixture");
        assert_eq!(
            index.resolve(&found[0]),
            Resolution::ItemOrdinal {
                section: "1".to_owned()
            },
            "an item ordinal must be reported as not checked, never as resolved"
        );

        // And the same shape against a section that is not there is a defect,
        // not an unchecked ordinal.
        let absent = planted_path("CI_CD", "9.1");
        let found = index.references_in(absent.as_bytes(), "fixture");
        assert!(
            matches!(index.resolve(&found[0]), Resolution::Unresolved { .. }),
            "an item ordinal inside a section that does not exist is still a defect"
        );
    }

    #[test]
    fn an_index_built_where_there_is_no_specification_is_refused() {
        let error = DocumentIndex::from_root(Path::new("/nonexistent-root-for-this-test"))
            .expect_err("an index built over nothing must refuse");
        assert!(
            matches!(error, SpecRefError::Io { .. }),
            "reading a directory that is not there is an IO refusal: {error}"
        );
        assert_eq!(
            error.methodology_ref(),
            "AICD \u{a7}14",
            "every refusal names the section it rests on"
        );
        assert!(
            error.source().is_some(),
            "the IO refusal carries the error it wraps"
        );
    }

    #[test]
    fn the_reader_reads_bytes_so_a_file_that_is_not_utf_8_is_still_checked() {
        let index = index();
        let mut bytes = vec![0xff, 0xfe, 0x00];
        bytes.extend_from_slice(planted_path("LLD", "9").as_bytes());
        bytes.push(0x80);
        let found = index.references_in(&bytes, "fixture");
        assert_eq!(found.len(), 1, "a reference in undecodable bytes is found");
        assert!(
            matches!(index.resolve(&found[0]), Resolution::Unresolved { .. }),
            "and is judged"
        );
    }
}
