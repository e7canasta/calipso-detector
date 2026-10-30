#!/usr/bin/env bash
# Builds the detector of a revision without touching the working tree:
#   build.sh REV OUT
# A worktree in a temporary directory, with calipso-golden linked beside it so
# the path dependencies between both repositories resolve to the copy (as in
# check-guardians.sh); its own target, cached per revision under target/loop.
set -euo pipefail
rev=$1 out=$2
cd "$(dirname "$0")/../.."
root=$(pwd)
sha=$(git rev-parse --short=12 "$rev")
tmp=$(mktemp -d)
trap 'git -C "$root" worktree remove --force "$tmp/calipso-detector" 2>/dev/null || true; rm -rf "$tmp"' EXIT
git worktree add -q --detach "$tmp/calipso-detector" "$sha"
ln -s "$(cd "$root/../calipso-golden" && pwd)" "$tmp/calipso-golden"
(cd "$tmp/calipso-detector" && CARGO_TARGET_DIR="$root/target/loop/$sha" cargo build -q --release -p detector)
cp "$root/target/loop/$sha/release/calipso-detector" "$out"
echo "$out <- $sha"
