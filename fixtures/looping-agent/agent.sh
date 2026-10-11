#!/bin/sh
#
# agent.sh: a stand-in agent that never converges. Fixture ORI-T-0076,
# criterion ORI-P1-009, AICD §12 (budgets).
#
# It is launched through the engine's headless adapter as an ordinary
# non-interactive CLI, and every run is one failed attempt:
#
#     sh agent.sh [--attempt N] [--delay S] [anything else]
#
# WHAT IT DOES. It prints a few fixed lines on standard output, optionally
# waits S seconds first, and exits with status 3, which the headless adapter
# reports as StopReason::Other("exit 3"). It does nothing else. It never
# succeeds: there is no argument, no environment and no file that changes the
# outcome, because a stand-in that could converge is not a looping agent.
#
# EXIT STATUS.  3  the attempt ran and did not converge (the only outcome).
#               64 the arguments were wrong (usage); nothing else happened.
#               Any other status is not this script's doing (the shell could
#               not start it, or something killed it).
#
# EVERY WAY IT ENDS IS BOUNDED. There is no loop. The only wait is --delay,
# which is refused above 10 seconds, so the longest possible run is 10 seconds
# plus process start. A signal ends a run at once, whatever it is doing: the
# script sets no trap, so the default action applies, and the status a shell
# sees is 128 plus the signal (143 for SIGTERM). The `sleep` it may leave
# behind is at most 10 seconds long and holds none of this script's standard
# streams, so the caller's pipes close the moment the script dies.
#
# WHAT IT DOES NOT TOUCH. It reads no environment variable, no file and no
# standard input. It writes no file, and writes nothing to standard error: its
# whole transcript is standard output, because that is all the headless
# adapter keeps. It calls no network and spawns nothing except that one
# `sleep`. The engine injects credentials into a session's environment at
# spawn, so the point about the environment is a point about credentials: this
# script cannot print, store or forward one, because it never looks.
#
# ARGUMENTS ARE DATA. The engine passes the real prompt and its own flags in
# argv (`-p "<prompt>" --dangerously-skip-permissions`, say). Anything this
# script does not recognise is ignored and never echoed, evaluated or
# executed.
#
# POSIX sh only: no bashisms, no external command but `sleep`, and `sleep` only
# when --delay is above zero. Line endings must be LF; .gitattributes beside
# this file sees to that.

set -u

attempt=unknown
delay=0

usage() {
    printf 'looping-agent: usage error: %s\n' "$1"
    printf 'looping-agent: usage: sh agent.sh [--attempt N] [--delay S]\n'
    exit 64
}

# A value is a short run of decimal digits and nothing else, so that nothing
# a caller passes can reach the output unchecked or make a number overflow.
digits() {
    case "$1" in
        '' | *[!0-9]*) return 1 ;;
    esac
    [ "${#1}" -le "$2" ]
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --attempt)
            [ "$#" -ge 2 ] || usage '--attempt needs a value'
            digits "$2" 6 || usage '--attempt takes 1 to 6 digits'
            attempt=$2
            shift 2
            ;;
        --delay)
            [ "$#" -ge 2 ] || usage '--delay needs a value'
            digits "$2" 2 || usage '--delay takes 0 to 10 seconds'
            [ "$2" -le 10 ] || usage '--delay takes 0 to 10 seconds'
            delay=$2
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

printf 'looping-agent: stand-in agent, not a model (fixture ORI-T-0076)\n'
printf 'looping-agent: attempt=%s\n' "$attempt"

if [ "$delay" -gt 0 ]; then
    # Detached from every stream, so that if this script is killed the sleep
    # it leaves behind cannot hold the caller's pipes open for its remaining
    # seconds; a foreground sleep would inherit them. Waited for, so that the
    # delay is still part of this run.
    sleep "$delay" </dev/null >/dev/null 2>&1 &
    wait "$!"
fi

printf 'looping-agent: result=not-converged\n'
exit 3
