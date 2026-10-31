#!/usr/bin/env bash
# A clip's keyframes, named by their source second, for frames.sh: the frames
# the live detector looks at with keyframe_only.
#
#   frames-of.sh CLIP.mp4 OUT_DIR
set -euo pipefail
clip=$1 out=$2
mkdir -p "$out"
ffmpeg -v error -skip_frame nokey -i "$clip" -fps_mode passthrough -q:v 2 "$out/k%06d.jpg"
ffprobe -v error -skip_frame nokey -select_streams v -show_entries frame=pts_time -of csv=p=0 "$clip" \
  | sed 's/,$//' | grep . > "$out/.pts"
i=0
while read -r t; do
  i=$((i + 1))
  mv "$out/$(printf 'k%06d.jpg' "$i")" "$out/$(printf '%g' "$t").jpg"
done < "$out/.pts"
rm "$out/.pts"
echo "$out: $i frames"
