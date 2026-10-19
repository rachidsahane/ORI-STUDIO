# ORI-T-0185: the hook script records only when git runs it for its own event, `status()` judges links as `install()` does, and a way past a missing directory is read physically (Sonnet: the lead decided every rule below; implement it)

| Field | Value |
|---|---|
| Worktree | `/home/user/wt/ORI-T-0185` |
| Branch | `fix/ORI-T-0185-hook-links`, at `<BASE>` (the session head; its last commit records this ticket's claim 120 in `ops/lock-table.md`) |
| Tier | **1** (`spec/RISK_MAP.md`, row `crates/ori-watch (watcher, hooks)`) |
| Declared scope (claim 120) | `crates/ori-watch/src/hooks.rs` only: the script template (`HEADER`), `install`'s hooks-directory check (moved into a helper, behaviour unchanged), `status`, the chain check's handling of a missing directory, the per-process walk (refactored into a shared walk, behaviour unchanged), the docs this ticket makes false, and new tests. Nothing else in the repository. **No existing test, test helper or test comment changes** |
| Spec trailer | `Spec: ARCHITECTURE.md#5-data-flows` |
| Ticket trailer | `Ticket: ORI-T-0185`, **one per commit** |
| Commit types | only `feat`, `fix`, `docs`, `ci`, `chore`. **Never `test(...)`** |

## Why

`ops/lock-table.md` allocated this ticket with three cases. It must land before ORI-T-0055, which installs the hooks.

1. **A later checkout changes what a kept link reaches.** The install passes, and the next `git checkout other` records a false `post-merge`. The re-review's skeptic reproduced three layouts with real git, and the lead's mapping reproduced them again:
   - `post-checkout -> ../../d/.git/hooks/post-merge` while branch `other` tracks `d -> /proc/self/cwd`;
   - `post-checkout -> ../../d/post-merge` while `other` tracks `d -> .git/hooks`;
   - `post-checkout -> ../../f`, where `f` is a regular file on `main` and a link to `.git/hooks/post-merge` on `other`.

   What a checkout will put at a path cannot be known at the install.
2. **`status()` does not judge links.** A link in `ori-original/` named for a managed hook, or a link in the hooks directory, added after a clean install, makes `status()` report the chain healthy. The next `install()` refuses it.
3. **The chain check reads a way past a missing directory by its text.** It ignores a link before a `..`. So `hooks/L -> ../refs` with `post-checkout -> L/remotes/../../hooks/post-merge` passes before the first `git fetch` creates `refs/remotes`, and records a false `post-merge` on every checkout after it. The lead's mapping reproduced this with real git.

**The lead's decision on case 1.** The rule "refuse at install any hook link whose way enters the working tree" is not adopted:
- It refuses common layouts, such as a per-file link to a tracked hook script, which the whole install would then refuse.
- It still misses ways through linked worktrees made later, `core.worktree`, an outer repository and mounts.

The script checks the name it was run under instead (Decision 1). Git runs a hook as `<hooks directory>/<its event's name>`, so a script reached through a link from another event's name, or run from `ori-original/`, can tell, whatever the tree has become. That closes the false records of all three layouts, of case 3 and of case 2's links, at run time. The install-time checks stay: they keep a veto from being silently replaced, which a run-time check cannot restore.

Read first, on your base:
- `crates/ori-watch/src/hooks.rs`:
  - the module doc;
  - `install`, `status`, `refuse_if_a_link_reaches_a_managed_hook`, `refuse_a_preserved_dir_that_is_not_ours`;
  - `follow_chain_to_managed_hook` (and its long doc), `way_enters_what_the_install_creates`, `lexically_normal`, `existing_directory`, `way_enters_a_per_process_directory`, `is_per_process_directory`;
  - `HEADER`, `body_for`, `tail_for`, `render_script`;
  - the test module's helpers: `skeleton`, `repository_with_a_users_post_commit`, `expect_refused_link`, `hook_dir_names`;
  - `mod running`'s helpers: `git`, `git_ok`, `real_repo`, `log_of`.
- the lead's proof of the guard with real git 2.43 (layout B, the `ori-original` chain, the drain), all three as Decision 1 states: `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/proof-0185b/` (`mkscript.sh` is the guard in a reduced script);
- the lead's design brief, `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/brief-0185.md`: facts with line numbers and the reproductions. It is data. Where it and this ticket differ, this ticket decides; where it and the code differ, the code is the fact.

A fact-check built a prototype of this package before dispatch (`/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/fc-0185/` and `fc-0185-tests/`, data): the arm compiles, clippy is clean on Linux and on the Windows target, and the existing tests pass. Write your own implementation; do not copy it.

## Decision 1: the script records only when git runs it for its own event

In `HEADER`, change the marker line to `# ori-studio-hook v3: ...`, keeping the rest of that line.

After the definition of `ori_emit` and **before** the line `ori_head=$(git rev-parse HEAD ...)`, add this block. It is written here as shell; escape it for the Rust string the way the rest of `HEADER` is escaped (`\"` and `\\n`).

```sh
# Git runs a hook as <hooks directory>/<its event's name>. Run under another
# name, or from ori-original/, this script was reached through a link or run by
# another script, not by git for its own event: it records nothing, runs
# nothing, and lets git go on. It reads its standard input first, so a writer
# that feeds this event is never cut off.
ori_run_as=$(basename -- "$0")
ori_run_from=$(basename -- "$(dirname -- "$0")")
if [ "$ori_run_as" != "$ori_hook" ] || [ "$ori_run_from" = 'ori-original' ]; then
    printf 'ori-studio: the %s hook was run as %s, not by git for its own event; it recorded nothing and ran nothing.\n' "$ori_hook" "$(ori_clean "$0")" >&2
    [ -t 0 ] || cat >/dev/null 2>&1
    exit 0
fi
```

The rules behind it:
- **Exit 0, never another status.** Git takes a non-zero status from `post-checkout` as the checkout's own status, and from a vetoing event such as `reference-transaction` as a veto. A script reached by mistake must not block git. This follows the principle the module doc's "A hook that cannot reach the engine says so" states for an unwritable spool.
- **No `git` call before the guard.** The guard comes before `ori_head`.
- **The printed path goes through `ori_clean`**, like every value the script reads at run time.
- **Input is drained only when it is not a terminal.** Git gives a hook either a pipe it closes or `/dev/null`. A person running the script by hand at a terminal is not made to type end-of-file.
- **What the guard does not stop.** These go into the module doc as stated limits (Decision 5):
  - a script that runs the engine's hook by its own path (`"$(dirname "$0")/post-merge"`): it passes the guard and records;
  - a preserved hook that runs `"$(dirname "$0")/../post-commit"`: it passes, and records again for each round. This is the `$0`-relative change the module doc already names;
  - a process of the same user that writes the spool directly;
  - a later checkout that changes what a kept link's working-tree script does. That script runs under the hook's own name and does what the branch now says; it records falsely only by running the engine's script by its path (the first limit). Likewise, a checkout can turn a file that another script runs, by a path whose last name is a hook's (`tools/post-merge`), into a link to the engine's script. That run passes the guard and records.

  Each needs code that already runs as the user, or a change to the hooks directory itself, which `status()` and the next `install()` judge.

## Decision 2: `status()` refuses what `install()` refuses about the hooks directory and its links

- Move `install`'s check of the hooks directory (the `match fs::symlink_metadata(&hooks_dir)` block) into a free function, `fn refuse_a_hooks_dir_that_is_not_a_directory(hooks_dir: &Path) -> Result<(), HooksError>`, beside `refuse_a_preserved_dir_that_is_not_ours`. Its behaviour is exactly the block's:
  - a link, or anything not a directory, gives `HooksDirNotADirectory`;
  - absence passes;
  - any other error gives `Io`.

  `install` calls it where the block was.
- `status()` becomes, in this order:
  1. `check_git_dir`;
  2. `refuse_a_hooks_dir_that_is_not_a_directory`;
  3. `refuse_a_preserved_dir_that_is_not_ours`;
  4. `self.refuse_if_a_link_reaches_a_managed_hook(&hooks_dir)?`;
  5. the four hooks, as today.

  It does not check `core.hooksPath`: the module doc's "What is not covered" already says `status` reports installed hooks, not that git will run them.
- **`status`'s doc and `# Errors`.** It judges the hooks directory and the links in it and in `ori-original/` exactly as `install` does. A link made after an install that reaches a managed hook, or whose way depends on the process, makes it refuse rather than report the hooks healthy. Name the errors it can now return:
  - `HooksDirNotADirectory`, for the hooks directory too;
  - `HookLinksToManagedHook`;
  - `HookLinkDependsOnProcess`;
  - `Io`, when the scan cannot be made.
- **`refuse_if_a_link_reaches_a_managed_hook`'s doc.** Its first sentence becomes "Refuses the installation, or the status report, if ...". Its "(`install` refused it otherwise)" becomes "(`install` and `status` refused it otherwise)".
- **`HookStatus::chains_to_original`'s doc** says the preserved hook "is run after recording". Make it say:
  - it is run after recording when it is executable;
  - the field reports only that an entry is there.

  The computation does not change.
- **The module doc's sentence calling `status` the liveness check** (in "A hook that cannot reach the engine says so") gains: it judges the links as an install does.

## Decision 3: a way past a missing directory is read physically

**Refactor, behaviour unchanged.** Move the body of `way_enters_a_per_process_directory` into `fn walk_the_way(path: &Path, fold_case: bool) -> io::Result<WayEnd>`, with:

```rust
/// Where `walk_the_way` stopped.
enum WayEnd {
    /// It entered this per-process directory.
    PerProcess(PathBuf),
    /// Its steps ran out in this directory, which is there; the path is canonical.
    Directory(PathBuf),
    /// Its steps ran out past a component that is not there, or at a file.
    Elsewhere,
    /// It read more than `LINK_HOPS_CAP` links.
    TooManyLinks,
}
```

Each `return` of today's body maps one to one:
- every `Ok(Some(dir))` becomes `PerProcess(dir)`;
- the cap's `Ok(None)` becomes `TooManyLinks`;
- the file at the last step becomes `Elsewhere`, and so does the unreachable `_ => return Ok(None)` after a root;
- the end of the loop becomes `Directory(current)` when `missing_depth == 0`, else `Elsewhere`.

`way_enters_a_per_process_directory` keeps its name and signature, and becomes:

```rust
Ok(match walk_the_way(path, fold_case)? {
    WayEnd::PerProcess(directory) => Some(directory),
    WayEnd::Directory(_) | WayEnd::Elsewhere | WayEnd::TooManyLinks => None,
})
```

Its doc keeps what describes its answer, plus one sentence: the walk is shared with the chain check's physical reading. What describes the walk itself moves to `walk_the_way`'s doc. Every test of the per-process walk (`ori_t_0180_*`) pins its `through` values, and they must pass unchanged.

**The change, in `follow_chain_to_managed_hook`'s `NotFound` arm only.** The arm becomes the code below. Replace the comment above the arm too (Docs, below). Run `cargo fmt` after pasting: rustfmt splits the `is_same_directory(...)` call one argument per line.

```rust
Err(error) if error.kind() == io::ErrorKind::NotFound => {
    if way_enters_what_the_install_creates(parent, hooks_dir, canonical_hooks, fold_case)? {
        return Ok(Some(written));
    }
    // The way as git's process will find it, if every directory made after
    // the install is a real one: links before the missing component followed
    // where they stand, names past it taken by their text, and a `..` that
    // climbs back to a directory that is there going on physically.
    let physical = match walk_the_way(parent, fold_case)? {
        WayEnd::Directory(directory) => Some(directory),
        WayEnd::TooManyLinks => {
            return Err(io::Error::other(format!(
                "more than {LINK_HOPS_CAP} links in the way {}",
                parent.display()
            )));
        }
        WayEnd::PerProcess(_) | WayEnd::Elsewhere => None,
    };
    // The text's reading, kept as a second look that fails closed: its
    // landing is still refused if it is a hook's place.
    let lexical = lexically_normal(parent);
    let text = existing_directory(&lexical)?;
    if names_a_managed_hook(&name, fold_case)
        && is_same_directory(text.as_deref().unwrap_or(&lexical), hooks_dir, canonical_hooks)
    {
        return Ok(Some(written));
    }
    match (physical, text) {
        (Some(directory), _) | (None, Some(directory)) => (directory, true),
        (None, None) => (lexical, false),
    }
}
```

The code after the `match` is unchanged. Its check `names_a_managed_hook(&name) && is_same_directory(&directory, ...)` now judges the physical landing, and the walk goes on from it.

The rules behind it:
- **The physical landing decides where the walk goes on.** When both readings land on directories that are there and differ, the walk goes on from the physical one, the way's real directory.
- **The text keeps a one-hop look.** The text reading refuses only when its landing is a hook's place. Where the physical reading is not a directory that is there, the walk goes on from the text's landing, as today: an over-refusal, kept.
- **More than `LINK_HOPS_CAP` links in the physical reading is an error.** It refuses the install as `Io` naming the link (AICD §14). The per-process walk reads its own cap as no, and its doc already says the chain check that follows refuses such a way.
- **A per-process directory met by the physical reading is not resolved:** `walk_the_way` stops before `canonicalize`. Such a reading falls back to the text, as today. The per-process walk, which runs first on every link, has already refused any way it finds entering one.

**Docs to align**, each a passage this decision makes false:
- **The comment above the `NotFound` arm**, from "A directory of the way is not there" to "(a `..` after a link)". Replace it with: "A directory of the way is not there. If the way first fails at what this install creates, the link is refused, whatever it would reach. Otherwise the way is read physically (`walk_the_way`), and its text is read as a second look that refuses only at a hook's place. The walk goes on from the physical landing when it is a directory that is there, else from the text's."
- **`follow_chain_to_managed_hook`'s bullet (b).** A way that fails first at a directory that is not there is read as git's process will find it if that directory is later made as a real one: links before the missing component followed where they stand, names after it taken by their text, and a `..` back to a directory that is there going on physically. Its text is still read as a second look at that hop, refused only if it lands on a hook's place: an over-refusal kept because it fails closed. Give three examples, saying what each shows:
  - `nowhere/../post-merge`: both readings land on the hooks directory;
  - `hooks/L -> ../refs` with `L/remotes/../../hooks/post-merge`: only the physical reading does (the text lands on `hooks/hooks`);
  - `hooks/L -> ../refs` with `L/../nowhere/../post-merge`: only the text's second look does (the physical landing is `.git`).

  Drop the old framing of `nowhere/../post-merge` as "an over-refusal": the physical reading now reaches the hook by itself.
- **The same doc's passage on a way through a regular file** ("Behind a missing directory, a way through a regular file is read as reaching nothing, and the link passes, unless ... or its text reaches a hook, rule (b)"): "or its text reaches a hook, rule (b)" becomes "or either reading of rule (b) reaches a hook". The physical reading treats a file with steps after it as a directory that is not there.
- **`refuse_if_a_link_reaches_a_managed_hook`'s doc**, the sentences beginning "A way that first fails at any other missing directory reaches nothing, but its text is still read".
- **The module doc's paragraph beginning "The first: a link that, followed to its end"**, the clause "any other directory that does not exist yet ... means the link reaches nothing, though its text is still read and refused if that reading reaches a hook".

## Decision 4: the stated limit on the working tree

Three passages say a directory made later is a real directory, and cite a worktree directory a checkout creates:
- the module doc's paragraph beginning "The second: a link whose way enters `/proc`";
- `follow_chain_to_managed_hook`'s bullet (d);
- `way_enters_a_per_process_directory`'s "Past a component that is not there", now in `walk_the_way`'s doc. It words it "a worktree directory that a checkout creates".

A checkout can make a link or a file where a way passes (case 1). So in each:
- keep the example of a directory git makes in `.git`;
- add that the working tree is judged as it stands at the install, and a later checkout can make a way reach a hook it did not reach then;
- add that the script's own check of the name it was run under keeps such a way from recording (Decision 1), and that the next `install()` or `status()` refuses it.

## Decision 5: the module doc says what the script does when it is not run by git

Add a section after "# A hook that cannot reach the engine says so (AICD §14)", titled `# A script not run by git for its own event records nothing (AICD §13)`. It says:
- git runs a hook as `<hooks directory>/<its event's name>`;
- a script run under another name, or from `ori-original/`, prints one line on standard error, reads its standard input unless it is a terminal, records nothing, runs nothing and exits 0;
- it closes case 1's false records whatever a later checkout puts on the way of a hook git runs, and case 2's chains, at run time;
- the four limits of Decision 1, in a list.

The module doc's sentence "The generated script is a constant template with exactly two values substituted" stays true; leave it.

## Tests

All new, each beginning `ori_t_0185_`, and `#[cfg(unix)]` where they make links. **No existing test, helper or test comment changes.**

**Placement.**
- The running tests (1 to 4) go in `mod running`, on its helpers.
- Tests 5 to 15 go in the main test module, **above** the comment `// ---- the scripts, run by a real shell and by real git ----`. Never put them between `#[cfg(unix)]` and `mod running {`: the attribute would then apply to your test instead of the module, and the Windows check fails.

**Docs.** No `///` or `//!` comment names a test function in bare backticks: the doc-citation check in `crates/ori-gates/src/sections.rs` fails on an unregistered one, and its register is out of scope. If a doc must point at a test, write it as a path with `::` (`tests::running::ori_t_0185_...`), as the module doc already does, or describe it in words.

**Stale comments.** A comment of an existing test that this ticket makes stale stays; list it in your report. Expected:
- in `ori_t_0174_a_way_that_fails_at_a_directory_the_install_does_not_create_is_still_read_by_its_text`: "the way reaches nothing" and "These are over-refusals, kept". The physical reading now reaches the same hook;
- in `ori_t_0174_a_loop_where_a_missing_directory_is_read_by_its_text_refuses_the_install`: the Io now comes from the physical reading's link cap, before the text is read.

**The script (in `mod running`):**
1. `ori_t_0185_a_checkout_that_makes_a_kept_link_reach_the_engine_records_nothing`, layout B with real git:
   - `real_repo` leaves `main` unborn: commit on `main` first, then `checkout -b other`, add and commit a symbolic link `d -> .git/hooks`, and `checkout main`;
   - on `main`, make `hooks/post-checkout -> ../../d/post-merge`;
   - `install()` succeeds; this ticket does not refuse it;
   - `git checkout other` exits 0. The spool has no `post-merge` record, and git's standard error contains `not by git for its own event`.

   If git will not commit or check out that link in this environment, say so and use the regular-file layout (C) instead.
2. `ori_t_0185_a_script_run_from_ori_original_records_once`:
   - install into a real repository;
   - make `ori-original/` if it is not there, and `ori-original/post-commit -> ../post-commit`;
   - one commit exits 0, and the spool holds exactly one `post-commit` record for it.
3. `ori_t_0185_a_script_run_under_another_name_reads_all_its_input_and_exits_zero`:
   - after an install, link `hooks/not-a-hook -> pre-push`, and run it directly with `Command`, with `Stdio::piped()` input;
   - write 256 KiB with `write_all` as 256 lines of 1024 bytes each (1023 bytes of `x` and a newline). The write must succeed;
   - the status is 0, there is no new record, and standard error holds the line;
   - do the same for `post-rewrite`.

   Keep the line count at 256: under m1, `pre-push` writes one record per line, at about 18 ms each.
4. `ori_t_0185_the_script_records_under_its_own_name`: run `hooks/post-merge` directly, under its own name, with argument `0`. It records once. The existing tests cover git's own runs; this pins the guard's other side through the same `Command` path as test 3.

**`status()`:**

5. `ori_t_0185_status_refuses_a_link_added_after_an_install`:
   - after a clean install, `post-checkout -> post-merge` in the hooks directory makes `status()` give `HookLinksToManagedHook`, naming that link and its written target;
   - in a second scratch: `repository_with_a_users_post_commit`, installed, so `ori-original/` holds the kept `post-commit`; then `ori-original/pre-push -> ../post-merge`. The same, naming that entry. Use `pre-push`, not the kept `post-commit`, so no kept hook is removed.
6. `ori_t_0185_status_refuses_a_link_through_a_per_process_directory`:
   - after an install, `post-checkout -> /proc/self/cwd/.git/hooks/post-merge`;
   - match `status()`'s error directly: `HookLinkDependsOnProcess { path, through, .. }` with `through == /proc`;
   - do not use `expect_refused_for_the_process`, which is Linux-only and calls `install()`. On macOS `/proc` is missing, and the walk still gives `through == /proc`.
7. `ori_t_0185_status_refuses_a_hooks_directory_that_is_a_link_or_a_file`:
   - after an install, move the hooks directory aside and put a link to it in its place: `status()` gives `HooksDirNotADirectory { path }`, naming the hooks directory;
   - in a second scratch, put a regular file in its place: the same.
8. `ori_t_0185_status_accepts_a_link_a_first_install_kept`:
   - a user's `pre-push -> ../shared-hook.sh`, with `.git/shared-hook.sh` written as in `ori_t_0054_a_hook_that_is_a_symlink_is_someone_elses_and_is_kept_not_written_through` (which uses `post-commit`);
   - after `install()`, `status()` succeeds, and `pre-push` is `Installed` with `chains_to_original` true.

**The chain check.** `skeleton` makes only `.git`. Create `.git/hooks` yourself, `.git/refs` for tests 9 to 15, and `.git/objects/info` for test 9 (i).

9. `ori_t_0185_a_way_past_a_directory_git_makes_later_is_read_physically`:
   - layout (ii): `hooks/L -> ../refs`, `post-checkout -> L/remotes/../../hooks/post-merge`, `refs/remotes` absent. `install()` refuses with `HookLinksToManagedHook`, naming `post-checkout` and its written target, and writes nothing;
   - layout (i), in a second scratch: `.git/y -> objects/info`, `post-checkout -> ../logs/../y/../../hooks/post-merge`, `.git/logs` absent. Refused the same way.
10. `ori_t_0185_a_way_past_a_missing_directory_goes_on_from_its_physical_landing`: `hooks/L -> ../refs`, `.git/relay -> hooks/post-merge`, and `post-checkout -> L/remotes/../../relay` is refused. The text's landing, `hooks/relay`, is not there; the physical one, `.git`, holds the relay.
11. `ori_t_0185_a_physical_reading_that_meets_a_loop_refuses_as_an_io_error`: `hooks/L -> ../refs`, `.git/refs/loop -> loop`, and `post-checkout -> L/remotes/../../refs/loop/post-merge`. `install()` gives `Io` naming `post-checkout`, and writes nothing. The text's landing, `hooks/refs/loop`, is not there, so only the physical reading meets the loop.
12. `ori_t_0185_the_chain_check_reads_layout_ii_physically`, a unit test: `follow_chain_to_managed_hook(&hooks_dir, &canonical, &hooks_dir.join("post-checkout"), false)` on test 9's layout (ii) is `Ok(Some(written))`.
13. `ori_t_0185_the_text_reading_still_refuses_a_landing_on_the_hooks_directory`: `hooks/L -> ../refs`, `post-checkout -> L/../nowhere/../post-merge`. `install()` refuses with `HookLinksToManagedHook`, naming `post-checkout` and `L/../nowhere/../post-merge`, and writes nothing (`hook_dir_names` is `["L", "post-checkout"]`). The physical landing is `.git`, which holds no hook; the text landing is `.git/hooks`.
14. `ori_t_0185_a_way_whose_physical_reading_ends_past_a_missing_name_goes_on_from_its_text`: make `.git/hooks/sub/`; `hooks/L -> ../refs`, `hooks/sub/relay -> ../post-merge`, `post-checkout -> L/../sub/relay`. `install()` refuses with `HookLinksToManagedHook`, naming `post-checkout` and `L/../sub/relay`, and writes nothing (`hook_dir_names` is `["L", "post-checkout", "sub"]`). The physical reading ends at `.git/sub`, which is not there; the text lands on `.git/hooks/sub`, which holds the relay.
15. `ori_t_0185_a_text_landing_on_a_loop_refuses_as_an_io_error`: `hooks/loopdir -> loopdir`, `hooks/L -> ../refs`, `post-checkout -> L/../nowhere/../loopdir/x`. `install()` gives `Io` naming `post-checkout`, and writes nothing.
    - The physical reading climbs back from the missing `.git/nowhere` and ends at `.git/loopdir`, which is not there.
    - The text lands on `hooks/loopdir`, a loop. This keeps the text reading's own error path covered.
    - The link `loopdir` itself passes the scan, like the existing kept loop links.

## Mutation proofs

Run each in a `git archive` copy under your worktree's `target/`, with its own `CARGO_TARGET_DIR`. Edit the copy in place for each mutation and undo it afterwards: never re-extract over a built copy, because the extracted files' old mtimes make cargo skip the rebuild. Delete the copy when done.

| # | Mutation | Must fail |
|---|---|---|
| m1 | the guard tests only `ori_run_from` | tests 1, 3 |
| m2 | the guard tests only `ori_run_as` | test 2 |
| m3 | delete the drain line | test 3 |
| m4 | the guard exits 1 | test 1 (and 3) |
| m5 | `status` without the link scan | tests 5, 6 |
| m6 | `status` without the hooks-directory check | test 7 |
| m7 | the `NotFound` arm without the physical reading (`physical` always `None`) | tests 9, 10, 12 (and 11, if the `TooManyLinks` arm goes with it) |
| m8 | the arm goes on from the text's landing when both are there | test 10 |
| m9 | `TooManyLinks` maps to `None` in the arm | test 11 |
| m10 | `walk_the_way` ends `Directory(current)` whatever `missing_depth` is | test 14 |
| m11 | delete the text reading's check before `match (physical, text)` | test 13 |
| m12 | `existing_directory(&lexical)?` becomes `existing_directory(&lexical).ok().flatten()` | test 15 |

## Checks

- The commands in the **Gates.** bullet of "Standing facts" below, the Windows compile check included.
- `cargo test -p ori-watch --locked`, run twice.
- `cargo test -p ori-watch --doc --locked`.
- Gate 13 against `<BASE>`.
- Every test function, test helper and test comment that exists at `<BASE>` is byte-identical on your head. Compare brace-matched bodies with a script of your own, and report the command and its result.

Report every exit status as a number. List what is not tested and why:
- Git for Windows and its `$0` (the running tests are Unix only);
- the invocation-by-path and `..`-recursion limits, which are stated, not closed;
- what ORI-T-0055 must decide: when `install()` and `status()` are re-run, and what the engine does when either refuses.
