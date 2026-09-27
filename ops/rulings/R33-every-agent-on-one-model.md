# R33. Every agent runs on Opus 5.5, so the lead shares a model family with the coders it reviews

| | |
|---|---|
| Ruling | R33 |
| Tier | 2: it removes a separation-of-duties control from this session's reviews |
| Made by | The operator, in session, 2026-09-24. Recorded by the lead on 2026-09-27 |

## The decision

"Use opus 5.5 for every agents" (the operator, 2026-09-24).

From that day the lead session and every coder, reviewer and workflow agent it launches run on one model, Claude Opus 5.5. The commit history agrees: all 64 commits on `main` authored from 2026-09-24 onward name Claude Opus 5.5 as co-author, and the last commit naming Claude Sonnet 5 was authored on 2026-09-23.

## The deviation, stated

One model family now writes the code and reviews it. The written rules forbid that for the product's own lead and coder identities:

- AICD §7: "AICD requires that the lead/reviewer run on a different model family than the coders it reviews."
- `spec/adr/ADR-0001-stack.md`: "A lead identity and the coders it reviews never share a model family, and the check is enforced at identity creation, not at review time."
- `spec/adr/ADR-0002-single-operator.md` restates it among what the agents must respect, and lists independent review by a different model, AICD §38's first substitute, as SO-1. No tier 2 pull request reviewed in this session can claim SO-1 as satisfied.
- `.claude/agents/lead.md` tells a lead agent that finds it has been given the same model as its coders to refuse to start.

The product enforces the rule at `broker.identity.create` (ORI-T-0028, ORI-T-0108). This build session creates no product identity, so nothing mechanical checks it here.

The operator decided it. This record makes it visible and does not resolve it.

## The mitigation actually used

- Every review round uses fresh, independent reviewers, launched for that round, with adversarial prompts.
- A finding counts only when a separate agent reproduces it.
- A reviewer that returns nothing counts as unverified, never as clean.

## What it does not mitigate

Shared blind spots of one model family. AICD §7 gives the reason in one sentence: "If the coder agents and the reviewing agent share the same underlying model, they share the same blind spots, and a review by the same intelligence that produced the code is weaker than it looks." More reviewers of the same family, and reproduction by the same family, narrow what one agent misses. They do not reach what the family misses.
