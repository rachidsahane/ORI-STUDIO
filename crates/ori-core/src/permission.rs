//! The permission function: AICD §17.
//!
//! `spec/SECURITY_NOTES.md` "Authorization model" states the whole design in one
//! sentence: "Permissions are a function of (actor, role, resource, action)
//! evaluated in `ori-core` and enforced by the component that owns the resource
//! (broker for credentials, orchestrator for tickets and merges, memory for
//! retrieval, watch for the tree). No permission is expressed only in an
//! instruction file. The permission matrix of AICD §17 is the source;
//! `spec/ENV_SETUP.md` section 5 records the manifest per identity."
//!
//! [`permits`] is that function. It is pure, total and has no IO, which is what
//! `spec/LLD.md` section 2 requires of this crate. It decides; it enforces
//! nothing.
//!
//! ```mermaid
//! flowchart LR
//!   caller["the component that owns the resource"] -- "actor, role, resource, action" --> f["ori-core::permission::permits"]
//!   f -- "Allowed with the limit the cell names" --> enforce["the owner checks the limit it alone can check"]
//!   f -- "Refused, with AICD §17 as the reason" --> refuse["the owner refuses and records the event"]
//!   f -- "NotGoverned" --> human["not an agent identity: the human rules of AICD §18 apply, and this is not a grant"]
//! ```
//!
//! # Where the vocabulary comes from
//!
//! AICD §17's permission matrix is seven agent rows against six columns. The
//! columns are [`Resource`]; the verbs its cells use are [`Action`]; the
//! qualifiers its cells attach to those verbs ("own branch", "tier 0",
//! "telemetry only") are [`Constraint`]. Nothing here was invented: a test
//! requires every [`Action`] and every [`Constraint`] to be named by at least
//! one cell, and each grant carries the cell fragment it was read from.
//!
//! One resource is not a column of that matrix, [`Resource::Secrets`]. It is
//! required by criterion ORI-P1-019 in `spec/criteria/phase-1.md` and by AICD
//! §27's secrets architecture, and no role holds any action on it, so adding it
//! widens nothing.
//!
//! # What this module decides and what it cannot
//!
//! It answers one question: does the matrix grant this action on this resource
//! to an identity of this role. It does not know which branch is whose, which
//! ticket the actor filed, or what tier a pull request carries, so a cell that
//! says "own branch" is answered with [`Decision::Allowed`] carrying
//! [`Constraint::Own`] and the owning component checks the ownership. That
//! split is `spec/SECURITY_NOTES.md`'s "evaluated in `ori-core` and enforced by
//! the component that owns the resource", read literally.
//!
//! It also does not issue, hold or revoke anything. A grant here is not a
//! credential: `spec/ENV_SETUP.md` section 5 records what each identity is
//! issued, the broker issues it, and an identity can be refused here and still
//! hold no credential for the same action, which is criterion ORI-P1-018's
//! shape exactly.
//!
//! # Why no `Error` and no `RefusalKind`
//!
//! [`crate::error::RefusalKind`] carries no permission refusal, and
//! `crates/ori-core/src/error.rs` is outside this ticket's declared scope, so
//! none could be added. That turned out not to be a workaround: CLAUDE.md rule 9
//! asks that every refusal carry a [`MethodologyRef`], and every
//! [`Decision::Refused`] carries one, read back by
//! [`Decision::methodology_ref`] under the name
//! [`crate::error::Error`] gives the same reader. What is missing is only the
//! bridge for a caller that wants a `Result`, and the ticket that adds it is
//! named in this branch's report.
//!
//! Must not: do IO, or import any other workspace crate (`spec/LLD.md` section
//! 2). Nothing here reads a file, a clock or an environment variable, and
//! nothing here reads a keychain or a `.env` file: [`is_secret_location`]
//! decides about a path as a value and never opens it.

use core::fmt;

use crate::error::MethodologyRef;
use crate::types::Actor;
use crate::types::Role;

/// The methodology section the permission matrix is: AICD §17.
const PERMISSION_MATRIX: u8 = 17;

/// The methodology section the secrets architecture is: AICD §27.
const SECRETS_ARCHITECTURE: u8 = 27;

// ---------------------------------------------------------------------------
// The vocabulary
// ---------------------------------------------------------------------------

/// What a permission is held over: AICD §17.
///
/// Derived from the columns of AICD §17's permission matrix, which are "Code
/// repositories", "CI", "Staging", "Production", "Tickets" and "Specification",
/// plus [`Resource::Secrets`], which that matrix has no column for and
/// `spec/criteria/phase-1.md`'s ORI-P1-019 requires.
///
/// The list is the matrix's columns and nothing else. A finer resource, a
/// single file or a single ticket, is the owning component's business: this
/// type is the axis the matrix is written on.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Resource {
    /// The code repositories of the product.
    CodeRepository,
    /// Continuous integration.
    Ci,
    /// The staging environment.
    Staging,
    /// Production.
    Production,
    /// The ticket queue.
    Tickets,
    /// The specification under `spec/`.
    Specification,
    /// The OS keychain and any `.env*` file.
    ///
    /// Not a column of AICD §17. It is here because criterion ORI-P1-019 needs
    /// the decision "no agent may read the OS keychain or a `.env*` file" to
    /// exist somewhere, and because AICD §27's secrets architecture states the
    /// rule it expresses: "A single secrets manager is the only place
    /// credentials live. Nothing in repositories, prompts, instruction files or
    /// memory." `spec/SECURITY_NOTES.md` "Secrets" names the one component that
    /// may touch it, and that component is not an agent.
    ///
    /// Every role is refused every action on it, so this value grants nothing
    /// to anybody.
    Secrets,
}

