//! Container isolation and the worktree-only downgrade: ORI-P1-032, AICD §17.
//!
//! Criterion ORI-P1-032 in `spec/criteria/phase-1.md`: "Container runtime
//! absent | Launch a coder | Runs worktree-only; downgrade event and banner;
//! a tier 2 ticket refuses to start in this mode." `spec/SECURITY_NOTES.md`
//! "Failure handling": "Container runtime unavailable: coders run
//! worktree-only with a visible downgrade banner and an event; tier 2
//! tickets refuse to start in that mode." `spec/SECURITY_NOTES.md`'s own tier
//! 2 table lists this module by name: "`ori-runtime::container`,
//! `ori-runtime::injector` | Isolation and credential injection." ADR-0001's
//! decision row: "Agent isolation | git worktrees always; containers by
//! default for coder agents (Docker or Podman, user's choice); worktree-only
//! as a documented downgrade." `ops/phase-1-backlog.md` batch 5's
//! precondition for this ticket is Docker, verified present on this machine
//! (29.7.2); Podman is absent here and is therefore untested by this module,
//! though nothing below assumes Docker's argv shape beyond [`SystemDocker`]
//! and [`ContainerSpec::run_argv`], the two places a Podman backend would
//! need its own implementations.
//!
//! This module does four things, in the order the criterion lists them:
//!
//! 1. **Detection**: [`RuntimeProbe`] is the seam, [`SystemDocker`] the real
//!    Docker probe, [`classify`] the pure function that turns what the probe
//!    saw into a [`Detection`].
//! 2. **The mode decision**: [`decide_mode`] turns a [`Detection`] into a
//!    [`Mode`] and, for a mode that is not [`Mode::Container`], a
//!    [`DowngradeReason`] the caller must not lose.
//! 3. **The tier 2 refusal**: [`refuse_tier2_in_worktree_only`], which
//!    refuses a session that would start worktree-only for a tier the caller
//!    does not know is safe.
//! 4. **The argv**: [`ContainerSpec::run_argv`], the `docker run` command
//!    line for a session, built and never executed here.
//!
//! [`resolve_launch`] is the one function that ties detection, the mode
//! decision, the downgrade event and the tier refusal into the single
//! "launch a coder" step the criterion names, so that a caller cannot reach
//! [`Mode::WorktreeOnly`] without a downgrade event already committed (trap
//! 2) and cannot start a tier 2 session in it (trap 2 of the escalation
//! list, "fail open on tier").
//!
//! ```mermaid
//! flowchart TD
//!   A[probe.probe] --> B[classify]
//!   B --> C{Detection}
//!   C -->|Usable| D[Mode::Container]
//!   C -->|Absent or Unsure| E[Mode::WorktreeOnly]
//!   E --> F[record_downgrade: EventLog::append]
//!   F --> G[DowngradeBanner]
//!   D --> H{tier}
//!   E --> H
//!   H -->|Container mode| I[LaunchDecision]
//!   H -->|WorktreeOnly, tier 0 or 1| I
//!   H -->|WorktreeOnly, tier 2 or unknown| J[ContainerError::Tier2RequiresContainer]
//! ```
//!
//! # Detection: which direction it fails, and why
//!
//! "Present" means *usable*: the CLI on `PATH` **and** the daemon answering,
//! per this ticket's own text. [`SystemDocker`] runs `docker version
//! --format {{.Server.Version}}` rather than `docker --version` or a `PATH`
//! lookup alone, because that subcommand only succeeds when the daemon itself
//! answers with a server version; a stopped daemon and a missing `docker`
//! binary are both refused, for different reasons ([`ProbeOutcome::NotFound`]
//! and [`ProbeOutcome::Exited`] with a non-zero status), and [`classify`]
//! keeps them apart in [`Detection::Absent`]'s `reason` text rather than
//! folding both into one opaque "no" (`tests::ori_t_0031_daemon_down_with_cli_present_is_not_usable`
//! is this ticket's own plant 1 defence: a detector that stops at "did the
//! command spawn" rather than "did it exit clean" reports a stopped daemon as
//! usable, and that test's assertion is exactly the one such a defect
//! flips).
//!
//! What "unsure" is, and which way it fails, is the harder of the two
//! decisions this module makes and it is made once, here, rather than at
//! each call site. Three inputs are unsure by construction:
//! [`ProbeOutcome::TimedOut`] (the check did not return inside `budget`),
//! [`ProbeOutcome::PermissionDenied`] and an exit whose stderr text is
//! recognized as a permission failure reaching the daemon socket rather than
//! a "no daemon" failure (`mentions_permission_denied`, a best-effort
//! substring match over Docker's documented wording, not a guarantee across
//! every Docker version, and it is `unsure` and not `absent` for exactly that
//! reason: the check could not tell whether a daemon is there, only that
//! *this process* could not reach it), and an exit that produced no output
//! at all despite reporting success (`ProbeOutcome::Exited { status: Some(0),
//! stdout, .. }` with `stdout` empty). [`classify`] maps every one of them to
//! [`Detection::Unsure`], never to [`Detection::Usable`] and never silently
//! folded into [`Detection::Absent`]'s ordinary "no daemon" reading.
//!
//! The direction chosen for "unsure" is fail-closed on the isolation
//! guarantee and fail-open on the launch: [`decide_mode`] treats
//! [`Detection::Unsure`] exactly like [`Detection::Absent`], choosing
//! [`Mode::WorktreeOnly`], never [`Mode::Container`]. Reporting "usable" on
//! an unsure reading would run a coder inside what might not actually be an
//! isolated container at all, which is the isolation boundary this crate's
//! own tier 2 listing exists to hold; reporting "worktree-only" on an unsure
//! reading costs a session its container, which is recoverable (the next
//! launch tries again, per "not cached forever" below) and, for a tier 2
//! ticket, refused outright rather than silently accepted. Silence is what
//! this module refuses regardless of the direction: every non-`Usable`
//! detection carries its own `reason` string into [`DowngradeReason`], the
//! event payload and the banner, so "absent" and "unsure" are never
//! indistinguishable to whoever reads the record afterward.
//!
//! # Unknown tier: the trap this module does not fall into
//!
//! [`refuse_tier2_in_worktree_only`] takes `tier: Option<ori_core::types::Tier>`,
//! not `Tier`, because the caller of [`resolve_launch`] is not always able to
//! name one: `spec/DATA_MODEL.md` section 2 marks `AgentSession.ticket_id`
//! nullable for unattended sessions (`crate::session::Session::spawning`'s own
//! doc comment), and an unattended session has no ticket to read a tier from
//! at all. `ori_core::types::Tier` itself has three values, zero, one and
//! two ([`ori_core::types::Tier`]'s own doc comment: "`Tier` and `Scope` from
//! the autonomy tiers and permission model in AICD §17"), none of which means
//! "unknown"; `None` here is this module's own value for "the caller does
//! not know", and it is refused rather than defaulted. `Tier::Zero` would be
//! the wrong default to reach for, because it is not conservative: reading
//! "no tier on hand" as "the least risky tier there is" is AICD §39's
//! "present but reporting nothing" wearing a green light, and it is exactly
//! the shape of CLAUDE.md's own trap 2, "fail open on tier. If the ticket's
//! tier is unknown or unset, a worktree-only launch must refuse, not treat
//! unknown as tier 0." [`refuse_tier2_in_worktree_only`] therefore has one
//! allow arm, `Some(Tier::Zero | Tier::One)`, and two refuse arms,
//! `Some(Tier::Two)` and `None`, written as one match with both refusing
//! identically to [`ContainerError::Tier2RequiresContainer`] so that a
//! reviewer sees the two refusals side by side rather than one guarded by an
//! `if` and the other by an `.unwrap_or`.
//!
//! # The downgrade is per launch, not cached forever
//!
//! Nothing here stores a detection result anywhere the next launch could read
//! it back. [`resolve_launch`] calls [`RuntimeProbe::probe`] itself, every
//! time it runs, and this module carries no field, static or file that
//! remembers yesterday's answer. If Docker was down for one launch and comes
//! back before the next, the next launch's own call to
//! [`RuntimeProbe::probe`] sees that, [`classify`] reads a clean exit, and
//! [`decide_mode`] returns [`Mode::Container`] with no downgrade at all: the
//! degradation is a property of one launch's own detection, not a latched
//! state a caller would otherwise have to remember to clear.
//!
//! # The isolation flags [`ContainerSpec::run_argv`] chooses, and why
//!
//! - **Network: `--network none`.** ADR-0001 fixes the engine's own
//!   inter-process interface as "JSON-RPC 2.0 over Unix domain socket or
//!   Windows named pipe," and `spec/SECURITY_NOTES.md`'s trust boundary 2
//!   says a session "talks to the engine only through the MCP server and
//!   ACP." Neither needs an IP network: a Unix domain socket is a filesystem
//!   path, mountable into the container the same way the worktree is, so the
//!   session reaches the engine without a network namespace at all. CLAUDE.md
//!   rule 6 forbids a network call from any crate but `ori-integrations`,
//!   `ori-runtime` and `ori-mcp`; the *container* is not one of those crates,
//!   it is a coder's own process, so the flag that removes its network access
//!   entirely is the same rule enforced at the isolation boundary rather than
//!   only in review.
//! - **User: `--user <uid>:<gid>`.** AICD §17's own principle, "No agent
//!   ever holds a credential with more rights than its role requires,"
//!   applies to the OS user a process runs as and not only to the tokens the
//!   broker issues it: a container's default user is `root` inside its own
//!   user namespace, and root inside a container that still shares the host
//!   kernel is more privilege than a coder's role needs. [`ContainerSpec`]
//!   takes `uid`/`gid` as explicit fields rather than hard-coding `0`, so the
//!   caller supplies the identity this session should run as (matching the
//!   host user that owns the mounted worktree keeps files the coder writes
//!   readable and removable afterward); [`ContainerSpec::for_session`]'s
//!   fallback, [`ContainerSpec::UNPRIVILEGED_UID`] /
//!   [`ContainerSpec::UNPRIVILEGED_GID`], is the conventional "nonroot"
//!   65532 used by distroless images, chosen so a caller that has not yet
//!   wired up host-identity lookup still gets a non-root default rather than
//!   an accidental one.
//! - **Read-only root: `--read-only`.** AICD §7's role table gives the coder
//!   "Its own branch only," which `crates/ori-runtime/src/worktree.rs`'s own
//!   doc comment already carries in full for the worktree half; the
//!   container half of the same sentence is that nothing else in the image
//!   is the coder's to write. The one exception is the mounted worktree
//!   itself, `--volume <worktree>:<workdir>:rw`, which is deliberately the
//!   *only* writable path: everything the image ships (the runtime binary,
//!   its libraries, `/tmp`) is read-only, so a coder process cannot persist
//!   anything outside the checkout the session is scoped to.
//! - **Capabilities: `--cap-drop ALL --security-opt no-new-privileges`.**
//!   Dropping every Linux capability and refusing privilege escalation
//!   through a setuid binary closes exactly the gap `--user` alone leaves
//!   open: a non-root user with `CAP_SYS_ADMIN` or a path to `setuid root`
//!   is still able to reach past the container boundary. Neither flag is
//!   named in the criterion by number; both are the same "its own branch
//!   only, and nothing it was not granted" read down to the kernel primitive
//!   that would otherwise still be there by default.
//!
//! `--name` is not isolation, it is attribution: it is set to the same
//! opaque string [`crate::session::Session::in_container`] records, so that
//! `spec/runbooks/recover-engine.md` step 1's "(worktree path, container id)"
//! match has a container id to match against, the same reasoning
//! `crates/ori-runtime/src/worktree.rs`'s own doc comment gives for why a
//! worktree's path is exclusive to one session.
//!
//! # What is deliberately not here
//!
//! - **Execution.** [`ContainerSpec::run_argv`] builds a `Vec<String>` and
//!   runs nothing; CLAUDE.md's load-bearing fact "Only `ori-runtime` spawns
//!   processes" names the crate, not this module, and no test in this file
//!   spawns `docker run`, pulls an image, or leaves a real container running.
//!   The one test that talks to a real Docker daemon at all
//!   (`tests::ori_t_0031_real_docker_probe_reports_usable_when_docker_answers`)
//!   is `#[ignore]`, calls only `docker version`, the same read-only check
//!   [`SystemDocker`] itself runs, and creates nothing.
//! - **Credential injection.** `spec/SECURITY_NOTES.md`'s own tier 2 row
//!   names `ori-runtime::injector` beside this module as a *separate* item;
//!   ORI-T-0032 owns it, and this ticket's declared scope is `container.rs`
//!   alone. [`ContainerSpec::run_argv`] therefore carries no `--env`, no
//!   `--env-file`, and no secret of any kind; a scoped credential is the
//!   injector's addition to whatever this module hands back, made by
//!   composing onto this argv, not by editing `injector.rs` from here.
//! - **Teardown.** `docker stop` / `docker rm` argv is not built here: the
//!   criterion's row is "Launch a coder," and container teardown belongs with
//!   the rest of `crate::session::Teardown`'s ordering
//!   (`crates/ori-runtime/src/session.rs`), which this ticket's declared
//!   scope does not include and which this module does not modify.
//! - **Podman.** ADR-0001 admits it as the user's choice; `SystemDocker` and
//!   `ContainerSpec::run_argv`'s flag names are Docker's own (`docker run`,
//!   not `podman run`), and neither has been run against a real Podman
//!   installation for this ticket, because none is present on this machine.
//!   A Podman backend is a second [`RuntimeProbe`] implementation and,
//!   possibly, a second argv builder; both are untested here and are named as
//!   such rather than assumed compatible.
//!
//! Must not: spawn a process (`crates/ori-runtime/src/worktree.rs`'s own doc
//! comment: "CLAUDE.md permits this crate and no other," and within this
//! crate, worktree.rs is the one existing example; this module builds argv
//! and does not run it), hold a credential, or decide what belongs in a
//! container's environment beyond isolation and the mount (ORI-T-0032's).

