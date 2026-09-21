#!/usr/bin/env bash
#
# prove.sh: the proof harness for gate 7 of spec/CI_CD.md section 1.
#
# ===========================================================================
# THIS SCRIPT PROVES TWO OF GATE 7'S THREE CHECKS. IT REFUSES TO CLAIM THE
# THIRD, AND EXIT 0 FROM IT IS NOT A CLAIM THAT GATE 7 IS INSTALLED.
# ===========================================================================
#
#   advisories (cargo-audit)        PROVEN here. Seen to fail on a planted
#                                   lockfile naming time 0.1.44, and to pass
#                                   on one naming a version with no advisory,
#                                   with a loaded advisory database in both.
#   licences and bans (cargo-deny)  PROVEN here. Each half seen to fail on its
#                                   own planted input with the other half
#                                   staying clean, so each is attributable.
#   secret scan                     NOT PROVEN, AND NOT PROVABLE BY THIS
#                                   SCRIPT. It is not installed. Escalation
#                                   E-0003 asks the operator what implements
#                                   it. The secret rows below are kept and
#                                   still run, and what they establish is what
#                                   `scripts/secret-scan.sh` DOES, never that
#                                   gate 7's third check EXISTS.
#
# THE DISTINCTION THIS SCRIPT NOW TURNS ON. AICD §14 says a gate is installed
# only when it has been seen to fail. That is necessary and it is not
# sufficient. A check can be seen to fail on every input somebody thought to
# plant and still be no protection, because what it misses is the set nobody
# planted. The secret rows below are green and were green through two attempts
# in which the scanner missed, in turn, every file holding a NUL byte and every
# credential encoded UTF-16. Fourteen green rows said nothing about either.
# So "seen to fail on a planted defect" is reported for the secret scan as
# evidence of behaviour, and the installation claim is withheld.
#
# Ticket: ORI-T-0016.  Spec: spec/runbooks/prove-gate.md, spec/TESTING.md
# sections 1 and 4, spec/CI_CD.md section 1 gate 7, spec/ENV_SETUP.md section
# 4, spec/CONVENTIONS.md "Dependencies".  Record: ops/gates/gate-7.md, which
# records gate 7 as PARTIALLY installed.
#
# ===========================================================================
# WHAT THIS SCRIPT IS FOR, AND WHAT MAKES GATE 7 DIFFERENT
# ===========================================================================
#
# AICD §14: "a gate is installed only when it has been seen to fail". Gates 1
# and 2 already existed as jobs and had to be proven. Gate 7 did not exist at
# all: nothing in `.github/workflows/ci.yml` ran cargo-audit, cargo-deny or any
# secret scan, `scripts/gates.sh` reported gate 7 unavailable, and the gate was
# a line in a specification. ORI-T-0016 builds it and proves it in one ticket,
# so this harness is the only thing standing between "gate 7 exists" and "gate
# 7 works".
#
# GATE 7 IS THREE CHECKS, NOT ONE, and they fail in three different tools for
# three different reasons. Each is run separately and attributably, the way
# gate 1 proved its fmt and clippy halves apart. Two of the three are proven;
# the third is run and reported, not proven:
#
#   advisories       `cargo audit --deny warnings`. A lockfile naming a version
#                    with a real published advisory must fail; one naming a
#                    version with none must pass.
#   licenses, bans   `cargo deny ... check licenses bans sources`. A crate
#                    under a license deny.toml does not allow must fail the
#                    licenses check and leave the bans check clean; a wildcard
#                    version requirement must fail the bans check and leave the
#                    licenses check clean. Each half is separately attributable
#                    because cargo-deny prints a verdict per check.
#   secret scan      NOT A GATE. `scripts/secret-scan.sh` is advisory: it
#                    matches six byte patterns against printable-ASCII runs of
#                    a repository's tree AND its history. Nine planted
#                    repositories: four of text, one shallow which the scanner
#                    must REFUSE rather than report clean, and four about
#                    content that is printable ASCII wrapped in non-printable
#                    bytes. All nine are evidence of what this scanner does.
#                    None of them is evidence that gate 7's secret scan exists,
#                    because the shapes it cannot see are not among them and
#                    cannot be: see the banner at the top of this file.
#
# THE CONSTRAINT THAT SHAPED THE FIXTURE. This workspace has zero third-party
# dependencies, and adding one is an escalation trigger (spec/CONVENTIONS.md,
# AICD §12). So the advisory half is planted as a Cargo.lock with no manifest,
# naming a real vulnerable version that nothing ever builds or downloads, and
# the cargo-deny halves are planted as path-only workspaces, which cargo-deny
# resolves with no network. Nothing under `fixtures/planted/gate-7/` is a
# dependency of anything.
#
# WHAT IS IN THIS DIRECTORY AND WHAT IS NOT. Planted defects, and the harness
# that runs a gate against them. The dependency policy is `deny.toml` at the
# repository root and the scanner is `scripts/secret-scan.sh`: a policy the
# whole repository is judged by and a scanner the gate invokes are not inputs a
# gate is run against, so they do not live in a fixture directory (ruling R28).
# This harness asserts on every run that neither has come back here as a second
# copy, because a gate whose implementation exists twice can be repaired in the
# copy CI does not run.
#
# THE TWO DEFECTS THIS HARNESS DID NOT CATCH, AND WHAT THAT SETTLED.
#
# First. Attempt 1 proved all ten of its rows and every one of them was about
# text. The scanner matched with `grep -I`, which reports no match for a file
# holding a NUL byte without reading it, so every such file was skipped,
# counted as scanned, and reported clean, in both halves. Ten green rows said
# nothing about any of them. Four rows were added, pinning the scanner's own
# account of how it read each file (`1 as binary`) and where the match came
# from (`run` rather than `line`), so a return to skipping is a DISAGREE and
# not a quieter pass. Those four rows are good and they stay.
#
# Second, and this is the one that ended the argument. Fourteen green rows then
# said nothing about a credential encoded UTF-16. The lead planted one,
# committed it, and the repaired scanner exited 0 and reported no finding with
# the key in the file. base64, gzip, UTF-32 and a key split across a chunk
# boundary are the same hole.
#
# WHAT AN EARLIER VERSION OF THIS COMMENT CLAIMED, AND WHY IT WAS WRONG. It
# said the four rows covered "a keystore, a .p12, a compiled binary with a
# token baked in, a SQLite file or an .env saved with a byte-order mark". All
# five were then built and run against the repaired scanner. Three of the five
# are still silently clean: a real .p12 and a real Java keystore store the key
# DER-encoded and encrypted, and a UTF-16 .env stores it one byte apart from
# itself, so in all three the credential is in the file and the string is not.
# A sixth, the UTF-8 byte-order mark, is found and always was: it has no NUL
# byte. The real output of all six runs is in ops/gates/gate-7.md.
#
# What the four rows actually cover is one shape: printable ASCII with
# non-printable bytes around it, which is the compiled-object and SQLite shape.
# That is a real shape and worth the rows. It is not five shapes, and a comment
# claiming it was is this project's own defect class written into the harness
# meant to catch it.
#
# ===========================================================================
# THE SIX QUESTIONS, AND WHY EACH NEEDS ITS OWN ANSWER
# ===========================================================================
#
# ORI-T-0013 and ORI-T-0014 found seven defects between them in the harnesses
# for gates 1 and 2, every one a code path reporting success for a state that
# is not success, and not one of them found by reading. Their questions are
# asked here, in their shape, and a sixth is added that neither needed.
#
#   1. IS THIS A COMMAND CI RUNS?  Stage 4. An anchored grep for the command
#      string in `.github/workflows/ci.yml` cannot tell a command CI runs from
#      a command CI does not: the line can belong to a job that is not in the
#      required aggregate's `needs:`, or to a job or step carrying
#      `continue-on-error:` or an `if:`, or it can be text inside another
#      step's shell script, or the workflow's triggers can mean nothing fires
#      on a pull request at all. Every one leaves the line byte-identical. The
#      file is parsed (`workflow-facts.awk`) and the question asked of the
#      structure, of all three commands.
#
#   2. DOES GATE 7 GIVE EACH INPUT THE VERDICT IT OWES?  Stages 7 and 8. Ten
#      planted inputs across the three commands, judged by one comparator run
#      twice over one set of observations with every expectation flipped.
#
#   3. IS THE ANSWER THE ANSWER TO THE PLANTED QUESTION?  Stage 6. A non-zero
#      exit is not evidence on its own. cargo-audit exits non-zero when it
#      cannot reach the advisory database; cargo-deny exits non-zero when it
#      cannot read a manifest; the secret scanner exits non-zero when it cannot
#      scan at all. None of those is the gate catching anything. Each case
#      names the signatures its planted mechanism leaves in the output,
#      including signatures that must be ABSENT, and an answer without them
#      proves nothing.
#
#   4. IS THE JOB THAT ASKS THE FIRST THREE ITSELF LIVE?  Stage 4b.
#      `continue-on-error: true` on `gate-7-proof` turns every refusal below
#      into a green check: the harness still runs, still refuses, still writes
#      its annotation, and GitHub records the job as a success. The same six
#      structural conditions are asked of this script's own job, which it finds
#      in the file by the command that invokes it AND by name, with both
#      required to agree.
#
#   5. IS THE PLANT STILL PLANTED?  Stage 3b. Four of gate 7's ten inputs are
#      planted to be PASSED, and a pass looks the same whether the plant is
#      there or not. `advisory-clean` is a lockfile that must name a real crate
#      and no vulnerable one; `deny-clean` must name a license the policy
#      allows; the declared-fake sample must still carry the marker that makes
#      it declared, or the exemption it exists to exercise is never exercised
#      and this harness reports that the scan handles declared fakes while
#      demonstrating nothing. The plants are asserted against the fixture
#      sources directly, and a missing plant is a refusal.
#
#   6. IS THE HISTORY ACTUALLY ON THE RUNNER?  Stage 4c, and gates 1 and 2 did
#      not need it. Gate 7's secret scan is the only gate whose reach depends
#      on an input to an action rather than on a command. `actions/checkout`
#      defaults to `fetch-depth: 1`; in that clone there is one commit, so
#      "scan every blob any ref reaches" reads the tree and nothing else, and
#      the history half of the gate reports success having read no history.
#      `secret-scan.sh` refuses on a shallow repository rather than reporting
#      it clean, which makes the wrong value a red job; this stage reads the
#      value out of the workflow so that the reason is named rather than
#      discovered.
#
# ===========================================================================
# THE INVERSION, WHICH IS THE DANGEROUS PART
# ===========================================================================
#
# This job's success condition is the opposite of the jobs it proves: five of
# its ten cases pass when the gate command fails, and one passes when the gate
# command refuses to run. Getting that backwards produces a job that passes on
# every input, which is the exact defect AICD §14 names. Five things guard it,
# and none of them is a comment.
#
#   1. The case table is not all failures and not all passes. Four inputs gate
#      7 must pass, five it must fail, one it must refuse. One comparator
#      judges all ten, so a harness that reported failure for everything would
#      disagree with four rows and this run would go red.
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
#   3. The liveness check of stage 4 is a checker like any other, so it is
#      exercised before it is trusted: stage 3c runs it over the planted
#      workflows in `workflow-samples/`, one of which is live, ten of which are
#      dead in ten different ways and one of which cannot be read. The
#      self-identification check of stage 3d, the checkout-depth check of stage
#      3e and the attribution check of stage 3f are exercised the same way,
#      each against inputs whose owed answers differ, and each table is
#      required to contain every answer its check can give.
#
#   4. The plants are asserted, not assumed (question 5 above).
#
#   5. The secret scanner is a checker this ticket wrote, so it is held to the
#      same rule as the rest: its planted repositories include one it must call
#      clean, two it must call dirty in two different places, one whose only
#      secret is in a file that no longer exists, and one it must refuse. A
#      scanner that answered "clean" to everything, or "dirty" to everything,
#      or that quietly reported a shallow clone clean, disagrees with that
#      table.
#
# Separating observation from judgement is what makes those passes honest: the
# ten gate commands run once, their exit statuses and their output are
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
# `set -e` is deliberately NOT used. Six of the ten commands here are expected
# not to exit 0, and an errexit shell that leaves before the verdict is a gate
# that exits 0 without checking anything. The EXIT trap below turns any such
# early departure into exit 2.
#
# ===========================================================================
# EXIT STATUS
# ===========================================================================
#
#   0  the proof holds: gate 7's three commands are run by a job whose failure
#      fails the run, that job's checkout asks for the whole history, the job
#      that runs this script is live by those same six conditions, every
#      planted input is still planted, gate 7 caught every input carrying a
#      planted defect, passed every clean one, refused the one it cannot scan,
#      each answer carried the signature of the mechanism planted for it, and
#      this harness was shown able to tell those answers apart.
#   1  gate 7 did not behave the way ops/gates/gate-7.md claims. Either a check
#      has been weakened or a planted input no longer contains what it was
#      planted to contain. Gate 7 may not be cited until this is 0 again
#      (spec/runbooks/prove-gate.md, "Rollback: revoke the proof").
#   2  this harness could not prove what it claims: it could not tell agreement
#      from disagreement, or every case disagreed, which means a comparison is
#      inverted and does not say whether the inversion is in this harness or in
#      the gate, or the workflow no longer runs a command under proof where its
#      failure would fail the run, or it no longer runs this script there, or
#      this script could not find itself in the workflow, or the `gate-7` job's
#      checkout no longer asks for the whole history, or a planted input is no
#      longer planted, or an answer was given for a reason that is not the
#      planted one. Nothing was proven either way.
#   3  a prerequisite is missing, so no gate command ran at all: cargo-audit,
#      cargo-deny or git is not usable here, or the RustSec advisory database
#      could not be loaded. Nothing was checked, and this is not a pass.
#
# Run it from anywhere: `bash fixtures/planted/gate-7/prove.sh`.

