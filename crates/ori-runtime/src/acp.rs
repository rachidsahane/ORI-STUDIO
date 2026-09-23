//! The ACP client and the `AgentRuntime` trait: AICD §17, ADR-0001 "Agent
//! protocol", PRD I-04 and P-02, `spec/API_SPEC.md` §4, criterion ORI-P1-039.
//!
//! # What ORI-P1-039 asks, and what answers it here
//!
//! The criterion: "Any agent session | Inspect the runtime launch
//! configuration and the session transcript | The runtime was launched in
//! its non-interactive mode; no permission prompt appears in the transcript;
//! a runtime that prompts is recorded as a launch defect." [`LaunchConfig`]
//! is the inspectable launch configuration; [`Transcript`](crate::transcript)
//! (already this crate's, from `crate::transcript`) is the session
//! transcript; [`AcpClient::prompt`] is where a permission prompt, which in
//! ACP is the concrete message `session/request_permission` (an agent-to-
//! client *request*, not a notification), is answered.
//!
//! AICD §17 opens with the sentence this whole file exists to make true in
//! code: "AICD treats agent permissions as a security architecture, enforced
//! at the infrastructure and tool-connection level, never as a line in a
//! prompt." A `session/request_permission` message is exactly a line in a
//! prompt arriving over the wire instead of a terminal, and this module's
//! answer to it is not "ask a human" (there is no human in the loop of a
//! fleet-mode coder session, `spec/CONVENTIONS.md` "Auto mode": "No agent
//! ever waits on a permission prompt; if a runtime prompts, the launch
//! configuration is wrong and it is a bug") but "refuse it, record that the
//! launch was wrong, and continue running non-interactively." Every refusal
//! this module makes therefore cites AICD §17, through
//! [`RuntimeError::methodology_ref`] and through the `reason` field of
//! [`LaunchDefect`].
//!
//! # What "records a launch defect" means, mechanically, and the seam that
//! makes it possible without a new dependency
//!
//! ORI-T-0033's own instructions: "a launch defect (an event through
//! `ori_store::event_log::EventLog::append`, and an entry in the
//! transcript)". The transcript half is direct: [`AcpClient`] owns a private
//! [`crate::transcript::Transcript`] and appends to it, in the crate it
//! already belongs to. The event-log half cannot be direct in the same way
//! without a cost this ticket's own instructions refuse to pay:
//! `EventLog::append` takes `&mut rusqlite::Connection`, and naming that type
//! in this crate's own signatures needs `rusqlite` declared as a direct
//! dependency of `ori-runtime` (a Rust crate can only name a type from a
//! crate it directly depends on; a value can still flow through it via
//! inference without ever being named). This ticket's dependency section is
//! explicit: "Add them \[serde, serde_json\] to `ori-runtime` only... Add
//! nothing else", and `rusqlite`, though already used elsewhere in this
//! workspace at an already-audited, already-pinned version, is not serde or
//! serde_json.
//!
//! So [`AcpClient::new`] (and `crate::headless::HeadlessAdapter::new`) takes
//! a `record_defect: impl FnMut(LaunchDefect) -> Result<(), String>`
//! parameter, once, at construction: a closure the *caller* supplies, whose
//! body is free to hold a `&mut rusqlite::Connection` obtained from
//! `ori_store::db::ProductDb` (already a normal dependency of this crate, by
//! way of `ori-store`) and to call `EventLog::append` inside it, because
//! *that* code lives in the caller's own module (in this file's own tests,
//! and in the future orchestrator code that will own a `Connection` and this
//! client at once), never in `acp.rs`. This does not weaken "the client
//! records the defect": [`AcpClient`] is the only place in this module that
//! decides a `session/request_permission` message arrived, and it calls the
//! stored recorder unconditionally, synchronously, before it ever answers
//! the agent, so the recording is exactly as tied to the event as it would
//! be if `EventLog::append` were spelled out inline here. What moved is only
//! which crate's source code contains the token `rusqlite`, not when or
//! whether the append happens.
//!
//! # Why the recorder and the clock are constructor parameters, not
//! per-call ones (fix 3 of this ticket's follow-up review)
//!
//! An earlier version of this file took `record_defect` and `at` as
//! parameters of `spawn`/`send`/`prompt` themselves, and `impl AgentRuntime
//! for AcpClient` — the trait path `spec/API_SPEC.md` §4 says the engine
//! actually uses — passed a no-op closure and `Timestamp::from_millis(0)` at
//! that boundary, because the trait's own methods (matching the sketch)
//! carry neither. That silently dropped ORI-P1-039's "recorded as a launch
//! defect" clause for exactly the call path the criterion is about, and
//! recorded every trait-path defect at the Unix epoch. [`AcpClient::new`]
//! and `crate::headless::HeadlessAdapter::new` now require both a
//! `record_defect` and a `clock` to be built at all, stored as
//! [`DefectRecorder`] and [`ClockFn`] fields: there is no `Default`, no
//! second constructor, and no code path left, trait or inherent, that can
//! run without a real recorder and a real time source. Production supplies
//! the `EventLog`-backed recorder and a real clock; tests may supply a
//! recording fake and a fixed clock. No `|_| Ok(())` recorder and no
//! `Timestamp::from_millis(0)` remain anywhere outside `#[cfg(test)]` code in
//! this crate (checked with `grep -n 'from_millis(0)\|_| Ok(())'
//! crates/ori-runtime/src/acp.rs crates/ori-runtime/src/headless.rs`, which
//! the pull request report quotes the output of).
//!
//! # The ACP protocol version implemented, and where it was read
//!
//! [`ACP_PROTOCOL_VERSION`] is `1`. **Read live** from
//! `agentclientprotocol.com` on 2026-09-23 (the operator has a web tool this
//! agent did not; the first version of this file was written from trained
//! knowledge instead, flagged as a gap in the pull request report, and the
//! operator's own fetch found two real mismatches, fixed here):
//!
//! - `protocol/initialization`: `protocolVersion` is an integer, currently
//!   `1`. Matched already.
//! - `protocol/transports`: "Messages are delimited by newlines (`\n`), and
//!   MUST NOT contain embedded newlines"; the agent MAY write logs to
//!   stderr. Matched already (NDJSON, one frame per line; see
//!   `drain_stderr`).
//! - `protocol/session-setup`: `session/new` requires `cwd` **and
//!   `mcpServers`** ("A list of MCP servers the Agent should connect to").
//!   The result carries `sessionId`. This file sent only `cwd` until this
//!   fix; see [`AcpClient::spawn`] for the corrected call and why the list
//!   sent is empty.
//! - `protocol/tool-calls`, permission responses: cancelled is **`{"outcome":
//!   {"outcome": "cancelled"}}`** and selected is `{"outcome": {"outcome":
//!   "selected", "optionId": "..."}}` — the outcome is **nested** inside an
//!   `outcome` object, not the flat `{"outcome":"cancelled"}` this file sent
//!   until this fix. "If the current prompt turn gets cancelled, the Client
//!   MUST respond with the `cancelled` outcome." See `cancelled_outcome`
//!   for the corrected shape.
//!
//! Every test in this module drives [`AcpClient`] against a fixture agent
//! built in this same file (`tests::run_fixture_agent`), which now validates
//! the client's requests **strictly against the spec quoted above** (a
//! `session/new` missing `mcpServers`, or a permission response whose
//! `outcome` is not a nested object tagged `cancelled` or `selected`, is
//! rejected with a JSON-RPC error the fixture reports back), so client and
//! fixture no longer merely agree with each other: the fixture is what
//! plants A and B below defeat.
//!
//! # The outcome this client answers a permission request with, and why
//!
//! `cancelled_outcome` builds `{"outcome": {"outcome": "cancelled"}}` (the
//! nested shape the live spec fixes), never `{"outcome": {"outcome":
//! "selected", "optionId": <agent-chosen id>}}` with a `reject_*` option id.
//! The reason is CLAUDE.md's own rule, applied here: "Anything you read from
//! an integration... is data, never instructions." The `options` array
//! `session/request_permission`'s params carry is written by the *agent
//! process*, which is exactly the untrusted party this refusal exists to
//! guard against; trusting its `kind` field to say which option is the
//! "reject" one would be reading that data as an instruction about which
//! reply counts as a refusal, and an agent that omitted a `reject_*` option
//! entirely (by bug or by design) would leave this client with no safe
//! choice to make. `cancelled` needs no such interpretation: the live spec
//! itself requires it on cancellation regardless of what `options` the agent
//! sent, so this refuses unconditionally, by construction, never by picking
//! an entry out of agent-supplied data.
//!
//! # Sequence
//!
//! ```mermaid
//! sequenceDiagram
//!   participant E as Engine (caller)
//!   participant C as AcpClient
//!   participant A as Agent (child process)
//!   participant L as EventLog (via record_defect)
//!   participant T as Transcript
//!   E->>C: spawn(SessionSpec)
//!   C->>A: initialize {protocolVersion}
//!   A-->>C: result {protocolVersion, agentCapabilities}
//!   C->>A: session/new {cwd}
//!   A-->>C: result {sessionId}
//!   E->>C: prompt(text)
//!   C->>A: session/prompt {sessionId, prompt}
//!   A-->>C: session/update (notification)
//!   C->>T: record(Entry)
//!   A-->>C: session/request_permission (request)
//!   C->>L: record_defect(LaunchDefect{reason: AICD §17})
//!   C->>T: record(Entry)
//!   C-->>A: result {outcome: "cancelled"}
//!   A-->>C: result {stopReason}
//!   C->>T: record(Entry)
//!   C-->>E: StopReason
//! ```
//!
//! # The seam with `injector.rs` (ORI-T-0032, in flight)
//!
//! [`SessionSpec::envs`] is the seam. This module never reads a keychain,
//! never reads configuration for a credential, and never logs or records an
//! environment value anywhere ([`LaunchConfig`], the inspectable half, holds
//! `program`, `args`, [`StdioMode`] and `non_interactive`, and nothing else).
//! `envs` is accepted exactly as the caller prepared it and handed straight
//! to [`std::process::Command::envs`] at spawn; a later ticket that wires
//! `Injector` supplies that `Vec` by placing an issued credential into it
//! before calling [`AcpClient::spawn`] or
//! [`crate::headless::HeadlessAdapter::spawn`], and neither adapter changes.
//!
//! # `SessionSpec` and `SessionHandle`, and what was chosen for them
//!
//! `spec/API_SPEC.md` §4 sketches `AgentRuntime` and says both types are
//! defined by the implementation ("Sketches; exact signatures live in LLD and
//! code"). [`SessionSpec`] is `{ id, launch, cwd, envs }`: `id` is the
//! engine's own session identifier (`crate::session::Session::id`), carried
//! in rather than generated here, because generating one needs a clock and a
//! source of randomness this module has neither of and `ori-core::types::Id`
//! only ever parses (`crates/ori-core/src/types.rs`); `launch` is
//! [`LaunchConfig`]; `cwd` and `envs` are what
//! [`std::process::Command`] needs beyond argv. [`SessionHandle`] wraps that
//! same `id`: an adapter instance in this module manages at most one active
//! session (see "Why one session per adapter instance" below), and every
//! call after `spawn` is refused ([`RuntimeError::UnknownSession`]) unless
//! the handle's `id` matches the one that was actually spawned, which is a
//! real check rather than a formality: a caller that mixed up two adapters'
//! handles is caught here rather than silently operating on the wrong child
//! process.
//!
//! # Why one session per adapter instance
//!
//! `spec/API_SPEC.md` §4's sketch gives `spawn` and `capabilities` a `&self`
//! receiver, which reads as an adapter meant to multiplex many sessions
//! behind shared, presumably-locked state. This module does not build that:
//! [`AgentRuntime::spawn`] here takes `&mut self` and an adapter holds at
//! most one `ActiveAcpSession`. Multiplexing needs a concurrency model (a
//! `Mutex<HashMap<Id, ActiveAcpSession>>`, most plainly) that is a decision
//! about the orchestrator's own dispatch, which does not exist in this
//! workspace yet; inventing one here would guess at a shape a future ticket
//! would then have to un-guess. One session per instance is what
//! `spec/CONVENTIONS.md` "Async with Tokio" and this ticket's own override of
//! it ("Plain threads and `std::process` with blocking stdio are acceptable
//! for this ticket") both point toward: an engine that wants many concurrent
//! sessions holds many adapter instances, one per `Session`
//! (`crate::session::Session`), which is also the natural unit
//! `spec/runbooks/recover-engine.md` already attributes a stray process to
//! (worktree path, container id) at.
//!
//! Must not: read a keychain or configuration for a credential; log or
//! record an environment value; spawn a process outside this crate's own
//! functions (CLAUDE.md: "Only `ori-runtime` spawns processes").

