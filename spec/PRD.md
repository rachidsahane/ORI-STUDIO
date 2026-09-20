# PRD: Ori Studio

Derives from PROJECT_BRIEF. This document is the complete functional catalog of Ori Studio: every function the AICD methodology (v0.3) implies, organized by the methodology's own structure, with a priority, a delivery phase and the methodology section it implements. ROADMAP schedules these; acceptance criteria reference them by identifier.

Priority: P0 first release; P1 first release if possible; P2 later. Phase: ROADMAP phase.

## 1. Users and seats

| User | Seat(s) | In Ori Studio |
|---|---|---|
| Operator | Architect, verification lead, reliability and governance, product owner (one person today) | Everything below |
| Product person | Product owner | Framing, brief approval, product signals, evolution map, assistant chat |
| Reviewer, auditor, new team member | Any seat, read | Specification, decisions, evidence, audit trail |

## 2. The core promise

A person who has never run AICD opens Ori Studio, describes a product idea or points at an existing repository, and is walked to a running AICD fleet without reading the methodology first. The methodology is enforced by the tool, explained by the assistant, and visible in the dashboard. Agents work in auto mode: no permission prompts, ever; control is exercised through the methodology's control points only.

## 3. Operating principles that shape every function

- **Approval is one click.** Every "approval" in the methodology is an approval action by the owning seat inside Ori Studio, recorded as an event. Nothing manual, nothing cryptographic.
- **Auto mode.** Ori Studio holds full rights on a project. Each agent identity receives, at spawn and silently, exactly what its role needs (ENV_SETUP permission manifest). There are no interactive tool-permission dialogs. What prevents "rubbish" is the verification chain: role scopes, gates proven on planted defects, three-layer review, tiers, and the operator's decisions at control points.
- **One project, one engine, one window.** Several Ori Studio windows can be open, each on its own project, each with its own engine database, worktrees, containers and MCP sessions. Application-level configuration is shared across projects: the version control host connection and the model provider keys. A project may override a provider key.
- **Mermaid everywhere.** Every diagram the tool generates or displays (architecture, domain model, state machines, flows, maps) is Mermaid source, rendered in the UI and stored as text in the specification.
- **The methodology travels with the project.** The AICD methodology HTML ships in every project under `methodology/`, versioned, so agents and humans cite the same text; the citation gate validates against its section index.

## 4. Function catalog

### 4.1 Application and projects (AICD §26, §27)

| ID | Function | Priority | Phase |
|---|---|---|---|
| A-01 | Application-level configuration: version control host connection (one app-scoped identity), model providers with keys (stored in the OS keychain), default model per role, notification defaults, container runtime detection | P0 | 1 |
| A-02 | Project registry: create, import, open in a window, close, remove (never deletes the repository), list; one engine database per project | P0 | 1 |
| A-03 | Multiple windows and instances: each window binds to one project; opening a project already open focuses its window; single-writer lock per project database | P0 | 3 |
| A-04 | Per-project overrides: provider key, model per role, container backend, notification routes | P0 | 1 |
| A-05 | Per-project MCP sessions: the operator connects MCP servers to a project; the engine exposes each server to the roles the operator allows; sessions are never shared across projects | P0 | 1 |
| A-06 | Organizational repository: the layer 2 repository (methodology, role templates, policies, calibration, profiles) configured once and read by every project | P0 | 1 |
| A-07 | Portfolio dashboard: all open projects with queue depth, escalations, cost, incidents, agent states; one review queue across projects ordered by tier, then project priority, then age | P1 | 3 |
| A-08 | AICD version per project; refusal to open a project on a newer methodology version without the upgrade flow; upgrade flow applies adopted Change Proposals | P1 | 4 |
| A-09 | Methodology bundle in the project (`methodology/` with the HTML and the machine-readable section index), updated through the upgrade flow | P0 | 1 |
| A-10 | Explain-in-context: every refusal, disabled action, escalation and control has an "explain" action showing the methodology section and the reason in this instance | P1 | 3 |
| A-11 | Terminal per project for pairing mode, every command logged to the audit trail; edits outside `spec/` from the terminal trigger unattributed-change detection | P1 | 3 |
| A-12 | File tree and read-only file content viewer; diff viewer; `spec/` editable | P0 | 3 |

