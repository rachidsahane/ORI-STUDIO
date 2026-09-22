//! The merge queue's decision: AICD §13, AICD §38; `spec/CI_CD.md` section 2.
//!
//! `spec/CI_CD.md` section 2 is the whole specification, verbatim: "Only the
//! merge queue merges. It rebases, re-runs gates 1 to 10, plus 13 and 14, on
//! the rebased head, checks tier approvals, and merges. Tier 0 auto-merge
//! requires the lead's approval and green gates; tier 1 one human; tier 2 two
//! humans or the single-operator profile substitutes, recorded."
//! `spec/RISK_MAP.md` tiers this file at 2: "Merge path, tiers". CLAUDE.md's
//! load-bearing facts state the same rule from the other side: "Only
//! `ori-orchestrator::merge_queue` calls `VcsHost::merge`."
//!
//! # Decision, not action
//!
//! [`MergeQueue::decide`] is the whole of this module: a pure function from a
//! [`Candidate`] to a [`Decision`]. It performs no rebase, runs no gate, calls
//! no adapter, opens no process, makes no network call and holds no
//! credential. `spec/LLD.md` section 2 gives this crate `MergeQueue` and
//! nothing that would let it act: "Must not: Call adapters directly except
//! `VcsHost::merge` through `MergeQueue`", and this ticket's instruction
//! narrows it further, in words worth carrying into the code: this module must
//! not merge anything, and a design that needs to perform a merge to be
//! testable is a finding, not a licence. There is deliberately no method here
//! that LLD's carve-out would let call `VcsHost::merge`. Adding one would have
//! to name the trait Escalation E-0002 leaves open (below), and the ticket
//! that adds it opens once that escalation resolves; nothing here blocks it,
//! and nothing here anticipates which side it lands on.
//!
//! ```mermaid
//! flowchart LR
//!   PR[pull request] --> RB[rebase onto main]
//!   RB --> GR["re-run gates 1-10, 13, 14 on the rebased head"]
//!   GR --> CAND["Candidate: head, gate runs, approvals, substitutes, rollback plan"]
//!   CAND --> Q["MergeQueue::decide"]
//!   Q -->|"Decision::Merge"| LATER["a future call to VcsHost::merge, once E-0002 names the trait"]
//!   Q -->|"Decision::Refuse(reasons)"| HUMAN["each reason, with its MethodologyRef"]
//! ```
//!
//! # What a gate run is, to this module, and what it deliberately does not know
//!
//! `crates/ori-gates` owns running a gate (`spec/LLD.md` section 2: `GateDef`,
//! `Runner`, the built-in runners); it does not yet export them; a sibling is
//! building `GateDef` and `Runner` as this ticket is written, and importing
//! what does not exist is not possible. Even once they exist, LLD's ownership
//! table keeps "do" in `ori-gates` and gives this crate only the decision, so
//! this module was never going to call a runner. What the decision needs is
//! smaller than a runner: for each required gate, whether it ran, whether it
//! passed, and which commit it ran against. [`GateRun`] is exactly that report
//! and nothing more; this file has no dependency edge on `ori-gates` in
//! practice even though the crate manifest (written before this ticket) lists
//! one. A caller that owns an actual `Runner` translates its verdicts into
//! `GateRun` values; this module never runs one itself.
//!
//! [`REQUIRED_GATES`] transcribes `spec/CI_CD.md` section 2's "gates 1 to 10,
//! plus 13 and 14" as the twelve gate numbers of `spec/CI_CD.md` section 1's
//! list that a merge is conditioned on. Gate 11 (the platform build matrix) and
//! gate 12 (the significance labeler, which section 1 itself says "runs on
//! merge") are section 1 gates that section 2 does not name, and are absent
//! from this list on purpose, not by omission.
//!
//! # The rebase-then-stale-gate problem, which is the reason this file exists
//!
//! `spec/CI_CD.md` section 2's order is rebase, then re-run, then check
//! approvals, then merge. Nothing in that sentence stops a second commit
//! landing on `main` and the branch being rebased again after gates already
//! ran once; nothing stops a caller checking approvals before re-running gates
//! by mistake; nothing, read as English, stops a queue from merging a
//! rebased head that its own gates never saw. [`Candidate::head`] is the
//! commit the queue would merge, [`GateRun::ran_against`] is the commit a gate
//! actually ran against, and [`MergeQueue::decide`] refuses whenever those two
//! disagree for a required gate, with [`Reason::GateStale`], whatever the
//! `passed` field says. A gate that passed against a commit that is not the
//! one being merged has proven nothing about the one being merged; treating it
//! as proof is the exact failure this control exists to prevent, and it is the
//! one this ticket's prompt names as the one a careless implementation gets
//! wrong.
//!
//! # Where `VcsHost` is not defined, and why: Escalation E-0002
//!
//! `ops/escalations/E-0002-vcshost-trait-location.md` is open on where adapter
//! traits live: `spec/LLD.md` section 2 gives `ori-orchestrator` a carve-out to
//! call `VcsHost::merge`, but the same section's dependency graph draws no
//! edge from this crate to `ori-integrations`, where the same section's table
//! puts adapter traits, so the trait the carve-out names cannot currently be
//! written down anywhere this crate can see it. The escalation's own words:
//! "Will block: ORI-T-0053 (merge queue) ... The question binds when
//! ORI-T-0053 builds the merge queue in batch 8." It does now, and the answer
//! taken here is the one the escalation leaves open for: no `VcsHost` trait is
//! declared in this file. [`Decision::Merge`] is this module's whole output;
//! it names no method to call and no trait to call it on. Whichever way E-0002
//! resolves, Option A (the trait moves to `ori-core`, which this crate already
//! depends on) or Option B (a new, listed `ori-orchestrator -> ori-integrations`
//! edge), a future ticket names `dyn VcsHost` in a new, small function that
//! takes a `Decision::Merge` and performs the call; nothing here commits to
//! either shape.
//!
//! # Tier 0 and tier 1
//!
//! AICD §13's "Merge policy by risk tier" table, read per row:
//!
//! - Tier 0: "CI green and lead agent approval. Auto-merge allowed." No human
//!   verifier beyond the lead. [`Tier::Zero`] therefore adds no approval check
//!   beyond [`Reason::LeadApprovalMissing`].
//! - Tier 1: "CI green, lead agent approval, one human verifier who reviews
//!   the plan, the coverage matrix, the report and the diff summary."
//!   [`Tier::One`] additionally requires at least one distinct human approver,
//!   [`Reason::HumanApprovalsInsufficient`].
//!
//! Both tiers also carry the gate check every candidate carries, described
//! above.
//!
//! # Tier 2, the single-operator profile, and the reading this module assumes
//!
//! AICD §13's tier 2 row: "CI green, lead agent approval, two humans, written
//! rollback plan, and at least one human reads the diff itself."
//! `spec/CI_CD.md` section 2 restates the human-count clause as "two humans
//! or the single-operator profile substitutes, recorded." `ADR-0002` records
//! this project's exception: Ori Studio has one human, so "two humans" is
//! unreachable by construction, and the accepted substitute is AICD §38's
//! list, which that ADR splits into six named items because two of its five
//! bullets carry clauses in different states: "SO-1, SO-2, SO-3a, SO-3b, SO-4
//! and SO-5", each stated in a pull request as "satisfied, approximated, not
//! applicable or unavailable, with the reason." [`Substitute`] and
//! [`SubstituteState`] transcribe exactly that vocabulary; nothing here
//! invents a seventh state or a different spelling.
//!
//! AICD §38 states its own list as "required together": every one of the six
//! must be accounted for, not a majority and not the convenient ones. This
//! module reads "accounted for" as ADR-0002 defines its own words: "'Not
//! applicable' is claimed only where the substitute has no object; where it
//! has an object and no mechanism, the word is 'unavailable'." A substitute
//! marked [`SubstituteState::NotApplicable`], [`SubstituteState::Approximated`]
//! or [`SubstituteState::Satisfied`] does not block the substitute path;
//! [`SubstituteState::Unavailable`] does, because that is the word ADR-0002
//! reserves for a requirement with a real object and no mechanism to meet it,
//! and a queue that let a written word stand in for a missing mechanism would
//! be the "present but reporting nothing" defect AICD §39 names, in the merge
//! path itself. This module does not read `ADR-0002`'s availability table
//! and does not hold an opinion on which substitutes are available today: the
//! table is dated, is rewritten at the close of every phase, and is not
//! something a crate with no IO can read (`spec/LLD.md` section 2). A caller
//! builds each [`SubstituteRecord`] from that table and hands it to
//! [`Candidate::substitutes`]; what happens if a caller hands over an empty
//! list, or marks something unavailable that the ADR would call satisfied, is
//! exactly what the tests below prove: the module refuses rather than assumes.
//!
//! What this means for tier 2 today, stated rather than hidden: ADR-0002's own
//! table records SO-3b as a substitute "with an object and no mechanism" for a
//! change carrying a migration, and records SO-3a, SO-4 and SO-5 as having "no
//! object" in phases 1 and 2, and SO-1 as approximable but not satisfiable
//! while its checklist does not exist. A caller that reports that table
//! faithfully will find the substitute path closed for most tier 2 changes
//! today, and the two-human path closed always, because there is one human.
//! ADR-0002 states plainly that refusing to pretend otherwise is the right
//! failure mode: "No agent lowers a tier, widens a scope or reclassifies a
//! module to avoid this exception." This module agrees by construction: it has
//! no bypass, no default-allow, and no special case for "there is only one
//! human here". Refusing every tier 2 candidate a caller hands it today is not
//! this module malfunctioning; recording exactly why, with the reasons this
//! module names, is what the caller does with the refusal.
//!
//! Tier 2 also carries two checks AICD §13 states independently of the
//! human-count clause, and this module checks both regardless of which path
//! the human-count clause took: [`Candidate::rollback_plan`] must be present
//! and non-blank ([`Reason::RollbackPlanMissing`]), and at least one recorded
//! human approval must carry [`Approval::read_diff`]
//! ([`Reason::DiffNotRead`]).
//!
//! # Escalation E-0007, and which reading this module assumes
//!
//! `ops/escalations/E-0007-may-the-lead-merge.md` is open, with the operator,
//! on whether AICD §17's permission-matrix cell "Lead / Reviewer, Code
//! repositories: Read all; merge tier 0" means the lead may cause a tier 0
//! merge to happen (reading 1, authority) or that the lead may perform one
//! with its own hands (reading 2, capability), against three project documents
//! that say the merge queue alone merges. This module does not resolve it and
//! is written so that it does not have to: [`MergeQueue::decide`] never calls
//! `ori_core::permission::permits`, never asks what a lead identity is allowed
//! to do, and treats a recorded [`Approver::Lead`] approval purely as a fact
//! about the candidate, the same way it treats a recorded human approval. That
//! is reading 1, applied at this module's boundary: the lead's approval is an
//! input the queue consumes, "for the merge queue to perform"
//! (`spec/ENV_SETUP.md` section 5), and [`MergeQueue::decide`] is the one place
//! in this crate that is allowed to read it that way, because it is also the
//! one place `spec/SECURITY_NOTES.md` trust boundary 4 says performs the
//! merge. Under reading 2, nothing in this file changes: a
//! [`Reason::LeadApprovalMissing`] refusal still reads exactly the same
//! candidate data the same way. What would change under reading 2 is a
//! different file entirely, `crates/ori-core/src/permission.rs`, whose
//! `permits(Lead, CodeRepository, Merge)` would move from `Allowed` to
//! `Refused`; that file is untouched here, outside this ticket's declared
//! scope, and this module's behavior does not depend on which value it
//! returns. E-0007 stays open.
//!
//! Must not: merge, rebase, run a gate, call an adapter, do IO, spawn a
//! process, make a network call, or hold a credential (`spec/LLD.md` section 2,
//! CLAUDE.md load-bearing facts, and this ticket's instruction).

