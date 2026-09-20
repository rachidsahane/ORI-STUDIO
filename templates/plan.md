# Implementation plan

What a coder submits after reading its context package and before writing any code. It is reviewed before the code exists, and its declared scope is what the lock table claims.

Source: derived, not reproduced. AICD §10 names the Plan step and states that the coder agent produces an implementation plan reviewed before any code exists; AICD §12 makes declared scope the input to the lock table; AICD §11 fixes the mandatory ticket contents this plan restates. Appendix A has no entry for this artifact, so the field list below is this product's shape, not one the methodology specifies.

## Fields

| Field | Required | Content |
|---|---|---|
| Ticket and category | Yes | Identifier, category, proposed risk tier as filed |
| Specification anchor | Yes | The section implemented, violated or to be modified |
| Criteria covered | Yes (`none` if infrastructure) | Criterion identifiers this ticket satisfies |
| Declared scope | Yes | Every module this plan will touch, by path. The lead refuses a ticket whose scope overlaps a claimed module |
| Out of scope | Yes (`none` if none) | Modules deliberately not touched, where a reader would expect otherwise |
| Approach | Yes | The steps, in order, with the decision behind each |
| Tests | Yes | The test to be added per criterion, and the level it sits at |
| Self-assessed risk tier | Yes | 0, 1 or 2, with the area that sets it |
| Budget | Yes | Max attempts, max wall-clock time, max tokens, as the ticket states them |
| Escalation triggers foreseen | Yes (`none` if none) | Any trigger of AICD §12 this approach is likely to hit, and when |

## Form

| Field | Value |
|---|---|
| Ticket and category | |
| Specification anchor | |
| Criteria covered | |
| Self-assessed risk tier | |
| Budget | |
| Escalation triggers foreseen | |

### Declared scope

| Path | What changes |
|---|---|
| | |

### Out of scope

### Approach

1.

### Tests

| Criterion | Test name | Level |
|---|---|---|
| | | |

## Rules

- Discovering mid-ticket that an undeclared module must be touched means stop and re-declare. If that module is claimed, it is an escalation with trigger `scope_conflict` (AICD §12).
- A plan that contradicts the specification is not implemented. It is escalated with trigger `spec_conflict` (AICD §12).
- Deviations from this plan are reported in the pull request report, not applied silently (AICD appendix A.3).
