//! The escalation triggers and the question each one raises: AICD §12.
//!
//! AICD §12's "Lead agent escalation triggers" is one sentence and a list: "The
//! lead agent must escalate to a human, and may not decide alone, when any of
//! the following occurs", then seven things, then "Each escalation is a question
//! addressed to a human, with the lead agent's recommendation attached."
//! `spec/PRD.md` section 4 carries it as F-06, with eight.
//!
//! # Seven in the methodology, eight here
//!
//! The seven AICD §12 lists are, in its order: a change touches an area covered
//! by an ADR; a test had to be modified or removed for the build to pass; a new
//! dependency or external service is needed; the coder's plan contradicts the
//! specification; two tickets in progress conflict; a change would alter a
//! public or internal contract; the coder's report contains a security concern.
//!
//! The eighth,
//! [`Trigger::PreconditionMissing`],
//! is this product's. AICD §39
//! adopts the rule that a ticket's preconditions are verified before it is
//! worked, and states no trigger for it; `templates/escalation.md` records the
//! same division, `CLAUDE.md` lists all eight as triggers of AICD §12, and
//! `spec/PRD.md` F-06 names all eight without saying where each came from.
//! [`Origin`] is that difference carried in the code
//! rather than in a comment,
//! so that a reader of an escalation can see which authority it rests on.
//!
//! # A trigger is data and a predicate, and what decides it is evidence
//!
//! [`Trigger`] is data: it is the
//! `trigger (enum from methodology 12)` column of
//! `spec/DATA_MODEL.md` section 2's Escalation row, it has a wire spelling that
//! `CLAUDE.md` and `templates/escalation.md` already fix, and it is what a human
//! queue sorts on.
//!
//! It is also a predicate, over a [`Situation`]:
//! the findings the crates that
//! can see them have gathered. None of those observations is made here. The
//! modified-test gate in `ori-gates` sees a diff that touches a test, the lock
//! table sees a claimed module, `ori-memory` sees the ADR that covers an area.
//! What this module does is hold the rule that turns their findings into the
//! set of triggers that fired, once, so that eight callers do not each have
//! their own version of it.
//!
//! The thing a caller hands over is deliberately evidence and not a verdict.
//! `templates/escalation.md` requires an escalation's context to be "what was
//! found, where, with the evidence a human needs to judge it. No conclusions
//! presented as facts", and a boolean carries no evidence, so a caller passing
//! booleans could raise an escalation on its own say-so. A trigger here fires
//! when there is at least one [`Finding`] for it
//! and never otherwise, and
//! [`Escalation::raise`] refuses a
//! trigger with no finding behind it.
//!
//! ```mermaid
//! flowchart LR
//!   G["ori-gates: a modified test"] --> S[Situation]
//!   L["lock table: a claimed module"] --> S
//!   M["ori-memory: an ADR-covered area"] --> S
//!   C["the coder's report: a security concern"] --> S
//!   S --> F["Situation::fired"]
//!   F --> R["Escalation::raise, with question and recommendation"]
//!   R --> H["the human review queue, at the cadence (AICD §12)"]
//! ```
//!
//! # What is not here
//!
//! The answer half, and the row. `spec/DATA_MODEL.md` section 2 gives the
//! Escalation row a `state (open, answered)`, an `answered_by`, an `answer` and
//! an `answered_at`, and an `id`, a `ticket_id` and a `context_package_ref`;
//! `templates/escalation.md` asks a raised escalation for four more fields,
//! "Ticket and anchor", "Raised by", "Options rejected" and "What is blocked".
//! Every one of those is a fact about the session or the store rather than
//! about the trigger: who raised it and when, which package it came with, what
//! is waiting. What is here is the part AICD §12 itself fixes, the trigger, the
//! evidence, the question and the recommendation, and a caller assembling the
//! stored record adds the rest. A complete record is named in this ticket's
//! report as not built.
//!
//! Routing is not here either. AICD §12 sends escalations to "a human review
//! queue that is processed at a fixed cadence" and lets only incidents interrupt;
//! `spec/LLD.md` section 2 gives that to `ori-notify`'s `Router`.
//!
//! Must not: decide an escalation, or route one (AICD §12 gives the decision to
//! a human and `spec/LLD.md` section 2 gives the routing to `ori-notify`).

