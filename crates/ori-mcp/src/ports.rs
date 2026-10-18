//! The ports: what the server reaches through the engine, declared here and
//! implemented by `ori-engine`: AICD §17, AICD §25.
//!
//! # Why traits here
//!
//! The server core (slice S2) must reach a ticket's category, a specification
//! pull request, a source of identifiers and the operator's MCP servers, and
//! `spec/LLD.md` section 2 draws no edge from `ori-mcp` to `ori-orchestrator`,
//! `ori-flows` or `ori-broker`, and none is added (`ops/batch-10-plan.md`,
//! decision D1). The edge the graph does draw is `ENG --> MCP`: `ori-engine`
//! depends on this crate, so a trait declared here and implemented there adds no
//! edge. These are not the adapter traits of `spec/API_SPEC.md` section 4, and
//! nothing is added to `ori-core` for them.
//!
//! ```mermaid
//! flowchart LR
//!   ENG["ori-engine"] -- "depends on" --> MCP["ori-mcp: Server, ToolScopes, ports"]
//!   ENG -- "depends on" --> ORCH["ori-orchestrator"]
//!   ENG -- "depends on" --> BRK["ori-broker"]
//!   ENG -. "implements the ports with what it holds" .-> MCP
//!   MCP -. "never" .-> ORCH
//!   MCP -. "never" .-> BRK
//! ```
//!
//! # What is declared, and what waits
//!
//! Four ports: [`crate::ports::IdSource`], [`crate::ports::TicketDesk`],
//! [`crate::ports::SpecDesk`] and [`crate::ports::UserToolHost`].
//! `PlanDesk` and `EscalationDesk` are not here. Each takes agent text that must
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
//!   [`crate::scopes::UserToolGrant`]**, which only
//!   [`crate::scopes::ToolScopes::authorize_user_tool`] can make, so no call
//!   reaches an operator's server without `scopes.rs`'s decision.
//!
//! A port also receives the [`crate::scopes::Caller`] that the server core built
//! from the session binding, never an identity read from a tool argument, and
//! decides nothing about whether the caller's role may use the tool: that
//! decision was made in `scopes.rs` before the port was called.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/ports.rs`, "What agents
//! can do". Every method here is something an agent's call can cause.

use core::fmt;

use ori_core::error::{Error, MethodologyRef};
use ori_core::types::{Category, Id};
use ori_store::db::ProductDb;

use crate::protocol::{CallToolResult, JsonObject, Tool};
use crate::scopes::{Caller, UserToolGrant};

/// Why a port could not do what it was asked: AICD §17.
///
/// Three kinds, because the server core answers them differently: a refusal by
/// the component behind the port is passed on with its own reason, an argument
/// the port could not accept is the caller's mistake, and an unavailable
/// component is the engine's.
///
/// None of the texts may be copied into an `mcp.*` event, which holds
/// identifiers and lengths and never text (`spec/API_SPEC.md` section 3): they
/// can carry what the component behind the port said.
///
/// The `Display` is written by hand, as ruling R20 allows until `thiserror` is
/// adopted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PortError {
    /// The component behind the port refused. The error keeps its
    /// [`MethodologyRef`] and its `E_` code, such as `E_UPGRADE_ONLY` for a
    /// category that would be lowered (`spec/API_SPEC.md` section 1,
    /// `tickets.setCategory`).
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
    /// Called for `aicd_ticket_upgrade`, which only the lead holds
    /// ([`crate::scopes::ToolScopes`] decided that before this call). The
    /// implementation refuses a category that is not above the ticket's current
    /// one with an error that keeps the code `E_UPGRADE_ONLY`
    /// ([`PortError::Refused`]).
    ///
    /// # Errors
    ///
    /// [`PortError::Refused`] for a refusal by the component behind the port,
    /// [`PortError::Invalid`] for a ticket that does not exist in the caller's
    /// product, [`PortError::Unavailable`] when it could not answer.
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
    /// Called for `aicd_spec_propose`, which only the documentation role holds.
    /// The agent never writes `spec/` itself: the content becomes a proposal a
    /// human reviews (`spec/API_SPEC.md` section 3, the second promise).
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
/// **Every method takes a `&UserToolGrant`**, which only
/// [`crate::scopes::ToolScopes::authorize_user_tool`] can make, so no call
/// reaches an operator's server without `scopes.rs`'s decision, and the host
/// that implements this holds no policy. The grant names the product, the
/// server and the role; the implementation reaches the server of that name in
/// that product and no other.
///
/// The server's own secret stays with the engine, which connects to the server
/// itself, and no credential passes through this port. What the server returns
/// is untrusted data (`spec/API_SPEC.md` section 3, "Proxied tools",
/// condition 3): the implementation returns text blocks only, and the server
/// core labels every block untrusted before it reaches a session.
pub trait UserToolHost {
    /// The tools of the granted server, as the server named them: untrusted
    /// data, names and descriptions included.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the server cannot be reached.
    fn list_tools(&mut self, grant: &UserToolGrant) -> Result<Vec<Tool>, PortError>;

    /// Calls `tool` of the granted server with `arguments`, and returns its
    /// result.
    ///
    /// `tool` is the server's own name for the tool, the part after the first
    /// `__` of the name the agent called ([`crate::scopes::split_proxied`]).
    ///
    /// # Errors
    ///
    /// [`PortError::Invalid`] for a tool the server does not have,
    /// [`PortError::Unavailable`] when the server cannot be reached or does not
    /// answer.
    fn call_tool(
        &mut self,
        grant: &UserToolGrant,
        tool: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, PortError>;
}

#[cfg(test)]
mod tests {
    use ori_core::error::RefusalKind;
    use ori_core::types::Role;

    use super::*;
    use crate::scopes::{ExposedServer, ServerName, ToolScopes};

    const PRODUCT: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

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
        fn list_tools(&mut self, grant: &UserToolGrant) -> Result<Vec<Tool>, PortError> {
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
            grant: &UserToolGrant,
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

        // The only way to a grant is the decision in scopes.rs.
        let product = id(PRODUCT);
        let caller = Caller::new(Role::Qa, product.clone());
        let exposed = ExposedServer::new(
            product,
            ServerName::parse("github").expect("a server name"),
            vec![Role::Qa],
            true,
        );
        let grant = ToolScopes::authorize_user_tool(&caller, &exposed).expect("a grant");
        let host: &mut dyn UserToolHost = &mut scripted;
        let tools = host.list_tools(&grant).expect("a list");
        assert_eq!(tools[0].name, "github-tool");
        let result = host
            .call_tool(&grant, "create_issue", None)
            .expect("a result");
        assert_eq!(result.content[0].as_text(), Some("github:create_issue"));
    }
}
