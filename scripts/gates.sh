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
#                    when cargo-audit appears on PATH the reason for gate 7
#                    changes by itself. The list cannot quietly go stale.
#
#   blocked          gates.sh has a runner and this machine could not run it.
#                    The coder usually fixes it in one command, and until they
#                    do, their code has been checked by nobody. Exit 3.
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
GATE_NAME[7]='cargo-audit, cargo-deny, secret scan'
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
# Gates 3 to 14: gates.sh has no runner for any of them. Each one is probed
# rather than assumed, so the reason printed is a fact about this machine and
# this tree at this moment.
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

gate_7() {
    probe_unavailable 7 'tooling or configuration' cmd:cargo-audit cmd:cargo-deny path:deny.toml
    # Third-party packages carry a source line in Cargo.lock; the workspace's
    # own path crates do not. This is what cargo-audit and cargo-deny would
    # have to judge.
    local third_party=0
    if [ -f "$REPO_ROOT/Cargo.lock" ]; then
        third_party=$(grep -c '^source = ' "$REPO_ROOT/Cargo.lock" 2>/dev/null || true)
    fi
    if [ "$third_party" -eq 0 ]; then
        append_note 7 'Cargo.lock names 0 third-party packages today, so an advisory or license check here could not be seen to fail on a planted defect (AICD §14)'
    else
        append_note 7 "Cargo.lock names $third_party third-party package(s), so an advisory and license check has something to judge; wire it here"
    fi
    append_note 7 'the secret scan covers tree and history and has no local runner either'
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
    # Two reasons, and both are stated rather than assumed. The runner does not
    # exist, and in this fleet the lead makes the commits while the coder
    # produces the work, so the commits this gate reads are not made in this
    # worktree. The count below is what this branch actually carries.
    probe_unavailable 13 'runner' path:crates/ori-gates/src/trailers.rs
    local own=''
    if command -v git >/dev/null 2>&1; then
        own=$(git rev-list --count main..HEAD 2>/dev/null || true)
    fi
    if [ -n "$own" ]; then
        append_note 13 "in this fleet the lead makes the commits and the coder produces the work, and this branch carries $own commit(s) of its own, so the trailers this gate reads are not written here"
    else
        append_note 13 'in this fleet the lead makes the commits and the coder produces the work, and no commit count could be read here, so the trailers this gate reads are not written here'
    fi
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

    STAGE='reading arguments'
    while [ "$#" -gt 0 ]; do
        case "$1" in
            -h|--help) usage; finish 0 ;;
            --self-check) only_self_check=1; shift ;;
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

    STAGE='gates 3 to 14, availability probes'
    head1 "Gates 3 to 14: probing availability"
    gate_3; gate_4; gate_5; gate_6; gate_7; gate_8
    gate_9; gate_10; gate_11; gate_12; gate_13; gate_14

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
                say "  ${C_RED}INCOMPLETE.${C_RESET} $blocked gate(s) that gates.sh can run did not run on this machine."
                say "  What they cover has been checked by nobody. The summary names each one and what to install."
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
