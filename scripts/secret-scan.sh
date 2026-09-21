#!/usr/bin/env bash
#
# secret-scan.sh: an ADVISORY secret scan over a repository's tree and history.
#
# ===========================================================================
# THIS IS NOT A GATE. NOTHING MAY CITE IT AS ONE.
# ===========================================================================
#
# Gate 7's secret scan is NOT INSTALLED. What implements it is escalation
# E-0003, open with the operator, and this script is not an answer to it. It
# is kept because it catches the common case, and catching some is better than
# catching none, but a pass from it is not evidence that a repository holds no
# credential and it is not evidence that gate 7 was met.
#
# WHY IT CANNOT BE THE GATE. It matches byte patterns against printable-ASCII
# runs. A credential that is not printable ASCII in the bytes on disk is
# invisible to it and produces exit 0. That is not a bug to be repaired, it is
# what a pattern matcher over bytes is: every encoding is another hole, and
# each repair opens the next one. Two attempts at ORI-T-0016 proved this by
# losing the race twice. The reproduction is in ops/gates/gate-7.md: an AWS key
# written UTF-16, committed, scanned by this script, exit 0, "no finding".
# base64, gzip, UTF-32 and a key split across a chunk boundary are the same
# hole, and are not reproduced separately because they are not separate.
#
# WHAT ALREADY COVERS THIS REPOSITORY. GitGuardian runs on every pull request
# here, as the check named "GitGuardian Security Checks", and has since before
# gate 7 was written. It is an advisory check, not a required one. Whether it,
# a self-hosted scanner, or something else is what gate 7's secret scan means
# is E-0003's question, and until it is answered this half of gate 7 is
# reported blocked by `scripts/gates.sh` rather than passed.
#
# Ticket: ORI-T-0016.  Spec: spec/CI_CD.md section 1 gate 7 ("secret scan (tree
# and history)"), spec/ENV_SETUP.md section 4 ("no secret in the repository, in
# `.env` files tracked by git, in fixtures, in tests, in logs, in events, in
# tickets or in reports. The secret scan gate covers tree and history"),
# spec/TESTING.md section 1 row "Security".  Record: ops/gates/gate-7.md, which
# records gate 7 as PARTIALLY installed.  Rule: AICD section 14, which is the
# reason this file says it is not installed rather than being deleted or
# quietly counted.
#
# It lives beside the other scripts a gate runs (ruling R28: a policy the whole
# repository is judged by and a scanner a gate invokes are not fixtures, and do
# not live in a fixture directory).
#
# ===========================================================================
# WHAT IT SCANS, AND WHY "HISTORY" IS NOT A SYNONYM FOR "TREE"
# ===========================================================================
#
# A secret that was committed and then deleted is still in the repository. The
# tree is clean, `git grep` says nothing, and `git log -p` still hands the
# credential to anyone who clones. So this script asks two separate questions
# and reports them separately:
#
#   tree     every file git tracks at HEAD, as it stands in the working tree.
#   history  every blob reachable from any ref, which is what a clone hands the
#            next person, including blobs no commit's tree points at any more.
#
# THE WAY THIS CHECK FAILS SILENTLY, WHICH IS WHY IT REFUSES RATHER THAN PASSES.
# `actions/checkout` clones with `fetch-depth: 1` by default. In that clone
# there is exactly one commit, so "every blob reachable from any ref" is the
# tree and nothing else: the history half of this gate reports success having
# looked at no history at all. That is the defect class of AICD section 39,
# "present but reporting nothing", inside the gate meant to catch secrets. This
# script therefore refuses to scan a shallow repository (exit 3) instead of
# reporting it clean, and the `gate-7` job in `.github/workflows/ci.yml` sets
# `fetch-depth: 0` for that reason. Every count this script prints is printed
# whatever the verdict, so a scan of nothing cannot look like a clean scan.
#
# ===========================================================================
# EVERY FILE IS SCANNED, OR IT IS REPORTED AS NOT SCANNED. THERE IS NO THIRD
# OUTCOME, AND THERE USED TO BE
# ===========================================================================
#
# THE DEFECT THIS SECTION EXISTS BECAUSE OF. Until ORI-T-0016 attempt 2 the
# matcher below ran `grep -I`, which makes grep treat a file holding a NUL byte
# as binary and report no match without reading it. Every such file was skipped,
# counted as scanned, and contributed nothing:
#
#   grep -I on a file with a NUL byte and an AWS key : exit 1, no match
#   grep -a on the same file, same pattern           : exit 0, the key is there
#   this scanner's verdict on a tree holding that file, in the wording it
#   used at the time, which is not the wording it uses now:
#       "clean: no secret-shaped string outside a declared fake,
#        in 2 tracked file(s) and 2 history blob(s)", exit 0
#
# WHAT THAT REPAIR ACTUALLY BOUGHT, MEASURED RATHER THAN CLAIMED. An earlier
# version of this comment said "a keystore, a .p12, a compiled binary with a
# token baked in, a SQLite file, an .env someone saved with a byte-order mark:
# every one of them was silently clean", and left a reader to conclude that all
# five are now caught. Three of the five are still silently clean. The five
# were built and run against this repaired scanner, each in its own throwaway
# repository, and the exit statuses were:
#
#   a compiled binary with the key baked in as ASCII   exit 1, found
#   a SQLite file with the key in a text column        exit 1, found, "run 8"
#   an .env saved with a UTF-8 byte-order mark         exit 1, found -- and it
#                                                      was never missed: it has
#                                                      no NUL byte, so grep -I
#                                                      read it as text all along
#   an .env saved with a UTF-16 byte-order mark        exit 0, NOT FOUND
#   a PKCS#12 (.p12) holding a private key             exit 0, NOT FOUND
#   a Java keystore (JKS) holding a private key        exit 0, NOT FOUND
#
# The reason is one line long and it is not fixable here. This scanner matches
# printable-ASCII runs. A SQLite file and a compiled object store the
# credential as printable ASCII with non-printable bytes around it, which is
# what the repair addressed. A .p12 and a JKS store it DER-encoded and
# encrypted, and UTF-16 stores it one byte apart from itself, so in all three
# the credential is in the file and the string is not. Every count this script
# prints on those three is honest: it read every byte. It just cannot see what
# is in them.
#
# That correction is recorded because the sentence it replaces is this
# project's own defect class, AICD section 39, written into the gate meant to
# catch it: a claim of coverage that nobody re-derived. The real output for all
# six runs is in ops/gates/gate-7.md.
#
# THE RULE NOW. A file is scanned, or it is named in the output as not scanned.
# It is never skipped while being counted as scanned. Nothing is exempt for
# being binary, for being large, for its extension or for its path. A file that
# could not be read is a refusal (exit 3), not a pass, and the paths are printed.
#
# HOW BINARY CONTENT IS SCANNED, AND WHY NOT SIMPLY `grep -a` ON THE BYTES.
# A file holding a NUL byte is scanned as binary: every byte of it is read, and
# its printable-ASCII runs are matched rather than its raw bytes. A run is a
# maximal stretch of tab, newline and printable ASCII; every other byte ends the
# run. The reason is that a credential is printable text, and matching raw bytes
# lets one pattern span a NUL boundary and report a string that exists nowhere
# in the file, which is a finding a human cannot act on. Splitting first means
# every match is a string that really is in the file. Findings from a binary say
# `run N` where a text finding says `line N`, because a binary has no lines: run
# N is the Nth printable run, counting from the start of the file.
#
# WHAT SCANNING BINARY CONTENT COSTS, stated because it is not free:
#
#   1. Time and IO. Every byte of every tracked file and of every reachable
#      blob is read, twice for a binary (once to classify it, once to split it).
#      There is no size cap, deliberately: a cap is the same silent skip wearing
#      a different hat, and "the artifact was 40MB so we did not look" is the
#      defect above. What the run cost instead is printed, in bytes and in
#      files, so the cost is visible and can be argued with.
#   2. False positives that text scanning does not produce. A compiled binary
#      carrying the literal `TOKEN=` followed by twenty base64 characters is a
#      finding here. That is deliberate: an embedded token is exactly what this
#      half exists to find, and a human looking at one costs less than a
#      credential that ships. A long random blob can in principle satisfy
#      `sk-[A-Za-z0-9_-]{24,}` by accident; that has not happened in this
#      repository, and if it does the answer is a narrower pattern in a ticket,
#      not an exempted file.
#
# ===========================================================================
# A DECLARED FAKE, AND HOW IT IS TOLD FROM A SECRET
# ===========================================================================
#
# spec/TESTING.md section 5 gives `fixtures/migrated-with-drift` "a tracked env
# file with fake keys", and `.gitignore` re-includes `/fixtures/**/.env` so that
# it can be tracked. A gate that failed on it would be unusable; a gate that
# exempted it would be a gate with a hole exactly where ENV_SETUP section 4
# says secrets must not be ("no secret ... in fixtures").
#
# THE RULE HERE IS ABOUT THE VALUE, NOT ABOUT THE PATH. A match is exempt when
# the matched text itself contains the marker FIXTURE<FAKE>, written in full in
# MARKER below. No path is exempt, no directory is exempt, and there is no
# allow list of files.
#
# THIS CONVENTION IS NOT IN ANY SPECIFICATION. No document states that a
# fixture's fake secret is exempt only when the marker is inside the value.
# TESTING section 5 says a fixture has "fake keys" and ENV_SETUP section 4 says
# fixtures hold no secret; neither says how a gate tells one from the other, so
# this gate had to decide and this is the decision it made. It is recorded in
# ops/gates/gate-7.md as a convention this gate imposes and an open question for
# the operator, and it belongs in TESTING section 5 or ENV_SETUP section 4 if it
# is kept.
#
# WHY THAT RULE AND NOT A PATH ALLOW LIST. A path allow list makes every byte
# at that path invisible, so a real credential dropped into the fixture's env
# file is a secret this gate is configured not to see. The marker rule cannot
# be satisfied by a working credential: to carry the marker the value has to be
# edited, and an edited credential does not authenticate. The fake is fake in
# the value, and the gate can check that, which is the whole difference.
#
# WHAT THE RULE COSTS, stated plainly because it is not free:
#
#   1. Every fixture's fake key must carry the marker inside the value.
#      `fixtures/migrated-with-drift/.env` does not exist yet (ORI-T-0074
#      builds it); when it does, its values must carry the marker or this gate
#      fails on it. The failure message below says exactly that, so the cost is
#      one readable failure and not a mystery.
#   2. A fixture that needs a byte-exact realistic credential, a recorded
#      integration response for instance, cannot have one. That is deliberate:
#      such a recording is what ENV_SETUP section 4 forbids.
#   3. Someone who can edit a value can add the marker to a real secret and
#      make it invisible. The value is then not the credential any more, so
#      what they have smuggled is a credential with eleven characters replaced,
#      but it is recoverable by anyone who knows the trick. The last link is a
#      human reading the diff, which is the same place gate 1's recursion stops
#      and the reason `.github/workflows` and this script are tier 2 paths.
#
# ===========================================================================
# WHAT IT IS NOT
# ===========================================================================
#
# A pattern scanner finds shapes it was told about, IN THE ENCODING IT WAS
# TOLD ABOUT. Everything below is a hole, and the list is not a repair queue:
# closing any one of them opens the next, which is why this script is advisory
# and gate 7's secret scan is blocked on E-0003 rather than on this file.
#
#   not found: a credential in any encoding but ASCII. UTF-16, UTF-32 and a
#              UTF-8 string split by a NUL are all runs of one or two
#              characters here, and match nothing. Reproduced in
#              ops/gates/gate-7.md with real output, exit 0.
#   not found: a credential inside a container that encodes or encrypts it. A
#              .p12, a Java keystore, a PGP or age blob, a DER key. Measured,
#              not assumed: both were built and both came back exit 0.
#   not found: base64, gzip, or any other transformation of the bytes.
#   not found: a credential with no recognisable shape, or a password in prose.
#   not found: a secret split across a chunk boundary of whatever reads it.
#   not found: a secret in a submodule, or in objects no ref reaches
#
# It also does not read objects that no ref reaches
# (a dangling blob, a reflog entry), because those do not survive a clone and
# the thing being protected is what a clone hands out. A submodule is not
# silently skipped either: a gitlink entry is reported as not scanned and makes
# this run a refusal, because the contents of another repository have to be
# scanned in that repository. ops/gates/gate-7.md records all of that under what
# the proof does not establish. The patterns are in PATTERNS below, one per
# line, and adding one is an ordinary edit.
#
# ===========================================================================
# EXIT STATUS
# ===========================================================================
#
#   0  scanned, and no secret-shaped PRINTABLE-ASCII string was found that is
#      not a declared fake. This is NOT "there is no credential here": see
#      WHAT IT IS NOT above. Three of the six shapes this scanner was once
#      claimed to cover exit 0 with the credential sitting in the file. The
#      counts above the verdict say what was read, which is every byte; they
#      do not say what was understood.
#   1  a secret-shaped string was found in the tree or in the history. Every
#      finding is printed with its location and the pattern that matched.
#   3  this repository could not be scanned, or part of it could not be, so
#      this is not a pass: git is missing, the path is not a git repository,
#      the repository is shallow (so its history is not there to scan), it has
#      no commits, it tracks no files, or a tracked entry or a reachable blob
#      could not be read. Everything that was not scanned is named.
#
# Usage:  bash scripts/secret-scan.sh [--repo <path>]
# Default repository: the one containing the working directory.

