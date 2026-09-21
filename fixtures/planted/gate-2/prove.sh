#!/usr/bin/env bash
#
# prove.sh: the proof harness for gate 2 of spec/CI_CD.md section 1.
#
# Ticket: ORI-T-0014.  Spec: spec/runbooks/prove-gate.md, spec/TESTING.md
# sections 1 and 4, spec/CI_CD.md section 1 gate 2.  Proof: ops/gates/gate-2.md.
# Criterion served: ORI-P1-013.
#
# ===========================================================================
# WHAT THIS SCRIPT IS FOR, AND HOW GATE 2 DIFFERS FROM GATE 1
# ===========================================================================
#
# AICD §14: "a gate is installed only when it has been seen to fail". A gate
# that has only ever been observed passing is not installed, because the
# recurring defect class the rule exists for is "present but reporting
# nothing": "a checker that exits successfully on every input, a workflow file
# whose trigger never fires, a test stage that a deploy path bypasses".
#
# Gate 1 checks a property of the code: the tree is formatted, the tree is
# lint clean. Gate 2 checks a property of the TESTS, and that is a different
# kind of thing, because a test suite can be green and prove nothing. This is
# not a hypothesis about the gate. It is a property of `cargo test` itself:
#
#   - it exits 0 on a package with no tests at all, printing "running 0 tests"
#     and "test result: ok";
#   - it exits 0 on tests that assert nothing;
#   - it exits 0 when every test carries `#[ignore]`, printing "0 passed;
#     0 failed; N ignored" and "test result: ok";
#   - `--workspace` is as wide as the workspace's `members` list, and a package
#     left out of that list is a package whose failing tests never run.
#
# In all four the gate reports success and the thing it is meant to protect is
# absent. So this harness plants both directions and RECORDS BOTH. It does not
# invent a check that would make gate 2 catch the last four; gate 2 is
# `cargo test --workspace --locked` and nothing else, and what the four inputs
# below establish is what gate 2 does not establish. ops/gates/gate-2.md says
# so in as many words, and the verdict at the bottom of this script says it on
# every run, so that a reader of the CI log is told as plainly as a reader of
# the proof file.
#
# A one-time record that a human once saw gate 2 fail satisfies the letter of
# AICD §14 and expires the moment somebody edits the gate. This script is the
# stronger form: it re-presents the planted inputs on every pull request, and
# it fails the run when the gate stops behaving the way the proof file claims.
#
# ===========================================================================
# THE FIVE QUESTIONS, AND WHY EACH NEEDS ITS OWN ANSWER
# ===========================================================================
#
# ORI-T-0013 found four defects in the equivalent harness for gate 1, each one
# a code path reporting success for a state that is not success, and not one of
# them was found by reading. Its four questions are asked here, in its shape,
# and a fifth is added that gate 1 did not need.
#
#   1. IS THIS A COMMAND CI RUNS?  Stage 4. An anchored grep for the command
#      string in `.github/workflows/ci.yml` cannot tell a command CI runs from
#      a command CI does not: the line can belong to a job that is not in the
#      required aggregate's `needs:`, or to a job or step carrying
#      `continue-on-error:` or an `if:`, or it can be text inside another
#      step's shell script, or the workflow's triggers can mean nothing fires
#      on a pull request at all. Every one leaves the line byte-identical. The
#      file is parsed (`workflow-facts.awk`) and the question asked of the
#      structure.
#
#   2. DOES GATE 2 GIVE EACH INPUT THE VERDICT IT OWES?  Stages 7 and 8. Eight
#      planted packages, one gate command, eight cases, judged by one
#      comparator run twice over one set of observations.
#
#   3. IS THE ANSWER THE ANSWER TO THE PLANTED QUESTION?  Stage 6. A non-zero
#      exit is not evidence on its own: a package that no longer compiles, a
#      `Cargo.lock` that `--locked` rejects, or a missing manifest all produce
#      one, and none of them is a failing test. Each case names the signatures
#      its planted mechanism leaves in the output, including signatures that
#      must be ABSENT, and an answer without them proves nothing.
#
#   4. IS THE JOB THAT ASKS THE FIRST THREE ITSELF LIVE?  Stage 4b. Question 1
#      asked about the `test` job and not about `gate-2-proof`, the job that
#      runs this script. `continue-on-error: true` on that one line turns every
#      refusal below into a green check: the harness still runs, still refuses,
#      still writes its annotation, and GitHub records the job as a success.
#      The same six structural conditions are asked of this script's own job,
#      which it finds in the file by the command that invokes it AND by name,
#      with both required to agree.
#
#   5. IS THE PLANT STILL PLANTED?  Stage 3b, and gate 1 did not need it.
#      Gate 1's planted defects announce themselves in the gate's own output:
#      a formatting diff, a named lint. Four of gate 2's eight inputs are
#      planted to be passed, and a pass looks the same whether the defect is
#      there or not. "cargo test exits 0 on a package with no tests" is a claim
#      about a package that has no tests; repair the package and the row still
#      agrees, and this harness would report that gate 2 behaves as documented
#      while demonstrating nothing. So the plants are asserted against the
#      fixture sources directly, and a missing plant is a refusal.
#
# ===========================================================================
# THE INVERSION, WHICH IS THE DANGEROUS PART
# ===========================================================================
#
# This job's success condition is the opposite of every other job in the
# pipeline: three of its eight cases pass when the gate command fails. Getting
# that backwards produces a job that passes on every input, which is the exact
# defect AICD §14 names. Four things guard it, and none of them is a comment.
#
#   1. The case table is not all failures and not all passes. Five inputs gate
#      2 must pass, three it must fail. One comparator judges all eight, so a
#      harness that reported failure for everything would disagree with five
#      rows and this run would go red.
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
#      The check is per row and it is not a count. ops/gates/gate-1.md records
#      why: a count is only complementary while the direct pass agrees on every
#      row, so a real fixture defect made gate 1's harness report ITSELF broken.
#      Per-row complementarity is the property actually wanted and it holds
#      whatever the gate does.
#
#   3. The liveness check of stage 4 is a checker like any other, so it gets
#      the same treatment before it is trusted: stage 3c runs it over the
#      planted workflows in `workflow-samples/`, one of which is live, ten of
#      which are dead in ten different ways and one of which cannot be read. A
#      liveness check that answered "live" to everything, or "dead" to
#      everything, disagrees with that table and this run reports the harness
#      broken. The self-identification check of stage 3d and the attribution
#      check of stage 3e are exercised the same way, each against inputs whose
#      owed answers differ.
#
#   4. The plants are asserted, not assumed (question 5 above), and the reader
#      those assertions are made with gets the same treatment as every other
#      checker here: stage 3b runs it first over a file whose only assertion is
#      in a comment, which it must not see, and a file whose assertion is
#      written across four lines, which it must. Both shapes are defects this
#      harness has had.
#
# Separating observation from judgement is what makes those passes honest: the
# eight gate commands run once, their exit statuses and their output are
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
# `set -e` is deliberately NOT used. Three of the eight commands here are
# expected to fail, and an errexit shell that leaves before the verdict is a
# gate that exits 0 without checking anything. The EXIT trap below turns any
# such early departure into exit 2.
#
# ===========================================================================
# EXIT STATUS
# ===========================================================================
#
#   0  the proof holds: gate 2 is run by a job whose failure fails the run, the
#      job that runs this script is live by those same six conditions, every
#      planted input is still planted, gate 2 failed each of the three inputs
#      carrying a failing test, it passed each of the five it passes (one of
#      them correct and four of them not), each answer carried the signature of
#      the mechanism planted for it, and this harness was shown able to tell
#      those answers apart. What that does and does not establish about the
#      test suite is printed with the verdict, because it is the part a reader
#      is most likely to get wrong.
#   1  gate 2 did not behave the way ops/gates/gate-2.md claims. Either the
#      gate has been weakened or a planted input no longer produces what it was
#      planted to produce. Gate 2 may not be cited until this is 0 again
#      (spec/runbooks/prove-gate.md, "Rollback: revoke the proof").
#   2  this harness could not prove what it claims: it could not tell agreement
#      from disagreement, or EVERY case disagreed, which means a comparison is
#      inverted and does not say whether the inversion is in this harness or in
#      the gate, or the workflow no longer runs the command under proof where
#      its failure would fail the run, or it no longer runs this script there,
#      or this script could not find itself in the workflow at all, or a
#      planted input is no longer planted, or it could not read a fixture's
#      code apart from that fixture's prose, or a gate command answered for a
#      reason that is not the planted one. Nothing was proven either way.
#   3  a prerequisite is missing, so no gate command ran at all. Nothing was
#      checked.
#
# Run it from anywhere: `bash fixtures/planted/gate-2/prove.sh`.

