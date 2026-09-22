//! The domain types: `Product`, `Ticket`, `Document` and the value types they
//! are built from (`spec/LLD.md` section 2).
//!
//! # Where the sections come from
//!
//! `spec/ARCHITECTURE.md` section 2's component table has no row for this crate,
//! so no section can be read off a table here. Every citation below is derived
//! from the specification text that governs the individual type, and each item
//! says which sentence it was derived from. That is deliberate: rulings R10, R12
//! and R13 in `ops/rulings.md` record three citations taken at one remove from
//! that table which were wrong, and the note under them names the table as the
//! common cause.
//!
//! The field lists are `spec/DATA_MODEL.md` section 2. The state values are its
//! section 3, which this ticket represents but does not drive: the transition
//! functions are ORI-T-0020 (`Ticket`) and ORI-T-0021 (`Document`, `Phase`), and
//! the permission function is ORI-T-0022.
//!
//! # What is deliberately not here
//!
//! `Phase`, `Criterion`, `Event`, `Escalation` and the rest of
//! `spec/DATA_MODEL.md` section 2's entities. `spec/LLD.md` section 2 gives this
//! crate `Product`, `Ticket`, `Document`, `Category`, `Tier`, `Role` and
//! `Scope`; the other entities belong to the crates that own them, and `Phase`
//! arrives with its state machine in ORI-T-0021.
//!
//! Must not: do IO, or import any other workspace crate (`spec/LLD.md` section
//! 2). Nothing in this module reads a file, a clock or an environment variable.
//! That is why there is no `Timestamp::now` and no identifier generator: both
//! need ambient state, and both belong to the crate that has it.

use core::fmt;
use core::str::FromStr;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::error::Error;
use crate::error::Result;

