# R44. The lead does the design; coders on Sonnet 5.5, Haiku 5.5 for settled mechanical work; the lead verifies everything

| | |
|---|---|
| Ruling | R44 |
| Tier | 2: it changes which model writes code and what the lead hands a coder |
| Made by | The operator, in session, 2026-10-18. Recorded by the lead the same day |
| Narrows | [[R39]] |

## The decision

"When launching agents to work, do the logic reflection for them and make the coder use sonnet 5.5 and if the task is really easy coding and all the logic has been already done, put haiku 5.5 and the lead in opus 5.5 always verify what the coders did" (the operator, 2026-10-18).

1. **The lead does the design.** Before a coder is launched, the lead reads the code the ticket touches and writes, in the ticket package, the decisions a coder would otherwise make: the approach, the types and signatures where they matter, the refusal and its `MethodologyRef`, the tests and what each must prove. The coder implements and reports. A coder that finds the lead's design wrong says so and stops on that point, as CR-006 already asks; it does not redesign silently.
2. **Coders run on Claude Sonnet 5.5** ([[R39]], unchanged).
3. **Claude Haiku 5.5** is used only when the coding is plainly mechanical **and** every decision is already in the package: exact text to replace, exact names, exact files. A ticket that leaves a judgement to the coder is not a Haiku ticket. The lead records the choice in the claim.
4. **The lead, on Claude Opus 5.5, verifies every coder's work** before integration: the lead's own reading of the diff, and the gates re-run on the integrated tree. Independent review agents on Opus are added where the change is tier 2 or larger than a mechanical edit.

## What it does not change

[[R39]]'s statement on AICD §7 stands: no report claims SO-1. A Haiku coder and an Opus verifier are two lines of one provider too.