use core::fmt;
use core::str::FromStr;

use ori_core::error::MethodologyRef;

/// Something that obliges the lead to ask a human: AICD §12.
///
/// Seven of the eight are AICD §12's own list, under the names
/// `templates/escalation.md` and `CLAUDE.md` give them; the eighth is this
/// product's, and [`Origin`] is where that is recorded. The order is AICD §12's,
/// with the product trigger last.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Trigger {
    /// A change touches an area covered by an ADR.
    AdrArea,
    /// A test had to be modified or removed for the build to pass.
    TestModified,
    /// A new dependency or external service is needed.
    NewDependency,
    /// The coder's plan contradicts the specification.
    SpecConflict,
    /// Two tickets in progress conflict, or a declared scope overlaps a module
    /// another ticket has claimed.
    ScopeConflict,
    /// A change would alter a public or internal contract.
    ContractChange,
    /// The coder's report contains a security concern.
    Security,
    /// A precondition the ticket names does not exist.
    PreconditionMissing,
}

impl Trigger {
    /// How many triggers there are: AICD §12 and `spec/PRD.md` section 4.
    ///
    /// Eight: AICD §12's seven and this product's one. Used as the width of the
    /// evidence array in [`Situation`], so a ninth trigger is a compile error
    /// there rather than a trigger nothing can carry evidence for.
    pub const COUNT: usize = 8;

    /// Every trigger, in AICD §12's order with the product trigger last.
    pub const ALL: [Self; Self::COUNT] = [
        Self::AdrArea,
        Self::TestModified,
        Self::NewDependency,
        Self::SpecConflict,
        Self::ScopeConflict,
        Self::ContractChange,
        Self::Security,
        Self::PreconditionMissing,
    ];

    /// The name this trigger is written under: AICD §12.
    ///
    /// The spellings are fixed already and are not invented here: `CLAUDE.md`
    /// writes each as `aicd_escalate(trigger="...")`, `templates/escalation.md`
    /// tabulates the same eight, and `spec/criteria/phase-1.md` writes
    /// `test_modified` into ORI-P1-010's expected result. They are what a
    /// stored Escalation row carries, so they are pinned rather than derived
    /// from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AdrArea => "adr_area",
            Self::TestModified => "test_modified",
            Self::NewDependency => "new_dependency",
            Self::SpecConflict => "spec_conflict",
            Self::ScopeConflict => "scope_conflict",
            Self::ContractChange => "contract_change",
            Self::Security => "security",
            Self::PreconditionMissing => "precondition_missing",
        }
    }

    /// Which authority puts this trigger on the list: AICD §12.
    ///
    /// The module comment sets out the difference and where each side of it is
    /// written down. It is carried per trigger because an escalation raised
    /// under a product trigger and one raised under a methodology trigger are
    /// answerable to different documents, and a human asked to change the list
    /// needs to know which one it is changing.
    pub fn origin(self) -> Origin {
        match self {
            Self::AdrArea
            | Self::TestModified
            | Self::NewDependency
            | Self::SpecConflict
            | Self::ScopeConflict
            | Self::ContractChange
            | Self::Security => Origin::Methodology,
            Self::PreconditionMissing => Origin::Product,
        }
    }

    /// Where this trigger sits in a per-trigger array: no AICD section applies,
    /// it is an implementation detail of [`Situation`].
    fn index(self) -> usize {
        match self {
            Self::AdrArea => 0,
            Self::TestModified => 1,
            Self::NewDependency => 2,
            Self::SpecConflict => 3,
            Self::ScopeConflict => 4,
            Self::ContractChange => 5,
            Self::Security => 6,
            Self::PreconditionMissing => 7,
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Trigger {
    type Err = EscalationError;

    fn from_str(spelling: &str) -> Result<Self, Self::Err> {
        Trigger::ALL
            .into_iter()
            .find(|trigger| trigger.as_str() == spelling)
            .ok_or_else(|| EscalationError::UnknownTrigger {
                spelling: spelling.to_owned(),
            })
    }
}

/// Which document puts a trigger on the list: AICD §12 and AICD §39.
///
/// Derived from the difference between AICD §12's seven-item list and the eight
/// `spec/PRD.md` F-06 and `CLAUDE.md` carry, which the module comment sets out.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Origin {
    /// One of the seven AICD §12 lists itself.
    Methodology,
    /// This product's own, added under a rule the methodology states without
    /// naming a trigger for it.
    Product,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Methodology => "AICD §12",
            Self::Product => "this product",
        })
    }
}

