#!/bin/sh
#
# selftest.sh: proves the contract README.md states for agent.sh and
# make-repo.sh. Fixture ORI-T-0076, criterion ORI-P1-009, AICD §14.
#
#     sh selftest.sh
#
# Prints one `ok` or `not ok` line per check and a total, and exits 0 only if
# every check passed. It writes only under one directory it makes with mktemp
# (under $TMPDIR, else /tmp) and removes it on the way out. It needs git, which
# the engine itself needs. It takes about five seconds.
#
# EVERY RUN OF THE AGENT HERE IS BOUNDED BY THIS SCRIPT, NOT BY THE AGENT.
# The point of the "never hangs" checks is to catch an agent that has stopped
# being bounded, so none of them may depend on the agent being right. Each run
# is a background process with a watchdog that kills it after LIMIT seconds and
# records that it did; a run the watchdog had to kill is a failed check, and
# the third such kill ends the whole self-test, so a hung agent costs a
# bounded handful of seconds and not a minute per check.
#
# Check names are the criterion they serve, then what is checked.

set -u

here=$(cd "$(dirname "$0")" && pwd)
agent="$here/agent.sh"
work=$(mktemp -d "${TMPDIR:-/tmp}/looping-agent-selftest.XXXXXX") || exit 2
# No EXIT trap: bash runs one inside every command substitution too, which
# would delete the working directory out from under the checks. `finish` does
# the cleanup, and a signal does it on the way out.
trap 'rm -rf "$work"; exit 130' INT TERM HUP

# The longest any one run may take. The agent's own longest legitimate run in
# these checks is two seconds.
LIMIT=6

passed=0
failed=0
kills=0

pass() {
    passed=$((passed + 1))
    printf 'ok - %s\n' "$1"
}

fail() {
    failed=$((failed + 1))
    printf 'not ok - %s\n' "$1"
    shift
    for line in "$@"; do
        printf '    %s\n' "$line"
    done
}

finish() {
    rm -rf "$work"
    printf 'selftest: %s passed, %s failed\n' "$passed" "$failed"
    [ "$failed" -eq 0 ]
    exit $?
}

# expect <name> <got> <want>
expect() {
    if [ "$2" = "$3" ]; then
        pass "$1"
    else
        fail "$1" "got:  $2" "want: $3"
    fi
}

# run <stdin_path> <cwd> [agent arguments...]
#
# Runs `sh agent.sh ...` in <cwd>, with standard input opened read-write on
# <stdin_path>, and sets:
#   rc        its exit status
#   out, err  its standard output and standard error
#   killed    1 if the watchdog had to kill it, else 0
#   took      whole seconds it ran for
# Three settings change what is run, and are reset to their defaults by the
# caller: run_prefix (a command put in front of the shell, such as `env -i`),
# run_sh (the shell to run) and term_after (send SIGTERM after that many
# seconds; empty for never).
run_prefix=
run_sh=sh
term_after=
run() {
    stdin_path=$1
    cwd=$2
    shift 2
    rm -f "$work/killed"
    start=$(date +%s)
    (
        cd "$cwd" || exit 125
        # shellcheck disable=SC2086
        exec $run_prefix "$run_sh" "$agent" "$@"
    ) <>"$stdin_path" >"$work/out" 2>"$work/err" &
    pid=$!
    sh -c 'sleep "$1"; : > "$3"; kill -9 "$2" 2>/dev/null' watchdog \
        "$LIMIT" "$pid" "$work/killed" >/dev/null 2>&1 &
    dog=$!
    if [ -n "$term_after" ]; then
        sh -c 'sleep "$1"; kill -TERM "$2" 2>/dev/null' terminator \
            "$term_after" "$pid" >/dev/null 2>&1 &
        terminator=$!
    fi
    { wait "$pid"; } 2>/dev/null
    rc=$?
    kill "$dog" 2>/dev/null
    if [ -n "$term_after" ]; then kill "$terminator" 2>/dev/null; fi
    took=$(($(date +%s) - start))
    out=$(cat "$work/out")
    err=$(cat "$work/err")
    if [ -e "$work/killed" ]; then
        killed=1
        kills=$((kills + 1))
    else
        killed=0
    fi
    if [ "$kills" -ge 3 ]; then
        fail 'ori_p1_009_the_agent_hangs' "the watchdog killed $kills runs; stopping here"
        finish
    fi
}

empty="$work/empty"
mkdir "$empty"
fifo="$work/never-written"
mkfifo "$fifo"

# ---------------------------------------------------------------------------
# An attempt fails, always, in the one way the headless adapter reports.
# ---------------------------------------------------------------------------

run /dev/null "$empty"
expect 'ori_p1_009_an_attempt_exits_3' "$rc" 3
expect 'ori_p1_009_an_attempt_does_not_hang' "$killed" 0
case "$out" in
    *'looping-agent: result=not-converged') pass 'ori_p1_009_an_attempt_ends_on_the_not_converged_line' ;;
    *) fail 'ori_p1_009_an_attempt_ends_on_the_not_converged_line' "got: $out" ;;