use core::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::Tier;

// ---------------------------------------------------------------------------
// A commit, named
// ---------------------------------------------------------------------------

/// A commit hash, opaque to this module beyond comparison: AICD §13.
///
/// No methodology section defines a hash format; this exists so that
/// [`Candidate::head`] and [`GateRun::ran_against`] cannot be compared as bare
/// [`String`] values by accident at a call site that meant to compare
/// something else.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Sha(String);

impl Sha {
    /// A commit hash carrying `value`, as the caller's rebase or gate runner
    /// reports it.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The hash as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

/// The gate numbers `spec/CI_CD.md` section 2 conditions a merge on: "gates 1
/// to 10, plus 13 and 14".
///
/// Read against `spec/CI_CD.md` section 1's numbered list: 1 fmt/clippy, 2
/// tests, 3 contract tests, 4 coverage matrix, 5 modified-test detector, 6
/// mutation score, 7 audit/deny/secret scan, 8 forbidden-action test, 9
/// citation gate, 10 UI checks, 13 commit-trailer gate, 14 diagram gate. Gate
/// 11 (the platform build matrix) and gate 12 (the significance labeler,
/// which section 1 states "runs on merge") are deliberately absent: section 2
/// names twelve gates, not fourteen, and a merge that also waited on the
/// labeler would be waiting on itself, since the labeler runs only once
/// merging has already happened.
pub const REQUIRED_GATES: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 13, 14];

