//! The headless CLI adapter: AICD §17, ADR-0001 "Agent protocol", PRD I-04
//! and P-02, `spec/API_SPEC.md` §4, criterion ORI-P1-039.
//!
//! ADR-0001's decision row: "Agent protocol | ACP client; headless CLI
//! adapter trait for runtimes without ACP." [`HeadlessAdapter`] is that
//! fallback: a runtime with no JSON-RPC channel to negotiate a permission
//! refusal over (`crate::acp`), launched instead with whatever non-interactive
//! flags its own CLI defines, and with standard input closed so that no
//! question it tries to ask can ever be answered, or even received.
//!
//! # What "prompting" can mean with stdin closed
//!
//! A CLI without a protocol channel back to this client has exactly one way
//! left to ask a human something: block a read on file descriptor 0 (a
//! `y/n` confirmation, a `readline` loop, a pager) and wait for a byte that
//! answers it. [`HeadlessAdapter::spawn`] gives that file descriptor
//! [`std::process::Stdio::null`] ("`/dev/null`" on unix, "`NUL`" on Windows,
//! both platforms this product's own [`std::process::Stdio::null`]
//! documentation names): a read against either returns end-of-file, zero
//! bytes, at once, on every platform this product ships to. The read call
//! cannot suspend the calling thread waiting on input that will never
//! arrive, because the kernel answers it immediately rather than queuing it
//! behind a producer that does not exist. This is a structural guarantee,
//! not a runtime check: there is no `if` anywhere in this module that
//! decides "this looks like a prompt" by matching output text, which would
//! be the exact pattern-matching trap `crate::transcript`'s own module doc
//! comment rejects for secrets, applied here to a different kind of content.
//! Prompting is made *impossible*, not *detected*.
//!
//! What this does not, and cannot, make impossible: a CLI that reads
//! end-of-file on what would have been a confirmation prompt and *fails
//! open* (proceeds as though permission were granted) rather than *fails
//! closed* (refuses and exits). That is a property of the CLI's own flag
//! choice; the caller is responsible for including the CLI's real
//! non-interactive/auto-approve flag (a `--dangerously-skip-permissions` or
//! equivalent) in [`LaunchConfig::args`](crate::acp::LaunchConfig::args),
//! which is inspectable, not this module's to guess for a CLI it does not
//! control (see `HeadlessAdapter::spawn`'s own doc comment).
//!
//! What this module *can* and does detect, as defense in depth: a run that
//! does not finish within `RUN_TIMEOUT` is killed and reported as
//! [`crate::acp::RuntimeError::Timeout`], covering the residual case of a CLI
//! that busy-polls a closed stdin (reading zero bytes over and over) rather
//! than genuinely blocking on it. A run with everything it needs from argv
//! finishes in bounded time; one still running past that bound is, from the
//! outside, indistinguishable from one stuck waiting for a human, and is
//! treated the same way rather than merely as an ordinary slow process.
//!
//! # The deadline covers the whole run, reading included
//!
//! "Finishes" means two things, both inside the one deadline that
//! [`HeadlessAdapter`]'s `send` starts: the child has exited, **and** its
//! standard output has reached end-of-file. The second is not implied by the
//! first. A read of a pipe returns end-of-file only when *every* holder of the
//! pipe's write end has closed it, so a child that keeps its standard output
//! open while it sleeps, or a descendant it leaves behind that inherited the
//! pipe, keeps a plain read blocked for as long as that holder lives. Reading
//! first and starting the clock afterwards (the shape this module had before
//! ORI-T-0140) therefore left the clock unstarted for exactly the runs it
//! exists to bound: `sh -c 'sleep 23'` came back as an ordinary success after
//! 23 seconds, with no timeout and no launch defect.
//!
//! Output is now collected on a reader thread that `send` starts before it
//! waits, and `send` waits on the child and then on the reader's result, both
//! against the same deadline:
//!
//! ```mermaid
//! flowchart TD
//!   S[send: start the deadline, start the reader thread] --> W{child exited<br/>before the deadline?}
//!   W -- no --> K[kill and reap the child, abandon the reader]
//!   W -- yes --> R{reader reached end-of-file<br/>before the deadline?}
//!   R -- no --> A[abandon the reader; the child is already gone]
//!   R -- yes --> OK[stop reason from the exit status, output recorded]
//!   K --> T[record the launch defect and the transcript entry,<br/>return RuntimeError::Timeout]
//!   A --> T
//! ```
//!
//! A run whose child exited but whose output never closed in time is
//! refused the same way as one whose child never exited, rather than
//! reported as a success carrying output that may be incomplete: the same
//! method (`headless.run`), the same
//! [`crate::acp::LaunchDefect::reason`], the same transcript entry shape, the
//! same error; only the defect's detail text differs, to say which of the two
//! it was.
//!
//! **What is cleaned up, and what is not.**
//!
//! - The child this module spawned is killed and reaped at the deadline, on
//!   every platform (`crate::acp::kill_child`).
//! - The reader thread is *abandoned*, not stopped: `std` has no portable way
//!   to interrupt a blocked read, and this crate takes no dependency to get
//!   one. It owns only the read end of the pipe and a small buffer, never a
//!   session, a lock or a credential, and it ends by itself when the last
//!   holder of the write end exits or closes it. What it keeps is bounded
//!   (`STDOUT_RETAIN` bytes; the rest is read and discarded so a chatty
//!   child never blocks on a full pipe), so an abandoned reader cannot grow
//!   without limit while it waits.
//! - A **descendant** of the child (a grandchild the child started and did
//!   not wait for) is *not* killed by this module: `std` can kill only the
//!   process it spawned, and a process-group kill needs a platform call this
//!   crate does not make. An orphan left behind is the business of the
//!   runtime's container and of session teardown, not of this module: in a
//!   container session, removing the container ends every process in it, and
//!   `crate::container` leaves that removal to the session's teardown
//!   ordering (`crate::session::Teardown`). In the worktree-only
//!   downgrade (`spec/SECURITY_NOTES.md`, "Failure handling") there is no
//!   container to remove, and nothing in this crate reaches a descendant
//!   there today; that gap is named, not closed, here. What this module does
//!   guarantee is the narrower, load-bearing thing: such an orphan cannot
//!   keep `send` blocked past the deadline.
//!
//! Windows: the deadline logic is `std` only and behaves the same, with the
//! same two limits (only the spawned child is terminated; a descendant that
//! holds the pipe makes the reader linger until it exits). The tests that
//! need a shell to leave a descendant behind are unix-only; the test that
//! needs only the test binary itself to hold the pipe is not.
//!
//! # Why this file shares its types with `crate::acp`
//!
//! `LaunchConfig`, `StdioMode`, `SessionSpec`, `SessionHandle`,
//! `AgentRuntime`, `RuntimeError`, `LaunchDefect`, `StopReason`,
//! `RuntimeCaps` and the process helpers (`crate::acp::wait_with_timeout`,
//! `crate::acp::kill_child`, `crate::acp::drain_stderr`) are declared in
//! `crate::acp` and reused here rather than duplicated: `spec/API_SPEC.md`
//! §4 gives both adapters one trait, and a second, incompatible
//! `LaunchConfig` in this file would make "inspect the runtime launch
//! configuration" (ORI-P1-039) mean two different things depending on which
//! adapter answered. [`LaunchConfig::headless`] is this file's own addition
//! to that shared type, an inherent-impl block in a different module from
//! the type's definition, legal because both live in this one crate; it is
//! the only constructor in the whole crate that builds a [`StdioMode::Null`]
//! launch configuration.
//!
//! # The seam with `injector.rs` (ORI-T-0032, in flight)
//!
//! Exactly `crate::acp`'s: [`SessionSpec::envs`] is accepted as prepared and
//! handed to [`std::process::Command::envs`], never read, never logged, never
//! the source of a decision this module makes. See `crate::acp`'s own module
//! doc comment for the fuller statement, which applies here unchanged.
//!
//! # One shot, not a turn-based session
//!
//! A headless CLI with no protocol channel has no way to receive a *second*
//! prompt after it starts: there is no stdin to write one to (it is closed,
//! by design, for the reason above) and no notion of "session" beyond the
//! one process. So the prompt this adapter's `AgentRuntime::send` takes is
//! not written anywhere at call time; the real prompt must already be part
//! of [`LaunchConfig::args`](crate::acp::LaunchConfig::args) when
//! [`HeadlessAdapter::spawn`] is called (the ordinary shape for a
//! non-interactive CLI: `claude -p "<prompt>" --dangerously-skip-permissions`,
//! say). [`HeadlessAdapter::send`]'s first call is what actually waits for
//! that one run to finish and harvests its captured output; a second call is
//! refused ([`crate::acp::RuntimeError::AlreadyCompleted`]), because there is
//! no second turn to run.
//!
//! Must not: read a keychain or configuration for a credential; log or
//! record an environment value; spawn a process outside this crate's own
//! functions (CLAUDE.md: "Only `ori-runtime` spawns processes").

