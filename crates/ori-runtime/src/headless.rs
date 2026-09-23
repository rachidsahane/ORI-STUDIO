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
//! does not exit within `RUN_TIMEOUT` is killed and reported as
//! [`crate::acp::RuntimeError::Timeout`], covering the residual case of a CLI
//! that busy-polls a closed stdin (reading zero bytes over and over) rather
//! than genuinely blocking on it. A run with everything it needs from argv
//! finishes in bounded time; one still running past that bound is, from the
//! outside, indistinguishable from one stuck waiting for a human, and is
//! treated the same way rather than merely as an ordinary slow process.
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

use std::io::Read;
use std::process::Child;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

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
    stdout: ChildStdout,
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
            stdout,
            completed: false,
            transcript: Transcript::new(),
        });
        Ok(handle)
    }

    /// Waits for the one-shot run to finish (bounded by `RUN_TIMEOUT`) and
    /// records its captured output. See this module's own doc comment, "One
    /// shot, not a turn-based session", for why `prompt` is not written
    /// anywhere here and why a second call is refused.
    ///
    /// The recorder and clock this adapter was constructed with are used
    /// when the process does not exit within `RUN_TIMEOUT`: from outside
    /// this module that is indistinguishable from a runtime blocked on a
    /// prompt it should never have been able to raise, so it is recorded
    /// the same way, over the same [`RuntimeError::methodology_ref`], AICD
    /// §17.
    fn send(&mut self, handle: &SessionHandle, prompt: &str) -> Result<StopReason, RuntimeError> {
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

        let mut output = Vec::new();
        let read_result = session.stdout.read_to_end(&mut output);
        let wait_result = wait_with_timeout(&mut session.child, RUN_TIMEOUT);
        session.completed = true;

        let status = match wait_result {
            Ok(status) => status,
            Err(error @ RuntimeError::Timeout { .. }) => {
                // Indistinguishable, from here, from a runtime blocked on a
                // prompt it should never have raised: recorded the same way.
                // See this module's own doc comment, "What 'prompting' can
                // mean with stdin closed".
                let defect = LaunchDefect {
                    at,
                    method: "headless.run",
                    detail: format!(
                        "the headless run did not exit within {RUN_TIMEOUT:?}; killed and refused \
                         rather than left to keep waiting on a prompt it should never have raised"
                    ),
                    reason: LaunchDefect::reason(),
                };
                (self.record_defect)(defect.clone())
                    .map_err(|message| RuntimeError::DefectSink { message })?;
                session.transcript = session.transcript.record(Entry::new(
                    1,
                    at,
                    Some("headless.run"),
                    prompt,
                    &format!("refused: timeout ({})", defect.reason),
                )?)?;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        read_result.map_err(|error| RuntimeError::Io {
            message: error.to_string(),
        })?;
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
}
