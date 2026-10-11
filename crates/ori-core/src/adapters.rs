//! The adapter traits, declared in `ori-core`, and the types in their
//! signatures: AICD §5, AICD §27, `spec/API_SPEC.md` section 4,
//! `spec/LLD.md` section 2.
//!
//! AICD §5 makes the engine open to extension and closed to modification: an
//! integration is an adapter behind a trait, registered by name, and the
//! engine never learns which one it holds. `spec/API_SPEC.md` section 4 lists
//! the seven traits; `spec/LLD.md` section 2 (as amended by ORI-T-0127, on the
//! operator's answer to escalation E-0002) says where they live: here, in the
//! crate every other crate may depend on, "together with the types in their
//! signatures", so that the crate that implements a trait and the crate that
//! holds it as a trait object both reach it from below and need no dependency
//! on each other.
//!
//! # What is declared here, and where it is implemented
//!
//! | Trait | Declared | Implemented in |
//! |---|---|---|
//! | [`VcsHost`], [`ErrorTracking`], [`Analytics`], [`Notification`], [`Ci`] | here | `ori-integrations` (ORI-T-0059 and later) |
//! | [`ModelProvider`] | here | `ori-integrations`; `spec/LLD.md` section 2 does not say where, see its doc |
//! | `AgentRuntime` | **in progress**: staged as ORI-T-0141, see below | `ori-runtime` (`AcpClient`, `HeadlessAdapter`) |
//!
//! # `AgentRuntime`: the move is in progress, staged as ORI-T-0141
//!
//! `spec/API_SPEC.md` section 4 and `spec/LLD.md` section 2 say all seven
//! traits are declared in `ori-core`, and `AgentRuntime` will be; it is not
//! here yet. By the operator's decision (ruling R42, 2026-10-11) its move is
//! staged as ORI-T-0141, after ORI-T-0140, and this ticket (ORI-T-0058) lands
//! the six slot traits and `AgentIdentity`. Nothing is rejected: the trait
//! stays where it is, declared in `ori-runtime`'s `acp.rs`, implemented by
//! `AcpClient` and `HeadlessAdapter`, and untouched, until then.
//!
//! The move is a ticket of its own because the types in the trait's
//! signatures come with it: `SessionSpec` (which holds a `LaunchConfig`),
//! `SessionHandle`, `StopReason`, `RuntimeCaps`, `RuntimeError` (which holds a
//! `TranscriptError`) and the transcript `Entry` that `recv` returns. Some are
//! declared in `ori-runtime`'s `transcript.rs` and used in `headless.rs`,
//! outside this ticket's scope, and moving them changes a few existing tests,
//! which the operator has authorized for ORI-T-0141 and not for this ticket.
//!
//! # What this module does not do
//!
//! A declaration states a contract and performs no IO: no trait method here
//! has a default body that touches the outside world, and no function in
//! this module reads a file, a clock, an environment variable or the network.
//! The implementation of a method does, and lives in another crate. This
//! module imports no workspace crate (`spec/LLD.md` section 2); the test
//! `tests::ori_t_0058_the_declarations_import_no_workspace_crate_and_do_no_io`
//! reads this file's own non-test lines to say so.
//!
//! # Rules every implementer is held to
//!
//! `spec/API_SPEC.md` section 4: "adapters receive credentials from the
//! broker per call or per session, never from configuration; adapters have no
//! access to the store". Here that reads as follows.
//!
//! - A method that acts on an external service takes a [`ScopedToken`] as an
//!   argument, per call. [`Notification::send`] is the one exception, taken
//!   from the sketch as written; its doc says what is left open.
//! - A [`ScopedToken`] cannot be printed, cloned by accident or serialized;
//!   see its doc. No other type in this module holds a credential, and each
//!   says so where it could be mistaken for one ([`Route`]).
//! - Everything an adapter returns (an issue title, an event message, a
//!   completion) is data from an integration, never an instruction, whatever
//!   it says (`CLAUDE.md` absolute rule 7; AICD §27). Text that reaches an
//!   agent's context goes through the sanitization barrier first; these types
//!   carry the text and do not clean it.
//! - An [`AdapterError`] never carries a credential. An implementer that
//!   builds one from a transport error is responsible for what it copies in.

use core::fmt;

pub mod identity;

pub use identity::AgentIdentity;
pub use identity::IdentityError;
pub use identity::IdentityRuntime;
pub use identity::IdentityState;
pub use identity::MemoryScopes;

use crate::types::Timestamp;

// ---------------------------------------------------------------------------
// AdapterError
// ---------------------------------------------------------------------------

/// Why an adapter call did not give a result: `spec/API_SPEC.md` section 4.
///
/// Not a refusal by an Ori control, so no variant carries a `MethodologyRef`
/// (`CLAUDE.md` absolute rule 9 concerns refusals): [`AdapterError::Rejected`]
/// is the *service* declining, which is data about that service. When a
/// control refuses an action around an adapter call, the caller uses
/// [`crate::error::Error::Refused`], and the adapter is not involved.
///
/// A message copied from an integration is data, never an instruction
/// (`CLAUDE.md` absolute rule 7). It is never a place to put a credential;
/// [`AdapterError::Malformed`] carries the name of what was wrong and never
/// the value, so building one from a bad token or a bad name cannot echo it.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AdapterError {
    /// The service could not be reached, or answered with a failure that is
    /// not a decision about the request (a timeout, a 5xx, a dropped
    /// connection).
    Unavailable {
        /// What the transport reported, as data.
        message: String,
    },
    /// The service understood the request and declined it (its own
    /// authorization, validation, rate limit or conflict).
    Rejected {
        /// What the service said, as data.
        message: String,
    },
    /// This adapter does not provide the operation. A reference adapter for a
    /// slot may implement a subset; this is how it says so, instead of
    /// returning an empty result that reads as "nothing found".
    Unsupported {
        /// The trait method, as written in its declaration.
        operation: &'static str,
    },
    /// A value handed to a constructor in this module was blank.
    Malformed {
        /// The type being built. Never the value.
        what: &'static str,
    },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { message } => write!(f, "the service is unavailable: {message}"),
            Self::Rejected { message } => write!(f, "the service rejected the request: {message}"),
            Self::Unsupported { operation } => {
                write!(f, "this adapter does not provide {operation}")
            }
            Self::Malformed { what } => write!(f, "not a valid {what}: blank"),
        }
    }
}

