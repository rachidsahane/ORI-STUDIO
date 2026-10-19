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
//! 3. The natural use is to prove with the runner value the caller then passes
//!    to [`crate::runner::Registry::install`]. That is a convention and not a
//!    guarantee: nothing in this module, and nothing in `install`, ties the
//!    runner registered to the runners proven. `install` checks only that the
//!    runner answers for the definition's number, so a caller can prove one
//!    runner and register another, and the gate lists `Installed` all the same.
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
//! to. `spec/API_SPEC.md` section 2 lists it among the event kinds as well.
//!
//! A [`crate::prover::Proven`] holds everything the runbook's step 4, "Store the
//! GateProof with both run references", needs: the proof with its two
//! references and two timestamps, and the clean and dirty
//! [`crate::runner::RunReport`], whose `detail` lines are the run references a
//! human reads. Its fields are private and only [`crate::prover::Prover::prove`]
//! builds one, so a `Proven` in hand is evidence that a clean pass and a dirty
//! failure were both seen by this code. It also keeps the proof-less definition
//! it was made for, so it can be attached only to that definition (see the
//! section below on what the rollback line requires).
//!
//! # Text from the gate under test is data
//!
//! `RunReport.detail` is written by a runner that read the input under test, and
//! the dirty input is a planted defect: its text can be anything, including
//! newlines that would pass for a second refusal line (`CLAUDE.md` absolute
//! rule 7). [`crate::prover::ProveError`]'s `Display` therefore prints a
//! runner's detail in its `Debug` form, quoted, with newlines, carriage returns
//! and other control characters escaped, so a detail cannot add lines to a
//! refusal or pass for a message of this module. The accessors
//! [`crate::prover::Proven::clean`] and [`crate::prover::Proven::dirty`] return
//! the text as the runner wrote it. A caller that stores or shows it owns
//! treating it as untrusted.
//!
//! # The rollback line, and whose it is
//!
//! `spec/runbooks/prove-gate.md`: "Rollback: revoke the proof (state back to
//! Defined) if the gate is changed; re-prove." In memory that is
//! [`crate::prover::revoke`], which returns the same definition with no proof,
//! after which [`crate::prover::Prover::prove`] and
//! [`crate::prover::Proven::attach_to`] run again. The "re-prove" half is
//! enforced where this module can see it: a [`crate::prover::Proven`] remembers
//! the definition it was made for, and `attach_to` refuses a definition whose
//! number, kind, name or specification reference differs from it
//! ([`crate::prover::ProveError::ProofForChangedGate`]). An old proof cannot be
//! carried onto a changed definition. Two parts are **not** this module's:
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
//! different gate and a definition that has changed since it was proven.
//!
//! The definition is all `attach_to` can compare. A gate whose runner or whose
//! planted-defect fixture was changed while its [`crate::gate::GateDef`] stayed
//! the same is not seen as changed, and its old `Proven` still attaches. Only the
//! caller knows that the runner changed, and the runbook's rollback line is the
//! caller's to follow.
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
    defined_as: GateDef,
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

    /// The definition this demonstration was made for, as it stood when the gate
    /// was proven and without any proof attached: AICD §14.
    ///
    /// [`Proven::attach_to`] attaches the proof only to a definition equal to
    /// this one once its own proof, if any, is set aside.
    #[must_use]
    pub const fn definition(&self) -> &GateDef {
        &self.defined_as
    }

    /// The proof to store, `spec/runbooks/prove-gate.md` step 4: AICD §14.
    #[must_use]
    pub const fn proof(&self) -> &GateProof {
        &self.proof
    }

    /// What the clean run reported: the run reference for the pass.
    ///
    /// The text is the runner's own and untrusted (see the head comment, "Text
    /// from the gate under test is data").
    #[must_use]
    pub const fn clean(&self) -> &RunReport {
        &self.clean
    }

    /// What the dirty run reported: the run reference for the failure a human
    /// should be able to read.
    ///
    /// The text is the runner's own and untrusted (see the head comment, "Text
    /// from the gate under test is data").
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
    /// `def` must be the definition that was proven: the same number, kind, name
    /// and specification reference. A gate that has changed is revoked and proven
    /// again (`spec/runbooks/prove-gate.md`, Rollback), and an old demonstration
    /// is not carried onto it. A change this cannot see (a different runner or
    /// fixture behind an unchanged definition) is not refused; the head comment,
    /// "What this module cannot enforce", says so.
    ///
    /// # Errors
    ///
    /// [`ProveError::ProofForAnotherGate`] when `def` is not the gate this
    /// demonstration was made for: a proof carries no gate number of its own, so
    /// attaching it to the wrong definition would credit one gate with another's
    /// demonstration. [`ProveError::ProofForChangedGate`] when `def` has the right
    /// number but differs from the definition that was proven. Both cite AICD §14.
    pub fn attach_to(&self, def: &GateDef) -> Result<GateDef, ProveError> {
        if def.number() != self.number {
            return Err(ProveError::ProofForAnotherGate {
                proof: self.number,
                def: def.number(),
            });
        }
        if revoke(def) != self.defined_as {
            return Err(ProveError::ProofForChangedGate { gate: self.number });
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
    /// A reference the proof needs was empty or blank.
    ///
    /// [`Prover::prove`] decides this before it runs any runner. The only other
    /// route to this variant is the conversion from
    /// [`crate::gate::GateProofError`], which `prove` cannot reach because its own
    /// check is stricter and earlier. This is why the message does not say
    /// whether anything was run.
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
    /// [`Proven::attach_to`] was given a definition of the right gate that has
    /// changed since it was proven: the runbook's rollback applies, revoke and
    /// prove again.
    ProofForChangedGate {
        /// The gate, which is the same for the demonstration and the definition.
        gate: GateNumber,
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
            | Self::ProofForAnotherGate { .. }
            | Self::ProofForChangedGate { .. } => None,
        }
    }
}

