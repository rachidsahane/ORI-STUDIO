//! The modified-test detector: AICD §14, gate 5 of `spec/CI_CD.md` section 1.
//!
//! AICD §14 states the rule this module mechanises: "Any pull request in which
//! an existing test was modified, weakened or removed triggers an escalation to
//! a human, whatever the tier." AICD §12 lists the same fact as an escalation
//! trigger, "a test had to be modified or removed for the build to pass".
//! `spec/PRD.md` V-04 names it as a P0 function and `spec/CI_CD.md` section 1
//! item 5 wires it into the pipeline as "escalation label on any changed or
//! removed existing test".
//!
//! Until this module existed the rule was an honour system. `CLAUDE.md`
//! absolute rule 3 states it to every agent and nothing checked it.
//!
//! # What this module owns, and what it does not
//!
//! Criterion ORI-P1-010 in `spec/criteria/phase-1.md` reads: "PR whose diff
//! modifies an existing test | Gate run | Modified-test gate fails; escalation
//! opened with trigger `test_modified`; PR cannot reach InReview". That is
//! three clauses with three owners, and this crate owns one of them.
//!
//! | Clause | Owner |
//! |---|---|
//! | "Modified-test gate fails" | this module |
//! | "escalation opened with trigger `test_modified`" | `ori-orchestrator`, which owns escalations |
//! | "PR cannot reach InReview" | `ori-core`, in its ticket machine, which already refuses it |
//!
//! The third is already built. `crates/ori-core/src/ticket.rs` carries a
//! `TestModification` enumeration with three values, `None`, `EscalationOpen`
//! and `EscalationMissing`, and its ticket machine refuses the move to
//! `InReview` on the third. This module produces what that machine consumes:
//! [`crate::modified_tests::Report::test_modification`] answers in those three values, and
//! [`crate::modified_tests::TestModification`] here is a restatement of that type rather than an
//! import, because `ori-gates` does not depend on `ori-core` and adding the
//! edge is outside this ticket's declared scope. `SectionsError::methodology_ref`
//! in `crates/ori-gates/src/sections.rs` returns text for the same reason. A
//! test below reads `crates/ori-core/src/ticket.rs` as text and fails if the
//! two spellings drift apart, so the restatement is held rather than trusted.
//!
//! Wiring this into `scripts/gates.sh` and `.github/workflows/ci.yml` is not
//! part of this ticket. What the wiring would be is recorded at the end of this
//! comment.
//!
//! # The shape of the detector
//!
//! ```mermaid
//! flowchart TB
//!   GIT[git diff of the merge base against the head] --> CH[FileChange: path, before, after]
//!   CH --> LEX[delimiter-aware projection: strings and comments blanked, depth per line]
//!   LEX --> IT[items of the test surface: #test functions, support items, doc-test fences]
//!   IT --> XC[cross-check: attribute count against collected count]
//!   XC --> CMP[pair before against after by name]
//!   CMP --> ADD[additive check on every pair that differs]
//!   ADD --> REP[Report: findings plus a census of what was read]
//!   REP --> TM[TestModification: None, EscalationOpen, EscalationMissing]
//! ```
//!
//! # Decision 1: where the input comes from
//!
//! Not from unified-diff hunks. A unified diff carries hunks and not files, so
//! a reader of hunks has to guess which function a changed line sits in, from
//! the hunk header's section heading, which git fills with a heuristic that
//! for Rust commonly names the enclosing `mod` rather than the enclosing
//! function. Guessing there is how a detector reports the wrong test, or none.
//!
//! What this module reads is a pair of whole file versions, [`crate::modified_tests::FileChange`],
//! which it parses independently and compares item by item. That is what makes
//! a rename visible: a renamed test is a name in the before version with no
//! counterpart in the after version, whatever the diff chose to call it.
//!
//! [`crate::modified_tests::scan`] is a pure function over those pairs, with no IO, so every case in
//! the table below is an input and not an environment.
//! [`crate::modified_tests::changes_from_git`] is the one part that runs a process: `git merge-base`,
//! `git diff --name-status` and `git show`. `crates/ori-gates/src/sections.rs`
//! and `crates/ori-gates/src/spec_refs.rs` read the tree with `std::fs` and
//! that is not enough here, because the before version of a changed file is
//! not on disk anywhere. `--no-renames` is passed on purpose: a renamed file
//! is shown as a deletion and an addition, so both sides are visible, and the
//! cross-file move check below is what keeps a pure move from being reported.
//!
//! # Decision 2: what counts as a test
//!
//! Four things, each with the reason it is in.
//!
//! 1. A function carrying an attribute whose path is `test` or ends `::test`.
//!    That covers the plain attribute and the runtime wrappers.
//! 2. Every item inside a module carrying `cfg(test)`, not only the test
//!    functions in it. A helper that supplies a test's inputs is part of the
//!    test: a function returning a list that a coverage test iterates weakens
//!    that test by returning less, without a line of the test body changing.
//!    AICD §14's word is "weakened", and this is what weakening looks like in
//!    this repository. The cost is stated under decision 5.
//! 3. Every item in a file under a `tests/` directory, which is an integration
//!    test target in full.
//! 4. A doc-test fence in a `///` or `//!` comment, compared as a whole.
//!
//! And one thing that is test surface without being Rust: a file under
//! `fixtures/planted/`. `spec/CONVENTIONS.md` "Tests" and `spec/TESTING.md`
//! section 4 make that directory the planted defects a gate is proved on, and
//! AICD §14 makes a gate installed only by that proof. Changing a planted
//! defect changes the proof of an installed gate. Those files are reported at
//! file level and never parsed, because several of them are deliberately
//! malformed and a parser is the wrong instrument for them.
//!
//! # Decision 3: what counts as modification
//!
//! Removal, rename, a changed signature, a changed attribute set, and any
//! change to the bytes of an existing body. Reformatting is included, and that
//! is a decision rather than an oversight: separating a reformatting from a
//! semantic change needs a parser that agrees with `rustc`, and every case such
//! a parser got wrong would be a silent pass. The strictness costs little,
//! because AICD §12 makes this a trigger and not a prohibition; a reformatting
//! escalation is answered in one line. What the report does instead of
//! guessing is say how deep the change went, in [`crate::modified_tests::Texture`], so the human
//! reading the escalation sees "whitespace only" and answers it in one line.
//!
//! One thing is deliberately not compared: a `//` or `///` comment written
//! above an item, outside its body. It cannot change what the test runs, and
//! doc-test fences, which can, are compared separately and by content.
//!
//! # Decision 4: the line on an added assertion, and where it is wrong
//!
//! Two merged tickets took a position on this before the detector existed.
//! ORI-T-0093 inserted three assertions into the body of an existing test in
//! `crates/ori-core/src/types.rs` (commit 39f7921) and inserted two more into
//! an existing test in `crates/ori-core/src/error.rs` (commit d66080e),
//! deleting no line of either body, and each recorded the judgement that a
//! pure insertion is additive rather than a modification.
//!
//! The judgement is right in its reason and incomplete in its test. The reason
//! is sound: AICD §14 exists because "passing tests by changing tests is the
//! most common way an agent hides a regression", and an insertion that leaves
//! every original assertion byte-identical cannot hide one, because every
//! original assertion still runs and still fails on everything it failed on
//! before. A rule that escalated on every strengthened test would fire on the
//! one change to a test that is unambiguously good, and a gate that fires on
//! good changes is a gate people learn to wave through.
//!
//! The test is incomplete because "every original line survives byte-identical"
//! does not imply "every original assertion still runs". Four insertions
//! satisfy it and neuter the test, and all four are refused here:
//!
//! | The insertion | Why the original assertions stop counting |
//! |---|---|
//! | `#[ignore]` above the function | Nothing in the body runs at all |
//! | `return;` near the top | Everything after it is unreachable |
//! | `for _ in 0..0 {` before the body and `}` after it | Every surviving line runs zero times |
//! | `let subject = 0;` above an assertion on `subject` | The assertion still runs and asserts nothing |
//!
//! So the rule implemented is a guarded one. An insertion is additive when all
//! of the following hold, and a modification otherwise:
//!
//! 1. The attribute set is unchanged, which refuses the first row.
//! 2. The signature line is unchanged.
//! 3. Every original body line survives byte-identical, in its original order,
//!    **and at its original delimiter depth**, which refuses the third row:
//!    wrapping a body in a loop either reindents it, which changes the bytes,
//!    or it does not, which changes the depth.
//! 4. No inserted line carries a control-flow escape: `return`, `break`,
//!    `continue` or `process::exit` anywhere; `?` where the signature has a
//!    return type; `panic!`, `unreachable!`, `todo!` or `unimplemented!` where
//!    the attributes say `should_panic`, because there such a line is a pass
//!    and not a failure. That refuses the second row.
//! 5. No inserted line binds, with `let` or with `for`, a name that a surviving
//!    original line mentions. That refuses the fourth row.
//!
//! What is still open, and stated rather than hidden: an insertion can mutate
//! state through a method call, `subject.clear()` above an assertion on
//! `subject`, and rule 5 does not see it because no name is bound. Closing that
//! needs to know which calls mutate, which needs types, which needs a compiler.
//! It is the residual hole in the additive rule and the reason the rule is
//! written as a narrow exception to "any change escalates" rather than as the
//! default.
//!
//! # Decision 5: what this gate would have said about the two merged tickets
//!
//! It would have passed commit 39f7921 and commit 6bb0236, and failed commit
//! d66080e. The first two are insertions of the kind rule 4 was written for.
//! The third inserted two assertions into a test body, which is additive, and
//! also replaced the two helpers that test iterates: `every_refusal` stopped
//! being a function and became a macro expansion, and `refusal_tag` was
//! rewritten. Under decision 2 those are test surface and their change is a
//! finding. That is the intended answer rather than a cost: the helper is what
//! supplies the coverage test its inputs, that commit's own message records the
//! old helper's doc comment as having claimed a guarantee it did not give, and
//! a human seeing the escalation is exactly the review AICD §14 asks for.
//!
//! # Decision 6: finding nothing is not passing
//!
//! AICD §14 names the defect class by name: "present but reporting nothing: a
//! checker that exits successfully on every input". A detector for this rule is
//! unusually exposed to it, because most pull requests legitimately produce no
//! finding, so a broken reader looks exactly like a clean tree.
//!
//! Three refusals guard it, and all three are failures rather than passes.
//!
//! - An empty change set is [`crate::modified_tests::ModifiedTestsError::NothingToCheck`]. A gate run
//!   over no file has checked nothing and must not report a pass.
//! - A file whose before version is not delimiter-balanced is
//!   [`crate::modified_tests::ModifiedTestsError::UnbalancedDelimiters`]. The projection failed, so
//!   every depth below it is meaningless.
//! - A file whose before version carries test attributes that the item walker
//!   did not turn into items is [`crate::modified_tests::ModifiedTestsError::TestCountMismatch`], and
//!   one under a `tests/` directory from which nothing at all was collected is
//!   [`crate::modified_tests::ModifiedTestsError::ReaderFoundNothing`]. The count is taken from the
//!   projection by a different route from the walker, so breaking the walker
//!   changes one number and not the other.
//!
//! [`crate::modified_tests::Census`] carries the rest: how many files were read, how many had test
//! surface, how many items and doc tests were compared. A report of no findings
//! over a census of zero items says so on its face.
//!
//! # This is a detector, and gate 5 is not installed
//!
//! `spec/LLD.md` section 2 puts "report a gate installed without a proof" in
//! this crate's must-not column, so this says the opposite in its own words.
//! `spec/TESTING.md` section 4: "Every gate in CI_CD ships with a planted
//! defect under `fixtures/planted/` and a proof recorded in `ops/gates/`. A
//! gate is not cited as protection in any document until its proof exists".
//! Neither exists for gate 5. This ticket's planted defects were run against a
//! copy of this tree outside the repository, which proves the detector and does
//! not install the gate, and this ticket's declared scope reaches neither
//! `fixtures/` nor `ops/`. Gate 5 is therefore Defined and not Installed, in
//! the vocabulary of criterion ORI-P1-012, `scripts/gates.sh` still reports it
//! as having no runner, and no document may cite it as protection. Three
//! things are outstanding: `fixtures/planted/gate-5/`, `ops/gates/gate-5.md`
//! and the wiring below.
//!
//! # One place where this narrows the specification, said out loud
//!
//! `spec/PRD.md` V-04 reads "any change to an existing test escalates whatever
//! the tier", and `spec/CI_CD.md` section 1 item 5 reads "escalation label on
//! any changed or removed existing test". Read to the letter, an assertion
//! added inside an existing test body is a change to an existing test, and both
//! sentences would escalate it. Decision 4 does not, under five guards.
//!
//! The narrowing is deliberate and it is a question for a human rather than a
//! reading this module is entitled to settle. What is on its side: AICD §14 is
//! the governing text and its three verbs are "modified, weakened or removed",
//! with the stated reason that "passing tests by changing tests is the most
//! common way an agent hides a regression"; an insertion that leaves every
//! original assertion running hides nothing; and two merged tickets have
//! already relied on that reading, so the letter of V-04 would have failed
//! ORI-T-0093 and ORI-T-0098 retroactively. What is against it: the two
//! sentences say what they say. Nothing here edits them. If a human prefers the
//! letter, decision 4's exception is one function, and deleting it makes this
//! module escalate on every insertion.
//!
//! # What the wiring would be, when a ticket is written for it
//!
//! In `scripts/gates.sh`, `gate_5` would stop calling `probe_unavailable` and
//! would run a binary or a `cargo test` target in this crate over
//! `git merge-base HEAD main` and `HEAD`, mapping the four outcomes onto the
//! script's five states: a report with no findings and a non-empty census is
//! `passed`; a report with findings is `failed`; every
//! [`crate::modified_tests::ModifiedTestsError`] is `blocked`, never `passed`, because nothing was
//! checked. In `.github/workflows/ci.yml` the job would need `fetch-depth: 0`
//! on its checkout for the same reason the `gate-13` job does: the merge base
//! does not exist in a shallow clone, and a gate that cannot name its range
//! must block rather than pass. Neither file is touched by this ticket.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// The input
// ---------------------------------------------------------------------------

