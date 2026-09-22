//! The Document state machine as pure functions: AICD §9.
//!
//! AICD §9 is the specification system. It gives every document an owner in its
//! document table and states the rule this module walks, "Specification changes
//! are pull requests, reviewed and merged under the same risk tiers as code".
//! `spec/DATA_MODEL.md` section 3 draws that review cycle as a diagram and
//! states two invariants under it. `spec/LLD.md` section 2 gives this crate "the
//! state machines expressed as pure functions"; this is the Document one.
//!
//! Nothing here reads a clock, a file or another workspace crate. Every value a
//! decision needs is passed in.
//!
//! # The cycle
//!
//! ```mermaid
//! stateDiagram-v2
//!   [*] --> Missing
//!   Missing --> Draft: generated or created
//!   Draft --> UnderReview
//!   UnderReview --> Draft: modifications requested
//!   UnderReview --> Approved: seat approves
//!   Approved --> Stale: drift audit finds divergence
//!   Stale --> UnderReview
//! ```
//!
//! Copied from `spec/DATA_MODEL.md` section 3, which is the source; the copy is
//! held against that source by nothing mechanical, and the test module restates
//! the same seven lines so that the table below and the diagram cannot drift
//! apart without a failure. `spec/CONVENTIONS.md` requires every diagram an
//! agent produces to be Mermaid source, which is why the copy is the diagram and
//! not a picture of it.
//!
//! # What an event is here
//!
//! `spec/LLD.md` section 2 writes the shape as `Ticket::apply(event) ->
//! Result<Ticket>` and puts the `Event` entity in `ori-store`, which this crate
//! may not import. So the transition input is not the store's row: it is
//! [`DocumentEvent`], a value naming one move of the diagram, and the store's
//! row is what a caller reads one of these out of. The names
//! [`DocumentEvent::as_str`] returns are the `domain.verb_past` form
//! `spec/LLD.md` section 4 fixes; only `document.approved` is spelled by the
//! specification, in criterion ORI-P1-040, and the other four are derived from
//! the diagram's edge labels and are the store's to fix, not this crate's.
//!
//! # What this module refuses, and what it only states
//!
//! Three decisions belong to this crate and two of them can be refused here.
//!
//! The pair of states is refused: [`Document::apply`] returns
//! `RefusalKind::DocumentTransition` for a move the diagram does not draw. The
//! signing seat is refused: it returns `RefusalKind::DocumentSeatMismatch` for
//! the first invariant, "only the owning seat may sign".
//!
//! The third is stated and not refused. The diagram's edges carry labels, so the
//! relation it draws is (state, event) and not only (from, to), and two cells of
//! the first are not in the second: `document.created` applied to a document
//! under review, and `document.changes_requested` applied to a document that is
//! missing. Both reach a state the pair table admits, because two edges of the
//! diagram end at `Draft`. Refusing them needs a refusal that names an event,
//! and the enumeration in `crates/ori-core/src/error.rs` names only states; the
//! nearest variant would print "a document cannot move from under_review to
//! draft", which is false, and a false sentence in front of a human is the
//! defect class AICD §39 exists for. So [`DocumentEvent::fires_from`] states the
//! labelled relation as a total predicate, the two cells are pinned by a test,
//! and the refusal this would need is reported rather than improvised. That
//! variant is `DocumentEventNotApplicable { state, event }`, and adding it is a
//! change to an enumeration other crates read, which CLAUDE.md makes an
//! escalation with trigger `contract_change`.
//!
//! # The human actor
//!
//! Criterion ORI-P1-040 requires the `document.approved` event to carry "seat
//! and human actor". A seat is held by a person: AICD §18 organizes the team in
//! seats and `spec/DATA_MODEL.md` section 2 gives the `Seat` row a "holder
//! (human identity)". [`Signature::of`] is therefore the only door into an
//! approval and it yields nothing for an agent or for the system. It yields
//! `None` rather than a refusal because an agent attempting an approval is a
//! permission decision, and `crates/ori-core/src/error.rs` records that this
//! crate holds no permission refusal yet: the permission function is ORI-T-0022.
//! Inventing a refusal here would put that decision in two places.
//!
//! # The owning seat is not a field
//!
//! `spec/DATA_MODEL.md` section 2's `Document` row has no owner column, while
//! `spec/API_SPEC.md` returns "documents with state, owner seat, verification
//! date" and PRD D-01 lists "owner seat" among a document's attributes. Until
//! the row carries it, the owner cannot be read off the value, so
//! [`Document::apply`] takes it. That is a gap in the specification and is
//! reported as one.

use core::fmt;

