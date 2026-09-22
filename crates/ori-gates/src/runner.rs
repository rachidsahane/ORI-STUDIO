//! The `Runner` trait and the registry that turns a set of
//! [`crate::gate::GateDef`] values into a `gates.list` answer:
//! `spec/LLD.md` section 2, `spec/CI_CD.md` section 1.
//!
//! `spec/LLD.md` section 2 gives this crate `GateDef`, the `Runner` trait,
//! the built-in runners and `Prover`. [`crate::gate`] builds the first.
//! `Prover` does not exist here: `spec/RISK_MAP.md`
//! marks `crates/ori-gates/src/prover.rs` tier 2 for "Gate integrity" and
//! this ticket's declared scope is `crates/ori-gates/src/gate.rs` and
//! `crates/ori-gates/src/runner.rs`, nothing else. What this module owns is
//! the trait, [`crate::runner::Outcome`], the registry, and one wrapper
//! around an existing runner in this crate, to show the trait fits.
//!
//! # Why a trait, not a function pointer
//!
//! [`crate::coverage::CoverageMatrix::read`] takes a product root and nothing
//! else. [`crate::modified_tests::scan`] takes a slice of already-gathered
//! file changes; the git-touching half of that module,
//! `crate::modified_tests::changes_from_git`, additionally needs a base
//! revision and a head revision. `crate::sections` and `crate::spec_refs`
//! call no single function at all: their checks are private to their own
//! `#[cfg(test)]` modules (the section below, "Whether the trait fits",
//! reads that in full). Four real runners, four different input shapes, and
//! two of the four are not callable as a function at all. A bare `fn(&Path)
//! -> Outcome` would fit exactly one of the four and force every other
//! runner to smuggle its real configuration in through a side channel, which
//! is the same problem restated. [`crate::runner::Runner`] instead takes no
//! argument beyond `&self`, and each implementation is a small struct that
//! holds whatever its wrapped check actually needs (a root path, a change
//! set, a git range) as fields set once at construction. The registry then
//! holds `Box<dyn Runner>` and calls `.run()` uniformly, and the
//! heterogeneity lives where it is real, in the wrapper, rather than being
//! forced into one shared signature that fits nothing well. Gathering fresh
//! input for a given run (running `git diff`, walking a tree) is therefore
//! the caller's job, done once before a `Runner` is built for that run;
//! `run` itself stays a pure read of whatever the wrapper was given.
//!
//! # Whether `blocked` is a verdict or an error
//!
//! [`crate::coverage`]'s `Verdict` has two values, `Passed` and `Failed`, and
//! its own doc comment says a run that cannot read its input "returns
//! `CoverageError` instead and is a failure in the caller's hands": blocked
//! is folded into "not a pass" but is not named as its own thing anywhere in
//! that module's types. [`crate::modified_tests`] disagrees, in its own
//! words: "a report with no findings and a non-empty census is `passed`; a
//! report with findings is `failed`; every `ModifiedTestsError` is
//! `blocked`, never `passed`, because nothing was checked." `scripts/gates.sh`
//! settles which reading this module follows. Its header names blocked as one
//! of five states on purpose and is explicit about the cost of merging it
//! into anything else: folding blocked into failed once painted a working
//! gate red forever until a human gave up reading the exit code, and folding
//! it into "not available" once let a real `rustfmt` violation through
//! because the missing prerequisite hid the sibling check that was actually
//! installed. [`crate::runner::Outcome`] therefore has three values, `Passed`,
//! `Failed` and `Blocked`, as one enum rather than as a verdict plus a
//! separately-handled error: a caller matching on `Outcome` cannot compile
//! having forgotten the blocked arm, where a caller matching on `Result<Verdict,
//! E>` can `unwrap_or(Verdict::Passed)` an error away without the compiler ever
//! objecting. Planted defect 3 below is exactly that mistake, made on purpose,
//! to show the test that must catch it.
//!
//! # Whether the trait fits the four existing runners
//!
//! [`crate::runner::CoverageRunner`] and [`crate::runner::ModifiedTestsRunner`]
//! below wrap [`crate::coverage`] and [`crate::modified_tests`] without
//! changing either file: both expose a plain function returning a
//! `Result<T, E>` over an input the wrapper struct can hold, which is exactly
//! what [`crate::runner::Runner::run`] needs to call once and translate.
//! `crate::sections` and `crate::spec_refs` are a different, harder case, and
//! wrapping either is not attempted here. `crate::sections`'s own head
//! comment names the reason: the whole-repository scan that would BE its
//! gate ("the reader ... every `#[test] fn`", the citation walk) is a
//! `#[cfg(test)]` item private to that module's own test module, not a
//! public function, because widening it to be callable from outside would be
//! an edit to an existing test module, which `spec/CONVENTIONS.md` under
//! "Tests" forbids an agent to make. `crate::spec_refs` has a public,
//! callable half, `DocumentIndex::from_root` and
//! `DocumentIndex::resolve`, but the repository-wide walk that collects every
//! reference to resolve is, by that module's own account, "duplicated"
//! rather than shared for the same reason, and stays private to its test
//! module too. So two of the four runners this crate already has cannot be
//! wrapped at all without either editing an existing test (forbidden) or
//! reimplementing a whole-repository citation scan a second time in this
//! file (which would not be "the trait fits", it would be a second copy of
//! `crate::sections` wearing a trait). That two of four real runners are
//! structurally unwrappable is this ticket's finding, not a gap in the
//! trait: a trait can only wrap what its target exposes to call.
//!
//! ```mermaid
//! flowchart TB
//!   G[GateDef, from crate::gate] --> R[Registry::install]
//!   RUN[Box dyn Runner, optional] --> R
//!   R --> L[Registry::list]
//!   L --> LST["GateListing: number, kind, name, state"]
//!   RUN -.->|CoverageRunner| COV[crate::coverage::CoverageMatrix]
//!   RUN -.->|ModifiedTestsRunner| MOD[crate::modified_tests::scan]
//! ```
//!
//! # Why the links here are written `crate::runner::` and `crate::gate::`
//!
//! `lib.rs` carries a `///` on this module's declaration, so rustdoc resolves
//! this file's `//!` links in the crate root's scope rather than this
//! module's. `crates/ori-gates/src/sections.rs` records the same trap. Do not
//! shorten these.
//!
//! Must not: add a dependency (`CLAUDE.md` absolute rule 6); nothing outside
//! `std` and this crate's own modules is used. Must not: report a gate
//! installed without a proof (`spec/LLD.md` section 2): see
//! [`crate::runner::GateState`] and [`crate::runner::Registry::list`].

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::coverage::{self, CoverageMatrix};
use crate::gate::{DefinitionState, GateDef, GateKind, GateNumber};
use crate::modified_tests::{self, FileChange};

