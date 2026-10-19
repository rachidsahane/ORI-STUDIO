# R42. The operator authorizes the test changes the adapter-trait move needs, and stages `AgentRuntime`

| | |
|---|---|
| Ruling | R42 |
| Tier | 2: one test is in `crates/ori-memory/src/scope.rs`, which `spec/RISK_MAP.md` tiers 2 |
| Made by | The operator, in session, 2026-10-11. Recorded by the lead the same day |
| Follows | [[R37]], [[R40]], [[R41]]; [[E-0002]] |

## The questions and the answers

ORI-T-0058's coder stopped on two escalations (`test_modified`, and a scope that could not hold the move). The lead put both to the operator.

1. **"Authorize"**: `scope::tests::ori_t_0038_identity_payload_layout_and_rules_match_ori_broker_registration` reads `crates/ori-broker/src/identity.rs` as text to check that the scope enforcer's identity reader matches its writer. ORI-T-0058 moved those lines to `crates/ori-core/src/adapters/identity.rs`. The test may change **the path it reads, and nothing else**.
2. **"Stage it after ORI-T-0140"**: moving `AgentRuntime` into `ori-core` drags `LaunchConfig`, `StdioMode`, `RuntimeError`, `transcript::Entry` and `TranscriptError` with it, touches `crates/ori-runtime/src/transcript.rs` and `headless.rs`, and changes two existing tests. It becomes its own ticket, ORI-T-0141, after ORI-T-0140 releases `headless.rs`. ORI-T-0058 lands the six slot traits and `AgentIdentity`. The two test changes ORI-T-0141 needs are authorized now:
   - `crates/ori-runtime/src/acp.rs`, the test that builds `SessionHandle { id: ... }` by struct literal: it may call `SessionHandle::new(...)` instead, the same value;
   - the test-only `LaunchConfig::assemble(program, args, StdioMode::Inherit, true)` calls in `acp.rs` and `headless.rs`: they may call `LaunchConfig::new(program, args, StdioMode::Inherit)`, which always sets `non_interactive: true`, so the value is the same. A public, non-test `assemble` that could build an interactive launch is **not** authorized: it would weaken the fail-closed launch configuration.

## The condition

As for [[R40]] and [[R41]]: no assertion is removed or loosened, and the lead counts assertion lines before and after each changed test and reports both.
