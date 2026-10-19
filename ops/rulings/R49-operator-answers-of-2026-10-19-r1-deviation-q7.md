# R49. Operator answers, 2026-10-19: ORI-T-0197 interim deviation and batch 12 Q7

| | |
|---|---|
| Ruling | R49 |
| Tier | 1: accepts an interim implementation deviation on `crates/ori-flows/src/registry.rs` and directs Q7 work; no tier 2 path |
| Made by | The operator, in session, 2026-10-19, after reading the lead review on [PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85). Recorded by the lead the same day |
| Follows | [[R48]]; batch 12 `ops/batch-12-plan.md` Q7; ORI-T-0197 package; lead review on PR 85 (non-blocking; SO-1 not satisfied — Composer reviewed Composer) |

1. **ORI-T-0197 (slice R1) on [PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85)** (`feat/ORI-T-0197-flows-registry`, commits `683b320`, `66a7f9f`, `a742233`; **35** tests). The lead review is posted and **non-blocking**. **The operator accepts the interim deviation** in `apply_document_approved`: when the folded document is `Draft`, the fold applies `DocumentEvent::Submitted` **in memory only** before `Approved`, so a log can show `document.created` then `document.approved` with **no** `document.submitted` event — contrary to `spec/DATA_MODEL.md` §3 and `DocumentEvent::fires_from` (`document.approved` requires `UnderReview`). **Condition:** remove this bridge when `document.submitted` is implemented. After that, approval is allowed only after a logged submit, and the fold **refuses** `document.approved` unless the document is `UnderReview`. **Follow-up:** **ORI-T-0199** (claim **124**).
2. **Batch 12 Q7 (`contract_change`).** **Yes.** Implement **`flows.documents.submit`** appending **`document.submitted`**. This unblocks the DOC slice's submit path, satisfies ORI-P1-040's UnderReview precondition via the state machine, and retires the R1 bridge per point 1. PD-3 / API_SPEC documentation may still serialize with ORI-T-0192; the **code** path is **ORI-T-0199** before DOC's approve tests assume submit.

## ORI-T-0197 implementation recorded

| Item | Value |
|---|---|
| Pull request | [85](https://github.com/rachidsahane/ORI-STUDIO/pull/85) |
| Commits | `683b320`, `66a7f9f`, `a742233` |
| Tests | 35 (32 integration `ori_t_0197_registry.rs` + 3 unit in `registry.rs` per PR branch; package had 34 before the third commit added `unit_read_object_payload_rejects_non_object`) |
| Deviation | In-memory Draft→Submitted on approve only; **accepted until ORI-T-0199** |
| Claim 122 | **Implemented** on PR 85; **held until PR 85 merges** (then released per usual tier 1 path) |