impl Resource {
    /// Every resource, in the order AICD §17's columns run, with
    /// [`Resource::Secrets`] last because it is not one of them.
    pub const ALL: &'static [Self] = &[
        Self::CodeRepository,
        Self::Ci,
        Self::Staging,
        Self::Production,
        Self::Tickets,
        Self::Specification,
        Self::Secrets,
    ];

    /// The resource as AICD §17 heads its column, lower cased.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodeRepository => "code repositories",
            Self::Ci => "ci",
            Self::Staging => "staging",
            Self::Production => "production",
            Self::Tickets => "tickets",
            Self::Specification => "specification",
            Self::Secrets => "secrets",
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What may be done to a resource: AICD §17.
///
/// Derived from the verbs AICD §17's matrix cells use, and from no other
/// source. Thirteen verbs appear across the forty-two cells; each value below is
/// one of them, and a test requires every value to be named by at least one
/// cell. A verb no cell uses would be a capability nobody granted.
///
/// The matrix grants nothing by implication. AICD §17's principles close the
/// question for every pair its cells do not name: "Humans hold the permissions
/// that agents do not: tier 1 and 2 merges, permission changes, runbook
/// approval, and anything not explicitly granted." So [`Action::Run`] on
/// [`Resource::Ci`] for a coder does not carry [`Action::Read`] with it, and
/// nothing here reads one verb into another.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Action {
    /// Read the resource.
    Read,
    /// Write to the resource.
    Write,
    /// Run the resource, which for CI is starting a run.
    Run,
    /// Deploy to the resource.
    Deploy,
    /// Merge a pull request.
    Merge,
    /// Comment on a ticket.
    Comment,
    /// Create a ticket.
    Create,
    /// Assign a ticket.
    Assign,
    /// Raise a ticket's category, which AICD §11 allows in one direction only.
    UpgradeCategory,
    /// Propose a change to the specification, which reaches a human.
    Propose,
    /// Draft, which reaches nobody until a human takes it up.
    Draft,
    /// Roll back a deployment.
    Rollback,
    /// Execute a runbook.
    ExecuteRunbook,
}

impl Action {
    /// Every action, in the order the matrix's verbs first appear.
    pub const ALL: &'static [Self] = &[
        Self::Read,
        Self::Write,
        Self::Run,
        Self::Deploy,
        Self::Merge,
        Self::Comment,
        Self::Create,
        Self::Assign,
        Self::UpgradeCategory,
        Self::Propose,
        Self::Draft,
        Self::Rollback,
        Self::ExecuteRunbook,
    ];

    /// The word AICD §17's cells write this action with, lower cased.
    ///
    /// This is what ties a value of this enum to the cell it was read from: a
    /// grant's fragment must begin with this word, which a test checks for all
    /// forty-two grants.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Run => "run",
            Self::Deploy => "deploy",
            Self::Merge => "merge",
            Self::Comment => "comment",
            Self::Create => "create",
            Self::Assign => "assign",
            Self::UpgradeCategory => "upgrade category",
            Self::Propose => "propose",
            Self::Draft => "draft",
            Self::Rollback => "rollback",
            Self::ExecuteRunbook => "runbooks",
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The limit a matrix cell puts on a verb it grants: AICD §17.
///
/// Derived from the qualifiers AICD §17's cells attach to their verbs, and from
/// no other source: "write own branch", "merge tier 0", "read telemetry only"
/// and the rest. Twelve qualifiers appear across the forty-two grants; each
/// value below is one of them, and a test requires every value to be named by at
/// least one cell.
///
/// A limit is returned, never taken. None of these can be checked without
/// knowing a fact this crate does not hold (which branch belongs to whom, what
/// tier a pull request carries, what kind a ticket is), and
/// `spec/SECURITY_NOTES.md` "Authorization model" gives that check to the
/// component that owns the resource. Deciding here and enforcing there is the
/// design; deciding here on a fact the caller asserted would be the caller
/// deciding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Constraint {
    /// Only what belongs to this actor: its own branch, its own ticket.
    Own,
    /// Only a branch that carries a specification change.
    SpecBranch,
    /// Only a change at risk tier 0, which is the tier AICD §13 lets an agent
    /// merge.
    TierZero,
    /// Only telemetry, not production itself.
    Telemetry,
    /// Only analytics, not production itself.
    Analytics,
    /// Only from a release tag.
    FromTags,
    /// Only an incident ticket.
    IncidentTicket,
    /// Only a product signal ticket.
    ProductSignalTicket,
    /// Only acceptance criteria.
    Criteria,
    /// Only through a pull request, which is what puts a human in the path.
    ViaPullRequest,
    /// Only the runbooks.
    Runbooks,
    /// Only the product brief.
    ProductBrief,
}

impl Constraint {
    /// Every limit, in the order the matrix's qualifiers first appear.
    pub const ALL: &'static [Self] = &[
        Self::Own,
        Self::SpecBranch,
        Self::TierZero,
        Self::Telemetry,
        Self::Analytics,
        Self::FromTags,
        Self::IncidentTicket,
        Self::ProductSignalTicket,
        Self::Criteria,
        Self::ViaPullRequest,
        Self::Runbooks,
        Self::ProductBrief,
    ];

    /// The words AICD §17's cells write this limit with, lower cased.
    ///
    /// This is what ties a value of this enum to the cell it was read from: the
    /// part of a grant's fragment after its verb must contain these words when
    /// the grant carries this limit, and must contain no limit's words when it
    /// carries none. A test checks both for all forty-two grants.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::SpecBranch => "spec branches",
            Self::TierZero => "tier 0",
            Self::Telemetry => "telemetry",
            Self::Analytics => "analytics",
            Self::FromTags => "from tags",
            Self::IncidentTicket => "incident",
            Self::ProductSignalTicket => "product signal",
            Self::Criteria => "criteria",
            Self::ViaPullRequest => "via pull request",
            Self::Runbooks => "runbooks",
            Self::ProductBrief => "product brief",
        }
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// The matrix
// ---------------------------------------------------------------------------

/// One verb a matrix cell grants, with the limit the cell puts on it and the
/// fragment of the cell it was read from: AICD §17.
///
/// The fragment is carried so that the typed grant can be checked against the
/// prose it was transcribed from rather than against a second copy of itself.
/// It is also what the "explain" action of PRD A-10 has to show a human: the
/// words the methodology actually used.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Grant {
    /// The verb the cell grants.
    pub action: Action,
    /// The limit the cell puts on it, absent when the cell puts none.
    pub limit: Option<Constraint>,
    /// The fragment of the cell this grant was read from, as AICD §17 writes it.
    pub fragment: &'static str,
}

