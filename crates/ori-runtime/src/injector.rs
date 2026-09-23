//! Credential injection at spawn: AICD §17, AICD §27, `spec/LLD.md` section 2
//! ("`Injector` (env or file at spawn)"), `spec/SECURITY_NOTES.md` trust
//! boundary 2 ("They receive credentials at spawn, scoped and expiring; they
//! never receive the operator's tokens").
//!
//! Criteria this module carries the runtime half of:
//!
//! - ORI-P1-037: "Provider key set at application level; project B has an
//!   override | Spawn a coder in A and in B | A receives the application
//!   key, B receives its override; neither key appears in any event or
//!   log." `ori_broker::keychain::resolve_provider_binding` is the
//!   resolution mechanism (`ori-broker`'s own scope); this module is the
//!   half that actually spawns and is where "neither key appears" has to
//!   hold against a real child process and its real argv, not only against
//!   an event payload.
//! - ORI-P1-020: "Any session ended (any outcome) | Inspect issuances |
//!   Every issuance for the session is revoked with a timestamp before the
//!   process is terminated." `ori_broker::issuance::revoke_session`
//!   guarantees the half before its own return; its own module doc comment
//!   names the half it cannot guarantee: "that the caller waits for that
//!   receipt before terminating the process." [`Injector::end`] is that
//!   half.
//!
//! # What is here
//!
//! - [`Injector`], a stateless unit type whose two methods are this
//!   module's whole job: [`Injector::spawn`] resolves a binding, reads its
//!   secret, records the issuance and starts the child with the secret
//!   placed only through [`std::process::Command::env`]; [`Injector::end`]
//!   revokes before it stops anything.
//! - [`SpawnRequest`], the caller-supplied half of a spawn (which binding,
//!   which identity, which argv), bundled the way
//!   `ori_broker::issuance::NewIssuance` bundles [`ori_broker::issuance::issue_credential`]'s
//!   own fields (clippy's plain-argument-list threshold is seven).
//! - [`SpawnedSession`], the type that holds the running child. It holds
//!   exactly what [`Injector::end`] needs to revoke first: the session
//!   identifier `ori_broker::issuance::revoke_session` filters on, and
//!   nothing about the operator's own tokens, which this module never reads
//!   at all (see "Where the secret comes from, and where it does not" below).
//! - [`Ended`], what [`Injector::end`] hands back: the [`crate::session::Session`]
//!   with both teardown steps recorded, the
//!   [`ori_broker::issuance::RevocationReceipt`] that proved the first one,
//!   and the child's [`std::process::ExitStatus`].
//! - [`InjectorError`], wrapping a refused [`crate::session::SessionError`]
//!   or [`ori_broker::issuance::IssuanceError`] (each keeps its own
//!   [`ori_core::error::MethodologyRef`]), an operational
//!   [`ori_broker::keychain::KeychainError`] or [`std::io::Error`], or this
//!   module's own two refusals: no binding resolves, or a
//!   [`ori_broker::issuance::RevocationReceipt`] was handed to the wrong
//!   session's [`SpawnedSession`].
//!
//! # Where the secret comes from, and where it does not
//!
//! [`Injector::spawn`] reads exactly one secret: the value
//! [`ori_broker::keychain::resolve_provider_binding`] names for this
//! product and provider, through [`ori_broker::keychain::Keychain::get`].
//! It never reads `spec/ENV_SETUP.md`'s application-level VCS host tokens,
//! never reads `.env*`, never reads the operator's own credentials for
//! anything; "the operator's own tokens are never read" (this ticket's own
//! text) is true here because nothing in this file ever asks the keychain,
//! or anything else, for one.
//!
//! # How the secret reaches the child, and every place it provably does not
//!
//! `build_command` is the one place this module places the secret
//! anywhere: `command.env(env_var, plaintext)`, inside
//! [`ori_broker::keychain::Secret::expose`]'s closure, so the plaintext
//! never escapes into a local variable this function's body could
//! accidentally reuse. Three things this module never does with it, each
//! named because a real defect looks exactly like it:
//!
//! 1. **Never an argument.** [`SpawnRequest::args`] is the caller's own
//!    argv, untouched; `build_command` never calls
//!    [`std::process::Command::arg`] with the secret. A command-line
//!    argument is visible to every other process on the machine through
//!    `ps` (Unix) or the process list (Windows) for the life of the child,
//!    which an environment variable is not (a `/proc/<pid>/environ` read on
//!    Linux needs the same or greater privilege as the process itself;
//!    Windows and macOS keep a process's own environment out of the plain
//!    process listing entirely). `tests::ori_t_0032_the_secret_never_appears_in_the_built_commands_argv`
//!    asserts this directly against [`std::process::Command::get_args`],
//!    not against prose.
//! 2. **Never `std::env::set_var` in this process.** That call mutates the
//!    engine's own environment, which every process this engine spawns
//!    afterward inherits, whether or not it is this session's child, for as
//!    long as the engine runs; it is also a documented data race the moment
//!    two sessions spawn concurrently (`std::env::set_var`'s own safety
//!    note). `build_command` only ever calls
//!    [`std::process::Command::env`], which stages the variable on the
//!    [`std::process::Command`] value itself and hands it to the child at
//!    `exec`/`CreateProcess` time; the parent's own environment table is
//!    never touched.
//!    `tests::ori_t_0032_the_secret_never_appears_in_the_parents_own_environment`
//!    asserts `std::env::var` for the injected name is `Err` in the test
//!    process itself, both before and after a real spawn.
//! 3. **Never a file, a transcript or an event.** No function in this
//!    module ever calls [`ori_broker::keychain::Secret::expose`] except
//!    inside `build_command`'s one closure; grep for `.expose(` in this
//!    file to check. [`ori_broker::issuance::issue_credential`]'s own
//!    payload never carries a secret either (see that function's own doc
//!    comment); this module passes it `request.scope`, never anything read
//!    from the keychain.
//!
//! # An injected child never inherits the engine's stdio
//!
//! [`Injector::spawn`] gives every child three explicit [`Stdio`] values,
//! never [`std::process::Command`]'s own default (inherit the parent's
//! stdin, stdout and stderr; for this module's own process, the engine's
//! own): standard input is always [`Stdio::null`], whatever
//! [`SpawnRequest::capture_stdout`] is set to; standard output is
//! [`Stdio::piped`] only when that field asks for it, [`Stdio::null`]
//! otherwise, never inherited either way; standard error is always
//! [`Stdio::null`], because nothing in this module ever reads a piped one,
//! and an unread pipe fills and blocks the child once its OS buffer is
//! full, which `null` cannot do. `spec/SECURITY_NOTES.md` trust boundary 2
//! is the property this closes: sessions "talk to the engine only through
//! the MCP server and ACP"; an inherited stdin or stdout would give an
//! injected child a third, unaccounted channel straight into the engine's
//! own process, bypassing both. [`crate::headless`]'s own module doc
//! comment states the same shape of property for its adapter, "standard
//! input closed so that no question it tries to ask can ever be answered,
//! or even received" (ORI-T-0033); this module did not carry it for its own
//! child until this ticket, ORI-T-0111, which closes it here for all three
//! streams, not stdin alone.
//!
//! `tests::ori_t_0111_an_injected_child_never_inherits_this_processs_own_stdin`,
//! `tests::ori_t_0111_a_child_spawned_with_capture_stdout_false_never_inherits_this_processs_own_stdout`
//! and `tests::ori_t_0111_a_child_never_inherits_this_processs_own_stderr_either`
//! each prove one stream, over a real grandchild of a real wrapper process
//! (itself spawned through [`Injector::spawn`], so the property is checked
//! against this module's own real code path, not trusted from the source
//! above), never by inspecting a [`std::process::Command`] alone: a
//! `Command` never told to change a stream from its default reports nothing
//! about that stream either way through
//! [`std::process::Command::get_args`] or
//! [`std::process::Command::get_envs`], which is exactly how this defect
//! went unnoticed until a real run of this module's own test suite printed
//! a grandchild's report straight into its own libtest summary.
//!
//! # Kill cannot be reached without a receipt: what makes it impossible, and
//! # what still bypasses it
//!
//! [`SpawnedSession::stop`] is the only function in this crate that calls
//! [`std::process::Child::kill`] on a session's child, and its signature is
//! `stop(self, receipt: &ori_broker::issuance::RevocationReceipt)`.
//! [`ori_broker::issuance::RevocationReceipt`] has no public constructor
//! anywhere in `ori-broker` (that crate's own module doc comment states this
//! as its vacuity guard); the only way any value of that type exists at all
//! is as [`ori_broker::issuance::revoke_session`]'s own return. So within
//! [`Injector::end`] as written, the statement that calls `.stop(&receipt)`
//! cannot appear before the statement `let receipt = revoke_session(...)?;`
//! that defines `receipt`: Rust refuses to compile a use of a binding before
//! its own `let`. Reordering the two calls inside this function is not a
//! matter of discipline here, it does not compile. That is the "impossible"
//! this ticket asks for, and it is why plant 1 in the table below is
//! expected to fail the build rather than merely fail an assertion.
//!
//! What is not closed, named rather than left for a reader to discover:
//!
//! - **A determined edit within this same file** could still add a second,
//!   bypassing method that reaches [`SpawnedSession`]'s private `child`
//!   field directly without a receipt (Rust's privacy is module-scoped, and
//!   `tests` is a child module of this one). This module provides no such
//!   method; adding one is a visible, reviewable diff to this file, not a
//!   silent one, and `tests::ori_p1_020_every_outcome_revokes_before_the_process_is_recorded_stopped`
//!   would need to be routed around too, since it drives the real
//!   [`Injector::end`] path and checks the receipt's own count.
//! - **The child dying on its own** before [`Injector::end`] runs.
//!   [`SpawnedSession::stop`] handles this already: `Child::kill` on an
//!   already-exited process fails (there is nothing left to signal), the
//!   failure is discarded, and `Child::wait` still reaps it and reports its
//!   real exit status either way. Revocation still happens first regardless,
//!   because [`Injector::end`]'s statement order does not depend on whether
//!   the child is still alive.
//! - **A caller that drops [`SpawnedSession`] instead of calling
//!   [`Injector::end`].** [`std::process::Child`]'s own [`Drop`] does not
//!   kill the process (documented on that type); a dropped [`SpawnedSession`]
//!   leaves both the child running and its credential unrevoked, which is
//!   exactly the orphan [`crate::session::Session::residue`] is built to
//!   report once a caller does eventually try to end that session. This
//!   module cannot close that seam: it has no way to run code when a value
//!   is dropped without an `unsafe`-free reason to reach for one (there is
//!   none here), and CLAUDE.md's rule on `unsafe` (a ticket that names why,
//!   which this one does not) forbids reaching for it anyway. The seam is
//!   closed by the caller's obligation to call [`Injector::end`] for every
//!   session it spawns, the same discipline `ori_broker::issuance`'s own
//!   module doc comment names for `revoke_session` itself.
//!
//! ```mermaid
//! sequenceDiagram
//!   participant Injector as Injector (this module)
//!   participant Keychain as ori-broker::keychain
//!   participant Broker as ori-broker::issuance
//!   participant OS as the operating system
//!   Injector->>Keychain: resolve_provider_binding, then Keychain::get
//!   Keychain-->>Injector: Secret (never logged)
//!   Injector->>Broker: issue_credential
//!   Broker-->>Injector: Event (credential.issued, no secret)
//!   Injector->>OS: build_command (Command::env only), spawn
//!   OS-->>Injector: SpawnedSession (Child, session_id)
//!   Note over Injector,OS: the session runs
//!   Injector->>Broker: revoke_session
//!   Broker-->>Injector: RevocationReceipt
//!   Injector->>Injector: Session::record(CredentialsRevoked, receipt.at())
//!   Injector->>OS: SpawnedSession::stop(receipt): kill, then wait
//!   OS-->>Injector: ExitStatus
//!   Injector->>Injector: Session::record(ProcessStopped, stopped_at)
//! ```
//!
//! Must not: hold credentials beyond a session (`spec/LLD.md` section 2).
//! [`SpawnedSession`] holds a [`ori_core::types::Id`], never a
//! [`ori_broker::keychain::Secret`]; once [`Injector::end`] consumes it, no
//! value in this crate names that session's secret at all, because none ever
//! did outside `build_command`'s own stack frame. Must not: let a child
//! inherit this process's own stdio; see "An injected child never inherits
//! the engine's stdio" above.

