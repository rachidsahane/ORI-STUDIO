//! The closing rules of a ticket, as the evidence a close is decided on:
//! AICD §11, AICD §16.
//!
//! `spec/LLD.md` section 2 gives this crate `ClosingRules`. This module is that
//! entry, and it is deliberately smaller than the name suggests, because the
//! rule itself is already written once and writing it twice is the defect.
//!
//! # What the two rules are
//!
//! AICD §11 closes a ticket only on "the documentation agent's specification
//! update (or its explicit 'no change needed')". AICD §16 adds the second step
//! of its loop for a defect: "a fix without new acceptance criteria produces a
//! bug that can return. Both steps are required for a ticket to close."
//! `spec/PRD.md` states them as one function, T-05, and
//! `spec/DATA_MODEL.md` section 3 makes them an invariant of the Ticket
//! machine: a ticket "cannot enter `Closed` without a `spec_update` event (or
//! `no_change_needed`) and, if `kind` is `defect`, an accepted criterion
//! referencing it".
//!
//! # Why this module decides nothing
//!
//! `crates/ori-core/src/ticket.rs` holds both rules as entry guards on
//! `Closed` and refuses with `RefusalKind::SpecUpdateMissing` and
//! `RefusalKind::CriterionMissing`. It reaches that decision from two values a
//! caller hands it, and its own doc comment says whose job the values are:
//! "This is a report about the event log, not a part of the ticket row. The
//! caller reads the log, which this crate may not, and states what it found."
//!
//! This module is the caller's half. It reads what was recorded and names the
//! record that answers each rule. It does not say whether the close is allowed,
//! because a second copy of a refusal in a second crate is a copy that can
//! drift from the first, and the only thing worse than an unenforced rule is
//! two enforcements of it that disagree.
//!
//! ```mermaid
//! flowchart LR
//!   LOG[(event log)] --> READ[ClosingEvidence::read]
//!   CRIT[(criteria referencing the ticket)] --> READ
//!   READ -->|witnesses| SEAM[the tickets.close handler]
//!   SEAM -->|SpecUpdate, CriterionCoverage| CORE[ori-core Ticket::apply]
//!   CORE -->|accept or refuse with a MethodologyRef| SEAM
//! ```
//!
//! # Why it cannot refuse even if it wanted to
//!
//! `spec/CONVENTIONS.md` requires every refusal to carry a `MethodologyRef`,
//! and that type belongs to `ori-core`, which is not among this crate's
//! dependencies: the manifest lists `ori-gates`, `ori-memory`, `ori-runtime`
//! and `ori-store`. So the seam drawn above, the step that turns a witness into
//! the values `Ticket::apply` takes, cannot be written in this crate as its
//! manifest stands. Adding that one line is outside ticket ORI-T-0052's
//! declared scope and is named in its closing report instead of taken.
//!
//! # The two witnesses, and why one of them is an `Option` and not a third case
//!
//! `ori-core` spells the first answer with three values, because it receives a
//! verdict and must be able to receive "neither was recorded". Here the absent
//! case is the absence of a record, which [`Option`] already says, so
//! [`crate::closing::DocumentationOutcome`] carries only the two answers
//! AICD §11 accepts. The same reading applies to the second rule: a witness is
//! an accepted criterion or there is none.
//!
//! # Where the Merged and Deployed disagreement lands
//!
//! Criteria ORI-P1-006 and ORI-P1-007 in `spec/criteria/phase-1.md` both put
//! the ticket in `Merged`, and `spec/DATA_MODEL.md` section 3 draws
//! `Deployed --> Closed` with no edge from `Merged`. `ori-core` resolved it by
//! answering the entry guards before the transition table, so the refusal each
//! criterion names is the one a `Merged` ticket hears. Nothing here re-decides
//! that: this module never reads the ticket's state, so the disagreement cannot
//! bind on it. What it does impose is a negative duty on the caller above it,
//! which is that the evidence is read before the state is screened. A handler
//! that refused `tickets.close` on a `Merged` ticket for its state alone would
//! make both criteria unreachable while every test below still passed.
//!
//! Must not: decide a close, refuse one, or read the log itself;
//! `spec/LLD.md` section 2 puts the log in `ori-store` and the decision in
//! `ori-core`.

// ---------------------------------------------------------------------------
// What the documentation agent recorded
// ---------------------------------------------------------------------------