use crate::error::Error;
use crate::error::RefusalKind;
use crate::error::Result;
use crate::types::Actor;
use crate::types::Document;
use crate::types::DocumentSet;
use crate::types::DocumentState;
use crate::types::Id;
use crate::types::Seat;
use crate::types::Timestamp;

/// Where a document starts: AICD §9.
///
/// `spec/DATA_MODEL.md` section 3's Document diagram draws `[*] --> Missing`.
/// Criterion ORI-P1-001 reads the same value from the other side: `ori init`
/// produces a skeleton with "every foundation document in state Missing".
pub const INITIAL: DocumentState = DocumentState::Missing;

/// Every move the review cycle connects, as (from, to): AICD §9.
///
/// The six edges of `spec/DATA_MODEL.md` section 3's Document diagram, in the
/// order it draws them. This is the whole of the relation. A pair that is not
/// here is refused, and that includes every pair whose two states are the same:
/// the diagram draws no self transition, so an event that would leave a document
/// where it is does not move it.
pub const TRANSITIONS: &[(DocumentState, DocumentState)] = &[
    (DocumentState::Missing, DocumentState::Draft),
    (DocumentState::Draft, DocumentState::UnderReview),
    (DocumentState::UnderReview, DocumentState::Draft),
    (DocumentState::UnderReview, DocumentState::Approved),
    (DocumentState::Approved, DocumentState::Stale),
    (DocumentState::Stale, DocumentState::UnderReview),
];

/// Whether the review cycle connects `from` to `to`: AICD §9.
///
/// The predicate only. [`Document::apply`] is what refuses a move it answers
/// `false` for, and `ori-flows` is what offers the move in the first place
/// (`spec/LLD.md` section 2).
#[must_use]
pub fn connects(from: DocumentState, to: DocumentState) -> bool {
    TRANSITIONS
        .iter()
        .any(|(drawn_from, drawn_to)| *drawn_from == from && *drawn_to == to)
}

/// Who signed an approval, and the human behind the seat: AICD §18.
///
/// Derived from AICD §18, which organizes the team "in three seats plus a
/// product owner" and gives each seat what it approves, read with
/// `spec/DATA_MODEL.md` section 2's `Seat` row, "seat (...), holder (human
/// identity)". A seat is a set of responsibilities held by a person, so an
/// approval names both, which is what criterion ORI-P1-040 asks the
/// `document.approved` event to carry: "seat and human actor".
///
/// The fields are private and [`Signature::of`] is the only constructor. The one
/// thing this type asserts is that a human made the approval, and public fields
/// would let a struct literal assert it falsely. That is the opposite choice
/// from `MethodologyRef` in `crates/ori-core/src/error.rs`, whose fields are
/// public because `spec/LLD.md` section 4 writes its shape out as a contract;
/// nothing writes this one out, so nothing requires the door to be open.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Signature {
    /// The seat that signed.
    seat: Seat,
    /// The human identity holding that seat.
    holder: Id,
    /// When the signature was made.
    at: Timestamp,
}

impl Signature {
    /// The signature of `actor` sitting in `seat`, and `None` when `actor` is
    /// not a human: AICD §18.
    ///
    /// AICD §18 gives approvals to seats and seats to people, so an agent
    /// identity and the system hold none and can sign nothing. Criterion
    /// ORI-P1-040 is where that shows: the event it describes carries a human
    /// actor.
    ///
    /// The answer is an [`Option`] and not a refusal. A refusal would have to
    /// carry a methodology section (CLAUDE.md rule 9) for a decision this crate
    /// does not yet make: an agent attempting a human's action is the permission
    /// function's answer, which is ORI-T-0022, and
    /// `crates/ori-core/src/error.rs` records that no permission refusal exists
    /// here yet. What this constructor does instead is make the approval
    /// unbuildable, so no caller can reach [`Document::apply`] with one.
    #[must_use]
    pub fn of(seat: Seat, actor: &Actor, at: Timestamp) -> Option<Self> {
        match actor {
            Actor::Human(holder) => Some(Self {
                seat,
                holder: holder.clone(),
                at,
            }),
            Actor::Agent(_) | Actor::System => None,
        }
    }

    /// The seat that signed, which the first invariant of
    /// `spec/DATA_MODEL.md` section 3 checks against the owning seat.
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    /// The human identity behind the seat, which criterion ORI-P1-040 requires
    /// the `document.approved` event to carry.
    #[must_use]
    pub const fn holder(&self) -> &Id {
        &self.holder
    }

    /// When the signature was made, which becomes the document's `approved_at`.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} held by {}", self.seat, self.holder)
    }
}

