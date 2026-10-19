# ORI-T-0187, repair 2: the re-review's corrections to repair 1 (Haiku: every decision is made; apply exactly)

| Field | Value |
|---|---|
| Worktree | `/home/user/wt/ORI-T-0187` |
| Branch | `feat/ORI-T-0187-report-tool`, at `38f41ae` |
| Tier | **2** (`spec/RISK_MAP.md`: `crates/ori-mcp/src/tools/`); the operator merges |
| Scope | `crates/ori-mcp/src/tools/report.rs` only: doc comments, and changes inside this ticket's own tests 8, 16 and 17 (comments and test 17's tamper), which have not reached `main`. Nothing else |
| Why | A focused re-review of repair 1, the lead's own words and tests, confirmed three faults of the lead's: test 17's tamper was also caught by `read_range`, so a skipped `EventLog::verify` still passed; the log-cost sentence miscounted the barrier's read; and PD-2 was cited for work it never covered. The lead wrote every edit and proved them in a copy: fmt, workspace clippy, strict doc, `ori-mcp` (234) and `ori-gates` (194) tests, the Windows compile check, and the mutation that skips the verify, which fails test 17 |

## The edits

The lead's script holds each edit's exact old and new text. It asserts that each old text occurs exactly once, and it writes nothing unless every edit applies. **It removes no test.** Copy it into your worktree's `target/` and run it there with `python3 -I`, from the worktree root:

```
mkdir -p /home/user/wt/ORI-T-0187/target/r2
cp /tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/edits-0187-r2/apply.py /home/user/wt/ORI-T-0187/target/r2/apply.py
cd /home/user/wt/ORI-T-0187 && python3 -I target/r2/apply.py
```

It must print `applied 7 edits`. If it exits with any other message, stop and report the message.

## Commit

Stage only `crates/ori-mcp/src/tools/report.rs`, and commit with exactly this message:

```
fix(mcp): make aicd_report's log test see a skipped verify, correct its docs

Repair 2 of ORI-T-0187, from the focused re-review of repair 1. Test 17
now breaks the log where only EventLog::verify looks, the last event's
product, so a handler that skipped the verify fails it. The log-cost
sentence counts the barrier's one verified read, made for every report
it is handed. The report events and the Report row are ORI-T-0192's to
reconcile, not PD-2's. The provenance clause and three test comments
are corrected.

Ticket: ORI-T-0187
Spec: API_SPEC.md#3-agent-facing-mcp-server
Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CQ4vWTcpGWb4B2jvnPi4RM
```

## Checks (each to a file under the worktree's `target/r2/`, exit status read on the next line)

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
3. `cargo test -p ori-mcp -p ori-gates --locked` (report the passed counts)
4. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`
5. `CARGO_TARGET_DIR=/home/user/wt/ORI-T-0187/target/win cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings`
6. gate 13: `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/gate13.sh /home/user/wt/ORI-T-0187 4121562 HEAD`

Report the commit hash, the six exit statuses as numbers, and the passed counts of check 3.

## Standing rules (you are a coder on Claude Haiku 5.5, ruling R44: this package carries every decision; apply it exactly)

- **First**, prove where you are: run `cd /home/user/wt/ORI-T-0187 && git rev-parse --abbrev-ref HEAD` and do nothing until it prints exactly `feat/ORI-T-0187-report-tool`.
- `CLAUDE.md` at the repository root applies. Write only inside `/home/user/wt/ORI-T-0187`, and only the file the package names. Never write to `/home/user/ORI-STUDIO`.
- Commit locally on your branch only. **Never push**, never open a pull request, never touch another branch, never rewrite an existing commit.
- Change no test except as the lead's script does. Never add a crate.
- Never kill a process by pattern (`pkill`, `killall`); other agents' builds run on this machine.
- Write every output or log file under `/home/user/wt/ORI-T-0187/target/r2/`. Run each check to a file and read its exit status on the next line, never through a pipe.
- If anything does not match what the package says (the script's message, a check failing for a reason the package does not foresee, a refusal of any command), **stop and report exactly what you found**. Do not improvise a fix and do not try another route: the lead decides.
- Your final message: the commit hash, each check's exit status as a number, and anything that did not match.