set -uo pipefail

# ---------------------------------------------------------------------------
# EVERY TOOL BELOW WORKS ON BYTES, NOT ON CHARACTERS.
#
# This is not tidiness. In a UTF-8 locale, BSD `tr` exits 1 with "Illegal byte
# sequence" on input that is not valid UTF-8, which is most binary content, and
# the first run of this scanner after binary files stopped being skipped
# reported both of this repository's JPEGs as not scanned for exactly that
# reason. In C the same command reads them without complaint. The pattern
# classes are ASCII by construction too ([A-Za-z0-9], [[:space:]]), so C is
# also the locale in which they mean what they were written to mean.
#
# It is worth noticing how that was found: the run reported two files it could
# not read and refused, instead of counting them as scanned and printing
# "clean". The rule this scanner was repaired to follow found the next defect
# in the scanner itself on its first run.
# ---------------------------------------------------------------------------

export LC_ALL=C

# ---------------------------------------------------------------------------
# The marker, assembled rather than written, so that this line is not itself a
# declared fake for every pattern below. The two halves are never a match on
# their own.
# ---------------------------------------------------------------------------

MARKER='FIXTURE'
MARKER="${MARKER}FAKE"

# ---------------------------------------------------------------------------
# The patterns. One per record: <id> <extended regular expression>.
#
# Each is written so that its own text does not match it, because this file is
# a tracked file and the scan reads tracked files. `AKIA[0-9A-Z]{16}` needs
# sixteen characters from the class after AKIA, and the character after AKIA
# here is `[`, which is not in it. Every pattern below was checked that way and
# the whole set is checked by running this scanner over this repository, which
# is what the `gate-7` job does on every pull request.
# ---------------------------------------------------------------------------

