//! `AgentIdentity` and the creation path: AICD §7, `spec/DATA_MODEL.md`
//! section 2.
//!
//! `spec/LLD.md` section 2 gives `ori-broker` "`Identity`, `Issuance`,
//! `Keychain` (keyring), `ForbiddenActionTest`"; this module is the first of
//! those four, spelled `AgentIdentity` after `spec/DATA_MODEL.md` section 2's
//! row name. `spec/ENV_SETUP.md` section 5 is the permission manifest one
//! identity of a given role is issued against; this module holds the identity
//! itself, not the manifest.
//!
//! # Fields, and where each one comes from
//!
//! `spec/DATA_MODEL.md` section 2: "**AgentIdentity** | id, product_id, role
//! (coder, lead, qa, operations, documentation, product_signal, assistant),
//! model, runtime (acp, headless), scopes (memory), permissions (json), state
//! (active, suspended) |". Seven fields there; eight below, `family` being
//! the one the row does not yet name.
//!
//! - `id`, `product_id`: [`ori_core::types::Id`], the same identifier type
//!   every other entity in this workspace uses. This module never generates
//!   one (no clock, no source of randomness lives here either; the caller
//!   that has one, ORI-T-0027's spawn path or a flow, supplies it, the same
//!   pattern `ori_core::types::Id` documents for itself).
//! - `role`: [`ori_core::types::Role`], not redefined here. AICD §7 names
//!   seven agent roles and `ori_core::permission` already carries AICD §17's
//!   permission matrix over exactly that type; a second `Role` in this crate
//!   would be a second copy of a seven-way enumeration with no way to keep
//!   the two in step, and `permissions_snapshot` below would have to guess
//!   which one AICD §17 was talking about.
//! - `model`: a plain string naming the model this identity runs on. Refused
//!   here only if empty, which no cross-model rule needs to know about.
//! - `family`: [`ori_core::types::ModelFamily`], required, never optional.
//!   ORI-T-0108, implementing the operator's ruling on escalation 6
//!   (`ops/phase-1-backlog.md`): ADR-0001 "Model family" originally recorded
//!   the family on `ProviderBinding`, which the ruling replaced with this
//!   field, because `ProviderBinding` (`keychain.rs`) carries no `model` and
//!   cannot tell apart two identities on the same provider but different
//!   models. A runtime adapter declares the family; the caller that has one
//!   (a flow, `ori-runtime`'s spawn path) passes it here already validated
//!   ([`ori_core::types::ModelFamily::parse`] refused anything malformed
//!   before this function ever saw it), so [`AgentIdentity::create`] itself
//!   does no further validation of it, only stores it. This function does
//!   not enforce ADR-0001's cross-model refusal itself (see "What 'the
//!   creation path' is, here" below); [`crate::registration::register_identity`]
//!   does, using `family.rs`'s [`crate::family::refuse_lead_sharing_family_with_coders`]
//!   and [`crate::family::refuse_coder_sharing_family_with_leads`].
//! - `runtime`: [`IdentityRuntime`], the wire values `spec/DATA_MODEL.md`
//!   section 2 states, "acp, headless".
//! - `scopes`: [`MemoryScopes`], the memory layers and filters
//!   `spec/ENV_SETUP.md` section 5's "Memory scope" column names per role
//!   (`"Canonical, organizational, operational filtered to declared scope,
//!   code map"` for a coder, and so on). `ori-memory` is not built yet
//!   (`crates/ori-memory/src/lib.rs` is a stub with no types), so this holds
//!   the scope names as opaque, non-empty strings rather than reaching for a
//!   `ScopeEnforcer` that does not exist; `ori-broker` does not depend on
//!   `ori-memory` in `spec/LLD.md` section 2's dependency diagram either way.
//! - `permissions`: a snapshot of what AICD §17's matrix grants this role,
//!   over every [`ori_core::permission::Resource`], rendered as text by
//!   `permissions_snapshot`. Computed from
//!   [`ori_core::permission::grants`], never hand-written, which is what
//!   makes "must not duplicate or contradict" true structurally rather than
//!   by care: there is only one place in this workspace that says what AICD
//!   §17 grants a role, and this field reads it rather than restating it.
//! - `state`: [`IdentityState`], "active, suspended". [`AgentIdentity::create`]
//!   always produces [`IdentityState::Active`]; a suspend/resume transition
//!   function is a state machine this ticket's declared scope does not cover
//!   (the same split ORI-T-0020 drew for `Ticket::apply` against `types.rs`).
//!
//! # What "the creation path" is, here
//!
//! [`AgentIdentity::create`], a pure constructor: it validates `model` and
//! `scopes`, derives `permissions` from `role`, stores `family` as given, and
//! returns an [`IdentityState::Active`] value. It does no IO and holds no
//! connection, matching this crate's dependency-only need for `ori-store`'s
//! *types* (`ori_core::types::Id`) here; the event this identity's creation
//! is recorded under, and the full `CredentialIssuance` lifecycle a session
//! needs, are `keychain.rs`'s `keychain::record_binding_resolved`,
//! ORI-T-0027's `issuance.rs`, and ORI-T-0108's
//! `crate::registration::register_identity` respectively, not this function.
//! This function staying pure is deliberate, not an oversight: ADR-0001's
//! cross-model refusal needs to compare against every other identity already
//! known for the product, which needs a read of the event log, which is IO;
//! putting that read here would make every existing caller of `create` (a
//! pure, in-memory constructor today, including this module's own tests)
//! suddenly need a database connection it has no reason to hold. See
//! [`crate::registration`]'s own doc comment for where that comparison
//! actually happens instead, and why a caller cannot silently skip it there.
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md`
//! section 2, inherited from the crate). Nothing here reads or writes a
//! secret at all; that is entirely `keychain.rs`'s.

use core::fmt;
use core::str::FromStr;
use std::collections::BTreeSet;

use ori_core::permission;
use ori_core::types::Id;
use ori_core::types::ModelFamily;
use ori_core::types::Role;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A value given to this module did not parse into the type it names.
///
/// Not a `ori_core::error::Error::Refused`-shaped refusal: nothing here is a
/// control refusing a governed action, so nothing here carries a
/// `MethodologyRef` (CLAUDE.md rule 9 is about refusals; input that failed to
/// parse is not one, the same split `ori_core::error::Error` and
/// `ori_store::event_log::EventLogError` both draw for themselves).
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
/// present and non-empty, the same restraint `ori_core::types::Scope`
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
/// here is one already returned by `ori_core::permission`, either
/// [`permission::Resource::as_str`] or a grant's own `fragment` (itself a
/// verbatim slice of AICD §17's prose, per that module's own doc comment); none
/// is invented here, and none is escaped, because a matrix cell's fragment is
/// plain prose ("write own branch", "Deploy from tags") that this workspace's
/// own tests already hold to containing no `"` or `\`
/// (`ori_core::permission::tests::ori_t_0022_every_grant_is_described_by_the_cell_fragment_it_came_from`
/// parses every fragment as plain text, and a `"` or `\` inside one would have
/// broken that parse already).
///
/// This is what makes "must not duplicate or contradict" AICD §17's matrix
/// true by construction rather than by review: nothing here re-derives a
/// grant, this only formats what `ori_core::permission::grants` already
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER_ULID: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    fn id() -> Id {
        Id::parse(SAMPLE_ULID).expect("a well formed ULID parses")
    }

    fn product() -> Id {
        Id::parse(OTHER_ULID).expect("a well formed ULID parses")
    }

    fn scopes() -> MemoryScopes {
        MemoryScopes::new(["canonical", "code_map"]).expect("two non-empty scope names")
    }

    /// A declared model family for a test that does not care which, added by
    /// ORI-T-0108 alongside `AgentIdentity::create`'s new required
    /// parameter.
    fn family(text: &str) -> ModelFamily {
        ModelFamily::parse(text).expect("a non-empty family parses")
    }

    #[test]
    fn ori_t_0026_create_reads_every_field_of_the_data_model_row() {
        let identity = AgentIdentity::create(
            id(),
            product(),
            Role::Coder,
            "claude-test-model",
            IdentityRuntime::Headless,
            scopes(),
            family("claude-3"),
        )
        .expect("a valid identity is created");

        assert_eq!(identity.id(), &id());
        assert_eq!(identity.product_id(), &product());
        assert_eq!(identity.role(), Role::Coder);
        assert_eq!(identity.model(), "claude-test-model");
        assert_eq!(identity.runtime(), IdentityRuntime::Headless);
        assert_eq!(identity.scopes().len(), 2);
        assert_eq!(identity.state(), IdentityState::Active);
    }

    #[test]
    fn ori_t_0026_create_always_starts_an_identity_active() {
        for role in Role::ALL {
            let identity = AgentIdentity::create(
                id(),
                product(),
                *role,
                "m",
                IdentityRuntime::Acp,
                MemoryScopes::default(),
                family("m-family"),
            )
            .expect("a minimal identity is created");
            assert_eq!(identity.state(), IdentityState::Active);
        }
    }

    #[test]
    fn ori_t_0026_an_empty_or_whitespace_model_is_refused() {
        for bad in ["", "   ", "\t\n"] {
            let err = AgentIdentity::create(
                id(),
                product(),
                Role::Coder,
                bad,
                IdentityRuntime::Acp,
                MemoryScopes::default(),
                family("m-family"),
            )
            .expect_err("an empty model is refused");
            assert!(matches!(
                err,
                IdentityError::Malformed { what: "model", .. }
            ));
        }
    }

    #[test]
    fn ori_t_0026_a_model_is_trimmed() {
        let identity = AgentIdentity::create(
            id(),
            product(),
            Role::Coder,
            "  claude  ",
            IdentityRuntime::Acp,
            MemoryScopes::default(),
            family("claude-3"),
        )
        .expect("a model with surrounding whitespace is created");
        assert_eq!(identity.model(), "claude");
    }

    #[test]
    fn ori_t_0026_memory_scopes_refuse_an_empty_entry() {
        let err = MemoryScopes::new(["canonical", "   "]).expect_err("a blank scope is refused");
        assert!(matches!(err, IdentityError::Malformed { .. }));
    }

    #[test]
    fn ori_t_0026_memory_scopes_deduplicate_and_sort() {
        let scopes =
            MemoryScopes::new(["code_map", "canonical", "code_map"]).expect("valid scopes");
        assert_eq!(scopes.len(), 2);
        assert_eq!(
            scopes.iter().collect::<Vec<_>>(),
            vec!["canonical", "code_map"]
        );
    }

    #[test]
    fn ori_t_0026_identity_runtime_round_trips_the_data_model_spellings() {
        for value in IdentityRuntime::ALL {
            assert_eq!(
                value
                    .as_str()
                    .parse::<IdentityRuntime>()
                    .expect("round trip"),
                *value
            );
        }
        assert_eq!(IdentityRuntime::Acp.as_str(), "acp");
        assert_eq!(IdentityRuntime::Headless.as_str(), "headless");
        assert_eq!(IdentityRuntime::ALL.len(), 2);
        assert!("interactive".parse::<IdentityRuntime>().is_err());
    }

    #[test]
    fn ori_t_0026_identity_state_round_trips_the_data_model_spellings() {
        for value in IdentityState::ALL {
            assert_eq!(
                value.as_str().parse::<IdentityState>().expect("round trip"),
                *value
            );
        }
        assert_eq!(IdentityState::Active.as_str(), "active");
        assert_eq!(IdentityState::Suspended.as_str(), "suspended");
        assert_eq!(IdentityState::ALL.len(), 2);
        assert!("gone".parse::<IdentityState>().is_err());
    }

    #[test]
    fn ori_t_0026_permissions_are_derived_from_the_permission_matrix_not_hand_written() {
        // Every fragment ori_core::permission::grants returns for this role,
        // over every resource, appears in the snapshot, and the snapshot
        // names every resource. A snapshot that duplicated or contradicted
        // the matrix instead of reading it would drift from this the moment
        // either was edited without the other; this reads only from
        // ori_core::permission, so it cannot.
        let identity = AgentIdentity::create(
            id(),
            product(),
            Role::Qa,
            "m",
            IdentityRuntime::Acp,
            MemoryScopes::default(),
            family("m-family"),
        )
        .expect("identity created");
        let snapshot = identity.permissions();

        for resource in permission::Resource::ALL {
            assert!(
                snapshot.contains(&format!("\"{}\":[", resource.as_str())),
                "the snapshot names no key for {resource}: {snapshot}"
            );
            for grant in permission::grants(Role::Qa, *resource) {
                assert!(
                    snapshot.contains(&format!("\"{}\"", grant.fragment)),
                    "the snapshot is missing the granted fragment {:?} for {resource}: {snapshot}",
                    grant.fragment
                );
            }
        }
        // Qa holds nothing on the code repositories (ORI-P1-018): the
        // snapshot must say so as an empty list, not omit the resource.
        assert!(snapshot.contains("\"code repositories\":[]"));
    }

    #[test]
    fn ori_t_0026_permissions_differ_by_role_the_way_the_matrix_does() {
        let coder = AgentIdentity::create(
            id(),
            product(),
            Role::Coder,
            "m",
            IdentityRuntime::Acp,
            MemoryScopes::default(),
            family("m-family"),
        )
        .expect("identity created");
        let qa = AgentIdentity::create(
            id(),
            product(),
            Role::Qa,
            "m",
            IdentityRuntime::Acp,
            MemoryScopes::default(),
            family("m-family"),
        )
        .expect("identity created");
        assert_ne!(coder.permissions(), qa.permissions());
    }

    #[test]
    fn ori_t_0026_two_identities_built_from_the_same_fields_are_equal() {
        let a = AgentIdentity::create(
            id(),
            product(),
            Role::Coder,
            "m",
            IdentityRuntime::Acp,
            scopes(),
            family("m-family"),
        )
        .expect("identity a");
        let b = AgentIdentity::create(
            id(),
            product(),
            Role::Coder,
            "m",
            IdentityRuntime::Acp,
            scopes(),
            family("m-family"),
        )
        .expect("identity b");
        assert_eq!(a, b);
    }
}