impl std::error::Error for AdapterError {}

// ---------------------------------------------------------------------------
// ScopedToken
// ---------------------------------------------------------------------------

/// A credential an adapter acts with, scoped to what it may do:
/// `spec/API_SPEC.md` section 4, AICD §27, `spec/SECURITY_NOTES.md`
/// "Secrets".
///
/// `spec/SECURITY_NOTES.md` "Secrets": "Secrets are never logged, never
/// serialized into events, never returned by any RPC." A type cannot stop a
/// caller from choosing to leak what it holds; it can remove the paths that
/// leak it without anyone choosing to. This type follows what `ori-broker`'s
/// `Secret` does for the keychain, and removes these:
///
/// 1. **No derived `Debug`.** [`fmt::Debug`] is written by hand and prints the
///    scope and the word `REDACTED`, never the value, so a container that
///    derives `Debug` over a token redacts it too.
/// 2. **`Display` redacts as well**, for the log line that reaches for it.
/// 3. **No `Serialize`, structurally.** `ori-core` has no dependencies, so
///    the trait is not nameable here; adding it would first need a
///    dependency, which is an escalation (`new_dependency`) and not a
///    missing `impl` block.
/// 4. **No `Clone`, `PartialEq`, `Default`, `Deref`, `AsRef<str>` or
///    `Into<String>`.** A copy would be a second place the value lives, and
///    an equality failure message prints both sides. The one way to read the
///    value is [`ScopedToken::expose`], which hands a `&str` to a closure.
///
/// What it does not attempt: zeroing its buffer on drop, which needs `unsafe`
/// or a dependency this ticket may not add.
///
/// Who makes one: the broker issues the credential, and the adapter's
/// `VcsHost::identity_token` (or the engine, for the other slots) wraps it;
/// this module declares the type and issues nothing (`CLAUDE.md`: "Only
/// `ori-broker` issues credentials"). The `scope` is a description of what the
/// token may do, written for a human ("repo ori/studio, contents write"); it
/// is not a secret and is printed. A scope string must never contain the
/// credential.
pub struct ScopedToken {
    value: Box<str>,
    scope: String,
}

impl ScopedToken {
    /// Holds `value` as a credential limited to `scope`.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Malformed`] when `value` or `scope` is empty or only
    /// whitespace. The error names `"token value"` or `"token scope"` and
    /// never the text.
    pub fn new(value: impl Into<Box<str>>, scope: impl Into<String>) -> Result<Self, AdapterError> {
        let value = value.into();
        let scope = scope.into();
        if value.trim().is_empty() {
            return Err(AdapterError::Malformed {
                what: "token value",
            });
        }
        if scope.trim().is_empty() {
            return Err(AdapterError::Malformed {
                what: "token scope",
            });
        }
        Ok(Self { value, scope })
    }

    /// What this token may do, in words. Not a secret.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Hands the credential, as `&str`, to `f`, and returns only what `f`
    /// returns. The one way to read the value: a closure that copies it out
    /// can always do so, but it is a visible act at the call site.
    pub fn expose<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(&self.value)
    }
}

impl fmt::Debug for ScopedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedToken")
            .field("scope", &self.scope)
            .field("value", &"REDACTED")
            .finish()
    }
}

impl fmt::Display for ScopedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REDACTED")
    }
}

// ---------------------------------------------------------------------------
// Opaque names
// ---------------------------------------------------------------------------

/// Declares a type that holds one non-blank piece of text an adapter hands
/// out and later takes back (a pull request reference, a CI run reference, a
/// notification route). No methodology section applies: this is a local
/// device that keeps the constructor and the accessors of these types the
/// same.
macro_rules! opaque_text {
    ($(#[$meta:meta])* $name:ident, $what:literal) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct $name(String);

        impl $name {
            /// Holds `text`, trimmed.
            ///
            /// # Errors
            ///
            /// [`AdapterError::Malformed`] when `text` is empty or only
            /// whitespace.
            pub fn new(text: &str) -> Result<Self, AdapterError> {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Err(AdapterError::Malformed { what: $what });
                }
                Ok(Self(trimmed.to_owned()))
            }

            /// The text, as the adapter that made it wrote it.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

opaque_text!(
    /// A pull request, named the way its host names it: `spec/API_SPEC.md`
    /// section 4. Opaque to everything but the adapter that returned it
    /// ([`VcsHost::open_pr`]); a caller hands it back and does not parse it.
    PullRequestRef,
    "pull request reference"
);

opaque_text!(
    /// One CI run, named the way its system names it: `spec/API_SPEC.md`
    /// section 4. Opaque to everything but the adapter that returned it
    /// ([`Ci::trigger`]).
    CiRunRef,
    "CI run reference"
);

opaque_text!(
    /// A commit, named the way its host names it: `spec/API_SPEC.md` section 4,
    /// AICD §13. What [`PullRequestStatus::head`] reports and what
    /// [`VcsHost::merge`] is told to expect: the commit the gates passed on.
    /// Compared by the adapter, as text, against the host's answer; not
    /// parsed by a caller.
    CommitId,
    "commit id"
);

opaque_text!(
    /// Where a notification goes, by name: a channel key or a recipient key
    /// the adapter resolves (`spec/API_SPEC.md` section 4, `Notification`).
    ///
    /// **Not a credential and must not hold one.** A destination that is
    /// itself a secret URL (some chat systems issue one) is not a `Route`:
    /// the route names it, and the adapter obtains the URL from the broker.
    /// Hence this type derives `Debug` and `Display` and prints its text.
    Route,
    "notification route"
);

// ---------------------------------------------------------------------------
// VcsHost
// ---------------------------------------------------------------------------

/// Where a pull request stands: `spec/API_SPEC.md` section 4,
/// [`VcsHost::pr_status`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PullRequestState {
    /// Open and not merged.
    Open,
    /// Merged. The only state [`VcsHost::merge`] leads to.
    Merged,
    /// Closed without merging.
    Closed,
}

/// The outcome of one named check on a pull request: AICD §13,
/// [`VcsHost::set_check`].
///
/// **Fail closed.** [`CheckOutcome::Passed`] is the host's explicit statement
/// of success and nothing else. Every other host conclusion maps to
/// [`CheckOutcome::Failed`] (the check finished without success: failure,
/// cancelled, skipped, timed out, neutral, stale, action required) or to
/// [`CheckOutcome::Pending`] (it has not finished, or the adapter cannot tell
/// whether it has), never to `Passed`. A conclusion the adapter does not
/// recognise is `Failed`. There is deliberately no `Cancelled`, `Skipped` or
/// `Unknown` variant: a caller that waits for `Passed` must not have a
/// fourth way to be told something other than "pass".
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CheckOutcome {
    /// Not finished, or the adapter cannot tell.
    Pending,
    /// The host said it succeeded, explicitly.
    Passed,
    /// Finished without success, or not recognised.
    Failed,
}

/// One named check and its outcome: AICD §13. What the gates report to the
/// host through [`VcsHost::set_check`] and what [`VcsHost::pr_status`] reads
/// back.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CheckReport {
    /// The check's name as the host shows it (a gate name, in the engine's
    /// own use). Data.
    pub name: String,
    /// How it stands.
    pub outcome: CheckOutcome,
}

/// What [`VcsHost::pr_status`] returns: `spec/API_SPEC.md` section 4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestStatus {
    /// Where the pull request stands.
    pub state: PullRequestState,
    /// The commit the pull request's head is at, as the host reports it at
    /// the moment of this answer. A caller that gates a pull request records
    /// this, runs the gates on exactly this commit, and passes it back to
    /// [`VcsHost::merge`] as the head to expect (CI_CD section 2: the merge
    /// queue re-runs the gates on the rebased head, then merges).
    pub head: CommitId,
    /// The checks the host holds for `head`, in the host's order. See
    /// [`VcsHost::pr_status`] for what an implementer owes here.
    pub checks: Vec<CheckReport>,
}