/// What a [`crate::runner::Runner`] itself can honestly report about one run:
/// `spec/CI_CD.md` section 1, `scripts/gates.sh`'s answer space narrowed to
/// the three states that mean a runner actually ran.
///
/// `scripts/gates.sh` names five states in total; the other two, "not
/// available" (no runner exists) and "not evaluated" (nothing has run yet),
/// describe facts about the registry, not about a runner that was asked to
/// run, so [`crate::runner::Runner::run`] cannot produce either and this type
/// does not carry them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The gate ran here and was clean.
    Passed,
    /// The gate ran here and was not clean.
    Failed,
    /// The gate has a runner and no verdict came out of it: nothing was
    /// checked. Never produced by mapping an error to a pass.
    Blocked,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passed => f.write_str("passed"),
            Self::Failed => f.write_str("failed"),
            Self::Blocked => f.write_str("blocked"),
        }
    }
}

/// One run of one [`crate::runner::Runner`]: the state and one line of why.
///
/// Derived from AICD §14's "present but reporting nothing": an [`Outcome`]
/// alone tells a reader nothing about what was actually read, so every run
/// carries a sentence with it, the way [`crate::coverage::CoverageMatrix::render`]
/// and [`crate::modified_tests::Report::lines`] do for their own callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunReport {
    /// What the run found.
    pub outcome: Outcome,
    /// One line of why, for the pull request report or `gates.list`.
    pub detail: String,
}

