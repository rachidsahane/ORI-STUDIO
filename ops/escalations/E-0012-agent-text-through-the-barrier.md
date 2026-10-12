# Escalation E-0012: how agent plans, escalation text and run reports reach storage through the barrier

| Field | Value |
|---|---|
| Trigger | `contract_change` |
| Ticket and anchor | ORI-T-0057, slices S4 and S9, `ops/batch-10-plan.md`; `spec/DATA_MODEL.md` section 2 (`Plan`, `Report`, `MemoryRecord`), `spec/API_SPEC.md` section 3 (`aicd_report`), `spec/SECURITY_NOTES.md` ("free text from agents passes the sanitization barrier before storage") |
| Raised by | The batch 10 planning coder, 2026-10-11; recorded by the lead |
| Question | `barrier::submit_report` accepts six record kinds and is the only public entry to the barrier. How do agent plans, escalation text, and the "run and audit" reports API_SPEC names reach storage through it? |
| Recommendation | (1) `barrier.rs` gains one public entry that returns the sealed sanitized field, and the server's ports take that type, so barrier passage is checked at compile time. This is tier 2, after claim 81 releases `barrier.rs`. (2) `aicd_report.kind` is one of `closing`, `blocked`, `qa_run`, `drift_audit`, mapped onto the existing record kinds (closing report, blocked report, finding, finding), as `scope.rs` already maps QA runs. `content` is an object of named string fields, listed per kind from `templates/closing-report.md` and `templates/blocked-report.md`. (3) Kinds by role: coder {closing, blocked}; documentation {closing, drift_audit, blocked}; qa {qa_run, blocked}; operations {blocked} until phase 4 |
| What is blocked | Slice S4 beyond `closing` and `blocked`, which proceed now; slice S9 |
| State | Answered by the operator in session on 2026-10-11: **a sealed public barrier entry, and four report kinds** (`closing`, `blocked`, `qa_run`, `drift_audit`) mapped onto existing record kinds with an allowed list per role, as recommended |

### Context

The specification names different sets of report kinds in different places:
- `spec/API_SPEC.md` section 3: "closing, blocked, run and audit".
- `spec/DATA_MODEL.md` `Report.kind`: closing, blocked, qa_run, drift_audit, incident, post_mortem.
- `MemoryRecord.kind` and `barrier.rs`: six values.
- AICD §25: three kinds.

`Plan.content` and escalation text have no barrier path at all, and `sanitize_field` is private.

### Options rejected

| Option | Why not |
|---|---|
| A new record kind per text (plan, question, evidence) | Many new kinds, and plans and escalations are not operational-memory reports |
| Store plan and escalation text unsanitized | Contradicts SECURITY_NOTES |
| Make `sanitize_field` public as it is | It returns an unsealed `String`, so the compile-time guarantee is lost |