/// What an adapter asks the webhook receiver to deliver to it:
/// `spec/API_SPEC.md` section 4, [`VcsHost::webhooks`].
///
/// **Declared for the receiver and carries no secret.** The shared secret that
/// verifies a delivery is obtained from the broker by the receiver. Webhook
/// verification is inbound trust (`spec/RISK_MAP.md` tiers it 2 in
/// `ori-integrations`), and its scheme is deliberately not fixed here: which
/// header carries the signature, which algorithm, how replay is refused. That
/// is for the first implementer (ORI-T-0059 and the webhook receiver), who
/// extends this type as a reviewed change.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WebhookSpec {
    /// The host's own names for the events the adapter wants delivered.
    pub events: Vec<String>,
}

/// The version-control host slot: `spec/API_SPEC.md` section 4, AICD §13.
///
/// One implementation per host, in `ori-integrations`; the engine holds it as
/// a trait object. Object safe: every method takes `&self`, none is generic.
/// It carries no `Send` or `Sync` bound; a holder that shares it across
/// threads names `dyn VcsHost + Send + Sync`.
///
/// # `merge`, and where its restriction is enforced
///
/// `spec/API_SPEC.md` section 4: "the `merge` method exists only on the
/// `VcsHost` trait object held by the merge queue", and `CLAUDE.md`: "Only
/// `ori-orchestrator::merge_queue` calls `VcsHost::merge`". Declaring the
/// method in `ori-core` makes it *nameable* from every crate. The rule is
/// about who is **handed the object**: `ori-engine`, the composition root,
/// gives the merge queue the adapter that `ori-integrations` implements, and
/// no other holder is given one it is meant to merge with. **No mechanical
/// check enforces this yet.** A crate that holds a `dyn VcsHost` for another
/// purpose could call `merge` and the compiler would accept it; today the
/// guard is the rule, review, and the tier 2 status of the merge queue and of
/// `ori-integrations`' merge exposure (`spec/RISK_MAP.md`). Two ways to make
/// it mechanical are recorded here for the ticket that owns the merge queue
/// (ORI-T-0053): a grep-style gate over the workspace for the call, or
/// splitting `merge` into a sub-trait that only the merge queue holds. Neither
/// is built or decided here.
///
/// # Left for the first implementer (ORI-T-0059)
///
/// The sketch elides every argument but one. These are the smallest honest
/// ones: a branch is named by text, a pull request by [`PullRequestRef`]. Not
/// decided: how a host that has no per-identity tokens answers
/// [`VcsHost::identity_token`], how the host's own pagination is followed
/// (that the whole list is read is decided: see [`VcsHost::pr_status`]),
/// whether [`VcsHost::merge`] returns the merge commit, merge strategy, and
/// the signature scheme of [`WebhookSpec`].
pub trait VcsHost {
    /// A token limited to what `identity`'s role may do on this host
    /// (AICD §17). The adapter obtains the credential from the broker; it is
    /// never read from configuration.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the host cannot issue one.
    fn identity_token(&self, identity: &AgentIdentity) -> Result<ScopedToken, AdapterError>;

    /// Creates `branch` from `from` (a branch name or a commit the host
    /// resolves).
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the host declines or is unavailable.
    fn create_branch(
        &self,
        token: &ScopedToken,
        branch: &str,
        from: &str,
    ) -> Result<(), AdapterError>;