PATTERNS='aws-access-key-id AKIA[0-9A-Z]{16}
private-key-block -----BEGIN [A-Z ]*PRIVATE KEY-----
github-token gh[pousr]_[A-Za-z0-9]{36}
provider-api-key sk-[A-Za-z0-9_-]{24,}
slack-token xox[abprs]-[A-Za-z0-9-]{10,}
secret-assignment (PRIVATE_KEY|API_KEY|CLIENT_TOKEN|SECRET|TOKEN)[A-Z0-9_]*[[:space:]]*[=:][[:space:]]*.?[A-Za-z0-9+/_=-]{20,}'

# The same patterns as one argument vector for grep, built once. A file costs
# one pass over the whole pattern set rather than one pass per pattern, which is
# what pays for reading binary content that used to be skipped: the per-pattern
# loop below then runs only over lines that already matched something.
GREP_ARGS=()
PATTERN_COUNT=0
while IFS=' ' read -r _pat_id _pat_re; do
    [ -n "${_pat_id:-}" ] || continue
    GREP_ARGS+=(-e "$_pat_re")
    PATTERN_COUNT=$((PATTERN_COUNT + 1))
done <<<"$PATTERNS"

TAB=$'\t'

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

say() { printf '%s\n' "$*"; }

# A message a human must see. Under GitHub Actions it is additionally emitted
# as a workflow annotation, so that it lands on the pull request check and not
# only in a log somebody has to open (spec/runbooks/prove-gate.md step 3).
emit() {
    local kind="$1"
    shift
    printf '%s: %s\n' "$kind" "$*"
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        printf '::%s file=scripts/secret-scan.sh::%s\n' "$kind" "$*"
    fi
}