### 4.2 Seats and people (AICD §18, §19)

| ID | Function | Priority | Phase |
|---|---|---|---|
| S-01 | Seats per project: architect, verification lead, reliability and governance, product owner; one person may hold several; each approval names the seat and the person | P0 | 1 |
| S-02 | Dashboard views by seat (supervisor, controller, co-architect, product owner, reliability and governance) as the methodology's dashboard section defines, including what is deliberately hidden | P0 | 3 |
| S-03 | Review windows: configurable cadence; the inbox groups what waits for the window versus what interrupts | P0 | 3 |
| S-04 | Deliberate practice: scheduled monthly architecture review without AI, rotation of reviewers across projects, post-mortem ownership assignment; reminders and records | P2 | 4 |
| S-05 | Second-seat arrival: when a second human takes a seat, the single-operator exception ADR expires automatically and tier 2 reverts to two humans | P1 | 3 |

### 4.3 Specification system (AICD §9, §23, §32, §33)

| ID | Function | Priority | Phase |
|---|---|---|---|
| D-01 | Document registry: every document with kind, set (foundation, phase, migration), owner seat, state (missing, draft, under review, approved, stale), verified-against-code date | P0 | 1 |
| D-02 | Framing session (gate G0) in the assistant chat: challenge the idea, extract problem, user, success statement, non-goals, validation grid; ends in a framing record the operator confirms | P0 | 3 |
| D-03 | PROJECT_BRIEF generation from the framing record; view; request modifications in the chat; iterate; approve. Nothing else generated before the brief is approved | P0 | 3 |
| D-04 | Foundation set generation on request, one document at a time, each a review item; then the phase set for roadmap phase 1 | P0 | 3 |
| D-05 | Phase set generation for each later phase, just in time, requiring the previous phase closed | P0 | 3 |
| D-06 | Specification editor: Mermaid diagrams rendered inline; citation checker on save; every human edit becomes a specification PR | P0 | 3 |
| D-07 | Request-changes loop: the operator asks for modifications in the chat, the assistant produces a new draft, the diff is shown | P0 | 3 |
| D-08 | Spec anchors: tickets reference a document and section; the UI resolves and shows them | P0 | 1 |
| D-09 | Drift audit (weekly and on demand): documentation agent compares canonical documents with code, updates verification dates, files divergences; stale documents block readiness | P0 | 1 |
| D-10 | Templates shipped: every document kind, ticket, PR report, ADR, blocked report, post-mortem, escalation, Change Proposal, criterion, persona; editable at organizational level | P0 | 1 |
| D-11 | Agent instruction files as three layers (organizational base, product base, role file), generated from templates, versioned, tested by the forbidden-action test and re-calibrated on change | P0 | 1 |
| D-12 | Design artifacts: `design/` with stable identifiers, referenced from PRD journeys and criteria, verified by the QA agent (structure, visual tolerance, accessibility, breakpoints); design changes typed as behavioral or decisional | P1 | 4 |
| D-13 | ADR management: create, inherited status, escalation scope per ADR feeding the lead's triggers, revisit conditions, supersession | P0 | 1 |
| D-14 | Stack ADR flow: requirements, options, decision, cost, revisit conditions, generated with the assistant per project (AICD §20) | P0 | 3 |
| D-15 | Load-bearing paths: the product base instruction file names paths an agent must never infer as dead, with evidence; the migration flow populates it | P0 | 1 |

### 4.4 Readiness and launch (AICD §23)

