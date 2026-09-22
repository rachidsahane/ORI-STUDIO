//! The `Gate` entity: `spec/DATA_MODEL.md` section 2.
//!
//! `spec/DATA_MODEL.md` section 2 gives `Gate` these fields: "id, product_id,
//! kind (lint, types, tests, coverage_matrix, mutation, dependency_audit,
//! secret_scan, build, modified_tests, significance, liveness, citation,
//! commit_trailers, diagram, forbidden_action), definition (json), state
//! (defined, proven, installed, inert)", with the note "`installed` requires a
//! `GateProof`". Section 3 of the same document draws the state machine this
//! module makes real in code: "`Defined` -> `Proven` (`GateProof` present) ->
//! `Installed`. A `GateRun` with status `missing` on an installed gate flips
//! it to `Inert`, which is an incident."
//!
//! # What is modelled here, and what is not
//!
//! This module has no persistence and no store dependency (`ori-gates`
//! depends on nothing but `std`, so `product_id`, `id` as a stored ULID and
//! `GateProof.gate_id` are storage-layer concerns this module does not carry:
//! a [`crate::gate::GateProof`] is composed directly inside the
//! [`crate::gate::GateDef`] it proves rather than referenced by a foreign
//! key, because nothing here writes either one to a database). `Inert` is
//! also not modelled: it is reached by a `GateRun` with status `missing`,
//! and `GateRun` is a record of an actual pipeline execution that this module
//! never sees. Whoever wires a gate's real execution history owes that
//! fourth state.
//!
//! # Why `Installed` cannot be produced here
//!
//! `spec/LLD.md` section 2 lists this crate's must-not column as "report a
//! gate installed without a proof". [`crate::gate::GateDef::state`] answers in
//! [`crate::gate::DefinitionState`], which has two values, `Defined` and
//! `Proven`, and no third. A `GateDef` alone cannot know whether a
//! [`crate::runner::Runner`] is actually wired for it: that fact lives in
//! [`crate::runner::Registry`], which is the only place
//! [`crate::runner::GateState::Installed`] is ever produced. Criterion
//! ORI-P1-012 ("Gate defined, no proof... `gates.list`
//! shows Defined, not Installed") is this ticket's authority for the
//! distinction; making a wrong answer a type error, rather than a convention an
//! implementation might forget, is how this module answers it.
//!
//! ```mermaid
//! flowchart LR
//!   D[GateDef, no proof] -->|state| SD[DefinitionState::Defined]
//!   D -->|with_proof| P[GateDef, has a complete GateProof]
//!   P -->|state| SP[DefinitionState::Proven]
//!   SD -.-> RD[Registry::list: GateState::Defined]
//!   SP -.->|no Runner registered| RP[Registry::list: GateState::Proven]
//!   SP -.->|Runner registered| RI[Registry::list: GateState::Installed]
//! ```
//!
//! # Why the links here are written `crate::gate::`
//!
//! `lib.rs` carries a `///` on this module's declaration, so rustdoc resolves
//! this file's links in the crate root's scope rather than this module's.
//! `crates/ori-gates/src/sections.rs` and `crates/ori-gates/src/spec_refs.rs`
//! record the same trap. Do not shorten these.
//!
//! Must not: add a dependency (`CLAUDE.md` absolute rule 6). Nothing outside
//! `std` is used.

use std::error::Error;
use std::fmt;

/// How many gates `spec/CI_CD.md` section 1 numbers, 1 to 14.
///
/// `spec/CI_CD.md` section 1 lists fourteen required gates in a numbered list,
/// and `scripts/gates.sh` and `ops/gates/gate-N.md` both key on that number.
/// The liveness gate (`spec/CI_CD.md` section 1's closing paragraph, "the
/// liveness gate (scheduled) fails loudly") is not one of the fourteen and is
/// not representable by [`crate::gate::GateNumber`]; whichever ticket wires it
/// owes it an identity of its own.
pub const NUMBERED_GATE_COUNT: u8 = 14;