/// One required gate's report against one commit: AICD §13, `spec/CI_CD.md`
/// section 2.
///
/// This is a report, not a run: [`MergeQueue::decide`] never executes a gate,
/// so every fact it can act on about one has to arrive already decided. The
/// fields are public because there is no invariant across them for a
/// constructor to protect; [`MergeQueue::decide`] is where the fact that
/// matters, whether the run's [`GateRun::ran_against`] is the candidate's
/// [`Candidate::head`], is read and acted on.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GateRun {
    /// The gate number, from `spec/CI_CD.md` section 1's numbered list.
    pub gate: u8,
    /// The commit this run actually ran against.
    pub ran_against: Sha,
    /// Whether the gate was clean on that commit.
    pub passed: bool,
}

impl GateRun {
    /// One gate's report: gate `gate` ran against `ran_against` and came back
    /// `passed`.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub fn new(gate: u8, ran_against: Sha, passed: bool) -> Self {
        Self {
            gate,
            ran_against,
            passed,
        }
    }
}

// ---------------------------------------------------------------------------
// The single-operator substitutes
// ---------------------------------------------------------------------------

/// One of the six substitute items `ADR-0002` names for tier 2's two-human
/// requirement: AICD §38.
///
/// AICD §38 lists five bullets; `ADR-0002` splits the third ("Rollback
/// rehearsed on staging for that specific change before merge, and for
/// changes with data migrations, a snapshot taken immediately before the
/// migration with a rehearsed restore") into SO-3a and SO-3b, "because its two
/// clauses are in different states", and the other four keep the ADR's own
/// numbers. This enum is that six-item list, spelled the way `ADR-0002` writes
/// it, because that is the list a tier 2 pull request states against.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Substitute {
    /// Independent review by a different model than the coder, with a hard
    /// veto, through the adversarial checklist.
    So1,
    /// The operator's own full diff read, in a separate session from plan
    /// approval, at least several hours later.
    So2,
    /// Rollback rehearsed on staging for that specific change before merge.
    So3a,
    /// For changes with data migrations, a snapshot taken immediately before
    /// the migration with a rehearsed restore.
    So3b,
    /// Migrations backward compatible with the previous tag, behind a flag
    /// where the change is behavioral.
    So4,
    /// Limited rollout where the product supports it, with the operations
    /// agent authorized to halt.
    So5,
}

impl Substitute {
    /// How many substitutes `ADR-0002` names, and the width of a complete
    /// tier 2 record.
    pub const COUNT: usize = 6;

    /// Every substitute, in `ADR-0002`'s own order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::So1,
        Self::So2,
        Self::So3a,
        Self::So3b,
        Self::So4,
        Self::So5,
    ];

    /// The spelling `ADR-0002` writes this substitute under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::So1 => "SO-1",
            Self::So2 => "SO-2",
            Self::So3a => "SO-3a",
            Self::So3b => "SO-3b",
            Self::So4 => "SO-4",
            Self::So5 => "SO-5",
        }
    }
}

impl fmt::Display for Substitute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a tier 2 pull request states one substitute: `ADR-0002`, "satisfied,
/// approximated, not applicable or unavailable, with the reason".
///
/// `ADR-0002` also fixes what separates the last two: "'Not applicable' is
/// claimed only where the substitute has no object; where it has an object
/// and no mechanism, the word is 'unavailable'." [`SubstituteState::blocks`]
/// is where that distinction is spent: only [`SubstituteState::Unavailable`]
/// stops the substitute path, because it is the one word in this list that
/// names a real, unmet requirement rather than an absent or partly-met one.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SubstituteState {
    /// The substitute was carried out in full.
    Satisfied,
    /// The substitute was carried out in part; `ADR-0002` gives SO-1 as the
    /// example, performable without the checklist AICD §38 names.
    Approximated,
    /// The substitute has no object for this change (`ADR-0002`: "no object").
    NotApplicable,
    /// The substitute has an object and no mechanism to meet it
    /// (`ADR-0002`: "unavailable").
    Unavailable,
}

impl SubstituteState {
    /// Whether this state, alone, closes the substitute path for the
    /// substitute it is recorded against: `ADR-0002`.
    ///
    /// Only [`SubstituteState::Unavailable`] does. AICD §38 requires all six
    /// "together", and `ADR-0002` reserves "unavailable" for a substitute with
    /// a real object and no mechanism, which is the one case a queue cannot
    /// treat as met without the "present but reporting nothing" defect
    /// AICD §39 names.
    #[must_use]
    pub const fn blocks(self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

/// One substitute, as a caller reports it for one candidate: `ADR-0002`.
///
/// `reason` is required text because `ADR-0002` requires every state to carry
/// one ("with the reason"); this module does not validate its contents beyond
/// existing, because judging whether a reason is a good one is the human
/// review this control feeds, not a rule this module can check.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SubstituteRecord {
    /// Which of the six this record is.
    pub substitute: Substitute,
    /// Its state for this candidate.
    pub state: SubstituteState,
    /// Why: `ADR-0002` requires a reason for every state, not only a refusal.
    pub reason: String,
}

impl SubstituteRecord {
    /// One substitute recorded as `state`, with `reason`.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub fn new(substitute: Substitute, state: SubstituteState, reason: impl Into<String>) -> Self {
        Self {
            substitute,
            state,
            reason: reason.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------

/// Who an [`Approval`] is from: AICD §13, AICD §17.
///
/// [`Approver::Human`] carries an identifier so that two approvals from the
/// same person can be told apart from two approvals from different people,
/// which is what tier 2's "two humans" turns on (AICD §13). The identifier is
/// whatever the caller's identity system uses; this module only ever compares
/// it for equality.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Approver {
    /// The lead agent, whose approval AICD §13 requires at every tier.
    Lead,
    /// A human, named by the identifier the caller's identity system uses.
    Human(String),
}

/// One recorded approval: AICD §13.
///
/// `read_diff` is meaningful only for a human approval and only matters at
/// tier 2, where AICD §13 requires "at least one human reads the diff itself"
/// as a clause separate from the count of approvers; it is carried on every
/// approval rather than as a separate list because it is a property of one
/// human's action, not a fact about the candidate as a whole.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Approval {
    /// Who approved.
    pub approver: Approver,
    /// Whether this approver is recorded as having read the diff itself.
    pub read_diff: bool,
}

impl Approval {
    /// The lead's approval, carrying no diff-read claim: AICD §13 never asks
    /// the lead to read the diff, only a human.
    #[must_use]
    pub const fn lead() -> Self {
        Self {
            approver: Approver::Lead,
            read_diff: false,
        }
    }

