//! The coverage matrix, gate 4 of `spec/CI_CD.md` section 1: AICD §14.
//!
//! AICD §14 puts human verification one level above the tests: "the human
//! verification must happen one level up: on the mapping between what the
//! specification demands and what the tests prove". The mechanical half of
//! that is one sentence of the same section, and it is what this module is:
//! "CI enforces the mapping: every criterion identifier must appear in at
//! least one test, and every test must map to a criterion, or it is flagged."
//!
//! `spec/TESTING.md` section 2 is this product's restatement of it. Criteria
//! live in `spec/criteria/` as tables of identifiers; tests live under
//! `crates/` and `apps/` and embed the identifiers they cover in their names.
//! This module reads both corpora and reports the mapping between them.
//!
//! # The asymmetry, and which document settles it
//!
//! The two sides are not treated alike, and the difference is load bearing.
//!
//! An uncovered criterion FAILS the gate. An unmapped test is LISTED and does
//! not fail it.
//!
//! Criterion ORI-P1-011, the criterion this module is built against, is the
//! authority: "Gate fails and lists the criterion; a test naming no criterion
//! is listed as unmapped". AICD §14 agrees ("or it is flagged"), `spec/PRD.md`
//! V-03 agrees ("unmapped tests flagged"), and `spec/CONVENTIONS.md` under
//! "Tests" agrees ("A test without a criterion is flagged by the coverage
//! matrix gate").
//!
//! One sentence disagrees. `spec/TESTING.md` section 2 says the gate "fails if
//! a criterion has no test or a test names no criterion", which would refuse a
//! build over the unmapped side too. That sentence is the outlier against a
//! criterion, the methodology and two other documents, and following it here
//! would refuse every build of this repository today: on the tree this module
//! landed on, most tests legitimately cover no criterion and two modules carry
//! a recorded reason why. The disagreement is not resolved in code. It is one
//! sentence in one document, it is reported for a specification PR, and until
//! that PR lands this module follows the criterion.
//!
//! # What fails, what is listed
//!
//! | Finding | Verdict |
//! |---|---|
//! | A criterion no test names | fails |
//! | A test naming an identifier no criterion carries | fails |
//! | A test naming no criterion | listed |
//!
//! The middle row is not in ORI-P1-011's text and is derived rather than
//! quoted. A test named after `ORI-P1-999` claims coverage of something that
//! does not exist, so it would enter the matrix as a mapping to nothing; AICD
//! §39 adopted "references are checked mechanically" from exactly this failure
//! shape, and [`crate::sections`] and [`crate::spec_refs`] apply the same rule
//! to the two other corpora this repository cites. A matrix that resolved
//! names against nothing would be the "present but reporting nothing" defect
//! class AICD §39 names.
//!
//! # The floors
//!
//! A coverage gate has two ways to report a clean tree without looking at one,
//! and both are quiet. Read no criteria and every criterion is covered,
//! vacuously. Read no tests and, with no criteria either, nothing is
//! uncovered. Each is refused rather than reported as a pass. These five are
//! errors and not verdicts, and a caller that treats an error as anything but
//! a failed gate has reintroduced the defect:
//!
//! - [`crate::coverage::CoverageError::NoCriteria`]
//! - [`crate::coverage::CoverageError::NoCriteriaFiles`]
//! - [`crate::coverage::CoverageError::EmptyCriteriaFile`]
//! - [`crate::coverage::CoverageError::NoTests`]
//! - [`crate::coverage::CoverageError::NoSourceUnderRoot`]
//!
//! So are the three ways a test can go unseen: an attribute the reader does
//! not recognise, one written where the reader would have to guess at the
//! function it belongs to, and one that names no function at all. A test the
//! reader cannot see is a test the matrix loses in silence.
//!
//! - [`crate::coverage::CoverageError::UnknownTestAttribute`]
//! - [`crate::coverage::CoverageError::AttributeNotAlone`]
//! - [`crate::coverage::CoverageError::AttributeWithoutFunction`]
//!
//! # What this module is not
//!
//! It is not an installed gate. AICD §14: "a gate is installed only when it
//! has been seen to fail", and `spec/LLD.md` section 2 puts "report a gate
//! installed without a proof" in this crate's must-not column. Three things
//! are missing and none of them is in this ticket's declared scope: the
//! `GateDef` and `Runner` types (ORI-T-0041), the wiring in `scripts/gates.sh`
//! (which probes for a runner at a different path than the one the backlog
//! gives this ticket, so it will go on reporting gate 4 as having no local
//! runner until someone reconciles the two names), and the proof record under
//! `ops/gates/`. What runs here is a set of tests under gate 2. This module
//! refuses no build, and no document may cite it as protection.
//!
//! # Reading it on this repository
//!
//! `cargo test -p ori-gates coverage -- --nocapture` prints the current
//! matrix, the uncovered list and the unmapped list. The printed block is what
//! goes in a pull request report. No count is frozen in this comment: a
//! numeral here is read by nothing and goes stale silently, which is a lesson
//! this crate has already paid for once.
//!
//! # Duplication, and why
//!
//! The directory walk and the function declaration reader here are close
//! copies of `#[cfg(test)]` items private to [`crate::sections`]'s test
//! module. They cannot be imported, and widening them would be an edit to an
//! existing test module, which `spec/CONVENTIONS.md` under "Tests" forbids an
//! agent. [`crate::spec_refs`] made the same call for the same reason and
//! records it at its head. The two walks also differ in contract: that one
//! panics because it is a test fixture, these return
//! [`crate::coverage::CoverageError`] because they are not.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Where the accepted acceptance criteria live, relative to the product root.
///
/// AICD §14: the criteria are written before the build and are the input to
/// the mapping. `spec/TESTING.md` section 3 puts them in `criteria/` per
/// phase.
pub const CRITERIA_DIR: &str = "spec/criteria";

/// The trees that hold this product's tests, each with why it is read.
///
/// No methodology section names a directory; this is `spec/LLD.md` section 1's
/// layout. Both are read because a test under `apps/` is a test.
pub const TEST_ROOTS: [(&str, &str); 2] = [
    (
        "crates",
        "the workspace's own crates, where every unit test sits",
    ),
    ("apps", "the desktop shell, which is workspace source too"),
];

/// Directory names the scan does not descend into, each with the reason it is
/// not product source.
///
/// No methodology section applies; this is a fact about the working copy.
pub const SCAN_EXCLUSIONS: [(&str, &str); 2] = [
    (".git", "object database, not repository content"),
    ("target", "build output, not repository content"),
];

/// The attribute spellings that make a function a test, each with the reason
/// the reader knows it.
///
/// AICD §14: a test the reader cannot see is a test the matrix loses, so the
/// set is a register and anything shaped like a test attribute that is not in
/// it is [`CoverageError::UnknownTestAttribute`] rather than a silent skip.
pub const TEST_ATTRIBUTES: [(&str, &str); 2] = [
    ("#[test]", "the standard library's test attribute"),
    (
        "#[tokio::test]",
        "the async form, which `spec/CONVENTIONS.md` under \"Rust\" makes this \
         workspace's only async runtime",
    ),
];

/// The words Rust allows between the start of a declaration line and `fn`.
///
/// No methodology section applies. A copy of the list [`crate::sections`] uses,
/// for the reason the head of this module gives. Admitting a line as a
/// declaration only when everything before `fn` is drawn from this set is what
/// keeps the reader out of prose and out of quoted source fixtures, and this
/// file is full of the second kind.
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

/// The line below which a criteria file's rows are proposals, not criteria.
///
/// `spec/TESTING.md` section 2: "Proposed criteria (from the QA agent) do not
/// count until accepted." `spec/criteria/phase-1.md` writes the boundary as a
/// sentence beginning with these words.
const PROPOSED_MARKER: &str = "proposed criteria";

