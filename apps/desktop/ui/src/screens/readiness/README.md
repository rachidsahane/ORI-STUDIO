# Screen: readiness

The readiness computation and the missing list. `spec/ROADMAP.md` phase 3 states
the behavior as an exit criterion: Launch disabled with a correct missing list,
and enabled when the list is complete.

The missing list is the screen's real content. A disabled button with no reason
is the failure this screen exists to prevent.

**On the anchor.** The words "readiness", "Launch" and "missing list" are this
product's, from `spec/PRD.md` section 6 and `spec/ROADMAP.md`; AICD §23 uses
none of them. What §23 does supply is the thing being computed: a new product
follows a fixed sequence of gates G0 to G7, each with a stated exit criterion,
and "nothing is built before the specification exists". Readiness is those exit
criteria evaluated, and the missing list is the ones not yet met. Cite §23 for
the gate sequence and its exit criteria, not for a readiness feature it does not
describe.

## What belongs here

The route, the layout and the components only this screen uses. Anything a
second screen needs moves to `../../components`.

## What does not

Engine access. This screen reads from `../../store` and calls `../../rpc`; it
does not call `invoke`, `fetch` or a Tauri event listener of its own
(`spec/ARCHITECTURE.md` section 4: the UI has no private path into the engine).

The computation itself. Readiness is decided by the engine, where the CLI reads
the same answer; a screen that decided which items count would give the UI a
rule the CLI does not have.

## Tier

`spec/RISK_MAP.md`: tier 0 for styling and layout, tier 1 for anything that
sends an RPC.
