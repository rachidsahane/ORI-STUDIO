# Ticket

The unit of work. Every change, whether requested by a human or discovered by an agent, is a ticket, filled at filing time and completed at validation.

Source: AICD appendix A.2, reproduced field for field. The field table adds nothing. The rules below the form are not part of appendix A.2; each names the section that states it.

## Fields

| Field | Required | Content |
|---|---|---|
| Title | Yes | One line, imperative |
| Category | Yes | Auto, Behavioral, Decisional, Product signal |
| Proposed risk tier | Yes | 0, 1 or 2 |
| Specification anchor | Yes | Section implemented, violated or to be modified |
| Acceptance criteria | Yes | Identifiers covered by this ticket |
| Evidence (defects) | Yes for defects | Reproduction steps, logs, traces, session replays, severity, affected users |
| Budget | Yes | Max attempts, max wall-clock time, max tokens |
| Filed by | Yes | Human or agent identity |
| Closing requirements | Yes | Specification update reference, new criteria reference |

## Form

| Field | Value |
|---|---|
| Title | |
| Category | |
| Proposed risk tier | |
| Specification anchor | |
| Acceptance criteria | |
| Evidence (defects) | |
| Budget | |
| Filed by | |
| Closing requirements | |

## Rules that apply to a ticket

- A ticket that deletes, drops, revokes or rewrites states a belief and its source, never a conclusion. It begins with a discovery stage that changes nothing and ends with the human's decision. AICD §39 states this rule and states that the ticket template carries it.
- Every precondition this ticket names is verified to exist at approval time, not at execution time. Record the verification beside the precondition. AICD §39 records this as a rule adopted; it does not say the ticket template carries it. It is carried here because `CLAUDE.md` makes `precondition_missing` an escalation trigger, which a ticket filer needs to know at filing time.
- The category may be upgraded by an agent and never downgraded by one. Only a human downgrades (AICD §11, the upgrade-only rule).
