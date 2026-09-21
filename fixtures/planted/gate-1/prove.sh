#!/usr/bin/env bash
#
# prove.sh: the proof harness for gate 1 of spec/CI_CD.md section 1.
#
# Ticket: ORI-T-0013.  Spec: spec/runbooks/prove-gate.md, spec/TESTING.md
# section 4, spec/CI_CD.md section 1 gate 1.  Proof: ops/gates/gate-1.md.
# Criterion served: ORI-P1-013.
#
# ===========================================================================
# WHAT THIS SCRIPT IS FOR
# ===========================================================================
#
# AICD §14: "a gate is installed only when it has been seen to fail". A gate
# that has only ever been observed passing is not installed, because the
# recurring defect class the rule exists for is "present but reporting
# nothing": "a checker that exits successfully on every input, a workflow file
# whose trigger never fires, a test stage that a deploy path bypasses".
#
# A one-time record that a human once saw gate 1 fail satisfies the letter of
# that rule and expires the moment somebody edits the gate. This script is the
# stronger form: it re-presents the planted defects to gate 1 on every pull
# request, and it fails the run when the gate stops catching them.
#
# ===========================================================================
# THE FOUR QUESTIONS, AND WHY EACH NEEDS ITS OWN ANSWER
# ===========================================================================
#
# "Gate 1 caught the planted defect" is four claims, and the first version of
# this harness checked only the second one. Review broke it three times, in the
# three places it was not looking. All four are checked now, each in its own
# stage:
#
#   1. IS THIS A COMMAND CI RUNS?  Stage 4.  The old check was an anchored
#      grep for the command string anywhere in `.github/workflows/ci.yml`. A
#      line can match while belonging to a job that is not in the required
#      aggregate's `needs:`, or a job carrying `continue-on-error:` or an
#      `if:`, or while being text inside another step's shell script, or while
#      the workflow's triggers mean nothing fires on a pull request at all.
#      Every one of those leaves the byte-identical line in the file. Gate 1's
#      fmt half was switched off entirely, in exactly that way, and this
#      harness reported that the proof held. The file is now parsed
#      (`workflow-facts.awk`) and the question asked of the structure.
#
#   2. DOES GATE 1 GIVE EACH INPUT THE VERDICT IT OWES?  Stages 7 and 8.
#      Three planted packages, two halves of the gate, six cases, judged by one
#      comparator run twice.
#
#   3. IS THE FAILURE THE ONE THIS PROOF PLANTED?  Stage 6.  The old check
#      read "the gate command exited non-zero" as "the gate caught the planted
#      defect". It never asked what the command was complaining about. A
#      fixture with a syntax error, an unparsable `rustfmt.toml`, a missing
#      file or a broken manifest also exits non-zero, and this harness reported
#      the proof held for a formatting gate presented with no formatting
#      defect, and for a lint gate presented with no lint. That is this gate's
#      own defect class reproduced inside its proof: a check reporting success
#      for a state that is not the success it claims. Each case now names the
#      signature its planted mechanism leaves in the output, and a failure
#      without that signature proves nothing.
#
#   4. IS THE JOB THAT ASKS THE FIRST THREE ITSELF LIVE?  Stage 4b.  Question
#      1 was asked about the `fmt` and `clippy` jobs and never about
#      `gate-1-proof`, the job that runs this script. `continue-on-error: true`
#      on that one line turns every refusal below into a green check: the
#      harness still runs, still refuses, still writes its annotation, and
#      GitHub records the job as a success. That is this gate's own defect
#      class at one more level of recursion, the check that proves the gate
#      cannot be weakened being weakenable by the exact mechanism it rejects in
#      others. The same six structural conditions are now asked of this
#      script's own job, which it finds in the file by the command that invokes
#      it and by name, with both required to agree.
#
#      WHERE THAT RECURSION STOPS, said plainly because it does not stop inside
#      this file. A refusal from this script fails this script's job, and the
#      `ci` aggregate reads that job's result, so as the file stands the
#      refusal fails the run. If a later commit puts `continue-on-error:` on
#      this job anyway, this script refuses, the refusal is on the pull request
#      as an annotation, and the run is green, because that is what the key
#      means. What changes is that the weakening is stated out loud by the
#      thing being weakened instead of passing in silence. The last link is
#      outside this file: a human reading the diff of
#      `.github/workflows/ci.yml`, which is a tier 2 path for this reason.
#
# ===========================================================================
# THE INVERSION, WHICH IS THE DANGEROUS PART
# ===========================================================================
#
# This job's success condition is the opposite of every other job in the
# pipeline: it passes when the command it runs fails. Getting that backwards
# produces a job that passes on every input, which is the exact defect AICD §14
# names. Three things guard it, and none of them is a comment.
#
#   1. The case table below is not all failures. `clean/` is an input gate 1
#      must PASS. One comparator judges all six cases, so a harness that
#      reported failure for everything would disagree with the four clean rows
#      and this run would go red.
#
#   2. The comparator is run twice over one set of observations: once against
#      the real expectations, and once against every expectation flipped. Row
#      by row, the two passes must reach OPPOSITE conclusions. A comparator
#      that always answers "agree" agrees in both passes; one that always
#      answers "DISAGREE" disagrees in both; either way a row matches itself
#      and this run reports that the harness, not the gate, is broken. That
#      second pass is this harness's own planted defect, and it runs on every
#      invocation rather than once.
#
#      The check is per row, and it is not a count. The first version of it
#      asserted that the flipped pass must disagree on all six rows, which is
#      only true while the direct pass agrees on all six. Repairing the planted
#      formatting defect, as an experiment, made the flipped pass disagree on
#      five, and the harness reported ITSELF broken for a defect that was in
#      the fixture. ops/gates/gate-1.md records that run. Per-row
#      complementarity is the property actually wanted and it holds whatever
#      the gate does.
#
#   3. The liveness check of stage 4 is a new checker, so it gets the same
#      treatment before it is trusted: stage 3 runs it over the planted
#      workflows in `workflow-samples/`, one of which is live, ten of which
#      are dead in ten different ways and one of which cannot be read. A
#      liveness check that answered "live" to everything, or "dead" to
#      everything, disagrees with that table and this run reports the harness
#      broken. The attribution check of stage 6 is exercised the same way,
#      against one text and two patterns that must reach opposite answers.
#
# Separating observation from judgement is what makes those passes honest: the
# six gate commands run once, their exit statuses and their output are
# recorded, and the judgement is a pure function of what was recorded.
#
# ===========================================================================
# CAPTURING EXIT STATUS
# ===========================================================================
#
# ops/gates/branch-protection.md records a failure of this project's own
# making: a proof harness wrote `if git push ... | tee log | tail -6; then`,
# and a shell pipeline returns the status of its LAST command, so the test read
# `tail`, which always succeeds, and reported that a refused push had
# succeeded. The rule it earned: a gate must capture the exit status of the
# command it is gating, before that command's output is piped anywhere.
#
# There is no pipeline anywhere in run_gate below. The gate command's output is
# redirected to a file and `$?` is read on the next line.
#
# `set -e` is deliberately NOT used. Half the commands here are expected to
# fail, and an errexit shell that leaves before the verdict is a gate that
# exits 0 without checking anything. The EXIT trap below turns any such early
# departure into exit 2.
#
# ===========================================================================
# EXIT STATUS
# ===========================================================================
#
#   0  the proof holds: gate 1 is run by a job whose failure fails the run,
#      the job that runs this script is live by those same six conditions, gate
#      1 passed every input it should pass, it failed every input it should
#      fail, each failure carried the signature of the defect planted for it,
#      and this harness was shown able to tell those answers apart. Whether a
#      failed run blocks a merge is branch protection and not this file; the
#      verdict text states what is and is not established.
#   1  gate 1 did not behave the way this proof claims. Either the gate has
#      been weakened or a planted defect no longer contains a defect. Gate 1
#      may not be cited until this is 0 again (spec/runbooks/prove-gate.md,
#      "Rollback: revoke the proof").
#   2  this harness could not prove what it claims: it could not tell agreement
#      from disagreement, or the workflow no longer runs the command under
#      proof where its failure would fail the run, or it no longer runs this
#      script there, or this script could not find itself in the workflow at
#      all, or a gate command failed for a reason that is not the planted
#      defect. Nothing was proven either way.
#   3  a prerequisite is missing, so no gate command ran at all. Nothing was
#      checked.
#
# Run it from anywhere: `bash fixtures/planted/gate-1/prove.sh`.