# ---------------------------------------------------------------------------
# The floor. No path out of this script reaches exit 0 except the verdict at
# the bottom. `scripts/gates.sh` carries the same construction and the comment
# there records what it was built for: an abort before the verdict used to
# leave the shell exiting with the status of the last command that ran, which
# on a crashed run was a success.
# ---------------------------------------------------------------------------

STAGE='starting up'
VERDICT_REACHED=0
WORK_DIR=''

floor() {
    local status=$?
    if [ -n "$WORK_DIR" ] && [ -d "$WORK_DIR" ]; then rm -rf "$WORK_DIR"; fi
    if [ "$VERDICT_REACHED" -eq 0 ]; then
        emit error "secret-scan.sh stopped during stage '$STAGE' and was about to exit $status without reaching a verdict. Nothing was scanned, so this run reports that rather than a status that could be read as a clean scan."
        exit 3
    fi
    exit "$status"
}
trap floor EXIT

# ---------------------------------------------------------------------------
# Arguments
# ---------------------------------------------------------------------------

REPO_ARG='.'
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo)
            if [ "$#" -lt 2 ]; then
                VERDICT_REACHED=1
                emit error "--repo needs a path"
                exit 3
            fi
            REPO_ARG="$2"
            shift 2
            ;;
        --help|-h)
            VERDICT_REACHED=1
            # The header, however long it grows. A line range here would print
            # the wrong half of the file the first time somebody edited it.
            awk 'NR > 1 { if ($0 !~ /^#/ && $0 != "") exit; print }' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            VERDICT_REACHED=1
            emit error "unknown argument '$1'. Usage: secret-scan.sh [--repo <path>]"
            exit 3
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Stage 1: can this repository be scanned at all?
#
# Every answer here is exit 3, never exit 0. "I could not look" is a different
# answer from "I looked and found nothing", and giving them the same exit
# status is the defect this gate exists to catch, one level up.
# ---------------------------------------------------------------------------

