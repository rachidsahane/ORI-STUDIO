//! `CredentialIssuance`: issuance, scoping and revocation on session end.
//! AICD §17, `spec/DATA_MODEL.md` section 2.
//!
//! Criterion ORI-P1-020 in `spec/criteria/phase-1.md`: "Any session ended (any
//! outcome) | Inspect issuances | Every issuance for the session is revoked
//! with a timestamp before the process is terminated." `spec/DATA_MODEL.md`
//! section 2's row: "**CredentialIssuance** | id, identity_id, session_id,
//! scope, issued_at, expires_at, revoked_at | Never stores the secret."
//! Section 4: "Every `CredentialIssuance` is bound to one session and expires
//! with it." `spec/SECURITY_NOTES.md` "Secrets": "Credential issuance is
//! recorded without the secret. Expiry is enforced by the issuing system
//! where possible (installation tokens) and by revocation on session end
//! otherwise."
//!
//! # Where issuances live
//!
//! This ticket's declared scope is this file alone (plus the one `pub mod
//! issuance;` line in `crates/ori-broker/src/lib.rs`); there is no
//! `credential_issuances` table, and adding one is a migration in
//! `crates/ori-store`, out of scope. The design this module uses instead,
//! consistent with CLAUDE.md's load-bearing fact that "the event log in
//! `ori-store` is append-only and hash-chained... Projections are derived",
//! is the one `crates/ori-broker/src/keychain.rs`'s own
//! `record_binding_resolved` already established for `ProviderBinding`
//! resolution: **record issuance and revocation as events**, through
//! [`ori_store::event_log::EventLog::append`], and answer "inspect issuances"
//! by reading the log back ([`issuances_for_session`]). Two event kinds:
//! `credential.issued` ([`issue_credential`]) and `credential.revoked`
//! ([`revoke_session`]). Every payload carries `id`, `identity_id`,
//! `session_id`, `scope` and timestamps, and never the secret: no local
//! variable in this file is ever bound to a [`crate::keychain::Secret`] or the
//! output of [`crate::keychain::Secret::expose`], the same absence
//! `keychain.rs`'s own `binding_resolved_payload` documents for itself, and
//! this module does not import [`crate::keychain::Secret`] at all, so there
//! is nothing here a payload builder could even reach for.
//!
//! # What "scope" is held as
//!
//! `spec/DATA_MODEL.md` and `spec/SECURITY_NOTES.md` name a `scope` field on
//! `CredentialIssuance` but state no grammar for it (no comma list, no
//! capability tag syntax). [`IssuanceScope`] is therefore one opaque,
//! trimmed, non-empty string, the same restraint `identity.rs`'s
//! `MemoryScopes` documents for the memory scope labels it holds ("nothing
//! here decides what a label means, only that it is present and non-empty").
//! A grammar is not invented here; a later ticket that needs one extends this
//! type once the specification states it.
//!
//! # Four traps, and how this module closes each one
//!
//! 1. **Vacuity** (AICD §39, "present but reporting nothing"). "Every
//!    issuance for the session is revoked" is trivially true of a session
//!    that never had one, or of a revoke whose read found nothing because of
//!    a query bug. [`revoke_session`] returns [`RevocationReceipt`], a value
//!    with no public constructor anywhere in this crate: the only way to
//!    build one is to call [`revoke_session`] itself, and its
//!    [`RevocationReceipt::revoked_count`] is exactly how many issuances that
//!    call found and revoked, not a boolean. A caller (this module's own
//!    tests, `ori-runtime`'s session-end path) that issued a known N
//!    credentials to a session and then reads `revoked_count() != N` back
//!    from the receipt has caught the vacuity trap the moment it happens,
//!    rather than trusting a bare `Ok(())`.
//! 2. **Scoping.** [`revoke_session`] and [`issuances_for_session`] both
//!    filter strictly on the `session_id` embedded in each event's own
//!    payload: an issuance for session B is never present in the projection
//!    [`issuances_for_session`] builds for session A (see
//!    `project_issuances`), so a `credential.revoked` event for A can only
//!    ever match ids already known to belong to A. Revoking A cannot reach
//!    B's issuances structurally, not only by care at one call site.
//! 3. **"Any outcome".** Neither [`issue_credential`] nor [`revoke_session`]
//!    takes an `AgentSession` outcome as a parameter at all: revocation is
//!    the same call regardless of why the session ended
//!    (`spec/DATA_MODEL.md` section 3's `AgentSession` state machine, "On any
//!    terminal state, credentials issued to the session are revoked", names
//!    four terminal states and draws no distinction between them for this
//!    purpose). `outcome (completed, blocked, escalated, killed)` is
//!    `ori-runtime::session::Outcome`, in a crate this one does not depend on
//!    (`ori-runtime` depends on `ori-broker`, `spec/LLD.md` section 2's
//!    dependency diagram, never the reverse), so this module cannot name that
//!    type; `tests::ori_p1_020_every_outcome_revokes_the_sessions_issuances`
//!    re-declares the same four labels locally and drives every one of them
//!    through this same, single, outcome-blind revocation path.
//! 4. **"Before the process is terminated."** Only `ori-runtime` spawns
//!    processes (CLAUDE.md load-bearing facts), so this module cannot itself
//!    enforce an ordering against a process it never holds a handle to. See
//!    "The ordering seam" below for exactly which half this module carries
//!    and which half it hands off.
//!
//! Revocation is idempotent: a second [`revoke_session`] call for a session
//! whose issuances are already all revoked finds nothing left to revoke and
//! appends no event, returning a receipt with
//! [`RevocationReceipt::revoked_count`] `0` rather than recording a second,
//! redundant set of revocations
//! (`tests::ori_t_0027_a_second_revoke_of_an_already_revoked_session_is_a_no_op`).
//! An issuance past [`Issuance::expires_at`] is treated as dead
//! ([`Issuance::is_active`] is `false`) whether or not it was ever formally
//! revoked, but [`revoke_session`] still revokes it if it has not been: an
//! expired-but-unrevoked issuance is exactly the row `spec/DATA_MODEL.md`
//! section 4's "expires with it" describes, and ORI-P1-020 asks that *every*
//! issuance for the session carry a `revoked_at`, not only the ones still
//! live.
//!
//! # The ordering seam: what this module guarantees, and what it hands off
//!
//! This module guarantees the **half before termination**: by the time
//! [`revoke_session`] returns `Ok`, the `credential.revoked` event it
//! produced (when it produced one) has already been committed through
//! [`ori_store::event_log::EventLog::append`], which itself only returns
//! after `tx.commit()` has succeeded. [`RevocationReceipt`] is the evidence
//! of that: it cannot be constructed except by a completed revocation, so a
//! caller that holds one is holding proof the revocation already happened,
//! durably, before this function returned control.
//!
//! What this module **cannot** guarantee is that the caller actually waits
//! for that `Ok` before terminating the process. `ori-runtime` is the only
//! crate that spawns and kills a session's process, so the discipline "hold
//! the receipt before you terminate" has to be enforced in
//! `ori-runtime`'s session-end path (ORI-T-0032, or wherever that path lives)
//! by that crate choosing to call [`revoke_session`] and match on its `Ok`
//! before issuing the kill, not by anything this module can observe or
//! block. This module makes that discipline checkable, not automatic: a
//! session-end path that skips the call has no receipt to point to, which is
//! a fact a reviewer or a future gate can look for, but not one this
//! function can refuse on its own, since it never runs on the same call
//! stack as the terminate call.
//!
//! ```mermaid
//! sequenceDiagram
//!   participant Runtime as ori-runtime (owns the process)
//!   participant Broker as ori-broker::issuance (this module)
//!   participant Log as ori-store EventLog
//!   Runtime->>Broker: issue_credential(session, scope, ...)
//!   Broker->>Log: append credential.issued
//!   Note over Runtime,Broker: session runs
//!   Runtime->>Runtime: session reaches a terminal outcome<br/>(completed, blocked, escalated, killed)
//!   Runtime->>Broker: revoke_session(session_id)
//!   Broker->>Log: read issuances for session_id
//!   Broker->>Log: append credential.revoked (if any unrevoked)
//!   Log-->>Broker: committed
//!   Broker-->>Runtime: RevocationReceipt (count, at)
//!   Note over Runtime: guaranteed by this module, up to here
//!   Runtime->>Runtime: terminate the process
//!   Note over Runtime: guaranteed by ori-runtime's own session-end<br/>path holding the receipt before this step, not by this module
//! ```
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md` section
//! 2, inherited from the crate). Nothing here reads or writes a secret;
//! payloads carry only `id`, `identity_id`, `session_id`, `scope` and
//! timestamps.

use core::fmt;
use std::collections::BTreeMap;

use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use ori_store::event_log::EventLogError;

use crate::identity::AgentIdentity;

// ---------------------------------------------------------------------------
// IssuanceError
// ---------------------------------------------------------------------------

/// A failure issuing, reading or revoking a credential.
///
/// `EventLog` wraps whatever [`ori_store::event_log::EventLog`] returned;
/// `Malformed` is this module's own parse failure over a payload it read
/// back. Neither is a refusal by a control of this module's own, the same
/// split `crate::identity::IdentityError` and
/// `crate::keychain::KeychainError` both draw for themselves; a genuine
/// refusal inside `EventLogError` (`MissingActor`, `SeqNotMonotonic`,
/// `PrevMismatch`) still carries its own `MethodologyRef`, reachable through
/// [`IssuanceError::methodology_ref`].
#[derive(Debug)]
#[non_exhaustive]
pub enum IssuanceError {
    /// A value did not parse into the type or shape named by `what`, either
    /// given to this module or read back out of an event payload this
    /// module itself wrote.
    Malformed {
        /// The field or shape the value was read as.
        what: &'static str,
        /// The value as it was given or found.
        value: String,
    },
    /// The event log refused or failed the call.
    EventLog(EventLogError),
}

impl IssuanceError {
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
    pub fn methodology_ref(&self) -> Option<ori_core::error::MethodologyRef> {
        match self {
            Self::EventLog(inner) => inner.methodology_ref(),
            Self::Malformed { .. } => None,
        }
    }
}

impl fmt::Display for IssuanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
            Self::EventLog(inner) => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for IssuanceError {}

impl From<EventLogError> for IssuanceError {
    fn from(err: EventLogError) -> Self {
        Self::EventLog(err)
    }
}

// ---------------------------------------------------------------------------
// IssuanceScope
// ---------------------------------------------------------------------------

/// The scope a credential was issued under: `spec/DATA_MODEL.md` section 2,
/// `CredentialIssuance.scope`.
///
/// Held as a single trimmed, non-empty string; see this module's own doc
/// comment, "What 'scope' is held as", for why no grammar is imposed beyond
/// that.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuanceScope(String);

impl IssuanceScope {
    /// Reads a scope, refusing one that is empty or only whitespace.
    pub fn parse(text: impl Into<String>) -> Result<Self, IssuanceError> {
        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(IssuanceError::malformed("scope", text));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The scope as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IssuanceScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Issuance
// ---------------------------------------------------------------------------

/// One credential issuance, projected from the event log:
/// `spec/DATA_MODEL.md` section 2's `CredentialIssuance` row.
///
/// Never constructed directly by a caller outside this module: every value
/// came from [`issuances_for_session`] reading back what [`issue_credential`]
/// and [`revoke_session`] actually wrote, the same discipline
/// `crate::identity::AgentIdentity`'s own doc comment states for itself
/// ("a value built any other way is not a promise this module made about the
/// identity").
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Issuance {
    id: Id,
    identity_id: Id,
    session_id: Id,
    scope: IssuanceScope,
    issued_at: Timestamp,
    expires_at: Option<Timestamp>,
    revoked_at: Option<Timestamp>,
}

impl Issuance {
    /// The issuance identifier.
    #[must_use]
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// The identity this credential was issued to.
    #[must_use]
    pub const fn identity_id(&self) -> &Id {
        &self.identity_id
    }

    /// The session this credential is bound to: `spec/DATA_MODEL.md` section
    /// 4, "Every `CredentialIssuance` is bound to one session and expires
    /// with it."
    #[must_use]
    pub const fn session_id(&self) -> &Id {
        &self.session_id
    }

    /// The scope this credential was issued under.
    #[must_use]
    pub const fn scope(&self) -> &IssuanceScope {
        &self.scope
    }

    /// When this credential was issued.
    #[must_use]
    pub const fn issued_at(&self) -> Timestamp {
        self.issued_at
    }

    /// When this credential expires on its own, absent when nothing but
    /// revocation on session end retires it (`spec/SECURITY_NOTES.md`
    /// "Secrets": "Expiry is enforced by the issuing system where possible
    /// ... and by revocation on session end otherwise").
    #[must_use]
    pub const fn expires_at(&self) -> Option<Timestamp> {
        self.expires_at
    }

    /// When this credential was revoked, absent until [`revoke_session`]
    /// revokes it.
    #[must_use]
    pub const fn revoked_at(&self) -> Option<Timestamp> {
        self.revoked_at
    }

    /// Whether this issuance has been revoked.
    #[must_use]
    pub const fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    /// Whether `now` is at or past [`Issuance::expires_at`]. `false` for an
    /// issuance with no `expires_at` at all: nothing but revocation retires
    /// one of those.
    #[must_use]
    pub fn is_expired(&self, now: Timestamp) -> bool {
        match self.expires_at {
            Some(expires_at) => now.millis() >= expires_at.millis(),
            None => false,
        }
    }

    /// Whether this issuance is still live: neither revoked nor expired.
    /// "An issuance past `expires_at` is treated as dead whether or not it
    /// was revoked" (this ticket's own instructions): expiry alone is
    /// enough to make this `false`, with no dependency on
    /// [`Issuance::is_revoked`].
    #[must_use]
    pub fn is_active(&self, now: Timestamp) -> bool {
        !self.is_revoked() && !self.is_expired(now)
    }
}

// ---------------------------------------------------------------------------
// RevocationReceipt
// ---------------------------------------------------------------------------

/// Evidence that a session's issuances were inspected and revoked: the
/// vacuity guard this module's own doc comment names under "Four traps",
/// point 1.
///
/// Every field is private and there is no public constructor anywhere in
/// this crate; the only way a value of this type exists at all is as
/// [`revoke_session`]'s own return value. A caller cannot fabricate "I
/// revoked N credentials" without actually calling [`revoke_session`] and
/// being handed back what it found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationReceipt {
    session_id: Id,
    revoked_count: u64,
    at: Timestamp,
}

impl RevocationReceipt {
    /// The session this revocation was for.
    #[must_use]
    pub const fn session_id(&self) -> &Id {
        &self.session_id
    }

    /// How many issuances this call actually revoked. `0` is a legitimate
    /// answer (a session with no issuances, or a second call after every
    /// issuance was already revoked; see this module's doc comment,
    /// "idempotent"), and is exactly what distinguishes "found nothing to
    /// do" from "found N and revoked them", which is the point of this type
    /// existing at all rather than [`revoke_session`] returning `()`.
    #[must_use]
    pub const fn revoked_count(&self) -> u64 {
        self.revoked_count
    }

    /// When this revocation was recorded.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }
}

// ---------------------------------------------------------------------------
// issue_credential
// ---------------------------------------------------------------------------

/// The fields [`issue_credential`] needs beyond `db`, `at`, `actor` and
/// `identity`, bundled into one value rather than four more plain
/// parameters: clippy's own threshold for a plain argument list is seven,
/// the same reason `ori_store::event_log::digest_row` takes a whole `Event`
/// rather than its fields separately (that function's own doc comment names
/// the same rule).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewIssuance {
    /// The issuance identifier. The caller's to supply: this module has no
    /// clock and no source of randomness, the same restraint
    /// [`crate::identity::AgentIdentity::create`] documents for itself.
    pub id: Id,
    /// The session this credential is bound to.
    pub session_id: Id,
    /// The scope this credential is issued under.
    pub scope: IssuanceScope,
    /// When this credential expires on its own, absent when only revocation
    /// on session end retires it.
    pub expires_at: Option<Timestamp>,
}

/// Records that a credential was issued to `identity`, under
/// `request.scope`, without the secret: `spec/SECURITY_NOTES.md` "Secrets",
/// "Credential issuance is recorded without the secret."
///
/// Writes one `credential.issued` event, through
/// [`ori_store::event_log::EventLog::append`], into `identity`'s own
/// product's log (`identity.product_id()`), the same routing
/// `crate::keychain::record_binding_resolved` uses.
///
/// # Errors
///
/// Whatever [`ori_store::event_log::EventLog::append`] returns; see that
/// function's own documentation.
pub fn issue_credential(
    db: &mut ProductDb,
    at: Timestamp,
    actor: Actor,
    identity: &AgentIdentity,
    request: NewIssuance,
) -> Result<Event, IssuanceError> {
    let payload = issued_payload(
        &request.id,
        identity.id(),
        &request.session_id,
        &request.scope,
        at,
        request.expires_at,
    );
    Ok(EventLog::append(
        db.connection(),
        identity.product_id().clone(),
        at,
        actor,
        "credential.issued",
        None,
        payload,
    )?)
}

// ---------------------------------------------------------------------------
// issuances_for_session
// ---------------------------------------------------------------------------

/// Answers "inspect issuances" (ORI-P1-020's own phrase) for `session_id`,
/// by reading `db`'s whole event log and projecting every
/// `credential.issued`/`credential.revoked` pair that names this session.
///
/// Takes `&mut ProductDb`, not the `Connection` underneath it, because
/// [`ori_store::db::ProductDb::connection`] itself only ever hands out a
/// `&mut`; this module never names the `rusqlite::Connection` type directly
/// (it would need `rusqlite` declared as a direct dependency of this crate to
/// do so, the same catch-up `ori-core` needed and documented in this crate's
/// own `Cargo.toml`, and does not need it since every call here is made and
/// consumed within one function body).
///
/// # Errors
///
/// Whatever reading or verifying the log returns, or a
/// [`IssuanceError::Malformed`] payload this module itself is responsible
/// for having written correctly in the first place.
pub fn issuances_for_session(
    db: &mut ProductDb,
    session_id: &Id,
) -> Result<Vec<Issuance>, IssuanceError> {
    let events = read_all_events(db)?;
    project_issuances(&events, session_id)
}

// ---------------------------------------------------------------------------
// revoke_session
// ---------------------------------------------------------------------------

/// Revokes every unrevoked issuance for `session_id`: ORI-P1-020, "Every
/// issuance for the session is revoked with a timestamp."
///
/// Reads the log ([`issuances_for_session`]), and, when at least one
/// issuance for this session is not yet revoked, appends exactly one
/// `credential.revoked` event carrying every such issuance's id and `at` as
/// its `revoked_at`, then returns a [`RevocationReceipt`] naming how many
/// that was. When nothing needs revoking (no issuance ever existed for this
/// session, or every one of them was already revoked by an earlier call),
/// no event is appended and the receipt reports `revoked_count() == 0`: see
/// this module's doc comment, "idempotent".
///
/// # Errors
///
/// Whatever reading or appending to the log returns.
pub fn revoke_session(
    db: &mut ProductDb,
    at: Timestamp,
    actor: Actor,
    session_id: &Id,
) -> Result<RevocationReceipt, IssuanceError> {
    let events = read_all_events(db)?;
    let issuances = project_issuances(&events, session_id)?;
    let to_revoke: Vec<&Issuance> = issuances.iter().filter(|i| !i.is_revoked()).collect();

    if to_revoke.is_empty() {
        return Ok(RevocationReceipt {
            session_id: session_id.clone(),
            revoked_count: 0,
            at,
        });
    }

    // Every issuance in `to_revoke` was read back from an event in
    // `events` (project_issuances only ever builds an Issuance from a
    // credential.issued row it found there), so `events` is not empty here.
    // Handled rather than asserted, per spec/CONVENTIONS.md "Rust", "No
    // unwrap, expect or panic! outside tests": a caller that somehow reaches
    // this branch with an empty `events` gets the same "nothing to revoke"
    // answer as a session with no issuances at all, not a panic.
    let Some(product_id) = events.first().map(Event::product_id) else {
        return Ok(RevocationReceipt {
            session_id: session_id.clone(),
            revoked_count: 0,
            at,
        });
    };

    let ids: Vec<&Id> = to_revoke.iter().map(|issuance| issuance.id()).collect();
    let payload = revoked_payload(session_id, at, &ids);
    let revoked_count = u64::try_from(to_revoke.len()).unwrap_or(u64::MAX);

    EventLog::append(
        db.connection(),
        product_id.clone(),
        at,
        actor,
        "credential.revoked",
        None,
        payload,
    )?;

    Ok(RevocationReceipt {
        session_id: session_id.clone(),
        revoked_count,
        at,
    })
}

// ---------------------------------------------------------------------------
// Reading the log
// ---------------------------------------------------------------------------

/// Reads every event currently in `db`'s log, the whole-log read
/// `issuances_for_session` and `revoke_session` both project over.
///
/// Goes through [`EventLog::verify`] first for its `tip_seq`, rather than a
/// raw `SELECT`, so that the same reader-completeness guard
/// [`EventLog::verify`] already carries
/// (`ori_store::event_log::EventLogError::ReaderIncomplete`) stands between
/// this module and a broken reader silently returning fewer rows than the
/// table holds; an empty log (`tip_seq` absent) is answered with an empty
/// `Vec` rather than a call to [`EventLog::read_range`] that has no valid
/// range to ask for. Calls `db.connection()` once per use (see
/// [`issuances_for_session`]'s own doc comment for why), each call ending
/// before the next begins, so nothing here holds two overlapping borrows of
/// `db` at once.
fn read_all_events(db: &mut ProductDb) -> Result<Vec<Event>, IssuanceError> {
    let report = EventLog::verify(db.connection())?;
    match report.tip_seq {
        Some(tip) => Ok(EventLog::read_range(db.connection(), 1, tip)?),
        None => Ok(Vec::new()),
    }
}

/// The pure half of [`issuances_for_session`]: given every event in the log,
/// builds every [`Issuance`] whose `session_id` is `session_id`, folding in
/// any `credential.revoked` event that also names `session_id` and this
/// issuance's own id.
///
/// Split out so [`revoke_session`] can read `events` once and use both the
/// projection and the raw events (for `product_id`) without a second trip
/// to the database.
fn project_issuances(events: &[Event], session_id: &Id) -> Result<Vec<Issuance>, IssuanceError> {
    let mut by_id: BTreeMap<String, Issuance> = BTreeMap::new();

    for event in events {
        match event.kind() {
            "credential.issued" => {
                let issuance = parse_issued(event)?;
                if issuance.session_id.as_str() == session_id.as_str() {
                    by_id.insert(issuance.id.as_str().to_owned(), issuance);
                }
            }
            "credential.revoked" => {
                let revoked = parse_revoked(event)?;
                if revoked.session_id.as_str() == session_id.as_str() {
                    for id in &revoked.issuance_ids {
                        if let Some(existing) = by_id.get_mut(id.as_str()) {
                            existing.revoked_at = Some(revoked.revoked_at);
                        }
                    }
                }
            }
            // Any other event kind in this product's log (credential.bound,
            // ticket.*, gate.*, and so on) is not this module's concern; see
            // ori_store::event_log's own doc comment on payload not being
            // interpreted beyond what a reader specifically looks for.
            _ => {}
        }
    }

    Ok(by_id.into_values().collect())
}

// ---------------------------------------------------------------------------
// Payload encoding
// ---------------------------------------------------------------------------

/// The payload [`issue_credential`] writes: `id`, `identity_id`,
/// `session_id`, `scope`, `issued_at`, `expires_at` (`null` when absent), and
/// nothing else. No field here is ever a secret; see this module's own doc
/// comment.
fn issued_payload(
    id: &Id,
    identity_id: &Id,
    session_id: &Id,
    scope: &IssuanceScope,
    issued_at: Timestamp,
    expires_at: Option<Timestamp>,
) -> String {
    let expires_at_json = match expires_at {
        Some(t) => t.millis().to_string(),
        None => "null".to_owned(),
    };
    format!(
        "{{\"id\":\"{}\",\"identity_id\":\"{}\",\"session_id\":\"{}\",\"scope\":\"{}\",\"issued_at\":{},\"expires_at\":{}}}",
        json_escape(id.as_str()),
        json_escape(identity_id.as_str()),
        json_escape(session_id.as_str()),
        json_escape(scope.as_str()),
        issued_at.millis(),
        expires_at_json,
    )
}

/// The payload [`revoke_session`] writes: `session_id`, `revoked_at`, and
/// `issuance_ids`, the array of every issuance id revoked by this one call.
/// One event covers every issuance this call revoked, so the revocation of
/// a session with several outstanding issuances is one atomic append, not
/// several partial ones.
fn revoked_payload(session_id: &Id, revoked_at: Timestamp, ids: &[&Id]) -> String {
    let mut out = format!(
        "{{\"session_id\":\"{}\",\"revoked_at\":{},\"issuance_ids\":[",
        json_escape(session_id.as_str()),
        revoked_at.millis(),
    );
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&json_escape(id.as_str()));
        out.push('"');
    }
    out.push_str("]}");
    out
}

/// Escapes `"` and `\` for the small hand-written JSON payloads above. This
/// crate has no JSON parser or serializer as a dependency (adding one is a
/// `new_dependency` escalation this ticket does not raise); `payload` is
/// stored and hashed as the bytes it was given, per `ori_store::event_log`'s
/// own doc comment, so the encoding here only needs to round-trip through
/// this module's own reader, not through any general-purpose consumer.
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