/// One accepted acceptance criterion, as its file spells it.
///
/// AICD §14: "Each criterion has an identifier, a precondition, an action, an
/// expected observable result, and a type". Only the identifier and where it
/// is written matter to the mapping, so only those are carried here.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Criterion {
    /// The identifier as written, `ORI-P1-011`.
    pub id: String,
    /// The phase part of the identifier, `P1`.
    pub phase: String,
    /// The file it is written in, relative to the product root, `/` separated.
    pub file: String,
    /// The 1-based line of its table row.
    pub line: usize,
}

/// One test function, and the criteria its name names.
///
/// AICD §14: "every criterion identifier must appear in at least one test".
/// The name is the whole of what the mapping reads, so the body is never
/// parsed.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TestFn {
    /// The function name.
    pub name: String,
    /// The file it is declared in, relative to the product root, `/`
    /// separated.
    pub file: String,
    /// The 1-based line of its attribute.
    pub line: usize,
    /// The attribute spelling that made it a test, from [`TEST_ATTRIBUTES`].
    pub attribute: String,
    /// Every criterion identifier its name embeds, in the order it embeds
    /// them, deduplicated. Empty means unmapped.
    pub covers: Vec<String>,
}

/// What one line of the matrix reports.
///
/// AICD §14 and ORI-P1-011 between them fix three, and which of the three
/// refuses a build is [`FindingKind::fails`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FindingKind {
    /// A criterion no test names. ORI-P1-011: the gate fails and lists it.
    UncoveredCriterion,
    /// A test naming an identifier no criteria file carries.
    DanglingReference,
    /// A test naming no criterion. ORI-P1-011: listed, not failed.
    UnmappedTest,
}

impl FindingKind {
    /// Whether a finding of this kind refuses the build.
    ///
    /// ORI-P1-011 fixes two of the three answers and the head of this module
    /// derives the third.
    #[must_use]
    pub fn fails(self) -> bool {
        match self {
            Self::UncoveredCriterion | Self::DanglingReference => true,
            Self::UnmappedTest => false,
        }
    }

    /// The methodology section the finding rests on (`CLAUDE.md` absolute rule
    /// 9).
    ///
    /// Returned as text rather than as `ori_core::MethodologyRef` for the
    /// reason `SectionsError` in [`crate::sections`] gives: `ori-gates` does
    /// not depend on `ori-core`, and adding the edge is outside this ticket's
    /// declared scope.
    #[must_use]
    pub fn methodology_ref(self) -> &'static str {
        match self {
            // The mapping rule itself, and the verification AICD §14 puts one
            // level above the tests.
            Self::UncoveredCriterion | Self::UnmappedTest => "AICD §14",
            // A reference that resolves to nothing, checked mechanically.
            Self::DanglingReference => "AICD §39",
        }
    }

    /// A short label for the rendered matrix.
    ///
    /// No methodology section applies; this is presentation.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::UncoveredCriterion => "uncovered criterion",
            Self::DanglingReference => "dangling reference",
            Self::UnmappedTest => "unmapped test",
        }
    }
}

/// One line of the matrix: what was found, where, and what it means.
///
/// AICD §14: the evidence a human reads at the pull request is the matrix and
/// the agent's list of what is not covered, so every finding names its subject
/// and its place rather than only counting.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Finding {
    /// Which of the three this is.
    pub kind: FindingKind,
    /// The criterion identifier or the test name the finding is about.
    pub subject: String,
    /// `file:line` of the subject, relative to the product root.
    pub location: String,
    /// One sentence of what it means, naming the section it rests on.
    pub detail: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({}) {}",
            self.kind.label(),
            self.subject,
            self.location,
            self.detail
        )
    }
}

/// The gate's answer.
///
/// AICD §14: a gate reports one state and a state that is not a pass is a
/// refusal. There is no third value; a run that could not read its input
/// returns [`CoverageError`] instead and is a failure in the caller's hands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Verdict {
    /// Every criterion is named by at least one test and every name resolves.
    Passed,
    /// At least one criterion is named by no test, or at least one test names
    /// an identifier no criterion carries.
    Failed,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passed => f.write_str("PASSED"),
            Self::Failed => f.write_str("FAILED"),
        }
    }
}

/// The criteria, the tests, and the mapping between them.
///
/// AICD §14: this is the artifact the verification lead reads at the pull
/// request, which is why it is a value that can be rendered and asserted on
/// rather than a function that prints.
#[derive(Clone, Debug)]
pub struct CoverageMatrix {
    criteria: Vec<Criterion>,
    tests: Vec<TestFn>,
    proposed: Vec<String>,
}

impl CoverageMatrix {
    /// The matrix of one product tree.
    ///
    /// AICD §14. Reads every `.md` file under [`CRITERIA_DIR`] and every `.rs`
    /// file under [`TEST_ROOTS`], and applies the floors at the head of this
    /// module.
    ///
    /// # Errors
    ///
    /// Every variant of [`CoverageError`]. All of them are gate failures in
    /// the caller's hands: none of them is a pass.
    pub fn read(root: &Path) -> Result<Self, CoverageError> {
        let mut criteria = Vec::new();
        let mut proposed = Vec::new();
        let mut tests = Vec::new();

        let criteria_dir = root.join(CRITERIA_DIR);
        let mut criteria_files: Vec<PathBuf> = read_dir_sorted(&criteria_dir)?
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        criteria_files.sort();
        if criteria_files.is_empty() {
            return Err(CoverageError::NoCriteriaFiles {
                directory: display_path(&criteria_dir),
            });
        }
        for path in &criteria_files {
            let relative = relative_to(root, path);
            let text = read_text(path, &relative)?;
            let (accepted, proposals) = criteria_in(&text, &relative)?;
            if accepted.is_empty() {
                return Err(CoverageError::EmptyCriteriaFile { file: relative });
            }
            criteria.extend(accepted);
            proposed.extend(proposals);
        }

        for (name, why) in TEST_ROOTS {
            let root_path = root.join(name);
            let sources: Vec<PathBuf> = walk(&root_path)?
                .into_iter()
                .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
                .collect();
            if sources.is_empty() {
                return Err(CoverageError::NoSourceUnderRoot {
                    root: display_path(&root_path),
                    why: why.to_string(),
                });
            }
            for path in &sources {
                let relative = relative_to(root, path);
                let text = read_text(path, &relative)?;
                tests.extend(tests_in(&text, &relative)?);
            }
        }

        Self::new(criteria, tests, proposed)
    }

    /// The matrix of criteria and tests already read.
    ///
    /// AICD §14. The floors live here rather than in [`CoverageMatrix::read`]
    /// so that a caller assembling a matrix from anywhere, a fixture included,
    /// cannot assemble an empty one and call it covered.
    ///
    /// # Errors
    ///
    /// [`CoverageError::NoCriteria`], [`CoverageError::NoTests`] and
    /// [`CoverageError::DuplicateCriterion`].
    pub fn new(
        criteria: Vec<Criterion>,
        tests: Vec<TestFn>,
        proposed: Vec<String>,
    ) -> Result<Self, CoverageError> {
        if criteria.is_empty() {
            return Err(CoverageError::NoCriteria);
        }
        if tests.is_empty() {
            return Err(CoverageError::NoTests);
        }
        let mut seen: BTreeMap<&str, &Criterion> = BTreeMap::new();
        for criterion in &criteria {
            if let Some(first) = seen.get(criterion.id.as_str()) {
                return Err(CoverageError::DuplicateCriterion {
                    id: criterion.id.clone(),
                    first: format!("{}:{}", first.file, first.line),
                    second: format!("{}:{}", criterion.file, criterion.line),
                });
            }
            seen.insert(criterion.id.as_str(), criterion);
        }
        for id in &proposed {
            if let Some(first) = seen.get(id.as_str()) {
                return Err(CoverageError::DuplicateCriterion {
                    id: id.clone(),
                    first: format!("{}:{}", first.file, first.line),
                    second: "the same file, below its proposed-criteria line".to_string(),
                });
            }
        }

        let mut criteria = criteria;
        criteria.sort();
        let mut tests = tests;
        tests.sort();
        let mut proposed = proposed;
        proposed.sort();
        proposed.dedup();
        Ok(Self {
            criteria,
            tests,
            proposed,
        })
    }