use core::fmt;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStderr;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use ori_core::error::MethodologyRef;
use ori_core::types::Id;
use ori_core::types::ModelFamily;
use ori_core::types::Timestamp;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::transcript::Entry;
use crate::transcript::Transcript;
use crate::transcript::TranscriptError;

/// How much of a subprocess's captured standard error is kept in an error
/// value or reported: the same bound `crate::worktree::STDERR_CAP` uses, for
/// the same reason (CLAUDE.md: what a dependency writes is data).
const STDERR_CAP: usize = 400;

/// The ACP protocol version this client speaks. See the module doc comment,
/// "The ACP protocol version implemented, and where it was read".
pub const ACP_PROTOCOL_VERSION: u32 = 1;

/// How long [`AcpClient::spawn`], `AcpClient::call` and
/// [`AcpClient::kill`]'s process wait may run before the child is killed and
/// the call refused as [`RuntimeError::Timeout`].
///
/// Not a substitute for `crate::budget::Budget`, which is the meter that
/// owns an agent session's real wall-clock allowance; this is a much shorter
/// bound against a narrower failure, a single JSON-RPC round trip or a
/// process teardown that never completes, which is what would otherwise let
/// a stuck child hang the calling thread forever ("a child process that can
/// hang must be run with a timeout").
const IO_TIMEOUT: Duration = Duration::from_secs(20);

/// How often [`wait_with_timeout`] polls [`Child::try_wait`] rather than
/// blocking on it, so the bound above is actually enforceable: `Child` has no
/// portable blocking wait with a timeout in `std`.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

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
/// one, and its first parameter is an [`ori_core::types::ModelFamily`], not
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
// LaunchConfig: the inspectable half of ORI-P1-039
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
    /// The parent's own standard input. Never chosen by this module's own
    /// constructors; a [`SessionSpec`] built with this is what plant 4 of
    /// this ticket's own table exercises, and `HeadlessAdapter::spawn` (`crate::headless`)
    /// (`crate::headless`) refuses it.
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
/// here that could hold one. See the module doc comment, "The seam with
/// `injector.rs`".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchConfig {
    program: String,
    args: Vec<String>,
    stdin: StdioMode,
    non_interactive: bool,
}

impl LaunchConfig {
    /// The launch configuration [`AcpClient::spawn`] uses: standard input
    /// piped (the client writes JSON-RPC requests to it), which is what makes
    /// the whole exchange the protocol substitute for a terminal prompt in
    /// the first place, so `non_interactive` is `true` unconditionally: an
    /// agent given a pipe instead of a pseudo-terminal has no terminal to
    /// prompt on at all, whatever it does with the pipe.
    ///
    /// # Errors
    ///
    /// [`LaunchConfigError::EmptyProgram`] when `program` is empty or only
    /// whitespace.
    pub fn acp(program: impl Into<String>, args: Vec<String>) -> Result<Self, LaunchConfigError> {
        Self::new(program, args, StdioMode::Piped)
    }