STAGE='checking that the repository can be scanned'

if ! command -v git >/dev/null 2>&1; then
    VERDICT_REACHED=1
    emit error "git is not on this machine, so neither the tree nor the history can be read. Nothing was scanned."
    exit 3
fi

for tool in tr wc grep awk sort cut sed; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        VERDICT_REACHED=1
        emit error "$tool is not on this machine. It is what splits a file holding a NUL byte into the printable runs this scan matches, so without it binary content would be skipped, which is the defect this scanner was repaired for. Nothing was scanned."
        exit 3
    fi
done

if [ "$PATTERN_COUNT" -eq 0 ]; then
    VERDICT_REACHED=1
    emit error "PATTERNS is empty, so this scanner has nothing to look for and would report every repository clean. Nothing was scanned."
    exit 3
fi

REPO="$(git -C "$REPO_ARG" rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$REPO" ] || [ ! -d "$REPO" ]; then
    VERDICT_REACHED=1
    emit error "'$REPO_ARG' is not inside a git repository, so there is no tree and no history to scan. Nothing was scanned."
    exit 3
fi

say "advisory secret scan (ASCII patterns, tree and history). NOT gate 7's secret scan, which is not installed: escalation E-0003, ops/gates/gate-7.md."
say "  repository : $REPO"

shallow="$(git -C "$REPO" rev-parse --is-shallow-repository 2>/dev/null)"
if [ "$shallow" != 'false' ]; then
    VERDICT_REACHED=1
    emit error "this repository is shallow (git rev-parse --is-shallow-repository said '$shallow'), so its history is not here to be scanned. A shallow clone holds one commit, and scanning it would report 'history clean' having read the tree and nothing else. That is a check reporting success for a state that is not success, so this is a refusal and not a pass. In CI, set 'fetch-depth: 0' on the checkout step; locally, 'git fetch --unshallow'."
    exit 3
fi

commits="$(git -C "$REPO" rev-list --count --all 2>/dev/null || printf '0')"
if [ "${commits:-0}" -eq 0 ]; then
    VERDICT_REACHED=1
    emit error "this repository has no commit reachable from any ref, so there is no history to scan and nothing to say about it. Nothing was scanned."
    exit 3
fi

# ---------------------------------------------------------------------------
# Stage 2: the matcher, used for both halves.
#
# It takes a file and reports one record per finding. A match is a finding
# unless the matched text carries the marker, so the decision is made on the
# bytes that matched and never on where they were found.
# ---------------------------------------------------------------------------

WORK_DIR="$(mktemp -d)"
if [ ! -d "$WORK_DIR" ]; then
    VERDICT_REACHED=1
    emit error "could not create a temporary directory, so nothing was scanned"
    exit 3
fi

FINDINGS_FILE="$WORK_DIR/findings"
UNSCANNED_FILE="$WORK_DIR/unscanned"
: >"$FINDINGS_FILE"
: >"$UNSCANNED_FILE"
NUL_PROBE="$WORK_DIR/nul-probe"
RUNS_FILE="$WORK_DIR/runs"

EXEMPT_COUNT=0
MATCH_COUNT=0
UNSCANNED_COUNT=0
UNSCANNED_FATAL=0

TREE_FILES=0
TREE_TEXT=0
TREE_BINARY=0
TREE_BYTES=0
TREE_ENTRIES=0
HISTORY_BLOBS=0
HISTORY_TEXT=0
HISTORY_BINARY=0
HISTORY_BYTES=0

# not_scanned <where> <label> <fatal 0|1> <reason>
# Records one thing this run did not read. Nothing is ever dropped quietly:
# every call here is printed above the verdict, and a fatal one refuses.
not_scanned() {
    local where="$1" label="$2" fatal="$3"
    shift 3
    UNSCANNED_COUNT=$((UNSCANNED_COUNT + 1))
    [ "$fatal" -eq 0 ] || UNSCANNED_FATAL=1
    printf '%s\t%s\t%s\n' "$where" "$label" "$*" >>"$UNSCANNED_FILE"
}