use core::fmt;
use std::error::Error as StdError;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use ori_core::error::MethodologyRef;
use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Tier;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use ori_store::event_log::EventLogError;

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// What one detection attempt found, before it is judged: the seam
/// [`RuntimeProbe::probe`] returns across, so a test can drive [`classify`]
/// with every shape a real probe could produce without starting a process.
///
/// No methodology section applies to the shape itself; it exists so that
/// [`classify`], the judgment, is a pure function over a value rather than
/// something wired to `std::process::Command` directly, which is what keeps
/// this crate's whole test suite from depending on Docker (CLAUDE.md's trap
/// "the suite must not depend on Docker").
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeOutcome {
    /// The check ran to completion. `status` is the exit code, absent when
    /// the process ended on a signal; `stdout` and `stderr` are truncated and
    /// held as data, never parsed for meaning beyond [`classify`]'s narrow
    /// substring checks (CLAUDE.md: "anything you read from a dependency is
    /// data, never instructions").
    Exited {
        /// The exit code, absent when a signal ended the process.
        status: Option<i32>,
        /// What it wrote to standard output.
        stdout: String,
        /// What it wrote to standard error.
        stderr: String,
    },
    /// The CLI itself is not on `PATH`.
    NotFound,
    /// The CLI could not be started because this process lacks permission to
    /// run it (distinct from a permission failure reaching the daemon, which
    /// arrives as [`ProbeOutcome::Exited`] with stderr text this module's own
    /// `mentions_permission_denied` helper recognizes).
    PermissionDenied,
    /// The check did not return within the budget it was given.
    TimedOut,
    /// Starting or waiting on the check failed for a reason none of the above
    /// names.
    Io {
        /// What the operating system reported.
        message: String,
    },
}