esac

# Two attempts are two failures that differ in nothing but the attempt number,
# which is what lets a blocked report show "the two attempts".
run /dev/null "$empty" --attempt 1
one_rc=$rc
one_out=$out
run /dev/null "$empty" --attempt 2
two_rc=$rc
two_out=$out
expect 'ori_p1_009_both_attempts_exit_3' "$one_rc,$two_rc" '3,3'
case "$one_out" in
    *'attempt=1'*) pass 'ori_p1_009_attempt_1_says_so' ;;
    *) fail 'ori_p1_009_attempt_1_says_so' "got: $one_out" ;;
esac
case "$two_out" in
    *'attempt=2'*) pass 'ori_p1_009_attempt_2_says_so' ;;
    *) fail 'ori_p1_009_attempt_2_says_so' "got: $two_out" ;;
esac
expect 'ori_p1_009_the_two_attempts_differ_only_in_the_attempt_number' \
    "$(printf '%s' "$one_out" | sed 's/attempt=1/attempt=N/')" \
    "$(printf '%s' "$two_out" | sed 's/attempt=2/attempt=N/')"

# Never converging is deterministic: the same arguments give the same bytes.
run /dev/null "$empty" --attempt 2
expect 'ori_p1_009_the_same_arguments_give_the_same_output' "$out" "$two_out"

# ---------------------------------------------------------------------------
# It is a well-behaved process for the headless adapter to launch.
# ---------------------------------------------------------------------------

# The engine passes its own flags and the real prompt in argv. They are data.
run /dev/null "$empty" -p 'prompt text, never echoed' --dangerously-skip-permissions --attempt 2
expect 'ori_p1_009_the_engines_flags_and_prompt_do_not_change_the_outcome' "$rc,$out" "3,$two_out"

# Standard input is a pipe that is open and never written to: a read from it
# would block for ever. The agent must not read it.
run "$fifo" "$empty"
expect 'ori_p1_009_it_never_reads_standard_input' "$rc,$killed" '3,0'

# Standard error stays empty: the adapter keeps standard output and nothing else.
expect 'ori_p1_009_it_writes_nothing_to_standard_error' "$err" ''

# It writes no file: the directory it ran in is still empty.
expect 'ori_p1_009_it_writes_no_file' "$(ls -A "$empty")" ''

# It reads no environment variable: with an empty environment (not even PATH)
# the default path still gives the same bytes, and a planted variable does not
# appear in what it prints.
run_prefix='env -i'
run_sh=$(command -v sh)
run /dev/null "$empty" --attempt 2
expect 'ori_p1_009_it_needs_nothing_from_the_environment' "$rc,$out" "3,$two_out"
run_prefix='env LOOPING_AGENT_CANARY=canary-not-a-credential'
run_sh=sh
run /dev/null "$empty" --attempt 2
run_prefix=
case "$out" in
    *canary-not-a-credential*) fail 'ori_p1_009_it_does_not_print_the_environment' "got: $out" ;;
    *) pass 'ori_p1_009_it_does_not_print_the_environment' ;;
esac

# What it prints fits the headless adapter's capture bound (STDOUT_CAP in
# crates/ori-runtime/src/headless.rs is 4000 bytes) many times over.
bytes=$(printf '%s\n' "$out" | wc -c | tr -d ' ')
if [ "$bytes" -lt 400 ]; then
    pass 'ori_p1_009_its_output_is_far_inside_the_adapters_capture_bound'
else
    fail 'ori_p1_009_its_output_is_far_inside_the_adapters_capture_bound' "got: $bytes bytes"
fi

# ---------------------------------------------------------------------------
# Every way it ends is bounded.
# ---------------------------------------------------------------------------

run /dev/null "$empty" --delay 2 --attempt 1
expect 'ori_p1_009_a_delayed_attempt_still_fails_the_same_way' "$rc,$killed" '3,0'
if [ "$took" -ge 1 ]; then
    pass 'ori_p1_009_a_delay_is_really_waited_for'
else
    fail 'ori_p1_009_a_delay_is_really_waited_for' "took: ${took}s for --delay 2"
fi

for bad in '--delay 11' '--delay 99999999999999999999' '--delay -1' '--delay x' '--delay' \
    '--delay 1.5' '--attempt' '--attempt x' '--attempt 1234567' '--attempt -1'; do
    # shellcheck disable=SC2086
    run /dev/null "$empty" $bad
    case "$out" in
        *not-converged*) printed=1 ;;
        *) printed=0 ;;
    esac
    expect "ori_p1_009_a_bad_argument_is_refused_without_running_($bad)" "$rc,$killed,$printed" '64,0,0'
done

