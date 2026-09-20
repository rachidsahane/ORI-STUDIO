# Runbook: recover-engine

Trigger: the engine starts and finds sessions in Running state from a previous process.
Preconditions: the product database opens; the lock file is stale or absent.
Steps:
1. For each Running session: revoke every credential issuance (record `credential.revoked`), terminate any process matching the session (worktree path, container id), release its lock entries, set the session Killed with reason `engine_restart`, return its ticket to Queued, append `session.recovered`.
2. Verify no orphan container or process remains; record the check.
3. Re-run the liveness gate; if any gate is `missing`, open an incident.
4. Rebuild projections if the log hash chain verification fails at any point; if the chain is broken, open an incident and refuse to launch sessions until a human resolves.
Verification: `fleet.state` shows no Running session from before the restart; `broker.identities` shows no active issuance older than the restart.
Rollback: none; the procedure is idempotent.
May run: engine automatically; human reviews the incident list.
