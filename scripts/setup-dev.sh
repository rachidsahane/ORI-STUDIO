#!/usr/bin/env bash
#
# scripts/setup-dev.sh: prepare and verify a development machine for Ori Studio.
#
# Ticket ORI-T-0003. Spec anchor: ENV_SETUP §1.
#
# ENV_SETUP §1 says this script "installs everything above and runs the
# forbidden-action test against a fixture product; a fresh clone must pass it
# before any work". Two of those things do not exist yet: the forbidden-action
# test (ORI-T-0029) and the fixture products (ORI-T-0073 to ORI-T-0076). This
# script therefore reports every requirement in one of eight states and never
# passes over one in silence:
#
#   OK            present and verified now
#   INSTALLED     installed by this run
#   MISSING       required, absent, and this run could not supply it (fatal)
#   BROKEN        on PATH but it does not run; fatal once the tree needs it
#   ABSENT        required and absent, and this run was told to install nothing
#   UNCHECKED     its state is unknown, because something the check itself
#                 needs is not there. Never a pass, and it says what is missing
#   OPTIONAL      absent, and the specification says the machine works without it
#   NOT YET       specified, not built yet, named with the ticket that builds it
#
# No status above is a pass except OK and INSTALLED, and those two are the only
# ones a terminal PASS can be built from. A requirement that could not be
# checked is never counted as one that was. A setup script that silently skips
# its most important check, or that reports an absent thing as present, is the
# defect class this project keeps finding: a control that is present and
# reporting nothing, or reporting the opposite of what it found (AICD §39).
#
# Usage:
#   scripts/setup-dev.sh                 install what is missing, then verify
#   scripts/setup-dev.sh --check-only    verify only, install nothing
#   scripts/setup-dev.sh --self-test     check this script's own status handling
#   scripts/setup-dev.sh --dry-run       report what a real run would install,
#                                        and change nothing whatsoever
#   scripts/setup-dev.sh --help
#
# Environment:
#   ORI_SETUP_DRY_RUN=1   same as --dry-run
#
# Terminal states. There is one line and one exit status per state, and no
# state prints a line a reader could mistake for "everything is fine":
#
#   FAIL             1   a required item is missing or broken
#   NOT READY        1   a required item is absent and this run installed
#                        nothing, which is what --dry-run and --check-only are
#   INCONCLUSIVE     1   nothing at all could be verified. This is the floor:
#                        a run that checked nothing can never exit 0
#   PASS WITH GAPS   0   everything checkable is present, and at least one
#                        requirement of ENV_SETUP §1 is unmet or unverifiable
#   PASS             0   every required item present and verified, no gaps
#
# The choice is made in one function, print_result, which reads four counters
# and nothing else, so --self-test drives it through every one of those states.
#
# --dry-run and --check-only change nothing on the machine, including through
# rustup: see the RUSTUP_AUTO_INSTALL note below for why that needs saying.
# Idempotent: a second run on a complete machine installs nothing and exits 0.
# Targets bash 3.2, which is what macOS ships, so no associative arrays.

set -euo pipefail

# ---------------------------------------------------------------------------
# The exit-status rule this script is built around
# ---------------------------------------------------------------------------
#
# A shell pipeline returns the exit status of its LAST command. Written as
#
#     if some_check | tee log | tail -6; then   # WRONG
#
# the `if` tests `tail`, which succeeds on every input, so a failing check
# reports success. That is a control that is present and reporting nothing
# (AICD §39).
#
# Every command this script gates on runs through `run_capture`, which captures
# that command's own status into a variable before any output is shown, and
# puts no pipe between the command and the decision. `set -o pipefail` above is
# the second layer for anything that slips past that rule, and `--self-test`
# proves it is actually in effect rather than merely written down.
run_capture() {
  local logfile="$1"
  shift
  local status=0
  "$@" >"$logfile" 2>&1 || status=$?
  return "$status"
}

# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

REPORT_STATUS=()
REPORT_ITEM=()
REPORT_NOTE=()

# Four counters, and every status maps to exactly one of them. A status that
# maps to none would be a row that is printed and then counted nowhere, which
# is how a run ends up certifying a machine it never checked.
FATAL_COUNT=0      # MISSING, FAILED: required and not usable
PLANNED_COUNT=0    # ABSENT: required, not here, and this run installed nothing
GAP_COUNT=0        # NOT YET, BROKEN, UNCHECKED: reported, not satisfied or
                   # not knowable, and not fatal today
VERIFIED_COUNT=0   # OK, INSTALLED: actually checked and actually good

record() {
  # record <status> <item> <note>
  REPORT_STATUS+=("$1")
  REPORT_ITEM+=("$2")
  REPORT_NOTE+=("$3")
  case "$1" in
    MISSING|FAILED)    FATAL_COUNT=$((FATAL_COUNT + 1)) ;;
    ABSENT)            PLANNED_COUNT=$((PLANNED_COUNT + 1)) ;;
    "NOT YET"|BROKEN|UNCHECKED)
                       GAP_COUNT=$((GAP_COUNT + 1)) ;;
    OK|INSTALLED)      VERIFIED_COUNT=$((VERIFIED_COUNT + 1)) ;;
    OPTIONAL)          : ;;
    *)                 die "internal: unknown status '$1' for '$2'" ;;
  esac
  printf '  %-9s %-28s %s\n' "$1" "$2" "$3"
}

