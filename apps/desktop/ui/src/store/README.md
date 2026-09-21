# ui/src/store

Solid stores, fed by engine events. `spec/LLD.md` section 3: "`store/` (Solid
stores fed by events)".

## What belongs here

One store per domain the UI shows (tickets, sessions, escalations, gates,
documents, readiness, and so on), each subscribing to the engine event stream
forwarded from the Tauri backend through the Tauri event system, and each
exposing read-only accessors to screens.

## The rule that shapes it

The stores are **fed by events**, not by polling and not by re-reading after
every action. A screen action calls `../rpc`; the engine appends its events; the
store updates when the event arrives. That is the same direction as the engine
itself: `spec/LLD.md` section 5 has projections updating from the log, and
`CLAUDE.md` states the invariant the UI inherits, that projections are derived
and a wrong projection is fixed in the projector, never in the log.

So a store never writes a value it wished for and waits for the engine to agree.
An optimistic local edit is a second source of truth, and the screen would show
state the event log does not contain.

## What does not belong here

- RPC calls. Stores consume events; screens call `../rpc`.
- Rendering, markup, or anything that imports from `../components`.
- Derived values expensive enough to need a cache of their own. If a projection
  is needed, it belongs in the engine, where the CLI gets it too.

## Tier

Tier 1 under `spec/RISK_MAP.md`: a store subscribes to the engine.
