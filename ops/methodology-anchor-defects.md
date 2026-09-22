# Methodology anchor defects: AICD v0.3

| | |
|---|---|
| Ticket | ORI-T-0006, tier 0 |
| Subject | `methodology/AICD_Methodology_v0.3.html`, 1519 lines, as shipped in this project |
| Spec anchor | AICD §39 (lessons from the first application), AICD §32 (agent instruction design), AICD §14 (gate installation) |
| Method | Mechanical extraction of every `<h1>`, `<h2>`, `<h3>`, every `id` attribute and every `href="#..."`, then a reading of every prose cross-reference of the form "section N" against the section it names |
| Status | Report only. This ticket changes no file in `methodology/`. |
| Carries to | The methodology repository, for the methodology's author |

## 1. The problem and its consequence

The heading anchors in `AICD_Methodology_v0.3.html` do not match the section numbers they sit on. Sections 1 to 22 and 32 to 40 are correct (`s1` to `s22`, `s32` to `s40`); there is no element with id `s23`; sections 23 to 30 carry `s24` to `s31`, an off-by-one; and `s31` is used twice, first on section 30 and again on section 31, so section 31 has no anchor of its own and every link to `#s31` resolves to section 30 (a browser resolves a duplicate id to the first occurrence in document order). The consequence is that citation by section number and title is safe, because the numbering in the headings, in the table of contents and in the prose is internally consistent, while citation by anchor is unsafe for nine of the forty sections and fails silently: a reader following `#s24` lands on a real section that is not the one cited, with nothing to indicate the miss. Subsection citation has no anchor at all, safe or otherwise, because no `<h2>` or `<h3>` in the document carries an id.

| Citation form | Status today |
|---|---|
| Section number, "section 23", "AICD §23" | Safe. Numbering is correct and consistent across headings, table of contents and prose, with the two exceptions in table 3.3. |
| Section number plus title | Safe, and the only form that survives the fix unchanged. |
| Subsection number, "AICD §24.2", "A.5" | Safe as text. The subsection numbering is sound: 24.1 to 24.8 sit inside section 24, A.1 to A.5 inside appendix A. No anchor exists for any of them (D-14). |
| Anchor, sections 1 to 22 | Safe. `#s1` to `#s22` resolve correctly. |
| Anchor, sections 23 to 30 | Unsafe. Off by one. `#s23` resolves to nothing; `#s24` to `#s31` resolve one section early. |
| Anchor, section 31 | Unreachable. No unique id exists. |
| Anchor, sections 32 to 40 | Safe. |
| Anchor, appendices A, B, C | Resolvable but not guessable: the ids are `a1`, `a2`, `a3`, not the letters. |
| Anchor, any subsection (24.1 to 24.8, A.1 to A.5) | Unavailable. No subsection heading carries an id. |

## 2. What was examined

| Element | Count | With an id |
|---|---|---|
| `<h1>` total | 54 | 43 |
| `<h1>` numbered section headings (1 to 40) | 40 | 40 attributes, 39 distinct ids |
| `<h1>` appendix headings (A, B, C) | 3 | 3 (`a1`, `a2`, `a3`) |
| `<h1>` part titles and front matter | 11 | 0 |
| `<h2>` | 76 | 0 |
| `<h3>` | 24 | 0 |
| `<h4>` | 0 | 0 |
| Internal links `href="#..."` | 53 | 52 distinct targets |

Every id in the file: `part1` to `part8`, `appx`, `author`, `s1` to `s22`, `s24` to `s40`, `a1` to `a3`. Fifty-three id attributes, fifty-two distinct values.

## 3. Defects

### 3.1 Anchor defects on numbered sections

| # | Anchor as it is | Line | Section it sits on | Title | Should be |
|---|---|---|---|---|---|
| D-01 | `s23`, absent from the file | none | 23 | Starting a new product under AICD | Supplied by applying D-02 |
| D-02 | `s24` | 837 | 23 | Starting a new product under AICD | `s23` |
| D-03 | `s25` | 910 | 24 | Migrating an existing product into AICD | `s24` |
| D-04 | `s26` | 1033 | 25 | Memory service: implementation model | `s25` |
| D-05 | `s27` | 1060 | 26 | Operating several products with one team | `s26` |
| D-06 | `s28` | 1073 | 27 | Security and compliance in depth | `s27` |
| D-07 | `s29` | 1105 | 28 | The fleet dashboard | `s28` |
| D-08 | `s30` | 1126 | 29 | Evolving the methodology: the extension process | `s29` |
| D-09 | `s31`, first occurrence | 1145 | 30 | Calibration procedures | `s30` |
| D-10 | `s31`, second occurrence, duplicate | 1181 | 31 | Simulation personas and finding quality | `s31`, unique once D-09 is renumbered |

