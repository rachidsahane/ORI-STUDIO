# .claude/agents: copied from spec/agents/, not authored here

This directory holds the subagent definitions Claude Code loads for this repository. They are copies of `spec/agents/<role>.md`. The specification is the source of truth (AICD §9); these files are its build artifact.

Six role files are copied byte for byte: `assistant.md`, `coder.md`, `documentation.md`, `lead.md`, `operations.md`, `qa.md`. Their instructions are the role layer of AICD §32.

`spec/agents/CLAUDE.md` is not copied here. It is the product base layer, which AICD §32 keeps distinct from the role file, and it carries no YAML frontmatter and no `name`, `description`, `model` or `tools` key. Loading it as a subagent definition would produce a broken agent.

## How this directory came to be, and what does not maintain it

ORI-T-0008 copied the six files by hand. Nothing regenerates them.

No generator, gate, job or script exists that produces this directory from `spec/agents/`, and none is scheduled. `spec/CI_CD.md` section 1 lists the fourteen required gates; none of them reads `.claude/agents/`, compares it with `spec/agents/`, or checks a frontmatter value. The repository contains no `scripts/` directory and no `.github/` directory. Nothing currently detects drift between this directory and `spec/agents/`.

The gate that should exist and does not: a drift gate on every pull request that compares each `.claude/agents/<role>.md` byte for byte with `spec/agents/<role>.md`, and fails on any difference, on a role file present in one directory and absent from the other, and on a file in this directory that is neither a role file nor this README. It would be gate 15 in `spec/CI_CD.md` section 1. Adding it is a specification change and belongs to the documentation role. AICD §39 requires a gate to be proven on a planted defect before any artifact cites it, so this README cites it only as absent.

Until that gate exists, a copy here can diverge from the specification and nothing will say so. Treat the two directories as synchronized only by the change that touched both.

## Editing

An edit made here is an unattributed change to the fleet's configuration. It changes how an agent behaves with no ticket, no review, no tier and no record in the specification. It is not discarded by any process, because no process runs over this directory. It persists until a human notices, and every audit that reads `spec/agents/` reads the wrong file in the meantime.

To change a role, change `spec/agents/<role>.md` through the documentation role's specification PR (AICD §9), and copy the result here in the same change. AICD §32: "Changing an instruction file is a parameter or extension change under section 29, never a silent edit."

## What is not here

AICD §7's role table defines seven roles, and so does the `AgentIdentity` role enum in `spec/DATA_MODEL.md` (coder, lead, qa, operations, documentation, product_signal, assistant). `spec/agents/` holds six role files. The seventh, `product_signal`, has no role file, so it has no definition here either. `spec/PRD.md` F-10 and O-01 place the product signal agent in phase 4.

Whether the missing role file is a deliberate deferral to phase 4 or a gap in the foundation set is not stated anywhere in the specification. This ticket did not resolve it. It is recorded here, not closed.

## Verification

The `model:` values, whether `fable` is accepted, the control `lead.md` actually carries, and whether a file without frontmatter is loaded from this directory were checked against evidence on this machine. The record, including what the evidence does not establish, is in `VERIFICATION.md` beside this file.