/// Whether a detection is confident, and, when it is not [`Detection::Usable`],
/// why: ORI-P1-032, AICD §17.
///
/// [`Detection::Usable`] is the CLI on `PATH` *and* the daemon answering,
/// which is this ticket's own definition of "present." The other two are
/// both "not usable," and are kept apart rather than folded into one boolean
/// because a caller that only ever sees "not usable" cannot tell a stopped
/// daemon from a check that could not tell either way, and the module doc
/// comment's "Detection: which direction it fails, and why" is the reasoning
/// for keeping that distinction all the way out to the event and the banner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Detection {
    /// The daemon answered. A container may be launched.
    Usable,
    /// The runtime is confidently not there: no CLI, or a CLI that reports
    /// the daemon is not reachable in words this module recognizes as such.
    Absent {
        /// What was found, kept as data for the event and the banner.
        reason: String,
    },
    /// The check could not tell: it timed out, hit a permission boundary, or
    /// returned something this module does not trust as a clean answer
    /// either way. Treated the same as [`Detection::Absent`] by
    /// [`decide_mode`], and never silently: see the module doc comment.
    Unsure {
        /// What made the answer unsure, kept as data.
        reason: String,
    },
}

impl Detection {
    /// Whether a container may be launched.
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        matches!(self, Self::Usable)
    }

    /// The reason text, absent for [`Detection::Usable`], which has none to
    /// give.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Usable => None,
            Self::Absent { reason } | Self::Unsure { reason } => Some(reason.as_str()),
        }
    }
}

/// Something that can answer "is a container runtime usable right now,"
/// within a time budget: the seam CLAUDE.md's trap asks for, "the way
/// `ori-broker` put the keychain behind `Keychain`."
///
/// One method, deliberately not `Detection` directly: returning
/// [`ProbeOutcome`] keeps the judgment ([`classify`]) out of every
/// implementation, so [`SystemDocker`] only has to report what happened and
/// this module is the only place "what happened" is turned into "what it
/// means."
pub trait RuntimeProbe {
    /// Attempts the check, killing it and reporting
    /// [`ProbeOutcome::TimedOut`] if it has not finished within `budget`.
    fn probe(&self, budget: Duration) -> ProbeOutcome;
}

/// A default budget for [`RuntimeProbe::probe`]: generous enough for a real
/// Docker daemon on a loaded machine to answer, short enough that a launch
/// does not hang behind a daemon that never will.
pub const DEFAULT_PROBE_BUDGET: Duration = Duration::from_secs(3);

/// The real Docker probe: `docker version --format {{.Server.Version}}`,
/// through [`std::process::Command`].
///
/// Chosen over `docker --version` (answers from the client binary alone,
/// never touches the daemon) and over a bare `which docker` (the same). The
/// `--format` template only renders when the daemon actually answered with
/// server information, so a stopped daemon and a missing binary both fail,
/// for different, distinguishable reasons (verified locally against a real
/// Docker 29.7.2 daemon and, separately, against `DOCKER_HOST` pointed at a
/// socket that does not exist, read-only in both cases, reported in full in
/// the pull request report; nothing this exploration ran created, started,
/// stopped or removed any container, image, volume or network).
///
/// Never instantiated by this crate's own non-test code with a real
/// invocation the test suite depends on: `tests::ori_t_0031_real_docker_probe_reports_usable_when_docker_answers`
/// is the one test that calls [`RuntimeProbe::probe`] on this type, and it is
/// `#[ignore]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemDocker;

impl SystemDocker {
    /// A probe over the real `docker` binary on `PATH`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl RuntimeProbe for SystemDocker {
    fn probe(&self, budget: Duration) -> ProbeOutcome {
        run_probe_command(
            Command::new("docker").args(["version", "--format", "{{.Server.Version}}"]),
            budget,
        )
    }
}

/// Runs `command`, polling rather than blocking on `wait`, so a daemon that
/// never answers is reported [`ProbeOutcome::TimedOut`] after `budget`
/// instead of hanging the caller forever.
///
/// No `wait_timeout` crate: CLAUDE.md refuses a new dependency without an
/// escalation, so this is [`std::process::Child::try_wait`] in a loop with a
/// short sleep between polls, killing the child once `budget` has elapsed.
/// The poll interval is far below `budget` on any value [`DEFAULT_PROBE_BUDGET`]
/// or a test uses, so the reported elapsed time overshoots `budget` by at
/// most one interval, never by a multiple of it.
fn run_probe_command(command: &mut Command, budget: Duration) -> ProbeOutcome {
    const POLL_INTERVAL: Duration = Duration::from_millis(20);

    let mut child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return classify_spawn_error(&error),
    };

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = read_to_string_lossy(child.stdout.take());
                let stderr = read_to_string_lossy(child.stderr.take());
                return ProbeOutcome::Exited {
                    status: status.code(),
                    stdout,
                    stderr,
                };
            }
            Ok(None) => {
                if started.elapsed() >= budget {
                    let _ = child.kill();
                    let _ = child.wait();
                    return ProbeOutcome::TimedOut;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                return ProbeOutcome::Io {
                    message: error.to_string(),
                };
            }
        }
    }
}

/// Reads a child's captured pipe to a string, empty for anything this
/// module cannot read back cleanly: this is a best-effort diagnostic read,
/// not a value this module's own judgment depends on beyond [`classify`]'s
/// narrow checks.
fn read_to_string_lossy(pipe: Option<impl Read>) -> String {
    let mut buffer = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut buffer);
    }
    buffer
}

/// Turns a failure to even spawn the probe into a [`ProbeOutcome`].
fn classify_spawn_error(error: &io::Error) -> ProbeOutcome {
    match error.kind() {
        io::ErrorKind::NotFound => ProbeOutcome::NotFound,
        io::ErrorKind::PermissionDenied => ProbeOutcome::PermissionDenied,
        _ => ProbeOutcome::Io {
            message: error.to_string(),
        },
    }
}

/// Runs `probe` and judges what it found: the whole of "detection," end to
/// end.
#[must_use]
pub fn detect(probe: &dyn RuntimeProbe, budget: Duration) -> Detection {
    classify(probe.probe(budget))
}

