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
  # The core has no contract module of its own (phase 2 of S7): what is
  # published is named as scene-contract names it. Its tests too.
  found=$(grep -rnE 'crate::contract|scene_core::contract' \
            crates/scene-core/src crates/scene-core/tests || true)
  found="$found$(grep -nE '^\s*(pub(\(crate\))? )?mod contract;' crates/scene-core/src/lib.rs || true)"
  [ -z "$found" ] || fail "the core names a contract module of its own: $found"
  # The common projection writes the contract's v5 alone (ADR 2026-11-02-retirar-v4):
  # no SchemaId, and no v4 name, which the retired schema would bring back.
  found=$(grep -nE 'SchemaId|SceneV4|EventsV4' crates/scene-core/src/project/mod.rs || true)
  [ -z "$found" ] || fail "C2: the common projection names a retired version: $found"
  # Only project and cycle name the published scene and its events: the rule is a
  # cargo test, which reads the sources without any tool of the system (F1).
  # Its output is shown when it fails: the name of the guardian that failed is
  # what a fixture declares (check-guardians.sh).
  cargo test -q -p scene-core --test guardians \
    || fail "the domain names the published scene or events (cargo test -p scene-core --test guardians)"
}
step "the domain without contract"
check_domain_contract

# ADR 2026-11-02-retirar-v4: the detector publishes v5 alone. A v4 schema name,
# the projection that built it or its SchemaId variants are not written again
# in the detector's code (the `archivo/` of the repository is not code).
check_no_retired_v4() {
  local found
  # The names the retired contract had: its schema ids, its module path, its
  # type names and the file names it was published under.
  found=$(grep -rnE 'calipso\.(scene|events)\.v4|\.(scene|events)\.v4|project::v4|scene_contract::v4|\bv4::|\b(Scene|Events)V4\b|SchemaId::(SceneV4|EventsV4)' \
            crates --include='*.rs' || true)
  [ -z "$found" ] || fail "ADR 2026-11-02-retirar-v4: v4 reappears in the detector: $found"
  # The retired schemas and their examples are not files of the contract either.
  found=$(find contract -name '*v4*' 2>/dev/null || true)
  [ -z "$found" ] || fail "ADR 2026-11-02-retirar-v4: the retired v4 contract is back in contract/: $found"
}
step "no retired v4 in the detector"
check_no_retired_v4

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

# The room does not depend on the layers above it (DESIGN, glossary: head
# area): it answers questions of space with the site, the policy and the
# shared vocabulary, and whoever concludes something asks it. What the room
# reads from the turn arrives as a point, never as a reading, a belief or a
# decision.
check_room_asks_nothing_above() {
  local layers='evidence|look|belief|refine|attention|verification|cycle|conclusion|chronicle|project|contract|sensor'
  local found
  found=$(grep -rnE "crate::(\{[^}]*\b)?($layers)\b" crates/scene-core/src/room/ || true)
  [ -z "$found" ] || fail "the room depends on a layer above it: $found"
}
step "the room asks nothing above it"
check_room_asks_nothing_above

# Each decider receives its own policy slice (ADR 2026-11-02,
# politica-en-rebanadas): the occupancy names no verification, bed posture or
# face policy, so no setting of those layers can change a count.
check_occupancy_names_its_slice() {
  local found
  found=$(grep -rnE 'VerifyPolicy|BedPosturePolicy|FacePolicy' \
            crates/scene-core/src/belief/occupancy/ || true)
  [ -z "$found" ] || fail "the occupancy names a policy of another layer: $found"
}
step "the occupancy names its own slice"
check_occupancy_names_its_slice

# The language's leaf (ADR 2026-11-02, crates-del-lenguaje): scene-geom is
# points, boxes and quadrilaterals. It depends on no domain and on no IO, as
# the core does not (rule 8), so every crate above it may use it.
# A leaf is what it says: scene-geom's direct dependencies are a closed list, the
# one of its Cargo.toml today. Anything else, even a crate with no IO, fails
# here; a new dependency is added to this list on purpose, with its reason.
GEOM_DEPS='serde|schemars'
check_geom_is_a_leaf() {
  local tree found
  # The first line is scene-geom itself; the rest are its direct dependencies.
  tree=$(cargo tree -p scene-geom -e normal --depth 1 --prefix none --format '{p}') \
    || fail "could not read the scene-geom tree"
  found=$(printf '%s\n' "$tree" | tail -n +2 | awk '{print $1}' \
            | grep -vE "^($GEOM_DEPS)$" | sort -u || true)
  [ -z "$found" ] || fail "scene-geom is not a leaf: it depends on $found"
}
step "scene-geom is a leaf"
check_geom_is_a_leaf

# The language is apart from the core (ADR 2026-11-02, crates-del-lenguaje):
# scene-look speaks no IO and reaches neither the domain nor a sibling that
# does; sense (which runs the models, so it has IO) reaches the language and
# the wire, never scene-core.
check_language_apart_from_core() {
  local tree found
  tree=$(cargo tree -p scene-look -e normal --prefix none --format '{p}') \
    || fail "could not read the scene-look tree"
  found=$(printf '%s\n' "$tree" | awk '{print $1}' \
          | grep -Ex "scene-core|$FORBIDDEN_CORE_DEPS" | sort -u || true)
  [ -z "$found" ] || fail "scene-look is not apart from the core: it depends on $found"
  tree=$(cargo tree -p sense --features onnx -e normal --prefix none --format '{p}') \
    || fail "could not read the sense tree"
  found=$(printf '%s\n' "$tree" | awk '{print $1}' | grep -Ex "scene-core" | sort -u || true)
  [ -z "$found" ] || fail "sense depends on the domain: $found"
}
step "scene-look is apart from the core; sense does not depend on scene-core"
check_language_apart_from_core

# Every guardian's fixture is run: a file in scripts/fixtures/ that
# check-guardians.sh does not name is never broken on purpose, and a name
# without its file would let the guardian pass in silence (the two empty
# fixtures found on 2026-11-02 were both of these).
check_fixtures_are_all_run() {
  local files named orphan missing
  files=$(ls scripts/fixtures/*.sh | sort)
  named=$(grep -oE 'scripts/fixtures/[a-z0-9-]+\.sh' scripts/check-guardians.sh | sort -u)
  orphan=$(comm -23 <(printf '%s\n' "$files") <(printf '%s\n' "$named"))
  missing=$(comm -13 <(printf '%s\n' "$files") <(printf '%s\n' "$named"))
  [ -z "$orphan" ] || fail "a fixture check-guardians.sh does not run: $orphan"
  [ -z "$missing" ] || fail "check-guardians.sh names a fixture with no file: $missing"
}
step "every fixture is run"
check_fixtures_are_all_run

step "tests"
cargo test --workspace --quiet || fail "tests"


step "licenses and advisories"
cargo deny --log-level error check licenses advisories || fail "licenses or advisories"

printf '\n✓ commit verified\n'
