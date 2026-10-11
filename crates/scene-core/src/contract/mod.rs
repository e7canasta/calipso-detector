//! The published contract, per version. Each version is a module that defines
//! what it publishes and re-exports the domain vocabulary it uses (C3).
//!
//! `contract` re-exports the published version, v4, so consumers can name
//! `scene_core::contract::Scene`. A consumer that wants to pin a version names
//! it in full (`scene_core::contract::v4::Scene`): when the next version is
//! published next to it (C2), its code does not change.

mod common;
pub mod v4;

pub use v4::*;
