//! The detector shell: it translates the world towards the core.
// What is `pub` is reachable from outside; the rest says `pub(crate)`.
#![warn(unreachable_pub)]
pub mod build;
pub mod config;
mod decoder;
pub mod driver;
pub mod publish;
pub mod senses;
pub mod sensor;
pub mod source;
