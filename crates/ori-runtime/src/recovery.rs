//! Engine restart recovery: AICD §12, `spec/runbooks/recover-engine.md`.
//!
//! The runbook's trigger: "the engine starts and finds sessions in Running
//! state from a previous process." Its step 1, verbatim: "For each Running
//! session: revoke every credential issuance (record `credential.revoked`),
//! terminate any process matching the session (worktree path, container id),
//! release its lock entries, set the session Killed with reason
//! `engine_restart`, return its ticket to Queued, append
//! `session.recovered`." This module drives that step, over
//! `crate::session::Session`, which `crates/ori-runtime/src/session.rs` builds
//! and whose own doc comment names this ticket as the one that drives it:
//! "The restart that drives them is ORI-T-0034."
//!
//! # Clause by clause, ORI-P1-031, and who owes what
//!
//! "Engine killed while a coder session is Running | Engine restart | Session
//! marked Killed, credentials revoked, locks released, ticket Queued, event
//! `session.recovered`; no orphan process."
//!
//! - **Session marked Killed**: this crate, `Session::end` with
//!   [`crate::session::Outcome::Killed`]. [`recover_session`] is what calls it,
//!   after every step the outcome requires is recorded.
//! - **Credentials revoked**: `ori-broker`'s `Issuance`. [`RestartActions::revoke_credentials`]
//!   is the seam; this module records the step through
//!   `Session::record` once the trait call reports it done, and refuses to
//!   proceed if it does not.
//! - **Locks released**: `ori-orchestrator`'s `LockTable`.
//!   [`RestartActions::release_locks`] is the seam, the same shape.
//! - **Ticket Queued**: `ori-core`'s `Ticket::apply`. [`RestartActions::requeue_ticket`]
//!   is the seam. See "A second gap, found in the course of this one" below:
//!   this clause has a real problem this module did not invent and cannot fix.
//! - **Event `session.recovered`**: `ori-store`'s `EventLog`.
//!   [`RestartActions::append_recovered_event`] is the seam.
//! - **No orphan process**: split two ways. The worktree half is answered by
//!   `Session::left_an_orphan`, which [`recover_session`] checks before it will
//!   report success, and which does ask the filesystem (see
//!   `crates/ori-runtime/src/session.rs`'s own note on what that guarantees).
//!   The process half is [`sweep`]; see "What recovery can honestly do about an
//!   orphan process" below.
//!
//! Why a trait ([`RestartActions`]) rather than calling `ori-broker`,
//! `ori-orchestrator` or `ori-store` directly: `spec/LLD.md` section 2's
//! dependency graph draws `ori-runtime -> ori-broker` and `ori-runtime ->
//! ori-store`, so both are reachable, but both crates are still the skeletons
//! `ecbb71c` left them as (`Identity`, `Issuance`, `EventLog` and the rest are
//! not written yet), so there is nothing concrete here to call. The trait is
//! also the right shape once they exist: `spec/LLD.md` section 2 gives
//! `ori-orchestrator` the lock table and the ticket transition is `ori-core`'s
//! pure function, and this crate does not depend on `ori-orchestrator` at all
//! (the graph draws that edge the other way, `ORC --> RT`) and reaching
//! `ori-orchestrator`'s decision to call `Ticket::apply` from here would be
//! exactly the sideways dependency `spec/LLD.md` section 2 forbids. Whoever
//! wires a real restart (a future ticket, in `ori-engine` or
//! `ori-orchestrator`, both of which are downstream of every crate this needs)
//! implements [`RestartActions`] against the real crates; this module only
//! fixes the order and refuses to claim success early.
//!
//! # There is no pid in the data model, and what recovery can honestly do
//! about an orphan process
//!
//! `spec/DATA_MODEL.md` section 2's `AgentSession` row is `id, identity_id,
//! ticket_id, worktree, container_id, started_at, ended_at, budget_used,
//! outcome`. No process identifier. `crates/ori-runtime/src/session.rs`
//! explains why that is not an oversight to work around: a pid field here
//! would be one no projection in `ori-store` could rebuild, since nothing
//! durable stores it. `spec/runbooks/recover-engine.md` step 1 matches a stray
//! process by "(worktree path, container id)" for exactly that reason, and
//! [`ProcessMark`] and [`sweep`] are that match applied to this crate's own
//! records.
//!
//! What that buys, honestly: [`sweep`] can tell a caller which live marks
//! correspond to no session this restart knows about (a genuine, unattributed
//! orphan, worth an incident) and which correspond to a session recovery
//! processed. What it cannot buy: certainty that a mark's process is actually
//! gone. That is exactly the boundary `Session::end` already draws for the
//! [`crate::session::TeardownStep::ProcessStopped`] step, which this module
//! inherits rather than widens: [`RestartActions::confirm_process_stopped`] is
//! an attestation the caller makes (by however it verifies a process against a
//! worktree path and a container id, which is OS-specific work this crate does
//! not do and would need a new dependency to do generically), and
//! [`recover_session`] trusts it the same way `Session::record` trusts every
//! other step. Inventing a pid field, or a claim of certainty this module
//! cannot back, would both be worse than the honest, narrower guarantee: this
//! module knows the caller said the process stopped, and it knows what marks
//! are unaccounted for among the ones it was shown.
//!
//! # The gap `crates/ori-runtime/src/session.rs` names, and what this module
//! does about it
//!
//! `spec/DATA_MODEL.md` section 3 draws exactly one edge out of `Spawning`, to
//! `Running`. An engine that dies mid-spawn leaves a session neither that
//! machine nor ORI-P1-031's precondition ("while a coder session is Running")
//! covers. The internal classifier does not fold a `Spawning` session found at restart
//! into the `Running` bucket, and it does not silently drop it either:
//! [`recover_all`] reports it as [`Ineligible::Spawning`], present in the
//! outcome by name, so a caller can decide (most plausibly: open an incident,
//! since nothing in the specified machine says what state such a session
//! should end in). Adding the edge is a specification change and this ticket's
//! declared scope is three files in `crates/ori-runtime`, none of them
//! `spec/DATA_MODEL.md`.
//!
//! # A second gap, found in the course of this one
//!
//! ORI-P1-031 asks for "ticket Queued" directly from a session that was
//! `Running`, which means the ticket was `InProgress`
//! (`spec/DATA_MODEL.md` section 3's Ticket diagram: `Queued --> InProgress`,
//! nothing else enters it). That diagram draws exactly one edge into
//! `Queued`: `Validated --> Queued`, plus `Blocked --> Queued: re-planned`.
//! There is no `InProgress --> Queued` edge. `crates/ori-core/src/ticket.rs`
//! transcribes the diagram "edge for edge" into its transition table
//! (`TRANSITIONS`, checked in that file), so `Ticket::apply` as specified
//! refuses the exact move ORI-P1-031's fifth clause asks for.
//!
//! This is not this ticket's gap to close: `crates/ori-core/src/ticket.rs` is
//! not in this ticket's declared scope, and routing around it here (for
//! instance, by having [`RestartActions::requeue_ticket`]'s real
//! implementation walk the ticket through `Blocked` first) would misrepresent
//! the ticket's own history — it was not blocked on a budget, it was killed by
//! a restart — for the sake of making a table agree with itself. What this
//! module does instead is the same thing it does for the `Spawning` gap: it
//! does not paper over the refusal. [`RestartActions::requeue_ticket`] returns
//! a `Result`; if a real implementation calls `Ticket::apply` and the
//! specified machine refuses it, that `Err` propagates out of
//! [`recover_session`] as [`RecoveryError::ActionFailed`], and the session is
//! **not** reported recovered. A silent success here would be the "present but
//! reporting nothing" defect (AICD §39) applied to a specification gap instead
//! of to a test; refusing loudly is the same policy CLAUDE.md rule 10 states
//! for a different kind of ticket, applied here to a claim rather than to an
//! action: prove it, or say plainly that it cannot be proven yet.
//!
//! # The cadence this module does not run
//!
//! `crates/ori-runtime/src/budget.rs`'s module doc covers the budget meter's
//! half of this question; the restart trigger this module answers is
//! event-driven ("the engine starts") rather than periodic, so no polling
//! cadence applies to [`recover_all`] itself. It runs once per restart, over
//! whatever [`RestartActions::revoke_credentials`] and its siblings were able
//! to confirm at that moment.
//!
//! Must not: call an adapter directly except through [`RestartActions`], merge,
//! or write `spec/` or `ops/` (CLAUDE.md rule 5, `spec/LLD.md` section 2).