/// One decoded `credential.revoked` event.
struct Revoked {
    session_id: Id,
    revoked_at: Timestamp,
    issuance_ids: Vec<Id>,
}

/// Reads one `credential.issued` event back into an [`Issuance`], with
/// `revoked_at` absent (folded in separately by `project_issuances` from any
/// matching `credential.revoked` event).
fn parse_issued(event: &Event) -> Result<Issuance, IssuanceError> {
    let payload = event.payload();
    let id = parse_id_field(payload, "id")?;
    let identity_id = parse_id_field(payload, "identity_id")?;
    let session_id = parse_id_field(payload, "session_id")?;
    let scope_text = field_str(payload, "scope")
        .ok_or_else(|| IssuanceError::malformed("credential.issued scope", payload))?;
    let scope = IssuanceScope::parse(scope_text)?;
    let issued_at_millis = field_i64_required(payload, "issued_at")
        .ok_or_else(|| IssuanceError::malformed("credential.issued issued_at", payload))?;
    let expires_at_millis = field_i64_nullable(payload, "expires_at")
        .ok_or_else(|| IssuanceError::malformed("credential.issued expires_at", payload))?;

    Ok(Issuance {
        id,
        identity_id,
        session_id,
        scope,
        issued_at: Timestamp::from_millis(issued_at_millis),
        expires_at: expires_at_millis.map(Timestamp::from_millis),
        revoked_at: None,
    })
}