/// One file as the pull request found it and as it left it: AICD §14.
///
/// Derived from AICD §14's subject, "a pull request in which an existing test
/// was modified": the unit of that sentence is a file with two versions, so
/// that is the unit the detector takes. `before` is `None` for a file the
/// change added and `after` is `None` for one it deleted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChange {
    /// The path relative to the repository root, with forward slashes.
    pub path: String,
    /// The file as it was at the merge base, or `None` if it did not exist.
    pub before: Option<String>,
    /// The file as it is at the head, or `None` if the change deleted it.
    pub after: Option<String>,
}

impl FileChange {
    /// A file the change modified: AICD §14, the ordinary case of its rule.
    #[must_use]
    pub fn modified(path: &str, before: &str, after: &str) -> Self {
        Self {
            path: path.to_owned(),
            before: Some(before.to_owned()),
            after: Some(after.to_owned()),
        }
    }

    /// A file the change added, which has no before version: AICD §14.
    #[must_use]
    pub fn added(path: &str, after: &str) -> Self {
        Self {
            path: path.to_owned(),
            before: None,
            after: Some(after.to_owned()),
        }
    }

    /// A file the change deleted, which has no after version: AICD §14.
    #[must_use]
    pub fn deleted(path: &str, before: &str) -> Self {
        Self {
            path: path.to_owned(),
            before: Some(before.to_owned()),
            after: None,
        }
    }
}

// ---------------------------------------------------------------------------
// The output
// ---------------------------------------------------------------------------

/// What kind of test surface an item is: AICD §14, decision 2 of this module.
///
/// Derived from AICD §14's list of what verification is made of, narrowed to
/// the forms this repository writes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Surface {
    /// A function carrying an attribute whose path is `test` or ends `::test`.
    Test,
    /// An item inside a `cfg(test)` module or a `tests/` file that is not
    /// itself a test function: a helper, a fixture, a macro, a `use`.
    Support,
    /// A code fence in a doc comment that rustdoc would run.
    DocTest,
    /// A file under `fixtures/planted/`, which is a gate's proof.
    PlantedFixture,
}

impl fmt::Display for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Test => "test",
            Self::Support => "test support item",
            Self::DocTest => "doc test",
            Self::PlantedFixture => "planted fixture",
        })
    }
}

/// How deep a change to a body went: AICD §14, decision 3 of this module.
///
/// No methodology section grades a modification, so this grades nothing: every
/// value is a finding and escalates. It exists so that the human answering the
/// escalation can see in the report which of them they are answering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Texture {
    /// The bodies are the same once every run of whitespace is collapsed. A
    /// reformatting is this.
    Whitespace,
    /// The bodies differ only in comments or in string literals, which the
    /// projection blanks. An assertion's expected string is in this bucket and
    /// is not cosmetic, which is why the name says text and not cosmetic.
    TextOnly,
    /// The bodies differ in code.
    Code,
}

impl fmt::Display for Texture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Whitespace => "whitespace only",
            Self::TextOnly => "comments or string literals only",
            Self::Code => "code",
        })
    }
}

/// One existing test the pull request did not leave alone: AICD §14.
///
/// Derived from AICD §14's three verbs, "modified, weakened or removed". Each
/// variant is one way this repository's tests can take one of them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Finding {
    /// An item present before and absent after. A rename lands here, with the
    /// item that carries its body named in `replaced_by`.
    Removed {
        /// The file it was in.
        path: String,
        /// Its name before.
        name: String,
        /// What it was.
        surface: Surface,
        /// An item added to the same file whose body is the removed one's,
        /// which is what a rename looks like from here.
        replaced_by: Option<String>,
    },
    /// An item whose attribute set changed. An added `ignore` is this.
    AttributesChanged {
        /// The file it is in.
        path: String,
        /// Its name.
        name: String,
        /// What it is.
        surface: Surface,
        /// The attributes before, joined.
        before: String,
        /// The attributes after, joined.
        after: String,
    },
    /// An item whose body changed in a way the additive rule does not admit.
    BodyChanged {
        /// The file it is in.
        path: String,
        /// Its name.
        name: String,
        /// What it is.
        surface: Surface,
        /// How deep the change went.
        texture: Texture,
        /// Which clause of the additive rule refused it.
        refused_by: String,
    },
    /// A doc-test fence present before and absent after.
    DocTestChanged {
        /// The file it is in.
        path: String,
        /// The fence's first non-empty line, which is how a reader finds it.
        first_line: String,
    },
    /// A file under `fixtures/planted/` whose bytes changed.
    PlantedFixtureChanged {
        /// The file.
        path: String,
    },
}

impl Finding {
    /// The file the finding is about: AICD §14, so a report can be read.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Removed { path, .. }
            | Self::AttributesChanged { path, .. }
            | Self::BodyChanged { path, .. }
            | Self::DocTestChanged { path, .. }
            | Self::PlantedFixtureChanged { path } => path,
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Removed {
                path,
                name,
                surface,
                replaced_by: Some(new_name),
            } => write!(
                f,
                "{path}: the {surface} {name} is gone, and {new_name} was added with its body, \
                 so it was renamed. A rename is a deletion and an addition and the two halves \
                 are not each other's excuse"
            ),
            Self::Removed {
                path,
                name,
                surface,
                replaced_by: None,
            } => write!(f, "{path}: the {surface} {name} was removed"),
            Self::AttributesChanged {
                path,
                name,
                surface,
                before,
                after,
            } => write!(
                f,
                "{path}: the attributes of the {surface} {name} changed from [{before}] to \
                 [{after}]"
            ),
            Self::BodyChanged {
                path,
                name,
                surface,
                texture,
                refused_by,
            } => write!(
                f,
                "{path}: the body of the {surface} {name} changed ({texture}); it is not an \
                 additive insertion because {refused_by}"
            ),
            Self::DocTestChanged { path, first_line } => write!(
                f,
                "{path}: a doc test was removed or changed; its first line was {first_line}"
            ),
            Self::PlantedFixtureChanged { path } => write!(
                f,
                "{path}: a planted defect changed, and a planted defect is the proof that an \
                 installed gate has been seen to fail (AICD §14)"
            ),
        }
    }
}

/// What the detector read, so that reading nothing cannot look like a pass.
///
/// Derived from AICD §14's named defect class, "present but reporting nothing".
/// A report of no findings means nothing until this says how much was looked
/// at.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Census {
    /// Files in the change set.
    pub files: usize,
    /// Files whose before version carried any test surface.
    pub files_with_test_surface: usize,
    /// Items collected from the before versions.
    pub items_before: usize,
    /// Items collected from the after versions.
    pub items_after: usize,
    /// Test functions among the items collected from the before versions.
    pub tests_before: usize,
    /// Doc-test fences collected from the before versions.
    pub doc_tests_before: usize,
    /// Items that survived byte-identical.
    pub unchanged: usize,
    /// Items an additive insertion grew, which are not findings.
    pub additive: usize,
    /// Items the change added, which are not findings.
    pub added: usize,
    /// Items that moved between files with their name and body intact, which
    /// are not findings.
    pub moved: usize,
}

