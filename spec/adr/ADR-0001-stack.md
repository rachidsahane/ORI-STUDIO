# ADR-0001: Technology stack of the Ori Studio

| | |
|---|---|
| Status | Accepted |
| Date | September 2026 |
| Deciders | Architect (Alim Sahane) |
| Methodology | AICD section 20 (stack selection policy), section 23 (foundation set) |
| Escalation scope | Any change to a technology named in "Decision" is decisional and tier 2 |

## Context

Ori Studio is an open-source, local-first desktop application with an engine that must also run headless on servers and in CI. It manages processes (agent runtimes, containers, terminals), git repositories, embedded search and vector indexes, a durable event log, and OS keychains, across macOS, Windows and Linux. Performance and a single static binary per platform are requirements from PROJECT_BRIEF section 7. The team is one operator directing AI agents, so the stack must also be one that agents implement well, which favors mature, heavily documented technologies.

## Options considered

### Engine language
- **Rust.** Native performance, single static binary, first-class libraries for every component needed (PTY, git, SQLite, full-text, vectors, tree-sitter, keychain, containers), memory safety across long-running daemons, same language as the desktop shell's backend. Slower iteration than Go or TypeScript.
- **Go.** Fast iteration, good for daemons, weaker embedded search and tree-sitter story, would split the codebase from the desktop shell.
- **TypeScript on Node.** One language end to end with the UI, largest agent-tooling ecosystem, but a heavy runtime for a daemon, weaker isolation, and every native dependency becomes a binding.

### Desktop shell
- **Tauri 2.** Rust backend in-process, system webview, small binaries, official plugins for shell, notifications, updater, and OS integration. Webview differences across platforms require testing on each.
- **Electron.** Most mature, bundles Chromium (consistent rendering), large binaries, a second runtime beside the engine.
- **Eclipse Theia.** A framework for building IDE-like workbenches with terminals, file trees and Monaco already integrated. Electron-based, large, and pulls the product toward being an editor, which is a non-goal.

### UI framework
- **SolidJS.** Fine-grained reactivity, no virtual DOM, among the fastest of the mainstream frameworks, full access to Monaco and xterm bindings.
- **React.** Broadest ecosystem, slower under the update rates of a live fleet view.
- **Svelte 5.** Comparable performance to Solid, slightly smaller ecosystem for editor components.
- **Dioxus or Leptos (Rust to WebAssembly).** One language everywhere, but Monaco and xterm still require JavaScript interop, so the cost is paid without removing the boundary.

### Storage and indexes
- **SQLite** for all durable local state versus an embedded key-value store: SQLite wins on tooling, inspectability and transactions.
- **Full-text:** tantivy versus SQLite FTS5. Both embedded; tantivy is faster at scale, FTS5 is zero extra dependency.
- **Vectors:** sqlite-vec, usearch, LanceDB. All embedded; sqlite-vec keeps everything in one file.
- **Code map:** tree-sitter, the only serious option for many languages with one API.

### Agent isolation
- git worktrees only; worktrees plus containers (Docker or Podman); microVMs. Containers give real permission boundaries at acceptable cost; microVMs are heavier than a desktop tool should require by default.

### Agent protocol
- ACP (Agent Client Protocol) as the primary interface; headless CLI adapters as fallback; a bundled runtime (rejected by PROJECT_BRIEF principle 5).

### Model family
- **Exact model id.** Precise, but a version bump or a renamed snapshot of the same model reads as a different model, and the separation is lost without anyone noticing.
- **Family identifier.** The provider's own grouping of models sharing a lineage; stable across version changes, and the coarsest unit at which two reviewers are genuinely different. Chosen, because AICD §7 separates a lead from its coders at exactly this level.
- **Provider.** Wrong in both directions: one provider serves several families, and two providers can serve the same family.

### Inter-process interface
- JSON-RPC 2.0 over a local socket; gRPC; a REST server. JSON-RPC is the simplest to expose to both the desktop UI and the CLI, and the same shape ACP and MCP use.

## Decision

| Component | Choice |
|---|---|
| Engine, CLI, all backend logic | Rust, stable toolchain, Cargo workspace, one static binary per platform |
| Desktop shell | Tauri 2, engine linked in-process, engine also runnable as a daemon with the same API |
| UI | SolidJS with TypeScript, Vite; Monaco (read-only except `spec/`) for files and diffs; xterm.js for the terminal |
| Durable state | SQLite (per product, one file), WAL mode, schema migrations versioned |
| Full-text index | tantivy |
| Vector index | sqlite-vec (phase 2), embeddings through the user's own provider |
| Code map | tree-sitter with per-language grammars loaded on demand |
| Event log | Append-only table in SQLite; every state change is an event; state is a projection |
| Agent isolation | git worktrees always; containers by default for coder agents (Docker or Podman, user's choice); worktree-only as a documented downgrade |
| Agent protocol | ACP client; headless CLI adapter trait for runtimes without ACP |
| Model family | The unit of cross-model separation: the provider's model family identifier, declared by each runtime adapter through its `RuntimeCaps` and recorded on the `ProviderBinding`; `broker.identity.create` refuses a lead identity whose family equals that of the coders it reviews |
| Credentials | OS keychain through a Rust keyring library; per-agent short-lived tokens; version control host through app installation tokens |
| Inter-process interface | JSON-RPC 2.0 over Unix domain socket or Windows named pipe; WebSocket transport for headless remote (phase 4) |
| MCP | Official MCP semantics: the engine is an MCP host for the user's servers and an MCP server toward agents |
| Packaging and updates | Tauri bundler; approved releases; per-OS channels: macOS (dmg, Homebrew), Windows (msi, winget), Linux (AppImage, deb, rpm, Flatpak); Tauri updater for in-app updates |
| License | Apache 2.0 |

## Consequences

Easier: one language for everything behind the UI; small, fast binaries; every component embedded, so the offline-capable requirement is met by construction; the same engine runs in the app, the CLI and CI.

Harder: Rust compile times and a stricter learning curve for contributors; webview differences across platforms require a test matrix; containers require Docker or Podman installed, so the worktree-only downgrade must be robust; JavaScript remains present for Monaco and xterm.

What the agents must respect: no new runtime dependency (no Node in the engine, no Python), no database server, no network call from the engine except to user-configured providers and integrations, no bundled model or agent, and any new crate that touches credentials, processes or the merge path is tier 2. A lead identity and the coders it reviews never share a model family, and the check is enforced at identity creation, not at review time.

## Revisit conditions

A Rust-native editor and terminal component of Monaco and xterm quality reaching maturity (would allow dropping JavaScript); ACP adoption stalling (would promote the headless adapter to primary); Tauri webview inconsistencies proving unmanageable (would reopen Electron).