/// One thing that was found, and where: AICD §12.
///
/// Derived from AICD §12's "Each escalation is a question addressed to a human"
/// and from `templates/escalation.md`'s Context field, "What was found, where,
/// with the evidence a human needs to judge it. No conclusions presented as
/// facts". Both halves are required because a finding with no location cannot
/// be checked by the human it is addressed to, and an unverifiable finding is a
/// conclusion presented as a fact.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Finding {
    what: String,
    found_at: String,
}

impl Finding {
    /// One finding, refusing one that says nothing or points nowhere: AICD §12.
    pub fn new(what: &str, found_at: &str) -> Result<Self, EscalationError> {
        if what.trim().is_empty() {
            return Err(EscalationError::Blank { field: "what" });
        }
        if found_at.trim().is_empty() {
            return Err(EscalationError::Blank { field: "where" });
        }

        Ok(Self {
            what: what.trim().to_owned(),
            found_at: found_at.trim().to_owned(),
        })
    }

    /// What was found: AICD §12.
    pub fn what(&self) -> &str {
        &self.what
    }

    /// Where it was found: AICD §12.
    pub fn found_at(&self) -> &str {
        &self.found_at
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.what, self.found_at)
    }
}

/// What has been found about a ticket in flight, per trigger: AICD §12.
///
/// Derived from AICD §12's trigger list read as a set of questions about a
/// ticket rather than as a set of events: "when any of the following occurs"
/// needs something to have observed the occurrence, and this is where those
/// observations are collected before the rule runs over them.
///
/// A situation with nothing in it is the ordinary case: most tickets trip no
/// trigger, and [`Situation::fired`] answers an empty list for an empty
/// situation. That is the one answer this type must get right in both
/// directions, since a rule that fires on every ticket is noise that gets
/// switched off, and one that fires on none is an escalation path that is
/// present and reports nothing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Situation {
    evidence: [Vec<Finding>; Trigger::COUNT],
}

impl Situation {
    /// A ticket about which nothing has been found: AICD §12.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one finding against one trigger: AICD §12.
    ///
    /// The caller says which trigger its finding bears on, because the crate
    /// that made the observation is the one that knows: `ori-gates` knows a
    /// modified test when it parses a diff, and this module could not tell that
    /// finding from a contract change by looking at it.
    pub fn record(&mut self, trigger: Trigger, finding: Finding) {
        self.evidence[trigger.index()].push(finding);
    }

    /// What has been found for one trigger: AICD §12.
    pub fn evidence(&self, trigger: Trigger) -> &[Finding] {
        &self.evidence[trigger.index()]
    }

    /// Whether one trigger has fired: AICD §12.
    ///
    /// It has fired when something was found for it, and not otherwise. There
    /// is no threshold and no severity: AICD §12 says "when any of the
    /// following occurs", and one occurrence is an occurrence.
    pub fn fires(&self, trigger: Trigger) -> bool {
        !self.evidence(trigger).is_empty()
    }

