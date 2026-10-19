#!/bin/sh
# check.sh: the one command this product's CI runs, and the whole of its
# "tests". It checks that VERSION is a major.minor.patch number and nothing else.
#
# It passes. The fixture's defect is not in this script: the workflow that
# would run it never fires, so the passing check is never asked for.
set -eu

version=$(cat VERSION)
case "$version" in
    [0-9]*.[0-9]*.[0-9]*)
        echo "check: VERSION $version is well formed"
        ;;
    *)
        echo "check: VERSION '$version' is not major.minor.patch" >&2
        exit 1
        ;;
esac
