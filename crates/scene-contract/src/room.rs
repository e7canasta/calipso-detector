//! The room: the machine's mode, the phase and the last cell, and why a
//! scene is republished or an event happened.

use crate::{Cell, Counts, Progress, SampleRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- shared by every version of the contract
/// The mode of the layer-0 machine: the name of its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Nobody in the room.
    Empty,
    /// One occupant, off the bed.
    AloneOut,
    /// One occupant in bed, seen in the stable count.
    AloneInBedSeen,
    /// One occupant in bed, retained.
    AloneInBedHeld,
    /// More than one occupant.
    Accompanied,
}

/// The phase of rule 6: until the first climb into bed, whatever appears in
/// it is adopted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// The bed has no history yet: whoever is clearly seen in it is adopted.
    Booting,
    /// Someone climbed into bed from the floor: the bed only rises from it.
    Running,
}

// --- moved from scene-core/src/conclusion.rs
/// Why the scene is republished without a new observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RepublishReason {
    /// The sample did not decode.
    DecodeFailed,
    /// The decoder produced no image.
    DecodeEmpty,
    /// The sensor state changed.
    Sensor,
    /// The main detector failed: the belief is that of the last observation.
    DetectFailed,
}

// --- moved from scene-core/src/common.rs
/// A new count that has not stabilized yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pending {
    /// The new count per zone, waiting to hold.
    pub counts: Counts,
    /// Source seconds it has held, against the required dwell.
    pub duration: Progress<f64>,
}

/// A republished scene: what happened and when what it claims is from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Republished {
    /// Why this sample was not observed.
    pub reason: RepublishReason,
    /// The last observed sample; `None` if there has been none yet.
    pub observed_at: Option<SampleRef>,
}

/// Why an event happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Cause {
    /// The zone count stabilized at a new value.
    Stabilized,
    /// The shell reported a reception change.
    SensorReported,
    /// The map learned something about the static world.
    MapLearned,
    /// The bed had no history yet since the engine started: what is seen is
    /// adopted, stabilized, without a step from the floor.
    Baseline,
}

/// The room: the machine's mode, the phase and the last cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Room {
    /// The machine's state.
    pub mode: Mode,
    /// How many people are in the room, seen or retained.
    pub occupants: u32,
    /// Since when the mode holds.
    pub since: Option<SampleRef>,
    /// A new count that has not held long enough yet.
    pub pending: Option<Pending>,
    /// Whether the bed has history yet (rule 6).
    pub phase: Phase,
    /// The cell of the last transition; `None` before the first.
    pub last_cell: Option<Cell>,
}