print_result() {
  # The single place a terminal line is chosen. It reads the four counters and
  # nothing else, which is what makes every terminal state reachable from
  # --self-test. Returns the exit status the run should end with.
  if [ "$FATAL_COUNT" -gt 0 ]; then
    printf 'RESULT: FAIL, %d required item(s) missing or broken.\n' "$FATAL_COUNT"
    return 1
  fi
  if [ "$PLANNED_COUNT" -gt 0 ]; then
    printf 'RESULT: NOT READY, %d required item(s) absent; this run installed nothing.\n' "$PLANNED_COUNT"
    printf 'Re-run without --dry-run or --check-only to install them.\n'
    return 1
  fi
  if [ "$VERIFIED_COUNT" -eq 0 ]; then
    # The floor. Without it, a run in which nothing could be checked has no
    # fatal rows either, and exits 0 on a machine it knows nothing about.
    printf 'RESULT: INCONCLUSIVE, nothing on this machine could be verified.\n'
    return 1
  fi
  if [ "$GAP_COUNT" -gt 0 ]; then
    printf 'RESULT: PASS WITH GAPS, %d requirement(s) of ENV_SETUP §1 unverified, listed above.\n' "$GAP_COUNT"
    return 0
  fi
  printf 'RESULT: PASS, every required item is present and verified.\n'
  return 0
}

section() {
  printf '\n== %s\n' "$1"
}

die() {
  printf '\nsetup-dev.sh: %s\n' "$1" >&2
  exit 1
}

first_line() {
  # The first line of a captured log, with builtins only. `$(head -1 ...)` in a
  # note is unchecked: when head cannot run, the row still prints, with an
  # empty note, and reads as a verified item with nothing to say about itself.
  local l=""
  IFS= read -r l <"$1" || :
  printf '%s' "$l"
}

show_log() {
  # Print a captured log, indented, after the decision has already been made.
  local logfile="$1"
  if [ -s "$logfile" ]; then
    printf '            ----- output -----\n'
    # `|| [ -n "$line" ]` prints a final line that has no trailing newline.
    # Without it the last line of a failing check's output is dropped, which
    # for the forbidden-action test is exactly the line a refusal failure ends
    # on: the output is shown and the verdict inside it is not (AICD §39).
    while IFS= read -r line || [ -n "$line" ]; do
      printf '            %s\n' "$line"
    done <"$logfile"
    printf '            ------------------\n'
  fi
}

# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

self_test() {
  local tmp failures=0
  tmp="$(mktemp)"
  trap 'rm -f "$tmp" 2>/dev/null || :' RETURN

  printf 'self-test: status handling\n'

  if run_capture "$tmp" false; then
    printf '  FAIL  run_capture reported success for a failing command\n'
    failures=$((failures + 1))
  else
    printf '  ok    run_capture reports failure for a failing command\n'
  fi

  if run_capture "$tmp" true; then
    printf '  ok    run_capture reports success for a succeeding command\n'
  else
    printf '  FAIL  run_capture reported failure for a succeeding command\n'
    failures=$((failures + 1))
  fi

  # The trap this script exists to avoid: without pipefail, a failing command
  # piped into a formatter reports the formatter's status. Prove pipefail is
  # actually in effect here rather than asserting it in a comment.
  if false | cat >/dev/null 2>&1; then
    printf '  FAIL  pipefail is not in effect: a piped failing check would report success\n'
    failures=$((failures + 1))
  else
    printf '  ok    pipefail is in effect: a piped failing check reports failure\n'
  fi

  # And show the inverted form really does pass, so the rule above is evidence
  # and not folklore.
  set +o pipefail
  if false | cat >/dev/null 2>&1; then
    printf '  ok    demonstrated: without pipefail the same pipeline reports success\n'
  else
    printf '  FAIL  demonstration did not reproduce the inverted pipeline\n'
    failures=$((failures + 1))
  fi
  set -o pipefail

  # Every terminal state this script can end in, driven through the one
  # function that decides them. The question each case asks is the one that
  # produced this script's defects: for this state, what does it print, and
  # could a reader mistake it for "everything is fine"?
  expect_result() {
    # expect_result <fatal> <planned> <gap> <verified> <want status> <want text>
    local out="" status=0
    out="$(FATAL_COUNT=$1 PLANNED_COUNT=$2 GAP_COUNT=$3 VERIFIED_COUNT=$4; print_result)" || status=$?
    local head="${out%%$'\n'*}"
    if [ "$status" -eq "$5" ] && [ "${head#RESULT: $6}" != "$head" ]; then
      printf '  ok    fatal=%s planned=%s gap=%s verified=%s -> %s, exit %s\n' "$1" "$2" "$3" "$4" "$6" "$5"
    else
      printf '  FAIL  fatal=%s planned=%s gap=%s verified=%s -> %s, exit %s (wanted %s, exit %s)\n' \
        "$1" "$2" "$3" "$4" "$head" "$status" "$6" "$5"
      failures=$((failures + 1))
    fi
  }

  # AICD §14: a check is trusted only once it has been seen to fail on a
  # planted defect. These eight cases were run against a copy of this script
  # with the INCONCLUSIVE floor removed, and the two zero-verified cases
  # failed, which is what makes them a test and not a decoration.
  printf 'self-test: terminal states\n'
  expect_result 1 0 0 5 1 "FAIL"
  expect_result 1 2 3 5 1 "FAIL"
  expect_result 0 2 0 5 1 "NOT READY"
  expect_result 0 1 3 5 1 "NOT READY"
  expect_result 0 0 0 0 1 "INCONCLUSIVE"
  expect_result 0 0 4 0 1 "INCONCLUSIVE"
  expect_result 0 0 3 5 0 "PASS WITH GAPS"
  expect_result 0 0 0 5 0 "PASS, every required item"

  if [ "$failures" -ne 0 ]; then
    printf 'self-test: %d failure(s)\n' "$failures"
    return 1
  fi
  printf 'self-test: pass\n'
  return 0
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

CHECK_ONLY=0
DRY_RUN="${ORI_SETUP_DRY_RUN:-0}"

usage() {
  # Print the header comment block, whatever length it grows to. A fixed line
  # range here silently truncates the documented contract the moment the header
  # is edited, and prints no error when it does.
  awk 'NR >= 3 { if ($0 !~ /^#/) exit; sub(/^#[ ]?/, ""); print }' "$0"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --check-only) CHECK_ONLY=1 ;;
    --self-test)  self_test; exit $? ;;
    --dry-run)    DRY_RUN=1 ;;
    -h|--help)    usage; exit 0 ;;
    *)            die "unknown argument: $1 (try --help)" ;;
  esac
  shift