| ID | Function | Priority | Phase |
|---|---|---|---|
| L-01 | Readiness computation: every foundation document approved, phase 1 set approved, security notes name tier 2 modules, permission manifest complete, risk map covers every module, identities creatable, integrations for required slots configured | P0 | 1 |
| L-02 | Launch button enabled only when ready; readiness panel lists exactly what is lacking with a link to each item | P0 | 3 |
| L-03 | Launch: identities created, credentials issued and forbidden-action test executed, memory loaded, gates proven on planted defects, ticket queue seeded from the phase's criteria, G4 trivial-ticket test run and shown | P0 | 1 |
| L-04 | Phase control: start requires the phase set approved and the previous phase closed; close requires every criterion covered, matrix complete, exhaustive QA run with no open Auto or Behavioral tickets; exploration feedback in product language becomes tickets | P0 | 2 |
| L-05 | Go-live checklist (G6): tag, QA run on the candidate, promotion of the same tag, observability confirmed live, rollback rehearsed; items tick from evidence only | P1 | 4 |
| L-06 | Steady-state entry (G7): strictest tiers for the first period; loosening only through a governance decision recorded as an ADR | P0 | 2 |

### 4.5 Migration of an existing product (AICD §24)

| ID | Function | Priority | Phase |
|---|---|---|---|
| M-01 | Import and origin detection: documents present or not, tests present or not, stack, deployment configuration; migration plan proposed | P0 | 1 |
| M-02 | M0 freeze and snapshot: freeze declared, baseline tag created (its existence verified), inventory produced (repositories, branches with unmerged work, environments, services, data stores, queues, jobs, third parties, domains, certificates, credential locations, humans with access), unattributed-change detection on | P0 | 1 |
| M-03 | M1 access attribution: seat assignment; credential inventory; revocation of credentials for integrations that no longer exist (before rotation); rotation checklist; per-agent identities with migration-time permissions; console steps as a checklist with exact steps; each verified by attempting the forbidden action | P0 | 2 |
| M-04 | Env report: every env file, tracked or ignored, history presence, key names, value length and entropy class, classification per key; never a value in any output; secret scan of tree and history | P0 | 1 |
| M-05 | M2 as-built specification: the fourteen documents in order, each generated by the documentation agent and a review item for its owner seat; as-built first, intended second | P0 | 1 |
| M-06 | Divergence register: one entry per disagreement between intended and as-built; each resolved only by a human decision (code is right, intent updates; or intent is right, ticket created) | P0 | 1 |
| M-07 | Inherited ADRs generated from decisions embedded in code, with revisit conditions | P0 | 3 |
| M-08 | Risk map derived from data classification, security map and debt register; every module tiered | P0 | 3 |
| M-09 | M3 verification baseline: characterization criteria per journey; existing test audit and mapping; baseline measurements (matrix completeness, mutation score, security findings, load) recorded to calibration; test environment configured; first exhaustive QA run | P0 | 3 |
| M-10 | M4 observability onboarding in read-only mode; categorization calibration period before automatic validation | P0 | 4 |
| M-11 | M5 fleet activation with three autonomy steps (shadow, tier discipline, steady state), each gated on evidence the engine computes | P0 | 4 |
| M-12 | Migration exit checklist ticking from evidence; migration panel showing phases M0 to M5 with progress | P0 | 3 |
| M-13 | Special cases handled as plan variants: shared codebase (shared component documented once, changes decisional for every product), team of coders (no hybrid period), no tests (longer M3, shadow until threshold), inherited stack (never a rewrite) | P1 | 4 |
| M-14 | Identifier aggregation control: inventory documents cite identifiers by name; identifiers live in a separate document with its own memory scope | P0 | 1 |

### 4.6 Tickets (AICD §11)