/// One cell of the matrix: one role against one resource.
///
/// Private: the two things a caller needs from a cell are [`grants`] and
/// [`cell`], and a constructible cell would be a matrix a caller could write.
struct Cell {
    role: Role,
    resource: Resource,
    text: &'static str,
    grants: &'static [Grant],
}

/// Writes one cell in the shape AICD §17 prints it, so that the table below can
/// be read against the document row by row.
///
/// No methodology section applies: this is a local device for keeping the table
/// readable, not an implementation of a methodology rule.
macro_rules! cell {
    ($role:ident, $resource:ident, $text:literal, [ $( $action:ident $(: $limit:ident)? => $fragment:literal ),* $(,)? ]) => {
        Cell {
            role: Role::$role,
            resource: Resource::$resource,
            text: $text,
            grants: &[ $( Grant {
                action: Action::$action,
                limit: cell!(@limit $($limit)?),
                fragment: $fragment,
            } ),* ],
        }
    };
    (@limit) => { None };
    (@limit $limit:ident) => { Some(Constraint::$limit) };
}

/// AICD §17's permission matrix, transcribed cell by cell, plus the secrets
/// column no role holds anything on.
///
/// Forty-nine cells: the seven roles of [`Role`] against the seven values of
/// [`Resource`]. Forty-two of them are AICD §17's, and the `text` of each is
/// that cell verbatim. The seven [`Resource::Secrets`] cells are "None", which
/// is what AICD §27 and criterion ORI-P1-019 make them.
///
/// The deliberate omissions are two. `spec/ENV_SETUP.md` section 5's manifest
/// lists two identities this table has no row for, `implementor (bootstrap
/// only)` and `operator (human)`; neither is a value of [`Role`], the operator
/// is not an agent, and AICD §17 has a row for neither.
const MATRIX: &[Cell] = &[
    // Coder: AICD §7 gives it "Its own branch only. Never main, never
    // production."
    cell!(Coder, CodeRepository, "Read; write own branch", [
        Read => "Read",
        Write: Own => "write own branch",
    ]),
    cell!(Coder, Ci, "Run", [Run => "Run"]),
    cell!(Coder, Staging, "Deploy own branch", [Deploy: Own => "Deploy own branch"]),
    cell!(Coder, Production, "None", []),
    cell!(Coder, Tickets, "Read own; comment", [
        Read: Own => "Read own",
        Comment => "comment",
    ]),
    cell!(Coder, Specification, "Read", [Read => "Read"]),
    cell!(Coder, Secrets, "None", []),
    // Lead / Reviewer.
    cell!(Lead, CodeRepository, "Read all; merge tier 0", [
        Read => "Read all",
        Merge: TierZero => "merge tier 0",
    ]),
    cell!(Lead, Ci, "Read", [Read => "Read"]),
    cell!(Lead, Staging, "Read", [Read => "Read"]),
    cell!(Lead, Production, "None", []),
    cell!(Lead, Tickets, "Read, assign, upgrade category", [
        Read => "Read",
        Assign => "assign",
        UpgradeCategory => "upgrade category",
    ]),
    cell!(Lead, Specification, "Read", [Read => "Read"]),
    cell!(Lead, Secrets, "None", []),
    // QA: AICD §7 gives it "Staging environment, tickets. No code
    // repositories."
    cell!(Qa, CodeRepository, "None", []),
    cell!(Qa, Ci, "Read", [Read => "Read"]),
    cell!(Qa, Staging, "Read, write", [Read => "Read", Write => "write"]),
    cell!(Qa, Production, "Read telemetry only", [Read: Telemetry => "Read telemetry only"]),
    cell!(Qa, Tickets, "Create, read", [Create => "Create", Read => "read"]),
    cell!(Qa, Specification, "Read; propose criteria", [
        Read => "Read",
        Propose: Criteria => "propose criteria",
    ]),
    cell!(Qa, Secrets, "None", []),
    // Operations: AICD §7 gives it "Deployments, rollbacks, infrastructure
    // within runbooks. No code changes."
    cell!(Operations, CodeRepository, "None", []),
    cell!(Operations, Ci, "Read", [Read => "Read"]),
    cell!(Operations, Staging, "Read, write", [Read => "Read", Write => "write"]),
    cell!(Operations, Production, "Deploy from tags, rollback, runbooks", [
        Deploy: FromTags => "Deploy from tags",
        Rollback => "rollback",
        ExecuteRunbook => "runbooks",
    ]),
    cell!(Operations, Tickets, "Create incident, read", [
        Create: IncidentTicket => "Create incident",
        Read => "read",
    ]),
    cell!(Operations, Specification, "Read runbooks", [Read: Runbooks => "Read runbooks"]),
    cell!(Operations, Secrets, "None", []),
    // Documentation.
    cell!(Documentation, CodeRepository, "Read; write spec branches", [
        Read => "Read",
        Write: SpecBranch => "write spec branches",
    ]),
    cell!(Documentation, Ci, "None", []),
    cell!(Documentation, Staging, "None", []),
    cell!(Documentation, Production, "None", []),
    cell!(Documentation, Tickets, "Read", [Read => "Read"]),
    cell!(Documentation, Specification, "Propose via pull request", [
        Propose: ViaPullRequest => "Propose via pull request",
    ]),
    cell!(Documentation, Secrets, "None", []),
    // Product signal: AICD §7 gives it "Never files tickets for coders."
    cell!(ProductSignal, CodeRepository, "None", []),
    cell!(ProductSignal, Ci, "None", []),
    cell!(ProductSignal, Staging, "None", []),
    cell!(ProductSignal, Production, "Read analytics only", [
        Read: Analytics => "Read analytics only",
    ]),
    cell!(ProductSignal, Tickets, "Create product signal", [
        Create: ProductSignalTicket => "Create product signal",
    ]),
    cell!(ProductSignal, Specification, "Read product brief", [
        Read: ProductBrief => "Read product brief",
    ]),
    cell!(ProductSignal, Secrets, "None", []),
    // Assistant: AICD §7 gives it "Drafts only".
    cell!(Assistant, CodeRepository, "Read", [Read => "Read"]),
    cell!(Assistant, Ci, "None", []),
    cell!(Assistant, Staging, "None", []),
    cell!(Assistant, Production, "None", []),
    cell!(Assistant, Tickets, "Read", [Read => "Read"]),
    cell!(Assistant, Specification, "Draft only", [Draft => "Draft only"]),
    cell!(Assistant, Secrets, "None", []),
];

