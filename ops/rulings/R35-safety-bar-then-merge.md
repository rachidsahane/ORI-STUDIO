# R35. Convergence: safety bar, then merge

| | |
|---|---|
| Ruling | R35 |
| Tier | 1: it applies to tier 1 tickets |
| Made by | The operator, in session, 2026-09-27. Recorded by the lead on 2026-09-27 |

## The ruling

"Safety bar, then merge" (the operator, 2026-09-27).

When a tier 1 ticket has not converged after three adversarial review rounds:

1. One targeted round fixes only the items inside the operator's threat model for that work ([[R34]] for the code map).
2. Only those items, plus regressions, are re-verified.
3. The ticket merges.
4. Every completeness gap becomes a follow-up ticket, allocated in `ops/lock-table.md` under [[R31]] rule 1 and stated as the defect.

## First uses

- Pull request 70, ORI-T-0036, the code map. Its body records three self-repair rounds that confirmed 7, 5 and 6 findings, each round narrower, without converging; the ninth round was the safety round.
- Pull request 71, ORI-T-0035, the index and freshness, merged under the same ruling the same day.

Their follow-ups are allocated in `ops/lock-table.md`, section "Batches 2 to 6", as ORI-T-0112 to ORI-T-0122. One further item is recorded there as a constraint on ORI-T-0061 rather than as a ticket, because the backlog already holds that work.

## What it does not change

It does not lower the bar for a finding: a finding still counts only when a separate agent reproduces it ([[R33]]). A completeness gap is a ticket in `ops/lock-table.md`, not a line in a pull request body. The ruling as given covers tier 1 tickets and says nothing about tier 2.
