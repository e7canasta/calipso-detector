#!/usr/bin/env bash
# The quick level (rule 17), before each commit: format, then clippy and the
# tests of the crates that changed and of the crates that depend on them. No
# license audit, no guardians, no #[ignore]d tests (camera, NATS, models):
# those are the full gate's, scripts/check.sh, once per version.
#   scripts/quick.sh          what is about to be committed (or, with nothing
#                             to commit, the last commit)
#   scripts/quick.sh BASE     everything since BASE, e.g. main
set -euo pipefail
cd "$(dirname "$0")/.."
if [ $# -gt 0 ]; then
  base=$1
elif [ -n "$(git status --porcelain)" ]; then
  base=HEAD
else
  base=HEAD~1
fi

step() { printf '\n== %s\n' "$1"; }
fail() { printf '\n✗ %s\n' "$1" >&2; exit 1; }

# What changed: the commits since BASE, the working tree and new files.
changed=$({
  git diff --name-only "$(git merge-base "$base" HEAD)"   # commits and working tree
  git ls-files --others --exclude-standard
} | sort -u)
[ -n "$changed" ] || { echo "nothing changed against $base"; exit 0; }

# The crates to check: those whose files changed, then every crate that
# depends on them, from cargo's own graph. A change outside the crates that
# shapes the build (manifest, lock, lints, toolchain) checks the workspace;
# the features belong to the crate whose harness runs them, and the profiles
# in config/ to the detector, which loads every one of them.
crates=$(cargo metadata --format-version 1 --no-deps | CHANGED="$changed" python3 -I -c '
import json, os, sys
meta = json.load(sys.stdin)
changed = os.environ["CHANGED"].split()
root = meta["workspace_root"]
packages = {p["name"]: p for p in meta["packages"]}
dirs = {p["name"]: p["manifest_path"][len(root) + 1:].rsplit("/", 1)[0] for p in packages.values()}
whole = ("Cargo.toml", "Cargo.lock", "clippy.toml", "rust-toolchain.toml", "deny.toml")
hit = set()
for path in changed:
    if path in whole:
        hit = set(packages)
        break
    if path.startswith(("features/shell/", "config/")):
        hit.add("detector")
    elif path.startswith(("features/", "contract/")):
        hit.add("scene-core")
    hit.update(n for n, d in dirs.items() if path.startswith(d + "/"))
# Whoever depends on a changed crate, until nothing new joins.
grew = True
while grew:
    grew = False
    for name, p in packages.items():
        if name not in hit and any(d["name"] in hit for d in p["dependencies"]):
            hit.add(name)
            grew = True
print(" ".join(sorted(hit)))
')

step "format"
cargo fmt --all --check || fail "format: run cargo fmt --all"

[ -n "$crates" ] || { echo "no crate changed: format only"; exit 0; }
args=$(printf -- '-p %s ' $crates)

step "lints: $crates"
cargo clippy $args --all-targets --quiet -- -D warnings || fail "clippy"

step "tests: $crates"
cargo test $args --quiet || fail "tests"

printf '\n✓ quick level (%s against %s)\n' "$crates" "$base"
