# ADR-0003: Full-text index engine

| | |
|---|---|
| Status | Accepted |
| Date | September 2026 |
| Deciders | Architect (Alim Sahane) |
| Supersedes | ADR-0001, "Decision" table, "Full-text index" row only. Every other row of ADR-0001 stands; its "Options considered" entry for full-text is left as written, for history |
| Methodology | AICD §20 (stack selection policy), AICD §14 ("a gate is installed only when it has been seen to fail") |
| Escalation scope | Any change to the full-text index engine named in "Decision" is decisional and tier 2, carrying forward ADR-0001's own escalation scope for this row unchanged |

## Context

ADR-0001 §Options considered ("Storage and indexes") weighed the two candidates and recorded the trade-off it was made under: "**Full-text:** tantivy versus SQLite FTS5. Both embedded; tantivy is faster at scale, FTS5 is zero extra dependency." ADR-0001 §Decision chose tantivy: "| Full-text index | tantivy |".

Building that decision surfaced a cost the options paragraph did not have in view. Ticket ORI-T-0035 (repository indexer) added `tantivy =0.26.2` (`default-features = false`, `features = ["mmap"]`). It added 107 crates to `Cargo.lock` and failed gate 7, `cargo-audit`, `cargo-deny` (advisories, licenses), secret scan (CI_CD §1). Gate 7 is proven and installed in the sense AICD §14 requires of any gate cited as protection: "A gate that has only ever been observed passing is not installed. Every gate ... enters service only after a demonstration on a planted defect: it passes on a clean tree, it fails on the planted defect, and the failure is visible where a human would look." Its refusal here is exactly that gate doing its job, not a false negative to be waived.

The refusal had two independent causes:

- `cargo deny check` refused 12 duplicate crate versions: tantivy's `fs4` pulls `windows-sys 0.59`/`windows-targets 0.52` against `0.60`/`0.53` already pinned via `keyring`; its `uuid` pulls `getrandom 0.4`/`r-efi 6` against `0.3`/`5` via `proptest`.
- `cargo audit --deny warnings` denied advisory RUSTSEC-2026-0253 (`lru 0.16.4`, "Potential use-after-free due to lack of panic safety in `LruCache::pop()`"), fixed only in `lru >= 0.18.2`, outside tantivy's `^0.16.3` range.

tantivy 0.25.0 was tried as a substitute and was worse.

The SQLite already bundled for durable state (`rusqlite =0.40.2`, `features = ["bundled"]`, via `libsqlite3-sys 0.38.2`, `crates/ori-store/Cargo.toml`) is compiled with `-DSQLITE_ENABLE_FTS5`. A probe created an FTS5 table against that build and returned a `MATCH` hit: the capability ADR-0001 costed as "zero extra dependency" is already present in the workspace, unused.

The operator decided, in session on 2026-09-23: replace tantivy with SQLite FTS5, rather than relax gate 7 or hold the repository indexer.

## Options considered

- **Keep tantivy and relax gate 7 for its dependency graph.** Rejected. RUSTSEC-2026-0253 is a use-after-free advisory, not a license or style nuisance, and gate 7 was proven on a planted defect before it was ever cited as protection (AICD §14); carving an exception into a proven gate for one crate's transitive graph is the "present but reporting nothing" failure AICD §14 names, applied to this one dependency rather than removed from the codebase.
- **SQLite FTS5.** Chosen. Already present in the linked `libsqlite3-sys` build; no new crate; no change to the duplicate-version or advisory surface gate 7 already polices; keeps the durable-state engine and the full-text engine as the same embedded SQLite file ADR-0001 §Decision already chose for "Durable state".
- **Hold the repository indexer until tantivy's dependency graph clears `cargo deny` and `cargo audit` on its own.** Rejected for now. PRD K-02 (full-text repository indexer) is P0, scheduled at ROADMAP phase 1 batch 6, and the indexer is a precondition for the retrieval package, the citation checker and the drift audit that batch also delivers; holding it has no term, since no date is known for a tantivy release whose graph clears this workspace's gates.

## Decision

SQLite FTS5 is the one full-text index engine. `crates/ori-store`'s already-bundled `rusqlite`/`libsqlite3-sys` (`-DSQLITE_ENABLE_FTS5`) serves it; no new crate is added. Ranking is by SQLite's `bm25()`. ADR-0001 §Decision, "Full-text index" row, is corrected to point here; its "Options considered" entry is left as written, for history, and this document's Context reproduces the two lines the decision changes.

## Consequences

Easier: gate 7 stays green without an exception written against it; the repository indexer adds no crate and no new duplicate-version or advisory surface for `cargo deny` and `cargo audit` to track; the durable-state engine and the full-text engine are the same embedded SQLite file, so `index/` (LLD §6) is SQLite state like `product.sqlite`, not a second embedded engine with its own file format and its own dependency graph.

Harder: the scaling trade-off ADR-0001 §Options considered recorded, "tantivy is faster at scale", is foregone for as long as this decision stands. That trade-off is not a live concern today, since ORI-P1-030 (`criteria/phase-1.md`) has no recorded baseline yet to measure FTS5 against, and it is carried below as a revisit condition rather than as a reason to reopen this decision now.

What the agents must respect: no agent adds `tantivy`, or any other full-text crate, without escalating `new_dependency` and reopening this ADR; the repository indexer (`ori-memory::Indexer`) is built against `rusqlite` only.

## Revisit conditions

- A tantivy release (or another full-text crate) whose dependency graph passes `cargo deny check` and `cargo audit --deny warnings` unmodified in this workspace, checked against the pinned versions current at the time (`keyring` for `windows-sys`/`windows-targets`, `proptest` for `getrandom`/`r-efi`, and the `lru` floor RUSTSEC-2026-0253 sets).
- An index size, once ORI-P1-030 has a recorded baseline (`criteria/phase-1.md`), at which SQLite FTS5 measurably fails that baseline.
- Either condition is evidence to bring back to this ADR, not a standing reason to reopen it before then.
