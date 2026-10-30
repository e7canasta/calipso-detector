#!/usr/bin/env bash
# The characterization test of a step on a looped clip (white book, volume
# VII, ch. 12): the baseline and the candidate, one after the other, on the
# same profile and clip, recorded apart and compared scene by scene.
#
#   ab.sh BASELINE CANDIDATE PROFILE LOOP_SECS OUT_DIR [LOOPS]
#
# BASELINE and CANDIDATE are revisions (built with build.sh) or paths to
# binaries; "." is the working tree. PROFILE publishes on a prefix of its own
# (never the demo's) and has no verifier. LOOP_SECS is the clip's length.
# Each binary runs LOOPS + 1 loops: the first, the cold start, is not
# compared. Run one detector at a time on the clip: two at once share the CPU
# and drop samples.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
baseline=$1 candidate=$2 profile=$3 loop_secs=$4 out=$5 loops=${6:-2}
mkdir -p "$out"
: "${CALIPSO_MODELS:?the models directory}"
prefix=$(sed -n 's/^prefix *= *"\(.*\)"/\1/p' "$profile" | head -1)
[ -n "$prefix" ] || { echo "the profile needs [nats] prefix: never the demo's" >&2; exit 2; }
grep -q '^\[verify\]' "$profile" && { echo "drop [verify] from the profile: it is not repeatable" >&2; exit 2; }

binary() {  # name rev-or-path
  if [ "$2" = "." ]; then
    (cd "$root" && cargo build -q --release -p detector)
    echo "$root/target/release/calipso-detector"
  elif [ -x "$2" ]; then
    echo "$2"
  else
    "$here/build.sh" "$2" "$out/calipso-detector-$1" >&2
    echo "$out/calipso-detector-$1"
  fi
}

run() {  # name binary
  local secs=$(( (loops + 1) * loop_secs + 30 ))
  python3 -I "$here/record.py" "$prefix.>" "$secs" "$out/$1.jsonl" > /dev/null &
  local rec=$!
  sleep 1
  "$2" --config "$profile" > "$out/$1.log" 2>&1 &
  local det=$!
  wait "$rec"
  kill "$det"
  for _ in $(seq 15); do kill -0 "$det" 2>/dev/null || break; sleep 1; done
  kill -9 "$det" 2>/dev/null || true
  wait "$det" 2>/dev/null || true
  echo "$1: $(wc -l < "$out/$1.jsonl") messages"
}

a=$(binary baseline "$baseline")
b=$(binary candidate "$candidate")
run baseline "$a"
run candidate "$b"
python3 -I "$here/same.py" "$out/baseline.jsonl" "$out/candidate.jsonl" | tee "$out/same.txt"