    /// Opens a pull request from `head` into `base`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the host declines or is unavailable.
    fn open_pr(
        &self,
        token: &ScopedToken,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<PullRequestRef, AdapterError>;

    /// Where `pr` stands, with its head commit and the checks the host holds
    /// for that head.
    ///
    /// An implementation owes two things, both so that a caller waiting for
    /// every check to be [`CheckOutcome::Passed`] cannot be told "yes" by
    /// mistake:
    ///
    /// - **Fail closed.** A host conclusion other than an explicit success
    ///   maps to [`CheckOutcome::Failed`] or [`CheckOutcome::Pending`], never
    ///   to `Passed` (see [`CheckOutcome`]).
    /// - **All of it or an error.** `checks` holds every check the host has
    ///   for `head`: the implementation follows the host's pagination to the
    ///   end, or returns [`AdapterError::Unavailable`]. It never returns a
    ///   partial page, because a check missing from the list looks to a
    ///   caller like a check that does not exist. An empty `checks` says the
    ///   host holds none for `head`; it is not a pass. Which checks must be
    ///   present is the caller's rule (the merge queue's), not the adapter's.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the host declines or is unavailable, or when the
    /// complete list could not be read.
    fn pr_status(
        &self,
        token: &ScopedToken,
        pr: &PullRequestRef,
    ) -> Result<PullRequestStatus, AdapterError>;

    /// Merges `pr`, **only if its head is `expected_head`**. Called from
    /// `ori-orchestrator::merge_queue` only; see the trait's doc for where
    /// that is enforced and that nothing checks it mechanically yet.
    ///
    /// `expected_head` is the commit the gates passed on. `spec/CI_CD.md`
    /// section 2: the merge queue rebases, re-runs the gates on the rebased
    /// head, then merges. A push between the gate run and the merge would
    /// otherwise merge a head the gates never saw, so the head is part of the
    /// request, not something the host is trusted to resolve.
    ///
    /// An implementation **must refuse** ([`AdapterError::Rejected`]) and
    /// merge nothing when the pull request's head at the time of the merge is
    /// not `expected_head`. It should hand `expected_head` to the host's own
    /// compare-and-set where the host has one, so that the check and the
    /// merge are one host operation; where the host has none, the
    /// implementation says in its own docs what window remains between its
    /// check and its merge. Passing the check to a host that ignores the
    /// parameter does not meet this rule.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Rejected`] when the head differs, a required check
    /// failed, or there is a conflict; [`AdapterError`] otherwise when the
    /// host declines or is unavailable.
    ///
    /// **After [`AdapterError::Unavailable`] the outcome is unknown**: the
    /// host may have merged before the connection dropped. The caller reads
    /// [`VcsHost::pr_status`] (state and head) and decides from that; it does
    /// not call `merge` again blindly.
    fn merge(
        &self,
        token: &ScopedToken,
        pr: &PullRequestRef,
        expected_head: &CommitId,
    ) -> Result<(), AdapterError>;

    /// Sets a named check on the commit `head` of `pr`.
    ///
    /// `head` is the commit the gates ran on, named by the caller for the same
    /// reason [`VcsHost::merge`] names the head to expect: a result reported
    /// against "whatever the head is now" could mark a commit the gates never
    /// ran on as passed, and a host's branch protection would then let it
    /// through. The check attaches to exactly `head`; an implementation must
    /// not attach it to a different commit because the pull request moved.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the host declines or is unavailable.
    fn set_check(
        &self,
        token: &ScopedToken,
        pr: &PullRequestRef,
        head: &CommitId,
        check: &CheckReport,
    ) -> Result<(), AdapterError>;

    /// What this host wants delivered to the webhook receiver. Does no IO.
    fn webhooks(&self) -> WebhookSpec;
}

// ---------------------------------------------------------------------------
// ErrorTracking
// ---------------------------------------------------------------------------

/// One grouped error an error tracker reports: AICD §16.
///
/// Every text field is production content from an integration: data, never
/// instructions, and it reaches an agent only through the sanitization
/// barrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorIssue {
    /// The tracker's own identifier for the issue, passed back to
    /// [`ErrorTracking::issue_events`].
    pub id: String,
    /// The tracker's title for it. Data.
    pub title: String,
    /// How many times it has occurred, as the tracker counts.
    pub occurrences: u64,
}

/// One occurrence of an [`ErrorIssue`]: AICD §16. Production content: see
/// [`ErrorIssue`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorEvent {
    /// When it happened, as the tracker reports.
    pub at: Timestamp,
    /// The message the tracker recorded. Data.
    pub message: String,
}

/// The error-tracking slot: `spec/API_SPEC.md` section 4, AICD §16. Read
/// only. Object safe.
///
/// Left for the first implementer: pagination and cursors (here a plain
/// `limit`), filtering by environment or time window, stack-trace structure
/// (here a message), and the scrubbing of personal data in a message before
/// it is returned.
pub trait ErrorTracking {
    /// The most recent issues, newest first, at most `limit`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the tracker declines or is unavailable.
    fn recent_issues(
        &self,
        token: &ScopedToken,
        limit: usize,
    ) -> Result<Vec<ErrorIssue>, AdapterError>;

    /// The most recent occurrences of the issue the tracker names `issue_id`,
    /// newest first, at most `limit`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the tracker declines or is unavailable.
    fn issue_events(
        &self,
        token: &ScopedToken,
        issue_id: &str,
        limit: usize,
    ) -> Result<Vec<ErrorEvent>, AdapterError>;
}

// ---------------------------------------------------------------------------
// Analytics
// ---------------------------------------------------------------------------

/// A named group of users an analytics system defines: AICD §16.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    /// The system's identifier for it, passed back to [`Analytics::events`].
    pub id: String,
    /// Its name. Data.
    pub name: String,
}

/// One analytics event: AICD §16. Production content: data, never
/// instructions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalyticsEvent {
    /// When it happened.
    pub at: Timestamp,
    /// The event's name as the system records it. Data.
    pub name: String,
}

/// The product-analytics slot: `spec/API_SPEC.md` section 4, AICD §16. Read
/// only. Object safe.
///
/// Left for the first implementer: properties on events, time windows,
/// aggregation (here raw events), pagination, and anonymization of anything
/// user-identifying before it leaves the adapter.
pub trait Analytics {
    /// The segments the system defines.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the system declines or is unavailable.
    fn segments(&self, token: &ScopedToken) -> Result<Vec<Segment>, AdapterError>;

    /// The most recent events of the segment the system names `segment_id`,
    /// newest first, at most `limit`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the system declines or is unavailable.
    fn events(
        &self,
        token: &ScopedToken,
        segment_id: &str,
        limit: usize,
    ) -> Result<Vec<AnalyticsEvent>, AdapterError>;
}