| ID | Function | Priority | Phase |
|---|---|---|---|
| T-01 | Ticket lifecycle: filed, categorized, validated, queued, in progress, blocked, escalated, in review, merged, deployed, closed | P0 | 1 |
| T-02 | Four categories with automatic validation for Auto and Behavioral (once calibrated), human approval for Decisional, product owner routing for Product signal | P0 | 1 |
| T-03 | Upgrade-only rule: agents raise category, only humans lower it | P0 | 1 |
| T-04 | Mandatory contents: category, kind, tier, spec anchor, evidence for defects, criteria for features, budget, filed-by, declared scope | P0 | 1 |
| T-05 | Closing rules: no close without spec update or explicit no-change; defects need an accepted criterion | P0 | 1 |
| T-06 | Precondition verification: every precondition a ticket names (a tag, a file, a service) is checked to exist at approval time, not execution time | P0 | 1 |
| T-07 | Destructive ticket protocol: any ticket that deletes, drops, revokes or rewrites carries a belief and its source, runs a discovery stage that changes nothing, and stops for a human decision before execution | P0 | 1 |
| T-08 | Ticket board with the five views (review queue, coder queue, blocked, decisional awaiting approval, product signals) | P0 | 3 |
| T-09 | Feedback in product language from the chat becomes tickets with proposed category | P0 | 3 |

### 4.7 Fleet, pairing and escalation (AICD §12)

| ID | Function | Priority | Phase |
|---|---|---|---|
| F-01 | Fleet mode: lead pulls the validated queue, checks declared scope against the lock table, assigns coders, reviews, escalates or approves | P0 | 2 |
| F-02 | Lock table of modules claimed by in-flight tickets; refusal of overlapping starts; re-declaration mid-ticket as an escalation trigger; lead partition by product area at volume | P0 | 1 (table), 2 (partition) |
| F-03 | Merge queue: rebase, re-run gates, tier check, merge; the only merge path | P0 | 1 (tier 0 and 1), 2 (full) |
| F-04 | Pairing mode: the operator drives a runtime directly from the terminal on a ticket; every action logged; reserved for decisional, escalated and new architecture work | P1 | 3 |
| F-05 | Budgets per ticket (attempts, wall clock, tokens) enforced by the runtime; at the limit the coder stops and writes a blocked report; never grinds | P0 | 1 |
| F-06 | Escalation triggers: ADR area, modified test, new dependency, spec conflict, scope conflict, contract change, security concern, precondition missing; each opens an escalation with recommendation and context package; escalations land in the human queue at the review cadence, incidents interrupt | P0 | 1 |
| F-07 | Fleet view: identities with state, ticket, budget used, worktree, container, model; live; collapsible | P0 | 3 |
| F-08 | Session transcripts viewable; sessions killable with reason; kill revokes credentials first, terminates second, releases locks third | P0 | 2 |
| F-09 | Cross-model enforcement: a lead on the same model as its coders is refused; model fallback on a tier 2 review is an escalation | P0 | 2 |
| F-10 | Unattended agents (QA, operations, documentation, product signal) launched by the daemon on triggers and schedules with the same runtime | P0 | 4 |

### 4.8 Git and release (AICD §13)

| ID | Function | Priority | Phase |
|---|---|---|---|
| G-01 | Protected main, one branch per ticket, no long-lived branches, rebase before ready; enforced through the host adapter and the merge queue | P0 | 1 |
| G-02 | Commit trailers (ticket, spec anchor) generated for agents and validated by a gate | P0 | 1 |
| G-03 | PR report generated from the plan, tests, coverage matrix, not-tested list, tier, rollback plan | P0 | 1 |
| G-04 | Tiers: tier 0 auto-merge after lead approval and green gates; tier 1 one human; tier 2 two humans (or the single-operator substitutes, recorded) and one human reads the diff | P0 | 2 |
| G-05 | Tags per deploy, changelog generation, rollback as previous-tag redeploy, migrations backward compatible with the previous tag | P0 | 4 |
| G-06 | Traceability: any line to commit to ticket to spec section to decision, navigable in the UI | P1 | 3 |

