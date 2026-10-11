//! What every contract version shares: the publisher's identity, the profile,
//! the parts of the scene no version changes (the bed, the floor, the subject,
//! the evidence, the republication) and the causes. It also re-exports the
//! domain vocabulary every version publishes. No version depends on another:
//! each re-exports this and defines its own.

// The domain vocabulary every version publishes: its modules own it.
pub use crate::attention::{Attention, Look, LookOutcome, LookResult, SkipReason};
pub use crate::body::{BodyPoint, Joint, Keypoint, Pose, SubjectDepth};
pub use crate::conclusion::RepublishReason;
pub use crate::evidence::{BodySource, RejectReason};
use crate::geom::Rect;
pub use crate::look::{FailureKind, LookKind, ProbeKindMismatch};
pub use crate::map::{Calibration, FrameInfo, Geometry, MapState};
pub use crate::provenance::Provenance;
pub use crate::refine::{Lying, LyingState, Posture, PostureState};
pub use crate::sensor::SensorState;
pub use crate::time::{SampleRef, SessionId, SourceSecs};
pub use crate::zone::{Counts, Zone};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identity of the publishing shell; the core does not build these values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub site: String,
    pub camera: String,
    pub run_id: String,
    pub build: Build,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Build {
    pub version: String,
    pub git: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    pub camera: Option<LayerRef>,
    pub resident: Option<LayerRef>,
    pub hash: ProfileHash,
    /// What each configured sense looks with. `hash` already covers them; this
    /// names them for whoever watches, as in v2. Empty if the shell does not
    /// report them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub senses: Vec<SenseProfile>,
}

/// The model and thresholds of a sense.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SenseProfile {
    pub kind: LookKind,
    /// The model file, without its path.
    pub model: String,
    /// Below this score the backend does not return the candidate.
    pub backend_confidence: f32,
    /// Below this score the policy does not admit the candidate.
    pub min_confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayerRef {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Timing {
    pub wait_ms: f64,
    pub processing_ms: f64,
}

/// Hash of the effective policy the engine ran with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileHash(pub String);

/// A quantity reached against the one required.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Progress<T> {
    pub have: T,
    pub need: T,
}

/// A new count that has not stabilized yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pending {
    pub counts: Counts,
    /// Source seconds it has held, against the required dwell.
    pub duration: Progress<f64>,
}

/// Is someone in the bed?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedState {
    Empty,
    /// At least one visible occupant in the bed's stable count.
    Seen,
    /// Nobody seen, with retained occupants.
    Held,
    /// The bed geometry does not apply to the image.
    Unknown,
}

/// Someone who stopped being seen in bed without an observed exit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Held {
    pub bbox: Rect,
    /// The last sample they were seen in.
    pub last_seen: SampleRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Bed {
    pub state: BedState,
    /// Occupants seen in the bed, in the stable count. It is belief: if the
    /// scene did not come from an observation, `republished` says so.
    pub seen: u32,
    pub since: Option<SampleRef>,
    pub held: Vec<Held>,
    /// Stable bodies in the bed that are not occupants: they appeared without
    /// anyone coming from the floor. They do not count in the room.
    pub unexplained: u32,
}

/// Off the bed, no longer being seen is leaving the room: nobody is retained.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Floor {
    pub seen: u32,
}

/// The subject: the room's only occupant, if there is exactly one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Subject {
    pub zone: Zone,
    pub bbox: Rect,
    pub confidence: f32,
    pub provenance: Provenance,
    pub pose: Option<Pose>,
    pub depth: Option<SubjectDepth>,
    /// Second level (902): absent without a pose or without bed geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lying: Option<Lying>,
    /// Second level (904): absent without a pose or without per-point depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<Posture>,
}

/// What was admitted in this sample, apart from the stabilized belief.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    pub bodies: BodiesEvidence,
    pub faces: FacesEvidence,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodiesStatus {
    Seen,
    Unzoned,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodiesEvidence {
    pub status: BodiesStatus,
    pub items: Vec<BodySeen>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodySeen {
    pub zone: Option<Zone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_bed: Option<bool>,
    pub bbox: Rect,
    pub confidence: f32,
    pub source: BodySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FacesStatus {
    Ok,
    Empty,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FacesEvidence {
    pub status: FacesStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<SkipReason>,
    pub items: Vec<FaceSeen>,
}

impl Default for FacesEvidence {
    fn default() -> Self {
        Self {
            status: FacesStatus::Skipped,
            reason: Some(SkipReason::NotRequested),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FaceSeen {
    pub bbox: Rect,
    pub confidence: f32,
    pub on_subject: bool,
}

/// A republished scene: what happened and when what it claims is from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Republished {
    pub reason: RepublishReason,
    /// The last observed sample; `None` if there has been none yet.
    pub observed_at: Option<SampleRef>,
}

/// Why an event happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
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