/// One gate's position in `spec/CI_CD.md` section 1's numbered pipeline.
///
/// This is the identity `scripts/gates.sh` keys on and `ops/gates/` spells as
/// `gate-N.md`, not the ULID `spec/DATA_MODEL.md` section 2 gives `Gate.id`:
/// this module carries no store and mints no ULID, and the number is what
/// every consumer this repository actually has (the script, the proof
/// records) already keys on. [`crate::gate::GateNumber::new`] is the only
/// constructor, and it refuses a value outside the range the pipeline numbers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GateNumber(u8);

impl GateNumber {
    /// A gate number, refusing one `spec/CI_CD.md` section 1 does not carry.
    ///
    /// # Errors
    ///
    /// [`crate::gate::GateNumberError::OutOfRange`] when `value` is `0` or
    /// greater than [`NUMBERED_GATE_COUNT`].
    pub fn new(value: u8) -> Result<Self, GateNumberError> {
        if value == 0 || value > NUMBERED_GATE_COUNT {
            return Err(GateNumberError::OutOfRange { value });
        }
        Ok(Self(value))
    }

    /// The number as `spec/CI_CD.md` section 1 and `ops/gates/gate-N.md` write
    /// it.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for GateNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gate {}", self.0)
    }
}

/// Why a [`crate::gate::GateNumber`] could not be built.
///
/// No methodology section applies: this is a range check on the pipeline
/// listing `spec/CI_CD.md` section 1 carries, not a policy refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateNumberError {
    /// The value was `0` or greater than [`NUMBERED_GATE_COUNT`].
    OutOfRange {
        /// The value that was refused.
        value: u8,
    },
}

impl GateNumberError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute
    /// rule 9).
    #[must_use]
    pub const fn methodology_ref(&self) -> &'static str {
        // AICD §39: a reference that resolves to nothing is the failure this
        // repository checks mechanically, and a gate number outside the
        // fourteen `spec/CI_CD.md` section 1 lists resolves to nothing there.
        "AICD §39"
    }
}

impl fmt::Display for GateNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { value } => write!(
                f,
                "{value} names no gate: `spec/CI_CD.md` section 1 numbers gates 1 to \
                 {NUMBERED_GATE_COUNT}"
            ),
        }
    }
}

impl Error for GateNumberError {}

/// What kind of check a gate is: `spec/DATA_MODEL.md` section 2, `Gate.kind`.
///
/// The fifteen values are copied from the entity row exactly: fourteen are
/// `spec/CI_CD.md` section 1's numbered pipeline (lint and types share item 1;
/// dependency audit and secret scan share item 7), and the fifteenth,
/// `Liveness`, is the scheduled gate the same section describes outside the
/// numbered list. No canonical mapping from a [`crate::gate::GateNumber`] to
/// one or two of these is fixed here: most of the fourteen (mutation,
/// dependency audit, secret scan, build, the UI checks, significance, commit
/// trailers, diagram) have no Rust implementation yet, and guessing their
/// pairing would be a claim this module cannot back.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GateKind {
    /// `cargo fmt --check`. Item 1 of `spec/CI_CD.md` section 1.
    Lint,
    /// `cargo clippy -D warnings`. Item 1 of `spec/CI_CD.md` section 1.
    Types,
    /// `cargo test`, unit, property and integration. Item 2.
    Tests,
    /// Criteria to tests. Item 4. [`crate::coverage`] is the unwired runner.
    CoverageMatrix,
    /// Mutation score threshold. Item 6.
    Mutation,
    /// `cargo-audit`, `cargo-deny`. Item 7.
    DependencyAudit,
    /// Secret scan, tree and history. Item 7.
    SecretScan,
    /// Build of all three platform binaries. Item 11.
    Build,
    /// Escalation label on a changed or removed existing test. Item 5.
    /// [`crate::modified_tests`] is the unwired runner.
    ModifiedTests,
    /// Runs on merge, sets `significant`. Item 12.
    Significance,
    /// Scheduled: a pipeline that does not run is a failure. Not in the
    /// numbered list.
    Liveness,
    /// Every `AICD §n` resolves. Item 9. [`crate::sections`] and
    /// [`crate::spec_refs`] are the unwired, narrower checks.
    Citation,
    /// Conventional Commits, `Ticket:` and `Spec:` trailers. Item 13.
    CommitTrailers,
    /// Every diagram under `spec/` is Mermaid source. Item 14.
    Diagram,
    /// Forbidden-action test against `fixtures/new-product`. Item 8.
    ForbiddenAction,
}