    /// One human's approval, from `identifier`, recording whether they read
    /// the diff.
    #[must_use]
    pub fn human(identifier: impl Into<String>, read_diff: bool) -> Self {
        Self {
            approver: Approver::Human(identifier.into()),
            read_diff,
        }
    }
}

// ---------------------------------------------------------------------------
// The candidate
// ---------------------------------------------------------------------------

/// Everything [`MergeQueue::decide`] is handed about one pull request:
/// `spec/CI_CD.md` section 2.
///
/// Every field is a report from elsewhere in the pipeline: the tier from the
/// ticket the pull request closes, the head from the rebase, the gate runs
/// from whatever ran them, the approvals and substitutes from the record a
/// human or the lead wrote. This module reads them; it produces none of them
/// itself, having no IO to produce them with.
///
/// Fields are public and [`Candidate::new`] sets only the three that have no
/// sensible empty default; the rest start empty and a caller fills them in,
/// the same shape [`GateRun`] and [`Approval`] use, because there is no
/// cross-field invariant here for a constructor to protect: every check this
/// module makes is read out at [`MergeQueue::decide`] time, once, against the
/// whole candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The pull request this candidate is, for messages only; never compared.
    pub pull_request: String,
    /// The risk tier `spec/RISK_MAP.md` and the ticket assign it.
    pub tier: Tier,
    /// The commit that would be merged: the head after the rebase
    /// `spec/CI_CD.md` section 2 performs first.
    pub head: Sha,
    /// The reported runs of the gates the candidate needs.
    pub gate_runs: Vec<GateRun>,
    /// The recorded approvals.
    pub approvals: Vec<Approval>,
    /// The recorded single-operator substitutes, for a tier 2 candidate.
    pub substitutes: Vec<SubstituteRecord>,
    /// The written rollback plan text, for a tier 2 candidate: AICD §13.
    pub rollback_plan: Option<String>,
}

impl Candidate {
    /// A candidate for `pull_request`, at `tier`, whose rebase produced
    /// `head`, with no gate runs, approvals, substitutes or rollback plan yet
    /// recorded.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub fn new(pull_request: impl Into<String>, tier: Tier, head: Sha) -> Self {
        Self {
            pull_request: pull_request.into(),
            tier,
            head,
            gate_runs: Vec::new(),
            approvals: Vec::new(),
            substitutes: Vec::new(),
            rollback_plan: None,
        }
    }
}

/// Whether `plan` is a rollback plan AICD §13 would call "written": present
/// and not only whitespace.
fn rollback_plan_written(plan: &Option<String>) -> bool {
    plan.as_deref().is_some_and(|text| !text.trim().is_empty())
}

// ---------------------------------------------------------------------------
// Reasons and the decision
// ---------------------------------------------------------------------------

/// One reason [`MergeQueue::decide`] refused a candidate, carrying its own
/// [`MethodologyRef`]: CLAUDE.md rule 9.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Reason {
    /// A required gate has no recorded run at all.
    GateMissing {
        /// The gate number, from [`REQUIRED_GATES`].
        gate: u8,
    },
    /// A required gate ran against the candidate's head and was not clean.
    GateFailed {
        /// The gate number, from [`REQUIRED_GATES`].
        gate: u8,
    },
    /// A required gate's recorded run did not run against the candidate's
    /// head: the rebase-then-stale-gate problem this module exists for.
    GateStale {
        /// The gate number, from [`REQUIRED_GATES`].
        gate: u8,
        /// The commit the run actually ran against.
        ran_against: Sha,
    },
    /// No approval from [`Approver::Lead`] is recorded, at any tier.
    LeadApprovalMissing,
    /// Fewer distinct human approvers are recorded than the tier requires.
    HumanApprovalsInsufficient {
        /// How many distinct humans the tier requires.
        required: u8,
        /// How many distinct humans are recorded.
        present: u8,
    },
    /// A tier 2 candidate, on the substitute path, has no record for one of
    /// the six.
    SubstituteMissing {
        /// The substitute with no record.
        substitute: Substitute,
    },
    /// A tier 2 candidate, on the substitute path, records one of the six as
    /// [`SubstituteState::Unavailable`].
    SubstituteUnavailable {
        /// The substitute recorded unavailable.
        substitute: Substitute,
    },
    /// A tier 2 candidate has no written rollback plan.
    RollbackPlanMissing,
    /// A tier 2 candidate has no human approval recorded with
    /// [`Approval::read_diff`].
    DiffNotRead,
}

impl Reason {
    /// The section this reason is refused under: AICD §13 for everything
    /// about gates, lead approval, human counts, the rollback plan and the
    /// diff read; AICD §38 for the six substitutes, which are that section's
    /// own list. Built as a value rather than through the fallible
    /// `MethodologyRef::at`, in the shape `EscalationError::reason` in
    /// `crates/ori-orchestrator/src/escalation.rs` already uses for the same
    /// reason: every section cited below is a literal already known to
    /// resolve.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        match self {
            Self::SubstituteMissing { .. } | Self::SubstituteUnavailable { .. } => MethodologyRef {
                section: 38,
                subsection: None,
            },
            _ => MethodologyRef {
                section: 13,
                subsection: None,
            },
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GateMissing { gate } => {
                write!(f, "gate {gate} has no recorded run against this head")
            }
            Self::GateFailed { gate } => write!(f, "gate {gate} is not green"),
            Self::GateStale { gate, ran_against } => write!(
                f,
                "gate {gate} ran against {ran_against}, not the head being merged"
            ),
            Self::LeadApprovalMissing => write!(f, "no lead approval is recorded"),
            Self::HumanApprovalsInsufficient { required, present } => write!(
                f,
                "{present} of {required} required distinct human approvals are recorded"
            ),
            Self::SubstituteMissing { substitute } => {
                write!(f, "{substitute} has no recorded state")
            }
            Self::SubstituteUnavailable { substitute } => {
                write!(f, "{substitute} is recorded unavailable")
            }
            Self::RollbackPlanMissing => write!(f, "no written rollback plan is recorded"),
            Self::DiffNotRead => write!(f, "no human approval is recorded as having read the diff"),
        }
    }
}

/// What [`MergeQueue::decide`] found: `spec/CI_CD.md` section 2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Every check this module makes passed. This is not a merge, and this
    /// module never performs one; see the module documentation.
    Merge,
    /// At least one check failed. Every reason found is carried, not only the
    /// first, so a human reading a refusal sees the whole picture at once.
    Refuse(Vec<Reason>),
}

impl Decision {
    /// Whether this decision is [`Decision::Merge`].
    #[must_use]
    pub const fn is_merge(&self) -> bool {
        matches!(self, Self::Merge)
    }

