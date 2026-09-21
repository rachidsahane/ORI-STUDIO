#!/usr/bin/env bash
#
# gates.sh: the local gate set a coder runs before opening a pull request.
#
# Spec: spec/CI_CD.md section 1 (the fourteen required gates),
#       spec/CONVENTIONS.md "Rust" (fmt and clippy -D warnings are gates).
# Ticket: ORI-T-0004.
#
# WHAT THIS SCRIPT PROMISES
#
# For every one of the fourteen gates in CI_CD section 1 it reports exactly one
# state, and never omits a gate. A gate that is missing from a report is
# indistinguishable from a gate that passed, and that is the "present but
# reporting nothing" defect class named in AICD §39 and carried as a named
# defect class by spec/PRD.md Z-02. AICD §14 states the matching rule: a gate is
# installed only when it has been seen to fail on a planted defect.
#
# THE FIVE STATES, AND WHY THERE ARE FIVE
#
#   passed          the gate ran here and was clean.
#   failed          the gate ran here and was not clean.
#   blocked         gates.sh has a runner for this gate and this machine could
#                   not run it. Nothing checked your code. Exit 3.
#   not available   gates.sh has no runner for this gate, because none exists
#                   yet or because the gate runs on merge or in the CI matrix.
#                   The reason is produced by a live probe on every run.
#   not evaluated   the state every gate starts in. If one survives to the
#                   summary, gates.sh has a bug and exits 2 rather than pretend.
#
# "blocked" and "not available" were one state in the first version of this
# script, and that was a defect. With `rustup component add clippy` never run,
# gate 1 reported "not available" and the script exited 0 while a real rustfmt
# violation sat in the tree, because one missing prerequisite suppressed a
# sibling check that was installed and working. "I have no runner for this" and
# "your machine could not run my runner" are different answers to different
# questions, and only the first one is nobody's fault.
#
# THREE RULES THIS SCRIPT FOLLOWS, AND WHY
#
# 1. A gate command's exit status is captured directly, before its output is
#    formatted, read or truncated. Output goes to a file; the file is formatted
#    afterwards. This script never writes `if some_gate | tee log | tail -6`.
#    A shell pipeline yields the status of its LAST command, so that form reads
#    `tail`, which succeeds on every input, and reports a failed gate as a pass.
#    This project shipped exactly that defect once already. `set -o pipefail` is
#    set as well, but the file-then-format shape is the actual control: it means
#    no gate command is ever inside a pipeline in the first place.
#
# 2. The harness proves itself on every run. Before the first gate runs,
#    self_check() drives the same run_cmd machinery with a command known to
#    fail and a command known to succeed, and asserts the recorded states came
#    back failed and passed. It then drives the exit policy itself with planted
#    tallies, including "nothing ran at all", and asserts the verdict. A harness
#    that cannot tell those apart aborts the run (exit 2) instead of reporting
#    fourteen reassuring lines. This is the planted defect of AICD §14 applied
#    to the instrument and to its verdict.
#
# 3. A prerequisite probe asks the tool, not the PATH. `command -v cargo-clippy`
#    answers a question about PATH, and that is not the question: cargo finds
#    its subcommands beside its own binary as well, so on this machine
#    `command -v cargo-clippy` fails while `cargo clippy --version` prints a
#    version and works. A probe that reports a working tool as absent silences a
#    check that would have run. Each sub-check probes by running the tool's own
#    --version and capturing its status through run_cmd.
#
# EXIT STATUS, AND WHERE ITS FLOOR IS
#
#   0  every gate gates.sh can run here ran, and none failed.
#   1  at least one gate failed.
#   2  gates.sh itself could not run correctly: repository root not found,
#      harness self-check failed, exit policy failed its own check, a gate left
#      unevaluated, a state that contradicts the gate table, or the run ended
#      anywhere other than at a verdict it chose to report.
#   3  nothing failed and nothing was proven: a gate gates.sh has a runner for
#      could not run on this machine, or no gate reached a verdict at all.
#  130 interrupted by SIGINT, 143 by SIGTERM, 129 by SIGHUP. These are the
#      ordinary 128+signal statuses, kept because that is what every caller,
#      shell and CI runner already reads as "killed, not finished".
#
# Why 3 exists. This script is the last thing a coder runs before saying the
# work is done, so the one question its exit code must answer without ambiguity
# is "may I say that". Exit 0 is that permission. "I could not check" is not a
# quieter version of "I checked and it was fine"; it is a different answer, and
# giving both the same exit code is this script's own defect class one level up,
# a run that reports success for a state that is not success.
#
# Why "not available" is still not an error. Two different facts used to hide
# under that phrase, and the script now keeps them apart:
#
#   no local runner  nothing the coder does at their desk changes it. Making it
#                    an error would paint every run red forever, and a run that
#                    is always red teaches its users to ignore the exit code.
#                    An ignored exit code is a gate that has stopped gating.
#                    These gates are still printed every run, each with a reason
#                    produced by a live probe rather than a hard-coded sentence:
#                    when cargo-mutants appears on PATH the reason for gate 6
#                    changes by itself. The list cannot quietly go stale. It can
#                    still be wrong about the wiring, and was: gate 7 sat here
#                    with three hard-coded reasons until ORI-T-0016, and two of
#                    the three had been falsified by the ticket that wired it.
#                    The third, that the secret scan had no local runner, turned
#                    out to be right; gate 7 now reports it blocked rather than
#                    unavailable, because gates.sh does run something for it and
#                    hiding that behind 'not available' is the state this table
#                    exists to forbid.
#
#   blocked          gates.sh has a runner and no verdict came out of it. Two
#                    different things land here and the note tells them apart:
#                    (a) this machine could not run it, which the coder usually
#                    fixes in one command; (b) the gate is not installed in this
#                    project yet and an escalation is open on it, which the
#                    coder cannot fix at all. Gate 7's secret scan is (b),
#                    escalation E-0003, so this script exits 3 on every run
#                    until that is answered. That is the cost of not painting a
#                    two-thirds-installed gate green, and it is the right cost:
#                    an INCOMPLETE that stays until somebody decides something
#                    is a standing question, where a green gate 7 would be a
#                    standing lie. Either way the code that gate covers has been
#                    checked by nobody. Exit 3.
#
# And the floor: if no gate at all reached passed or failed, the run proved
# nothing, whatever the reasons were, so it exits 3 even when every unavailable
# gate was the harmless kind. The floor is tested separately from the blocked
# rule on purpose, so that it still holds if the gate table is ever rewired.
#
# WHY THE FLOOR IS IN A TRAP AND NOT ONLY IN verdict_for()
#
# The floor above lives at the end of main(), and a floor at the end of a
# function only holds for a run that reaches the end of that function. It did
# not. `env -u HOME scripts/gates.sh` printed one line, "HOME: unbound
# variable", and exited 0, having run no gate at all. That is the exact defect
# class of AICD §14, "a checker that exits successfully on every input", in the
# script whose whole purpose is to refuse to be that.
#
# Two separate faults produced it, and both are fixed here.
#
#   1. `set -u` aborted the script at an unguarded $HOME long before main()
#      ran, so verdict_for() was never consulted. Every expansion of a variable
#      this script does not itself assign is now guarded: $HOME, $PATH and
#      ${BASH_SOURCE[0]} were the three, and $HOME is not hypothetical, because
#      CI containers and cron environments routinely unset it, and CI is where
#      gate 8 and gate 13 run.
#
#   2. The abort's own status, 1, was then overwritten with 0 by the EXIT trap.
#      On bash 3.2, which is the system bash on macOS and the shell this script
#      runs under here, an EXIT trap that returns normally leaves the shell
#      exiting with the status of the LAST COMMAND THE TRAP RAN. The trap ended
#      in `rm -rf "$LOG_DIR"`, which succeeds, so a crashed run reported
#      success. The trap now ends in an explicit `exit`, which pins the status
#      on every bash version rather than relying on one.
#
# So the exit status is decided in exactly one place, on_exit(), and every
# deliberate exit goes through finish(), which is the only thing that tells
# on_exit() the run ended where it meant to. Anything else, an unbound
# variable, a `set -e` abort, a signal, a failed redirection, or simply falling
# off the end of the file, leaves that flag clear, and on_exit() then refuses
# to report a status that could be read as success. This is why a flag and a
# trap were chosen over, say, checking the status at each call site: a call
# site can only cover a path someone thought of, and the paths that produced
# this defect are the ones nobody thought of.
#
# SIGINT was a second, quieter instance of the same thing. The old trap ran
# cleanup on INT, which deleted the log directory, and then let the script
# CONTINUE with its log directory gone. Every later run_cmd redirection failed,
# and those failures were reported as gate states with invented reasons: an
# interrupted run printed "blocked: cargo is not on PATH" about a machine where
# cargo was on PATH. A signal now terminates the run.
#
# The floor is itself a gate, so AICD §14 applies to it: self_check() runs this
# script three times with a planted abort (an unbound variable, a failing
# command, a SIGINT) and asserts each one is non-zero, and once with --help and
# asserts that one is 0. A floor that has only ever been seen to let clean runs
# through is not installed.
#
# USAGE
#
#   scripts/gates.sh              run the gate set and print the summary
#   scripts/gates.sh --self-check run only the harness self-check and stop
#   scripts/gates.sh --help       this text, abbreviated
#
# Safe to run from any directory inside the repository, and safe to run
# repeatedly: it builds into the normal cargo target directory and changes no
# tracked file. Gate output goes to a temporary directory, which is removed on a
# clean run and kept, with its path printed, when a gate failed or was blocked.

set -euo pipefail

# ---------------------------------------------------------------------------
# The floor. Installed before anything else can fail, because what it exists to
# catch is a failure that happens before the rest of the script runs.
#
# Everything on_exit() reads is initialised here, above the trap, so that the
# trap cannot itself trip `set -u` on its way to reporting that something
# tripped `set -u`.
# ---------------------------------------------------------------------------

STAGE='startup, before the repository root was resolved'
EXIT_DECIDED=0
SIGNAL_NAME=''
LOG_DIR=''
KEEP_LOGS=0
C_RESET=''; C_BOLD=''; C_DIM=''
C_GREEN=''; C_RED=''; C_YELLOW=''

# finish: the only way this script exits on purpose. Marking the exit as
# decided is what separates "gates.sh reported a verdict of N" from "gates.sh
# stopped running and N is whatever happened to be in $? at the time".
finish() {
    EXIT_DECIDED=1
    exit "$1"
}

on_exit() {
    local status=$?
    set +e
    trap - EXIT INT TERM HUP

    if [ "$EXIT_DECIDED" -ne 1 ]; then
        KEEP_LOGS=1
        printf '\n%sgates.sh: this run ended without reaching a verdict.%s\n' "$C_RED" "$C_RESET"
        if [ -n "$SIGNAL_NAME" ]; then
            printf '  Interrupted by SIG%s during: %s\n' "$SIGNAL_NAME" "$STAGE"
        else
            printf '  Stopped during: %s\n' "$STAGE"
            printf '  The line above this banner, if any, is the shell error that stopped it.\n'
            # The run did not choose to end here, so whatever status was in
            # flight is not a verdict and must not be reported as one. 2 is this
            # script's "gates.sh itself could not run correctly", which is
            # exactly what happened.
            status=2
        fi
        # Deliberately not "nothing was checked": some gates may well have run
        # and passed before this. What is true in every case is that no verdict
        # was reached, so whatever was printed above is partial and the run
        # grants no permission to open a pull request.
        printf '  %sNo verdict was reached, so anything printed above is partial.%s\n' "$C_RED" "$C_RESET"
        printf '  %sThis run is not a pass. Do not read it as one.%s\n' "$C_RED" "$C_RESET"
    fi

    if [ -n "$LOG_DIR" ] && [ -d "$LOG_DIR" ]; then
        if [ "$KEEP_LOGS" -eq 1 ] && [ -n "$(ls -A "$LOG_DIR" 2>/dev/null)" ]; then
            printf '\n  Full logs are kept at: %s\n' "$LOG_DIR"
        else
            rm -rf "$LOG_DIR"
        fi
    fi

    exit "$status"
}
trap on_exit EXIT

# A signal keeps its ordinary 128+n status rather than being folded into 2,
# because "killed" and "broken" are different facts and every caller already
# distinguishes them. The handler exits: the old one cleaned up and let the
# script carry on with its log directory deleted.
on_signal() {
    SIGNAL_NAME="$1"
    exit "$2"
}
trap 'on_signal INT 130' INT
trap 'on_signal TERM 143' TERM
trap 'on_signal HUP 129' HUP

