//! `AgentIdentity` and the creation path, declared in `ori-core`: AICD §7,
//! `spec/DATA_MODEL.md` section 2.
//!
//! Moved here from `ori-broker`'s `identity.rs` by ORI-T-0058, because the
//! `VcsHost` trait of `spec/API_SPEC.md` section 4 takes an `AgentIdentity`
//! in its signature and a trait declared in `ori-core` can name only types
//! `ori-core` declares (`spec/LLD.md` section 2). Nothing in the move is a
//! redesign: the types, their fields, their constructors, their errors and
//! their `Debug`, `Display` and `FromStr` output are byte for byte what
//! `ori-broker` declared, and `ori_broker::identity` re-exports every one of
//! them at its old path, so the type is the same type at both paths.
//!
//! # Why every one of these could move
//!
//! The ticket asked for a check first: does `AgentIdentity::create` do IO or
//! call the keychain? It does neither. It trims one string, calls
//! [`crate::permission::grants`] (a pure function of the role) and stores
//! what it was given; the same holds for [`MemoryScopes::new`] and for the
//! `FromStr` of [`IdentityRuntime`] and [`IdentityState`]. So no part of the
//! identity had to be split off and left behind. What stays in `ori-broker`
//! is everything that does need the broker: `keychain.rs`, `issuance.rs`,
//! `family.rs` (the cross-model refusal) and `registration.rs` (which
//! compares a new identity with the others the event log knows).
//!
//! # Fields, and where each one comes from
//!
//! `spec/DATA_MODEL.md` section 2: "**AgentIdentity** | id, product_id, role
//! (coder, lead, qa, operations, documentation, product_signal, assistant),
//! model, runtime (acp, headless), scopes (memory), permissions (json), state
//! (active, suspended) |". Eight fields there; nine below, `family` being
//! the one the row does not yet name.
//!
//! - `id`, `product_id`: [`crate::types::Id`], the same identifier type every
//!   other entity in this workspace uses. This module never generates one.
//! - `role`: [`crate::types::Role`], not redefined here.
//! - `model`: a plain string naming the model this identity runs on. Refused
//!   only if empty.
//! - `family`: [`crate::types::ModelFamily`], required, never optional
//!   (ADR-0001 "Model family"; ORI-T-0108). The creation path stores it as
//!   given; it was validated by [`crate::types::ModelFamily::parse`] before
//!   it arrived. The cross-model refusal of ADR-0001 is enforced in
//!   `ori-broker`, not here, because it needs a read of the event log.
//! - `runtime`: [`IdentityRuntime`], the wire values `spec/DATA_MODEL.md`
//!   section 2 states, "acp, headless".
//! - `scopes`: [`MemoryScopes`], opaque non-empty labels. `ori-memory` does
//!   not define a typed scope yet.
//! - `permissions`: a text snapshot of what AICD §17's matrix grants the role,
//!   computed from [`crate::permission::grants`] and never hand-written.
//! - `state`: [`IdentityState`]. [`AgentIdentity::create`] always produces
//!   [`IdentityState::Active`].
//!
//! Must not: do IO, read a clock, or touch a credential. Nothing here holds a
//! secret at all.

use core::fmt;
use core::str::FromStr;
use std::collections::BTreeSet;

use crate::permission;
use crate::types::Id;
use crate::types::ModelFamily;
use crate::types::Role;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A value given to this module did not parse into the type it names.
///
/// Not a refusal-shaped error of [`crate::error::Error`]: nothing here is a
/// control refusing a governed action, so nothing here carries a
/// `MethodologyRef` (CLAUDE.md rule 9 is about refusals; input that failed to
/// parse is not one, the same split the event log's own error type draws for
/// itself).
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum IdentityError {
    /// A value did not parse into the type or shape named by `what`.
    Malformed {
        /// The field or type the value was read as.
        what: &'static str,
        /// The value as it was given.
        value: String,
    },
}

impl IdentityError {
    fn malformed(what: &'static str, value: impl Into<String>) -> Self {
        Self::Malformed {
            what,
            value: value.into(),
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
        }
    }
}

impl std::error::Error for IdentityError {}

// ---------------------------------------------------------------------------
// IdentityRuntime
// ---------------------------------------------------------------------------

/// Which runtime an identity's sessions run under: `spec/DATA_MODEL.md`
/// section 2, `AgentIdentity.runtime (acp, headless)`.
///
/// `spec/LLD.md` section 2 gives `ori-runtime` `AcpClient` and
/// `HeadlessAdapter`; this is the value that says which of the two an
/// identity's sessions are launched through, not either implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IdentityRuntime {
    /// The Agent Client Protocol.
    Acp,
    /// A headless adapter, `ori-runtime`'s `HeadlessAdapter` trait.
    Headless,
}