/// One gate's check, callable uniformly whatever it wraps: `spec/LLD.md`
/// section 2.
///
/// See this module's head comment, "Why a trait, not a function pointer",
/// for the reasoning against the alternative. A [`crate::runner::Runner`]
/// answers for exactly one [`crate::gate::GateNumber`], which
/// [`crate::runner::Registry::install`] checks against the
/// [`crate::gate::GateDef`] it is paired with, so a runner cannot silently
/// answer for a gate other than the one it claims.
pub trait Runner {
    /// Which gate this runner checks.
    fn number(&self) -> GateNumber;

    /// Run the check and report what happened: AICD §14.
    ///
    /// An implementation must map every error from what it wraps to
    /// [`Outcome::Blocked`], never to [`Outcome::Passed`]: an error means
    /// nothing was checked, and reporting a pass for that is the defect class
    /// AICD §14 exists to catch. [`crate::runner::ModifiedTestsRunner::run`]
    /// is the reference implementation of that rule.
    fn run(&self) -> RunReport;
}

/// Wraps [`crate::coverage::CoverageMatrix`] as a [`crate::runner::Runner`],
/// gate 4 of `spec/CI_CD.md` section 1.
///
/// Demonstrates the trait against the cleanest of the four existing runners:
/// [`crate::coverage::CoverageMatrix::read`] is a plain function of a product
/// root, fully public, with no dependency on anything private to a test
/// module.
#[derive(Clone, Debug)]
pub struct CoverageRunner {
    number: GateNumber,
    root: PathBuf,
}

impl CoverageRunner {
    /// A coverage-matrix runner over `root`, answering as `number`.
    #[must_use]
    pub fn new(number: GateNumber, root: impl Into<PathBuf>) -> Self {
        Self {
            number,
            root: root.into(),
        }
    }
}

impl Runner for CoverageRunner {
    fn number(&self) -> GateNumber {
        self.number
    }

    fn run(&self) -> RunReport {
        match CoverageMatrix::read(&self.root) {
            Ok(matrix) => {
                let outcome = match matrix.verdict() {
                    coverage::Verdict::Passed => Outcome::Passed,
                    coverage::Verdict::Failed => Outcome::Failed,
                };
                RunReport {
                    outcome,
                    detail: matrix.verdict().to_string(),
                }
            }
            // An error is nothing checked, never a pass: see this trait's
            // `run` doc.
            Err(error) => RunReport {
                outcome: Outcome::Blocked,
                detail: error.to_string(),
            },
        }
    }
}

/// Wraps [`crate::modified_tests::scan`] as a [`crate::runner::Runner`],
/// gate 5 of `spec/CI_CD.md` section 1.
///
/// `crate::modified_tests`'s own head comment already states the mapping this
/// wraps: "a report with no findings and a non-empty census is `passed`; a
/// report with findings is `failed`; every `ModifiedTestsError` is `blocked`,
/// never `passed`". This type makes that mapping a
/// [`crate::runner::Runner`] rather than a comment describing one.
#[derive(Clone, Debug)]
pub struct ModifiedTestsRunner {
    number: GateNumber,
    changes: Vec<FileChange>,
}

impl ModifiedTestsRunner {
    /// A modified-test runner over an already-gathered change set, answering
    /// as `number`.
    ///
    /// `changes` is gathered by the caller, with
    /// `crate::modified_tests::changes_from_git` or otherwise, before this is
    /// built: see this module's head comment on why that IO stays outside
    /// `run`.
    #[must_use]
    pub fn new(number: GateNumber, changes: Vec<FileChange>) -> Self {
        Self { number, changes }
    }
}

