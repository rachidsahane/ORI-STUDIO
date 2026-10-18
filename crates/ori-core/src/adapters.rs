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
//! | [`ModelProvider`] | here | not chosen: `spec/LLD.md` section 2 names no implementer, see its doc |
//! | [`AgentRuntime`] | here | `ori-runtime` (`AcpClient`, `HeadlessAdapter`) |
//!
//! # `AgentRuntime`, and the types in its signatures
//!
//! `spec/API_SPEC.md` section 4 and `spec/LLD.md` section 2 say all seven
//! traits are declared in `ori-core`, and since ORI-T-0141 `AgentRuntime` is.
//! By the operator's decision (ruling R42, 2026-10-11) its move was staged
//! after the six slot traits (ORI-T-0058) because the types in its signatures
//! come with it, and they now live here too, each re-exported at the path it
//! had in `ori-runtime` (`acp` for all but two, `transcript` for [`Entry`] and
//! [`TranscriptError`]) so that no caller changed:
//!
//! | Declared here | Role in the trait |
//! |---|---|
//! | [`SessionSpec`] | the argument of `spawn`; holds a [`LaunchConfig`] |
//! | [`LaunchConfig`], [`LaunchConfigError`], [`StdioMode`] | the inspectable launch configuration of criterion ORI-P1-039 |
//! | [`SessionHandle`] | what `spawn` returns and every later call takes |
//! | [`StopReason`] | what `send` returns |
//! | [`Entry`] | what `recv` returns a slice of: one transcript entry |
//! | [`RuntimeCaps`], [`RuntimeCapsError`] | what `capabilities` returns |
//! | [`RuntimeError`] | the error of every method; holds a [`TranscriptError`] |
//!
//! What stays in `ori-runtime` is everything that performs IO or is not in the
//! trait's signatures: `Transcript` (the ordered record of entries, which
//! writes and reads a file), `AcpClient`, `HeadlessAdapter`, and
//! `LaunchDefect` with its recorder and clock types. Each moved type's
//! doc says which of its neighbours it now names only as text.
//!
//! # What this module does not do
//!
//! A declaration states a contract and performs no IO: no trait method here
//! has a default body that touches the outside world, and no function in
//! this module reads a file, a clock, an environment variable or the network.
//! The implementation of a method does, and lives in another crate. The types
//! moved from `ori-runtime` hold [`PathBuf`]s (in [`SessionSpec`]'s `cwd` and
//! in [`TranscriptError`]) and a [`Duration`] (in [`RuntimeError`]) as values;
//! holding either reads nothing.
//! This module imports no workspace crate (`spec/LLD.md` section 2); the test
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
//!   see its doc. No other type in this module is meant to hold a credential,
//!   and each says so where it could be mistaken for one ([`Route`]). The one
//!   exception is [`SessionSpec::envs`], the environment the caller prepares
//!   for a spawned child, where an injector may place an issued credential;
//!   its `Debug` prints each variable's name and never its value
//!   (ORI-T-0160), while `Clone` still copies the values; its doc says so
//!   too.
//! - Everything an adapter returns (an issue title, an event message, a
//!   completion) is data from an integration, never an instruction, whatever
//!   it says (`CLAUDE.md` absolute rule 7; AICD §27). Text that reaches an
//!   agent's context goes through the sanitization barrier first; these types
//!   carry the text and do not clean it.
//! - An [`AdapterError`] never carries a credential. An implementer that
//!   builds one from a transport error is responsible for what it copies in.

use core::fmt;
use std::path::PathBuf;
use std::time::Duration;

pub mod identity;

pub use identity::AgentIdentity;
pub use identity::IdentityError;
pub use identity::IdentityRuntime;
pub use identity::IdentityState;
pub use identity::MemoryScopes;

use crate::error::MethodologyRef;
use crate::types::Id;
use crate::types::ModelFamily;
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
/// `ori-broker` issues credentials").
///
/// # The scope is a different type from the value
///
/// The scope is a [`TokenScope`]: a description of what the token may do,
/// written for a human ("repo ori/studio, contents write"). It is not a secret
/// and `Debug` prints it. Had the constructor taken the value and the scope as
/// two strings, swapping them would be an argument-order slip that compiles,
/// and it would put the credential in every `{:?}`. As two types, the slip
/// does not compile, in either direction:
///
/// ```
/// use ori_core::adapters::{ScopedToken, TokenScope};
///
/// let scope = TokenScope::new("repo ori/studio, contents write").expect("a scope");
/// let token = ScopedToken::new("not-a-real-credential", scope).expect("a token");
/// assert_eq!(token.scope().as_str(), "repo ori/studio, contents write");
/// ```
///
/// The scope where the credential goes:
///
/// ```compile_fail
/// use ori_core::adapters::{ScopedToken, TokenScope};
///
/// let scope = TokenScope::new("repo ori/studio, contents write").expect("a scope");
/// let _ = ScopedToken::new(scope, "not-a-real-credential");
/// ```
///
/// A plain string where the scope goes:
///
/// ```compile_fail
/// use ori_core::adapters::ScopedToken;
///
/// let _ = ScopedToken::new("not-a-real-credential", "repo ori/studio, contents write");
/// ```
///
/// (The first example compiles, so the other two are not failing for an
/// unrelated reason. `tests::ori_t_0058_a_scope_and_a_token_value_cannot_be_swapped`
/// checks the same at the type level.)
///
/// What this does not stop: a caller who writes `TokenScope::new(secret)` on
/// purpose. It makes that a visible act at the call site, not a slip.
pub struct ScopedToken {
    value: Box<str>,
    scope: TokenScope,
}

impl ScopedToken {
    /// Holds `value` as a credential limited to `scope`.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Malformed`] when `value` is empty or only whitespace.
    /// The error names `"token value"` and never the text.
    pub fn new(value: impl Into<Box<str>>, scope: TokenScope) -> Result<Self, AdapterError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AdapterError::Malformed {
                what: "token value",
            });
        }
        Ok(Self { value, scope })
    }

    /// What this token may do, in words. Not a secret.
    #[must_use]
    pub const fn scope(&self) -> &TokenScope {
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
    /// What a [`ScopedToken`] may do, in words written for a human: `spec/API_SPEC.md`
    /// section 4. **Not a credential and must never hold one**: it is printed by
    /// `Debug` and `Display`, and it is a type of its own so that it cannot be
    /// passed where a [`ScopedToken`]'s value goes (see that type's doc).
    TokenScope,
    "token scope"
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
    /// **An [`AgentIdentity`] value is not proof that the identity was
    /// registered.** `AgentIdentity::create` is a pure constructor, and since
    /// it is declared in `ori-core` it is reachable from every crate that
    /// depends on `ori-core`. The step that checks a new identity against the
    /// others the event log knows (ADR-0001's cross-model refusal, in
    /// `ori-broker`'s `registration` module) is one a caller can skip, and
    /// nothing enforces mechanically that it did not; `registration.rs` names
    /// that seam in its own module doc. So an implementation must not issue a
    /// token on this value's word alone. It gets the credential from the
    /// broker (`ori-broker` issues credentials) and does not mint one from
    /// the identity's fields, and it refuses ([`AdapterError::Rejected`]) an
    /// identity whose [`IdentityState`] is not `Active`. Today the broker's
    /// issuance path does not itself check that the identity was registered
    /// either (`issuance.rs` does not mention registration), so this
    /// declaration cannot promise that a registered identity is the only kind
    /// that reaches here; closing that seam is the job of whoever wires the
    /// end-to-end identity path, and it is not closed by this declaration.
    ///
    /// # Errors
    ///
    /// [`AdapterError`] when the host cannot issue one, or refuses the
    /// identity.
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

// ---------------------------------------------------------------------------
// StdioMode, LaunchConfig: the inspectable half of ORI-P1-039
// ---------------------------------------------------------------------------

/// What standard input a launched runtime is given: PRD P-02, ORI-P1-039.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StdioMode {
    /// The platform's null device (`/dev/null` on unix, `NUL` on Windows): a
    /// read against it returns end-of-file immediately, on every platform.
    Null,
    /// A pipe this process reads and writes: what the ACP client needs, since
    /// it *is* the protocol channel.
    Piped,
    /// The parent's own standard input. Never chosen by [`LaunchConfig::acp`]
    /// or [`LaunchConfig::headless`]; only [`LaunchConfig::new`] takes it, so
    /// that a caller can build the one shape both adapters in `ori-runtime`
    /// refuse (`AcpClient::spawn` and `HeadlessAdapter::spawn` each answer a
    /// [`SessionSpec`] built with it with [`RuntimeError::WrongStdinMode`]).
    Inherit,
}