/// Whether an escalation is open for this pull request: AICD §12.
///
/// Derived from AICD §12's escalation protocol. The detector cannot know this;
/// it is handed in so that [`Report::test_modification`] can answer in the
/// three values the ticket machine consumes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Escalation {
    /// An escalation with trigger `test_modified` is open.
    Open,
    /// No escalation is open.
    Absent,
}

/// What a pull request did to the tests that already existed: AICD §12.
///
/// A restatement of `TestModification` in `crates/ori-core/src/ticket.rs`,
/// which the ticket machine reads to decide whether a ticket may enter
/// `InReview`. It is restated and not imported because `ori-gates` does not
/// depend on `ori-core`. A test below reads that file as text and fails if the
/// three spellings drift apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestModification {
    /// The pull request modified no test that already existed.
    None,
    /// It modified one, and an escalation is open for it.
    EscalationOpen,
    /// It modified one, and no escalation is open.
    EscalationMissing,
}

impl TestModification {
    /// The three spellings, in the order the ticket machine declares them.
    ///
    /// Derived from nothing in the methodology: it exists so that the
    /// restatement can be compared with its original mechanically, which is
    /// AICD §39's rule applied to a type name.
    pub const SPELLINGS: [&'static str; 3] = ["None", "EscalationOpen", "EscalationMissing"];
}

/// What a gate run found: AICD §14.
///
/// Derived from AICD §14's rule about existing tests, together with its rule
/// that a gate must be able to be seen to fail: the findings are the failure
/// and the census is what makes an absence of findings mean anything.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    findings: Vec<Finding>,
    census: Census,
}

impl Report {
    /// Every existing test the pull request did not leave alone: AICD §14.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// What the detector read: AICD §14's "present but reporting nothing".
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// Whether any existing test was modified, weakened or removed: AICD §14.
    #[must_use]
    pub fn modified_an_existing_test(&self) -> bool {
        !self.findings.is_empty()
    }

    /// The value the ticket machine of `ori-core` consumes: AICD §12.
    ///
    /// Derived from AICD §12's escalation trigger and from the invariant of
    /// `spec/DATA_MODEL.md` section 3 that the ticket machine enforces, "a
    /// ticket whose PR modified an existing test has an open `Escalation`
    /// before `InReview` can proceed".
    #[must_use]
    pub fn test_modification(&self, escalation: Escalation) -> TestModification {
        if !self.modified_an_existing_test() {
            return TestModification::None;
        }
        match escalation {
            Escalation::Open => TestModification::EscalationOpen,
            Escalation::Absent => TestModification::EscalationMissing,
        }
    }

    /// The report as the lines a gate run prints: AICD §14.
    ///
    /// Every line says what it is about. A run that found nothing says what it
    /// read, because AICD §14 forbids a silent pass more than it forbids a
    /// noisy one.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        if self.findings.is_empty() {
            return vec![format!(
                "no existing test was modified: {} item(s) and {} doc test(s) compared across {} \
                 of {} changed file(s), {} unchanged, {} grown by an additive insertion, {} \
                 added, {} moved",
                self.census.items_before,
                self.census.doc_tests_before,
                self.census.files_with_test_surface,
                self.census.files,
                self.census.unchanged,
                self.census.additive,
                self.census.added,
                self.census.moved,
            )];
        }
        let mut out = vec![format!(
            "{} existing test(s) were modified, weakened or removed. AICD §14 requires an \
             escalation with trigger test_modified, whatever the tier",
            self.findings.len()
        )];
        out.extend(self.findings.iter().map(ToString::to_string));
        out
    }
}

// ---------------------------------------------------------------------------
// The refusals
// ---------------------------------------------------------------------------

/// Why a gate run checked nothing: AICD §14.
///
/// Every variant is a refusal and never a pass. AICD §14's recurring defect
/// class is a checker that exits successfully on every input, and each of these
/// is a way this one could have done that.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModifiedTestsError {
    /// The change set is empty, so no file was compared.
    NothingToCheck,
    /// A file's before version is not delimiter-balanced, so the projection
    /// that every depth below rests on failed.
    UnbalancedDelimiters {
        /// The file.
        path: String,
        /// The delimiter depth left at the end of it.
        depth: isize,
    },
    /// A file's before version carries test attributes the item walker did not
    /// turn into items.
    TestCountMismatch {
        /// The file.
        path: String,
        /// Test attributes counted in the projection.
        attributes: usize,
        /// Test functions the walker collected.
        collected: usize,
    },
    /// Nothing at all was collected from a file that is test surface by its
    /// path.
    ReaderFoundNothing {
        /// The file.
        path: String,
    },
    /// A git command failed, so the change set could not be determined.
    Git {
        /// The command, as it was run.
        command: String,
        /// What it said.
        detail: String,
    },
}

impl ModifiedTestsError {
    /// The methodology section this refusal rests on: `CLAUDE.md` rule 9.
    ///
    /// Every variant answers AICD §14, which is the section that makes a gate
    /// that cannot read its input fail loudly instead of passing quietly.
    #[must_use]
    pub const fn methodology_ref(&self) -> &'static str {
        "AICD §14"
    }
}

impl fmt::Display for ModifiedTestsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToCheck => f.write_str(
                "the change set is empty, so this run compared no file and checked no test. That \
                 is not a pass",
            ),
            Self::UnbalancedDelimiters { path, depth } => write!(
                f,
                "{path}: the before version ends at delimiter depth {depth} rather than 0, so \
                 the projection is wrong and every item boundary taken from it is wrong with it"
            ),
            Self::TestCountMismatch {
                path,
                attributes,
                collected,
            } => write!(
                f,
                "{path}: the projection counts {attributes} test attribute(s) in the before \
                 version and the item walker collected {collected} test function(s). The walker \
                 is broken, and a broken walker finds no modification in any input"
            ),
            Self::ReaderFoundNothing { path } => write!(
                f,
                "{path}: this file is test surface by its path and the reader collected no item \
                 from its before version, so it would report nothing about any change to it"
            ),
            Self::Git { command, detail } => write!(
                f,
                "the change set could not be determined: `{command}` failed: {detail}"
            ),
        }
    }
}

impl std::error::Error for ModifiedTestsError {}

// ---------------------------------------------------------------------------
// The projection: one pass over a file, delimiter-aware
// ---------------------------------------------------------------------------

/// One source line, with strings and comments blanked and its depth recorded.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CodeLine {
    /// The line exactly as written, without its newline.
    raw: String,
    /// The line with comment text removed and string contents blanked, which
    /// is what every structural question below is asked of.
    code: String,
    /// The delimiter depth at the start of the line. Braces, parentheses and
    /// brackets all count: a signature or a macro call wrapped across lines
    /// moves this the same way a block does, which is what decision 4's third
    /// clause needs.
    depth: usize,
    /// The text after `///` or `//!`, when the line is a doc comment.
    doc: Option<String>,
}

#[derive(Clone, Copy)]
enum Lex {
    Normal,
    Block(usize),
    Str,
    RawStr(usize),
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn char_literal_len(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at + 1) == Some(&'\\') {
        let mut j = at + 2;
        while j < chars.len() && j < at + 12 {
            if chars[j] == '\'' {
                return Some(j - at + 1);
            }
            j += 1;
        }
        return None;
    }
    if chars.get(at + 2) == Some(&'\'') && chars.get(at + 1).is_some() {
        return Some(3);
    }
    None
}

fn raw_string_hashes(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'r') {
        return None;
    }
    if at > 0 && is_ident_char(chars[at - 1]) && chars[at - 1] != 'b' {
        return None;
    }
    let mut hashes = 0usize;
    let mut j = at + 1;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) == Some(&'"') {
        Some(hashes)
    } else {
        None
    }
}

/// Project a source file into lines whose code is separable from their text.
///
/// Returns the lines and the delimiter depth left at the end, which is 0 for a
/// file that parses and is the first thing the caller refuses on.
fn project(source: &str) -> (Vec<CodeLine>, isize) {
    let mut out: Vec<CodeLine> = Vec::new();
    let mut state = Lex::Normal;
    let mut depth: isize = 0;

    for raw in source.lines() {
        let start_depth = usize::try_from(depth.max(0)).unwrap_or(0);
        let chars: Vec<char> = raw.chars().collect();
        let mut code = String::with_capacity(chars.len());
        let mut doc: Option<String> = None;
        let mut i = 0usize;

        while i < chars.len() {
            match state {
                Lex::Normal => {
                    let c = chars[i];
                    if c == '/' && chars.get(i + 1) == Some(&'/') {
                        let rest: String = chars[i..].iter().collect();
                        let is_doc = (rest.starts_with("///") && !rest.starts_with("////"))
                            || rest.starts_with("//!");
                        if is_doc {
                            doc = Some(rest.chars().skip(3).collect());
                        }
                        i = chars.len();
                    } else if c == '/' && chars.get(i + 1) == Some(&'*') {
                        state = Lex::Block(1);
                        i += 2;
                    } else if let Some(hashes) = raw_string_hashes(&chars, i) {
                        state = Lex::RawStr(hashes);
                        code.push('"');
                        i += hashes + 2;
                    } else if c == '"' {
                        state = Lex::Str;
                        code.push('"');
                        i += 1;
                    } else if c == '\'' {
                        if let Some(len) = char_literal_len(&chars, i) {
                            code.push_str("''");
                            i += len;
                        } else {
                            code.push('\'');
                            i += 1;
                        }
                    } else {
                        if matches!(c, '{' | '(' | '[') {
                            depth += 1;
                        } else if matches!(c, '}' | ')' | ']') {
                            depth -= 1;
                        }
                        code.push(c);
                        i += 1;
                    }
                }
                Lex::Str => {
                    let mut j = i;
                    let mut closed = false;
                    while j < chars.len() {
                        if chars[j] == '\\' {
                            j += 2;
                            continue;
                        }
                        if chars[j] == '"' {
                            j += 1;
                            closed = true;
                            break;
                        }
                        j += 1;
                    }
                    if closed {
                        code.push('"');
                        state = Lex::Normal;
                    }
                    i = j.min(chars.len());
                }
                Lex::RawStr(hashes) => {
                    let mut j = i;
                    let mut closed = false;
                    while j < chars.len() {
                        if chars[j] == '"'
                            && (0..hashes).all(|k| chars.get(j + 1 + k) == Some(&'#'))
                        {
                            j += 1 + hashes;
                            closed = true;
                            break;
                        }
                        j += 1;
                    }
                    if closed {
                        code.push('"');
                        state = Lex::Normal;
                    }
                    i = j.min(chars.len());
                }
                Lex::Block(open) => {
                    let mut j = i;
                    let mut nesting = open;
                    while j < chars.len() {
                        if chars[j] == '*' && chars.get(j + 1) == Some(&'/') {
                            nesting -= 1;
                            j += 2;
                            if nesting == 0 {
                                break;
                            }
                            continue;
                        }
                        if chars[j] == '/' && chars.get(j + 1) == Some(&'*') {
                            nesting += 1;
                            j += 2;
                            continue;
                        }
                        j += 1;
                    }
                    state = if nesting == 0 {
                        Lex::Normal
                    } else {
                        Lex::Block(nesting)
                    };
                    i = j.min(chars.len());
                }
            }
        }

        out.push(CodeLine {
            raw: raw.to_owned(),
            code,
            depth: start_depth,
            doc,
        });
    }

    (out, depth)
}

