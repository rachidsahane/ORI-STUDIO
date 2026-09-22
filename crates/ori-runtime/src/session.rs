//! The agent session lifecycle: AICD §12.
//!
//! `spec/LLD.md` section 2 gives this crate `Session`. The states are
//! `spec/DATA_MODEL.md` section 3's AgentSession machine, `Spawning to Running
//! to (Blocked or Escalated or Completed or Killed)`, and the fields are its
//! section 2 row. The order in which a session lets go of what it holds is
//! `spec/LLD.md` section 5: "killing a session revokes its credentials first,
//! then terminates the process, then releases its locks, in that order".
//!
//! # Why AICD §12 and not AICD §7
//!
//! AICD §7 is the separation of duties: which agent may do what. It says
//! nothing about a session beginning or ending. AICD §12 is where a session
//! exists at all ("It assigns each ticket to a coder agent, which works on its
//! own branch"), and it names two of the four outcomes outright: the budget
//! protocol, where "the coder stops, writes a blocked report [...] and hands
//! off", is [`Outcome::Blocked`], and the escalation trigger list is
//! [`Outcome::Escalated`]. The individual teardown steps cite the sections
//! their own rules come from rather than inheriting this one; see
//! [`StepKind::reason`].
//!
//! # What this module satisfies of ORI-P1-031, and what it does not
//!
//! The criterion reads: "Engine killed while a coder session is Running |
//! Engine restart | Session marked Killed, credentials revoked, locks released,
//! ticket Queued, event `session.recovered`; no orphan process".
//!
//! Four of those clauses are not this crate's to perform. Revocation is
//! `Issuance` in `ori-broker`, lock release is `LockTable` in
//! `ori-orchestrator`, the ticket returning to `Queued` is `Lifecycle` in
//! `ori-orchestrator`, and appending `session.recovered` is `EventLog` in
//! `ori-store` (`spec/LLD.md` section 2). The restart that drives them is
//! ORI-T-0034.
//!
//! What is here is the session state, and the order. This module is the
//! supervisor's record: it refuses a session that ends before the steps
//! happened, it refuses them out of order, and it reports what a session still
//! holds. The other crates perform the steps; nothing else decides whether a
//! session may say it is done.
//!
//! # What "no orphan" means here, mechanically
//!
//! An orphan is a resource a session still holds after it has ended. There are
//! four, they are enumerated by [`Held`], and each is answered differently:
//!
//! - Credentials, the process and the locks are answered by the record.
//!   [`Session::end`] refuses unless every step the outcome requires has been
//!   recorded, so a session that ends holding one of the three is not
//!   representable. That is the whole guarantee for those three, and it is
//!   worth naming precisely: this module knows that the engine said the
//!   process stopped, not that the operating system agrees.
//! - The worktree is answered by the filesystem. [`Session::residue`] asks
//!   whether the directory is still there after teardown recorded it removed.
//!   That is the one claim here that can be caught in a lie.
//!
//! There is deliberately no process identifier anywhere in this module.
//! `spec/DATA_MODEL.md` section 2's `AgentSession` row carries `id`,
//! `identity_id`, `ticket_id`, `worktree`, `container_id`, `started_at`,
//! `ended_at`, `budget_used` and `outcome`, and no pid.
//! `spec/runbooks/recover-engine.md` step 1 matches a stray process to a
//! session by "(worktree path, container id)" for that reason. So the orphan
//! process clause of ORI-P1-031 is satisfied here by keeping those two marks
//! unambiguous, which is what `Worktree` and `WorktreeSet` in
//! `crates/ori-runtime/src/worktree.rs` are for, and the scan that uses them at
//! restart is ORI-T-0034's step 2, "Verify no orphan container or process
//! remains; record the check". Inventing a pid field to look more complete here
//! would put a field in this crate that no projection in `ori-store` could
//! rebuild.
//!
//! # A gap in the specified machine, reported and not papered over
//!
//! `spec/DATA_MODEL.md` section 3 draws exactly one edge out of `Spawning`, to
//! `Running`. An engine that dies while a session is still spawning therefore
//! leaves a session this machine cannot end, and ORI-P1-031's precondition
//! ("while a coder session is Running") does not cover that case either. This
//! module implements the machine as specified and refuses `Spawning` to
//! `Killed`. Adding the edge would be a specification change, which is the
//! documentation role's (CLAUDE.md rule 5), so it is reported rather than
//! taken.

use core::fmt;
use core::str::FromStr;
use std::error::Error;

use ori_core::error::MethodologyRef;
use ori_core::types::Id;
use ori_core::types::Timestamp;

use crate::worktree::Worktree;

/// How a session ended: AICD §12.
///
/// The four values and their spellings are `spec/DATA_MODEL.md` section 2's
/// `AgentSession` row, "outcome (completed, blocked, escalated, killed)". Two of
/// them are AICD §12's own: [`Outcome::Blocked`] is the end of the budget
/// protocol, "When any limit is exceeded, the coder stops, writes a blocked
/// report [...] and hands off", and [`Outcome::Escalated`] is the end of the
/// escalation protocol in the same section.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Outcome {
    /// The session did what it was spawned to do.
    Completed,
    /// A budget was exhausted and a blocked report was written (AICD §12).
    Blocked,
    /// A trigger fired and the question went to a human (AICD §12).
    Escalated,
    /// The session was ended by the engine rather than by itself, which is what
    /// ORI-P1-031 requires of a session found Running after a restart.
    Killed,
}

impl Outcome {
    /// Every outcome, in the order `spec/DATA_MODEL.md` section 2 lists them.
    pub const ALL: &'static [Self] = &[
        Self::Completed,
        Self::Blocked,
        Self::Escalated,
        Self::Killed,
    ];

    /// The spelling `spec/DATA_MODEL.md` section 2 gives this outcome.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::Escalated => "escalated",
            Self::Killed => "killed",
        }
    }

    /// Whether the engine ended the session rather than the session ending
    /// itself.
    ///
    /// The distinction is what decides whether the locks go back: a killed
    /// session's ticket returns to `Queued` (ORI-P1-031) and must release them,
    /// where a session that finished leaves them with a ticket that is still in
    /// flight (ORI-P1-008).
    #[must_use]
    pub const fn was_killed(self) -> bool {
        matches!(self, Self::Killed)
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Outcome {
    type Err = SessionError;

    fn from_str(text: &str) -> Result<Self, SessionError> {
        Self::ALL
            .iter()
            .copied()
            .find(|outcome| outcome.as_str() == text)
            .ok_or_else(|| SessionError::NotAnOutcome {
                text: text.to_owned(),
            })
    }
}

/// Where a session is: AICD §12.
///
/// `spec/DATA_MODEL.md` section 3 writes the machine as "Spawning to Running to
/// (Blocked or Escalated or Completed or Killed)". The four terminal states are
/// named after the four outcomes its section 2 row stores, so they are held as
/// one [`Outcome`] inside one variant rather than as four more states. That is
/// not a convenience: it makes "a terminal state has an outcome" and "an
/// outcome means the session is over" true by construction, where six flat
/// variants plus a nullable outcome column would make them two conventions that
/// can disagree.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SessionState {
    /// The session is being started and is not yet working.
    Spawning,
    /// The agent is working.
    Running,
    /// The session is over, with the outcome recorded.
    Ended(Outcome),
}

