//! The prover: AICD §14's planted-defect demonstration, as code.
//!
//! AICD §14: "A gate that has only ever been observed passing is not installed.
//! Every gate ... enters service only after a demonstration on a planted defect:
//! it passes on a clean tree, it fails on the planted defect, and the failure is
//! visible where a human would look." `spec/runbooks/prove-gate.md` turns that
//! into five steps, and steps 2 to 4 are what [`crate::prover::Prover`] does: run
//! the gate against the clean input and expect a pass, run it against the planted
//! defect and expect a failure, and produce the [`crate::gate::GateProof`] that
//! `spec/DATA_MODEL.md` section 3 requires before a gate can leave `Defined`.
//! `spec/LLD.md` section 2 gives this crate `Prover` and a must-not column that
//! reads "Report a gate installed without a proof".
//!
//! A proof is produced in exactly one case: the clean run is
//! [`crate::runner::Outcome::Passed`] and the dirty run is
//! [`crate::runner::Outcome::Failed`]. Every other pair of outcomes is a refusal
//! ([`crate::prover::ProveError`]) that carries the run report that caused it, so
//! the reason is in the error a human reads and not only in a return code.
//!
//! ```mermaid
//! flowchart TD
//!   A[prove: gate definition, clean runner, dirty runner, two references] --> R{both references<br/>non-blank?}
//!   R -- no --> X1[ReferenceMissing]
//!   R -- yes --> N{both runners answer<br/>for the definition's number?}
//!   N -- no --> X2[RunnerNumberMismatch]
//!   N -- yes --> C[run the clean runner]
//!   C --> CO{Passed?}
//!   CO -- Failed or Blocked --> X3[CleanRunNotPassed]
//!   CO -- Passed --> T1[read the clock: passed_clean_at]
//!   T1 --> D[run the dirty runner]
//!   D --> DO{Failed?}
//!   DO -- Passed or Blocked --> X4[DirtyRunNotFailed]
//!   DO -- Failed --> T2[read the clock: failed_dirty_at]
//!   T2 --> P[Proven: a complete GateProof and both run reports]
//! ```
//!
//! # How the prover obtains a runner per input
//!
//! [`crate::runner::Runner::run`] takes no argument: each runner is built over
//! the one input it checks (a product root, a change set, a git range), as
//! `crate::runner`'s own head comment explains. So there is no `prove(gate,
//! input)` here, because a prover that took an input could only hand it to a
//! runner by a side channel. The caller builds **two runners of the same gate,
//! one over the clean input and one over the planted-defect input**, and hands
//! both to [`crate::prover::Prover::prove`] in a [`crate::prover::ProofAttempt`].
//! Three reasons, in order of weight:
//!
//! 1. The prover reads no file. Only `ori-watch` reads the working tree outside
//!    a session (`CLAUDE.md`, load-bearing facts), and gathering input is the
//!    caller's job for every runner in this crate. A prover that opened
//!    `fixtures/planted/` itself would be a second reader of the tree.
//! 2. Building the runner is where the heterogeneity is real. A coverage runner
//!    wants a root, a modified-test runner wants a change set. A factory trait
//!    here would only move that code to a place that knows less about it.
//! 3. The runner the prover runs is the runner the caller will register. The
//!    caller proves with the very value it then passes to
//!    [`crate::runner::Registry::install`], so the thing demonstrated and the
//!    thing installed cannot drift apart inside this module.
//!
//! What follows from the choice, stated so it is not mistaken for a guarantee:
//! the prover cannot see which input either runner was built over. That the
//! "clean" runner really was given a clean tree and the "dirty" one really was
//! given `planted_defect_ref` is the caller's claim, recorded in the proof and
//! checkable by a human who opens the reference. The prover checks only that the
//! reference is present, that both runners answer for the gate being proven, and
//! that the two verdicts are the two the rule requires. It also does not run a
//! runner twice, so a runner that flips between verdicts on identical input
//! would be proven by one lucky pair; `spec/runbooks/prove-gate.md`'s
//! verification line ("`gates.prove` re-run reproduces both results") is how a
//! flaky gate is caught, and it is a second call by the caller.
//!
//! # Where the proof goes: returned, not appended
//!
//! `spec/API_SPEC.md` section 1 says `gates.prove({gate_id})` returns "GateProof
//! or failure", and ORI-P1-013 says "proof stored with evidence refs". The
//! prover does **not** append to the event log; it returns a
//! [`crate::prover::Proven`] for the caller to store. Four reasons:
//!
//! 1. `spec/LLD.md` section 5: "The event log is written by a single writer
//!    task; all mutations are messages to it, so ordering and the hash chain are
//!    guaranteed." A library function in this crate that took a connection and
//!    appended would be a second writer path around that task.
//! 2. The prover does not know who is proving. `spec/DATA_MODEL.md` section 4
//!    admits the `system` actor only for scheduled triggers and watchers, and
//!    proving is a coder's act accepted by the verification lead, so the actor
//!    is the caller's to name.
//! 3. [`crate::gate::GateDef`] carries a [`crate::gate::GateNumber`] and no
//!    ULID, and no product (`crate::gate`'s head comment says why). The
//!    `gate_id` that `spec/DATA_MODEL.md` section 2 gives `GateProof` is a
//!    storage-layer fact this crate does not hold, so an event written here
//!    could not be keyed the way the entity is.
//! 4. No document gives the payload of a proof event and no projection reads
//!    one. Hand-writing JSON here, with a manifest that carries no JSON crate
//!    and a scope that does not include the manifest, would fix a contract that
//!    belongs to whoever writes the projection.
//!
//! The event kind the writer should use is one the specification already names:
//! [`crate::prover::GATE_PROVEN_EVENT_KIND`], `gate.proven`, which `spec/LLD.md`
//! section 4 gives as its example of the `domain.verb_past` naming rule and
//! `spec/design/DESIGN.md` lists as the event the readiness screen subscribes
//! to. `spec/API_SPEC.md` section 2 lists `gate.run` and `gate.inert` among the
//! kinds and not `gate.proven`; the list there is introduced by "Kinds
//! include", so it is not closed, but it is a gap in that document that this
//! module reports rather than papers over.
//!
//! A [`crate::prover::Proven`] holds everything the runbook's step 4, "Store the
//! GateProof with both run references", needs: the proof with its two
//! references and two timestamps, and the clean and dirty
//! [`crate::runner::RunReport`], whose `detail` lines are the run references a
//! human reads. Its fields are private and only [`crate::prover::Prover::prove`]
//! builds one, so a `Proven` in hand is evidence that a clean pass and a dirty
//! failure were both seen by this code.
//!
//! # The rollback line, and whose it is
//!
//! `spec/runbooks/prove-gate.md`: "Rollback: revoke the proof (state back to
//! Defined) if the gate is changed; re-prove." In memory that is
//! [`crate::prover::revoke`], which returns the same definition with no proof,
//! after which [`crate::prover::Prover::prove`] and
//! [`crate::prover::Proven::attach_to`] run again. Two parts are **not** this
//! module's:
//!
//! - [`crate::runner::Registry`] has no way to replace or remove an entry, so a
//!   revoked definition reaches `gates.list` by building a new registry from the
//!   revoked definition. That is `crate::runner`'s (ORI-T-0041).
//! - Revoking durably is an event, because the log is append-only and a stored
//!   proof is never edited. Its kind is not named by any document, and the
//!   projection that would turn `gate.proven` and a revocation into the `Gate`
//!   row's state does not exist; `crates/ori-store/src/projections/` holds
//!   ticket, escalation and lock projections only. Whichever ticket writes the
//!   gate projection and wires `gates.prove` owns it. The backlog's batch 13
//!   (`ops/phase-1-backlog.md`) names no such ticket, and this module reports
//!   that gap.
//!
//! # What this module cannot enforce
//!
//! [`crate::gate::GateProof::new`] and [`crate::gate::GateDef::with_proof`] are
//! public, and a proof carries no gate number, so code that never calls the
//! prover can still build a complete-looking proof and attach it. Storage needs
//! that constructor (a proof read back from the log must be rebuilt), so
//! sealing it is a design question for `crate::gate`, which is outside this
//! module's scope. What this module does is make the honest path hard to get
//! wrong: [`crate::prover::Proven::attach_to`] refuses a definition for a
//! different gate.
//!
//! # Why the links here are written `crate::prover::`
//!
//! `lib.rs` carries a `///` on this module's declaration, so rustdoc resolves
//! this file's `//!` links in the crate root's scope rather than this module's.
//! `crate::gate` and `crate::runner` record the same trap. Do not shorten these,
//! and do not link a private item from a public item's docs.
//!
//! Must not: report a gate installed without a proof (`spec/LLD.md` section 2),
//! add a dependency (`CLAUDE.md` absolute rule 6), read a file, or append to the
//! event log. Nothing outside `std` and this crate's own modules is used, apart
//! from `ori_core::error::MethodologyRef`, which the refusals carry.

