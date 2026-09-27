# R34. The code map's threat model: a hostile repository that holds still

| | |
|---|---|
| Ruling | R34 |
| Tier | 1: the code map is tier 1 in `spec/RISK_MAP.md`; this ruling bounds what its review must find |
| Made by | The operator, in session, 2026-09-24, at ORI-T-0036 review. Recorded by the lead on 2026-09-27 |

## In scope

A static hostile repository. Nothing in it may cause a hang, a read outside the root or inside `.git`, unbounded time or memory, or a map reported as more complete than it is.

## Out of scope

A live writer changing the tree while it is being mapped, with one exception: the mapper must never hang, even then.

## Where it is enforced

The "Threat model" section of the module doc of `crates/ori-memory/src/code_map.rs` states this ruling, dated there 2026-09-24. It lists what a live writer can still do, rather than claiming it closed, and names the tests that hold the never-hang property against a race. The same section says on which platforms that property was verified and on which it was not.

## How it is used

[[R35]] uses this line to decide what a safety item is when a tier 1 ticket has not converged: a finding inside it is fixed before merge, and a finding outside it becomes a follow-up ticket.
