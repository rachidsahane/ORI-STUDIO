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
//! model, family, runtime (acp, headless), scopes (memory), permissions
//! (json), state (active, suspended) |". Nine fields there and nine in the
//! struct; the struct lists `family` last, the row lists it after `model`.
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
//!   the two in step, and `permissions_snapshot` (a private function beside
//!   `AgentIdentity::create`, now in `ori-core`) would have to guess which
//!   one AICD §17 was talking about.
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
//!   `permissions_snapshot` (private, in `ori-core`). Computed from
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
//! # Where the declarations live (ORI-T-0058)
//!
//! `AgentIdentity`, `IdentityError`, `IdentityRuntime`, `IdentityState` and
//! `MemoryScopes` are declared in `ori-core` (`ori_core::adapters::identity`),
//! because the `VcsHost` trait of `spec/API_SPEC.md` section 4 takes an
//! `AgentIdentity` and `ori-core` may name only types it declares
//! (`spec/LLD.md` section 2). They are re-exported below at the paths they
//! always had, so the type is the same type at both paths and no caller
//! changed. Nothing about them was redesigned: `create` does no IO and never
//! called the keychain, so the whole identity moved and nothing had to be
//! split off.
//!
//! The text above this section was written when they were declared in this
//! file. It describes their behaviour, which did not change, and it is still
//! true of the types, with this reading: where it says "here", "this module"
//! or "this function" about `create`, `MemoryScopes` or the private
//! `permissions_snapshot`, those are now declared in `ori-core`'s
//! `adapters/identity.rs`, which carries the matching documentation. What
//! this file still owns is the re-export, the registration seam pointed to above
//! (`registration.rs`), and the tests of the types' behaviour, which live
//! below and ran unchanged through the move.
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md`
//! section 2, inherited from the crate). Nothing here reads or writes a
//! secret at all; that is entirely `keychain.rs`'s.

pub use ori_core::adapters::identity::AgentIdentity;
pub use ori_core::adapters::identity::IdentityError;
pub use ori_core::adapters::identity::IdentityRuntime;
pub use ori_core::adapters::identity::IdentityState;
pub use ori_core::adapters::identity::MemoryScopes;

// The test module below, unchanged, reaches these through `use super::*`, as
// it did when this module imported them for its own declarations. Nothing
// outside the tests needs them here any more, so they are test-only.
#[cfg(test)]
use ori_core::permission;
#[cfg(test)]
use ori_core::types::Id;
#[cfg(test)]
use ori_core::types::ModelFamily;
#[cfg(test)]
use ori_core::types::Role;

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

/// ORI-T-0058: the declarations moved to `ori-core` are the same types at
/// the path `ori-broker` has always exported them from. A separate module
/// from `tests` above, which is untouched.
#[cfg(test)]
mod ori_t_0058_reexports {
    use core::any::TypeId;

    use ori_core::adapters;
    use ori_core::types::Id;
    use ori_core::types::ModelFamily;
    use ori_core::types::Role;

    /// Takes the identity by the path `ori-broker` exports it at.
    fn model_through_the_broker_path(identity: &crate::identity::AgentIdentity) -> &str {
        identity.model()
    }

    /// Takes the identity by the path `ori-core` declares it at.
    fn model_through_the_core_path(identity: &adapters::AgentIdentity) -> &str {
        identity.model()
    }

    #[test]
    fn ori_t_0058_the_moved_identity_is_the_same_type_at_both_paths() {
        let built_through_core = adapters::AgentIdentity::create(
            Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("a ULID"),
            Id::parse("01BX5ZZKBKACTAV9WEVGEMMVRZ").expect("a ULID"),
            Role::Coder,
            "claude-test-model",
            adapters::IdentityRuntime::Headless,
            adapters::MemoryScopes::new(["canonical"]).expect("one scope"),
            ModelFamily::parse("claude-3").expect("a family"),
        )
        .expect("a valid identity");

        // A function typed with one path accepts a value built through the
        // other, in both directions: this does not compile unless the two
        // paths name one type.
        assert_eq!(
            model_through_the_broker_path(&built_through_core),
            "claude-test-model"
        );
        let built_through_broker: crate::identity::AgentIdentity = built_through_core.clone();
        assert_eq!(
            model_through_the_core_path(&built_through_broker),
            "claude-test-model"
        );
        assert_eq!(built_through_core, built_through_broker);
    }

    #[test]
    fn ori_t_0058_every_moved_identity_type_has_one_type_id_at_both_paths() {
        assert_eq!(
            TypeId::of::<crate::identity::AgentIdentity>(),
            TypeId::of::<adapters::AgentIdentity>()
        );
        assert_eq!(
            TypeId::of::<crate::identity::IdentityError>(),
            TypeId::of::<adapters::IdentityError>()
        );
        assert_eq!(
            TypeId::of::<crate::identity::IdentityRuntime>(),
            TypeId::of::<adapters::IdentityRuntime>()
        );
        assert_eq!(
            TypeId::of::<crate::identity::IdentityState>(),
            TypeId::of::<adapters::IdentityState>()
        );
        assert_eq!(
            TypeId::of::<crate::identity::MemoryScopes>(),
            TypeId::of::<adapters::MemoryScopes>()
        );
    }

    #[test]
    fn ori_t_0058_the_moved_identity_keeps_the_debug_output_it_had() {
        // The names a derived `Debug` prints are the type's and its fields',
        // so a move that kept both keeps the output; this pins the one
        // string a caller could have matched on.
        let identity = adapters::AgentIdentity::create(
            Id::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("a ULID"),
            Id::parse("01BX5ZZKBKACTAV9WEVGEMMVRZ").expect("a ULID"),
            Role::Qa,
            "m",
            adapters::IdentityRuntime::Acp,
            adapters::MemoryScopes::default(),
            ModelFamily::parse("f").expect("a family"),
        )
        .expect("a valid identity");
        let printed = format!("{identity:?}");
        assert!(printed.starts_with("AgentIdentity { id: "), "{printed}");
        assert!(printed.contains("runtime: Acp"), "{printed}");
        assert!(printed.contains("state: Active"), "{printed}");
    }
}
