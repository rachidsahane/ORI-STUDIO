# R46. Three operator answers, 2026-10-18 afternoon: ORI-T-0161's test change, ORI-T-0165's retry, and ORI-P1-038's order row

| | |
|---|---|
| Ruling | R46 |
| Tier | 2: it authorizes a change to a test in `crates/ori-memory/src/scope.rs`, which `spec/RISK_MAP.md` tiers 2 |
| Made by | The operator, in session, 2026-10-18, each as the lead recommended. Recorded by the lead the same day |
| Follows | [[R40]], [[R41]], [[R42]], [[R45]]; `ops/lock-table.md`, "Claim 94 reviewed" and "Claim 96 reviewed" |

1. **`test_modified` for ORI-T-0161: authorized.** Two existing tests may change, exactly as the coder's patch (`target/test-edits-0161.patch` in ORI-T-0161's worktree) changes them:
   - `scope::tests::ori_t_0038_evidence_is_only_the_blobs_stored_with_the_record`;
   - `oplog::tests::ori_t_0039_the_operational_log_filters_by_ticket_kind_and_id_without_dropping_a_reused_id`.

   Both built, through `submit_report`, states the fixed write can no longer produce. The patch plants the same events by hand, and in the oplog helper gives each call a fresh evidence id. The stale paragraph of `scope.rs`'s module doc on raw evidence (review finding 0161-F3) is corrected in the same change. Claim 94 extends to those two test modules and that paragraph.
2. **ORI-T-0165: retry once, with this approval.** The three exact replacements, which carry [[R45]] point 1 into PRD F-03, CI_CD section 1 and ARCHITECTURE section 9, are dispatched again, citing this answer. If the session's permission classifier refuses again, the lead gives the operator the text and stops.
3. **ORI-P1-038 gains the case that pins the order** (S1 review). A role the server does not list, calling from another project, is refused for the project before its role is looked at. A documentation ticket changes the criterion's row; S1's tests already cover the case.

## The condition

As for [[R40]] to [[R42]]: no assertion is removed or loosened, and the lead counts assertion lines before and after each changed test and reports both. Review note 0161-F15 is recorded with this answer: the one assertion whose message changes (`orphans == 1`) now checks the planted fixture as the read side sees it, not the barrier's write.