### 4.9 Verification (AICD §14, §7)

| ID | Function | Priority | Phase |
|---|---|---|---|
| V-01 | Acceptance criteria authoring with the assistant: structured (id, type, precondition, action, expected, tier), per phase, in `criteria/` | P0 | 3 |
| V-02 | Test plan by the QA agent: one row per criterion, level, test name; human verification of the plan before build | P0 | 2 |
| V-03 | Coverage matrix gate: criterion to test mapping enforced in CI; unmapped tests flagged; posted on the PR | P0 | 1 |
| V-04 | Modified-test detector: any change to an existing test escalates whatever the tier | P0 | 1 |
| V-05 | Mutation testing with a threshold that only rises | P0 | 2 |
| V-06 | Gate proving: no gate installed until it fails on a planted defect; proof stored; citation gate refuses to cite unproven gates | P0 | 1 |
| V-07 | Three-layer review: deterministic checks (static analysis, structural diff summary, contract checks, dependency and secret scans), adversarial model review through the checklist, human review by tier | P0 | 2 |
| V-08 | Adversarial checklist as a versioned policy, grown from post-mortems | P0 | 2 |
| V-09 | Evidence review panel: coverage matrix, gate runs, mutation delta, checklist result, modified tests, diff for tier 2, in one screen | P0 | 3 |
| V-10 | Separate-session rule for tier 2 under the single-operator profile: diff read in a different session from plan approval, with a minimum delay | P0 | 3 |
| V-11 | Test type vocabulary available to plans: unit, integration, end-to-end, security, load, chaos, simulation, property-based and fuzzing, contract, mutation | P0 | 2 |

### 4.10 Continuous test environment (AICD §15, §31)

| ID | Function | Priority | Phase |
|---|---|---|---|
| Q-01 | Staging orchestration through the CI slot and the runtime; seed and anonymization runbooks executed by operations, verified by QA | P0 | 4 |
| Q-02 | Event-driven cadence: significance labeler on every merge from category and declared scope; exhaustive run on significant merges, cheap checks on schedule; never a run for cosmetic changes | P0 | 1 (label), 4 (runs) |
| Q-03 | Significant-modification list as organizational policy, extensible per project, calibrated backwards from production defects | P0 | 1 |
| Q-04 | Run budgets (parallel runs, wall clock, tokens) and saturation stop | P0 | 4 |
| Q-05 | Persona derivation from analytics segments: segment, characterize, review, version in `criteria/personas.md`; quarterly regeneration | P1 | 4 |
| Q-06 | Finding quality: fingerprinting, reproducibility gate, severity by user impact, known-issue suppression and regression reopen, flaky list | P0 | 4 |
| Q-07 | Proposed criteria from findings in `proposed` state until the verification lead accepts; acceptance rate tracked | P0 | 2 |
| Q-08 | Scoped penetration and chaos activity: written scope, logged, never leaving the environment | P1 | 4 |

### 4.11 Operations and observability (AICD §16)

| ID | Function | Priority | Phase |
|---|---|---|---|
| O-01 | Three watchers: operations (health against SLOs), QA (correctness), product signal (usage), each filing its own tickets | P0 | 4 |
| O-02 | SLO definitions per project from OBSERVABILITY; incident on breach; incidents interrupt | P0 | 4 |
| O-03 | Runbooks: registry, authorization per agent, rehearsal requirement before installation, execution logging; re-enabling is human | P0 | 4 |
| O-04 | Post-mortems: human-owned, written with AI, recorded in operational memory, producing criteria, runbook updates or ADRs; loop closure enforced by the closing rules | P0 | 4 |
| O-05 | Untrusted input rule: everything from telemetry presented as quoted, typed, provenance-tagged data | P0 | 1 |
| O-06 | Liveness gate: fails loudly when an expected workflow or agent run has not happened inside its window; inert-but-present is a failure state | P0 | 4 |

