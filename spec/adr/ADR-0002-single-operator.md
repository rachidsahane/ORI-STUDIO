# ADR-0002: Single-operator exception to the tier 2 merge policy

| | |
|---|---|
| Status | Accepted |
| Date | September 2026 |
| Deciders | Architect and reliability and governance (Alim Sahane, holding both seats) |
| Methodology | AICD §38 (profile: the single operator), AICD §13 (git and release workflow: merge policy by risk tier), AICD §18 (human roles), AICD §39 (lessons from the first application: the no-way-back migration rule) |
| Escalation scope | Any change to the substitutes named in "Decision", to the tier 2 surface in `SECURITY_NOTES` or `RISK_MAP`, or to who holds a seat, is decisional and tier 2, and is escalated with trigger `adr_area` |

## Context

AICD §13 sets one merge requirement for tier 2: "CI green, lead agent approval, two humans, written rollback plan, and at least one human reads the diff itself." Ori Studio has one human. PROJECT_BRIEF §2 and §8 state it: the operator holds all four seats of AICD §18, architect, verification lead, reliability and governance, and product owner, so the second human of §13 does not exist and cannot be produced by rearranging the seats.

The core does not bend. AICD §38 opens with the point and does not soften it: "Tier 2 requires two humans. A single operator cannot provide that, and the core does not bend. This profile records the only accepted substitute, so that a solo operator runs the methodology with a documented, expiring exception rather than a silent gap." The requirement is not lowered here, and no module is moved out of tier 2 to avoid it. What this ADR records is the exception and its five substitutes, and what part of them is real today.

What the exception governs is not a corner of the product. It is the control structure, named in `SECURITY_NOTES` "Tier 2 modules" and tiered again by `RISK_MAP`:

| Surface | Items |
|---|---|
| Control structure | `ori-core` state machines and the permission function; `ori-orchestrator::lifecycle` (tier and category rules) |
| The merge path | `ori-orchestrator::merge_queue`; `ori-integrations::*::merge` and webhook verification |
| Credentials and identity | `ori-broker` (all); `ori-rpc::auth` and transports |
| Isolation | `ori-runtime::container`, `ori-runtime::injector` |
| Memory boundaries | `ori-memory::barrier`, `ori-memory::scope` |
| Audit trail and data | `ori-store::event_log`; schema migrations |
| Gate integrity | `ori-gates::prover`, `ori-gates::liveness` |
| What agents can reach | `ori-mcp::tool_scopes` |
| Detection | `ori-watch::attribution` |
| Supply chain | the release pipeline and signing; `.github/workflows` and `scripts/release*` (`RISK_MAP`) |
| Specification | `SECURITY_NOTES`, `ENV_SETUP`, `RISK_MAP` and the ADRs, tiered at 2 by `RISK_MAP`; the `SECURITY_NOTES` table does not list them |

Seven of the eighteen tickets in batch 1 of phase 1 are tier 2 against that map. Every batch from batch 2 to batch 13 carries at least one tier 2 ticket. Batches 14 and 15, the CLI and the fixtures, carry none.

The exception has been in force since before this record existed. Batch 0 changed `SECURITY_NOTES`, `ENV_SETUP`, `RISK_MAP` and the ADRs, self-assessed tier 2, and merged with one human, no second approval and no ADR; its own report flags that and offers to have this ticket merged first. This ADR is therefore written after its first use, and it merges under the exception it records. Both facts are stated rather than tidied away.

## Options considered

- **Build no tier 2 module until a second seat exists.** Honest and useless: the tier 2 surface is the control structure, so phase 1 stops at batch 1 and the product stops with it. The date a second seat is filled is unknown, so this is a deferral with no term, and it also defers the only thing that makes the rest of the methodology enforced rather than advisory.
- **Treat tier 2 as tier 1: one human, one approval, no substitutes.** Cheapest, and it deletes the control precisely where damage is irreversible: credentials, the merge path, the audit trail, migrations. The tier would still be written in `RISK_MAP` and would protect nothing, which is the failure AICD §39 records as a control that existed on paper. Rejected: it removes the control instead of substituting for it.
- **Recruit a second reviewer for tier 2 only, from outside the seats.** Produces a second pair of eyes without a second seat, but a reviewer who owns nothing, has not read the specification and holds no permission approves without the judgment AICD §18 attaches to a seat, and it puts a person outside the permission model on the merge path. There is also nobody to recruit today. Rejected.
- **Apply the five substitutes of AICD §38, together, on every tier 2 change, and record them on the merge.** The only option the methodology accepts. It costs the operator two separated sessions and a rehearsal per tier 2 change. Its weakness is that the substitutes depend on tooling and environments that ROADMAP schedules in phases 2 to 4, so the profile cannot be carried in full during phases 1 and 2. Chosen, with that weakness recorded below rather than hidden.