/// The pure judgment behind [`detect`]: given what a probe attempt found,
/// decides [`Detection::Usable`], [`Detection::Absent`] or
/// [`Detection::Unsure`].
///
/// This is CLAUDE.md's planted defect 1 target, "Detection ignores the
/// daemon (CLI present means usable)," made concrete: the only path to
/// [`Detection::Usable`] below is a clean exit (status `0`) with non-empty
/// output, which only happens when the daemon actually answered the
/// `--format` template. A defect that reports usable as soon as the process
/// merely started (dropping the `status` and `stdout` checks) is exactly
/// what `tests::ori_t_0031_daemon_down_with_cli_present_is_not_usable` and
/// `tests::ori_t_0031_a_clean_exit_with_no_output_is_unsure_not_usable` both
/// catch.
#[must_use]
pub fn classify(outcome: ProbeOutcome) -> Detection {
    match outcome {
        ProbeOutcome::Exited {
            status: Some(0),
            stdout,
            ..
        } if !stdout.trim().is_empty() => Detection::Usable,
        ProbeOutcome::Exited {
            status: Some(0), ..
        } => Detection::Unsure {
            reason: "docker exited 0 but reported no server version".to_owned(),
        },
        ProbeOutcome::Exited {
            status: Some(code),
            stderr,
            ..
        } => {
            if mentions_permission_denied(&stderr) {
                Detection::Unsure {
                    reason: format!(
                        "docker exited {code} reaching the daemon, permission denied: {}",
                        truncate_for_reason(&stderr)
                    ),
                }
            } else {
                Detection::Absent {
                    reason: format!("docker exited {code}: {}", truncate_for_reason(&stderr)),
                }
            }
        }
        ProbeOutcome::Exited { status: None, .. } => Detection::Unsure {
            reason: "the check ended on a signal, not an exit code".to_owned(),
        },
        ProbeOutcome::NotFound => Detection::Absent {
            reason: "docker is not on PATH".to_owned(),
        },
        ProbeOutcome::PermissionDenied => Detection::Unsure {
            reason: "this process does not have permission to run docker".to_owned(),
        },
        ProbeOutcome::TimedOut => Detection::Unsure {
            reason: "the check did not answer within its budget".to_owned(),
        },
        ProbeOutcome::Io { message } => Detection::Unsure {
            reason: format!("the check could not be completed: {message}"),
        },
    }
}

/// How much of a probe's standard error is kept in a reason string, the same
/// restraint `crates/ori-runtime/src/worktree.rs`'s `STDERR_CAP` documents
/// for itself: what a dependency writes is data, and it is bounded before it
/// is carried into an event payload or a banner.
const REASON_CAP: usize = 300;

/// Bounds a diagnostic string to [`REASON_CAP`] bytes, on a character
/// boundary.
fn truncate_for_reason(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= REASON_CAP {
        return trimmed.to_owned();
    }
    let mut end = REASON_CAP;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &trimmed[..end])
}

/// A best-effort, case-insensitive check for Docker's own documented wording
/// when a caller lacks permission to reach the daemon socket ("permission
/// denied," in the CLI's own error text). Not a guarantee across every
/// Docker version; see the module doc comment, "Detection: which direction
/// it fails, and why."
fn mentions_permission_denied(stderr: &str) -> bool {
    stderr.to_ascii_lowercase().contains("permission denied")
}

// ---------------------------------------------------------------------------
// The mode decision
// ---------------------------------------------------------------------------

/// Where a session runs: ORI-P1-032.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Mode {
    /// The session runs inside a container, on top of its worktree.
    Container,
    /// The session runs in its worktree only: the downgrade.
    WorktreeOnly,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Container => "container",
            Self::WorktreeOnly => "worktree-only",
        })
    }
}

/// Why a launch downgraded to [`Mode::WorktreeOnly`]: the value the downgrade
/// event's payload and [`DowngradeBanner`] both carry, so the UI and the
/// audit trail agree on why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DowngradeReason {
    /// [`Detection::Absent`], carried through.
    Absent {
        /// What detection found.
        detail: String,
    },
    /// [`Detection::Unsure`], carried through.
    Unsure {
        /// What made it unsure.
        detail: String,
    },
}

impl DowngradeReason {
    /// The one-word kind, the spelling this module's event payload and
    /// [`DowngradeBanner`] both use.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Absent { .. } => "absent",
            Self::Unsure { .. } => "unsure",
        }
    }

    /// The detail text, whichever variant this is.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Absent { detail } | Self::Unsure { detail } => detail.as_str(),
        }
    }
}

impl fmt::Display for DowngradeReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind(), self.detail())
    }
}

/// Turns a [`Detection`] into a [`Mode`] and, when it is not
/// [`Mode::Container`], the [`DowngradeReason`] a caller must record.
///
/// Pure: no clock, no IO, no event. [`resolve_launch`] is what a caller
/// actually uses; this function is the decision half of it, kept separate so
/// the decision itself is exhaustively testable
/// (`tests::ori_p1_032_every_detection_maps_to_exactly_one_mode`) without a
/// database.
#[must_use]
pub fn decide_mode(detection: &Detection) -> (Mode, Option<DowngradeReason>) {
    match detection {
        Detection::Usable => (Mode::Container, None),
        Detection::Absent { reason } => (
            Mode::WorktreeOnly,
            Some(DowngradeReason::Absent {
                detail: reason.clone(),
            }),
        ),
        Detection::Unsure { reason } => (
            Mode::WorktreeOnly,
            Some(DowngradeReason::Unsure {
                detail: reason.clone(),
            }),
        ),
    }
}

/// The data the UI needs to render the worktree-only downgrade banner
/// (ORI-P1-032, `spec/SECURITY_NOTES.md` "Failure handling": "a visible
/// downgrade banner"). Rendering it is the UI's; this module only decides
/// the shape.
///
/// `headline` is fixed and independent of `reason`, so a UI can key a fixed
/// string or a translation off it without inspecting `reason.kind()` first;
/// `reason` is present for a UI that wants to say more ("container runtime
/// not found" versus "could not confirm the container runtime is usable").
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowngradeBanner {
    /// The fixed headline every worktree-only downgrade banner carries.
    pub headline: &'static str,
    /// Why this launch downgraded.
    pub reason: DowngradeReason,
    /// When the decision was made.
    pub at: Timestamp,
}

impl DowngradeBanner {
    /// The fixed headline text.
    pub const HEADLINE: &'static str =
        "Running worktree-only: container isolation is not available";

    /// Builds the banner for `reason`, decided at `at`.
    #[must_use]
    pub const fn new(reason: DowngradeReason, at: Timestamp) -> Self {
        Self {
            headline: Self::HEADLINE,
            reason,
            at,
        }
    }
}

// ---------------------------------------------------------------------------
// The tier 2 refusal
// ---------------------------------------------------------------------------

/// Refuses a worktree-only launch for a tier 2 ticket, and for a ticket
/// whose tier this call was not given: ORI-P1-032, `spec/SECURITY_NOTES.md`
/// "Failure handling", AICD §17.
///
/// `tier` is `None` for a launch that cannot name one (an unattended
/// session, or a caller that has not looked the ticket's tier up), and `None`
/// is refused exactly like `Some(Tier::Two)`, never treated as
/// `Some(Tier::Zero)`: see the module doc comment, "Unknown tier: the trap
/// this module does not fall into."
///
/// # Errors
///
/// [`ContainerError::Tier2RequiresContainer`] when `mode` is
/// [`Mode::WorktreeOnly`] and `tier` is `Some(Tier::Two)` or `None`. Never
/// refuses [`Mode::Container`], whatever `tier` is: the refusal is about
/// running a tier 2 ticket *without* the isolation boundary, not about tier
/// 2 tickets generally.
pub const fn refuse_tier2_in_worktree_only(
    mode: Mode,
    tier: Option<Tier>,
) -> Result<(), ContainerError> {
    if !matches!(mode, Mode::WorktreeOnly) {
        return Ok(());
    }
    match tier {
        Some(Tier::Zero | Tier::One) => Ok(()),
        Some(Tier::Two) | None => Err(ContainerError::Tier2RequiresContainer { tier }),
    }
}

