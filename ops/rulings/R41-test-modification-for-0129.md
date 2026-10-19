# R41. The operator authorizes the test change ORI-T-0129 needs

| | |
|---|---|
| Ruling | R41 |
| Tier | 2: it changes a test of the Ticket state machine (`spec/RISK_MAP.md`: `ori-core` state machines) |
| Made by | The operator, in session, 2026-10-11. Recorded by the lead the same day |
| Follows | [[R37]], [[R40]] |

## The answer

"Authorize" (the operator, 2026-10-11), to the question whether ORI-T-0129 may change the test module of `crates/ori-core/src/ticket.rs`. That module transcribes `spec/DATA_MODEL.md` section 3's Ticket diagram as constants: fourteen lines in `DIAGRAM`, `DIAGRAM_EDGES = 14`, a message saying fourteen edges, and `ALL_PAIRS - permitted == 130`. ORI-T-0128 drew a fifteenth edge, `InProgress --> Queued: engine recovery only` (3298433, on `claude/sweet-maxwell-jkaiie`).

## The condition

Only the transcription changes: the new diagram line is added to `DIAGRAM` verbatim, and the counts that follow from it become fifteen and 129. No assertion is removed or loosened, and no other test in the file changes. The lead counts assertion lines in the module before and after and reports both.
