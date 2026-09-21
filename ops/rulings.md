# Lead rulings

Decisions the lead made under the operator's standing rule 5: anything the specification already decides, the lead decides. Each names the document that decides it. A ruling that authorizes a deviation from a specification document is recorded here **and** the document is corrected in a specification PR in the same batch, under standing rule 6. A ruling is not a specification change; it is the record of how an existing specification was read.

| # | Batch | Subject |
|---|---|---|
| R1 | 1 | `sections.json` granularity and what gate 9 accepts |
| R2 | 1 | Canonical versus generated instruction file |
| R3 | 1 | `agents/CLAUDE.md` excluded from `.claude/agents/` |
| R4 | 1 | "asapproved" is a corruption of "assigned" |
| R5 | 1 | `templates/` lives at the repository root |
| R6 | 1 | ADR-0002 merges first |
| R7 | 1 | Scope extension, ORI-T-0007 |
| R8 | 1 | `ori-integrations` does not depend on `ori-store` |
| R9 | 1 | Scope extension, ORI-T-0001 |
| R10 | 1 | `ori-watch` cites AICD §13 and §23, not §39 |
| R11 | 1 | ORI-T-0001 is tier 2, and the backlog's tier rule is corrected |
| R12 | 1 | `ori-notify` cites AICD §12 and §16, not §28 |
| R13 | 1 | `ori-mcp` cites AICD §8 and §25 |
| R14 | 1 | Two weaker citations accepted with a note, to be settled when gate 9 is installed |

## R1. `sections.json` granularity, and what gate 9 accepts

The methodology carries numbered subsections in exactly two places: `§24.1` to `§24.8`, and appendix `A.1` to `A.5`. Existing valid citations use them: CLAUDE.md cites "AICD appendix A.5", ENV_SETUP cites "AICD §24.2". CONVENTIONS requires every reference to resolve. Therefore `sections.json` indexes the 40 sections, the 3 appendices, `§24.1` to `§24.8` and `A.1` to `A.5`, and gate 9 accepts any of those forms. A reference to a subsection with no numbered heading fails. Decided by CONVENTIONS plus the methodology's heading structure. Applies to ORI-T-0005 and ORI-T-0047.

## R2. Canonical versus generated instruction file

`spec/agents/CLAUDE.md` is canonical. The root `CLAUDE.md` is generated from it and carries a header naming its source. Decided by spec/README.md's own statement that the root file derives from the canonical one.

## R3. `agents/CLAUDE.md` is excluded from `.claude/agents/`

It has no YAML frontmatter and none of the `name`, `description`, `model` or `tools` keys, so loading it as a subagent definition produces a broken agent. Decided by the structural difference AICD §32 draws between the product base and the role file.

## R4. "asapproved" is a corruption of "assigned"

It appears in root CLAUDE.md absolute rule 1 and in spec/agents/coder.md, both reading "the ticket you were asapproved". Fixed as a defect fix under standing rule 6.

## R5. `templates/` lives at the repository root

LLD section 1's repository layout places it there, beside `spec/`, `methodology/`, `ops/`, `profiles/` and `fixtures/`. Decided by LLD, the authority on layout.

## R6. ADR-0002 merges first

ADR-0002 records the exception under which every later tier 2 change in batch 1 merges. A record that arrives after the changes it governs records nothing. Lead ordering decision under AICD §12.

## R7. Scope extension, ORI-T-0007

Extended to `spec/agents/coder.md` and `spec/README.md` so the rest of the repository stops contradicting R2, R3 and R4. Neither module was claimed by another in-flight ticket.

## R8. `ori-integrations` does not depend on `ori-store`

LLD section 2's Mermaid graph puts `INT` among the crates with an edge to `STORE`. The same section's table gives ori-integrations the "Must not" of "Read the store", and API_SPEC section 4 states "adapters have no access to the store". Two documents against one edge of one diagram, and the two agree with the architectural reason: an adapter that can read the store can be handed the whole product by a compromised integration. `ori-integrations` depends on `ori-core` only, for the types its traits take. LLD section 2's graph is corrected in a specification PR in this batch.

## R9. Scope extension, ORI-T-0001

`crates/ori-cli/src/main.rs` added to the declared scope, since `ori-cli` is a binary and the scope named only `lib.rs`.

## R10. `ori-watch` cites AICD §13 and §23, not §39

