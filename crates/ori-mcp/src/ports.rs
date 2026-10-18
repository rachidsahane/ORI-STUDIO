//! The ports: what the server reaches through the engine, declared here and
//! implemented by `ori-engine`: AICD §17.
//!
//! # Why traits here
//!
//! The server core (slice S2) must reach a ticket's category, a specification
//! pull request, a source of identifiers, the operator's MCP servers, the time
//! and the state of the session it serves, and `spec/LLD.md` section 2 draws no
//! edge from `ori-mcp` to `ori-orchestrator`, `ori-flows` or `ori-broker`, and
//! none is added (`ops/batch-10-plan.md`, decision D1). The edge the graph does draw is `ENG --> MCP`: `ori-engine`
//! depends on this crate, so a trait declared here and implemented there adds no
//! edge. These are not the adapter traits of `spec/API_SPEC.md` section 4, and
//! nothing is added to `ori-core` for them.
//!
//! ```mermaid
//! flowchart LR
//!   ENG["ori-engine"] -- "depends on" --> MCP["ori-mcp: Server, ToolScopes, six ports"]
//!   ENG -- "depends on" --> ORCH["ori-orchestrator"]
//!   ENG -- "depends on" --> BRK["ori-broker"]
//!   ENG -. "implements IdSource, TicketDesk, SpecDesk, UserToolHost, Clock and SessionDesk with what it holds" .-> MCP
//!   MCP -. "never" .-> ORCH
//!   MCP -. "never" .-> BRK
//! ```
//!
//! # What is declared, and what waits
//!
//! Six ports: [`crate::ports::IdSource`], [`crate::ports::TicketDesk`],
//! [`crate::ports::SpecDesk`], [`crate::ports::UserToolHost`],
//! [`crate::ports::Clock`] and [`crate::ports::SessionDesk`]. The first four
//! are slice S1's; the last two are slice S2's, and the server core calls them
//! itself: [`crate::ports::Clock`] stamps every event the server appends, and
//! [`crate::ports::SessionDesk`] says whether the session the connection serves
//! may still act. `PlanDesk` and `EscalationDesk` are not here. Each takes agent text that must
//! pass the sanitization barrier first, and what they need is the barrier's
//! sealed sanitized type, which `ori-memory` does not export yet (escalation
//! E-0012's public entry, plan prerequisite PM2). They arrive with slice S9, so
//! that no port in this file takes a raw `String` where the barrier's output
//! belongs.
//!
//! # What every port promises
//!
//! - **Every method that writes takes `&mut ProductDb`**, so the single writer
//!   stays the caller's: the engine's writer task holds the one
//!   [`ori_store::db::ProductDb`] of a product (`spec/LLD.md` section 5), and
//!   an implementation writes through the handle it is given and opens none of
//!   its own.
//! - **No port carries a credential value.** A credential is a name the engine
//!   resolves through the broker; an operator's server's own secret stays with
//!   the engine, which connects to the server itself (`spec/API_SPEC.md`
//!   section 3, "Proxied tools", condition 4).
//! - **[`crate::ports::UserToolHost`] takes a
//!   [`crate::scopes::UserToolGrant`] by value**, which only
//!   [`crate::scopes::ToolScopes::authorize_user_tool`] can make and which each
//!   host call consumes, so no call reaches an operator's server without a
//!   decision of `scopes.rs` made for that call.
//!
//! [`crate::ports::Clock`] and [`crate::ports::SessionDesk`] are `Send`, which
//! the first four are not required to be: the server holds them as trait
//! objects and is itself `Send`, so the engine can build it on one thread and
//! hand it to the product's writer task.
//!
//! A port also receives the [`crate::scopes::Caller`] that the server core built
//! from the session binding, never an identity read from a tool argument. The
//! caller carries the agent identity of the session, and a port that writes
//! records its write under it. A desk never assumes that the server core's scope
//! check ran: it makes the checks of the component it routes the change through,
//! and each method below says which.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/ports.rs`, "What agents
//! can do". Every method here is something an agent's call can cause.

use core::fmt;

use ori_core::error::{Error, MethodologyRef};
use ori_core::types::{Category, Id, Timestamp};
use ori_store::db::ProductDb;

use crate::protocol::{CallToolResult, JsonObject, Tool};
use crate::scopes::{Caller, UserToolGrant};
use crate::server::Binding;