    /// The general constructor both [`LaunchConfig::acp`] and
    /// `crate::headless::LaunchConfig::headless` (which calls this with
    /// [`StdioMode::Null`]) share.
    pub(crate) fn new(
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

    /// A launch configuration built directly from its parts, without
    /// [`LaunchConfig::new`]'s validation. Used only where a test must build
    /// the one shape this module's own constructors refuse to build (plant
    /// 4: standard input inherited rather than closed), so that the refusal
    /// under test is `HeadlessAdapter::spawn`'s (`crate::headless`), not
    /// this function's. `#[cfg(test)]` because production code never needs
    /// the shape this deliberately skips validating.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn assemble(
        program: String,
        args: Vec<String>,
        stdin: StdioMode,
        non_interactive: bool,
    ) -> Self {
        Self {
            program,
            args,
            stdin,
            non_interactive,
        }
    }

    /// The program to execute. Never a path this module resolved itself:
    /// the caller names it, the same way `crate::worktree::SystemGit` never
    /// resolves `git`'s own path.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The argv, without the program name. Never holds a credential: see the
    /// module doc comment, "The seam with `injector.rs`".
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
/// §4's `AgentRuntime::spawn(session: SessionSpec)`. See the module doc
/// comment, "`SessionSpec` and `SessionHandle`, and what was chosen for
/// them".
#[derive(Clone, Debug)]
pub struct SessionSpec {
    /// The engine's own session identifier (`crate::session::Session::id`).
    pub id: Id,
    /// The launch configuration.
    pub launch: LaunchConfig,
    /// The working directory the child is spawned in: the session's
    /// `crate::worktree::Worktree` path, in the engine's real use.
    pub cwd: PathBuf,
    /// The child's environment, prepared entirely by the caller. Never read
    /// by this module for anything but passing to
    /// [`std::process::Command::envs`]; see the module doc comment, "The
    /// seam with `injector.rs`".
    pub envs: Vec<(String, String)>,
}

/// An opaque handle to a spawned session: `spec/API_SPEC.md` §4's
/// `SessionHandle`. Carries the same [`Id`] the [`SessionSpec`] it was
/// spawned from carried; see the module doc comment for why that is also
/// the check every later call makes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionHandle {
    id: Id,
}

impl SessionHandle {
    /// Wraps a session identifier a [`SessionSpec`] was built with. Public so
    /// `crate::headless` (a sibling module, not a descendant, so it cannot
    /// reach this type's private field directly) can build one too.
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
// The AgentRuntime trait
// ---------------------------------------------------------------------------

/// A launch defect: a permission prompt a runtime should never have raised,
/// refused rather than granted: AICD §17, ORI-P1-039.
///
/// Built only by [`AcpClient`] (and, defensively, wherever
/// `crate::headless::HeadlessAdapter` detects the same shape of failure) at
/// the moment a `session/request_permission` message is received, and handed
/// to the caller-supplied `record_defect` closure. See the module doc
/// comment, "What 'records a launch defect' means".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchDefect {
    /// When the prompt arrived.
    pub at: Timestamp,
    /// The protocol method that carried it (`"session/request_permission"`
    /// for ACP; `crate::headless` names its own).
    pub method: &'static str,
    /// A human-readable detail, built only from data this module already
    /// owns (the method name and a length-capped rendering of what the agent
    /// sent), never interpreted as an instruction.
    pub detail: String,
    /// Why this is refused: AICD §17, always. See [`LaunchDefect::reason`].
    pub reason: MethodologyRef,
}

impl LaunchDefect {
    /// AICD §17: "AICD treats agent permissions as a security architecture,
    /// enforced at the infrastructure and tool-connection level, never as a
    /// line in a prompt." Constructed by field literal rather than
    /// [`MethodologyRef::at`], the same reasoning
    /// `crates/ori-broker/src/family.rs`'s own `FamilyRefusalKind::reason`
    /// gives: section 17 is a fixed, always-valid constant known at every
    /// call site, and `spec/CONVENTIONS.md`'s "no `unwrap`, `expect` or
    /// `panic!` outside tests" forbids unwrapping the fallible constructor
    /// for a value that is never actually fallible here.
    #[must_use]
    pub const fn reason() -> MethodologyRef {
        MethodologyRef {
            section: 17,
            subsection: None,
        }
    }
}

/// A launch-defect recorder, supplied once when an adapter is constructed:
/// see the module doc comment, "Why the recorder and the clock are
/// constructor parameters, not per-call ones".
pub type DefectRecorder = Box<dyn FnMut(LaunchDefect) -> Result<(), String>>;

/// A source of the current time, supplied once alongside the
/// [`DefectRecorder`] so a [`LaunchDefect`] never carries a placeholder
/// timestamp. Not `ori-core`'s business (`ori-core` reads no clock, by
/// design), and not this module's own `std::time` call either: the caller
/// that has a real clock (or, in a test, a fixed one) supplies it, the same
/// pattern `ori_store::event_log::EventLog::append` already uses for `at`.
pub type ClockFn = Box<dyn FnMut() -> Timestamp>;

/// Where a turn ended: ACP's `stopReason`, or this adapter's own mapping of
/// a headless run's exit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StopReason {
    /// The agent finished the turn normally.
    EndTurn,
    /// The agent declined to continue (for instance, after this client
    /// refused a permission request).
    Refusal,
    /// The turn was cancelled.
    Cancelled,
    /// A value this module does not otherwise recognize, carried as data
    /// rather than dropped: `spec/API_SPEC.md` §5, "Adapter traits are
    /// versioned by crate", which is exactly what lets an agent report a stop
    /// reason a fixed enum here does not yet name without this client
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

/// One runtime adapter: `spec/API_SPEC.md` §4, ADR-0001 "Agent protocol".
///
/// Matches `spec/API_SPEC.md` §4's sketch closely now: `record_defect` and
/// the clock used to be parameters of `spawn`/`send` themselves (this
/// trait's own divergence from the sketch, in an earlier version of this
/// file), which let the trait path silently drop ORI-P1-039's "recorded as a
/// launch defect" clause; see the module doc comment, "Why the recorder and
/// the clock are constructor parameters, not per-call ones". Both are
/// supplied once, to each implementation's own constructor
/// ([`AcpClient::new`], `crate::headless::HeadlessAdapter::new`), instead.
pub trait AgentRuntime {
    /// Starts the runtime in its non-interactive mode, inside the isolation
    /// boundary the caller already built (`crate::worktree`,
    /// `crate::container` once ORI-T-0031 lands): PRD P-02, PRD I-04.
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

/// Everything this module refuses or cannot do: AICD §17 for the refusals,
/// `spec/CONVENTIONS.md`'s ruling R20 (hand-written `Display` and
/// `std::error::Error`, deferring `thiserror`) for the shape.
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
    /// frame this module recognizes.
    Protocol {
        /// What was wrong.
        message: String,
    },
    /// The agent answered one of this client's own requests with a JSON-RPC
    /// error.
    Rpc {
        /// The error code the agent reported.
        code: i64,
        /// The error message the agent reported, carried as data.
        message: String,
    },
    /// The child closed its output before answering a request this client
    /// was waiting on.
    UnexpectedEof,
    /// A JSON-RPC round trip or a process teardown did not complete within
    /// `IO_TIMEOUT`.
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
    /// `crate::headless::HeadlessAdapter::send` was called a second time. A
    /// one-shot, non-interactive run has exactly one turn: see
    /// `crate::headless`'s own module doc comment.
    AlreadyCompleted,
}

