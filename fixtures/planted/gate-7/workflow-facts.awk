# workflow-facts.awk: print the structure of a GitHub Actions workflow file.
#
# Ticket: ORI-T-0016.  Spec: spec/CI_CD.md section 1 gate 7,
# spec/runbooks/prove-gate.md.  Rule: AICD §14.
# Read by `fixtures/planted/gate-7/prove.sh`; exercised on every run against
# the samples in `fixtures/planted/gate-7/workflow-samples/`.
#
# THIS FILE IS A COPY OF `fixtures/planted/gate-1/workflow-facts.awk`, AND THE
# COPY IS DELIBERATE. ORI-T-0016 was asked to reuse gate 1's shape and to
# factor what generalises, unless factoring would couple the two gates so that
# breaking one breaks the other. It would, in both directions:
#
#   - gate 1 is an INSTALLED gate (ops/gates/gate-1.md). A shared parser makes
#     every edit to it an edit to gate 1's proof, so a change made for gate 7
#     would require gate 1 to be re-proved, and gate 1's proof would stop being
#     reproducible from gate 1's own directory.
#   - a parser bug that made one gate's prover refuse would make both refuse on
#     the same commit, which turns one repairable failure into two red gates
#     and removes the second opinion.
#   - the shared location would be a new path outside this ticket's declared
#     scope (`.github/workflows/ci.yml`, `fixtures/planted/gate-7/**`), and
#     gate 1's copy is outside it too.
#
# What a copy costs is that a defect found in one is not fixed in the other,
# and that cost is paid deliberately: each copy has its own
# `workflow-samples/` table, each prover exercises its own copy over that table
# on every run before it believes a word it says, and the two tables ask for
# the answers their own gate needs. A divergence therefore shows up as a
# sample disagreeing in the prover that owns the copy, not as silence.
#
# WHY IT EXISTS AT ALL. The first version of gate 1's prove.sh asserted that
# the gate command appeared somewhere in `.github/workflows/ci.yml` as an
# anchored `run:` line. A whole-file grep cannot tell a command CI runs from a
# command CI does not: the line can belong to a job that is not in the
# aggregate's `needs:`, or to a job carrying `continue-on-error:` or an `if:`,
# or it can be text inside another step's shell script, or the workflow's
# triggers can be such that the job never runs at all. Every one of those
# leaves the byte-identical line in the file, so the grep is satisfied while
# the gate is dead and every check is green. That is AICD §39's "present but
# reporting nothing" and it is the cheapest way to switch a gate off.
#
# This parser answers the structural half of that question. It does not
# evaluate GitHub expressions and does not try to: where a key could make a job
# conditional it reports the key and prove.sh refuses, which is a deliberate
# choice to be wrong in the direction of asking for a re-proof.
#
# WHAT IT PRINTS, one record per line, fields separated by one space:
#
#   TRIGGER <name>              a key under the workflow's `on:`
#   TRIGGERKEY <name> <key>     a key under that trigger: a filter that
#                               narrows when it fires
#   JOB <job>
#   JOBKEY <job> <key>          a key of that job, `if` and
#                               `continue-on-error` among them
#   NEEDS <job> <needed>        one entry of that job's `needs:`
#   STEP <job> <n>              the n-th step of that job, counted from 1
#   STEPKEY <job> <n> <key>
#   RUN <job> <n> <command>     a `run:` whose value is on the same line
#   RUNBLOCK <job> <n>          a `run:` written as a block scalar, whose text
#                               this parser does not compare
#   STEPWITH <job> <n> <k> <v>  a key under that step's `with:`, with its
#                               value. Gate 7's copy prints this and gate 1's
#                               and gate 2's do not, because gate 7 is the only
#                               gate whose liveness depends on an input to an
#                               action rather than on a command: its secret
#                               scan covers history, and the history is only on
#                               the runner when the checkout step is given
#                               `fetch-depth: 0`. Nothing else in this file
#                               changed for it.
#   ERROR <line> <what>         this file has a shape this parser does not
#                               read. It is never a licence to continue:
#                               prove.sh treats any ERROR as "the workflow
#                               could not be read" and proves nothing.
#
# WHAT IT ASSUMES ABOUT THE FILE, and fails closed rather than guessing when
# the assumption does not hold: block style throughout, two-space indentation,
# job names alone on their line at two spaces, job keys at four, steps at six,
# step keys at eight. `.github/workflows/ci.yml` is written that way and its
# own `ci` job asserts the same shape for its own reasons.
#
# POSIX awk only. It runs under the one-true-awk on a developer's macOS and
# under mawk or gawk on the GitHub runner, so no gensub, no interval
# expressions and no GNU extensions.

