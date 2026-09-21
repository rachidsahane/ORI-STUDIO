# Screen: settings

Two scopes, kept visibly apart because `spec/ARCHITECTURE.md` section 3 makes
them different scopes with different storage:

- Application, shared by every window: host connection, provider keys, default
  model per role, notification defaults, container runtime, organizational
  repository path.
- Project, this window only: provider key and model overrides, MCP servers and
  their role exposure, integration slots, seats, notification routes, data
  classification.

No secret value is ever displayed, logged or round-tripped through this screen.
`spec/ENV_SETUP.md` section 4 keeps secrets in the OS keychain and lists where
they may not appear; the screen shows a name, an owner, a scope and a rotation
state, and hands the value to the keychain without reading it back.

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
