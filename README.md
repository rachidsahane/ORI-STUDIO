# Ori Studio

An open source, local first desktop workspace and engine for running a software
product under AICD, a methodology in which AI agents build, test, operate and
document software, and humans specify, verify, decide and supervise.

Ori Studio is not an agent and not a code editor. It is meant to be the layer
above your agents: the thing that gives them roles, separated permissions,
tickets, risk tiers, gates and memory, and gives you one place to supervise
them. By design it drives the agent runtimes you already use, connects to the
tools you already have, uses your own model keys, and operates no server of its
own.

**None of that works yet.** This is phase 1 of five, and phase 1 has barely
started. What a clone gets you today is a specification, the methodology, and a
workspace of sixteen crates that compile and do nothing. Read
[Status](#status) before you go further.

## The problem

Existing agentic tools (Kiro, Cursor, Claude Code, Codex and the rest) have
converged on the same feature set: specifications, subagents, hooks,
automations. None of them has an organization layer. That layer is:

- roles with separated permissions, enforced by the tool rather than by a prompt
- risk tiers that decide what may merge without a human
- an escalation protocol with defined triggers
- a migration procedure for a product that already exists
- a memory that distinguishes what the user decided from what an agent inferred

Without it, AICD is applied by hand, and applying it by hand currently takes a
four week bootstrap per product: reverse engineer the codebase, write the agent
instruction files, install the CI gates, configure a ticket system, wire
observability, set up permissions. Every team that wants AICD does that again,
and gets it slightly wrong. The methodology's controls live in documents and
conventions that a tool could enforce and no tool does.

The argument is made in full in
[`spec/PROJECT_BRIEF.md`](spec/PROJECT_BRIEF.md) section 1.

## Who it is for

An engineer with system design experience who supervises AI agents building and
maintaining production software, and who does not write code by hand. In AICD's
terms, the architect, verification lead, reliability and governance, and product
owner seats. Today that is often one person holding all four, so the tool has to
work for one person and grow to a team of several without rework.

Secondarily: a product owner without an engineering background, who talks only
to the assistant and reads the product signal view; and an auditor or a new team
member who needs to read the specification, the decisions and the evidence
behind any change.

### Not for

Developers who want an AI assisted code editor. **Ori Studio does not edit code
and will not become an editor.** That sentence is
[`spec/PROJECT_BRIEF.md`](spec/PROJECT_BRIEF.md) section 2's, and section 6
lists code editing under "Out of scope for the first release, and non-goals"
with the note "Not in the first release, not ever." It is a non-goal and not a
deferral. Anyone who wants to hand edit generated code should use a different
tool.

## Status

The roadmap has five phases. This tree is in phase 1: the headless engine and
the CLI, with no UI. Phase 1 is not close to done. What follows is what a clone
actually contains today, not what the roadmap describes.

| | Today |
|---|---|
| Specification | Written. The registry in [`spec/README.md`](spec/README.md) lists 24 documents with an owner seat and a state. Most carry the state Draft, and phase 1 cannot launch until the foundation set is approved. |
| Methodology | Present: `methodology/AICD_Methodology_v0.3.html`, with a generated machine readable section index at `methodology/sections.json`. |
| Engine code | Sixteen crates under `crates/`, plus a desktop scaffold at `apps/desktop/`. Almost all of them are a `Cargo.toml` and one source file holding a doc comment that names what the crate will own and which AICD sections it implements, and nothing else. The exceptions are `ori-gates`, which contains the generator for the methodology section index and the checks built on it, and `ori-core`, which contains the domain types and the error enum every refusal carries. Which crates are more than a doc comment today is a question `wc -l crates/*/src/*.rs` answers, and that is the check to run rather than reading this row as current. |
| Tests | `ori-gates` and `ori-core` have them and no other crate does. `ori-gates`'s are all in `src/sections.rs` and cover the section index generator, the scan that resolves this repository's `AICD` citations against it, and the check that a doc comment naming a test names a test that exists. `ori-core`'s cover the domain types and the `MethodologyRef` error enum. The rest have none, because there is nothing in them yet to test. That is a reading of the tree on the day it was written, not a standing fact: `cargo test --workspace` prints a result line per crate and is the check to run rather than reading this row as current. |
| The `ori` binary | Builds. Its `main` is empty. |
| Desktop app | Directory structure and READMEs only. There is no Tauri dependency, no `package.json` and no UI code. The application is phase 3. |
| CI | `.github/workflows/ci.yml` runs on every pull request, on a push to `main` and in the merge queue: build, `fmt`, `clippy` and `test` across macOS, Windows and Linux, the supply-chain and commit-trailer checks, a planted-defect proof job for each of gates 1, 2, 7 and 13, and a `ci` job that fails unless every one of them succeeded. `ops/gates/gate-1.md` records one green run and one deliberately red one. That is not fourteen working gates. A gate counts as installed here only once a proof file records it failing on a planted defect, and `ops/gates/` holds those files: gates 1 (`fmt` and `clippy`), 2 (`cargo test`) and 13 (commit trailers) are recorded **Installed**, and gate 7 is recorded **partially installed, two checks of three**, because `cargo-audit` and `cargo-deny` are proven and its secret scan is not built and may not be cited as protection anywhere (escalation `ops/escalations/E-0003-secret-scan-implementation.md`). Every other gate has no proof file at all. The **State** line at the top of each file in `ops/gates/` is the check; this row is a reading of them and goes stale the day one changes. Nor does CI block anything yet, because `ci` is not a required status check and `main` requires none, so a red run marks the pull request and stops no merge; `ops/gates/branch-protection.md` records why. Locally, `scripts/gates.sh` still reports all fourteen and has a runner for gates 1, 2, 7 and 13. |

**One thing that will mislead you if nobody says it.** After
`cargo build --workspace`, the binary at `target/debug/ori` exists, runs, prints
nothing at all, and exits `0`. That is an empty `fn main()`, not a command that
succeeded. The CLI's commands are batch 14 of phase 1 in
[`spec/ROADMAP.md`](spec/ROADMAP.md); none of them is written. Do not read that
exit status as the tool working.

The phases, their deliverables and their exit criteria are in
[`spec/ROADMAP.md`](spec/ROADMAP.md), which is the authority. In one line:
headless engine and CLI, then the fleet, then the desktop app, then unattended
operation, then Ori Studio brought under AICD through itself.

## The shape it is being built into

```mermaid
flowchart TB
    H["You: the four AICD seats"]
    CLI["ori CLI (phase 1)"]
    APP["Desktop app (phase 3)"]
    ENG["Engine: tickets, categories, risk tiers,<br/>budgets, escalation, lock table,<br/>merge queue, gates runner, event log"]
    MEM["Memory service: four layers, code map,<br/>sanitization barrier, per-agent scopes"]
    BRK["Credential broker: one identity per agent,<br/>short-lived scoped credentials,<br/>forbidden-action test"]
    RT["Your agent runtimes,<br/>over ACP and headless adapters"]
    TOOL["Your tools, over MCP and APIs:<br/>version control host, CI, error tracking,<br/>analytics, notifications"]

    H --> CLI
    H --> APP
    CLI --> ENG
    APP --> ENG
    ENG --> MEM
    ENG --> BRK
    ENG --> RT
    ENG --> TOOL
    BRK -. "injected at spawn" .-> RT
```

The engine is the product; the CLI and the desktop app are thin clients of it,
and the same engine runs headless on a server or in CI. Source:
[`spec/PROJECT_BRIEF.md`](spec/PROJECT_BRIEF.md) section 3 and principle 1.

Inside Ori Studio, every one of those boxes is a skeleton today. The engine, the
memory service, the broker and the CLI are crates with no implementation in
them, and the desktop app is a directory tree with no application in it.

## What the interface is designed to be

Ori Studio has no interface yet. The two images below are its design
reference opened in a browser: a mockup of the desktop application phase 3
will build, not a program you can run today.

![Dark desktop window titled Ori Studio, on the "Dashboard by seat" tab. A left drawer lists four seats held by one person, Architect, Verification lead, Reliability and governance and Product owner, each with a count of items waiting on it. The main column carries an open incident banner, a strip reading queue depth 14, escalations 2, incidents 1, cost today $41 and matrix 96%, then one panel per seat holding only that seat's items: an ADR revisit condition and a blocked contract change for the architect, a tier 2 diff read and three proposed criteria for the verification lead, the incident and a passing forbidden-action test for reliability and governance. Each panel is footed with a line naming what is hidden from that seat. A right-hand assistant panel answers two questions about the fleet from operational memory and offers a decisional confirmation card with an accept button and a decline button.](docs/images/dashboard-by-seat.jpg)

Dashboard by seat. One person holds all four AICD seats here, so each panel
carries only what that seat decides and names what it is hiding. The
assistant answers from operational memory, cites the methodology section
behind a refusal, and puts a decisional change behind a confirmation instead
of making it.

![The same window on the "Fleet" tab. A table of six agent identities gives each one a ticket, a budget bar with elapsed time or attempt count, a model, GPT-5.1, Claude Opus, Gemini 3 Pro or Claude Sonnet, and an isolation mode of worktree or container: lead-01 is reviewing coder-02 with cross-model review enforced, coder-01 and qa-01 are running in containers, coder-02 sits at 100 percent of its budget, stopped and blocked on a contract change, and docs-01 is idle. Below the table, a lock table holds four scopes and records one refused start, a merge queue lists three tickets at tier 0, tier 1 and tier 2, and the broker reports the forbidden-action test passing on six identities out of six.](docs/images/fleet.jpg)

Fleet. Six agent identities, each with its ticket, its budget, its model and
an isolation mode of worktree or container, the lead reviewing a coder on a
different model. Below them, a lock table that refused a start because
another ticket had already claimed the scope, a merge queue in which the
tier 0 ticket is green, the tier 1 ticket awaits one human and the tier 2
ticket is held for a separate session, and the credential broker's
forbidden-action test, kept as evidence.

The reference is `spec/design/Ori Studio.html`, which is self contained and
opens in any browser from a clone.
[`spec/design/DESIGN.md`](spec/design/DESIGN.md) records it as DSN-001,
approved for layout and tone and expressly not a complete flow set, and its
section 6 lists fourteen further screens and flows that are still to design.
The mockup is React; Ori Studio's UI is SolidJS, so it is ported, not reused.

## Its relationship to AICD

AICD is a separate document with its own life, and Ori Studio implements it
rather than defining it.

A copy of the methodology ships inside this repository, at
`methodology/AICD_Methodology_v0.3.html`, so that agents and humans cite the
same text and a citation gate can check every `AICD §n` reference against the
generated index in `methodology/sections.json`. The index exists; the gate that
consumes it is gate 9 of [`spec/CI_CD.md`](spec/CI_CD.md) section 1, which is
specified, ticketed and not yet installed. Each release of Ori Studio names the
AICD version it is built under. This tree is built under v0.3.

The specification under `spec/` is the source of truth, and the code is its
build artifact. That is the operating rule of this repository rather than a
statement about documentation: a behavior the specification does not describe is
a change to the specification first, and a behavior the methodology does not
describe is a change proposal to the methodology first. It is also why `spec/`
is considerably fuller than `crates/`.

## Where to look

| Read this | For |
|---|---|
| [`spec/PROJECT_BRIEF.md`](spec/PROJECT_BRIEF.md) | The one document to read first. The problem, the users, the scope, the non-goals, the principles, the risks. Everything else derives from it. |
| [`spec/README.md`](spec/README.md) | The specification registry: every document, its set, its owner seat and its state. Start here to find out whether the document you want exists yet. |
| [`spec/ROADMAP.md`](spec/ROADMAP.md) | The five phases, what each delivers, and the exit criteria each has to meet. |
| [`spec/ARCHITECTURE.md`](spec/ARCHITECTURE.md) | The components and how they fit together. |
| [`CLAUDE.md`](CLAUDE.md) | How agents work in this repository: the rules every role obeys, the escalation triggers, and how a ticket is worked. Read it before sending an agent at this tree. |
| `methodology/AICD_Methodology_v0.3.html` | The methodology itself. |

## Building it today

You need git and rustup already installed; the setup script reports those two as
missing rather than installing them, and handles the rest itself.

```sh
git clone <this repository>
cd <the clone>
scripts/setup-dev.sh
cargo build --workspace
```

`scripts/setup-dev.sh` checks every requirement of
[`spec/ENV_SETUP.md`](spec/ENV_SETUP.md) section 1, installs what is missing and
what it can supply, and prints one line per requirement with an explicit state.
Some requirements are reported as `NOT YET`, meaning specified but not built,
each named with the ticket that will deliver it; the script counts those as
unmet and says so rather than passing over them, so a clean machine today ends
at `PASS WITH GAPS` and not at `PASS`. `scripts/setup-dev.sh --check-only`
verifies and installs nothing. `scripts/setup-dev.sh --help` lists the rest.

What `cargo build --workspace` produces is a workspace of crate skeletons that
compile. It is not a usable tool, and running the resulting binary does nothing,
as described under [Status](#status).

Before opening a pull request, run `scripts/gates.sh`. It reports a state for
every one of the fourteen gates in [`spec/CI_CD.md`](spec/CI_CD.md) section 1
and never omits one. Today it has a local runner for gates 1, 2, 7 and 13; for
the rest it prints the reason it has none, because a gate that is silently
skipped is indistinguishable from a gate that passed. Gate 7's runner is two
checks of three: its secret scan is reported blocked, never run and never
passed, which is escalation
[`E-0003`](ops/escalations/E-0003-secret-scan-implementation.md), and that one
blocked check is why the script exits `3`, incomplete, rather than `0` on a
clean tree. Which gates it can actually run is what the run itself prints, one
line per gate with an explicit state, and that is the check rather than this
paragraph. Every gate has a function in the script, and a function whose whole
job is to say why there is nothing to run looks like one that runs something
until you read the state it prints.

## Contributing

There is no contributor guide yet, and the project is not set up to take patches
from outside.

What exists is the process the project runs on itself, in
[`CLAUDE.md`](CLAUDE.md): changes arrive as tickets with a declared scope, a
plan, a risk tier and a closing report, worked by an agent fleet in isolated
worktrees, and specification changes go through a separate documentation role
rather than being edited in place. Reading that file is the fastest way to
understand what the product is trying to be, because the repository is run by
the rules the product is meant to enforce.

To propose something, read [`spec/PROJECT_BRIEF.md`](spec/PROJECT_BRIEF.md) and
[`spec/ROADMAP.md`](spec/ROADMAP.md) first and say which phase your proposal
belongs to. A proposal that contradicts the brief is a change to the brief, and
that is a conversation worth having on its own.

## License

Apache 2.0 for the software, in [`LICENSE`](LICENSE), which is the Apache text
unmodified. The methodology is CC BY 4.0 instead: that covers
`methodology/AICD_Methodology_v0.3.html` and the `methodology/sections.json`
generated from it. [`NOTICE`](NOTICE) is the authority on which licence covers
which path, and it carries the attribution the CC BY files ask for.

Two things here are unresolved, and neither is settled by the files above.
[`spec/PROJECT_BRIEF.md`](spec/PROJECT_BRIEF.md)'s header table states that the
name AICD is protected by a trademark policy; no such policy exists in this
repository or is linked from it. And the methodology HTML carries no licence
notice inside itself, so a copy of that file taken on its own travels with no
statement of its terms. Both are known gaps, not considered choices.