/// The cell for one role and one resource, or `None` when the table has no row
/// for the pair.
///
/// No methodology section applies: this is the lookup behind [`permits`]. A
/// missing row cannot happen, and a test enumerates all forty-nine pairs to keep
/// it that way, but a lookup that failed would leave the caller with no grants
/// and therefore a refusal, which is the direction a permission function is
/// allowed to be wrong in.
fn lookup(role: Role, resource: Resource) -> Option<&'static Cell> {
    MATRIX
        .iter()
        .find(|cell| cell.role == role && cell.resource == resource)
}

/// Everything AICD §17's matrix grants this role on this resource, which is
/// empty for every cell that reads "None".
#[must_use]
pub fn grants(role: Role, resource: Resource) -> &'static [Grant] {
    match lookup(role, resource) {
        Some(cell) => cell.grants,
        None => &[],
    }
}

/// The cell of AICD §17's matrix for this role and this resource, verbatim.
///
/// This is what the "explain" action of PRD A-10 shows a human beside a
/// refusal: not a paraphrase of the rule but the row of the table the decision
/// came from.
#[must_use]
pub fn cell(role: Role, resource: Resource) -> &'static str {
    match lookup(role, resource) {
        Some(cell) => cell.text,
        None => "None",
    }
}

// ---------------------------------------------------------------------------
// The decision
// ---------------------------------------------------------------------------

/// What the permission function answers: AICD §17.
///
/// Three values, because the honest answer has three shapes and a `bool` has
/// two. AICD §17's matrix grants some cells outright, grants others within a
/// limit only the owning component can check, and governs agent identities
/// only: "Humans hold the permissions that agents do not: tier 1 and 2 merges,
/// permission changes, runbook approval, and anything not explicitly granted."
///
/// A `Result` would have been the house shape, but expressing a refusal as
/// [`crate::error::Error`] needs a [`crate::error::RefusalKind`] variant that
/// does not exist and that this ticket's declared scope did not include. Nothing
/// is lost by the shape: the refusal carries the [`MethodologyRef`] that
/// CLAUDE.md rule 9 and criterion ORI-P1-033 require, and
/// [`Decision::methodology_ref`] and [`Decision::is_refusal`] are the same two
/// readers [`crate::error::Error`] offers, spelled the same way.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[must_use]
pub enum Decision {
    /// The matrix grants it, within the limit the cell names.
    Allowed {
        /// The limit the cell puts on the grant, absent when it puts none. The
        /// component that owns the resource checks it; this crate cannot.
        limit: Option<Constraint>,
    },
    /// The matrix does not grant it, under the section [`refusal_for`] names
    /// for the resource.
    Refused {
        /// The section this refusal is made under, which is AICD §17 for every
        /// resource of the matrix and AICD §27 for the secrets that are not on
        /// it.
        reason: MethodologyRef,
    },
    /// AICD §17's matrix does not govern this actor, because the matrix is a
    /// table of agent roles and this actor is not an agent identity.
    ///
    /// This is not a grant and must not be read as one. A human holds what
    /// AICD §18's seats hold and the operator row of `spec/ENV_SETUP.md`
    /// section 5 records; the engine acting as [`Actor::System`] is the trusted
    /// component of `spec/SECURITY_NOTES.md` "Auto mode and where control
    /// lives". Neither question is this table's, and answering
    /// [`Decision::Refused`] for them would refuse the operator their own
    /// product while answering [`Decision::Allowed`] would grant what nobody
    /// checked. [`Decision::is_allowed`] is false for this value.
    NotGoverned,
}

impl Decision {
    /// The methodology section this decision was made under, which every
    /// refusal carries and nothing else does.
    ///
    /// The shape is [`crate::error::Error::methodology_ref`]'s, for the reason
    /// that module gives: a refusal is a control refusing an action and always
    /// names the rule, and everything else would put a reference in front of a
    /// human that explains nothing. CLAUDE.md rule 9 and criterion ORI-P1-033
    /// are what require the first half.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Refused { reason } => Some(reason.clone()),
            Self::Allowed { .. } | Self::NotGoverned => None,
        }
    }

    /// Whether the matrix refused the action, as opposed to granting it or not
    /// governing the actor at all.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// Whether the action is granted.
    ///
    /// True for [`Decision::Allowed`] alone. [`Decision::NotGoverned`] is not a
    /// grant, and a caller that tests for "not refused" instead of this has
    /// granted every human and the engine itself everything.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    /// The limit the granting cell names, absent when there is none and when
    /// nothing was granted.
    #[must_use]
    pub const fn limit(&self) -> Option<Constraint> {
        match self {
            Self::Allowed { limit } => *limit,
            Self::Refused { .. } | Self::NotGoverned => None,
        }
    }
}

/// The section a refusal on this resource is made under: AICD §17, AICD §27.
///
/// Every resource of AICD §17's matrix refuses under AICD §17.
/// [`Resource::Secrets`] is not one of its columns and refuses under AICD §27's
/// secrets architecture, "A single secrets manager is the only place credentials
/// live", which is the rule criterion ORI-P1-019 tests.
#[must_use]
pub fn refusal_for(resource: Resource) -> MethodologyRef {
    let section = match resource {
        Resource::Secrets => SECRETS_ARCHITECTURE,
        _ => PERMISSION_MATRIX,
    };
    MethodologyRef {
        section,
        subsection: None,
    }
}

