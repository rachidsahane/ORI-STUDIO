# looping-agent

A stand-in agent that never converges, for the end-to-end suite to run the budget and blocked-report path against: `spec/TESTING.md` section 5 ("a headless adapter that never converges, for budgets and blocked reports"), AICD §12, criterion ORI-P1-009 in `spec/criteria/phase-1.md`.

It is not a model and not a product with a specification. It is a process the engine's headless adapter (`ori-runtime`, `spec/LLD.md` section 2) launches like any non-interactive agent CLI, and which fails the attempt it is given, every time, in a way the test can name in advance.

## What serves what

| File | Serves |
|---|---|
| `agent.sh` | ORI-P1-009, "Headless adapter fails twice": the process that fails. |
| `make-repo.sh` | `spec/TESTING.md` section 5, "a repository of its own": makes this directory into one (see below). |
| `.gitattributes` | The same: keeps `agent.sh` runnable in every worktree of that repository, on Windows too. |
| `selftest.sh` | Proof of the contract below: 41 checks named `ori_p1_009_...`, under ten seconds. |
| `README.md` | This contract. |

ORI-P1-009 has five clauses. This fixture is the first only ("Headless adapter fails twice"). "Session ends Blocked", "a blocked report record exists with the two attempts", "credentials revoked" and "ticket returns to Queued on re-plan" belong to `Session::end`, `BlockedReport`, the broker and `Ticket::apply`, and the suite asserts them; the fixture only supplies two failed attempts for them to react to.

## The contract

```
sh agent.sh [--attempt N] [--delay S] [anything else]
```

| Argument | Meaning |
|---|---|
| `--attempt N` | `N` is 1 to 6 decimal digits. Echoed in the output so that two attempts can be told apart. Without it the output says `attempt=unknown`. |
| `--delay S` | `S` is 0 to 10 whole seconds. Waits `S` seconds before failing, so a wall-clock meter has something to read. Above 10 is refused. Default 0. |
| anything else | Ignored, never echoed, never evaluated. The engine passes the real prompt and its own flags (`-p "<prompt>" --dangerously-skip-permissions`, say) in argv, and they are data. |

**Standard output**, exactly, on every run that gets as far as failing (about 130 bytes, ASCII, no timestamp, no process id, so the same arguments give the same bytes):

```
looping-agent: stand-in agent, not a model (fixture ORI-T-0076)
looping-agent: attempt=<N or unknown>
looping-agent: result=not-converged
```

**Exit status**

| Status | Meaning |
|---|---|
| 3 | The attempt ran and did not converge. The only outcome of a valid invocation. The headless adapter reports it as `StopReason::Other("exit 3")`. |
| 64 | Usage error (a bad `--attempt` or `--delay`). Standard output says why; nothing else happened. |
| killed by a signal | No exit status exists. A shell reports 128 plus the signal (143 for SIGTERM); the headless adapter, which reads `ExitStatus::code()`, reports `Other("exit signal")`. |
| anything else | Not this script's doing: the shell could not start it. |

**What "fails" means for each attempt.** An attempt is one launch of the process. It fails when it exits 3 having printed the three lines above. It never exits 0 and there is nothing to configure that makes it: no argument, no environment variable and no file changes the outcome, because a stand-in that could converge is not a looping agent. Attempt 1 and attempt 2 are two such failures that differ in the one line `attempt=<N>`, which is what a blocked report listing "the two attempts" has to be able to show.

