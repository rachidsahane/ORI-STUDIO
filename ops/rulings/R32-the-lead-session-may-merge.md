# R32. The lead session may merge pull requests itself, on three conditions

| | |
|---|---|
| Ruling | R32 |
| Tier | 2: it governs the merge path |
| Made by | The operator, in session, on or before 2026-09-23. Recorded by the lead on 2026-09-27 |
| Scope | This build session. Not the product's own lead identity: see "What this is not" |

## The authorization

The operator told the lead session to "merge and continue". From then on the lead merged pull requests itself instead of stopping for the operator at each one.

## When

`gh` cannot say who merged. It attributes every merge in this repository, pull requests 1 to 71, to one account, `rachidsahane`, and the lead session acts through that account: its closing comment on pull request 45 ("the scope I wrote here was wrong") was posted from it.

The lead reports pull request 53 as the first merge it performed: merged 2026-09-23T13:20:04Z, with pull request 57 fifteen seconds later. The record around it is consistent with that. Pull request 59, the lead's rebuild of pull request 55 on the new `main`, was opened at 13:21:55Z, and pull requests 54, 55, 56 and 58 were closed as superseded between 13:22:13Z and 13:23:37Z with the lead's comment ("Branch kept, not deleted"). Nothing in `gh` confirms or excludes an earlier lead merge, so this record dates the authorization "on or before 2026-09-23".

## The conditions the lead applies

1. Every CI check on the pull request reports pass and none is pending: 18 of 18 on this repository today.
2. Rebase merges only. The repository allows nothing else: `allow_rebase_merge` is true, merge commits and squash merges are off.
3. The lead never force-pushes and never deletes a ref.

## Checked against the fifteen merges from pull request 53 to 71

| Condition | What the history shows |
|---|---|
| 18 of 18, none pending | **Broken once.** Pull request 69 merged at 2026-09-23T20:46:28Z while `test (windows-latest)` was still running (it finished at 20:49:30Z) and before `ci` had started (20:49:33Z). Both passed afterwards. The other fourteen merged after all 18 checks had passed |
| Rebase only | Held; the repository refuses anything else |
| No force-push | Held: no force-push event on any of the fifteen timelines |
| No deleted ref | Held by the lead, with one qualification. The repository's `delete_branch_on_merge` setting is true, so GitHub deletes a merged pull request's head branch itself, 2 to 4 seconds after each of these merges, attributed to the merging account. Every merge the lead performs therefore deletes one remote branch through that setting. The branches of the four superseded pull requests (54, 55, 56, 58) are kept |

## What it departs from

The authorization sets aside, for this session, three written rules, and amends none of them:

- `CLAUDE.md` absolute rule 2, for every role: "You never merge".
- `.claude/agents/lead.md`: "You never merge: you approve, the merge queue performs the merge."
- `spec/adr/ADR-0002-single-operator.md`, "What the agents must respect": "No agent merges, at any tier", and "The operator merges every tier 1 and tier 2 change personally". Seven of the fifteen merges above were tier 2 by their titles: 53, 59, 60, 62, 63, 66 and 69.

The operator decided it, and this record makes the departure visible. The rulings archive's own rule is that a ruling authorizing a deviation from a specification document is paired with a specification pull request correcting that document in the same batch. No such pull request exists for ADR-0002. Whether one should, or whether this authorization ends with the session, is the operator's to say.

## What this is not

It is not an answer to [[E-0007]]. E-0007 asks whether the product's own lead identity may merge tier 0, as AICD §17 grants, when `spec/ENV_SETUP.md`, `spec/SECURITY_NOTES.md` and `CLAUDE.md` say the merge queue alone merges. That is a question about what Ori Studio lets its lead identity do. This record is about who presses merge while Ori Studio is being built. E-0007 stays open with the operator, and ORI-T-0029 stays blocked on it.
