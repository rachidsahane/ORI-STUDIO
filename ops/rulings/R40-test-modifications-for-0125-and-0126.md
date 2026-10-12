# R40. The operator authorizes the test modifications ORI-T-0125 and ORI-T-0126 need

| | |
|---|---|
| Ruling | R40 |
| Tier | 1: both change tests of tier 1 modules (`spec/RISK_MAP.md`: code map, indexer) |
| Made by | The operator, in session, 2026-10-11. Recorded by the lead the same day |
| Follows | [[R37]], the last time the operator answered `test_modified` |

## The answer

"Authorize both" (the operator, 2026-10-11), to the question whether a coder may modify the existing tests that two recorded defects need changed. `CLAUDE.md` rule 3 forbids an agent to modify an existing test without that answer.

| Ticket | Tests | Defect |
|---|---|---|
| ORI-T-0125 | `ori_t_0036_retained_heading_text_is_bounded_for_many_modules_citing_long_headings`, `ori_t_0036_a_spec_document_the_deadline_interrupted_is_recorded`, `ori_t_0036_the_rust_declaration_walk_checks_its_deadline`, in `crates/ori-memory/src/code_map.rs` | They time out under machine load, on `main` as well, and pass alone in about a second |
| ORI-T-0126 | Five tests in `crates/ori-memory/src/code_map.rs` and `crates/ori-memory/src/indexer.rs`, named in `ops/lock-table.md` | They fail whenever the suite runs as root, because mode 000 does not restrict root |

## The condition, which is the lead's and which review checks

What each test proves on an ordinary, unloaded, non-root runner does not get weaker. Concretely:

- No assertion is removed. An assertion may move behind a check that states why it cannot be exercised here, and that check prints its reason; it never silently passes.
- A deadline test keeps testing that the deadline is checked and honoured. Making it robust to load means changing how the test arranges or measures time, not widening the bound until it no longer bounds anything.
- A root-run test that cannot make a path unreadable says so in its output, and the part of the test that does not depend on the path being unreadable still runs.
- The lead's review counts assertion lines before and after, per test, and reports both numbers, as for [[R37]].
