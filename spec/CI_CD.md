# CI_CD: Ori Studio

How a validated change reaches a release. Runbooks for deploy, rollback and recovery are in `runbooks/`.

## 1. Pipeline on every pull request

Gates, all required, each proven on a planted defect before it is cited anywhere:

1. `fmt`, `clippy -D warnings`
2. `cargo test` (unit, property, integration), workspace-wide
3. Contract tests
4. Coverage matrix gate (criteria ↔ tests)
5. Modified-test detector (escalation label on any changed or removed existing test)
6. Mutation score threshold
7. `cargo-audit`, `cargo-deny` (advisories, licenses), secret scan (tree and history)
8. Forbidden-action test against `fixtures/new-product`
9. Citation gate (every `AICD §n` resolves)
10. UI: type check, lint, unit tests, build
11. Build of all three platform binaries (cross-compilation or matrix)
12. Significance labeler (runs on merge; sets `significant`)

A pipeline that does not run is a failure: the liveness gate (scheduled) fails loudly if any required workflow has not executed inside its window.

## 2. Merge

Only the merge queue merges. It rebases, re-runs gates 1 to 10 on the rebased head, checks tier approvals, and merges. Tier 0 auto-merge requires the lead's approval and green gates; tier 1 one human; tier 2 two humans or the single-operator profile substitutes, recorded.

## 3. Staging

On merge to `main` with `significant`: build, deploy the headless engine and the UI to staging, run the exhaustive QA run (end-to-end journeys against fixtures, the screenshot matrix, resilience, performance, simulation). Findings become tickets.

## 4. Release

On tag `v*`:
1. Build approved binaries for macOS (universal), Windows, Linux; generate the updater manifest and sign it.
2. Run the forbidden-action test and the full end-to-end suite against the release binaries.
3. Publish: GitHub Release with assets and generated changelog; Homebrew tap; winget manifest; AppImage, deb, rpm, Flatpak; updater manifest.
4. Operations agent verifies each channel installs and starts (smoke), records the result.
5. A human approves the final publish step (GitHub Environment with required reviewer).

## 5. Rollback

A release is rolled back by publishing the previous tag's artifacts to every channel and pointing the updater manifest at it. The runbook is rehearsed on staging before every minor release. There is no data migration to roll back on users' machines: SQLite schema migrations are forward-only and backward compatible with the previous minor.

## 6. Runbooks

- `runbooks/release.md`: the tag-to-channels procedure and verification.
- `runbooks/rollback.md`: previous-tag republish and updater manifest revert.
- `runbooks/recover-engine.md`: what the engine does on restart after a crash and what the operator checks.
- `runbooks/rotate-credentials.md`: rotation order for every secret in ENV_SETUP.
- `runbooks/prove-gate.md`: how a new gate gets its planted defect and proof.
