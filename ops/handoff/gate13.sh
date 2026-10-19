#!/usr/bin/env bash
# Run gate 13 (scripts/gates.sh --commit-trailers) over base..head in repo $1
# by simulating a merge_group payload, which names exactly that range.
repo=$1; base=$(git -C "$repo" rev-parse "$2"); head=$(git -C "$repo" rev-parse "$3")
p=$(mktemp); out=$(mktemp)
printf '{"merge_group":{"base_sha":"%s","head_sha":"%s"}}' "$base" "$head" > "$p"
cd "$repo" && GITHUB_ACTIONS=true GITHUB_EVENT_NAME=merge_group GITHUB_EVENT_PATH="$p" bash scripts/gates.sh --commit-trailers > "$out" 2>&1
rc=$?
tail -25 "$out"
rm -f "$p" "$out"
echo "gate13 exit: $rc"