impl SessionState {
    /// Every state the machine has, which is two plus one per outcome.
    pub const ALL: &'static [Self] = &[
        Self::Spawning,
        Self::Running,
        Self::Ended(Outcome::Completed),
        Self::Ended(Outcome::Blocked),
        Self::Ended(Outcome::Escalated),
        Self::Ended(Outcome::Killed),
    ];

    /// Whether `spec/DATA_MODEL.md` section 3 draws an edge from this state to
    /// that one.
    ///
    /// The whole table is two rules: `Spawning` goes to `Running`, and
    /// `Running` goes to any ended state. Nothing else, and in particular no
    /// self transition and no edge out of an ended state: a session is a
    /// process's lifetime and does not restart.
    #[must_use]
    pub const fn may_advance_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Spawning, Self::Running) | (Self::Running, Self::Ended(_))
        )
    }

    /// The outcome, for a state that has one.
    #[must_use]
    pub const fn outcome(self) -> Option<Outcome> {
        match self {
            Self::Ended(outcome) => Some(outcome),
            _ => None,
        }
    }

    /// Whether the session is over.
    #[must_use]
    pub const fn has_ended(self) -> bool {
        matches!(self, Self::Ended(_))
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawning => f.write_str("spawning"),
            Self::Running => f.write_str("running"),
            Self::Ended(outcome) => f.write_str(outcome.as_str()),
        }
    }
}

/// What a session holds and has to let go of: AICD §12.
///
/// The list is closed and it is the list ORI-P1-031 and
/// `spec/runbooks/recover-engine.md` step 1 name between them: credentials,
/// the process, the lock entries, and the worktree. One variant per resource is
/// what lets [`Session::residue`] answer "what is left" with a value rather
/// than with a boolean that cannot say what was left.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Held {
    /// Credentials issued to the session by `ori-broker`.
    Credentials,
    /// The agent process the runtime spawned.
    Process,
    /// Lock entries the ticket's declared scope claimed (AICD §12).
    Locks,
    /// The git worktree the session works in.
    Worktree,
}

impl Held {
    /// Every resource a session can hold.
    pub const ALL: &'static [Self] = &[
        Self::Credentials,
        Self::Process,
        Self::Locks,
        Self::Worktree,
    ];

    /// The resource in the words the criterion and the runbook use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Credentials => "credentials",
            Self::Process => "process",
            Self::Locks => "locks",
            Self::Worktree => "worktree",
        }
    }
}

impl fmt::Display for Held {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What happened to the worktree when the session ended.
///
/// No methodology section applies directly. The reason the disposition is
/// recorded at all is that neither `spec/LLD.md` section 5 nor
/// `spec/runbooks/recover-engine.md` says a worktree is always deleted, and it
/// is not always right to delete one: a killed session's checkout may be the
/// only copy of work a human wants to read. So the requirement is that the
/// disposition is decided and recorded, never that it is one particular value.
/// An undecided worktree is the orphan; a retained one is a decision with an
/// owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Disposition {
    /// The worktree was removed from disk.
    Removed,
    /// The worktree was deliberately kept.
    Retained,
}

impl Disposition {
    /// The disposition as one word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Retained => "retained",
        }
    }
}

impl fmt::Display for Disposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One step of letting go, without its data: AICD §12.
///
/// The kinds exist separately from [`TeardownStep`] so that an error can name
/// the step that is missing without carrying a disposition it does not know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StepKind {
    /// `ori-broker` revoked every issuance bound to this session.
    CredentialsRevoked,
    /// The agent process is no longer running.
    ProcessStopped,
    /// `ori-orchestrator` released the lock entries the ticket claimed.
    LocksReleased,
    /// The worktree was removed or deliberately retained.
    WorktreeReleased,
}

impl StepKind {
    /// Every step, in the order `spec/LLD.md` section 5 fixes, with the
    /// worktree last.
    pub const ALL: &'static [Self] = &[
        Self::CredentialsRevoked,
        Self::ProcessStopped,
        Self::LocksReleased,
        Self::WorktreeReleased,
    ];

    /// The step that must have happened before this one.
    ///
    /// `spec/LLD.md` section 5 fixes the first three: "killing a session revokes
    /// its credentials first, then terminates the process, then releases its
    /// locks, in that order". The worktree hangs off the process rather than
    /// off the locks, because what it needs is that nothing is writing to the
    /// checkout, and that is the process stopping. Nothing in the specification
    /// orders the worktree against the locks, so this does not either: an order
    /// invented here would be refused later against a caller doing nothing
    /// wrong.
    #[must_use]
    pub const fn prerequisite(self) -> Option<Self> {
        match self {
            Self::CredentialsRevoked => None,
            Self::ProcessStopped => Some(Self::CredentialsRevoked),
            Self::LocksReleased | Self::WorktreeReleased => Some(Self::ProcessStopped),
        }
    }

    /// What the step lets go of.
    #[must_use]
    pub const fn releases(self) -> Held {
        match self {
            Self::CredentialsRevoked => Held::Credentials,
            Self::ProcessStopped => Held::Process,
            Self::LocksReleased => Held::Locks,
            Self::WorktreeReleased => Held::Worktree,
        }
    }

    /// The methodology section a refusal about this step rests on.
    ///
    /// Each is derived from the sentence that makes the step a rule, not from
    /// `spec/ARCHITECTURE.md` section 2's component table: rulings R10, R12 and
    /// R13 in `ops/rulings.md` record three citations taken from that table
    /// which were wrong, and its runtime row is the one this module would have
    /// read.
    ///
    /// - Credentials: AICD §27's secrets architecture, "Agents receive
    ///   credentials at runtime, scoped to the task and expiring with it". A
    ///   session that ends with its issuances live is a credential that outlived
    ///   its task.
    /// - The process: AICD §17's principle that "Every agent action is written
    ///   to an immutable audit trail: which agent, which ticket, which tool,
    ///   what input, what output, when". A process that outlives its session
    ///   acts under no session and therefore under no ticket, so its actions
    ///   cannot carry that tuple.
    /// - The locks: AICD §12's lock table, which "refuses to start a ticket
    ///   whose declared scope overlaps one already claimed". A lock nobody
    ///   released blocks every future ticket over those modules.
    /// - The worktree: AICD §7's role table, "Its own branch only", which is the
    ///   sentence `crates/ori-runtime/src/worktree.rs` carries in full.
    #[must_use]
    pub const fn reason(self) -> MethodologyRef {
        let section = match self {
            Self::CredentialsRevoked => 27,
            Self::ProcessStopped => 17,
            Self::LocksReleased => 12,
            Self::WorktreeReleased => 7,
        };
        MethodologyRef {
            section,
            subsection: None,
        }
    }

    /// The step in the words the specification uses for it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CredentialsRevoked => "credentials revoked",
            Self::ProcessStopped => "process stopped",
            Self::LocksReleased => "locks released",
            Self::WorktreeReleased => "worktree released",
        }
    }
}