use std::error::Error;
use std::fmt;

use ori_core::error::MethodologyRef;

use crate::gate::{GateDef, GateNumber, GateProof, GateProofError};
use crate::runner::{Outcome, RunReport, Runner};

/// The event kind a gate's proof is recorded under: `gate.proven`.
///
/// AICD §14 requires the proof to exist before a gate is cited as protection;
/// this is the name the writer of the event log gives the event that records it.
/// The kind is the one `spec/LLD.md` section 4 gives as its example of the
/// `domain.verb_past` rule and `spec/design/DESIGN.md` lists for the readiness
/// screen. This module never appends it (see the head comment, "Where the proof
/// goes"); the constant exists so the one caller that does append cannot spell
/// it two ways.
pub const GATE_PROVEN_EVENT_KIND: &str = "gate.proven";

/// Which of the two references a proof needs: AICD §14.
///
/// `spec/TESTING.md` section 4 requires both a planted defect under
/// `fixtures/planted/` and a record under `ops/gates/`; an unnamed one is not a
/// proof anyone can check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofReference {
    /// Where the planted defect lives, `fixtures/planted/` by
    /// `spec/TESTING.md` section 4.
    PlantedDefect,
    /// Where the proof is recorded, `ops/gates/` by `spec/TESTING.md` section 4.
    Evidence,
}

impl fmt::Display for ProofReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlantedDefect => f.write_str("planted-defect reference"),
            Self::Evidence => f.write_str("evidence reference"),
        }
    }
}

/// Everything one proof attempt needs besides the gate's definition: AICD §14.
///
/// The two runners are two instances of the same gate, one built over the clean
/// input and one over the planted-defect input; the head comment, "How the
/// prover obtains a runner per input", says why the caller builds them. The
/// fields are named, not positional, so that the clean and the dirty runner are
/// not swapped by a call site. A swap would be refused anyway (the clean run
/// would fail), but a refusal that says the wrong thing is a worse message.
pub struct ProofAttempt<'a> {
    /// The runner built over the clean input. It must report
    /// [`Outcome::Passed`].
    pub clean: &'a dyn Runner,
    /// The runner built over the planted-defect input. It must report
    /// [`Outcome::Failed`].
    pub dirty: &'a dyn Runner,
    /// Where the planted defect lives, relative to the product root. Becomes
    /// [`GateProof::planted_defect_ref`].
    pub planted_defect_ref: &'a str,
    /// Where the proof is recorded, relative to the product root. Becomes
    /// [`GateProof::evidence_ref`].
    pub evidence_ref: &'a str,
}