use std::io::ErrorKind;
use std::io::Read;
use std::process::Child;
use std::process::ChildStdout;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use ori_core::types::Id;
use ori_core::types::Timestamp;

use crate::acp::AgentRuntime;
use crate::acp::ClockFn;
use crate::acp::DefectRecorder;
use crate::acp::LaunchConfig;
use crate::acp::LaunchConfigError;
use crate::acp::LaunchDefect;
use crate::acp::RuntimeCaps;
use crate::acp::RuntimeError;
use crate::acp::SessionHandle;
use crate::acp::SessionSpec;
use crate::acp::StdioMode;
use crate::acp::StopReason;
use crate::acp::drain_stderr;
use crate::acp::kill_child;
use crate::acp::wait_with_timeout;
use crate::transcript::Entry;
use crate::transcript::Transcript;

/// How long a one-shot headless run may execute before it is killed and
/// reported as [`RuntimeError::Timeout`]: defense in depth against a CLI
/// that busy-polls its closed stdin rather than blocking on it. See the
/// module doc comment, "What 'prompting' can mean with stdin closed".
const RUN_TIMEOUT: Duration = Duration::from_secs(20);

/// How much of a one-shot run's captured stdout is kept in the transcript
/// entry: the same bound and reasoning as `crate::acp::STDERR_CAP`.
const STDOUT_CAP: usize = 4000;

/// How many raw bytes of a run's stdout the reader thread keeps. Everything
/// past this is read and thrown away, so a chatty child never blocks on a full
/// pipe and an abandoned reader never grows without limit; see the module doc
/// comment, "The deadline covers the whole run, reading included".
///
/// `STDOUT_CAP` plus 8 bytes of slack for UTF-8: the transcript text is
/// `String::from_utf8_lossy` of these bytes, cut at `STDOUT_CAP` bytes of
/// *that text*. Lossy decoding never makes text shorter than its input (each
/// invalid run of up to 3 bytes becomes one 3-byte replacement character) and
/// a sequence is at most 4 bytes, so cutting the raw bytes 8 past the cap
/// leaves the first `STDOUT_CAP` bytes of the text exactly what they would
/// have been had every byte been kept.
const STDOUT_RETAIN: usize = STDOUT_CAP + 8;

impl LaunchConfig {
    /// The launch configuration [`HeadlessAdapter::spawn`] requires:
    /// standard input closed. See this module's own doc comment, "What
    /// 'prompting' can mean with stdin closed".
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
}

/// One headless session's process and transcript state.
struct ActiveHeadlessSession {
    id: Id,
    child: Child,
    /// The child's stdout until `send` hands it to the reader thread; `None`
    /// afterwards. A one-shot run is read once, so it is moved, not shared.
    stdout: Option<ChildStdout>,
    completed: bool,
    transcript: Transcript,
}

/// A headless CLI adapter, behind [`AgentRuntime`]: ADR-0001 "Agent
/// protocol", `spec/API_SPEC.md` §4.
pub struct HeadlessAdapter {
    caps: RuntimeCaps,
    session: Option<ActiveHeadlessSession>,
    record_defect: DefectRecorder,
    clock: ClockFn,
}

impl HeadlessAdapter {
    /// An adapter declaring `caps`, with no session spawned yet.
    ///
    /// `record_defect` and `clock` are required here, once, rather than at
    /// each call: see `crate::acp`'s own module doc comment, "Why the
    /// recorder and the clock are constructor parameters, not per-call
    /// ones" (fix 3 of this ticket's follow-up review). There is no other
    /// constructor and no `Default`, so a `HeadlessAdapter` cannot exist
    /// without both, and its one possible defect (a run that times out,
    /// see `AgentRuntime::send`'s own doc comment) is always recorded
    /// through a real recorder.
    pub fn new(
        caps: RuntimeCaps,
        record_defect: impl FnMut(LaunchDefect) -> Result<(), String> + 'static,
        clock: impl FnMut() -> Timestamp + 'static,
    ) -> Self {
        Self {
            caps,
            session: None,
            record_defect: Box::new(record_defect),
            clock: Box::new(clock),
        }
    }