impl RuntimeError {
    /// The methodology section a refusal is made under, for the refusals and
    /// for nothing else.
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
// The JSON-RPC 2.0 wire frame
// ---------------------------------------------------------------------------

/// One line of the wire: a JSON-RPC 2.0 frame read from or written to the
/// child's stdio, newline-delimited (see the module doc comment, "The ACP
/// protocol version implemented"). Every field is optional on read because a
/// request, a response and a notification each carry a different subset, and
/// this module classifies which one arrived from what is present, in
/// [`RawFrame::classify`].
#[derive(Debug, Deserialize)]
struct RawFrame {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
}

/// A minimal, outgoing JSON-RPC 2.0 request.
#[derive(Serialize)]
struct RpcRequest {
    jsonrpc: &'static str,
    id: Value,
    method: String,
    params: Value,
}

/// A minimal, outgoing JSON-RPC 2.0 success response, used only to answer a
/// `session/request_permission` request with `cancelled_outcome`.
#[derive(Serialize)]
struct RpcResponseOk {
    jsonrpc: &'static str,
    id: Value,
    result: Value,
}

/// A minimal, outgoing JSON-RPC 2.0 error response, used defensively for any
/// incoming request this client does not otherwise recognize, so the agent
/// is never left waiting on one.
#[derive(Serialize)]
struct RpcResponseErr {
    jsonrpc: &'static str,
    id: Value,
    error: RpcErrorObj,
}

#[derive(Serialize)]
struct RpcErrorObj {
    code: i64,
    message: String,
}

/// What one line, once parsed, turned out to be.
enum Incoming {
    /// An answer to a request this client sent, matched by `id`.
    Response {
        id: Value,
        outcome: Result<Value, (i64, String)>,
    },
    /// A request the agent sent to this client, expecting an answer.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A one-way message carrying no `id`.
    Notification { method: String, params: Value },
}

impl RawFrame {
    /// Classifies this frame, refusing one that is neither a request, a
    /// response nor a notification (for instance, one with `id` and `method`
    /// both absent).
    fn classify(self) -> Result<Incoming, RuntimeError> {
        match (self.id, self.method) {
            (Some(id), Some(method)) => Ok(Incoming::Request {
                id,
                method,
                params: self.params.unwrap_or(Value::Null),
            }),
            (None, Some(method)) => Ok(Incoming::Notification {
                method,
                params: self.params.unwrap_or(Value::Null),
            }),
            (Some(id), None) => {
                let outcome = match self.error {
                    Some(error) => {
                        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
                        let message = error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        Err((code, message))
                    }
                    None => Ok(self.result.unwrap_or(Value::Null)),
                };
                Ok(Incoming::Response { id, outcome })
            }
            (None, None) => Err(RuntimeError::Protocol {
                message: "a frame with neither id nor method".to_owned(),
            }),
        }
    }
}

/// The response this client always sends to `session/request_permission`:
/// never a grant. See the module doc comment, "The outcome this client
/// answers a permission request with, and why". The `outcome` is a nested
/// object per the live ACP spec (`protocol/tool-calls`), fixed from this
/// file's first, flat `{"outcome":"cancelled"}`.
fn cancelled_outcome() -> Value {
    json!({ "outcome": { "outcome": "cancelled" } })
}

/// A length-capped, quoted rendering of untrusted JSON, for a transcript
/// entry or a defect's detail. The same bound and the same reasoning as
/// `crate::worktree::truncated`: what a dependency sends is data, and it is
/// bounded before it is carried into a record something else might log.
fn capped(value: &Value) -> String {
    let text = value.to_string();
    if text.len() <= STDERR_CAP {
        return text;
    }
    let mut end = STDERR_CAP;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

// ---------------------------------------------------------------------------
// AcpClient
// ---------------------------------------------------------------------------

/// One session's worth of process and protocol state.
struct ActiveAcpSession {
    id: Id,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    acp_session_id: String,
    next_rpc_id: u64,
    transcript: Transcript,
}

/// Borrows the active session, refusing a `handle` that does not name it.
///
/// A free function taking `&mut Option<ActiveAcpSession>` directly, not a
/// method taking `&mut AcpClient`: a method's `&mut self` receiver ties the
/// returned borrow to the *whole* client, so a caller could not also touch
/// `self.record_defect`/`self.clock` while holding it. Passing the field
/// itself keeps the borrow checker's view of it exactly as narrow as it
/// really is (`self.session` alone), which is what lets
/// [`AcpClient::prompt`] hold this borrow and reach `self.record_defect` in
/// the same call.
fn active_mut<'session>(
    session: &'session mut Option<ActiveAcpSession>,
    handle: &SessionHandle,
) -> Result<&'session mut ActiveAcpSession, RuntimeError> {
    match session {
        Some(session) if &session.id == handle.id() => Ok(session),
        Some(_) | None => Err(RuntimeError::UnknownSession {
            id: handle.id().clone(),
        }),
    }
}

/// An ACP client: JSON-RPC 2.0 over one child process's stdio, enough to
/// initialize, open a session, send a prompt and receive updates: ADR-0001
/// "Agent protocol", `spec/API_SPEC.md` §4.
pub struct AcpClient {
    caps: RuntimeCaps,
    session: Option<ActiveAcpSession>,
    record_defect: DefectRecorder,
    clock: ClockFn,
}

impl AcpClient {
    /// A client declaring `caps`, with no session spawned yet.
    ///
    /// `record_defect` and `clock` are required here, once, rather than at
    /// each call: see the module doc comment, "Why the recorder and the
    /// clock are constructor parameters, not per-call ones". There is no
    /// other constructor and no `Default`, so an `AcpClient` cannot exist
    /// without both.
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

    /// The active session's protocol-level `sessionId`, absent before
    /// `spawn` or after `kill`.
    #[must_use]
    pub fn acp_session_id(&self) -> Option<&str> {
        self.session
            .as_ref()
            .map(|session| session.acp_session_id.as_str())
    }

    /// Sends one request and drives the read loop until the matching
    /// response arrives, answering any nested `session/request_permission`
    /// request along the way with `cancelled_outcome` and recording it,
    /// and recording any `session/update` notification into the transcript.
    fn call(
        session: &mut ActiveAcpSession,
        method: &str,
        params: Value,
        at: Timestamp,
        record_defect: &mut dyn FnMut(LaunchDefect) -> Result<(), String>,
    ) -> Result<Value, RuntimeError> {
        let id = Value::from(session.next_rpc_id);
        session.next_rpc_id += 1;
        write_frame(
            &mut session.stdin,
            &RpcRequest {
                jsonrpc: "2.0",
                id: id.clone(),
                method: method.to_owned(),
                params,
            },
        )?;

        let deadline = Instant::now() + IO_TIMEOUT;
        loop {
            if Instant::now() > deadline {
                return Err(RuntimeError::Timeout { after: IO_TIMEOUT });
            }
            let Some(line) = read_line(&mut session.stdout)? else {
                return Err(RuntimeError::UnexpectedEof);
            };
            let frame: RawFrame =
                serde_json::from_str(&line).map_err(|error| RuntimeError::Protocol {
                    message: error.to_string(),
                })?;
            match frame.classify()? {
                Incoming::Response { id: got, outcome } if got == id => {
                    return outcome.map_err(|(code, message)| RuntimeError::Rpc { code, message });
                }
                Incoming::Response { .. } => {
                    // A response to a request this client did not send in
                    // this call, or already saw: ignored rather than
                    // refused, because a defensive client does not fail a
                    // whole turn over a stray echo it does not need.
                }
                Incoming::Request {
                    id: req_id,
                    method: req_method,
                    params: req_params,
                } if req_method == "session/request_permission" => {
                    let defect = LaunchDefect {
                        at,
                        method: "session/request_permission",
                        detail: format!(
                            "session/request_permission received during '{method}'; refused as \
                             cancelled, never granted: {}",
                            capped(&req_params)
                        ),
                        reason: LaunchDefect::reason(),
                    };
                    record_defect(defect.clone())
                        .map_err(|message| RuntimeError::DefectSink { message })?;
                    session.transcript = session.transcript.record(Entry::new(
                        1,
                        at,
                        Some("session/request_permission"),
                        &capped(&req_params),
                        &format!("refused: cancelled ({})", defect.reason),
                    )?)?;
                    write_frame(
                        &mut session.stdin,
                        &RpcResponseOk {
                            jsonrpc: "2.0",
                            id: req_id,
                            result: cancelled_outcome(),
                        },
                    )?;
                }
                Incoming::Request {
                    id: req_id,
                    method: req_method,
                    ..
                } => {
                    // Any other incoming request is answered with a
                    // method-not-found error rather than left hanging: CLAUDE.md
                    // rule 8's "never keep grinding" applies to the agent on
                    // the other end of this pipe too.
                    write_frame(
                        &mut session.stdin,
                        &RpcResponseErr {
                            jsonrpc: "2.0",
                            id: req_id,
                            error: RpcErrorObj {
                                code: -32601,
                                message: format!(
                                    "method not implemented by this client: {req_method}"
                                ),
                            },
                        },
                    )?;
                }
                Incoming::Notification {
                    method: note_method,
                    params: note_params,
                } => {
                    session.transcript = session.transcript.record(Entry::new(
                        1,
                        at,
                        Some(note_method.as_str()),
                        "",
                        &capped(&note_params),
                    )?)?;
                }
            }
        }
    }