impl fmt::Display for StepKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One step of letting go, with what it needs to carry: AICD §12.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TeardownStep {
    /// `ori-broker` reports every issuance for this session revoked.
    CredentialsRevoked,
    /// The agent process is no longer running.
    ProcessStopped,
    /// `ori-orchestrator` reports the session's lock entries released.
    LocksReleased,
    /// The worktree was removed, or deliberately kept.
    WorktreeReleased(Disposition),
}

impl TeardownStep {
    /// Which step this is, without its data.
    #[must_use]
    pub const fn kind(self) -> StepKind {
        match self {
            Self::CredentialsRevoked => StepKind::CredentialsRevoked,
            Self::ProcessStopped => StepKind::ProcessStopped,
            Self::LocksReleased => StepKind::LocksReleased,
            Self::WorktreeReleased(_) => StepKind::WorktreeReleased,
        }
    }
}

impl fmt::Display for TeardownStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorktreeReleased(disposition) => {
                write!(f, "{} ({disposition})", self.kind())
            }
            other => write!(f, "{}", other.kind()),
        }
    }
}

/// What a session has let go of, and when: AICD §12.
///
/// The order is `spec/LLD.md` section 5's and it is enforced rather than
/// documented: [`Teardown::record`] refuses a step whose prerequisite is
/// unrecorded, and refuses one timestamped before the step it must follow.
/// ORI-P1-020 is the criterion that asks for the second of those in so many
/// words, "Every issuance for the session is revoked with a timestamp before
/// the process is terminated"; that criterion belongs to ORI-T-0027 and
/// ORI-T-0032, and what this type owes it is that the runtime cannot record the
/// two in the wrong order.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Teardown {
    credentials_revoked_at: Option<Timestamp>,
    process_stopped_at: Option<Timestamp>,
    locks_released_at: Option<Timestamp>,
    worktree_released: Option<(Timestamp, Disposition)>,
}

impl Teardown {
    /// Nothing let go of yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// When a step was recorded, if it was.
    #[must_use]
    pub fn at(&self, step: StepKind) -> Option<Timestamp> {
        match step {
            StepKind::CredentialsRevoked => self.credentials_revoked_at,
            StepKind::ProcessStopped => self.process_stopped_at,
            StepKind::LocksReleased => self.locks_released_at,
            StepKind::WorktreeReleased => self.worktree_released.map(|(at, _)| at),
        }
    }

    /// What was decided about the worktree, once it has been decided.
    #[must_use]
    pub fn disposition(&self) -> Option<Disposition> {
        self.worktree_released.map(|(_, what)| what)
    }

    /// Records a step, refusing one out of order, one recorded twice, and one
    /// timestamped before the step it must follow.
    pub fn record(&self, step: TeardownStep, at: Timestamp) -> Result<Self, SessionError> {
        let kind = step.kind();
        if self.at(kind).is_some() {
            return Err(SessionError::StepRepeated { step: kind });
        }
        if let Some(earlier) = kind.prerequisite() {
            match self.at(earlier) {
                None => {
                    return Err(SessionError::StepOutOfOrder {
                        step: kind,
                        earlier,
                    });
                }
                Some(recorded) if recorded > at => {
                    return Err(SessionError::StepNotAfter {
                        step: kind,
                        at,
                        earlier,
                        recorded,
                    });
                }
                Some(_) => {}
            }
        }
        let mut next = *self;
        match step {
            TeardownStep::CredentialsRevoked => next.credentials_revoked_at = Some(at),
            TeardownStep::ProcessStopped => next.process_stopped_at = Some(at),
            TeardownStep::LocksReleased => next.locks_released_at = Some(at),
            TeardownStep::WorktreeReleased(what) => next.worktree_released = Some((at, what)),
        }
        Ok(next)
    }

    /// The latest timestamp recorded, if any step has been.
    #[must_use]
    pub fn last_at(&self) -> Option<Timestamp> {
        StepKind::ALL.iter().filter_map(|step| self.at(*step)).max()
    }
}

/// One agent session: AICD §12.
///
/// The fields are `spec/DATA_MODEL.md` section 2's `AgentSession` row, less
/// `budget_used`, which arrives with the meter that spends it in ORI-T-0034
/// (`ops/phase-1-backlog.md` batch 5). `container_id` is held as the opaque
/// string that row calls it: the type behind it, and the runtime that issues
/// it, are ORI-T-0031's.
///
/// Transitions take `&self` and return a new session, the way
/// `ori_core::ticket::Ticket::apply` does, so that a refused transition leaves
/// the caller holding the session it had.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    id: Id,
    identity: Id,
    ticket: Option<Id>,
    worktree: Worktree,
    container: Option<String>,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
    state: SessionState,
    teardown: Teardown,
}

impl Session {
    /// A session being spawned for an identity, in its worktree.
    ///
    /// The ticket is optional because `spec/DATA_MODEL.md` section 2 marks it
    /// "nullable for unattended": the QA, operations, documentation and product
    /// signal agents run on triggers rather than on a ticket
    /// (`spec/ARCHITECTURE.md` section 4).
    ///
    /// A worktree belonging to another session is refused here rather than
    /// later, because the pairing of session to worktree is what
    /// `spec/runbooks/recover-engine.md` uses to attribute a stray process.
    pub fn spawning(
        id: Id,
        identity: Id,
        ticket: Option<Id>,
        worktree: Worktree,
        started_at: Timestamp,
    ) -> Result<Self, SessionError> {
        if worktree.session() != &id {
            return Err(SessionError::WorktreeOfAnotherSession {
                session: id,
                worktree: worktree.session().clone(),
            });
        }
        Ok(Self {
            id,
            identity,
            ticket,
            worktree,
            container: None,
            started_at,
            ended_at: None,
            state: SessionState::Spawning,
            teardown: Teardown::new(),
        })
    }