D-10 is the only duplicate id in the file. It is also the only invalid HTML among these defects; D-01 to D-09 are well-formed and wrong, which is why nothing has flagged them.

### 3.2 Table of contents links affected

The table of contents, lines 118 to 183, links to the ids as they are, so it is self-consistent with one exception.

| # | Entry | Target | Where it lands | Should be |
|---|---|---|---|---|
| D-11 | "31. Simulation personas and finding quality", line 166 | `#s31` | Section 30, Calibration procedures | `#s31` after D-09 is renumbered |

The eight entries for sections 23 to 30, lines 154, 155 and 158 to 163, point at `#s24` to `#s31` and land correctly today. They must be renumbered in the same edit as D-02 to D-09 or they will break.

### 3.3 Prose cross-references that name the wrong section

Sixty-one cross-references of the form "section N" were read against the section they name. Fifty-nine are correct. Two are not.

| # | Reference as it is | Line | In section | What it describes | Should be |
|---|---|---|---|---|---|
| D-12 | "Section 30 describes the per-product view" | 1070 | 26, Operating several products with one team | The per-product dashboard view, which is section 28, The fleet dashboard. Section 30 is Calibration procedures. | section 28 |
| D-13 | "as the open questions in section 23 are explored" | 1499 | Colophon, after appendix C | The open questions, which are section 40, Open questions and next explorations. Section 23 is Starting a new product under AICD. | section 40 |

Neither is a consequence of the anchor drift: the prose everywhere else uses the correct numbers, including throughout Parts VI to VIII where the ids are wrong. The numbering is the sound index in this document; the ids are not.

### 3.4 Structural gaps, not wrong but not usable

| # | Observation | Consequence |
|---|---|---|
| D-14 | No `<h2>` or `<h3>` carries an id, 100 headings in total, including the numbered subsections 24.1 to 24.8 and the templates A.1 to A.5 | Subsection citation cannot be anchored. `AICD appendix A.5` and `AICD §24.3`, both used in working artifacts, resolve as text against the numbering and have no anchor target. |
| D-15 | Appendix ids are ordinals, `a1`, `a2`, `a3`, while the headings are letters, A, B, C | An anchor cannot be derived from a citation. "Appendix A" suggests `#a`, `#aA` or `#appendixA`, none of which exists. |
| D-16 | The appendices wrapper uses `id="appx"` while the eight parts use `part1` to `part8` | Breaks the one pattern a generator or a reader would infer. |
| D-17 | The nine part-title headings carry no id; the id sits on the wrapping `<div class="part">` | A heading-level extraction finds no anchor for any part. Functionally harmless, worth knowing. |
| D-18 | `<h1>Contents</h1>` at line 119 has no id | The table of contents cannot be linked to. |

## 4. The fix

The renumbering is the "Should be" column of table 3.1. Applied, it makes the rule "the anchor for section N is `sN`" true for all forty sections with no exceptions.

Procedure:

1. Edit the eight section headings at lines 837, 910, 1033, 1060, 1073, 1105, 1126 and 1145 together with the eight table of contents links at lines 154, 155 and 158 to 163, in one commit. A heading and the table of contents entry that points at it are one edit, not two: the entry's target changes with the id it names. The entry for section 31 at line 166 is not touched; D-09 makes its existing target unique.