/// Why a port could not do what it was asked: AICD §17.
///
/// Three kinds, because the server core answers them differently: a refusal by
/// the component behind the port is passed on with its own code and methodology
/// section, an argument the port could not accept is the caller's mistake, and
/// an unavailable component is the engine's.
///
/// The free text in [`PortError::Invalid`]'s `why` and in
/// [`PortError::Unavailable`]'s `what` may hold implementation detail, such as
/// a server's command or its stderr. It reaches neither an `mcp.*` event, which
/// holds identifiers and lengths and never text (`spec/API_SPEC.md` section 3),
/// nor an agent's reply: the server core replies with the failure's kind and
/// code only.
///
/// The `Display` is written by hand, as ruling R20 allows until `thiserror` is
/// adopted; converting it is mechanical, each arm becoming an `#[error]`
/// attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PortError {
    /// The component behind the port refused. The error keeps its
    /// [`MethodologyRef`] and its `E_` code, such as `E_UPGRADE_ONLY` for a
    /// category that would be lowered (`spec/API_SPEC.md` section 1,
    /// `tickets.setCategory`).
    ///
    /// It must hold an [`Error`] that is a refusal, one with a
    /// [`MethodologyRef`]. The server core treats a `Refused` without one as an
    /// implementation defect and reports the failure as `invalid`.
    Refused(Error),
    /// An argument was not acceptable to the component behind the port.
    Invalid {
        /// The argument, by the name the tool gives it.
        field: &'static str,
        /// Why it was not acceptable.
        why: String,
    },
    /// The component behind the port could not be reached or could not answer.
    Unavailable {
        /// What was unavailable.
        what: String,
    },
}

impl PortError {
    /// The methodology section of the refusal this error carries, for
    /// [`PortError::Refused`], and `None` for the other two kinds, which are not
    /// control refusals (the shape of [`ori_core::error::Error::methodology_ref`],
    /// for the reason that module gives): AICD §17.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Refused(error) => error.methodology_ref(),
            Self::Invalid { .. } | Self::Unavailable { .. } => None,
        }
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) => write!(f, "{error}"),
            Self::Invalid { field, why } => write!(f, "{field} is not acceptable: {why}"),
            Self::Unavailable { what } => write!(f, "{what} is unavailable"),
        }
    }
}

impl std::error::Error for PortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(error) => Some(error),
            Self::Invalid { .. } | Self::Unavailable { .. } => None,
        }
    }
}

/// A source of identifiers: `Id` never generates one (`ori-core`), because a
/// ULID needs a clock and a source of randomness, which the engine holds.
///
/// Takes no `ProductDb` and writes nothing: the identifier is a value, and the
/// caller records it. No credential passes through this port.
pub trait IdSource {
    /// The next identifier, never one returned before.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the engine's clock or random source
    /// cannot answer.
    fn next_id(&mut self) -> Result<Id, PortError>;
}

/// Ticket operations the server reaches through the engine: AICD §11, AICD §17.
///
/// The implementation holds the rules of `ori-orchestrator`; this crate holds
/// none of them. A method that writes takes `&mut ProductDb`, so the single
/// writer stays the caller's. No credential passes through this port.
pub trait TicketDesk {
    /// Raises a ticket's category, which AICD §11 allows in one direction only.
    ///
    /// Called for `aicd_ticket_upgrade`. The implementation never assumes the
    /// server core's scope check ran, so it:
    ///
    /// - records its write with `Actor::Agent`, naming `caller.identity()` as
    ///   the agent;
    /// - refuses a caller whose product is not `db`'s product;
    /// - routes the change through `ori-orchestrator`'s `Lifecycle::advance`,
    ///   which evaluates AICD §17's matrix itself.
    ///
    /// It refuses what `ori-core`'s `Ticket::apply` refuses as a category
    /// downgrade, with an error that keeps the code `E_UPGRADE_ONLY`
    /// ([`PortError::Refused`]), and nothing else.
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal by the component behind the port,
    /// [`PortError::Invalid`] for a ticket the caller's product does not hold,
    /// [`PortError::Unavailable`] when it could not answer.
    fn upgrade(
        &mut self,
        db: &mut ProductDb,
        caller: &Caller,
        ticket: &Id,
        to: Category,
    ) -> Result<(), PortError>;
}

/// Specification operations the server reaches through the engine: AICD §9,
/// AICD §17.
///
/// A method that writes takes `&mut ProductDb`, so the single writer stays the
/// caller's. No credential passes through this port: opening the pull request
/// uses a credential the engine resolves through the broker, by name, and the
/// reply carries only the pull request's reference.
pub trait SpecDesk {
    /// Opens a specification pull request; returns its reference for the reply.
    ///
    /// Called for `aicd_spec_propose`. The agent never writes `spec/` itself:
    /// the content becomes a proposal a human reviews (`spec/API_SPEC.md`
    /// section 3, the second promise).
    ///
    /// The implementation never assumes the server core's scope check ran, so
    /// it:
    ///
    /// - records its write with `Actor::Agent`, naming `caller.identity()` as
    ///   the agent;
    /// - refuses a caller whose product is not `db`'s product;
    /// - routes the proposal through the owning component's own checks, which
    ///   evaluate AICD §17's matrix for the specification themselves.
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal by the component behind the port,
    /// [`PortError::Invalid`] for a document that is not one of the
    /// specification's, [`PortError::Unavailable`] when it could not answer.
    fn propose(
        &mut self,
        db: &mut ProductDb,
        caller: &Caller,
        document: &Id,
        content: &str,
    ) -> Result<String, PortError>;
}

