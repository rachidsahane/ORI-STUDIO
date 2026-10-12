#!/usr/bin/env bash
# make-repo.sh: turn this fixture into a git repository with history.
#
# Usage:  bash make-repo.sh <destination>
#
# <destination> must be absent or an empty directory, and must not be inside
# another git repository. The script prints the destination's absolute path
# and nothing else on standard output.
#
# WHY THIS EXISTS. This fixture is checked in as plain files because a `.git`
# directory cannot be committed inside another repository, and a submodule or
# a bundle would put a second object database under a repository whose secret
# scan and history checks read one. The end-to-end suite and any test that
# needs a repository with history call this script instead, so that every test
# gets the same repository, byte for byte, and nobody rebuilds it by hand.
#
# WHAT IT COPIES. Every file of the product, and nothing of the harness. The
# harness files are README.md, which lists the disagreements planted in this
# product, and this script. Copying the answer key into the repository a
# migration reads would let the migration read it. A `target/` directory and
# any `.git` directory beside this script are not copied either.
#
# WHAT IT BUILDS. Three commits on `main`, in this order, so that the
# documentation predates the CI and environment files and the history shows
# the environment file arriving after the first commit:
#
#   1. Cargo.toml, Cargo.lock, .gitignore, src/, tests/
#   2. docs/
#   3. .github/ and .env
#
# and one branch, `feature/carrier-webhooks`, one commit ahead of `main` and
# never merged, so that an M0 inventory has a branch with unmerged work to
# find. The branch is left checked out as `main` when the script ends. There
# are no tags: the baseline tag is the migration's to create. The commit
# identity and dates are fixed so that two runs produce the same commits.
#
# The script ends by checking that the working tree is clean. A file added to
# this fixture and not named in a commit below fails the script, which is the
# loud way to learn that the commit list is stale.

set -euo pipefail

die() {
    printf 'make-repo.sh: %s\n' "$*" >&2
    exit 1
}

[ "$#" -eq 1 ] || die "usage: bash make-repo.sh <destination>"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest="$1"

created=0
if [ -e "$dest" ]; then
    [ -d "$dest" ] || die "$dest exists and is not a directory"
    [ -z "$(ls -A "$dest")" ] || die "$dest is not empty"
else
    mkdir -p "$dest"
    created=1
fi
dest="$(cd "$dest" && pwd -P)"

if git -C "$dest" rev-parse --git-dir >/dev/null 2>&1; then
    # Leave nothing behind: remove the directory this run created.
    if [ "$created" -eq 1 ]; then rmdir "$dest"; fi
    die "$dest is inside a git repository already; a repository nested in one hides its files from the outer one"
fi

# Nothing from the machine's git configuration may change the result.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_CONFIG_NOSYSTEM=1

git_in_dest() {
    git -C "$dest" \
        -c user.name="Pat Okafor" \
        -c user.email="pat@parceltrack.example" \
        -c commit.gpgsign=false \
        -c core.autocrlf=false \
        -c core.hooksPath=/dev/null \
        "$@"
}

commit() {
    local when="$1" message="$2"
    shift 2
    git_in_dest add -f -- "$@"
    GIT_AUTHOR_DATE="$when" GIT_COMMITTER_DATE="$when" git_in_dest commit -q -m "$message"
}

# The product: every file except the harness files and build output.
while IFS= read -r -d '' file; do
    mkdir -p "$dest/$(dirname "$file")"
    cp -p "$here/$file" "$dest/$file"
done < <(cd "$here" && find . \( -name target -o -name .git \) -prune -o \
    -type f ! -path ./README.md ! -path ./make-repo.sh -print0)

git init -q "$dest"
git_in_dest symbolic-ref HEAD refs/heads/main

commit "2025-03-03T10:00:00+00:00" "Import parceltrack 0.3.0" \
    Cargo.toml Cargo.lock .gitignore src tests
commit "2025-03-10T10:00:00+00:00" "Write the first documentation" \
    docs
commit "2025-03-17T10:00:00+00:00" "Add CI, code owners and a local environment" \
    .github .env

git_in_dest checkout -q -b feature/carrier-webhooks
cat >"$dest/docs/webhooks-draft.md" <<'DRAFT'
# Carrier webhooks (draft)

Carriers will post status updates to parceltrack instead of being polled.
Not implemented. This branch holds the design only.
DRAFT
commit "2025-03-24T10:00:00+00:00" "Draft the carrier webhook design" \
    docs/webhooks-draft.md
git_in_dest checkout -q main

left="$(git_in_dest status --porcelain)"
[ -z "$left" ] || die "files were copied and not committed, so the commit list in this script is stale: $left"

printf '%s\n' "$dest"
