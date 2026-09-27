# R38. Tier 2 merges return to the operator; the lead session merges tier 0 and tier 1

| | |
|---|---|
| Ruling | R38 |
| Tier | 2: it governs the merge path |
| Made by | The operator, in session, 2026-09-27. Recorded by the lead the same day |
| Narrows | [[R32]] |

## The question

[[R32]] recorded the operator's "merge and continue", under which the lead session merged fifteen pull requests, seven of them tier 2, and set out what that departed from: `spec/adr/ADR-0002-single-operator.md` says "No agent merges, at any tier" and "The operator merges every tier 1 and tier 2 change personally". [[R33]] recorded that every agent now runs on one model, so ADR-0002's first single-operator substitute, independent review by a different model family, does not hold for any review this session performs. The lead put both facts to the operator and asked how merges should go from now on.

## The answer

"Tier 2 back to me" (the operator, 2026-09-27). Precisely:

1. The lead session merges **tier 0 and tier 1** pull requests itself, only when every CI check reports pass with none pending, rebase merges only, never a force-push and never a deleted ref.
2. **Every tier 2 pull request is merged by the operator personally.** The lead prepares it fully verified, says so in the pull request, and reports it ready. It does not merge it.
3. A tier is read from `spec/RISK_MAP.md`, not from a pull request's title. A tier 1 ticket that changes a tier 2 path is tier 2 for that pull request: ORI-T-0039, whose repair writes `barrier.rs`, is the first case.

## What follows

- ADR-0002 is amended to say this, by the documentation role, as ORI-T-0123. ADRs are tier 2 in `spec/RISK_MAP.md`, so the operator merges that amendment too.
- [[R32]] stays as written: it is the record of what happened under the earlier authorization, including the seven tier 2 merges, and nothing in it is rewritten.
- [[E-0007]], which asks what the product's own lead identity may merge, is a different question and stays open.