/// A successful demonstration: a complete proof and the two runs behind it.
///
/// AICD §14. Only [`crate::prover::Prover::prove`] builds one, and it builds one
/// only after a clean pass and a dirty failure, so holding a value of this type
/// is evidence of both.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proven {
    number: GateNumber,
    proof: GateProof,
    clean: RunReport,
    dirty: RunReport,
}

impl Proven {
    /// The gate this demonstration was made for.
    #[must_use]
    pub const fn number(&self) -> GateNumber {
        self.number
    }

    /// The proof to store, `spec/runbooks/prove-gate.md` step 4: AICD §14.
    #[must_use]
    pub const fn proof(&self) -> &GateProof {
        &self.proof
    }

    /// What the clean run reported: the run reference for the pass.
    #[must_use]
    pub const fn clean(&self) -> &RunReport {
        &self.clean
    }

    /// What the dirty run reported: the run reference for the failure a human
    /// should be able to read.
    #[must_use]
    pub const fn dirty(&self) -> &RunReport {
        &self.dirty
    }

    /// The definition with this proof attached: `spec/DATA_MODEL.md` section 3,
    /// `Defined -> Proven (GateProof present)`.
    ///
    /// Returns a new definition and leaves `def` untouched, so a caller that
    /// holds the old one decides when to replace it. A definition that already
    /// carries a proof gets this one in its place.
    ///
    /// # Errors
    ///
    /// [`ProveError::ProofForAnotherGate`] when `def` is not the gate this
    /// demonstration was made for: a proof carries no gate number of its own, so
    /// attaching it to the wrong definition would credit one gate with another's
    /// demonstration (AICD §14).
    pub fn attach_to(&self, def: &GateDef) -> Result<GateDef, ProveError> {
        if def.number() != self.number {
            return Err(ProveError::ProofForAnotherGate {
                proof: self.number,
                def: def.number(),
            });
        }
        Ok(def.clone().with_proof(self.proof.clone()))
    }
}

/// Why a gate was not proven: AICD §14.
///
/// Every variant is a refusal to produce a proof, and none maps to one. The two
/// run-related variants carry the [`RunReport`] that decided them, so what a
/// human reads includes the runner's own account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProveError {
    /// A reference the proof needs was empty or blank. Nothing was run.
    ReferenceMissing {
        /// Which reference.
        which: ProofReference,
    },
    /// A runner answers for a different gate than the definition being proven.
    /// Nothing was run.
    RunnerNumberMismatch {
        /// The gate being proven.
        def: GateNumber,
        /// The gate the clean runner answers for.
        clean: GateNumber,
        /// The gate the dirty runner answers for.
        dirty: GateNumber,
    },
    /// The clean run did not pass: it failed, or it was blocked. The dirty
    /// runner was not run.
    CleanRunNotPassed {
        /// The gate being proven.
        gate: GateNumber,
        /// What the clean run reported.
        run: RunReport,
    },
    /// The dirty run did not fail: it passed, or it was blocked. A blocked run
    /// is the "present but reporting nothing" case and never counts as a
    /// failure on the defect.
    DirtyRunNotFailed {
        /// The gate being proven.
        gate: GateNumber,
        /// What the dirty run reported.
        run: RunReport,
    },
    /// [`Proven::attach_to`] was given a definition for a different gate.
    ProofForAnotherGate {
        /// The gate the demonstration was made for.
        proof: GateNumber,
        /// The gate of the definition it was offered to.
        def: GateNumber,
    },
}

impl ProveError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute
    /// rule 9): AICD §14, whose rule a gate is installed only when it has been
    /// seen to fail is what every variant protects.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        MethodologyRef {
            section: 14,
            subsection: None,
        }
    }

    /// The run that decided this refusal, for the variants a run decides.
    #[must_use]
    pub const fn run(&self) -> Option<&RunReport> {
        match self {
            Self::CleanRunNotPassed { run, .. } | Self::DirtyRunNotFailed { run, .. } => Some(run),
            Self::ReferenceMissing { .. }
            | Self::RunnerNumberMismatch { .. }
            | Self::ProofForAnotherGate { .. } => None,
        }
    }
}

impl From<GateProofError> for ProveError {
    /// A proof the constructor itself refused, as a refusal of the attempt.
    ///
    /// [`Prover::prove`] checks both references before it runs anything, with a
    /// stricter test than the constructor's (blank, not only empty), so this
    /// conversion is the constructor's own refusal kept honest rather than
    /// unwrapped.
    fn from(error: GateProofError) -> Self {
        match error {
            GateProofError::EmptyPlantedDefectRef => Self::ReferenceMissing {
                which: ProofReference::PlantedDefect,
            },
            GateProofError::EmptyEvidenceRef => Self::ReferenceMissing {
                which: ProofReference::Evidence,
            },
        }
    }
}

