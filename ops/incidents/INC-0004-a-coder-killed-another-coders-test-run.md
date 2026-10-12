# INC-0004: a coder killed another coder's test run by pattern

| | |
|---|---|
| Kind | Interference between parallel agents on one machine |
| Tickets | ORI-T-0039 (acted), ORI-T-0054 (affected) |
| Detected | ORI-T-0039's coder disclosed it at the top of its own report, 2026-10-11 |
| Damage | None to any commit or file. One test run of ORI-T-0054 was killed and must be re-run |

## What happened

Stopping a stale background test run of its own, ORI-T-0039's coder ran `pkill -f "cargo test --workspace --locked"`. Every coder in the session runs that exact command line in its own worktree, so the pattern also matched ORI-T-0054's run. `pgrep` showed that process (PID 2143) before the kill and not after. The coder said so first, before its results, and named the ticket affected. The lead told ORI-T-0054's coder to re-run.

## Why this is the lead's defect as much as the coder's

The standing facts given to every coder said where to write and what never to modify. They said nothing about processes, on a machine where up to thirteen agents ran the same commands at once. A rule that matters only when agents share a machine was missing from the one document written because they do.

## The repair

From this incident every package's standing facts carry: never kill a process by pattern; kill only a PID you started yourself and recorded. Packages already dispatched were not re-sent; the coder affected was told directly.