/// What happened to a document: AICD §9.
///
/// One value per edge of `spec/DATA_MODEL.md` section 3's Document diagram, with
/// the two unlabelled edges into `UnderReview` sharing
/// [`DocumentEvent::Submitted`] because the diagram gives them one meaning.
///
/// The `Event` entity is `ori-store`'s (`spec/LLD.md` section 2) and this crate
/// may not import it, so this is what a caller reads out of a stored event
/// before asking this machine to decide. It carries only what a decision needs:
/// the seat and the human for an approval, the moment the drift audit looked for
/// a divergence, and nothing at all for the other three.
///
/// The enumeration is closed rather than `non_exhaustive`, because the diagram
/// it is read from is closed. A downstream match that stops compiling when an
/// edge is added to the diagram is the check that behavior is wanted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentEvent {
    /// The document was generated or written, which is the edge out of
    /// `Missing`.
    Created,
    /// The document was submitted to its owning seat, which is the edge into
    /// `UnderReview` from `Draft` and the one from `Stale`.
    Submitted,
    /// The seat asked for modifications, which is the edge back to `Draft`.
    ChangesRequested,
    /// The owning seat signed, which is the edge into `Approved` and the whole
    /// of criterion ORI-P1-040.
    Approved {
        /// The seat that signed and the human behind it.
        signature: Signature,
    },
    /// The drift audit found the document and the code disagree, which is the
    /// edge into `Stale`.
    Drifted {
        /// When the drift audit looked.
        at: Timestamp,
    },
}

impl DocumentEvent {
    /// The event kind, in the `domain.verb_past` form `spec/LLD.md` section 4
    /// fixes: AICD §13.
    ///
    /// AICD §13 makes the log the audit trail, which is what gives an event a
    /// stable name at all; `spec/LLD.md` section 4 fixes the form, "Events:
    /// `domain.verb_past` (`ticket.validated`, `gate.proven`)", and
    /// `spec/API_SPEC.md` lists `document.*` among the kinds a client
    /// subscribes to.
    ///
    /// Only `document.approved` is spelled by the specification, by criterion
    /// ORI-P1-040. The other four are derived from the diagram's edge labels and
    /// from the two client API methods that drive them,
    /// `flows.documents.approve` and `flows.documents.requestChanges`. The store
    /// owns the wire form and may spell them otherwise; what this crate fixes is
    /// the one the criterion names.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "document.created",
            Self::Submitted => "document.submitted",
            Self::ChangesRequested => "document.changes_requested",
            Self::Approved { .. } => "document.approved",
            Self::Drifted { .. } => "document.drifted",
        }
    }

    /// Where this event leaves the document: AICD §9.
    ///
    /// Each edge of `spec/DATA_MODEL.md` section 3's Document diagram ends
    /// somewhere, and every edge one event stands for ends in the same place, so
    /// the target is a property of the event alone.
    #[must_use]
    pub const fn target(&self) -> DocumentState {
        match self {
            Self::Created | Self::ChangesRequested => DocumentState::Draft,
            Self::Submitted => DocumentState::UnderReview,
            Self::Approved { .. } => DocumentState::Approved,
            Self::Drifted { .. } => DocumentState::Stale,
        }
    }

    /// Whether `spec/DATA_MODEL.md` section 3 draws this event leaving `state`:
    /// AICD §9.
    ///
    /// The labelled relation, which is narrower than [`connects`] in exactly two
    /// cells: `document.created` out of `UnderReview` and
    /// `document.changes_requested` out of `Missing`. Both end at `Draft`, which
    /// the other edge into `Draft` makes reachable from those states, so the
    /// pair table admits what the labels do not.
    ///
    /// Stated and not refused. The head of this module carries the reason and
    /// the refusal variant it would take.
    #[must_use]
    pub fn fires_from(&self, state: DocumentState) -> bool {
        match self {
            Self::Created => state == DocumentState::Missing,
            Self::Submitted => state == DocumentState::Draft || state == DocumentState::Stale,
            Self::ChangesRequested | Self::Approved { .. } => state == DocumentState::UnderReview,
            Self::Drifted { .. } => state == DocumentState::Approved,
        }
    }
}