/// The host toward the operator's MCP servers, as the server core sees it:
/// AICD §17, AICD §26.
///
/// **Every method takes a [`UserToolGrant`] by value**, which only
/// [`crate::scopes::ToolScopes::authorize_user_tool`] can make, and each host
/// call consumes it. The grant cannot be copied, so one decision covers one host
/// call; the caller (slices S2 and S6) calls `authorize_user_tool` for every
/// call, from the registry's state at that moment. No call reaches an
/// operator's server without `scopes.rs`'s decision, and the host that
/// implements this holds no policy. The grant names the product, the server and
/// the role. It names the server by name, and the implementation resolves that
/// name in that product when the call is made and reaches no other server.
///
/// The server's own secret stays with the engine, which connects to the server
/// itself, and no credential passes through this port.
///
/// **What the server returns is untrusted data, and the host is not trusted to
/// filter it** (`spec/API_SPEC.md` section 3, "Proxied tools", condition 3). The
/// implementation returns what the operator's server returned. The server
/// core's proxy (slice S6, `tools/proxy.rs`, tier 2) enforces condition 3 on
/// whatever the host returns: it keeps text blocks only, drops every other
/// block and any `structured_content`, and labels every block it forwards
/// untrusted.
pub trait UserToolHost {
    /// The tools of the granted server, as the server named them: untrusted
    /// data, names and descriptions included. The grant is consumed.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the server cannot be reached.
    fn list_tools(&mut self, grant: UserToolGrant) -> Result<Vec<Tool>, PortError>;

    /// Calls `tool` of the granted server with `arguments`, and returns its
    /// result as the server returned it. The grant is consumed.
    ///
    /// `tool` is the server's own name for the tool, the part after the first
    /// `__` of the name the agent called ([`crate::scopes::split_proxied`]),
    /// once the server core has matched it against the server's tool list.
    ///
    /// # Errors
    ///
    /// [`PortError::Invalid`] for a tool the server does not have,
    /// [`PortError::Unavailable`] when the server cannot be reached or does not
    /// answer.
    fn call_tool(
        &mut self,
        grant: UserToolGrant,
        tool: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, PortError>;
}

/// The engine's clock: AICD §17.
///
/// Every event the server appends takes its `at` from here, **never from a
/// message**: an agent cannot choose the time of its own audit trail, and the
/// call budget's window runs on this clock too. `ori-core` has no clock
/// (`Timestamp` is built from a value the caller has), so the engine, which has
/// one, implements this.
///
/// Takes no `ProductDb` and writes nothing; no credential passes through this
/// port. The server reads it once at the entry of every call, which is the
/// call's own time, and once more when it records a failure. `Send`, because the
/// server that holds it is handed to the product's writer task.
pub trait Clock: Send {
    /// The current time. It may step backwards (a clock set back by an
    /// operator): the call budget starts a new window when it does.
    fn now(&mut self) -> Timestamp;
}

/// Whether the session a connection serves may still act: AICD §17.
///
/// A connection grants what the session behind it holds, and nothing once that
/// session is not active (suspended for an escalation, ended, revoked). The
/// server asks before it looks at what a call names, and closes the connection
/// on a refusal, so a session that is over cannot probe the tool surface.
///
/// The caller holds the single `ProductDb` writer ([`crate::ports`]'s "What
/// every port promises"): an implementation reads the session's state through
/// the handle it is given and opens none of its own. No credential passes
/// through this port. `Send`, because the server that holds it is handed to the
/// product's writer task.
pub trait SessionDesk: Send {
    /// Whether the session `binding` names is active.
    ///
    /// `binding` is the engine's own record of the session the connection
    /// serves; the implementation answers for that session and for no other.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the session's state could not be read.
    /// The server refuses the call and closes the connection: it fails closed.
    fn is_active(&mut self, db: &mut ProductDb, binding: &Binding) -> Result<bool, PortError>;
}

#[cfg(test)]
mod tests {
    use ori_core::error::RefusalKind;
    use ori_core::types::Role;

    use super::*;
    use crate::scopes::{ExposedServer, ServerName, ToolScopes};

    const PRODUCT: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    /// A valid identifier for the agent identity of a session.
    const AGENT: &str = "01D39ZY06FGSCTVN4T2V9PKHFZ";

    fn id(text: &str) -> Id {
        Id::parse(text).expect("a valid ULID")
    }

    /// A refusal from the component behind a port: AICD §11's upgrade-only rule.
    fn upgrade_only() -> Error {
        Error::refused(RefusalKind::CategoryDowngrade {
            from: Category::Decisional,
            to: Category::Auto,
        })
    }

