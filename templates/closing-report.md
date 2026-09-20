# Closing report

The structured record an agent writes into operational memory when its ticket ends. It is what a later agent reads to learn that an approach was already tried.

Source: derived, not reproduced. AICD §8 places agent closing reports in layer 3, operational memory, appended and never edited; AICD §25 states that every record in the operational log store carries the agent identity, the ticket, the product and the timestamp, and is indexed by module and by criterion. Appendix A has no entry for this artifact, so the field list below is this product's shape, not one the methodology specifies.

## Fields

| Field | Required | Content |
|---|---|---|
| Agent identity | Yes | The identity that did the work |
| Ticket | Yes | Identifier and link |
| Product | Yes | Product identifier |
| Timestamp | Yes | When the record was written |
| Modules touched | Yes | Paths. The record is indexed by module |
| Criteria covered | Yes (`none` if infrastructure) | Criterion identifiers. The record is indexed by criterion |
| Outcome | Yes | What the ticket produced, and the state it left the work in |
| Approaches tried and rejected | Yes (`none` if none) | Each approach and why it failed. This is what "we tried that and it broke X" means |
| Deviations from the plan | Yes (`none` if none) | What changed against the submitted plan, and why |
| Escalation decisions | Yes (`none` if none) | Trigger, the decision returned, who made it |
| Specification update | Yes | Reference to the documentation role's update, or the explicit statement that no change is needed |
| New criteria | Yes for defects | Criteria added so the case cannot return |
| What the next agent should know | Yes (`none` if none) | The finding that is not obvious from the diff |
| Provenance | Yes if any content derives from production | Names the evidence identifiers and confirms the content passed the sanitization barrier |

## Form

| Field | Value |
|---|---|
| Agent identity | |
| Ticket | |
| Product | |
| Timestamp | |
| Modules touched | |
| Criteria covered | |
| Outcome | |
| Specification update | |
| New criteria | |
| Provenance | |

### Approaches tried and rejected

| Approach | Why it failed |
|---|---|
| | |

### Deviations from the plan

### Escalation decisions

| Trigger | Decision | Decided by |
|---|---|---|
| | | |

### What the next agent should know

## Rules

- Agents write to operational memory only through structured records, never as free notes (AICD §25).
- Records are appended, never edited (AICD §8).
- Raw evidence is referenced by identifier and stored out of band. It is not pasted into this record (AICD §8).
- A ticket cannot close on a fix alone: the specification update, and new criteria for defects, are closing requirements (AICD §11, AICD §16).
