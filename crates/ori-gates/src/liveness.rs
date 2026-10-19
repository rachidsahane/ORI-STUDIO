//! The liveness definition: AICD §14, `spec/PRD.md` section 4.20 (Z-02) and
//! section 4.11 (O-06), `spec/CI_CD.md` section 1.
//!
//! `spec/CI_CD.md` section 1 closes its list of gates with the sentence "A
//! pipeline that does not run is a failure: the liveness gate (scheduled) fails
//! loudly if any required workflow has not executed inside its window."
//! `spec/PRD.md` O-06 says the same of "an expected workflow or agent run" and
//! adds "inert-but-present is a failure state". AICD §14 names the defect class
//! both come from, "present but reporting nothing", and gives one of its three
//! examples as "a workflow file whose trigger never fires".
//!
//! This module is the **definition** of that check and nothing more: pure logic
//! over inputs the caller supplies. It reads no file, no store, no process
//! table and no clock. Gathering the records, scheduling the sweep, writing a
//! `missing` run and opening the incident are the phase 4 caller's work; this
//! is the part that decides, so that phase 4 can assert it.
//!
//! # The decisions, stated once
//!
//! 1. **An expected run** is a [`crate::liveness::RunKind`] (a workflow or an
//!    agent run), a name, and a [`crate::liveness::Window`]. The name is the
//!    key: an expectation matches the observed runs carrying exactly that name,
//!    byte for byte, and no others. A name that is empty or blank is refused,
//!    because an expectation about nothing matches nothing and would then be
//!    "missing" for a reason no reader could act on.
//! 2. **A window** is a positive number of seconds, counted back from "now".
//!    It is the longest silence the expectation tolerates, not the schedule's
//!    period: a daily job wants the period plus whatever lateness the scheduler
//!    is allowed, which is the caller's choice and is written down where the
//!    expectation is declared. A window of zero or less is refused, because it
//!    would make every expectation missing or only the instantaneous one live.
//!    The window is **closed at both ends**: a run that started exactly one
//!    window ago is inside it, and one second earlier is outside. Windows
//!    trail "now"; a fixed slot ("between 02:00 and 03:00 each night") is not
//!    modelled here.
//! 3. **Time is an argument.** Every decision takes `now` as Unix seconds UTC,
//!    the unit [`crate::gate::GateProof`] already uses. Nothing here reads the
//!    wall clock, so a test, and a replay of a past sweep, give the same answer
//!    every time. A test in this module reads this file's own source to keep it
//!    that way.
//! 4. **An observed run** is a name, a start time and a
//!    [`crate::liveness::ObservedStatus`]. The start time is the one counted,
//!    not the finish: liveness asks whether the trigger fired, which a run still
//!    in progress already shows, and a run that never finishes is that run's own
//!    gate's failure, not an absence. The status mirrors `GateRun.status` in
//!    `spec/DATA_MODEL.md` section 2: `pass`, `fail`, `error`, `missing`.
//!    A run that failed or errored **did execute**, and is live; whether it
//!    passed is the gate's own verdict, not this one's. A record of status
//!    `missing` is a record of **absence** and is never evidence of execution.
//!    That last rule is load-bearing: `spec/DATA_MODEL.md` records the inert
//!    case as a `GateRun` with status `missing`, and the likeliest writer of
//!    such a record is the sweep that found the absence. A definition that
//!    counted any record would be satisfied by its own previous finding, and
//!    the second sweep would call the dead workflow live. (That the sweep
//!    writes the record is this module's inference, not the data model's
//!    words; the rule holds whoever writes it.) A caller whose source has
//!    statuses outside the four maps them first, and maps anything that did
//!    not actually start to `missing`.
//! 5. **No record at all is missing, never live.** An empty list of observed
//!    runs, or a list holding only runs under other names, is the case AICD §14
//!    calls "present but reporting nothing", and the one this module most needs
//!    to get right: an implementation that treats "nothing found" as "nothing
//!    wrong" passes on exactly the repository the gate exists for. The same
//!    holds one level up: an assessment of **no expectations at all** is
//!    refused as [`crate::liveness::LivenessError::NothingExpected`], never
//!    returned as a clean report, because a liveness gate that checked nothing
//!    and passed is itself present but reporting nothing.
//! 6. **A run dated after "now" is not counted.** At the moment of the
//!    decision it has not happened. A record from the future means the
//!    recorder's clock is ahead of the decider's, or the stamp is wrong, or it
//!    was written by hand; whichever, counting it would let one bad timestamp
//!    keep a dead workflow green until the clock caught up, or for ever. It is
//!    set aside, and [`crate::liveness::Judgement::future_ignored`] says how
//!    many were, so a human can see why. A caller that wants to tolerate skew
//!    passes a later `now`; this module adds no tolerance of its own. Only a
//!    record that claims an execution is counted there: a future-dated record
//!    of status `missing` claims nothing was run, so it is not "a run set
//!    aside" and does not raise the count. It is still a record under that
//!    name, and it still makes the reason "only records of absence" rather
//!    than "no record at all".
//! 7. **An installed gate with a missing run is inert.** `spec/DATA_MODEL.md`
//!    section 3: "A GateRun with status `missing` on an installed gate flips it
//!    to `Inert`, which is an incident." An expectation may be bound to a
//!    gate with [`crate::liveness::ExpectedRun::for_gate`]; if the bound
//!    [`crate::runner::GateState`] is `Installed` and the run is missing, the
//!    verdict is [`crate::liveness::Verdict::Inert`] rather than
//!    [`crate::liveness::Verdict::Missing`]. A gate that is only `Defined` or
//!    `Proven` is not installed, so the data model's transition to `Inert` does
//!    not apply to it: its missing run is a plain `Missing`, still a failure of
//!    this check, and not an incident about a gate that never took service.
//! 8. **Every failure carries AICD §14.** [`crate::liveness::Verdict::reason`]
//!    and [`crate::liveness::LivenessError::reason`] give the
//!    [`ori_core::error::MethodologyRef`] (`CLAUDE.md` absolute rule 9), and
//!    every rendered line for a non-live verdict names the section in words.
//!
//! ```mermaid
//! flowchart TB
//!   E["ExpectedRun: kind, name, window, optional gate"] --> J[judge]
//!   O["ObservedRun list, supplied by the caller"] --> J
//!   N["now, supplied by the caller"] --> J
//!   J --> L["Verdict::Live"]
//!   J --> M["Verdict::Missing, with a MissingReason"]
//!   J --> I["Verdict::Inert, installed gate only"]
//!   M --> F["the liveness check fails loudly"]
//!   I --> F
//!   I --> INC["an incident"]
//! ```
//!
//! # How this relates to `GateState` in `runner.rs`
//!
//! [`crate::runner::GateState`] has three values, `Defined`, `Proven` and
//! `Installed`. `spec/DATA_MODEL.md` section 2 gives `Gate.state` four, the
//! fourth being `inert`, and [`crate::gate`]'s head comment records that
//! whoever wires a gate's real execution history owes it. This module is the
//! rule that decides when a gate has earned it, and it does **not** add the
//! value: `runner.rs` is outside this ticket's scope. What it provides instead
//! is the fact the fourth state is reached by, [`crate::liveness::Verdict::Inert`],
//! carrying the gate's number through [`crate::liveness::ExpectedRun::gate`].
//! The ticket that adds `GateState::Inert` to `runner.rs` takes the report this
//! module returns as an input; until then an installed gate that has gone
//! silent is reported here and nowhere else, and `Registry::list` still says
//! `Installed` for it. That is a real gap, and this module cannot close it from
//! where it stands.
//!
//! This module only ever **reads** a [`crate::runner::GateState`]. It never
//! produces `Installed`, so it cannot make this crate report a gate installed
//! without a proof (`spec/LLD.md` section 2).
//!
//! # Why this is not a `Runner`
//!
//! [`crate::runner::Runner::number`] answers with a [`crate::gate::GateNumber`],
//! one to fourteen, and [`crate::gate::NUMBERED_GATE_COUNT`] explains that the
//! liveness gate is not one of the fourteen. It has no number to answer with.
//! [`crate::liveness::Report::outcome`] and [`crate::liveness::outcome_of`] give
//! the mapping onto [`crate::runner::Outcome`] that a wrapper would use: every
//! expectation live is `Passed`, any other verdict is `Failed`, and a
//! [`crate::liveness::LivenessError`] is `Blocked`, never `Passed`, in the way
//! the `Runner` trait's own documentation requires of an implementation.
//!
//! # What this module does not do
//!
//! It does not read the repository, the event log or a scheduler; it does not
//! write a `GateRun`; it does not open an incident; it does not check that
//! every document citing a gate names its trigger, the third part of
//! `spec/PRD.md` Z-02; and it carries no planted-defect proof of its own, so
//! no document may cite it as protection yet (AICD §14, and the same must-not
//! column). `fixtures/inert-gate` is the planted defect the phase 4 caller will
//! be proven against, and nothing here depends on it.
//!
//! # Why the links here are written `crate::liveness::`
//!
//! `lib.rs` carries a `///` on this module's declaration, so rustdoc resolves
//! this file's `//!` links in the crate root's scope rather than this
//! module's. [`crate::gate`] records the same trap. Do not shorten these.
//!
//! Must not: read the clock, do IO, or add a dependency (`CLAUDE.md` absolute
//! rule 6). Nothing outside `std`, `ori-core` and this crate's own modules is
//! used.

