# Phase 1 backlog: Ori Studio

Operational record. Produced by the lead from `ROADMAP.md` "Phase 1 detail" after batch 0 merged. Not a specification document: `ROADMAP.md` remains the authority on phases, `RISK_MAP.md` on tiers, `criteria/phase-1.md` on what must be proven.

| | |
|---|---|
| Phase | 1, headless engine and CLI |
| Batches | 15, in dependency order, plus batch 0 (specification corrections, merged) |
| Tickets | 79, numbered ORI-T-0001 to ORI-T-0079 |
| Methodology | AICD v0.3. Ticket format AICD appendix A.2. Blocked report format AICD appendix A.5 |
| Profile | Single operator (AICD §38). Every tier 2 change carries the five substitutes, recorded in the PR report |

## How to read this

**Budget.** Every ticket: 3 attempts, 45 minutes wall clock per attempt. Token budget is deliberately absent: AICD §21 requires budgets to be performance-relative to a measured baseline, and no calibration exists until batch 15. Until then the attempt and wall-clock limits are the only budget, and the first coder to exhaust one writes a blocked report rather than continuing.

**Tier.** Taken from `RISK_MAP.md`, never from the ticket's own opinion. Where a ticket's declared scope touches a path `RISK_MAP` tiers 2, the ticket is tier 2 whatever its size and whatever its content. CLAUDE.md states it without qualification: "A change to any of them is tier 2 whatever the ticket says." An earlier version of this line carved out crate skeletons as tier 1; that carve-out was wrong and is removed (ruling R11). Agents may raise a tier and only a human may lower one.

**Declared scope.** The modules the ticket claims in the lock table. Two tickets may run in parallel only when their declared scopes are disjoint. The scope lock map below proves that for every parallel group.

**Preconditions.** Each is stated with its verification. `verified` means I checked it exists in the tree or in a merged PR at the time this backlog was written. `UNVERIFIED` means it does not exist yet and the ticket may not be planned until it does, per AICD §39 ("every precondition a ticket names is verified to exist at approval time").

**Criteria.** The `ORI-P1-nnn` identifiers from `criteria/phase-1.md` the ticket's tests must name. A ticket with no criterion is infrastructure and says so.

## Phase-wide preconditions

| Precondition | State | Blocks |
|---|---|---|
| Git remote on the version control host | **UNVERIFIED** | Every ticket: no PR can be opened |
| Initial commit on `main` | **UNVERIFIED** | Every ticket: no branch can be cut |
| Rust stable toolchain on the operator's machine | **UNVERIFIED** | Every ticket from batch 1 onward |
| Branch protection on `main` | **UNVERIFIED**, operator applies, settings supplied separately | Batch 1 exit |
| Organizational repository (AICD §34, layer 2) | **UNVERIFIED** | ORI-T-0009 only |
| Batch 0 merged | Pending your merge | Every ticket |

Batch 1 does not start until the first three are verified. I check them again at batch 1 planning and stop if any is absent.

## Control points

| Point | What I bring you |
|---|---|
| Before each batch | The batch plan: tickets, declared scope, criteria, tier, order, conflicts |
| Every AICD §12 trigger | The escalation with my recommendation |
| Every tier 2 PR | The diff, the adversarial checklist answered, the rollback line, the five §38 substitutes |
| After each batch | The batch report: merged, blocked, escalated, criteria covered, gates state |
| Batch 1, additionally | Branch protection settings, then a stop until you confirm they are on |

---

## Batch 1: workspace, toolchain, CI skeleton