    /// The transcript recorded for the active session, absent before
    /// `spawn`.
    #[must_use]
    pub fn transcript(&self) -> Option<&Transcript> {
        self.session.as_ref().map(|session| &session.transcript)
    }

    /// `AgentRuntime::send` with the deadline as a parameter, so that a test
    /// can exercise the whole-run bound in a second rather than in
    /// `RUN_TIMEOUT`'s twenty. `send` is this function called with
    /// `RUN_TIMEOUT`; nothing else calls it.
    fn send_within(
        &mut self,
        handle: &SessionHandle,
        prompt: &str,
        timeout: Duration,
    ) -> Result<StopReason, RuntimeError> {
        let at = (self.clock)();
        let session = match &mut self.session {
            Some(session) if &session.id == handle.id() => session,
            Some(_) | None => {
                return Err(RuntimeError::UnknownSession {
                    id: handle.id().clone(),
                });
            }
        };
        if session.completed {
            return Err(RuntimeError::AlreadyCompleted);
        }
        // The pipe is moved to the reader thread, so this run can be read
        // exactly once: from here on the session is spent, whatever happens.
        let stdout = session.stdout.take().ok_or_else(|| RuntimeError::Io {
            message: "the child's stdout was already taken".to_owned(),
        })?;
        session.completed = true;

        let (status, output) = match run_to_end(&mut session.child, stdout, timeout)? {
            RunEnd::Finished { status, output } => (status, output),
            // Indistinguishable, from here, from a runtime blocked on a
            // prompt it should never have raised: recorded the same way.
            // See this module's own doc comment, "What 'prompting' can mean
            // with stdin closed".
            RunEnd::ChildStillRunning => {
                return refuse_timeout(
                    &mut self.record_defect,
                    &mut session.transcript,
                    at,
                    prompt,
                    timeout,
                    format!(
                        "the headless run did not exit within {timeout:?}; killed and refused \
                         rather than left to keep waiting on a prompt it should never have raised"
                    ),
                );
            }
            RunEnd::OutputStillOpen => {
                return refuse_timeout(
                    &mut self.record_defect,
                    &mut session.transcript,
                    at,
                    prompt,
                    timeout,
                    format!(
                        "the headless run's child exited but its standard output was still held \
                         open {timeout:?} after the run began, so its output never ended; refused \
                         rather than reported as complete"
                    ),
                );
            }
        };
        let captured = String::from_utf8_lossy(&output);
        let captured = capped(&captured);
        let stop_reason = if status.success() {
            StopReason::EndTurn
        } else {
            StopReason::Other(format!(
                "exit {}",
                status
                    .code()
                    .map_or_else(|| "signal".to_owned(), |code| code.to_string())
            ))
        };
        session.transcript = session.transcript.record(Entry::new(
            1,
            at,
            Some("headless.run"),
            prompt,
            &captured,
        )?)?;
        Ok(stop_reason)
    }
}

impl AgentRuntime for HeadlessAdapter {
    /// Launches the child with its non-interactive argv and standard input
    /// closed. Refuses ([`RuntimeError::WrongStdinMode`]) a `spec` whose
    /// [`LaunchConfig::stdin`](crate::acp::LaunchConfig::stdin) is not
    /// [`StdioMode::Null`], the same fail-closed check `crate::acp::AcpClient`
    /// makes for its own required mode, so a caller that assembled the wrong
    /// `LaunchConfig` is refused at `spawn` rather than launching a runtime
    /// that can prompt.
    ///
    /// Whatever `spec.launch` says, once accepted, the real
    /// [`std::process::Command`] this function builds is always given
    /// [`Stdio::null`] for standard input: the refusal above is a check on
    /// the caller's declaration, not the only thing standing between this
    /// module and an inherited terminal.
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