set -uo pipefail

# ---------------------------------------------------------------------------
# Location
# ---------------------------------------------------------------------------

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
WORKFLOW_REL='.github/workflows/ci.yml'
AWK_PARSER="$HERE/workflow-facts.awk"
SAMPLE_DIR="$HERE/workflow-samples"
# The scanner and the policy do not live in this fixture directory. Ruling R28:
# a fixture directory holds planted defects, which are inputs a gate is run
# against; a policy the whole repository is judged by and a scanner the gate
# invokes are neither. `deny.toml` sits where cargo-deny looks by default, and
# the scanner sits beside the other scripts a gate runs.
SCANNER="$REPO_ROOT/scripts/secret-scan.sh"
DENY_CONFIG="$REPO_ROOT/deny.toml"
DENY_CONFIG_REL='deny.toml'
SCANNER_REL='scripts/secret-scan.sh'
FAKE_SAMPLE="$HERE/secret-samples/declared-fake.env"

# The three commands the `gate-7` job runs, written once here and compared
# against the workflow file below so that this script cannot drift from what it
# is reporting on. spec/CI_CD.md section 1 item 7 is "cargo-audit, cargo-deny
# (advisories, licenses), secret scan (tree and history)".
#
# TWO OF THESE THREE ARE GATE COMMANDS. The third, SCAN_CMD, is advisory: gate
# 7's secret scan is not installed and E-0003 is open on what implements it.
# It is still checked for liveness here, and that check still earns its place:
# an advisory step that silently stopped running would be worse than no step,
# and it is the only thing verifying the runner has a history to scan at all.
# Liveness is not installation, and the verdict at the bottom keeps them apart.
AUDIT_CMD='cargo audit --deny warnings'
DENY_CMD="cargo deny --manifest-path Cargo.toml --config $DENY_CONFIG_REL check licenses bans sources"
SCAN_CMD="bash $SCANNER_REL"

# The job that runs them, whose checkout must ask for the whole history, and
# the value it must ask for. `fetch-depth: 0` is the only thing that puts a
# history on the runner for the secret scan to read.
GATE_JOB='gate-7'
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

