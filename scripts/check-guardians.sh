#!/usr/bin/env bash
# The guardians have tests too: each fixture breaks a rule on a copy of the
# workspace and `check.sh` must FAIL.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd)
export CARGO_TARGET_DIR="$root/target/guardians"
status=0
for fixture in scripts/fixtures/*.sh; do
  name=$(basename "$fixture" .sh)
  work=$(mktemp -d)
  rsync -a --exclude target --exclude .git "$root/" "$work/"
  (cd "$work" && bash "$root/$fixture")
  if (cd "$work" && scripts/check.sh >"$work/check.log" 2>&1); then
    printf '✗ %s: the guardian did NOT fail\n' "$name"; status=1
  else
    printf '✓ %s: failed as it should (%s)\n' "$name" "$(grep -m1 '✗' "$work/check.log" | sed 's/^✗ //')"
  fi
  rm -rf "$work"
done
exit $status
