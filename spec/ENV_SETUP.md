# ENV_SETUP and PERMISSIONS: Ori Studio

Every credential and environment the product needs, and which identity holds which credential at which scope (the permission manifest, AICD §17 and §23). Secrets are never in this file; only their names, owners and scopes.

## 1. Development environment

| Requirement | Purpose | Notes |
|---|---|---|
| Rust stable (pinned in `rust-toolchain.toml`) | Engine, CLI, desktop backend | `cargo`, `clippy`, `rustfmt` |
| Node LTS + pnpm | UI build only | Never a runtime dependency of the engine |
| Tauri 2 prerequisites per OS | Desktop build | WebView2 on Windows, WebKitGTK on Linux |
| Docker or Podman | Container isolation for coder agents | Optional; worktree-only downgrade without it |
| git | Everything | Hooks installed by the watcher |
| `cargo-mutants`, `cargo-audit`, `cargo-deny` | Gates | Installed by `scripts/setup-dev.sh` |
| tree-sitter grammars | Code map | Fetched at build for the supported language set; others on demand |
| Fonts and system libraries for the UI test matrix | Screenshot tests | Per OS in `scripts/` |

`scripts/setup-dev.sh` installs everything above and runs the forbidden-action test against a fixture product; a fresh clone must pass it before any work.

## 2. Environments

| Environment | What runs | Who deploys |
|---|---|---|
| Local | Desktop app, CLI, engine, agent sessions, fixtures | Operator and coder agents (their worktrees) |
| Staging (continuous test environment) | The engine headless against the fixture products, the UI in the screenshot matrix, the QA agent runs | Operations agent from tags |
| Production | Released binaries on users' machines | Release pipeline from tags, approved; there is no server-side production |

## 3. Configuration scopes

| Scope | Items |
|---|---|
| Application (all windows, all projects) | Host connection (`VCS_APP_*`), provider keys (`PROVIDER_<NAME>_API_KEY`), default model per role, notification defaults, container runtime, organizational repository path |
| Project (one window) | Provider key overrides, model per role overrides, MCP servers and their role exposure, integration slots (error tracking, analytics, notification, CI), seats, notification routes, data classification |

## 4. Secrets inventory (names only)

| Secret name | Owner seat | Held by | Scope | Rotation |
|---|---|---|---|---|
| `VCS_APP_ID`, `VCS_APP_PRIVATE_KEY` | Reliability and governance | Engine keychain | Issue installation tokens per repository | Yearly, or on any exposure |
| `PROVIDER_<NAME>_API_KEY` (one per model provider the operator uses) | Operator | Application keychain; optional per-project override entry | Used by the runtime per session, per role model | Operator's policy |
| `MCP_<SERVER>_*` (per project) | Operator | Project keychain entries | Exposed to that project's engine only | Operator's policy |
| `CI_TOKEN` | Reliability and governance | CI secrets store | Release pipeline, checks | Yearly |
| `SIGNING_KEY_<OS>` | Reliability and governance | CI secrets store, hardware-backed where possible | Release signing | Per platform policy |
| `UPDATER_PRIVATE_KEY` | Reliability and governance | CI secrets store | Signing update manifests | Yearly |
| `NOTIFY_<ADAPTER>_SECRET` | Operator | Engine keychain | Notification slot | Operator's policy |
| `ERRORS_<ADAPTER>_TOKEN`, `ANALYTICS_<ADAPTER>_TOKEN` | Operator | Engine keychain | Read-only scopes only | Operator's policy |
| `ENGINE_CLIENT_TOKEN` | Engine | Keychain | Local client authentication | Per installation |

Rules: no secret in the repository, in `.env` files tracked by git, in fixtures, in tests, in logs, in events, in tickets or in reports. The secret scan gate covers tree and history.

## 5. Permission manifest per identity

Enforced by the broker (credential scopes), the orchestrator (ticket and merge rules), the memory scope enforcer, the MCP tool scopes and the runtime (container mounts). Any deviation is a decisional ticket and an update to this table.

All identities run in auto mode: no interactive prompts; the row is what they receive at spawn.

| Identity | Repository | CI | Staging | Production (users' machines) | Tickets | Specification | Memory scope | Credentials received |
|---|---|---|---|---|---|---|---|---|
| coder | Read; write own worktree and branch | Trigger and read for own branch | Deploy own branch to the fixture environment | None | Read own; plan and report | Read | Canonical, organizational, operational filtered to declared scope, code map | Installation token scoped to own branch; provider key for its model, per session |
| lead | Read all; approve tier 0 merges for the merge queue to perform | Read | Read | None | Read, assign, upgrade category, escalate | Read | All layers for the product plus organizational | Installation token scoped to review; provider key for its model; no merge credential |
| qa | None | Read; trigger QA runs | Read, write | None | Create, read | Read; propose criteria | Canonical, criteria, operational defects, code map (migration only: code read) | Staging credentials; error tracking read; analytics read; provider key |
| operations | None | Read | Read, write | Release pipeline trigger from tags (phase 4) | Create incident, read | Read runbooks | Runbooks, incidents, infrastructure ADRs | Staging credentials; CI trigger; provider key |
| documentation | Read; write `spec/` branches | None | None | None | Read | Propose through PR | Canonical, organizational, merged diffs, code map | Installation token scoped to spec branches; provider key |
| product_signal | None | None | None | None | Create product signal | Read product brief | Analytics summaries, brief | Analytics read; provider key |
| assistant | Read | None | None | None | Read; file on operator's request | Draft only | Everything the operator can read | Provider key; no tokens |
| implementor (bootstrap only) | Read; write `spec/`, `ops/`, `.github/`, `templates/`, `scripts/aicd/` on a bootstrap branch | None | None | None | Create backlog | Write on bootstrap branch | All | Installation token scoped to the bootstrap branch |
| operator (human) | Everything | Everything | Everything | Approves releases | Everything | Everything | Everything | Owns the keychain |

## 6. Forbidden actions (the test, AICD §23 G3)

One per identity, attempted at launch, on every permission change, and in the release pipeline; each must be refused and the refusal recorded:

| Identity | Forbidden action attempted |
|---|---|
| coder | Push to `main` |
| lead | Merge any pull request |
| qa | Write a file in the repository |
| operations | Modify a source file |
| documentation | Write outside `spec/` |
| product_signal | Create a ticket in a category other than product signal |
| assistant | Write any file |
| any | Read the keychain; read `.env*`; call a network host not in the integration configuration |

## 7. Migration-time permissions

When a product is imported, identities start with the stricter migration column of AICD §24.2: documentation and assistant read all; qa reads code for characterization only; operations reads telemetry only; lead and coders are not instantiated until M5. Ori Studio enforces this by not creating those identities before phase M5 starts.
