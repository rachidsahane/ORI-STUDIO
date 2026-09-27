//! `ScopeEnforcer`: what each identity may read from memory, decided at query
//! time from the product's own event log, every refusal logged: AICD §25.
//!
//! # What this module owes
//!
//! AICD §25 names the component in its component table: "Scope enforcer: Maps
//! each agent identity to a role and each role to a set of readable sources
//! and filters. Enforced at query time; an agent cannot request outside its
//! scope, and the refusal is logged." Its "Multi-product layout" draws the
//! boundary: "An agent identity belongs to one product; cross-product
//! retrieval exists only for the architect's assistant agent and the
//! organizational drift audit." `spec/PRD.md` section 4.13 carries the first
//! as K-05, "Scope enforcement per identity at query time; refusals logged",
//! and the second as K-09, priority P1, phase 2.
//!
//! The table enforced is `spec/ENV_SETUP.md` section 5, its "Memory scope"
//! column, one cell per identity; the section names this component among the
//! ones that enforce it. `spec/API_SPEC.md` section 3 names the three agent
//! tools that will call it: `aicd_context` (all roles), `aicd_search` (all
//! roles, "Scoped search over allowed layers") and `aicd_evidence` (lead, qa,
//! operations, "Raw evidence, returned labeled untrusted, logged"). None of
//! the three exists yet (escalation E-0005); [`ScopeEnforcer::authorize`] is
//! what they will call. `spec/SECURITY_NOTES.md` "Authorization model" puts
//! the enforcement here: permissions are "enforced by the component that owns
//! the resource (... memory for retrieval ...)".
//!
//! The criteria this module serves are ORI-P1-022 (a coder's `aicd_context`
//! package holds only canonical, organizational, operational records for the
//! modules it touches and their code map, no evidence blob) and ORI-P1-036
//! (`memory.search` in one product never returns another product's records),
//! both in `spec/criteria/phase-1.md`.
//!
//! # Where the table lives, and the one question asked of `ori-core`
//!
//! AICD §17's matrix, in `ori_core::permission`, has no memory-layer column.
//! The ruling for this ticket (ORI-T-0038) is that the memory scope table
//! belongs in this module and `ori-core` is not edited: [`grant`] is that
//! table, written as one exhaustive `match` on [`Role`] and, inside each arm,
//! one exhaustive `match` on [`Source`], with no wildcard arm anywhere, so a
//! new role or a new source fails to compile here until someone decides who
//! reads it, instead of inheriting a grant. The one AICD §17 question this
//! module does ask `ori-core` is whether a role may read a ticket at all
//! (`Resource::Tickets`, `Action::Read`), for a context request: the coder's
//! "Read own" and product signal's missing read come from there, not from a
//! second copy of the matrix.
//!
//! # The sources and the grants
//!
//! Every source `spec/ENV_SETUP.md` section 5's column names has a [`Source`]
//! value, including the five no module produces yet (organizational
//! knowledge, merged diffs, analytics summaries, infrastructure ADRs, code
//! read), so that no later ticket can add a source without first deciding
//! who reads it. "Canonical" is AICD §8's layer 1, "Product brief,
//! architecture, ADRs, domain model, API contracts, conventions, runbooks",
//! plus the acceptance criteria; it is split into six sources so the roles
//! whose cells name only a part of it (operations: runbooks and
//! infrastructure ADRs; product signal: the brief) can be granted that part.
//!
//! "yes" is the whole source within the product, "scope" is filtered to the
//! coder's declared scope, "migration" is granted only while the product is
//! being migrated, "no" is refused. The `operator` column is `Actor::Human`
//! (see "The operator" below). `raw_evidence` is never part of a context,
//! search or read authorization, whatever this table says: AICD §8 shows raw
//! evidence "only on explicit request", so "yes" there means an explicit
//! [`Query::Evidence`] request is granted, labeled untrusted, and logged.
//!
//! | Source | coder | lead | qa | operations | documentation | product_signal | assistant | operator |
//! |---|---|---|---|---|---|---|---|---|
//! | `canonical_section` | yes | yes | yes | no | yes | no | yes | yes |
//! | `adr` | yes | yes | yes | no | yes | no | yes | yes |
//! | `criterion` | yes | yes | yes | no | yes | no | yes | yes |
//! | `brief` | yes | yes | yes | no | yes | yes | yes | yes |
//! | `runbook` | yes | yes | yes | yes | yes | no | yes | yes |
//! | `infrastructure_adr` | yes | yes | yes | yes | yes | no | yes | yes |
//! | `organizational` | yes | yes | no | no | yes | no | yes | yes |
//! | `operational_record` | scope | yes | no | no | no | no | yes | yes |
//! | `operational_defect` | scope | yes | yes | no | no | no | yes | yes |
//! | `incident` | scope | yes | no | yes | no | no | yes | yes |
//! | `code_map` | scope | yes | yes | no | yes | no | yes | yes |
//! | `merged_diff` | no | yes | no | no | yes | no | yes | yes |
//! | `analytics_summary` | no | yes | no | no | no | yes | yes | yes |
//! | `raw_evidence` | no | yes | yes | yes | no | no | no | yes |
//! | `code_read` | no | yes | migration | no | no | no | yes | yes |
//!
//! Three readings of the specification text are decisions, each stated here
//! and each checked against the specification file by the test module:
//!
//! - The lead's "All layers for the product plus organizational" is every
//!   source, and the assistant's "Everything the operator can read" is every
//!   source but raw evidence, because `spec/API_SPEC.md` section 3 grants
//!   `aicd_evidence` to lead, qa and operations only and is the narrower text.
//!   qa and operations are granted raw evidence by that same row although
//!   their "Memory scope" cells do not name it.
//! - The coder's "code map" is filtered to its declared scope, because
//!   ORI-P1-022 says "the code map of M"; the other roles' "code map" is not.
//! - `operational_defect` is a [`RecordKind::Finding`] record: the six record
//!   kinds of `spec/DATA_MODEL.md` section 2 have no "defect" kind, and a
//!   finding is the kind a QA run report files. Incidents and post-mortems are
//!   `incident`; closing reports, blocked reports and escalation decisions are
//!   `operational_record`.
//!
//! One conflict between two texts is not decided here but met in the
//! refusing direction. `spec/API_SPEC.md` section 3 lists `aicd_context` for
//! "all", and AICD §17's permission matrix gives product signal's Tickets
//! cell as "Create product signal", with no read. A context request is a
//! ticket read (the coder's "Read own" is enforced on it the same way), so
//! product signal's is refused, `ticket_read_not_granted`, until the
//! specification says which text governs. Its `aicd_search` is not a ticket
//! read and is granted over its row.
//!
//! The table in this doc comment is itself checked against [`grant`] by
//! `tests::ori_t_0038_module_doc_table_matches_the_grant_function`, and
//! [`grant`] against the specification by
//! `tests::ori_t_0038_grant_table_matches_env_setup_memory_scope_column` and
//! `tests::ori_t_0038_raw_evidence_grants_match_api_spec_aicd_evidence_roles`,
//! so neither the prose nor the code can drift from the specification
//! silently.
//!
//! # Where every fact comes from, and never from the request
//!
//! - **The identity**: [`Principal`], built by the transport that
//!   authenticated the caller: `ori-mcp` for an agent session, `ori-rpc` for
//!   the operator. [`MemoryRequest`] carries no principal, so what an agent
//!   sends cannot become one.
//! - **The role**: the identity's `identity.created` event in this product's
//!   own log, the payload `crates/ori-broker/src/registration.rs` writes
//!   (`id`, `product_id`, `role`, `model`, `family`, `runtime`). This crate
//!   reads the log through `ori-store` and does not depend on `ori-broker`,
//!   so it reads every such event, whoever it registers, no more loosely
//!   than that file does: all six fields required, no other field and none
//!   twice, the payload byte for byte what its writer writes for the values
//!   read (its reader takes the first `"key":"` in the text, so any other
//!   layout could read differently there), and each value held to its
//!   rules: two identifiers, a role, a family `ModelFamily::parse` accepts,
//!   and the runtime `acp` or `headless`. Anything else is refused,
//!   `identity_record_malformed`, for every requester, as that file's reader
//!   refuses the whole log over it.
//! - **The product**: the [`ProductDb`] the call is made against. Never a
//!   field of the request.
//! - **The coder's declared scope**: the rows
//!   `crates/ori-store/src/projections/lock.rs` folds for the coder's ticket
//!   from this product's `lock.claimed` and `lock.released` events, folded
//!   here the same way and in the path form
//!   `crates/ori-orchestrator/src/lock_table.rs` claims. That projection
//!   keys a live claim by its module text, as its own payload reader reads
//!   it: a later claim of the same text by another ticket takes the row
//!   over, and the coder's ticket no longer holds it, even once the other
//!   ticket releases it. A release is by ticket and total, and the ticket is
//!   the event's own ticket column, never a payload field. This module
//!   agrees with that keying rather than refusing on it: two tickets live on
//!   the same module text is a state `LockTable::claim` refuses to write, and
//!   the projection's rows are the engine's record of who holds what. A live
//!   claim of the coder's ticket that the projection's reader cannot read,
//!   or that serde reads as another module, or whose module is not a path
//!   [`ModulePath::parse`] accepts, leaves its scope unknown, and is refused,
//!   `lock_record_malformed`, until the ticket's release. A `lock.*` event
//!   with no ticket column is refused, `lock_record_unattributed`, as that
//!   projection refuses it: skipped, another ticket's claim outside the
//!   scope would vanish from that ticket's history and let its records in,
//!   and a release of the coder's own ticket would leave its claims live.
//!   What the agent says its scope is never enters the decision.
//! - **Which ticket a coder works**: from the log where the log records it,
//!   from the engine otherwise. The log can record it through two events:
//!   `credential.issued` ties an identity to a session
//!   (`crates/ori-broker/src/issuance.rs`), and `lock.claimed` may carry the
//!   `session_id` holding the claim (`spec/DATA_MODEL.md` section 2's
//!   `LockEntry`). A ticket claimed under a session the identity holds a
//!   live credential for is a ticket the log assigns it. Live is
//!   `issuance.rs`'s own `is_active`: issued to the identity, not revoked,
//!   and not past its `expires_at` at the time the request is decided.
//!   `issuance.rs` says an issuance past its `expires_at` "is treated as dead
//!   ... whether or not it was ever formally revoked", and
//!   `spec/DATA_MODEL.md` section 4 has every issuance expire with its
//!   session. A session is live while any one of its issuances to the
//!   identity is. A `credential.revoked` event ends every issuance of its
//!   session issued before it, whichever issuance ids it lists, because the
//!   broker writes one only from `revoke_session`, which lists every
//!   unrevoked issuance of the session. A credential event is read the way
//!   `issuance.rs` reads it back, every field it writes required
//!   (`expires_at` present, a number or `null`), no other field and none
//!   twice, and the payload byte for byte what its writer writes for the
//!   values read, since its reader, like registration.rs's, takes the first
//!   `"key":` in the text: one it would refuse as malformed, or could read as
//!   another identity's or another session's, is one whose revocation cannot
//!   be relied on, and is refused here, `assignment_record_malformed`. While the identity holds a live session,
//!   so is a lock claim whose session cannot be read, since it may be the
//!   identity's. Today no module writes `lock.claimed` at all (`LockTable`
//!   is a pure function, and
//!   no `AgentSession` row of `spec/DATA_MODEL.md` section 2 is recorded), so
//!   in practice the ticket is the engine-supplied value,
//!   [`Principal::with_assigned_ticket`]. Whoever builds the principal owns
//!   that value: `ori-mcp`, whose tool scopes are tier 2 (`spec/RISK_MAP.md`),
//!   sets it from the session the engine spawned for the ticket and never
//!   from a tool argument. When the log does record tickets for the
//!   identity, the engine's value must be one of them, and with no engine
//!   value the log's must be exactly one; anything else is refused. Whichever
//!   ticket that settles on stands only when nothing in the log ties it to
//!   another identity: a `lock.claimed` event of the ticket naming a session
//!   the log ever issued to another identity, live, revoked or expired, is a
//!   refusal, `assignment_held_elsewhere`, and so is, while the log issues
//!   any session to another identity, a claim of the ticket whose session
//!   cannot be read, `assignment_record_malformed`. Without that, the
//!   identity's own sessions tying nothing once revoked or expired, or
//!   before they claim anything, would leave the engine's value unchecked,
//!   and revoking a misbehaving agent's credential would turn a refusal into
//!   a grant of another identity's ticket. A ticket handed on from one
//!   identity to another is therefore refused to the second until the log
//!   can tell the two apart. The ticket must be filed in this product either
//!   way, and its declared scope is read from the log, never supplied with
//!   it. A claim's session is read both by serde and the way the lock
//!   projection's payload reader reads it, and a claim the two read apart is
//!   one whose session cannot be read.
//! - **What the engine does not record yet**: no module writes
//!   `ticket.filed` or `lock.claimed` outside tests today; only the
//!   `ori-store` tests append them. Until the engine records `ticket.filed`,
//!   every context request and every coder request is refused,
//!   `ticket_not_in_product`. Until it records `lock.claimed`, a coder's
//!   declared scope is empty, so its scope-filtered sources admit nothing but
//!   its own ticket's records. Both fail closed, and writing either event is
//!   not this module's work: the grants above are reachable in a product only
//!   once the engine records them.
//! - **The time**: `at`, the time [`ScopeEnforcer::authorize`] decides at,
//!   supplied by its caller, which owns it as it owns the principal: `ori-mcp`
//!   reads it from the engine's clock, never from a tool argument. It stamps
//!   the events this module appends and decides which credentials have
//!   expired, as `issuance.rs`'s `is_active(now)` takes its time from its
//!   caller.
//! - **Whether the product is being migrated**: the log records no phase
//!   either, so it is engine-supplied too, as [`ScopeEnforcer::new`]'s
//!   [`ProductStage`], owned by the composition root that reads it from
//!   `ori-flows`' phase control. It decides only qa's migration-only code
//!   read.
//! - **What the request says about itself**: [`Claims`], recorded in the
//!   refusal event and compared, never believed. A claim the principal or the
//!   log contradicts (another product, another identity, another role, a
//!   different declared scope) is refused and logged; a claim that agrees
//!   changes nothing. That is where an impersonation attempt shows up in the
//!   audit trail instead of disappearing.
//!
//! # Failing closed
//!
//! Every unknown is a refusal, logged: [`Actor::System`]; an identity with no
//! `identity.created` event in this product's log; any `identity.created`
//! event `crates/ori-broker/src/registration.rs` would refuse or could read
//! differently (it cannot be told whose it is, or what role); an identity
//! recorded under a product other than this one, or under two roles; a role
//! or a source the table does not name; a log that does not verify; a coder
//! with no ticket from the log or the engine, an engine ticket the log
//! contradicts, two log tickets and no engine ticket, a ticket the log ties
//! to another identity, a credential event
//! `crates/ori-broker/src/issuance.rs` would not read back (any field it
//! writes missing or of another type, `expires_at` included) or could read
//! differently, a live session of the identity the log also issues to another
//! identity, a lock claim whose session cannot be read while the identity
//! holds a live session or, for a claim of the assigned ticket, while the log
//! issues any session to another identity, a `lock.*` event with no ticket
//! column, a ticket not filed in this product, or a live lock claim of it
//! that does not parse; a ticket read limit from AICD §17 this module cannot
//! check; an evidence request for a record id more than one record carries,
//! or whose evidence cannot be told apart from another submission's. An
//! expired credential is not an unknown: it is dead, and ties no ticket to
//! its identity's live sessions, though what was claimed under it stays that
//! identity's, so no other identity is assigned it. Results are filtered by
//! the same rule as requests ([`Authorization::filter`]): a result this
//! module cannot classify, cannot place in the product, or cannot place
//! inside a coder's declared scope is dropped.
//!
//! # The operator, the engine, and other products
//!
//! [`Actor::Human`] is the operator: `spec/ENV_SETUP.md` section 5's operator
//! row reads "Everything", and `spec/adr/ADR-0002-single-operator.md` records
//! one human holding every seat. Authenticating that human is the client
//! transport's job, not this module's: `spec/SECURITY_NOTES.md` "Trust
//! boundaries" has local clients authenticate "with a per-installation token
//! stored in the keychain", and `ori-rpc`, which will carry it, is not built
//! yet. Whoever builds an operator [`Principal`] owns that check. The
//! operator reads every source of the product the call is made against, raw
//! evidence through an explicit request only, logged like anyone else's.
//!
//! [`Actor::System`] reads nothing through this module. `spec/DATA_MODEL.md`
//! section 4 admits `system` "only for scheduled triggers and watchers"; a
//! watcher reads the tree through `ori-watch`, and the organizational drift
//! audit's cross-product retrieval is K-09, phase 2.
//!
//! No role gets cross-product retrieval in phase 1: that is K-09. A request
//! made against a product whose log does not register the identity is
//! refused and logged in that product's log, and every result is checked for
//! the product it came from before it is admitted.
//!
//! # Module paths
//!
//! Declared scope matching is by path component, never by string prefix:
//! `crates/ori-memory` covers `crates/ori-memory/src/x.rs` and never
//! `crates/ori-memory-evil/x.rs`. [`ModulePath::parse`] refuses an empty
//! path, an absolute one (a leading `/`, or a `:` in the first component, a
//! drive letter), a backslash anywhere, a `.`, `..` or empty component, a
//! `*`, surrounding whitespace and control characters, and normalizes one
//! trailing `/` away. The component rule is `ori_core::types::Scope`'s own
//! (`Scope::claims` names a directory or a path under it on `/` boundaries);
//! the stricter parse is added here because a scope decision must refuse
//! what `Scope::new` would store.
//!
//! Case is compared exactly, byte for byte, with no Unicode normalization.
//! Git records paths case-sensitively, and exact comparison fails in the
//! refusing direction: `Crates/ori-memory/x.rs` is outside a scope of
//! `crates/ori-memory`, never inside it. Folding case would widen a scope on
//! every case-sensitive checkout, which is the direction a scope may not be
//! wrong in.
//!
//! # Which records are inside a declared scope
//!
//! An operational record carries the ticket it was filed on and no module, so
//! a coder's scope-filtered record sources (`operational_record`,
//! `operational_defect`, `incident`) are filtered by ticket. A record is
//! inside the declared scope when its ticket is the coder's own, or when its
//! ticket claimed at least one module and every module it ever claimed,
//! released or not, lies within the declared scope. The declared scope is the
//! rows the lock projection keeps for the coder's ticket (see "Where every
//! fact comes from"), so a module another ticket took over is not in it, and
//! that ticket's records are not the coder's for having claimed it. A ticket
//! that also claimed anything outside it is outside, a parent directory such
//! as `crates` included, and so is a ticket with a claim that does not parse,
//! a ticket that claimed nothing, and a record with no ticket. Containment is
//! the refusing direction: such a ticket's records may be about modules the
//! coder may not read, and a record cannot be split into the part that is
//! about M and the part that is not. Past tickets that worked only inside the
//! declared scope are its operational history, which is what ORI-P1-022 calls
//! the records "for M". Which ticket made a claim is the lock event's own
//! ticket column; a `lock.*` event without one refuses the request outright,
//! `lock_record_unattributed`, rather than dropping a claim from some
//! ticket's history and so admitting that ticket's records.
//!
//! # Raw evidence, and which blobs are a record's
//!
//! An evidence request names a record, and `barrier::NewReport`'s
//! `record_id` is supplied by the caller: nothing makes it unique. The
//! enforcer refuses when more than one `memory.record_created` event in this
//! product's log carries the id (`record_id_ambiguous`), checks that one
//! record against the reader's scope, and hands over only the blobs that
//! record was submitted with, never every blob that names the id.
//!
//! The log records no submission, so which `memory.evidence_stored` events
//! are a record's is read from how `barrier::submit_report` writes them: one
//! per field, appended immediately before the record's own event, by the
//! same actor, stamped with the same time, through the single writer task
//! `spec/LLD.md` section 5 describes. The record's blobs are that run. An
//! evidence event naming the id anywhere else, such as one left by a
//! submission that failed after storing some of its evidence, is not the
//! record's and is never handed over. The request is refused
//! (`evidence_not_attributable`) when the run cannot be the record's alone:
//! it holds more events than the record has fields, which is what a failed
//! submission by the same actor at the same time leaves immediately before
//! it; an evidence id in it is named by any other evidence event, since the
//! evidence file is named by that id and may hold another submission's raw
//! text; or an evidence event does not parse. Were a record's evidence ever
//! appended apart from it, the run would come up short and the read would be
//! refused, never widened.
//!
//! What the log cannot show is not checked here: `barrier::submit_report`
//! writes an evidence file before it appends the event, so a submission that
//! failed between the two may have overwritten a file with no event to say
//! so. Closing that belongs to `barrier.rs`, whose evidence ids are
//! caller-supplied too.
//!
//! # What is logged
//!
//! Every refusal appends exactly one [`REFUSAL_EVENT_KIND`] event to the log
//! of the product the request was made against, with the requesting actor as
//! the event's actor and a payload naming the request, the reason and the
//! [`MethodologyRef`] (AICD §25). Every granted raw evidence read appends one
//! [`EVIDENCE_ACCESS_EVENT_KIND`] event before the evidence is handed over. If
//! either append fails, the request is refused all the same, and the failure
//! is surfaced as [`ScopeError::RefusedUnlogged`]: a logging failure never
//! becomes an allow.
//!
//! No payload carries agent free text. A search query is recorded by its
//! length, never its text; a source or role name the table does not know is
//! recorded as a count, and a claimed declared scope by its number of
//! entries: `spec/SECURITY_NOTES.md` "Secrets" says secrets are
//! "never serialized into events", and an agent's free text can carry a
//! credential it was issued.
//!
//! # The seam the retrieval stands on
//!
//! ORI-T-0039's `Retrieval` assembles context packages only through this
//! module. [`Authorization`] has no public constructor and no public field:
//! the only way to hold one is for [`ScopeEnforcer::authorize`] to have
//! returned it. It carries what the retrieval needs (the reader and its role,
//! the product, the authorized query, the ticket and the coder's declared
//! scope where they apply, the allowed sources with their filters, and for an
//! evidence request the evidence blobs), and [`Authorization::filter`] is the
//! result filter the retrieval must apply to everything it gathered before
//! returning it. The retrieval queries only [`Authorization::sources`], and
//! filters anyway, because a check on the request alone would let a broad
//! search return what the request could not have named.
//!
//! What the compiler holds, and what only ORI-T-0039's review can:
//!
//! - The request side is the compiler's: no [`Authorization`] exists that
//!   [`ScopeEnforcer::authorize`] did not decide, and none can be altered
//!   after. Each sealed value's doc comment pins that with one example per
//!   field that fails to compile for no other reason than the field's
//!   privacy, and
//!   `tests::ori_t_0038_sealed_values_keep_every_field_private_and_pinned_by_a_single_reason_example`
//!   refuses any visibility on their fields, `pub(crate)` included, since
//!   ORI-T-0039's retrieval will sit in this crate.
//! - The result side is the compiler's as far as the package's own type
//!   reaches. [`Authorization::filter`] returns an [`Admitted`], which has
//!   no public constructor either and records the product and the reader it
//!   was admitted for. A package type that holds only [`Admitted`] values,
//!   and [`Authorization::evidence`] for an evidence request, cannot be
//!   handed an unfiltered result. Choosing that type is ORI-T-0039's work,
//!   and its review has to check that it does.
//! - Nothing in the type system stops a retrieval from reading around this
//!   module: `barrier::read_records`, `barrier::read_evidence`, the indexer
//!   and the code map are all public. The review has to check that nothing
//!   they return reaches a package except through [`Authorization::filter`]
//!   or [`Authorization::evidence`].
//! - The filter decides on fields it cannot check. Only a
//!   [`Candidate::Record`] and a [`Candidate::Evidence`] hold values that
//!   `barrier.rs` alone builds, from the log, with no public constructor.
//!   `SearchHit` and `code_map::Module` are plain structs with every field
//!   public, which any caller, another crate included, can build or change;
//!   they carry no product; and no module produces the unproduced sources
//!   yet. Every field of the other three variants is therefore the
//!   retrieval's word, not only their labels, and the filter's product,
//!   source and declared scope checks on them are only as good as the
//!   retrieval's handling. The review has to check that each such candidate
//!   is built from the index, code map or store of the
//!   [`Authorization::product_id`] it is filtered under, with the hit or the
//!   module exactly as that index or code map returned it, and with each
//!   label set from where the item was read, never from the item's content.
//!
//! The table is every field the filter decides on and every field it passes
//! through unread, per [`Candidate`] variant.
//! `tests::ori_t_0038_module_doc_names_every_candidate_field_the_result_filter_decides_on`
//! changes each field in turn and fails when the filter's decisions and this
//! table disagree.
//!
//! | Candidate | Field | Decides | Checked here |
//! |---|---|---|---|
//! | Candidate::Record | the record | product, source, declared scope (by its ticket) | yes: only `barrier.rs` builds one, from the log |
//! | Candidate::Evidence | the blob | nothing: never admitted | yes: only `barrier.rs` builds one, from the log |
//! | Candidate::Document | `product_id` | product | no: the retrieval's word |
//! | Candidate::Document | `hit.kind` | source | no: the retrieval's word |
//! | Candidate::Document | `hit.path` | source, declared scope | no: the retrieval's word |
//! | Candidate::Document | `hit.title`, `hit.score` | nothing: passed through unread | no: the retrieval's word |
//! | Candidate::Module | `product_id` | product | no: the retrieval's word |
//! | Candidate::Module | `module.path` | declared scope | no: the retrieval's word |
//! | Candidate::Module | every other field of `code_map::Module` | nothing: passed through unread | no: the retrieval's word |
//! | Candidate::Unproduced | `product_id` | product | no: the retrieval's word |
//! | Candidate::Unproduced | `source` | source | no: the retrieval's word |
//! | Candidate::Unproduced | `locator` | nothing: passed through unread | no: the retrieval's word |
//!
//! ```mermaid
//! sequenceDiagram
//!   participant Agent as agent session (untrusted)
//!   participant Mcp as ori-mcp server (tier 2)
//!   participant Enf as ScopeEnforcer (this module)
//!   participant Log as the product's event log
//!   participant Ret as Retrieval (ORI-T-0039)
//!   Agent->>Mcp: aicd_context, aicd_search or aicd_evidence
//!   Mcp->>Mcp: Principal from the authenticated session, never from arguments
//!   Mcp->>Enf: authorize(db, at, principal, request)
//!   Enf->>Log: read identity.created, ticket.filed, lock and credential events
//!   alt refused
//!     Enf->>Log: append memory.access_refused
//!     Enf-->>Mcp: ScopeError, carrying its MethodologyRef
//!   else raw evidence granted
//!     Enf->>Log: append memory.evidence_accessed
//!     Enf-->>Mcp: Authorization holding the evidence blobs, untrusted
//!   else granted
//!     Enf-->>Mcp: Authorization
//!   end
//!   Mcp->>Ret: assemble the package under the Authorization
//!   Ret->>Ret: query only Authorization::sources
//!   Ret->>Enf: Authorization::filter(candidates)
//!   Enf-->>Ret: Admitted, only the admitted candidates
//!   Ret-->>Mcp: the context package, holding Admitted values only
//! ```
//!
//! # What this module does not do
//!
//! - `spec/ENV_SETUP.md` section 7's stricter migration column (AICD §24.2)
//!   is not enforced here beyond qa's migration-only code read that section
//!   5's column itself states. Lead and coder identities do not exist during
//!   a migration (section 7 says so), and the other rows would need a ticket
//!   of their own.
//! - An identity's suspension: no event records one yet.
//! - The per-call event `spec/API_SPEC.md` section 3 requires ("Every call is
//!   an `Event` with the identity as actor") is the MCP server's; this module
//!   logs refusals and evidence reads.
//! - Classifying an ADR as an infrastructure ADR, and indexing the
//!   organizational layer, merged diffs, analytics summaries or code: no
//!   module produces them yet, so they reach the filter only as
//!   [`Candidate::Unproduced`].
//!
//! Must not: return unsanitized production content in a package
//! (`spec/LLD.md` section 2, this crate's own doc comment). Raw evidence
//! leaves this module only through an explicit, logged evidence request, as
//! [`EvidenceBlob`]s that expose no text.

use core::fmt;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use ori_core::error::MethodologyRef;
use ori_core::permission;
use ori_core::permission::Action;
use ori_core::permission::Constraint;
use ori_core::permission::Decision;
use ori_core::permission::Resource;
use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::ModelFamily;
use ori_core::types::Role;
use ori_core::types::Scope;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use ori_store::event_log::EventLogError;
use serde::Deserialize;
use serde::Serialize;

use crate::barrier;
use crate::barrier::BarrierError;
use crate::barrier::EvidenceBlob;
use crate::barrier::MemoryRecord;
use crate::barrier::RecordKind;
use crate::code_map;
use crate::indexer::DocumentKind;
use crate::indexer::SearchHit;
use crate::indexer::document_file;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The event kind every refusal is recorded under: AICD §25, "the refusal is
/// logged".
pub const REFUSAL_EVENT_KIND: &str = "memory.access_refused";

/// The event kind a granted raw evidence read is recorded under: AICD §8,
/// raw evidence is shown "only on explicit request", and `spec/API_SPEC.md`
/// section 3 says `aicd_evidence` is "logged".
pub const EVIDENCE_ACCESS_EVENT_KIND: &str = "memory.evidence_accessed";

/// AICD §25, the memory service: every refusal this module makes cites it.
const MEMORY_SERVICE_SECTION: u8 = 25;

/// AICD §8, the memory architecture and its raw evidence rule.
const MEMORY_ARCHITECTURE_SECTION: u8 = 8;

/// The event kinds this module reads from the log, as the modules that write
/// them spell them.
const IDENTITY_CREATED: &str = "identity.created";
const TICKET_FILED: &str = "ticket.filed";
const LOCK_CLAIMED: &str = "lock.claimed";
const LOCK_RELEASED: &str = "lock.released";
/// Every event kind `crates/ori-store/src/projections/lock.rs` owns begins
/// with this.
const LOCK_KIND_PREFIX: &str = "lock.";
const CREDENTIAL_ISSUED: &str = "credential.issued";
const CREDENTIAL_REVOKED: &str = "credential.revoked";
const RECORD_CREATED: &str = "memory.record_created";
const EVIDENCE_STORED: &str = "memory.evidence_stored";

/// The brief's file, as components, so no path string is written here whole.
const BRIEF_FILE: [&str; 2] = ["spec", "PROJECT_BRIEF.md"];

/// The runbooks' directory, as components.
const RUNBOOK_DIR: [&str; 2] = ["spec", "runbooks"];

/// The directory every canonical document the indexer walks sits under.
const CANONICAL_ROOT: [&str; 1] = ["spec"];

/// The reference every refusal of this module carries: AICD §25.
const fn memory_service_ref() -> MethodologyRef {
    MethodologyRef {
        section: MEMORY_SERVICE_SECTION,
        subsection: None,
    }
}

// ---------------------------------------------------------------------------
// Source, Filter, Access, and the table
// ---------------------------------------------------------------------------

/// One readable source of memory: AICD §25, "each role to a set of readable
/// sources and filters".
///
/// One value per source `spec/ENV_SETUP.md` section 5's "Memory scope" column
/// names, with AICD §8's canonical layer split into the six parts some cells
/// name alone. See the module doc comment, "The sources and the grants".
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Source {
    /// A section of a canonical specification document other than the brief
    /// and the runbooks: AICD §8 layer 1.
    CanonicalSection,
    /// An architecture decision record not classified as infrastructure.
    Adr,
    /// An acceptance criterion.
    Criterion,
    /// The product brief.
    Brief,
    /// A runbook.
    Runbook,
    /// An ADR about infrastructure. No module classifies one yet.
    InfrastructureAdr,
    /// Organizational knowledge, AICD §8 layer 2. No module indexes it yet.
    Organizational,
    /// A closing report, blocked report or escalation decision: AICD §8
    /// layer 3.
    OperationalRecord,
    /// A defect in operational memory: a finding.
    OperationalDefect,
    /// An incident or a post-mortem.
    Incident,
    /// The code map (AICD §25, "Code map builder").
    CodeMap,
    /// A merged diff. No module produces one yet.
    MergedDiff,
    /// An analytics summary. No module produces one yet.
    AnalyticsSummary,
    /// Raw evidence, AICD §8: served only on an explicit, logged request.
    RawEvidence,
    /// The product's source code itself, read in full rather than through the
    /// code map. No module serves it yet.
    CodeRead,
}

impl Source {
    /// Every source, in the order the module doc comment's table lists them:
    /// AICD §25.
    pub const ALL: [Self; 15] = [
        Self::CanonicalSection,
        Self::Adr,
        Self::Criterion,
        Self::Brief,
        Self::Runbook,
        Self::InfrastructureAdr,
        Self::Organizational,
        Self::OperationalRecord,
        Self::OperationalDefect,
        Self::Incident,
        Self::CodeMap,
        Self::MergedDiff,
        Self::AnalyticsSummary,
        Self::RawEvidence,
        Self::CodeRead,
    ];

    /// The name a request uses for this source, and the one a logged event
    /// records: AICD §25.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalSection => "canonical_section",
            Self::Adr => "adr",
            Self::Criterion => "criterion",
            Self::Brief => "brief",
            Self::Runbook => "runbook",
            Self::InfrastructureAdr => "infrastructure_adr",
            Self::Organizational => "organizational",
            Self::OperationalRecord => "operational_record",
            Self::OperationalDefect => "operational_defect",
            Self::Incident => "incident",
            Self::CodeMap => "code_map",
            Self::MergedDiff => "merged_diff",
            Self::AnalyticsSummary => "analytics_summary",
            Self::RawEvidence => "raw_evidence",
            Self::CodeRead => "code_read",
        }
    }

    /// Reads a source name back, `None` for any name the table does not
    /// carry: AICD §25.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|source| source.as_str() == text)
    }

    /// Whether a module of this crate produces items of this source as a
    /// typed value today. Such items reach [`Authorization::filter`] only as
    /// that type, which the filter classifies itself; a
    /// [`Candidate::Unproduced`] naming one of these sources is refused, so a
    /// caller cannot relabel a typed item into a source it is not.
    const fn has_typed_producer(self) -> bool {
        match self {
            Self::CanonicalSection
            | Self::Adr
            | Self::Criterion
            | Self::Brief
            | Self::Runbook
            | Self::OperationalRecord
            | Self::OperationalDefect
            | Self::Incident
            | Self::CodeMap
            | Self::RawEvidence => true,
            Self::InfrastructureAdr
            | Self::Organizational
            | Self::MergedDiff
            | Self::AnalyticsSummary
            | Self::CodeRead => false,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The filter a grant carries: AICD §25, "readable sources and filters".
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Filter {
    /// The whole source, within the product.
    Whole,
    /// Only items inside the reader's declared scope: `spec/ENV_SETUP.md`
    /// section 5's coder cell, "operational filtered to declared scope".
    DeclaredScope,
    /// Only while the product is being migrated: qa's "(migration only: code
    /// read)".
    MigrationOnly,
}

impl Filter {
    /// The word the module doc comment's table writes this filter as: AICD
    /// §25.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Whole => "yes",
            Self::DeclaredScope => "scope",
            Self::MigrationOnly => "migration",
        }
    }
}