    /// Every accepted criterion, sorted by identifier.
    ///
    /// No methodology section applies; this is an accessor.
    #[must_use]
    pub fn criteria(&self) -> &[Criterion] {
        &self.criteria
    }

    /// Every test the reader found, sorted by name then place.
    ///
    /// No methodology section applies; this is an accessor.
    #[must_use]
    pub fn tests(&self) -> &[TestFn] {
        &self.tests
    }

    /// Identifiers found below a criteria file's proposed-criteria line.
    ///
    /// `spec/TESTING.md` section 2: proposals do not count until accepted.
    /// They are carried so that the matrix can say it saw them rather than
    /// leaving a reader to wonder whether the reader stopped early.
    #[must_use]
    pub fn proposed(&self) -> &[String] {
        &self.proposed
    }

    /// The tests whose names name one criterion, in matrix order.
    ///
    /// No methodology section applies; this is the mapping itself.
    #[must_use]
    pub fn covering_tests(&self, id: &str) -> Vec<&TestFn> {
        self.tests
            .iter()
            .filter(|test| test.covers.iter().any(|covered| covered == id))
            .collect()
    }

    /// Every criterion no test names.
    ///
    /// ORI-P1-011: these are what the gate fails on and lists.
    #[must_use]
    pub fn uncovered(&self) -> Vec<&Criterion> {
        self.criteria
            .iter()
            .filter(|criterion| self.covering_tests(&criterion.id).is_empty())
            .collect()
    }

    /// Every test whose name names no criterion.
    ///
    /// ORI-P1-011: these are listed as unmapped and do not fail the gate.
    #[must_use]
    pub fn unmapped(&self) -> Vec<&TestFn> {
        self.tests
            .iter()
            .filter(|test| test.covers.is_empty())
            .collect()
    }

    /// Every test name that names an identifier no criterion carries, with
    /// that identifier.
    ///
    /// AICD §39: a reference checked mechanically, against the corpus it
    /// claims to point into.
    #[must_use]
    pub fn dangling(&self) -> Vec<(&TestFn, &str)> {
        let known: BTreeSet<&str> = self.criteria.iter().map(|c| c.id.as_str()).collect();
        let mut out = Vec::new();
        for test in &self.tests {
            for id in &test.covers {
                if !known.contains(id.as_str()) {
                    out.push((test, id.as_str()));
                }
            }
        }
        out
    }

    /// Every line of the matrix, failures first, each with its section.
    ///
    /// AICD §14: the pull request evidence is the list, not the count.
    #[must_use]
    pub fn findings(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        for criterion in self.uncovered() {
            out.push(Finding {
                kind: FindingKind::UncoveredCriterion,
                subject: criterion.id.clone(),
                location: format!("{}:{}", criterion.file, criterion.line),
                detail: format!(
                    "no test name embeds it, so nothing proves it ({})",
                    FindingKind::UncoveredCriterion.methodology_ref()
                ),
            });
        }
        for (test, id) in self.dangling() {
            out.push(Finding {
                kind: FindingKind::DanglingReference,
                subject: test.name.clone(),
                location: format!("{}:{}", test.file, test.line),
                detail: format!(
                    "names {id}, which no criteria file carries, so it claims coverage of \
                     nothing ({})",
                    FindingKind::DanglingReference.methodology_ref()
                ),
            });
        }
        for test in self.unmapped() {
            out.push(Finding {
                kind: FindingKind::UnmappedTest,
                subject: test.name.clone(),
                location: format!("{}:{}", test.file, test.line),
                detail: format!(
                    "names no criterion; listed, not failed ({})",
                    FindingKind::UnmappedTest.methodology_ref()
                ),
            });
        }
        out
    }

    /// The gate's answer for this tree.
    ///
    /// ORI-P1-011. Failed when any finding fails, which is every uncovered
    /// criterion and every dangling reference and nothing else.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if self.findings().iter().any(|finding| finding.kind.fails()) {
            Verdict::Failed
        } else {
            Verdict::Passed
        }
    }

    /// The matrix as markdown, for the pull request.
    ///
    /// `spec/PRD.md` V-03 requires it posted on the pull request; AICD §14
    /// makes it the artifact the verifier reads. Deterministic: sorted
    /// throughout, so two runs on one tree produce one text.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# Coverage matrix (gate 4)\n\n");
        out.push_str("| Criterion | Covered by |\n|---|---|\n");
        for criterion in &self.criteria {
            let covering = self.covering_tests(&criterion.id);
            let cell = if covering.is_empty() {
                "NOTHING".to_string()
            } else {
                covering
                    .iter()
                    .map(|test| format!("{} ({}:{})", test.name, test.file, test.line))
                    .collect::<Vec<_>>()
                    .join("<br>")
            };
            out.push_str(&format!("| {} | {} |\n", criterion.id, cell));
        }

        let uncovered = self.uncovered();
        let dangling = self.dangling();
        let unmapped = self.unmapped();

        out.push_str(&format!(
            "\n## Uncovered criteria: {} (each one fails this gate)\n\n",
            uncovered.len()
        ));
        if uncovered.is_empty() {
            out.push_str("None.\n");
        }
        for criterion in &uncovered {
            out.push_str(&format!(
                "- {} ({}:{})\n",
                criterion.id, criterion.file, criterion.line
            ));
        }

        out.push_str(&format!(
            "\n## Dangling references: {} (each one fails this gate)\n\n",
            dangling.len()
        ));
        if dangling.is_empty() {
            out.push_str("None.\n");
        }
        for (test, id) in &dangling {
            out.push_str(&format!(
                "- {} names {id}, which no criteria file carries ({}:{})\n",
                test.name, test.file, test.line
            ));
        }

        out.push_str(&format!(
            "\n## Unmapped tests: {} (listed, not failed: ORI-P1-011)\n\n",
            unmapped.len()
        ));
        if unmapped.is_empty() {
            out.push_str("None.\n");
        }
        for test in &unmapped {
            out.push_str(&format!("- {} ({}:{})\n", test.name, test.file, test.line));
        }

        if !self.proposed.is_empty() {
            out.push_str(&format!(
                "\n## Proposed, not counted: {}\n\n",
                self.proposed.len()
            ));
            for id in &self.proposed {
                out.push_str(&format!("- {id}\n"));
            }
        }

        out.push_str(&format!(
            "\n{} criterion(s), {} covered, {} uncovered; {} test(s), {} mapped, {} unmapped, \
             {} dangling reference(s).\nVerdict: {}\n",
            self.criteria.len(),
            self.criteria.len() - uncovered.len(),
            uncovered.len(),
            self.tests.len(),
            self.tests.len() - unmapped.len(),
            unmapped.len(),
            dangling.len(),
            self.verdict()
        ));
        out
    }
}