    #[test]
    fn ori_t_0166_a_refused_port_error_keeps_its_methodology_ref_and_its_code() {
        let error = PortError::Refused(upgrade_only());
        let reference = error
            .methodology_ref()
            .expect("a refusal carries a reference");
        assert!(reference.resolves());
        assert_eq!(reference.section, 11);
        match &error {
            PortError::Refused(inner) => assert_eq!(inner.code(), Some("E_UPGRADE_ONLY")),
            other => panic!("not the refusal that was built: {other:?}"),
        }
        let text = error.to_string();
        assert!(
            text.contains("refused") && text.contains("AICD §11"),
            "{text}"
        );
        let source = std::error::Error::source(&error).expect("a refusal has a source");
        assert_eq!(source.to_string(), text);
    }

    #[test]
    fn ori_t_0166_an_invalid_or_unavailable_port_error_carries_no_methodology_ref() {
        let invalid = PortError::Invalid {
            field: "ticket_id",
            why: "no such ticket".to_owned(),
        };
        let unavailable = PortError::Unavailable {
            what: "the id source".to_owned(),
        };
        assert_eq!(invalid.methodology_ref(), None);
        assert_eq!(unavailable.methodology_ref(), None);
        assert_eq!(
            invalid.to_string(),
            "ticket_id is not acceptable: no such ticket"
        );
        assert_eq!(unavailable.to_string(), "the id source is unavailable");
        assert!(std::error::Error::source(&invalid).is_none());
        assert!(std::error::Error::source(&unavailable).is_none());
    }

    /// A port that answers from a script, to show the traits can be implemented
    /// and used as objects.
    struct Scripted {
        next: u32,
    }

    impl IdSource for Scripted {
        fn next_id(&mut self) -> Result<Id, PortError> {
            self.next += 1;
            if self.next > 1 {
                return Err(PortError::Unavailable {
                    what: "the id source".to_owned(),
                });
            }
            Ok(id(PRODUCT))
        }
    }

    impl TicketDesk for Scripted {
        fn upgrade(
            &mut self,
            _db: &mut ProductDb,
            _caller: &Caller,
            _ticket: &Id,
            _to: Category,
        ) -> Result<(), PortError> {
            Err(PortError::Refused(upgrade_only()))
        }
    }

    impl SpecDesk for Scripted {
        fn propose(
            &mut self,
            _db: &mut ProductDb,
            _caller: &Caller,
            _document: &Id,
            _content: &str,
        ) -> Result<String, PortError> {
            Ok("pull/1".to_owned())
        }
    }

    impl UserToolHost for Scripted {
        fn list_tools(&mut self, grant: UserToolGrant) -> Result<Vec<Tool>, PortError> {
            Ok(vec![Tool {
                name: format!("{}-tool", grant.server()),
                title: None,
                description: None,
                input_schema: JsonObject::new(),
                output_schema: None,
                annotations: None,
                execution: None,
            }])
        }

        fn call_tool(
            &mut self,
            grant: UserToolGrant,
            tool: &str,
            _arguments: Option<JsonObject>,
        ) -> Result<CallToolResult, PortError> {
            Ok(CallToolResult::text(format!("{}:{tool}", grant.server())))
        }
    }

    #[test]
    fn ori_t_0166_the_ports_are_object_safe_and_a_host_is_reached_only_with_a_grant() {
        let mut scripted = Scripted { next: 0 };
        // Each port is usable as a trait object, which is how the engine hands
        // them to the server core.
        let ids: &mut dyn IdSource = &mut scripted;
        assert_eq!(ids.next_id(), Ok(id(PRODUCT)));
        assert!(matches!(ids.next_id(), Err(PortError::Unavailable { .. })));
        let _: Option<&mut dyn TicketDesk> = None;
        let _: Option<&mut dyn SpecDesk> = None;

        // The only way to a grant is the decision in scopes.rs, and each host
        // call consumes the grant it was made for: two calls are two decisions.
        let product = id(PRODUCT);
        let caller = Caller::new(id(AGENT), Role::Qa, product.clone());
        let exposed = ExposedServer::new(
            product,
            ServerName::parse("github").expect("a server name"),
            vec![Role::Qa],
            true,
        );
        let host: &mut dyn UserToolHost = &mut scripted;
        let grant = ToolScopes::authorize_user_tool(&caller, &exposed).expect("a grant");
        let tools = host.list_tools(grant).expect("a list");
        assert_eq!(tools[0].name, "github-tool");
        let grant = ToolScopes::authorize_user_tool(&caller, &exposed).expect("a second grant");
        let result = host
            .call_tool(grant, "create_issue", None)
            .expect("a result");
        assert_eq!(result.content[0].as_text(), Some("github:create_issue"));
    }
}