/// What the table answers for one reader and one source: AICD §25.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Access {
    /// Readable, under this filter.
    Granted(Filter),
    /// Not readable.
    Denied,
}

impl Access {
    /// The word the module doc comment's table writes this answer as: AICD
    /// §25.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Granted(filter) => filter.as_str(),
            Self::Denied => "no",
        }
    }
}

/// The memory scope table for agent roles: AICD §25, `spec/ENV_SETUP.md`
/// section 5, "Memory scope".
///
/// Exhaustive on both axes with no wildcard arm: a new [`Role`] or a new
/// [`Source`] does not compile here until its row or column is decided.
#[must_use]
pub const fn grant(role: Role, source: Source) -> Access {
    use Access::Denied;
    use Access::Granted;
    use Filter::DeclaredScope;
    use Filter::MigrationOnly;
    use Filter::Whole;

    match role {
        // "Canonical, organizational, operational filtered to declared
        // scope, code map"; the code map narrowed by ORI-P1-022.
        Role::Coder => match source {
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Brief
            | Source::Runbook
            | Source::InfrastructureAdr
            | Source::Organizational => Granted(Whole),
            Source::OperationalRecord
            | Source::OperationalDefect
            | Source::Incident
            | Source::CodeMap => Granted(DeclaredScope),
            Source::MergedDiff
            | Source::AnalyticsSummary
            | Source::RawEvidence
            | Source::CodeRead => Denied,
        },
        // "All layers for the product plus organizational".
        Role::Lead => match source {
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Brief
            | Source::Runbook
            | Source::InfrastructureAdr
            | Source::Organizational
            | Source::OperationalRecord
            | Source::OperationalDefect
            | Source::Incident
            | Source::CodeMap
            | Source::MergedDiff
            | Source::AnalyticsSummary
            | Source::RawEvidence
            | Source::CodeRead => Granted(Whole),
        },
        // "Canonical, criteria, operational defects, code map (migration
        // only: code read)"; raw evidence from `spec/API_SPEC.md` section 3.
        Role::Qa => match source {
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Brief
            | Source::Runbook
            | Source::InfrastructureAdr
            | Source::OperationalDefect
            | Source::CodeMap
            | Source::RawEvidence => Granted(Whole),
            Source::CodeRead => Granted(MigrationOnly),
            Source::Organizational
            | Source::OperationalRecord
            | Source::Incident
            | Source::MergedDiff
            | Source::AnalyticsSummary => Denied,
        },
        // "Runbooks, incidents, infrastructure ADRs"; raw evidence from
        // `spec/API_SPEC.md` section 3.
        Role::Operations => match source {
            Source::Runbook
            | Source::InfrastructureAdr
            | Source::Incident
            | Source::RawEvidence => Granted(Whole),
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Brief
            | Source::Organizational
            | Source::OperationalRecord
            | Source::OperationalDefect
            | Source::CodeMap
            | Source::MergedDiff
            | Source::AnalyticsSummary
            | Source::CodeRead => Denied,
        },
        // "Canonical, organizational, merged diffs, code map".
        Role::Documentation => match source {
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Brief
            | Source::Runbook
            | Source::InfrastructureAdr
            | Source::Organizational
            | Source::MergedDiff
            | Source::CodeMap => Granted(Whole),
            Source::OperationalRecord
            | Source::OperationalDefect
            | Source::Incident
            | Source::AnalyticsSummary
            | Source::RawEvidence
            | Source::CodeRead => Denied,
        },
        // "Analytics summaries, brief".
        Role::ProductSignal => match source {
            Source::AnalyticsSummary | Source::Brief => Granted(Whole),
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Runbook
            | Source::InfrastructureAdr
            | Source::Organizational
            | Source::OperationalRecord
            | Source::OperationalDefect
            | Source::Incident
            | Source::CodeMap
            | Source::MergedDiff
            | Source::RawEvidence
            | Source::CodeRead => Denied,
        },
        // "Everything the operator can read", less raw evidence, which
        // `spec/API_SPEC.md` section 3 does not grant the assistant.
        Role::Assistant => match source {
            Source::CanonicalSection
            | Source::Adr
            | Source::Criterion
            | Source::Brief
            | Source::Runbook
            | Source::InfrastructureAdr
            | Source::Organizational
            | Source::OperationalRecord
            | Source::OperationalDefect
            | Source::Incident
            | Source::CodeMap
            | Source::MergedDiff
            | Source::AnalyticsSummary
            | Source::CodeRead => Granted(Whole),
            Source::RawEvidence => Denied,
        },
    }
}

/// The memory scope of the operator, `Actor::Human`: AICD §25,
/// `spec/ENV_SETUP.md` section 5's operator row, "Everything".
///
/// Exhaustive on [`Source`] with no wildcard arm, for the same reason as
/// [`grant`].
#[must_use]
pub const fn operator_grant(source: Source) -> Access {
    match source {
        Source::CanonicalSection
        | Source::Adr
        | Source::Criterion
        | Source::Brief
        | Source::Runbook
        | Source::InfrastructureAdr
        | Source::Organizational
        | Source::OperationalRecord
        | Source::OperationalDefect
        | Source::Incident
        | Source::CodeMap
        | Source::MergedDiff
        | Source::AnalyticsSummary
        | Source::RawEvidence
        | Source::CodeRead => Access::Granted(Filter::Whole),
    }
}

/// Where the product is in its life, as far as this table cares: AICD §24.
///
/// Engine-supplied; see the module doc comment, "Where every fact comes
/// from".
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProductStage {
    /// Any time the product is not being migrated into AICD.
    Standard,
    /// Phases M0 to M5 of a migration (AICD §24): qa's code read is granted.
    Migration,
}

// ---------------------------------------------------------------------------
// ModulePath
// ---------------------------------------------------------------------------

/// Why a module path was refused: AICD §12's declared scope, read strictly.
/// See the module doc comment, "Module paths".
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PathRejection {
    /// Nothing was given.
    Empty,
    /// The path begins or ends with whitespace.
    SurroundingWhitespace,
    /// The path holds a control character.
    ControlCharacter,
    /// The path holds a backslash.
    Backslash,
    /// The path is absolute: a leading `/`, or a `:` in its first component.
    Absolute,
    /// The path holds a `*`, a glob `ori_core::types::Scope` refuses too.
    Glob,
    /// Two separators in a row.
    EmptyComponent,
    /// A `.` component.
    CurrentDirectory,
    /// A `..` component.
    ParentDirectory,
}

impl fmt::Display for PathRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Empty => "the path is empty",
            Self::SurroundingWhitespace => "the path begins or ends with whitespace",
            Self::ControlCharacter => "the path holds a control character",
            Self::Backslash => "the path holds a backslash",
            Self::Absolute => "the path is absolute",
            Self::Glob => "the path holds a glob",
            Self::EmptyComponent => "the path holds an empty component",
            Self::CurrentDirectory => "the path holds a `.` component",
            Self::ParentDirectory => "the path holds a `..` component",
        };
        f.write_str(text)
    }
}

impl std::error::Error for PathRejection {}

/// A repository-relative module path, `/`-separated, checked strictly:
/// AICD §12 ("Every implementation plan declares the modules it will touch").
///
/// See the module doc comment, "Module paths".
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath(String);

impl ModulePath {
    /// Reads a module path, refusing what the module doc comment lists and
    /// dropping one trailing `/`: AICD §12.
    ///
    /// # Errors
    ///
    /// The [`PathRejection`] naming the first rule the path breaks.
    pub fn parse(text: &str) -> Result<Self, PathRejection> {
        if text.is_empty() {
            return Err(PathRejection::Empty);
        }
        if text.trim() != text {
            return Err(PathRejection::SurroundingWhitespace);
        }
        if text.chars().any(char::is_control) {
            return Err(PathRejection::ControlCharacter);
        }
        if text.contains('\\') {
            return Err(PathRejection::Backslash);
        }
        if text.starts_with('/') {
            return Err(PathRejection::Absolute);
        }
        if text.contains('*') {
            return Err(PathRejection::Glob);
        }
        let body = text.strip_suffix('/').unwrap_or(text);
        for (index, component) in body.split('/').enumerate() {
            match component {
                "" => return Err(PathRejection::EmptyComponent),
                "." => return Err(PathRejection::CurrentDirectory),
                ".." => return Err(PathRejection::ParentDirectory),
                _ => {}
            }
            if index == 0 && component.contains(':') {
                return Err(PathRejection::Absolute);
            }
        }
        Ok(Self(body.to_owned()))
    }

    /// The path, `/`-separated, with no trailing `/`: AICD §12.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this path is `directory` or sits under it, compared component
    /// by component and never as a string prefix: AICD §12.
    #[must_use]
    pub fn is_within(&self, directory: &Self) -> bool {
        let mut mine = self.0.split('/');
        directory
            .0
            .split('/')
            .all(|theirs| mine.next() == Some(theirs))
    }

    /// Whether either path is within the other: AICD §12, the overlap the
    /// lock table refuses on.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.is_within(other) || other.is_within(self)
    }

    /// Whether this path's leading components are exactly `prefix`.
    fn starts_with_components(&self, prefix: &[&str]) -> bool {
        let mut mine = self.0.split('/');
        prefix.iter().all(|theirs| mine.next() == Some(*theirs))
    }

    /// Whether this path's components are exactly `components`.
    fn equals_components(&self, components: &[&str]) -> bool {
        self.0.split('/').eq(components.iter().copied())
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Principal, requests, claims
// ---------------------------------------------------------------------------

/// Who is asking, as the transport that authenticated them established it:
/// AICD §25, "Maps each agent identity to a role".
///
/// Built by the engine, never by an agent: `ori-mcp` builds one for an agent
/// session and `ori-rpc` for the operator. [`MemoryRequest`] has no field
/// that could become one. The role is not here: it is read from the log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Principal {
    actor: Actor,
    assigned_ticket: Option<Id>,
}

impl Principal {
    /// The principal the transport authenticated, with no ticket assignment:
    /// AICD §25.
    #[must_use]
    pub const fn authenticated(actor: Actor) -> Self {
        Self {
            actor,
            assigned_ticket: None,
        }
    }

    /// The ticket the engine assigned this session to: AICD §7, the coder
    /// "Takes one validated ticket".
    ///
    /// The engine's own record, set by `ori-mcp` from the session it spawned
    /// for the ticket and never from a tool argument; see the module doc
    /// comment, "Where every fact comes from".
    #[must_use]
    pub fn with_assigned_ticket(self, ticket_id: Id) -> Self {
        Self {
            assigned_ticket: Some(ticket_id),
            ..self
        }
    }

    /// The authenticated actor: AICD §25.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The engine-supplied ticket assignment, if any: AICD §7.
    #[must_use]
    pub const fn assigned_ticket(&self) -> Option<&Id> {
        self.assigned_ticket.as_ref()
    }
}

/// What is being asked for: AICD §25's retrieval queries, and
/// `spec/API_SPEC.md` section 3's three memory tools.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Query {
    /// `aicd_context(ticket_id)` and `memory.context`: the context package for
    /// one ticket.
    Context {
        /// The ticket the package is for.
        ticket_id: Id,
    },
    /// `aicd_search(query)` and `memory.search`: a scoped search.
    Search {
        /// The query text, free text from the caller; never logged.
        text: String,
        /// The sources to search, by [`Source::as_str`] name; empty searches
        /// every source the reader may read.
        sources: Vec<String>,
    },
    /// `aicd_evidence(record_id)`: raw evidence for one record.
    Evidence {
        /// The record whose evidence is asked for.
        record_id: Id,
    },
    /// A read of one whole source, for the retrieval's other task-oriented
    /// queries ("criteria touching payments").
    Read {
        /// The source, by [`Source::as_str`] name.
        source: String,
    },
}

impl Query {
    /// The name a logged event records this query kind as: AICD §25.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Context { .. } => "context",
            Self::Search { .. } => "search",
            Self::Evidence { .. } => "evidence",
            Self::Read { .. } => "read",
        }
    }
}

/// What a request says about its own sender: AICD §25, "an agent cannot
/// request outside its scope".
///
/// Recorded and compared, never believed; see the module doc comment, "Where
/// every fact comes from". `memory.search` and `memory.context` carry a
/// `product_id` (`spec/API_SPEC.md` section 1), and an agent's tool call may
/// carry anything; `ori-mcp` passes what it received here rather than
/// dropping it, so an impersonation attempt is refused and recorded.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Claims {
    /// The product the request says it is for.
    pub product_id: Option<Id>,
    /// The identity the request says it comes from.
    pub identity: Option<Id>,
    /// The role the request says its sender holds, as sent.
    pub role: Option<String>,
    /// The declared scope the request says its sender holds, as sent.
    pub declared_scope: Option<Vec<String>>,
}

/// One request to read memory: AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRequest {
    /// What is asked for.
    pub query: Query,
    /// What the request says about its sender.
    pub claims: Claims,
}

impl MemoryRequest {
    /// A request with no claims: AICD §25.
    #[must_use]
    pub fn new(query: Query) -> Self {
        Self {
            query,
            claims: Claims::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Who an authorization was granted to, once resolved: AICD §25, "Maps each
/// agent identity to a role".
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reader {
    /// An agent identity, with the role this product's log records for it.
    Agent {
        /// The identity.
        identity: Id,
        /// Its role, from its `identity.created` event.
        role: Role,
    },
    /// The operator, `Actor::Human`.
    Operator {
        /// The human's identity.
        identity: Id,
    },
}

impl Reader {
    /// The role, absent for the operator: AICD §7.
    #[must_use]
    pub const fn role(&self) -> Option<Role> {
        match self {
            Self::Agent { role, .. } => Some(*role),
            Self::Operator { .. } => None,
        }
    }

    /// The actor this reader is, for the events recorded on its behalf: AICD
    /// §17, "Every agent action is written to an immutable audit trail".
    #[must_use]
    pub fn actor(&self) -> Actor {
        match self {
            Self::Agent { identity, .. } => Actor::Agent(identity.clone()),
            Self::Operator { identity } => Actor::Human(identity.clone()),
        }
    }

    /// What the table grants this reader on `source`: AICD §25.
    #[must_use]
    pub const fn access(&self, source: Source) -> Access {
        match self {
            Self::Agent { role, .. } => grant(*role, source),
            Self::Operator { .. } => operator_grant(source),
        }
    }
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why a request was refused: AICD §25.
///
/// No variant carries agent free text; see the module doc comment, "What is
/// logged".
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RefusalReason {
    /// The engine acting as [`Actor::System`] reads nothing here.
    SystemPrincipal,
    /// The product's log could not be read and verified, so no role or scope
    /// can be established.
    LogUnreadable {
        /// What failed.
        detail: String,
    },
    /// A claim in the request contradicts the principal or the log.
    ClaimContradicted {
        /// Which claim: `product_id`, `identity`, `role` or `declared_scope`.
        claim: &'static str,
    },
    /// A claim in the request names something the table does not know.
    ClaimMalformed {
        /// Which claim.
        claim: &'static str,
    },
    /// The product's log holds no `identity.created` event for the identity.
    UnregisteredIdentity,
    /// An `identity.created` event could not be read, or names another
    /// product for this identity.
    IdentityRecordMalformed {
        /// Which event, and what was wrong with it.
        detail: String,
    },
    /// The log records the identity under more than one role.
    IdentityAmbiguous,
    /// A requested source name is not one the table names.
    UnknownSource {
        /// Its position among the requested sources, from zero.
        position: usize,
    },
    /// The reader's scope is filtered to a declared scope, and neither the
    /// log nor the engine names a ticket to read one from.
    NoAssignment,
    /// The engine-supplied ticket is not among the tickets the log ties to
    /// the identity's live sessions.
    AssignmentContradicted,
    /// The log ties the identity's live sessions to more than one ticket and
    /// the engine supplied none to choose between them.
    AssignmentAmbiguous,
    /// The log ties the assigned ticket to another identity: a `lock.claimed`
    /// event of the ticket names a session the log issues, or once issued, to
    /// an identity other than the requesting one, live or not.
    AssignmentHeldElsewhere {
        /// The ticket.
        ticket_id: Id,
    },
    /// Which tickets the log ties to the identity cannot be told: a
    /// `credential.issued` or `credential.revoked` event
    /// `crates/ori-broker/src/issuance.rs` would not read back or could read
    /// differently, a live session of the identity the log also issues to
    /// another identity, while the identity holds a live session a
    /// `lock.claimed` event whose session cannot be read, or, while the log
    /// issues any session to another identity, a claim of the assigned ticket
    /// whose session cannot be read.
    AssignmentRecordMalformed {
        /// Which event.
        detail: String,
    },
    /// A `lock.*` event carries no ticket, so whose claim or release it
    /// records cannot be told. `crates/ori-store/src/projections/lock.rs`
    /// refuses the same event.
    LockRecordUnattributed {
        /// The event's `seq`.
        seq: u64,
    },
    /// The ticket is not filed in this product's log.
    TicketNotInProduct {
        /// The ticket.
        ticket_id: Id,
    },
    /// The ticket is not the one the identity is assigned: "Tickets: Read
    /// own".
    TicketNotOwn {
        /// The ticket asked for.
        ticket_id: Id,
    },
    /// AICD §17's matrix grants the role no ticket read.
    TicketReadNotGranted {
        /// The role.
        role: Role,
    },
    /// AICD §17 grants the ticket read under a limit this module cannot
    /// check.
    TicketReadLimitUnsupported {
        /// The limit, as AICD §17 writes it.
        limit: &'static str,
    },
    /// A live `lock.claimed` event for the assigned ticket does not parse.
    LockRecordMalformed {
        /// The ticket.
        ticket_id: Id,
    },
    /// The requested source is outside the reader's scope.
    SourceNotInScope {
        /// The source.
        source: Source,
    },
    /// The requested source is granted only during a migration, and the
    /// product is not being migrated.
    NotInMigration {
        /// The source.
        source: Source,
    },
    /// Raw evidence was named in a search or a read; AICD §8 serves it only
    /// on an explicit evidence request.
    EvidenceOnlyOnExplicitRequest,
    /// The reader is not granted raw evidence.
    EvidenceNotGranted,
    /// The record is not in this product's log.
    RecordNotInProduct {
        /// The record.
        record_id: Id,
    },
    /// The record is outside the reader's scope, so its evidence is too.
    RecordNotInScope {
        /// The record.
        record_id: Id,
        /// The source the record belongs to.
        source: Source,
    },
    /// More than one `memory.record_created` event in this product's log
    /// carries the record id, so which record's evidence is asked for cannot
    /// be told.
    RecordIdAmbiguous {
        /// The record id.
        record_id: Id,
    },
    /// The evidence stored with the record cannot be told apart from another
    /// submission's; see the module doc comment, "Raw evidence, and which
    /// blobs are a record's".
    EvidenceNotAttributable {
        /// The record.
        record_id: Id,
        /// What could not be told apart.
        detail: String,
    },
    /// The evidence read could not be logged, so it is not granted.
    EvidenceAccessNotLogged {
        /// What failed.
        detail: String,
    },
}

impl RefusalReason {
    /// The stable code a logged event records this reason as: AICD §25.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SystemPrincipal => "system_principal",
            Self::LogUnreadable { .. } => "log_unreadable",
            Self::ClaimContradicted { .. } => "claim_contradicted",
            Self::ClaimMalformed { .. } => "claim_malformed",
            Self::UnregisteredIdentity => "unregistered_identity",
            Self::IdentityRecordMalformed { .. } => "identity_record_malformed",
            Self::IdentityAmbiguous => "identity_ambiguous",
            Self::UnknownSource { .. } => "unknown_source",
            Self::NoAssignment => "no_assignment",
            Self::AssignmentContradicted => "assignment_contradicted",
            Self::AssignmentAmbiguous => "assignment_ambiguous",
            Self::AssignmentHeldElsewhere { .. } => "assignment_held_elsewhere",
            Self::AssignmentRecordMalformed { .. } => "assignment_record_malformed",
            Self::LockRecordUnattributed { .. } => "lock_record_unattributed",
            Self::TicketNotInProduct { .. } => "ticket_not_in_product",
            Self::TicketNotOwn { .. } => "ticket_not_own",
            Self::TicketReadNotGranted { .. } => "ticket_read_not_granted",
            Self::TicketReadLimitUnsupported { .. } => "ticket_read_limit_unsupported",
            Self::LockRecordMalformed { .. } => "lock_record_malformed",
            Self::SourceNotInScope { .. } => "source_not_in_scope",
            Self::NotInMigration { .. } => "not_in_migration",
            Self::EvidenceOnlyOnExplicitRequest => "evidence_only_on_explicit_request",
            Self::EvidenceNotGranted => "evidence_not_granted",
            Self::RecordNotInProduct { .. } => "record_not_in_product",
            Self::RecordNotInScope { .. } => "record_not_in_scope",
            Self::RecordIdAmbiguous { .. } => "record_id_ambiguous",
            Self::EvidenceNotAttributable { .. } => "evidence_not_attributable",
            Self::EvidenceAccessNotLogged { .. } => "evidence_access_not_logged",
        }
    }
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemPrincipal => {
                f.write_str("the engine acting as system reads nothing through memory scopes")
            }
            Self::LogUnreadable { detail } => write!(
                f,
                "this product's event log could not be read and verified: {detail}"
            ),
            Self::ClaimContradicted { claim } => write!(
                f,
                "the request's {claim} claim contradicts what the engine and this product's log record"
            ),
            Self::ClaimMalformed { claim } => write!(
                f,
                "the request's {claim} claim is not a value the scope table knows"
            ),
            Self::UnregisteredIdentity => f.write_str(
                "this product's log holds no identity.created event for the requesting identity",
            ),
            Self::IdentityRecordMalformed { detail } => write!(
                f,
                "an identity.created event in this product's log cannot be relied on: {detail}"
            ),
            Self::IdentityAmbiguous => f.write_str(
                "this product's log registers the requesting identity under more than one role",
            ),
            Self::UnknownSource { position } => write!(
                f,
                "requested source {position} is not a source the scope table names"
            ),
            Self::NoAssignment => f.write_str(
                "the scope is filtered to a declared scope and neither the log nor the engine names the ticket",
            ),
            Self::AssignmentContradicted => f.write_str(
                "the engine-supplied ticket is not one this product's log ties to the identity's live sessions",
            ),
            Self::AssignmentAmbiguous => f.write_str(
                "this product's log ties the identity's live sessions to more than one ticket",
            ),
            Self::AssignmentHeldElsewhere { ticket_id } => write!(
                f,
                "this product's log ties ticket {ticket_id} to another identity's session, so it is not this identity's assignment"
            ),
            Self::AssignmentRecordMalformed { detail } => write!(
                f,
                "an event recording the identity's sessions, or the claims made under them, cannot be relied on: {detail}"
            ),
            Self::LockRecordUnattributed { seq } => write!(
                f,
                "the lock event at seq {seq} carries no ticket, so whose claim or release it records cannot be told"
            ),
            Self::TicketNotInProduct { ticket_id } => {
                write!(f, "ticket {ticket_id} is not filed in this product")
            }
            Self::TicketNotOwn { ticket_id } => write!(
                f,
                "ticket {ticket_id} is not the ticket this identity is assigned (read own)"
            ),
            Self::TicketReadNotGranted { role } => {
                write!(f, "role {role} is granted no ticket read")
            }
            Self::TicketReadLimitUnsupported { limit } => write!(
                f,
                "the ticket read is granted only under the limit {limit:?}, which this check cannot evaluate"
            ),
            Self::LockRecordMalformed { ticket_id } => write!(
                f,
                "a live lock claim of ticket {ticket_id} does not parse, so its declared scope is unknown"
            ),
            Self::SourceNotInScope { source } => {
                write!(f, "source {source} is outside this reader's memory scope")
            }
            Self::NotInMigration { source } => write!(
                f,
                "source {source} is granted only while the product is being migrated"
            ),
            Self::EvidenceOnlyOnExplicitRequest => f.write_str(
                "raw evidence is served only on an explicit evidence request for one record",
            ),
            Self::EvidenceNotGranted => f.write_str("this reader is not granted raw evidence"),
            Self::RecordNotInProduct { record_id } => {
                write!(f, "record {record_id} is not in this product's log")
            }
            Self::RecordNotInScope { record_id, source } => write!(
                f,
                "record {record_id} is {source}, outside this reader's memory scope"
            ),
            Self::RecordIdAmbiguous { record_id } => write!(
                f,
                "more than one record in this product's log carries the id {record_id}, so whose evidence is asked for cannot be told"
            ),
            Self::EvidenceNotAttributable { record_id, detail } => write!(
                f,
                "the evidence stored with record {record_id} cannot be told apart from another submission's: {detail}"
            ),
            Self::EvidenceAccessNotLogged { detail } => write!(
                f,
                "the evidence read could not be recorded, so it is not granted: {detail}"
            ),
        }
    }
}

/// One refusal, as it was decided and recorded: AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    actor: Actor,
    product_id: String,
    reason: RefusalReason,
    request: MemoryRequest,
    event_seq: Option<u64>,
}

impl Refusal {
    /// Who asked: AICD §25.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The product the request was made against: AICD §25.
    #[must_use]
    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    /// Why it was refused: AICD §25.
    #[must_use]
    pub const fn reason(&self) -> &RefusalReason {
        &self.reason
    }

    /// What was asked: AICD §25.
    #[must_use]
    pub const fn request(&self) -> &MemoryRequest {
        &self.request
    }

    /// The `seq` of the event recording this refusal, absent when recording
    /// it failed: AICD §25, "the refusal is logged".
    #[must_use]
    pub const fn event_seq(&self) -> Option<u64> {
        self.event_seq
    }

    /// The section this refusal is made under: AICD §25.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        memory_service_ref()
    }
}

/// Why an event this module had to append was not appended: AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LogFailure {
    /// The log refused or failed the append.
    EventLog(EventLogError),
    /// The payload could not be encoded.
    Payload(String),
    /// The product database's own identifier is not an [`Id`].
    ProductId(String),
}

impl fmt::Display for LogFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLog(inner) => write!(f, "{inner}"),
            Self::Payload(message) => {
                write!(f, "the event payload could not be encoded: {message}")
            }
            Self::ProductId(text) => write!(f, "the product identifier {text:?} is not an Id"),
        }
    }
}

/// A refused request: AICD §25. Every value is a refusal and carries AICD
/// §25 through [`ScopeError::methodology_ref`].
///
/// Boxed so that `Result<Authorization, ScopeError>` stays small.
#[derive(Debug)]
pub enum ScopeError {
    /// Refused, and the refusal was recorded.
    Refused(Box<Refusal>),
    /// Refused, and recording the refusal failed. Still refused.
    RefusedUnlogged {
        /// The refusal.
        refusal: Box<Refusal>,
        /// Why recording it failed.
        failure: Box<LogFailure>,
    },
}

impl ScopeError {
    /// The refusal: AICD §25.
    #[must_use]
    pub fn refusal(&self) -> &Refusal {
        match self {
            Self::Refused(refusal) | Self::RefusedUnlogged { refusal, .. } => refusal,
        }
    }

    /// Why it was refused: AICD §25.
    #[must_use]
    pub fn reason(&self) -> &RefusalReason {
        self.refusal().reason()
    }

    /// Whether the refusal was recorded: AICD §25, "the refusal is logged".
    #[must_use]
    pub const fn is_logged(&self) -> bool {
        matches!(self, Self::Refused(_))
    }

    /// Why recording the refusal failed, when it did: AICD §25.
    #[must_use]
    pub fn log_failure(&self) -> Option<&LogFailure> {
        match self {
            Self::Refused(_) => None,
            Self::RefusedUnlogged { failure, .. } => Some(failure.as_ref()),
        }
    }

    /// The section every refusal of this module is made under: AICD §25.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        memory_service_ref()
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => write!(
                f,
                "refused under {}: {}",
                refusal.methodology_ref(),
                refusal.reason()
            ),
            Self::RefusedUnlogged { refusal, failure } => write!(
                f,
                "refused under {}: {}; recording the refusal failed: {failure}",
                refusal.methodology_ref(),
                refusal.reason()
            ),
        }
    }
}

impl std::error::Error for ScopeError {}

// ---------------------------------------------------------------------------
// Authorization and the result filter
// ---------------------------------------------------------------------------

/// A coder's declared scope, read from the log: AICD §12, "Every
/// implementation plan declares the modules it will touch".
///
/// Held only inside an [`Authorization`], which reads it out through
/// [`Authorization::declared_scope`] and [`Authorization::declared_modules`];
/// nothing hands one out. Public so that the field of [`Authorization`]
/// holding it has a nameable type, and so that each example pinning that
/// field fails for the field's privacy alone. Like [`Authorization`], none can
/// be built or altered outside this crate, for the reasons, and under the
/// same test, given there:
///
/// ```compile_fail
/// fn widen(a: ori_memory::scope::DeclaredScope) -> ori_memory::scope::DeclaredScope {
///     ori_memory::scope::DeclaredScope { ..a }
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::DeclaredScope) {
///     let _ = &mut a.ticket_id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::DeclaredScope) {
///     let _ = &mut a.scope;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::DeclaredScope) {
///     let _ = &mut a.modules;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::DeclaredScope) {
///     let _ = &mut a.in_scope_tickets;
/// }
/// ```
///
/// The path resolves, though nothing outside this crate reads through it:
///
/// ```
/// fn reach(a: &mut ori_memory::scope::DeclaredScope) {
///     let _ = a;
/// }
/// ```
#[derive(Debug)]
pub struct DeclaredScope {
    /// The ticket the scope is declared for: the coder's assignment.
    ticket_id: Id,
    scope: Scope,
    modules: Vec<ModulePath>,
    /// The tickets whose operational records are "for M": the coder's own,
    /// and every other ticket that claimed at least one module and whose
    /// every claim, released or not, lies within `modules`. See the module
    /// doc comment, "Which records are inside a declared scope".
    in_scope_tickets: BTreeSet<Id>,
}

impl DeclaredScope {
    fn covers_path(&self, path: &ModulePath) -> bool {
        self.modules.iter().any(|module| path.is_within(module))
    }

    fn covers_ticket(&self, ticket_id: Option<&Id>) -> bool {
        ticket_id.is_some_and(|ticket| self.in_scope_tickets.contains(ticket))
    }

    fn module_names(&self) -> BTreeSet<&str> {
        self.modules.iter().map(ModulePath::as_str).collect()
    }
}

/// Proof that one request was authorized, and everything the retrieval needs
/// to serve it: AICD §25, "Enforced at query time".
///
/// No public constructor and no public field: the only way to hold one is for
/// [`ScopeEnforcer::authorize`] to have returned it. See the module doc
/// comment, "The seam the retrieval stands on".
///
/// Outside this crate none can be built, and a real one can be neither read
/// around its accessors nor altered. Each example below fails to compile for
/// one reason alone, a private field: a functional update names no field, so
/// it compiles exactly when every field is visible, and a field borrowed
/// mutably through a real value compiles exactly when that field is. Each
/// field exists, as
/// `tests::ori_t_0038_sealed_values_keep_every_field_private_and_pinned_by_a_single_reason_example`
/// checks against the definition; that test also refuses any visibility on a
/// field, `pub(crate)` included, which would let a sibling module of this
/// crate build or alter one and which no example compiled outside the crate
/// can see.
///
/// ```compile_fail
/// fn widen(a: ori_memory::scope::Authorization) -> ori_memory::scope::Authorization {
///     ori_memory::scope::Authorization { ..a }
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.reader;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.product_id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.query;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.ticket_id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.declared;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.sources;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.evidence;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Authorization) {
///     let _ = &mut a.evidence_access_seq;
/// }
/// ```
///
/// The paths resolve, and a real value is reached through its accessors:
///
/// ```
/// fn reach(a: &mut ori_memory::scope::Authorization) {
///     let _ = (a.reader(), a.product_id(), a.query(), a.ticket_id());
///     let _ = (a.declared_scope(), a.declared_modules(), a.sources());
///     let _ = (a.evidence(), a.evidence_access_seq());
/// }
/// ```
#[derive(Debug)]
pub struct Authorization {
    reader: Reader,
    product_id: Id,
    query: Query,
    ticket_id: Option<Id>,
    declared: Option<DeclaredScope>,
    sources: BTreeMap<Source, Filter>,
    evidence: Vec<EvidenceBlob>,
    evidence_access_seq: Option<u64>,
}

impl Authorization {
    /// Who this was granted to: AICD §25.
    #[must_use]
    pub const fn reader(&self) -> &Reader {
        &self.reader
    }

    /// The reader's role, absent for the operator: AICD §7.
    #[must_use]
    pub const fn role(&self) -> Option<Role> {
        self.reader.role()
    }

    /// The product this was granted in, and the only one its results may come
    /// from: AICD §25, "An agent identity belongs to one product".
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The query this was granted for, and the only one the retrieval may run
    /// under it: AICD §25.
    #[must_use]
    pub const fn query(&self) -> &Query {
        &self.query
    }

    /// The ticket this concerns: the context request's ticket, the coder's
    /// assigned ticket, or the evidence record's ticket: AICD §25.
    #[must_use]
    pub const fn ticket_id(&self) -> Option<&Id> {
        self.ticket_id.as_ref()
    }

    /// The coder's declared scope, read from the log, when the reader's
    /// scope is filtered to one: AICD §12.
    #[must_use]
    pub fn declared_scope(&self) -> Option<&Scope> {
        self.declared.as_ref().map(|declared| &declared.scope)
    }

    /// The declared scope's modules, as checked paths: AICD §12.
    #[must_use]
    pub fn declared_modules(&self) -> &[ModulePath] {
        self.declared
            .as_ref()
            .map_or(&[], |declared| declared.modules.as_slice())
    }

    /// Every source the retrieval may query under this authorization, with
    /// the filter [`Authorization::filter`] applies to it: AICD §25. Never
    /// holds [`Source::RawEvidence`].
    #[must_use]
    pub const fn sources(&self) -> &BTreeMap<Source, Filter> {
        &self.sources
    }

    /// Whether the retrieval may query `source`: AICD §25.
    #[must_use]
    pub fn allows(&self, source: Source) -> bool {
        self.sources.contains_key(&source)
    }

    /// The raw evidence an explicit evidence request was granted, empty for
    /// any other query: AICD §8. Every blob answers
    /// [`EvidenceBlob::untrusted`] with `true`; the retrieval presents it
    /// quoted and labeled untrusted.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceBlob] {
        &self.evidence
    }

    /// The `seq` of the event recording the evidence read, for an evidence
    /// request: AICD §8.
    #[must_use]
    pub const fn evidence_access_seq(&self) -> Option<u64> {
        self.evidence_access_seq
    }

    /// Whether one gathered result may be returned under this authorization:
    /// AICD §25, the same rule the request was checked against, applied to
    /// the result.
    ///
    /// Admitted only when it is from this authorization's product, classifies
    /// to a source in [`Authorization::sources`], and, under
    /// [`Filter::DeclaredScope`], is provably inside the declared scope. An
    /// [`EvidenceBlob`] is never admitted: raw evidence leaves this module
    /// only through [`Authorization::evidence`].
    #[must_use]
    pub fn admits(&self, candidate: &Candidate) -> bool {
        let Some(product_id) = candidate.product_id() else {
            return false;
        };
        if product_id != &self.product_id {
            return false;
        }
        let Some(source) = candidate.classify() else {
            return false;
        };
        let Some(filter) = self.sources.get(&source) else {
            return false;
        };
        match filter {
            Filter::Whole | Filter::MigrationOnly => true,
            Filter::DeclaredScope => self
                .declared
                .as_ref()
                .is_some_and(|declared| candidate.within(declared)),
        }
    }

    /// Keeps only what [`Authorization::admits`]: AICD §25. The result filter
    /// the retrieval applies to everything it gathered, returned as an
    /// [`Admitted`], which nothing else can build.
    #[must_use]
    pub fn filter(&self, candidates: Vec<Candidate>) -> Admitted {
        Admitted {
            product_id: self.product_id.clone(),
            reader: self.reader.clone(),
            candidates: candidates
                .into_iter()
                .filter(|candidate| self.admits(candidate))
                .collect(),
        }
    }
}

