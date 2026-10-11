//! The published contract, per version. Each version is a module that defines
//! what it publishes and re-exports the domain vocabulary it uses (C3).
//!
//! `contract` re-exports v3 so consumers keep naming
//! `scene_core::contract::Scene` while v3 is the published version (C1). v4 is
//! named in full: `scene_core::contract::v4::Scene`.

mod common;
pub mod v3;
pub mod v4;

pub use v3::*;
