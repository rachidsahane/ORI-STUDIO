# Specification: Ori Studio

Canonical knowledge of the product (AICD §9, §23). Every document has an owner seat and a state. Ori Studio's own readiness rule applies to this folder: Launch of phase 1 requires every foundation document approved and the phase 1 set approved.

| Document | Set | Owner seat | State |
|---|---|---|---|
| PROJECT_BRIEF.md | Foundation | Product owner, architect | Draft (generated with AI, awaiting approval) |
| methodology/ (AICD HTML + `sections.json`) | Foundation | Architect | To add: copy the methodology HTML from the AICD repository and generate the section index |
| adr/ADR-0001-stack.md | Foundation | Architect | Draft |
| ARCHITECTURE.md | Foundation | Architect | Draft |
| DATA_MODEL.md | Foundation (entity level) and Phase 1 (detail) | Architect | Draft |
| SECURITY_NOTES.md | Foundation | Architect, reliability and governance | Draft |
| RISK_MAP.md | Foundation | Architect, reliability and governance | Draft |
| ENV_SETUP.md (with permission manifest) | Foundation | Reliability and governance | Draft |
| CONVENTIONS.md | Foundation | Architect | Draft |
| OBSERVABILITY.md | Foundation | Reliability and governance | Draft |
| CI_CD.md, runbooks/ | Foundation | Reliability and governance | Draft |
| ROADMAP.md | Foundation | Product owner, architect | Draft |
| agents/ (CLAUDE.md and role files) | Foundation | Architect | Draft |
| PRD.md (complete function catalog) | Foundation (catalog) and Phase 1 (phase 1 items) | Product owner | Draft |
| API_SPEC.md | Phase 1 | Architect | Draft |
| LLD.md | Phase 1 | Architect | Draft |
| TESTING.md | Phase 1 | Verification lead | Draft |
| criteria/phase-1.md | Phase 1 | Verification lead | Draft |
| QA test plan for phase 1 | Phase 1 | QA agent produces, verification lead verifies | Missing |
| criteria/personas.md | Phase 3 | Verification lead | Missing (not needed before phase 3) |

Not yet present, by design: the phase sets for phases 2 to 5 (written when each phase approaches), the design artifacts (gate G2, `design/`), and the Vibe Engineer style DESIGN document, which for this product is the screen inventory in PRD section 5 until G2.

How to use this folder in Claude Code: read PROJECT_BRIEF.md, then the document your ticket anchors; `agents/CLAUDE.md` is the base instruction file to copy to the repository root as `CLAUDE.md`, and `agents/*.md` are the subagent definitions to copy to `.claude/agents/`.
