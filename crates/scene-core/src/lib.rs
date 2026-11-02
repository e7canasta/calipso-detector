//! The sans-IO core of calipso-detector.
//!
//! It believes on partial evidence, decides where to look and projects what it
//! believes. It sees no pixels, calls no models, reads no clocks and touches
//! no network, disk or threads (rule 8). It is a deterministic function of
//! `(Policy, SiteMap, Input*, Reading*)`.
//!
//! The design reference, below, is `DESIGN.md`: it reads the same in the
//! repository and in `cargo doc`.
#![doc = include_str!("../DESIGN.md")]
// The public API is the published language and the loop protocol: every
// public item says what it is (`cargo doc` and the JSON Schema read it).
#![warn(missing_docs)]
// What is `pub` is reachable from outside; the rest says `pub(crate)`.
#![warn(unreachable_pub)]
// Rule 21: the core does not panic. No `unwrap`, no `expect`, no indexing that
// could go out of bounds: every impossible case is modelled in the types. Tests
// are shell and stay outside.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
        clippy::indexing_slicing
    )
)]

// The public API is the loop protocol and its vocabulary: `Monitor` and `Turn`,
// what is asked and read (`look`), what is published (`contract`, per version:
// `contract::v4`), what configures it (`policy`, `map`) and the geometry. How
// the core believes and decides is internal: it can be redesigned without
// breaking anyone.
//
// The loop, in the order of one turn:
//   look (ask) → evidence (admit) → belief (believe: layer 0 in
//   `belief::occupancy`, and the subject) → refine (the second level) →
//   conclusion (what the engine delivers) → chronicle (what is remembered of
//   the domain) → project (the only one that translates to `contract`).
//   `attention` decides, at each point of the turn, what else to
//   look at: it reads the belief and never writes it.
// `cycle` sequences the turn and `Monitor` composes engine, chronicle and
// projection; `policy` and `map` configure; `geom` and `estimation` are
// domain-free maths; `reception` remembers what the shell reports of the source.
// The vocabulary that the domain and the contract share (`time`, `zone`, `body`,
// `provenance` and the types of `look`, `evidence`, `attention`, `map`,
// `reception`, `refine`) is owned
// by the domain and re-exported by each contract version.
mod attention;
mod belief;
mod body;
mod chronicle;
mod conclusion;

mod cycle;
mod estimation;
mod evidence;
// Geometry is a crate of its own (ADR 2026-11-02, crates-del-lenguaje): the
// core re-exports it under its old path, so `scene_core::geom::Rect` is the same.
pub use scene_geom as geom;
#[cfg(test)]
mod golden;
// The perception language is a crate of its own (ADR 2026-11-02, crates-del-lenguaje):
// the core re-exports it under its old path, `scene_core::look`.
pub use scene_look as look;
pub mod map;
mod outcome;
pub mod policy;
pub mod project;
mod provenance;
mod reception;
mod refine;
mod room;
mod time;
pub mod units;
mod verification;
mod zone;

pub use cycle::{Input, Monitor, NoFrame, Output, Turn};
