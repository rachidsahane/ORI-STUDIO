//! The MCP wire shared by the server and the host: AICD §8.
//!
//! One synchronous, transport-agnostic module that both halves of this crate
//! use: the server toward agents (`spec/API_SPEC.md` section 3) and the host
//! toward the operator's servers (`ops/batch-10-plan.md`, decision D2 and slice
//! W0). It reads and writes JSON-RPC 2.0 messages one per line, types the MCP
//! messages phase 1 needs, negotiates the protocol version, and keeps the
//! connection state machine as data. It holds **no scope decision, no tool and
//! no policy**: which role may call which tool is `ToolScopes`, and what the
//! server answers is the server core; both are tier 2 and neither is here.
//!
//! # Sources, and what wins when they differ
//!
//! The Model Context Protocol specification, version `2025-11-25` (the
//! constant [`crate::protocol::LATEST_PROTOCOL_VERSION`]), read from the
//! specification's own repository: its TypeScript schema (the normative text),
//! and the base overview, lifecycle, transports and server tools pages. Where a
//! page and the schema disagree the schema wins. They differ in words in one
//! place that matters here: the overview says an id is an integer and the
//! schema says `number`, which cannot say integer, so they do not conflict and
//! the narrower reading is kept (below). Separately, the error response's
//! optional `id` is where the `2025-11-25` schema differs from the previous
//! version's.
//!
//! `ADR-0001` (`spec/adr/ADR-0001-stack.md`) fixes "Official MCP semantics" for
//! the engine's MCP surface and JSON-RPC 2.0 as the shape of its messages, and
//! escalation E-0008 approved the JSON crate. The versioning page that would
//! state the version string format was never obtained: its fetch failed, and the
//! file that stands for it holds a 404 body (`404: Not Found`), not a page. The
//! `YYYY-MM-DD` shape this module checks is therefore inferred from the three
//! version strings the other files contain (`2025-11-25`, `2025-06-18` and the
//! lifecycle page's `2024-11-05`).
//!
//! # Framing
//!
//! The stdio transport: one message per line, UTF-8, "MUST NOT contain
//! embedded newlines", nothing else on the stream. [`crate::protocol::LineReader`]
//! reads a line from any `BufRead` and refuses one longer than a cap, without
//! buffering it: it looks at the reader's own buffer, copies only what fits
//! under the cap, and gives up the moment a line cannot. The line buffer is
//! never grown past the cap, so what the reader holds is at most the cap plus
//! whatever the underlying reader buffers (parsing a line costs more, see
//! below); `crates/ori-runtime/src/acp.rs` reads lines the
//! same way (`fill_buf`, scan for the newline, check the length before copying),
//! and this is that approach rewritten, not imported, because `ori-mcp` does not
//! depend on `ori-runtime`. Unlike `acp.rs` there is no thread and no queue: a
//! read blocks, and deadlines belong to the caller.
//!
//! After a refusal the reader stays refused. A line that was too long has its
//! tail still in the stream, and a tail the peer chose can itself be a
//! well-formed message; reading on would hand it to the caller as one. `acp.rs`
//! also spends the session on an over-long line, because "the stream is then out
//! of step"; this is the same decision with the smuggling case spelled out. The
//! caller answers the refusal once, with an error response that has no id (the
//! line's id is unknown), and then closes the connection; "Errors" below states
//! the rule for when an error is answered.
//!
//! [`crate::protocol::MAX_LINE_BYTES`] is 1 MiB, the figure `acp.rs` uses. It is
//! the default for **agent connections** (slice S2), whose inputs are
//! length-capped, and it is not a figure for every line an operator's server may
//! send, so the host (slice H2) chooses its own:
//!
//! - **Requests from an agent** are tool inputs, which `spec/SECURITY_NOTES.md`
//!   ("Input validation") says are validated and length-capped. The largest, a
//!   plan or a report, is a small fraction of a mebibyte.
//! - **Results from an operator's server** are not so bounded. A server can
//!   legitimately send a line longer than 1 MiB, and a limit on what this side
//!   later forwards to an agent cannot bound the line a server sends to the
//!   host, which is read first. The host must therefore choose its own cap
//!   through [`crate::protocol::LineReader::with_max_line_bytes`], weighing the
//!   memory-per-byte figure in the last bullet. A line over the cap ends that
//!   connection (see "After a refusal" above), which is fail-closed: the host
//!   loses one server's connection and never reads past a line it did not
//!   accept. The line is longer than the text it carries: JSON escaping can
//!   write a control character as 6 bytes (`\u0001`) and a 4-byte character as
//!   a 12-byte surrogate pair (3 times its size).
//! - **Binary blocks are the case 1 MiB does not cover.** An image or audio block
//!   is base64, 4/3 of the file: with the default cap, a screenshot over about
//!   768 KiB cannot be read and ends the connection, although the host would only
//!   have replaced it with a placeholder. A host that decides to tolerate them
//!   raises the cap for operator connections; each step up is memory per
//!   connection (next item).
//! - **The cap bounds the line, not what parsing it costs.** Parsing builds a
//!   [`serde_json::Value`] tree that can be far larger than the text. Measured
//!   once for this module (a counting allocator, `serde_json` 1.0.151 as pinned,
//!   a release build, a line of 1 MiB), the tree was about the size of the text
//!   for one long string or for ordinary escaped text, 8 times for an array of
//!   one-character strings, 16 times for an array of integers or of empty
//!   objects, and 83 times (87 MiB) for an array of one-key objects such as
//!   `{"a":1}`. A hostile peer can therefore make one parse hold on the order of
//!   90 MiB at the default cap, transiently and once per connection. That is the
//!   whole of what was measured, the tree by itself, and no test re-measures it.
//!   What decoding the tree into a typed value adds is reasoned, not measured.
//!   Decoding takes the message by value
//!   ([`crate::protocol::Request::into_params`],
//!   [`crate::protocol::Notification::into_params`],
//!   [`crate::protocol::ResultResponse::into_result`]) through
//!   `serde_json::from_value`, which moves each string and each object key out
//!   of the tree into the typed value instead of copying it. The peak for one
//!   message is then the parsed tree plus a transient (the containers being
//!   rebuilt at that moment), not twice the tree. Deserializing from `&Value`,
//!   which [`crate::protocol::Request::params_as`] and
//!   [`crate::protocol::ResultResponse::result_as`] do, copies every string,
//!   map and nested value out of the tree and would cost about a second tree;
//!   those two are for a message this side built, never for a peer's. The one
//!   thing a test can pin without measuring memory is pinned: a large string
//!   reaches the typed value at the address it had in the tree. The host reads
//!   operator results, the large case, and pays this figure for every
//!   connection; the server reads small requests and may pick a smaller cap
//!   than the default for agent connections. The figure scales with the cap.
//!
//! This settles the cap for MCP only. ORI-T-0149's question for ACP is a
//! different protocol with different frames (a diff carries a file); nothing
//! here answers it. The writer does not cap: how large a result the server
//! returns is the tool layer's bound ("a bounded context package", AICD §25).
//!
//! # JSON-RPC
//!
//! - **Ids** are a string or an integer. The overview page says "string or
//!   integer" and "MUST NOT be `null`"; the schema says `string | number`, which
//!   is TypeScript's only numeric type and cannot say integer, so the page's
//!   narrower reading is followed: a fractional or oversized number is refused
//!   with -32600, and `null` is refused. An id is echoed back exactly.
//! - **An error response without an id.** The `2025-11-25` schema declares the
//!   error response's `id` optional (`id?: RequestId`); the `2025-06-18` schema
//!   required it. This module writes an error response to an unreadable input
//!   with no `id` member (never `"id": null`, which the schema's type does not
//!   allow), and reads one with `id` absent or `null`. That is the concrete
//!   reason this module supports `2025-11-25` only.
//! - **Batches are refused** with -32600. Neither schema file contains a batch
//!   type or the word "batch": `JSONRPCMessage` is the union of request,
//!   notification and the two responses, and the transports page says "Messages
//!   are individual JSON-RPC requests, notifications, or responses". The files
//!   do not state a refusal outright; the refusal rests on that absence and on
//!   decision D2.
//! - **`params`** is an object when present (the schema's
//!   `params?: { [key: string]: any }`), never an array and never `null`.
//! - **Unknown members** of a message envelope and of a typed structure are
//!   ignored, so a newer peer's additions do not break an older reader; fields
//!   a type does not list (icons, `_meta`, annotations on text) are dropped on
//!   read and never written.
//! - **Duplicate members** in one object are not refused: `serde_json` keeps the
//!   last, so `{"method":"a","method":"b"}` is a call to `b`. Nothing here
//!   compares two parsers, so this matters only if a gateway ever sits between
//!   the peers and reads such a message differently.
//! - **Nesting** is bounded by `serde_json`'s recursion limit (128), because the
//!   `unbounded_depth` feature is not enabled: a line of a million `[` is a parse
//!   error, not a stack overflow.
//!
//! # The messages
//!
//! `initialize`, `notifications/initialized`, `ping`, `tools/list`,
//! `tools/call` and `notifications/cancelled`, plus `notifications/tools/list_changed`
//! because a host must hear that a server's list changed. Everything else is
//! "method not found" for a request ([`crate::protocol::KnownRequest::decode`])
//! and ignored for a notification ([`crate::protocol::KnownNotification::decode`]).
//!
//! **Each direction has its own method set**, and both decoders take the
//! [`crate::protocol::Direction`] the message travelled. A server receives
//! `initialize`, `ping`, `tools/list` and `tools/call`, and the notifications
//! `notifications/initialized` and `notifications/cancelled`. A client receives
//! `ping` and nothing else as a request, and the notifications
//! `notifications/cancelled` and `notifications/tools/list_changed`: phase 1's
//! client offers no sampling, roots or elicitation (the `sampling`, `roots` and
//! `elicitation` members of `ClientCapabilities` in `schema-2025-11-25.ts`, which
//! are what would allow a server to send `sampling/createMessage`,
//! `roots/list` or `elicitation/create`), and no tasks (its `tasks` member), so a
//! server's `initialize`, `tools/list` or `tools/call`, like any other request a
//! server sends, is "method not found". The lifecycle table
//! ([`crate::protocol::ConnectionState::permits`]) is about phases and does not
//! know methods; direction is the decoder's. A known notification travelling the
//! wrong way is ignored like an unknown one, because nothing answers a
//! notification.
//!
//! **Content blocks.** [`crate::protocol::ContentBlock`] types the text block and
//! carries every other block (image, audio, resource link, embedded resource) as
//! the raw JSON object it arrived as, with its `type` tag checked. The host
//! turns a non-text block into a placeholder and never forwards its bytes, so it
//! needs the tag and nothing else; typing each shape would bind the host to
//! blocks it discards, and would make a block type newer than this file's schema
//! fail a whole result that is otherwise fine.
//!
//! # Version negotiation
//!
//! Exactly `2025-11-25` is supported ([`crate::protocol::SUPPORTED_PROTOCOL_VERSIONS`]).
//! `2025-06-18` is not added because nothing in the files asks for it and
//! because of the error-response difference above: this side writes an
//! id-less error response, which a `2025-06-18` peer's schema does not allow.
//! The consequence for the host is that an operator's server that only speaks an
//! older version is refused ([`crate::protocol::check_server_version`]). The
//! lifecycle page words that as "SHOULD disconnect", but the schema wins over the
//! prose and reads "If the client cannot support this version, it MUST
//! disconnect" (`schema-2025-11-25.ts`, `InitializeResult.protocolVersion`), so
//! the refusal is a must. Adding a version later is one entry in that array plus
//! a review of this paragraph.
//!
//! # Connection state
//!
//! [`crate::protocol::ConnectionState`] is the lifecycle page's three phases as
//! a value. It decides nothing by itself: the server and the host ask it
//! ([`crate::protocol::ConnectionState::permits`]) and tell it what happened
//! ([`crate::protocol::ConnectionState::advance`]).
//!
//! ```mermaid
//! stateDiagram-v2
//!   [*] --> Fresh
//!   Fresh --> Initialized: initialize answered with a result
//!   Initialized --> Ready: notifications/initialized
//!   Ready --> [*]: the transport closes
//! ```
//!
//! The page says the client "SHOULD NOT send requests other than pings before
//! the server has responded to the `initialize` request" and the server "SHOULD
//! NOT send requests other than pings and logging before receiving the
//! `initialized` notification", and that the client "MUST send an `initialized`
//! notification to indicate it is ready to begin normal operations". This module
//! reads the last sentence strictly: operation begins at `Ready`, so a request
//! other than a ping that arrives in `Initialized` is out of order. A server that
//! wants to be lenient there changes one arm of
//! [`crate::protocol::ConnectionState::permits`].
//!
//! # Errors
//!
//! Every error this module returns is a protocol error: a JSON-RPC error code
//! ([`crate::protocol::ErrorCode`]) or a framing failure that has one. None is a
//! control refusal, so none carries a `MethodologyRef`; `acp.rs` follows the
//! same convention for its `Protocol` and `Io` errors. A request refused on
//! policy grounds (a role without a tool, a scope, a cap on a result) is not
//! this module's to refuse.
//!
//! **What an error carries of the peer's text.** The `message` of an error is
//! fixed text, and [`std::fmt::Display`] prints the message and the code and never
//! the data. Peer text can reach `data`, in two forms, both cut to a number of
//! characters: an echo of a method name or a requested version (`method`,
//! `requested`), cut to `MAX_ECHO_CHARS` (128); and `detail`, the description
//! `serde_json` gives of why parsing or decoding failed, cut to `MAX_DETAIL_CHARS`
//! (256), which can quote what the peer sent (the offending value or an unknown
//! variant, for instance). A caller that logs `data` is logging peer input.
//!
//! **When to answer a protocol error.** One rule, restated at every place that
//! returns an error to a caller who might answer it:
//!
//! - an error carrying an id answers that request;
//! - an error with no id, raised on input that could not be read as a request,
//!   is answered once with an id-less error response (the `2025-11-25` schema's
//!   `JSONRPCErrorResponse` has `id?`);
//! - an error raised on a notification, whether decoding its parameters or
//!   checking it against the connection state, is never answered, because
//!   nothing answers a notification.

use std::fmt;
use std::io::{self, BufRead, ErrorKind, Write};

use serde::de::{self, DeserializeOwned};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value, json};

/// A JSON object, the shape of MCP's `params`, `result` and capability values.
pub type JsonObject = Map<String, Value>;

/// The JSON-RPC version member every message carries.
pub const JSONRPC_VERSION: &str = "2.0";

/// The MCP protocol version this module speaks, the schema's
/// `LATEST_PROTOCOL_VERSION`.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";

/// The protocol versions this side answers with: exactly one, see the module
/// documentation, "Version negotiation".
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[LATEST_PROTOCOL_VERSION];

/// The longest line, in bytes and without its newline, a [`LineReader`]
/// accepts by default: 1 MiB.
///
/// It is the cap `acp.rs` uses and the default for agent connections, whose
/// inputs are length-capped. An operator's server can legitimately send a longer
/// line, so the host chooses its own cap with
/// [`LineReader::with_max_line_bytes`]; the module documentation, "Framing", says
/// why, and what each byte of cap costs in memory. A line over the cap ends the
/// connection, which is fail-closed. The cap bounds the memory a peer can make
/// this process hold for one line, not for parsing it.
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

/// The longest echo of peer text (a method name, a requested version) placed in
/// an error's `data`, in characters.
const MAX_ECHO_CHARS: usize = 128;

/// The longest parser or deserializer detail placed in an error's `data`, in
/// characters.
const MAX_DETAIL_CHARS: usize = 256;

/// The method name of the initialize request.
pub const METHOD_INITIALIZE: &str = "initialize";
/// The method name of the ping request, valid in every state.
pub const METHOD_PING: &str = "ping";
/// The method name of the tool listing request.
pub const METHOD_TOOLS_LIST: &str = "tools/list";
/// The method name of the tool call request.
pub const METHOD_TOOLS_CALL: &str = "tools/call";
/// The notification a client sends once initialization is done.
pub const NOTIFICATION_INITIALIZED: &str = "notifications/initialized";
/// The notification that cancels an in-flight request.
pub const NOTIFICATION_CANCELLED: &str = "notifications/cancelled";
/// The notification a server sends when its tool list changed.
pub const NOTIFICATION_TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
/// The logging notification, the only non-ping message a server may send
/// before it has received `notifications/initialized`.
pub const NOTIFICATION_MESSAGE: &str = "notifications/message";