    /// Records the container the session runs in, which ORI-T-0031 creates.
    pub fn in_container(&self, container: &str) -> Result<Self, SessionError> {
        if container.trim().is_empty() {
            return Err(SessionError::EmptyContainerId);
        }
        let mut next = self.clone();
        next.container = Some(container.to_owned());
        Ok(next)
    }

    /// The session identifier.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The agent identity running it.
    #[must_use]
    pub fn identity(&self) -> &Id {
        &self.identity
    }

    /// The ticket it is working, absent for an unattended session.
    #[must_use]
    pub fn ticket(&self) -> Option<&Id> {
        self.ticket.as_ref()
    }

    /// The worktree it works in.
    #[must_use]
    pub fn worktree(&self) -> &Worktree {
        &self.worktree
    }

    /// The container it runs in, absent when it runs worktree only
    /// (ORI-P1-032).
    #[must_use]
    pub fn container(&self) -> Option<&str> {
        self.container.as_deref()
    }

    /// When it started.
    #[must_use]
    pub fn started_at(&self) -> Timestamp {
        self.started_at
    }

    /// When it ended, absent while it has not.
    #[must_use]
    pub fn ended_at(&self) -> Option<Timestamp> {
        self.ended_at
    }

    /// Where it is.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// How it ended, absent while it has not.
    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        self.state.outcome()
    }

    /// What it has let go of, and when.
    #[must_use]
    pub fn teardown(&self) -> Teardown {
        self.teardown
    }

    /// The agent is working.
    pub fn running(&self, at: Timestamp) -> Result<Self, SessionError> {
        self.advance(SessionState::Running, at)
    }

    /// Records one step of letting go.
    ///
    /// Only a running session may: a session that has not started has nothing
    /// to let go of, and one that has ended let go already, so a step arriving
    /// afterwards is the engine reporting the same teardown twice or reporting
    /// one that did not happen.
    pub fn record(&self, step: TeardownStep, at: Timestamp) -> Result<Self, SessionError> {
        if self.state != SessionState::Running {
            return Err(SessionError::StepOutsideRunning {
                step: step.kind(),
                state: self.state,
            });
        }
        let mut next = self.clone();
        next.teardown = self.teardown.record(step, at)?;
        Ok(next)
    }

    /// Ends the session, refusing unless it has let go of what this outcome
    /// requires.
    ///
    /// Credentials and the process are required whatever the outcome:
    /// `spec/DATA_MODEL.md` section 3 says "On any terminal state, credentials
    /// issued to the session are revoked", and a session whose process is still
    /// running has not ended in any sense a human would recognise.
    ///
    /// The locks are required of a killed session and of no other, and this is
    /// the one place where the four outcomes are not alike. ORI-P1-031 puts
    /// "locks released" and "ticket Queued" in one clause: the kill hands the
    /// ticket back to the queue, so it must hand the modules back too. A
    /// session that completed leaves a ticket in flight, and ORI-P1-008 has
    /// that ticket keep its claim until it closes ("after the first closes, the
    /// second starts"). Requiring a lock release from every outcome would
    /// release the modules of a ticket that is still being reviewed, which is
    /// the lock table failing open.
    ///
    /// The worktree is required of every outcome, as a decision and not as a
    /// particular decision: see [`Disposition`].
    pub fn end(&self, outcome: Outcome, at: Timestamp) -> Result<Self, SessionError> {
        for step in self.required_steps(outcome) {
            match self.teardown.at(step) {
                None => {
                    return Err(SessionError::Incomplete {
                        outcome,
                        missing: step,
                    });
                }
                Some(recorded) if recorded > at => {
                    return Err(SessionError::EndBeforeStep { step, at, recorded });
                }
                Some(_) => {}
            }
        }
        let mut next = self.advance(SessionState::Ended(outcome), at)?;
        next.ended_at = Some(at);
        Ok(next)
    }

    /// The steps this outcome requires before the session may end.
    ///
    /// No methodology section applies on its own; the derivation is in
    /// [`Session::end`].
    fn required_steps(&self, outcome: Outcome) -> Vec<StepKind> {
        StepKind::ALL
            .iter()
            .copied()
            .filter(|step| *step != StepKind::LocksReleased || outcome.was_killed())
            .collect()
    }

    /// Everything the session has not let go of yet, in the order of
    /// [`StepKind::ALL`].
    ///
    /// For a running session this is the work a restart has to do, which is
    /// `spec/runbooks/recover-engine.md` step 1. For an ended one it is read by
    /// [`Session::residue`], which decides which of it is an orphan.
    ///
    /// The worktree counts as outstanding while no disposition is recorded, and
    /// also when the recorded disposition is [`Disposition::Removed`] and the
    /// directory is still on disk. That second case is the only one here that
    /// asks the filesystem anything, and it is the only one that can catch the
    /// record being wrong.
    #[must_use]
    pub fn outstanding(&self) -> Vec<Held> {
        StepKind::ALL
            .iter()
            .copied()
            .filter(|step| match self.teardown.at(*step) {
                None => true,
                Some(_) => {
                    *step == StepKind::WorktreeReleased
                        && self.teardown.disposition() == Some(Disposition::Removed)
                        && self.worktree.exists()
                }
            })
            .map(StepKind::releases)
            .collect()
    }

    /// What an ended session is still holding that its ending should have let
    /// go of: ORI-P1-031's "no orphan".
    ///
    /// Empty while the session is running, because a running session is
    /// supposed to hold everything. Empty for an ended session that let go of
    /// what its outcome required. Otherwise it names what is left, so that the
    /// caller reporting it can say which resource rather than only that
    /// something is wrong.
    ///
    /// The locks of a session that was not killed are outstanding and are not
    /// residue: they belong to a ticket that is still in flight. The reasoning
    /// is in [`Session::end`].
    #[must_use]
    pub fn residue(&self) -> Vec<Held> {
        if !self.state.has_ended() {
            return Vec::new();
        }
        let keeps_locks = self.outcome().is_some_and(|outcome| !outcome.was_killed());
        self.outstanding()
            .into_iter()
            .filter(|held| !(keeps_locks && *held == Held::Locks))
            .collect()
    }

    /// Whether this session left an orphan behind.
    #[must_use]
    pub fn left_an_orphan(&self) -> bool {
        !self.residue().is_empty()
    }

    /// Moves to a state, refusing an edge `spec/DATA_MODEL.md` section 3 does
    /// not draw and a timestamp before the session started.
    fn advance(&self, to: SessionState, at: Timestamp) -> Result<Self, SessionError> {
        if !self.state.may_advance_to(to) {
            return Err(SessionError::Transition {
                from: self.state,
                to,
            });
        }
        if at < self.started_at {
            return Err(SessionError::BeforeStart {
                at,
                started_at: self.started_at,
            });
        }
        let mut next = self.clone();
        next.state = to;
        Ok(next)
    }
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "session {} ({}) for identity {}",
            self.id, self.state, self.identity
        )
    }
}

