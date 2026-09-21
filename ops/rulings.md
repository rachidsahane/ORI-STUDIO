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
| R15 | 1 | ENV_SETUP section 1 overstates the forbidden-action requirement |
| R16 | 1 | **Corrected.** Where the forbidden-action test lives and how shell callers reach it |
| R17 | 1 | `target/` belongs to ORI-T-0003 |
| R18 | 1 | Gate 13 is not checkable from a coder worktree |
| R19 | 1 | The unresolvable citations in the design mockup are reported, not corrected |
| R20 | 1 | `thiserror` deferred to batch 2, alongside gate 7 |
| R21 | 1 | `Cargo.lock` is an in-scope consequence of a workspace members entry |
| R22 | 1 | The UI tree is `ui/src/` |
| R23 | 1 | LLD governs directory layout; DESIGN governs the phase 3 screen set |
| R24 | 1 | AICD section 28 is internally inconsistent; upstream, not ours |
| R25 | 1 | Operational records under `ops/` are the lead's to write until `aicd_report` exists |
| R26 | 1 | How CI obtains `cargo-audit` and `cargo-deny` is an implementation choice, not a new dependency |
| R27 | 1 | "Unreadable" is a parser verdict, not a YAML validity claim. The lead was wrong |
| R28 | 1 | A gate's implementation does not live in a fixture directory |
| R29 | 1 | AICD §28 stands for `ori-cli`, and the methodology is thin here |

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


## How R15 to R24 were nearly lost, and what it cost

**This section records a process failure by the lead.** R15 to R24 were issued inside the prompts of a workflow and were never written here. Four coders cited them faithfully in code and documentation, producing six references to rulings that did not exist in the repository: `setup-dev.sh` cited R16, and `sections.rs` cited R19 four times and R20 once.

That is the fabricated-reference defect class of AICD §39, authored by the lead. It is the second occurrence. The first was R8, which the ORI-T-0001 reviewer caught with the finding that the ruling authorizing a deviation from LLD existed nowhere in the tree. R8 was then recorded, and the same mistake was repeated at ten times the scale in the next wave.

The rule, now explicit: **a ruling exists when it is written in this file, not when the lead states it.** A ruling communicated only in a prompt is unauditable, uncheckable by the citation gate, and invisible to anyone reading the repository afterwards. No ticket may be dispatched citing a ruling number that is not already committed here.

Both occurrences were caught by review, not by the lead. Neither would have been caught by gate 9 as currently specified, since `R16` is not an `AICD §n` citation and resolves against nothing. That is an argument for widening the citation gate's rule set at ORI-T-0047 to cover internal ruling references as well as methodology ones.

## R15. ENV_SETUP section 1 overstates the forbidden-action requirement

ENV_SETUP section 1 states that `scripts/setup-dev.sh` "runs the forbidden-action test against a fixture product; a fresh clone must pass it before any work." That is unsatisfiable today: the harness (ORI-T-0029) and the fixtures (ORI-T-0073 to ORI-T-0076) do not exist. Reporting it as not yet available, naming the tickets, is correct and is what the script must do. ENV_SETUP is amended in a specification PR so the requirement binds when those tickets land, rather than reading as a rule a fresh clone already violates.

## R16. Where the forbidden-action test lives, corrected

**The version of this ruling issued in the wave 2 prompt was wrong**, and the review caught it. It accepted `scripts/forbidden-action-test.sh` as the harness path. `ops/phase-1-backlog.md` assigns ORI-T-0029 the declared scope `crates/ori-broker/src/forbidden.rs`, tier 2, so the harness is a Rust module in the broker, not a shell script, and wiring the project's most important control to a path no ticket produces would have left it permanently reporting "not yet available".

Corrected: the harness is `crates/ori-broker/src/forbidden.rs` (ORI-T-0029). ORI-T-0029's declared scope is extended now, before it is planned, to add a thin wrapper at `scripts/forbidden-action-test.sh` that invokes the Rust harness and exits with its status, so that shell-level callers (`scripts/setup-dev.sh`, and CI_CD gate 8) have one stable entry point. The wrapper is in ORI-T-0029's scope and nothing else claims it.

## R17. `target/` belongs to ORI-T-0003

`.gitignore` is ORI-T-0003's declared scope. No other ticket edits it; the lead stages explicitly until it merges.

## R18. Gate 13 is not checkable from a coder worktree

