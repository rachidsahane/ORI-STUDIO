# R47. Three operator answers, 2026-10-19: ORI-T-0190, ORI-T-0191 and ORI-T-0165 before PR 78 merge

| | |
|---|---|
| Ruling | R47 |
| Tier | 2: it settles a tier 2 security fix before merge, documents a tier 2 core limit for merge, and defers a specification edit |
| Made by | The operator, in session, 2026-10-19, after reading decision briefs. Recorded by the lead the same day |
| Follows | [[R44]], [[R45]], [[R46]]; PR 78 escalations; independent review of ORI-T-0194 at [pullrequestreview-5574873291](https://github.com/rachidsahane/ORI-STUDIO/pull/78#pullrequestreview-5574873291) |

1. **ORI-T-0190 (category lowered in two agent calls: `decisional` → `product_signal` → `auto`).** **Accepted as a documented known limit for the PR 78 merge.** `ori-core`'s ladder allows the two-step move without `E_UPGRADE_ONLY`, which weakens AICD §11's "only a human can downgrade". The operator does not block merge on a rule change in `crates/ori-core/src/ticket.rs` now. **ORI-T-0190 stays a follow-up ticket** to be designed and fixed after the merge. S5 (when built) should state the limit; no second enforcement is required for merge.
2. **ORI-T-0191 (text scrub lets Unicode format characters through).** **Must be fixed before PR 78 merges.** A coder is dispatched on branch `fix/ORI-T-0191-scrub-format-chars` (pull request against `claude/sweet-maxwell-jkaiie`). Declared scope: `crates/ori-memory/src/retrieval.rs` and every copy of the scrub predicate, including `crates/ori-mcp/src/host/registry.rs` and the S4 blank check in `crates/ori-mcp/src/tools/report.rs`. The operator directed the coder to **design the repair from the ticket text** in `ops/lock-table.md` ("ORI-T-0191, the scrub lets invisible format characters through"), which is a **deviation from [[R44]]** (the lead designs, the coder types). **A lead review is owed before that pull request merges.** Claim **123** holds the work (`ops/lock-table.md`).
3. **ORI-T-0165 (three specification sites lacking "Ori's own merge path" wording).** **Deferred until after the PR 78 merge.** The three replacements that carry [[R45]] point 1 into PRD F-03, CI_CD section 1 and ARCHITECTURE section 9 are not retried now. Claim 95 stays released; the operator may land the text after merge without another [[R46]] retry.

## ORI-T-0194 review recorded

The independent adversarial review of ORI-T-0194 (batch 10 slice H1) was performed and posted at [pullrequestreview-5574873291](https://github.com/rachidsahane/ORI-STUDIO/pull/78#pullrequestreview-5574873291). **Nothing blocking.** The operator **accepts the registry lockout** (same trade as [[R45]] point 3 for evidence). **`spec/RISK_MAP.md` keeps the host registry at tier 1** until PD-1; the raised tier on the ticket stands for scrutiny, not for a new row before the operator answers batch 12 PD-1. **Claim 121 is released** ([[R39]] satisfied; repairs, if any, follow the usual repair path on the session branch).