set -uo pipefail

# ---------------------------------------------------------------------------
# Location
# ---------------------------------------------------------------------------

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
WORKFLOW_REL='.github/workflows/ci.yml'
AWK_PARSER="$HERE/workflow-facts.awk"
SAMPLE_DIR="$HERE/workflow-samples"

# The command under proof, written once here and compared against the workflow
# file below so that this script cannot drift from the gate it claims to be
# proving. spec/CI_CD.md section 1 item 2 is "cargo test (unit, property,
# integration), workspace-wide"; this is the command `.github/workflows/ci.yml`
# runs for it.
TEST_CMD='cargo test --workspace --locked'

# The job that is to be the one required check on `main`, and the event that
# must fire the workflow. A gate command whose job is not gated by the first,
# or whose workflow does not answer the second, runs without failing anything.
#
# `ci` is NOT a required status check today. ops/gates/branch-protection.md
# records that as deliberate ("a required check that never reports leaves every
# pull request pending indefinitely") and records when it changes: "each check
# name is added on the day its proof file lands". So the strongest true
# statement this script can make about the aggregate is that a failure reaches
# it and fails the run. Whether that red run stops a merge is a repository
# setting nothing in this file can read, and the verdict at the bottom says so
# instead of implying otherwise.
AGGREGATE_JOB='ci'
REQUIRED_TRIGGER='pull_request'

