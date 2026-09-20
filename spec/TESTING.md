# TESTING: Ori Studio

Test strategy per level, thresholds, and how the coverage matrix works for this product. Acceptance criteria live in `criteria/` per phase (phase 1 in `criteria/phase-1.md`). Humans write criteria; agents write tests; humans verify the plan (AICD §14).

## 1. Levels and what each proves

| Level | Tool | Proves | Where |
|---|---|---|---|
| Unit | `cargo test` | State machines, invariants, sanitization, permission function, parsers | Beside code |
| Property-based | `proptest` | Invariants hold for generated event sequences (ticket, document, phase machines never reach an invalid state; hash chain unbroken; lock table never overlaps) | Beside code |
| Integration | `cargo test` in `crates/*/tests` | Crates together against a temp SQLite file and a fixture repository | Per crate |
| End-to-end | Engine driven through JSON-RPC against `fixtures/` products; UI through Playwright against the Tauri dev build | Journeys J-01 to J-07 | `tests/e2e/` |
| Contract | Schema tests for the Client API, the MCP tools and each adapter trait; fixture recordings of real integration responses | Interfaces do not drift | `tests/contract/` |
| Security | `cargo-audit`, `cargo-deny`, secret scan on tree and history, the forbidden-action test, planted injection strings through the barrier, path escape attempts through every file RPC | Threat model controls hold | CI, every PR |
| Resilience | Kill the engine mid-session, remove the container runtime, cut the provider, corrupt the index | Recovery rules in SECURITY_NOTES hold | `tests/resilience/` |
| Performance | Criterion benchmarks: retrieval package under load, event throughput, dashboard update latency | Non-functional requirements of PRD §7 | `benches/` |
| Mutation | `cargo-mutants` | The suite detects introduced faults | CI, threshold only rises |
| Simulation | Persona-driven runs of Ori Studio itself: a solo operator migrating a fixture product, an operator ignoring escalations, an agent looping | Behaviors no scripted test would find | Staging, on significant modification |
| Screenshot matrix | Playwright on macOS, Windows, Linux webviews | Rendering parity | Staging |

## 2. The coverage matrix

- Every criterion has an identifier `ORI-<phase>-<nnn>`.
- Every test name embeds the identifier(s) it covers. The coverage matrix gate parses test names and criteria files, fails if a criterion has no test or a test names no criterion, and posts the matrix on the pull request.
- Proposed criteria (from the QA agent) do not count until accepted.

## 3. Thresholds

Set from baselines at the end of phase 1 by the calibration procedure, recorded in `ops/calibration.md`, and only ever raised: coverage matrix completeness (target 100 percent of accepted criteria), mutation score, performance budgets per benchmark.

## 4. Gate proving

Every gate in CI_CD ships with a planted defect under `fixtures/planted/` and a proof recorded in `ops/gates/`. A gate is not cited as protection in any document until its proof exists (AICD §14).

## 5. Ori Studio tests itself

Because the product is a methodology engine, the fixtures are AICD products: `fixtures/new-product` (empty, for J-01), `fixtures/migrated-with-drift` (documents that disagree with code, an inert workflow, a tracked env file with fake keys, for J-02 and Z-01), `fixtures/inert-gate` (a repository whose workflow is present but never fires, for the liveness gate and the "present but reporting nothing" defect class), `fixtures/looping-agent` (a headless adapter that never converges, for budgets and blocked reports). The inert workflow in `migrated-with-drift` is there for migration to detect it; `inert-gate` is there for the liveness gate to fail on it. The end-to-end suite is the methodology's own exit criteria executed against these.
