# Contributing

Thank you for looking at calipso-detector. This file says how a change gets
in; [`RULES.md`](RULES.md) says what no change may break, and
[`crates/scene-core/DESIGN.md`](crates/scene-core/DESIGN.md) is the reference
for how the engine is built.

## Before you start

- Toolchain: the one pinned in `rust-toolchain.toml`, plus `cargo-deny` for
  the gate's license audit. The shell's end-to-end tests also use FFmpeg,
  MediaMTX, `nats-server` and the ONNX models in `CALIPSO_MODELS`.
- `scripts/check.sh` is the gate: format, clippy, the dependency rules,
  every test and the license audit. The end-to-end tests that need a camera
  server, NATS or the models are `#[ignore]`d and say what they require.
- `scripts/check-guardians.sh` proves that each gate rule fails when it is
  broken. Run it when you touch `check.sh` or a lint.

## How a change is made

1. **Specification first** (rules 14 and 15). A behaviour change starts as a
   Gherkin scenario in [`features/`](features/README.md), in the capability it
   belongs to, and it runs red before any code is written.
2. **Red, green, refactor** (rule 16). The smallest code that turns the
   scenario green; then the refactor, with every scenario still green. A
   refactor does not change a table cell, a published event or a `.feature`.
3. **The contract is published language** (rule 13, C1–C5 in `DESIGN.md`). A
   change to what `calipso.scene` or `calipso.events` publish needs a new
   contract version, never an edit of a published one. The JSON Schema
   snapshots catch it; regenerate them only on purpose:
   `CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-core --test contract_schema`.
4. **The core does no IO and does not panic** (rules 8 and 21). If you need a
   clock, a file or a model, it belongs to the shell (`detector`, `sense`,
   `ingest`).

## Commits

- One logical change per commit, and **every commit passes the gate**
  (rule 17). For a series: `git rebase -x scripts/check.sh main`.
- Subject: `area: what changes`, in the imperative, under 72 characters
  (`attention: renew a still pose before it expires`). The body says why, and
  cites the rule or principle it serves.
- Sign off every commit (`git commit -s`): the `Signed-off-by` line certifies
  the [Developer Certificate of Origin](https://developercertificate.org/).

## Review

[`CODEOWNERS`](CODEOWNERS) names who reviews each subsystem. A change that
crosses the contract needs a review from both sides: the engine and a
consumer.
