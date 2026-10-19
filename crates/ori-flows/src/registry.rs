//! Document registry: fold of `product.created` and `document.*` events: AICD §9, §23.
//!
//! Owner seats are derived from document kind ([`owner_for_kind`]); they never
//! appear in event payloads (`ops/batch-12-plan.md`, D12).

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path};

use ori_core::document::{DocumentEvent, Signature};
use ori_core::error::MethodologyRef;
use ori_core::types::{
    Actor, Document, DocumentKind, DocumentSet, DocumentState, Id, ProductOrigin, Seat, Timestamp,
};
use ori_store::db::ProductDb;
use ori_store::event_log::{Event, EventHash, EventLog, EventLogError};
use serde::{Deserialize, Serialize};

/// Kind of the event that creates a product: `spec/API_SPEC.md` section 2 (AICD §23).
pub const PRODUCT_CREATED_KIND: &str = "product.created";
/// Kind that registers a document not listed in `product.created`: AICD §9.
pub const DOCUMENT_REGISTERED_KIND: &str = "document.registered";
/// Kind when a document is first written: AICD §9.
pub const DOCUMENT_CREATED_KIND: &str = "document.created";
/// Kind when a document is revised: AICD §9.
pub const DOCUMENT_REVISED_KIND: &str = "document.revised";
/// Kind when review asks for changes: AICD §9.
pub const DOCUMENT_CHANGES_REQUESTED_KIND: &str = "document.changes_requested";
/// Kind when the owning seat signs: AICD §9.
pub const DOCUMENT_APPROVED_KIND: &str = "document.approved";
/// Reserved; the fold refuses until batch 12 Q7 fixes the payload.
pub const DOCUMENT_SUBMITTED_KIND: &str = "document.submitted";
/// Reserved; the fold refuses until phase F fixes the payload.
pub const DOCUMENT_DRIFTED_KIND: &str = "document.drifted";

const PAYLOAD_SECTION: u8 = 8;
const AUDIT_SECTION: u8 = 13;
const TEAM_SECTION: u8 = 18;
const FOUNDATION_SECTION: u8 = 23;
const SECURITY_SECTION: u8 = 27;

const INSTRUCTIONS_MAX: usize = 8192;

const fn whole_section(section: u8) -> MethodologyRef {
    MethodologyRef {
        section,
        subsection: None,
    }
}

/// One foundation document row from `ops/batch-12-plan.md` (D12).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundationRow {
    /// Document kind wire spelling.
    pub kind: &'static str,
    /// Repository-relative path under `spec/`.
    pub path: &'static str,
}

/// The eleven foundation documents for `origin: "new"`: AICD §23.
pub const FOUNDATION: [FoundationRow; 11] = [
    FoundationRow {
        kind: "brief",
        path: "spec/PROJECT_BRIEF.md",
    },
    FoundationRow {
        kind: "architecture",
        path: "spec/ARCHITECTURE.md",
    },
    FoundationRow {
        kind: "adr",
        path: "spec/adr/ADR-0001-stack.md",
    },
    FoundationRow {
        kind: "data_model",
        path: "spec/DATA_MODEL.md",
    },
    FoundationRow {
        kind: "security_notes",
        path: "spec/SECURITY_NOTES.md",
    },
    FoundationRow {
        kind: "env_setup",
        path: "spec/ENV_SETUP.md",
    },
    FoundationRow {
        kind: "conventions",
        path: "spec/CONVENTIONS.md",
    },
    FoundationRow {
        kind: "observability",
        path: "spec/OBSERVABILITY.md",
    },
    FoundationRow {
        kind: "ci_cd",
        path: "spec/CI_CD.md",
    },
    FoundationRow {
        kind: "roadmap",
        path: "spec/ROADMAP.md",
    },
    FoundationRow {
        kind: "agent_instructions",
        path: "spec/agents/CLAUDE.md",
    },
];

