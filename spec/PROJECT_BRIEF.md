# PROJECT_BRIEF: Ori Studio

| | |
|---|---|
| Name | Ori Studio. In Yoruba tradition, Ori is not an external god but the personal divinity that lives in each person's head: the essence of consciousness, intellect and concentration. Ori Studio is where that faculty directs the work of the agents. |
| Status | Foundation document, gate G1 of AICD section 23 |
| Version | 0.1, September 2026 |
| Owner | Product owner and architect: Alim Sahane |
| Methodology | Built under AICD v0.3. This product is the reference implementation of the methodology and will be the second product migrated into it. |
| License | Apache 2.0 for the software. The AICD methodology document is CC BY 4.0. The name AICD is protected by a trademark policy. |

This is the one document every human reads in full before anything else exists. Everything else in the specification derives from it. If a later document contradicts this one, this one is corrected first, deliberately, and the other regenerated.

## 1. The problem

AICD replaces the traditional software development workflow with one in which AI agents build, test, operate and document software, and humans specify, verify, decide and supervise. The methodology works, and applying it on a live product has already caught errors that would have destroyed that product.

But applying it today takes a four-week bootstrap: a human runs an "implementor" agent session that reverse-engineers the codebase, writes agent instruction files, installs CI gates, configures a ticket system, wires observability, sets up permissions, and hands the human a long list of manual steps. Every team that wants AICD has to do this again, by hand, and will get it slightly wrong. The methodology's controls (separation of duties, risk tiers, escalation, scoped permissions, memory) live in documents and conventions that a tool could enforce but no tool does.

Existing agentic tools (Kiro, Cursor, Claude Code, Codex and the others) have converged on specs, subagents, hooks and automations. None of them has an organization layer: roles with separated permissions, risk tiers, an escalation protocol, a migration procedure for existing products, or a memory that distinguishes what the user decided from what an agent inferred. They are agents. AICD needs a workspace that directs agents.

## 2. Who it is for

**Primary user.** An engineer with system design experience who supervises AI agents building and maintaining production software, and who does not write code by hand. In AICD terms: the architect, verification lead, reliability and governance, and product owner seats. Today that is one person holding all four; the tool must work for one person and grow to a team of several without rework.

**Secondary users.** A product owner without an engineering background who talks only to the assistant and reads the product signal view. An auditor or new team member who needs to read the specification, the decisions and the evidence for any change.

**Not for.** Developers who want an AI-assisted code editor. Ori Studio does not edit code and will not become an editor. A user who wants to hand-edit generated code should use another tool.

## 3. What it is

An open-source, local-first desktop workspace that runs a software product under the AICD methodology. It contains the engine that enforces the methodology (tickets, categories, risk tiers, budgets, escalation, the lock table and merge queue, the gates runner), the memory service (the four memory layers, the code map, the sanitization barrier, per-agent scopes), and the credential broker (one identity per agent, short-lived scoped credentials, the forbidden-action test). It drives the user's own coding agent runtimes through open protocols, connects to the user's own tools through MCP and APIs, and uses the user's own model API keys. It ships the methodology as defaults: the agent role templates, the specification templates, the guided flows for starting a new product and for migrating an existing one, and the profiles.

The human's surface is a dashboard organized by seat, a review queue, a specification editor, an escalation inbox, an optional live view of the agent fleet, a file viewer with content and diffs, a terminal for pairing mode, and an assistant to talk to. The assistant is the front door; the fleet is visible when the user wants it and invisible when they do not.

## 4. Product principles

These follow from the methodology and are not negotiable in the design.

