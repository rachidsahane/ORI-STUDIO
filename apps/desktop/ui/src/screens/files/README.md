# Screen: files

Tree, viewer and diff over the repository, with Monaco in read-only mode
(`spec/LLD.md` section 3). Documents under `spec/` are edited in `../spec`, not
here.

The viewer shows repository content. It is not an editor, and adding write
actions here would route a change around the specification path and around the
attribution the watcher depends on (`spec/PRD.md` Z-01).

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