        let mut command = Command::new(spec.launch.program());
        command
            .args(spec.launch.args())
            .current_dir(&spec.cwd)
            .envs(spec.envs.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| RuntimeError::Io {
            message: error.to_string(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| RuntimeError::Io {
            message: "the child's stdout was not piped".to_owned(),
        })?;
        if let Some(stderr) = child.stderr.take() {
            drain_stderr(stderr);
        }

        let handle = SessionHandle::new(spec.id.clone());
        self.session = Some(ActiveHeadlessSession {
            id: spec.id,
            child,
            stdout: Some(stdout),
            completed: false,
            transcript: Transcript::new(),
        });
        Ok(handle)
    }

    /// Waits for the one-shot run to finish (bounded by `RUN_TIMEOUT`, which
    /// covers reading its output as well as waiting for it to exit) and
    /// records its captured output. See this module's own doc comment, "One
    /// shot, not a turn-based session", for why `prompt` is not written
    /// anywhere here and why a second call is refused, and "The deadline
    /// covers the whole run, reading included", for what is and is not
    /// cleaned up when the deadline passes.
    ///
    /// The recorder and clock this adapter was constructed with are used
    /// when the run does not finish within `RUN_TIMEOUT`, whether because
    /// the process does not exit or because its output never ends: from
    /// outside this module that is indistinguishable from a runtime blocked
    /// on a prompt it should never have been able to raise, so it is
    /// recorded the same way, over the same [`RuntimeError::methodology_ref`],
    /// AICD §17.
    fn send(&mut self, handle: &SessionHandle, prompt: &str) -> Result<StopReason, RuntimeError> {
        self.send_within(handle, prompt, RUN_TIMEOUT)
    }

    fn recv(&self, handle: &SessionHandle) -> Result<&[Entry], RuntimeError> {
        match &self.session {
            Some(session) if &session.id == handle.id() => Ok(session.transcript.entries()),
            Some(_) | None => Err(RuntimeError::UnknownSession {
                id: handle.id().clone(),
            }),
        }
    }

    fn kill(&mut self, handle: &SessionHandle) -> Result<(), RuntimeError> {
        let mut session = match self.session.take() {
            Some(session) if &session.id == handle.id() => session,
            Some(other) => {
                self.session = Some(other);
                return Err(RuntimeError::UnknownSession {
                    id: handle.id().clone(),
                });
            }
            None => {
                return Err(RuntimeError::UnknownSession {
                    id: handle.id().clone(),
                });
            }
        };
        let _ = kill_child(&mut session.child);
        Ok(())
    }

    fn capabilities(&self) -> RuntimeCaps {
        self.caps.clone()
    }
}

/// How a bounded run ended; see `run_to_end`.
enum RunEnd {
    /// The child exited and its stdout reached end-of-file, both before the
    /// deadline.
    Finished {
        /// How the child exited.
        status: ExitStatus,
        /// The first `STDOUT_RETAIN` bytes of what it wrote.
        output: Vec<u8>,
    },
    /// The deadline passed with the child still running. It was killed and
    /// reaped before this was returned.
    ChildStillRunning,
    /// The child exited, but something still held its stdout open when the
    /// deadline passed, so the output never ended. The reader thread was
    /// abandoned; see the module doc comment.
    OutputStillOpen,
}

/// Runs the deadline over the whole of a one-shot run: starts a reader thread
/// that collects `stdout`, waits for `child` to exit, then waits for the
/// reader to report end-of-file, all against the one deadline `timeout` after
/// this call begins. AICD §12 (an agent that hangs is stopped) and §17.
///
/// # Errors
///
/// [`RuntimeError::Io`] when the reader thread cannot be started (the child
/// is killed first, so nothing is left running unwatched), when polling the
/// child fails, when reading its output fails, or when the reader thread ends
/// without reporting. A timeout is not an error here: it is a [`RunEnd`] the
/// caller turns into a refusal.
fn run_to_end(
    child: &mut Child,
    stdout: ChildStdout,
    timeout: Duration,
) -> Result<RunEnd, RuntimeError> {
    let deadline = Instant::now() + timeout;
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("ori-headless-stdout".to_owned())
        .spawn(move || {
            // The receiver is gone when the run was abandoned; there is
            // nobody left to tell.
            let _ = sender.send(collect_bounded(stdout, STDOUT_RETAIN));
        });
    if let Err(error) = spawned {
        let _ = kill_child(child);
        return Err(RuntimeError::Io {
            message: format!("the stdout reader thread could not start: {error}"),
        });
    }

    let status = match wait_with_timeout(child, timeout) {
        Ok(status) => status,
        Err(RuntimeError::Timeout { .. }) => return Ok(RunEnd::ChildStillRunning),
        Err(error) => return Err(error),
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receiver.recv_timeout(remaining) {
        Ok(Ok(output)) => Ok(RunEnd::Finished { status, output }),
        Ok(Err(error)) => Err(RuntimeError::Io {
            message: error.to_string(),
        }),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(RunEnd::OutputStillOpen),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(RuntimeError::Io {
            message: "the stdout reader thread ended without reporting".to_owned(),
        }),
    }
}

/// Reads `source` to end-of-file, keeping the first `retain` bytes and
/// discarding the rest. Discarding is reading: the pipe is drained either
/// way, so a child that writes more than `retain` never blocks on a full
/// pipe. `Interrupted` is retried, as `Read::read_to_end` does.
fn collect_bounded(mut source: impl Read, retain: usize) -> std::io::Result<Vec<u8>> {
    let mut kept = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match source.read(&mut chunk) {
            Ok(0) => return Ok(kept),
            Ok(read) => {
                let room = retain.saturating_sub(kept.len());
                kept.extend_from_slice(&chunk[..read.min(room)]);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

/// Records a run that did not finish in time and refuses it: the launch
/// defect through the recorder the adapter was built with, then the refusal
/// in the transcript, then [`RuntimeError::Timeout`]. AICD §17 (the defect's
/// reason). Always `Err`, except when the recorder or the transcript itself
/// fails, in which case that failure is the error.
fn refuse_timeout(
    record_defect: &mut DefectRecorder,
    transcript: &mut Transcript,
    at: Timestamp,
    prompt: &str,
    timeout: Duration,
    detail: String,
) -> Result<StopReason, RuntimeError> {
    let defect = LaunchDefect {
        at,
        method: "headless.run",
        detail,
        reason: LaunchDefect::reason(),
    };
    (*record_defect)(defect.clone()).map_err(|message| RuntimeError::DefectSink { message })?;
    *transcript = transcript.record(Entry::new(
        1,
        at,
        Some("headless.run"),
        prompt,
        &format!("refused: timeout ({})", defect.reason),
    )?)?;
    Err(RuntimeError::Timeout { after: timeout })
}

/// A length-capped rendering of captured process output, for a transcript
/// entry: the same reasoning as `crate::acp::capped`, over stdout rather
/// than over one JSON value.
fn capped(text: &str) -> String {
    if text.len() <= STDOUT_CAP {
        return text.to_owned();
    }
    let mut end = STDOUT_CAP;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;
    use std::io::Write;
    use std::time::Instant;

    use ori_core::types::ModelFamily;

    use super::*;

    const START: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    fn id(tail: &str) -> Id {
        Id::parse(&format!("01ARZ3NDEKTSV4RRFFQ69{tail}")).expect("a ULID")
    }

    fn caps() -> RuntimeCaps {
        RuntimeCaps::new(
            ModelFamily::parse("openai-gpt-4").expect("a non-empty family parses"),
            "headless-fixture",
        )
        .expect("a valid name")
    }

    /// An adapter with a fixed clock (`START`) and the caller's own defect
    /// recorder: every test builds one this way now that both are
    /// constructor parameters (fix 3, ORI-T-0033's follow-up review), rather
    /// than a call-time no-op no code path can be left to substitute.
    fn adapter(
        record_defect: impl FnMut(LaunchDefect) -> Result<(), String> + 'static,
    ) -> HeadlessAdapter {
        HeadlessAdapter::new(caps(), record_defect, || START)
    }

    // -----------------------------------------------------------------------
    // The fixture: a re-exec of this test binary with a sentinel argument
    // and a sentinel environment variable (the same trick `crate::acp`'s
    // tests use), which tries to read one line from its own stdin and
    // reports what it got before exiting.
    // -----------------------------------------------------------------------

    const FIXTURE_MODE_VAR: &str = "ORI_T_0033_HEADLESS_FIXTURE_MODE";
    const FIXTURE_TEST_NAME: &str =
        "ori_t_0033_headless_fixture_agent_entrypoint_do_not_call_directly";

    #[test]
    fn ori_t_0033_headless_fixture_agent_entrypoint_do_not_call_directly() {
        if std::env::var(FIXTURE_MODE_VAR).is_err() {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        }
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut line = String::new();
        let read = input.read_line(&mut line).unwrap_or(0);
        let mut out = std::io::stdout();
        if read == 0 {
            let _ = writeln!(out, "{{\"stdin\":\"eof\"}}");
        } else {
            let _ = writeln!(out, "{{\"stdin\":\"data\",\"bytes\":{read}}}");
        }
        let _ = out.flush();
    }

    fn fixture_exe() -> String {
        std::env::current_exe()
            .expect("this test binary's own path")
            .to_string_lossy()
            .into_owned()
    }

    fn spawn_fixture_spec(id_tail: &str) -> SessionSpec {
        let launch = LaunchConfig::headless(
            fixture_exe(),
            vec![
                FIXTURE_TEST_NAME.to_owned(),
                "--test-threads=1".to_owned(),
                "--nocapture".to_owned(),
            ],
        )
        .expect("a non-empty program");
        SessionSpec {
            id: id(id_tail),
            launch,
            cwd: std::env::temp_dir(),
            envs: vec![(FIXTURE_MODE_VAR.to_owned(), "1".to_owned())],
        }
    }

    // -----------------------------------------------------------------------
    // ORI-T-0033, plant 4: the launch configuration closes stdin. Data-level,
    // deterministic on every platform.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_headless_launch_config_closes_stdin() {
        let launch =
            LaunchConfig::headless("some-agent-cli", vec!["-p".to_owned(), "hi".to_owned()])
                .expect("a non-empty program");
        assert_eq!(launch.stdin(), StdioMode::Null);
        assert!(launch.non_interactive());
    }

    #[test]
    fn ori_t_0033_spawn_refuses_a_launch_configuration_whose_stdin_is_not_null() {
        let mut adapter = adapter(|_| Ok(()));
        let mut spec = spawn_fixture_spec("RFSE1");
        // The wrong shape plant 4 names: stdin inherited rather than closed.
        spec.launch = LaunchConfig::assemble(
            spec.launch.program().to_owned(),
            spec.launch.args().to_vec(),
            StdioMode::Inherit,
            true,
        );
        let refusal = adapter
            .spawn(spec)
            .expect_err("stdin must be null for a headless adapter");
        assert_eq!(
            refusal,
            RuntimeError::WrongStdinMode {
                required: StdioMode::Null,
                found: StdioMode::Inherit,
            }
        );
        assert!(refusal.is_refusal());
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(17)
        );
    }

    // -----------------------------------------------------------------------
    // ORI-P1-039: a real run with stdin null never blocks reading it, proved
    // over a real child process with a bounded wait (trap: a hanging child
    // must be run with a timeout).
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_039_a_headless_run_with_stdin_null_never_blocks_on_a_read() {
        let mut adapter = adapter(|defect: LaunchDefect| -> Result<(), String> {
            panic!("no defect expected for a well-behaved fixture: {defect:?}")
        });
        let started = Instant::now();
        let handle = adapter
            .spawn(spawn_fixture_spec("STDN1"))
            .expect("the fixture spawns with stdin null");
        let stop_reason = adapter
            .send(&handle, "unused for a one-shot run")
            .expect("the one-shot run completes");
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "a read against a closed stdin returns immediately; this took {elapsed:?}"
        );
        assert_eq!(stop_reason, StopReason::EndTurn);

        // Read before `kill`, which drops the session (and its transcript)
        // as part of tearing it down.
        let transcript = adapter
            .transcript()
            .expect("recorded, before kill drops it")
            .clone();
        adapter.kill(&handle).expect("teardown");
        assert_eq!(transcript.len(), 1, "{:?}", transcript.entries());
        assert!(
            transcript.entries()[0]
                .output()
                .contains("\"stdin\":\"eof\""),
            "the fixture observed end-of-file, not a byte of a prompt: {}",
            transcript.entries()[0].output()
        );
    }

    #[test]
    fn ori_p1_039_a_second_send_on_a_one_shot_session_is_refused() {
        let mut adapter = adapter(|_| Ok(()));
        let handle = adapter.spawn(spawn_fixture_spec("SCND2")).expect("spawns");
        adapter
            .send(&handle, "first")
            .expect("the first send runs the one shot to completion");
        let refusal = adapter
            .send(&handle, "second")
            .expect_err("there is no second turn for a one-shot run");
        assert_eq!(refusal, RuntimeError::AlreadyCompleted);
        adapter.kill(&handle).expect("teardown");
    }

    // -----------------------------------------------------------------------
    // The contrast: the very same fixture, given a stdin the test keeps
    // open and never closes, would block. This is what proves the positive
    // case above is not vacuous: closing stdin is what changes the outcome,
    // not the fixture always reporting EOF regardless of its stdin.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_the_same_fixture_given_an_open_stdin_would_block_proving_the_contrast_is_real() {
        let exe = fixture_exe();
        let mut command = Command::new(&exe);
        command
            .arg(FIXTURE_TEST_NAME)
            .arg("--test-threads=1")
            .arg("--nocapture")
            .env(FIXTURE_MODE_VAR, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("the fixture binary spawns");
        // Deliberately never write to, and never close, the child's stdin:
        // held here for the life of `child_stdin` below, exactly the
        // "still open, no producer" condition `HeadlessAdapter::spawn`'s
        // `Stdio::null()` exists to avoid.
        let child_stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("stdout is piped");

        // A generous grace period, then a non-killing check
        // (`Child::try_wait` never blocks) that the process is still
        // running: the block is demonstrated, not assumed.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            child.try_wait().expect("try_wait does not itself block"),
            None,
            "the fixture is still blocked reading its open, unclosed stdin"
        );

        // Now close it: the fixture sees end-of-file and exits on its own,
        // bounded rather than trusted to (trap: a child that can hang is run
        // with a timeout).
        drop(child_stdin);
        let status = wait_with_timeout(&mut child, Duration::from_secs(5))
            .expect("closing stdin now lets the fixture see end-of-file and exit");
        assert!(status.success());
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        assert!(
            output.contains("\"stdin\":\"eof\""),
            "once stdin is closed, even late, the same fixture reports eof: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0033, plant 4, closed deterministically: `HeadlessAdapter::spawn`
    // must never give its own child *this process's* stdin, whatever that
    // happens to be. The tests above prove the positive case (a real run
    // finishes fast) and the contrast (the same fixture would block without
    // a null stdin), but neither one can tell "really null" apart from
    // "inherited, and this harness's own stdin already reads as end-of-file"
    // on a sandboxed runner where ambient stdin is closed anyway: on such a
    // runner, a plant that swapped `Stdio::null()` for `Stdio::inherit()`
    // deep inside `HeadlessAdapter::spawn` would pass both tests above by
    // accident, not because the code is right.
    //
    // This test removes that accident. It re-executes this test binary as a
    // *wrapper* (`ori_t_0033_headless_inherit_wrapper_entrypoint_do_not_call_directly`)
    // whose own stdin this test pins to a pipe it opens and never writes to
    // or closes. The wrapper then calls the real `HeadlessAdapter::spawn`
    // and `send`, exactly as the tests above do, and reports what the
    // *fixture two levels down* saw on its stdin. If `HeadlessAdapter::spawn`
    // ever gives that fixture anything other than a genuinely null stdin,
    // the only stdin available to inherit is the wrapper's own, which is the
    // pipe this test holds open, so the fixture would block reading it and
    // `HeadlessAdapter::send`'s own bounded wait would time out. Whether the
    // ambient stdin of the process running this suite happens to be a
    // terminal, `/dev/null`, or already closed makes no difference here: the
    // pipe below is this test's own, held open, so "would have blocked" is
    // demonstrated rather than assumed on every platform this runs on.
    // -----------------------------------------------------------------------

    const INHERIT_WRAPPER_MODE_VAR: &str = "ORI_T_0033_HEADLESS_INHERIT_WRAPPER";
    const INHERIT_WRAPPER_TEST_NAME: &str =
        "ori_t_0033_headless_inherit_wrapper_entrypoint_do_not_call_directly";

    #[test]
    fn ori_t_0033_headless_inherit_wrapper_entrypoint_do_not_call_directly() {
        if std::env::var(INHERIT_WRAPPER_MODE_VAR).is_err() {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        }
        let mut adapter = adapter(|_| Ok(()));
        let report = match adapter.spawn(spawn_fixture_spec("WRAP1")) {
            Ok(handle) => {
                let outcome = adapter.send(&handle, "unused");
                let saw_eof = adapter
                    .transcript()
                    .and_then(|transcript| transcript.entries().first().cloned())
                    .is_some_and(|entry| entry.output().contains("\"stdin\":\"eof\""));
                let _ = adapter.kill(&handle);
                match outcome {
                    Ok(_) if saw_eof => "eof",
                    Ok(_) => "other",
                    Err(RuntimeError::Timeout { .. }) => "timeout",
                    Err(_) => "error",
                }
            }
            Err(_) => "spawn_error",
        };
        let mut out = std::io::stdout();
        let _ = writeln!(out, "WRAPPER_RESULT:{report}");
        let _ = out.flush();
    }

    #[test]
    fn ori_p1_039_headless_spawn_never_lets_its_child_inherit_this_processs_own_stdin() {
        let exe = fixture_exe();
        let mut command = Command::new(&exe);
        command
            .arg(INHERIT_WRAPPER_TEST_NAME)
            .arg("--test-threads=1")
            .arg("--nocapture")
            .env(INHERIT_WRAPPER_MODE_VAR, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("the wrapper binary spawns");
        // Held open deliberately, for the life of `child_stdin`: see this
        // test's own doc comment above. Never written to, never closed here.
        let child_stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("stdout is piped");

        let started = Instant::now();
        // Bounded well past HeadlessAdapter's own 20s RUN_TIMEOUT: a correct
        // adapter finishes in well under a second; a plant that lets the
        // grandchild inherit the pipe held below finishes closer to 20s
        // (RUN_TIMEOUT elapsing) rather than never, so this wait is bounded
        // either way (trap: a child that can hang is run with a timeout).
        let status = wait_with_timeout(&mut child, Duration::from_secs(30))
            .expect("the wrapper always exits, win or lose, within its own bounded wait");
        let elapsed = started.elapsed();
        drop(child_stdin);

        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        assert!(
            status.success(),
            "the wrapper itself did not panic: {output}"
        );
        assert!(
            output.contains("WRAPPER_RESULT:eof"),
            "the fixture two levels down must see end-of-file, never this test's own open pipe: \
             {output}"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "a genuinely null stdin resolves in well under a second; this took {elapsed:?}, \
             which is what inheriting the open pipe held above and hitting RUN_TIMEOUT looks like"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0140: the deadline covers the whole run, reading included.
    //
    // Before this ticket `send` read the child's stdout to end-of-file and
    // only then started `RUN_TIMEOUT`, so a child that kept stdout open (or
    // left a descendant holding it) blocked `send` for as long as the holder
    // lived and the clock never started. These tests run real children and
    // bound every wait. The deadline is passed to `send_within` as one
    // second, not `RUN_TIMEOUT`'s twenty; the misbehaving children hold the
    // pipe for `T0140_HOLD`, far longer than the deadline plus its margin, so
    // a `send` that waited on the holder instead of the deadline fails the
    // elapsed-time assertions rather than passing slowly.
    // -----------------------------------------------------------------------

    use std::cell::RefCell;
    use std::rc::Rc;

    /// The deadline the ORI-T-0140 tests give a run.
    const T0140_DEADLINE: Duration = Duration::from_secs(1);
    /// How long a misbehaving child (or its descendant) holds the pipe.
    const T0140_HOLD_SECONDS: u64 = 15;
    /// How far past the deadline a correct `send` may be on a loaded machine.
    /// Well under `T0140_HOLD_SECONDS` minus the deadline, so the margin
    /// cannot hide the defect.
    const T0140_MARGIN: Duration = Duration::from_secs(8);

    type RecordedDefects = Rc<RefCell<Vec<LaunchDefect>>>;

    /// An adapter whose recorder keeps every defect it is handed.
    fn recording_adapter() -> (HeadlessAdapter, RecordedDefects) {
        let seen: RecordedDefects = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let adapter = adapter(move |defect| {
            sink.borrow_mut().push(defect);
            Ok(())
        });
        (adapter, seen)
    }

    const HOLD_MODE_VAR: &str = "ORI_T_0140_HOLD_STDOUT_SECONDS";
    const HOLD_TEST_NAME: &str = "ori_t_0140_hold_fixture_entrypoint_do_not_call_directly";

    /// The cross-platform misbehaving child: a re-exec of this test binary
    /// that writes one line and then keeps its stdout open, without closing
    /// it, for the number of seconds the environment names.
    #[test]
    fn ori_t_0140_hold_fixture_entrypoint_do_not_call_directly() {
        let Ok(seconds) = std::env::var(HOLD_MODE_VAR) else {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        };
        let seconds: u64 = seconds.parse().unwrap_or(0);
        let mut out = std::io::stdout();
        let _ = writeln!(out, "holding");
        let _ = out.flush();
        std::thread::sleep(Duration::from_secs(seconds));
    }

    fn hold_spec(id_tail: &str, seconds: u64) -> SessionSpec {
        let mut spec = spawn_fixture_spec(id_tail);
        spec.launch = LaunchConfig::headless(
            fixture_exe(),
            vec![
                HOLD_TEST_NAME.to_owned(),
                "--test-threads=1".to_owned(),
                "--nocapture".to_owned(),
            ],
        )
        .expect("a non-empty program");
        spec.envs = vec![(HOLD_MODE_VAR.to_owned(), seconds.to_string())];
        spec
    }

    /// A shell child: `sh -c <script>`, stdin null like every headless run.
    #[cfg(unix)]
    fn shell_spec(id_tail: &str, script: &str) -> SessionSpec {
        SessionSpec {
            id: id(id_tail),
            launch: LaunchConfig::headless("sh", vec!["-c".to_owned(), script.to_owned()])
                .expect("a non-empty program"),
            cwd: std::env::temp_dir(),
            envs: Vec::new(),
        }
    }

    /// Asserts a run was refused as a timeout and recorded exactly once, the
    /// way the module promises: the launch defect (method, reason, a detail
    /// containing `detail_fragment`), the transcript's refusal entry, and no
    /// second `send`.
    fn assert_refused_as_timeout(
        adapter: &mut HeadlessAdapter,
        handle: &SessionHandle,
        defects: &RecordedDefects,
        outcome: Result<StopReason, RuntimeError>,
        detail_fragment: &str,
    ) {
        assert_eq!(
            outcome,
            Err(RuntimeError::Timeout {
                after: T0140_DEADLINE
            })
        );
        let defects = defects.borrow();
        assert_eq!(defects.len(), 1, "exactly one defect: {defects:?}");
        assert_eq!(defects[0].method, "headless.run");
        assert_eq!(defects[0].reason, LaunchDefect::reason());
        assert_eq!(defects[0].at, START);
        assert!(
            defects[0].detail.contains(detail_fragment),
            "the detail names which of the two timeouts this was: {}",
            defects[0].detail
        );
        let transcript = adapter.transcript().expect("the session is still there");
        assert_eq!(transcript.len(), 1, "{:?}", transcript.entries());
        assert!(
            transcript.entries()[0]
                .output()
                .starts_with("refused: timeout"),
            "{}",
            transcript.entries()[0].output()
        );
        assert_eq!(
            adapter.send_within(handle, "again", T0140_DEADLINE),
            Err(RuntimeError::AlreadyCompleted),
            "a timed-out one-shot run is spent"
        );
    }

    fn child_status(adapter: &mut HeadlessAdapter) -> Option<ExitStatus> {
        adapter
            .session
            .as_mut()
            .expect("the session is still there")
            .child
            .try_wait()
            .expect("try_wait does not itself block")
    }

    #[test]
    fn ori_t_0140_a_child_holding_stdout_open_past_the_deadline_is_a_timeout_within_it() {
        let (mut adapter, defects) = recording_adapter();
        let handle = adapter
            .spawn(hold_spec("TA140", T0140_HOLD_SECONDS))
            .expect("the holding fixture spawns");

        let started = Instant::now();
        let outcome = adapter.send_within(&handle, "unused", T0140_DEADLINE);
        let elapsed = started.elapsed();

        assert!(
            elapsed >= T0140_DEADLINE && elapsed < T0140_DEADLINE + T0140_MARGIN,
            "reported at the deadline, not when the holder let go ({T0140_HOLD_SECONDS}s): \
             {elapsed:?}"
        );
        assert_refused_as_timeout(
            &mut adapter,
            &handle,
            &defects,
            outcome,
            "did not exit within",
        );
        let status = child_status(&mut adapter).expect("the child was killed and reaped");
        assert!(!status.success(), "a killed child does not exit cleanly");
        adapter.kill(&handle).expect("teardown");
    }

    #[test]
    fn ori_t_0140_a_recorder_that_refuses_the_defect_is_the_error_and_the_run_is_still_bounded() {
        let mut adapter = adapter(|_| Err("the sink is full".to_owned()));
        let handle = adapter
            .spawn(hold_spec("TB140", T0140_HOLD_SECONDS))
            .expect("the holding fixture spawns");

        let started = Instant::now();
        let outcome = adapter.send_within(&handle, "unused", T0140_DEADLINE);
        let elapsed = started.elapsed();

        assert_eq!(
            outcome,
            Err(RuntimeError::DefectSink {
                message: "the sink is full".to_owned()
            })
        );
        assert!(
            elapsed < T0140_DEADLINE + T0140_MARGIN,
            "bounded even when recording fails: {elapsed:?}"
        );
        assert!(
            child_status(&mut adapter).is_some(),
            "the child was killed before the defect was recorded"
        );
        adapter.kill(&handle).expect("teardown");
    }

    #[test]
    fn ori_t_0140_a_normal_run_still_returns_its_output_and_end_turn_without_a_defect() {
        let (mut adapter, defects) = recording_adapter();
        // Zero seconds: writes its line and exits at once, stdout closed.
        let handle = adapter
            .spawn(hold_spec("TC140", 0))
            .expect("the fixture spawns");

        let started = Instant::now();
        let outcome = adapter.send(&handle, "the prompt");
        let elapsed = started.elapsed();

        assert_eq!(outcome, Ok(StopReason::EndTurn));
        assert!(
            elapsed < T0140_MARGIN,
            "no waiting on a deadline: {elapsed:?}"
        );
        assert!(defects.borrow().is_empty(), "{:?}", defects.borrow());
        let transcript = adapter.transcript().expect("recorded");
        assert_eq!(transcript.len(), 1, "{:?}", transcript.entries());
        assert!(
            transcript.entries()[0].output().contains("holding"),
            "{}",
            transcript.entries()[0].output()
        );
        assert_eq!(transcript.entries()[0].input(), "the prompt");
        adapter.kill(&handle).expect("teardown");
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0140_a_normal_shell_run_returns_its_output_and_a_failing_one_its_exit_code() {
        let (mut adapter, defects) = recording_adapter();
        let handle = adapter
            .spawn(shell_spec("TD140", "printf 'out'; exit 3"))
            .expect("sh spawns");
        let outcome = adapter.send(&handle, "unused");
        assert_eq!(outcome, Ok(StopReason::Other("exit 3".to_owned())));
        assert_eq!(
            adapter.transcript().expect("recorded").entries()[0].output(),
            "out"
        );
        assert!(defects.borrow().is_empty(), "{:?}", defects.borrow());
        adapter.kill(&handle).expect("teardown");
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0140_output_far_past_the_pipe_buffer_is_drained_and_capped_as_before() {
        // 300 000 bytes is several pipe buffers: a reader that only started
        // once the child had exited would deadlock the child on a full pipe.
        let (mut adapter, defects) = recording_adapter();
        let handle = adapter
            .spawn(shell_spec(
                "TE140",
                "dd if=/dev/zero bs=1000 count=300 2>/dev/null | tr '\\0' x",
            ))
            .expect("sh spawns");

        let started = Instant::now();
        let outcome = adapter.send(&handle, "unused");
        let elapsed = started.elapsed();

        assert_eq!(outcome, Ok(StopReason::EndTurn));
        assert!(elapsed < T0140_MARGIN, "{elapsed:?}");
        assert!(defects.borrow().is_empty(), "{:?}", defects.borrow());
        let output = adapter.transcript().expect("recorded").entries()[0]
            .output()
            .to_owned();
        assert_eq!(output, format!("{}...", "x".repeat(STDOUT_CAP)));
        adapter.kill(&handle).expect("teardown");
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0140_a_child_that_exits_leaving_a_descendant_on_stdout_is_not_waited_on_past_the_deadline()
     {
        let (mut adapter, defects) = recording_adapter();
        // The shell backgrounds a long sleep, which inherits stdout, and
        // exits at once with status 0.
        let handle = adapter
            .spawn(shell_spec(
                "TF140",
                &format!("sleep {T0140_HOLD_SECONDS} & exit 0"),
            ))
            .expect("sh spawns");

        let started = Instant::now();
        let outcome = adapter.send_within(&handle, "unused", T0140_DEADLINE);
        let elapsed = started.elapsed();

        assert!(
            elapsed >= T0140_DEADLINE && elapsed < T0140_DEADLINE + T0140_MARGIN,
            "reported at the deadline, not when the descendant let go ({T0140_HOLD_SECONDS}s): \
             {elapsed:?}"
        );
        assert_refused_as_timeout(&mut adapter, &handle, &defects, outcome, "still held open");
        let status = child_status(&mut adapter).expect("the shell itself had exited");
        assert!(
            status.success(),
            "the child exited cleanly; only its descendant kept the output open: {status:?}"
        );
        adapter.kill(&handle).expect("teardown");
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0140_a_descendant_that_outlives_the_killed_child_does_not_keep_send_blocked() {
        let (mut adapter, defects) = recording_adapter();
        // A background sleep (the descendant that will outlive the kill) and
        // a foreground one (what keeps the child itself running).
        let handle = adapter
            .spawn(shell_spec(
                "TG140",
                &format!("sleep {T0140_HOLD_SECONDS} & sleep {T0140_HOLD_SECONDS}"),
            ))
            .expect("sh spawns");

        let started = Instant::now();
        let outcome = adapter.send_within(&handle, "unused", T0140_DEADLINE);
        let elapsed = started.elapsed();

        assert!(
            elapsed >= T0140_DEADLINE && elapsed < T0140_DEADLINE + T0140_MARGIN,
            "the killed child's descendant still holds the pipe for {T0140_HOLD_SECONDS}s; \
             send must not wait for it: {elapsed:?}"
        );
        assert_refused_as_timeout(
            &mut adapter,
            &handle,
            &defects,
            outcome,
            "did not exit within",
        );
        assert!(
            child_status(&mut adapter).is_some(),
            "the child itself was killed and reaped"
        );
        adapter.kill(&handle).expect("teardown");
    }

    // -----------------------------------------------------------------------
    // ORI-T-0140: the bounded collector keeps what the transcript cap would
    // have kept, and nothing more. No process involved.
    // -----------------------------------------------------------------------

    /// A reader that hands out `data` in pieces of at most `step` bytes.
    struct Chunked<'a> {
        data: &'a [u8],
        step: usize,
    }

    impl Read for Chunked<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let n = self.step.min(buffer.len()).min(self.data.len());
            buffer[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    fn transcript_text(bytes: &[u8]) -> String {
        capped(&String::from_utf8_lossy(bytes))
    }

    #[test]
    fn ori_t_0140_the_bounded_collector_keeps_exactly_what_the_transcript_cap_would_have_kept() {
        // Multi-byte characters, an incomplete one and invalid bytes, placed
        // so that the cut falls at every offset around the cap.
        let tails: [&[u8]; 3] = [
            "\u{20ac}\u{1f600}\u{e9}".as_bytes(),
            &[0xE2, 0x82, 0xFF, 0xF0, 0x9F, 0x98, 0x20, 0xC3],
            "\u{1f600}".as_bytes(),
        ];
        for tail in tails {
            for lead in 0..24 {
                let mut data = vec![b'a'; STDOUT_CAP - 12 + lead];
                for _ in 0..16 {
                    data.extend_from_slice(tail);
                }
                for step in [1, 7, 8192] {
                    let kept = collect_bounded(Chunked { data: &data, step }, STDOUT_RETAIN)
                        .expect("an in-memory reader does not fail");
                    assert!(kept.len() <= STDOUT_RETAIN);
                    assert_eq!(
                        transcript_text(&kept),
                        transcript_text(&data),
                        "lead {lead}, step {step}, tail {tail:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn ori_t_0140_the_bounded_collector_drains_far_more_than_it_keeps() {
        let data = vec![b'z'; 1_000_000];
        let kept = collect_bounded(
            Chunked {
                data: &data,
                step: 8192,
            },
            STDOUT_RETAIN,
        )
        .expect("an in-memory reader does not fail");
        assert_eq!(kept.len(), STDOUT_RETAIN);
        let short = collect_bounded(
            Chunked {
                data: b"short",
                step: 2,
            },
            STDOUT_RETAIN,
        )
        .expect("an in-memory reader does not fail");
        assert_eq!(short, b"short");
    }

    #[test]
    fn ori_t_0140_the_bounded_collector_retries_an_interrupted_read_and_reports_a_failed_one() {
        struct Flaky {
            calls: u32,
            fail_for_good: bool,
        }
        impl Read for Flaky {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.calls += 1;
                match self.calls {
                    1 => Err(std::io::Error::from(ErrorKind::Interrupted)),
                    2 => {
                        buffer[0] = b'k';
                        Ok(1)
                    }
                    _ if self.fail_for_good => Err(std::io::Error::other("the pipe broke")),
                    _ => Ok(0),
                }
            }
        }
        let kept = collect_bounded(
            Flaky {
                calls: 0,
                fail_for_good: false,
            },
            STDOUT_RETAIN,
        )
        .expect("an interrupted read is retried");
        assert_eq!(kept, b"k");
        let failure = collect_bounded(
            Flaky {
                calls: 0,
                fail_for_good: true,
            },
            STDOUT_RETAIN,
        )
        .expect_err("any other error ends the collection");
        assert_eq!(failure.to_string(), "the pipe broke");
    }
}
