//! The registry of the operator's MCP servers: the record of which servers the
//! operator added to a product, the two events that record it, and the rules an
//! addition must satisfy: AICD §17, AICD §26, AICD §27.
//!
//! # What this module owes
//!
//! `spec/API_SPEC.md` section 1 lists `project.mcp.add`, `list` and `remove`, and section 3's
//! "Proxied tools" lets the operator expose tools of the operator's own servers to roles, under
//! five conditions. Condition 1 says only servers the operator added are proxied, each only to the
//! roles it lists; condition 2 fixes the name; condition 5 says a server added without the
//! operator's acknowledgement of its reach is exposed to no role. This module is the record those
//! conditions read from. [`crate::scopes::ToolScopes::authorize_user_tool`] decides on an
//! [`crate::scopes::ExposedServer`], and [`crate::host::registry::Registry::exposed`] is where one
//! comes from. `spec/DATA_MODEL.md` makes the `McpServer` table a projection rebuilt from the event
//! log, so the registry is a fold of two events over the log of one product, and no other state is
//! authoritative.
//!
//! The Client API methods that would call it (`project.mcp.add`, `list` and
//! `remove`) are the engine's, over `ori-rpc`'s dispatch; both are later work,
//! and no record yet assigns those methods.
//!
//! # The two events
//!
//! [`crate::host::registry::SERVER_ADDED_KIND`] (`mcp.server_added`) and
//! [`crate::host::registry::SERVER_REMOVED_KIND`] (`mcp.server_removed`) are two of the kinds
//! `spec/API_SPEC.md` section 2 lists. Both are appended by the registry with
//! `Actor::Human(operator)`, the identity the caller passes: no method of the registry takes an
//! `Actor`, so no call of the registry records an addition as an agent's or the system's, and an
//! addition appended around the registry is refused by the fold
//! ([`crate::host::registry::EventDefect::NotTheOperator`]). That the identity is a human's is the
//! caller's to make true: every RPC call is an event with a human actor (`spec/SECURITY_NOTES.md`,
//! trust boundary 1), authenticated by `ori-rpc` (later), and exposure is the operator's act, never
//! an agent's (`spec/ENV_SETUP.md` section 5).
//!
//! The payloads are written by `serde_json::to_string` from two structs, never
//! from `json!`, so the keys keep this order and the text is byte for byte
//! stable:
//!
//! ```text
//! mcp.server_added   {"server_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","name":"github","transport":"stdio","config":{"command":"github-mcp","args":["--read-only"]},"credential_ref":"MCP_GITHUB_01ARZ3NDEKTSV4RRFFQ69G5FAV","exposed_to_roles":["qa"],"acknowledged_reach":true}
//! mcp.server_removed {"server_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","name":"github"}
//! ```
//!
//! `credential_ref` is always written, as `null` when the server has no secret
//! of its own. Reading takes two passes over the same text, because serde's
//! derive reads a struct from a JSON array as well, one element per field: the
//! first pass reads the text as a `serde_json::Value` and refuses a top level
//! that is not an object, and a `config` that is not an object; the second
//! deserializes the same text into the struct, which refuses a key written
//! twice, an unknown key, a missing field other than `credential_ref`, a value
//! of the wrong JSON type and any text after the object. The registry never
//! shows serde's own message, which names the offending key.
//!
//! These events carry more than `spec/API_SPEC.md` section 3 lists for
//! `mcp.server_added` (name, transport, exposed roles and `acknowledged_reach`):
//! the server's id, its configuration and its credential reference, because
//! `spec/DATA_MODEL.md` makes `McpServer`, whose fields include those, a
//! projection rebuilt from the log. Section 3's opening paragraph says the
//! `mcp.*` events carry identifiers and lengths, never agent free text, and that
//! sentence is about the content of a call. Nothing in these two events comes
//! from an agent. The reading is the lead's, and ORI-T-0192 (the specification
//! reconciliation, later) is to record its wording.
//!
//! # The fold, and what one defective event costs
//!
//! [`crate::host::registry::Registry::load`] reads and verifies the whole log and folds it. Every
//! event is first checked for its product: an event of another product is refused whatever its
//! kind. An event whose kind is neither `mcp.server_added` nor `mcp.server_removed` is then
//! skipped, whoever its actor is (the kind is compared exactly, so `mcp.server_addedx` and
//! `MCP.SERVER_ADDED` are skipped); the two kinds are applied under the checks listed on
//! [`crate::host::registry::EventDefect`]. [`crate::host::registry::Registry::refresh`] folds only
//! what was appended since, after a verified read that checks the log is the one the registry
//! folded; [`crate::host::registry::Registry::add`] and [`crate::host::registry::Registry::remove`]
//! begin with it, and apply the event they append through the same function the fold uses.
//!
//! The event log is append-only and hash-chained (`spec/DATA_MODEL.md` section 2, `Event`), so no
//! event can be repaired or removed. When the fold refuses one, the registry records it and is
//! blocked for good: `refresh`, `add` and `remove` answer the same refusal and append nothing, and
//! [`crate::host::registry::Registry::exposed`] answers `None` for every name, so no grant is
//! decided on a state the log no longer supports. A registry loaded later from the same log refuses
//! the same event, and returns no registry. The way out is a human decision, carried out as a
//! change to this fold by a later ticket, never a change to the log. The cost is stated plainly:
//! **one defective `mcp.server_*` event, appended by any code holding the product's `ProductDb`,
//! stops `add`, `remove`, `load` and every exposure for that product, permanently, until then.**
//! That is the trade operator ruling R45, point 3, accepted for evidence, made here for the
//! registry.
//!
//! Only a defective event blocks. A database of another product handed to the
//! registry, a log that is not the one the registry folded, a failed read and an
//! encoding failure never do, and change nothing: `exposed` keeps answering the
//! last verified state while `add` and `remove` fail at their `refresh`. The
//! registry cannot tell its own log, edited past its triggers, from another
//! database handed to it, and an `Sql` error of the database may pass. Code
//! able to rewrite rows could as well append a human `mcp.server_added` the fold
//! accepts, so a tampered log is outside what the registry can stop; a restarted
//! engine's `load` answers the same error and exposes nothing.
//!
//! # How to read it
//!
//! - **Reading does not refresh.** [`crate::host::registry::Registry::servers`], `by_name`, `by_id`
//!   and `exposed` do not touch the database, and a removal takes effect for a decision only when
//!   that decision reads the registry `remove` updated.
//! - **A server's name is not its identity across removals.** A name may be added
//!   again under a new id, so whatever keys a connection or a cache to a server
//!   must key it by id.
//! - **The cost is the phase 1 cost, stated and not optimized.** `load` reads
//!   and verifies the whole log, and each `refresh`, `add` and `remove` verifies
//!   it again: O(log length) per call.
//!
//! # What this module does not do
//!
//! - It connects to no server and launches nothing: that is the host's client
//!   (slice H2, later), and spawning is `ori-runtime`'s (prerequisite PR1, later).
//! - It stores, reads and deletes no secret: the engine (later) gives
//!   `project.mcp.add`'s `secret` to the broker under the reference this registry
//!   derives, and resolving a reference is per product (PR1, later).
//! - It implements no Client API method: `project.mcp.add`, `list`, `remove` and
//!   `test` are the engine's and `ori-rpc`'s (later), and `test` needs H2.
//! - It proxies nothing: slice S6 (later) does, deciding with
//!   [`crate::scopes::ToolScopes::authorize_user_tool`] on
//!   [`crate::host::registry::Registry::exposed`].
//! - It does not check the operator's seat or authenticate the caller: the Client
//!   API's local authentication is `ori-rpc`'s (later), and the registry records
//!   the identity it is given.
//! - It does not detect a secret in a command or an argument, and it records no environment:
//!   [`crate::host::registry::StdioConfig`] says why, and what an operator must do. A reference to
//!   a server's secret is a name in the broker's keychain, which is one namespace for every
//!   product: [`crate::host::registry::CredentialRef`] says what follows.

use core::fmt;
use core::str::FromStr;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use ori_core::error::MethodologyRef;
use ori_core::types::{Actor, Id, Role, Timestamp};
use ori_store::db::ProductDb;
use ori_store::event_log::{Event, EventHash, EventLog, EventLogError};
use serde::{Deserialize, Serialize};

use crate::scopes::{ExposedServer, ServerName, ServerNameError};

/// The kind of the event that records an addition: `mcp.server_added`, one of
/// the kinds `spec/API_SPEC.md` section 2 lists (AICD §17).
pub const SERVER_ADDED_KIND: &str = "mcp.server_added";

/// The kind of the event that records a removal: `mcp.server_removed`, one of
/// the kinds `spec/API_SPEC.md` section 2 lists (AICD §17).
pub const SERVER_REMOVED_KIND: &str = "mcp.server_removed";

/// The longest a command, or one argument, may be, in bytes. An event payload
/// stays bounded, so a log read and folded whole stays bounded too (AICD §27).
pub const CONFIG_TEXT_MAX: usize = 4096;

/// The most arguments a command may have. An event payload stays bounded
/// (AICD §27).
pub const ARGS_MAX: usize = 64;

/// AICD §8, "Memory architecture": the append-only log is the store every
/// projection derives from, and a log that is not the one folded, or an event
/// the fold cannot read, is a refusal to project from it.
const MEMORY_SECTION: u8 = 8;

/// AICD §13, "Git and release workflow": the audit trail traces every change to
/// one record, so one identifier names one server for good.
const AUDIT_SECTION: u8 = 13;

/// AICD §17, "Autonomy tiers and the permission model": exposing a server to a
/// role is a permission, and only the operator grants it.
const PERMISSIONS_SECTION: u8 = 17;

/// AICD §26, "Operating several products with one team": one fleet per product,
/// and a product's servers and events never mix with another's.
const PRODUCTS_SECTION: u8 = 26;

/// AICD §27, "Security and compliance in depth": a server's name, its transport
/// and the command that launches it are what an untrusted party would forge.
const SECURITY_SECTION: u8 = 27;

/// A reference to a whole methodology section, built without a check because
/// every caller in this module passes one of the constants above.
const fn whole_section(section: u8) -> MethodologyRef {
    MethodologyRef {
        section,
        subsection: None,
    }
}

// ---------------------------------------------------------------------------
// The types of an addition
// ---------------------------------------------------------------------------

/// How the host reaches a server: AICD §27.
///
/// `spec/API_SPEC.md` section 3, in its last paragraph, says phase 1 connects to
/// stdio servers only and `project.mcp.add` accepts `stdio` and no other. A
/// remote (HTTP) server needs an HTTP and TLS client crate the workspace does not
/// have, which is a `new_dependency` decision, and `spec/ROADMAP.md`, "Later (not
/// scheduled)", records it as later (operator ruling R45, point 4).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Transport {
    /// A server the host launches as a process and speaks to over its standard
    /// input and output.
    Stdio,
}

impl Transport {
    /// The spelling the payload and `project.mcp.add` use: `stdio`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
        }
    }

    /// Reads a transport. Accepts exactly `stdio`, case-sensitive.
    ///
    /// # Errors
    ///
    /// [`ServerError::TransportNotSupported`] for every other text, the empty
    /// text included.
    pub fn parse(text: &str) -> Result<Transport, ServerError> {
        if text == Self::Stdio.as_str() {
            Ok(Self::Stdio)
        } else {
            Err(ServerError::TransportNotSupported)
        }
    }
}

/// What is needed to launch a stdio server: a command and its arguments
/// (AICD §27).
///
/// # The secret rule, and its limit
///
/// `spec/DATA_MODEL.md` section 2 makes `McpServer.config` non-secret, and
/// `spec/ENV_SETUP.md` section 4 ("no secret ... in events") and
/// `spec/SECURITY_NOTES.md`, "Secrets" ("never serialized into events"; a secret
/// in any event is an incident) keep secrets out of the log. Ori cannot tell
/// whether an argument holds a secret, so the configuration, recorded in clear in
/// `mcp.server_added`, is non-secret by the operator's contract, not by a check.
/// A secret put in an argument is in the log for good: the log is append-only,
/// the event is streamed to every `events.subscribe` client, and it is an
/// incident: rotate it. A server's secret belongs in `project.mcp.add`'s
/// `secret`, which the engine hands to `ori-broker`, the only component that
/// touches the keychain; this registry records only its name
/// ([`CredentialRef`]).
///
/// # No environment
///
/// Phase 1 records no environment variables. A server's secret is to reach its
/// process through `ori-runtime`'s environment injection at spawn (prerequisite
/// PR1, later, not built); the variable it is injected under is PR1's and H2's
/// decision, and this registry records no environment.
///
/// # What is refused
///
/// A command is not empty and at most [`CONFIG_TEXT_MAX`] bytes; there are at
/// most [`ARGS_MAX`] arguments, each at most [`CONFIG_TEXT_MAX`] bytes (an empty
/// argument is accepted); and no command or argument holds an unsafe character:
/// any character for which `char::is_control` is true, and the characters that
/// set text direction or end a line. A NUL cannot be passed to a process, since
/// it ends a C string. The configuration is printed by later surfaces
/// (`events.subscribe` clients, `project.mcp.list`, a CLI), where a control or a
/// reordering character would let it display as something other than what is
/// launched.
///
/// # Debug
///
/// The `Debug` output prints the command and the number of arguments, never an
/// argument, because Ori cannot tell whether an argument holds a secret.
#[derive(Clone, Eq, PartialEq)]
pub struct StdioConfig {
    command: String,
    args: Vec<String>,
}

impl StdioConfig {
    /// Checks and builds a configuration (AICD §27).
    ///
    /// The checks run in this order, and the first failure is the answer:
    ///
    /// 1. the command is empty;
    /// 2. the command is longer than [`CONFIG_TEXT_MAX`] bytes;
    /// 3. the command holds an unsafe character;
    /// 4. there are more than [`ARGS_MAX`] arguments;
    /// 5. each argument in turn, in order: longer than [`CONFIG_TEXT_MAX`]
    ///    bytes, then an unsafe character.
    ///
    /// # Errors
    ///
    /// The [`ServerError`] of the first check that fails. None of them echoes
    /// the text.
    pub fn new(command: String, args: Vec<String>) -> Result<StdioConfig, ServerError> {
        if command.is_empty() {
            return Err(ServerError::CommandEmpty);
        }
        if command.len() > CONFIG_TEXT_MAX {
            return Err(ServerError::CommandTooLong {
                length: command.len(),
            });
        }
        if command.chars().any(is_unsafe_char) {
            return Err(ServerError::CommandHasControl);
        }
        if args.len() > ARGS_MAX {
            return Err(ServerError::TooManyArgs { count: args.len() });
        }
        for (index, arg) in args.iter().enumerate() {
            if arg.len() > CONFIG_TEXT_MAX {
                return Err(ServerError::ArgTooLong {
                    index,
                    length: arg.len(),
                });
            }
            if arg.chars().any(is_unsafe_char) {
                return Err(ServerError::ArgHasControl { index });
            }
        }
        Ok(StdioConfig { command, args })
    }