# ---------------------------------------------------------------------------
# Presentation
# ---------------------------------------------------------------------------

# The colour variables were initialised empty above the floor trap, so a
# terminal only ever turns them on; nothing here can leave one unset.
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_DIM=$'\033[2m'
    C_GREEN=$'\033[32m'; C_RED=$'\033[31m'; C_YELLOW=$'\033[33m'
fi

say()  { printf '%s\n' "$*"; }
head1() { printf '\n%s%s%s\n' "$C_BOLD" "$*" "$C_RESET"; }
dim()  { printf '%s%s%s\n' "$C_DIM" "$*" "$C_RESET"; }

# ---------------------------------------------------------------------------
# Planted aborts for the floor. AICD §14: a gate enters service only after it
# has been seen to fail on a planted defect. The floor is a gate over this
# script's own exit status, so self_check() runs this same file with each of
# these and asserts the status is not 0.
#
# Handled here, at the top level and before the log directory is created, for
# two reasons: the planted child must abort on the real trap rather than on a
# copy of it that could drift from it, and it must leave nothing behind.
# ---------------------------------------------------------------------------

plant_abort_if_asked() {
    case "${1:-}" in
        --plant-abort=unbound)
            STAGE='planted abort: an unset variable expanded under set -u'
            say "planted abort: expanding a variable that is certainly unset"
            # unset makes this independent of the caller's environment, which
            # is the whole point: $HOME was unset by the caller, not by us.
            unset ORI_GATES_PLANTED_UNSET_PROBE
            say "$ORI_GATES_PLANTED_UNSET_PROBE"
            say "planted abort: set -u did not abort; the floor was not exercised"
            ;;
        --plant-abort=errexit)
            STAGE='planted abort: a command failing under set -e'
            say "planted abort: running a command that fails"
            false
            say "planted abort: set -e did not abort; the floor was not exercised"
            ;;
        --plant-abort=signal)
            STAGE='planted abort: SIGINT delivered to this process'
            say "planted abort: sending SIGINT to this process"
            kill -INT $$
            say "planted abort: SIGINT was not delivered; the floor was not exercised"
            ;;
    esac
}

plant_abort_if_asked "$@"

# ---------------------------------------------------------------------------
# Repository root: resolved from this script's own location, so the script
# works from any working directory and does not depend on git being usable.
# ---------------------------------------------------------------------------

