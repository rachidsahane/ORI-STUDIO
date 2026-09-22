//! The Phase state machine as pure functions: AICD §23.
//!
//! AICD §23 is "Starting a new product under AICD", the G0 to G7 sequence, and
//! AICD §24 is the M0 to M5 sequence a migration walks;
//! `spec/DATA_MODEL.md` section 2 puts both behind one entity, "Migration phases
//! and roadmap phases share the entity", and its section 3 writes the machine
//! they share. `spec/ROADMAP.md` is this product's own instance of it.
//!
//! # Whether this belongs in this crate
//!
//! `spec/LLD.md` section 2's row for `ori-core` names `Product`, `Ticket`,
//! `Document`, `Category`, `Tier`, `Role`, `Scope` and the error enum, and does
//! not name `Phase`. It also names, separately from that list of types, "state
//! machines as pure functions". `spec/DATA_MODEL.md` section 3 draws five
//! machines, and `spec/RISK_MAP.md` tiers "crates/ori-core (state machines,
//! permission function)" at 2 in the plural. Read together: the list of entities
//! is closed and `Phase` is not in it; the clause about machines is general and
//! this machine is one.
//!
//! So this module holds the machine and not the entity, and that split is not a
//! compromise between two readings, it is forced. `spec/DATA_MODEL.md` section
//! 2 gives `Phase` an `exit_criteria (json)` field, and a crate whose manifest
//! carries no dependency cannot hold a json value; `key (P1.., M0..M5,
//! G0..G7)` is a spelling of this product's roadmap rather than of the
//! methodology, and parsing it needs an ordering this crate cannot see. What is
//! left when both are removed is exactly what a state machine is: a set of
//! states, the moves between them, and the conditions on those moves. All three
//! are here, as pure functions over values a caller passes in.
//!
//! `PhaseControl` in `ori-flows` (`spec/LLD.md` section 2) is what applies them,
//! because it is the crate that can see a product's phases in order and the
//! documents, criteria and gate runs the conditions ask about.
//!
//! # The machine
//!
//! ```mermaid
//! stateDiagram-v2
//!   [*] --> Planned
//!   Planned --> Ready: its document set approved
//!   Ready --> Active: the previous phase is Closed
//!   Active --> Closing: all criteria covered, matrix complete, QA run clean
//!   Closing --> Closed
//! ```
//!
//! `spec/DATA_MODEL.md` section 3 writes this machine as a sentence and not as a
//! diagram, unlike the Ticket and Document machines above it. The diagram is
//! therefore derived rather than copied, and `spec/CONVENTIONS.md` is why it is
//! a diagram at all in Mermaid source: "Every diagram, in every document,
//! generated or hand-written, is Mermaid source." The sentence stays the
//! authority; the test module restates it and reads the chain below out of it.
//!
//! # What this module states and refuses
//!
//! It refuses nothing. The enumeration in `crates/ori-core/src/error.rs` carries
//! a refusal for a ticket transition and one for a document transition and none
//! for a phase, so a refusal made here would either carry a methodology section
//! chosen to fit, which CLAUDE.md rule 9 and AICD §39 are both against, or
//! borrow a variant about another entity. The variant this machine would need is
//! `PhaseTransition { from, to }`, alongside the two that exist, and adding it
//! is a change to an enumeration other crates read, which CLAUDE.md makes an
//! escalation with trigger `contract_change`. Until it exists, everything here
//! is a predicate, in the shape `Scope::overlaps` in
//! `crates/ori-core/src/types.rs` already uses for the lock table: the crate
//! answers, and the crate that can refuse refuses.

use core::fmt;
use core::str::FromStr;

use crate::error::Error;
use crate::error::Result;
use crate::types::DocumentState;

/// Where a phase is in its life: AICD §23.
///
/// The five states of `spec/DATA_MODEL.md` section 3's Phase sentence,
/// `Planned → Ready → Active → Closing → Closed`, in the order it writes them.
///
/// Section 2 writes the `Phase` row's state field bare, so the specification
/// fixes no spelling for these values and the ones [`PhaseState::as_str`]
/// returns are derived: the sentence's own names, lower cased, which is the
/// relation `spec/DATA_MODEL.md` section 3's other machines and the wire
/// spellings in `crates/ori-core/src/types.rs` already stand in. The note at
/// `NOT_SPELLED_BY_SECTION_2` in that file records the same silence for
/// `DocumentState` and `TicketState`, and nothing in this crate asserts a
/// spelling the document does not state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PhaseState {
    /// The phase exists on the roadmap and its document set is not signed.
    Planned,
    /// Its document set is approved, and it has not started.
    Ready,
    /// The phase is running: its tickets are worked.
    Active,
    /// Its criteria are covered, the matrix is complete and the QA run is
    /// clean, and it is being wound up.
    Closing,
    /// The phase is over.
    Closed,
}