/// Everything this module refuses or cannot read: AICD §12.
///
/// The split is `ori_core::error::Error`'s: a control refusing an action
/// carries a methodology reason, and a value that did not parse does not.
/// [`SessionError::methodology_ref`] is where each reason is attached, once, so
/// that two call sites cannot cite two sections for one refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SessionError {
    /// An edge the AgentSession machine does not draw.
    Transition {
        /// Where the session was.
        from: SessionState,
        /// Where it was asked to go.
        to: SessionState,
    },
    /// A teardown step arrived before the step it must follow.
    StepOutOfOrder {
        /// The step that arrived.
        step: StepKind,
        /// The step that has not been recorded yet.
        earlier: StepKind,
    },
    /// A teardown step is timestamped before the step it must follow.
    StepNotAfter {
        /// The step that arrived.
        step: StepKind,
        /// The time it claims.
        at: Timestamp,
        /// The step it must follow.
        earlier: StepKind,
        /// The time that step was recorded at.
        recorded: Timestamp,
    },
    /// A teardown step was recorded twice.
    StepRepeated {
        /// The step.
        step: StepKind,
    },
    /// A teardown step arrived for a session that is not running.
    StepOutsideRunning {
        /// The step that arrived.
        step: StepKind,
        /// Where the session is.
        state: SessionState,
    },
    /// The session was asked to end while it still holds something its outcome
    /// requires it to let go of.
    Incomplete {
        /// The outcome asked for.
        outcome: Outcome,
        /// The step that has not been recorded.
        missing: StepKind,
    },
    /// The session was asked to end at a time before a step it recorded.
    EndBeforeStep {
        /// The step.
        step: StepKind,
        /// The end time asked for.
        at: Timestamp,
        /// The time the step was recorded at.
        recorded: Timestamp,
    },
    /// A transition was asked for at a time before the session started.
    BeforeStart {
        /// The time asked for.
        at: Timestamp,
        /// When the session started.
        started_at: Timestamp,
    },
    /// The session was given a worktree belonging to another session.
    WorktreeOfAnotherSession {
        /// This session.
        session: Id,
        /// The session the worktree belongs to.
        worktree: Id,
    },
    /// A container identifier that is empty or only whitespace.
    EmptyContainerId,
    /// Text that is not one of the four outcomes.
    NotAnOutcome {
        /// The text as given.
        text: String,
    },
}

impl SessionError {
    /// The methodology section this refusal rests on.
    ///
    /// The lifecycle refusals rest on AICD §12, which is where a session exists
    /// at all; the teardown refusals inherit the reason of the step at issue,
    /// through [`StepKind::reason`], so that a missing revocation cites the
    /// section about credentials and a missing lock release cites the section
    /// about the lock table. The two variants that are a value failing to parse
    /// return `None`.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        let section = match self {
            Self::Transition { .. } | Self::BeforeStart { .. } => 12,
            Self::StepOutOfOrder { earlier, .. } | Self::StepNotAfter { earlier, .. } => {
                return Some(earlier.reason());
            }
            Self::StepRepeated { step } | Self::StepOutsideRunning { step, .. } => {
                return Some(step.reason());
            }
            Self::Incomplete { missing, .. } => return Some(missing.reason()),
            Self::EndBeforeStep { step, .. } => return Some(step.reason()),
            Self::WorktreeOfAnotherSession { .. } => 7,
            Self::EmptyContainerId | Self::NotAnOutcome { .. } => return None,
        };
        Some(MethodologyRef {
            section,
            subsection: None,
        })
    }

    /// Whether a control refused the action, as opposed to a value failing to
    /// parse.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        !matches!(self, Self::EmptyContainerId | Self::NotAnOutcome { .. })
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transition { from, to } => {
                write!(f, "a session cannot move from {from} to {to}")
            }
            Self::StepOutOfOrder { step, earlier } => write!(
                f,
                "a session records '{earlier}' before '{step}' (spec/LLD.md section 5)"
            ),
            Self::StepNotAfter {
                step,
                at,
                earlier,
                recorded,
            } => write!(
                f,
                "'{step}' at {at} is before '{earlier}' at {recorded}, and it must follow it"
            ),
            Self::StepRepeated { step } => {
                write!(f, "'{step}' was already recorded for this session")
            }
            Self::StepOutsideRunning { step, state } => {
                write!(
                    f,
                    "'{step}' belongs to a running session, and this one is {state}"
                )
            }
            Self::Incomplete { outcome, missing } => write!(
                f,
                "a session cannot end {outcome} while it still holds its {}: '{missing}' is not \
                 recorded",
                missing.releases()
            ),
            Self::EndBeforeStep { step, at, recorded } => write!(
                f,
                "a session cannot end at {at}, before '{step}' at {recorded}"
            ),
            Self::BeforeStart { at, started_at } => {
                write!(f, "{at} is before the session started at {started_at}")
            }
            Self::WorktreeOfAnotherSession { session, worktree } => write!(
                f,
                "session {session} was given the worktree of session {worktree}"
            ),
            Self::EmptyContainerId => f.write_str("a container identifier is not empty"),
            Self::NotAnOutcome { text } => write!(
                f,
                "not an outcome: {text:?}; the four are completed, blocked, escalated, killed"
            ),
        }
    }
}