/// Every criterion identifier a test name embeds, deduplicated, in order.
///
/// AICD §14: "every criterion identifier must appear in at least one test".
/// `spec/TESTING.md` section 2 fixes the identifier as `ORI-<phase>-<nnn>`, and
/// a Rust function name carries neither hyphen nor upper case, so the form
/// this reads is `ori_p1_011`.
///
/// The phase part must be `p` and digits, which is what `spec/criteria/`
/// numbers its phases. That is deliberate and it is what keeps a name like
/// `ori_t_0042_something`, a ticket identifier, out of the mapping: it is a
/// test that names no criterion, and ORI-P1-011 says such a test is listed and
/// not failed. Reading a ticket identifier as a criterion reference would fail
/// the build over a naming convention several tickets used on purpose.
///
/// A reference is bounded on both sides: it starts at the name's start or
/// after `_`, and ends at the name's end or at `_`. So `ori_p1_0111_x` names
/// `ORI-P1-0111`, which no criteria file carries, and is reported as dangling
/// rather than quietly read as `ORI-P1-011`.
#[must_use]
pub fn criterion_references(test_name: &str) -> Vec<String> {
    let lower = test_name.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut found: Vec<String> = Vec::new();
    let mut at = 0usize;

    while at < bytes.len() {
        if at + 4 > bytes.len() || &bytes[at..at + 4] != b"ori_" {
            at += 1;
            continue;
        }
        if at > 0 && bytes[at - 1] != b'_' {
            at += 1;
            continue;
        }
        let mut cursor = at + 4;
        if bytes.get(cursor) != Some(&b'p') {
            at += 1;
            continue;
        }
        cursor += 1;
        let phase_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == phase_start || bytes.get(cursor) != Some(&b'_') {
            at += 1;
            continue;
        }
        let phase = lower[phase_start..cursor].to_string();
        cursor += 1;
        let number_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == number_start {
            at += 1;
            continue;
        }
        match bytes.get(cursor) {
            None | Some(&b'_') => {}
            Some(_) => {
                at += 1;
                continue;
            }
        }
        let identifier = format!("ORI-P{phase}-{}", &lower[number_start..cursor]);
        if !found.contains(&identifier) {
            found.push(identifier);
        }
        at = cursor;
    }

    found
}

/// The accepted criteria of one criteria file, and the identifiers proposed
/// below its boundary line.
///
/// `spec/TESTING.md` section 2: proposals do not count until accepted, and
/// `spec/criteria/phase-1.md` marks the boundary with a sentence beginning
/// "Proposed criteria".
///
/// A row is a criterion when its first cell is exactly an identifier. A first
/// cell that contains one without being one, `**ORI-P1-011**` for instance, is
/// [`CoverageError::UnreadableCriterionCell`] and not a skip: a criterion the
/// reader steps over is a criterion nothing asks for a test for, which is the
/// vacuous pass the floors exist to stop.
///
/// # Errors
///
/// [`CoverageError::UnreadableCriterionCell`] and
/// [`CoverageError::PhaseMismatch`].
pub fn criteria_in(text: &str, file: &str) -> Result<(Vec<Criterion>, Vec<String>), CoverageError> {
    let expected_phase = phase_of_file(file);
    let mut accepted = Vec::new();
    let mut proposed = Vec::new();
    let mut past_boundary = false;

    for (offset, raw) in text.lines().enumerate() {
        let line = offset + 1;
        let trimmed = raw.trim();
        if trimmed.to_ascii_lowercase().starts_with(PROPOSED_MARKER) {
            past_boundary = true;
            continue;
        }
        if !trimmed.starts_with('|') {
            continue;
        }
        let Some(cell) = trimmed
            .trim_start_matches('|')
            .split('|')
            .next()
            .map(str::trim)
        else {
            continue;
        };
        if !cell.contains("ORI-P") {
            continue;
        }
        let Some(phase) = identifier_phase(cell) else {
            return Err(CoverageError::UnreadableCriterionCell {
                file: file.to_string(),
                line,
                found: cell.to_string(),
            });
        };
        if past_boundary {
            proposed.push(cell.to_string());
            continue;
        }
        if let Some(expected) = expected_phase.as_deref()
            && expected != phase
        {
            return Err(CoverageError::PhaseMismatch {
                file: file.to_string(),
                line,
                id: cell.to_string(),
                expected: expected.to_string(),
            });
        }
        accepted.push(Criterion {
            id: cell.to_string(),
            phase,
            file: file.to_string(),
            line,
        });
    }

    Ok((accepted, proposed))
}

/// Every test function one Rust source declares, with the criteria its name
/// names.
///
/// AICD §14. A test is the first function declared after a line that is
/// exactly one of [`TEST_ATTRIBUTES`]. Exactly, so that the `"#[test]\n"` of a
/// quoted fixture is not read as an attribute of the file quoting it; this
/// file and [`crate::sections`] are both full of those, and a phantom test
/// would map a criterion to a test that does not exist.
///
/// # Errors
///
/// [`CoverageError::UnknownTestAttribute`],
/// [`CoverageError::AttributeNotAlone`] and
/// [`CoverageError::AttributeWithoutFunction`]. Each is a test the reader
/// would otherwise have lost silently.
pub fn tests_in(source: &str, file: &str) -> Result<Vec<TestFn>, CoverageError> {
    let declarations = function_declarations(source);
    let mut out = Vec::new();

    for (offset, raw) in source.lines().enumerate() {
        let line = offset + 1;
        let trimmed = raw.trim();

        let Some((attribute, _)) = TEST_ATTRIBUTES
            .iter()
            .find(|(spelling, _)| *spelling == trimmed)
        else {
            // Two shapes that must not become silent skips: an attribute the
            // register does not know, and a known one sharing its line with
            // something else. cargo fmt is a gate here, and it puts an
            // attribute on its own line, so the second shape means the source
            // did not go through it.
            if let Some((spelling, _)) = TEST_ATTRIBUTES
                .iter()
                .find(|(spelling, _)| trimmed.starts_with(*spelling))
            {
                return Err(CoverageError::AttributeNotAlone {
                    file: file.to_string(),
                    line,
                    attribute: (*spelling).to_string(),
                    found: trimmed.to_string(),
                });
            }
            if looks_like_test_attribute(trimmed) {
                return Err(CoverageError::UnknownTestAttribute {
                    file: file.to_string(),
                    line,
                    found: trimmed.to_string(),
                });
            }
            continue;
        };

        let Some((_, name)) = declarations.iter().find(|(at, _)| *at > line) else {
            return Err(CoverageError::AttributeWithoutFunction {
                file: file.to_string(),
                line,
                attribute: (*attribute).to_string(),
            });
        };
        out.push(TestFn {
            name: (*name).to_string(),
            file: file.to_string(),
            line,
            attribute: (*attribute).to_string(),
            covers: criterion_references(name),
        });
    }

    Ok(out)
}

/// Whether a line is shaped like a test attribute the reader does not know.
///
/// AICD §14: the register in [`TEST_ATTRIBUTES`] is only a floor if something
/// notices an attribute outside it. The shape is `#[`, a path whose last
/// segment is `test`, optional arguments, `]`, which admits `#[some::test]`
/// and refuses `#[cfg(test)]`, whose last path segment is `cfg`.
fn looks_like_test_attribute(trimmed: &str) -> bool {
    let Some(inner) = trimmed.strip_prefix("#[") else {
        return false;
    };
    let Some(inner) = inner.strip_suffix(']') else {
        return false;
    };
    let path = match inner.split('(').next() {
        Some(path) => path.trim(),
        None => inner.trim(),
    };
    path.rsplit("::").next().map(str::trim) == Some("test")
}

/// Every `fn` one source declares, as (1-based line, name).
///
/// No methodology section applies. A copy of [`crate::sections`]'s reader, for
/// the reason the head of this module gives. Line based and prefix checked
/// rather than a search for the token, because a search finds the word in
/// prose and inside quoted source fixtures, and every phantom it adds is a
/// name a mapping could resolve against.
fn function_declarations(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();

    for (offset, raw) in source.lines().enumerate() {
        let line = raw.trim();
        let bytes = line.as_bytes();
        let mut at = 0usize;
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
        // A declaration's name is followed by its parameters or its generics.
        // Anything else is a word that happened to follow the token.
        if cursor > start && matches!(bytes.get(cursor), Some(b'(') | Some(b'<')) {
            out.push((offset + 1, &line[start..cursor]));
        }
    }

    out
}