use core::fmt;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

use ori_core::error::MethodologyRef;
use ori_core::types::Id;
use ori_core::types::Timestamp;

use crate::session::Disposition;
use crate::session::Held;
use crate::session::Outcome;
use crate::session::Session;
use crate::session::SessionError;
use crate::session::SessionState;
use crate::session::StepKind;
use crate::session::TeardownStep;

/// One step of `spec/runbooks/recover-engine.md` step 1, without its data:
/// AICD §12.
///
/// The first four are `crate::session::StepKind`'s four, in the order
/// `spec/LLD.md` section 5 fixes; [`RecoveryStep::reason`] calls that enum's
/// own `reason()` for them rather than restating the section numbers, so the
/// two files cannot drift on what a shared step means. `TicketRequeued` and
/// `EventAppended` are the runbook's two remaining actions, which
/// `crate::session::Session` has no notion of because they are not things a
/// session holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryStep {
    /// `ori-broker` revoked every issuance bound to this session.
    CredentialsRevoked,
    /// The agent process is confirmed no longer running.
    ProcessStopped,
    /// `ori-orchestrator` released the lock entries the ticket claimed.
    LocksReleased,
    /// The worktree was removed or deliberately retained.
    WorktreeDisposed,
    /// `ori-core`'s `Ticket::apply` returned the ticket to `Queued`.
    TicketRequeued,
    /// `ori-store` appended `session.recovered`.
    EventAppended,
}

