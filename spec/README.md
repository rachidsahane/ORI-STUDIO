# Specification: Ori Studio

Canonical knowledge of the product (AICD §9, §23). Every document has an owner seat and a state. Ori Studio's own readiness rule applies to this folder: Launch of phase 1 requires every foundation document approved and the phase 1 set approved.

| Document | Set | Owner seat | State |
|---|---|---|---|
| PROJECT_BRIEF.md | Foundation | Product owner, architect | Draft (generated with AI, awaiting approval) |
| methodology/ (AICD HTML + `sections.json`) | Foundation | Architect | HTML present as `AICD_Methodology_v0.3.html`; `sections.json` Missing: generated in batch 1 of phase 1 |
| adr/ADR-0001-stack.md | Foundation | Architect | Draft |
| adr/ADR-0002-single-operator.md | Foundation | Architect, reliability and governance | Missing: due in batch 1 of phase 1; records the AICD §38 exception, status accepted, expiring the day a second seat is filled |
| ARCHITECTURE.md | Foundation | Architect | Draft |
| DATA_MODEL.md | Foundation (entity level) and Phase 1 (detail) | Architect | Draft |
| SECURITY_NOTES.md | Foundation | Architect, reliability and governance | Draft |
| RISK_MAP.md | Foundation | Architect, reliability and governance | Draft |
| ENV_SETUP.md (with permission manifest) | Foundation | Reliability and governance | Draft |
| CONVENTIONS.md | Foundation | Architect | Draft |
| OBSERVABILITY.md | Foundation | Reliability and governance | Draft |
| CI_CD.md, runbooks/ | Foundation | Reliability and governance | Draft; in `runbooks/` only `recover-engine.md` and `prove-gate.md` are written (phase 1), `release.md`, `rollback.md` and `rotate-credentials.md` are phase 4 |
| ROADMAP.md | Foundation | Product owner, architect | Draft |
| agents/ (CLAUDE.md and role files) | Foundation | Architect | Role files Draft; `agents/CLAUDE.md` Missing: it is the canonical product base instruction file and is created in batch 1 of phase 1 from the copy at the repository root |
| templates/ (plan, closing report, blocked report, escalation, PR report, ticket, ADR, acceptance criterion, post-mortem, Change Proposal) | Foundation | Architect | Missing: no file exists; seeded from AICD appendix A and written in batch 1 of phase 1 |
| PRD.md (complete function catalog) | Foundation (catalog) and Phase 1 (phase 1 items) | Product owner | Draft |
| API_SPEC.md | Phase 1 | Architect | Draft |
| LLD.md | Phase 1 | Architect | Draft |
| TESTING.md | Phase 1 | Verification lead | Draft |
| criteria/phase-1.md | Phase 1 | Verification lead | Draft |
| QA test plan for phase 1 | Phase 1 | QA agent produces, verification lead verifies | Missing |
| design/DESIGN.md | Phase 3 (ROADMAP schedules the UI) | Product owner, architect | Draft |
| design/Ori Studio.html (the visual reference) | Phase 3 | Product owner, architect | Reference, approved for layout and tone; DESIGN.md binds a different filename in its DSN-001 row, `design/ori-studio-mockup-v1.html`, which is not present; the mismatch is open pending the operator's decision |
| criteria/personas.md | Phase 3 | Verification lead | Missing (not needed before phase 3) |

Not yet present, by design: the phase sets for phases 2 to 5, written when each phase approaches. The gate G2 artifacts under `design/` are present: `design/DESIGN.md` is this product's Vibe Engineer style DESIGN document, it binds the visual reference and elaborates the screen inventory in PRD section 6; the screens and flows it lists as still to design become design tickets in phase 3.

How to use this folder in Claude Code: read PROJECT_BRIEF.md, then the document your ticket anchors. `agents/CLAUDE.md` is the canonical product base instruction file and the `CLAUDE.md` at the repository root is a copy of it; today only the root copy exists, and batch 1 writes `agents/CLAUDE.md` from that copy and copies it back to the root. `agents/*.md` are the subagent definitions copied to `.claude/agents/` in the same batch.