function err(msg) {
    printf "ERROR %d %s\n", NR, msg
    errors = errors + 1
}

function indent_of(s) {
    match(s, /^ */)
    return RLENGTH
}

# The key of a `key: value` line, or "" when the line is not that shape.
function keyname(s, i) {
    if (s !~ /^[A-Za-z_][A-Za-z0-9_-]*:/) {
        return ""
    }
    i = index(s, ":")
    return substr(s, 1, i - 1)
}

# Everything after the first colon, trimmed. An empty result means the value is
# on the following lines, not that there is none.
function keyvalue(s, i, v) {
    i = index(s, ":")
    v = substr(s, i + 1)
    sub(/^[ ]+/, "", v)
    sub(/[ ]+$/, "", v)
    return v
}

# A value of `|` or `>` opens a block scalar: everything indented deeper than
# the key belongs to it and is text, not structure. Tracking this is the whole
# reason a shell script inside a `run: |` cannot forge a `run:` line.
function open_block_if_scalar(v, keyind) {
    if (v ~ /^[|>]/) {
        inblock = 1
        blockind = keyind
        return 1
    }
    return 0
}

function emit_needs(j, v, x, n, parts, i) {
    if (v == "") {
        return
    }
    if (v ~ /^\[/) {
        if (v !~ /\]$/) {
            err("the `needs:` of job `" j "` is an unterminated flow list, so this parser cannot tell which jobs it names")
            return
        }
        x = substr(v, 2, length(v) - 2)
        n = split(x, parts, ",")
        for (i = 1; i <= n; i++) {
            gsub(/[ \t]/, "", parts[i])
            if (parts[i] != "") {
                print "NEEDS " j " " parts[i]
            }
        }
        return
    }
    if (v ~ /^[A-Za-z0-9_-]+$/) {
        print "NEEDS " j " " v
        return
    }
    err("the `needs:` of job `" j "` has a shape this parser does not read: " v)
}

# Returns the key it read, so the caller can remember it: a `with:` opens a
# block of action inputs whose keys sit two spaces deeper, and those are read
# by the ind == 10 branch below rather than here.
function step_key(s, keyind, k, v) {
    k = keyname(s)
    if (k == "") {
        err("a step line in job `" job "` that is not `key:`; this parser will not guess what it is")
        return ""
    }
    v = keyvalue(s)
    print "STEPKEY " job " " step " " k
    if (open_block_if_scalar(v, keyind)) {
        if (k == "run") {
            print "RUNBLOCK " job " " step
        }
        return k
    }
    if (k == "run") {
        print "RUN " job " " step " " v
    }
    return k
}

BEGIN {
    section = ""
    job = ""
    trig = ""
    step = 0
    insteps = 0
    inblock = 0
    blockind = 0
    lastjobkey = ""
    laststepkey = ""
    errors = 0
}