use std::error::Error;
use std::fmt;

use ori_core::error::MethodologyRef;

use crate::gate::GateNumber;
use crate::runner::{GateState, Outcome};

/// The methodology section every failure in this module is made under.
///
/// AICD §14: "present but reporting nothing". Built as a value rather than
/// through `MethodologyRef::at`, which is fallible, in the shape
/// `SignificanceError::reason` in `crates/ori-gates/src/significance.rs`
/// already uses.
const fn aicd_14() -> MethodologyRef {
    MethodologyRef {
        section: 14,
        subsection: None,
    }
}

/// Why this module refused an input: AICD §14.
///
/// Every variant is a refusal to build or assess something that could only
/// ever report nothing. In the vocabulary of [`crate::runner::Outcome`] a
/// caller maps every one of them to `Blocked`, never to `Passed`:
/// [`crate::liveness::outcome_of`] does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LivenessError {
    /// A window of zero seconds or fewer.
    NonPositiveWindow {
        /// The value that was refused.
        secs: i64,
    },
    /// An expectation whose name is empty or only whitespace.
    BlankName,
    /// An assessment was asked of no expectations at all.
    NothingExpected,
}

impl LivenessError {
    /// The methodology section this refusal rests on: AICD §14 (`CLAUDE.md`
    /// absolute rule 9).
    #[must_use]
    pub const fn reason(&self) -> MethodologyRef {
        aicd_14()
    }
}

impl fmt::Display for LivenessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveWindow { secs } => write!(
                f,
                "a window of {secs} seconds tolerates no silence at all, so it could only ever \
                 report nothing: a window must be at least one second (AICD §14)"
            ),
            Self::BlankName => f.write_str(
                "an expectation with an empty or blank name matches no run, so it would be \
                 missing for a reason nobody could act on (AICD §14)",
            ),
            Self::NothingExpected => f.write_str(
                "nothing is expected to have run, so a clean answer would mean nothing was \
                 checked: that is a liveness check present but reporting nothing (AICD §14)",
            ),
        }
    }
}

impl Error for LivenessError {}

/// How far back from "now" an expected run may be silent: AICD §14.
///
/// A positive number of seconds. The window is the closed interval from
/// `now - secs` to `now`: both ends are inside it. See this module's head
/// comment, decision 2.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Window {
    secs: i64,
}

impl Window {
    /// A window of `secs` seconds, refusing one that is not positive: AICD §14.
    ///
    /// # Errors
    ///
    /// [`crate::liveness::LivenessError::NonPositiveWindow`] when `secs` is
    /// zero or negative.
    pub const fn new(secs: i64) -> Result<Self, LivenessError> {
        if secs <= 0 {
            return Err(LivenessError::NonPositiveWindow { secs });
        }
        Ok(Self { secs })
    }

    /// The window's length in seconds.
    #[must_use]
    pub const fn secs(self) -> i64 {
        self.secs
    }

    /// The earliest start time still inside the window at `now`, Unix seconds
    /// UTC. A run that started at exactly this second is inside.
    ///
    /// Saturates rather than overflowing, so a `now` near the bottom of the
    /// range gives an answer instead of a panic.
    #[must_use]
    pub const fn opens_at(self, now: i64) -> i64 {
        now.saturating_sub(self.secs)
    }
}

/// What kind of thing is expected to have run: `spec/PRD.md` section 4.11, O-06
/// ("an expected workflow or agent run").
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RunKind {
    /// A CI workflow, `spec/CI_CD.md` section 1.
    Workflow,
    /// A scheduled or triggered agent run, such as a watcher.
    AgentRun,
}