# This harness's own job, and the command that runs it. Everything above is a
# question this script asks about other jobs. This is the one it asks about
# itself, and gate 1's harness did not ask it until its third attempt: add
# `continue-on-error: true` to that job and every refusal this script can issue
# becomes a green check.
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
SELF_JOB='gate-7-proof'
SELF_FILE="$HERE/$(basename -- "${BASH_SOURCE[0]}")"
case "$SELF_FILE" in
    "$REPO_ROOT"/*) SELF_REL="${SELF_FILE#"$REPO_ROOT"/}" ;;
    *)              SELF_REL='' ;;
esac
SELF_CMD="bash $SELF_REL"

# The commands as argument vectors, split once here rather than by leaving an
# unquoted variable to the shell's word splitting and globbing. Neither command
# contains a glob character today; this does not depend on that staying true.
read -r -a AUDIT_ARGV <<<"$AUDIT_CMD"

# The declared-fake marker, assembled rather than written, exactly as
# secret-scan.sh assembles it and for the same reason: a tracked file that
# spelled it out beside a secret-shaped string would be declaring that string
# fake, and this file has no business declaring anything fake.
MARKER='FIXTURE'
MARKER="${MARKER}FAKE"

# What the marker is replaced with to manufacture the caught half of the secret
# fixture. Eleven characters, the same length as the marker, all of them upper
# case letters so that the derived value still matches every pattern including
# the one whose character class is `[A-Z ]`. This string on its own matches no
# pattern, which is why it can be written here.
FILLER='ZQMWNBVCXKJ'

# ---------------------------------------------------------------------------
# The case table: one row per planted input, with the check it belongs to, the
# verdict gate 7 owes it, the class of thing the row is about, and the
# signatures the planted mechanism leaves in the output.
#
# THE VERDICT COLUMN HAS THREE VALUES, not two. `pass` is exit 0, `fail` is the
# gate catching something, and `refuse` is the gate saying it could not check.
# The third exists because the sharpest planted input in this fixture is a
# shallow clone, where the only wrong answer that matters is `pass`: a scanner
# that reported a shallow clone clean would be a history check that read no
# history, and it would be green for ever.
#
# THE CLASS COLUMN IS NOT DECORATION. It is what lets the verdict separate the
# rows that are the AICD §14 demonstration (`catches`) from the controls
# (`control`) and from the row about a state the gate declines to judge
# (`refuses`).
#
# A signature beginning with `!` must NOT appear. Several rows here are about
# what the output does not say: `deny-license-defect` is only attributable if
# the bans check stayed clean while the licenses check failed, and the reverse
# for `deny-bans-defect`, because a fixture that broke both would prove neither.
# ---------------------------------------------------------------------------

CASE_ID=()
CASE_CHECK=()
CASE_INPUT=()
CASE_EXP=()
CASE_CLASS=()
CASE_SIG=()

add_case() {
    # $1 id, $2 check, $3 input, $4 expected verdict, $5 class, $6.. signatures.
    CASE_ID+=("$1")
    CASE_CHECK+=("$2")
    CASE_INPUT+=("$3")
    CASE_EXP+=("$4")
    CASE_CLASS+=("$5")
    shift 5
    local sig='' pat
    for pat in "$@"; do
        sig="$sig$pat"$'\n'
    done
    CASE_SIG+=("$sig")
}

# --- the advisory half -----------------------------------------------------
#
# `Loaded [0-9]+ security advisories` is a signature on BOTH rows and it is the
# important one. Without it, a cargo-audit that loaded an empty database, or
# none, would report every lockfile clean and the clean row would agree. The
# pattern requires at least three digits, so an empty or truncated database
# cannot satisfy it.

add_case  advisory-clean   advisories  advisory-clean   pass  control \
    '^ *Loaded [0-9]{3,} security advisor' \
    '^ *Scanning Cargo\.lock for vulnerabilities \(2 crate dependencies\)$' \
    '!RUSTSEC-' \
    '!vulnerability found' \
    '!^error'

add_case  advisory-defect  advisories  advisory-defect  fail  catches \
    '^ *Loaded [0-9]{3,} security advisor' \
    '^ *Scanning Cargo\.lock for vulnerabilities \(2 crate dependencies\)$' \
    '^Crate: +time$' \
    '^Version: +0\.1\.44$' \
    '^ID: +RUSTSEC-2020-0071$' \
    '^error: 1 vulnerability found!$' \
    '!could not|failed to fetch'

# --- the licenses and bans half --------------------------------------------
#
# cargo-deny prints one verdict per check on its last line, which is what makes
# the two halves separately attributable: a fixture that failed both checks
# would prove neither, and the negative signatures below are what say so.

add_case  deny-clean           licenses  deny-clean           pass  control \
    '^bans ok, licenses ok, sources ok$' \
    '!FAILED' \
    '!error\['

add_case  deny-license-defect  licenses  deny-license-defect  fail  catches \
    '^error\[rejected\]: failed to satisfy license requirements$' \
    'GPL-3\.0-only' \
    'rejected: license is not explicitly allowed' \
    '^bans ok, licenses FAILED, sources ok$' \
    '!^bans FAILED'

add_case  deny-bans-defect     licenses  deny-bans-defect     fail  catches \
    '^error\[wildcard\]: found 1 wildcard dependency for crate .gate-7-deny-bans-root.$' \
    'wildcard dependency' \
    '^bans FAILED, licenses ok, sources ok$' \
    '!licenses FAILED'

# --- the secret scan half --------------------------------------------------
#
# The four repositories below are built by stage 5 in a temporary directory
# from one shipped file, `secret-samples/declared-fake.env`. The caught half is
# that file with the declared-fake marker replaced by eleven other characters,
# which leaves every value the same shape and the same length and leaves none
# of them declared. It is manufactured rather than shipped because a tracked
# file holding an undeclared secret-shaped string is the thing this gate exists
# to reject, and a fixture that made the repository fail its own gate would be
# a fixture nobody could merge.
#
# `secret-declared-fake` is the row with the most to say. It is a PASS, and a
# pass that exempted nothing would mean the sample had lost its secrets and the
# exemption path was never taken. So its signature pins the count of matched
# strings that carried the marker. Change the sample and this row refuses until
# the count here is changed with it.

add_case  secret-clean          secrets  clean          pass    control \
    '^  findings \(tree\)     : 0$' \
    '^  findings \(history\)  : 0$' \
    '^  matches examined    : 0$' \
    '^no finding: no secret-shaped printable-ASCII string outside a declared fake' \
    '^THIS IS NOT A CLEAN BILL AND NOT A GATE\.' \
    '!FINDING '

add_case  secret-declared-fake  secrets  declared-fake  pass    control \
    '^  matches examined    : 18$' \
    '^  declared fakes      : 18 ' \
    '^  findings \(tree\)     : 0$' \
    '^  findings \(history\)  : 0$' \
    '!FINDING '

add_case  secret-in-tree        secrets  tree-secret    fail    catches \
    '^  declared fakes      : 0 ' \
    '^  findings \(tree\)     : 9$' \
    '^  findings \(history\)  : 9$' \
    '^  FINDING tree config\.env line [0-9]+ aws-access-key-id$' \
    '^  FINDING tree config\.env line [0-9]+ private-key-block$' \
    '^  FINDING tree config\.env line [0-9]+ github-token$' \
    '^  FINDING tree config\.env line [0-9]+ provider-api-key$' \
    '^  FINDING tree config\.env line [0-9]+ slack-token$' \
    '^  FINDING tree config\.env line [0-9]+ secret-assignment$' \
    'error: the secret scan found 9 in the tree and 9 in the history'

# The one with teeth. The file was committed and then deleted, so the tree is
# clean and `git grep` says nothing, and the blob is still there for anyone who
# clones. A tree-only scanner passes this repository and would pass it for
# ever. The signature is therefore as much about the zero as about the nine.
add_case  secret-in-history     secrets  history-secret fail    catches \
    '^  findings \(tree\)     : 0$' \
    '^  findings \(history\)  : 9$' \
    '^  FINDING history [0-9a-f]{12} config\.env line [0-9]+ aws-access-key-id$' \
    'error: the secret scan found 0 in the tree and 9 in the history' \
    '!FINDING tree '

# The sharpest one, and the only row in this fixture whose owed answer is a
# refusal. `actions/checkout` clones with `fetch-depth: 1` by default; in that
# clone the scanner can see one commit and would report the history clean
# having read the tree. The scanner must say it cannot scan, and it must say it
# with a status that is not 0 and not the status it uses for a finding, because
# "I could not look" is a different answer from both.
add_case  secret-shallow        secrets  shallow        refuse  refuses \
    'is shallow' \
    'so its history is not here to be scanned' \
    'this is a refusal and not a pass' \
    '!^no finding: ' \
    '!^  findings '

# --- the four rows about content a scanner is most tempted to skip ---------
#
# THE DEFECT THESE FOUR WERE ADDED FOR. The scanner matched with `grep -I`,
# which reports no match for any file holding a NUL byte without reading it.
# The file was skipped, counted as scanned, and contributed nothing, in both
# halves, and the verdict printed was "clean ... in 2 tracked file(s) and 2
# history blob(s)", exit 0.
#
# WHAT THESE FOUR ROWS COVER, AND WHAT THEY DO NOT. They cover ONE shape:
# printable ASCII with non-printable bytes around it. A compiled object with a
# token baked in has that shape, and so does a SQLite file, which stores text
# columns as UTF-8; both were built and both are now caught, exit 1.
#
# They do NOT cover a container that encodes or encrypts the credential, and an
# earlier version of this comment claimed they did. A real .p12 and a real Java
# keystore were built and run: exit 0, no finding, key in the file. Nor do they
# cover a change of encoding: a UTF-16 .env is exit 0 as well. The file this
# fixture manufactures is named `ascii-in-nul-bytes.bin` and is NEITHER of those things;
# it is ASCII between NUL bytes wearing a JKS magic number. See `keystore()`
# below, which now says so. The measured results are in ops/gates/gate-7.md.
#
# The rows are written so that a return to skipping is a DISAGREE and not a
# quieter pass: each one pins `1 as binary` in the counts, which is the scanner
# saying it took the binary path, and each pins `run` rather than `line` in the
# finding, which is the scanner saying the match came out of the printable runs
# of a binary. `!FINDING ... line ` is the negative half: a scanner that
# stopped classifying and read the bytes as text would still find these, and
# this proof would then be agreeing with a mechanism it did not plant.
#
# The declared-fake row is the control, and it is the one that shows binary
# scanning is matching rather than failing on anything that is not text: the
# same file inside a binary, marker intact, must be found eighteen times and
# exempted eighteen times.

add_case  secret-nul-declared-fake secrets nul-declared-fake pass control \
    '^  scanned \(tree\)      : 2 file\(s\), 1 as text and 1 as binary, [0-9]+ byte\(s\)$' \
    '^  not scanned         : 0$' \
    '^  matches examined    : 18$' \
    '^  declared fakes      : 18 ' \
    '^  findings \(tree\)     : 0$' \
    '^  findings \(history\)  : 0$' \
    '!FINDING '

add_case  secret-nul-tree       secrets  nul-tree-secret    fail    catches \
    '^  scanned \(tree\)      : 2 file\(s\), 1 as text and 1 as binary, [0-9]+ byte\(s\)$' \
    '^  not scanned         : 0$' \
    '^  declared fakes      : 0 ' \
    '^  findings \(tree\)     : 9$' \
    '^  findings \(history\)  : 9$' \
    '^  FINDING tree ascii-in-nul-bytes\.bin run [0-9]+ aws-access-key-id$' \
    '^  FINDING tree ascii-in-nul-bytes\.bin run [0-9]+ private-key-block$' \
    '^  FINDING tree ascii-in-nul-bytes\.bin run [0-9]+ github-token$' \
    '^  FINDING tree ascii-in-nul-bytes\.bin run [0-9]+ provider-api-key$' \
    '^  FINDING tree ascii-in-nul-bytes\.bin run [0-9]+ slack-token$' \
    '^  FINDING tree ascii-in-nul-bytes\.bin run [0-9]+ secret-assignment$' \
    'error: the secret scan found 9 in the tree and 9 in the history' \
    '!FINDING tree ascii-in-nul-bytes\.bin line '

# The same file, committed and then deleted, so the working tree holds no
# binary at all and the only copy left is the blob. This is the row that would
# have stayed green under `grep -I` for ever: nothing in the tree to see, and a
# history half that answered "no match" without reading the blob.
add_case  secret-nul-history    secrets  nul-history-secret fail    catches \
    '^  scanned \(tree\)      : 1 file\(s\), 1 as text and 0 as binary, [0-9]+ byte\(s\)$' \
    '^  scanned \(history\)   : 2 blob\(s\), 1 as text and 1 as binary, [0-9]+ byte\(s\)' \
    '^  not scanned         : 0$' \
    '^  findings \(tree\)     : 0$' \
    '^  findings \(history\)  : 9$' \
    '^  FINDING history [0-9a-f]{12} ascii-in-nul-bytes\.bin run [0-9]+ aws-access-key-id$' \
    'error: the secret scan found 0 in the tree and 9 in the history' \
    '!FINDING tree '

# Genuinely large and genuinely binary: 16 MiB of non-printable filler with a
# credential baked into it, which is the shape of a compiled artifact somebody
# committed. It is here because "scan every byte" is a claim with a bill
# attached, and the bill has to be paid in front of a witness rather than
# assumed small. The scanned-bytes count in the signature is eight digits, so
# a scanner that sampled the first few kilobytes and stopped would not satisfy
# this row.
add_case  secret-large-binary   secrets  large-binary       fail    catches \
    '^  scanned \(tree\)      : 2 file\(s\), 1 as text and 1 as binary, 1[0-9]{7} byte\(s\)$' \
    '^  scanned \(history\)   : 2 blob\(s\), 1 as text and 1 as binary, 1[0-9]{7} byte\(s\)' \
    '^  not scanned         : 0$' \
    '^  findings \(tree\)     : 9$' \
    '^  findings \(history\)  : 9$' \
    '^  FINDING tree blob\.bin run [0-9]{4,} aws-access-key-id$' \
    '^  FINDING history [0-9a-f]{12} blob\.bin run [0-9]{4,} github-token$' \
    'error: the secret scan found 9 in the tree and 9 in the history' \
    '!FINDING tree blob\.bin line '

CASE_COUNT=${#CASE_ID[@]}

OBS_VERDICT=()
OBS_STATUS=()
OBS_LOG=()

# ---------------------------------------------------------------------------
# The sample table: the planted defects for the liveness check of stage 4.
# Each file under workflow-samples/ is a whole workflow carrying exactly one
# way of switching gate 7 off, or none. The classification this harness owes
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
# they are owed about gate 7's command is the answer they are owed about this
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
# The checkout-depth table: the planted inputs for stage 4c. Gate 7 is the only
# gate in this repository whose reach depends on an input to an action, and the
# check that reads it gets the same treatment as every other checker here.
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
        printf '::%s file=fixtures/planted/gate-7/prove.sh::%s\n' "$kind" "$*"
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
# against planted inputs and stages 4, 4b, 4c and 6 can use them against the
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
            trigger_why="      - the '$REQUIRED_TRIGGER' trigger carries the filter key(s) ${keys% }, which narrow which pull requests fire this workflow. That narrowing is a deliberate change to when gate 7 runs, so it needs a deliberate re-proof rather than a harness deciding which pull requests are allowed to skip the gate"
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

# Answer, from those same facts, how deep the checkout of a named job is. Sets
# DEPTH_STATE to the value of `fetch-depth` under a step's `with:`, or to
# `absent` when no step of that job sets it, or to `ambiguous` when the steps
# of that job disagree.
#
# WHY THIS EXISTS. It is the one thing in this workflow that decides whether
# gate 7's secret scan has a history to read. `actions/checkout` defaults to a
# depth of 1, and a repository cloned that way holds one commit, so a scan of
# every blob reachable from a ref reads the tree and reports the history clean.
# secret-scan.sh refuses rather than reporting that, so the wrong value here is
# a red job; this check exists so the reason is named in this harness's output
# rather than met as a surprise refusal in another job's log.
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

# Remove the colour codes a tool writes for a terminal. cargo-deny and
# cargo-audit colour their output whether or not they are writing to one, and
# the workflow sets CARGO_TERM_COLOR=always on top of that, so an anchored
# signature matches nothing on the raw bytes: the line really begins with an
# escape sequence. The signatures above are written against what a human reads,
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
#   1 a signature did not match: this is not the answer this proof planted
#   2 a signature is not a usable regular expression
#
# A signature that begins with `!` must NOT match. Several rows here are about
# what the output does not say, which is the only way to attribute a check that
# stayed clean while the one beside it failed.
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

# ---------------------------------------------------------------------------
# Stage 1: what is running this
# ---------------------------------------------------------------------------

STAGE='reporting the environment'
rule 'Environment'
say "gate 7 proof harness, ORI-T-0016, spec/CI_CD.md section 1 gate 7"
say "  repository root : $REPO_ROOT"
say "  fixture root    : $HERE"
say "  platform        : $(uname -s 2>/dev/null || echo unknown) $(uname -m 2>/dev/null || echo unknown)"
if [ -n "${GITHUB_ACTIONS:-}" ]; then
    say "  running under   : GitHub Actions, runner OS ${RUNNER_OS:-unknown}, event ${GITHUB_EVENT_NAME:-unknown}"
else
    say "  running under   : a local shell"
fi

# ---------------------------------------------------------------------------
# Stage 2: the tooling. A missing tool makes gate 7 unrunnable, and a harness
# that cannot run the gate must say so rather than report a pass.
# scripts/gates.sh carries the same distinction and the same reason.
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
        emit error "'$*' exited $status, so $label is not usable here and no verdict of gate 7 on this machine would mean anything"
        probe_failed=1
        return
    fi
    printf '  %-16s %s\n' "$label" "${out%%$'\n'*}"
}
probe 'cargo'       cargo --version
probe 'cargo-audit' cargo audit --version
probe 'cargo-deny'  cargo deny --version
probe 'git'         git --version
# awk reads the workflow file. Probed by running a program rather than by
# asking for a version banner, because the three awks this has to run under
# disagree about which version flag they take and agree about this.
probe 'awk'         awk 'BEGIN { print "a usable POSIX awk" }'
if [ "$probe_failed" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "nothing was checked: the tooling this proof needs is not usable on this machine. spec/ENV_SETUP.md section 1 lists cargo-audit and cargo-deny as gate tools installed by scripts/setup-dev.sh. This is not a pass and not a failure of gate 7."
    exit 3
fi

WORK_DIR="$(mktemp -d)"
if [ ! -d "$WORK_DIR" ]; then
    VERDICT_REACHED=1
    emit error "could not create a temporary directory to hold the gate output, so nothing was run"
    exit 3
fi

# The advisory database is not on this machine; cargo-audit fetches it from
# RustSec over the network. A run that cannot reach it answers differently from
# a run that can, and the difference is not a fact about gate 7.
#
# WHY THIS PROBE EXISTS, AND WHAT IT PREVENTS. Without it, an unreachable
# database makes cargo-audit exit non-zero on the CLEAN lockfile, the comparator
# reads that as a disagreement, and this harness reports "gate 7 did not behave
# the way ops/gates/gate-7.md claims" when what actually happened was a network
# outage. That is this harness attributing a failure to the gate for a reason
# the gate had nothing to do with, which is the same class of defect as
# attributing a success to it. "I could not check" is a third answer and it
# exits 3.
#
# The probe is a real run of the real command over the clean fixture, so it also
# warms the database cache and makes every run below read the same one.
STAGE='checking that the advisory database can be loaded'
rule 'The RustSec advisory database'
probe_log="$WORK_DIR/advisory-db.probe"
( cd "$HERE/advisory-clean" || exit 255; "${AUDIT_ARGV[@]}" ) >"$probe_log" 2>&1
probe_status=$?
strip_ansi "$probe_log" "$probe_log.plain"
sed 's/^/  | /' "$probe_log.plain"
if ! grep -Eq '^ *Loaded [0-9]{3,} security advisor' "$probe_log.plain"; then
    VERDICT_REACHED=1
    emit error "cargo-audit did not report loading an advisory database here (it exited $probe_status). Everything gate 7's advisory half says is a comparison against that database, so without it a clean answer means the database was empty and a failing answer means the fetch failed. Neither is a fact about gate 7. This machine is probably offline or cannot reach https://github.com/RustSec/advisory-db.git. Nothing was checked, and this is not a pass and not a failure of gate 7."
    exit 3
fi
say ''
say '  The database loaded. Every advisory verdict below is a comparison against it,'
say '  and the case table requires that line in the output of both advisory rows, so'
say '  a database that emptied later in this run could not produce a clean answer.'

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

# All three verdicts must be represented, and so must all three commands the
# `gate-7` job runs. A table of failures only cannot detect a gate that fails
# on everything; a table of passes only cannot detect a gate that passes on
# everything, which is the AICD §14 defect class itself; and a table that
# forgot one of the three would report on two of them while the third ran
# unwatched. That applies to the advisory scanner as much as to the two gate
# commands: it is not a gate, and a step that runs unwatched is still worse
# than a step that is watched.
expect_pass=0
expect_fail=0
expect_refuse=0
have_advisories=0
have_licenses=0
have_secrets=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    case "${CASE_EXP[$i]}" in
        pass) expect_pass=$((expect_pass + 1)) ;;
        fail|refuse)
            if [ "${CASE_EXP[$i]}" = 'fail' ]; then
                expect_fail=$((expect_fail + 1))
            else
                expect_refuse=$((expect_refuse + 1))
            fi
            # A row that expects a non-zero answer and names no signature is a
            # row that would accept any non-zero answer at all, which is the
            # hole this stage exists to close.
            if [ -z "${CASE_SIG[$i]//$'\n'/}" ]; then
                note_bad "case ${CASE_ID[$i]} expects gate 7 to answer '${CASE_EXP[$i]}' and names no signature, so any non-zero answer whatever would be recorded as the planted defect being caught"
            fi
            ;;
        *) note_bad "case ${CASE_ID[$i]} expects '${CASE_EXP[$i]}', which is none of 'pass', 'fail', 'refuse'" ;;
    esac
    case "${CASE_CHECK[$i]}" in
        advisories) have_advisories=$((have_advisories + 1)) ;;
        licenses)   have_licenses=$((have_licenses + 1)) ;;
        secrets)    have_secrets=$((have_secrets + 1)) ;;
        *) note_bad "case ${CASE_ID[$i]} belongs to check '${CASE_CHECK[$i]}', which is none of gate 7's three" ;;
    esac
    i=$((i + 1))
done
say "  case table       : $CASE_COUNT cases, $expect_pass pass, $expect_fail fail, $expect_refuse refuse"
say "  by check         : advisories $have_advisories, licenses and bans $have_licenses, secret scan $have_secrets"
if [ "$expect_pass" -eq 0 ]; then
    note_bad "no case expects gate 7 to pass, so this run could not tell a working gate from one that fails on every input"
fi
if [ "$expect_fail" -eq 0 ]; then
    note_bad "no case expects gate 7 to fail, so this run could not tell a working gate from one that passes on every input, which is the defect class of AICD §14"
fi
if [ "$have_advisories" -eq 0 ] || [ "$have_licenses" -eq 0 ] || [ "$have_secrets" -eq 0 ]; then
    note_bad "the 'gate-7' job runs three commands and this table does not exercise all three (advisories $have_advisories, licenses $have_licenses, secrets $have_secrets). Gate 7 is two installed checks and one that is not installed (E-0003); this table has to cover all three commands anyway, because an unexercised command is one nothing watches, and the row that goes missing is the row nobody misses."
fi

# The planted inputs must be present and must be what they claim to be.
for lock in advisory-clean advisory-defect; do
    if [ ! -f "$HERE/$lock/Cargo.lock" ]; then
        note_bad "$lock/Cargo.lock is missing, so the input this proof claims to present to cargo-audit is not there"
    elif [ -f "$HERE/$lock/Cargo.toml" ]; then
        note_bad "$lock/Cargo.toml exists. These fixtures are lockfiles with no manifest on purpose: nothing may be able to resolve or build them, because the crates they name are not dependencies of this repository and adding one is an escalation trigger."
    fi
done
for ws in deny-clean deny-license-defect deny-bans-defect; do
    for required in 'Cargo.toml' 'root/Cargo.toml' 'dep/Cargo.toml'; do
        if [ ! -f "$HERE/$ws/$required" ]; then
            note_bad "$ws/$required is missing, so the input this proof claims to present to cargo-deny is not there"
        fi
    done
    if [ -f "$HERE/$ws/Cargo.toml" ] && ! grep -q '^\[workspace\]' "$HERE/$ws/Cargo.toml"; then
        note_bad "$ws/Cargo.toml has no [workspace] table, so this package is not its own workspace root and the repository workspace may resolve it"
    fi
done

if [ ! -f "$DENY_CONFIG" ]; then
    note_bad "$DENY_CONFIG_REL is missing from the repository root, so cargo-deny has no policy to enforce and every licenses verdict below would be about cargo-deny's defaults rather than about this repository's policy"
fi
if [ -f "$HERE/deny.toml" ]; then
    note_bad "a second deny.toml is back under fixtures/planted/gate-7/. Ruling R28 puts the dependency policy at the repository root, where cargo-deny looks by default; two copies is one copy nobody reads."
fi
if [ ! -f "$SCANNER" ]; then
    note_bad "$SCANNER_REL is missing, so gate 7's third check does not exist and cannot be proven"
fi
if [ -f "$HERE/secret-scan.sh" ]; then
    note_bad "a second secret-scan.sh is back under fixtures/planted/gate-7/. Ruling R28 puts it beside the other scripts a gate runs; a gate whose implementation has two copies is a gate that can be repaired in the copy CI does not run."
fi
if [ ! -f "$FAKE_SAMPLE" ]; then
    note_bad "secret-samples/declared-fake.env is missing, so both halves of the secret fixture are gone: the declared fake this scan must exempt and the caught secret derived from it"
fi

# The local gate set must run this gate too, and this is the check that says so.
#
# WHY IT IS HERE. CLAUDE.md step 4 makes `scripts/gates.sh` the gate set a coder
# runs before opening a pull request. Attempt 1 wired gate 7 into CI and left
# that script reporting it "not available", with three reasons the same ticket
# had falsified: that the policy file was absent, that an advisory check here
# "could not be seen to fail on a planted defect", and that the secret scan had
# no local runner. All three were false, in the one place a coder would have
# believed them, and nothing in this harness noticed. It notices now: a gate
# that CI runs and the local gate set declines to run is a gate a coder cannot
# run, whatever this proof says about CI.
GATES_SH="$REPO_ROOT/scripts/gates.sh"
if [ ! -f "$GATES_SH" ]; then
    note_bad "scripts/gates.sh does not exist, so there is no local gate set for a coder to run gate 7 in (CLAUDE.md step 4)"
else
    if ! grep -q "GATE_RUNNER\[7\]='local'" "$GATES_SH"; then
        note_bad "scripts/gates.sh does not declare a local runner for gate 7. CLAUDE.md step 4 makes that script the gate set a coder runs before opening a pull request, so gate 7 would be unrunnable at a desk while this file proves it in CI."
    fi
    if grep -q 'probe_unavailable 7' "$GATES_SH"; then
        note_bad "scripts/gates.sh still probes gate 7 for availability instead of running it. That probe's reasons are hard-coded sentences, and the last set of them was falsified by the ticket that wired this gate."
    fi
    if ! grep -q "$SCANNER_REL" "$GATES_SH"; then
        note_bad "scripts/gates.sh never mentions $SCANNER_REL, so the secret scan is not part of the local gate set and a coder's last check before a pull request reads neither the tree nor the history for credentials"
    fi
    if ! grep -q -- "--config $DENY_CONFIG_REL" "$GATES_SH"; then
        note_bad "scripts/gates.sh does not pass --config $DENY_CONFIG_REL to cargo-deny, so whatever it enforces locally is not the policy in this repository"
    fi
    say "  local gate set   : scripts/gates.sh declares a local runner for gate 7 and names both $SCANNER_REL and $DENY_CONFIG_REL"
fi

# The root workspace must not reach into the fixture. A member under
# `fixtures/` would put a planted defect into the real build.
if [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
    note_bad "$REPO_ROOT/Cargo.toml does not exist, so REPO_ROOT was resolved wrongly and nothing below can be trusted"
elif grep -q 'fixtures' "$REPO_ROOT/Cargo.toml"; then
    note_bad "the root Cargo.toml mentions 'fixtures'. If a planted package has become a workspace member the real build no longer stays green, and this proof is no longer proving what it says."
else
    say "  root workspace   : names no member under fixtures/"
fi

# The parser and its planted workflows.
if [ ! -f "$AWK_PARSER" ]; then
    note_bad "$AWK_PARSER is missing, so this harness cannot read the workflow file and cannot tell whether it is proving commands anything runs"
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
# Four of the ten rows are planted to be PASSED, and a pass looks the same
# whether the plant is still there or not. ORI-T-0014 paid for this lesson in
# gate 2's fixture. Each assertion below is about the fixture source, not about
# what a tool said.

STAGE='checking that every planted input is still planted'
rule 'The plants are still planted'
plant_bad=0
plant_check() {
    # $1 description, $2 result (0 good), $3 what a failure means.
    local desc="$1" ok="$2" why="$3"
    if [ "$ok" -eq 0 ]; then
        printf '  %-58s %s\n' "$desc" 'yes'
    else
        printf '  %-58s %s\n' "$desc" 'NO'
        emit error "$why"
        plant_bad=1
    fi
}

grep -q '^name = "time"' "$HERE/advisory-defect/Cargo.lock" && \
    grep -q '^version = "0.1.44"' "$HERE/advisory-defect/Cargo.lock"
plant_check 'advisory-defect names time 0.1.44' $? \
    "advisory-defect/Cargo.lock no longer names time 0.1.44, the version RUSTSEC-2020-0071 is against. Whatever cargo-audit says about it now, it is not the planted advisory."

grep -qE '^source = "registry\+' "$HERE/advisory-defect/Cargo.lock"
plant_check 'advisory-defect names a registry package' $? \
    "advisory-defect/Cargo.lock names no package with a registry source, so cargo-audit has no third-party crate to judge and would report it clean for a reason that is not the gate working."

grep -qE '^source = "registry\+' "$HERE/advisory-clean/Cargo.lock"
plant_check 'advisory-clean names a registry package too' $? \
    "advisory-clean/Cargo.lock names no package with a registry source. A clean input with nothing in it cannot tell a working cargo-audit from one that exits 0 on everything."

grep -q 'license = "GPL-3.0-only"' "$HERE/deny-license-defect/dep/Cargo.toml"
plant_check 'deny-license-defect carries a copyleft license' $? \
    "deny-license-defect/dep/Cargo.toml no longer declares a license deny.toml rejects, so the licenses check has nothing planted to catch."

grep -qE 'version = "\*"' "$HERE/deny-bans-defect/root/Cargo.toml"
plant_check 'deny-bans-defect carries a wildcard requirement' $? \
    "deny-bans-defect/root/Cargo.toml no longer carries a wildcard version requirement, so the bans check has nothing planted to catch."

grep -q 'license = "Apache-2.0"' "$HERE/deny-clean/dep/Cargo.toml"
plant_check 'deny-clean carries an allowed license' $? \
    "deny-clean/dep/Cargo.toml no longer declares Apache-2.0. A clean input the policy would reject anyway proves nothing about the policy."

grep -q 'allow = \[' "$DENY_CONFIG" && grep -q '"Apache-2.0"' "$DENY_CONFIG"
plant_check 'deny.toml states a license allow list' $? \
    "$DENY_CONFIG_REL no longer states an allow list containing Apache-2.0, so the licenses check is not enforcing spec/CONVENTIONS.md's rule and a clean verdict from it means nothing."

grep -q "$MARKER" "$FAKE_SAMPLE"
plant_check 'the declared-fake sample still carries the marker' $? \
    "secret-samples/declared-fake.env no longer carries the declared-fake marker, so the row that proves the scan exempts a declared fake would be exempting nothing and passing for the wrong reason."

# Six lines of the sample must be secret-shaped, one per pattern. Counting them
# here is what makes the `declared fakes : 18` signature below an assertion
# about the fixture rather than a number somebody copied from a log.
fake_values="$(grep -c "^[A-Z_]*=.*$MARKER\|^[A-Z_]*=\"-----BEGIN $MARKER" "$FAKE_SAMPLE" || true)"
[ "${fake_values:-0}" -ge 6 ]
plant_check "the declared-fake sample carries at least 6 marked values (${fake_values:-0})" $? \
    "secret-samples/declared-fake.env carries ${fake_values:-0} marked values and the case table's signatures are written for six. Either the sample lost a line or a pattern lost its exemplar, and in both cases part of the scan is no longer exercised."

grep -q "MARKER=\"\${MARKER}FAKE\"" "$SCANNER"
plant_check 'the scanner assembles the marker rather than spelling it' $? \
    "$SCANNER_REL no longer assembles the marker from two halves. Spelled out beside a pattern it would declare that pattern's own text a fake, which is a hole in the scanner rather than in the fixture."

say ''
if [ "$plant_bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "at least one planted input is no longer planted, so the rows that depend on it would agree with the case table while demonstrating nothing. Repair the fixture and re-run. Nothing was proven."
    exit 2
fi
say '  Every planted input above is still there. Four of the ten rows below are'
say '  passes, and a pass looks identical whether its plant is present or not, so'
say '  these assertions are the only thing standing between those rows and a'
say '  proof that proves nothing.'

# --- stage 3c: the self-check of the liveness check ------------------------
#
# The same argument as the flipped comparator. A liveness check that answered
# "live" to everything would report the gate proved however the workflow was
# weakened; one that answered "dead" to everything would block every run. The
# table below contains all three answers, so neither constant survives it.

STAGE='checking that this harness can tell a live gate command from a dead one'
rule 'Self-check: telling a live gate command from a dead one'
say 'Each file below is a whole workflow carrying exactly one way of switching'
say 'gate 7 off, or none. They are planted defects for the check stage 4 is'
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
    classify_command "$facts" "$AUDIT_CMD" "$AGGREGATE_JOB"
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
say "Gate 7's secret scan is the only check in this repository whose reach"
say 'depends on an input to an action rather than on a command. Every answer'
say 'the reader can give has a planted workflow here that is owed it.'
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
# One text, three patterns, three different answers owed: one the text carries,
# one it does not, and one negated signature that must be read as satisfied
# BECAUSE the text does not carry it. A matcher that always agreed, or always
# refused, fails one of the three. The negated row is here because gate 1's
# harness had only positive signatures and several rows in this fixture turn on
# what the output does NOT say.

STAGE='checking that this harness can tell one failure from another'
rule 'Self-check: telling the planted answer from any other answer'
selfcheck_log="$WORK_DIR/attribution.selfcheck"
printf '%s\n' 'bans ok, licenses FAILED, sources ok' >"$selfcheck_log"
say "  text        : $(cat "$selfcheck_log")"

output_matches "$selfcheck_log" $'^bans ok, licenses FAILED, sources ok$\n'
present=$?
output_matches "$selfcheck_log" $'^error\\[wildcard\\]\n'
absent=$?
output_matches "$selfcheck_log" $'!^bans FAILED\n'
negated_ok=$?
output_matches "$selfcheck_log" $'!licenses FAILED\n'
negated_violated=$?
printf '  %-52s %s\n' 'a signature the text carries' "match=$present (0 owed)"
printf '  %-52s %s\n' 'a signature the text does not carry' "match=$absent (1 owed)"
printf '  %-52s %s\n' 'a NOT-signature the text satisfies' "match=$negated_ok (0 owed)"
printf '  %-52s %s\n' 'a NOT-signature the text violates' "match=$negated_violated (1 owed)"
if [ "$present" -ne 0 ] || [ "$absent" -ne 1 ] || [ "$negated_ok" -ne 0 ] || [ "$negated_violated" -ne 1 ]; then
    VERDICT_REACHED=1
    emit error "this harness cannot tell one answer from another: it answered $present, $absent, $negated_ok, $negated_violated where 0, 1, 0, 1 are owed. Every attribution below would be meaningless."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stage 4: the commands under proof must be commands CI runs, in a job whose
# failure fails the run. Without this, gate 7 could be switched off in ci.yml
# while this harness went on proving commands nothing executes.
# ---------------------------------------------------------------------------

STAGE='checking that the workflow runs the commands under proof'
rule 'The three commands under proof are commands CI runs'

if [ ! -f "$REPO_ROOT/$WORKFLOW_REL" ]; then
    VERDICT_REACHED=1
    emit error "$WORKFLOW_REL does not exist, so this harness cannot confirm that it proves commands CI actually runs. Nothing was proven."
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
for cmd in "$AUDIT_CMD" "$DENY_CMD" "$SCAN_CMD"; do
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
            note_bad "'$cmd' is not run by any job of $WORKFLOW_REL whose failure would fail this run. Either gate 7 was weakened, or this harness is proving a command nothing executes. A proof of a command nothing runs is the defect class of AICD §39, so it is refused rather than reported."
            ;;
    esac
done

if [ "$bad" -ne 0 ]; then
    VERDICT_REACHED=1
    emit error "a command under proof is not one CI runs where its failure fails the run, so no gate command was run here. Nothing was proven."
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
            emit error "no job of $WORKFLOW_REL runs this harness ('$SELF_CMD'), so nothing in CI presents these planted defects to gate 7 and this run says nothing about any commit but this one. A proof CI does not run is the defect class of AICD §39. Nothing was proven."
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
# Stage 4c: and the job that runs gate 7 must check out a history to scan.
# ---------------------------------------------------------------------------

STAGE='checking that the gate job checks out a history to scan'
rule 'The gate job checks out the whole history'
checkout_depth "$workflow_facts" "$GATE_JOB"
say "  job '$GATE_JOB' fetch-depth: $DEPTH_STATE (required: $REQUIRED_FETCH_DEPTH)"
say "$DEPTH_WHY"
if [ "$DEPTH_STATE" != "$REQUIRED_FETCH_DEPTH" ]; then
    VERDICT_REACHED=1
    emit error "the '$GATE_JOB' job of $WORKFLOW_REL checks out with fetch-depth '$DEPTH_STATE' and gate 7's secret scan needs '$REQUIRED_FETCH_DEPTH'. spec/CI_CD.md section 1 gate 7 and spec/ENV_SETUP.md section 4 both say the scan covers tree AND history; actions/checkout's default of 1 puts one commit on the runner, and a history scan over one commit is a scan of the tree under another name. secret-scan.sh refuses a shallow repository rather than reporting it clean, so this would be a red job rather than a silent pass, but it would be a red job for a reason nobody had written down. Nothing was proven."
    exit 2
fi
say ''
say "  That is what puts a history on the runner. It is also the only reason the"
say "  'secret-in-history' row below means anything in CI: without it, the scanner"
say "  refuses on every run and the history half of gate 7 is never exercised on this"
say "  repository at all."

# ---------------------------------------------------------------------------
# Stage 5: observation. Run each case once. Record the exit status and the
# output. Judge nothing yet.
#
# The secret rows need their inputs built first: four throwaway git
# repositories, made from one shipped file, in a temporary directory that goes
# away with this run.
# ---------------------------------------------------------------------------

STAGE='building the planted repositories for the secret scan'
rule 'Building the planted repositories'

REPO_DIR="$WORK_DIR/repos"
mkdir -p "$REPO_DIR"

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

# The size of the large binary, in bytes: 16 MiB. It is written here once so
# that the number in ops/gates/gate-7.md and the number this run actually
# scanned are the same number.
LARGE_BYTES=16777216

nul_wrapped() {
    # $1 destination, $2 the env file whose bytes go inside it.
    #
    # WHAT THIS BUILDS, SAID ACCURATELY. Ordinary ASCII text with NUL bytes on
    # either side of it, behind four bytes that look like a Java keystore's
    # magic number, written out so a reader can see what is being imitated.
    # This function was called `keystore()` and wrote `keystore.p12`. It is NOT
    # a keystore and NOT a .p12, and both names were renamed rather than
    # annotated, because a file name is read by people who never reach the
    # comment underneath it.
    #
    # WHAT IT IS A FAITHFUL STAND-IN FOR. A compiled object with a token baked
    # into it, and a SQLite file, which stores text columns as UTF-8. Both keep
    # the credential as printable ASCII with non-printable bytes around it, and
    # that is exactly this shape. Both were built for real and both are caught.
    #
    # WHAT IT IS NOT A STAND-IN FOR, and an earlier version of this comment
    # said it was: a real .p12 or a real JKS. Those store the key DER-encoded
    # and encrypted, so the credential never appears as a printable run and the
    # scanner reports exit 0 on them with the key in the file. Both were built
    # for real, with openssl, and both came back clean. A fixture that imitates
    # the easy half of a file format and is described as the format is a claim
    # of coverage nobody re-derived, which is the defect class this whole
    # directory exists to catch. ops/gates/gate-7.md carries the output.
    #
    # The NUL byte on either side is still the whole trick for the defect these
    # rows were added for: it is all `grep -I` needed to report no match
    # without reading the file.
    printf 'JKS\000\000\000\002\000' >"$1" || return 1
    cat "$2" >>"$1" || return 1
    printf '\000\001\002\000end\n' >>"$1" || return 1
    return 0
}

large_filler() {
    # $1 destination. Exactly LARGE_BYTES bytes of mostly non-printable filler,
    # built by doubling a short seed rather than read from /dev/urandom, so
    # that two runs of this proof scan the same bytes and a finding here is
    # never a coincidence somebody has to reproduce.
    local out="$1" seed="$WORK_DIR/filler.seed" cur="$WORK_DIR/filler.cur"
    printf 'ORI\000binary\000filler\000block\000\377\376\375\000\001\002\003\004\005\006\007\010\013\014\016\017\020\021\022\023\024\025\026\027\030\031\032\033\034\035\036\037' >"$seed" || return 1
    cp "$seed" "$cur" || return 1
    while [ "$(wc -c <"$cur")" -lt "$LARGE_BYTES" ]; do
        cat "$cur" "$cur" >"$cur.2" || return 1
        mv "$cur.2" "$cur" || return 1
    done
    head -c "$LARGE_BYTES" <"$cur" >"$out" || return 1
    return 0
}

new_repo() {
    # $1 name. A repository with one ordinary commit and nothing else.
    local name="$1" d="$REPO_DIR/$1"
    mkdir -p "$d" || return 1
    git_quiet git -C "$d" init -q || return 1
    git_quiet git -C "$d" config user.email 'gate-7@example.invalid' || return 1
    git_quiet git -C "$d" config user.name 'gate 7 fixture' || return 1
    git_quiet git -C "$d" config commit.gpgsign false || return 1
    printf 'A planted repository for gate 7 (ORI-T-0016).\n' >"$d/README.md" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'base' || return 1
    return 0
}

repo_build_failed=0
build_repos() {
    local d

    # clean: nothing secret-shaped anywhere.
    new_repo clean || return 1

    # declared-fake: the shipped sample, committed as it is. Every value in it
    # matches a pattern and every one carries the marker, so the scan must find
    # them all and exempt them all.
    new_repo declared-fake || return 1
    d="$REPO_DIR/declared-fake"
    cp "$FAKE_SAMPLE" "$d/config.env" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'a fixture env file with declared fake keys' || return 1

    # tree-secret: the same file with the marker replaced by eleven other
    # characters. Same shapes, same lengths, nothing declared.
    new_repo tree-secret || return 1
    d="$REPO_DIR/tree-secret"
    sed "s/$MARKER/$FILLER/g" "$FAKE_SAMPLE" >"$d/config.env" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'a credential nobody declared' || return 1

    # history-secret: the same, committed and then deleted. The tree is clean.
    # The blob is still reachable, which is what a clone hands the next person.
    new_repo history-secret || return 1
    d="$REPO_DIR/history-secret"
    sed "s/$MARKER/$FILLER/g" "$FAKE_SAMPLE" >"$d/config.env" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'a credential nobody declared' || return 1
    rm -f "$d/config.env" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'remove it, which repairs nothing' || return 1

    # shallow: a depth-1 clone of the one above, which is the shape
    # actions/checkout produces by default. `file://` is required, because git
    # ignores --depth for a local path clone.
    git_quiet git clone -q --depth 1 "file://$REPO_DIR/history-secret" "$REPO_DIR/shallow" || return 1

    # The four repositories about binary content. Everything secret in them is
    # the same derived file as above, so nothing new is shipped and nothing in
    # this fixture holds an undeclared credential at rest.
    local derived="$WORK_DIR/derived.env"
    sed "s/$MARKER/$FILLER/g" "$FAKE_SAMPLE" >"$derived" || return 1

    # nul-declared-fake: the DECLARED sample inside a file with NUL bytes. The
    # control: every value must be found and every value exempted, which is how
    # a scan that reads binaries is told from one that fails on them.
    new_repo nul-declared-fake || return 1
    d="$REPO_DIR/nul-declared-fake"
    nul_wrapped "$d/ascii-in-nul-bytes.bin" "$FAKE_SAMPLE" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'declared fakes wrapped in NUL bytes' || return 1

    # nul-tree-secret: the derived sample inside the same shape of file.
    new_repo nul-tree-secret || return 1
    d="$REPO_DIR/nul-tree-secret"
    nul_wrapped "$d/ascii-in-nul-bytes.bin" "$derived" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'a credential wrapped in NUL bytes, nobody declared' || return 1

    # nul-history-secret: the same, committed and then deleted. Under `grep -I`
    # this repository was clean twice over: nothing binary in the tree to see,
    # and a history half that answered "no match" without reading the blob.
    new_repo nul-history-secret || return 1
    d="$REPO_DIR/nul-history-secret"
    nul_wrapped "$d/ascii-in-nul-bytes.bin" "$derived" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'a credential wrapped in NUL bytes, nobody declared' || return 1
    rm -f "$d/ascii-in-nul-bytes.bin" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'remove it, which repairs nothing' || return 1

    # large-binary: 16 MiB of filler with the same values baked in, which is
    # the shape of a compiled artifact somebody committed.
    new_repo large-binary || return 1
    d="$REPO_DIR/large-binary"
    large_filler "$d/blob.bin" || return 1
    cat "$derived" >>"$d/blob.bin" || return 1
    printf '\000\001\002\000end\n' >>"$d/blob.bin" || return 1
    git_quiet git -C "$d" add -A || return 1
    git_quiet git -C "$d" commit -q -m 'a build artifact with a credential in it' || return 1

    return 0
}

if ! build_repos; then
    VERDICT_REACHED=1
    emit error "the planted repositories for gate 7's secret scan could not be built, so the third of the gate that covers tree and history was not exercised at all. This is a broken fixture, not a verdict about gate 7. Nothing was proven."
    exit 3
fi

for r in clean declared-fake tree-secret history-secret shallow \
         nul-declared-fake nul-tree-secret nul-history-secret large-binary; do
    tracked="$(git -C "$REPO_DIR/$r" ls-files 2>/dev/null | grep -c . || true)"
    commits="$(git -C "$REPO_DIR/$r" rev-list --count --all 2>/dev/null || printf '?')"
    shallow_state="$(git -C "$REPO_DIR/$r" rev-parse --is-shallow-repository 2>/dev/null || printf '?')"
    printf '  %-18s %s tracked file(s), %s commit(s), shallow=%s\n' "$r" "${tracked:-0}" "$commits" "$shallow_state"
done

# The shallow clone must actually be shallow, or the row that proves the
# scanner refuses one is presenting it with an ordinary repository.
if [ "$(git -C "$REPO_DIR/shallow" rev-parse --is-shallow-repository 2>/dev/null)" != 'true' ]; then
    VERDICT_REACHED=1
    emit error "the repository built to be shallow is not shallow, so the row that proves the scanner refuses a shallow clone would be presenting it with a full one and passing for the wrong reason. This git may ignore --depth for a file:// clone. Nothing was proven."
    exit 3
fi

# The four binary plants must actually be binary. A file that lost its NUL byte
# is scanned as text and found anyway, so the row would still say 'fail' while
# demonstrating nothing about the defect it was planted for. This asserts the
# input rather than trusting the verdict, which is the same argument as stage
# 3b one level down.
nul_probe="$WORK_DIR/nul.probe"
for pair in 'nul-declared-fake/ascii-in-nul-bytes.bin' 'nul-tree-secret/ascii-in-nul-bytes.bin' 'large-binary/blob.bin'; do
    f="$REPO_DIR/$pair"
    if [ ! -f "$f" ]; then
        VERDICT_REACHED=1
        emit error "the planted binary $pair was not built, so the rows about content a scanner is tempted to skip have no input. Nothing was proven about them."
        exit 3
    fi
    total="$(wc -c <"$f")"
    if ! LC_ALL=C tr -d '\000' <"$f" >"$nul_probe" 2>/dev/null; then
        VERDICT_REACHED=1
        emit error "tr could not read the planted binary $pair, so this harness cannot show that it holds a NUL byte and cannot claim the rows below are about binary content."
        exit 3
    fi
    stripped="$(wc -c <"$nul_probe")"
    if [ "$((total))" -eq "$((stripped))" ]; then
        VERDICT_REACHED=1
        emit error "the planted file $pair holds no NUL byte, so it is ordinary text and the scanner would find its contents whether or not it reads binary files. The rows planted for the grep -I defect would pass while demonstrating nothing. Nothing was proven."
        exit 3
    fi
    printf '  %-34s %s byte(s), holds a NUL byte\n' "$pair" "$((total))"
done

large_size="$(wc -c <"$REPO_DIR/large-binary/blob.bin")"
if [ "$((large_size))" -lt "$LARGE_BYTES" ]; then
    VERDICT_REACHED=1
    emit error "large-binary/blob.bin is $((large_size)) bytes and this proof claims at least $LARGE_BYTES. The row about what scanning every byte of a large artifact costs would be measuring something smaller than it says. Nothing was proven about it."
    exit 3
fi
say "  large-binary/blob.bin              $((large_size)) byte(s), at least the $LARGE_BYTES this proof claims"

if ! git -C "$REPO_DIR/nul-history-secret" ls-files 2>/dev/null | grep -q -v 'ascii-in-nul-bytes'; then
    VERDICT_REACHED=1
    emit error "nul-history-secret tracks nothing but the NUL-wrapped file, so its tree half has no ordinary file and the row that turns on 'tree clean, history not' is not the shape it claims. Nothing was proven about it."
    exit 3
fi
if git -C "$REPO_DIR/nul-history-secret" ls-files 2>/dev/null | grep -q 'ascii-in-nul-bytes'; then
    VERDICT_REACHED=1
    emit error "nul-history-secret still tracks the NUL-wrapped file at HEAD, so the binary is in the tree and the row that proves the history half reads binary blobs would be proving the tree half instead. Nothing was proven about it."
    exit 3
fi

STAGE='running gate 7 against every input'

run_gate() {
    # $1 check, $2 input, $3 output file. Returns the gate command's own exit
    # status. No pipeline: the output is redirected to a file and nothing else
    # reads the status first.
    #
    # 255 is reserved for "the command did not run at all", so that a `cd` that
    # failed can never be recorded as gate 7 reporting a failure. Neither
    # cargo-audit, cargo-deny nor the scanner exits 255.
    local check="$1" input="$2" out="$3"
    local status
    case "$check" in
        advisories)
            ( cd "$HERE/$input" || exit 255; "${AUDIT_ARGV[@]}" ) >"$out" 2>&1
            status=$?
            ;;
        licenses)
            ( cd "$HERE/$input" || exit 255
              cargo deny --manifest-path Cargo.toml --config "$DENY_CONFIG" check licenses bans sources ) >"$out" 2>&1
            status=$?
            ;;
        secrets)
            bash "$SCANNER" --repo "$REPO_DIR/$input" >"$out" 2>&1
            status=$?
            ;;
        *)
            emit error "unknown check '$check' in the case table"
            return 255
            ;;
    esac
    return "$status"
}

rule 'Observation: gate 7 run against each input, once'
say 'Each command below runs once. Its exit status is captured on the next line,'
say 'with no pipeline between, and nothing is judged until stage 7.'
say ''
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    id="${CASE_ID[$i]}"
    chk="${CASE_CHECK[$i]}"
    input="${CASE_INPUT[$i]}"
    log="$WORK_DIR/$id.log"
    case "$chk" in
        advisories) shown="( cd fixtures/planted/gate-7/$input && $AUDIT_CMD )" ;;
        licenses)   shown="( cd fixtures/planted/gate-7/$input && cargo deny --manifest-path Cargo.toml --config <root>/$DENY_CONFIG_REL check licenses bans sources )" ;;
        secrets)    shown="bash $SCANNER_REL --repo <planted>/$input" ;;
        *)          shown='unknown' ;;
    esac

    say "--- $id ($chk)"
    say "    command    : $shown"
    run_gate "$chk" "$input" "$log"
    status=$?
    if [ "$status" -eq 255 ]; then
        VERDICT_REACHED=1
        emit error "the gate command for '$id' could not be started at all (status 255). That is not gate 7 reporting anything, and this harness will not record it as a verdict."
        exit 2
    fi
    # Three verdicts, not two. 3 is the scanner's "I could not scan", which is
    # neither a pass nor a catch, and reading it as either would be this gate's
    # own defect class inside its proof.
    case "$status" in
        0) verdict='pass' ;;
        3) if [ "$chk" = 'secrets' ]; then verdict='refuse'; else verdict='fail'; fi ;;
        *) verdict='fail' ;;
    esac
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
# below: those are the rows where gate 7 stopped catching something, and that
# is a gate failure (exit 1), not an unreadable observation (exit 2).
# ---------------------------------------------------------------------------

STAGE='checking that each answer is the answer this proof planted'
rule 'Attribution: the answer gate 7 gave is the answer to the planted question'
printf '  %-24s %-11s %-9s %s\n' 'input' 'check' 'observed' 'attributable to the planted mechanism'
printf '  %-24s %-11s %-9s %s\n' '------------------------' '-----------' '---------' '-------------------------------------'
attribution_bad=0
attribution_broken=0
i=0
while [ "$i" -lt "$CASE_COUNT" ]; do
    if [ "${OBS_VERDICT[$i]}" != "${CASE_EXP[$i]}" ]; then
        printf '  %-24s %-11s %-9s %s\n' \
            "${CASE_ID[$i]}" "${CASE_CHECK[$i]}" "${OBS_VERDICT[$i]}" \
            "not asked: the gate did not answer '${CASE_EXP[$i]}' here, which the table below reports"
        i=$((i + 1))
        continue
    fi
    output_matches "${OBS_LOG[$i]}" "${CASE_SIG[$i]}"
    status=$?
    case "$status" in
        0) printf '  %-24s %-11s %-9s %s\n' "${CASE_ID[$i]}" "${CASE_CHECK[$i]}" "${OBS_VERDICT[$i]}" 'yes' ;;
        1)
            printf '  %-24s %-11s %-9s %s\n' "${CASE_ID[$i]}" "${CASE_CHECK[$i]}" "${OBS_VERDICT[$i]}" "NO: $ATTRIB_WHY"
            attribution_bad=$((attribution_bad + 1))
            ;;
        *)
            printf '  %-24s %-11s %-9s %s\n' "${CASE_ID[$i]}" "${CASE_CHECK[$i]}" "${OBS_VERDICT[$i]}" "HARNESS: $ATTRIB_WHY"
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
    emit error "$attribution_bad case(s) gave gate 7's owed answer for a reason this proof did not plant. cargo-audit exits non-zero when it cannot reach the advisory database; cargo-deny exits non-zero when it cannot read a manifest; the scanner exits non-zero when it cannot scan. None of those is the gate catching anything, and a proof that counted them would be reporting success for a state that is not success. Read the attribution table above: it names the row and the signature that did not match. Two different things land here and the table tells them apart. Either a planted input has acquired a second thing wrong with it, and the repair is in the fixture; or the gate still answers 'fail' and no longer answers it for the planted reason, and the repair is in the gate. A secret scan that stopped reading history arrives exactly this way: it still catches the secret in the tree, and the count it prints for the history is no longer the planted one. Nothing was proven about gate 7 by this run."
    exit 2
fi

# ---------------------------------------------------------------------------
# Stages 7 and 8: judgement. One comparator, two expectation sets.
#
# Flipping a three-valued expectation needs a rule, and the rule is that the
# opposite of an answer is any other answer. `pass` flips to `fail`, `fail` to
# `pass`, and `refuse` flips to `pass`, because the only wrong answer that
# matters for the shallow row is the one that would let a history scan report
# clean having read no history.
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
    printf '  %-24s %-11s %-9s %-9s %-6s %s\n' 'input' 'check' 'expected' 'observed' 'exit' 'result'
    printf '  %-24s %-11s %-9s %-9s %-6s %s\n' '------------------------' '-----------' '---------' '---------' '------' '--------'
    i=0
    while [ "$i" -lt "$CASE_COUNT" ]; do
        expected="${CASE_EXP[$i]}"
        if [ "$mode" = 'inverted' ]; then
            case "$expected" in
                pass)   expected='fail' ;;
                fail)   expected='pass' ;;
                refuse) expected='pass' ;;
            esac
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
        printf '  %-24s %-11s %-9s %-9s %-6s %s\n' \
            "${CASE_ID[$i]}" "${CASE_CHECK[$i]}" "$expected" "$observed" "$status" "$agreement"
        i=$((i + 1))
    done
}

STAGE='judging the observations against the real expectations'
rule 'The proof: what gate 7 owes each input'
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
        non_complementary="$non_complementary ${CASE_ID[$i]}"
    fi
    i=$((i + 1))
done
say "  complementarity: direct $direct_mismatches disagreement(s), flipped $inverted_mismatches; the two must differ on every row"
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
    emit error "every one of the $CASE_COUNT cases disagreed, which no gate that always passes or always fails can produce: those still agree with part of the table. Look for an inverted comparison first, in this harness or in a gate command itself, before looking at the planted inputs."
fi

if [ "$direct_mismatches" -ne 0 ]; then
    gate_broken=1
    emit error "gate 7 did not behave the way ops/gates/gate-7.md claims: $direct_mismatches of $CASE_COUNT cases disagreed. Read the table above. A row expecting 'fail' that observed 'pass' means the check stopped catching something that is still planted in the tree, so gate 7 is no longer proven (AICD §14) and no document may cite it until it is re-proved. A row expecting 'refuse' that observed 'pass' is worse: it means the secret scan reported a repository clean whose history it could not read."
fi

VERDICT_REACHED=1

if [ "$harness_broken" -ne 0 ]; then
    exit 2
fi
if [ "$gate_broken" -ne 0 ]; then
    exit 1
fi

say "GATE 7 IS PARTIALLY INSTALLED. TWO CHECKS OF THREE."
say "  advisories (cargo-audit)        installed, and proven below"
say "  licences and bans (cargo-deny)  installed, and proven below"
say "  secret scan                     NOT INSTALLED. Escalation E-0003."
say ''
say "This script does not claim, and exit 0 from it does not mean, that gate 7's"
say "secret scan is installed. The secret rows below are evidence of what"
say "$SCANNER_REL does. They are not evidence that the check exists, because"
say "what that scanner misses is not among them and cannot be put among them: a"
say "pattern matcher over bytes is blind to every encoding it was not told about,"
say "and each repair opens the next hole. ops/gates/gate-7.md carries the"
say "reproduction: an AWS key written UTF-16, committed, scanned, exit 0."
say ''
say "ESTABLISHED BY THIS RUN:"
say "  - the three commands the '$GATE_JOB' job runs, two gate commands and one"
say "    advisory, are run by a job of $WORKFLOW_REL that an unfiltered"
say "    '$REQUIRED_TRIGGER' fires, with no 'if:' and no 'continue-on-error:' on the job"
say "    or on the steps, and '$AGGREGATE_JOB' waits on that job and carries no"
say "    'continue-on-error:' itself, so their failure fails this run;"
say "  - that job checks out with fetch-depth $REQUIRED_FETCH_DEPTH, so there is a history on the"
say "    runner for the secret scan to read;"
say "  - the '$SELF_JOB' job that runs this script is live by those same six"
say "    conditions, so the refusals above are refusals that fail this run too;"
say "  - every planted input is still planted, asserted against the fixture sources;"
say "  - cargo-audit failed a lockfile naming a real published advisory and passed one"
say "    naming none, with a loaded advisory database in both cases;"
say "  - cargo-deny failed a copyleft license the policy does not allow and a wildcard"
say "    version requirement, each in its own check with the other check staying clean,"
say "    and passed a workspace that violates neither;"
say "  - WHAT THE ADVISORY SCANNER DID, which is not the same as what gate 7 has:"
say "    it found a credential in a tree, found one whose file had been deleted and"
say "    survives only in the history, exempted every declared fake in the shipped"
say "    sample and nothing else, and REFUSED a shallow clone rather than reporting"
say "    it clean;"
say "  - it found the same credential inside a file holding NUL bytes, in the tree and"
say "    in the history separately, and inside a $LARGE_BYTES-byte binary, reading every"
say "    byte of each and reporting the count; and it exempted the declared fakes inside"
say "    a binary too, so what it does with binary content is scan it and not fail on"
say "    it. Under the grep -I this scanner used until attempt 2, all four of those"
say "    repositories were reported clean. That shape is printable ASCII between"
say "    non-printable bytes: a compiled object, a SQLite file. It is NOT a real .p12"
say "    and NOT a real Java keystore, which encrypt the key and come back exit 0;"
say "  - each of those answers carried the signature of the mechanism planted for it;"
say "  - and this harness was shown, on this run, able to tell those answers apart: its"
say "    comparator, its liveness check, its self-identification check, its"
say "    checkout-depth check and its attribution check were each run over planted"
say "    inputs whose owed answers differ."
say ''
say "NOT ESTABLISHED BY THIS RUN, AND NOT ESTABLISHABLE BY ANY RUN OF THIS SCRIPT:"
say "  - that gate 7 has ever failed on this repository's own dependencies. It has"
say "    none: the workspace is seventeen path crates and zero third-party packages,"
say "    so the advisory and license checks pass on an empty set. What is proven here"
say "    is that they would not pass on a full one;"
say "  - THAT GATE 7'S SECRET SCAN IS INSTALLED. IT IS NOT. Escalation E-0003 asks"
say "    the operator what implements it, and this script is not an answer to that"
say "    question. Every secret row above is a statement about $SCANNER_REL"
say "    and about nothing else;"
say "  - that the advisory scanner finds a credential in any encoding but ASCII. It"
say "    matches printable-ASCII runs. An AWS key written UTF-16, a real .p12, a real"
say "    Java keystore, base64 and gzip each hold the key and each come back exit 0,"
say "    measured and recorded in ops/gates/gate-7.md. Nor does it find a credential"
say "    with no recognisable shape, or one split across a chunk boundary. This list"
say "    is not a repair queue: closing any one of these opens the next, which is why"
say "    E-0003 exists instead of a third attempt at the scanner;"
say "  - that GitGuardian, which has run on every pull request to this repository"
say "    since before gate 7 was written and is an advisory check rather than a"
say "    required one, is or is not the answer to E-0003. That is the operator's"
say "    decision and this script cannot make it;"
say "  - what scanning binary content costs on a repository larger than this one. What"
say "    this run measured is printed above, in bytes and in files, for the inputs"
say "    planted here. There is no size cap, deliberately, because a cap is the same"
say "    silent skip wearing a different hat;"
say "  - that the declared-fake convention is anybody's rule but this gate's. No"
say "    specification says a fixture's fake secret is exempt only when the marker is"
say "    inside the value; TESTING section 5 says a fixture has fake keys and ENV_SETUP"
say "    section 4 says fixtures hold no secret, and neither says how a gate tells one"
say "    from the other. It is an unspecified convention this gate imposes, recorded in"
say "    ops/gates/gate-7.md as an open question for the operator;"
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
say "Gate 7's two installed checks have been seen to fail on a planted defect on this"
say "commit, which is the demonstration AICD §14 asks for before a gate enters"
say "service. The advisory scanner has been seen to fail on planted inputs too, and"
say "that is not the same claim: being seen to fail on what somebody thought to plant"
say "is necessary and is not sufficient, and the set nobody planted is where this one"
say "loses. GATE 7 IS PARTIALLY INSTALLED: two checks of three, with the secret scan"
say "open as escalation E-0003. Nothing may cite gate 7 as installed, and nothing may"
say "cite $SCANNER_REL as protection. ops/gates/gate-7.md records the"
say "whole of it, including the reproduction that settled it."
exit 0