impl fmt::Display for ProveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = self.methodology_ref();
        match self {
            Self::ReferenceMissing { which } => write!(
                f,
                "refused ({reason}): the {which} is empty, and a proof that names nowhere to \
                 check it is not a proof. Nothing was run"
            ),
            Self::RunnerNumberMismatch { def, clean, dirty } => write!(
                f,
                "refused ({reason}): proving {def}, but the clean runner answers for {clean} and \
                 the dirty runner for {dirty}. A demonstration of another gate proves nothing \
                 here. Nothing was run"
            ),
            Self::CleanRunNotPassed { gate, run } => {
                let why = match run.outcome {
                    Outcome::Failed => {
                        "it failed on the clean input, so it cannot be told apart from a gate \
                         that fails on everything"
                    }
                    Outcome::Blocked => {
                        "it was blocked on the clean input, so nothing was checked and a gate \
                         that could not run proves nothing"
                    }
                    Outcome::Passed => "it passed, which is not a refusal",
                };
                write!(
                    f,
                    "refused ({reason}): {gate} is not proven, {why}. The runner said: {}",
                    run.detail
                )
            }
            Self::DirtyRunNotFailed { gate, run } => {
                let why = match run.outcome {
                    Outcome::Passed => {
                        "it passed on the planted defect, so it cannot see the defect: present \
                         but reporting nothing"
                    }
                    Outcome::Blocked => {
                        "it was blocked on the planted defect, so nothing was checked. A gate \
                         that could not run did not fail on the defect: present but reporting \
                         nothing"
                    }
                    Outcome::Failed => "it failed, which is not a refusal",
                };
                write!(
                    f,
                    "refused ({reason}): {gate} is not proven, {why}. The runner said: {}",
                    run.detail
                )
            }
            Self::ProofForAnotherGate { proof, def } => write!(
                f,
                "refused ({reason}): this demonstration was made for {proof} and the definition \
                 offered is {def}"
            ),
        }
    }
}

impl Error for ProveError {}

/// The planted-defect demonstration: `spec/LLD.md` section 2, AICD §14.
///
/// Holds the clock the proof's two timestamps come from, so that no time is
/// read here: `ori-store`'s `ProductDb::open` takes its time as a parameter for
/// the same reason, and a test of this module never depends on when it ran. The
/// clock is read once after each run that is accepted and never otherwise, so a
/// refused attempt invents no timestamp.
pub struct Prover<C> {
    clock: C,
}

impl<C: FnMut() -> i64> Prover<C> {
    /// A prover that stamps a proof with `clock`, which answers in Unix seconds,
    /// UTC, the unit [`GateProof::passed_clean_at`] documents: AICD §14.
    pub const fn new(clock: C) -> Self {
        Self { clock }
    }

    /// Demonstrates one gate on a clean input and on its planted defect:
    /// AICD §14, `spec/runbooks/prove-gate.md` steps 2 to 4.
    ///
    /// The checks that need no run come first, so a refused attempt has run
    /// nothing. The dirty runner is run only after the clean one passed.
    ///
    /// # Errors
    ///
    /// [`ProveError::ReferenceMissing`] when either reference is empty or blank;
    /// [`ProveError::RunnerNumberMismatch`] when either runner answers for a
    /// gate other than `def`; [`ProveError::CleanRunNotPassed`] when the clean
    /// run is not [`Outcome::Passed`]; [`ProveError::DirtyRunNotFailed`] when the
    /// dirty run is not [`Outcome::Failed`]. All of them cite AICD §14.
    pub fn prove(
        &mut self,
        def: &GateDef,
        attempt: &ProofAttempt<'_>,
    ) -> Result<Proven, ProveError> {
        let planted_defect_ref =
            require_reference(ProofReference::PlantedDefect, attempt.planted_defect_ref)?;
        let evidence_ref = require_reference(ProofReference::Evidence, attempt.evidence_ref)?;

        let number = def.number();
        let clean_number = attempt.clean.number();
        let dirty_number = attempt.dirty.number();
        if clean_number != number || dirty_number != number {
            return Err(ProveError::RunnerNumberMismatch {
                def: number,
                clean: clean_number,
                dirty: dirty_number,
            });
        }

        let clean = attempt.clean.run();
        // Every outcome is named: a variant added to `Outcome` later must be
        // decided here by a person, not defaulted into a proof.
        match clean.outcome {
            Outcome::Passed => {}
            Outcome::Failed | Outcome::Blocked => {
                return Err(ProveError::CleanRunNotPassed {
                    gate: number,
                    run: clean,
                });
            }
        }
        let passed_clean_at = (self.clock)();

        let dirty = attempt.dirty.run();
        match dirty.outcome {
            Outcome::Failed => {}
            Outcome::Passed | Outcome::Blocked => {
                return Err(ProveError::DirtyRunNotFailed {
                    gate: number,
                    run: dirty,
                });
            }
        }
        let failed_dirty_at = (self.clock)();

        let proof = GateProof::new(
            planted_defect_ref,
            evidence_ref,
            passed_clean_at,
            failed_dirty_at,
        )?;
        Ok(Proven {
            number,
            proof,
            clean,
            dirty,
        })
    }
}

/// The reference, or the refusal when it is empty or only whitespace.
fn require_reference(which: ProofReference, value: &str) -> Result<&str, ProveError> {
    if value.trim().is_empty() {
        return Err(ProveError::ReferenceMissing { which });
    }
    Ok(value)
}