use core::fmt;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::process::Child;
use std::process::ChildStdout;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;

use ori_broker::identity::AgentIdentity;
use ori_broker::issuance::IssuanceError;
use ori_broker::issuance::IssuanceScope;
use ori_broker::issuance::NewIssuance;
use ori_broker::issuance::RevocationReceipt;
use ori_broker::issuance::issue_credential;
use ori_broker::issuance::revoke_session;
use ori_broker::keychain::Keychain;
use ori_broker::keychain::KeychainError;
use ori_broker::keychain::ProviderBinding;
use ori_broker::keychain::Secret;
use ori_broker::keychain::resolve_provider_binding;
use ori_core::error::MethodologyRef;
use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;

use crate::session::Session;
use crate::session::SessionError;
use crate::session::TeardownStep;

// ---------------------------------------------------------------------------
// credential_env_var_name
// ---------------------------------------------------------------------------

/// The environment variable a session's provider credential is injected
/// under, for `provider`: `spec/ENV_SETUP.md` section 4's
/// `PROVIDER_<NAME>_API_KEY` convention, applied to the one variable the
/// child actually receives, so the name a human reading `spec/ENV_SETUP.md`
/// already knows is the name the child sees, not a name invented here.
#[must_use]
pub fn credential_env_var_name(provider: &str) -> String {
    format!("PROVIDER_{}_API_KEY", provider.to_ascii_uppercase())
}

// ---------------------------------------------------------------------------
// build_command
// ---------------------------------------------------------------------------

/// Builds the [`std::process::Command`] a session's child runs from:
/// `program` and `args` exactly as the caller supplied them, plus one
/// environment variable, `env_var`, set to `secret`'s plaintext through
/// [`ori_broker::keychain::Secret::expose`] and
/// [`std::process::Command::env`] only. Constructing a [`Command`] does not
/// spawn anything ([`std::process::Command::new`]'s own documentation); this
/// function is pure in that sense, and is exercised directly (never only
/// through [`Injector::spawn`]) by every test in this module that inspects
/// what a session's child was actually given, via
/// [`std::process::Command::get_args`] and
/// [`std::process::Command::get_envs`], stable since Rust 1.57, without
/// spawning a process to find out.
fn build_command(program: &OsStr, args: &[OsString], env_var: &str, secret: &Secret) -> Command {
    let mut command = Command::new(program);
    command.args(args);
    secret.expose(|plaintext| {
        command.env(env_var, plaintext);
    });
    command
}

// ---------------------------------------------------------------------------
// SpawnRequest
// ---------------------------------------------------------------------------

/// What [`Injector::spawn`] needs beyond a keychain, a database, a time and
/// an actor, bundled into one value: clippy's own plain-argument-list
/// threshold is seven, the same reason
/// [`ori_broker::issuance::NewIssuance`] bundles
/// [`ori_broker::issuance::issue_credential`]'s own fields (that type's own
/// doc comment names the same rule).
pub struct SpawnRequest<'a> {
    /// Every binding known for this application and its projects; passed
    /// straight to [`resolve_provider_binding`].
    pub bindings: &'a [ProviderBinding],
    /// The provider this session needs a key for (`"openai"`,
    /// `"anthropic"`, and so on).
    pub provider: &'a str,
    /// The identity this session runs as. Its own `product_id` is what
    /// [`resolve_provider_binding`] resolves against.
    pub identity: &'a AgentIdentity,
    /// The session this credential is bound to
    /// (`spec/DATA_MODEL.md` section 4, "Every `CredentialIssuance` is bound
    /// to one session and expires with it").
    pub session_id: Id,
    /// The issuance identifier. The caller's to supply: neither this module
    /// nor `ori_broker::issuance` has a clock or a source of randomness.
    pub issuance_id: Id,
    /// The scope this credential is issued under.
    pub scope: IssuanceScope,
    /// When this credential expires on its own, absent when only revocation
    /// on session end retires it.
    pub expires_at: Option<Timestamp>,
    /// The program to run.
    pub program: OsString,
    /// The arguments to run it with. Never the secret; see this module's
    /// own doc comment.
    pub args: Vec<OsString>,
    /// Whether to pipe the child's standard output back to this process, so
    /// a caller (in production, the transcript writer; in this module's own
    /// tests, [`SpawnedSession::take_stdout`]) can read it. `false` closes
    /// it ([`Stdio::null`]); it never inherits the engine's own stdout
    /// either way. See this module's own doc comment, "An injected child
    /// never inherits the engine's stdio": the child's standard input and
    /// standard error are not caller-controlled at all, and are always
    /// closed the same way, whatever this field is set to.
    pub capture_stdout: bool,
}

// ---------------------------------------------------------------------------
// SpawnedSession
// ---------------------------------------------------------------------------

/// The running child of one session, and the session identifier
/// [`Injector::end`] needs to revoke it: `spec/LLD.md` section 2's
/// `Injector`.
///
/// Holds nothing else: no [`ori_broker::keychain::Secret`] (see this
/// module's own doc comment, "Must not"), and no
/// [`ori_broker::issuance::RevocationReceipt`], because the only place one
/// of those exists is a local variable inside [`Injector::end`], never a
/// field of a value that outlives that call.
#[derive(Debug)]
pub struct SpawnedSession {
    child: Child,
    session_id: Id,
}

impl SpawnedSession {
    /// The session this child belongs to, which is also the one
    /// [`SpawnedSession::stop`] refuses to stop on any other session's
    /// receipt.
    #[must_use]
    pub const fn session_id(&self) -> &Id {
        &self.session_id
    }