impl PhaseState {
    /// Every value, in the order `spec/DATA_MODEL.md` section 3 writes them.
    pub const ALL: &'static [Self] = &[
        Self::Planned,
        Self::Ready,
        Self::Active,
        Self::Closing,
        Self::Closed,
    ];

    /// The spelling this value is stored as, derived from the name
    /// `spec/DATA_MODEL.md` section 3 writes it with.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Closing => "closing",
            Self::Closed => "closed",
        }
    }
}

impl fmt::Display for PhaseState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PhaseState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        for state in Self::ALL {
            if state.as_str() == s {
                return Ok(*state);
            }
        }
        Err(Error::malformed("PhaseState", s))
    }
}

/// Where a phase starts: AICD §23.
///
/// `spec/DATA_MODEL.md` section 3's sentence begins at `Planned`, and
/// `spec/ROADMAP.md` is the document that puts a phase there: a phase exists as
/// soon as the roadmap names it, before anything of its is signed.
pub const INITIAL: PhaseState = PhaseState::Planned;

/// Every move the phase life connects, as (from, to): AICD §23.
///
/// The four steps of `spec/DATA_MODEL.md` section 3's sentence, in the order it
/// writes them. The life is a chain: no step is drawn backwards, none is drawn
/// past its neighbour, and a phase that has closed does not reopen. A pair that
/// is not here is not a move, and that includes every pair whose two states are
/// the same.
pub const TRANSITIONS: &[(PhaseState, PhaseState)] = &[
    (PhaseState::Planned, PhaseState::Ready),
    (PhaseState::Ready, PhaseState::Active),
    (PhaseState::Active, PhaseState::Closing),
    (PhaseState::Closing, PhaseState::Closed),
];

/// Whether the phase life connects `from` to `to`: AICD §23.
#[must_use]
pub fn connects(from: PhaseState, to: PhaseState) -> bool {
    TRANSITIONS
        .iter()
        .any(|(written_from, written_to)| *written_from == from && *written_to == to)
}

/// The one state a phase can move to next, and `None` for `Closed`: AICD §23.
///
/// The life `spec/DATA_MODEL.md` section 3 writes is a chain, so "which moves
/// are legal from here" has at most one answer. This is that fact in code, and
/// the test module holds it against [`TRANSITIONS`] rather than assuming it.
#[must_use]
pub fn successor(from: PhaseState) -> Option<PhaseState> {
    TRANSITIONS
        .iter()
        .find(|(written_from, _)| *written_from == from)
        .map(|(_, written_to)| *written_to)
}

/// Whether a phase's document set allows it to become `Ready`: AICD §23.
///
/// The condition `spec/DATA_MODEL.md` section 3 writes on the first step,
/// `Planned → Ready` "(its document set approved)". AICD §23 is the sequence it
/// belongs to, whose G-gates each end in a human signature, and
/// `spec/ROADMAP.md` is where this product's phase sets are listed, one "Phase
/// set to sign before start" per phase.
///
/// An empty set answers `false`, which is the one place this predicate departs
/// from reading the sentence literally. A check that runs over nothing and
/// reports success is the "present but reporting nothing" defect of AICD §39,
/// and a phase whose set has not been assembled is exactly the case that would
/// reach this with nothing to check. Every phase `spec/ROADMAP.md` names has a
/// set, so nothing legitimate is refused.
#[must_use]
pub fn document_set_approved(states: impl IntoIterator<Item = DocumentState>) -> bool {
    let mut seen = false;
    for state in states {
        seen = true;
        if state != DocumentState::Approved {
            return false;
        }
    }
    seen
}

/// Whether the phase before this one allows it to become `Active`: AICD §23.
///
/// The invariant `spec/DATA_MODEL.md` section 3 states under the Phase
/// sentence: "A phase cannot become `Active` unless the previous phase is
/// `Closed` (roadmap) or its predecessor migration phase is `Closed`." The two
/// halves of that sentence are one rule over two sequences, the roadmap's and
/// the migration's, so what this asks for is the state of the predecessor in
/// whichever sequence the phase belongs to. Which phase that is, and whether it
/// exists, is the caller's to know: `spec/DATA_MODEL.md` section 2 gives `Phase`
/// a `key (P1.., M0..M5, G0..G7)` whose ordering is a fact about a product's
/// roadmap and not about this crate.
///
/// `None` is the first phase of a sequence, which has no predecessor to wait
/// for and is allowed. AICD §23 puts no phase before the first, and
/// `spec/ROADMAP.md` phase 1 is this product's case: nothing precedes it.
#[must_use]
pub fn predecessor_allows_activation(predecessor: Option<PhaseState>) -> bool {
    match predecessor {
        None => true,
        Some(state) => state == PhaseState::Closed,
    }
}

