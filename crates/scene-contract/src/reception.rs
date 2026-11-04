//! What the shell reports of the source: the reception state and the size of
//! a decoded frame.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- moved from scene-core/src/reception.rs
/// The reception state, as the shell reports it. It says nothing about the bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SensorState {
    /// Opening the stream.
    Connecting,
    /// Samples are arriving.
    Streaming,
    /// Connected, but no sample within the stall timeout.
    Stalled,
    /// The stream dropped; trying again.
    Reconnecting,
}

// --- moved from scene-core/src/map.rs
/// Size of a decoded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FrameInfo {
    /// In pixels.
    pub width: u32,
    /// In pixels.
    pub height: u32,
}
