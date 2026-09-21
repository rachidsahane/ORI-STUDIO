# Screen: project dashboard

Per project, by seat. `spec/PRD.md` section 6 calls it "Project dashboard (by
seat)"; AICD §28 gives the rows, one group per human function: queue depth by
category, tickets in progress with time in state, blocked items, escalations
waiting, pull requests awaiting approval by tier, cost this month against the
calibrated expectation, deployment state and the last rollback test.

AICD §28 also forbids three things on this surface, and they are easiest to add
here: agent working memory and intermediate reasoning, raw production telemetry,
and vanity metrics (lines produced, commits, tickets closed without their
category and tier). A count of closed tickets without its category and tier
breakdown is a vanity metric by §28's own wording.

## What belongs here

The route, the layout and the components only this screen uses. Anything a
second screen needs moves to `../../components`.

## What does not

Engine access. This screen reads from `../../store` and calls `../../rpc`; it
does not call `invoke`, `fetch` or a Tauri event listener of its own
(`spec/ARCHITECTURE.md` section 4: the UI has no private path into the engine).

## Tier

`spec/RISK_MAP.md`: tier 0 for styling and layout, tier 1 for anything that
sends an RPC.