    /// Stops the child, refusing to run at all unless `receipt` proves
    /// (`ori_broker::issuance::RevocationReceipt::session_id`) that this
    /// exact session's credentials were already revoked. See this module's
    /// own doc comment, "Kill cannot be reached without a receipt", for what
    /// makes calling this before a genuine revocation impossible to write
    /// inside [`Injector::end`], and what still is not closed.
    ///
    /// `Child::kill` on a child that already exited on its own fails (there
    /// is nothing left to signal); that failure is discarded here rather
    /// than propagated, because "the child dying on its own" is a named,
    /// accepted case (this module's own doc comment) and not a reason to
    /// refuse ending the session. `Child::wait` still reaps the process and
    /// reports its real exit status regardless of whether the kill signal
    /// was the thing that ended it.
    ///
    /// # Errors
    ///
    /// [`InjectorError::ReceiptSessionMismatch`] if `receipt` names a
    /// different session; [`InjectorError::Wait`] if the operating system
    /// could not be asked for the child's exit status at all.
    pub fn stop(mut self, receipt: &RevocationReceipt) -> Result<ExitStatus, InjectorError> {
        if receipt.session_id() != &self.session_id {
            return Err(InjectorError::ReceiptSessionMismatch {
                expected: self.session_id.clone(),
                actual: receipt.session_id().clone(),
            });
        }
        let _ignored_already_exited = self.child.kill();
        self.child.wait().map_err(InjectorError::Wait)
    }

    /// Takes the child's standard output, if [`SpawnRequest::capture_stdout`]
    /// asked for it to be piped rather than inherited. `None` on a second
    /// call, or if it was never piped in the first place
    /// ([`std::process::Child::stdout`]'s own behaviour).
    ///
    /// Used by this module's own tests to prove what a real child process
    /// received without ever printing the secret itself; see "The test
    /// child design" in this ticket's own report, and
    /// `tests::ori_t_0032_child_reporter`.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }
}

// ---------------------------------------------------------------------------
// Ended
// ---------------------------------------------------------------------------

/// What [`Injector::end`] hands back once a session's credentials are
/// revoked and its process is stopped, in that order.
pub struct Ended {
    /// The session, with `CredentialsRevoked` and `ProcessStopped` both
    /// recorded (`crate::session::Teardown`).
    pub session: Session,
    /// The proof that revocation happened, and how many issuances it found.
    pub receipt: RevocationReceipt,
    /// The child's exit status, from [`SpawnedSession::stop`].
    pub status: ExitStatus,
}

// ---------------------------------------------------------------------------
// Injector
// ---------------------------------------------------------------------------

/// The credential injector: `spec/LLD.md` section 2 ("`Injector` (env or
/// file at spawn)"), AICD §17, AICD §27.
///
/// Stateless: a unit type whose two methods are this ticket's two halves,
/// "Spawn with a credential" and "End with revocation first, structurally".
/// Named `Injector` rather than left as two free functions because
/// `spec/LLD.md` section 2 names this crate's item that, the same way
/// `crate::session::Session` and `crate::worktree::Worktree` are named after
/// their own rows in that table.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Injector;

impl Injector {
    /// Resolves which binding this identity's product and `request.provider`
    /// receive, reads its secret, records the issuance, and starts the
    /// child with the secret placed only through `build_command`. See this
    /// module's own doc comment, "How the secret reaches the child".
    ///
    /// The issuance is recorded (durably; `ori_broker::issuance::issue_credential`
    /// only returns after its own `EventLog::append` commits) before the
    /// child is spawned, so a session that is later found running always has
    /// a matching `credential.issued` event to revoke against; a spawn that
    /// fails after that leaves an issuance with no process, which
    /// [`ori_broker::issuance::revoke_session`]'s own idempotence handles the
    /// same as any other unrevoked issuance.
    ///
    /// # Errors
    ///
    /// [`InjectorError::NoBinding`] if neither an application default nor a
    /// project override resolves; [`InjectorError::Keychain`] if the
    /// resolved binding's key reference is not in the keychain;
    /// [`InjectorError::Issuance`] if recording the issuance fails;
    /// [`InjectorError::Spawn`] if the operating system refuses to start the
    /// process at all.
    pub fn spawn(
        keychain: &dyn Keychain,
        db: &mut ProductDb,
        at: Timestamp,
        actor: Actor,
        request: SpawnRequest<'_>,
    ) -> Result<SpawnedSession, InjectorError> {
        let binding = resolve_provider_binding(
            request.bindings,
            request.identity.product_id(),
            request.provider,
        )
        .ok_or_else(|| InjectorError::NoBinding {
            product_id: request.identity.product_id().clone(),
            provider: request.provider.to_owned(),
        })?;
        let secret = keychain.get(binding.key_ref())?;

        issue_credential(
            db,
            at,
            actor,
            request.identity,
            NewIssuance {
                id: request.issuance_id,
                session_id: request.session_id.clone(),
                scope: request.scope,
                expires_at: request.expires_at,
            },
        )?;

        let env_var = credential_env_var_name(request.provider);
        let mut command = build_command(&request.program, &request.args, &env_var, &secret);
        // An injected child never inherits the engine's own stdio: see this
        // module's own doc comment, "An injected child never inherits the
        // engine's stdio". Standard input is always closed, whatever
        // request.capture_stdout is: nothing here gives an agent the
        // engine's own standard input.
        command.stdin(Stdio::null());
        command.stdout(if request.capture_stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        // Standard error is always closed too: never inherited, and never
        // piped either, because nothing in this module ever reads a piped
        // stderr, and an unread pipe fills and blocks the child once its OS
        // buffer is full. A caller that needs the child's stderr captured
        // is a later, explicit ticket's choice, not this one's default.
        command.stderr(Stdio::null());
        let child = command.spawn().map_err(InjectorError::Spawn)?;

        Ok(SpawnedSession {
            child,
            session_id: request.session_id,
        })
    }

    /// Ends a session for any outcome the same way: revokes every issuance
    /// [`ori_broker::issuance::revoke_session`] finds for it, records that on
    /// `session` from the receipt's own timestamp, and only then stops the
    /// child and records that too. See this module's own doc comment, "Kill
    /// cannot be reached without a receipt".
    ///
    /// Deliberately does not take a `crate::session::Outcome`: revocation is
    /// the same call whatever the outcome, the same restraint
    /// `ori_broker::issuance`'s own module doc comment states for
    /// [`revoke_session`] and [`issue_credential`] ("Neither... takes an
    /// `AgentSession` outcome as a parameter at all"). This crate, unlike
    /// `ori-broker`, can name `crate::session::Outcome` directly
    /// (`spec/LLD.md` section 2's dependency diagram runs `ori-runtime` to
    /// `ori-broker`, not the reverse), so
    /// `tests::ori_p1_020_every_outcome_revokes_before_the_process_is_recorded_stopped`
    /// drives the real enum through an exhaustive `match`, with no wildcard
    /// arm, rather than a mirror declared locally the way
    /// `ori_broker::issuance`'s own equivalent test has to.
    ///
    /// `revoked_at` and `stopped_at` are two separate timestamps, not one
    /// reused for both: this crate reads no clock of its own anywhere
    /// (`crate::worktree`, `crate::session`, `crate::budget` and
    /// `crate::transcript` all take `at: Timestamp` from their caller), and a
    /// real kill takes real wall-clock time after a real revocation returns,
    /// so the caller that does have a clock supplies both. `stopped_at`
    /// earlier than `revoked_at` is refused by `crate::session::Teardown::record`
    /// itself (`SessionError::StepNotAfter`), not by this function.
    ///
    /// # Errors
    ///
    /// [`InjectorError::Issuance`] if revocation itself fails;
    /// [`InjectorError::Session`] if `session` refuses either step (it is
    /// not `Running`, or `stopped_at` is before `revoked_at`);
    /// [`InjectorError::Wait`] if the operating system could not report the
    /// child's exit status.
    pub fn end(
        spawned: SpawnedSession,
        session: Session,
        db: &mut ProductDb,
        revoked_at: Timestamp,
        stopped_at: Timestamp,
        actor: Actor,
    ) -> Result<Ended, InjectorError> {
        let receipt = revoke_session(db, revoked_at, actor, spawned.session_id())?;
        let session = session.record(TeardownStep::CredentialsRevoked, receipt.at())?;
        let status = spawned.stop(&receipt)?;
        let session = session.record(TeardownStep::ProcessStopped, stopped_at)?;
        Ok(Ended {
            session,
            receipt,
            status,
        })
    }
}

// ---------------------------------------------------------------------------
// InjectorError
// ---------------------------------------------------------------------------

/// Everything [`Injector`] and [`SpawnedSession`] refuse or cannot do: AICD
/// §17, AICD §27.
///
/// The split is the one `ori_broker::keychain::KeychainError` and
/// `ori_broker::issuance::IssuanceError` both draw for themselves: a wrapped
/// refusal keeps the [`MethodologyRef`] it already carried
/// ([`InjectorError::Issuance`], [`InjectorError::Session`]);
/// [`InjectorError::ReceiptSessionMismatch`] is a refusal of this module's
/// own and carries one; the rest are operational failures (no binding
/// configured, the operating system refusing a call) and carry none.
#[derive(Debug)]
#[non_exhaustive]
pub enum InjectorError {
    /// Neither an application-level default nor a project override resolves
    /// for this product and provider.
    NoBinding {
        /// The product a binding was sought for.
        product_id: Id,
        /// The provider a binding was sought for.
        provider: String,
    },
    /// The keychain failed to produce the secret a resolved binding named.
    Keychain(KeychainError),
    /// Issuing or revoking a credential failed.
    Issuance(IssuanceError),
    /// The session refused a teardown step: out of order, repeated, or
    /// recorded for a session that is not running.
    Session(SessionError),
    /// The operating system refused to start the child process.
    Spawn(io::Error),
    /// The operating system could not be asked for the child's exit status.
    Wait(io::Error),
    /// [`SpawnedSession::stop`] was given a
    /// [`RevocationReceipt`] for a different session than the one it holds:
    /// `spec/SECURITY_NOTES.md` trust boundary 2, a credential is scoped to
    /// one session, and so is the proof that it was revoked.
    ReceiptSessionMismatch {
        /// The session [`SpawnedSession::stop`] was called on.
        expected: Id,
        /// The session the receipt was actually for.
        actual: Id,
    },
}

impl InjectorError {
    /// The methodology section a refusal was made under, for the refusal
    /// this error wraps or carries, and `None` for an operational failure.
    /// See this type's own doc comment.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Issuance(inner) => inner.methodology_ref(),
            Self::Session(inner) => inner.methodology_ref(),
            // AICD §27's secrets architecture: a credential's revocation is
            // scoped to the one session it was issued to, the same section
            // crate::session::StepKind::CredentialsRevoked itself cites.
            Self::ReceiptSessionMismatch { .. } => Some(MethodologyRef {
                section: 27,
                subsection: None,
            }),
            Self::NoBinding { .. } | Self::Keychain(_) | Self::Spawn(_) | Self::Wait(_) => None,
        }
    }
}

