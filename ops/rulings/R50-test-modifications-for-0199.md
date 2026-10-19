# R50. The operator authorizes the two ORI-T-0197 test edits ORI-T-0199 needs

| | |
|---|---|
| Ruling | R50 |
| Tier | 1: both change integration tests in `crates/ori-flows` (`spec/RISK_MAP.md`: whole `ori-flows`) |
| Made by | The operator, in session, 2026-10-19. Recorded by the lead the same day |
| Follows | [[R49]]; ORI-T-0199 package `ops/tickets/ORI-T-0199-document-submitted.md` escalation (`test_modified`), commit `94081a1` on `lead/batch-12-plan` |

## The answer

**Approved** (the operator, 2026-10-19), to the question whether ORI-T-0199 may modify the two ORI-T-0197 integration tests in `crates/ori-flows/tests/ori_t_0197_registry.rs` so approval paths include a logged `document.submitted` after [[R49]]'s interim approve bridge is removed. `CLAUDE.md` absolute rule 3 forbids an agent to modify an existing test without that answer.

## Authorized edits only

ORI-T-0199 may modify **exactly** these two tests, as the package replacement table describes, **without weakening any assertion**. **No other existing test may change.**

| Test | Authorized change |
|---|---|
| `ori_t_0197_record_approver_holding_seat_succeeds` | After `document.created`, **`record` `document.submitted`**, then `document.approved` with holder; **same final approved state** |
| `ori_t_0197_fold_refuses_document_submitted_kind` | With valid `{document_id}` payload after `document.created`, **fold succeeds** and document is **`UnderReview`**; **rename acceptable**; malformed submitted payloads are covered by new **`ori_t_0199_*`** tests |

## Dispatch precondition

**ORI-T-0199 is dispatched only after** [PR 85](https://github.com/rachidsahane/ORI-STUDIO/pull/85) (ORI-T-0197 / R1) **merges into `claude/sweet-maxwell-jkaiie`**. Until then the ticket stays designed; the coder must not edit the listed tests without this ruling ([[R50]]), and must not be dispatched early.

## The condition, which review checks

What each test proves does not get weaker. Malformed-payload and refusal coverage moves to **`ori_t_0199_*`** tests rather than deleting assertions from the ORI-T-0197 file. The lead review counts assertion lines before and after, per modified test, and reports both numbers, as for [[R37]] and [[R40]].