    /// Every trigger that has fired, in AICD §12's order.
    ///
    /// A ticket can trip more than one and they are all returned: a change that
    /// touches an ADR area and also needs a new dependency raises two
    /// questions, and answering one of them answers nothing about the other.
    pub fn fired(&self) -> Vec<Trigger> {
        Trigger::ALL
            .into_iter()
            .filter(|trigger| self.fires(*trigger))
            .collect()
    }
}

/// A question addressed to a human, with the recommendation attached:
/// AICD §12.
///
/// Derived from AICD §12's "Each escalation is a question addressed to a human,
/// with the lead agent's recommendation attached", and holding the four fields
/// `spec/DATA_MODEL.md` section 2's Escalation row requires at the moment one is
/// raised: the trigger, the evidence behind it, the question and the
/// recommendation. The module comment says which fields of that row are not
/// here and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escalation {
    trigger: Trigger,
    evidence: Vec<Finding>,
    question: String,
    recommendation: String,
}

impl Escalation {
    /// Raises one escalation from a situation, refusing three things: AICD §12.
    ///
    /// 1. A trigger that did not fire in this situation. An escalation with no
    ///    finding behind it is a conclusion presented as a fact, which
    ///    `templates/escalation.md` forbids in as many words, and it is how an
    ///    escalation path becomes noise.
    /// 2. A blank question. AICD §12 says an escalation *is* a question; one
    ///    without a question is a notification, and the human queue is not a
    ///    notification feed.
    /// 3. A blank recommendation. AICD §12 requires it attached, and its
    ///    purpose is that the human has something concrete to accept or reject
    ///    rather than a problem handed back.
    ///
    /// The evidence is copied out of the situation rather than referenced, so
    /// that what a human is answering is what was found when the question was
    /// asked, not what the situation has become since.
    pub fn raise(
        trigger: Trigger,
        situation: &Situation,
        question: &str,
        recommendation: &str,
    ) -> Result<Self, EscalationError> {
        if !situation.fires(trigger) {
            return Err(EscalationError::NoEvidence { trigger });
        }
        if question.trim().is_empty() {
            return Err(EscalationError::Blank { field: "question" });
        }
        if recommendation.trim().is_empty() {
            return Err(EscalationError::Blank {
                field: "recommendation",
            });
        }

        Ok(Self {
            trigger,
            evidence: situation.evidence(trigger).to_vec(),
            question: question.trim().to_owned(),
            recommendation: recommendation.trim().to_owned(),
        })
    }

    /// Which trigger raised it: AICD §12.
    pub fn trigger(&self) -> Trigger {
        self.trigger
    }

    /// What was found, as it stood when the question was asked: AICD §12.
    pub fn evidence(&self) -> &[Finding] {
        &self.evidence
    }

    /// The single decision being asked of a human: AICD §12.
    pub fn question(&self) -> &str {
        &self.question
    }

    /// What the agent would do, attached for the human to accept or reject:
    /// AICD §12.
    pub fn recommendation(&self) -> &str {
        &self.recommendation
    }
}

/// Why an escalation, a finding or a trigger name was refused: AICD §12.
///
/// `spec/CONVENTIONS.md` names `thiserror` as the house error convention and
/// ruling R20 in `ops/rulings.md` defers it, stating what stands until then: "a
/// hand-written `Display` and `std::error::Error` implementation stands, with a
/// comment naming the conversion". This is that implementation, and the
/// conversion is mechanical: each arm of the `Display` below becomes an
/// `#[error("...")]` on its variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscalationError {
    /// The trigger has nothing found behind it in this situation.
    NoEvidence {
        /// The trigger that was asked for.
        trigger: Trigger,
    },
    /// A required field was blank.
    Blank {
        /// The field.
        field: &'static str,
    },
    /// The name does not spell any of the eight triggers.
    UnknownTrigger {
        /// What was offered.
        spelling: String,
    },
}