impl GateKind {
    /// The spelling `spec/DATA_MODEL.md` section 2 writes for this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lint => "lint",
            Self::Types => "types",
            Self::Tests => "tests",
            Self::CoverageMatrix => "coverage_matrix",
            Self::Mutation => "mutation",
            Self::DependencyAudit => "dependency_audit",
            Self::SecretScan => "secret_scan",
            Self::Build => "build",
            Self::ModifiedTests => "modified_tests",
            Self::Significance => "significance",
            Self::Liveness => "liveness",
            Self::Citation => "citation",
            Self::CommitTrailers => "commit_trailers",
            Self::Diagram => "diagram",
            Self::ForbiddenAction => "forbidden_action",
        }
    }
}

impl fmt::Display for GateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A planted-defect proof: `spec/DATA_MODEL.md` section 2, `GateProof`.
///
/// `spec/TESTING.md` section 4: "Every gate in CI_CD ships with a planted
/// defect under `fixtures/planted/` and a proof recorded in `ops/gates/`. A
/// gate is not cited as protection in any document until its proof exists
/// (AICD §14)." AICD §14 fixes what a proof is: a gate must be seen to pass on
/// a clean tree and to fail on the planted defect. `gate_id` from the entity
/// row is not carried here: this type is composed directly inside the
/// [`crate::gate::GateDef`] it proves rather than stored as a row with a
/// foreign key, for the reason this module's head comment gives.
///
/// Every field is required at construction. A [`crate::gate::GateProof`] that
/// exists in a running program is, by construction, a complete one: there is
/// no way to build one that has only ever seen a clean pass, or only ever seen
/// the planted defect, or that names no fixture and no record. `gates.sh`'s
/// own self-check states the same principle about itself: "a floor that has
/// only ever been seen to let clean runs through is not installed."
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateProof {
    planted_defect_ref: String,
    evidence_ref: String,
    passed_clean_at: i64,
    failed_dirty_at: i64,
}

impl GateProof {
    /// A complete planted-defect proof: AICD §14.
    ///
    /// # Errors
    ///
    /// [`crate::gate::GateProofError::EmptyPlantedDefectRef`] or
    /// [`crate::gate::GateProofError::EmptyEvidenceRef`] when either reference
    /// is empty: `spec/TESTING.md` section 4 requires both a fixture under
    /// `fixtures/planted/` and a record under `ops/gates/`, and an empty
    /// string names neither.
    pub fn new(
        planted_defect_ref: impl Into<String>,
        evidence_ref: impl Into<String>,
        passed_clean_at: i64,
        failed_dirty_at: i64,
    ) -> Result<Self, GateProofError> {
        let planted_defect_ref = planted_defect_ref.into();
        let evidence_ref = evidence_ref.into();
        if planted_defect_ref.is_empty() {
            return Err(GateProofError::EmptyPlantedDefectRef);
        }
        if evidence_ref.is_empty() {
            return Err(GateProofError::EmptyEvidenceRef);
        }
        Ok(Self {
            planted_defect_ref,
            evidence_ref,
            passed_clean_at,
            failed_dirty_at,
        })
    }

    /// Where the planted defect this proof was seen to fail on lives, relative
    /// to the product root: `spec/TESTING.md` section 4, `fixtures/planted/`.
    #[must_use]
    pub fn planted_defect_ref(&self) -> &str {
        &self.planted_defect_ref
    }

    /// Where the proof is recorded, relative to the product root:
    /// `spec/TESTING.md` section 4, `ops/gates/`.
    #[must_use]
    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }

    /// When this gate was last seen to pass on a clean tree, Unix seconds UTC:
    /// AICD §14.
    #[must_use]
    pub const fn passed_clean_at(&self) -> i64 {
        self.passed_clean_at
    }

    /// When this gate was last seen to fail on the planted defect, Unix
    /// seconds UTC: AICD §14.
    #[must_use]
    pub const fn failed_dirty_at(&self) -> i64 {
        self.failed_dirty_at
    }
}