done

# ---------------------------------------------------------------------------
# Repository root
# ---------------------------------------------------------------------------

# Every path in this script is measured from REPO_ROOT, so this resolution is
# not allowed to fail quietly. It is done with builtins only: `${var%/*}`, `cd`
# and `pwd` need nothing on PATH, whereas `$(dirname ...)` does, and when
# dirname is not found the substitution is empty, `cd ""` succeeds, and the
# root silently becomes the parent of whatever directory the operator happened
# to be in. Seen once, on a deliberately minimal PATH: the run reported
# "MISSING  rust-toolchain.toml  absent at the repository root", naming a file
# that was sitting next to the script it had just run.
SCRIPT_SRC="${BASH_SOURCE[0]}"
SCRIPT_NAME="${SCRIPT_SRC##*/}"
SCRIPT_REL="${SCRIPT_SRC%/*}"
if [ "$SCRIPT_REL" = "$SCRIPT_SRC" ]; then
  # invoked by bare name, with no directory part at all
  SCRIPT_REL="."
fi
SCRIPT_DIR="$(cd -- "$SCRIPT_REL" && pwd)" || die "cannot enter the directory of $SCRIPT_SRC"
[ -f "$SCRIPT_DIR/$SCRIPT_NAME" ] || die "resolved $SCRIPT_DIR as this script's directory, but $SCRIPT_NAME is not in it"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)" || die "cannot enter the repository root above $SCRIPT_DIR"
[ -d "$REPO_ROOT/scripts" ] || die "resolved $REPO_ROOT as the repository root, but it holds no scripts/ directory"
cd "$REPO_ROOT" || die "cannot enter $REPO_ROOT"

# Every check writes its output here before anything is decided about it, so a
# run without a log directory cannot report on anything and must not try.
LOG_DIR="$(mktemp -d)" || die "cannot create a temporary log directory (is mktemp on PATH?)"
[ -n "$LOG_DIR" ] && [ -d "$LOG_DIR" ] || die "cannot create a temporary log directory (is mktemp on PATH?)"
# `|| :` so that the cleanup cannot decide the exit status. Verified on bash
# 3.2 and 5: when the last command of an EXIT trap fails, its status replaces
# the one the script chose (a run that ended `exit 1` with an unavailable `rm`
# exits 127 instead), while a trap that ends successfully leaves it alone. The
# exit status of this script is print_result's verdict and nothing else.
trap 'rm -rf "$LOG_DIR" 2>/dev/null || :' EXIT

# Any rustup or cargo invocation inside a directory holding a
# rust-toolchain.toml makes rustup install that toolchain on the spot, before
# the command it was asked to run. `rustup toolchain list` is such a command.
# So the guard goes on unconditionally, for every mode, and is lifted for
# exactly one call: the deliberate `rustup toolchain install` below.
#
# Guarding only --check-only, as this script did, left --dry-run downloading a
# toolchain and then reporting the toolchain it had just installed as one it
# had found. A dry run that mutates the machine is not a dry run, and a run
# that creates the state it reports is a check that verifies its own output
# (AICD §39).
#
# Verified against rustup 1.29.1 in a tree pinned to an absent version:
# `rustup toolchain list` alone prints "info: syncing channel updates" and
# fetches; with RUSTUP_AUTO_INSTALL=0 it lists what is there and touches
# nothing.
export RUSTUP_AUTO_INSTALL=0

printf 'Ori Studio development setup\n'
printf 'repository: %s\n' "$REPO_ROOT"
if [ "$CHECK_ONLY" -eq 1 ]; then
  printf 'mode:       check only, nothing will be installed\n'
elif [ "$DRY_RUN" != "0" ]; then
  printf 'mode:       dry run, nothing is installed; what a real run would install\n'
  printf '            is reported as ABSENT and this run exits non-zero for it\n'
else
  printf 'mode:       install and verify\n'
fi

# ---------------------------------------------------------------------------
# 1. Base tools
# ---------------------------------------------------------------------------

section "Base tools"

# `command -v` answers "is there a file of that name on PATH", which is not
# the question. Run the thing. A row built from an unchecked $(git --version)
# prints OK with an empty note when git is on PATH and will not start.
if command -v git >/dev/null 2>&1; then
  if run_capture "$LOG_DIR/git.log" git --version; then
    record OK "git" "$(first_line "$LOG_DIR/git.log")"
    HAVE_GIT=1
  else
    record FAILED "git" "on PATH at $(command -v git) but 'git --version' failed"
    show_log "$LOG_DIR/git.log"
    HAVE_GIT=0
  fi
else
  record MISSING "git" "ENV_SETUP §1: git is required for everything"
  HAVE_GIT=0
fi

# rustup, not just some rustc. A machine with a distribution rustc and no
# rustup ignores rust-toolchain.toml entirely: the build runs, the pin does
# not, and nothing says so. That is the pin present and reporting nothing, so
# its absence is fatal rather than a warning.
if command -v rustup >/dev/null 2>&1; then
  if run_capture "$LOG_DIR/rustup.log" rustup --version; then
    record OK "rustup" "$(first_line "$LOG_DIR/rustup.log")"
    HAVE_RUSTUP=1
  else
    # On PATH and not usable. HAVE_RUSTUP stays 0 so that every check below
    # says it could not be run, rather than running it against a rustup that
    # does not work and reporting whatever comes back.
    record FAILED "rustup" "on PATH at $(command -v rustup) but 'rustup --version' failed"
    show_log "$LOG_DIR/rustup.log"
    HAVE_RUSTUP=0
  fi
