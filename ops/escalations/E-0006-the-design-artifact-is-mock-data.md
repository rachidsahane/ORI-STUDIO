# Escalation E-0006: the design artifact is mock data about a different product, and four tickets were built around it

| | |
|---|---|
| Trigger | `spec_conflict` |
| Raised by | The lead, at ORI-T-0097 review, from a finding its coder made while scanning every file in the repository |
| Blocks | Nothing. It **unblocks** open escalation 4, which has been with the operator since batch 1 |
| State | Open, with the operator |

## What the file actually contains

`spec/design/Ori Studio.html` line 405 is a 342 KB JSON-escaped blob of mock user interface data. The lead verified its contents directly:

```
$ grep -oE "Ledgerline|ledger/src/entry\.rs|db/migrations/[0-9]+\.sql" "spec/design/Ori Studio.html" | sort | uniq -c
   8 Ledgerline
   1 db/migrations/0031.sql
   1 ledger/src/entry.rs
```

The rows are sample records for a fictional product called **Ledgerline**, with file paths this repository does not have:

```
['DVG-04', 'DATA_MODEL` `§5', 'Ledger entries are immutable after posting',
 'Allows amendment', 'ledger/src/entry.rs:12', 'open'],
['TCK-0445', 'Auto', 0, 'Citation gate flags 2 dead §-references in ops memory',
 'CONVENTIONS` `§2', 'qa-01', '11h'],
```

Every specification reference in that blob is a prop. So are its sixteen `AICD §n` citations, **including the twelve unresolvable ones** that ruling R19 records, that `RECORDED_UNRESOLVED` in `crates/ori-gates/src/sections.rs` exempts, and that open escalation 4 is open about.

One of the twelve is a mock screenshot of the citation gate refusing a bad citation:

> `Save refused - [AICD` `§17.9] on line 52 is not a section that exists.`

The quotations in this record are deliberately split across code spans, here and in the two mock rows above. Written whole, it is a real `AICD` subsection citation to a subsection that does not exist, and the citation scanner reads every file in this repository including this one. **The lead wrote it whole first and the gate refused the commit**, which is recorded at the end of this file. `ops/methodology-anchor-defects.md` solves the same problem by writing the twelve bare, as `§17.9` without the prefix, and that is the convention.

**The fabricated reference is the content of a mockup demonstrating the gate that refuses fabricated references.**

## Why this is an escalation and not a cleanup

Open escalation 4, in `ops/phase-1-backlog.md`, asks: *"Does gate 9 check markdown only, or everything under `spec/`?"* It has been open since batch 1 and it was raised because of the twelve references in this file.

**The file answers its own question**, and nobody opened it. Instead:

| Ticket | What it built around this file |
|---|---|
| ORI-T-0005 | recorded the twelve references as anchor defects |
| ORI-T-0014 | `RECORDED_UNRESOLVED`, an exemption list carrying the twelve numbers |
| ORI-T-0085, ORI-T-0091 | carried that exemption forward and asserted it still needed |
| ORI-T-0097 | excluded the file by name, with a marker asserted on every run, after reading it |

Four tickets of machinery, one open escalation, and one ruling, all managing sample data. The lead reviewed every one of those tickets and never asked what the twelve references were references to.

## The question

Three answers are available and they are not equivalent:

1. **Gate 9 checks markdown only.** The simplest, and it makes the twelve disappear as a category rather than as an exemption. It also means a fabricated reference in a shipped HTML design artifact is never checked, which is the case that produced this file.
2. **Gate 9 checks everything under `spec/`, and this file is excluded by name**, the way ORI-T-0097's new check excludes it: one entry, with the reason recorded and the marker asserted present on every run so the exemption cannot outlive its subject.
3. **The mock data is replaced with references that resolve.** It is a design artifact and its realism is the point, so this is the lead's least preferred: it makes a mockup lie about a different product in order to satisfy a checker.

**The lead recommends 2**, because it is the only one that keeps the file checkable if its content ever changes, and because ORI-T-0097 has already implemented exactly that shape for its own check and proved the exemption cannot go stale.

Whichever is chosen, **ruling R19 and `RECORDED_UNRESOLVED` should record that the twelve references are sample data**, not defects awaiting repair. Today both read as though a fix were owed.

## What this says about the fleet, and it is not flattering

The finding cost one agent one `grep` while it was scanning every file in the repository for something else. It was available to every ticket that touched the exemption list, and to the lead at every review.

The pattern is the one [[CR-007]] measured: **this class is bounded only by how hard somebody looks**, and nobody looked at the file itself because five records already described it.


## The lead committed this defect while documenting it

The first version of this record quoted the mockup's line whole, with the prefix. `citations_resolve_everywhere_but_the_recorded_design_artifact` refused it:

```
test sections::tests::citations_resolve_everywhere_but_the_recorded_design_artifact ... FAILED
citations: 56 distinct, 659 occurrences, over 310 files
```

Four checks red on three platforms, on a pull request whose entire subject is that a mockup of a citation gate contains a citation the gate would refuse.

**The gate worked.** It is one of the four installed under AICD §14, it read a file written minutes earlier by the lead, and it refused exactly what it was built to refuse. That is worth more than the escalation it was blocking, and it is the first time in this project that an installed gate has caught the lead rather than a coder.

It also makes the recommendation concrete. Option 3 in the question above, replacing the mock data with references that resolve, would have this same cost every time the mockup shows a refusal: a design artifact cannot demonstrate a gate refusing a bad citation without containing one. **Option 2, exclusion by name with the marker asserted on every run, is the only answer that lets a mockup stay honest.**
