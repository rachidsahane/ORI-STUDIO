# Screen: terminal

xterm.js connected to the `terminal.*` RPC family. `spec/LLD.md` section 3 is
explicit about the reason input goes through RPC rather than to a local pty:
"input is sent through RPC so it is logged".

That is the load-bearing property of this screen. A change here that sent
keystrokes anywhere but through `terminal.*` would produce exactly the
unattributed change `spec/PRD.md` Z-01 and the `ori-watch` crate exist to catch,
and `spec/ROADMAP.md` phase 3 has an exit criterion for it: an unattributed edit
from the terminal produces a notification, an incident and a merge block within
two seconds.

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