else
  record MISSING "rustup" "without rustup, rust-toolchain.toml is not enforced; install from https://rustup.rs"
  HAVE_RUSTUP=0
fi

# ---------------------------------------------------------------------------
# 2. Rust toolchain, per rust-toolchain.toml
# ---------------------------------------------------------------------------

section "Rust toolchain (rust-toolchain.toml)"

PINNED_CHANNEL=""
if [ -f "$REPO_ROOT/rust-toolchain.toml" ]; then
  # Parsed with builtins. A `sed | head` here decides what the pin says, and
  # when sed cannot run it decides it quietly: empty output is indistinguishable
  # from a file with no channel line, and the run then reports the pin as
  # unreadable when it was never read.
  while IFS= read -r tc_line || [ -n "$tc_line" ]; do
    tc_trimmed="${tc_line#"${tc_line%%[![:space:]]*}"}"
    case "$tc_trimmed" in
      channel*=*\"*\"*)
        tc_rest="${tc_trimmed#*\"}"
        PINNED_CHANNEL="${tc_rest%%\"*}"
        break
        ;;
    esac
  done <"$REPO_ROOT/rust-toolchain.toml"
  if [ -n "$PINNED_CHANNEL" ]; then
    record OK "rust-toolchain.toml" "pins channel $PINNED_CHANNEL"
  else
    record MISSING "rust-toolchain.toml" "present but no [toolchain] channel could be read"
  fi
else
  record MISSING "rust-toolchain.toml" "absent at the repository root (ORI-T-0003 delivers it)"
fi

TOOLCHAIN_OK=0
TOOLCHAIN_STATUS=""   # what the toolchain row recorded, so the component rows
                      # below can say "not checked" rather than "missing" when
                      # the reason is that this run was told to install nothing
record_toolchain() {
  TOOLCHAIN_STATUS="$1"
  record "$1" "toolchain $PINNED_CHANNEL" "$2"
}

if [ "$HAVE_RUSTUP" -eq 1 ] && [ -n "$PINNED_CHANNEL" ]; then
  # Detect with `rustup toolchain list`, which installs nothing. Asking the
  # toolchain directly (`rustup run <channel> rustc --version`) or running any
  # cargo command in this directory can make rustup auto-install the pinned
  # toolchain, which would turn --check-only into a download.
  TCLIST_OK=1
  if ! run_capture "$LOG_DIR/tclist.log" rustup toolchain list; then
    TCLIST_OK=0
  fi
  # Matched with builtins, for the reason this script keeps running into:
  # `grep -q` answers 0 for "found" and 1 for "not found", and it also answers
  # 127 when grep itself could not be run, which this code then read as "not
  # found". On a PATH without grep the run reported the pinned toolchain as
  # not installed while it sat in the very list it had just fetched, and a
  # default run would have reinstalled it. A `case` over the lines cannot fail
  # that way: there is no external program left to fail. Quoting the channel
  # inside the pattern keeps it literal, so a channel containing a glob
  # character matches itself and nothing else.
  TOOLCHAIN_FOUND=0
  while IFS= read -r tc_installed || [ -n "$tc_installed" ]; do
    case "$tc_installed" in
      "$PINNED_CHANNEL"-*) TOOLCHAIN_FOUND=1; break ;;
    esac
  done <"$LOG_DIR/tclist.log"

  if [ "$TCLIST_OK" -ne 1 ]; then
    # "The probe failed" and "the toolchain is not installed" are different
    # answers. Emptying the log and carrying on turned the first into the
    # second: on a machine where the toolchain is in fact present, the run
    # said "not installed" and a default run would have reinstalled it, with
    # the real error never printed.
    record_toolchain FAILED "'rustup toolchain list' failed, so the pin could not be checked at all"
    show_log "$LOG_DIR/tclist.log"
  elif [ "$TOOLCHAIN_FOUND" -eq 1 ]; then
    TOOLCHAIN_OK=1
    record_toolchain OK "installed"
  elif [ "$CHECK_ONLY" -eq 1 ]; then
    record_toolchain MISSING "not installed; run without --check-only, or: rustup toolchain install"
  elif [ "$DRY_RUN" != "0" ]; then
    record_toolchain ABSENT "not installed; a real run would run 'rustup toolchain install --no-self-update'"
  else
    printf '  ...       %-28s installing (rustup toolchain install)\n' "toolchain $PINNED_CHANNEL"
    # With no argument this reads rust-toolchain.toml, including its components
    # list, and is a no-op when everything is already present. That is what
    # makes a second run of this script install nothing.
    # The one call that is allowed to install: the guard exported above is
    # lifted here and nowhere else, for this command only, so that every other
    # rustup and cargo invocation in this script stays incapable of changing
    # the machine.
    if run_capture "$LOG_DIR/tc.log" env RUSTUP_AUTO_INSTALL=1 rustup toolchain install --no-self-update; then
      if run_capture "$LOG_DIR/tcv.log" rustup run "$PINNED_CHANNEL" rustc --version; then
        TOOLCHAIN_OK=1
        record_toolchain INSTALLED "$(first_line "$LOG_DIR/tcv.log")"
      else
        record_toolchain FAILED "installed, but 'rustc --version' would not run under it"
        show_log "$LOG_DIR/tcv.log"
      fi
    else
      record_toolchain FAILED "'rustup toolchain install' failed"
      show_log "$LOG_DIR/tc.log"
    fi
  fi
fi