## Decision

Ori Studio runs under the single-operator profile of AICD §38 until a second seat is filled. Tier 2 is not lowered, and no module is exempted from it. The rule of the profile is that every tier 2 change carries all five substitutes together.

| # | The rule this project follows (AICD §38) |
|---|---|
| SO-1 | Independent review by a different model than the coder, with a hard veto rather than an advisory opinion, working through the adversarial checklist. |
| SO-2 | The operator's own full diff read, in a separate session from the one in which the plan was approved, at least several hours later. |
| SO-3 | Rollback rehearsed on staging for that specific change before merge, and for changes with data migrations, a snapshot taken immediately before the migration with a rehearsed restore. |
| SO-4 | Migrations backward compatible with the previous tag, behind a flag where the change is behavioral. |
| SO-5 | Limited rollout where the product supports it, with the operations agent authorized to halt. |

That table is the destination, not the state of the project. Three of the five cannot be carried in phases 1 and 2; "What is in force today" states which and why, and "The interim" states that the rule for the meantime is undecided and is the operator's.

Where the satisfaction of a substitute is recorded is also phased. Today it is recorded in the pull request report, which the backlog's control points require of every tier 2 PR. `DATA_MODEL` §4 makes it an invariant of the merge event ("Every `PullRequest` merge event references the approvals that satisfied its tier, and for tier 2, two distinct approvers (or the single-operator profile's substitutes, recorded)"), and that invariant binds from the phase that builds the parts it names: the event log is ORI-T-0023 and the merge queue is ORI-T-0053, batches 3 and 8 of phase 1.

For SO-1 the unit of difference is the model family, decided in ADR-0001 and refused at identity creation, not at review time. AICD §38 attaches two further rules to the exception; they bind the agents and are stated under "Consequences".

### What is in force today

Stated against the specification, not against intent. "Phase" is the ROADMAP phase that delivers the function; "available now" is phase 1, batch 1. SO-3 is split, because its two clauses are in different states.

