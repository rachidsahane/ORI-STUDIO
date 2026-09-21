# R30. Serially numbered operational records get one file each

| | |
|---|---|
| Ruling | R30 |
| Tier | 1 |
| Made by | The lead, at ORI-T-0085 review |
| Supersedes | Nothing. It changes the shape of `ops/calibration.md` and `ops/rulings.md` from here on |

## What went wrong

`ops/calibration.md` and `ops/rulings.md` are single files holding a serially numbered series, and the fleet edits them on parallel branches. That does not work, and it has now failed three times.

At ORI-T-0085 the coder reported that `ops/calibration.md` holds CR-001 to CR-004 in `main`, CR-005 only on `feat/ORI-T-0018-smoke`, and CR-006 only on `ops/batch-1-report`, so the copy on the second branch runs 001, 002, 003, 004, **006**. The lead checked it mechanically rather than accepting it:

```
$ git merge-tree --write-tree origin/feat/ORI-T-0018-smoke origin/ops/batch-1-report
CONFLICT (content): Merge conflict in ops/calibration.md
```

Two records, each appended to the end of the same file on a different branch, conflict. Either the merge stops, or a careless resolution drops one and nobody notices, because **no gate reads `ops/`** (CR-005).

## Why the previous diagnosis was wrong

`ops/rulings.md` already records this class once, for rulings R15 to R24, and diagnosed it as the lead failing to write records down. That diagnosis was wrong, or at least incomplete. The lead wrote these two records, committed them and pushed them. What failed is the container: **a serially numbered series in one file, edited by parallel work, with nothing checking it.**

The fix for a diagnosis of forgetfulness is a reminder, and reminders have not worked. The fix for a structural problem is structure.

## The ruling

1. From CR-006 and R30 onward, **each calibration record and each ruling is its own file**, under `ops/calibration/` and `ops/rulings/`, named for its identifier. This is the shape `ops/escalations/` has always had, and `ops/escalations/` has never had this problem.
2. `ops/calibration.md` and `ops/rulings.md` keep CR-001 to CR-005 and R1 to R29 where they are. **Nothing is moved.** Moving them would rewrite records to tidy them, which is the opposite of what an operational log is for. They become the archive of the series up to the change, and an index line is added to each when the next ticket that may touch them runs.
3. A record's identifier is claimed the way a module is claimed: written into its own file in the branch that creates it. Two branches cannot claim the same number without the file colliding by name, which git reports as an add/add conflict rather than silently merging two records into one.

## What this does not fix

It does not give `ops/` a gate. CR-005 proposes that to the methodology and it is still owed. This ruling makes the collisions loud; it does not make them impossible, and nothing here checks that a record's identifier is unique or that its cross-references resolve.