set -uo pipefail

# ---------------------------------------------------------------------------
# Location
# ---------------------------------------------------------------------------

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
WORKFLOW_REL='.github/workflows/ci.yml'
AWK_PARSER="$HERE/workflow-facts.awk"
SAMPLE_DIR="$HERE/workflow-samples"

# The two commands under proof, written once here and compared against the
# workflow file below so that this script cannot drift from the gate it claims
# to be proving.
FMT_CMD='cargo fmt --all --check'
CLIPPY_CMD='cargo clippy --workspace --all-targets --locked -- -D warnings'

# The job that is to be the one required check on `main`, and the event that
# must fire the workflow. A gate command whose job is not gated by the first,
# or whose workflow does not answer the second, runs without failing anything.
#
# `ci` is NOT a required status check today. ops/gates/branch-protection.md
# records that as deliberate ("a required check that never reports leaves every
# pull request pending indefinitely") and records when it changes: "each check
# name is added on the day its proof file lands, starting with ORI-T-0013". So
# the strongest true statement this script can make about the aggregate is that
# a failure reaches it and fails the run. Whether that red run stops a merge is
# a repository setting nothing in this file can read, and the verdict at the
# bottom says so instead of implying otherwise.
AGGREGATE_JOB='ci'
REQUIRED_TRIGGER='pull_request'