impl RecoveryStep {
    /// The methodology section a refusal about this step rests on.
    ///
    /// `TicketRequeued` cites AICD §11, "Ticket lifecycle and categories",
    /// which is the section `crates/ori-core/src/ticket.rs`'s own module
    /// comment cites for the machine this step drives. `EventAppended` cites
    /// AICD §8, "Memory architecture", whose layer 3 the glossary defines as
    /// "The append-only log of tickets, incidents, decisions and attempts",
    /// which `session.recovered` is one entry of; `ori-store`'s own module
    /// comment cites the same section for the log this appends to.
    #[must_use]
    pub const fn reason(self) -> MethodologyRef {
        match self {
            Self::CredentialsRevoked => StepKind::CredentialsRevoked.reason(),
            Self::ProcessStopped => StepKind::ProcessStopped.reason(),
            Self::LocksReleased => StepKind::LocksReleased.reason(),
            Self::WorktreeDisposed => StepKind::WorktreeReleased.reason(),
            Self::TicketRequeued => MethodologyRef {
                section: 11,
                subsection: None,
            },
            Self::EventAppended => MethodologyRef {
                section: 8,
                subsection: None,
            },
        }
    }

    /// The step in words, for an error message.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CredentialsRevoked => "credentials revoked",
            Self::ProcessStopped => "process confirmed stopped",
            Self::LocksReleased => "locks released",
            Self::WorktreeDisposed => "worktree disposed",
            Self::TicketRequeued => "ticket requeued",
            Self::EventAppended => "session.recovered appended",
        }
    }
}

impl fmt::Display for RecoveryStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a caller must do to drive one session through
/// `spec/runbooks/recover-engine.md` step 1: AICD §12.
///
/// Six methods, one per action the runbook names, called by [`recover_session`]
/// in the order `spec/LLD.md` section 5 fixes for the first four and the
/// runbook's own order for the last two. Each returns a
/// [`RecoveryError`] so a real implementation can report exactly what it
/// tried against the crate that owns it (`ori-broker`, `ori-orchestrator`,
/// `ori-core`, `ori-store`); this module never constructs the success value of
/// an action it did not perform.
pub trait RestartActions {
    /// `ori-broker` revokes every issuance bound to this session.
    fn revoke_credentials(&mut self, session: &Session, at: Timestamp)
    -> Result<(), RecoveryError>;

    /// The caller confirms the session's process is no longer running,
    /// matched by (worktree path, container id) per the runbook. See the
    /// module doc's note on what this module can and cannot verify itself.
    fn confirm_process_stopped(
        &mut self,
        session: &Session,
        at: Timestamp,
    ) -> Result<(), RecoveryError>;

    /// `ori-orchestrator`'s lock table releases the session's lock entries.
    fn release_locks(&mut self, session: &Session, at: Timestamp) -> Result<(), RecoveryError>;

    /// The worktree is removed or deliberately retained, and the caller says
    /// which.
    fn dispose_worktree(
        &mut self,
        session: &Session,
        at: Timestamp,
    ) -> Result<Disposition, RecoveryError>;

    /// `ori-core`'s `Ticket::apply` returns the ticket to `Queued`. See the
    /// module doc's "second gap" for what this can honestly promise today.
    fn requeue_ticket(&mut self, session: &Session, at: Timestamp) -> Result<(), RecoveryError>;

    /// `ori-store` appends `session.recovered`.
    fn append_recovered_event(
        &mut self,
        session: &Session,
        at: Timestamp,
    ) -> Result<(), RecoveryError>;
}

