# ORI-T-0194: batch 10 slice H1, the registry of the operator's MCP servers (Sonnet: the lead decided every rule below; implement it)

| Field | Value |
|---|---|
| Worktree | `/home/user/wt/ORI-T-0194` |
| Branch | `feat/ORI-T-0194-mcp-registry`, at `<BASE>` (the session head; its last commit records this ticket's claim 121 in `ops/lock-table.md`) |
| Tier | **2, raised by the lead.** `spec/RISK_MAP.md`'s row `crates/ori-mcp (host)` gives 1, and gate 12's tier 2 list for ori-mcp names only `scopes.rs`, `ports.rs`, `server.rs` and `tools/` (`crates/ori-gates/src/significance.rs`). But this registry supplies the facts `ToolScopes::authorize_user_tool` grants on, so a defect in its fold widens what agents can reach. The backlog lets an agent raise a tier ("Agents may raise a tier and only a human may lower one", `ops/phase-1-backlog.md`, "How to read this"). Nothing in your work changes because of it; the operator merges |
| Declared scope (claim 121) | New files `crates/ori-mcp/src/host.rs` and `crates/ori-mcp/src/host/registry.rs`. In `crates/ori-mcp/src/lib.rs`, one module declaration with its doc comment (Decision 1). **Nothing else**: not `scopes.rs`, `ports.rs`, `server.rs`, `tools/`, `protocol.rs`, not `Cargo.toml` (no dependency is added), not `Cargo.lock`, no other crate. **No existing test changes** |
| Spec anchor | `spec/API_SPEC.md`: the preamble's rule on mutating methods; section 1, `project.mcp.add`, `list`, `remove`; section 2, the kinds `mcp.server_added` and `mcp.server_removed`; section 3, its opening paragraph and "Proxied tools". `spec/DATA_MODEL.md`: the preamble, section 2's `McpServer` and `Event` rows, section 4. `spec/ENV_SETUP.md` section 4 (the rules on secrets and the `MCP_<SERVER>_*` row) and section 5 (the paragraph on exposing a server). `spec/SECURITY_NOTES.md`, "Secrets" |
| Spec trailer | `Spec: API_SPEC.md#3-agent-facing-mcp-server` |
| Ticket trailer | `Ticket: ORI-T-0194`, **one per commit** |
| Commit types | only `feat`, `fix`, `docs`, `ci`, `chore`. **Never `test(...)`** |
| Model | Sonnet 5.5. `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` |

## Why

`ops/batch-10-plan.md` splits ORI-T-0056 (the MCP host) into H1, the registry, and H2, the client. H1 is this ticket: the record of which servers the operator added to a product, the two events that record it, and the rules an addition must satisfy.

What waits on it:
- S6 (later) will call `ToolScopes::authorize_user_tool(&caller, &server)` (`crates/ori-mcp/src/scopes.rs`) for every proxied call, at `server.rs`'s "S6 POINT 2", which S2 left for it. That function's doc says the `ExposedServer` it takes comes from "the registry's projection of `mcp.server_added` and `mcp.server_removed` (slice H1)". Nothing builds one yet.
- H2 (later), the client, resolves a granted server to the command it launches.
- The engine (`ori-engine`, ORI-T-0070, batch 13, later) is expected to implement `project.mcp.add`, `list` and `remove` over this registry, through `ori-rpc`'s dispatch (ORI-T-0066). No record yet assigns these methods, so every doc names them as later work.

Read first, on your base:
- `ops/batch-10-plan.md`, all of it;
- `ops/escalations/E-0010-proxying-the-operators-mcp-tools.md`, the answer; `ops/rulings/R45-operator-answers-of-2026-10-18.md`, points 3 and 4;
- `spec/API_SPEC.md`: the preamble (the rule that every mutating method returns the `Event.seq` it produced); section 1's `project.mcp.*` line; section 2's list of kinds; section 3's opening paragraph and from "### Proxied tools" to the end of the section;
- `spec/DATA_MODEL.md`: the preamble ("the tables below are projections rebuilt from the log"), section 2's `McpServer` and `Event` rows, section 4;
- `spec/ENV_SETUP.md` section 4 (its "Rules" line and the `MCP_<SERVER>_*` row) and section 5 (the paragraph that begins "Exposing a server of the operator's to a role");
- `spec/SECURITY_NOTES.md`, "Secrets" and trust boundary 1; `spec/ROADMAP.md`, "Later (not scheduled)";
- `crates/ori-mcp/src/scopes.rs`: `ServerName` and its `parse`, `ServerNameError`, `ExposedServer`, `ToolScopes::authorize_user_tool`, `UserToolGrant`, and how the module builds a `MethodologyRef` (`whole_section` and its constants);
- `crates/ori-mcp/src/ports.rs`: the module doc's "What every port promises" and `UserToolHost`;
- `crates/ori-mcp/src/lib.rs`;
- `crates/ori-store/src/event_log.rs`: `EventLog::append`, `EventLog::read_range`, `EventLog::verify` and `VerifyReport`, `Event` (with `digest` and `hash_prev`), `EventHash`, `EventLogError` (with `methodology_ref` and `is_refusal`);
- `crates/ori-store/src/db.rs`: `ProductDb::open`, `connection`, `product_id`;
- `crates/ori-store/src/rebuild.rs`: `RebuildError`, whose `EventLog` arm passes the inner error's `methodology_ref` and `is_refusal` through. This ticket's error does the same;
- `crates/ori-broker/src/issuance.rs` (`read_all_events`) and `crates/ori-memory/src/scope.rs` (its verified read before a fold): the verified read this registry copies;
- `crates/ori-memory/src/retrieval.rs`: `is_unsafe_char`, whose character set Decision 2 repeats;
- `crates/ori-core/src/types.rs`: `Id`, `Timestamp`, `Actor`, `Role` (`ALL`, `as_str`, `FromStr`);
- in `crates/ori-mcp/src/server.rs`'s tests: `Scratch` and how a test opens a `ProductDb`.

The lead proved Decision 3's serde behaviour against the pinned serde 1.0.229 and serde_json 1.0.151: `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/proof-0194/src/main.rs` (it prints `ALL CLAIMS HOLD`).

## Decision 1: where the code goes

- **`crates/ori-mcp/src/host.rs`** is the host's module root. It holds the module doc (Decision 9) and one line, `pub mod registry;`.
- **`crates/ori-mcp/src/host/registry.rs`** holds everything below, and its tests.
- **`crates/ori-mcp/src/lib.rs`** gains this declaration, placed before `pub mod ports;` so that the modules stay in alphabetical order. Nothing else in `lib.rs` changes:

  ```rust
  /// The host toward the operator's MCP servers: the registry of the servers the
  /// operator added to the product, the events that record them, and the rules
  /// an addition must satisfy (AICD §17, AICD §26).
  pub mod host;
  ```

`ops/batch-10-plan.md`'s slice table declares `src/host.rs` for both H1 and H2. This ticket makes it a module directory instead, so that H2 can add `host/client.rs` beside the registry; claim 121 records the files, and RISK_MAP's `crates/ori-mcp (host)` row covers both.

The registry **adds no dependency**. It uses `ori-core`, `ori-store`, `serde` and `serde_json`, all already in `crates/ori-mcp/Cargo.toml`.

**No `unwrap`, `expect` or `panic!` outside tests** (`spec/CONVENTIONS.md`). Decision 4 says how each fallible step is answered.

## Decision 2: the types of an addition

All public, in `registry.rs`. Fields private, read through accessors.

**`Transport`**: an enum with one variant, `Stdio`, marked `#[non_exhaustive]`, deriving `Clone, Copy, Debug, Eq, Hash, PartialEq`.
- `pub const fn as_str(self) -> &'static str` gives `"stdio"`.
- `pub fn parse(text: &str) -> Result<Transport, ServerError>` accepts exactly `stdio` (case-sensitive) and refuses everything else, the empty text included, with `ServerError::TransportNotSupported`.
- Its doc cites `spec/API_SPEC.md` section 3's last paragraph (stdio only in phase 1) and `spec/ROADMAP.md`, "Later (not scheduled)", for the reason: a remote (HTTP) server needs an HTTP and TLS client crate the workspace does not have, which is a `new_dependency` decision (operator ruling R45, point 4).

**`StdioConfig`**: what is needed to launch a stdio server. Fields `command: String`, `args: Vec<String>`. Derives `Clone, Eq, PartialEq`, and a **hand-written `Debug`** that prints the command and the number of arguments, never an argument: `StdioConfig { command: "github-mcp", args: 2 argument(s) }`. Its doc line says why: Ori cannot tell whether an argument holds a secret.
- `pub fn new(command: String, args: Vec<String>) -> Result<StdioConfig, ServerError>`. Checks in this order, the first failure answered:
  1. `command` is empty: `CommandEmpty`;
  2. `command` is longer than `CONFIG_TEXT_MAX` bytes: `CommandTooLong { length }`;
  3. `command` holds an unsafe character (below): `CommandHasControl`;
  4. more than `ARGS_MAX` arguments: `TooManyArgs { count }`;
  5. then each argument in turn, in order: longer than `CONFIG_TEXT_MAX` bytes, `ArgTooLong { index, length }`; else holds an unsafe character, `ArgHasControl { index }`. An empty argument is accepted.
- **An unsafe character** is one for which `char::is_control` is true (NUL, the other C0 controls, DEL, the C1 controls), or one of U+061C, U+200E, U+200F, U+2028, U+2029, U+202A to U+202E, U+2066 to U+2069. That is the set `ori-memory`'s `retrieval::is_unsafe_char` scrubs, which is `pub(crate)` there and cannot be called from this crate; write a private function with the same set and say in its doc that it repeats that rule, and that a widening decided under ORI-T-0191 (later; its recorded scope is `crates/ori-memory/src/retrieval.rs`) must be repeated here, because this copy does not follow it on its own. The reasons, in the doc: a NUL cannot be passed to a process (it ends a C string), and the configuration is printed by later surfaces (`events.subscribe` clients, `project.mcp.list`, a CLI), where a control or reordering character would let it display as something other than what is launched.
- Accessors `command(&self) -> &str` and `args(&self) -> &[String]`.
- Constants, public: `pub const CONFIG_TEXT_MAX: usize = 4096;` and `pub const ARGS_MAX: usize = 64;`. Their docs say why: an event payload stays bounded.
- **Its doc states the secret rule and its limit**, in these words or close to them: "`spec/DATA_MODEL.md` section 2 makes `McpServer.config` non-secret, and `spec/ENV_SETUP.md` section 4 ('no secret ... in events') and `spec/SECURITY_NOTES.md`, 'Secrets' ('never serialized into events'; a secret in any event is an incident) keep secrets out of the log. Ori cannot tell whether an argument holds a secret, so the configuration, recorded in clear in `mcp.server_added`, is non-secret by the operator's contract, not by a check. A secret put in an argument is in the log for good: the log is append-only, the event is streamed to every `events.subscribe` client, and it is an incident: rotate it. A server's secret belongs in `project.mcp.add`'s `secret`, which the engine hands to `ori-broker`, the only component that touches the keychain; this registry records only its name (`CredentialRef`)."
- **No environment variables in phase 1**, and the doc says so: a server's secret is to reach its process through `ori-runtime`'s environment injection at spawn (prerequisite PR1, later, not built); the variable it is injected under is PR1's and H2's decision; this registry records no environment.

**`CredentialRef`**: the name, within the server's product, of the keychain entry holding the server's own secret. Never a value. Field `String`; derives `Clone, Debug, Eq, Hash, PartialEq`.
- **It is derived, never chosen**: `pub fn for_server(name: &ServerName, id: &Id) -> CredentialRef` gives `MCP_`, then the name upper-cased with each `-` written `_`, then `_`, then `id.as_str()` (26 characters). For `github` with id `01ARZ3NDEKTSV4RRFFQ69G5FAV`: `MCP_GITHUB_01ARZ3NDEKTSV4RRFFQ69G5FAV`. The longest is 4 + 32 + 1 + 26 = 63 bytes.
- `pub fn as_str(&self) -> &str`. No other constructor.
- Its doc says why it carries the id: a reference belongs to one server for good. A server added again after a removal has a new id, so it never inherits the removed server's secret, and no two servers' references collide, although `a`, `a-` and `a-b` all upper-case to overlapping prefixes. It cites `spec/ENV_SETUP.md` section 4's `MCP_<SERVER>_*` row (the spec leaves the `*` open; this registry fills it with the server's id, and writes `<SERVER>` as the name upper-cased with each `-` written `_`) and condition 4 of "Proxied tools".
- Its doc also says: the broker's keychain is one namespace for every product (`crates/ori-broker/src/keychain.rs`, its `SERVICE`), so whatever resolves a reference (PR1, later) keys the entry by the server's `product_id` together with the reference, never by the reference alone (`spec/ENV_SETUP.md` section 4, "per project"; AICD §26). And: the registry does not delete a removed server's keychain entry; that is the engine's and the broker's (later).