# This harness's own job, and the command that runs it. Everything above is a
# question this script asks about other jobs. This is the one it asks about
# itself, and it is the one no version of it asked until now: `prove.sh`
# asserted that neither the `fmt` job nor the `clippy` job nor their steps
# carried `continue-on-error:` or a dead `if:`, and never asserted the same
# about `gate-1-proof`. Add `continue-on-error: true` to that job and every
# refusal this script can issue becomes a green check.
#
# HOW THIS SCRIPT FINDS ITSELF IN THE FILE IT IS PARSING, and why it is both of
# these and not either one:
#
#   by command  SELF_CMD is built from this file's own path relative to the
#               repository root, so it is the string the workflow must write to
#               run THIS script. A harness that looked itself up by job name
#               alone would find nothing after a rename and, written the
#               natural way, would skip its own check and carry on: the
#               self-check switched off by a rename, which is cheaper than the
#               weakening it exists to catch.
#   by name     SELF_JOB is the name that job has. A harness that looked itself
#               up by the command alone would follow a rename without noticing,
#               and a renamed proof job is a deliberate edit to the job that
#               carries the proof. It gets a re-proof, not a guess.
#
# The two must agree: exactly one job runs this script, and it is that job. Any
# other answer is refused (exit 2) rather than reported as a pass.
SELF_JOB='gate-1-proof'
SELF_FILE="$HERE/$(basename -- "${BASH_SOURCE[0]}")"
case "$SELF_FILE" in
    "$REPO_ROOT"/*) SELF_REL="${SELF_FILE#"$REPO_ROOT"/}" ;;
    *)              SELF_REL='' ;;
esac
SELF_CMD="bash $SELF_REL"

# The same two commands as argument vectors, split once here rather than by
# leaving an unquoted variable to the shell's word splitting and globbing.
# Neither command contains a glob character today; this does not depend on that
# staying true.
read -r -a FMT_ARGV <<<"$FMT_CMD"
read -r -a CLIPPY_ARGV <<<"$CLIPPY_CMD"

# ---------------------------------------------------------------------------
# The case table: one row per (input, half of gate 1), with the verdict gate 1
# owes that row and the signature its planted mechanism leaves in the output.
# Three inputs times two halves. Every row is exercised.
#
# The signatures are what makes a failure attributable. For a row that expects
# a FAILURE, every pattern listed must appear in what the gate command printed,
# or the failure is some other failure and this proof has shown nothing. For a
# row that expects a PASS, the patterns are the evidence that the command
# actually ran over the package rather than exiting 0 without looking; a row
# with no pattern at all means the command must have printed nothing, which is
# what a clean `cargo fmt --check` does.
# ---------------------------------------------------------------------------

CASE_PKG=()
CASE_CHK=()
CASE_EXP=()
CASE_SIG=()

add_case() {
    # $1 package, $2 half of gate 1, $3 expected verdict, $4.. the signatures.
    CASE_PKG+=("$1")
    CASE_CHK+=("$2")
    CASE_EXP+=("$3")
    shift 3
    local sig='' pat
    for pat in "$@"; do
        sig="$sig$pat"$'\n'
    done
    CASE_SIG+=("$sig")
}

#         package        half    owed   signature of the mechanism that row is about
add_case  clean          fmt     pass
add_case  clean          clippy  pass   '^[[:space:]]*(Finished|Checking|Compiling)'
add_case  fmt-defect     fmt     fail   '^Diff in .*fmt-defect.src.lib\.rs:[0-9]' \
                                        '^-pub fn sum\(a:i32,b:i32\)->i32\{a\+b\}$'
add_case  fmt-defect     clippy  pass   '^[[:space:]]*(Finished|Checking|Compiling)'
add_case  clippy-defect  fmt     pass
add_case  clippy-defect  clippy  fail   '^error: length comparison to zero$' \
                                        'clippy::len.zero'

CASE_COUNT=${#CASE_PKG[@]}

OBS_VERDICT=()
OBS_STATUS=()
OBS_LOG=()

# ---------------------------------------------------------------------------
# The sample table: the planted defects for the liveness check of stage 4.
# Each file under workflow-samples/ is a whole workflow carrying exactly one
# way of switching gate 1 off, or none. The classification this harness owes
# each of them is here; the check is run against all of them on every
# invocation, before it is used on the real workflow file.
# ---------------------------------------------------------------------------

SAMPLE_FILE=()
SAMPLE_EXP=()

add_sample() {
    SAMPLE_FILE+=("$1")
    SAMPLE_EXP+=("$2")
}

add_sample  live.yml                               live
add_sample  dead-command-absent.yml                dead
add_sample  dead-job-not-in-needs.yml              dead
add_sample  dead-no-real-trigger.yml               dead
add_sample  dead-narrowed-trigger.yml              dead
add_sample  dead-command-is-block-scalar-text.yml  dead
add_sample  dead-job-if.yml                        dead
add_sample  dead-step-if.yml                       dead
add_sample  dead-job-continue-on-error.yml         dead
add_sample  dead-step-continue-on-error.yml        dead
add_sample  dead-aggregate-continue-on-error.yml   dead
add_sample  unreadable-quoted-job-name.yml         unreadable

SAMPLE_COUNT=${#SAMPLE_FILE[@]}

# ---------------------------------------------------------------------------
# The self-identification table: the planted defects for the check of stage 4b,
# which asks the six structural questions about the job that runs this script
# rather than about the jobs it proves. Finding this script in the file is the
# part of that with more ways to go wrong than to go right, so every answer the
# check can give has an input here that is owed it.
#
# Two of the twelve workflows above serve here unchanged, because the answer
# they are owed about gate 1's command is the answer they are owed about this
# script's: `live.yml` runs no such command at all, and a file the parser
# cannot read is unreadable whatever is asked of it.
# ---------------------------------------------------------------------------

SELF_SAMPLE_FILE=()
SELF_SAMPLE_EXP=()

add_self_sample() {
    SELF_SAMPLE_FILE+=("$1")
    SELF_SAMPLE_EXP+=("$2")
}

add_self_sample  self-live.yml                   live
add_self_sample  self-job-continue-on-error.yml  dead
add_self_sample  self-renamed.yml                misnamed
add_self_sample  self-two-jobs.yml               ambiguous
add_self_sample  live.yml                        missing
add_self_sample  unreadable-quoted-job-name.yml  unreadable

SELF_SAMPLE_COUNT=${#SELF_SAMPLE_FILE[@]}

# Every answer classify_self can give, in LC_ALL=C sort order. The table above
# must cover all of them: a check that answered the same way to every input
# would otherwise still agree with a table that asked only one question.
SELF_ANSWERS='ambiguous dead live misnamed missing unreadable'

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

say() { printf '%s\n' "$*"; }

rule() {
    say ''
    say "=== $* ==="
    say ''
}

# A message a human must see. Under GitHub Actions it is additionally emitted
# as a workflow annotation, which is what puts it on the pull request check
# rather than only in the log a human has to open and scroll. That is
# spec/runbooks/prove-gate.md step 3, "visible where a human would look".
emit() {
    local kind="$1"
    shift
    printf '%s: %s\n' "$kind" "$*"
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        printf '::%s file=fixtures/planted/gate-1/prove.sh::%s\n' "$kind" "$*"
    fi
}

# ---------------------------------------------------------------------------
# The floor. No path out of this script reaches exit 0 except the verdict at
# the bottom.
# ---------------------------------------------------------------------------

STAGE='starting up'
VERDICT_REACHED=0
WORK_DIR=''

floor() {
    local status=$?
    if [ "$VERDICT_REACHED" -eq 0 ]; then
        emit error "prove.sh stopped during stage '$STAGE' and was about to exit $status without reaching a verdict. Nothing was proven, so this run reports failure rather than a status that could be read as a pass."
        if [ -n "$WORK_DIR" ] && [ -d "$WORK_DIR" ]; then rm -rf "$WORK_DIR"; fi
        exit 2
    fi
    if [ -n "$WORK_DIR" ] && [ -d "$WORK_DIR" ]; then rm -rf "$WORK_DIR"; fi
    exit "$status"
}
trap floor EXIT

# ---------------------------------------------------------------------------
# The two checks this proof adds, as functions, so that stage 3 can run them
# against planted inputs and stages 4 and 6b can use them against the real
# ones. One implementation, exercised before it is believed.
# ---------------------------------------------------------------------------

# Read a workflow file into the flat facts workflow-facts.awk prints.
# 0 the facts were produced, 2 awk itself could not run.
parse_workflow() {
    # $1 workflow file, $2 facts file to write.
    local wf="$1" facts="$2" status
    awk -f "$AWK_PARSER" "$wf" >"$facts" 2>"$facts.awk-stderr"
    status=$?
    if [ "$status" -ne 0 ]; then
        return 2
    fi
    return 0
}

# Answer, from those facts, whether a command is one this workflow runs where
# its failure would fail the run. Sets LIVENESS to one of:
#
#   live        a job that runs and that the required aggregate waits on
#               carries this command, unguarded
#   dead        the command is not run, or is run somewhere that cannot fail
#               this run: a job the aggregate does not wait on, a job or step
#               carrying `if:` or `continue-on-error:`, a workflow that no
#               pull request fires, or text inside another step's script
#   unreadable  the file has a shape workflow-facts.awk does not read, so no
#               answer about it is possible and none is invented
#
# It also sets LIVENESS_JOBS to the names of the jobs that carry the command,
# whatever the answer. classify_self below asks WHICH job that is, and that is
# a different question from whether the command is live: a command run by two
# jobs can be live, and "the job that runs this script" still names no single
# job.
#
# An `if:` on the gate's job or step is reported dead rather than evaluated.
# This harness does not read GitHub expressions and will not guess whether
# `if: github.actor != 'x'` is true on the run that matters; a gate that may
# not run needs a deliberate re-proof, not a guess.
LIVENESS=''
LIVENESS_WHY=''
LIVENESS_JOBS=''
classify_command() {
    # $1 facts file, $2 command, $3 aggregate job name.
    local facts="$1" cmd="$2" agg="$3"
    local tag j n rest why found live where trigger_why agg_why occ_why keys
    LIVENESS=''
    LIVENESS_WHY=''
    LIVENESS_JOBS=''

    if grep -q '^ERROR ' "$facts"; then
        LIVENESS='unreadable'
        LIVENESS_WHY="$(sed -n 's/^ERROR /      - at line /p' "$facts")"
        return
    fi

    trigger_why=''
    if ! grep -qxF "TRIGGER $REQUIRED_TRIGGER" "$facts"; then
        trigger_why="      - the workflow has no '$REQUIRED_TRIGGER' trigger, so nothing in it runs when a pull request is opened or pushed to, whatever the jobs say"
    else
        keys="$(sed -n "s/^TRIGGERKEY $REQUIRED_TRIGGER //p" "$facts")"
        keys="$(printf '%s' "$keys" | tr '\n' ' ')"
        if [ -n "$keys" ]; then
            trigger_why="      - the '$REQUIRED_TRIGGER' trigger carries the filter key(s) ${keys% }, which narrow which pull requests fire this workflow. That narrowing is a deliberate change to when gate 1 runs, so it needs a deliberate re-proof rather than a harness deciding which pull requests are allowed to skip the gate"
        fi
    fi

    agg_why=''
    if ! grep -qxF "JOB $agg" "$facts"; then
        agg_why="      - the workflow defines no job named '$agg', which is the job ops/gates/branch-protection.md names as the first required check to be added, so nothing here collects the gate jobs into one result"
    elif grep -qxF "JOBKEY $agg continue-on-error" "$facts"; then
        agg_why="      - the aggregate job '$agg' carries 'continue-on-error', so it reports success when it fails and gates nothing"
    fi

    found=0
    live=0
    where=''
    occ_why=''
    while read -r tag j n rest; do
        [ "$tag" = 'RUN' ] || continue
        [ "$rest" = "$cmd" ] || continue
        found=$((found + 1))
        # The jobs that carry this command, deduplicated, in file order. Job
        # names come from workflow-facts.awk's `keyname`, which accepts only
        # `[A-Za-z_][A-Za-z0-9_-]*`, so this list holds no whitespace and no
        # glob character and splitting it on spaces is safe.
        case " $LIVENESS_JOBS " in
            *" $j "*) ;;
            *) LIVENESS_JOBS="${LIVENESS_JOBS:+$LIVENESS_JOBS }$j" ;;
        esac
        why=''
        if [ "$j" = "$agg" ]; then
            why="$why the aggregate job cannot gate itself;"
        fi
        if grep -qxF "JOBKEY $j if" "$facts"; then
            why="$why job '$j' carries an 'if:' key and this harness does not evaluate GitHub expressions;"
        fi
        if grep -qxF "JOBKEY $j continue-on-error" "$facts"; then
            why="$why job '$j' carries 'continue-on-error', so it reports success when this command fails;"
        fi
        if grep -qxF "STEPKEY $j $n if" "$facts"; then
            why="$why step $n of job '$j' carries an 'if:' key;"
        fi
        if grep -qxF "STEPKEY $j $n continue-on-error" "$facts"; then
            why="$why step $n of job '$j' carries 'continue-on-error', so the step's failure is not the job's;"
        fi
        if ! grep -qxF "NEEDS $agg $j" "$facts"; then
            why="$why job '$j' is not in the '$agg' aggregate's needs:, so its failure never reaches '$agg';"
        fi
        if [ -z "$why" ]; then
            live=1
            where="job '$j', step $n"
        else
            occ_why="$occ_why
      - it is step $n of job '$j', but$why"
        fi
    done <"$facts"

    if [ "$found" -eq 0 ]; then
        LIVENESS='dead'
        LIVENESS_WHY="      - no step in this workflow has this command as its 'run:'. A match anywhere else in the file is text, not a command."
        return
    fi
    if [ "$live" -eq 1 ] && [ -z "$trigger_why" ] && [ -z "$agg_why" ]; then
        LIVENESS='live'
        LIVENESS_WHY="      - run by $where, unguarded, and '$agg' waits on that job"
        return
    fi
    LIVENESS='dead'
    LIVENESS_WHY="$trigger_why"
    if [ -n "$agg_why" ]; then
        LIVENESS_WHY="${LIVENESS_WHY:+$LIVENESS_WHY
}$agg_why"
    fi
    if [ -n "$occ_why" ]; then
        LIVENESS_WHY="${LIVENESS_WHY}$occ_why"
    fi
    if [ "$live" -eq 1 ]; then
        LIVENESS_WHY="${LIVENESS_WHY:+$LIVENESS_WHY
}      - $where does run it, but the workflow around it means that failure reaches nobody"
    fi
    # `occ_why` carries its own leading newline so that reasons stack; drop it
    # when it is the first thing in the list.
    LIVENESS_WHY="${LIVENESS_WHY#$'\n'}"
}

# Answer, from those same facts, whether the job that runs THIS script is one
# whose failure fails the run. Sets SELF_STATE to one of:
#
#   live        exactly one job runs this script, it is the job this script
#               identifies itself as, and it meets the same six structural
#               conditions classify_command applies to the jobs it proves
#   dead        that job runs this script where its failure cannot fail the
#               run: `if:` or `continue-on-error:` on the job or the step, a
#               job the aggregate does not wait on, a `continue-on-error:` on
#               the aggregate, or a trigger no pull request fires unfiltered
#   missing     no job runs this script at all
#   misnamed    a job runs this script and it is not the one named by SELF_JOB
#   ambiguous   more than one job runs it, so "the job that runs this script"
#               names no single job and no answer about it is possible
#   unreadable  the file has a shape workflow-facts.awk does not read
#
# Only `live` is a pass. Every other answer is a refusal, including the three
# that are not about weakening at all: a harness that cannot find itself has
# stopped checking itself, and stopping quietly is the defect class this whole
# fixture is about.
SELF_STATE=''
SELF_WHY=''
classify_self() {
    # $1 facts file.
    local facts="$1" n runner
    SELF_STATE=''
    SELF_WHY=''
    classify_command "$facts" "$SELF_CMD" "$AGGREGATE_JOB"
    if [ "$LIVENESS" = 'unreadable' ]; then
        SELF_STATE='unreadable'
        SELF_WHY="$LIVENESS_WHY"
        return
    fi
    # Safe to split: see the note in classify_command about job names.
    set -- $LIVENESS_JOBS
    n=$#
    if [ "$n" -eq 0 ]; then
        SELF_STATE='missing'
        SELF_WHY="      - no step of this workflow has '$SELF_CMD' as its 'run:', so nothing here runs this harness. Either the job that ran it is gone, or its 'run:' was rewritten, or this fixture was moved without the workflow being moved with it."
        return
    fi
    if [ "$n" -gt 1 ]; then
        SELF_STATE='ambiguous'
        SELF_WHY="      - $n jobs run this harness ($LIVENESS_JOBS), so 'the job that runs this script' names no single job. This harness will not pick one of them to report on."
        return
    fi
    runner="$1"
    if [ "$runner" != "$SELF_JOB" ]; then
        SELF_STATE='misnamed'
        SELF_WHY="      - this harness is run by job '$runner' and it identifies itself as '$SELF_JOB', so the job it was written to check does not exist under that name. It refuses rather than quietly stopping checking itself."
        return
    fi
    SELF_STATE="$LIVENESS"
    SELF_WHY="$LIVENESS_WHY"
}

# Remove the colour codes a tool writes for a terminal. `cargo` and `rustfmt`
# colour their output whether or not they are writing to one, and the workflow
# sets CARGO_TERM_COLOR=always on top of that, so `^error: length comparison to
# zero$` matches nothing on the raw bytes: the line really begins with an
# escape sequence. The signatures below are written against what a human reads,
# and this is what makes the two the same text. The gate command's exit status
# is captured by the caller before this runs and cannot be swallowed by it.
strip_ansi() {
    # $1 the captured output, $2 the file to write the readable form to.
    local esc
    esc="$(printf '\033')"
    sed "s/${esc}\[[0-9;]*m//g; s/${esc}(B//g" "$1" >"$2"
}

# Is what the gate command printed the complaint this case planted?
#   0 every signature matched, or none was given and the command said nothing
#   1 a signature did not match: this is not the failure this proof planted
#   2 a signature is not a usable regular expression
ATTRIB_WHY=''
output_matches() {
    # $1 file holding the captured output, $2 newline-separated signatures.
    local log="$1" sigs="$2"
    local pat status any=0
    ATTRIB_WHY=''
    while IFS= read -r pat; do
        [ -n "$pat" ] || continue
        any=1
        grep -Eq -e "$pat" "$log"
        status=$?
        if [ "$status" -eq 1 ]; then
            ATTRIB_WHY="nothing in the output matches /$pat/"
            return 1
        fi
        if [ "$status" -ne 0 ]; then
            ATTRIB_WHY="/$pat/ is not a usable extended regular expression: grep exited $status"
            return 2
        fi
    done <<<"$sigs"
    if [ "$any" -eq 0 ] && [ -s "$log" ]; then
        ATTRIB_WHY='the command printed something on an input it has nothing to say about'
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# Stage 1: what is running this
# ---------------------------------------------------------------------------

STAGE='reporting the environment'
rule 'Environment'
say "gate 1 proof harness, ORI-T-0013, spec/CI_CD.md section 1 gate 1"
say "  repository root : $REPO_ROOT"
say "  fixture root    : $HERE"
say "  platform        : $(uname -s 2>/dev/null || echo unknown) $(uname -m 2>/dev/null || echo unknown)"
if [ -n "${GITHUB_ACTIONS:-}" ]; then
    say "  running under   : GitHub Actions, runner OS ${RUNNER_OS:-unknown}, event ${GITHUB_EVENT_NAME:-unknown}"
else
    say "  running under   : a local shell"
fi

# ---------------------------------------------------------------------------
# Stage 2: the toolchain. A missing component makes gate 1 unrunnable, and a
# harness that cannot run the gate must say so rather than report a pass.
# scripts/gates.sh carries the same distinction and the same reason.
# ---------------------------------------------------------------------------

STAGE='probing the toolchain'
rule 'Toolchain'
probe_failed=0
probe() {
    local label="$1"
    shift
    local out status
    out="$("$@" 2>&1)"
    status=$?
    if [ "$status" -ne 0 ]; then
        emit error "'$*' exited $status, so $label is not usable here and no verdict of gate 1 on this machine would mean anything"
        probe_failed=1
        return
    fi
    printf '  %-16s %s\n' "$label" "${out%%$'\n'*}"
}
probe 'cargo'        cargo --version
probe 'rustc'        rustc --version
probe 'cargo fmt'    cargo fmt --version
probe 'cargo clippy' cargo clippy --version
# awk reads the workflow file. Probed by running a program rather than by
# asking for a version banner, because the three awks this has to run under
# disagree about which version flag they take and agree about this.
probe 'awk'          awk 'BEGIN { print "a usable POSIX awk" }'
if [ "$probe_failed" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "nothing was checked: the toolchain this proof needs is not usable on this machine. This is not a pass and not a failure of gate 1."
    exit 3
fi

WORK_DIR="$(mktemp -d)"
if [ ! -d "$WORK_DIR" ]; then
    VERDICT_REACHED=1
    emit error "could not create a temporary directory to hold the gate output, so nothing was run"
    exit 3
fi

# ---------------------------------------------------------------------------
# Stage 3: assertions about the fixture, and the self-check of the two checks
# stages 4 and 6b are about to perform. Every one of these is a way for this
# proof to be quietly meaningless, so each is checked rather than assumed.
# ---------------------------------------------------------------------------

STAGE='checking the fixture'
rule 'Preconditions'
bad=0

note_bad() { emit error "$*"; bad=1; }

if [ "$CASE_COUNT" -eq 0 ]; then
    note_bad "the case table is empty, so this harness would run no gate command and then report success"
fi

# Both answers must be represented. A table of failures only cannot detect a
# gate that fails on everything; a table of passes only cannot detect a gate
# that passes on everything, which is the AICD §14 defect class itself.
expect_pass=0
expect_fail=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    case "${CASE_EXP[$i]}" in
        pass) expect_pass=$((expect_pass + 1)) ;;
        fail)
            expect_fail=$((expect_fail + 1))
            # A row that expects a failure and names no signature is a row that
            # would accept any failure at all, which is the hole this stage
            # exists to close.
            if [ -z "${CASE_SIG[$i]//$'\n'/}" ]; then
                note_bad "case $i (${CASE_PKG[$i]}, ${CASE_CHK[$i]}) expects gate 1 to fail and names no signature, so any failure whatever would be recorded as the planted defect being caught"
            fi
            ;;
        *) note_bad "case $i expects '${CASE_EXP[$i]}', which is neither 'pass' nor 'fail'" ;;
    esac
    i=$((i + 1))
done
say "  case table       : $CASE_COUNT cases, $expect_pass expected to pass, $expect_fail expected to fail"
if [ "$expect_pass" -eq 0 ]; then
    note_bad "no case expects gate 1 to pass, so this run could not tell a working gate from one that fails on every input"
fi
if [ "$expect_fail" -eq 0 ]; then
    note_bad "no case expects gate 1 to fail, so this run could not tell a working gate from one that passes on every input, which is the defect class of AICD §14"
fi

# The planted packages must be present, must be packages, and must each be
# their own workspace root. Without the `[workspace]` table a planted package
# would be resolved against the repository workspace and the real build would
# stop being green, which is the constraint this fixture layout exists to
# satisfy.
seen_pkgs=' '
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    pkg="${CASE_PKG[$i]}"
    dir="$HERE/$pkg"
    # Each package appears in more than one case; report it once.
    case "$seen_pkgs" in
        *" $pkg "*) i=$((i + 1)); continue ;;
    esac
    seen_pkgs="$seen_pkgs$pkg "
    for required in 'Cargo.toml' 'Cargo.lock' 'src/lib.rs'; do
        if [ ! -f "$dir/$required" ]; then
            note_bad "$pkg/$required is missing, so the input this proof claims to present to gate 1 is not there"
        fi
    done
    if [ -f "$dir/Cargo.toml" ] && ! grep -q '^\[workspace\]' "$dir/Cargo.toml"; then
        note_bad "$pkg/Cargo.toml has no [workspace] table, so this package is not its own workspace root and the repository workspace may resolve it"
    fi
    i=$((i + 1))
done

# The root workspace must not reach into the fixture. `cargo fmt --all` and
# `cargo clippy --workspace` at the repository root resolve the explicit member
# list in the root Cargo.toml; a member under `fixtures/` would put a planted
# defect into the real build.
if [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
    note_bad "$REPO_ROOT/Cargo.toml does not exist, so REPO_ROOT was resolved wrongly and nothing below can be trusted"
elif grep -q 'fixtures' "$REPO_ROOT/Cargo.toml"; then
    note_bad "the root Cargo.toml mentions 'fixtures'. If a planted package has become a workspace member the real build no longer stays green, and this proof is no longer proving what it says."
else
    say "  root workspace   : names no member under fixtures/"
fi

# The parser and its planted workflows.
if [ ! -f "$AWK_PARSER" ]; then
    note_bad "$AWK_PARSER is missing, so this harness cannot read the workflow file and cannot tell whether it is proving a command anything runs"
fi
i=0
while [ "$i" -lt "$SAMPLE_COUNT" ]; do
    if [ ! -f "$SAMPLE_DIR/${SAMPLE_FILE[$i]}" ]; then
        note_bad "workflow-samples/${SAMPLE_FILE[$i]} is missing, so the check that this harness can tell a live gate command from a dead one has lost one of its inputs"
    fi
    i=$((i + 1))
done
i=0
while [ "$i" -lt "$SELF_SAMPLE_COUNT" ]; do
    if [ ! -f "$SAMPLE_DIR/${SELF_SAMPLE_FILE[$i]}" ]; then
        note_bad "workflow-samples/${SELF_SAMPLE_FILE[$i]} is missing, so the check that this harness can find the job that runs it has lost one of its inputs"
    fi
    i=$((i + 1))
done

# This script's own path under the repository root is how it recognises the
# command that runs it. Without it there is no self-identification to do and
# stage 4b would have nothing to ask.
if [ -z "$SELF_REL" ]; then
    note_bad "this script is at $SELF_FILE, which is not under the repository root $REPO_ROOT, so it cannot say what command a workflow at that root would write to run it and cannot check the job that runs it"
else
    say "  this script      : $SELF_REL, invoked as '$SELF_CMD'"
fi

if [ "$bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "the preconditions of this proof do not hold, so no gate command was run. Nothing was proven."
    exit 2
fi

# --- the self-check of the liveness check ----------------------------------
#
# The same argument as the flipped comparator. A liveness check that answered
# "live" to everything would report the gate proved however the workflow was
# weakened; one that answered "dead" to everything would block every run. The
# table below contains all three answers, so neither constant survives it.

STAGE='checking that this harness can tell a live gate command from a dead one'
rule 'Self-check: telling a live gate command from a dead one'
say 'Each file below is a whole workflow carrying exactly one way of switching'
say 'gate 1 off, or none. They are planted defects for the check stage 4 is'
say 'about to run against the real workflow, and they are run on every'
say 'invocation rather than once.'
say ''

sample_live=0
sample_dead=0
sample_unreadable=0
i=0
while [ "$i" -lt "$SAMPLE_COUNT" ]; do
    case "${SAMPLE_EXP[$i]}" in
        live)       sample_live=$((sample_live + 1)) ;;
        dead)       sample_dead=$((sample_dead + 1)) ;;
        unreadable) sample_unreadable=$((sample_unreadable + 1)) ;;
        *)
            VERDICT_REACHED=1
            emit error "sample ${SAMPLE_FILE[$i]} expects '${SAMPLE_EXP[$i]}', which is not one of live, dead, unreadable"
            exit 2
            ;;
    esac
    i=$((i + 1))
done
if [ "$sample_live" -eq 0 ] || [ "$sample_dead" -eq 0 ] || [ "$sample_unreadable" -eq 0 ]; then
    VERDICT_REACHED=1
    emit error "the sample table does not contain all three answers (live $sample_live, dead $sample_dead, unreadable $sample_unreadable), so a liveness check that answered the same way to every input would still agree with it"
    exit 2
fi

printf '  %-40s %-11s %-11s %s\n' 'planted workflow' 'owed' 'answered' 'result'
printf '  %-40s %-11s %-11s %s\n' '----------------------------------------' '-----------' '-----------' '--------'
sample_bad=0
i=0
while [ "$i" -lt "$SAMPLE_COUNT" ]; do
    sample="${SAMPLE_FILE[$i]}"
    facts="$WORK_DIR/sample.$i.facts"
    parse_workflow "$SAMPLE_DIR/$sample" "$facts"
    if [ $? -eq 2 ]; then
        VERDICT_REACHED=1
        emit error "awk could not read workflow-samples/$sample at all, so this harness cannot show that it tells a live gate command from a dead one"
        exit 2
    fi
    classify_command "$facts" "$FMT_CMD" "$AGGREGATE_JOB"
    if [ "$LIVENESS" = "${SAMPLE_EXP[$i]}" ]; then
        outcome='agree'
    else
        outcome='DISAGREE'
        sample_bad=$((sample_bad + 1))
    fi
    printf '  %-40s %-11s %-11s %s\n' "$sample" "${SAMPLE_EXP[$i]}" "$LIVENESS" "$outcome"
    if [ "$outcome" = 'DISAGREE' ]; then
        say "$LIVENESS_WHY"
    fi
    i=$((i + 1))
done
say ''
say "  disagreements: $sample_bad of $SAMPLE_COUNT (this must be 0)"
if [ "$sample_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot tell a gate command a workflow runs from one it does not: $sample_bad of the $SAMPLE_COUNT planted workflows were classified wrongly. Until that is repaired, anything stage 4 says about the real workflow means nothing."
    exit 2
fi

# --- the self-check of the self-identification check ------------------------
#
# The same argument once more, one level in. Stage 4b asks about the job that
# runs this script the six questions stage 4 asks about the jobs it proves, and
# the check that asks them has to find this script in the file first. Every
# answer it can give has a planted input here that is owed it, so no constant
# answer survives this table: not "live" (which would prove nothing on every
# weakened file), not "dead" (which would block every run), and not the three
# answers that mean this harness has lost track of itself.

STAGE='checking that this harness can find the job that runs it'
rule 'Self-check: finding the job that runs this harness, and judging it'
say 'Each file below is a whole workflow that runs this harness, or does not,'
say 'in one of the ways that has an answer of its own. They are run on every'
say 'invocation, before anything is said about the real workflow file.'
say ''

self_answers_seen="$(printf '%s\n' "${SELF_SAMPLE_EXP[@]}" | LC_ALL=C sort -u | tr '\n' ' ')"
self_answers_seen="${self_answers_seen% }"
if [ "$self_answers_seen" != "$SELF_ANSWERS" ]; then
    VERDICT_REACHED=1
    emit error "the self-identification table is owed the answers [$self_answers_seen] and classify_self can give [$SELF_ANSWERS]. A table that does not ask for every answer is a table a check with a blind spot, or a constant answer, could still agree with."
    exit 2
fi

printf '  %-40s %-11s %-11s %s\n' 'planted workflow' 'owed' 'answered' 'result'
printf '  %-40s %-11s %-11s %s\n' '----------------------------------------' '-----------' '-----------' '--------'
self_sample_bad=0
i=0
while [ "$i" -lt "$SELF_SAMPLE_COUNT" ]; do
    sample="${SELF_SAMPLE_FILE[$i]}"
    facts="$WORK_DIR/selfsample.$i.facts"
    parse_workflow "$SAMPLE_DIR/$sample" "$facts"
    if [ $? -eq 2 ]; then
        VERDICT_REACHED=1
        emit error "awk could not read workflow-samples/$sample at all, so this harness cannot show that it finds the job that runs it"
        exit 2
    fi
    classify_self "$facts"
    if [ "$SELF_STATE" = "${SELF_SAMPLE_EXP[$i]}" ]; then
        outcome='agree'
    else
        outcome='DISAGREE'
        self_sample_bad=$((self_sample_bad + 1))
    fi
    printf '  %-40s %-11s %-11s %s\n' "$sample" "${SELF_SAMPLE_EXP[$i]}" "$SELF_STATE" "$outcome"
    if [ "$outcome" = 'DISAGREE' ]; then
        say "$SELF_WHY"
    fi
    i=$((i + 1))
done
say ''
say "  disagreements: $self_sample_bad of $SELF_SAMPLE_COUNT (this must be 0)"
if [ "$self_sample_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot find the job that runs it: $self_sample_bad of the $SELF_SAMPLE_COUNT planted workflows were classified wrongly. One cause is that this fixture was moved and the samples still name its old path, in which case they and $WORKFLOW_REL move together. Until it is repaired, anything stage 4b says about the real workflow means nothing."
    exit 2
fi

# --- the self-check of the attribution check -------------------------------
#
# One text, two patterns, opposite answers owed. A matcher that always agreed,
# or always refused, fails one of the two rows.

STAGE='checking that this harness can tell one failure from another'
rule 'Self-check: telling the planted failure from any other failure'
selfcheck_log="$WORK_DIR/attribution.selfcheck"
printf '%s\n' 'error: length comparison to zero' >"$selfcheck_log"
say "  text        : $(cat "$selfcheck_log")"

output_matches "$selfcheck_log" $'^error: length comparison to zero$\n'
present=$?
output_matches "$selfcheck_log" $'clippy::len.zero\n'
absent=$?
printf '  %-46s %s\n' "a signature the text carries" "match=$present (0 owed)"
printf '  %-46s %s\n' "a signature the text does not carry" "match=$absent (1 owed)"
if [ "$present" -ne 0 ] || [ "$absent" -ne 1 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot tell one failure from another: asked whether a text carries a signature it does have it answered $present (0 owed), and for one it does not have it answered $absent (1 owed). Every attribution below would be meaningless."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4: the command under proof must be a command CI runs, in a job whose
# failure fails the run. Without this the gate could be switched off in ci.yml
# while this harness went on proving a command nothing executes.
# ---------------------------------------------------------------------------

STAGE='checking that the workflow runs the commands under proof'
rule 'The commands under proof are commands CI runs'

if [ ! -f "$REPO_ROOT/$WORKFLOW_REL" ]; then
    VERDICT_REACHED=1
    emit error "$WORKFLOW_REL does not exist, so this harness cannot confirm that it proves a command CI actually runs. Nothing was proven."
    exit 2
fi

workflow_facts="$WORK_DIR/ci.facts"
parse_workflow "$REPO_ROOT/$WORKFLOW_REL" "$workflow_facts"
if [ $? -eq 2 ]; then
    VERDICT_REACHED=1
    emit error "awk could not read $WORKFLOW_REL, so nothing is known about what CI runs. Nothing was proven."
    exit 2
fi

say "  workflow         : $WORKFLOW_REL"
say "  aggregate job    : '$AGGREGATE_JOB', which ops/gates/branch-protection.md names as the first required check to be added and which is not required yet"
say "  trigger required : $REQUIRED_TRIGGER"
say "  triggers found   : $(sed -n 's/^TRIGGER //p' "$workflow_facts" | tr '\n' ' ')"
say ''
for cmd in "$FMT_CMD" "$CLIPPY_CMD"; do
    classify_command "$workflow_facts" "$cmd" "$AGGREGATE_JOB"
    say "  '$cmd'"
    say "    $LIVENESS"
    say "$LIVENESS_WHY"
    case "$LIVENESS" in
        live) ;;
        unreadable)
            note_bad "$WORKFLOW_REL has a shape this harness does not read, so it cannot say whether '$cmd' is a command CI runs. A job it failed to see is a job it would have said nothing about, so it proves nothing rather than proving something about the part it understood."
            ;;
        *)
            note_bad "'$cmd' is not run by any job of $WORKFLOW_REL whose failure would fail this run. Either the gate was weakened, or this harness is proving a command nothing executes. A proof of a command nothing runs is the defect class of AICD §39, so it is refused rather than reported."
            ;;
    esac
done

if [ "$bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "the command under proof is not one CI runs where its failure fails the run, so no gate command was run here. Nothing was proven."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4b: and the job that runs THIS script must be live by the same six
# conditions. Stage 4 asks whether gate 1's commands can fail the run. Until
# now nothing asked whether the job asking that question can: `prove.sh`
# asserted that neither the `fmt` job nor the `clippy` job nor their steps
# carried `continue-on-error:` or a dead `if:`, and never asserted it about
# `gate-1-proof`. One line there leaves every check on the pull request green
# and switches the whole demonstration off.
# ---------------------------------------------------------------------------

STAGE='checking that the job which runs this harness can itself fail the run'
rule 'The job that runs this harness is one whose failure fails the run'
say "  this script      : $SELF_REL"
say "  CI must run it as: '$SELF_CMD'"
say "  and under the job name '$SELF_JOB'. Both, and they must be the same job."
say ''
classify_self "$workflow_facts"
say "  $SELF_STATE"
say "$SELF_WHY"
if [ "$SELF_STATE" != 'live' ]; then
    VERDICT_REACHED=1
    case "$SELF_STATE" in
        dead)
            emit error "the job that runs this harness is not one whose failure fails the run, for the reason printed above. Every refusal this script can issue then fails nothing: the demonstration AICD §14 asks for is switched off while every check on the pull request stays green, and the only trace is this annotation. That is the defect class this fixture exists for, applied to the fixture itself. Nothing was proven."
            ;;
        missing)
            emit error "no job of $WORKFLOW_REL runs this harness ('$SELF_CMD'), so nothing in CI presents these planted defects to gate 1 and this run says nothing about any commit but this one. A proof CI does not run is the defect class of AICD §39. Nothing was proven."
            ;;
        misnamed)
            emit error "this harness is run by a job that is not '$SELF_JOB', so the job it was written to check no longer exists under that name. A renamed proof job is a deliberate edit to the job that carries the proof: update SELF_JOB in this script in the same commit and re-prove. This harness refuses rather than silently stopping checking itself. Nothing was proven."
            ;;
        ambiguous)
            emit error "more than one job of $WORKFLOW_REL runs this harness, so 'the job that runs this script' names no single job and this run cannot say whose liveness it checked. Nothing was proven."
            ;;
        *)
            emit error "$WORKFLOW_REL has a shape this harness does not read, so it cannot say whether the job that runs it can fail the run. Nothing was proven."
            ;;
    esac
    exit 2
fi
say ''
say "  So a refusal from this script fails this run, as the file stands. It does not"
say "  follow that it always will. 'continue-on-error:' added to '$SELF_JOB' later"
say "  makes this check refuse, and a job carrying that key does not fail the run when"
say "  it refuses: the refusal becomes an annotation on the pull request, which is a"
say "  human noticing rather than a gate failing. The recursion does not terminate"
say "  inside this file, and the verdict at the bottom says so rather than implying"
say "  that it does."

# ---------------------------------------------------------------------------
# Stage 5: observation. Run each case once. Record the exit status and the
# output. Judge nothing yet.
# ---------------------------------------------------------------------------

STAGE='running gate 1 against every input'

run_gate() {
    # $1 package directory, $2 check, $3 output file. Returns the gate
    # command's own exit status. No pipeline: the output is redirected to a
    # file and nothing else reads the status first.
    local dir="$1" check="$2" out="$3"
    local status
    #
    # 255 is reserved for "the command did not run at all", so that a `cd` that
    # failed can never be recorded as gate 1 reporting a failure. cargo exits 0,
    # 1 or 101 here and never 255.
    case "$check" in
        fmt)
            ( cd "$dir" || exit 255; "${FMT_ARGV[@]}" ) >"$out" 2>&1
            status=$?
            ;;
        clippy)
            ( cd "$dir" || exit 255; "${CLIPPY_ARGV[@]}" ) >"$out" 2>&1
            status=$?
            ;;
        *)
            emit error "unknown check '$check' in the case table"
            return 255
            ;;
    esac
    return "$status"
}

rule 'Observation: gate 1 run against each input, once'
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    pkg="${CASE_PKG[$i]}"
    chk="${CASE_CHK[$i]}"
    log="$WORK_DIR/$pkg.$chk.log"
    case "$chk" in
        fmt)    shown="$FMT_CMD" ;;
        clippy) shown="$CLIPPY_CMD" ;;
        *)      shown="unknown" ;;
    esac

    say "--- $pkg, $chk"
    say "    command    : ( cd fixtures/planted/gate-1/$pkg && $shown )"
    run_gate "$HERE/$pkg" "$chk" "$log"
    status=$?
    if [ "$status" -eq 255 ]; then
        VERDICT_REACHED=1
        emit error "the gate command for '$pkg, $chk' could not be started at all (status 255). That is not gate 1 reporting a failure, and this harness will not record it as one."
        exit 2
    fi
    if [ "$status" -eq 0 ]; then verdict='pass'; else verdict='fail'; fi
    strip_ansi "$log" "$log.plain"
    OBS_STATUS[$i]="$status"
    OBS_VERDICT[$i]="$verdict"
    OBS_LOG[$i]="$log.plain"
    say "    exit status: $status"
    say "    verdict    : $verdict"
    say "    output     : (colour codes removed; this is the text the signatures are matched against)"
    if [ -s "$log.plain" ]; then
        sed 's/^/      | /' "$log.plain"
    else
        say "      | (no output)"
    fi
    say ''
    i=$((i + 1))
done

# Every case must have been observed. An array left short would make the
# judgement below read an unset element under `set -u` and abort, which the
# floor would turn into exit 2, but saying it here names the cause.
if [ "${#OBS_VERDICT[@]}" -ne "$CASE_COUNT" ]; then
    VERDICT_REACHED=1
    emit error "observed ${#OBS_VERDICT[@]} of $CASE_COUNT cases, so some gate command did not run and this harness will not judge a partial run"
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 6: attribution. A non-zero exit is not evidence on its own. Where the
# gate answered what this table owes, the output must show that it answered it
# for the reason this row planted.
#
# The rows where the gate answered something else are left to the comparator
# below: those are the rows where gate 1 stopped catching a defect, and that is
# a gate failure (exit 1), not an unreadable observation (exit 2).
# ---------------------------------------------------------------------------

STAGE='checking that each failure is the failure this proof planted'
rule 'Attribution: the answer gate 1 gave is the answer to the planted question'
printf '  %-15s %-7s %-9s %s\n' 'input' 'half' 'observed' 'attributable to the planted mechanism'
printf '  %-15s %-7s %-9s %s\n' '---------------' '-------' '---------' '-------------------------------------'
attribution_bad=0
attribution_broken=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${OBS_VERDICT[$i]}" != "${CASE_EXP[$i]}" ]; then
        printf '  %-15s %-7s %-9s %s\n' \
            "${CASE_PKG[$i]}" "${CASE_CHK[$i]}" "${OBS_VERDICT[$i]}" \
            "not asked: the gate did not answer '${CASE_EXP[$i]}' here, which the table below reports"
        i=$((i + 1))
        continue
    fi
    output_matches "${OBS_LOG[$i]}" "${CASE_SIG[$i]}"
    status=$?
    case "$status" in
        0) printf '  %-15s %-7s %-9s %s\n' "${CASE_PKG[$i]}" "${CASE_CHK[$i]}" "${OBS_VERDICT[$i]}" 'yes' ;;
        1)
            printf '  %-15s %-7s %-9s %s\n' "${CASE_PKG[$i]}" "${CASE_CHK[$i]}" "${OBS_VERDICT[$i]}" "NO: $ATTRIB_WHY"
            attribution_bad=$((attribution_bad + 1))
            ;;
        *)
            printf '  %-15s %-7s %-9s %s\n' "${CASE_PKG[$i]}" "${CASE_CHK[$i]}" "${OBS_VERDICT[$i]}" "HARNESS: $ATTRIB_WHY"
            attribution_broken=$((attribution_broken + 1))
            ;;
    esac
    i=$((i + 1))
done
say ''
say "  unattributable: $attribution_bad of $CASE_COUNT (this must be 0)"

if [ "$attribution_broken" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "$attribution_broken of the signatures in this harness's own case table are not usable regular expressions, so those rows were never really checked. Nothing was proven."
    exit 2
fi
if [ "$attribution_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "$attribution_bad case(s) gave gate 1's owed answer for a reason this proof did not plant. A command that exits non-zero because a fixture no longer parses, or because a manifest is broken, is not gate 1 catching a formatting defect or a lint: it is the gate reporting on a question nobody asked. Read the attribution table above, repair the fixture so the planted defect is the only thing wrong with it, and re-run. Nothing was proven about gate 1 by this run."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stages 7 and 8: judgement. One comparator, two expectation sets.
# ---------------------------------------------------------------------------

MISMATCH_COUNT=0
JUDGE_AGREE=()

judge() {
    # $1 is 'direct' (the real expectations) or 'inverted' (every expectation
    # flipped). This is the only place a verdict is compared to an expectation,
    # so both passes exercise the same code.
    local mode="$1"
    local i expected observed status agreement
    MISMATCH_COUNT=0
    JUDGE_AGREE=()
    printf '  %-15s %-7s %-9s %-9s %-6s %s\n' 'input' 'half' 'expected' 'observed' 'exit' 'result'
    printf '  %-15s %-7s %-9s %-9s %-6s %s\n' '---------------' '-------' '---------' '---------' '------' '--------'
    i=0
    while [ "$i" -lt "$CASE_COUNT" ]; do
        expected="${CASE_EXP[$i]}"
        if [ "$mode" = 'inverted' ]; then
            if [ "$expected" = 'pass' ]; then expected='fail'; else expected='pass'; fi
        fi
        observed="${OBS_VERDICT[$i]}"
        status="${OBS_STATUS[$i]}"
        if [ "$observed" = "$expected" ]; then
            agreement='agree'
            JUDGE_AGREE[$i]=1
        else
            agreement='DISAGREE'
            JUDGE_AGREE[$i]=0
            MISMATCH_COUNT=$((MISMATCH_COUNT + 1))
        fi
        printf '  %-15s %-7s %-9s %-9s %-6s %s\n' \
            "${CASE_PKG[$i]}" "${CASE_CHK[$i]}" "$expected" "$observed" "$status" "$agreement"
        i=$((i + 1))
    done
}

STAGE='judging the observations against the real expectations'
rule 'The proof: what gate 1 owes each input'
judge 'direct'
direct_mismatches="$MISMATCH_COUNT"
DIRECT_AGREE=()
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    DIRECT_AGREE[$i]="${JUDGE_AGREE[$i]}"
    i=$((i + 1))
done
say ''
say "  disagreements: $direct_mismatches of $CASE_COUNT (this must be 0)"

STAGE='judging the observations against flipped expectations'
rule 'Harness self-check: the same comparator, every expectation flipped'
say 'Every row below MUST reach the opposite conclusion from the same row above.'
say 'This pass presents the comparator with a planted defect of its own:'
say 'expectations that contradict the ones just used, over identical'
say 'observations. A comparator that answers the same way in both passes is not'
say 'comparing anything, and the proof above would be a formality with no'
say 'content. While the gate is healthy every row above agrees, so every row'
say 'below reads DISAGREE; that is a consequence, not the check.'
say ''
judge 'inverted'
inverted_mismatches="$MISMATCH_COUNT"
INVERTED_AGREE=()
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    INVERTED_AGREE[$i]="${JUDGE_AGREE[$i]}"
    i=$((i + 1))
done
say ''
say "  disagreements: $inverted_mismatches of $CASE_COUNT"

# ---------------------------------------------------------------------------
# Stage 9: the verdict. The only exit 0 in this file.
# ---------------------------------------------------------------------------

STAGE='reaching a verdict'
rule 'Verdict'

harness_broken=0
gate_broken=0

# Per-row complementarity: for every case, exactly one of the two passes may
# agree. This is what "the comparator is comparing" means, and unlike a count
# it stays true when the gate itself is broken.
non_complementary=''
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${DIRECT_AGREE[$i]}" = "${INVERTED_AGREE[$i]}" ]; then
        non_complementary="$non_complementary ${CASE_PKG[$i]}/${CASE_CHK[$i]}"
    fi
    i=$((i + 1))
done
say "  complementarity: direct $direct_mismatches disagreement(s), flipped $inverted_mismatches; the two must sum to $CASE_COUNT and differ on every row"
if [ -n "$non_complementary" ]; then
    harness_broken=1
    emit error "this harness cannot tell agreement from disagreement. These case(s) reached the SAME conclusion under the real expectations and under their exact opposites:$non_complementary. A comparator that answers the same way for an expectation and its negation is not comparing anything, so the proof above means nothing, whatever it says."
fi

if [ "$direct_mismatches" -eq "$CASE_COUNT" ] && [ "$CASE_COUNT" -gt 1 ]; then
    # Every single row wrong is a pattern no constant gate can produce. A gate
    # command replaced by `true` reports pass everywhere, which the rows that
    # expect a pass still agree with; one replaced by `false` reports fail
    # everywhere, which the rows that expect a failure still agree with. Only
    # an inverted comparison, here or in the gate, gets every row wrong. Say
    # so, because the message below would otherwise send a reader to look at a
    # fixture that is fine.
    emit error "every one of the $CASE_COUNT cases disagreed, which no gate that always passes or always fails can produce: those still agree with part of the table. Look for an inverted comparison first, in this harness or in the gate command itself, before looking at the planted inputs."
fi

if [ "$direct_mismatches" -ne 0 ]; then
    gate_broken=1
    emit error "gate 1 did not behave the way ops/gates/gate-1.md claims: $direct_mismatches of $CASE_COUNT cases disagreed. Read the table above. A row expecting 'fail' that observed 'pass' means the gate stopped catching a defect that is still planted in the tree, so gate 1 is no longer proven (AICD §14) and no document may cite it until it is re-proved."
fi

VERDICT_REACHED=1

if [ "$harness_broken" -ne 0 ]; then
    exit 2
fi
if [ "$gate_broken" -ne 0 ]; then
    exit 1
fi

say "ESTABLISHED BY THIS RUN:"
say "  - gate 1's two commands are run by jobs of $WORKFLOW_REL that an unfiltered"
say "    '$REQUIRED_TRIGGER' fires, with no 'if:' and no 'continue-on-error:' on the job"
say "    or on the step, and '$AGGREGATE_JOB' waits on each of those jobs and carries no"
say "    'continue-on-error:' itself, so their failure fails this run;"
say "  - the '$SELF_JOB' job that runs this script is live by those same six"
say "    conditions, so the refusals above are refusals that fail this run too;"
say "  - gate 1 passed all $expect_pass inputs it owes a pass and failed all $expect_fail inputs it owes a failure;"
say "  - each of those failures carried the signature of the defect planted for it;"
say "  - and this harness was shown, on this run, able to tell those answers apart: its"
say "    comparator, its liveness check, its self-identification check and its"
say "    attribution check were each run over planted inputs whose owed answers differ."
say ''
say "NOT ESTABLISHED BY THIS RUN, AND NOT ESTABLISHABLE BY ANY RUN OF THIS SCRIPT:"
say "  - that a failing '$AGGREGATE_JOB' blocks a merge. That is branch protection and"
say "    not this file. '$AGGREGATE_JOB' is not a required status check on 'main':"
say "    ops/gates/branch-protection.md records that 'main' has no required checks at"
say "    all, that this is deliberate, and that each check name is added on the day its"
say "    proof file lands. Until '$AGGREGATE_JOB' is required, a red run here colours the"
say "    pull request and stops nothing;"
say "  - that the failure is visible where a human would look. This script cannot watch"
say "    the pull request check it writes to;"
say "  - that the job running this script will still be live on the next commit. A"
say "    'continue-on-error:' added to '$SELF_JOB' later makes this script refuse, and a"
say "    job carrying that key does not fail the run when it refuses. The refusal is"
say "    then an annotation and a human reading the diff of $WORKFLOW_REL is the last"
say "    link. That path is tier 2 for this reason."
say ''
say "Gate 1 has been seen to fail on a planted defect, on this commit. That is the"
say "demonstration AICD §14 asks for before a gate enters service, and it is two"
say "thirds of the rule; the third is the visibility clause above. ops/gates/gate-1.md"
say "records what is still owed before gate 1 may be called installed."
exit 0