/// Drives one `Running` session through the six actions, in order, refusing to
/// report it recovered unless every one of them succeeded and the session
/// itself agrees it holds nothing afterward: AICD §12.
///
/// The teardown steps are recorded on the session through
/// `crate::session::Session::record` as each action succeeds, which means a
/// bug that calls two actions out of order is caught by
/// `crates/ori-runtime/src/session.rs`'s own ordering check (`StepOutOfOrder`)
/// rather than by anything new here: this function does not re-police an
/// order `Session` already polices.
pub fn recover_session(
    session: &Session,
    actions: &mut dyn RestartActions,
    at: Timestamp,
) -> Result<Session, RecoveryError> {
    if session.state() != SessionState::Running {
        return Err(RecoveryError::NotRunning {
            state: session.state(),
        });
    }

    actions.revoke_credentials(session, at)?;
    let next = session
        .record(TeardownStep::CredentialsRevoked, at)
        .map_err(RecoveryError::Session)?;

    actions.confirm_process_stopped(session, at)?;
    let next = next
        .record(TeardownStep::ProcessStopped, at)
        .map_err(RecoveryError::Session)?;

    actions.release_locks(session, at)?;
    let next = next
        .record(TeardownStep::LocksReleased, at)
        .map_err(RecoveryError::Session)?;

    let disposition = actions.dispose_worktree(session, at)?;
    let next = next
        .record(TeardownStep::WorktreeReleased(disposition), at)
        .map_err(RecoveryError::Session)?;

    let next = next
        .end(Outcome::Killed, at)
        .map_err(RecoveryError::Session)?;

    if next.left_an_orphan() {
        return Err(RecoveryError::Orphan {
            session: next.id().clone(),
            residue: next.residue(),
        });
    }

    actions.requeue_ticket(session, at)?;
    actions.append_recovered_event(session, at)?;

    Ok(next)
}

/// Why a session found at restart was not attempted: AICD §12.
///
/// No methodology section applies to the value itself; see the module doc's
/// note on the `Spawning` gap for [`Ineligible::Spawning`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ineligible {
    /// `spec/DATA_MODEL.md` section 3 draws no edge from `Spawning` to
    /// `Killed`; see the module doc.
    Spawning,
    /// The session already carries an outcome; the runbook's trigger is
    /// sessions found `Running`, and this one is not this restart's problem.
    AlreadyEnded(Outcome),
}

/// Whether a session is this restart's concern, and why not when it is not:
/// the one place [`recover_all`] decides which sessions it will touch.
///
/// A separate, small function rather than inlined into [`recover_all`]'s loop
/// so that "which sessions does a restart pick up" is one answer with one name
/// that a test can call directly, and (per the "prove it" list on this
/// ticket) one place a planted defect can break on its own, distinctly from a
/// defect in the driving loop itself.
fn classify(session: &Session) -> Result<(), Ineligible> {
    match session.state() {
        SessionState::Running => Ok(()),
        SessionState::Spawning => Err(Ineligible::Spawning),
        SessionState::Ended(outcome) => Err(Ineligible::AlreadyEnded(outcome)),
    }
}

/// The sessions among these that this restart is responsible for: the
/// runbook's trigger, "the engine starts and finds sessions in Running state
/// from a previous process".
#[must_use]
pub fn eligible(sessions: &[Session]) -> Vec<&Session> {
    sessions
        .iter()
        .filter(|session| classify(session).is_ok())
        .collect()
}

/// What one restart did with every session it was shown: AICD §12.
///
/// Three buckets, and every session handed to [`recover_all`] lands in
/// exactly one of them: [`RecoveryOutcome::processed`] equals the number of
/// sessions given, always, which is the conservation check the "prove it"
/// list's items 4, 5 and 6 turn on. A session that vanishes between the input
/// and the three buckets is the "present but reporting nothing" defect this
/// type is built not to allow.
#[derive(Debug, Default)]
pub struct RecoveryOutcome {
    recovered: Vec<Session>,
    failed: Vec<(Id, RecoveryError)>,
    not_eligible: Vec<(Id, Ineligible)>,
}

impl RecoveryOutcome {
    /// Sessions marked `Killed`, with credentials, process, locks and worktree
    /// all accounted for and no orphan found.
    #[must_use]
    pub fn recovered(&self) -> &[Session] {
        &self.recovered
    }

    /// Sessions this restart attempted and could not finish, with why.
    #[must_use]
    pub fn failed(&self) -> &[(Id, RecoveryError)] {
        &self.failed
    }

    /// Sessions this restart did not attempt, with why.
    #[must_use]
    pub fn not_eligible(&self) -> &[(Id, Ineligible)] {
        &self.not_eligible
    }

    /// How many sessions landed in one of the three buckets: for the
    /// conservation check against the number of sessions given to
    /// [`recover_all`].
    #[must_use]
    pub fn processed(&self) -> usize {
        self.recovered.len() + self.failed.len() + self.not_eligible.len()
    }
}