/// Whether a byte can sit inside a Rust identifier.
///
/// No methodology section applies.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The phase part of an identifier cell, when the cell is exactly one.
///
/// `spec/TESTING.md` section 2 fixes the form `ORI-<phase>-<nnn>`.
fn identifier_phase(cell: &str) -> Option<String> {
    let rest = cell.strip_prefix("ORI-P")?;
    let (phase_digits, number) = rest.split_once('-')?;
    if phase_digits.is_empty() || !phase_digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("P{phase_digits}"))
}

/// The phase a criteria file's name claims, when it claims one.
///
/// `spec/TESTING.md` section 3: criteria live in `criteria/` per phase, phase
/// 1 in `criteria/phase-1.md`. A file not named that way is read without the
/// cross check rather than refused, because no document forbids another name.
fn phase_of_file(file: &str) -> Option<String> {
    let stem = file.rsplit('/').next()?.strip_suffix(".md")?;
    let digits = stem.strip_prefix("phase-")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("P{digits}"))
}

/// Every file under one directory, recursively, sorted, excluding
/// [`SCAN_EXCLUSIONS`].
///
/// AICD §14: a file the walk cannot read must not become a file the walk
/// passes over, so every failure is returned and none is skipped.
fn walk(directory: &Path) -> Result<Vec<PathBuf>, CoverageError> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];

    while let Some(current) = pending.pop() {
        for path in read_dir_sorted(&current)? {
            let metadata = fs::symlink_metadata(&path).map_err(|error| CoverageError::Io {
                path: display_path(&path),
                error,
            })?;
            if metadata.is_dir() {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if SCAN_EXCLUSIONS
                    .iter()
                    .any(|(excluded, _)| *excluded == name)
                {
                    continue;
                }
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(CoverageError::NotAFile {
                    path: display_path(&path),
                });
            }
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// One directory's entries, sorted, with the path in any error.
///
/// No methodology section applies.
fn read_dir_sorted(directory: &Path) -> Result<Vec<PathBuf>, CoverageError> {
    let listing = fs::read_dir(directory).map_err(|error| CoverageError::Io {
        path: display_path(directory),
        error,
    })?;
    let mut paths = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|error| CoverageError::Io {
            path: display_path(directory),
            error,
        })?;
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

/// One file as text, with the repository-relative path in any error.
///
/// No methodology section applies.
fn read_text(path: &Path, relative: &str) -> Result<String, CoverageError> {
    fs::read_to_string(path).map_err(|error| CoverageError::Io {
        path: relative.to_string(),
        error,
    })
}

/// A path relative to the product root, `/` separated.
///
/// No methodology section applies.
fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A path as text for a message.
///
/// No methodology section applies.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// What stops the matrix being read at all.
///
/// AICD §14: every one of these is a gate failure and none is a pass. A caller
/// that maps an error to "not available" has rebuilt the defect class AICD §39
/// names, a check that is present and reports nothing.
///
/// Hand written rather than derived because `thiserror` is deferred to batch 2
/// by ruling R20 in `ops/rulings.md`. When it lands, each arm of the
/// [`fmt::Display`] match below becomes one attribute on the variant it
/// prints, the `error` field of [`CoverageError::Io`] takes the source
/// attribute, and both hand written implementations are deleted. No message
/// and no call site changes.
#[derive(Debug)]
pub enum CoverageError {
    /// A file or directory could not be read.
    Io {
        /// What was attempted.
        path: String,
        /// What the filesystem said.
        error: io::Error,
    },
    /// The criteria directory holds no markdown file.
    NoCriteriaFiles {
        /// The directory that was read.
        directory: String,
    },
    /// A criteria file carries no accepted criterion.
    EmptyCriteriaFile {
        /// The file that was read.
        file: String,
    },
    /// No accepted criterion was found anywhere.
    NoCriteria,
    /// No test was found anywhere.
    NoTests,
    /// A test root holds no Rust source.
    NoSourceUnderRoot {
        /// The root that was read.
        root: String,
        /// Why that root is read at all.
        why: String,
    },
    /// A table cell contains an identifier without being one.
    UnreadableCriterionCell {
        /// The file it is in.
        file: String,
        /// The 1-based line.
        line: usize,
        /// The cell as written.
        found: String,
    },
    /// A criterion's phase disagrees with the file it is written in.
    PhaseMismatch {
        /// The file it is in.
        file: String,
        /// The 1-based line.
        line: usize,
        /// The identifier as written.
        id: String,
        /// The phase the file name claims.
        expected: String,
    },
    /// One identifier is carried by two rows.
    DuplicateCriterion {
        /// The identifier carried twice.
        id: String,
        /// Where it was first seen.
        first: String,
        /// Where it was seen again.
        second: String,
    },
    /// A line is shaped like a test attribute and is not in the register.
    UnknownTestAttribute {
        /// The file it is in.
        file: String,
        /// The 1-based line.
        line: usize,
        /// The line as written.
        found: String,
    },
    /// A known test attribute shares its line with something else.
    AttributeNotAlone {
        /// The file it is in.
        file: String,
        /// The 1-based line.
        line: usize,
        /// The attribute the line starts with.
        attribute: String,
        /// The line as written.
        found: String,
    },
    /// A test attribute is followed by no function declaration.
    AttributeWithoutFunction {
        /// The file it is in.
        file: String,
        /// The 1-based line.
        line: usize,
        /// The attribute that names nothing.
        attribute: String,
    },
    /// A directory entry is neither a file nor a directory.
    NotAFile {
        /// The entry.
        path: String,
    },
}

impl CoverageError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute
    /// rule 9).
    ///
    /// Returned as text rather than as `ori_core::MethodologyRef` for the
    /// reason `SectionsError` in [`crate::sections`] gives: `ori-gates` does
    /// not depend on `ori-core`.
    #[must_use]
    pub fn methodology_ref(&self) -> &'static str {
        match self {
            // A reference to a criterion that is not there, or a name that
            // cannot be resolved: references are checked mechanically.
            Self::UnreadableCriterionCell { .. }
            | Self::PhaseMismatch { .. }
            | Self::DuplicateCriterion { .. } => "AICD §39",
            // Everything else is a way for this gate to look at nothing and
            // report a pass, which is what a gate may not do.
            _ => "AICD §14",
        }
    }
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "cannot read {path}: {error}"),
            Self::NoCriteriaFiles { directory } => write!(
                f,
                "{directory} holds no markdown file, so there are no criteria to map tests to and \
                 every test would read as unmapped against an empty corpus"
            ),
            Self::EmptyCriteriaFile { file } => write!(
                f,
                "{file} is a criteria file with no criterion row in it, which means the reader is \
                 broken or the table moved; either way the criteria in it would go unasked for"
            ),
            Self::NoCriteria => f.write_str(
                "no accepted criterion was read anywhere, so \"every criterion is covered\" is \
                 vacuously true and this gate would pass any tree at all",
            ),
            Self::NoTests => f.write_str(
                "no test was read anywhere, so nothing could cover anything; a gate that reports \
                 a pass here is reporting that it did not look",
            ),
            Self::NoSourceUnderRoot { root, why } => write!(
                f,
                "no Rust source was found under {root}, which is read because it is {why}; either \
                 the tree moved or the walk is broken, and in both cases the tests there would be \
                 lost without a word"
            ),
            Self::UnreadableCriterionCell { file, line, found } => write!(
                f,
                "{file}:{line}: the first cell is {found:?}, which contains an identifier without \
                 being one; a row the reader steps over is a criterion nothing asks for a test for"
            ),
            Self::PhaseMismatch {
                file,
                line,
                id,
                expected,
            } => write!(
                f,
                "{file}:{line}: {id} is not a criterion of phase {expected}, which is the phase \
                 this file's name claims; one of the two is wrong"
            ),
            Self::DuplicateCriterion { id, first, second } => write!(
                f,
                "{id} is carried twice, at {first} and at {second}, so a test naming it maps to an \
                 ambiguous row"
            ),
            Self::UnknownTestAttribute { file, line, found } => write!(
                f,
                "{file}:{line}: {found:?} is shaped like a test attribute and is not one this \
                 reader knows; a test it cannot see is a test this matrix loses silently"
            ),
            Self::AttributeNotAlone {
                file,
                line,
                attribute,
                found,
            } => write!(
                f,
                "{file}:{line}: {found:?} starts with {attribute} and does not end there; the \
                 reader takes an attribute only when it has its line to itself, and cargo fmt is a \
                 gate here, so this source did not go through it"
            ),
            Self::AttributeWithoutFunction {
                file,
                line,
                attribute,
            } => write!(
                f,
                "{file}:{line}: {attribute} is followed by no function declaration, so the reader \
                 has an attribute and no name to map"
            ),
            Self::NotAFile { path } => write!(
                f,
                "{path} is neither a file nor a directory, so the walk would pass over it without \
                 reading it"
            ),
        }
    }
}

