#!/usr/bin/env bash
#
# prove.sh: the proof harness for gate 13 of spec/CI_CD.md section 1.
#
# ===========================================================================
# GATE 13 IS ONE CHECK ASKED OF EVERY COMMIT ON A PULL REQUEST, AND HALF OF
# PROVING IT IS PROVING THAT "EVERY COMMIT" NAMES A SET.
# ===========================================================================
#
# spec/CI_CD.md section 1 item 13: "Commit-trailer gate (every commit on the
# PR: Conventional Commits, `Ticket: <id>` and `Spec: <document>#<section>`
# trailers, per CONVENTIONS; PRD G-02)". spec/CONVENTIONS.md "Git".
# spec/PRD.md G-02.
#
# Ticket: ORI-T-0017.  Spec: spec/runbooks/prove-gate.md, spec/TESTING.md
# sections 1 and 4, spec/CI_CD.md section 1 item 13, spec/CONVENTIONS.md "Git".
# Record: ops/gates/gate-13.md.  Rule: AICD §14.
#
# ===========================================================================
# THE DEFECT THIS GATE EXISTS FOR, AND WHY A REGEX CANNOT BE THE GATE
# ===========================================================================
#
# Git parses trailers out of the LAST PARAGRAPH of a commit message and out of
# nothing else. A message written like this:
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
# carries both required lines, correctly spelled, at the start of their lines,
# and any human and any regex finds them. `git interpret-trailers --parse`
# returns ONE trailer for it, `Co-Authored-By`, because the last paragraph is
# the Co-Authored-By line on its own. `%(trailers:key=Ticket,valueonly)` is
# empty. Everything downstream that reads trailers, the traceability chain of
# AICD §13 among them, sees nothing.
#
# That message is not hypothetical. The lead shipped exactly this shape in the
# first commit this project made under CONVENTIONS, which is why
# `messages/trap-trailers-in-earlier-paragraph.txt` is in this directory and
# why stage 7 below exists: it runs a regex over that message and the gate over
# the same message, prints both answers side by side, and refuses if the regex
# does not accept it or if git does parse it. A proof of this gate that did not
# put that contrast in its own output would be recording the conclusion and
# throwing away the evidence.
#
# The gate's implementation therefore asks git and never a regex, and it lives
# in ONE place, `scripts/gates.sh`, which CI calls and which this harness
# calls. Ruling R28: a fixture directory holds inputs a gate is run against,
# never the checker the gate invokes.
#
# ===========================================================================
# THE SECOND HALF, WHICH IS ENUMERATION, AND WHICH IS WHERE THIS GATE WOULD DIE
# QUIETLY
# ===========================================================================
#
# "Every commit on the PR" is a set, and a gate that computes the empty set and
# reports success has checked nothing while looking exactly like a gate that
# checked everything and found nothing wrong. That is AICD §39's "present but
# reporting nothing" and it is the defect class this whole project is built
# against.
#
# There are four events under which `.github/workflows/ci.yml` runs, and the
# set is different under each:
#
#   pull_request    merge-base(base.sha, head.sha)..head.sha, cross-checked
#                   against the commit count GitHub itself reports
#   merge_group     merge_group.base_sha..head_sha, which are the REBASED
#                   commits: new objects carrying the messages under review
#                   (spec/CI_CD.md section 2)
#   push            before..after, the commits this push landed
#   workflow_dispatch  the single checked-out commit, which is a smaller claim
#                   than the one the gate makes on a pull request, and the
#                   verdict says so
#
# and off a GitHub event, at a coder's desk, the set is the branch's own
# commits against `main`.
#
# NONE of those paths may produce an empty set and an answer of "pass". The
# case table below plants nine ways for the set to come out empty or
# undeterminable and one way for there to be legitimately nothing at a desk,
# and every one of those eleven rows carries `!^  PASSED\.` as a required
# negative signature. That negative is the single most load-bearing assertion
# in this file.
#
# THE SHALLOW CLONE IS THE ONE THAT WOULD HAVE HAPPENED. `actions/checkout`
# defaults to `fetch-depth: 1`. In that clone the base SHA a pull_request
# payload names is simply not an object the repository holds, so a gate that
# enumerated "whatever is present" would check a shorter range and report on it
# as though it were the whole pull request. Gate 13 refuses instead, and stage
# 4c below reads `fetch-depth` out of the `gate-13` job so that the reason is
# named here rather than met as a surprise refusal on somebody's pull request.
#
# ===========================================================================
# THE FIVE QUESTIONS, AND WHY EACH NEEDS ITS OWN ANSWER
# ===========================================================================
#
# ORI-T-0013, ORI-T-0014 and ORI-T-0016 found defects between them in the
# harnesses for gates 1, 2 and 7, every one a code path reporting success for a
# state that is not success. Their questions are asked here, in their shape.
#
#   1. IS THIS A COMMAND CI RUNS?  Stage 4. An anchored grep for the command
#      string in `.github/workflows/ci.yml` cannot tell a command CI runs from
#      a command CI does not: the line can belong to a job that is not in the
#      required aggregate's `needs:`, or to a job or step carrying
#      `continue-on-error:` or an `if:`, or it can be text inside another
#      step's shell script, or the workflow's triggers can mean nothing fires
#      on a pull request at all. Every one leaves the line byte-identical. The
#      file is parsed (`workflow-facts.awk`) and the question asked of the
#      structure. That is the same discipline one layer up from the gate
#      itself: a `run:` line that is really a here-document reads the same to a
#      grep as a `Ticket:` line that is really body text.
#
#   2. DOES GATE 13 GIVE EACH INPUT THE VERDICT IT OWES?  Stages 8 and 9. One
#      planted input per row, judged by one comparator run twice over one set
#      of observations with every expectation flipped, requiring per-row
#      complementarity.
#
#   3. IS THE ANSWER THE ANSWER TO THE PLANTED QUESTION?  Stage 6 records and
#      stage 8b judges. A non-zero exit is not evidence on its own: gate 13
#      exits 1 when a commit does not conform and 2 when it could not name the
#      commits at all, and a row that accepted any non-zero answer would count
#      the second as the first. Each row names the reason codes and the
#      sentences its planted mechanism leaves in the output, including
#      signatures that must be ABSENT.
#
#   4. IS THE JOB THAT ASKS THE FIRST THREE ITSELF LIVE?  Stage 4b.
#      `continue-on-error: true` on `gate-13-proof` turns every refusal below
#      into a green check: the harness still runs, still refuses, still writes
#      its annotation, and GitHub records the job as a success. The same six
#      structural conditions are asked of this script's own job, which it finds
#      in the file by the command that invokes it AND by name, with both
#      required to agree. All three existing gates ship this check.
#
#   5. IS THE PLANT STILL PLANTED?  Stage 3b. Eleven of the rows below are
#      planted to be PASSED, and a pass looks the same whether the plant is
#      there or not. The trap message in particular is a pass for a regex and a
#      refusal for the gate, and it is only that while its `Ticket:` and
#      `Spec:` lines sit in a paragraph that is not the last one. Every planted
#      message is asserted against its source file before any gate runs.
#
# ===========================================================================
# THE INVERSION, WHICH IS THE DANGEROUS PART
# ===========================================================================
#
# This job's success condition is the opposite of the job it proves: most of
# its rows pass when the gate command fails or refuses. Getting that backwards
# produces a job that passes on every input, which is the exact defect AICD §14
# names. Four things guard it, and none of them is a comment.
#
#   1. The case table is not all failures and not all passes. Eleven inputs
#      gate 13 must pass, twenty-four it must fail, nine it must refuse and two
#      it must report as nothing checked. One comparator judges all of them, so
#      a harness that reported failure for everything would disagree with
#      eleven rows and this run would go red.
#
#   2. The comparator is run twice over one set of observations: once against
#      the real expectations, and once against every expectation flipped. Row
#      by row, the two passes must reach OPPOSITE conclusions. A comparator
#      that always answers "agree" agrees in both passes; one that always
#      answers "DISAGREE" disagrees in both; either way a row matches itself
#      and this run reports that the harness, not the gate, is broken.
#
#      The check is per row and it is not a count. ops/gates/gate-1.md records
#      why: a count is only complementary while the direct pass agrees on every
#      row, so a real fixture defect made gate 1's harness report ITSELF
#      broken. Per-row complementarity is the property actually wanted and it
#      holds whatever the gate does.
#
#   3. Every checker this harness adds is exercised before it is trusted. The
#      liveness check of stage 4 is run over twelve planted workflows in
#      `workflow-samples/`; the self-identification check of stage 4b over six;
#      the checkout-depth check of stage 4c over four; and the attribution
#      matcher over a text carrying three different answers. Each table is
#      required to contain every answer its check can give.
#
#   4. The plants are asserted, not assumed (question 5 above).
#
# Separating observation from judgement is what makes those passes honest: the
# gate runs once per row in stage 6, its exit status and its output are
# recorded, and every judgement after that is a pure function of what was
# recorded.
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
# `set -e` is deliberately NOT used. Most of the rows here expect a non-zero
# status, and an errexit shell that leaves before the verdict is a gate that
# exits 0 without checking anything. The EXIT trap below turns any such early
# departure into exit 2.
#
# WHY THE PLANTED RUNS HAVE `GITHUB_ACTIONS` UNSET. `scripts/gates.sh` emits
# `::error::` annotations under GitHub Actions, and those annotations are
# addressed to the author of the commit under review. The commits under review
# here are planted ones that exist only inside this run and inside a temporary
# directory, so their annotations would land on the pull request as thirty-odd
# lines reading "gate 13 failed" about commits nobody wrote. This harness's own
# annotations, written by `emit` below, are not suppressed: those are the ones
# a human is meant to see.
#
# ===========================================================================
# EXIT STATUS
# ===========================================================================
#
#   0  the proof holds: gate 13's command is run by a job whose failure fails
#      the run, that job's checkout asks for the whole history, the job that
#      runs this script is live by those same six conditions, every planted
#      message is still planted, the trap message is accepted by a regex and
#      refused by the gate, gate 13 gave every planted input the verdict it
#      owes, no input on which it checked nothing was reported as a pass, each
#      answer carried the signature of the mechanism planted for it, and this
#      harness was shown able to tell those answers apart.
#   1  gate 13 did not behave the way ops/gates/gate-13.md claims. Either a
#      check has been weakened or a planted input no longer contains what it
#      was planted to contain. Gate 13 may not be cited until this is 0 again
#      (spec/runbooks/prove-gate.md, "Rollback: revoke the proof").
#   2  this harness could not prove what it claims: it could not tell agreement
#      from disagreement, or every case disagreed, which means a comparison is
#      inverted and does not say whether the inversion is in this harness or in
#      the gate, or the workflow no longer runs the gate command where its
#      failure would fail the run, or it no longer runs this script there, or
#      this script could not find itself in the workflow, or the `gate-13`
#      job's checkout no longer asks for the whole history, or a planted input
#      is no longer planted, or an answer was given for a reason that is not
#      the planted one. Nothing was proven either way.
#   3  a prerequisite is missing, so no gate command ran at all: git, awk, jq
#      or `scripts/gates.sh` is not usable here, or the planted repositories
#      could not be built. Nothing was checked, and this is not a pass.
#
# Run it from anywhere: `bash fixtures/planted/gate-13/prove.sh`.

set -uo pipefail

# ---------------------------------------------------------------------------
# Location
# ---------------------------------------------------------------------------

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
WORKFLOW_REL='.github/workflows/ci.yml'
AWK_PARSER="$HERE/workflow-facts.awk"
SAMPLE_DIR="$HERE/workflow-samples"
MESSAGE_DIR="$HERE/messages"

# The gate's implementation does not live in this fixture directory. Ruling
# R28: a fixture directory holds planted defects, which are inputs a gate is
# run against; the checker a gate invokes is not one. `scripts/gates.sh` sits
# beside the other scripts a gate runs, CI calls it, a coder calls it, and this
# harness calls it, so there is exactly one implementation and all three
# callers reach it. A second copy inline in the workflow would be the other
# half of the same defect: a gate that can be repaired in the copy CI does not
# run.
GATES_SH="$REPO_ROOT/scripts/gates.sh"
GATES_SH_REL='scripts/gates.sh'

# The command the `gate-13` job runs, written once here and compared against
# the workflow file below so that this script cannot drift from what it is
# reporting on.
GATE_CMD="bash $GATES_SH_REL --commit-trailers"

# The job that runs it, whose checkout must ask for the whole history, and the
# value it must ask for. `fetch-depth: 0` is the only thing that puts the base
# and head commits of a pull request on the runner.
GATE_JOB='gate-13'
REQUIRED_FETCH_DEPTH='0'

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