/// Derives the owning seat for a document kind (lead reading, D12).
///
/// Named owners from `spec/README.md` where a single seat is listed; otherwise
/// the first AICD §23 verifier ([`Seat::VerificationLead`]). As-built kinds
/// (`as_built_*`) map to [`Seat::Architect`] (register row 10).
#[must_use]
pub fn owner_for_kind(kind: &DocumentKind) -> Seat {
    match kind {
        DocumentKind::Brief | DocumentKind::Roadmap => Seat::ProductOwner,
        DocumentKind::EnvSetup | DocumentKind::Observability | DocumentKind::CiCd => {
            Seat::ReliabilityGovernance
        }
        DocumentKind::AsBuilt(_) => Seat::Architect,
        DocumentKind::Architecture
        | DocumentKind::Adr
        | DocumentKind::DataModel
        | DocumentKind::SecurityNotes
        | DocumentKind::Conventions
        | DocumentKind::AgentInstructions => Seat::Architect,
        _ => Seat::VerificationLead,
    }
}

/// Why the fold refused an event: AICD §8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventDefect {
    /// Payload could not be read or broke a field rule.
    Payload,
    /// Actor was not a human where required.
    NotHuman,
    /// Remote URL or PR carried password userinfo.
    SecretInUrl,
    /// `repo_path` was not a canonical absolute path.
    RepoPath,
    /// Seat list invalid.
    Seats,
    /// Foundation document list invalid for `origin: "new"`.
    Foundation,
    /// Kind not handled in R1.
    KindReserved,
    /// Registration broke path, id, kind or set rules.
    Registration,
    /// Approver did not hold the signing seat.
    SeatHolder,
    /// Migrated product without seats cannot approve migration-set docs.
    MigratedSeats,
    /// State machine refused the transition.
    Transition,
    /// Document id unknown.
    UnknownDocument,
}

impl EventDefect {
    /// Methodology section for control refusals surfaced to callers.
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        match self {
            Self::Payload => whole_section(PAYLOAD_SECTION),
            Self::NotHuman | Self::SeatHolder | Self::MigratedSeats => whole_section(TEAM_SECTION),
            Self::SecretInUrl => whole_section(SECURITY_SECTION),
            Self::RepoPath | Self::Seats | Self::Foundation | Self::Registration => {
                whole_section(FOUNDATION_SECTION)
            }
            Self::KindReserved | Self::Transition | Self::UnknownDocument => {
                whole_section(AUDIT_SECTION)
            }
        }
    }
}

impl fmt::Display for EventDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Payload => "payload",
            Self::NotHuman => "not_human",
            Self::SecretInUrl => "secret_in_url",
            Self::RepoPath => "repo_path",
            Self::Seats => "seats",
            Self::Foundation => "foundation",
            Self::KindReserved => "kind_reserved",
            Self::Registration => "registration",
            Self::SeatHolder => "seat_holder",
            Self::MigratedSeats => "migrated_seats",
            Self::Transition => "transition",
            Self::UnknownDocument => "unknown_document",
        })
    }
}

