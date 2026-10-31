#!/usr/bin/env bash
# The characterization test over frames (white book, volume VII, ch. 12): the
# baseline and the candidate over the same directory of frames, without a
# camera and without waiting, so no sample is dropped and the machine's load
# does not matter. Compared scene by scene with same.py.
#
#   frames.sh BASELINE CANDIDATE PROFILE FRAMES_DIR OUT_DIR
#
# BASELINE and CANDIDATE are revisions (from the one that brought
# calipso-detector-frames on), "." for the working tree, or paths to a
# calipso-detector-frames binary. FRAMES_DIR holds JPEG frames named by their
# source second; extract a clip's keyframes with frames-of.sh.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
baseline=$1 candidate=$2 profile=$3 frames=$4 out=$5
mkdir -p "$out"
: "${CALIPSO_MODELS:?the models directory}"

binary() {  # name rev-or-path
  if [ "$2" = "." ]; then
    (cd "$root" && cargo build -q --release -p detector --bin calipso-detector-frames)
    cp "$root/target/release/calipso-detector-frames" "$out/calipso-detector-frames-$1"
  elif [ -x "$2" ]; then
    cp "$2" "$out/calipso-detector-frames-$1"
  else
    "$here/build.sh" "$2" "$out/calipso-detector-$1" >&2
    [ -x "$out/calipso-detector-$1-frames" ] \
      || { echo "$2 has no calipso-detector-frames: use a later revision" >&2; exit 2; }
    mv "$out/calipso-detector-$1-frames" "$out/calipso-detector-frames-$1"
  fi
  echo "$out/calipso-detector-frames-$1"
}

a=$(binary baseline "$baseline")
b=$(binary candidate "$candidate")
for side in baseline candidate; do
  bin=$a; [ "$side" = candidate ] && bin=$b
  "$bin" --config "$profile" --frames "$frames" --out "$out/$side.jsonl" 2> "$out/$side.log"
  echo "$side: $(wc -l < "$out/$side.jsonl") messages"
done
python3 -I "$here/same.py" "$out/baseline.jsonl" "$out/candidate.jsonl" | tee "$out/same.txt"