impl fmt::Display for StdioMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Null => "null",
            Self::Piped => "piped",
            Self::Inherit => "inherit",
        })
    }
}

/// The runtime launch configuration ORI-P1-039 says must be inspectable:
/// "Inspect the runtime launch configuration and the session transcript".
///
/// Deliberately excludes the process environment. `spec/API_SPEC.md` §4:
/// "adapters receive credentials from the broker per call or per session,
/// never from configuration"; this type is what a test, and later an
/// operator, reads to answer "was this launched non-interactively", and an
/// environment variable is never part of that answer, so there is no field
/// here that could hold one. See `ori-runtime`'s `acp` module doc comment,
/// "The seam with `injector.rs`".
///
/// # `non_interactive` cannot be `false`
///
/// ORI-P1-039 is a fail-closed launch: a runtime is launched in its
/// non-interactive mode or not at all. Every constructor here ([`LaunchConfig::acp`],
/// [`LaunchConfig::headless`] and the general [`LaunchConfig::new`] they share)
/// sets `non_interactive` to `true` and takes no parameter that could say
/// otherwise; the field is private to this module, and there is no setter
/// and no other way to build a value, in this module or outside it. So no
/// `LaunchConfig` can
/// claim an interactive launch, whatever [`StdioMode`] it carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchConfig {
    program: String,
    args: Vec<String>,
    stdin: StdioMode,
    non_interactive: bool,
}

impl LaunchConfig {
    /// The launch configuration `AcpClient::spawn` (in `ori-runtime`) uses:
    /// standard input piped (the client writes JSON-RPC requests to it),
    /// which is what makes the whole exchange the protocol substitute for a
    /// terminal prompt in the first place, so `non_interactive` is `true`
    /// unconditionally: an agent given a pipe instead of a pseudo-terminal
    /// has no terminal to prompt on at all, whatever it does with the pipe.
    ///
    /// # Errors
    ///
    /// [`LaunchConfigError::EmptyProgram`] when `program` is empty or only
    /// whitespace.
    pub fn acp(program: impl Into<String>, args: Vec<String>) -> Result<Self, LaunchConfigError> {
        Self::new(program, args, StdioMode::Piped)
    }

    /// The launch configuration `HeadlessAdapter::spawn` (in `ori-runtime`)
    /// requires: standard input closed ([`StdioMode::Null`]), so a read of it
    /// returns end-of-file at once and a runtime that wants to prompt has
    /// nothing to prompt through. `non_interactive` is `true`. See
    /// `ori-runtime`'s `headless` module doc comment, "What 'prompting' can
    /// mean with stdin closed".
    ///
    /// Declared here, beside [`LaunchConfig::acp`], and not in `ori-runtime`'s
    /// `headless` module where it began: an inherent `impl` for a type
    /// declared in another crate is not allowed (E0116), and keeping it an
    /// associated function keeps every call, `LaunchConfig::headless(..)`,
    /// spelled as it was.
    ///
    /// # Errors
    ///
    /// [`LaunchConfigError::EmptyProgram`] when `program` is empty or only
    /// whitespace.
    pub fn headless(
        program: impl Into<String>,
        args: Vec<String>,
    ) -> Result<Self, LaunchConfigError> {
        Self::new(program, args, StdioMode::Null)
    }

    /// The general constructor [`LaunchConfig::acp`] and
    /// [`LaunchConfig::headless`] share, for any [`StdioMode`].
    ///
    /// Public so that a caller outside this crate can build a launch
    /// configuration for a mode the two named constructors do not choose
    /// ([`StdioMode::Inherit`], which both adapters in `ori-runtime` refuse,
    /// is what their tests need). Exposing it does not open a way to an
    /// interactive launch: it sets `non_interactive` to `true` for every
    /// `stdin` it is given and has no parameter that could change that
    /// (ORI-P1-039, the fail-closed launch). That is the only reason it is
    /// public. A constructor that took `non_interactive` as an argument
    /// would not be.
    ///
    /// # Errors
    ///
    /// [`LaunchConfigError::EmptyProgram`] when `program` is empty or only
    /// whitespace.
    pub fn new(
        program: impl Into<String>,
        args: Vec<String>,
        stdin: StdioMode,
    ) -> Result<Self, LaunchConfigError> {
        let program = program.into();
        if program.trim().is_empty() {
            return Err(LaunchConfigError::EmptyProgram);
        }
        Ok(Self {
            program,
            args,
            stdin,
            non_interactive: true,
        })
    }

    /// The program to execute. Never a path this module resolved itself:
    /// the caller names it, the same way `ori-runtime`'s `worktree::SystemGit`
    /// never resolves `git`'s own path.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The argv, without the program name. Never holds a credential: see
    /// `ori-runtime`'s `acp` module doc comment, "The seam with
    /// `injector.rs`".
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// What standard input the child is given.
    #[must_use]
    pub const fn stdin(&self) -> StdioMode {
        self.stdin
    }

    /// Whether this launch is the runtime's non-interactive mode: PRD P-02.
    #[must_use]
    pub const fn non_interactive(&self) -> bool {
        self.non_interactive
    }
}

impl fmt::Display for LaunchConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} (stdin={}, non_interactive={})",
            self.program,
            self.args.join(" "),
            self.stdin,
            self.non_interactive
        )
    }
}

/// Why building a [`LaunchConfig`] was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LaunchConfigError {
    /// `program` was empty or only whitespace.
    EmptyProgram,
}

impl fmt::Display for LaunchConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProgram => f.write_str("a launch program is not empty"),
        }
    }
}

impl std::error::Error for LaunchConfigError {}

// ---------------------------------------------------------------------------
// SessionSpec, SessionHandle
// ---------------------------------------------------------------------------

/// What the engine gives an adapter to start a session: `spec/API_SPEC.md`
/// §4's `AgentRuntime::spawn(session: SessionSpec)`. See `ori-runtime`'s
/// `acp` module doc comment, "`SessionSpec` and `SessionHandle`, and what was
/// chosen for them".
#[derive(Clone)]
pub struct SessionSpec {
    /// The engine's own session identifier (`ori-runtime`'s
    /// `session::Session::id`).
    pub id: Id,
    /// The launch configuration.
    pub launch: LaunchConfig,
    /// The working directory the child is spawned in: the session's
    /// `ori-runtime` `worktree::Worktree` path, in the engine's real use.
    pub cwd: PathBuf,
    /// The child's environment, prepared entirely by the caller. An adapter
    /// reads it for nothing but passing it to [`std::process::Command::envs`]
    /// at spawn; see `ori-runtime`'s `acp` module doc comment, "The seam with
    /// `injector.rs`". Declaring the field performs no IO; the spawn does,
    /// and lives in `ori-runtime`.
    ///
    /// **May carry a credential; `Debug` prints names only.** An issued
    /// credential an injector places in the child's environment is placed
    /// here. [`SessionSpec`]'s `Debug` prints each variable's name and
    /// `"REDACTED"` in place of its value, as [`ScopedToken`]'s does
    /// (ORI-T-0160). `Clone` still copies the values, so a clone is as
    /// sensitive as the original, and nothing here stops a caller from
    /// printing `envs` itself.
    pub envs: Vec<(String, String)>,
}

