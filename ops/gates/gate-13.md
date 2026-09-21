# Proof: CI gate 13, the commit-trailer gate

| | |
|---|---|
| Gate | CI_CD section 1, item 13 |
| Command | `bash scripts/gates.sh --commit-trailers` |
| Ticket | ORI-T-0017 |
| **State** | **Proven, not Installed** |
| Written by | The lead, from the coder's verified evidence (ruling R25) |

Gate 13 did not exist before this ticket: nothing in `.github/workflows/ci.yml` read a commit message, `scripts/gates.sh` reported it unavailable, and the gate was a line in a specification. This ticket builds it and proves it. What is owed for Installed is at the end.

## The defect this gate exists for

Git parses trailers out of the **last paragraph** of a commit message and out of nothing else. A message carrying `Ticket:` and `Spec:` in an earlier paragraph, with `Co-Authored-By:` alone at the end, has both lines where any human and any regex finds them, and `git interpret-trailers --parse` returns neither.

That is not hypothetical. **It is the shape the lead shipped in the first commit this project made under CONVENTIONS**, and it is planted as `fixtures/planted/gate-13/messages/trap-trailers-in-earlier-paragraph.txt`. The prover prints the contrast on every run:

| asked | answer |
|---|---|
| `grep -E '^Ticket: ORI-T-[0-9]{4}$'` | **MATCHES** |
| `grep -E '^Spec: [A-Za-z0-9_/-]+\.md#...'` | **MATCHES** |
| `git interpret-trailers --parse` | `Co-Authored-By:` and nothing else |
| `%(trailers:key=Ticket,valueonly)` | *(empty)* |
| gate 13 | **exit 1**, `TICKET_UNPARSED SPEC_UNPARSED` |

**A regex-based gate 13 passes that commit** and reports the traceability chain of AICD §13 intact while every consumer of those trailers sees nothing. That is AICD §39's "present but reporting nothing" sitting inside the gate that guards the audit trail. So every trailer question is asked of `%(trailers:only=true,unfold=true)`, which is git's own parser.

A regex is used in exactly one direction: when git reports no `Ticket:` trailer and the message nevertheless contains a `Ticket:`-shaped line, the refusal is `TICKET_UNPARSED` rather than `TICKET_MISSING`, so the author is told the line is in the wrong paragraph instead of being told it is absent. There is no path from it to a pass.

## The other half is enumeration, and it is where this gate would have died quietly

"Every commit on the PR" is a set. **A gate that computes the empty set and reports success has checked nothing while looking exactly like a gate that checked everything and found nothing wrong.**

| event | the set | if empty or unnameable |
|---|---|---|
| `pull_request` | `merge-base(base, head)..head`, cross-checked against `pull_request.commits` | **exit 2** |
| `merge_group` | `base_sha..head_sha`, the rebased commits | **exit 2** |
| `push` to main | `before..after` | **exit 2** |
| `workflow_dispatch` | the single checked-out commit, stated as a smaller claim | **exit 2** |
| no event, a desk | the branch's own commits against `main` | **exit 3**, reported `blocked` |

Eleven planted states were run and **none printed `PASSED`**: a shallow clone, an empty range, a commit count GitHub and the gate disagree about, an unreadable payload, a payload naming no SHAs, an event with no rule, a created ref, a push that moved nothing, an empty merge group, a branch with no commits of its own, and a repository with no base branch.

**`fetch-depth: 0` is load-bearing.** `actions/checkout` defaults to 1, and in that clone the SHAs the payload names are not objects the repository holds. The gate refuses and names the fix rather than enumerating what is present, so deleting that line turns the job red instead of turning the gate off. The prover reads the value out of `ci.yml` on every run.

Enforced structurally: every `refuse` and `nothing` row carries `!^  PASSED\.` as a required negative signature, and the prover refuses to run if one stops carrying it.

## Passes clean, fails on the planted defect

46 planted inputs, each run once, exit captured without a pipeline: 11 pass, 24 fail, 9 refuse, 2 nothing checked. 0 direct disagreements, 46 flipped, complementary on every row, 0 unattributable.