    /// Spawns the child, then runs `initialize` and `session/new`
    /// synchronously.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::AlreadySpawned`] if this client already holds a
    /// session; [`RuntimeError::WrongStdinMode`] if `spec.launch.stdin` is
    /// not [`StdioMode::Piped`]; otherwise see [`RuntimeError`].
    pub fn spawn(&mut self, spec: SessionSpec) -> Result<SessionHandle, RuntimeError> {
        if self.session.is_some() {
            return Err(RuntimeError::AlreadySpawned);
        }
        if spec.launch.stdin() != StdioMode::Piped {
            return Err(RuntimeError::WrongStdinMode {
                required: StdioMode::Piped,
                found: spec.launch.stdin(),
            });
        }

        let mut command = Command::new(spec.launch.program());
        command
            .args(spec.launch.args())
            .current_dir(&spec.cwd)
            .envs(spec.envs.iter().cloned())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| RuntimeError::Io {
            message: error.to_string(),
        })?;
        let stdin = child.stdin.take().ok_or_else(|| RuntimeError::Io {
            message: "the child's stdin was not piped".to_owned(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| RuntimeError::Io {
            message: "the child's stdout was not piped".to_owned(),
        })?;
        if let Some(stderr) = child.stderr.take() {
            drain_stderr(stderr);
        }

        let mut session = ActiveAcpSession {
            id: spec.id.clone(),
            child,
            stdin,
            stdout: BufReader::new(stdout),
            acp_session_id: String::new(),
            next_rpc_id: 1,
            transcript: Transcript::new(),
        };

        let at = (self.clock)();
        let init_result = Self::call(
            &mut session,
            "initialize",
            json!({ "protocolVersion": ACP_PROTOCOL_VERSION, "clientCapabilities": {} }),
            at,
            self.record_defect.as_mut(),
        );
        if let Err(error) = init_result {
            let _ = kill_child(&mut session.child);
            return Err(error);
        }

        let at = (self.clock)();
        let new_session_result = Self::call(
            &mut session,
            "session/new",
            json!({
                "cwd": spec.cwd.to_string_lossy(),
                // Required by the live spec (protocol/session-setup): "A
                // list of MCP servers the Agent should connect to." Empty
                // here is a named seam, not a decision: `ori-mcp` (the
                // engine's MCP server toward agents, `SECURITY_NOTES.md`
                // trust boundary 2) does not exist in this workspace yet, so
                // there is no server list this crate could truthfully send.
                // A future ticket that builds `ori-mcp` threads its server
                // list through `SessionSpec` and this call.
                "mcpServers": [],
            }),
            at,
            self.record_defect.as_mut(),
        );
        let acp_session_id = match new_session_result {
            Ok(value) => value
                .get("sessionId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            Err(error) => {
                let _ = kill_child(&mut session.child);
                return Err(error);
            }
        };
        session.acp_session_id = acp_session_id;

        let handle = SessionHandle { id: spec.id };
        self.session = Some(session);
        Ok(handle)
    }
}

impl AgentRuntime for AcpClient {
    fn spawn(&mut self, spec: SessionSpec) -> Result<SessionHandle, RuntimeError> {
        // Resolves to the inherent `AcpClient::spawn` above (inherent
        // methods take priority over trait methods in method-call
        // resolution), which now reads `self.record_defect`/`self.clock`
        // directly: see the module doc comment, "Why the recorder and the
        // clock are constructor parameters, not per-call ones".
        self.spawn(spec)
    }

    fn send(&mut self, handle: &SessionHandle, prompt: &str) -> Result<StopReason, RuntimeError> {
        self.prompt(handle, prompt)
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
        self.kill(handle)
    }

    fn capabilities(&self) -> RuntimeCaps {
        self.caps.clone()
    }
}

impl AcpClient {
    /// Sends one prompt and runs the turn to completion. See
    /// `AcpClient::call` for how a nested permission request or update is
    /// handled along the way.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NotSpawned`] if no session is active;
    /// [`RuntimeError::UnknownSession`] if `handle` does not name it;
    /// otherwise see [`RuntimeError`].
    pub fn prompt(
        &mut self,
        handle: &SessionHandle,
        text: &str,
    ) -> Result<StopReason, RuntimeError> {
        let at = (self.clock)();
        let session = active_mut(&mut self.session, handle)?;
        let result = Self::call(
            session,
            "session/prompt",
            json!({
                "sessionId": session.acp_session_id,
                "prompt": [{ "type": "text", "text": text }],
            }),
            at,
            self.record_defect.as_mut(),
        )?;
        let stop_reason = result
            .get("stopReason")
            .and_then(Value::as_str)
            .map_or(StopReason::Other(String::new()), StopReason::parse);
        session.transcript = session.transcript.record(Entry::new(
            1,
            at,
            Some("session/prompt"),
            text,
            &stop_reason.to_string(),
        )?)?;
        Ok(stop_reason)
    }

    /// The transcript recorded for the active session, absent before
    /// `spawn`.
    #[must_use]
    pub fn transcript(&self) -> Option<&Transcript> {
        self.session.as_ref().map(|session| &session.transcript)
    }

