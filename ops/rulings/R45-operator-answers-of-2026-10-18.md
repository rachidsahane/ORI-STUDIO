# R45. Four operator answers, 2026-10-18: proxied reach, the `ori-mcp` tier 2 row, evidence lockout, stdio-only host

| | |
|---|---|
| Ruling | R45 |
| Tier | 2: it settles a merge-path question (ADR-0002), a tier 2 surface change, and a security trade |
| Made by | The operator, in session, 2026-10-18, each as the lead recommended. Recorded by the lead the same day |
| Follows | [[E-0010]], [[E-0013]], ADR-0002; the review recorded in `ops/lock-table.md`, "Wave of 2026-10-12" |

1. **Proxied tools carry the operator's reach, by explicit opt-in** (review finding 0156-F1). The promises in `spec/API_SPEC.md` section 3 that agents "never receive the merge operation" and "never receive credentials through MCP" bind Ori's own tools and Ori's own credentials. An operator's MCP server may hold its own credentials and tools that act outside Ori's merge queue. Exposing such a server to a role requires the operator's explicit acknowledgement of exactly that, recorded as an event. `spec/SECURITY_NOTES.md` and ADR-0002 say the merge-queue guarantee covers Ori's own merge path only.
2. **The tier 2 RISK_MAP row `crates/ori-mcp/src/scopes.rs, ports.rs, server.rs, tools/` is confirmed** as the `adr_area` decision ADR-0002 asks for (review finding 0156-F9). The tier 2 tables of `spec/SECURITY_NOTES.md` and ADR-0002 name the same four modules.
3. **Evidence lockout accepted for now** (review finding 0144-F1). One evidence event that fails ORI-T-0144's check makes every explicit evidence request in that product refuse, permanently, because the log is append-only. That is accepted as the cost of failing closed. The doc says "permanent and product-wide", and a follow-up designs a recovery path (ORI-T-0164).
4. **The MCP host connects to stdio servers only in phase 1.** Remote (HTTP) servers are a later `new_dependency` decision, asked once together with `ori-rpc`'s transport question. `spec/ROADMAP.md` and `spec/API_SPEC.md` say so.
