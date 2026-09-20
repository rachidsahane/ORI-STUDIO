# Blocked report

What an agent writes instead of continuing when any budget limit is exceeded, or when it cannot proceed. Writing it and handing off is the only permitted end to an exhausted budget.

Source: AICD appendix A.5, reproduced field for field. The field table adds nothing. The note below the form is not from appendix A.5 and names its section.

## Fields

| Field | Required | Content |
|---|---|---|
| Ticket | Yes | Link |
| Budget consumed | Yes | Attempts, time, tokens |
| Approaches tried | Yes | Each approach and why it failed |
| Hypothesis | Yes | Why the agent believes the ticket is blocked |
| What it would try next | Yes | With the reasoning |
| What it needs from a human | Yes | A decision, information, a permission, a specification clarification |

## Form

| Field | Value |
|---|---|
| Ticket | |
| Budget consumed | attempts / time / tokens |
| Hypothesis | |
| What it would try next | |
| What it needs from a human | |

### Approaches tried

| Approach | Why it failed |
|---|---|
| | |

## Note, not from appendix A.5

AICD §12 states where this report goes: blocked reports are written to operational memory so the next attempt starts informed. Appendix A.5 fixes the fields only.