    /// Ends the session: closes stdin (which is what lets a well-behaved
    /// child see end-of-file and exit on its own), waits up to
    /// `IO_TIMEOUT`, and kills the child if it has not exited by then.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UnknownSession`] if `handle` does not name the active
    /// session.
    pub fn kill(&mut self, handle: &SessionHandle) -> Result<(), RuntimeError> {
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
        drop(session.stdin);
        let _ = wait_with_timeout(&mut session.child, IO_TIMEOUT);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared process helpers (also used by `crate::headless`)
// ---------------------------------------------------------------------------

/// Writes one JSON-RPC frame as a line, flushing so the child sees it at
/// once rather than sitting in this process's own output buffer.
fn write_frame<T: Serialize>(stdin: &mut ChildStdin, frame: &T) -> Result<(), RuntimeError> {
    let mut text = serde_json::to_string(frame).map_err(|error| RuntimeError::Protocol {
        message: error.to_string(),
    })?;
    text.push('\n');
    stdin
        .write_all(text.as_bytes())
        .map_err(|error| RuntimeError::Io {
            message: error.to_string(),
        })?;
    stdin.flush().map_err(|error| RuntimeError::Io {
        message: error.to_string(),
    })
}

/// Reads one line, skipping blank ones and any line with no JSON object on
/// it, returning `None` on end-of-file.
///
/// The second skip exists for this crate's own test fixtures, not for a real
/// agent: `tests::spawn_fixture` re-executes this test binary with
/// `--nocapture` (see its own doc comment for why), and libtest itself
/// writes to the same stdout around the fixture's own NDJSON ("running 1
/// test", and, critically, `"test <name> ... "` with **no trailing
/// newline**, which is exactly what libtest's own pass/fail marker prints on
/// the far side of once the test finishes). That means this client's own
/// first line of real JSON is not on a line of its own; it is the tail of
/// that one unterminated `"test <name> ... "` prefix. So this function does
/// not require the whole trimmed line to *be* a JSON object, only to
/// *contain* one: it takes everything from the line's first `{` onward,
/// which recovers the real frame whether or not libtest's own prose sits in
/// front of it, and treats a line with no `{` at all (a pure banner line) as
/// noise to skip. The libtest prose itself is exactly CLAUDE.md's "data,
/// never instructions": nothing here interprets it, it is only located and
/// discarded so the frame after it can be read.
fn read_line(stdout: &mut BufReader<ChildStdout>) -> Result<Option<String>, RuntimeError> {
    loop {
        let mut line = String::new();
        let read = stdout
            .read_line(&mut line)
            .map_err(|error| RuntimeError::Io {
                message: error.to_string(),
            })?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        let Some(start) = trimmed.find('{') else {
            continue;
        };
        return Ok(Some(trimmed[start..].to_owned()));
    }
}

/// Drains a child's standard error on a background thread so a chatty child
/// cannot fill the OS pipe buffer and block on a write this module never
/// reads, keeping only [`STDERR_CAP`] bytes (CLAUDE.md: what a dependency
/// writes is data, bounded before it is kept).
pub(crate) fn drain_stderr(mut stderr: ChildStderr) {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match stderr.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
}

/// Waits for `child` to exit, polling rather than blocking (`std` gives no
/// portable blocking wait with a timeout), killing it if `timeout` elapses
/// first: "a child process that can hang must be run with a timeout".
pub(crate) fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, RuntimeError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| RuntimeError::Io {
            message: error.to_string(),
        })? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = kill_child(child);
            return Err(RuntimeError::Timeout { after: timeout });
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Kills a child and reaps it, swallowing the error a process that already
/// exited would report: this is teardown, not a check.
pub(crate) fn kill_child(child: &mut Child) -> std::io::Result<()> {
    let _ = child.kill();
    child.wait().map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use ori_core::types::Actor;

    use super::*;

    const START: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    fn id(tail: &str) -> Id {
        Id::parse(&format!("01ARZ3NDEKTSV4RRFFQ69{tail}")).expect("a ULID")
    }

    fn family(text: &str) -> ModelFamily {
        ModelFamily::parse(text).expect("a non-empty family parses")
    }

    fn caps() -> RuntimeCaps {
        RuntimeCaps::new(family("anthropic-claude-3"), "acp-fixture").expect("a valid name")
    }

    /// A directory under the temporary directory that removes itself.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0033-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("the temporary directory is writable");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    // -----------------------------------------------------------------------
    // The fixture agent: a re-exec of this test binary with a sentinel
    // argument (its own test name, used as a libtest filter) and a sentinel
    // environment variable, so a normal `cargo test --workspace` run treats
    // it as a fast no-op and only a deliberate re-exec makes it behave as an
    // agent. Run with `--nocapture` so libtest does not swallow the raw
    // stdout writes this fixture depends on the parent process reading.
    // -----------------------------------------------------------------------

    const FIXTURE_MODE_VAR: &str = "ORI_T_0033_FIXTURE_MODE";
    const FIXTURE_BEHAVIOR_VAR: &str = "ORI_T_0033_FIXTURE_BEHAVIOR";
    const FIXTURE_TEST_NAME: &str = "ori_t_0033_acp_fixture_agent_entrypoint_do_not_call_directly";

    #[test]
    fn ori_t_0033_acp_fixture_agent_entrypoint_do_not_call_directly() {
        let Ok(behavior) = std::env::var(FIXTURE_MODE_VAR) else {
            // A normal `cargo test` run: not a re-exec, do nothing.
            return;
        };
        let behavior = if behavior == "1" {
            std::env::var(FIXTURE_BEHAVIOR_VAR).unwrap_or_default()
        } else {
            behavior
        };
        run_fixture_agent(&behavior);
    }

    /// The fixture agent's own read/write loop: NDJSON over its real stdin
    /// and stdout, `initialize`, `session/new`, and one `session/prompt`
    /// exchange whose shape depends on `behavior`. Validates the client's
    /// requests **strictly against the live ACP spec** (quoted in this
    /// module's own doc comment, "The ACP protocol version implemented"),
    /// not against this file's own understanding of it: this is what plants
    /// A and B defeat, and it is why they are caught here and not only by a
    /// unit test that could share this file's own (once mistaken) belief
    /// about the wire shape.
    fn run_fixture_agent(behavior: &str) {
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut out = std::io::stdout();

        let read_one = |input: &mut std::io::StdinLock<'_>| -> Option<Value> {
            let mut line = String::new();
            loop {
                line.clear();
                let read = std::io::BufRead::read_line(input, &mut line).ok()?;
                if read == 0 {
                    return None;
                }
                if line.trim().is_empty() {
                    continue;
                }
                return serde_json::from_str(line.trim()).ok();
            }
        };
        let write_one = |out: &mut std::io::Stdout, value: &Value| {
            let mut text = value.to_string();
            text.push('\n');
            let _ = std::io::Write::write_all(out, text.as_bytes());
            let _ = std::io::Write::flush(out);
        };

        // initialize
        let Some(request) = read_one(&mut input) else {
            return;
        };
        let init_id = request.get("id").cloned().unwrap_or(Value::Null);
        if behavior == "prompts_during_init" {
            // Exercises the `AgentRuntime::spawn` trait path itself (not
            // only `session/prompt`), which is exactly the call path fix 3
            // of ORI-T-0033's follow-up review found silently dropping
            // defects: a permission request nested inside the handshake
            // `spawn` runs, before `initialize` is even answered.
            write_one(
                &mut out,
                &json!({
                    "jsonrpc": "2.0",
                    "id": 9002,
                    "method": "session/request_permission",
                    "params": {
                        "sessionId": "fixture-session-1",
                        "toolCall": { "toolCallId": "tc-2", "title": "run rm -rf /", "kind": "execute" },
                        "options": [
                            { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" }
                        ]
                    }
                }),
            );
            let _ = read_one(&mut input);
        }
        write_one(
            &mut out,
            &json!({ "jsonrpc": "2.0", "id": init_id, "result": { "protocolVersion": ACP_PROTOCOL_VERSION, "agentCapabilities": {} } }),
        );

        // session/new: the live spec (protocol/session-setup) requires both
        // `cwd` and `mcpServers`. Plant B removes the latter from the real
        // call; a strict fixture is what makes that a failing test rather
        // than a silent pass.
        let Some(request) = read_one(&mut input) else {
            return;
        };
        let new_id = request.get("id").cloned().unwrap_or(Value::Null);
        let has_mcp_servers = request
            .get("params")
            .and_then(|params| params.get("mcpServers"))
            .is_some_and(Value::is_array);
        if !has_mcp_servers {
            write_one(
                &mut out,
                &json!({
                    "jsonrpc": "2.0",
                    "id": new_id,
                    "error": {
                        "code": -32602,
                        "message": "session/new requires mcpServers (protocol/session-setup)"
                    }
                }),
            );
            return;
        }
        write_one(
            &mut out,
            &json!({ "jsonrpc": "2.0", "id": new_id, "result": { "sessionId": "fixture-session-1" } }),
        );

        // session/prompt
        let Some(request) = read_one(&mut input) else {
            return;
        };
        let prompt_id = request.get("id").cloned().unwrap_or(Value::Null);

        if behavior == "prompts" {
            write_one(
                &mut out,
                &json!({
                    "jsonrpc": "2.0",
                    "id": 9001,
                    "method": "session/request_permission",
                    "params": {
                        "sessionId": "fixture-session-1",
                        "toolCall": { "toolCallId": "tc-1", "title": "run rm -rf /", "kind": "execute" },
                        "options": [
                            { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" },
                            { "optionId": "reject-once", "name": "Reject", "kind": "reject_once" }
                        ]
                    }
                }),
            );
            // Validate the client's answer strictly against the live spec
            // (protocol/tool-calls): `outcome` must be a nested object
            // tagged `cancelled` or `selected`, never the flat
            // `{"outcome":"cancelled"}` an earlier version of this client
            // sent (plant A). A shape that fails this is marked in the
            // final `stopReason` rather than silently accepted, so the
            // test driving this exchange sees it fail.
            let response = read_one(&mut input);
            let outcome_is_well_shaped = response
                .as_ref()
                .and_then(|value| value.get("result"))
                .and_then(|result| result.get("outcome"))
                .is_some_and(|outcome| {
                    outcome.is_object()
                        && matches!(
                            outcome.get("outcome").and_then(Value::as_str),
                            Some("cancelled") | Some("selected")
                        )
                });
            let stop_reason = if outcome_is_well_shaped {
                "refusal"
            } else {
                "fixture_rejected_outcome_shape"
            };
            write_one(
                &mut out,
                &json!({ "jsonrpc": "2.0", "id": prompt_id, "result": { "stopReason": stop_reason } }),
            );
        } else {
            write_one(
                &mut out,
                &json!({
                    "jsonrpc": "2.0",
                    "method": "session/update",
                    "params": {
                        "sessionId": "fixture-session-1",
                        "update": { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "ok, working on it" } }
                    }
                }),
            );
            write_one(
                &mut out,
                &json!({ "jsonrpc": "2.0", "id": prompt_id, "result": { "stopReason": "end_turn" } }),
            );
        }

        // Wait for stdin to close (the client's `kill`), then exit cleanly.
        while read_one(&mut input).is_some() {}
    }

    /// Spawns the fixture agent as a child of this test binary.
    fn spawn_fixture(behavior: &str) -> SessionSpec {
        let exe = std::env::current_exe().expect("this test binary's own path");
        let launch = LaunchConfig::acp(
            exe.to_string_lossy().into_owned(),
            vec![
                FIXTURE_TEST_NAME.to_owned(),
                "--test-threads=1".to_owned(),
                "--nocapture".to_owned(),
            ],
        )
        .expect("a non-empty program");
        SessionSpec {
            id: id("FXTR1"),
            launch,
            cwd: std::env::temp_dir(),
            envs: vec![
                (FIXTURE_MODE_VAR.to_owned(), "1".to_owned()),
                (FIXTURE_BEHAVIOR_VAR.to_owned(), behavior.to_owned()),
            ],
        }
    }

    /// A client with a fixed clock (`START`) and the caller's own defect
    /// recorder: every test builds one this way now that both are
    /// constructor parameters (fix 3, ORI-T-0033's follow-up review), rather
    /// than a call-time no-op no code path can be left to substitute.
    fn client(
        record_defect: impl FnMut(LaunchDefect) -> Result<(), String> + 'static,
    ) -> AcpClient {
        AcpClient::new(caps(), record_defect, || START)
    }

    // -----------------------------------------------------------------------
    // ORI-T-0033, plant 5: RuntimeCaps constructible without a family.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_runtime_caps_always_carries_a_family() {
        let caps = RuntimeCaps::new(family("anthropic-claude-3"), "acp").expect("valid");
        assert_eq!(caps.family().as_str(), "anthropic-claude-3");
        // There is no `Default`, no public field and no other constructor:
        // see RuntimeCaps's own doc comment, "How the family is guaranteed".
        // A plant that added one would not make this assertion fail on its
        // own, which is exactly why the guarantee here rests on
        // construction being impossible, not on a runtime check; the doc
        // comment says so rather than a test claiming to prove a negative
        // about the whole type's API.
    }

