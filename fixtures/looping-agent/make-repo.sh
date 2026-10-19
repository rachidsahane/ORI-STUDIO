#!/bin/sh
#
# make-repo.sh: turn this directory into a repository of its own.
# Fixture ORI-T-0076, spec/TESTING.md section 5.
#
#     sh make-repo.sh <destination>
#
# This directory is committed inside the Ori Studio repository, which cannot
# hold a nested .git. So the fixture is plain files, and a test that needs a
# repository makes one: this script copies every file of this directory to
# <destination> and commits them there as the first and only commit of a new
# repository on branch `main`. <destination> must not exist yet, or must be
# empty. It prints nothing on success.
#
# The result does not depend on the machine it was made on, so a test can
# assert against it: the identity, the dates and the message are fixed, and no
# hook, template, signing key or line-ending setting from the caller's own git
# configuration is applied. The same bytes give the same commit hash.
#
# POSIX sh. On Windows it needs the `sh` that Git for Windows ships, like
# agent.sh; a test that cannot run a shell makes the same repository with the
# same git commands from its own code (see README.md).

set -eu

[ "$#" -eq 1 ] || {
    echo 'usage: sh make-repo.sh <destination>' >&2
    exit 64
}
dest=$1
here=$(cd "$(dirname "$0")" && pwd)

if [ -e "$dest" ]; then
    [ -d "$dest" ] || {
        echo "make-repo.sh: $dest exists and is not a directory" >&2
        exit 1
    }
    [ -z "$(ls -A "$dest")" ] || {
        echo "make-repo.sh: $dest is not empty" >&2
        exit 1
    }
fi

mkdir -p "$dest"
cp -R "$here/." "$dest/"

# `--template=` so that no hook from the caller's git templates is installed;
# the identity and dates are fixed so that the commit hash is reproducible.
git init --quiet --template= "$dest"
git -C "$dest" symbolic-ref HEAD refs/heads/main
git -C "$dest" config user.name 'looping-agent fixture'
git -C "$dest" config user.email 'fixture@looping-agent.invalid'
git -C "$dest" config core.autocrlf false
git -C "$dest" config commit.gpgsign false
git -C "$dest" add -A
GIT_AUTHOR_DATE='2026-01-01T00:00:00+00:00' \
    GIT_COMMITTER_DATE='2026-01-01T00:00:00+00:00' \
    git -C "$dest" commit --quiet --no-verify -m 'fixture: looping-agent'