/// Whether this actor, holding this role, may take this action on this
/// resource: AICD §17.
///
/// This is `spec/SECURITY_NOTES.md` "Authorization model"'s function of
/// "(actor, role, resource, action) evaluated in `ori-core`", and AICD §17's
/// matrix is what it evaluates.
///
/// # How the four arguments are used
///
/// `actor` decides whether the matrix applies at all. AICD §17's rows are agent
/// roles, so an [`Actor::Agent`] is decided by the table and any other actor is
/// [`Decision::NotGoverned`]. `role` chooses the row, `resource` the column, and
/// `action` the verb inside the cell.
///
/// The pairing of `actor` and `role` is the caller's to get right: which role an
/// agent identity holds is a field of `AgentIdentity` in `spec/DATA_MODEL.md`
/// section 2, which lives in `ori-store` and which this crate cannot read. A
/// caller that passes an identity with a role it does not hold is answered about
/// the role it passed.
///
/// # What a grant is not
///
/// It is not a credential. `spec/ENV_SETUP.md` section 5 records what each
/// identity is issued and the broker issues it; a role can be granted an action
/// here and hold nothing that could perform it. It is also not the last check: a
/// grant carrying a [`Constraint`] is granted only within that limit, and the
/// component that owns the resource is the one that can check it.
pub fn permits(actor: &Actor, role: Role, resource: Resource, action: Action) -> Decision {
    match actor {
        Actor::Agent(_) => (),
        // AICD §17: "Humans hold the permissions that agents do not". The engine
        // itself is `spec/SECURITY_NOTES.md`'s trusted component. Neither is a
        // row of this table.
        Actor::Human(_) | Actor::System => return Decision::NotGoverned,
    }
    match grants(role, resource)
        .iter()
        .find(|grant| grant.action == action)
    {
        Some(grant) => Decision::Allowed { limit: grant.limit },
        None => Decision::Refused {
            reason: refusal_for(resource),
        },
    }
}

// ---------------------------------------------------------------------------
// Secret locations
// ---------------------------------------------------------------------------

/// The prefix `spec/CONVENTIONS.md` and `spec/ENV_SETUP.md` section 6 write the
/// forbidden file as, `.env*`.
const ENV_PREFIX: &str = ".env";