/// Runs [`recover_session`] over every `Running` session in `sessions`, and
/// accounts for every other one: AICD §12,
/// `spec/runbooks/recover-engine.md` step 1.
pub fn recover_all(
    sessions: &[Session],
    actions: &mut dyn RestartActions,
    at: Timestamp,
) -> RecoveryOutcome {
    let mut result = RecoveryOutcome::default();
    for session in sessions {
        match classify(session) {
            Ok(()) => match recover_session(session, actions, at) {
                Ok(next) => result.recovered.push(next),
                Err(err) => result.failed.push((session.id().clone(), err)),
            },
            Err(reason) => result.not_eligible.push((session.id().clone(), reason)),
        }
    }
    result
}

/// The mark `spec/runbooks/recover-engine.md` step 1 attributes a stray
/// process to a session by: the worktree path and the container id, the only
/// two `spec/DATA_MODEL.md` section 2 fields `AgentSession` carries that a
/// live process can be matched against. See the module doc's note on why
/// there is no pid to match instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessMark {
    /// The worktree path the live process appears to be working in.
    pub worktree: PathBuf,
    /// The container id the live process appears to be running in, absent for
    /// a worktree-only session (ORI-P1-032).
    pub container: Option<String>,
}

impl ProcessMark {
    /// A mark at this worktree path, in this container.
    #[must_use]
    pub fn new(worktree: &Path, container: Option<&str>) -> Self {
        Self {
            worktree: worktree.to_path_buf(),
            container: container.map(str::to_owned),
        }
    }

    /// Whether this mark is the one `session` would be attributed to.
    #[must_use]
    fn matches(&self, session: &Session) -> bool {
        session.worktree().path() == self.worktree.as_path()
            && session.container() == self.container.as_deref()
    }
}

/// `spec/runbooks/recover-engine.md` step 2: "Verify no orphan container or
/// process remains; record the check."
///
/// Every mark in `live` that matches no session in `sessions` is returned: a
/// process or container the caller found running that no session on record
/// accounts for. `sessions` should be every session this crate knows about,
/// not only the ones [`recover_all`] just processed, so that a mark belonging
/// to a session that was already `Ended` before this restart (and is
/// therefore not in `eligible`'s output) is not reported as unattributed. An
/// empty result is the check passing; it is returned as a `Vec` and not a
/// `bool` so a caller can name what it found rather than only that something
/// was wrong, matching `crate::session::Session::residue`'s shape.
#[must_use]
pub fn sweep(sessions: &[Session], live: &[ProcessMark]) -> Vec<ProcessMark> {
    live.iter()
        .filter(|mark| !sessions.iter().any(|session| mark.matches(session)))
        .cloned()
        .collect()
}

/// Everything this module refuses or reports: AICD §12.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RecoveryError {
    /// [`recover_session`] was asked to recover a session that is not
    /// `Running`.
    NotRunning {
        /// Where the session actually is.
        state: SessionState,
    },
    /// A [`RestartActions`] method reported it could not perform its step.
    ActionFailed {
        /// The step that failed.
        step: RecoveryStep,
        /// What the caller reported.
        reason: String,
    },
    /// `crate::session::Session` refused a transition or a teardown step.
    Session(SessionError),
    /// The session was driven to `Killed` but still holds something
    /// afterward: `Session::left_an_orphan` found residue. See
    /// `crates/ori-runtime/src/session.rs`'s note on the one claim here the
    /// filesystem can catch in a lie.
    Orphan {
        /// The session.
        session: Id,
        /// What it still holds.
        residue: Vec<Held>,
    },
}

impl RecoveryError {
    /// The methodology section this refusal rests on, when one applies.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::NotRunning { .. } | Self::Orphan { .. } => Some(MethodologyRef {
                section: 12,
                subsection: None,
            }),
            Self::ActionFailed { step, .. } => Some(step.reason()),
            Self::Session(err) => err.methodology_ref(),
        }
    }
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning { state } => {
                write!(
                    f,
                    "recovery only drives a Running session, and this one is {state}"
                )
            }
            Self::ActionFailed { step, reason } => {
                write!(f, "'{step}' was not performed: {reason}")
            }
            Self::Session(err) => write!(f, "{err}"),
            Self::Orphan { session, residue } => {
                let names: Vec<&str> = residue.iter().map(|held| held.as_str()).collect();
                write!(
                    f,
                    "session {session} was driven to Killed but still holds {}",
                    names.join(", ")
                )
            }
        }
    }
}

impl Error for RecoveryError {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::worktree::Worktree;

    const START: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    fn at(offset: i64) -> Timestamp {
        Timestamp::from_millis(START.millis() + offset)
    }

    /// A fresh, valid, distinct ULID on every call: a five-digit counter
    /// zero-padded into the tail of a fixed 21-character ULID prefix, which is
    /// always exactly [`ori_core::types::Id::parse`]'s required 26 characters
    /// and always Crockford base 32 (digits are all valid symbols).
    fn fresh_id() -> Id {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        Id::parse(&format!("01ARZ3NDEKTSV4RRFFQ69{unique:05}")).expect("a ULID")
    }