# rustfmt and clippy: CI_CD gate 1 is `fmt` and `clippy -D warnings`. Verified
# through `cargo`, from the repository root, so what is checked is what a gate
# run would actually resolve to under the pin rather than whatever happens to
# be on PATH under some other toolchain.
check_component() {
  # check_component <label> <cargo subcommand>
  local label="$1" subcmd="$2" log="$LOG_DIR/component-$2.log"
  if [ "$HAVE_RUSTUP" -ne 1 ]; then
    record UNCHECKED "$label" "rustup is absent or unusable, so nothing can be resolved under the pin"
    return
  fi
  if [ "$TOOLCHAIN_OK" -ne 1 ]; then
    # UNCHECKED, not MISSING. `cargo fmt --version` resolves through the pin,
    # so with the pinned toolchain unusable this check cannot run at all, and
    # nothing whatever is known about the component. Reporting it MISSING
    # states a fact about rustfmt that this run did not establish.
    if [ "$TOOLCHAIN_STATUS" = "ABSENT" ]; then
      record UNCHECKED "$label" "the pinned toolchain is absent; a real run installs it with its components (rust-toolchain.toml)"
    else
      record UNCHECKED "$label" "the pinned toolchain is not usable, so 'cargo $subcmd --version' cannot be run under it"
    fi
    return
  fi
  if run_capture "$log" cargo "$subcmd" --version; then
    record OK "$label" "$(first_line "$log")"
  else
    record MISSING "$label" "required by CI_CD gate 1; listed in rust-toolchain.toml components"
    show_log "$log"
  fi
}

check_component "rustfmt" "fmt"
check_component "clippy" "clippy"

# Installed cross-compilation targets, reported and never required. See the
# comment in rust-toolchain.toml for why the pin does not list any: the CI
# matrix builds each platform natively, and a rustup target alone does not
# supply the linker a real cross-compile needs.
if [ "$HAVE_RUSTUP" -eq 1 ]; then
  if run_capture "$LOG_DIR/targets.log" rustup target list --installed; then
    if [ -s "$LOG_DIR/targets.log" ]; then
      targets_list="$(<"$LOG_DIR/targets.log")"
      record OK "rustup targets" "${targets_list//$'\n'/ }"
    else
      record OK "rustup targets" "none installed, which is expected: the CI matrix builds natively"
    fi
  else
    record OPTIONAL "rustup targets" "could not be listed"
  fi
fi

# ---------------------------------------------------------------------------
# 3. Gate tools named by ENV_SETUP §1
# ---------------------------------------------------------------------------

section "Gate tools (cargo-mutants, cargo-audit, cargo-deny)"

install_cargo_tool() {
  # install_cargo_tool <binary> <crate> <why>
  local bin="$1" crate="$2" why="$3" log="$LOG_DIR/$1.log"
  # All three are cargo subcommands: `cargo-mutants`, `cargo-audit`,
  # `cargo-deny`. Verify through `cargo <sub> --version`, which is how the
  # gates will call them, not `cargo-mutants --version`, which fails on a
  # perfectly good installation because the binary expects its subcommand name
  # as the first argument. Checking the wrong invocation would report a working
  # tool as broken, and a check that lies in either direction is no check.
  local sub="${bin#cargo-}"

  if command -v "$bin" >/dev/null 2>&1 && [ "$TOOLCHAIN_OK" -ne 1 ]; then
    # `cargo <sub> --version` goes through the rustup shim, which refuses
    # before it ever reaches the tool when the pinned toolchain is not
    # installed. Verifying anyway reported three perfectly good tools as
    # FAILED, each with "reinstall with: cargo install --locked ...", which
    # names the wrong cause and sends the operator to fix the wrong thing.
    record UNCHECKED "$bin" "$why; on PATH, but 'cargo $sub' cannot run until the pinned toolchain is installed"
    return
  fi

  if command -v "$bin" >/dev/null 2>&1; then
    # Ask the tool itself, not just PATH: a stale or half-written shim is
    # present and reports nothing.
    if run_capture "$log" cargo "$sub" --version; then
      record OK "$bin" "$(first_line "$log")"
    else
      record FAILED "$bin" "on PATH but 'cargo $sub --version' failed; reinstall with: cargo install --locked $crate"
      show_log "$log"
    fi
    return
  fi

  if [ "$CHECK_ONLY" -eq 1 ]; then
    record MISSING "$bin" "$why; install with: cargo install --locked $crate"
    return
  fi

  if [ "$DRY_RUN" != "0" ]; then
    # ABSENT, not OK. The tool is not on this machine; all the dry run knows is
    # what it would have done about that. Recording it OK made --dry-run
    # certify a machine with none of the gate 6 and gate 7 tools as complete,
    # which is the opposite of the truth in the one mode whose whole job is to
    # tell the operator what is missing.
    record ABSENT "$bin" "$why; absent, a real run would run 'cargo install --locked $crate'"
    return
  fi

  printf '  ...       %-28s installing (cargo install --locked %s)\n' "$bin" "$crate"
  if run_capture "$log" cargo install --locked "$crate"; then
    # Do not take `cargo install`'s word for it. Re-verify the invocation the
    # gates use, so "INSTALLED" means the tool actually runs.
    if command -v "$bin" >/dev/null 2>&1 && run_capture "$log" cargo "$sub" --version; then
      record INSTALLED "$bin" "$why, $(first_line "$log")"
    else
      record FAILED "$bin" "cargo install reported success but 'cargo $sub --version' does not run; check \$CARGO_HOME/bin"
      show_log "$log"
    fi
  else
    record FAILED "$bin" "cargo install --locked $crate failed"
    show_log "$log"
  fi
}