Nothing else starts until this merges: every later PR depends on these gates being real. Exit: every gate below proven on a planted defect, the smoke ticket merged, `cargo build --workspace` green on macOS, Windows and Linux in CI.

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0001 | **Cargo workspace and the sixteen crate skeletons** | **2** | `Cargo.toml`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs` | LLD §1, §2 | none (infrastructure) |
| ORI-T-0002 | `apps/desktop` scaffold, not built | 1 | `apps/desktop/**` | LLD §1, §3 | none |
| ORI-T-0003 | `rust-toolchain.toml`, `.gitignore`, `scripts/setup-dev.sh` | 1 | `rust-toolchain.toml`, `.gitignore`, `scripts/setup-dev.sh` | ENV_SETUP §1 | none |
| ORI-T-0004 | `scripts/gates.sh`: the local gate set a coder runs before opening a PR | 1 | `scripts/gates.sh` | CONVENTIONS "Rust", CI_CD §1 | none |
| ORI-T-0005 | `methodology/` bundle and `sections.json` generator | 1 | `methodology/**`, `crates/ori-gates/src/sections.rs` | PRD A-09, Z-03 | none |
| ORI-T-0006 | Anchor defect report for the methodology repository | 0 | `ops/methodology-anchor-defects.md` | AICD §39 | none |
| ORI-T-0007 | `spec/agents/CLAUDE.md` as canonical base, root copy derived | 1 | `spec/agents/CLAUDE.md`, `CLAUDE.md` | AICD §32, README | none |
| ORI-T-0008 | `.claude/agents/` from `spec/agents/*.md`, `fable` acceptance verified | 1 | `.claude/agents/**` | AICD §32 | none |
| ORI-T-0009 | `templates/` seeded from AICD appendix A | 1 | `templates/**` | PRD D-10, AICD A.1 to A.5 | none |
| ORI-T-0010 | `policies/significant-modification.md` in the organizational repository | 1 | organizational repo `policies/` | AICD §15, PRD Q-03 | none |
| ORI-T-0011 | **ADR-0002: the single-operator exception** | **2** | `spec/adr/ADR-0002-single-operator.md` | AICD §38 | none |
| ORI-T-0012 | **CI workflow skeleton and the three-platform build** | **2** | `.github/workflows/ci.yml` | CI_CD §1 | none |
| ORI-T-0013 | **Gate 1 (`fmt`, `clippy`) with planted defect and proof** | **2** | `.github/workflows/ci.yml` (gate 1 job), `fixtures/planted/fmt-clippy/**`, `ops/gates/fmt-clippy.md` | CI_CD §1.1, AICD §14 | none |
| ORI-T-0014 | **Gate 2 (`cargo test`) with planted defect and proof** | **2** | `.github/workflows/ci.yml` (gate 2 job), `fixtures/planted/tests/**`, `ops/gates/tests.md` | CI_CD §1.2, AICD §14 | none |
| ORI-T-0015 | **Gate 3 (contract tests) with planted defect and proof** | **2** | `.github/workflows/ci.yml` (gate 3 job), `fixtures/planted/gate-3/**`, `ops/gates/gate-3.md` | CI_CD §1.3, AICD §14 | none |
| ORI-T-0016 | **Gate 7 (audit, deny, secret scan) with planted defect and proof** | **2** | `.github/workflows/ci.yml` (gate 7 job), `deny.toml`, `fixtures/planted/supply-chain/**`, `ops/gates/supply-chain.md` | CI_CD §1.7, AICD §14 | none |
| ORI-T-0017 | **Gate 13 (commit trailers) with planted defect and proof** | **2** | `.github/workflows/ci.yml` (gate 13 job), `fixtures/planted/trailers/**`, `ops/gates/trailers.md` | CI_CD §1.13, PRD G-02 | none |
| ORI-T-0018 | Smoke ticket: a doc comment change travels the full path | 0 | ~~`crates/ori-core/src/lib.rs`~~ **`crates/ori-cli/src/main.rs`** (doc comment only) | AICD §23 G4 | none |

**Correction, recorded after the fact.** ORI-T-0018 landed in `crates/ori-cli/src/main.rs`, not the path this row named, and it entered no claim in `ops/lock-table.md` before it ran. Both are recorded in the lock table and in CR-005 of `ops/calibration.md`. The row is corrected here rather than left to disagree with the commit.

**Correction to the declared scopes above, recorded rather than rewritten.** Every gate ticket in this table named its planted fixtures and its proof record by a descriptive name (`fixtures/planted/fmt-clippy/**`, `ops/gates/tests.md`, and so on). **Not one of those paths exists.** What shipped, in every case, is numbered after the gate: `fixtures/planted/gate-1`, `gate-2`, `gate-7`, `gate-13`, and `ops/gates/gate-1.md` through `gate-13.md`.

The rows for tickets that have already merged are left as they were written, because an operational log is not tidied after the fact ([[R30]]). The rows for tickets that have not started, ORI-T-0015, ORI-T-0047 and ORI-T-0048, are corrected in place to the convention that actually shipped, because a coder declares its scope from this table and would otherwise claim a path that has never existed.

Found by ORI-T-0087's coder, which reported the fixture half. The `ops/gates/` half is wider than it reported and was found on verification: the backlog is wrong about both names for all seven gate tickets. The convention now has a second home in `CLAUDE.md`, which since ORI-T-0087 states "one directory per gate that has been proven" and names them, so the two records can now disagree loudly instead of quietly.


**Gate 9 (citation) is deliberately absent from batch 1.** Your ruling: it stays `Defined`, not `Installed`, until the methodology HTML anchors are fixed and `sections.json` is regenerated. ORI-T-0005 builds the generator and ORI-T-0006 produces the defect list you carry to the methodology repository; the gate itself is ticketed in batch 7 as ORI-T-0047, gated on your fix.

**Tier 0 auto-merge is disabled for the whole of batch 1**, per your ruling: no auto-merge on unproven gates. ORI-T-0018 is tier 0 and I bring it to you to merge by hand; that is the G4 trivial-ticket test done manually, as your first prompt describes.

**Order.** ORI-T-0001 first, alone: every other ticket in the batch needs the workspace. Then three parallel groups with disjoint scope:
- Group A: ORI-T-0002, ORI-T-0003, ORI-T-0004
- Group B: ORI-T-0005, ORI-T-0006, ORI-T-0007, ORI-T-0008, ORI-T-0009
- Group C: ORI-T-0011
Then ORI-T-0012 alone (it creates the file the five gate tickets all edit). Then ORI-T-0013 to ORI-T-0017 **serially, not in parallel**: they all declare `.github/workflows/ci.yml` and the lock table will refuse the second one. Then ORI-T-0018 last, after every gate has a proof.

ORI-T-0010 is held: its precondition, the organizational repository, does not exist. See the open escalations below.

**Preconditions, batch 1.** Remote: UNVERIFIED. Initial commit: UNVERIFIED. Toolchain: UNVERIFIED. `spec/LLD.md` §1 crate list: verified. `spec/CI_CD.md` §1 gate list including new gates 13 and 14: verified in batch 0. `methodology/AICD_Methodology_v0.3.html`: verified present. `spec/agents/*.md` six role files: verified present. AICD appendix A templates: verified present in the methodology HTML.

---

## Batch 2: ori-core

Domain types, state machines, the permission function. No IO, no workspace dependencies (CLAUDE.md load-bearing fact).

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0019 | Domain types and the error enum with `MethodologyRef` | 1 | `crates/ori-core/src/types.rs`, `error.rs` | LLD §2, §4; DATA_MODEL §2 | ORI-P1-033 |
| ORI-T-0020 | **Ticket state machine with property tests** | **2** | `crates/ori-core/src/ticket.rs` | DATA_MODEL §3 Ticket | ORI-P1-005, ORI-P1-006, ORI-P1-007 |
| ORI-T-0021 | **Document and Phase state machines with property tests** | **2** | `crates/ori-core/src/document.rs`, `phase.rs` | DATA_MODEL §3 Document, Phase | ORI-P1-040 |
| ORI-T-0022 | **The permission function** | **2** | `crates/ori-core/src/permission.rs` | SECURITY_NOTES "Authorization model"; AICD §17 | ORI-P1-018, ORI-P1-019 |

Tier 2 on three of four: `RISK_MAP` tiers `crates/ori-core (state machines, permission function)` at 2. Only ORI-T-0019's other types are tier 1.

**Order.** ORI-T-0019 first. Then ORI-T-0020, ORI-T-0021, ORI-T-0022 in parallel: disjoint files, all depending only on 0019.

**Preconditions.** Batch 1 merged and every batch 1 gate proven: UNVERIFIED until it is. `DATA_MODEL.md` Ticket `kind` field: verified in batch 0. `proptest` is a new dependency: **escalation `new_dependency` required before ORI-T-0020 is planned.**

---

## Batch 3: ori-store

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0023 | **Event log: append, hash chain, read range** | **2** | `crates/ori-store/src/event_log.rs` | DATA_MODEL §2 Event, §4; LLD §5 | ORI-P1-028 |
| ORI-T-0024 | **Schema migrations and `ProductDb` open and lock** | **2** | `crates/ori-store/migrations/**`, `crates/ori-store/src/db.rs` | ARCHITECTURE §8; LLD §6 | ORI-P1-036 |
| ORI-T-0025 | Projections and rebuild | 1 | `crates/ori-store/src/projections/**`, `rebuild.rs` | DATA_MODEL §1, §4 | ORI-P1-028 |

**Order.** ORI-T-0023 and ORI-T-0024 in parallel. ORI-T-0025 after both.

**Preconditions.** Batch 2 merged. `rusqlite` and a hashing crate are new dependencies: **escalation `new_dependency` required.**

---

## Batch 4: ori-broker

Injection is no longer here: batch 0 moved `Injector` to `ori-runtime`. The broker issues, the runtime injects.

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0026 | **Identities and the keychain** | **2** | `crates/ori-broker/src/identity.rs`, `keychain.rs` | ENV_SETUP §5; SECURITY_NOTES "Secrets" | ORI-P1-037 |
| ORI-T-0027 | **Issuance, scoping and revocation on session end** | **2** | `crates/ori-broker/src/issuance.rs` | DATA_MODEL §2 CredentialIssuance; AICD §17 | ORI-P1-020 |
| ORI-T-0028 | **Cross-model refusal at identity creation** | **2** | `crates/ori-broker/src/family.rs` | ADR-0001 "model family"; AICD §7 | ORI-P1-035 |
| ORI-T-0029 | **Forbidden-action test harness** | **2** | `crates/ori-broker/src/forbidden.rs` | ENV_SETUP §6; AICD §23 G3 | ORI-P1-017, ORI-P1-018, ORI-P1-019 |

Whole crate is tier 2 (`RISK_MAP`: `crates/ori-broker | 2 | Credentials`).

**Order.** ORI-T-0026 first. Then 0027, 0028, 0029 in parallel.

**Preconditions.** Batch 3 merged. ADR-0001's model family decision: verified in batch 0. `keyring` is a new dependency: **escalation `new_dependency` required.**

---

## Batch 5: ori-runtime

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0030 | Worktrees and session lifecycle | 1 | `crates/ori-runtime/src/session.rs`, `worktree.rs` | LLD §2, §5 | ORI-P1-031 |
| ORI-T-0031 | **Container isolation and the worktree-only downgrade** | **2** | `crates/ori-runtime/src/container.rs` | ADR-0001; SECURITY_NOTES "Failure handling" | ORI-P1-032 |
| ORI-T-0032 | **Credential injection at spawn** | **2** | `crates/ori-runtime/src/injector.rs` | LLD §2 (batch 0); SECURITY_NOTES trust boundary 2 | ORI-P1-020, ORI-P1-037 |
| ORI-T-0033 | ACP client and one headless adapter, non-interactive launch | 1 | `crates/ori-runtime/src/acp.rs`, `headless.rs` | ADR-0001; PRD I-04, P-02 | ORI-P1-039 |
| ORI-T-0034 | Budget meter, transcripts, session recovery | 1 | `crates/ori-runtime/src/budget.rs`, `transcript.rs`, `recovery.rs` | AICD §12; runbooks/recover-engine.md | ORI-P1-009, ORI-P1-031 |

**Order.** ORI-T-0030 first. Then 0031, 0032, 0033 in parallel. ORI-T-0034 last (it needs sessions and the adapter).

**Preconditions.** Batch 4 merged. Docker present on the machine: **verified** (Docker 29.7.2). Podman: absent, and the ADR offers it as the user's choice, so ORI-T-0031 targets Docker and records Podman as untested.

---

## Batch 6: ori-memory

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0035 | Repository indexer (tantivy) and freshness | 1 | `crates/ori-memory/src/indexer.rs`, `freshness.rs` | AICD §25; PRD K-02, K-07 | ORI-P1-026 |
| ORI-T-0036 | Code map (tree-sitter: Rust, TypeScript, Python, Go) | 1 | `crates/ori-memory/src/code_map.rs` | PRD K-03 | ORI-P1-030 |
| ORI-T-0037 | **Sanitization barrier with planted injection tests** | **2** | `crates/ori-memory/src/barrier.rs` | AICD §8; SECURITY_NOTES "Injection" | ORI-P1-021 |
| ORI-T-0038 | **Scope enforcer** | **2** | `crates/ori-memory/src/scope.rs` | ENV_SETUP §5; AICD §25 | ORI-P1-022 |
| ORI-T-0039 | Operational log and the retrieval package | 1 | `crates/ori-memory/src/oplog.rs`, `retrieval.rs` | AICD §25; PRD K-01, K-06 | ORI-P1-022 |
| ORI-T-0040 | Citation checker and drift audit | 1 | `crates/ori-memory/src/citation.rs`, `drift.rs` | PRD Z-03, D-09 | ORI-P1-012, ORI-P1-026, ORI-P1-033 |

**Order.** ORI-T-0035, ORI-T-0036, ORI-T-0037 in parallel. Then ORI-T-0038 and ORI-T-0039 in parallel. Then ORI-T-0040.

**Preconditions.** Batch 5 merged. `sections.json` from ORI-T-0005: verified once batch 1 merges. **The citation checker's rule set is undecided** and is an open escalation below. `tantivy` and `tree-sitter` are new dependencies: **escalation `new_dependency` required.**

---

## Batch 7: ori-gates

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0041 | Gate definitions and the runner trait | 1 | `crates/ori-gates/src/gate.rs`, `runner.rs` | DATA_MODEL §2 Gate; CI_CD §1 | ORI-P1-012 |
| ORI-T-0042 | Coverage matrix gate (gate 4) | 1 | `crates/ori-gates/src/coverage.rs` | TESTING §2; PRD V-03 | ORI-P1-011 |
| ORI-T-0043 | Modified-test detector (gate 5) | 1 | `crates/ori-gates/src/modified_tests.rs` | AICD §14; PRD V-04 | ORI-P1-010 |
| ORI-T-0044 | Significance labeler (gate 12) | 1 | `crates/ori-gates/src/significance.rs` | AICD §15; PRD Q-02, Q-03 | ORI-P1-027 |
| ORI-T-0045 | **Prover: the planted-defect demonstration** | **2** | `crates/ori-gates/src/prover.rs` | AICD §14; runbooks/prove-gate.md | ORI-P1-013 |
| ORI-T-0046 | **Liveness definition** | **2** | `crates/ori-gates/src/liveness.rs` | PRD Z-02, O-06; CI_CD §1 | none in phase 1 (asserted in phase 4) |
| ORI-T-0047 | Citation gate (gate 9), installed only after your HTML fix | 1 | `crates/ori-gates/src/citation_gate.rs`, `.github/workflows/ci.yml` (gate 9 job), `fixtures/planted/gate-9/**`, `ops/gates/gate-9.md` | CI_CD §1.9; PRD Z-03 | ORI-P1-012 |
| ORI-T-0048 | Diagram gate (gate 14) | 1 | `crates/ori-gates/src/diagram.rs`, `.github/workflows/ci.yml` (gate 14 job), `fixtures/planted/gate-14/**`, `ops/gates/gate-14.md` | CI_CD §1.14; CONVENTIONS "Diagrams" | ORI-P1-041 |

**Order.** ORI-T-0041 first. Then 0042 to 0046 in parallel. Then ORI-T-0047 and ORI-T-0048 **serially**: both declare `.github/workflows/ci.yml`.

**Preconditions.** Batch 6 merged. ORI-T-0047's precondition is your corrected methodology HTML and a regenerated `sections.json`: **UNVERIFIED, and it is yours.** The ticket is not planned until you confirm.

---

## Batch 8: ori-orchestrator

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0049 | **Lifecycle: validated transitions, categories, tiers** | **2** | `crates/ori-orchestrator/src/lifecycle.rs` | AICD §11; DATA_MODEL §3 | ORI-P1-005 |
| ORI-T-0050 | Lock table | 1 | `crates/ori-orchestrator/src/lock_table.rs` | AICD §12; PRD F-02 | ORI-P1-008 |
| ORI-T-0051 | Escalation triggers and budgets | 1 | `crates/ori-orchestrator/src/escalation.rs`, `budgets.rs` | AICD §12; PRD F-05, F-06 | ORI-P1-009, ORI-P1-010 |
| ORI-T-0052 | Closing rules | 1 | `crates/ori-orchestrator/src/closing.rs` | AICD §11, §16; PRD T-05 | ORI-P1-006, ORI-P1-007 |
| ORI-T-0053 | **Merge queue, tier 0 and tier 1** | **2** | `crates/ori-orchestrator/src/merge_queue.rs` | CI_CD §2; AICD §13, §38 | none directly; exercised by batch 15 |

**Order.** ORI-T-0049 first. Then 0050, 0051, 0052 in parallel. ORI-T-0053 last.

**Preconditions.** Batch 7 merged, so the merge queue has real gates to re-run. Your ruling on tier 0 auto-merge, which applies from batch 2 onward and is disabled only during batch 1: recorded.

---

## Batch 9: ori-watch

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0054 | Tree watcher and git hooks | 1 | `crates/ori-watch/src/watcher.rs`, `hooks.rs` | ARCHITECTURE §5.2 | ORI-P1-015 |
| ORI-T-0055 | **Attribution, unattributed change, merge block** | **2** | `crates/ori-watch/src/attribution.rs` | PRD Z-01; AICD §39 | ORI-P1-014, ORI-P1-015 |

**Order.** ORI-T-0054 then ORI-T-0055.

**Preconditions.** Batch 8 merged (the merge block needs the merge queue). ORI-P1-014 rewritten to drop notification routing: verified in batch 0.

---

## Batch 10: ori-mcp

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0056 | MCP host toward the operator's servers | 1 | `crates/ori-mcp/src/host.rs` | API_SPEC §3; PRD I-03 | ORI-P1-038 |
| ORI-T-0057 | **MCP server and tool scopes for the phase 1 roles** | **2** | `crates/ori-mcp/src/server.rs`, `scopes.rs` | API_SPEC §3 | ORI-P1-038 |

**Order.** Parallel: disjoint files. ORI-P1-038 needs both, so its test lands with ORI-T-0057.

**Preconditions.** Batch 6 merged (retrieval) and batch 4 merged (identities).

---

## Batch 11: ori-integrations

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0058 | Adapter traits | 1 | `crates/ori-integrations/src/traits.rs` | API_SPEC §4 | none |
| ORI-T-0059 | **Version control host reference adapter: installation tokens, webhook verification, merge exposure** | **2** | `crates/ori-integrations/src/reference/vcs/**` | API_SPEC §4; SECURITY_NOTES trust boundary 3 | ORI-P1-017 |

**Order.** ORI-T-0058 then ORI-T-0059.

**Preconditions.** Batch 4 merged. A version control host application registered and its app id and private key in the keychain: **UNVERIFIED, and it is yours.**

---

## Batch 12: ori-flows

Batch 0 added `ori-flows`, L-03 and M-05 to phase 1, so this batch now carries the launch success path and the M2 as-built generation.

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0060 | `init`: specification skeleton and the document registry | 1 | `crates/ori-flows/src/init.rs` | AICD §23 G1; PRD D-01 | ORI-P1-001 |
| ORI-T-0061 | Readiness computation | 1 | `crates/ori-flows/src/readiness.rs` | PRD L-01 | ORI-P1-002, ORI-P1-026 |
| ORI-T-0062 | **`launch`: refusal path and success path (L-03)** | **2** | `crates/ori-flows/src/launch.rs` | AICD §23 G4; PRD L-03 | ORI-P1-003, ORI-P1-004 |
| ORI-T-0063 | Document generation and approval | 1 | `crates/ori-flows/src/documents.rs` | API_SPEC §1 flows; PRD D-04 | ORI-P1-016, ORI-P1-040 |
| ORI-T-0064 | `migrate` M0: inventory, env report, unattributed detection on | 1 | `crates/ori-flows/src/migrate/m0.rs` | AICD §24.1; PRD M-02, M-04 | ORI-P1-024 |
| ORI-T-0065 | `migrate` M2: as-built documents and divergence register (M-05, M-06) | 1 | `crates/ori-flows/src/migrate/m2.rs` | AICD §24.3; PRD M-05, M-06 | ORI-P1-025 |

**Order.** ORI-T-0060 first. Then ORI-T-0061 and ORI-T-0064 in parallel. Then ORI-T-0062, ORI-T-0063, ORI-T-0065 in parallel.

**Preconditions.** Batches 2, 3, 6, 7, 8 merged. `templates/` from ORI-T-0009: verified once batch 1 merges. ORI-T-0064 needs `fixtures/migrated-with-drift`, which batch 15 builds: **ordering conflict, see open escalations.**

---

## Batch 13: ori-rpc and ori-engine

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0066 | Method registry, validation, dispatch | 1 | `crates/ori-rpc/src/registry.rs`, `dispatch.rs` | API_SPEC §1 | ORI-P1-023 |
| ORI-T-0067 | Generated JSON Schema per method | 1 | `crates/ori-rpc/src/schema.rs`, `build.rs` | API_SPEC §5 (batch 0) | ORI-P1-034 |
| ORI-T-0068 | **Transports and local client authentication** | **2** | `crates/ori-rpc/src/transport/**`, `auth.rs` | SECURITY_NOTES trust boundary 1 | ORI-P1-036 |
| ORI-T-0069 | Event subscriptions | 1 | `crates/ori-rpc/src/subscriptions.rs` | API_SPEC §2 | none |
| ORI-T-0070 | `ori-engine` composition root | 1 | `crates/ori-engine/src/**` | LLD §2; ARCHITECTURE §1 | ORI-P1-036 |

**Order.** ORI-T-0066 first. Then 0067, 0068, 0069 in parallel. ORI-T-0070 last.

**Preconditions.** Batches 8, 9, 10, 11, 12 merged.

---

## Batch 14: ori-cli

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0071 | `ori` commands mirroring the Client API | 1 | `crates/ori-cli/src/**` | API_SPEC §1; PRD ROADMAP phase 1 | ORI-P1-001 to ORI-P1-004, ORI-P1-024 |
| ORI-T-0072 | `--json` output validated against the generated schema | 1 | `crates/ori-cli/src/json.rs` | API_SPEC §5 | ORI-P1-034 |

**Order.** ORI-T-0071 then ORI-T-0072.

**Preconditions.** Batch 13 merged.

---

## Batch 15: fixtures, end-to-end suite, baselines

| Ticket | Title | Tier | Declared scope | Spec anchor | Criteria |
|---|---|---|---|---|---|
| ORI-T-0073 | `fixtures/new-product` | 1 | `fixtures/new-product/**` | TESTING §5 (batch 0) | none |
| ORI-T-0074 | `fixtures/migrated-with-drift` (documents that disagree with code, an inert workflow, a tracked env file with fake keys) | 1 | `fixtures/migrated-with-drift/**` | TESTING §5 | none |
| ORI-T-0075 | `fixtures/inert-gate` (workflow present, never fires; for the liveness gate) | 1 | `fixtures/inert-gate/**` | TESTING §5; PRD Z-02 | none |
| ORI-T-0076 | `fixtures/looping-agent` | 1 | `fixtures/looping-agent/**` | TESTING §5 | ORI-P1-009 |
| ORI-T-0077 | End-to-end suite | 1 | `tests/e2e/**` | TESTING §1 | every criterion not covered above |
| ORI-T-0078 | Performance baselines | 1 | `benches/**`, `ops/baselines.md` | OBSERVABILITY §2; TESTING §3 | ORI-P1-029, ORI-P1-030 |
| ORI-T-0079 | Budget calibration on the first model | 1 | `ops/calibration.md` | AICD §21, §30; PRD C-01 | none |

**Order.** ORI-T-0073 to ORI-T-0076 in parallel. Then ORI-T-0077. Then ORI-T-0078 and ORI-T-0079 in parallel.

**Preconditions.** Batch 14 merged, except the fixture tickets, which must move earlier: see open escalations.

---

## Scope lock map

No module appears against two tickets that I schedule in parallel. The three places where the lock table would refuse a parallel start, and which I therefore serialise:

| Contended module | Tickets | Resolution |
|---|---|---|
| `.github/workflows/ci.yml` | ORI-T-0012, 0013, 0014, 0015, 0016, 0017, 0047, 0048 | Serial, in that order. ORI-T-0012 creates the file; each gate ticket adds one job |
| `crates/ori-orchestrator/src/merge_queue.rs` | ORI-T-0053 only | Sole claimant, and the only caller of `VcsHost::merge` |
| `spec/**` | ORI-T-0007, ORI-T-0011 | Different files, parallel is safe; both go through the documentation role |

## Criteria coverage

All 41 criteria of `criteria/phase-1.md` are claimed by at least one ticket. Concentrations worth noting: batch 4 carries the three security refusals (017, 018, 019), batch 6 carries both memory security criteria (021, 022), batch 12 carries the whole flows set (001 to 004, 016, 024, 025, 026, 040).

## Open escalations, for your review window

| # | Trigger | Question |
|---|---|---|
| 1 | `precondition_missing` | The organizational repository (AICD §34) does not exist. ORI-T-0010 seeds `policies/significant-modification.md` into it. Do I create it as a sibling repository in batch 1, or does ORI-T-0010 move to a later batch? |
| 2 | `precondition_missing` | ORI-T-0064 (`migrate` M0) and ORI-T-0077 (end-to-end) need the fixtures that batch 15 builds. My recommendation: move ORI-T-0073 to ORI-T-0076 from batch 15 to batch 1, where they are just directories of files and block nothing. Batch 15 then holds only the suite, the baselines and the calibration |
| 3 | `new_dependency` | Five external crates are unavoidable in phase 1: `proptest` (batch 2), `rusqlite` (batch 3), `keyring` (batch 4), `tantivy` and `tree-sitter` (batch 6). I recommend approving them as one decision at batch 2 planning rather than four separate interruptions |
| 4 | `spec_conflict` | The citation gate's scope is undefined. `spec/design/Ori Studio.html` contains twelve fabricated methodology subsection references (§11.7, §12.3, §17.3, §17.4, §17.9, §23.1, §23.4, §23.5, §26.1, §27.2, §39.1, §39.3) inside display copy. Does gate 9 check markdown only, or everything under `spec/`? |
| 5 | `contract_change` | `spec/design/DESIGN.md` binds DSN-001 to `design/ori-studio-mockup-v1.html`; the file present is `spec/design/Ori Studio.html`. Rename the file, or repoint the document? Batch 0 records the mismatch in the registry rather than resolving it |
| 6 | `spec_conflict` | **Model family storage.** Your ruling stores the family on the provider binding. `ProviderBinding` is keyed per (product, provider) and carries no model, so a lead and a coder on the same provider but different models resolve to the same family and would be refused wrongly. My recommendation: the runtime adapter still declares the family, but it is resolved at `broker.identity.create` and recorded on `AgentIdentity`, which already carries `model`. A second point: the refusal lives in `ori-broker`, `RuntimeCaps` lives in `ori-runtime`, and LLD has ori-runtime depending on ori-broker, so the broker cannot call the runtime. The family should be a value type in `ori-core` that the engine passes to the broker. Both are one-line corrections to ADR-0001 and one field on `AgentIdentity`, and I have not made them |
| 7 | `spec_conflict` | **API schema emission path.** Batch 0 defines the generated JSON Schema per method but deliberately names no directory, because `target/` appears in no document and LLD §1's layout registers no build output. Either register the build output in LLD §1, or leave the path to ORI-T-0067 in code. I have taken the second reading in batch 0 |
| 8 | `contract_change` | **`Ticket.kind` has no setter.** `tickets.create` takes `kind?` and `aicd_ticket_create` proposes it, but nothing can correct it afterwards, while `tickets.close` now refuses a defect close without an accepted criterion. A ticket filed with the wrong kind is stuck, and an agent that could flip `defect` to `chore` would escape the criterion requirement entirely. My recommendation: add `tickets.setKind({id, kind})`, human actor only, mirroring the upgrade-only logic already applied to category. That is an API_SPEC change, so it is yours |
| 9 | `spec_conflict` | **M-05's fourteen as-built documents now split across phases.** Moving M-05 to phase 1 means phase 1 claims "the fourteen documents in order", but two of the fourteen are separate PRD rows still in phase 3: M-07 (inherited ADRs, AICD §24.3 document 11) and M-08 (risk map, document 13). My recommendation: narrow M-05's phase 1 scope to the twelve it can complete and let M-07 and M-08 bring the other two in phase 3, which is what ORI-P1-025 actually asserts. The alternative is pulling M-07 and M-08 into phase 1, which widens phase 1 |

## Pre-existing findings batch 0 does not touch

Recorded so they are not lost. None is caused by batch 0 and none blocks batch 1.

| Finding | Where |
|---|---|
| Q-02's "runs" half is delivered in no phase: PRD says "1 (label), 4 (runs)", ROADMAP phase 1 delivers the label and phase 4 lists no Q-02 | PRD §4.10, ROADMAP phases 1 and 4 |
| Journeys J-08, J-09 and J-10 have no end-to-end coverage: TESTING scopes the suite to J-01 to J-07 while PRD defines ten and says journeys are what the suite exercises | TESTING §1, PRD §5 |
| The methodology bundle has a version-bearing filename, `AICD_Methodology_v0.3.html`. Batch 0 points CLAUDE.md at the real name. Whether the bundle should ship under a version-stable name is worth deciding before PRD A-09's upgrade flow is built | CLAUDE.md, PRD A-09 |
| `spec/agents/CLAUDE.md` does not exist; only the root copy does. Batch 0 records this honestly in the registry and ORI-T-0007 creates it | README, ORI-T-0007 |