    /// The command to launch.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// The arguments, in the operator's order.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

impl fmt::Debug for StdioConfig {
    /// Writes the command and the number of arguments, never an argument.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdioConfig")
            .field("command", &self.command)
            .field("args", &format_args!("{} argument(s)", self.args.len()))
            .finish()
    }
}

/// Whether a character may not appear in a command or an argument: AICD §27.
///
/// This repeats the rule of `ori-memory`'s `retrieval::is_unsafe_char`, which is
/// `pub(crate)` there and cannot be called from this crate: a control character
/// (`char::is_control`, which covers NUL, the other C0 controls, DEL and the C1
/// controls), U+061C, U+200E, U+200F, U+2028, U+2029, U+202A to U+202E and U+2066
/// to U+2069. A widening of that rule decided under ORI-T-0191 (later; its
/// recorded scope is `crates/ori-memory/src/retrieval.rs`) must be repeated here,
/// because this copy does not follow it on its own.
///
/// A NUL cannot be passed to a process, since it ends a C string. The
/// configuration is printed by later surfaces (`events.subscribe` clients,
/// `project.mcp.list`, a CLI), where a control or reordering character would let
/// it display as something other than what is launched.
fn is_unsafe_char(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

/// The name, within the server's product, of the keychain entry that holds the
/// server's own secret: AICD §26, AICD §27. Never a value.
///
/// `spec/ENV_SETUP.md` section 4 names a server's secrets `MCP_<SERVER>_*` (per
/// project) and leaves the `*` open. This registry fills it with the server's id,
/// and writes `<SERVER>` as the name upper-cased with each `-` written `_`: for
/// `github` with id `01ARZ3NDEKTSV4RRFFQ69G5FAV`, the reference is
/// `MCP_GITHUB_01ARZ3NDEKTSV4RRFFQ69G5FAV`. The longest is 4 + 32 + 1 + 26 = 63
/// bytes. Condition 4 of `spec/API_SPEC.md` section 3, "Proxied tools", keeps a
/// server's own secret with the engine.
///
/// **It is derived, never chosen**: [`CredentialRef::for_server`] is the only
/// constructor. It carries the id so that a reference belongs to one server for
/// good. A server added again after a removal has a new id, so it never inherits
/// the removed server's secret, and no two servers' references collide, although
/// `a`, `a-` and `a-b` all upper-case to overlapping prefixes.
///
/// The broker's keychain is one namespace for every product
/// (`crates/ori-broker/src/keychain.rs`, its `SERVICE`), so whatever resolves a
/// reference (prerequisite PR1, later) keys the entry by the server's
/// `product_id` together with the reference, never by the reference alone
/// (`spec/ENV_SETUP.md` section 4, "per project"; AICD §26). The registry does
/// not delete a removed server's keychain entry; that is the engine's and the
/// broker's (later).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CredentialRef(String);

impl CredentialRef {
    /// The reference of the server with this name and id (AICD §26).
    #[must_use]
    pub fn for_server(name: &ServerName, id: &Id) -> CredentialRef {
        CredentialRef(format!(
            "MCP_{}_{}",
            name.as_str().to_ascii_uppercase().replace('-', "_"),
            id.as_str()
        ))
    }

    /// The reference as text, never a secret.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An addition the operator asked for, every rule checked: AICD §17, AICD §27.
///
/// The one rule [`NewServer::new`] checks is that no role is listed twice; the
/// name, the transport and the configuration are values that were checked when
/// they were built ([`ServerName`], [`Transport`], [`StdioConfig`]).
///
/// `exposed_to_roles` keeps the operator's order and may be empty. Roles are
/// accepted when `acknowledged_reach` is false: `spec/API_SPEC.md` section 1 says
/// "a server added without `acknowledged_reach: true` is exposed to no role", so
/// the addition is recorded as the operator wrote it, and
/// [`crate::scopes::ToolScopes::authorize_user_tool`] refuses every call to it
/// ([`crate::scopes::ToolRefusal::ServerReachNotAcknowledged`]). Every role of
/// `Role::ALL` may be listed: which roles a server is exposed to is the
/// operator's decision (`spec/ENV_SETUP.md` section 5).
///
/// The id comes from the caller. `ori-core`'s `Id` never generates one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewServer {
    id: Id,
    name: ServerName,
    transport: Transport,
    config: StdioConfig,
    credential_ref: Option<CredentialRef>,
    exposed_to_roles: Vec<Role>,
    acknowledged_reach: bool,
}

impl NewServer {
    /// Checks an addition and builds it (AICD §17).
    ///
    /// `with_secret` says whether `project.mcp.add` was given a `secret`. When
    /// true, the server's credential reference is
    /// [`CredentialRef::for_server`] of its name and id; when false, it has none.
    ///
    /// # Errors
    ///
    /// [`ServerError::DuplicateRole`] naming the first role met a second time
    /// while walking `exposed_to_roles`.
    pub fn new(
        id: Id,
        name: ServerName,
        transport: Transport,
        config: StdioConfig,
        with_secret: bool,
        exposed_to_roles: Vec<Role>,
        acknowledged_reach: bool,
    ) -> Result<NewServer, ServerError> {
        let mut seen: Vec<Role> = Vec::with_capacity(exposed_to_roles.len());
        for role in &exposed_to_roles {
            if seen.contains(role) {
                return Err(ServerError::DuplicateRole { role: *role });
            }
            seen.push(*role);
        }
        let credential_ref = with_secret.then(|| CredentialRef::for_server(&name, &id));
        Ok(NewServer {
            id,
            name,
            transport,
            config,
            credential_ref,
            exposed_to_roles,
            acknowledged_reach,
        })
    }

    /// The identifier the caller gave the server.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The name the operator gave the server.
    #[must_use]
    pub fn name(&self) -> &ServerName {
        &self.name
    }

    /// How the host reaches the server.
    #[must_use]
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// What launches the server.
    #[must_use]
    pub fn config(&self) -> &StdioConfig {
        &self.config
    }

    /// The reference to the server's own secret, `None` when it has none.
    #[must_use]
    pub fn credential_ref(&self) -> Option<&CredentialRef> {
        self.credential_ref.as_ref()
    }

    /// The roles the operator listed, in the operator's order.
    #[must_use]
    pub fn exposed_to_roles(&self) -> &[Role] {
        &self.exposed_to_roles
    }

    /// Whether the operator acknowledged the server's reach.
    #[must_use]
    pub fn acknowledged_reach(&self) -> bool {
        self.acknowledged_reach
    }
}

/// A server the registry holds: AICD §26.
///
/// There is no public constructor and no public method that builds an
/// [`ExposedServer`]. The registry builds one only in [`Registry::exposed`], so
/// that an `ExposedServer` is taken from the registry at the moment of a decision
/// and never from a `RegisteredServer` kept from an earlier call: a removed
/// server's copy would still carry its roles and `acknowledged_reach`.
/// `ExposedServer::new` is public in `scopes.rs`, and code that builds one from
/// anything else is outside what this crate can stop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredServer {
    id: Id,
    product_id: Id,
    name: ServerName,
    transport: Transport,
    config: StdioConfig,
    credential_ref: Option<CredentialRef>,
    exposed_to_roles: Vec<Role>,
    acknowledged_reach: bool,
    added_at: Timestamp,
    added_by: Id,
    added_seq: u64,
}

impl RegisteredServer {
    /// The server's identifier, which is never reused.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The product the operator added the server to.
    #[must_use]
    pub fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The name the operator gave the server.
    #[must_use]
    pub fn name(&self) -> &ServerName {
        &self.name
    }

    /// How the host reaches the server.
    #[must_use]
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// What launches the server.
    #[must_use]
    pub fn config(&self) -> &StdioConfig {
        &self.config
    }

    /// The reference to the server's own secret, `None` when it has none.
    #[must_use]
    pub fn credential_ref(&self) -> Option<&CredentialRef> {
        self.credential_ref.as_ref()
    }

    /// The roles the operator listed, in the operator's order. They take effect
    /// only when [`RegisteredServer::acknowledged_reach`] is true (AICD §17).
    #[must_use]
    pub fn exposed_to_roles(&self) -> &[Role] {
        &self.exposed_to_roles
    }

    /// Whether the operator acknowledged the server's reach.
    #[must_use]
    pub fn acknowledged_reach(&self) -> bool {
        self.acknowledged_reach
    }

    /// When the addition was recorded, as the caller gave it.
    #[must_use]
    pub fn added_at(&self) -> Timestamp {
        self.added_at
    }

    /// The human identity in the actor of the addition.
    #[must_use]
    pub fn added_by(&self) -> &Id {
        &self.added_by
    }

    /// The `seq` of the server's `mcp.server_added` event, which is what
    /// `project.mcp.add` returns (`spec/API_SPEC.md`, the preamble: every
    /// mutating method returns the `Event.seq` it produced).
    #[must_use]
    pub fn added_seq(&self) -> u64 {
        self.added_seq
    }

    /// The facts [`crate::scopes::ToolScopes::authorize_user_tool`] decides on.
    /// Private, and called only by [`Registry::exposed`].
    fn exposure(&self) -> ExposedServer {
        ExposedServer::new(
            self.product_id.clone(),
            self.name.clone(),
            self.exposed_to_roles.clone(),
            self.acknowledged_reach,
        )
    }
}

/// What [`Registry::remove`] returns: the server that went and the `seq` of the
/// event that removed it (AICD §17).
///
/// `spec/API_SPEC.md`, in its preamble, says every mutating method returns the
/// `Event.seq` it produced. There is no public constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemovedServer {
    server: RegisteredServer,
    removed_seq: u64,
}

impl RemovedServer {
    /// The server as the registry held it before the removal.
    #[must_use]
    pub fn server(&self) -> &RegisteredServer {
        &self.server
    }

    /// The `seq` of the server's `mcp.server_removed` event.
    #[must_use]
    pub fn removed_seq(&self) -> u64 {
        self.removed_seq
    }

    /// The removed server, by value.
    #[must_use]
    pub fn into_server(self) -> RegisteredServer {
        self.server
    }
}

// ---------------------------------------------------------------------------
// The errors
// ---------------------------------------------------------------------------

/// An addition that breaks a rule: AICD §27, AICD §17.
///
/// No variant echoes the text that was refused. The `Display` is written by
/// hand, as ruling R20 allows until `thiserror` is adopted; converting it is
/// mechanical, each arm becoming an `#[error]` attribute.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ServerError {
    /// The transport is not `stdio`, the only one phase 1 accepts (AICD §27).
    TransportNotSupported,
    /// The command is empty (AICD §27).
    CommandEmpty,
    /// The command is longer than [`CONFIG_TEXT_MAX`] bytes (AICD §27).
    CommandTooLong {
        /// The length of the command in bytes.
        length: usize,
    },
    /// The command holds an unsafe character (AICD §27).
    CommandHasControl,
    /// There are more than [`ARGS_MAX`] arguments (AICD §27).
    TooManyArgs {
        /// How many arguments were given.
        count: usize,
    },
    /// An argument is longer than [`CONFIG_TEXT_MAX`] bytes (AICD §27).
    ArgTooLong {
        /// The position of the first such argument, from zero.
        index: usize,
        /// Its length in bytes.
        length: usize,
    },
    /// An argument holds an unsafe character (AICD §27).
    ArgHasControl {
        /// The position of the first such argument, from zero.
        index: usize,
    },
    /// A role is listed twice (AICD §17).
    DuplicateRole {
        /// The first role met a second time.
        role: Role,
    },
}

impl ServerError {
    /// The methodology section the refusal is made under: AICD §27 for every
    /// variant except [`ServerError::DuplicateRole`], which is AICD §17.
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        match self {
            Self::TransportNotSupported
            | Self::CommandEmpty
            | Self::CommandTooLong { .. }
            | Self::CommandHasControl
            | Self::TooManyArgs { .. }
            | Self::ArgTooLong { .. }
            | Self::ArgHasControl { .. } => whole_section(SECURITY_SECTION),
            Self::DuplicateRole { .. } => whole_section(PERMISSIONS_SECTION),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransportNotSupported => {
                f.write_str("refused: the transport is not supported, and phase 1 accepts stdio only")
            }
            Self::CommandEmpty => f.write_str("refused: the command to launch is empty"),
            Self::CommandTooLong { length } => write!(
                f,
                "refused: the command is {length} bytes long and at most {CONFIG_TEXT_MAX} are allowed"
            ),
            Self::CommandHasControl => f.write_str(
                "refused: the command holds a control character or a character that sets text direction",
            ),
            Self::TooManyArgs { count } => write!(
                f,
                "refused: {count} arguments were given and at most {ARGS_MAX} are allowed"
            ),
            Self::ArgTooLong { index, length } => write!(
                f,
                "refused: argument {index} is {length} bytes long and at most {CONFIG_TEXT_MAX} are allowed"
            ),
            Self::ArgHasControl { index } => write!(
                f,
                "refused: argument {index} holds a control character or a character that sets text direction"
            ),
            Self::DuplicateRole { role } => {
                write!(f, "refused: the role {role} is listed more than once")
            }
        }
    }
}

impl std::error::Error for ServerError {}

/// Why the fold refuses an event: AICD §17, AICD §26, AICD §27, AICD §8.
///
/// For an event of the kind `mcp.server_added`, the fold checks in this order:
/// the actor is a human, the payload reads, the server id parses, the name
/// parses, the transport and the configuration pass, each role parses, the whole
/// passes [`NewServer::new`], the payload's credential reference is the one the
/// server's own name and id derive, and the id and the name are free. For
/// `mcp.server_removed`: the actor is a human, the payload reads, the server id
/// parses, a live server has that id, and its name is the payload's.
///
/// No variant echoes the text that was refused. The `Display` is written by
/// hand, as ruling R20 allows until `thiserror` is adopted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EventDefect {
    /// The actor is not a human: an agent's or the system's addition exposes
    /// nothing (AICD §17).
    NotTheOperator,
    /// The event belongs to another product, whatever its kind (AICD §26).
    ForeignProduct,
    /// The payload does not read: not JSON, not an object where one is required,
    /// a repeated or unknown key, a missing field, a value of the wrong type, or
    /// text after the object (AICD §8).
    Payload,
    /// The server id is not a ULID (AICD §8).
    ServerId,
    /// The name breaks the rule of [`ServerName::parse`].
    Name(ServerNameError),
    /// The transport, the configuration or the roles break a rule of
    /// [`ServerError`].
    Server(ServerError),
    /// A role is not one of `Role::ALL` (AICD §8).
    Role,
    /// The credential reference is not the one the server's name and id derive
    /// (AICD §27).
    CredentialRef,
    /// The id was added before, live or removed (AICD §8).
    IdTaken,
    /// A live server has the name (AICD §8).
    NameTaken,
    /// No live server has the id: an id never added, and an id already removed,
    /// are alike (AICD §8).
    NotLive,
    /// The live server with the id has another name than the event's (AICD §8).
    NameMismatch,
}