# scan_file <file on disk> <where: tree|history> <label to print>
# Appends one record per finding to FINDINGS_FILE and counts exemptions.
# Returns 0 when the file was read in full, 1 when it could not be read, in
# which case the caller records it as not scanned.
#
# Sets SCAN_KIND to text or binary and SCAN_BYTES to the size read.
SCAN_KIND=''
SCAN_BYTES=0
scan_file() {
    local path="$1" where="$2" label="$3"
    local id pat line hits matched exempt_here status target unit total stripped

    SCAN_KIND=''
    SCAN_BYTES=0

    total="$(wc -c <"$path" 2>/dev/null)" || return 1
    total=$((total))

    # Text or binary, decided on the whole file and not on its first buffer.
    # `grep -I` decides this from the first few kilobytes and answers "binary,
    # no match" without reading the rest; that was the defect. A NUL anywhere
    # makes the file binary here, and either way every byte is read.
    if ! tr -d '\000' <"$path" >"$NUL_PROBE" 2>/dev/null; then return 1; fi
    stripped="$(wc -c <"$NUL_PROBE" 2>/dev/null)" || return 1
    stripped=$((stripped))

    if [ "$total" -ne "$stripped" ]; then
        SCAN_KIND='binary'
        unit='run'
        # Every byte that is not tab, newline or printable ASCII ends a run.
        # `-s` then squeezes the newlines this just produced, so a stretch of
        # non-printable bytes is one run boundary and not one per byte. It
        # cannot join two printable runs, because every non-printable byte
        # still produces a newline; all it removes is empty runs. On the 16 MiB
        # binary in the fixture it turns 10,785,384 runs into 1,198,402 and
        # takes the scan of that file from about ten seconds to about three.
        if ! tr -cs '\11\12\40-\176' '\n' <"$path" >"$RUNS_FILE" 2>/dev/null; then return 1; fi
        target="$RUNS_FILE"
    else
        SCAN_KIND='text'
        unit='line'
        target="$path"
    fi
    SCAN_BYTES="$total"

    # One pass over the whole pattern set. The status is read directly and this
    # is not a pipeline, so nothing downstream can overwrite it: 0 is a match,
    # 1 is no match, anything above 1 is grep failing, which is a file that was
    # not scanned rather than a file with nothing in it.
    hits="$(grep -a -n -E "${GREP_ARGS[@]}" -- "$target" 2>/dev/null)"
    status=$?
    if [ "$status" -gt 1 ]; then return 1; fi
    [ "$status" -eq 0 ] || return 0
    [ -n "$hits" ] || return 0

    while IFS= read -r line; do
        [ -n "$line" ] || continue
        # Everything after the first colon is the matched line's text; the
        # number before it is where it is. Re-extract the matched bytes,
        # because the exemption is about them and not about the line.
        local lineno text
        lineno="${line%%:*}"
        text="${line#*:}"
        while IFS=' ' read -r id pat; do
            [ -n "$id" ] || continue
            matched="$(printf '%s\n' "$text" | grep -o -E -e "$pat" 2>/dev/null)"
            [ -n "$matched" ] || continue
            exempt_here=1
            while IFS= read -r m; do
                [ -n "$m" ] || continue
                MATCH_COUNT=$((MATCH_COUNT + 1))
                case "$m" in
                    *"$MARKER"*) EXEMPT_COUNT=$((EXEMPT_COUNT + 1)) ;;
                    *)           exempt_here=0 ;;
                esac
            done <<<"$matched"
            if [ "$exempt_here" -eq 0 ]; then
                printf 'FINDING %s %s %s %s %s\n' "$where" "$label" "$unit" "$lineno" "$id" >>"$FINDINGS_FILE"
            fi
        done <<<"$PATTERNS"
    done <<<"$hits"
    return 0
}

# ---------------------------------------------------------------------------
# Stage 3: the tree. Every file git tracks, as it stands.
#
# `ls-files -s` rather than `ls-files`, because the mode and the object name are
# what let this half say something true about an entry it cannot read from the
# working tree: a symbolic link, a file whose permissions deny it, a submodule.
# Each of those is named below rather than skipped.
# ---------------------------------------------------------------------------

STAGE='scanning the tree'
tree_list="$WORK_DIR/tree-list"
deferred="$WORK_DIR/deferred"
: >"$deferred"
if ! git -C "$REPO" ls-files -s -z >"$tree_list" 2>/dev/null; then
    VERDICT_REACHED=1
    emit error "git ls-files failed in $REPO, so the tree could not be listed. Nothing was scanned."
    exit 3
fi

