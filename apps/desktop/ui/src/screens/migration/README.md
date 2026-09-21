# Screen: migration

The migration flow: phases M0 to M5 (AICD §24.1 to §24.6), the divergence
register, and the migration exit checklist (AICD §24.7).

AICD §24.2 restricts identities during migration, and `spec/ENV_SETUP.md`
section 7 says Ori Studio enforces it by not creating lead and coder identities
before M5 starts. This screen shows that state rather than offering actions the
engine will refuse.

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
