# Escalation E-0011: `aicd_escalate` carries no evidence, and the orchestrator refuses an escalation without it

| Field | Value |
|---|---|
| Trigger | `contract_change` |
| Ticket and anchor | ORI-T-0057, slice S9, `ops/batch-10-plan.md`; `spec/API_SPEC.md` section 3, `aicd_escalate(ticket_id, trigger, question, recommendation)` |
| Raised by | The batch 10 planning coder, 2026-10-11; recorded by the lead |
| Question | How does an agent's `aicd_escalate` satisfy `ori-orchestrator`'s `Escalation::raise`, which refuses a trigger with no finding behind it? |
| Recommendation | Add `evidence: [{what, found_at}]` to the tool, 1 to 16 entries. This is additive, which `spec/API_SPEC.md` section 5 allows ("new tools are additive"; a new argument is the same kind of change). It also matches `templates/escalation.md`'s Context field ("what was found, where, with the evidence a human needs") |
| What is blocked | `aicd_escalate` only (slice S9) |

### Context

`crates/ori-orchestrator/src/escalation.rs` refuses to raise a trigger without a `Finding`, and its module doc says a verdict is not evidence. The tool as specified carries a question and a recommendation, both of them the agent's own words.

### Options rejected

| Option | Why not |
|---|---|
| The server builds one `Finding` from the agent's question | The evidence is then the agent's own assertion, which `escalation.rs` and the template exist to prevent |
| Let `Escalation::raise` accept an agent's escalation unverified | Weakens a control that exists so escalations are not noise |