/// Declares a fieldless enum together with the one spelling each value carries
/// outside the engine, so that the value list, `as_str`, `Display` and
/// `FromStr` cannot disagree with each other.
///
/// No methodology section applies: this is a local device for keeping four
/// views of one list in step, not an implementation of a methodology rule.
macro_rules! wire_enum {
    (
        $(#[$enum_meta:meta])*
        $name:ident, $what:literal {
            $( $(#[$variant_meta:meta])* $variant:ident => $wire:literal , )+
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
        }

        impl $name {
            /// Every value, in the order the specification lists them.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )+ ];

            /// The spelling this value carries in `spec/DATA_MODEL.md` section 2
            /// and in everything derived from it.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $wire, )+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self> {
                match s {
                    $( $wire => Ok(Self::$variant), )+
                    other => Err(Error::malformed($what, other)),
                }
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------

/// The 32 symbols of Crockford base 32, which is the alphabet a ULID is written
/// in. `I`, `L`, `O` and `U` are absent by design.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The number of symbols in the canonical text form of a ULID.
const ULID_LEN: usize = 26;

/// An entity identifier.
///
/// No methodology section applies. The rule is `spec/DATA_MODEL.md` section 2's
/// opening sentence, "Identifiers are ULIDs unless stated", and the exceptions
/// it states (`Criterion.id` is human readable, `Phase.key` is `P1`, `M0` or
/// `G0` shaped) are carried by those entities rather than by this type.
///
/// This type parses and holds; it never generates. Generating a ULID needs a
/// clock and a source of randomness, and this crate reads neither.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(String);

impl Id {
    /// Reads the canonical text form of a ULID.
    ///
    /// Letters are accepted in either case and held upper case, which is what
    /// Crockford base 32 specifies, so that two spellings of one identifier
    /// compare equal. A first symbol above `7` is refused because the timestamp
    /// of such a value does not fit the 48 bits a ULID gives it.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() != ULID_LEN {
            return Err(Error::malformed("Id", text));
        }
        let mut upper = String::with_capacity(ULID_LEN);
        for byte in text.bytes() {
            let byte = byte.to_ascii_uppercase();
            if !CROCKFORD.contains(&byte) {
                return Err(Error::malformed("Id", text));
            }
            upper.push(char::from(byte));
        }
        // The first symbol carries the top bits of the 48 bit timestamp.
        if upper.as_bytes()[0] > b'7' {
            return Err(Error::malformed("Id", text));
        }
        Ok(Self(upper))
    }

    /// The identifier in its canonical text form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Id {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

/// A point in time, as milliseconds since the Unix epoch, UTC.
///
/// No methodology section applies. The rule is `spec/DATA_MODEL.md` section 2's
/// "Timestamps are UTC"; milliseconds are chosen because a ULID's own timestamp
/// is milliseconds since the same epoch, so an identifier and the row it keys
/// carry the same unit.
///
/// There is no constructor that reads a clock. `spec/LLD.md` section 2 forbids
/// this crate any IO, and the caller that has a clock passes the value in.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Builds a timestamp from milliseconds since the Unix epoch, UTC.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Milliseconds since the Unix epoch, UTC.
    #[must_use]
    pub const fn millis(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Who performed an action: AICD §13.
///
/// Derived from AICD §13's statement that git is the audit trail through which
/// "any line of code can be traced to a commit, to a ticket, to a paragraph of
/// specification, to a human decision", which is what requires every recorded
/// action to name its performer. `spec/DATA_MODEL.md` section 2 gives `Event` the
/// field as "actor (human identity or agent identity or system)" and its section
/// 4 states the invariant this enum exists to make representable: "Every `Event`
/// has an actor; `system` is allowed only for scheduled triggers and watchers".
///
/// The `Event` entity itself belongs to `ori-store` (`spec/LLD.md` section 2);
/// this is only the value its actor field and `Ticket.filed_by` hold.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Actor {
    /// A person, identified by the seat holder's identity.
    Human(Id),
    /// An agent identity (`spec/DATA_MODEL.md` section 2, `AgentIdentity`).
    Agent(Id),
    /// The engine acting on its own, which section 4 admits only for scheduled
    /// triggers and watchers.
    System,
}

impl Actor {
    /// The identity behind the actor, absent for [`Actor::System`].
    #[must_use]
    pub const fn identity(&self) -> Option<&Id> {
        match self {
            Self::Human(id) | Self::Agent(id) => Some(id),
            Self::System => None,
        }
    }

    /// Whether this is the engine acting on its own.
    #[must_use]
    pub const fn is_system(&self) -> bool {
        matches!(self, Self::System)
    }

    /// The kind of actor, in the spelling `spec/DATA_MODEL.md` section 2 uses.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Human(_) => "human",
            Self::Agent(_) => "agent",
            Self::System => "system",
        }
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.identity() {
            Some(id) => write!(f, "{}:{id}", self.kind()),
            None => f.write_str(self.kind()),
        }
    }
}

/// The section of the specification a ticket implements, violates or modifies:
/// AICD §9.
///
/// Derived from AICD §9's rule "A ticket that changes behavior must reference
/// the specification section it implements or modifies. A ticket with no
/// specification anchor is either a defect or a decisional ticket", and from
/// AICD §11's list of mandatory ticket contents, which names "Specification
/// anchor: the section implemented, violated or to be modified".
///
/// The anchor is held as the text the filer wrote, refusing only emptiness.
/// `spec/DATA_MODEL.md` section 2 fixes no format for this field.
/// `spec/CONVENTIONS.md` fixes `<document>#<section>` for the `Spec:` commit
/// trailer, and [`SpecAnchor::split`] reads that form when it is present, but
/// anchors in `ops/phase-1-backlog.md` are written in other forms as well, so
/// requiring one here would refuse anchors the project itself uses.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpecAnchor(String);

impl SpecAnchor {
    /// Reads an anchor, refusing one that is empty or only whitespace.
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(Error::malformed("SpecAnchor", text));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The anchor as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The document and the section within it, when the anchor is written in
    /// the `<document>#<section>` form `spec/CONVENTIONS.md` fixes for the
    /// `Spec:` commit trailer, and `None` when it is written in any other form.
    #[must_use]
    pub fn split(&self) -> Option<(&str, &str)> {
        let (document, section) = self.0.split_once('#')?;
        if document.is_empty() || section.is_empty() || section.contains('#') {
            return None;
        }
        Some((document, section))
    }
}

impl fmt::Display for SpecAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for SpecAnchor {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

/// What an agent may spend on a ticket before it must stop: AICD §12.
///
/// Derived from AICD §12's "Budgets and the blocked report": "Every ticket
/// carries a budget: attempts, wall-clock time, tokens. When any limit is
/// exceeded, the coder stops, writes a blocked report." The three fields are the
/// three that sentence names, in the spelling `spec/DATA_MODEL.md` section 2
/// gives them, "budget (attempts, wall_clock_s, tokens)".
///
/// This is the value only. The meter that spends it is `Budget` in `ori-runtime`
/// (`spec/LLD.md` section 2), which is the crate that can observe a clock.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Budget {
    /// How many attempts the agent may make.
    pub attempts: u32,
    /// Wall clock seconds the agent may spend.
    pub wall_clock_s: u64,
    /// Tokens the agent may spend.
    pub tokens: u64,
}

/// The modules a plan declares it will touch: AICD §12.
///
/// Derived from AICD §12's "Conflict handling at scale": "AICD does not rely on
/// serialization; it relies on declared scope. Every implementation plan
/// declares the modules it will touch. The lead agent maintains a lock table of
/// modules currently claimed by in-flight tickets and refuses to start a ticket
/// whose declared scope overlaps one already claimed."
///
/// # Which "scope" this is
///
/// `spec/LLD.md` section 2 names `Scope` in this crate without saying which of
/// the three things the specification calls a scope it means.
/// `spec/DATA_MODEL.md` section 2 uses the word for `Ticket.declared_scope
/// (modules)`, for `AgentIdentity.scopes (memory)` and for
/// `CredentialIssuance.scope`. This type is the first, because it is the one
/// this crate's own `Ticket` carries and the one AICD §12 makes a control. The
/// memory scope is enforced by `ScopeEnforcer` in `ori-memory` and the
/// credential scope by `Issuance` in `ori-broker`, both per `spec/LLD.md`
/// section 2, and neither is this type.
///
/// # What a module path is here
///
/// A repository relative path with `/` separators, holding no `*`. Glob
/// patterns are refused rather than stored, because a pattern this type does not
/// expand would answer [`Scope::overlaps`] with a confident `false`, which is
/// the lock table failing open. `ops/lock-table.md` records claims in glob form
/// today; a caller converting those into this type states the directory instead.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Scope {
    modules: BTreeSet<String>,
}

impl Scope {
    /// Reads a declared scope, refusing an entry that is empty, that is only
    /// whitespace, or that holds a `*`.
    pub fn new<I, S>(modules: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut set = BTreeSet::new();
        for module in modules {
            let module = module.as_ref();
            let normalized = module.trim().trim_end_matches('/');
            if normalized.is_empty() || normalized.contains('*') {
                return Err(Error::malformed("Scope module path", module));
            }
            set.insert(normalized.to_owned());
        }
        Ok(Self { modules: set })
    }

    /// The declared module paths, sorted.
    pub fn modules(&self) -> impl Iterator<Item = &str> {
        self.modules.iter().map(String::as_str)
    }

    /// How many module paths were declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Whether nothing was declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Whether this scope claims `path`, either by naming it or by naming a
    /// directory it sits under.
    #[must_use]
    pub fn claims(&self, path: &str) -> bool {
        let path = path.trim().trim_end_matches('/');
        self.modules.iter().any(|module| covers(module, path))
    }

    /// Whether two declared scopes overlap, which is what AICD §12 has the lock
    /// table refuse a second ticket on.
    ///
    /// The predicate only. The lock table that applies it is `LockTable` in
    /// `ori-orchestrator` (`spec/LLD.md` section 2).
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.modules.iter().any(|mine| {
            other
                .modules
                .iter()
                .any(|theirs| covers(mine, theirs) || covers(theirs, mine))
        })
    }
}

/// Whether `directory` names `path` or a directory `path` sits under.
///
/// No methodology section applies: this is the path arithmetic behind
/// [`Scope::overlaps`], not a rule of its own.
fn covers(directory: &str, path: &str) -> bool {
    if directory == path {
        return true;
    }
    path.len() > directory.len()
        && path.starts_with(directory)
        && path.as_bytes()[directory.len()] == b'/'
}

// ---------------------------------------------------------------------------
// The small closed enumerations
// ---------------------------------------------------------------------------

wire_enum! {
    /// How much human involvement a ticket needs before it is worked: AICD §11.
    ///
    /// Derived from AICD §11's "The four categories" table, which names these
    /// four and states the involvement each carries.
    ///
    /// # Why this does not derive `Ord`
    ///
    /// AICD §11's upgrade-only rule is a ladder over the first three: "The lead
    /// agent may upgrade a category (Auto to Behavioral, Behavioral to
    /// Decisional) but never downgrade it." [`Category::ProductSignal`] is not
    /// on that ladder; the same table calls it "Not a defect: an insight about
    /// usage or value" that "Never enters the coder queue directly". A derived
    /// `Ord` would place it above `Decisional` purely because it is declared
    /// last, and a comparison written against that would read as the upgrade
    /// rule while meaning something else. The rule itself is ORI-T-0020, which
    /// carries ORI-P1-005.
    Category, "Category" {
        /// A clear defect whose fix stays inside the current specification.
        Auto => "auto",
        /// Changes observable behavior, inside the brief and the architecture.
        Behavioral => "behavioral",
        /// Touches the specification, the architecture, the data model,
        /// security, external cost, a contract, or any tier 2 area.
        Decisional => "decisional",
        /// An insight about usage or value. Goes to the product owner only.
        ProductSignal => "product_signal",
    }
}

wire_enum! {
    /// The risk tier of a change, which fixes what it takes to merge it:
    /// AICD §13.
    ///
    /// Derived from AICD §13's "Merge policy by risk tier" table, which defines
    /// the three tiers and their merge requirements. AICD §17 names that table
    /// as the merge axis of the autonomy model ("The risk tiers of section 13
    /// define what an agent may merge") but does not define the tiers.
    ///
    /// The ordering is the one the table gives, so `Tier::Zero < Tier::Two`
    /// reads as "less is required to merge it". Ruling R11 in `ops/rulings.md`
    /// applies AICD §11's upgrade-only rule to this ladder as well: an agent may
    /// raise a tier and only a human may lower one.
    #[derive(Ord, PartialOrd)]
    Tier, "Tier" {
        /// Documentation, logs, copy, dependency patch versions, formatting,
        /// test-only additions.
        Zero => "0",
        /// Feature and fix code that touches no tier 2 area and changes no
        /// contract.
        One => "1",
        /// Authentication and authorization, money, data schema and migrations,
        /// infrastructure, secrets, external contracts, anything an ADR covers.
        Two => "2",
    }
}

impl Tier {
    /// The tier as the number `spec/RISK_MAP.md` and AICD §13 write it.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Zero => 0,
            Self::One => 1,
            Self::Two => 2,
        }
    }

    /// Reads a tier from the number AICD §13 writes it as.
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Zero),
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            other => Err(Error::malformed("Tier", other.to_string())),
        }
    }
}