impl fmt::Display for RunKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workflow => f.write_str("workflow"),
            Self::AgentRun => f.write_str("agent run"),
        }
    }
}

/// The gate an expectation is bound to, and the state the caller holds it in:
/// `spec/DATA_MODEL.md` section 3.
///
/// The state is read from [`crate::runner::GateState`], whose `Installed` value
/// only `Registry::list` produces. This type never produces one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GateBinding {
    /// The gate's position in `spec/CI_CD.md` section 1's numbered pipeline.
    pub number: GateNumber,
    /// The state the caller holds the gate in.
    pub state: GateState,
}

impl GateBinding {
    /// Whether a missing run on this gate is an incident: only for
    /// `Installed`, per `spec/DATA_MODEL.md` section 3.
    ///
    /// An exhaustive `match` on purpose, with no wildcard arm. When a fourth
    /// [`crate::runner::GateState`] value (`Inert`, which this module's head
    /// comment says is owed) is added, this stops compiling until its author
    /// decides what a missing run means for a gate already in that state. A
    /// `matches!` here would have kept compiling and silently answered
    /// "not installed" for it.
    #[must_use]
    pub const fn is_installed(&self) -> bool {
        match self.state {
            GateState::Installed => true,
            GateState::Defined | GateState::Proven => false,
        }
    }
}

/// One workflow or agent run that is expected to have happened inside its
/// window: AICD §14, `spec/PRD.md` O-06.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedRun {
    kind: RunKind,
    name: String,
    window: Window,
    gate: Option<GateBinding>,
}

impl ExpectedRun {
    /// An expectation, refusing a blank name: AICD §14.
    ///
    /// # Errors
    ///
    /// [`crate::liveness::LivenessError::BlankName`] when `name` is empty or
    /// only whitespace.
    pub fn new(
        kind: RunKind,
        name: impl Into<String>,
        window: Window,
    ) -> Result<Self, LivenessError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(LivenessError::BlankName);
        }
        Ok(Self {
            kind,
            name,
            window,
            gate: None,
        })
    }

    /// A workflow expectation: shorthand for [`ExpectedRun::new`] with
    /// [`RunKind::Workflow`].
    ///
    /// # Errors
    ///
    /// As [`ExpectedRun::new`].
    pub fn workflow(name: impl Into<String>, window: Window) -> Result<Self, LivenessError> {
        Self::new(RunKind::Workflow, name, window)
    }

    /// An agent-run expectation: shorthand for [`ExpectedRun::new`] with
    /// [`RunKind::AgentRun`].
    ///
    /// # Errors
    ///
    /// As [`ExpectedRun::new`].
    pub fn agent_run(name: impl Into<String>, window: Window) -> Result<Self, LivenessError> {
        Self::new(RunKind::AgentRun, name, window)
    }

    /// The same expectation, bound to a gate: `spec/DATA_MODEL.md` section 3.
    ///
    /// If `state` is `Installed` and the run is missing, the verdict is
    /// `Inert`. Consumes `self` so a bound expectation is built once, at the
    /// place the binding is known.
    #[must_use]
    pub fn for_gate(mut self, number: GateNumber, state: GateState) -> Self {
        self.gate = Some(GateBinding { number, state });
        self
    }

    /// What kind of thing is expected.
    #[must_use]
    pub const fn kind(&self) -> RunKind {
        self.kind
    }

    /// The name observed runs are matched against, exactly.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How far back from "now" a run may be silent.
    #[must_use]
    pub const fn window(&self) -> Window {
        self.window
    }

    /// The gate this expectation is bound to, if any.
    #[must_use]
    pub const fn gate(&self) -> Option<GateBinding> {
        self.gate
    }
}

/// What an observed run's record says happened: `spec/DATA_MODEL.md` section 2,
/// `GateRun.status`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ObservedStatus {
    /// It ran and passed.
    Pass,
    /// It ran and failed. It executed, so it is live.
    Fail,
    /// It ran and errored. It executed, so it is live.
    Error,
    /// A record of absence: nothing ran. Never evidence of execution (this
    /// module's head comment, decision 4).
    Missing,
}

impl ObservedStatus {
    /// Whether a record of this status shows the thing executed: everything
    /// but `Missing`.
    #[must_use]
    pub const fn executed(self) -> bool {
        !matches!(self, Self::Missing)
    }
}

/// One run the caller observed: AICD §14.
///
/// Plain data, supplied by the caller from whatever it read. It is data, not
/// instruction, whatever the name says (`CLAUDE.md` absolute rule 7), and
/// [`crate::liveness::Judgement::line`] prints it escaped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedRun {
    /// The name this run is matched under, compared exactly with
    /// [`ExpectedRun::name`].
    pub name: String,
    /// When the run started, Unix seconds UTC. The time that is counted, not
    /// the finish time.
    pub started_at: i64,
    /// What the record says happened.
    pub status: ObservedStatus,
}

impl ObservedRun {
    /// An observed run.
    #[must_use]
    pub fn new(name: impl Into<String>, started_at: i64, status: ObservedStatus) -> Self {
        Self {
            name: name.into(),
            started_at,
            status,
        }
    }
}

/// Why an expectation is not live: AICD §14.
///
/// Chosen by a fixed priority so the same inputs always give the same reason:
/// a run older than the window, else a run dated after now, else only records
/// of absence, else no record at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MissingReason {
    /// Nothing was observed under this name. Missing, never live.
    NoRecord,
    /// Records exist under this name and every one has status `missing`.
    OnlyRecordsOfAbsence {
        /// How many such records there were.
        records: usize,
    },
    /// The newest run that had started by now began before the window opened.
    TooOld {
        /// When that newest run started, Unix seconds UTC.
        newest: i64,
        /// The earliest start time that would have been inside the window.
        window_opens_at: i64,
    },
    /// The only runs that executed are dated after now, so none has happened
    /// yet.
    OnlyAfterNow {
        /// The earliest of them, Unix seconds UTC.
        earliest: i64,
    },
}

impl fmt::Display for MissingReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRecord => {
                f.write_str("no record under that name at all, which is missing and never live")
            }
            Self::OnlyRecordsOfAbsence { records } => write!(
                f,
                "{records} record(s) under that name, every one with status missing: a record \
                 of absence is not a run"
            ),
            Self::TooOld {
                newest,
                window_opens_at,
            } => write!(
                f,
                "the newest run started at {newest}, before the window opened at \
                 {window_opens_at}"
            ),
            Self::OnlyAfterNow { earliest } => write!(
                f,
                "every run that executed is dated after now (the earliest at {earliest}), and \
                 a run that has not happened yet is not a run"
            ),
        }
    }
}