| Substitute | What implements it | Phase | Available now |
|---|---|---|---|
| SO-1, independent model review with a hard veto | PRD F-09 (a lead on its coders' model family refused at identity creation), V-07 (three-layer review), V-08 (the adversarial checklist as a versioned policy), G-04 (the tier check in the merge queue) | 2 | No |
| SO-2, separate-session diff read | PRD V-10 (the separate-session rule with a minimum delay), V-09 (the evidence panel the diff is read in), G-04 | 3 for V-09 and V-10, 2 for G-04 | No |
| SO-3a, rollback rehearsed on staging for that specific change | PRD Q-01 (staging orchestration), O-03 (rehearsal before a runbook is installed), L-05 (go-live rollback rehearsal); `runbooks/rollback.md`, which README schedules in phase 4 | 4 | No |
| SO-3b, snapshot immediately before a data migration, with a rehearsed restore | Nothing implements it for Ori Studio's own store. PRD Z-05 blocks no-way-back paths *detected while migrating a product into AICD*; it does not cover the engine's own schema | Z-05 is 3; a snapshot and restore for the engine's own store is unscheduled | No. And unlike SO-3a it has an object from batch 3 of phase 1 |
| SO-4, migrations backward compatible with the previous tag, flagged where behavioral | PRD G-05; the rule itself is written in CI_CD §5 | 4 | No. The rule is written and reviewable now, nothing checks it before G-05, and phase 1 has no previous tag to be compatible with |
| SO-5, limited rollout with the operations agent authorized to halt | Nothing implements it. CI_CD §4 publishes one tag to every channel at once and points the updater manifest at it; there is no staged rollout. The operations agent arrives with PRD O-01 and F-10. Staged rollout with automatic halt is specified only for the mobile profile, PRD R-02, priority P2, unscheduled | 4 for the operations agent; staged rollout unscheduled | No |
| The profile itself | PRD R-01: the profile enabled automatically when one human holds every seat, the five substitutes enforced, expiring ADR | 3 | No |

Read plainly: none of the five substitutes is enforced today, and R-01, the function that would enforce them, is a phase 3 function while the exception has been in force since the first tier 2 merge. Below that headline the rows are not alike, and the difference is the useful part of this record. A substitute with no object is genuinely inapplicable. A substitute with an object and no mechanism is a gap. One substitute can be performed in full by hand, and one can only be approximated.

- **SO-2 can be performed in full, by hand, from today.** It costs the operator a delay and the discipline to open the diff in a new session. Nothing records that it happened until V-10.
- **SO-1 can be approximated, not satisfied.** The operator can run review agents with a veto, which is what batch 0 did, but the batch 0 report states it could not assert a different model family, because the enforcement is F-09 in phase 2. The checklist the substitute names does not exist either: V-08 is phase 2. "Through the adversarial checklist" cannot be satisfied when there is no checklist to work through.
- **SO-3a, SO-4 and SO-5 have no object in phases 1 and 2.** There is no staging environment (Q-01, phase 4), no release, no deployed instance, no previous tag and no rollout: ROADMAP schedules the release pipeline in phase 4, and CI_CD §4 releases only on a tag. For a change that is code and documents only, rollback is `git revert` of the merge commit, which is what batch 0's rollback plan says and which is a real rollback path for that kind of change. It is not the substitute AICD §38 names, and calling it one would be a false claim of compliance.
- **SO-3b has an object inside phase 1, and no mechanism. This is the gap in this table that loses data.** ORI-T-0024 (batch 3 of phase 1, tier 2, declared scope `crates/ori-store/migrations/**` and `crates/ori-store/src/db.rs`) builds the store's schema migrations, and `RISK_MAP` tiers `crates/ori-store/src/event_log.rs, migrations/` at 2 for audit trail and data integrity. ARCHITECTURE §8 runs migrations forward on open and names no down-migration; CI_CD §5 states the same forward-only rule for users' machines. The data those migrations run against is not hypothetical: a ROADMAP phase 1 exit criterion is that the operator's own SM Pronostic migration backlog is executed through the CLI for at least one full phase, so a real store holding a real append-only, hash-chained event log (ORI-T-0023) exists inside phase 1. `git revert` of a merge commit does not restore a database that a forward-only migration has already altered. The substitute AICD §38 names for exactly this case, a snapshot taken immediately before the migration with a rehearsed restore, has nothing implementing it, and PRD Z-05 does not reach it: Z-05 is phase 3 and is written for no-way-back paths detected while migrating a product into AICD, not for the engine's own store. AICD §39 states the consequence as a block, and it is carried into "What the agents must respect" below rather than deferred.

### The interim, which is a gap and not a policy

Three documents assert the substitutes as the alternative to two humans: PRD R-01, CI_CD §2 ("two humans or the single-operator profile substitutes, recorded") and DATA_MODEL §4. The same specification schedules them in phases 2 to 4. Tier 2 changes merge from batch 1 of phase 1. Nothing in `spec/` states what a tier 2 change carries in between. The operational record reports the five substitutes per tier 2 pull request (`ops/phase-1-backlog.md` control points, `ops/tickets/ORI-T-0000-batch-0.md`), which answers what to report and not what is required, and which is a reporting practice rather than a control.

**Undecided. This requires the operator's decision, and this ADR does not make one.** No interim rule is in force, and this ADR grants no discretion in place of one. Until the operator writes a rule here through a specification PR, every tier 2 change merged in phases 1 and 2 merges with the gap open, and states in its pull request report, per substitute, whether it was satisfied, approximated or had no mechanism.

The lead's recommendation, reproduced verbatim so that the decision has a concrete proposal to accept or reject, escalated under trigger `spec_conflict` and not decided here:

> name the three as unavailable in ADR-0002 with an explicit interim rule that tier 2 changes in phases 1 and 2 carry the two substitutes that exist plus a written rollback line in the PR report, and that the ADR is amended when each of the other three lands

Two facts from the table above bear on it, and the operator should weigh them before accepting. First, "the two substitutes that exist" are SO-1 and SO-2, and the table records SO-1 as approximable rather than satisfiable while V-08's checklist does not exist; on the table's reading one substitute can be carried in full and one in part. Second, the three unavailable substitutes are not uniformly without an object: SO-3b acquires one at batch 3 of phase 1, so a rule that defers all three until a later phase lands leaves the migration gap uncovered for the rest of phase 1.

An ADR claiming five controls in force while none is enforced would be the "present but reporting nothing" defect of AICD §39 in documentary form, which is the defect this project's own gate-proving rule exists to prevent. An ADR that instead left what a tier 2 change carries to the judgment of the person making the change would be the same defect in the opposite disguise. Neither is written here.

## Consequences

**Merge order.** This ADR merges alone and first in batch 1, ahead of ORI-T-0012 to ORI-T-0017, the batch's six other tier 2 tickets, by the lead's merge-order ruling for batch 1. Each of those merges under the exception this record states, and a record that arrives after the changes it governs records nothing. It does not repair batch 0, which merged before this record existed; that is stated in Context.

**Easier.** The product can be built without a second seat, with the exception visible, bounded and expiring rather than silent. The tier stays where `RISK_MAP` puts it, so nothing has to be re-tiered when the second seat arrives.

**Harder.** Every tier 2 change costs the operator two sessions separated by hours, and the plan approval and the merge can never happen in one sitting. From phase 4 it also costs a rehearsal on staging per change. The lead agent's approval never substitutes for the operator's, so the operator is a serial bottleneck on the control structure, which is most of phase 1. And while the substitutes are unbuilt, every tier 2 merge is an exercise of an exception whose compensating controls are not yet in place, which is a standing risk the operator carries knowingly rather than one the tool absorbs.

**What the agents must respect.**

- No agent merges, at any tier. AICD §38 sets the floor ("no agent merges above tier 0"); Ori Studio is stricter and the stricter rule is the one in force here. The merge queue alone merges (CI_CD §2; ARCHITECTURE §9; SECURITY_NOTES "Auto mode and where control lives": "It cannot: it never held that credential; the merge queue alone merges"). No agent identity holds a merge credential: ENV_SETUP §5 gives the lead "approve tier 0 merges for the merge queue to perform" and "no merge credential". CONVENTIONS lists merging first under what agents never do. Tier 0 auto-merge is the queue acting on the lead's approval, not an agent merging, and it is additionally disabled for the whole of batch 1 by the operator's ruling, because no gate is proven yet.
- The operator merges every tier 1 and tier 2 change personally (AICD §38). An agent that finds itself holding a merge credential, or about to supply the second approval on a tier 2 pull request, stops and escalates.
- A lead and the coders it reviews never share a model family (ADR-0001), and a model fallback on a tier 2 review is an escalation (PRD F-09).
- Every tier 2 pull request carries a written rollback plan (AICD §13), and it states the real rollback path rather than a rehearsal that did not happen. While there is no release, that path is `git revert` of the merge commit for a change that is code and documents only. It is not `git revert` for a change that has already altered a database: a forward-only migration is not undone by reverting the commit that introduced it, and a rollback line that claims otherwise is false.
- **A change that adds or alters a schema migration is a no-way-back path until a pre-migration snapshot with a rehearsed restore exists.** AICD §39 states the rule and states it as blocking: "A deploy path with no way back is tier 2 and blocks agent autonomy on migrations until a pre-migration snapshot with a rehearsed restore exists." It applies from ORI-T-0024 in batch 3 of phase 1, not from phase 4. Until the snapshot and restore exist, an agent working such a ticket says so in its plan, does not claim SO-3b as satisfied or as not applicable in its report, and the ticket is worked with that block in force under the operator's decision at the control point.
- Every tier 2 pull request states each of SO-1, SO-2, SO-3a, SO-3b, SO-4 and SO-5 as satisfied, approximated, not applicable or unavailable, with the reason. "Not applicable" is claimed only where the substitute has no object; where it has an object and no mechanism, the word is "unavailable".
- No agent lowers a tier, widens a scope or reclassifies a module to avoid this exception. Any widening of permissions or lowering of a tier is a decisional ticket producing an ADR (PRD P-09).

## Revisit conditions

- **Expiry, as AICD §38 requires it.** The day a second seat is filled. Tier 2 reverts to two humans under AICD §13, and this ADR is superseded by one that records the restoration, its status becoming `Superseded (by ADR-000n)`. PRD S-05 makes the expiry automatic in the tool: when a second human takes a seat, the single-operator exception ADR expires and tier 2 reverts to two humans. S-05 is a phase 3 function, so until phase 3 the expiry is manual, and the operator supersedes this ADR in a specification PR on the day the seat is filled.
- **The availability table is rewritten at the close of phases 2, 3 and 4**, as F-09, V-08, V-10, R-01, Q-01 and G-05 land. A phase that closes without its rows changing state is a divergence and belongs in the drift audit (PRD D-09), not in silence.
- **On the operator's interim decision**, which is recorded in this ADR rather than in the backlog.
- **At batch 3 of phase 1, before ORI-T-0024 merges.** That is the known date on which SO-3b acquires an object, and the point from which AICD §39's block applies to this project's own store. Whether the interim decision has settled it by then or not, this ADR is reread at that point and the SO-3b row is restated against what exists.
- **When a mechanism for SO-3b is specified.** This ADR does not know which mechanism gives the engine's own store a pre-migration snapshot with a rehearsed restore, or in which phase, because no PRD row covers it: Z-05 covers products being migrated into AICD, not Ori Studio's own database. What would establish it is a PRD row and a ROADMAP phase, which are the architect's to write, not this ADR's to assume.
