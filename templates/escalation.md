# Escalation

A question addressed to a human, raised the moment a trigger fires, with the agent's recommendation attached. The agent stops on the triggering matter until the answer comes back.

Source: derived, not reproduced. AICD §12 defines the escalation protocol, lists the triggers, and requires that each escalation be a question addressed to a human with the agent's recommendation attached. Appendix A has no entry for this artifact, so the field list below is this product's shape, not one the methodology specifies.

## Fields

| Field | Required | Content |
|---|---|---|
| Trigger | Yes | One of the triggers below, by name |
| Ticket and anchor | Yes | Identifier, and the specification section in question |
| Raised by | Yes | Agent identity and timestamp |
| Context | Yes | What was found, where, with the evidence a human needs to judge it. No conclusions presented as facts |
| Question | Yes | The single decision being asked of the human, phrased so that an answer resolves it |
| Recommendation | Yes | What the agent would do, and why |
| Options rejected | Yes (`none` if none) | Alternatives considered, each with the reason it was not recommended |
| What is blocked | Yes | What stops until the answer arrives, and what continues |

## Triggers

The first seven are the trigger list of AICD §12, under this product's names. The eighth, `precondition_missing`, is this product's trigger for the precondition rule AICD §39 adopted; §39 states the rule and does not itself name a trigger. `CLAUDE.md` lists all eight.

| Trigger | Fires when |
|---|---|
| `adr_area` | A change touches an area covered by an ADR |
| `test_modified` | A test had to be modified or removed for the build to pass |
| `new_dependency` | A new dependency or external service is needed |
| `spec_conflict` | The plan contradicts the specification |
| `scope_conflict` | Two tickets in progress conflict, or declared scope overlaps a claimed module |
| `contract_change` | A change would alter a public or internal contract |
| `security` | The report contains a security concern |
| `precondition_missing` | A precondition the ticket names does not exist |

## Form

| Field | Value |
|---|---|
| Trigger | |
| Ticket and anchor | |
| Raised by | |
| Question | |
| Recommendation | |
| What is blocked | |

### Context

### Options rejected

| Option | Why not |
|---|---|
| | |

Escalations land in the human review queue and are processed at a fixed cadence. Only incidents interrupt a human outside it (AICD §12).