# ${BASH_SOURCE[0]} is guarded because it is genuinely unbound when this file
# is fed to bash on standard input, and an unbound expansion here would abort
# the script in the one function whose job is to tell it where it is.
resolve_self_path() {
    local src="${BASH_SOURCE[0]:-$0}" dir
    # Follow symlinks to the real script.
    while [ -L "$src" ]; do
        dir=$(cd -P "$(dirname "$src")" && pwd)
        src=$(readlink "$src")
        case "$src" in
            /*) ;;
            *) src="$dir/$src" ;;
        esac
    done
    dir=$(cd -P "$(dirname "$src")" && pwd)
    printf '%s/%s\n' "$dir" "$(basename "$src")"
}

SELF_PATH=$(resolve_self_path)
REPO_ROOT=$(cd -P "$(dirname "$SELF_PATH")/.." && pwd)
STAGE='startup, after the repository root was resolved'

if [ ! -f "$REPO_ROOT/Cargo.toml" ] || [ ! -f "$REPO_ROOT/spec/CI_CD.md" ]; then
    say "gates.sh: cannot locate the repository root."
    say "gates.sh: resolved '$REPO_ROOT', which has no Cargo.toml and no spec/CI_CD.md."
    say "gates.sh: keep this script at scripts/gates.sh in the repository."
    finish 2
fi

cd "$REPO_ROOT"

# The log directory. Its cleanup lives in on_exit() with the floor, so that one
# handler owns both and a signal cannot delete the directory out from under a
# run that then keeps going. A failure here is a failure of gates.sh itself.
if ! LOG_DIR=$(mktemp -d "${TMPDIR:-/tmp}/ori-gates.XXXXXX" 2>/dev/null); then
    LOG_DIR=''
    say "gates.sh: could not create a temporary directory under '${TMPDIR:-/tmp}'."
    say "gates.sh: nothing can be run without somewhere to put gate output."
    finish 2
fi

# cargo is commonly installed outside the default PATH. Add the standard
# location if, and only if, cargo is not already reachable, and say so.
#
# $HOME and $PATH are both guarded. $HOME is unset in CI containers and in some
# cron environments, and expanding it bare under `set -u` is what aborted this
# script before it had run a single gate. An empty $HOME is treated as unset
# too, because "/.cargo/bin/cargo" is a path this script has no business
# probing. $PATH is guarded so that an empty one cannot become a trailing colon,
# which bash reads as the current directory and which would let a file called
# cargo in the working tree be run as the toolchain.
CARGO_PATH_NOTE=''
if ! command -v cargo >/dev/null 2>&1; then
    if [ -n "${HOME:-}" ] && [ -x "${HOME}/.cargo/bin/cargo" ]; then
        if [ -n "${PATH:-}" ]; then
            PATH="${HOME}/.cargo/bin:${PATH}"
        else
            PATH="${HOME}/.cargo/bin"
        fi
        export PATH
        CARGO_PATH_NOTE="added \$HOME/.cargo/bin to PATH for this run"
    elif [ -z "${HOME:-}" ]; then
        CARGO_PATH_NOTE='cargo is not on PATH and HOME is unset or empty, so the usual $HOME/.cargo/bin fallback was not tried; gates 1 and 2 report blocked below'
    fi
fi

# ---------------------------------------------------------------------------
# The gate table. Fourteen entries, matching spec/CI_CD.md section 1 one to one.
# Index 1 to 14.
#
# GATE_RUNNER records whether gates.sh has a runner for the gate. It is a fact
# about this script, not about this machine, and it is what separates "blocked"
# from "not available": only a gate whose runner is 'local' can be blocked, and
# only a gate whose runner is 'elsewhere' can be not available.
#
# Every entry starts as "not evaluated" on purpose: nothing in this script may
# default a gate to a passing or benign state.
# ---------------------------------------------------------------------------

GATE_COUNT=14

GATE_NAME[1]='fmt, clippy -D warnings'
GATE_NAME[2]='cargo test, workspace wide'
GATE_NAME[3]='contract tests'
GATE_NAME[4]='coverage matrix, criteria to tests'
GATE_NAME[5]='modified-test detector'
GATE_NAME[6]='mutation score threshold'
GATE_NAME[7]='cargo-audit, cargo-deny, secret scan (secret scan NOT installed, E-0003)'
GATE_NAME[8]='forbidden-action test, fixtures/new-product'
GATE_NAME[9]='citation gate, every AICD section reference resolves'
GATE_NAME[10]='UI type check, lint, unit tests, build'
GATE_NAME[11]='build of all three platform binaries'
GATE_NAME[12]='significance labeler'
GATE_NAME[13]='commit-trailer gate'
GATE_NAME[14]='diagram gate, Mermaid source under spec/'

i=1
while [ "$i" -le "$GATE_COUNT" ]; do
    GATE_STATE[$i]='not evaluated'
    GATE_NOTE[$i]='this script failed to evaluate this gate, which is a bug in gates.sh'
    GATE_RUNNER[$i]='elsewhere'
    i=$((i + 1))
done

GATE_RUNNER[1]='local'
GATE_RUNNER[2]='local'
GATE_RUNNER[7]='local'
# Gate 13 has a local runner as of ORI-T-0017, and CI runs the same one:
# `.github/workflows/ci.yml`'s `gate-13` job invokes this script with
# `--commit-trailers`. There is one implementation and three callers.
GATE_RUNNER[13]='local'

set_state() { GATE_STATE[$1]="$2"; GATE_NOTE[$1]="$3"; }

append_note() { GATE_NOTE[$1]="${GATE_NOTE[$1]}; $2"; }

# Facts measured during this run and reused by later gates, so that a gate's
# reason is drawn from what this run actually observed. 'unknown' is the honest
# starting value, and no gate may read it as a number.
TESTS_RAN='unknown'

# ---------------------------------------------------------------------------
# run_cmd: run a command, capture ITS exit status, then format its output.
#
# The redirection to a file means the command is not part of any pipeline, so
# its status is its own. `|| status=$?` captures that status while keeping
# set -e from aborting the script. Nothing reads or trims the output until the
# status is already recorded in RUN_STATUS.
# ---------------------------------------------------------------------------

RUN_STATUS=0
RUN_LOG=''

run_cmd() {
    local label="$1"; shift
    # If the log directory is gone, the redirection below fails and its status,
    # 1, would be recorded as the gate command's own status and reported as a
    # gate result with a reason that never happened. An interrupted run used to
    # do exactly that and print "cargo is not on PATH" about a machine where
    # cargo was on PATH. There is no honest gate state for this, so it is what
    # it is: gates.sh cannot run.
    if [ -z "$LOG_DIR" ] || [ ! -d "$LOG_DIR" ]; then
        say "gates.sh: the log directory '$LOG_DIR' is gone mid-run, so no command status can be trusted."
        finish 2
    fi
    RUN_LOG="$LOG_DIR/$label.log"
    RUN_STATUS=0
    "$@" >"$RUN_LOG" 2>&1 || RUN_STATUS=$?
    return 0
}

show_output() {
    # Formatting only, and only ever after RUN_STATUS has been recorded.
    # Called for a failure or a block, so it also pins the log directory so the
    # coder can read the whole thing.
    local log="$1" lines="${2:-40}"
    KEEP_LOGS=1
    say ""
    dim "    last $lines lines of output:"
    tail -n "$lines" "$log" 2>/dev/null | sed 's/^/    | /' || true
    dim "    full log: $log"
}

# ---------------------------------------------------------------------------
# The exit policy, as one function, so that it can be driven with planted
# tallies by the self-check below and cannot drift from the verdict main()
# prints. See "EXIT STATUS, AND WHERE ITS FLOOR IS" in the header for the
# reasoning behind each line.
# ---------------------------------------------------------------------------

verdict_for() {
    local passed="$1" failed="$2" blocked="$3" unevaluated="$4"
    if [ "$unevaluated" -ne 0 ]; then printf '2'; return 0; fi
    if [ "$failed" -ne 0 ]; then printf '1'; return 0; fi
    # The floor, tested before the blocked rule and independently of it: a run
    # in which no gate reached a verdict proved nothing at all.
    if [ "$((passed + failed))" -eq 0 ]; then printf '3'; return 0; fi
    if [ "$blocked" -ne 0 ]; then printf '3'; return 0; fi
    printf '0'
}

# ---------------------------------------------------------------------------
# Harness self-check. AICD §14: a gate enters service only after it has been
# seen to fail on a planted defect. The planted defects here are a command that
# fails quietly, a command that fails loudly after printing many lines, a
# command that succeeds, and a set of tallies that must not be read as success.
# If run_cmd or verdict_for cannot tell them apart, every line this script
# prints afterwards is worthless, so the run stops.
# ---------------------------------------------------------------------------

self_check() {
    local failures=0

    run_cmd selfcheck-pass true
    if [ "$RUN_STATUS" -ne 0 ]; then
        say "  harness self-check: a succeeding command was recorded as status $RUN_STATUS"
        failures=$((failures + 1))
    fi

    run_cmd selfcheck-fail false
    if [ "$RUN_STATUS" -eq 0 ]; then
        say "  harness self-check: a failing command was recorded as a pass"
        failures=$((failures + 1))
    fi

    # The one that matters: a command that produces a lot of output and then
    # fails. This is the shape that a `cmd | tee | tail` check swallows.
    run_cmd selfcheck-noisy-fail sh -c 'i=0; while [ $i -lt 200 ]; do echo "line $i"; i=$((i+1)); done; exit 3'
    if [ "$RUN_STATUS" -ne 3 ]; then
        say "  harness self-check: a noisy command that exited 3 was recorded as status $RUN_STATUS"
        say "  harness self-check: the gate command's status is being lost to its output handling"
        failures=$((failures + 1))
    fi

    # Planted tallies for the exit policy. Each is "passed failed blocked
    # unevaluated:expected exit code", and each is a state a reader must not be
    # able to mistake for "everything is fine".
    local c args want got
    for c in \
        '2 0 0 0:0' \
        '1 1 0 0:1' \
        '1 0 1 0:3' \
        '0 0 2 0:3' \
        '0 0 0 0:3' \
        '13 0 0 1:2' \
        '0 1 1 1:2'
    do
        args="${c%%:*}"
        want="${c##*:}"
        # shellcheck disable=SC2086
        got=$(verdict_for $args)
        if [ "$got" != "$want" ]; then
            say "  harness self-check: exit policy on tallies ($args) returned $got, expected $want"
            failures=$((failures + 1))
        fi
    done

    # Planted defects for the floor itself. Each runs this very script, on this
    # machine, with a deliberate abort, and asserts the status is the one the
    # floor promises. AICD §14: a gate is installed only once it has been seen
    # to fail on a planted defect, and the floor is a gate over this script's
    # own exit status. The --help case is the other half of that rule, the
    # demonstration that it passes on a clean tree: a floor that returned
    # non-zero for everything would satisfy the three aborts and mean nothing.
    local plant kind want
    for plant in \
        'unbound:2' \
        'errexit:2' \
        'signal:130' \
        'help:0'
    do
        kind="${plant%%:*}"
        want="${plant##*:}"
        if [ "$kind" = 'help' ]; then
            run_cmd 'selfcheck-floor-help' bash "$SELF_PATH" --help
        else
            run_cmd "selfcheck-floor-$kind" bash "$SELF_PATH" "--plant-abort=$kind"
        fi
        if [ "$RUN_STATUS" != "$want" ]; then
            if [ "$kind" = 'help' ]; then
                say "  harness self-check: a clean run of this script exited $RUN_STATUS, expected $want"
                say "  harness self-check: the floor is returning non-zero for everything, which proves nothing"
            else
                say "  harness self-check: a run aborted by a planted '$kind' exited $RUN_STATUS, expected $want"
                if [ "$RUN_STATUS" -eq 0 ]; then
                    say "  harness self-check: an aborted run is reporting success; the floor is bypassable"
                fi
            fi
            failures=$((failures + 1))
        fi
    done

    if [ "$failures" -ne 0 ]; then
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Probes for the gates gates.sh has no runner for. missing_cmds and
# missing_paths print what is absent and nothing when everything is present;
# join_reason turns that into the sentence the summary prints. Every reason is
# produced here, at run time, from the state of this machine and this tree. None
# is a constant sentence that could outlive the condition it describes.
# ---------------------------------------------------------------------------

missing_cmds() {
    local missing='' c
    for c in "$@"; do
        command -v "$c" >/dev/null 2>&1 || missing="$missing $c"
    done
    printf '%s' "${missing# }"
}

missing_paths() {
    local missing='' p
    for p in "$@"; do
        [ -e "$REPO_ROOT/$p" ] || missing="$missing $p"
    done
    printf '%s' "${missing# }"
}

join_reason() {
    # join_reason "<what was missing>" "<kind>" -> a reason sentence, or the
    # "no runner wired" sentence when nothing was missing.
    local missing="$1" kind="$2"
    if [ -n "$missing" ]; then
        printf '%s not present: %s' "$kind" "$missing"
    else
        printf 'prerequisites are present, but gates.sh has no runner wired for this gate yet; wire it here'
    fi
}

# A gate with no local runner is "not available" whether or not its
# prerequisites happen to be installed. Reporting "prerequisites are present, no
# runner wired" is the deliberate design: the day the tooling lands, the reason
# on this line changes by itself, and the change is the prompt to finish the
# gate. It never turns into a silent pass.

probe_unavailable() {
    # $1 = gate index, $2 = kind label, rest = "cmd:NAME" or "path:REL" tokens
    local idx="$1" kind="$2"; shift 2
    local cmds='' paths='' tok
    for tok in "$@"; do
        case "$tok" in
            cmd:*)  cmds="$cmds ${tok#cmd:}" ;;
            path:*) paths="$paths ${tok#path:}" ;;
        esac
    done
    local missing=''
    if [ -n "$cmds" ]; then
        # shellcheck disable=SC2086
        missing=$(missing_cmds $cmds)
    fi
    local mp=''
    if [ -n "$paths" ]; then
        # shellcheck disable=SC2086
        mp=$(missing_paths $paths)
    fi
    if [ -n "$mp" ]; then
        if [ -n "$missing" ]; then missing="$missing $mp"; else missing="$mp"; fi
    fi
    set_state "$idx" 'not available' "$(join_reason "$missing" "$kind")"
}

# ---------------------------------------------------------------------------
# Gate 13: the commit-trailer gate.
#
# spec/CI_CD.md section 1 item 13: "every commit on the PR: Conventional
# Commits, `Ticket: <id>` and `Spec: <document>#<section>` trailers, per
# CONVENTIONS; PRD G-02". spec/CONVENTIONS.md "Git". spec/PRD.md G-02.
# Ticket: ORI-T-0017. Proof: fixtures/planted/gate-13/, ops/gates/gate-13.md.
#
# THE IMPLEMENTATION LIVES HERE, IN ONE PLACE, AND CI CALLS IT.
#
# `.github/workflows/ci.yml` runs `bash scripts/gates.sh --commit-trailers`
# and `fixtures/planted/gate-13/prove.sh` runs the same command against
# planted repositories. There is exactly one implementation of this gate and
# all three callers reach it. Ruling R28 is the reason it is not under
# `fixtures/planted/gate-13/`: a fixture directory holds inputs a gate is run
# against, never the checker the gate invokes. A second copy inline in the
# workflow would be the other half of the same defect, a gate that can be
# repaired in the copy CI does not run.
#
# WHY IT USES GIT'S OWN TRAILER PARSER AND NEVER A REGEX OVER THE MESSAGE.
#
# This is the whole point of the gate and it is not a style preference. Git
# parses trailers out of the LAST PARAGRAPH of a commit message and nothing
# else. A message written like this:
#
#     feat(gates): a subject
#
#     A body paragraph.
#
#     Ticket: ORI-T-0017
#     Spec: CI_CD.md#1-pipeline-on-every-pull-request
#
#     Co-Authored-By: Someone <someone@example.com>
#
# has both required lines, correctly spelled, visible to any human and to any
# regex. Git parses NEITHER of them: `git log --format='%(trailers:key=Ticket)'`
# returns empty, because the trailer block is the last paragraph and the last
# paragraph here holds only `Co-Authored-By`. Everything downstream that reads
# trailers, the traceability chain of AICD §13, changelog generation, the audit
# chain, sees nothing.
#
# A regex-based gate passes that commit. It would confirm the trailers are
# present while the mechanism that consumes them sees nothing, which is AICD
# §39's "present but reporting nothing" sitting inside the gate that guards the
# audit trail. So every trailer question below is asked of
# `%(trailers:only=true,unfold=true)`, which is git's own parser, and the
# answer is whatever git says.
#
# A regex over the message text IS used, in exactly one direction and for
# exactly one purpose: when git reports no `Ticket:` trailer and the message
# nevertheless contains a `Ticket:`-shaped line, the refusal is reported as
# TICKET_UNPARSED rather than TICKET_MISSING, so the author is told the line is
# in the wrong paragraph instead of being told it is absent. That regex can
# only ever turn one refusal into a better-explained refusal. No path exists
# from it to a pass, and `fixtures/planted/gate-13/prove.sh` asserts that.
#
# WHAT IS ENFORCED, AND WHICH PART OF IT IS A DECISION
#
# Structure, from Conventional Commits v1.0.0 itself:
#   <type>[(<scope>)][!]: <description>
#   one colon, one space, a non-empty description that does not begin with a
#   further space.
#
# Vocabulary, and this is the one decision this gate makes:
#   the type is one of `feat fix docs ci chore` and the scope, when present,
#   matches [a-z0-9-]+. Conventional Commits v1.0.0 mandates only `feat` and
#   `fix` and leaves the rest to the project; spec/CONVENTIONS.md says
#   "Conventional Commits" and names no list. So the list is a repository
#   decision and the only evidence of that decision is what the repository
#   does: of the 23 commits on `main` at ORI-T-0017, 22 carry a conventional
#   subject and they use exactly those five types (docs 8, feat 7, ci 4, fix 2,
#   chore 1) and nine scopes, all matching [a-z0-9-]+. A closed set is chosen
#   over an open [a-z]+ because an open set accepts `wip:`, `misc:` and
#   `update:`, which is the vocabulary rot Conventional Commits exists to
#   prevent. The cost is stated rather than hidden: the first legitimate
#   `refactor:` or `test:` commit fails this gate, and the fix is one word
#   added to CT_TYPES below in a ticket. That is the deliberate decision an
#   open set would let past in silence.
#
# WHAT IS DELIBERATELY NOT ENFORCED, so that nobody reads a green gate 13 as
# more than it is:
#   - subject length. CONVENTIONS names none and Conventional Commits names
#     none. The longest subject on `main` is 79 characters.
#   - capitalisation of the description, and a trailing full stop. Both are
#     Angular-convention habits that all 22 conventional commits happen to
#     follow, and neither is in Conventional Commits v1.0.0 or in CONVENTIONS.
#     A gate that fails a commit for a rule no specification states is a gate
#     inventing policy.
#   - that the `Spec:` anchor resolves. See ct_check_commit: the document half
#     is OBSERVED and printed, never judged. The reasons are in ops/gates/gate-13.md.
#   - that a subject paragraph is one line. `%s` is what every consumer
#     displays and `%s` collapses a wrapped subject paragraph into one line, so
#     this gate judges what the consumer sees.
# ---------------------------------------------------------------------------

# The vocabulary. One source for the subject pattern and for the message that
# is printed when a type is refused, so the two cannot drift.
CT_TYPES='feat fix docs ci chore'
CT_SCOPE_RE='^[a-z0-9-]+$'
CT_TICKET_RE='^ORI-T-[0-9][0-9][0-9][0-9]$'
# `<document>#<section>`, with the document a relative path under spec/ ending
# in .md and the section a non-empty GitHub-style anchor. No leading slash and
# no `..`: a trailer is a pointer into the specification, not into a filesystem.
CT_SPEC_RE='^[A-Za-z0-9_-]+(/[A-Za-z0-9_-]+)*\.md#[A-Za-z0-9][A-Za-z0-9._-]*$'

# The subject grammar of Conventional Commits v1.0.0, as a bash ERE. The type
# and scope are captured rather than constrained here, so that a subject with
# the right SHAPE and the wrong vocabulary is reported as a vocabulary problem
# and not as an unreadable subject. Telling those two apart is what makes a
# refusal actionable.
CT_SUBJECT_RE='^([A-Za-z0-9_]+)(\(([^)]+)\))?(!)?: (.+)$'

# Under GitHub Actions a message a human must see is additionally emitted as a
# workflow annotation, which is what puts it on the pull request check rather
# than only in a log somebody has to open. spec/runbooks/prove-gate.md step 3.
ct_emit() {
    local kind="$1"; shift
    printf '%s: %s\n' "$kind" "$*"
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        printf '::%s file=scripts/gates.sh::%s\n' "$kind" "$*"
    fi
}

# --- one commit ------------------------------------------------------------
#
# Sets CT_CODES to a space-separated list of reason codes, empty when the
# commit conforms, and CT_DETAIL to the human sentences behind them. Every
# code names the mechanism that produced it, so that a planted defect can be
# attributed to the thing planted in it rather than to "the gate said no".
#
#   SUBJECT_SHAPE     the subject is not <type>[(scope)][!]: <description>
#   SUBJECT_TYPE      the shape is right and the type is not one of CT_TYPES
#   SUBJECT_SCOPE     the shape is right and the scope is not [a-z0-9-]+
#   TICKET_MISSING    git's trailer parser reports no Ticket: trailer, and the
#                     message contains no Ticket:-shaped line either
#   TICKET_UNPARSED   git's trailer parser reports none and the message DOES
#                     contain one: it is not in the last paragraph, so the
#                     mechanism that consumes trailers cannot see it
#   TICKET_CASE       git parsed a trailer whose key is a case variant of
#                     Ticket. Git's own lookup is case-insensitive, so this
#                     one is consumed; CONVENTIONS writes `Ticket:` and this
#                     gate enforces that spelling, because the trailer is also
#                     read by humans and by tools that are not git
#   TICKET_DUPLICATE  more than one Ticket: trailer, so its value is ambiguous
#   TICKET_VALUE      present, single, and not ORI-T-NNNN
#   SPEC_*            the same five, for the Spec: trailer
CT_CODES=''
CT_DETAIL=''
ct_check_commit() {
    local repo="$1" sha="$2"
    local subject message trailers status
    local type scope desc
    local key n value
    CT_CODES=''
    CT_DETAIL=''

    ct_note() { CT_CODES="${CT_CODES:+$CT_CODES }$1"; CT_DETAIL="${CT_DETAIL}      - $2"$'\n'; }

    subject=$(git -C "$repo" show -s --format='%s' "$sha" 2>/dev/null) || subject=''
    message=$(git -C "$repo" show -s --format='%B' "$sha" 2>/dev/null) || message=''

    # Git's own parser. Its exit status is captured before anything reads the
    # output, because a git that does not understand this format string would
    # otherwise hand an empty string to every check below and every commit
    # would be reported as carrying no trailers at all: a gate failing on
    # everything, which is as useless as one passing on everything and rather
    # more convincing.
    trailers=$(git -C "$repo" show -s --format='%(trailers:only=true,unfold=true)' "$sha" 2>/dev/null)
    status=$?
    if [ "$status" -ne 0 ]; then
        CT_CODES='HARNESS'
        CT_DETAIL="      - git could not read the trailers of $sha (exit $status). This git may not understand '%(trailers:only=true,unfold=true)', in which case nothing below would mean anything."$'\n'
        return 0
    fi

    # --- the subject -------------------------------------------------------
    if [[ "$subject" =~ $CT_SUBJECT_RE ]]; then
        type="${BASH_REMATCH[1]}"
        scope="${BASH_REMATCH[3]}"
        desc="${BASH_REMATCH[5]}"
        case "$desc" in
            ' '*) ct_note SUBJECT_SHAPE "the description begins with a further space: Conventional Commits puts exactly one space after the colon" ;;
        esac
        case " $CT_TYPES " in
            *" $type "*) ;;
            *) ct_note SUBJECT_TYPE "the type is '$type' and this repository's types are: $CT_TYPES. Adding one is a deliberate decision, made by editing CT_TYPES in scripts/gates.sh in a ticket" ;;
        esac
        if [ -n "$scope" ] && ! printf '%s\n' "$scope" | grep -Eq "$CT_SCOPE_RE"; then
            ct_note SUBJECT_SCOPE "the scope is '$scope' and a scope here matches $CT_SCOPE_RE"
        fi
    else
        ct_note SUBJECT_SHAPE "the subject is not '<type>[(<scope>)][!]: <description>': [$subject]"
    fi

    # --- the trailers, asked of git and of nothing else --------------------
    for key in Ticket Spec; do
        # One line per trailer, the original spelling of the key preserved,
        # folded continuations unfolded onto their own trailer line exactly as
        # a consumer reading %(trailers:key=...,valueonly) would receive them.
        n=$(printf '%s\n' "$trailers" | grep -c "^$key:" 2>/dev/null) || n=0
        if [ "$n" -eq 0 ]; then
            # Three different states hide behind "git reports none", and a
            # coder can only act on the one they are actually in.
            if printf '%s\n' "$trailers" | grep -qi "^$key:"; then
                ct_note "$(printf '%s' "$key" | tr '[:lower:]' '[:upper:]')_CASE" \
                    "git parsed a trailer whose key is a case variant of '$key'. Git's own lookup is case-insensitive so that one is consumed, but spec/CONVENTIONS.md writes '$key:' and this gate enforces that spelling"
            elif printf '%s\n' "$message" | grep -Eq "^[[:space:]]*$key:"; then
                ct_note "$(printf '%s' "$key" | tr '[:lower:]' '[:upper:]')_UNPARSED" \
                    "the message contains a '$key:' line and GIT'S TRAILER PARSER DOES NOT SEE IT. Git reads trailers out of the LAST PARAGRAPH of the message only. Move the '$key:' line into the final paragraph, beside Co-Authored-By, with no blank line between them. Until then every tool that reads trailers, the traceability chain of AICD §13 among them, sees nothing here"
            else
                ct_note "$(printf '%s' "$key" | tr '[:lower:]' '[:upper:]')_MISSING" \
                    "no '$key:' trailer. spec/CONVENTIONS.md 'Git': every message ends with 'Ticket: <id>' and 'Spec: <document>#<section>'"
            fi
            continue
        fi
        if [ "$n" -gt 1 ]; then
            ct_note "$(printf '%s' "$key" | tr '[:lower:]' '[:upper:]')_DUPLICATE" \
                "$n '$key:' trailers, so the value a consumer reads is ambiguous"
            continue
        fi
        value=$(printf '%s\n' "$trailers" | sed -n "s/^$key:[[:space:]]*//p" | head -1)
        case "$key" in
            Ticket)
                if ! printf '%s\n' "$value" | grep -Eq "$CT_TICKET_RE"; then
                    ct_note TICKET_VALUE "the Ticket trailer's value is [$value] and a ticket identifier here matches $CT_TICKET_RE"
                fi
                ;;
            Spec)
                if ! printf '%s\n' "$value" | grep -Eq "$CT_SPEC_RE"; then
                    ct_note SPEC_VALUE "the Spec trailer's value is [$value] and spec/CONVENTIONS.md 'Git' asks for '<document>#<section>', matching $CT_SPEC_RE"
                else
                    # OBSERVED, NEVER JUDGED. ops/gates/gate-13.md argues the
                    # case; the short form is that the anchor half cannot be
                    # resolved by anything that exists yet, so a check of the
                    # document alone would be green while the anchor pointed
                    # nowhere, and that a commit whose cited document is
                    # renamed later in the same pull request could never be
                    # repaired, because CONVENTIONS forbids rewriting history.
                    local doc="${value%%#*}"
                    if [ -f "$repo/spec/$doc" ]; then
                        CT_DETAIL="${CT_DETAIL}      - observed, not judged: spec/$doc exists in the tree this run checked out"$'\n'
                    else
                        CT_DETAIL="${CT_DETAIL}      - observed, not judged: spec/$doc is NOT in the tree this run checked out. This gate does not fail a commit for it; see ops/gates/gate-13.md"$'\n'
                    fi
                fi
                ;;
        esac
    done
    return 0
}