    /// The reasons a refusal carries, or an empty slice for [`Decision::Merge`].
    #[must_use]
    pub fn reasons(&self) -> &[Reason] {
        match self {
            Self::Merge => &[],
            Self::Refuse(reasons) => reasons,
        }
    }
}

// ---------------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------------

/// The merge queue's decision function: `spec/CI_CD.md` section 2.
///
/// A zero-sized type rather than a free function only so that the crate's
/// public surface names `MergeQueue`, the item `spec/LLD.md` section 2 gives
/// this crate. It carries no state, because [`MergeQueue::decide`] needs none:
/// every fact it acts on arrives on the [`Candidate`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MergeQueue;

impl MergeQueue {
    /// Decides one candidate: `spec/CI_CD.md` section 2's order, "checks tier
    /// approvals", read after gates, which this function also checks.
    ///
    /// Collects every reason a candidate fails, across gates and approvals,
    /// rather than stopping at the first: [`Decision::Refuse`] never hides a
    /// second problem behind the first one found.
    #[must_use]
    pub fn decide(candidate: &Candidate) -> Decision {
        let mut reasons = gate_reasons(candidate);
        reasons.extend(approval_reasons(candidate));

        if reasons.is_empty() {
            Decision::Merge
        } else {
            Decision::Refuse(reasons)
        }
    }
}

/// Every [`Reason`] the gates on `candidate` produce: `spec/CI_CD.md`
/// section 2, "re-runs gates 1 to 10, plus 13 and 14, on the rebased head".
///
/// Every gate in [`REQUIRED_GATES`] is checked, at every tier: `spec/CI_CD.md`
/// section 2 conditions all three tiers on "green gates", not only tier 0.
///
/// A gate is looked up by scanning every recorded run for it, not by taking
/// the first one found: [`Candidate::gate_runs`] is a plain [`Vec`], and nothing
/// stops a caller recording more than one run for the same gate number, an
/// old one from before a second rebase alongside a fresh one from after it.
/// Reading only the first entry would make the verdict depend on the order
/// the caller happened to push them in, so a stale run recorded before a
/// fresh, passing one could shadow it and refuse a candidate that is actually
/// fine. What decides is whether a run against the current head exists
/// anywhere in the list, not where in the list it sits.
fn gate_reasons(candidate: &Candidate) -> Vec<Reason> {
    let mut reasons = Vec::new();

    for &gate in REQUIRED_GATES {
        let runs_for_gate: Vec<&GateRun> = candidate
            .gate_runs
            .iter()
            .filter(|run| run.gate == gate)
            .collect();

        match runs_for_gate
            .iter()
            .find(|run| run.ran_against == candidate.head)
        {
            Some(fresh) => {
                if !fresh.passed {
                    reasons.push(Reason::GateFailed { gate });
                }
            }
            None => match runs_for_gate.first() {
                Some(stale) => reasons.push(Reason::GateStale {
                    gate,
                    ran_against: stale.ran_against.clone(),
                }),
                None => reasons.push(Reason::GateMissing { gate }),
            },
        }
    }

    reasons
}

/// Every [`Reason`] the approvals on `candidate` produce, for its tier:
/// AICD §13.
fn approval_reasons(candidate: &Candidate) -> Vec<Reason> {
    let mut reasons = Vec::new();

    let lead_approved = candidate
        .approvals
        .iter()
        .any(|approval| matches!(approval.approver, Approver::Lead));
    if !lead_approved {
        reasons.push(Reason::LeadApprovalMissing);
    }

    let distinct_humans = distinct_human_approvers(candidate);

    match candidate.tier {
        Tier::Zero => {}
        Tier::One => {
            const REQUIRED: u8 = 1;
            if distinct_humans < REQUIRED {
                reasons.push(Reason::HumanApprovalsInsufficient {
                    required: REQUIRED,
                    present: distinct_humans,
                });
            }
        }
        Tier::Two => {
            const REQUIRED: u8 = 2;
            let substitute_gaps = substitute_reasons(candidate);
            let humans_path_open = distinct_humans < REQUIRED;
            let substitutes_path_open = !substitute_gaps.is_empty();
            if humans_path_open && substitutes_path_open {
                reasons.push(Reason::HumanApprovalsInsufficient {
                    required: REQUIRED,
                    present: distinct_humans,
                });
                reasons.extend(substitute_gaps);
            }

            if !rollback_plan_written(&candidate.rollback_plan) {
                reasons.push(Reason::RollbackPlanMissing);
            }

            let diff_read = candidate.approvals.iter().any(|approval| {
                matches!(approval.approver, Approver::Human(_)) && approval.read_diff
            });
            if !diff_read {
                reasons.push(Reason::DiffNotRead);
            }
        }
    }

    reasons
}

/// How many distinct human identifiers hold an approval on `candidate`:
/// AICD §13's "two humans" and "one human verifier" both count people, not
/// approvals, so two approvals from the same identifier count once.
fn distinct_human_approvers(candidate: &Candidate) -> u8 {
    let mut identifiers: Vec<&str> = candidate
        .approvals
        .iter()
        .filter_map(|approval| match &approval.approver {
            Approver::Human(identifier) => Some(identifier.as_str()),
            Approver::Lead => None,
        })
        .collect();
    identifiers.sort_unstable();
    identifiers.dedup();
    identifiers.len() as u8
}

/// Every [`Reason`] the substitute path is missing for a tier 2 `candidate`:
/// `ADR-0002`. Empty exactly when all six of [`Substitute::ALL`] are recorded
/// and none is [`SubstituteState::blocks`].
///
/// Built against the fixed six-element [`Substitute::ALL`], never against
/// `candidate.substitutes` itself: a reader built the other way round would
/// ask "does every substitute I was given satisfy itself", which is true of
/// an empty list, and would turn a reader that found nothing into a queue
/// that merges. This is the shape this ticket's planted defect 7 names.
fn substitute_reasons(candidate: &Candidate) -> Vec<Reason> {
    let mut reasons = Vec::new();

    for substitute in Substitute::ALL {
        match candidate
            .substitutes
            .iter()
            .find(|record| record.substitute == substitute)
        {
            None => reasons.push(Reason::SubstituteMissing { substitute }),
            Some(record) if record.state.blocks() => {
                reasons.push(Reason::SubstituteUnavailable { substitute });
            }
            Some(_) => {}
        }
    }

    reasons
}

#[cfg(test)]
mod tests {
    use super::Approval;
    use super::Candidate;
    use super::Decision;
    use super::GateRun;
    use super::MergeQueue;
    use super::REQUIRED_GATES;
    use super::Reason;
    use super::Sha;
    use super::Substitute;
    use super::SubstituteRecord;
    use super::SubstituteState;
    use ori_core::types::Tier;