while IFS= read -r -d '' rec; do
    [ -n "$rec" ] || continue
    meta="${rec%%$TAB*}"
    rel="${rec#*$TAB}"
    mode="${meta%% *}"
    rest="${meta#* }"
    objname="${rest%% *}"
    TREE_ENTRIES=$((TREE_ENTRIES + 1))

    if [ "$mode" = '160000' ]; then
        not_scanned tree "$rel" 1 "a submodule (gitlink $objname). Its contents are another repository's tree and history and are not in this one, so this scan cannot read them. Run this scanner in that repository."
        continue
    fi
    if [ -L "$REPO/$rel" ]; then
        printf '%s\t%s\t%s\n' "$rel" "$objname" 'a symbolic link, so the working tree holds no content of its own here' >>"$deferred"
        continue
    fi
    if [ ! -f "$REPO/$rel" ]; then
        printf '%s\t%s\t%s\n' "$rel" "$objname" 'tracked, but there is no regular file at this path in the working tree' >>"$deferred"
        continue
    fi
    if [ ! -r "$REPO/$rel" ]; then
        printf '%s\t%s\t%s\n' "$rel" "$objname" 'the file is there and this process may not read it' >>"$deferred"
        continue
    fi

    if scan_file "$REPO/$rel" tree "$rel"; then
        TREE_FILES=$((TREE_FILES + 1))
        TREE_BYTES=$((TREE_BYTES + SCAN_BYTES))
        if [ "$SCAN_KIND" = 'binary' ]; then
            TREE_BINARY=$((TREE_BINARY + 1))
        else
            TREE_TEXT=$((TREE_TEXT + 1))
        fi
    else
        not_scanned tree "$rel" 1 "the file could not be read to the end, so no part of it was matched"
    fi
done <"$tree_list"

if [ "$TREE_ENTRIES" -eq 0 ]; then
    VERDICT_REACHED=1
    emit error "this repository tracks no file, so the tree half of this scan read nothing. A clean report over nothing is not a clean repository. Nothing was scanned."
    exit 3
fi

# ---------------------------------------------------------------------------
# Stage 4: the history. Every blob any ref reaches, which is what a clone hands
# the next person, including blobs whose file was deleted years ago.
# ---------------------------------------------------------------------------

STAGE='scanning the history'
objects="$WORK_DIR/objects"
if ! git -C "$REPO" rev-list --objects --all >"$objects" 2>/dev/null; then
    VERDICT_REACHED=1
    emit error "git rev-list --objects --all failed in $REPO, so the history could not be enumerated. Nothing is known about it, and this run will not report a clean history it did not read."
    exit 3
fi

types="$WORK_DIR/types"
if ! cut -d' ' -f1 <"$objects" | git -C "$REPO" cat-file --batch-check='%(objectname) %(objecttype)' >"$types" 2>/dev/null; then
    VERDICT_REACHED=1
    emit error "git cat-file --batch-check failed in $REPO, so the objects in the history could not be classified. Nothing was scanned in the history."
    exit 3
fi

blobs="$WORK_DIR/blobs"
awk '$2 == "blob" { print $1 }' <"$types" | sort -u >"$blobs"

# The path each blob was last seen under, for the report. A blob can have had
# several; one recognisable name is what a human needs to find it.
names="$WORK_DIR/names"
awk 'NF > 1 { name = $2; for (i = 3; i <= NF; i++) name = name " " $i; if (!($1 in seen)) { seen[$1] = 1; print $1 "\t" name } }' <"$objects" >"$names"

blob_file="$WORK_DIR/blob"
while IFS= read -r sha; do
    [ -n "$sha" ] || continue
    if ! git -C "$REPO" cat-file blob "$sha" >"$blob_file" 2>/dev/null; then
        VERDICT_REACHED=1
        emit error "git cat-file could not read blob $sha, which a ref of this repository reaches. Part of the history was therefore not scanned, and this run will not report on a history it only partly read."
        exit 3
    fi
    name="$(awk -F'\t' -v s="$sha" '$1 == s { print $2; exit }' "$names")"
    [ -n "$name" ] || name='(no path recorded)'
    if scan_file "$blob_file" history "${sha:0:12} $name"; then
        HISTORY_BLOBS=$((HISTORY_BLOBS + 1))
        HISTORY_BYTES=$((HISTORY_BYTES + SCAN_BYTES))
        if [ "$SCAN_KIND" = 'binary' ]; then
            HISTORY_BINARY=$((HISTORY_BINARY + 1))
        else
            HISTORY_TEXT=$((HISTORY_TEXT + 1))
        fi
    else
        not_scanned history "${sha:0:12} $name" 1 "the blob was written out and could not be read back to the end, so no part of it was matched"
    fi
done <"$blobs"

if [ "$HISTORY_BLOBS" -eq 0 ] && [ "$UNSCANNED_COUNT" -eq 0 ]; then
    VERDICT_REACHED=1
    emit error "no blob was reachable from any ref of this repository, so the history half of this scan read nothing. Nothing was scanned."
    exit 3