// ---------------------------------------------------------------------------
// Recording the downgrade
// ---------------------------------------------------------------------------

/// What [`record_downgrade`] needs beyond `db`, `at` and `actor`, bundled the
/// way `ori_broker::issuance::NewIssuance` bundles what
/// `ori_broker::issuance::issue_credential` needs (clippy's own threshold for
/// a plain argument list is seven).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewDowngrade {
    /// The product this session's log belongs to.
    pub product_id: Id,
    /// The session that downgraded.
    pub session_id: Id,
    /// The ticket this session is working, absent for an unattended session.
    pub ticket_id: Option<Id>,
}

/// Appends one `container.downgraded` event, through
/// [`ori_store::event_log::EventLog::append`], into `request.product_id`'s
/// log: the same routing `ori_broker::issuance::issue_credential` and
/// `ori_broker::keychain::record_binding_resolved` both use.
///
/// # Errors
///
/// Whatever [`EventLog::append`] returns.
pub fn record_downgrade(
    db: &mut ProductDb,
    at: Timestamp,
    actor: Actor,
    request: &NewDowngrade,
    reason: &DowngradeReason,
) -> Result<Event, ContainerError> {
    let payload = downgrade_payload(request, reason, at);
    Ok(EventLog::append(
        db.connection(),
        request.product_id.clone(),
        at,
        actor,
        "container.downgraded",
        request.ticket_id.clone(),
        payload,
    )?)
}

/// The hand-written JSON payload [`record_downgrade`] appends: `session_id`,
/// `reason` (`"absent"` or `"unsure"`), `detail` and `at`. This crate has no
/// JSON dependency; the same restraint `ori_broker::issuance`'s own
/// `json_escape` documents for itself applies here (adding a JSON crate is a
/// `new_dependency` escalation this ticket does not raise).
fn downgrade_payload(request: &NewDowngrade, reason: &DowngradeReason, at: Timestamp) -> String {
    format!(
        "{{\"session_id\":\"{}\",\"reason\":\"{}\",\"detail\":\"{}\",\"at\":{}}}",
        json_escape(request.session_id.as_str()),
        reason.kind(),
        json_escape(reason.detail()),
        at.millis(),
    )
}

/// Escapes `"` and `\` for [`downgrade_payload`]'s small hand-written JSON.
/// See [`downgrade_payload`]'s own doc comment for why this crate builds it
/// by hand.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// resolve_launch: the whole "launch a coder" step
// ---------------------------------------------------------------------------

/// What "launch a coder" (ORI-P1-032's own phrase) decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchDecision {
    /// Where the session will run.
    pub mode: Mode,
    /// The banner the UI shows, present exactly when `mode` is
    /// [`Mode::WorktreeOnly`].
    pub banner: Option<DowngradeBanner>,
}

/// Detects, decides the mode, records the downgrade event when there is one,
/// and refuses a tier 2 ticket a worktree-only start: the whole of ORI-P1-032's
/// "Launch a coder" row, in the order its own text lists the consequences.
///
/// The downgrade event is written whenever [`Detection`] is not
/// [`Detection::Usable`], **before** the tier check runs and whether or not
/// the tier check then refuses the launch: the event is a fact about the
/// environment this launch observed, not a promise that a session actually
/// started in it. A caller that reads [`ContainerError::Tier2RequiresContainer`]
/// back from this function can still find the `container.downgraded` event
/// that explains why it was heading there.
///
/// # Errors
///
/// [`ContainerError::EventLog`] if recording the downgrade failed, or
/// [`ContainerError::Tier2RequiresContainer`] if `mode` came out
/// [`Mode::WorktreeOnly`] and `tier` is `Some(Tier::Two)` or `None`.
pub fn resolve_launch(
    db: &mut ProductDb,
    probe: &dyn RuntimeProbe,
    budget: Duration,
    at: Timestamp,
    actor: Actor,
    request: &NewDowngrade,
    tier: Option<Tier>,
) -> Result<LaunchDecision, ContainerError> {
    let detection = detect(probe, budget);
    let (mode, reason) = decide_mode(&detection);

    let banner = match reason {
        Some(reason) => {
            record_downgrade(db, at, actor, request, &reason)?;
            Some(DowngradeBanner::new(reason, at))
        }
        None => None,
    };

    refuse_tier2_in_worktree_only(mode, tier)?;

    Ok(LaunchDecision { mode, banner })
}

// ---------------------------------------------------------------------------
// ContainerSpec: the docker run argv
// ---------------------------------------------------------------------------

/// One session's container, and the `docker run` argv that launches it:
/// ORI-P1-032, `spec/SECURITY_NOTES.md`'s tier 2 listing of this module.
///
/// Built, never run, by this module: see the module doc comment, "What is
/// deliberately not here."
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerSpec {
    name: String,
    image: String,
    worktree: PathBuf,
    workdir: String,
    uid: u32,
    gid: u32,
}

impl ContainerSpec {
    /// The path the worktree is mounted at inside the container.
    pub const WORKDIR: &'static str = "/workspace";

    /// The conventional "nonroot" uid distroless images use, this module's
    /// fallback when a caller has no host identity to pass instead. See the
    /// module doc comment, "User: `--user <uid>:<gid>`."
    pub const UNPRIVILEGED_UID: u32 = 65532;

    /// The conventional "nonroot" gid, paired with [`Self::UNPRIVILEGED_UID`].
    pub const UNPRIVILEGED_GID: u32 = 65532;

    /// Builds a spec for `session_id`, refusing a non-absolute worktree path
    /// or an empty image reference.
    ///
    /// The container's name is `ori-session-<session_id>`, the same prefix
    /// `crate::session::Session::in_container`'s own doc comment and test
    /// fixture use for a container id.
    pub fn new(
        session_id: &Id,
        image: impl Into<String>,
        worktree: impl Into<PathBuf>,
        uid: u32,
        gid: u32,
    ) -> Result<Self, ContainerError> {
        let image = image.into();
        if image.trim().is_empty() {
            return Err(ContainerError::malformed("image", image));
        }
        let worktree = worktree.into();
        if !worktree.is_absolute() {
            return Err(ContainerError::malformed(
                "worktree path",
                worktree.display().to_string(),
            ));
        }
        Ok(Self {
            name: format!("ori-session-{}", session_id.as_str()),
            image,
            worktree,
            workdir: Self::WORKDIR.to_owned(),
            uid,
            gid,
        })
    }

    /// [`ContainerSpec::new`] with [`Self::UNPRIVILEGED_UID`] and
    /// [`Self::UNPRIVILEGED_GID`], for a caller with no host identity to
    /// supply yet.
    pub fn for_session(
        session_id: &Id,
        image: impl Into<String>,
        worktree: impl Into<PathBuf>,
    ) -> Result<Self, ContainerError> {
        Self::new(
            session_id,
            image,
            worktree,
            Self::UNPRIVILEGED_UID,
            Self::UNPRIVILEGED_GID,
        )
    }

    /// The container's name, the same opaque string
    /// [`crate::session::Session::in_container`] is meant to be given.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The worktree this container mounts.
    #[must_use]
    pub fn worktree(&self) -> &Path {
        &self.worktree
    }