impl Runner for ModifiedTestsRunner {
    fn number(&self) -> GateNumber {
        self.number
    }

    fn run(&self) -> RunReport {
        match modified_tests::scan(&self.changes) {
            Ok(report) => {
                if report.modified_an_existing_test() {
                    RunReport {
                        outcome: Outcome::Failed,
                        detail: report.lines().join("; "),
                    }
                } else {
                    RunReport {
                        outcome: Outcome::Passed,
                        detail: report.lines().join("; "),
                    }
                }
            }
            // An error is nothing checked, never a pass: see this trait's
            // `run` doc, and crate::modified_tests's own stated mapping.
            Err(error) => RunReport {
                outcome: Outcome::Blocked,
                detail: error.to_string(),
            },
        }
    }
}

/// The three states a listed gate can be in: `spec/DATA_MODEL.md` section 2's
/// `Gate.state`, minus `Inert`, which [`crate::gate`]'s head comment explains
/// is out of this module's scope (it needs a `GateRun`, which nothing here
/// tracks).
///
/// Only [`crate::runner::Registry::list`] produces [`GateState::Installed`]:
/// see [`crate::gate::DefinitionState`] for why a [`crate::gate::GateDef`]
/// alone cannot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateState {
    /// No proof is attached. `gates.list` must show this, never `Installed`,
    /// for a gate with no [`crate::gate::GateProof`]: ORI-P1-012.
    Defined,
    /// A proof is attached, and no [`crate::runner::Runner`] is registered
    /// for it.
    Proven,
    /// A proof is attached, and a [`crate::runner::Runner`] is registered:
    /// `spec/DATA_MODEL.md` section 2, "`installed` requires a `GateProof`".
    Installed,
}

impl fmt::Display for GateState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Defined => f.write_str("Defined"),
            Self::Proven => f.write_str("Proven"),
            Self::Installed => f.write_str("Installed"),
        }
    }
}

/// The honest [`GateState`] of one [`crate::gate::GateDef`], given whether a
/// [`crate::runner::Runner`] is registered for it.
///
/// A free function rather than a method on anything, so that every planted
/// defect below that targets this rule mutates exactly one thing: this
/// function's body.
#[must_use]
fn honest_state(def: &GateDef, has_runner: bool) -> GateState {
    match (def.state(), has_runner) {
        (DefinitionState::Defined, _) => GateState::Defined,
        (DefinitionState::Proven, false) => GateState::Proven,
        (DefinitionState::Proven, true) => GateState::Installed,
    }
}

/// One line of `gates.list`: `spec/DATA_MODEL.md` section 2, criterion
/// ORI-P1-012's "`gates.list` shows Defined, not Installed".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateListing {
    /// This gate's position in `spec/CI_CD.md` section 1's numbered pipeline.
    pub number: GateNumber,
    /// What kind of check this gate is.
    pub kind: GateKind,
    /// The short human label this gate is listed under.
    pub name: String,
    /// The gate's honest state: never `Installed` without a proof.
    pub state: GateState,
}

impl fmt::Display for GateListing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({}): {}",
            self.number, self.name, self.kind, self.state
        )
    }
}

struct RegistryEntry {
    def: GateDef,
    runner: Option<Box<dyn Runner>>,
}