**`NewServer`**: an addition the operator asked for, every rule checked. Derives `Clone, Debug, Eq, PartialEq`.
- `pub fn new(id: Id, name: ServerName, transport: Transport, config: StdioConfig, with_secret: bool, exposed_to_roles: Vec<Role>, acknowledged_reach: bool) -> Result<NewServer, ServerError>`. Seven parameters, which clippy's `too_many_arguments` allows. One check: no role appears twice in `exposed_to_roles`, else `DuplicateRole { role }`, naming **the first role met a second time** while walking the list (`[coder, qa, qa, coder]` names `Qa`).
- `with_secret` says whether `project.mcp.add` was given a `secret`. When true, the server's credential reference is `CredentialRef::for_server(&name, &id)`; when false, it has none.
- `exposed_to_roles` keeps the operator's order. It may be empty. Roles **are accepted when `acknowledged_reach` is false**: `spec/API_SPEC.md` section 1 says "a server added without `acknowledged_reach: true` is exposed to no role", so the addition is recorded as the operator wrote it, and `ToolScopes::authorize_user_tool` refuses every call to it (`ServerReachNotAcknowledged`). Say this in the doc.
- Every role of `Role::ALL` may be listed. Which roles a server is exposed to is the operator's decision (`spec/ENV_SETUP.md` section 5).
- The id comes from the caller. `ori-core`'s `Id` never generates one (`ops/batch-10-plan.md`, decision D1).
- Accessors for every field, `credential_ref(&self) -> Option<&CredentialRef>` among them.

