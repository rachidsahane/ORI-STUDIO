//! `register_identity`: the one place `AgentIdentity::create`'s cross-model
//! refusal (ADR-0001 "Model family", AICD §7, criterion ORI-P1-035) is
//! actually enforced against every other identity that exists. ORI-T-0108,
//! implementing the operator's ruling on escalation 6
//! (`ops/phase-1-backlog.md`).
//!
//! # Why this is a separate module from `identity.rs`
//!
//! `identity.rs`'s own doc comment says `AgentIdentity::create` "does no IO
//! and holds no connection", and stays that way in ORI-T-0108: the family
//! field is now required on the struct, but `create` only stores it, it does
//! not compare it against anything. The comparison needs to know about every
//! other identity already registered for the product, which needs a read of
//! the event log, which is IO `identity.rs` was never given and should not
//! be, the same separation `keychain.rs` (`record_binding_resolved`) and
//! `issuance.rs` (`issue_credential`, `revoke_session`) already draw for
//! their own event-recording responsibilities. This module is the third of
//! that family, not a fourth thing bolted onto `identity.rs`.
//!
//! # Where "the coders it reviews" comes from, and how omission is prevented
//!
//! There is no persisted table of identities anywhere in this workspace;
//! `crates/ori-store` has no `identity.created` event kind before this
//! ticket and no migration is added by it (out of this ticket's declared
//! scope, and the operator's instruction was explicit that none is needed).
//! The design this module uses instead is the one `issuance.rs` already
//! established for `CredentialIssuance`: **record as an event, answer
//! "inspect" by reading the log back**. [`register_identity`] is the only
//! function in this crate that appends an `identity.created` event, and it
//! is also the only function that reads existing ones back
//! (`identities_for_product`), in the same call, before deciding whether
//! the append it is about to make is allowed. A caller of
//! [`register_identity`] supplies `db`, `at`, `actor` and the
//! [`crate::identity::AgentIdentity`] it wants registered; it does not, and
//! cannot, supply the set [`crate::family::refuse_lead_sharing_family_with_coders`]
//! and [`crate::family::refuse_coder_sharing_family_with_leads`] compare
//! against, because that set is read by this function itself, from the log,
//! every time it runs. There is no parameter here a caller could hand `&[]`
//! to and have the check pass vacuously: the only way to make the check see
//! fewer identities than actually exist is to make `identities_for_product`
//! itself misread the log, which is exactly what `read_all_events`'s use of
//! [`ori_store::event_log::EventLog::verify`] (rather than a raw read) stands
//! against, the same reader-completeness guard
//! ([`ori_store::event_log::EventLogError::ReaderIncomplete`])
//! `issuance.rs`'s `read_all_events` already relies on for exactly this
//! reason. See "Plant 3" in this ticket's report for why this property
//! cannot be demonstrated by injecting a broken reader without editing
//! `ori-store`, which this ticket must not do, and what is checked instead.
//!
//! What still bypasses this: nothing stops a caller from calling
//! [`crate::identity::AgentIdentity::create`] on its own and simply never
//! calling [`register_identity`] at all, and nothing stops a caller from
//! using an `AgentIdentity` value that was never registered (spawning a
//! session against it, say). `AgentIdentity::create` itself has no way to
//! refuse this, it is a pure constructor with no event log to check against,
//! by design (see `identity.rs`'s own doc comment). This is a real, named
//! seam: whoever wires the end-to-end `broker.identity.create` path (a flow,
//! or `ori-runtime`'s spawn path) is the one that has to choose to call
//! [`register_identity`] and hold its `Ok` before treating an identity as
//! real, the same discipline `issuance.rs`'s own doc comment names for
//! `revoke_session` and session termination ("this module cannot guarantee
//! that the caller actually waits"). Nothing today enforces that discipline
//! mechanically; it is a gap for a future ticket that wires that path, not
//! one this module can close by itself, because only `ori-runtime` spawns
//! sessions and this crate does not depend on it.
//!
//! # Product scoping
//!
//! `identities_for_product` filters explicitly on `product_id`, even though
//! every event actually read here already comes from one `ProductDb`, which
//! `ori_store::event_log::EventLog::append`'s own `ProductMismatch` refusal
//! already makes single-product for the whole life of that file (`Product`,
//! `spec/DATA_MODEL.md` section 2, "One SQLite file per product"). The filter
//! is defense in depth, not dead code: it is what makes "a lead in product A
//! is never compared against a coder in product B" a property of this
//! function's own logic, checkable directly
//! (`tests::ori_t_0108_identities_for_product_excludes_another_products_identity`,
//! which combines real events read back from two separately opened
//! `ProductDb`s into one list and confirms the filter, not the file layout,
//! is what keeps them apart), rather than a property that happens to hold
//! only because of how `ProductDb` is used elsewhere today.
//!
//! # Order: check, then append, never the other way round
//!
//! [`register_identity`] runs the family check before it ever calls
//! [`ori_store::event_log::EventLog::append`]; a refused identity is never
//! written (`tests::ori_t_0108_a_refused_registration_appends_no_event`
//! checks the log's own event count is unchanged after a refusal, not only
//! that the call returned an error). Reversing that order would record an
//! identity ADR-0001 says should not exist, silently correct on every read
//! after the first (nothing reads it back and re-checks it), the AICD §39
//! failure mode of a check that ran too late to matter.
//!
//! ```mermaid
//! sequenceDiagram
//!   participant Caller
//!   participant Reg as register_identity
//!   participant Log as ori-store EventLog
//!   Caller->>Reg: register_identity(db, at, actor, identity)
//!   Reg->>Log: EventLog::verify (reader-completeness guard)
//!   Reg->>Log: EventLog::read_range(1, tip)
//!   Reg->>Reg: identities_for_product(events, identity.product_id())
//!   alt identity.role() is Lead
//!     Reg->>Reg: refuse_lead_sharing_family_with_coders
//!   else identity.role() is Coder
//!     Reg->>Reg: refuse_coder_sharing_family_with_leads
//!   else any other role
//!     Reg->>Reg: no family check (ADR-0001 names only lead/coder)
//!   end
//!   alt refused
//!     Reg-->>Caller: Err (no event appended)
//!   else allowed
//!     Reg->>Log: EventLog::append("identity.created")
//!     Log-->>Reg: committed
//!     Reg-->>Caller: Ok(Event)
//!   end
//! ```
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md` section
//! 2, inherited from the crate). Nothing here reads or writes a secret; the
//! payload carries `id`, `product_id`, `role`, `model`, `family` and
//! `runtime`, and nothing else.

use core::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::ModelFamily;
use ori_core::types::Role;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use ori_store::event_log::EventLogError;

use crate::family;
use crate::family::FamilyError;
use crate::identity::AgentIdentity;
use crate::identity::IdentityRuntime;

// ---------------------------------------------------------------------------
// IdentityRegistrationError
// ---------------------------------------------------------------------------

/// A failure registering or reading back an identity.
///
/// `EventLog` wraps whatever [`ori_store::event_log::EventLog`] returned;
/// `Family` wraps a refusal or a malformed value from `family.rs`; `Malformed`
/// is this module's own parse failure over a payload it read back. The same
/// split `crate::issuance::IssuanceError` draws for itself: a genuine
/// refusal (inside `EventLogError` or a [`FamilyError::Refused`]) still
/// carries its own [`MethodologyRef`], reachable through
/// [`IdentityRegistrationError::methodology_ref`].
#[derive(Debug)]
#[non_exhaustive]
pub enum IdentityRegistrationError {
    /// A value did not parse into the type or shape named by `what`, read
    /// back out of an `identity.created` event payload this module itself
    /// wrote.
    Malformed {
        /// The field or shape the value was read as.
        what: &'static str,
        /// The value as it was given or found.
        value: String,
    },
    /// The event log refused or failed the call.
    EventLog(EventLogError),
    /// `family.rs` refused the registration: ADR-0001 "Model family", AICD
    /// §7, criterion ORI-P1-035.
    Family(FamilyError),
}

impl IdentityRegistrationError {
    fn malformed(what: &'static str, value: impl Into<String>) -> Self {
        Self::Malformed {
            what,
            value: value.into(),
        }
    }

    /// The methodology section a refusal was made under, for the refusal
    /// this error wraps and for nothing else (see this type's own doc
    /// comment).
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::EventLog(inner) => inner.methodology_ref(),
            Self::Family(FamilyError::Refused { reason, .. }) => Some(reason.clone()),
            Self::Family(FamilyError::Malformed { .. }) | Self::Malformed { .. } => None,
        }
    }
}

impl fmt::Display for IdentityRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
            Self::EventLog(inner) => write!(f, "{inner}"),
            Self::Family(inner) => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for IdentityRegistrationError {}

impl From<EventLogError> for IdentityRegistrationError {
    fn from(err: EventLogError) -> Self {
        Self::EventLog(err)
    }
}

impl From<FamilyError> for IdentityRegistrationError {
    fn from(err: FamilyError) -> Self {
        Self::Family(err)
    }
}

// ---------------------------------------------------------------------------
// RegisteredIdentity
// ---------------------------------------------------------------------------

/// One identity projected back out of the event log: the row
/// [`register_identity`] itself wrote, read by `identities_for_product`.
///
/// Never constructed directly by a caller outside this module, the same
/// discipline `crate::issuance::Issuance`'s own doc comment states for
/// itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredIdentity {
    id: Id,
    product_id: Id,
    role: Role,
    model: String,
    family: ModelFamily,
    runtime: IdentityRuntime,
}

impl RegisteredIdentity {
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
    /// family", AICD §7.
    #[must_use]
    pub const fn family(&self) -> &ModelFamily {
        &self.family
    }

    /// Which runtime this identity's sessions launch through.
    #[must_use]
    pub const fn runtime(&self) -> IdentityRuntime {
        self.runtime
    }
}

// ---------------------------------------------------------------------------
// register_identity
// ---------------------------------------------------------------------------

/// Registers `identity`, refusing it if it shares a declared model family
/// with an identity already registered on the other side of the lead/coder
/// boundary for the same product: ADR-0001 "Model family", AICD §7,
/// criterion ORI-P1-035.
///
/// Reads every `identity.created` event already in `db`'s log
/// (`identities_for_product`, filtered to `identity.product_id()`), then:
/// when `identity.role()` is [`Role::Lead`], calls
/// [`crate::family::refuse_lead_sharing_family_with_coders`] against every
/// registered [`Role::Coder`]; when it is [`Role::Coder`], calls
/// [`crate::family::refuse_coder_sharing_family_with_leads`] against every
/// registered [`Role::Lead`]; for any other role, ADR-0001 names no check, so
/// none runs. Only once that check passes (or does not apply) does this
/// function append one `identity.created` event, carrying `id`,
/// `product_id`, `role`, `model`, `family` and `runtime`, through
/// [`ori_store::event_log::EventLog::append`], into `identity`'s own
/// product's log (`identity.product_id()`, the same routing
/// `crate::keychain::record_binding_resolved` and `crate::issuance::issue_credential`
/// use). Never anything secret: `identity.rs`'s `AgentIdentity` carries no
/// secret field to leak in the first place.
///
/// See this module's own doc comment, "Where 'the coders it reviews' comes
/// from, and how omission is prevented", for why there is no parameter here
/// a caller could supply an empty set through.
///
/// # Errors
///
/// [`IdentityRegistrationError::Family`] when the check above refuses;
/// [`IdentityRegistrationError::EventLog`] when reading or appending to the
/// log fails; [`IdentityRegistrationError::Malformed`] when an
/// `identity.created` event already in the log does not parse (a defect in
/// an earlier write, not in this call).
pub fn register_identity(
    db: &mut ProductDb,
    at: Timestamp,
    actor: Actor,
    identity: &AgentIdentity,
) -> Result<Event, IdentityRegistrationError> {
    let events = read_all_events(db)?;
    let existing = identities_for_product(&events, identity.product_id())?;

    let candidate = family::ModelFamily::from_core(identity.family().clone());
    match identity.role() {
        Role::Lead => {
            let coder_families: Vec<family::ModelFamily> = existing
                .iter()
                .filter(|registered| registered.role() == Role::Coder)
                .map(|registered| family::ModelFamily::from_core(registered.family().clone()))
                .collect();
            let coder_refs: Vec<Option<&family::ModelFamily>> =
                coder_families.iter().map(Some).collect();
            family::refuse_lead_sharing_family_with_coders(Some(&candidate), &coder_refs)?;
        }
        Role::Coder => {
            let lead_families: Vec<family::ModelFamily> = existing
                .iter()
                .filter(|registered| registered.role() == Role::Lead)
                .map(|registered| family::ModelFamily::from_core(registered.family().clone()))
                .collect();
            let lead_refs: Vec<Option<&family::ModelFamily>> =
                lead_families.iter().map(Some).collect();
            family::refuse_coder_sharing_family_with_leads(Some(&candidate), &lead_refs)?;
        }
        Role::Qa
        | Role::Operations
        | Role::Documentation
        | Role::ProductSignal
        | Role::Assistant => {
            // ADR-0001's invariant is stated only over the lead/coder
            // boundary ("A lead identity and the coders it reviews never
            // share a model family"); no check applies to the other five
            // roles.
        }
    }

    let payload = identity_created_payload(identity);
    Ok(EventLog::append(
        db.connection(),
        identity.product_id().clone(),
        at,
        actor,
        "identity.created",
        None,
        payload,
    )?)
}

// ---------------------------------------------------------------------------
// Reading the log
// ---------------------------------------------------------------------------

/// Reads every event currently in `db`'s log, the same shape
/// `crate::issuance::issuance.rs`'s own `read_all_events` uses and for the
/// same reason: going through [`EventLog::verify`] first for its `tip_seq`,
/// rather than a raw `SELECT`, puts the reader-completeness guard
/// [`EventLog::verify`] already carries
/// (`ori_store::event_log::EventLogError::ReaderIncomplete`) between this
/// module and a broken reader silently returning fewer rows than the table
/// holds, which is exactly the omission this ticket's report discusses under
/// "Plant 3".
fn read_all_events(db: &mut ProductDb) -> Result<Vec<Event>, IdentityRegistrationError> {
    let report = EventLog::verify(db.connection())?;
    match report.tip_seq {
        Some(tip) => Ok(EventLog::read_range(db.connection(), 1, tip)?),
        None => Ok(Vec::new()),
    }
}

/// The pure half of [`register_identity`]'s read: every [`RegisteredIdentity`]
/// among `events` whose own `product_id` equals `product_id`. See this
/// module's own doc comment, "Product scoping", for why this filters
/// explicitly rather than trusting that `events` already came from one
/// product's file.
fn identities_for_product(
    events: &[Event],
    product_id: &Id,
) -> Result<Vec<RegisteredIdentity>, IdentityRegistrationError> {
    let mut out = Vec::new();
    for event in events {
        if event.kind() == "identity.created" {
            let registered = parse_identity_created(event)?;
            if registered.product_id().as_str() == product_id.as_str() {
                out.push(registered);
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Payload encoding
// ---------------------------------------------------------------------------

/// The payload [`register_identity`] writes: `id`, `product_id`, `role`,
/// `model`, `family`, `runtime`, and nothing else. No field here is ever a
/// secret; `AgentIdentity` carries no secret-shaped field to begin with.
fn identity_created_payload(identity: &AgentIdentity) -> String {
    format!(
        "{{\"id\":\"{}\",\"product_id\":\"{}\",\"role\":\"{}\",\"model\":\"{}\",\"family\":\"{}\",\"runtime\":\"{}\"}}",
        json_escape(identity.id().as_str()),
        json_escape(identity.product_id().as_str()),
        identity.role(),
        json_escape(identity.model()),
        json_escape(identity.family().as_str()),
        identity.runtime(),
    )
}

/// Escapes `"` and `\` for the small hand-written JSON payload above. This
/// crate has no JSON parser or serializer as a dependency (adding one is a
/// `new_dependency` escalation this ticket does not raise), the same
/// deliberate choice `crate::keychain` and `crate::issuance` each document
/// for their own payload builders.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