    /// A [`GateRun`] for every gate `REQUIRED_GATES` names, all against
    /// `head` and all clean, which is what a rebase followed by a fresh
    /// re-run of every required gate looks like.
    fn all_gates_green(head: &Sha) -> Vec<GateRun> {
        REQUIRED_GATES
            .iter()
            .map(|&gate| GateRun::new(gate, head.clone(), true))
            .collect()
    }

    fn all_substitutes(state: SubstituteState) -> Vec<SubstituteRecord> {
        Substitute::ALL
            .iter()
            .map(|&substitute| SubstituteRecord::new(substitute, state, "test fixture"))
            .collect()
    }

    fn assert_refused_with(decision: &Decision, reason: &Reason) {
        match decision {
            Decision::Merge => panic!("expected a refusal, got Decision::Merge"),
            Decision::Refuse(reasons) => assert!(
                reasons.contains(reason),
                "expected {reason:?} among {reasons:?}"
            ),
        }
    }

    // -------------------------------------------------------------------
    // Tier 0 and tier 1: AICD §13's first two rows.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_tier_zero_merges_on_lead_approval_and_green_gates() {
        let head = Sha::new("deadbeef");
        let mut candidate = Candidate::new("PR-1", Tier::Zero, head.clone());
        candidate.gate_runs = all_gates_green(&head);
        candidate.approvals = vec![Approval::lead()];