/// Every field but the environment's values, which may hold an issued
/// credential (see [`SessionSpec::envs`]): each variable prints as its name
/// and `"REDACTED"`, the way [`ScopedToken`]'s `Debug` prints its value.
impl fmt::Debug for SessionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let envs: Vec<(&str, &str)> = self
            .envs
            .iter()
            .map(|(name, _)| (name.as_str(), "REDACTED"))
            .collect();
        f.debug_struct("SessionSpec")
            .field("id", &self.id)
            .field("launch", &self.launch)
            .field("cwd", &self.cwd)
            .field("envs", &envs)
            .finish()
    }
}

/// An opaque handle to a spawned session: `spec/API_SPEC.md` §4's
/// `SessionHandle`. Carries the same [`Id`] the [`SessionSpec`] it was
/// spawned from carried; see `ori-runtime`'s `acp` module doc comment for
/// why that is also the check every later call makes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionHandle {
    id: Id,
}

impl SessionHandle {
    /// Wraps a session identifier a [`SessionSpec`] was built with. Public so
    /// that the adapters in `ori-runtime` (`AcpClient`, `HeadlessAdapter`),
    /// which are in another crate and so cannot reach this type's private
    /// field, can build one.
    #[must_use]
    pub const fn new(id: Id) -> Self {
        Self { id }
    }

    /// The session identifier this handle names.
    #[must_use]
    pub const fn id(&self) -> &Id {
        &self.id
    }
}

// ---------------------------------------------------------------------------
// RuntimeCaps
// ---------------------------------------------------------------------------

/// What a runtime adapter declares about itself: `spec/API_SPEC.md` §4's
/// `AgentRuntime::capabilities`, ADR-0001 "Model family".
///
/// # How the family is guaranteed
///
/// The operator's ruling on escalation 6 (carried by `crates/ori-broker/src/family.rs`'s
/// own doc comment and by ADR-0001's decision row): "the runtime adapter
/// declares the family through `RuntimeCaps`... and the engine passes it to
/// `broker.identity.create`". [`RuntimeCaps::new`] is the only way to build
/// one, and its first parameter is a [`ModelFamily`], not
/// an `Option<ModelFamily>` and not a `String` a caller could leave blank:
/// [`ModelFamily::parse`] already refuses an empty or whitespace-only value
/// (`crates/ori-core/src/types.rs`), so a value of that type reaching this
/// constructor already passed that check once. There is no [`Default`] impl,
/// no public field, and no second constructor, so a `RuntimeCaps` cannot be
/// built by any path that does not supply a family: an adapter with no
/// declared family is not constructible, the same fail-closed shape
/// `crates/ori-broker/src/family.rs`'s own `ModelFamily::declare` uses for
/// itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCaps {
    family: ModelFamily,
    adapter_name: String,
}

impl RuntimeCaps {
    /// Declares an adapter's capabilities, refusing an empty or
    /// whitespace-only `adapter_name`. `family` cannot be empty by
    /// construction: see this type's own doc comment.
    ///
    /// # Errors
    ///
    /// [`RuntimeCapsError::EmptyAdapterName`] when `adapter_name` is empty or
    /// only whitespace.
    pub fn new(
        family: ModelFamily,
        adapter_name: impl Into<String>,
    ) -> Result<Self, RuntimeCapsError> {
        let adapter_name = adapter_name.into();
        if adapter_name.trim().is_empty() {
            return Err(RuntimeCapsError::EmptyAdapterName);
        }
        Ok(Self {
            family,
            adapter_name,
        })
    }

    /// The declared model family. Always present: see this type's own doc
    /// comment, "How the family is guaranteed".
    #[must_use]
    pub const fn family(&self) -> &ModelFamily {
        &self.family
    }

    /// The adapter's own name, for a launch record or a log line to say which
    /// adapter ran, never a secret.
    #[must_use]
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }
}

/// Why building a [`RuntimeCaps`] was refused: ADR-0001 "Model family".
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeCapsError {
    /// `adapter_name` was empty or only whitespace.
    EmptyAdapterName,
}

impl fmt::Display for RuntimeCapsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAdapterName => f.write_str("an adapter name is not empty"),
        }
    }
}

impl std::error::Error for RuntimeCapsError {}

// ---------------------------------------------------------------------------
// StopReason
// ---------------------------------------------------------------------------

/// Where a turn ended: ACP's `stopReason`, or `ori-runtime`'s headless
/// adapter's own mapping of a headless run's exit. What
/// [`AgentRuntime::send`] returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StopReason {
    /// The agent finished the turn normally.
    EndTurn,
    /// The agent declined to continue (for instance, after `ori-runtime`'s
    /// ACP client refused a permission request).
    Refusal,
    /// The turn was cancelled.
    Cancelled,
    /// A value this type does not otherwise recognize, carried as data
    /// rather than dropped: `spec/API_SPEC.md` §5, "Adapter traits are
    /// versioned by crate", which is exactly what lets an agent report a stop
    /// reason a fixed enum here does not yet name without an adapter
    /// refusing to compile against it.
    Other(String),
}

impl StopReason {
    /// Reads a `stopReason` string into this type.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        match text {
            "end_turn" => Self::EndTurn,
            "refusal" => Self::Refusal,
            "cancelled" => Self::Cancelled,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndTurn => f.write_str("end_turn"),
            Self::Refusal => f.write_str("refusal"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Other(text) => write!(f, "{text}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Entry, TranscriptError
// ---------------------------------------------------------------------------

/// One entry of a session transcript: AICD §17.
///
/// The per-action part of AICD §17's audit-trail sentence: "which tool, what
/// input, what output, when", plus the attempt it belongs to. `tool` is
/// absent for an entry that is the agent's own reasoning or a note rather than
/// a tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    attempt: u32,
    at: Timestamp,
    tool: Option<String>,
    input: String,
    output: String,
}

impl Entry {
    /// One entry, refusing an attempt numbered zero, a blank tool name given
    /// as `Some`, and an entry with neither input nor output.
    ///
    /// An attempt numbered from one matches
    /// `ori_orchestrator::budgets::Approach::new`'s same refusal, for the same
    /// reason: criterion ORI-P1-009 turns on the attempts being distinguishable
    /// and counted, and a zeroth attempt is not a thing that happened. An entry
    /// with nothing in either field is a line that would occupy a slot in the
    /// record and prove nothing, which is exactly the record this criterion
    /// asks not to be empty.
    pub fn new(
        attempt: u32,
        at: Timestamp,
        tool: Option<&str>,
        input: &str,
        output: &str,
    ) -> Result<Self, TranscriptError> {
        if attempt == 0 {
            return Err(TranscriptError::AttemptNotNumbered);
        }
        let tool = match tool {
            Some(name) if name.trim().is_empty() => {
                return Err(TranscriptError::Blank { field: "tool" });
            }
            Some(name) => Some(name.trim().to_owned()),
            None => None,
        };
        if input.trim().is_empty() && output.trim().is_empty() {
            return Err(TranscriptError::Blank {
                field: "input or output",
            });
        }
        Ok(Self {
            attempt,
            at,
            tool,
            input: input.to_owned(),
            output: output.to_owned(),
        })
    }

    /// Which attempt this entry belongs to, numbered from one.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    /// When this entry happened.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }

    /// The tool this entry is about, absent for the agent's own reasoning.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_deref()
    }

    /// What went in.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// What came out.
    #[must_use]
    pub fn output(&self) -> &str {
        &self.output
    }
}

/// Everything a session transcript refuses or cannot read: AICD §17.
///
/// Declared here because [`Entry::new`] returns it and [`RuntimeError`] holds
/// one. [`TranscriptError::OutOfOrder`], [`TranscriptError::AttemptWentBackward`],
/// [`TranscriptError::Io`] and [`TranscriptError::Corrupt`] are raised by
/// `ori-runtime`'s `Transcript`, which stays there because it writes and reads
/// a file; the variants are plain data and building one performs no IO.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TranscriptError {
    /// An entry was recorded against attempt zero.
    AttemptNotNumbered,
    /// A required field was blank.
    Blank {
        /// The field, named as [`Entry::new`]'s doc comment names it.
        field: &'static str,
    },
    /// An entry was timestamped before the last one recorded.
    OutOfOrder {
        /// The time the new entry claims.
        at: Timestamp,
        /// The time the last recorded entry claims.
        last: Timestamp,
    },
    /// An entry's attempt number is lower than the last one recorded.
    AttemptWentBackward {
        /// The attempt the new entry claims.
        attempt: u32,
        /// The attempt the last recorded entry claims.
        last: u32,
    },
    /// The filesystem refused the read or the write.
    Io {
        /// The path involved.
        path: PathBuf,
        /// What the operating system reported.
        message: String,
    },
    /// A file `ori-runtime`'s `Transcript::read_from` read does not parse as
    /// one its `Transcript::write_to` would have written.
    Corrupt {
        /// The file.
        path: PathBuf,
        /// The line, numbered from one.
        line: usize,
        /// What about it did not parse.
        reason: &'static str,
    },
}

