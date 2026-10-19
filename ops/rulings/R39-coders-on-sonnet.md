# R39. Coders run on Sonnet 5.5; the lead session stays on Opus 5.5

| | |
|---|---|
| Ruling | R39 |
| Tier | 2: it changes the model separation between the coders and the reviewer of their work |
| Made by | The operator, in session, 2026-10-11. Recorded by the lead the same day |
| Narrows | [[R33]] |

## The decision

"Continue the development of ORI STUDIO, with coder agents in sonnet 5.5" (the operator, 2026-10-11).

From this session, every coder agent the lead session launches runs on Claude Sonnet 5.5, set on each launch. The lead session, which plans, reviews and integrates, runs on Claude Opus 5.5. Review agents the lead launches are not coders and stay on the lead's model.

## What it changes in [[R33]]

[[R33]] recorded that every agent ran on one model, so no review in that session could claim AICD §7's separation or ADR-0002's first substitute, SO-1. From this ruling the code and its review come from two different models.

Whether that is the separation AICD §7 asks for is **not** decided here. `spec/adr/ADR-0001-stack.md` defines the unit as the provider's "family identifier", "the provider's own grouping of models sharing a lineage". Sonnet and Opus are two model lines from one provider, released together under one generation number. A reader who takes "family" to mean the line may call SO-1 satisfied; a reader who takes it to mean the provider's lineage may not. This record states the fact and leaves the claim to the operator: no pull request report written under this ruling claims SO-1 as satisfied, and each says which models wrote and reviewed it.

## What stays as it was

- [[R33]]'s mitigations still apply to every review: fresh reviewers per round, adversarial prompts, a finding counts only when a separate agent reproduces it, and a reviewer that returns nothing counts as unverified.
- [[R38]]'s merge rule: the lead merges tier 0 and tier 1 once every check passes; the operator merges every tier 2 change.
- `.claude/agents/coder.md` still names `opus` in its front matter. It is a copy of `spec/agents/coder.md`, which is the documentation role's to change, so the model is set on each launch instead of in the file. If the operator wants the role file to say Sonnet, that is a documentation ticket and the paired copy, the same shape as ORI-T-0123 and ORI-T-0124.