/// The set of gates this product knows about, and what `gates.list` reads:
/// `spec/DATA_MODEL.md` section 2.
///
/// Holds at most one entry per [`crate::gate::GateNumber`]: two entries
/// claiming the same number would make `gates.list` print two different
/// answers for what `ops/gates/gate-N.md` records as one gate, which is a
/// narrower case of the "present but reporting nothing" defect class AICD
/// §39 names (here, "present and reporting two things").
#[derive(Default)]
pub struct Registry {
    entries: Vec<RegistryEntry>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one gate, with an optional runner: `spec/DATA_MODEL.md` section 2.
    ///
    /// # Errors
    ///
    /// [`RegistryError::NumberAlreadyInstalled`] if `def.number()` is already
    /// held. [`RegistryError::RunnerNumberMismatch`] if `runner` answers for a
    /// different [`crate::gate::GateNumber`] than `def` does: a runner
    /// registered under the wrong gate's entry would make `gates.list` credit
    /// one gate with a check that actually verifies another.
    pub fn install(
        &mut self,
        def: GateDef,
        runner: Option<Box<dyn Runner>>,
    ) -> Result<(), RegistryError> {
        if self
            .entries
            .iter()
            .any(|entry| entry.def.number() == def.number())
        {
            return Err(RegistryError::NumberAlreadyInstalled {
                number: def.number(),
            });
        }
        if let Some(runner) = &runner
            && runner.number() != def.number()
        {
            return Err(RegistryError::RunnerNumberMismatch {
                def: def.number(),
                runner: runner.number(),
            });
        }
        self.entries.push(RegistryEntry { def, runner });
        Ok(())
    }

    /// Whether this registry holds any gate at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many gates this registry holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `gates.list`: every gate this registry holds, with its honest state.
    ///
    /// Ordered by [`crate::gate::GateNumber`], so two runs over the same
    /// registry print the same order.
    #[must_use]
    pub fn list(&self) -> Vec<GateListing> {
        let mut out: Vec<GateListing> = self
            .entries
            .iter()
            .map(|entry| GateListing {
                number: entry.def.number(),
                kind: entry.def.kind(),
                name: entry.def.name().to_owned(),
                state: honest_state(&entry.def, entry.runner.is_some()),
            })
            .collect();
        out.sort_by_key(|listing| listing.number.get());
        out
    }

    /// Refuses to certify an empty registry: ORI-P1-012, criterion tests item
    /// 6.
    ///
    /// # Errors
    ///
    /// [`AuditError::Empty`] when this registry holds no gate. A check that
    /// asks "does any gate here wrongly claim `Installed`" over zero gates is
    /// vacuously true, and a citation gate that read that as "every gate here
    /// is proven" would be exactly the "present but reporting nothing" defect
    /// AICD §39 names, one level up: the registry itself, not a single gate,
    /// would be present and reporting nothing.
    pub fn audit(&self) -> Result<(), AuditError> {
        if self.entries.is_empty() {
            return Err(AuditError::Empty);
        }
        Ok(())
    }

    /// Every gate whose [`crate::runner::Runner`] is registered: the input to
    /// actually running the pipeline, as opposed to [`Self::list`], which
    /// only reports state.
    pub fn runners(&self) -> impl Iterator<Item = &dyn Runner> {
        self.entries
            .iter()
            .filter_map(|entry| entry.runner.as_deref())
    }
}

/// Why [`Registry::install`] refused a gate: `spec/DATA_MODEL.md` section 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// A gate with this number is already installed.
    NumberAlreadyInstalled {
        /// The number already held.
        number: GateNumber,
    },
    /// The runner answers for a different gate than the one it was paired
    /// with.
    RunnerNumberMismatch {
        /// The gate the caller tried to pair the runner with.
        def: GateNumber,
        /// The gate the runner actually answers for.
        runner: GateNumber,
    },
}

impl RegistryError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute
    /// rule 9).
    #[must_use]
    pub const fn methodology_ref(&self) -> &'static str {
        // AICD §39: two entries or a runner mismatched to its gate is a
        // reference that resolves to the wrong thing, checked mechanically.
        "AICD §39"
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NumberAlreadyInstalled { number } => {
                write!(f, "{number} is already installed in this registry")
            }
            Self::RunnerNumberMismatch { def, runner } => write!(
                f,
                "a runner for {runner} was paired with the entry for {def}: gates.list would \
                 credit {def} with a check that actually verifies {runner}"
            ),
        }
    }
}

impl Error for RegistryError {}

/// Why [`Registry::audit`] refused: ORI-P1-012.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditError {
    /// The registry holds no gate.
    Empty,
}