impl std::error::Error for CoverageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two naming conventions in this module, both deliberate, and the gate
    // this file implements prints the consequence.
    //
    // The tests that prove what ORI-P1-011 describes carry ori_p1_011 in their
    // names. They are what covers that criterion, and before this file no test
    // in this repository covered it.
    //
    // The rest prove the instruments: the two readers, the walk, the error
    // paths. No criterion in spec/criteria/ covers a reader, so they carry the
    // ticket identifier instead of an invented criterion. They appear in this
    // gate's own unmapped list, which is the honest answer and is reported in
    // the pull request rather than exempted away.

    /// The repository root, two levels above `crates/ori-gates`.
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("a crate lives two levels below the repository root")
            .to_path_buf()
    }

    // ---- fixtures ----
    //
    // Written as one string literal per line, the way sections.rs writes its
    // source fixtures. A raw multi-line literal would put a line that trims to
    // exactly "#[test]" into this file, and the reader in this module would
    // then read this file's fixtures as this file's tests. That is the phantom
    // a sibling ticket found in its own reader, and the shape of the fixture
    // is what keeps it out.

    const CRITERIA_FIXTURE: &str = concat!(
        "# Acceptance criteria: Phase 1\n",
        "\n",
        "| ID | Type | Tier | Precondition | Action | Expected result |\n",
        "|---|---|---|---|---|---|\n",
        "| ORI-P1-001 | F | 1 | Empty directory | do a thing | a result |\n",
        "| ORI-P1-002 | F | 1 | Something | do another | another result |\n",
        "\n",
        "Proposed criteria from the QA agent are appended below this line.\n",
        "\n",
        "| ORI-P1-900 | F | 1 | Proposed | proposed | proposed |\n",
    );

    const SOURCE_FIXTURE: &str = concat!(
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    #[test]\n",
        "    fn ori_p1_001_the_first_criterion_holds() {}\n",
        "\n",
        "    #[test]\n",
        "    fn ori_p1_002_and_ori_p1_001_together() {}\n",
        "\n",
        "    #[test]\n",
        "    fn ori_t_0042_the_reader_reads() {}\n",
        "}\n",
    );

    fn fixture_matrix() -> CoverageMatrix {
        let (criteria, proposed) =
            criteria_in(CRITERIA_FIXTURE, "spec/criteria/phase-1.md").expect("the fixture parses");
        let tests =
            tests_in(SOURCE_FIXTURE, "crates/ori-x/src/lib.rs").expect("the fixture parses");
        CoverageMatrix::new(criteria, tests, proposed).expect("the fixture has both sides")
    }

    // ---- ORI-P1-011: what the criterion describes ----

    /// A criterion no test names fails the gate and is named in the report.
    ///
    /// ORI-P1-011, first half: "Gate fails and lists the criterion".
    #[test]
    fn ori_p1_011_a_criterion_with_no_test_fails_the_gate_and_is_listed() {
        let source = concat!(
            "    #[test]\n",
            "    fn ori_p1_001_the_first_criterion_holds() {}\n",
        );
        let (criteria, proposed) =
            criteria_in(CRITERIA_FIXTURE, "spec/criteria/phase-1.md").expect("the fixture parses");
        let tests = tests_in(source, "crates/ori-x/src/lib.rs").expect("the fixture parses");
        let matrix = CoverageMatrix::new(criteria, tests, proposed).expect("both sides are there");

        assert_eq!(matrix.verdict(), Verdict::Failed);
        let uncovered: Vec<&str> = matrix
            .uncovered()
            .iter()
            .map(|criterion| criterion.id.as_str())
            .collect();
        assert_eq!(uncovered, vec!["ORI-P1-002"]);

        let report = matrix.render();
        assert!(
            report.contains("ORI-P1-002 (spec/criteria/phase-1.md:6)"),
            "the report must name the criterion and where it is written:\n{report}"
        );
        assert!(
            report.contains("Uncovered criteria: 1"),
            "the report must count them:\n{report}"
        );
        let findings = matrix.findings();
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UncoveredCriterion
                    && finding.subject == "ORI-P1-002"
                    && finding.kind.fails())
        );
    }

    /// A test naming no criterion is listed and does not fail the gate.
    ///
    /// ORI-P1-011, second half: "a test naming no criterion is listed as
    /// unmapped". The asymmetry is the whole of it. Failing here would refuse
    /// every build of this repository, where most tests legitimately cover no
    /// criterion.
    #[test]
    fn ori_p1_011_a_test_naming_no_criterion_is_listed_and_does_not_fail() {
        let matrix = fixture_matrix();

        assert_eq!(
            matrix.verdict(),
            Verdict::Passed,
            "an unmapped test must not fail the gate:\n{}",
            matrix.render()
        );
        let unmapped: Vec<&str> = matrix
            .unmapped()
            .iter()
            .map(|test| test.name.as_str())
            .collect();
        assert_eq!(unmapped, vec!["ori_t_0042_the_reader_reads"]);

        let report = matrix.render();
        assert!(
            report.contains("Unmapped tests: 1"),
            "the report must list it:\n{report}"
        );
        assert!(
            report.contains("ori_t_0042_the_reader_reads (crates/ori-x/src/lib.rs:9)"),
            "the report must say which test and where:\n{report}"
        );
        assert!(
            matrix
                .findings()
                .iter()
                .all(|finding| finding.kind != FindingKind::UnmappedTest || !finding.kind.fails())
        );
    }

    /// A test named after a criterion that does not exist fails the gate.
    ///
    /// Derived, not quoted: AICD §39 has references checked mechanically, and
    /// a name resolving to nothing would enter the matrix as coverage of
    /// nothing.
    #[test]
    fn ori_p1_011_a_test_naming_a_criterion_that_does_not_exist_fails() {
        let source = concat!(
            "    #[test]\n",
            "    fn ori_p1_001_the_first_criterion_holds() {}\n",
            "\n",
            "    #[test]\n",
            "    fn ori_p1_002_holds_too() {}\n",
            "\n",
            "    #[test]\n",
            "    fn ori_p1_999_covers_a_criterion_nobody_wrote() {}\n",
        );
        let (criteria, proposed) =
            criteria_in(CRITERIA_FIXTURE, "spec/criteria/phase-1.md").expect("the fixture parses");
        let tests = tests_in(source, "crates/ori-x/src/lib.rs").expect("the fixture parses");
        let matrix = CoverageMatrix::new(criteria, tests, proposed).expect("both sides are there");

        assert_eq!(matrix.verdict(), Verdict::Failed);
        let dangling: Vec<(&str, &str)> = matrix
            .dangling()
            .iter()
            .map(|(test, id)| (test.name.as_str(), *id))
            .collect();
        assert_eq!(
            dangling,
            vec![("ori_p1_999_covers_a_criterion_nobody_wrote", "ORI-P1-999")]
        );
        assert!(
            matrix.render().contains("Dangling references: 1"),
            "the report must count them:\n{}",
            matrix.render()
        );
        assert!(
            matrix.unmapped().is_empty(),
            "a dangling name is not an unmapped test; it names something, and the something is \
             not there"
        );
    }

    /// A criterion covered by any test is covered, however many name it.
    ///
    /// ORI-P1-011 by its converse: the gate passes the side it is clean on.
    /// AICD §14 requires the identifier in "at least one test".
    #[test]
    fn ori_p1_011_one_test_naming_two_criteria_covers_both() {
        let matrix = fixture_matrix();

        assert_eq!(matrix.verdict(), Verdict::Passed);
        assert!(matrix.uncovered().is_empty());
        let names: Vec<&str> = matrix
            .covering_tests("ORI-P1-002")
            .iter()
            .map(|test| test.name.as_str())
            .collect();
        assert_eq!(names, vec!["ori_p1_002_and_ori_p1_001_together"]);
        assert_eq!(matrix.covering_tests("ORI-P1-001").len(), 2);
    }

    /// A proposed criterion is not asked for a test.
    ///
    /// `spec/TESTING.md` section 2: "Proposed criteria (from the QA agent) do
    /// not count until accepted." ORI-P1-011 would otherwise fail the gate on
    /// a proposal nobody has accepted.
    #[test]
    fn ori_p1_011_a_proposed_criterion_does_not_have_to_be_covered() {
        let matrix = fixture_matrix();

        assert_eq!(matrix.criteria().len(), 2);
        assert_eq!(matrix.proposed(), ["ORI-P1-900"]);
        assert!(
            matrix
                .uncovered()
                .iter()
                .all(|criterion| criterion.id != "ORI-P1-900")
        );
        assert!(
            matrix.render().contains("Proposed, not counted: 1"),
            "a reader must be able to see the boundary was found:\n{}",
            matrix.render()
        );
    }

    /// An empty criteria list is refused, not passed.
    ///
    /// AICD §14. With no criteria, every criterion is covered and the gate
    /// reports a clean tree it never looked at. This is planted defect 6 of
    /// this ticket's proof and the shape this gate exists to catch.
    #[test]
    fn ori_p1_011_an_empty_criteria_list_is_an_error_and_not_a_pass() {
        let tests =
            tests_in(SOURCE_FIXTURE, "crates/ori-x/src/lib.rs").expect("the fixture parses");
        let error = CoverageMatrix::new(Vec::new(), tests, Vec::new())
            .expect_err("an empty criteria list must not produce a matrix");

        assert!(matches!(error, CoverageError::NoCriteria));
        assert_eq!(error.methodology_ref(), "AICD §14");
        assert!(
            error.to_string().contains("vacuously true"),
            "the message must say why: {error}"
        );
    }

    /// An empty test list is refused, not passed.
    ///
    /// AICD §14, and planted defect 7 of this ticket's proof.
    #[test]
    fn ori_p1_011_an_empty_test_list_is_an_error_and_not_a_pass() {
        let (criteria, proposed) =
            criteria_in(CRITERIA_FIXTURE, "spec/criteria/phase-1.md").expect("the fixture parses");
        let error = CoverageMatrix::new(criteria, Vec::new(), proposed)
            .expect_err("an empty test list must not produce a matrix");

        assert!(matches!(error, CoverageError::NoTests));
        assert_eq!(error.methodology_ref(), "AICD §14");
        assert!(
            error.to_string().contains("did not look"),
            "the message must say why: {error}"
        );
    }

    /// The gate reports this repository, and the report is the finding.
    ///
    /// ORI-P1-011 run against the tree it ships in. This asserts the floors
    /// and prints the matrix; it does not assert the verdict, because the
    /// verdict on this tree is FAILED and has been since before this module
    /// existed. Asserting it would turn gate 2 red for a fact about the
    /// repository that this ticket has no claim to fix, so the matrix is
    /// printed for the pull request and the operator decides what it means.
    /// That is the same call ruling R19 made for the unresolvable citations in
    /// `crate::sections`: report, correct nothing.
    #[test]
    fn ori_p1_011_the_gate_reads_this_repository_and_reports_the_matrix() {
        let root = repo_root();
        let matrix = CoverageMatrix::read(&root).expect("this repository has criteria and tests");

        // The floors, checked here rather than assumed, because every one of
        // them is a way for the run above to have looked at nothing.
        assert!(
            matrix.criteria().len() >= 41,
            "spec/criteria/phase-1.md carried 41 accepted criteria when this landed and criteria \
             are only added; {} were read, so the reader is broken",
            matrix.criteria().len()
        );
        assert!(
            matrix.tests().len() >= 146,
            "gate 2 counted 146 tests when this landed; {} were read, so the reader is broken",
            matrix.tests().len()
        );
        assert!(
            matrix.tests().iter().any(|test| test.name
                == "ori_p1_011_the_gate_reads_this_repository_and_reports_the_matrix"),
            "the reader did not find this test's own name among the {} it collected, so it is \
             not reading test functions the way they are actually written",
            matrix.tests().len()
        );
        assert!(
            matrix
                .criteria()
                .iter()
                .any(|criterion| criterion.id == "ORI-P1-011"),
            "the criterion this gate is built against was not read from spec/criteria/"
        );
        assert!(
            !matrix.covering_tests("ORI-P1-011").is_empty(),
            "this file covers ORI-P1-011 and the matrix does not say so, so the mapping is broken"
        );

        println!("{}", matrix.render());
    }

    // ---- ORI-T-0042: the instruments ----

    /// The reader takes only bounded, phase-shaped identifiers from a name.
    #[test]
    fn ori_t_0042_a_name_carries_the_identifiers_it_embeds_and_no_others() {
        assert_eq!(
            criterion_references("ori_p1_011_a_thing"),
            vec!["ORI-P1-011".to_string()]
        );
        assert_eq!(
            criterion_references("covers_ori_p1_011_and_ori_p2_007_at_once"),
            vec!["ORI-P1-011".to_string(), "ORI-P2-007".to_string()]
        );
        assert_eq!(
            criterion_references("ori_p1_011_and_ori_p1_011_again"),
            vec!["ORI-P1-011".to_string()],
            "one identifier written twice is one reference"
        );
        assert_eq!(
            criterion_references("ori_p1_0111_x"),
            vec!["ORI-P1-0111".to_string()],
            "a fourth digit makes a different identifier, reported as dangling rather than read \
             as the three-digit one"
        );
        assert!(
            criterion_references("ori_t_0042_the_reader_reads").is_empty(),
            "a ticket identifier is not a criterion identifier"
        );
        assert!(
            criterion_references("memori_p1_011_x").is_empty(),
            "a reference must start at the name's start or after an underscore"
        );
        assert!(criterion_references("ori_p_011_x").is_empty());
        assert!(criterion_references("ori_1_011_x").is_empty());
        assert!(criterion_references("ori_p1_x").is_empty());
    }

    /// The test reader separates attributes from prose, quoted source and
    /// `cfg(test)`.
    #[test]
    fn ori_t_0042_the_test_reader_reads_attributes_and_not_the_text_around_them() {
        let source = concat!(
            "//! A module mentioning a test attribute in prose: #[test].\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    const FIXTURE: &str = concat!(\n",
            "        \"#[test]\\n\",\n",
            "        \"fn ori_p1_777_a_quoted_fixture() {}\\n\",\n",
            "    );\n",
            "\n",
            "    #[test]\n",
            "    #[ignore = \"an attribute between the two\"]\n",
            "    fn ori_p1_001_a_real_one() {}\n",
            "}\n",
        );
        let tests = tests_in(source, "crates/ori-x/src/lib.rs").expect("the fixture parses");

        let names: Vec<&str> = tests.iter().map(|test| test.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["ori_p1_001_a_real_one"],
            "the quoted fixture, the prose and the cfg attribute are not tests of this file"
        );
        assert_eq!(tests[0].line, 9, "the line reported is the attribute's");
        assert_eq!(tests[0].attribute, "#[test]");
    }

    /// The async spelling is read, so an async test is not lost.
    #[test]
    fn ori_t_0042_the_async_attribute_in_the_register_is_read() {
        let source = concat!(
            "    #[tokio::test]\n",
            "    async fn ori_p1_001_an_async_one() {}\n",
        );
        let tests = tests_in(source, "crates/ori-x/src/lib.rs").expect("the fixture parses");

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].attribute, "#[tokio::test]");
        assert_eq!(tests[0].covers, vec!["ORI-P1-001".to_string()]);
    }

    /// An attribute outside the register is refused rather than skipped.
    #[test]
    fn ori_t_0042_an_unregistered_test_attribute_is_refused() {
        let source = concat!(
            "    #[async_std::test]\n",
            "    async fn ori_p1_001_a_runtime_nobody_registered() {}\n",
        );
        let error = tests_in(source, "crates/ori-x/src/lib.rs")
            .expect_err("an attribute the register does not know must not be skipped");

        assert!(matches!(error, CoverageError::UnknownTestAttribute { .. }));
        assert_eq!(error.methodology_ref(), "AICD §14");
        assert!(error.to_string().contains("loses silently"), "{error}");
    }

    /// An attribute sharing its line is refused rather than guessed at.
    #[test]
    fn ori_t_0042_an_attribute_sharing_its_line_is_refused() {
        let source = "    #[test] fn ori_p1_001_all_on_one_line() {}\n";
        let error = tests_in(source, "crates/ori-x/src/lib.rs")
            .expect_err("an attribute on a shared line must not be skipped");

        assert!(matches!(error, CoverageError::AttributeNotAlone { .. }));
        assert!(error.to_string().contains("cargo fmt"), "{error}");
    }

    /// A test attribute at the end of a file names nothing and is refused.
    #[test]
    fn ori_t_0042_a_test_attribute_with_no_function_after_it_is_refused() {
        let source = concat!("mod tests {\n", "    #[test]\n", "}\n");
        let error = tests_in(source, "crates/ori-x/src/lib.rs")
            .expect_err("an attribute with no function must not be skipped");

        assert!(matches!(
            error,
            CoverageError::AttributeWithoutFunction { .. }
        ));
    }

    /// A criteria row that hides its identifier in markup is refused.
    #[test]
    fn ori_t_0042_a_criterion_cell_that_is_not_exactly_an_identifier_is_refused() {
        let text = concat!("| ID | Type |\n", "|---|---|\n", "| **ORI-P1-001** | F |\n",);
        let error = criteria_in(text, "spec/criteria/phase-1.md")
            .expect_err("a cell that only contains an identifier must not be read past");

        assert!(matches!(
            error,
            CoverageError::UnreadableCriterionCell { .. }
        ));
        assert_eq!(error.methodology_ref(), "AICD §39");
    }

    /// A criterion in the wrong phase file is refused.
    #[test]
    fn ori_t_0042_a_criterion_whose_phase_disagrees_with_its_file_is_refused() {
        let text = concat!("| ID | Type |\n", "|---|---|\n", "| ORI-P2-001 | F |\n",);
        let error = criteria_in(text, "spec/criteria/phase-1.md")
            .expect_err("a phase 2 criterion in the phase 1 file must not pass unremarked");

        assert!(matches!(error, CoverageError::PhaseMismatch { .. }));
        assert!(
            error.to_string().contains("one of the two is wrong"),
            "{error}"
        );
    }

    /// One identifier carried twice makes the mapping ambiguous and is
    /// refused.
    #[test]
    fn ori_t_0042_a_duplicated_identifier_is_refused() {
        let text = concat!(
            "| ID | Type |\n",
            "|---|---|\n",
            "| ORI-P1-001 | F |\n",
            "| ORI-P1-001 | F |\n",
        );
        let (criteria, proposed) =
            criteria_in(text, "spec/criteria/phase-1.md").expect("it parses");
        let tests =
            tests_in(SOURCE_FIXTURE, "crates/ori-x/src/lib.rs").expect("the fixture parses");
        let error = CoverageMatrix::new(criteria, tests, proposed)
            .expect_err("two rows with one identifier must not produce a matrix");

        assert!(matches!(error, CoverageError::DuplicateCriterion { .. }));
    }

    /// A criteria file with no criterion row reads as empty, and the walk
    /// above turns that into a refusal.
    #[test]
    fn ori_t_0042_a_criteria_file_with_no_criterion_row_reads_as_empty() {
        let (criteria, proposed) = criteria_in(
            "# A document with prose and no table\n",
            "spec/criteria/phase-1.md",
        )
        .expect("prose parses to nothing");

        assert!(criteria.is_empty());
        assert!(proposed.is_empty());
        // CoverageMatrix::read turns this into EmptyCriteriaFile; the reader
        // itself reports what it found, which is nothing.
    }

    /// The phase of a file is read from its name, and only when it says one.
    #[test]
    fn ori_t_0042_the_phase_of_a_criteria_file_comes_from_its_name() {
        assert_eq!(
            phase_of_file("spec/criteria/phase-1.md"),
            Some("P1".to_string())
        );
        // A later phase, and a file that claims no phase. Both are written
        // without the leading directory of the real criteria tree on purpose:
        // neither file exists, and spelled as a path under the specification
        // they would be two references to documents this repository does not
        // carry, which the reader in spec_refs.rs resolves and refuses. The
        // function reads the file name and nothing else, so the two forms
        // exercise the same code.
        assert_eq!(
            phase_of_file("criteria/phase-12.md"),
            Some("P12".to_string())
        );
        assert_eq!(phase_of_file("criteria/notes.md"), None);
    }

    /// The register is a register: what is in it is what the reader takes.
    #[test]
    fn ori_t_0042_every_registered_attribute_is_recognised_by_the_shape_check() {
        for (spelling, why) in TEST_ATTRIBUTES {
            assert!(
                looks_like_test_attribute(spelling),
                "{spelling} is registered because it is {why}, and the shape check does not see \
                 it, so an unregistered sibling of it would pass unnoticed"
            );
        }
        assert!(!looks_like_test_attribute("#[cfg(test)]"));
        assert!(!looks_like_test_attribute("#[should_panic]"));
        assert!(!looks_like_test_attribute("// #[test]"));
    }

    /// The rendered matrix is stable, so two runs on one tree read alike.
    #[test]
    fn ori_t_0042_the_rendered_matrix_is_deterministic() {
        let first = fixture_matrix().render();
        let second = fixture_matrix().render();
        assert_eq!(first, second);
        assert!(first.starts_with("# Coverage matrix (gate 4)"));
        assert!(first.contains("Verdict: PASSED"));
    }

    /// Every finding carries the section it rests on.
    ///
    /// `CLAUDE.md` absolute rule 9, and AICD §14 for the failing pair.
    #[test]
    fn ori_t_0042_every_finding_carries_a_methodology_reference() {
        for kind in [
            FindingKind::UncoveredCriterion,
            FindingKind::DanglingReference,
            FindingKind::UnmappedTest,
        ] {
            let reference = kind.methodology_ref();
            assert!(
                reference == "AICD §14" || reference == "AICD §39",
                "{} carries {reference}, which is neither section this gate derives from",
                kind.label()
            );
        }
        assert!(FindingKind::UncoveredCriterion.fails());
        assert!(FindingKind::DanglingReference.fails());
        assert!(!FindingKind::UnmappedTest.fails());
    }

    /// A missing criteria directory is an error, not an empty pass.
    #[test]
    fn ori_t_0042_a_tree_with_no_criteria_directory_is_an_error() {
        let error = CoverageMatrix::read(Path::new("/nonexistent-product-root-for-ori-t-0042"))
            .expect_err("a tree with no criteria directory must not produce a matrix");

        assert!(matches!(error, CoverageError::Io { .. }));
        assert_eq!(error.methodology_ref(), "AICD §14");
    }
}
