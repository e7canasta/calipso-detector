#!/usr/bin/env bash
# The guardians have tests too: each fixture breaks a rule on a copy of the
# workspace and `check.sh` must FAIL.
#
# Each fixture keeps its copy at a fixed path, target/guardians/work/NAME.
# Cargo hashes a workspace crate with its path: a new temporary directory per
# run meant ten new builds per run that nothing reused or removed (21 GB of
# target/ on 2026-10-31). With fixed paths the next run reuses each build, and
# all the copies share one target, so the dependencies compile once.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd)
guardians="$root/target/guardians"
export CARGO_TARGET_DIR="$guardians/build"
# Each copy is built to be checked, not edited: the incremental cache only
# grew (2 GB on a second run).
export CARGO_INCREMENTAL=0
golden=$(cd "$root/../calipso-golden" && pwd)
# The builds of the temporary copies, from before the fixed paths, and the
# incremental cache, from before it was off.
rm -rf "$guardians/debug" "$guardians/tmp" "$guardians/build/debug/incremental"
status=0
for fixture in scripts/fixtures/*.sh; do
  name=$(basename "$fixture" .sh)
  # The copy keeps its name next to calipso-golden: the path dependencies
  # between both repositories resolve to the copy.
  dir="$guardians/work/$name"
  work="$dir/calipso-detector"
  mkdir -p "$work"
  ln -sfn "$golden" "$dir/calipso-golden"
  # Back to the workspace as it is now. rsync would bring each file back with
  # its old time, older than the build of the broken copy, and cargo would
  # take that build as fresh: every file it brings back is touched, so cargo
  # rebuilds what changed since the last run, and only that.
  rsync -a --delete --exclude target --exclude .git --out-format='%n' \
    "$root/" "$work/" | while IFS= read -r file; do
    if [ -f "$work/$file" ]; then touch "$work/$file"; fi
  done
  (cd "$work" && bash "$root/$fixture")
  if (cd "$work" && scripts/check.sh >"$dir/check.log" 2>&1); then
    printf '✗ %s: the guardian did NOT fail\n' "$name"; status=1
  else
    printf '✓ %s: failed as it should (%s)\n' "$name" "$(grep -m1 '✗' "$dir/check.log" | sed 's/^✗ //')"
  fi
done
exit $status
