# Escalation E-0013: binding an MCP connection to one session by delivering it a per-session endpoint

| Field | Value |
|---|---|
| Trigger | `security` |
| Ticket and anchor | Batch 10 prerequisite PE1 (the endpoint and the bridge) and slice S2, `ops/batch-10-plan.md`; `spec/SECURITY_NOTES.md` trust boundary 2 |
| Raised by | The batch 10 planning coder, 2026-10-11; recorded by the lead |
| Question | In phase 1, may a session's MCP identity be bound by handing that session alone a per-session local endpoint, accepting the residual risk below? |
| Recommendation | Yes, on six conditions the documentation role records in `spec/SECURITY_NOTES.md`. (i) The endpoint sits in a per-session directory of mode 0700. (ii) No secret is in its path. (iii) Tier 2 tickets refuse to start in worktree-only mode. This is specified (`spec/SECURITY_NOTES.md`, "Container runtime unavailable"; ORI-P1-032) but not yet built, so the condition is a dependency, not a fact. (iv) Container mode mounts only that session's endpoint (phase 2). (v) Once the engine has a cryptographic random source, which it needs for identifiers anyway, the endpoint name becomes unguessable. (vi) A later `ori-memory` ticket adds a role lookup to the scope enforcer, so tools other than the memory tools do not rest on the engine's role alone |
| What is blocked | PE1 only. Every slice in `ori-mcp` is tested over in-memory channels and proceeds |

### Context

- The agent is the MCP client and connects to whatever ACP `session/new.mcpServers` names (today an empty list, `crates/ori-runtime/src/acp.rs`).
- The plan has the agent spawn a small relay that talks to a local endpoint the engine opens for that session alone. The connection is bound to the session by construction, and no message field can change the binding.
- In worktree-only mode (phase 1), a hostile process running as the same user that learns another session's endpoint could speak as that session. Delivering the endpoint is the binding; there is no secret behind it.
- The workspace has no cryptographic random source in its normal dependency graph.

### Options rejected

| Option | Why not |
|---|---|
| A bearer-token handshake | Needs a random source and a credential issued by the broker: a new-dependency decision |
| Peer credentials on the socket | `UnixStream::peer_cred` is unstable in std |
| An inherited file descriptor | An ACP agent's children do not reliably inherit one |
