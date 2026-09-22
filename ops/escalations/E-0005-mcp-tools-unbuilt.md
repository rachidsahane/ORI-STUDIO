# Escalation E-0005: the product's own tool surface does not exist, and its instructions do not say so

| | |
|---|---|
| Trigger | `precondition_missing` |
| Raised by | The lead, at ORI-T-0089 review, after three agents reported it independently |
| Blocks | Nothing outright. Every ticket worked so far silently deviates from the procedure its own instructions mandate |
| State | Open, with the operator |

## What three agents found separately

`CLAUDE.md` section "How to work a ticket" is the mandated procedure for every role. Its steps 1, 2, 5 and 6 are `aicd_context(ticket_id)`, `aicd_plan_submit`, the pull request, and `aicd_report kind=closing`. Every one of its eight escalation triggers is specified as an `aicd_escalate(trigger=...)` call. `spec/README.md` and `spec/agents/documentation.md` route every specification change through `aicd_spec_propose`.

**None of those tools exists.**

```
$ wc -l < crates/ori-mcp/src/lib.rs
22
$ grep -c 'aicd_' crates/ori-mcp/src/lib.rs
0
$ grep -c 'aicd_' CLAUDE.md
12
```

`crates/ori-mcp/src/lib.rs` is a module doc comment. The tools are `spec/API_SPEC.md`'s description of **this product's** MCP surface, which phase 1 builds and has not built yet.

ORI-T-0085's coder reported it, then ORI-T-0087's, then ORI-T-0089's. Each found it independently, because nothing in the tree told them.

## Why it is being recorded rather than worked around

The consequences are real and they are already here:

1. **The scope lock is not enforced by anything.** `aicd_plan_submit` is what returns `E_SCOPE_LOCKED`. No ticket in this project has had its declared scope checked by anything except the coder's own care and the lead's reading of `ops/lock-table.md`. That is the control `ops/incidents/INC-0002` exists because of.
2. **No escalation can be fired.** ORI-T-0089 reached `precondition_missing` and could not raise it, because `aicd_escalate` is one of the missing tools. An agent that hits a trigger and cannot report it through the mandated channel is the "present but reporting nothing" class, applied to the escalation path itself.
3. **The documentation role cannot write the way its role file requires.** Its only stated write path is `aicd_spec_propose`. Every documentation ticket so far has run on a direct override from the lead, given in a prompt, which is how rulings R15 to R24 were lost.
4. **It is the same defect this round has been fixing.** A requirement written as an observation. `CLAUDE.md` states the procedure in the present tense as a thing an agent does, and the agent cannot do it. Round 3 corrected exactly that shape in `spec/TESTING.md` section 5 and in `CLAUDE.md`'s fixtures bullet, while this one sat in `CLAUDE.md`'s own workflow section.

## The lead's error, recorded

The lead told ORI-T-0089's documentation agent that this absence "is recorded". It is not:

```
$ grep -rn 'aicd_spec_propose' ops/
(no output)
```

The lead had written it in the body of pull request 32, which is not the repository. That is the third consecutive occurrence of the defect `ops/calibration/CR-006-the-lead-reasons-from-branches.md` names and states the remedy for. This file is the remedy applied, one occurrence later than it should have been.

## The question

`CLAUDE.md` and the role files describe a control surface that phase 1 has not built. Which is it:

1. **Mark the procedure as forward-looking**, the way ORI-T-0089 has just marked `spec/TESTING.md` section 5, and state the interim path: branch, worktree, declared scope in the lock table, report in the pull request. This is what every ticket already does. It is a paired instruction-file change under AICD §32 and AICD §29, so it is the operator's, not the lead's.
2. **Leave it, and accept that the instructions describe the destination rather than the road.** Defensible for a product that builds its own tooling, but then every agent that reads them rediscovers the gap, as three have.

**The lead recommends 1**, and recommends it be one ticket covering `CLAUDE.md`, `spec/agents/documentation.md` and `spec/README.md` together, because three documents state the same unbuilt procedure and fixing one leaves the others to reintroduce it. That is the lesson of ORI-T-0087 and ORI-T-0089, which were the same defect in two documents and had to be fixed twice.

## What this does not ask for

It does not ask for the tools to be built early. `ori-mcp` has its own place in the backlog and pulling it forward to satisfy a doc comment would be the tail wagging the dog. It asks that the instructions stop asserting a present tense they do not have.