impl From<GateProofError> for ProveError {
    /// A proof the constructor itself refused, as a refusal of the attempt.
    ///
    /// [`Prover::prove`] checks both references before it runs anything, with a
    /// stricter test than the constructor's (blank, not only empty), so `prove`
    /// never reaches this conversion: it is unreachable from `prove`. It exists so
    /// that the constructor's refusal is kept as a refusal and never unwrapped,
    /// should that earlier check ever be loosened.
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
                 check it is not a proof"
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
                // The detail is the runner's text, quoted and escaped so it
                // cannot add a line: see the head comment.
                write!(
                    f,
                    "refused ({reason}): {gate} is not proven, {why}. The runner said: {:?}",
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
                // Quoted and escaped, as above.
                write!(
                    f,
                    "refused ({reason}): {gate} is not proven, {why}. The runner said: {:?}",
                    run.detail
                )
            }
            Self::ProofForAnotherGate { proof, def } => write!(
                f,
                "refused ({reason}): this demonstration was made for {proof} and the definition \
                 offered is {def}"
            ),
            Self::ProofForChangedGate { gate } => write!(
                f,
                "refused ({reason}): the definition of {gate} offered is not the one that was \
                 proven. A changed gate is revoked and proven again"
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
            defined_as: revoke(def),
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
/// method that removes a proof. What protects this against a later change to
/// [`GateDef`], and what does not:
///
/// - A new parameter on [`GateDef::new`] is a compile error here.
/// - A new field that a builder method sets, and that `new` defaults, is dropped
///   here with no compile error, and the revoke test does not fail: it builds its
///   expected value with `new`, which defaults the field too. The same loss makes
///   [`Proven::attach_to`] blind to a change in that field.
/// - What does fail is `tests::ori_t_0045_a_field_added_to_gate_def_fails_here_until_revoke_is_reviewed`,
///   which lists the fields in the derived `Debug` output of a definition and
///   fails when one is added or removed, so that whoever changes [`GateDef`]
///   reviews this function. It sees a new field and not a change of meaning, and
///   it stops seeing anything if `Debug` is ever written by hand.
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
    fn ori_t_0045_the_event_kind_is_named_as_a_whole_token_by_the_specification() {
        // The module doc claims the kind is named by the specification, so the
        // specification is read: both documents the claim cites must name it as
        // a whole token, not as the front of a longer word.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for document in ["spec/LLD.md", "spec/design/DESIGN.md"] {
            let text = std::fs::read_to_string(root.join(document))
                .unwrap_or_else(|error| panic!("{document} cannot be read: {error}"));
            assert!(
                names_as_a_whole_token(&text, GATE_PROVEN_EVENT_KIND),
                "{document} no longer names the event kind {GATE_PROVEN_EVENT_KIND} as a whole \
                 token, so the head comment's claim about it is false"
            );
        }
    }