/// Why a [`crate::gate::GateProof`] could not be built: AICD §14.
///
/// Every variant is a refusal to record a proof that names nowhere to check
/// it, which is the "present but reporting nothing" defect class AICD §39
/// names applied to the proof record itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateProofError {
    /// `planted_defect_ref` was empty.
    EmptyPlantedDefectRef,
    /// `evidence_ref` was empty.
    EmptyEvidenceRef,
}

impl GateProofError {
    /// The methodology section this refusal rests on (`CLAUDE.md` absolute
    /// rule 9).
    #[must_use]
    pub const fn methodology_ref(&self) -> &'static str {
        "AICD §14"
    }
}

impl fmt::Display for GateProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlantedDefectRef => f.write_str(
                "a proof naming no planted-defect fixture is not a proof: `spec/TESTING.md` \
                 section 4 requires one under `fixtures/planted/`",
            ),
            Self::EmptyEvidenceRef => f.write_str(
                "a proof naming no evidence record is not a proof: `spec/TESTING.md` section 4 \
                 requires one under `ops/gates/`",
            ),
        }
    }
}

impl Error for GateProofError {}

/// The two states a [`crate::gate::GateDef`] alone can honestly report:
/// `spec/DATA_MODEL.md` section 3's `Gate` state machine, restricted to what
/// this type, on its own, can know.
///
/// The third value the state machine names, `Installed`, needs a fact this
/// type does not carry, whether a [`crate::runner::Runner`] is actually
/// wired, so it is not a value this type can produce. [`crate::runner::GateState`]
/// carries all three, and only [`crate::runner::Registry`] can produce its
/// `Installed` value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionState {
    /// No [`crate::gate::GateProof`] is attached.
    Defined,
    /// A complete [`crate::gate::GateProof`] is attached.
    Proven,
}

impl fmt::Display for DefinitionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Defined => f.write_str("defined"),
            Self::Proven => f.write_str("proven"),
        }
    }
}

/// One gate, as `spec/DATA_MODEL.md` section 2 defines the `Gate` entity.
///
/// `name` and `spec_ref` stand in for the entity row's `definition (json)`
/// field: this module has no store and writes no JSON, so it carries the two
/// human-readable facts a `gates.list` line needs instead of a serialized
/// blob. `product_id` and the entity's own `id` (a ULID by
/// `spec/DATA_MODEL.md`'s "Identifiers are ULIDs unless stated") are not
/// carried for the same reason this module's head comment gives for
/// `GateProof.gate_id`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateDef {
    number: GateNumber,
    kind: GateKind,
    name: String,
    spec_ref: String,
    proof: Option<GateProof>,
}

impl GateDef {
    /// A newly defined gate, with no proof: `spec/DATA_MODEL.md` section 3,
    /// the state machine's start, `[*] --> Defined`.
    #[must_use]
    pub fn new(
        number: GateNumber,
        kind: GateKind,
        name: impl Into<String>,
        spec_ref: impl Into<String>,
    ) -> Self {
        Self {
            number,
            kind,
            name: name.into(),
            spec_ref: spec_ref.into(),
            proof: None,
        }
    }

    /// The same gate with a proof attached: `spec/DATA_MODEL.md` section 3,
    /// `Defined -> Proven (GateProof present)`.
    ///
    /// Consumes `self` rather than mutating in place, so that a `GateDef`
    /// already shared by reference cannot be promoted out from under a reader
    /// holding one: a proof is attached once, at definition time, not
    /// discovered later by a caller who only had a `&GateDef`.
    #[must_use]
    pub fn with_proof(mut self, proof: GateProof) -> Self {
        self.proof = Some(proof);
        self
    }

    /// This gate's position in `spec/CI_CD.md` section 1's numbered pipeline.
    #[must_use]
    pub const fn number(&self) -> GateNumber {
        self.number
    }

    /// What kind of check this gate is: `spec/DATA_MODEL.md` section 2.
    #[must_use]
    pub const fn kind(&self) -> GateKind {
        self.kind
    }