impl AuditError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute
    /// rule 9).
    #[must_use]
    pub const fn methodology_ref(&self) -> &'static str {
        "AICD §14"
    }
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str(
                "this registry holds no gate, so every claim about what it proves is vacuous. \
                 refused rather than reported as clean",
            ),
        }
    }
}

impl Error for AuditError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{GateKind, GateNumber, GateProof};

    fn number(value: u8) -> GateNumber {
        GateNumber::new(value).expect("test values are all within 1..=14")
    }

    fn proven_def(gate_number: u8, kind: GateKind, name: &str) -> GateDef {
        let proof = GateProof::new(
            format!("fixtures/planted/gate-{gate_number}"),
            format!("ops/gates/gate-{gate_number}.md"),
            1_700_000_000,
            1_700_000_060,
        )
        .expect("both references are non-empty");
        GateDef::new(number(gate_number), kind, name, "spec/CI_CD.md section 1").with_proof(proof)
    }

    struct StubRunner {
        number: GateNumber,
    }

    impl Runner for StubRunner {
        fn number(&self) -> GateNumber {
            self.number
        }

        fn run(&self) -> RunReport {
            RunReport {
                outcome: Outcome::Passed,
                detail: "stub".to_owned(),
            }
        }
    }

    #[test]
    fn ori_p1_012_no_proof_reports_defined_even_with_a_runner_registered() {
        let def = GateDef::new(
            number(9),
            GateKind::Citation,
            "citation gate",
            "spec/CI_CD.md section 1",
        );
        let mut registry = Registry::new();
        registry
            .install(def, Some(Box::new(StubRunner { number: number(9) })))
            .expect("a fresh registry accepts the first entry");
        let listing = &registry.list()[0];
        assert_eq!(
            listing.state,
            GateState::Defined,
            "ORI-P1-012: a gate with no proof must report Defined, never Installed, whatever the \
             runner wiring is"
        );
    }

    #[test]
    fn ori_p1_012_proof_without_a_runner_reports_proven_not_installed() {
        let def = proven_def(4, GateKind::CoverageMatrix, "coverage matrix");
        let mut registry = Registry::new();
        registry
            .install(def, None)
            .expect("a fresh registry accepts the first entry");
        assert_eq!(registry.list()[0].state, GateState::Proven);
    }

    #[test]
    fn ori_p1_012_proof_with_a_runner_reports_installed() {
        let def = proven_def(4, GateKind::CoverageMatrix, "coverage matrix");
        let mut registry = Registry::new();
        registry
            .install(def, Some(Box::new(StubRunner { number: number(4) })))
            .expect("a fresh registry accepts the first entry");
        assert_eq!(registry.list()[0].state, GateState::Installed);
    }

    #[test]
    fn ori_p1_012_a_mixed_registry_reports_each_gate_honestly() {
        // Planted defects 4 and 5 both mutate honest_state to a constant
        // answer. This one test fails on either constant, because it holds
        // one gate that must never read Installed and one that must never
        // read Defined.
        let mut registry = Registry::new();
        registry
            .install(
                GateDef::new(
                    number(9),
                    GateKind::Citation,
                    "citation gate",
                    "spec/CI_CD.md section 1",
                ),
                None,
            )
            .expect("a fresh registry accepts the first entry");
        registry
            .install(
                proven_def(4, GateKind::CoverageMatrix, "coverage matrix"),
                Some(Box::new(StubRunner { number: number(4) })),
            )
            .expect("gate 4 was not yet installed");
        let listings = registry.list();
        let gate_9 = listings
            .iter()
            .find(|l| l.number == number(9))
            .expect("gate 9 listed");
        let gate_4 = listings
            .iter()
            .find(|l| l.number == number(4))
            .expect("gate 4 listed");
        assert_eq!(gate_9.state, GateState::Defined);
        assert_eq!(gate_4.state, GateState::Installed);
    }

    #[test]
    fn ori_p1_012_registry_refuses_a_runner_paired_with_the_wrong_gate() {
        let def = GateDef::new(
            number(5),
            GateKind::ModifiedTests,
            "modified tests",
            "spec/CI_CD.md section 1",
        );
        let mut registry = Registry::new();
        let result = registry.install(def, Some(Box::new(StubRunner { number: number(9) })));
        assert!(matches!(
            result,
            Err(RegistryError::RunnerNumberMismatch { .. })
        ));
    }

    #[test]
    fn ori_p1_012_empty_registry_refuses_audit_rather_than_passing_vacuously() {
        // Planted defect 6. A naive check phrased as "no entry violates the
        // rule" is true of an empty registry by definition, which is the trap
        // this test is against: Registry::audit must refuse instead.
        let registry = Registry::new();
        assert!(registry.is_empty());
        assert!(
            matches!(registry.audit(), Err(AuditError::Empty)),
            "an empty registry must not be certified as honest"
        );
    }

    #[test]
    fn ori_p1_012_coverage_runner_maps_a_read_error_to_blocked_never_passed() {
        let empty_root = std::env::temp_dir().join("ori-gates-ori-p1-012-empty-root-probe");
        let _ = std::fs::create_dir_all(&empty_root);
        let runner = CoverageRunner::new(number(4), empty_root);
        let report = runner.run();
        assert_eq!(
            report.outcome,
            Outcome::Blocked,
            "an unreadable coverage matrix must report blocked, not passed or failed: {}",
            report.detail
        );
    }

    #[test]
    fn ori_p1_012_modified_tests_runner_maps_an_empty_change_set_to_blocked() {
        // Planted defect 3's real-code demonstration: crate::modified_tests
        // deterministically refuses an empty change set
        // (ModifiedTestsError::NothingToCheck). The wrapper must report
        // Blocked, never Passed, for that refusal.
        let runner = ModifiedTestsRunner::new(number(5), Vec::new());
        let report = runner.run();
        assert_eq!(report.outcome, Outcome::Blocked, "{}", report.detail);
    }

    #[test]
    fn ori_p1_012_modified_tests_runner_reports_failed_on_a_weakened_test() {
        let before = "//! demo\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0041_fixture() {\n        assert!(1 + 1 == 2);\n    }\n}\n";
        let after = "//! demo\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0041_fixture() {\n    }\n}\n";
        let changes = vec![FileChange::modified(
            "crates/demo/src/lib.rs",
            before,
            after,
        )];
        let runner = ModifiedTestsRunner::new(number(5), changes);
        let report = runner.run();
        assert_eq!(report.outcome, Outcome::Failed, "{}", report.detail);
    }

    #[test]
    fn ori_t_0041_registry_refuses_a_second_entry_for_the_same_number() {
        let mut registry = Registry::new();
        registry
            .install(
                GateDef::new(number(1), GateKind::Lint, "fmt", "spec/CI_CD.md section 1"),
                None,
            )
            .expect("first install");
        let second = registry.install(
            GateDef::new(
                number(1),
                GateKind::Types,
                "clippy",
                "spec/CI_CD.md section 1",
            ),
            None,
        );
        assert!(matches!(
            second,
            Err(RegistryError::NumberAlreadyInstalled { .. })
        ));
    }

    #[test]
    fn ori_t_0041_runners_iterates_only_installed_runners() {
        let mut registry = Registry::new();
        registry
            .install(
                GateDef::new(
                    number(9),
                    GateKind::Citation,
                    "citation gate",
                    "spec/CI_CD.md section 1",
                ),
                None,
            )
            .expect("no runner yet for gate 9");
        registry
            .install(
                GateDef::new(
                    number(4),
                    GateKind::CoverageMatrix,
                    "coverage matrix",
                    "spec/CI_CD.md section 1",
                ),
                Some(Box::new(StubRunner { number: number(4) })),
            )
            .expect("gate 4 has a runner");
        assert_eq!(registry.runners().count(), 1);
    }
}
