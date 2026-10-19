# Handoff: continuing the lead session of 2026-10-19 elsewhere

Written by the lead on 2026-10-19, at the operator's request, so that the work can continue in another session or container. The container this session ran in is ephemeral; everything needed is in this repository. The record of every decision is `ops/lock-table.md`, whose last sections give the state below in full.

## Where things stand

| | |
|---|---|
| Session branch | `claude/sweet-maxwell-jkaiie`, everything committed and pushed |
| Pull request | [PR 78](https://github.com/rachidsahane/ORI-STUDIO/pull/78), open, **not merged**, base `main` at `7bed207`. It is tier 2, so only the operator merges it (CLAUDE.md rule 2, ADR-0002). A squash merge is recommended (its description says why) |
| CI | **No run can start**: the GitHub account is locked over billing ("The job was not started because your account is locked due to a billing issue"). The last run that executed, on `64fef94`, was green on all 17 checks. `d9554ab` and later have not been checked by CI |
| Claims held | 20, 112 (ORI-T-0163), 117 (ORI-T-0187), 121 (ORI-T-0194). Next free identifier: **ORI-T-0195** |
| Phase 1 | Batch 10 (`ori-mcp`) in progress; batch 12 (`ori-flows`) planned, not adopted; batches 13 to 15 (except the fixtures) not started |

## What to do next, in order

1. **Restore CI** (operator: the account's billing), then re-run CI on the branch head. Treat any failure by the PR rules: fix and push, never skip a test.
2. **Review ORI-T-0194** (batch 10 slice H1, the registry of the operator's MCP servers; `crates/ori-mcp/src/host.rs`, `host/registry.rs`). It was integrated at the operator's request **before** its independent review, which is owed before merge (ruling R39). The design is `ops/tickets/ORI-T-0194-mcp-registry.md`. The review the lead was running had five lenses, each a separate reviewer, then two skeptics per blocking or should-fix finding who try to refute it:
   - **conformance**: the code against every decision of the package;
   - **security**: any path where an exposure survives that should not (a removed or blocked server, an agent or system actor, a foreign product, a wrong credential reference, a payload shape the two-pass reader lets through, a panic reachable from a log row);
   - **tests**: each of the package's 21 tests present and pinning its answer; at least five mutations re-run;
   - **docs**: every doc sentence true against the code and the cited spec; nothing unbuilt in the present tense; the Mermaid diagram parses;
   - **contract**: usable by S6, H2 and the engine; `Registry: Send`; nothing public that should not be.

   Reviewers read only. A probe or a mutation goes in its own `git archive` copy with **its own `CARGO_TARGET_DIR`**: a shared target let one copy's binary run in place of another's (recorded in the lock table). The coder's report (35 new tests, 12 mutations killed, deviations) is summarised in the lock table section "paused at the operator's request". Repairs follow R44 (the lead designs, a coder types), then claim 121 is released.
3. **Batch 12 (`ori-flows`)**: `ops/batch-12-plan-draft.md` is the planner's draft, and `ops/batch-12-plan-review.md` holds four reviewers' findings on it. The lead's calls on that review are below, under "The lead's calls on the batch 12 review". Write the adopted plan as `ops/batch-12-plan.md`, put its questions to the operator, then design slice **R1** (the document registry and the shared ports), the only slice that needs no operator answer.
4. **With the operator** (PR 78's description, "Escalations", lists them all): ORI-T-0187 and ORI-T-0163 below; ORI-T-0165; ORI-T-0190 and ORI-T-0191 (security); ORI-T-0193; ORI-T-0194's lockout trade and the registry file's tier; ORI-T-0131, ORI-T-0135, ORI-T-0154, ORI-T-0164.

## The files in this directory

The work below existed only on local branches and in the lead's scratchpad, so it is kept here as patches. Nothing here is applied to the code.

| File | What it is |
|---|---|
| `ORI-T-0187-branch.patch` | Slice S4 (`aicd_report`), the coder's three commits (`a1f545c`, `c509975`, `38f41ae`) on base `4121562`, from `git format-patch`. Reviewed, nothing blocking; repaired once |
| `ORI-T-0187-repair-2.patch` | The lead's proved repair 2 of S4, a unified diff of `crates/ori-mcp/src/tools/report.rs` against `38f41ae`. The safety classifier refused the coder's command that applied it ("Code from External"); the operator decides |
| `ORI-T-0187-repair-2-prompt.md` | The coder prompt for that repair, with the exact commit message. Its paths name the old scratchpad |
| `ORI-T-0163-branch.patch` | The tree watcher looking at its own root, three commits (`f454a3e`, `fd5ee06`, `d31679b`) on base `10cd14c`. Reviewed, nothing blocking; repaired once. Must land before anything installs the hooks or starts the watcher (ORI-T-0055) |
| `ORI-T-0163-repair-2.patch` | The lead's proved repair 2, a unified diff of `crates/ori-watch/src/watcher.rs` against `d31679b`; refused by the classifier the same way |
| `ORI-T-0163-repair-2-prompt.md` | Its coder prompt and commit message |
| `gate13.sh` | The lead's helper that runs gate 13 (commit trailers) over a range: `bash gate13.sh <repo> <base> <head>`. It calls `scripts/gates.sh --commit-trailers` with a simulated merge-group payload |

The lead verified, before writing these files, that each repair-2 patch reproduces byte for byte the proved file of its repair from the branch tip it names. To rebuild a branch: `git checkout -b <name> <base>`, then `git am <branch.patch>`; to apply a repair on top: `git apply <repair-2.patch>`, then commit with the message in its prompt. The operator's three courses for each repair 2 are in PR 78's description: apply it yourself or authorize it, integrate at the tip with the findings as a follow-up, or leave it waiting.

Two other local branches had one commit each not on the session branch (`40ae004` on ORI-T-0040's branch, `6822a80` on ORI-T-0186's). Both are earlier forms of commits integrated as `0247cf1` and `a072c5d`, so they are not kept.

## The lead's calls on the batch 12 review

These are the decisions the lead took on `ops/batch-12-plan-review.md` before the pause. They override the draft where they differ.

- An item a reviewer marked "adopt" is adopted; "adopt with change" is adopted with the reviewer's proposed fix; anything a reviewer called the operator's call becomes a question to the operator, with the reviewer's recommendation, and the decision that depends on it waits. Where two fixes conflict, the narrower one is taken and the choice is stated.
- **D12, the owner seat**: never written into an event payload. It comes from a code table by document kind (the FOUNDATION constant and its as-built and phase counterparts), an interim reading recorded as such; Q9 (seat sets, joint verification) goes to the operator. Payloads record kind, path and set, never the owner.
- **D2, the payloads**: fixed now only for the kinds batch 12 appends; `document.drifted` and `document.submitted` are deferred (the second waits on Q7). `document.changes_requested` requires a human actor and caps the instruction's length. **The blocking finding is adopted**: a `repo_remote` with a password in its userinfo is refused before anything is appended (AICD §27, never echoed); scp-style and password-free forms are accepted; `repo_path` is recorded canonical and absolute; the fold refuses a `product.created` that breaks either rule. Every payload is read in two passes (a `serde_json::Value` shape check, then the struct with `deny_unknown_fields`) and written with `serde_json::to_string` of the same struct, never `json!`, as ORI-T-0194 does.
- **D1, the fold**: a refused event locks the product's document registry, the same trade as R45 point 3 and ORI-T-0194's lockout; it goes to the operator with H1's. The fold, not the writer, is the control. The FOUNDATION rows (kind, path) are fixed in the plan, because each path enters an immutable event.
- **D13, seats**: the holder check lives in `registry::record`, so every approval path makes it (**the second blocking finding**), not only `approve`. The seat list in `product.created` is validated: each of the four seats exactly once, each holder a human identity. The "engine fills the seats" clause is removed.
- **D5**: `ports.rs` holds `IdSource`, `Clock` and `PortError`, with `PortError` carrying `Refused` and `Denied` (each with a `MethodologyRef`), `Invalid` and `Unavailable`, as ori-mcp's does.
- **D4**: the `ori-flows -> ori-memory` edge is adopted for in-memory entry points only; anything in ori-memory that reads the working tree (the citation checker's file reads) is reached through a port. The reason given for excluding `ori-gates` is corrected.
- **D6**: the "alternative" ruling is removed; the P-W / P-G overlap is settled as the reviewer proposes.
- **D8**: adopted with the step-order fix; L2 leaves batch 12 under the backlog's precondition rule; no security-escalation clause.
- **D10**: adopted with the reviewers' fixes; the M-04 secret-scan narrowing, the Z-04 archive half and the reading of ARCHITECTURE:129 are operator questions.
- **D11**: M2b moves to its own file, `crates/ori-flows/src/migrate/divergence.rs`, so it depends only on R1; M2a waits on Q2, and on how Q3 records "M2 running".
- **D14**: the "identities" reading of ROADMAP:14 is an operator question; the rule 10 reason is fixed; the three gaps the reviewer names (a `repo_path` that is not a usable directory, who holds the `ProductDb` after init, where `aicd_version` comes from) are closed with the reviewer's proposals, or become questions where there is none.
- **Tiers**: the lead raises E0 (the env report), L1 and L2 (launch) to tier 2 itself at claim time, as for ORI-T-0194, so they do not wait on PD-1. PD-1 (the RISK_MAP rows) stays a prerequisite for the operator, with an `adr_area` escalation and the SECURITY_NOTES and ADR-0002 tier tables.
- **The phase-set gap** (nothing in phase 1 registers phase-set documents, so readiness can never be true) is an operator question with options and a recommendation.
- **M0a** needs a way to create a migrated product (P-I, `products.import`, origin `migrated`): a prerequisite the lead allocates before M0a, not an operator question unless the specification leaves it open.
- Every factual error the reviewers found is corrected (among them the stale claims row, `7bed207`, the scope of SECURITY_NOTES:51, P-V's tier 2, P-S's owner, ROADMAP:14, the fixture split's count, the E-0003 line, LLD:74 against AICD §11).
- **What can start**: R1 now; after R1, R2, RD, DOC (without `submit`, and `spec.write` only on Missing or Draft under Q8's option c), E0 and M2b; L1 after RD and R1; M0a once P-I is allocated and built; L2, M2a and M0b wait (on Q4 and Q5, on Q2 and Q3, and on ORI-T-0055 and ORI-T-0070).

## How the lead worked (ruling R44 and the session's practice)

- The lead (Opus 5.5) designs every ticket in a package (`ops/tickets/` holds the recent ones); a coder on Sonnet 5.5 types it, or Haiku 5.5 when every line is already the lead's. Packages are fact-checked by independent agents before dispatch, and each fact-check of this session found errors of the lead's.
- Claims are recorded in `ops/lock-table.md` before dispatch. Each coder works in its own worktree and branch, commits locally, and never pushes; the lead integrates by cherry-pick onto the session branch after the review, with fmt, clippy (also `--target x86_64-pc-windows-gnu`), strict rustdoc, the tests and gate 13.
- Commits carry exactly one `Ticket:` trailer and one `Spec:` trailer (no `spec/` prefix); types are only `feat`, `fix`, `docs`, `ci`, `chore`.
- No existing test changes without a `test_modified` escalation (CLAUDE.md rule 3). Never kill a process by pattern. Never `rm -rf` a variable path.