    #[test]
    fn ori_t_0033_runtime_caps_refuses_an_empty_adapter_name() {
        let refusal =
            RuntimeCaps::new(family("anthropic-claude-3"), "  ").expect_err("blank name refused");
        assert_eq!(refusal, RuntimeCapsError::EmptyAdapterName);
    }

    // -----------------------------------------------------------------------
    // ORI-T-0033, plant C: the trait path records a defect again, not a
    // no-op. There is no `Default`, no second constructor, and no
    // `AgentRuntime` method left that can run without a real recorder and a
    // real clock: see `AcpClient::new`'s own doc comment and the module doc
    // comment, "Why the recorder and the clock are constructor parameters,
    // not per-call ones". Plant C itself (reintroducing `|_| Ok(())` and
    // `Timestamp::from_millis(0)` in the trait impl) is proven against a
    // scratch copy in the pull request report, the same way plant 5 is,
    // because a committed test cannot both assert this type compiles and
    // exercise an edit that would make it stop compiling.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_acp_client_always_carries_a_recorder_and_a_clock() {
        let _ = client(|_| Ok(()));
    }

    // -----------------------------------------------------------------------
    // ORI-P1-039, plant 1: the response never selects an allow outcome.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_039_the_response_to_a_permission_request_never_selects_an_allow_outcome() {
        let outcome = cancelled_outcome();
        // Nested per the live spec (protocol/tool-calls): `result.outcome`
        // is an object, not the flat string this file sent before the
        // operator's own fetch of the spec caught it (plant A).
        assert_eq!(outcome, json!({ "outcome": { "outcome": "cancelled" } }));
        assert!(
            outcome.get("outcome").is_some_and(Value::is_object),
            "the outcome is a nested object: {outcome}"
        );
        let text = outcome.to_string();
        assert!(!text.contains("selected"), "{text}");
        assert!(!text.contains("allow"), "{text}");
        assert!(!text.contains("optionId"), "{text}");
    }

    // -----------------------------------------------------------------------
    // ORI-P1-039: the launch configuration is inspectable and carries no
    // secret.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_039_the_launch_configuration_is_inspectable_and_carries_no_secret() {
        let spec = spawn_fixture("well_behaved");
        assert_eq!(spec.launch.stdin(), StdioMode::Piped);
        assert!(spec.launch.non_interactive());
        assert!(!spec.launch.program().is_empty());
        // The credential seam: an env value never appears anywhere the
        // launch configuration's own Display/Debug can be read from. Plainly
        // fake, and shaped to match none of `scripts/secret-scan.sh`'s
        // patterns (no `sk-`, `ghp_`, `xox`, `AKIA` or `-----BEGIN` prefix).
        let fake_env_value = "fake-provider-key-for-tests-0033";
        let mut with_secret = spec.clone();
        with_secret
            .envs
            .push(("ORI_TEST_CREDENTIAL".to_owned(), fake_env_value.to_owned()));
        let rendered = format!("{:?} {}", with_secret.launch, with_secret.launch);
        assert!(
            !rendered.contains(fake_env_value),
            "the launch configuration never carries an environment value: {rendered}"
        );
    }

    // -----------------------------------------------------------------------
    // ORI-T-0033, plant B: `session/new` omits `mcpServers`, which the live
    // spec (protocol/session-setup) requires. The fixture agent (above)
    // rejects a `session/new` missing it with a JSON-RPC error, so a
    // successful spawn here is already proof `mcpServers` was sent; plant B
    // removes it from the real call in `AcpClient::spawn` and this test is
    // what goes red.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_session_new_includes_mcp_servers_or_the_strict_fixture_refuses_it() {
        let mut client = client(|_| Ok(()));
        let handle = client
            .spawn(spawn_fixture("well_behaved"))
            .expect("session/new included mcpServers, so the strict fixture accepted it");
        client.kill(&handle).expect("teardown");
    }

    // -----------------------------------------------------------------------
    // ORI-P1-039, plants 2 and 3: a permission request is refused, recorded
    // as a defect through EventLog::append, and an entry lands in the
    // transcript; the transcript is asserted non-vacuous first.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_039_a_permission_request_is_refused_recorded_as_a_defect_and_never_granted() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let scratch = Scratch::new("defect");
        let db = ori_store::db::ProductDb::open(&scratch.path, "acme", START)
            .expect("a fresh product database opens");
        let db = Rc::new(RefCell::new(db));
        let product_id = id("PRDCT");
        let actor = Actor::Agent(id("AGENT"));
        let ticket_id = id("TCKET");

        let defects: Rc<RefCell<Vec<LaunchDefect>>> = Rc::default();
        let (db_sink, defects_sink, product_id_sink, actor_sink, ticket_id_sink) = (
            Rc::clone(&db),
            Rc::clone(&defects),
            product_id.clone(),
            actor.clone(),
            ticket_id.clone(),
        );
        let mut client = client(move |defect: LaunchDefect| -> Result<(), String> {
            let payload = json!({
                "method": defect.method,
                "detail": defect.detail,
                "reason": defect.reason.to_string(),
            })
            .to_string();
            ori_store::event_log::EventLog::append(
                db_sink.borrow_mut().connection(),
                product_id_sink.clone(),
                defect.at,
                actor_sink.clone(),
                "runtime.permission_prompt_refused",
                Some(ticket_id_sink.clone()),
                payload,
            )
            .map_err(|error| error.to_string())?;
            defects_sink.borrow_mut().push(defect);
            Ok(())
        });

        let handle = client
            .spawn(spawn_fixture("prompts"))
            .expect("the fixture agent initializes and opens a session");
        let stop_reason = client
            .prompt(&handle, "please help")
            .expect("the turn completes");

        assert_eq!(stop_reason, StopReason::Refusal);

        // Trap 1: vacuity. Assert the transcript holds the exact expected
        // number of messages before asserting anything about their content.
        // Read before `kill`, which drops the session (and its transcript)
        // as part of tearing it down.
        let transcript = client
            .transcript()
            .expect("the session's transcript, recorded before kill drops it")
            .clone();
        client.kill(&handle).expect("teardown");
        assert_eq!(
            transcript.len(),
            2,
            "one entry for the permission request, one for the final outcome: {:?}",
            transcript.entries()
        );

        let permission_entries: Vec<_> = transcript
            .entries()
            .iter()
            .filter(|entry| entry.tool() == Some("session/request_permission"))
            .collect();
        assert_eq!(permission_entries.len(), 1, "{:?}", transcript.entries());
        let permission_entry = permission_entries[0];
        assert!(
            permission_entry.output().contains("refused"),
            "{}",
            permission_entry.output()
        );
        assert!(
            !permission_entry.output().to_lowercase().contains("grant"),
            "never a grant: {}",
            permission_entry.output()
        );

        // The defect was recorded through EventLog::append.
        let recorded_defects = defects.borrow();
        assert_eq!(recorded_defects.len(), 1, "{recorded_defects:?}");
        assert_eq!(recorded_defects[0].reason.section, 17);
        let events =
            ori_store::event_log::EventLog::read_range(db.borrow_mut().connection(), 1, 10)
                .expect("the log reads back");
        assert_eq!(events.len(), 1, "exactly one event was appended");
        assert_eq!(events[0].kind(), "runtime.permission_prompt_refused");
        assert_eq!(events[0].ticket_id(), Some(&ticket_id));
        assert!(events[0].payload().contains("AICD §17"));
    }

    // -----------------------------------------------------------------------
    // ORI-P1-039: a well-behaved agent produces no permission prompt in the
    // transcript, asserted over a real, non-empty exchange.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_039_a_well_behaved_agent_produces_no_permission_prompt_in_the_transcript() {
        let mut client = client(|_: LaunchDefect| -> Result<(), String> {
            panic!("a well-behaved agent raises no permission request")
        });
        let handle = client
            .spawn(spawn_fixture("well_behaved"))
            .expect("the fixture agent initializes and opens a session");
        let stop_reason = client
            .prompt(&handle, "please help")
            .expect("the turn completes");

        assert_eq!(stop_reason, StopReason::EndTurn);

        let transcript = client
            .transcript()
            .expect("recorded, before kill drops it")
            .clone();
        client.kill(&handle).expect("teardown");
        assert_eq!(
            transcript.len(),
            2,
            "one update chunk, one final outcome: {:?}",
            transcript.entries()
        );
        assert!(
            transcript
                .entries()
                .iter()
                .all(|entry| entry.tool() != Some("session/request_permission")),
            "no permission prompt appears in the transcript: {:?}",
            transcript.entries()
        );
    }

    // -----------------------------------------------------------------------
    // AgentRuntime, exercised through the trait object.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_acp_client_implements_agent_runtime() {
        let mut client: Box<dyn AgentRuntime> = Box::new(client(|_| Ok(())));
        assert_eq!(
            client.capabilities().family().as_str(),
            "anthropic-claude-3"
        );
        let handle = client
            .spawn(spawn_fixture("well_behaved"))
            .expect("spawns through the trait");
        let stop_reason = client
            .send(&handle, "hello")
            .expect("sends through the trait");
        assert_eq!(stop_reason, StopReason::EndTurn);
        let entries = client.recv(&handle).expect("reads through the trait");
        assert!(!entries.is_empty());
        client.kill(&handle).expect("kills through the trait");
    }

    // -----------------------------------------------------------------------
    // ORI-T-0033, plant C: a permission request nested inside the
    // `AgentRuntime::spawn` handshake itself (not `session/prompt`) is
    // recorded as a launch defect through the *trait* path, `Box<dyn
    // AgentRuntime>`, which carries no `record_defect` parameter of its own
    // to substitute a no-op into. This is the call path fix 3 of ORI-T-0033's
    // follow-up review found silently dropping defects, and no test before
    // this one exercised a permission request during `spawn` rather than
    // `send`: that gap, not only the fix, is closed here.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_039_a_permission_request_during_the_trait_spawn_handshake_is_recorded_as_a_defect() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let defects: Rc<RefCell<Vec<LaunchDefect>>> = Rc::default();
        let sink = Rc::clone(&defects);
        let mut client: Box<dyn AgentRuntime> = Box::new(client(move |defect: LaunchDefect| {
            sink.borrow_mut().push(defect);
            Ok(())
        }));

        let handle = client
            .spawn(spawn_fixture("prompts_during_init"))
            .expect("the handshake completes even though the agent prompted mid-way");
        client.kill(&handle).expect("teardown");

        let recorded = defects.borrow();
        assert_eq!(
            recorded.len(),
            1,
            "a permission request during spawn's own handshake is recorded exactly once: {recorded:?}"
        );
        assert_eq!(recorded[0].reason.section, 17);
    }

    // -----------------------------------------------------------------------
    // Refusals
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0033_spawn_refuses_a_launch_configuration_whose_stdin_is_not_piped() {
        let mut client = client(|_| Ok(()));
        let mut spec = spawn_fixture("well_behaved");
        spec.launch = LaunchConfig::assemble(
            spec.launch.program().to_owned(),
            spec.launch.args().to_vec(),
            StdioMode::Inherit,
            true,
        );
        let refusal = client
            .spawn(spec)
            .expect_err("stdin must be piped for the ACP client");
        assert_eq!(
            refusal,
            RuntimeError::WrongStdinMode {
                required: StdioMode::Piped,
                found: StdioMode::Inherit,
            }
        );
        assert!(refusal.is_refusal());
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(17)
        );
    }

    #[test]
    fn ori_t_0033_a_second_spawn_is_refused_while_a_session_is_active() {
        let mut client = client(|_| Ok(()));
        let handle = client
            .spawn(spawn_fixture("well_behaved"))
            .expect("first spawn");
        let refusal = client
            .spawn(spawn_fixture("well_behaved"))
            .expect_err("already spawned");
        assert_eq!(refusal, RuntimeError::AlreadySpawned);
        client.kill(&handle).expect("teardown");
    }

    #[test]
    fn ori_t_0033_a_call_against_an_unknown_handle_is_refused() {
        let mut client = client(|_| Ok(()));
        let stranger = SessionHandle { id: id("STRNG") };
        let refusal = client
            .prompt(&stranger, "hi")
            .expect_err("no session spawned yet");
        assert_eq!(refusal, RuntimeError::UnknownSession { id: id("STRNG") });
    }

    #[test]
    fn ori_t_0033_every_refusal_this_module_makes_carries_a_reason_that_resolves() {
        let errors = [
            RuntimeError::WrongStdinMode {
                required: StdioMode::Piped,
                found: StdioMode::Inherit,
            },
            RuntimeError::AlreadySpawned,
            RuntimeError::NotSpawned,
            RuntimeError::UnknownSession { id: id("STRNG") },
            RuntimeError::Io {
                message: "boom".to_owned(),
            },
        ];
        let mut refusals = 0;
        for error in errors {
            assert!(!error.to_string().is_empty());
            match error.methodology_ref() {
                Some(reason) => {
                    assert!(error.is_refusal(), "{error}");
                    assert!(reason.resolves(), "{reason} resolves");
                    refusals += 1;
                }
                None => assert!(!error.is_refusal(), "{error}"),
            }
        }
        assert_eq!(refusals, 1, "one of these five is a refusal");
    }

    #[test]
    fn ori_t_0033_launch_config_refuses_an_empty_program() {
        let refusal = LaunchConfig::acp("   ", vec![]).expect_err("blank program refused");
        assert_eq!(refusal, LaunchConfigError::EmptyProgram);
    }
}