/// Refusal from the document registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// Product id on the database is not a ULID.
    ProductNotAnId,
    /// Database belongs to another product.
    WrongProduct,
    /// Log read or verify failed.
    Log(EventLogError),
    /// Log moved beneath the folded tip.
    LogMoved { folded_seq: u64 },
    /// Fold refused an event; registry is blocked at `seq`.
    Event { seq: u64, defect: EventDefect },
    /// Payload could not be encoded.
    Encode,
    /// Pre-append validation failed (same defect semantics as fold).
    Refused { defect: EventDefect },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProductNotAnId => f.write_str("product id is not a ULID"),
            Self::WrongProduct => f.write_str("wrong product database"),
            Self::Log(err) => write!(f, "{err}"),
            Self::LogMoved { folded_seq } => {
                write!(f, "log moved below folded seq {folded_seq}")
            }
            Self::Event { seq, defect } => {
                write!(f, "refused event at seq {seq}: {defect}")
            }
            Self::Encode => f.write_str("payload encode failed"),
            Self::Refused { defect } => write!(f, "refused: {defect}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Log(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatWire {
    /// Seat wire spelling.
    pub seat: String,
    /// Human holder ULID.
    pub holder: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentWire {
    /// Document ULID.
    pub id: String,
    /// Document kind.
    pub kind: String,
    /// Repository-relative path.
    pub path: String,
    /// Document set name.
    pub set: String,
}

/// Wire shape of `product.created`: AICD §23.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductCreatedWire {
    /// Product display name.
    pub name: String,
    /// Canonical absolute repository path on this machine.
    pub repo_path: String,
    /// Optional remote URL without password userinfo.
    #[serde(default)]
    pub repo_remote: Option<String>,
    /// Bundled methodology version string.
    pub aicd_version: String,
    /// `new` or `migrated`.
    pub origin: String,
    /// Seat holders (empty for migrated phase 1).
    pub seats: Vec<SeatWire>,
    /// Foundation documents (`new` origin only).
    pub documents: Vec<DocumentWire>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentRegisteredWire {
    document_id: String,
    kind: String,
    path: String,
    set: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentPrWire {
    document_id: String,
    pr: String,
    content_len: u64,
    citations_unresolved: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentChangesWire {
    document_id: String,
    instructions: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentApprovedWire {
    document_id: String,
    seat: String,
}

fn read_object_payload<T: for<'de> Deserialize<'de>>(payload: &str) -> Result<T, EventDefect> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| EventDefect::Payload)?;
    if !value.is_object() {
        return Err(EventDefect::Payload);
    }
    serde_json::from_str(payload).map_err(|_| EventDefect::Payload)
}

fn is_unsafe_char(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn url_has_password_userinfo(url: &str) -> bool {
    if url.contains('@')
        && !url.starts_with("git@")
        && let Some(rest) = url.split("://").nth(1)
        && let Some(at) = rest.find('@')
    {
        let userinfo = &rest[..at];
        return userinfo.contains(':');
    }
    false
}

fn validate_pr(pr: &str) -> Result<(), EventDefect> {
    if pr.chars().all(|ch| ch.is_ascii_digit()) {
        return Ok(());
    }
    if url_has_password_userinfo(pr) {
        return Err(EventDefect::SecretInUrl);
    }
    if pr.contains("://") || pr.starts_with("git@") {
        return Ok(());
    }
    Err(EventDefect::Payload)
}

fn validate_instructions(text: &str) -> Result<(), EventDefect> {
    if text.trim().is_empty() {
        return Err(EventDefect::Payload);
    }
    if text.len() > INSTRUCTIONS_MAX {
        return Err(EventDefect::Payload);
    }
    if text.chars().any(is_unsafe_char) {
        return Err(EventDefect::Payload);
    }
    Ok(())
}

fn canonical_absolute_repo_path(path: &str) -> Result<String, EventDefect> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(EventDefect::RepoPath);
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| EventDefect::RepoPath)?;
    canonical
        .into_os_string()
        .into_string()
        .map_err(|_| EventDefect::RepoPath)
}

fn parse_seats(
    wires: &[SeatWire],
    origin: ProductOrigin,
) -> Result<HashMap<Seat, Id>, EventDefect> {
    if origin == ProductOrigin::Migrated && wires.is_empty() {
        return Ok(HashMap::new());
    }
    if wires.len() != Seat::ALL.len() {
        return Err(EventDefect::Seats);
    }
    let mut map = HashMap::new();
    for wire in wires {
        let seat: Seat = wire.seat.parse().map_err(|_| EventDefect::Seats)?;
        let holder = Id::parse(&wire.holder).map_err(|_| EventDefect::Seats)?;
        if map.insert(seat, holder).is_some() {
            return Err(EventDefect::Seats);
        }
    }
    if map.len() != Seat::ALL.len() {
        return Err(EventDefect::Seats);
    }
    Ok(map)
}

fn foundation_matches(documents: &[DocumentWire]) -> bool {
    if documents.len() != FOUNDATION.len() {
        return false;
    }
    for (row, doc) in FOUNDATION.iter().zip(documents.iter()) {
        if doc.kind != row.kind || doc.path != row.path || doc.set != "foundation" {
            return false;
        }
        if Id::parse(&doc.id).is_err() {
            return false;
        }
    }
    true
}

fn path_under_spec(path: &str) -> bool {
    let path = Path::new(path);
    path.components().next() == Some(Component::Normal("spec".as_ref()))
}

fn parse_set(text: &str) -> Result<DocumentSet, EventDefect> {
    text.parse().map_err(|_| EventDefect::Registration)
}

fn parse_kind(text: &str) -> Result<DocumentKind, EventDefect> {
    text.parse().map_err(|_| EventDefect::Registration)
}

#[derive(Clone, Debug)]
struct ProductSnapshot {
    origin: ProductOrigin,
    seats: HashMap<Seat, Id>,
}

#[derive(Clone, Debug)]
struct State {
    last_seq: u64,
    last_digest: Option<EventHash>,
    product: Option<ProductSnapshot>,
    documents: BTreeMap<String, Document>,
    paths: BTreeSet<String>,
}

impl State {
    fn empty() -> Self {
        Self {
            last_seq: 0,
            last_digest: None,
            product: None,
            documents: BTreeMap::new(),
            paths: BTreeSet::new(),
        }
    }
}

fn document_from_wire(
    product_id: &Id,
    wire: &DocumentWire,
    state: DocumentState,
) -> Result<Document, EventDefect> {
    Ok(Document {
        id: Id::parse(&wire.id).map_err(|_| EventDefect::Payload)?,
        product_id: product_id.clone(),
        path: wire.path.clone(),
        kind: parse_kind(&wire.kind)?,
        set: parse_set(&wire.set)?,
        state,
        approved_by: None,
        approved_at: None,
        verified_against_code_at: None,
    })
}

fn apply_product_created(
    state: &mut State,
    product_id: &Id,
    event: &Event,
) -> Result<(), EventDefect> {
    let Actor::Human(_) = event.actor() else {
        return Err(EventDefect::NotHuman);
    };
    let wire: ProductCreatedWire = read_object_payload(event.payload())?;
    if wire
        .repo_remote
        .as_deref()
        .is_some_and(url_has_password_userinfo)
    {
        return Err(EventDefect::SecretInUrl);
    }
    let repo_path = canonical_absolute_repo_path(&wire.repo_path)?;
    let origin: ProductOrigin = wire.origin.parse().map_err(|_| EventDefect::Payload)?;
    let seats = parse_seats(&wire.seats, origin)?;
    match origin {
        ProductOrigin::New if !foundation_matches(&wire.documents) => {
            return Err(EventDefect::Foundation);
        }
        ProductOrigin::Migrated if !wire.documents.is_empty() => {
            return Err(EventDefect::Foundation);
        }
        _ => {}
    }
    state.product = Some(ProductSnapshot { origin, seats });
    state.documents.clear();
    state.paths.clear();
    for doc in &wire.documents {
        let document = document_from_wire(product_id, doc, DocumentState::Missing)?;
        if !state.paths.insert(document.path.clone()) {
            return Err(EventDefect::Registration);
        }
        state
            .documents
            .insert(document.id.as_str().to_owned(), document);
    }
    let _ = repo_path;
    state.last_seq = event.seq();
    state.last_digest = Some(event.digest());
    Ok(())
}

fn holder_for_seat(state: &State, seat: Seat) -> Option<&Id> {
    state.product.as_ref()?.seats.get(&seat)
}

fn check_migrated_approval(state: &State, doc: &Document, seat: Seat) -> Result<(), EventDefect> {
    let product = state.product.as_ref().ok_or(EventDefect::UnknownDocument)?;
    if product.origin == ProductOrigin::Migrated
        && product.seats.is_empty()
        && doc.set == DocumentSet::Migration
    {
        let _ = seat;
        return Err(EventDefect::MigratedSeats);
    }
    Ok(())
}

fn apply_document_registered(
    state: &mut State,
    product_id: &Id,
    event: &Event,
) -> Result<(), EventDefect> {
    let wire: DocumentRegisteredWire = read_object_payload(event.payload())?;
    if !path_under_spec(&wire.path) {
        return Err(EventDefect::Registration);
    }
    let set = parse_set(&wire.set)?;
    let kind = parse_kind(&wire.kind)?;
    let id = Id::parse(&wire.document_id).map_err(|_| EventDefect::Payload)?;
    if state.documents.contains_key(id.as_str()) || !state.paths.insert(wire.path.clone()) {
        return Err(EventDefect::Registration);
    }
    let document = Document {
        id,
        product_id: product_id.clone(),
        path: wire.path,
        kind,
        set,
        state: DocumentState::Missing,
        approved_by: None,
        approved_at: None,
        verified_against_code_at: None,
    };
    state
        .documents
        .insert(document.id.as_str().to_owned(), document);
    state.last_seq = event.seq();
    state.last_digest = Some(event.digest());
    Ok(())
}

fn apply_with_document_event(
    state: &mut State,
    event: &Event,
    build: impl FnOnce(&Document, &Event) -> Result<DocumentEvent, EventDefect>,
) -> Result<(), EventDefect> {
    let document_id = extract_document_id(event)?;
    let Some(doc) = state.documents.get(&document_id).cloned() else {
        return Err(EventDefect::UnknownDocument);
    };
    let product = state.product.as_ref().ok_or(EventDefect::UnknownDocument)?;
    let doc_event = build(&doc, event)?;
    if !doc_event.fires_from(doc.state) {
        return Err(EventDefect::Transition);
    }
    let owner = owner_for_kind(&doc.kind);
    let next = doc
        .apply(&doc_event, owner)
        .map_err(|_| EventDefect::Transition)?;
    let _ = product;
    state.documents.insert(document_id, next);
    state.last_seq = event.seq();
    state.last_digest = Some(event.digest());
    Ok(())
}

fn extract_document_id(event: &Event) -> Result<String, EventDefect> {
    let value: serde_json::Value =
        serde_json::from_str(event.payload()).map_err(|_| EventDefect::Payload)?;
    let id = value
        .get("document_id")
        .and_then(serde_json::Value::as_str)
        .ok_or(EventDefect::Payload)?;
    Ok(id.to_owned())
}

fn apply_document_created(state: &mut State, event: &Event) -> Result<(), EventDefect> {
    let wire: DocumentPrWire = read_object_payload(event.payload())?;
    validate_pr(&wire.pr)?;
    apply_with_document_event(state, event, |_, _| Ok(DocumentEvent::Created))
}

fn apply_document_revised(state: &mut State, event: &Event) -> Result<(), EventDefect> {
    let wire: DocumentPrWire = read_object_payload(event.payload())?;
    validate_pr(&wire.pr)?;
    apply_with_document_event(state, event, |_, _| Ok(DocumentEvent::ChangesRequested))
}

fn apply_document_changes_requested(state: &mut State, event: &Event) -> Result<(), EventDefect> {
    let Actor::Human(_) = event.actor() else {
        return Err(EventDefect::NotHuman);
    };
    let wire: DocumentChangesWire = read_object_payload(event.payload())?;
    validate_instructions(&wire.instructions)?;
    apply_with_document_event(state, event, |_, _| Ok(DocumentEvent::ChangesRequested))
}

fn apply_document_approved(state: &mut State, event: &Event) -> Result<(), EventDefect> {
    let Actor::Human(holder) = event.actor() else {
        return Err(EventDefect::NotHuman);
    };
    let wire: DocumentApprovedWire = read_object_payload(event.payload())?;
    let seat: Seat = wire.seat.parse().map_err(|_| EventDefect::Payload)?;
    let document_id = wire.document_id;
    let Some(doc) = state.documents.get(&document_id).cloned() else {
        return Err(EventDefect::UnknownDocument);
    };
    check_migrated_approval(state, &doc, seat)?;
    let Some(expected) = holder_for_seat(state, seat) else {
        return Err(EventDefect::SeatHolder);
    };
    if expected != holder {
        return Err(EventDefect::SeatHolder);
    }
    let signature = Signature::of(seat, event.actor(), event.at()).ok_or(EventDefect::NotHuman)?;
    let owner = owner_for_kind(&doc.kind);
    if seat != owner {
        return Err(EventDefect::Transition);
    }
    let approved_event = DocumentEvent::Approved {
        signature: signature.clone(),
    };
    let mut working = doc.clone();
    if working.state == DocumentState::Draft {
        // Interim until `document.submitted` exists (batch 12 Q7): approval from
        // Draft applies the submit edge in memory only.
        working = working
            .apply(&DocumentEvent::Submitted, owner)
            .map_err(|_| EventDefect::Transition)?;
    }
    if !approved_event.fires_from(working.state) {
        return Err(EventDefect::Transition);
    }
    let next = working
        .apply(&approved_event, owner)
        .map_err(|_| EventDefect::Transition)?;
    state.documents.insert(document_id, next);
    state.last_seq = event.seq();
    state.last_digest = Some(event.digest());
    Ok(())
}

fn advance_tip(state: &mut State, event: &Event) {
    state.last_seq = event.seq();
    state.last_digest = Some(event.digest());
}

fn fold_event(state: &mut State, product_id: &Id, event: &Event) -> Result<(), EventDefect> {
    if event.product_id() != product_id {
        return Err(EventDefect::Payload);
    }
    match event.kind() {
        PRODUCT_CREATED_KIND => apply_product_created(state, product_id, event),
        DOCUMENT_REGISTERED_KIND => apply_document_registered(state, product_id, event),
        DOCUMENT_CREATED_KIND => apply_document_created(state, event),
        DOCUMENT_REVISED_KIND => apply_document_revised(state, event),
        DOCUMENT_CHANGES_REQUESTED_KIND => apply_document_changes_requested(state, event),
        DOCUMENT_APPROVED_KIND => apply_document_approved(state, event),
        DOCUMENT_SUBMITTED_KIND | DOCUMENT_DRIFTED_KIND => Err(EventDefect::KindReserved),
        _ => {
            advance_tip(state, event);
            Ok(())
        }
    }
}

fn chains_to_tip(
    first: &Event,
    last: &Event,
    last_digest: Option<EventHash>,
    tip_digest: Option<EventHash>,
) -> bool {
    first.hash_prev() == last_digest.unwrap_or(EventHash::GENESIS)
        && Some(last.digest()) == tip_digest
}

/// Folded document registry for one product: AICD §9, §23.
#[derive(Debug)]
pub struct DocumentRegistry {
    product_id: Id,
    state: State,
    blocked: Option<(u64, EventDefect)>,
}

impl DocumentRegistry {
    /// Reads and verifies the log, folding every event: AICD §8.
    ///
    /// # Errors
    ///
    /// [`RegistryError::Event`] when the fold refuses an event (lockout).
    pub fn read(db: &mut ProductDb) -> Result<Self, RegistryError> {
        let product_id = Id::parse(db.product_id()).map_err(|_| RegistryError::ProductNotAnId)?;
        let mut registry = Self {
            product_id,
            state: State::empty(),
            blocked: None,
        };
        registry.sync(db)?;
        Ok(registry)
    }

    fn sync(&mut self, db: &mut ProductDb) -> Result<(), RegistryError> {
        if let Some((seq, defect)) = &self.blocked {
            return Err(RegistryError::Event {
                seq: *seq,
                defect: defect.clone(),
            });
        }
        match Id::parse(db.product_id()) {
            Ok(id) if id == self.product_id => {}
            _ => return Err(RegistryError::WrongProduct),
        }
        let report = EventLog::verify(db.connection()).map_err(RegistryError::Log)?;
        let tip = report.tip_seq.unwrap_or(0);
        let last_seq = self.state.last_seq;
        if tip < last_seq || (tip == last_seq && report.tip_digest != self.state.last_digest) {
            return Err(RegistryError::LogMoved {
                folded_seq: last_seq,
            });
        }
        if tip == last_seq {
            return Ok(());
        }
        let wanted = tip - last_seq;
        let events =
            EventLog::read_range(db.connection(), last_seq + 1, tip).map_err(RegistryError::Log)?;
        if u64::try_from(events.len()).unwrap_or(u64::MAX) != wanted {
            return Err(RegistryError::Log(EventLogError::ReaderIncomplete {
                counted: wanted,
                read: u64::try_from(events.len()).unwrap_or(0),
            }));
        }
        let (Some(first), Some(last)) = (events.first(), events.last()) else {
            return Err(RegistryError::Log(EventLogError::ReaderIncomplete {
                counted: wanted,
                read: 0,
            }));
        };
        if !chains_to_tip(first, last, self.state.last_digest, report.tip_digest) {
            return Err(RegistryError::LogMoved {
                folded_seq: last_seq,
            });
        }
        let mut next = self.state.clone();
        for event in &events {
            if let Err(defect) = fold_event(&mut next, &self.product_id, event) {
                self.blocked = Some((event.seq(), defect.clone()));
                return Err(RegistryError::Event {
                    seq: event.seq(),
                    defect,
                });
            }
        }
        self.state = next;
        Ok(())
    }

    /// Whether the registry is blocked after a refused fold event.
    #[must_use]
    pub fn blocked(&self) -> Option<(u64, &EventDefect)> {
        self.blocked.as_ref().map(|(seq, d)| (*seq, d))
    }

    /// Documents in the last good folded state.
    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.state.documents.values()
    }

    /// Appends `product.created` after validation: AICD §23.
    pub fn create_product(
        &mut self,
        db: &mut ProductDb,
        at: Timestamp,
        actor: Actor,
        wire: ProductCreatedWire,
    ) -> Result<u64, RegistryError> {
        self.sync(db)?;
        let Actor::Human(_) = actor else {
            return Err(RegistryError::Refused {
                defect: EventDefect::NotHuman,
            });
        };
        if wire
            .repo_remote
            .as_deref()
            .is_some_and(url_has_password_userinfo)
        {
            return Err(RegistryError::Refused {
                defect: EventDefect::SecretInUrl,
            });
        }
        let repo_path = canonical_absolute_repo_path(&wire.repo_path)
            .map_err(|defect| RegistryError::Refused { defect })?;
        let mut wire = wire;
        wire.repo_path = repo_path;
        let origin: ProductOrigin = wire.origin.parse().map_err(|_| RegistryError::Refused {
            defect: EventDefect::Payload,
        })?;
        parse_seats(&wire.seats, origin).map_err(|defect| RegistryError::Refused { defect })?;
        match origin {
            ProductOrigin::New if !foundation_matches(&wire.documents) => {
                return Err(RegistryError::Refused {
                    defect: EventDefect::Foundation,
                });
            }
            ProductOrigin::Migrated if !wire.documents.is_empty() => {
                return Err(RegistryError::Refused {
                    defect: EventDefect::Foundation,
                });
            }
            _ => {}
        }
        let payload = serde_json::to_string(&wire).map_err(|_| RegistryError::Encode)?;
        let event = EventLog::append(
            db.connection(),
            self.product_id.clone(),
            at,
            actor,
            PRODUCT_CREATED_KIND,
            None,
            payload,
        )
        .map_err(RegistryError::Log)?;
        match apply_product_created(&mut self.state, &self.product_id, &event) {
            Ok(()) => Ok(event.seq()),
            Err(defect) => {
                self.blocked = Some((event.seq(), defect.clone()));
                Err(RegistryError::Event {
                    seq: event.seq(),
                    defect,
                })
            }
        }
    }

    /// Appends one `document.*` event after validation: AICD §9.
    pub fn record(
        &mut self,
        db: &mut ProductDb,
        at: Timestamp,
        actor: Actor,
        kind: &str,
        payload: &str,
    ) -> Result<u64, RegistryError> {
        self.sync(db)?;
        if kind == DOCUMENT_APPROVED_KIND || kind == DOCUMENT_CHANGES_REQUESTED_KIND {
            let Actor::Human(_) = actor else {
                return Err(RegistryError::Refused {
                    defect: EventDefect::NotHuman,
                });
            };
        }
        if kind == DOCUMENT_CHANGES_REQUESTED_KIND {
            let wire: DocumentChangesWire =
                read_object_payload(payload).map_err(|defect| RegistryError::Refused { defect })?;
            validate_instructions(&wire.instructions)
                .map_err(|defect| RegistryError::Refused { defect })?;
        }
        if kind == DOCUMENT_CREATED_KIND || kind == DOCUMENT_REVISED_KIND {
            let wire: DocumentPrWire =
                read_object_payload(payload).map_err(|defect| RegistryError::Refused { defect })?;
            validate_pr(&wire.pr).map_err(|defect| RegistryError::Refused { defect })?;
        }
        if kind == DOCUMENT_APPROVED_KIND {
            let wire: DocumentApprovedWire =
                read_object_payload(payload).map_err(|defect| RegistryError::Refused { defect })?;
            let seat: Seat = wire.seat.parse().map_err(|_| RegistryError::Refused {
                defect: EventDefect::Payload,
            })?;
            let Actor::Human(holder) = &actor else {
                return Err(RegistryError::Refused {
                    defect: EventDefect::NotHuman,
                });
            };
            let Some(doc) = self.state.documents.get(&wire.document_id) else {
                return Err(RegistryError::Refused {
                    defect: EventDefect::UnknownDocument,
                });
            };
            check_migrated_approval(&self.state, doc, seat)
                .map_err(|defect| RegistryError::Refused { defect })?;
            let Some(expected) = holder_for_seat(&self.state, seat) else {
                return Err(RegistryError::Refused {
                    defect: EventDefect::SeatHolder,
                });
            };
            if expected != holder {
                return Err(RegistryError::Refused {
                    defect: EventDefect::SeatHolder,
                });
            }
        }
        read_object_payload::<serde_json::Value>(payload)
            .map_err(|defect| RegistryError::Refused { defect })?;
        let event = EventLog::append(
            db.connection(),
            self.product_id.clone(),
            at,
            actor,
            kind,
            None,
            payload.to_owned(),
        )
        .map_err(RegistryError::Log)?;
        if let Err(defect) = fold_event(&mut self.state, &self.product_id, &event) {
            self.blocked = Some((event.seq(), defect.clone()));
            return Err(RegistryError::Event {
                seq: event.seq(),
                defect,
            });
        }
        Ok(event.seq())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_payload_pr_rejects_userinfo() {
        assert_eq!(
            validate_pr("https://user:pass@example.com/pr/1"),
            Err(EventDefect::SecretInUrl)
        );
        assert!(validate_pr("42").is_ok());
        assert!(validate_pr("https://example.com/pr/1").is_ok());
    }

    #[test]
    fn unit_instructions_cap_and_control() {
        assert!(validate_instructions("  ").is_err());
        assert!(validate_instructions(&"x".repeat(INSTRUCTIONS_MAX + 1)).is_err());
        assert!(validate_instructions("hello\u{0001}").is_err());
        assert!(validate_instructions("please fix").is_ok());
    }
}