/// Reads one `identity.created` event back into a [`RegisteredIdentity`].
fn parse_identity_created(event: &Event) -> Result<RegisteredIdentity, IdentityRegistrationError> {
    let payload = event.payload();
    let id = parse_id_field(payload, "id")?;
    let product_id = parse_id_field(payload, "product_id")?;
    let role_text = field_str(payload, "role")
        .ok_or_else(|| IdentityRegistrationError::malformed("identity.created role", payload))?;
    let role: Role = role_text
        .parse()
        .map_err(|_| IdentityRegistrationError::malformed("identity.created role", role_text))?;
    let model = field_str(payload, "model")
        .ok_or_else(|| IdentityRegistrationError::malformed("identity.created model", payload))?;
    let family_text = field_str(payload, "family")
        .ok_or_else(|| IdentityRegistrationError::malformed("identity.created family", payload))?;
    let family = ModelFamily::parse(&family_text).map_err(|_| {
        IdentityRegistrationError::malformed("identity.created family", family_text)
    })?;
    let runtime_text = field_str(payload, "runtime")
        .ok_or_else(|| IdentityRegistrationError::malformed("identity.created runtime", payload))?;
    let runtime: IdentityRuntime = runtime_text.parse().map_err(|_| {
        IdentityRegistrationError::malformed("identity.created runtime", runtime_text)
    })?;

    Ok(RegisteredIdentity {
        id,
        product_id,
        role,
        model,
        family,
        runtime,
    })
}

