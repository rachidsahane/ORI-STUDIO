# R43. The recovery edge is guarded in the same change that adds it

| | |
|---|---|
| Ruling | R43 |
| Tier | 2: it changes a test of the Ticket state machine (`spec/RISK_MAP.md`: `ori-core` state machines) |
| Made by | The operator, in session, 2026-10-11. Recorded by the lead the same day |
| Follows | [[R41]]; closes ORI-T-0142 into ORI-T-0129 |

## The question

ORI-T-0129 adds `InProgress --> Queued` to the ticket machine. Its independent review found that, with no guard, `crates/ori-orchestrator/src/lifecycle.rs`'s `Lifecycle::advance` lets any actor in any role make that move: `verb(InProgress, Queued)` is `None`, so no permission is checked, and leaving an in-flight state releases every lock the ticket holds, while its coder session may still be running with its worktree and credentials. Nothing calls `advance` outside tests today, so the path is latent. `spec/DATA_MODEL.md` section 3 says the edge is "the engine's move and no one else's", and section 4 (ORI-T-0130) makes its actor `system`.

## The answer

"Add guard in ORI-T-0129" (the operator, 2026-10-11). `Ticket::apply` refuses `InProgress --> Queued` unless the event's actor is `Actor::System`, with a `RefusalKind::TicketTransition` naming the actor and a `MethodologyRef`. The edge never reaches `main` unguarded.

## The test change this authorizes

`ticket::tests::ori_t_0020_apply_accepts_exactly_the_pairs_the_diagram_draws` feeds a human actor to every drawn edge through the shared helper. It may give **this one edge** an `Actor::System` event, by a pair-aware helper or by a branch for that pair. Every other pair keeps its human actor, and no assertion is removed or loosened. ORI-T-0129's own new test that pins "any actor is accepted" is not an existing test and is replaced.

## What it does not do

`system` is also the actor of scheduled triggers and watchers, so the guard is wider than the sentence it enforces. A role or verb check for the move in `lifecycle.rs` is not part of this ruling; it remains a follow-up.
