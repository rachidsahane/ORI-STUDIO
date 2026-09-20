# ROADMAP: Ori Studio

The authority on phases. Each phase is a batch of tickets with its own acceptance criteria (`criteria/phase-<n>.md`) and a phase set of documents approved before it starts (AICD §23). Features are PRD identifiers.

## Phase 1: Headless engine and CLI

**Goal.** The engine exists, enforces the methodology's core, and the CLI exposes it. No UI. The author's own migration backlog runs through it instead of through an implementor session.

**Delivers.** A-01, A-02, A-04, A-05, A-06, A-09; S-01; D-01, D-08, D-09, D-10, D-11, D-13, D-15; L-01, L-03; M-01, M-02, M-04, M-05, M-06, M-14; T-01 to T-07; F-02 (table), F-03 (tier 0 and 1), F-05, F-06; G-01 to G-03; V-03, V-04, V-06; Q-02 (label), Q-03; O-05; P-01 to P-05; K-01 to K-07, K-10; I-01, I-03, I-04, I-05; Z-01 to Z-04, Z-06; the crates ori-core, ori-store, ori-memory (without vectors), ori-broker (identities and keychain), ori-runtime (worktrees, one ACP runtime, one headless adapter, credential injection, budgets), ori-gates (coverage matrix, modified tests, citation, prover), ori-orchestrator (lifecycle, lock table, escalation, closing rules; merge queue for tier 0 and 1 with a single human), ori-flows (init skeleton and readiness, migrate M0 and M2 document generation, divergence register), ori-watch, ori-mcp (server with the phase 1 tool subset, host), ori-integrations (traits, one vcs host reference adapter), ori-rpc (uds and named pipe), ori-engine, ori-cli.

**Phase set to sign before start.** The PRD section 4 catalog rows for the feature identifiers in Delivers above, API_SPEC, DATA_MODEL, LLD, TESTING, `criteria/phase-1.md`, the QA agent's test plan for it.

**Exit criteria.**
- `ori init` on `fixtures/new-product` produces the specification skeleton, identities, and refuses launch with the correct missing list.
- `ori migrate` on `fixtures/migrated-with-drift` produces M0 inventory and the divergence register, detects the inert workflow and the tracked env file, and turns on unattributed-change detection.
- A tier 0 ticket travels filed → merged with no human action; a tier 1 ticket stops for a human; a modified test escalates.
- The forbidden-action test passes for every phase 1 identity.
- Every gate has a proof.
- The author's SM Pronostic migration backlog is executed through the CLI for at least one full phase.
- Performance budgets for event throughput and retrieval latency are measured and recorded as baselines.

## Phase 2: The fleet

**Goal.** Parallel coders, a lead on a second model, real isolation, real permissions.

**Delivers.** L-04, L-06; M-03; F-01, F-02 (partition), F-03 (full), F-08, F-09; G-04; V-02, V-05, V-07, V-08, V-11; Q-07; P-08, P-09; K-08, K-09; C-01, C-04; E-01, E-02; I-02 (CI), I-06; containers, the credential broker complete with the forbidden-action test on every permission change, lock table at scale, merge queue complete, calibration records, vectors in memory, the CI slot reference adapter.

**Exit criteria.** Three coders on non-overlapping tickets complete in parallel; two on overlapping scope are serialized by the lock table; a coder attempting a forbidden action inside its container is refused and the refusal is an event; cross-model review is enforced (a lead on the same model as its coder is refused at identity creation); a calibration run produces medians and budgets for two models.

## Phase 3: Ori Studio

**Goal.** The desktop app: everything a human sees and touches.

**Delivers.** A-03, A-07, A-10, A-11, A-12; S-02, S-03, S-05; D-02 to D-07, D-14; L-02; M-07, M-08, M-09, M-12; T-08, T-09; F-04, F-07; G-06; V-01, V-09, V-10; P-06; C-07, C-08; N-01 to N-05; X-01 to X-03; I-02 (notification); R-01; Z-05; screens of PRD §6; the assistant role.

**Exit criteria.** Journeys J-01, J-03, J-04, J-05, J-07 executed end to end through the UI on all three platforms; Launch disabled with a correct missing list and enabled when complete; an unattributed edit from the terminal produces a notification, an incident and a merge block within two seconds; a tier 2 approval requires a separate session from plan approval; screenshot matrix green.

## Phase 4: Unattended

**Goal.** The daemon on a server or in CI running QA, operations and documentation agents; the remaining slots; go-live and migration completion flows.

**Delivers.** A-08; S-04; D-12; L-05; M-10, M-11, M-13; F-10; G-05; Q-01, Q-04 to Q-06, Q-08; O-01 to O-04, O-06; P-04 (export), P-07; C-02, C-03, C-05, C-06; E-03; I-02 (errors, analytics); the WebSocket transport with token authentication; scheduled triggers; liveness gate; the release pipeline as specified in CI_CD; runbooks rehearsed.

**Exit criteria.** Journey J-02 end to end on `fixtures/migrated-with-drift` through the migration exit checklist; a QA run triggered by a significant merge files findings without human involvement; the liveness gate fails loudly when a workflow is disabled; rollback rehearsed on staging.

## Phase 5: Self-migration

**Goal.** Ori Studio brought under AICD through itself.

**Delivers.** Ori Studio's own repository imported as a product; its specification (this folder) as canonical knowledge; its fleet running its own tickets; the dogfood test of PROJECT_BRIEF section 5 satisfied.

**Exit criteria.** From the end of this phase, every change to Ori Studio is a ticket worked by Ori Studio's own fleet, reviewed in Ori Studio's own review queue, merged by its own merge queue.

## Later (not scheduled)

Mobile, disconnected and regulated profiles as guided flows; team features; hosted daemon as a separate product; ACP-native assistant across products.

## Phase 1 detail

Ticket batches in dependency order. Each becomes several tickets with declared scope.

1. Workspace, toolchain, CI skeleton with the gates of CI_CD 1 to 3, 7 and 9, each proven on a planted defect.
2. ori-core: types, state machines with property tests, permission function, methodology reasons.
3. ori-store: event log with hash chain, projections, migrations, rebuild.
4. ori-broker: identities, keychain, forbidden-action test harness.
5. ori-runtime: worktrees, ACP client, one headless adapter, credential injection at spawn, budgets, transcripts, session recovery.
6. ori-memory: repository indexer (tantivy), code map (tree-sitter, initial languages: Rust, TypeScript, Python, Go), operational log, sanitization barrier with planted injection tests, scope enforcer, retrieval package, citation checker, drift audit.
7. ori-gates: coverage matrix, modified tests, significance, prover, liveness definition.
8. ori-orchestrator: lifecycle, lock table, escalation triggers, budgets, closing rules, merge queue (tier 0 and 1).
9. ori-watch: tree watcher, git hooks, attribution, unattributed changes, merge block.
10. ori-mcp: server tools for phase 1 roles, host.
11. ori-integrations: traits, vcs host reference adapter with installation tokens and webhooks.
12. ori-flows: init (skeleton, readiness), migrate (M0, M2 document generation through the documentation role, divergence register).
13. ori-rpc and ori-engine: method registry, transports, subscriptions, composition.
14. ori-cli: commands mirroring the Client API; JSON output for scripting.
15. Fixtures: the four products; end-to-end suite; performance baselines; calibration of budgets on the first model.