/// Reads a string field and parses it as an [`Id`], for the two id-shaped
/// string fields [`parse_identity_created`] reads.
fn parse_id_field(payload: &str, key: &'static str) -> Result<Id, IdentityRegistrationError> {
    let text = field_str(payload, key)
        .ok_or_else(|| IdentityRegistrationError::malformed(key, payload))?;
    Id::parse(&text).map_err(|_| IdentityRegistrationError::malformed(key, text))
}

/// Reads the quoted string value of `"key":"..."`, unescaping `\"` and
/// `\\`, `None` if `key` is not present as a quoted-string field. The same
/// small reader `crate::issuance`'s own `field_str` implements, duplicated
/// here rather than shared: each module in this crate that writes its own
/// hand-rolled JSON reads it back with its own hand-rolled reader, the
/// pattern `crate::keychain` and `crate::issuance` already both follow.
fn field_str(payload: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = payload.find(&needle)? + needle.len();
    let mut out = String::new();
    let mut chars = payload[start..].chars();
    loop {
        match chars.next()? {
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => out.push(other),
            },
            '"' => return Some(out),
            ch => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use ori_core::types::Actor;

    use super::*;
    use crate::identity::MemoryScopes;

    // -----------------------------------------------------------------------
    // Scratch layout, the same self-cleaning scratch directory pattern
    // crates/ori-broker/src/issuance.rs's own tests use.
    // -----------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let root = std::env::temp_dir();
            let path = root.join(format!(
                "ori-t-0108-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a fresh scratch directory can be created");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A valid ULID, distinguished by `label`, the same sanitizing scheme
    /// `crates/ori-store/src/event_log.rs`'s and `crate::issuance`'s own
    /// tests use.
    fn id(label: &str) -> Id {
        let mut safe = String::with_capacity(25);
        for ch in label.chars().take(25) {
            let mapped = match ch.to_ascii_uppercase() {
                upper @ ('0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z') => {
                    upper
                }
                'I' | 'L' => '1',
                'O' => '0',
                'U' => 'V',
                _ => '0',
            };
            safe.push(mapped);
        }
        Id::parse(&format!("0{safe:0>25}"))
            .expect("a sanitized, zero-padded, '0'-prefixed 26-character string always parses")
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    fn family(text: &str) -> ModelFamily {
        ModelFamily::parse(text).expect("a non-empty family parses")
    }

    fn open_db(scratch: &Scratch, product_id: &Id) -> ProductDb {
        ProductDb::open(&scratch.path, product_id.as_str(), at(1_000))
            .expect("a fresh product database opens")
    }

    fn identity(id_label: &str, product_id: Id, role: Role, family_text: &str) -> AgentIdentity {
        AgentIdentity::create(
            id(id_label),
            product_id,
            role,
            "fake-test-model",
            IdentityRuntime::Headless,
            MemoryScopes::default(),
            family(family_text),
        )
        .expect("a valid identity is created")
    }

    // -----------------------------------------------------------------------
    // A lead and a coder sharing a family: refused (plant 1 and plant 2's
    // positive case).
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_035_a_lead_registered_after_a_same_family_coder_is_refused() {
        let scratch = Scratch::new("lead-after-coder");
        let product_id = id("PRODUCT0000000000000000010");
        let mut db = open_db(&scratch, &product_id);

        let coder = identity(
            "CODER000000000000000000010",
            product_id.clone(),
            Role::Coder,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(2_000), Actor::System, &coder).expect("coder registers");

        let lead = identity(
            "LEAD0000000000000000000010",
            product_id.clone(),
            Role::Lead,
            "anthropic-claude-3",
        );
        let err = register_identity(&mut db, at(3_000), Actor::System, &lead)
            .expect_err("a lead sharing its coder's family is refused");
        assert!(matches!(
            err,
            IdentityRegistrationError::Family(FamilyError::Refused { .. })
        ));
        assert_eq!(
            err.methodology_ref(),
            Some(MethodologyRef {
                section: 7,
                subsection: None
            })
        );
    }

    #[test]
    fn ori_p1_035_a_coder_registered_after_a_same_family_lead_is_refused() {
        let scratch = Scratch::new("coder-after-lead");
        let product_id = id("PRODUCT0000000000000000011");
        let mut db = open_db(&scratch, &product_id);

        let lead = identity(
            "LEAD0000000000000000000011",
            product_id.clone(),
            Role::Lead,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(2_000), Actor::System, &lead).expect("lead registers");

        let coder = identity(
            "CODER000000000000000000011",
            product_id.clone(),
            Role::Coder,
            "anthropic-claude-3",
        );
        let err = register_identity(&mut db, at(3_000), Actor::System, &coder)
            .expect_err("a coder sharing its lead's family is refused (the reverse path)");
        assert!(matches!(
            err,
            IdentityRegistrationError::Family(FamilyError::Refused { .. })
        ));
    }

    #[test]
    fn ori_t_0108_distinct_families_are_allowed_in_both_directions() {
        let scratch = Scratch::new("distinct");
        let product_id = id("PRODUCT0000000000000000012");
        let mut db = open_db(&scratch, &product_id);

        let coder = identity(
            "CODER000000000000000000012",
            product_id.clone(),
            Role::Coder,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(2_000), Actor::System, &coder).expect("coder registers");

        let lead = identity(
            "LEAD0000000000000000000012",
            product_id.clone(),
            Role::Lead,
            "openai-gpt-4",
        );
        register_identity(&mut db, at(3_000), Actor::System, &lead)
            .expect("a lead on a distinct family from its coder is allowed");
    }

    // -----------------------------------------------------------------------
    // Roles ADR-0001 does not name are never refused.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0108_a_qa_identity_is_never_family_checked() {
        let scratch = Scratch::new("qa-unchecked");
        let product_id = id("PRODUCT0000000000000000013");
        let mut db = open_db(&scratch, &product_id);

        let lead = identity(
            "LEAD0000000000000000000013",
            product_id.clone(),
            Role::Lead,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(2_000), Actor::System, &lead).expect("lead registers");

        let qa = identity(
            "QA00000000000000000000013A",
            product_id.clone(),
            Role::Qa,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(3_000), Actor::System, &qa)
            .expect("a qa identity sharing a family with a lead is not refused: ADR-0001 names only lead/coder");
    }

    // -----------------------------------------------------------------------
    // The omission property (plant 3's positive demonstration): a lead
    // registered against three prior coders, only one of which shares its
    // family, is still refused. A read that vacuously missed even one of the
    // three would let this pass.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0108_every_prior_coder_is_read_back_not_only_the_first_or_the_last() {
        let scratch = Scratch::new("every-prior-coder");
        let product_id = id("PRODUCT0000000000000000014");
        let mut db = open_db(&scratch, &product_id);

        for (label, family_text) in [
            ("CODERA00000000000000000014", "openai-gpt-4"),
            ("CODERB00000000000000000014", "google-gemini"),
            ("CODERC00000000000000000014", "anthropic-claude-3"),
        ] {
            let coder = identity(label, product_id.clone(), Role::Coder, family_text);
            register_identity(&mut db, at(2_000), Actor::System, &coder).expect("coder registers");
        }

        // Read back directly: the three coders above, and only them.
        let events = read_all_events(&mut db).expect("events read back");
        let existing = identities_for_product(&events, &product_id).expect("identities parse");
        assert_eq!(
            existing.len(),
            3,
            "the read must find all three coders actually registered, not fewer"
        );

        // The third coder (only) shares the new lead's family; a read that
        // silently dropped it would let this lead through.
        let lead = identity(
            "LEAD0000000000000000000014",
            product_id.clone(),
            Role::Lead,
            "anthropic-claude-3",
        );
        let err = register_identity(&mut db, at(3_000), Actor::System, &lead)
            .expect_err("the lead shares the third coder's family and must be refused");
        assert!(matches!(
            err,
            IdentityRegistrationError::Family(FamilyError::Refused { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // Order: a refused registration appends nothing.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0108_a_refused_registration_appends_no_event() {
        let scratch = Scratch::new("refused-appends-nothing");
        let product_id = id("PRODUCT0000000000000000015");
        let mut db = open_db(&scratch, &product_id);

        let coder = identity(
            "CODER000000000000000000015",
            product_id.clone(),
            Role::Coder,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(2_000), Actor::System, &coder).expect("coder registers");

        let events_before = EventLog::verify(db.connection())
            .expect("log verifies before the refused attempt")
            .events_checked;

        let lead = identity(
            "LEAD0000000000000000000015",
            product_id.clone(),
            Role::Lead,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(3_000), Actor::System, &lead)
            .expect_err("the lead shares the coder's family and is refused");

        let events_after = EventLog::verify(db.connection())
            .expect("log verifies after the refused attempt")
            .events_checked;
        assert_eq!(
            events_before, events_after,
            "a refused registration must not append an identity.created event"
        );
    }

    // -----------------------------------------------------------------------
    // Product scoping: an identity in another product is never compared.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0108_identities_for_product_excludes_another_products_identity() {
        let scratch = Scratch::new("cross-product");
        let product_a = id("PRODUCTA00000000000000016");
        let product_b = id("PRODUCTB00000000000000016");

        let mut db_a = open_db(&scratch, &product_a);
        let coder_a = identity(
            "CODERA00000000000000000016",
            product_a.clone(),
            Role::Coder,
            "anthropic-claude-3",
        );
        let event_a = register_identity(&mut db_a, at(2_000), Actor::System, &coder_a)
            .expect("product A's coder registers");

        let mut db_b = open_db(&scratch, &product_b);
        let coder_b = identity(
            "CODERB00000000000000000016",
            product_b.clone(),
            Role::Coder,
            "openai-gpt-4",
        );
        let event_b = register_identity(&mut db_b, at(2_100), Actor::System, &coder_b)
            .expect("product B's coder registers");

        // Combine real events from two different products' logs into one
        // list directly, the only way to construct a mixed-product Vec<Event>
        // at all (a single ProductDb's own log cannot hold two products'
        // events: EventLog::append's own ProductMismatch refusal prevents
        // it), and confirm the filter, not the file layout, is what keeps
        // them apart.
        let combined = vec![event_a, event_b];
        let only_a = identities_for_product(&combined, &product_a).expect("identities parse");
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].id(), coder_a.id());

        let only_b = identities_for_product(&combined, &product_b).expect("identities parse");
        assert_eq!(only_b.len(), 1);
        assert_eq!(only_b[0].id(), coder_b.id());

        // And end to end: a lead in product A sharing product B's coder's
        // family is not refused, because product B's coder is never in the
        // set product A's own registration reads.
        let lead_a = identity(
            "LEADA0000000000000000016",
            product_a.clone(),
            Role::Lead,
            "openai-gpt-4",
        );
        register_identity(&mut db_a, at(3_000), Actor::System, &lead_a).expect(
            "a lead in product A sharing product B's coder's family must not be refused: \
             products are never compared against each other",
        );
    }

    // -----------------------------------------------------------------------
    // The payload round trips, and never carries a secret-shaped field.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0108_the_registered_identity_round_trips_every_field() {
        let scratch = Scratch::new("round-trip");
        let product_id = id("PRODUCT0000000000000000017");
        let mut db = open_db(&scratch, &product_id);

        let coder = identity(
            "CODER000000000000000000017",
            product_id.clone(),
            Role::Coder,
            "anthropic-claude-3",
        );
        register_identity(&mut db, at(2_000), Actor::System, &coder).expect("coder registers");

        let events = read_all_events(&mut db).expect("events read back");
        let existing = identities_for_product(&events, &product_id).expect("identities parse");
        assert_eq!(existing.len(), 1);
        let registered = &existing[0];
        assert_eq!(registered.id(), coder.id());
        assert_eq!(registered.product_id(), coder.product_id());
        assert_eq!(registered.role(), coder.role());
        assert_eq!(registered.model(), coder.model());
        assert_eq!(registered.family(), coder.family());
        assert_eq!(registered.runtime(), coder.runtime());

        assert!(!events[0].payload().to_lowercase().contains("secret"));
    }

    // -----------------------------------------------------------------------
    // A malformed identity.created payload is refused, not silently skipped.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0108_an_identity_created_event_missing_a_field_is_refused_not_silently_skipped() {
        let scratch = Scratch::new("malformed");
        let product_id = id("PRODUCT0000000000000000018");
        let mut db = open_db(&scratch, &product_id);

        // A hand-built payload missing "family" entirely, appended directly
        // (not through register_identity, which cannot build a malformed
        // one itself): models an event a future, buggier writer produced.
        EventLog::append(
            db.connection(),
            product_id.clone(),
            at(2_000),
            Actor::System,
            "identity.created",
            None,
            format!(
                "{{\"id\":\"{}\",\"product_id\":\"{}\",\"role\":\"coder\",\"model\":\"m\",\"runtime\":\"headless\"}}",
                id("IDENTITY000000000000000018").as_str(),
                product_id.as_str(),
            ),
        )
        .expect("the malformed row itself appends fine; EventLog does not parse payload");

        let events = read_all_events(&mut db).expect("events read back");
        let err = identities_for_product(&events, &product_id)
            .expect_err("a payload missing family must be refused, not read as zero identities");
        assert!(matches!(
            err,
            IdentityRegistrationError::Malformed {
                what: "identity.created family",
                ..
            }
        ));
    }
}
