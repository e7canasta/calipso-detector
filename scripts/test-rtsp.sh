#!/usr/bin/env bash
# Serves a synthetic video, without people. On exit, it cleans up its two processes.
# Usage: scripts/test-rtsp.sh [port [fixture.h264]]
set -euo pipefail
port=${1:-18554}
fixture=${2:-$(dirname "$0")/../crates/ingest/tests/fixtures/testsrc.h264}
[[ "$port" =~ ^[0-9]+$ ]] && ((port > 0 && port < 65536)) || { echo 'Invalid port' >&2; exit 2; }
for tool in mediamtx ffmpeg; do
  command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 2; }
done
[[ -f "$fixture" ]] || { echo "$fixture does not exist" >&2; exit 2; }
rtsp_work_dir=$(mktemp -d /tmp/calipso-rtsp.XXXXXX)
server_pid=''
publisher_pid=''
cleanup() {
  trap - EXIT INT TERM
  [[ -z "$publisher_pid" ]] || kill "$publisher_pid" 2>/dev/null || true
  [[ -z "$publisher_pid" ]] || wait "$publisher_pid" 2>/dev/null || true
  [[ -z "$server_pid" ]] || kill "$server_pid" 2>/dev/null || true
  [[ -z "$server_pid" ]] || wait "$server_pid" 2>/dev/null || true
  # Only the temporary directory this invocation created.
  rm -rf "$rtsp_work_dir"
}
trap cleanup EXIT
trap 'exit 0' INT TERM
cat > "$rtsp_work_dir/mediamtx.yml" <<CONFIG
logLevel: warn
rtspAddress: 127.0.0.1:$port
rtspTransports: [tcp]
rtmp: no
hls: no
webrtc: no
srt: no
paths:
  test:
    source: publisher
    runOnReady: touch $rtsp_work_dir/ready
CONFIG
mediamtx "$rtsp_work_dir/mediamtx.yml" >&2 &
server_pid=$!
# Wait for the listener before starting the publisher.
for ((attempt=0; attempt<100; attempt++)); do
  kill -0 "$server_pid" 2>/dev/null || exit 1
  if (echo > /dev/tcp/127.0.0.1/"$port") 2>/dev/null; then break; fi
  sleep 0.1
done
url="rtsp://127.0.0.1:$port/test"
# The raw H.264 demuxer cannot rewind with stream_loop. Remuxing without
# re-encoding gives the fixture an index and timestamps to loop it.
ffmpeg -nostdin -hide_banner -loglevel error -framerate 10 -i "$fixture" \
  -c:v copy -an "$rtsp_work_dir/fixture.mp4"
ffmpeg -nostdin -hide_banner -loglevel error -re -stream_loop -1 \
  -i "$rtsp_work_dir/fixture.mp4" -c:v copy -an -f rtsp -rtsp_transport tcp "$url" >&2 &
publisher_pid=$!
for ((attempt=0; attempt<100; attempt++)); do
  kill -0 "$server_pid" 2>/dev/null && kill -0 "$publisher_pid" 2>/dev/null || exit 1
  if [[ -f "$rtsp_work_dir/ready" ]]; then
    # The script's contract: the first stdout line says publishing has started.
    echo "$url"
    wait "$server_pid" "$publisher_pid"
    exit 0
  fi
  sleep 0.1
done
echo 'The RTSP publisher was not ready in 10 s' >&2
exit 1