impl fmt::Display for DocumentEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Document {
    /// One move of the review cycle: AICD §9.
    ///
    /// The transition function `spec/LLD.md` section 2 names, which it writes
    /// for the other machine as `Ticket::apply(event) -> Result<Ticket>`. Pure:
    /// it reads the document and returns the next one, and the document it was
    /// given is untouched whether it refuses or not. That is the receiver a
    /// projection wants. `ori-store`'s log is append-only, so a projector that
    /// refuses an event has to carry on from the state it already held, and a
    /// receiver taken by value would have consumed it.
    ///
    /// `owner` is the seat that owns this document, which
    /// `spec/DATA_MODEL.md` section 2's `Document` row does not carry as a
    /// field. The head of this module records that gap. It is required on every
    /// call and not only on an approval, because AICD §9's document table gives
    /// every document an owner and a caller that cannot name it is a caller that
    /// does not know which document it holds.
    ///
    /// # Refusals
    ///
    /// The pair first, the seat second. A move the diagram does not draw is
    /// refused whoever attempts it, so `RefusalKind::DocumentTransition` comes
    /// before `RefusalKind::DocumentSeatMismatch`: a stranger approving a draft
    /// is told the draft was never up for approval, which is true of the
    /// document, rather than told about itself.
    ///
    /// # Errors
    ///
    /// `RefusalKind::DocumentTransition` when the diagram does not connect the
    /// document's state to the event's target, and
    /// `RefusalKind::DocumentSeatMismatch` when a seat that is not `owner`
    /// signs, which is the first invariant under
    /// `spec/DATA_MODEL.md` section 3's Document diagram: "only the owning seat
    /// may sign".
    pub fn apply(&self, event: &DocumentEvent, owner: Seat) -> Result<Self> {
        let from = self.state;
        let to = event.target();
        if !connects(from, to) {
            return Err(Error::refused_with(
                RefusalKind::DocumentTransition { from, to },
                format!("the event was {event}"),
            ));
        }

        let mut next = self.clone();
        next.state = to;
        match event {
            DocumentEvent::Approved { signature } => {
                if signature.seat() != owner {
                    return Err(Error::refused_with(
                        RefusalKind::DocumentSeatMismatch {
                            owner,
                            signer: signature.seat(),
                        },
                        format!("the signature was {signature}"),
                    ));
                }
                next.approved_by = Some(signature.seat());
                next.approved_at = Some(signature.at());
            }
            DocumentEvent::Drifted { at } => {
                next.verified_against_code_at = Some(*at);
            }
            DocumentEvent::Created | DocumentEvent::Submitted | DocumentEvent::ChangesRequested => {
            }
        }
        Ok(next)
    }
}

/// Whether this document disables Launch: AICD §23.
///
/// The second invariant under `spec/DATA_MODEL.md` section 3's Document diagram:
/// "a foundation document in any state but `Approved` disables Launch". AICD §23
/// is what that sentence guards, the G0 to G7 sequence a new product starts
/// through, and criterion ORI-P1-003 is the same rule seen from outside: with
/// one foundation document short, `ori launch` is "Refused with reason citing
/// AICD §23 and the one missing document".
///
/// The predicate only. `Readiness` and the launch refusal are `ori-flows`
/// (`spec/LLD.md` section 2), which is the crate that can see every document of
/// a product at once; this answers for one.
#[must_use]
pub fn disables_launch(document: &Document) -> bool {
    document.set == DocumentSet::Foundation && document.state != DocumentState::Approved
}