// ---------------------------------------------------------------------------
// Error codes and the errors this module returns
// ---------------------------------------------------------------------------

/// JSON-RPC: invalid JSON was received.
pub const PARSE_ERROR: i64 = -32700;
/// JSON-RPC: the JSON sent is not a valid request object.
pub const INVALID_REQUEST: i64 = -32600;
/// JSON-RPC: the method does not exist or is not available.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC: invalid method parameters.
pub const INVALID_PARAMS: i64 = -32602;
/// JSON-RPC: internal error.
pub const INTERNAL_ERROR: i64 = -32603;

/// A JSON-RPC error code, the five the schema names and any other integer.
///
/// Build one from a received integer with [`ErrorCode::from_code`], which maps
/// the five standard values to their variants; `Other(-32600)` compares unequal
/// to `InvalidRequest`, so do not construct `Other` for a standard code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ErrorCode {
    /// -32700: the line is not JSON.
    ParseError,
    /// -32600: the JSON is not a valid message (and every batch).
    InvalidRequest,
    /// -32601: the method is not one this side serves.
    MethodNotFound,
    /// -32602: the method's parameters are wrong, or a version is malformed.
    InvalidParams,
    /// -32603: a failure of this side, not of the peer.
    InternalError,
    /// Any other integer, an implementation-specific code.
    Other(i64),
}

impl ErrorCode {
    /// The integer that goes on the wire.
    #[must_use]
    pub const fn code(self) -> i64 {
        match self {
            Self::ParseError => PARSE_ERROR,
            Self::InvalidRequest => INVALID_REQUEST,
            Self::MethodNotFound => METHOD_NOT_FOUND,
            Self::InvalidParams => INVALID_PARAMS,
            Self::InternalError => INTERNAL_ERROR,
            Self::Other(code) => code,
        }
    }

    /// The code for a received integer: a standard value becomes its named
    /// variant, anything else becomes `Other`.
    #[must_use]
    pub const fn from_code(code: i64) -> Self {
        match code {
            PARSE_ERROR => Self::ParseError,
            INVALID_REQUEST => Self::InvalidRequest,
            METHOD_NOT_FOUND => Self::MethodNotFound,
            INVALID_PARAMS => Self::InvalidParams,
            INTERNAL_ERROR => Self::InternalError,
            other => Self::Other(other),
        }
    }
}

/// A failure at the protocol level, carrying the JSON-RPC code it maps to.
///
/// Not a control refusal and not a `MethodologyRef` holder: see the module
/// documentation, "Errors". The message is fixed text; peer text appears only,
/// bounded, in [`ProtocolError::data`]. [`fmt::Display`] prints the message and
/// the code and never the data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolError {
    code: ErrorCode,
    message: String,
    id: Option<RequestId>,
    data: Option<Value>,
}

impl ProtocolError {
    /// A protocol error with a code and a message, no id and no data.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            id: None,
            data: None,
        }
    }

    /// The same error carrying the id of the request it answers, when one
    /// could be read.
    #[must_use]
    pub fn with_id(mut self, id: Option<RequestId>) -> Self {
        self.id = id;
        self
    }

    /// The same error carrying structured detail for the peer.
    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// The JSON-RPC code.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// The fixed message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The id of the offending request, present only when the input had the
    /// shape of a request and its id was readable: the id to answer with. The
    /// module documentation, "Errors", gives the rule for when to answer.
    #[must_use]
    pub const fn id(&self) -> Option<&RequestId> {
        self.id.as_ref()
    }

    /// Structured detail for the peer, if any.
    #[must_use]
    pub const fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    /// The error response to send for this error: its id when it has one, and
    /// no id member otherwise (see the module documentation, "JSON-RPC").
    #[must_use]
    pub fn to_error_response(&self) -> ErrorResponse {
        ErrorResponse {
            id: self.id.clone(),
            error: ErrorObject {
                code: self.code.code(),
                message: self.message.clone(),
                data: self.data.clone(),
            },
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message, self.code.code())
    }
}

impl std::error::Error for ProtocolError {}

/// Why a line could not be read or a message could not be written.
///
/// Every variant maps to a JSON-RPC code ([`FrameError::code`]); none is a
/// control refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FrameError {
    /// A line was longer than the cap. The reader stays refused afterwards.
    TooLong {
        /// The cap that was exceeded, in bytes.
        limit: usize,
    },
    /// The underlying reader or writer failed.
    Io {
        /// What kind of failure it was.
        kind: ErrorKind,
        /// The operating system's description, for a log.
        message: String,
    },
    /// A message to be written contained a newline. Nothing was written.
    EmbeddedNewline,
    /// A message could not be serialized. Nothing was written.
    Serialize {
        /// The serializer's description, for a log.
        message: String,
    },
}

impl FrameError {
    /// The JSON-RPC code this failure maps to: an over-long line is -32600
    /// (it was refused unread, so it is not known to be invalid JSON, which
    /// would be -32700), and the rest are this side's own -32603.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::TooLong { .. } => ErrorCode::InvalidRequest,
            Self::Io { .. } | Self::EmbeddedNewline | Self::Serialize { .. } => {
                ErrorCode::InternalError
            }
        }
    }

    /// This failure as a [`ProtocolError`], with a fixed message.
    #[must_use]
    pub fn to_protocol_error(&self) -> ProtocolError {
        let message = match self {
            Self::TooLong { .. } => "message line too long",
            Self::Io { .. } => "transport failure",
            Self::EmbeddedNewline => "message contains a newline",
            Self::Serialize { .. } => "message could not be serialized",
        };
        let error = ProtocolError::new(self.code(), message);
        match self {
            Self::TooLong { limit } => error.with_data(json!({ "limit": limit })),
            _ => error,
        }
    }
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { limit } => write!(f, "a line is longer than {limit} bytes"),
            Self::Io { kind, message } => write!(f, "transport failure ({kind}): {message}"),
            Self::EmbeddedNewline => f.write_str("a message to write contains a newline"),
            Self::Serialize { message } => {
                write!(f, "a message could not be serialized: {message}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io {
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

/// Why [`LineReader::read_message`] returned no message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReceiveError {
    /// The stream failed or a line was refused; nothing more can be trusted
    /// from this reader.
    Frame(FrameError),
    /// This one line was not a valid message; the next line may be fine.
    Message(ProtocolError),
}

impl ReceiveError {
    /// Whether the connection must be closed after this error.
    #[must_use]
    pub const fn is_stream_fatal(&self) -> bool {
        matches!(self, Self::Frame(_))
    }

    /// This error as a [`ProtocolError`], to answer the peer with by the rule in
    /// the module documentation, "Errors": the error's own id when it has one,
    /// otherwise one id-less error response.
    #[must_use]
    pub fn to_protocol_error(&self) -> ProtocolError {
        match self {
            Self::Frame(error) => error.to_protocol_error(),
            Self::Message(error) => error.clone(),
        }
    }
}

impl fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(f),
            Self::Message(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ReceiveError {}

/// At most [`MAX_ECHO_CHARS`] characters of peer text, for an error's `data`.
fn echo(text: &str) -> String {
    text.chars().take(MAX_ECHO_CHARS).collect()
}

/// At most [`MAX_DETAIL_CHARS`] characters of a deserializer's description.
fn detail(error: &impl fmt::Display) -> String {
    error.to_string().chars().take(MAX_DETAIL_CHARS).collect()
}

fn invalid_request(message: &'static str) -> ProtocolError {
    ProtocolError::new(ErrorCode::InvalidRequest, message)
}

// ---------------------------------------------------------------------------
// Framing
// ---------------------------------------------------------------------------

/// Reads newline-delimited lines from a `BufRead`, refusing one longer than a
/// cap without buffering it: the stdio transport's framing, see the module
/// documentation, "Framing".
///
/// The buffer holding the current line never grows past the cap. A final line
/// with no newline before end of input is returned as a line; a blank line is
/// returned as an empty line (it is not a message, and [`parse_message`] says
/// so). After an error from the underlying reader or an over-long line the
/// reader keeps returning that error.
#[derive(Debug)]
pub struct LineReader<R> {
    inner: R,
    max_line_bytes: usize,
    line: Vec<u8>,
    failed: Option<FrameError>,
}

impl<R: BufRead> LineReader<R> {
    /// A reader with the default cap, [`MAX_LINE_BYTES`].
    #[must_use]
    pub fn new(inner: R) -> Self {
        Self::with_max_line_bytes(inner, MAX_LINE_BYTES)
    }

    /// A reader with a chosen cap in bytes, newline not counted.
    #[must_use]
    pub fn with_max_line_bytes(inner: R, max_line_bytes: usize) -> Self {
        Self {
            inner,
            max_line_bytes,
            line: Vec::new(),
            failed: None,
        }
    }

    /// The cap this reader enforces.
    #[must_use]
    pub const fn max_line_bytes(&self) -> usize {
        self.max_line_bytes
    }

    /// The next line without its newline, `None` at end of input.
    ///
    /// The slice is valid until the next call.
    ///
    /// # Errors
    ///
    /// [`FrameError::TooLong`] when the line exceeds the cap (nothing past the
    /// cap was copied), [`FrameError::Io`] when the underlying reader fails.
    /// Either is returned again by every later call.
    pub fn read_line(&mut self) -> Result<Option<&[u8]>, FrameError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        match self.fill_line() {
            Ok(true) => Ok(Some(&self.line)),
            Ok(false) => Ok(None),
            Err(error) => {
                self.failed = Some(error.clone());
                Err(error)
            }
        }
    }

    /// The next line parsed as a message, `None` at end of input.
    ///
    /// # Errors
    ///
    /// [`ReceiveError::Frame`] when the line could not be read: the input could
    /// not be read as a request, so it is answered once with an id-less error
    /// response, and the connection is closed (when the transport itself failed,
    /// the answer cannot be written either, and closing is all there is).
    /// [`ReceiveError::Message`] when it was read and is not a valid message: an
    /// error carrying an id answers that request, one with no id is answered once
    /// with an id-less error response, and reading goes on. The module
    /// documentation, "Errors", states this rule, and that an error raised while
    /// decoding a notification is never answered.
    pub fn read_message(&mut self) -> Result<Option<Message>, ReceiveError> {
        match self.read_line() {
            Ok(Some(line)) => parse_message(line).map(Some).map_err(ReceiveError::Message),
            Ok(None) => Ok(None),
            Err(error) => Err(ReceiveError::Frame(error)),
        }
    }

    /// Assembles one line into `self.line`; `Ok(false)` at end of input.
    fn fill_line(&mut self) -> Result<bool, FrameError> {
        self.line.clear();
        loop {
            let available = match self.inner.fill_buf() {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(FrameError::from(error)),
            };
            if available.is_empty() {
                return Ok(!self.line.is_empty());
            }
            let newline = available.iter().position(|&byte| byte == b'\n');
            let take = newline.unwrap_or(available.len());
            // `self.line.len() <= self.max_line_bytes` always holds, so the
            // subtraction cannot underflow, and nothing is copied or consumed
            // for a line that cannot fit.
            if take > self.max_line_bytes - self.line.len() {
                return Err(FrameError::TooLong {
                    limit: self.max_line_bytes,
                });
            }
            let needed = self.line.len() + take;
            if needed > self.line.capacity() {
                // Grow geometrically but never past the cap.
                let target = needed
                    .max(self.line.capacity().saturating_mul(2))
                    .max(256)
                    .min(self.max_line_bytes);
                self.line.reserve_exact(target - self.line.len());
            }
            self.line.extend_from_slice(&available[..take]);
            self.inner.consume(take + usize::from(newline.is_some()));
            if newline.is_some() {
                return Ok(true);
            }
        }
    }
}

/// Writes one line, refusing one that contains a newline.
///
/// `line` must not contain `\n` or `\r`: the transport's "MUST NOT contain
/// embedded newlines" is enforced on the bytes themselves, not trusted to the
/// serializer. The line and its terminating newline go out in one `write_all`,
/// then the writer is flushed.
///
/// # Errors
///
/// [`FrameError::EmbeddedNewline`] (nothing is written) or [`FrameError::Io`].
pub fn write_line<W: Write>(writer: &mut W, line: &[u8]) -> Result<(), FrameError> {
    if line.iter().any(|&byte| byte == b'\n' || byte == b'\r') {
        return Err(FrameError::EmbeddedNewline);
    }
    let mut framed = Vec::with_capacity(line.len() + 1);
    framed.extend_from_slice(line);
    framed.push(b'\n');
    writer.write_all(&framed)?;
    writer.flush()?;
    Ok(())
}

/// Serializes one message to its single compact line, without the terminator.
///
/// `serde_json`'s compact serializer escapes every control character inside a
/// string, so a newline in a message's text becomes `\n` on the wire; the
/// check for a raw newline is a second line of defence.
///
/// # Errors
///
/// [`FrameError::Serialize`] or [`FrameError::EmbeddedNewline`].
pub fn serialize_message(message: &Message) -> Result<Vec<u8>, FrameError> {
    let bytes = serde_json::to_vec(message).map_err(|error| FrameError::Serialize {
        message: detail(&error),
    })?;
    if bytes.iter().any(|&byte| byte == b'\n' || byte == b'\r') {
        return Err(FrameError::EmbeddedNewline);
    }
    Ok(bytes)
}

/// Serializes one message and writes it as one line.
///
/// # Errors
///
/// As [`serialize_message`] and [`write_line`].
pub fn write_message<W: Write>(writer: &mut W, message: &Message) -> Result<(), FrameError> {
    let bytes = serialize_message(message)?;
    write_line(writer, &bytes)
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 messages
// ---------------------------------------------------------------------------

/// A request id: a string or an integer, never `null`.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// An integer id.
    Number(i64),
    /// A string id.
    String(String),
}

impl From<i64> for RequestId {
    fn from(id: i64) -> Self {
        Self::Number(id)
    }
}

impl From<&str> for RequestId {
    fn from(id: &str) -> Self {
        Self::String(id.to_owned())
    }
}

impl From<String> for RequestId {
    fn from(id: String) -> Self {
        Self::String(id)
    }
}

/// The object inside an error response: `code`, `message`, optional `data`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ErrorObject {
    /// The error type; an integer.
    pub code: i64,
    /// A short description.
    pub message: String,
    /// Detail defined by the sender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A request: it expects a response with the same id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// The id to answer with.
    pub id: RequestId,
    /// The method name.
    pub method: String,
    /// The parameters: an object when present.
    pub params: Option<Value>,
}

/// A notification: one way, never answered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notification {
    /// The method name.
    pub method: String,
    /// The parameters: an object when present.
    pub params: Option<Value>,
}

/// A successful response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultResponse {
    /// The id of the request answered.
    pub id: RequestId,
    /// The result: an object.
    pub result: Value,
}

/// An error response. The id is absent when the request's could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorResponse {
    /// The id of the request answered, when known.
    pub id: Option<RequestId>,
    /// What went wrong.
    pub error: ErrorObject,
}

/// Any one JSON-RPC message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    /// A request.
    Request(Request),
    /// A notification.
    Notification(Notification),
    /// A successful response.
    ResultResponse(ResultResponse),
    /// An error response.
    ErrorResponse(ErrorResponse),
}

/// `value` as a JSON object, or an internal error naming `what`.
fn to_object<T: Serialize + ?Sized>(value: &T, what: &'static str) -> Result<Value, ProtocolError> {
    match serde_json::to_value(value) {
        Ok(object @ Value::Object(_)) => Ok(object),
        Ok(_) => Err(ProtocolError::new(
            ErrorCode::InternalError,
            format!("{what} is not a JSON object"),
        )),
        Err(error) => Err(ProtocolError::new(
            ErrorCode::InternalError,
            format!("{what} could not be serialized"),
        )
        .with_data(json!({ "detail": detail(&error) }))),
    }
}

/// Deserializes `params` (absent means an empty object) into `T` from a borrow.
///
/// This copies every string, map and nested value out of the tree; it is for a
/// message this side built. A peer's message goes through [`params_moved`].
fn params_copied<T: DeserializeOwned>(params: Option<&Value>) -> Result<T, serde_json::Error> {
    match params {
        Some(value) => T::deserialize(value),
        None => T::deserialize(&Value::Object(Map::new())),
    }
}

/// Deserializes `params` (absent means an empty object) into `T` by value:
/// `serde_json::from_value` moves each string and key out of the tree instead of
/// copying it. See the module documentation, "Framing", on memory.
fn params_moved<T: DeserializeOwned>(params: Option<Value>) -> Result<T, serde_json::Error> {
    serde_json::from_value(params.unwrap_or_else(|| Value::Object(Map::new())))
}