/// Whether a path names a place credentials live: AICD §27.
///
/// Derived from AICD §27's secrets architecture, "A single secrets manager is
/// the only place credentials live. Nothing in repositories, prompts,
/// instruction files or memory", and from the one forbidden action
/// `spec/ENV_SETUP.md` section 6 puts on every identity: "Read the keychain;
/// read `.env*`". Criterion ORI-P1-019 is the test of it.
///
/// True when any component of the path is named `.env` or begins with it, so
/// that `.env`, `.env.local`, `crates/x/.env` and anything under a `.env`
/// directory all answer true. Answering true for a directory as well as a file
/// is deliberate: the cost of being wrong in that direction is a refusal, and
/// the cost of being wrong in the other is a leaked credential.
///
/// The comparison ignores ASCII case for the same reason. The default file
/// systems of two of this product's three platforms are case insensitive, so
/// `.ENV` and `.env` are one file there, and a reader that matched only the
/// lower-case spelling would refuse one name and allow the other name of the
/// same bytes. Every spelling of the prefix is refused on every platform
/// instead, which costs a refusal on a case-sensitive file system where a file
/// genuinely named `.ENV` is a different file, and that is the direction this
/// function is allowed to be wrong in.
///
/// # What this does not do
///
/// It does not canonicalize, resolve a symbolic link, or touch the filesystem.
/// This crate may not (`spec/LLD.md` section 2), and the path a caller passes is
/// already canonicalized and checked against the product root by the caller:
/// `spec/SECURITY_NOTES.md` "Input validation" puts that where the IO is, and
/// criterion ORI-P1-023 is its test. A path that escapes the root is refused
/// before it reaches this function.
///
/// It reads bytes, not intentions. A percent-encoded, unicode-normalized or
/// homoglyph spelling of the prefix is not recognized, and neither is a name a
/// file system would fold onto `.env` by a rule wider than ASCII case. What
/// would settle those is a generator over paths rather than a list of them, and
/// `spec/ROADMAP.md`'s property testing is not available: escalation E-0004 is
/// open with the operator.
///
/// It says nothing about the OS keychain, which is not a path.
/// [`Resource::Secrets`] is the keychain, and every role is refused every action
/// on it.
#[must_use]
pub fn is_secret_location(path: &str) -> bool {
    path.split(['/', '\\']).any(|component| {
        let component = component.as_bytes();
        let prefix = ENV_PREFIX.as_bytes();
        component.len() >= prefix.len() && component[..prefix.len()].eq_ignore_ascii_case(prefix)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ULID that parses, for an agent identity no test cares about.
    const SAMPLE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// An agent actor, which is the only kind AICD §17's matrix governs.
    fn agent() -> Actor {
        Actor::Agent(crate::types::Id::parse(SAMPLE_ULID).expect("a well formed ULID parses"))
    }

    /// AICD §17's permission matrix as the methodology prints it, transcribed
    /// from `methodology/AICD_Methodology_v0.3.html`, plus the secrets column.
    ///
    /// This is the second copy, and it is the point: the implementation holds
    /// the matrix as typed cells, this holds it as the document's own prose, and
    /// a defect in either fails a test rather than agreeing with itself.
    const CELLS: &[(Role, Resource, &str)] = &[
        (
            Role::Coder,
            Resource::CodeRepository,
            "Read; write own branch",
        ),
        (Role::Coder, Resource::Ci, "Run"),
        (Role::Coder, Resource::Staging, "Deploy own branch"),
        (Role::Coder, Resource::Production, "None"),
        (Role::Coder, Resource::Tickets, "Read own; comment"),
        (Role::Coder, Resource::Specification, "Read"),
        (Role::Coder, Resource::Secrets, "None"),
        (
            Role::Lead,
            Resource::CodeRepository,
            "Read all; merge tier 0",
        ),
        (Role::Lead, Resource::Ci, "Read"),
        (Role::Lead, Resource::Staging, "Read"),
        (Role::Lead, Resource::Production, "None"),
        (
            Role::Lead,
            Resource::Tickets,
            "Read, assign, upgrade category",
        ),
        (Role::Lead, Resource::Specification, "Read"),
        (Role::Lead, Resource::Secrets, "None"),
        (Role::Qa, Resource::CodeRepository, "None"),
        (Role::Qa, Resource::Ci, "Read"),
        (Role::Qa, Resource::Staging, "Read, write"),
        (Role::Qa, Resource::Production, "Read telemetry only"),
        (Role::Qa, Resource::Tickets, "Create, read"),
        (Role::Qa, Resource::Specification, "Read; propose criteria"),
        (Role::Qa, Resource::Secrets, "None"),
        (Role::Operations, Resource::CodeRepository, "None"),
        (Role::Operations, Resource::Ci, "Read"),
        (Role::Operations, Resource::Staging, "Read, write"),
        (
            Role::Operations,
            Resource::Production,
            "Deploy from tags, rollback, runbooks",
        ),
        (Role::Operations, Resource::Tickets, "Create incident, read"),
        (Role::Operations, Resource::Specification, "Read runbooks"),
        (Role::Operations, Resource::Secrets, "None"),
        (
            Role::Documentation,
            Resource::CodeRepository,
            "Read; write spec branches",
        ),
        (Role::Documentation, Resource::Ci, "None"),
        (Role::Documentation, Resource::Staging, "None"),
        (Role::Documentation, Resource::Production, "None"),
        (Role::Documentation, Resource::Tickets, "Read"),
        (
            Role::Documentation,
            Resource::Specification,
            "Propose via pull request",
        ),
        (Role::Documentation, Resource::Secrets, "None"),
        (Role::ProductSignal, Resource::CodeRepository, "None"),
        (Role::ProductSignal, Resource::Ci, "None"),
        (Role::ProductSignal, Resource::Staging, "None"),
        (
            Role::ProductSignal,
            Resource::Production,
            "Read analytics only",
        ),
        (
            Role::ProductSignal,
            Resource::Tickets,
            "Create product signal",
        ),
        (
            Role::ProductSignal,
            Resource::Specification,
            "Read product brief",
        ),
        (Role::ProductSignal, Resource::Secrets, "None"),
        (Role::Assistant, Resource::CodeRepository, "Read"),
        (Role::Assistant, Resource::Ci, "None"),
        (Role::Assistant, Resource::Staging, "None"),
        (Role::Assistant, Resource::Production, "None"),
        (Role::Assistant, Resource::Tickets, "Read"),
        (Role::Assistant, Resource::Specification, "Draft only"),
        (Role::Assistant, Resource::Secrets, "None"),
    ];

    /// Every triple AICD §17's matrix grants, read off the same transcription
    /// and typed independently of the implementation's table.
    ///
    /// Forty-two rows, one per verb in a cell. Everything not here is refused,
    /// which is AICD §17's "anything not explicitly granted" read as code.
    const GRANTED: &[(Role, Resource, Action, Option<Constraint>)] = &[
        (Role::Coder, Resource::CodeRepository, Action::Read, None),
        (
            Role::Coder,
            Resource::CodeRepository,
            Action::Write,
            Some(Constraint::Own),
        ),
        (Role::Coder, Resource::Ci, Action::Run, None),
        (
            Role::Coder,
            Resource::Staging,
            Action::Deploy,
            Some(Constraint::Own),
        ),
        (
            Role::Coder,
            Resource::Tickets,
            Action::Read,
            Some(Constraint::Own),
        ),
        (Role::Coder, Resource::Tickets, Action::Comment, None),
        (Role::Coder, Resource::Specification, Action::Read, None),
        (Role::Lead, Resource::CodeRepository, Action::Read, None),
        (
            Role::Lead,
            Resource::CodeRepository,
            Action::Merge,
            Some(Constraint::TierZero),
        ),
        (Role::Lead, Resource::Ci, Action::Read, None),
        (Role::Lead, Resource::Staging, Action::Read, None),
        (Role::Lead, Resource::Tickets, Action::Read, None),
        (Role::Lead, Resource::Tickets, Action::Assign, None),
        (Role::Lead, Resource::Tickets, Action::UpgradeCategory, None),
        (Role::Lead, Resource::Specification, Action::Read, None),
        (Role::Qa, Resource::Ci, Action::Read, None),
        (Role::Qa, Resource::Staging, Action::Read, None),
        (Role::Qa, Resource::Staging, Action::Write, None),
        (
            Role::Qa,
            Resource::Production,
            Action::Read,
            Some(Constraint::Telemetry),
        ),
        (Role::Qa, Resource::Tickets, Action::Create, None),
        (Role::Qa, Resource::Tickets, Action::Read, None),
        (Role::Qa, Resource::Specification, Action::Read, None),
        (
            Role::Qa,
            Resource::Specification,
            Action::Propose,
            Some(Constraint::Criteria),
        ),
        (Role::Operations, Resource::Ci, Action::Read, None),
        (Role::Operations, Resource::Staging, Action::Read, None),
        (Role::Operations, Resource::Staging, Action::Write, None),
        (
            Role::Operations,
            Resource::Production,
            Action::Deploy,
            Some(Constraint::FromTags),
        ),
        (
            Role::Operations,
            Resource::Production,
            Action::Rollback,
            None,
        ),
        (
            Role::Operations,
            Resource::Production,
            Action::ExecuteRunbook,
            None,
        ),
        (
            Role::Operations,
            Resource::Tickets,
            Action::Create,
            Some(Constraint::IncidentTicket),
        ),
        (Role::Operations, Resource::Tickets, Action::Read, None),
        (
            Role::Operations,
            Resource::Specification,
            Action::Read,
            Some(Constraint::Runbooks),
        ),
        (
            Role::Documentation,
            Resource::CodeRepository,
            Action::Read,
            None,
        ),
        (
            Role::Documentation,
            Resource::CodeRepository,
            Action::Write,
            Some(Constraint::SpecBranch),
        ),
        (Role::Documentation, Resource::Tickets, Action::Read, None),
        (
            Role::Documentation,
            Resource::Specification,
            Action::Propose,
            Some(Constraint::ViaPullRequest),
        ),
        (
            Role::ProductSignal,
            Resource::Production,
            Action::Read,
            Some(Constraint::Analytics),
        ),
        (
            Role::ProductSignal,
            Resource::Tickets,
            Action::Create,
            Some(Constraint::ProductSignalTicket),
        ),
        (
            Role::ProductSignal,
            Resource::Specification,
            Action::Read,
            Some(Constraint::ProductBrief),
        ),
        (
            Role::Assistant,
            Resource::CodeRepository,
            Action::Read,
            None,
        ),
        (Role::Assistant, Resource::Tickets, Action::Read, None),
        (
            Role::Assistant,
            Resource::Specification,
            Action::Draft,
            None,
        ),
    ];

    /// The refusal the methodology makes on this resource, built from the
    /// sections the documents name rather than from the implementation's own
    /// constants: AICD §17 for the matrix's own columns, AICD §27's secrets
    /// architecture for the keychain and the `.env*` files, which are not one.
    fn refused(resource: Resource) -> Decision {
        let section = if resource == Resource::Secrets {
            27
        } else {
            17
        };
        Decision::Refused {
            reason: MethodologyRef {
                section,
                subsection: None,
            },
        }
    }

    /// What [`GRANTED`] says about one triple.
    fn expected(role: Role, resource: Resource, action: Action) -> Decision {
        match GRANTED
            .iter()
            .find(|(r, res, a, _)| *r == role && *res == resource && *a == action)
        {
            Some((_, _, _, limit)) => Decision::Allowed { limit: *limit },
            None => refused(resource),
        }
    }

    /// The fragments of one cell, as AICD §17 separates them, and empty for a
    /// cell that reads "None".
    fn fragments(text: &str) -> Vec<String> {
        if text == "None" {
            return Vec::new();
        }
        text.split([';', ','])
            .map(|fragment| fragment.trim().to_owned())
            .filter(|fragment| !fragment.is_empty())
            .collect()
    }

    /// Reports the first `limit` of a list of findings and how many there were,
    /// which is what makes a failure readable when a defect breaks hundreds of
    /// cells at once.
    fn report(findings: &[String], limit: usize) -> String {
        let mut out = format!("{} of them:\n", findings.len());
        for finding in findings.iter().take(limit) {
            out.push_str("  ");
            out.push_str(finding);
            out.push('\n');
        }
        if findings.len() > limit {
            out.push_str(&format!("  and {} more\n", findings.len() - limit));
        }
        out
    }

    #[test]
    fn ori_t_0022_the_table_has_one_cell_for_every_role_and_every_resource() {
        assert_eq!(
            MATRIX.len(),
            Role::ALL.len() * Resource::ALL.len(),
            "the table is seven roles against seven resources"
        );
        for role in Role::ALL {
            for resource in Resource::ALL {
                let found = MATRIX
                    .iter()
                    .filter(|cell| cell.role == *role && cell.resource == *resource)
                    .count();
                assert_eq!(found, 1, "cells for {role} against {resource}");
            }
        }
    }

    #[test]
    fn ori_t_0022_every_cell_is_the_one_aicd_17_prints() {
        assert_eq!(
            CELLS.len(),
            MATRIX.len(),
            "the transcription and the table are different sizes"
        );
        for (role, resource, text) in CELLS {
            assert_eq!(
                cell(*role, *resource),
                *text,
                "the cell for {role} against {resource}"
            );
        }
    }

    #[test]
    fn ori_t_0022_every_grant_is_described_by_the_cell_fragment_it_came_from() {
        for (role, resource, text) in CELLS {
            let fragments = fragments(text);
            let grants = grants(*role, *resource);
            assert_eq!(
                grants.len(),
                fragments.len(),
                "{role} against {resource}: the cell reads {text:?}"
            );
            for (grant, fragment) in grants.iter().zip(fragments.iter()) {
                assert_eq!(
                    grant.fragment, fragment,
                    "{role} against {resource}: the grant cites a fragment the cell does not have"
                );
                let lowered = fragment.to_lowercase();
                let verb = grant.action.as_str();
                assert!(
                    lowered.starts_with(verb)
                        && (lowered.len() == verb.len() || lowered.as_bytes()[verb.len()] == b' '),
                    "{role} against {resource}: {fragment:?} does not begin with the verb {verb:?}"
                );
                let rest = lowered[verb.len()..].trim();
                for limit in Constraint::ALL {
                    let named = rest.contains(limit.as_str());
                    let carried = grant.limit == Some(*limit);
                    assert_eq!(
                        named, carried,
                        "{role} against {resource}: {fragment:?} against the limit {limit}"
                    );
                }
            }
        }
    }

    #[test]
    fn ori_t_0022_every_action_and_every_limit_is_named_by_a_cell() {
        for action in Action::ALL {
            assert!(
                MATRIX
                    .iter()
                    .any(|cell| cell.grants.iter().any(|grant| grant.action == *action)),
                "no cell of AICD §17 grants {action}, so it is a capability nobody granted"
            );
        }
        for limit in Constraint::ALL {
            assert!(
                MATRIX
                    .iter()
                    .any(|cell| cell.grants.iter().any(|grant| grant.limit == Some(*limit))),
                "no cell of AICD §17 limits a grant to {limit}, so it is a limit nobody wrote"
            );
        }
    }

    #[test]
    fn ori_t_0022_the_whole_product_of_role_resource_and_action_is_decided() {
        let actor = agent();
        let mut wrong = Vec::new();
        let mut allowed = 0usize;
        let mut refused = 0usize;
        for role in Role::ALL {
            for resource in Resource::ALL {
                for action in Action::ALL {
                    let want = expected(*role, *resource, *action);
                    let got = permits(&actor, *role, *resource, *action);
                    if want.is_allowed() {
                        allowed += 1;
                    } else {
                        refused += 1;
                    }
                    if got != want {
                        wrong.push(format!(
                            "{role} on {resource}, action {action}: the matrix says {want:?} and \
                             the function said {got:?} (the cell reads {:?})",
                            cell(*role, *resource)
                        ));
                    }
                }
            }
        }
        assert_eq!(
            allowed + refused,
            Role::ALL.len() * Resource::ALL.len() * Action::ALL.len(),
            "the enumeration did not cover the whole product"
        );
        assert_eq!(allowed, GRANTED.len(), "AICD §17 grants forty-two triples");
        assert!(
            wrong.is_empty(),
            "the permission function disagrees with AICD §17, {}",
            report(&wrong, 12)
        );
    }

    #[test]
    fn ori_t_0022_a_grant_carries_the_limit_its_cell_names() {
        let actor = agent();
        let write_own_branch =
            permits(&actor, Role::Coder, Resource::CodeRepository, Action::Write);
        assert_eq!(write_own_branch.limit(), Some(Constraint::Own));
        let merge_tier_zero = permits(&actor, Role::Lead, Resource::CodeRepository, Action::Merge);
        assert_eq!(merge_tier_zero.limit(), Some(Constraint::TierZero));
        let read_all = permits(&actor, Role::Lead, Resource::CodeRepository, Action::Read);
        assert_eq!(
            read_all.limit(),
            None,
            "the lead reads all of it, which is a grant with no limit"
        );
        assert_eq!(
            permits(&actor, Role::Qa, Resource::CodeRepository, Action::Read).limit(),
            None,
            "a refusal carries no limit"
        );
    }

    #[test]
    fn ori_t_0022_an_actor_that_is_not_an_agent_is_not_governed_by_the_matrix() {
        let human =
            Actor::Human(crate::types::Id::parse(SAMPLE_ULID).expect("a well formed ULID parses"));
        for actor in [&human, &Actor::System] {
            for role in Role::ALL {
                for resource in Resource::ALL {
                    for action in Action::ALL {
                        let decision = permits(actor, *role, *resource, *action);
                        assert_eq!(
                            decision,
                            Decision::NotGoverned,
                            "{actor} as {role} on {resource}, action {action}"
                        );
                        assert!(
                            !decision.is_allowed(),
                            "not governed is not a grant, and {actor} was granted {action} on \
                             {resource}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ori_p1_018_qa_may_not_write_any_file_in_the_repository() {
        let actor = agent();
        let decision = permits(&actor, Role::Qa, Resource::CodeRepository, Action::Write);
        assert_eq!(
            decision,
            refused(Resource::CodeRepository),
            "AICD §17 gives qa {:?} on the code repositories, and AICD §7 says \"No code \
             repositories\"",
            cell(Role::Qa, Resource::CodeRepository)
        );
        assert!(!decision.is_allowed());
        let reason = decision
            .methodology_ref()
            .expect("a refusal carries the section it was made under");
        assert_eq!(
            reason.section, 17,
            "the refusal cites the permission matrix"
        );
        assert!(
            reason.resolves(),
            "criterion ORI-P1-033: the reference resolves in the methodology index"
        );
    }

    #[test]
    fn ori_p1_018_qa_holds_nothing_at_all_on_the_code_repositories() {
        let actor = agent();
        assert!(
            grants(Role::Qa, Resource::CodeRepository).is_empty(),
            "AICD §17's cell for qa against the code repositories is \"None\""
        );
        for action in Action::ALL {
            assert_eq!(
                permits(&actor, Role::Qa, Resource::CodeRepository, *action),
                refused(Resource::CodeRepository),
                "qa was granted {action} on the code repositories"
            );
        }
    }

    #[test]
    fn ori_p1_019_no_agent_role_holds_anything_over_the_keychain_or_an_env_file() {
        let actor = agent();
        let mut wrong = Vec::new();
        for role in Role::ALL {
            for action in Action::ALL {
                let decision = permits(&actor, *role, Resource::Secrets, *action);
                if decision != refused(Resource::Secrets) {
                    wrong.push(format!(
                        "{role} was answered {decision:?} for {action} on the keychain and \
                         .env files"
                    ));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "AICD §27: a single secrets manager is the only place credentials live, {}",
            report(&wrong, 12)
        );
        assert_eq!(
            refusal_for(Resource::Secrets).section,
            27,
            "the secrets refusal cites the secrets architecture, not the matrix"
        );
        assert!(refusal_for(Resource::Secrets).resolves());
    }

    #[test]
    fn ori_p1_019_an_env_file_anywhere_in_a_path_is_a_secret_location() {
        // Paths as values. Nothing here is opened, which CLAUDE.md rule 4 and
        // spec/ENV_SETUP.md section 6 forbid.
        for secret in [
            ".env",
            ".env.local",
            ".env.production",
            "./.env",
            "crates/ori-broker/.env",
            "/Users/someone/product/.env.test",
            ".env/inside-a-directory-named-env",
            "a\\windows\\path\\.env",
            // The same file as .env on the default file system of two of the
            // three platforms this product ships to.
            ".ENV",
            ".Env.Local",
            "crates/ori-broker/.ENV",
        ] {
            assert!(
                is_secret_location(secret),
                "{secret} names a place credentials live and was not recognized"
            );
        }
        for ordinary in [
            "crates/ori-core/src/permission.rs",
            "spec/ENV_SETUP.md",
            "environment.rs",
            "docs/env-setup.md",
            "",
        ] {
            assert!(
                !is_secret_location(ordinary),
                "{ordinary} is an ordinary path and was refused as a secret one"
            );
        }
    }

    #[test]
    fn ori_p1_033_every_decision_this_function_makes_carries_a_reference_that_resolves() {
        let actor = agent();
        for role in Role::ALL {
            for resource in Resource::ALL {
                for action in Action::ALL {
                    let decision = permits(&actor, *role, *resource, *action);
                    let reason = decision.methodology_ref();
                    assert_eq!(
                        reason.is_some(),
                        decision.is_refusal(),
                        "{role} on {resource}, action {action}: a refusal and nothing else \
                         carries a section"
                    );
                    if let Some(reason) = reason {
                        assert!(
                            reason.resolves(),
                            "{role} on {resource}, action {action}: the reason resolves against \
                             no section of the methodology"
                        );
                    }
                }
            }
        }
        for resource in Resource::ALL {
            assert!(
                refusal_for(*resource).resolves(),
                "the refusal section for {resource} resolves against no section"
            );
        }
    }
}