# This harness's own job, and the command that runs it.
#
# HOW THIS SCRIPT FINDS ITSELF IN THE FILE IT IS PARSING, and why it is both of
# these and not either one:
#
#   by command  SELF_CMD is built from this file's own path relative to the
#               repository root, so it is the string the workflow must write to
#               run THIS script. A harness that looked itself up by job name
#               alone would find nothing after a rename and, written the
#               natural way, would skip its own check and carry on.
#   by name     SELF_JOB is the name that job has. A harness that looked itself
#               up by the command alone would follow a rename without noticing,
#               and a renamed proof job is a deliberate edit to the job that
#               carries the proof. It gets a re-proof, not a guess.
#
# The two must agree: exactly one job runs this script, and it is that job. Any
# other answer is refused (exit 2) rather than reported as a pass.
SELF_JOB='gate-13-proof'
SELF_FILE="$HERE/$(basename -- "${BASH_SOURCE[0]}")"
case "$SELF_FILE" in
    "$REPO_ROOT"/*) SELF_REL="${SELF_FILE#"$REPO_ROOT"/}" ;;
    *)              SELF_REL='' ;;
esac
SELF_CMD="bash $SELF_REL"

# The message whose contrast is the point of this gate, named once so stage 3b
# and stage 7 cannot drift apart.
TRAP_ID='trap-trailers-in-earlier-paragraph'

# The vocabulary `scripts/gates.sh` declares, repeated here only so that stage
# 3b can assert the two have not drifted: several signatures below quote these
# strings back out of the gate's own refusal text, and a row quoting a string
# the gate no longer prints would be unattributable rather than wrong.
EXPECTED_TYPES='feat fix docs ci chore'

# ---------------------------------------------------------------------------
# The case table: one row per planted input, with the family it belongs to, the
# verdict gate 13 owes it, the class of thing the row is about, the setup that
# builds it, and the signatures the planted mechanism leaves in the output.
#
# THE VERDICT COLUMN HAS FOUR VALUES, not two, because `bash scripts/gates.sh
# --commit-trailers` answers four different questions and giving two of them
# one name is this gate's own defect class:
#
#   pass     exit 0. Every commit in the set conforms, and the set was not
#            empty.
#   fail     exit 1. At least one commit does not conform.
#   refuse   exit 2. The set could not be determined, so no commit was
#            checked. An unknown event, an unreadable payload, a base or head
#            commit missing from a shallow clone, an empty range, or GitHub and
#            the gate disagreeing about how many commits the pull request has.
#   nothing  exit 3. There was legitimately nothing to enumerate, which happens
#            only off a GitHub event: a local branch with no commits of its
#            own. Nothing was checked, and this status says so rather than
#            reporting a pass.
#
# THE FAMILY COLUMN IS NOT DECORATION. `message` rows present one commit and
# ask what the gate says about its message; `range` rows ask what the gate does
# with a set of more than one commit and with a merge commit in it; and
# `enumeration` rows ask what the gate does when the set is empty, unnameable
# or produced by an event other than `pull_request`. A table missing a family
# would be a table reporting on a gate whose other half ran unwatched.
#
# A signature beginning with `!` must NOT appear. The most important one in
# this file is `!^  PASSED\.`, which every `refuse` and every `nothing` row
# carries and which stage 3 asserts they carry: a run that checked no commit
# and printed PASSED is the whole defect this gate is built against, and it
# would otherwise be caught only by the exit status, which one weakened line in
# `scripts/gates.sh` could change.
# ---------------------------------------------------------------------------

CASE_ID=()
CASE_FAMILY=()
CASE_SETUP=()
CASE_EXP=()
CASE_CLASS=()
CASE_SIG=()

add_case() {
    # $1 id, $2 family, $3 setup token, $4 expected verdict, $5 class,
    # $6.. signatures.
    CASE_ID+=("$1")
    CASE_FAMILY+=("$2")
    CASE_SETUP+=("$3")
    CASE_EXP+=("$4")
    CASE_CLASS+=("$5")
    shift 5
    local sig='' pat
    for pat in "$@"; do
        sig="$sig$pat"$'\n'
    done
    CASE_SIG+=("$sig")
}

# --- the clean controls ----------------------------------------------------
#
# Four messages the gate must accept. Without them this run could not tell a
# working gate from one that refuses everything, and a gate that refuses
# everything is not a gate, it is an outage.
#
# `clean-full` also pins the OTHER half of the observation the gate makes about
# the `Spec:` document and never judges: in a planted repository there is no
# `spec/` tree, so the gate prints "is NOT in the tree this run checked out"
# and passes the commit anyway. `spec-doc-present` below pins the half where
# the document is there. Both branches of a deliberate non-enforcement are
# exercised, because a non-enforcement nobody runs is indistinguishable from an
# enforcement nobody wrote.

add_case  clean-full      message  msg:clean-full      pass  control \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  [0-9a-f]{12} ok +feat\(gates\): a message with everything this gate asks for$' \
    'observed, not judged: spec/CI_CD\.md is NOT in the tree this run checked out' \
    '^  PASSED\.' \
    '!REFUSED' \
    '!      codes: '

add_case  clean-no-body   message  msg:clean-no-body   pass  control \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  [0-9a-f]{12} ok +fix\(gates\): a subject and a trailer block, no body$' \
    '^  PASSED\.' \
    '!REFUSED'

add_case  clean-no-scope  message  msg:clean-no-scope  pass  control \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  [0-9a-f]{12} ok +docs: a conventional subject with no scope at all$' \
    '^  PASSED\.' \
    '!REFUSED'

# Conventional Commits v1.0.0 puts `!` before the colon. No commit on `main`
# uses it and the grammar has it, so the gate accepts it rather than refusing a
# shape the specification it cites defines. This row is the one that would go
# red if somebody tightened the subject pattern by hand.
add_case  clean-breaking  message  msg:clean-breaking  pass  control \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  [0-9a-f]{12} ok +feat\(store\)!: the breaking-change marker is part of the grammar$' \
    '^  PASSED\.' \
    '!REFUSED'

# --- the trailers are absent ------------------------------------------------
#
# Three different states hide behind "git reports no Ticket trailer", and a
# coder can only act on the one they are actually in. The negative signatures
# are what keep them apart: a gate that answered MISSING to all three would
# agree with the verdict column of every one of these rows and disagree with
# every signature.

add_case  no-trailers   message  msg:no-trailers   fail  catches \
    '^      codes: TICKET_MISSING SPEC_MISSING$' \
    "no 'Ticket:' trailer\. spec/CONVENTIONS\.md 'Git'" \
    "no 'Spec:' trailer\. spec/CONVENTIONS\.md 'Git'" \
    '^  checked 1 commit\(s\), 1 refused$' \
    '^  FAILED\.' \
    '!UNPARSED' \
    '!^  PASSED\.'

add_case  ticket-only   message  msg:ticket-only   fail  catches \
    '^      codes: SPEC_MISSING$' \
    "no 'Spec:' trailer\." \
    '^  FAILED\.' \
    '!TICKET_' \
    '!^  PASSED\.'

add_case  spec-only     message  msg:spec-only     fail  catches \
    '^      codes: TICKET_MISSING$' \
    "no 'Ticket:' trailer\." \
    '^  FAILED\.' \
    '!SPEC_' \
    '!^  PASSED\.'

# --- the trailers are there and the value is not --------------------------

add_case  ticket-value-bad      message  msg:ticket-value-bad      fail  catches \
    '^      codes: TICKET_VALUE$' \
    "the Ticket trailer's value is \[T-17\] and a ticket identifier here matches \^ORI-T-\[0-9\]\[0-9\]\[0-9\]\[0-9\]\\\$" \
    '^  FAILED\.' \
    '!TICKET_MISSING' \
    '!TICKET_UNPARSED' \
    '!^  PASSED\.'

# Git returns both. A consumer reading %(trailers:key=Ticket,valueonly) gets
# two lines and has to pick one, and whichever it picks the other commit's
# ticket is wrong. The gate refuses rather than picking.
add_case  ticket-duplicate      message  msg:ticket-duplicate      fail  catches \
    '^      codes: TICKET_DUPLICATE$' \
    "2 'Ticket:' trailers, so the value a consumer reads is ambiguous" \
    '^  FAILED\.' \
    '!TICKET_VALUE' \
    '!^  PASSED\.'

# The one that IS consumed. Git's trailer lookup is case-insensitive, so this
# trailer reaches every git consumer; the gate refuses it anyway, because
# CONVENTIONS writes `Ticket:` and the trailer is also read by humans and by
# tools that are not git. A gate that only asked "can git see it" would pass
# this row, which is why the code has a name of its own.
add_case  ticket-lowercase-key  message  msg:ticket-lowercase-key  fail  catches \
    '^      codes: TICKET_CASE$' \
    "git parsed a trailer whose key is a case variant of 'Ticket'" \
    '^  FAILED\.' \
    '!TICKET_MISSING' \
    '!TICKET_UNPARSED' \
    '!^  PASSED\.'

# Git folds an indented continuation into the value, so the value a consumer
# resolves is the identifier with a sentence stuck to it. The signature pins
# the whole folded value, because a gate that read only the first token would
# report this commit clean and the trailer would still be unresolvable.
add_case  ticket-folded-value   message  msg:ticket-folded-value   fail  catches \
    '^      codes: TICKET_VALUE$' \
    "the Ticket trailer's value is \[ORI-T-0017 and a continuation line that becomes part of the value\]" \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  spec-value-no-anchor    message  msg:spec-value-no-anchor    fail  catches \
    '^      codes: SPEC_VALUE$' \
    "the Spec trailer's value is \[CI_CD\.md\] and spec/CONVENTIONS\.md 'Git' asks for '<document>#<section>'" \
    '^  FAILED\.' \
    '!SPEC_MISSING' \
    '!^  PASSED\.'

add_case  spec-value-empty-anchor message  msg:spec-value-empty-anchor fail  catches \
    '^      codes: SPEC_VALUE$' \
    "the Spec trailer's value is \[CI_CD\.md#\]" \
    '^  FAILED\.' \
    '!^  PASSED\.'

# --- the subject -----------------------------------------------------------
#
# The shape and the vocabulary are told apart deliberately: a subject with the
# right shape and the wrong type is a vocabulary problem, and reporting it as
# an unreadable subject would send the author to repair the wrong thing. The
# `!SUBJECT_SHAPE` negatives on the last two rows are what assert that.

add_case  subject-no-colon      message  msg:subject-no-colon      fail  catches \
    '^      codes: SUBJECT_SHAPE$' \
    "the subject is not '<type>\[\(<scope>\)\]\[!\]: <description>': \[feat gates a subject with no colon at all\]" \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  subject-no-space      message  msg:subject-no-space      fail  catches \
    '^      codes: SUBJECT_SHAPE$' \
    "\[feat\(gates\):no space after the colon\]" \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  subject-double-space  message  msg:subject-double-space  fail  catches \
    '^      codes: SUBJECT_SHAPE$' \
    'the description begins with a further space: Conventional Commits puts exactly one space after the colon' \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  subject-empty-description message msg:subject-empty-description fail catches \
    '^      codes: SUBJECT_SHAPE$' \
    "\[feat\(gates\):\]" \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  subject-unknown-type  message  msg:subject-unknown-type  fail  catches \
    '^      codes: SUBJECT_TYPE$' \
    "the type is 'wip' and this repository's types are: feat fix docs ci chore" \
    '^  FAILED\.' \
    '!SUBJECT_SHAPE' \
    '!^  PASSED\.'

add_case  subject-uppercase-scope message msg:subject-uppercase-scope fail catches \
    '^      codes: SUBJECT_SCOPE$' \
    "the scope is 'Gates' and a scope here matches \^\[a-z0-9-\]\+\\\$" \
    '^  FAILED\.' \
    '!SUBJECT_SHAPE' \
    '!^  PASSED\.'

# --- THE TRAP, which is the point of this gate -----------------------------
#
# Three messages whose `Ticket:` and `Spec:` lines are present, correctly
# spelled and at the start of their lines, and which git's trailer parser does
# not see. A regex-based gate passes all three. Stage 7 below prints the regex
# answer and the git answer side by side for the first of them, which is the
# shape the lead actually shipped.
#
# The refusal code is UNPARSED and not MISSING, and `!MISSING` is what asserts
# it: "the line is in the wrong paragraph" and "the line is absent" are
# different repairs, and a gate that reported the second for the first would
# send an author to add a line that is already there.

add_case  trap-trailers-in-earlier-paragraph message msg:trap-trailers-in-earlier-paragraph fail catches \
    '^  [0-9a-f]{12} REFUSED +feat\(gates\): THE TRAP, exactly as the lead wrote it$' \
    '^      codes: TICKET_UNPARSED SPEC_UNPARSED$' \
    "the message contains a 'Ticket:' line and GIT'S TRAILER PARSER DOES NOT SEE IT" \
    "the message contains a 'Spec:' line and GIT'S TRAILER PARSER DOES NOT SEE IT" \
    'Git reads trailers out of the LAST PARAGRAPH of the message only' \
    '^  checked 1 commit\(s\), 1 refused$' \
    '^  FAILED\.' \
    '!TICKET_MISSING' \
    '!SPEC_MISSING' \
    '!^  PASSED\.'

# The same defect with no body at all: the trailers share the subject's
# paragraph. `%s` collapses that paragraph into one line, so the subject a
# consumer displays swallows both trailers, and the gate still reports them
# UNPARSED rather than reporting a malformed subject.
add_case  trap-trailers-glued-to-subject message msg:trap-trailers-glued-to-subject fail catches \
    '^      codes: TICKET_UNPARSED SPEC_UNPARSED$' \
    "GIT'S TRAILER PARSER DOES NOT SEE IT" \
    '^  FAILED\.' \
    '!TICKET_MISSING' \
    '!^  PASSED\.'

# The near miss, and the one a careful author produces: the last paragraph IS
# the trailer block and it opens with a sentence, so git parses none of it.
add_case  trap-trailers-in-prose-paragraph message msg:trap-trailers-in-prose-paragraph fail catches \
    '^      codes: TICKET_UNPARSED SPEC_UNPARSED$' \
    "GIT'S TRAILER PARSER DOES NOT SEE IT" \
    '^  FAILED\.' \
    '!TICKET_MISSING' \
    '!^  PASSED\.'

# --- the range: more than one commit, and merge commits --------------------
#
# CI_CD says EVERY commit on the pull request, not the head of it. These rows
# are the difference between a gate that reads `HEAD` and a gate that reads the
# range, and that difference is invisible on a one-commit branch, which is what
# most branches are.

add_case  range-three-clean  range  range-three-clean  pass  control \
    '^  checked 3 commit\(s\), 0 refused$' \
    'GitHub reports 3 commit\(s\) on this pull request and this gate enumerated 3; they agree' \
    '^  PASSED\.' \
    '!REFUSED'

# The head commit is clean and an earlier one is not. A gate reading only the
# tip reports this pull request green. The signatures require BOTH verdicts in
# the same table, so that a gate which checked the range and a gate which
# checked the tip cannot produce the same output.
add_case  range-defect-not-at-head  range  range-defect-not-at-head  fail  catches \
    '^  checked 2 commit\(s\), 1 refused$' \
    '^  [0-9a-f]{12} REFUSED +wip\(gates\): a type this repository does not use$' \
    '^  [0-9a-f]{12} ok +feat\(gates\): a message with everything this gate asks for$' \
    '^  FAILED\.' \
    '!^  PASSED\.'

# MERGE COMMITS, and why there is no exemption. `allow_merge_commit` is false,
# so the merge queue rebases and produces none. A merge commit INSIDE a pull
# request's range therefore means somebody merged `main` into their branch
# instead of rebasing onto it, which spec/CONVENTIONS.md "Git" forbids in as
# many words: "Rebase on `main` before ready". Failing it is the correct
# verdict, not a false positive, and this row is the half of that claim that
# can be observed.
add_case  range-merge-commit-inside  range  range-merge-commit-inside  fail  catches \
    '^  checked 3 commit\(s\), 1 refused$' \
    '^  [0-9a-f]{12} REFUSED +Merge pull request #2 from fixture/side$' \
    '^      codes: SUBJECT_SHAPE TICKET_MISSING SPEC_MISSING$' \
    '^  FAILED\.' \
    '!^  PASSED\.'

# The other half. `main` carries one merge commit, ffab3fd, from before
# rebase-only merges were configured; it has no trailers and never will,
# because CONVENTIONS forbids rewriting history. It is nevertheless never
# enumerated, and NOT because it is exempt: it is an ancestor of the base of
# every range this gate can build, and a range excludes its base. This row is
# that sentence made observable: the base here IS a merge commit with no
# trailers, and the run passes without ever reading it. `!Merge pull request`
# is the assertion, because a gate that did enumerate the base would print its
# subject in the table.
add_case  range-base-is-merge-commit  range  range-base-is-merge-commit  pass  control \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  PASSED\.' \
    '!Merge pull request' \
    '!REFUSED'

# The `Spec:` document is OBSERVED and never judged, and this row is the branch
# of that observation where the document exists. `clean-full` above is the
# branch where it does not, and BOTH pass. A future edit that turned the
# observation into an enforcement would make one of these two rows disagree.
add_case  spec-doc-present  range  spec-doc-present  pass  control \
    '^  checked 1 commit\(s\), 0 refused$' \
    'observed, not judged: spec/CI_CD\.md exists in the tree this run checked out' \
    '^  PASSED\.' \
    '!REFUSED'

# --- enumeration: the set is empty, unnameable, or from another event ------
#
# Every row from here to the end of the table carries `!^  PASSED\.`, and stage
# 3 refuses to run if one of them stops carrying it.

# THE ONE THAT WOULD HAVE HAPPENED. A depth-1 clone is what
# `actions/checkout` produces by default, and the base SHA the event payload
# names is not an object it holds. The gate refuses and names the fix; it does
# not enumerate what happens to be present.
add_case  enum-shallow-clone  enumeration  enum-shallow-clone  refuse  refuses \
    "is not in this clone\. The checkout is too shallow for this gate: give the job 'fetch-depth: 0'" \
    'Enumerating what happens to be present would check a shorter range and report on it as though it were the whole pull request' \
    '^  NOTHING WAS CHECKED\.' \
    'This is not a pass and not a failure of any commit\.' \
    '!^  PASSED\.' \
    '!^  checked '

# base.sha == head.sha. GitHub does not open a pull request with no commits, so
# an empty set here means the range was computed wrongly. The gate says so
# rather than reporting that every commit in an empty set conforms, which is
# true and useless.
add_case  enum-empty-range  enumeration  enum-empty-range  refuse  refuses \
    'the range [0-9a-f]+\.\.[0-9a-f]+ is empty, so this run would check no commit at all' \
    'a gate that checks nothing must not report success' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.' \
    '!^  checked '

# GitHub says how many commits this pull request has; the gate says which ones.
# If the two disagree, one of them is wrong about the thing the gate exists to
# cover, and neither answer may be reported as a pass. This is the row that
# catches a range that is short for a reason nobody anticipated.
add_case  enum-count-disagrees  enumeration  enum-count-disagrees  refuse  refuses \
    'GitHub reports 3 commit\(s\) on this pull request and this gate enumerated 1' \
    'it will not report a pass on a set it cannot vouch for' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.'

add_case  enum-payload-missing  enumeration  enum-payload-missing  refuse  refuses \
    'there is no readable event payload at GITHUB_EVENT_PATH' \
    'so the commits of this pull request cannot be named' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.'

add_case  enum-payload-no-shas  enumeration  enum-payload-no-shas  refuse  refuses \
    'the event payload names no pull_request\.base\.sha or no pull_request\.head\.sha' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.'

# An event nobody wrote a rule for gets a refusal. The payload handed to this
# row is a perfectly good pull_request payload, so the refusal is attributable
# to the event and not to a missing input.
add_case  enum-unknown-event  enumeration  enum-unknown-event  refuse  refuses \
    "this gate has no rule for the event 'schedule'" \
    'inventing a set here is how a gate ends up checking something other than what it reports' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.' \
    '!there is no readable event payload'

# --- push, which is what fires on `main` -----------------------------------

add_case  push-clean  enumeration  push-clean  pass  control \
    '^  commit set       : push: [0-9a-f]+\.\.[0-9a-f]+$' \
    'on a push there is no pull request, so the set is the commits this push landed on the branch' \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  PASSED\.' \
    '!REFUSED'

add_case  push-defect  enumeration  push-defect  fail  catches \
    '^  commit set       : push: [0-9a-f]+\.\.[0-9a-f]+$' \
    '^      codes: TICKET_MISSING SPEC_MISSING$' \
    '^  FAILED\.' \
    '!^  PASSED\.'

# An all-zero `before` means the ref was created by this push. CONVENTIONS
# forbids force pushing and history rewriting, so on `main` that state is
# itself the thing to look at, and the gate will not invent a range for it.
add_case  push-created-ref  enumeration  push-created-ref  refuse  refuses \
    "this push reports an all-zero 'before', which means the ref was created by this push" \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.'

add_case  push-empty-range  enumeration  push-empty-range  refuse  refuses \
    'the push range [0-9a-f]+\.\.[0-9a-f]+ is empty, so this run would check no commit' \
    'A push that moved a ref backwards or not at all is not a state this gate reports a pass on' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.'

# --- merge_group, which is spec/CI_CD.md section 2 -------------------------
#
# The queue rebases, so the commits it presents are NEW OBJECTS carrying the
# messages under review. A gate that had cached a verdict against a SHA would
# have nothing to say about them; this one reads the messages again.

add_case  merge-group-clean  enumeration  merge-group-clean  pass  control \
    '^  commit set       : merge_group: [0-9a-f]+\.\.[0-9a-f]+$' \
    'the merge queue rebases, so these are new commit objects carrying the messages under review' \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  PASSED\.' \
    '!REFUSED'

add_case  merge-group-defect  enumeration  merge-group-defect  fail  catches \
    '^  commit set       : merge_group: [0-9a-f]+\.\.[0-9a-f]+$' \
    '^      codes: TICKET_UNPARSED SPEC_UNPARSED$' \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  merge-group-empty  enumeration  merge-group-empty  refuse  refuses \
    'the merge group range [0-9a-f]+\.\.[0-9a-f]+ is empty' \
    'A merge queue entry with no commits is not a state this gate can report a pass on' \
    '^  NOTHING WAS CHECKED\.' \
    '!^  PASSED\.'

# --- workflow_dispatch, which names one commit and says so -----------------

add_case  dispatch-clean  enumeration  dispatch-clean  pass  control \
    '^  commit set       : workflow_dispatch: the single commit [0-9a-f]+$' \
    'a manual dispatch names no pull request and no range, so the set is the one commit that was checked out' \
    'This is a smaller claim than the one this gate makes on a pull request and the verdict says so' \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  PASSED\.'

add_case  dispatch-defect  enumeration  dispatch-defect  fail  catches \
    '^  commit set       : workflow_dispatch: the single commit [0-9a-f]+$' \
    '^      codes: SPEC_VALUE$' \
    '^  FAILED\.' \
    '!^  PASSED\.'

# --- off a GitHub event: a coder's desk ------------------------------------
#
# The state a coder's worktree is usually in. In this fleet the lead makes the
# commits and the coder produces the work, so a coder's branch commonly carries
# no commit of its own, and `scripts/gates.sh` reports gate 13 BLOCKED there
# rather than passed. These two rows are that sentence made observable: exit 3,
# not exit 0, and the text says "This is not a pass" in the place a reader
# looks.

add_case  local-branch-clean  enumeration  local-branch-clean  pass  control \
    '^  event            : \(none: a local run\)$' \
    '^  commit set       : local: merge-base\(main, HEAD\)=[0-9a-f]+ \.\. HEAD$' \
    "off a GitHub event the set is this branch's own commits against 'main'" \
    '^  checked 1 commit\(s\), 0 refused$' \
    '^  PASSED\.'

add_case  local-branch-defect  enumeration  local-branch-defect  fail  catches \
    '^  event            : \(none: a local run\)$' \
    '^      codes: TICKET_DUPLICATE$' \
    '^  FAILED\.' \
    '!^  PASSED\.'

add_case  local-no-commits  enumeration  local-no-commits  nothing  refuses \
    "this branch carries no commit of its own against 'main'" \
    'That is not a pass: nothing was checked' \
    '^  NOTHING WAS CHECKED\.' \
    '^  This is not a pass\. Do not read it as one\.' \
    '!^  PASSED\.' \
    '!^  checked '

add_case  local-no-base  enumeration  local-no-base  nothing  refuses \
    "no base branch resolves here: neither 'origin/main' nor 'main' names a commit in this repository" \
    '^  NOTHING WAS CHECKED\.' \
    '^  This is not a pass\. Do not read it as one\.' \
    '!^  PASSED\.'

CASE_COUNT=${#CASE_ID[@]}

OBS_VERDICT=()
OBS_STATUS=()
OBS_LOG=()
CASE_REPO=()
CASE_EVENT=()
CASE_PAYLOAD=()
CASE_SHA=()

# ---------------------------------------------------------------------------
# The sample table: the planted defects for the liveness check of stage 4.
# Each file under workflow-samples/ is a whole workflow carrying exactly one
# way of switching gate 13 off, or none. The classification this harness owes
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
# they are owed about gate 13's command is the answer they are owed about this
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
# The checkout-depth table: the planted inputs for stage 4c. Gate 13 and gate 7
# are the two gates in this repository whose reach depends on an input to an
# action rather than on a command, and the check that reads it gets the same
# treatment as every other checker here.
# ---------------------------------------------------------------------------

DEPTH_SAMPLE_FILE=()
DEPTH_SAMPLE_EXP=()

add_depth_sample() {
    DEPTH_SAMPLE_FILE+=("$1")
    DEPTH_SAMPLE_EXP+=("$2")
}

add_depth_sample  depth-full.yml        0
add_depth_sample  depth-shallow.yml     1
add_depth_sample  depth-absent.yml      absent
add_depth_sample  depth-two-values.yml  ambiguous

DEPTH_SAMPLE_COUNT=${#DEPTH_SAMPLE_FILE[@]}
DEPTH_ANSWERS='0 1 absent ambiguous'

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
        printf '::%s file=fixtures/planted/gate-13/prove.sh::%s\n' "$kind" "$*"
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
# against planted inputs and stages 4, 4b, 4c and 8b can use them against the
# real ones. One implementation, exercised before it is believed.
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
            trigger_why="      - the '$REQUIRED_TRIGGER' trigger carries the filter key(s) ${keys% }, which narrow which pull requests fire this workflow. That narrowing is a deliberate change to when gate 13 runs, so it needs a deliberate re-proof rather than a harness deciding which pull requests are allowed to skip the gate"
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

# Answer, from those same facts, how deep the checkout of a named job is. Sets
# DEPTH_STATE to the value of `fetch-depth` under a step's `with:`, or to
# `absent` when no step of that job sets it, or to `ambiguous` when the steps
# of that job disagree.
#
# WHY THIS EXISTS. It is the one thing in this workflow that decides whether
# gate 13 can name the commits a pull request carries. `actions/checkout`
# defaults to a depth of 1, and in a repository cloned that way the base and
# head SHAs the event payload names are simply not objects the clone holds.
# Gate 13 refuses on that rather than enumerating what is present, so the wrong
# value here is a red job; this check exists so the reason is named in this
# harness's output rather than met as a surprise refusal in another job's log.
#
# It does not model `actions/checkout`. Two steps setting different depths is
# `ambiguous`, not "the last one wins", because a prover that reasoned about
# one action's semantics would be reasoning about all of them.
DEPTH_STATE=''
DEPTH_WHY=''
checkout_depth() {
    # $1 facts file, $2 job name.
    local facts="$1" job="$2" values seen v
    DEPTH_STATE=''
    DEPTH_WHY=''
    if grep -q '^ERROR ' "$facts"; then
        DEPTH_STATE='unreadable'
        DEPTH_WHY="      - the workflow could not be parsed, so nothing is known about the checkout of job '$job'"
        return
    fi
    values="$(sed -n "s/^STEPWITH $job [0-9][0-9]* fetch-depth //p" "$facts")"
    if [ -z "$values" ]; then
        DEPTH_STATE='absent'
        DEPTH_WHY="      - no step of job '$job' sets 'fetch-depth', so the checkout takes actions/checkout's default of 1 and the runner gets a shallow clone"
        return
    fi
    seen=''
    while IFS= read -r v; do
        [ -n "$v" ] || continue
        case " $seen " in
            *" $v "*) ;;
            *) seen="${seen:+$seen }$v" ;;
        esac
    done <<<"$values"
    case "$seen" in
        *' '*)
            DEPTH_STATE='ambiguous'
            DEPTH_WHY="      - the steps of job '$job' set 'fetch-depth' to more than one value ($seen), so this harness cannot say how deep the checkout is without modelling actions/checkout, which it does not do"
            ;;
        *)
            DEPTH_STATE="$seen"
            DEPTH_WHY="      - job '$job' checks out with fetch-depth $seen"
            ;;
    esac
}

# Remove the colour codes a tool writes for a terminal. `scripts/gates.sh`
# colours only when its standard output is a terminal, and here it is a file,
# so today this changes nothing. It is kept because every anchored signature in
# the table above would match nothing if that ever stopped being true, and a
# harness that started disagreeing with a healthy gate over an escape sequence
# would send its reader to look at the gate. The gate command's exit status is
# captured by the caller before this runs and cannot be swallowed by it.
strip_ansi() {
    # $1 the captured output, $2 the file to write the readable form to.
    local esc
    esc="$(printf '\033')"
    sed "s/${esc}\[[0-9;]*m//g; s/${esc}(B//g" "$1" >"$2"
}

# Is what the gate command printed the complaint this case planted?
#   0 every signature matched, or none was given and the command said nothing
#   1 a signature did not match: this is not the answer this proof planted
#   2 a signature is not a usable regular expression
#
# A signature that begins with `!` must NOT match. Most rows here carry one,
# and on every `refuse` and `nothing` row the negated signature is
# `!^  PASSED\.`, which is the assertion that a run which checked no commit did
# not report success.
ATTRIB_WHY=''
output_matches() {
    # $1 file holding the captured output, $2 newline-separated signatures.
    local log="$1" sigs="$2"
    local pat status any=0 negated
    ATTRIB_WHY=''
    while IFS= read -r pat; do
        [ -n "$pat" ] || continue
        any=1
        negated=0
        case "$pat" in
            '!'*) negated=1; pat="${pat#!}" ;;
        esac
        grep -Eq -e "$pat" "$log"
        status=$?
        if [ "$status" -gt 1 ]; then
            ATTRIB_WHY="/$pat/ is not a usable extended regular expression: grep exited $status"
            return 2
        fi
        if [ "$negated" -eq 0 ] && [ "$status" -eq 1 ]; then
            ATTRIB_WHY="nothing in the output matches /$pat/"
            return 1
        fi
        if [ "$negated" -eq 1 ] && [ "$status" -eq 0 ]; then
            ATTRIB_WHY="the output matches /$pat/, which this case requires it NOT to"
            return 1
        fi
    done <<<"$sigs"
    if [ "$any" -eq 0 ] && [ -s "$log" ]; then
        ATTRIB_WHY='the command printed something on an input it has nothing to say about'
        return 1
    fi
    return 0
}

# The last paragraph of a message file, which is the only part git reads
# trailers out of. Written once here because stage 3b asserts a property of it
# for four different messages, and a copy per message is four chances to assert
# something subtly different.
last_paragraph() {
    # $1 message file. Prints the final blank-line-separated block.
    awk '
        { lines[NR] = $0 }
        END {
            start = 1
            for (i = NR; i >= 1; i--) {
                if (lines[i] ~ /^[ \t]*$/) { start = i + 1; break }
            }
            for (i = start; i <= NR; i++) {
                if (lines[i] !~ /^[ \t]*$/) print lines[i]
            }
        }
    ' "$1"
}

# ---------------------------------------------------------------------------
# Stage 1: what is running this
# ---------------------------------------------------------------------------

STAGE='reporting the environment'
rule 'Environment'
say "gate 13 proof harness, ORI-T-0017, spec/CI_CD.md section 1 item 13"
say "  repository root : $REPO_ROOT"
say "  fixture root    : $HERE"
say "  gate command    : $GATE_CMD"
say "  platform        : $(uname -s 2>/dev/null || echo unknown) $(uname -m 2>/dev/null || echo unknown)"
if [ -n "${GITHUB_ACTIONS:-}" ]; then
    say "  running under   : GitHub Actions, runner OS ${RUNNER_OS:-unknown}, event ${GITHUB_EVENT_NAME:-unknown}"
else
    say "  running under   : a local shell"
fi

# ---------------------------------------------------------------------------
# Stage 2: the tooling. A missing tool makes gate 13 unrunnable, and a harness
# that cannot run the gate must say so rather than report a pass.
# scripts/gates.sh carries the same distinction and the same reason.
#
# jq is on this list because gate 13's enumeration reads the event payload with
# it on three of its four events. Without jq the gate refuses, correctly, on
# every one of those rows, and this harness would then be recording a fact
# about the machine while reporting it as a fact about the gate.
#
# cargo is NOT on this list. `--commit-trailers` runs one gate and that gate
# reads git and a workflow file; a Rust toolchain has nothing to do with it,
# and probing for one would make this proof unrunnable for a reason unrelated
# to what it proves.
# ---------------------------------------------------------------------------

STAGE='probing the tooling'
rule 'Tooling'
probe_failed=0
probe() {
    local label="$1"
    shift
    local out status
    out="$("$@" 2>&1)"
    status=$?
    if [ "$status" -ne 0 ]; then
        emit error "'$*' exited $status, so $label is not usable here and no verdict of gate 13 on this machine would mean anything"
        probe_failed=1
        return
    fi
    printf '  %-16s %s\n' "$label" "${out%%$'\n'*}"
}
probe 'git'   git --version
probe 'jq'    jq --version
# awk reads the workflow file. Probed by running a program rather than by
# asking for a version banner, because the three awks this has to run under
# disagree about which version flag they take and agree about this.
probe 'awk'   awk 'BEGIN { print "a usable POSIX awk" }'
# The gate itself. `--help` is the one invocation of it that says in its own
# output that it checked no code, so probing with it cannot be mistaken for
# running the gate.
if [ ! -f "$GATES_SH" ]; then
    emit error "$GATES_SH_REL does not exist, so the gate this script proves has no implementation to run and nothing below would mean anything"
    probe_failed=1
else
    probe 'gates.sh' bash "$GATES_SH" --help
fi
if [ "$probe_failed" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "nothing was checked: the tooling this proof needs is not usable on this machine. This is not a pass and not a failure of gate 13."
    exit 3
fi

WORK_DIR="$(mktemp -d)"
if [ ! -d "$WORK_DIR" ]; then
    VERDICT_REACHED=1
    emit error "could not create a temporary directory to hold the planted repositories and the gate output, so nothing was run"
    exit 3
fi

# ---------------------------------------------------------------------------
# Stage 3: assertions about the fixture. Every one of these is a way for this
# proof to be quietly meaningless, so each is checked rather than assumed.
# ---------------------------------------------------------------------------

STAGE='checking the fixture'
rule 'Preconditions'
bad=0

note_bad() { emit error "$*"; bad=1; }

if [ "$CASE_COUNT" -eq 0 ]; then
    note_bad "the case table is empty, so this harness would run no gate command and then report success"
fi

# All four verdicts must be represented, and so must all three families. A
# table of failures only cannot detect a gate that fails on everything; a table
# of passes only cannot detect a gate that passes on everything, which is the
# AICD §14 defect class itself; a table with no `refuse` or `nothing` row says
# nothing about the half of this gate that decides which commits exist; and a
# family that went missing is a family nothing watches.
expect_pass=0
expect_fail=0
expect_refuse=0
expect_nothing=0
have_message=0
have_range=0
have_enumeration=0
missing_negative=''
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    case "${CASE_EXP[$i]}" in
        pass) expect_pass=$((expect_pass + 1)) ;;
        fail|refuse|nothing)
            case "${CASE_EXP[$i]}" in
                fail)    expect_fail=$((expect_fail + 1)) ;;
                refuse)  expect_refuse=$((expect_refuse + 1)) ;;
                nothing) expect_nothing=$((expect_nothing + 1)) ;;
            esac
            # A row that expects a non-zero answer and names no signature is a
            # row that would accept any non-zero answer at all, which is the
            # hole this stage exists to close. Gate 13 has two different
            # non-zero answers for two different reasons, so this matters here
            # more than it did for gate 7.
            if [ -z "${CASE_SIG[$i]//$'\n'/}" ]; then
                note_bad "case ${CASE_ID[$i]} expects gate 13 to answer '${CASE_EXP[$i]}' and names no signature, so any non-zero answer whatever would be recorded as the planted defect being caught"
            fi
            ;;
        *) note_bad "case ${CASE_ID[$i]} expects '${CASE_EXP[$i]}', which is none of 'pass', 'fail', 'refuse', 'nothing'" ;;
    esac
    # THE ASSERTION THIS WHOLE FIXTURE TURNS ON. Every row whose owed answer is
    # "I checked no commit" must carry, as a required negative signature, the
    # statement that the gate did not print PASSED. Without it those rows would
    # rest on the exit status alone, and one weakened line in scripts/gates.sh
    # could change the exit status of a run that checked nothing.
    case "${CASE_EXP[$i]}" in
        refuse|nothing)
            case "${CASE_SIG[$i]}" in
                *'!^  PASSED\.'*) ;;
                *) note_bad "case ${CASE_ID[$i]} owes the answer '${CASE_EXP[$i]}', which means no commit was checked, and it does not require the output NOT to say PASSED. That negative is the assertion this fixture exists for; a row without it rests on an exit status alone." ;;
            esac
            ;;
    esac
    case "${CASE_FAMILY[$i]}" in
        message)     have_message=$((have_message + 1)) ;;
        range)       have_range=$((have_range + 1)) ;;
        enumeration) have_enumeration=$((have_enumeration + 1)) ;;
        *) note_bad "case ${CASE_ID[$i]} belongs to family '${CASE_FAMILY[$i]}', which is none of message, range, enumeration" ;;
    esac
    i=$((i + 1))
done
say "  case table       : $CASE_COUNT cases, $expect_pass pass, $expect_fail fail, $expect_refuse refuse, $expect_nothing nothing"
say "  by family        : message $have_message, range $have_range, enumeration $have_enumeration"
if [ "$expect_pass" -eq 0 ]; then
    note_bad "no case expects gate 13 to pass, so this run could not tell a working gate from one that refuses every input"
fi
if [ "$expect_fail" -eq 0 ]; then
    note_bad "no case expects gate 13 to fail, so this run could not tell a working gate from one that passes on every input, which is the defect class of AICD §14"
fi
if [ "$expect_refuse" -eq 0 ] || [ "$expect_nothing" -eq 0 ]; then
    note_bad "the table has no 'refuse' row or no 'nothing' row (refuse $expect_refuse, nothing $expect_nothing). Those are the rows where the gate checked no commit, and a gate that reported success there is the whole reason this fixture exists."
fi
if [ "$have_message" -eq 0 ] || [ "$have_range" -eq 0 ] || [ "$have_enumeration" -eq 0 ]; then
    note_bad "the table does not exercise all three families (message $have_message, range $have_range, enumeration $have_enumeration). A gate 13 that read messages correctly and enumerated nothing would agree with every message row."
fi

# Every planted message must be named by the table, and every message the table
# names must exist. An unexercised planted message is one nothing watches, and
# the row that goes missing is the row nobody misses.
for f in "$MESSAGE_DIR"/*.txt; do
    if [ ! -f "$f" ]; then
        note_bad "messages/ holds no .txt file at all, so the inputs this proof claims to present to gate 13 are not there"
        break
    fi
    stem="$(basename -- "$f" .txt)"
    found=0
    i=0
    while [ "$i" -lt "$CASE_COUNT" ]; do
        case "${CASE_SETUP[$i]}" in
            "msg:$stem") found=1; break ;;
        esac
        i=$((i + 1))
    done
    if [ "$found" -eq 0 ]; then
        note_bad "messages/$stem.txt is planted and no row of the case table presents it to gate 13, so nothing watches whatever it was planted to catch"
    fi
done
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    case "${CASE_SETUP[$i]}" in
        msg:*)
            stem="${CASE_SETUP[$i]#msg:}"
            if [ ! -f "$MESSAGE_DIR/$stem.txt" ]; then
                note_bad "case ${CASE_ID[$i]} presents messages/$stem.txt and that file is not there"
            fi
            ;;
    esac
    i=$((i + 1))
done

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
i=0
while [ "$i" -lt "$DEPTH_SAMPLE_COUNT" ]; do
    if [ ! -f "$SAMPLE_DIR/${DEPTH_SAMPLE_FILE[$i]}" ]; then
        note_bad "workflow-samples/${DEPTH_SAMPLE_FILE[$i]} is missing, so the check that this harness can read the depth of a job's checkout has lost one of its inputs"
    fi
    i=$((i + 1))
done

# The local gate set must run this gate too, and this is the check that says
# so. CLAUDE.md step 4 makes `scripts/gates.sh` the gate set a coder runs
# before opening a pull request. A gate that CI runs and the local gate set
# declines to run is a gate a coder cannot run, whatever this proof says about
# CI. ORI-T-0016 found gate 7 in exactly that state with three hard-coded
# reasons, two of them already falsified.
if ! grep -q "GATE_RUNNER\[13\]='local'" "$GATES_SH"; then
    note_bad "$GATES_SH_REL does not declare a local runner for gate 13, so gate 13 would be unrunnable at a desk while this file proves it in CI"
fi
if grep -q 'probe_unavailable 13' "$GATES_SH"; then
    note_bad "$GATES_SH_REL still probes gate 13 for availability instead of running it. That probe's reasons are hard-coded sentences, and this ticket falsified them."
fi
if ! grep -q -- '--commit-trailers' "$GATES_SH"; then
    note_bad "$GATES_SH_REL does not accept --commit-trailers, which is the form CI runs and the form this harness presents planted repositories to"
fi
if [ -f "$HERE/gates.sh" ]; then
    note_bad "a second gates.sh is back under fixtures/planted/gate-13/. Ruling R28 puts the checker beside the other scripts a gate runs; a gate whose implementation has two copies is a gate that can be repaired in the copy CI does not run."
fi
say "  local gate set   : $GATES_SH_REL declares a local runner for gate 13 and accepts --commit-trailers"

# The root workspace must not reach into the fixture. A member under
# `fixtures/` would put a planted defect into the real build.
if [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
    note_bad "$REPO_ROOT/Cargo.toml does not exist, so REPO_ROOT was resolved wrongly and nothing below can be trusted"
elif grep -q 'fixtures' "$REPO_ROOT/Cargo.toml"; then
    note_bad "the root Cargo.toml mentions 'fixtures'. If a planted package has become a workspace member the real build no longer stays green, and this proof is no longer proving what it says."
else
    say "  root workspace   : names no member under fixtures/"
fi

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

# --- stage 3b: the plants are asserted, not assumed -------------------------
#
# Eleven of the rows are planted to be PASSED, and a pass looks the same
# whether the plant is still there or not. ORI-T-0014 paid this lesson in gate
# 2's fixture. The rows planted to FAIL need it just as much here, because the
# defect in a commit message is one character in a file nobody re-reads: a
# `Ticket:` line that drifted into the last paragraph would turn the trap into
# an ordinary clean message, the row would report `pass` where `fail` is owed,
# and the only visible symptom would be this harness blaming the gate.
#
# Every assertion below is about the message source, not about what a tool
# said.

STAGE='checking that every planted input is still planted'
rule 'The plants are still planted'
plant_bad=0
plant_check() {
    # $1 description, $2 result (0 good), $3 what a failure means.
    local desc="$1" ok="$2" why="$3"
    if [ "$ok" -eq 0 ]; then
        printf '  %-60s %s\n' "$desc" 'yes'
    else
        printf '  %-60s %s\n' "$desc" 'NO'
        emit error "$why"
        plant_bad=1
    fi
}

# THE CENTRAL PLANT. The trap is only the trap while both required lines sit in
# a paragraph that is NOT the last one. Three properties, asserted separately,
# because each of them alone would let a repaired message through:
#   the lines are there, at the start of their lines, correctly spelled;
#   the last paragraph does not contain them;
#   the last paragraph is not empty, so there IS a later paragraph and the
#   message is not simply one that ends with its trailers.
TRAP_FILE="$MESSAGE_DIR/$TRAP_ID.txt"
grep -qE '^Ticket: ORI-T-0017$' "$TRAP_FILE" && grep -qE '^Spec: [A-Za-z0-9_-]+\.md#' "$TRAP_FILE"
plant_check "$TRAP_ID carries a correctly spelled Ticket: and Spec: line" $? \
    "messages/$TRAP_ID.txt no longer carries a correctly spelled 'Ticket:' and 'Spec:' line at the start of a line. The whole point of that message is that a regex finds both and git finds neither; without the lines there is no contrast to draw and stage 7 would be demonstrating nothing."

last_paragraph "$TRAP_FILE" >"$WORK_DIR/trap.lastpara"
! grep -qE '^(Ticket|Spec):' "$WORK_DIR/trap.lastpara"
plant_check "$TRAP_ID keeps them OUT of the last paragraph" $? \
    "messages/$TRAP_ID.txt now has its 'Ticket:' or 'Spec:' line in the LAST paragraph, which is the only paragraph git reads trailers out of. The message has been repaired into an ordinary conforming one, so the row that owes 'fail' would observe 'pass' and this harness would report that gate 13 stopped catching something it never had to catch."

grep -qE '^Co-Authored-By: ' "$WORK_DIR/trap.lastpara"
plant_check "$TRAP_ID ends with a Co-Authored-By paragraph of its own" $? \
    "messages/$TRAP_ID.txt no longer ends with a 'Co-Authored-By:' paragraph. That trailing paragraph is what makes the required trailers stop being the last one, and it is also the exact shape the lead shipped. Without it the message is not the planted defect."

# The control for the trap: the same trailers, in the last paragraph, where git
# reads them. If this one stopped parsing, every UNPARSED row would still say
# UNPARSED and nothing would say the gate can read a trailer at all.
last_paragraph "$MESSAGE_DIR/clean-full.txt" >"$WORK_DIR/cleanfull.lastpara"
grep -qE '^Ticket: ORI-T-0017$' "$WORK_DIR/cleanfull.lastpara" && \
    grep -qE '^Spec: [A-Za-z0-9_-]+\.md#[A-Za-z0-9]' "$WORK_DIR/cleanfull.lastpara"
plant_check 'clean-full keeps its trailers IN the last paragraph' $? \
    "messages/clean-full.txt no longer has both required trailers in its last paragraph, so the one row that shows gate 13 can read a trailer at all is no longer showing it. Every 'the parser did not see it' row would then be agreeing with a parser that sees nothing anywhere."

grep -qE '^Co-Authored-By: ' "$WORK_DIR/cleanfull.lastpara"
plant_check 'clean-full puts Co-Authored-By beside them, not after' $? \
    "messages/clean-full.txt no longer carries 'Co-Authored-By:' in the same paragraph as its required trailers. That is the repair the gate's own refusal text tells an author to make, and this row is the only place it is shown to work."

# The remaining plants, one per defect class. Each is the property the row's
# signature was written against.
grep -qE '^feat\(store\)!: ' "$MESSAGE_DIR/clean-breaking.txt"
plant_check 'clean-breaking still carries the breaking-change marker' $? \
    "messages/clean-breaking.txt no longer has '!' before the colon, so the row that shows the gate accepts the Conventional Commits breaking-change marker is not testing it."

! grep -qEi '^(Ticket|Spec):' "$MESSAGE_DIR/no-trailers.txt"
plant_check 'no-trailers really has no trailer-shaped line anywhere' $? \
    "messages/no-trailers.txt now contains a Ticket- or Spec-shaped line. Its whole job is to be the message where the refusal must say MISSING and not UNPARSED, and those are different repairs."

[ "$(grep -cE '^Ticket: ' "$MESSAGE_DIR/ticket-duplicate.txt")" -eq 2 ]
plant_check 'ticket-duplicate carries exactly two Ticket: lines' $? \
    "messages/ticket-duplicate.txt no longer carries exactly two 'Ticket:' lines, so the row that shows the gate refuses an ambiguous value has nothing ambiguous in it."

grep -qE '^ticket: ' "$MESSAGE_DIR/ticket-lowercase-key.txt"
plant_check 'ticket-lowercase-key carries a lowercase key' $? \
    "messages/ticket-lowercase-key.txt no longer carries a lowercase 'ticket:' key. That row is the one trailer in this fixture that git DOES consume, and the gate refuses it anyway; without the lowercase key it demonstrates nothing."

grep -qE '^[ \t]+and a continuation line' "$MESSAGE_DIR/ticket-folded-value.txt"
plant_check 'ticket-folded-value carries an indented continuation line' $? \
    "messages/ticket-folded-value.txt no longer carries an indented continuation line under its 'Ticket:' trailer, so the row that shows git folds it into the value has nothing folded in it."

grep -qE '^Ticket: T-17$' "$MESSAGE_DIR/ticket-value-bad.txt"
plant_check 'ticket-value-bad carries a value that is not a ticket id' $? \
    "messages/ticket-value-bad.txt no longer carries 'Ticket: T-17', and the signature of that row quotes the value back out of the gate's refusal."

grep -qE '^Spec: CI_CD\.md$' "$MESSAGE_DIR/spec-value-no-anchor.txt"
plant_check 'spec-value-no-anchor cites a document with no section' $? \
    "messages/spec-value-no-anchor.txt no longer carries 'Spec: CI_CD.md' with no anchor."

grep -qE '^Spec: CI_CD\.md#$' "$MESSAGE_DIR/spec-value-empty-anchor.txt"
plant_check 'spec-value-empty-anchor cites a separator and nothing after it' $? \
    "messages/spec-value-empty-anchor.txt no longer carries 'Spec: CI_CD.md#' with an empty anchor, which is the value that reads as an anchor to a regex counting characters and as no anchor at all to a reader."

head -1 "$MESSAGE_DIR/subject-unknown-type.txt" | grep -qE '^wip\(gates\): '
plant_check 'subject-unknown-type still uses a type outside the vocabulary' $? \
    "messages/subject-unknown-type.txt no longer opens with 'wip(gates): '."

head -1 "$MESSAGE_DIR/subject-uppercase-scope.txt" | grep -qE '^feat\(Gates\): '
plant_check 'subject-uppercase-scope still uses a scope outside [a-z0-9-]' $? \
    "messages/subject-uppercase-scope.txt no longer opens with 'feat(Gates): '."

# The vocabulary the gate enforces is a repository decision, and one row's
# signature quotes it back verbatim out of the refusal. If the decision changes
# the signature must change with it, in the same commit, rather than this row
# becoming unattributable.
grep -qxF "CT_TYPES='$EXPECTED_TYPES'" "$GATES_SH"
plant_check "scripts/gates.sh still declares CT_TYPES='$EXPECTED_TYPES'" $? \
    "$GATES_SH_REL no longer declares CT_TYPES='$EXPECTED_TYPES'. That list is a deliberate repository decision and the subject-unknown-type row quotes it out of the gate's own refusal text, so the two must change together. Update the signature in this file in the same commit as the vocabulary."

say ''
if [ "$plant_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "at least one planted input is no longer planted, so the rows that depend on it would agree with the case table while demonstrating nothing. Repair the fixture and re-run. Nothing was proven."
    exit 2
fi
say '  Every planted input above is still what it was planted to be. Eleven of the'
say '  rows below are passes, and a pass looks identical whether its plant is'
say '  present or not; the three assertions about the trap message are the only'
say '  thing standing between the central row of this fixture and a message that'
say '  was quietly repaired into an ordinary one.'

# --- stage 3c: the self-check of the liveness check ------------------------
#
# The same argument as the flipped comparator. A liveness check that answered
# "live" to everything would report the gate proved however the workflow was
# weakened; one that answered "dead" to everything would block every run. The
# table below contains all three answers, so neither constant survives it.

STAGE='checking that this harness can tell a live gate command from a dead one'
rule 'Self-check: telling a live gate command from a dead one'
say 'Each file below is a whole workflow carrying exactly one way of switching'
say 'gate 13 off, or none. They are planted defects for the check stage 4 is'
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
    classify_command "$facts" "$GATE_CMD" "$AGGREGATE_JOB"
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

# --- stage 3d: the self-check of the self-identification check --------------

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

# --- stage 3e: the self-check of the checkout-depth check -------------------

STAGE='checking that this harness can read the depth of a job checkout'
rule 'Self-check: reading how much history the checkout asks for'
say "Gate 13's reach depends on an input to an action rather than on a command:"
say 'the base and head commits a pull request names are objects a shallow clone'
say 'does not hold. Every answer the reader can give has a planted workflow here'
say 'that is owed it.'
say ''

depth_answers_seen="$(printf '%s\n' "${DEPTH_SAMPLE_EXP[@]}" | LC_ALL=C sort -u | tr '\n' ' ')"
depth_answers_seen="${depth_answers_seen% }"
if [ "$depth_answers_seen" != "$DEPTH_ANSWERS" ]; then
    VERDICT_REACHED=1
    emit error "the checkout-depth table is owed the answers [$depth_answers_seen] and the check can give [$DEPTH_ANSWERS]. A table that does not ask for every answer is a table a check with a constant answer could still agree with."
    exit 2
fi

printf '  %-40s %-11s %-11s %s\n' 'planted workflow' 'owed' 'answered' 'result'
printf '  %-40s %-11s %-11s %s\n' '----------------------------------------' '-----------' '-----------' '--------'
depth_sample_bad=0
i=0
while [ "$i" -lt "$DEPTH_SAMPLE_COUNT" ]; do
    sample="${DEPTH_SAMPLE_FILE[$i]}"
    facts="$WORK_DIR/depthsample.$i.facts"
    parse_workflow "$SAMPLE_DIR/$sample" "$facts"
    if [ $? -eq 2 ]; then
        VERDICT_REACHED=1
        emit error "awk could not read workflow-samples/$sample at all, so this harness cannot show that it reads a checkout's depth"
        exit 2
    fi
    checkout_depth "$facts" "$GATE_JOB"
    if [ "$DEPTH_STATE" = "${DEPTH_SAMPLE_EXP[$i]}" ]; then
        outcome='agree'
    else
        outcome='DISAGREE'
        depth_sample_bad=$((depth_sample_bad + 1))
    fi
    printf '  %-40s %-11s %-11s %s\n' "$sample" "${DEPTH_SAMPLE_EXP[$i]}" "$DEPTH_STATE" "$outcome"
    if [ "$outcome" = 'DISAGREE' ]; then
        say "$DEPTH_WHY"
    fi
    i=$((i + 1))
done
say ''
say "  disagreements: $depth_sample_bad of $DEPTH_SAMPLE_COUNT (this must be 0)"
if [ "$depth_sample_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot read how much history a job's checkout asks for: $depth_sample_bad of the $DEPTH_SAMPLE_COUNT planted workflows were read wrongly. Until that is repaired, anything stage 4c says about the real workflow means nothing."
    exit 2
fi

# --- stage 3f: the self-check of the attribution check ----------------------
#
# One text, four patterns, four different answers owed: one the text carries,
# one it does not, one negated signature that must be read as satisfied BECAUSE
# the text does not carry it, and one negated signature the text violates. A
# matcher that always agreed, or always refused, fails two of the four. The
# negated rows are here because nearly every row in this fixture turns on what
# the output does NOT say, and on eleven of them the negated signature is the
# only thing separating "the gate refused to enumerate" from "the gate reported
# a pass over nothing".

STAGE='checking that this harness can tell one answer from another'
rule 'Self-check: telling the planted answer from any other answer'
selfcheck_log="$WORK_DIR/attribution.selfcheck"
printf '%s\n' '      codes: TICKET_UNPARSED SPEC_UNPARSED' >"$selfcheck_log"
say "  text        : $(cat "$selfcheck_log")"

output_matches "$selfcheck_log" $'^      codes: TICKET_UNPARSED SPEC_UNPARSED$\n'
present=$?
output_matches "$selfcheck_log" $'^      codes: TICKET_MISSING$\n'
absent=$?
output_matches "$selfcheck_log" $'!^  PASSED\\.\n'
negated_ok=$?
output_matches "$selfcheck_log" $'!TICKET_UNPARSED\n'
negated_violated=$?
printf '  %-52s %s\n' 'a signature the text carries' "match=$present (0 owed)"
printf '  %-52s %s\n' 'a signature the text does not carry' "match=$absent (1 owed)"
printf '  %-52s %s\n' 'a NOT-signature the text satisfies' "match=$negated_ok (0 owed)"
printf '  %-52s %s\n' 'a NOT-signature the text violates' "match=$negated_violated (1 owed)"
if [ "$present" -ne 0 ] || [ "$absent" -ne 1 ] || [ "$negated_ok" -ne 0 ] || [ "$negated_violated" -ne 1 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot tell one answer from another: it answered $present, $absent, $negated_ok, $negated_violated where 0, 1, 0, 1 are owed. Every attribution below would be meaningless, and the '!^  PASSED\\.' assertion that eleven rows rest on would be vacuous."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4: the command under proof must be a command CI runs, in a job whose
# failure fails the run. Without this, gate 13 could be switched off in ci.yml
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
say "  aggregate job    : '$AGGREGATE_JOB', which ops/gates/branch-protection.md names as the first required check to be added and which is not required yet"
say "  trigger required : $REQUIRED_TRIGGER"
say "  triggers found   : $(sed -n 's/^TRIGGER //p' "$workflow_facts" | tr '\n' ' ')"
say ''
classify_command "$workflow_facts" "$GATE_CMD" "$AGGREGATE_JOB"
say "  '$GATE_CMD'"
say "    $LIVENESS"
say "$LIVENESS_WHY"
case "$LIVENESS" in
    live) ;;
    unreadable)
        note_bad "$WORKFLOW_REL has a shape this harness does not read, so it cannot say whether '$GATE_CMD' is a command CI runs. A job it failed to see is a job it would have said nothing about, so it proves nothing rather than proving something about the part it understood."
        ;;
    *)
        note_bad "'$GATE_CMD' is not run by any job of $WORKFLOW_REL whose failure would fail this run. Either gate 13 was weakened, or this harness is proving a command nothing executes. A proof of a command nothing runs is the defect class of AICD §39, so it is refused rather than reported."
        ;;
esac

if [ "$bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "the command under proof is not one CI runs where its failure fails the run, so no gate command was run here. Nothing was proven."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4b: and the job that runs THIS script must be live by the same six
# conditions.
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
            emit error "no job of $WORKFLOW_REL runs this harness ('$SELF_CMD'), so nothing in CI presents these planted defects to gate 13. A proof CI does not run is the defect class of AICD §39. Nothing was proven."
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
# Stage 4c: and the job that runs gate 13 must check out a history to read.
# ---------------------------------------------------------------------------

STAGE='checking that the gate job checks out a history to read'
rule 'The gate job checks out the whole history'
checkout_depth "$workflow_facts" "$GATE_JOB"
say "  job '$GATE_JOB' fetch-depth: $DEPTH_STATE (required: $REQUIRED_FETCH_DEPTH)"
say "$DEPTH_WHY"
if [ "$DEPTH_STATE" != "$REQUIRED_FETCH_DEPTH" ]; then
    VERDICT_REACHED=1
    emit error "the '$GATE_JOB' job of $WORKFLOW_REL checks out with fetch-depth '$DEPTH_STATE' and gate 13 needs '$REQUIRED_FETCH_DEPTH'. actions/checkout's default of 1 puts one commit on the runner, and the base and head SHAs a pull_request payload names are then not objects the clone holds. Gate 13 refuses on that rather than enumerating what is present, so this would be a red job rather than a silent pass, but it would be a red job for a reason nobody had written down. Nothing was proven."
    exit 2
fi
say ''
say "  That is what puts the commits of a pull request on the runner. It is also the"
say "  only reason the 'enum-shallow-clone' row below is a planted defect rather than"
say "  a description of how this gate runs in CI every day."

# ---------------------------------------------------------------------------
# Stage 5: building the planted repositories.
#
# Every row gets its own throwaway repository in a temporary directory that
# goes away with this run, and its own event payload. Nothing here is a commit
# in this repository: a fixture that planted a defective commit message in the
# tree would be a fixture that made the repository fail its own gate, and
# CONVENTIONS forbids rewriting history to repair one.
#
# WHY THE EVENT IS SIMULATED RATHER THAN INHERITED. Gate 13's enumeration is
# half the gate, and the only way to present it with a pull request whose
# commits are known is to write the payload. `--repo` changes only WHICH
# repository git reads, never HOW the commit set is computed, so the
# enumeration every row below exercises is the enumeration CI uses. Inheriting
# this run's own event would be worse than useless: under GitHub Actions the
# real payload names SHAs that exist in this repository and in none of the
# planted ones, so every row would refuse, for a reason that is a fact about
# the runner rather than about the gate.
# ---------------------------------------------------------------------------

STAGE='building the planted repositories'
rule 'Building the planted repositories'

REPO_DIR="$WORK_DIR/repos"
PAYLOAD_DIR="$WORK_DIR/payloads"
mkdir -p "$REPO_DIR" "$PAYLOAD_DIR"

# The message every base commit carries. A range excludes its base, so nothing
# here ever reads it; it is written conforming anyway, so that a reader who
# goes looking finds a message this project would accept rather than one that
# raises a question the fixture does not answer.
BASE_MSG="$WORK_DIR/base-message.txt"
cat >"$BASE_MSG" <<'BASEMSG'
chore(fixture): the base commit every planted range excludes

Ticket: ORI-T-0017
Spec: CI_CD.md#1-pipeline-on-every-pull-request
BASEMSG

git_quiet() {
    # A git command whose output belongs in this harness's log only when it
    # fails. The status is returned; nothing is piped.
    local out status
    out="$("$@" 2>&1)"
    status=$?
    if [ "$status" -ne 0 ]; then
        say "    git failed: $*"
        printf '%s\n' "$out" | sed 's/^/      | /'
    fi
    return "$status"
}

# A repository with one base commit on a branch called `main`. The branch name
# is set with symbolic-ref rather than `git init -b`, because the local-run
# rows below look for `origin/main` and then `main`, and `init.defaultBranch`
# is a per-machine setting this fixture will not depend on.
new_repo() {
    local name="$1" d="$REPO_DIR/$1"
    mkdir -p "$d" || return 1
    git_quiet git -C "$d" init -q || return 1
    git_quiet git -C "$d" symbolic-ref HEAD refs/heads/main || return 1
    git_quiet git -C "$d" config user.email 'gate-13@example.invalid' || return 1
    git_quiet git -C "$d" config user.name 'gate 13 fixture' || return 1
    git_quiet git -C "$d" config commit.gpgsign false || return 1
    printf 'A planted repository for gate 13 (ORI-T-0017).\n' >"$d/README.md" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -F "$BASE_MSG" || return 1
    return 0
}

COMMIT_N=0
commit_msg() {
    # $1 repository, $2 message file. Every commit touches a file of its own,
    # so that a merge of two planted branches never conflicts.
    local d="$1" msg="$2"
    COMMIT_N=$((COMMIT_N + 1))
    printf 'change %s\n' "$COMMIT_N" >"$d/change-$COMMIT_N.txt" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -F "$msg" || return 1
    return 0
}

at() { git -C "$1" rev-parse HEAD 2>/dev/null; }

pr_payload() {
    # $1 out, $2 base sha, $3 head sha, $4 the commit count GitHub reports.
    printf '{"pull_request":{"base":{"sha":"%s"},"head":{"sha":"%s"},"commits":%s}}\n' \
        "$2" "$3" "$4" >"$1"
}
push_payload() {
    printf '{"before":"%s","after":"%s"}\n' "$2" "$3" >"$1"
}
mg_payload() {
    printf '{"merge_group":{"base_sha":"%s","head_sha":"%s"}}\n' "$2" "$3" >"$1"
}

set_input() {
    # $1 index, $2 repository, $3 event name (empty for a local run),
    # $4 payload path (empty when the event needs none), $5 GITHUB_SHA.
    CASE_REPO[$1]="$2"
    CASE_EVENT[$1]="$3"
    CASE_PAYLOAD[$1]="$4"
    CASE_SHA[$1]="$5"
}

ZERO_SHA='0000000000000000000000000000000000000000'

build_one() {
    local i="$1"
    local setup="${CASE_SETUP[$i]}" id="${CASE_ID[$i]}"
    local d="$REPO_DIR/$id" p="$PAYLOAD_DIR/$id.json"
    local stem base head mid

    case "$setup" in
        msg:*)
            stem="${setup#msg:}"
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/$stem.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 1
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        range-three-clean)
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-no-body.txt" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-no-scope.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 3
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        range-defect-not-at-head)
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/subject-unknown-type.txt" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 2
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        range-merge-commit-inside)
            new_repo "$id" || return 1
            base="$(at "$d")"
            git_quiet git -C "$d" checkout -q -b side || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-no-body.txt" || return 1
            git_quiet git -C "$d" checkout -q main || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            git_quiet git -C "$d" merge -q --no-ff -m 'Merge pull request #2 from fixture/side' side || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 3
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        range-base-is-merge-commit)
            new_repo "$id" || return 1
            git_quiet git -C "$d" checkout -q -b side || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-no-body.txt" || return 1
            git_quiet git -C "$d" checkout -q main || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-no-scope.txt" || return 1
            git_quiet git -C "$d" merge -q --no-ff -m 'Merge pull request #1 from fixture/side' side || return 1
            mid="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$mid" "$head" 1
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        spec-doc-present)
            # The gate looks for the cited document in the tree it was handed,
            # so the document goes in a commit the range excludes. What is
            # under review is the message; what is observed is the tree.
            new_repo "$id" || return 1
            mkdir -p "$d/spec" || return 1
            printf '# CI_CD (a stand-in, so that the cited document exists here)\n' >"$d/spec/CI_CD.md" || return 1
            git_quiet git -C "$d" add -A || return 1
            git_quiet git -C "$d" commit -q -F "$BASE_MSG" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 1
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        enum-shallow-clone)
            # `file://` is required: git ignores --depth for a local path
            # clone, and a clone that is not shallow would present this row
            # with an ordinary repository and pass for the wrong reason.
            new_repo "$id-source" || return 1
            base="$(at "$REPO_DIR/$id-source")"
            commit_msg "$REPO_DIR/$id-source" "$MESSAGE_DIR/clean-full.txt" || return 1
            commit_msg "$REPO_DIR/$id-source" "$MESSAGE_DIR/clean-no-body.txt" || return 1
            head="$(at "$REPO_DIR/$id-source")"
            git_quiet git clone -q --depth 1 "file://$REPO_DIR/$id-source" "$d" || return 1
            if [ "$(git -C "$d" rev-parse --is-shallow-repository 2>/dev/null)" != 'true' ]; then
                say "    the clone built to be shallow is not shallow; this git may ignore --depth for a file:// clone"
                return 1
            fi
            pr_payload "$p" "$base" "$head" 2
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        enum-empty-range)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$head" "$head" 1
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        enum-count-disagrees)
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 3
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        enum-payload-missing)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            # The payload is deliberately not written.
            set_input "$i" "$d" pull_request "$PAYLOAD_DIR/$id-never-written.json" ''
            ;;

        enum-payload-no-shas)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            printf '{}\n' >"$p"
            set_input "$i" "$d" pull_request "$p" ''
            ;;

        enum-unknown-event)
            # A perfectly good pull_request payload, handed to an event this
            # gate has no rule for. The refusal is then attributable to the
            # event and not to a missing input, which is what the negative
            # signature on that row asserts.
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            pr_payload "$p" "$base" "$head" 1
            set_input "$i" "$d" schedule "$p" ''
            ;;

        push-clean)
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            push_payload "$p" "$base" "$head"
            set_input "$i" "$d" push "$p" ''
            ;;

        push-defect)
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/no-trailers.txt" || return 1
            head="$(at "$d")"
            push_payload "$p" "$base" "$head"
            set_input "$i" "$d" push "$p" ''
            ;;

        push-created-ref)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            push_payload "$p" "$ZERO_SHA" "$head"
            set_input "$i" "$d" push "$p" ''
            ;;

        push-empty-range)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            push_payload "$p" "$head" "$head"
            set_input "$i" "$d" push "$p" ''
            ;;

        merge-group-clean)
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            mg_payload "$p" "$base" "$head"
            set_input "$i" "$d" merge_group "$p" ''
            ;;

        merge-group-defect)
            # The trap message, presented to the merge queue. The queue
            # rebases, so the commit it carries is a new object with the same
            # message, and a gate that had cached a verdict against a SHA would
            # have nothing to say about it.
            new_repo "$id" || return 1
            base="$(at "$d")"
            commit_msg "$d" "$MESSAGE_DIR/$TRAP_ID.txt" || return 1
            head="$(at "$d")"
            mg_payload "$p" "$base" "$head"
            set_input "$i" "$d" merge_group "$p" ''
            ;;

        merge-group-empty)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            mg_payload "$p" "$head" "$head"
            set_input "$i" "$d" merge_group "$p" ''
            ;;

        dispatch-clean)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            head="$(at "$d")"
            set_input "$i" "$d" workflow_dispatch '' "$head"
            ;;

        dispatch-defect)
            new_repo "$id" || return 1
            commit_msg "$d" "$MESSAGE_DIR/spec-value-no-anchor.txt" || return 1
            head="$(at "$d")"
            set_input "$i" "$d" workflow_dispatch '' "$head"
            ;;

        local-branch-clean)
            new_repo "$id" || return 1
            git_quiet git -C "$d" checkout -q -b work || return 1
            commit_msg "$d" "$MESSAGE_DIR/clean-full.txt" || return 1
            set_input "$i" "$d" '' '' ''
            ;;

        local-branch-defect)
            new_repo "$id" || return 1
            git_quiet git -C "$d" checkout -q -b work || return 1
            commit_msg "$d" "$MESSAGE_DIR/ticket-duplicate.txt" || return 1
            set_input "$i" "$d" '' '' ''
            ;;

        local-no-commits)
            # The state a coder's worktree is usually in: HEAD is the base
            # branch and the branch carries no commit of its own.
            new_repo "$id" || return 1
            set_input "$i" "$d" '' '' ''
            ;;

        local-no-base)
            # No `main` and no `origin/main`, so "the commits on this branch"
            # names no set at all. Built by hand rather than through new_repo,
            # which exists to produce the opposite.
            mkdir -p "$d" || return 1
            git_quiet git -C "$d" init -q || return 1
            git_quiet git -C "$d" symbolic-ref HEAD refs/heads/trunk || return 1
            git_quiet git -C "$d" config user.email 'gate-13@example.invalid' || return 1
            git_quiet git -C "$d" config user.name 'gate 13 fixture' || return 1
            git_quiet git -C "$d" config commit.gpgsign false || return 1
            printf 'no main branch here\n' >"$d/README.md" || return 1
            git_quiet git -C "$d" add -A || return 1
            git_quiet git -C "$d" commit -q -F "$BASE_MSG" || return 1
            set_input "$i" "$d" '' '' ''
            ;;

        *)
            say "    unknown setup token '$setup' for case $id"
            return 1
            ;;
    esac
    return 0
}

i=0
build_failed=''
while [ "$i" -lt "$CASE_COUNT" ]; do
    if ! build_one "$i"; then
        build_failed="${CASE_ID[$i]}"
        break
    fi
    i=$((i + 1))
done
if [ -n "$build_failed" ]; then
    VERDICT_REACHED=1
    emit error "the planted repository for '$build_failed' could not be built, so that row was never presented to gate 13. This is a broken fixture, not a verdict about gate 13. Nothing was proven."
    exit 3
fi

# Every row must have an input, and the input must be a git repository. An
# array left short would make the observation below read an unset element under
# `set -u`; saying it here names the cause.
if [ "${#CASE_REPO[@]}" -ne "$CASE_COUNT" ]; then
    VERDICT_REACHED=1
    emit error "built ${#CASE_REPO[@]} inputs for $CASE_COUNT cases, so some row has no planted repository and this harness will not judge a partial run"
    exit 3
fi

printf '  %-36s %-16s %s\n' 'planted input' 'event' 'what is in it'
printf '  %-36s %-16s %s\n' '------------------------------------' '----------------' '----------------------------------------'
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    d="${CASE_REPO[$i]}"
    n_commits="$(git -C "$d" rev-list --count --all 2>/dev/null || printf '?')"
    shallow_state="$(git -C "$d" rev-parse --is-shallow-repository 2>/dev/null || printf '?')"
    printf '  %-36s %-16s %s commit(s) reachable, shallow=%s\n' \
        "${CASE_ID[$i]}" "${CASE_EVENT[$i]:-(local run)}" "$n_commits" "$shallow_state"
    i=$((i + 1))
done

# ---------------------------------------------------------------------------
# Stage 6: observation. Run the gate once per row. Record the exit status and
# the output. Judge nothing yet.
# ---------------------------------------------------------------------------

STAGE='running gate 13 against every planted input'

run_gate() {
    # $1 repository, $2 event, $3 payload path, $4 GITHUB_SHA, $5 output file.
    # Returns the gate command's own exit status. No pipeline: the output is
    # redirected to a file and nothing else reads the status first.
    #
    # 255 is reserved for "the command did not run at all", so that a shell
    # that could not start bash can never be recorded as gate 13 reporting
    # something. `scripts/gates.sh` exits 0, 1, 2, 3, 129, 130 or 143 and never
    # 255.
    #
    # The environment is built rather than inherited. GITHUB_ACTIONS is unset
    # so that the annotations gates.sh writes about PLANTED commits do not land
    # on a real pull request; see the note at the top of this file.
    local repo="$1" event="$2" payload="$3" sha="$4" out="$5"
    local status
    (
        unset GITHUB_ACTIONS GITHUB_EVENT_NAME GITHUB_EVENT_PATH GITHUB_SHA
        export NO_COLOR=1
        if [ -n "$event" ]; then export GITHUB_EVENT_NAME="$event"; fi
        if [ -n "$payload" ]; then export GITHUB_EVENT_PATH="$payload"; fi
        if [ -n "$sha" ]; then export GITHUB_SHA="$sha"; fi
        exec bash "$GATES_SH" --commit-trailers --repo "$repo"
    ) >"$out" 2>&1
    status=$?
    return "$status"
}

rule 'Observation: gate 13 run against each planted input, once'
say 'Each command below runs once. Its exit status is captured on the next line,'
say 'with no pipeline between, and nothing is judged until stage 8. Only the'
say 'part of the output that is about gate 13 is reproduced here; the harness'
say 'self-check `scripts/gates.sh` runs before every gate is real and is not'
say 'what these rows are about.'
say ''
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    id="${CASE_ID[$i]}"
    log="$WORK_DIR/$id.log"
    shown="bash $GATES_SH_REL --commit-trailers --repo <planted>/$id"
    say "--- $id (${CASE_FAMILY[$i]})"
    say "    command    : $shown"
    if [ -n "${CASE_EVENT[$i]}" ]; then
        say "    event      : GITHUB_EVENT_NAME=${CASE_EVENT[$i]}"
    else
        say "    event      : none, which is a local run at a coder's desk"
    fi
    run_gate "${CASE_REPO[$i]}" "${CASE_EVENT[$i]}" "${CASE_PAYLOAD[$i]}" "${CASE_SHA[$i]}" "$log"
    status=$?
    if [ "$status" -eq 255 ]; then
        VERDICT_REACHED=1
        emit error "the gate command for '$id' could not be started at all (status 255). That is not gate 13 reporting anything, and this harness will not record it as a verdict."
        exit 2
    fi
    # Four verdicts, not two. Gate 13 has two different non-zero answers for
    # two different reasons, and reading either as the other would be this
    # gate's own defect class inside its proof.
    case "$status" in
        0) verdict='pass' ;;
        1) verdict='fail' ;;
        2) verdict='refuse' ;;
        3) verdict='nothing' ;;
        *)
            VERDICT_REACHED=1
            emit error "the gate command for '$id' exited $status, which is none of gate 13's four answers (0 pass, 1 fail, 2 could not enumerate, 3 nothing to enumerate). 129, 130 and 143 mean it was killed by a signal. This harness will not record a status it cannot read as a verdict."
            exit 2
            ;;
    esac
    strip_ansi "$log" "$log.plain"
    OBS_STATUS[$i]="$status"
    OBS_VERDICT[$i]="$verdict"
    OBS_LOG[$i]="$log.plain"
    say "    exit status: $status"
    say "    verdict    : $verdict"
    say "    output     : (the gate 13 section; this is the text the signatures are matched against)"
    if [ -s "$log.plain" ]; then
        sed -n '/^Gate 13 (alone)/,$p' "$log.plain" | sed 's/^/      | /'
    else
        say "      | (no output)"
    fi
    say ''
    i=$((i + 1))
done

if [ "${#OBS_VERDICT[@]}" -ne "$CASE_COUNT" ]; then
    VERDICT_REACHED=1
    emit error "observed ${#OBS_VERDICT[@]} of $CASE_COUNT cases, so some gate command did not run and this harness will not judge a partial run"
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 7: THE CENTRAL CONTRAST. A regex accepts the trap message and the gate
# refuses it.
#
# This is the one thing in this file that is worth reading if nothing else is.
# Everything above establishes that the gate runs and that this harness can
# tell one answer from another; this stage says what the gate is FOR.
#
# Three answers are printed for one commit message:
#
#   what a regex says   the check a reasonable person writes first, and the
#                       check this gate could have been. Both required lines
#                       are present, correctly spelled, at the start of their
#                       lines. It accepts.
#   what git says       `git interpret-trailers --parse`, which is the parser
#                       every consumer of these trailers goes through. It
#                       returns Co-Authored-By and nothing else.
#   what gate 13 says   REFUSED, with the two lines named and the repair
#                       spelled out.
#
# The first two are asserted here and a failure of either is exit 2: if the
# regex stops accepting, or git starts parsing, this fixture is no longer the
# defect it claims to be and the contrast is a story rather than an
# observation. The third is judged by the comparator below like every other
# row, so that there is exactly one place in this file where a verdict is
# compared to an expectation.
# ---------------------------------------------------------------------------

STAGE='contrasting a regex with git on the trap message'
rule 'The central contrast: a regex accepts it, git parses neither line'

trap_i=-1
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${CASE_ID[$i]}" = "$TRAP_ID" ]; then trap_i="$i"; fi
    i=$((i + 1))
done
if [ "$trap_i" -lt 0 ]; then
    VERDICT_REACHED=1
    emit error "the case table has no row with the id '$TRAP_ID', so the message this whole gate exists for was never presented to it. Nothing was proven about the defect this fixture is named after."
    exit 2
fi

trap_repo="${CASE_REPO[$trap_i]}"
trap_sha="$(git -C "$trap_repo" rev-parse HEAD 2>/dev/null)"
trap_body="$WORK_DIR/trap.body"
git -C "$trap_repo" show -s --format='%B' "$trap_sha" >"$trap_body" 2>/dev/null
trap_read=$?
if [ "$trap_read" -ne 0 ] || [ ! -s "$trap_body" ]; then
    VERDICT_REACHED=1
    emit error "the message of the planted trap commit could not be read back out of git (exit $trap_read), so this stage has nothing to contrast. Nothing was proven about it."
    exit 2
fi

say "The commit message, as git stores it:"
say ''
sed 's/^/      | /' "$trap_body"
say ''

# --- what a regex says ------------------------------------------------------
#
# This is not a straw man. It is the check a careful person writes: anchored to
# the start of a line, requiring the exact key and the exact separator. It is
# also, precisely, a check that is looking at the text and not at the
# mechanism.
#
# The two patterns are held in variables and both printed and run, so that what
# this stage reports having asked is the question it asked. A proof that
# described its own check in prose and ran a different one would be the defect
# it is about.
REGEX_TICKET='^Ticket: ORI-T-[0-9]{4}$'
REGEX_SPEC='^Spec: [A-Za-z0-9_/-]+\.md#[A-Za-z0-9][A-Za-z0-9._-]*$'
regex_ticket=1
regex_spec=1
grep -qE "$REGEX_TICKET" "$trap_body" && regex_ticket=0
grep -qE "$REGEX_SPEC" "$trap_body" && regex_spec=0
regex_hits="$(grep -cE '^(Ticket|Spec): ' "$trap_body" 2>/dev/null || printf '0')"
say "  WHAT A REGEX SAYS"
say "    grep -E '$REGEX_TICKET'"
say "      $( [ "$regex_ticket" -eq 0 ] && printf 'MATCHES' || printf 'no match' )"
say "    grep -E '$REGEX_SPEC'"
say "      $( [ "$regex_spec" -eq 0 ] && printf 'MATCHES' || printf 'no match' )"
say "    trailer-shaped lines found in the message     : $regex_hits"
if [ "$regex_ticket" -eq 0 ] && [ "$regex_spec" -eq 0 ]; then
    say "    verdict of a regex-based gate                 : ACCEPT. Both required"
    say "      trailers are present, correctly spelled, at the start of their lines."
else
    say "    verdict of a regex-based gate                 : reject"
fi
say ''

# --- what git says ----------------------------------------------------------
trap_parsed="$WORK_DIR/trap.parsed"
git interpret-trailers --parse <"$trap_body" >"$trap_parsed" 2>"$trap_parsed.err"
parse_status=$?
trap_ticket_value="$(git -C "$trap_repo" show -s --format='%(trailers:key=Ticket,valueonly,unfold=true)' "$trap_sha" 2>/dev/null)"
trap_spec_value="$(git -C "$trap_repo" show -s --format='%(trailers:key=Spec,valueonly,unfold=true)' "$trap_sha" 2>/dev/null)"
say "  WHAT GIT SAYS, and git is the only reader whose answer matters: every"
say "  consumer of these trailers goes through this parser."
say "    git interpret-trailers --parse (exit $parse_status) returned:"
if [ -s "$trap_parsed" ]; then
    sed 's/^/      | /' "$trap_parsed"
else
    say "      | (nothing)"
fi
say "    %(trailers:key=Ticket,valueonly) : [$(printf '%s' "$trap_ticket_value" | tr '\n' ' ')]"
say "    %(trailers:key=Spec,valueonly)   : [$(printf '%s' "$trap_spec_value" | tr '\n' ' ')]"
say ''

contrast_bad=0
if [ "$regex_ticket" -ne 0 ] || [ "$regex_spec" -ne 0 ]; then
    emit error "a regex anchored to the start of a line does NOT find both required trailers in messages/$TRAP_ID.txt. That message exists to be the one a regex accepts and git does not, so without that half there is no contrast to draw and this stage demonstrates nothing."
    contrast_bad=1
fi
if [ "$parse_status" -ne 0 ]; then
    emit error "'git interpret-trailers --parse' exited $parse_status on the trap message, so what git thinks of it is unknown and nothing below can be said about the difference between git and a regex."
    contrast_bad=1
fi
if [ -n "$trap_ticket_value" ] || [ -n "$trap_spec_value" ]; then
    emit error "git's trailer parser now DOES return a Ticket or a Spec trailer for messages/$TRAP_ID.txt. Either the message was repaired, or this git parses trailers differently from the one this fixture was written against. Either way the central claim of gate 13, that a message can carry both lines and be invisible to every consumer of them, is no longer demonstrated by this input. Nothing was proven about it."
    contrast_bad=1
fi
if ! grep -qE '^Co-Authored-By: ' "$trap_parsed"; then
    emit error "git's trailer parser returned no Co-Authored-By trailer for the trap message either, so it parsed nothing at all rather than parsing the LAST paragraph. The contrast this stage draws is between a parser that reads the last paragraph and a regex that reads the whole message; a parser that read neither would satisfy the assertions above for the wrong reason."
    contrast_bad=1
fi
if [ "$contrast_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "the central contrast of this fixture could not be demonstrated on this machine. Nothing was proven."
    exit 2
fi

say "  WHAT GATE 13 SAYS, observed in stage 6 and judged in stage 9 with every"
say "  other row:"
say "    exit status : ${OBS_STATUS[$trap_i]}"
say "    verdict     : ${OBS_VERDICT[$trap_i]} (owed: ${CASE_EXP[$trap_i]})"
if grep -qE '^      codes: ' "${OBS_LOG[$trap_i]}"; then
    sed -n 's/^      codes: /    reason codes: /p' "${OBS_LOG[$trap_i]}" | sed 's/^/  /'
fi
say ''
say "  SO: both required trailers are in this message, a regex finds both, and git"
say "  finds NEITHER. A regex-based gate 13 would pass this commit and report that"
say "  the traceability chain of AICD §13 is intact, while every tool that reads"
say "  trailers, changelog generation and the audit chain among them, sees nothing"
say "  there at all. That is AICD §39's 'present but reporting nothing' sitting"
say "  inside the gate that guards the audit trail. It is not hypothetical: this"
say "  message is the shape the lead shipped in the first commit this project made"
say "  under CONVENTIONS, which is why gate 13 asks git and asks nothing else."

# ---------------------------------------------------------------------------
# Stage 8: attribution. A non-zero exit is not evidence on its own. Where the
# gate answered what the table owes, the output must show that it answered it
# for the reason this row planted.
#
# This matters more here than it did for gate 7, because gate 13's two non-zero
# answers are close together. Exit 1 is "a commit does not conform" and exit 2
# is "I could not tell which commits to read", and a harness that accepted any
# non-zero status would count a gate that had stopped being able to enumerate
# anything as a gate catching every planted defect. Every `fail` row therefore
# names the reason codes it expects, and every `refuse` row names the sentence
# the enumeration printed.
#
# The rows where the gate answered something else are left to the comparator
# below: those are the rows where gate 13 stopped doing its job, and that is a
# gate failure (exit 1), not an unreadable observation (exit 2).
# ---------------------------------------------------------------------------

STAGE='checking that each answer is the answer this proof planted'
rule 'Attribution: the answer gate 13 gave is the answer to the planted question'
printf '  %-36s %-12s %-9s %s\n' 'input' 'family' 'observed' 'attributable to the planted mechanism'
printf '  %-36s %-12s %-9s %s\n' '------------------------------------' '------------' '---------' '-------------------------------------'
attribution_bad=0
attribution_broken=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${OBS_VERDICT[$i]}" != "${CASE_EXP[$i]}" ]; then
        printf '  %-36s %-12s %-9s %s\n' \
            "${CASE_ID[$i]}" "${CASE_FAMILY[$i]}" "${OBS_VERDICT[$i]}" \
            "not asked: the gate did not answer '${CASE_EXP[$i]}' here, which the table below reports"
        i=$((i + 1))
        continue
    fi
    output_matches "${OBS_LOG[$i]}" "${CASE_SIG[$i]}"
    status=$?
    case "$status" in
        0) printf '  %-36s %-12s %-9s %s\n' "${CASE_ID[$i]}" "${CASE_FAMILY[$i]}" "${OBS_VERDICT[$i]}" 'yes' ;;
        1)
            printf '  %-36s %-12s %-9s %s\n' "${CASE_ID[$i]}" "${CASE_FAMILY[$i]}" "${OBS_VERDICT[$i]}" "NO: $ATTRIB_WHY"
            attribution_bad=$((attribution_bad + 1))
            ;;
        *)
            printf '  %-36s %-12s %-9s %s\n' "${CASE_ID[$i]}" "${CASE_FAMILY[$i]}" "${OBS_VERDICT[$i]}" "HARNESS: $ATTRIB_WHY"
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
    emit error "$attribution_bad case(s) gave gate 13's owed answer for a reason this proof did not plant. Read the attribution table above: it names the row and the signature that did not match. Two different things land here and the table tells them apart. Either a planted input has acquired a second thing wrong with it, and the repair is in the fixture; or the gate still answers 'fail' and no longer answers it for the planted reason, and the repair is in the gate. A gate 13 that stopped asking git and started asking a regex arrives exactly this way: it still refuses the messages with no trailers at all, and the three trap rows stop carrying TICKET_UNPARSED because they stop being refused. Nothing was proven about gate 13 by this run."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stages 9 and 10: judgement. One comparator, two expectation sets.
#
# Flipping a four-valued expectation needs a rule, and the rule is that the
# opposite of an answer is any other answer, with `pass` as the flip of the
# three non-passing answers. That is not an arbitrary choice: `pass` is the
# only wrong answer that matters for a row whose owed answer is `refuse` or
# `nothing`, because a gate that reported a pass over a set it never read is
# the defect this whole fixture exists to catch.
# ---------------------------------------------------------------------------

MISMATCH_COUNT=0
JUDGE_AGREE=()

# The flip rule, in one place, used by the comparator and asserted by the
# verdict. `pass` is the flip of the three non-passing answers because `pass`
# is the only wrong answer that matters for a row whose owed answer is
# `refuse` or `nothing`.
flip_of() {
    case "$1" in
        pass)    printf 'fail' ;;
        fail)    printf 'pass' ;;
        refuse)  printf 'pass' ;;
        nothing) printf 'pass' ;;
        *)       printf '%s' "$1" ;;
    esac
}

judge() {
    # $1 is 'direct' (the real expectations) or 'inverted' (every expectation
    # flipped). This is the only place a verdict is compared to an expectation,
    # so both passes exercise the same code.
    local mode="$1"
    local i expected observed status agreement
    MISMATCH_COUNT=0
    JUDGE_AGREE=()
    printf '  %-36s %-12s %-9s %-9s %-6s %s\n' 'input' 'family' 'expected' 'observed' 'exit' 'result'
    printf '  %-36s %-12s %-9s %-9s %-6s %s\n' '------------------------------------' '------------' '---------' '---------' '------' '--------'
    i=0
    while [ "$i" -lt "$CASE_COUNT" ]; do
        expected="${CASE_EXP[$i]}"
        if [ "$mode" = 'inverted' ]; then
            expected="$(flip_of "$expected")"
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
        printf '  %-36s %-12s %-9s %-9s %-6s %s\n' \
            "${CASE_ID[$i]}" "${CASE_FAMILY[$i]}" "$expected" "$observed" "$status" "$agreement"
        i=$((i + 1))
    done
}

STAGE='judging the observations against the real expectations'
rule 'The proof: what gate 13 owes each input'
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
# Stage 11: the verdict. The only exit 0 in this file.
# ---------------------------------------------------------------------------

STAGE='reaching a verdict'
rule 'Verdict'

harness_broken=0
gate_broken=0

# Per-row complementarity: for every case, exactly one of the two passes may
# agree. This is what "the comparator is comparing" means, and unlike a count
# it stays true when the gate itself is broken.
#
# A ROW CAN FAIL THAT IN TWO WAYS AND THEY ARE NOT THE SAME FINDING. Gate 7's
# harness, whose shape this file follows, reported both as "this harness cannot
# tell agreement from disagreement", and ops/gates/gate-1.md records the cost
# of that conflation once already: a real fixture defect made gate 1's harness
# report ITSELF broken and sent its reader to look at the comparator. Gate 13
# has four verdicts rather than three, so the conflation is not a corner here,
# it is the ordinary consequence of a weakened gate. The two are separated:
#
#   agreed in BOTH passes   Impossible for a comparator that is comparing. The
#                           flipped expectation is never equal to the direct
#                           one (asserted below), and one observation cannot
#                           equal two different values. So this really is a
#                           comparator that answers a constant, and it is a
#                           harness failure: nothing was proven (exit 2).
#   disagreed in BOTH       The gate answered a THIRD value: neither the
#                           expectation nor the value the flip rule maps it to.
#                           That is the gate doing something the table does not
#                           allow, it already counts as a direct disagreement,
#                           and it is a gate failure (exit 1). The reader is
#                           told which rows and that the flip rule, not the
#                           comparator, is what made them look symmetrical.
#
# The flip rule is itself asserted first, because both readings above depend on
# it: a rule that mapped some expectation to itself would make the flipped pass
# a copy of the direct one and the whole self-check vacuous. `flip_of` is the
# same function the comparator used, so this asserts the rule that ran.
flip_identity=''
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "$(flip_of "${CASE_EXP[$i]}")" = "${CASE_EXP[$i]}" ]; then
        flip_identity="$flip_identity ${CASE_ID[$i]}"
    fi
    i=$((i + 1))
done
if [ -n "$flip_identity" ]; then
    harness_broken=1
    emit error "the flip rule maps the expectation of these case(s) to itself:$flip_identity. The flipped pass would then be a copy of the direct one, every row would look complementary, and the self-check below would be a formality with no content."
fi

both_agree=''
both_disagree=''
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${DIRECT_AGREE[$i]}" = "${INVERTED_AGREE[$i]}" ]; then
        if [ "${DIRECT_AGREE[$i]}" = '1' ]; then
            both_agree="$both_agree ${CASE_ID[$i]}"
        else
            both_disagree="$both_disagree ${CASE_ID[$i]} (owed ${CASE_EXP[$i]}, observed ${OBS_VERDICT[$i]});"
        fi
    fi
    i=$((i + 1))
done
say "  complementarity: direct $direct_mismatches disagreement(s), flipped $inverted_mismatches; the two must differ on every row"
if [ -n "$both_agree" ]; then
    harness_broken=1
    emit error "this harness cannot tell agreement from disagreement. These case(s) AGREED under the real expectations and AGREED again under their exact opposites:$both_agree. One observation cannot equal two different expectations, so a comparator that answers 'agree' to both is not comparing anything, and the proof above means nothing whatever it says."
fi
if [ -n "$both_disagree" ]; then
    # Not a harness failure. These rows are already counted in
    # direct_mismatches, so the gate verdict below fires; this names them so
    # that a reader is not sent to the comparator for a fault in the gate.
    emit error "these case(s) disagreed in BOTH passes, which means gate 13 answered a value that is neither what the table owes them nor what the flip rule maps that to:$both_disagree This is the gate answering a third thing, not the comparator failing: the commonest cause is a gate that has stopped being able to enumerate and now answers 'refuse' where a commit-level 'fail' is owed. It is counted as a gate failure below."
fi

if [ "$direct_mismatches" -eq "$CASE_COUNT" ] && [ "$CASE_COUNT" -gt 1 ]; then
    # Every single row wrong is a pattern no constant gate can produce. A gate
    # command replaced by `true` reports pass everywhere, which the rows that
    # expect a pass still agree with; one replaced by `false` reports fail
    # everywhere, which the rows that expect a failure still agree with. Only
    # an inverted comparison, here or in the gate, gets every row wrong. Say
    # so, because the message below would otherwise send a reader to look at a
    # fixture that is fine.
    emit error "every one of the $CASE_COUNT cases disagreed, which no gate that always passes or always fails can produce: those still agree with part of the table. Look for an inverted comparison first, in this harness or in the gate itself, before looking at the planted inputs."
fi

if [ "$direct_mismatches" -ne 0 ]; then
    gate_broken=1
    emit error "gate 13 did not behave the way ops/gates/gate-13.md claims: $direct_mismatches of $CASE_COUNT cases disagreed. Read the table above. A row expecting 'fail' that observed 'pass' means the check stopped catching something that is still planted, so gate 13 is no longer proven (AICD §14) and no document may cite it until it is re-proved. A row expecting 'refuse' or 'nothing' that observed 'pass' is worse: it means the gate reported success over a set of commits it never read, which is the defect class this fixture exists for and which no reader of a green check could have detected."
fi

VERDICT_REACHED=1

if [ "$harness_broken" -ne 0 ]; then
    exit 2
fi
if [ "$gate_broken" -ne 0 ]; then
    exit 1
fi

say "GATE 13 HAS BEEN SEEN TO FAIL ON A PLANTED DEFECT, AND SEEN TO REFUSE RATHER"
say "THAN PASS WHEN IT COULD NOT READ A COMMIT."
say ''
say "ESTABLISHED BY THIS RUN:"
say "  - '$GATE_CMD' is run by a job of $WORKFLOW_REL that an"
say "    unfiltered '$REQUIRED_TRIGGER' fires, with no 'if:' and no 'continue-on-error:'"
say "    on the job or on the step, and '$AGGREGATE_JOB' waits on that job and carries no"
say "    'continue-on-error:' itself, so its failure fails this run;"
say "  - that job checks out with fetch-depth $REQUIRED_FETCH_DEPTH, which is the only state in which the"
say "    base and head commits a pull request names are objects the runner holds;"
say "  - the '$SELF_JOB' job that runs this script is live by those same six"
say "    conditions, so the refusals above are refusals that fail this run too;"
say "  - every planted message is still planted, asserted against the message files"
say "    themselves and not against what any tool said about them;"
say "  - THE CONTRAST: on messages/$TRAP_ID.txt a regex anchored"
say "    to the start of a line finds both required trailers, git's own parser finds"
say "    NEITHER and returns only Co-Authored-By, and gate 13 refuses the commit and"
say "    names the repair. Stage 7 above carries the three answers as this run"
say "    observed them;"
say "  - gate 13 refused every planted defect in a commit message, and the refusal"
say "    named the mechanism: a trailer in the wrong paragraph is UNPARSED and not"
say "    MISSING, a lowercase key is CASE and not MISSING, a folded continuation is"
say "    read as part of the value, and a subject with the right shape and the wrong"
say "    vocabulary is a TYPE or SCOPE refusal and not a SHAPE one;"
say "  - it read EVERY commit in a range and not the head of it: a range whose head"
say "    is clean and whose first commit is not was refused;"
say "  - a merge commit inside a range was refused with no exemption, and a range"
say "    whose BASE is a merge commit passed without the base ever being read;"
say "  - and, the half that would have gone unnoticed: on nine planted ways for the"
say "    commit set to be empty or unnameable, and on two where there is legitimately"
say "    nothing at a desk, gate 13 answered 'I checked nothing' with a status that is"
say "    not 0 and never printed PASSED. That covers a shallow clone, an empty range,"
say "    a count GitHub and the gate disagree about, an unreadable payload, a payload"
say "    naming no SHAs, an event nobody wrote a rule for, a created ref, a push that"
say "    moved nothing, an empty merge group, a branch with no commits of its own, and"
say "    a repository with no base branch;"
say "  - each of those answers carried the signature of the mechanism planted for it;"
say "  - and this harness was shown, on this run, able to tell those answers apart:"
say "    its comparator, its liveness check, its self-identification check, its"
say "    checkout-depth check and its attribution check were each run over planted"
say "    inputs whose owed answers differ."
say ''
say "NOT ESTABLISHED BY THIS RUN:"
say "  - that the 'Spec:' anchor resolves, or that the document it names exists. The"
say "    document half is OBSERVED and printed and never judged, and both branches of"
say "    that observation are exercised above: a commit citing a document that is in"
say "    the tree and one citing a document that is not both PASS. Nothing in this"
say "    repository resolves a markdown anchor yet, and a gate that failed a commit"
say "    for an anchor nobody can check would be inventing policy;"
say "  - that the vocabulary this gate enforces is anybody's rule but this"
say "    repository's. spec/CONVENTIONS.md says 'Conventional Commits' and names no"
say "    list of types; the list is a decision read off what the repository does, and"
say "    the first legitimate 'refactor:' or 'test:' commit fails this gate until one"
say "    word is added to CT_TYPES in a ticket. That cost is stated rather than"
say "    hidden;"
say "  - that gate 13 has ever failed on a commit in THIS repository's history. Every"
say "    commit on 'main' since the specification baseline carries both trailers where"
say "    git can read them. What is proven here is that it would not pass one that"
say "    did not;"
say "  - that a real GitHub runner hands this gate the commits this harness hands it."
say "    The payloads above are written by this script. What they exercise is the"
say "    enumeration code CI runs, on repositories whose commits are known; what they"
say "    cannot exercise is GitHub's side of the contract. The cross-check against the"
say "    commit count GitHub reports is what turns a disagreement there into a refusal"
say "    rather than a quiet short range;"
say "  - that the failure is visible where a human would look. This script cannot"
say "    watch the pull request check it writes to;"
say "  - that a failing '$AGGREGATE_JOB' blocks a merge. That is branch protection and not this"
say "    file. '$AGGREGATE_JOB' is not a required status check on 'main':"
say "    ops/gates/branch-protection.md records that 'main' has no required checks at"
say "    all, that this is deliberate, and that each check name is added on the day its"
say "    proof file lands. Until '$AGGREGATE_JOB' is required, a red run here colours the pull"
say "    request and stops nothing;"
say "  - that the job running this script will still be live on the next commit. A"
say "    'continue-on-error:' added to '$SELF_JOB' later makes this script refuse, and a"
say "    job carrying that key does not fail the run when it refuses. The refusal is"
say "    then an annotation and a human reading the diff of $WORKFLOW_REL is the last"
say "    link. That path is tier 2 for this reason."
say ''
say "Gate 13 has been seen to fail on a planted defect on this commit, which is the"
say "demonstration AICD §14 asks for before a gate enters service, and it has been"
say "seen to refuse rather than pass on every planted way of having nothing to check."
say "ops/gates/gate-13.md records the whole of it."
exit 0