# --- which commits ---------------------------------------------------------
#
# CI_CD says "every commit on the PR". Working out exactly which those are on a
# GitHub runner is half this gate, and the other half of the defect class: a
# gate that enumerates nothing and reports success has checked nothing while
# looking exactly like a gate that checked everything and found nothing wrong.
# So there is no path through this function that yields an empty set and an
# answer of 'ok'.
#
# Sets CT_ENUM_STATE to one of:
#   ok        CT_COMMITS holds one or more commits, oldest first
#   refuse    this event names a commit set and it could not be determined.
#             Nothing was checked and the run says so (exit 2)
#   nothing   there is legitimately nothing to check here, which happens only
#             off a GitHub event: a local run sitting on the base branch with
#             no commits of its own. Nothing was checked and the run says so
#             (exit 3). It is never reported as a pass
CT_ENUM_STATE=''
CT_ENUM_WHY=''
CT_RANGE_DESC=''
CT_COMMITS=''
ct_enumerate() {
    local repo="$1"
    local event="${GITHUB_EVENT_NAME:-}"
    local payload="${GITHUB_EVENT_PATH:-}"
    local base head want got mb before after sha ref
    CT_ENUM_STATE=''
    CT_ENUM_WHY=''
    CT_RANGE_DESC=''
    CT_COMMITS=''

    ct_refuse() { CT_ENUM_STATE='refuse'; CT_ENUM_WHY="$1"; }
    ct_nothing() { CT_ENUM_STATE='nothing'; CT_ENUM_WHY="$1"; }

    # Every SHA this function is handed comes from an event payload, and a SHA
    # that is not in the clone is the shallow-checkout failure: the gate would
    # enumerate a shorter range, or none, and report on it as though it were
    # the whole pull request. Named rather than tolerated.
    ct_have() { git -C "$repo" cat-file -e "$1^{commit}" 2>/dev/null; }

    case "$event" in
        pull_request|pull_request_target)
            if [ -z "$payload" ] || [ ! -f "$payload" ]; then
                ct_refuse "the event is '$event' and there is no readable event payload at GITHUB_EVENT_PATH ('$payload'), so the commits of this pull request cannot be named"
                return 0
            fi
            if ! command -v jq >/dev/null 2>&1; then
                ct_refuse "the event is '$event' and jq is not on this runner, so the event payload cannot be read and the commits of this pull request cannot be named"
                return 0
            fi
            base=$(jq -r '.pull_request.base.sha // empty' "$payload" 2>/dev/null) || base=''
            head=$(jq -r '.pull_request.head.sha // empty' "$payload" 2>/dev/null) || head=''
            want=$(jq -r '.pull_request.commits // empty' "$payload" 2>/dev/null) || want=''
            if [ -z "$base" ] || [ -z "$head" ]; then
                ct_refuse "the event payload names no pull_request.base.sha or no pull_request.head.sha, so the commits of this pull request cannot be named"
                return 0
            fi
            if ! ct_have "$base"; then
                ct_refuse "the base commit $base named by the event payload is not in this clone. The checkout is too shallow for this gate: give the job 'fetch-depth: 0'. Enumerating what happens to be present would check a shorter range and report on it as though it were the whole pull request"
                return 0
            fi
            if ! ct_have "$head"; then
                ct_refuse "the head commit $head named by the event payload is not in this clone. The checkout is too shallow for this gate: give the job 'fetch-depth: 0'"
                return 0
            fi
            mb=$(git -C "$repo" merge-base "$base" "$head" 2>/dev/null) || mb=''
            if [ -z "$mb" ]; then
                ct_refuse "no merge base resolves between $base and $head, so 'the commits on this pull request' names no set"
                return 0
            fi
            CT_COMMITS=$(git -C "$repo" rev-list --reverse "$mb..$head" 2>/dev/null) || CT_COMMITS=''
            CT_RANGE_DESC="$event: merge-base($base, $head)=$mb .. $head"
            if [ -z "$CT_COMMITS" ]; then
                ct_refuse "the range $mb..$head is empty, so this run would check no commit at all. GitHub does not open a pull request with no commits, so an empty set here means the range was computed wrongly, and a gate that checks nothing must not report success"
                return 0
            fi
            # The cross-check. GitHub itself says how many commits this pull
            # request has; this gate says which ones. If the two disagree, one
            # of them is wrong about the thing the gate exists to cover, and
            # neither answer may be reported as a pass.
            got=$(printf '%s\n' "$CT_COMMITS" | grep -c . 2>/dev/null) || got=0
            if [ -n "$want" ] && [ "$want" != "$got" ]; then
                ct_refuse "GitHub reports $want commit(s) on this pull request and this gate enumerated $got from $mb..$head. Until those agree the gate cannot say it checked every commit on the pull request, and it will not report a pass on a set it cannot vouch for"
                return 0
            fi
            CT_ENUM_STATE='ok'
            CT_ENUM_WHY="GitHub reports $want commit(s) on this pull request and this gate enumerated $got; they agree"
            ;;
        merge_group)
            # spec/CI_CD.md section 2: the merge queue rebases and re-runs
            # gates 1 to 10 plus 13 and 14 on the rebased head. The set is the
            # rebased commits, which are not the pull request's commits: they
            # are new objects with the same messages.
            if [ -z "$payload" ] || [ ! -f "$payload" ] || ! command -v jq >/dev/null 2>&1; then
                ct_refuse "the event is 'merge_group' and either there is no readable payload at GITHUB_EVENT_PATH ('$payload') or jq is absent, so the rebased commits cannot be named"
                return 0
            fi
            base=$(jq -r '.merge_group.base_sha // empty' "$payload" 2>/dev/null) || base=''
            head=$(jq -r '.merge_group.head_sha // empty' "$payload" 2>/dev/null) || head=''
            if [ -z "$base" ] || [ -z "$head" ]; then
                ct_refuse "the merge_group payload names no base_sha or no head_sha, so the rebased commits cannot be named"
                return 0
            fi
            if ! ct_have "$base" || ! ct_have "$head"; then
                ct_refuse "the merge group's base ($base) or head ($head) is not in this clone; the checkout is too shallow for this gate"
                return 0
            fi
            CT_COMMITS=$(git -C "$repo" rev-list --reverse "$base..$head" 2>/dev/null) || CT_COMMITS=''
            CT_RANGE_DESC="merge_group: $base..$head"
            if [ -z "$CT_COMMITS" ]; then
                ct_refuse "the merge group range $base..$head is empty. A merge queue entry with no commits is not a state this gate can report a pass on"
                return 0
            fi
            CT_ENUM_STATE='ok'
            CT_ENUM_WHY='the merge queue rebases, so these are new commit objects carrying the messages under review (spec/CI_CD.md section 2)'
            ;;
        push)
            if [ -z "$payload" ] || [ ! -f "$payload" ] || ! command -v jq >/dev/null 2>&1; then
                ct_refuse "the event is 'push' and either there is no readable payload at GITHUB_EVENT_PATH ('$payload') or jq is absent, so the commits this push landed cannot be named"
                return 0
            fi
            before=$(jq -r '.before // empty' "$payload" 2>/dev/null) || before=''
            after=$(jq -r '.after // empty' "$payload" 2>/dev/null) || after=''
            if [ -z "$before" ] || [ -z "$after" ]; then
                ct_refuse "the push payload names no 'before' or no 'after', so the commits this push landed cannot be named"
                return 0
            fi
            case "$before" in
                *[!0]*) ;;
                *)
                    ct_refuse "this push reports an all-zero 'before', which means the ref was created by this push. spec/CONVENTIONS.md 'Git' forbids force pushing and history rewriting, so on 'main' that state is itself the thing to look at, and this gate will not invent a range for it"
                    return 0
                    ;;
            esac
            if ! ct_have "$before" || ! ct_have "$after"; then
                ct_refuse "the push's before ($before) or after ($after) is not in this clone; the checkout is too shallow for this gate, or the ref was rewritten"
                return 0
            fi
            CT_COMMITS=$(git -C "$repo" rev-list --reverse "$before..$after" 2>/dev/null) || CT_COMMITS=''
            CT_RANGE_DESC="push: $before..$after"
            if [ -z "$CT_COMMITS" ]; then
                ct_refuse "the push range $before..$after is empty, so this run would check no commit. A push that moved a ref backwards or not at all is not a state this gate reports a pass on"
                return 0
            fi
            CT_ENUM_STATE='ok'
            CT_ENUM_WHY='on a push there is no pull request, so the set is the commits this push landed on the branch'
            ;;
        workflow_dispatch)
            # There is no pull request and no range. The only commit this event
            # names is the one that was checked out, so that is the one that is
            # checked, and the report says so rather than reporting a pass over
            # a set nobody defined.
            sha="${GITHUB_SHA:-}"
            if [ -z "$sha" ]; then
                sha=$(git -C "$repo" rev-parse HEAD 2>/dev/null) || sha=''
            fi
            if [ -z "$sha" ] || ! ct_have "$sha"; then
                ct_refuse "the event is 'workflow_dispatch' and no commit could be resolved for it (GITHUB_SHA='${GITHUB_SHA:-}')"
                return 0
            fi
            CT_COMMITS=$(git -C "$repo" rev-parse "$sha^{commit}" 2>/dev/null) || CT_COMMITS=''
            CT_RANGE_DESC="workflow_dispatch: the single commit $sha"
            if [ -z "$CT_COMMITS" ]; then
                ct_refuse "the commit $sha could not be resolved"
                return 0
            fi
            CT_ENUM_STATE='ok'
            CT_ENUM_WHY='a manual dispatch names no pull request and no range, so the set is the one commit that was checked out. This is a smaller claim than the one this gate makes on a pull request and the verdict says so'
            ;;
        '')
            # A local run at a coder's desk. The set is this branch's own
            # commits: everything since it left the base branch.
            for ref in origin/main main; do
                if git -C "$repo" rev-parse --verify --quiet "$ref^{commit}" >/dev/null 2>&1; then
                    base="$ref"
                    break
                fi
            done
            if [ -z "${base:-}" ]; then
                ct_nothing "no base branch resolves here: neither 'origin/main' nor 'main' names a commit in this repository, so 'the commits on this branch' names no set"
                return 0
            fi
            mb=$(git -C "$repo" merge-base "$base" HEAD 2>/dev/null) || mb=''
            if [ -z "$mb" ]; then
                ct_nothing "no merge base resolves between '$base' and HEAD, so there is no range for this gate to read"
                return 0
            fi
            CT_COMMITS=$(git -C "$repo" rev-list --reverse "$mb..HEAD" 2>/dev/null) || CT_COMMITS=''
            CT_RANGE_DESC="local: merge-base($base, HEAD)=$mb .. HEAD"
            if [ -z "$CT_COMMITS" ]; then
                ct_nothing "this branch carries no commit of its own against '$base' (merge base $mb is HEAD), so there is nothing here for this gate to read. That is not a pass: nothing was checked"
                return 0
            fi
            CT_ENUM_STATE='ok'
            CT_ENUM_WHY="off a GitHub event the set is this branch's own commits against '$base'"
            ;;
        *)
            ct_refuse "this gate has no rule for the event '$event'. CI_CD section 1 item 13 says 'every commit on the PR' and section 2 adds the merge queue; an event nobody wrote a rule for gets a refusal, because inventing a set here is how a gate ends up checking something other than what it reports"
            ;;
    esac
    return 0
}