/// What a phase has to show before it can start closing: AICD §15.
///
/// The condition `spec/DATA_MODEL.md` section 3 writes on the third step,
/// `Active → Closing` "(all criteria covered, matrix complete, QA run clean)",
/// as three named fields rather than three arguments in a row, so that two of
/// them cannot be swapped at a call site without the compiler noticing. AICD §15
/// is the continuous verification section those three come from, and criteria
/// ORI-P1-011 and ORI-P1-013 are where the first two are made checkable.
///
/// This is not `Phase.exit_criteria` of `spec/DATA_MODEL.md` section 2. That
/// field is the list of criteria one phase exits on, per product and per phase;
/// these three are the gate the machine itself draws, the same for every phase.
///
/// Every field is evidence gathered elsewhere: the coverage matrix and the QA
/// run are `ori-gates` and the QA role (`spec/LLD.md` section 2), and no field
/// here is computed. What this type contributes is that all three are required
/// and named.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExitConditions {
    /// Every accepted criterion of the phase is covered by a ticket.
    pub criteria_covered: bool,
    /// The coverage matrix is complete: every criterion names a test and every
    /// test names a criterion (`spec/TESTING.md` section 2).
    pub matrix_complete: bool,
    /// The QA run over the phase reported no finding.
    pub qa_run_clean: bool,
}

impl ExitConditions {
    /// Whether all three conditions hold, which is what the third step asks:
    /// AICD §15.
    #[must_use]
    pub const fn met(&self) -> bool {
        self.criteria_covered && self.matrix_complete && self.qa_run_clean
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The sentence `spec/DATA_MODEL.md` section 3 writes the Phase machine
    /// with, copied, backticks and all.
    ///
    /// The chain under test is read out of this rather than written a second
    /// time, so a table that connects nothing, a table that connects everything
    /// and a table one step out all fail. Nothing holds the copy against the
    /// document: this crate may not read a file (`spec/LLD.md` section 2), which
    /// is the limit `crates/ori-core/src/error.rs` restates the methodology
    /// index under and the same gap.
    const SENTENCE: &str = "`Planned → Ready` (its document set approved) `→ Active → Closing` \
                            (all criteria covered, matrix complete, QA run clean) `→ Closed`.";

    /// The invariant `spec/DATA_MODEL.md` section 3 states under that sentence,
    /// copied. Read by a human, cited by the guard test below.
    const INVARIANT: &str = "A phase cannot become `Active` unless the previous phase is \
                             `Closed` (roadmap) or its predecessor migration phase is `Closed`.";

    /// The name `spec/DATA_MODEL.md` section 3 writes each state as.
    const NAMES: &[(&str, PhaseState)] = &[
        ("Planned", PhaseState::Planned),
        ("Ready", PhaseState::Ready),
        ("Active", PhaseState::Active),
        ("Closing", PhaseState::Closing),
        ("Closed", PhaseState::Closed),
    ];

    /// The states [`SENTENCE`] names, in the order it names them.
    ///
    /// The parenthesised conditions are dropped, the backticks with them, and
    /// the sentence is cut at its full stop; what is left is split on the arrow
    /// it is written with.
    fn written_chain() -> Vec<PhaseState> {
        let mut plain = String::new();
        let mut depth = 0_usize;
        for character in SENTENCE.chars() {
            match character {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                '`' => {}
                _ if depth == 0 => plain.push(character),
                _ => {}
            }
        }
        let chain = plain
            .split_once('.')
            .map_or(plain.as_str(), |(head, _)| head);
        chain
            .split('\u{2192}')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| {
                for (written, state) in NAMES {
                    if *written == name {
                        return *state;
                    }
                }
                panic!("the sentence names a phase state {name} and NAMES has no value for it");
            })
            .collect()
    }

    /// Where `state` sits in `PhaseState::ALL`, for reporting a cell.
    fn index_of(state: PhaseState) -> usize {
        PhaseState::ALL
            .iter()
            .position(|listed| *listed == state)
            .expect("PhaseState::ALL lists every state")
    }

    #[test]
    fn ori_t_0021_the_phase_chain_is_the_one_the_data_model_writes() {
        let written = written_chain();
        assert_eq!(
            written.len(),
            5,
            "the copied sentence names {} states and spec/DATA_MODEL.md section 3 names five",
            written.len()
        );
        assert_eq!(
            written,
            PhaseState::ALL.to_vec(),
            "PhaseState::ALL and the sentence it is read from are not the same list, in the same \
             order"
        );

        let steps: Vec<(PhaseState, PhaseState)> =
            written.windows(2).map(|pair| (pair[0], pair[1])).collect();
        assert_eq!(
            TRANSITIONS.to_vec(),
            steps,
            "TRANSITIONS and the sentence it is read from are not the same chain"
        );

        // The whole truth table, all twenty-five ordered pairs of the five
        // states, cell by cell. A table answering false everywhere and a table
        // answering true everywhere each fail here, and so does one that is
        // wrong about a single cell.
        let expected: BTreeSet<(usize, usize)> = steps
            .iter()
            .map(|(from, to)| (index_of(*from), index_of(*to)))
            .collect();
        let mut connected = BTreeSet::new();
        for from in PhaseState::ALL {
            for to in PhaseState::ALL {
                if connects(*from, *to) {
                    connected.insert((index_of(*from), index_of(*to)));
                }
            }
        }
        assert_eq!(
            connected, expected,
            "connects and the sentence disagree over the twenty-five pairs of five states"
        );
        assert_eq!(
            connected.len(),
            4,
            "four of the twenty-five pairs are steps of the chain"
        );
    }