/// Which of AICD §11's two acceptable answers a record carries: AICD §11.
///
/// Derived from AICD §11's closing rule, "'Closed' requires the documentation
/// agent's specification update (or its explicit 'no change needed')", which
/// names two answers and treats them alike. `spec/DATA_MODEL.md` section 3
/// spells them `spec_update` and `no_change_needed`.
///
/// Neither value is better evidence than the other, so this type carries no
/// method asking which satisfies the rule: both do, and the case that does not
/// is the one no record of either kind exists for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentationOutcome {
    /// The documentation agent recorded a specification update.
    SpecUpdate,
    /// The documentation agent recorded its explicit "no change needed".
    NoChangeNeeded,
}

impl DocumentationOutcome {
    /// The spelling `spec/DATA_MODEL.md` section 3 gives this answer.
    ///
    /// No methodology section applies beyond the type's own. The spelling is
    /// pinned here and in a test because it is what the log holds and what a
    /// reader of the log matches on, so a rename that the data model did not
    /// make is a decoding fault rather than a matter of taste.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpecUpdate => "spec_update",
            Self::NoChangeNeeded => "no_change_needed",
        }
    }
}

/// One logged record that answers AICD §11's closing rule: AICD §11.
///
/// The `seq` is `spec/DATA_MODEL.md` section 2's `Event.seq`, the monotonic
/// position of the record in the append-only log. It is carried rather than
/// dropped because a witness a human can go and read is worth more than a
/// boolean, and because it is what orders two answers when a ticket has both.
///
/// The fields are public because this is what a reader of the log builds; there
/// is nothing a constructor could refuse that [`ClosingEvidence::read`] does not
/// decide with the whole set in hand.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocumentationRecord {
    /// The position of the record in the log (`spec/DATA_MODEL.md` section 2).
    pub seq: u64,
    /// Which of the two answers it records.
    pub outcome: DocumentationOutcome,
}

impl DocumentationRecord {
    /// A record at `seq` carrying `outcome`.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub const fn new(seq: u64, outcome: DocumentationOutcome) -> Self {
        Self { seq, outcome }
    }
}

// ---------------------------------------------------------------------------
// The criteria that reference the ticket
// ---------------------------------------------------------------------------

/// The state of a `Criterion`: AICD §16.
///
/// Transcribed from `spec/DATA_MODEL.md` section 2, which gives `Criterion` the
/// states "proposed, accepted, rejected, superseded". It is a transcription of
/// the data model and not of a rule; which of these states satisfies AICD §16
/// is [`ClosingEvidence::read`]'s question.
///
/// It lives in this crate because `spec/LLD.md` section 2 gives `ori-core`
/// `Product`, `Ticket`, `Document`, `Category`, `Tier`, `Role` and `Scope`, and
/// `Criterion` is in none of those; reading criteria is this crate's side of the
/// seam. If a later ticket does put `Criterion` in `ori-core`, this enum is to
/// be replaced by that one rather than kept beside it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CriterionState {
    /// Proposed by the QA agent and not yet agreed by a human.
    Proposed,
    /// Accepted.
    Accepted,
    /// Rejected.
    Rejected,
    /// Replaced by a later criterion.
    Superseded,
}

impl CriterionState {
    /// Every state, in the order `spec/DATA_MODEL.md` section 2 lists them.
    ///
    /// Public for the same reason `ori-core` publishes its transition table: a
    /// caller that needs to enumerate the states, and the test that checks this
    /// module answers all of them, must read this list rather than keep a
    /// second one that can disagree with it.
    pub const ALL: &[Self] = &[
        Self::Proposed,
        Self::Accepted,
        Self::Rejected,
        Self::Superseded,
    ];

    /// Whether the criterion is accepted.
    ///
    /// No methodology section applies: this is the field, not the rule. The
    /// rule that only an accepted criterion answers AICD §16 is stated once, in
    /// [`ClosingEvidence::read`].
    #[must_use]
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }

    /// The spelling `spec/DATA_MODEL.md` section 2 gives this state.
    ///
    /// No methodology section applies beyond the type's own. Pinned here and in
    /// a test for the reason [`DocumentationOutcome::as_str`] gives.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }
}

