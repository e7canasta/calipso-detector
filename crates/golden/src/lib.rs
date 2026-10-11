//! Golden scenes: scenes written by the pair or distilled from a real
//! night. It is a context of its own, with its own language (span, on floor,
//! near bed); it fits the core's public API and the core does not know it.
//!
//! - [`language`]: read a scene (`.md`) or a Gherkin table as spans.
//! - [`world`]: what each sense sees in each sample (the scripted frame).
//! - [`expected`]: what the engine must believe at the end of each span.
//! - [`distill`]: a published night (`scene.v3`) turned into a scene, without
//!   pixels.

// The same rules as the core: no panics; the impossible is an error with the
// line where it is.
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

pub mod distill;
pub mod expected;
pub mod language;
pub mod world;