/// The -32602 error for parameters that do not fit, carrying the request's id
/// when there is one and an echo of the method and the decoder's description.
fn invalid_params(id: Option<RequestId>, method: &str, error: &serde_json::Error) -> ProtocolError {
    ProtocolError::new(ErrorCode::InvalidParams, "invalid params")
        .with_id(id)
        .with_data(json!({ "method": echo(method), "detail": detail(error) }))
}

/// The -32601 error for a method this side does not serve in this direction,
/// carrying the request's id and an echo of the method.
fn method_not_found(id: RequestId, method: &str) -> ProtocolError {
    ProtocolError::new(ErrorCode::MethodNotFound, "method not found")
        .with_id(Some(id))
        .with_data(json!({ "method": echo(method) }))
}

/// Decodes a request's parameters by value, handing the id back with them so a
/// caller that consumed the request still has it, and moving it into the error
/// when the parameters do not fit.
fn decode_params<T: DeserializeOwned>(
    id: RequestId,
    method: &str,
    params: Option<Value>,
) -> Result<(RequestId, T), ProtocolError> {
    match params_moved(params) {
        Ok(decoded) => Ok((id, decoded)),
        Err(error) => Err(invalid_params(Some(id), method, &error)),
    }
}

impl Request {
    /// A request with no parameters.
    #[must_use]
    pub fn without_params(id: RequestId, method: impl Into<String>) -> Self {
        Self {
            id,
            method: method.into(),
            params: None,
        }
    }

    /// A request whose parameters are `params` serialized.
    ///
    /// # Errors
    ///
    /// An internal error when `params` does not serialize to a JSON object.
    pub fn with_params<P: Serialize + ?Sized>(
        id: RequestId,
        method: impl Into<String>,
        params: &P,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            id,
            method: method.into(),
            params: Some(to_object(params, "params")?),
        })
    }

    /// The parameters as a `T`, from a borrow; absent parameters read as an
    /// empty object.
    ///
    /// This copies the tree: every string, map and nested value is copied out of
    /// it. On a message a peer sent, use [`Request::into_params`], which moves
    /// them instead (module documentation, "Framing").
    ///
    /// # Errors
    ///
    /// -32602, carrying this request's id, when the parameters do not fit `T`.
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, ProtocolError> {
        params_copied(self.params.as_ref())
            .map_err(|error| invalid_params(Some(self.id.clone()), &self.method, &error))
    }

    /// The parameters as a `T`, consuming the request; absent parameters read
    /// as an empty object. The form to use on a peer's message: strings and maps
    /// are moved out of the tree, not copied.
    ///
    /// # Errors
    ///
    /// -32602, carrying this request's id and the echoed method, when the
    /// parameters do not fit `T`; the same error as [`Request::params_as`].
    pub fn into_params<T: DeserializeOwned>(self) -> Result<T, ProtocolError> {
        let Self { id, method, params } = self;
        decode_params(id, &method, params).map(|(_, decoded)| decoded)
    }
}

impl Notification {
    /// A notification with no parameters.
    #[must_use]
    pub fn without_params(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            params: None,
        }
    }

    /// A notification whose parameters are `params` serialized.
    ///
    /// # Errors
    ///
    /// An internal error when `params` does not serialize to a JSON object.
    pub fn with_params<P: Serialize + ?Sized>(
        method: impl Into<String>,
        params: &P,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            method: method.into(),
            params: Some(to_object(params, "params")?),
        })
    }

    /// The parameters as a `T`, from a borrow; absent parameters read as an
    /// empty object.
    ///
    /// This copies the tree: every string, map and nested value is copied out of
    /// it. On a message a peer sent, use [`Notification::into_params`], which
    /// moves them instead (module documentation, "Framing").
    ///
    /// # Errors
    ///
    /// -32602 (with no id: a notification has none, and by the rule in the
    /// module documentation, "Errors", it is never answered) when they do not
    /// fit `T`.
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, ProtocolError> {
        params_copied(self.params.as_ref())
            .map_err(|error| invalid_params(None, &self.method, &error))
    }

    /// The parameters as a `T`, consuming the notification; absent parameters
    /// read as an empty object. The form to use on a peer's message: strings and
    /// maps are moved out of the tree, not copied.
    ///
    /// # Errors
    ///
    /// -32602 with no id, as [`Notification::params_as`], when they do not fit
    /// `T`.
    pub fn into_params<T: DeserializeOwned>(self) -> Result<T, ProtocolError> {
        let Self { method, params } = self;
        params_moved(params).map_err(|error| invalid_params(None, &method, &error))
    }
}

impl ResultResponse {
    /// A response with an empty result object, the answer to `ping`.
    #[must_use]
    pub fn empty(id: RequestId) -> Self {
        Self {
            id,
            result: Value::Object(Map::new()),
        }
    }

    /// A response whose result is `result` serialized.
    ///
    /// # Errors
    ///
    /// An internal error when `result` does not serialize to a JSON object.
    pub fn with_result<T: Serialize + ?Sized>(
        id: RequestId,
        result: &T,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            id,
            result: to_object(result, "result")?,
        })
    }

    /// The result as a `T`, from a borrow, for the side that made the request.
    ///
    /// This copies the tree: every string, map and nested value is copied out of
    /// it, and a tool result is the large case. On a peer's response, use
    /// [`ResultResponse::into_result`], which moves them instead (module
    /// documentation, "Framing").
    ///
    /// The code is -32600, the nearest standard one for "this response is not
    /// a valid response to that method"; it is never sent, the caller maps it
    /// to its own error.
    ///
    /// # Errors
    ///
    /// -32600 when the result does not fit `T`.
    pub fn result_as<T: DeserializeOwned>(&self) -> Result<T, ProtocolError> {
        T::deserialize(&self.result).map_err(|error| invalid_result(&error))
    }

    /// The result as a `T`, consuming the response, for the side that made the
    /// request. The form to use on a peer's response: strings and maps are moved
    /// out of the tree, not copied.
    ///
    /// # Errors
    ///
    /// -32600 when the result does not fit `T`; the same error as
    /// [`ResultResponse::result_as`], and never sent.
    pub fn into_result<T: DeserializeOwned>(self) -> Result<T, ProtocolError> {
        serde_json::from_value(self.result).map_err(|error| invalid_result(&error))
    }
}

/// The -32600 error for a result that does not fit its type, with the decoder's
/// description as `detail`.
fn invalid_result(error: &serde_json::Error) -> ProtocolError {
    invalid_request("invalid result").with_data(json!({ "detail": detail(error) }))
}

impl ErrorResponse {
    /// An error response for `error`.
    #[must_use]
    pub fn from_protocol_error(error: &ProtocolError) -> Self {
        error.to_error_response()
    }
}

impl From<Request> for Message {
    fn from(request: Request) -> Self {
        Self::Request(request)
    }
}

impl From<Notification> for Message {
    fn from(notification: Notification) -> Self {
        Self::Notification(notification)
    }
}

impl From<ResultResponse> for Message {
    fn from(response: ResultResponse) -> Self {
        Self::ResultResponse(response)
    }
}

impl From<ErrorResponse> for Message {
    fn from(response: ErrorResponse) -> Self {
        Self::ErrorResponse(response)
    }
}

#[derive(Serialize)]
struct RequestWire<'a> {
    jsonrpc: &'static str,
    id: &'a RequestId,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a Value>,
}

#[derive(Serialize)]
struct NotificationWire<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a Value>,
}

#[derive(Serialize)]
struct ResultWire<'a> {
    jsonrpc: &'static str,
    id: &'a RequestId,
    result: &'a Value,
}

#[derive(Serialize)]
struct ErrorWire<'a> {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a RequestId>,
    error: &'a ErrorObject,
}

impl Serialize for Request {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RequestWire {
            jsonrpc: JSONRPC_VERSION,
            id: &self.id,
            method: &self.method,
            params: self.params.as_ref(),
        }
        .serialize(serializer)
    }
}

impl Serialize for Notification {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        NotificationWire {
            jsonrpc: JSONRPC_VERSION,
            method: &self.method,
            params: self.params.as_ref(),
        }
        .serialize(serializer)
    }
}

impl Serialize for ResultResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ResultWire {
            jsonrpc: JSONRPC_VERSION,
            id: &self.id,
            result: &self.result,
        }
        .serialize(serializer)
    }
}

impl Serialize for ErrorResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ErrorWire {
            jsonrpc: JSONRPC_VERSION,
            id: self.id.as_ref(),
            error: &self.error,
        }
        .serialize(serializer)
    }
}

impl Serialize for Message {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Request(message) => message.serialize(serializer),
            Self::Notification(message) => message.serialize(serializer),
            Self::ResultResponse(message) => message.serialize(serializer),
            Self::ErrorResponse(message) => message.serialize(serializer),
        }
    }
}

/// Parses one line as one message.
///
/// # Errors
///
/// -32700 when the bytes are not JSON (including invalid UTF-8, trailing
/// characters and nesting past `serde_json`'s limit); -32600 when the JSON is
/// not a valid message, including every batch. See [`Message::from_value`].
pub fn parse_message(line: &[u8]) -> Result<Message, ProtocolError> {
    let value: Value = serde_json::from_slice(line).map_err(|error| {
        ProtocolError::new(ErrorCode::ParseError, "parse error")
            .with_data(json!({ "detail": detail(&error) }))
    })?;
    Message::from_value(value)
}

impl Message {
    /// Classifies a JSON value as a request, a notification or a response.
    ///
    /// A request or notification has a string `method`; a request also has an
    /// id (string or integer, not `null`), a notification none. A response has
    /// no `method` and exactly one of `result` (an object, with an id) or
    /// `error` (an object with an integer `code` and a string `message`, id
    /// optional). `jsonrpc` must be `"2.0"`.
    ///
    /// # Errors
    ///
    /// -32600 for anything else, carrying the id when the value had the shape
    /// of a request and its id was readable. A JSON array is a batch, which
    /// this side does not support, and gets the same code.
    pub fn from_value(value: Value) -> Result<Self, ProtocolError> {
        let object = match value {
            Value::Object(object) => object,
            Value::Array(_) => return Err(invalid_request("batches are not supported")),
            _ => return Err(invalid_request("a message is a JSON object")),
        };
        let request_shaped = object.contains_key("method");
        let id_hint = if request_shaped {
            object
                .get("id")
                .and_then(|id| RequestId::deserialize(id).ok())
        } else {
            None
        };
        classify(object).map_err(|error| error.with_id(id_hint))
    }
}

/// The classification behind [`Message::from_value`].
fn classify(mut object: JsonObject) -> Result<Message, ProtocolError> {
    match object.get("jsonrpc") {
        Some(Value::String(version)) if version == JSONRPC_VERSION => {}
        _ => return Err(invalid_request("jsonrpc must be \"2.0\"")),
    }
    if let Some(method) = object.remove("method") {
        let Value::String(method) = method else {
            return Err(invalid_request("method must be a string"));
        };
        if object.contains_key("result") || object.contains_key("error") {
            return Err(invalid_request(
                "a request or notification has no result or error",
            ));
        }
        let params = match object.remove("params") {
            None => None,
            Some(params @ Value::Object(_)) => Some(params),
            Some(_) => return Err(invalid_request("params must be an object")),
        };
        return match object.remove("id") {
            None => Ok(Message::Notification(Notification { method, params })),
            Some(Value::Null) => Err(invalid_request("a request id must not be null")),
            Some(id) => match RequestId::deserialize(&id) {
                Ok(id) => Ok(Message::Request(Request { id, method, params })),
                Err(_) => Err(invalid_request("a request id is a string or an integer")),
            },
        };
    }
    match (object.remove("result"), object.remove("error")) {
        (Some(result @ Value::Object(_)), None) => match object.remove("id") {
            Some(id) if !id.is_null() => match RequestId::deserialize(&id) {
                Ok(id) => Ok(Message::ResultResponse(ResultResponse { id, result })),
                Err(_) => Err(invalid_request("a response id is a string or an integer")),
            },
            _ => Err(invalid_request("a result response needs an id")),
        },
        (Some(_), None) => Err(invalid_request("a result is an object")),
        (None, Some(error)) => {
            let Ok(error) = ErrorObject::deserialize(&error) else {
                return Err(invalid_request(
                    "an error is an object with an integer code and a string message",
                ));
            };
            let id = match object.remove("id") {
                None | Some(Value::Null) => None,
                Some(id) => match RequestId::deserialize(&id) {
                    Ok(id) => Some(id),
                    Err(_) => {
                        return Err(invalid_request("a response id is a string or an integer"));
                    }
                },
            };
            Ok(Message::ErrorResponse(ErrorResponse { id, error }))
        }
        _ => Err(invalid_request(
            "neither a request, a notification nor a response",
        )),
    }
}

// ---------------------------------------------------------------------------
// MCP messages
// ---------------------------------------------------------------------------

/// Name and version of a client or server: the schema's `Implementation`.
///
/// `icons` is not typed: it is dropped on read and never written.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// The programmatic name.
    pub name: String,
    /// A name for people.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The implementation's version.
    pub version: String,
    /// What it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Its website.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
}

/// What a client offers: the schema's `ClientCapabilities`. Each present
/// member is an object; unknown members are ignored, because only negotiated
/// capabilities may be used and an unknown one is never negotiated.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// Non-standard capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<JsonObject>,
    /// Filesystem roots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<JsonObject>,
    /// LLM sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<JsonObject>,
    /// Elicitation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<JsonObject>,
    /// Task-augmented requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<JsonObject>,
}

/// The `tools` capability: the schema's `{ listChanged?: boolean }`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    /// Whether the server sends `notifications/tools/list_changed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// What a server offers: the schema's `ServerCapabilities`. Only `tools` is
/// typed; the others are carried as objects. Unknown members are ignored.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Non-standard capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<JsonObject>,
    /// Log messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<JsonObject>,
    /// Argument completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<JsonObject>,
    /// Prompt templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<JsonObject>,
    /// Readable resources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<JsonObject>,
    /// Callable tools: present when the server serves `tools/list` and `tools/call`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    /// Task-augmented requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<JsonObject>,
}

/// The parameters of `initialize`: the schema's `InitializeRequestParams`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// The latest version the client supports.
    pub protocol_version: String,
    /// What the client offers.
    pub capabilities: ClientCapabilities,
    /// Who the client is.
    pub client_info: Implementation,
}

/// The result of `initialize`: the schema's `InitializeResult`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// The version the server will use, see [`negotiate_version`].
    pub protocol_version: String,
    /// What the server offers.
    pub capabilities: ServerCapabilities,
    /// Who the server is.
    pub server_info: Implementation,
    /// Hints on how to use the server, which a client may add to a prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// The parameters of `tools/list`: the schema's `PaginatedRequestParams`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ListToolsParams {
    /// An opaque position returned by an earlier page's `nextCursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Hints about a tool's behaviour: the schema's `ToolAnnotations`. A client
/// must treat these as untrusted unless the server is.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// A title for people.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The tool does not modify its environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// The tool may perform destructive updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// Repeating the call has no further effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// The tool interacts with an open world of external entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

/// A tool a server offers: the schema's `Tool`.
///
/// The name and the schema are not validated here (the name rules are
/// "SHOULD" and a tool whose `inputSchema` is not an object fails to read);
/// whether to admit a tool is the registry's decision. `icons` and `_meta` are
/// dropped on read and never written.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// The unique name.
    pub name: String,
    /// A name for people.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// What the tool does, as a hint to the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A JSON Schema object for the arguments.
    pub input_schema: JsonObject,
    /// A JSON Schema object for `structuredContent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<JsonObject>,
    /// Behaviour hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    /// Execution properties such as task support, carried as an object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<JsonObject>,
}

/// The result of `tools/list`: the schema's `ListToolsResult`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// This page of tools.
    pub tools: Vec<Tool>,
    /// Present when there may be more pages; pass it back as the cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The parameters of `tools/call`: the schema's `CallToolRequestParams`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallToolParams {
    /// The tool's name.
    pub name: String,
    /// The arguments, an object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<JsonObject>,
    /// A request for task-augmented execution, carried so a server that did
    /// not declare tasks can see it and refuse rather than silently ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<JsonObject>,
    /// The request's `_meta` (a progress token, for instance).
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<JsonObject>,
}

/// A text content block: the schema's `TextContent`. Its `annotations` and
/// `_meta` are dropped on read and never written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextContent {
    /// The text.
    pub text: String,
}