The commit-trailer gate checks commits on a pull request, and in this fleet the lead makes the commits, so a coder worktree has no commits to check. `scripts/gates.sh` reports it as not available and states that as the reason.

## R19. The unresolvable citations in the design mockup are reported, not corrected

ORI-T-0005 found that of the distinct methodology citations in the repository, twelve do not resolve, all inside `spec/design/Ori Studio.html`, a design artifact whose text is display copy in a visual mockup. Whether gate 9's scope covers non-markdown files under `spec/` is an open question with the operator, recorded as open escalation 4 in the backlog. The index reports them; nothing is corrected until that is answered.

## R20. `thiserror` deferred to batch 2

CONVENTIONS names `thiserror` as the house error convention, so adopting it implements the specification rather than adding an unsanctioned dependency. But the first external crate this project takes on should land alongside the dependency audit that watches it, CI_CD gate 7, which arrives with ORI-T-0016. Until then a hand-written `Display` and `std::error::Error` implementation stands, with a comment naming the conversion.

## R21. `Cargo.lock` is an in-scope consequence of a workspace members entry

A ticket that adds a workspace member necessarily changes `Cargo.lock`. This is accepted as in scope on the same basis as R9, and does not require a re-declaration.

## R22. The UI tree is `ui/src/`

`spec/design/DESIGN.md` section 2 names `ui/src/tokens.css` as a concrete path, which settles where LLD section 3's unqualified names sit.

## R23. LLD governs directory layout; DESIGN governs the phase 3 screen set

Three documents name different screen sets: LLD section 3 lists twelve, PRD section 6 fifteen, DESIGN section 1 eighteen. For scaffolding directories, LLD governs, because LLD is the authority on layout. The authoritative screen set for phase 3 is DESIGN's. The three-way divergence is real and is recorded for a specification PR.

## R24. AICD section 28 is internally inconsistent

Its prose says "the four human functions" while its own table has five rows, the fifth being Reliability and governance, which AICD §18 lists as a seat rather than one of the four functions. This is a defect in the methodology, not in this repository. It joins the anchor defects in the report the operator carries upstream.


## R25. Operational records under `ops/` are the lead's to write until the engine exists

The ORI-T-0013 coder raised a real conflict the lead created. CLAUDE.md absolute rule 5 says "You never write to `spec/` or `ops/` directly. Specification changes go through the documentation role's PR; operational records go through the `aicd_report` tool." The ticket's declared scope, granted by the lead as lock claim 11, included `ops/gates/gate-1.md`. So the ticket instructed a coder to break an absolute rule.

`aicd_report` does not exist: it is an MCP tool of an engine that is sixteen empty crates. Until it does, the lead writes operational records, which is already what has happened for the rulings, the lock table, the calibration records and the incidents.

The division for a gate proof: **the coder produces and verifies the evidence, the lead records it.** The coder builds the planted defect, runs the gate against it, and returns the real output. The lead writes `ops/gates/<gate>.md` from that output and owns it in the commit. This keeps the producing agent separate from the agent that records what was produced, which is the point of AICD §7, and it stops a coder writing its own proof of its own work.

`spec/runbooks/prove-gate.md` step 4 says "Store the GateProof with both run references" without naming who stores it, and `spec/TESTING.md` section 4 says a proof is "recorded in `ops/gates/`" without naming an author. Neither contradicts this. When `ori-gates::Prover` (ORI-T-0045) and `aicd_report` exist, the engine stores the proof and this ruling expires.


## R26. How CI obtains `cargo-audit` and `cargo-deny` is not a new dependency

ORI-T-0016 escalated that its gate 7 jobs fetch two release artifacts from github.com at run time, pinned by SHA-256, and asked whether that is a `new_dependency` under CLAUDE.md rule 6.

It is not. `spec/ENV_SETUP.md` section 1 already lists `cargo-mutants`, `cargo-audit` and `cargo-deny` as required development tooling, and `spec/CI_CD.md` section 1 makes gate 7 `cargo-audit` and `cargo-deny` by name. Using them implements the specification rather than extending it. CLAUDE.md rule 6 governs runtime dependencies of the engine and network calls from the engine; a CI runner fetching a tool is neither.

What the escalation was right about is that this is a supply-chain surface inside the supply-chain gate, so the manner matters. Pinning each artifact by SHA-256 is required and is what the ticket did. It is strictly better than `cargo install` from crates.io, which resolves a version range at run time and builds arbitrary build scripts. The operator is told, because the decision is theirs to overturn, but the ticket is not blocked on it.