    /// The `docker run` argv for this session, without the leading `docker`.
    ///
    /// See the module doc comment, "The isolation flags `ContainerSpec::run_argv`
    /// chooses, and why," for each flag's reasoning. No `--rm`: this module
    /// does not decide whether a container is removed on exit or kept for a
    /// human to inspect, the same restraint `crates/ori-runtime/src/worktree.rs`'s
    /// `remove_argv` documents for not passing `--force`, and teardown is out
    /// of this ticket's declared scope regardless.
    #[must_use]
    pub fn run_argv(&self) -> Vec<String> {
        vec![
            "run".to_owned(),
            "--name".to_owned(),
            self.name.clone(),
            "--network".to_owned(),
            "none".to_owned(),
            "--read-only".to_owned(),
            "--cap-drop".to_owned(),
            "ALL".to_owned(),
            "--security-opt".to_owned(),
            "no-new-privileges".to_owned(),
            "--user".to_owned(),
            format!("{}:{}", self.uid, self.gid),
            "--volume".to_owned(),
            format!("{}:{}:rw", self.worktree.to_string_lossy(), self.workdir),
            "--workdir".to_owned(),
            self.workdir.clone(),
            self.image.clone(),
        ]
    }
}

// ---------------------------------------------------------------------------
// ContainerError
// ---------------------------------------------------------------------------

/// Everything this module refuses or cannot do: ORI-P1-032, AICD §17.
///
/// The split mirrors `ori_broker::issuance::IssuanceError` and
/// `crate::worktree::WorktreeError`: [`ContainerError::Tier2RequiresContainer`]
/// is a control refusing an action and carries a [`MethodologyRef`]; the rest
/// are a value that did not parse or the event log reporting its own
/// failure, and neither is a control refusing anything, so neither cites a
/// section.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContainerError {
    /// A tier 2 ticket, or a launch whose tier is not known, cannot start
    /// worktree-only. `spec/SECURITY_NOTES.md` "Failure handling": "tier 2
    /// tickets refuse to start in that mode."
    Tier2RequiresContainer {
        /// The tier the launch carried, absent when the caller did not know
        /// it.
        tier: Option<Tier>,
    },
    /// A value given to this module did not parse into the shape named by
    /// `what`.
    Malformed {
        /// The field or shape the value was read as.
        what: &'static str,
        /// The value as given.
        value: String,
    },
    /// The event log refused or failed [`record_downgrade`]'s append.
    EventLog(EventLogError),
}

impl ContainerError {
    fn malformed(what: &'static str, value: impl Into<String>) -> Self {
        Self::Malformed {
            what,
            value: value.into(),
        }
    }

    /// The methodology section this refusal rests on, for the refusal and
    /// for nothing else.
    ///
    /// [`ContainerError::Tier2RequiresContainer`] cites AICD §17: this
    /// crate's own `ori_core::types::Tier` doc comment already derives `Tier`
    /// itself from "the autonomy tiers and permission model in AICD §17,"
    /// and `spec/SECURITY_NOTES.md`'s own "Authorization model" section
    /// names the same section as the source of the permission matrix this
    /// refusal enforces at the isolation boundary: "The permission matrix of
    /// AICD §17 is the source."
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Tier2RequiresContainer { .. } => Some(MethodologyRef {
                section: 17,
                subsection: None,
            }),
            Self::Malformed { .. } => None,
            Self::EventLog(inner) => inner.methodology_ref(),
        }
    }

    /// Whether a control refused the action, as opposed to a value failing
    /// to parse or the event log reporting an unrelated failure.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::Tier2RequiresContainer { .. })
    }
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tier2RequiresContainer { tier } => write!(
                f,
                "refused: a tier 2 ticket cannot start worktree-only ({})",
                match tier {
                    Some(tier) => format!("tier is {tier}"),
                    None => "tier is not known".to_owned(),
                }
            ),
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
            Self::EventLog(inner) => write!(f, "{inner}"),
        }
    }
}

impl StdError for ContainerError {}

impl From<EventLogError> for ContainerError {
    fn from(error: EventLogError) -> Self {
        Self::EventLog(error)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use super::*;

    // -----------------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0031-{label}-{}-{unique}",
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

    fn open_db(scratch: &Scratch, product_id: &Id) -> ProductDb {
        ProductDb::open(&scratch.path, product_id.as_str(), at(1_000))
            .expect("a fresh product database opens")
    }

    fn request(product_id: &Id) -> NewDowngrade {
        NewDowngrade {
            product_id: product_id.clone(),
            session_id: id("SESSION"),
            ticket_id: Some(id("TICKET")),
        }
    }

    /// A probe that reports one fixed outcome, whatever budget it is given.
    struct FixedProbe(ProbeOutcome);

    impl RuntimeProbe for FixedProbe {
        fn probe(&self, _budget: Duration) -> ProbeOutcome {
            self.0.clone()
        }
    }

    fn exited(status: i32, stdout: &str, stderr: &str) -> ProbeOutcome {
        ProbeOutcome::Exited {
            status: Some(status),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
        }
    }

    // -----------------------------------------------------------------------
    // classify: plant 1, "detection ignores the daemon"
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_032_a_usable_daemon_is_detected_usable() {
        let detection = classify(exited(0, "29.7.2\n", ""));
        assert_eq!(detection, Detection::Usable);
        assert!(detection.is_usable());
        assert!(detection.reason().is_none());
    }

    #[test]
    fn ori_t_0031_daemon_down_with_cli_present_is_not_usable() {
        // Real docker on this machine, with DOCKER_HOST pointed at a socket
        // that does not exist, reported exactly this shape: exit 1, a
        // "failed to connect" message on stderr, nothing on stdout. A
        // detector that reports usable as soon as the command merely starts
        // (planted defect 1: "CLI present means usable") would call this
        // Detection::Usable; this assertion is what catches that.
        let detection = classify(exited(
            1,
            "",
            "failed to connect to the docker API at unix:///tmp/nonexistent.sock",
        ));
        assert_eq!(
            detection,
            Detection::Absent {
                reason: "docker exited 1: failed to connect to the docker API at \
                          unix:///tmp/nonexistent.sock"
                    .to_owned(),
            }
        );
        assert!(!detection.is_usable());
    }

    #[test]
    fn ori_t_0031_a_missing_cli_is_absent_not_unsure() {
        assert_eq!(
            classify(ProbeOutcome::NotFound),
            Detection::Absent {
                reason: "docker is not on PATH".to_owned(),
            }
        );
    }

    #[test]
    fn ori_t_0031_a_clean_exit_with_no_output_is_unsure_not_usable() {
        // "unexpected output": a status 0 with nothing to show for it is not
        // trusted as a real answer either way.
        let detection = classify(exited(0, "  \n", ""));
        assert!(!detection.is_usable());
        assert!(matches!(detection, Detection::Unsure { .. }));
    }

    #[test]
    fn ori_t_0031_a_timeout_is_unsure_never_usable_and_never_silent() {
        let detection = classify(ProbeOutcome::TimedOut);
        assert!(!detection.is_usable());
        assert!(matches!(detection, Detection::Unsure { .. }));
        assert!(
            detection.reason().is_some_and(|reason| !reason.is_empty()),
            "an unsure detection always carries a reason"
        );
    }

    #[test]
    fn ori_t_0031_permission_denied_reaching_the_socket_is_unsure_not_absent() {
        let detection = classify(exited(
            1,
            "",
            "permission denied while trying to connect to the Docker daemon socket",
        ));
        assert!(
            matches!(detection, Detection::Unsure { .. }),
            "{detection:?}"
        );
    }

    #[test]
    fn ori_t_0031_a_spawn_permission_error_is_unsure() {
        assert!(matches!(
            classify(ProbeOutcome::PermissionDenied),
            Detection::Unsure { .. }
        ));
    }

    #[test]
    fn ori_t_0031_a_signal_exit_is_unsure() {
        assert!(matches!(
            classify(ProbeOutcome::Exited {
                status: None,
                stdout: String::new(),
                stderr: String::new(),
            }),
            Detection::Unsure { .. }
        ));
    }

    #[test]
    fn ori_t_0031_an_io_failure_is_unsure() {
        assert!(matches!(
            classify(ProbeOutcome::Io {
                message: "broken pipe".to_owned(),
            }),
            Detection::Unsure { .. }
        ));
    }

    // -----------------------------------------------------------------------
    // decide_mode
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_032_every_detection_maps_to_exactly_one_mode() {
        let (mode, reason) = decide_mode(&Detection::Usable);
        assert_eq!(mode, Mode::Container);
        assert!(reason.is_none());

        for detection in [
            Detection::Absent {
                reason: "no docker".to_owned(),
            },
            Detection::Unsure {
                reason: "timed out".to_owned(),
            },
        ] {
            let (mode, reason) = decide_mode(&detection);
            assert_eq!(
                mode,
                Mode::WorktreeOnly,
                "{detection:?} downgrades to worktree-only"
            );
            assert!(reason.is_some(), "{detection:?} carries a reason");
        }
    }

    #[test]
    fn ori_t_0031_absent_and_unsure_are_told_apart_in_the_downgrade_reason() {
        let (_, absent) = decide_mode(&Detection::Absent {
            reason: "no docker".to_owned(),
        });
        assert_eq!(absent.expect("a reason").kind(), "absent");

        let (_, unsure) = decide_mode(&Detection::Unsure {
            reason: "timed out".to_owned(),
        });
        assert_eq!(unsure.expect("a reason").kind(), "unsure");
    }

    // -----------------------------------------------------------------------
    // The tier 2 refusal: plants 3, 4 and 5
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_032_a_tier_2_ticket_refuses_to_start_worktree_only() {
        let refusal = refuse_tier2_in_worktree_only(Mode::WorktreeOnly, Some(Tier::Two))
            .expect_err("a tier 2 ticket does not start worktree-only");
        assert_eq!(
            refusal,
            ContainerError::Tier2RequiresContainer {
                tier: Some(Tier::Two)
            }
        );
        assert!(refusal.is_refusal());
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(17)
        );
        assert!(
            refusal
                .methodology_ref()
                .expect("cites AICD §17")
                .resolves()
        );
    }