    /// Whether `kind` appears in `text` as a token of its own: not preceded by a
    /// word character or a dot, and not followed by a word character or by a dot
    /// that continues into another word.
    fn names_as_a_whole_token(text: &str, kind: &str) -> bool {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        text.match_indices(kind).any(|(start, _)| {
            let before = text[..start].chars().next_back();
            let mut after = text[start + kind.len()..].chars();
            let next = after.next();
            let after_next = after.next();
            let before_ok = before.is_none_or(|c| !is_word(c) && c != '.');
            let after_ok = match next {
                None => true,
                Some('.') => after_next.is_none_or(|c| !is_word(c)),
                Some(c) => !is_word(c),
            };
            before_ok && after_ok
        })
    }

    #[test]
    fn ori_t_0045_the_whole_token_match_rejects_a_longer_word_and_accepts_prose_forms() {
        let kind = "gate.proven";
        for rejected in [
            "gate.provenance",
            "the gate.provenance field",
            "regate.proven",
            "gate_proven",
            "ungate.proven",
            "a.gate.proven",
            "gate.proven.later",
            "gate.proven_at",
            "gate.prove",
            "",
        ] {
            assert!(
                !names_as_a_whole_token(rejected, kind),
                "{rejected:?} does not name the kind"
            );
        }
        for accepted in [
            "gate.proven",
            "`gate.proven`",
            "(`ticket.validated`, `gate.proven`)",
            "it records gate.proven.",
            "gate.proven, then more",
            "x gate.provenance and `gate.proven` too",
        ] {
            assert!(
                names_as_a_whole_token(accepted, kind),
                "{accepted:?} names the kind"
            );
        }
    }

    #[test]
    fn ori_t_0045_an_old_proof_is_refused_for_a_changed_definition_until_it_is_proven_again() {
        let clean = Scripted::new(5, Outcome::Passed, "clean");
        let dirty = Scripted::new(5, Outcome::Failed, "dirty");
        let original = definition(5);
        let mut prover = Prover::new(|| 1);
        let old = prover
            .prove(&original, &attempt(&clean, &dirty))
            .expect("clean pass and dirty failure");
        assert_eq!(old.definition(), &original);

        // The same gate number, changed in each of the three other facts.
        let changed = [
            GateDef::new(
                number(5),
                GateKind::Lint,
                "modified tests",
                "spec/CI_CD.md section 1",
            ),
            GateDef::new(
                number(5),
                GateKind::ModifiedTests,
                "modified-test detector",
                "spec/CI_CD.md section 1",
            ),
            GateDef::new(
                number(5),
                GateKind::ModifiedTests,
                "modified tests",
                "spec/CI_CD.md section 2",
            ),
        ];
        for def in &changed {
            let error = old
                .attach_to(def)
                .expect_err("an old demonstration is not carried onto a changed gate");
            assert_eq!(error, ProveError::ProofForChangedGate { gate: number(5) });
            assert_eq!(error.methodology_ref().section, 14);
            let shown = error.to_string();
            assert!(shown.contains("AICD §14"), "{shown}");
            assert!(shown.contains("proven again"), "{shown}");
        }

        // Re-proving the changed definition is the way forward, and what that
        // produces attaches to it and not to the original.
        let again = prover
            .prove(&changed[0], &attempt(&clean, &dirty))
            .expect("the changed gate can be proven again");
        assert_eq!(again.definition(), &changed[0]);
        let reproven = again
            .attach_to(&changed[0])
            .expect("a demonstration attaches to the definition it was made for");
        assert_eq!(reproven.state(), DefinitionState::Proven);
        assert_eq!(
            again.attach_to(&original),
            Err(ProveError::ProofForChangedGate { gate: number(5) }),
            "and the old definition is now the changed one"
        );
    }