| planted class | rows | gate 13 |
|---|---|---|
| clean messages (body, no body, no scope, breaking `!`) | 4 | exit 0 |
| trailers absent, three distinguishable states | 3 | exit 1, `*_MISSING` |
| trailers present, value wrong | 6 | exit 1, `*_VALUE` / `_DUPLICATE` / `_CASE` |
| subject shape and vocabulary | 6 | exit 1, `SUBJECT_*` |
| **the trap, three variants** | 3 | **exit 1, `*_UNPARSED`** |
| a defect at the first commit with a clean head | 1 | exit 1, `checked 2, 1 refused` |
| a merge commit inside the range | 1 | exit 1, no exemption |
| a range whose base is a merge commit | 1 | exit 0, base never read |
| eleven ways for the set to be empty | 11 | exit 2 or 3, never 0 |

## Merge commits, and why there is no exemption

`main` carries one merge commit, `ffab3fd`, from before rebase-only merges were configured. It has no trailers and never will, because CONVENTIONS forbids rewriting history. It is never enumerated, and **not because it is exempt**: it is an ancestor of the base of every range this gate can build, and a range excludes its base. Both halves are proven.

The exemption a reader might expect is refused deliberately. "Has two or more parents" exempts exactly the case that should fail, because a merge commit inside a pull request means somebody merged `main` into their branch instead of rebasing, which CONVENTIONS forbids. "The subject starts with `Merge pull request`" is a regex over the message, which is what this gate exists to reject.

## What is deliberately not enforced

1. **That the `Spec:` anchor resolves.** The document half is observed and printed and never judged. Both branches pass: a commit citing a document in the tree, and one citing a document that is not. Nothing here resolves a markdown anchor yet, and a commit whose cited document is renamed later could never be repaired, because history is not rewritten.
2. **Subject length, capitalisation, trailing full stop.** Neither Conventional Commits v1.0.0 nor CONVENTIONS states them.
3. **The type vocabulary is a repository decision, not a standard.** `feat fix docs ci chore`, read off what the 22 conventional commits on `main` actually do. The cost is stated rather than hidden: the first legitimate `refactor:` or `test:` commit fails this gate, and the fix is one word in a ticket.

## The proof is a job

`gate-13-proof` runs on every pull request, following the shape the three earlier gates paid for. One change: gate 7 reports any non-complementary row as "the harness cannot tell agreement from disagreement". With four verdicts that is wrong most of the time, and on the first adversarial run it blamed the harness for a fault entirely in the gate, which is the conflation `ops/gates/gate-1.md` already records paying for once. The two are now separated: agreeing in **both** passes is impossible for a comparator that is comparing, so it is a harness failure; disagreeing in **both** means the gate answered a third value, which is a gate failure and is named as such.

## Seen to fail

Eight weakenings applied to a copy of the tree; the prover caught every one.

| weakening | prover |
|---|---|
| the gate reads trailers with a regex | **exit 1**, 8 of 46 disagree, all three trap rows flip fail to pass |
| an empty set reported ok, floor removed | **exit 1** |
| `continue-on-error: true` on `gate-13` | exit 2 |
| `fetch-depth: 1` on the checkout | exit 2 |
| `gate-13-proof` renamed | exit 2, `misnamed` |
| `gate-13` dropped from `ci`'s `needs:` | exit 2 |
| the trap message repaired by deleting one blank line | exit 2 |
| a planted message deleted | exit 2 |

## What is not established

1. **The visibility clause of AICD §14.** Owed: a green `gate-13-proof` on a real run, and an observed red `gate-13` on a real non-conforming commit.
2. **That a real runner hands this gate the commits the harness hands it.** The payloads are written by the prover. GitHub's side of the contract cannot be exercised here; the cross-check against `pull_request.commits` turns a disagreement there into a refusal rather than a quietly short range.
3. **That gate 13 has ever failed on a commit in this repository's history.** Every commit since the baseline carries both trailers where git reads them. What is proven is that it would not pass one that did not.
4. **That a failing `ci` blocks a merge.** `ci` is not a required status check. Four proofs now record this as owed.
5. **That the job running the prover stays live.** The recursion does not terminate inside the file; that path is tier 2 for this reason.

## Rollback

`git revert`. Gate 13 ceases to exist: no job, no local runner verdict, and nothing may cite it.
