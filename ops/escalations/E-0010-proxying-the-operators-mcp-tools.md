# Escalation E-0010: may the MCP server proxy the operator's own MCP servers' tools to roles?

| Field | Value |
|---|---|
| Trigger | `spec_conflict` |
| Ticket and anchor | ORI-T-0057, slice S6 (the proxy and ORI-P1-038), `ops/batch-10-plan.md`; `spec/LLD.md` section 2 (`ori-mcp` row), `spec/API_SPEC.md` section 3 |
| Raised by | The batch 10 planning coder (Sonnet 5.5, [[R39]]), 2026-10-11; recorded by the lead, who checked the cited lines |
| Question | May the MCP server expose the tools of the operator's own MCP servers to the roles the operator names, given that `spec/LLD.md`'s `ori-mcp` row forbids exposing "a tool not in API_SPEC" and `spec/API_SPEC.md` section 3 forbids giving agents file-write tools to `spec/` or `ops/`? |
| Recommendation | Yes, under four conditions. (1) Read LLD's "must not" as bounding Ori's own eleven tools. (2) Add a paragraph to API_SPEC section 3. Proxied tools exist only for servers the operator added to the project, and only for the roles in `exposed_to_roles`. They are named `<server>__<tool>` and return untrusted text blocks only. They never carry a credential of Ori's. (3) State in `spec/ENV_SETUP.md` section 5 that exposing a server to a role can widen that role beyond the manifest, so exposure is an operator act, recorded as an event. (4) Keep API_SPEC section 3's write rule binding on Ori's own tools |
| What is blocked | Slice S6, and with it ORI-P1-038. The host (H1, H2) and every other slice proceed |
| State | Answered by the operator in session on 2026-10-11: **yes, with the four conditions of the recommendation**. LLD's rule bounds Ori's own eleven tools; proxied tools are named `<server>__<tool>`, exist only for servers the operator added and roles the operator listed, return untrusted text only, and carry no credential of Ori's; API_SPEC section 3 and ENV_SETUP section 5 say so |

### Context

- PRD A-05 and I-03 require the engine to expose the operator's MCP servers to roles by policy (`spec/PRD.md`, I-03: "connect any MCP server to a project; expose to roles by policy").
- ORI-P1-038 (`spec/criteria/phase-1.md`) is exactly that case: the coder in A is refused by role, qa in B by project, and qa in A succeeds.
- `spec/SECURITY_NOTES.md` keeps MCP server credentials with the engine. So the agent cannot be handed a server's configuration to connect to it directly, and the engine must proxy the tools.
- A user server may hold a file tool that writes `spec/` or `ops/`, and Ori cannot tell which tool writes where.

### Options rejected

| Option | Why not |
|---|---|
| Proxy nothing | Contradicts A-05 and I-03, and ORI-P1-038 becomes unsatisfiable |
| Give the agent the server's configuration | Puts the operator's credential with the agent (SECURITY_NOTES), and a stdio server would then be launched outside the engine's control |
| Proxy only tools that declare themselves read-only | MCP tool annotations are hints from an untrusted server |