### 4.12 Permissions and security (AICD §17, §27)

| ID | Function | Priority | Phase |
|---|---|---|---|
| P-01 | One identity per agent role per project; short-lived scoped credentials issued at spawn by the broker, revoked at session end; no agent holds more than its role | P0 | 1 |
| P-02 | Auto mode: no interactive permission prompts for agents; runtimes launched in their non-interactive mode inside the isolation boundary | P0 | 1 |
| P-03 | Forbidden-action test on launch, on every permission change, on every release; results shown and stored | P0 | 1 |
| P-04 | Immutable, hash-chained audit trail of every human and agent action; search; export for a time window in auditor-readable form | P0 | 1 (trail), 4 (export) |
| P-05 | Secrets architecture: OS keychain only; owners, rotation periods and last-rotated dates visible; a secret appearing anywhere it should not is an incident with rotation first | P0 | 1 |
| P-06 | Data classification per project (personal, financial, credentials, content, aggregation) governing test-environment copies, provider routing and tier 2 modules | P0 | 3 |
| P-07 | Provider routing by classification: which data may be sent to which model provider; anonymized or synthetic data only where required | P1 | 4 |
| P-08 | Threat-model controls: injection through telemetry and dependencies, compromised identity containment, supply chain (dependency audit, new dependency escalation, license check), insider (dual approval, audited permission changes), provider (agnostic layer, calibration re-run, data handling), agent drift (cross-model, human diff, monthly review, mutation) | P0 | 2 |
| P-09 | Autonomy loosening as governance: any widening of a role's permissions or lowering of a tier is a decisional ticket producing an ADR | P0 | 2 |

### 4.13 Memory (AICD §8, §25)

| ID | Function | Priority | Phase |
|---|---|---|---|
| K-01 | Four layers: canonical (git, per project), organizational (git, shared), operational (append-only structured records), working (ephemeral per session, promoted only through reports) | P0 | 1 |
| K-02 | Repository indexer: full-text, document graph (sections, ADRs, criteria, modules), re-index on merge | P0 | 1 |
| K-03 | Code map (tree-sitter): modules, interfaces, dependency edges, entry points, covering tests, spec sections per module | P0 | 1 |
| K-04 | Sanitization barrier: typed records only, length caps, provenance and untrusted flags, raw evidence stored apart and never indexed, planted-injection tests | P0 | 1 |
| K-05 | Scope enforcement per identity at query time; refusals logged | P0 | 1 |
| K-06 | Retrieval API: bounded, task-oriented context packages with provenance and verification date; inspectable by the operator | P0 | 1 |
| K-07 | Freshness tracker and staleness exposure; drift audit integration | P0 | 1 |
| K-08 | Vector index and embeddings through the user's provider (phase 2) | P1 | 2 |
| K-09 | Multi-project layout: one organizational index, per-project partitions, cross-project retrieval only for the assistant and the organizational drift audit | P1 | 2 |
| K-10 | Rebuild from repository plus event log; export of operational memory to `ops/` as structured files | P0 | 1 |

### 4.14 Calibration and economics (AICD §21, §30)

| ID | Function | Priority | Phase |
|---|---|---|---|
| C-01 | Calibration sets per category with known outcomes; runs on model adoption or change; medians recorded; budgets as multiples | P1 | 2 |
| C-02 | Categorization calibration: blind comparison of agent versus human categories; dangerous disagreement rate; automatic validation switched on only below the threshold | P0 | 4 |
| C-03 | Escalation trigger calibration from outcomes (agreed, disagreed, unnecessary, missed) | P1 | 4 |
| C-04 | Retrieval quality calibration with a question set; blocks memory changes on regression | P1 | 2 |
| C-05 | Significance trigger calibration backwards from production defects | P1 | 4 |
| C-06 | Criteria acceptance rate and finding noise rate | P1 | 4 |
| C-07 | Metrics tracked per project per month: tokens and cost per ticket by category, cost per merged PR, test environment cost, memory cost, human hours per approved ticket, escalation rate, blocked rate and attempts, validated-to-deployed time, defects in production versus environment, mutation score and matrix completeness; drafting-error rate | P1 | 3 |
| C-08 | Cost view against calibrated expectation | P1 | 3 |