# --- the whole gate --------------------------------------------------------
#
# MERGE COMMITS, and why there is no exemption.
#
# `main` carries one merge commit, ffab3fd, from before rebase-only merges were
# configured. It has no trailers and never will, because CONVENTIONS forbids
# rewriting history. It is nevertheless never enumerated here, and not because
# it is exempt: it is an ancestor of the base of every range this gate can
# build, and a range excludes its base. The gate does not need a rule for it
# and does not have one.
#
# Nothing else that reaches this gate should be a merge commit either.
# `allow_merge_commit` is false, so the merge queue rebases and produces none.
# A merge commit INSIDE a pull request's range therefore means somebody merged
# `main` into their branch instead of rebasing onto it, which spec/CONVENTIONS.md
# "Git" forbids in as many words: "Rebase on `main` before ready". Failing it is
# the correct verdict, not a false positive.
#
# So the exemption a reader might expect is refused deliberately, and the
# shapes it would have taken are both worse:
#   - "has two or more parents" exempts exactly the case above, which is the
#     case that should fail.
#   - "the subject starts with `Merge pull request`" is a regex over the
#     message, which is the mechanism this entire gate exists to reject.
# fixtures/planted/gate-13/ proves both halves: a range CONTAINING a merge
# commit fails, and a range whose BASE is a merge commit passes.
CT_RESULT=''
CT_CHECKED=0
CT_FAILED=0
ct_run() {
    local repo="$1"
    local sha subject codes n=0 bad=0
    CT_RESULT=''
    CT_CHECKED=0
    CT_FAILED=0

    if ! command -v git >/dev/null 2>&1; then
        CT_RESULT='refuse'
        ct_emit error "git is not on PATH, so gate 13 read no commit at all. Nothing was checked."
        return 0
    fi
    if [ ! -d "$repo/.git" ] && [ ! -f "$repo/.git" ]; then
        if ! git -C "$repo" rev-parse --git-dir >/dev/null 2>&1; then
            CT_RESULT='refuse'
            ct_emit error "'$repo' is not a git repository, so gate 13 read no commit at all. Nothing was checked."
            return 0
        fi
    fi

    ct_enumerate "$repo"
    say "  event            : ${GITHUB_EVENT_NAME:-(none: a local run)}"
    say "  commit set       : ${CT_RANGE_DESC:-(none)}"
    case "$CT_ENUM_STATE" in
        ok)
            say "  why this set     : $CT_ENUM_WHY"
            ;;
        refuse)
            CT_RESULT='refuse'
            ct_emit error "gate 13 could not determine which commits to check, so it checked none. $CT_ENUM_WHY"
            return 0
            ;;
        nothing)
            CT_RESULT='nothing'
            say "  nothing to check : $CT_ENUM_WHY"
            return 0
            ;;
        *)
            CT_RESULT='refuse'
            ct_emit error "gate 13's enumeration returned '$CT_ENUM_STATE', which is not one of its three answers. That is a bug in this script, not a result."
            return 0
            ;;
    esac

    say ''
    printf '  %-12s %-9s %s\n' 'commit' 'verdict' 'subject'
    printf '  %-12s %-9s %s\n' '------------' '---------' '----------------------------------------------'
    while IFS= read -r sha; do
        [ -n "$sha" ] || continue
        n=$((n + 1))
        subject=$(git -C "$repo" show -s --format='%s' "$sha" 2>/dev/null) || subject='(unreadable)'
        ct_check_commit "$repo" "$sha"
        codes="$CT_CODES"
        if [ -z "$codes" ]; then
            printf '  %-12s %-9s %s\n' "${sha:0:12}" 'ok' "$subject"
            if [ -n "$CT_DETAIL" ]; then printf '%s' "$CT_DETAIL"; fi
        else
            bad=$((bad + 1))
            printf '  %-12s %-9s %s\n' "${sha:0:12}" 'REFUSED' "$subject"
            printf '      codes: %s\n' "$codes"
            printf '%s' "$CT_DETAIL"
        fi
    done <<EOF
$CT_COMMITS
EOF

    CT_CHECKED="$n"
    CT_FAILED="$bad"

    # The floor of this gate. Everything above can be correct and this can
    # still be the state that matters: a loop that ran zero times prints a
    # header, a rule, and nothing, and a caller reading only the exit status
    # cannot tell it from a clean pass.
    if [ "$n" -eq 0 ]; then
        CT_RESULT='refuse'
        ct_emit error "gate 13 enumerated a non-empty commit set and then checked none of it. That is a bug in this script, not a result, and it is exactly the state this gate exists to refuse."
        return 0
    fi

    say ''
    say "  checked $n commit(s), $bad refused"
    if [ "$bad" -ne 0 ]; then
        CT_RESULT='fail'
        ct_emit error "gate 13: $bad of $n commit(s) under review do not meet spec/CONVENTIONS.md 'Git'. Every code and every reason is in the table above. Amend or reword the commits named there; spec/CI_CD.md section 1 item 13 asks this of EVERY commit on the pull request, not of the branch as a whole."
        return 0
    fi
    CT_RESULT='pass'
    return 0
}

# ---------------------------------------------------------------------------
# Gate 1: fmt and clippy -D warnings. CI_CD 1.1, CONVENTIONS "Rust".
#
# One gate, two sub-checks, probed and reported one at a time. The gate passes
# only when both passed. If one is blocked and the other is clean, the gate is
# blocked, not passed: half of gate 1 is not gate 1, and the summary names which
# half ran. The all-or-nothing prerequisite probe this replaces is described in
# "THE FIVE STATES" in the header.
# ---------------------------------------------------------------------------