// ---------------------------------------------------------------------------
// Notification
// ---------------------------------------------------------------------------

/// What a notification says: `spec/API_SPEC.md` section 4. Text for a human;
/// never holds a credential, and the engine builds it from its own data, not
/// from adapter output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payload {
    /// A one-line summary.
    pub title: String,
    /// The body.
    pub body: String,
}

/// The notification slot: `spec/API_SPEC.md` section 4. Object safe.
///
/// The sketch is `send(&self, route: Route, payload: Payload)` and passes no
/// token; this declaration keeps that, with a `Result` added. How a
/// notification adapter obtains the credential its channel needs (per
/// session, from the broker, held by the adapter) is left for the first
/// implementer; if it turns out to be per call, this signature changes with a
/// reviewed contract change, not silently. Also left: delivery guarantees,
/// retries, rate limits and formatting.
pub trait Notification {
    /// Delivers `payload` to `route`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the channel declines or is unavailable.
    fn send(&self, route: Route, payload: Payload) -> Result<(), AdapterError>;
}

// ---------------------------------------------------------------------------
// Ci
// ---------------------------------------------------------------------------

/// Where a CI run stands: `spec/API_SPEC.md` section 4, [`Ci::status`].
///
/// **Fail closed**, as [`CheckOutcome`] is. [`CiRunStatus::Succeeded`] is the
/// system's explicit statement of success and nothing else. A run that was
/// cancelled, skipped, timed out, ended neutral or in a state the adapter does
/// not recognise is [`CiRunStatus::Failed`]; a run not yet started or not yet
/// finished is [`CiRunStatus::Queued`] or [`CiRunStatus::Running`]. There is
/// deliberately no `Cancelled` variant: it would be a way to say "not
/// succeeded" that a caller testing for `Succeeded` must remember to treat as
/// a failure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CiRunStatus {
    /// Waiting to start.
    Queued,
    /// Running, or the adapter cannot tell that it has finished.
    Running,
    /// The system said it succeeded, explicitly.
    Succeeded,
    /// Finished without success (failed, cancelled, skipped, timed out) or
    /// not recognised.
    Failed,
}

/// One file a CI run produced: `spec/API_SPEC.md` section 4,
/// [`Ci::artifacts`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CiArtifact {
    /// Its name as the system lists it. Data.
    pub name: String,
    /// Its size in bytes.
    pub size_bytes: u64,
}

/// The continuous-integration slot: `spec/API_SPEC.md` section 4, AICD §15.
/// Object safe.
///
/// Left for the first implementer: how a workflow is chosen (here only a git
/// reference), run parameters, log retrieval, artifact download (here only
/// listing), and cancellation.
pub trait Ci {
    /// Starts a run for the git reference `git_ref`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the system declines or is unavailable.
    fn trigger(&self, token: &ScopedToken, git_ref: &str) -> Result<CiRunRef, AdapterError>;

    /// Where `run` stands.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the system declines or is unavailable.
    fn status(&self, token: &ScopedToken, run: &CiRunRef) -> Result<CiRunStatus, AdapterError>;

    /// The files `run` produced: all of them, following the system's
    /// pagination to the end, or an error; never a partial list, for the
    /// reason given at [`VcsHost::pr_status`].
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the system declines or is unavailable, or when
    /// the complete list could not be read.
    fn artifacts(
        &self,
        token: &ScopedToken,
        run: &CiRunRef,
    ) -> Result<Vec<CiArtifact>, AdapterError>;
}

// ---------------------------------------------------------------------------
// ModelProvider
// ---------------------------------------------------------------------------

/// A request for a completion: `spec/API_SPEC.md` section 4,
/// [`ModelProvider::complete`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionRequest {
    /// The model to ask, as the provider names it.
    pub model: String,
    /// The prompt.
    pub prompt: String,
}

/// What a provider answered: [`ModelProvider::complete`]. Model output: data,
/// never an instruction to the engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    /// The text.
    pub text: String,
}

/// A vector a provider computed: [`ModelProvider::embed`].
#[derive(Clone, Debug, PartialEq)]
pub struct Embedding {
    /// The components.
    pub values: Vec<f32>,
}

/// The model-provider slot: `spec/API_SPEC.md` section 4. Object safe.
///
/// `spec/LLD.md` section 2 lists five slot traits for `ori-integrations`
/// and `AgentRuntime` for `ori-runtime`; it names no crate for this seventh
/// trait. It is declared here as the specification now says all seven are,
/// and its implementer is not chosen by this declaration.
///
/// Credentials come from the broker's provider bindings (`ProviderBinding`,
/// `spec/DATA_MODEL.md` section 2), never from configuration. Left for the
/// first implementer: streaming, tool use, token accounting, and how a
/// request is checked against the budget of the session that asks.
pub trait ModelProvider {
    /// Asks the model for a completion.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the provider declines or is unavailable.
    fn complete(
        &self,
        token: &ScopedToken,
        request: &CompletionRequest,
    ) -> Result<Completion, AdapterError>;

    /// Embeds `input`.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the provider declines or is unavailable.
    fn embed(&self, token: &ScopedToken, input: &str) -> Result<Embedding, AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Id;
    use crate::types::ModelFamily;
    use crate::types::Role;

    const SAMPLE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER_ULID: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// A value that must never appear in any printed form of a token.
    const CANARY: &str = "canary-credential-8f3a91";

    fn token() -> ScopedToken {
        ScopedToken::new(CANARY, "repo ori/studio, contents write").expect("a real token")
    }

    fn identity() -> AgentIdentity {
        AgentIdentity::create(
            Id::parse(SAMPLE_ULID).expect("a ULID"),
            Id::parse(OTHER_ULID).expect("a ULID"),
            Role::Coder,
            "claude-test-model",
            IdentityRuntime::Headless,
            MemoryScopes::new(["canonical"]).expect("one scope"),
            ModelFamily::parse("claude-3").expect("a family"),
        )
        .expect("a valid identity")
    }

    // -----------------------------------------------------------------------
    // Fakes: one per trait, so that `dyn Trait` is built, not just named.
    // -----------------------------------------------------------------------