# `cargo install` needs a working cargo. Without it, say so once and mark the
# three tools missing rather than emitting three identical install failures.
if command -v cargo >/dev/null 2>&1; then
  install_cargo_tool "cargo-mutants" "cargo-mutants" "CI_CD gate 6, mutation score threshold"
  install_cargo_tool "cargo-audit"   "cargo-audit"   "CI_CD gate 7, advisories"
  install_cargo_tool "cargo-deny"    "cargo-deny"    "CI_CD gate 7, licenses and bans"
else
  record MISSING "cargo-mutants" "cargo is absent, cannot install"
  record MISSING "cargo-audit" "cargo is absent, cannot install"
  record MISSING "cargo-deny" "cargo is absent, cannot install"
fi

# ---------------------------------------------------------------------------
# 4. UI toolchain: checked, never installed
# ---------------------------------------------------------------------------

section "UI toolchain (Node, pnpm)"

# This script checks for Node and pnpm and does not install them. Three
# reasons, all of them specification, not preference:
#
#   1. ENV_SETUP §1 calls Node "UI build only. Never a runtime
#      dependency of the engine", and CLAUDE.md forbids adding a Node runtime
#      dependency to the engine. A setup script that silently provisions a Node
#      toolchain on a machine makes that call on the operator's behalf.
#   2. Node is almost always managed by a per-developer version manager (nvm,
#      fnm, volta, asdf, mise) or a system package manager. A script that
#      installs Node behind one of those breaks the developer's other projects
#      and is not idempotent in any meaningful sense.
#   3. Nothing in the tree needs it yet. `apps/desktop` is ORI-T-0002, scaffold
#      only, not built.
#
# So it is reported, with the consequence spelled out. It becomes fatal exactly
# when the tree acquires a UI to build, which is the point at which CI_CD gate
# 10 (UI type check, lint, unit tests, build) can actually run.

UI_PRESENT=0
if [ -d "$REPO_ROOT/apps/desktop" ] || [ -f "$REPO_ROOT/package.json" ]; then
  UI_PRESENT=1
fi

check_ui_tool() {
  # check_ui_tool <binary> <hint>
  #
  # Three distinct states, because collapsing them reports the wrong one. A
  # binary that is on PATH and will not run is not "absent" and it is
  # certainly not "needed later": it is broken now, and the operator who
  # installed it believes it works. The single `&&` this replaces filed a
  # broken node under "required once apps/desktop is built".
  local bin="$1" hint="$2" log="$LOG_DIR/ui-$1.log"
  if command -v "$bin" >/dev/null 2>&1; then
    if run_capture "$log" "$bin" --version; then
      record OK "$bin" "$(first_line "$log")"
    elif [ "$UI_PRESENT" -eq 1 ]; then
      record FAILED "$bin" "on PATH at $(command -v "$bin") but '$bin --version' failed, and a UI tree needs it (CI_CD gate 10)"
      show_log "$log"
    else
      record BROKEN "$bin" "on PATH at $(command -v "$bin") but '$bin --version' failed; fatal once apps/desktop is built (ORI-T-0002)"
      show_log "$log"
    fi
  elif [ "$UI_PRESENT" -eq 1 ]; then
    record MISSING "$bin" "a UI tree exists, so CI_CD gate 10 cannot run without it; $hint"
  else
    record "NOT YET" "$bin" "required once apps/desktop is built (ORI-T-0002); not installed by this script; $hint"
  fi
}

check_ui_tool "node" "install Node LTS with your version manager"
check_ui_tool "pnpm" "corepack enable, or install pnpm with your package manager"

# ---------------------------------------------------------------------------
# 5. Optional and per-OS prerequisites
# ---------------------------------------------------------------------------

section "Optional and per-OS prerequisites"

# ENV_SETUP §1: "Docker or Podman ... Optional; worktree-only downgrade
# without it." Reported, never fatal, and the downgrade is named so that an
# operator who sees OPTIONAL knows what they are losing.
# Checked by running it, for the same reason as git above: the row claims the
# machine can use container isolation, which a name on PATH does not establish.
# `--version` is answered by the client alone, so this says nothing about
# whether a daemon is up, and the note does not pretend otherwise.
if command -v docker >/dev/null 2>&1 && run_capture "$LOG_DIR/docker.log" docker --version; then
  record OK "container runtime" "$(first_line "$LOG_DIR/docker.log") on PATH; coder agents can use container isolation"
elif command -v podman >/dev/null 2>&1 && run_capture "$LOG_DIR/podman.log" podman --version; then
  record OK "container runtime" "$(first_line "$LOG_DIR/podman.log") on PATH; coder agents can use container isolation"
elif command -v docker >/dev/null 2>&1 || command -v podman >/dev/null 2>&1; then
  record OPTIONAL "container runtime" "a container runtime is on PATH but '--version' failed: coder agents run worktree-only (ENV_SETUP §1)"
else
  record OPTIONAL "container runtime" "no docker or podman: coder agents run worktree-only (ENV_SETUP §1)"
fi