impl Error for SessionError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use super::*;

    const START: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    fn at(offset: i64) -> Timestamp {
        Timestamp::from_millis(START.millis() + offset)
    }

    fn id(tail: &str) -> Id {
        Id::parse(&format!("01ARZ3NDEKTSV4RRFFQ69{tail}")).expect("a ULID")
    }

    /// A session in a worktree that is nowhere near a real filesystem.
    fn spawning() -> Session {
        let session = id("G5FAV");
        let worktree = Worktree::new(
            session.clone(),
            "/nowhere/ori-t-0030/01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "feat/ORI-T-0030",
        )
        .expect("a valid worktree");
        Session::spawning(session, id("G5FAW"), Some(id("G5FAX")), worktree, START)
            .expect("the worktree is this session's")
    }

    /// A running session in a worktree at a path the caller chose.
    fn running_at(path: PathBuf) -> Session {
        let session = id("G5FAV");
        let worktree =
            Worktree::new(session.clone(), path, "feat/ORI-T-0030").expect("a valid worktree");
        Session::spawning(session, id("G5FAW"), Some(id("G5FAX")), worktree, START)
            .expect("valid")
            .running(at(1))
            .expect("spawning goes to running")
    }

    /// A directory under the temporary directory that removes itself.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("after 1970")
                .as_nanos();
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0030-session-{label}-{}-{unique}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("the temporary directory is writable");
            let path = path.canonicalize().expect("just created");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Walks the whole teardown of a killed session.
    fn killed(session: &Session, disposition: Disposition) -> Session {
        session
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("first")
            .record(TeardownStep::ProcessStopped, at(11))
            .expect("second")
            .record(TeardownStep::LocksReleased, at(12))
            .expect("third")
            .record(TeardownStep::WorktreeReleased(disposition), at(13))
            .expect("fourth")
            .end(Outcome::Killed, at(14))
            .expect("the session ends killed")
    }

    // ---------------------------------------------------------------------
    // ORI-P1-031: engine killed while a coder session is Running
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_031_a_running_session_ends_killed_and_holds_nothing_afterwards() {
        let session = spawning().running(at(1)).expect("running");
        assert_eq!(session.state(), SessionState::Running);
        assert_eq!(
            session.outstanding(),
            vec![
                Held::Credentials,
                Held::Process,
                Held::Locks,
                Held::Worktree
            ],
            "a running session holds all four, which is what a restart has to undo"
        );

        let ended = killed(&session, Disposition::Removed);

        assert_eq!(
            ended.state(),
            SessionState::Ended(Outcome::Killed),
            "the session is marked Killed"
        );
        assert_eq!(ended.outcome(), Some(Outcome::Killed));
        assert_eq!(ended.ended_at(), Some(at(14)));
        assert!(
            ended.outstanding().is_empty(),
            "and it holds nothing: {:?}",
            ended.outstanding()
        );
        assert!(!ended.left_an_orphan(), "so there is no orphan");
    }

    #[test]
    fn ori_p1_031_a_session_cannot_end_killed_before_its_credentials_are_revoked() {
        let session = spawning().running(at(1)).expect("running");
        let refusal = session
            .end(Outcome::Killed, at(9))
            .expect_err("nothing was let go of");
        assert_eq!(
            refusal,
            SessionError::Incomplete {
                outcome: Outcome::Killed,
                missing: StepKind::CredentialsRevoked,
            }
        );
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(27),
            "a credential that outlives its task is AICD §27's rule"
        );
        assert_eq!(
            session.state(),
            SessionState::Running,
            "and the session is where it was"
        );
    }

    #[test]
    fn ori_p1_031_the_process_cannot_be_reported_stopped_before_the_credentials_are_revoked() {
        let session = spawning().running(at(1)).expect("running");
        let refusal = session
            .record(TeardownStep::ProcessStopped, at(10))
            .expect_err("spec/LLD.md section 5 fixes the order");
        assert_eq!(
            refusal,
            SessionError::StepOutOfOrder {
                step: StepKind::ProcessStopped,
                earlier: StepKind::CredentialsRevoked,
            }
        );
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(27)
        );
    }

    #[test]
    fn ori_p1_031_a_killed_session_that_never_released_its_locks_cannot_end() {
        let session = spawning()
            .running(at(1))
            .expect("running")
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("first")
            .record(TeardownStep::ProcessStopped, at(11))
            .expect("second")
            .record(TeardownStep::WorktreeReleased(Disposition::Removed), at(12))
            .expect("the worktree does not wait for the locks");

        let refusal = session
            .end(Outcome::Killed, at(13))
            .expect_err("a killed session hands its modules back");
        assert_eq!(
            refusal,
            SessionError::Incomplete {
                outcome: Outcome::Killed,
                missing: StepKind::LocksReleased,
            }
        );
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(12),
            "a lock nobody released is AICD §12's lock table failing closed forever"
        );
    }

    #[test]
    fn ori_p1_031_a_worktree_still_on_disk_after_the_session_ended_is_reported() {
        let scratch = Scratch::new("residue");
        let path = scratch.path.join("session");
        fs::create_dir_all(&path).expect("writable");

        let ended = killed(&running_at(path.clone()), Disposition::Removed);

        assert_eq!(
            ended.outcome(),
            Some(Outcome::Killed),
            "the record says the session ended cleanly"
        );
        assert_eq!(
            ended.residue(),
            vec![Held::Worktree],
            "and the directory is still there, so the record is wrong"
        );
        assert!(ended.left_an_orphan());

        // Remove it and the same session reports nothing. The reader answers
        // from the filesystem, not from a constant.
        fs::remove_dir_all(&path).expect("removable");
        assert!(
            ended.residue().is_empty(),
            "the worktree is gone and the session is clean"
        );
        assert!(!ended.left_an_orphan());
    }

    #[test]
    fn ori_p1_031_a_retained_worktree_is_a_decision_and_not_an_orphan() {
        let scratch = Scratch::new("retained");
        let path = scratch.path.join("session");
        fs::create_dir_all(&path).expect("writable");

        let ended = killed(&running_at(path), Disposition::Retained);
        assert_eq!(ended.teardown().disposition(), Some(Disposition::Retained));
        assert!(
            ended.residue().is_empty(),
            "a worktree kept on purpose is accounted for"
        );
    }

    // ---------------------------------------------------------------------
    // The machine, enumerated
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0030_the_transition_table_is_the_data_model_diagram_and_nothing_else() {
        // spec/DATA_MODEL.md section 3: Spawning to Running to (Blocked or
        // Escalated or Completed or Killed). Five edges, and 36 pairs to check.
        let drawn = [
            (SessionState::Spawning, SessionState::Running),
            (
                SessionState::Running,
                SessionState::Ended(Outcome::Completed),
            ),
            (SessionState::Running, SessionState::Ended(Outcome::Blocked)),
            (
                SessionState::Running,
                SessionState::Ended(Outcome::Escalated),
            ),
            (SessionState::Running, SessionState::Ended(Outcome::Killed)),
        ];
        assert_eq!(SessionState::ALL.len(), 6);
        let mut allowed = 0;
        for from in SessionState::ALL {
            for to in SessionState::ALL {
                let expected = drawn.contains(&(*from, *to));
                assert_eq!(
                    from.may_advance_to(*to),
                    expected,
                    "the edge {from} to {to} is {}drawn",
                    if expected { "" } else { "not " }
                );
                if expected {
                    allowed += 1;
                }
                if from == to {
                    assert!(
                        !from.may_advance_to(*to),
                        "no state moves to itself: {from}"
                    );
                }
            }
        }
        assert_eq!(allowed, 5, "five edges out of thirty-six pairs");
    }

    #[test]
    fn ori_t_0030_an_ended_session_cannot_start_again() {
        let ended = killed(
            &spawning().running(at(1)).expect("running"),
            Disposition::Removed,
        );
        let refusal = ended
            .running(at(20))
            .expect_err("a session does not restart");
        assert_eq!(
            refusal,
            SessionError::Transition {
                from: SessionState::Ended(Outcome::Killed),
                to: SessionState::Running,
            }
        );
        assert!(
            ended.record(TeardownStep::LocksReleased, at(20)).is_err(),
            "and nothing is torn down twice"
        );
    }

    #[test]
    fn ori_t_0030_a_spawning_session_cannot_be_killed_without_running_first() {
        // spec/DATA_MODEL.md section 3 draws one edge out of Spawning. The gap
        // this leaves for an engine that dies mid-spawn is named at the head of
        // this module and is a question for the specification, not a transition
        // invented here.
        let refusal = spawning()
            .end(Outcome::Killed, at(5))
            .expect_err("the machine draws no such edge");
        assert!(
            matches!(
                refusal,
                SessionError::Incomplete { .. } | SessionError::Transition { .. }
            ),
            "{refusal}"
        );
        assert!(
            spawning()
                .record(TeardownStep::CredentialsRevoked, at(5))
                .is_err(),
            "and a spawning session has nothing to let go of yet"
        );
    }

    #[test]
    fn ori_t_0030_every_ordering_of_the_four_steps_is_accepted_only_where_it_is_legal() {
        // Four steps, 24 orderings, all of them enumerated. Legal means each
        // step follows the one spec/LLD.md section 5 puts before it, so
        // credentials first, then the process, then the locks and the worktree
        // in either order: two of the twenty-four.
        let steps = [
            TeardownStep::CredentialsRevoked,
            TeardownStep::ProcessStopped,
            TeardownStep::LocksReleased,
            TeardownStep::WorktreeReleased(Disposition::Removed),
        ];
        let mut legal = 0;
        let mut orderings = 0;
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let order = [a, b, c, d];
                        if order
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != 4
                        {
                            continue;
                        }
                        orderings += 1;
                        let mut session = spawning().running(at(1)).expect("running");
                        let mut accepted = true;
                        for (offset, which) in order.iter().enumerate() {
                            let step = steps[*which];
                            match session.record(step, at(10 + offset as i64)) {
                                Ok(next) => session = next,
                                Err(_) => {
                                    accepted = false;
                                    break;
                                }
                            }
                        }
                        let expected = position(&order, 0) < position(&order, 1)
                            && position(&order, 1) < position(&order, 2)
                            && position(&order, 1) < position(&order, 3);
                        assert_eq!(accepted, expected, "ordering {order:?}");
                        if expected {
                            legal += 1;
                            assert!(
                                session.end(Outcome::Killed, at(20)).is_ok(),
                                "a complete teardown ends the session: {order:?}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(orderings, 24, "every permutation was tried");
        assert_eq!(legal, 2, "two of them are legal");
    }

    /// Where a step sits in an ordering.
    fn position(order: &[usize; 4], step: usize) -> usize {
        order
            .iter()
            .position(|which| *which == step)
            .expect("every step appears once")
    }

    #[test]
    fn ori_t_0030_a_step_recorded_twice_is_refused() {
        let session = spawning()
            .running(at(1))
            .expect("running")
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("first");
        let refusal = session
            .record(TeardownStep::CredentialsRevoked, at(11))
            .expect_err("once only");
        assert_eq!(
            refusal,
            SessionError::StepRepeated {
                step: StepKind::CredentialsRevoked,
            }
        );
    }

    #[test]
    fn ori_t_0030_a_step_timestamped_before_the_step_it_follows_is_refused() {
        // ORI-P1-020 asks for this in so many words: every issuance is revoked
        // "with a timestamp before the process is terminated". The criterion
        // belongs to ORI-T-0027 and ORI-T-0032; what is owed here is that the
        // runtime cannot record the pair the wrong way round.
        let session = spawning()
            .running(at(1))
            .expect("running")
            .record(TeardownStep::CredentialsRevoked, at(20))
            .expect("first");
        let refusal = session
            .record(TeardownStep::ProcessStopped, at(10))
            .expect_err("the process stopped before the credentials were revoked");
        assert_eq!(
            refusal,
            SessionError::StepNotAfter {
                step: StepKind::ProcessStopped,
                at: at(10),
                earlier: StepKind::CredentialsRevoked,
                recorded: at(20),
            }
        );
        assert!(
            session.record(TeardownStep::ProcessStopped, at(20)).is_ok(),
            "the same millisecond is not before"
        );
    }

    #[test]
    fn ori_t_0030_a_session_cannot_end_before_a_step_it_recorded() {
        let session = spawning().running(at(1)).expect("running");
        let torn = session
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("first")
            .record(TeardownStep::ProcessStopped, at(11))
            .expect("second")
            .record(TeardownStep::WorktreeReleased(Disposition::Removed), at(12))
            .expect("third");
        let refusal = torn
            .end(Outcome::Completed, at(5))
            .expect_err("the end is after the steps");
        assert!(
            matches!(refusal, SessionError::EndBeforeStep { .. }),
            "{refusal}"
        );
    }

    #[test]
    fn ori_t_0030_a_transition_leaves_the_session_it_was_applied_to_alone() {
        let session = spawning();
        let running = session.running(at(1)).expect("running");
        assert_eq!(session.state(), SessionState::Spawning);
        assert_eq!(running.state(), SessionState::Running);

        let torn = running
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("recorded");
        assert_eq!(
            running.teardown().at(StepKind::CredentialsRevoked),
            None,
            "the session the step was applied to is unchanged"
        );
        assert_eq!(
            torn.teardown().at(StepKind::CredentialsRevoked),
            Some(at(10))
        );
    }

    // ---------------------------------------------------------------------
    // Outcomes are told apart
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0030_a_session_that_completes_is_not_reported_killed() {
        let ended = spawning()
            .running(at(1))
            .expect("running")
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("first")
            .record(TeardownStep::ProcessStopped, at(11))
            .expect("second")
            .record(TeardownStep::WorktreeReleased(Disposition::Removed), at(12))
            .expect("third")
            .end(Outcome::Completed, at(13))
            .expect("a completed session needs no lock release");

        assert_eq!(ended.outcome(), Some(Outcome::Completed));
        assert_ne!(ended.outcome(), Some(Outcome::Killed));
        assert_eq!(ended.state(), SessionState::Ended(Outcome::Completed));
        assert!(!ended.outcome().expect("ended").was_killed());
    }

    #[test]
    fn ori_t_0030_each_of_the_four_outcomes_is_reachable_and_is_itself() {
        for outcome in Outcome::ALL {
            let session = spawning().running(at(1)).expect("running");
            let torn = session
                .record(TeardownStep::CredentialsRevoked, at(10))
                .expect("first")
                .record(TeardownStep::ProcessStopped, at(11))
                .expect("second");
            let torn = if outcome.was_killed() {
                torn.record(TeardownStep::LocksReleased, at(12))
                    .expect("a kill hands the modules back")
            } else {
                torn
            };
            let ended = torn
                .record(TeardownStep::WorktreeReleased(Disposition::Removed), at(13))
                .expect("fourth")
                .end(*outcome, at(14))
                .expect("the session ends");
            assert_eq!(ended.outcome(), Some(*outcome));
            assert_eq!(ended.state(), SessionState::Ended(*outcome));
            for other in Outcome::ALL {
                assert_eq!(
                    ended.outcome() == Some(*other),
                    outcome == other,
                    "{outcome} is not {other}"
                );
            }
        }
    }

    #[test]
    fn ori_t_0030_a_completed_session_keeps_the_lock_its_ticket_still_holds() {
        // ORI-P1-008: the second ticket starts "after the first closes", not
        // after the first coder's session ends. So the lock is outstanding and
        // it is not an orphan.
        let ended = spawning()
            .running(at(1))
            .expect("running")
            .record(TeardownStep::CredentialsRevoked, at(10))
            .expect("first")
            .record(TeardownStep::ProcessStopped, at(11))
            .expect("second")
            .record(TeardownStep::WorktreeReleased(Disposition::Removed), at(12))
            .expect("third")
            .end(Outcome::Completed, at(13))
            .expect("ended");

        assert_eq!(
            ended.outstanding(),
            vec![Held::Locks],
            "the lock entries are still there"
        );
        assert!(
            ended.residue().is_empty(),
            "and they belong to the ticket, not to the session that ended"
        );
    }

    #[test]
    fn ori_t_0030_the_four_outcomes_carry_the_spellings_the_data_model_fixes() {
        assert_eq!(Outcome::ALL.len(), 4);
        assert_eq!(
            Outcome::ALL
                .iter()
                .map(|outcome| outcome.as_str())
                .collect::<Vec<_>>(),
            vec!["completed", "blocked", "escalated", "killed"]
        );
        for outcome in Outcome::ALL {
            assert_eq!(
                outcome.as_str().parse::<Outcome>().expect("round trips"),
                *outcome
            );
        }
        assert!("finished".parse::<Outcome>().is_err());
        assert!("Killed".parse::<Outcome>().is_err(), "one spelling only");
    }

    #[test]
    fn ori_t_0030_the_four_resources_are_the_four_the_steps_release() {
        assert_eq!(StepKind::ALL.len(), Held::ALL.len());
        let released: Vec<Held> = StepKind::ALL.iter().map(|step| step.releases()).collect();
        assert_eq!(released, Held::ALL.to_vec(), "one step per resource");
    }

    // ---------------------------------------------------------------------
    // Refusals
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0030_every_refusal_this_module_makes_carries_a_reason_that_resolves() {
        let errors = [
            SessionError::Transition {
                from: SessionState::Spawning,
                to: SessionState::Ended(Outcome::Killed),
            },
            SessionError::StepOutOfOrder {
                step: StepKind::ProcessStopped,
                earlier: StepKind::CredentialsRevoked,
            },
            SessionError::StepNotAfter {
                step: StepKind::ProcessStopped,
                at: at(1),
                earlier: StepKind::CredentialsRevoked,
                recorded: at(2),
            },
            SessionError::StepRepeated {
                step: StepKind::LocksReleased,
            },
            SessionError::StepOutsideRunning {
                step: StepKind::LocksReleased,
                state: SessionState::Spawning,
            },
            SessionError::Incomplete {
                outcome: Outcome::Killed,
                missing: StepKind::WorktreeReleased,
            },
            SessionError::EndBeforeStep {
                step: StepKind::ProcessStopped,
                at: at(1),
                recorded: at(2),
            },
            SessionError::BeforeStart {
                at: at(-1),
                started_at: START,
            },
            SessionError::WorktreeOfAnotherSession {
                session: id("G5FAV"),
                worktree: id("G5FAW"),
            },
            SessionError::EmptyContainerId,
            SessionError::NotAnOutcome {
                text: "done".to_owned(),
            },
        ];
        let mut refusals = 0;
        for error in errors {
            assert!(!error.to_string().is_empty());
            match error.methodology_ref() {
                Some(reason) => {
                    assert!(error.is_refusal(), "a reason belongs to a refusal: {error}");
                    assert!(reason.resolves(), "{reason} resolves in the methodology");
                    refusals += 1;
                }
                None => assert!(!error.is_refusal(), "a refusal carries a reason: {error}"),
            }
        }
        assert_eq!(refusals, 9, "nine of the eleven are controls refusing");
    }

    #[test]
    fn ori_t_0030_each_step_cites_the_section_its_own_rule_comes_from() {
        // Four steps, four sections, written out so that a change to one is a
        // change a reviewer sees rather than a silent re-citation.
        let expected = [
            (StepKind::CredentialsRevoked, 27u8),
            (StepKind::ProcessStopped, 17),
            (StepKind::LocksReleased, 12),
            (StepKind::WorktreeReleased, 7),
        ];
        for (step, section) in expected {
            assert_eq!(step.reason().section, section, "{step}");
            assert!(step.reason().resolves());
        }
    }

    #[test]
    fn ori_t_0030_a_worktree_belonging_to_another_session_is_refused() {
        let mine = id("G5FAV");
        let theirs = id("G5FAW");
        let worktree = Worktree::new(theirs.clone(), "/nowhere/theirs", "feat/a").expect("valid");
        let refusal = Session::spawning(mine.clone(), id("G5FAX"), None, worktree, START)
            .expect_err("a worktree is one session's");
        assert_eq!(
            refusal,
            SessionError::WorktreeOfAnotherSession {
                session: mine,
                worktree: theirs,
            }
        );
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(7)
        );
    }

    #[test]
    fn ori_t_0030_an_unattended_session_has_no_ticket_and_a_container_is_optional() {
        let session = id("G5FAV");
        let worktree = Worktree::new(session.clone(), "/nowhere/one", "feat/a").expect("valid");
        let unattended = Session::spawning(session, id("G5FAW"), None, worktree, START)
            .expect("spec/DATA_MODEL.md section 2 makes the ticket nullable");
        assert!(unattended.ticket().is_none());
        assert!(unattended.container().is_none(), "worktree only by default");

        let contained = unattended.in_container("ori-session-1").expect("recorded");
        assert_eq!(contained.container(), Some("ori-session-1"));
        assert!(
            unattended.in_container("  ").is_err(),
            "an empty container identifier is not a container"
        );
    }

    #[test]
    fn ori_t_0030_a_transition_before_the_session_started_is_refused() {
        let refusal = spawning()
            .running(Timestamp::from_millis(START.millis() - 1))
            .expect_err("time does not run backwards");
        assert!(
            matches!(refusal, SessionError::BeforeStart { .. }),
            "{refusal}"
        );
    }
}