/// The results [`Authorization::filter`] admitted, and only those: AICD §25,
/// the result side of "Enforced at query time".
///
/// No public constructor and no public field, like [`Authorization`]: the
/// only way to hold one is for [`Authorization::filter`] to have returned it,
/// so a package type that holds [`Admitted`] values cannot be handed a result
/// that was never filtered. It records the product and the reader it was
/// admitted for, so the package can check both against its own
/// [`Authorization`]. See the module doc comment, "The seam the retrieval
/// stands on".
///
/// Outside this crate none can be built or altered, for the reasons, and
/// under the same test, given for [`Authorization`]:
///
/// ```compile_fail
/// fn widen(a: ori_memory::scope::Admitted) -> ori_memory::scope::Admitted {
///     ori_memory::scope::Admitted { ..a }
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Admitted) {
///     let _ = &mut a.product_id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Admitted) {
///     let _ = &mut a.reader;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::scope::Admitted) {
///     let _ = &mut a.candidates;
/// }
/// ```
///
/// The paths resolve, and a real value is reached through its accessors:
///
/// ```
/// fn reach(a: &mut ori_memory::scope::Admitted) {
///     let _ = (a.product_id(), a.reader(), a.candidates());
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Admitted {
    product_id: Id,
    reader: Reader,
    candidates: Vec<Candidate>,
}

impl Admitted {
    /// The product the results were admitted in: AICD §25.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// The reader they were admitted for: AICD §25.
    #[must_use]
    pub const fn reader(&self) -> &Reader {
        &self.reader
    }

    /// The admitted results, in the order they were gathered: AICD §25.
    #[must_use]
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }
}

impl core::ops::Deref for Admitted {
    type Target = [Candidate];

    fn deref(&self) -> &[Candidate] {
        &self.candidates
    }
}

impl PartialEq<Vec<Candidate>> for Admitted {
    fn eq(&self, other: &Vec<Candidate>) -> bool {
        &self.candidates == other
    }
}

/// One result the retrieval gathered, before it is filtered: AICD §25.
///
/// A [`Candidate::Record`] and a [`Candidate::Evidence`] hold values only
/// `barrier.rs` builds, from the log. Every field of the other three
/// variants is set by whoever built the candidate, the [`SearchHit`] and the
/// `code_map::Module` inside it included, since both are plain structs with
/// public fields: the filter classifies and scopes those variants by fields
/// it cannot check. The module doc comment's table, under "The seam the
/// retrieval stands on", lists which field decides what.
///
/// Outside this crate neither a [`MemoryRecord`] nor an [`EvidenceBlob`] can
/// be built or altered, for the reasons, and under the same test, given for
/// [`Authorization`]; the test reads their definitions in `barrier.rs`.
///
/// ```compile_fail
/// fn widen(a: ori_memory::barrier::MemoryRecord) -> ori_memory::barrier::MemoryRecord {
///     ori_memory::barrier::MemoryRecord { ..a }
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.product_id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.layer;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.kind;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.structured;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.provenance;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = &mut a.untrusted;
/// }
/// ```
///
/// The paths resolve, and a real value is reached through its accessors:
///
/// ```
/// fn reach(a: &mut ori_memory::barrier::MemoryRecord) {
///     let _ = (a.id(), a.product_id(), a.layer(), a.kind());
///     let _ = (a.structured(), a.provenance(), a.untrusted());
/// }
/// ```
///
/// ```compile_fail
/// fn widen(a: ori_memory::barrier::EvidenceBlob) -> ori_memory::barrier::EvidenceBlob {
///     ori_memory::barrier::EvidenceBlob { ..a }
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::EvidenceBlob) {
///     let _ = &mut a.id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::EvidenceBlob) {
///     let _ = &mut a.record_id;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::EvidenceBlob) {
///     let _ = &mut a.content_ref;
/// }
/// ```
///
/// ```compile_fail
/// fn touch(a: &mut ori_memory::barrier::EvidenceBlob) {
///     let _ = &mut a.content_type;
/// }
/// ```
///
/// The paths resolve, and a real value is reached through its accessors:
///
/// ```
/// fn reach(a: &mut ori_memory::barrier::EvidenceBlob) {
///     let _ = (a.id(), a.record_id(), a.content_ref(), a.content_type());
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum Candidate {
    /// An operational memory record.
    Record(MemoryRecord),
    /// An evidence blob. Never admitted.
    Evidence(EvidenceBlob),
    /// A hit from a product's repository index.
    Document {
        /// The product whose index it came from.
        product_id: Id,
        /// The hit.
        hit: SearchHit,
    },
    /// A module of a product's code map.
    Module {
        /// The product whose code map it came from.
        product_id: Id,
        /// The module.
        module: code_map::Module,
    },
    /// An item of a source no module of this crate produces as a typed value
    /// yet. Refused for any source that has one.
    Unproduced {
        /// The product it came from.
        product_id: Id,
        /// Its source.
        source: Source,
        /// Where it came from, for provenance.
        locator: String,
    },
}

impl Candidate {
    /// The product this item came from, absent for an evidence blob, which
    /// names none: AICD §25, "partitioned by product identifier".
    #[must_use]
    pub const fn product_id(&self) -> Option<&Id> {
        match self {
            Self::Record(record) => Some(record.product_id()),
            Self::Evidence(_) => None,
            Self::Document { product_id, .. }
            | Self::Module { product_id, .. }
            | Self::Unproduced { product_id, .. } => Some(product_id),
        }
    }

    /// The source this item belongs to, `None` when it cannot be classified:
    /// AICD §25, "Every item returned carries its source".
    #[must_use]
    pub fn classify(&self) -> Option<Source> {
        match self {
            Self::Record(record) => Some(record_source(record.kind())),
            Self::Evidence(_) => Some(Source::RawEvidence),
            Self::Document { hit, .. } => document_source(hit),
            Self::Module { .. } => Some(Source::CodeMap),
            Self::Unproduced { source, .. } => {
                if source.has_typed_producer() {
                    None
                } else {
                    Some(*source)
                }
            }
        }
    }

    /// Whether this item is provably inside `declared`.
    fn within(&self, declared: &DeclaredScope) -> bool {
        match self {
            Self::Record(record) => declared.covers_ticket(record.provenance().ticket_id()),
            Self::Document { hit, .. } => match hit.kind {
                DocumentKind::Module => {
                    ModulePath::parse(&hit.path).is_ok_and(|path| declared.covers_path(&path))
                }
                DocumentKind::Section | DocumentKind::Adr | DocumentKind::Criterion => false,
            },
            Self::Module { module, .. } => {
                ModulePath::parse(&module.path).is_ok_and(|path| declared.covers_path(&path))
            }
            Self::Evidence(_) | Self::Unproduced { .. } => false,
        }
    }
}

/// The source a record of `kind` belongs to. Exhaustive: a new
/// [`RecordKind`] does not compile here until it is placed.
const fn record_source(kind: RecordKind) -> Source {
    match kind {
        RecordKind::ClosingReport | RecordKind::BlockedReport | RecordKind::EscalationDecision => {
            Source::OperationalRecord
        }
        RecordKind::Finding => Source::OperationalDefect,
        RecordKind::Incident | RecordKind::PostMortem => Source::Incident,
    }
}

/// The source an index hit belongs to, from its kind and its path, `None`
/// for a canonical kind outside `spec/` or a path that does not parse.
fn document_source(hit: &SearchHit) -> Option<Source> {
    match hit.kind {
        DocumentKind::Module => Some(Source::CodeMap),
        DocumentKind::Section | DocumentKind::Adr | DocumentKind::Criterion => {
            let file = ModulePath::parse(document_file(&hit.path)).ok()?;
            if !file.starts_with_components(&CANONICAL_ROOT) {
                return None;
            }
            match hit.kind {
                DocumentKind::Adr => Some(Source::Adr),
                DocumentKind::Criterion => Some(Source::Criterion),
                DocumentKind::Section if file.equals_components(&BRIEF_FILE) => Some(Source::Brief),
                DocumentKind::Section if file.starts_with_components(&RUNBOOK_DIR) => {
                    Some(Source::Runbook)
                }
                DocumentKind::Section => Some(Source::CanonicalSection),
                DocumentKind::Module => None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ScopeEnforcer
// ---------------------------------------------------------------------------

/// The scope enforcer: AICD §25, "Enforced at query time; an agent cannot
/// request outside its scope, and the refusal is logged".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeEnforcer {
    stage: ProductStage,
}

/// What [`ScopeEnforcer`] decided before anything was appended.
enum Decided {
    Granted(Authorization),
    Evidence(Authorization),
}

impl ScopeEnforcer {
    /// An enforcer for a product at `stage`, which the engine supplies: AICD
    /// §25.
    #[must_use]
    pub const fn new(stage: ProductStage) -> Self {
        Self { stage }
    }

    /// The stage this enforcer was built for: AICD §24.
    #[must_use]
    pub const fn stage(&self) -> ProductStage {
        self.stage
    }

    /// Decides one request against this product's log: AICD §25.
    ///
    /// `db` is the product the request is made against, and the only product
    /// it can be granted in. The role, the product, the coder's declared
    /// scope and the ticket's existence are read from `db`'s log; nothing is
    /// read from `request` but what it asks for, and its claims are only
    /// compared. A refusal appends one [`REFUSAL_EVENT_KIND`] event; a
    /// granted evidence request appends one [`EVIDENCE_ACCESS_EVENT_KIND`]
    /// event before it is returned.
    ///
    /// `at` is the time the request is decided at: it stamps either event,
    /// and it decides which credentials have expired, so which sessions the
    /// log still ties to the identity. The caller owns it as it owns the
    /// principal: `ori-mcp` reads it from the engine's clock, never from a
    /// tool argument. See the module doc comment, "Where every fact comes
    /// from".
    ///
    /// # Errors
    ///
    /// [`ScopeError::Refused`] for every refusal, recorded;
    /// [`ScopeError::RefusedUnlogged`] when recording it failed, which is
    /// still a refusal.
    pub fn authorize(
        &self,
        db: &mut ProductDb,
        at: Timestamp,
        principal: &Principal,
        request: &MemoryRequest,
    ) -> Result<Authorization, ScopeError> {
        match self.decide(db, at, principal, request) {
            Ok(Decided::Granted(authorization)) => Ok(authorization),
            Ok(Decided::Evidence(mut authorization)) => {
                match append_evidence_access(db, at, &authorization) {
                    Ok(seq) => {
                        authorization.evidence_access_seq = Some(seq);
                        Ok(authorization)
                    }
                    Err(failure) => {
                        let reason = RefusalReason::EvidenceAccessNotLogged {
                            detail: failure.to_string(),
                        };
                        Err(refuse(db, at, principal, request, reason))
                    }
                }
            }
            Err(reason) => Err(refuse(db, at, principal, request, reason)),
        }
    }

    /// Everything [`ScopeEnforcer::authorize`] decides, as of `at`, with
    /// nothing appended.
    fn decide(
        &self,
        db: &mut ProductDb,
        at: Timestamp,
        principal: &Principal,
        request: &MemoryRequest,
    ) -> Result<Decided, RefusalReason> {
        let principal_id = match principal.actor() {
            Actor::System => return Err(RefusalReason::SystemPrincipal),
            Actor::Human(id) | Actor::Agent(id) => id.clone(),
        };
        let product_id = Id::parse(db.product_id()).map_err(|_| RefusalReason::LogUnreadable {
            detail: "the product database's identifier is not an Id".to_owned(),
        })?;
        if request
            .claims
            .product_id
            .as_ref()
            .is_some_and(|claimed| claimed != &product_id)
        {
            return Err(RefusalReason::ClaimContradicted {
                claim: "product_id",
            });
        }
        if request
            .claims
            .identity
            .as_ref()
            .is_some_and(|claimed| claimed != &principal_id)
        {
            return Err(RefusalReason::ClaimContradicted { claim: "identity" });
        }

        let events = read_log(db).map_err(|err| RefusalReason::LogUnreadable {
            detail: err.to_string(),
        })?;

        let reader = match principal.actor() {
            Actor::Human(identity) => Reader::Operator {
                identity: identity.clone(),
            },
            Actor::Agent(identity) => Reader::Agent {
                identity: identity.clone(),
                role: registered_role(&events, &product_id, identity)?,
            },
            Actor::System => return Err(RefusalReason::SystemPrincipal),
        };
        check_claimed_role(&request.claims, &reader)?;

        let needs_declared_scope = Source::ALL
            .iter()
            .any(|source| reader.access(*source) == Access::Granted(Filter::DeclaredScope));
        let declared = if needs_declared_scope {
            let sessions = read_sessions(&events, &principal_id, at)?;
            let logged = logged_assignments(&events, &sessions)?;
            let ticket_id = assignment(principal.assigned_ticket(), &logged)?;
            check_assignment_unshared(&events, &ticket_id, &sessions)?;
            Some(declared_scope(&events, &ticket_id)?)
        } else {
            None
        };
        check_claimed_scope(&request.claims, declared.as_ref())?;

        let mut authorization = Authorization {
            reader,
            product_id,
            query: request.query.clone(),
            ticket_id: declared.as_ref().map(|declared| declared.ticket_id.clone()),
            declared,
            sources: BTreeMap::new(),
            evidence: Vec::new(),
            evidence_access_seq: None,
        };

        match &request.query {
            Query::Context { ticket_id } => {
                if !ticket_filed(&events, ticket_id) {
                    return Err(RefusalReason::TicketNotInProduct {
                        ticket_id: ticket_id.clone(),
                    });
                }
                let assigned = authorization
                    .declared
                    .as_ref()
                    .map(|declared| &declared.ticket_id);
                check_ticket_read(&authorization.reader, ticket_id, assigned)?;
                authorization.ticket_id = Some(ticket_id.clone());
                authorization.sources = self.readable_sources(&authorization.reader);
                Ok(Decided::Granted(authorization))
            }
            Query::Search { sources, .. } => {
                let named = parse_sources(sources)?;
                authorization.sources = self.requested_sources(&authorization.reader, &named)?;
                Ok(Decided::Granted(authorization))
            }
            Query::Read { source } => {
                let named = parse_sources(std::slice::from_ref(source))?;
                authorization.sources = self.requested_sources(&authorization.reader, &named)?;
                Ok(Decided::Granted(authorization))
            }
            Query::Evidence { record_id } => {
                self.grant_evidence(db, &events, &mut authorization, record_id)?;
                Ok(Decided::Evidence(authorization))
            }
        }
    }

    /// Every source `reader` may query without naming one, less raw
    /// evidence, which is served only on an explicit request.
    fn readable_sources(&self, reader: &Reader) -> BTreeMap<Source, Filter> {
        let mut out = BTreeMap::new();
        for source in Source::ALL {
            if source == Source::RawEvidence {
                continue;
            }
            match reader.access(source) {
                Access::Granted(Filter::MigrationOnly) => {
                    if self.stage == ProductStage::Migration {
                        out.insert(source, Filter::MigrationOnly);
                    }
                }
                Access::Granted(filter @ (Filter::Whole | Filter::DeclaredScope)) => {
                    out.insert(source, filter);
                }
                Access::Denied => {}
            }
        }
        out
    }

    /// The sources a search or read names, each checked; every readable
    /// source when none is named.
    fn requested_sources(
        &self,
        reader: &Reader,
        named: &[Source],
    ) -> Result<BTreeMap<Source, Filter>, RefusalReason> {
        if named.is_empty() {
            return Ok(self.readable_sources(reader));
        }
        let mut out = BTreeMap::new();
        for source in named {
            let source = *source;
            if source == Source::RawEvidence {
                return Err(RefusalReason::EvidenceOnlyOnExplicitRequest);
            }
            match reader.access(source) {
                Access::Denied => return Err(RefusalReason::SourceNotInScope { source }),
                Access::Granted(Filter::MigrationOnly) => {
                    if self.stage != ProductStage::Migration {
                        return Err(RefusalReason::NotInMigration { source });
                    }
                    out.insert(source, Filter::MigrationOnly);
                }
                Access::Granted(filter @ (Filter::Whole | Filter::DeclaredScope)) => {
                    out.insert(source, filter);
                }
            }
        }
        Ok(out)
    }

    /// Checks an evidence request and fills `authorization` with the blobs it
    /// grants: the reader must be granted raw evidence, exactly one record in
    /// this product's log must carry the id, the reader must be able to read
    /// that record, and only the blobs stored with it are handed over. See
    /// the module doc comment, "Raw evidence, and which blobs are a record's".
    fn grant_evidence(
        &self,
        db: &mut ProductDb,
        events: &[Event],
        authorization: &mut Authorization,
        record_id: &Id,
    ) -> Result<(), RefusalReason> {
        match authorization.reader.access(Source::RawEvidence) {
            Access::Granted(Filter::Whole) => {}
            Access::Granted(Filter::DeclaredScope | Filter::MigrationOnly) | Access::Denied => {
                return Err(RefusalReason::EvidenceNotGranted);
            }
        }
        let not_in_product = || RefusalReason::RecordNotInProduct {
            record_id: record_id.clone(),
        };
        let ambiguous = || RefusalReason::RecordIdAmbiguous {
            record_id: record_id.clone(),
        };
        // Every record event carrying the id, whichever product its payload
        // names: two of them is a refusal, never the first one found.
        let record_event = match record_events(events, record_id)?.as_slice() {
            [] => return Err(not_in_product()),
            [one] => *one,
            _ => return Err(ambiguous()),
        };
        // The record itself, read through the barrier; a second read, so it
        // is held to the same count.
        let mut records: Vec<MemoryRecord> = barrier::read_records(db)
            .map_err(barrier_unreadable)?
            .into_iter()
            .filter(|record| record.id() == record_id)
            .collect();
        let record = match (records.pop(), records.is_empty()) {
            (None, _) => return Err(not_in_product()),
            (Some(record), true) => record,
            (Some(_), false) => return Err(ambiguous()),
        };
        if record.product_id() != &authorization.product_id {
            return Err(not_in_product());
        }
        let source = record_source(record.kind());
        let readable = match authorization.reader.access(source) {
            Access::Granted(Filter::Whole) => true,
            Access::Granted(Filter::DeclaredScope) => authorization
                .declared
                .as_ref()
                .is_some_and(|declared| declared.covers_ticket(record.provenance().ticket_id())),
            Access::Granted(Filter::MigrationOnly) => self.stage == ProductStage::Migration,
            Access::Denied => false,
        };
        if !readable {
            return Err(RefusalReason::RecordNotInScope {
                record_id: record_id.clone(),
                source,
            });
        }
        authorization.evidence = attributed_evidence(db, events, record_event, &record)?;
        authorization.ticket_id = record.provenance().ticket_id().cloned();
        authorization.sources = BTreeMap::new();
        Ok(())
    }
}

/// `memory.record_created`'s payload, the field this module reads, as
/// `barrier.rs` writes it.
#[derive(Deserialize)]
struct RecordCreatedWire {
    id: String,
}

/// `memory.evidence_stored`'s payload, the fields this module reads, as
/// `barrier.rs` writes them.
#[derive(Deserialize)]
struct EvidenceStoredWire {
    id: String,
    record_id: String,
}

/// Every `memory.record_created` event in `events` whose record id is
/// `record_id`. A record event that does not parse is a refusal: whose it is
/// cannot be told.
fn record_events<'e>(events: &'e [Event], record_id: &Id) -> Result<Vec<&'e Event>, RefusalReason> {
    let mut found = Vec::new();
    for event in events.iter().filter(|event| event.kind() == RECORD_CREATED) {
        let malformed = || RefusalReason::LogUnreadable {
            detail: format!("the record event at seq {} does not parse", event.seq()),
        };
        let wire: RecordCreatedWire =
            serde_json::from_str(event.payload()).map_err(|_| malformed())?;
        if &Id::parse(&wire.id).map_err(|_| malformed())? == record_id {
            found.push(event);
        }
    }
    Ok(found)
}

/// One `memory.evidence_stored` event, read: its blob's id and the record it
/// names.
struct StoredEvidence {
    blob_id: Id,
    record_id: Id,
}

/// The blobs `record` was submitted with, and no others: the run of
/// `memory.evidence_stored` events immediately before `record_event` that
/// name the record and share its actor and time. Refused when that run
/// cannot be the record's alone. See the module doc comment, "Raw evidence,
/// and which blobs are a record's".
fn attributed_evidence(
    db: &mut ProductDb,
    events: &[Event],
    record_event: &Event,
    record: &MemoryRecord,
) -> Result<Vec<EvidenceBlob>, RefusalReason> {
    let record_id = record.id();
    let refuse = |detail: String| RefusalReason::EvidenceNotAttributable {
        record_id: record_id.clone(),
        detail,
    };

    // Every evidence event in the log, by seq: one that does not parse could
    // be anyone's, this record's included.
    let mut stored: BTreeMap<u64, StoredEvidence> = BTreeMap::new();
    for event in events
        .iter()
        .filter(|event| event.kind() == EVIDENCE_STORED)
    {
        let malformed = || {
            refuse(format!(
                "the evidence event at seq {} does not parse",
                event.seq()
            ))
        };
        let wire: EvidenceStoredWire =
            serde_json::from_str(event.payload()).map_err(|_| malformed())?;
        stored.insert(
            event.seq(),
            StoredEvidence {
                blob_id: Id::parse(&wire.id).map_err(|_| malformed())?,
                record_id: Id::parse(&wire.record_id).map_err(|_| malformed())?,
            },
        );
    }

    // The run: walk back from the record's own event while each event is
    // evidence for this record, by the same actor, at the same time.
    let mut run: Vec<&StoredEvidence> = Vec::new();
    for event in events
        .iter()
        .rev()
        .skip_while(|event| event.seq() != record_event.seq())
        .skip(1)
    {
        let Some(entry) = stored.get(&event.seq()) else {
            break;
        };
        if &entry.record_id != record_id
            || event.actor() != record_event.actor()
            || event.at() != record_event.at()
        {
            break;
        }
        run.push(entry);
    }
    run.reverse();

    if run.len() != record.structured().len() {
        return Err(refuse(format!(
            "{} evidence events precede the record, which has {} fields",
            run.len(),
            record.structured().len()
        )));
    }

    // The blobs themselves, read through the barrier. The evidence file is
    // named by the evidence id, so each id in the run must be named by no
    // other evidence event in the log, the run's own included, or the file
    // may hold another submission's raw text: every blob carrying one of the
    // run's ids is read back, and there must be exactly one per run entry,
    // each for this record. Being a second read, it also holds the log to
    // what the first read showed.
    let wanted: BTreeSet<&Id> = run.iter().map(|entry| &entry.blob_id).collect();
    let blobs: Vec<EvidenceBlob> = barrier::read_evidence(db)
        .map_err(barrier_unreadable)?
        .into_iter()
        .filter(|blob| wanted.contains(blob.id()))
        .collect();
    if wanted.len() != run.len()
        || blobs.len() != run.len()
        || blobs.iter().any(|blob| blob.record_id() != record_id)
    {
        return Err(refuse(
            "an evidence id of the record is named by another evidence event, so its file may hold another submission's raw text".to_owned(),
        ));
    }
    Ok(blobs)
}

/// A barrier read failure, as the refusal it becomes.
fn barrier_unreadable(err: BarrierError) -> RefusalReason {
    RefusalReason::LogUnreadable {
        detail: err.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Reading the log
// ---------------------------------------------------------------------------

/// Every event in `db`'s log, through [`EventLog::verify`] first, the same
/// reader-completeness guard `barrier.rs` and `ori-broker`'s registration
/// rely on: a reader that silently returned fewer rows would read as "not
/// registered", which is a refusal, but a verified read is what makes a
/// grant trustworthy.
fn read_log(db: &mut ProductDb) -> Result<Vec<Event>, EventLogError> {
    let report = EventLog::verify(db.connection())?;
    match report.tip_seq {
        Some(tip) => EventLog::read_range(db.connection(), 1, tip),
        None => Ok(Vec::new()),
    }
}

/// The runtimes `crates/ori-broker/src/identity.rs`'s `IdentityRuntime`
/// reads back, by the names its `FromStr` accepts, and no others. Written
/// here because this crate does not depend on `ori-broker`;
/// `tests::ori_t_0038_identity_payload_layout_and_rules_match_ori_broker_registration`
/// checks them against that file.
const IDENTITY_RUNTIMES: [&str; 2] = ["acp", "headless"];

/// `identity.created`'s payload: the six fields
/// `crates/ori-broker/src/registration.rs` writes, each required and read as
/// text, no other field, and none twice (serde's derive refuses a duplicate
/// field).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityCreatedWire {
    id: String,
    product_id: String,
    role: String,
    model: String,
    family: String,
    runtime: String,
}

impl IdentityCreatedWire {
    /// The exact text registration.rs's writer produces for these values:
    /// the six fields in its order with no whitespace, `id`, `product_id`,
    /// `model` and `family` escaped as its `json_escape` escapes them, and
    /// `role` and `runtime` written as they are.
    fn as_registration_writes_it(&self) -> String {
        format!(
            "{{\"id\":\"{}\",\"product_id\":\"{}\",\"role\":\"{}\",\"model\":\"{}\",\"family\":\"{}\",\"runtime\":\"{}\"}}",
            broker_escape(&self.id),
            broker_escape(&self.product_id),
            self.role,
            broker_escape(&self.model),
            broker_escape(&self.family),
            self.runtime,
        )
    }
}

/// Escapes `"` and `\`, and nothing else, as the `json_escape` of
/// `crates/ori-broker/src/registration.rs` and of
/// `crates/ori-broker/src/issuance.rs` each does.
fn broker_escape(text: &str) -> String {
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

/// One `identity.created` event, read.
struct IdentityRead {
    id: Id,
    product_id: Id,
    role: Role,
}

/// Reads one `identity.created` event, refusing, with what was wrong, every
/// payload `crates/ori-broker/src/registration.rs` would refuse to read back
/// and every payload its reader could read differently from this one.
///
/// That reader finds each field as the first `"key":"` in the text: it
/// refuses a field written any other way (a space after the colon, a number)
/// and takes the first match wherever it sits (inside a nested object, or
/// the first of two copies), and it undoes only `\"` and `\\`. Requiring the
/// payload to be byte for byte what its writer writes for the six values
/// serde read makes both readers read the same six values; those are then
/// held to its rules: `id` and `product_id` an [`Id`], `role` a [`Role`],
/// `model` any text, `family` one [`ModelFamily::parse`] accepts, and
/// `runtime` one of `IDENTITY_RUNTIMES`.
///
/// It refuses more than that reader in three ways, each the refusing
/// direction: the fields in another order than the writer's, an escape the
/// writer never writes (`\/`, a `\u` escape), and a control character the
/// writer writes raw and JSON does not allow raw.
fn read_identity(event: &Event) -> Result<IdentityRead, &'static str> {
    let wire: IdentityCreatedWire = serde_json::from_str(event.payload()).map_err(|_| {
        "does not parse as the six fields ori-broker's registration writes, each once, so whose it is cannot be told"
    })?;
    if wire.as_registration_writes_it() != event.payload() {
        return Err(
            "is not laid out byte for byte as ori-broker's registration writes it, so its reader may read other values",
        );
    }
    let id = Id::parse(&wire.id).map_err(|_| "carries an id that is not an Id")?;
    let product_id =
        Id::parse(&wire.product_id).map_err(|_| "carries a product_id that is not an Id")?;
    let role: Role = wire
        .role
        .parse()
        .map_err(|_| "carries a role the table does not name")?;
    ModelFamily::parse(&wire.family)
        .map_err(|_| "carries a family ori-broker's registration refuses")?;
    if !IDENTITY_RUNTIMES.contains(&wire.runtime.as_str()) {
        return Err("carries a runtime ori-broker's registration refuses");
    }
    Ok(IdentityRead {
        id,
        product_id,
        role,
    })
}

/// The role this product's log records for `identity`. Every
/// `identity.created` event is read in full, whoever it registers, as
/// `crates/ori-broker/src/registration.rs` reads every one of them: one it
/// would refuse refuses the request, since it cannot be told whose it is.
fn registered_role(
    events: &[Event],
    product_id: &Id,
    identity: &Id,
) -> Result<Role, RefusalReason> {
    let mut roles: Vec<Role> = Vec::new();
    for event in events
        .iter()
        .filter(|event| event.kind() == IDENTITY_CREATED)
    {
        let malformed = |what: &str| RefusalReason::IdentityRecordMalformed {
            detail: format!("the event at seq {} {what}", event.seq()),
        };
        let read = read_identity(event).map_err(malformed)?;
        if &read.id != identity {
            continue;
        }
        if &read.product_id != product_id {
            return Err(malformed("registers this identity for another product"));
        }
        if !roles.contains(&read.role) {
            roles.push(read.role);
        }
    }
    match roles.as_slice() {
        [] => Err(RefusalReason::UnregisteredIdentity),
        [role] => Ok(*role),
        _ => Err(RefusalReason::IdentityAmbiguous),
    }
}

/// Whether `ticket_id` is filed in this log.
fn ticket_filed(events: &[Event], ticket_id: &Id) -> bool {
    events
        .iter()
        .any(|event| event.kind() == TICKET_FILED && event.ticket_id() == Some(ticket_id))
}

/// `lock.claimed`'s payload, the fields this module reads. `session_id` is
/// optional, as `crates/ori-store/src/projections/lock.rs` reads it.
#[derive(Deserialize)]
struct LockClaimWire {
    module: String,
    #[serde(default)]
    session_id: Option<String>,
}

/// The value of `key` as `crates/ori-store/src/projections/payload.rs`'s
/// `field` reads it for the lock projection, step for step: the text after
/// the first `"key"` anywhere in the payload, that text's first `:`, any
/// whitespace and an opening `"`, up to the next `"`, with no escape undone;
/// `None` where that reader finds none. The lock projection keys its rows by
/// what this returns for `module` and records what it returns for
/// `session_id`, so this module reads both through it too, and refuses to
/// rely on a claim serde reads differently.
/// `tests::ori_p1_022_the_lock_fold_keeps_the_rows_the_lock_projection_keeps`
/// holds the two readers together over hostile payloads.
fn projection_field<'p>(payload: &'p str, key: &str) -> Option<&'p str> {
    let needle = format!("\"{key}\"");
    let start = payload.find(needle.as_str())?;
    let after_key = payload.get(start + needle.len()..)?;
    let colon = after_key.find(':')?;
    let after_colon = after_key.get(colon + 1..)?.trim_start();
    let after_open_quote = after_colon.strip_prefix('"')?;
    let end = after_open_quote.find('"')?;
    after_open_quote.get(..end)
}

/// One `lock.claimed` event, read.
struct ClaimRead {
    /// The module text the lock projection keys the claim's row by, `None`
    /// when its reader finds none, and the projection refuses the event.
    key: Option<String>,
    /// The module, when serde reads the payload, reads the same module text
    /// as the projection's reader, and that text is a path
    /// [`ModulePath::parse`] accepts; `None` otherwise.
    module: Option<ModulePath>,
}

/// Reads a `lock.claimed` event's module both ways; see [`ClaimRead`].
fn read_claim(event: &Event) -> ClaimRead {
    let key = projection_field(event.payload(), "module");
    let module = key.and_then(|key| {
        let wire: LockClaimWire = serde_json::from_str(event.payload()).ok()?;
        if wire.module == key {
            ModulePath::parse(key).ok()
        } else {
            None
        }
    });
    ClaimRead {
        key: key.map(str::to_owned),
        module,
    }
}

/// The `lock.*` events of a log, folded the way
/// `crates/ori-store/src/projections/lock.rs` folds them into its rows.
#[derive(Default)]
struct LockFold {
    /// The live rows, keyed as the projection keys them: by the module text
    /// its reader reads, so a later claim of the same text by another ticket
    /// takes the row over. Each holds the claiming ticket and the module as
    /// [`read_claim`] reads it.
    live: BTreeMap<String, (Id, Option<ModulePath>)>,
    /// The tickets holding a claim the projection's reader cannot read, so
    /// which row it holds cannot be told, until the ticket's release.
    unreadable_live: BTreeSet<Id>,
    /// Every claim each ticket ever made, released or not, `None` where the
    /// claim cannot be read.
    ever: BTreeMap<Id, Vec<Option<ModulePath>>>,
}

/// Folds every `lock.*` event of `events` in log order, the way the lock
/// projection does: a claim upserts the row of its module text, whichever
/// ticket held it; a release deletes every row of its ticket and no other.
/// Refused when any lock event carries no ticket ([`lock_holder`]), as the
/// projection refuses it.
fn fold_locks(events: &[Event]) -> Result<LockFold, RefusalReason> {
    let mut fold = LockFold::default();
    for event in events {
        let Some(ticket) = lock_holder(event)? else {
            continue;
        };
        match event.kind() {
            LOCK_CLAIMED => {
                let claim = read_claim(event);
                fold.ever
                    .entry(ticket.clone())
                    .or_default()
                    .push(claim.module.clone());
                match claim.key {
                    Some(key) => {
                        fold.live.insert(key, (ticket.clone(), claim.module));
                    }
                    None => {
                        fold.unreadable_live.insert(ticket.clone());
                    }
                }
            }
            LOCK_RELEASED => {
                fold.live.retain(|_, (holder, _)| holder != ticket);
                fold.unreadable_live.remove(ticket);
            }
            _ => {}
        }
    }
    Ok(fold)
}

/// `credential.issued`'s payload: every field
/// `crates/ori-broker/src/issuance.rs` writes, each required the way that
/// file's own reader requires it, no other field, and none twice. An
/// issuance the broker cannot read back is one it cannot revoke either, so it
/// is never read here as live.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialIssuedWire {
    id: String,
    identity_id: String,
    session_id: String,
    scope: String,
    /// Required, and read only to lay the payload out again: an issuance's
    /// liveness is its revocation and its `expires_at`, as the broker's
    /// `is_active` has it.
    issued_at: i64,
    /// Required, `null` for an issuance only revocation retires.
    #[serde(deserialize_with = "required_nullable_millis")]
    expires_at: Option<i64>,
}