impl EscalationError {
    /// The methodology section this refusal is made under: AICD §12.
    ///
    /// AICD §12 is where the triggers, the question and the recommendation all
    /// come from, including for the one trigger this product adds, because what
    /// is refused is always the shape of an escalation and never the rule that
    /// put a trigger on the list. Built as a value rather than through
    /// `MethodologyRef::at`, which is fallible, in the shape
    /// `RefusalKind::reason` in `crates/ori-core/src/error.rs` already uses for
    /// the same reason.
    pub fn reason(&self) -> MethodologyRef {
        MethodologyRef {
            section: 12,
            subsection: None,
        }
    }
}

impl fmt::Display for EscalationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEvidence { trigger } => write!(
                f,
                "nothing was found for `{trigger}`, and an escalation carries the evidence a \
                 human needs to judge it (AICD §12)"
            ),
            Self::Blank { field } => write!(
                f,
                "an escalation needs a {field}, and this one leaves it blank (AICD §12)"
            ),
            Self::UnknownTrigger { spelling } => write!(
                f,
                "`{spelling}` is not one of the {} escalation triggers (AICD §12)",
                Trigger::COUNT
            ),
        }
    }
}

impl std::error::Error for EscalationError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// One finding, distinct per trigger so that a rule which returns the wrong
    /// trigger's evidence is visible rather than plausible.
    fn finding(trigger: Trigger) -> Finding {
        Finding::new(
            &format!("something that bears on {trigger}"),
            &format!("crates/ori-orchestrator/src/{trigger}.rs:1"),
        )
        .expect("both halves are filled")
    }

    // ---- the list itself ----

    #[test]
    fn ori_t_0051_eight_triggers_of_which_seven_are_the_methodology_s() {
        assert_eq!(Trigger::ALL.len(), Trigger::COUNT);

        let from_methodology: Vec<Trigger> = Trigger::ALL
            .into_iter()
            .filter(|trigger| trigger.origin() == Origin::Methodology)
            .collect();
        let from_product: Vec<Trigger> = Trigger::ALL
            .into_iter()
            .filter(|trigger| trigger.origin() == Origin::Product)
            .collect();

        // AICD §12 lists seven: ADR area, modified test, new dependency, plan
        // against the specification, two tickets in conflict, contract change,
        // security concern. The count is checked as well as the membership,
        // because a count alone passes if a trigger is swapped for another.
        assert_eq!(
            from_methodology,
            vec![
                Trigger::AdrArea,
                Trigger::TestModified,
                Trigger::NewDependency,
                Trigger::SpecConflict,
                Trigger::ScopeConflict,
                Trigger::ContractChange,
                Trigger::Security,
            ],
            "these seven are AICD §12's own list, in its order"
        );
        assert_eq!(
            from_product,
            vec![Trigger::PreconditionMissing],
            "precondition_missing is this product's eighth; AICD §12 does not list it"
        );
    }

    #[test]
    fn ori_t_0051_every_trigger_has_its_own_spelling_and_the_spelling_round_trips() {
        let spellings: Vec<&str> = Trigger::ALL.into_iter().map(Trigger::as_str).collect();
        assert_eq!(
            spellings,
            vec![
                "adr_area",
                "test_modified",
                "new_dependency",
                "spec_conflict",
                "scope_conflict",
                "contract_change",
                "security",
                "precondition_missing",
            ],
            "the spellings CLAUDE.md and templates/escalation.md already fix"
        );

        for trigger in Trigger::ALL {
            assert_eq!(
                trigger.as_str().parse::<Trigger>(),
                Ok(trigger),
                "a stored escalation row reads back as the trigger it was written as"
            );
        }

        // A name that is not one of the eight is refused rather than folded
        // into the nearest one.
        for unknown in ["", "ADR_AREA", "adr-area", "budget_exceeded", "adr_area "] {
            assert!(
                matches!(
                    unknown.parse::<Trigger>(),
                    Err(EscalationError::UnknownTrigger { .. })
                ),
                "`{unknown}` is not a trigger of this product and was accepted as one"
            );
        }
    }

    // ---- the predicate, over every situation there is ----

    #[test]
    fn ori_t_0051_a_situation_with_nothing_found_fires_nothing() {
        let quiet = Situation::new();

        assert!(
            quiet.fired().is_empty(),
            "most tickets trip no trigger, and a rule that fires on a ticket nobody found \
             anything about is noise"
        );
        for trigger in Trigger::ALL {
            assert!(
                !quiet.fires(trigger),
                "{trigger} fired on an empty situation"
            );
            assert!(quiet.evidence(trigger).is_empty());
        }
    }

    #[test]
    fn ori_t_0051_each_trigger_fires_on_its_own_evidence_and_on_no_other() {
        for fired in Trigger::ALL {
            let mut situation = Situation::new();
            situation.record(fired, finding(fired));

            assert_eq!(
                situation.fired(),
                vec![fired],
                "one finding about {fired} fires {fired} and nothing else"
            );
            assert_eq!(
                situation.evidence(fired),
                &[finding(fired)],
                "the escalation carries the finding that fired it"
            );
            for other in Trigger::ALL.into_iter().filter(|other| *other != fired) {
                assert!(
                    situation.evidence(other).is_empty(),
                    "{other} was handed {fired}'s evidence"
                );
            }
        }
    }

    #[test]
    fn ori_t_0051_every_combination_of_findings_fires_exactly_its_own_triggers() {
        // Eight triggers, each either found or not: 256 situations, which is
        // every situation the rule can be asked about, so this is an
        // enumeration and not a sample. It is what separates a rule from the
        // two rules that agree with it on the cases anyone thinks to write:
        // one that fires on everything and one that fires on nothing.
        let mut seen = 0_usize;

        for pattern in 0..(1_u32 << Trigger::COUNT) {
            let mut situation = Situation::new();
            let mut expected = Vec::new();
            for (bit, trigger) in Trigger::ALL.into_iter().enumerate() {
                if pattern & (1 << bit) != 0 {
                    situation.record(trigger, finding(trigger));
                    expected.push(trigger);
                }
            }

            assert_eq!(
                situation.fired(),
                expected,
                "situation {pattern:08b}: the triggers that fired are not the findings recorded"
            );
            for trigger in Trigger::ALL {
                assert_eq!(
                    situation.fires(trigger),
                    expected.contains(&trigger),
                    "situation {pattern:08b}: {trigger}"
                );
            }
            seen += 1;
        }

        assert_eq!(seen, 256, "two states in each of eight triggers");
    }

    #[test]
    fn ori_t_0051_a_second_finding_for_a_trigger_does_not_fire_a_second_trigger() {
        let mut situation = Situation::new();
        situation.record(Trigger::AdrArea, finding(Trigger::AdrArea));
        situation.record(
            Trigger::AdrArea,
            Finding::new("a second ADR area", "spec/adr/ADR-0002-single-operator.md")
                .expect("both halves are filled"),
        );

        assert_eq!(situation.fired(), vec![Trigger::AdrArea]);
        assert_eq!(
            situation.evidence(Trigger::AdrArea).len(),
            2,
            "both findings are kept: a human judging one ADR area is not judging the other"
        );
    }

    // ---- raising the question ----

    #[test]
    fn ori_t_0051_an_escalation_carries_the_trigger_the_evidence_the_question_and_the_recommendation()
     {
        let mut situation = Situation::new();
        situation.record(Trigger::TestModified, finding(Trigger::TestModified));

        let raised = Escalation::raise(
            Trigger::TestModified,
            &situation,
            "may the assertion in this test change, or does the ticket stop here?",
            "stop the ticket: the test states the behaviour the criterion was accepted on",
        )
        .expect("a fired trigger with a question and a recommendation");

        assert_eq!(raised.trigger(), Trigger::TestModified);
        assert_eq!(raised.evidence(), &[finding(Trigger::TestModified)]);
        assert!(raised.question().ends_with('?'));
        assert!(raised.recommendation().starts_with("stop the ticket"));
    }

    #[test]
    fn ori_t_0051_an_escalation_with_no_finding_behind_it_is_refused() {
        let quiet = Situation::new();

        for trigger in Trigger::ALL {
            assert_eq!(
                Escalation::raise(trigger, &quiet, "may I?", "my recommendation"),
                Err(EscalationError::NoEvidence { trigger }),
                "{trigger} was raised about a ticket nobody found anything on"
            );
        }

        // Evidence for one trigger is not evidence for another.
        let mut one = Situation::new();
        one.record(Trigger::Security, finding(Trigger::Security));
        assert_eq!(
            Escalation::raise(Trigger::AdrArea, &one, "may I?", "my recommendation"),
            Err(EscalationError::NoEvidence {
                trigger: Trigger::AdrArea
            })
        );
        assert!(Escalation::raise(Trigger::Security, &one, "may I?", "my recommendation").is_ok());
    }

    #[test]
    fn ori_t_0051_an_escalation_without_a_question_or_a_recommendation_is_refused() {
        let mut situation = Situation::new();
        situation.record(Trigger::NewDependency, finding(Trigger::NewDependency));

        for blank in ["", " ", "\t", "\n"] {
            assert_eq!(
                Escalation::raise(Trigger::NewDependency, &situation, blank, "pin the version"),
                Err(EscalationError::Blank { field: "question" }),
                "AICD §12: an escalation is a question addressed to a human"
            );
            assert_eq!(
                Escalation::raise(
                    Trigger::NewDependency,
                    &situation,
                    "may this crate be added?",
                    blank
                ),
                Err(EscalationError::Blank {
                    field: "recommendation"
                }),
                "AICD §12: with the lead agent's recommendation attached"
            );
        }
    }

    #[test]
    fn ori_t_0051_a_finding_that_says_nothing_or_points_nowhere_is_refused() {
        assert_eq!(
            Finding::new("  ", "crates/ori-orchestrator/src/escalation.rs"),
            Err(EscalationError::Blank { field: "what" })
        );
        assert_eq!(
            Finding::new("a test was modified", ""),
            Err(EscalationError::Blank { field: "where" })
        );
        assert!(Finding::new("a test was modified", "crates/ori-core/src/ticket.rs:900").is_ok());
    }

    #[test]
    fn ori_t_0051_an_escalation_holds_the_evidence_as_it_stood_when_it_was_raised() {
        let mut situation = Situation::new();
        situation.record(Trigger::ScopeConflict, finding(Trigger::ScopeConflict));

        let raised = Escalation::raise(
            Trigger::ScopeConflict,
            &situation,
            "may this ticket start against a claimed module?",
            "queue it behind the claim",
        )
        .expect("a fired trigger with a question and a recommendation");

        situation.record(
            Trigger::ScopeConflict,
            Finding::new("a second overlap", "ops/lock-table.md").expect("both halves are filled"),
        );

        assert_eq!(
            raised.evidence().len(),
            1,
            "a human answers the question that was asked, not one that grew afterwards"
        );
        assert_eq!(situation.evidence(Trigger::ScopeConflict).len(), 2);
    }

    #[test]
    fn ori_t_0051_every_refusal_names_the_methodology_section_it_is_made_under() {
        let refusals = [
            EscalationError::NoEvidence {
                trigger: Trigger::AdrArea,
            },
            EscalationError::Blank { field: "question" },
            EscalationError::UnknownTrigger {
                spelling: "budget_exceeded".to_owned(),
            },
        ];

        for refusal in refusals {
            assert_eq!(
                refusal.reason(),
                MethodologyRef {
                    section: 12,
                    subsection: None
                }
            );
            let sentence = refusal.to_string();
            assert!(
                sentence.contains("AICD §12"),
                "a refusal a human reads says what it is made under: {sentence}"
            );
        }
    }
}