1. **The engine is the product.** The desktop app and the CLI are thin clients of one engine that also runs headless on a server or in CI. Nothing that matters lives only in the UI.
2. **Humans edit the specification and nothing else.** Files under the specification folder are editable. Every other file is view-only in Ori Studio. The terminal is a logged escape hatch, not a workaround.
3. **Bring your own everything.** The user's model keys, agent runtimes, repositories, observability, analytics, notification and CI tools. Ori Studio ships adapters and defaults for none of them; it ships slots. No key ever leaves the user's machine to a server Ori Studio operates, because Ori Studio operates no server.
4. **Controls are enforced by the tool, not by prompts.** Permissions are enforced by the credential broker and process isolation. Tiers are enforced by the merge gate. Scopes are enforced by the memory service. An instruction file is documentation of a control, never the control.
5. **Open protocols only.** Agent runtimes through the Agent Client Protocol, tools through the Model Context Protocol, instruction conventions the runtimes already read. Ori Studio never requires a runtime or a provider of its own.
6. **Local-first, offline-capable core.** The engine, memory, tickets and gates work with no network. Network is needed only for the user's own providers and integrations.
7. **The methodology document is the source of truth for the tool.** The tool implements a stated AICD version. A behavior the methodology does not describe is a change proposal to the methodology first.
8. **Auto mode.** Agents never see an interactive permission prompt. Ori Studio holds full rights on a project; each agent identity receives, silently at spawn, exactly what its role needs and nothing else. What prevents bad output is the verification chain (scopes, proven gates, three-layer review, tiers, the operator's decisions at control points), not dialogs.
9. **One project per window, shared application configuration.** Several windows may be open on several projects, each with its own engine database, worktrees, containers and MCP sessions. The version control host connection and the model provider keys are configured once at application level; a project may override a provider key.
10. **Mermaid everywhere.** Every diagram, generated or displayed, is Mermaid source stored as text in the specification.
11. **The methodology travels with the project.** The AICD methodology HTML ships in every project under `methodology/`, so agents and humans cite the same text and the citation gate validates against its section index.
12. **Tool-agnostic integration categories.** The methodology needs a version control host, an error tracker, an analytics source, a notification channel and a CI runner. Each is an adapter slot. Ori Studio ships reference adapters for the first of each so the flows can be demonstrated, and treats every slot as replaceable by the user.

## 5. Definition of success

Ori Studio succeeds when a product can be brought under AICD, and operated under it, without an implementor session and without hand-written wiring. Measured as:

- **The bootstrap test.** A live product with existing documentation is migrated into AICD through Ori Studio's guided migration flow (phases M0 to M5) in under one day of the operator's active time, with every artifact of the methodology's migration exit checklist produced, and no manual wiring outside the tool except entering credentials in the user's own third-party consoles.
- **The dogfood test.** Ori Studio itself is developed under AICD, through Ori Studio, from the end of its second roadmap phase onward.
- **The enforcement test.** The forbidden-action test (every agent attempts the one action its role forbids) passes from inside the tool, on every supported platform, on every release.
- **The exit-criterion test.** Every gate Ori Studio installs demonstrates failure on a planted defect before it is reported as installed.
- **Adoption.** Within twelve months of the first public release, at least ten products outside the author's own are operated under AICD through Ori Studio, with their operators contributing at least one accepted Change Proposal to the methodology between them.

## 6. Scope

### In scope for the first release

- The engine: orchestrator (ticket lifecycle, four categories, three risk tiers, budgets, blocked reports, escalation triggers, lock table, merge queue), gates runner, calibration records, event log.
- The memory service: canonical and organizational layers from git, operational memory as an append-only store, working memory per task, the code map, the sanitization barrier, scoped retrieval per agent identity, the citation checker, the drift audit.
- The credential broker: per-agent identities, short-lived scoped credentials, injection at process spawn, the forbidden-action test.
- The agent runtime: spawning coder, lead, QA, operations, documentation and assistant agents in isolated worktrees or containers, through ACP and headless adapters, on the user's chosen runtimes and models, with cross-model review enforced.
- The guided flows: new product (gates G0 to G7) and migration (phases M0 to M5), producing the specification set and the as-built documents.
- Ori Studio UI: dashboard by seat, review queue with coverage matrix and evidence, specification editor, escalation inbox, fleet view, file tree with file content and diffs, terminal, assistant chat, notification routing.
- Integration slots with one reference adapter each: version control host, error tracking, analytics, notifications, CI.
- The CLI, exposing the same engine headlessly.
- Distribution for macOS, Windows and Linux, through the standard channels of each.

### Out of scope for the first release, and non-goals

- **Code editing.** Not in the first release, not ever. This is a non-goal, not a deferral.
- **A hosted service.** No cloud, no accounts, no telemetry sent to the project. A hosted engine for unattended agents may come later as a separate product.
- **Shipping models, agents or credits.** Ori Studio never resells inference and never bundles a runtime.
- **Team collaboration features** (multi-user real time, shared cloud state). The first release is one operator per project window; the artifacts are in git and shared through it.
- **Mobile clients** of Ori Studio itself.
- **The mobile, disconnected and regulated profiles** of the methodology as guided flows. The profiles are documented; the tool supports them through configuration in a later release.
- **Being a general agent orchestration framework.** Ori Studio implements AICD. Configurability exists where the methodology has parameters and nowhere else.

## 7. Fixed technical constraints

Recorded here because they are decided; the stack ADR gives the reasoning and the alternatives considered.

- Engine, CLI and all backend logic in **Rust**, one static binary per platform.
- Desktop shell **Tauri 2**, the engine running in-process for the desktop app and as a daemon for headless use, with the same JSON-RPC interface for both.
- UI in **SolidJS** with TypeScript, rendered by the Tauri webview; Monaco in read-only mode for file content and diffs, editable only for the specification folder; xterm.js for the terminal.
- Embedded storage only: **SQLite** for the event log, tickets, operational memory and calibration; embedded full-text and vector indexes; **tree-sitter** for the code map. No database server, no external index service.
- Agent isolation by git worktree always, and by container by default for coder agents.
- Agent protocol **ACP** first, headless CLI adapters second.
- Version control host identity through installation-scoped, short-lived app tokens, never personal tokens.
- Every release approved and distributed through each platform's standard channels (at minimum: a package manager and a direct installer per OS), from one release pipeline.
- Offline-capable core: every feature in section 6 that does not inherently need a third party works without network.

## 8. Key risks

| Risk | Why it matters | How the brief answers it |
|---|---|---|
| The engine is a serious backend for a small team | Phase 1 carries nearly all the technical risk | Phase 1 is headless and ships before any UI; the CLI is the first usable product |
| Agent runtimes and protocols are moving fast | An adapter written today may be obsolete in months | ACP as the primary interface, adapters isolated behind one trait, no runtime bundled |
| Differentiation erodes as large tools add features | Any single feature will be copied | The differentiation is the organization layer and the methodology; the document stays the source of truth |
| Enforcement gaps make the controls decorative | A permission model that lives in prompts is the failure AICD exists to prevent | Principle 4; the enforcement test in the definition of success; containers by default |
| The methodology changes while the tool is built | The tool must implement a stated version | Each release names its AICD version; methodology changes go through Change Proposals; the tool's roadmap follows adopted proposals |
| Single operator | Tier 2 requires two humans | The single-operator profile of the methodology applies to this project until a second seat exists |

## 9. Framing session answers

Recorded from gate G0, as the methodology requires.

- **Who has this problem?** Anyone who has built software with AI agents and now has to maintain it with a team, and any team that wants agents to do the building without losing control of what runs in production.
- **What already solves it?** Nothing solves the organization layer. Agentic IDEs and CLIs solve the agent layer and are getting better at it every release; they are inputs to this product, not competitors.
- **What makes this different?** It implements a complete, documented methodology with enforced controls, and it migrates existing products, which no agentic tool addresses.
- **What would make it fail?** Building the UI before the engine; bundling a runtime or a provider; letting configurability grow until it is a framework instead of a methodology; not dogfooding it.
- **What is the smallest version that proves the idea?** The CLI running the author's own migration backlog end to end without an implementor session.

## 10. Roadmap

The phases, their deliverables and exit criteria are defined in `ROADMAP.md`, which is the authority. In one line: headless engine and CLI, then the fleet, then Ori Studio, then unattended operation, then self-migration.

## 11. Terms used

AICD terms are defined in the methodology's glossary and are not redefined here. The word "studio" or "Ori Studio" in this document means the whole product; "engine" means the Rust core shared by the desktop app and the CLI; "seat" means one of the four human roles of AICD section 18.