impl CredentialIssuedWire {
    /// The exact text issuance.rs's writer produces for these values: the
    /// six fields in its order with no whitespace, the four text fields
    /// escaped as its `json_escape` escapes them, and `expires_at` written
    /// `null` when absent.
    fn as_issuance_writes_it(&self) -> String {
        let expires_at = self
            .expires_at
            .map_or_else(|| "null".to_owned(), |millis| millis.to_string());
        format!(
            "{{\"id\":\"{}\",\"identity_id\":\"{}\",\"session_id\":\"{}\",\"scope\":\"{}\",\"issued_at\":{},\"expires_at\":{}}}",
            broker_escape(&self.id),
            broker_escape(&self.identity_id),
            broker_escape(&self.session_id),
            broker_escape(&self.scope),
            self.issued_at,
            expires_at,
        )
    }
}

/// `credential.revoked`'s payload: every field the broker writes, each
/// required the way its reader requires it, no other field, and none twice.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialRevokedWire {
    session_id: String,
    /// Required, and read only to lay the payload out again.
    revoked_at: i64,
    issuance_ids: Vec<String>,
}

impl CredentialRevokedWire {
    /// The exact text issuance.rs's writer produces for these values, laid
    /// out as [`CredentialIssuedWire::as_issuance_writes_it`] describes, the
    /// issuance ids quoted and separated by bare commas.
    fn as_issuance_writes_it(&self) -> String {
        let mut out = format!(
            "{{\"session_id\":\"{}\",\"revoked_at\":{},\"issuance_ids\":[",
            broker_escape(&self.session_id),
            self.revoked_at,
        );
        for (index, id) in self.issuance_ids.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push('"');
            out.push_str(&broker_escape(id));
            out.push('"');
        }
        out.push_str("]}");
        out
    }
}

/// Reads a field that must be present and may be `null`. serde reads a
/// missing `Option` field as `None` unless the field names its own reader,
/// and a missing `expires_at` is not an issuance that never expires.
fn required_nullable_millis<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i64>::deserialize(deserializer)
}

/// One `credential.issued` event, read.
struct IssuanceRead {
    holder: Id,
    session: Id,
    expires_at: Option<Timestamp>,
}

/// Reads a `credential.issued` event the way `issuance.rs` reads it back,
/// `None` for one it would refuse as malformed or could read differently.
///
/// That file's reader finds each field as the first `"key":` in the text,
/// as `crates/ori-broker/src/registration.rs`'s does (see [`read_identity`]),
/// so the payload must be byte for byte what its writer writes for the
/// values serde read, and those values are held to its rules.
fn read_issuance(event: &Event) -> Option<IssuanceRead> {
    let wire: CredentialIssuedWire = serde_json::from_str(event.payload()).ok()?;
    if wire.as_issuance_writes_it() != event.payload() {
        return None;
    }
    if Id::parse(&wire.id).is_err() || wire.scope.trim().is_empty() {
        return None;
    }
    Some(IssuanceRead {
        holder: Id::parse(&wire.identity_id).ok()?,
        session: Id::parse(&wire.session_id).ok()?,
        expires_at: wire.expires_at.map(Timestamp::from_millis),
    })
}

/// Reads a `credential.revoked` event the way `issuance.rs` reads it back,
/// returning the session it revokes; `None` for one it would refuse or could
/// read differently, as [`read_issuance`] has it.
fn read_revocation(event: &Event) -> Option<Id> {
    let wire: CredentialRevokedWire = serde_json::from_str(event.payload()).ok()?;
    if wire.as_issuance_writes_it() != event.payload() {
        return None;
    }
    if wire
        .issuance_ids
        .iter()
        .any(|text| Id::parse(text).is_err())
    {
        return None;
    }
    Id::parse(&wire.session_id).ok()
}

/// Whether an issuance expiring at `expires_at` is live at `at`: the rule of
/// `issuance.rs`'s `is_expired`, dead from its `expires_at` on.
fn unexpired_at(expires_at: Option<Timestamp>, at: Timestamp) -> bool {
    expires_at.is_none_or(|expires_at| at < expires_at)
}

/// The ticket a `lock.*` event is recorded against, read from the event's
/// own ticket column as `crates/ori-store/src/projections/lock.rs` reads it,
/// never from its payload; `None` for an event of any other kind.
///
/// A lock event with no ticket column is refused, as that projection refuses
/// it. Skipped, another ticket's claim outside the declared scope would
/// vanish from that ticket's history and let its records in, and a release
/// of the coder's own ticket would leave its claims live.
fn lock_holder(event: &Event) -> Result<Option<&Id>, RefusalReason> {
    if !event.kind().starts_with(LOCK_KIND_PREFIX) {
        return Ok(None);
    }
    event
        .ticket_id()
        .map(Some)
        .ok_or(RefusalReason::LockRecordUnattributed { seq: event.seq() })
}

/// The session a `lock.claimed` event was made under, `None` when it names
/// none. Refused when that cannot be read: the payload is not the shape the
/// lock projection reads, the projection's reader ([`projection_field`])
/// reads another `session_id` than serde does, or it is not an [`Id`].
fn claim_session(event: &Event) -> Result<Option<Id>, RefusalReason> {
    let unreadable = || RefusalReason::AssignmentRecordMalformed {
        detail: format!(
            "the lock claim at seq {} does not say readably which session it was made under",
            event.seq()
        ),
    };
    let wire: LockClaimWire = serde_json::from_str(event.payload()).map_err(|_| unreadable())?;
    if wire.session_id.as_deref() != projection_field(event.payload(), "session_id") {
        return Err(unreadable());
    }
    wire.session_id
        .map(|text| Id::parse(&text).map_err(|_| unreadable()))
        .transpose()
}

/// What this product's credential events say, read once, the way
/// `crates/ori-broker/src/issuance.rs` reads them back.
struct SessionRecord {
    /// The requesting identity.
    identity: Id,
    /// Every identity each session was ever issued to: revoked, expired or
    /// live.
    issued_to: BTreeMap<Id, BTreeSet<Id>>,
    /// The sessions the requesting identity holds a live credential for at
    /// the time decided.
    live: BTreeSet<Id>,
}

impl SessionRecord {
    /// Whether the log ever issued `session` to an identity other than the
    /// requesting one.
    fn issued_to_another(&self, session: &Id) -> bool {
        self.issued_to
            .get(session)
            .is_some_and(|holders| holders.iter().any(|holder| holder != &self.identity))
    }

    /// Whether the log ever issued any session to an identity other than
    /// the requesting one.
    fn any_issued_to_another(&self) -> bool {
        self.issued_to
            .values()
            .any(|holders| holders.iter().any(|holder| holder != &self.identity))
    }
}

/// Reads every credential event of `events` for `identity` at `at`.
///
/// A session is live while at least one of its issuances to the identity is
/// neither revoked nor past its `expires_at` at `at`, the rule of
/// `crates/ori-broker/src/issuance.rs`'s `is_active`. A `credential.revoked`
/// event ends every issuance of its session issued before it, whichever
/// issuance ids it lists: the broker writes one only from `revoke_session`,
/// which lists every unrevoked issuance of the session. Refused when a
/// credential event is one the broker would not read back.
fn read_sessions(
    events: &[Event],
    identity: &Id,
    at: Timestamp,
) -> Result<SessionRecord, RefusalReason> {
    let malformed = |event: &Event| RefusalReason::AssignmentRecordMalformed {
        detail: format!("the event at seq {} does not parse", event.seq()),
    };
    // The expiry of each of the identity's issuances of each session since
    // that session was last revoked.
    let mut issued: BTreeMap<Id, Vec<Option<Timestamp>>> = BTreeMap::new();
    let mut issued_to: BTreeMap<Id, BTreeSet<Id>> = BTreeMap::new();
    for event in events {
        match event.kind() {
            CREDENTIAL_ISSUED => {
                let IssuanceRead {
                    holder,
                    session,
                    expires_at,
                } = read_issuance(event).ok_or_else(|| malformed(event))?;
                issued_to
                    .entry(session.clone())
                    .or_default()
                    .insert(holder.clone());
                if &holder == identity {
                    issued.entry(session).or_default().push(expires_at);
                }
            }
            CREDENTIAL_REVOKED => {
                let session = read_revocation(event).ok_or_else(|| malformed(event))?;
                issued.remove(&session);
            }
            _ => {}
        }
    }
    let live = issued
        .into_iter()
        .filter(|(_, expiries)| {
            expiries
                .iter()
                .any(|expires_at| unexpired_at(*expires_at, at))
        })
        .map(|(session, _)| session)
        .collect();
    Ok(SessionRecord {
        identity: identity.clone(),
        issued_to,
        live,
    })
}

/// Every ticket this product's log ties to the identity's live sessions: the
/// tickets whose `lock.claimed` events name a session the identity holds a
/// live credential for. Empty when the log records none, which is every log
/// today (see the module doc comment, "Where every fact comes from").
///
/// Refused when which tickets are the identity's cannot be told: a live
/// session of the identity that the log also issues to another identity,
/// since a session id is not unique by construction and a claim under a
/// shared session cannot be told to be this identity's; and, while the
/// identity holds a live session, a lock claim whose session cannot be read,
/// or a lock event with no ticket. With no live session no claim can tie a
/// ticket to the identity's live sessions; whether the log ties the assigned
/// ticket to anyone else is [`check_assignment_unshared`]'s question.
fn logged_assignments(
    events: &[Event],
    sessions: &SessionRecord,
) -> Result<BTreeSet<Id>, RefusalReason> {
    for session in &sessions.live {
        if sessions.issued_to_another(session) {
            return Err(RefusalReason::AssignmentRecordMalformed {
                detail: format!("session {session} is issued to more than one identity"),
            });
        }
    }
    let mut tickets = BTreeSet::new();
    if sessions.live.is_empty() {
        return Ok(tickets);
    }
    for event in events {
        let Some(ticket_id) = lock_holder(event)? else {
            continue;
        };
        if event.kind() != LOCK_CLAIMED {
            continue;
        }
        if claim_session(event)?.is_some_and(|session| sessions.live.contains(&session)) {
            tickets.insert(ticket_id.clone());
        }
    }
    Ok(tickets)
}

/// The coder's ticket: the log's, where the log records one, and the
/// engine's otherwise. The engine's value must be one the log records when
/// the log records any; two the log records with no engine value to choose
/// between them is a refusal, and so is neither.
fn assignment(engine: Option<&Id>, logged: &BTreeSet<Id>) -> Result<Id, RefusalReason> {
    match engine {
        Some(ticket_id) if logged.is_empty() || logged.contains(ticket_id) => Ok(ticket_id.clone()),
        Some(_) => Err(RefusalReason::AssignmentContradicted),
        None => {
            let mut recorded = logged.iter();
            match (recorded.next(), recorded.next()) {
                (Some(ticket_id), None) => Ok(ticket_id.clone()),
                (Some(_), Some(_)) => Err(RefusalReason::AssignmentAmbiguous),
                (None, _) => Err(RefusalReason::NoAssignment),
            }
        }
    }
}

/// Refuses the assignment [`assignment`] settled on when anything in the log
/// ties that ticket to an identity other than the requesting one: a
/// `lock.claimed` event of the ticket naming a session the log ever issued
/// to another identity, whether that session is live, revoked or expired
/// (`assignment_held_elsewhere`). A claim of the ticket whose session cannot
/// be read may be such a claim, and is refused as such while the log issues
/// any session to another identity (`assignment_record_malformed`); a claim
/// naming no session, or a session the log never issued, ties the ticket to
/// nobody.
///
/// This is what keeps an engine value from outliving what the log says: the
/// identity's own sessions tie nothing once they are revoked or expired, or
/// before it has claimed anything, and in all three cases the engine's
/// ticket would otherwise stand whoever's ticket the log says it is.
fn check_assignment_unshared(
    events: &[Event],
    ticket_id: &Id,
    sessions: &SessionRecord,
) -> Result<(), RefusalReason> {
    for event in events {
        let Some(holder) = lock_holder(event)? else {
            continue;
        };
        if event.kind() != LOCK_CLAIMED || holder != ticket_id {
            continue;
        }
        match claim_session(event) {
            Ok(Some(session)) => {
                if sessions.issued_to_another(&session) {
                    return Err(RefusalReason::AssignmentHeldElsewhere {
                        ticket_id: ticket_id.clone(),
                    });
                }
            }
            Ok(None) => {}
            Err(unreadable) => {
                if sessions.any_issued_to_another() {
                    return Err(unreadable);
                }
            }
        }
    }
    Ok(())
}

/// The declared scope of `ticket_id`: the rows the lock projection folds
/// for it ([`fold_locks`]), and the tickets whose records are inside them
/// (see the module doc comment, "Which records are inside a declared
/// scope"). Refused when any lock event of the log carries no ticket, since
/// whose claim or release it records cannot be told ([`lock_holder`]), and
/// when a live row of the ticket, or a live claim of it the projection's
/// reader cannot read, leaves its modules unknown.
fn declared_scope(events: &[Event], ticket_id: &Id) -> Result<DeclaredScope, RefusalReason> {
    if !ticket_filed(events, ticket_id) {
        return Err(RefusalReason::TicketNotInProduct {
            ticket_id: ticket_id.clone(),
        });
    }
    let malformed = || RefusalReason::LockRecordMalformed {
        ticket_id: ticket_id.clone(),
    };
    let fold = fold_locks(events)?;
    if fold.unreadable_live.contains(ticket_id) {
        return Err(malformed());
    }
    let mut held: BTreeSet<ModulePath> = BTreeSet::new();
    for (holder, module) in fold.live.values() {
        if holder == ticket_id {
            held.insert(module.clone().ok_or_else(malformed)?);
        }
    }
    let modules: Vec<ModulePath> = held.into_iter().collect();
    let scope = Scope::new(modules.iter().map(ModulePath::as_str)).map_err(|_| malformed())?;
    let mut in_scope_tickets: BTreeSet<Id> = fold
        .ever
        .into_iter()
        .filter(|(_, claimed)| {
            !claimed.is_empty()
                && claimed.iter().all(|claim| {
                    claim
                        .as_ref()
                        .is_some_and(|path| modules.iter().any(|module| path.is_within(module)))
                })
        })
        .map(|(ticket, _)| ticket)
        .collect();
    in_scope_tickets.insert(ticket_id.clone());
    Ok(DeclaredScope {
        ticket_id: ticket_id.clone(),
        scope,
        modules,
        in_scope_tickets,
    })
}

// ---------------------------------------------------------------------------
// Checks on the request
// ---------------------------------------------------------------------------

/// Refuses a role claim the log contradicts, or one the table does not name.
fn check_claimed_role(claims: &Claims, reader: &Reader) -> Result<(), RefusalReason> {
    let Some(text) = &claims.role else {
        return Ok(());
    };
    let claimed: Role = text
        .parse()
        .map_err(|_| RefusalReason::ClaimMalformed { claim: "role" })?;
    if reader.role() == Some(claimed) {
        Ok(())
    } else {
        Err(RefusalReason::ClaimContradicted { claim: "role" })
    }
}

/// Refuses a declared scope claim that is not exactly the log's.
fn check_claimed_scope(
    claims: &Claims,
    declared: Option<&DeclaredScope>,
) -> Result<(), RefusalReason> {
    let Some(claimed) = &claims.declared_scope else {
        return Ok(());
    };
    let mut claimed_paths: BTreeSet<ModulePath> = BTreeSet::new();
    for text in claimed {
        let path = ModulePath::parse(text).map_err(|_| RefusalReason::ClaimMalformed {
            claim: "declared_scope",
        })?;
        claimed_paths.insert(path);
    }
    let claimed_names: BTreeSet<&str> = claimed_paths.iter().map(ModulePath::as_str).collect();
    let recorded = declared
        .map(DeclaredScope::module_names)
        .unwrap_or_default();
    if claimed_names == recorded {
        Ok(())
    } else {
        Err(RefusalReason::ClaimContradicted {
            claim: "declared_scope",
        })
    }
}

/// Reads every named source, refusing the first the table does not name.
fn parse_sources(names: &[String]) -> Result<Vec<Source>, RefusalReason> {
    names
        .iter()
        .enumerate()
        .map(|(position, name)| {
            Source::parse(name).ok_or(RefusalReason::UnknownSource { position })
        })
        .collect()
}

