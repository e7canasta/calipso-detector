//! The conclusion: what the engine delivers when a turn closes, before the
//! chronicle and the contract. It is the seam between deciding and publishing:
//! the engine (`cycle`) produces it, the chronicle and the projection
//! (`chronicle`, `project`) consume it, and neither is read back (project 24,
//! D6).

use crate::attention::Attention;
use crate::belief::Believed;
use crate::evidence::Perceived;
use crate::map::{FrameInfo, MapState};
use crate::sensor::SensorChanged;
use crate::sensor::SensorState;
use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the engine concludes from a closed turn.
#[derive(Debug)]
pub(crate) enum Conclusion {
    /// There was an observation.
    Observed(Box<Observation>),
    /// No new observation: the last one is republished, declared as such.
    Republish {
        reason: RepublishReason,
        changed: Option<SensorChanged>,
        sensor: SensorState,
        map: MapState,
    },
}

/// What a turn observed and believed, together and only once.
#[derive(Debug)]
pub(crate) struct Observation {
    pub at: SampleRef,
    pub believed: Believed,
    pub refined: crate::refine::Refinement,
    pub evidence: Perceived,
    pub frame: Option<FrameInfo>,
    pub calibrated: Option<crate::map::BedCalibrated>,
    pub attention: Attention,
    pub map: MapState,
    pub sensor: SensorState,
    /// How long a new count must hold (`Persistence`): the projection publishes
    /// it as what remains of the pending count. It is policy, not layer-0
    /// state, so it does not travel with the pending count.
    pub dwell_secs: f64,
}

/// Why the scene is republished without a new observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
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
