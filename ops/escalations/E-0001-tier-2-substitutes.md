# Escalation E-0001: what a tier 2 change carries in phases 1 and 2

Format: AICD §12. Raised by the lead, batch 1 of phase 1.

| | |
|---|---|
| Trigger | `spec_conflict` |
| Raised by | Lead, during review of ORI-T-0011 |
| Blocks | Nothing today. ADR-0002 records the gap rather than closing it |
| Decides | Every tier 2 merge in phases 1 and 2, which is most of phase 1 |
| State | Open, with the operator |

## The question

AICD §38 requires five substitutes **together** for every tier 2 change under the single-operator profile. Three of the five have no mechanism in phases 1 and 2.

| Substitute (AICD §38) | Implemented by | Phase | Available now |
|---|---|---|---|
| Independent review by a different model, hard veto, adversarial checklist | PRD F-09, V-07, V-08 | 2 | Partially: the lead reviews through the checklist, but the cross-model refusal at identity creation is phase 2 and `broker.identity.create` does not exist |
| The operator's full diff read, separate session, several hours later | PRD V-10 | 3 | Yes, performed by hand. The enforced separate-session rule is phase 3 |
| Rollback rehearsed on staging for that specific change | PRD L-05, CI_CD §5, `runbooks/rollback.md` | 4 | **No.** No staging, and the runbook is phase 4 |
| Migrations backward compatible with the previous tag, behind a flag | PRD G-05 | 4 | **No mechanism, and it has an object.** ORI-T-0024 puts `crates/ori-store/migrations/**` in batch 3 of phase 1, tiered 2 by RISK_MAP |
| Limited rollout, operations agent authorised to halt | PRD R-01, O-03 | 3 and 4 | **No.** Nothing is deployed |

The distinction that matters, and that ORI-T-0011's first attempt got wrong: a substitute with no object is genuinely inapplicable, and a substitute with an object and no mechanism is a gap. The migration substitute is the second kind, in phase 1, in batch 3.

## Why the lead is not deciding it

Nothing in the specification answers it. §38 says the five are required together and that the core does not bend. PROJECT_BRIEF §8, PRD R-01, CI_CD §2 and DATA_MODEL §4 all defer to "the single-operator profile substitutes, recorded", and none says what happens when a substitute cannot be recorded because it does not exist. Writing a rule into an Accepted ADR to fill the gap would be the "present but reporting nothing" defect of AICD §39 in documentary form: a control that reads as protection in every document that cites it while protecting nothing.

## The lead's recommendation

Amend ADR-0002 to state an explicit interim rule, rather than leaving tier 2 governed by a substitute set that cannot be satisfied:

1. Name the three unavailable substitutes as unavailable, with the phase that delivers each. ADR-0002 already does this.
2. Require, for every tier 2 change in phases 1 and 2: the lead's adversarial checklist answered in writing, the operator's own diff read, and a written rollback line in the PR report naming the exact revert or restore procedure for that change.
3. For any tier 2 change touching `crates/ori-store/migrations/**` before phase 4, require additionally a rehearsed restore against a fixture database, since that is the one substitute with a real object in phase 1 and a fixture is a sufficient stand-in for staging.
4. Amend the ADR as each of the other three substitutes lands, and record the amendment as a parameter change under AICD §29.

The alternative, deferring every tier 2 module until phase 3, is not viable: phase 1 is almost entirely tier 2 modules.

## What happens until it is answered

Tier 2 PRs continue to stop for the operator, which they would anyway. The gap is recorded in ADR-0002 and here. No tier 2 change is blocked by this escalation; what is missing is the written rule for what those changes must carry.
