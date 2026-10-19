# fixtures/new-product

Ticket ORI-T-0073. Spec anchor: `spec/TESTING.md` section 5 ("`fixtures/new-product` (empty, for J-01)") and journey J-01 in `spec/PRD.md` section 5.

This is an AICD product fixture, one of the four the end-to-end suite runs against. It is not a planted-defect harness: those live under `fixtures/planted/` and satisfy nothing in `spec/TESTING.md` section 5.

## What it is for

It stands for a product that does not exist yet: a directory with nothing in it, on which a person starts a product under AICD §23. The only thing that matters about it is that it is empty. Everything the engine produces from it (the `spec/` skeleton, `ops/`, the product database, the first events) must be visibly the engine's work, and it cannot be if the starting point already holds some.

| Who uses it | What they take from it |
|---|---|
| ORI-P1-001 (`spec/criteria/phase-1.md`) | The precondition, "Empty directory", for the action `ori init --name X`. This is the criterion the fixture exists for. |
| Phase 1 exit criteria (`spec/ROADMAP.md`) | "`ori init` on `fixtures/new-product` produces the specification skeleton, identities, and refuses launch with the correct missing list." |
| Journey J-01, and ORI-P1-002, ORI-P1-003, ORI-P1-004 as its later steps | The start of the journey. Their own preconditions ("PROJECT_BRIEF Draft, nothing else", "all foundation documents Approved except one", "fully ready") are states of a product the engine has already initialised and the suite has driven forward. The fixture supplies the empty beginning only; it holds none of those states. The criteria text does not name this fixture, so wiring them to it is the end-to-end suite's work (ORI-T-0077). |
| CI gate 8, forbidden-action test (`spec/CI_CD.md` section 1 item 8) | `scripts/gates.sh` and `scripts/setup-dev.sh` probe for the path `fixtures/new-product`, and `scripts/setup-dev.sh` is written to hand this directory to `scripts/forbidden-action-test.sh`. That runner is ORI-T-0029's and does not exist yet. Whatever it does, it must not write into this directory (see "Rules" below). |

## What it holds

This directory holds this `README.md` and nothing else. **`README.md` is documentation about the fixture and is not part of the product.** The product is everything in this directory except `README.md`, and that is nothing, so the product is an empty directory.

A directory with nothing in it cannot be committed, so the committed fixture is this file, and a test derives the empty product from it. The harness does not copy `README.md`.

## Turning it into the empty directory and repository

Never run the engine in this directory. Derive a fresh product directory per test, run in that, and delete it afterwards. Shell is shown; a Rust harness does the same three steps with `std::fs`.

```sh
FIXTURE=fixtures/new-product        # relative to the repository root
PRODUCT="$(mktemp -d)"              # a fresh directory per test

# 1. Take everything the fixture holds except README.md.
cp -a "$FIXTURE"/. "$PRODUCT"/
rm -- "$PRODUCT/README.md"

# 2. Assert it is empty. Do not assume it. If someone has added a file to
#    the fixture, this is where that is caught, instead of the test running
#    on a product that is no longer new.
[ -z "$(ls -A "$PRODUCT")" ] || { echo "fixtures/new-product is not empty" >&2; exit 1; }
```

That is form A, **the empty directory**: no `.git`, nothing. It is ORI-P1-001's precondition word for word, and the form to use unless the criterion under test needs a repository.

When the engine or a criterion needs a repository, the same empty directory is turned into one:

```sh
# Form B, an empty repository: initialised, no commits.
git -C "$PRODUCT" init --initial-branch=main

# Form C, an empty repository with a first commit, if something needs a HEAD.
git -C "$PRODUCT" -c user.name=fixture -c user.email=fixture@example.invalid \
    commit --allow-empty -m "chore: empty product"
```

`spec/API_SPEC.md` gives `products.create` a `repo_path` and says nothing about whether that path must already be a repository, or have a commit. This fixture does not decide that: forms A, B and C are all derived from the same empty content, and the end-to-end suite (ORI-T-0077) picks the form each criterion needs. If a criterion turns out to need a form that is not one of these three, the suite builds it on top of the derived directory and does not add files here.

## Why it is not a repository of its own in the tree

`spec/TESTING.md` section 5 says each fixture is "a repository of its own". In the tree it cannot be one: git does not track the contents of a `.git` directory inside another repository. Adding a nested repository that has a commit records only a bare pointer to that commit (mode 160000, no `.gitmodules`), which a clone does not restore, and adding one with no commit is refused outright. So the committed fixture is the *source* of a repository, and the repository itself is made by the test (forms B and C above), in the same way `fixtures/planted/gate-13/prove.sh` builds its planted repositories at run time. A submodule would pin another repository and a remote, and a git bundle is an opaque binary that review and the secret scan cannot read; neither is used, and for an empty product neither would hold anything to preserve.

## What it deliberately does not contain

- No `spec/` and no `ops/`. Creating them is what ORI-P1-001 asserts `ori init` does.
- No `.git`, no `.gitignore`, no `.gitkeep`. A placeholder file would be the product's first file.
- No source code, manifest, workflow or CI configuration.
- No `.env*` file and no credential of any kind, real or fake. The tracked env file with fake keys belongs to `fixtures/migrated-with-drift`.
- No documents in any state. Document state (Missing, Draft, Approved) is recorded by the engine in the product database (`spec/DATA_MODEL.md`, Document), not in file content.
- No expected-output files. What `ori init` must produce is stated in `spec/criteria/phase-1.md`, and a copy here would be a second statement to keep in step with it.

The existing-product cases (documents that disagree with code, an inert workflow) are `fixtures/migrated-with-drift` and `fixtures/inert-gate`, under AICD §24 and the liveness gate. They are not this.

## Rules for changing it

1. This directory holds `README.md` and no other file. A change that adds one stops being this fixture; add a new fixture instead.
2. A test never writes into it and never runs an engine process with it as the working directory. Derive a copy.
3. A harness asserts the derived directory is empty before it runs (step 2 above). One that assumes it would run `ori init` on a product that is not new and still report on ORI-P1-001.