# Tauri 2 prerequisites, per OS. Reported rather than enforced while
# apps/desktop is scaffold-only (ORI-T-0002); it becomes fatal on the same
# condition as Node, above.
case "$(uname -s)" in
  Darwin)
    if run_capture "$LOG_DIR/xcode.log" xcode-select -p; then
      record OK "Tauri prerequisites" "macOS: command line tools at $(first_line "$LOG_DIR/xcode.log")"
    elif [ "$UI_PRESENT" -eq 1 ]; then
      record MISSING "Tauri prerequisites" "macOS: run 'xcode-select --install'"
    else
      record "NOT YET" "Tauri prerequisites" "macOS: 'xcode-select --install' needed once apps/desktop is built"
    fi
    ;;
  Linux)
    if command -v pkg-config >/dev/null 2>&1 && \
       run_capture "$LOG_DIR/webkit.log" pkg-config --exists webkit2gtk-4.1; then
      record OK "Tauri prerequisites" "Linux: webkit2gtk-4.1 found"
    elif [ "$UI_PRESENT" -eq 1 ]; then
      record MISSING "Tauri prerequisites" "Linux: WebKitGTK (webkit2gtk-4.1) and its development headers"
    else
      record "NOT YET" "Tauri prerequisites" "Linux: WebKitGTK needed once apps/desktop is built"
    fi
    ;;
  MINGW*|MSYS*|CYGWIN*)
    record "NOT YET" "Tauri prerequisites" "Windows: WebView2 runtime; verified by the Windows CI job (ORI-T-0012)"
    ;;
  *)
    record "NOT YET" "Tauri prerequisites" "unrecognised platform $(uname -s); check ENV_SETUP §1 by hand"
    ;;
esac

# ---------------------------------------------------------------------------
# 6. This repository's own invariants
# ---------------------------------------------------------------------------

section "Repository invariants"

# The reason .gitignore was urgent (ORI-T-0003): `target/` untracked and
# unignored is tens of megabytes one `git add -A` away from being permanent,
# and CONVENTIONS "Git" rules out rewriting history to undo it. Verified on
# every setup run so a later edit cannot quietly drop it.
# HAVE_GIT, not `command -v git`. Every row below is an answer from git, so a
# git that does not run does not produce a weaker answer here, it produces no
# answer at all: with one on PATH that exits non-zero, this section printed
# "OK  Cargo.lock is tracked  not ignored, as a binary workspace requires",
# which is a verdict about .gitignore that nothing established.
if [ "$HAVE_GIT" -ne 1 ]; then
  record UNCHECKED "repository invariants" "git is absent or does not run, so .gitignore could not be inspected"
elif { [ -d "$REPO_ROOT/.git" ] || [ -f "$REPO_ROOT/.git" ]; }; then
  # Two things this check has to get right, both found by planting the defect
  # and watching the check fail to notice:
  #
  #   `--no-index`. Without it, `git check-ignore` stays silent about any path
  #   that is already tracked, so the Cargo.lock check below would report "not
  #   ignored" for a tracked Cargo.lock no matter what .gitignore said. That is
  #   a check present and reporting nothing (AICD §39).
  #
  #   A path INSIDE target/, not the bare name. The pattern `target/` matches
  #   directories only, and git cannot tell that a name with no directory on
  #   disk is one, so `check-ignore target` reports "not ignored" on exactly
  #   the fresh clone this script exists to verify. `target/debug` is
  #   unambiguous whether or not anything has been built.
  #   Exit status 0, 1 and "anything else" are three different answers.
  #   `git check-ignore` exits 0 for ignored, 1 for not ignored and 128 when it
  #   could not look (a broken index, an unreadable .gitignore, a bad
  #   pathspec). Treating everything non-zero as "not ignored" turned a git
  #   failure into a verdict about .gitignore, and for Cargo.lock below it
  #   turned that failure into an OK row.
  ignore_status=0
  run_capture "$LOG_DIR/ignore.log" git -C "$REPO_ROOT" check-ignore -q --no-index -- "target/debug" || ignore_status=$?
  case "$ignore_status" in
    0) record OK ".gitignore covers target/" "build output cannot be committed by accident" ;;
    1) record MISSING ".gitignore covers target/" "add 'target/' to .gitignore before running cargo" ;;
    *) record FAILED ".gitignore covers target/" "could not be checked: git check-ignore exited $ignore_status"
       show_log "$LOG_DIR/ignore.log" ;;
  esac

  # The other half of the same invariant. CONVENTIONS "Rust" pins versions by
  # Cargo.lock and CI_CD gate 7 audits it, so an ignored lockfile would leave a
  # binary workspace resolving dependencies afresh and the advisory gate
  # reading whatever it got. Checked here because it is a one-line .gitignore
  # edit away and nothing else would notice.
  lock_status=0
  run_capture "$LOG_DIR/lock.log" git -C "$REPO_ROOT" check-ignore -q --no-index -- "Cargo.lock" || lock_status=$?
  case "$lock_status" in
    0) record FAILED "Cargo.lock is tracked" "Cargo.lock is IGNORED; remove that pattern (CI_CD gate 7, CONVENTIONS Rust)" ;;
    1) record OK "Cargo.lock is tracked" "not ignored, as a binary workspace requires" ;;
    *) record FAILED "Cargo.lock is tracked" "could not be checked: git check-ignore exited $lock_status"
       show_log "$LOG_DIR/lock.log" ;;
  esac
else
  record OPTIONAL "repository invariants" "not a git checkout, nothing to verify"
fi

# ---------------------------------------------------------------------------
# 7. Specified, not built yet
# ---------------------------------------------------------------------------
#
# Everything below is named by ENV_SETUP §1 and does not exist in the
# tree today. Each is reported with the ticket that delivers it. None is
# skipped, and none is counted as a pass.

section "Specified, not built yet"

# The one ENV_SETUP §1 singles out: "installs everything above and runs
# the forbidden-action test against a fixture product; a fresh clone must pass
# it before any work". It is CI_CD gate 8 and ENV_SETUP §6.
#
# The path probed below is the shell entry point to the test, not the test.
# Lead ruling R16, as `ops/rulings.md` now records it, puts the harness in
# `crates/ori-broker/src/forbidden.rs`, a Rust module in the broker built by
# ORI-T-0029 at tier 2, and extends that same ticket's declared scope to add a
# thin `scripts/forbidden-action-test.sh` that invokes the harness and exits
# with its status. ORI-T-0029 therefore produces both halves, and the wrapper
# exists so that shell callers, this script and CI_CD gate 8, have one stable
# entry point. Probing the wrapper rather than the module is the point of the
# ruling: a shell check cannot invoke a Rust module, and the wrapper is the
# contract. R16 was corrected on exactly this point before it was recorded, so
# this comment follows the text in `ops/rulings.md` and not any earlier
# statement of the ruling.
#
# The check is written so that it starts running by itself the moment the
# wrapper and a fixture both land, rather than needing someone to remember.
FORBIDDEN_RUNNER="$REPO_ROOT/scripts/forbidden-action-test.sh"
FORBIDDEN_FIXTURE="$REPO_ROOT/fixtures/new-product"