### 4.15 Evolution of the methodology (AICD §29)

| ID | Function | Priority | Phase |
|---|---|---|---|
| E-01 | Parameter changes recorded in organizational knowledge with owner seat; re-measured after model change | P1 | 2 |
| E-02 | Tooling ADRs at organizational level | P1 | 2 |
| E-03 | Change Proposals: authoring from the template, review by seats, adoption with a version increment; a product's methodology version upgrade applies adopted proposals | P2 | 4 |

### 4.16 Notifications and chat (AICD §28)

| ID | Function | Priority | Phase |
|---|---|---|---|
| N-01 | Routing rules: interrupt (incidents, unattributed change, inert gate, credential exposure), review window (escalations, blocked, PR approvals, QA summaries, drift), digest (twice daily); desktop notifications plus the notification slot | P0 | 3 |
| N-02 | Assistant chat as the front door: framing, document generation and modification, "what is the fleet doing", explain, file a ticket, explore a new feature with options and trade-offs | P0 | 3 |
| N-03 | Confirmation cards: escalations, decisional tickets, tier approvals, category downgrades, readiness questions, migration decisions, divergence decisions; answered from the card; every answer audited | P0 | 3 |
| N-04 | Report cards: closing, blocked, QA run, drift audit, incident, post-mortem, calibration | P0 | 3 |
| N-05 | Inbox with the same cards, filtered by route | P0 | 3 |

### 4.17 Map and evolution

| ID | Function | Priority | Phase |
|---|---|---|---|
| X-01 | Project map: roadmap phases with progress (criteria covered, tickets closed, what shipped) for new products; migration phases M0 to M5 then the roadmap for transitioned products; Mermaid rendering | P0 | 3 |
| X-02 | Evolution timeline: versions, significant modifications, incidents, ADRs, methodology version changes | P0 | 3 |
| X-03 | Traceability navigation from any line, commit, ticket, criterion or decision to its neighbors | P1 | 3 |

### 4.18 Integrations (PROJECT_BRIEF principle 12, AICD §7)

| ID | Function | Priority | Phase |
|---|---|---|---|
| I-01 | Version control host slot at application level with app-scoped identities; repositories, branches, PRs, checks, webhooks; one reference adapter | P0 | 1 |
| I-02 | Error tracking slot (read), analytics slot (read), notification slot, CI slot; one reference adapter each; configured per project | P0 (CI, notify), P1 (errors, analytics) | 2, 3, 4 |
| I-03 | MCP host: connect any MCP server to a project; expose to roles by policy; MCP server toward agents with the scoped tool set | P0 | 1 |
| I-04 | Agent runtime slot: ACP client; headless CLI adapters; runtimes launched in auto (non-interactive) mode inside isolation | P0 | 1 |
| I-05 | Model provider slot: keys at application level, per-project override; routing per role and per data classification | P0 | 1 |
| I-06 | Integration health per slot, degraded banners, nothing blocked except merges needing a CI status from that slot | P0 | 2 |

### 4.19 Profiles (AICD §35 to §38)

