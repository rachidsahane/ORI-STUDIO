# R48. Operator answer, 2026-10-19: PD-1 for the MCP host registry (`adr_area`)

| | |
|---|---|
| Ruling | R48 |
| Tier | 2: approves a change to `spec/RISK_MAP.md`, `spec/SECURITY_NOTES.md` and ADR-0002's tier 2 surface |
| Made by | The operator, in session, 2026-10-19, after reading decision briefs. Recorded by the lead the same day |
| Follows | [[R45]], [[R47]]; batch 10 PD-1 precedent; ORI-T-0194 review at [pullrequestreview-5574873291](https://github.com/rachidsahane/ORI-STUDIO/pull/78#pullrequestreview-5574873291) |

1. **PD-1 for the MCP registry is approved.** **`crates/ori-mcp/src/host/registry.rs` is tier 2** on `spec/RISK_MAP.md`, with the same module named in `spec/SECURITY_NOTES.md` "Tier 2 modules" and ADR-0002's tier 2 surface table. The specification change is [PR 81](https://github.com/rachidsahane/ORI-STUDIO/pull/81) (`adr_area` escalation in its body). Its commit carries a wrong `Ticket: ORI-T-0195` trailer; the squash merge must use **`Ticket: ORI-T-0194`**. PR 81 fails CI alone (`ori_t_0044`: fifteen tier-2 rows in RISK_MAP vs fourteen in `RISK_MAP_TIER_TWO`). A coder stacks the paired gate 12 restatement on PR 81 under **`Ticket: ORI-T-0194`**; **both merge together** before PR 78.

## Pull requests in flight (recorded)

| PR | Role |
|---|---|
| [81](https://github.com/rachidsahane/ORI-STUDIO/pull/81) | PD-1 spec: MCP registry tier 2 (+ stacked gate 12 fix) |
| [83](https://github.com/rachidsahane/ORI-STUDIO/pull/83) | ORI-T-0191 fix (claim 123); **tier 2 merge** because it touches `host/registry.rs`. Independent review in progress |
| [80](https://github.com/rachidsahane/ORI-STUDIO/pull/80) | CLAUDE.md source fix (`spec/agents/CLAUDE.md`) |
| [82](https://github.com/rachidsahane/ORI-STUDIO/pull/82) | Regenerated root `CLAUDE.md`, stacked on PR 80 |

**ORI-T-0191 scope split ([[R47]] point 2).** `crates/ori-mcp/src/tools/report.rs` is **not on the session branch** (S4 exists only as `ops/handoff/ORI-T-0187-branch.patch`). The S4 blank-check portion of ORI-T-0191 moves to **ORI-T-0195**: when S4 lands, it must call `ori_memory::retrieval::is_blank_after_untrusted_scrub` (see `ops/lock-table.md`).
