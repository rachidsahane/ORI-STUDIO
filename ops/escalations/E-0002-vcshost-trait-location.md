# Escalation E-0002: where the adapter traits are declared

Format: AICD §12. Raised by the coder on ORI-T-0001, confirmed by the lead.

| | |
|---|---|
| Trigger | `contract_change` (API_SPEC section 4) |
| Raised by | Coder, ORI-T-0001; lead confirms it is a real contradiction and does not resolve it |
| Blocks | Nothing yet. Empty skeletons do not force the question |
| Will block | ORI-T-0053 (merge queue) and ORI-T-0059 (vcs host reference adapter), batches 8 and 11 |
| State | Open, with the operator |

## The contradiction

`spec/LLD.md` section 2's crate table gives `ori-orchestrator` the "Must not" of "Call adapters directly except `VcsHost::merge` through `MergeQueue`". That carve-out only means anything if `ori-orchestrator` can name the `VcsHost` trait.

The same table assigns "Traits in API_SPEC section 4" to `ori-integrations`. So the trait is declared in `ori-integrations` and named in `ori-orchestrator`.

But LLD section 2's dependency graph has no edge from `ORC` to `INT`, and the section's own rule is "Dependencies point downward only. A crate may depend on those below it, never above or sideways unless listed." An unlisted sideways edge is forbidden, so as written `ori-orchestrator` cannot name `VcsHost`, and the carve-out that governs the only merge path in the product cannot be expressed.

`spec/API_SPEC.md` section 4 states the same requirement from the other side: "the `merge` method exists only on the `VcsHost` trait object held by the merge queue". A trait object held by the merge queue is a trait the merge queue's crate must name.

## Why the lead is not deciding it

This changes an internal contract, which CLAUDE.md makes an escalation trigger without exception. The specification does not decide it: LLD contradicts itself, and API_SPEC restates the requirement without saying where the trait lives. Standing rule 5 gives the lead anything the specification already decides; this is not that.

## The two resolutions, and the lead's recommendation

**Option A, recommended. The adapter traits move to `ori-core`.** `ori-core` sits below everything and already owns the domain types the traits take, such as `AgentIdentity`. `ori-integrations` implements them; `ori-orchestrator` names `dyn VcsHost` without a sideways edge; the graph stays acyclic and the downward-only rule holds unmodified. Cost: `ori-core`'s "Must not do IO" constraint has to be read as covering implementations, not trait declarations, which is already how it works for every trait in Rust.

**Option B. Add a listed `ORC --> INT` edge.** Smaller diff, but it puts a crate that must not read the store above a crate that may, inverting the layering the rest of section 2 is built on, and it makes every adapter a compile-time dependency of the orchestrator.

Either way `spec/LLD.md` section 2 and `spec/API_SPEC.md` section 4 both change, so this is a specification PR whichever you choose.

## What happens until it is answered

ORI-T-0001 encoded the graph literally: `ori-orchestrator` has no `ori-integrations` dependency. That is the specification as written, not a resolution. Empty skeletons compile either way. The question binds when ORI-T-0053 builds the merge queue in batch 8.