impl IdentityRuntime {
    /// Both values, in the order `spec/DATA_MODEL.md` section 2 lists them.
    pub const ALL: &'static [Self] = &[Self::Acp, Self::Headless];

    /// The spelling `spec/DATA_MODEL.md` section 2 gives this value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Acp => "acp",
            Self::Headless => "headless",
        }
    }
}

impl fmt::Display for IdentityRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for IdentityRuntime {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "acp" => Ok(Self::Acp),
            "headless" => Ok(Self::Headless),
            other => Err(IdentityError::malformed("IdentityRuntime", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// IdentityState
// ---------------------------------------------------------------------------

/// Where an identity is in its lifecycle: `spec/DATA_MODEL.md` section 2,
/// `AgentIdentity.state (active, suspended)`.
///
/// The transition between the two (a human, per `spec/ENV_SETUP.md` section 5
/// "Any deviation is a decisional ticket", suspending an identity) is not a
/// function this module provides; this ticket's declared scope is the
/// identity and its creation path, not a suspend/resume state machine, and
/// [`AgentIdentity::create`] only ever produces [`IdentityState::Active`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IdentityState {
    /// May be assigned sessions.
    Active,
    /// May not; existing credentials are expected to have been revoked by
    /// whatever suspended it (ORI-T-0027's concern, not this module's).
    Suspended,
}

impl IdentityState {
    /// Both values, in the order `spec/DATA_MODEL.md` section 2 lists them.
    pub const ALL: &'static [Self] = &[Self::Active, Self::Suspended];

    /// The spelling `spec/DATA_MODEL.md` section 2 gives this value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
        }
    }
}