/// Reads one `credential.revoked` event back into its three fields.
fn parse_revoked(event: &Event) -> Result<Revoked, IssuanceError> {
    let payload = event.payload();
    let session_id = parse_id_field(payload, "session_id")?;
    let revoked_at_millis = field_i64_required(payload, "revoked_at")
        .ok_or_else(|| IssuanceError::malformed("credential.revoked revoked_at", payload))?;
    let id_texts = field_str_array(payload, "issuance_ids")
        .ok_or_else(|| IssuanceError::malformed("credential.revoked issuance_ids", payload))?;
    let issuance_ids = id_texts
        .into_iter()
        .map(|text| {
            Id::parse(&text)
                .map_err(|_| IssuanceError::malformed("credential.revoked issuance id", text))
        })
        .collect::<Result<Vec<Id>, IssuanceError>>()?;

    Ok(Revoked {
        session_id,
        revoked_at: Timestamp::from_millis(revoked_at_millis),
        issuance_ids,
    })
}

/// Reads a string field and parses it as an [`Id`], for the three id-shaped
/// string fields `parse_issued` and `parse_revoked` both read.
fn parse_id_field(payload: &str, key: &'static str) -> Result<Id, IssuanceError> {
    let text = field_str(payload, key).ok_or_else(|| IssuanceError::malformed(key, payload))?;
    Id::parse(&text).map_err(|_| IssuanceError::malformed(key, text))
}

