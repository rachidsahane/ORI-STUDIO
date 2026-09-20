# PR report: ORI-T-0000, batch 0, specification corrections

Format: AICD appendix A.3.

| Field | Value |
|---|---|
| Ticket and anchor | ORI-T-0000. Anchors: every document changed, listed below. No code. |
| Branch | `spec/ORI-T-0000-batch-0` |
| Category | Decisional (it changes the specification, AICD §11) |
| **Risk tier** | **2.** `RISK_MAP` tiers `spec/` at 1 but at **2 for SECURITY_NOTES, ENV_SETUP, RISK_MAP and ADRs**. This PR changes all four. |

## Plan, and deviations from it

The plan was your ruling list, applied verbatim. Three passes were needed, and the second and third are deviations from the plan worth your attention:

- **Pass 1** applied your fourteen rulings on disjoint file scopes.
- **Pass 2** applied ten consequential edits your rulings imply in documents the rulings did not name. Without them batch 0 would have shipped new contradictions of exactly the class it exists to remove: PRD's Phase column for L-03 and M-05, ARCHITECTURE's and CLAUDE.md's credential boundary, `lead.md`'s merge rights, DATA_MODEL's Gate enum, the `Ticket.kind` interfaces, PRD Z-01's notification scope, and CI_CD's release assets.
- **Pass 3** closed four remaining self-contradictions, including one in ROADMAP's own build order.

**One deviation is my error, not yours.** I instructed pass 1 to *move* the inert workflow out of `fixtures/migrated-with-drift` into the new `fixtures/inert-gate`. That broke ORI-P1-024 and your phase 1 exit criterion, both of which require `ori migrate` to detect an inert workflow in `migrated-with-drift`. Your four-fixture ruling is additive. Pass 3 restored it: `migrated-with-drift` keeps its inert workflow because migration must detect one, and `inert-gate` is a separate fixture that proves the liveness gate.

## Edits outside your ruling list, each declared

| Edit | Why |
|---|---|
| Root `CLAUDE.md`: "three fixtures" to "four fixtures" | Your ruling named LLD and TESTING. Leaving CLAUDE.md at three would have created a fresh contradiction in the file every agent reads. |
| Root `CLAUDE.md`: `methodology/AICD_Methodology.html` to `AICD_Methodology_v0.3.html` | The named file does not exist. Agents were being sent to a path that does not resolve. |
| `SECURITY_NOTES` and `RISK_MAP`: `PERMISSIONS.md` to `ENV_SETUP.md` §5 | Your ruling put this fix in README, but README never mentioned `PERMISSIONS.md`; the two references live in these files. |
| `ENV_SETUP` §6: lead's forbidden action from "Merge a tier 1 pull request" to "Merge any pull request" | Direct consequence of "the lead holds no merge token". The old test was weaker than the new rule. |
| `DATA_MODEL`: Gate enum gains a value for the forbidden-action test (gate 8) | Pre-existing gap, not caused by batch 0. Batch 0 is the batch that syncs the gate list to the enum, so closing it here was a one-word fix. |
| `PRD` O-01: "its own ticket kind" reworded | `kind` became a reserved field in this PR; the sentence now reads as a false claim about it. |
| `spec/agents/qa.md`: proposed `kind` named alongside proposed category | The instruction layer must match the MCP tool contract (AICD §32). |

## Tests

**None. There is no code, no test harness and no CI in this repository yet.** That is not an omission, it is the state of the tree: `main` has no commits and there is no Rust toolchain. The gates that would check this PR are built in batch 1.

Verification was therefore human-and-agent review, not automated:

| Check | How |
|---|---|
| Every ruling applied to the exact text named | Two independent reviewers per scope in pass 1, one per scope in pass 2, each running `diff -u` against the pristine baseline |
| No scope creep | Same reviewers, hunk by hunk: every hunk must trace to a ruling |
| No new cross-document contradiction | Three repository-wide consistency sweeps, the last one after the final edit |
| House rules (no em dashes, Mermaid only, `AICD §n` form, no renumbering) | Grepped across the whole working copy |

## Coverage matrix

Not applicable: no criterion is implemented by this PR. Batch 0 changes documents that phase 1 criteria depend on, and the criteria themselves are unchanged except ORI-P1-014, whose expected result you ruled narrower.

## Not tested, and why

- **That the corrected citations resolve.** The citation gate is `Defined`, not `Installed`, by your ruling, and stays so until you fix the methodology HTML anchors. The corrections were verified by reading the methodology; they are not machine-checked. This is the honest weak point of this PR.
- **That the specification set is now free of contradictions.** Three sweeps found progressively fewer, and the last found none that is must-fix. That is evidence, not proof. Six findings remain open by design and are listed as escalations.
- **That `flows.documents.approve` has no other callers.** Grepped across all markdown; `signOff` appears nowhere. Not machine-checked.

## Rollback plan

`git revert` of the merge commit. There is no code, no schema, no deployment and no state: the entire change is markdown in `spec/` and one root instruction file. Rollback is complete and instantaneous, and nothing depends on batch 0 until batch 1 starts.

## Tier 2 under the single-operator profile (AICD §38)

Stated honestly rather than ticked. Of the five substitutes:

| Substitute | State |
|---|---|
| Independent review by a different model than the author, with a hard veto, through the adversarial checklist | **Partial.** Editors and reviewers ran as separate agents with separate contexts and the reviewers had a veto I honoured, but I cannot assert they ran on a different model family: the mechanism that would enforce that, ADR-0001's family decision, is what this PR introduces and `broker.identity.create` does not exist yet. |
| The operator's own full diff read, in a separate session from plan approval, several hours later | **Yours to perform.** The diff is 21 files. |
| Rollback rehearsed on staging before merge | **Not applicable.** No staging exists; rollback is `git revert`. |
| Migrations backward compatible, behind a flag where behavioral | **Not applicable.** No migrations, no code. |
| Limited rollout with the operations agent authorised to halt | **Not applicable.** No rollout. |

**Flag:** batch 0 is a tier 2 change merged before `ADR-0002-single-operator.md` exists to record the exception governing it. ADR-0002 is ORI-T-0011 in batch 1. If you would rather the ADR precede its first use, ORI-T-0011 can be pulled out of batch 1 and merged first, as a one-file PR.

## Escalations encountered

Six, none resolved by me, all listed with recommendations in `ops/phase-1-backlog.md`: the organizational repository's absence, the fixture ordering, five new crate dependencies, the citation gate's scope, the DESIGN.md filename mismatch, the model family storage location, the API schema path, `Ticket.kind` having no setter, and M-05's fourteen documents splitting across phases.

Two of those change a contract and are therefore yours before batch 1 planning: **`Ticket.kind` has no setter** and **the model family cannot be stored where your ruling puts it**.
