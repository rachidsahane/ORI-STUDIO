# ORI-T-0163, repair 2: the found loss keeps its place and time, and four doc corrections (Haiku: every decision is made; apply exactly)

| Field | Value |
|---|---|
| Worktree | `/home/user/wt/ORI-T-0163` |
| Branch | `fix/ORI-T-0163-root-probe`, at `d31679b` |
| Tier | 1 (`spec/RISK_MAP.md`: `crates/ori-watch (watcher, hooks)`) |
| Scope (claim 112) | six text replacements in `crates/ori-watch/src/watcher.rs`. One changes the delivery step in `next_batch`; one adds a new test; the others are doc comments, and a comment of this ticket's own new test. Nothing else. No test that existed before this ticket changes |
| Why | The focused re-review of repair 1 found two should-fix items. First, a batch that delivers a root loss `mark_reconciled` found beside another loss listed the root last, with a later time than the health holds: the lead's Decision 2 prescribed `record()`. Second, a Windows doc bullet contradicted itself. It also left three notes. The lead wrote the replacements and proved them in a copy: fmt, clippy, the Windows clippy, strict doc, `ori-watch` 143 passed, `ori-gates` 194 passed, the new test 20 runs out of 20, and the old delivery code fails it |

## The edits

The lead's script holds each replacement's exact old and new text and asserts each old text occurs exactly once. Copy it into your worktree's `target/` and run it there with `python3 -I`, on the one file:

```
mkdir -p /home/user/wt/ORI-T-0163/target/r2
cp /tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/edits-0163-r2/apply.py /home/user/wt/ORI-T-0163/target/r2/apply.py
cd /home/user/wt/ORI-T-0163 && python3 -I target/r2/apply.py crates/ori-watch/src/watcher.rs
```

It must print `applied 6`. If it exits with any other message, stop and report the message.

## Commit

Stage only `crates/ori-watch/src/watcher.rs`, and commit with exactly this message:

```
fix(watch): deliver a found root loss first, at the time it was found

The focused review of repair 1 found that a batch delivering a root
loss mark_reconciled found beside another loss listed the root last,
with a later first_observed_at than the health holds. The delivered
state is now the found loss with the batch's own absorbed into it, so
the batch reads as the health does; a new test pins it. The Windows
bullet says notify unwatches a deleted root only once its name has
gone, the three rules say a stopped backend overrides them, the look's
cost names the Windows open, and the delete-pending test's comment
says CI's Windows leg exercises the open.

Ticket: ORI-T-0163
Spec: ARCHITECTURE.md#5-data-flows
Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01CQ4vWTcpGWb4B2jvnPi4RM
```

## Checks (each to a file under the worktree's `target/r2/`, exit status read on the next line)

1. `cargo fmt --all --check`
2. `cargo clippy -p ori-watch --all-targets --locked -- -D warnings`
3. `CARGO_TARGET_DIR=/home/user/wt/ORI-T-0163/target/win cargo clippy -p ori-watch --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings`
4. `cargo test -p ori-watch -p ori-gates --locked` (report the passed counts)
5. `RUSTDOCFLAGS="-D warnings" cargo doc -p ori-watch --no-deps --locked`
6. gate 13: `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/gate13.sh /home/user/wt/ORI-T-0163 10cd14c HEAD`

Report the commit hash, the six exit statuses as numbers, and the passed counts of check 4.

## Standing rules (you are a coder on Claude Haiku 5.5, ruling R44: this package carries every decision; apply it exactly)

- **First**, prove where you are: run `cd /home/user/wt/ORI-T-0163 && git rev-parse --abbrev-ref HEAD` and do nothing until it prints exactly `fix/ORI-T-0163-root-probe`.
- `CLAUDE.md` at the repository root applies. Write only inside `/home/user/wt/ORI-T-0163`, and only the file the package names. Never write to `/home/user/ORI-STUDIO`.
- Commit locally on your branch only. **Never push**, never open a pull request, never touch another branch, never rewrite an existing commit.
- Never modify or delete a test that existed before this ticket; the script only adds one test and rewords one comment of this ticket's own test. Never add a crate.
- Never kill a process by pattern (`pkill`, `killall`); other agents' builds run on this machine.
- Write every output or log file under `/home/user/wt/ORI-T-0163/target/r2/` (check 3 uses `target/win`). Run each check to a file and read its exit status on the next line, never through a pipe.
- If anything does not match what the package says (the script's message, a check failing for a reason the package does not foresee), **stop and report exactly what you found**. Do not improvise a fix: the lead decides.
- Your final message: the commit hash, each check's exit status as a number, and anything that did not match.
