# Screen: chat

The assistant surface, with cards. The assistant identity reads what the
operator can read and writes nothing: `spec/ENV_SETUP.md` section 5 gives it no
tokens and section 6 makes "write any file" its forbidden action.

So a card in this screen offers an action the operator takes; the assistant does
not take it.

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