impl EventDefect {
    /// The methodology section the refusal is made under: AICD §17 for
    /// [`EventDefect::NotTheOperator`], AICD §26 for
    /// [`EventDefect::ForeignProduct`], AICD §27 for
    /// [`EventDefect::CredentialRef`], the inner error's for
    /// [`EventDefect::Name`] and [`EventDefect::Server`], and AICD §8 for the
    /// rest (`ori-store`'s `ProjectionError::UnreadablePayload` and
    /// `MalformedPayload` cite it for the same kind of refusal).
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        match self {
            Self::NotTheOperator => whole_section(PERMISSIONS_SECTION),
            Self::ForeignProduct => whole_section(PRODUCTS_SECTION),
            Self::CredentialRef => whole_section(SECURITY_SECTION),
            Self::Payload
            | Self::ServerId
            | Self::Role
            | Self::NotLive
            | Self::NameMismatch
            | Self::IdTaken
            | Self::NameTaken => whole_section(MEMORY_SECTION),
            Self::Name(error) => error.methodology_ref(),
            Self::Server(error) => error.methodology_ref(),
        }
    }
}

impl fmt::Display for EventDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTheOperator => {
                f.write_str("the event's actor is not a human, so it is not an act of the operator")
            }
            Self::ForeignProduct => f.write_str("the event belongs to another product"),
            Self::Payload => f.write_str("the event's payload does not read"),
            Self::ServerId => f.write_str("the event's server id is not a ULID"),
            Self::Name(error) => write!(f, "the event's server name is refused: {error}"),
            Self::Server(error) => write!(f, "the event's addition is refused: {error}"),
            Self::Role => f.write_str("the event lists a role that is not one of the seven"),
            Self::CredentialRef => f.write_str(
                "the event's credential reference is not the one its server's name and id derive",
            ),
            Self::IdTaken => f.write_str("the event's server id was added before"),
            Self::NameTaken => f.write_str("a live server already has the event's server name"),
            Self::NotLive => f.write_str("no live server has the event's server id"),
            Self::NameMismatch => {
                f.write_str("the live server with the event's server id has another name")
            }
        }
    }
}

impl std::error::Error for EventDefect {}

/// Why a registry operation failed: AICD §26, AICD §27, AICD §17, AICD §13,
/// AICD §8.
///
/// No variant echoes input text: a command, an argument, a credential reference,
/// a payload or serde's message. A role, a server name or an id may be named. The
/// one exception is [`RegistryError::Log`], whose `Display` is the event log's
/// own. The `Display` is written by hand, as ruling R20 allows until `thiserror`
/// is adopted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RegistryError {
    /// The database's product identifier is not a ULID, so no registry can be
    /// loaded from it (AICD §26).
    ProductNotAnId,
    /// The database is not the product this registry belongs to (AICD §26).
    WrongProduct,
    /// The server's id was added before, live or removed, so it is taken
    /// (AICD §13: one identifier, one record in the audit trail).
    IdTaken {
        /// The id.
        id: Id,
    },
    /// A live server already has the name (AICD §27).
    NameTaken {
        /// The name.
        name: ServerName,
    },
    /// No live server has the id (AICD §17).
    NotFound {
        /// The id.
        id: Id,
    },
    /// The fold refused an event, and the registry is blocked for good
    /// (the module documentation, "The fold, and what one defective event costs").
    Event {
        /// The `seq` of the refused event.
        seq: u64,
        /// Why it was refused.
        defect: EventDefect,
    },
    /// The log is not the one the registry folded (AICD §8).
    LogMoved {
        /// The `seq` of the last event the registry folded.
        folded_seq: u64,
    },
    /// An event payload could not be encoded. These payloads never fail to
    /// encode; the variant exists so that nothing is appended if one does.
    Encode,
    /// The event log refused or failed (AICD §13 and AICD §8, for the refusals).
    Log(EventLogError),
}

impl RegistryError {
    /// The methodology section a refusal is made under, `None` for a failure
    /// that is no control's refusal.
    ///
    /// AICD §26 for [`RegistryError::ProductNotAnId`] and
    /// [`RegistryError::WrongProduct`], AICD §13 for [`RegistryError::IdTaken`],
    /// AICD §27 for [`RegistryError::NameTaken`], AICD §17 for
    /// [`RegistryError::NotFound`], AICD §8 for [`RegistryError::LogMoved`], the
    /// defect's for [`RegistryError::Event`], the inner error's for
    /// [`RegistryError::Log`] (as `RebuildError::EventLog` does), and none for
    /// [`RegistryError::Encode`].
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::ProductNotAnId | Self::WrongProduct => Some(whole_section(PRODUCTS_SECTION)),
            Self::IdTaken { .. } => Some(whole_section(AUDIT_SECTION)),
            Self::NameTaken { .. } => Some(whole_section(SECURITY_SECTION)),
            Self::NotFound { .. } => Some(whole_section(PERMISSIONS_SECTION)),
            Self::Event { defect, .. } => Some(defect.methodology_ref()),
            Self::LogMoved { .. } => Some(whole_section(MEMORY_SECTION)),
            Self::Encode => None,
            Self::Log(error) => error.methodology_ref(),
        }
    }

    /// Whether a control refused the operation, as opposed to a failure: true
    /// for every variant except [`RegistryError::Encode`], and
    /// [`RegistryError::Log`] when the event log's own error is no refusal.
    #[must_use]
    pub fn is_refusal(&self) -> bool {
        match self {
            Self::ProductNotAnId
            | Self::WrongProduct
            | Self::IdTaken { .. }
            | Self::NameTaken { .. }
            | Self::NotFound { .. }
            | Self::Event { .. }
            | Self::LogMoved { .. } => true,
            Self::Encode => false,
            Self::Log(error) => error.is_refusal(),
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProductNotAnId => f.write_str(
                "refused: the database's product identifier is not a ULID, so no registry can be loaded from it",
            ),
            Self::WrongProduct => f.write_str(
                "refused: this database is not the product this registry belongs to",
            ),
            Self::IdTaken { id } => write!(
                f,
                "refused: the server id {id} was added before and is never reused"
            ),
            Self::NameTaken { name } => {
                write!(f, "refused: a live server is already named {name}")
            }
            Self::NotFound { id } => write!(f, "refused: no live server has the id {id}"),
            Self::Event { seq, defect } => write!(
                f,
                "refused: the event at seq {seq} cannot be folded ({defect}), and the registry is blocked until a later ticket changes the fold"
            ),
            Self::LogMoved { folded_seq } => write!(
                f,
                "refused: the log is not the one this registry folded up to seq {folded_seq}"
            ),
            Self::Encode => f.write_str("an event payload could not be encoded, and nothing was appended"),
            Self::Log(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RegistryError {}

// ---------------------------------------------------------------------------
// The payloads
// ---------------------------------------------------------------------------

/// The payload of `mcp.server_added`, in the order its keys are written.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AddedWire {
    server_id: String,
    name: String,
    transport: String,
    config: ConfigWire,
    credential_ref: Option<String>,
    exposed_to_roles: Vec<String>,
    acknowledged_reach: bool,
}

/// The `config` of an `mcp.server_added` payload.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigWire {
    command: String,
    args: Vec<String>,
}

/// The payload of `mcp.server_removed`, in the order its keys are written.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RemovedWire {
    server_id: String,
    name: String,
}

/// Writes the payload of an addition.
fn added_payload(server: &NewServer) -> Result<String, serde_json::Error> {
    serde_json::to_string(&AddedWire {
        server_id: server.id.as_str().to_owned(),
        name: server.name.as_str().to_owned(),
        transport: server.transport.as_str().to_owned(),
        config: ConfigWire {
            command: server.config.command.clone(),
            args: server.config.args.clone(),
        },
        credential_ref: server
            .credential_ref
            .as_ref()
            .map(|r| r.as_str().to_owned()),
        exposed_to_roles: server
            .exposed_to_roles
            .iter()
            .map(|role| role.as_str().to_owned())
            .collect(),
        acknowledged_reach: server.acknowledged_reach,
    })
}

/// Writes the payload of a removal.
fn removed_payload(server: &RegisteredServer) -> Result<String, serde_json::Error> {
    serde_json::to_string(&RemovedWire {
        server_id: server.id.as_str().to_owned(),
        name: server.name.as_str().to_owned(),
    })
}

/// Reads the payload of `mcp.server_added` in two passes (the module
/// documentation, "The two events"): the shape on a `serde_json::Value`, then the
/// same text into the struct.
fn read_added(payload: &str) -> Result<AddedWire, EventDefect> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| EventDefect::Payload)?;
    let shape_holds = value.as_object().is_some_and(|object| {
        object
            .get("config")
            .is_some_and(serde_json::Value::is_object)
    });
    if !shape_holds {
        return Err(EventDefect::Payload);
    }
    serde_json::from_str::<AddedWire>(payload).map_err(|_| EventDefect::Payload)
}

/// Reads the payload of `mcp.server_removed` in two passes, as [`read_added`].
fn read_removed(payload: &str) -> Result<RemovedWire, EventDefect> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| EventDefect::Payload)?;
    if !value.is_object() {
        return Err(EventDefect::Payload);
    }
    serde_json::from_str::<RemovedWire>(payload).map_err(|_| EventDefect::Payload)
}

// ---------------------------------------------------------------------------
// The state and the fold
// ---------------------------------------------------------------------------

/// What the fold builds: the position it reached in the log, the live servers
/// and every id the log has ever added.
#[derive(Clone, Debug)]
struct State {
    /// The `seq` of the last event folded, 0 for none.
    last_seq: u64,
    /// That event's digest, `None` for none.
    last_digest: Option<EventHash>,
    /// The live servers, by name, so that they list in name order.
    live: BTreeMap<String, RegisteredServer>,
    /// Every server id the log has ever added, live or removed.
    ever_added: BTreeSet<Id>,
}

impl State {
    /// The state before any event.
    fn empty() -> State {
        State {
            last_seq: 0,
            last_digest: None,
            live: BTreeMap::new(),
            ever_added: BTreeSet::new(),
        }
    }
}

/// Which state check an addition failed: the id first, then the name.
enum Conflict {
    /// The id was added before, live or removed.
    Id,
    /// A live server has the name.
    Name,
}

/// The state checks of an addition, for [`Registry::add`] and the fold alike, so
/// that the two paths cannot disagree (AICD §13, AICD §27).
///
/// The id is checked against every id ever added: an id is never reused, so a
/// removed server's id cannot come back as another server. The name is checked
/// against the live servers: condition 2 of `spec/API_SPEC.md` section 3,
/// "Proxied tools", makes the server part of `<server>__<tool>` the name given at
/// addition and splits it off at the first `__`, and this check makes that name
/// resolve to one live server. A removed server's name may be added again, under
/// a new id.
fn check_state(state: &State, server: &NewServer) -> Result<(), Conflict> {
    if state.ever_added.contains(&server.id) {
        return Err(Conflict::Id);
    }
    if state.live.contains_key(server.name.as_str()) {
        return Err(Conflict::Name);
    }
    Ok(())
}

/// Applies an `mcp.server_added` event to the state, checking in the order
/// [`EventDefect`] lists. On success it sets `last_seq` and `last_digest` to the
/// event's, after every check has passed and before it inserts, and returns the
/// server it inserted. On a refusal the state is unchanged.
fn apply_added<'s>(
    state: &'s mut State,
    product_id: &Id,
    event: &Event,
) -> Result<&'s RegisteredServer, EventDefect> {
    let Actor::Human(operator) = event.actor() else {
        return Err(EventDefect::NotTheOperator);
    };
    let wire = read_added(event.payload())?;
    let id = Id::parse(&wire.server_id).map_err(|_| EventDefect::ServerId)?;
    let name = ServerName::parse(&wire.name).map_err(EventDefect::Name)?;
    let transport = Transport::parse(&wire.transport).map_err(EventDefect::Server)?;
    let config =
        StdioConfig::new(wire.config.command, wire.config.args).map_err(EventDefect::Server)?;
    let mut roles = Vec::with_capacity(wire.exposed_to_roles.len());
    for text in &wire.exposed_to_roles {
        roles.push(Role::from_str(text).map_err(|_| EventDefect::Role)?);
    }
    let checked = NewServer::new(
        id,
        name,
        transport,
        config,
        wire.credential_ref.is_some(),
        roles,
        wire.acknowledged_reach,
    )
    .map_err(EventDefect::Server)?;
    if wire.credential_ref.as_deref() != checked.credential_ref().map(CredentialRef::as_str) {
        return Err(EventDefect::CredentialRef);
    }
    check_state(state, &checked).map_err(|conflict| match conflict {
        Conflict::Id => EventDefect::IdTaken,
        Conflict::Name => EventDefect::NameTaken,
    })?;
    let server = RegisteredServer {
        id: checked.id.clone(),
        product_id: product_id.clone(),
        name: checked.name.clone(),
        transport: checked.transport,
        config: checked.config,
        credential_ref: checked.credential_ref,
        exposed_to_roles: checked.exposed_to_roles,
        acknowledged_reach: checked.acknowledged_reach,
        added_at: event.at(),
        added_by: operator.clone(),
        added_seq: event.seq(),
    };
    match state.live.entry(checked.name.as_str().to_owned()) {
        Entry::Vacant(entry) => {
            state.last_seq = event.seq();
            state.last_digest = Some(event.digest());
            state.ever_added.insert(checked.id);
            Ok(entry.insert(server))
        }
        Entry::Occupied(_) => Err(EventDefect::NameTaken),
    }
}

/// Applies an `mcp.server_removed` event to the state, checking in the order
/// [`EventDefect`] lists. On success it sets `last_seq` and `last_digest` to the
/// event's and returns the server that left the live map; its id stays in the set
/// of ids ever added. On a refusal the state is unchanged.
fn apply_removed(state: &mut State, event: &Event) -> Result<RegisteredServer, EventDefect> {
    let Actor::Human(_) = event.actor() else {
        return Err(EventDefect::NotTheOperator);
    };
    let wire = read_removed(event.payload())?;
    let id = Id::parse(&wire.server_id).map_err(|_| EventDefect::ServerId)?;
    let Some(live) = state.live.values().find(|server| server.id == id) else {
        return Err(EventDefect::NotLive);
    };
    if live.name.as_str() != wire.name {
        return Err(EventDefect::NameMismatch);
    }
    let key = live.name.as_str().to_owned();
    match state.live.remove(&key) {
        Some(server) => {
            state.last_seq = event.seq();
            state.last_digest = Some(event.digest());
            Ok(server)
        }
        None => Err(EventDefect::NotLive),
    }
}

/// Folds one event: the product is checked first, whatever the kind; an event of
/// either of the two kinds is applied; every other kind is skipped, whoever its
/// actor is. A skipped event still moves the position of the state to its own.
fn fold(state: &mut State, product_id: &Id, event: &Event) -> Result<(), EventDefect> {
    if event.product_id() != product_id {
        return Err(EventDefect::ForeignProduct);
    }
    match event.kind() {
        SERVER_ADDED_KIND => {
            apply_added(state, product_id, event)?;
        }
        SERVER_REMOVED_KIND => {
            apply_removed(state, event)?;
        }
        _ => {
            state.last_seq = event.seq();
            state.last_digest = Some(event.digest());
        }
    }
    Ok(())
}

