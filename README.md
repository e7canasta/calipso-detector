# calipso-detector

The Calipso scene-perception engine. One process per camera answers two care
questions: **how many people are in the room?** and **is someone in the bed,
even when they cannot be seen?**

It is built as an **active-perception loop**: a sans-IO core that believes and
decides where to look, and a thin shell that only looks and delivers. The core
publishes facts per sample on a versioned contract; everything that consumes
those facts lives in other repositories.

## Layout

| Crate | What it is | Depends on |
| --- | --- | --- |
| [`scene-core`](crates/scene-core) | the sans-IO core: belief, attention, the published contract | `serde`, `schemars` |
| [`ingest`](crates/ingest) | RTSP, a one-slot mailbox, on-demand H.264 decode | nothing from the domain |
| [`sense`](crates/sense) | the senses: runs each probe with a model (YOLO, ONNX) | `scene-core` |
| [`detector`](crates/detector) | the per-camera binary: profile, injected source and senses, loop driver, NATS publishing | all of the above |
| [`golden`](crates/golden) | golden scenes: their language, the scripted world, the expectations, `calipso-distill` | `scene-core` |

| Directory | What it holds |
| --- | --- |
| [`contract/`](contract) | JSON schemas for `calipso.scene.v{3,4}` and `calipso.events.v{3,4}`, examples, the layer-0 transition table |
| [`features/`](features) | the executable Gherkin specification of the core and the shell |
| [`golden/`](golden) | golden scenes that run through the real detector without camera or models |
| [`config/`](config) | example camera profiles |
| [`scripts/`](scripts) | `check.sh` (the per-commit gate) and its own tests |

## Build and check

```sh
scripts/check.sh            # format, clippy, core dependency rules, tests, licenses
scripts/check-guardians.sh  # each gate rule is proven to fail when broken
cargo run -p detector -- --config config/example.toml --check
cargo run -p detector -- --config config/scene.toml --scene golden/s1-staff-put-to-bed.md --fast --jsonl /tmp/s1.jsonl
```

## Documentation

| Document | What it is |
| --- | --- |
| [`crates/scene-core/DESIGN.md`](crates/scene-core/DESIGN.md) | The engine's reference: invariants, the loop, layer 0, module map, glossary |
| [`RULES.md`](RULES.md) | What is not negotiable, each rule with its guardian; the code cites them by number |
| [`crates/detector/README.md`](crates/detector/README.md) | Running the per-camera binary: profiles, NATS, golden scenes |
| [`golden/README.md`](golden/README.md) | The golden scene language |
| [`contract/`](contract) | The published JSON schemas, examples and the layer-0 table |

## Not in this repository

The engine does one thing. These consume its contract and live apart:

- `calipso-shark`: recording and analysis from NATS (the transition tape, summaries, the occupancy TUI, the meters).
- `calipso-rerun-bridge`: draws the published scene and events in Rerun.
- `calipso-docs`: books, specifications, workshops and design history.
