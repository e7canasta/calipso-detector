//! What every contract version shares: the publisher's identity, the profile,
//! the parts of the scene no version changes (the bed, the floor, the subject,
//! the evidence, the republication) and the causes. It also re-exports the
//! domain vocabulary every version publishes. No version depends on another:
//! each re-exports this and defines its own.

// The domain vocabulary every version publishes: its modules own it.
pub use crate::attention::{Attention, Look, LookOutcome, LookResult, SkipReason};
pub use crate::belief::{BedPosture, BedPostureSeen, BedPostureState, Parts};
pub use crate::body::{BedPoint, BedSite, BodyPoint, Joint, Keypoint, Pose, SubjectDepth};
pub use crate::conclusion::RepublishReason;
pub use crate::evidence::{BodySource, RejectReason};
use crate::geom::Rect;
pub use crate::head::{HeadEvidence, HeadPlace, HeadPlaceState};
pub use crate::look::{Failure, LookKind};
pub use crate::map::{Calibration, FrameInfo, Geometry, MapState};
pub use crate::provenance::Provenance;
pub use crate::refine::{
    BodyGroup, BodyGroups, Disparities, Head, Lying, LyingState, Posture, PostureState, Verdict,
    VerdictState,
};
pub use crate::sensor::SensorState;
pub use crate::time::{SampleRef, SessionId, SourceSecs};
pub use crate::verification::{HeldCheck, HeldCheckState, Verifier};
pub use crate::zone::{Counts, Zone};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identity of the publishing shell; the core does not build these values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    /// The site the camera belongs to (a subject token).
    pub site: String,
    /// The camera within the site (a subject token).
    pub camera: String,
    /// One per process start: a new run restarts every numbering.
    pub run_id: String,
    /// The binary that published.
    pub build: Build,
}

/// The publishing binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Build {
    /// The crate version.
    pub version: String,
    /// The commit it was built from.
    pub git: String,
}

/// The configuration the engine ran with, as the consumer needs to compare runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    /// The camera layer of the profile.
    pub camera: Option<LayerRef>,
    /// The resident layer of the profile; `None` while there is none.
    pub resident: Option<LayerRef>,
    /// Two runs with the same hash believed with the same policy and map.
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
    /// The look this sense answers.
    pub kind: LookKind,
    /// The model file, without its path.
    pub model: String,
    /// Below this score the backend does not return the candidate.
    pub backend_confidence: f32,
    /// Below this score the policy does not admit the candidate.
    pub min_confidence: Option<f32>,
    /// The pose only: below this score a keypoint is not seen, and the core
    /// neither measures nor reasons with it. Absent in every other sense.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_keypoint_score: Option<f32>,
}

/// A layer of the profile, by name and revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayerRef {
    /// The layer's name.
    pub id: String,
    /// Its revision; `0` while layers are not versioned.
    pub version: u32,
}

/// How long the shell took with a sample, in wall-clock milliseconds. It is
/// measured by the shell and never decides anything (rule 9).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Timing {
    /// From the sample's arrival to the start of its turn.
    pub wait_ms: f64,
    /// The whole turn: decoding, every look and the projection.
    pub processing_ms: f64,
}

/// Hash of the effective policy the engine ran with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileHash(pub String);

/// A quantity reached against the one required.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Progress<T> {
    /// What has been reached.
    pub have: T,
    /// What is required.
    pub need: T,
}

/// A new count that has not stabilized yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pending {
    /// The new count per zone, waiting to hold.
    pub counts: Counts,
    /// Source seconds it has held, against the required dwell.
    pub duration: Progress<f64>,
}

/// Is someone in the bed?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedState {
    /// Nobody in the bed.
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
    /// Where they were last seen, in image pixels.
    pub bbox: Rect,
    /// The last sample they were seen in.
    pub last_seen: SampleRef,
    /// Whether a verifier looked since: only with a verifier configured
    /// (white book, volume III). It never moves the count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<HeldCheck>,
}