/// A criterion that references the ticket: AICD §16.
///
/// `spec/DATA_MODEL.md` section 1 draws `Criterion }o--o{ Ticket : covered_by`,
/// so a ticket may be referenced by several criteria in several states. This is
/// one such reference, reduced to the two fields the closing rule turns on: the
/// identifier, which is human readable per `spec/DATA_MODEL.md` section 2, and
/// the state.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CriterionRef {
    /// The human-readable identifier, such as `ORI-P1-007`.
    pub id: String,
    /// The state the criterion is in.
    pub state: CriterionState,
}

impl CriterionRef {
    /// A reference to `id` in `state`.
    ///
    /// No methodology section applies: this is the constructor, not a rule.
    #[must_use]
    pub fn new(id: impl Into<String>, state: CriterionState) -> Self {
        Self {
            id: id.into(),
            state,
        }
    }
}

// ---------------------------------------------------------------------------
// The evidence
// ---------------------------------------------------------------------------

/// What the log and the criteria say about closing one ticket: AICD §11,
/// AICD §16.
///
/// One witness per rule, or none where the rule is unwitnessed, plus the
/// criteria that reference the ticket without answering AICD §16, which is what
/// lets a refusal name them. `spec/API_SPEC.md` section 1 gives `tickets.close`
/// nothing but an identifier, so this is the whole of what the call has to go
/// on before `ori-core` is asked.
///
/// The fields are private and [`ClosingEvidence::read`] is the only
/// constructor, because the three are computed together from one input and a
/// caller that could set them separately could state a witness that witnesses
/// nothing, which is the failure mode this type exists to make impossible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosingEvidence {
    documentation: Option<DocumentationRecord>,
    accepted_criterion: Option<CriterionRef>,
    unaccepted_criteria: Vec<CriterionRef>,
}

impl ClosingEvidence {
    /// Reads the evidence for one ticket: AICD §11, AICD §16.
    ///
    /// `records` are the documentation agent's answers logged against the
    /// ticket and `criteria` are the criteria that reference it. Both are
    /// produced by a reader of the store, which is why they arrive as values:
    /// `spec/LLD.md` section 2 puts the log in `ori-store`.
    ///
    /// Three selection rules, each of which the tests below pin:
    ///
    /// 1. The witness for AICD §11 is the record with the lowest `seq`. Both
    ///    outcomes satisfy the section, so the choice changes which record a
    ///    refusal or an acceptance names and never changes the answer. The
    ///    lowest is the first the documentation agent recorded, and the log is
    ///    append-only, so nothing later unmakes it.
    /// 2. The witness for AICD §16 is the first accepted criterion in the order
    ///    given. Only `accepted` qualifies: `spec/TESTING.md` section 2 says
    ///    "Proposed criteria (from the QA agent) do not count until accepted",
    ///    `spec/DATA_MODEL.md` section 2 says they "never enter the coverage
    ///    matrix", and a rejected or superseded criterion is not in force.
    /// 3. Every criterion that is not accepted is kept, in the order given, so
    ///    that a refusal can say which criteria reference the ticket and why
    ///    none of them counted. `ori-core` cannot say this: it never sees an
    ///    identifier.
    ///
    /// Nothing about the ticket's state is read, and nothing about its kind.
    /// Whether AICD §16 applies at all is a question about `Ticket.kind`, and
    /// `ori-core` answers it; a `chore` with no criterion is a complete set of
    /// evidence here and a permitted close there.
    #[must_use]
    pub fn read(records: &[DocumentationRecord], criteria: &[CriterionRef]) -> Self {
        Self {
            documentation: records.iter().min_by_key(|record| record.seq).copied(),
            accepted_criterion: criteria
                .iter()
                .find(|criterion| criterion.state.is_accepted())
                .cloned(),
            unaccepted_criteria: criteria
                .iter()
                .filter(|criterion| !criterion.state.is_accepted())
                .cloned()
                .collect(),
        }
    }

    /// The record that answers AICD §11, if one was logged.
    ///
    /// `None` is the case criterion ORI-P1-006 describes: nothing is recorded,
    /// and the close is refused by `ori-core` citing AICD §11.
    #[must_use]
    pub const fn documentation(&self) -> Option<DocumentationRecord> {
        self.documentation
    }