| ID | Function | Priority | Phase |
|---|---|---|---|
| R-01 | Single-operator profile: enabled automatically when one human holds every seat; the five substitutes enforced (independent model review with veto, separate-session diff read with delay, rollback rehearsed on staging, backward-compatible flagged migrations, limited rollout where possible); expiring ADR | P0 | 3 |
| R-02 | Mobile profile: release trains, kill switches and remote flags as a tier 2 rule, compatibility window ADR with two-way contract tests, device matrix, staged rollout with automatic halt, store-policy decisional items, upgrade tests, per-version SLOs | P2 | later |
| R-03 | Disconnected profile: diagnostic bundle ingestion, local simulation mirror, lagged loop | P2 | later |
| R-04 | Regulated profile: evidence mapping export per control family, tier 2 for regulated data, provider routing by classification, retention rules, accountable-human naming | P2 | later |

### 4.20 Lessons encoded as controls (AICD §39)

| ID | Function | Priority | Phase |
|---|---|---|---|
| Z-01 | Unattributed change detection: any change outside `spec/` not attributed to a session on a ticket raises an unattributed-change event, opens an incident with the diff, blocks merges from the branch until resolved as an exception ticket or a revert; covers disk edits, commits and pushes; notification routing arrives with N-01 in phase 3 | P0 | 1 |
| Z-02 | "Present but reporting nothing" as a named defect class: gate proving, liveness, and the rule that every artifact citing a gate names its trigger | P0 | 1 |
| Z-03 | Citation and reference integrity gate over specification, operational memory and instruction files, against the methodology's machine-readable section index; drafting-error rate recorded | P0 | 1 |
| Z-04 | Revocation as a migration step distinct from rotation; env report covers backup and archive files | P0 | 1 |
| Z-05 | No-way-back deploy paths detected during migration (no down-migration, no snapshot) are tier 2 and block agent autonomy on migrations until a rehearsed snapshot and restore exist | P0 | 3 |
| Z-06 | Load-bearing paths named with evidence in the product base instruction file, populated by the migration flow | P0 | 1 |

## 5. Journeys

Journeys are what acceptance criteria and the end-to-end suite exercise.

- **J-01 New product, idea to Launch:** A-02 → D-02 → D-03 → D-04 → L-01, L-02 → L-03 → T-08, F-07.
- **J-02 Existing product, import to steady state:** M-01 → M-02 → M-03, M-04 → M-05 to M-08 → M-09 → M-10 → M-11 → M-12.
- **J-03 Daily supervision:** S-02 → T-08 → N-03 → N-04 → X-01.
- **J-04 An escalation:** F-06 → N-03 → decision recorded → ticket continues.
- **J-05 A manual edit:** Z-01 end to end, resolution as exception or revert.
- **J-06 A new feature on a running product:** N-02 → D-06 (spec PR) → D-05 → T-01 → F-01.
- **J-07 A tier 2 review:** V-09 → V-10 → G-04 → merge.
- **J-08 A significant merge:** Q-02 → Q-04 → Q-06 → Q-07 → T-01.
- **J-09 An incident:** O-02 → O-03 → O-04 → loop closure.
- **J-10 Two projects in two windows:** A-03 → A-05 → A-07, with no state shared except application-level configuration.

## 6. Screen inventory

Home (projects, portfolio), Project dashboard (by seat), Fleet, Tickets (five views), Inbox, Map and evolution, Specification (registry, editor, approvals), Files (tree, viewer, diff), Terminal, Chat (with cards), Readiness, Migration, Calibration and cost, Audit trail, Settings (application: host connection, providers, defaults; project: overrides, MCP servers, integrations, seats, routes, classification).

## 7. Non-functional requirements

- Engine operations without a third party complete offline.
- Fleet view and dashboard update within two seconds of an engine event (baseline to measure in phase 1).
- Negligible idle CPU; startup to dashboard under three seconds.
- Any project's methodology-defined state is rebuildable from its repository plus its event log.
- Every action by a human or an agent is in the audit trail with identity, ticket, time, input and output.
- Several windows on several projects with no cross-project state except application-level configuration.

## 8. Out of scope

See PROJECT_BRIEF section 6: no code editing, no hosted service, no bundled runtime or model, no user telemetry to the project.