/// Whether a document's signature fields agree with its state: AICD §9.
///
/// [`Document::apply`] keeps this true by construction, and it is stated
/// separately because a struct literal can break it: `spec/LLD.md` section 4
/// makes `Document`'s fields part of what other crates read and
/// `crates/ori-core/src/types.rs` leaves them public, so this crate cannot be
/// the only door into the value. `MethodologyRef::resolves` in
/// `crates/ori-core/src/error.rs` answers the same question for the same reason,
/// and this is that answer for this type.
///
/// What is checked, read off the reachable states of
/// `spec/DATA_MODEL.md` section 3's Document diagram:
///
/// - a signature is whole or absent, never half, because one call records both
///   halves;
/// - `Approved` and `Stale` carry one, because the only edge into `Approved` is
///   the signing edge and the only edge into `Stale` leaves `Approved`;
/// - `Missing` carries none, because nothing returns to it;
/// - `Draft` and `UnderReview` may carry one or not, because each is reachable
///   both before a first approval and after one.
///
/// A false answer is a document no sequence of events could have produced. It is
/// not a refusal: nothing was attempted, so there is no action to refuse.
#[must_use]
pub fn signature_agrees_with_state(document: &Document) -> bool {
    let seat = document.approved_by.is_some();
    let at = document.approved_at.is_some();
    if seat != at {
        return false;
    }
    match document.state {
        DocumentState::Missing => !seat,
        DocumentState::Approved | DocumentState::Stale => seat,
        DocumentState::Draft | DocumentState::UnderReview => true,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::types::DocumentKind;

    /// A ULID that parses, for the human behind a seat.
    const HOLDER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// The seven lines `spec/DATA_MODEL.md` section 3 draws the Document
    /// machine with, copied.
    ///
    /// The table under test is read from these rather than written out a second
    /// time, so a table that connects nothing and a table that connects
    /// everything both fail, and so does a table that quietly gains or loses one
    /// edge. Nothing holds this copy against the document itself: this crate may
    /// not read a file (`spec/LLD.md` section 2), which is the same limit
    /// `crates/ori-core/src/error.rs` restates the methodology index under, and
    /// the same gap.
    const DIAGRAM: &[&str] = &[
        "[*] --> Missing",
        "Missing --> Draft: generated or created",
        "Draft --> UnderReview",
        "UnderReview --> Draft: modifications requested",
        "UnderReview --> Approved: seat approves",
        "Approved --> Stale: drift audit finds divergence",
        "Stale --> UnderReview",
    ];

    /// The node name `spec/DATA_MODEL.md` section 3 draws each state as.
    ///
    /// Section 2 writes the `Document` row's state field bare, so the wire
    /// spellings in `crates/ori-core/src/types.rs` are not the document's and
    /// cannot be used to read the diagram. These names are.
    const NODES: &[(&str, DocumentState)] = &[
        ("Missing", DocumentState::Missing),
        ("Draft", DocumentState::Draft),
        ("UnderReview", DocumentState::UnderReview),
        ("Approved", DocumentState::Approved),
        ("Stale", DocumentState::Stale),
    ];

    /// The state a diagram node names, or `None` for the start marker.
    fn node(name: &str) -> Option<DocumentState> {
        if name == "[*]" {
            return None;
        }
        for (drawn, state) in NODES {
            if *drawn == name {
                return Some(*state);
            }
        }
        panic!("the diagram draws a node named {name} and NODES has no state for it");
    }

    /// The edges [`DIAGRAM`] draws between two states, dropping the birth edge.
    fn drawn_edges() -> Vec<(DocumentState, DocumentState)> {
        let mut edges = Vec::new();
        for line in DIAGRAM {
            let (from, rest) = line
                .split_once("-->")
                .unwrap_or_else(|| panic!("{line} is not an edge of a Mermaid state diagram"));
            let head = rest.split(':').next().unwrap_or("").trim();
            match (node(from.trim()), node(head)) {
                (Some(from), Some(to)) => edges.push((from, to)),
                (None, Some(_)) => {}
                (_, None) => panic!("{line} ends at the start marker"),
            }
        }
        edges
    }

    /// One document, in `state`, of `set`, with no signature.
    fn document(state: DocumentState, set: DocumentSet) -> Document {
        Document {
            id: Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("a well formed ULID"),
            product_id: Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAX").expect("a well formed ULID"),
            path: "spec/DATA_MODEL.md".to_owned(),
            kind: DocumentKind::DataModel,
            set,
            state,
            approved_by: None,
            approved_at: None,
            verified_against_code_at: None,
        }
    }

    /// The signature of the architect, which is the seat AICD §9's document
    /// table gives the domain model.
    fn architect() -> Signature {
        Signature::of(
            Seat::Architect,
            &Actor::Human(Id::parse(HOLDER).expect("a well formed ULID")),
            Timestamp::from_millis(1_764_000_000_000),
        )
        .expect("a human can sign")
    }

    /// One value of every [`DocumentEvent`], which the cell by cell tests run
    /// over.
    ///
    /// Written out because two of the five carry a field. The exhaustive match
    /// in [`event_tag`] is what refuses a variant that is not listed.
    fn every_event() -> Vec<DocumentEvent> {
        vec![
            DocumentEvent::Created,
            DocumentEvent::Submitted,
            DocumentEvent::ChangesRequested,
            DocumentEvent::Approved {
                signature: architect(),
            },
            DocumentEvent::Drifted {
                at: Timestamp::from_millis(1_764_000_000_001),
            },
        ]
    }

    /// Where [`every_event`] lists this event's variant.
    ///
    /// Exhaustive and wildcard free, so a variant added to [`DocumentEvent`]
    /// stops the build here, and the assertion below is what turns that into a
    /// value in the list rather than only an arm.
    fn event_tag(event: &DocumentEvent) -> usize {
        match event {
            DocumentEvent::Created => 0,
            DocumentEvent::Submitted => 1,
            DocumentEvent::ChangesRequested => 2,
            DocumentEvent::Approved { .. } => 3,
            DocumentEvent::Drifted { .. } => 4,
        }
    }

    #[test]
    fn ori_t_0021_every_document_event_is_covered_by_these_tests() {
        let events = every_event();
        for (index, event) in events.iter().enumerate() {
            assert_eq!(
                event_tag(event),
                index,
                "every_event lists {event} at position {index} and event_tag puts that variant \
                 at another, so one variant is listed twice and another not at all"
            );
        }
        assert_eq!(
            events.len(),
            5,
            "spec/DATA_MODEL.md section 3 draws six edges between states and two of them, the \
             two into UnderReview, are one event, so there are five"
        );
    }

    #[test]
    fn ori_t_0021_the_transition_table_is_the_diagram_the_data_model_draws() {
        let drawn = drawn_edges();
        assert_eq!(
            drawn.len(),
            6,
            "the copied diagram draws {} edges between states and spec/DATA_MODEL.md section 3 \
             draws six",
            drawn.len()
        );
        assert_eq!(
            TRANSITIONS.to_vec(),
            drawn,
            "TRANSITIONS and the diagram it is read from are not the same relation, in the same \
             order"
        );

        // The whole truth table, all twenty-five ordered pairs of the five
        // states, cell by cell. A table that answers false everywhere and a
        // table that answers true everywhere each fail here, and so does one
        // that is wrong about a single cell.
        let expected: BTreeSet<(usize, usize)> = drawn
            .iter()
            .map(|(from, to)| (index_of(*from), index_of(*to)))
            .collect();
        let mut connected = BTreeSet::new();
        for from in DocumentState::ALL {
            for to in DocumentState::ALL {
                if connects(*from, *to) {
                    connected.insert((index_of(*from), index_of(*to)));
                }
            }
        }
        assert_eq!(
            connected, expected,
            "connects and the diagram disagree over the twenty-five pairs of five states"
        );
        assert_eq!(connected.len(), 6, "six of the twenty-five pairs are drawn");
    }

    /// Where `state` sits in `DocumentState::ALL`, for reporting a cell.
    fn index_of(state: DocumentState) -> usize {
        DocumentState::ALL
            .iter()
            .position(|listed| *listed == state)
            .expect("DocumentState::ALL lists every state")
    }

    #[test]
    fn ori_t_0021_a_document_starts_missing() {
        assert_eq!(INITIAL, DocumentState::Missing);
        // Nothing returns to it: the start marker is the only edge in.
        for from in DocumentState::ALL {
            assert!(
                !connects(*from, DocumentState::Missing),
                "the diagram draws no edge back to Missing, and one was found out of {from}"
            );
        }
    }

    #[test]
    fn ori_t_0021_no_event_leaves_a_document_in_the_state_it_is_in() {
        for state in DocumentState::ALL {
            assert!(
                !connects(*state, *state),
                "the diagram draws no self transition and {state} has one"
            );
            for event in every_event() {
                if event.target() != *state {
                    continue;
                }
                let refused = document(*state, DocumentSet::Foundation)
                    .apply(&event, Seat::Architect)
                    .expect_err("a move to the state the document is in is refused");
                assert!(refused.is_refusal(), "{state} and {event}: {refused}");
            }
        }
    }

    #[test]
    fn ori_t_0021_every_edge_the_diagram_draws_can_be_walked() {
        for (from, to) in drawn_edges() {
            let walked = every_event().into_iter().any(|event| {
                event.target() == to
                    && document(from, DocumentSet::Foundation)
                        .apply(&event, Seat::Architect)
                        .is_ok_and(|next| next.state == to)
            });
            assert!(
                walked,
                "the diagram draws {from} to {to} and no event walks it"
            );
        }
    }

    #[test]
    fn ori_t_0021_a_refused_move_names_the_pair_and_cites_the_section() {
        let refused = document(DocumentState::Missing, DocumentSet::Foundation)
            .apply(
                &DocumentEvent::Approved {
                    signature: architect(),
                },
                Seat::Architect,
            )
            .expect_err("a missing document cannot be approved");
        match &refused {
            Error::Refused {
                kind:
                    RefusalKind::DocumentTransition {
                        from: DocumentState::Missing,
                        to: DocumentState::Approved,
                    },
                detail,
            } => assert_eq!(detail.as_deref(), Some("the event was document.approved")),
            other => panic!("the refusal names the pair, and this one is {other:?}"),
        }
        let reason = refused
            .methodology_ref()
            .expect("a refusal carries a reason");
        assert!(reason.resolves(), "{reason} resolves in the index");
        assert_eq!(reason.to_string(), "AICD §9");
        assert!(refused.to_string().contains("missing"), "{refused}");
        assert!(refused.to_string().contains("approved"), "{refused}");
    }

    #[test]
    fn ori_t_0021_the_labelled_edges_are_narrower_than_the_pairs_in_exactly_two_cells() {
        // Every cell of the twenty-five the five states and five events make.
        // fires_from is the relation the diagram's labels draw; connects is the
        // relation its arrows draw. The second admits two moves the first does
        // not, because two labelled edges end at Draft, and this pins which two
        // so the gap cannot widen without a failure.
        let mut wider = Vec::new();
        for state in DocumentState::ALL {
            for event in every_event() {
                let by_label = event.fires_from(*state);
                let by_pair = connects(*state, event.target());
                assert!(
                    !by_label || by_pair,
                    "{event} is drawn leaving {state} and the pair table refuses it, so the two \
                     readings of the diagram disagree about an edge it draws"
                );
                if by_pair && !by_label {
                    wider.push(format!("{event} out of {state}"));
                }
            }
        }
        assert_eq!(
            wider,
            vec![
                "document.changes_requested out of missing".to_owned(),
                "document.created out of under_review".to_owned(),
            ],
            "the two cells the pair table admits and the labels do not are named in the head of \
             this module, with the refusal variant that would close them"
        );
    }

    #[test]
    fn ori_t_0021_the_event_names_are_the_domain_verb_past_form() {
        let names: Vec<&str> = every_event().iter().map(DocumentEvent::as_str).collect();
        assert_eq!(
            names,
            vec![
                "document.created",
                "document.submitted",
                "document.changes_requested",
                "document.approved",
                "document.drifted",
            ]
        );
        for name in names {
            let (domain, verb) = name
                .split_once('.')
                .expect("spec/LLD.md section 4 writes an event as domain.verb_past");
            assert_eq!(domain, "document");
            assert!(!verb.is_empty());
        }
    }

    #[test]
    fn ori_t_0021_the_drift_audit_records_when_it_looked() {
        let at = Timestamp::from_millis(1_764_000_000_123);
        let approved = document(DocumentState::Approved, DocumentSet::Foundation);
        let stale = approved
            .apply(&DocumentEvent::Drifted { at }, Seat::Architect)
            .expect("the drift audit may find a divergence in an approved document");
        assert_eq!(stale.state, DocumentState::Stale);
        assert_eq!(stale.verified_against_code_at, Some(at));
        // The document it was read from is untouched, which is what lets a
        // projection carry on from the state it held.
        assert_eq!(approved.state, DocumentState::Approved);
        assert_eq!(approved.verified_against_code_at, None);
    }

    #[test]
    fn ori_t_0021_a_foundation_document_that_is_not_approved_disables_launch() {
        for state in DocumentState::ALL {
            for set in DocumentSet::ALL {
                let expected = *set == DocumentSet::Foundation && *state != DocumentState::Approved;
                assert_eq!(
                    disables_launch(&document(*state, *set)),
                    expected,
                    "a {set} document in state {state}"
                );
            }
        }
    }

    #[test]
    fn ori_t_0021_a_signature_that_could_not_have_been_earned_is_visible() {
        let signature = architect();
        for state in DocumentState::ALL {
            let mut unsigned = document(*state, DocumentSet::Foundation);
            let mut signed = unsigned.clone();
            signed.approved_by = Some(signature.seat());
            signed.approved_at = Some(signature.at());
            let mut half = unsigned.clone();
            half.approved_by = Some(signature.seat());

            let reachable_signed = matches!(
                state,
                DocumentState::Draft
                    | DocumentState::UnderReview
                    | DocumentState::Approved
                    | DocumentState::Stale
            );
            let reachable_unsigned = matches!(
                state,
                DocumentState::Missing | DocumentState::Draft | DocumentState::UnderReview
            );
            assert_eq!(
                signature_agrees_with_state(&signed),
                reachable_signed,
                "a signed document in state {state}"
            );
            assert_eq!(
                signature_agrees_with_state(&unsigned),
                reachable_unsigned,
                "an unsigned document in state {state}"
            );
            assert!(
                !signature_agrees_with_state(&half),
                "a document in state {state} carrying a seat and no time is half signed"
            );

            // And what the machine produces always agrees.
            unsigned.state = DocumentState::UnderReview;
            let approved = unsigned
                .apply(
                    &DocumentEvent::Approved {
                        signature: signature.clone(),
                    },
                    Seat::Architect,
                )
                .expect("the owning seat may sign a document under review");
            assert!(signature_agrees_with_state(&approved));
        }
    }

    #[test]
    fn ori_p1_040_one_call_moves_a_document_under_review_to_approved() {
        // "Any document in UnderReview | Owning seat calls flows.documents.approve
        // | Single call; state Approved".
        let under_review = document(DocumentState::UnderReview, DocumentSet::Foundation);
        let signature = architect();
        let approved = under_review
            .apply(
                &DocumentEvent::Approved {
                    signature: signature.clone(),
                },
                Seat::Architect,
            )
            .expect("the owning seat approves a document under review");
        assert_eq!(approved.state, DocumentState::Approved);
        assert_eq!(approved.approved_by, Some(Seat::Architect));
        assert_eq!(approved.approved_at, Some(signature.at()));
    }

    #[test]
    fn ori_p1_040_no_step_but_the_approval_is_required() {
        // "no other step required": one application of one event leaves a
        // document that is approved, internally consistent, and no longer
        // holding Launch back. What this crate cannot answer for is the rest of
        // the sentence, which is one call of an RPC in ori-flows and one event
        // appended by ori-store.
        let approved = document(DocumentState::UnderReview, DocumentSet::Foundation)
            .apply(
                &DocumentEvent::Approved {
                    signature: architect(),
                },
                Seat::Architect,
            )
            .expect("the owning seat approves a document under review");
        assert!(signature_agrees_with_state(&approved));
        assert!(!disables_launch(&approved));
    }

    #[test]
    fn ori_p1_040_a_seat_that_does_not_own_the_document_cannot_sign_it() {
        // The first invariant under spec/DATA_MODEL.md section 3's Document
        // diagram, over every seat that is not the owner.
        let under_review = document(DocumentState::UnderReview, DocumentSet::Foundation);
        let owner = Seat::Architect;
        let mut refused_for = Vec::new();
        for seat in Seat::ALL {
            let signature = Signature::of(
                *seat,
                &Actor::Human(Id::parse(HOLDER).expect("a well formed ULID")),
                Timestamp::from_millis(1_764_000_000_000),
            )
            .expect("a human can sign");
            let outcome = under_review.apply(&DocumentEvent::Approved { signature }, owner);
            if *seat == owner {
                assert!(outcome.is_ok(), "the owning seat signs");
                continue;
            }
            let refused = outcome.expect_err("only the owning seat may sign");
            match &refused {
                Error::Refused {
                    kind:
                        RefusalKind::DocumentSeatMismatch {
                            owner: named,
                            signer,
                        },
                    ..
                } => {
                    assert_eq!(*named, owner);
                    assert_eq!(signer, seat);
                }
                other => panic!("the refusal names the two seats, and this one is {other:?}"),
            }
            let reason = refused
                .methodology_ref()
                .expect("a refusal carries a reason");
            assert_eq!(reason.to_string(), "AICD §9");
            assert!(reason.resolves());
            refused_for.push(*seat);
        }
        assert_eq!(
            refused_for.len(),
            Seat::ALL.len() - 1,
            "every seat but the owner is refused, and AICD §18 names four"
        );
    }

    #[test]
    fn ori_p1_040_only_a_human_actor_can_sign_an_approval() {
        // "one document.approved event with seat and human actor". An agent
        // identity and the system hold no seat, so neither can produce the
        // signature an approval is built from.
        let id = Id::parse(HOLDER).expect("a well formed ULID");
        let at = Timestamp::from_millis(1_764_000_000_000);
        assert!(
            Signature::of(Seat::Architect, &Actor::Human(id.clone()), at).is_some(),
            "a human holds a seat"
        );
        assert!(
            Signature::of(Seat::Architect, &Actor::Agent(id.clone()), at).is_none(),
            "an agent identity holds no seat and signs nothing"
        );
        assert!(
            Signature::of(Seat::Architect, &Actor::System, at).is_none(),
            "the system holds no seat and signs nothing"
        );
        let signature = Signature::of(Seat::Architect, &Actor::Human(id.clone()), at)
            .expect("a human can sign");
        assert_eq!(signature.seat(), Seat::Architect);
        assert_eq!(signature.holder(), &id);
        assert_eq!(signature.at(), at);
    }

    #[test]
    fn ori_p1_040_the_event_that_approves_is_named_document_approved() {
        assert_eq!(
            DocumentEvent::Approved {
                signature: architect(),
            }
            .as_str(),
            "document.approved",
            "criterion ORI-P1-040 names the event, and spec/API_SPEC.md lists document.* among \
             the kinds a client subscribes to"
        );
    }

    #[test]
    fn ori_t_0021_a_document_that_is_not_under_review_cannot_be_approved() {
        for state in DocumentState::ALL {
            if *state == DocumentState::UnderReview {
                continue;
            }
            let refused = document(*state, DocumentSet::Foundation)
                .apply(
                    &DocumentEvent::Approved {
                        signature: architect(),
                    },
                    Seat::Architect,
                )
                .expect_err("the only edge into Approved leaves UnderReview");
            assert!(refused.is_refusal(), "a document in state {state}");
        }
    }
}
