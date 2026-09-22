# Escalation E-0007: AICD §17 says the lead merges tier 0, and three project documents say it cannot

| | |
|---|---|
| Trigger | `spec_conflict` |
| Raised by | The lead, from ORI-T-0022's finding, which implemented AICD §17 verbatim and reported every divergence rather than blending them |
| Blocks | **ORI-T-0029**, the forbidden-action test. Whichever way this goes, one of two tickets is wrong, and the loser is a test change |
| State | Open, with the operator |

## The four documents, verified

| Source | What it says |
|---|---|
| AICD §17, permission matrix | Lead / Reviewer, Code repositories: **"Read all; merge tier 0"** |
| `spec/ENV_SETUP.md` section 5 | lead, Repository: **"Read all; approve tier 0 merges for the merge queue to perform"**, credentials: **no merge credential** |
| `spec/ENV_SETUP.md` section 6, forbidden actions | **"lead \| Merge any pull request"** must be refused |
| `spec/SECURITY_NOTES.md` trust boundary 4 | "it never held that credential; **the merge queue alone merges**" |
| `CLAUDE.md`, load-bearing facts | "Only `ori-orchestrator::merge_queue` calls `VcsHost::merge`" |

`spec/SECURITY_NOTES.md`'s authorization model names AICD §17 as **the source** and `spec/ENV_SETUP.md` section 5 as the manifest. So the source and the manifest disagree about the single most consequential cell in the matrix.

## What ORI-T-0022 did, and why it is the right thing to have done

It implemented the source. `permits(Lead, CodeRepository, Merge)` returns `Allowed { limit: TierZero }`, and it reported the divergence instead of quietly narrowing to the manifest.

That is correct under CLAUDE.md rule 7 and AICD §39: a coder that reconciles two specifications on its own authority has made a specification decision nobody recorded. The alternative would have produced a permission function that silently disagrees with the document `ori-broker` will be built from, with nothing saying so.

## Two readings, both coherent

1. **The cell means authority.** The lead *authorizes* a tier 0 merge; the merge queue performs it; the lead holds no merge credential. All five sources then agree, and `spec/ENV_SETUP.md` section 5's wording ("approve tier 0 merges for the merge queue to perform") is the precise statement of what §17's four words compress. **The permission function keeps `Allowed { limit: TierZero }` and ORI-T-0029 asserts that no merge credential is ever issued to the lead**, which is a different claim about a different layer.
2. **The cell means capability.** Then the project's three documents narrow the methodology, `permits(Lead, CodeRepository, Merge)` must become `Refused`, and AICD §17's cell wants an amendment upstream.

**The lead recommends reading 1.** It is the only one under which nothing is wrong: the matrix answers "what may this role cause to happen", the credential answers "what may this role do with its own hands", and the two are different questions that this product deliberately separates. Reading 2 requires calling the methodology wrong about a cell that the project's own manifest merely elaborates.

## Why it cannot wait for ORI-T-0029

ORI-T-0029 builds `crates/ori-broker/src/forbidden.rs`, which `spec/ENV_SETUP.md` section 6 requires to refuse "lead: merge any pull request". Under reading 1 that test and the permission function coexist, because they are about different layers. Under reading 2 they contradict, and the permission function is wrong.

Writing ORI-T-0029 before this is answered means writing a test that may have to be changed, and a test change by an agent is trigger `test_modified` and a stop.

## Three more divergences in the same pair of documents

ORI-T-0022 found these while transcribing the matrix. None blocks anything today; all will bind when `ori-broker` is built.

**`spec/ENV_SETUP.md` is narrower than AICD §17 on Production**, so the permission function is the wider of the two in three rows: qa ("Read telemetry only" against "None"), product_signal ("Read analytics only" against "None"), operations ("Deploy from tags, rollback, runbooks" against "Release pipeline trigger from tags (phase 4)"). `spec/ENV_SETUP.md` heads that column "Production (users' machines)", which for a local-first desktop product may be a different object rather than a disagreement. Nothing says so.

**`spec/ENV_SETUP.md` is wider in five places**, so the function refuses things the manifest issues credentials for: coder `Read` on CI, qa `Run` on CI, lead escalate on tickets, assistant create on tickets, coder plan and report.

**AICD §7 contradicts AICD §17 inside the methodology**, twice: §7 gives documentation "Reads: Specification, merged diffs, code" while §17's Specification cell is "Propose via pull request" with no read; and §7 gives the assistant "broad read access" while §17 gives it None on CI, Staging and Production. Implementing §17 with no implied privileges therefore refuses the documentation role read access to the specification, which is plainly odd for the role whose job is the specification. That one is a methodology-level decision and changes cells, not code.