    struct FakeVcs;

    impl VcsHost for FakeVcs {
        fn identity_token(&self, identity: &AgentIdentity) -> Result<ScopedToken, AdapterError> {
            ScopedToken::new(CANARY, format!("role {}", identity.role()))
        }
        fn create_branch(&self, _: &ScopedToken, _: &str, _: &str) -> Result<(), AdapterError> {
            Ok(())
        }
        fn open_pr(
            &self,
            _: &ScopedToken,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<PullRequestRef, AdapterError> {
            PullRequestRef::new("pr-7")
        }
        fn pr_status(
            &self,
            _: &ScopedToken,
            _: &PullRequestRef,
        ) -> Result<PullRequestStatus, AdapterError> {
            Ok(PullRequestStatus {
                state: PullRequestState::Open,
                head: CommitId::new("aaa111")?,
                checks: vec![CheckReport {
                    name: "gate 2".to_owned(),
                    outcome: CheckOutcome::Passed,
                }],
            })
        }
        fn merge(
            &self,
            _: &ScopedToken,
            _: &PullRequestRef,
            _: &CommitId,
        ) -> Result<(), AdapterError> {
            Err(AdapterError::Rejected {
                message: "a required check is pending".to_owned(),
            })
        }
        fn set_check(
            &self,
            _: &ScopedToken,
            _: &PullRequestRef,
            _: &CommitId,
            _: &CheckReport,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
        fn webhooks(&self) -> WebhookSpec {
            WebhookSpec {
                events: vec!["pull_request".to_owned()],
            }
        }
    }

    struct FakeErrors;

    impl ErrorTracking for FakeErrors {
        fn recent_issues(
            &self,
            _: &ScopedToken,
            _: usize,
        ) -> Result<Vec<ErrorIssue>, AdapterError> {
            Ok(vec![ErrorIssue {
                id: "i-1".to_owned(),
                title: "boom".to_owned(),
                occurrences: 3,
            }])
        }
        fn issue_events(
            &self,
            _: &ScopedToken,
            _: &str,
            _: usize,
        ) -> Result<Vec<ErrorEvent>, AdapterError> {
            Err(AdapterError::Unsupported {
                operation: "issue_events",
            })
        }
    }

    struct FakeAnalytics;

    impl Analytics for FakeAnalytics {
        fn segments(&self, _: &ScopedToken) -> Result<Vec<Segment>, AdapterError> {
            Ok(Vec::new())
        }
        fn events(
            &self,
            _: &ScopedToken,
            _: &str,
            _: usize,
        ) -> Result<Vec<AnalyticsEvent>, AdapterError> {
            Ok(Vec::new())
        }
    }

    struct FakeNotification;

    impl Notification for FakeNotification {
        fn send(&self, _: Route, _: Payload) -> Result<(), AdapterError> {
            Ok(())
        }
    }

    struct FakeCi;

    impl Ci for FakeCi {
        fn trigger(&self, _: &ScopedToken, _: &str) -> Result<CiRunRef, AdapterError> {
            CiRunRef::new("run-1")
        }
        fn status(&self, _: &ScopedToken, _: &CiRunRef) -> Result<CiRunStatus, AdapterError> {
            Ok(CiRunStatus::Running)
        }
        fn artifacts(
            &self,
            _: &ScopedToken,
            _: &CiRunRef,
        ) -> Result<Vec<CiArtifact>, AdapterError> {
            Ok(Vec::new())
        }
    }

    struct FakeModels;

    impl ModelProvider for FakeModels {
        fn complete(
            &self,
            _: &ScopedToken,
            request: &CompletionRequest,
        ) -> Result<Completion, AdapterError> {
            Ok(Completion {
                text: request.prompt.clone(),
            })
        }
        fn embed(&self, _: &ScopedToken, _: &str) -> Result<Embedding, AdapterError> {
            Ok(Embedding {
                values: vec![0.0, 1.0],
            })
        }
    }

    // -----------------------------------------------------------------------
    // Object safety
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0058_vcs_host_is_object_safe_and_merge_is_reachable_on_the_trait_object() {
        let host: Box<dyn VcsHost> = Box::new(FakeVcs);
        let identity = identity();
        let token = host.identity_token(&identity).expect("a token");
        assert_eq!(token.scope(), "role coder");

        let pr = host
            .open_pr(&token, "feat/x", "main", "title", "body")
            .expect("a pull request");
        let status = host.pr_status(&token, &pr).expect("a status");
        assert_eq!(status.state, PullRequestState::Open);
        assert_eq!(status.checks.len(), 1);
        host.create_branch(&token, "feat/y", "main")
            .expect("a branch");
        host.set_check(
            &token,
            &pr,
            &status.head,
            &CheckReport {
                name: "gate 2".to_owned(),
                outcome: CheckOutcome::Failed,
            },
        )
        .expect("a check");
        assert_eq!(host.webhooks().events, vec!["pull_request".to_owned()]);

        // The method is nameable on the object, which is the point of
        // declaring it here; that only the merge queue calls it is not
        // something this test, or any test yet, can say (see the trait doc).
        let refusal = host
            .merge(&token, &pr, &status.head)
            .expect_err("the fake declines");
        assert!(matches!(refusal, AdapterError::Rejected { .. }));
    }

    /// A reference host that keeps the merge rule: it merges only the head it
    /// was told to expect. It shows what an implementer owes; the trait cannot
    /// make it so, and the test below is about the contract's shape, not
    /// about any real host.
    struct PinningVcs {
        head: std::sync::Mutex<CommitId>,
        state: std::sync::Mutex<PullRequestState>,
    }

    impl PinningVcs {
        fn at(head: &str) -> Self {
            Self {
                head: std::sync::Mutex::new(CommitId::new(head).expect("a commit")),
                state: std::sync::Mutex::new(PullRequestState::Open),
            }
        }

        /// Someone pushes to the pull request's branch.
        fn push(&self, head: &str) {
            *self.head.lock().expect("not poisoned") = CommitId::new(head).expect("a commit");
        }
    }

    impl VcsHost for PinningVcs {
        fn identity_token(&self, _: &AgentIdentity) -> Result<ScopedToken, AdapterError> {
            Err(AdapterError::Unsupported {
                operation: "identity_token",
            })
        }
        fn create_branch(&self, _: &ScopedToken, _: &str, _: &str) -> Result<(), AdapterError> {
            Ok(())
        }
        fn open_pr(
            &self,
            _: &ScopedToken,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<PullRequestRef, AdapterError> {
            PullRequestRef::new("pr-7")
        }
        fn pr_status(
            &self,
            _: &ScopedToken,
            _: &PullRequestRef,
        ) -> Result<PullRequestStatus, AdapterError> {
            Ok(PullRequestStatus {
                state: *self.state.lock().expect("not poisoned"),
                head: self.head.lock().expect("not poisoned").clone(),
                checks: Vec::new(),
            })
        }
        fn merge(
            &self,
            _: &ScopedToken,
            _: &PullRequestRef,
            expected_head: &CommitId,
        ) -> Result<(), AdapterError> {
            let head = self.head.lock().expect("not poisoned");
            if &*head != expected_head {
                return Err(AdapterError::Rejected {
                    message: "the head is not the one the gates passed on".to_owned(),
                });
            }
            *self.state.lock().expect("not poisoned") = PullRequestState::Merged;
            Ok(())
        }
        fn set_check(
            &self,
            _: &ScopedToken,
            _: &PullRequestRef,
            _: &CommitId,
            _: &CheckReport,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
        fn webhooks(&self) -> WebhookSpec {
            WebhookSpec::default()
        }
    }

    #[test]
    fn ori_t_0058_merge_is_pinned_to_the_head_the_gates_passed_on() {
        let pinning = PinningVcs::at("aaa111");
        let host: &dyn VcsHost = &pinning;
        let token = token();
        let pr = PullRequestRef::new("pr-7").expect("a reference");

        // The gates run on the head the status reports.
        let gated = host.pr_status(&token, &pr).expect("a status").head;
        assert_eq!(gated.as_str(), "aaa111");

        // A push lands between the gate run and the merge. The merge names
        // the commit the gates passed on, so it is refused and nothing merges.
        pinning.push("bbb222");
        let refusal = host
            .merge(&token, &pr, &gated)
            .expect_err("the head moved after the gates ran");
        assert!(matches!(refusal, AdapterError::Rejected { .. }));
        let after = host.pr_status(&token, &pr).expect("a status");
        assert_eq!(after.state, PullRequestState::Open);
        assert_eq!(after.head.as_str(), "bbb222");

        // After a refusal the caller re-reads, gates the new head, and pins
        // the merge to that one.
        host.merge(&token, &pr, &after.head)
            .expect("the head is the one named");
        assert_eq!(
            host.pr_status(&token, &pr).expect("a status").state,
            PullRequestState::Merged
        );
    }

    #[test]
    fn ori_t_0058_the_five_other_slot_traits_are_object_safe() {
        let token = token();
        let errors: Box<dyn ErrorTracking> = Box::new(FakeErrors);
        assert_eq!(errors.recent_issues(&token, 5).expect("issues").len(), 1);
        assert!(matches!(
            errors.issue_events(&token, "i-1", 5),
            Err(AdapterError::Unsupported {
                operation: "issue_events"
            })
        ));

        let analytics: Box<dyn Analytics> = Box::new(FakeAnalytics);
        assert!(analytics.segments(&token).expect("segments").is_empty());
        assert!(
            analytics
                .events(&token, "s-1", 5)
                .expect("events")
                .is_empty()
        );

        let notification: Box<dyn Notification> = Box::new(FakeNotification);
        notification
            .send(
                Route::new("ops-channel").expect("a route"),
                Payload {
                    title: "t".to_owned(),
                    body: "b".to_owned(),
                },
            )
            .expect("sent");

        let ci: Box<dyn Ci> = Box::new(FakeCi);
        let run = ci.trigger(&token, "main").expect("a run");
        assert_eq!(
            ci.status(&token, &run).expect("a status"),
            CiRunStatus::Running
        );
        assert!(ci.artifacts(&token, &run).expect("artifacts").is_empty());

        let models: Box<dyn ModelProvider> = Box::new(FakeModels);
        let completion = models
            .complete(
                &token,
                &CompletionRequest {
                    model: "m".to_owned(),
                    prompt: "hello".to_owned(),
                },
            )
            .expect("a completion");
        assert_eq!(completion.text, "hello");
        assert_eq!(
            models
                .embed(&token, "x")
                .expect("an embedding")
                .values
                .len(),
            2
        );
    }

    #[test]
    fn ori_t_0058_a_trait_object_can_be_shared_across_threads_when_the_holder_says_so() {
        // The traits carry no Send or Sync bound; a holder that shares one
        // names `dyn VcsHost + Send + Sync`, which a plain implementer meets.
        let host: std::sync::Arc<dyn VcsHost + Send + Sync> = std::sync::Arc::new(FakeVcs);
        let other = std::sync::Arc::clone(&host);
        let events = std::thread::spawn(move || other.webhooks().events)
            .join()
            .expect("the thread finishes");
        assert_eq!(events, host.webhooks().events);
    }

    // -----------------------------------------------------------------------
    // Secrets
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0058_a_scoped_token_never_prints_its_value() {
        let token = token();
        let debug = format!("{token:?}");
        let alternate = format!("{token:#?}");
        let display = format!("{token}");
        for printed in [&debug, &alternate, &display] {
            assert!(
                !printed.contains(CANARY),
                "a printed form of the token holds its value: {printed}"
            );
            assert!(printed.contains("REDACTED"), "{printed}");
        }
        // The scope is not a secret and is shown, so a log line says what the
        // token was for.
        assert!(debug.contains("repo ori/studio, contents write"));
    }

    #[test]
    fn ori_t_0058_a_container_that_derives_debug_over_a_token_redacts_it_too() {
        #[derive(Debug)]
        struct Holder {
            _token: ScopedToken,
            _more: Vec<ScopedToken>,
            _maybe: Option<ScopedToken>,
        }
        let holder = Holder {
            _token: token(),
            _more: vec![token(), token()],
            _maybe: Some(token()),
        };
        assert!(!format!("{holder:?}").contains(CANARY));
        assert!(!format!("{holder:#?}").contains(CANARY));
    }

    #[test]
    fn ori_t_0058_the_one_way_to_read_a_token_is_expose() {
        let token = token();
        assert_eq!(token.expose(|value| value.to_owned()), CANARY);
        assert_eq!(token.expose(str::len), CANARY.len());
    }

    #[test]
    fn ori_t_0058_a_blank_token_or_scope_is_refused_without_echoing_either() {
        for (value, scope, what) in [
            ("", "scope", "token value"),
            ("  \t", "scope", "token value"),
            (CANARY, "", "token scope"),
            (CANARY, "   ", "token scope"),
        ] {
            let refusal = ScopedToken::new(value, scope).expect_err("a blank part is refused");
            assert_eq!(refusal, AdapterError::Malformed { what });
            assert!(!format!("{refusal:?}{refusal}").contains(CANARY));
        }
    }

    #[test]
    fn ori_t_0058_a_scoped_token_is_not_clone() {
        // A missing trait cannot be called to prove it is missing. This uses
        // the standard probe: an inherent associated const that exists only
        // when `T: Clone` shadows a trait const that says `false`, so the
        // path resolves to `true` for a `Clone` type and `false` otherwise.
        trait Probe {
            const IS_CLONE: bool;
        }
        struct Wrap<T>(core::marker::PhantomData<T>);
        impl<T> Probe for Wrap<T> {
            const IS_CLONE: bool = false;
        }
        impl<T: Clone> Wrap<T> {
            const IS_CLONE: bool = true;
        }
        let observed = [Wrap::<String>::IS_CLONE, Wrap::<ScopedToken>::IS_CLONE];
        assert_eq!(
            observed,
            [true, false],
            "the probe must see String as Clone and ScopedToken as not Clone"
        );
    }

    #[test]
    fn ori_t_0058_an_adapter_error_never_carries_a_value_it_was_built_from() {
        let refusal = PullRequestRef::new("  ").expect_err("a blank reference");
        assert_eq!(
            refusal,
            AdapterError::Malformed {
                what: "pull request reference"
            }
        );
        assert_eq!(
            Route::new("\n").expect_err("blank"),
            AdapterError::Malformed {
                what: "notification route"
            }
        );
        assert_eq!(
            CiRunRef::new("").expect_err("blank"),
            AdapterError::Malformed {
                what: "CI run reference"
            }
        );
    }

    #[test]
    fn ori_t_0058_opaque_names_are_trimmed_and_print_their_text() {
        let route = Route::new("  ops-channel ").expect("a route");
        assert_eq!(route.as_str(), "ops-channel");
        assert_eq!(route.to_string(), "ops-channel");
        assert_eq!(
            PullRequestRef::new("7").expect("a ref"),
            PullRequestRef::new(" 7").expect("a ref")
        );
    }

    // -----------------------------------------------------------------------
    // The moved identity types, seen from `ori-core`
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0058_the_moved_identity_keeps_its_creation_path_and_wire_spellings() {
        let identity = identity();
        assert_eq!(identity.role(), Role::Coder);
        assert_eq!(identity.model(), "claude-test-model");
        assert_eq!(identity.runtime(), IdentityRuntime::Headless);
        assert_eq!(identity.state(), IdentityState::Active);
        assert_eq!(identity.family().as_str(), "claude-3");
        assert!(identity.permissions().contains("\"code repositories\":["));

        assert_eq!(IdentityRuntime::Acp.to_string(), "acp");
        assert_eq!(
            "headless".parse::<IdentityRuntime>(),
            Ok(IdentityRuntime::Headless)
        );
        assert_eq!(
            "suspended".parse::<IdentityState>(),
            Ok(IdentityState::Suspended)
        );
        assert!(matches!(
            "gone".parse::<IdentityState>(),
            Err(IdentityError::Malformed {
                what: "IdentityState",
                ..
            })
        ));
        let blank_model = AgentIdentity::create(
            Id::parse(SAMPLE_ULID).expect("a ULID"),
            Id::parse(OTHER_ULID).expect("a ULID"),
            Role::Qa,
            "  ",
            IdentityRuntime::Acp,
            MemoryScopes::default(),
            ModelFamily::parse("f").expect("a family"),
        );
        assert!(matches!(
            blank_model,
            Err(IdentityError::Malformed { what: "model", .. })
        ));
    }

    // -----------------------------------------------------------------------
    // No IO, no workspace crate
    // -----------------------------------------------------------------------

    /// The lines of `source` before its test module, with comments dropped.
    fn code_lines(source: &str) -> Vec<&str> {
        source
            .split("\n#[cfg(test)]\nmod ")
            .next()
            .unwrap_or_default()
            .lines()
            .map(str::trim_start)
            .filter(|line| !line.starts_with("//"))
            .collect()
    }

    #[test]
    fn ori_t_0058_the_declarations_import_no_workspace_crate_and_do_no_io() {
        // Needles are assembled so that this test's own text is not the
        // thing it finds; the scan is also limited to lines before the first
        // test module.
        let forbidden: Vec<String> = [
            "std::fs",
            "std::net",
            "std::process",
            "std::env",
            "std::time::SystemTime",
            "std::time::Instant",
            "std::io",
            "ori_store",
            "ori_broker",
            "ori_runtime",
            "ori_integrations",
            "ori_memory",
            "ori_orchestrator",
        ]
        .iter()
        .map(|needle| (*needle).to_owned())
        .collect();
        for (name, source) in [
            ("adapters.rs", include_str!("adapters.rs")),
            ("adapters/identity.rs", include_str!("adapters/identity.rs")),
        ] {
            let lines = code_lines(source);
            assert!(
                lines.len() > 100,
                "{name}: the scan saw {} code lines; it is not reading the file",
                lines.len()
            );
            for line in lines {
                for needle in &forbidden {
                    assert!(
                        !line.contains(needle.as_str()),
                        "{name} touches {needle}: {line}"
                    );
                }
            }
        }
    }
}
