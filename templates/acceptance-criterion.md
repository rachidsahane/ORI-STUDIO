# Acceptance criterion

One observable behavior the system must have. Written by a human before the behavior exists, mapped to tests by agents, and required before a defect ticket can close.

Source: AICD appendix A.1, reproduced field for field. The field table adds nothing. The note below the form is not from appendix A.1 and says so.

## Fields

| Field | Required | Content |
|---|---|---|
| ID | Yes | Product code, feature code, sequence number. Example: PAY-REFUND-004 |
| Type | Yes | functional, security, performance, resilience, simulation |
| Precondition | Yes | The state of the system and the actor before the action |
| Action | Yes | What the actor does, in observable terms |
| Expected result | Yes | What must be observable afterwards, including side effects and what must not happen |
| Risk tier | Yes | 0, 1 or 2, according to the area touched |
| Notes | Yes (`none` if none) | Edge cases, related criteria, known limitations |

## Form

| Field | Value |
|---|---|
| ID | |
| Type | |
| Precondition | |
| Action | |
| Expected result | |
| Risk tier | |
| Notes | |

## Note, not from appendix A.1

Appendix A.1 defines one criterion. It does not define a file that collects several, and it does not fix a column order for one.

`spec/criteria/phase-1.md` is the only collected criteria file in this repository. It declares "Format per AICD appendix A.1" and then tabulates six of these seven fields as columns, in this order: `ID`, `Type`, `Tier`, `Precondition`, `Action`, `Expected result`. It carries no `Notes` column, and it writes `Tier` where appendix A.1 writes `Risk tier`.

Whether that is the convention for every collected file in this product, or a shape particular to that one file, is not stated anywhere in `spec/`. `spec/CONVENTIONS.md` is where a documentation-role ticket would state it. Until it does, copy the shape of `spec/criteria/phase-1.md` and do not treat it as specified.
