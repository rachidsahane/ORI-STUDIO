# RISK_MAP: Ori Studio

Every module of the repository with its tier and the reason. Maintained with the code; the coverage of this map is a readiness item.

| Path | Tier | Reason |
|---|---|---|
| crates/ori-core (state machines, permission function) | 2 | Control structure |
| crates/ori-core (other types) | 1 | |
| crates/ori-store/src/event_log.rs, migrations/ | 2 | Audit trail, data integrity |
| crates/ori-store (projections) | 1 | Rebuildable |
| crates/ori-memory/src/barrier.rs, scope.rs | 2 | Injection, boundaries |
| crates/ori-memory (indexer, code map, retrieval, drift, citation) | 1 | |
| crates/ori-broker | 2 | Credentials |
| crates/ori-runtime/src/container.rs, injector.rs | 2 | Isolation |
| crates/ori-runtime (acp, headless, worktree, budget, transcript) | 1 | |
| crates/ori-gates/src/prover.rs, liveness.rs | 2 | Gate integrity |
| crates/ori-gates (runners) | 1 | |
| crates/ori-orchestrator/src/merge_queue.rs, lifecycle.rs | 2 | Merge path, tiers |
| crates/ori-orchestrator (lock table, escalation, budgets) | 1 | |
| crates/ori-flows | 1 | Approvals are events checked in core |
| crates/ori-watch/src/attribution.rs | 2 | Unattributed change detection |
| crates/ori-watch (watcher, hooks) | 1 | |
| crates/ori-mcp (server tool scopes) | 2 | What agents can do |
| crates/ori-mcp (host) | 1 | |
| crates/ori-integrations (merge exposure, webhook verification) | 2 | |
| crates/ori-integrations (adapters otherwise) | 1 | |
| crates/ori-notify, ori-calibration | 1 | |
| crates/ori-rpc (auth, transports) | 2 | Client authentication |
| crates/ori-rpc (dispatch, schema) | 1 | |
| crates/ori-engine, ori-cli | 1 | Composition |
| apps/desktop/src-tauri (commands) | 1 | Thin wrappers |
| apps/desktop/ui | 0 for styling and layout, 1 for anything that sends an RPC | |
| spec/ | 1 (2 for SECURITY_NOTES, PERMISSIONS, RISK_MAP, ADRs) | Specification changes are PRs |
| templates/, profiles/ | 1 | Shipped methodology defaults |
| .github/workflows, scripts/release* | 2 | Supply chain |
| docs, README, copy | 0 | |
