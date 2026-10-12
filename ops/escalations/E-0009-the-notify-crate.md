# Escalation E-0009: the `notify` crate for the tree watcher

| | |
|---|---|
| Trigger | `new_dependency` |
| Raised by | The lead, at batch 9 planning, before ORI-T-0054 was dispatched |
| Blocks | ORI-T-0054 (tree watcher and git hooks), and through it ORI-T-0055 (attribution) and criteria ORI-P1-014 and ORI-P1-015 |
| State | Answered by the operator in session on 2026-10-11: **approved**, on the terms of E-0004: exact pin, `Cargo.lock` committed, gate 7 decides. If gate 7 refuses it, the coder stops and the question returns to the operator; `deny.toml` is not relaxed to admit it |

## Context

`spec/LLD.md` section 2 names the crate in `ori-watch`'s row: "`TreeWatcher` (notify crate)". ORI-P1-014 asks for an unattributed change to be detected "within two seconds" of a disk edit. No escalation had approved `notify`, and `CLAUDE.md` rule 6 is about who decides what enters the trust boundary, not about who first wrote the name down (E-0004, "Why this is raised at all").

## What the lead measured before asking

In a scratch worktree, not in the repository, on 2026-10-11:

- `notify` 8.2.0, license CC0-1.0, which `deny.toml`'s license list allows. The newest release on the index is `9.0.0-rc.5`, a release candidate, which the lead does not recommend.
- Adding it to `crates/ori-watch/Cargo.toml` adds about twenty crates to `Cargo.lock`: `inotify`, `kqueue`, `mio`, `walkdir`, `notify-types` and the `windows-targets` 0.53 family among them.
- It adds a **second `windows-sys`**: 0.61.2, through `mio`, beside the 0.60.2 already locked (through `keyring` and through `notify` itself). The lead tried `mio` 1.2.4, 1.1.1, 1.0.4, 1.0.3 and 1.0.2; none uses 0.60.2. `deny.toml` sets `multiple-versions = "deny"` with no skip list, which is the rule that refused `tantivy` (E-0004).
- **Not established:** whether gate 7 refuses it. `cargo-deny` is not installed in this container. `Cargo.lock` on `main` already holds two `core-foundation` and two `security-framework` versions under `keyring`, and gate 7 reported `bans ok` on `main` at 7bed207 (CI run 36347420109), so `cargo tree --target all` and `cargo deny` do not see the same graph. CI decides.

## Options put to the operator

1. Approve `notify`, exact pin, gate 7 decides. **Recommended.** Chosen.
2. A polling watcher on `std` alone. No new crate, but a departure from `spec/LLD.md` section 2 that would need a documentation-role change.
3. Hold batch 9.

## Gate 7 refused it, and the question came back (2026-10-12)

CI on 3e53283, the first run to carry ORI-T-0054, failed gate 7: `windows-sys` is locked at 0.60.2 (through `keyring` 3.6.3, `notify` 8.2.0 itself and `winapi-util`) and at 0.61.2 (through `mio` 1.2.4 under `notify`). On these terms the lead withdrew ORI-T-0054 from the session branch (6d406e4) and put the question to the operator again, with what the lead measured:

- `notify` pulls `mio` only on Linux, Android, macOS and the BSDs, and `mio` pulls `windows-sys` only on Windows, so the two versions never build together on any one target. Every `notify` release has the same pair.
- With CI's own `cargo-deny` 0.20.2 (fetched by the pinned URL and checked against the pinned digest), run in a scratch directory against `feat/ORI-T-0054-watch`'s lock: `deny.toml` as it is fails bans; adding `[graph] targets` for the six supported triples still fails, because `cargo-deny` keeps an edge if any listed target matches it; one skip entry `{ crate = "windows-sys@0.61.2", reason = ... }` passes bans, licenses and sources.

**Answered by the operator in session on 2026-10-12: one version-pinned skip entry**, `windows-sys@0.61.2`, with its reason written in `deny.toml` and here. It is pinned to the version, so any other duplicate, or a later bump of either `windows-sys`, is still refused. It is not a general relaxation of `multiple-versions = "deny"`. The entry lands with ORI-T-0054's repair (ORI-T-0159), never alone.
