# CR-007: sweeping for one defect class on purpose, and what the sweep itself measured

For five rounds this project found the same defect class two or three at a time, incidentally, never on purpose: **a record that states something other than the truth**, and its second face, **a check that runs and verifies nothing** (AICD §39). Round 4 went looking for all of it at once.

## The harness

Six lenses over six surfaces, run in parallel: `spec/`, `ops/`, doc comments in `crates/`, vacuous checks, cross-document contradictions, and the gate machinery. Each lens was required to produce a runnable command and its observed output, not a reading. Every finding was then handed to an independent agent whose instructions were to **refute** it and to default to refuted when unsure.

38 agents, 955 tool calls.

## The numbers, including the one that is about the harness

| | |
|---|---|
| Raised | 32 |
| Survived refutation | 32 |
| Refuted | 0 |
| Refutation rate | **0 per cent** |

**A 0 per cent refutation rate is a finding about the instrument, not a clean bill of health**, and it is recorded here as one. CR-001 measured 130 raised and 42 surviving, a 68 per cent refutation rate, and concluded that a lens refuting above 90 per cent is asking too broad a question. This is the same measurement failing from the other end.

Two readings, and the evidence does not fully separate them:

1. **The sweep prompts demanded evidence that the verifier could re-run**, which is a real difference from CR-001's review, where lenses were asked for judgements. A finding that arrives with a command attached is harder to refute because it has already been tested once.
2. **The refuters were not adversarial enough.** They shared a model and a prompt shape with the finders, which is the separation AICD §7 exists to enforce and which this harness did not enforce.

The lead spot-checked four findings by hand rather than accepting the rate. Three held exactly. One, the `setup-dev.sh` `Cargo.lock` finding, is **partly wrong as written**: its stated claim is that `Cargo.lock` is not tracked, and `git ls-files --error-unmatch Cargo.lock` exits 0, so it is. What survives of it is narrower and still real: the check is labelled "Cargo.lock is tracked" and what it actually runs is `git check-ignore`, so it reports on ignoring and not on tracking, and an untracked-but-unignored lockfile would pass it.

So the true refutation rate is **not zero**, and the right conclusion is that this harness cannot measure its own rate. **The next audit must give the refuters a different model from the finders**, which is what AICD §7 asks for and what CR-001 had by construction.

## What it found anyway

32 findings survived, 12 of them rated high. The sharpest three:

**`ops/gates/gate-2.md` cited the wrong run, and the gate was moved to Installed on it.** The record read "Run 35598542173, this ticket's own pull request: thirteen jobs green including `gate-2-proof`." That run is **ORI-T-0013's** pull request: twelve jobs, `gate-1-proof`, and no `gate-2-proof` at all. The correct run is **35608800130**, which has exactly thirteen jobs including `gate-2-proof`. `ops/gates/gate-1.md` cites 35598542173 correctly, so the lead copied gate 1's identifier into gate 2's file and paired it with gate 2's own accurate description.

This is the sharpest instance of the class yet recorded, because **a reader who followed the citation would have found a run that disproves the sentence citing it**. It is also exactly what CR-005 proposed to the methodology, met one step worse than predicted: CR-005 said a record citing a run id that does not resolve is an uninstalled gate. This one resolves, to the wrong run.

**`ops/gates/gate-7.md` claimed evidence in a commit that was never made.** Its "What is not established" list said the visibility clause was "Established by the run of this pull request, recorded in a follow-up commit." No such commit exists, and no gate 7 run identifier appears anywhere under `ops/`. A false entry in the list of things a gate has *not* established is the most expensive place to put one.

**A test whose name promises coverage checks only its own consistency.** `ori_p1_033_every_refusal_this_crate_can_make_is_covered_by_these_tests` in `crates/ori-core/src/error.rs` compares the length of a deduplicated tag list against the length of the list it came from. `refusal_tag`'s wildcard-free match does force every `RefusalKind` variant to be listed *there*, so the compiler catches a new variant. It does not force `every_refusal()` to **produce** one of each, so a variant present in the match and absent from the list passes. **The lead reviewed this test, praised it in the pull request body as the project's anti-"present but reporting nothing" device, and was wrong.**

## What the sweep is worth

Five rounds of incidental discovery produced roughly twenty instances of this class. One pass produced thirty-two, twelve of them high, in twenty-two minutes of wall clock.

The lesson is not that agents find defects. It is that **this class is invisible to every gate in the repository and is therefore bounded only by how hard somebody looks for it.** `ops/` is read by no gate (CR-005). Doc comments are read by no gate. Cross-document agreement is read by no gate. The only control that has ever caught an instance is a reader who was asked to look, and the only variable is how many readers and how systematically.

## What it recommends to the methodology

Alongside the notes of CR-001, CR-003, CR-005 and CR-006:

1. **A periodic adversarial sweep for this class is a control, not a cleanup.** It should be scheduled, scoped by lens, and measured by refutation rate like any review under AICD §30.
2. **The refuters must differ from the finders in model, not only in prompt.** AICD §7 already requires this of review; this record is what it looks like when the requirement is skipped.
3. **An evidence citation is checkable and nothing checks it.** Run identifiers, test names, file paths and ticket identifiers in operational records either resolve or they do not, and the gate 2 defect shows that resolving is not enough: a citation must resolve **to the thing described**.