/// AICD §17's ticket read, for a context request: the operator reads any
/// ticket of the product; an agent reads what the matrix grants its role,
/// and "own" is the assignment [`assignment`] settled on.
fn check_ticket_read(
    reader: &Reader,
    ticket_id: &Id,
    assigned: Option<&Id>,
) -> Result<(), RefusalReason> {
    let (identity, role) = match reader {
        Reader::Operator { .. } => return Ok(()),
        Reader::Agent { identity, role } => (identity, *role),
    };
    match permission::permits(
        &Actor::Agent(identity.clone()),
        role,
        Resource::Tickets,
        Action::Read,
    ) {
        Decision::Allowed { limit: None } => Ok(()),
        Decision::Allowed {
            limit: Some(Constraint::Own),
        } => {
            if assigned == Some(ticket_id) {
                Ok(())
            } else {
                Err(RefusalReason::TicketNotOwn {
                    ticket_id: ticket_id.clone(),
                })
            }
        }
        Decision::Allowed {
            limit:
                Some(
                    limit @ (Constraint::SpecBranch
                    | Constraint::TierZero
                    | Constraint::Telemetry
                    | Constraint::Analytics
                    | Constraint::FromTags
                    | Constraint::IncidentTicket
                    | Constraint::ProductSignalTicket
                    | Constraint::Criteria
                    | Constraint::ViaPullRequest
                    | Constraint::Runbooks
                    | Constraint::ProductBrief),
                ),
        } => Err(RefusalReason::TicketReadLimitUnsupported {
            limit: limit.as_str(),
        }),
        Decision::Refused { .. } | Decision::NotGoverned => {
            Err(RefusalReason::TicketReadNotGranted { role })
        }
    }
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// Records `reason` as one refusal event and returns the refusal, recorded
/// or not. The only place a refusal is built.
fn refuse(
    db: &mut ProductDb,
    at: Timestamp,
    principal: &Principal,
    request: &MemoryRequest,
    reason: RefusalReason,
) -> ScopeError {
    let mut refusal = Refusal {
        actor: principal.actor().clone(),
        product_id: db.product_id().to_owned(),
        reason,
        request: request.clone(),
        event_seq: None,
    };
    match append_refusal(db, at, &refusal) {
        Ok(seq) => {
            refusal.event_seq = Some(seq);
            ScopeError::Refused(Box::new(refusal))
        }
        Err(failure) => ScopeError::RefusedUnlogged {
            refusal: Box::new(refusal),
            failure: Box::new(failure),
        },
    }
}

/// An actor, as a payload records it.
#[derive(Serialize)]
struct ActorWire {
    kind: &'static str,
    identity: Option<String>,
}

impl ActorWire {
    fn from_actor(actor: &Actor) -> Self {
        Self {
            kind: actor.kind(),
            identity: actor.identity().map(|id| id.as_str().to_owned()),
        }
    }
}

/// A methodology reference, as a payload records it.
#[derive(Serialize)]
struct MethodologyRefWire {
    section: u8,
    subsection: Option<String>,
}

impl MethodologyRefWire {
    fn from_ref(reference: MethodologyRef) -> Self {
        Self {
            section: reference.section,
            subsection: reference.subsection,
        }
    }
}

/// A request's claims, as a payload records them: typed values only.
#[derive(Serialize)]
struct ClaimsWire {
    product_id: Option<String>,
    identity: Option<String>,
    role: Option<&'static str>,
    role_unrecognized: bool,
    declared_scope_entries: Option<usize>,
}

/// A request, as a payload records it: no free text.
#[derive(Serialize)]
struct RequestWire {
    kind: &'static str,
    ticket_id: Option<String>,
    record_id: Option<String>,
    query_chars: Option<usize>,
    sources: Vec<&'static str>,
    sources_unrecognized: usize,
    claims: ClaimsWire,
}

/// The reason, as a payload records it.
#[derive(Serialize)]
struct ReasonWire {
    code: &'static str,
    detail: String,
}

/// `memory.access_refused`'s payload.
#[derive(Serialize)]
struct RefusalWire {
    product_id: String,
    actor: ActorWire,
    request: RequestWire,
    reason: ReasonWire,
    methodology_ref: MethodologyRefWire,
}

/// `memory.evidence_accessed`'s payload.
#[derive(Serialize)]
struct EvidenceAccessWire {
    product_id: String,
    actor: ActorWire,
    role: Option<&'static str>,
    record_id: Option<String>,
    evidence_ids: Vec<String>,
    untrusted: bool,
    methodology_ref: MethodologyRefWire,
}

fn request_wire(request: &MemoryRequest) -> RequestWire {
    let (ticket_id, record_id, query_chars, raw_sources) = match &request.query {
        Query::Context { ticket_id } => (Some(ticket_id.to_string()), None, None, &[][..]),
        Query::Search { text, sources } => (None, None, Some(text.chars().count()), &sources[..]),
        Query::Evidence { record_id } => (None, Some(record_id.to_string()), None, &[][..]),
        Query::Read { source } => (None, None, None, std::slice::from_ref(source)),
    };
    let mut sources: Vec<&'static str> = Vec::new();
    let mut sources_unrecognized = 0usize;
    for name in raw_sources {
        match Source::parse(name) {
            Some(source) => {
                if !sources.contains(&source.as_str()) {
                    sources.push(source.as_str());
                }
            }
            None => sources_unrecognized += 1,
        }
    }
    RequestWire {
        kind: request.query.kind(),
        ticket_id,
        record_id,
        query_chars,
        sources,
        sources_unrecognized,
        claims: claims_wire(&request.claims),
    }
}

fn claims_wire(claims: &Claims) -> ClaimsWire {
    let parsed_role = claims.role.as_deref().map(str::parse::<Role>);
    ClaimsWire {
        product_id: claims.product_id.as_ref().map(ToString::to_string),
        identity: claims.identity.as_ref().map(ToString::to_string),
        role: match &parsed_role {
            Some(Ok(role)) => Some(role.as_str()),
            Some(Err(_)) | None => None,
        },
        role_unrecognized: matches!(parsed_role, Some(Err(_))),
        declared_scope_entries: claims.declared_scope.as_ref().map(Vec::len),
    }
}

/// Appends one refusal event, returning its `seq`.
fn append_refusal(db: &mut ProductDb, at: Timestamp, refusal: &Refusal) -> Result<u64, LogFailure> {
    let product_id = Id::parse(db.product_id())
        .map_err(|_| LogFailure::ProductId(db.product_id().to_owned()))?;
    let wire = RefusalWire {
        product_id: product_id.as_str().to_owned(),
        actor: ActorWire::from_actor(&refusal.actor),
        request: request_wire(&refusal.request),
        reason: ReasonWire {
            code: refusal.reason.code(),
            detail: refusal.reason.to_string(),
        },
        methodology_ref: MethodologyRefWire::from_ref(refusal.methodology_ref()),
    };
    let payload =
        serde_json::to_string(&wire).map_err(|err| LogFailure::Payload(err.to_string()))?;
    EventLog::append(
        db.connection(),
        product_id,
        at,
        refusal.actor.clone(),
        REFUSAL_EVENT_KIND,
        None,
        payload,
    )
    .map(|event| event.seq())
    .map_err(LogFailure::EventLog)
}

/// Appends one evidence access event, returning its `seq`.
fn append_evidence_access(
    db: &mut ProductDb,
    at: Timestamp,
    authorization: &Authorization,
) -> Result<u64, LogFailure> {
    let actor = authorization.reader.actor();
    let record_id = match &authorization.query {
        Query::Evidence { record_id } => Some(record_id.as_str().to_owned()),
        Query::Context { .. } | Query::Search { .. } | Query::Read { .. } => None,
    };
    let wire = EvidenceAccessWire {
        product_id: authorization.product_id.as_str().to_owned(),
        actor: ActorWire::from_actor(&actor),
        role: authorization.reader.role().map(Role::as_str),
        record_id,
        evidence_ids: authorization
            .evidence
            .iter()
            .map(|blob| blob.id().as_str().to_owned())
            .collect(),
        untrusted: true,
        methodology_ref: MethodologyRefWire::from_ref(MethodologyRef {
            section: MEMORY_ARCHITECTURE_SECTION,
            subsection: None,
        }),
    };
    let payload =
        serde_json::to_string(&wire).map_err(|err| LogFailure::Payload(err.to_string()))?;
    EventLog::append(
        db.connection(),
        authorization.product_id.clone(),
        at,
        actor,
        EVIDENCE_ACCESS_EVENT_KIND,
        authorization.ticket_id.clone(),
        payload,
    )
    .map(|event| event.seq())
    .map_err(LogFailure::EventLog)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use ori_store::projections::Projection;
    use ori_store::projections::ProjectionError;
    use ori_store::projections::lock::LockProjection;
    use serde_json::Value;

    use super::*;
    use crate::barrier::BarrierConfig;
    use crate::barrier::FieldName;
    use crate::barrier::NewReport;
    use crate::barrier::NewReportField;
    use crate::barrier::Submission;
    use crate::barrier::submit_report;
    use crate::code_map::EntryPoint;
    use crate::code_map::Interface;
    use crate::code_map::InterfaceKind;
    use crate::code_map::Language;
    use crate::code_map::SpecCitation;

    // -------------------------------------------------------------------
    // Scratch layout and identifiers, the same self-cleaning pattern
    // barrier.rs's own tests use.
    // -------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0038-{label}-{}-{unique}",
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

    /// A valid ULID, distinguished by `label`.
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

    const fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    /// One product: its database, its id, and the scratch directory it
    /// lives in, dropped in that order.
    struct Product {
        db: ProductDb,
        id: Id,
        _scratch: Scratch,
    }

    fn product(label: &str, product_label: &str) -> Product {
        let scratch = Scratch::new(label);
        let product_id = id(product_label);
        let db = ProductDb::open(&scratch.path, product_id.as_str(), at(1))
            .expect("a fresh product database opens");
        Product {
            db,
            id: product_id,
            _scratch: scratch,
        }
    }

    fn operator_actor() -> Actor {
        Actor::Human(id("OPERATOR"))
    }

    fn append(p: &mut Product, kind: &str, ticket: Option<&Id>, payload: String) -> Event {
        EventLog::append(
            p.db.connection(),
            p.id.clone(),
            at(100),
            operator_actor(),
            kind,
            ticket.cloned(),
            payload,
        )
        .expect("a fixture event appends")
    }

    /// The payload `crates/ori-broker/src/registration.rs` writes, field for
    /// field (`tests::ori_t_0038_identity_payload_shape_matches_ori_broker_registration`
    /// checks the keys against that file).
    fn register_in(p: &mut Product, identity: &Id, role: Role, named_product: &Id) {
        let payload = format!(
            "{{\"id\":\"{identity}\",\"product_id\":\"{named_product}\",\"role\":\"{}\",\"model\":\"model-a\",\"family\":\"family-a\",\"runtime\":\"acp\"}}",
            role.as_str()
        );
        append(p, "identity.created", None, payload);
    }

    fn register(p: &mut Product, identity: &Id, role: Role) {
        let named = p.id.clone();
        register_in(p, identity, role, &named);
    }

    fn file_ticket(p: &mut Product, ticket: &Id) {
        append(
            p,
            "ticket.filed",
            Some(ticket),
            r#"{"category":"auto","kind":"feature"}"#.to_owned(),
        );
    }

    fn claim(p: &mut Product, ticket: &Id, module: &str) {
        let payload = serde_json::json!({ "module": module }).to_string();
        append(p, "lock.claimed", Some(ticket), payload);
    }

    fn release(p: &mut Product, ticket: &Id) {
        append(p, "lock.released", Some(ticket), "{}".to_owned());
    }

    fn record(
        p: &mut Product,
        label: &str,
        kind: RecordKind,
        ticket: Option<&Id>,
        evidence_label: Option<&str>,
    ) -> MemoryRecord {
        let fields = evidence_label
            .map(|evidence| NewReportField {
                name: FieldName::parse("summary").expect("a field name"),
                raw_text: "raw text as submitted".to_owned(),
                evidence_id: id(evidence),
            })
            .into_iter()
            .collect();
        submit_report(
            &mut p.db,
            at(200),
            Actor::Agent(id("AUTHOR")),
            NewReport {
                record_id: id(label),
                kind,
                ticket_id: ticket.cloned(),
                fields,
            },
            &BarrierConfig::default(),
        )
        .expect("a fixture record is submitted")
        .record()
        .clone()
    }

    /// The last `seq` in the log, read without verifying the chain, so a
    /// test that breaks the chain on purpose can still count.
    fn tip(p: &mut Product) -> u64 {
        let last: i64 =
            p.db.connection()
                .query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |row| {
                    row.get(0)
                })
                .expect("the fixture log counts");
        u64::try_from(last).expect("a seq is never negative")
    }

    fn last_event(p: &mut Product) -> Event {
        let seq = tip(p);
        EventLog::read_range(p.db.connection(), seq, seq)
            .expect("the last event reads")
            .pop()
            .expect("the log holds an event")
    }

    fn payload_json(event: &Event) -> Value {
        serde_json::from_str(event.payload()).expect("the payload is JSON")
    }

    fn read(source: &str) -> MemoryRequest {
        MemoryRequest::new(Query::Read {
            source: source.to_owned(),
        })
    }

    fn search(text: &str, sources: &[&str]) -> MemoryRequest {
        MemoryRequest::new(Query::Search {
            text: text.to_owned(),
            sources: sources.iter().map(|name| (*name).to_owned()).collect(),
        })
    }

    fn context(ticket: &Id) -> MemoryRequest {
        MemoryRequest::new(Query::Context {
            ticket_id: ticket.clone(),
        })
    }

    fn evidence(record_id: &Id) -> MemoryRequest {
        MemoryRequest::new(Query::Evidence {
            record_id: record_id.clone(),
        })
    }

    const fn standard() -> ScopeEnforcer {
        ScopeEnforcer::new(ProductStage::Standard)
    }

    const fn migration() -> ScopeEnforcer {
        ScopeEnforcer::new(ProductStage::Migration)
    }

    fn identity_of(role: Role) -> Id {
        id(&format!("ID{}", role.as_str().to_ascii_uppercase()))
    }

    fn agent(identity: &Id) -> Principal {
        Principal::authenticated(Actor::Agent(identity.clone()))
    }

    fn principal_for(role: Role, ticket: &Id) -> Principal {
        let principal = agent(&identity_of(role));
        if role == Role::Coder {
            principal.with_assigned_ticket(ticket.clone())
        } else {
            principal
        }
    }

    fn operator() -> Principal {
        Principal::authenticated(operator_actor())
    }

    /// Asserts a grant that appends nothing, which is every grant but an
    /// evidence read.
    fn granted(
        p: &mut Product,
        enforcer: ScopeEnforcer,
        principal: &Principal,
        request: &MemoryRequest,
    ) -> Authorization {
        let before = tip(p);
        let authorization = enforcer
            .authorize(&mut p.db, at(500), principal, request)
            .unwrap_or_else(|err| panic!("expected {request:?} to be granted, got: {err}"));
        assert_eq!(tip(p), before, "a grant of {request:?} appended an event");
        assert_eq!(authorization.product_id(), &p.id);
        authorization
    }

    /// Asserts a refusal with `code`, recorded as exactly one event carrying
    /// the actor, the request, the reason and AICD §25.
    fn refused(
        p: &mut Product,
        enforcer: ScopeEnforcer,
        principal: &Principal,
        request: &MemoryRequest,
        code: &str,
    ) -> ScopeError {
        let before = tip(p);
        let err = enforcer
            .authorize(&mut p.db, at(500), principal, request)
            .expect_err("expected a refusal");
        assert_eq!(
            err.reason().code(),
            code,
            "{request:?} was refused for the wrong reason: {err}"
        );
        assert!(err.is_logged(), "the refusal was not recorded: {err}");
        assert_eq!(
            err.methodology_ref(),
            MethodologyRef {
                section: 25,
                subsection: None
            }
        );
        assert_eq!(
            tip(p),
            before + 1,
            "a refusal must append exactly one event"
        );
        let event = last_event(p);
        assert_eq!(event.kind(), REFUSAL_EVENT_KIND);
        assert_eq!(event.actor(), principal.actor());
        assert_eq!(err.refusal().event_seq(), Some(event.seq()));
        let payload = payload_json(&event);
        assert_eq!(payload["reason"]["code"], code);
        assert_eq!(payload["methodology_ref"]["section"], 25);
        assert_eq!(payload["request"]["kind"], request.query.kind());
        assert_eq!(payload["product_id"], p.id.as_str());
        assert_eq!(payload["actor"]["kind"], principal.actor().kind());
        err
    }

    /// A product holding every role, ticket M claiming `crates/m` (the
    /// coder's assignment) and ticket O claiming `crates/other`.
    struct World {
        p: Product,
        ticket_m: Id,
        ticket_o: Id,
    }

    fn world(label: &str) -> World {
        let mut p = product(label, "PRODUCTA");
        for role in Role::ALL {
            register(&mut p, &identity_of(*role), *role);
        }
        let ticket_m = id("TICKETM");
        let ticket_o = id("TICKETO");
        file_ticket(&mut p, &ticket_m);
        claim(&mut p, &ticket_m, "crates/m");
        file_ticket(&mut p, &ticket_o);
        claim(&mut p, &ticket_o, "crates/other");
        World {
            p,
            ticket_m,
            ticket_o,
        }
    }

    /// A path under `spec/`, spelled in pieces: a whole path written here
    /// would be read by the specification reference check as a reference.
    fn spec_doc(file: &str, anchor: &str) -> String {
        format!("{}/{file}#{anchor}", "spec")
    }

    fn doc(product_id: &Id, kind: DocumentKind, path: &str) -> Candidate {
        Candidate::Document {
            product_id: product_id.clone(),
            hit: SearchHit {
                path: path.to_owned(),
                kind,
                title: "a title".to_owned(),
                score: -1.0,
            },
        }
    }

    fn code_module(product_id: &Id, path: &str) -> Candidate {
        Candidate::Module {
            product_id: product_id.clone(),
            module: code_map::Module {
                path: path.to_owned(),
                language: Language::Rust,
                parsed_with_errors: false,
                interfaces: Vec::new(),
                dependency_edges: Vec::new(),
                dependency_edges_truncated: false,
                entry_points: Vec::new(),
                covering_tests: Vec::new(),
                spec_sections: Vec::new(),
                spec_sections_truncated: false,
            },
        }
    }

    fn unproduced(product_id: &Id, source: Source) -> Candidate {
        Candidate::Unproduced {
            product_id: product_id.clone(),
            source,
            locator: "fixture".to_owned(),
        }
    }

    // -------------------------------------------------------------------
    // The specification, read from the repository.
    // -------------------------------------------------------------------

    fn repo_file(parts: &[&str]) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("..");
        path.push("..");
        for part in parts {
            path.push(part);
        }
        path
    }

    fn spec_text(stem: &str) -> String {
        let file = format!("{stem}.md");
        let path = repo_file(&["spec", &file]);
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
    }

    /// The rows of the first Markdown table under the heading that starts
    /// with `heading_prefix`, separator row dropped.
    fn table_under(text: &str, heading_prefix: &str) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        let mut inside = false;
        for line in text.lines() {
            if line.starts_with(heading_prefix) {
                inside = true;
                continue;
            }
            if !inside {
                continue;
            }
            if line.starts_with("## ") {
                break;
            }
            let trimmed = line.trim();
            if trimmed.starts_with('|') {
                let cells: Vec<String> = trimmed
                    .trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().to_owned())
                    .collect();
                if cells
                    .iter()
                    .all(|cell| !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':'))
                {
                    continue;
                }
                rows.push(cells);
            } else if !rows.is_empty() {
                break;
            }
        }
        rows
    }

    const CANONICAL: [Source; 6] = [
        Source::CanonicalSection,
        Source::Adr,
        Source::Criterion,
        Source::Brief,
        Source::Runbook,
        Source::InfrastructureAdr,
    ];

    /// How each phrase of the "Memory scope" column reads as sources: the
    /// interpretation the module doc comment states, written a second time
    /// here, independently of `grant`, so that the two can be compared.
    fn phrase_meaning(phrase: &str) -> Option<Vec<(Source, Filter)>> {
        let whole = |sources: &[Source]| -> Vec<(Source, Filter)> {
            sources
                .iter()
                .map(|source| (*source, Filter::Whole))
                .collect()
        };
        let meaning = match phrase.to_lowercase().as_str() {
            "canonical" => whole(&CANONICAL),
            "organizational" => whole(&[Source::Organizational]),
            "operational filtered to declared scope" => vec![
                (Source::OperationalRecord, Filter::DeclaredScope),
                (Source::OperationalDefect, Filter::DeclaredScope),
                (Source::Incident, Filter::DeclaredScope),
            ],
            "code map" => whole(&[Source::CodeMap]),
            "all layers for the product plus organizational"
            | "everything the operator can read" => whole(&Source::ALL),
            "criteria" => whole(&[Source::Criterion]),
            "operational defects" => whole(&[Source::OperationalDefect]),
            "code map (migration only: code read)" => vec![
                (Source::CodeMap, Filter::Whole),
                (Source::CodeRead, Filter::MigrationOnly),
            ],
            "runbooks" => whole(&[Source::Runbook]),
            "incidents" => whole(&[Source::Incident]),
            "infrastructure adrs" => whole(&[Source::InfrastructureAdr]),
            "merged diffs" => whole(&[Source::MergedDiff]),
            "analytics summaries" => whole(&[Source::AnalyticsSummary]),
            "brief" => whole(&[Source::Brief]),
            _ => return None,
        };
        Some(meaning)
    }

    /// Where an acceptance criterion narrows a cell: ORI-P1-022's "the code
    /// map of M".
    const NARROWINGS: [(Role, Source, Filter); 1] =
        [(Role::Coder, Source::CodeMap, Filter::DeclaredScope)];

    /// The roles `spec/API_SPEC.md` section 3 grants `aicd_evidence`.
    fn api_spec_tool_roles(tool_prefix: &str) -> String {
        let text = spec_text("API_SPEC");
        let rows = table_under(&text, "## 3.");
        assert!(
            rows.len() > 3,
            "the MCP tool table was not found, so nothing below stands for anything"
        );
        rows.iter()
            .find(|row| row[0].starts_with(tool_prefix))
            .unwrap_or_else(|| panic!("no row for {tool_prefix} in the MCP tool table"))[1]
            .clone()
    }

    fn api_spec_evidence_roles() -> Vec<Role> {
        api_spec_tool_roles("`aicd_evidence(")
            .split(", ")
            .map(|name| {
                name.parse::<Role>()
                    .unwrap_or_else(|_| panic!("{name:?} in the aicd_evidence row is not a role"))
            })
            .collect()
    }

    // -------------------------------------------------------------------
    // The table against the specification and against its own doc.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0038_grant_table_matches_env_setup_memory_scope_column() {
        let text = spec_text("ENV_SETUP");
        let rows = table_under(&text, "## 5.");
        assert!(
            rows.len() >= 10,
            "the permission manifest table was not found whole: {} rows",
            rows.len()
        );
        let column = rows[0]
            .iter()
            .position(|cell| cell == "Memory scope")
            .expect("the manifest has a Memory scope column");
        let evidence_roles = api_spec_evidence_roles();

        let mut seen: Vec<Role> = Vec::new();
        let mut operator_rows = 0;
        for row in &rows[1..] {
            let identity = row[0].as_str();
            let cell = row.get(column).expect("every row has the column");
            if identity == "operator (human)" {
                operator_rows += 1;
                assert_eq!(cell, "Everything");
                for source in Source::ALL {
                    assert_eq!(operator_grant(source), Access::Granted(Filter::Whole));
                }
                continue;
            }
            let Ok(role) = identity.parse::<Role>() else {
                // The bootstrap implementor is not a Role: no identity.created
                // event can name it, so the enforcer refuses it as
                // unregistered.
                assert!(
                    identity.starts_with("implementor"),
                    "row {identity:?} is neither a role nor the bootstrap implementor"
                );
                continue;
            };
            seen.push(role);

            let mut expected: BTreeMap<Source, Filter> = BTreeMap::new();
            for phrase in cell.split(", ") {
                let meaning = phrase_meaning(phrase).unwrap_or_else(|| {
                    panic!(
                        "{role}'s Memory scope cell {cell:?} holds {phrase:?}, which this test \
                         cannot read: the specification or this module changed, and the \
                         change needs deciding"
                    )
                });
                for (source, filter) in meaning {
                    if let Some(previous) = expected.insert(source, filter) {
                        assert_eq!(previous, filter, "{role}: {source} read two ways");
                    }
                }
            }
            for (narrowed, source, filter) in NARROWINGS {
                if narrowed == role {
                    assert!(expected.contains_key(&source));
                    expected.insert(source, filter);
                }
            }
            if evidence_roles.contains(&role) {
                expected.insert(Source::RawEvidence, Filter::Whole);
            } else {
                expected.remove(&Source::RawEvidence);
            }

            for source in Source::ALL {
                let want = expected
                    .get(&source)
                    .map_or(Access::Denied, |filter| Access::Granted(*filter));
                assert_eq!(
                    grant(role, source),
                    want,
                    "{role} on {source}: the specification's cell reads {cell:?}"
                );
            }
        }
        assert_eq!(operator_rows, 1);
        for role in Role::ALL {
            assert_eq!(
                seen.iter().filter(|found| *found == role).count(),
                1,
                "{role} must have exactly one row in the manifest"
            );
        }
    }

    #[test]
    fn ori_t_0038_raw_evidence_grants_match_api_spec_aicd_evidence_roles() {
        let roles = api_spec_evidence_roles();
        assert_eq!(roles.len(), 3, "the aicd_evidence row read {roles:?}");
        for role in Role::ALL {
            assert_eq!(
                matches!(grant(*role, Source::RawEvidence), Access::Granted(_)),
                roles.contains(role),
                "{role}'s raw evidence grant disagrees with the aicd_evidence row"
            );
        }
        // Context and search are listed for every role in the same table.
        // Search is granted every role over its row
        // (tests::ori_t_0038_every_role_reaches_exactly_its_env_setup_row).
        // Context is a ticket read, and AICD §17 grants product signal no
        // ticket read, so context is refused it by role alone
        // (tests::ori_t_0038_context_authorizes_each_roles_whole_row_and_never_raw_evidence):
        // the conflict the module doc comment records. Should either text
        // change, this test fails, and the refusal is to be revisited.
        assert_eq!(api_spec_tool_roles("`aicd_context("), "all");
        assert_eq!(api_spec_tool_roles("`aicd_search("), "all");
        for role in Role::ALL {
            let reads_tickets = matches!(
                permission::permits(
                    &Actor::Agent(identity_of(*role)),
                    *role,
                    Resource::Tickets,
                    Action::Read,
                ),
                Decision::Allowed { .. }
            );
            assert_eq!(
                reads_tickets,
                *role != Role::ProductSignal,
                "{role}'s ticket read in AICD section 17's matrix"
            );
        }
    }

    #[test]
    fn ori_t_0038_module_doc_table_matches_the_grant_function() {
        let text = include_str!("scope.rs");
        let header: Vec<&str> = text
            .lines()
            .find(|line| line.starts_with("//! | Source |"))
            .expect("the module doc comment carries the table")
            .trim_start_matches("//!")
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        assert_eq!(header.len(), 9, "the header is {header:?}");

        let mut rows = 0;
        for line in text.lines().filter(|line| line.starts_with("//! | `")) {
            let cells: Vec<&str> = line
                .trim_start_matches("//!")
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            let source = Source::parse(cells[0].trim_matches('`'))
                .unwrap_or_else(|| panic!("{} is not a source", cells[0]));
            assert_eq!(cells.len(), header.len(), "{line}");
            for (reader, cell) in header[1..].iter().zip(&cells[1..]) {
                let access = if *reader == "operator" {
                    operator_grant(source)
                } else {
                    grant(reader.parse::<Role>().expect("a role column"), source)
                };
                assert_eq!(access.as_str(), *cell, "{reader} on {source}");
            }
            rows += 1;
        }
        assert_eq!(rows, Source::ALL.len());
    }

    // -------------------------------------------------------------------
    // Reachability: every grant is reachable and every other source is
    // refused, per role, against a literal row (not against `grant`).
    // -------------------------------------------------------------------

    /// Each role's row of `spec/ENV_SETUP.md` section 5, as source names a
    /// plain read may reach in a product that is not being migrated.
    fn env_setup_row(role: Role) -> &'static [&'static str] {
        match role {
            Role::Coder => &[
                "canonical_section",
                "adr",
                "criterion",
                "brief",
                "runbook",
                "infrastructure_adr",
                "organizational",
                "operational_record",
                "operational_defect",
                "incident",
                "code_map",
            ],
            Role::Lead | Role::Assistant => &[
                "canonical_section",
                "adr",
                "criterion",
                "brief",
                "runbook",
                "infrastructure_adr",
                "organizational",
                "operational_record",
                "operational_defect",
                "incident",
                "code_map",
                "merged_diff",
                "analytics_summary",
                "code_read",
            ],
            Role::Qa => &[
                "canonical_section",
                "adr",
                "criterion",
                "brief",
                "runbook",
                "infrastructure_adr",
                "operational_defect",
                "code_map",
            ],
            Role::Operations => &["runbook", "incident", "infrastructure_adr"],
            Role::Documentation => &[
                "canonical_section",
                "adr",
                "criterion",
                "brief",
                "runbook",
                "infrastructure_adr",
                "organizational",
                "merged_diff",
                "code_map",
            ],
            Role::ProductSignal => &["analytics_summary", "brief"],
        }
    }

    const CODER_SCOPE_FILTERED: [&str; 4] = [
        "operational_record",
        "operational_defect",
        "incident",
        "code_map",
    ];

    /// A role's literal row with the filter each source is read under, as
    /// the retrieval must receive it: the coder's four scope-filtered
    /// sources under `DeclaredScope`, qa's code read added under
    /// `MigrationOnly` during a migration, everything else `Whole`.
    fn row_map(role: Role, stage: ProductStage) -> BTreeMap<Source, Filter> {
        let mut out: BTreeMap<Source, Filter> = env_setup_row(role)
            .iter()
            .map(|name| {
                let source = Source::parse(name).expect("a source name");
                let filter = if role == Role::Coder && CODER_SCOPE_FILTERED.contains(name) {
                    Filter::DeclaredScope
                } else {
                    Filter::Whole
                };
                (source, filter)
            })
            .collect();
        if role == Role::Qa && stage == ProductStage::Migration {
            out.insert(Source::CodeRead, Filter::MigrationOnly);
        }
        out
    }

    /// The operator's row: every source whole, raw evidence only on an
    /// explicit request.
    fn operator_row() -> BTreeMap<Source, Filter> {
        Source::ALL
            .into_iter()
            .filter(|source| *source != Source::RawEvidence)
            .map(|source| (source, Filter::Whole))
            .collect()
    }

    #[test]
    fn ori_t_0038_every_role_reaches_exactly_its_env_setup_row() {
        let mut w = world("sweep");
        for role in Role::ALL {
            let role = *role;
            let principal = principal_for(role, &w.ticket_m);
            let row = env_setup_row(role);
            for source in Source::ALL {
                let request = read(source.as_str());
                if row.contains(&source.as_str()) {
                    let authorization = granted(&mut w.p, standard(), &principal, &request);
                    let filter =
                        if role == Role::Coder && CODER_SCOPE_FILTERED.contains(&source.as_str()) {
                            Filter::DeclaredScope
                        } else {
                            Filter::Whole
                        };
                    assert_eq!(authorization.sources().len(), 1, "{role} {source}");
                    assert_eq!(authorization.sources().get(&source), Some(&filter));
                    assert_eq!(authorization.role(), Some(role));
                } else if source == Source::RawEvidence {
                    refused(
                        &mut w.p,
                        standard(),
                        &principal,
                        &request,
                        "evidence_only_on_explicit_request",
                    );
                } else if role == Role::Qa && source == Source::CodeRead {
                    refused(
                        &mut w.p,
                        standard(),
                        &principal,
                        &request,
                        "not_in_migration",
                    );
                    let authorization = granted(&mut w.p, migration(), &principal, &request);
                    assert_eq!(
                        authorization.sources().get(&Source::CodeRead),
                        Some(&Filter::MigrationOnly)
                    );
                } else {
                    refused(
                        &mut w.p,
                        standard(),
                        &principal,
                        &request,
                        "source_not_in_scope",
                    );
                }
            }

            // A search naming no source reaches the whole row at once, and so
            // does one naming the whole row: each source under the same
            // filter a read of it gets, never a wider one.
            let authorization = granted(&mut w.p, standard(), &principal, &search("x", &[]));
            let names: BTreeSet<&str> = authorization
                .sources()
                .keys()
                .map(|source| source.as_str())
                .collect();
            let expected: BTreeSet<&str> = row.iter().copied().collect();
            assert_eq!(names, expected, "{role}'s whole-row search");
            assert_eq!(
                authorization.sources(),
                &row_map(role, ProductStage::Standard),
                "{role}'s whole-row search, filters included"
            );
            let authorization = granted(&mut w.p, standard(), &principal, &search("x", row));
            assert_eq!(
                authorization.sources(),
                &row_map(role, ProductStage::Standard),
                "{role}'s search naming its row, filters included"
            );
        }
    }

    #[test]
    fn ori_t_0038_context_authorizes_each_roles_whole_row_and_never_raw_evidence() {
        let mut w = world("context-rows");
        let ticket = w.ticket_m.clone();
        for role in Role::ALL {
            let role = *role;
            let principal = principal_for(role, &ticket);
            if role == Role::ProductSignal {
                refused(
                    &mut w.p,
                    standard(),
                    &principal,
                    &context(&ticket),
                    "ticket_read_not_granted",
                );
                continue;
            }
            let authorization = granted(&mut w.p, standard(), &principal, &context(&ticket));
            let names: BTreeSet<&str> = authorization
                .sources()
                .keys()
                .map(|source| source.as_str())
                .collect();
            let expected: BTreeSet<&str> = env_setup_row(role).iter().copied().collect();
            assert_eq!(names, expected, "{role}'s context sources");
            assert_eq!(
                authorization.sources(),
                &row_map(role, ProductStage::Standard),
                "{role}'s context sources, filters included"
            );
            assert!(!authorization.allows(Source::RawEvidence));
            assert!(authorization.evidence().is_empty());
            assert_eq!(authorization.ticket_id(), Some(&ticket));
        }

        let qa = principal_for(Role::Qa, &ticket);
        let authorization = granted(&mut w.p, migration(), &qa, &context(&ticket));
        assert_eq!(
            authorization.sources().get(&Source::CodeRead),
            Some(&Filter::MigrationOnly)
        );
        assert_eq!(
            authorization.sources(),
            &row_map(Role::Qa, ProductStage::Migration)
        );

        let authorization = granted(&mut w.p, standard(), &operator(), &context(&ticket));
        assert_eq!(authorization.sources().len(), Source::ALL.len() - 1);
        assert_eq!(authorization.sources(), &operator_row());
        assert!(!authorization.allows(Source::RawEvidence));
        assert_eq!(authorization.role(), None);
    }

    #[test]
    fn ori_t_0038_product_signal_reads_only_analytics_and_the_brief() {
        let mut w = world("product-signal");
        let principal = principal_for(Role::ProductSignal, &w.ticket_m);
        let authorization = granted(&mut w.p, standard(), &principal, &search("drop-off", &[]));
        let names: BTreeSet<&str> = authorization
            .sources()
            .keys()
            .map(|source| source.as_str())
            .collect();
        assert_eq!(
            names,
            ["analytics_summary", "brief"]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(
            authorization.sources(),
            &row_map(Role::ProductSignal, ProductStage::Standard)
        );
        refused(
            &mut w.p,
            standard(),
            &principal,
            &search("drop-off", &["brief", "adr"]),
            "source_not_in_scope",
        );
    }

    #[test]
    fn ori_t_0038_operator_reads_every_source_and_raw_evidence_only_on_request() {
        let mut w = world("operator");
        for source in Source::ALL {
            let request = read(source.as_str());
            if source == Source::RawEvidence {
                refused(
                    &mut w.p,
                    standard(),
                    &operator(),
                    &request,
                    "evidence_only_on_explicit_request",
                );
            } else {
                let authorization = granted(&mut w.p, standard(), &operator(), &request);
                assert_eq!(authorization.sources().get(&source), Some(&Filter::Whole));
                assert_eq!(authorization.reader().actor(), operator_actor());
            }
        }
        refused(
            &mut w.p,
            standard(),
            &operator(),
            &context(&id("NEVERFILED")),
            "ticket_not_in_product",
        );
    }

    // -------------------------------------------------------------------
    // Failing closed on who is asking.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0038_system_actor_is_refused_and_logged() {
        let mut w = world("system");
        let system = Principal::authenticated(Actor::System);
        refused(
            &mut w.p,
            standard(),
            &system,
            &read("canonical_section"),
            "system_principal",
        );
        let event = last_event(&mut w.p);
        assert_eq!(event.actor(), &Actor::System);
    }

    #[test]
    fn ori_t_0038_unregistered_identity_is_refused_not_treated_as_coder() {
        let mut w = world("unregistered");
        let stranger = id("STRANGER");
        refused(
            &mut w.p,
            standard(),
            &agent(&stranger),
            &read("canonical_section"),
            "unregistered_identity",
        );
        // Even carrying the assignment a coder would carry.
        let as_if_coder = agent(&stranger).with_assigned_ticket(w.ticket_m.clone());
        refused(
            &mut w.p,
            standard(),
            &as_if_coder,
            &context(&w.ticket_m),
            "unregistered_identity",
        );
        refused(
            &mut w.p,
            standard(),
            &as_if_coder,
            &read("operational_record"),
            "unregistered_identity",
        );
        let event = last_event(&mut w.p);
        assert_eq!(event.actor(), &Actor::Agent(stranger));
    }

    #[test]
    fn ori_t_0038_malformed_foreign_or_conflicting_identity_records_are_refused() {
        // A payload that does not parse: nobody can be told apart, so
        // everybody is refused.
        let mut p = product("identity-garbage", "PRODUCTC");
        let lead = identity_of(Role::Lead);
        register(&mut p, &lead, Role::Lead);
        append(&mut p, "identity.created", None, "not json".to_owned());
        refused(
            &mut p,
            standard(),
            &agent(&lead),
            &read("adr"),
            "identity_record_malformed",
        );

        // Registered for another product, though written in this log.
        let mut p = product("identity-foreign", "PRODUCTC");
        let foreign = id("PRODUCTD");
        register_in(&mut p, &lead, Role::Lead, &foreign);
        refused(
            &mut p,
            standard(),
            &agent(&lead),
            &read("adr"),
            "identity_record_malformed",
        );

        // A role the table does not name.
        let mut p = product("identity-role", "PRODUCTC");
        let payload = format!(
            "{{\"id\":\"{lead}\",\"product_id\":\"{}\",\"role\":\"janitor\"}}",
            p.id
        );
        append(&mut p, "identity.created", None, payload);
        refused(
            &mut p,
            standard(),
            &agent(&lead),
            &read("adr"),
            "identity_record_malformed",
        );

        // Two roles for one identity.
        let mut p = product("identity-two-roles", "PRODUCTC");
        register(&mut p, &lead, Role::Lead);
        register(&mut p, &lead, Role::Coder);
        refused(
            &mut p,
            standard(),
            &agent(&lead),
            &read("adr"),
            "identity_ambiguous",
        );

        // The same role twice is one role.
        let mut p = product("identity-same-twice", "PRODUCTC");
        register(&mut p, &lead, Role::Lead);
        register(&mut p, &lead, Role::Lead);
        let authorization = granted(&mut p, standard(), &agent(&lead), &read("adr"));
        assert_eq!(authorization.role(), Some(Role::Lead));
    }

    #[test]
    fn ori_t_0038_role_product_identity_and_scope_come_from_the_log_never_the_request() {
        let mut w = world("claims");
        let coder = principal_for(Role::Coder, &w.ticket_m);
        let lead = principal_for(Role::Lead, &w.ticket_m);

        let with_claims =
            |request: MemoryRequest, claims: Claims| MemoryRequest { claims, ..request };

        // A coder claiming to be a lead gets nothing a lead would.
        let request = with_claims(
            read("merged_diff"),
            Claims {
                role: Some("lead".to_owned()),
                ..Claims::default()
            },
        );
        refused(&mut w.p, standard(), &coder, &request, "claim_contradicted");
        let request = with_claims(
            read("canonical_section"),
            Claims {
                role: Some("lead".to_owned()),
                ..Claims::default()
            },
        );
        refused(&mut w.p, standard(), &coder, &request, "claim_contradicted");
        let request = with_claims(
            read("canonical_section"),
            Claims {
                role: Some("janitor".to_owned()),
                ..Claims::default()
            },
        );
        refused(&mut w.p, standard(), &coder, &request, "claim_malformed");
        let request = with_claims(
            read("canonical_section"),
            Claims {
                role: Some("coder".to_owned()),
                ..Claims::default()
            },
        );
        let authorization = granted(&mut w.p, standard(), &coder, &request);
        assert_eq!(authorization.role(), Some(Role::Coder));

        // The operator holds no role to claim.
        let request = with_claims(
            read("canonical_section"),
            Claims {
                role: Some("lead".to_owned()),
                ..Claims::default()
            },
        );
        refused(
            &mut w.p,
            standard(),
            &operator(),
            &request,
            "claim_contradicted",
        );

        // Another product, another identity.
        let request = with_claims(
            read("adr"),
            Claims {
                product_id: Some(id("PRODUCTB")),
                ..Claims::default()
            },
        );
        refused(&mut w.p, standard(), &lead, &request, "claim_contradicted");
        let request = with_claims(
            read("adr"),
            Claims {
                product_id: Some(w.p.id.clone()),
                identity: Some(identity_of(Role::Lead)),
                ..Claims::default()
            },
        );
        granted(&mut w.p, standard(), &lead, &request);
        let request = with_claims(
            read("adr"),
            Claims {
                identity: Some(identity_of(Role::Lead)),
                ..Claims::default()
            },
        );
        refused(&mut w.p, standard(), &coder, &request, "claim_contradicted");

        // A declared scope wider than the log's, or different from it, is
        // refused; the log's own, spelled with a trailing slash, is not.
        for (claimed, code) in [
            (vec!["crates"], Some("claim_contradicted")),
            (vec!["crates/m", "crates/n"], Some("claim_contradicted")),
            (vec!["crates/other"], Some("claim_contradicted")),
            (vec!["crates/m/../other"], Some("claim_malformed")),
            (vec!["crates/m/"], None),
        ] {
            let request = with_claims(
                read("code_map"),
                Claims {
                    declared_scope: Some(claimed.iter().map(|m| (*m).to_owned()).collect()),
                    ..Claims::default()
                },
            );
            match code {
                Some(code) => {
                    refused(&mut w.p, standard(), &coder, &request, code);
                }
                None => {
                    let authorization = granted(&mut w.p, standard(), &coder, &request);
                    assert_eq!(
                        authorization
                            .declared_modules()
                            .iter()
                            .map(ModulePath::as_str)
                            .collect::<Vec<_>>(),
                        vec!["crates/m"]
                    );
                }
            }
        }
        // A role with no declared scope claiming one.
        let request = with_claims(
            read("adr"),
            Claims {
                declared_scope: Some(vec!["crates/m".to_owned()]),
                ..Claims::default()
            },
        );
        refused(&mut w.p, standard(), &lead, &request, "claim_contradicted");
    }

    // -------------------------------------------------------------------
    // The coder's ticket and declared scope.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0038_coder_is_refused_another_tickets_context() {
        let mut w = world("read-own");
        let coder = principal_for(Role::Coder, &w.ticket_m);
        let authorization = granted(&mut w.p, standard(), &coder, &context(&w.ticket_m));
        assert_eq!(authorization.ticket_id(), Some(&w.ticket_m));
        let other = w.ticket_o.clone();
        refused(
            &mut w.p,
            standard(),
            &coder,
            &context(&other),
            "ticket_not_own",
        );
        refused(
            &mut w.p,
            standard(),
            &coder,
            &context(&id("NEVERFILED")),
            "ticket_not_in_product",
        );
        // The lead reads any ticket of the product.
        let lead = principal_for(Role::Lead, &w.ticket_m);
        granted(&mut w.p, standard(), &lead, &context(&other));
    }

    #[test]
    fn ori_t_0038_coder_without_a_filed_assignment_is_refused() {
        let mut w = world("assignment");
        let unassigned = agent(&identity_of(Role::Coder));
        refused(
            &mut w.p,
            standard(),
            &unassigned,
            &read("canonical_section"),
            "no_assignment",
        );
        let foreign = agent(&identity_of(Role::Coder)).with_assigned_ticket(id("NEVERFILED"));
        refused(
            &mut w.p,
            standard(),
            &foreign,
            &read("canonical_section"),
            "ticket_not_in_product",
        );
    }

    #[test]
    fn ori_t_0038_declared_scope_is_the_live_lock_claims_of_the_assigned_ticket() {
        let mut w = world("live-claims");
        let ticket = w.ticket_m.clone();
        let coder = principal_for(Role::Coder, &ticket);
        claim(&mut w.p, &ticket, "crates/n/");
        let modules = |authorization: &Authorization| -> Vec<String> {
            authorization
                .declared_modules()
                .iter()
                .map(|module| module.as_str().to_owned())
                .collect()
        };
        let authorization = granted(&mut w.p, standard(), &coder, &read("code_map"));
        assert_eq!(modules(&authorization), vec!["crates/m", "crates/n"]);
        let scope = authorization
            .declared_scope()
            .expect("a coder's authorization carries its declared scope");
        assert!(scope.claims("crates/n/src/lib.rs"));
        assert!(!scope.claims("crates/other/src/lib.rs"));

        release(&mut w.p, &ticket);
        let authorization = granted(&mut w.p, standard(), &coder, &read("code_map"));
        assert!(modules(&authorization).is_empty());

        claim(&mut w.p, &ticket, "crates/p");
        let authorization = granted(&mut w.p, standard(), &coder, &read("code_map"));
        assert_eq!(modules(&authorization), vec!["crates/p"]);

        // Another ticket's malformed claim narrows nothing of this one.
        let other = w.ticket_o.clone();
        claim(&mut w.p, &other, "../escape");
        granted(&mut w.p, standard(), &coder, &read("code_map"));

        // A live malformed claim of the coder's own ticket leaves its scope
        // unknown, which is a refusal.
        claim(&mut w.p, &ticket, "crates/../escape");
        refused(
            &mut w.p,
            standard(),
            &coder,
            &read("code_map"),
            "lock_record_malformed",
        );
        release(&mut w.p, &ticket);
        append(
            &mut w.p,
            "lock.claimed",
            Some(&ticket),
            "not json".to_owned(),
        );
        refused(
            &mut w.p,
            standard(),
            &coder,
            &read("code_map"),
            "lock_record_malformed",
        );
        // Released, it no longer counts.
        release(&mut w.p, &ticket);
        granted(&mut w.p, standard(), &coder, &read("code_map"));
    }

    /// The payload `crates/ori-broker/src/issuance.rs` writes for
    /// `credential.issued`, field for field.
    fn issue(p: &mut Product, identity: &Id, session: &Id) {
        let payload = format!(
            "{{\"id\":\"{}\",\"identity_id\":\"{identity}\",\"session_id\":\"{session}\",\"scope\":\"branch\",\"issued_at\":100,\"expires_at\":null}}",
            id("ISSUANCE")
        );
        append(p, "credential.issued", None, payload);
    }

    fn revoke(p: &mut Product, session: &Id) {
        let payload =
            format!("{{\"session_id\":\"{session}\",\"revoked_at\":300,\"issuance_ids\":[]}}");
        append(p, "credential.revoked", None, payload);
    }

    fn claim_in_session(p: &mut Product, ticket: &Id, module: &str, session: &Id) {
        let payload =
            serde_json::json!({ "module": module, "session_id": session.as_str() }).to_string();
        append(p, "lock.claimed", Some(ticket), payload);
    }

    #[test]
    fn ori_t_0038_the_assignment_is_read_from_the_log_where_the_log_records_it() {
        let mut w = world("logged-assignment");
        let coder = identity_of(Role::Coder);
        let ticket_m = w.ticket_m.clone();
        let ticket_o = w.ticket_o.clone();
        let session_m = id("SESSIONM");
        issue(&mut w.p, &coder, &session_m);
        claim_in_session(&mut w.p, &ticket_m, "crates/m", &session_m);

        // No engine value: the log's ticket, and "read own" is that ticket.
        let unassigned = agent(&coder);
        let authorization = granted(&mut w.p, standard(), &unassigned, &context(&ticket_m));
        assert_eq!(authorization.ticket_id(), Some(&ticket_m));
        refused(
            &mut w.p,
            standard(),
            &unassigned,
            &context(&ticket_o),
            "ticket_not_own",
        );
        // The engine agreeing with the log, and contradicting it.
        let on_m = agent(&coder).with_assigned_ticket(ticket_m.clone());
        let on_o = agent(&coder).with_assigned_ticket(ticket_o.clone());
        granted(&mut w.p, standard(), &on_m, &read("code_map"));
        refused(
            &mut w.p,
            standard(),
            &on_o,
            &read("code_map"),
            "assignment_contradicted",
        );

        // Another identity's session ties nothing to this one. It claims a
        // ticket of its own: a ticket it claimed would be tied to it, and
        // never this identity's (see
        // tests::ori_p1_022_an_engine_ticket_the_log_ties_to_another_identity_is_refused_live_revoked_expired_or_before_any_claim).
        let session_x = id("SESSIONX");
        let ticket_p = id("TICKETP");
        file_ticket(&mut w.p, &ticket_p);
        issue(&mut w.p, &identity_of(Role::Qa), &session_x);
        claim_in_session(&mut w.p, &ticket_p, "crates/p", &session_x);
        let authorization = granted(&mut w.p, standard(), &unassigned, &read("code_map"));
        assert_eq!(authorization.ticket_id(), Some(&ticket_m));

        // Two live sessions on two tickets: the engine must choose, and may
        // choose only one the log records.
        let session_o = id("SESSIONO");
        issue(&mut w.p, &coder, &session_o);
        claim_in_session(&mut w.p, &ticket_o, "crates/other", &session_o);
        refused(
            &mut w.p,
            standard(),
            &unassigned,
            &read("code_map"),
            "assignment_ambiguous",
        );
        let authorization = granted(&mut w.p, standard(), &on_o, &read("code_map"));
        assert_eq!(authorization.ticket_id(), Some(&ticket_o));

        // Revoked sessions tie nothing: the engine's value alone again.
        revoke(&mut w.p, &session_m);
        revoke(&mut w.p, &session_o);
        refused(
            &mut w.p,
            standard(),
            &unassigned,
            &read("code_map"),
            "no_assignment",
        );
        granted(&mut w.p, standard(), &on_o, &read("code_map"));

        // A credential event that does not parse leaves whose sessions are
        // whose unknown.
        append(&mut w.p, "credential.issued", None, "not json".to_owned());
        refused(
            &mut w.p,
            standard(),
            &on_m,
            &read("code_map"),
            "assignment_record_malformed",
        );
    }

    #[test]
    fn ori_t_0038_a_session_issued_to_two_identities_ties_no_ticket_to_either() {
        let mut w = world("shared-session");
        let coder = identity_of(Role::Coder);
        let ticket_m = w.ticket_m.clone();
        let ticket_o = w.ticket_o.clone();
        // Another identity's session, and a claim of ticket O made under it.
        let shared = id("SESSIONSHARED");
        issue(&mut w.p, &identity_of(Role::Qa), &shared);
        claim_in_session(&mut w.p, &ticket_o, "crates/other", &shared);
        // The coder's own session ties ticket M to it.
        let own = id("SESSIONOWN");
        issue(&mut w.p, &coder, &own);
        claim_in_session(&mut w.p, &ticket_m, "crates/m", &own);
        let unassigned = agent(&coder);
        let authorization = granted(&mut w.p, standard(), &unassigned, &read("code_map"));
        assert_eq!(authorization.ticket_id(), Some(&ticket_m));

        // The same session id issued to the coder too: which ticket is the
        // coder's cannot be told, and O's claim must not become its own.
        revoke(&mut w.p, &own);
        issue(&mut w.p, &coder, &shared);
        for principal in [
            agent(&coder),
            agent(&coder).with_assigned_ticket(ticket_o.clone()),
        ] {
            refused(
                &mut w.p,
                standard(),
                &principal,
                &context(&ticket_o),
                "assignment_record_malformed",
            );
        }
        // Revoked, the shared session ties nothing again.
        revoke(&mut w.p, &shared);
        let on_m = agent(&coder).with_assigned_ticket(ticket_m.clone());
        granted(&mut w.p, standard(), &on_m, &context(&ticket_m));
    }

    /// `credential.issued` as `crates/ori-broker/src/issuance.rs` writes it,
    /// field for field, with an `expires_at`.
    fn issue_expiring(p: &mut Product, identity: &Id, session: &Id, expires_at: i64) {
        let payload = format!(
            "{{\"id\":\"{}\",\"identity_id\":\"{identity}\",\"session_id\":\"{session}\",\"scope\":\"branch\",\"issued_at\":100,\"expires_at\":{expires_at}}}",
            id("ISSUANCE")
        );
        append(p, "credential.issued", None, payload);
    }

    /// Decides `request` at `when`, rather than at the fixed time the
    /// `granted` and `refused` helpers use.
    fn authorize_at(
        p: &mut Product,
        when: i64,
        principal: &Principal,
        request: &MemoryRequest,
    ) -> Result<Authorization, ScopeError> {
        standard().authorize(&mut p.db, at(when), principal, request)
    }

    fn module_names(authorization: &Authorization) -> Vec<&str> {
        authorization
            .declared_modules()
            .iter()
            .map(ModulePath::as_str)
            .collect()
    }

    #[test]
    fn ori_p1_022_a_credential_past_its_expires_at_ties_no_ticket_whether_or_not_it_was_revoked() {
        let mut w = world("expired-session");
        let coder = identity_of(Role::Coder);
        let ticket_m = w.ticket_m.clone();
        let ticket_o = w.ticket_o.clone();
        // A credential for a session that expires at 400 and is never
        // revoked, and a claim of ticket O made under that session.
        let dead = id("SESSIONDEAD");
        issue_expiring(&mut w.p, &coder, &dead, 400);
        claim_in_session(&mut w.p, &ticket_o, "crates/other", &dead);
        let unassigned = agent(&coder);
        let on_m = agent(&coder).with_assigned_ticket(ticket_m.clone());

        // Before it expires the session is live: the log assigns O, and the
        // engine saying M contradicts it.
        let early = authorize_at(&mut w.p, 399, &unassigned, &read("code_map"))
            .unwrap_or_else(|err| panic!("the session is live at 399: {err}"));
        assert_eq!(early.ticket_id(), Some(&ticket_o));
        let err = authorize_at(&mut w.p, 399, &on_m, &read("code_map"))
            .expect_err("M contradicts the live session's ticket");
        assert_eq!(err.reason().code(), "assignment_contradicted");

        // At its expires_at and after, it is dead, as ori-broker's
        // `is_active` has it (`now >= expires_at`), revoked or not: O is
        // never the coder's, and the engine's M stands.
        for when in [400, 401, 500] {
            let err = authorize_at(&mut w.p, when, &unassigned, &context(&ticket_o))
                .expect_err("a dead session assigns no ticket");
            assert_eq!(err.reason().code(), "no_assignment", "at {when}: {err}");
            let err = authorize_at(&mut w.p, when, &unassigned, &read("operational_record"))
                .expect_err("a dead session assigns no declared scope");
            assert_eq!(err.reason().code(), "no_assignment", "at {when}: {err}");
            let package =
                authorize_at(&mut w.p, when, &on_m, &context(&ticket_m)).unwrap_or_else(|err| {
                    panic!("at {when}, a dead session contradicts nothing: {err}")
                });
            assert_eq!(package.ticket_id(), Some(&ticket_m));
            assert_eq!(module_names(&package), vec!["crates/m"]);
        }
        refused(
            &mut w.p,
            standard(),
            &unassigned,
            &context(&ticket_o),
            "no_assignment",
        );
        granted(&mut w.p, standard(), &on_m, &read("code_map"));

        // A session stays live while any one of its issuances to the
        // identity is, and dies with the last of them.
        let mixed = id("SESSIONMIXED");
        issue_expiring(&mut w.p, &coder, &mixed, 300);
        issue_expiring(&mut w.p, &coder, &mixed, 10_000);
        claim_in_session(&mut w.p, &ticket_m, "crates/m", &mixed);
        let authorization = granted(&mut w.p, standard(), &unassigned, &read("code_map"));
        assert_eq!(authorization.ticket_id(), Some(&ticket_m));
        let err = authorize_at(&mut w.p, 10_000, &unassigned, &read("code_map"))
            .expect_err("every issuance of the session has expired");
        assert_eq!(err.reason().code(), "no_assignment");
    }

    #[test]
    fn ori_t_0038_a_credential_event_ori_broker_cannot_read_back_is_refused() {
        let coder = identity_of(Role::Coder);
        let session = id("SESSIONBAD");
        let issuance = id("ISSUANCE");
        let issued = |id_text: &str, rest: &str| {
            format!(
                "{{\"id\":\"{id_text}\",\"identity_id\":\"{coder}\",\"session_id\":\"{session}\"{rest}}}"
            )
        };
        let good = issuance.as_str();
        let revoked = |rest: &str| format!("{{\"session_id\":\"{session}\"{rest}}}");
        let cases: Vec<(&str, &str, String)> = vec![
            (
                "no expires_at",
                "credential.issued",
                issued(good, ",\"scope\":\"branch\",\"issued_at\":100"),
            ),
            (
                "a text expires_at",
                "credential.issued",
                issued(
                    good,
                    ",\"scope\":\"branch\",\"issued_at\":100,\"expires_at\":\"never\"",
                ),
            ),
            (
                "a fractional expires_at",
                "credential.issued",
                issued(
                    good,
                    ",\"scope\":\"branch\",\"issued_at\":100,\"expires_at\":400.5",
                ),
            ),
            (
                "no issued_at",
                "credential.issued",
                issued(good, ",\"scope\":\"branch\",\"expires_at\":null"),
            ),
            (
                "no scope",
                "credential.issued",
                issued(good, ",\"issued_at\":100,\"expires_at\":null"),
            ),
            (
                "a blank scope",
                "credential.issued",
                issued(
                    good,
                    ",\"scope\":\"  \",\"issued_at\":100,\"expires_at\":null",
                ),
            ),
            (
                "an issuance id that is not an Id",
                "credential.issued",
                issued(
                    "not-an-id",
                    ",\"scope\":\"branch\",\"issued_at\":100,\"expires_at\":null",
                ),
            ),
            (
                "a revocation with no revoked_at",
                "credential.revoked",
                revoked(",\"issuance_ids\":[]"),
            ),
            (
                "a revocation with no issuance_ids",
                "credential.revoked",
                revoked(",\"revoked_at\":300"),
            ),
            (
                "a revocation naming an issuance id that is not an Id",
                "credential.revoked",
                revoked(",\"revoked_at\":300,\"issuance_ids\":[\"not-an-id\"]"),
            ),
        ];
        for (index, (label, kind, payload)) in cases.into_iter().enumerate() {
            let mut w = world(&format!("broker-shape-{index}"));
            let on_m = agent(&coder).with_assigned_ticket(w.ticket_m.clone());
            granted(&mut w.p, standard(), &on_m, &read("code_map"));
            append(&mut w.p, kind, None, payload);
            let before = tip(&mut w.p);
            match standard().authorize(&mut w.p.db, at(500), &on_m, &read("code_map")) {
                Ok(_) => panic!("{label}: granted over a credential event ori-broker refuses"),
                Err(err) => {
                    assert_eq!(
                        err.reason().code(),
                        "assignment_record_malformed",
                        "{label}: {err}"
                    );
                    assert!(err.is_logged(), "{label}");
                }
            }
            assert_eq!(tip(&mut w.p), before + 1, "{label}");
        }
    }

    #[test]
    fn ori_t_0038_credential_payload_shape_and_expiry_rule_match_ori_broker_issuance() {
        let path = repo_file(&["crates", "ori-broker", "src", "issuance.rs"]);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
        // The two payloads, as issuance.rs writes them.
        for written in [
            r#""{{\"id\":\"{}\",\"identity_id\":\"{}\",\"session_id\":\"{}\",\"scope\":\"{}\",\"issued_at\":{},\"expires_at\":{}}}""#,
            r#""{{\"session_id\":\"{}\",\"revoked_at\":{},\"issuance_ids\":[""#,
        ] {
            assert!(
                text.contains(written),
                "issuance.rs no longer writes {written} the way this module reads it"
            );
        }
        // Its reading of them: expires_at is required and may be null, and
        // an issuance is dead from its expires_at on.
        for rule in [
            r#"field_i64_nullable(payload, "expires_at")"#,
            r#"field_i64_required(payload, "issued_at")"#,
            r#"field_i64_required(payload, "revoked_at")"#,
            r#"field_str_array(payload, "issuance_ids")"#,
            "now.millis() >= expires_at.millis()",
            "!self.is_revoked() && !self.is_expired(now)",
        ] {
            assert!(
                text.contains(rule),
                "issuance.rs no longer reads a credential the way this module does: {rule}"
            );
        }
    }

    #[test]
    fn ori_p1_022_a_lock_event_with_no_ticket_column_is_refused_never_skipped() {
        let mut w = world("unattributed-lock");
        let ticket_m = w.ticket_m.clone();
        let hidden = id("THIDDEN");
        file_ticket(&mut w.p, &hidden);
        claim(&mut w.p, &hidden, "crates/m/y");
        let records: Vec<Candidate> = [
            ("RECHC", RecordKind::ClosingReport),
            ("RECHF", RecordKind::Finding),
            ("RECHI", RecordKind::Incident),
        ]
        .into_iter()
        .map(|(label, kind)| Candidate::Record(record(&mut w.p, label, kind, Some(&hidden), None)))
        .collect();
        let coder = principal_for(Role::Coder, &ticket_m);
        // Every claim of THIDDEN so far lies inside crates/m, so its records
        // are M's operational history.
        let package = granted(&mut w.p, standard(), &coder, &context(&ticket_m));
        for candidate in &records {
            assert!(package.admits(candidate), "{candidate:?}");
        }

        // THIDDEN claims a module outside the scope, recorded with no ticket
        // column; the payload names the ticket, and nothing reads a ticket
        // from a payload. ori-store's lock projection refuses the event.
        let payload =
            serde_json::json!({ "module": "crates/secret", "ticket_id": hidden.as_str() })
                .to_string();
        let event = append(&mut w.p, "lock.claimed", None, payload);
        assert!(matches!(
            LockProjection.apply(w.p.db.connection(), &event),
            Err(ProjectionError::MissingTicketId { .. })
        ));
        for request in [
            context(&ticket_m),
            search("q", &[]),
            read("operational_record"),
            read("operational_defect"),
            read("incident"),
            read("code_map"),
            read("canonical_section"),
        ] {
            refused(
                &mut w.p,
                standard(),
                &coder,
                &request,
                "lock_record_unattributed",
            );
        }
        // A reader with no declared scope reads no lock record.
        let lead = principal_for(Role::Lead, &ticket_m);
        let lead_package = granted(&mut w.p, standard(), &lead, &context(&ticket_m));
        assert!(
            records
                .iter()
                .all(|candidate| lead_package.admits(candidate))
        );

        // Every lock event kind, the release included: a release of M with
        // no ticket column, skipped, would leave M's claims live.
        for (index, kind) in ["lock.released", "lock.claimed", "lock.reassigned"]
            .into_iter()
            .enumerate()
        {
            let mut w = world(&format!("unattributed-lock-{index}"));
            let coder = principal_for(Role::Coder, &w.ticket_m);
            granted(&mut w.p, standard(), &coder, &read("code_map"));
            let event = append(&mut w.p, kind, None, "{}".to_owned());
            assert!(
                matches!(
                    LockProjection.apply(w.p.db.connection(), &event),
                    Err(ProjectionError::MissingTicketId { .. })
                ),
                "{kind}"
            );
            refused(
                &mut w.p,
                standard(),
                &coder,
                &read("code_map"),
                "lock_record_unattributed",
            );
        }

        // Claimed under the coder's live session, with no ticket column:
        // which ticket the log ties to the coder cannot be told.
        let mut w = world("unattributed-lock-session");
        let coder = identity_of(Role::Coder);
        let session = id("SESSIONLIVE");
        issue(&mut w.p, &coder, &session);
        let ticket_m = w.ticket_m.clone();
        claim_in_session(&mut w.p, &ticket_m, "crates/m", &session);
        let unassigned = agent(&coder);
        granted(&mut w.p, standard(), &unassigned, &read("code_map"));
        let payload =
            serde_json::json!({ "module": "crates/secret", "session_id": session.as_str() })
                .to_string();
        append(&mut w.p, "lock.claimed", None, payload);
        for principal in [
            agent(&coder),
            agent(&coder).with_assigned_ticket(ticket_m.clone()),
        ] {
            refused(
                &mut w.p,
                standard(),
                &principal,
                &read("code_map"),
                "lock_record_unattributed",
            );
        }
    }

    #[test]
    fn ori_p1_022_a_lock_claim_whose_session_cannot_be_read_is_refused_while_the_identity_holds_a_live_session()
     {
        let coder = identity_of(Role::Coder);
        let live = id("SESSIONLIVE");
        let payloads: [(&str, String); 5] = [
            ("not json", "not json".to_owned()),
            (
                "a session id that is not an Id",
                serde_json::json!({ "module": "crates/other", "session_id": "not-an-id" })
                    .to_string(),
            ),
            (
                "a session id that is not text",
                serde_json::json!({ "module": "crates/other", "session_id": 7 }).to_string(),
            ),
            (
                "no module",
                serde_json::json!({ "session_id": live.as_str() }).to_string(),
            ),
            ("an array", "[]".to_owned()),
        ];
        for (index, (label, payload)) in payloads.into_iter().enumerate() {
            let mut w = world(&format!("claim-session-{index}"));
            let ticket_m = w.ticket_m.clone();
            let ticket_o = w.ticket_o.clone();
            issue(&mut w.p, &coder, &live);
            claim_in_session(&mut w.p, &ticket_m, "crates/m", &live);
            let unassigned = agent(&coder);
            let authorization = granted(&mut w.p, standard(), &unassigned, &read("code_map"));
            assert_eq!(authorization.ticket_id(), Some(&ticket_m), "{label}");
            // Ticket O claimed under a session that cannot be read: it may be
            // the coder's, making its assignment ambiguous.
            append(&mut w.p, "lock.claimed", Some(&ticket_o), payload);
            for principal in [
                agent(&coder),
                agent(&coder).with_assigned_ticket(ticket_m.clone()),
            ] {
                match standard().authorize(&mut w.p.db, at(500), &principal, &read("code_map")) {
                    Ok(_) => panic!("{label}: granted over a claim whose session is unknown"),
                    Err(err) => assert_eq!(
                        err.reason().code(),
                        "assignment_record_malformed",
                        "{label}: {err}"
                    ),
                }
            }
        }

        // With no live session, no claim can tie a ticket to the identity,
        // so another ticket's unreadable claim changes nothing.
        let mut w = world("claim-session-none");
        let ticket_o = w.ticket_o.clone();
        append(
            &mut w.p,
            "lock.claimed",
            Some(&ticket_o),
            "not json".to_owned(),
        );
        let on_m = principal_for(Role::Coder, &w.ticket_m);
        let authorization = granted(&mut w.p, standard(), &on_m, &read("code_map"));
        assert_eq!(module_names(&authorization), vec!["crates/m"]);
    }

    // -------------------------------------------------------------------
    // ORI-P1-022: the coder's package, filtered.
    // -------------------------------------------------------------------

    #[test]
    fn ori_p1_022_coder_context_package_holds_only_canonical_organizational_operational_for_m_and_code_map_of_m()
     {
        let mut w = world("p1-022");
        let mut other = product("p1-022-other", "PRODUCTB");
        let a = w.p.id.clone();
        let b = other.id.clone();

        // Ticket O is released; E claims a sibling that shares M's prefix; W
        // claims M's parent; N claims nothing.
        let ticket_o = w.ticket_o.clone();
        release(&mut w.p, &ticket_o);
        let ticket_e = id("TICKETE");
        let ticket_w = id("TICKETW");
        let ticket_n = id("TICKETN");
        for (ticket, module) in [
            (&ticket_e, Some("crates/m-evil")),
            (&ticket_w, Some("crates")),
            (&ticket_n, None),
        ] {
            file_ticket(&mut w.p, ticket);
            if let Some(module) = module {
                claim(&mut w.p, ticket, module);
            }
        }
        let ticket_m = w.ticket_m.clone();
        let finding_m = record(
            &mut w.p,
            "RECFM",
            RecordKind::Finding,
            Some(&ticket_m),
            Some("EVFM"),
        );
        let incident_m = record(
            &mut w.p,
            "RECIM",
            RecordKind::Incident,
            Some(&ticket_m),
            None,
        );
        let closing_o = record(
            &mut w.p,
            "RECCO",
            RecordKind::ClosingReport,
            Some(&ticket_o),
            None,
        );
        let blocked_none = record(&mut w.p, "RECBN", RecordKind::BlockedReport, None, None);
        let closing_e = record(
            &mut w.p,
            "RECCE",
            RecordKind::ClosingReport,
            Some(&ticket_e),
            None,
        );
        let escalation_w = record(
            &mut w.p,
            "RECEW",
            RecordKind::EscalationDecision,
            Some(&ticket_w),
            None,
        );
        let postmortem_n = record(
            &mut w.p,
            "RECPN",
            RecordKind::PostMortem,
            Some(&ticket_n),
            None,
        );
        // Another product's finding, on a ticket with the same identifier.
        let finding_b = record(
            &mut other,
            "RECFB",
            RecordKind::Finding,
            Some(&ticket_m),
            None,
        );
        let blob = barrier::read_evidence(&mut w.p.db)
            .expect("evidence reads back")
            .pop()
            .expect("the finding's field stored one blob");

        // (label, candidate, admitted for the coder, admitted for the lead)
        let cases: Vec<(&str, Candidate, bool, bool)> = vec![
            (
                "section",
                doc(&a, DocumentKind::Section, &spec_doc("LLD.md", "crates")),
                true,
                true,
            ),
            (
                "adr",
                doc(
                    &a,
                    DocumentKind::Adr,
                    &spec_doc("adr/ADR-0001-stack.md", "decision"),
                ),
                true,
                true,
            ),
            (
                "criterion",
                doc(
                    &a,
                    DocumentKind::Criterion,
                    &spec_doc("criteria/phase-1.md", "ori-p1-022"),
                ),
                true,
                true,
            ),
            (
                "brief",
                doc(
                    &a,
                    DocumentKind::Section,
                    &spec_doc("PROJECT_BRIEF.md", "purpose"),
                ),
                true,
                true,
            ),
            (
                "runbook",
                doc(
                    &a,
                    DocumentKind::Section,
                    &spec_doc("runbooks/restore.md", "steps"),
                ),
                true,
                true,
            ),
            (
                "section outside spec",
                doc(
                    &a,
                    DocumentKind::Section,
                    &format!("notes/{}#intro", "readme.md"),
                ),
                false,
                false,
            ),
            (
                "section escaping spec",
                doc(
                    &a,
                    DocumentKind::Section,
                    &format!("{}/../{}", "spec", "secret.md"),
                ),
                false,
                false,
            ),
            (
                "module hit in M",
                doc(&a, DocumentKind::Module, "crates/m/src/lib.rs"),
                true,
                true,
            ),
            (
                "module hit sharing M's prefix",
                doc(&a, DocumentKind::Module, "crates/m-evil/src/lib.rs"),
                false,
                true,
            ),
            (
                "module hit elsewhere",
                doc(&a, DocumentKind::Module, "crates/other/src/lib.rs"),
                false,
                true,
            ),
            (
                "module hit escaping M",
                doc(&a, DocumentKind::Module, "crates/m/../other/src/lib.rs"),
                false,
                true,
            ),
            (
                "module hit in another case",
                doc(&a, DocumentKind::Module, "Crates/m/src/lib.rs"),
                false,
                true,
            ),
            (
                "code map in M",
                code_module(&a, "crates/m/src/a.rs"),
                true,
                true,
            ),
            (
                "code map elsewhere",
                code_module(&a, "crates/other/src/b.rs"),
                false,
                true,
            ),
            (
                "code map of another product",
                code_module(&b, "crates/m/src/a.rs"),
                false,
                false,
            ),
            ("finding for M", Candidate::Record(finding_m), true, true),
            ("incident for M", Candidate::Record(incident_m), true, true),
            (
                "closing report of a released ticket elsewhere",
                Candidate::Record(closing_o),
                false,
                true,
            ),
            (
                "blocked report with no ticket",
                Candidate::Record(blocked_none),
                false,
                true,
            ),
            (
                "closing report sharing M's prefix",
                Candidate::Record(closing_e),
                false,
                true,
            ),
            (
                "escalation on M's parent",
                Candidate::Record(escalation_w),
                false,
                true,
            ),
            (
                "post-mortem of an unclaimed ticket",
                Candidate::Record(postmortem_n),
                false,
                true,
            ),
            (
                "another product's finding",
                Candidate::Record(finding_b),
                false,
                false,
            ),
            ("evidence blob", Candidate::Evidence(blob), false, false),
            (
                "organizational",
                unproduced(&a, Source::Organizational),
                true,
                true,
            ),
            (
                "infrastructure adr",
                unproduced(&a, Source::InfrastructureAdr),
                true,
                true,
            ),
            (
                "merged diff",
                unproduced(&a, Source::MergedDiff),
                false,
                true,
            ),
            (
                "analytics summary",
                unproduced(&a, Source::AnalyticsSummary),
                false,
                true,
            ),
            ("code read", unproduced(&a, Source::CodeRead), false, true),
            (
                "a typed source relabelled",
                unproduced(&a, Source::OperationalRecord),
                false,
                false,
            ),
            (
                "evidence relabelled",
                unproduced(&a, Source::RawEvidence),
                false,
                false,
            ),
            (
                "another product's organizational",
                unproduced(&b, Source::Organizational),
                false,
                false,
            ),
            (
                "another product's section",
                doc(&b, DocumentKind::Section, &spec_doc("LLD.md", "x")),
                false,
                false,
            ),
        ];

        let coder = principal_for(Role::Coder, &ticket_m);
        let package = granted(&mut w.p, standard(), &coder, &context(&ticket_m));
        assert_eq!(package.ticket_id(), Some(&ticket_m));
        assert_eq!(
            package
                .declared_modules()
                .iter()
                .map(ModulePath::as_str)
                .collect::<Vec<_>>(),
            vec!["crates/m"]
        );
        let lead = principal_for(Role::Lead, &ticket_m);
        let lead_package = granted(&mut w.p, standard(), &lead, &context(&ticket_m));

        for (label, candidate, for_coder, for_lead) in &cases {
            assert_eq!(package.admits(candidate), *for_coder, "coder: {label}");
            assert_eq!(lead_package.admits(candidate), *for_lead, "lead: {label}");
        }

        let all: Vec<Candidate> = cases.iter().map(|case| case.1.clone()).collect();
        let expected: Vec<Candidate> = cases
            .iter()
            .filter(|case| case.2)
            .map(|case| case.1.clone())
            .collect();
        let kept = package.filter(all);
        assert_eq!(kept, expected);
        assert!(
            kept.iter()
                .all(|candidate| !matches!(candidate, Candidate::Evidence(_))),
            "no evidence blob in a coder's package"
        );
        assert!(
            kept.iter()
                .all(|candidate| candidate.product_id() == Some(&a)),
            "nothing from another product"
        );
    }

    #[test]
    fn ori_p1_022_records_are_inside_the_declared_scope_only_when_every_claim_of_their_ticket_is() {
        let mut w = world("containment");
        let ticket_m = w.ticket_m.clone();
        // M claims crates/m again after every other ticket below has made
        // its claims: TICKETEXACT's claim of crates/m, made while M held it,
        // takes the row over, as the lock projection keys it (see
        // tests::ori_p1_022_the_lock_fold_keeps_the_rows_the_lock_projection_keeps).
        release(&mut w.p, &ticket_m);
        // (ticket label, its claims, released afterwards, inside `crates/m`)
        let tickets: [(&str, &[&str], bool, bool); 8] = [
            ("TICKETINSIDE", &["crates/m/src"], true, true),
            ("TICKETEXACT", &["crates/m"], true, true),
            (
                "TICKETTWOIN",
                &["crates/m/src", "crates/m/tests"],
                false,
                true,
            ),
            ("TICKETPARENT", &["crates"], true, false),
            ("TICKETBOTH", &["crates/m/src", "crates/other"], true, false),
            ("TICKETSIBLING", &["crates/m-evil"], true, false),
            (
                "TICKETBADCLAIM",
                &["crates/m/src", "crates/m/../x"],
                true,
                false,
            ),
            ("TICKETNOCLAIM", &[], false, false),
        ];
        let mut cases: Vec<(String, Candidate, bool)> = Vec::new();
        for (label, claims, released, inside) in tickets {
            let ticket = id(label);
            file_ticket(&mut w.p, &ticket);
            for module in claims {
                claim(&mut w.p, &ticket, module);
            }
            if released {
                release(&mut w.p, &ticket);
            }
            let rec = record(
                &mut w.p,
                &format!("REC{label}"),
                RecordKind::ClosingReport,
                Some(&ticket),
                None,
            );
            cases.push((label.to_owned(), Candidate::Record(rec), inside));
        }
        claim(&mut w.p, &ticket_m, "crates/m");
        let own = record(
            &mut w.p,
            "RECOWN",
            RecordKind::Finding,
            Some(&ticket_m),
            None,
        );
        cases.push(("own ticket".to_owned(), Candidate::Record(own), true));

        let coder = principal_for(Role::Coder, &ticket_m);
        let check = |w: &mut World, cases: &[(String, Candidate, bool)], moment: &str| {
            for request in [
                context(&ticket_m),
                search("q", &[]),
                search("q", &["operational_record", "operational_defect"]),
                read("operational_record"),
                read("operational_defect"),
            ] {
                let authorization = granted(&mut w.p, standard(), &coder, &request);
                for (label, candidate, inside) in cases {
                    let source_asked = candidate
                        .classify()
                        .is_some_and(|source| authorization.allows(source));
                    assert_eq!(
                        authorization.admits(candidate),
                        *inside && source_asked,
                        "{moment}, {request:?}: {label}"
                    );
                }
            }
        };
        check(&mut w, &cases, "M on crates/m");

        // M moves to crates/q: its own records stay in, and the tickets that
        // worked only inside crates/m are no longer inside its scope.
        release(&mut w.p, &ticket_m);
        claim(&mut w.p, &ticket_m, "crates/q");
        let moved: Vec<(String, Candidate, bool)> = cases
            .into_iter()
            .map(|(label, candidate, _)| {
                let inside = label == "own ticket";
                (label, candidate, inside)
            })
            .collect();
        check(&mut w, &moved, "M on crates/q");
    }

    /// One gathered result, labeled by the test itself rather than by
    /// `Candidate::classify`, so the filter is checked against an oracle
    /// that does not share its code.
    struct Labeled {
        label: &'static str,
        candidate: Candidate,
        /// From product A, the product every authorization is granted in.
        home: bool,
        /// Its source, `None` for what nothing may classify.
        source: Option<Source>,
        /// Inside the coder's declared scope, `crates/m`.
        in_m: bool,
    }

    /// Whether `item` must be admitted to a reader holding `row` who asked
    /// for `asked`.
    fn oracle_admits(
        row: &BTreeMap<Source, Filter>,
        asked: &BTreeSet<Source>,
        item: &Labeled,
    ) -> bool {
        let Some(source) = item.source else {
            return false;
        };
        if !item.home || !asked.contains(&source) {
            return false;
        }
        match row.get(&source) {
            None => false,
            Some(Filter::DeclaredScope) => item.in_m,
            Some(Filter::Whole | Filter::MigrationOnly) => true,
        }
    }

    #[test]
    fn ori_p1_022_every_query_kind_filters_its_results_by_the_rule_its_request_was_granted_under() {
        let mut w = world("oracle");
        let mut other = product("oracle-other", "PRODUCTB");
        let (a, b) = (w.p.id.clone(), other.id.clone());
        let ticket_m = w.ticket_m.clone();
        let ticket_o = w.ticket_o.clone();
        let ticket_i = id("TICKETI");
        let ticket_w = id("TICKETW");
        file_ticket(&mut w.p, &ticket_i);
        claim(&mut w.p, &ticket_i, "crates/m/src");
        release(&mut w.p, &ticket_i);
        file_ticket(&mut w.p, &ticket_w);
        claim(&mut w.p, &ticket_w, "crates");

        let rec = |p: &mut Product, label: &str, kind: RecordKind, ticket: Option<&Id>| {
            Candidate::Record(record(p, label, kind, ticket, None))
        };
        let closing_m = rec(&mut w.p, "OCM", RecordKind::ClosingReport, Some(&ticket_m));
        let finding_m = rec(&mut w.p, "OFM", RecordKind::Finding, Some(&ticket_m));
        let incident_i = rec(&mut w.p, "OII", RecordKind::Incident, Some(&ticket_i));
        let blocked_i = rec(&mut w.p, "OBI", RecordKind::BlockedReport, Some(&ticket_i));
        let closing_o = rec(&mut w.p, "OCO", RecordKind::ClosingReport, Some(&ticket_o));
        let finding_o = rec(&mut w.p, "OFO", RecordKind::Finding, Some(&ticket_o));
        let postmortem_o = rec(&mut w.p, "OPO", RecordKind::PostMortem, Some(&ticket_o));
        let escalation_w = rec(
            &mut w.p,
            "OEW",
            RecordKind::EscalationDecision,
            Some(&ticket_w),
        );
        let finding_w = rec(&mut w.p, "OFW", RecordKind::Finding, Some(&ticket_w));
        let blocked_none = rec(&mut w.p, "OBN", RecordKind::BlockedReport, None);
        let finding_b = rec(&mut other, "OFB", RecordKind::Finding, Some(&ticket_m));
        let submitted = record(
            &mut w.p,
            "OEVIDENCE",
            RecordKind::Finding,
            Some(&ticket_m),
            Some("OEVBLOB"),
        );
        let blob = barrier::read_evidence(&mut w.p.db)
            .expect("evidence reads back")
            .into_iter()
            .find(|blob| blob.record_id() == submitted.id())
            .expect("the finding's field stored one blob");

        let item = |label: &'static str,
                    candidate: Candidate,
                    home: bool,
                    source: Option<Source>,
                    in_m: bool| Labeled {
            label,
            candidate,
            home,
            source,
            in_m,
        };
        use Source as S;
        let items: Vec<Labeled> = vec![
            item(
                "section",
                doc(&a, DocumentKind::Section, &spec_doc("LLD.md", "x")),
                true,
                Some(S::CanonicalSection),
                false,
            ),
            item(
                "adr",
                doc(
                    &a,
                    DocumentKind::Adr,
                    &spec_doc("adr/ADR-0001-stack.md", "d"),
                ),
                true,
                Some(S::Adr),
                false,
            ),
            item(
                "criterion",
                doc(
                    &a,
                    DocumentKind::Criterion,
                    &spec_doc("criteria/phase-1.md", "c"),
                ),
                true,
                Some(S::Criterion),
                false,
            ),
            item(
                "brief",
                doc(
                    &a,
                    DocumentKind::Section,
                    &spec_doc("PROJECT_BRIEF.md", "p"),
                ),
                true,
                Some(S::Brief),
                false,
            ),
            item(
                "runbook",
                doc(
                    &a,
                    DocumentKind::Section,
                    &spec_doc("runbooks/restore.md", "s"),
                ),
                true,
                Some(S::Runbook),
                false,
            ),
            item(
                "section outside spec",
                doc(
                    &a,
                    DocumentKind::Section,
                    &format!("notes/{}#i", "readme.md"),
                ),
                true,
                None,
                false,
            ),
            item(
                "module hit in M",
                doc(&a, DocumentKind::Module, "crates/m/src/lib.rs"),
                true,
                Some(S::CodeMap),
                true,
            ),
            item(
                "module hit sharing M's prefix",
                doc(&a, DocumentKind::Module, "crates/m-evil/src/lib.rs"),
                true,
                Some(S::CodeMap),
                false,
            ),
            item(
                "module hit elsewhere",
                doc(&a, DocumentKind::Module, "crates/other/src/lib.rs"),
                true,
                Some(S::CodeMap),
                false,
            ),
            item(
                "code map in M",
                code_module(&a, "crates/m/src/a.rs"),
                true,
                Some(S::CodeMap),
                true,
            ),
            item(
                "code map elsewhere",
                code_module(&a, "crates/other/src/b.rs"),
                true,
                Some(S::CodeMap),
                false,
            ),
            item(
                "closing report for M",
                closing_m,
                true,
                Some(S::OperationalRecord),
                true,
            ),
            item(
                "finding for M",
                finding_m,
                true,
                Some(S::OperationalDefect),
                true,
            ),
            item(
                "incident inside M",
                incident_i,
                true,
                Some(S::Incident),
                true,
            ),
            item(
                "blocked report inside M",
                blocked_i,
                true,
                Some(S::OperationalRecord),
                true,
            ),
            item(
                "closing report elsewhere",
                closing_o,
                true,
                Some(S::OperationalRecord),
                false,
            ),
            item(
                "finding elsewhere",
                finding_o,
                true,
                Some(S::OperationalDefect),
                false,
            ),
            item(
                "post-mortem elsewhere",
                postmortem_o,
                true,
                Some(S::Incident),
                false,
            ),
            item(
                "escalation on M's parent",
                escalation_w,
                true,
                Some(S::OperationalRecord),
                false,
            ),
            item(
                "finding on M's parent",
                finding_w,
                true,
                Some(S::OperationalDefect),
                false,
            ),
            item(
                "blocked report with no ticket",
                blocked_none,
                true,
                Some(S::OperationalRecord),
                false,
            ),
            item(
                "evidence blob",
                Candidate::Evidence(blob),
                true,
                Some(S::RawEvidence),
                false,
            ),
            item(
                "organizational",
                unproduced(&a, S::Organizational),
                true,
                Some(S::Organizational),
                false,
            ),
            item(
                "infrastructure adr",
                unproduced(&a, S::InfrastructureAdr),
                true,
                Some(S::InfrastructureAdr),
                false,
            ),
            item(
                "merged diff",
                unproduced(&a, S::MergedDiff),
                true,
                Some(S::MergedDiff),
                false,
            ),
            item(
                "analytics summary",
                unproduced(&a, S::AnalyticsSummary),
                true,
                Some(S::AnalyticsSummary),
                false,
            ),
            item(
                "code read",
                unproduced(&a, S::CodeRead),
                true,
                Some(S::CodeRead),
                false,
            ),
            item(
                "a typed source relabelled",
                unproduced(&a, S::Incident),
                true,
                None,
                true,
            ),
            item(
                "another product's finding",
                finding_b,
                false,
                Some(S::OperationalDefect),
                true,
            ),
            item(
                "another product's code map",
                code_module(&b, "crates/m/src/a.rs"),
                false,
                Some(S::CodeMap),
                true,
            ),
            item(
                "another product's section",
                doc(&b, DocumentKind::Section, &spec_doc("LLD.md", "x")),
                false,
                Some(S::CanonicalSection),
                false,
            ),
            item(
                "another product's organizational",
                unproduced(&b, S::Organizational),
                false,
                Some(S::Organizational),
                false,
            ),
        ];
        let all: Vec<Candidate> = items.iter().map(|item| item.candidate.clone()).collect();

        let readers: Vec<Option<Role>> = Role::ALL
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
            .collect();
        let mut checked = 0usize;
        for stage in [ProductStage::Standard, ProductStage::Migration] {
            let enforcer = ScopeEnforcer::new(stage);
            for reader in &readers {
                let (principal, row) = match reader {
                    Some(role) => (principal_for(*role, &ticket_m), row_map(*role, stage)),
                    None => (operator(), operator_row()),
                };
                let whole: BTreeSet<Source> = row.keys().copied().collect();
                let names: Vec<&str> = row.keys().map(|source| source.as_str()).collect();
                let mut shapes: Vec<(String, MemoryRequest, BTreeSet<Source>)> = vec![
                    (
                        "search naming nothing".to_owned(),
                        search("q", &[]),
                        whole.clone(),
                    ),
                    (
                        "search naming the row".to_owned(),
                        search("q", &names),
                        whole.clone(),
                    ),
                ];
                if *reader != Some(Role::ProductSignal) {
                    shapes.push(("context".to_owned(), context(&ticket_m), whole.clone()));
                }
                for source in row.keys() {
                    let one: BTreeSet<Source> = [*source].into_iter().collect();
                    shapes.push((format!("read {source}"), read(source.as_str()), one.clone()));
                    shapes.push((
                        format!("search {source}"),
                        search("q", &[source.as_str()]),
                        one,
                    ));
                }
                for (shape, request, asked) in shapes {
                    let authorization = granted(&mut w.p, enforcer, &principal, &request);
                    let context = format!("{reader:?} in {stage:?}, {shape}");
                    for item in &items {
                        assert_eq!(
                            authorization.admits(&item.candidate),
                            oracle_admits(&row, &asked, item),
                            "{context}: {}",
                            item.label
                        );
                    }
                    let expected: Vec<&Labeled> = items
                        .iter()
                        .filter(|item| oracle_admits(&row, &asked, item))
                        .collect();
                    assert_eq!(
                        authorization.filter(all.clone()),
                        expected
                            .iter()
                            .map(|item| item.candidate.clone())
                            .collect::<Vec<_>>(),
                        "{context}"
                    );
                    // Every source asked for is reachable through the filter:
                    // it admits something, not nothing.
                    for source in &asked {
                        assert!(
                            expected.iter().any(|item| item.source == Some(*source)),
                            "{context}: nothing of {source} is admitted"
                        );
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 300, "only {checked} authorizations were checked");
    }

    // -------------------------------------------------------------------
    // ORI-P1-036: two products.
    // -------------------------------------------------------------------

    #[test]
    fn ori_p1_036_a_request_against_another_product_is_refused_and_logged_there() {
        let mut a = product("p1-036-a", "PRODUCTA");
        let mut b = product("p1-036-b", "PRODUCTB");
        let lead = id("LEADOFA");
        register(&mut a, &lead, Role::Lead);
        let principal = agent(&lead);

        granted(&mut a, standard(), &principal, &read("adr"));
        let before_a = tip(&mut a);
        refused(
            &mut b,
            standard(),
            &principal,
            &read("adr"),
            "unregistered_identity",
        );
        assert_eq!(
            tip(&mut a),
            before_a,
            "nothing recorded in the other product"
        );

        // Saying it is for product A does not help in B.
        let request = MemoryRequest {
            claims: Claims {
                product_id: Some(a.id.clone()),
                ..Claims::default()
            },
            ..read("adr")
        };
        refused(
            &mut b,
            standard(),
            &principal,
            &request,
            "claim_contradicted",
        );

        // A ticket of product A, asked for in product A's own log by the
        // same lead, is fine; asked for in B by B's own lead, it is not there.
        let ticket_a = id("TICKETA");
        file_ticket(&mut a, &ticket_a);
        granted(&mut a, standard(), &principal, &context(&ticket_a));
        let lead_b = id("LEADOFB");
        register(&mut b, &lead_b, Role::Lead);
        refused(
            &mut b,
            standard(),
            &agent(&lead_b),
            &context(&ticket_a),
            "ticket_not_in_product",
        );
    }

    #[test]
    fn ori_p1_036_search_results_from_another_product_are_filtered_out() {
        let mut a = product("p1-036-results-a", "PRODUCTA");
        let mut b = product("p1-036-results-b", "PRODUCTB");
        let lead_a = id("LEADOFA");
        let lead_b = id("LEADOFB");
        register(&mut a, &lead_a, Role::Lead);
        register(&mut b, &lead_b, Role::Lead);
        let ticket = id("TICKETSHARED");
        file_ticket(&mut a, &ticket);
        file_ticket(&mut b, &ticket);
        let record_a = record(&mut a, "RECA", RecordKind::Finding, Some(&ticket), None);
        let record_b = record(&mut b, "RECB", RecordKind::Finding, Some(&ticket), None);
        let (ia, ib) = (a.id.clone(), b.id.clone());

        let mixed = vec![
            Candidate::Record(record_a.clone()),
            Candidate::Record(record_b.clone()),
            doc(
                &ia,
                DocumentKind::Adr,
                &spec_doc("adr/ADR-0003-full-text-index.md", "d"),
            ),
            doc(
                &ib,
                DocumentKind::Adr,
                &spec_doc("adr/ADR-0003-full-text-index.md", "d"),
            ),
            code_module(&ia, "crates/x/src/lib.rs"),
            code_module(&ib, "crates/x/src/lib.rs"),
            unproduced(&ia, Source::Organizational),
            unproduced(&ib, Source::Organizational),
        ];

        let in_b = granted(&mut b, standard(), &agent(&lead_b), &search("finding", &[]));
        let kept = in_b.filter(mixed.clone());
        assert_eq!(kept.len(), 4, "B's search kept {kept:?}");
        assert!(
            kept.iter()
                .all(|candidate| candidate.product_id() == Some(&ib))
        );
        assert!(kept.contains(&Candidate::Record(record_b)));

        let in_a = granted(&mut a, standard(), &agent(&lead_a), &search("finding", &[]));
        let kept = in_a.filter(mixed);
        assert_eq!(kept.len(), 4, "A's search kept {kept:?}");
        assert!(
            kept.iter()
                .all(|candidate| candidate.product_id() == Some(&ia))
        );
        assert!(kept.contains(&Candidate::Record(record_a)));
    }

    // -------------------------------------------------------------------
    // What the result filter decides on, and what it can check.
    // -------------------------------------------------------------------

    /// A code map module at `path` with every other field empty.
    fn module_value(path: &str) -> code_map::Module {
        code_map::Module {
            path: path.to_owned(),
            language: Language::Rust,
            parsed_with_errors: false,
            interfaces: Vec::new(),
            dependency_edges: Vec::new(),
            dependency_edges_truncated: false,
            entry_points: Vec::new(),
            covering_tests: Vec::new(),
            spec_sections: Vec::new(),
            spec_sections_truncated: false,
        }
    }

    /// The module doc comment's table of what the result filter decides on:
    /// (variant, field, what it decides, whether this module checks it), one
    /// entry per field a cell names.
    fn filter_field_rows() -> Vec<(String, String, String, String)> {
        let mut rows = Vec::new();
        for line in include_str!("scope.rs")
            .lines()
            .filter(|line| line.starts_with("//! | Candidate::"))
        {
            let cells: Vec<&str> = line
                .trim_start_matches("//!")
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            assert_eq!(cells.len(), 4, "{line}");
            for field in cells[1].split(", ") {
                rows.push((
                    cells[0].trim_matches('`').to_owned(),
                    field.trim_matches('`').to_owned(),
                    cells[2].to_owned(),
                    cells[3].to_owned(),
                ));
            }
        }
        rows
    }

    #[test]
    fn ori_t_0038_module_doc_names_every_candidate_field_the_result_filter_decides_on() {
        let mut w = world("filter-fields");
        let other = product("filter-fields-other", "PRODUCTB");
        let (a, b) = (w.p.id.clone(), other.id.clone());
        let ticket_m = w.ticket_m.clone();
        // Every reader, each under the widest request it may make.
        let mut authorizations = Vec::new();
        for role in Role::ALL {
            let principal = principal_for(*role, &ticket_m);
            authorizations.push(granted(
                &mut w.p,
                migration(),
                &principal,
                &search("q", &[]),
            ));
        }
        authorizations.push(granted(
            &mut w.p,
            migration(),
            &operator(),
            &search("q", &[]),
        ));

        let with_hit = |product_id: &Id, edit: &dyn Fn(&mut SearchHit)| {
            let mut hit = SearchHit {
                path: "crates/m/src/lib.rs".to_owned(),
                kind: DocumentKind::Module,
                title: "a title".to_owned(),
                score: -1.0,
            };
            edit(&mut hit);
            Candidate::Document {
                product_id: product_id.clone(),
                hit,
            }
        };
        let with_module = |product_id: &Id, edit: &dyn Fn(&mut code_map::Module)| {
            let mut module = module_value("crates/m/src/a.rs");
            edit(&mut module);
            Candidate::Module {
                product_id: product_id.clone(),
                module,
            }
        };
        let gathered_hit = with_hit(&a, &|_| {});
        let gathered_module = with_module(&a, &|_| {});
        let gathered_unproduced = unproduced(&a, Source::Organizational);
        // (variant, field, the candidate as gathered, the same candidate
        // with that one field set otherwise). `module.dependency_edges` is
        // not here: an edge's target has no constructor outside code_map.rs.
        let document = "Candidate::Document";
        let module = "Candidate::Module";
        let unproduced_variant = "Candidate::Unproduced";
        let mutations: Vec<(&str, &str, &Candidate, Candidate)> = vec![
            (document, "product_id", &gathered_hit, with_hit(&b, &|_| {})),
            (
                document,
                "hit.kind",
                &gathered_hit,
                with_hit(&a, &|hit| hit.kind = DocumentKind::Section),
            ),
            (
                document,
                "hit.path",
                &gathered_hit,
                with_hit(&a, &|hit| hit.path = "crates/other/src/lib.rs".to_owned()),
            ),
            (
                document,
                "hit.title",
                &gathered_hit,
                with_hit(&a, &|hit| hit.title = "another title".to_owned()),
            ),
            (
                document,
                "hit.score",
                &gathered_hit,
                with_hit(&a, &|hit| hit.score = 7.5),
            ),
            (
                module,
                "product_id",
                &gathered_module,
                with_module(&b, &|_| {}),
            ),
            (
                module,
                "module.path",
                &gathered_module,
                with_module(&a, &|m| m.path = "crates/other/src/a.rs".to_owned()),
            ),
            (
                module,
                "module.language",
                &gathered_module,
                with_module(&a, &|m| m.language = Language::Go),
            ),
            (
                module,
                "module.parsed_with_errors",
                &gathered_module,
                with_module(&a, &|m| m.parsed_with_errors = true),
            ),
            (
                module,
                "module.interfaces",
                &gathered_module,
                with_module(&a, &|m| {
                    m.interfaces.push(Interface {
                        name: "secret".to_owned(),
                        kind: InterfaceKind::Function,
                        line: 1,
                    });
                }),
            ),
            (
                module,
                "module.dependency_edges_truncated",
                &gathered_module,
                with_module(&a, &|m| m.dependency_edges_truncated = true),
            ),
            (
                module,
                "module.entry_points",
                &gathered_module,
                with_module(&a, &|m| {
                    m.entry_points.push(EntryPoint {
                        name: "main".to_owned(),
                        line: 1,
                    });
                }),
            ),
            (
                module,
                "module.covering_tests",
                &gathered_module,
                with_module(&a, &|m| {
                    m.covering_tests.push("crates/other/tests/t.rs".to_owned());
                }),
            ),
            (
                module,
                "module.spec_sections",
                &gathered_module,
                with_module(&a, &|m| {
                    m.spec_sections.push(SpecCitation {
                        doc_index: 0,
                        heading_index: None,
                    });
                }),
            ),
            (
                module,
                "module.spec_sections_truncated",
                &gathered_module,
                with_module(&a, &|m| m.spec_sections_truncated = true),
            ),
            (
                unproduced_variant,
                "product_id",
                &gathered_unproduced,
                unproduced(&b, Source::Organizational),
            ),
            (
                unproduced_variant,
                "source",
                &gathered_unproduced,
                unproduced(&a, Source::MergedDiff),
            ),
            (
                unproduced_variant,
                "locator",
                &gathered_unproduced,
                Candidate::Unproduced {
                    product_id: a.clone(),
                    source: Source::Organizational,
                    locator: "elsewhere".to_owned(),
                },
            ),
        ];

        let rows = filter_field_rows();
        assert!(
            rows.len() > 3,
            "the module doc comment carries no table of what the result filter decides on"
        );
        let row_for = |variant: &str, field: &str| {
            rows.iter()
                .find(|row| row.0 == variant && row.1 == field)
                .or_else(|| {
                    rows.iter()
                        .find(|row| row.0 == variant && row.1.starts_with("every other field"))
                })
        };
        for (variant, field, gathered, changed) in &mutations {
            assert!(
                authorizations.iter().any(|auth| auth.admits(gathered)),
                "{variant} as gathered is admitted to no reader, so nothing below is tested"
            );
            let decides = authorizations
                .iter()
                .any(|auth| auth.admits(gathered) != auth.admits(changed));
            let row = row_for(variant, field).unwrap_or_else(|| {
                panic!("the module doc comment does not say what {variant}'s {field} decides")
            });
            assert_eq!(
                !row.2.starts_with("nothing"),
                decides,
                "{variant}'s {field}: the module doc comment says it decides {:?}",
                row.2
            );
            assert_eq!(
                row.3, "no: the retrieval's word",
                "{variant}'s {field} is set by the retrieval and checked by nothing here"
            );
        }
        // Every field the table says decides something is exercised above,
        // and the two variants this module can rely on are the ones only
        // barrier.rs builds.
        for row in &rows {
            match row.0.as_str() {
                "Candidate::Record" | "Candidate::Evidence" => {
                    assert!(row.3.starts_with("yes"), "{row:?}");
                }
                _ => {
                    if !row.2.starts_with("nothing") {
                        assert!(
                            mutations
                                .iter()
                                .any(|(variant, field, _, _)| *variant == row.0 && *field == row.1),
                            "{row:?} is not exercised"
                        );
                    }
                }
            }
        }
        for variant in ["Candidate::Record", "Candidate::Evidence"] {
            assert!(rows.iter().any(|row| row.0 == variant), "{variant}");
        }
    }

    #[test]
    fn ori_p1_022_the_result_filter_returns_an_admitted_value_bound_to_its_product_and_reader() {
        let mut w = world("admitted");
        let a = w.p.id.clone();
        let b = id("PRODUCTB");
        let ticket_m = w.ticket_m.clone();
        let coder = principal_for(Role::Coder, &ticket_m);
        let package = granted(&mut w.p, standard(), &coder, &context(&ticket_m));
        let gathered = vec![
            code_module(&a, "crates/m/src/a.rs"),
            code_module(&a, "crates/other/src/b.rs"),
            code_module(&b, "crates/m/src/a.rs"),
            doc(&a, DocumentKind::Section, &spec_doc("LLD.md", "x")),
        ];
        let admitted = package.filter(gathered.clone());
        assert_eq!(admitted.product_id(), &a);
        assert_eq!(admitted.reader(), package.reader());
        assert_eq!(
            admitted.candidates(),
            &[gathered[0].clone(), gathered[3].clone()]
        );
        // What was admitted is admitted again, whole, and nothing is added.
        assert_eq!(package.filter(admitted.candidates().to_vec()), admitted);

        // The same results filtered for another reader are bound to it.
        let lead = principal_for(Role::Lead, &ticket_m);
        let lead_package = granted(&mut w.p, standard(), &lead, &context(&ticket_m));
        let for_lead = lead_package.filter(gathered);
        assert_eq!(for_lead.reader(), lead_package.reader());
        assert_ne!(for_lead.reader(), admitted.reader());
        assert_eq!(for_lead.len(), 3);
    }

    // -------------------------------------------------------------------
    // Module paths.
    // -------------------------------------------------------------------

    fn path(text: &str) -> ModulePath {
        ModulePath::parse(text).unwrap_or_else(|err| panic!("{text:?}: {err}"))
    }

    #[test]
    fn ori_t_0038_module_paths_match_by_component_never_by_prefix() {
        let scope = path("crates/ori-memory");
        assert!(path("crates/ori-memory/src/x.rs").is_within(&scope));
        assert!(path("crates/ori-memory").is_within(&scope));
        assert!(!path("crates/ori-memory-evil/x.rs").is_within(&scope));
        assert!(!path("crates/ori-memory-evil").is_within(&scope));
        assert!(!path("crates/ori-memoryx").is_within(&scope));
        assert!(!path("crates/ori").is_within(&scope));
        assert!(!path("crates").is_within(&scope));
        assert!(path("crates").overlaps(&scope));
        assert!(!path("crates/ori-memory-evil").overlaps(&scope));
        // Case is compared exactly: the refusing direction.
        assert!(!path("Crates/ori-memory/x.rs").is_within(&scope));
        assert!(!path("crates/ORI-MEMORY/x.rs").is_within(&scope));
        // One trailing slash is the same path.
        assert_eq!(path("crates/ori-memory/"), scope);

        // The component rule is ori_core::types::Scope's own.
        let core = Scope::new(["crates/ori-memory"]).expect("a scope");
        for candidate in [
            "crates/ori-memory",
            "crates/ori-memory/src/x.rs",
            "crates/ori-memory-evil/x.rs",
            "crates/ori-memoryx",
            "crates/ori",
            "crates",
            "Crates/ori-memory/x.rs",
        ] {
            assert_eq!(
                path(candidate).is_within(&scope),
                core.claims(candidate),
                "{candidate}"
            );
        }
    }

    #[test]
    fn ori_t_0038_module_paths_refuse_dot_dot_empty_backslash_absolute_and_drop_one_trailing_slash()
    {
        for (text, rejection) in [
            ("", PathRejection::Empty),
            (" crates/m", PathRejection::SurroundingWhitespace),
            ("crates/m ", PathRejection::SurroundingWhitespace),
            ("crates/\u{0}m", PathRejection::ControlCharacter),
            ("crates/m\n", PathRejection::SurroundingWhitespace),
            ("crates\\m", PathRejection::Backslash),
            ("/crates/m", PathRejection::Absolute),
            ("/", PathRejection::Absolute),
            ("C:/crates/m", PathRejection::Absolute),
            ("C:crates", PathRejection::Absolute),
            ("crates/*", PathRejection::Glob),
            ("crates//m", PathRejection::EmptyComponent),
            ("crates/m//", PathRejection::EmptyComponent),
            ("./crates", PathRejection::CurrentDirectory),
            ("crates/./m", PathRejection::CurrentDirectory),
            ("..", PathRejection::ParentDirectory),
            ("crates/../m", PathRejection::ParentDirectory),
            ("crates/m/..", PathRejection::ParentDirectory),
        ] {
            assert_eq!(ModulePath::parse(text), Err(rejection), "{text:?}");
        }
        assert_eq!(path("crates/m/").as_str(), "crates/m");
        assert_eq!(path("crates/m").as_str(), "crates/m");
    }

    // -------------------------------------------------------------------
    // What is logged, and what a logging failure does.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0038_each_refusal_appends_exactly_one_event_with_actor_request_reason_and_reference() {
        let mut w = world("refusal-event");
        let coder = principal_for(Role::Coder, &w.ticket_m);
        let first = refused(
            &mut w.p,
            standard(),
            &coder,
            &read("merged_diff"),
            "source_not_in_scope",
        );
        let event = last_event(&mut w.p);
        let payload = payload_json(&event);
        assert_eq!(payload["actor"]["kind"], "agent");
        assert_eq!(
            payload["actor"]["identity"],
            identity_of(Role::Coder).as_str()
        );
        assert_eq!(
            payload["request"]["sources"],
            serde_json::json!(["merged_diff"])
        );
        assert_eq!(payload["methodology_ref"]["subsection"], Value::Null);
        assert_eq!(event.ticket_id(), None);
        assert_eq!(
            first.refusal().request(),
            &read("merged_diff"),
            "the refusal carries the request as made"
        );

        let before = tip(&mut w.p);
        refused(
            &mut w.p,
            standard(),
            &coder,
            &read("analytics_summary"),
            "source_not_in_scope",
        );
        refused(
            &mut w.p,
            standard(),
            &coder,
            &context(&w.ticket_o.clone()),
            "ticket_not_own",
        );
        assert_eq!(tip(&mut w.p), before + 2, "two refusals, two events");
    }

    #[test]
    fn ori_t_0038_a_refusal_whose_log_append_fails_is_still_refused() {
        let mut w = world("refusal-unlogged");
        let coder = principal_for(Role::Coder, &w.ticket_m);
        let before = tip(&mut w.p);
        w.p.db
            .connection()
            .execute_batch("PRAGMA query_only = ON;")
            .expect("the connection goes read-only");

        let err = standard()
            .authorize(&mut w.p.db, at(500), &coder, &read("merged_diff"))
            .expect_err("a refusal that cannot be logged is still a refusal");
        assert!(!err.is_logged());
        assert!(err.log_failure().is_some());
        assert_eq!(err.reason().code(), "source_not_in_scope");
        assert_eq!(err.refusal().event_seq(), None);
        assert_eq!(err.methodology_ref().section, 25);

        // A grant needs no append, so it still goes through: the failure
        // above is the refusal's, not the enforcer's.
        standard()
            .authorize(&mut w.p.db, at(500), &coder, &read("canonical_section"))
            .expect("a grant appends nothing");

        w.p.db
            .connection()
            .execute_batch("PRAGMA query_only = OFF;")
            .expect("the connection goes writable again");
        assert_eq!(tip(&mut w.p), before, "nothing was recorded");
    }

    #[test]
    fn ori_t_0038_an_unreadable_log_is_a_logged_refusal() {
        let mut w = world("tampered");
        let lead = principal_for(Role::Lead, &w.ticket_m);
        w.p.db
            .connection()
            .execute_batch(
                "DROP TRIGGER IF EXISTS events_no_update; \
                 UPDATE events SET payload = '{\"tampered\":true}' WHERE seq = 1;",
            )
            .expect("the fixture tampers with the first row");
        refused(&mut w.p, standard(), &lead, &read("adr"), "log_unreadable");
    }

    #[test]
    fn ori_t_0038_unknown_source_names_and_query_text_never_reach_the_log() {
        let mut w = world("no-free-text");
        let coder = principal_for(Role::Coder, &w.ticket_m);
        let marker = "PLANTEDMARKERTEXT";
        refused(
            &mut w.p,
            standard(),
            &coder,
            &search("q", &["canonical_section", marker]),
            "unknown_source",
        );
        let event = last_event(&mut w.p);
        assert!(!event.payload().contains(marker), "{}", event.payload());
        let payload = payload_json(&event);
        assert_eq!(
            payload["request"]["sources"],
            serde_json::json!(["canonical_section"])
        );
        assert_eq!(payload["request"]["sources_unrecognized"], 1);

        let query = format!("find {marker} please");
        refused(
            &mut w.p,
            standard(),
            &coder,
            &search(&query, &["merged_diff"]),
            "source_not_in_scope",
        );
        let event = last_event(&mut w.p);
        assert!(!event.payload().contains(marker), "{}", event.payload());
        assert_eq!(
            payload_json(&event)["request"]["query_chars"],
            query.chars().count()
        );

        let request = MemoryRequest {
            claims: Claims {
                role: Some(marker.to_owned()),
                declared_scope: Some(vec![format!("../{marker}")]),
                ..Claims::default()
            },
            ..read("adr")
        };
        refused(&mut w.p, standard(), &coder, &request, "claim_malformed");
        let event = last_event(&mut w.p);
        assert!(!event.payload().contains(marker), "{}", event.payload());
        let payload = payload_json(&event);
        assert_eq!(payload["request"]["claims"]["role_unrecognized"], true);
        assert_eq!(payload["request"]["claims"]["declared_scope_entries"], 1);
    }

    // -------------------------------------------------------------------
    // Raw evidence.
    // -------------------------------------------------------------------

    struct EvidenceWorld {
        w: World,
        finding: MemoryRecord,
        incident: MemoryRecord,
        closing: MemoryRecord,
    }

    fn evidence_world(label: &str) -> EvidenceWorld {
        let mut w = world(label);
        let ticket = w.ticket_m.clone();
        let finding = record(
            &mut w.p,
            "RECF",
            RecordKind::Finding,
            Some(&ticket),
            Some("EVF"),
        );
        let incident = record(
            &mut w.p,
            "RECI",
            RecordKind::Incident,
            Some(&ticket),
            Some("EVI"),
        );
        let closing = record(
            &mut w.p,
            "RECC",
            RecordKind::ClosingReport,
            Some(&ticket),
            Some("EVC"),
        );
        EvidenceWorld {
            w,
            finding,
            incident,
            closing,
        }
    }

    /// Asserts an evidence grant: the blobs of `record`, untrusted, and one
    /// access event recorded before they were handed over.
    fn evidence_granted(
        p: &mut Product,
        principal: &Principal,
        record: &MemoryRecord,
    ) -> Authorization {
        let before = tip(p);
        let authorization = standard()
            .authorize(&mut p.db, at(600), principal, &evidence(record.id()))
            .unwrap_or_else(|err| panic!("expected an evidence grant, got: {err}"));
        assert_eq!(
            tip(p),
            before + 1,
            "an evidence read appends exactly one event"
        );
        let event = last_event(p);
        assert_eq!(event.kind(), EVIDENCE_ACCESS_EVENT_KIND);
        assert_eq!(event.actor(), principal.actor());
        assert_eq!(authorization.evidence_access_seq(), Some(event.seq()));
        assert_eq!(event.ticket_id(), record.provenance().ticket_id());
        let payload = payload_json(&event);
        assert_eq!(payload["untrusted"], true);
        assert_eq!(payload["record_id"], record.id().as_str());
        assert!(!authorization.evidence().is_empty());
        assert!(authorization.evidence().iter().all(EvidenceBlob::untrusted));
        assert!(
            authorization
                .evidence()
                .iter()
                .all(|blob| blob.record_id() == record.id())
        );
        assert_eq!(
            payload["evidence_ids"],
            serde_json::json!(
                authorization
                    .evidence()
                    .iter()
                    .map(|blob| blob.id().as_str())
                    .collect::<Vec<_>>()
            )
        );
        assert!(
            authorization.sources().is_empty(),
            "an evidence grant queries nothing else"
        );
        authorization
    }

    #[test]
    fn ori_t_0038_evidence_is_granted_only_to_lead_qa_operations_labeled_untrusted_and_logged() {
        let mut e = evidence_world("evidence");
        let ticket = e.w.ticket_m.clone();

        let lead = principal_for(Role::Lead, &ticket);
        let authorization = evidence_granted(&mut e.w.p, &lead, &e.finding);
        // The evidence grant admits nothing through the filter, the record
        // itself included.
        assert!(!authorization.admits(&Candidate::Record(e.finding.clone())));
        for blob in authorization.evidence() {
            assert!(!authorization.admits(&Candidate::Evidence(blob.clone())));
        }
        evidence_granted(&mut e.w.p, &lead, &e.closing);

        let qa = principal_for(Role::Qa, &ticket);
        evidence_granted(&mut e.w.p, &qa, &e.finding);
        refused(
            &mut e.w.p,
            standard(),
            &qa,
            &evidence(e.closing.id()),
            "record_not_in_scope",
        );
        refused(
            &mut e.w.p,
            standard(),
            &qa,
            &evidence(e.incident.id()),
            "record_not_in_scope",
        );

        let operations = principal_for(Role::Operations, &ticket);
        evidence_granted(&mut e.w.p, &operations, &e.incident);
        refused(
            &mut e.w.p,
            standard(),
            &operations,
            &evidence(e.finding.id()),
            "record_not_in_scope",
        );

        for role in [
            Role::Coder,
            Role::Documentation,
            Role::ProductSignal,
            Role::Assistant,
        ] {
            refused(
                &mut e.w.p,
                standard(),
                &principal_for(role, &ticket),
                &evidence(e.finding.id()),
                "evidence_not_granted",
            );
        }

        evidence_granted(&mut e.w.p, &operator(), &e.closing);
        refused(
            &mut e.w.p,
            standard(),
            &lead,
            &evidence(&id("NOSUCHRECORD")),
            "record_not_in_product",
        );

        // Named in a search or a read, raw evidence is refused even to a
        // role granted it.
        refused(
            &mut e.w.p,
            standard(),
            &lead,
            &search("trace", &["raw_evidence"]),
            "evidence_only_on_explicit_request",
        );
        refused(
            &mut e.w.p,
            standard(),
            &operator(),
            &read("raw_evidence"),
            "evidence_only_on_explicit_request",
        );
    }

    #[test]
    fn ori_t_0038_evidence_access_that_cannot_be_logged_is_refused() {
        let mut e = evidence_world("evidence-unlogged");
        let lead = principal_for(Role::Lead, &e.w.ticket_m);
        let before = tip(&mut e.w.p);
        e.w.p
            .db
            .connection()
            .execute_batch("PRAGMA query_only = ON;")
            .expect("the connection goes read-only");
        let err = standard()
            .authorize(&mut e.w.p.db, at(600), &lead, &evidence(e.finding.id()))
            .expect_err("an evidence read that cannot be logged is not granted");
        assert_eq!(err.reason().code(), "evidence_access_not_logged");
        assert!(!err.is_logged());
        assert_eq!(err.methodology_ref().section, 25);
        e.w.p
            .db
            .connection()
            .execute_batch("PRAGMA query_only = OFF;")
            .expect("the connection goes writable again");
        assert_eq!(tip(&mut e.w.p), before);
    }

    #[test]
    fn ori_t_0038_evidence_for_a_record_id_two_records_carry_is_refused_whichever_comes_first() {
        // (label, the reader, a kind it reads, a kind it does not)
        let pairs = [
            (
                "qa",
                Role::Qa,
                RecordKind::Finding,
                RecordKind::ClosingReport,
            ),
            (
                "operations",
                Role::Operations,
                RecordKind::Incident,
                RecordKind::Finding,
            ),
        ];
        for (label, role, readable, unreadable) in pairs {
            for (order, first, second) in [
                ("readable-first", readable, unreadable),
                ("unreadable-first", unreadable, readable),
            ] {
                let mut w = world(&format!("reused-{label}-{order}"));
                let ticket = w.ticket_m.clone();
                let shared = id("RECSHARED");
                record(&mut w.p, "RECSHARED", first, Some(&ticket), Some("EVFIRST"));
                record(
                    &mut w.p,
                    "RECSHARED",
                    second,
                    Some(&ticket),
                    Some("EVSECOND"),
                );
                // A record the reader reads, alone under its id, is granted:
                // the refusal below is the reuse, not the reader.
                let alone = record(
                    &mut w.p,
                    "RECALONE",
                    readable,
                    Some(&ticket),
                    Some("EVALONE"),
                );
                let principal = principal_for(role, &ticket);
                evidence_granted(&mut w.p, &principal, &alone);
                for asking in [principal, principal_for(Role::Lead, &ticket), operator()] {
                    refused(
                        &mut w.p,
                        standard(),
                        &asking,
                        &evidence(&shared),
                        "record_id_ambiguous",
                    );
                }
            }
        }
    }

    /// Submits a report by `author` at `when`, one field per `(name,
    /// evidence label)`, each field's raw text naming the record and field.
    fn submit(
        p: &mut Product,
        when: i64,
        author: &str,
        label: &str,
        kind: RecordKind,
        fields: &[(&str, &str)],
    ) -> Result<Submission, BarrierError> {
        let ticket = id("TICKETM");
        let fields = fields
            .iter()
            .map(|(name, evidence)| NewReportField {
                name: FieldName::parse(name).expect("a field name"),
                raw_text: format!("raw text of {label}, field {name}"),
                evidence_id: id(evidence),
            })
            .collect();
        submit_report(
            &mut p.db,
            at(when),
            Actor::Agent(id(author)),
            NewReport {
                record_id: id(label),
                kind,
                ticket_id: Some(ticket),
                fields,
            },
            &BarrierConfig::default(),
        )
    }

    /// Puts a directory where the evidence file `evidence` would be written,
    /// so the barrier fails on that field after storing the ones before it:
    /// the submission that failed half way.
    fn block_evidence_file(p: &Product, evidence: &str) {
        let path =
            p.db.dir()
                .join("evidence")
                .join(format!("{}.txt", id(evidence)));
        fs::create_dir_all(&path).expect("a directory where the evidence file would go");
    }

    fn evidence_ids(authorization: &Authorization) -> Vec<Id> {
        authorization
            .evidence()
            .iter()
            .map(|blob| blob.id().clone())
            .collect()
    }

    #[test]
    fn ori_t_0038_evidence_is_only_the_blobs_stored_with_the_record() {
        let one_field = [("summary", "EVX")];

        // Two fields, right after another record by the same author at the
        // same time: exactly the record's two blobs, in order.
        let mut w = world("evidence-run");
        let qa = principal_for(Role::Qa, &w.ticket_m);
        submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECY",
            RecordKind::Finding,
            &[("summary", "EVY")],
        )
        .expect("a submission");
        let x = submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::Finding,
            &[("summary", "EVX1"), ("detail", "EVX2")],
        )
        .expect("a submission")
        .record()
        .clone();
        let authorization = evidence_granted(&mut w.p, &qa, &x);
        assert_eq!(evidence_ids(&authorization), vec![id("EVX1"), id("EVX2")]);

        // A submission under the same id that failed after the record, and
        // one that failed before it by another author at another time: their
        // evidence is not the record's and is not handed over.
        for (label, failed_first, when, author) in [
            ("evidence-orphan-after", false, 200, "AUTHOR"),
            ("evidence-orphan-before", true, 150, "AUTHORTWO"),
        ] {
            let mut w = world(label);
            let qa = principal_for(Role::Qa, &w.ticket_m);
            let fail = |p: &mut Product| {
                block_evidence_file(p, "EVB");
                submit(
                    p,
                    when,
                    author,
                    "RECX",
                    RecordKind::ClosingReport,
                    &[("summary", "EVA"), ("detail", "EVB")],
                )
                .expect_err("the second field cannot be stored");
            };
            if failed_first {
                fail(&mut w.p);
            }
            let x = submit(
                &mut w.p,
                200,
                "AUTHOR",
                "RECX",
                RecordKind::Finding,
                &one_field,
            )
            .expect("a submission")
            .record()
            .clone();
            if !failed_first {
                fail(&mut w.p);
            }
            let orphans = barrier::read_evidence(&mut w.p.db)
                .expect("evidence reads back")
                .into_iter()
                .filter(|blob| blob.id() == &id("EVA") && blob.record_id() == x.id())
                .count();
            assert_eq!(orphans, 1, "{label}: the failed submission left its blob");
            let authorization = evidence_granted(&mut w.p, &qa, &x);
            assert_eq!(evidence_ids(&authorization), vec![id("EVX")], "{label}");
        }

        // The same failure by the same author at the same time, right before
        // the record, cannot be told apart from the record's own: refused.
        let mut w = world("evidence-orphan-adjacent");
        let qa = principal_for(Role::Qa, &w.ticket_m);
        block_evidence_file(&w.p, "EVB");
        submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::ClosingReport,
            &[("summary", "EVA"), ("detail", "EVB")],
        )
        .expect_err("the second field cannot be stored");
        submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::Finding,
            &one_field,
        )
        .expect("a submission");
        refused(
            &mut w.p,
            standard(),
            &qa,
            &evidence(&id("RECX")),
            "evidence_not_attributable",
        );

        // A later record storing evidence under the same evidence id
        // overwrote the file: the record's blob may hold the other's raw
        // text, so it is refused.
        let mut w = world("evidence-id-reused");
        let qa = principal_for(Role::Qa, &w.ticket_m);
        submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::Finding,
            &[("summary", "EVSHARED")],
        )
        .expect("a submission");
        submit(
            &mut w.p,
            300,
            "AUTHOR",
            "RECZ",
            RecordKind::ClosingReport,
            &[("summary", "EVSHARED")],
        )
        .expect("a submission");
        refused(
            &mut w.p,
            standard(),
            &qa,
            &evidence(&id("RECX")),
            "evidence_not_attributable",
        );

        // A failed resubmission under the same record id reused the record's
        // evidence id, overwriting its file: refused, though every event
        // naming that id names this record.
        let mut w = world("evidence-id-resubmitted");
        let qa = principal_for(Role::Qa, &w.ticket_m);
        submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::Finding,
            &one_field,
        )
        .expect("a submission");
        block_evidence_file(&w.p, "EVB");
        submit(
            &mut w.p,
            300,
            "AUTHOR",
            "RECX",
            RecordKind::ClosingReport,
            &[("summary", "EVX"), ("detail", "EVB")],
        )
        .expect_err("the second field cannot be stored");
        refused(
            &mut w.p,
            standard(),
            &qa,
            &evidence(&id("RECX")),
            "evidence_not_attributable",
        );

        // Two fields of one record stored under one evidence id: the second
        // overwrote the first's file.
        let mut w = world("evidence-id-twice-in-record");
        let qa = principal_for(Role::Qa, &w.ticket_m);
        submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::Finding,
            &[("summary", "EVX"), ("detail", "EVX")],
        )
        .expect("a submission");
        refused(
            &mut w.p,
            standard(),
            &qa,
            &evidence(&id("RECX")),
            "evidence_not_attributable",
        );

        // An evidence event that does not parse could be anyone's.
        let mut w = world("evidence-garbage");
        let qa = principal_for(Role::Qa, &w.ticket_m);
        let x = submit(
            &mut w.p,
            200,
            "AUTHOR",
            "RECX",
            RecordKind::Finding,
            &one_field,
        )
        .expect("a submission")
        .record()
        .clone();
        evidence_granted(&mut w.p, &qa, &x);
        append(
            &mut w.p,
            "memory.evidence_stored",
            None,
            "not json".to_owned(),
        );
        refused(
            &mut w.p,
            standard(),
            &qa,
            &evidence(x.id()),
            "evidence_not_attributable",
        );
    }

    // -------------------------------------------------------------------
    // The payload this module reads is the one ori-broker writes.
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0038_identity_payload_shape_matches_ori_broker_registration() {
        let path = repo_file(&["crates", "ori-broker", "src", "registration.rs"]);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
        assert!(text.contains("\"identity.created\""));
        for key in ["id", "product_id", "role"] {
            let written = format!("\\\"{key}\\\":\\\"{{}}\\\"");
            assert!(
                text.contains(&written),
                "registration.rs no longer writes {key} the way this module reads it"
            );
        }
    }

    /// This file up to its test module: what the module itself says and
    /// does, without the strings its tests search it for.
    fn module_code() -> &'static str {
        include_str!("scope.rs")
            .split_once("\n#[cfg(test)]\nmod tests {")
            .expect("the file has a test module")
            .0
    }

    #[test]
    fn ori_t_0038_identity_payload_layout_and_rules_match_ori_broker_registration() {
        let registration = fs::read_to_string(repo_file(&[
            "crates",
            "ori-broker",
            "src",
            "registration.rs",
        ]))
        .expect("registration.rs reads");
        let identity =
            fs::read_to_string(repo_file(&["crates", "ori-broker", "src", "identity.rs"]))
                .expect("identity.rs reads");
        let this = module_code();
        // The writer's layout, written the same way in both files.
        let layout = r#""{{\"id\":\"{}\",\"product_id\":\"{}\",\"role\":\"{}\",\"model\":\"{}\",\"family\":\"{}\",\"runtime\":\"{}\"}}","#;
        assert!(registration.contains(layout), "registration.rs's layout");
        assert!(this.contains(layout), "this module's layout");
        // Its arguments, in order, escaped where it escapes them.
        let written = [
            "json_escape(identity.id().as_str()),",
            "json_escape(identity.product_id().as_str()),",
            "identity.role(),",
            "json_escape(identity.model()),",
            "json_escape(identity.family().as_str()),",
            "identity.runtime(),",
        ]
        .map(|line| format!("        {line}\n"))
        .concat();
        assert!(
            registration.contains(&written),
            "registration.rs's arguments"
        );
        let mirrored = [
            "broker_escape(&self.id),",
            "broker_escape(&self.product_id),",
            "self.role,",
            "broker_escape(&self.model),",
            "broker_escape(&self.family),",
            "self.runtime,",
        ]
        .map(|line| format!("            {line}\n"))
        .concat();
        assert!(this.contains(&mirrored), "this module's arguments");
        // Its escape: `"` and `\`, nothing else.
        for arm in [
            r#"'"' => out.push_str("\\\""),"#,
            r#"'\\' => out.push_str("\\\\"),"#,
            "_ => out.push(ch),",
        ] {
            assert!(registration.contains(arm), "registration.rs: {arm}");
            assert!(this.contains(arm), "this module: {arm}");
        }
        // Its reader's rules.
        for rule in [
            "let role: Role = role_text",
            r#"let model = field_str(payload, "model")"#,
            "let family = ModelFamily::parse(&family_text)",
            "let runtime: IdentityRuntime = runtime_text.parse()",
        ] {
            assert!(registration.contains(rule), "registration.rs: {rule}");
        }
        let mut runtimes = Vec::new();
        for line in identity.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix('"')
                && let Some((name, arm)) = rest.split_once("\" => Ok(Self::")
                && (arm.starts_with("Acp)") || arm.starts_with("Headless)"))
            {
                runtimes.push(name.to_owned());
            }
        }
        assert!(
            identity
                .contains(r#"other => Err(IdentityError::malformed("IdentityRuntime", other)),"#),
            "IdentityRuntime reads no other name"
        );
        assert_eq!(runtimes, IDENTITY_RUNTIMES.map(str::to_owned).to_vec());
    }

    #[test]
    fn ori_t_0038_an_identity_record_ori_broker_would_refuse_or_read_otherwise_is_refused() {
        let qa = identity_of(Role::Qa);
        let lead = identity_of(Role::Lead);
        let cases: [(&str, &str); 13] = [
            (
                "no model",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","family":"family-a","runtime":"acp"}"#,
            ),
            (
                "no family",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","runtime":"acp"}"#,
            ),
            (
                "no runtime",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"family-a"}"#,
            ),
            (
                "an empty family",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"","runtime":"acp"}"#,
            ),
            (
                "a blank family",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"   ","runtime":"acp"}"#,
            ),
            (
                "an unknown runtime",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"family-a","runtime":"bogus"}"#,
            ),
            (
                "a model that is not text",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":7,"family":"family-a","runtime":"acp"}"#,
            ),
            (
                "the role twice",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"family-a","runtime":"acp","role":"lead"}"#,
            ),
            (
                "a nested role ahead of the role",
                r#"{"meta":{"role":"lead"},"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"family-a","runtime":"acp"}"#,
            ),
            (
                "a space after a colon",
                r#"{"id":"{Q}","product_id":"{P}","role": "qa","model":"model-a","family":"family-a","runtime":"acp"}"#,
            ),
            (
                "an escaped role",
                r#"{"id":"{Q}","product_id":"{P}","role":"{ESCAPED_QA}","model":"model-a","family":"family-a","runtime":"acp"}"#,
            ),
            (
                "the fields in another order than the writer's",
                r#"{"role":"qa","id":"{Q}","product_id":"{P}","model":"model-a","family":"family-a","runtime":"acp"}"#,
            ),
            (
                "a field the writer does not write",
                r#"{"id":"{Q}","product_id":"{P}","role":"qa","model":"model-a","family":"family-a","runtime":"acp","note":"x"}"#,
            ),
        ];
        // "qa" with its first letter written as a JSON unicode escape, built
        // from the backslash's code point.
        let escaped_qa = format!("{}u0071a", char::from(92_u8));
        for (index, (label, template)) in cases.into_iter().enumerate() {
            let mut p = product(&format!("identity-owner-{index}"), "PRODUCTC");
            register(&mut p, &lead, Role::Lead);
            let payload = template
                .replace("{Q}", qa.as_str())
                .replace("{P}", p.id.as_str())
                .replace("{ESCAPED_QA}", &escaped_qa);
            if label == "an escaped role" {
                let wire: IdentityCreatedWire =
                    serde_json::from_str(&payload).expect("the escape is JSON");
                assert_eq!(wire.role, "qa", "serde reads the escape as qa");
            }
            append(&mut p, "identity.created", None, payload);
            // The identity it names, and anyone else: ori-broker's reader
            // refuses the whole log over it.
            for principal in [agent(&qa), agent(&lead)] {
                let before = tip(&mut p);
                match standard().authorize(&mut p.db, at(500), &principal, &read("adr")) {
                    Ok(_) => panic!("{label}: granted over an identity record ori-broker refuses"),
                    Err(err) => {
                        assert_eq!(
                            err.reason().code(),
                            "identity_record_malformed",
                            "{label}: {err}"
                        );
                        assert!(err.is_logged(), "{label}");
                    }
                }
                assert_eq!(tip(&mut p), before + 1, "{label}");
            }
        }

        // Laid out as the writer lays it out, with a model and a family the
        // writer escapes: read, and read as the writer's values.
        let mut p = product("identity-owner-escaped", "PRODUCTC");
        let stranger = id("STRANGER");
        let payload = format!(
            r#"{{"id":"{stranger}","product_id":"{}","role":"documentation","model":"a\"b\\c","family":"f\"g","runtime":"headless"}}"#,
            p.id
        );
        append(&mut p, "identity.created", None, payload);
        register(&mut p, &qa, Role::Qa);
        let authorization = granted(&mut p, standard(), &agent(&qa), &read("adr"));
        assert_eq!(authorization.role(), Some(Role::Qa));
        let authorization = granted(&mut p, standard(), &agent(&stranger), &read("adr"));
        assert_eq!(authorization.role(), Some(Role::Documentation));
    }

    // -------------------------------------------------------------------
    // The assignment the engine supplies stands only where nothing in the
    // log ties its ticket to another identity.
    // -------------------------------------------------------------------

    #[test]
    fn ori_p1_022_an_engine_ticket_the_log_ties_to_another_identity_is_refused_live_revoked_expired_or_before_any_claim()
     {
        let coder = identity_of(Role::Coder);
        let rival = id("RIVALCODER");
        // How the coder's own sessions stand when it asks.
        let coder_states = [
            "no session",
            "a live session with no claim",
            "a revoked session that claimed M",
            "an expired session that claimed M",
        ];
        // How the rival's session, under which it claimed O, stands.
        let rival_states = ["live", "revoked", "expired"];
        for (c, coder_state) in coder_states.into_iter().enumerate() {
            for (r, rival_state) in rival_states.into_iter().enumerate() {
                let label = format!("coder with {coder_state}, rival's session {rival_state}");
                let mut w = world(&format!("held-elsewhere-{c}-{r}"));
                let ticket_m = w.ticket_m.clone();
                let ticket_o = w.ticket_o.clone();
                register(&mut w.p, &rival, Role::Coder);
                let rival_session = id("SESSIONRIVAL");
                if rival_state == "expired" {
                    issue_expiring(&mut w.p, &rival, &rival_session, 300);
                } else {
                    issue(&mut w.p, &rival, &rival_session);
                }
                claim_in_session(&mut w.p, &ticket_o, "crates/other", &rival_session);
                if rival_state == "revoked" {
                    revoke(&mut w.p, &rival_session);
                }
                let own_session = id("SESSIONOWN");
                match coder_state {
                    "a live session with no claim" => issue(&mut w.p, &coder, &own_session),
                    "a revoked session that claimed M" => {
                        issue(&mut w.p, &coder, &own_session);
                        claim_in_session(&mut w.p, &ticket_m, "crates/m", &own_session);
                        revoke(&mut w.p, &own_session);
                    }
                    "an expired session that claimed M" => {
                        issue_expiring(&mut w.p, &coder, &own_session, 300);
                        claim_in_session(&mut w.p, &ticket_m, "crates/m", &own_session);
                    }
                    _ => {}
                }

                // The engine says O: every request is refused, and logged.
                let on_o = agent(&coder).with_assigned_ticket(ticket_o.clone());
                for request in [
                    context(&ticket_o),
                    read("code_map"),
                    read("canonical_section"),
                    read("operational_record"),
                    search("q", &[]),
                ] {
                    let err = refused(
                        &mut w.p,
                        standard(),
                        &on_o,
                        &request,
                        "assignment_held_elsewhere",
                    );
                    assert!(
                        matches!(
                            err.reason(),
                            RefusalReason::AssignmentHeldElsewhere { ticket_id } if ticket_id == &ticket_o
                        ),
                        "{label}: {err}"
                    );
                }
                // The engine saying M, which the log ties to nobody else,
                // stands.
                let on_m = agent(&coder).with_assigned_ticket(ticket_m.clone());
                let package = granted(&mut w.p, standard(), &on_m, &context(&ticket_m));
                assert_eq!(package.ticket_id(), Some(&ticket_m), "{label}");
                assert_eq!(module_names(&package), vec!["crates/m"], "{label}");
            }
        }

        // A claim of the assigned ticket whose session cannot be read, live or
        // released, may be another identity's claim once the log issues any
        // session to another identity.
        let mut w = world("held-elsewhere-unreadable");
        let ticket_m = w.ticket_m.clone();
        let on_m = principal_for(Role::Coder, &ticket_m);
        append(
            &mut w.p,
            "lock.claimed",
            Some(&ticket_m),
            "not json".to_owned(),
        );
        release(&mut w.p, &ticket_m);
        granted(&mut w.p, standard(), &on_m, &read("code_map"));
        issue(&mut w.p, &rival, &id("SESSIONRIVAL"));
        refused(
            &mut w.p,
            standard(),
            &on_m,
            &read("code_map"),
            "assignment_record_malformed",
        );

        // A claim whose session the lock projection reads as the rival's
        // while serde reads the coder's: which one it is cannot be told.
        let mut w = world("held-elsewhere-two-readings");
        let ticket_m = w.ticket_m.clone();
        let own_session = id("SESSIONOWN");
        let rival_session = id("SESSIONRIVAL");
        issue(&mut w.p, &coder, &own_session);
        issue(&mut w.p, &rival, &rival_session);
        let payload = format!(
            r#"{{"module":"crates/m","n":"session_id","a":"{rival_session}","session_id":"{own_session}"}}"#
        );
        append(&mut w.p, "lock.claimed", Some(&ticket_m), payload);
        let on_m = agent(&coder).with_assigned_ticket(ticket_m.clone());
        refused(
            &mut w.p,
            standard(),
            &on_m,
            &read("code_map"),
            "assignment_record_malformed",
        );
    }

    // -------------------------------------------------------------------
    // The declared scope is the rows the lock projection keeps.
    // -------------------------------------------------------------------

    /// The lock projection's rows, `module` to ticket and session, as its
    /// own table holds them.
    fn projection_rows(p: &mut Product) -> BTreeMap<String, (String, Option<String>)> {
        let conn = p.db.connection();
        let mut statement = conn
            .prepare("SELECT module, ticket_id, session_id FROM proj_locks")
            .expect("the lock projection's table reads");
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, (row.get(1)?, row.get(2)?)))
            })
            .expect("the lock projection's rows read")
            .collect::<Result<_, _>>()
            .expect("every row reads")
    }

    /// This module's fold of the same log, in the same shape, with each
    /// row's session as [`claim_session`] reads the claim that made it.
    fn fold_rows(p: &mut Product) -> BTreeMap<String, (String, Option<String>)> {
        let events = read_log(&mut p.db).expect("the log reads");
        let fold = fold_locks(&events).expect("every lock event carries a ticket");
        fold.live
            .into_iter()
            .map(|(key, (ticket, _))| {
                let session = events
                    .iter()
                    .rev()
                    .find(|event| {
                        event.kind() == LOCK_CLAIMED
                            && projection_field(event.payload(), "module") == Some(key.as_str())
                    })
                    .and_then(|event| claim_session(event).ok().flatten())
                    .map(|session| session.as_str().to_owned());
                (key, (ticket.as_str().to_owned(), session))
            })
            .collect()
    }

    #[test]
    fn ori_p1_022_the_lock_fold_keeps_the_rows_the_lock_projection_keeps() {
        // A later claim of the same module by another ticket takes it over,
        // as the projection keys its rows: ticket M keeps no module O took,
        // and O's records are not M's.
        let mut w = world("lock-keyed");
        let ticket_m = w.ticket_m.clone();
        let ticket_o = w.ticket_o.clone();
        let a = w.p.id.clone();
        claim(&mut w.p, &ticket_m, "crates/n");
        claim(&mut w.p, &ticket_o, "crates/m");
        let closing_o = Candidate::Record(record(
            &mut w.p,
            "RECKO",
            RecordKind::ClosingReport,
            Some(&ticket_o),
            None,
        ));
        let finding_m = Candidate::Record(record(
            &mut w.p,
            "RECKM",
            RecordKind::Finding,
            Some(&ticket_m),
            None,
        ));
        let in_m = code_module(&a, "crates/m/src/a.rs");
        let in_n = code_module(&a, "crates/n/src/b.rs");
        let coder = principal_for(Role::Coder, &ticket_m);
        let package = granted(&mut w.p, standard(), &coder, &context(&ticket_m));
        assert_eq!(module_names(&package), vec!["crates/n"]);
        assert!(
            !package.admits(&closing_o),
            "O's record after O took crates/m"
        );
        assert!(!package.admits(&in_m), "crates/m after O took it");
        assert!(package.admits(&finding_m));
        assert!(package.admits(&in_n));
        let scope = package.declared_scope().expect("a declared scope");
        assert!(!scope.claims("crates/m/src/a.rs"));

        // O's release does not hand the module back: the projection holds no
        // row for it.
        release(&mut w.p, &ticket_o);
        let package = granted(&mut w.p, standard(), &coder, &read("code_map"));
        assert_eq!(module_names(&package), vec!["crates/n"]);

        // M claims it again, and holds it.
        claim(&mut w.p, &ticket_m, "crates/m");
        let package = granted(&mut w.p, standard(), &coder, &read("code_map"));
        assert_eq!(module_names(&package), vec!["crates/m", "crates/n"]);

        // A takeover the projection's reader sees where serde sees another
        // module: the projection keys the row by what its reader reads.
        let payload = r#"{"note":"module","x":"crates/n","module":"crates/zzz"}"#.to_owned();
        append(&mut w.p, "lock.claimed", Some(&ticket_o), payload);
        let package = granted(&mut w.p, standard(), &coder, &read("code_map"));
        assert_eq!(module_names(&package), vec!["crates/m"]);
        assert!(
            !package.admits(&closing_o),
            "O's claim is unreadable to this module"
        );

        // The fold keeps exactly the projection's rows, event by event, over
        // payloads the two readers could read apart.
        let mut p = product("lock-differential", "PRODUCTA");
        let m = id("TICKETM");
        let o = id("TICKETO");
        let q = id("TICKETQ");
        let session = id("SESSIONA");
        let events: Vec<(&Id, &str, String)> = vec![
            (&m, "lock.claimed", r#"{"module":"crates/a"}"#.to_owned()),
            (&m, "lock.claimed", r#"{"module": "crates/b"}"#.to_owned()),
            (&m, "lock.claimed", r#"{"module" : "crates/c"}"#.to_owned()),
            (&o, "lock.claimed", r#"{"module":"crates\/d"}"#.to_owned()),
            (&o, "lock.claimed", r#"{"module":"crates/e\"x"}"#.to_owned()),
            (
                &o,
                "lock.claimed",
                r#"{"note":"module","x":"crates/a","module":"crates/zzz"}"#.to_owned(),
            ),
            (&q, "lock.claimed", r#"{"module":"crates/a/"}"#.to_owned()),
            (
                &q,
                "lock.claimed",
                r#"{"module":"crates/b","module":"crates/f"}"#.to_owned(),
            ),
            (&q, "lock.claimed", r#"{"module":7}"#.to_owned()),
            (&q, "lock.claimed", "not json".to_owned()),
            (
                &m,
                "lock.claimed",
                r#"{"session_id":"module","module":"crates/g"}"#.to_owned(),
            ),
            (
                &m,
                "lock.claimed",
                format!(r#"{{"module":"crates/h","session_id":"{session}"}}"#),
            ),
            (
                &o,
                "lock.claimed",
                format!(
                    r#"{{"module":"crates/i","n":"session_id","a":"{session}","session_id":"x"}}"#
                ),
            ),
            (&m, "lock.claimed", r#"{"module":"crates/c"}"#.to_owned()),
            (&o, "lock.released", "{}".to_owned()),
            (&o, "lock.claimed", r#"{"module":"crates/h"}"#.to_owned()),
            (&m, "lock.released", "{}".to_owned()),
            (&m, "lock.claimed", r#"{"module":"crates/a"}"#.to_owned()),
        ];
        for (index, (ticket, kind, payload)) in events.into_iter().enumerate() {
            let event = append(&mut p, kind, Some(ticket), payload);
            match LockProjection.apply(p.db.connection(), &event) {
                Ok(()) | Err(ProjectionError::MalformedPayload { .. }) => {}
                Err(err) => panic!("event {index}: {err}"),
            }
            let projected = projection_rows(&mut p);
            let folded = fold_rows(&mut p);
            let keys = |rows: &BTreeMap<String, (String, Option<String>)>| {
                rows.iter()
                    .map(|(module, (ticket, _))| (module.clone(), ticket.clone()))
                    .collect::<BTreeMap<_, _>>()
            };
            assert_eq!(keys(&folded), keys(&projected), "after event {index}");
            // Where this module reads a claim's session at all, it reads the
            // one the projection recorded.
            for (module, (_, session)) in &folded {
                if session.is_some() {
                    assert_eq!(
                        session, &projected[module].1,
                        "after event {index}: {module}"
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // The sealed values: no field reachable outside the module that owns it,
    // and every field pinned by an example that fails for that alone.
    // -------------------------------------------------------------------

    /// The fields of `Authorization`, spelled out so that adding, removing or
    /// renaming one fails to compile here until the pins are revisited.
    const AUTHORIZATION_FIELDS: [&str; 8] = [
        "reader",
        "product_id",
        "query",
        "ticket_id",
        "declared",
        "sources",
        "evidence",
        "evidence_access_seq",
    ];

    fn authorization_fields_are_exhaustive(authorization: Authorization) {
        let Authorization {
            reader: _,
            product_id: _,
            query: _,
            ticket_id: _,
            declared: _,
            sources: _,
            evidence: _,
            evidence_access_seq: _,
        } = authorization;
    }

    /// The fields of `Admitted`, for the same reason.
    const ADMITTED_FIELDS: [&str; 3] = ["product_id", "reader", "candidates"];

    fn admitted_fields_are_exhaustive(admitted: Admitted) {
        let Admitted {
            product_id: _,
            reader: _,
            candidates: _,
        } = admitted;
    }

    /// The fields a struct's definition in `source` declares, and the lines
    /// that declare them.
    fn declared_fields<'s>(source: &'s str, name: &str) -> Vec<(&'s str, &'s str)> {
        let opening = format!("pub struct {name} {{");
        let mut lines = source.lines().skip_while(|line| *line != opening);
        assert_eq!(lines.next(), Some(opening.as_str()), "{name} is defined");
        lines
            .take_while(|line| *line != "}")
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with("#["))
            .map(|line| {
                let field = line
                    .split(':')
                    .next()
                    .expect("a field line names its field")
                    .trim();
                let field = field.rsplit(' ').next().unwrap_or(field);
                (field, line)
            })
            .collect()
    }

    /// The fields of `DeclaredScope`, for the same reason.
    const DECLARED_SCOPE_FIELDS: [&str; 4] = ["ticket_id", "scope", "modules", "in_scope_tickets"];

    fn declared_scope_fields_are_exhaustive(declared: DeclaredScope) {
        let DeclaredScope {
            ticket_id: _,
            scope: _,
            modules: _,
            in_scope_tickets: _,
        } = declared;
    }

    #[test]
    fn ori_t_0038_sealed_values_keep_every_field_private_and_pinned_by_a_single_reason_example() {
        let _: fn(Authorization) = authorization_fields_are_exhaustive;
        let _: fn(Admitted) = admitted_fields_are_exhaustive;
        let _: fn(DeclaredScope) = declared_scope_fields_are_exhaustive;
        let this = module_code();
        let sealed: [(&str, &str, &str, &[&str]); 5] = [
            (
                "Authorization",
                "ori_memory::scope::Authorization",
                this,
                &AUTHORIZATION_FIELDS,
            ),
            (
                "Admitted",
                "ori_memory::scope::Admitted",
                this,
                &ADMITTED_FIELDS,
            ),
            (
                "DeclaredScope",
                "ori_memory::scope::DeclaredScope",
                this,
                &DECLARED_SCOPE_FIELDS,
            ),
            (
                "MemoryRecord",
                "ori_memory::barrier::MemoryRecord",
                include_str!("barrier.rs"),
                &[
                    "id",
                    "product_id",
                    "layer",
                    "kind",
                    "structured",
                    "provenance",
                    "untrusted",
                ],
            ),
            (
                "EvidenceBlob",
                "ori_memory::barrier::EvidenceBlob",
                include_str!("barrier.rs"),
                &["id", "record_id", "content_ref", "content_type"],
            ),
        ];
        for (name, path, source, expected) in sealed {
            let fields = declared_fields(source, name);
            let names: Vec<&str> = fields.iter().map(|(field, _)| *field).collect();
            assert_eq!(names, expected, "{name}'s fields");
            for (field, line) in &fields {
                // No `pub`, `pub(crate)`, `pub(super)` or `pub(in ...)`: a
                // sibling module, ORI-T-0039's retrieval included, may no more
                // build or alter one than another crate may.
                assert!(
                    !line.starts_with("pub"),
                    "{name}.{field} is visible outside its module: {line}"
                );
                let touch = format!(
                    "/// ```compile_fail\n/// fn touch(a: &mut {path}) {{\n///     let _ = &mut a.{field};\n/// }}\n/// ```\n"
                );
                assert!(
                    this.contains(&touch),
                    "no single-reason pin for {name}.{field}"
                );
            }
            let widen = format!(
                "/// ```compile_fail\n/// fn widen(a: {path}) -> {path} {{\n///     {path} {{ ..a }}\n/// }}\n/// ```\n"
            );
            assert!(this.contains(&widen), "no functional update pin for {name}");
            let reach = format!("/// ```\n/// fn reach(a: &mut {path}) {{\n");
            assert!(
                this.contains(&reach),
                "no example showing {name}'s path resolves"
            );
        }
    }

    #[test]
    fn ori_t_0038_credential_payload_layout_matches_ori_broker_issuance() {
        let issuance =
            fs::read_to_string(repo_file(&["crates", "ori-broker", "src", "issuance.rs"]))
                .expect("issuance.rs reads");
        let this = module_code();
        let issued = r#""{{\"id\":\"{}\",\"identity_id\":\"{}\",\"session_id\":\"{}\",\"scope\":\"{}\",\"issued_at\":{},\"expires_at\":{}}}","#;
        let revoked = r#""{{\"session_id\":\"{}\",\"revoked_at\":{},\"issuance_ids\":[","#;
        for layout in [issued, revoked] {
            assert!(issuance.contains(layout), "issuance.rs: {layout}");
            assert!(this.contains(layout), "this module: {layout}");
        }
        let lines = |indent: &str, lines: &[&str]| -> String {
            lines
                .iter()
                .map(|line| format!("{indent}{line}\n"))
                .collect()
        };
        for (theirs, ours) in [
            (
                lines(
                    "        ",
                    &[
                        issued,
                        "json_escape(id.as_str()),",
                        "json_escape(identity_id.as_str()),",
                        "json_escape(session_id.as_str()),",
                        "json_escape(scope.as_str()),",
                        "issued_at.millis(),",
                        "expires_at_json,",
                    ],
                ),
                lines(
                    "            ",
                    &[
                        issued,
                        "broker_escape(&self.id),",
                        "broker_escape(&self.identity_id),",
                        "broker_escape(&self.session_id),",
                        "broker_escape(&self.scope),",
                        "self.issued_at,",
                        "expires_at,",
                    ],
                ),
            ),
            (
                lines(
                    "        ",
                    &[
                        revoked,
                        "json_escape(session_id.as_str()),",
                        "revoked_at.millis(),",
                    ],
                ),
                lines(
                    "            ",
                    &[
                        revoked,
                        "broker_escape(&self.session_id),",
                        "self.revoked_at,",
                    ],
                ),
            ),
            (
                [
                    "        if index > 0 {",
                    "            out.push(',');",
                    "        }",
                    "        out.push('\"');",
                    "        out.push_str(&json_escape(id.as_str()));",
                    "        out.push('\"');",
                    "    }",
                    "    out.push_str(\"]}\");",
                ]
                .map(|line| format!("{line}\n"))
                .concat(),
                [
                    "            if index > 0 {",
                    "                out.push(',');",
                    "            }",
                    "            out.push('\"');",
                    "            out.push_str(&broker_escape(id));",
                    "            out.push('\"');",
                    "        }",
                    "        out.push_str(\"]}\");",
                ]
                .map(|line| format!("{line}\n"))
                .concat(),
            ),
        ] {
            assert!(
                issuance.contains(&theirs),
                "issuance.rs no longer writes:\n{theirs}"
            );
            assert!(
                this.contains(&ours),
                "this module no longer lays out:\n{ours}"
            );
        }
        assert!(issuance.contains(r#"None => "null".to_owned(),"#));
        assert!(
            this.contains(r#".map_or_else(|| "null".to_owned(), |millis| millis.to_string());"#)
        );
    }

    #[test]
    fn ori_t_0038_a_credential_event_ori_broker_could_read_differently_is_refused() {
        let coder = identity_of(Role::Coder);
        let rival = id("RIVALCODER");
        let session = id("SESSIONBAD");
        let other_session = id("SESSIONOTHER");
        let issuance = id("ISSUANCE");
        let cases: [(&str, &str, String); 8] = [
            (
                "a holder nested ahead of the holder",
                "credential.issued",
                format!(
                    r#"{{"meta":{{"identity_id":"{rival}"}},"id":"{issuance}","identity_id":"{coder}","session_id":"{session}","scope":"branch","issued_at":100,"expires_at":null}}"#
                ),
            ),
            (
                "the holder twice",
                "credential.issued",
                format!(
                    r#"{{"id":"{issuance}","identity_id":"{coder}","session_id":"{session}","scope":"branch","issued_at":100,"expires_at":null,"identity_id":"{rival}"}}"#
                ),
            ),
            (
                "a space after a colon",
                "credential.issued",
                format!(
                    r#"{{"id":"{issuance}","identity_id": "{coder}","session_id":"{session}","scope":"branch","issued_at":100,"expires_at":null}}"#
                ),
            ),
            (
                "the fields in another order than the writer's",
                "credential.issued",
                format!(
                    r#"{{"session_id":"{session}","id":"{issuance}","identity_id":"{coder}","scope":"branch","issued_at":100,"expires_at":null}}"#
                ),
            ),
            (
                "a field the writer does not write",
                "credential.issued",
                format!(
                    r#"{{"id":"{issuance}","identity_id":"{coder}","session_id":"{session}","scope":"branch","issued_at":100,"expires_at":null,"note":"x"}}"#
                ),
            ),
            (
                "a revoked session nested ahead of the session",
                "credential.revoked",
                format!(
                    r#"{{"meta":{{"session_id":"{other_session}"}},"session_id":"{session}","revoked_at":300,"issuance_ids":[]}}"#
                ),
            ),
            (
                "a revoked session twice",
                "credential.revoked",
                format!(
                    r#"{{"session_id":"{session}","revoked_at":300,"issuance_ids":[],"session_id":"{other_session}"}}"#
                ),
            ),
            (
                "issuance ids laid out otherwise than the writer's",
                "credential.revoked",
                format!(r#"{{"session_id":"{session}","revoked_at":300,"issuance_ids":[ ]}}"#),
            ),
        ];
        for (index, (label, kind, payload)) in cases.into_iter().enumerate() {
            let mut w = world(&format!("broker-layout-{index}"));
            let on_m = agent(&coder).with_assigned_ticket(w.ticket_m.clone());
            granted(&mut w.p, standard(), &on_m, &read("code_map"));
            append(&mut w.p, kind, None, payload);
            let before = tip(&mut w.p);
            match standard().authorize(&mut w.p.db, at(500), &on_m, &read("code_map")) {
                Ok(_) => panic!(
                    "{label}: granted over a credential event ori-broker could read otherwise"
                ),
                Err(err) => {
                    assert_eq!(
                        err.reason().code(),
                        "assignment_record_malformed",
                        "{label}: {err}"
                    );
                    assert!(err.is_logged(), "{label}");
                }
            }
            assert_eq!(tip(&mut w.p), before + 1, "{label}");
        }

        // Laid out as the writer lays it out, with issuance ids: read.
        let mut w = world("broker-layout-written");
        let on_m = agent(&coder).with_assigned_ticket(w.ticket_m.clone());
        issue(&mut w.p, &coder, &session);
        let payload = format!(
            r#"{{"session_id":"{session}","revoked_at":300,"issuance_ids":["{issuance}","{}"]}}"#,
            id("ISSUANCETWO")
        );
        append(&mut w.p, "credential.revoked", None, payload);
        granted(&mut w.p, standard(), &on_m, &read("code_map"));
    }
}
