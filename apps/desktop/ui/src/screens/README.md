# ui/src/screens

One directory per screen. `spec/LLD.md` section 3: "Structure by screen".

A screen directory owns its route, its layout and the components only it uses.
Anything a second screen needs moves to `../components`. A screen never talks to
the engine directly: it reads from `../store` and calls `../rpc`.

## Where this directory list comes from, and what it is not

The directory list here follows `spec/LLD.md` section 3, because LLD is the
authority on layout. **It is not the authoritative screen set.** The
authoritative screen set for phase 3 is `spec/design/DESIGN.md`, which governs
that question when phase 3 reaches it.

The three documents do not currently agree, and the divergence is recorded
rather than resolved by this ticket: `spec/LLD.md` section 3 names twelve
screens, `spec/PRD.md` section 6 names fifteen (the twelve plus Home, Calibration
and cost, and Audit trail), and `spec/design/DESIGN.md` section 1 describes the
reference as covering eighteen. Reconciling them is a specification question for
the documentation role; the lead is carrying it to a specification pull request.
So this ticket created exactly the twelve directories LLD section 3 names, added
none of its own, and removed none. A reader who takes the twelve rows below as
the screen set has read this directory for something it does not decide; see
`../../README.md`.

| Directory | Screen | What it shows | Anchor |
|---|---|---|---|
| `dashboard/` | Project dashboard, by seat | Queue depth by category, tickets in progress with time in state, blocked items, escalations waiting, pull requests awaiting approval by tier, cost this month against the calibrated expectation, deployment state, last rollback test | AICD §28 supervisor row; PRD section 6 |
| `fleet/` | Fleet | The agent sessions of this project and their state. Not their working memory or intermediate reasoning: AICD §28 lists that first among what is deliberately not shown | AICD §28; PRD section 6 |
| `tickets/` | Tickets, five views | Tickets by state, category and tier, with their plans, reports and coverage matrices | AICD §11; PRD section 6 |
| `inbox/` | Inbox | The one review queue: escalations with their context package, tier 1 and 2 approvals, blocked reports. Ordered by risk tier, then product priority, then age; incidents jump the queue | AICD §12 (the queue and its cadence), §26 (one queue, its ordering), §28 (escalations shown with their context package), §16 (incidents interrupt); PRD section 6 |
| `map/` | Map and evolution | The product map and how it changed | PRD section 6 |
| `spec/` | Specification: registry, editor, approvals | The documents under `spec/`, their owner and last-verified date, and the only editable Monaco surface in the application | AICD §9 (the document set, the last-verified date, the pull request rule); AICD §23 (what approval approves); LLD section 3; PRD section 6 |
| `files/` | Files: tree, viewer, diff | Repository content and diffs, read-only Monaco | LLD section 3; PRD section 6 |
| `terminal/` | Terminal | xterm.js over the `terminal.*` RPC. Input goes through RPC so that it is logged and attributable | LLD section 3; PRD Z-01 |
| `chat/` | Chat, with cards | The assistant surface. The assistant writes no files (`spec/ENV_SETUP.md` section 6) | PRD section 6 |
| `settings/` | Settings | Application scope (host connection, providers, defaults) and project scope (overrides, MCP servers, integrations, seats, routes, classification), kept visibly separate because `spec/ARCHITECTURE.md` section 3 makes them different scopes | ARCHITECTURE section 3; PRD section 6 |
| `readiness/` | Readiness | The readiness computation and the missing list that disables Launch | AICD §23 (the gates G0 to G7 and their exit criteria; the feature words are the product's, see that directory's README); ROADMAP phase 3; PRD section 6 |
| `migration/` | Migration | The M0 to M5 phases, the divergence register, the migration exit checklist | AICD §24, §24.7; PRD section 6 |
