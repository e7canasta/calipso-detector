#!/usr/bin/env bash
# The guardian: one command that verifies a commit (rule 17).
# For each commit of a series:  git rebase -x scripts/check.sh main
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$1"; }
fail() { printf '\n✗ %s\n' "$1" >&2; exit 1; }

# Rule 8: the core does not depend on IO crates. cargo-deny bans for the whole
# workspace and not per crate, so scene-core's tree is checked directly.
FORBIDDEN_CORE_DEPS='image|ort|retina|tokio|async-nats|ffmpeg-next|ffmpeg-sys-next|reqwest|mio'
check_core_deps() {
  local found
  found=$(cargo tree -p scene-core -e normal --prefix none --format '{p}' \
            | awk '{print $1}' | grep -Ex "$FORBIDDEN_CORE_DEPS" | sort -u || true)
  [ -z "$found" ] || fail "rule 8: scene-core depends on IO: $found"
}

step "format"
cargo fmt --all --check || fail "format: run cargo fmt --all"

step "lints"
cargo clippy --workspace --all-targets --quiet -- -D warnings || fail "clippy"

step "core dependencies"
check_core_deps

# ingest hides the transport, without depending on the domain.
check_ingest_deps() {
  local tree
  tree=$(cargo tree -p ingest -e normal --prefix none --format '{p}') \
    || fail "could not read the ingest tree"
  if printf '%s\n' "$tree" | awk '{print $1}' | grep -qx scene-core; then
    fail "ingest depends on scene-core"
  fi
}
step "ingest dependencies"
check_ingest_deps

# The camera process does one thing. Scenes, their language and the replay
# live in calipso-golden and depend on this repo, never the other way around.
check_detector_deps() {
  local tree
  tree=$(cargo tree -p detector -e normal --prefix none --format '{p}') \
    || fail "could not read the detector tree"
  if printf '%s\n' "$tree" | awk '{print $1}' | grep -qx golden; then
    fail "detector depends on golden: the scene language lives in calipso-golden"
  fi
}
step "detector dependencies"
check_detector_deps

# sense is synchronous: its ONNX adapters stay without transport too.
check_sense_deps() {
  local tree found
  tree=$(cargo tree -p sense --all-features -e normal --prefix none --format '{p}') \
    || fail "could not read the sense tree"
  found=$(printf '%s\n' "$tree" \
    | awk '$1 == "tokio" || $1 == "async-nats" { print $1 }' | sort -u)
  [ -z "$found" ] || fail "sense depends on async transport: $found"
}
step "sense dependencies (with ONNX)"
check_sense_deps

# The domain does not speak contract (DESIGN.md §2): it decides with its own
# vocabulary and `contract` re-exports it. Only `project` translates, and
# `cycle` names the contract because `Output` carries it. The chronicle
# remembers the domain.
check_domain_contract() {
  local found
  found=$(grep -rl 'crate::contract' crates/scene-core/src \
            | grep -vE '^crates/scene-core/src/(contract/|project/|cycle\.rs$)' \
            || true)
  [ -z "$found" ] || fail "the domain imports the contract: $found"
  # What no version changes is projected into `contract::common`: a next
  # version shares it without depending on v4 (C2).
  found=$(grep -nE 'contract::v[0-9]|v[0-9]+::' crates/scene-core/src/project/mod.rs || true)
  [ -z "$found" ] || fail "C2: the common projection names a version: $found"
}
step "the domain without contract"
check_domain_contract

# Rule 11: `cycle` only sequences. Whether a sense is configured is decided by
# the module that decides with it: the sequencer hands it the whole `Policy`.
check_cycle_sequences() {
  local found
  found=$(grep -rnE 'let Some\(.*\) = .*\.policy\.|\.policy\.[a-z_]+\.(map_or|is_some|is_none)' \
            crates/scene-core/src/cycle.rs crates/scene-core/src/cycle/ || true)
  [ -z "$found" ] || fail "rule 11: cycle asks which senses are configured: $found"
}
step "cycle only sequences"
check_cycle_sequences

# The questions of space have one owner (white book, volume VII, ch. 01): the
# layers ask `room` and get named facts back. The bed's plane stays with the
# map, which crops with it, the room's pieces, which answer with it, and the
# shadow, which measures a geometry that does not decide yet.
check_space_has_one_owner() {
  local found
  found=$(grep -rl 'BedPlane' crates/scene-core/src \
            | grep -vE '^crates/scene-core/src/(map\.rs|map/|room/(mod|frontal|two_planes|shadow)\.rs)' \
            || true)
  [ -z "$found" ] || fail "a question of space outside room: $found"
}
step "space has one owner"
check_space_has_one_owner

step "tests"
cargo test --workspace --quiet || fail "tests"


step "licenses and advisories"
cargo deny --log-level error check licenses advisories || fail "licenses or advisories"

printf '\n✓ commit verified\n'
