# INC-0003: two coders wrote to the primary checkout in one wave, and the lead's own commit template was refused by gate 13

| | |
|---|---|
| Kind | `unattributed_change` in effect, twice, though both actors are known |
| Tickets | ORI-T-0053 and ORI-T-0044 |
| Detected | ORI-T-0053 caught itself; ORI-T-0044 was caught by ORI-T-0053's report |
| Damage | None to any commit. Working tree only, on both occasions |

## What happened

Every coder ticket in this wave carried the line "Write nothing to `/Users/alimsahane/Documents/ORI STUDIO`" in its header, with the worktree path given immediately above it. Two of five wrote there anyway.

**ORI-T-0053 caught itself.** It noticed `git worktree list` and `git status` disagreeing with what it expected, reverted the primary checkout, verified it clean, and rebuilt the same work in the correct worktree from scratch. It then reported the incident at the top of its own hand-back, before its results, and separately flagged that the same directory carried somebody else's uncommitted work which it deliberately did not touch, on the grounds that it was out of scope and not its to judge.

**ORI-T-0044 did not notice.** Its work, 1218 lines of `significance.rs` plus a manifest edge and a `mod` line, was sitting uncommitted on `main`'s working tree when ORI-T-0053's report arrived. The lead copied all of it to the scratchpad before touching anything, confirmed `main`'s committed tree was intact, and told that coder to move to its worktree and say when it was clear.

## Why this is the lead's defect and not the coders'

Two of five in one wave is a rate, not an accident. The instruction was present, correctly worded, and ignored twice, which means the instruction is in the wrong place or the wrong shape.

The worktree path appears in a header block alongside a dozen other standing facts. Nothing in the ticket asks the coder to **confirm** it is in the right directory before writing, and nothing fails if it is not. That is the same defect class this project has been finding all round: a control that is stated rather than exercised.

**The repair, for every ticket from here**: the first instruction is to `cd` to the worktree and run `git rev-parse --abbrev-ref HEAD`, and not to proceed until it prints the expected branch name. A statement becomes a check.

## A second lead defect, found by the same two coders

**The commit template in every ticket this session is wrong.** It shows:

```
Ticket: ORI-T-nnnn
Spec: DOC.md#anchor

<a sentence of prose>

Co-Authored-By: ...
```

Git parses trailers only from the **last paragraph**. That template splits them, so `Ticket:` and `Spec:` are invisible to every tool that reads trailers, including the traceability chain of AICD §13.

Gate 13 refused it for both coders on their first commit (`TICKET_UNPARSED`, `SPEC_UNPARSED`), and both amended before the lead saw it. ORI-T-0041 named it as "exactly the present but reporting nothing class this repository names".

**Gate 13 caught the lead's template twice.** It is the one gate in this repository that has now caught the lead more often than a coder, and it caught a defect the lead had written into every ticket of the wave.

## What is not damaged

`main`'s committed tree was never touched. No commit was made from the primary checkout by either coder. Both incidents were working-tree only, and every byte of ORI-T-0044's work was preserved before the lead cleaned it.


## Two standing facts of the lead's that were wrong, found by the same wave

ORI-T-0044 corrected both. They are recorded here because every ticket of the wave carried them.

**Trap 1 undersold its own failure mode.** The lead's ticket said an intra-doc link in a module's `//!` header resolves in the crate root's scope, which reads as "re-qualify it with `crate::<module>::<Item>` and it works". **For a private item that is false.** From that resolution context a private item is not mis-scoped, it is invisible: rustdoc reports "no item named X in module significance", not "private", and a fully qualified path fails too. There is a second, harsher rule underneath, `rustdoc::private_intra_doc_links`, which is deny-level under `-D warnings` and fires for a link from **any public item's documentation** to a private item, not only from the header. The fix for a private item is not to bracket-link it at all.

**A fifth trap the lead never documented.** `crates/ori-gates/src/sections.rs` carries a check that fails the whole workspace suite when a doc comment anywhere contains a bare snake_case span matching a declared `#[test]` function that is not registered in that file's own table. The lead's standing facts named only the narrower case of deliberately backticking a test name.

The register is in `sections.rs`, which was outside ORI-T-0044's declared scope and claimed by a sibling in the same wave, so it could not register anything. Its repair is the useful part: write the citation with a `tests::` or `ori_core::...::tests::` prefix, because the check's own grammar excludes any span carrying `::`. That is a fix available to a coder who may not touch the register, and it should be in the standing facts rather than rediscovered.

Both corrections have the same shape as everything else this wave found: a rule stated once in a header, never exercised, and wrong in a way only contact reveals.
