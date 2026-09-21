# Screen: tickets

Five views over the ticket queue. Every ticket carries its category and tier
(AICD §11), its plan and declared scope, its report, its coverage matrix and its
gate results.

Category and tier are never decoration: AICD §11 makes category upgrade-only,
and `spec/RISK_MAP.md` plus lead ruling R11 decide the tier from the declared
scope. The screen displays what the engine decided; it does not compute either
one.

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