/// Reads the quoted string value of `"key":"..."`, unescaping `\"` and
/// `\\`, `None` if `key` is not present as a quoted-string field.
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

/// The raw text of `"key":<value>` up to (not including) the next top-level
/// `,` or `}`, for the plain-number-or-`null` fields this module writes.
/// Never used for a quoted string field (see [`field_str`] for that), so a
/// comma or brace inside the value itself never arises.
fn find_value_span<'a>(payload: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\":");
    let start = payload.find(&needle)? + needle.len();
    let rest = &payload[start..];
    let end = rest.find([',', '}'])?;
    Some(rest[..end].trim())
}

/// Reads a required plain integer field, `None` if `key` is absent or does
/// not parse.
fn field_i64_required(payload: &str, key: &str) -> Option<i64> {
    find_value_span(payload, key)?.parse::<i64>().ok()
}

/// Reads a nullable plain integer field: `Some(None)` for a literal `null`,
/// `Some(Some(value))` for a number, `None` (the outer one) only if `key` is
/// entirely absent or the value is neither.
fn field_i64_nullable(payload: &str, key: &str) -> Option<Option<i64>> {
    let span = find_value_span(payload, key)?;
    if span == "null" {
        Some(None)
    } else {
        span.parse::<i64>().ok().map(Some)
    }
}