**Every way it ends is bounded.** There is no loop. The only wait is `--delay`, capped at 10 seconds, so the longest possible run is 10 seconds plus process start. A signal ends a run at once, because the script sets no trap: SIGTERM, and SIGKILL (which is what the adapter's `kill` sends on unix). A delayed run leaves behind one `sleep` of at most 10 seconds when it is killed, and that `sleep` holds none of the script's standard streams, so the caller's pipes close the moment the script dies. That matters because the adapter reads standard output to end of file before it waits for the process (see "What the existing adapter does and does not give"). The self-test runs every check under its own watchdog, so a regression that made the agent unbounded fails a check and does not hang the run.

**What it does not touch.** It makes no network call and spawns nothing but one `sleep`, and that only when `--delay` is above zero. It reads no environment variable, no file and no standard input. It writes no file and nothing to standard error. The engine injects a session's credential into its environment at spawn, so "reads no environment variable" is the point that matters: the script cannot print, store or forward a credential, because it never looks. `selftest.sh` runs it with an empty environment and with a planted variable to show both.

## How a test configures attempts = 2 against it

The budget is the ticket's, not the fixture's. The fixture has no idea how many attempts it is allowed and does not count them; it fails the one it is given. "Attempts = 2" is therefore configured where the budget lives, in the `Meter` (`ori-runtime`, `budget.rs`), and the test makes the engine, not the fixture, stop after two:

```mermaid
sequenceDiagram
  participant T as Test or engine
  participant H as HeadlessAdapter
  participant A as sh agent.sh
  participant M as Meter (attempts = 2)
  T->>H: spawn, launch sh agent.sh --attempt 1, cwd = worktree
  H->>A: process, stdin closed
  A-->>H: three lines, exit 3
  H-->>T: send returns Other("exit 3")
  T->>M: attempt_ended
  M-->>T: MayContinue
  T->>H: a new adapter, spawn --attempt 2
  A-->>H: three lines, exit 3
  H-->>T: Other("exit 3")
  T->>M: attempt_ended
  M-->>T: StopAndReport([Attempts])
  Note over T: no third launch; the session ends Blocked
```

One headless run is one attempt: the adapter is one-shot (a second `send` on the same session is refused with `AlreadyCompleted`), so each attempt is a new `HeadlessAdapter` and a new `spawn`. In code, per attempt:

```rust
let launch = LaunchConfig::headless(
    "sh",
    vec!["agent.sh".into(), "--attempt".into(), n.to_string(),
         "-p".into(), prompt.clone(), "--dangerously-skip-permissions".into()],
)?;
let handle = adapter.spawn(SessionSpec { id, launch, cwd: worktree.clone(), envs })?;
let stop = adapter.send(&handle, &prompt)?;       // StopReason::Other("exit 3")
let output = adapter.transcript().expect("recorded").entries()[0].output(); // the three lines
meter = meter.attempt_ended(now, tokens)?;        // verdict after the 2nd: StopAndReport([Attempts])
```

with `Meter::new(Budget { attempts: 2, wall_clock_s: .., tokens: .. }, started_at)`. A test that wants to catch an engine that grinds on counts its own launches: a third spawn is the failure, and the fixture would fail it identically.

This shape was run against the real `HeadlessAdapter` and `Meter` while the fixture was written (a scratch test outside the repository, since the suite is ORI-T-0077's): two attempts, each `Other("exit 3")` with its own `attempt=<N>` line, the verdict `MayContinue` after the first and `StopAndReport([Attempts])` after the second, the worktree still clean, and a planted credential-shaped environment variable absent from both transcripts.

What the fixture does not move: the tokens dimension. It reports no usage, so a test that wants tokens or wall clock to run out passes those readings to the meter itself, and `--delay` is the only real wall-clock lever.

## Becoming a repository of its own

`spec/TESTING.md` section 5 gives each fixture "a repository of its own". A `.git` directory cannot be committed inside this repository, and a submodule or a bundle would put a second history and a network fetch behind a test input, so none is used. The fixture is plain files here and becomes a repository when a test needs one:

```
sh make-repo.sh <destination>
```

copies every file of this directory to `<destination>` (which must not exist or must be empty) and commits them there as the single commit of a new repository on branch `main`. The result does not depend on the machine: a fixed identity, fixed dates, no hooks from the caller's git templates, no signing, `core.autocrlf` off, and `.gitattributes` (this directory's own, since the copy inherits nothing from this repository) forcing LF. The same bytes give the same commit hash. The engine then makes its worktree from that repository (`git worktree add`) and launches `sh agent.sh` with that worktree as the working directory; `agent.sh` is tracked, so it is there.

A test that cannot run a shell makes the same repository from its own code: copy the directory, `git init`, set the identity and `core.autocrlf=false` locally, `git add -A`, `git commit`. `make-repo.sh` is a convenience over those steps, not the definition.

## Windows

CI runs on `windows-latest`, and this fixture is a POSIX shell script. What that means, stated as what is known and what is not:

- Windows cannot start a `.sh` file as a program. Launch it the same way on every platform: program `sh`, arguments `["agent.sh", ...]`, working directory the repository or worktree. Then no path is converted and no test code is platform-specific.
- `sh` has to be on the `PATH` of the process that spawns it. The engine needs git, and Git for Windows ships `sh.exe` (under `usr\bin`), but whether that directory is on `PATH` depends on how git was installed. **Not verified**: whether `sh` is on `PATH` on `windows-latest`; this fixture has not been run on Windows at all.
- If `sh` is not found, `HeadlessAdapter::spawn` returns `RuntimeError::Io`, at once (shown on Linux with a program that does not exist; the operating system's message differs on Windows). It does not hang and it does not look like a failed attempt, so a test expecting `Other("exit 3")` fails loudly rather than passing for the wrong reason. The suite should treat that as a missing prerequisite, and it must not skip silently.
- Line endings are the other way this breaks. Git for Windows defaults to `core.autocrlf=true`, which would write CRLF into every worktree and leave a script a shell cannot run. `.gitattributes` here (`* text=auto eol=lf`, `*.sh text eol=lf`) prevents it for the repository `make-repo.sh` makes, and the Ori Studio repository's own `.gitattributes` does the same for the committed copy. `selftest.sh` checks that no script carries a carriage return.
- `Child::kill` on Windows is `TerminateProcess`: the shell dies, and the detached `sleep`, if any, runs out its own at most 10 seconds holding no pipe. That is the same as the unix case above, by the same design, but it has not been observed on Windows.
- `selftest.sh` is written for a POSIX `sh` and uses `mkfifo`, `kill -9` and `env -i`. It is not claimed to run under Git for Windows' `sh`.
- A shell-free Windows run is possible by the route `crates/ori-runtime/src/headless.rs` takes for its own tests: re-execute the test binary as the agent. That agent would have to reproduce the contract above (three lines, exit 3). It is the suite's decision, not this fixture's.

## What the existing adapter does and does not give

Checked by reading `crates/ori-runtime/src/acp.rs` and `headless.rs`, and by running the fixture through the real adapter. Nothing in `ori-runtime` was changed.

- **It can launch an arbitrary command with no code change.** `LaunchConfig::headless(program, args)` takes any non-empty program and any argv, and `SessionSpec` carries the working directory and the environment.
- **The launch is argv only.** Stdin is closed, so the prompt must be in argv; `send`'s `prompt` argument is recorded in the transcript and written nowhere. The fixture ignores both.
- **Exit status reaches the caller as a string.** A non-zero exit is `StopReason::Other("exit 3")`; there is no integer to compare.
- **Only standard output is kept**, capped at 4000 bytes; standard error is read and thrown away. The fixture's whole transcript is therefore standard output.
- **`RUN_TIMEOUT` (20 seconds) does not bound a child that is still holding its standard output.** `HeadlessAdapter::send` reads standard output to end of file before it starts the timed wait, so a child that runs for 23 seconds came back `Ok(EndTurn)` after 23.0 seconds with no `Timeout` and no defect recorded, in a scratch run against the real adapter. It also means a grandchild that inherits the pipe keeps `send` blocked after the child has exited (shown the same way: a shell that backgrounded a 4-second `sleep` and exited at once came back after 4.0 seconds). That is a defect in `ori-runtime`, not something this fixture can or should work around, so it is reported and left. What it means here: the fixture bounds itself (10 seconds at most) and detaches its one subprocess from every stream, and a test must not rely on `RUN_TIMEOUT` to cut off a misbehaving stand-in.
- **No ACP mode.** The fixture speaks no JSON-RPC, so `AcpClient` cannot drive it. The criterion names the headless adapter; an ACP stand-in would be a separate fixture.

## Self-test

```
sh selftest.sh
```

41 checks, under ten seconds, no network, writes only under a directory it makes with `mktemp` and removes. It covers each statement of the contract above and the repository recipe, and it was run against 13 planted defects in `agent.sh` (an agent that reads standard input, exits 0, writes a file, prints the environment, has no delay cap, accepts an oversized number, hangs, carries CRLF, names `curl`, ignores SIGTERM, writes to standard error, prints a process id, or sleeps in the foreground holding the caller's pipes) and every one failed the check named for it. It is not wired into CI: `.github/workflows/ci.yml` is outside this fixture's scope.

## How this repository's scanners treat this directory

Both citation scanners (`crates/ori-gates/src/sections.rs` for `AICD §n`, `crates/ori-gates/src/spec_refs.rs` for this repository's `spec/` documents) walk the whole tree and skip only `.git` and `target`; they do not skip `fixtures/`. So every `AICD §n` and every `spec/...` reference in this directory is checked against this repository's methodology index and specification, and a fixture's own `spec/` documents (this one has none) would not be indexed, because the specification index is built from the repository's root `spec/` only. References here are written to be true of this repository, and none is written about a document this fixture would carry itself.