2. Apply the eight renames in ascending order, line 837 first and line 1145 last. The reasoning, because the conclusion is easy to get backwards:

   - Every one of the eight ids moves down by one. `s24` becomes `s23`, `s25` becomes `s24`, and so on to `s31` becoming `s30`.
   - A rename is collision-free only when its target id is free at the moment it is applied.
   - At the start exactly one target in the range is free: `s23`, which no element carries (D-01). Every other target is held by the element one row above it in table 3.1.
   - So exactly one first step is collision-free, `s24` to `s23` on line 837, and it frees `s24` for line 910. Each step frees the target of the next.
   - The argument repeats to the end. Ascending is collision-free, and it is the only collision-free order, because at every intermediate state exactly one of the remaining renames has a free target.
   - Descending fails at its first step. Renaming `s31` on line 1145 to `s30` collides with the live `s30` on line 1126; renaming that to `s29` collides with the live `s29` on line 1105; and so on for all eight. Descending does not make room, it consumes it.

   | Step | Line | From | To | Target is free because |
   |---|---|---|---|---|
   | 1 | 837 | `s24` | `s23` | nothing carries `s23` (D-01) |
   | 2 | 910 | `s25` | `s24` | step 1 vacated it |
   | 3 | 1033 | `s26` | `s25` | step 2 vacated it |
   | 4 | 1060 | `s27` | `s26` | step 3 vacated it |
   | 5 | 1073 | `s28` | `s27` | step 4 vacated it |
   | 6 | 1105 | `s29` | `s28` | step 5 vacated it |
   | 7 | 1126 | `s30` | `s29` | step 6 vacated it |
   | 8 | 1145 | `s31` | `s30` | step 7 vacated it |

   Two qualifications. First, the file already holds one duplicate before any edit, `s31` on lines 1145 and 1181 (D-10). The ascending order introduces no new duplicate, and step 8 removes the existing one, leaving line 1181 as the unique `s31`. Second, the order constrains id attributes only. The eight table of contents values are `href` targets, not ids, so a repeated value among them is legal at any intermediate state. The order matters wherever a rename can see the result of the one before it: a sequence of single edits, a chain of substitutions in one script, a procedure that validates the file between steps. A true simultaneous mapping of all eight values has no intermediate state and no ordering constraint, but ascending is safe in every case and costs nothing.

3. Correct the two prose references, D-12 and D-13.

4. Optional, and recommended if the document will be cited by section and subsection: give every `<h2>` and `<h3>` an id derived from its number where it has one (`s24-1` to `s24-8`, `a1-1` to `a1-5`) and from its slug where it does not. This is additive and breaks nothing.

5. Optional: rename `appx` to `part9` for consistency with D-16, and add an id to the Contents heading for D-18. Both are cosmetic; `appx` is already linked from the table of contents and works.

**Warning: this breaks existing external links, silently.** Every link that today points at `#s24` through `#s31` will, after the fix, resolve to a different section than it did before, and the section it lands on will be a real one. A bookmark, a citation in another repository, a slide, or a PDF outline entry that names `#s26` currently opens section 25 and will afterwards open section 26. Nothing will report the change. There is no way to preserve the old targets while fixing the numbering, because the ids being reassigned are the ones the old links use. Publish table 3.1 as the mapping alongside the fix so anyone holding an old link can correct it.

The fix changes no methodology content. AICD §29 gives no version increment to parameter and tooling changes, a minor increment to an adopted extension and a major increment to a core change; an anchor correction is none of the three, so it does not move the version number. Record it as an erratum line in appendix C, Document history, so that a reader can tell a v0.3 file with correct anchors from one without.

## 5. What this project does in the meantime

**The index.** ORI-T-0005 generates `methodology/sections.json`. Under the lead's ruling R1 it indexes the forty numbered sections, the three appendices, the subsections §24.1 to §24.8 and the templates A.1 to A.5. Those are every numbered heading the document carries: section 24 is the only section with numbered subsections, appendix A the only appendix with numbered items. The generator reads the numbers and the titles, not the `id` attributes, so no defect in table 3.1 enters the index.