gate_1() {
    local fmt_state='not evaluated' fmt_note=''
    local clippy_state='not evaluated' clippy_note=''
    local cargo_ok=0

    run_cmd gate-01-probe-cargo cargo --version
    if [ "$RUN_STATUS" -eq 0 ]; then
        cargo_ok=1
    fi

    # --- sub-check 1 of 2: formatting -------------------------------------
    say "  sub-check 1 of 2: cargo fmt --all --check"
    if [ "$cargo_ok" -eq 0 ]; then
        fmt_state='blocked'
        fmt_note='blocked, cargo is not on PATH, so nothing checked the formatting of this tree'
        say "    ${C_RED}blocked${C_RESET}: cargo is not on PATH"
    else
        run_cmd gate-01-probe-fmt cargo fmt --version
        if [ "$RUN_STATUS" -ne 0 ]; then
            fmt_state='blocked'
            fmt_note="blocked, 'cargo fmt --version' exited $RUN_STATUS here (rustup component add rustfmt), so nothing checked the formatting of this tree"
            say "    ${C_RED}blocked${C_RESET}: 'cargo fmt --version' exited $RUN_STATUS (rustup component add rustfmt)"
            show_output "$RUN_LOG" 6
        else
            run_cmd gate-01-fmt cargo fmt --all --check
            if [ "$RUN_STATUS" -eq 0 ]; then
                fmt_state='passed'
                fmt_note='cargo fmt --all --check clean'
                say "    ${C_GREEN}ok${C_RESET}"
            else
                fmt_state='failed'
                fmt_note="cargo fmt --all --check exited $RUN_STATUS"
                say "    ${C_RED}failed, status $RUN_STATUS${C_RESET}"
                show_output "$RUN_LOG" 40
            fi
        fi
    fi

    # --- sub-check 2 of 2: lints ------------------------------------------
    say "  sub-check 2 of 2: cargo clippy --workspace --all-targets --all-features -- -D warnings"
    if [ "$cargo_ok" -eq 0 ]; then
        clippy_state='blocked'
        clippy_note='blocked, cargo is not on PATH, so nothing lint-checked this tree'
        say "    ${C_RED}blocked${C_RESET}: cargo is not on PATH"
    else
        run_cmd gate-01-probe-clippy cargo clippy --version
        if [ "$RUN_STATUS" -ne 0 ]; then
            clippy_state='blocked'
            clippy_note="blocked, 'cargo clippy --version' exited $RUN_STATUS here (rustup component add clippy), so nothing lint-checked this tree"
            say "    ${C_RED}blocked${C_RESET}: 'cargo clippy --version' exited $RUN_STATUS (rustup component add clippy)"
            show_output "$RUN_LOG" 6
        else
            run_cmd gate-01-clippy cargo clippy --workspace --all-targets --all-features -- -D warnings
            if [ "$RUN_STATUS" -eq 0 ]; then
                clippy_state='passed'
                clippy_note='cargo clippy -D warnings clean'
                say "    ${C_GREEN}ok${C_RESET}"
            else
                clippy_state='failed'
                clippy_note="cargo clippy -D warnings exited $RUN_STATUS"
                say "    ${C_RED}failed, status $RUN_STATUS${C_RESET}"
                show_output "$RUN_LOG" 40
            fi
        fi
    fi

    # --- the gate's own state, worst sub-check wins ------------------------
    local note="fmt: $fmt_note; clippy: $clippy_note"
    if [ "$fmt_state" = 'failed' ] || [ "$clippy_state" = 'failed' ]; then
        set_state 1 'failed' "$note"
    elif [ "$fmt_state" = 'blocked' ] || [ "$clippy_state" = 'blocked' ]; then
        set_state 1 'blocked' "$note"
    elif [ "$fmt_state" = 'passed' ] && [ "$clippy_state" = 'passed' ]; then
        set_state 1 'passed' "$note"
    fi
    # Any other combination leaves gate 1 at 'not evaluated', which the summary
    # turns into exit 2. There is deliberately no else branch that guesses.
}

# ---------------------------------------------------------------------------
# Gate 2: cargo test, workspace wide. CI_CD 1.2.
#
# The note counts tests, not lines of output. A count of test-result lines
# reads like coverage and is not: cargo prints one such line per test binary,
# including for every binary that ran nothing, and this workspace today runs no
# tests at all. What a clean run of zero tests proves is that the workspace
# compiles under cfg(test), and the note says exactly that.
# ---------------------------------------------------------------------------

gate_2() {
    run_cmd gate-02-probe-cargo cargo --version
    if [ "$RUN_STATUS" -ne 0 ]; then
        set_state 2 'blocked' 'blocked, cargo is not on PATH, so no test ran here'
        say "  ${C_RED}blocked${C_RESET}: cargo is not on PATH, so no test ran here"
        return 0
    fi

    say "  cargo test --workspace --all-features"
    run_cmd gate-02-test cargo test --workspace --all-features
    local status=$RUN_STATUS
    if [ "$status" -ne 0 ]; then
        say "    ${C_RED}failed, status $status${C_RESET}"
        show_output "$RUN_LOG" 40
        set_state 2 'failed' "cargo test --workspace --all-features exited $status"
        return 0
    fi

    local binaries ran
    binaries=$(grep -c '^test result:' "$RUN_LOG" 2>/dev/null || true)
    ran=$(awk '/^test result:/ { for (i = 2; i <= NF; i++) if ($i == "passed;") total += $(i - 1) } END { print total + 0 }' "$RUN_LOG" 2>/dev/null || printf '0')
    TESTS_RAN="$ran"

    if [ "$ran" -eq 0 ]; then
        say "    ${C_GREEN}ok${C_RESET}, and ${C_YELLOW}0 tests ran${C_RESET} across ${binaries:-0} test binaries"
        say "    ${C_YELLOW}A clean run of no tests is evidence that the workspace compiles under cfg(test), nothing more.${C_RESET}"
        set_state 2 'passed' "cargo test --workspace --all-features exited 0; 0 tests ran across ${binaries:-0} test binaries, so this gate is evidence that the test build compiles and nothing more"
    else
        say "    ${C_GREEN}ok${C_RESET}, $ran test(s) across ${binaries:-0} test binaries"
        set_state 2 'passed' "cargo test --workspace --all-features clean; $ran test(s) ran across ${binaries:-0} test binaries"
    fi
}

# ---------------------------------------------------------------------------
# Gate 7: cargo-audit, cargo-deny, secret scan. CI_CD 1.7, ENV_SETUP section 4,
# CONVENTIONS "Dependencies".
#
# GATE 7 IS PARTIALLY INSTALLED. TWO CHECKS OF THREE. Read this before reading
# a green line below as gate 7 being met.
#
#   advisories (cargo-audit)          INSTALLED, seen to fail on a planted
#                                     lockfile naming time 0.1.44
#   licences and bans (cargo-deny)    INSTALLED, each half seen to fail
#                                     separately and attributably
#   secret scan                       NOT INSTALLED. Escalation E-0003.
#
# WHY THE THIRD IS NOT INSTALLED. Two attempts built a pattern scanner, and a
# pattern matcher over bytes loses this race by construction: attempt 1 missed
# every file holding a NUL byte, attempt 2 fixed that and misses UTF-16, and
# base64, gzip, UTF-32 and a key split across a chunk boundary are the same
# hole. The lead planted an AWS key encoded UTF-16, committed it, and the
# repaired scanner exited 0 and reported no finding with the key in the file.
# Separately, this repository already runs GitGuardian on every pull request
# and has since before gate 7 was written. E-0003 asks the operator what
# implements gate 7's secret scan; until it is answered this script reports
# that sub-check blocked, and gate 7 with it.
#
# THE CONSEQUENCE, STATED SO THAT NOBODY IS SURPRISED BY IT. A blocked gate
# makes this whole script exit 3, INCOMPLETE, on every run, until E-0003 is
# answered. That is deliberate and it is the point: the alternative is exit 0
# with gate 7 marked passed on the strength of a check that reports exit 0 for
# a credential sitting in the tree, and this script's own header calls that its
# defect class. Exit 3 is not exit 1: nothing has failed. What it says is that
# gate 7 has not been fully checked by anybody, which is true.
#
# UNTIL ORI-T-0016 THIS GATE WAS A PROBE AND THREE SENTENCES, and the sentences
# were wrong by the time anyone read them. It reported "not available" for a
# missing `deny.toml`, said Cargo.lock's empty third-party list meant an
# advisory check "could not be seen to fail on a planted defect", and said the
# secret scan "has no local runner either". The first two are repaired: the
# policy is at the repository root and both cargo checks have been seen to fail
# on planted inputs (`fixtures/planted/gate-7/prove.sh`). The third sentence
# was right for the wrong reason and is still right: the secret scan has no
# gate runner, here or in CI. CLAUDE.md step 4 makes this script the gate set a
# coder runs before opening a pull request, which is the one place a coder
# would believe a sentence like that, so it says the true one.
#
# ONE GATE, THREE CHECKS, IN THREE TOOLS, reported one at a time for the reason
# gate 1 reports fmt and clippy apart: a reader of a red run has to be able to
# say which of the three failed without opening a log, and two thirds of a gate
# passing is not the gate passing. Two thirds is exactly where gate 7 stands,
# and the line below says "blocked" rather than "passed" for that reason.
#
# WHAT IS A BLOCK HERE AND WHAT IS A FAILURE. cargo-audit exits non-zero when
# it cannot reach the advisory database, cargo-deny exits non-zero when it
# cannot read a manifest, and the scanner exits 3 when it could not scan. None
# of those is the gate catching anything, and recording one as a failure would
# send a coder to look for a vulnerability that is not there. Each check below
# reads what the tool printed before deciding which it was, and the advisory
# check refuses a pass that loaded no advisories, because a clean report
# against an empty database is the vacuous pass AICD §14 exists to forbid.
# ---------------------------------------------------------------------------