{
    line = $0
    sub(/\r$/, "", line)

    # Inside a block scalar nothing is structure. This test comes first, before
    # blank lines and before comments, because both are content there.
    if (inblock) {
        if (line ~ /^[ \t]*$/) {
            next
        }
        if (indent_of(line) > blockind) {
            next
        }
        inblock = 0
    }

    if (line ~ /^[ \t]*$/) {
        next
    }
    if (line ~ /^[ ]*#/) {
        next
    }
    if (line ~ /^[ ]*\t/) {
        err("the indentation on this line contains a tab, which YAML forbids; this parser will not guess at the nesting")
        next
    }

    ind = indent_of(line)
    rest = substr(line, ind + 1)

    # --- top level ---------------------------------------------------------
    if (ind == 0) {
        k = keyname(rest)
        if (k == "") {
            err("a top-level line that is not `key:`")
            next
        }
        v = keyvalue(rest)
        job = ""
        trig = ""
        insteps = 0
        step = 0
        lastjobkey = ""
        laststepkey = ""
        if (k == "on") {
            section = "on"
            if (v != "" && v !~ /^#/) {
                err("`on:` carries its value on the same line; this parser reads only the block form, so it will not report which triggers fire this workflow")
            }
            next
        }
        if (k == "jobs") {
            section = "jobs"
            if (v != "" && v !~ /^#/) {
                err("`jobs:` carries a value on the same line")
            }
            next
        }
        section = "other"
        open_block_if_scalar(v, 0)
        next
    }

    # --- the triggers ------------------------------------------------------
    if (section == "on") {
        if (ind == 2) {
            k = keyname(rest)
            if (k == "") {
                err("a line at two spaces under `on:` that is not `<trigger>:`")
                next
            }
            trig = k
            print "TRIGGER " trig
            v = keyvalue(rest)
            if (v != "" && v !~ /^#/) {
                err("the trigger `" trig "` carries a value on the same line; this parser reads only the block form")
            }
            next
        }
        if (ind == 4 && trig != "") {
            k = keyname(rest)
            if (k != "") {
                print "TRIGGERKEY " trig " " k
            }
            next
        }
        next
    }

    # --- the jobs ----------------------------------------------------------
    if (section == "jobs") {
        if (ind == 2) {
            k = keyname(rest)
            if (k == "") {
                err("a line at two spaces under `jobs:` that is not a plain `<job>:`; this parser will not guess whether it defines a job, and a job it failed to see is a job nothing here reports on")
                job = ""
                next
            }
            v = keyvalue(rest)
            if (v != "" && v !~ /^#/) {
                err("the job `" k "` carries a value on the same line")
            }
            job = k
            insteps = 0
            step = 0
            lastjobkey = ""
            laststepkey = ""
            print "JOB " job
            next
        }
        if (job == "") {
            next
        }
        if (ind == 4) {
            k = keyname(rest)
            if (k == "") {
                err("a line at four spaces in job `" job "` that is not `key:`")
                next
            }
            v = keyvalue(rest)
            print "JOBKEY " job " " k
            lastjobkey = k
            laststepkey = ""
            insteps = (k == "steps")
            if (insteps) {
                step = 0
            }
            if (open_block_if_scalar(v, 4)) {
                next
            }
            if (k == "needs") {
                emit_needs(job, v)
            }
            next
        }
        if (ind == 6) {
            if (insteps) {
                if (rest !~ /^- /) {
                    err("a line at six spaces inside the `steps:` of job `" job "` that does not begin a step")
                    next
                }
                step = step + 1
                print "STEP " job " " step
                laststepkey = step_key(substr(rest, 3), 8)
                next
            }
            if (lastjobkey == "needs" && rest ~ /^- /) {
                emit_needs(job, substr(rest, 3))
                next
            }
            next
        }
        if (ind == 8 && insteps && step > 0) {
            laststepkey = step_key(rest, 8)
            next
        }
        # A key of a step's `with:`, which is an input to an action rather than
        # a command. Only `with:` is read this way. An `env:` block sits at the
        # same depth and `.github/workflows/ci.yml` has one, so treating every
        # line at ten spaces as a `with:` key would report the aggregate job's
        # environment as action inputs and would err on shapes that are fine.
        if (ind == 10 && insteps && step > 0 && laststepkey == "with") {
            k = keyname(rest)
            if (k == "") {
                err("a line under the `with:` of step " step " in job `" job "` that is not `key:`; this parser will not guess what input it sets")
                next
            }
            v = keyvalue(rest)
            if (open_block_if_scalar(v, 10)) {
                print "STEPWITH " job " " step " " k " (block scalar)"
                next
            }
            print "STEPWITH " job " " step " " k " " v
            next
        }
        next
    }

    # --- anywhere else: track block scalars and nothing more ---------------
    k = keyname(rest)
    if (k != "") {
        v = keyvalue(rest)
        open_block_if_scalar(v, ind)
    }
}