impl fmt::Display for InjectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBinding {
                product_id,
                provider,
            } => write!(
                f,
                "no provider binding resolves for product {product_id} and provider {provider:?}"
            ),
            Self::Keychain(inner) => write!(f, "{inner}"),
            Self::Issuance(inner) => write!(f, "{inner}"),
            Self::Session(inner) => write!(f, "{inner}"),
            Self::Spawn(inner) => write!(f, "the child process could not be started: {inner}"),
            Self::Wait(inner) => {
                write!(
                    f,
                    "the child process's exit status could not be read: {inner}"
                )
            }
            Self::ReceiptSessionMismatch { expected, actual } => write!(
                f,
                "a revocation receipt for session {actual} cannot stop session {expected}'s \
                 process"
            ),
        }
    }
}

impl std::error::Error for InjectorError {}

impl From<KeychainError> for InjectorError {
    fn from(err: KeychainError) -> Self {
        Self::Keychain(err)
    }
}

impl From<IssuanceError> for InjectorError {
    fn from(err: IssuanceError) -> Self {
        Self::Issuance(err)
    }
}

impl From<SessionError> for InjectorError {
    fn from(err: SessionError) -> Self {
        Self::Session(err)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::io::BufRead;
    use std::io::Read;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use std::time::Instant;

    use ori_broker::identity::IdentityRuntime;
    use ori_broker::identity::MemoryScopes;
    use ori_broker::issuance::issuances_for_session;
    use ori_broker::keychain::InMemoryKeychain;
    use ori_broker::keychain::KeyRef;
    use ori_core::types::ModelFamily;
    use ori_core::types::Role;
    use ori_store::event_log::EventLog;

    use super::*;
    use crate::acp::wait_with_timeout;
    use crate::session::Disposition;
    use crate::session::Outcome;
    use crate::session::StepKind;
    use crate::worktree::Worktree;

    // -----------------------------------------------------------------------
    // Fixtures, the same shapes crate::session::tests, ori_broker::keychain::tests
    // and ori_broker::issuance::tests each already use.
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
                "ori-t-0032-{label}-{}-{unique}",
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
    /// `ori_broker::keychain::tests` and `ori_broker::issuance::tests` use.
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

    fn open_db(scratch: &Scratch, product_id: &Id) -> ProductDb {
        ProductDb::open(&scratch.path, product_id.as_str(), at(1_000))
            .expect("a fresh product database opens")
    }

    fn app_binding(provider: &str, key_ref: &str) -> ProviderBinding {
        ProviderBinding::new(
            id("BINDING-APP"),
            None,
            provider,
            KeyRef::parse(key_ref).expect("a key ref"),
            Vec::new(),
        )
        .expect("a valid application-level binding")
    }

    fn product_binding(product_id: Id, provider: &str, key_ref: &str) -> ProviderBinding {
        ProviderBinding::new(
            id("BINDING-PROD"),
            Some(product_id),
            provider,
            KeyRef::parse(key_ref).expect("a key ref"),
            Vec::new(),
        )
        .expect("a valid per-project binding")
    }

    /// An absolute path, on every platform, that names nothing on disk: the
    /// same fixture `crate::session::tests::absent` documents for itself.
    fn absent(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "ori-t-0032-absent-{label}-{}-{unique}",
            std::process::id()
        ));
        assert!(!path.exists(), "this fixture names nothing on disk");
        path
    }

    /// A running session, whose worktree is absolute and not on disk (this
    /// module never touches the worktree; only credentials and the process
    /// are its concern).
    fn running_session(session_id: Id) -> Session {
        let worktree = Worktree::new(
            session_id.clone(),
            absent("session").join(session_id.as_str()),
            "feat/ORI-T-0032",
        )
        .expect("a valid worktree");
        Session::spawning(session_id.clone(), id("IDENTITY"), None, worktree, at(1))
            .expect("the worktree is this session's")
            .running(at(2))
            .expect("spawning goes to running")
    }

    /// The path of the test binary currently running: the re-exec target for
    /// this module's two child fixtures. See the module's own report,
    /// "the test child design".
    fn current_test_binary() -> PathBuf {
        std::env::current_exe().expect("the running test binary has a path")
    }

    /// The argv (without the leading program) that re-runs exactly one test
    /// of this same binary, with output uncaptured.
    ///
    /// `--ignored` is required because both child fixtures below carry
    /// `#[ignore]`: an ordinary `cargo test` run must never execute either
    /// of them on its own (one sleeps 20 seconds unconditionally, and
    /// neither asserts anything, so left un-ignored they would cost every
    /// suite run real wall time on all three CI platforms while verifying
    /// nothing, AICD §39's defect class in miniature). `--exact` still
    /// selects only the one named test even with `--ignored` set, so this
    /// re-exec runs the fixture and only the fixture.
    fn reexec_args(test_path: &str) -> Vec<OsString> {
        vec![
            OsString::from(test_path),
            OsString::from("--ignored"),
            OsString::from("--exact"),
            OsString::from("--nocapture"),
            OsString::from("--test-threads=1"),
        ]
    }

    /// `(length, a non-cryptographic hash)` of `value`, the std-only stand-in
    /// this module's report names for a real digest: no new dependency is
    /// added for it, and the point is only ever "the child received the same
    /// bytes", never "the hash is forgeable-resistant".
    fn hash_len(value: &str) -> (usize, u64) {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        (value.len(), hasher.finish())
    }

    /// The prefix `tests::ori_t_0032_child_reporter` prints, so a scan of its
    /// stdout for this exact needle cannot be confused with libtest's own
    /// banner text around it.
    const CHILD_REPORT_PREFIX: &str = "ORI_T_0032_CHILD_REPORT";

    /// Reads a report line out of `stdout`, `(len, hash)` if the child found
    /// its expected variable present, `None` if it reported absent. Panics on
    /// any other shape, since a report in neither form means the fixture
    /// itself broke, not that the assertion under test failed.
    ///
    /// Searches for the prefix anywhere in the text rather than requiring a
    /// line to start with it: libtest's own `--nocapture` writes "test
    /// `<name>` ... " immediately before the test body runs and appends
    /// "ok\n" immediately after, with no separator of its own, so the
    /// report this fixture prints lands mid-line between the two, not at a
    /// line's own start.
    fn parse_report(stdout: &str) -> Option<(usize, u64)> {
        let start = stdout
            .find(CHILD_REPORT_PREFIX)
            .unwrap_or_else(|| panic!("no {CHILD_REPORT_PREFIX} report in child output: {stdout}"));
        let rest = &stdout[start..];
        let line = rest.lines().next().expect("at least one line follows");
        if line.contains("present=0") {
            return None;
        }
        let len = field(line, "len=").parse().expect("len is a number");
        let hash = field(line, "hash=").parse().expect("hash is a number");
        Some((len, hash))
    }

    fn field<'a>(line: &'a str, key: &str) -> &'a str {
        let start = line.find(key).expect("field present") + key.len();
        line[start..]
            .split_whitespace()
            .next()
            .expect("a value follows the field")
    }

    // -----------------------------------------------------------------------
    // The child fixtures. Neither ever prints a secret's own bytes; see
    // this module's report, "the test child design".
    // -----------------------------------------------------------------------

    /// Reports, on one line, whether `credential_env_var_name("openai")` is
    /// present in this process's own environment and, if so, its length and
    /// a hash, never the value.
    ///
    /// `#[ignore]`d: this asserts nothing on its own (it is a fixture, not a
    /// criterion or property this crate owns), so an ordinary `cargo test`
    /// run must never count it as a passing test that verified something.
    /// It runs only when a parent test re-execs this same binary with
    /// `reexec_args`, which passes `--ignored --exact` for exactly this
    /// test's path.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0032_child_reporter() {
        let var = credential_env_var_name("openai");
        match std::env::var(&var) {
            Ok(value) => {
                let (len, hash) = hash_len(&value);
                println!("{CHILD_REPORT_PREFIX} present=1 len={len} hash={hash}");
            }
            Err(_) => println!("{CHILD_REPORT_PREFIX} present=0"),
        }
    }

    /// Sleeps long enough that a caller which failed to kill it would notice:
    /// used only by `tests::ori_t_0032_stop_actually_terminates_a_still_running_child`
    /// to prove `SpawnedSession::stop` really signals the process rather
    /// than waiting for it to finish on its own.
    ///
    /// `#[ignore]`d for the same reason as `tests::ori_t_0032_child_reporter`
    /// above, and doubly so here: left un-ignored, every ordinary `cargo
    /// test` run on every CI platform would pay this fixture's full 20
    /// second sleep for no assertion at all.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0032_child_long_running() {
        std::thread::sleep(Duration::from_secs(20));
    }

    // -----------------------------------------------------------------------
    // ORI-P1-037: the runtime half. Read this against the module doc
    // comment's "How the secret reaches the child" before changing it.
    // -----------------------------------------------------------------------

    /// Obviously fake: never a value that resembles a real provider key.
    const APP_SECRET: &str = "fake-application-openai-key-for-tests-AAA111";
    /// Obviously fake, and distinct from [`APP_SECRET`] so a test that
    /// accidentally compared the wrong pair could not pass by coincidence.
    const PROJECT_B_SECRET: &str = "fake-project-b-override-openai-key-for-tests-BBB222";

    #[test]
    fn ori_p1_037_a_and_b_each_receive_only_their_own_provider_key_and_neither_appears_in_any_event_log_or_the_childs_argv_or_environment()
     {
        let scratch = Scratch::new("provider-binding");
        let product_a = id("PRODUCT-A");
        let product_b = id("PRODUCT-B");

        let keychain = InMemoryKeychain::new();
        let app_key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        let b_key_ref = KeyRef::parse("ori/product/b/provider/openai").expect("a key ref");
        keychain
            .set(&app_key_ref, Secret::new(APP_SECRET))
            .expect("the application key is stored");
        keychain
            .set(&b_key_ref, Secret::new(PROJECT_B_SECRET))
            .expect("project B's override is stored");

        let bindings = vec![
            app_binding("openai", app_key_ref.as_str()),
            product_binding(product_b.clone(), "openai", b_key_ref.as_str()),
        ];

        let identity_a = coder_identity(product_a.clone(), id("IDENTITY-A"));
        let identity_b = coder_identity(product_b.clone(), id("IDENTITY-B"));
        let session_a = id("SESSION-A");
        let session_b = id("SESSION-B");

        let mut db_a = ProductDb::open(&scratch.path, product_a.as_str(), at(1_000))
            .expect("product A's database opens");
        let mut db_b = ProductDb::open(&scratch.path, product_b.as_str(), at(1_000))
            .expect("product B's database opens");

        // --- "spawn a coder in A and in B", for real: a real child process
        // per side, driven through Injector::spawn, never a hand-rolled
        // Command that bypasses it. ---
        let program = current_test_binary();
        let request_a = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity_a,
            session_id: session_a.clone(),
            issuance_id: id("ISSUANCE-A"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: program.clone().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
            capture_stdout: true,
        };
        let mut spawned_a =
            Injector::spawn(&keychain, &mut db_a, at(2_000), Actor::System, request_a)
                .expect("A spawns");

        let request_b = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity_b,
            session_id: session_b.clone(),
            issuance_id: id("ISSUANCE-B"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: program.into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
            capture_stdout: true,
        };
        let mut spawned_b =
            Injector::spawn(&keychain, &mut db_b, at(2_000), Actor::System, request_b)
                .expect("B spawns");

        // Reading each child's stdout to EOF blocks only until that child's
        // own process exits (the reporter prints one line and returns), and
        // it never inspects argv or the environment table itself, only what
        // the child chose to print, which is never the secret's own bytes.
        let mut stdout_a = String::new();
        spawned_a
            .take_stdout()
            .expect("A's stdout was captured")
            .read_to_string(&mut stdout_a)
            .expect("A's child's stdout reads back");
        let mut stdout_b = String::new();
        spawned_b
            .take_stdout()
            .expect("B's stdout was captured")
            .read_to_string(&mut stdout_b)
            .expect("B's child's stdout reads back");

        // --- "A receives the application key, B receives its override" ---
        let report_a = parse_report(&stdout_a).expect("A's child reports the variable present");
        let report_b = parse_report(&stdout_b).expect("B's child reports the variable present");
        assert_eq!(
            report_a,
            hash_len(APP_SECRET),
            "A's real child process must have received the application key, not something else"
        );
        assert_eq!(
            report_b,
            hash_len(PROJECT_B_SECRET),
            "B's real child process must have received its own override, not A's key"
        );
        assert_ne!(
            report_a, report_b,
            "A and B must not have received the same bytes"
        );

        // --- end both sessions: revoke first, then reap the (already
        // finished) children. ---
        let session_a_running = running_session(session_a.clone());
        let session_b_running = running_session(session_b.clone());
        let ended_a = Injector::end(
            spawned_a,
            session_a_running,
            &mut db_a,
            at(3_000),
            at(3_000),
            Actor::System,
        )
        .expect("A's session ends");
        let ended_b = Injector::end(
            spawned_b,
            session_b_running,
            &mut db_b,
            at(3_000),
            at(3_000),
            Actor::System,
        )
        .expect("B's session ends");
        assert_eq!(ended_a.receipt.revoked_count(), 1);
        assert_eq!(ended_b.receipt.revoked_count(), 1);

        // --- step 2: the produced set is non-empty and its exact size, read
        // from the store itself, not a counter this test kept (the vacuity
        // trap this module's own doc comment and ori_broker::keychain's both
        // name). One credential.issued and one credential.revoked event per
        // product. ---
        let report_events_a = EventLog::verify(db_a.connection()).expect("A's log verifies");
        let report_events_b = EventLog::verify(db_b.connection()).expect("B's log verifies");
        assert_eq!(
            report_events_a.events_checked, 2,
            "product A's log must hold exactly issue and revoke, no more, no fewer"
        );
        assert_eq!(
            report_events_b.events_checked, 2,
            "product B's log must hold exactly issue and revoke, no more, no fewer"
        );

        // --- step 3: scan every produced byte: the typed events, the raw
        // files on disk, and the built commands' own argv and env table. ---
        let events_a = EventLog::read_range(
            db_a.connection(),
            1,
            report_events_a.tip_seq.expect("A has a tip"),
        )
        .expect("A's events read back");
        let events_b = EventLog::read_range(
            db_b.connection(),
            1,
            report_events_b.tip_seq.expect("B has a tip"),
        )
        .expect("B's events read back");
        for event in events_a.iter().chain(events_b.iter()) {
            let payload = event.payload();
            assert!(
                !payload.contains(APP_SECRET),
                "the application key leaked into an event payload: {payload}"
            );
            assert!(
                !payload.contains(PROJECT_B_SECRET),
                "project B's key leaked into an event payload: {payload}"
            );
        }

        drop(db_a);
        drop(db_b);
        let on_disk = read_every_file_under(&scratch.path);
        assert!(
            !on_disk.is_empty(),
            "the durable log files must have actually been written to"
        );
        let haystack = String::from_utf8_lossy(&on_disk);
        assert!(
            !haystack.contains(APP_SECRET),
            "the application key leaked to disk"
        );
        assert!(
            !haystack.contains(PROJECT_B_SECRET),
            "project B's key leaked to disk"
        );

        // --- the argv this module actually built for each child, inspected
        // through Command's own stable getters rather than trusted from
        // prose. ---
        let env_var = credential_env_var_name("openai");
        let secret_app = Secret::new(APP_SECRET);
        let command_a = build_command(
            OsStr::new("irrelevant-for-this-check"),
            &reexec_args("injector::tests::ori_t_0032_child_reporter"),
            &env_var,
            &secret_app,
        );
        for arg in command_a.get_args() {
            assert!(
                arg.to_string_lossy() != APP_SECRET,
                "the secret must never appear as a command-line argument"
            );
        }
        let envs: Vec<_> = command_a.get_envs().collect();
        assert_eq!(
            envs.len(),
            1,
            "exactly one environment variable is added, nothing else"
        );
        assert_eq!(envs[0].0, OsStr::new(env_var.as_str()));
        assert_eq!(envs[0].1, Some(OsStr::new(APP_SECRET)));

        // --- the parent's own process environment was never touched. ---
        assert!(
            std::env::var(&env_var).is_err(),
            "the injector must never call std::env::set_var in its own process"
        );
    }

    fn read_every_file_under(root: &Path) -> Vec<u8> {
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
                    && let Ok(bytes) = fs::read(&path)
                {
                    out.extend(bytes);
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // ORI-P1-020: the runtime half, "every issuance for the session is
    // revoked with a timestamp before the process is terminated", exhaustive
    // over the real crate::session::Outcome, "any outcome" per this
    // ticket's own trap 4.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_020_every_outcome_revokes_before_the_process_is_recorded_stopped() {
        for outcome in Outcome::ALL {
            // Exhaustive on purpose, no `_` arm: adding a fifth Outcome
            // variant to crate::session without adding one here fails the
            // build, which is what this ticket's own trap 4 asks for.
            match outcome {
                Outcome::Completed | Outcome::Blocked | Outcome::Escalated | Outcome::Killed => {}
            }

            let scratch = Scratch::new(&format!("any-outcome-{}", outcome.as_str()));
            let product_id = id("PRODUCT-OUTCOME");
            let identity = coder_identity(product_id.clone(), id("IDENTITY-OUTCOME"));
            let session_id = id(&format!("SESSION-{}", outcome.as_str()));
            let keychain = InMemoryKeychain::new();
            let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
            keychain
                .set(&key_ref, Secret::new("fake-outcome-key-for-tests"))
                .expect("the key is stored");
            let bindings = vec![app_binding("openai", key_ref.as_str())];
            let mut db = open_db(&scratch, &product_id);

            let request = SpawnRequest {
                bindings: &bindings,
                provider: "openai",
                identity: &identity,
                session_id: session_id.clone(),
                issuance_id: id("ISSUANCE-OUTCOME"),
                scope: scope("provider:openai"),
                expires_at: None,
                program: current_test_binary().into_os_string(),
                args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
                capture_stdout: false,
            };
            let spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
                .unwrap_or_else(|err| panic!("spawn for outcome {}: {err}", outcome.as_str()));

            let session = running_session(session_id.clone());
            let ended = Injector::end(
                spawned,
                session,
                &mut db,
                at(3_000),
                at(3_100),
                Actor::System,
            )
            .unwrap_or_else(|err| panic!("end for outcome {}: {err}", outcome.as_str()));

            assert_eq!(
                ended.receipt.revoked_count(),
                1,
                "outcome {} must still revoke the session's issuance, not vacuously report zero",
                outcome.as_str()
            );
            assert_eq!(
                ended.session.teardown().at(StepKind::CredentialsRevoked),
                Some(at(3_000)),
                "outcome {}: credentials revoked at the receipt's own timestamp",
                outcome.as_str()
            );
            assert_eq!(
                ended.session.teardown().at(StepKind::ProcessStopped),
                Some(at(3_100)),
                "outcome {}: the process is recorded stopped only after",
                outcome.as_str()
            );

            let after = issuances_for_session(&mut db, &session_id).expect("issuances read back");
            assert_eq!(after.len(), 1);
            assert!(
                after[0].is_revoked(),
                "outcome {} left an unrevoked issuance: {:?}",
                outcome.as_str(),
                after[0]
            );

            // Finish the machine crate::session::Session defines, proving
            // this module's two steps are exactly what that machine expects
            // before locks and the worktree, which are out of this ticket's
            // scope, are recorded too.
            let finished = ended
                .session
                .record(TeardownStep::LocksReleased, at(3_200))
                .expect("locks released")
                .record(
                    TeardownStep::WorktreeReleased(Disposition::Retained),
                    at(3_300),
                )
                .expect("worktree released")
                .end(*outcome, at(3_400))
                .unwrap_or_else(|err| {
                    panic!("Session::end for outcome {}: {err}", outcome.as_str())
                });
            assert_eq!(finished.outcome(), Some(*outcome));
            assert!(
                finished.residue().is_empty(),
                "outcome {}: a session torn down in full leaves no residue: {:?}",
                outcome.as_str(),
                finished.residue()
            );
        }
    }

    // -----------------------------------------------------------------------
    // The vacuity trap at this module's own layer: a session with several
    // issuances accumulated over its life (not only the one Injector::spawn
    // itself issued) has every one of them revoked, not only the first.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_ending_a_session_revokes_every_issuance_it_ever_had_not_only_the_one_spawn_made()
    {
        let scratch = Scratch::new("multi-issuance");
        let product_id = id("PRODUCT-MULTI");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-MULTI"));
        let session_id = id("SESSION-MULTI");
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-multi-key-for-tests"))
            .expect("the key is stored");
        let bindings = vec![app_binding("openai", key_ref.as_str())];
        let mut db = open_db(&scratch, &product_id);

        // Two issuances recorded directly, as if this session had already
        // had a credential reissued once before the spawn this test drives.
        for label in ["EARLIER-A", "EARLIER-B"] {
            issue_credential(
                &mut db,
                at(1_500),
                Actor::System,
                &identity,
                NewIssuance {
                    id: id(label),
                    session_id: session_id.clone(),
                    scope: scope("provider:openai"),
                    expires_at: None,
                },
            )
            .expect("an earlier issuance is recorded");
        }

        let request = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity,
            session_id: session_id.clone(),
            issuance_id: id("ISSUANCE-SPAWNED"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
            capture_stdout: false,
        };
        let spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect("spawn succeeds");

        let session = running_session(session_id.clone());
        let ended = Injector::end(
            spawned,
            session,
            &mut db,
            at(3_000),
            at(3_100),
            Actor::System,
        )
        .expect("end succeeds");

        assert_eq!(
            ended.receipt.revoked_count(),
            3,
            "all three issuances (two earlier, one from this spawn) must be revoked, not a \
             vacuous count from only the first found"
        );
    }

    // -----------------------------------------------------------------------
    // SpawnedSession::stop refuses a receipt for a different session.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_stop_refuses_a_receipt_issued_for_a_different_session() {
        let scratch = Scratch::new("mismatch");
        let product_id = id("PRODUCT-MISMATCH");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-MISMATCH"));
        let session_id = id("SESSION-MISMATCH");
        let other_session_id = id("SESSION-OTHER");
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-mismatch-key-for-tests"))
            .expect("the key is stored");
        let bindings = vec![app_binding("openai", key_ref.as_str())];
        let mut db = open_db(&scratch, &product_id);

        let request = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity,
            session_id: session_id.clone(),
            issuance_id: id("ISSUANCE-MISMATCH"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
            capture_stdout: false,
        };
        let spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect("spawn succeeds");

        // A receipt for a session this SpawnedSession does not belong to:
        // revoking one that was never issued anything, which still yields a
        // genuine (if vacuous-count) RevocationReceipt, the only way one can
        // be constructed at all.
        let foreign_receipt = revoke_session(&mut db, at(2_500), Actor::System, &other_session_id)
            .expect("revoking an unrelated session still returns a receipt");

        let refusal = spawned
            .stop(&foreign_receipt)
            .expect_err("a receipt for another session must not stop this one");
        assert!(matches!(
            refusal,
            InjectorError::ReceiptSessionMismatch { .. }
        ));
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(27)
        );
    }

    // -----------------------------------------------------------------------
    // No binding resolves: reported, not a panic.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_spawning_with_no_binding_configured_is_reported_not_a_panic() {
        let scratch = Scratch::new("no-binding");
        let product_id = id("PRODUCT-NO-BINDING");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-NO-BINDING"));
        let keychain = InMemoryKeychain::new();
        let mut db = open_db(&scratch, &product_id);

        let request = SpawnRequest {
            bindings: &[],
            provider: "openai",
            identity: &identity,
            session_id: id("SESSION-NO-BINDING"),
            issuance_id: id("ISSUANCE-NO-BINDING"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
            capture_stdout: false,
        };
        let refusal = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect_err("no binding exists to resolve");
        assert!(matches!(refusal, InjectorError::NoBinding { .. }));
        assert_eq!(refusal.methodology_ref(), None);
    }

    // -----------------------------------------------------------------------
    // The environment variable name.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_the_env_var_name_follows_env_setup_mds_provider_name_api_key_convention() {
        assert_eq!(credential_env_var_name("openai"), "PROVIDER_OPENAI_API_KEY");
        assert_eq!(
            credential_env_var_name("anthropic"),
            "PROVIDER_ANTHROPIC_API_KEY"
        );
    }

    // -----------------------------------------------------------------------
    // build_command: the secret is only ever in the env table, under the
    // right name, and never in argv. This is what plant 2 and plant 3 must
    // break.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_the_secret_never_appears_in_the_built_commands_argv() {
        let secret = Secret::new("fake-argv-check-secret-for-tests");
        let args = vec![OsString::from("--some-flag"), OsString::from("some-value")];
        let command = build_command(
            OsStr::new("irrelevant-program"),
            &args,
            "PROVIDER_OPENAI_API_KEY",
            &secret,
        );
        for arg in command.get_args() {
            secret.expose(|plaintext| {
                assert_ne!(
                    arg.to_string_lossy(),
                    plaintext,
                    "the secret must never be passed as a command-line argument"
                );
            });
        }
        // The caller's own, unrelated args are untouched.
        let built_args: Vec<_> = command.get_args().collect();
        assert_eq!(
            built_args,
            vec![OsStr::new("--some-flag"), OsStr::new("some-value")]
        );
    }

    #[test]
    fn ori_t_0032_the_secret_never_appears_in_the_parents_own_environment() {
        let env_var = credential_env_var_name("openai");
        assert!(
            std::env::var(&env_var).is_err(),
            "nothing in this test process should have set this variable before build_command runs"
        );
        let secret = Secret::new("fake-parent-env-check-secret-for-tests");
        let _command = build_command(OsStr::new("irrelevant-program"), &[], &env_var, &secret);
        assert!(
            std::env::var(&env_var).is_err(),
            "build_command must place the secret on the Command value alone, via \
             Command::env, never through std::env::set_var in this process"
        );
    }

    // -----------------------------------------------------------------------
    // Stop genuinely terminates a still-running child rather than waiting
    // for it to finish on its own: without this, a defect that recorded
    // ProcessStopped without ever calling Child::kill would pass every
    // assertion above by coincidence (the reporter child exits almost
    // immediately on its own regardless).
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_stop_actually_terminates_a_still_running_child_rather_than_waiting_for_it_to_exit_on_its_own()
     {
        let scratch = Scratch::new("long-running");
        let product_id = id("PRODUCT-LONGRUN");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-LONGRUN"));
        let session_id = id("SESSION-LONGRUN");
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-longrun-key-for-tests"))
            .expect("the key is stored");
        let bindings = vec![app_binding("openai", key_ref.as_str())];
        let mut db = open_db(&scratch, &product_id);

        let request = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity,
            session_id: session_id.clone(),
            issuance_id: id("ISSUANCE-LONGRUN"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_long_running"),
            capture_stdout: false,
        };
        let spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect("spawn succeeds");

        let session = running_session(session_id);
        let started = Instant::now();
        let ended = Injector::end(
            spawned,
            session,
            &mut db,
            at(3_000),
            at(3_100),
            Actor::System,
        )
        .expect("end succeeds");
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(10),
            "ending the session must actually kill the still-sleeping child rather than wait \
             for its 20 second sleep to finish on its own; took {elapsed:?}"
        );
        assert!(
            !ended.status.success(),
            "a killed process does not report a successful exit"
        );
    }

    // -----------------------------------------------------------------------
    // A structural guard against a second kill path: no runtime assertion
    // above can tell "revoke then kill, both fast" apart from "kill then
    // revoke, both fast" by timing alone, both take microseconds regardless
    // of order, and `crate::session::Teardown::record` checks the
    // timestamps a caller supplies, not when the operating system call
    // actually ran; a rewrite of `Injector::end` that physically kills
    // first and then writes the same, correct-looking timestamps afterward
    // would pass every assertion above. What such a rewrite cannot do
    // without being visible here is add a second place in this file that
    // reaches `Child::kill`: `SpawnedSession::stop` is the only one, its
    // signature requires a `RevocationReceipt`, and this test reads this
    // file's own source, at build time, to prove that stays true.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0032_exactly_one_place_in_this_file_ever_calls_child_kill() {
        let source = include_str!("injector.rs");
        // Built from two pieces rather than written as one string literal,
        // so the search text does not match its own occurrence on this
        // line, which would otherwise count as a second hit.
        let needle = [".ki", "ll()"].concat();
        let occurrences = source.matches(needle.as_str()).count();
        assert_eq!(
            occurrences, 1,
            "SpawnedSession::stop must be the only function in this file that ever calls \
             Child::kill; a second call site is a bypass around the RevocationReceipt this \
             module's own doc comment says makes killing before revoking impossible to reach"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0111: an injected child never inherits the engine's stdio. Each
    // stream below gets a wrapper-based test in the shape ORI-T-0033's own
    // `crate::headless` tests use for stdin: a re-exec'd wrapper process
    // whose own stdio this test controls directly, so "did the grandchild's
    // output land in the wrapper's own stream" is observed over real file
    // descriptors, never inferred from `Command`'s own getters (which say
    // nothing about a stream `Command` was never told to change from its
    // default; see this module's own doc comment).
    // -----------------------------------------------------------------------

    /// Polls `spawned`'s child for exit without ever calling `Child::kill`
    /// itself, bounded by `timeout`. Used to observe, before this module's
    /// own single kill path ([`Injector::end`]) tears the child down either
    /// way, whether a grandchild exited on its own (its stdin genuinely
    /// closed, so a blocking read returns at once) or is still running (its
    /// stdin wrongly inherited a pipe a test holds open and never writes
    /// to, so the read blocks).
    ///
    /// Reaches [`SpawnedSession`]'s private `child` field directly: `tests`
    /// is a descendant of the module that field is private to, so this
    /// compiles without adding any method to [`SpawnedSession`] itself,
    /// public or otherwise, and without ever touching `Child::kill`, so it
    /// is not a second call site
    /// `tests::ori_t_0032_exactly_one_place_in_this_file_ever_calls_child_kill`
    /// polices.
    fn exited_within(spawned: &mut SpawnedSession, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if matches!(spawned.child.try_wait(), Ok(Some(_))) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // -----------------------------------------------------------------------
    // Stream 1: stdin.
    // -----------------------------------------------------------------------

    /// The exact line `tests::ori_t_0111_child_stdin_reporter` prints when
    /// its own blocking read of one line from stdin returned end-of-file at
    /// once. Never printed if it actually read data.
    const STDIN_REPORT_EOF: &str = "ORI_T_0111_STDIN_REPORT eof";
    const STDIN_WRAPPER_MODE_VAR: &str = "ORI_T_0111_STDIN_WRAPPER";
    const STDIN_WRAPPER_TEST_PATH: &str =
        "injector::tests::ori_t_0111_stdin_wrapper_entrypoint_do_not_call_directly";

    /// Reports, on one line, whether a blocking read of one line from this
    /// process's own stdin returned end-of-file at once or actually read
    /// data. Never prints what it read, only whether it read anything: this
    /// fixture is a probe of the stream's own state, not of any content on
    /// it.
    ///
    /// `#[ignore]`d for the same reason as `tests::ori_t_0032_child_reporter`:
    /// a normal `cargo test` run must never execute this fixture directly,
    /// only through a parent test's re-exec.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0111_child_stdin_reporter() {
        let mut line = String::new();
        let read = std::io::stdin().lock().read_line(&mut line).unwrap_or(0);
        if read == 0 {
            println!("{STDIN_REPORT_EOF}");
        } else {
            println!("ORI_T_0111_STDIN_REPORT data bytes={read}");
        }
    }

    /// Spawns `tests::ori_t_0111_child_stdin_reporter` through the real
    /// [`Injector::spawn`] and reports, on its own stdout, whether that
    /// grandchild saw its stdin closed. Gated behind
    /// [`STDIN_WRAPPER_MODE_VAR`] the same way
    /// `crate::headless`'s own inherit-wrapper fixture is gated in that
    /// module, so a broad `cargo test -- --ignored` run never executes this
    /// heavier, nested-spawn logic outside a deliberate re-exec.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0111_stdin_wrapper_entrypoint_do_not_call_directly() {
        if std::env::var(STDIN_WRAPPER_MODE_VAR).is_err() {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        }
        let scratch = Scratch::new("stdin-wrapper");
        let product_id = id("PRODUCT-STDINWRAP");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-STDINWRAP"));
        let session_id = id("SESSION-STDINWRAP");
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-stdinwrap-key-for-tests"))
            .expect("the key is stored");
        let bindings = vec![app_binding("openai", key_ref.as_str())];
        let mut db = open_db(&scratch, &product_id);
        let request = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity,
            session_id: session_id.clone(),
            issuance_id: id("ISSUANCE-STDINWRAP"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0111_child_stdin_reporter"),
            capture_stdout: true,
        };
        let mut spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect("the grandchild spawns");

        // Bounded, kill-free: did it exit on its own (stdin genuinely
        // closed) within a generous window, or is it still running (stdin
        // wrongly inherited this wrapper's own, held-open, never-written
        // pipe, so its read blocks)?
        let exited_on_its_own = exited_within(&mut spawned, Duration::from_secs(5));
        let report = if exited_on_its_own {
            let mut stdout = String::new();
            if let Some(mut out) = spawned.take_stdout() {
                let _ = out.read_to_string(&mut stdout);
            }
            if stdout.contains(STDIN_REPORT_EOF) {
                "eof"
            } else {
                "other"
            }
        } else {
            "blocked"
        };

        let session = running_session(session_id);
        let _ended = Injector::end(
            spawned,
            session,
            &mut db,
            at(3_000),
            at(3_100),
            Actor::System,
        )
        .expect("teardown always succeeds, whether the child exited or had to be killed");

        println!("WRAPPER_RESULT:{report}");
    }

    /// ORI-T-0111: a child [`Injector::spawn`] starts never inherits this
    /// process's own stdin, whatever `request.capture_stdout` is set to.
    ///
    /// Proved independently of the machine's own ambient stdin, the same
    /// way `crate::headless`'s own equivalent test does for
    /// `HeadlessAdapter::spawn`: this test re-execs this binary as a
    /// wrapper process and pins *that* wrapper's own stdin to a pipe it
    /// opens and never writes to or closes. The wrapper then calls the real
    /// [`Injector::spawn`], exactly as
    /// `tests::ori_t_0111_stdin_wrapper_entrypoint_do_not_call_directly`
    /// does, and reports what the grandchild two levels down actually saw.
    /// If [`Injector::spawn`] ever gave that grandchild anything other than
    /// a genuinely null stdin, the only stdin available to inherit is the
    /// wrapper's own, which is this held-open pipe, so the grandchild would
    /// block reading it and the wrapper's own bounded check
    /// (`exited_within`) would report `"blocked"` rather than hang forever,
    /// because [`Injector::end`] tears the grandchild down either way.
    /// Whether this test's own ambient stdin happens to be a terminal,
    /// closed, or already at end-of-file makes no difference: the pipe held
    /// below is this test's own.
    #[test]
    fn ori_t_0111_an_injected_child_never_inherits_this_processs_own_stdin() {
        let exe = current_test_binary();
        let mut command = Command::new(&exe);
        command
            .args(reexec_args(STDIN_WRAPPER_TEST_PATH))
            .env(STDIN_WRAPPER_MODE_VAR, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("the wrapper binary spawns");
        // Held open deliberately: never written to, never closed, for the
        // life of `held_stdin`. See this test's own doc comment.
        let held_stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("stdout is piped");

        let status = wait_with_timeout(&mut child, Duration::from_secs(30))
            .expect("the wrapper always exits, win or lose, within its own bounded wait");
        drop(held_stdin);

        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        assert!(
            status.success(),
            "the wrapper itself must not panic: {output}"
        );
        assert!(
            output.contains("WRAPPER_RESULT:eof"),
            "the grandchild must see end-of-file on its own stdin, never this wrapper's own, \
             held-open pipe: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // Stream 2: stdout when capture_stdout is false. Reuses
    // tests::ori_t_0032_child_reporter as the grandchild fixture: it
    // already prints one line and exits without touching stdin, exactly
    // what this stream's own probe needs.
    // -----------------------------------------------------------------------

    const STDOUT_WRAPPER_MODE_VAR: &str = "ORI_T_0111_STDOUT_WRAPPER";
    const STDOUT_WRAPPER_TEST_PATH: &str =
        "injector::tests::ori_t_0111_stdout_wrapper_entrypoint_do_not_call_directly";
    const STDOUT_WRAPPER_DONE: &str = "ORI_T_0111_STDOUT_WRAPPER_DONE";

    /// Spawns `tests::ori_t_0032_child_reporter` through the real
    /// [`Injector::spawn`] with `capture_stdout: false`, the exact flag
    /// value this ticket's own defect report names
    /// (`tests::ori_p1_020_every_outcome_revokes_before_the_process_is_recorded_stopped`
    /// already spawns with this same flag, silently, on every run before
    /// this ticket's fix). Reports its own completion on its own stdout,
    /// after the grandchild has already run and exited.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0111_stdout_wrapper_entrypoint_do_not_call_directly() {
        if std::env::var(STDOUT_WRAPPER_MODE_VAR).is_err() {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        }
        let scratch = Scratch::new("stdout-wrapper");
        let product_id = id("PRODUCT-STDOUTWRAP");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-STDOUTWRAP"));
        let session_id = id("SESSION-STDOUTWRAP");
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-stdoutwrap-key-for-tests"))
            .expect("the key is stored");
        let bindings = vec![app_binding("openai", key_ref.as_str())];
        let mut db = open_db(&scratch, &product_id);
        let request = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity,
            session_id: session_id.clone(),
            issuance_id: id("ISSUANCE-STDOUTWRAP"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0032_child_reporter"),
            capture_stdout: false,
        };
        let mut spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect("the grandchild spawns");

        // The reporter fixture never touches stdin and never sleeps, so it
        // always exits almost at once; this bound only guards against the
        // OS itself being slow to schedule it.
        let _ = exited_within(&mut spawned, Duration::from_secs(5));

        let session = running_session(session_id);
        let _ended = Injector::end(
            spawned,
            session,
            &mut db,
            at(3_000),
            at(3_100),
            Actor::System,
        )
        .expect("teardown always succeeds");

        println!("{STDOUT_WRAPPER_DONE}");
    }

    /// ORI-T-0111: a child spawned with `capture_stdout: false` never
    /// inherits this process's own stdout either, the defect this ticket
    /// fixes (`Injector::spawn` used to call `command.stdout(Stdio::piped())`
    /// only when `capture_stdout` was true, leaving the default, inherited,
    /// otherwise).
    ///
    /// The wrapper's own stdout is piped by this test, never inherited from
    /// this test's own process: if `Injector::spawn` ever let the
    /// grandchild inherit *its* stdout, the grandchild's own report would
    /// land directly in that same pipe, interleaved with the wrapper's own
    /// completion line. `Command::get_args`/`get_envs`-style inspection
    /// (`tests::ori_t_0032_the_secret_never_appears_in_the_built_commands_argv`'s
    /// own style) cannot observe this: a `Command` never told to change a
    /// stream from its default reports nothing about it either way, which
    /// is exactly how this defect went unnoticed.
    #[test]
    fn ori_t_0111_a_child_spawned_with_capture_stdout_false_never_inherits_this_processs_own_stdout()
     {
        let exe = current_test_binary();
        let mut command = Command::new(&exe);
        command
            .args(reexec_args(STDOUT_WRAPPER_TEST_PATH))
            .env(STDOUT_WRAPPER_MODE_VAR, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("the wrapper binary spawns");
        let mut stdout = child.stdout.take().expect("stdout is piped");

        let status = wait_with_timeout(&mut child, Duration::from_secs(30))
            .expect("the wrapper always exits within its own bounded wait");

        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        assert!(
            status.success(),
            "the wrapper itself must not panic: {output}"
        );
        assert!(
            output.contains(STDOUT_WRAPPER_DONE),
            "the wrapper must complete its own sequence: {output}"
        );
        assert!(
            !output.contains(CHILD_REPORT_PREFIX),
            "a child spawned with capture_stdout: false must never write into this wrapper's \
             own stdout: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // Stream 3: stderr, never inherited and never piped either (this module
    // reads no piped stderr anywhere, so `null` is the only choice that
    // cannot deadlock a child on a full, unread pipe).
    // -----------------------------------------------------------------------

    const STDERR_REPORT_MARKER: &str = "ORI_T_0111_STDERR_REPORT present";
    const STDERR_WRAPPER_MODE_VAR: &str = "ORI_T_0111_STDERR_WRAPPER";
    const STDERR_WRAPPER_TEST_PATH: &str =
        "injector::tests::ori_t_0111_stderr_wrapper_entrypoint_do_not_call_directly";
    const STDERR_WRAPPER_DONE: &str = "ORI_T_0111_STDERR_WRAPPER_DONE";

    /// Writes one line to this process's own stderr, never stdout, and
    /// exits. `#[ignore]`d for the same reason as every other fixture in
    /// this module: only ever run by a parent test's re-exec.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0111_child_stderr_reporter() {
        eprintln!("{STDERR_REPORT_MARKER}");
    }

    /// Spawns `tests::ori_t_0111_child_stderr_reporter` through the real
    /// [`Injector::spawn`] and reports its own completion on its own
    /// stderr, never its own stdout, so the outer test can pin exactly the
    /// stream under test.
    #[test]
    #[ignore = "child-process fixture: run only by re-exec from its parent test"]
    fn ori_t_0111_stderr_wrapper_entrypoint_do_not_call_directly() {
        if std::env::var(STDERR_WRAPPER_MODE_VAR).is_err() {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        }
        let scratch = Scratch::new("stderr-wrapper");
        let product_id = id("PRODUCT-STDERRWRAP");
        let identity = coder_identity(product_id.clone(), id("IDENTITY-STDERRWRAP"));
        let session_id = id("SESSION-STDERRWRAP");
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-stderrwrap-key-for-tests"))
            .expect("the key is stored");
        let bindings = vec![app_binding("openai", key_ref.as_str())];
        let mut db = open_db(&scratch, &product_id);
        let request = SpawnRequest {
            bindings: &bindings,
            provider: "openai",
            identity: &identity,
            session_id: session_id.clone(),
            issuance_id: id("ISSUANCE-STDERRWRAP"),
            scope: scope("provider:openai"),
            expires_at: None,
            program: current_test_binary().into_os_string(),
            args: reexec_args("injector::tests::ori_t_0111_child_stderr_reporter"),
            capture_stdout: false,
        };
        let mut spawned = Injector::spawn(&keychain, &mut db, at(2_000), Actor::System, request)
            .expect("the grandchild spawns");

        let _ = exited_within(&mut spawned, Duration::from_secs(5));

        let session = running_session(session_id);
        let _ended = Injector::end(
            spawned,
            session,
            &mut db,
            at(3_000),
            at(3_100),
            Actor::System,
        )
        .expect("teardown always succeeds");

        eprintln!("{STDERR_WRAPPER_DONE}");
    }

    /// ORI-T-0111: an injected child never inherits this process's own
    /// stderr either. Same wrapper shape as the stdout test above, pinned
    /// to the wrapper's own stderr pipe instead of its stdout, with the
    /// wrapper itself reporting completion on stderr too so a plant that
    /// inherited stderr and a wrapper that simply never ran are both
    /// distinguishable from the pass case.
    #[test]
    fn ori_t_0111_a_child_never_inherits_this_processs_own_stderr_either() {
        let exe = current_test_binary();
        let mut command = Command::new(&exe);
        command
            .args(reexec_args(STDERR_WRAPPER_TEST_PATH))
            .env(STDERR_WRAPPER_MODE_VAR, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("the wrapper binary spawns");
        let mut stderr = child.stderr.take().expect("stderr is piped");

        let status = wait_with_timeout(&mut child, Duration::from_secs(30))
            .expect("the wrapper always exits within its own bounded wait");

        let mut output = String::new();
        let _ = stderr.read_to_string(&mut output);
        assert!(
            status.success(),
            "the wrapper itself must not panic: {output}"
        );
        assert!(
            output.contains(STDERR_WRAPPER_DONE),
            "the wrapper must complete its own sequence: {output}"
        );
        assert!(
            !output.contains(STDERR_REPORT_MARKER),
            "an injected child must never write into this wrapper's own stderr: {output}"
        );
    }
}
