#!/usr/bin/env bash
# The guardians have tests too: each fixture breaks a rule on a copy of the
# workspace, and `check.sh` must FAIL at the step the fixture declares, with the
# text the fixture declares in the log. A fixture declares both in its first
# lines:
#   # expect-step: <the name of a step of check.sh>
#   # expect-text: <a text the log of that run must contain>
# A fixture that passes, fails at another step, or fails without its text makes
# this script fail. With names as arguments, only those fixtures run:
#   scripts/check-guardians.sh core-panics cycle-consults-a-policy
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
# Each fixture is named here: check.sh fails if a file in scripts/fixtures/ is
# not named, or a name has no file, so a guardian cannot lose its fixture in silence.
fixtures=(
  scripts/fixtures/common-projection-names-a-version.sh
  scripts/fixtures/contract-changes-without-snapshot.sh
  scripts/fixtures/core-depends-on-tokio.sh
  scripts/fixtures/core-panics.sh
  scripts/fixtures/core-reads-the-clock.sh
  scripts/fixtures/core-undocumented.sh
  scripts/fixtures/cycle-consults-a-policy.sh
  scripts/fixtures/domain-names-the-publication.sh
  scripts/fixtures/g2-round-reads-engine-field.sh
  scripts/fixtures/g3-domain-names-wire-type.sh
  scripts/fixtures/geom-depends-on-tokio.sh
  scripts/fixtures/geom-grows-a-dependency.sh
  scripts/fixtures/occupancy-reads-verify.sh
  scripts/fixtures/room-reads-a-layer.sh
  scripts/fixtures/rule24-dyn-decides.sh
  scripts/fixtures/sense-depends-on-core.sh
  scripts/fixtures/space-outside-room.sh
  scripts/fixtures/undefined-step-shell.sh
  scripts/fixtures/undefined-step.sh
  scripts/fixtures/unlisted-fixture.sh
)
if [ $# -gt 0 ]; then
  fixtures=()
  for name in "$@"; do
    [ -f "scripts/fixtures/$name.sh" ] || { echo "no fixture named $name" >&2; exit 2; }
    fixtures+=("scripts/fixtures/$name.sh")
  done
fi
for fixture in "${fixtures[@]}"; do
  name=$(basename "$fixture" .sh)
  # What the fixture declares: the step of check.sh where it must fail, and a
  # text the log of that run must contain.
  expect_step=$(sed -n 's/^# expect-step: //p' "$fixture" | head -1)
  expect_text=$(sed -n 's/^# expect-text: //p' "$fixture" | head -1)
  if [ -z "$expect_step" ] || [ -z "$expect_text" ]; then
    printf '✗ %s: declares no expect-step or no expect-text\n' "$name"; status=1
    continue
  fi
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
    printf '✗ %s: the guardian did NOT fail (declared: step "%s")\n' "$name" "$expect_step"; status=1
    continue
  fi
  # check.sh stops at its first failing step: the last step it started is the
  # one that failed.
  failed_at=$(grep '^== ' "$dir/check.log" | tail -1 | sed 's/^== //')
  if [ "$failed_at" != "$expect_step" ]; then
    printf '✗ %s: failed at step "%s", declared "%s"\n' "$name" "$failed_at" "$expect_step"; status=1
    continue
  fi
  if ! grep -qF -- "$expect_text" "$dir/check.log"; then
    printf '✗ %s: failed at "%s" but the log lacks "%s"\n' "$name" "$expect_step" "$expect_text"; status=1
    continue
  fi
  printf '✓ %s → %s → "%s"\n' "$name" "$expect_step" "$expect_text"
done
exit $status
