//! Desktop backend of Ori Studio: AICD §28, AICD §26.
//!
//! `spec/LLD.md` section 1 places this crate at `apps/desktop/src-tauri` and
//! gives it one job: host the engine and link `ori-engine`. `spec/LLD.md`
//! section 3 narrows it further, to Tauri commands that are thin wrappers
//! forwarding to `Engine::call`, one command per RPC method family, with engine
//! events forwarded to the webview.
//!
//! `apps/desktop` is not listed in the `spec/ARCHITECTURE.md` section 2
//! component table, so its sections are derived from what the two documents say
//! it is, the same way `ori-engine` and `ori-cli` derived theirs.
//!
//! AICD §28, "the fleet dashboard", is the substance of this crate rather than
//! a layer beneath it: §28 opens with "humans supervise through a dashboard,
//! not through terminals", and this application is that dashboard. §28 is the
//! authority both for what the surface shows (a row group per human function)
//! and for what it must refuse to show (agent working memory and
//! intermediate reasoning, raw production telemetry, vanity metrics). A command
//! added here that exposed any of the second list would break §28, which is why
//! `spec/ARCHITECTURE.md` section 4 requires that the webview reach the engine
//! only through commands wrapping the same JSON-RPC methods: the UI has no
//! private path into the engine, so §28's boundary is enforced in one place.
//!
//! AICD §26 is the authority for the window model. §26 requires "one fleet
//! instance per product, with its own agent identities, memory scopes, ticket
//! queue and continuous test environment", and that "fleets never share
//! credentials or context". `spec/ARCHITECTURE.md` section 3 realizes that rule
//! here: one engine per open project, a window bound to exactly one project,
//! several projects meaning several windows each with its own engine, database,
//! worktrees, containers, MCP sessions and agent identities.
//!
//! # State of this crate
//!
//! Scaffold only, per `ops/phase-1-backlog.md` ticket ORI-T-0002 and the phase 1
//! goal in `spec/ROADMAP.md`, which is explicitly "No UI". The desktop
//! application is phase 3. This crate therefore carries the shape and the
//! dependency edge and nothing else: no Tauri dependency, no commands, no
//! window, no runtime behavior. Tauri 2 arrives with the ticket that builds the
//! application; `spec/CONVENTIONS.md` makes adding a crate an escalation
//! trigger, and pulling Tauri's dependency tree into a phase whose goal is "No
//! UI" would buy nothing this phase can verify.
//!
//! ```mermaid
//! flowchart LR
//!   WEBVIEW[webview: apps/desktop/ui, SolidJS] -->|Tauri commands, phase 3| DESK[apps/desktop/src-tauri]
//!   DESK -->|Engine::call| ENG[ori-engine]
//!   DESK -->|Engine::subscribe| ENG
//!   ENG -->|events| DESK
//!   DESK -->|Tauri event system, phase 3| WEBVIEW
//! ```

// The edge `apps/desktop -> ori-engine` of the `spec/LLD.md` section 2 diagram
// is declared in `Cargo.toml` and made real here, so that the dependency is a
// fact of the compiled crate and not only of the manifest. The commands that
// will use `Engine` are phase 3; see the module below.
use ori_engine as _;

/// Tauri commands: thin wrappers over `Engine::call`, one per RPC method family.
///
/// Empty in phase 1. `spec/RISK_MAP.md` tiers `apps/desktop/src-tauri (commands)`
/// at 1, "thin wrappers", and that tier holds only while they stay thin: a
/// command may validate its arguments and forward, and may not decide anything.
/// Every authority check, every refusal and every `MethodologyRef` belongs to
/// the engine, because `spec/ARCHITECTURE.md` section 4 requires that the CLI
/// and the UI reach the same methods, so a rule enforced here would be a rule
/// the CLI does not have.
pub mod commands {}