/// The same gate with its proof taken away: the rollback of
/// `spec/runbooks/prove-gate.md`, "revoke the proof (state back to Defined) if
/// the gate is changed; re-prove" (AICD §14).
///
/// Built from the definition's four public facts, because [`GateDef`] has no
/// method that removes a proof. A field added to [`GateDef`] later would be
/// dropped here without a compile error; the test that compares a revoked
/// definition with a freshly built one is what fails when that happens.
///
/// This is the in-memory half of a rollback. The durable half is an event, which
/// is not this module's (see the head comment, "The rollback line, and whose it
/// is").
#[must_use]
pub fn revoke(def: &GateDef) -> GateDef {
    GateDef::new(def.number(), def.kind(), def.name(), def.spec_ref())
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::gate::{DefinitionState, GateKind};
    use crate::modified_tests::FileChange;
    use crate::runner::{GateState, ModifiedTestsRunner, Registry};

    fn number(value: u8) -> GateNumber {
        GateNumber::new(value).expect("test values are all within 1..=14")
    }

    fn definition(gate_number: u8) -> GateDef {
        GateDef::new(
            number(gate_number),
            GateKind::ModifiedTests,
            "modified tests",
            "spec/CI_CD.md section 1",
        )
    }

    /// A runner that always reports the one outcome it was given, and counts
    /// how many times it was asked.
    struct Scripted {
        number: GateNumber,
        outcome: Outcome,
        detail: &'static str,
        runs: Cell<u32>,
    }

    impl Scripted {
        fn new(gate_number: u8, outcome: Outcome, detail: &'static str) -> Self {
            Self {
                number: number(gate_number),
                outcome,
                detail,
                runs: Cell::new(0),
            }
        }
    }

    impl Runner for Scripted {
        fn number(&self) -> GateNumber {
            self.number
        }

        fn run(&self) -> RunReport {
            self.runs.set(self.runs.get() + 1);
            RunReport {
                outcome: self.outcome,
                detail: self.detail.to_owned(),
            }
        }
    }

    fn attempt<'a>(clean: &'a Scripted, dirty: &'a Scripted) -> ProofAttempt<'a> {
        ProofAttempt {
            clean,
            dirty,
            planted_defect_ref: "fixtures/planted/gate-5",
            evidence_ref: "ops/gates/gate-5.md",
        }
    }

    /// A clock that answers 100, 200, 300 and so on, and counts its readings.
    fn counting_clock(reads: &Cell<u32>) -> impl FnMut() -> i64 + '_ {
        move || {
            reads.set(reads.get() + 1);
            i64::from(reads.get()) * 100
        }
    }

    const ALL_OUTCOMES: [Outcome; 3] = [Outcome::Passed, Outcome::Failed, Outcome::Blocked];

    #[test]
    fn ori_p1_013_clean_passed_and_dirty_failed_yields_a_proof_and_the_gate_lists_installed() {
        let reads = Cell::new(0);
        let clean = Scripted::new(5, Outcome::Passed, "no test touched");
        let dirty = Scripted::new(5, Outcome::Failed, "a test lost its assertion");
        let def = definition(5);
        let mut prover = Prover::new(counting_clock(&reads));

        let proven = prover
            .prove(&def, &attempt(&clean, &dirty))
            .expect("a clean pass and a dirty failure is the one pair that proves a gate");

        assert_eq!(proven.number(), number(5));
        let proof = proven.proof();
        assert_eq!(proof.planted_defect_ref(), "fixtures/planted/gate-5");
        assert_eq!(proof.evidence_ref(), "ops/gates/gate-5.md");
        assert_eq!(proof.passed_clean_at(), 100);
        assert_eq!(proof.failed_dirty_at(), 200);
        assert_eq!(proven.clean().detail, "no test touched");
        assert_eq!(proven.dirty().detail, "a test lost its assertion");
        assert_eq!(clean.runs.get(), 1, "the clean runner ran exactly once");
        assert_eq!(dirty.runs.get(), 1, "the dirty runner ran exactly once");
        assert_eq!(reads.get(), 2, "one clock reading per accepted run");

        // The gate before proof: Defined even with its runner registered
        // (ORI-P1-012 is not weakened by this module).
        let mut before = Registry::new();
        before
            .install(
                def.clone(),
                Some(Box::new(Scripted::new(5, Outcome::Passed, "r"))),
            )
            .expect("a fresh registry accepts the first entry");
        assert_eq!(before.list()[0].state, GateState::Defined);

        // After the proof is attached and the runner registered: Installed.
        let proven_def = proven
            .attach_to(&def)
            .expect("the demonstration was made for this gate");
        assert_eq!(proven_def.state(), DefinitionState::Proven);
        assert_eq!(proven_def.proof(), Some(proof));
        let mut after = Registry::new();
        after
            .install(
                proven_def,
                Some(Box::new(Scripted::new(5, Outcome::Passed, "r"))),
            )
            .expect("a fresh registry accepts the first entry");
        assert_eq!(
            after.list()[0].state,
            GateState::Installed,
            "ORI-P1-013: a proven gate with its runner registered lists Installed"
        );
        // The same proven definition with no runner is Proven, not Installed.
        let mut without_runner = Registry::new();
        without_runner
            .install(proven.attach_to(&def).expect("same gate"), None)
            .expect("a fresh registry accepts the first entry");
        assert_eq!(without_runner.list()[0].state, GateState::Proven);
    }

    #[test]
    fn ori_p1_013_the_real_modified_tests_runner_is_proven_on_a_weakened_test() {
        // Gate 5's real runner over a clean change set and over the planted
        // defect: an existing test whose assertion was removed.
        let tests_before = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0045_fixture() {\n        assert!(1 + 1 == 2);\n    }\n}\n";
        let tests_weakened =
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ori_t_0045_fixture() {\n    }\n}\n";
        let code_before = "//! demo\npub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";
        let code_after = "//! demo\npub fn add(a: u32, b: u32) -> u32 {\n    b + a\n}\n";
        let clean_changes = vec![FileChange::modified(
            "crates/demo/src/lib.rs",
            &format!("{code_before}{tests_before}"),
            &format!("{code_after}{tests_before}"),
        )];
        let dirty_changes = vec![FileChange::modified(
            "crates/demo/src/lib.rs",
            &format!("{code_before}{tests_before}"),
            &format!("{code_before}{tests_weakened}"),
        )];
        let clean = ModifiedTestsRunner::new(number(5), clean_changes);
        let dirty = ModifiedTestsRunner::new(number(5), dirty_changes);
        let def = definition(5);
        let mut prover = Prover::new(|| 1_700_000_000);

        let proven = prover
            .prove(
                &def,
                &ProofAttempt {
                    clean: &clean,
                    dirty: &dirty,
                    planted_defect_ref: "fixtures/planted/gate-5",
                    evidence_ref: "ops/gates/gate-5.md",
                },
            )
            .expect("the real gate passes the clean change and fails the weakened test");

        assert_eq!(proven.clean().outcome, Outcome::Passed);
        assert_eq!(proven.dirty().outcome, Outcome::Failed);
        assert!(
            !proven.dirty().detail.is_empty(),
            "the failure carries a message a human can read"
        );
        let mut registry = Registry::new();
        registry
            .install(
                proven.attach_to(&def).expect("same gate"),
                Some(Box::new(clean)),
            )
            .expect("a fresh registry accepts the first entry");
        assert_eq!(registry.list()[0].state, GateState::Installed);
    }

    #[test]
    fn ori_p1_013_only_a_clean_pass_then_a_dirty_failure_yields_a_proof_across_every_outcome_pair()
    {
        let mut proofs = Vec::new();
        for clean_outcome in ALL_OUTCOMES {
            for dirty_outcome in ALL_OUTCOMES {
                let clean = Scripted::new(5, clean_outcome, "clean");
                let dirty = Scripted::new(5, dirty_outcome, "dirty");
                let mut prover = Prover::new(|| 1);
                match prover.prove(&definition(5), &attempt(&clean, &dirty)) {
                    Ok(_) => proofs.push((clean_outcome, dirty_outcome)),
                    Err(error) => assert_eq!(error.methodology_ref().section, 14),
                }
            }
        }
        assert_eq!(
            proofs,
            vec![(Outcome::Passed, Outcome::Failed)],
            "exactly one of the nine outcome pairs proves a gate"
        );
    }

    #[test]
    fn ori_p1_013_a_clean_run_that_fails_is_refused_and_the_dirty_runner_is_never_run() {
        let reads = Cell::new(0);
        let clean = Scripted::new(5, Outcome::Failed, "fails on the clean tree");
        let dirty = Scripted::new(5, Outcome::Failed, "fails on the defect");
        let mut prover = Prover::new(counting_clock(&reads));

        let error = prover
            .prove(&definition(5), &attempt(&clean, &dirty))
            .expect_err("a gate that fails the clean tree is not proven");

        assert!(
            matches!(
                &error,
                ProveError::CleanRunNotPassed { gate, run }
                    if *gate == number(5) && run.outcome == Outcome::Failed
            ),
            "{error:?}"
        );
        assert_eq!(error.methodology_ref().section, 14);
        assert!(error.methodology_ref().resolves());
        let shown = error.to_string();
        assert!(shown.contains("AICD §14"), "{shown}");
        assert!(shown.contains("gate 5"), "{shown}");
        assert!(shown.contains("fails on the clean tree"), "{shown}");
        assert_eq!(
            dirty.runs.get(),
            0,
            "no dirty run after a refused clean run"
        );
        assert_eq!(reads.get(), 0, "a refused attempt invents no timestamp");
    }

    #[test]
    fn ori_p1_013_a_clean_run_that_is_blocked_proves_nothing() {
        let reads = Cell::new(0);
        let clean = Scripted::new(5, Outcome::Blocked, "cannot read the matrix");
        let dirty = Scripted::new(5, Outcome::Failed, "fails on the defect");
        let mut prover = Prover::new(counting_clock(&reads));

        let error = prover
            .prove(&definition(5), &attempt(&clean, &dirty))
            .expect_err("a gate that could not run on the clean tree proves nothing");

        assert!(
            matches!(
                &error,
                ProveError::CleanRunNotPassed { run, .. } if run.outcome == Outcome::Blocked
            ),
            "{error:?}"
        );
        assert_eq!(error.methodology_ref().section, 14);
        let shown = error.to_string();
        assert!(shown.contains("blocked"), "{shown}");
        assert!(shown.contains("cannot read the matrix"), "{shown}");
        assert_eq!(dirty.runs.get(), 0);
        assert_eq!(reads.get(), 0);
    }

    #[test]
    fn ori_p1_013_a_dirty_run_that_passes_is_refused() {
        let clean = Scripted::new(5, Outcome::Passed, "clean");
        let dirty = Scripted::new(5, Outcome::Passed, "saw nothing wrong");
        let reads = Cell::new(0);
        let mut prover = Prover::new(counting_clock(&reads));

        let error = prover
            .prove(&definition(5), &attempt(&clean, &dirty))
            .expect_err("a gate that passes on its own planted defect is not proven");

        assert!(
            matches!(
                &error,
                ProveError::DirtyRunNotFailed { gate, run }
                    if *gate == number(5) && run.outcome == Outcome::Passed
            ),
            "{error:?}"
        );
        assert_eq!(error.methodology_ref().section, 14);
        let shown = error.to_string();
        assert!(shown.contains("AICD §14"), "{shown}");
        assert!(shown.contains("present but reporting nothing"), "{shown}");
        assert!(shown.contains("saw nothing wrong"), "{shown}");
        assert_eq!(reads.get(), 1, "only the accepted clean run was stamped");
    }

    #[test]
    fn ori_p1_013_a_dirty_run_that_is_blocked_is_the_present_but_reporting_nothing_case() {
        let clean = Scripted::new(5, Outcome::Passed, "clean");
        let dirty = Scripted::new(5, Outcome::Blocked, "the fixture directory is missing");
        let mut prover = Prover::new(|| 1);

        let error = prover
            .prove(&definition(5), &attempt(&clean, &dirty))
            .expect_err("a blocked dirty run never counts as a failure on the defect");

        assert!(
            matches!(
                &error,
                ProveError::DirtyRunNotFailed { run, .. } if run.outcome == Outcome::Blocked
            ),
            "{error:?}"
        );
        assert_eq!(error.methodology_ref().section, 14);
        let shown = error.to_string();
        assert!(shown.contains("blocked"), "{shown}");
        assert!(shown.contains("nothing was checked"), "{shown}");
        assert!(
            shown.contains("the fixture directory is missing"),
            "{shown}"
        );
        assert_eq!(
            error.run().map(|run| run.outcome),
            Some(Outcome::Blocked),
            "the run that decided the refusal is reachable from the error"
        );
    }

    #[test]
    fn ori_p1_013_a_runner_that_always_passes_cannot_be_proven() {
        // The "present but reporting nothing" gate: it passes on everything,
        // including the planted defect.
        let clean = Scripted::new(7, Outcome::Passed, "exit 0");
        let dirty = Scripted::new(7, Outcome::Passed, "exit 0");
        let mut prover = Prover::new(|| 1);

        let error = prover
            .prove(&definition(7), &attempt(&clean, &dirty))
            .expect_err("a gate that always passes cannot be proven");

        assert!(
            matches!(&error, ProveError::DirtyRunNotFailed { .. }),
            "{error:?}"
        );
        assert_eq!(error.methodology_ref().section, 14);
    }

    #[test]
    fn ori_p1_013_a_runner_that_always_fails_cannot_be_proven() {
        // The gate that is red on everything: it fails on the planted defect for
        // the wrong reason, which a demonstration must not credit.
        let clean = Scripted::new(7, Outcome::Failed, "exit 1");
        let dirty = Scripted::new(7, Outcome::Failed, "exit 1");
        let mut prover = Prover::new(|| 1);

        let error = prover
            .prove(&definition(7), &attempt(&clean, &dirty))
            .expect_err("a gate that always fails cannot be proven");

        assert!(
            matches!(&error, ProveError::CleanRunNotPassed { .. }),
            "{error:?}"
        );
        assert_eq!(error.methodology_ref().section, 14);
    }

    #[test]
    fn ori_p1_013_a_runner_that_is_blocked_on_either_input_or_both_cannot_be_proven() {
        let cases = [
            (Outcome::Blocked, Outcome::Failed),
            (Outcome::Passed, Outcome::Blocked),
            (Outcome::Blocked, Outcome::Blocked),
        ];
        for (clean_outcome, dirty_outcome) in cases {
            let clean = Scripted::new(7, clean_outcome, "clean");
            let dirty = Scripted::new(7, dirty_outcome, "dirty");
            let mut prover = Prover::new(|| 1);
            let result = prover.prove(&definition(7), &attempt(&clean, &dirty));
            let error = result.expect_err("a blocked run proves nothing");
            assert_eq!(
                error.methodology_ref().section,
                14,
                "{clean_outcome} then {dirty_outcome}"
            );
        }
    }

    #[test]
    fn ori_p1_013_runners_that_answer_for_another_gate_are_refused_before_anything_runs() {
        let cases = [(9, 5), (5, 9), (9, 9)];
        for (clean_number, dirty_number) in cases {
            let reads = Cell::new(0);
            let clean = Scripted::new(clean_number, Outcome::Passed, "clean");
            let dirty = Scripted::new(dirty_number, Outcome::Failed, "dirty");
            let mut prover = Prover::new(counting_clock(&reads));

            let error = prover
                .prove(&definition(5), &attempt(&clean, &dirty))
                .expect_err("a runner for another gate cannot prove this one");

            assert_eq!(
                error,
                ProveError::RunnerNumberMismatch {
                    def: number(5),
                    clean: number(clean_number),
                    dirty: number(dirty_number),
                }
            );
            assert_eq!(error.methodology_ref().section, 14);
            let shown = error.to_string();
            assert!(shown.contains("AICD §14"), "{shown}");
            assert!(shown.contains("gate 5"), "{shown}");
            assert_eq!(clean.runs.get(), 0, "nothing runs for the wrong gate");
            assert_eq!(dirty.runs.get(), 0, "nothing runs for the wrong gate");
            assert_eq!(reads.get(), 0);
        }
    }

    #[test]
    fn ori_p1_013_a_missing_or_blank_reference_is_refused_before_anything_runs() {
        let cases = [
            ("", "ops/gates/gate-5.md", ProofReference::PlantedDefect),
            ("   ", "ops/gates/gate-5.md", ProofReference::PlantedDefect),
            ("fixtures/planted/gate-5", "", ProofReference::Evidence),
            ("fixtures/planted/gate-5", "\t\n", ProofReference::Evidence),
        ];
        for (planted, evidence, which) in cases {
            let reads = Cell::new(0);
            let clean = Scripted::new(5, Outcome::Passed, "clean");
            let dirty = Scripted::new(5, Outcome::Failed, "dirty");
            let mut prover = Prover::new(counting_clock(&reads));

            let error = prover
                .prove(
                    &definition(5),
                    &ProofAttempt {
                        clean: &clean,
                        dirty: &dirty,
                        planted_defect_ref: planted,
                        evidence_ref: evidence,
                    },
                )
                .expect_err("a proof that names nowhere to check it is not a proof");

            assert_eq!(error, ProveError::ReferenceMissing { which });
            assert_eq!(error.methodology_ref().section, 14);
            assert!(error.to_string().contains("AICD §14"));
            assert_eq!(clean.runs.get(), 0, "nothing runs without both references");
            assert_eq!(dirty.runs.get(), 0, "nothing runs without both references");
            assert_eq!(reads.get(), 0);
        }
    }

    #[test]
    fn ori_p1_013_the_constructors_own_refusal_converts_to_a_missing_reference() {
        assert_eq!(
            ProveError::from(GateProofError::EmptyPlantedDefectRef),
            ProveError::ReferenceMissing {
                which: ProofReference::PlantedDefect
            }
        );
        assert_eq!(
            ProveError::from(GateProofError::EmptyEvidenceRef),
            ProveError::ReferenceMissing {
                which: ProofReference::Evidence
            }
        );
    }

    #[test]
    fn ori_p1_013_each_clock_reading_is_taken_after_the_run_it_stamps() {
        struct Logging<'a> {
            number: GateNumber,
            outcome: Outcome,
            name: &'static str,
            log: &'a RefCell<Vec<&'static str>>,
        }
        impl Runner for Logging<'_> {
            fn number(&self) -> GateNumber {
                self.number
            }
            fn run(&self) -> RunReport {
                self.log.borrow_mut().push(self.name);
                RunReport {
                    outcome: self.outcome,
                    detail: self.name.to_owned(),
                }
            }
        }

        let log = RefCell::new(Vec::new());
        let clean = Logging {
            number: number(5),
            outcome: Outcome::Passed,
            name: "clean run",
            log: &log,
        };
        let dirty = Logging {
            number: number(5),
            outcome: Outcome::Failed,
            name: "dirty run",
            log: &log,
        };
        let mut prover = Prover::new(|| {
            log.borrow_mut().push("clock");
            i64::try_from(log.borrow().len()).expect("a handful of entries")
        });

        let proven = prover
            .prove(
                &definition(5),
                &ProofAttempt {
                    clean: &clean,
                    dirty: &dirty,
                    planted_defect_ref: "fixtures/planted/gate-5",
                    evidence_ref: "ops/gates/gate-5.md",
                },
            )
            .expect("clean pass and dirty failure");

        assert_eq!(
            *log.borrow(),
            vec!["clean run", "clock", "dirty run", "clock"],
            "each stamp follows the run it records"
        );
        assert_eq!(proven.proof().passed_clean_at(), 2);
        assert_eq!(proven.proof().failed_dirty_at(), 4);
    }

    #[test]
    fn ori_p1_013_a_proof_cannot_be_attached_to_another_gates_definition() {
        let clean = Scripted::new(5, Outcome::Passed, "clean");
        let dirty = Scripted::new(5, Outcome::Failed, "dirty");
        let mut prover = Prover::new(|| 1);
        let proven = prover
            .prove(&definition(5), &attempt(&clean, &dirty))
            .expect("clean pass and dirty failure");

        let other = definition(9);
        let error = proven
            .attach_to(&other)
            .expect_err("gate 5's demonstration proves nothing about gate 9");

        assert_eq!(
            error,
            ProveError::ProofForAnotherGate {
                proof: number(5),
                def: number(9),
            }
        );
        assert_eq!(error.methodology_ref().section, 14);
        assert!(error.to_string().contains("AICD §14"));
        assert_eq!(other.state(), DefinitionState::Defined, "left untouched");
    }

    #[test]
    fn ori_p1_013_revoking_a_proof_returns_the_gate_to_defined_and_it_can_be_proven_again() {
        // spec/runbooks/prove-gate.md rollback: revoke, then re-prove.
        let clean = Scripted::new(5, Outcome::Passed, "clean");
        let dirty = Scripted::new(5, Outcome::Failed, "dirty");
        let original = definition(5);
        let mut prover = Prover::new(|| 1);
        let first = prover
            .prove(&original, &attempt(&clean, &dirty))
            .expect("clean pass and dirty failure");
        let proven_def = first.attach_to(&original).expect("same gate");
        assert_eq!(proven_def.state(), DefinitionState::Proven);

        let revoked = revoke(&proven_def);

        assert_eq!(revoked.state(), DefinitionState::Defined);
        assert!(revoked.proof().is_none());
        assert_eq!(
            revoked, original,
            "a revoked definition is the one that was first defined"
        );
        let mut registry = Registry::new();
        registry
            .install(
                revoked.clone(),
                Some(Box::new(Scripted::new(5, Outcome::Passed, "r"))),
            )
            .expect("a fresh registry accepts the first entry");
        assert_eq!(
            registry.list()[0].state,
            GateState::Defined,
            "a revoked gate lists Defined even with its runner registered"
        );

        let second = prover
            .prove(&revoked, &attempt(&clean, &dirty))
            .expect("the gate can be proven again");
        let reproven = second.attach_to(&revoked).expect("same gate");
        assert_eq!(reproven.state(), DefinitionState::Proven);
    }

    #[test]
    fn ori_p1_013_the_event_kind_is_one_the_specification_names() {
        // The module doc claims the kind is named by the specification, so the
        // specification is read: both documents the claim cites must carry it.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for document in ["spec/LLD.md", "spec/design/DESIGN.md"] {
            let text = std::fs::read_to_string(root.join(document))
                .unwrap_or_else(|error| panic!("{document} cannot be read: {error}"));
            assert!(
                text.contains(GATE_PROVEN_EVENT_KIND),
                "{document} no longer names the event kind {GATE_PROVEN_EVENT_KIND}, so the \
                 head comment's claim about it is false"
            );
        }
    }
}
