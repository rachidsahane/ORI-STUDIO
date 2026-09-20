# Runbooks

Procedures the operations agent may execute and humans rehearse. Each runbook states: trigger, preconditions, steps, verification, rollback, and who may run it. A runbook that has never been rehearsed on staging is not installed (AICD §14 applies to runbooks as to gates).

| Runbook | Trigger | May run |
|---|---|---|
| release.md | Tag `v*` pushed | Release pipeline; final publish approved by a human |
| rollback.md | Release defect confirmed | Operations agent, human confirms |
| recover-engine.md | Engine restart after crash | Engine automatically; human checks |
| rotate-credentials.md | Schedule or exposure | Human; operations agent verifies |
| prove-gate.md | New gate defined | Coder agent under a ticket; verification lead accepts |

Written in phase 1 (recover-engine, prove-gate) and phase 4 (release, rollback, rotate-credentials), each rehearsed before the phase closes.
