#!/usr/bin/env bash
# Builds the detector of a revision without touching the working tree:
#   build.sh REV OUT
# One worktree at a fixed path (target/loop/work), moved to each revision, with
# calipso-golden linked beside it so the path dependencies between both
# repositories resolve to the copy (as in check-guardians.sh); one release
# target for every revision (target/loop/build), so a revision rebuilds only
# what changed; and the binaries cached per revision (target/loop/bin/SHA).
# A temporary worktree and a target per revision never reused a build and
# kept 1.2 GB per revision (2026-10-31).
set -euo pipefail
rev=$1 out=$2
cd "$(dirname "$0")/../.."
root=$(pwd)
sha=$(git rev-parse --short=12 "$rev")
loop="$root/target/loop"
bin="$loop/bin/$sha"
mkdir -p "$loop"
# One build at a time: the worktree and the target are shared.
exec 9>"$loop/lock"
flock 9
# The targets per revision, from before the shared one.
find "$loop" -mindepth 1 -maxdepth 1 -type d -regextype posix-extended \
  -regex '.*/[0-9a-f]{12}' -exec rm -rf {} +
if [ ! -x "$bin/calipso-detector" ]; then
  work="$loop/work/calipso-detector"
  git worktree prune
  if [ -e "$work/.git" ]; then
    git -C "$work" checkout -q --force --detach "$sha"
  else
    git worktree add -q --detach "$work" "$sha"
  fi
  ln -sfn "$(cd "$root/../calipso-golden" && pwd)" "$loop/work/calipso-golden"
  # A revision without the frames binary must not take the previous one's.
  rm -f "$loop/build/release/calipso-detector-frames"
  (cd "$work" && CARGO_TARGET_DIR="$loop/build" CARGO_INCREMENTAL=0 \
    cargo build -q --release -p detector)
  rm -rf "$bin.tmp"
  mkdir -p "$bin.tmp"
  cp "$loop/build/release/calipso-detector" "$bin.tmp/"
  # The frames binary, from the revision that brought it on (frames.sh).
  frames="$loop/build/release/calipso-detector-frames"
  if [ -x "$frames" ]; then cp "$frames" "$bin.tmp/"; fi
  rm -rf "$bin"
  mv "$bin.tmp" "$bin"
fi
cp "$bin/calipso-detector" "$out"
if [ -x "$bin/calipso-detector-frames" ]; then
  cp "$bin/calipso-detector-frames" "$out-frames"
fi
echo "$out <- $sha"