    /// An absolute path, on every platform, that names nothing on disk.
    /// Mirrors `crate::session::tests::absent` and
    /// `crate::worktree::tests::absolute`: the fixture shape a Windows failure
    /// already taught this crate to use everywhere a worktree path is needed.
    fn absent(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir();
        assert!(
            root.is_absolute(),
            "the temporary directory is absolute on every platform this runs on: {}",
            root.display()
        );
        root.join(format!(
            "ori-t-0034-recovery-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    /// A `Running` session at a distinct, absent worktree path.
    fn running(label: &str) -> Session {
        let session = fresh_id();
        let worktree = Worktree::new(
            session.clone(),
            absent(label).join(session.as_str()),
            "feat/ORI-T-0030",
        )
        .expect("a valid worktree");
        Session::spawning(session, fresh_id(), None, worktree, START)
            .expect("valid")
            .running(at(1))
            .expect("spawning goes to running")
    }

    /// A `Spawning` session at a distinct, absent worktree path: never
    /// advanced to `Running`.
    fn spawning(label: &str) -> Session {
        let session = fresh_id();
        let worktree = Worktree::new(
            session.clone(),
            absent(label).join(session.as_str()),
            "feat/ORI-T-0030",
        )
        .expect("a valid worktree");
        Session::spawning(session, fresh_id(), None, worktree, START).expect("valid")
    }

    /// A test double that records every call it was given and answers success
    /// or a chosen failure per step, so a "prove it" scenario can break
    /// exactly one action without touching the others.
    struct Fake {
        calls: Vec<(&'static str, Id)>,
        fail: HashMap<&'static str, String>,
        disposition: Disposition,
    }

    impl Default for Fake {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                fail: HashMap::new(),
                disposition: Disposition::Removed,
            }
        }
    }

    impl Fake {
        fn failing(step: &'static str, reason: &str) -> Self {
            let mut fake = Self::default();
            fake.fail.insert(step, reason.to_owned());
            fake
        }

        fn step(&mut self, name: &'static str, session: &Session) -> Result<(), RecoveryError> {
            self.calls.push((name, session.id().clone()));
            if let Some(reason) = self.fail.get(name) {
                return Err(RecoveryError::ActionFailed {
                    step: step_kind(name),
                    reason: reason.clone(),
                });
            }
            Ok(())
        }
    }

    fn step_kind(name: &str) -> RecoveryStep {
        match name {
            "revoke" => RecoveryStep::CredentialsRevoked,
            "process" => RecoveryStep::ProcessStopped,
            "locks" => RecoveryStep::LocksReleased,
            "worktree" => RecoveryStep::WorktreeDisposed,
            "requeue" => RecoveryStep::TicketRequeued,
            "event" => RecoveryStep::EventAppended,
            other => panic!("unknown step in test double: {other}"),
        }
    }

    impl RestartActions for Fake {
        fn revoke_credentials(
            &mut self,
            session: &Session,
            _at: Timestamp,
        ) -> Result<(), RecoveryError> {
            self.step("revoke", session)
        }

        fn confirm_process_stopped(
            &mut self,
            session: &Session,
            _at: Timestamp,
        ) -> Result<(), RecoveryError> {
            self.step("process", session)
        }

        fn release_locks(
            &mut self,
            session: &Session,
            _at: Timestamp,
        ) -> Result<(), RecoveryError> {
            self.step("locks", session)
        }

        fn dispose_worktree(
            &mut self,
            session: &Session,
            _at: Timestamp,
        ) -> Result<Disposition, RecoveryError> {
            self.step("worktree", session)?;
            Ok(self.disposition)
        }

        fn requeue_ticket(
            &mut self,
            session: &Session,
            _at: Timestamp,
        ) -> Result<(), RecoveryError> {
            self.step("requeue", session)
        }

        fn append_recovered_event(
            &mut self,
            session: &Session,
            _at: Timestamp,
        ) -> Result<(), RecoveryError> {
            self.step("event", session)
        }
    }

    // ---------------------------------------------------------------------
    // ORI-P1-031
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_031_a_running_session_is_recovered_killed_with_no_orphan() {
        let session = running("a");
        let mut fake = Fake::default();
        let recovered =
            recover_session(&session, &mut fake, at(20)).expect("every action succeeds");

        assert_eq!(recovered.outcome(), Some(Outcome::Killed));
        assert!(!recovered.left_an_orphan());
        assert_eq!(
            fake.calls,
            vec![
                ("revoke", session.id().clone()),
                ("process", session.id().clone()),
                ("locks", session.id().clone()),
                ("worktree", session.id().clone()),
                ("requeue", session.id().clone()),
                ("event", session.id().clone()),
            ],
            "the runbook's own order"
        );
    }

    #[test]
    fn ori_p1_031_recover_all_recovers_a_running_session_and_reports_the_event() {
        let sessions = vec![running("b")];
        let mut fake = Fake::default();
        let outcome = recover_all(&sessions, &mut fake, at(20));

        assert_eq!(outcome.recovered().len(), 1);
        assert_eq!(outcome.recovered()[0].outcome(), Some(Outcome::Killed));
        assert!(outcome.failed().is_empty());
        assert!(outcome.not_eligible().is_empty());
        assert_eq!(outcome.processed(), 1);
    }

    #[test]
    fn ori_p1_031_sweep_finds_no_orphan_for_a_process_that_matches_a_known_session() {
        let session = running("c");
        let mark = ProcessMark::new(session.worktree().path(), session.container());
        assert!(sweep(&[session], &[mark]).is_empty());
    }

    #[test]
    fn ori_p1_031_sweep_reports_a_live_mark_no_session_accounts_for() {
        let session = running("d");
        let stray = ProcessMark::new(&absent("stray"), None);
        let orphans = sweep(&[session], std::slice::from_ref(&stray));
        assert_eq!(orphans, vec![stray]);
    }

    #[test]
    fn ori_p1_031_a_worktree_still_on_disk_after_removal_is_claimed_is_reported_not_recovered() {
        // Distinct from `crate::session::tests::ori_p1_031_a_worktree_still_on_disk_after_the_session_ended_is_reported`:
        // that test proves `Session::residue` catches the lie. This one proves
        // `recover_session` actually consults it and refuses to report the
        // session recovered, rather than trusting the `Disposition` a
        // `RestartActions` implementation claims.
        let session = running("orphan-worktree");
        std::fs::create_dir_all(session.worktree().path())
            .expect("the scratch directory is writable");

        let refusal = recover_session(&session, &mut Fake::default(), at(20))
            .expect_err("the worktree the caller claimed removed is still there");
        let _ = std::fs::remove_dir_all(session.worktree().path());

        match refusal {
            RecoveryError::Orphan {
                session: id,
                residue,
            } => {
                assert_eq!(&id, session.id());
                assert_eq!(residue, vec![Held::Worktree]);
            }
            other => panic!("expected Orphan, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------------
    // The `Spawning` gap
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_spawning_session_is_reported_ineligible_not_silently_killed_and_not_silently_dropped()
     {
        let still_spawning = spawning("spawning-gap");
        let session_id = still_spawning.id().clone();

        assert_eq!(classify(&still_spawning), Err(Ineligible::Spawning));

        let outcome = recover_all(&[still_spawning], &mut Fake::default(), at(5));
        assert!(outcome.recovered().is_empty(), "not silently killed");
        assert!(outcome.failed().is_empty(), "not attempted and failed");
        assert_eq!(
            outcome.not_eligible(),
            &[(session_id, Ineligible::Spawning)],
            "named, not dropped"
        );
        assert_eq!(outcome.processed(), 1);
    }

    // ---------------------------------------------------------------------
    // Prove it 2: a session recovered as something other than Killed
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_recover_session_refuses_a_session_that_is_not_running() {
        let still_spawning = spawning("not-running");

        let refusal = recover_session(&still_spawning, &mut Fake::default(), at(5))
            .expect_err("a Spawning session is not this function's to drive");
        assert_eq!(
            refusal,
            RecoveryError::NotRunning {
                state: SessionState::Spawning
            }
        );
    }

    // ---------------------------------------------------------------------
    // Prove it 3: a recovered session that still holds credentials or locks
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_failed_credential_revocation_stops_recovery_before_the_session_ends() {
        let session = running("e");
        let mut fake = Fake::failing("revoke", "the broker is unreachable");
        let refusal = recover_session(&session, &mut fake, at(20)).expect_err("revoke failed");
        assert_eq!(
            refusal,
            RecoveryError::ActionFailed {
                step: RecoveryStep::CredentialsRevoked,
                reason: "the broker is unreachable".to_owned(),
            }
        );
        assert_eq!(
            fake.calls,
            vec![("revoke", session.id().clone())],
            "nothing past the failed step was even attempted"
        );
    }

    #[test]
    fn ori_t_0034_a_failed_lock_release_stops_recovery_before_the_session_ends() {
        let session = running("f");
        let mut fake = Fake::failing("locks", "the lock table has no entry for this session");
        let refusal =
            recover_session(&session, &mut fake, at(20)).expect_err("lock release failed");
        assert_eq!(
            refusal,
            RecoveryError::ActionFailed {
                step: RecoveryStep::LocksReleased,
                reason: "the lock table has no entry for this session".to_owned(),
            }
        );
        assert_eq!(
            fake.calls,
            vec![
                ("revoke", session.id().clone()),
                ("process", session.id().clone()),
                ("locks", session.id().clone()),
            ],
            "credentials and process were handled; locks failed; nothing after ran"
        );
    }

    #[test]
    fn ori_t_0034_recover_all_reports_a_failed_session_in_the_failed_bucket_not_recovered() {
        let sessions = vec![running("g")];
        let mut fake = Fake::failing("locks", "unreachable");
        let outcome = recover_all(&sessions, &mut fake, at(20));
        assert!(outcome.recovered().is_empty());
        assert_eq!(outcome.failed().len(), 1);
        assert_eq!(outcome.processed(), 1);
    }

    // ---------------------------------------------------------------------
    // Prove it 4: every session is recovered (must fail if that were true of
    // a batch that includes a Spawning session)
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_mixed_batch_recovers_only_the_running_ones() {
        let running_session = running("h");
        let spawning_session = spawning("mixed");
        let spawning_id = spawning_session.id().clone();

        let sessions = vec![running_session.clone(), spawning_session];
        let outcome = recover_all(&sessions, &mut Fake::default(), at(20));

        assert_eq!(outcome.recovered().len(), 1, "only the Running one");
        assert_eq!(outcome.recovered()[0].id(), running_session.id());
        assert_eq!(
            outcome.not_eligible(),
            &[(spawning_id, Ineligible::Spawning)]
        );
        assert_eq!(
            outcome.processed(),
            sessions.len(),
            "every session given lands in exactly one bucket"
        );
    }

    // ---------------------------------------------------------------------
    // Prove it 5 and 6: no session is ever recovered / the reader finds none
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_known_population_of_running_sessions_is_actually_recovered() {
        // This is the assertion the "prove it" list's items 5 and 6 are
        // planted against, in a copy of this file: item 5 breaks the driving
        // loop so a failure is swallowed instead of counted; item 6 breaks
        // `classify` so a Running session is misread as something else. Both
        // mutations make this assertion fail, which is the point: a recovery
        // that recovers nothing from a population that has something to
        // recover is a failing test here, not a silent pass.
        let sessions = vec![running("i"), running("j")];
        let outcome = recover_all(&sessions, &mut Fake::default(), at(20));
        assert_eq!(
            outcome.recovered().len(),
            2,
            "two Running sessions were given; two must come back Killed"
        );
        assert_eq!(outcome.processed(), 2);
    }

    #[test]
    fn ori_t_0034_classify_reads_the_three_states_correctly() {
        let running_session = running("k");
        assert_eq!(classify(&running_session), Ok(()));

        let ended = recover_session(&running_session, &mut Fake::default(), at(10))
            .expect("recovers cleanly");
        assert_eq!(
            classify(&ended),
            Err(Ineligible::AlreadyEnded(Outcome::Killed))
        );
    }

    // ---------------------------------------------------------------------
    // `eligible`
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_eligible_selects_exactly_the_running_sessions() {
        let running_session = running("l");
        let spawning_session = spawning("eligible");

        let sessions = vec![running_session.clone(), spawning_session];
        let picked = eligible(&sessions);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].id(), running_session.id());
    }

    // ---------------------------------------------------------------------
    // `RecoveryStep::reason` agrees with `StepKind::reason`
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_the_shared_four_steps_cite_the_same_sections_as_step_kind() {
        assert_eq!(
            RecoveryStep::CredentialsRevoked.reason(),
            StepKind::CredentialsRevoked.reason()
        );
        assert_eq!(
            RecoveryStep::ProcessStopped.reason(),
            StepKind::ProcessStopped.reason()
        );
        assert_eq!(
            RecoveryStep::LocksReleased.reason(),
            StepKind::LocksReleased.reason()
        );
        assert_eq!(
            RecoveryStep::WorktreeDisposed.reason(),
            StepKind::WorktreeReleased.reason()
        );
    }

    #[test]
    fn ori_t_0034_every_recovery_error_that_is_a_refusal_resolves_a_methodology_reference() {
        let refusal = RecoveryError::NotRunning {
            state: SessionState::Spawning,
        };
        assert_eq!(refusal.methodology_ref().map(|r| r.section), Some(12));

        let refusal = RecoveryError::ActionFailed {
            step: RecoveryStep::TicketRequeued,
            reason: "refused: no InProgress -> Queued edge".to_owned(),
        };
        assert_eq!(refusal.methodology_ref().map(|r| r.section), Some(11));
    }
}