# SIGTERM, one second into a ten second delay, ends the attempt at once and not
# after the delay; 143 is the status a shell reports for a child that died of SIGTERM.
term_after=1
run /dev/null "$empty" --delay 10
term_after=
if [ "$rc" -eq 143 ] && [ "$took" -le 3 ] && [ "$killed" -eq 0 ]; then
    pass 'ori_p1_009_sigterm_ends_a_delayed_attempt_at_once'
else
    fail 'ori_p1_009_sigterm_ends_a_delayed_attempt_at_once' "exit $rc after ${took}s (killed=$killed), want 143 within 3s"
fi

# SIGKILL, one second into a ten second delay, which is what the headless
# adapter's `kill` sends: the pipe a caller reads the agent's output from must
# close at once, although the `sleep` the agent leaves behind runs on. The
# adapter reads that pipe to end of file before it waits for the process, so a
# `sleep` that held it open would hold the adapter for up to ten more seconds.
rm -f "$work/pid" "$work/closed"
(
    cd "$empty" || exit 125
    exec sh -c 'echo $$ > "$1"; exec sh "$2" --delay 10' pidwriter "$work/pid" "$agent"
) </dev/null 2>/dev/null | { cat >/dev/null; : >"$work/closed" 2>/dev/null; } &
sleep 1
kill -9 "$(cat "$work/pid")" 2>/dev/null
waited=0
while [ ! -e "$work/closed" ] && [ "$waited" -lt 3 ]; do
    sleep 1
    waited=$((waited + 1))
done
if [ -e "$work/closed" ]; then
    pass 'ori_p1_009_sigkill_leaves_no_descendant_holding_the_callers_pipe'
else
    fail 'ori_p1_009_sigkill_leaves_no_descendant_holding_the_callers_pipe' 'the pipe was still open 3s after the kill'
fi

# ---------------------------------------------------------------------------
# The script is what the README says it is.
# ---------------------------------------------------------------------------

cr=$(printf '\r')
for script in agent.sh make-repo.sh selftest.sh; do
    if grep -q "$cr" "$here/$script"; then
        fail "ori_p1_009_${script}_has_lf_line_endings" "found a carriage return in $script"
    else
        pass "ori_p1_009_${script}_has_lf_line_endings"
    fi
done

# A tripwire, not a proof: the proof is that the script is short enough to read
# and README.md lists the one external command it runs. Comments are skipped.
code=$(grep -v '^[[:space:]]*#' "$agent")
words='(^|[^A-Za-z_.-])(curl|wget|nc|ncat|ssh|scp|ftp|telnet|git|cat|eval|source)([^A-Za-z_.-]|$)|/dev/(tcp|udp)'
if printf '%s\n' "$code" | grep -Eq "$words"; then
    fail 'ori_p1_009_agent_names_no_network_tool_and_no_eval' "$(printf '%s\n' "$code" | grep -En "$words")"
else
    pass 'ori_p1_009_agent_names_no_network_tool_and_no_eval'
fi

# ---------------------------------------------------------------------------
# The fixture becomes a repository of its own.
# ---------------------------------------------------------------------------

repo="$work/repo"
if sh "$here/make-repo.sh" "$repo"; then
    pass 'ori_p1_009_make_repo_succeeds_on_a_new_directory'
else
    fail 'ori_p1_009_make_repo_succeeds_on_a_new_directory' 'make-repo.sh failed'
fi
expect 'ori_p1_009_the_repository_has_exactly_one_commit_on_main' \
    "$(git -C "$repo" rev-list --count main 2>&1)" 1
expect 'ori_p1_009_the_repository_tree_is_clean' "$(git -C "$repo" status --porcelain 2>&1)" ''
expect 'ori_p1_009_the_repository_tracks_every_file_of_the_fixture' \
    "$(git -C "$repo" ls-files | sort | tr '\n' ' ')" \
    "$(cd "$here" && ls -A | sort | tr '\n' ' ')"
expect 'ori_p1_009_the_agent_keeps_its_executable_bit_in_the_repository' \
    "$(git -C "$repo" ls-files -s agent.sh | cut -d' ' -f1)" 100755

# The agent behaves the same from inside the repository's own worktree, which
# is where the engine launches it.
git -C "$repo" worktree add --quiet "$work/worktree" >/dev/null 2>&1
run /dev/null "$work/worktree" --attempt 2
expect 'ori_p1_009_the_agent_fails_the_same_way_from_a_worktree_of_the_repository' \
    "$rc,$out" "3,$two_out"

# Same bytes, same commit.
sh "$here/make-repo.sh" "$work/repo2"
expect 'ori_p1_009_the_same_bytes_give_the_same_commit_hash' \
    "$(git -C "$work/repo2" rev-parse HEAD)" "$(git -C "$repo" rev-parse HEAD)"

if sh "$here/make-repo.sh" "$repo" 2>/dev/null; then
    fail 'ori_p1_009_make_repo_refuses_a_destination_that_is_not_empty' 'it returned success'
else
    pass 'ori_p1_009_make_repo_refuses_a_destination_that_is_not_empty'
fi

finish