/// A content block of a type other than `text`, carried as the raw JSON object
/// it arrived as. See the module documentation, "The messages".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtherContent {
    kind: String,
    raw: JsonObject,
}

impl OtherContent {
    /// The block's `type` tag (`image`, `audio`, `resource_link`, `resource`,
    /// or any later one).
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The block exactly as received.
    #[must_use]
    pub const fn raw(&self) -> &JsonObject {
        &self.raw
    }
}

/// One element of a tool result's `content`: the schema's `ContentBlock`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentBlock {
    /// A text block, typed.
    Text(TextContent),
    /// Any other block, raw.
    Other(OtherContent),
}

impl ContentBlock {
    /// A text block.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(TextContent { text: text.into() })
    }

    /// The block's `type` tag.
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Self::Text(_) => "text",
            Self::Other(other) => other.kind(),
        }
    }

    /// The text, when this is a text block.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(&text.text),
            Self::Other(_) => None,
        }
    }

    /// Reads a block from JSON.
    ///
    /// # Errors
    ///
    /// A fixed description when the value is not an object with a string
    /// `type`, or is a `text` block without a string `text`.
    pub fn from_value(value: Value) -> Result<Self, &'static str> {
        let Value::Object(mut object) = value else {
            return Err("a content block is an object");
        };
        let Some(Value::String(kind)) = object.get("type") else {
            return Err("a content block has a string type");
        };
        if kind == "text" {
            // The text is moved out of the object, not copied: it is the large
            // part of a tool result (module documentation, "Framing").
            return match object.remove("text") {
                Some(Value::String(text)) => Ok(Self::Text(TextContent { text })),
                _ => Err("a text block has a string text"),
            };
        }
        let kind = kind.clone();
        Ok(Self::Other(OtherContent { kind, raw: object }))
    }
}

impl Serialize for ContentBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct TextWire<'a> {
            #[serde(rename = "type")]
            kind: &'static str,
            text: &'a str,
        }
        match self {
            Self::Text(text) => TextWire {
                kind: "text",
                text: &text.text,
            }
            .serialize(serializer),
            Self::Other(other) => other.raw.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::from_value(value).map_err(de::Error::custom)
    }
}

/// The result of `tools/call`: the schema's `CallToolResult`.
///
/// A failure of the tool itself is a result with `isError` true, which the
/// model can read; a failure to find or run the call is a JSON-RPC error
/// response instead (the tools page, "Error Handling").
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// The unstructured result.
    pub content: Vec<ContentBlock>,
    /// A structured result, an object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<JsonObject>,
    /// Whether the call ended in an error; absent means it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl CallToolResult {
    /// A successful result of one text block.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::text(text)],
            structured_content: None,
            is_error: None,
        }
    }

    /// A tool-level error result of one text block.
    #[must_use]
    pub fn error_text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::text(text)],
            structured_content: None,
            is_error: Some(true),
        }
    }

    /// Whether the call ended in an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.is_error.unwrap_or(false)
    }
}

/// The parameters of `notifications/cancelled`: the schema's
/// `CancelledNotificationParams`.
///
/// A client must not cancel its own `initialize`; this type cannot tell which
/// request an id names, so that rule is for the caller that tracks in-flight
/// requests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledParams {
    /// The request to cancel; the schema makes it optional only for tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<RequestId>,
    /// Why, for a log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A request this wire knows, decoded: `initialize`, `ping`, `tools/list`,
/// `tools/call`. Which of them a side may receive depends on the direction the
/// request travelled, see [`KnownRequest::decode`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnownRequest {
    /// `initialize`.
    Initialize(InitializeParams),
    /// `ping`.
    Ping,
    /// `tools/list`.
    ToolsList(ListToolsParams),
    /// `tools/call`.
    ToolsCall(CallToolParams),
}

impl KnownRequest {
    /// Decodes a request that travelled in `direction`, by its method, and gives
    /// its id back with it (the request is consumed, and its parameters are moved
    /// out of the tree, not copied: module documentation, "Framing").
    ///
    /// Each direction has its own method set (module documentation, "The
    /// messages"):
    ///
    /// - [`Direction::ClientToServer`], what a server receives: `initialize`,
    ///   `ping`, `tools/list` and `tools/call`.
    /// - [`Direction::ServerToClient`], what a client receives: `ping` only.
    ///   Phase 1's client offers no sampling, roots or elicitation (the
    ///   `sampling`, `roots` and `elicitation` members of `ClientCapabilities`
    ///   in `schema-2025-11-25.ts`), nor tasks (its `tasks` member), so a server
    ///   has no request to send it but a ping. `initialize`, `tools/list` and
    ///   `tools/call` from a server are "method not found", exactly as an unknown
    ///   method is.
    ///
    /// Whether the connection's state allows the method is
    /// [`ConnectionState::check`]'s question, asked before this one.
    ///
    /// # Errors
    ///
    /// -32601 for any other method in that direction (decision D2: everything
    /// else is "method not found"), carrying the id and the echoed method, and
    /// -32602 for parameters that do not fit. Both carry the request's id, so by
    /// the rule in the module documentation, "Errors", each answers that request.
    pub fn decode(
        direction: Direction,
        request: Request,
    ) -> Result<(RequestId, Self), ProtocolError> {
        let Request { id, method, params } = request;
        match (direction, method.as_str()) {
            (_, METHOD_PING) => Ok((id, Self::Ping)),
            (Direction::ClientToServer, METHOD_INITIALIZE) => {
                decode_params(id, &method, params).map(|(id, p)| (id, Self::Initialize(p)))
            }
            (Direction::ClientToServer, METHOD_TOOLS_LIST) => {
                decode_params(id, &method, params).map(|(id, p)| (id, Self::ToolsList(p)))
            }
            (Direction::ClientToServer, METHOD_TOOLS_CALL) => {
                decode_params(id, &method, params).map(|(id, p)| (id, Self::ToolsCall(p)))
            }
            _ => Err(method_not_found(id, &method)),
        }
    }
}

/// A notification this wire knows, decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnownNotification {
    /// `notifications/initialized`.
    Initialized,
    /// `notifications/cancelled`.
    Cancelled(CancelledParams),
    /// `notifications/tools/list_changed`.
    ToolsListChanged,
}

impl KnownNotification {
    /// Decodes a notification that travelled in `direction`, by its method,
    /// consuming it (its parameters are moved out of the tree, not copied:
    /// module documentation, "Framing"). `Ok(None)` for one this wire does not
    /// know, which the receiver ignores.
    ///
    /// Each direction has its own set:
    ///
    /// - [`Direction::ClientToServer`]: `notifications/initialized` and
    ///   `notifications/cancelled`.
    /// - [`Direction::ServerToClient`]: `notifications/cancelled` and
    ///   `notifications/tools/list_changed`.
    ///
    /// A known notification travelling the wrong way (a client's
    /// `notifications/tools/list_changed`, a server's
    /// `notifications/initialized`) is `Ok(None)`, ignored like an unknown one:
    /// nothing answers a notification, so there is no one to refuse it to.
    ///
    /// # Errors
    ///
    /// -32602, with no id, for parameters that do not fit. By the rule in the
    /// module documentation, "Errors", an error raised while decoding a
    /// notification is never answered.
    pub fn decode(
        direction: Direction,
        notification: Notification,
    ) -> Result<Option<Self>, ProtocolError> {
        match (direction, notification.method.as_str()) {
            (Direction::ClientToServer, NOTIFICATION_INITIALIZED) => Ok(Some(Self::Initialized)),
            (_, NOTIFICATION_CANCELLED) => notification
                .into_params()
                .map(|params| Some(Self::Cancelled(params))),
            (Direction::ServerToClient, NOTIFICATION_TOOLS_LIST_CHANGED) => {
                Ok(Some(Self::ToolsListChanged))
            }
            _ => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Version negotiation
// ---------------------------------------------------------------------------

/// Whether `version` has the `YYYY-MM-DD` shape the specification's version
/// strings have. The calendar is not checked: `2025-13-45` has the shape.
fn is_version_shaped(version: &str) -> bool {
    let bytes = version.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, &byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn unsupported_version(requested: &str, supported: &[&str]) -> ProtocolError {
    ProtocolError::new(ErrorCode::InvalidParams, "unsupported protocol version")
        .with_data(json!({ "supported": supported, "requested": echo(requested) }))
}

/// The version a server answers an `initialize` with: the lifecycle page's
/// "If the server supports the requested protocol version, it MUST respond with
/// the same version. Otherwise, the server MUST respond with another protocol
/// version it supports. This SHOULD be the latest version supported by the
/// server."
///
/// `supported` holds this side's versions; the latest is the greatest, which for
/// `YYYY-MM-DD` strings is the lexicographic maximum. A requested version that
/// is not even version-shaped (`1.0.0`, as in the page's own error example,
/// `latest`, the empty string) is not "a version the client supports" and gets
/// -32602 with `{supported, requested}` as `data`, the page's example; a
/// well-formed version this side does not support is answered with the latest
/// one, and the client decides whether to go on ([`check_server_version`]).
///
/// # Errors
///
/// -32602 for a malformed `requested`; -32603 when `supported` is empty or
/// holds a malformed entry (a defect of the caller, never the peer's).
///
/// This function does not see the request, so the error carries no id: the
/// caller attaches the `initialize` request's id with [`ProtocolError::with_id`]
/// before answering (an error carrying an id answers that request, module
/// documentation, "Errors"), as in
/// `negotiate_version(..).map_err(|error| error.with_id(Some(id)))`.
pub fn negotiate_version<'a>(
    requested: &str,
    supported: &[&'a str],
) -> Result<&'a str, ProtocolError> {
    if supported.is_empty() || supported.iter().any(|version| !is_version_shaped(version)) {
        return Err(ProtocolError::new(
            ErrorCode::InternalError,
            "no usable supported protocol version",
        ));
    }
    if !is_version_shaped(requested) {
        return Err(unsupported_version(requested, supported));
    }
    if let Some(same) = supported.iter().find(|version| **version == requested) {
        return Ok(same);
    }
    supported.iter().copied().max().ok_or_else(|| {
        ProtocolError::new(
            ErrorCode::InternalError,
            "no usable supported protocol version",
        )
    })
}

/// The client's side of negotiation: whether the version a server answered with
/// is one this client supports. The lifecycle page says "If the client does not
/// support the version in the server's response, it SHOULD disconnect", but the
/// schema wins over the prose, and it reads "If the client cannot support this
/// version, it MUST disconnect" (`schema-2025-11-25.ts`,
/// `InitializeResult.protocolVersion`): an `Err` here is a must-disconnect.
///
/// # Errors
///
/// -32602 with `{supported, requested}` (the answered version, under the page's
/// key) when it is not in `supported`. The error is never sent; the caller
/// closes the connection.
pub fn check_server_version(answered: &str, supported: &[&str]) -> Result<(), ProtocolError> {
    if supported.contains(&answered) {
        Ok(())
    } else {
        Err(unsupported_version(answered, supported))
    }
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

/// Which way a message travels.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Direction {
    /// From the client to the server; the server receives it.
    ClientToServer,
    /// From the server to the client; the client receives it.
    ServerToClient,
}

/// A message sorted by what the lifecycle page distinguishes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MessageKind {
    /// The `initialize` request.
    Initialize,
    /// The `ping` request.
    Ping,
    /// The `tools/list` request.
    ToolsList,
    /// The `tools/call` request.
    ToolsCall,
    /// Any other request.
    OtherRequest,
    /// The `notifications/initialized` notification.
    Initialized,
    /// The `notifications/cancelled` notification.
    Cancelled,
    /// The `notifications/message` (logging) notification.
    LoggingMessage,
    /// Any other notification.
    OtherNotification,
    /// A result or error response.
    Response,
}

impl MessageKind {
    /// Every kind, for exhaustive checks.
    pub const ALL: [Self; 10] = [
        Self::Initialize,
        Self::Ping,
        Self::ToolsList,
        Self::ToolsCall,
        Self::OtherRequest,
        Self::Initialized,
        Self::Cancelled,
        Self::LoggingMessage,
        Self::OtherNotification,
        Self::Response,
    ];

    /// The kind of a request by its method name.
    #[must_use]
    pub fn of_request(method: &str) -> Self {
        match method {
            METHOD_INITIALIZE => Self::Initialize,
            METHOD_PING => Self::Ping,
            METHOD_TOOLS_LIST => Self::ToolsList,
            METHOD_TOOLS_CALL => Self::ToolsCall,
            _ => Self::OtherRequest,
        }
    }

    /// The kind of a notification by its method name.
    #[must_use]
    pub fn of_notification(method: &str) -> Self {
        match method {
            NOTIFICATION_INITIALIZED => Self::Initialized,
            NOTIFICATION_CANCELLED => Self::Cancelled,
            NOTIFICATION_MESSAGE => Self::LoggingMessage,
            _ => Self::OtherNotification,
        }
    }

    /// The kind of a message.
    #[must_use]
    pub fn of(message: &Message) -> Self {
        match message {
            Message::Request(request) => Self::of_request(&request.method),
            Message::Notification(notification) => Self::of_notification(&notification.method),
            Message::ResultResponse(_) | Message::ErrorResponse(_) => Self::Response,
        }
    }
}

/// A fact that moves a [`ConnectionState`] forward.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Transition {
    /// The `initialize` request was answered with a result.
    InitializeAnswered,
    /// `notifications/initialized` was sent (by the client) or received (by the
    /// server).
    InitializedNotified,
}

/// The lifecycle page's phases as a value: the handshake that opens the tool
/// connection AICD §8 requires.
///
/// - `Fresh`: `initialize` has not been answered with a result. A refused or
///   failed `initialize` leaves the connection here.
/// - `Initialized`: it has, and `notifications/initialized` has not yet been
///   sent or received.
/// - `Ready`: the operation phase.
///
/// Both ends keep one, from their own sends and receives; the table in
/// [`ConnectionState::permits`] is the same for both. The type decides nothing
/// by itself: the server core enforces it.
///
/// An end that writes from another thread than the one that reads advances its
/// own state to `Ready` (`advance(Transition::InitializedNotified)`) **before**
/// it writes `notifications/initialized`. Otherwise the server's first request
/// can arrive while the client's state still reads `Initialized`, and be
/// refused by [`ConnectionState::check`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConnectionState {
    /// Nothing has been agreed.
    Fresh,
    /// The server has answered `initialize`.
    Initialized,
    /// The client has sent `notifications/initialized`.
    Ready,
}