        assert_eq!(
            MergeQueue::decide(&candidate),
            Decision::Merge,
            "AICD §13 tier 0: CI green and lead agent approval is enough"
        );
    }

    #[test]
    fn ori_t_0053_tier_zero_with_no_lead_approval_is_refused() {
        let head = Sha::new("deadbeef");
        let mut candidate = Candidate::new("PR-1", Tier::Zero, head.clone());
        candidate.gate_runs = all_gates_green(&head);

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::LeadApprovalMissing,
        );
    }

    #[test]
    fn ori_t_0053_tier_zero_with_a_red_gate_is_refused() {
        let head = Sha::new("deadbeef");
        let mut candidate = Candidate::new("PR-1", Tier::Zero, head.clone());
        candidate.gate_runs = all_gates_green(&head);
        candidate.gate_runs[0] = GateRun::new(1, head, false);
        candidate.approvals = vec![Approval::lead()];

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::GateFailed { gate: 1 },
        );
    }

    #[test]
    fn ori_t_0053_tier_one_merges_on_lead_and_one_human_approval_and_green_gates() {
        let head = Sha::new("deadbeef");
        let mut candidate = Candidate::new("PR-2", Tier::One, head.clone());
        candidate.gate_runs = all_gates_green(&head);
        candidate.approvals = vec![Approval::lead(), Approval::human("verifier-1", false)];

        assert_eq!(MergeQueue::decide(&candidate), Decision::Merge);
    }

    #[test]
    fn ori_t_0053_tier_one_with_no_human_approval_is_refused() {
        let head = Sha::new("deadbeef");
        let mut candidate = Candidate::new("PR-2", Tier::One, head.clone());
        candidate.gate_runs = all_gates_green(&head);
        candidate.approvals = vec![Approval::lead()];

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::HumanApprovalsInsufficient {
                required: 1,
                present: 0,
            },
        );
    }

    // -------------------------------------------------------------------
    // Tier 2: AICD §13's third row, and ADR-0002's substitutes.
    // -------------------------------------------------------------------

    fn tier_two_candidate(head: &Sha) -> Candidate {
        let mut candidate = Candidate::new("PR-3", Tier::Two, head.clone());
        candidate.gate_runs = all_gates_green(head);
        candidate.approvals = vec![Approval::lead()];
        candidate.rollback_plan = Some("git revert the merge commit".to_owned());
        candidate
    }

    #[test]
    fn ori_t_0053_tier_two_merges_on_two_distinct_humans_rollback_plan_and_diff_read() {
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.approvals.push(Approval::human("architect", true));
        candidate
            .approvals
            .push(Approval::human("reliability", false));

        assert_eq!(
            MergeQueue::decide(&candidate),
            Decision::Merge,
            "two distinct humans, one of whom read the diff, plus a rollback plan"
        );
    }

    #[test]
    fn ori_t_0053_tier_two_with_one_human_and_no_substitutes_is_refused() {
        // Planted-defect scenario 1: a tier 2 merge admitted with one human
        // and no substitutes. Must fail.
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.approvals.push(Approval::human("operator", true));

        let decision = MergeQueue::decide(&candidate);
        assert_refused_with(
            &decision,
            &Reason::HumanApprovalsInsufficient {
                required: 2,
                present: 1,
            },
        );
        for substitute in Substitute::ALL {
            assert_refused_with(&decision, &Reason::SubstituteMissing { substitute });
        }
    }

    #[test]
    fn ori_t_0053_tier_two_merges_via_substitutes_when_all_six_are_recorded_and_none_unavailable() {
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.approvals.push(Approval::human("operator", true));
        candidate.substitutes = vec![
            SubstituteRecord::new(
                Substitute::So1,
                SubstituteState::Approximated,
                "no checklist yet",
            ),
            SubstituteRecord::new(
                Substitute::So2,
                SubstituteState::Satisfied,
                "read in a later session",
            ),
            SubstituteRecord::new(
                Substitute::So3a,
                SubstituteState::NotApplicable,
                "no staging yet",
            ),
            SubstituteRecord::new(
                Substitute::So3b,
                SubstituteState::NotApplicable,
                "no migration in this change",
            ),
            SubstituteRecord::new(
                Substitute::So4,
                SubstituteState::NotApplicable,
                "no previous tag yet",
            ),
            SubstituteRecord::new(
                Substitute::So5,
                SubstituteState::NotApplicable,
                "no rollout mechanism yet",
            ),
        ];

        assert_eq!(
            MergeQueue::decide(&candidate),
            Decision::Merge,
            "one human plus all six substitutes recorded, none unavailable, opens the substitute path"
        );
    }

    #[test]
    fn ori_t_0053_a_tier_two_substitute_recorded_unavailable_blocks_the_substitute_path() {
        // ADR-0002: SO-3b is unavailable for a change carrying a migration.
        // This is the gap the ADR states loses data if merged past.
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.approvals.push(Approval::human("operator", true));
        candidate.substitutes = all_substitutes(SubstituteState::Satisfied);
        let so3b = candidate
            .substitutes
            .iter_mut()
            .find(|record| record.substitute == Substitute::So3b)
            .expect("SO-3b was recorded above");
        so3b.state = SubstituteState::Unavailable;

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::SubstituteUnavailable {
                substitute: Substitute::So3b,
            },
        );
    }

    #[test]
    fn ori_t_0053_a_tier_two_merge_admitted_with_no_human_and_no_substitutes_is_refused() {
        // Planted-defect scenario 1's neighbor: zero humans is not a weaker
        // version of one human, it is the same refusal for a stronger reason.
        let head = Sha::new("deadbeef");
        let candidate = tier_two_candidate(&head);

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::HumanApprovalsInsufficient {
                required: 2,
                present: 0,
            },
        );
    }

    #[test]
    fn ori_t_0053_two_approvals_from_the_same_identity_are_one_human_not_two() {
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.approvals.push(Approval::human("operator", true));
        candidate.approvals.push(Approval::human("operator", true));

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::HumanApprovalsInsufficient {
                required: 2,
                present: 1,
            },
        );
    }

    #[test]
    fn ori_t_0053_tier_two_with_two_humans_and_no_diff_read_is_refused() {
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.approvals.push(Approval::human("a", false));
        candidate.approvals.push(Approval::human("b", false));

        assert_refused_with(&MergeQueue::decide(&candidate), &Reason::DiffNotRead);
    }

    #[test]
    fn ori_t_0053_tier_two_with_two_humans_and_no_rollback_plan_is_refused() {
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.rollback_plan = None;
        candidate.approvals.push(Approval::human("a", true));
        candidate.approvals.push(Approval::human("b", false));

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::RollbackPlanMissing,
        );
    }

    #[test]
    fn ori_t_0053_a_blank_rollback_plan_does_not_count_as_written() {
        let head = Sha::new("deadbeef");
        let mut candidate = tier_two_candidate(&head);
        candidate.rollback_plan = Some("   \n\t  ".to_owned());
        candidate.approvals.push(Approval::human("a", true));
        candidate.approvals.push(Approval::human("b", false));

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::RollbackPlanMissing,
        );
    }

    // -------------------------------------------------------------------
    // The rebase-then-stale-gate problem: planted-defect scenario 4.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_a_gate_that_ran_against_a_pre_rebase_head_is_stale_and_refused() {
        let old_head = Sha::new("before-the-second-rebase");
        let new_head = Sha::new("after-the-second-rebase");
        let mut candidate = Candidate::new("PR-4", Tier::Zero, new_head.clone());
        candidate.gate_runs = all_gates_green(&old_head);
        candidate.approvals = vec![Approval::lead()];

        let decision = MergeQueue::decide(&candidate);
        for &gate in REQUIRED_GATES {
            assert_refused_with(
                &decision,
                &Reason::GateStale {
                    gate,
                    ran_against: old_head.clone(),
                },
            );
        }
        assert!(
            !decision.is_merge(),
            "a candidate whose gates ran on a superseded head must never merge, \
             whatever `passed` says on each of those runs"
        );
    }

    #[test]
    fn ori_t_0053_one_stale_gate_among_otherwise_fresh_ones_still_refuses() {
        let old_head = Sha::new("stale");
        let new_head = Sha::new("fresh");
        let mut candidate = Candidate::new("PR-4", Tier::Zero, new_head.clone());
        candidate.gate_runs = all_gates_green(&new_head);
        candidate.gate_runs[0] = GateRun::new(1, old_head.clone(), true);
        candidate.approvals = vec![Approval::lead()];

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::GateStale {
                gate: 1,
                ran_against: old_head,
            },
        );
    }

    #[test]
    fn ori_t_0053_a_stale_duplicate_does_not_shadow_a_fresh_passing_run_for_the_same_gate() {
        // A caller may record more than one run for a gate (an old one from
        // before a second rebase, alongside a fresh one from after it). The
        // order those runs were pushed in must not decide the verdict: what
        // matters is whether a run against the current head exists anywhere
        // in the list. Here the stale one is listed first.
        let old_head = Sha::new("before-a-second-rebase");
        let new_head = Sha::new("after-a-second-rebase");
        let mut candidate = Candidate::new("PR-8", Tier::Zero, new_head.clone());
        candidate.gate_runs = all_gates_green(&new_head);
        candidate.gate_runs[0] = GateRun::new(1, old_head, true); // stale, listed first
        candidate.gate_runs.push(GateRun::new(1, new_head, true)); // fresh, listed second
        candidate.approvals = vec![Approval::lead()];

        assert_eq!(
            MergeQueue::decide(&candidate),
            Decision::Merge,
            "a fresh, passing run against the head being merged is authoritative \
             wherever it sits in the list"
        );
    }

    #[test]
    fn ori_t_0053_a_fresh_duplicate_does_not_let_a_stale_run_for_the_same_gate_pass_unnoticed() {
        // The mirror of the test above: the fresh run is listed first and a
        // leftover stale record for the same gate follows it. The candidate
        // still merges, because a run against the current head exists; the
        // leftover stale record is not itself a reason to refuse once a fresh
        // one is present. What defect 4 forbids is merging with NO fresh run
        // at all, not merging while old history is still lying around.
        let old_head = Sha::new("a-superseded-head");
        let new_head = Sha::new("the-current-head");
        let mut candidate = Candidate::new("PR-8", Tier::Zero, new_head.clone());
        candidate.gate_runs = all_gates_green(&new_head);
        candidate.gate_runs.push(GateRun::new(1, old_head, true)); // leftover stale record
        candidate.approvals = vec![Approval::lead()];

        assert_eq!(MergeQueue::decide(&candidate), Decision::Merge);
    }

    #[test]
    fn ori_t_0053_a_missing_required_gate_is_refused() {
        let head = Sha::new("deadbeef");
        let mut candidate = Candidate::new("PR-5", Tier::Zero, head.clone());
        candidate.gate_runs = all_gates_green(&head)
            .into_iter()
            .filter(|run| run.gate != 9)
            .collect();
        candidate.approvals = vec![Approval::lead()];

        assert_refused_with(
            &MergeQueue::decide(&candidate),
            &Reason::GateMissing { gate: 9 },
        );
    }

    #[test]
    fn ori_t_0053_the_required_gates_are_exactly_ci_cd_section_2s_list() {
        assert_eq!(
            REQUIRED_GATES,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 13, 14],
            "spec/CI_CD.md section 2: gates 1 to 10, plus 13 and 14; \
             11 (platform build) and 12 (the significance labeler, which \
             runs on merge) are section 1 gates section 2 does not name"
        );
    }

    // -------------------------------------------------------------------
    // Planted-defect scenario 7: an empty approval set must refuse.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_an_empty_approval_set_refuses_rather_than_vacuously_passing() {
        // Simulates a broken approval reader that found nothing: every tier
        // must refuse on an empty `approvals` list, never merge on it. A
        // check written as "every required approval I was given is present"
        // is vacuously true over an empty list; this proves the checks here
        // are written against the tier's fixed requirement instead.
        let head = Sha::new("deadbeef");
        for tier in [Tier::Zero, Tier::One, Tier::Two] {
            let mut candidate = Candidate::new("PR-6", tier, head.clone());
            candidate.gate_runs = all_gates_green(&head);
            candidate.rollback_plan = Some("git revert the merge commit".to_owned());
            // approvals and substitutes are left empty on purpose.

            let decision = MergeQueue::decide(&candidate);
            assert!(
                !decision.is_merge(),
                "tier {tier:?} must not merge on an empty approval set"
            );
            assert_refused_with(&decision, &Reason::LeadApprovalMissing);
        }
    }

    // -------------------------------------------------------------------
    // Every reason carries a resolving MethodologyRef: CLAUDE.md rule 9.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_every_reason_carries_a_methodology_reference_that_resolves() {
        let sample = [
            Reason::GateMissing { gate: 1 },
            Reason::GateFailed { gate: 2 },
            Reason::GateStale {
                gate: 3,
                ran_against: Sha::new("x"),
            },
            Reason::LeadApprovalMissing,
            Reason::HumanApprovalsInsufficient {
                required: 2,
                present: 0,
            },
            Reason::SubstituteMissing {
                substitute: Substitute::So1,
            },
            Reason::SubstituteUnavailable {
                substitute: Substitute::So3b,
            },
            Reason::RollbackPlanMissing,
            Reason::DiffNotRead,
        ];

        for reason in sample {
            let reference = reason.methodology_ref();
            assert!(
                reference.resolves(),
                "{reason:?} carries {reference}, which must resolve in the methodology index"
            );
        }
    }

    #[test]
    fn ori_t_0053_substitute_reasons_cite_aicd_38_and_everything_else_cites_aicd_13() {
        assert_eq!(
            Reason::SubstituteMissing {
                substitute: Substitute::So1
            }
            .methodology_ref()
            .section,
            38
        );
        assert_eq!(
            Reason::SubstituteUnavailable {
                substitute: Substitute::So5
            }
            .methodology_ref()
            .section,
            38
        );
        assert_eq!(Reason::LeadApprovalMissing.methodology_ref().section, 13);
        assert_eq!(Reason::RollbackPlanMissing.methodology_ref().section, 13);
        assert_eq!(Reason::DiffNotRead.methodology_ref().section, 13);
    }

    // -------------------------------------------------------------------
    // A refusal names every reason found, not only the first.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_a_refusal_lists_every_reason_found_not_only_the_first() {
        let old_head = Sha::new("stale");
        let new_head = Sha::new("fresh");
        let mut candidate = Candidate::new("PR-7", Tier::Two, new_head.clone());
        candidate.gate_runs = all_gates_green(&old_head); // every gate stale
        // no approvals, no substitutes, no rollback plan.

        let decision = MergeQueue::decide(&candidate);
        let reasons = match &decision {
            Decision::Merge => panic!("expected a refusal"),
            Decision::Refuse(reasons) => reasons,
        };

        assert!(
            reasons
                .iter()
                .any(|r| matches!(r, Reason::GateStale { .. }))
        );
        assert!(reasons.contains(&Reason::LeadApprovalMissing));
        assert!(reasons.contains(&Reason::HumanApprovalsInsufficient {
            required: 2,
            present: 0
        }));
        assert!(reasons.contains(&Reason::RollbackPlanMissing));
        assert!(reasons.contains(&Reason::DiffNotRead));
        assert!(
            reasons
                .iter()
                .filter(|r| matches!(r, Reason::SubstituteMissing { .. }))
                .count()
                == Substitute::COUNT,
            "every one of the six substitutes is unaccounted for and named"
        );
    }

    // -------------------------------------------------------------------
    // Planted-defect scenarios 5 and 6: neither always-merge nor
    // always-refuse survives a realistic mixed batch.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_a_realistic_batch_of_candidates_is_neither_always_merged_nor_always_refused() {
        let head = Sha::new("deadbeef");

        let clean_tier_zero = {
            let mut c = Candidate::new("clean-0", Tier::Zero, head.clone());
            c.gate_runs = all_gates_green(&head);
            c.approvals = vec![Approval::lead()];
            c
        };
        let tier_zero_missing_lead = {
            let mut c = Candidate::new("no-lead", Tier::Zero, head.clone());
            c.gate_runs = all_gates_green(&head);
            c
        };
        let tier_one_missing_human = {
            let mut c = Candidate::new("no-human", Tier::One, head.clone());
            c.gate_runs = all_gates_green(&head);
            c.approvals = vec![Approval::lead()];
            c
        };
        let clean_tier_two = {
            let mut c = tier_two_candidate(&head);
            c.approvals.push(Approval::human("a", true));
            c.approvals.push(Approval::human("b", false));
            c
        };
        let stale_tier_zero = {
            let mut c = Candidate::new("stale", Tier::Zero, head.clone());
            c.gate_runs = all_gates_green(&Sha::new("old"));
            c.approvals = vec![Approval::lead()];
            c
        };

        let batch = [
            clean_tier_zero,
            tier_zero_missing_lead,
            tier_one_missing_human,
            clean_tier_two,
            stale_tier_zero,
        ];
        let decisions: Vec<Decision> = batch.iter().map(MergeQueue::decide).collect();

        assert!(
            decisions.iter().any(Decision::is_merge),
            "a queue that merges nothing is broken, not safe"
        );
        assert!(
            decisions.iter().any(|d| !d.is_merge()),
            "a queue that merges everything checked nothing"
        );
    }

    // -------------------------------------------------------------------
    // Unchanged control: a fully green tier 0 candidate, restated once more
    // as the plain baseline every other test in this file varies from.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0053_a_clean_candidate_at_every_tier_merges() {
        let head = Sha::new("deadbeef");
        for tier in [Tier::Zero, Tier::One, Tier::Two] {
            let mut candidate = Candidate::new("clean", tier, head.clone());
            candidate.gate_runs = all_gates_green(&head);
            candidate.approvals = vec![Approval::lead()];
            if tier != Tier::Zero {
                candidate.approvals.push(Approval::human("a", true));
            }
            if tier == Tier::Two {
                candidate.approvals.push(Approval::human("b", false));
                candidate.rollback_plan = Some("git revert the merge commit".to_owned());
            }

            assert_eq!(
                MergeQueue::decide(&candidate),
                Decision::Merge,
                "tier {tier:?} with every requirement met must merge"
            );
        }
    }
}
