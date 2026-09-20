# AICD Change Proposal

The only way to add an agent role, a ticket category, a test family, a document type or a compliance overlay to AICD. Written into organizational knowledge, reviewed by every seat, adopted with a minor version increment.

Source: derived, not reproduced. AICD §29 lists what a Change Proposal contains and the condition under which it is adopted. Appendix A has no entry for this artifact, so the field list below is this product's arrangement of that list into a form, not one the methodology specifies.

## Fields

| Field | Required | Content |
|---|---|---|
| Title and identifier | Yes | One line, and a stable identifier |
| Proposed by | Yes | Seat and date |
| Problem observed | Yes | What happened on a real project, with evidence from the metrics of AICD §21. Not a hypothesis |
| Proposed extension | Yes | What is added, stated precisely enough to implement |
| Founding principle served | Yes | Which principle of AICD §3 it serves, and how |
| Violates none | Yes | A demonstration, principle by principle, that no founding principle is broken |
| Compensation structure replaced | Yes (`none` if none) | What existing structure it replaces, and the evidence that the limitation it compensated for is gone |
| Parameters introduced | Yes (`none` if none) | Each new parameter, and how it is calibrated (AICD §30) |
| Permission changes | Yes (`none` if none) | Every permission this changes, and the control that accompanies it |
| Seat reviews | Yes | One row per seat, with its verdict |
| Version increment | Yes | Minor, for an adopted extension |

## Form

| Field | Value |
|---|---|
| Title and identifier | |
| Proposed by | |
| Founding principle served | |
| Compensation structure replaced | |
| Version increment | Minor |

### Problem observed

| Evidence | Metric | Value |
|---|---|---|
| | | |

### Proposed extension

### Violates none

| Founding principle | Why it is not violated |
|---|---|
| | |

### Parameters introduced

| Parameter | Default | How it is calibrated |
|---|---|---|
| | | |

### Permission changes

| Permission | Control that accompanies it |
|---|---|
| | |

### Seat reviews

| Seat | Verdict | Date |
|---|---|---|
| | | |

## Rules

- A proposal is adopted only when every seat has reviewed it and the reliability and governance seat has confirmed it changes no permission without a corresponding control (AICD §29).
- Parameters and tooling do not need a proposal and do not change the version. Core is not changed by a team (AICD §29).
- Extensions never invalidate what a product already does. A product adopts a minor version by adopting the extension (AICD §29).
