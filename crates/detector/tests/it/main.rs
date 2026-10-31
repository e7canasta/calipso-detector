//! The detector's integration tests, in one binary: each file is a module, so
//! the dependencies (ONNX Runtime, FFmpeg, tokio) are linked once and not once
//! per file. Run one file with its module: `cargo test -p detector --test it
//! nats::`.

#[path = "../support/models.rs"]
#[allow(dead_code)]
mod models;
#[path = "../support/mod.rs"]
mod support;

mod config;
mod demo;
mod e2e;
mod faces;
mod nats;
mod publish;
mod role_models;
mod startup;
mod verifier;