/// What the definition decides for one expectation: AICD §14,
/// `spec/DATA_MODEL.md` section 3.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// A run that executed started inside the window.
    Live {
        /// The newest such run's start time, Unix seconds UTC.
        last_run_at: i64,
    },
    /// No run inside the window. A failure of the liveness check.
    Missing(MissingReason),
    /// No run inside the window, on an installed gate: it has gone inert,
    /// which is an incident (`spec/DATA_MODEL.md` section 3).
    Inert(MissingReason),
}

impl Verdict {
    /// Whether this is the one verdict that is not a failure.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self, Self::Live { .. })
    }

    /// Whether this is an incident, not only a failure: an installed gate that
    /// went silent.
    #[must_use]
    pub const fn is_incident(&self) -> bool {
        matches!(self, Self::Inert(_))
    }

    /// Why the run is missing, for either failing verdict; `None` when live.
    #[must_use]
    pub const fn missing_reason(&self) -> Option<&MissingReason> {
        match self {
            Self::Live { .. } => None,
            Self::Missing(reason) | Self::Inert(reason) => Some(reason),
        }
    }

    /// The methodology section a failing verdict is made under: AICD §14
    /// (`CLAUDE.md` absolute rule 9). `None` when live, which refuses nothing.
    #[must_use]
    pub const fn reason(&self) -> Option<MethodologyRef> {
        match self {
            Self::Live { .. } => None,
            Self::Missing(_) | Self::Inert(_) => Some(aicd_14()),
        }
    }
}

/// One expectation and what was decided for it: AICD §14.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Judgement {
    expected: ExpectedRun,
    now: i64,
    verdict: Verdict,
    future_ignored: usize,
}

impl Judgement {
    /// The expectation that was judged.
    #[must_use]
    pub const fn expected(&self) -> &ExpectedRun {
        &self.expected
    }

    /// The decision.
    #[must_use]
    pub const fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    /// How many records under this name that claim an execution were dated
    /// after "now" and so were not counted: this module's head comment,
    /// decision 6. A future-dated record of status `missing` claims no
    /// execution and is not counted here.
    #[must_use]
    pub const fn future_ignored(&self) -> usize {
        self.future_ignored
    }

    /// Whether the expectation is live.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.verdict.is_live()
    }

    /// Whether the expectation is an incident.
    #[must_use]
    pub const fn is_incident(&self) -> bool {
        self.verdict.is_incident()
    }

    /// One line saying what was decided and why, for the pull request report or
    /// the notification a human reads.
    ///
    /// The name is printed escaped (`{:?}`): it came from the caller's source,
    /// and a name holding a line break or an instruction is data
    /// (`CLAUDE.md` absolute rule 7), never a second line of this output.
    #[must_use]
    pub fn line(&self) -> String {
        let name = self.expected.name();
        let kind = self.expected.kind();
        let window = self.expected.window().secs();
        let ignored = if self.future_ignored == 0 {
            String::new()
        } else {
            format!(
                "; {} record(s) dated after now were not counted",
                self.future_ignored
            )
        };
        match &self.verdict {
            Verdict::Live { last_run_at } => format!(
                "live: {kind} {name:?} last started at {last_run_at}, {age} s before now, inside \
                 its {window} s window{ignored}",
                age = self.now.saturating_sub(*last_run_at),
            ),
            Verdict::Missing(reason) => format!(
                "MISSING: {kind} {name:?}: {reason}; it was expected to start within {window} s \
                 of now ({now}){ignored} (AICD §14)",
                now = self.now,
            ),
            Verdict::Inert(reason) => {
                let gate = self
                    .expected
                    .gate()
                    .map_or_else(String::new, |binding| format!("{} ", binding.number));
                format!(
                    "INERT, an incident: installed {gate}{kind} {name:?}: {reason}; it was \
                     expected to start within {window} s of now ({now}){ignored} (AICD §14)",
                    now = self.now,
                )
            }
        }
    }
}

/// Decide whether one expected run happened inside its window: AICD §14.
///
/// A pure function of its three inputs. `now` is Unix seconds UTC and is the
/// only source of time; see this module's head comment, decision 3.
///
/// A run counts when its name equals the expectation's, its status shows it
/// executed, and its start time is in `[now - window, now]`. Any run that counts
/// in that sense makes the verdict [`Verdict::Live`], reporting the newest one.
/// Otherwise the verdict is [`Verdict::Missing`], or [`Verdict::Inert`] when the
/// expectation is bound to an installed gate, and the [`MissingReason`] says
/// which way it was missing.
#[must_use]
pub fn judge(expected: &ExpectedRun, observed: &[ObservedRun], now: i64) -> Judgement {
    let opens_at = expected.window().opens_at(now);

    let mut matching: usize = 0;
    let mut newest_inside: Option<i64> = None;
    let mut newest_before_window: Option<i64> = None;
    let mut earliest_after_now: Option<i64> = None;
    let mut future_ignored: usize = 0;

    for run in observed.iter().filter(|run| run.name == expected.name()) {
        matching += 1;
        if !run.status.executed() {
            continue;
        }
        if run.started_at > now {
            future_ignored += 1;
            earliest_after_now = Some(
                earliest_after_now.map_or(run.started_at, |current| current.min(run.started_at)),
            );
        } else if run.started_at < opens_at {
            newest_before_window = Some(
                newest_before_window.map_or(run.started_at, |current| current.max(run.started_at)),
            );
        } else {
            newest_inside =
                Some(newest_inside.map_or(run.started_at, |current| current.max(run.started_at)));
        }
    }

    let verdict = if let Some(last_run_at) = newest_inside {
        Verdict::Live { last_run_at }
    } else {
        let reason = if let Some(newest) = newest_before_window {
            MissingReason::TooOld {
                newest,
                window_opens_at: opens_at,
            }
        } else if let Some(earliest) = earliest_after_now {
            MissingReason::OnlyAfterNow { earliest }
        } else if matching > 0 {
            MissingReason::OnlyRecordsOfAbsence { records: matching }
        } else {
            MissingReason::NoRecord
        };
        if expected
            .gate()
            .is_some_and(|binding| binding.is_installed())
        {
            Verdict::Inert(reason)
        } else {
            Verdict::Missing(reason)
        }
    };

    Judgement {
        expected: expected.clone(),
        now,
        verdict,
        future_ignored,
    }
}

/// The judgements for every expectation of one sweep: AICD §14.
///
/// Built only by [`crate::liveness::assess`], which refuses an empty sweep, so
/// a report always holds at least one judgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Report {
    now: i64,
    judgements: Vec<Judgement>,
}