When this report was written, `methodology/` contained one file, `AICD_Methodology_v0.3.html`, and `sections.json` did not exist. **It does now**: ORI-T-0005 built it in batch 1 (`ops/phase-1-backlog.md`, batch 1's ORI-T-0005 row).

**What gate 9 accepts.** Per R1: a section (`AICD §<n>`), an appendix (`AICD appendix A`, `B`, `C`), a subsection of section 24 (`AICD §24.1` to `§24.8`) and a template of appendix A (`A.1` to `A.5`, spelled either `AICD appendix A.5` or `AICD A.1`, both of which occur). A reference to a subsection that carries no numbered heading fails the gate. `spec/CONVENTIONS.md` line 37 states the requirement that every reference resolve; it does not state that it currently does, and this report does not either.

**What this repository cites today.** Not one form. Counted over every `.md` file in the tree on 2026-09-21:

| Form | Occurrences | Where, when few |
|---|---|---|
| `AICD §<n>`, section granularity | 93 | throughout `spec/`, `ops/`, `CLAUDE.md` |
| `AICD §24.<n>`, subsection | 4 | `spec/ENV_SETUP.md`:86; `ops/phase-1-backlog.md`:260, :261, :342 |
| `AICD appendix A.<n>` or `AICD A.<n>`, template | 6 | `CLAUDE.md`:22; `spec/criteria/phase-1.md`:3; `ops/tickets/ORI-T-0000-batch-0.md`:3; `ops/phase-1-backlog.md`:10 (twice), :64 |
| `AICD appendix <A\|B\|C>`, appendix as a whole | 3 | `spec/README.md`:21; `ops/phase-1-backlog.md`:64, :87 |
| Anchor or URL fragment | 0 | none anywhere, established by searching every `.md` and `spec/design/Ori Studio.html` for `#s<n>` and for the file name followed by `#` |

Ten of those references are finer than section granularity. All ten name a heading that exists, so all ten are inside the set R1 puts in the index. Two spellings of the template form are in use, and one reference is a range (`AICD A.1 to A.5`, `ops/phase-1-backlog.md`, batch 1's ORI-T-0009 row).

**Gate 9 status.** CI_CD gate 9 stays `Defined` and is not `Installed`. Two things hold it there, and they are different in kind:

| What holds it | Kind |
|---|---|
| The operator's ruling recorded at `ops/phase-1-backlog.md`, the paragraph headed "Gate 9 (citation) is deliberately absent from batch 1": the gate waits on the methodology HTML anchor fix and a regenerated `sections.json`. The ticket is ORI-T-0047 in batch 7, and line 191 records its precondition as unverified. | A decision, not a technical fact. The index does not read `id` attributes, so the defects in table 3.1 do not reach it. If gate 9 is later extended to check anchors or fragments, they reach it directly. |
| AICD §14: a gate enters service only after a demonstration on a planted defect, passing on a clean tree and failing on the planted defect, recorded in the ticket that installed it. | A methodology rule. It applies to gate 9 whatever the state of the anchors. |

AICD §39 names the defect class this avoids, "present but reporting nothing": a checker that exits successfully on every input reads as protection in every document that cites it while protecting nothing. A gate 9 marked `Installed` before its planted-defect demonstration would be exactly that.

**What is not established, and what would establish it.**

| Open | What would settle it |
|---|---|
| Whether gate 9 reads HTML under `spec/`, or markdown only. It decides whether the twelve references in section 6 are gate failures or out of scope. | Open escalation 4, `ops/phase-1-backlog.md`, open escalation 4 in the escalations table. The operator's ruling on it. |
| Whether gate 9 parses range citations (`AICD A.1 to A.5`, and any `§24.1 to §24.8` written later) or reads them as two separate references. | ORI-T-0047's rule set, which `ops/phase-1-backlog.md`, batch 7's ORI-T-0047 row records as undecided. |
| Whether the methodology's author accepts the renumbering in section 4, and when. Every date in ORI-T-0047's chain depends on it. | A reply on this report. Until then the precondition is unverified, and AICD §39 requires a precondition to be verified at approval time, not assumed. |

## 6. Known issue, out of scope for this report

`spec/design/Ori Studio.html` contains twelve distinct fabricated AICD subsection references, fourteen occurrences (§11.7, §12.3, §17.3 twice, §17.4, §17.9, §23.1, §23.4, §23.5, §26.1, §27.2, §39.1 twice, §39.3), inside display copy in a visual mockup. None of the sections they name has numbered subsections, so under R1 none resolves and each would fail gate 9 if the gate's scope includes HTML under `spec/`, which is the undecided question above. They are defects in this project's own design artifact rather than methodology anchor defects, and they are tracked as open escalation 4 in `ops/phase-1-backlog.md`.


---

## A correction to this report's own references

Found by the round 4 audit ([[CR-007]]). Five references in this file pointed into `ops/phase-1-backlog.md` **by line number**. Every one was correct when this report merged. A later commit inserted twelve lines above them, and the reference that mattered most, the operator's gate 9 ruling, then landed on an unrelated paragraph about ORI-T-0018.

Nothing moved and nothing was deleted. **A line number is a reference to a position, and a position is not a fact about the thing being referenced.** All five now cite by the row or heading they mean, which survives an insertion.

ORI-T-0086 reached the same conclusion from the other direction and its ticket said so in as many words: "Do not name a line number: line numbers rot, and this ticket exists because a record rotted." That instruction was given to a coder for a doc comment, and this file, written by the lead, was rotting the same way at the same time.