    #[test]
    fn ori_t_0021_a_phase_starts_planned_and_does_not_reopen() {
        assert_eq!(INITIAL, PhaseState::Planned);
        for from in PhaseState::ALL {
            assert!(
                !connects(*from, PhaseState::Planned),
                "nothing returns a phase to Planned, and a move was found out of {from}"
            );
            assert!(
                !connects(PhaseState::Closed, *from),
                "a closed phase does not reopen, and a move was found into {from}"
            );
            assert!(
                !connects(*from, *from),
                "the chain draws no step from {from} to itself"
            );
        }
    }

    #[test]
    fn ori_t_0021_each_phase_state_has_at_most_one_successor() {
        for from in PhaseState::ALL {
            let next: Vec<PhaseState> = PhaseState::ALL
                .iter()
                .copied()
                .filter(|to| connects(*from, *to))
                .collect();
            assert!(
                next.len() <= 1,
                "the life of a phase is a chain and {from} has {} successors",
                next.len()
            );
            assert_eq!(successor(*from), next.first().copied());
        }
        assert_eq!(successor(PhaseState::Closed), None, "Closed is the end");
        assert_eq!(successor(PhaseState::Planned), Some(PhaseState::Ready));
    }

    #[test]
    fn ori_t_0021_a_phase_becomes_ready_only_when_its_whole_document_set_is_approved() {
        assert!(document_set_approved([DocumentState::Approved]));
        assert!(document_set_approved([
            DocumentState::Approved,
            DocumentState::Approved,
        ]));
        assert!(
            !document_set_approved([]),
            "a set with nothing in it is not a set that was checked"
        );
        for state in DocumentState::ALL {
            let whole = document_set_approved([DocumentState::Approved, *state]);
            assert_eq!(
                whole,
                *state == DocumentState::Approved,
                "a set holding one document in state {state} beside an approved one"
            );
        }
    }

    #[test]
    fn ori_t_0021_a_phase_cannot_activate_until_its_predecessor_is_closed() {
        assert!(
            predecessor_allows_activation(None),
            "the first phase of a sequence has no predecessor to wait for: {INVARIANT}"
        );
        for state in PhaseState::ALL {
            assert_eq!(
                predecessor_allows_activation(Some(*state)),
                *state == PhaseState::Closed,
                "a predecessor in state {state}: {INVARIANT}"
            );
        }
    }

    #[test]
    fn ori_t_0021_all_three_exit_conditions_are_required_to_start_closing() {
        let mut met = 0_usize;
        for criteria_covered in [false, true] {
            for matrix_complete in [false, true] {
                for qa_run_clean in [false, true] {
                    let conditions = ExitConditions {
                        criteria_covered,
                        matrix_complete,
                        qa_run_clean,
                    };
                    let expected = criteria_covered && matrix_complete && qa_run_clean;
                    assert_eq!(
                        conditions.met(),
                        expected,
                        "criteria covered {criteria_covered}, matrix complete \
                         {matrix_complete}, QA run clean {qa_run_clean}"
                    );
                    if conditions.met() {
                        met += 1;
                    }
                }
            }
        }
        assert_eq!(
            met, 1,
            "one of the eight combinations of three conditions has all three"
        );
    }

    #[test]
    fn ori_t_0021_every_phase_state_round_trips_through_the_spelling_it_is_stored_as() {
        let mut spellings = BTreeSet::new();
        for state in PhaseState::ALL {
            let spelling = state.as_str();
            assert_eq!(
                spelling.parse::<PhaseState>().expect("round trip"),
                *state,
                "{state} does not read back as itself"
            );
            assert_eq!(state.to_string(), spelling);
            assert!(spellings.insert(spelling), "{spelling} is used twice");
        }
        assert_eq!(spellings.len(), 5);
        let error = "shipped"
            .parse::<PhaseState>()
            .expect_err("spec/DATA_MODEL.md section 3 names no such phase state");
        assert!(
            !error.is_refusal(),
            "a value that did not parse is not a control refusing an action"
        );
        assert!(error.methodology_ref().is_none());
    }
}
