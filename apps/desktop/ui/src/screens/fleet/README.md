# Screen: fleet

The agent sessions of this project: which identity, which ticket, which state,
which budget consumed. `spec/ARCHITECTURE.md` section 3 binds a window to one
project, so this screen never shows another project's sessions; AICD §26
requires that fleets share neither credentials nor context.

AICD §28 names "agent working memory and intermediate reasoning" first among
what is deliberately not shown, with its reason: "watching an agent think
invites intervention; the escalation protocol is the intervention path". A live
transcript view on this screen would be that mistake. Transcripts are evidence
attached to tickets, not a window into a running session.

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
