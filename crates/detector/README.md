# Per-camera detector

One process reads RTSP, decodes the freshest sample, drives `scene-core` with
the senses and publishes `scene.v4` and `events.v4`. The
core decides what to believe; the binary translates transport and clocks. It
neither publishes nor records images.

From the workspace root:

```sh
cargo run -p detector --bin calipso-detector -- --config config/example.toml --check
CALIPSO_MODELS=/path/to/models cargo run -p detector --bin calipso-detector -- --config config/example.toml
```

`--check` prints the profile as JSON without opening models, RTSP or NATS. A
model path can be absolute or relative to `CALIPSO_MODELS` (without that
variable, to the working directory). It needs `yolo26s.onnx`, ONNX Runtime and
FFmpeg available, as for the `sense` and `ingest` crates. Geometries are in
pixels of the declared resolution, which is required; they are not scaled.

## Local video

With `mediamtx`, `ffmpeg`, `nats-server` and the NATS CLI installed, use three
terminals (and a `nats-server` on 4222 if none is running):

```sh
scripts/test-rtsp.sh
nats sub 'vision.test.>'
CALIPSO_MODELS=/path/to/models cargo run -p detector --bin calipso-detector -- --config config/test.toml
```

The default ingest fixture is synthetic, 160×96, with no people. It produces
scenes about once per second. For a real model entry and exit, the automatic
demo builds black → the public NASA fixture → black as an ephemeral 512×512
H.264, serves it with the same RTSP script and validates every message it
observes. None of those pixels comes from the observed system; the temporary
video is deleted at the end.

```sh
cargo test -p detector
cargo test -p detector --test nats -- --ignored --nocapture
CALIPSO_MODELS=/path/to/models NATS_BIN=/path/to/nats cargo test -p detector --test demo -- --ignored --nocapture
```

The last two commands open temporary local servers; the demo checks SIGTERM and
SIGINT. `NATS_BIN` is optional if `nats` is on PATH.

## Camera and bus

Copy `config/example.toml`, adjust the URL and credentials, identifiers and
map, check it with `--check` and run it. Default subjects:
`vision.<site>.<id>.{scene,events}.v4`; `[nats].prefix` sets another
prefix.
SIGINT or SIGTERM cancel reception and engine, close the mailbox and give the
publisher up to three seconds to deliver what is pending. A model load failure
also closes the resources and exits with code 1. A panic aborts the whole
process; restarting it is the supervisor's job.

The handle assigns sequences before `try_send`, without waiting for the
network. The channel holds up to 64 publications; the actor keeps up to 1024
events and the last scene of each version. A full queue drops the newest event
and logs the loss. A gap in `seq` reveals losses; `Nats-Msg-Id = run_id-seq`
deduplicates retries. The actor confirms with `flush` before removing the front
and publishes the events before the scene, whose `last_event_seq` reflects what
was delivered. Core NATS confirms transport, not durable storage: JetStream for
durability and deduplication is a deployment decision. The shutdown deadline
may end with pending events if the broker stays unreachable.

## Profile: pose, depth and the second level

`config/example.toml` comments every key; `config/sample1.toml` is the measured
profile. The optional sections switch core policies on: without the
section, the policy stays off.

| Section | Key | Default | What it does |
| --- | --- | --- | --- |
| `[senses.pose]` | `model`, `backend_confidence`, `move_threshold`, `min_keypoint_score` | —, 0.25, 0.1, 0.3 | The subject's pose: requested when they moved that fraction of their diagonal; depth is measured at keypoints with that score |
| `[senses.pose]` | `max_age_secs` | absent | Requests again the pose of a seen, still subject when the last one is that old, in source seconds. Without the key, a still pose is not requested again |
| `[senses.depth]` | `model` | — | The subject's depth (with pose) and the empty bed's (with `[calibration]`) |
| `[senses.depth]` | `posture_tolerance` | 0.05 | The relative disparity `(upper − lower) / mean` up to which the posture is "sitting". Provisional |

Both second-level keys are serialized only when present (the tolerance, only
when it changes), so the hash of a profile that does not use them does not
change. Measurements on real nights propose leaving `posture_tolerance` at 0.05 and not
setting `max_age_secs` for `sample1`.