## R27. "Unreadable" is a parser verdict, not a claim about YAML validity. The lead was wrong

ORI-T-0084's ticket told the coder that "a sample owed `unreadable` that parses cleanly is the same defect in the other direction". The coder refused that framing and was right.

`unreadable` is the answer gate 1's awk parser gives when it cannot determine a workflow's structure. It is a verdict about the parser, not about YAML. A file can be perfectly valid YAML and still be unreadable to a line-oriented parser, which is exactly what `unreadable-quoted-job-name.yml` is: valid YAML whose job name is quoted in a form the parser will not guess at. That sample is owed `unreadable` and is correct as it stands.

The real defect, which the ticket did name correctly, is the opposite: a sample owed `dead` that a real YAML parser rejects, because it tests the parser against an input GitHub would never accept.

Second lead error of this kind. The first was asserting three copyright spellings as established without re-deriving them. Both were caught by a coder applying a standard the lead had given it.

## R28. A gate's implementation does not live in a fixture directory

ORI-T-0016 put `deny.toml`, the repository's dependency policy, and `secret-scan.sh`, the implementation of gate 7's third check, under `fixtures/planted/gate-7/`. It escalated this itself rather than leaving it silent.

It is wrong, and the cause is the lead's declared scope, which granted only `ci.yml` and `fixtures/planted/gate-7/**`. A fixture directory holds planted defects: inputs a gate is run against. A policy the whole repository is judged by, and a scanner the gate invokes, are neither.

`deny.toml` belongs at the repository root, which is where `cargo-deny` looks by default and where a reader expects a dependency policy. The scanner belongs with the other scripts a gate runs, beside `scripts/gates.sh`. The scope is extended to `deny.toml` and `scripts/secret-scan.sh`; neither is claimed by another in-flight ticket.


## R29. AICD §28 stands for `ori-cli`, and the thinness goes upstream

Ruling R14 accepted `ori-cli`'s citation of AICD §28 as thin but defensible, and did not address the one sentence that most looks like it disqualifies a command-line client. ORI-T-0018's coder found it and refused to leave it unaddressed:

> Humans supervise through a dashboard, not through terminals.

That is §28's opening sentence, verified in the methodology text.

**The citation stands**, on the coder's reading: the contrast §28 is drawing is between a designed supervision surface and watching an agent think, which §28 then hides from *every* surface, not between a dashboard and a command line. It does not reserve supervision to one renderer. The coder searched for a better section and there is none: "command line", "command-line" and "scripting" return zero hits in the whole methodology, and "terminal" appears exactly twice in 1519 lines, in §12's pairing mode and in that sentence. A methodology with no section about a command-line client leaves §28 as the only one about the surface this crate is.

**What was actually wrong is now fixed, and it was not the number.** The old comment derived §28 at two removes: ARCHITECTURE section 2's table, to `ori-rpc`'s row, to `ori-cli`. That is exactly the propagation path this file already blames for R10, R12 and R13. It was also wrong on its own terms, because **`ori-cli` does not depend on `ori-rpc`**: LLD section 2's graph and `Cargo.toml` both give it `ori-engine` only. And its conclusion, that the CLI "adds nothing of its own", understated the crate: because ARCHITECTURE section 9 makes the CLI capability-equal with the UI, §28's deliberately-not-shown list binds `ori` too, which is a real and checkable constraint.

**The thinness goes upstream.** ROADMAP phase 1 ships the CLI as the entire human surface with no UI at all, a state §28's prose does not contemplate. That joins R24's finding, that §28's prose says "the four human functions" while its own table has five rows, in the report the operator carries to the methodology repository. Two defects in one section is a section worth revisiting.

## A blind spot in gate 9, found by reading rather than running

The same coder's first draft used bare `§28` as a back-reference three times. CONVENTIONS requires every methodology reference to be `AICD §<n>`, and the citation checker deliberately collects only `AICD §n`, because the repository uses bare `§n` for its own documents.

So **a bare `§28` is a methodology reference that no gate checks and no test catches**, and it would have shipped green. It was caught by reading CONVENTIONS, not by running anything.

This is the second known blind spot in gate 9's rule set, alongside internal `Rn` ruling references. Both belong in ORI-T-0047 when the gate acquires a mechanical definition.