# This harness's own job, and the command that runs it. Everything above is a
# question this script asks about another job. This is the one it asks about
# itself.
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
SELF_JOB='gate-2-proof'
SELF_FILE="$HERE/$(basename -- "${BASH_SOURCE[0]}")"
case "$SELF_FILE" in
    "$REPO_ROOT"/*) SELF_REL="${SELF_FILE#"$REPO_ROOT"/}" ;;
    *)              SELF_REL='' ;;
esac
SELF_CMD="bash $SELF_REL"

# The command as an argument vector, split once here rather than by leaving an
# unquoted variable to the shell's word splitting and globbing. The command
# contains no glob character today; this does not depend on that staying true.
read -r -a TEST_ARGV <<<"$TEST_CMD"

# The planted implementation, and the correct one it was made from. Stage 3b
# asserts these against the fixture sources: four of the eight inputs below are
# planted to be PASSED by gate 2, and a pass looks identical whether the defect
# is still there or not.
DEFECT_LINE='pub fn remaining_budget(_spent: u32, limit: u32) -> u32 {'
CORRECT_EXPR='limit.saturating_sub(spent)'

# ---------------------------------------------------------------------------
# The case table: one row per planted package, with the verdict gate 2 owes it,
# the class of thing that row is about, and the signatures its planted
# mechanism leaves in the output.
#
# THE CLASS COLUMN IS NOT DECORATION. It is what lets the verdict separate "the
# gate caught this" from "the gate reported success on this", which for gate 2
# are both correct answers and mean opposite things. `catches` rows are the
# demonstration AICD §14 asks for. `blind` rows are the four states gate 2
# reports as success, and they are why ops/gates/gate-2.md exists in the shape
# it does.
#
# A signature beginning with `!` must NOT appear. Gate 1's harness had only
# positive signatures, which is enough when every planted defect announces
# itself in the output. Three of the four `blind` rows here are about something
# the output does not say: `unlisted-member` is attributable only by cargo
# never mentioning the package at all, and `no-tests` and `ignored-tests` only
# by no test binary reporting a test that ran.
# ---------------------------------------------------------------------------

CASE_PKG=()
CASE_EXP=()
CASE_CLASS=()
CASE_SIG=()

add_case() {
    # $1 package, $2 expected verdict, $3 class, $4.. the signatures.
    CASE_PKG+=("$1")
    CASE_EXP+=("$2")
    CASE_CLASS+=("$3")
    shift 3
    local sig='' pat
    for pat in "$@"; do
        sig="$sig$pat"$'\n'
    done
    CASE_SIG+=("$sig")
}

#         package               owed   class
add_case  clean                 pass   control \
    '^running 2 tests$' \
    '^test tests::remaining_budget_subtracts_the_spend \.\.\. ok$' \
    '^test an_exhausted_budget_is_zero \.\.\. ok$' \
    '^test src/lib\.rs - remaining_budget \(line [0-9]+\) \.\.\. ok$' \
    '^test result: ok\. 2 passed; 0 failed; 0 ignored;' \
    '^test result: ok\. 1 passed; 0 failed; 0 ignored;' \
    '!^test result: FAILED'

add_case  failing-unit          fail   catches \
    '^test tests::remaining_budget_subtracts_the_spend \.\.\. FAILED$' \
    '^assertion .left == right. failed$' \
    '^ *left: 10$' \
    '^ *right: 7$' \
    '^test result: FAILED\. 0 passed; 1 failed; 0 ignored;' \
    '^error: test failed, to rerun pass .--lib.$' \
    '!^error\[E[0-9]' \
    '!could not compile'

add_case  failing-integration   fail   catches \
    '^test an_exhausted_budget_is_zero \.\.\. FAILED$' \
    'panicked at tests/budget\.rs:' \
    '^assertion .left == right. failed$' \
    '^test result: FAILED\. 0 passed; 1 failed; 0 ignored;' \
    '^error: test failed, to rerun pass .--test budget.$' \
    '!^error\[E[0-9]' \
    '!could not compile'

add_case  failing-doctest       fail   catches \
    '^test src/lib\.rs - remaining_budget \(line [0-9]+\) \.\.\. FAILED$' \
    '^assertion .left == right. failed$' \
    '^test result: FAILED\. 0 passed; 1 failed; 0 ignored;' \
    '^error: doctest failed, to rerun pass .--doc.$' \
    '!^error\[E[0-9]' \
    '!could not compile'

add_case  no-tests              pass   blind \
    '^running 0 tests$' \
    '^test result: ok\. 0 passed; 0 failed; 0 ignored;' \
    '!^test result: FAILED' \
    '!^running [1-9]' \
    '!^test result: ok\. [1-9]'

add_case  vacuous-tests         pass   blind \
    '^running 3 tests$' \
    '^test tests::remaining_budget_runs \.\.\. ok$' \
    '^test result: ok\. 3 passed; 0 failed; 0 ignored;' \
    '!^test result: FAILED'

add_case  ignored-tests         pass   blind \
    '^running 3 tests$' \
    '\.\.\. ignored' \
    '^test result: ok\. 0 passed; 0 failed; 3 ignored;' \
    '!^test result: FAILED' \
    '!^test result: ok\. [1-9]'

# The signature here is almost all negation, and the positive half deliberately
# says nothing about compilation. The obvious signature, `Compiling
# gate-2-listed`, was wrong: cargo prints it on the first run and not on a run
# where `target/` is already warm, so the attribution passed on a fresh
# checkout and refused on the second local run. A signature that answers
# differently depending on a build cache is not a signature of the planted
# mechanism. What IS the planted mechanism is that cargo never mentions
# `gate-2-unlisted` at all, in any state, and that the only test result in the
# output is the member's one passing test.
add_case  unlisted-member       pass   blind \
    '^test tests::remaining_budget_subtracts_the_spend \.\.\. ok$' \
    '^test result: ok\. 1 passed; 0 failed; 0 ignored;' \
    '!gate-2-unlisted' \
    '!^test result: FAILED' \
    '!^test result: ok\. [2-9]'

CASE_COUNT=${#CASE_PKG[@]}

OBS_VERDICT=()
OBS_STATUS=()
OBS_LOG=()
OBS_PASSED=()
OBS_FAILED=()
OBS_IGNORED=()
OBS_BINARIES=()

# ---------------------------------------------------------------------------
# The sample table: the planted defects for the liveness check of stage 4.
# Each file under workflow-samples/ is a whole workflow carrying exactly one
# way of switching gate 2 off, or none. The classification this harness owes
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
# rather than about the job it proves. Finding this script in the file is the
# part of that with more ways to go wrong than to go right, so every answer the
# check can give has an input here that is owed it.
#
# Two of the twelve workflows above serve here unchanged, because the answer
# they are owed about gate 2's command is the answer they are owed about this
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
        printf '::%s file=fixtures/planted/gate-2/prove.sh::%s\n' "$kind" "$*"
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
# The checks this proof adds, as functions, so that stage 3 can run them
# against planted inputs and stages 4, 4b and 6 can use them against the real
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
            trigger_why="      - the '$REQUIRED_TRIGGER' trigger carries the filter key(s) ${keys% }, which narrow which pull requests fire this workflow. That narrowing is a deliberate change to when gate 2 runs, so it needs a deliberate re-proof rather than a harness deciding which pull requests are allowed to skip the gate"
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
#               conditions classify_command applies to the job it proves
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

# Remove the colour codes a tool writes for a terminal. `cargo` colours its own
# progress lines whether or not it is writing to one, and the workflow sets
# CARGO_TERM_COLOR=always on top of that, so `^error: doctest failed` matches
# nothing on the raw bytes: the line really begins with an escape sequence. The
# signatures above are written against what a human reads, and this is what
# makes the two the same text. The gate command's exit status is captured by
# the caller before this runs and cannot be swallowed by it.
strip_ansi() {
    # $1 the captured output, $2 the file to write the readable form to.
    local esc
    esc="$(printf '\033')"
    sed "s/${esc}\[[0-9;]*m//g; s/${esc}(B//g" "$1" >"$2"
}

# Count what actually ran, from the `test result:` lines cargo's test harness
# prints, one per test binary. Sets TESTS_PASSED, TESTS_FAILED, TESTS_IGNORED
# and TESTS_BINARIES.
#
# THIS IS AN OBSERVATION AND NOT A CHECK. No verdict in this harness depends on
# it. It is here because "gate 2 exited 0" and "gate 2 exited 0 having run no
# test at all" are the same line in a CI log, and the whole point of this
# fixture is that they are not the same fact. The counts are printed beside
# each verdict and gathered in the summary so that ops/gates/gate-2.md can
# state what a green gate 2 establishes without anybody having to reason it
# out. Turning any of this into a threshold would be inventing gate 4 or gate 6
# inside gate 2's proof, which ORI-T-0014 was told not to do.
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_IGNORED=0
TESTS_BINARIES=0
count_tests() {
    # $1 the readable output file.
    local counts
    counts="$(awk '
        /^test result:/ {
            for (i = 2; i <= NF; i++) {
                if ($i == "passed;")  { p += $(i - 1) }
                if ($i == "failed;")  { f += $(i - 1) }
                if ($i == "ignored;") { g += $(i - 1) }
            }
            b = b + 1
        }
        END { printf "%d %d %d %d\n", p + 0, f + 0, g + 0, b + 0 }
    ' "$1")"
    # Safe to split: awk prints exactly four integers separated by spaces.
    set -- $counts
    TESTS_PASSED="$1"
    TESTS_FAILED="$2"
    TESTS_IGNORED="$3"
    TESTS_BINARIES="$4"
}

# Is what the gate command printed the answer this case planted?
#   0 every signature is satisfied
#   1 a signature is not: this is not the answer this proof planted
#   2 a signature is not a usable regular expression
#
# A signature beginning with `!` must NOT match. The rest of the line is an
# extended regular expression either way. Stage 3e exercises all three return
# values and both kinds of signature before any of this is believed.
ATTRIB_WHY=''
output_matches() {
    # $1 file holding the captured output, $2 newline-separated signatures.
    local log="$1" sigs="$2"
    local pat body negated status
    ATTRIB_WHY=''
    while IFS= read -r pat; do
        [ -n "$pat" ] || continue
        case "$pat" in
            '!'*) negated=1; body="${pat#!}" ;;
            *)    negated=0; body="$pat" ;;
        esac
        grep -Eq -e "$body" "$log" 2>/dev/null
        status=$?
        if [ "$status" -gt 1 ]; then
            ATTRIB_WHY="/$body/ is not a usable extended regular expression: grep exited $status"
            return 2
        fi
        if [ "$negated" -eq 0 ] && [ "$status" -ne 0 ]; then
            ATTRIB_WHY="nothing in the output matches /$body/"
            return 1
        fi
        if [ "$negated" -eq 1 ] && [ "$status" -eq 0 ]; then
            ATTRIB_WHY="the output matches /$body/, which this case requires it not to"
            return 1
        fi
    done <<<"$sigs"
    return 0
}

# ---------------------------------------------------------------------------
# Stage 1: what is running this
# ---------------------------------------------------------------------------

STAGE='reporting the environment'
rule 'Environment'
say "gate 2 proof harness, ORI-T-0014, spec/CI_CD.md section 1 gate 2"
say "  repository root : $REPO_ROOT"
say "  fixture root    : $HERE"
say "  platform        : $(uname -s 2>/dev/null || echo unknown) $(uname -m 2>/dev/null || echo unknown)"
if [ -n "${GITHUB_ACTIONS:-}" ]; then
    say "  running under   : GitHub Actions, runner OS ${RUNNER_OS:-unknown}, event ${GITHUB_EVENT_NAME:-unknown}"
else
    say "  running under   : a local shell"
fi

# ---------------------------------------------------------------------------
# Stage 2: the toolchain. A missing component makes gate 2 unrunnable, and a
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
        emit error "'$*' exited $status, so $label is not usable here and no verdict of gate 2 on this machine would mean anything"
        probe_failed=1
        return
    fi
    printf '  %-16s %s\n' "$label" "${out%%$'\n'*}"
}
probe 'cargo'  cargo --version
probe 'rustc'  rustc --version
# awk reads the workflow file. Probed by running a program rather than by
# asking for a version banner, because the three awks this has to run under
# disagree about which version flag they take and agree about this.
probe 'awk'    awk 'BEGIN { print "a usable POSIX awk" }'
if [ "$probe_failed" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "nothing was checked: the toolchain this proof needs is not usable on this machine. This is not a pass and not a failure of gate 2."
    exit 3
fi

WORK_DIR="$(mktemp -d)"
if [ ! -d "$WORK_DIR" ]; then
    VERDICT_REACHED=1
    emit error "could not create a temporary directory to hold the gate output, so nothing was run"
    exit 3
fi

# ---------------------------------------------------------------------------
# Stage 3: assertions about the fixture and about this harness's own tables.
# Every one of these is a way for this proof to be quietly meaningless, so each
# is checked rather than assumed.
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
# that passes on everything, which is the AICD §14 defect class itself. Both
# classes must be represented too: without a `catches` row there is no
# demonstration, and without a `blind` row this fixture would say nothing about
# the part of gate 2 that matters most.
expect_pass=0
expect_fail=0
class_catches=0
class_blind=0
class_control=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    case "${CASE_EXP[$i]}" in
        pass) expect_pass=$((expect_pass + 1)) ;;
        fail) expect_fail=$((expect_fail + 1)) ;;
        *)    note_bad "case $i expects '${CASE_EXP[$i]}', which is neither 'pass' nor 'fail'" ;;
    esac
    case "${CASE_CLASS[$i]}" in
        catches) class_catches=$((class_catches + 1)) ;;
        blind)   class_blind=$((class_blind + 1)) ;;
        control) class_control=$((class_control + 1)) ;;
        *)       note_bad "case $i is of class '${CASE_CLASS[$i]}', which is none of catches, blind, control" ;;
    esac
    # A row that names no signature is a row that would accept any answer at
    # all, which is the hole stage 6 exists to close.
    if [ -z "${CASE_SIG[$i]//$'\n'/}" ]; then
        note_bad "case $i (${CASE_PKG[$i]}) names no signature, so any output whatever would be recorded as the planted mechanism having produced it"
    fi
    i=$((i + 1))
done
say "  case table       : $CASE_COUNT cases, $expect_pass expected to pass, $expect_fail expected to fail"
say "  by class         : $class_catches gate 2 catches, $class_blind gate 2 reports as success, $class_control control"
if [ "$expect_pass" -eq 0 ]; then
    note_bad "no case expects gate 2 to pass, so this run could not tell a working gate from one that fails on every input"
fi
if [ "$expect_fail" -eq 0 ]; then
    note_bad "no case expects gate 2 to fail, so this run could not tell a working gate from one that passes on every input, which is the defect class of AICD §14"
fi
if [ "$class_catches" -eq 0 ]; then
    note_bad "no case is of class 'catches', so this run contains no demonstration that gate 2 catches anything and AICD §14 is not satisfied by it"
fi
if [ "$class_blind" -eq 0 ]; then
    note_bad "no case is of class 'blind', so this fixture no longer records what gate 2 reports as success, which is the half of this ticket ops/gates/gate-2.md is written from"
fi

# The planted packages must be present, must be cargo packages, and must each
# be their own workspace root. Without the `[workspace]` table a planted
# package would be resolved against the repository workspace and the real build
# would stop being green, which is the constraint this fixture layout exists to
# satisfy.
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    pkg="${CASE_PKG[$i]}"
    dir="$HERE/$pkg"
    for required in 'Cargo.toml' 'Cargo.lock'; do
        if [ ! -f "$dir/$required" ]; then
            note_bad "$pkg/$required is missing, so the input this proof claims to present to gate 2 is not there. Without Cargo.lock the gate command's own '--locked' fails, which is not gate 2 reporting on a test."
        fi
    done
    if [ -f "$dir/Cargo.toml" ] && ! grep -q '^\[workspace\]' "$dir/Cargo.toml"; then
        note_bad "$pkg/Cargo.toml has no [workspace] table, so this package is not its own workspace root and the repository workspace may resolve it"
    fi
    i=$((i + 1))
done

# The root workspace must not reach into the fixture. `cargo test --workspace`
# at the repository root resolves the explicit member list in the root
# Cargo.toml; a member under `fixtures/` would put a planted failing test into
# the real build.
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

# ---------------------------------------------------------------------------
# Stage 3b: the plants. Gate 1's harness did not need this stage, because every
# defect it plants announces itself in the gate's own output. Four of the eight
# inputs here are planted to be PASSED, and a pass looks identical whether the
# plant is there or not: repair `no-tests/src/lib.rs` by giving it a test and
# the row still agrees, while the thing this fixture exists to demonstrate has
# quietly gone. That is this gate's own defect class inside its proof, so the
# plants are asserted against the sources rather than inferred from a verdict.
# ---------------------------------------------------------------------------

STAGE='checking that every planted input is still planted'
rule 'The plants: every input still contains what it was planted to contain'

# EVERY ASSERTION BELOW IS MADE AGAINST THE CODE AND NOT AGAINST THE FILE, and
# that distinction was not a design decision, it was a defect this stage had on
# its first run. The fixtures under this directory explain themselves at
# length, so `no-tests/src/lib.rs` says in its module documentation that it
# carries no `#[cfg(test)]` module, and `ignored-tests/src/lib.rs` says that
# every one of its tests carries `#[ignore]`. A grep over the whole file finds
# those words in the prose: this stage reported that `no-tests` had acquired a
# test, and counted four `#[ignore]` against three `#[test]` in a file whose
# three tests are all ignored. Both refusals were correct about the bytes and
# wrong about the fixture, which is the same class of error as everything else
# this harness is arranged to catch: a check reporting on a state that is not
# the state it names.
#
# `code_of` strips line comments before the grep: `//` for Rust, `#` for TOML.
# It handles only line comments, so it refuses outright on a file containing
# `/*` rather than quietly reading a block comment as code. The two assertions
# that are genuinely about documentation, the presence of a `///` example in
# `failing-doctest` and its absence in `no-tests`, ask for mode `raw` and say
# so.

plant_bad=0
plant_report() {
    # $1 'ok' or the reason it is not, $2 what was being asserted.
    if [ "$1" = 'ok' ]; then
        printf '  %-54s %s\n' "$2" 'ok'
    else
        printf '  %-54s %s\n' "$2" 'GONE'
        emit error "a planted input is no longer planted: $2. $1 Nothing this harness could report about that row would mean anything, so it refuses."
        plant_bad=1
    fi
}

# Write the part of a fixture file that is code, not commentary, to $2.
# Returns 1 when the file is absent or carries a block comment, which this
# stripper does not read.
code_of() {
    # $1 file, relative to HERE or absolute, $2 file to write.
    local src="$1"
    case "$1" in
        /*) ;;
        *)  src="$HERE/$1" ;;
    esac
    if [ ! -f "$src" ]; then
        return 1
    fi
    if grep -qF '/*' "$src"; then
        return 1
    fi
    case "$1" in
        *.rs)   sed 's|//.*$||' "$src" >"$2" ;;
        *.toml) sed 's|#.*$||'  "$src" >"$2" ;;
        *)      cat "$src" >"$2" ;;
    esac
    return 0
}

# The same code with every newline turned into a space, so that a grep sees a
# construct written across several lines as the one thing it is.
#
# THIS EXISTS BECAUSE THE LINE-ORIENTED VERSION HAD A BLIND SPOT, and the blind
# spot was found by writing the input rather than by reading the code. The
# assertion this stage must not find in `vacuous-tests` is one that names what
# `remaining_budget` returned, and `grep` reads one line at a time, so
#
#     assert_eq!(
#         remaining_budget(3, 10),
#         7
#     );
#
# which is how rustfmt writes a long one, matched nothing. The input stopped
# being vacuous, the plant check reported it still planted, and the refusal
# that followed came from the comparator and blamed gate 2 for a fixture that
# had been repaired. Returns what code_of returns.
joined_code_of() {
    # $1 file, relative to HERE or absolute, $2 file to write.
    if ! code_of "$1" "$2.lines"; then
        return 1
    fi
    tr '\n' ' ' <"$2.lines" >"$2"
    return 0
}

# An assertion about what `remaining_budget` returned, in the joined code. It
# is one pattern, used by the plant assertion below and by the self-check that
# runs before it, so the two cannot drift apart.
ASSERT_RE='assert[a-z_]*! *\([^)]*remaining_budget'

PLANT_N=0
plant_subject() {
    # $1 mode (code, joined or raw), $2 file relative to HERE, $3 label.
    # Writes the text to grep to PLANT_TEXT. Returns 1 when there is none.
    PLANT_TEXT=''
    if [ ! -f "$HERE/$2" ]; then
        plant_report "$2 does not exist." "$3"
        return 1
    fi
    if [ "$1" = 'raw' ]; then
        PLANT_TEXT="$HERE/$2"
        return 0
    fi
    PLANT_N=$((PLANT_N + 1))
    PLANT_TEXT="$WORK_DIR/plant.$PLANT_N"
    if [ "$1" = 'joined' ]; then
        joined_code_of "$2" "$PLANT_TEXT" && return 0
    else
        code_of "$2" "$PLANT_TEXT" && return 0
    fi
    plant_report "$2 carries a block comment, which this harness's comment stripper does not read; it will not guess which of its bytes are code." "$3"
    return 1
}

plant_present() {
    # $1 mode, $2 file, $3 fixed string, $4 label, $5 why it matters.
    plant_subject "$1" "$2" "$4" || return
    if grep -qF -e "$3" "$PLANT_TEXT"; then
        plant_report ok "$4"
    else
        plant_report "$2 no longer contains \"$3\". $5" "$4"
    fi
}

plant_absent() {
    # $1 mode, $2 file, $3 extended regexp, $4 label, $5 why.
    plant_subject "$1" "$2" "$4" || return
    if grep -Eq -e "$3" "$PLANT_TEXT" 2>/dev/null; then
        plant_report "$2 now contains something matching /$3/. $5" "$4"
    else
        plant_report ok "$4"
    fi
}

# --- the self-check of the reader the assertions below are made with -------
#
# `code_of` and `joined_code_of` are checkers, so they get what every other
# checker in this file gets: inputs whose owed answers differ, run before
# anything they say is believed. Both of them have been wrong here, and neither
# defect was found by reading.
#
#   The first version read the whole file. `no-tests/src/lib.rs` documents, in
#   its own prose, that it carries no `#[cfg(test)]` module; the grep found
#   those words and reported that the fixture had been repaired.
#
#   The second read one line at a time. An assertion rustfmt had wrapped over
#   four lines was invisible to it, so a `vacuous-tests` that had stopped being
#   vacuous still reported "ok" and the refusal that followed came from the
#   comparator and blamed gate 2.
#
# One file below carries its only assertion in a comment and this reader must
# not see it; the other carries one spread over four lines and it must.
STAGE='checking that this harness reads the code of a fixture and not its prose'
say 'First the reader itself, on two files whose owed answers differ. Both'
say 'shapes below have been read wrongly in this harness before.'
say ''

pc_prose="$WORK_DIR/plantcheck-in-a-comment.rs"
cat >"$pc_prose" <<'PLANTCHECK'
//! This file's documentation quotes the very thing the reader looks for:
//! assert_eq!(remaining_budget(3, 10), 7);
//! It is prose. Nothing here asserts anything about the answer.
#[cfg(test)]
mod tests {
    #[test]
    fn remaining_budget_runs() {
        let _ = remaining_budget(3, 10);
        assert!(true);
    }
}
PLANTCHECK

pc_wrapped="$WORK_DIR/plantcheck-across-lines.rs"
cat >"$pc_wrapped" <<'PLANTCHECK'
#[cfg(test)]
mod tests {
    #[test]
    fn remaining_budget_subtracts_the_spend() {
        assert_eq!(
            remaining_budget(3, 10),
            7
        );
    }
}
PLANTCHECK

PC_FILE=("$pc_prose" "$pc_wrapped")
PC_OWED=(1 0)
PC_WHAT=('an assertion that is only in a comment' 'an assertion written across four lines')
PC_COUNT=${#PC_FILE[@]}
pc_bad=0
i=0
while [ "$i" -lt "$PC_COUNT" ]; do
    pc_joined="$WORK_DIR/plantcheck.joined.$i"
    if joined_code_of "${PC_FILE[$i]}" "$pc_joined"; then
        grep -Eq -e "$ASSERT_RE" "$pc_joined"
        pc_got=$?
    else
        pc_got=9
    fi
    if [ "$pc_got" -eq "${PC_OWED[$i]}" ]; then
        pc_outcome='agree'
    else
        pc_outcome='DISAGREE'
        pc_bad=$((pc_bad + 1))
    fi
    printf '  %-46s owed %s, answered %s  %s\n' \
        "${PC_WHAT[$i]}" "${PC_OWED[$i]}" "$pc_got" "$pc_outcome"
    i=$((i + 1))
done
say ''
say "  disagreements: $pc_bad of $PC_COUNT (this must be 0)"
if [ "$pc_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot tell a fixture's code from its prose, or cannot see an assertion written across more than one line: $pc_bad of the $PC_COUNT planted files were read wrongly. Every plant assertion below would be made with a reader that has just been shown not to work, so none of them means anything."
    exit 2
fi
say ''
say 'Now the fixtures.'
say ''

plant_present code clean/src/lib.rs "$CORRECT_EXPR" \
    'clean: the implementation is correct' \
    'The control input must be an input gate 2 has no complaint about, or the pass it produces says nothing.'
plant_present code clean/tests/budget.rs '#[test]' \
    'clean: an integration test exists' \
    'The control must exercise the same three target kinds the failing inputs do.'
plant_present raw clean/src/lib.rs '```' \
    'clean: a documentation example exists' \
    'The control must exercise the doc test level too, or the pass it produces says nothing about it.'

plant_present code failing-unit/src/lib.rs "$DEFECT_LINE" \
    'failing-unit: the defect is in the library' \
    'The failing test must fail because the library is wrong, not because the test is.'
plant_present code failing-unit/src/lib.rs '#[test]' \
    'failing-unit: a unit test exists' \
    'Without it this package has nothing that can fail.'

plant_present code failing-integration/src/lib.rs "$DEFECT_LINE" \
    'failing-integration: the defect is in the library' \
    'Same reason as failing-unit.'
plant_present code failing-integration/tests/budget.rs 'assert_eq!' \
    'failing-integration: the integration test asserts' \
    'A tests/ file that asserts nothing cannot fail, and this row expects a failure.'
plant_absent code failing-integration/src/lib.rs '#\[test\]' \
    'failing-integration: nothing else can fail' \
    'A unit test here would make the failure attributable to two mechanisms.'

plant_present code failing-doctest/src/lib.rs "$DEFECT_LINE" \
    'failing-doctest: the defect is in the library' \
    'Same reason as failing-unit.'
plant_present raw failing-doctest/src/lib.rs '```' \
    'failing-doctest: a documentation example exists' \
    'The doc test is the only thing in this package that can fail.'
plant_absent code failing-doctest/src/lib.rs '#\[test\]' \
    'failing-doctest: nothing else can fail' \
    'A unit test here would make the failure attributable to two mechanisms.'

plant_present code no-tests/src/lib.rs "$DEFECT_LINE" \
    'no-tests: the library is defective' \
    'A package with no tests and no defect demonstrates nothing: what makes this input worth recording is that gate 2 is green over code that is wrong.'
plant_absent code no-tests/src/lib.rs '#\[test\]|#\[cfg\(test\)\]' \
    'no-tests: no unit test and no cfg(test) module' \
    'Either would give this package a test, and the input is the absence of every kind.'
plant_absent raw no-tests/src/lib.rs '```' \
    'no-tests: no documentation example' \
    'A doc example is a test too, and cargo runs it. This is the one assertion here that must read the comments, because that is where a doc test lives.'
if [ -e "$HERE/no-tests/tests" ]; then
    plant_report "no-tests/tests exists." 'no-tests: no tests/ directory'
else
    plant_report ok 'no-tests: no tests/ directory'
fi

plant_present code vacuous-tests/src/lib.rs "$DEFECT_LINE" \
    'vacuous-tests: the library is defective' \
    'The point of this input is that the tests run and the defect survives.'
plant_present code vacuous-tests/src/lib.rs '#[test]' \
    'vacuous-tests: tests exist and run' \
    'Without tests this is the no-tests input again and the two rows would be one.'
plant_absent joined vacuous-tests/src/lib.rs "$ASSERT_RE" \
    'vacuous-tests: nothing asserts the answer' \
    'An assertion naming remaining_budget would make one of these tests real, and this input is tests that assert nothing about what the function returned. The match is made over the code with its newlines removed, so an assertion written across several lines is one this check sees.'

plant_present code ignored-tests/src/lib.rs "$DEFECT_LINE" \
    'ignored-tests: the library is defective' \
    'The ignored tests must be tests that would catch something.'
if plant_subject code ignored-tests/src/lib.rs 'ignored-tests: every test is ignored'; then
    n_test=$(grep -c '#\[test\]' "$PLANT_TEXT" || true)
    n_ignore=$(grep -c '#\[ignore' "$PLANT_TEXT" || true)
    if [ "${n_test:-0}" -gt 0 ] && [ "${n_test:-0}" -eq "${n_ignore:-0}" ]; then
        plant_report ok "ignored-tests: all ${n_test} test(s) are ignored"
    else
        plant_report "the code has ${n_test:-0} #[test] and ${n_ignore:-0} #[ignore], so a test here would run." \
            'ignored-tests: every test is ignored'
    fi
fi

plant_present code unlisted-member/listed/src/lib.rs "$CORRECT_EXPR" \
    'unlisted-member: the member is correct' \
    'The member is what makes the run green and its test count non-zero.'
plant_present code unlisted-member/unlisted/src/lib.rs "$DEFECT_LINE" \
    'unlisted-member: the unlisted package is defective' \
    'Without the defect there is no failing test for the workspace to be silent about.'
plant_present code unlisted-member/unlisted/src/lib.rs 'assert_eq!' \
    'unlisted-member: the unlisted package has a failing test' \
    'The input is a test that would fail and never runs.'
plant_present code unlisted-member/Cargo.toml 'members = ["listed"]' \
    'unlisted-member: members names only the member' \
    'Adding unlisted/ to members would make cargo run its test and this row would expect a failure.'
plant_absent code unlisted-member/Cargo.toml 'exclude' \
    'unlisted-member: nothing excludes the package' \
    'An exclude entry would be a deliberate statement that the package is outside the workspace, and what is planted here is the absence of any such statement.'

say ''
if [ "$plant_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "at least one planted input no longer contains what it was planted to contain, so a verdict from this run would be a verdict about a fixture that has been repaired. Nothing was proven."
    exit 2
fi
say "  every planted input still contains what ops/gates/gate-2.md says it does"

# ---------------------------------------------------------------------------
# Stage 3c: the self-check of the liveness check.
#
# The same argument as the flipped comparator. A liveness check that answered
# "live" to everything would report the gate proved however the workflow was
# weakened; one that answered "dead" to everything would block every run. The
# table below contains all three answers, so neither constant survives it.
# ---------------------------------------------------------------------------

STAGE='checking that this harness can tell a live gate command from a dead one'
rule 'Self-check: telling a live gate command from a dead one'
say 'Each file below is a whole workflow carrying exactly one way of switching'
say 'gate 2 off, or none. They are planted defects for the check stage 4 is'
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
    classify_command "$facts" "$TEST_CMD" "$AGGREGATE_JOB"
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

# ---------------------------------------------------------------------------
# Stage 3d: the self-check of the self-identification check.
#
# The same argument once more, one level in. Stage 4b asks about the job that
# runs this script the six questions stage 4 asks about the job it proves, and
# the check that asks them has to find this script in the file first. Every
# answer it can give has a planted input here that is owed it, so no constant
# answer survives this table.
# ---------------------------------------------------------------------------

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

# ---------------------------------------------------------------------------
# Stage 3e: the self-check of the attribution check.
#
# One text and five signatures whose owed answers differ, covering both kinds
# of signature and all three return values. A matcher that always agreed, or
# always refused, or that read `!` as an ordinary character, fails a row here.
# ---------------------------------------------------------------------------

STAGE='checking that this harness can tell one answer from another'
rule 'Self-check: telling the planted answer from any other answer'
selfcheck_log="$WORK_DIR/attribution.selfcheck"
printf '%s\n' 'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured' >"$selfcheck_log"
say "  text        : $(cat "$selfcheck_log")"
say ''

ATT_SIG=()
ATT_OWED=()
ATT_WHAT=()
add_att() { ATT_SIG+=("$1"); ATT_OWED+=("$2"); ATT_WHAT+=("$3"); }
add_att '^test result: FAILED\.'  0 'a signature the text carries'
add_att '^test result: ok\.'      1 'a signature the text does not carry'
add_att '!^test result: ok\.'     0 'a negated signature whose subject is absent'
add_att '!^test result: FAILED\.' 1 'a negated signature whose subject is present'
add_att '['                       2 'a signature that is not a usable regexp'
ATT_COUNT=${#ATT_SIG[@]}

att_bad=0
i=0
while [ "$i" -lt "$ATT_COUNT" ]; do
    output_matches "$selfcheck_log" "${ATT_SIG[$i]}"$'\n'
    got=$?
    if [ "$got" -eq "${ATT_OWED[$i]}" ]; then
        outcome='agree'
    else
        outcome='DISAGREE'
        att_bad=$((att_bad + 1))
    fi
    printf '  %-46s %-26s owed %s, answered %s  %s\n' \
        "${ATT_WHAT[$i]}" "/${ATT_SIG[$i]}/" "${ATT_OWED[$i]}" "$got" "$outcome"
    i=$((i + 1))
done
say ''
say "  disagreements: $att_bad of $ATT_COUNT (this must be 0)"
if [ "$att_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot tell one answer from another: $att_bad of the $ATT_COUNT planted signatures were judged wrongly. Every attribution below would be meaningless."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4: the command under proof must be a command CI runs, in a job whose
# failure fails the run. Without this the gate could be switched off in ci.yml
# while this harness went on proving a command nothing executes.
# ---------------------------------------------------------------------------

STAGE='checking that the workflow runs the command under proof'
rule 'The command under proof is a command CI runs'

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
say "  aggregate job    : '$AGGREGATE_JOB', which ops/gates/branch-protection.md names as the first required check to be added"
say "  trigger required : $REQUIRED_TRIGGER"
say "  triggers found   : $(sed -n 's/^TRIGGER //p' "$workflow_facts" | tr '\n' ' ')"
say ''
classify_command "$workflow_facts" "$TEST_CMD" "$AGGREGATE_JOB"
say "  '$TEST_CMD'"
say "    $LIVENESS"
say "$LIVENESS_WHY"
case "$LIVENESS" in
    live) ;;
    unreadable)
        note_bad "$WORKFLOW_REL has a shape this harness does not read, so it cannot say whether '$TEST_CMD' is a command CI runs. A job it failed to see is a job it would have said nothing about, so it proves nothing rather than proving something about the part it understood."
        ;;
    *)
        note_bad "'$TEST_CMD' is not run by any job of $WORKFLOW_REL whose failure would fail this run. Either the gate was weakened, or this harness is proving a command nothing executes. A proof of a command nothing runs is the defect class of AICD §39, so it is refused rather than reported."
        ;;
esac

if [ "$bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "the command under proof is not one CI runs where its failure fails the run, so no gate command was run here. Nothing was proven."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4b: and the job that runs THIS script must be live by the same six
# conditions. Stage 4 asks whether gate 2's command can fail the run. This asks
# whether the job asking that question can. One line of `continue-on-error:`
# here leaves every check on the pull request green and switches the whole
# demonstration off.
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
            emit error "no job of $WORKFLOW_REL runs this harness ('$SELF_CMD'), so nothing in CI presents these planted inputs to gate 2 and this run says nothing about any commit but this one. A proof CI does not run is the defect class of AICD §39. Nothing was proven."
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
# Stage 5: observation. Run each case once. Record the exit status, the output
# and what ran. Judge nothing yet.
# ---------------------------------------------------------------------------

STAGE='running gate 2 against every input'

run_gate() {
    # $1 package directory, $2 output file. Returns the gate command's own exit
    # status. No pipeline: the output is redirected to a file and nothing else
    # reads the status first.
    #
    # 255 is reserved for "the command did not run at all", so that a `cd` that
    # failed can never be recorded as gate 2 reporting a failure. cargo exits
    # 0, 1 or 101 here and never 255.
    local dir="$1" out="$2"
    local status
    ( cd "$dir" || exit 255; "${TEST_ARGV[@]}" ) >"$out" 2>&1
    status=$?
    return "$status"
}

rule 'Observation: gate 2 run against each input, once'
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    pkg="${CASE_PKG[$i]}"
    log="$WORK_DIR/$pkg.log"

    say "--- $pkg   (${CASE_CLASS[$i]})"
    say "    command    : ( cd fixtures/planted/gate-2/$pkg && $TEST_CMD )"
    run_gate "$HERE/$pkg" "$log"
    status=$?
    if [ "$status" -eq 255 ]; then
        VERDICT_REACHED=1
        emit error "the gate command for '$pkg' could not be started at all (status 255). That is not gate 2 reporting a failure, and this harness will not record it as one."
        exit 2
    fi
    if [ "$status" -eq 0 ]; then verdict='pass'; else verdict='fail'; fi
    strip_ansi "$log" "$log.plain"
    count_tests "$log.plain"
    OBS_STATUS[$i]="$status"
    OBS_VERDICT[$i]="$verdict"
    OBS_LOG[$i]="$log.plain"
    OBS_PASSED[$i]="$TESTS_PASSED"
    OBS_FAILED[$i]="$TESTS_FAILED"
    OBS_IGNORED[$i]="$TESTS_IGNORED"
    OBS_BINARIES[$i]="$TESTS_BINARIES"
    say "    exit status: $status"
    say "    verdict    : $verdict"
    say "    tests      : $TESTS_PASSED passed, $TESTS_FAILED failed, $TESTS_IGNORED ignored, across $TESTS_BINARIES test binaries"
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
# Stage 6: attribution. An exit status is not evidence on its own. Where the
# gate answered what this table owes, the output must show that it answered it
# for the reason this row planted.
#
# The rows where the gate answered something else are left to the comparator
# below: those are the rows where gate 2 stopped behaving as documented, and
# that is a gate failure (exit 1), not an unreadable observation (exit 2).
# ---------------------------------------------------------------------------

STAGE='checking that each answer is the answer this proof planted'
rule 'Attribution: the answer gate 2 gave is the answer to the planted question'
printf '  %-21s %-9s %-9s %s\n' 'input' 'class' 'observed' 'attributable to the planted mechanism'
printf '  %-21s %-9s %-9s %s\n' '---------------------' '---------' '---------' '-------------------------------------'
attribution_bad=0
attribution_broken=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${OBS_VERDICT[$i]}" != "${CASE_EXP[$i]}" ]; then
        printf '  %-21s %-9s %-9s %s\n' \
            "${CASE_PKG[$i]}" "${CASE_CLASS[$i]}" "${OBS_VERDICT[$i]}" \
            "not asked: the gate did not answer '${CASE_EXP[$i]}' here, which the table below reports"
        i=$((i + 1))
        continue
    fi
    output_matches "${OBS_LOG[$i]}" "${CASE_SIG[$i]}"
    status=$?
    case "$status" in
        0) printf '  %-21s %-9s %-9s %s\n' "${CASE_PKG[$i]}" "${CASE_CLASS[$i]}" "${OBS_VERDICT[$i]}" 'yes' ;;
        1)
            printf '  %-21s %-9s %-9s %s\n' "${CASE_PKG[$i]}" "${CASE_CLASS[$i]}" "${OBS_VERDICT[$i]}" "NO: $ATTRIB_WHY"
            attribution_bad=$((attribution_bad + 1))
            ;;
        *)
            printf '  %-21s %-9s %-9s %s\n' "${CASE_PKG[$i]}" "${CASE_CLASS[$i]}" "${OBS_VERDICT[$i]}" "HARNESS: $ATTRIB_WHY"
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
    emit error "$attribution_bad case(s) gave gate 2's owed answer for a reason this proof did not plant. A command that exits non-zero because a package no longer compiles, or because '--locked' rejected a stale Cargo.lock, is not gate 2 catching a failing test; a command that exits 0 having compiled nothing is not gate 2 passing a clean suite. Read the attribution table above, repair the fixture so the planted mechanism is the only thing at work in it, and re-run. Nothing was proven about gate 2 by this run."
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
    printf '  %-21s %-9s %-9s %-9s %-6s %s\n' 'input' 'class' 'expected' 'observed' 'exit' 'result'
    printf '  %-21s %-9s %-9s %-9s %-6s %s\n' '---------------------' '---------' '---------' '---------' '------' '--------'
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
        printf '  %-21s %-9s %-9s %-9s %-6s %s\n' \
            "${CASE_PKG[$i]}" "${CASE_CLASS[$i]}" "$expected" "$observed" "$status" "$agreement"
        i=$((i + 1))
    done
}

STAGE='judging the observations against the real expectations'
rule 'The proof: what gate 2 owes each input'
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
        non_complementary="$non_complementary ${CASE_PKG[$i]}"
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
    # an inverted comparison gets every row wrong, and the inversion can be in
    # this harness or in the gate.
    #
    # THIS IS EXIT 2 AND NOT EXIT 1, and the difference was found by planting
    # it. Inverting the comparison in `judge` above leaves per-row
    # complementarity intact, because the two passes still differ on every row:
    # the direct pass disagrees eight times and the flipped pass agrees eight
    # times. So the complementarity check does not fire, and the natural
    # reading of "the direct pass disagreed" is `gate_broken`, which exits 1
    # and tells the reader that gate 2 stopped catching its planted defects.
    # That is a claim this run cannot support. Which of the two is inverted is
    # exactly what an all-rows-wrong run cannot distinguish, so the honest
    # status is 2, "nothing was proven either way", and the message names both
    # candidates and puts the harness first.
    harness_broken=1
    emit error "every one of the $CASE_COUNT cases disagreed, which no gate that always passes or always fails can produce: those still agree with part of the table. Only an inverted comparison does, and this run cannot tell whether the inversion is in this harness's comparator or in the gate command itself, so it reports that nothing was proven rather than blaming the gate. Look at the comparator in judge() first, then at what the workflow actually runs, before looking at the planted inputs."
fi

if [ "$direct_mismatches" -ne 0 ]; then
    gate_broken=1
    emit error "gate 2 did not behave the way ops/gates/gate-2.md claims: $direct_mismatches of $CASE_COUNT cases disagreed. Read the table above. A 'catches' row that observed 'pass' means the gate stopped catching a failing test that is still planted in the tree, so gate 2 is no longer proven (AICD §14) and no document may cite it until it is re-proved. A 'blind' row that observed 'fail' means cargo's behaviour changed and the proof file's account of what gate 2 does not establish is out of date, which is also a re-proof."
fi

VERDICT_REACHED=1

if [ "$harness_broken" -ne 0 ]; then
    exit 2
fi
if [ "$gate_broken" -ne 0 ]; then
    exit 1
fi

say ''
say "WHAT GATE 2 DID WITH EACH INPUT, which is the table ops/gates/gate-2.md is written from:"
say ''
printf '  %-21s %-9s %-6s %-8s %s\n' 'input' 'class' 'exit' 'verdict' 'tests that ran'
printf '  %-21s %-9s %-6s %-8s %s\n' '---------------------' '---------' '------' '--------' '--------------'
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    printf '  %-21s %-9s %-6s %-8s %s\n' \
        "${CASE_PKG[$i]}" "${CASE_CLASS[$i]}" "${OBS_STATUS[$i]}" "${OBS_VERDICT[$i]}" \
        "${OBS_PASSED[$i]} passed, ${OBS_FAILED[$i]} failed, ${OBS_IGNORED[$i]} ignored"
    i=$((i + 1))
done

say ''
say "ESTABLISHED BY THIS RUN:"
say "  - gate 2's command is run by a job of $WORKFLOW_REL that an unfiltered"
say "    '$REQUIRED_TRIGGER' fires, with no 'if:' and no 'continue-on-error:' on the job"
say "    or on the step, and '$AGGREGATE_JOB' waits on that job and carries no"
say "    'continue-on-error:' itself, so its failure fails this run;"
say "  - the '$SELF_JOB' job that runs this script is live by those same six"
say "    conditions, so the refusals above are refusals that fail this run too;"
say "  - every planted input still contains what it was planted to contain;"
say "  - gate 2 failed all $class_catches input(s) carrying a test that fails, at the unit,"
say "    integration and documentation levels, and it passed the control;"
say "  - gate 2 PASSED all $class_blind input(s) that report nothing, which is what this fixture"
say "    exists to record and is not a defect in the gate but a property of it;"
say "  - each answer carried the signature of the mechanism planted for it;"
say "  - and this harness was shown, on this run, able to tell those answers apart: its"
say "    comparator, its liveness check, its self-identification check and its"
say "    attribution check were each run over planted inputs whose owed answers differ."
say ''
say "WHAT A GREEN GATE 2 THEREFORE ESTABLISHES, said plainly because it is less than a"
say "green check looks like:"
say "  - every test that this workspace's 'members' list reaches, that is not marked"
say "    '#[ignore]', and that asserts something, passed on this runner;"
say "  - and the workspace compiles under cfg(test) on this runner."
say ''
say "WHAT IT DOES NOT ESTABLISH, AND NO RUN OF THIS SCRIPT CAN MAKE IT ESTABLISH:"
say "  - that any test exists. 'no-tests' above exits 0 with 0 tests run, over a"
say "    library that returns the wrong answer;"
say "  - that the tests which ran assert anything. 'vacuous-tests' above exits 0 with"
say "    3 tests passing over that same wrong library;"
say "  - that the tests which exist ran. 'ignored-tests' above exits 0 with 3 tests"
say "    ignored and 0 run, over that same wrong library;"
say "  - that 'workspace-wide' reaches every crate in the tree. 'unlisted-member'"
say "    above exits 0 while a failing test sits in a package the workspace's"
say "    'members' list does not name and cargo never mentions;"
say "  - therefore: that the code is correct, or that it is covered. Gate 2 is a"
say "    regression detector for the tests that exist, run and assert. The gates that"
say "    make it mean more are spec/CI_CD.md section 1 gate 4, the coverage matrix"
say "    (every criterion has a test, every test names a criterion), and gate 6, the"
say "    mutation score threshold (the tests fail when the code is broken). Neither"
say "    exists yet. Until they do, a green gate 2 is worth exactly the four lines"
say "    above and no more, and no document may cite it for more;"
say "  - that a failing '$AGGREGATE_JOB' blocks a merge. That is branch protection and"
say "    not this file. ops/gates/branch-protection.md records that 'main' has no"
say "    required status checks at all and that each name is added on the day its"
say "    proof file lands;"
say "  - that the failure is visible where a human would look. This script cannot watch"
say "    the pull request check it writes to. That clause of AICD §14 is established by"
say "    an observed run and recorded in ops/gates/gate-2.md by the lead;"
say "  - that the job running this script will still be live on the next commit. A"
say "    'continue-on-error:' added to '$SELF_JOB' later makes this script refuse, and a"
say "    job carrying that key does not fail the run when it refuses. The refusal is"
say "    then an annotation and a human reading the diff of $WORKFLOW_REL is the last"
say "    link. That path is tier 2 for this reason."
say ''
say "Gate 2 has been seen to fail on a planted defect, on this commit, and has been"
say "seen to pass four inputs it should arguably not pass. Both halves are the"
say "demonstration; ops/gates/gate-2.md records what follows from each."
exit 0