    #[test]
    fn ori_p1_032_an_unknown_tier_refuses_worktree_only_and_is_not_read_as_tier_zero() {
        let refusal = refuse_tier2_in_worktree_only(Mode::WorktreeOnly, None)
            .expect_err("an unknown tier is not treated as tier 0");
        assert_eq!(
            refusal,
            ContainerError::Tier2RequiresContainer { tier: None }
        );
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(17)
        );

        // The direct comparison this defence exists for: an unknown tier and
        // an explicit tier 0 are refused differently, so a mutation that
        // reads "tier unknown" as "tier 0" is caught by this pair, not only
        // by the assertion above in isolation.
        assert!(refuse_tier2_in_worktree_only(Mode::WorktreeOnly, Some(Tier::Zero)).is_ok());
        assert!(refuse_tier2_in_worktree_only(Mode::WorktreeOnly, None).is_err());
    }

    #[test]
    fn ori_t_0031_tier_zero_and_tier_one_start_worktree_only() {
        assert!(refuse_tier2_in_worktree_only(Mode::WorktreeOnly, Some(Tier::Zero)).is_ok());
        assert!(refuse_tier2_in_worktree_only(Mode::WorktreeOnly, Some(Tier::One)).is_ok());
    }

    #[test]
    fn ori_t_0031_container_mode_refuses_nothing_whatever_the_tier() {
        for tier in [None, Some(Tier::Zero), Some(Tier::One), Some(Tier::Two)] {
            assert!(
                refuse_tier2_in_worktree_only(Mode::Container, tier).is_ok(),
                "{tier:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // resolve_launch, end to end: plant 2 and trap 5 (vacuity)
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_032_worktree_only_writes_exactly_one_downgrade_event_and_container_mode_writes_none()
    {
        let scratch = Scratch::new("resolve");
        let product = id("PRODUCT");
        let mut db = open_db(&scratch, &product);

        // A baseline event, so the log is not empty and "no downgrade event"
        // cannot be mistaken for "an empty log with nothing in it to miss"
        // (CLAUDE.md's vacuity trap 5).
        EventLog::append(
            db.connection(),
            product.clone(),
            at(1),
            Actor::System,
            "ticket.queued",
            None,
            "{}",
        )
        .expect("a baseline event appends");
        let before = EventLog::verify(db.connection()).expect("verifies");
        assert_eq!(before.events_checked, 1);

        // Container runtime present: resolve_launch must write no event at
        // all, proven by an exact tip_seq comparison against the baseline
        // above, not by searching for "no container.downgraded events" in
        // what could otherwise be an empty or unrelated log.
        let usable = FixedProbe(exited(0, "29.7.2\n", ""));
        let decision = resolve_launch(
            &mut db,
            &usable,
            DEFAULT_PROBE_BUDGET,
            at(2),
            Actor::System,
            &request(&product),
            Some(Tier::Two),
        )
        .expect("a usable runtime and a tier 2 ticket both start in container mode");
        assert_eq!(decision.mode, Mode::Container);
        assert!(decision.banner.is_none());
        let after_container = EventLog::verify(db.connection()).expect("verifies");
        assert_eq!(
            after_container.events_checked, before.events_checked,
            "container mode wrote no event: the log is exactly as long as before"
        );

        // Container runtime absent: exactly one more event than the
        // baseline, and it is the downgrade.
        let absent = FixedProbe(ProbeOutcome::NotFound);
        let decision = resolve_launch(
            &mut db,
            &absent,
            DEFAULT_PROBE_BUDGET,
            at(3),
            Actor::System,
            &request(&product),
            Some(Tier::One),
        )
        .expect("a tier 1 ticket starts worktree-only");
        assert_eq!(decision.mode, Mode::WorktreeOnly);
        let banner = decision.banner.expect("a downgrade banner");
        assert_eq!(banner.headline, DowngradeBanner::HEADLINE);
        assert_eq!(banner.reason.kind(), "absent");

        let after_downgrade = EventLog::verify(db.connection()).expect("verifies");
        assert_eq!(
            after_downgrade.events_checked,
            before.events_checked + 1,
            "exactly one event was added by the downgrade, no more and no fewer"
        );
        let written = EventLog::read_range(
            db.connection(),
            after_downgrade.tip_seq.expect("a tip"),
            after_downgrade.tip_seq.expect("a tip"),
        )
        .expect("reads the last event");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].kind(), "container.downgraded");
        assert_eq!(written[0].ticket_id(), Some(&id("TICKET")));
        assert!(written[0].payload().contains("\"reason\":\"absent\""));
    }

    #[test]
    fn ori_p1_032_a_tier_2_ticket_downgrading_is_refused_but_the_event_is_still_recorded() {
        // The refusal and the event are independent: the event describes
        // what the environment looked like, and it is recorded whether or
        // not the launch itself is then allowed to proceed.
        let scratch = Scratch::new("refused");
        let product = id("PRODUCT");
        let mut db = open_db(&scratch, &product);
        let absent = FixedProbe(ProbeOutcome::NotFound);

        let refusal = resolve_launch(
            &mut db,
            &absent,
            DEFAULT_PROBE_BUDGET,
            at(1),
            Actor::System,
            &request(&product),
            Some(Tier::Two),
        )
        .expect_err("a tier 2 ticket does not start worktree-only");
        assert_eq!(
            refusal,
            ContainerError::Tier2RequiresContainer {
                tier: Some(Tier::Two)
            }
        );

        let report = EventLog::verify(db.connection()).expect("verifies");
        assert_eq!(
            report.events_checked, 1,
            "the downgrade event was written even though the launch was refused"
        );
    }

    #[test]
    fn ori_t_0031_an_unknown_tier_downgrading_is_refused_the_same_way() {
        let scratch = Scratch::new("unknown-tier");
        let product = id("PRODUCT");
        let mut db = open_db(&scratch, &product);
        let absent = FixedProbe(ProbeOutcome::NotFound);

        let refusal = resolve_launch(
            &mut db,
            &absent,
            DEFAULT_PROBE_BUDGET,
            at(1),
            Actor::System,
            &request(&product),
            None,
        )
        .expect_err("an unknown tier is refused, not treated as tier 0");
        assert_eq!(
            refusal,
            ContainerError::Tier2RequiresContainer { tier: None }
        );
    }

    // -----------------------------------------------------------------------
    // Not cached forever
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0031_a_launch_that_finds_docker_back_uses_the_container_the_very_next_call() {
        let scratch = Scratch::new("not-cached");
        let product = id("PRODUCT");
        let mut db = open_db(&scratch, &product);

        let absent = FixedProbe(ProbeOutcome::NotFound);
        let first = resolve_launch(
            &mut db,
            &absent,
            DEFAULT_PROBE_BUDGET,
            at(1),
            Actor::System,
            &request(&product),
            Some(Tier::Zero),
        )
        .expect("tier 0 starts worktree-only");
        assert_eq!(first.mode, Mode::WorktreeOnly);

        // No field, static or cache anywhere in this module remembers that:
        // the very next call, with docker back, is Mode::Container.
        let usable = FixedProbe(exited(0, "29.7.2\n", ""));
        let second = resolve_launch(
            &mut db,
            &usable,
            DEFAULT_PROBE_BUDGET,
            at(2),
            Actor::System,
            &request(&product),
            Some(Tier::Zero),
        )
        .expect("docker is back");
        assert_eq!(second.mode, Mode::Container);
        assert!(second.banner.is_none());
    }

    // -----------------------------------------------------------------------
    // ContainerSpec: the docker run argv
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_032_the_run_argv_mounts_the_worktree_and_carries_no_rm_no_secret() {
        let worktree = std::env::temp_dir().join("ori-t-0031-worktree");
        let spec = ContainerSpec::for_session(&id("SESSION"), "ori/coder:latest", &worktree)
            .expect("a valid spec");
        assert_eq!(
            spec.name(),
            format!("ori-session-{}", id("SESSION").as_str())
        );
        assert_eq!(spec.worktree(), worktree);

        let argv = spec.run_argv();
        assert_eq!(argv[0], "run");
        assert!(argv.contains(&"--network".to_owned()));
        assert!(argv.contains(&"none".to_owned()));
        assert!(argv.contains(&"--read-only".to_owned()));
        assert!(argv.contains(&"--cap-drop".to_owned()));
        assert!(argv.contains(&"ALL".to_owned()));
        assert!(argv.contains(&"--security-opt".to_owned()));
        assert!(argv.contains(&"no-new-privileges".to_owned()));
        assert!(argv.contains(&"--user".to_owned()));
        assert!(argv.contains(&format!(
            "{}:{}",
            ContainerSpec::UNPRIVILEGED_UID,
            ContainerSpec::UNPRIVILEGED_GID
        )));
        let volume = format!(
            "{}:{}:rw",
            worktree.to_string_lossy(),
            ContainerSpec::WORKDIR
        );
        assert!(argv.contains(&volume));
        assert!(argv.last().is_some_and(|last| last == "ori/coder:latest"));
        assert!(
            !argv.iter().any(|arg| arg == "--rm"),
            "teardown is not this module's decision"
        );
        assert!(
            !argv.iter().any(|arg| arg.contains("--env")),
            "credential injection is ORI-T-0032's, not built here"
        );
    }

    #[test]
    fn ori_t_0031_a_relative_worktree_is_refused() {
        let error = ContainerSpec::for_session(&id("SESSION"), "img", "relative/path")
            .expect_err("a relative path is refused");
        assert!(matches!(
            error,
            ContainerError::Malformed {
                what: "worktree path",
                ..
            }
        ));
        assert!(!error.is_refusal());
        assert!(error.methodology_ref().is_none());
    }

    #[test]
    fn ori_t_0031_an_empty_image_is_refused() {
        let worktree = std::env::temp_dir().join("ori-t-0031-worktree-2");
        let error = ContainerSpec::for_session(&id("SESSION"), "   ", &worktree)
            .expect_err("an empty image is refused");
        assert!(matches!(
            error,
            ContainerError::Malformed { what: "image", .. }
        ));
    }

    // -----------------------------------------------------------------------
    // Every refusal carries a resolving reason, or none at all
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0031_every_error_this_module_makes_is_classified_correctly() {
        let errors = [
            ContainerError::Tier2RequiresContainer {
                tier: Some(Tier::Two),
            },
            ContainerError::Tier2RequiresContainer { tier: None },
            ContainerError::Malformed {
                what: "image",
                value: String::new(),
            },
        ];
        let mut refusals = 0;
        for error in errors {
            assert!(!error.to_string().is_empty());
            match error.methodology_ref() {
                Some(reason) => {
                    assert!(error.is_refusal(), "{error:?}");
                    assert!(reason.resolves(), "{reason} resolves");
                    refusals += 1;
                }
                None => assert!(!error.is_refusal(), "{error:?}"),
            }
        }
        assert_eq!(refusals, 2, "the two tier refusals are the controls here");
    }

    // -----------------------------------------------------------------------
    // The real probe: never depended on by the rest of the suite
    // -----------------------------------------------------------------------

    #[test]
    #[ignore = "talks to a real Docker daemon; run locally with `cargo test -- --ignored` \
                on a machine that has one. Read-only: runs only `docker version`, the same \
                check SystemDocker's own probe issues, and creates nothing."]
    fn ori_t_0031_real_docker_probe_reports_usable_when_docker_answers() {
        let detection = detect(&SystemDocker::new(), DEFAULT_PROBE_BUDGET);
        assert!(
            detection.is_usable(),
            "this machine had a usable Docker daemon when this ticket was written: {detection:?}"
        );
    }

    #[test]
    fn ori_t_0031_detection_never_runs_a_real_process_outside_the_ignored_test_above() {
        // A structural note, not an assertion the compiler can check for us:
        // every other test in this module drives classify/decide_mode/
        // resolve_launch through FixedProbe, never through SystemDocker.
        // Grep for `SystemDocker` in this file's #[cfg(test)] block outside
        // the ignored test above to confirm.
        let never_touches_docker = FixedProbe(exited(0, "0.0.0\n", ""));
        assert!(classify(never_touches_docker.probe(DEFAULT_PROBE_BUDGET)).is_usable());
    }
}