ARCHITECTURE section 2's watch row cites "Lessons (39)". AICD §39, "Lessons from the first application", contains nothing about a working-tree watcher, attribution or unattributed change detection: its eight rows concern a deleted directory, a missing rollback point, inert CI workflows, fabricated section references, identifier aggregation, stale credentials, load-bearing paths and migrations. The methodology's actual basis for the control is `AICD §13`, git as the spine and the audit trail through which every change enters, and `AICD §23`, whose inherited golden rule is "Never touch the code. Every change is requested in language. A manual edit puts the system outside what the agents know." ARCHITECTURE section 2's watch row is corrected in a specification PR in this batch.

This ruling is itself an instance of the defect class §39 records, found in a document citing §39.

## R11. ORI-T-0001 is tier 2, and the backlog's tier rule is corrected

The coder self-assessed ORI-T-0001 as tier 2 and upgraded it. The upgrade is accepted, and under AICD §11's upgrade-only rule only a human may lower it.

RISK_MAP tiers `crates/ori-broker` at 2 with whole-crate granularity, and CLAUDE.md states "A change to any of them is tier 2 whatever the ticket says". ORI-T-0001 creates `crates/ori-broker/Cargo.toml` and `crates/ori-broker/src/lib.rs`.

`ops/phase-1-backlog.md` contradicted itself on this: "Where a ticket creates a file under a path RISK_MAP tiers 2, the ticket is tier 2 whatever its size" followed immediately by "A crate skeleton that contains no tier 2 module content is tier 1". The second sentence is wrong and is removed. CLAUDE.md is absolute and nothing narrows it. Every ticket in the backlog whose declared scope touches a RISK_MAP tier 2 path is re-tiered at its batch plan.

## R12. `ori-notify` cites AICD §12 and §16, not §28

ARCHITECTURE section 2's notify row cites "28, PRD N-01" and the crate kept only the methodology half. AICD §28 is "The fleet dashboard"; a search of its full body for `notif`, `digest`, `interrupt`, `review window` and `cadence` returns zero hits for all five. It governs what the supervision surface shows and hides, not how anything is routed.

The rule the crate's `Router` implements is in §12: "Blocked and escalated items land in a human review queue that is processed at a fixed cadence (for example twice a day). Only incidents interrupt a human outside the cadence." §16 carries the interrupt half: "Incidents are the one case where a human is interrupted outside the review cadence."

Found by the coder's own audit of all sixteen doc comments, verified independently by the lead. ARCHITECTURE section 2's notify row is corrected in a specification PR in this batch.

## R13. `ori-mcp` cites AICD §8 and §25

The strings "MCP" and "Model Context" appear nowhere in the methodology. That is correct and not a defect: MCP is a tooling choice recorded in ADR-0001, and AICD §5 makes tooling an extension point, naming "how the memory service is implemented" among the things that may change without touching the core.

The crate's headline concept is stated in §8, not §25: "On top of the four layers sits a memory service, exposed to the agents as a tool connection." §25 supports the other half, the scope enforcer and the retrieval API behind `ToolScopes` and the server's memory tools. Citing §25 alone pointed at the implementation model while omitting the section that establishes the tool connection at all.

## R14. Two weaker citations accepted with a note

The same audit raised two further doubts. Both are thin rather than wrong, and are accepted as they stand:

- `ori-rpc` and, derived from it, `ori-cli` cite §28. §28 does govern what the human supervision surface shows, which is what the RPC layer serves, but it says nothing about a transport, a method registry or subscriptions. ARCHITECTURE section 2 pairs it with API_SPEC, which is where those live.
- `ori-integrations` cites §16, which genuinely covers the observability slots and supplies the untrusted-input rule, but not the version control host or CI slots, which come from §13 and §15. ARCHITECTURE pairs it with PROJECT_BRIEF principle 12, where the slot design originates.

Both are recorded here rather than changed, because a thin citation that is defensible is not the defect class §39 names, and churning them now would cost more than it buys. They are revisited when gate 9 is installed under ORI-T-0047, which is the point at which "resolves" acquires a mechanical definition.

## Note on how R10, R12 and R13 were found

All three are the same defect class, found in the same ticket, in doc comments written faithfully from ARCHITECTURE section 2's component table. The table is the common cause: it cites bare section numbers with no prose tying each to what the component does, so an error in it propagates silently into every crate that reads it. Two of the three errors originate in the table itself.

The coder found R12 and R13 by auditing all sixteen citations unprompted after being corrected once on R10. That is worth recording as evidence for the calibration note in CR-001: the correction generalised without being told to generalise.