impl Report {
    /// The "now" the sweep was decided at, Unix seconds UTC.
    #[must_use]
    pub const fn now(&self) -> i64 {
        self.now
    }

    /// One judgement per expectation, in the order the expectations were given.
    #[must_use]
    pub fn judgements(&self) -> &[Judgement] {
        &self.judgements
    }

    /// Whether every expectation is live.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.judgements.iter().all(Judgement::is_live)
    }

    /// The judgements that are not live: failures, incidents included.
    pub fn failures(&self) -> impl Iterator<Item = &Judgement> {
        self.judgements.iter().filter(|j| !j.is_live())
    }

    /// The judgements that are incidents: installed gates gone inert.
    pub fn incidents(&self) -> impl Iterator<Item = &Judgement> {
        self.judgements.iter().filter(|j| j.is_incident())
    }

    /// The runner vocabulary for this report: `Passed` when every expectation
    /// is live, `Failed` otherwise. See this module's head comment, "Why this
    /// is not a `Runner`".
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.is_live() {
            Outcome::Passed
        } else {
            Outcome::Failed
        }
    }

    /// One line per judgement, in order: [`Judgement::line`].
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.judgements.iter().map(Judgement::line).collect()
    }
}

/// Judge every expectation of one sweep at `now`, refusing an empty sweep:
/// AICD §14.
///
/// Each expectation is judged independently against the whole of `observed`.
/// Two expectations with the same name are two judgements; nothing is merged or
/// deduplicated.
///
/// # Errors
///
/// [`crate::liveness::LivenessError::NothingExpected`] when `expected` is
/// empty: a check of nothing is not a pass.
pub fn assess(
    expected: &[ExpectedRun],
    observed: &[ObservedRun],
    now: i64,
) -> Result<Report, LivenessError> {
    if expected.is_empty() {
        return Err(LivenessError::NothingExpected);
    }
    Ok(Report {
        now,
        judgements: expected
            .iter()
            .map(|one| judge(one, observed, now))
            .collect(),
    })
}