/// Reads a flat JSON array of quoted strings, `"key":["a","b"]`. Splits on
/// `,` and trims quotes, which is sufficient and only sufficient for the one
/// shape this module ever writes into `issuance_ids`: ULIDs, which contain
/// neither `,` nor `"` (`ori_core::types::Id`'s own alphabet is Crockford
/// base 32), so no element ever needs escaping. Not a general JSON array
/// reader; nothing in this module claims it is one.
fn field_str_array(payload: &str, key: &str) -> Option<Vec<String>> {
    let needle = format!("\"{key}\":[");
    let start = payload.find(&needle)? + needle.len();
    let rest = &payload[start..];
    let end = rest.find(']')?;
    let inner = rest[..end].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    Some(
        inner
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_owned())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use ori_core::types::Actor;
    use ori_core::types::Role;

    use super::*;
    use crate::identity::IdentityRuntime;
    use crate::identity::MemoryScopes;

    // -----------------------------------------------------------------------
    // Scratch layout, the same self-cleaning scratch directory pattern
    // crates/ori-broker/src/keychain.rs's own tests use.
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
                "ori-t-0027-{label}-{}-{unique}",
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
    /// `crates/ori-store/src/event_log.rs`'s own tests use.
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

    fn scope(text: &str) -> IssuanceScope {
        IssuanceScope::parse(text).expect("a non-empty scope")
    }

    fn coder_identity(product_id: Id, identity_id: Id) -> AgentIdentity {
        AgentIdentity::create(
            identity_id,
            product_id,
            Role::Coder,
            "fake-test-model",
            IdentityRuntime::Headless,
            MemoryScopes::default(),
            family("fake-test-family"),
        )
        .expect("a valid identity is created")
    }

    /// A declared model family for a test that does not care which, added by
    /// ORI-T-0108 alongside `AgentIdentity::create`'s new required
    /// parameter.
    fn family(text: &str) -> ori_core::types::ModelFamily {
        ori_core::types::ModelFamily::parse(text).expect("a non-empty family parses")
    }

    fn open_db(scratch: &Scratch, product_id: &Id) -> ProductDb {
        ProductDb::open(&scratch.path, product_id.as_str(), at(1_000))
            .expect("a fresh product database opens")
    }

    // -----------------------------------------------------------------------
    // IssuanceScope
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_issuance_scope_refuses_empty_or_whitespace() {
        for bad in ["", "   ", "\t\n"] {
            let err = IssuanceScope::parse(bad).expect_err("an empty scope is refused");
            assert!(matches!(
                err,
                IssuanceError::Malformed { what: "scope", .. }
            ));
        }
    }

    #[test]
    fn ori_t_0027_issuance_scope_is_trimmed() {
        let scope = IssuanceScope::parse("  repository:write  ").expect("a valid scope");
        assert_eq!(scope.as_str(), "repository:write");
    }

    // -----------------------------------------------------------------------
    // ORI-P1-020, the vacuity trap: issue a known, non-trivial number of
    // credentials, assert that count is what a fresh read reports, then
    // revoke and assert every one is revoked with a timestamp, and that the
    // receipt itself reports the same count. A revoke that read zero
    // issuances (a query bug) would make revoked_count() 0 here, not 3, and
    // this test would fail loudly rather than reading "revoked nothing" as
    // "revoked everything".
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_020_every_issuance_for_the_session_is_revoked_with_a_timestamp() {
        let scratch = Scratch::new("vacuity");
        let product_id = id("PRODUCT0000000000000000001");
        let identity_id = id("IDENTITY000000000000000001");
        let session_id = id("SESSION0000000000000000001");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        let mut db = open_db(&scratch, &product_id);

        // A known, non-trivial number: three, not one, so a revoke that only
        // ever touches "the first issuance it finds" cannot pass this test
        // by accident.
        for label in ["A", "B", "C"] {
            issue_credential(
                &mut db,
                at(2_000),
                Actor::Agent(identity_id.clone()),
                &identity,
                NewIssuance {
                    id: id(&format!("ISSUANCE0000000000000{label}")),
                    session_id: session_id.clone(),
                    scope: scope("repository:read"),
                    expires_at: None,
                },
            )
            .expect("issuance is recorded");
        }

        // Before revoking: the read itself must report the non-trivial
        // count, not a vacuous zero, and none of the three is revoked yet.
        let before = issuances_for_session(&mut db, &session_id)
            .expect("issuances for the session read back");
        assert_eq!(
            before.len(),
            3,
            "the read must find the three issuances actually recorded, not zero"
        );
        assert!(before.iter().all(|i| !i.is_revoked()));

        let receipt =
            revoke_session(&mut db, at(3_000), Actor::System, &session_id).expect("revoke");
        assert_eq!(
            receipt.revoked_count(),
            3,
            "the receipt must report the number actually revoked, not a vacuous zero over an \
             empty read"
        );
        assert_eq!(receipt.session_id(), &session_id);
        assert_eq!(receipt.at(), at(3_000));

        let after = issuances_for_session(&mut db, &session_id)
            .expect("issuances for the session read back again");
        assert_eq!(after.len(), 3);
        for issuance in &after {
            assert!(
                issuance.is_revoked(),
                "every issuance for the session must be revoked: {issuance:?}"
            );
            assert_eq!(issuance.revoked_at(), Some(at(3_000)));
        }
    }

    // -----------------------------------------------------------------------
    // ORI-P1-020, the scoping trap: revoking session A must not touch
    // session B's issuances.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_020_revoking_one_session_never_revokes_another_sessions_issuances() {
        let scratch = Scratch::new("scoping");
        let product_id = id("PRODUCT0000000000000000002");
        let identity_id = id("IDENTITY000000000000000002");
        let session_a = id("SESSIONA000000000000000002");
        let session_b = id("SESSIONB000000000000000002");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        let mut db = open_db(&scratch, &product_id);

        issue_credential(
            &mut db,
            at(2_000),
            Actor::Agent(identity_id.clone()),
            &identity,
            NewIssuance {
                id: id("ISSUANCEA00000000000000002"),
                session_id: session_a.clone(),
                scope: scope("repository:read"),
                expires_at: None,
            },
        )
        .expect("session A's issuance is recorded");
        issue_credential(
            &mut db,
            at(2_100),
            Actor::Agent(identity_id.clone()),
            &identity,
            NewIssuance {
                id: id("ISSUANCEB00000000000000002"),
                session_id: session_b.clone(),
                scope: scope("repository:read"),
                expires_at: None,
            },
        )
        .expect("session B's issuance is recorded");

        let receipt =
            revoke_session(&mut db, at(3_000), Actor::System, &session_a).expect("revoke A");
        assert_eq!(receipt.revoked_count(), 1);

        let a_after =
            issuances_for_session(&mut db, &session_a).expect("session A's issuances read back");
        assert_eq!(a_after.len(), 1);
        assert!(a_after[0].is_revoked(), "session A's issuance is revoked");

        let b_after =
            issuances_for_session(&mut db, &session_b).expect("session B's issuances read back");
        assert_eq!(b_after.len(), 1);
        assert!(
            !b_after[0].is_revoked(),
            "revoking session A must not revoke session B's issuance, and it did: {:?}",
            b_after[0]
        );
    }

    // -----------------------------------------------------------------------
    // ORI-P1-020, "any outcome": revocation is the same call regardless of
    // why the session ended. See this module's own doc comment, "Four
    // traps", point 3, for why the four labels are declared locally rather
    // than imported from ori-runtime.
    // -----------------------------------------------------------------------

    /// Mirrors `ori_runtime::session::Outcome`'s four values and spellings
    /// (`spec/DATA_MODEL.md` section 2's `AgentSession.outcome`), declared
    /// locally because this crate does not and must not depend on
    /// `ori-runtime` (`spec/LLD.md` section 2's dependency diagram runs the
    /// other way). This enum exists only so the test below can loop over an
    /// exhaustive `match` that fails to compile if a fifth variant is added
    /// to it without a matching arm; it does not, and cannot, track
    /// `ori-runtime`'s own enum gaining a variant, which is named as a
    /// limitation in this ticket's report rather than left implicit.
    #[derive(Clone, Copy, Debug)]
    enum TestOutcome {
        Completed,
        Blocked,
        Escalated,
        Killed,
    }

    impl TestOutcome {
        const ALL: &'static [Self] = &[
            Self::Completed,
            Self::Blocked,
            Self::Escalated,
            Self::Killed,
        ];

        /// Exhaustive on purpose, no `_` arm: adding a variant to
        /// `TestOutcome` above without adding one here fails the build,
        /// which is what "fails to compile if a variant is added" (this
        /// ticket's own text) asks for.
        const fn label(self) -> &'static str {
            match self {
                Self::Completed => "completed",
                Self::Blocked => "blocked",
                Self::Escalated => "escalated",
                Self::Killed => "killed",
            }
        }
    }

    #[test]
    fn ori_p1_020_every_outcome_revokes_the_sessions_issuances() {
        let scratch = Scratch::new("any-outcome");
        let product_id = id("PRODUCT0000000000000000003");
        let identity_id = id("IDENTITY000000000000000003");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        let mut db = open_db(&scratch, &product_id);

        for (index, outcome) in TestOutcome::ALL.iter().enumerate() {
            let session_id = id(&format!("SESSION-OUTCOME-{index}"));
            issue_credential(
                &mut db,
                at(2_000),
                Actor::Agent(identity_id.clone()),
                &identity,
                NewIssuance {
                    id: id(&format!("ISSUANCE-OUTCOME-{index}")),
                    session_id: session_id.clone(),
                    scope: scope("repository:read"),
                    expires_at: None,
                },
            )
            .expect("issuance recorded");

            // The call ori-runtime's session-end path makes is this one,
            // unconditionally, whatever outcome.label() says; killed is the
            // one CLAUDE.md and this ticket both single out as the path most
            // likely to have cleanup skipped, and it gets no different
            // treatment here than any other outcome.
            let receipt = revoke_session(&mut db, at(3_000), Actor::System, &session_id)
                .unwrap_or_else(|err| panic!("revoke after outcome {}: {err}", outcome.label()));
            assert_eq!(
                receipt.revoked_count(),
                1,
                "outcome {} must still revoke the session's issuance",
                outcome.label()
            );

            let after = issuances_for_session(&mut db, &session_id).expect("issuances read back");
            assert_eq!(after.len(), 1);
            assert!(
                after[0].is_revoked(),
                "outcome {} left an unrevoked issuance: {:?}",
                outcome.label(),
                after[0]
            );
        }
    }

    // -----------------------------------------------------------------------
    // Idempotency: a second revoke of an already-revoked session records
    // nothing further.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_a_second_revoke_of_an_already_revoked_session_is_a_no_op() {
        let scratch = Scratch::new("idempotent");
        let product_id = id("PRODUCT0000000000000000004");
        let identity_id = id("IDENTITY000000000000000004");
        let session_id = id("SESSION0000000000000000004");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        let mut db = open_db(&scratch, &product_id);

        issue_credential(
            &mut db,
            at(2_000),
            Actor::Agent(identity_id.clone()),
            &identity,
            NewIssuance {
                id: id("ISSUANCE0000000000000004A"),
                session_id: session_id.clone(),
                scope: scope("repository:read"),
                expires_at: None,
            },
        )
        .expect("issuance recorded");

        let first =
            revoke_session(&mut db, at(3_000), Actor::System, &session_id).expect("first revoke");
        assert_eq!(first.revoked_count(), 1);

        let events_before = EventLog::verify(db.connection())
            .expect("log verifies after the first revoke")
            .events_checked;

        let second =
            revoke_session(&mut db, at(4_000), Actor::System, &session_id).expect("second revoke");
        assert_eq!(
            second.revoked_count(),
            0,
            "nothing is left to revoke the second time"
        );

        let events_after = EventLog::verify(db.connection())
            .expect("log still verifies after the second revoke")
            .events_checked;
        assert_eq!(
            events_before, events_after,
            "a second revoke of an already-revoked session must not record a second set of \
             revocations"
        );

        let after = issuances_for_session(&mut db, &session_id).expect("issuances read back");
        assert_eq!(after.len(), 1);
        assert_eq!(
            after[0].revoked_at(),
            Some(at(3_000)),
            "the original revocation timestamp must not be overwritten by the no-op second call"
        );
    }

    // -----------------------------------------------------------------------
    // Idempotency, the vacuous-session case: revoking a session that never
    // had an issuance is also a no-op, and is distinguishable (by
    // revoked_count) from "revoked something".
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_revoking_a_session_with_no_issuances_reports_zero_not_an_error() {
        let scratch = Scratch::new("no-issuances");
        let product_id = id("PRODUCT0000000000000000005");
        let session_id = id("SESSION0000000000000000005");
        let mut db = open_db(&scratch, &product_id);

        let receipt = revoke_session(&mut db, at(3_000), Actor::System, &session_id)
            .expect("revoking a session with nothing issued still succeeds");
        assert_eq!(receipt.revoked_count(), 0);

        let report = EventLog::verify(db.connection()).expect("an empty log still verifies");
        assert_eq!(
            report.events_checked, 0,
            "no credential.revoked event is written when there was nothing to revoke"
        );
    }

    // -----------------------------------------------------------------------
    // Expiry: an issuance past expires_at is dead whether or not it was
    // revoked, but revoke_session still stamps revoked_at on it.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_an_issuance_past_expiry_is_inactive_even_before_it_is_revoked() {
        let scratch = Scratch::new("expiry");
        let product_id = id("PRODUCT0000000000000000006");
        let identity_id = id("IDENTITY000000000000000006");
        let session_id = id("SESSION0000000000000000006");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        let mut db = open_db(&scratch, &product_id);

        issue_credential(
            &mut db,
            at(2_000),
            Actor::Agent(identity_id.clone()),
            &identity,
            NewIssuance {
                id: id("ISSUANCE0000000000000006A"),
                session_id: session_id.clone(),
                scope: scope("repository:read"),
                expires_at: Some(at(2_500)),
            },
        )
        .expect("issuance with an expiry recorded");

        let issuances = issuances_for_session(&mut db, &session_id).expect("issuances read back");
        let issuance = &issuances[0];
        assert!(!issuance.is_revoked(), "not yet revoked");
        assert!(
            issuance.is_expired(at(3_000)),
            "past its own expires_at, this must read as expired"
        );
        assert!(
            !issuance.is_active(at(3_000)),
            "expired and unrevoked is still not active"
        );

        // revoke_session still revokes it: expiry does not exempt it from
        // ORI-P1-020's "every issuance for the session is revoked".
        let receipt = revoke_session(&mut db, at(4_000), Actor::System, &session_id)
            .expect("revoke an already-expired issuance");
        assert_eq!(receipt.revoked_count(), 1);
    }

    // -----------------------------------------------------------------------
    // The secret never appears in a payload. Not a criterion this ticket
    // names directly, but the same discipline record_binding_resolved's own
    // test enforces, and this module writes payloads by hand the same way.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_scope_is_recorded_but_no_secret_shaped_field_exists_to_leak() {
        let scratch = Scratch::new("no-secret-field");
        let product_id = id("PRODUCT0000000000000000007");
        let identity_id = id("IDENTITY000000000000000007");
        let session_id = id("SESSION0000000000000000007");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        let mut db = open_db(&scratch, &product_id);

        let event = issue_credential(
            &mut db,
            at(2_000),
            Actor::Agent(identity_id.clone()),
            &identity,
            NewIssuance {
                id: id("ISSUANCE0000000000000007A"),
                session_id: session_id.clone(),
                scope: scope("repository:read"),
                expires_at: None,
            },
        )
        .expect("issuance recorded");

        // The payload's fields are exactly the six named in this module's
        // own doc comment: id, identity_id, session_id, scope, issued_at,
        // expires_at. No seventh field, secret-shaped or otherwise.
        let payload = event.payload();
        for expected_key in [
            "\"id\":",
            "\"identity_id\":",
            "\"session_id\":",
            "\"scope\":",
            "\"issued_at\":",
            "\"expires_at\":",
        ] {
            assert!(
                payload.contains(expected_key),
                "the payload is missing {expected_key}: {payload}"
            );
        }
        assert!(!payload.to_lowercase().contains("secret"));
    }

    // -----------------------------------------------------------------------
    // The full products root, byte for byte, is scanned for a fake secret
    // string, the same "any event or log" discipline
    // crate::keychain::tests::ori_p1_037_... uses, applied here even though
    // no secret is ever given to this module to leak: this is a structural
    // guarantee (there is no Secret-typed field in scope), and this test
    // records that the discipline was checked, not merely asserted in prose.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_no_file_under_the_products_root_ever_holds_a_secret_shaped_string() {
        let scratch = Scratch::new("disk-scan");
        let product_id = id("PRODUCT0000000000000000008");
        let identity_id = id("IDENTITY000000000000000008");
        let session_id = id("SESSION0000000000000000008");
        let identity = coder_identity(product_id.clone(), identity_id.clone());
        {
            let mut db = open_db(&scratch, &product_id);
            issue_credential(
                &mut db,
                at(2_000),
                Actor::Agent(identity_id.clone()),
                &identity,
                NewIssuance {
                    id: id("ISSUANCE0000000000000008A"),
                    session_id: session_id.clone(),
                    scope: scope("repository:read"),
                    expires_at: None,
                },
            )
            .expect("issuance recorded");
            revoke_session(&mut db, at(3_000), Actor::System, &session_id).expect("revoke");
        }

        let bytes = read_every_file_under(&scratch.path);
        assert!(
            !bytes.is_empty(),
            "the durable files must have been written"
        );
        let haystack = String::from_utf8_lossy(&bytes).to_lowercase();
        assert!(!haystack.contains("fake-secret-for-tests-should-never-appear"));
    }

    /// Every byte of every regular file under `root`, the same whole-tree
    /// scan `crate::keychain::tests` uses.
    fn read_every_file_under(root: &std::path::Path) -> Vec<u8> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    stack.push(path);
                } else if file_type.is_file()
                    && let Ok(contents) = fs::read(&path)
                {
                    out.extend(contents);
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // A malformed payload (a query bug's cousin: a reader that finds the
    // right row but misparses it) is refused rather than silently treated
    // as absent.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0027_a_credential_issued_event_missing_a_field_is_refused_not_silently_skipped() {
        let scratch = Scratch::new("malformed");
        let product_id = id("PRODUCT0000000000000000009");
        let session_id = id("SESSION0000000000000000009");
        let mut db = open_db(&scratch, &product_id);

        // A hand-built payload missing "scope" entirely, appended directly
        // (not through issue_credential, which cannot build a malformed one
        // itself): models an event a future, buggier writer produced.
        EventLog::append(
            db.connection(),
            product_id.clone(),
            at(2_000),
            Actor::System,
            "credential.issued",
            None,
            format!(
                "{{\"id\":\"{}\",\"identity_id\":\"{}\",\"session_id\":\"{}\",\"issued_at\":2000,\"expires_at\":null}}",
                id("ISSUANCE0000000000000009A").as_str(),
                id("IDENTITY000000000000000009").as_str(),
                session_id.as_str(),
            ),
        )
        .expect("the malformed row itself appends fine; EventLog does not parse payload");

        let err = issuances_for_session(&mut db, &session_id)
            .expect_err("a payload missing scope must be refused, not read as zero issuances");
        assert!(matches!(
            err,
            IssuanceError::Malformed {
                what: "credential.issued scope",
                ..
            }
        ));
    }
}