/// The bed: who is believed in it, seen or held.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Bed {
    /// Empty, seen, held or unknown.
    pub state: BedState,
    /// Occupants seen in the bed, in the stable count. It is belief: if the
    /// scene did not come from an observation, `republished` says so.
    pub seen: u32,
    /// Since when `state` holds; `None` before the first turn.
    pub since: Option<SampleRef>,
    /// Who is retained in the bed without being seen, oldest first.
    pub held: Vec<Held>,
    /// Stable bodies in the bed that are not occupants: they appeared without
    /// anyone coming from the floor. They do not count in the room.
    pub unexplained: u32,
}

/// Off the bed, no longer being seen is leaving the room: nobody is retained.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Floor {
    /// Occupants seen off the bed, in the stable count.
    pub seen: u32,
}

/// The subject: the room's only occupant, if there is exactly one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Subject {
    /// Where the subject is: in bed or on the floor.
    pub zone: Zone,
    /// Their box, in image pixels.
    pub bbox: Rect,
    /// The detector's score for that box.
    pub confidence: f32,
    /// Whether the box was seen in this turn or is held from before.
    pub provenance: Provenance,
    /// The subject's pose, when the policy takes it.
    pub pose: Option<Pose>,
    /// The subject's depth, when the policy measures it.
    pub depth: Option<SubjectDepth>,
    /// Second level: absent without a pose or without bed geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lying: Option<Lying>,
    /// Second level: absent without a pose or without per-point depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<Posture>,
    /// Second level: the arbiter's verdict, only in bed for now (R16, 1605).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    /// Layer 1: the bed posture of the resident left lying, when the profile
    /// follows it (R18, 1806).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_posture: Option<BedPosture>,
    /// The occupancy of the subject's parts against the bed and the headboard,
    /// with what is retained unseen (white book, volume I, ch. 09). Absent
    /// without the bed's geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Parts>,
}

/// What was admitted in this sample, apart from the stabilized belief.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// The bodies admitted from the person detectors.
    pub bodies: BodiesEvidence,
    /// The faces admitted from the face detector.
    pub faces: FacesEvidence,
}

/// Whether this sample has bodies to show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodiesStatus {
    /// The detector read and each body has its zone.
    Seen,
    /// The detector read, but the bed geometry does not apply: no zones.
    Unzoned,
    /// No body reading in this sample (the detector failed or nothing was
    /// decoded).
    #[default]
    Unavailable,
}

/// The bodies of this sample.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodiesEvidence {
    /// Whether there was a reading, with or without zones.
    pub status: BodiesStatus,
    /// One per admitted person, after consolidating overlapping boxes.
    pub items: Vec<BodySeen>,
}

/// A body admitted in this sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodySeen {
    /// In bed or on the floor; `None` when the bed geometry does not apply.
    pub zone: Option<Zone>,
    /// On the floor: whether it touches the grown bed. Absent in bed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_bed: Option<bool>,
    /// The box, in image pixels.
    pub bbox: Rect,
    /// The detector's score.
    pub confidence: f32,
    /// Which detector saw it.
    pub source: BodySource,
}

/// What happened with faces in this sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FacesStatus {
    /// Looked for, and at least one admitted.
    Ok,
    /// Looked for, and none admitted.
    Empty,
    /// The face detector failed.
    Failed,
    /// Not looked for; `reason` says why.
    Skipped,
}

/// The faces of this sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FacesEvidence {
    /// Whether faces were looked for and what came of it.
    pub status: FacesStatus,
    /// Why faces were not looked for; only with `skipped`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<SkipReason>,
    /// The admitted faces.
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

/// A face admitted in this sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FaceSeen {
    /// The box, in image pixels.
    pub bbox: Rect,
    /// The face detector's score.
    pub confidence: f32,
    /// Whether it lies on the subject seen in this sample.
    pub on_subject: bool,
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