# What this check recorded, kept so the summary can be built from the verdict
# instead of from a counter that knows nothing about it.
FORBIDDEN_STATUS=""
FORBIDDEN_GAP=""

record_forbidden() {
  FORBIDDEN_STATUS="$1"
  record "$1" "forbidden-action test" "$2"
}

if [ -e "$FORBIDDEN_RUNNER" ] && [ ! -x "$FORBIDDEN_RUNNER" ]; then
  # Present and not executable is not "not built yet". Once ORI-T-0029 lands, a
  # lost permission bit (an unzipped archive, a checkout that dropped the mode)
  # would otherwise turn this project's most important control off and blame an
  # already-delivered ticket for its absence, while the run exits 0.
  record_forbidden FAILED "scripts/forbidden-action-test.sh exists but is not executable: chmod +x it (ENV_SETUP §6, CI_CD gate 8)"
elif [ -x "$FORBIDDEN_RUNNER" ] && [ ! -d "$FORBIDDEN_FIXTURE" ]; then
  FORBIDDEN_GAP="the fixture fixtures/new-product (ORI-T-0073 to ORI-T-0076)"
  record_forbidden "NOT YET" "NOT RUN, missing $FORBIDDEN_GAP"
elif [ -x "$FORBIDDEN_RUNNER" ]; then
  if run_capture "$LOG_DIR/forbidden.log" "$FORBIDDEN_RUNNER" "$FORBIDDEN_FIXTURE"; then
    record_forbidden OK "passed against fixtures/new-product"
  else
    record_forbidden FAILED "FAILED against fixtures/new-product (ENV_SETUP §6, CI_CD gate 8)"
    show_log "$LOG_DIR/forbidden.log"
  fi
else
  FORBIDDEN_GAP="the runner scripts/forbidden-action-test.sh (ORI-T-0029)"
  if [ ! -d "$FORBIDDEN_FIXTURE" ]; then
    FORBIDDEN_GAP="$FORBIDDEN_GAP and the fixture fixtures/new-product (ORI-T-0073 to ORI-T-0076)"
  fi
  record_forbidden "NOT YET" "NOT RUN, missing $FORBIDDEN_GAP"
fi

# ENV_SETUP §1, git row: "Hooks installed by the watcher". ori-watch is
# a crate skeleton (ORI-T-0001) with no hook installer yet.
record "NOT YET" "git hooks" "installed by ori-watch, which has no hook installer yet"

# ENV_SETUP §1: "tree-sitter grammars ... Fetched at build for the
# supported language set". No build step fetches them yet.
record "NOT YET" "tree-sitter grammars" "fetched at build; no build step fetches them yet (code map)"

# ENV_SETUP §1: "Fonts and system libraries for the UI test matrix ...
# Per OS in scripts/". Those per-OS scripts do not exist.
record "NOT YET" "UI test matrix fonts" "per-OS scripts in scripts/ do not exist yet (screenshot matrix, TESTING)"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

printf '\n'
printf '===========================================================================\n'
printf 'Summary\n'
printf '===========================================================================\n'

i=0
count=${#REPORT_ITEM[@]}
while [ "$i" -lt "$count" ]; do
  printf '  %-9s %-28s %s\n' "${REPORT_STATUS[$i]}" "${REPORT_ITEM[$i]}" "${REPORT_NOTE[$i]}"
  i=$((i + 1))
done

if [ "$((GAP_COUNT + PLANNED_COUNT))" -gt 0 ]; then
  printf '\n'
  printf '!! %d requirement(s) named by ENV_SETUP §1 are not satisfied on this\n' "$((GAP_COUNT + PLANNED_COUNT))"
  printf '!! machine. Each is listed above under its own status: ABSENT (here is\n'
  printf '!! what a real run would install), BROKEN (on PATH and it does not run),\n'
  printf '!! UNCHECKED (its state is unknown and the row says why), NOT YET (not\n'
  printf '!! built yet, with the ticket that delivers it). This run does not mean\n'
  printf '!! any of them passes.\n'
fi

# The forbidden-action test gets its own sentence, and that sentence is built
# from what the check above actually recorded. Keyed on a counter instead, it
# asserted that the test "has not run even once" in runs where it had just run
# and passed, and would have gone on asserting it long after ORI-T-0029 landed,
# because node, pnpm, git hooks, grammars and fonts hold that counter above
# zero indefinitely.
case "$FORBIDDEN_STATUS" in
  "NOT YET")
    printf '\n'
    printf '!! The forbidden-action test that ENV_SETUP §1 requires of a fresh clone\n'
    printf '!! before any work HAS NOT RUN on this machine. Missing:\n'
    printf '!!   %s\n' "$FORBIDDEN_GAP"
    printf '!! No result below should be read as that requirement being met.\n'
    ;;
  FAILED)
    printf '\n'
    printf '!! The forbidden-action test that ENV_SETUP §1 requires of a fresh clone\n'
    printf '!! did not pass. Until it does, this machine is not fit for agent work.\n'
    ;;
  OK)
    printf '\n'
    printf '!! The forbidden-action test ran on this machine and passed.\n'
    ;;
esac

printf '\n'
status=0
print_result || status=$?
exit "$status"
