# R36. Container network: egress now, provider-only later

| | |
|---|---|
| Ruling | R36 |
| Tier | 2: `crates/ori-runtime/src/container.rs` is tier 2 in `spec/RISK_MAP.md` |
| Made by | The operator, in session, at ORI-T-0031 review, on or before 2026-09-23 (commit 7d4f0b3, pull request 62). Recorded by the lead on 2026-09-27 |

## The decision

"Egress now, provider-only later."

A container launched for a real session has outbound network access: `ContainerSpec::for_launch`, the one path a real launch uses, always chooses `NetworkPolicy::Egress`, written into the argv as `--network bridge`. `NetworkPolicy` has no default, so a caller that does not choose does not compile.

## Why

The first draft hard-coded `--network none`, reading `CLAUDE.md` rule 6 as covering the agent process. Rule 6 limits which crates of this repository may make network calls. An agent runtime calls its model provider itself, so with no network no real agent could run in container mode, and every session would have fallen back to worktree-only, which has the host's network and the host's filesystem both.

## What container mode is, stated plainly

Filesystem, capability and privilege isolation, not network isolation: a read-only root, the worktree as the only writable mount, every capability dropped, no new privileges, a non-root user. The module doc of `crates/ori-runtime/src/container.rs` says the same.

`spec/ENV_SETUP.md` lists, among the actions refused to every identity, calling a network host not in the integration configuration. In container mode today nothing at the network layer refuses it.

## Provider-only egress

ORI-T-0110, tier 2: an engine-side proxy that restricts a container's outbound network to the operator's configured provider endpoints. Allocated in pull request 62's body, recorded in `ops/lock-table.md` on 2026-09-27, not started.