wire_enum! {
    /// What an agent identity is for: AICD §7.
    ///
    /// Derived from AICD §7's "The roles" table, which names these seven and
    /// gives each its responsibility, what it reads and what it writes. AICD
    /// §17's permission matrix has a row for each of the same seven, and
    /// `spec/DATA_MODEL.md` section 2 gives `AgentIdentity` the field as "role
    /// (coder, lead, qa, operations, documentation, product_signal,
    /// assistant)", which fixes the spellings below.
    ///
    /// This is the agent side. The human side is [`Seat`].
    Role, "Role" {
        /// Takes one validated ticket and implements it on its own branch.
        Coder => "coder",
        /// Orders the queue, assigns, reviews, escalates, merges tier 0.
        Lead => "lead",
        /// Turns acceptance criteria into test plans and runs them.
        Qa => "qa",
        /// Watches production health, deploys from tags, executes runbooks.
        Operations => "operations",
        /// Proposes the specification update on every merge; runs the drift
        /// audit.
        Documentation => "documentation",
        /// Watches usage analytics and files product signal tickets.
        ProductSignal => "product_signal",
        /// The human's thinking partner. Drafts only, writes nothing else.
        Assistant => "assistant",
    }
}

wire_enum! {
    /// A human seat: AICD §18.
    ///
    /// Derived from AICD §18's seat table, "The team is organized in three seats
    /// plus a product owner", which names these four and what each approves.
    /// `spec/DATA_MODEL.md` section 2 gives `Seat` the field as "seat
    /// (architect, verification_lead, reliability_governance, product_owner)",
    /// which fixes the spellings below.
    ///
    /// A seat is a set of responsibilities, not a person: AICD §18 states that
    /// "Early on, one person may hold more than one seat". The holder is a field
    /// of the `Seat` row in `spec/DATA_MODEL.md` section 2, not a part of this
    /// value.
    Seat, "Seat" {
        /// Canonical knowledge: architecture, ADRs, domain model, contracts,
        /// conventions, agent instructions. Approves decisional tickets.
        Architect => "architect",
        /// Acceptance criteria, test plans, thresholds. Approves tier 1 pull
        /// requests and ticket category downgrades.
        VerificationLead => "verification_lead",
        /// Identities, permissions, secrets, infrastructure, runbooks, the audit
        /// trail.
        ReliabilityGovernance => "reliability_governance",
        /// The product brief, prioritization and what is worth building.
        ProductOwner => "product_owner",
    }
}

// ---------------------------------------------------------------------------
// Product
// ---------------------------------------------------------------------------

wire_enum! {
    /// Whether a product started under AICD or was brought into it: AICD §23,
    /// AICD §24.
    ///
    /// Derived from the two sections that are the two answers: AICD §23 is
    /// "Starting a new product under AICD" and AICD §24 is "Migrating an
    /// existing product into AICD". `spec/DATA_MODEL.md` section 2 gives
    /// `Product` the field as "origin (new, migrated)", which fixes the
    /// spellings below.
    ProductOrigin, "ProductOrigin" {
        /// Started under AICD, through the G0 to G7 gates of AICD §23.
        New => "new",
        /// Brought into AICD, through the M0 to M5 phases of AICD §24.
        Migrated => "migrated",
    }
}

/// One product the engine runs: AICD §26.
///
/// Derived from AICD §26, "Operating several products with one team", which
/// makes the product the unit everything else is scoped to: "One fleet instance
/// per product, with its own agent identities, memory scopes, ticket queue and
/// continuous test environment. Fleets never share credentials or context."
/// `spec/DATA_MODEL.md` section 2's `Product` row supplies the fields and the
/// note "One SQLite file per product".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    /// The product identifier.
    pub id: Id,
    /// What the product is called.
    pub name: String,
    /// Where its repository sits on this machine.
    pub repo_path: PathBuf,
    /// The repository's remote, absent until one is configured.
    ///
    /// `spec/DATA_MODEL.md` section 2 does not mark this nullable, but criterion
    /// ORI-P1-001 creates a product from an empty directory, where no remote
    /// exists yet.
    pub repo_remote: Option<String>,
    /// The version of the methodology this product runs, which is the version
    /// under `methodology/` (`spec/LLD.md` section 1).
    pub aicd_version: String,
    /// Whether the product was started under AICD or migrated into it.
    pub origin: ProductOrigin,
    /// When the product was created.
    pub created_at: Timestamp,
}

// ---------------------------------------------------------------------------
// Ticket
// ---------------------------------------------------------------------------

wire_enum! {
    /// What a ticket is: AICD §11.
    ///
    /// Derived from AICD §11's "Mandatory ticket contents", which separates
    /// "For defects" from "For features", and from its closing rule, which puts
    /// a further condition on defects alone: "'Closed' requires the
    /// documentation agent's specification update (or its explicit 'no change
    /// needed') and, for defects, new acceptance criteria that cover the case."
    /// The third value is `spec/DATA_MODEL.md` section 2's, which gives `Ticket`
    /// the field as "kind (defect, feature, chore)".
    TicketKind, "TicketKind" {
        /// Something the product does that the specification says it must not.
        Defect => "defect",
        /// Something the product does not yet do.
        Feature => "feature",
        /// Work that changes no behavior.
        Chore => "chore",
    }
}