gate_7() {
    local adv_state='not evaluated' adv_note=''
    local lic_state='not evaluated' lic_note=''
    local sec_state='not evaluated' sec_note=''

    # Third-party packages carry a source line in Cargo.lock; the workspace's
    # own path crates do not. This is a fact about what the two cargo checks
    # have to judge here, and it is read on this run rather than asserted.
    local third_party=0
    if [ -f "$REPO_ROOT/Cargo.lock" ]; then
        third_party=$(grep -c '^source = ' "$REPO_ROOT/Cargo.lock" 2>/dev/null || true)
    fi

    # --- advisories ------------------------------------------------------
    say "  cargo audit --deny warnings"
    run_cmd gate-07-probe-audit cargo audit --version
    if [ "$RUN_STATUS" -ne 0 ]; then
        adv_state='blocked'
        adv_note="blocked, 'cargo audit --version' exited $RUN_STATUS here, so nothing compared Cargo.lock against the RustSec database (spec/ENV_SETUP.md section 1 lists cargo-audit; scripts/setup-dev.sh installs it)"
        say "    ${C_RED}blocked${C_RESET}: 'cargo audit --version' exited $RUN_STATUS (spec/ENV_SETUP.md section 1; scripts/setup-dev.sh installs it)"
    else
        run_cmd gate-07-audit cargo audit --deny warnings
        local audit_status=$RUN_STATUS loaded=''
        loaded=$(sed -n 's/.*Loaded \([0-9][0-9]*\) security advisor.*/\1/p' "$RUN_LOG" 2>/dev/null | head -1)
        if [ -z "$loaded" ]; then
            adv_state='blocked'
            adv_note="blocked, cargo-audit printed no 'Loaded N security advisories' line and exited $audit_status, so the advisory database was not loaded on this machine and any verdict from it would be a verdict against an empty database"
            say "    ${C_RED}blocked${C_RESET}: no advisory database was loaded (exit $audit_status), so nothing was compared against anything"
            show_output "$RUN_LOG" 20
        elif [ "$audit_status" -eq 0 ]; then
            adv_state='passed'
            adv_note="cargo audit --deny warnings clean against $loaded loaded advisories; Cargo.lock names $third_party third-party package(s), so this pass is about the workspace's own path crates and the check's teeth are shown by fixtures/planted/gate-7/prove.sh instead"
            say "    ${C_GREEN}ok${C_RESET}, against $loaded loaded advisories; Cargo.lock names $third_party third-party package(s)"
        else
            adv_state='failed'
            adv_note="cargo audit --deny warnings exited $audit_status against $loaded loaded advisories"
            say "    ${C_RED}failed, status $audit_status${C_RESET}"
            show_output "$RUN_LOG" 40
        fi
    fi

    # --- licenses, bans and sources --------------------------------------
    say "  cargo deny --manifest-path Cargo.toml --config deny.toml check licenses bans sources"
    run_cmd gate-07-probe-deny cargo deny --version
    if [ "$RUN_STATUS" -ne 0 ]; then
        lic_state='blocked'
        lic_note="blocked, 'cargo deny --version' exited $RUN_STATUS here, so no license, ban or source policy was enforced (spec/ENV_SETUP.md section 1 lists cargo-deny; scripts/setup-dev.sh installs it)"
        say "    ${C_RED}blocked${C_RESET}: 'cargo deny --version' exited $RUN_STATUS (spec/ENV_SETUP.md section 1; scripts/setup-dev.sh installs it)"
    elif [ ! -f "$REPO_ROOT/deny.toml" ]; then
        lic_state='blocked'
        lic_note='blocked, deny.toml is not at the repository root, so cargo-deny would fall back to its own defaults and a clean verdict would be about those rather than about this repository policy'
        say "    ${C_RED}blocked${C_RESET}: deny.toml is missing from the repository root"
    else
        run_cmd gate-07-deny cargo deny --manifest-path Cargo.toml --config deny.toml check licenses bans sources
        local deny_status=$RUN_STATUS deny_summary=''
        deny_summary=$(grep -E 'bans (ok|FAILED), licenses (ok|FAILED), sources (ok|FAILED)' "$RUN_LOG" 2>/dev/null | tail -1)
        if [ -z "$deny_summary" ]; then
            lic_state='blocked'
            lic_note="blocked, cargo-deny exited $deny_status without printing a per-check verdict, so it did not get as far as judging this workspace against deny.toml"
            say "    ${C_RED}blocked${C_RESET}: cargo-deny exited $deny_status without reaching a verdict on any check"
            show_output "$RUN_LOG" 20
        elif [ "$deny_status" -eq 0 ]; then
            lic_state='passed'
            lic_note="cargo-deny clean against deny.toml: $deny_summary; Cargo.lock names $third_party third-party package(s), so the license and ban checks pass on an empty set here and are shown to fail by fixtures/planted/gate-7/prove.sh"
            say "    ${C_GREEN}ok${C_RESET}, $deny_summary"
        else
            lic_state='failed'
            lic_note="cargo-deny exited $deny_status against deny.toml: $deny_summary"
            say "    ${C_RED}failed, status $deny_status${C_RESET}: $deny_summary"
            show_output "$RUN_LOG" 40
        fi
    fi

    # --- the secret scan: BLOCKED, and blocked whatever the scanner says ---
    #
    # THIS SUB-CHECK CANNOT REACH 'passed' OR 'failed', BY CONSTRUCTION. The
    # reason is above, at the head of gate_7: what implements gate 7's secret
    # scan is escalation E-0003, open with the operator, and until it is
    # answered there is nothing here whose verdict would mean gate 7's third
    # check was met. `scripts/secret-scan.sh` still runs, because its findings
    # are worth having and a coder should see them, but its exit status is
    # reported as INFORMATION and never promoted to a gate verdict.
    #
    # Not even exit 1. A finding from an advisory scanner is a thing to go and
    # look at, and the line below says so in the loudest terms this script has.
    # But calling it 'failed' would make the complementary case -- exit 0 --
    # readable as 'passed', and exit 0 from this scanner is the line an AWS key
    # written UTF-16 produces with the key sitting in the tree. One verdict for
    # a check that cannot be trusted in either direction is the honest number.
    say "  bash scripts/secret-scan.sh (advisory; not a gate verdict)"
    if [ ! -f "$REPO_ROOT/scripts/secret-scan.sh" ]; then
        sec_state='blocked'
        sec_note='blocked, gate 7 secret scan NOT INSTALLED (escalation E-0003: what implements it is an open question with the operator). The advisory scanner scripts/secret-scan.sh is also missing, so nothing read this tree or its history for credentials at all'
        say "    ${C_RED}blocked${C_RESET}: gate 7's secret scan is not installed (E-0003), and the advisory scanner is missing too"
    else
        run_cmd gate-07-secrets bash "$REPO_ROOT/scripts/secret-scan.sh"
        local scan_status=$RUN_STATUS scanned='' advisory=''
        scanned=$(sed -n 's/^no finding: no secret-shaped printable-ASCII string outside a declared fake, //p' "$RUN_LOG" 2>/dev/null | head -1)
        sec_state='blocked'
        case "$scan_status" in
            0)
                advisory="the advisory scanner found nothing, ${scanned:-no counts printed} -- which is also what it reports for a credential that is not printable ASCII, so it is not evidence of absence"
                say "    ${C_RED}blocked${C_RESET}: gate 7's secret scan is NOT INSTALLED (E-0003)"
                say "      the advisory scanner found nothing: ${scanned:-no counts printed}"
                say "      that is not a clean bill. A UTF-16 key, a .p12 or a keystore exits 0 here too."
                ;;
            1)
                advisory='THE ADVISORY SCANNER FOUND A SECRET-SHAPED STRING that is not a declared fake, in the tree or in the history. Go and look at it now. Rotate first (spec/runbooks/rotate-credentials.md), because deleting the file leaves the blob reachable. It is recorded as advisory rather than as a gate failure only because the gate itself is not installed; the finding is real'
                say "    ${C_RED}blocked${C_RESET}: gate 7's secret scan is NOT INSTALLED (E-0003), and"
                say "    ${C_RED}the advisory scanner FOUND a secret-shaped string. Go and look.${C_RESET}"
                say "      rotate before repairing (spec/runbooks/rotate-credentials.md)"
                show_output "$RUN_LOG" 40
                ;;
            3)
                advisory='the advisory scanner reported that it could not scan, which is its answer for a shallow clone, a missing tool, or a tracked entry or blob it could not read; the log names what was not scanned'
                say "    ${C_RED}blocked${C_RESET}: gate 7's secret scan is NOT INSTALLED (E-0003)"
                say "      the advisory scanner could not scan either; it names what it could not read"
                show_output "$RUN_LOG" 30
                ;;
            *)
                advisory="the advisory scanner exited $scan_status, which is not one of its three verdicts, so nothing it printed is a statement about this repository"
                say "    ${C_RED}blocked${C_RESET}: gate 7's secret scan is NOT INSTALLED (E-0003)"
                say "      the advisory scanner exited $scan_status, which is not one of its verdicts"
                show_output "$RUN_LOG" 30
                ;;
        esac
        sec_note="blocked, gate 7 secret scan NOT INSTALLED (escalation E-0003: what implements gate 7's secret scan is an open question with the operator, and this repository already runs GitGuardian on every pull request as an advisory check). scripts/secret-scan.sh is advisory and matches printable-ASCII patterns only, so neither of its verdicts is a gate verdict: $advisory. See ops/gates/gate-7.md, which records gate 7 as partially installed"
    fi

    # --- the gate ---------------------------------------------------------
    local note="advisories: $adv_note; licenses and bans: $lic_note; secret scan: $sec_note"
    if [ "$adv_state" = 'failed' ] || [ "$lic_state" = 'failed' ] || [ "$sec_state" = 'failed' ]; then
        set_state 7 'failed' "$note"
    elif [ "$adv_state" = 'blocked' ] || [ "$lic_state" = 'blocked' ] || [ "$sec_state" = 'blocked' ]; then
        set_state 7 'blocked' "$note"
    elif [ "$adv_state" = 'passed' ] && [ "$lic_state" = 'passed' ] && [ "$sec_state" = 'passed' ]; then
        set_state 7 'passed' "$note"
    else
        # Unreachable by construction: every branch above assigns one of the
        # three. Saying so rather than defaulting to a benign state is the rule
        # this whole script is built on.
        set_state 7 'not evaluated' "gate 7 left a sub-check unevaluated (advisories $adv_state, licenses $lic_state, secrets $sec_state), which is a bug in gates.sh"
    fi
}

# ---------------------------------------------------------------------------
# Gates 3 to 6 and 8 to 14: gates.sh has no runner for any of them. Each one is
# probed rather than assumed, so the reason printed is a fact about this machine
# and this tree at this moment.
# ---------------------------------------------------------------------------

gate_3() {
    probe_unavailable 3 'test directory' path:tests/contract
}

gate_4() {
    # The coverage matrix parses spec/criteria/*.md and test names. The criteria
    # exist; the parser does not.
    probe_unavailable 4 'runner' path:crates/ori-gates/src/coverage_matrix.rs
}

gate_5() {
    # What is missing is the detector. Whether the comparison it would need is
    # possible here is a separate question, so the script asks git instead of
    # asserting an answer.
    probe_unavailable 5 'runner' path:crates/ori-gates/src/modified_tests.rs
    local base=''
    if command -v git >/dev/null 2>&1; then
        base=$(git merge-base HEAD main 2>/dev/null || true)
    fi
    if [ -n "$base" ]; then
        append_note 5 "the merge base against main does resolve here (${base:0:12}), so what is missing is the detector and not the history"
    else
        append_note 5 'no merge base against main resolves here either, so there is no diff for a detector to read'
    fi
}

gate_6() {
    probe_unavailable 6 'tooling' cmd:cargo-mutants
    # A mutation score is the ratio of mutants that tests catch. Say what this
    # run measured about the other half of that ratio, so that the reason
    # changes by itself the day the workspace has tests.
    if [ "$TESTS_RAN" = 'unknown' ]; then
        append_note 6 'a mutation score is the ratio of mutants that tests catch, and this run could not count the tests'
    elif [ "$TESTS_RAN" -eq 0 ]; then
        append_note 6 'a mutation score is the ratio of mutants that tests catch, and gate 2 counted 0 tests here, so a wired threshold could only report a vacuous pass, which AICD §14 forbids'
    else
        append_note 6 "gate 2 counted $TESTS_RAN test(s), so a mutation threshold now has tests to measure and can be seen to fail on a planted defect"
    fi
}

gate_8() {
    probe_unavailable 8 'fixture' path:fixtures/new-product
}

gate_9() {
    # Held open deliberately: the section index is not generated yet, and the
    # methodology HTML's anchors are being corrected first.
    probe_unavailable 9 'section index' path:methodology/sections.json
    append_note 9 'this gate stays defined and not installed until the index is generated'
}

gate_10() {
    probe_unavailable 10 'UI scaffold or tooling' cmd:node path:apps/desktop/package.json
}

gate_11() {
    # Cross compilation for macOS, Windows and Linux is a CI matrix. Report what
    # is actually installed here rather than asserting it cannot be done.
    local installed=0
    if command -v rustup >/dev/null 2>&1; then
        installed=$(rustup target list --installed 2>/dev/null | grep -c . || true)
        set_state 11 'not available' \
            "cross compilation for the three platforms runs in the CI matrix; $installed target(s) installed locally and gates.sh wires no cross build"
    else
        set_state 11 'not available' \
            'rustup not present, so installed cross-compilation targets cannot be determined; this gate runs in the CI matrix'
    fi
}

gate_12() {
    # CI_CD section 1 states this one runs on merge, not on the pull request.
    set_state 12 'not available' \
        'runs on merge and sets the significant label through the repository API; there is no merge and no label here'
}

gate_13() {
    # A real runner as of ORI-T-0017. The same ct_run() that
    # `bash scripts/gates.sh --commit-trailers` runs in CI, over the same
    # implementation, differing only in which commits it enumerates: off a
    # GitHub event the set is this branch's own commits against origin/main.
    #
    # In this fleet the lead makes the commits and the coder produces the work,
    # so a coder's worktree usually carries no commit of its own. That is
    # reported as BLOCKED, never as passed: gates.sh has a runner for gate 13
    # and this run checked no commit, and "I could not check" never shares a
    # state with "I checked and it was fine". The note says which of the two
    # happened.
    ct_run "$REPO_ROOT"
    case "$CT_RESULT" in
        pass)
            set_state 13 'passed' "$CT_CHECKED commit(s) on this branch carry a Conventional Commits subject and the Ticket: and Spec: trailers, read with git's own trailer parser; range: $CT_RANGE_DESC"
            ;;
        fail)
            set_state 13 'failed' "$CT_FAILED of $CT_CHECKED commit(s) on this branch do not meet spec/CONVENTIONS.md \"Git\"; the table above names each one and why"
            ;;
        nothing)
            set_state 13 'blocked' "nothing was checked: $CT_ENUM_WHY. In this fleet the lead makes the commits and the coder produces the work, so a coder's worktree commonly reaches this state; it is not a pass"
            ;;
        *)
            set_state 13 'blocked' "nothing was checked: ${CT_ENUM_WHY:-gate 13 refused to name a commit set on this machine}"
            ;;
    esac
}

gate_14() {
    probe_unavailable 14 'runner' path:crates/ori-gates/src/diagrams.rs
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

state_color() {
    case "$1" in
        passed)          printf '%s' "$C_GREEN" ;;
        failed)          printf '%s' "$C_RED" ;;
        blocked)         printf '%s' "$C_RED" ;;
        'not evaluated') printf '%s' "$C_RED" ;;
        *)               printf '%s' "$C_YELLOW" ;;
    esac
}

print_summary() {
    head1 "Summary: all ${GATE_COUNT} gates of spec/CI_CD.md section 1"
    printf '  %-3s %-15s %s\n' '#' 'State' 'Gate'
    printf '  %-3s %-15s %s\n' '---' '---------------' '--------------------------------------------'

    local i col
    i=1
    while [ "$i" -le "$GATE_COUNT" ]; do
        col=$(state_color "${GATE_STATE[$i]}")
        printf '  %-3s %s%-15s%s %s\n' "$i" "$col" "${GATE_STATE[$i]}" "$C_RESET" "${GATE_NAME[$i]}"
        printf '      %s%s%s\n' "$C_DIM" "${GATE_NOTE[$i]}" "$C_RESET"
        i=$((i + 1))
    done
}

