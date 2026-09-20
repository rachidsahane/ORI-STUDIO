# Verification record: ORI-T-0008

`ops/phase-1-backlog.md` line 63 states this ticket as "`.claude/agents/` from `spec/agents/*.md`, `fable` acceptance verified". This file is the verification half. It separates what evidence on this machine establishes from what it does not, and names what would establish the rest.

Checks run 2026-09-21 on the operator's machine, Darwin 25.5.0. Repository `/Users/alimsahane/Documents/ORI STUDIO` at `ffab3fd`.

## 1. Every distinct `model:` value in the six role files

| File | `model:` |
|---|---|
| `spec/agents/assistant.md` | `fable` |
| `spec/agents/coder.md` | `opus` |
| `spec/agents/documentation.md` | `sonnet` |
| `spec/agents/lead.md` | `fable` |
| `spec/agents/operations.md` | `sonnet` |
| `spec/agents/qa.md` | `opus` |

Three distinct values: `fable`, `opus`, `sonnet`. No role file uses `inherit`, uses `haiku`, or names a full model id. `diff` of each copy in this directory against its `spec/agents/` original reports no difference on any of the six, so the same three values hold here.

## 2. What was searched, and what was found

**a. The repository.** Root contains `.git`, `.gitignore`, `CLAUDE.md`, `methodology/`, `ops/`, `spec/`. There is no `.claude/`, no `scripts/`, no `.github/`, and no code in any language. Nothing in the repository validates, generates, or checks a `model:` value or the contents of this directory.

**b. `spec/CI_CD.md` section 1.** Fourteen required gates. Gate 9 is the citation gate, gate 14 is the diagram gate and is scoped to diagrams under `spec/`. None of the fourteen reads `.claude/agents/`, compares it with `spec/agents/`, or inspects frontmatter. The liveness gate named in the same section covers workflows that exist; it cannot cover a gate never written.

**c. The installed Claude Code binary.** `/Users/alimsahane/.local/bin/claude` is a symlink to `/Users/alimsahane/.local/share/claude/versions/2.1.278`, the newest of the five versions present. Strings recovered from that binary:

- The agent-definition schema's description of the `model` field: "Model alias (e.g. 'fable', 'opus', 'sonnet', 'haiku') or full model ID (e.g. 'claude-fable-5'). 'inherit' uses the main model; if omitted, uses the default subagent model when one is configured, else the main model".
- Two alias arrays: `T1=["sonnet","opus","haiku","fable","best","sonnet[1m]","opus[1m]","fable[1m]","opusplan"]` and `["sonnet","opus","haiku","fable"]`. A set is built from `T1` plus catalog model ids and tested with a trailing `[1m]` stripped.
- A tier-name array `["sonnet","opus","haiku","fable","mythos"]`.
- A validator branch for agent files, keyed on file type `agent` and scope `project`. Where the frontmatter block is absent it returns success and emits no warning for a project-scope agent. Where the frontmatter is present but unparseable it emits, for a project-scope agent, "At runtime this agent does not load at all" followed by "with no frontmatter name it is treated as a co-located reference document and skipped"; for any other scope, "At runtime this agent loads with its name taken from the filename and every other frontmatter field silently dropped."

**d. Third-party documentation on this machine.** `/Users/alimsahane/.claude/plugins/marketplaces/claude-plugins-official/plugins/plugin-dev/skills/agent-development/SKILL.md` gives the `model` field's format as `inherit/sonnet/opus/haiku` in its frontmatter table. `scripts/validate-agent.sh` in the same skill accepts `inherit|sonnet|opus|haiku` and warns "Unknown model: $MODEL (valid: inherit, sonnet, opus, haiku)" on anything else. Both omit `fable`, and both disagree with the binary shipped alongside them. Both are plugin-authoring aids, not the loader. Treated as outdated, and noted because a future agent running that script against `assistant.md` or `lead.md` will get a warning that is wrong.

## 3. Established

1. `fable` is a recognized model alias in the Claude Code build installed here (2.1.278). It appears in the agent-definition schema's own description of the `model` field, in both alias arrays, and in the tier-name array. The sentence shipped in attempt 1, that `model` "is not checked against a list of permitted values", was wrong: alias lists exist in the binary.
2. The six role files carry exactly three distinct values, all three of which appear in every alias list found in 2c.
3. The six copies in this directory are byte-identical to `spec/agents/`.
4. Nothing in the repository or in `spec/CI_CD.md` regenerates this directory or detects drift from `spec/agents/`.

## 4. Not established

1. **Whether the operator's account is entitled to `fable` at run time.** An alias being in the binary's list is not an entitlement. This is the operator's to confirm. What would establish it: the operator checking the account's model access, or a single spawn of `assistant.md` or `lead.md` that succeeds and reports the model it is running on.
2. **Which of the alias lists the subagent loader actually consults, and what happens on a value the account cannot use.** The strings in 2c were read out of a binary, not observed in use. Whether an unusable alias fails at load, fails at spawn, or falls back silently to another model is unknown. A silent fallback matters here: it would put the lead on a coder's model without the lead knowing. What would establish it: spawning each of the six subagents once and recording the model each reports.
3. **Whether `spec/agents/` is meant to hold seven role files.** `product_signal` is in AICD §7's role table, in the `AgentIdentity` enum in `spec/DATA_MODEL.md`, in the `spec/ENV_SETUP.md` permission manifest, and in `spec/PRD.md` F-10 and O-01, which place it in phase 4. No document says whether its role file is deferred to phase 4 or missing from the foundation set. What would establish it: an operator ruling, or a `spec/README.md` row stating the intent.

## 5. The control `lead.md` carries, as `lead.md` states it

Line 7: "You are on a different model than the coders you review, on purpose; if you detect you have been given the same model, refuse to start (AICD §7)." The refusal condition is detecting the same model as the coders it reviews. It is not a condition about any particular model identity.

Last line: "If a safety fallback changes your model mid-review on a tier 2 change, say so explicitly in the review and mark it for human reading." A model substitution during a review is handled by disclosure. It is not a stop condition. Attempt 1 stated the opposite.

AICD §7, "Why the lead agent runs on a different model", requires the lead to run on a different model family than the coders it reviews and gives the reason: shared blind spots. It states no rule for a substitution mid-run.

On disk the configuration satisfies the rule: `lead.md` is `fable`, the coders in `coder.md` are `opus`. Whether it is satisfied at run time depends on item 2 of section 4.

## 6. Whether this file and `README.md` are safe in this directory

The claim under test: a `.md` file in `.claude/agents/` with no YAML frontmatter is not loaded as a subagent. `README.md` and this file both have no frontmatter.

Evidence for it, from 2c: the validator in 2.1.278 emits, for a project-scope agent file whose frontmatter does not parse, "At runtime this agent does not load at all" and "with no frontmatter name it is treated as a co-located reference document and skipped". It emits no warning at all when the frontmatter block is absent for a project-scope agent.

That is the shipped binary describing its own runtime behavior. It is not a load observed on this machine, and the quoted message is attached to the unparseable-frontmatter branch rather than the absent-frontmatter branch. Treat it as a strong indication, not a fact. What would establish it: listing the available subagents in a session where this directory is the project's `.claude/agents/`, and confirming that no agent named `README` or `VERIFICATION` appears.

If it turns out these files are loaded, the fix is to move both out of `.claude/agents/`, not to add frontmatter to them.
