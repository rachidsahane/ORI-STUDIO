# Screen: specification

Three surfaces over `spec/`: the document registry, the editor, and the
approvals.

AICD §9 is the anchor for the first two. It fixes "the same set of documents"
for every product with an owner each, and it requires that "every document
carries a 'last verified against code' date maintained by the drift audit":
that pair is the registry. §9 also carries the rule the editor has to respect,
"specification changes are pull requests, reviewed and merged under the same
risk tiers as code". §9 says nothing about an approval step, so the approvals
surface is anchored in AICD §23 instead, whose gate sequence is what is being
approved, and in `spec/PRD.md` section 6, which names the three surfaces.

This is the one place in the application where Monaco is editable.
`spec/LLD.md` section 3: "Monaco is loaded lazily; read-only everywhere except
documents under `spec/`." Everything else that shows a file uses the read-only
viewer in `../files`.

An edit here is a specification change and follows the specification path;
`spec/CONVENTIONS.md` has humans editing `spec/` only through Ori Studio or a
specification PR, and agents only through the documentation role.

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