# A state that contradicts the gate table is a bug in this script, not a
# result: only a gate with a local runner can be blocked, and only a gate
# without one can be not available.
check_states_consistent() {
    local i bad=0
    i=1
    while [ "$i" -le "$GATE_COUNT" ]; do
        if [ "${GATE_STATE[$i]}" = 'blocked' ] && [ "${GATE_RUNNER[$i]}" != 'local' ]; then
            say "  gate $i is blocked but gates.sh has no runner for it"
            bad=1
        fi
        if [ "${GATE_STATE[$i]}" = 'not available' ] && [ "${GATE_RUNNER[$i]}" = 'local' ]; then
            say "  gate $i reports 'not available' although gates.sh has a runner for it; that state hides a blocked machine"
            bad=1
        fi
        i=$((i + 1))
    done
    return "$bad"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

usage() {
    cat <<'EOF'
gates.sh: the local gate set a coder runs before opening a pull request.

  scripts/gates.sh                run the gate set and print the summary
  scripts/gates.sh --self-check   run only the harness self-check and stop
  scripts/gates.sh --commit-trailers [--repo DIR]
                                  run gate 13 alone, over the commits the
                                  current event names, and exit with gate 13's
                                  own verdict. This is the form CI runs
                                  (.github/workflows/ci.yml, job `gate-13`) and
                                  the form fixtures/planted/gate-13/prove.sh
                                  presents planted repositories to. --repo
                                  changes only WHICH repository git reads,
                                  never HOW the commit set is computed, so the
                                  enumeration the proof exercises is the
                                  enumeration CI uses.
  scripts/gates.sh --help         this text

Every gate in spec/CI_CD.md section 1 is reported in one of five states:
passed, failed, blocked (gates.sh has a runner and this machine could not run
it), not available (gates.sh has no runner for it) or not evaluated (a bug).
Runs from any directory inside the repository.

Exit status
  0  every gate gates.sh can run here ran, and none failed
  1  at least one gate failed
  2  gates.sh itself could not run correctly, which includes a run that ended
     anywhere other than at a verdict it chose to report
  3  nothing failed and nothing was proven: a runner gates.sh has could not run
     on this machine, or no gate reached a verdict at all
 130 interrupted by SIGINT; 143 by SIGTERM; 129 by SIGHUP

Exit status of --commit-trailers, which reports on one gate and so answers a
narrower question:
  0  every commit in the set this event names conforms, and the set was not
     empty
  1  at least one commit does not
  2  the set could not be determined, so no commit was checked. An unknown
     event, an unreadable event payload, a base or head commit missing from a
     shallow clone, or GitHub and this gate disagreeing about how many commits
     the pull request has
  3  there was legitimately nothing to enumerate, which happens only off a
     GitHub event: a local branch with no commits of its own. Nothing was
     checked, and this status says so rather than reporting a pass

This script is the last thing a coder runs before saying the work is done, so
"I could not check" never shares an exit code with "I checked and it was fine".
A gate gates.sh has no runner for is not an error: nothing at the coder's desk
changes it, and a run that is always red teaches its users to ignore the exit
code. A gate whose runner could not run here is an error, because the coder can
usually fix it in one command and until then nobody has checked their code.

Exit 0 is only ever reached from the summary below, or from --help and
--self-check, which say in their own output that they checked no code. A run
that stops before the summary, for any reason at all, cannot exit 0.
EOF
}

main() {
    local only_self_check=0
    local only_commit_trailers=0
    local ct_repo="$REPO_ROOT"

    STAGE='reading arguments'
    while [ "$#" -gt 0 ]; do
        case "$1" in
            -h|--help) usage; finish 0 ;;
            --self-check) only_self_check=1; shift ;;
            --commit-trailers) only_commit_trailers=1; shift ;;
            --repo)
                if [ "$#" -lt 2 ] || [ -z "${2:-}" ]; then
                    say "gates.sh: --repo needs a directory"
                    finish 2
                fi
                ct_repo="$2"
                shift 2
                ;;
            --plant-abort=*)
                # Handled at the top level, before the log directory exists.
                # Reaching here means that handler did not fire.
                say "gates.sh: '$1' was not handled before the run started"
                finish 2
                ;;
            *) say "gates.sh: unknown argument '$1'"; say ''; usage; finish 2 ;;
        esac
    done

    say "${C_BOLD}Ori Studio local gate set${C_RESET}"
    say "  repository: $REPO_ROOT"
    if command -v cargo >/dev/null 2>&1; then
        say "  toolchain:  $(cargo --version 2>/dev/null || echo 'cargo version unavailable')"
    else
        say "  toolchain:  cargo not on PATH"
    fi
    if [ -n "$CARGO_PATH_NOTE" ]; then
        dim "  note:       $CARGO_PATH_NOTE"
    fi
    say "  spec:       spec/CI_CD.md section 1, ${GATE_COUNT} gates"

    STAGE='the harness self-check'
    head1 "Harness self-check (AICD §14: seen to fail before trusted)"
    if self_check; then
        say "  ${C_GREEN}ok${C_RESET}: a failing gate command is recorded as failed, including a noisy one,"
        say "  the exit policy returns non-zero for a run in which nothing could be checked,"
        say "  and a run of this script aborted by an unbound variable, a failing command or"
        say "  a SIGINT exits 2, 2 and 130, while a clean --help run of it still exits 0"
    else
        say "  ${C_RED}the harness cannot distinguish a failed command from a passing one,${C_RESET}"
        say "  ${C_RED}or its exit policy would report an unchecked run as success,${C_RESET}"
        say "  ${C_RED}or an aborted run of this script does not report a non-zero status.${C_RESET}"
        say "  Refusing to report gate results from an instrument that does not work."
        finish 2
    fi

    if [ "$only_commit_trailers" -eq 1 ]; then
        STAGE='gate 13 alone, the commit-trailer gate'
        head1 "Gate 13 (alone): ${GATE_NAME[13]}"
        say "  spec/CI_CD.md section 1 item 13, spec/CONVENTIONS.md \"Git\", spec/PRD.md G-02"
        say "  repository       : $ct_repo"
        say ""
        ct_run "$ct_repo"
        say ""
        case "$CT_RESULT" in
            pass)
                say "  ${C_GREEN}PASSED.${C_RESET} Every one of the $CT_CHECKED commit(s) this event names carries a"
                say "  Conventional Commits subject and a Ticket: and a Spec: trailer that GIT'S OWN"
                say "  PARSER reads, which is the only reader whose answer matters: every consumer of"
                say "  these trailers goes through it."
                say ""
                say "  NOT ESTABLISHED: that the Spec: anchor resolves. The document half is printed"
                say "  above as an observation and is never judged; nothing in this repository resolves"
                say "  a markdown anchor yet. ops/gates/gate-13.md states why that is the choice."
                finish 0
                ;;
            fail)
                say "  ${C_RED}FAILED.${C_RESET} $CT_FAILED of $CT_CHECKED commit(s) do not meet spec/CONVENTIONS.md \"Git\"."
                finish 1
                ;;
            refuse)
                say "  ${C_RED}NOTHING WAS CHECKED.${C_RESET} Gate 13 could not name the commits this event covers,"
                say "  so it read none of them. This is not a pass and not a failure of any commit."
                finish 2
                ;;
            nothing)
                say "  ${C_RED}NOTHING WAS CHECKED.${C_RESET} There was no commit here for gate 13 to read."
                say "  This is not a pass. Do not read it as one."
                finish 3
                ;;
            *)
                say "  ${C_RED}Gate 13 returned '$CT_RESULT', which is not one of its four answers.${C_RESET}"
                say "  That is a bug in gates.sh, not a result."
                finish 2
                ;;
        esac
    fi

    if [ "$only_self_check" -eq 1 ]; then
        say ""
        say "Self-check only, requested with --self-check. ${C_YELLOW}No gate was run,${C_RESET}"
        say "so exit 0 here means the harness works, not that anything was checked."
        # This is the one exit 0 that is not a summary, and it is deliberate:
        # the caller asked a question about the instrument, and this status is
        # the answer to that question and to nothing else. The two lines above
        # are printed before it so the status is never read on its own.
        finish 0
    fi

    STAGE='gate 1, fmt and clippy'
    head1 "Gate 1: ${GATE_NAME[1]}"
    gate_1
    STAGE='gate 2, cargo test'
    head1 "Gate 2: ${GATE_NAME[2]}"
    gate_2

    STAGE='gate 7, the supply chain'
    head1 "Gate 7: ${GATE_NAME[7]}"
    gate_7

    STAGE='gate 13, the commit-trailer gate'
    head1 "Gate 13: ${GATE_NAME[13]}"
    gate_13

    STAGE='gates 3 to 6, 8 to 12 and 14, availability probes'
    head1 "Gates 3 to 6, 8 to 12 and 14: probing availability"
    gate_3; gate_4; gate_5; gate_6; gate_8
    gate_9; gate_10; gate_11; gate_12; gate_14

    local probed=0 i
    i=1
    while [ "$i" -le "$GATE_COUNT" ]; do
        if [ "${GATE_RUNNER[$i]}" = 'elsewhere' ]; then probed=$((probed + 1)); fi
        i=$((i + 1))
    done
    say "  probed $probed gate(s) that gates.sh has no runner for; each reason in the summary below"
    say "  is a fact about this machine and this tree, read on this run"

    STAGE='the summary'
    print_summary

    local passed=0 failed=0 blocked=0 unavailable=0 unevaluated=0
    i=1
    while [ "$i" -le "$GATE_COUNT" ]; do
        case "${GATE_STATE[$i]}" in
            passed)          passed=$((passed + 1)) ;;
            failed)          failed=$((failed + 1)) ;;
            blocked)         blocked=$((blocked + 1)) ;;
            'not available') unavailable=$((unavailable + 1)) ;;
            *)               unevaluated=$((unevaluated + 1)) ;;
        esac
        i=$((i + 1))
    done

    say ""
    say "  $passed passed, $failed failed, $blocked blocked here, $unavailable with no local runner, out of $GATE_COUNT."

    say ""
    if ! check_states_consistent; then
        say "  ${C_RED}A gate's state contradicts the gate table.${C_RESET}"
        say "  That is a bug in gates.sh, not a result. Exiting 2 rather than report a verdict."
        finish 2
    fi

    local verdict
    verdict=$(verdict_for "$passed" "$failed" "$blocked" "$unevaluated")

    case "$verdict" in
        2)
            say "  ${C_RED}$unevaluated gate(s) were never evaluated.${C_RESET}"
            say "  That is a bug in gates.sh, not a result. Exiting 2 rather than report a verdict."
            finish 2
            ;;
        1)
            say "  ${C_RED}FAILED.${C_RESET} Fix the failing gate(s) before opening the pull request."
            finish 1
            ;;
        3)
            if [ "$((passed + failed))" -eq 0 ]; then
                say "  ${C_RED}NOTHING WAS CHECKED.${C_RESET} No gate reached a verdict on this run."
                say "  This is not a pass. Do not read it as one, and do not open the pull request on it."
            else
                say "  ${C_RED}INCOMPLETE.${C_RESET} $blocked gate(s) that gates.sh can run did not reach a verdict on this run."
                say "  What they cover has been checked by nobody. The summary names each one and why."
                say "  A gate can land here for two different reasons and the summary tells them apart:"
                say "  something is missing from this machine and you can install it, or the gate itself"
                say "  is not installed in this project yet and an escalation is open on it. Gate 7's"
                say "  secret scan is the second kind (E-0003), so this run will say INCOMPLETE until"
                say "  that is answered. INCOMPLETE is not FAILED; nothing here failed."
            fi
            say "  $unavailable gate(s) have no local runner at all; those are a fact about the project,"
            say "  they are listed above, and they are not what this exit code is about."
            finish 3
            ;;
        0)
            say "  ${C_GREEN}No gate failed, and every gate gates.sh can run here ran.${C_RESET}"
            say "  $unavailable of $GATE_COUNT gates have no local runner. They are not evidence of anything."
            say "  Carry that list into the pull request report's \"not tested and why\" section;"
            say "  CI is what runs the rest."
            finish 0
            ;;
        *)
            # verdict_for returned something outside its own contract. Exit 0
            # is the one answer that must never be reached by falling through a
            # branch nobody wrote.
            say "  ${C_RED}The exit policy returned '$verdict', which is not one of its four answers.${C_RESET}"
            say "  That is a bug in gates.sh, not a result. Exiting 2 rather than report a verdict."
            finish 2
            ;;
    esac
}

main "$@"
