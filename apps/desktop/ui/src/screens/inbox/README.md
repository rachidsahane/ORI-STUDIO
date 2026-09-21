# Screen: inbox

The single review queue. AICD §12 puts blocked and escalated items into a human
review queue processed at a fixed cadence, and AICD §26 makes it one queue
across products, ordered by risk tier first, then by product priority, then by
age, with incidents jumping the queue.

AICD §12 gives each item's shape: "each escalation is a question addressed to a
human, with the lead agent's recommendation attached". The recommendation is
shown as a recommendation, never as a default the human confirms by inertia.

What is shown alongside it comes from AICD §28's co-architect row, not from §12,
which does not mention a context package: "escalations with their context
package, the ADR touched, the lead agent's question and recommendation".

AICD §16 carries the one exception to the cadence: incidents are the only thing
that interrupts a human outside it.

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