/// The runner vocabulary for the result of [`crate::liveness::assess`]: AICD §14.
///
/// `Passed` only when every expectation is live; `Failed` when any is not;
/// `Blocked` on any [`crate::liveness::LivenessError`], never `Passed`, as
/// [`crate::runner::Runner::run`] requires of an implementation.
#[must_use]
pub fn outcome_of(result: &Result<Report, LivenessError>) -> Outcome {
    match result {
        Ok(report) => report.outcome(),
        Err(_) => Outcome::Blocked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000;
    const DAY: i64 = 86_400;

    fn day() -> Window {
        Window::new(DAY).expect("a day is a positive window")
    }

    fn wf(name: &str) -> ExpectedRun {
        ExpectedRun::workflow(name, day()).expect("the name is not blank")
    }

    fn ran(name: &str, started_at: i64) -> ObservedRun {
        ObservedRun::new(name, started_at, ObservedStatus::Pass)
    }

    fn gate(n: u8) -> GateNumber {
        GateNumber::new(n).expect("a numbered gate")
    }

    #[test]
    fn ori_t_0046_a_run_inside_its_window_is_live() {
        let judged = judge(&wf("ci.yml"), &[ran("ci.yml", NOW - 3_600)], NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Live {
                last_run_at: NOW - 3_600
            }
        );
        assert!(judged.is_live());
        assert!(!judged.is_incident());
    }

    #[test]
    fn ori_t_0046_the_newest_run_inside_the_window_is_the_one_reported() {
        let observed = [
            ran("ci.yml", NOW - 9_000),
            ran("ci.yml", NOW - 600),
            ran("ci.yml", NOW - 5_000),
        ];
        let judged = judge(&wf("ci.yml"), &observed, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Live {
                last_run_at: NOW - 600
            }
        );
    }

    #[test]
    fn ori_t_0046_no_run_inside_the_window_is_missing() {
        // A run exists, but only for a different workflow, and the right one's
        // only record is a day and a half old.
        let observed = [
            ran("other.yml", NOW - 60),
            ran("ci.yml", NOW - DAY - DAY / 2),
        ];
        let judged = judge(&wf("ci.yml"), &observed, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::TooOld {
                newest: NOW - DAY - DAY / 2,
                window_opens_at: NOW - DAY,
            })
        );
        assert!(!judged.is_live());
    }

    #[test]
    fn ori_t_0046_no_record_at_all_is_missing_never_live() {
        let empty = judge(&wf("ci.yml"), &[], NOW);
        assert_eq!(empty.verdict(), &Verdict::Missing(MissingReason::NoRecord));
        assert!(!empty.is_live(), "nothing found is not nothing wrong");

        // Records exist, none under this name: the same absence.
        let elsewhere = judge(&wf("ci.yml"), &[ran("release.yml", NOW - 10)], NOW);
        assert_eq!(
            elsewhere.verdict(),
            &Verdict::Missing(MissingReason::NoRecord)
        );
    }

    #[test]
    fn ori_t_0046_a_run_just_outside_the_window_is_missing_and_the_edge_is_inside() {
        let on_the_edge = judge(&wf("ci.yml"), &[ran("ci.yml", NOW - DAY)], NOW);
        assert!(
            on_the_edge.is_live(),
            "the window is closed: a run exactly one window ago is inside"
        );

        let one_second_late = judge(&wf("ci.yml"), &[ran("ci.yml", NOW - DAY - 1)], NOW);
        assert_eq!(
            one_second_late.verdict(),
            &Verdict::Missing(MissingReason::TooOld {
                newest: NOW - DAY - 1,
                window_opens_at: NOW - DAY,
            }),
            "one second before the window opens is outside"
        );

        let at_now = judge(&wf("ci.yml"), &[ran("ci.yml", NOW)], NOW);
        assert!(at_now.is_live(), "the window is closed at now as well");
    }

    #[test]
    fn ori_t_0046_an_installed_gate_with_a_missing_run_is_reported_inert() {
        let expected = wf("gate-4.yml").for_gate(gate(4), GateState::Installed);
        let judged = judge(&expected, &[], NOW);
        assert_eq!(judged.verdict(), &Verdict::Inert(MissingReason::NoRecord));
        assert!(
            judged.is_incident(),
            "inert is an incident, not only a miss"
        );
        assert!(!judged.is_live());
    }

    #[test]
    fn ori_t_0046_an_installed_gate_with_a_live_run_is_live_not_inert() {
        let expected = wf("gate-4.yml").for_gate(gate(4), GateState::Installed);
        let judged = judge(&expected, &[ran("gate-4.yml", NOW - 5)], NOW);
        assert!(judged.is_live());
        assert!(!judged.is_incident());
    }

    #[test]
    fn ori_t_0046_a_gate_that_is_not_installed_is_missing_not_inert() {
        for state in [GateState::Defined, GateState::Proven] {
            let expected = wf("gate-4.yml").for_gate(gate(4), state);
            let judged = judge(&expected, &[], NOW);
            assert_eq!(
                judged.verdict(),
                &Verdict::Missing(MissingReason::NoRecord),
                "a gate in state {state} never took service, so it cannot go inert"
            );
            assert!(!judged.is_incident());
            assert!(!judged.is_live(), "it is still a failure of the check");
        }
    }

    #[test]
    fn ori_t_0046_a_run_in_the_future_is_not_counted_as_live() {
        // Why: at the moment of the decision a run dated after now has not
        // happened. Counting it would let one wrong stamp keep a dead workflow
        // green until the clock caught up.
        let judged = judge(&wf("ci.yml"), &[ran("ci.yml", NOW + 60)], NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::OnlyAfterNow { earliest: NOW + 60 })
        );
        assert_eq!(judged.future_ignored(), 1);
        assert!(!judged.is_live());

        // A year ahead does not vouch for a workflow that last ran two days ago.
        let observed = [ran("ci.yml", NOW + 365 * DAY), ran("ci.yml", NOW - 2 * DAY)];
        let judged = judge(&wf("ci.yml"), &observed, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::TooOld {
                newest: NOW - 2 * DAY,
                window_opens_at: NOW - DAY,
            })
        );
        assert_eq!(judged.future_ignored(), 1);
    }

    #[test]
    fn ori_t_0046_a_future_record_is_set_aside_but_a_real_run_beside_it_still_counts() {
        let observed = [ran("ci.yml", NOW + DAY), ran("ci.yml", NOW - 100)];
        let judged = judge(&wf("ci.yml"), &observed, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Live {
                last_run_at: NOW - 100
            }
        );
        assert_eq!(judged.future_ignored(), 1);
        assert!(
            judged
                .line()
                .contains("1 record(s) dated after now were not counted"),
            "a human must be able to see a record was set aside: {}",
            judged.line()
        );
    }

    #[test]
    fn ori_t_0046_a_stale_reason_names_the_newest_old_run_and_a_future_reason_the_earliest() {
        let stale = [
            ran("ci.yml", NOW - 5 * DAY),
            ran("ci.yml", NOW - 2 * DAY),
            ran("ci.yml", NOW - 9 * DAY),
        ];
        assert_eq!(
            judge(&wf("ci.yml"), &stale, NOW).verdict(),
            &Verdict::Missing(MissingReason::TooOld {
                newest: NOW - 2 * DAY,
                window_opens_at: NOW - DAY,
            }),
            "the reason must name how recently it last ran, not how long ago it first did"
        );

        let ahead = [
            ran("ci.yml", NOW + 7 * DAY),
            ran("ci.yml", NOW + 60),
            ran("ci.yml", NOW + DAY),
        ];
        let judged = judge(&wf("ci.yml"), &ahead, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::OnlyAfterNow { earliest: NOW + 60 })
        );
        assert_eq!(judged.future_ignored(), 3);
    }

    #[test]
    fn ori_t_0046_a_record_of_absence_is_not_a_run() {
        // A sweep that finds nothing may itself write a `missing` run. The
        // next sweep must not take that record as proof the thing ran.
        let observed = [ObservedRun::new(
            "ci.yml",
            NOW - 10,
            ObservedStatus::Missing,
        )];
        let judged = judge(&wf("ci.yml"), &observed, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::OnlyRecordsOfAbsence { records: 1 })
        );

        let installed = wf("gate-4.yml").for_gate(gate(4), GateState::Installed);
        let observed = [ObservedRun::new(
            "gate-4.yml",
            NOW - 10,
            ObservedStatus::Missing,
        )];
        assert!(
            judge(&installed, &observed, NOW).is_incident(),
            "a missing GateRun on an installed gate is the inert case"
        );
    }

    #[test]
    fn ori_t_0046_a_run_that_failed_or_errored_still_executed_and_is_live() {
        for status in [
            ObservedStatus::Pass,
            ObservedStatus::Fail,
            ObservedStatus::Error,
        ] {
            let observed = [ObservedRun::new("ci.yml", NOW - 10, status)];
            assert!(
                judge(&wf("ci.yml"), &observed, NOW).is_live(),
                "status {status:?} shows the trigger fired; whether it passed is the gate's verdict"
            );
        }
    }

    #[test]
    fn ori_t_0046_names_match_exactly_and_nothing_else_vouches() {
        for near in ["CI.yml", "ci.yml ", " ci.yml", "ci", "ci.yml.bak"] {
            let judged = judge(&wf("ci.yml"), &[ran(near, NOW - 10)], NOW);
            assert_eq!(
                judged.verdict(),
                &Verdict::Missing(MissingReason::NoRecord),
                "{near:?} is not ci.yml"
            );
        }
    }

    #[test]
    fn ori_t_0046_now_is_an_argument_so_the_same_run_ages_out() {
        let observed = [ran("ci.yml", NOW - 100)];
        let expected = wf("ci.yml");
        assert!(judge(&expected, &observed, NOW).is_live());
        assert!(
            judge(&expected, &observed, NOW).is_live(),
            "same inputs, same answer"
        );
        // The window closes DAY after the run started: DAY - 100 + NOW + 100.
        assert!(judge(&expected, &observed, NOW - 100 + DAY).is_live());
        assert!(!judge(&expected, &observed, NOW - 100 + DAY + 1).is_live());
        // And before the run started it is a run from the future.
        assert!(!judge(&expected, &observed, NOW - 101).is_live());
    }

    #[test]
    fn ori_t_0046_extreme_times_saturate_instead_of_panicking() {
        let window = day();
        assert_eq!(window.opens_at(i64::MIN), i64::MIN);
        assert_eq!(window.opens_at(i64::MAX), i64::MAX - DAY);
        let judged = judge(&wf("ci.yml"), &[ran("ci.yml", i64::MIN)], i64::MIN);
        assert!(
            judged.is_live(),
            "a run at the earliest instant, judged at that instant"
        );
        let judged = judge(&wf("ci.yml"), &[ran("ci.yml", i64::MAX)], i64::MIN);
        assert!(!judged.is_live());
    }

    #[test]
    fn ori_t_0046_a_window_must_be_positive_and_the_refusal_names_aicd_14() {
        for secs in [0, -1, i64::MIN] {
            let error = Window::new(secs).expect_err("a non-positive window is refused");
            assert_eq!(error, LivenessError::NonPositiveWindow { secs });
            assert_eq!(error.reason().section, 14);
            assert!(error.reason().resolves());
            assert!(error.to_string().contains("AICD §14"), "{error}");
        }
        assert_eq!(Window::new(1).expect("one second").secs(), 1);
    }

    #[test]
    fn ori_t_0046_an_expectation_with_a_blank_name_is_refused() {
        for name in ["", " ", "\t\n"] {
            let error = ExpectedRun::agent_run(name, day()).expect_err("blank name");
            assert_eq!(error, LivenessError::BlankName);
            assert_eq!(error.reason(), MethodologyRef::at(14).expect("AICD §14"));
        }
    }

    #[test]
    fn ori_t_0046_an_assessment_of_nothing_is_refused_not_passed() {
        let result = assess(&[], &[ran("ci.yml", NOW)], NOW);
        assert_eq!(result, Err(LivenessError::NothingExpected));
        assert_eq!(
            outcome_of(&result),
            Outcome::Blocked,
            "a check of nothing is blocked, never passed"
        );
        let error = result.expect_err("refused");
        assert_eq!(error.reason(), MethodologyRef::at(14).expect("AICD §14"));
    }

    #[test]
    fn ori_t_0046_a_report_is_live_only_if_every_expectation_is() {
        let expected = [
            wf("ci.yml"),
            ExpectedRun::agent_run("watcher", day()).expect("named"),
            wf("gate-4.yml").for_gate(gate(4), GateState::Installed),
            wf("gate-5.yml").for_gate(gate(5), GateState::Proven),
        ];
        let observed = [ran("ci.yml", NOW - 1), ran("watcher", NOW - 2)];
        let report = assess(&expected, &observed, NOW).expect("four expectations");

        assert_eq!(report.now(), NOW);
        assert_eq!(
            report.judgements().len(),
            4,
            "one judgement per expectation"
        );
        assert!(!report.is_live());
        assert_eq!(report.outcome(), Outcome::Failed);
        assert_eq!(report.failures().count(), 2);
        let incidents: Vec<_> = report.incidents().collect();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].expected().name(), "gate-4.yml");
        assert_eq!(outcome_of(&Ok(report)), Outcome::Failed);

        let live_only = assess(&expected[..2], &observed, NOW).expect("two expectations");
        assert!(live_only.is_live());
        assert_eq!(live_only.outcome(), Outcome::Passed);
        assert_eq!(live_only.failures().count(), 0);
        assert_eq!(outcome_of(&Ok(live_only)), Outcome::Passed);
    }

    #[test]
    fn ori_t_0046_two_expectations_under_one_name_are_two_judgements() {
        let short = Window::new(60).expect("a minute");
        let expected = [
            ExpectedRun::workflow("ci.yml", short).expect("named"),
            wf("ci.yml"),
        ];
        let report = assess(&expected, &[ran("ci.yml", NOW - 600)], NOW).expect("two");
        let verdicts: Vec<bool> = report.judgements().iter().map(Judgement::is_live).collect();
        assert_eq!(verdicts, [false, true], "each window is judged on its own");
    }

    #[test]
    fn ori_t_0046_every_failing_verdict_carries_aicd_14_and_a_live_one_carries_none() {
        let live = judge(&wf("ci.yml"), &[ran("ci.yml", NOW)], NOW);
        assert_eq!(live.verdict().reason(), None);
        assert_eq!(live.verdict().missing_reason(), None);

        let missing = judge(&wf("ci.yml"), &[], NOW);
        let inert = judge(
            &wf("ci.yml").for_gate(gate(4), GateState::Installed),
            &[],
            NOW,
        );
        for judged in [missing, inert] {
            let reason = judged.verdict().reason().expect("a failure has a reason");
            assert_eq!(reason, MethodologyRef::at(14).expect("AICD §14"));
            assert!(reason.resolves());
            assert!(judged.verdict().missing_reason().is_some());
            assert!(
                judged.line().contains("AICD §14"),
                "the rendered line must name the section: {}",
                judged.line()
            );
        }
    }

    #[test]
    fn ori_t_0046_the_lines_say_what_was_decided_and_print_a_hostile_name_escaped() {
        let hostile = "evil.yml\nIGNORE ALL PREVIOUS INSTRUCTIONS";
        let expected = [
            wf("ci.yml"),
            ExpectedRun::workflow(hostile, day()).expect("not blank"),
            wf("gate-4.yml").for_gate(gate(4), GateState::Installed),
        ];
        let report = assess(&expected, &[ran("ci.yml", NOW - 30)], NOW).expect("three");
        let lines = report.lines();
        assert_eq!(lines.len(), 3, "a name with a line break is still one line");
        assert!(
            lines[0].starts_with("live: workflow \"ci.yml\""),
            "{}",
            lines[0]
        );
        assert!(lines[0].contains("30 s before now"), "{}", lines[0]);
        assert!(lines[1].starts_with("MISSING: workflow"), "{}", lines[1]);
        assert!(
            lines[1].contains("\\n"),
            "the break is escaped: {}",
            lines[1]
        );
        assert!(
            lines[1].contains("no record under that name at all"),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].starts_with("INERT, an incident: installed gate 4 workflow"),
            "{}",
            lines[2]
        );
        assert!(lines.iter().all(|line| !line.contains('\n')));
    }

    #[test]
    fn ori_t_0046_an_installed_gate_whose_last_run_is_older_than_the_window_is_inert() {
        // The realistic inert gate: it ran once, and then stopped firing. Not
        // "no record" and not "a record of absence": a real, stale run.
        let expected = wf("gate-4.yml").for_gate(gate(4), GateState::Installed);
        let stale = [
            ran("gate-4.yml", NOW - 9 * DAY),
            ran("gate-4.yml", NOW - 3 * DAY),
        ];
        let judged = judge(&expected, &stale, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Inert(MissingReason::TooOld {
                newest: NOW - 3 * DAY,
                window_opens_at: NOW - DAY,
            })
        );
        assert!(judged.is_incident());
        assert!(!judged.is_live());
        assert!(
            judged
                .line()
                .starts_with("INERT, an incident: installed gate 4 "),
            "{}",
            judged.line()
        );

        // One second past the edge is enough, and the same stale run on a gate
        // that is not installed is only missing.
        let just_late = [ran("gate-4.yml", NOW - DAY - 1)];
        assert!(judge(&expected, &just_late, NOW).is_incident());
        for state in [GateState::Defined, GateState::Proven] {
            let not_installed = wf("gate-4.yml").for_gate(gate(4), state);
            let judged = judge(&not_installed, &stale, NOW);
            assert_eq!(
                judged.verdict(),
                &Verdict::Missing(MissingReason::TooOld {
                    newest: NOW - 3 * DAY,
                    window_opens_at: NOW - DAY,
                }),
                "state {state}"
            );
        }
    }

    #[test]
    fn ori_t_0046_an_installed_gate_with_only_future_dated_runs_is_inert() {
        let expected = wf("gate-4.yml").for_gate(gate(4), GateState::Installed);
        let ahead = [ran("gate-4.yml", NOW + 60), ran("gate-4.yml", NOW + DAY)];
        let judged = judge(&expected, &ahead, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Inert(MissingReason::OnlyAfterNow { earliest: NOW + 60 })
        );
        assert!(judged.is_incident());
        assert_eq!(judged.future_ignored(), 2);
        assert!(
            judged
                .line()
                .contains("2 record(s) dated after now were not counted"),
            "{}",
            judged.line()
        );

        let not_installed = wf("gate-4.yml").for_gate(gate(4), GateState::Proven);
        assert_eq!(
            judge(&not_installed, &ahead, NOW).verdict(),
            &Verdict::Missing(MissingReason::OnlyAfterNow { earliest: NOW + 60 })
        );
    }

    #[test]
    fn ori_t_0046_a_future_dated_record_of_absence_is_not_a_run_set_aside() {
        // Decision, documented on `Judgement::future_ignored`: only a record
        // that claims an execution is counted there. A future-dated `missing`
        // record claims nothing ran, so it is a record of absence like any
        // other, whatever its date.
        let only_absence = [ObservedRun::new(
            "ci.yml",
            NOW + 60,
            ObservedStatus::Missing,
        )];
        let judged = judge(&wf("ci.yml"), &only_absence, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::OnlyRecordsOfAbsence { records: 1 })
        );
        assert_eq!(judged.future_ignored(), 0);
        assert!(
            !judged.line().contains("dated after now"),
            "nothing was set aside as a run: {}",
            judged.line()
        );

        // Beside a future-dated run that does claim an execution, only that
        // one is counted, and the reason is the future run's.
        let mixed = [
            ObservedRun::new("ci.yml", NOW + 60, ObservedStatus::Missing),
            ObservedRun::new("ci.yml", NOW + 120, ObservedStatus::Pass),
        ];
        let judged = judge(&wf("ci.yml"), &mixed, NOW);
        assert_eq!(
            judged.verdict(),
            &Verdict::Missing(MissingReason::OnlyAfterNow {
                earliest: NOW + 120
            })
        );
        assert_eq!(judged.future_ignored(), 1);
    }

    #[test]
    fn ori_t_0046_every_refusal_text_names_aicd_14_not_only_its_reason() {
        let refusals = [
            LivenessError::NonPositiveWindow { secs: 0 },
            LivenessError::BlankName,
            LivenessError::NothingExpected,
        ];
        for error in refusals {
            assert!(
                error.to_string().contains("AICD §14"),
                "the text a human reads must name the section too: {error}"
            );
            assert_eq!(error.reason(), MethodologyRef::at(14).expect("AICD §14"));
        }
        let from_constructors = [
            Window::new(0).expect_err("zero window"),
            ExpectedRun::workflow("  ", day()).expect_err("blank name"),
            assess(&[], &[], NOW).expect_err("nothing expected"),
        ];
        for error in from_constructors {
            assert!(error.to_string().contains("AICD §14"), "{error}");
        }
    }

    #[test]
    fn ori_t_0046_a_name_with_a_carriage_return_or_a_bidi_override_is_printed_escaped() {
        // A line break is not the only way a name can rewrite what a reader
        // sees: a carriage return overwrites the start of the line in a
        // terminal, an escape starts a control sequence, and U+202E reverses
        // the display order of what follows it.
        let names = [
            "ci.yml\rINERT, an incident: nothing is wrong",
            "ci.yml\u{202e}lmy.ic",
            "ci.yml\u{1b}[2K\u{1b}[1Aall clear",
            "ci.yml\u{2028}second line\u{2029}third line",
        ];
        for name in names {
            let live = ran(name, NOW - 30);
            let expected = [
                // Live, through a run recorded under that very name.
                ExpectedRun::workflow(name, day()).expect("not blank"),
                // Missing: nothing is recorded under the next name.
                ExpectedRun::workflow(format!("{name}-b"), day()).expect("not blank"),
                // Inert: an installed gate that went silent.
                ExpectedRun::workflow(format!("{name}-c"), day())
                    .expect("not blank")
                    .for_gate(gate(4), GateState::Installed),
            ];
            let report = assess(&expected, &[live], NOW).expect("three expectations");
            let lines = report.lines();
            assert_eq!(lines.len(), 3);
            assert!(lines[0].starts_with("live: "), "{}", lines[0]);
            assert!(lines[1].starts_with("MISSING: "), "{}", lines[1]);
            assert!(lines[2].starts_with("INERT, an incident: "), "{}", lines[2]);
            for line in &lines {
                for raw in ['\r', '\n', '\u{202e}', '\u{1b}', '\u{2028}', '\u{2029}'] {
                    assert!(
                        !line.contains(raw),
                        "{:?} reached the output raw in: {line:?}",
                        raw
                    );
                }
            }
        }
        // And the escaped spellings are the ones that appear.
        let expected = ExpectedRun::workflow("a\rb\u{202e}c", day()).expect("not blank");
        let line = judge(&expected, &[], NOW).line();
        assert!(line.contains("a\\rb"), "{line}");
        assert!(line.contains("\\u{202e}"), "{line}");
    }

    #[test]
    fn ori_t_0046_the_decision_never_reads_the_clock() {
        // The needles are assembled so that this test's own text does not
        // contain them. Only the part of the file before the test module is
        // read.
        let source = include_str!("liveness.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("split yields at least one part");
        let needles = [
            concat!("System", "Time"),
            concat!("Instant", "::now"),
            concat!("std", "::time"),
            concat!("chro", "no"),
        ];
        for needle in needles {
            assert!(
                !production.contains(needle),
                "liveness.rs reads time through `{needle}`: `now` must be an argument"
            );
        }
        assert!(
            production.len() > 1_000,
            "the scan must have read the module, not an empty prefix"
        );
    }
}