impl ConnectionState {
    /// The state's name, as it appears in an error's `data`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Initialized => "initialized",
            Self::Ready => "ready",
        }
    }

    /// Whether a message of `kind` travelling in `direction` is allowed in this
    /// state, per the lifecycle page.
    ///
    /// | State | client to server | server to client |
    /// |---|---|---|
    /// | `Fresh` | `initialize`, `ping`, responses | `ping`, logging, responses |
    /// | `Initialized` | `ping`, `notifications/initialized`, `notifications/cancelled`, responses | `ping`, logging, responses |
    /// | `Ready` | everything but a second `initialize` or `initialized` | the same |
    ///
    /// A response is allowed everywhere: it answers a request that was itself
    /// checked when it was sent, and matching it to that request is the
    /// caller's table of in-flight ids. The table is about phases and knows
    /// kinds, not methods or who may receive them: in `Ready` a request of an
    /// unknown method, or of a method the receiving side does not serve, is
    /// allowed here, and [`KnownRequest::decode`], which takes the direction,
    /// answers it "method not found".
    ///
    /// `notifications/cancelled` in `Fresh` is refused, and the reason is not
    /// that nothing else can be in flight, because a ping can be. In `Fresh` the
    /// only requests in flight are `initialize`, whose cancellation the
    /// lifecycle forbids ("MUST NOT"), and pings, which are not worth an
    /// exception. A refused notification is dropped, never answered, so the
    /// refusal costs nothing.
    #[must_use]
    pub const fn permits(self, direction: Direction, kind: MessageKind) -> bool {
        if matches!(kind, MessageKind::Response) {
            return true;
        }
        match (self, direction) {
            (Self::Fresh, Direction::ClientToServer) => {
                matches!(kind, MessageKind::Initialize | MessageKind::Ping)
            }
            (Self::Initialized, Direction::ClientToServer) => matches!(
                kind,
                MessageKind::Ping | MessageKind::Initialized | MessageKind::Cancelled
            ),
            (Self::Fresh | Self::Initialized, Direction::ServerToClient) => {
                matches!(kind, MessageKind::Ping | MessageKind::LoggingMessage)
            }
            (Self::Ready, _) => !matches!(kind, MessageKind::Initialize | MessageKind::Initialized),
        }
    }

    /// Checks a message against the table.
    ///
    /// # Errors
    ///
    /// -32600 with `{state}` as data. By the rule in the module documentation,
    /// "Errors": the error carries the request's id when the message is a
    /// request, and that answers the request; for a notification the error has
    /// no id and the caller drops the message, because nothing answers a
    /// notification.
    pub fn check(self, direction: Direction, message: &Message) -> Result<(), ProtocolError> {
        if self.permits(direction, MessageKind::of(message)) {
            return Ok(());
        }
        let id = match message {
            Message::Request(request) => Some(request.id.clone()),
            _ => None,
        };
        Err(
            invalid_request("message not allowed in this connection state")
                .with_id(id)
                .with_data(json!({ "state": self.name() })),
        )
    }

    /// The state after `transition`.
    ///
    /// # Errors
    ///
    /// -32600 when the transition does not follow from this state
    /// (`InitializeAnswered` is valid only in `Fresh`, `InitializedNotified`
    /// only in `Initialized`).
    pub fn advance(self, transition: Transition) -> Result<Self, ProtocolError> {
        match (self, transition) {
            (Self::Fresh, Transition::InitializeAnswered) => Ok(Self::Initialized),
            (Self::Initialized, Transition::InitializedNotified) => Ok(Self::Ready),
            _ => Err(
                invalid_request("transition does not follow from this state")
                    .with_data(json!({ "state": self.name() })),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::io::{BufReader, Cursor, Read};

    use super::*;

    // ----- helpers ---------------------------------------------------------

    fn wire(message: &Message) -> Vec<u8> {
        let mut out = Vec::new();
        write_message(&mut out, message).expect("write");
        out
    }

    fn read_back(bytes: &[u8]) -> Message {
        let mut reader = LineReader::new(bytes);
        let message = reader.read_message().expect("read").expect("a message");
        assert_eq!(reader.read_message(), Ok(None), "exactly one line");
        message
    }

    fn round_trip(message: &Message) -> Message {
        let bytes = wire(message);
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(
            bytes.iter().filter(|&&b| b == b'\n').count(),
            1,
            "one line, one newline"
        );
        read_back(&bytes)
    }

    fn id(n: i64) -> RequestId {
        RequestId::Number(n)
    }

    fn parse(text: &str) -> Result<Message, ProtocolError> {
        parse_message(text.as_bytes())
    }

    fn client_info() -> Implementation {
        Implementation {
            name: "ExampleClient".to_owned(),
            title: Some("Example Client Display Name".to_owned()),
            version: "1.0.0".to_owned(),
            description: None,
            website_url: None,
        }
    }

    fn server_info() -> Implementation {
        Implementation {
            name: "ori-mcp".to_owned(),
            title: None,
            version: "0.1.0".to_owned(),
            description: None,
            website_url: None,
        }
    }

    // ----- round trips, one per message type -------------------------------

    #[test]
    fn ori_t_0158_round_trip_request_with_and_without_params() {
        let bare = Message::from(Request::without_params(id(1), METHOD_PING));
        assert_eq!(round_trip(&bare), bare);
        let with = Message::from(
            Request::with_params(
                RequestId::from("abc"),
                METHOD_TOOLS_CALL,
                &json!({"name": "t", "arguments": {"a": 1}}),
            )
            .expect("object params"),
        );
        assert_eq!(round_trip(&with), with);
    }

    #[test]
    fn ori_t_0158_round_trip_notification() {
        let bare = Message::from(Notification::without_params(NOTIFICATION_INITIALIZED));
        assert_eq!(round_trip(&bare), bare);
        let with = Message::from(
            Notification::with_params(
                NOTIFICATION_CANCELLED,
                &CancelledParams {
                    request_id: Some(id(5)),
                    reason: Some("user".to_owned()),
                },
            )
            .expect("params"),
        );
        assert_eq!(round_trip(&with), with);
    }

    #[test]
    fn ori_t_0158_round_trip_result_response() {
        let message = Message::from(
            ResultResponse::with_result(id(9), &json!({"k": [1, 2, {"x": null}]})).expect("object"),
        );
        assert_eq!(round_trip(&message), message);
        let empty = Message::from(ResultResponse::empty(RequestId::from("p")));
        assert_eq!(round_trip(&empty), empty);
    }

    #[test]
    fn ori_t_0158_round_trip_error_response_with_and_without_id() {
        let with_id = Message::from(ErrorResponse {
            id: Some(id(3)),
            error: ErrorObject {
                code: METHOD_NOT_FOUND,
                message: "method not found".to_owned(),
                data: Some(json!({"method": "x"})),
            },
        });
        assert_eq!(round_trip(&with_id), with_id);
        let without = ProtocolError::new(ErrorCode::ParseError, "parse error").to_error_response();
        assert_eq!(without.id, None);
        let message = Message::from(without);
        let text = String::from_utf8(wire(&message)).expect("utf-8");
        assert!(!text.contains("\"id\""), "no id member at all: {text}");
        assert_eq!(round_trip(&message), message);
    }

    #[test]
    fn ori_t_0158_round_trip_initialize_request_and_result() {
        let params = InitializeParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_owned(),
            capabilities: ClientCapabilities {
                roots: Some(
                    json!({"listChanged": true})
                        .as_object()
                        .cloned()
                        .expect("obj"),
                ),
                sampling: Some(JsonObject::new()),
                ..ClientCapabilities::default()
            },
            client_info: client_info(),
        };
        let request =
            Message::from(Request::with_params(id(1), METHOD_INITIALIZE, &params).expect("params"));
        let back = round_trip(&request);
        assert_eq!(back, request);
        let Message::Request(back) = back else {
            panic!("a request")
        };
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, back),
            Ok((id(1), KnownRequest::Initialize(params)))
        );

        let result = InitializeResult {
            protocol_version: LATEST_PROTOCOL_VERSION.to_owned(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
                ..ServerCapabilities::default()
            },
            server_info: server_info(),
            instructions: Some("Optional instructions for the client".to_owned()),
        };
        let response = Message::from(ResultResponse::with_result(id(1), &result).expect("result"));
        let Message::ResultResponse(back) = round_trip(&response) else {
            panic!("a result")
        };
        assert_eq!(back.result_as::<InitializeResult>(), Ok(result));
    }

    #[test]
    fn ori_t_0158_round_trip_ping_and_initialized() {
        let ping = Request::without_params(id(2), METHOD_PING);
        let Message::Request(ping) = round_trip(&Message::from(ping)) else {
            panic!("a request")
        };
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, ping),
            Ok((id(2), KnownRequest::Ping))
        );
        let initialized = Notification::without_params(NOTIFICATION_INITIALIZED);
        let Message::Notification(initialized) = round_trip(&Message::from(initialized)) else {
            panic!("a notification")
        };
        assert_eq!(
            KnownNotification::decode(Direction::ClientToServer, initialized),
            Ok(Some(KnownNotification::Initialized))
        );
    }

    #[test]
    fn ori_t_0158_round_trip_tools_list_with_pagination() {
        let first = Request::with_params(id(3), METHOD_TOOLS_LIST, &ListToolsParams::default())
            .expect("params");
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, first),
            Ok((
                id(3),
                KnownRequest::ToolsList(ListToolsParams { cursor: None })
            ))
        );
        let page = ListToolsResult {
            tools: vec![Tool {
                name: "get_weather".to_owned(),
                title: Some("Weather Information Provider".to_owned()),
                description: Some("Get current weather information for a location".to_owned()),
                input_schema: json!({
                    "type": "object",
                    "properties": {"location": {"type": "string"}},
                    "required": ["location"]
                })
                .as_object()
                .cloned()
                .expect("obj"),
                output_schema: None,
                annotations: Some(ToolAnnotations {
                    read_only_hint: Some(true),
                    ..ToolAnnotations::default()
                }),
                execution: Some(
                    json!({"taskSupport": "optional"})
                        .as_object()
                        .cloned()
                        .expect("obj"),
                ),
            }],
            next_cursor: Some("next-page-cursor".to_owned()),
        };
        let message = Message::from(ResultResponse::with_result(id(3), &page).expect("result"));
        let Message::ResultResponse(back) = round_trip(&message) else {
            panic!("a result")
        };
        let back: ListToolsResult = back.result_as().expect("typed");
        assert_eq!(back, page);
        // The second page asks with the cursor the first returned.
        let second = Request::with_params(
            id(4),
            METHOD_TOOLS_LIST,
            &ListToolsParams {
                cursor: back.next_cursor,
            },
        )
        .expect("params");
        let Message::Request(second) = round_trip(&Message::from(second)) else {
            panic!("a request")
        };
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, second),
            Ok((
                id(4),
                KnownRequest::ToolsList(ListToolsParams {
                    cursor: Some("next-page-cursor".to_owned())
                })
            ))
        );
    }

    #[test]
    fn ori_t_0158_round_trip_tools_call_and_result() {
        let params = CallToolParams {
            name: "get_weather".to_owned(),
            arguments: Some(
                json!({"location": "New York"})
                    .as_object()
                    .cloned()
                    .expect("obj"),
            ),
            task: None,
            meta: Some(
                json!({"progressToken": 7})
                    .as_object()
                    .cloned()
                    .expect("obj"),
            ),
        };
        let request = Request::with_params(id(2), METHOD_TOOLS_CALL, &params).expect("params");
        let Message::Request(request) = round_trip(&Message::from(request)) else {
            panic!("a request")
        };
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, request),
            Ok((id(2), KnownRequest::ToolsCall(params)))
        );

        let result = CallToolResult {
            content: vec![
                ContentBlock::text("Current weather in New York:\nTemperature: 72\u{b0}F"),
                ContentBlock::from_value(
                    json!({"type": "image", "data": "AAAA", "mimeType": "image/png"}),
                )
                .expect("block"),
            ],
            structured_content: Some(json!({"t": 22.5}).as_object().cloned().expect("obj")),
            is_error: Some(false),
        };
        let message = Message::from(ResultResponse::with_result(id(2), &result).expect("result"));
        let Message::ResultResponse(back) = round_trip(&message) else {
            panic!("a result")
        };
        let back: CallToolResult = back.result_as().expect("typed");
        assert_eq!(back, result);
        assert!(!back.is_error());
        assert_eq!(
            back.content[0].as_text().map(|t| t.lines().count()),
            Some(2)
        );
        assert_eq!(back.content[1].kind(), "image");
        assert_eq!(back.content[1].as_text(), None);
    }

    #[test]
    fn ori_t_0158_round_trip_cancelled_and_list_changed() {
        let cancelled = Notification::with_params(
            NOTIFICATION_CANCELLED,
            &CancelledParams {
                request_id: Some(RequestId::from("req-1")),
                reason: None,
            },
        )
        .expect("params");
        let Message::Notification(back) = round_trip(&Message::from(cancelled)) else {
            panic!("a notification")
        };
        assert_eq!(
            KnownNotification::decode(Direction::ClientToServer, back),
            Ok(Some(KnownNotification::Cancelled(CancelledParams {
                request_id: Some(RequestId::from("req-1")),
                reason: None
            })))
        );
        let changed = Notification::without_params(NOTIFICATION_TOOLS_LIST_CHANGED);
        assert_eq!(
            KnownNotification::decode(Direction::ServerToClient, changed),
            Ok(Some(KnownNotification::ToolsListChanged))
        );
        let unknown = Notification::without_params("notifications/progress");
        assert_eq!(
            KnownNotification::decode(Direction::ClientToServer, unknown),
            Ok(None)
        );
        let bad = Notification {
            method: NOTIFICATION_CANCELLED.to_owned(),
            params: Some(json!({"requestId": 1.5})),
        };
        let error =
            KnownNotification::decode(Direction::ClientToServer, bad).expect_err("bad params");
        assert_eq!(error.code(), ErrorCode::InvalidParams);
        assert_eq!(error.id(), None);
    }

    #[test]
    fn ori_t_0158_call_tool_result_reads_is_error_and_defaults_it() {
        let ok: CallToolResult = serde_json::from_value(json!({"content": []})).expect("ok");
        assert!(!ok.is_error());
        let failed = CallToolResult::error_text("Invalid departure date");
        assert!(failed.is_error());
        let value = serde_json::to_value(&failed).expect("value");
        assert_eq!(value["isError"], json!(true));
        assert_eq!(value["content"][0]["type"], json!("text"));
        // `content` is required by the schema.
        assert!(serde_json::from_value::<CallToolResult>(json!({})).is_err());
    }

    // ----- the specification's own examples, kept small --------------------

    #[test]
    fn ori_t_0158_the_lifecycle_pages_initialize_examples_parse() {
        let request = parse(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25",
            "capabilities":{"roots":{"listChanged":true},"sampling":{},"elicitation":{"form":{},"url":{}}},
            "clientInfo":{"name":"ExampleClient","title":"Example Client Display Name","version":"1.0.0",
            "description":"An example MCP client application","websiteUrl":"https://example.com",
            "icons":[{"src":"https://example.com/icon.png","mimeType":"image/png","sizes":["48x48"]}]}}}"#
                .replace('\n', " ")
                .as_str(),
        )
        .expect("request");
        let Message::Request(request) = request else {
            panic!("a request")
        };
        let Ok((_, KnownRequest::Initialize(params))) =
            KnownRequest::decode(Direction::ClientToServer, request)
        else {
            panic!("initialize")
        };
        assert_eq!(params.protocol_version, "2025-11-25");
        assert_eq!(params.client_info.name, "ExampleClient");
        assert!(params.capabilities.elicitation.is_some());

        let response = parse(
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25",
            "capabilities":{"logging":{},"prompts":{"listChanged":true},"tools":{"listChanged":true}},
            "serverInfo":{"name":"ExampleServer","version":"1.0.0"},"instructions":"Optional instructions for the client"}}"#
                .replace('\n', " ")
                .as_str(),
        )
        .expect("response");
        let Message::ResultResponse(response) = response else {
            panic!("a result")
        };
        let result: InitializeResult = response.result_as().expect("typed");
        assert_eq!(
            result.capabilities.tools,
            Some(ToolsCapability {
                list_changed: Some(true)
            })
        );
        assert!(result.capabilities.logging.is_some());
        assert_eq!(
            result.instructions.as_deref(),
            Some("Optional instructions for the client")
        );
        assert!(matches!(
            parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Ok(Message::Notification(_))
        ));
    }

    #[test]
    fn ori_t_0158_the_tools_pages_examples_parse() {
        let list = parse(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"cursor":"optional-cursor-value"}}"#)
            .expect("list");
        let Message::Request(list) = list else {
            panic!("a request")
        };
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, list),
            Ok((
                id(1),
                KnownRequest::ToolsList(ListToolsParams {
                    cursor: Some("optional-cursor-value".to_owned())
                })
            ))
        );
        let error = parse(
            r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32602,"message":"Unknown tool: invalid_tool_name"}}"#,
        )
        .expect("error");
        let Message::ErrorResponse(error) = error else {
            panic!("an error")
        };
        assert_eq!(error.id, Some(id(3)));
        assert_eq!(
            ErrorCode::from_code(error.error.code),
            ErrorCode::InvalidParams
        );
        let execution_error = parse(
            r#"{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"Invalid departure date"}],"isError":true}}"#,
        )
        .expect("result");
        let Message::ResultResponse(execution_error) = execution_error else {
            panic!("a result")
        };
        assert!(
            execution_error
                .result_as::<CallToolResult>()
                .expect("typed")
                .is_error()
        );
    }

    #[test]
    fn ori_t_0158_non_text_blocks_are_carried_raw_and_checked() {
        for block in [
            json!({"type": "image", "data": "AAAA", "mimeType": "image/png", "annotations": {"priority": 0.9}}),
            json!({"type": "audio", "data": "AAAA", "mimeType": "audio/wav"}),
            json!({"type": "resource_link", "uri": "file:///a", "name": "a"}),
            json!({"type": "resource", "resource": {"uri": "file:///a", "text": "x"}}),
            json!({"type": "a_type_newer_than_this_file"}),
        ] {
            let parsed = ContentBlock::from_value(block.clone()).expect("carried");
            assert_ne!(parsed.kind(), "text");
            assert_eq!(
                serde_json::to_value(&parsed).expect("value"),
                block,
                "verbatim"
            );
        }
        for bad in [
            json!("text"),
            json!([]),
            json!({}),
            json!({"type": 5}),
            json!({"type": "text"}),
            json!({"type": "text", "text": 5}),
        ] {
            assert!(ContentBlock::from_value(bad).is_err());
        }
    }

    // ----- the line reader --------------------------------------------------

    #[test]
    fn ori_t_0158_a_line_over_the_default_cap_is_refused_and_the_cap_itself_is_not() {
        // Built at run time: one byte over the cap, then exactly at it.
        let mut over = vec![b'a'; MAX_LINE_BYTES + 1];
        over.push(b'\n');
        let mut reader = LineReader::new(over.as_slice());
        assert_eq!(
            reader.read_line(),
            Err(FrameError::TooLong {
                limit: MAX_LINE_BYTES
            })
        );

        let prefix = br#"{"jsonrpc":"2.0","method":"x","params":{"p":""#;
        let suffix = br#""}}"#;
        let mut exact = prefix.to_vec();
        exact.resize(MAX_LINE_BYTES - suffix.len(), b'a');
        exact.extend_from_slice(suffix);
        assert_eq!(exact.len(), MAX_LINE_BYTES);
        exact.push(b'\n');
        let mut reader = LineReader::new(exact.as_slice());
        assert!(matches!(
            reader.read_message(),
            Ok(Some(Message::Notification(_)))
        ));
        assert_eq!(MAX_LINE_BYTES, 1024 * 1024);
    }

    /// A `BufRead` that never ends and never has a newline.
    struct Endless {
        chunk: [u8; 64],
        consumed: usize,
    }

    impl Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(self.chunk.len());
            buf[..n].copy_from_slice(&self.chunk[..n]);
            self.consumed += n;
            Ok(n)
        }
    }

    impl BufRead for Endless {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Ok(&self.chunk)
        }
        fn consume(&mut self, amount: usize) {
            self.consumed += amount;
        }
    }

    #[test]
    fn ori_t_0158_the_reader_never_holds_more_than_the_cap_plus_a_constant() {
        // An endless line: the reader must give up, not read to the end.
        let cap = 1000;
        let mut reader = LineReader::with_max_line_bytes(
            Endless {
                chunk: [b'a'; 64],
                consumed: 0,
            },
            cap,
        );
        assert_eq!(reader.read_line(), Err(FrameError::TooLong { limit: cap }));
        assert!(
            reader.inner.consumed <= cap,
            "consumed {}",
            reader.inner.consumed
        );
        assert!(
            reader.line.capacity() <= cap,
            "buffer grew to {}",
            reader.line.capacity()
        );

        // A long line delivered through a small BufReader: same bound, and the
        // source is not read to its end (it is four caps long).
        let long = vec![b'b'; cap * 4];
        let mut reader =
            LineReader::with_max_line_bytes(BufReader::with_capacity(128, Cursor::new(long)), cap);
        assert!(reader.read_line().is_err());
        assert!(reader.line.capacity() <= cap);
        assert!(
            reader.inner.get_ref().position() <= u64::try_from(cap + 128).expect("small"),
            "read {} bytes of {}",
            reader.inner.get_ref().position(),
            cap * 4
        );

        // The bound also holds at the default cap, with a line a megabyte over.
        let huge = vec![b'c'; MAX_LINE_BYTES + 4096];
        let mut reader = LineReader::new(BufReader::with_capacity(8192, huge.as_slice()));
        assert!(reader.read_line().is_err());
        assert!(reader.line.capacity() <= MAX_LINE_BYTES);
    }

    #[test]
    fn ori_t_0158_after_a_refusal_the_tail_of_the_line_is_never_read_as_a_message() {
        let cap = 64;
        let mut stream = vec![b'a'; cap + 8];
        stream.extend_from_slice(br#"{"jsonrpc":"2.0","method":"ping","id":1}"#);
        stream.extend_from_slice(b"\n");
        stream.extend_from_slice(br#"{"jsonrpc":"2.0","method":"ping","id":2}"#);
        stream.extend_from_slice(b"\n");
        // A small buffer, so the reader has consumed part of the line when it
        // gives up: a reader that simply read on would then find the tail.
        let mut reader =
            LineReader::with_max_line_bytes(BufReader::with_capacity(16, stream.as_slice()), cap);
        let first = reader.read_message().expect_err("too long");
        assert!(first.is_stream_fatal());
        assert_eq!(first.to_protocol_error().code(), ErrorCode::InvalidRequest);
        // Every later read repeats the refusal; no message ever comes out.
        for _ in 0..3 {
            assert_eq!(reader.read_message(), Err(first.clone()));
            assert_eq!(reader.read_line(), Err(FrameError::TooLong { limit: cap }));
        }
    }

    #[test]
    fn ori_t_0158_the_reader_handles_framing_edges() {
        // Several lines, a blank one, a final line with no newline.
        let mut reader = LineReader::new(&b"one\n\ntwo\r\nthree"[..]);
        assert_eq!(reader.read_line(), Ok(Some(&b"one"[..])));
        assert_eq!(reader.read_line(), Ok(Some(&b""[..])));
        assert_eq!(reader.read_line(), Ok(Some(&b"two\r"[..])));
        assert_eq!(reader.read_line(), Ok(Some(&b"three"[..])));
        assert_eq!(reader.read_line(), Ok(None));
        assert_eq!(reader.read_line(), Ok(None));
        // A blank line is returned as a line and is not a message.
        let mut reader = LineReader::new(&b"\n"[..]);
        let error = reader.read_message().expect_err("blank");
        assert!(!error.is_stream_fatal());
        assert_eq!(error.to_protocol_error().code(), ErrorCode::ParseError);
        // Trailing CR is JSON whitespace and parses.
        let mut reader = LineReader::new(&b"{\"jsonrpc\":\"2.0\",\"method\":\"x\"}\r\n"[..]);
        assert!(matches!(
            reader.read_message(),
            Ok(Some(Message::Notification(_)))
        ));
    }

    /// Fails once with `Interrupted`, then serves `data`.
    struct Interrupting {
        interrupted: bool,
        data: &'static [u8],
    }

    impl Read for Interrupting {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(self.data.len());
            buf[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    impl BufRead for Interrupting {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::Error::from(ErrorKind::Interrupted));
            }
            Ok(self.data)
        }
        fn consume(&mut self, amount: usize) {
            self.data = &self.data[amount..];
        }
    }

    #[test]
    fn ori_t_0158_an_interrupted_read_is_retried() {
        let mut reader = LineReader::new(Interrupting {
            interrupted: false,
            data: b"{\"jsonrpc\":\"2.0\",\"method\":\"x\"}\n",
        });
        assert!(matches!(
            reader.read_message(),
            Ok(Some(Message::Notification(_)))
        ));
        assert_eq!(reader.read_message(), Ok(None));
    }

    struct Failing;

    impl Read for Failing {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(ErrorKind::BrokenPipe, "gone"))
        }
    }

    impl BufRead for Failing {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::Error::new(ErrorKind::BrokenPipe, "gone"))
        }
        fn consume(&mut self, _amount: usize) {}
    }

    #[test]
    fn ori_t_0158_an_io_failure_is_a_framing_error_that_repeats() {
        let mut reader = LineReader::new(Failing);
        let error = reader.read_line().expect_err("io");
        assert!(matches!(
            error,
            FrameError::Io {
                kind: ErrorKind::BrokenPipe,
                ..
            }
        ));
        assert_eq!(error.code(), ErrorCode::InternalError);
        assert_eq!(reader.read_line(), Err(error));
    }

    // ----- parse errors -----------------------------------------------------

    #[test]
    fn ori_t_0158_a_line_that_is_not_json_gives_parse_error() {
        for text in [
            "",
            "not json",
            "{",
            r#"{"jsonrpc":"2.0","method":"ping""#,
            r#"{"jsonrpc":"2.0","method":"ping"} trailing"#,
            "{'jsonrpc':'2.0'}",
            "\u{feff}{}",
        ] {
            let error = parse(text).expect_err(text);
            assert_eq!(error.code(), ErrorCode::ParseError, "{text}");
            assert_eq!(error.code().code(), -32700);
            assert_eq!(error.id(), None);
        }
        // Not UTF-8.
        let error = parse_message(b"{\"jsonrpc\":\"2.0\",\"method\":\"\xff\"}").expect_err("utf-8");
        assert_eq!(error.code(), ErrorCode::ParseError);
        // A lone surrogate escape is not a Unicode scalar value.
        let error = parse(r#"{"jsonrpc":"2.0","method":"\ud800"}"#).expect_err("surrogate");
        assert_eq!(error.code(), ErrorCode::ParseError);
        // Display never carries the peer's text.
        let error = parse("not json at all, secret-looking-text").expect_err("parse");
        assert!(!error.to_string().contains("secret-looking-text"));
    }

    #[test]
    fn ori_t_0158_json_that_is_not_a_valid_request_gives_invalid_request() {
        for text in [
            "5",
            "\"ping\"",
            "null",
            "true",
            "{}",
            r#"{"method":"ping","id":1}"#,
            r#"{"jsonrpc":"1.0","method":"ping","id":1}"#,
            r#"{"jsonrpc":2.0,"method":"ping","id":1}"#,
            r#"{"jsonrpc":"2.0","method":5,"id":1}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":1,"params":[1]}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":1,"params":null}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":1,"params":"x"}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":1,"result":{}}"#,
            r#"{"jsonrpc":"2.0","id":1}"#,
            r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{"code":1,"message":"m"}}"#,
            r#"{"jsonrpc":"2.0","id":1,"result":5}"#,
            r#"{"jsonrpc":"2.0","result":{}}"#,
            r#"{"jsonrpc":"2.0","id":null,"result":{}}"#,
            r#"{"jsonrpc":"2.0","id":1,"error":"boom"}"#,
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":1.5,"message":"m"}}"#,
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":1}}"#,
            r#"{"jsonrpc":"2.0","id":1.5,"error":{"code":1,"message":"m"}}"#,
        ] {
            let error = parse(text).expect_err(text);
            assert_eq!(error.code(), ErrorCode::InvalidRequest, "{text}");
            assert_eq!(error.code().code(), -32600);
        }
        // A readable id on a request-shaped value comes back with the error.
        let error =
            parse(r#"{"jsonrpc":"2.0","method":"ping","id":7,"params":[1]}"#).expect_err("params");
        assert_eq!(error.id(), Some(&id(7)));
        let response = error.to_error_response();
        assert_eq!(response.id, Some(id(7)));
        assert_eq!(response.error.code, -32600);
        // A response-shaped value does not hand its id back for an answer.
        let error = parse(r#"{"jsonrpc":"2.0","id":7,"result":5}"#).expect_err("result");
        assert_eq!(error.id(), None);
    }

    #[test]
    fn ori_t_0158_a_batch_is_refused_with_invalid_request() {
        for text in [
            "[]",
            r#"[{"jsonrpc":"2.0","method":"ping","id":1}]"#,
            r#"[{"jsonrpc":"2.0","method":"ping","id":1},{"jsonrpc":"2.0","method":"ping","id":2}]"#,
            "[1,2,3]",
        ] {
            let error = parse(text).expect_err(text);
            assert_eq!(error.code(), ErrorCode::InvalidRequest, "{text}");
            assert_eq!(error.message(), "batches are not supported");
            assert_eq!(error.id(), None);
        }
        let mut reader =
            LineReader::new(&b"[{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":1}]\n"[..]);
        let error = reader.read_message().expect_err("batch");
        assert!(!error.is_stream_fatal(), "the next line may be fine");
    }

    #[test]
    fn ori_t_0158_an_id_of_null_is_refused_and_other_ids_are_exact() {
        let error = parse(r#"{"jsonrpc":"2.0","method":"ping","id":null}"#).expect_err("null");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        assert_eq!(error.id(), None);
        for text in [
            r#"{"jsonrpc":"2.0","method":"ping","id":true}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":1.5}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":1.0}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":[1]}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":{}}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":18446744073709551615}"#,
        ] {
            let error = parse(text).expect_err(text);
            assert_eq!(error.code(), ErrorCode::InvalidRequest, "{text}");
        }
        // String and integer ids are echoed back as they came.
        for text in [
            r#"{"jsonrpc":"2.0","method":"ping","id":0}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":-5}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":9223372036854775807}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":"0"}"#,
            r#"{"jsonrpc":"2.0","method":"ping","id":""}"#,
        ] {
            let Message::Request(request) = parse(text).expect(text) else {
                panic!("a request")
            };
            let answer = Message::from(ResultResponse::empty(request.id.clone()));
            let again: Value =
                serde_json::from_slice(&serialize_message(&answer).expect("bytes")).expect("json");
            let original: Value = serde_json::from_str(text).expect("json");
            assert_eq!(again["id"], original["id"], "{text}");
        }
        // An error response may omit the id or carry null; a result may not.
        for text in [
            r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"m"}}"#,
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"m"}}"#,
        ] {
            let Message::ErrorResponse(response) = parse(text).expect(text) else {
                panic!("an error")
            };
            assert_eq!(response.id, None);
        }
    }

    #[test]
    fn ori_t_0158_a_duplicate_member_resolves_to_the_last_one() {
        // Pinned so a change of parser behaviour shows up; see the module
        // documentation, "JSON-RPC".
        let Ok(Message::Request(request)) =
            parse(r#"{"jsonrpc":"2.0","id":1,"method":"ping","method":"tools/list"}"#)
        else {
            panic!("a request")
        };
        assert_eq!(request.method, "tools/list");
    }

    #[test]
    fn ori_t_0158_deep_nesting_is_a_parse_error_not_a_stack_overflow() {
        let depth = MAX_LINE_BYTES / 2;
        let arrays = "[".repeat(depth);
        assert_eq!(
            parse(&arrays).expect_err("deep").code(),
            ErrorCode::ParseError
        );
        let objects = r#"{"a":"#.repeat(depth / 8);
        assert_eq!(
            parse(&objects).expect_err("deep").code(),
            ErrorCode::ParseError
        );
    }

    #[test]
    fn ori_t_0158_unknown_members_are_ignored_and_known_fields_are_checked() {
        let Message::Request(request) = parse(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","extra":true,"params":{"name":"t","future":1}}"#,
        )
        .expect("request") else {
            panic!("a request")
        };
        assert!(matches!(
            KnownRequest::decode(Direction::ClientToServer, request),
            Ok((RequestId::Number(1), KnownRequest::ToolsCall(_)))
        ));
        // `name` is required, `arguments` is an object.
        for params in [
            json!({}),
            json!({"name": 5}),
            json!({"name": "t", "arguments": [1]}),
        ] {
            let request = Request::with_params(id(2), METHOD_TOOLS_CALL, &params).expect("object");
            let error =
                KnownRequest::decode(Direction::ClientToServer, request).expect_err("bad params");
            assert_eq!(error.code(), ErrorCode::InvalidParams);
            assert_eq!(error.code().code(), -32602);
            assert_eq!(error.id(), Some(&id(2)));
        }
        // Absent params read as an empty object: fine for ping and tools/list,
        // not for initialize.
        assert_eq!(
            KnownRequest::decode(
                Direction::ClientToServer,
                Request::without_params(id(3), METHOD_TOOLS_LIST)
            ),
            Ok((id(3), KnownRequest::ToolsList(ListToolsParams::default())))
        );
        assert_eq!(
            KnownRequest::decode(
                Direction::ClientToServer,
                Request::without_params(id(4), METHOD_INITIALIZE)
            )
            .expect_err("missing")
            .code(),
            ErrorCode::InvalidParams
        );
    }

    #[test]
    fn ori_t_0158_an_unknown_method_is_method_not_found_with_a_bounded_echo() {
        let request = Request::without_params(id(8), "resources/list");
        let error = KnownRequest::decode(Direction::ClientToServer, request).expect_err("unknown");
        assert_eq!(error.code(), ErrorCode::MethodNotFound);
        assert_eq!(error.code().code(), -32601);
        assert_eq!(error.id(), Some(&id(8)));
        assert_eq!(error.data(), Some(&json!({"method": "resources/list"})));
        let long = Request::without_params(id(9), "m".repeat(10_000));
        let error = KnownRequest::decode(Direction::ClientToServer, long).expect_err("unknown");
        let echoed = error
            .data()
            .and_then(|d| d["method"].as_str())
            .expect("echo");
        assert_eq!(echoed.chars().count(), MAX_ECHO_CHARS);
        assert!(!error.to_string().contains("mmmm"));
    }

    #[test]
    fn ori_t_0158_the_error_codes_are_the_schemas() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
        for code in [
            ErrorCode::ParseError,
            ErrorCode::InvalidRequest,
            ErrorCode::MethodNotFound,
            ErrorCode::InvalidParams,
            ErrorCode::InternalError,
            ErrorCode::Other(-32042),
        ] {
            assert_eq!(ErrorCode::from_code(code.code()), code);
        }
        // A frame failure maps to a code; none is a refusal with a methodology
        // reference (the type carries none).
        assert_eq!(
            FrameError::TooLong { limit: 1 }.to_protocol_error().code(),
            ErrorCode::InvalidRequest
        );
        assert_eq!(FrameError::EmbeddedNewline.code(), ErrorCode::InternalError);
    }

    // ----- the writer -------------------------------------------------------

    #[test]
    fn ori_t_0158_an_embedded_newline_cannot_be_written() {
        for line in [&b"{\"a\":\n1}"[..], b"a\rb", b"\n", b"{}\n{}"] {
            let mut out = Vec::new();
            assert_eq!(write_line(&mut out, line), Err(FrameError::EmbeddedNewline));
            assert!(out.is_empty(), "nothing was written");
        }
        // Text with a newline in it is escaped by the serializer: one line on
        // the wire, and the newline is intact on the way back.
        let message = Message::from(
            Notification::with_params("x", &json!({"text": "line one\nline two\r\n\u{2028}end"}))
                .expect("params"),
        );
        let bytes = wire(&message);
        assert_eq!(bytes.iter().filter(|&&b| b == b'\n').count(), 1);
        assert!(!bytes[..bytes.len() - 1].contains(&b'\r'));
        let Message::Notification(back) = read_back(&bytes) else {
            panic!("a notification")
        };
        assert_eq!(
            back.params.expect("params")["text"],
            json!("line one\nline two\r\n\u{2028}end")
        );
        // Pretty-printed JSON is refused at the line writer.
        let pretty = serde_json::to_vec_pretty(&json!({"a": 1})).expect("pretty");
        assert_eq!(
            write_line(&mut Vec::new(), &pretty),
            Err(FrameError::EmbeddedNewline)
        );
    }

    /// Counts the calls to `write`, to show a line goes out in one.
    struct CountingWriter {
        writes: usize,
        bytes: Vec<u8>,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ori_t_0158_a_line_and_its_terminator_go_out_in_one_write() {
        let mut writer = CountingWriter {
            writes: 0,
            bytes: Vec::new(),
        };
        let message = Message::from(Request::without_params(id(1), METHOD_PING));
        write_message(&mut writer, &message).expect("write");
        assert_eq!(writer.writes, 1);
        assert_eq!(writer.bytes.last(), Some(&b'\n'));
        assert_eq!(
            String::from_utf8(writer.bytes).expect("utf-8"),
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n"
        );
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(ErrorKind::BrokenPipe, "gone"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ori_t_0158_a_failed_write_is_a_framing_error() {
        let message = Message::from(Request::without_params(id(1), METHOD_PING));
        let error = write_message(&mut FailingWriter, &message).expect_err("io");
        assert!(matches!(
            error,
            FrameError::Io {
                kind: ErrorKind::BrokenPipe,
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0158_params_and_results_must_be_objects() {
        assert!(Request::with_params(id(1), "x", &json!([1, 2])).is_err());
        assert!(Request::with_params(id(1), "x", &5).is_err());
        assert!(Notification::with_params("x", &json!("s")).is_err());
        assert!(ResultResponse::with_result(id(1), &json!(null)).is_err());
        assert!(ResultResponse::with_result(id(1), &json!({})).is_ok());
    }

    // ----- version negotiation ---------------------------------------------

    #[test]
    fn ori_t_0158_version_negotiation_for_a_supported_version() {
        assert_eq!(
            negotiate_version("2025-11-25", SUPPORTED_PROTOCOL_VERSIONS),
            Ok("2025-11-25")
        );
        // With two supported versions the requested one is answered, whichever.
        let two = ["2025-06-18", "2025-11-25"];
        assert_eq!(negotiate_version("2025-06-18", &two), Ok("2025-06-18"));
        assert_eq!(negotiate_version("2025-11-25", &two), Ok("2025-11-25"));
    }

    #[test]
    fn ori_t_0158_version_negotiation_for_an_unsupported_version() {
        // Well formed but not supported: answered with the server's latest.
        for requested in ["2025-06-18", "2024-11-05", "2099-01-01", "2025-13-45"] {
            assert_eq!(
                negotiate_version(requested, SUPPORTED_PROTOCOL_VERSIONS),
                Ok(LATEST_PROTOCOL_VERSION),
                "{requested}"
            );
        }
        // "Latest" is the greatest, wherever it sits in the list.
        let unordered = ["2025-06-18", "2025-11-25", "2024-11-05"];
        assert_eq!(
            negotiate_version("2023-01-01", &unordered),
            Ok("2025-11-25")
        );
        // The client's side of it: the answer must be one it supports.
        assert_eq!(
            check_server_version("2025-11-25", SUPPORTED_PROTOCOL_VERSIONS),
            Ok(())
        );
        let error = check_server_version("2024-11-05", SUPPORTED_PROTOCOL_VERSIONS)
            .expect_err("older server");
        assert_eq!(error.code(), ErrorCode::InvalidParams);
        assert_eq!(
            error.data(),
            Some(&json!({"supported": ["2025-11-25"], "requested": "2024-11-05"}))
        );
    }

    #[test]
    fn ori_t_0158_version_negotiation_for_a_malformed_version() {
        let long = "9".repeat(10_000);
        for requested in [
            "",
            "1.0.0",
            "latest",
            "2025-6-18",
            "2025/11/25",
            "2025-11-25 ",
            " 2025-11-25",
            "20251125",
            "2025-11-255",
            "abcd-ef-gh",
            "2025-1a-25",
            "-2025-11-2",
            "\u{ff12}\u{ff10}\u{ff12}\u{ff15}-11-25",
            long.as_str(),
        ] {
            let error =
                negotiate_version(requested, SUPPORTED_PROTOCOL_VERSIONS).expect_err(requested);
            assert_eq!(error.code(), ErrorCode::InvalidParams, "{requested:?}");
            assert_eq!(error.code().code(), -32602);
            let data = error.data().expect("data");
            assert_eq!(data["supported"], json!(["2025-11-25"]));
            assert!(data["requested"].as_str().expect("str").chars().count() <= MAX_ECHO_CHARS);
        }
        // The lifecycle page's own error example: requested "1.0.0".
        let error = negotiate_version("1.0.0", &["2024-11-05"]).expect_err("1.0.0");
        assert_eq!(
            error.data(),
            Some(&json!({"supported": ["2024-11-05"], "requested": "1.0.0"}))
        );
        // A bad supported list is the caller's defect, not the peer's.
        let empty: [&str; 0] = [];
        assert_eq!(
            negotiate_version("2025-11-25", &empty)
                .expect_err("empty")
                .code(),
            ErrorCode::InternalError
        );
        assert_eq!(
            negotiate_version("2025-11-25", &["nope"])
                .expect_err("bad")
                .code(),
            ErrorCode::InternalError
        );
    }

    #[test]
    fn ori_t_0158_the_supported_versions_are_exactly_the_latest_and_well_formed() {
        assert_eq!(SUPPORTED_PROTOCOL_VERSIONS, &["2025-11-25"]);
        assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25");
        assert!(
            SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .all(|v| is_version_shaped(v))
        );
    }

    // ----- connection state -------------------------------------------------

    /// The kinds permitted in `state` and `direction`, written out by hand from
    /// the lifecycle page, independent of `permits`.
    fn assert_exactly(state: ConnectionState, direction: Direction, allowed: &[MessageKind]) {
        for kind in MessageKind::ALL {
            assert_eq!(
                state.permits(direction, kind),
                allowed.contains(&kind),
                "{state:?} {direction:?} {kind:?}"
            );
        }
    }

    #[test]
    fn ori_t_0158_fresh_accepts_only_initialize_and_ping_from_the_client() {
        use MessageKind as K;
        let state = ConnectionState::Fresh;
        assert_exactly(
            state,
            Direction::ClientToServer,
            &[K::Initialize, K::Ping, K::Response],
        );
        // The server may send a ping or a log message and answer, nothing else.
        assert_exactly(
            state,
            Direction::ServerToClient,
            &[K::Ping, K::LoggingMessage, K::Response],
        );
        // As messages: tools/call is refused with the request's id; initialize is fine.
        let call = Message::from(Request::without_params(id(4), METHOD_TOOLS_CALL));
        let error = state
            .check(Direction::ClientToServer, &call)
            .expect_err("state");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        assert_eq!(error.id(), Some(&id(4)));
        assert_eq!(error.data(), Some(&json!({"state": "fresh"})));
        let init = Message::from(Request::without_params(id(1), METHOD_INITIALIZE));
        assert_eq!(state.check(Direction::ClientToServer, &init), Ok(()));
        // A notification refused has no id to answer with.
        let early = Message::from(Notification::without_params(NOTIFICATION_INITIALIZED));
        let error = state
            .check(Direction::ClientToServer, &early)
            .expect_err("early");
        assert_eq!(error.id(), None);
    }

    #[test]
    fn ori_t_0158_initialized_accepts_only_ping_and_the_initialized_notification() {
        use MessageKind as K;
        let state = ConnectionState::Initialized;
        assert_exactly(
            state,
            Direction::ClientToServer,
            &[K::Ping, K::Initialized, K::Cancelled, K::Response],
        );
        assert_exactly(
            state,
            Direction::ServerToClient,
            &[K::Ping, K::LoggingMessage, K::Response],
        );
        // A second initialize is out of order; so is real work.
        let again = Message::from(Request::without_params(id(2), METHOD_INITIALIZE));
        assert!(state.check(Direction::ClientToServer, &again).is_err());
        let list = Message::from(Request::without_params(id(3), METHOD_TOOLS_LIST));
        assert!(state.check(Direction::ClientToServer, &list).is_err());
        let ping = Message::from(Request::without_params(id(4), METHOD_PING));
        assert_eq!(state.check(Direction::ClientToServer, &ping), Ok(()));
    }

    #[test]
    fn ori_t_0158_ready_accepts_everything_but_a_second_handshake() {
        use MessageKind as K;
        let state = ConnectionState::Ready;
        let all_but_handshake = [
            K::Ping,
            K::ToolsList,
            K::ToolsCall,
            K::OtherRequest,
            K::Cancelled,
            K::LoggingMessage,
            K::OtherNotification,
            K::Response,
        ];
        assert_exactly(state, Direction::ClientToServer, &all_but_handshake);
        assert_exactly(state, Direction::ServerToClient, &all_but_handshake);
        let again = Message::from(Notification::without_params(NOTIFICATION_INITIALIZED));
        assert!(state.check(Direction::ClientToServer, &again).is_err());
        let call = Message::from(Request::without_params(id(5), METHOD_TOOLS_CALL));
        assert_eq!(state.check(Direction::ClientToServer, &call), Ok(()));
    }

    #[test]
    fn ori_t_0158_the_state_advances_only_along_the_lifecycle() {
        let fresh = ConnectionState::Fresh;
        let initialized = fresh
            .advance(Transition::InitializeAnswered)
            .expect("first");
        assert_eq!(initialized, ConnectionState::Initialized);
        let ready = initialized
            .advance(Transition::InitializedNotified)
            .expect("second");
        assert_eq!(ready, ConnectionState::Ready);
        for (state, transition) in [
            (ConnectionState::Fresh, Transition::InitializedNotified),
            (ConnectionState::Initialized, Transition::InitializeAnswered),
            (ConnectionState::Ready, Transition::InitializeAnswered),
            (ConnectionState::Ready, Transition::InitializedNotified),
        ] {
            let error = state.advance(transition).expect_err("out of order");
            assert_eq!(error.code(), ErrorCode::InvalidRequest);
        }
    }

    #[test]
    fn ori_t_0158_message_kinds_follow_the_method_names() {
        assert_eq!(
            MessageKind::of_request("initialize"),
            MessageKind::Initialize
        );
        assert_eq!(
            MessageKind::of_request("tools/call"),
            MessageKind::ToolsCall
        );
        assert_eq!(
            MessageKind::of_request("sampling/createMessage"),
            MessageKind::OtherRequest
        );
        assert_eq!(
            MessageKind::of_notification("notifications/message"),
            MessageKind::LoggingMessage
        );
        assert_eq!(
            MessageKind::of_notification("notifications/progress"),
            MessageKind::OtherNotification
        );
        let response = Message::from(ResultResponse::empty(id(1)));
        assert_eq!(MessageKind::of(&response), MessageKind::Response);
        assert_eq!(MessageKind::ALL.len(), 10);
        // `ALL` lists each kind once: as a set it has ten members. (`dedup`
        // only removes adjacent repeats, so it could not show this.)
        let distinct: HashSet<MessageKind> = MessageKind::ALL.iter().copied().collect();
        assert_eq!(distinct.len(), 10, "ALL lists each kind once");
        // And `ALL` lists every kind: the match below has no wildcard arm, so a
        // new variant does not compile until it has an arm here, and the arms'
        // numbers are checked against `ALL`, so it is then not forgotten there.
        let number = |kind: MessageKind| -> usize {
            match kind {
                MessageKind::Initialize => 0,
                MessageKind::Ping => 1,
                MessageKind::ToolsList => 2,
                MessageKind::ToolsCall => 3,
                MessageKind::OtherRequest => 4,
                MessageKind::Initialized => 5,
                MessageKind::Cancelled => 6,
                MessageKind::LoggingMessage => 7,
                MessageKind::OtherNotification => 8,
                MessageKind::Response => 9,
            }
        };
        let numbers: HashSet<usize> = MessageKind::ALL.iter().map(|&kind| number(kind)).collect();
        assert_eq!(
            numbers,
            (0..10).collect::<HashSet<usize>>(),
            "every variant the match lists is in ALL"
        );
    }

    // ----- decoding by value (repair 1, finding F1) -------------------------

    /// A `tools/call` line whose `arguments.blob` is `blob`.
    fn call_line_with_blob(blob: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"t","arguments":{{"blob":"{blob}"}}}}}}"#
        )
    }

    /// The address of the heap buffer of `arguments.blob` in a parsed request.
    fn blob_address_in_tree(request: &Request) -> *const u8 {
        request
            .params
            .as_ref()
            .and_then(|params| params["arguments"]["blob"].as_str())
            .expect("a string in the tree")
            .as_ptr()
    }

    /// The address of the heap buffer of `arguments.blob` in decoded params.
    fn blob_address_decoded(params: &CallToolParams) -> *const u8 {
        params
            .arguments
            .as_ref()
            .and_then(|arguments| arguments["blob"].as_str())
            .expect("a string in the typed value")
            .as_ptr()
    }

    #[test]
    fn ori_t_0158_decoding_a_request_by_value_moves_a_large_string_instead_of_copying_it() {
        // Built at run time; the largest message that fits the default cap is
        // about twice this.
        let blob = "a".repeat(512 * 1024);
        let line = call_line_with_blob(&blob);
        assert!(line.len() < MAX_LINE_BYTES);
        let Message::Request(request) = parse(&line).expect("request") else {
            panic!("a request")
        };
        let in_tree = blob_address_in_tree(&request);

        // The control: decoding from a borrow copies the string, so the check
        // below can tell a copy from a move.
        let copied: CallToolParams = request.params_as().expect("decodes");
        assert_eq!(copied.arguments.as_ref().expect("arguments")["blob"], blob);
        assert_ne!(
            blob_address_decoded(&copied),
            in_tree,
            "params_as copies the string out of the tree"
        );

        // By value, the typed value holds the very buffer the tree held.
        let params: CallToolParams = request.into_params().expect("decodes");
        assert_eq!(params.arguments.as_ref().expect("arguments")["blob"], blob);
        assert_eq!(
            blob_address_decoded(&params),
            in_tree,
            "into_params moves the string out of the tree"
        );
    }

    #[test]
    fn ori_t_0158_known_request_decode_moves_a_large_string_instead_of_copying_it() {
        let blob = "b".repeat(512 * 1024);
        let Message::Request(request) = parse(&call_line_with_blob(&blob)).expect("request") else {
            panic!("a request")
        };
        let in_tree = blob_address_in_tree(&request);
        let (answered, decoded) =
            KnownRequest::decode(Direction::ClientToServer, request).expect("decodes");
        assert_eq!(answered, id(1));
        let KnownRequest::ToolsCall(params) = decoded else {
            panic!("tools/call")
        };
        assert_eq!(blob_address_decoded(&params), in_tree);
    }

    #[test]
    fn ori_t_0158_decoding_a_result_by_value_moves_a_large_text_instead_of_copying_it() {
        let text = "t".repeat(512 * 1024);
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":3,"result":{{"content":[{{"type":"text","text":"{text}"}}]}}}}"#
        );
        let Message::ResultResponse(response) = parse(&line).expect("result") else {
            panic!("a result")
        };
        let in_tree = response.result["content"][0]["text"]
            .as_str()
            .expect("a string in the tree")
            .as_ptr();
        let address_of =
            |result: &CallToolResult| result.content[0].as_text().expect("a text block").as_ptr();

        // The control: from a borrow the text is copied.
        let copied: CallToolResult = response.result_as().expect("decodes");
        assert_eq!(copied.content[0].as_text(), Some(text.as_str()));
        assert_ne!(address_of(&copied), in_tree, "result_as copies the text");

        // By value it is the same buffer, through the content block as well.
        let result: CallToolResult = response.into_result().expect("decodes");
        assert_eq!(result.content[0].as_text(), Some(text.as_str()));
        assert_eq!(address_of(&result), in_tree, "into_result moves the text");
    }

    #[test]
    fn ori_t_0158_by_value_and_by_reference_decoding_succeed_and_fail_alike() {
        // The same value on success, for a request, a notification and a result.
        let request = Request::with_params(
            id(1),
            METHOD_TOOLS_CALL,
            &json!({"name": "t", "arguments": {"a": [1, "two", null]}}),
        )
        .expect("object");
        assert_eq!(
            request.clone().into_params::<CallToolParams>(),
            request.params_as::<CallToolParams>()
        );
        assert!(request.params_as::<CallToolParams>().is_ok());
        let notification = Notification::with_params(
            NOTIFICATION_CANCELLED,
            &json!({"requestId": "r", "reason": "x"}),
        )
        .expect("object");
        assert_eq!(
            notification.clone().into_params::<CancelledParams>(),
            notification.params_as::<CancelledParams>()
        );
        assert!(notification.params_as::<CancelledParams>().is_ok());
        let response =
            ResultResponse::with_result(id(2), &CallToolResult::text("hi")).expect("object");
        assert_eq!(
            response.clone().into_result::<CallToolResult>(),
            response.result_as::<CallToolResult>()
        );
        assert!(response.result_as::<CallToolResult>().is_ok());

        // The same error on failure: code, id, and `data` (the echoed method and
        // the decoder's description), for each of the three.
        for params in [
            json!({}),
            json!({"name": 5}),
            json!({"name": "t", "arguments": [1]}),
        ] {
            let request = Request::with_params(id(2), METHOD_TOOLS_CALL, &params).expect("object");
            let by_ref = request.params_as::<CallToolParams>().expect_err("bad");
            let by_value = request
                .clone()
                .into_params::<CallToolParams>()
                .expect_err("bad");
            assert_eq!(by_value, by_ref);
            assert_eq!(by_value.code(), ErrorCode::InvalidParams);
            assert_eq!(by_value.id(), Some(&id(2)));
            let data = by_value.data().expect("data");
            assert_eq!(data["method"], json!("tools/call"));
            assert!(data["detail"].as_str().is_some_and(|d| !d.is_empty()));
        }
        let bad = Notification {
            method: NOTIFICATION_CANCELLED.to_owned(),
            params: Some(json!({"requestId": 1.5})),
        };
        let by_value = bad
            .clone()
            .into_params::<CancelledParams>()
            .expect_err("bad");
        assert_eq!(
            by_value,
            bad.params_as::<CancelledParams>().expect_err("bad")
        );
        assert_eq!(by_value.code(), ErrorCode::InvalidParams);
        assert_eq!(by_value.id(), None);
        let bad = ResultResponse::empty(id(3));
        let by_value = bad
            .clone()
            .into_result::<CallToolResult>()
            .expect_err("bad");
        assert_eq!(
            by_value,
            bad.result_as::<CallToolResult>().expect_err("bad")
        );
        assert_eq!(by_value.code(), ErrorCode::InvalidRequest);

        // Absent params read as an empty object by value too.
        assert_eq!(
            Request::without_params(id(4), METHOD_TOOLS_LIST).into_params::<ListToolsParams>(),
            Ok(ListToolsParams::default())
        );
        let missing = Request::without_params(id(5), METHOD_INITIALIZE)
            .into_params::<InitializeParams>()
            .expect_err("required");
        assert_eq!(missing.code(), ErrorCode::InvalidParams);
        assert_eq!(missing.id(), Some(&id(5)));
        assert_eq!(
            Notification::without_params("x").into_params::<ListToolsParams>(),
            Ok(ListToolsParams::default())
        );
    }

    #[test]
    fn ori_t_0158_a_text_block_still_needs_a_string_text_after_it_is_moved() {
        // `ContentBlock::from_value` now removes `text` instead of cloning it;
        // the refusals it had are unchanged.
        for bad in [json!({"type": "text"}), json!({"type": "text", "text": 5})] {
            assert_eq!(
                ContentBlock::from_value(bad),
                Err("a text block has a string text")
            );
        }
        assert_eq!(
            ContentBlock::from_value(json!({"type": "text", "text": "x", "annotations": {}})),
            Ok(ContentBlock::text("x"))
        );
    }

    #[test]
    fn ori_t_0158_the_detail_of_a_params_error_is_cut_and_the_message_stays_fixed() {
        // The module documentation says `detail` can quote the peer, cut to
        // `MAX_DETAIL_CHARS`, and that the text of the error does not.
        let big = "y".repeat(10_000);
        let request = Request::with_params(
            id(5),
            METHOD_TOOLS_CALL,
            &json!({"name": "t", "arguments": big}),
        )
        .expect("object");
        let error =
            KnownRequest::decode(Direction::ClientToServer, request).expect_err("bad params");
        let detail = error
            .data()
            .and_then(|data| data["detail"].as_str())
            .expect("detail");
        assert_eq!(detail.chars().count(), MAX_DETAIL_CHARS);
        assert!(
            detail.contains("yyyy"),
            "the detail quotes the peer: {detail}"
        );
        assert_eq!(error.message(), "invalid params");
        assert!(!error.to_string().contains("yyyy"));
    }

    // ----- direction (repair 1, finding F2) ----------------------------------

    #[test]
    fn ori_t_0158_a_servers_tools_call_and_tools_list_are_method_not_found_for_a_client() {
        // Valid parameters, so the refusal is the method's, not the params'.
        let call = Request::with_params(
            id(21),
            METHOD_TOOLS_CALL,
            &CallToolParams {
                name: "t".to_owned(),
                arguments: None,
                task: None,
                meta: None,
            },
        )
        .expect("params");
        let list = Request::with_params(id(22), METHOD_TOOLS_LIST, &ListToolsParams::default())
            .expect("params");
        let init = Request::with_params(
            id(23),
            METHOD_INITIALIZE,
            &InitializeParams {
                protocol_version: LATEST_PROTOCOL_VERSION.to_owned(),
                capabilities: ClientCapabilities::default(),
                client_info: client_info(),
            },
        )
        .expect("params");
        for (request, method, expected) in [
            (call, METHOD_TOOLS_CALL, 21),
            (list, METHOD_TOOLS_LIST, 22),
            (init, METHOD_INITIALIZE, 23),
        ] {
            let error = KnownRequest::decode(Direction::ServerToClient, request).expect_err(method);
            assert_eq!(error.code(), ErrorCode::MethodNotFound, "{method}");
            assert_eq!(error.code().code(), -32601);
            assert_eq!(error.id(), Some(&id(expected)), "{method}");
            assert_eq!(error.data(), Some(&json!({ "method": method })));
        }
        // Method-not-found wins over bad params: the method is refused before
        // its params are read.
        let bad =
            Request::with_params(id(24), METHOD_TOOLS_CALL, &json!({"name": 5})).expect("object");
        let error = KnownRequest::decode(Direction::ServerToClient, bad).expect_err("refused");
        assert_eq!(error.code(), ErrorCode::MethodNotFound);
        assert_eq!(error.id(), Some(&id(24)));
        // Every other method a server could send is refused the same way, and
        // the echoed method is bounded.
        for method in [
            "sampling/createMessage",
            "roots/list",
            "elicitation/create",
            "tasks/get",
            "resources/list",
        ] {
            let request = Request::without_params(id(25), method);
            let error = KnownRequest::decode(Direction::ServerToClient, request).expect_err(method);
            assert_eq!(error.code(), ErrorCode::MethodNotFound, "{method}");
            assert_eq!(error.id(), Some(&id(25)));
        }
        let long = Request::without_params(id(26), "m".repeat(10_000));
        let error = KnownRequest::decode(Direction::ServerToClient, long).expect_err("long");
        let echoed = error
            .data()
            .and_then(|data| data["method"].as_str())
            .expect("echo");
        assert_eq!(echoed.chars().count(), MAX_ECHO_CHARS);
    }

    #[test]
    fn ori_t_0158_a_servers_ping_is_a_ping_for_a_client_and_the_id_comes_back() {
        for direction in [Direction::ServerToClient, Direction::ClientToServer] {
            for request_id in [id(27), RequestId::from("p-1")] {
                let ping = Request::without_params(request_id.clone(), METHOD_PING);
                assert_eq!(
                    KnownRequest::decode(direction, ping),
                    Ok((request_id, KnownRequest::Ping)),
                    "{direction:?}"
                );
            }
        }
    }

    #[test]
    fn ori_t_0158_a_client_sent_list_changed_is_ignored_by_a_server() {
        let changed = Notification::without_params(NOTIFICATION_TOOLS_LIST_CHANGED);
        assert_eq!(
            KnownNotification::decode(Direction::ClientToServer, changed.clone()),
            Ok(None)
        );
        // In its own direction it is known.
        assert_eq!(
            KnownNotification::decode(Direction::ServerToClient, changed),
            Ok(Some(KnownNotification::ToolsListChanged))
        );
    }

    #[test]
    fn ori_t_0158_a_server_sent_initialized_is_ignored_by_a_client() {
        let initialized = Notification::without_params(NOTIFICATION_INITIALIZED);
        assert_eq!(
            KnownNotification::decode(Direction::ServerToClient, initialized.clone()),
            Ok(None)
        );
        // In its own direction it is known.
        assert_eq!(
            KnownNotification::decode(Direction::ClientToServer, initialized),
            Ok(Some(KnownNotification::Initialized))
        );
    }

    #[test]
    fn ori_t_0158_cancelled_is_known_both_ways_and_each_direction_has_its_own_set() {
        let params = CancelledParams {
            request_id: Some(id(9)),
            reason: Some("r".to_owned()),
        };
        let cancelled = Notification::with_params(NOTIFICATION_CANCELLED, &params).expect("params");
        for direction in [Direction::ClientToServer, Direction::ServerToClient] {
            assert_eq!(
                KnownNotification::decode(direction, cancelled.clone()),
                Ok(Some(KnownNotification::Cancelled(params.clone()))),
                "{direction:?}"
            );
            // Bad params are an error in either direction, with no id.
            let bad = Notification {
                method: NOTIFICATION_CANCELLED.to_owned(),
                params: Some(json!({"requestId": true})),
            };
            let error = KnownNotification::decode(direction, bad).expect_err("bad");
            assert_eq!(error.code(), ErrorCode::InvalidParams);
            assert_eq!(error.id(), None);
        }
        // The whole table: which known notifications each direction decodes.
        for (direction, method, known) in [
            (Direction::ClientToServer, NOTIFICATION_INITIALIZED, true),
            (
                Direction::ClientToServer,
                NOTIFICATION_TOOLS_LIST_CHANGED,
                false,
            ),
            (Direction::ClientToServer, NOTIFICATION_MESSAGE, false),
            (Direction::ServerToClient, NOTIFICATION_INITIALIZED, false),
            (
                Direction::ServerToClient,
                NOTIFICATION_TOOLS_LIST_CHANGED,
                true,
            ),
            (Direction::ServerToClient, NOTIFICATION_MESSAGE, false),
        ] {
            let decoded =
                KnownNotification::decode(direction, Notification::without_params(method));
            assert_eq!(
                decoded.expect("no params to fail").is_some(),
                known,
                "{direction:?} {method}"
            );
        }
        // For requests, the server's three methods are known to a server: with
        // no params `tools/call` and `initialize` fail on them (-32602), and
        // `tools/list` decodes, but none is "method not found".
        for method in [METHOD_INITIALIZE, METHOD_TOOLS_LIST, METHOD_TOOLS_CALL] {
            let to_server = KnownRequest::decode(
                Direction::ClientToServer,
                Request::without_params(id(30), method),
            );
            assert_ne!(
                to_server.err().map(|error| error.code()),
                Some(ErrorCode::MethodNotFound),
                "{method}"
            );
        }
    }

    // ----- notes from the review (repair 1) ------------------------------------

    #[test]
    fn ori_t_0158_negotiate_versions_error_takes_the_requests_id_and_round_trips() {
        let error =
            negotiate_version("1.0.0", SUPPORTED_PROTOCOL_VERSIONS).expect_err("malformed version");
        assert_eq!(error.id(), None, "negotiation does not see the request");
        let answered = error.with_id(Some(id(11)));
        let response = Message::from(answered.to_error_response());
        let Message::ErrorResponse(back) = round_trip(&response) else {
            panic!("an error response")
        };
        assert_eq!(back.id, Some(id(11)));
        assert_eq!(back.error.code, -32602);
        assert_eq!(
            back.error.data,
            Some(json!({"supported": ["2025-11-25"], "requested": "1.0.0"}))
        );
        // A string id is carried the same way.
        let by_string = negotiate_version("latest", SUPPORTED_PROTOCOL_VERSIONS)
            .expect_err("malformed version")
            .with_id(Some(RequestId::from("init-1")));
        assert_eq!(
            by_string.to_error_response().id,
            Some(RequestId::from("init-1"))
        );
    }

    /// Counts the calls to `fill_buf`, to show a read spanned several.
    struct CountingFills<R> {
        inner: R,
        fills: usize,
    }

    impl<R: Read> Read for CountingFills<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl<R: BufRead> BufRead for CountingFills<R> {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.fills += 1;
            self.inner.fill_buf()
        }
        fn consume(&mut self, amount: usize) {
            self.inner.consume(amount);
        }
    }

    #[test]
    fn ori_t_0158_a_valid_message_spanning_many_fill_buf_calls_decodes_exactly() {
        let params = CallToolParams {
            name: "get_weather".to_owned(),
            arguments: Some(
                json!({
                    "location": "New York",
                    "note": "line one\nline two \"quoted\" \\ back".repeat(8),
                    "unicode": "caf\u{e9} \u{1f600} \u{2028}",
                    "list": [1, 2.5, null, true, {"k": "v"}]
                })
                .as_object()
                .cloned()
                .expect("obj"),
            ),
            task: None,
            meta: Some(
                json!({"progressToken": "p"})
                    .as_object()
                    .cloned()
                    .expect("obj"),
            ),
        };
        let message = Message::from(
            Request::with_params(RequestId::from("multi"), METHOD_TOOLS_CALL, &params)
                .expect("params"),
        );
        let mut bytes = wire(&message);
        let line_length = bytes.len() - 1;
        assert!(line_length > 300, "a few hundred bytes: {line_length}");
        // A second message right behind it, so the boundary between two lines
        // inside one 16-byte buffer is crossed as well.
        bytes.extend_from_slice(&wire(&Message::from(Request::without_params(
            id(2),
            METHOD_PING,
        ))));

        let mut reader = LineReader::new(CountingFills {
            inner: BufReader::with_capacity(16, bytes.as_slice()),
            fills: 0,
        });
        let first = reader.read_message().expect("read").expect("a message");
        assert_eq!(first, message, "the message decodes exactly");
        assert!(
            reader.inner.fills >= line_length / 16,
            "{} fill_buf calls for {line_length} bytes",
            reader.inner.fills
        );
        let Message::Request(request) = first else {
            panic!("a request")
        };
        assert_eq!(
            KnownRequest::decode(Direction::ClientToServer, request),
            Ok((RequestId::from("multi"), KnownRequest::ToolsCall(params)))
        );
        let second = reader.read_message().expect("read").expect("a message");
        assert_eq!(
            second,
            Message::from(Request::without_params(id(2), METHOD_PING))
        );
        assert_eq!(reader.read_message(), Ok(None));
    }
}
