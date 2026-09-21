# workflow-facts.awk: print the structure of a GitHub Actions workflow file.
#
# Ticket: ORI-T-0013.  Spec: spec/CI_CD.md section 1 gate 1,
# spec/runbooks/prove-gate.md.  Rule: AICD §14.
# Read by `fixtures/planted/gate-1/prove.sh`; exercised on every run against
# the samples in `fixtures/planted/gate-1/workflow-samples/`.
#
# WHY THIS EXISTS. The first version of prove.sh asserted that gate 1's command
# appeared somewhere in `.github/workflows/ci.yml` as an anchored `run:` line.
# A whole-file grep cannot tell a command CI runs from a command CI does not:
# the line can belong to a job that is not in the aggregate's `needs:`, or to a
# job carrying `continue-on-error:` or an `if:`, or it can be text inside
# another step's shell script, or the workflow's triggers can be such that the
# job never runs at all. Every one of those leaves the byte-identical line in
# the file, so the grep is satisfied while gate 1 is dead and every check is
# green. That is AICD §39's "present but reporting nothing" and it is the
# cheapest way to switch gate 1 off.
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

function step_key(s, keyind, k, v) {
    k = keyname(s)
    if (k == "") {
        err("a step line in job `" job "` that is not `key:`; this parser will not guess what it is")
        return
    }
    v = keyvalue(s)
    print "STEPKEY " job " " step " " k
    if (open_block_if_scalar(v, keyind)) {
        if (k == "run") {
            print "RUNBLOCK " job " " step
        }
        return
    }
    if (k == "run") {
        print "RUN " job " " step " " v
    }
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
                step_key(substr(rest, 3), 8)
                next
            }
            if (lastjobkey == "needs" && rest ~ /^- /) {
                emit_needs(job, substr(rest, 3))
                next
            }
            next
        }
        if (ind == 8 && insteps && step > 0) {
            step_key(rest, 8)
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