/// Whether the events [`Registry::refresh`] read are the continuation of the log
/// it folded: the `first` of them chains from the last event folded (from
/// `EventHash::GENESIS` when none was), and the `last` of them is the tip the
/// verified read reported (AICD §8).
///
/// A read that fails either is not the log the registry folded, however the two
/// reads came to disagree, and the registry folds none of it.
fn chains_to_tip(
    first: &Event,
    last: &Event,
    last_digest: Option<EventHash>,
    tip_digest: Option<EventHash>,
) -> bool {
    first.hash_prev() == last_digest.unwrap_or(EventHash::GENESIS)
        && Some(last.digest()) == tip_digest
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

/// The servers the operator added to one product, folded from the product's
/// event log: AICD §17, AICD §26.
///
/// The module documentation says what the two events are and what one defective
/// event costs. In brief:
///
/// - **One registry per product**, held by the task that holds the product's
///   `ProductDb` (`spec/LLD.md` section 5, the single writer). Every
///   `mcp.server_*` event is appended through it.
/// - **Reading does not refresh**, and a removal takes effect for a decision only
///   when that decision reads the registry [`Registry::remove`] updated. There is
///   no `Clone`, so no snapshot of it can be handed out: slice S6 (later) is to
///   take the [`ExposedServer`] from the writer's registry at each call, and the
///   engine (later) is to answer `project.mcp.remove` only once no reader decides
///   on an older state.
/// - **A server's name is not its identity across removals**: a name may be
///   added again under a new id. Slice H2 (later) is to key connections and
///   caches by id, and to resolve a grant against the same registry state S6
///   authorized on.
/// - **`load` reads and verifies the whole log, and each `refresh`, `add` and
///   `remove` verifies it again**: O(log length) per call, the phase 1 cost,
///   stated and not optimized.
/// - **A refused event blocks the product for good**: see
///   [`Registry::blocked`] and the module documentation.
#[derive(Debug)]
pub struct Registry {
    product_id: Id,
    state: State,
    blocked: Option<(u64, EventDefect)>,
}

impl Registry {
    /// Reads and verifies the product's whole log and folds it (AICD §26,
    /// AICD §8).
    ///
    /// If the fold refuses an event, `load` returns that refusal and no
    /// registry: a restarted engine then has no registry to decide on, and
    /// exposes nothing.
    ///
    /// # Errors
    ///
    /// [`RegistryError::ProductNotAnId`] when the database's product identifier
    /// is not a ULID; otherwise what [`Registry::refresh`] answers.
    pub fn load(db: &mut ProductDb) -> Result<Registry, RegistryError> {
        let product_id = Id::parse(db.product_id()).map_err(|_| RegistryError::ProductNotAnId)?;
        let mut registry = Registry {
            product_id,
            state: State::empty(),
            blocked: None,
        };
        registry.refresh(db)?;
        Ok(registry)
    }

    /// Folds what was appended to the log since the registry last read it
    /// (AICD §26, AICD §8).
    ///
    /// The steps:
    ///
    /// 1. A blocked registry answers its recorded refusal and reads nothing: the
    ///    log is append-only and the fold deterministic, so the same event would
    ///    be refused again.
    /// 2. The database must be this registry's product.
    /// 3. A verified read, as `ori-broker` and `ori-memory` make before folding a
    ///    grant: `EventLog::verify` over the whole log. The registry compares
    ///    the tip with the position it folded: a tip below it, or an equal tip
    ///    with another digest, is [`RegistryError::LogMoved`]; an equal tip with
    ///    the same digest has nothing new; a higher tip is read with
    ///    `EventLog::read_range`, which must hold exactly the events between the
    ///    two, the first of which must chain from the last event folded and the
    ///    last of which must be the tip. So a `refresh` that answers `Ok` has
    ///    folded up to the tip.
    /// 4. The events are folded in order into a copy of the state. If every one
    ///    folds, the copy replaces the state; if one is refused, the state is
    ///    unchanged, the registry is blocked at that event, and the refusal is
    ///    answered.
    ///
    /// Only a defective event blocks. [`RegistryError::WrongProduct`],
    /// [`RegistryError::LogMoved`] and [`RegistryError::Log`] never do and change
    /// nothing.
    ///
    /// # Errors
    ///
    /// [`RegistryError::Event`] for a blocked registry or a refused event,
    /// [`RegistryError::WrongProduct`], [`RegistryError::LogMoved`], and
    /// [`RegistryError::Log`] for a failed or broken read.
    pub fn refresh(&mut self, db: &mut ProductDb) -> Result<(), RegistryError> {
        if let Some((seq, defect)) = &self.blocked {
            return Err(RegistryError::Event {
                seq: *seq,
                defect: defect.clone(),
            });
        }
        match Id::parse(db.product_id()) {
            Ok(id) if id == self.product_id => {}
            _ => return Err(RegistryError::WrongProduct),
        }
        let report = EventLog::verify(db.connection()).map_err(RegistryError::Log)?;
        let tip = report.tip_seq.unwrap_or(0);
        let last_seq = self.state.last_seq;
        let moved = RegistryError::LogMoved {
            folded_seq: last_seq,
        };
        if tip < last_seq || (tip == last_seq && report.tip_digest != self.state.last_digest) {
            return Err(moved);
        }
        if tip == last_seq {
            return Ok(());
        }
        let wanted = tip - last_seq;
        let events =
            EventLog::read_range(db.connection(), last_seq + 1, tip).map_err(RegistryError::Log)?;
        let read = u64::try_from(events.len()).unwrap_or(u64::MAX);
        if read != wanted {
            return Err(RegistryError::Log(EventLogError::ReaderIncomplete {
                counted: wanted,
                read,
            }));
        }
        let (Some(first), Some(last)) = (events.first(), events.last()) else {
            return Err(RegistryError::Log(EventLogError::ReaderIncomplete {
                counted: wanted,
                read,
            }));
        };
        if !chains_to_tip(first, last, self.state.last_digest, report.tip_digest) {
            return Err(moved);
        }
        let mut next = self.state.clone();
        for event in &events {
            if let Err(defect) = fold(&mut next, &self.product_id, event) {
                self.blocked = Some((event.seq(), defect.clone()));
                return Err(RegistryError::Event {
                    seq: event.seq(),
                    defect,
                });
            }
        }
        self.state = next;
        Ok(())
    }

    /// Records an addition the operator asked for and returns the server it
    /// recorded (AICD §17, AICD §26).
    ///
    /// The steps: [`Registry::refresh`]; the state checks (the id was never added,
    /// no live server has the name); write the payload; append `mcp.server_added`
    /// with `Actor::Human(operator)`, no ticket and `at`; apply the appended event
    /// through the function the fold uses. The returned server's
    /// [`RegisteredServer::added_seq`] is what `project.mcp.add` returns.
    ///
    /// If the apply refuses an event built from a checked [`NewServer`], which it
    /// cannot, the event is nonetheless in the log: the registry is blocked at it,
    /// the state is unchanged, and every later call and every later `load`
    /// refuses it the same way.
    ///
    /// # Errors
    ///
    /// What [`Registry::refresh`] answers; [`RegistryError::IdTaken`] and
    /// [`RegistryError::NameTaken`], with nothing appended;
    /// [`RegistryError::Encode`]; [`RegistryError::Log`] when the append fails,
    /// with the state unchanged; [`RegistryError::Event`] as above.
    pub fn add(
        &mut self,
        db: &mut ProductDb,
        at: Timestamp,
        operator: Id,
        server: NewServer,
    ) -> Result<&RegisteredServer, RegistryError> {
        self.refresh(db)?;
        check_state(&self.state, &server).map_err(|conflict| match conflict {
            Conflict::Id => RegistryError::IdTaken {
                id: server.id.clone(),
            },
            Conflict::Name => RegistryError::NameTaken {
                name: server.name.clone(),
            },
        })?;
        let payload = added_payload(&server).map_err(|_| RegistryError::Encode)?;
        let event = EventLog::append(
            db.connection(),
            self.product_id.clone(),
            at,
            Actor::Human(operator),
            SERVER_ADDED_KIND,
            None,
            payload,
        )
        .map_err(RegistryError::Log)?;
        match apply_added(&mut self.state, &self.product_id, &event) {
            Ok(registered) => Ok(registered),
            Err(defect) => {
                self.blocked = Some((event.seq(), defect.clone()));
                Err(RegistryError::Event {
                    seq: event.seq(),
                    defect,
                })
            }
        }
    }

    /// Records the removal of a live server and returns what was removed
    /// (AICD §17, AICD §26).
    ///
    /// The steps: [`Registry::refresh`]; a live server has the id; append
    /// `mcp.server_removed` with `Actor::Human(operator)`, no ticket and `at`;
    /// apply the appended event through the function the fold uses. The id stays
    /// taken, so the server's name may be added again but its id may not. The
    /// registry does not delete the server's keychain entry: that is the engine's
    /// and the broker's (later).
    ///
    /// # Errors
    ///
    /// What [`Registry::refresh`] answers; [`RegistryError::NotFound`], with
    /// nothing appended; [`RegistryError::Encode`]; [`RegistryError::Log`] when
    /// the append fails; [`RegistryError::Event`] as for [`Registry::add`].
    pub fn remove(
        &mut self,
        db: &mut ProductDb,
        at: Timestamp,
        operator: Id,
        id: &Id,
    ) -> Result<RemovedServer, RegistryError> {
        self.refresh(db)?;
        let Some(live) = self.by_id(id) else {
            return Err(RegistryError::NotFound { id: id.clone() });
        };
        let payload = removed_payload(live).map_err(|_| RegistryError::Encode)?;
        let event = EventLog::append(
            db.connection(),
            self.product_id.clone(),
            at,
            Actor::Human(operator),
            SERVER_REMOVED_KIND,
            None,
            payload,
        )
        .map_err(RegistryError::Log)?;
        match apply_removed(&mut self.state, &event) {
            Ok(server) => Ok(RemovedServer {
                server,
                removed_seq: event.seq(),
            }),
            Err(defect) => {
                self.blocked = Some((event.seq(), defect.clone()));
                Err(RegistryError::Event {
                    seq: event.seq(),
                    defect,
                })
            }
        }
    }

    /// The product this registry belongs to.
    #[must_use]
    pub fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The live servers, in name order. Does not touch the database, and keeps
    /// answering the last good state of a blocked registry, for the operator's
    /// view only.
    pub fn servers(&self) -> impl Iterator<Item = &RegisteredServer> {
        self.state.live.values()
    }

    /// The live server with this name. Does not touch the database, and keeps
    /// answering the last good state of a blocked registry.
    #[must_use]
    pub fn by_name(&self, name: &ServerName) -> Option<&RegisteredServer> {
        self.state.live.get(name.as_str())
    }

    /// The live server with this id: a removed server is not found. Does not
    /// touch the database, and keeps answering the last good state of a blocked
    /// registry.
    #[must_use]
    pub fn by_id(&self, id: &Id) -> Option<&RegisteredServer> {
        self.state.live.values().find(|server| server.id == *id)
    }

    /// The facts [`crate::scopes::ToolScopes::authorize_user_tool`] decides on
    /// for the live server with this name (AICD §17, AICD §26).
    ///
    /// `None` while the registry is blocked, whatever the name, so that no grant
    /// is decided on a state the log no longer supports; and `None` for a name no
    /// live server has, a removed server's included. The caller takes it from the
    /// registry at the moment of a decision and keeps none: code that builds an
    /// [`ExposedServer`] from anything else is outside what this crate can stop.
    #[must_use]
    pub fn exposed(&self, name: &ServerName) -> Option<ExposedServer> {
        if self.blocked.is_some() {
            return None;
        }
        self.state
            .live
            .get(name.as_str())
            .map(RegisteredServer::exposure)
    }

    /// The `seq` and the defect of the event the fold refused, if it refused one
    /// (AICD §8). The engine (later) is to show it.
    #[must_use]
    pub fn blocked(&self) -> Option<(u64, &EventDefect)> {
        self.blocked.as_ref().map(|(seq, defect)| (*seq, defect))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::scopes::{Caller, ToolRefusal, ToolScopes};

    // -------------------------------------------------------------------
    // Scratch directories and product databases, the self-cleaning pattern
    // `server.rs`'s tests use, with no dev-dependency.
    // -------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0194-{label}-{}-{unique}",
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

    /// A product database in a scratch directory. The database is the first
    /// field so that it closes before the directory goes.
    struct Home {
        db: ProductDb,
        scratch: Scratch,
    }

    impl Home {
        fn new(label: &str, product: &str) -> Self {
            let scratch = Scratch::new(label);
            let db = ProductDb::open(&scratch.path, product, ts(1))
                .expect("a fresh product database opens");
            Self { db, scratch }
        }

        /// Closes the database and opens the same directory again.
        fn reopen(self, product: &str) -> Self {
            let Self { db, scratch } = self;
            drop(db);
            let db = ProductDb::open(&scratch.path, product, ts(2))
                .expect("the product database reopens");
            Self { db, scratch }
        }
    }

    // -------------------------------------------------------------------
    // Values
    // -------------------------------------------------------------------

    const PRODUCT_A: &str = "01BX5ZZKBKACTAV9WEVGEMMVRY";
    const PRODUCT_B: &str = "01D39ZY06FGSCTVN4T2V9PKHFZ";
    /// The server id of the example in Decision 3 and the module documentation.
    const SERVER_EXAMPLE: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const MARKER: &str = "ORI0194MARKER";
    const AT: i64 = 1_700_000_000_000;

    fn ts(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    /// A valid identifier made of digits, distinguished by `n`.
    fn ulid(n: u64) -> Id {
        Id::parse(&format!("01{n:024}")).expect("digits are Crockford symbols")
    }

    fn id_text(n: u64) -> String {
        ulid(n).as_str().to_owned()
    }

    fn id_of(text: &str) -> Id {
        Id::parse(text).expect("a valid ULID")
    }

    fn product_a() -> Id {
        id_of(PRODUCT_A)
    }

    fn product_b() -> Id {
        id_of(PRODUCT_B)
    }

    fn operator() -> Id {
        ulid(9_001)
    }

    fn agent() -> Id {
        ulid(9_002)
    }

    fn name(text: &str) -> ServerName {
        ServerName::parse(text).expect("a valid server name")
    }

    fn config_of(command: &str, args: &[&str]) -> StdioConfig {
        StdioConfig::new(
            command.to_owned(),
            args.iter().map(|arg| (*arg).to_owned()).collect(),
        )
        .expect("a valid configuration")
    }

    /// An addition of `github-mcp --read-only` with the id `ulid(n)`.
    fn server(
        n: u64,
        server_name: &str,
        with_secret: bool,
        roles: &[Role],
        acknowledged: bool,
    ) -> NewServer {
        NewServer::new(
            ulid(n),
            name(server_name),
            Transport::Stdio,
            config_of("github-mcp", &["--read-only"]),
            with_secret,
            roles.to_vec(),
            acknowledged,
        )
        .expect("a valid addition")
    }

    fn loaded(home: &mut Home) -> Registry {
        Registry::load(&mut home.db).expect("the registry loads")
    }

    fn add_ok(registry: &mut Registry, home: &mut Home, new: NewServer) -> RegisteredServer {
        registry
            .add(&mut home.db, ts(AT), operator(), new)
            .expect("the server is added")
            .clone()
    }

    fn snapshot(registry: &Registry) -> Vec<RegisteredServer> {
        registry.servers().cloned().collect()
    }

    fn all_events(home: &mut Home) -> Vec<Event> {
        let report = EventLog::verify(home.db.connection()).expect("the log verifies");
        match report.tip_seq {
            Some(tip) => {
                EventLog::read_range(home.db.connection(), 1, tip).expect("the log reads back")
            }
            None => Vec::new(),
        }
    }

    fn event_count(home: &mut Home) -> usize {
        all_events(home).len()
    }

    /// Appends an event directly, around the registry.
    fn plant(home: &mut Home, product: &Id, actor: Actor, kind: &str, payload: &str) -> u64 {
        EventLog::append(
            home.db.connection(),
            product.clone(),
            ts(AT),
            actor,
            kind,
            None,
            payload,
        )
        .expect("the event is appended")
        .seq()
    }

    fn plant_human_added(home: &mut Home, n: u64, server_name: &str) -> u64 {
        plant(
            home,
            &product_a(),
            Actor::Human(operator()),
            SERVER_ADDED_KIND,
            &good_added(&id_text(n), server_name),
        )
    }

    // -------------------------------------------------------------------
    // Payload text, written by hand so that no test depends on the writer
    // -------------------------------------------------------------------

    /// An `mcp.server_added` payload. `command` goes between the quotes of a JSON
    /// string as written; `credential`, `roles` and `acknowledged` are JSON
    /// fragments.
    fn added_json(
        server_id: &str,
        server_name: &str,
        transport: &str,
        command: &str,
        credential: &str,
        roles: &str,
        acknowledged: &str,
    ) -> String {
        format!(
            r#"{{"server_id":"{server_id}","name":"{server_name}","transport":"{transport}","config":{{"command":"{command}","args":["--read-only"]}},"credential_ref":{credential},"exposed_to_roles":{roles},"acknowledged_reach":{acknowledged}}}"#
        )
    }

    /// The credential reference of a server, written as the JSON string the
    /// payload carries.
    fn derived(server_name: &str, server_id: &str) -> String {
        format!(
            "\"MCP_{}_{server_id}\"",
            server_name.to_ascii_uppercase().replace('-', "_")
        )
    }

    /// A valid `mcp.server_added` payload with the credential reference its name
    /// and id derive, exposed to `qa`, the reach acknowledged.
    fn good_added(server_id: &str, server_name: &str) -> String {
        added_json(
            server_id,
            server_name,
            "stdio",
            "github-mcp",
            &derived(server_name, server_id),
            r#"["qa"]"#,
            "true",
        )
    }

    fn removed_json(server_id: &str, server_name: &str) -> String {
        format!(r#"{{"server_id":"{server_id}","name":"{server_name}"}}"#)
    }

    /// `base` with the first `from` replaced by `to`, after checking `from` is
    /// there, so that a mutation of a payload can never be a silent no-op.
    fn changed(base: &str, from: &str, to: &str) -> String {
        assert!(base.contains(from), "the base payload holds {from}");
        base.replacen(from, to, 1)
    }

    // -------------------------------------------------------------------
    // 1 to 3: the registry and its two events
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0194_round_trip_a_fresh_load_agrees_with_what_add_built() {
        let mut home = Home::new("round-trip", PRODUCT_A);
        let mut registry = loaded(&mut home);
        assert_eq!(registry.servers().count(), 0);
        assert_eq!(registry.blocked(), None);

        let alpha = registry
            .add(
                &mut home.db,
                ts(AT + 1),
                operator(),
                server(1, "alpha", true, &[Role::Qa], true),
            )
            .expect("alpha is added")
            .clone();
        assert_eq!(alpha.added_at(), ts(AT + 1));
        assert_eq!(alpha.added_by(), &operator());
        assert_eq!(alpha.added_seq(), 1);
        assert_eq!(alpha.product_id(), &product_a());
        let beta = registry
            .add(
                &mut home.db,
                ts(AT + 2),
                operator(),
                server(2, "beta", false, &[Role::Lead], true),
            )
            .expect("beta is added")
            .clone();
        assert_eq!(beta.credential_ref(), None);
        let gamma = registry
            .add(
                &mut home.db,
                ts(AT + 3),
                operator(),
                server(3, "gamma", true, &[Role::Qa, Role::Coder], false),
            )
            .expect("gamma is added")
            .clone();
        assert!(!gamma.acknowledged_reach());
        assert_eq!(gamma.exposed_to_roles(), &[Role::Qa, Role::Coder]);
        let removed = registry
            .remove(&mut home.db, ts(AT + 4), operator(), beta.id())
            .expect("beta is removed");
        assert_eq!(removed.removed_seq(), 4);
        assert_eq!(removed.into_server(), beta);

        let kept = snapshot(&registry);
        assert_eq!(kept, vec![alpha, gamma]);
        drop(registry);

        let mut home = home.reopen(PRODUCT_A);
        let mut loaded_again = loaded(&mut home);
        assert_eq!(snapshot(&loaded_again), kept);
        assert_eq!(loaded_again.refresh(&mut home.db), Ok(()));
        assert_eq!(loaded_again.refresh(&mut home.db), Ok(()));
        assert_eq!(snapshot(&loaded_again), kept);
        assert_eq!(loaded_again.blocked(), None);
    }

    #[test]
    fn ori_t_0194_the_added_event_is_exactly_the_documented_payload() {
        let mut home = Home::new("added-event", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let github = NewServer::new(
            id_of(SERVER_EXAMPLE),
            name("github"),
            Transport::Stdio,
            config_of("github-mcp", &["--read-only"]),
            true,
            vec![Role::Qa],
            true,
        )
        .expect("a valid addition");
        let added = registry
            .add(&mut home.db, ts(AT), operator(), github)
            .expect("github is added")
            .clone();

        let events = all_events(&mut home);
        let last = events.last().expect("the addition is in the log");
        assert_eq!(last.kind(), "mcp.server_added");
        assert_eq!(SERVER_ADDED_KIND, "mcp.server_added");
        assert_eq!(last.actor(), &Actor::Human(operator()));
        assert_eq!(last.ticket_id(), None);
        assert_eq!(last.at(), ts(AT));
        assert_eq!(last.product_id(), &product_a());
        let expected = format!(
            r#"{{"server_id":"{SERVER_EXAMPLE}","name":"github","transport":"stdio","config":{{"command":"github-mcp","args":["--read-only"]}},"credential_ref":"MCP_GITHUB_{SERVER_EXAMPLE}","exposed_to_roles":["qa"],"acknowledged_reach":true}}"#
        );
        assert_eq!(last.payload(), expected);
        assert_eq!(added.added_seq(), last.seq());
        assert_eq!(
            added.credential_ref().map(CredentialRef::as_str),
            Some(format!("MCP_GITHUB_{SERVER_EXAMPLE}").as_str())
        );
    }

    #[test]
    fn ori_t_0194_the_added_event_writes_a_null_credential_reference_without_a_secret() {
        let mut home = Home::new("added-null", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let plain = NewServer::new(
            id_of(SERVER_EXAMPLE),
            name("github"),
            Transport::Stdio,
            config_of("github-mcp", &["--read-only"]),
            false,
            vec![Role::Qa],
            true,
        )
        .expect("a valid addition");
        let added = registry
            .add(&mut home.db, ts(AT), operator(), plain)
            .expect("github is added")
            .clone();

        let events = all_events(&mut home);
        let last = events.last().expect("the addition is in the log");
        let expected = format!(
            r#"{{"server_id":"{SERVER_EXAMPLE}","name":"github","transport":"stdio","config":{{"command":"github-mcp","args":["--read-only"]}},"credential_ref":null,"exposed_to_roles":["qa"],"acknowledged_reach":true}}"#
        );
        assert_eq!(last.payload(), expected);
        assert!(last.payload().contains(r#""credential_ref":null"#));
        assert_eq!(added.credential_ref(), None);
        assert_eq!(added.added_seq(), last.seq());
    }

    #[test]
    fn ori_t_0194_the_removed_event_is_exactly_the_documented_payload() {
        let mut home = Home::new("removed-event", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let github = NewServer::new(
            id_of(SERVER_EXAMPLE),
            name("github"),
            Transport::Stdio,
            config_of("github-mcp", &["--read-only"]),
            true,
            vec![Role::Qa],
            true,
        )
        .expect("a valid addition");
        let added = add_ok(&mut registry, &mut home, github);
        let removed = registry
            .remove(&mut home.db, ts(AT + 7), operator(), added.id())
            .expect("github is removed");

        let events = all_events(&mut home);
        let last = events.last().expect("the removal is in the log");
        assert_eq!(last.kind(), "mcp.server_removed");
        assert_eq!(SERVER_REMOVED_KIND, "mcp.server_removed");
        assert_eq!(last.actor(), &Actor::Human(operator()));
        assert_eq!(last.ticket_id(), None);
        assert_eq!(last.at(), ts(AT + 7));
        assert_eq!(
            last.payload(),
            format!(r#"{{"server_id":"{SERVER_EXAMPLE}","name":"github"}}"#)
        );
        assert_eq!(removed.removed_seq(), last.seq());
        assert_eq!(removed.server(), &added);
        assert_eq!(registry.servers().count(), 0);
        assert_eq!(registry.by_name(&name("github")), None);
        assert_eq!(registry.by_id(added.id()), None);
        assert_eq!(registry.exposed(&name("github")), None);
    }

    // -------------------------------------------------------------------
    // 4 to 6: names, ids, credential references, removal
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0194_a_live_name_and_every_id_ever_added_are_taken() {
        let mut home = Home::new("names", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let first = add_ok(
            &mut registry,
            &mut home,
            server(1, "github", true, &[Role::Qa], true),
        );
        assert_eq!(event_count(&mut home), 1);

        let same_name = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(2, "github", true, &[Role::Qa], true),
            )
            .err();
        assert_eq!(
            same_name,
            Some(RegistryError::NameTaken {
                name: name("github")
            })
        );
        assert_eq!(event_count(&mut home), 1);

        // The id is checked before the name: both are taken here.
        let both = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(1, "github", true, &[Role::Qa], true),
            )
            .err();
        assert_eq!(both, Some(RegistryError::IdTaken { id: ulid(1) }));

        registry
            .remove(&mut home.db, ts(AT), operator(), first.id())
            .expect("the first server is removed");
        let count_after_removal = event_count(&mut home);

        // The removal frees the name, under a new id.
        let second = add_ok(
            &mut registry,
            &mut home,
            server(2, "github", true, &[Role::Qa], true),
        );
        assert!(second.credential_ref().is_some());
        assert!(first.credential_ref().is_some());
        assert_ne!(second.credential_ref(), first.credential_ref());
        assert_eq!(event_count(&mut home), count_after_removal + 1);

        // The removed id is refused under another name.
        let removed_id = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(1, "other", true, &[Role::Qa], true),
            )
            .err();
        assert_eq!(removed_id, Some(RegistryError::IdTaken { id: ulid(1) }));
        // A live id is refused under another name.
        let live_id = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(2, "another", true, &[Role::Qa], true),
            )
            .err();
        assert_eq!(live_id, Some(RegistryError::IdTaken { id: ulid(2) }));
        assert_eq!(event_count(&mut home), count_after_removal + 1);
        assert_eq!(registry.blocked(), None);
    }

    #[test]
    fn ori_t_0194_credential_references_are_derived_from_name_and_id() {
        let id = id_of(SERVER_EXAMPLE);
        assert_eq!(
            CredentialRef::for_server(&name("github"), &id).as_str(),
            "MCP_GITHUB_01ARZ3NDEKTSV4RRFFQ69G5FAV"
        );
        assert_eq!(
            CredentialRef::for_server(&name("my-server"), &id).as_str(),
            format!("MCP_MY_SERVER_{SERVER_EXAMPLE}")
        );
        let longest = "a".repeat(32);
        let long_reference = CredentialRef::for_server(&name(&longest), &id);
        assert_eq!(long_reference.as_str().len(), 63);
        assert_eq!(
            long_reference.as_str(),
            format!("MCP_{}_{SERVER_EXAMPLE}", "A".repeat(32))
        );

        let mut home = Home::new("references", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let a = add_ok(&mut registry, &mut home, server(1, "a", true, &[], true));
        let a_b = add_ok(&mut registry, &mut home, server(2, "a-b", true, &[], true));
        let a_reference = a.credential_ref().expect("a has a reference").as_str();
        let a_b_reference = a_b.credential_ref().expect("a-b has a reference").as_str();
        assert_ne!(a_reference, a_b_reference);
        assert!(!a_reference.starts_with("MCP_A_B_"));
        assert!(a_b_reference.starts_with("MCP_A_B_"));
        assert_eq!(a_reference, format!("MCP_A_{}", id_text(1)));

        let without = NewServer::new(
            ulid(3),
            name("plain"),
            Transport::Stdio,
            config_of("plain-mcp", &[]),
            false,
            Vec::new(),
            false,
        )
        .expect("a valid addition");
        assert_eq!(without.credential_ref(), None);
    }

    #[test]
    fn ori_t_0194_remove_refuses_an_unknown_id_and_a_second_removal() {
        let mut home = Home::new("remove", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let github = add_ok(
            &mut registry,
            &mut home,
            server(1, "github", true, &[Role::Qa], true),
        );
        let count = event_count(&mut home);

        let unknown = registry
            .remove(&mut home.db, ts(AT), operator(), &ulid(77))
            .err();
        assert_eq!(unknown, Some(RegistryError::NotFound { id: ulid(77) }));
        assert_eq!(event_count(&mut home), count);

        registry
            .remove(&mut home.db, ts(AT), operator(), github.id())
            .expect("the first removal succeeds");
        let count_after = event_count(&mut home);
        assert_eq!(count_after, count + 1);
        let twice = registry
            .remove(&mut home.db, ts(AT), operator(), github.id())
            .err();
        assert_eq!(
            twice,
            Some(RegistryError::NotFound {
                id: github.id().clone()
            })
        );
        assert_eq!(event_count(&mut home), count_after);
        assert_eq!(registry.blocked(), None);
    }

    // -------------------------------------------------------------------
    // 7 to 10: the values of an addition
    // -------------------------------------------------------------------

    fn config_result(command: &str, args: &[&str]) -> Result<StdioConfig, ServerError> {
        StdioConfig::new(
            command.to_owned(),
            args.iter().map(|arg| (*arg).to_owned()).collect(),
        )
    }

    /// The unsafe characters the tests name.
    const UNSAFE: [char; 14] = [
        '\0', '\u{1b}', '\n', '\u{7f}', '\u{9b}', '\u{61c}', '\u{200e}', '\u{200f}', '\u{2028}',
        '\u{2029}', '\u{202a}', '\u{202e}', '\u{2066}', '\u{2069}',
    ];

    #[test]
    fn ori_t_0194_stdio_config_accepts_every_value_at_its_bound() {
        let command = "c".repeat(CONFIG_TEXT_MAX);
        assert!(config_result(&command, &[]).is_ok());
        let many = vec!["a"; ARGS_MAX];
        assert_eq!(many.len(), 64);
        assert!(config_result("c", &many).is_ok());
        let long_arg = "x".repeat(CONFIG_TEXT_MAX);
        assert!(config_result("c", &[long_arg.as_str()]).is_ok());
        let accepted = config_result("c", &["", "--flag", ""]).expect("empty arguments pass");
        assert_eq!(accepted.args(), &["", "--flag", ""]);
        assert_eq!(accepted.command(), "c");
        // Ordinary non-ASCII text and spaces are not unsafe.
        assert!(config_result("caf\u{e9} mcp", &["a b", "\u{e9}"]).is_ok());
        assert_eq!((CONFIG_TEXT_MAX, ARGS_MAX), (4096, 64));
    }

    #[test]
    fn ori_t_0194_stdio_config_refuses_each_value_past_its_bound() {
        let too_long = "c".repeat(CONFIG_TEXT_MAX + 1);
        assert_eq!(
            config_result(&too_long, &[]).err(),
            Some(ServerError::CommandTooLong { length: 4097 })
        );
        assert_eq!(
            config_result("", &[]).err(),
            Some(ServerError::CommandEmpty)
        );
        let sixty_five = vec!["a"; ARGS_MAX + 1];
        assert_eq!(
            config_result("c", &sixty_five).err(),
            Some(ServerError::TooManyArgs { count: 65 })
        );
        let long_arg = "x".repeat(CONFIG_TEXT_MAX + 1);
        assert_eq!(
            config_result("c", &["ok", long_arg.as_str()]).err(),
            Some(ServerError::ArgTooLong {
                index: 1,
                length: 4097
            })
        );
        assert_eq!(
            config_result("c", &["ok", "", "a\0b"]).err(),
            Some(ServerError::ArgHasControl { index: 2 })
        );
    }

    #[test]
    fn ori_t_0194_stdio_config_refuses_unsafe_characters_in_a_command_and_an_argument() {
        for ch in UNSAFE {
            let text = format!("a{ch}b");
            assert_eq!(
                config_result(&text, &[]).err(),
                Some(ServerError::CommandHasControl),
                "command with U+{:04X}",
                u32::from(ch)
            );
            assert_eq!(
                config_result("c", &[text.as_str()]).err(),
                Some(ServerError::ArgHasControl { index: 0 }),
                "argument 0 with U+{:04X}",
                u32::from(ch)
            );
            assert_eq!(
                config_result("c", &["ok", text.as_str()]).err(),
                Some(ServerError::ArgHasControl { index: 1 }),
                "argument 1 with U+{:04X}",
                u32::from(ch)
            );
        }
    }

    #[test]
    fn ori_t_0194_stdio_config_checks_in_the_documented_order() {
        let too_long_with_nul = format!("{}\0", "c".repeat(CONFIG_TEXT_MAX));
        assert_eq!(
            config_result(&too_long_with_nul, &[]).err(),
            Some(ServerError::CommandTooLong { length: 4097 })
        );
        assert_eq!(
            config_result("", &["a\0b"]).err(),
            Some(ServerError::CommandEmpty)
        );
        let sixty_five = vec!["a"; ARGS_MAX + 1];
        assert_eq!(
            config_result("a\0b", &sixty_five).err(),
            Some(ServerError::CommandHasControl)
        );
        let mut first_has_nul = vec!["a"; ARGS_MAX + 1];
        first_has_nul[0] = "a\0b";
        assert_eq!(
            config_result("c", &first_has_nul).err(),
            Some(ServerError::TooManyArgs { count: 65 })
        );
        let long_arg = "x".repeat(CONFIG_TEXT_MAX + 1);
        assert_eq!(
            config_result("c", &["a\0b", long_arg.as_str()]).err(),
            Some(ServerError::ArgHasControl { index: 0 })
        );
        let long_with_nul = format!("{}\0", "x".repeat(CONFIG_TEXT_MAX));
        assert_eq!(
            config_result("c", &[long_with_nul.as_str()]).err(),
            Some(ServerError::ArgTooLong {
                index: 0,
                length: 4097
            })
        );
    }

    #[test]
    fn ori_t_0194_stdio_config_debug_never_prints_an_argument() {
        let config = config_of("github-mcp", &["--token=ORI0194SECRET", "--other"]);
        let shown = format!("{config:?}");
        assert_eq!(
            shown,
            r#"StdioConfig { command: "github-mcp", args: 2 argument(s) }"#
        );
        assert!(!shown.contains("ORI0194SECRET"));
        assert!(!shown.contains("--other"));
        // The registered server's derived Debug reaches the same output.
        let mut home = Home::new("debug", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let new = NewServer::new(
            ulid(1),
            name("github"),
            Transport::Stdio,
            config,
            false,
            Vec::new(),
            false,
        )
        .expect("a valid addition");
        let registered = add_ok(&mut registry, &mut home, new);
        let shown = format!("{registered:?} {registry:?}");
        assert!(shown.contains("2 argument(s)"));
        assert!(!shown.contains("ORI0194SECRET"));
        assert!(!shown.contains("--other"));
    }

    #[test]
    fn ori_t_0194_roles_may_not_repeat_and_keep_the_operators_order() {
        let build = |roles: &[Role]| {
            NewServer::new(
                ulid(1),
                name("github"),
                Transport::Stdio,
                config_of("github-mcp", &[]),
                false,
                roles.to_vec(),
                true,
            )
        };
        assert_eq!(
            build(&[Role::Qa, Role::Qa]).err(),
            Some(ServerError::DuplicateRole { role: Role::Qa })
        );
        assert_eq!(
            build(&[Role::Coder, Role::Qa, Role::Coder]).err(),
            Some(ServerError::DuplicateRole { role: Role::Coder })
        );
        assert_eq!(
            build(&[Role::Coder, Role::Qa, Role::Qa, Role::Coder]).err(),
            Some(ServerError::DuplicateRole { role: Role::Qa })
        );
        assert_eq!(
            build(&[])
                .expect("an empty list is accepted")
                .exposed_to_roles(),
            &[]
        );
        let reversed: Vec<Role> = Role::ALL.iter().rev().copied().collect();
        assert_eq!(reversed.len(), 7);
        let all = build(&reversed).expect("all seven roles are accepted");
        assert_eq!(all.exposed_to_roles(), reversed.as_slice());
    }

    #[test]
    fn ori_t_0194_only_the_stdio_transport_parses() {
        assert_eq!(Transport::parse("stdio"), Ok(Transport::Stdio));
        assert_eq!(Transport::Stdio.as_str(), "stdio");
        for refused in ["STDIO", "http", "sse", "", "stdio ", " stdio", "Stdio"] {
            assert_eq!(
                Transport::parse(refused),
                Err(ServerError::TransportNotSupported),
                "{refused:?}"
            );
        }
    }

    // -------------------------------------------------------------------
    // 11 to 12: the fold refuses, and says where; and it skips what is not its own
    // -------------------------------------------------------------------

    struct Planted {
        actor: Actor,
        kind: &'static str,
        payload: String,
    }

    fn human(kind: &'static str, payload: String) -> Planted {
        Planted {
            actor: Actor::Human(operator()),
            kind,
            payload,
        }
    }

    fn added(payload: String) -> Planted {
        human(SERVER_ADDED_KIND, payload)
    }

    fn removed(payload: String) -> Planted {
        human(SERVER_REMOVED_KIND, payload)
    }

    fn by(actor: Actor, kind: &'static str, payload: String) -> Planted {
        Planted {
            actor,
            kind,
            payload,
        }
    }

    fn github_added() -> Planted {
        added(good_added(SERVER_EXAMPLE, "github"))
    }

    /// Plants every event in a fresh database of product A and loads: the answer
    /// must be a refusal of the last event, with this defect.
    fn assert_refused_at_last(label: &str, events: &[Planted], defect: EventDefect) {
        let mut home = Home::new("refused", PRODUCT_A);
        let product = product_a();
        for planted in events {
            plant(
                &mut home,
                &product,
                planted.actor.clone(),
                planted.kind,
                &planted.payload,
            );
        }
        let seq = u64::try_from(events.len()).expect("a small count");
        let outcome = Registry::load(&mut home.db);
        assert_eq!(
            outcome.err(),
            Some(RegistryError::Event { seq, defect }),
            "{label}"
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_an_actor_that_is_not_a_human() {
        assert_refused_at_last(
            "an agent's addition",
            &[by(
                Actor::Agent(agent()),
                SERVER_ADDED_KIND,
                good_added(SERVER_EXAMPLE, "github"),
            )],
            EventDefect::NotTheOperator,
        );
        assert_refused_at_last(
            "the system's addition",
            &[by(
                Actor::System,
                SERVER_ADDED_KIND,
                good_added(SERVER_EXAMPLE, "github"),
            )],
            EventDefect::NotTheOperator,
        );
        assert_refused_at_last(
            "an agent's removal",
            &[
                github_added(),
                by(
                    Actor::Agent(agent()),
                    SERVER_REMOVED_KIND,
                    removed_json(SERVER_EXAMPLE, "github"),
                ),
            ],
            EventDefect::NotTheOperator,
        );
        assert_refused_at_last(
            "the system's removal",
            &[
                github_added(),
                by(
                    Actor::System,
                    SERVER_REMOVED_KIND,
                    removed_json(SERVER_EXAMPLE, "github"),
                ),
            ],
            EventDefect::NotTheOperator,
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_payload_that_does_not_read() {
        let base = good_added(SERVER_EXAMPLE, "github");
        let array_of_seven = format!(
            r#"["{SERVER_EXAMPLE}","github","stdio",{{"command":"github-mcp","args":["--read-only"]}},"MCP_GITHUB_{SERVER_EXAMPLE}",["qa"],true]"#
        );
        let planted: Vec<(&str, String)> = vec![
            ("not json", "not json".to_owned()),
            ("the seven values as an array", array_of_seven),
            (
                "config as an array",
                changed(
                    &base,
                    r#"{"command":"github-mcp","args":["--read-only"]}"#,
                    r#"["github-mcp",["--read-only"]]"#,
                ),
            ),
            (
                "an unknown key at the top",
                changed(
                    &base,
                    r#""acknowledged_reach":true}"#,
                    r#""acknowledged_reach":true,"extra":1}"#,
                ),
            ),
            (
                "an unknown key inside config",
                changed(
                    &base,
                    r#""args":["--read-only"]}"#,
                    r#""args":["--read-only"],"env":{}}"#,
                ),
            ),
            (
                "a key twice at the top",
                changed(
                    &base,
                    r#""name":"github","#,
                    r#""name":"github","name":"github","#,
                ),
            ),
            (
                "a key twice inside config",
                changed(
                    &base,
                    r#""command":"github-mcp","#,
                    r#""command":"github-mcp","command":"github-mcp","#,
                ),
            ),
            (
                "config twice, the first an array",
                changed(&base, r#""config":{"#, r#""config":[],"config":{"#),
            ),
            (
                "acknowledged_reach missing",
                changed(&base, r#","acknowledged_reach":true"#, ""),
            ),
            (
                "acknowledged_reach as a string",
                changed(
                    &base,
                    r#""acknowledged_reach":true"#,
                    r#""acknowledged_reach":"true""#,
                ),
            ),
            ("text after the object", format!("{base} x")),
            (
                "config missing",
                changed(
                    &base,
                    r#""config":{"command":"github-mcp","args":["--read-only"]},"#,
                    "",
                ),
            ),
            (
                "credential_ref of the wrong type",
                changed(
                    &base,
                    &format!(r#""credential_ref":"MCP_GITHUB_{SERVER_EXAMPLE}""#),
                    r#""credential_ref":5"#,
                ),
            ),
        ];
        for (label, payload) in planted {
            assert_refused_at_last(label, &[added(payload)], EventDefect::Payload);
        }

        let removal_array = format!(r#"["{SERVER_EXAMPLE}","github"]"#);
        let removal_unknown = changed(
            &removed_json(SERVER_EXAMPLE, "github"),
            r#""name":"github"}"#,
            r#""name":"github","extra":1}"#,
        );
        let removal_missing = format!(r#"{{"server_id":"{SERVER_EXAMPLE}"}}"#);
        let removal_twice = format!(
            r#"{{"server_id":"{SERVER_EXAMPLE}","server_id":"{SERVER_EXAMPLE}","name":"github"}}"#
        );
        for (label, payload) in [
            ("a removal as an array", removal_array),
            ("a removal with an unknown key", removal_unknown),
            ("a removal without a name", removal_missing),
            ("a removal with a key twice", removal_twice),
            ("a removal that is not JSON", "not json".to_owned()),
        ] {
            assert_refused_at_last(
                label,
                &[github_added(), removed(payload)],
                EventDefect::Payload,
            );
        }
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_server_id_that_is_not_a_ulid() {
        let payload = good_added("not-a-ulid", "github");
        assert_refused_at_last("a bad id", &[added(payload)], EventDefect::ServerId);
        let removal = removed_json("not-a-ulid", "github");
        assert_refused_at_last(
            "a bad id in a removal",
            &[github_added(), removed(removal)],
            EventDefect::ServerId,
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_name_that_breaks_the_rule() {
        assert_refused_at_last(
            "a capital first letter",
            &[added(good_added(SERVER_EXAMPLE, "Aicd-x"))],
            EventDefect::Name(ServerNameError::FirstCharacter),
        );
        assert_refused_at_last(
            "the reserved prefix",
            &[added(good_added(SERVER_EXAMPLE, "aicdx"))],
            EventDefect::Name(ServerNameError::ReservedPrefix),
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_transport_or_a_command_that_breaks_a_rule() {
        let http = added_json(
            SERVER_EXAMPLE,
            "github",
            "http",
            "github-mcp",
            &derived("github", SERVER_EXAMPLE),
            r#"["qa"]"#,
            "true",
        );
        assert_refused_at_last(
            "http",
            &[added(http)],
            EventDefect::Server(ServerError::TransportNotSupported),
        );
        // A raw string holds the six characters backslash, u, 0, 0, 0, 0, which
        // JSON decodes to a NUL; Rust's own NUL escape would put a raw NUL byte in
        // the text, and serde_json would refuse the text as a whole instead.
        let nul = added_json(
            SERVER_EXAMPLE,
            "github",
            "stdio",
            r"gh\u0000mcp",
            &derived("github", SERVER_EXAMPLE),
            r#"["qa"]"#,
            "true",
        );
        assert!(!nul.contains('\0'));
        assert!(nul.contains(r"\u0000"));
        assert_refused_at_last(
            "a NUL in the command",
            &[added(nul)],
            EventDefect::Server(ServerError::CommandHasControl),
        );
        let esc = added_json(
            SERVER_EXAMPLE,
            "github",
            "stdio",
            r"gh\u001bmcp",
            &derived("github", SERVER_EXAMPLE),
            r#"["qa"]"#,
            "true",
        );
        assert!(!esc.contains('\u{1b}'));
        assert!(esc.contains(r"\u001b"));
        assert_refused_at_last(
            "an ESC in the command",
            &[added(esc)],
            EventDefect::Server(ServerError::CommandHasControl),
        );
        let empty = added_json(
            SERVER_EXAMPLE,
            "github",
            "stdio",
            "",
            &derived("github", SERVER_EXAMPLE),
            r#"["qa"]"#,
            "true",
        );
        assert_refused_at_last(
            "an empty command",
            &[added(empty)],
            EventDefect::Server(ServerError::CommandEmpty),
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_roles_that_are_unknown_or_repeated() {
        let credential = derived("github", SERVER_EXAMPLE);
        let with_roles = |roles: &str| {
            added_json(
                SERVER_EXAMPLE,
                "github",
                "stdio",
                "github-mcp",
                &credential,
                roles,
                "true",
            )
        };
        assert_refused_at_last(
            "a role that does not exist",
            &[added(with_roles(r#"["admin"]"#))],
            EventDefect::Role,
        );
        assert_refused_at_last(
            "a role in the wrong case",
            &[added(with_roles(r#"["QA"]"#))],
            EventDefect::Role,
        );
        assert_refused_at_last(
            "a role twice",
            &[added(with_roles(r#"["coder","qa","qa"]"#))],
            EventDefect::Server(ServerError::DuplicateRole { role: Role::Qa }),
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_credential_reference_the_server_does_not_derive() {
        let with_credential = |credential: &str| {
            added_json(
                SERVER_EXAMPLE,
                "github",
                "stdio",
                "github-mcp",
                credential,
                r#"["qa"]"#,
                "true",
            )
        };
        assert_refused_at_last(
            "another server's name",
            &[added(with_credential(
                r#""MCP_OTHER_01ARZ3NDEKTSV4RRFFQ69G5FAV""#,
            ))],
            EventDefect::CredentialRef,
        );
        assert_refused_at_last(
            "a free text",
            &[added(with_credential(r#""mcp_x""#))],
            EventDefect::CredentialRef,
        );
        assert_refused_at_last(
            "the right reference for another id",
            &[added(with_credential(&derived("github", &id_text(2))))],
            EventDefect::CredentialRef,
        );
        assert_refused_at_last(
            "an empty reference",
            &[added(with_credential(r#""""#))],
            EventDefect::CredentialRef,
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_taken_name_and_a_taken_id() {
        assert_refused_at_last(
            "a second live server with the name",
            &[github_added(), added(good_added(&id_text(2), "github"))],
            EventDefect::NameTaken,
        );
        assert_refused_at_last(
            "a live id under another name",
            &[github_added(), added(good_added(SERVER_EXAMPLE, "other"))],
            EventDefect::IdTaken,
        );
        assert_refused_at_last(
            "a removed id under another name",
            &[
                github_added(),
                removed(removed_json(SERVER_EXAMPLE, "github")),
                added(good_added(SERVER_EXAMPLE, "other")),
            ],
            EventDefect::IdTaken,
        );
        assert_refused_at_last(
            "a removed id under its own name",
            &[
                github_added(),
                removed(removed_json(SERVER_EXAMPLE, "github")),
                added(good_added(SERVER_EXAMPLE, "github")),
            ],
            EventDefect::IdTaken,
        );
    }

    #[test]
    fn ori_t_0194_the_fold_refuses_a_removal_of_a_server_that_is_not_live_or_is_misnamed() {
        assert_refused_at_last(
            "an id never added",
            &[removed(removed_json(&id_text(5), "github"))],
            EventDefect::NotLive,
        );
        assert_refused_at_last(
            "an id already removed",
            &[
                github_added(),
                removed(removed_json(SERVER_EXAMPLE, "github")),
                removed(removed_json(SERVER_EXAMPLE, "github")),
            ],
            EventDefect::NotLive,
        );
        assert_refused_at_last(
            "a live id under another name",
            &[
                github_added(),
                removed(removed_json(SERVER_EXAMPLE, "other")),
            ],
            EventDefect::NameMismatch,
        );
    }

    #[test]
    fn ori_t_0194_the_fold_skips_every_other_kind_and_accepts_what_it_should() {
        let mut home = Home::new("skips", PRODUCT_A);
        let product = product_a();
        plant_human_added(&mut home, 1, "alpha");
        for kind in [
            "mcp.tool_called",
            "mcp.server_addedx",
            "MCP.SERVER_ADDED",
            "ticket.created",
        ] {
            plant(&mut home, &product, Actor::Agent(agent()), kind, "not json");
        }
        // A skipped kind is skipped whoever its actor is.
        plant(
            &mut home,
            &product,
            Actor::System,
            "mcp.tool_refused",
            "not json",
        );
        // A valid addition whose payload omits `credential_ref`.
        let omitted = changed(
            &added_json(
                &id_text(2),
                "beta",
                "stdio",
                "github-mcp",
                "null",
                r#"["qa"]"#,
                "true",
            ),
            r#""credential_ref":null,"#,
            "",
        );
        assert!(!omitted.contains("credential_ref"));
        plant(
            &mut home,
            &product,
            Actor::Human(operator()),
            SERVER_ADDED_KIND,
            &omitted,
        );
        // A valid addition with whitespace around its object.
        let spaced = format!(" \n\t{} \r\n", good_added(&id_text(3), "gamma"));
        plant(
            &mut home,
            &product,
            Actor::Human(operator()),
            SERVER_ADDED_KIND,
            &spaced,
        );

        let mut registry = loaded(&mut home);
        let names: Vec<&str> = registry.servers().map(|s| s.name().as_str()).collect();
        assert_eq!(names, ["alpha", "beta", "gamma"]);
        let beta = registry.by_name(&name("beta")).expect("beta is live");
        assert_eq!(beta.credential_ref(), None);
        assert_eq!(beta.added_seq(), 7);
        let gamma = registry.by_name(&name("gamma")).expect("gamma is live");
        assert_eq!(gamma.added_seq(), 8);
        assert_eq!(registry.blocked(), None);
        // The registry is at the tip: an addition takes the next seq.
        let delta = add_ok(
            &mut registry,
            &mut home,
            server(4, "delta", false, &[], false),
        );
        assert_eq!(delta.added_seq(), 9);
    }

    // -------------------------------------------------------------------
    // 13 to 17: catching up, refusing for good, the wrong database
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0194_add_and_refresh_catch_up_with_the_log_before_they_check() {
        let mut home = Home::new("catch-up", PRODUCT_A);
        let mut registry = loaded(&mut home);
        assert_eq!(plant_human_added(&mut home, 1, "github"), 1);

        let refused = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(2, "github", false, &[Role::Qa], true),
            )
            .err();
        assert_eq!(
            refused,
            Some(RegistryError::NameTaken {
                name: name("github")
            })
        );
        // The refusal came after the catch-up: the planted server is now known.
        assert!(registry.by_name(&name("github")).is_some());
        assert_eq!(event_count(&mut home), 1);

        let mut second = Home::new("catch-up-refresh", PRODUCT_A);
        let mut other = loaded(&mut second);
        assert_eq!(other.servers().count(), 0);
        plant_human_added(&mut second, 1, "github");
        assert_eq!(other.servers().count(), 0, "reading does not refresh");
        assert_eq!(other.refresh(&mut second.db), Ok(()));
        let planted = other.by_name(&name("github")).expect("refresh folded it");
        assert_eq!(planted.added_seq(), 1);
        assert_eq!(planted.added_by(), &operator());
        assert_eq!(planted.product_id(), &product_a());
    }

    #[test]
    fn ori_t_0194_a_refused_event_changes_nothing_and_blocks_exposure() {
        let mut home = Home::new("blocked", PRODUCT_A);
        let mut registry = loaded(&mut home);
        add_ok(
            &mut registry,
            &mut home,
            server(1, "alpha", false, &[Role::Qa], true),
        );
        let alpha = name("alpha");
        assert!(registry.exposed(&alpha).is_some());
        let before = snapshot(&registry);
        assert_eq!(before.len(), 1);

        let product = product_a();
        plant_human_added(&mut home, 2, "beta");
        plant(
            &mut home,
            &product,
            Actor::Human(operator()),
            SERVER_REMOVED_KIND,
            &removed_json(&id_text(1), "alpha"),
        );
        let gamma_seq = plant(
            &mut home,
            &product,
            Actor::Agent(agent()),
            SERVER_ADDED_KIND,
            &good_added(&id_text(3), "gamma"),
        );
        let count = event_count(&mut home);
        let refused = RegistryError::Event {
            seq: gamma_seq,
            defect: EventDefect::NotTheOperator,
        };

        assert_eq!(registry.refresh(&mut home.db), Err(refused.clone()));
        assert_eq!(snapshot(&registry), before);
        assert_eq!(registry.by_name(&name("beta")), None);
        assert_eq!(
            registry.blocked(),
            Some((gamma_seq, &EventDefect::NotTheOperator))
        );
        assert_eq!(registry.exposed(&alpha), None);
        assert_eq!(registry.exposed(&name("beta")), None);

        let removal = registry
            .remove(&mut home.db, ts(AT), operator(), &ulid(1))
            .err();
        assert_eq!(removal, Some(refused.clone()));
        let addition = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(4, "delta", false, &[], false),
            )
            .err();
        assert_eq!(addition, Some(refused.clone()));
        assert_eq!(event_count(&mut home), count);

        assert_eq!(registry.refresh(&mut home.db), Err(refused.clone()));
        assert_eq!(
            registry.blocked(),
            Some((gamma_seq, &EventDefect::NotTheOperator))
        );
        assert_eq!(Registry::load(&mut home.db).err(), Some(refused));
    }

    #[test]
    fn ori_t_0194_a_database_of_another_product_is_refused_and_blocks_nothing() {
        let mut home_a = Home::new("wrong-a", PRODUCT_A);
        let mut home_b = Home::new("wrong-b", PRODUCT_B);
        let mut registry = loaded(&mut home_a);

        let refused = registry
            .add(
                &mut home_b.db,
                ts(AT),
                operator(),
                server(1, "github", false, &[], false),
            )
            .err();
        assert_eq!(refused, Some(RegistryError::WrongProduct));
        assert_eq!(event_count(&mut home_b), 0);
        assert_eq!(registry.blocked(), None);
        assert_eq!(registry.servers().count(), 0);

        let mut odd = Home::new("odd", "PRODUCT-A");
        assert_eq!(
            Registry::load(&mut odd.db).err(),
            Some(RegistryError::ProductNotAnId)
        );
        assert_eq!(
            registry.refresh(&mut odd.db),
            Err(RegistryError::WrongProduct)
        );
        assert_eq!(registry.blocked(), None);

        // The registry still serves its own product.
        let github = add_ok(
            &mut registry,
            &mut home_a,
            server(2, "github", false, &[], false),
        );
        assert_eq!(github.added_seq(), 1);
        assert_eq!(registry.blocked(), None);
    }

    #[test]
    fn ori_t_0194_an_event_of_another_product_is_refused_whatever_its_kind() {
        let mut home = Home::new("foreign-ticket", PRODUCT_A);
        let seq = plant(
            &mut home,
            &product_b(),
            Actor::Human(operator()),
            "ticket.created",
            "{}",
        );
        assert_eq!(seq, 1);
        assert_eq!(
            Registry::load(&mut home.db).err(),
            Some(RegistryError::Event {
                seq: 1,
                defect: EventDefect::ForeignProduct
            })
        );

        let mut second = Home::new("foreign-added", PRODUCT_A);
        plant(
            &mut second,
            &product_b(),
            Actor::Human(operator()),
            SERVER_ADDED_KIND,
            &good_added(SERVER_EXAMPLE, "github"),
        );
        assert_eq!(
            Registry::load(&mut second.db).err(),
            Some(RegistryError::Event {
                seq: 1,
                defect: EventDefect::ForeignProduct
            })
        );
    }

    #[test]
    fn ori_t_0194_a_log_that_is_not_the_one_folded_is_refused_and_blocks_nothing() {
        let mut first = Home::new("moved-first", PRODUCT_A);
        let mut registry = loaded(&mut first);
        add_ok(
            &mut registry,
            &mut first,
            server(1, "alpha", false, &[Role::Qa], true),
        );
        let before = snapshot(&registry);
        let moved = RegistryError::LogMoved { folded_seq: 1 };

        // An empty log of the same product.
        let mut empty = Home::new("moved-empty", PRODUCT_A);
        assert_eq!(registry.refresh(&mut empty.db), Err(moved.clone()));

        // A log of the same length, with another server in it.
        let mut same_length = Home::new("moved-same", PRODUCT_A);
        let mut other = loaded(&mut same_length);
        add_ok(
            &mut other,
            &mut same_length,
            server(2, "beta", false, &[Role::Qa], true),
        );
        assert_eq!(registry.refresh(&mut same_length.db), Err(moved.clone()));

        // A longer log whose first unread event does not chain from ours.
        add_ok(
            &mut other,
            &mut same_length,
            server(3, "gamma", false, &[Role::Qa], true),
        );
        assert_eq!(event_count(&mut same_length), 2);
        assert_eq!(registry.refresh(&mut same_length.db), Err(moved));

        assert_eq!(registry.blocked(), None);
        assert_eq!(snapshot(&registry), before);
        assert!(registry.exposed(&name("alpha")).is_some());
        // Its own log still works.
        assert_eq!(registry.refresh(&mut first.db), Ok(()));
        assert_eq!(snapshot(&registry), before);
    }

    #[test]
    fn ori_t_0194_a_broken_log_is_a_failure_that_blocks_nothing() {
        let mut home = Home::new("tampered", PRODUCT_A);
        let mut registry = loaded(&mut home);
        add_ok(
            &mut registry,
            &mut home,
            server(1, "alpha", false, &[Role::Qa], true),
        );
        add_ok(
            &mut registry,
            &mut home,
            server(2, "beta", false, &[Role::Qa], true),
        );
        let before = snapshot(&registry);
        // Rewriting a row is outside what the log's triggers allow; a test of
        // what the registry does when it happens drops the trigger first.
        home.db
            .connection()
            .execute_batch(
                "DROP TRIGGER events_no_update; \
                 UPDATE events SET payload = 'tampered' WHERE seq = 1;",
            )
            .expect("the scratch log is tampered with");

        let refreshed = registry.refresh(&mut home.db);
        assert!(
            matches!(
                refreshed,
                Err(RegistryError::Log(EventLogError::ChainBroken { .. }))
            ),
            "{refreshed:?}"
        );
        assert_eq!(registry.blocked(), None);
        assert_eq!(snapshot(&registry), before);
        assert!(registry.exposed(&name("alpha")).is_some());
        let added = registry
            .add(
                &mut home.db,
                ts(AT),
                operator(),
                server(3, "gamma", false, &[], false),
            )
            .err();
        assert!(matches!(
            added,
            Some(RegistryError::Log(EventLogError::ChainBroken { .. }))
        ));
        // A restarted engine's load answers the same failure and exposes nothing.
        assert!(matches!(
            Registry::load(&mut home.db).err(),
            Some(RegistryError::Log(EventLogError::ChainBroken { .. }))
        ));
    }

    // -------------------------------------------------------------------
    // 18: the registry's half of the proxy's decision
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0194_exposure_reaches_the_scopes_decision() {
        let mut home = Home::new("scopes", PRODUCT_A);
        let mut registry = loaded(&mut home);
        let github = add_ok(
            &mut registry,
            &mut home,
            server(1, "github", true, &[Role::Qa], true),
        );
        let exposed = registry
            .exposed(&name("github"))
            .expect("a live server is exposed");
        assert_eq!(exposed.product_id(), &product_a());
        assert_eq!(exposed.name(), &name("github"));
        assert_eq!(exposed.exposed_to_roles(), &[Role::Qa]);
        assert!(exposed.acknowledged_reach());

        let qa_of_a = Caller::new(agent(), Role::Qa, product_a());
        let grant = ToolScopes::authorize_user_tool(&qa_of_a, &exposed)
            .expect("qa of product A is granted");
        assert_eq!(grant.role(), Role::Qa);
        assert_eq!(grant.server(), &name("github"));
        assert_eq!(grant.product_id(), &product_a());

        let coder_of_a = Caller::new(agent(), Role::Coder, product_a());
        assert_eq!(
            ToolScopes::authorize_user_tool(&coder_of_a, &exposed).err(),
            Some(ToolRefusal::ServerNotExposedToRole {
                role: Role::Coder,
                server: name("github")
            })
        );
        let qa_of_b = Caller::new(agent(), Role::Qa, product_b());
        assert_eq!(
            ToolScopes::authorize_user_tool(&qa_of_b, &exposed).err(),
            Some(ToolRefusal::ServerNotInProject {
                server: name("github")
            })
        );

        // Roles listed without the acknowledgement take effect for nobody.
        add_ok(
            &mut registry,
            &mut home,
            server(2, "slack", false, &[Role::Qa, Role::Coder], false),
        );
        let slack = registry.exposed(&name("slack")).expect("slack is live");
        assert_eq!(slack.exposed_to_roles(), &[Role::Qa, Role::Coder]);
        assert!(!slack.acknowledged_reach());
        for caller in [&qa_of_a, &coder_of_a] {
            assert_eq!(
                ToolScopes::authorize_user_tool(caller, &slack).err(),
                Some(ToolRefusal::ServerReachNotAcknowledged {
                    server: name("slack")
                })
            );
        }

        // A removed server is exposed to nobody, and so is a name never added.
        registry
            .remove(&mut home.db, ts(AT), operator(), github.id())
            .expect("github is removed");
        assert_eq!(registry.exposed(&name("github")), None);
        assert_eq!(registry.exposed(&name("never-added")), None);
    }

    // -------------------------------------------------------------------
    // 19 to 21: the sections, the messages, the order
    // -------------------------------------------------------------------

    fn section_of(reference: MethodologyRef) -> u8 {
        assert!(reference.resolves(), "{reference:?} resolves");
        assert_eq!(reference.subsection, None);
        reference.section
    }

    /// Decision 6's table for `ServerError`, as an exhaustive `match`, so that a
    /// new variant fails to compile until its section is decided here too.
    fn server_error_section(error: &ServerError) -> u8 {
        match error {
            ServerError::TransportNotSupported
            | ServerError::CommandEmpty
            | ServerError::CommandTooLong { .. }
            | ServerError::CommandHasControl
            | ServerError::TooManyArgs { .. }
            | ServerError::ArgTooLong { .. }
            | ServerError::ArgHasControl { .. } => 27,
            ServerError::DuplicateRole { .. } => 17,
        }
    }

    fn every_server_error() -> Vec<ServerError> {
        vec![
            ServerError::TransportNotSupported,
            ServerError::CommandEmpty,
            ServerError::CommandTooLong { length: 4097 },
            ServerError::CommandHasControl,
            ServerError::TooManyArgs { count: 65 },
            ServerError::ArgTooLong {
                index: 1,
                length: 4097,
            },
            ServerError::ArgHasControl { index: 1 },
            ServerError::DuplicateRole { role: Role::Qa },
        ]
    }

    /// Decision 6's table for `EventDefect`.
    fn event_defect_section(defect: &EventDefect) -> u8 {
        match defect {
            EventDefect::NotTheOperator => 17,
            EventDefect::ForeignProduct => 26,
            EventDefect::CredentialRef => 27,
            EventDefect::Payload
            | EventDefect::ServerId
            | EventDefect::Role
            | EventDefect::NotLive
            | EventDefect::NameMismatch
            | EventDefect::IdTaken
            | EventDefect::NameTaken => 8,
            EventDefect::Name(error) => error.methodology_ref().section,
            EventDefect::Server(error) => error.methodology_ref().section,
        }
    }

    fn every_event_defect() -> Vec<EventDefect> {
        vec![
            EventDefect::NotTheOperator,
            EventDefect::ForeignProduct,
            EventDefect::Payload,
            EventDefect::ServerId,
            EventDefect::Name(ServerNameError::ReservedPrefix),
            EventDefect::Server(ServerError::CommandEmpty),
            EventDefect::Server(ServerError::DuplicateRole { role: Role::Qa }),
            EventDefect::Role,
            EventDefect::CredentialRef,
            EventDefect::IdTaken,
            EventDefect::NameTaken,
            EventDefect::NotLive,
            EventDefect::NameMismatch,
        ]
    }

    #[test]
    fn ori_t_0194_every_refusal_carries_the_section_decision_6_gives_it() {
        for error in every_server_error() {
            assert_eq!(
                section_of(error.methodology_ref()),
                server_error_section(&error),
                "{error:?}"
            );
        }
        for defect in every_event_defect() {
            assert_eq!(
                section_of(defect.methodology_ref()),
                event_defect_section(&defect),
                "{defect:?}"
            );
        }
        let name_error = ServerNameError::Empty;
        assert_eq!(
            EventDefect::Name(name_error).methodology_ref(),
            name_error.methodology_ref()
        );
        let server_error = ServerError::DuplicateRole { role: Role::Lead };
        assert_eq!(
            EventDefect::Server(server_error).methodology_ref(),
            server_error.methodology_ref()
        );

        let defect = EventDefect::ForeignProduct;
        let cases: Vec<(RegistryError, Option<u8>, bool)> = vec![
            (RegistryError::ProductNotAnId, Some(26), true),
            (RegistryError::WrongProduct, Some(26), true),
            (RegistryError::IdTaken { id: ulid(1) }, Some(13), true),
            (
                RegistryError::NameTaken {
                    name: name("github"),
                },
                Some(27),
                true,
            ),
            (RegistryError::NotFound { id: ulid(1) }, Some(17), true),
            (
                RegistryError::Event {
                    seq: 3,
                    defect: defect.clone(),
                },
                Some(26),
                true,
            ),
            (RegistryError::LogMoved { folded_seq: 3 }, Some(8), true),
            (RegistryError::Encode, None, false),
            (
                RegistryError::Log(EventLogError::Sql {
                    message: String::new(),
                }),
                None,
                false,
            ),
            (
                RegistryError::Log(EventLogError::MissingActor { seq: Some(1) }),
                Some(13),
                true,
            ),
        ];
        for (error, section, refusal) in cases {
            let reference = error.methodology_ref();
            assert_eq!(reference.map(section_of), section, "{error:?}");
            assert_eq!(error.is_refusal(), refusal, "{error:?}");
        }
        let event = RegistryError::Event {
            seq: 3,
            defect: EventDefect::NotTheOperator,
        };
        assert_eq!(
            event.methodology_ref(),
            Some(EventDefect::NotTheOperator.methodology_ref())
        );
    }

    #[test]
    fn ori_t_0194_no_display_echoes_the_text_it_refused() {
        let mut shown: Vec<String> = Vec::new();

        // Server errors, from inputs that carry the marker.
        let marked = format!("{MARKER}\0");
        let marked_long = format!("{MARKER}{}", "c".repeat(CONFIG_TEXT_MAX));
        let marked_args: Vec<String> = vec![MARKER.to_owned(); ARGS_MAX + 1];
        let marked_arg_long = format!("{MARKER}{}", "x".repeat(CONFIG_TEXT_MAX));
        let results = [
            StdioConfig::new(String::new(), vec![MARKER.to_owned()]),
            StdioConfig::new(marked_long.clone(), Vec::new()),
            StdioConfig::new(marked.clone(), Vec::new()),
            StdioConfig::new("c".to_owned(), marked_args),
            StdioConfig::new("c".to_owned(), vec![marked_arg_long]),
            StdioConfig::new("c".to_owned(), vec![MARKER.to_owned(), marked.clone()]),
        ];
        for result in results {
            let error = result.expect_err("every input here is refused");
            shown.push(error.to_string());
        }
        shown.push(Transport::parse(MARKER).expect_err("not stdio").to_string());
        shown.extend(every_server_error().iter().map(ToString::to_string));
        shown.extend(every_event_defect().iter().map(ToString::to_string));

        // Event defects and registry errors, from payloads planted and refused.
        let marked_payloads = [
            format!("not json {MARKER}"),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                r#""github-mcp""#,
                // The command carries the marker and an escaped NUL, which is
                // what makes the addition refused.
                &format!(r#""{MARKER}\u0000""#),
            ),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                r#""stdio""#,
                &format!(r#""{MARKER}""#),
            ),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                r#"["qa"]"#,
                &format!(r#"["{MARKER}"]"#),
            ),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                &format!(r#""credential_ref":"MCP_GITHUB_{SERVER_EXAMPLE}""#),
                &format!(r#""credential_ref":"{MARKER}""#),
            ),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                r#""name":"github""#,
                &format!(r#""name":"{MARKER}""#),
            ),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                SERVER_EXAMPLE,
                MARKER,
            ),
            changed(
                &good_added(SERVER_EXAMPLE, "github"),
                r#""credential_ref""#,
                &format!(r#""{MARKER}""#),
            ),
        ];
        for payload in marked_payloads {
            let mut home = Home::new("display", PRODUCT_A);
            plant(
                &mut home,
                &product_a(),
                Actor::Human(operator()),
                SERVER_ADDED_KIND,
                &payload,
            );
            let error = Registry::load(&mut home.db).expect_err("every planted payload is refused");
            assert!(matches!(error, RegistryError::Event { .. }), "{error:?}");
            shown.push(error.to_string());
        }
        let mut home = Home::new("display-removal", PRODUCT_A);
        plant_human_added(&mut home, 1, "github");
        plant(
            &mut home,
            &product_a(),
            Actor::Human(operator()),
            SERVER_REMOVED_KIND,
            &removed_json(SERVER_EXAMPLE, MARKER),
        );
        let error = Registry::load(&mut home.db).expect_err("the removal is refused");
        shown.push(error.to_string());

        // The other registry errors, with a marked product text where one fits.
        let mut marked_home = Home::new("display-product", &format!("PRODUCT-{MARKER}"));
        let marked_product =
            Registry::load(&mut marked_home.db).expect_err("the product text is not a ULID");
        assert_eq!(marked_product, RegistryError::ProductNotAnId);
        shown.push(marked_product.to_string());
        let mut home_a = Home::new("display-a", PRODUCT_A);
        let mut registry = loaded(&mut home_a);
        shown.push(
            registry
                .refresh(&mut marked_home.db)
                .expect_err("another product")
                .to_string(),
        );
        shown.push(
            registry
                .remove(&mut home_a.db, ts(AT), operator(), &ulid(1))
                .expect_err("not found")
                .to_string(),
        );
        add_ok(
            &mut registry,
            &mut home_a,
            server(1, "github", false, &[], false),
        );
        shown.push(
            registry
                .add(
                    &mut home_a.db,
                    ts(AT),
                    operator(),
                    server(2, "github", false, &[], false),
                )
                .expect_err("the name is taken")
                .to_string(),
        );
        shown.push(
            registry
                .add(
                    &mut home_a.db,
                    ts(AT),
                    operator(),
                    server(1, "other", false, &[], false),
                )
                .expect_err("the id is taken")
                .to_string(),
        );
        shown.push(RegistryError::LogMoved { folded_seq: 4 }.to_string());
        shown.push(RegistryError::Encode.to_string());

        assert!(shown.len() > 30);
        for text in &shown {
            assert!(!text.is_empty());
            assert!(!text.contains(MARKER), "{text}");
        }
    }

    #[test]
    fn ori_t_0194_a_read_chains_to_the_tip_only_from_the_folded_event_to_the_verified_tip() {
        let mut home = Home::new("chains", PRODUCT_A);
        for n in 1..=3 {
            plant_human_added(&mut home, n, &format!("server-{n}"));
        }
        let events = all_events(&mut home);
        assert_eq!(events.len(), 3);
        let digests: Vec<EventHash> = events.iter().map(Event::digest).collect();

        // From nothing folded to the tip.
        assert!(chains_to_tip(
            &events[0],
            &events[2],
            None,
            Some(digests[2])
        ));
        // From the first event folded to the tip.
        assert!(chains_to_tip(
            &events[1],
            &events[2],
            Some(digests[0]),
            Some(digests[2])
        ));
        // The last event read is not the tip the verified read reported.
        assert!(!chains_to_tip(
            &events[0],
            &events[2],
            None,
            Some(digests[1])
        ));
        assert!(!chains_to_tip(&events[0], &events[2], None, None));
        // The first event read does not chain from the event folded.
        assert!(!chains_to_tip(
            &events[1],
            &events[2],
            Some(digests[1]),
            Some(digests[2])
        ));
        assert!(!chains_to_tip(
            &events[1],
            &events[2],
            None,
            Some(digests[2])
        ));
        assert!(!chains_to_tip(
            &events[0],
            &events[2],
            Some(digests[0]),
            Some(digests[2])
        ));
    }

    #[test]
    fn ori_t_0194_servers_list_in_name_order() {
        let mut home = Home::new("order", PRODUCT_A);
        let mut registry = loaded(&mut home);
        for (n, server_name) in [(1, "zeta"), (2, "alpha"), (3, "mid")] {
            add_ok(
                &mut registry,
                &mut home,
                server(n, server_name, false, &[], false),
            );
        }
        let names: Vec<&str> = registry.servers().map(|s| s.name().as_str()).collect();
        assert_eq!(names, ["alpha", "mid", "zeta"]);
        let reloaded = loaded(&mut home);
        let names: Vec<&str> = reloaded.servers().map(|s| s.name().as_str()).collect();
        assert_eq!(names, ["alpha", "mid", "zeta"]);
    }
}