wire_enum! {
    /// Where a ticket is in its lifecycle: AICD §11.
    ///
    /// Derived from AICD §11's "Lifecycle", "Filed to Categorized to Validated
    /// to Queued to In progress to In review to Merged to Deployed to Closed".
    /// `spec/DATA_MODEL.md` section 3's Ticket diagram adds the three states
    /// that leave that line, `Rejected`, `Blocked` and `Escalated`, which are
    /// the outcomes AICD §11 and AICD §12 describe in prose.
    ///
    /// The transitions between these values are ORI-T-0020. This enum only
    /// makes the states representable.
    TicketState, "TicketState" {
        /// Filed by a human or an agent, not yet categorized.
        Filed => "filed",
        /// Carries a category, awaiting validation.
        Categorized => "categorized",
        /// Refused: the ticket will not be worked.
        Rejected => "rejected",
        /// Authorized to be worked. Automatic for `Auto` and `Behavioral`,
        /// human approved for `Decisional`.
        Validated => "validated",
        /// In the queue, awaiting assignment.
        Queued => "queued",
        /// Assigned, with its declared scope locked.
        InProgress => "in_progress",
        /// The agent exhausted its budget and wrote a blocked report.
        Blocked => "blocked",
        /// An escalation trigger fired and a human was asked.
        Escalated => "escalated",
        /// The pull request is ready for review.
        InReview => "in_review",
        /// The pull request landed.
        Merged => "merged",
        /// Released from a tag.
        Deployed => "deployed",
        /// The specification update and, for a defect, the accepted criterion
        /// are recorded.
        Closed => "closed",
    }
}

/// One unit of work: AICD §11.
///
/// Derived from AICD §11's opening sentence, "Tickets are the control point of
/// AICD. Every unit of work, whether requested by a human or discovered by an
/// agent, is a ticket." The fields are `spec/DATA_MODEL.md` section 2's `Ticket`
/// row, and AICD §11's "Mandatory ticket contents" is what makes `category`,
/// `tier`, `spec_anchor` and `budget` required rather than optional.
///
/// The transition function `Ticket::apply(event) -> Result<Ticket>` that
/// `spec/LLD.md` section 2 names is ORI-T-0020 and is not in this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ticket {
    /// The ticket identifier.
    pub id: Id,
    /// The product it belongs to.
    pub product_id: Id,
    /// What the ticket is called.
    pub title: String,
    /// How much human involvement it needs before it is worked.
    pub category: Category,
    /// Whether it is a defect, a feature or a chore.
    pub kind: TicketKind,
    /// The risk tier of the change it asks for.
    pub tier: Tier,
    /// Where it is in its lifecycle.
    pub state: TicketState,
    /// The specification section it implements, violates or modifies.
    pub spec_anchor: SpecAnchor,
    /// The modules its plan declared it would touch.
    pub declared_scope: Scope,
    /// What the agent working it may spend.
    pub budget: Budget,
    /// Who filed it.
    pub filed_by: Actor,
    /// The phase it belongs to.
    pub phase_id: Id,
    /// Whether the merged change was a significant modification, `None` until
    /// the label is set.
    ///
    /// `spec/DATA_MODEL.md` section 2 types this `bool` and adds "set at merge",
    /// which a plain `bool` cannot distinguish from a merged change that was
    /// labelled not significant. Criterion ORI-P1-027 spells the label
    /// `significant`, which is the spelling used here.
    pub significant: Option<bool>,
    /// The incident this ticket is, when it is one.
    pub incident_id: Option<Id>,
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

wire_enum! {
    /// Which set a document belongs to: AICD §9.
    ///
    /// Derived from AICD §9's "Every product has the same set of documents",
    /// read together with AICD §39's lesson that replaced the all-upfront
    /// specification with a "foundation set and phase set". The third value is
    /// the as-built set AICD §24.3 produces during a migration.
    /// `spec/DATA_MODEL.md` section 2 gives `Document` the field as "set
    /// (foundation, phase, migration)", which fixes the spellings below.
    DocumentSet, "DocumentSet" {
        /// Required before a product may launch.
        Foundation => "foundation",
        /// Belongs to one phase of the roadmap.
        Phase => "phase",
        /// Produced by a migration.
        Migration => "migration",
    }
}

wire_enum! {
    /// Where a document is in its review cycle: AICD §9.
    ///
    /// Derived from AICD §9's two rules, "Specification changes are pull
    /// requests, reviewed and merged under the same risk tiers as code" and
    /// "Every document carries a 'last verified against code' date maintained by
    /// the drift audit". The first gives the draft, review and approval states;
    /// the second gives `Stale`, which is what the drift audit sets.
    /// `spec/DATA_MODEL.md` section 3's Document diagram is the same five states.
    ///
    /// The transitions between these values are ORI-T-0021.
    DocumentState, "DocumentState" {
        /// The document does not exist yet.
        Missing => "missing",
        /// Written, not yet submitted.
        Draft => "draft",
        /// Submitted to its owning seat.
        UnderReview => "under_review",
        /// Signed by its owning seat.
        Approved => "approved",
        /// The drift audit found the document and the code disagree.
        Stale => "stale",
    }
}

/// The prefix `spec/DATA_MODEL.md` section 2 writes as `as_built_*`.
const AS_BUILT_PREFIX: &str = "as_built_";

/// Which document this is: AICD §9.
///
/// Derived from AICD §9's document table, which names the set every product
/// carries and its owner. The values below are `spec/DATA_MODEL.md` section 2's
/// list for `Document.kind`, which is AICD §9's table as this product splits it
/// across the files under `spec/`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DocumentKind {
    /// The product brief.
    Brief,
    /// The product requirements document.
    Prd,
    /// The architecture document.
    Architecture,
    /// One architecture decision record.
    Adr,
    /// The domain model.
    DataModel,
    /// The API and interface contracts.
    ApiSpec,
    /// The low-level design.
    Lld,
    /// Conventions and agent instructions.
    Conventions,
    /// Security notes.
    SecurityNotes,
    /// Environment setup and the permission manifest.
    EnvSetup,
    /// The permission manifest, where it is kept apart from `EnvSetup`.
    Permissions,
    /// The test strategy.
    Testing,
    /// The observability plan.
    Observability,
    /// The continuous integration and delivery plan.
    CiCd,
    /// The roadmap.
    Roadmap,
    /// One runbook.
    Runbook,
    /// Instruction files for the agent roles.
    AgentInstructions,
    /// The risk map.
    RiskMap,
    /// Acceptance criteria for one phase.
    Criteria,
    /// One of the as-built documents a migration produces (AICD §24.3), holding
    /// the part after `as_built_`.
    ///
    /// `spec/DATA_MODEL.md` section 2 writes this entry as the pattern
    /// `as_built_*` rather than as a value, so the suffix is carried here rather
    /// than enumerated.
    AsBuilt(String),
}