impl fmt::Display for IdentityState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for IdentityState {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            other => Err(IdentityError::malformed("IdentityState", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryScopes
// ---------------------------------------------------------------------------

/// The memory scope names an identity holds: `spec/DATA_MODEL.md` section 2,
/// `AgentIdentity.scopes (memory)`.
///
/// Held as a set of non-empty, trimmed labels (`spec/ENV_SETUP.md` section
/// 5's "Memory scope" column values, read as prose today: "Canonical,
/// organizational, operational filtered to declared scope, code map" for a
/// coder, and so on for the other six roles) rather than as a typed
/// `ScopeEnforcer` value, because `ori-memory` does not define one yet
/// (`crates/ori-memory/src/lib.rs` is an empty stub) and `ori-broker` does not
/// depend on `ori-memory` in `spec/LLD.md` section 2's dependency diagram.
/// When `ori-memory::ScopeEnforcer` exists, a later ticket converts these
/// labels into it; nothing here decides what a label means, only that it is
/// present and non-empty, the same restraint `crate::types::Scope`
/// documents for its own module paths ("A caller converting those into this
/// type states the directory instead").
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemoryScopes(BTreeSet<String>);

impl MemoryScopes {
    /// Reads a set of scope names, refusing one that is empty or only
    /// whitespace. Duplicates collapse, and the order given is not kept: two
    /// identities that hold the same scopes by any order or repetition are
    /// the same value, which is what lets [`MemoryScopes`] derive
    /// [`PartialEq`].
    pub fn new<I, S>(scopes: I) -> Result<Self, IdentityError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut set = BTreeSet::new();
        for scope in scopes {
            let scope = scope.as_ref();
            let trimmed = scope.trim();
            if trimmed.is_empty() {
                return Err(IdentityError::malformed("memory scope", scope));
            }
            set.insert(trimmed.to_owned());
        }
        Ok(Self(set))
    }

    /// The scope names, sorted.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    /// How many scope names are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no scope name is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Permissions snapshot
// ---------------------------------------------------------------------------

/// Renders what AICD §17's matrix grants `role`, over every
/// [`permission::Resource`], as the text [`AgentIdentity::permissions`]
/// holds.
///
/// Built by walking [`permission::Resource::ALL`] and, for each, calling
/// [`permission::grants`] and writing out every [`permission::Grant::fragment`]
/// it returns, in the order `permission::grants` returns them, which is the
/// order AICD §17's own cells write their fragments in. Every string written
/// here is one already returned by `crate::permission`, either
/// [`permission::Resource::as_str`] or a grant's own `fragment` (itself a
/// verbatim slice of AICD §17's prose, per that module's own doc comment); none
/// is invented here, and none is escaped, because a matrix cell's fragment is
/// plain prose ("write own branch", "Deploy from tags") that this workspace's
/// own tests already hold to containing no `"` or `\`
/// (`crate::permission::tests::ori_t_0022_every_grant_is_described_by_the_cell_fragment_it_came_from`
/// parses every fragment as plain text, and a `"` or `\` inside one would have
/// broken that parse already).
///
/// This is what makes "must not duplicate or contradict" AICD §17's matrix
/// true by construction rather than by review: nothing here re-derives a
/// grant, this only formats what `crate::permission::grants` already
/// decided.
fn permissions_snapshot(role: Role) -> String {
    let mut out = String::from("{");
    for (resource_index, resource) in permission::Resource::ALL.iter().enumerate() {
        if resource_index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(resource.as_str());
        out.push_str("\":[");
        let grants = permission::grants(role, *resource);
        for (grant_index, grant) in grants.iter().enumerate() {
            if grant_index > 0 {
                out.push(',');
            }
            out.push('"');
            out.push_str(grant.fragment);
            out.push('"');
        }
        out.push(']');
    }
    out.push('}');
    out
}

// ---------------------------------------------------------------------------
// AgentIdentity
// ---------------------------------------------------------------------------

/// One agent identity: AICD §7, `spec/DATA_MODEL.md` section 2.
///
/// Fields are private; every one of them either came through
/// [`AgentIdentity::create`]'s own validation or was derived by it
/// ([`AgentIdentity::permissions`], [`AgentIdentity::state`]). A value built
/// any other way is not a promise this module made about the identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIdentity {
    id: Id,
    product_id: Id,
    role: Role,
    model: String,
    runtime: IdentityRuntime,
    scopes: MemoryScopes,
    permissions: String,
    state: IdentityState,
    family: ModelFamily,
}

impl AgentIdentity {
    /// The creation path: `spec/DATA_MODEL.md` section 2's `AgentIdentity`
    /// row, in the shape ORI-T-0026 asks for, plus `family`
    /// (ORI-T-0108, the operator's ruling on escalation 6).
    ///
    /// Refuses ([`IdentityError::Malformed`]) an empty or whitespace-only
    /// `model`; `scopes` is already validated by [`MemoryScopes::new`], so a
    /// caller cannot construct one holding an empty label to begin with.
    /// `family` is already validated by [`ModelFamily::parse`] before it
    /// reaches here, the same way `scopes` is; there is no way to construct a
    /// [`ModelFamily`] this function would need to re-refuse, and no way to
    /// omit it; `family` is a required parameter, not `Option<ModelFamily>`,
    /// because an identity with no family is the fail-open case ORI-T-0028's
    /// checks exist to prevent (see the module doc comment, "Fields, and
    /// where each one comes from"). `permissions` is not a parameter: it is
    /// `permissions_snapshot` of `role`, always, so it can never disagree
    /// with `role`. The identity this returns always holds
    /// [`IdentityState::Active`]; there is no parameter to start one
    /// suspended, matching this module's doc comment, "What 'the creation
    /// path' is, here".
    pub fn create(
        id: Id,
        product_id: Id,
        role: Role,
        model: impl Into<String>,
        runtime: IdentityRuntime,
        scopes: MemoryScopes,
        family: ModelFamily,
    ) -> Result<Self, IdentityError> {
        let model = model.into();
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return Err(IdentityError::malformed("model", model));
        }
        let permissions = permissions_snapshot(role);
        Ok(Self {
            id,
            product_id,
            role,
            model: trimmed.to_owned(),
            runtime,
            scopes,
            permissions,
            state: IdentityState::Active,
            family,
        })
    }

    /// The identity identifier.
    #[must_use]
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// The product this identity belongs to.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// What this identity is for: AICD §7.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The model this identity runs on.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The declared model family this identity runs on: ADR-0001 "Model
    /// family", AICD §7. Required, never absent; see
    /// [`AgentIdentity::create`]'s own doc comment for why this is not
    /// `Option<&ModelFamily>`.
    #[must_use]
    pub const fn family(&self) -> &ModelFamily {
        &self.family
    }

    /// Which runtime this identity's sessions launch through.
    #[must_use]
    pub const fn runtime(&self) -> IdentityRuntime {
        self.runtime
    }

    /// The memory scope names this identity holds.
    #[must_use]
    pub const fn scopes(&self) -> &MemoryScopes {
        &self.scopes
    }

    /// What AICD §17's matrix grants this identity's role, rendered as text
    /// by `permissions_snapshot` at creation.
    #[must_use]
    pub fn permissions(&self) -> &str {
        &self.permissions
    }

    /// Where this identity is in its lifecycle.
    #[must_use]
    pub const fn state(&self) -> IdentityState {
        self.state
    }
}