fi

# The tree entries this half could not read from the working tree. Each one's
# committed blob was either read by the history half above, in which case the
# bytes git holds for it were matched and the entry is reported rather than
# refused, or it was not, in which case nothing read it and this run refuses.
while IFS="$TAB" read -r rel objname why; do
    [ -n "$rel" ] || continue
    if grep -q -x -F -e "$objname" "$blobs" 2>/dev/null; then
        not_scanned tree "$rel" 0 "$why; its committed blob ${objname:0:12} was read by the history half below, so the bytes git holds for it were matched"
    else
        not_scanned tree "$rel" 1 "$why, and its object $objname is not among the blobs any ref reaches, so nothing read it"
    fi
done <"$deferred"

# ---------------------------------------------------------------------------
# Stage 5: the verdict. The counts are printed whatever it is, so that a scan
# of nothing cannot be mistaken for a clean scan, and so that what reading
# every byte of every blob costs is visible rather than argued about.
# ---------------------------------------------------------------------------

STAGE='reporting'

tree_findings="$(grep -c '^FINDING tree ' "$FINDINGS_FILE" 2>/dev/null || true)"
history_findings="$(grep -c '^FINDING history ' "$FINDINGS_FILE" 2>/dev/null || true)"
tree_findings="${tree_findings:-0}"
history_findings="${history_findings:-0}"

say "  patterns            : $PATTERN_COUNT"
say "  tracked entries     : $TREE_ENTRIES"
say "  scanned (tree)      : $TREE_FILES file(s), $TREE_TEXT as text and $TREE_BINARY as binary, $TREE_BYTES byte(s)"
say "  scanned (history)   : $HISTORY_BLOBS blob(s), $HISTORY_TEXT as text and $HISTORY_BINARY as binary, $HISTORY_BYTES byte(s) (reachable from any ref; $commits commit(s))"
say "  not scanned         : $UNSCANNED_COUNT"
say "  matches examined    : $MATCH_COUNT"
say "  declared fakes      : $EXEMPT_COUNT (matched text carrying the declared-fake marker)"
say "  findings (tree)     : $tree_findings"
say "  findings (history)  : $history_findings"

if [ "$UNSCANNED_COUNT" -ne 0 ]; then
    say ''
    say 'NOT SCANNED'
    while IFS="$TAB" read -r where label why; do
        [ -n "$where" ] || continue
        say "  $where $label"
        say "    $why"
    done <"$UNSCANNED_FILE"
fi

VERDICT_REACHED=1

if [ "$UNSCANNED_FATAL" -ne 0 ]; then
    say ''
    emit error "something in this repository was not scanned, and it is named under NOT SCANNED above. A secret scan that skips a file and reports the rest clean is the defect this gate exists to catch, so this run says it could not scan rather than reporting a pass. Either make the entry readable, or scan it where it lives, and run again."
    exit 3
fi

if [ "$tree_findings" -eq 0 ] && [ "$history_findings" -eq 0 ]; then
    say ''
    say "no finding: no secret-shaped printable-ASCII string outside a declared fake, in $TREE_FILES tracked file(s) and $HISTORY_BLOBS history blob(s), $((TREE_BYTES + HISTORY_BYTES)) byte(s) read in full."
    say "THIS IS NOT A CLEAN BILL AND NOT A GATE. This scanner matches printable-ASCII runs, so a credential that is not printable ASCII in the bytes it just read is invisible to it and produced this same line: an AWS key written UTF-16, a .p12, a Java keystore, base64 and gzip all exit 0 here with the key in the file. Gate 7's secret scan is NOT installed; what implements it is escalation E-0003. See ops/gates/gate-7.md."
    exit 0
fi

say ''
say 'FINDINGS'
sed 's/^/  /' "$FINDINGS_FILE"
say ''
emit error "the secret scan found $tree_findings in the tree and $history_findings in the history. spec/ENV_SETUP.md section 4: no secret in the repository, in .env files tracked by git, in fixtures, in tests, in logs, in events, in tickets or in reports, and the gate covers tree and history. A finding in the history is not repaired by deleting the file: the blob stays reachable until the history is rewritten and the credential is rotated, and rotation comes first (spec/runbooks/rotate-credentials.md). A finding that says 'run' rather than 'line' is in a file holding a NUL byte, which is matched as printable runs rather than lines; a binary carrying a credential is still a credential in the repository. If a finding is a fixture's declared fake, the fake must carry the marker inside the value, not merely live under fixtures/: see the comment at the top of scripts/secret-scan.sh."
exit 1