impl DocumentKind {
    /// The spelling this kind carries in `spec/DATA_MODEL.md` section 2.
    #[must_use]
    pub fn as_str(&self) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow;
        let fixed = match self {
            Self::Brief => "brief",
            Self::Prd => "prd",
            Self::Architecture => "architecture",
            Self::Adr => "adr",
            Self::DataModel => "data_model",
            Self::ApiSpec => "api_spec",
            Self::Lld => "lld",
            Self::Conventions => "conventions",
            Self::SecurityNotes => "security_notes",
            Self::EnvSetup => "env_setup",
            Self::Permissions => "permissions",
            Self::Testing => "testing",
            Self::Observability => "observability",
            Self::CiCd => "ci_cd",
            Self::Roadmap => "roadmap",
            Self::Runbook => "runbook",
            Self::AgentInstructions => "agent_instructions",
            Self::RiskMap => "risk_map",
            Self::Criteria => "criteria",
            Self::AsBuilt(of) => return Cow::Owned(format!("{AS_BUILT_PREFIX}{of}")),
        };
        Cow::Borrowed(fixed)
    }

    /// Every kind that is a single value, which is every kind but
    /// [`DocumentKind::AsBuilt`].
    pub const FIXED: &'static [Self] = &[
        Self::Brief,
        Self::Prd,
        Self::Architecture,
        Self::Adr,
        Self::DataModel,
        Self::ApiSpec,
        Self::Lld,
        Self::Conventions,
        Self::SecurityNotes,
        Self::EnvSetup,
        Self::Permissions,
        Self::Testing,
        Self::Observability,
        Self::CiCd,
        Self::Roadmap,
        Self::Runbook,
        Self::AgentInstructions,
        Self::RiskMap,
        Self::Criteria,
    ];
}

impl fmt::Display for DocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

impl FromStr for DocumentKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(of) = s.strip_prefix(AS_BUILT_PREFIX) {
            return if of.is_empty() {
                Err(Error::malformed("DocumentKind", s))
            } else {
                Ok(Self::AsBuilt(of.to_owned()))
            };
        }
        Self::FIXED
            .iter()
            .find(|kind| kind.as_str() == s)
            .cloned()
            .ok_or_else(|| Error::malformed("DocumentKind", s))
    }
}

