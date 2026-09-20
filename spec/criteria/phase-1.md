# Acceptance criteria: Phase 1 (headless engine and CLI)

Format per AICD appendix A.1. Identifier `ORI-P1-nnn`. Types: functional (F), security (S), performance (P), resilience (R). Tier per RISK_MAP. Each criterion must map to at least one test whose name contains the identifier. Criteria are written by the verification lead; the QA agent produces the test plan; the verification lead verifies the plan before build.

| ID | Type | Tier | Precondition | Action | Expected result |
|---|---|---|---|---|---|
| ORI-P1-001 | F | 1 | Empty directory | `ori init --name X` | `spec/` skeleton with every foundation document in state Missing, `ops/` created, product database created, an event `product.created` with a human actor |
| ORI-P1-002 | F | 1 | Product with PROJECT_BRIEF Draft, nothing else | `ori readiness` | `ready: false`; missing list names PROJECT_BRIEF (not approved) and every other foundation document (missing); each entry carries the owning seat |
| ORI-P1-003 | F | 2 | Product with all foundation documents Approved except one | `ori launch` | Refused with reason citing AICD §23 and the one missing document; no identity created; no event other than `launch.refused` |
| ORI-P1-004 | F | 2 | Product fully ready | `ori launch` | Identities created for phase 1 roles; forbidden-action test executed and every attempt refused; gates proven; queue seeded from `criteria/phase-1.md`; the trivial tier 0 ticket of G4 merged without human action; `launch.completed` event |
| ORI-P1-005 | F | 1 | Ticket filed by an agent identity as Auto | Lead upgrades it to Decisional | Category is Decisional; an agent attempting to set it back to Auto receives `E_UPGRADE_ONLY`; a human can |
| ORI-P1-006 | F | 2 | Ticket Merged, no spec update event | `tickets.close` | Refused, reason cites AICD §11 closing rule |
| ORI-P1-007 | F | 2 | Defect ticket Merged with spec update but no accepted criterion referencing it | `tickets.close` | Refused, reason cites AICD §16 |
| ORI-P1-008 | F | 1 | Two validated tickets with overlapping declared scope | Lead assigns both | Second returns `E_SCOPE_LOCKED`; lock table shows one entry per module; after the first closes, the second starts |
| ORI-P1-009 | F | 1 | Coder session with budget attempts=2 | Headless adapter fails twice | Session ends Blocked; a blocked report record exists with the two attempts; credentials revoked; ticket returns to Queued on re-plan |
| ORI-P1-010 | F | 2 | PR whose diff modifies an existing test | Gate run | Modified-test gate fails; escalation opened with trigger `test_modified`; PR cannot reach InReview |
| ORI-P1-011 | F | 1 | Criterion with no test named after it | Coverage matrix gate | Gate fails and lists the criterion; a test naming no criterion is listed as unmapped |
| ORI-P1-012 | F | 2 | Gate defined, no proof | Any document or PR cites it as protection | Citation gate flags the gate as unproven; `gates.list` shows Defined, not Installed |
| ORI-P1-013 | F | 2 | Gate installed; its planted defect present | `gates.prove` | Passes clean, fails dirty, proof stored with evidence refs; gate state Installed |
| ORI-P1-014 | F | 2 | Product launched; operator edits a source file outside `spec/` on disk with no active session | Watcher tick | Within two seconds: `change.unattributed` event, incident ticket with the diff, notification routed as interrupt, merges from the branch blocked |
| ORI-P1-015 | F | 2 | Same as 014 but the change is inside an active coder session's worktree | Watcher tick | Attributed to the session; no incident |
| ORI-P1-016 | F | 1 | Operator edits a document under `spec/` through the engine | `spec.write` | A specification PR opened; citation check run; document state Draft; no unattributed change |
| ORI-P1-017 | S | 2 | Any agent identity | Attempt `git push origin main` from its session | Refused by the vcs token scope; refusal event recorded with the identity |
| ORI-P1-018 | S | 2 | qa identity | Attempt to write any file in the repository | Refused (no repository credential issued to qa); event recorded |
| ORI-P1-019 | S | 2 | Any agent session | Attempt to read the OS keychain or a `.env*` file | Refused; event recorded; the forbidden-action test reports it |
| ORI-P1-020 | S | 2 | Any session ended (any outcome) | Inspect issuances | Every issuance for the session is revoked with a timestamp before the process is terminated |
| ORI-P1-021 | S | 2 | Report submitted by an agent containing a planted injection string in a free-text field | `aicd_report` | Stored record has the field length-capped and the string neutralized; provenance `untrusted`; raw content only in an evidence blob, not in the index |
| ORI-P1-022 | S | 2 | Coder identity | `aicd_context` for a ticket touching module M | Package contains only canonical, organizational, operational records for M, and the code map of M; no evidence blobs; every item carries provenance and verification date |
| ORI-P1-023 | S | 1 | Any file RPC | Path with `..` or a symlink escaping the product root | Refused; event recorded |
| ORI-P1-024 | F | 1 | `fixtures/migrated-with-drift` | `ori migrate` | M0 inventory produced; the inert workflow detected and listed; the tracked env file reported by key names and shape only, no values in any output; unattributed-change detection on |
| ORI-P1-025 | F | 1 | Migration M2 running | Documentation agent produces as-built documents | Each is a Document in Draft; a divergence register exists with one entry per disagreement; no entry resolved without a human decision event |
| ORI-P1-026 | F | 1 | Product with stale document (drift audit found divergence) | `ori readiness` | Not ready; the stale document is listed with the divergence |
| ORI-P1-027 | F | 1 | Ticket Merged | Significance labeler | `significant` set only if declared scope or category matches the significant list; copy-only change is not significant |
| ORI-P1-028 | F | 1 | Any mutating RPC | Inspect the event log | One event with actor, ticket, payload, and a hash chained to the previous; `products.rebuild` reproduces every projection identically |
| ORI-P1-029 | P | 1 | 10,000 events in the log | `products.rebuild` | Completes within the recorded baseline; dashboard projection query under the baseline |
| ORI-P1-030 | P | 1 | Repository of 50,000 lines, 4 languages | `memory.context` | Package assembled under the recorded baseline; package size under the configured cap |
| ORI-P1-031 | R | 2 | Engine killed while a coder session is Running | Engine restart | Session marked Killed, credentials revoked, locks released, ticket Queued, event `session.recovered`; no orphan process |
| ORI-P1-032 | R | 1 | Container runtime absent | Launch a coder | Runs worktree-only; downgrade event and banner; a tier 2 ticket refuses to start in this mode |
| ORI-P1-033 | F | 1 | Any refusal by the engine | Inspect the error | Carries `MethodologyRef` with a section that resolves in the methodology index |
| ORI-P1-034 | F | 1 | CLI | Any command with `--json` | Output is valid JSON matching the API schema for that method |
| ORI-P1-035 | F | 1 | Lead identity on the same model as its coder | `broker.identity.create` for the lead | Refused, reason cites AICD §7 |
| ORI-P1-036 | F | 1 | Two projects registered | Open both through the CLI in two daemons | Two databases, two sockets, two lock files; a ticket in one is invisible to the other; `memory.search` in one never returns the other's records |
| ORI-P1-037 | F | 1 | Provider key set at application level; project B has an override | Spawn a coder in A and in B | A receives the application key, B receives its override; neither key appears in any event or log |
| ORI-P1-038 | F | 2 | MCP server added to project A, exposed to qa only | Coder in A and qa in B request it | Coder in A refused (role); qa in B refused (project); qa in A succeeds |
| ORI-P1-039 | F | 2 | Any agent session | Inspect the runtime launch configuration and the session transcript | The runtime was launched in its non-interactive mode; no permission prompt appears in the transcript; a runtime that prompts is recorded as a launch defect |
| ORI-P1-040 | F | 1 | Any document in UnderReview | Owning seat calls `flows.documents.approve` | Single call; state Approved; one `document.approved` event with seat and human actor; no other step required |
| ORI-P1-041 | F | 1 | Any generated document containing a diagram | Inspect | The diagram is a Mermaid fenced block; no ASCII diagram and no image of a diagram anywhere under `spec/` (gate) |

Proposed criteria from the QA agent are appended below this line in state `proposed` and move into the table only when accepted by the verification lead.