impl TranscriptError {
    /// Whether a rule of the record refused the entry, as opposed to the
    /// filesystem or a parse failure: AICD §17, the section that makes the
    /// record a control in the first place. The two variants that report a
    /// filesystem or a parse failure rather than a rule ([`TranscriptError::Io`]
    /// and [`TranscriptError::Corrupt`]) are not refusals, the same split
    /// `ori-runtime`'s `session::SessionError` draws between a refusal and a
    /// value failing to parse.
    ///
    /// `spec/CONVENTIONS.md` names `thiserror` as the house convention; ruling
    /// R20 in `ops/rulings.md` defers it and states what stands until then, "a
    /// hand-written `Display` and `std::error::Error` implementation [...],
    /// with a comment naming the conversion", which is what this is.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        !matches!(self, Self::Io { .. } | Self::Corrupt { .. })
    }
}

impl fmt::Display for TranscriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttemptNotNumbered => f.write_str(
                "an entry is recorded against attempt 0, and attempts are numbered from 1",
            ),
            Self::Blank { field } => write!(
                f,
                "a transcript entry needs \"{field}\" and this one leaves it blank"
            ),
            Self::OutOfOrder { at, last } => {
                write!(
                    f,
                    "an entry at {at} is before the last one recorded, at {last}"
                )
            }
            Self::AttemptWentBackward { attempt, last } => write!(
                f,
                "an entry for attempt {attempt} follows one for attempt {last}, which is backward"
            ),
            Self::Io { path, message } => {
                write!(
                    f,
                    "{} could not be read or written: {message}",
                    path.display()
                )
            }
            Self::Corrupt { path, line, reason } => {
                write!(
                    f,
                    "{}:{line} does not parse as a transcript entry: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for TranscriptError {}

// ---------------------------------------------------------------------------
// RuntimeError
// ---------------------------------------------------------------------------

/// Everything a runtime adapter refuses or cannot do: AICD §17 for the
/// refusals, `spec/CONVENTIONS.md`'s ruling R20 (hand-written `Display` and
/// `std::error::Error`, deferring `thiserror`) for the shape. The error of
/// every [`AgentRuntime`] method.
///
/// Only [`RuntimeError::WrongStdinMode`] is a refusal by a control and
/// carries a `MethodologyRef` (section 17, through
/// [`RuntimeError::methodology_ref`]); every other variant describes a
/// failure of the channel or of the caller's sequence of calls, and carries
/// none.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeError {
    /// The child process could not be spawned, or an I/O operation on its
    /// stdio failed.
    Io {
        /// What the operating system reported.
        message: String,
    },
    /// A line read from the child was not valid JSON, or not a JSON-RPC 2.0
    /// frame the adapter recognizes, or was longer than the line cap (1 MiB),
    /// in which case the session is spent as after a [`RuntimeError::Timeout`].
    Protocol {
        /// What was wrong.
        message: String,
    },
    /// The agent answered one of the client's own requests with a JSON-RPC
    /// error.
    Rpc {
        /// The error code the agent reported.
        code: i64,
        /// The error message the agent reported, carried as data.
        message: String,
    },
    /// The child closed its output before answering a request the client
    /// was waiting on.
    UnexpectedEof,
    /// A JSON-RPC round trip (the request's write, every read, and every reply
    /// the client writes, together) or a process teardown did not complete
    /// within the call's deadline, `IO_TIMEOUT` (in `ori-runtime`'s `acp`
    /// module) outside tests. For `ori-runtime`'s `AcpClient` the child has
    /// been killed and reaped and the session is spent: every later `prompt`
    /// is refused as [`RuntimeError::Io`], while the transcript stays
    /// readable and `kill` still works.
    ///
    /// # Where the methodology reference is, and is not
    ///
    /// [`RuntimeError::methodology_ref`] is `None` for this variant and
    /// [`RuntimeError::is_refusal`] is `false`: a timeout is a failure of the
    /// channel (AICD §12, an agent that gets stuck is stopped, not waited on),
    /// not a refusal by a control, and those two methods describe refusals
    /// only. The AICD §17 reference is carried by `LaunchDefect::reason` (in
    /// `ori-runtime`), on the launch defect a caller's recorder is handed,
    /// and the two adapters differ in whether a timeout produces one.
    /// `AcpClient` records no launch defect for a timeout: its only defect is
    /// a `session/request_permission`, which a silent agent never sends.
    /// `ori-runtime`'s `HeadlessAdapter` records one (method `headless.run`,
    /// reason AICD §17) when its run does not finish in time, because there
    /// silence is indistinguishable from a runtime blocked on a prompt it was
    /// given no way to answer. Neither path puts the reference on this error
    /// value.
    Timeout {
        /// How long this call waited before giving up.
        after: Duration,
    },
    /// The caller-supplied `record_defect` closure refused or failed.
    DefectSink {
        /// What it reported.
        message: String,
    },
    /// Appending to this adapter's own transcript was refused.
    Transcript(TranscriptError),
    /// A [`SessionHandle`] that does not name the session this adapter
    /// currently holds.
    UnknownSession {
        /// The identifier the handle carried.
        id: Id,
    },
    /// [`AgentRuntime::spawn`] was called while a session is already active.
    AlreadySpawned,
    /// A call other than `spawn` was made before any session was spawned.
    NotSpawned,
    /// A [`SessionSpec`] whose [`LaunchConfig::stdin`] is not the mode this
    /// adapter requires: AICD §17. ORI-P1-039's own launch-configuration
    /// clause is a refusal here, not a silent correction, so that a caller
    /// that built the wrong `LaunchConfig` learns that at `spawn`, not from a
    /// runtime that turns out to prompt.
    WrongStdinMode {
        /// What this adapter requires.
        required: StdioMode,
        /// What the caller supplied.
        found: StdioMode,
    },
    /// `HeadlessAdapter::send` (in `ori-runtime`) was called a second time. A
    /// one-shot, non-interactive run has exactly one turn: see
    /// `ori-runtime`'s `headless` module doc comment.
    AlreadyCompleted,
}

impl RuntimeError {
    /// The methodology section a refusal is made under, for the refusals and
    /// for nothing else. In particular [`RuntimeError::Timeout`] is `None`:
    /// see that variant's documentation for where AICD §12 and AICD §17 sit
    /// for a timeout.
    #[must_use]
    pub const fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::WrongStdinMode { .. } => Some(MethodologyRef {
                section: 17,
                subsection: None,
            }),
            Self::Io { .. }
            | Self::Protocol { .. }
            | Self::Rpc { .. }
            | Self::UnexpectedEof
            | Self::Timeout { .. }
            | Self::DefectSink { .. }
            | Self::Transcript(_)
            | Self::UnknownSession { .. }
            | Self::AlreadySpawned
            | Self::NotSpawned
            | Self::AlreadyCompleted => None,
        }
    }

    /// Whether a control refused the action, as opposed to an I/O or protocol
    /// failure.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::WrongStdinMode { .. })
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { message } => write!(f, "an I/O operation on the runtime failed: {message}"),
            Self::Protocol { message } => {
                write!(f, "not a JSON-RPC 2.0 frame this client reads: {message}")
            }
            Self::Rpc { code, message } => write!(f, "the agent returned error {code}: {message}"),
            Self::UnexpectedEof => f.write_str("the agent closed its output before answering"),
            Self::Timeout { after } => write!(f, "no answer within {after:?}"),
            Self::DefectSink { message } => {
                write!(f, "recording the launch defect failed: {message}")
            }
            Self::Transcript(inner) => write!(f, "the transcript refused this entry: {inner}"),
            Self::UnknownSession { id } => write!(f, "no active session named {id}"),
            Self::AlreadySpawned => f.write_str("this adapter already holds an active session"),
            Self::NotSpawned => f.write_str("no session has been spawned yet"),
            Self::WrongStdinMode { required, found } => write!(
                f,
                "refused: this adapter requires stdin={required}, and the launch configuration \
                 gave it stdin={found} ({})",
                MethodologyRef {
                    section: 17,
                    subsection: None,
                }
            ),
            Self::AlreadyCompleted => {
                f.write_str("this one-shot session already ran its single turn")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<TranscriptError> for RuntimeError {
    fn from(inner: TranscriptError) -> Self {
        Self::Transcript(inner)
    }
}

// ---------------------------------------------------------------------------
// The AgentRuntime trait
// ---------------------------------------------------------------------------

/// One runtime adapter: `spec/API_SPEC.md` §4, ADR-0001 "Agent protocol".
///
/// Declared here, as `spec/API_SPEC.md` §4 and `spec/LLD.md` section 2 say all
/// seven adapter traits are, and implemented in `ori-runtime` by `AcpClient`
/// and `HeadlessAdapter`, the only crate that spawns processes. It carries no
/// `Send` or `Sync` bound, as the six slot traits carry none; a holder adds
/// the markers it needs to the trait object it names.
///
/// Matches `spec/API_SPEC.md` §4's sketch closely: the launch-defect recorder
/// and the clock are not parameters of `spawn` or `send`. They used to be
/// (this trait's own divergence from the sketch, in an earlier version),
/// which let the trait path silently drop ORI-P1-039's "recorded as a launch
/// defect" clause; see `ori-runtime`'s `acp` module doc comment, "Why the
/// recorder and the clock are constructor parameters, not per-call ones".
/// Both are supplied once, to each implementation's own constructor
/// (`AcpClient::new`, `HeadlessAdapter::new`), instead. `LaunchDefect`, the
/// recorder and the clock are `ori-runtime`'s, not part of this trait's
/// signatures, and did not move.
pub trait AgentRuntime {
    /// Starts the runtime in its non-interactive mode, inside the isolation
    /// boundary the caller already built (`ori-runtime`'s `worktree` and
    /// `container` modules): PRD P-02, PRD I-04.
    ///
    /// # Errors
    ///
    /// See each implementation's own `Error` type.
    fn spawn(&mut self, spec: SessionSpec) -> Result<SessionHandle, RuntimeError>;

    /// Sends one prompt to the session, running until the turn ends,
    /// answering any permission prompt the agent raises with a refusal
    /// (never a grant) and recording it through the recorder and clock this
    /// adapter was constructed with, and this adapter's own transcript.
    ///
    /// # Errors
    ///
    /// See each implementation's own `Error` type.
    fn send(&mut self, handle: &SessionHandle, prompt: &str) -> Result<StopReason, RuntimeError>;

    /// The transcript entries recorded for this session so far.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UnknownSession`] when `handle` does not name the
    /// session this adapter holds.
    fn recv(&self, handle: &SessionHandle) -> Result<&[Entry], RuntimeError>;

    /// Ends the session and the process backing it.
    ///
    /// # Errors
    ///
    /// See each implementation's own `Error` type.
    fn kill(&mut self, handle: &SessionHandle) -> Result<(), RuntimeError>;

    /// What this adapter declares about itself. Always carries a
    /// [`ModelFamily`], through [`RuntimeCaps`]: see that type's own doc
    /// comment.
    fn capabilities(&self) -> RuntimeCaps;
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

    fn scope(text: &str) -> TokenScope {
        TokenScope::new(text).expect("a scope")
    }

    fn token() -> ScopedToken {
        ScopedToken::new(CANARY, scope("repo ori/studio, contents write")).expect("a real token")
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
            ScopedToken::new(
                CANARY,
                TokenScope::new(&format!("role {}", identity.role()))?,
            )
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
        assert_eq!(token.scope().as_str(), "role coder");

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
    fn ori_t_0058_the_slot_traits_carry_no_send_or_sync_bound_and_a_holder_adds_them_itself() {
        // The traits' docs say they carry no `Send` or `Sync` bound and that a
        // holder who shares one names `dyn Trait + Send + Sync`. This pins
        // that about the trait objects themselves, not about a fake: it
        // fails if a supertrait is added later, which is then a deliberate
        // contract change and not an accident. Probes as in the tests above:
        // an inherent const that exists only under a bound shadows a trait
        // const that says `false`, and the controls show both answers occur.
        use core::marker::PhantomData;

        trait Probe {
            const IS_SEND: bool;
            const IS_SYNC: bool;
        }
        struct Wrap<T: ?Sized>(PhantomData<T>);
        impl<T: ?Sized> Probe for Wrap<T> {
            const IS_SEND: bool = false;
            const IS_SYNC: bool = false;
        }
        impl<T: ?Sized + Send> Wrap<T> {
            const IS_SEND: bool = true;
        }
        impl<T: ?Sized + Sync> Wrap<T> {
            const IS_SYNC: bool = true;
        }

        // Controls: a type that is both, and one that is neither.
        assert_eq!(
            [Wrap::<String>::IS_SEND, Wrap::<String>::IS_SYNC],
            [true, true]
        );
        assert_eq!(
            [
                Wrap::<std::rc::Rc<u8>>::IS_SEND,
                Wrap::<std::rc::Rc<u8>>::IS_SYNC
            ],
            [false, false]
        );

        // The six slot traits, as bare trait objects: neither.
        let bare = [
            [Wrap::<dyn VcsHost>::IS_SEND, Wrap::<dyn VcsHost>::IS_SYNC],
            [
                Wrap::<dyn ErrorTracking>::IS_SEND,
                Wrap::<dyn ErrorTracking>::IS_SYNC,
            ],
            [
                Wrap::<dyn Analytics>::IS_SEND,
                Wrap::<dyn Analytics>::IS_SYNC,
            ],
            [
                Wrap::<dyn Notification>::IS_SEND,
                Wrap::<dyn Notification>::IS_SYNC,
            ],
            [Wrap::<dyn Ci>::IS_SEND, Wrap::<dyn Ci>::IS_SYNC],
            [
                Wrap::<dyn ModelProvider>::IS_SEND,
                Wrap::<dyn ModelProvider>::IS_SYNC,
            ],
        ];
        assert_eq!(
            bare,
            [[false, false]; 6],
            "a trait grew a Send or Sync bound"
        );

        // With the markers a holder adds: both. This is the form the docs name.
        let marked = [
            [
                Wrap::<dyn VcsHost + Send + Sync>::IS_SEND,
                Wrap::<dyn VcsHost + Send + Sync>::IS_SYNC,
            ],
            [
                Wrap::<dyn ErrorTracking + Send + Sync>::IS_SEND,
                Wrap::<dyn ErrorTracking + Send + Sync>::IS_SYNC,
            ],
            [
                Wrap::<dyn Analytics + Send + Sync>::IS_SEND,
                Wrap::<dyn Analytics + Send + Sync>::IS_SYNC,
            ],
            [
                Wrap::<dyn Notification + Send + Sync>::IS_SEND,
                Wrap::<dyn Notification + Send + Sync>::IS_SYNC,
            ],
            [
                Wrap::<dyn Ci + Send + Sync>::IS_SEND,
                Wrap::<dyn Ci + Send + Sync>::IS_SYNC,
            ],
            [
                Wrap::<dyn ModelProvider + Send + Sync>::IS_SEND,
                Wrap::<dyn ModelProvider + Send + Sync>::IS_SYNC,
            ],
        ];
        assert_eq!(marked, [[true, true]; 6]);
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
        for value in ["", "  \t"] {
            let refusal =
                ScopedToken::new(value, scope("a scope")).expect_err("a blank value is refused");
            assert_eq!(
                refusal,
                AdapterError::Malformed {
                    what: "token value"
                }
            );
        }
        for blank in ["", "   "] {
            let refusal = TokenScope::new(blank).expect_err("a blank scope is refused");
            assert_eq!(
                refusal,
                AdapterError::Malformed {
                    what: "token scope"
                }
            );
            assert!(!format!("{refusal:?}{refusal}").contains(CANARY));
        }
    }

    #[test]
    fn ori_t_0058_a_scope_and_a_token_value_cannot_be_swapped() {
        // Type level, with a control for each probe so that "false" means
        // "not implemented" and not "the probe is broken". An inherent
        // associated const that exists only under a bound shadows a trait
        // const that says `false`.
        trait Probe {
            const OK: bool;
        }
        struct AsValue<T>(core::marker::PhantomData<T>);
        impl<T> Probe for AsValue<T> {
            const OK: bool = false;
        }
        impl<T: Into<Box<str>>> AsValue<T> {
            const OK: bool = true;
        }
        struct AsScope<T>(core::marker::PhantomData<T>);
        impl<T> Probe for AsScope<T> {
            const OK: bool = false;
        }
        impl<T: Into<TokenScope>> AsScope<T> {
            const OK: bool = true;
        }
        let observed = [
            // Controls: what the value parameter accepts, and what the scope
            // parameter accepts.
            AsValue::<String>::OK,
            AsScope::<TokenScope>::OK,
            // The swaps: a scope as the value, and a plain string as the scope.
            AsValue::<TokenScope>::OK,
            AsScope::<&str>::OK,
            AsScope::<String>::OK,
        ];
        assert_eq!(
            observed,
            [true, true, false, false, false],
            "controls first, then the swaps: a scope is not a value, and a string is not a scope"
        );
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
    //
    // The real guarantee that `ori-core` imports no workspace crate is its
    // manifest: `[dependencies]` is empty, so the compiler refuses to resolve
    // `ori_store::x` or any other crate. The manifest test below pins that.
    //
    // What a manifest cannot say is that the standard library's IO is not
    // reached, since `std` comes with every crate. The scan below is a lexical
    // check for that, and for a workspace crate named in the source; it reads
    // this crate's own non-test lines and cannot see through a macro, a path
    // attribute or a spelling it does not know. It is a tripwire, not a proof,
    // and it is tested against samples it must catch (and one it must not) so
    // that a scan which finds nothing cannot pass for one that works.

    /// Every workspace crate, by the name a `use` gives it. The `ori_` prefix
    /// check below catches a new one; this list is what a reader can audit.
    const WORKSPACE_CRATES: [&str; 17] = [
        "ori_core",
        "ori_store",
        "ori_memory",
        "ori_broker",
        "ori_runtime",
        "ori_gates",
        "ori_orchestrator",
        "ori_flows",
        "ori_watch",
        "ori_mcp",
        "ori_integrations",
        "ori_notify",
        "ori_calibration",
        "ori_rpc",
        "ori_engine",
        "ori_cli",
        "ori_desktop",
    ];

    /// Standard-library paths that reach the outside world: files, sockets,
    /// processes, the environment, threads, clocks. A path is refused when it
    /// is, or lies under, one of these.
    const IO_PATHS: [&str; 9] = [
        "std::fs",
        "std::io",
        "std::net",
        "std::process",
        "std::env",
        "std::thread",
        "std::os",
        "std::time::SystemTime",
        "std::time::Instant",
    ];

    /// `source` before its test module, comments dropped: full-line comments,
    /// and a trailing `//` comment on a line with no string literal.
    fn code_of(source: &str) -> String {
        source
            .split("\n#[cfg(test)]\nmod ")
            .next()
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("//"))
            .map(|line| match line.split_once("//") {
                Some((code, _)) if !line.contains('"') => code,
                _ => line,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Splits `text` at commas that are not inside braces.
    fn split_top_level(text: &str) -> Vec<&str> {
        let mut parts = Vec::new();
        let mut depth = 0_u32;
        let mut start = 0;
        for (index, ch) in text.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    parts.push(&text[start..index]);
                    start = index + 1;
                }
                _ => {}
            }
        }
        parts.push(&text[start..]);
        parts
    }

    /// Adds every path `tree` names under `prefix` to `out`, braces expanded:
    /// `std::{fs, io::Read}` is `std::fs` and `std::io::Read`.
    fn expand_use(prefix: &str, tree: &str, out: &mut Vec<String>) {
        let tree = tree.trim();
        if tree.is_empty() {
            return;
        }
        if let (Some(open), Some(close)) = (tree.find('{'), tree.rfind('}')) {
            let head = format!("{prefix}{}", &tree[..open]);
            for part in split_top_level(&tree[open + 1..close]) {
                expand_use(&head, part, out);
            }
            return;
        }
        // `x as y` names `x`; `a::self` names `a`.
        let name = tree.split(" as ").next().unwrap_or(tree).trim();
        let path = format!("{prefix}{name}");
        out.push(path.strip_suffix("::self").unwrap_or(&path).to_owned());
    }

    /// Every path any `use` or `extern crate` in `code` names.
    fn used_paths(code: &str) -> Vec<String> {
        let mut paths = Vec::new();
        for statement in code.split(';') {
            let statement = statement.trim();
            let statement = statement
                .strip_prefix("pub ")
                .or_else(|| {
                    statement
                        .strip_prefix("pub(crate) ")
                        .or_else(|| statement.strip_prefix("pub(super) "))
                })
                .unwrap_or(statement)
                .trim();
            if let Some(tree) = statement.strip_prefix("use ") {
                expand_use("", tree, &mut paths);
            } else if let Some(name) = statement.strip_prefix("extern crate ") {
                paths.push(name.split(" as ").next().unwrap_or(name).trim().to_owned());
            }
        }
        paths
    }

    /// Whether `path` is, or lies under, `root`.
    fn is_under(path: &str, root: &str) -> bool {
        path == root
            || path
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with("::"))
    }

    /// What in `source` (before its test module) reaches IO or a workspace
    /// crate, as human-readable findings. Empty means the scan found nothing.
    fn violations(source: &str) -> Vec<String> {
        let code = code_of(source);
        let mut found = Vec::new();

        for path in used_paths(&code) {
            let path = path.trim_start_matches("::");
            let first = path.split("::").next().unwrap_or_default();
            if first.starts_with("ori_") || WORKSPACE_CRATES.contains(&first) {
                found.push(format!("imports the workspace crate in `{path}`"));
            }
            // `std` itself, a glob of it, or `std::time` whole: each brings
            // IO modules or clocks in under a shorter name.
            if matches!(path, "std" | "std::*" | "std::time" | "std::time::*") {
                found.push(format!("`{path}` brings IO or a clock in by a short name"));
            }
            if IO_PATHS.iter().any(|root| is_under(path, root)) {
                found.push(format!("imports `{path}`"));
            }
        }

        // Inline paths and names, with no `use`: `std::fs::read(..)`,
        // `ori_store::x`. Whole identifiers, so a longer name that merely
        // contains one of these is not a hit.
        let tokens: Vec<&str> = code
            .split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == ':'))
            .filter(|token| !token.is_empty())
            .collect();
        for token in tokens {
            let token = token.trim_start_matches("::");
            if IO_PATHS.iter().any(|root| is_under(token, root)) {
                found.push(format!("names `{token}` inline"));
            }
            let first = token.split("::").next().unwrap_or_default();
            if first.starts_with("ori_") && first != "ori_" {
                found.push(format!("names the workspace item `{token}` inline"));
            }
        }

        found.sort();
        found.dedup();
        found
    }

    #[test]
    fn ori_t_0058_the_scan_for_io_and_workspace_crates_catches_what_it_is_for() {
        // Each of these must be caught: brace imports, nested braces,
        // `self`, renames, a leading `::`, a glob, an inline path, and every
        // workspace crate by name.
        let must_catch = [
            "use std::{fs, io};",
            "use std::{collections::BTreeSet, fs::File};",
            "use std::io::{self, Read};",
            "use std::{process::Command, env};",
            "use std::fs as files;",
            "pub use std::net::TcpStream;",
            "use std::time::{Duration, SystemTime};",
            "use std::time::*;",
            "use std::*;",
            "use std::time;",
            "fn f() { let _ = std::fs::read(\"x\"); }",
            "fn f() { std::thread::sleep(d); }",
            "use ::ori_store::x;",
            "extern crate ori_broker;",
            "fn f() { ori_gates::run(); }",
            "use ori_new_crate_nobody_listed::x;",
        ];
        for name in WORKSPACE_CRATES {
            let source = format!("use {name}::thing;");
            assert!(
                !violations(&source).is_empty(),
                "the scan missed a use of {name}"
            );
        }
        for sample in must_catch {
            assert!(!violations(sample).is_empty(), "the scan missed: {sample}");
        }

        // And this must not be caught: what the declarations actually use,
        // plus a name that only contains a forbidden one.
        let clean = "use core::fmt;\nuse std::collections::BTreeSet;\nuse crate::types::Id;\n\
                     use std::{fmt::Debug, string::String};\n\
                     // use std::fs; in a comment is not an import\n\
                     fn f() { let std_fs_like = 1; let _ = std_fs_like; }\n\
                     \n#[cfg(test)]\nmod tests { use std::fs; }";
        assert_eq!(violations(clean), Vec::<String>::new());
    }

    #[test]
    fn ori_t_0058_the_declarations_import_no_workspace_crate_and_do_no_io() {
        for (name, source) in [
            ("adapters.rs", include_str!("adapters.rs")),
            ("adapters/identity.rs", include_str!("adapters/identity.rs")),
        ] {
            let code = code_of(source);
            assert!(
                code.len() > 2000,
                "{name}: the scan saw {} bytes of code; it is not reading the file",
                code.len()
            );
            assert!(
                !used_paths(&code).is_empty(),
                "{name}: the scan found no `use` at all; it is not parsing the file"
            );
            assert_eq!(violations(source), Vec::<String>::new(), "{name}");
        }
    }

    #[test]
    fn ori_t_0058_ori_core_declares_no_dependency_so_no_workspace_crate_can_be_imported() {
        // The real guarantee, as opposed to the tripwire above: with nothing
        // under `[dependencies]` (or the build or target sections) the
        // compiler cannot resolve any other crate. A dev-dependency does not
        // reach the library, but one on a workspace crate would build a second
        // copy of this crate for tests, so that is refused too.
        let manifest = include_str!("../Cargo.toml");
        let mut section = String::new();
        let mut problems = Vec::new();
        let mut entries_seen = 0;
        for line in manifest.lines().map(str::trim) {
            if line.starts_with('[') {
                section = line.to_owned();
            } else if !line.is_empty() && !line.starts_with('#') {
                entries_seen += 1;
                if section.contains("dev-dependencies") {
                    if line.starts_with("ori-") || line.starts_with("ori_") {
                        problems.push(format!("{section}: {line}"));
                    }
                } else if section.contains("dependencies") {
                    problems.push(format!("{section}: {line}"));
                }
            }
        }
        assert!(
            entries_seen > 0,
            "the manifest scan read no entries; it is not reading Cargo.toml"
        );
        assert!(
            manifest.contains("[dependencies]"),
            "the manifest has no [dependencies] section to check"
        );
        assert_eq!(
            problems,
            Vec::<String>::new(),
            "ori-core must declare no dependency (spec/LLD.md section 2). A new \
             dependency is an escalation (new_dependency), not an edit to this test"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0141: `AgentRuntime` and the types in its signatures, seen from
    // `ori-core`. The behaviour moved with the types and its tests stayed in
    // `ori-runtime`, where they run through the re-exports; these pin what is
    // new here: the one constructor that became public, and that the trait is
    // nameable and implementable from `ori-core` alone.
    // -----------------------------------------------------------------------

    /// Every [`StdioMode`]. The exhaustive `match` makes a mode added later
    /// fail to compile here until the tests below cover it.
    fn every_stdio_mode() -> [StdioMode; 3] {
        match StdioMode::Null {
            StdioMode::Null | StdioMode::Piped | StdioMode::Inherit => {}
        }
        [StdioMode::Null, StdioMode::Piped, StdioMode::Inherit]
    }

    #[test]
    fn ori_t_0141_a_launch_configuration_is_non_interactive_whichever_stdio_mode_built_it() {
        // ORI-P1-039, the fail-closed launch: `LaunchConfig::new` is public
        // because it sets `non_interactive` for every mode it is given.
        let args = vec!["-p".to_owned(), "hi".to_owned()];
        for mode in every_stdio_mode() {
            let launch =
                LaunchConfig::new("some-agent-cli", args.clone(), mode).expect("a program");
            assert!(
                launch.non_interactive(),
                "stdin={mode} must still be a non-interactive launch"
            );
            assert_eq!(launch.stdin(), mode);
            assert_eq!(launch.program(), "some-agent-cli");
            assert_eq!(launch.args().to_vec(), args);
            assert!(
                launch.to_string().ends_with("non_interactive=true)"),
                "{launch}"
            );
        }

        // The two named constructors choose their mode and are no different.
        let acp = LaunchConfig::acp("some-agent-cli", args.clone()).expect("a program");
        assert_eq!(acp.stdin(), StdioMode::Piped);
        assert!(acp.non_interactive());
        let headless = LaunchConfig::headless("some-agent-cli", args).expect("a program");
        assert_eq!(headless.stdin(), StdioMode::Null);
        assert!(headless.non_interactive());
    }

    #[test]
    fn ori_t_0141_a_launch_configuration_with_a_blank_program_is_refused_for_every_mode() {
        for mode in every_stdio_mode() {
            for blank in ["", "   ", "\t\n"] {
                assert_eq!(
                    LaunchConfig::new(blank, Vec::new(), mode).expect_err("a blank program"),
                    LaunchConfigError::EmptyProgram
                );
            }
        }
        assert_eq!(
            LaunchConfig::acp(" ", Vec::new()).expect_err("a blank program"),
            LaunchConfigError::EmptyProgram
        );
        assert_eq!(
            LaunchConfig::headless("", Vec::new()).expect_err("a blank program"),
            LaunchConfigError::EmptyProgram
        );
    }

    /// An adapter written against `ori-core` alone: no `ori-runtime`, no
    /// process. It keeps what the trait's contract asks of an implementer
    /// that the types can express: one session, a mode it requires, a
    /// transcript it returns a slice of.
    struct FakeRuntime {
        session: Option<Id>,
        transcript: Vec<Entry>,
    }

    impl FakeRuntime {
        fn check(&self, handle: &SessionHandle) -> Result<(), RuntimeError> {
            match &self.session {
                Some(held) if held == handle.id() => Ok(()),
                Some(_) => Err(RuntimeError::UnknownSession {
                    id: handle.id().clone(),
                }),
                None => Err(RuntimeError::NotSpawned),
            }
        }
    }

    impl AgentRuntime for FakeRuntime {
        fn spawn(&mut self, spec: SessionSpec) -> Result<SessionHandle, RuntimeError> {
            if self.session.is_some() {
                return Err(RuntimeError::AlreadySpawned);
            }
            if spec.launch.stdin() != StdioMode::Null {
                return Err(RuntimeError::WrongStdinMode {
                    required: StdioMode::Null,
                    found: spec.launch.stdin(),
                });
            }
            self.session = Some(spec.id.clone());
            Ok(SessionHandle::new(spec.id))
        }
        fn send(
            &mut self,
            handle: &SessionHandle,
            prompt: &str,
        ) -> Result<StopReason, RuntimeError> {
            self.check(handle)?;
            // `?` converts the `TranscriptError` through the `From` impl that
            // moved with the types.
            let entry = Entry::new(1, Timestamp::from_millis(1), Some("fake"), prompt, "")?;
            self.transcript.push(entry);
            Ok(StopReason::parse("end_turn"))
        }
        fn recv(&self, handle: &SessionHandle) -> Result<&[Entry], RuntimeError> {
            self.check(handle)?;
            Ok(&self.transcript)
        }
        fn kill(&mut self, handle: &SessionHandle) -> Result<(), RuntimeError> {
            self.check(handle)?;
            self.session = None;
            Ok(())
        }
        fn capabilities(&self) -> RuntimeCaps {
            RuntimeCaps::new(ModelFamily::parse("claude-3").expect("a family"), "fake")
                .expect("a name")
        }
    }

    #[test]
    fn ori_t_0141_agent_runtime_is_object_safe_and_implementable_from_ori_core_alone() {
        let mut runtime: Box<dyn AgentRuntime> = Box::new(FakeRuntime {
            session: None,
            transcript: Vec::new(),
        });
        let spec = |mode| SessionSpec {
            id: Id::parse(SAMPLE_ULID).expect("a ULID"),
            launch: LaunchConfig::new("fake-agent", Vec::new(), mode).expect("a program"),
            cwd: PathBuf::from("work"),
            envs: Vec::new(),
        };

        // A refusal by a control: wrong mode for this adapter, AICD section 17.
        let refusal = runtime
            .spawn(spec(StdioMode::Inherit))
            .expect_err("this adapter requires a closed stdin");
        assert!(refusal.is_refusal());
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(17)
        );

        let handle = runtime.spawn(spec(StdioMode::Null)).expect("spawned");
        assert_eq!(handle.id().as_str(), SAMPLE_ULID);
        assert_eq!(
            runtime
                .spawn(spec(StdioMode::Null))
                .expect_err("one session"),
            RuntimeError::AlreadySpawned
        );
        assert_eq!(
            runtime.send(&handle, "hello").expect("a turn"),
            StopReason::EndTurn
        );
        let entries = runtime.recv(&handle).expect("the transcript");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool(), Some("fake"));
        assert_eq!(entries[0].input(), "hello");
        assert_eq!(runtime.capabilities().adapter_name(), "fake");
        assert_eq!(runtime.capabilities().family().as_str(), "claude-3");

        // A `TranscriptError` reaches the caller as a `RuntimeError`, and is
        // not a refusal by a control.
        let blank = runtime.send(&handle, "  ").expect_err("nothing to record");
        assert_eq!(
            blank,
            RuntimeError::Transcript(TranscriptError::Blank {
                field: "input or output"
            })
        );
        assert!(!blank.is_refusal());
        assert_eq!(blank.methodology_ref(), None);

        let stranger = SessionHandle::new(Id::parse(OTHER_ULID).expect("a ULID"));
        assert_eq!(
            runtime.recv(&stranger).expect_err("not the held session"),
            RuntimeError::UnknownSession {
                id: Id::parse(OTHER_ULID).expect("a ULID")
            }
        );
        runtime.kill(&handle).expect("ended");
        assert_eq!(
            runtime.kill(&handle).expect_err("already ended"),
            RuntimeError::NotSpawned
        );
    }

    #[test]
    fn ori_t_0141_only_a_wrong_stdin_mode_is_a_refusal_and_it_cites_a_section_that_resolves() {
        // `RuntimeError`'s convention, moved unchanged: a state error or a
        // channel failure carries no `MethodologyRef`; the one refusal does.
        let every = vec![
            RuntimeError::Io {
                message: "boom".to_owned(),
            },
            RuntimeError::Protocol {
                message: "not json".to_owned(),
            },
            RuntimeError::Rpc {
                code: -32601,
                message: "no such method".to_owned(),
            },
            RuntimeError::UnexpectedEof,
            RuntimeError::Timeout {
                after: Duration::from_secs(20),
            },
            RuntimeError::DefectSink {
                message: "disk full".to_owned(),
            },
            RuntimeError::Transcript(TranscriptError::AttemptNotNumbered),
            RuntimeError::UnknownSession {
                id: Id::parse(SAMPLE_ULID).expect("a ULID"),
            },
            RuntimeError::AlreadySpawned,
            RuntimeError::NotSpawned,
            RuntimeError::WrongStdinMode {
                required: StdioMode::Piped,
                found: StdioMode::Inherit,
            },
            RuntimeError::AlreadyCompleted,
        ];
        // An exhaustive match, so a variant added later fails to compile here
        // until it is listed above and its convention is decided.
        for error in &every {
            match error {
                RuntimeError::Io { .. }
                | RuntimeError::Protocol { .. }
                | RuntimeError::Rpc { .. }
                | RuntimeError::UnexpectedEof
                | RuntimeError::Timeout { .. }
                | RuntimeError::DefectSink { .. }
                | RuntimeError::Transcript(_)
                | RuntimeError::UnknownSession { .. }
                | RuntimeError::AlreadySpawned
                | RuntimeError::NotSpawned
                | RuntimeError::WrongStdinMode { .. }
                | RuntimeError::AlreadyCompleted => {}
            }
        }
        assert_eq!(every.len(), 12);

        let mut refusals = 0;
        for error in &every {
            assert!(!error.to_string().is_empty());
            match error.methodology_ref() {
                Some(reason) => {
                    assert!(error.is_refusal(), "{error}");
                    assert_eq!(reason.section, 17);
                    assert!(reason.resolves(), "{reason} resolves");
                    assert!(
                        error.to_string().contains(&reason.to_string()),
                        "the message names its reference: {error}"
                    );
                    refusals += 1;
                }
                None => assert!(!error.is_refusal(), "{error}"),
            }
        }
        assert_eq!(refusals, 1, "WrongStdinMode is the one refusal");
    }

    #[test]
    fn ori_t_0160_a_session_specs_debug_names_each_variable_and_prints_no_value() {
        let spec = SessionSpec {
            id: Id::parse(SAMPLE_ULID).expect("a ULID"),
            launch: LaunchConfig::new("fake-agent", Vec::new(), StdioMode::Piped)
                .expect("a program"),
            cwd: PathBuf::from("work"),
            envs: vec![
                (
                    "ORI_ISSUED".to_owned(),
                    "value-that-must-never-print".to_owned(),
                ),
                ("PLAIN".to_owned(), "also-hidden".to_owned()),
            ],
        };
        for printed in [format!("{spec:?}"), format!("{spec:#?}")] {
            assert!(printed.contains("ORI_ISSUED"), "{printed}");
            assert!(printed.contains("PLAIN"), "{printed}");
            assert!(printed.contains("REDACTED"), "{printed}");
            assert!(
                !printed.contains("value-that-must-never-print"),
                "{printed}"
            );
            assert!(!printed.contains("also-hidden"), "{printed}");
            assert!(printed.contains("fake-agent"), "{printed}");
        }
        let copy = spec.clone();
        assert_eq!(copy.envs, spec.envs, "Clone still copies the values");
    }
}
