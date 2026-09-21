# CR-006: the lead reasons from branches and dispatches from main

Three tickets in a row found defects in the prompt the lead wrote them. Recorded together because the shape is one shape.

## 1. The lead used the unsafe idiom in the same breath as forbidding it

ORI-T-0019's ticket told the coder at length not to read `$?` through a pipe, citing CR-004, and offered two safe alternatives. **One of them does not work in this shell.**

```
$ echo "shell: $0; version: $ZSH_VERSION"
shell: /bin/zsh; version: 5.9
$ (true | true; echo "PIPESTATUS[0]='${PIPESTATUS[0]}'  pipestatus[1]='${pipestatus[1]}'")
PIPESTATUS[0]=''  pipestatus[1]='0'
```

`PIPESTATUS` is bash. In zsh the array is `pipestatus`, 1-indexed, so the bash spelling expands to the empty string.

**The lead then used it.** Three commands in that session ended `echo "push exit: ${PIPESTATUS[0]}"`, each printed `push exit:` followed by nothing, and the lead read all three as passing. The pushes did succeed, so nothing was lost, but a check that reported nothing was counted as a check. That is CR-004 exactly, committed inside the instruction warning about it.

**Fix, applied.** One idiom only: redirect to a file, capture `$?` on the next line. Two idioms where one is a trap is worse than one.

## 2. The lead cited records that were not in the tree the ticket was cut from

ORI-T-0019's ticket referred to escalation `E-0004` and to `CR-005`. The coder grepped and correctly reported that neither string appears in the repository. Both were real, written and pushed, in pull requests 25 and 26, and **neither was in `main`**, which is what its worktree was cut from.

This is **not** the class of rulings R15 to R24, where the record was never written at all. It is worse in one specific way: the record exists and the lead can point at it, while the agent doing the work correctly reports it absent and has no way to tell an unmerged record from an imagined one.

**Then it happened again, one ticket later, about this record.** ORI-T-0085's ticket cited "CR-002 through CR-006", and CR-005 and CR-006 were each reachable only from a different branch. The fix stated for defect 2 is "a ticket that cites an operational record states where the record lives: merged, or pull request N", and the ticket citing that fix did not do it. [[R30]] addresses the container; this addresses the citation.

## 3. Three assertions in three prompts, each stated as established, each unverified

The lead's tickets tell the coder to verify everything, including what the lead wrote. Three tickets, three false assertions:

| Ticket | The lead asserted | The truth | How the coder found it |
|---|---|---|---|
| ORI-T-0018 | (three defects in lead-written gate records) | confirmed defective | read the records |
| ORI-T-0019 | `E-0004` and `CR-005` exist | on branches, not in `main` | `grep` |
| ORI-T-0085 | `Spec: TESTING.md#1-test-types` | `spec/TESTING.md` section 1 is "Levels and what each proves"; no heading slugs to `#1-test-types` | read the file |
| ORI-T-0085 | "`ori-gates` cannot import `ori-core` either" | **false.** `spec/LLD.md` section 2: `ORC --> GATES`, `GATES --> STORE`, `STORE --> CORE`. `ori-gates` already links `ori-core` transitively | read LLD section 2 and both manifests |

The last is the most serious, because it asserted an *architectural constraint that does not exist* and would have justified a worse design. The coder took the lead's conclusion (a text parse, not an import) for its own reasons, which are sound: the import needs `crates/ori-gates/Cargo.toml`, outside its declared scope, and would make the branch fail to compile rather than fail a test until pull request 27 merges. It then rejected the lead's reason and put the real one on the record, so that nobody later reads "the architecture forbade it".

**That is the behaviour the fleet is supposed to have**, and it is three for three: ORI-T-0018's coder refused an instruction and was right, ORI-T-0019's found two prompt defects, ORI-T-0085's found two more and corrected the reasoning behind a third.

## The measurement

| | |
|---|---|
| Tickets run under the "verify what I wrote" instruction | 3 |
| Tickets that found a defect in the lead's prompt | 3 |
| False assertions found | 4 |
| Found by running something | 0 |
| Found by reading a file the lead did not read | 4 |

**Every one was found by reading, and every one would have been prevented by the same reading.** The cost of the check is seconds; the cost of the miss ranges from a wasted paragraph to a fabricated architectural constraint. This is the same result as CR-003, which measured the same thing about preconditions, and the conclusion is the same: the highest-yield control in this project is verification at the point of writing, not at the point of review.

**What follows for the methodology**, alongside the notes of CR-001, CR-003 and CR-005: AICD §12 tells the lead what to escalate and §32 tells it how instructions are layered, and neither says that a ticket is an artifact the lead is accountable for the accuracy of. Three tickets in, the ticket text is the least reliable document in this project and the only one nothing reviews.
