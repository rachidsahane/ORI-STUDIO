# Escalation E-0008: a JSON crate for the engine's JSON-RPC surfaces

| | |
|---|---|
| Trigger | `new_dependency` |
| Raised by | Lead, at batch 5 planning, before ORI-T-0033 was started |
| Blocks | ORI-T-0033 (ACP client and headless adapter). Later: `ori-rpc` (batch 13) and `ori-mcp` (batch 10) |
| State | Answered by the operator in session on 2026-09-23: `serde` and `serde_json` approved for the workspace, pinned |

## Context

`spec/adr/ADR-0001-stack.md` chooses JSON-RPC 2.0 for the engine's inter-process interface ("JSON-RPC 2.0 over Unix domain socket or Windows named pipe") and notes it is "the same shape ACP and MCP use". `spec/ARCHITECTURE.md` routes the CLI, the desktop and headless clients to the engine over JSON-RPC. No document names a Rust crate for JSON, and on 2026-09-23 `Cargo.lock` held none. CLAUDE.md absolute rule 6 requires escalating any new crate, and a specification choosing a wire format does not discharge that, for the same reason E-0004 gives: the rule is about who decides what enters the trust boundary.

## Question

Approve `serde` and `serde_json` for the workspace?

## Recommendation

Approve both, pinned to exact versions with `Cargo.lock` committed, for every crate that speaks JSON-RPC (`ori-runtime` for ACP now; `ori-rpc` and `ori-mcp` later), so the question is not asked three times. Both are MIT OR Apache-2.0, already within `deny.toml`'s license allow list.

## Options rejected

| Option | Why not |
|---|---|
| Approve for `ori-runtime` only | Returns the same question at batches 10 and 13 with nothing new to decide |
| Hand-write a minimal JSON reader and writer | A hand-written parser for input from an external agent process is itself a security surface, and `ori-store`'s projection payload reader (`crates/ori-store/src/projections/payload.rs`) already states it is not a JSON parser and names what it gets wrong |
| Hold ORI-T-0033 | The engine cannot drive an agent session until the ACP client exists |

## Answer

**Approved: `serde` and `serde_json`, pinned, workspace-wide.** The version chosen, and `cargo deny check` and `cargo audit` against it, are recorded in the pull request that first adds them.

## What is blocked

Nothing, once answered. ORI-T-0033 proceeds.