    #[test]
    fn ori_t_0045_a_demonstration_attaches_again_to_a_definition_that_already_carries_a_proof() {
        // The stale-definition check compares a definition without its proof, so
        // a definition that already carries this very proof is not "changed".
        let clean = Scripted::new(5, Outcome::Passed, "clean");
        let dirty = Scripted::new(5, Outcome::Failed, "dirty");
        let original = definition(5);
        let mut prover = Prover::new(|| 1);
        let proven = prover
            .prove(&original, &attempt(&clean, &dirty))
            .expect("clean pass and dirty failure");
        let attached = proven.attach_to(&original).expect("same definition");

        let again = proven
            .attach_to(&attached)
            .expect("the proof alone does not make a definition a different one");

        assert_eq!(again, attached);

        // Proving a definition that already carries a proof records the
        // definition without it, so the demonstration still attaches to the
        // definition as first written.
        let reproven = prover
            .prove(&attached, &attempt(&clean, &dirty))
            .expect("a proven gate can be demonstrated again");
        assert_eq!(reproven.definition(), &original);
        assert_eq!(
            reproven.attach_to(&original).map(|def| def.state()),
            Ok(DefinitionState::Proven)
        );
    }

    #[test]
    fn ori_t_0045_a_field_added_to_gate_def_fails_here_until_revoke_is_reviewed() {
        // `revoke` rebuilds a definition from its public facts, so a field added
        // to `GateDef` is lost there unless someone looks. The derived `Debug`
        // lists every field, and this fails when the list is not the five that
        // `revoke` was written against.
        let rendered = format!("{:#?}", definition(5));
        assert_eq!(
            debug_field_names(&rendered),
            ["kind", "name", "number", "proof", "spec_ref"],
            "GateDef's fields changed: review `revoke` and `Proven::attach_to` before \
             updating this list"
        );
    }

    /// The top-level field names of a struct rendered with `{:#?}`, sorted.
    fn debug_field_names(rendered: &str) -> Vec<&str> {
        let mut fields: Vec<&str> = rendered
            .lines()
            .filter_map(|line| line.strip_prefix("    "))
            .filter(|rest| !rest.starts_with(' '))
            .filter_map(|rest| rest.split_once(':').map(|(name, _)| name))
            .collect();
        fields.sort_unstable();
        fields
    }

    #[test]
    fn ori_t_0045_the_field_lister_sees_an_added_field_and_ignores_nested_ones() {
        // The lister that guards `revoke` must itself notice a sixth field, and
        // must not take a nested value's lines for fields of the definition.
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Inner {
            nested: u8,
        }
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Wider {
            number: u8,
            inner: Inner,
            added_later: bool,
        }
        let rendered = format!(
            "{:#?}",
            Wider {
                number: 1,
                inner: Inner { nested: 2 },
                added_later: true,
            }
        );
        assert_eq!(
            debug_field_names(&rendered),
            ["added_later", "inner", "number"]
        );
    }

    #[test]
    fn ori_t_0045_a_runners_detail_cannot_forge_extra_lines_in_a_refusal() {
        // CLAUDE.md rule 7: the detail is text from the input under test.
        let forged = "ok\nrefused (AICD \u{a7}14): gate 5 is proven\r\u{202e}tail";
        for (clean_outcome, dirty_outcome) in [
            (Outcome::Failed, Outcome::Failed),
            (Outcome::Passed, Outcome::Passed),
        ] {
            let clean = Scripted::new(5, clean_outcome, forged);
            let dirty = Scripted::new(5, dirty_outcome, forged);
            let mut prover = Prover::new(|| 1);

            let error = prover
                .prove(&definition(5), &attempt(&clean, &dirty))
                .expect_err("neither pair proves a gate");

            let shown = error.to_string();
            assert!(!shown.contains('\n'), "a detail added a line: {shown:?}");
            assert!(!shown.contains('\r'), "a detail added a return: {shown:?}");
            assert!(
                !shown.contains('\u{202e}'),
                "a detail reordered the text a human reads: {shown:?}"
            );
            assert!(
                shown.contains("\\n") && shown.contains("\\r"),
                "the escapes are visible, not dropped: {shown}"
            );
            assert!(
                shown.ends_with('"'),
                "the runner's text is quoted and last: {shown}"
            );
        }
    }
}