// ---------------------------------------------------------------------------
// The item walker
// ---------------------------------------------------------------------------

/// One item of the test surface, with everything the comparison asks of it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Item {
    name: String,
    surface: Surface,
    attributes: Vec<String>,
    /// Raw lines, the head line first.
    body: Vec<String>,
    /// The projected lines, in step with `body`.
    code: Vec<String>,
    /// Delimiter depth relative to the head line, in step with `body`.
    depths: Vec<usize>,
}

impl Item {
    fn signature(&self) -> &str {
        self.body.first().map_or("", String::as_str)
    }

    /// The body without its head line, which is what a rename leaves intact.
    fn tail(&self) -> &[String] {
        self.body.get(1..).unwrap_or(&[])
    }
}

const ATTRIBUTE_KEYWORDS: [&str; 10] = [
    "fn",
    "mod",
    "struct",
    "enum",
    "union",
    "trait",
    "const",
    "static",
    "type",
    "macro_rules!",
];

fn attribute_path(attribute: &str) -> String {
    let inner = attribute
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('!')
        .trim_start_matches('[')
        .trim_end_matches(']');
    inner
        .split(['(', ' ', ','])
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn is_test_attribute(attribute: &str) -> bool {
    let path = attribute_path(attribute);
    path == "test" || path.ends_with("::test")
}

fn is_cfg_test_attribute(attribute: &str) -> bool {
    if attribute_path(attribute) != "cfg" {
        return false;
    }
    let squeezed: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    squeezed
        .split(|c: char| !is_ident_char(c))
        .any(|word| word == "test")
}

fn read_attribute(lines: &[CodeLine], from: usize, end: usize) -> (String, usize) {
    let mut text = String::new();
    let mut open: isize = 0;
    let mut j = from;
    while j < end {
        for ch in lines[j].code.chars() {
            if ch == '[' {
                open += 1;
            } else if ch == ']' {
                open -= 1;
            }
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(lines[j].raw.trim());
        j += 1;
        if open <= 0 {
            break;
        }
    }
    (text, j)
}

fn item_end_index(lines: &[CodeLine], head: usize, end: usize, depth: usize) -> usize {
    let mut j = head;
    while j < end {
        let after = lines.get(j + 1).map_or(depth, |line| line.depth);
        if after <= depth {
            return j;
        }
        j += 1;
    }
    end.saturating_sub(1).max(head)
}

fn declared_name(head: &str) -> Option<(&'static str, String)> {
    let words: Vec<&str> = head
        .split(|c: char| !(is_ident_char(c) || c == '!'))
        .filter(|word| !word.is_empty())
        .collect();
    for (index, word) in words.iter().enumerate() {
        if let Some(keyword) = ATTRIBUTE_KEYWORDS.iter().find(|k| *k == word)
            && let Some(name) = words.get(index + 1)
        {
            return Some((keyword, (*name).to_owned()));
        }
    }
    None
}

fn truncated(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= 80 {
        return trimmed.to_owned();
    }
    trimmed.chars().take(80).collect()
}

/// Walk a range of lines at one delimiter depth and collect its test surface.
///
/// `in_test` is true inside a `cfg(test)` module or a `tests/` file, which is
/// what makes a plain helper test surface.
fn collect_items(
    lines: &[CodeLine],
    range: (usize, usize),
    depth: usize,
    prefix: &str,
    in_test: bool,
    out: &mut Vec<Item>,
) {
    let (start, end) = range;
    let mut i = start;
    let mut attributes: Vec<String> = Vec::new();

    while i < end {
        let line = &lines[i];
        let trimmed = line.code.trim();
        if trimmed.is_empty() || line.depth != depth {
            i += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            let (text, next) = read_attribute(lines, i, end);
            attributes.push(text);
            i = next;
            continue;
        }

        let stop = item_end_index(lines, i, end, depth);
        let declaration = declared_name(trimmed);
        let is_test = attributes.iter().any(|a| is_test_attribute(a));
        let opens_a_block = stop > i;
        let recurse = matches!(
            declaration.as_ref().map(|(keyword, _)| *keyword),
            Some("mod")
        ) || (trimmed.starts_with("impl") && opens_a_block);

        let name = match &declaration {
            Some((_, name)) => format!("{prefix}{name}"),
            None => format!("{prefix}{}", truncated(trimmed)),
        };

        let nested_in_test = in_test || attributes.iter().any(|a| is_cfg_test_attribute(a));

        if in_test || is_test {
            // A `mod` or an `impl` contributes its head line only. Its children
            // are collected by the recursion below, and a whole-block body here
            // would report every one of them twice.
            let last = if recurse { i } else { stop };
            let head_depth = lines[i].depth;
            let mut body = Vec::new();
            let mut code = Vec::new();
            let mut depths = Vec::new();
            for line in &lines[i..=last.min(lines.len() - 1)] {
                body.push(line.raw.clone());
                code.push(line.code.clone());
                depths.push(line.depth.saturating_sub(head_depth));
            }
            out.push(Item {
                name: name.clone(),
                surface: if is_test {
                    Surface::Test
                } else {
                    Surface::Support
                },
                attributes: attributes.clone(),
                body,
                code,
                depths,
            });
        }

        if recurse && stop > i {
            collect_items(
                lines,
                (i + 1, stop),
                depth + 1,
                &format!("{name}::"),
                nested_in_test,
                out,
            );
        }

        attributes.clear();
        i = stop + 1;
    }
}

/// Count the test attributes the projection can see, by a route the item walker
/// does not touch. This is the number the walker's output is checked against.
fn count_test_attributes(lines: &[CodeLine]) -> usize {
    lines
        .iter()
        .filter(|line| {
            let trimmed = line.code.trim();
            trimmed.starts_with('#') && is_test_attribute(trimmed)
        })
        .count()
}

// ---------------------------------------------------------------------------
// Doc tests
// ---------------------------------------------------------------------------

const RUNNABLE_FENCE_WORDS: [&str; 9] = [
    "rust",
    "should_panic",
    "no_run",
    "compile_fail",
    "edition2015",
    "edition2018",
    "edition2021",
    "edition2024",
    "",
];

fn fence_is_runnable(info: &str) -> bool {
    let info = info.trim();
    if info.is_empty() {
        return true;
    }
    info.split(',')
        .all(|word| RUNNABLE_FENCE_WORDS.contains(&word.trim()))
}

/// Every doc-test fence in a file, as the text between its delimiters.
///
/// Keyed by content and never by position: a fence that changed is a fence
/// present before and absent after, which is the same shape as a removal and
/// gets the same answer.
fn doc_tests(lines: &[CodeLine]) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<Vec<String>> = None;
    for line in lines {
        let Some(text) = &line.doc else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.starts_with("```") {
            match open.take() {
                Some(collected) => out.push(collected.join("\n")),
                None => {
                    if fence_is_runnable(trimmed.trim_start_matches('`')) {
                        open = Some(Vec::new());
                    }
                }
            }
            continue;
        }
        if let Some(collected) = open.as_mut() {
            collected.push(text.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The additive rule
// ---------------------------------------------------------------------------

const ESCAPE_TOKENS: [&str; 5] = [
    "return",
    "break",
    "continue",
    "process::exit",
    "std::process::exit",
];

const PANIC_TOKENS: [&str; 4] = ["panic!", "unreachable!", "todo!", "unimplemented!"];

fn contains_token(haystack: &str, needle: &str) -> bool {
    let chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    if needle_chars.is_empty() || needle_chars.len() > chars.len() {
        return false;
    }
    for start in 0..=chars.len() - needle_chars.len() {
        if chars[start..start + needle_chars.len()] != needle_chars[..] {
            continue;
        }
        let before_ok = start == 0 || !is_ident_char(chars[start - 1]);
        let end = start + needle_chars.len();
        let after_ok = end == chars.len() || !is_ident_char(chars[end]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Names an inserted line binds with `let` or with `for`.
fn bound_names(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let is_let = contains_token_at(&chars, i, "let");
        let is_for = contains_token_at(&chars, i, "for");
        if !is_let && !is_for {
            i += 1;
            continue;
        }
        let mut j = i + 3;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if contains_token_at(&chars, j, "mut") {
            j += 3;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
        }
        if chars.get(j) == Some(&'(') {
            let mut current = String::new();
            j += 1;
            while j < chars.len() && chars[j] != ')' {
                if is_ident_char(chars[j]) {
                    current.push(chars[j]);
                } else if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                j += 1;
            }
            if !current.is_empty() {
                out.push(current);
            }
        } else {
            let mut current = String::new();
            while j < chars.len() && is_ident_char(chars[j]) {
                current.push(chars[j]);
                j += 1;
            }
            if !current.is_empty() {
                out.push(current);
            }
        }
        i = j.max(i + 1);
    }
    out.retain(|name| name != "mut" && name != "_");
    out
}

fn contains_token_at(chars: &[char], at: usize, needle: &str) -> bool {
    let needle_chars: Vec<char> = needle.chars().collect();
    if at + needle_chars.len() > chars.len() {
        return false;
    }
    if chars[at..at + needle_chars.len()] != needle_chars[..] {
        return false;
    }
    let before_ok = at == 0 || !is_ident_char(chars[at - 1]);
    let end = at + needle_chars.len();
    let after_ok = end == chars.len() || !is_ident_char(chars[end]);
    before_ok && after_ok
}

/// Decide whether the change from `before` to `after` is a pure insertion that
/// leaves every original assertion running. `Ok` means additive, `Err` names
/// the clause of decision 4 that refused it.
fn additive(before: &Item, after: &Item) -> Result<usize, String> {
    if before.attributes != after.attributes {
        return Err(format!(
            "its attribute set changed from [{}] to [{}]",
            before.attributes.join(" "),
            after.attributes.join(" ")
        ));
    }
    if before.signature() != after.signature() {
        return Err("its signature line changed".to_owned());
    }

    let mut cursor = 0usize;
    let mut inserted: Vec<usize> = Vec::new();
    for index in 0..after.body.len() {
        let matches_original = cursor < before.body.len()
            && before.body[cursor] == after.body[index]
            && before.depths[cursor] == after.depths[index];
        if matches_original {
            cursor += 1;
        } else {
            inserted.push(index);
        }
    }
    if cursor != before.body.len() {
        return Err(format!(
            "{} original line(s) did not survive byte-identical at their original delimiter \
             depth, so the change is not an insertion",
            before.body.len() - cursor
        ));
    }
    if inserted.is_empty() {
        return Ok(0);
    }

    let returns_a_value = before.signature().contains("->");
    let should_panic = before
        .attributes
        .iter()
        .any(|attribute| attribute.contains("should_panic"));
    let surviving: String = before.code.join("\n");

    for index in &inserted {
        let code = &after.code[*index];
        for token in ESCAPE_TOKENS {
            if contains_token(code, token) {
                return Err(format!(
                    "an inserted line carries `{token}`, which can stop the original assertions \
                     running: {}",
                    truncated(&after.body[*index])
                ));
            }
        }
        if returns_a_value && code.contains('?') {
            return Err(format!(
                "an inserted line carries `?` in a test that returns a value, which can stop the \
                 original assertions running: {}",
                truncated(&after.body[*index])
            ));
        }
        if should_panic {
            for token in PANIC_TOKENS {
                if contains_token(code, token) {
                    return Err(format!(
                        "an inserted line carries `{token}` in a should_panic test, where that is \
                         a pass and not a failure: {}",
                        truncated(&after.body[*index])
                    ));
                }
            }
        }
        for name in bound_names(code) {
            if contains_token(&surviving, &name) {
                return Err(format!(
                    "an inserted line binds `{name}`, a name the original body already mentions, \
                     so a surviving assertion may now be reading the inserted value: {}",
                    truncated(&after.body[*index])
                ));
            }
        }
    }

    Ok(inserted.len())
}

/// Split a body into tokens: runs of identifier characters, and every other
/// non-whitespace character on its own.
///
/// Collapsing runs of whitespace is not enough to see a reformatting for what
/// it is, because `rustfmt` moves whitespace between tokens that had none:
/// `assert_eq!(total,` and `assert_eq!(\n    total,` collapse to two different
/// strings and tokenize to the same sequence.
fn tokenize(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for character in lines.join("\n").chars() {
        if is_ident_char(character) {
            current.push(character);
            continue;
        }
        if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if !character.is_whitespace() {
            out.push(character.to_string());
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn texture(before: &Item, after: &Item) -> Texture {
    if tokenize(&before.body) == tokenize(&after.body) {
        return Texture::Whitespace;
    }
    if tokenize(&before.code) == tokenize(&after.code) {
        return Texture::TextOnly;
    }
    Texture::Code
}

// ---------------------------------------------------------------------------
// The scan
// ---------------------------------------------------------------------------

fn is_whole_file_test(path: &str) -> bool {
    path.starts_with("tests/") || path.contains("/tests/")
}

fn is_planted_fixture(path: &str) -> bool {
    path.starts_with("fixtures/planted/")
}

fn is_rust(path: &str) -> bool {
    path.ends_with(".rs")
}

fn read_surface(path: &str, source: &str) -> Result<(Vec<Item>, Vec<String>), ModifiedTestsError> {
    let (lines, depth) = project(source);
    if depth != 0 {
        return Err(ModifiedTestsError::UnbalancedDelimiters {
            path: path.to_owned(),
            depth,
        });
    }
    let mut items = Vec::new();
    collect_items(
        &lines,
        (0, lines.len()),
        0,
        "",
        is_whole_file_test(path),
        &mut items,
    );
    Ok((items, doc_tests(&lines)))
}

fn check_reader(path: &str, source: &str, items: &[Item]) -> Result<(), ModifiedTestsError> {
    let (lines, _) = project(source);
    let attributes = count_test_attributes(&lines);
    let collected = items
        .iter()
        .filter(|item| item.surface == Surface::Test)
        .count();
    if attributes != collected {
        return Err(ModifiedTestsError::TestCountMismatch {
            path: path.to_owned(),
            attributes,
            collected,
        });
    }
    if is_whole_file_test(path)
        && items.is_empty()
        && lines.iter().any(|line| !line.code.trim().is_empty())
    {
        return Err(ModifiedTestsError::ReaderFoundNothing {
            path: path.to_owned(),
        });
    }
    Ok(())
}

struct FileScan {
    findings: Vec<Finding>,
    removed: Vec<Item>,
    added: Vec<Item>,
}

fn compare_file(change: &FileChange, census: &mut Census) -> Result<FileScan, ModifiedTestsError> {
    let path = change.path.as_str();
    let mut findings = Vec::new();

    if is_planted_fixture(path) {
        if change.before.is_some() && change.before != change.after {
            findings.push(Finding::PlantedFixtureChanged {
                path: path.to_owned(),
            });
            census.files_with_test_surface += 1;
        }
        return Ok(FileScan {
            findings,
            removed: Vec::new(),
            added: Vec::new(),
        });
    }

    if !is_rust(path) {
        return Ok(FileScan {
            findings,
            removed: Vec::new(),
            added: Vec::new(),
        });
    }

    let (before_items, before_fences) = match &change.before {
        Some(source) => {
            let (items, fences) = read_surface(path, source)?;
            check_reader(path, source, &items)?;
            (items, fences)
        }
        None => (Vec::new(), Vec::new()),
    };
    let (after_items, after_fences) = match &change.after {
        Some(source) => read_surface(path, source)?,
        None => (Vec::new(), Vec::new()),
    };

    census.items_before += before_items.len();
    census.items_after += after_items.len();
    census.tests_before += before_items
        .iter()
        .filter(|item| item.surface == Surface::Test)
        .count();
    census.doc_tests_before += before_fences.len();
    if !before_items.is_empty() || !before_fences.is_empty() {
        census.files_with_test_surface += 1;
    }

    let mut used = vec![false; after_items.len()];
    let mut removed: Vec<Item> = Vec::new();

    for before in &before_items {
        let paired = after_items.iter().enumerate().position(|(index, after)| {
            !used[index] && after.name == before.name && after.surface == before.surface
        });
        let Some(index) = paired else {
            removed.push(before.clone());
            continue;
        };
        used[index] = true;
        let after = &after_items[index];
        if before.attributes == after.attributes && before.body == after.body {
            census.unchanged += 1;
            continue;
        }
        if before.attributes != after.attributes {
            findings.push(Finding::AttributesChanged {
                path: path.to_owned(),
                name: before.name.clone(),
                surface: before.surface,
                before: before.attributes.join(" "),
                after: after.attributes.join(" "),
            });
            continue;
        }
        match additive(before, after) {
            Ok(_) => census.additive += 1,
            Err(refused_by) => findings.push(Finding::BodyChanged {
                path: path.to_owned(),
                name: before.name.clone(),
                surface: before.surface,
                texture: texture(before, after),
                refused_by,
            }),
        }
    }

    let added: Vec<Item> = after_items
        .iter()
        .enumerate()
        .filter(|(index, _)| !used[*index])
        .map(|(_, item)| item.clone())
        .collect();
    census.added += added.len();

    let after_set: BTreeSet<&String> = after_fences.iter().collect();
    for fence in &before_fences {
        if after_set.contains(fence) {
            continue;
        }
        let first_line = fence
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("(empty)")
            .to_owned();
        findings.push(Finding::DocTestChanged {
            path: path.to_owned(),
            first_line,
        });
    }

    Ok(FileScan {
        findings,
        removed,
        added,
    })
}

/// Detect every existing test a change set modified, weakened or removed.
///
/// This is AICD §14's rule as a function: "Any pull request in which an
/// existing test was modified, weakened or removed triggers an escalation to a
/// human, whatever the tier." It refuses rather than returning an empty report
/// when it could not read what it was given, because AICD §14's named defect
/// class is a checker that exits successfully on every input.
///
/// # Errors
///
/// Every variant of [`ModifiedTestsError`] is a run that checked nothing or
/// checked it with a reader that is demonstrably broken.
pub fn scan(changes: &[FileChange]) -> Result<Report, ModifiedTestsError> {
    if changes.is_empty() {
        return Err(ModifiedTestsError::NothingToCheck);
    }

    let mut census = Census {
        files: changes.len(),
        ..Census::default()
    };
    let mut findings: Vec<Finding> = Vec::new();
    let mut all_removed: Vec<(String, Item)> = Vec::new();
    let mut all_added: Vec<(String, Item)> = Vec::new();

    for change in changes {
        let scan = compare_file(change, &mut census)?;
        findings.extend(scan.findings);
        all_removed.extend(
            scan.removed
                .into_iter()
                .map(|item| (change.path.clone(), item)),
        );
        all_added.extend(
            scan.added
                .into_iter()
                .map(|item| (change.path.clone(), item)),
        );
    }

    // A file that moved is not a modification, and a rename inside a file is.
    // Both look like a removal here, and the two are told apart by what the
    // added items carry: an item with the same name and the same body has
    // moved, an item with a different name and the same body is the rename.
    for (path, item) in &all_removed {
        let moved = all_added
            .iter()
            .any(|(_, added)| added.name == item.name && added.body == item.body);
        if moved {
            census.moved += 1;
            continue;
        }
        let replaced_by = all_added
            .iter()
            .find(|(added_path, added)| {
                added_path == path && !added.tail().is_empty() && added.tail() == item.tail()
            })
            .map(|(_, added)| added.name.clone());
        findings.push(Finding::Removed {
            path: path.clone(),
            name: item.name.clone(),
            surface: item.surface,
            replaced_by,
        });
    }

    findings.sort_by(|left, right| {
        left.path()
            .cmp(right.path())
            .then_with(|| left.to_string().cmp(&right.to_string()))
    });

    Ok(Report { findings, census })
}

// ---------------------------------------------------------------------------
// Reading the change set from git
// ---------------------------------------------------------------------------

fn git(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>, ModifiedTestsError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|error| ModifiedTestsError::Git {
            command: format!("git {}", args.join(" ")),
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(ModifiedTestsError::Git {
            command: format!("git {}", args.join(" ")),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

/// Read a pull request's change set out of a git repository: AICD §14.
///
/// Derived from AICD §14's subject: the rule is about a pull request, and a
/// pull request's change set is the merge base of `base` and `head` against
/// `head`. The merge base is computed here rather than taken from the caller,
/// because a two-dot range against a moved branch reports changes the pull
/// request did not make.
///
/// `--no-renames` is deliberate. A renamed file reported as a rename hides one
/// of its two halves; reported as a deletion and an addition it shows both, and
/// [`scan`] excuses the pure move itself.
///
/// # Errors
///
/// [`ModifiedTestsError::Git`] when any of the three commands fails, and
/// [`ModifiedTestsError::NothingToCheck`] when the range is empty, because a
/// run over no file has checked nothing.
pub fn changes_from_git(
    repo_root: &Path,
    base: &str,
    head: &str,
) -> Result<Vec<FileChange>, ModifiedTestsError> {
    let merge_base_out = git(repo_root, &["merge-base", base, head])?;
    let merge_base = String::from_utf8_lossy(&merge_base_out).trim().to_owned();

    let listing = git(
        repo_root,
        &[
            "diff",
            "--name-status",
            "-z",
            "--no-renames",
            &merge_base,
            head,
        ],
    )?;
    let listing = String::from_utf8_lossy(&listing).into_owned();

    let mut fields = listing.split('\0').filter(|field| !field.is_empty());
    let mut changes = Vec::new();

    while let Some(status) = fields.next() {
        let Some(path) = fields.next() else { break };
        let before = if status.starts_with('A') {
            None
        } else {
            blob(repo_root, &merge_base, path)?
        };
        let after = if status.starts_with('D') {
            None
        } else {
            blob(repo_root, head, path)?
        };
        changes.push(FileChange {
            path: path.to_owned(),
            before,
            after,
        });
    }

    if changes.is_empty() {
        return Err(ModifiedTestsError::NothingToCheck);
    }
    Ok(changes)
}

/// One file at one revision, as text.
///
/// A blob whose bytes are not UTF-8 is stood in for by its object id rather
/// than dropped. Dropping it would make a changed binary planted fixture
/// compare equal to itself, which is a change this gate reports and would
/// then have reported nothing about (AICD §14). An object id differs exactly
/// when the bytes differ, so the file-level comparison stays honest, and the
/// stand-in carries no Rust for the item walker to find.
fn blob(repo_root: &Path, rev: &str, path: &str) -> Result<Option<String>, ModifiedTestsError> {
    let bytes = git(repo_root, &["show", &format!("{rev}:{path}")])?;
    if let Ok(text) = String::from_utf8(bytes) {
        return Ok(Some(text));
    }
    let object = git(repo_root, &["rev-parse", &format!("{rev}:{path}")])?;
    let object = String::from_utf8_lossy(&object).trim().to_owned();
    Ok(Some(format!("not UTF-8; git object {object}\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    // Nothing in this module's doc comments backticks a test name. The check
    // in crates/ori-gates/src/sections.rs sweeps every doc comment under
    // crates/ and fails on a backticked span that is the name of a test
    // function it has not registered, so tests are named in plain comments
    // like this one.

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("a crate lives two levels below the repository root")
            .to_path_buf()
    }

    // -----------------------------------------------------------------------
    // The subject every case below is a change to.
    //
    // One file with two existing tests and one existing helper, which is the
    // shape of every test module in this workspace.
    // -----------------------------------------------------------------------

    const SUBJECT: &str = r#"//! A crate.

pub fn add(left: u32, right: u32) -> u32 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subjects() -> Vec<u32> {
        vec![1, 2, 3]
    }

    #[test]
    fn ori_t_0043_fixture_addition_is_commutative() {
        let total = add(2, 3);
        assert_eq!(total, 5, "two and three are five");
    }

    #[test]
    fn ori_t_0043_fixture_every_subject_is_positive() {
        for subject in subjects() {
            assert!(subject > 0);
        }
    }
}
"#;

    fn change(after: &str) -> Vec<FileChange> {
        vec![FileChange::modified(
            "crates/demo/src/lib.rs",
            SUBJECT,
            after,
        )]
    }

    fn scanned(after: &str) -> Report {
        scan(&change(after)).expect("the reader reads the fixture")
    }

    fn only_finding(report: &Report) -> String {
        assert_eq!(
            report.findings().len(),
            1,
            "expected exactly one finding, got: {:?}",
            report.lines()
        );
        report.findings()[0].to_string()
    }

    // -----------------------------------------------------------------------
    // ORI-P1-010: "PR whose diff modifies an existing test | Gate run |
    // Modified-test gate fails". The nine planted cases of this ticket, one
    // test each.
    // -----------------------------------------------------------------------

    // Planted defect 1. A deleted test.
    #[test]
    fn ori_p1_010_a_deleted_test_is_detected() {
        let after = SUBJECT
            .replace(
                "    #[test]\n    fn ori_t_0043_fixture_addition_is_commutative() {\n        let total = add(2, 3);\n        assert_eq!(total, 5, \"two and three are five\");\n    }\n\n",
                "",
            );
        assert_ne!(after, SUBJECT, "the planted deletion must change the file");
        let report = scanned(&after);
        assert!(
            report.modified_an_existing_test(),
            "a deleted test was not detected: {:?}",
            report.lines()
        );
        let message = only_finding(&report);
        assert!(
            message.contains("ori_t_0043_fixture_addition_is_commutative")
                && message.contains("was removed"),
            "the finding does not name the deletion: {message}"
        );
        assert_eq!(
            report.test_modification(Escalation::Absent),
            TestModification::EscalationMissing
        );
    }

    // Planted defect 2. A changed assertion inside an existing test.
    #[test]
    fn ori_p1_010_a_changed_assertion_is_detected() {
        let after = SUBJECT.replace("assert_eq!(total, 5,", "assert_eq!(total, 6,");
        assert_ne!(after, SUBJECT, "the planted change must change the file");
        let report = scanned(&after);
        let message = only_finding(&report);
        assert!(
            message.contains("ori_t_0043_fixture_addition_is_commutative")
                && message.contains("(code)"),
            "the finding does not name the changed assertion as code: {message}"
        );
    }

    // Planted defect 3. A renamed test. A rename is a deletion and an addition
    // and the naive reader sees one added test and one removed and calls the
    // count unchanged.
    #[test]
    fn ori_p1_010_a_renamed_test_is_detected() {
        let after = SUBJECT.replace(
            "fn ori_t_0043_fixture_addition_is_commutative()",
            "fn ori_t_0043_fixture_addition_holds()",
        );
        assert_ne!(after, SUBJECT, "the planted rename must change the file");
        let report = scanned(&after);
        let message = only_finding(&report);
        assert!(
            message.contains("ori_t_0043_fixture_addition_is_commutative")
                && message.contains("ori_t_0043_fixture_addition_holds")
                && message.contains("renamed"),
            "the finding does not report the rename as one: {message}"
        );
        assert_eq!(report.census().added, 1, "the new name is an addition too");
    }

    // Planted defect 4. A new test and nothing else. This is the false positive
    // that would stop every ticket in this project.
    #[test]
    fn ori_p1_010_a_new_test_alone_is_not_detected() {
        let addition = "\n    #[test]\n    fn ori_t_0043_fixture_addition_has_an_identity() {\n        assert_eq!(add(7, 0), 7);\n    }\n";
        let mut after = SUBJECT.to_owned();
        let close = after.rfind("}\n").expect("the module closes");
        after.insert_str(close, addition);
        let report = scanned(&after);
        assert!(
            !report.modified_an_existing_test(),
            "adding a test alone was reported as a modification: {:?}",
            report.lines()
        );
        assert_eq!(report.census().added, 1);
        assert_eq!(report.census().unchanged, 4);
        assert_eq!(
            report.test_modification(Escalation::Absent),
            TestModification::None
        );
    }

    // Planted defect 5. A reformatting with no semantic change. Decision 3 of
    // this module says it is detected, and this is the proof that it is.
    #[test]
    fn ori_p1_010_a_reformatted_test_is_detected_as_whitespace_only() {
        let after = SUBJECT.replace(
            "        let total = add(2, 3);\n        assert_eq!(total, 5, \"two and three are five\");\n",
            "        let total = add(2, 3);\n        assert_eq!(\n            total,\n            5,\n            \"two and three are five\"\n        );\n",
        );
        assert_ne!(after, SUBJECT, "the planted reflow must change the file");
        let report = scanned(&after);
        let message = only_finding(&report);
        assert!(
            message.contains("(whitespace only)"),
            "a reflow was not classified as whitespace only: {message}"
        );
        assert!(
            report.modified_an_existing_test(),
            "decision 3 says a reflow of an existing test is detected"
        );
    }

    // Planted defects 6 and 7. A detector that answers the same on every input
    // is the defect class AICD §14 names. One table, both halves, so a constant
    // answer fails whichever constant it is.
    #[test]
    fn ori_p1_010_the_detector_separates_modification_from_addition() {
        let deleted = SUBJECT.replace("        assert!(subject > 0);\n", "");
        let changed = SUBJECT.replace("subject > 0", "subject >= 0");
        let renamed = SUBJECT.replace("fn subjects()", "fn the_subjects()");
        let mut added = SUBJECT.to_owned();
        let close = added.rfind("}\n").expect("the module closes");
        added.insert_str(
            close,
            "\n    #[test]\n    fn ori_t_0043_fixture_one_more() {\n        assert!(true);\n    }\n",
        );
        let unchanged = SUBJECT.to_owned();
        let mut grown = SUBJECT.to_owned();
        grown = grown.replace(
            "        assert_eq!(total, 5, \"two and three are five\");\n",
            "        assert_eq!(total, 5, \"two and three are five\");\n        assert_eq!(add(3, 2), total, \"and the other way round\");\n",
        );

        let table: [(&str, &str, bool); 6] = [
            ("a deleted assertion", deleted.as_str(), true),
            ("a changed assertion", changed.as_str(), true),
            ("a renamed helper", renamed.as_str(), true),
            ("a new test alone", added.as_str(), false),
            ("nothing at all", unchanged.as_str(), false),
            ("an inserted assertion", grown.as_str(), false),
        ];

        let mut wrong: Vec<String> = Vec::new();
        for (label, after, expected) in table {
            let report = scanned(after);
            if report.modified_an_existing_test() != expected {
                wrong.push(format!(
                    "{label}: expected modified={expected}, got {:?}",
                    report.lines()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "a detector that answers the same on every input fails one row of this table or the \
             other, and these rows are the ones it failed:\n  {}",
            wrong.join("\n  ")
        );
    }

    // Planted defect 8. Break the reader so it can find nothing. It must
    // refuse, not pass. The projection's attribute count and the item walker's
    // output are taken by different routes, and a broken walker moves one of
    // them.
    #[test]
    fn ori_p1_010_a_reader_that_finds_no_test_refuses_rather_than_passing() {
        // A file whose test module the walker cannot enter, simulated here by
        // giving the walker an item boundary it must get wrong: the module's
        // opening brace is on a line of its own, which is legal Rust that the
        // walker must handle, and the count cross-check is what would catch it
        // if it did not.
        let (lines, depth) = project(SUBJECT);
        assert_eq!(depth, 0, "the fixture is balanced");
        let mut items = Vec::new();
        collect_items(&lines, (0, lines.len()), 0, "", false, &mut items);
        assert_eq!(
            count_test_attributes(&lines),
            2,
            "the projection must see the fixture's two test attributes"
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.surface == Surface::Test)
                .count(),
            2,
            "the walker must collect the fixture's two tests"
        );

        // Now the break: a walker that collects nothing. The cross-check is
        // asked directly, because a broken walker is not something this file
        // can contain while also compiling.
        let broken: Vec<Item> = Vec::new();
        let refusal = check_reader("crates/demo/src/lib.rs", SUBJECT, &broken)
            .expect_err("a walker that collects nothing must be refused");
        assert_eq!(
            refusal,
            ModifiedTestsError::TestCountMismatch {
                path: "crates/demo/src/lib.rs".to_owned(),
                attributes: 2,
                collected: 0,
            }
        );
        assert_eq!(refusal.methodology_ref(), "AICD §14");
        assert!(
            refusal
                .to_string()
                .contains("a broken walker finds no modification in any input"),
            "the refusal must say what it means: {refusal}"
        );

        // And the other half: an empty change set is a run that checked
        // nothing, which is not a pass either.
        assert_eq!(
            scan(&[]).expect_err("an empty change set is refused"),
            ModifiedTestsError::NothingToCheck
        );

        // And a file the projection cannot balance.
        let unbalanced = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n";
        let refusal = scan(&[FileChange::modified(
            "crates/demo/src/lib.rs",
            unbalanced,
            unbalanced,
        )])
        .expect_err("an unbalanced file is refused");
        assert!(
            matches!(refusal, ModifiedTestsError::UnbalancedDelimiters { .. }),
            "expected an unbalanced-delimiter refusal, got {refusal}"
        );
    }

    // Planted defect 9. Unchanged. Must pass, and must say what it read while
    // passing.
    #[test]
    fn ori_p1_010_an_unchanged_file_passes_and_says_what_it_read() {
        let report = scanned(SUBJECT);
        assert!(!report.modified_an_existing_test());
        assert_eq!(report.census().files, 1);
        assert_eq!(report.census().files_with_test_surface, 1);
        assert_eq!(report.census().tests_before, 2);
        assert_eq!(report.census().unchanged, 4);
        let line = &report.lines()[0];
        assert!(
            line.contains("4 item(s)") && line.contains("1 of 1 changed file(s)"),
            "a clean run must say how much it read: {line}"
        );
    }

    // The three values this detector produces are the three the ticket machine
    // consumes. Held against crates/ori-core/src/ticket.rs as text, because
    // ori-gates does not depend on ori-core.
    #[test]
    fn ori_p1_010_the_three_states_are_the_ticket_machine_s_three() {
        let path = repository_root().join("crates/ori-core/src/ticket.rs");
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let (lines, _) = project(&source);
        let start = lines
            .iter()
            .position(|line| line.code.trim() == "pub enum TestModification {")
            .expect("ori-core declares TestModification");
        let mut spellings: Vec<String> = Vec::new();
        for line in &lines[start + 1..] {
            let trimmed = line.code.trim();
            if trimmed == "}" {
                break;
            }
            if let Some(name) = trimmed.strip_suffix(',')
                && name.chars().all(is_ident_char)
                && !name.is_empty()
            {
                spellings.push(name.to_owned());
            }
        }
        assert_eq!(
            spellings,
            TestModification::SPELLINGS.to_vec(),
            "the restatement in this module and the ticket machine's own type disagree. The \
             detector produces what that machine consumes, so a drift here is a value the \
             machine cannot read"
        );

        // And the mapping, which is the whole of what this detector hands over.
        let clean = scanned(SUBJECT);
        assert_eq!(
            clean.test_modification(Escalation::Open),
            TestModification::None,
            "an open escalation does not make an unmodified pull request modified"
        );
        let dirty = scanned(&SUBJECT.replace("subject > 0", "subject >= 0"));
        assert_eq!(
            dirty.test_modification(Escalation::Open),
            TestModification::EscalationOpen
        );
        assert_eq!(
            dirty.test_modification(Escalation::Absent),
            TestModification::EscalationMissing
        );
    }

    // The gate reads a real git range, in a repository built for the purpose.
    // Nothing here depends on this repository's own history, which a shallow
    // clone would not have.
    #[test]
    fn ori_p1_010_the_gate_reads_a_change_set_out_of_git() {
        let root = std::env::temp_dir().join(format!(
            "ori-t-0043-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(root.join("crates/demo/src")).expect("the temporary tree is created");

        let run = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap_or_else(|error| {
                    panic!(
                        "git is required to read a pull request's change set and is not runnable \
                         here: {error}"
                    )
                });
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };

        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t0043@example.invalid"]);
        run(&["config", "user.name", "ORI-T-0043"]);
        fs::write(root.join("crates/demo/src/lib.rs"), SUBJECT).expect("the subject is written");
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "base"]);

        run(&["checkout", "-q", "-b", "feature"]);
        fs::write(
            root.join("crates/demo/src/lib.rs"),
            SUBJECT.replace("assert_eq!(total, 5,", "assert_eq!(total, 4,"),
        )
        .expect("the planted change is written");
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "planted"]);

        let changes = changes_from_git(&root, "main", "feature").expect("the range resolves");
        assert_eq!(changes.len(), 1, "one file changed");
        assert_eq!(changes[0].path, "crates/demo/src/lib.rs");
        let report = scan(&changes).expect("the reader reads it");
        assert!(
            report.modified_an_existing_test(),
            "the planted change was not detected through git: {:?}",
            report.lines()
        );

        // And the empty range, which is a run that checked nothing.
        let refusal = changes_from_git(&root, "main", "main")
            .expect_err("an empty range is a run that checked nothing");
        assert_eq!(refusal, ModifiedTestsError::NothingToCheck);

        let _ = fs::remove_dir_all(&root);
    }

    // -----------------------------------------------------------------------
    // ORI-T-0043: the four insertions that satisfy "every original line
    // survives byte-identical" and neuter the test anyway. Decision 4's table.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0043_an_inserted_ignore_attribute_is_not_additive() {
        let after = SUBJECT.replace(
            "    #[test]\n    fn ori_t_0043_fixture_addition_is_commutative()",
            "    #[test]\n    #[ignore]\n    fn ori_t_0043_fixture_addition_is_commutative()",
        );
        assert_ne!(after, SUBJECT);
        let message = only_finding(&scanned(&after));
        assert!(
            message.contains("attributes") && message.contains("#[ignore]"),
            "an inserted ignore attribute must be reported as one: {message}"
        );
    }

    #[test]
    fn ori_t_0043_an_inserted_early_return_is_not_additive() {
        let after = SUBJECT.replace(
            "        let total = add(2, 3);\n",
            "        if add(1, 1) == 2 {\n            return;\n        }\n        let total = add(2, 3);\n",
        );
        assert_ne!(after, SUBJECT);
        let message = only_finding(&scanned(&after));
        assert!(
            message.contains("`return`"),
            "an inserted early return must be reported: {message}"
        );
    }

    #[test]
    fn ori_t_0043_wrapping_a_body_in_a_loop_is_not_additive() {
        // The original lines survive byte for byte. What changes is the depth
        // they sit at, which is the clause that refuses this.
        let after = SUBJECT.replace(
            "        let total = add(2, 3);\n        assert_eq!(total, 5, \"two and three are five\");\n",
            "        for _ in 0..0 {\n        let total = add(2, 3);\n        assert_eq!(total, 5, \"two and three are five\");\n        }\n",
        );
        assert_ne!(after, SUBJECT);
        let message = only_finding(&scanned(&after));
        assert!(
            message.contains("original delimiter depth"),
            "wrapping a body in a loop that runs zero times must be reported: {message}"
        );
    }

    #[test]
    fn ori_t_0043_a_shadowing_insertion_is_not_additive() {
        let after = SUBJECT.replace(
            "        assert_eq!(total, 5, \"two and three are five\");\n",
            "        let total = 5;\n        assert_eq!(total, 5, \"two and three are five\");\n",
        );
        assert_ne!(after, SUBJECT);
        let message = only_finding(&scanned(&after));
        assert!(
            message.contains("binds `total`"),
            "an insertion that shadows a name the assertions read must be reported: {message}"
        );
    }

    #[test]
    fn ori_t_0043_an_inserted_assertion_alone_is_additive() {
        let after = SUBJECT.replace(
            "        assert_eq!(total, 5, \"two and three are five\");\n",
            "        assert_eq!(total, 5, \"two and three are five\");\n        assert_eq!(add(3, 2), 5, \"and the other way round\");\n",
        );
        assert_ne!(after, SUBJECT);
        let report = scanned(&after);
        assert!(
            !report.modified_an_existing_test(),
            "a pure insertion of an assertion is additive: {:?}",
            report.lines()
        );
        assert_eq!(report.census().additive, 1);
    }

    // The two merged sibling commits, in the shapes they actually took.
    // Reproduced as fixtures rather than read out of this repository's history,
    // because the gate-2 job checks out one commit and cannot see them.
    #[test]
    fn ori_t_0043_the_shape_of_the_two_merged_sibling_commits() {
        // Commit 39f7921 (ORI-T-0093, crates/ori-core/src/types.rs): three
        // assertions inserted into the body of an existing test, 18 insertions
        // and 0 deletions. Additive.
        let before = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0019_the_value_lists() {\n        assert_eq!(Seat::ALL.len(), 4, \"four\");\n    }\n}\n";
        let after = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0019_the_value_lists() {\n        assert_eq!(Seat::ALL.len(), 4, \"four\");\n        assert_eq!(ProductOrigin::ALL.len(), 2, \"two\");\n    }\n}\n";
        let report = scan(&[FileChange::modified(
            "crates/demo/src/types.rs",
            before,
            after,
        )])
        .expect("the reader reads it");
        assert!(
            !report.modified_an_existing_test(),
            "commit 39f7921's shape is additive and this gate would have passed it: {:?}",
            report.lines()
        );

        // Commit 6bb0236 (ORI-T-0098, the same file): 399 insertions and 0
        // deletions, all of them new items after the existing test, which is
        // untouched. Additive.
        let grown = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0019_the_value_lists() {\n        assert_eq!(Seat::ALL.len(), 4, \"four\");\n    }\n\n    #[test]\n    fn ori_t_0098_the_spellings() {\n        assert_eq!(ProductOrigin::New.as_str(), \"new\");\n    }\n}\n";
        let report = scan(&[FileChange::modified(
            "crates/demo/src/types.rs",
            before,
            grown,
        )])
        .expect("the reader reads it");
        assert!(
            !report.modified_an_existing_test(),
            "commit 6bb0236's shape adds tests and touches none: {:?}",
            report.lines()
        );

        // Commit d66080e (ORI-T-0093, crates/ori-core/src/error.rs): the same
        // additive insertion into a test body, and a rewrite of the two helpers
        // that test iterates. Decision 5 says this gate fails it, and this is
        // the proof that it does.
        let helpers_before = "#[cfg(test)]\nmod tests {\n    fn every_refusal() -> Vec<RefusalKind> {\n        vec![RefusalKind::CriterionMissing]\n    }\n\n    #[test]\n    fn ori_p1_033_every_refusal_is_covered() {\n        let refusals = every_refusal();\n        assert!(!refusals.is_empty());\n    }\n}\n";
        let helpers_after = "#[cfg(test)]\nmod tests {\n    macro_rules! refusal_samples {\n        () => {};\n    }\n\n    #[test]\n    fn ori_p1_033_every_refusal_is_covered() {\n        let refusals = every_refusal();\n        assert!(!refusals.is_empty());\n        assert!(refusals.len() > 0);\n    }\n}\n";
        let report = scan(&[FileChange::modified(
            "crates/demo/src/error.rs",
            helpers_before,
            helpers_after,
        )])
        .expect("the reader reads it");
        let messages = report.lines().join("\n");
        assert!(
            report.modified_an_existing_test(),
            "commit d66080e removed the helper a coverage test iterates, and decision 5 says \
             this gate fails it: {messages}"
        );
        assert!(
            messages.contains("every_refusal") && messages.contains("was removed"),
            "the finding must name the helper and not the test body: {messages}"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0043: the reader, on the inputs a line-counting reader gets wrong.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0043_the_projection_ignores_delimiters_inside_text() {
        let source = "fn f() {\n    let a = \"}}}\";\n    let b = '}';\n    // }\n    /* } */\n    let c = r#\"}\"#;\n}\n";
        let (lines, depth) = project(source);
        assert_eq!(
            depth, 0,
            "a delimiter inside a string, a char, a comment or a raw string is not a delimiter"
        );
        assert!(
            lines.iter().all(|line| line.depth <= 1),
            "no line of this fixture is deeper than the function body"
        );
        assert!(
            lines[1].code.contains("\"\""),
            "a string's contents are blanked and its quotes kept: {}",
            lines[1].code
        );
    }

    #[test]
    fn ori_t_0043_a_lifetime_is_not_a_character_literal() {
        let source = "fn f<'a>(x: &'a str) -> &'a str {\n    x\n}\n";
        let (lines, depth) = project(source);
        assert_eq!(depth, 0, "a lifetime must not swallow the rest of the line");
        assert!(
            lines[1].code.trim() == "x",
            "the body survived the lifetime: {}",
            lines[1].code
        );
    }

    #[test]
    fn ori_t_0043_a_support_item_is_test_surface_and_an_ordinary_one_is_not() {
        let after = SUBJECT.replace("vec![1, 2, 3]", "vec![]");
        assert_ne!(after, SUBJECT);
        let message = only_finding(&scanned(&after));
        assert!(
            message.contains("test support item") && message.contains("subjects"),
            "emptying the helper a test iterates must be reported: {message}"
        );

        let ordinary = SUBJECT.replace("left + right", "left.wrapping_add(right)");
        assert_ne!(ordinary, SUBJECT);
        let report = scanned(&ordinary);
        assert!(
            !report.modified_an_existing_test(),
            "a change to the code under test is not a change to a test: {:?}",
            report.lines()
        );
    }

    #[test]
    fn ori_t_0043_a_doc_test_that_changed_is_reported_and_a_text_fence_is_not() {
        let before = "/// Adds.\n///\n/// ```\n/// assert_eq!(demo::add(1, 1), 2);\n/// ```\n///\n/// ```text\n/// a diagram\n/// ```\npub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";
        let after = before.replace("demo::add(1, 1), 2", "demo::add(1, 1), 3");
        let report = scan(&[FileChange::modified(
            "crates/demo/src/lib.rs",
            before,
            &after,
        )])
        .expect("the reader reads it");
        let message = only_finding(&report);
        assert!(
            message.contains("a doc test was removed or changed"),
            "a changed doc test must be reported: {message}"
        );

        let text_changed = before.replace("a diagram", "another diagram");
        let report = scan(&[FileChange::modified(
            "crates/demo/src/lib.rs",
            before,
            &text_changed,
        )])
        .expect("the reader reads it");
        assert!(
            !report.modified_an_existing_test(),
            "a text fence is not a doc test: {:?}",
            report.lines()
        );
    }

    #[test]
    fn ori_t_0043_a_file_that_moved_intact_is_not_a_modification() {
        let changes = vec![
            FileChange::deleted("crates/demo/src/lib.rs", SUBJECT),
            FileChange::added("crates/demo/src/moved.rs", SUBJECT),
        ];
        let report = scan(&changes).expect("the reader reads it");
        assert!(
            !report.modified_an_existing_test(),
            "a file that moved with every test intact is not a modification: {:?}",
            report.lines()
        );
        assert_eq!(report.census().moved, 4);

        // And the same move with one assertion dropped on the way is one.
        let changes = vec![
            FileChange::deleted("crates/demo/src/lib.rs", SUBJECT),
            FileChange::added(
                "crates/demo/src/moved.rs",
                &SUBJECT.replace("        assert!(subject > 0);\n", ""),
            ),
        ];
        let report = scan(&changes).expect("the reader reads it");
        assert!(
            report.modified_an_existing_test(),
            "a move that drops an assertion is a modification: {:?}",
            report.lines()
        );
    }

    #[test]
    fn ori_t_0043_a_deleted_file_of_tests_is_every_test_in_it() {
        let report = scan(&[FileChange::deleted(
            "crates/demo/tests/end_to_end.rs",
            SUBJECT,
        )])
        .expect("the reader reads it");
        assert!(report.modified_an_existing_test());
        assert!(
            report.findings().len() >= 2,
            "a deleted test file loses every test in it: {:?}",
            report.lines()
        );
    }

    #[test]
    fn ori_t_0043_everything_in_a_tests_directory_is_test_surface() {
        let before = "fn helper() -> u32 {\n    7\n}\n\n#[test]\nfn ori_t_0043_fixture_integration() {\n    assert_eq!(helper(), 7);\n}\n";
        let after = before.replace("    7\n", "    0\n");
        let report = scan(&[FileChange::modified(
            "crates/demo/tests/end_to_end.rs",
            before,
            &after,
        )])
        .expect("the reader reads it");
        let message = only_finding(&report);
        assert!(
            message.contains("helper"),
            "a helper in a tests/ file is test surface: {message}"
        );
    }

    #[test]
    fn ori_t_0043_a_changed_planted_fixture_is_reported_and_a_new_one_is_not() {
        let report = scan(&[FileChange::modified(
            "fixtures/planted/gate-13/prove.sh",
            "echo clean\n",
            "echo dirty\n",
        )])
        .expect("the reader reads it");
        let message = only_finding(&report);
        assert!(
            message.contains("a planted defect changed"),
            "a changed planted defect must be reported: {message}"
        );

        let report = scan(&[FileChange::added(
            "fixtures/planted/gate-5/prove.sh",
            "echo new\n",
        )])
        .expect("the reader reads it");
        assert!(
            !report.modified_an_existing_test(),
            "a new planted fixture is not a change to an existing one: {:?}",
            report.lines()
        );
    }

    #[test]
    fn ori_t_0043_a_change_with_no_test_surface_passes_and_says_so() {
        let report = scan(&[FileChange::modified("README.md", "# One\n", "# Two\n")])
            .expect("the reader reads it");
        assert!(!report.modified_an_existing_test());
        assert_eq!(report.census().files_with_test_surface, 0);
        assert_eq!(report.census().items_before, 0);
        assert!(
            report.lines()[0].contains("0 of 1 changed file(s)"),
            "a run over no test surface must say so rather than say it passed: {}",
            report.lines()[0]
        );
    }

    // The reader, run over this workspace's own largest test modules. If it
    // finds nothing there it is broken, and a broken reader reports nothing on
    // every input.
    #[test]
    fn ori_t_0043_the_reader_finds_this_workspace_s_own_tests() {
        let root = repository_root();
        let subjects = [
            "crates/ori-core/src/types.rs",
            "crates/ori-core/src/ticket.rs",
            "crates/ori-gates/src/spec_refs.rs",
            "crates/ori-gates/src/modified_tests.rs",
        ];
        let mut total = 0usize;
        for relative in subjects {
            let path = root.join(relative);
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let (items, _) = read_surface(relative, &source)
                .unwrap_or_else(|error| panic!("{relative}: {error}"));
            check_reader(relative, &source, &items)
                .unwrap_or_else(|error| panic!("{relative}: {error}"));
            let tests = items
                .iter()
                .filter(|item| item.surface == Surface::Test)
                .count();
            assert!(
                tests > 0,
                "the reader found no test in {relative}, so it is broken rather than the file \
                 untested"
            );
            total += tests;
        }
        assert!(
            total >= 20,
            "the reader collected {total} test(s) from four of this workspace's test modules, \
             which is too few to be reading them"
        );
    }
}