/// One document of the specification: AICD §9.
///
/// Derived from AICD §9's opening, "Because the specification is the source of
/// truth, it needs the discipline that source code has: a fixed structure, clear
/// ownership, versioning and review", and from its rule that "Every document
/// carries a 'last verified against code' date maintained by the drift audit",
/// which is the last field. The fields are `spec/DATA_MODEL.md` section 2's
/// `Document` row.
///
/// The transitions between document states are ORI-T-0021.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    /// The document identifier.
    pub id: Id,
    /// The product it belongs to.
    pub product_id: Id,
    /// Where it sits under `spec/`, repository relative, `/` separated.
    ///
    /// Held as text rather than as a path so that one document compares equal to
    /// itself across the three platforms `spec/TESTING.md` section 1 names.
    pub path: String,
    /// Which document this is.
    pub kind: DocumentKind,
    /// Which set it belongs to.
    pub set: DocumentSet,
    /// Where it is in its review cycle.
    pub state: DocumentState,
    /// The seat that signed it, absent until one has.
    ///
    /// `spec/DATA_MODEL.md` section 3 states the invariant this holds, "only the
    /// owning seat may sign". The holder behind the seat is a field of the `Seat`
    /// row, not of this one.
    pub approved_by: Option<Seat>,
    /// When it was signed, absent until it was.
    pub approved_at: Option<Timestamp>,
    /// When the drift audit last checked it against the code, absent until one
    /// has run.
    pub verified_against_code_at: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ULID that parses, used wherever a test needs an identifier and does
    /// not care which.
    const SAMPLE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn ori_t_0019_an_identifier_is_read_in_the_canonical_form_of_a_ulid() {
        let id = Id::parse(SAMPLE_ULID).expect("a well formed ULID parses");
        assert_eq!(id.as_str(), SAMPLE_ULID);
        assert_eq!(id.to_string(), SAMPLE_ULID);
    }

    #[test]
    fn ori_t_0019_an_identifier_is_held_upper_case_so_one_value_has_one_spelling() {
        let lower = Id::parse(&SAMPLE_ULID.to_lowercase()).expect("Crockford base 32 is case free");
        assert_eq!(
            lower,
            Id::parse(SAMPLE_ULID).expect("the upper case form parses")
        );
    }

    #[test]
    fn ori_p1_033_a_malformed_identifier_is_refused_and_is_not_a_methodology_refusal() {
        // Too short, too long, a symbol outside Crockford base 32, and a first
        // symbol whose timestamp does not fit 48 bits.
        for bad in [
            "",
            "01ARZ3NDEKTSV4RRFFQ69G5FA",
            "01ARZ3NDEKTSV4RRFFQ69G5FAVV",
            "01ARZ3NDEKTSV4RRFFQ69G5FAU",
            "81ARZ3NDEKTSV4RRFFQ69G5FAV",
        ] {
            let error = Id::parse(bad).expect_err("a malformed identifier is refused");
            assert!(
                !error.is_refusal(),
                "{bad} is bad input, not a control refusing an action"
            );
            assert!(
                error.methodology_ref().is_none(),
                "{bad} names no methodology section"
            );
        }
    }

    #[test]
    fn ori_t_0019_a_timestamp_is_milliseconds_since_the_unix_epoch() {
        let at = Timestamp::from_millis(1_764_000_000_000);
        assert_eq!(at.millis(), 1_764_000_000_000);
        assert!(Timestamp::from_millis(-1) < at);
    }

    #[test]
    fn ori_t_0019_an_actor_is_a_human_an_agent_or_the_system() {
        let id = Id::parse(SAMPLE_ULID).expect("the sample parses");
        assert_eq!(Actor::Human(id.clone()).identity(), Some(&id));
        assert_eq!(Actor::Agent(id.clone()).identity(), Some(&id));
        assert_eq!(Actor::System.identity(), None);
        assert!(Actor::System.is_system());
        assert!(!Actor::Agent(id).is_system());
    }

    #[test]
    fn ori_t_0019_a_spec_anchor_splits_only_when_it_is_written_in_the_trailer_form() {
        let trailer = SpecAnchor::parse("LLD.md#2-crate-responsibilities-and-dependency-direction")
            .expect("the commit trailer form parses");
        assert_eq!(
            trailer.split(),
            Some((
                "LLD.md",
                "2-crate-responsibilities-and-dependency-direction"
            ))
        );

        // The form ops/phase-1-backlog.md uses for ORI-T-0022 is an anchor too.
        let prose = SpecAnchor::parse("SECURITY_NOTES \"Authorization model\"")
            .expect("an anchor in another form is still an anchor");
        assert_eq!(prose.split(), None);
    }

    #[test]
    fn ori_p1_033_an_empty_spec_anchor_is_refused_and_is_not_a_methodology_refusal() {
        let error = SpecAnchor::parse("   ").expect_err("an empty anchor is refused");
        assert!(!error.is_refusal());
        assert!(error.methodology_ref().is_none());
    }

    #[test]
    fn ori_t_0019_a_scope_overlaps_another_when_one_claims_a_path_the_other_claims() {
        let core = Scope::new(["crates/ori-core"]).expect("a module path");
        let types = Scope::new(["crates/ori-core/src/types.rs"]).expect("a module path");
        let store = Scope::new(["crates/ori-store"]).expect("a module path");

        assert!(
            core.overlaps(&types),
            "a directory claims what sits under it"
        );
        assert!(
            types.overlaps(&core),
            "and the answer does not depend on the order"
        );
        assert!(!core.overlaps(&store));
        assert!(core.claims("crates/ori-core/src/error.rs"));
        assert!(
            !core.claims("crates/ori-core-extra/src/lib.rs"),
            "a prefix that is not a path boundary is not a claim"
        );
    }

    #[test]
    fn ori_t_0019_an_empty_scope_overlaps_nothing() {
        let empty = Scope::new(Vec::<String>::new()).expect("declaring nothing is allowed");
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert!(!empty.overlaps(&Scope::new(["crates/ori-core"]).expect("a module path")));
    }

    #[test]
    fn ori_p1_033_a_glob_is_refused_rather_than_stored_unexpanded() {
        // An unexpanded glob would answer `overlaps` with a confident false,
        // which is the lock table failing open.
        let error = Scope::new(["crates/ori-core/**"]).expect_err("a glob is refused");
        assert!(!error.is_refusal());
        assert!(error.methodology_ref().is_none());
    }

    #[test]
    fn ori_t_0019_every_enumeration_round_trips_through_the_spelling_it_is_stored_as() {
        for value in Category::ALL {
            assert_eq!(
                value.as_str().parse::<Category>().expect("round trip"),
                *value
            );
        }
        for value in Tier::ALL {
            assert_eq!(value.as_str().parse::<Tier>().expect("round trip"), *value);
            assert_eq!(Tier::from_u8(value.as_u8()).expect("round trip"), *value);
        }
        for value in Role::ALL {
            assert_eq!(value.as_str().parse::<Role>().expect("round trip"), *value);
        }
        for value in Seat::ALL {
            assert_eq!(value.as_str().parse::<Seat>().expect("round trip"), *value);
        }
        for value in ProductOrigin::ALL {
            assert_eq!(
                value.as_str().parse::<ProductOrigin>().expect("round trip"),
                *value
            );
        }
        for value in TicketKind::ALL {
            assert_eq!(
                value.as_str().parse::<TicketKind>().expect("round trip"),
                *value
            );
        }
        for value in TicketState::ALL {
            assert_eq!(
                value.as_str().parse::<TicketState>().expect("round trip"),
                *value
            );
        }
        for value in DocumentSet::ALL {
            assert_eq!(
                value.as_str().parse::<DocumentSet>().expect("round trip"),
                *value
            );
        }
        for value in DocumentState::ALL {
            assert_eq!(
                value.as_str().parse::<DocumentState>().expect("round trip"),
                *value
            );
        }
        for value in DocumentKind::FIXED {
            assert_eq!(
                &value.as_str().parse::<DocumentKind>().expect("round trip"),
                value
            );
        }
    }

    #[test]
    fn ori_t_0019_the_value_lists_are_the_ones_the_data_model_states() {
        assert_eq!(Category::ALL.len(), 4, "AICD §11 names four categories");
        assert_eq!(Tier::ALL.len(), 3, "AICD §13 names three tiers");
        assert_eq!(Role::ALL.len(), 7, "AICD §7 names seven agent roles");
        assert_eq!(
            Seat::ALL.len(),
            4,
            "AICD §18 names three seats plus a product owner"
        );
        assert_eq!(
            ProductOrigin::ALL.len(),
            2,
            "ProductOrigin::ALL: DATA_MODEL section 2 gives Product the field as \
             \"origin (new, migrated)\", which is two values"
        );
        assert_eq!(
            TicketKind::ALL.len(),
            3,
            "TicketKind::ALL: DATA_MODEL section 2 gives Ticket the field as \
             \"kind (defect, feature, chore)\", which is three values"
        );
        assert_eq!(
            DocumentSet::ALL.len(),
            3,
            "DocumentSet::ALL: DATA_MODEL section 2 gives Document the field as \
             \"set (foundation, phase, migration)\", which is three values"
        );
        assert_eq!(
            TicketState::ALL.len(),
            12,
            "DATA_MODEL section 3 draws twelve ticket states"
        );
        assert_eq!(
            DocumentState::ALL.len(),
            5,
            "DATA_MODEL section 3 draws five document states"
        );
        assert_eq!(
            DocumentKind::FIXED.len(),
            19,
            "DATA_MODEL section 2 lists nineteen named document kinds"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0098: the spellings, and not only how many there are.
    //
    // Every assertion above pins a length and none pins a spelling. ORI-T-0093
    // planted the consequence rather than arguing it: respelling
    // ProductOrigin's second value from "migrated" to "moved" leaves this crate
    // and spec/DATA_MODEL.md section 2 disagreeing with the whole workspace
    // green, at exit 0. The round-trip test above cannot see it, because it
    // leaves through as_str and returns through FromStr and a rename moves
    // both. ORI-T-0093 left the gap open for all ten lists rather than close it
    // for three, on the grounds that an asymmetric repair to a test about
    // checks that look like checks invites the next reader to guess which shape
    // is meant. This closes it for every list the document spells.
    //
    // # What is restated here, and what holds the restatement
    //
    // This crate may not do IO and may not import a workspace crate
    // (spec/LLD.md section 2, and CLAUDE.md's load-bearing facts), so no test
    // here can read spec/DATA_MODEL.md. The document's own sentences are
    // restated below instead, which is the bar that makes error.rs restate the
    // methodology index rather than read it.
    //
    // Nothing holds these sentences against spec/DATA_MODEL.md. error.rs's
    // restatement is held against the methodology by a test in
    // crates/ori-gates that reads error.rs as text and compares it with the
    // parsed document; the same machinery is what this restatement wants, and
    // crates/ori-gates is outside this ticket's declared scope, so this ticket
    // reports the need and does not build it. Until it exists, a restatement
    // altered in both halves at once, the sentence and the list under it, would
    // certify a spelling the document does not carry. Altering one half alone
    // fails here.
    //
    // # Six lists and not ten
    //
    // Section 2 writes six of this crate's ten lists out as values: Product's
    // origin, Seat's seat, AgentIdentity's role, Ticket's kind, Document's set
    // and Document's kind. For the other four it writes the field bare, and
    // section 3 draws two of them as state machines whose node names are not
    // wire spellings. Where the document states no spelling there is nothing to
    // assert against, and an assertion written anyway would take its
    // expectation from the code, which is a check certifying its own subject.
    // The four are recorded in NOT_SPELLED_BY_SECTION_2 so that the silence is
    // stated rather than left for a reader to rediscover.
    // -----------------------------------------------------------------------

    /// One value list as `spec/DATA_MODEL.md` section 2 writes it.
    ///
    /// The row and the quote are the document's, copied. The spellings are what
    /// the quote lists, written out so that a reader sees the list without
    /// parsing a sentence; the test below checks the two halves against each
    /// other, so the pair is not a second thing to keep in step by hand.
    #[derive(Clone, Copy)]
    struct Stated {
        /// The list, written as this crate names it.
        list: &'static str,
        /// The entity row of section 2 whose field carries the list.
        row: &'static str,
        /// That field, in the document's words, verbatim.
        quote: &'static str,
        /// The spellings the quote lists, in the order it lists them.
        spellings: &'static [&'static str],
        /// The pattern the quote ends with where it writes one in place of a
        /// value. Only `Document.kind` has one: the document writes the
        /// as-built family as `as_built_*`, and this crate carries it as
        /// `DocumentKind::AsBuilt` rather than as an entry of
        /// `DocumentKind::FIXED`. The as-built family has a test of its own
        /// further down this module, which is where that pattern is checked.
        pattern: Option<&'static str>,
    }

    /// Every value list of this crate whose spellings `spec/DATA_MODEL.md`
    /// section 2 states, with the sentence that states them.
    const SPELLED_BY_SECTION_2: [Stated; 6] = [
        Stated {
            list: "ProductOrigin::ALL",
            row: "Product",
            quote: "origin (new, migrated)",
            spellings: &["new", "migrated"],
            pattern: None,
        },
        Stated {
            list: "Seat::ALL",
            row: "Seat",
            quote: "seat (architect, verification_lead, reliability_governance, product_owner)",
            spellings: &[
                "architect",
                "verification_lead",
                "reliability_governance",
                "product_owner",
            ],
            pattern: None,
        },
        Stated {
            list: "Role::ALL",
            row: "AgentIdentity",
            quote: "role (coder, lead, qa, operations, documentation, product_signal, assistant)",
            spellings: &[
                "coder",
                "lead",
                "qa",
                "operations",
                "documentation",
                "product_signal",
                "assistant",
            ],
            pattern: None,
        },
        Stated {
            list: "TicketKind::ALL",
            row: "Ticket",
            quote: "kind (defect, feature, chore)",
            spellings: &["defect", "feature", "chore"],
            pattern: None,
        },
        Stated {
            list: "DocumentSet::ALL",
            row: "Document",
            quote: "set (foundation, phase, migration)",
            spellings: &["foundation", "phase", "migration"],
            pattern: None,
        },
        Stated {
            list: "DocumentKind::FIXED",
            row: "Document",
            quote: "kind (brief, prd, architecture, adr, data_model, api_spec, lld, conventions, \
                    security_notes, env_setup, permissions, testing, observability, ci_cd, \
                    roadmap, runbook, agent_instructions, risk_map, criteria, as_built_*)",
            spellings: &[
                "brief",
                "prd",
                "architecture",
                "adr",
                "data_model",
                "api_spec",
                "lld",
                "conventions",
                "security_notes",
                "env_setup",
                "permissions",
                "testing",
                "observability",
                "ci_cd",
                "roadmap",
                "runbook",
                "agent_instructions",
                "risk_map",
                "criteria",
            ],
            pattern: Some("as_built_*"),
        },
    ];

    /// Every value list of this crate that `spec/DATA_MODEL.md` section 2
    /// spells nowhere, with what the document does write and where the crate's
    /// spellings come from instead.
    ///
    /// Nothing is asserted against these four. The reason is written in the
    /// block above: an expectation taken from the code certifies the code. Each
    /// length is pinned by the test above this one; no spelling of theirs is
    /// pinned by anything, here or elsewhere in this crate.
    const NOT_SPELLED_BY_SECTION_2: [(&str, &str); 4] = [
        (
            "Category::ALL",
            "section 2 writes the Ticket row's field as bare \"category\". Section 3's Ticket \
             diagram writes \"auto (Auto, Behavioral) or human (Decisional)\" as a transition \
             condition, which names three of the four in prose and spells none of them. The \
             four spellings are AICD §11's category table as this crate renders it",
        ),
        (
            "Tier::ALL",
            "section 2 writes the field as bare \"tier\" in the Ticket, Criterion and \
             PullRequest rows. The three values are AICD §13's merge policy table, and the \
             spellings \"0\", \"1\" and \"2\" are the numbers that table and spec/RISK_MAP.md \
             write them as",
        ),
        (
            "TicketState::ALL",
            "section 2 writes the Ticket row's field as bare \"state\" and points at \"State \
             machine below\". Section 3's Ticket diagram draws twelve states, in node names \
             such as InProgress, which are not the wire spellings this crate carries",
        ),
        (
            "DocumentState::ALL",
            "section 2 writes the Document row's field as bare \"state\". Section 3's Document \
             diagram draws five states, in node names such as UnderReview, which are not the \
             wire spellings this crate carries",
        ),
    ];

    /// The section 2 sentence registered for one list.
    fn stated(list: &str) -> Stated {
        for entry in SPELLED_BY_SECTION_2 {
            if entry.list == list {
                return entry;
            }
        }
        panic!(
            "{list} is being checked against spec/DATA_MODEL.md section 2 and \
             SPELLED_BY_SECTION_2 registers no sentence for it"
        );
    }

    /// The values one section 2 field lists, in the order it writes them.
    ///
    /// Section 2 writes a closed field as a name and one parenthesis: "origin
    /// (new, migrated)". This reads what that parenthesis holds and splits it
    /// on commas. A quote it cannot read yields nothing, which is a failure at
    /// the caller and never a pass.
    fn values_in(quote: &str) -> Vec<&str> {
        let (Some(open), Some(close)) = (quote.find('('), quote.rfind(')')) else {
            return Vec::new();
        };
        if close < open {
            return Vec::new();
        }
        quote[open + 1..close]
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect()
    }

    /// Checks one list against the sentence of `spec/DATA_MODEL.md` section 2
    /// that states it, and returns the list it checked.
    ///
    /// Two things are checked. The spellings the crate carries are the
    /// document's, in the document's order; and each spelling the document
    /// writes, handed to `FromStr` as the document writes it, reads back as the
    /// value that sits in that position. The second is what a rename cannot
    /// hide: it starts from the document's text rather than from the crate's.
    fn assert_spelled_as_stated<T>(
        list: &'static str,
        values: &[T],
        spelling: impl Fn(&T) -> String,
    ) -> &'static str
    where
        T: FromStr + PartialEq + fmt::Debug,
        <T as FromStr>::Err: fmt::Display,
    {
        let entry = stated(list);
        let expected = entry.spellings;
        let found: Vec<String> = values.iter().map(spelling).collect();

        let mut defects: Vec<String> = Vec::new();
        for index in 0..expected.len().max(found.len()) {
            match (expected.get(index), found.get(index)) {
                (Some(want), Some(have)) if *want != have.as_str() => defects.push(format!(
                    "value {} of {}: the document spells it \"{want}\" and the crate spells it \
                     \"{have}\"",
                    index + 1,
                    expected.len()
                )),
                (Some(want), None) => defects.push(format!(
                    "the document lists \"{want}\" as value {} of {} and the crate's list ends \
                     at {}",
                    index + 1,
                    expected.len(),
                    found.len()
                )),
                (None, Some(have)) => defects.push(format!(
                    "the crate has \"{have}\" as value {} and the document's list ends at {}",
                    index + 1,
                    expected.len()
                )),
                _ => {}
            }
        }
        if !defects.is_empty() {
            let stated_set: BTreeSet<&str> = expected.iter().copied().collect();
            let found_set: BTreeSet<&str> = found.iter().map(String::as_str).collect();
            if stated_set == found_set {
                defects.push(
                    "every value the document spells is spelled by the crate and the order is \
                     not the document's. The order is part of what is pinned here: ALL is \
                     documented as the order the specification lists the values in, and Tier \
                     derives Ord from its declaration order, where AICD §13's merge ladder is \
                     what that order means"
                        .to_owned(),
                );
            }
        }
        assert!(
            defects.is_empty(),
            "{list} and spec/DATA_MODEL.md section 2 disagree. The document's {} row writes the \
             field as \"{}\".\n  document: {expected:?}\n  crate:    {found:?}\n  {}",
            entry.row,
            entry.quote,
            defects.join("\n  ")
        );

        for (index, want) in expected.iter().enumerate() {
            match want.parse::<T>() {
                Ok(parsed) => assert_eq!(
                    &parsed,
                    &values[index],
                    "{list}: spec/DATA_MODEL.md section 2 spells value {} of the {} row's field \
                     \"{want}\", and reading that spelling back gives another value",
                    index + 1,
                    entry.row
                ),
                Err(error) => panic!(
                    "{list}: spec/DATA_MODEL.md section 2 spells value {} of the {} row's field \
                     \"{want}\" and this crate refuses to read it: {error}",
                    index + 1,
                    entry.row
                ),
            }
        }

        list
    }

    #[test]
    fn ori_t_0019_the_value_lists_the_data_model_spells_carry_its_spellings() {
        // The floors. Each is a way for what follows to run over nothing and
        // report a pass, which is the class of defect this test was written to
        // remove, so each is a failure with its reason named instead.
        assert_eq!(
            SPELLED_BY_SECTION_2.len() + NOT_SPELLED_BY_SECTION_2.len(),
            10,
            "this crate declares ten value lists, nine wire_enum! invocations and \
             DocumentKind::FIXED, and each is either registered as spelled by \
             spec/DATA_MODEL.md section 2 or registered as not spelled by it. The ten are \
             counted by hand: a crate that may not do IO cannot read its own source to count \
             them, and nothing outside it counts them either"
        );
        assert_eq!(
            SPELLED_BY_SECTION_2.len(),
            6,
            "spec/DATA_MODEL.md section 2 writes six of this crate's ten value lists out as \
             values"
        );
        let pinned: usize = SPELLED_BY_SECTION_2
            .iter()
            .map(|entry| entry.spellings.len())
            .sum();
        assert_eq!(
            pinned, 38,
            "those six lists are thirty-eight spellings: 2 origins, 4 seats, 7 roles, 3 ticket \
             kinds, 3 document sets and 19 named document kinds"
        );
        let registered: BTreeSet<&str> = SPELLED_BY_SECTION_2
            .iter()
            .map(|entry| entry.list)
            .chain(NOT_SPELLED_BY_SECTION_2.iter().map(|(list, _)| *list))
            .collect();
        assert_eq!(
            registered.len(),
            10,
            "one list is registered twice, so the ten above stand for fewer than ten lists"
        );

        // The sentence and the list written under it are one restatement in two
        // halves. A half that drifts from the other is a restatement that has
        // stopped saying one thing, and it is caught here rather than carried
        // into the comparisons below.
        for entry in SPELLED_BY_SECTION_2 {
            let mut written: Vec<&str> = entry.spellings.to_vec();
            written.extend(entry.pattern);
            assert_eq!(
                values_in(entry.quote),
                written,
                "{}: the sentence restated from spec/DATA_MODEL.md section 2's {} row, \"{}\", \
                 and the spellings restated under it are not the same list",
                entry.list,
                entry.row,
                entry.quote
            );
        }

        let checked: BTreeSet<&str> = [
            assert_spelled_as_stated("ProductOrigin::ALL", ProductOrigin::ALL, |value| {
                value.as_str().to_owned()
            }),
            assert_spelled_as_stated("Seat::ALL", Seat::ALL, |value| value.as_str().to_owned()),
            assert_spelled_as_stated("Role::ALL", Role::ALL, |value| value.as_str().to_owned()),
            assert_spelled_as_stated("TicketKind::ALL", TicketKind::ALL, |value| {
                value.as_str().to_owned()
            }),
            assert_spelled_as_stated("DocumentSet::ALL", DocumentSet::ALL, |value| {
                value.as_str().to_owned()
            }),
            assert_spelled_as_stated("DocumentKind::FIXED", DocumentKind::FIXED, |value| {
                value.as_str().into_owned()
            }),
        ]
        .into_iter()
        .collect();
        let spelled: BTreeSet<&str> = SPELLED_BY_SECTION_2
            .iter()
            .map(|entry| entry.list)
            .collect();
        assert_eq!(
            checked, spelled,
            "a list registered as spelled by spec/DATA_MODEL.md section 2 that nothing above \
             compares with this crate is a sentence checked against nothing"
        );
    }

    #[test]
    fn ori_t_0019_the_as_built_family_carries_its_suffix() {
        let kind: DocumentKind = "as_built_architecture".parse().expect("the family parses");
        assert_eq!(kind, DocumentKind::AsBuilt("architecture".to_owned()));
        assert_eq!(kind.as_str(), "as_built_architecture");
        assert!(
            "as_built_".parse::<DocumentKind>().is_err(),
            "the prefix alone names no document"
        );
    }

    #[test]
    fn ori_t_0019_a_tier_ladder_orders_by_what_it_takes_to_merge() {
        assert!(Tier::Zero < Tier::One);
        assert!(Tier::One < Tier::Two);
    }

    #[test]
    fn ori_p1_033_a_value_outside_an_enumeration_is_refused_and_is_not_a_methodology_refusal() {
        for error in [
            "auto "
                .parse::<Category>()
                .expect_err("a trailing space is not the spelling"),
            "3".parse::<Tier>().expect_err("AICD §13 names no tier 3"),
            Tier::from_u8(3).expect_err("AICD §13 names no tier 3"),
            "reviewer"
                .parse::<Role>()
                .expect_err("AICD §7 names no reviewer role"),
            "operator"
                .parse::<Seat>()
                .expect_err("AICD §18 names no operator seat"),
        ] {
            assert!(!error.is_refusal());
            assert!(error.methodology_ref().is_none());
        }
    }
}