**`RegisteredServer`**: a server the registry holds. Derives `Clone, Debug, Eq, PartialEq`. Fields: `id: Id`, `product_id: Id`, `name: ServerName`, `transport: Transport`, `config: StdioConfig`, `credential_ref: Option<CredentialRef>`, `exposed_to_roles: Vec<Role>`, `acknowledged_reach: bool`, `added_at: Timestamp`, `added_by: Id` (the human identity in the event's actor), `added_seq: u64` (the seq of its `mcp.server_added`, which is what `project.mcp.add` returns).
- Public accessors for every field.
- **No public constructor and no public `exposed()`.** A private method builds its `ExposedServer`, called only by `Registry::exposed` (Decision 4), so that an `ExposedServer` is taken from the registry at the moment of a decision and never from a `RegisteredServer` kept from an earlier call (a removed server's copy would still carry its roles and `acknowledged_reach`). `ExposedServer::new` is public in `scopes.rs`, and the doc says what scopes.rs already says: code that builds one from anything else is outside what this crate can stop.

**`RemovedServer`**: what `remove` returns. Derives `Clone, Debug, Eq, PartialEq`. Fields `server: RegisteredServer` and `removed_seq: u64`, the seq of its `mcp.server_removed`. Accessors `server(&self) -> &RegisteredServer`, `removed_seq(&self) -> u64`, `into_server(self) -> RegisteredServer`. No public constructor. Its doc cites the API_SPEC preamble: every mutating method returns the `Event.seq` it produced.

## Decision 3: the two events

Kinds, public constants:

```rust
pub const SERVER_ADDED_KIND: &str = "mcp.server_added";
pub const SERVER_REMOVED_KIND: &str = "mcp.server_removed";
```

Both are appended with:
- `actor`: `Actor::Human(operator)`, the identity the caller passes. No method of the registry takes an `Actor`, so no call of the registry records an addition as an agent's or the system's; an addition appended around the registry is refused by the fold (`NotTheOperator`). That `operator` is a human's identity is the caller's to make true: every RPC call is an event with a human actor (`spec/SECURITY_NOTES.md`, trust boundary 1), authenticated by `ori-rpc` (ORI-T-0068, later), and exposure is the operator's act, never an agent's (`spec/ENV_SETUP.md` section 5);
- `ticket_id`: `None`;
- `at`: the timestamp the caller passes (the engine's clock).

**Payloads** are written with `serde_json::to_string` from two private structs that derive `Serialize` and `Deserialize`, each with `#[serde(deny_unknown_fields)]`. The field order below is the declaration order, and serde writes it in that order with no whitespace. **Do not build a payload with `json!`**: without serde_json's `preserve_order` feature it sorts the keys, which breaks the order and test 2. The writer always writes `credential_ref`, as `null` when there is none.

`mcp.server_added`, for example:

```json
{"server_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","name":"github","transport":"stdio","config":{"command":"github-mcp","args":["--read-only"]},"credential_ref":"MCP_GITHUB_01ARZ3NDEKTSV4RRFFQ69G5FAV","exposed_to_roles":["qa"],"acknowledged_reach":true}
```

`config` is a nested struct with the same attribute, fields `command`, `args`. Roles are written with `Role::as_str` and read with `Role::from_str`; the wire struct holds them as `Vec<String>`.

`mcp.server_removed`:

```json
{"server_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","name":"github"}
```

The `name` is there so that a reader of the event stream sees which server went without a lookup. The fold checks it (Decision 4).

**Reading a payload takes two passes.** serde's derive reads a struct from a JSON **array** too, one element per field in declaration order, and `deny_unknown_fields` does not prevent it (the lead's proof shows a seven-element array and a `config` written as `["gh-mcp",["--x"]]` both accepted). So:
1. parse the payload as `serde_json::Value`; answer `Payload` unless the top level is an object and, for `mcp.server_added`, its `config` member is an object;
2. then deserialize **the same text** into the struct with `serde_json::from_str`, which refuses a repeated key (`Value` keeps the last, so only this pass sees it), an unknown key, a missing field other than `credential_ref`, a value of the wrong JSON type, and any text after the object.

What else the proof shows: whitespace around the object is accepted; a missing `credential_ref` reads as `None`; `\u0000` inside a JSON string decodes to a NUL; the serializer writes the fields in declaration order and `null` for `None`. The registry never shows serde's error text, which names the offending key.

**Why these events carry more than call events do.** `spec/API_SPEC.md` section 3's opening paragraph says "The `mcp.*` events carry identifiers and lengths, never agent free text: the content of a call is recorded through the sanitization barrier ..., not in the event". That sentence is about the content of a call, and `spec/SECURITY_NOTES.md`'s "Input validation" calls it the rule that keeps agent free text out of the `mcp.*` events. Nothing in these two events comes from an agent. They also carry more than "Proxied tools" lists for `mcp.server_added` (name, transport, exposed roles, `acknowledged_reach`): the id, the configuration and the credential reference, because `spec/DATA_MODEL.md` makes `McpServer`, whose fields include those, a projection rebuilt from the log (its preamble and section 4). The lead reads the specification this way, and records the wording for ORI-T-0192 (the specification reconciliation); it is not a contradiction to stop on. Say this in the module doc, briefly.

## Decision 4: the registry

```rust
#[derive(Debug)]
pub struct Registry { /* private */ }
```

**No `Clone`.** A copy taken before a `remove` would keep granting the removed server (Decision 4's reading rule below). The fold clones a private `State`, not the `Registry`.

Private fields: the product's `Id`; a private `State` that derives `Clone` and `Debug`, holding `last_seq: u64` (the seq of the last event folded, 0 for none), `last_digest: Option<EventHash>` (that event's `digest()`, `None` for none), the live servers keyed by name (`BTreeMap<String, RegisteredServer>`, so they list in name order), and every server id the log has ever added, live or removed (`BTreeSet<Id>`); and `blocked: Option<(u64, EventDefect)>`, the first event the fold refused, if any.

**`pub fn load(db: &mut ProductDb) -> Result<Registry, RegistryError>`**
- Parses `db.product_id()` with `Id::parse`; failure is `RegistryError::ProductNotAnId`.
- Starts empty and runs the catch-up below. If the catch-up refuses, `load` returns that refusal and no registry: a restarted engine then has no registry to decide on, and exposes nothing.

**`pub fn refresh(&mut self, db: &mut ProductDb) -> Result<(), RegistryError>`**: the catch-up.
1. If `blocked` is set, answer `RegistryError::Event { seq, defect }` for the recorded event, read nothing, and stop. (The log is append-only and the fold is deterministic, so the same event would be refused again.)
2. The database must be this registry's product: `Id::parse(db.product_id())` must succeed and equal the registry's id, else `RegistryError::WrongProduct` (a product id that does not parse is `WrongProduct` here, not `ProductNotAnId`).
3. **A verified read**, as `ori-broker` and `ori-memory` make before folding a grant: `EventLog::verify(db.connection())`, an error being `RegistryError::Log(error)`. Its `VerifyReport` gives `tip_seq` and `tip_digest`. Let `tip` be `tip_seq.unwrap_or(0)` (an empty log is at 0, with no digest); never compare the `Option` itself with `Some(last_seq)`, because `None` orders below every `Some`. Then:
   - `tip < last_seq`, or `tip == last_seq` with `tip_digest != last_digest`: `RegistryError::LogMoved { folded_seq: last_seq }`. This is not the log the registry folded;
   - `tip == last_seq` with the same digest (an empty log at `last_seq` 0 included): nothing new; answer `Ok(())`;
   - `tip > last_seq`: read `EventLog::read_range(db.connection(), last_seq + 1, tip)`, an error being `RegistryError::Log(error)`. The read must hold exactly `tip - last_seq` events, else `RegistryError::Log(EventLogError::ReaderIncomplete { counted: tip - last_seq, read })`, the event log's own answer for a reader that returned fewer rows than it should (`read_range`, unlike `verify`, counts nothing). Take the first and last events with `first()` and `last()`, never by index. The first must carry `hash_prev() == last_digest` (`EventHash::GENESIS` when `last_seq` is 0), and the last's `digest()` must equal `tip_digest`; either mismatch is `LogMoved { folded_seq: last_seq }`. So a `refresh` that answers `Ok(())` has folded up to the tip, which `add` and `remove` rely on when they move `last_seq` to the event they append.
4. Fold the events in order **into a clone of the state**. For each event: if its `product_id()` is not the registry's, the fold refuses it (`EventDefect::ForeignProduct`), **whatever its kind**. Then an event whose `kind()` is exactly `SERVER_ADDED_KIND` or `SERVER_REMOVED_KIND` is applied as below; every other kind is skipped, `mcp.server_addedx`, `mcp.tool_called` and `MCP.SERVER_ADDED` included. **The kind is filtered before the actor is looked at**: a skipped kind is skipped whoever its actor is. After each event, folded or skipped, the clone's `last_seq` and `last_digest` become that event's.
5. On success the clone replaces the state. On the first refusal, the state is unchanged, `blocked` is set to that event's seq and defect, and the answer is `RegistryError::Event { seq, defect }`.

**While `blocked` is set**, permanently for this registry and for any registry loaded from the same log:
- `refresh`, `add` and `remove` answer the recorded `RegistryError::Event { seq, defect }` and append nothing;
- **`exposed` answers `None` for every name**, so no grant is decided on a state the log no longer supports;
- `servers`, `by_name` and `by_id` keep answering the last good state, for the operator's view only;
- `pub fn blocked(&self) -> Option<(u64, &EventDefect)>` lets the engine show it.

**Only an `EventDefect`, from the fold or from an apply, sets `blocked`.** `WrongProduct`, `LogMoved`, `Log` and `Encode` never do, and change nothing: `exposed` keeps answering the last verified state while `add` and `remove` keep failing at their `refresh`. The registry cannot tell its own log, edited past its triggers, from another database handed to it, and an `Sql` error may pass. Code able to rewrite rows could as well append a human `mcp.server_added` the fold accepts, so a tampered log is outside what the registry can stop; a restarted engine's `load` answers the same error and exposes nothing. The doc says so.

The event log is append-only and hash-chained (`spec/DATA_MODEL.md` section 2, `Event`; `CLAUDE.md`: "No code path updates or deletes an event"). So no event can be repaired or removed, and the way out is a human decision carried out as a change to this fold by a later ticket, never a change to the log. The doc says so, and states the cost plainly: **one defective `mcp.server_*` event, appended by any code holding the product's `ProductDb`, stops `add`, `remove`, `load` and every exposure for that product, permanently, until then.** That is the trade operator ruling R45, point 3, accepted for evidence, now made for the registry; the lead records it in `ops/` and puts it to the operator.

**Applying `mcp.server_added`**, checks in this order, each failure an `EventDefect`:
1. the actor is `Actor::Human(_)`, else `NotTheOperator` (an agent's or the system's addition exposes nothing);
2. the payload reads (Decision 3's two passes), else `Payload`;
3. `server_id` parses with `Id::parse`, else `ServerId`;
4. `name` parses with `ServerName::parse`, else `Name(error)`;
5. `transport` parses with `Transport::parse`, and `config` with `StdioConfig::new`, else `Server(error)`;
6. each role parses with `Role::from_str`, else `Role`;
7. `NewServer::new(id, name, transport, config, credential_ref.is_some(), roles, acknowledged_reach)` accepts the whole, else `Server(error)`;
8. the payload's `credential_ref` equals the checked server's own reference: `payload.credential_ref.as_deref() == new_server.credential_ref().map(CredentialRef::as_str)` (`NewServer::new` took `name` and `id` by value and derived the reference as `CredentialRef::for_server(&name, &id)` when `credential_ref` was not null), else `CredentialRef`;
9. the state checks of Decision 5, else `IdTaken` or `NameTaken`.

The new `RegisteredServer` takes `added_at` from the event's `at()`, `added_by` from the actor's identity, and `added_seq` from its `seq()`.

**Applying `mcp.server_removed`**, in order:
1. the actor is `Actor::Human(_)`, else `NotTheOperator`;
2. the payload reads (Decision 3, the object check then the struct), else `Payload`;
3. `server_id` parses, else `ServerId`;
4. a **live** server has that id, else `NotLive` (an id never added, and an id added and already removed, both answer `NotLive`);
5. its name equals the payload's `name` text, else `NameMismatch`.

The server leaves the live map; its id stays in the set of ids ever added.

**Write each apply as a function over the `State`** (for example `fn apply_added<'s>(state: &'s mut State, product_id: &Id, event: &Event) -> Result<&'s RegisteredServer, EventDefect>`), not as a `&mut self` method of `Registry`. **The apply itself sets `state.last_seq` and `state.last_digest` to the event's**, after every check has passed and before it inserts, so a refusal leaves them unchanged; the fold sets them itself for a skipped kind. For `mcp.server_added`, insert with `btree_map::Entry::Vacant(entry) => entry.insert(server)`, which returns the `&RegisteredServer` that `add` returns (the `Occupied` arm cannot be reached after check 9, and answers `NameTaken`). For `mcp.server_removed`, return the server that `BTreeMap::remove` gives back. No `expect` on either. **Do not set `last_seq` or `last_digest` in `add` after the apply returns, and do not set them first and restore them on a refusal**: the reference the apply returns borrows the whole `State` for the rest of `add`, so both fail to compile (rustc E0506; the re-check reproduced it). After the apply, `add` and `remove` touch only `blocked`, a field outside `State`.

**`pub fn add(&mut self, db: &mut ProductDb, at: Timestamp, operator: Id, server: NewServer) -> Result<&RegisteredServer, RegistryError>`**
1. `refresh(db)?` (this also answers a blocked registry and checks the product);
2. the state checks of Decision 5, answered as `RegistryError::IdTaken { id }` or `NameTaken { name }`;
3. serialize the payload with `serde_json::to_string`; an error, which these structs never produce, is `RegistryError::Encode`, and nothing is appended;
4. append it with `EventLog::append(db.connection(), product_id, at, Actor::Human(operator), SERVER_ADDED_KIND, None, payload)`. An append error is `RegistryError::Log(error)`, and the state is unchanged;
5. apply the appended event **through the same apply function the fold uses**; on success it has set `last_seq` and `last_digest` to the appended event's. That apply cannot refuse an event built from a checked `NewServer` after step 2. If it nonetheless does, set `blocked`, leave the state (`last_seq` included) unchanged, and answer `RegistryError::Event { seq, defect }`: the event is in the log, and every later call and every later `load` refuses it the same way. Test 1 (a fresh `load` agrees with what `add` built) is what keeps the two from diverging;
6. return the new server.

**`pub fn remove(&mut self, db: &mut ProductDb, at: Timestamp, operator: Id, id: &Id) -> Result<RemovedServer, RegistryError>`**
1. `refresh(db)?`;
2. a live server has that id, else `RegistryError::NotFound { id }`, nothing appended;
3. serialize and append `SERVER_REMOVED_KIND` with `{"server_id": ..., "name": ...}` as in `add` steps 3 and 4;
4. apply it through the fold's apply function, which sets `last_seq` and `last_digest`, and return `RemovedServer { server, removed_seq }`. On a refusal, as `add` step 5.

**Reading**, none of which touches the database:
- `pub fn product_id(&self) -> &Id`;
- `pub fn servers(&self) -> impl Iterator<Item = &RegisteredServer>`, in name order;
- `pub fn by_name(&self, name: &ServerName) -> Option<&RegisteredServer>`;
- `pub fn by_id(&self, id: &Id) -> Option<&RegisteredServer>`, live servers only;
- `pub fn exposed(&self, name: &ServerName) -> Option<ExposedServer>`: `None` while `blocked` is set, else the named live server's `ExposedServer`;
- `pub fn blocked(&self) -> Option<(u64, &EventDefect)>`.

The doc of `Registry` says:
- **one registry per product**, held by the task that holds the product's `ProductDb` (`spec/LLD.md` section 5, the single writer); every `mcp.server_*` event is appended through it;
- **reading does not refresh**, and a removal takes effect for a decision only when that decision reads the registry `remove` updated. There is no `Clone`, so no snapshot of it can be handed out; S6 (later) takes the `ExposedServer` from the writer's registry at each call, and the engine (later) answers `project.mcp.remove` only once no reader decides on an older state;
- **a server's name is not its identity across removals**: a name may be added again under a new id. H2 (later) keys connections and caches by id, and resolves a grant against the same registry state S6 authorized on;
- `load` reads and verifies the whole log, and each `refresh`, `add` and `remove` verifies it again: O(log length) per call, the phase 1 cost, stated and not optimized.

## Decision 5: the state checks, one function for both paths

A private function checks a `NewServer` against a `State` and is called by **both** `add` (step 2) and the fold (step 9 of applying `mcp.server_added`), so the two paths cannot disagree. In order:
1. the id is in the set of ids ever added, live or removed: the id is taken. An id is never reused, so a removed server's id cannot come back as another server;
2. a live server has the name: the name is taken. Condition 2 of "Proxied tools" makes the server part of `<server>__<tool>` the name given at addition and splits it off at the first `__`; this check makes that name resolve to one live server. The split is condition 2's; the resolution is this rule. A removed server's name may be added again, under a new id.

The function returns a small private enum the two callers map to their own error. Name its variants `Id` and `Name` (for example `enum Conflict { Id, Name }`); do not give them a shared suffix.

No credential reference check is needed: a reference carries its server's id (Decision 2), and ids are never reused.

## Decision 6: the errors

Three public enums, each `#[non_exhaustive]`, each with `pub fn methodology_ref(&self)`, a hand-written `Display` (ruling R20, as `scopes.rs` says it) and `impl std::error::Error`. **No `Display` echoes input text**: no command, argument, credential reference, payload or serde message. A role, a server name or an id may be named. The one exception is `RegistryError::Log`, whose `Display` is the event log's own.

Build each `MethodologyRef` the way `scopes.rs` does: a private `const fn whole_section(section: u8)` over named constants, each constant with a doc line saying why its section applies. Not `MethodologyRef::at(...).unwrap()`.

**`ServerError`**, an addition that breaks a rule. Derives `Clone, Copy, Debug, Eq, Hash, PartialEq`. `methodology_ref(&self) -> MethodologyRef`:

| Variant | Section |
|---|---|
| `TransportNotSupported` | AICD §27 |
| `CommandEmpty`, `CommandTooLong { length: usize }`, `CommandHasControl`, `TooManyArgs { count: usize }`, `ArgTooLong { index: usize, length: usize }`, `ArgHasControl { index: usize }` | AICD §27 |
| `DuplicateRole { role: Role }` | AICD §17 |

**`EventDefect`**, why the fold refuses an event. Derives `Clone, Debug, Eq, PartialEq`. `methodology_ref(&self) -> MethodologyRef`:

| Variant | Section |
|---|---|
| `NotTheOperator` | AICD §17 |
| `ForeignProduct` | AICD §26 |
| `CredentialRef` | AICD §27 |
| `Payload`, `ServerId`, `Role`, `NotLive`, `NameMismatch`, `IdTaken`, `NameTaken` | AICD §8 (the memory architecture; `ori-store`'s `ProjectionError::UnreadablePayload` and `MalformedPayload` cite it for the same kind of refusal) |
| `Name(ServerNameError)` | the inner error's (`ServerNameError::methodology_ref`, AICD §27) |
| `Server(ServerError)` | the inner error's |

**`RegistryError`**, why an operation failed. Derives `Clone, Debug, Eq, PartialEq` (`EventLogError` derives all four). `methodology_ref(&self) -> Option<MethodologyRef>` and `is_refusal(&self) -> bool`:

| Variant | Section | Refusal |
|---|---|---|
| `ProductNotAnId` | AICD §26 | yes |
| `WrongProduct` | AICD §26 | yes |
| `IdTaken { id: Id }` | AICD §13 (one id, one record in the audit trail) | yes |
| `NameTaken { name: ServerName }` | AICD §27 | yes |
| `NotFound { id: Id }` | AICD §17 | yes |
| `Event { seq: u64, defect: EventDefect }` | the defect's | yes |
| `LogMoved { folded_seq: u64 }` | AICD §8 | yes |
| `Encode` | none | no |
| `Log(EventLogError)` | the inner error's (`EventLogError::methodology_ref`), as `RebuildError::EventLog` does | the inner error's (`EventLogError::is_refusal`) |

Check every section number against `methodology/sections.json`: §8 "Memory architecture", §13 "Git and release workflow", §17 "Autonomy tiers and the permission model", §26 "Operating several products with one team", §27 "Security and compliance in depth".

## Decision 7: what H1 does not do

State each in the module doc of `registry.rs`, one line each:
- it connects to no server and launches nothing (H2, later; spawning is `ori-runtime`'s, prerequisite PR1, later);
- it stores, reads and deletes no secret: the engine gives `project.mcp.add`'s `secret` to the broker under the reference this registry derives, and resolving a reference is per product (PR1, later);
- it implements no Client API method: `project.mcp.add`, `list`, `remove` and `test` are the engine's and `ori-rpc`'s (later), and `test` needs H2;
- it proxies nothing: S6 (later) does, deciding with `ToolScopes::authorize_user_tool` on `Registry::exposed`;
- it does not check the operator's seat or authenticate the caller: the Client API's local authentication is `ori-rpc`'s (ORI-T-0068, later), and the registry records the identity it is given.

## Decision 8: rules for the tests' payload text

- Write a planted payload as a **raw string literal**, `r#"..."#`, so that the JSON text holds the characters exactly. For a NUL inside a JSON string, the raw string holds the six characters backslash, `u`, `0`, `0`, `0`, `0`, which serde_json decodes to a NUL. **Never use Rust's `\0`, `\u{0}` or `\u{0000}` escapes in a planted JSON text**, even if rustc suggests one: they put a raw NUL byte in the JSON text, serde_json refuses it as a control character, and the answer is then `Payload`. Assert `!payload.contains('\0')` on such a planted text. With `format!` over a raw string, write the JSON braces as `{{` and `}}`.
- Outside raw strings, never type a backslash followed directly by `u` and four hex digits: a tool layer can decode it before it reaches the file. Where a test calls `StdioConfig::new` directly (not through JSON), Rust's `'\0'` and `'\u{1b}'` are the right way to put a NUL or an ESC in a `String`.

## Decision 9: the docs

Every public item has a doc comment, and every one that implements a rule names its AICD section (CLAUDE.md rule 9). `host.rs`'s module doc says, briefly:
- what the host is: the half of `ori-mcp` that faces the operator's servers, as `server.rs` faces the agents (`spec/LLD.md` section 2's `Host`);
- that `spec/RISK_MAP.md` tiers it 1 (`crates/ori-mcp (host)`), that it holds no policy (who may reach a server is decided in `scopes.rs`, tier 2, on facts the registry supplies), and that ORI-T-0194 was raised to tier 2 for that reason;
- one Mermaid diagram, `flowchart LR`: the operator's `project.mcp.add` reaching the engine, `Registry::add` appending `mcp.server_added` to the product's event log, and S6 taking `Registry::exposed` to `ToolScopes::authorize_user_tool`, whose `UserToolGrant` goes to `UserToolHost`. Mark the engine and S6 as not yet built.

`registry.rs`'s module doc gives Decisions 3 to 5 in prose, the limits of Decision 2 (no secret detection in arguments; no environment; the keychain's one namespace) and Decision 4 (reading does not refresh; a refused event blocks the product for good; the verified read's cost), and Decision 7.

Write no sentence about code that does not exist as if it did. Where a doc names a later slice or ticket, name it as later.

## Tests

In `registry.rs`'s `#[cfg(test)] mod tests`, each named `ori_t_0194_...` after what it pins. Open real product databases in scratch directories, as `server.rs`'s tests do (copy the `Scratch` pattern; name the directories `ori-t-0194-...`). Plant defective events with `EventLog::append` directly; it refuses an empty payload, so use non-empty text such as `not json`. The list is a floor, not a ceiling. Each test asserts the answer it names, not just `is_err()`.

1. **Round trip.** Add three servers: one `with_secret`, one without, and one with `acknowledged_reach` false and roles `[qa, coder]`. Remove one. Keep `servers().cloned().collect::<Vec<_>>()`, drop the registry, drop and reopen the database, and `load` again: the loaded registry's `servers().cloned()` equals that snapshot, field for field, in name order. Then call `refresh` twice on the loaded registry with nothing new appended: both `Ok(())`, `servers()` unchanged.
2. **The added event, exactly.** After one `add`, the log's last event has kind `mcp.server_added`, actor `Actor::Human(operator)`, no ticket id, the `at` given, and a payload equal, byte for byte, to Decision 3's text for that server (with its credential reference `MCP_<NAME>_<ID>`); the returned server's `added_seq()` is that event's seq. Do the same with `with_secret` false: the payload holds `"credential_ref":null`.
3. **The removed event, exactly**, the same way, and `removed_seq()` is the log's last event's seq. The removed server is gone from `servers()`, `by_name`, `by_id` and `exposed`.
4. **Names and ids.** A second live server with the same name and a fresh id: `RegistryError::NameTaken`, and nothing appended (the log's event count is unchanged). Removing the first frees the name: the same name is added again under a new id. Both servers are added with `with_secret` true, so the new server's `credential_ref()` is `Some` and differs from the removed server's. The removed id is refused under another name: `IdTaken`. A live id reused under another name: `IdTaken`.
5. **Credential references.** `CredentialRef::for_server` gives `MCP_GITHUB_<id>` for `github` and `MCP_MY_SERVER_<id>` for `my-server`; a 32-byte name gives a 63-byte reference. Servers `a` and `a-b` added with secrets hold different references; server `a`'s reference does not begin with `MCP_A_B_`.
6. **Remove.** An unknown id: `NotFound`. Removing the same server twice: the second is `NotFound`. Nothing is appended by either.
7. **`StdioConfig` at every bound and in order.** Accepted: a command of 4096 bytes; 64 arguments; an argument of 4096 bytes; an empty argument. Refused: a command of 4097 bytes, `CommandTooLong { length: 4097 }`; empty, `CommandEmpty`; 65 arguments, `TooManyArgs { count: 65 }`. Each of NUL, ESC (`'\u{1b}'`), LF, DEL, U+009B, U+200E, U+2028 and U+202E is refused in a command (`CommandHasControl`) and in an argument (`ArgHasControl` with its index). Indices off zero: `["ok", <4097 bytes>]` gives `ArgTooLong { index: 1, length: 4097 }`, and `["ok", "", "a\0b"]` gives `ArgHasControl { index: 2 }`. Order: a command both too long and holding a NUL answers `CommandTooLong`; an empty command with a NUL argument answers `CommandEmpty`; a command holding a NUL with 65 arguments answers `CommandHasControl`; 65 arguments of which the first holds a NUL answer `TooManyArgs { count: 65 }`; `["a\0b", <4097 bytes>]` answers `ArgHasControl { index: 0 }` (each argument checked in turn); one argument of 4097 bytes holding a NUL answers `ArgTooLong { index: 0, length: 4097 }`.
8. **`StdioConfig`'s `Debug`** prints the command and `2 argument(s)` for two arguments, and contains neither argument's text.
9. **Roles.** `[qa, qa]` refused `DuplicateRole { role: Qa }`; `[coder, qa, coder]` names `Coder`; `[coder, qa, qa, coder]` names `Qa`; an empty list accepted; all seven of `Role::ALL` given in reverse order are accepted and `exposed_to_roles()` returns them in that reverse order.
10. **`Transport::parse`.** `Transport::parse("stdio")` is `Ok(Transport::Stdio)`, and `Transport::Stdio.as_str()` is `"stdio"`; `STDIO`, `http`, `sse`, `""` and `"stdio "` are each `Err(ServerError::TransportNotSupported)`.
11. **The fold refuses, and says where.** For each planted event, `Registry::load` answers `RegistryError::Event { seq, defect }` with the planted event's seq and this defect:
    - an `mcp.server_added` appended with `Actor::Agent(_)`, and one with `Actor::System`: `NotTheOperator`; an `mcp.server_removed` appended by an agent: `NotTheOperator`;
    - payloads, each `Payload`: not JSON; the valid payload written as the JSON array of its seven values; `config` written as `["github-mcp",["--read-only"]]`; an `mcp.server_removed` written as `["<id>","github"]`; an unknown key at the top and one inside `config`; a key twice at the top, and inside `config`; `"config":[],"config":{...}` (a repeated key whose first value is an array); `acknowledged_reach` missing; `acknowledged_reach` as the string `"true"`; a valid payload followed by ` x`;
    - `server_id` not a ULID: `ServerId`;
    - `name` `Aicd-x`: `Name(ServerNameError::FirstCharacter)` (`ServerName::parse` checks the first character before the reserved prefix); `name` `aicdx`: `Name(ServerNameError::ReservedPrefix)`;
    - `transport` `http`: `Server(ServerError::TransportNotSupported)`; a command holding a NUL (Decision 8) and one holding an ESC (the raw string's six characters backslash, `u`, `0`, `0`, `1`, `b`): `Server(ServerError::CommandHasControl)`;
    - a role `admin`: `Role`; a role twice: `Server(ServerError::DuplicateRole { .. })` naming it;
    - `credential_ref` `"MCP_OTHER_01ARZ3NDEKTSV4RRFFQ69G5FAV"` for `github`, `"mcp_x"`, and the right reference for another id: each `CredentialRef`;
    - a second live `mcp.server_added` with the same name and a fresh id: `NameTaken`; a different name with an id already added, live and (separately) removed: `IdTaken`;
    - an `mcp.server_removed` of an id never added, and of an id added and already removed: `NotLive`; of a live id with another name: `NameMismatch`.
12. **The fold skips what is not its own, and accepts what it should.** A log holding `mcp.tool_called`, `mcp.server_addedx`, `MCP.SERVER_ADDED` and `ticket.created` events, each appended with `Actor::Agent(_)` and the payload `not json`, among valid additions, loads, and holds exactly the valid additions. A valid human addition whose payload omits `credential_ref` loads with `credential_ref() == None`, and one with whitespace around its object loads.
13. **Catch-up before the checks.** Load a registry; append a valid human `mcp.server_added` for `github` directly with `EventLog::append`; then `add` another `github`: `NameTaken`. `refresh` alone also brings the planted server into `servers()`.
14. **A refusal changes nothing and blocks exposure.** Load a registry holding one server, `alpha`, exposed to `qa` with `acknowledged_reach` true, and keep a clone of `servers()`. Append directly, in this order: a valid human `mcp.server_added` for `beta` (a new id); a valid human `mcp.server_removed` of `alpha`; an `Actor::Agent(_)` `mcp.server_added` for `gamma` whose payload is otherwise valid. Then:
    - `refresh` answers `RegistryError::Event { seq: <gamma's seq>, defect: EventDefect::NotTheOperator }`;
    - `servers()` still equals the clone (only `alpha`), and `by_name(beta)` is `None`;
    - `blocked()` is `Some((<gamma's seq>, &NotTheOperator))`;
    - `exposed(alpha)` is `None`;
    - `remove` of `alpha` and `add` of a new server both answer the same `Event`, and the log's event count is unchanged;
    - a second `refresh` answers the same `Event`, with the same seq;
    - a fresh `load` of the same database answers the same `Event`.
15. **The wrong database.** A registry loaded from product A's database, given product B's database by `add`, answers `WrongProduct`, and B's log is unchanged. A database opened with the product id `PRODUCT-A` (not a ULID): `load` answers `ProductNotAnId`, and a registry of product A given that database by `refresh` answers `WrongProduct`. After each `WrongProduct`, `blocked()` is `None`, and an `add` given A's own database succeeds.
16. **Another product's event.** Open product A's database; while its log is empty, append one `ticket.created` event carrying product B's id (`Actor::Human`, payload `{}`): `load` answers `Event { seq: 1, defect: ForeignProduct }`, though the kind is not the registry's. In a second fresh database of product A, do the same with a valid human `mcp.server_added` carrying product B's id: the same answer.
17. **Not the log it folded.** Load a registry from product A's database at one scratch root, holding one server. Open a second database for the same product id at another scratch root: with an empty log, `refresh` answers `LogMoved { folded_seq: <the first registry's last_seq> }`; with a log of the same length but different events (another server added), the same answer; with a longer, different log, the same answer (its first unread event's `hash_prev` does not match).
18. **Exposure through the scopes.** For a server added for product A, exposed to `qa` with `acknowledged_reach` true, `ToolScopes::authorize_user_tool` with `Registry::exposed(..)` grants a qa caller of product A, and refuses a coder of A (`ServerNotExposedToRole`) and a qa caller of product B (`ServerNotInProject`). A server added with `acknowledged_reach` false and roles listed: `exposed` carries the roles and `false`, and a listed role is refused `ServerReachNotAcknowledged`. After `remove`, `exposed` is `None`. This is the registry's half of ORI-P1-038's shape; the criterion's own test stays S6's.
19. **Every refusal's section, pinned.** For every variant of the three enums (build one of each), `methodology_ref()` resolves, has no subsection, and its `section` equals Decision 6's table: `ServerError` 27 for every variant except `DuplicateRole` (17); `EventDefect` `NotTheOperator` 17, `ForeignProduct` 26, `CredentialRef` 27, `Payload`/`ServerId`/`Role`/`NotLive`/`NameMismatch`/`IdTaken`/`NameTaken` 8, `Name(e)` equal to `e.methodology_ref()`, `Server(e)` equal to `e.methodology_ref()`; `RegistryError` `ProductNotAnId`/`WrongProduct` 26, `IdTaken` 13, `NameTaken` 27, `NotFound` 17, `LogMoved` 8, `Event { defect, .. }` equal to `defect.methodology_ref()`. `RegistryError::Encode` has none and is not a refusal. `RegistryError::Log(EventLogError::Sql { .. })` has none and is not a refusal; `Log(EventLogError::MissingActor { seq: Some(1) })` carries AICD §13 and is a refusal. Every other `RegistryError` variant is a refusal.
20. **No `Display` echoes input.** Build `ServerError`s, `EventDefect`s and every `RegistryError` variant except `Log` from inputs carrying the marker `ORI0194MARKER` where an input can carry text (a command, an argument, a payload planted and refused): no `Display` contains the marker.
21. **Name order.** Servers added as `zeta`, `alpha`, `mid` list as `alpha`, `mid`, `zeta`.

## Mutation proofs

Make each change in your worktree **only after the `feat(mcp)` commit that adds `registry.rs` and its tests**: `git checkout -- <file>` restores a file from git's index, so on a new file not yet committed or staged it fails ("did not match any file(s) known to git") and leaves the mutation in place. Run the tests, record which test failed, revert with `git checkout -- crates/ori-mcp/src/host/registry.rs` (never `git reset`, never `git stash`), and confirm that `git status --short` prints nothing before the next mutation. A test strengthened because a mutation survived goes in a new commit (`fix(mcp): ...`), never an amend. Each mutation must fail at least one test, and you report which:

1. the fold accepts an `Actor::Agent` addition (remove apply check 1);
2. `add` skips `refresh`;
3. the state check forgets removed ids (checks only live ids);
4. the fold skips the credential reference check (apply check 8);
5. `credential_ref` is written omitted instead of `null` when absent (`#[serde(skip_serializing_if = "Option::is_none")]`);
6. the reader skips the object check (Decision 3's first pass);
7. `refresh` applies events to the state directly instead of to a clone;
8. the fold skips the removal's name check (`NameMismatch`);
9. `exposed` ignores `blocked`;
10. the fold checks the product only for `mcp.server_added` and `mcp.server_removed`;
11. `refresh` skips the anchor (`hash_prev` against `last_digest`, and the digest at an equal tip);
12. `StdioConfig::new` checks only `char::is_control`, not the listed formatting characters.

If one survives, add or strengthen a test until it does not, and say so.

## Checks

Run from the worktree, each writing its output to a file under `target/`, and read its exit status on the next line, never through a pipe:

- `cargo fmt --all --check`
- `cargo clippy -p ori-mcp --all-targets --locked -- -D warnings`
- `CARGO_TARGET_DIR=target/win cargo clippy -p ori-mcp --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ori-mcp --no-deps --locked`
- `cargo test -p ori-mcp --locked`
- `cargo test -p ori-gates --locked`
- `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/gate13.sh /home/user/wt/ORI-T-0194 <BASE> HEAD`: it must end `gate13 exit: 0`

Report each exit status as a number and the test counts.

## Commits

Small commits, each with a Conventional Commit subject in the imperative, a body saying what and why, and these trailers, in this order:

```
Ticket: ORI-T-0194
Spec: API_SPEC.md#3-agent-facing-mcp-server
Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CQ4vWTcpGWb4B2jvnPi4RM
```

A suggested split: `feat(mcp): add the registry of the operator's MCP servers` (types, events, fold, add, remove, and their tests), then `docs(mcp): ...` if a doc-only change follows. Never put two `Ticket:` lines in one commit.

## Rules

- `CLAUDE.md` at the repository root applies. Rule 3: no existing test changes. Rule 6: no dependency is added.
- Every write goes inside `/home/user/wt/ORI-T-0194`. **Never write to `/home/user/ORI-STUDIO`**, the lead's checkout.
- Commit locally only. Never push, never merge, never rebase, amend or reset a commit, never create another branch. Never kill a process by pattern.
- Your budget: 3 attempts, 45 minutes each. If you exhaust it, stop and write a blocked report in the format of AICD appendix A.5 as your final message.
- If a decision here contradicts the code or the specification as you find them, stop and say exactly where, rather than improvising. If a decision is merely incomplete, take the narrowest reading, implement it, and list it under deviations.

## What you hand back

Your final message reports:
- the commit hashes, with subjects;
- each check's exit status as a number, with the test counts;
- the mutation table: each mutation and the test that failed;
- deviations from this package, each with its reason;
- anything not tested, and why (for one: the short-read answer, `ReaderIncomplete` after `read_range`, cannot be reached through the real `read_range`; say so).