    /// The short human label this gate is listed under.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where this gate is specified, for a reader who wants the source rather
    /// than the label: AICD §39, a reference checked mechanically like any
    /// other in this repository.
    #[must_use]
    pub fn spec_ref(&self) -> &str {
        &self.spec_ref
    }

    /// The proof attached to this gate, if any: `spec/DATA_MODEL.md` section 3.
    #[must_use]
    pub fn proof(&self) -> Option<&GateProof> {
        self.proof.as_ref()
    }

    /// What this `GateDef` alone can honestly say about its own state:
    /// `spec/DATA_MODEL.md` section 3, restricted to [`DefinitionState`]'s two
    /// values.
    #[must_use]
    pub fn state(&self) -> DefinitionState {
        if self.proof.is_some() {
            DefinitionState::Proven
        } else {
            DefinitionState::Defined
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> GateProof {
        GateProof::new(
            "fixtures/planted/gate-4",
            "ops/gates/gate-4.md",
            1_700_000_000,
            1_700_000_060,
        )
        .expect("both references are non-empty")
    }

    #[test]
    fn ori_t_0041_gate_number_refuses_zero_and_anything_past_fourteen() {
        assert!(GateNumber::new(0).is_err(), "0 names no pipeline item");
        assert!(GateNumber::new(1).is_ok(), "1 is the first numbered gate");
        assert!(
            GateNumber::new(NUMBERED_GATE_COUNT).is_ok(),
            "14 is the last numbered gate"
        );
        assert!(
            GateNumber::new(NUMBERED_GATE_COUNT + 1).is_err(),
            "15 names no pipeline item"
        );
    }

    #[test]
    fn ori_t_0041_gate_kind_as_str_matches_data_model_spellings() {
        // spec/DATA_MODEL.md section 2: "kind (lint, types, tests,
        // coverage_matrix, mutation, dependency_audit, secret_scan, build,
        // modified_tests, significance, liveness, citation, commit_trailers,
        // diagram, forbidden_action)". Copied in the order the row writes it.
        let expected = [
            (GateKind::Lint, "lint"),
            (GateKind::Types, "types"),
            (GateKind::Tests, "tests"),
            (GateKind::CoverageMatrix, "coverage_matrix"),
            (GateKind::Mutation, "mutation"),
            (GateKind::DependencyAudit, "dependency_audit"),
            (GateKind::SecretScan, "secret_scan"),
            (GateKind::Build, "build"),
            (GateKind::ModifiedTests, "modified_tests"),
            (GateKind::Significance, "significance"),
            (GateKind::Liveness, "liveness"),
            (GateKind::Citation, "citation"),
            (GateKind::CommitTrailers, "commit_trailers"),
            (GateKind::Diagram, "diagram"),
            (GateKind::ForbiddenAction, "forbidden_action"),
        ];
        for (kind, spelling) in expected {
            assert_eq!(kind.as_str(), spelling);
        }
    }

    #[test]
    fn ori_p1_012_a_fresh_gate_def_carries_no_proof_and_reports_defined() {
        let def = GateDef::new(
            GateNumber::new(9).expect("9 is numbered"),
            GateKind::Citation,
            "citation gate",
            "spec/CI_CD.md section 1 item 9",
        );
        assert!(def.proof().is_none());
        assert_eq!(def.state(), DefinitionState::Defined);
    }

    #[test]
    fn ori_p1_012_a_gate_with_a_proof_reports_proven_not_defined() {
        let def = GateDef::new(
            GateNumber::new(4).expect("4 is numbered"),
            GateKind::CoverageMatrix,
            "coverage matrix",
            "spec/CI_CD.md section 1 item 4",
        )
        .with_proof(proof());
        assert!(def.proof().is_some());
        assert_eq!(
            def.state(),
            DefinitionState::Proven,
            "a gate with a proof must not report Defined (ORI-P1-012)"
        );
    }

    #[test]
    fn ori_p1_012_gate_proof_refuses_an_empty_reference() {
        assert!(GateProof::new("", "ops/gates/gate-1.md", 1, 2).is_err());
        assert!(GateProof::new("fixtures/planted/gate-1", "", 1, 2).is_err());
    }
}