    /// The accepted criterion that answers AICD §16, if there is one.
    ///
    /// `None` is the case criterion ORI-P1-007 describes for a defect: the
    /// specification update is there and the second step of AICD §16's loop is
    /// not.
    #[must_use]
    pub fn accepted_criterion(&self) -> Option<&CriterionRef> {
        self.accepted_criterion.as_ref()
    }

    /// The criteria that reference the ticket and are not accepted.
    ///
    /// Derived from AICD §16 by way of the selection rules in
    /// [`ClosingEvidence::read`]. This is detail for the refusal and never a
    /// reason for one: a list here with a witness in
    /// [`ClosingEvidence::accepted_criterion`] is an ordinary ticket whose
    /// criteria did not all reach acceptance.
    #[must_use]
    pub fn unaccepted_criteria(&self) -> &[CriterionRef] {
        &self.unaccepted_criteria
    }

    /// Those criteria named, for the sentence a human reads: AICD §16.
    ///
    /// `None` when there are none. The identifiers are the reason this exists:
    /// `ori-core` refuses with "no criterion references it" because that is all
    /// it can see, and this crate can say which ones do and what state they are
    /// in.
    #[must_use]
    pub fn unaccepted_summary(&self) -> Option<String> {
        if self.unaccepted_criteria.is_empty() {
            return None;
        }
        let named: Vec<String> = self
            .unaccepted_criteria
            .iter()
            .map(|criterion| format!("{} ({})", criterion.id, criterion.state.as_str()))
            .collect();
        Some(named.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::ClosingEvidence;
    use super::CriterionRef;
    use super::CriterionState;
    use super::DocumentationOutcome;
    use super::DocumentationRecord;

    fn spec_update(seq: u64) -> DocumentationRecord {
        DocumentationRecord::new(seq, DocumentationOutcome::SpecUpdate)
    }

    fn no_change(seq: u64) -> DocumentationRecord {
        DocumentationRecord::new(seq, DocumentationOutcome::NoChangeNeeded)
    }

    // -----------------------------------------------------------------------
    // ORI-P1-006: the specification update AICD §11 requires.
    //
    // ori-core's ticket machine also claims this criterion, with tests named
    // ori_p1_006_*. It claims the refusal; these claim the evidence the
    // refusal is made on, which that crate takes as a value and cannot read.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_006_a_ticket_with_nothing_recorded_has_no_witness_for_the_closing_rule() {
        let evidence = ClosingEvidence::read(
            &[],
            &[CriterionRef::new("ORI-P1-006", CriterionState::Accepted)],
        );

        assert_eq!(
            evidence.documentation(),
            None,
            "nothing was recorded, so nothing witnesses AICD §11's closing rule"
        );
        assert!(
            evidence.accepted_criterion().is_some(),
            "the other rule is answered, which is what makes this the ORI-P1-006 case and not both"
        );
    }

    #[test]
    fn ori_p1_006_either_answer_the_documentation_agent_records_is_a_witness() {
        for record in [spec_update(7), no_change(7)] {
            let evidence = ClosingEvidence::read(&[record], &[]);
            assert_eq!(
                evidence.documentation(),
                Some(record),
                "AICD §11 accepts the update or the explicit no change needed"
            );
        }
    }

    #[test]
    fn ori_p1_006_the_witness_is_the_first_answer_the_log_holds() {
        let evidence = ClosingEvidence::read(&[no_change(91), spec_update(12), no_change(40)], &[]);

        assert_eq!(
            evidence.documentation(),
            Some(spec_update(12)),
            "the log is append-only and ordered, so the first answer is the answer"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-P1-007: the accepted criterion AICD §16 requires of a defect.
    //
    // ori-core also claims this criterion, in the same way and for the same
    // reason as ORI-P1-006 above.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_007_a_spec_update_with_no_accepted_criterion_leaves_the_second_step_unwitnessed() {
        let evidence = ClosingEvidence::read(
            &[spec_update(3)],
            &[CriterionRef::new("ORI-P1-031", CriterionState::Proposed)],
        );

        assert!(
            evidence.documentation().is_some(),
            "the first step of AICD §16's loop is recorded, which is the precondition of ORI-P1-007"
        );
        assert_eq!(
            evidence.accepted_criterion(),
            None,
            "the second step is not"
        );
        assert_eq!(
            evidence.unaccepted_summary().as_deref(),
            Some("ORI-P1-031 (proposed)"),
            "the refusal can name what does reference the ticket"
        );
    }

    #[test]
    fn ori_p1_007_only_the_accepted_state_answers_the_second_step() {
        assert_eq!(
            CriterionState::ALL.len(),
            4,
            "spec/DATA_MODEL.md section 2 gives Criterion four states"
        );

        let mut witnessed = Vec::new();
        for state in CriterionState::ALL {
            let evidence = ClosingEvidence::read(
                &[spec_update(1)],
                &[CriterionRef::new("ORI-P1-007", *state)],
            );
            if evidence.accepted_criterion().is_some() {
                witnessed.push(*state);
                assert!(
                    evidence.unaccepted_criteria().is_empty(),
                    "a criterion is a witness or it is unaccepted, never both: {state:?}"
                );
            } else {
                assert_eq!(
                    evidence.unaccepted_criteria().len(),
                    1,
                    "a criterion that is not a witness is kept for the refusal: {state:?}"
                );
            }
        }

        assert_eq!(
            witnessed,
            vec![CriterionState::Accepted],
            "proposed criteria do not count until accepted (spec/TESTING.md section 2), and \
             rejected and superseded ones are not in force"
        );
    }

    #[test]
    fn ori_p1_007_an_accepted_criterion_is_found_among_the_others() {
        let evidence = ClosingEvidence::read(
            &[spec_update(1)],
            &[
                CriterionRef::new("ORI-P1-031", CriterionState::Superseded),
                CriterionRef::new("ORI-P1-032", CriterionState::Accepted),
                CriterionRef::new("ORI-P1-033", CriterionState::Proposed),
            ],
        );

        assert_eq!(
            evidence.accepted_criterion(),
            Some(&CriterionRef::new("ORI-P1-032", CriterionState::Accepted)),
            "one accepted criterion answers AICD §16 whatever else references the ticket"
        );
        assert_eq!(
            evidence.unaccepted_summary().as_deref(),
            Some("ORI-P1-031 (superseded), ORI-P1-033 (proposed)"),
            "the rest are kept in the order they were given"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0052: the module's own shape.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0052_the_evidence_is_read_from_the_inputs_and_not_assumed() {
        let nothing = ClosingEvidence::read(&[], &[]);
        assert_eq!(nothing.documentation(), None);
        assert_eq!(nothing.accepted_criterion(), None);
        assert!(nothing.unaccepted_criteria().is_empty());
        assert_eq!(nothing.unaccepted_summary(), None);

        let both = ClosingEvidence::read(
            &[spec_update(5)],
            &[CriterionRef::new("ORI-P1-007", CriterionState::Accepted)],
        );
        assert_eq!(both.documentation(), Some(spec_update(5)));
        assert!(both.accepted_criterion().is_some());
        assert!(both.unaccepted_criteria().is_empty());
        assert_eq!(both.unaccepted_summary(), None);

        assert_ne!(
            nothing, both,
            "a reader that answered the same for an empty log and a complete one would read nothing"
        );
    }

    #[test]
    fn ori_t_0052_the_spellings_are_the_ones_the_data_model_writes() {
        let states: Vec<&str> = CriterionState::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            states,
            vec!["proposed", "accepted", "rejected", "superseded"],
            "spec/DATA_MODEL.md section 2, Criterion.state"
        );
        assert_eq!(DocumentationOutcome::SpecUpdate.as_str(), "spec_update");
        assert_eq!(
            DocumentationOutcome::NoChangeNeeded.as_str(),
            "no_change_needed",
            "spec/DATA_MODEL.md section 3, the Ticket invariant"
        );
    }

    #[test]
    fn ori_t_0052_every_criterion_that_is_not_accepted_is_kept_in_order() {
        let evidence = ClosingEvidence::read(
            &[],
            &[
                CriterionRef::new("C-3", CriterionState::Rejected),
                CriterionRef::new("C-1", CriterionState::Proposed),
                CriterionRef::new("C-2", CriterionState::Superseded),
            ],
        );

        let kept: Vec<&str> = evidence
            .unaccepted_criteria()
            .iter()
            .map(|criterion| criterion.id.as_str())
            .collect();
        assert_eq!(
            kept,
            vec!["C-3", "C-1", "C-2"],
            "the order given is the order the log gave, and this module does not sort it"
        );
    }
}
