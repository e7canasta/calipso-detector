//! The wire: the published vocabulary of `scene.v4` and `events.v4`. Types
//! only, with no logic and no domain (ADR 2026-11-02, crates-del-lenguaje).
//! `scene-core` re-exports each one, so its paths do not change.
#![warn(missing_docs)]
#![warn(unreachable_pub)]

use scene_geom::{Quad, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- moved from scene-core/src/time.rs
/// Source session. Source time is only comparable within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SessionId(pub u64);

/// Seconds of source time (video PTS) within a session.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
pub struct SourceSecs(pub f64);

/// Which sample and when, in source time. It is the domain's only clock
/// (rule 9).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleRef {
    /// The stream session: a reconnection starts a new one.
    pub session: SessionId,
    /// The sample's number, increasing within the run.
    pub sequence: u64,
    /// The sample's presentation time, comparable only within its session.
    pub source_secs: SourceSecs,
}

// --- moved from scene-core/src/provenance.rs
/// Where a claim comes from (rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Provenance {
    /// Observed in this sample.
    Current,
    /// Observed earlier and retained without a current observation.
    Retained {
        /// The last sample it was observed in.
        since: SampleRef,
    },
    /// Computed earlier and still valid because the subject did not move.
    Stable {
        /// The sample it was computed in.
        since: SampleRef,
    },
}

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

// --- moved from scene-core/src/zone.rs
/// The zones of the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Zone {
    /// The delimited bed.
    Bed,
    /// The rest of the room.
    Floor,
}

/// What is seen in each zone, stabilized or yet to stabilize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Counts {
    /// Seen off the bed.
    pub floor: u32,
    /// Seen in the bed.
    pub bed: u32,
}

// --- moved from scene-core/src/look.rs
/// What was asked to be looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LookKind {
    /// The main person detector.
    Detect,
    /// The face detector.
    Faces,
    /// The finer detector over the bed, when the subject is lost.
    Reinforce,
    /// The subject's pose.
    Pose,
    /// A doubtful body's pose, before believing.
    PoseBody,
    /// The depth at image points.
    Depth,
    /// The depth of the empty bed, to learn it.
    Calibrate,
    /// The slow verifier, asked about a crop.
    Verify,
    /// A crop kept for a burst to the verifier.
    Keep,
}

impl LookKind {
    /// Its published name (`detect`, `faces`…).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Faces => "faces",
            Self::Reinforce => "reinforce",
            Self::Pose => "pose",
            Self::PoseBody => "pose_body",
            Self::Depth => "depth",
            Self::Calibrate => "calibrate",
            Self::Verify => "verify",
            Self::Keep => "keep",
        }
    }
}

impl std::fmt::Display for LookKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// A failure is data: the shell returns it and the ledger publishes it as is.
// The published schema names it `FailureKind`: renaming it changes the contract.
/// How a look failed, as published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "FailureKind")]
#[non_exhaustive]
pub enum Failure {
    /// The shell could not complete the probe (shutdown).
    Cancelled,
    /// The sense ran and failed (model or backend error).
    Model,
}

// --- moved from scene-core/src/common.rs
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

// --- moved from scene-core/src/v4.rs
/// Which schema a v4 message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub enum SchemaId {
    /// One scene per sample.
    #[serde(rename = "calipso.scene.v4")]
    SceneV4,
    /// Every event, in order.
    #[serde(rename = "calipso.events.v4")]
    EventsV4,
}

/// The envelope of every v4 message: the usual one, with v4's `SchemaId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope<T> {
    /// What the body is: a scene or an event.
    pub schema: SchemaId,
    /// Who published it.
    pub source: Source,
    /// Contiguous from 1 per run and per schema; a gap is a lost message.
    pub seq: u64,
    /// Source time of what is claimed: without a new sample, that of the last
    /// observed one.
    pub observed_at: SampleRef,
    /// Wall clock; set by publishing, never by the core.
    pub published_at_ms: u64,
    /// The configuration the engine ran with.
    pub profile: Profile,
    /// Events only: unique per event, for deduplication on redelivery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// Scenes only: how long the shell took with the sample.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<Timing>,
    /// Scenes only: the `seq` of the last event published before this scene,
    /// so a consumer knows which events the scene already reflects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_seq: Option<u64>,
    /// The scene or the event.
    pub body: T,
}

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
#[non_exhaustive]
pub enum Phase {
    /// The bed has no history yet: whoever is clearly seen in it is adopted.
    Booting,
    /// Someone climbed into bed from the floor: the bed only rises from it.
    Running,
}

/// The triple `[R, B, H]`: occupants of the room, of the bed (seen and
/// retained) and retained. They are counts: `0 ≤ H ≤ B ≤ R`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rbh(pub u32, pub u32, pub u32);

/// The pair `[F, V]`: admitted bodies seen on the floor and in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Fv(pub u32, pub u32);

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

// --- moved from scene-core/src/ledger.rs
/// Why nothing was looked at (rule 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkipReason {
    /// This sense was not requested.
    NotRequested,
    /// No body and no delimited headboard: nowhere to look for faces.
    NoFaceRegion,
    /// This loss was already reinforced: once per loss.
    LatchSpent,
    /// Nobody was seen in bed: there is no loss to reinforce.
    NoSubjectInBed,
    /// The geometry does not apply: where the bed is, is unknown.
    NoBedGeometry,
    /// The subject did not move since the last pose: the previous one holds.
    SubjectStill,
    /// The subject was posed in this turn as a doubtful body, before
    /// believing: that pose is read without looking again.
    PosedBefore,
    /// The subject is retained, not seen in this sample.
    NoCurrentSubject,
    /// Without a pose there are no points to measure depth at.
    NoPose,
    /// Someone is in the room: a person is not learned as the bed.
    RoomOccupied,
    /// Faces are in sight: someone may be covering the bed.
    FacesPresent,
    /// The last attempt failed and the retry delay has not passed yet.
    RetryWait,
    /// A verification is already in flight: one at a time per camera.
    VerifyBusy,
    /// The camera's verification budget for the hour is spent.
    BudgetSpent,
    /// The doubt cleared while the burst was being collected: nothing is asked
    /// (white book, volume III, ch. 12, V17).
    BurstResolved,
    /// What the burst asked about no longer holds: it is dropped unasked.
    BurstStale,
}

// --- moved from scene-core/src/mod.rs
/// Which detector the admitted body came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BodySource {
    /// The main detector, over the whole frame.
    Primary,
    /// The finer detector, over the bed.
    Reinforcement,
}

/// Why a candidate did not count as evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RejectReason {
    /// Another class than a person.
    NotPerson,
    /// Below the policy's minimum confidence.
    LowConfidence,
    /// Outside the region where the look was requested.
    OutsideRegion,
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

/// New places of the parts, waiting to hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsPending {
    /// The places waiting: `bed IN.OUT : headboard IN.OUT : BFK`.
    pub places: String,
    /// Source seconds they have held, against the required dwell.
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

// --- moved from scene-core/src/verification.rs
/// Who looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Verifier {
    /// The slow verifier: a vision-language model over a crop of the bed.
    Vlm,
}

// --- moved from scene-core/src/posture.rs
/// A group of the body's keypoints: the median depth of those seen and
/// measured, and how many of its keypoints were seen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyGroup {
    /// Median depth of its seen, measured keypoints; `None` if none.
    pub depth: Option<f32>,
    /// How many of its keypoints were seen (score at least the minimum).
    pub seen: u32,
    /// How many keypoints the group has.
    pub of: u32,
}

/// The groups the posture is measured with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyGroups {
    /// Nose, eyes and ears.
    pub head: BodyGroup,
    /// Both shoulders.
    pub shoulders: BodyGroup,
    /// Both hips: the reference.
    pub pelvis: BodyGroup,
}

/// Each disparity against the pelvis, `(a − pelvis) / mean`: relative,
/// without scale. `upper_pelvis` (head and shoulders together) decides.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Disparities {
    /// Head and shoulders against the pelvis: the one that decides.
    pub upper_pelvis: Option<f32>,
    /// The shoulders alone against the pelvis.
    pub shoulders_pelvis: Option<f32>,
    /// The head alone against the pelvis.
    pub head_pelvis: Option<f32>,
    /// Head and shoulders against the bed's centre: the stand-in for the
    /// hips, the baseline when the blanket hides them (R17). Absent in scenes
    /// recorded before it, and without the bed's points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_bed_centre: Option<f32>,
}

/// The posture by how head, torso and hips are ordered in depth.
/// Monocular depth separates lying from upright by the magnitude of the
/// disparity, not by its sign (measured on real nights): sitting and standing are not
/// distinguished and are published together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostureState {
    /// The hips clearly closer to the camera than the head and shoulders.
    Lying,
    /// Sitting, getting up or standing. Also reads `sitting` and `standing`,
    /// the values published before 904-5, so nights recorded with them can be
    /// replayed; publishes only `upright`.
    #[serde(alias = "sitting", alias = "standing")]
    Upright,
    /// One of the two groups has no measured keypoints.
    Unknown,
}

// --- moved from scene-core/src/lying.rs
/// Is the subject truly lying down? Second level: it changes no count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LyingState {
    /// Torso, hips, most keypoints and the visible feet inside the bed.
    Confirmed,
    /// Torso, hips and most keypoints inside; the feet are not visible.
    Assumed,
    /// The torso, the hips, most keypoints or a visible foot outside the bed.
    No,
    /// The torso or the hips are not visible.
    Unknown,
}

/// What the second level concludes from the pose over the bed plane, with that
/// pose's provenance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lying {
    /// Confirmed, assumed, no or unknown.
    pub state: LyingState,
    /// That of the pose it was concluded from.
    pub provenance: Provenance,
}

// --- moved from scene-core/src/verdict.rs
/// A head of the second level: who supports a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Head {
    /// The depth disparity of head and shoulders against the pelvis (904, 1604).
    Disparity,
}

// --- moved from scene-core/src/verification.rs
/// How much observation backs a body held in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeldCheckState {
    /// Nobody looked since it stopped being seen.
    Unverified,
    /// A verifier saw it: the belief has an observation behind it.
    Confirmed,
    /// A verifier looked and did not see it. It is not released: the blanket
    /// hides it from every verifier (book 4, frontier 3).
    NotConfirmed,
}

// --- moved from scene-core/src/body.rs
/// What the head's place was concluded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeadEvidence {
    /// An accepted face.
    Face,
    /// The pose's head keypoints (nose, eyes, ears) seen.
    PoseHead,
}

// --- moved from scene-core/src/map.rs
/// Whether the delimited bed matches the incoming image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Geometry {
    /// The image has the resolution the bed was delimited for.
    Valid,
    /// The resolution is not the declared one: "in bed" cannot be claimed.
    Invalid {
        /// The resolution the bed was delimited for.
        declared: FrameInfo,
        /// The resolution of the incoming image.
        actual: FrameInfo,
    },
}

// --- moved from scene-core/src/shadow.rs
/// A point of the image and where it falls in the bed's coordinates on a
/// plane: `u` across, `v` from the foot (0) to the headboard (1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlanePoint {
    /// In image pixels.
    pub x: f32,
    /// In image pixels.
    pub y: f32,
    /// Across the bed: 0 and 1 are its sides.
    pub u: f32,
    /// Along the bed: 0 the foot, 1 the headboard.
    pub v: f32,
}

// --- moved from scene-core/src/verification.rs
/// The provenance of a held body: whether a verifier looked, and when.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HeldCheck {
    /// Unverified, confirmed or not confirmed.
    pub state: HeldCheckState,
    /// Who looked; `None` while unverified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Verifier>,
    /// The sample it looked at; `None` while unverified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looked: Option<SampleRef>,
}

// --- moved from scene-core/src/body.rs
// --- moved from scene-core/src/reading.rs
/// What we believe of the resident from the places of their parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PartsReading {
    /// Neither the box nor the groin in the bed.
    OutOfBed,
    /// The groin in the bed, the box out: getting in or out.
    OnTheEdge,
    /// The shoulders and the face or the head at the headboard, nothing out.
    Lying,
    /// Something out of the bed while some of the torso is still at the
    /// headboard: leaving from lying.
    TryingToLeave,
    /// Everything in the bed, the whole boxes in it, nothing at the headboard.
    SittingUp,
    /// Not everything in the bed, nothing at the headboard.
    SittingInBed,
    /// The places match no rule.
    Undecided,
}

/// A pose keypoint, in whole-frame coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Keypoint {
    /// Pixels from the left edge.
    pub x: f32,
    /// Pixels from the top edge.
    pub y: f32,
    /// The pose model's confidence in this keypoint.
    pub score: f32,
}

/// A body joint per COCO-17: the keypoint's index in the pose gives its
/// name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Las descripciones de estos items serían parte del schema publicado; cambiarlo
// es un cambio de contrato (regla 13), no una doc.
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Joint {
    Nose,
    LeftEye,
    RightEye,
    LeftEar,
    RightEar,
    LeftShoulder,
    RightShoulder,
    LeftElbow,
    RightElbow,
    LeftWrist,
    RightWrist,
    LeftHip,
    RightHip,
    LeftKnee,
    RightKnee,
    LeftAnkle,
    RightAnkle,
}

impl Joint {
    /// The 17 joints, in the order the model gives them.
    pub const COCO: [Joint; 17] = [
        Joint::Nose,
        Joint::LeftEye,
        Joint::RightEye,
        Joint::LeftEar,
        Joint::RightEar,
        Joint::LeftShoulder,
        Joint::RightShoulder,
        Joint::LeftElbow,
        Joint::RightElbow,
        Joint::LeftWrist,
        Joint::RightWrist,
        Joint::LeftHip,
        Joint::RightHip,
        Joint::LeftKnee,
        Joint::RightKnee,
        Joint::LeftAnkle,
        Joint::RightAnkle,
    ];
}

/// A published pose point: where it is, how confident the model is and, if
/// measured, its depth on the depth model's scale, which is relative and not
/// metres. An unmeasured point carries `depth: null`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyPoint {
    /// The COCO-17 joint; `None` beyond the 17th keypoint.
    pub name: Option<Joint>,
    /// Pixels from the left edge.
    pub x: f32,
    /// Pixels from the top edge.
    pub y: f32,
    /// The pose model's confidence in this keypoint.
    pub score: f32,
    /// Its depth, if it was measured.
    pub depth: Option<f32>,
}

/// The subject's pose, with its own provenance (rule 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pose {
    /// In COCO-17 order.
    pub keypoints: Vec<BodyPoint>,
    /// Taken in this sample, or stable since a sample in which the subject
    /// did not move.
    pub provenance: Provenance,
}

/// The subject's depth (the median of its measured points), with its own
/// provenance. Kept for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubjectDepth {
    /// On the depth model's scale (the name says metres, the value is
    /// relative).
    pub meters: f32,
    /// Measured in this sample, or stable with the pose.
    pub provenance: Provenance,
}

/// A fixed place on the bed where depth is read in the same look as the body:
/// covered, the bed stands in for what the pose does not see (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedSite {
    /// The middle of the bed: lying, the hips, under the blanket or not.
    Centre,
}

impl BedSite {
    /// Every site, in the order they are measured and published.
    pub const ALL: [BedSite; 1] = [BedSite::Centre];
}

/// A published bed point: which site, where it falls in the image and its
/// depth, read in the same depth map as the body's keypoints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPoint {
    /// The site.
    pub name: BedSite,
    /// Pixels from the left edge.
    pub x: f32,
    /// Pixels from the top edge.
    pub y: f32,
    /// Its depth on the depth model's scale, if it was read.
    pub depth: Option<f32>,
}

/// Where the head is, and what said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HeadPlace {
    /// At the headboard, elsewhere, or unknown.
    pub state: HeadPlaceState,
    /// What it was concluded from; `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<HeadEvidence>,
}

/// Where the head is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeadPlaceState {
    /// At the headboard: lying in bed as a bed is used, the hips are at its
    /// centre.
    AtHeadboard,
    /// Seen in bed, below the headboard: lying here is not lying as a bed is
    /// used (R18, 1807).
    BelowHeadboard,
    /// Seen past the bed's sides (`u < 0` or `u > 1`): on the way out.
    AtSide,
    /// Seen, but neither at the headboard, in bed nor past its sides; or seen
    /// without the bed's plane.
    Elsewhere,
    /// Without the headboard delimited, or without a face or a head in sight.
    Unknown,
}

impl HeadPlace {
    /// Nothing to say where the head is.
    pub const UNKNOWN: HeadPlace = HeadPlace {
        state: HeadPlaceState::Unknown,
        by: None,
    };

    /// The head's place, concluded from what it was seen in.
    pub fn of(state: HeadPlaceState, by: HeadEvidence) -> Self {
        Self {
            state,
            by: Some(by),
        }
    }
}

// --- moved from scene-core/src/attention/ledger.rs
/// What a look returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum LookResult {
    /// The sense read: how many candidates, and why those that did not count
    /// were discarded.
    Read {
        /// How many the sense returned, before admission.
        candidates: u32,
        /// The discarded ones, grouped by reason.
        #[serde(with = "rejections")]
        #[schemars(with = "Vec<Rejected>")]
        rejected: Vec<RejectReason>,
    },
    /// The sense failed: it proves nothing (rule 1).
    Failed {
        /// How it failed.
        failure: Failure,
    },
    /// The verifier took the request: the answer comes in a later turn.
    Dispatched,
    /// The shell kept the crop for a burst.
    Kept {
        /// How sharp the shell measured it.
        sharpness: f32,
        /// Whether it counts for the burst: sharp enough for this camera.
        counted: bool,
    },
}

/// An entry of attention's ledger (rule 12): what was decided to look at or
/// not, and what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Look {
    /// Which sense.
    pub kind: LookKind,
    /// Asked, with what came back, or skipped, with why.
    pub outcome: LookOutcome,
}

/// What came of a look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum LookOutcome {
    /// A look was requested.
    Asked {
        /// `None`: the whole frame.
        region: Option<Rect>,
        /// What the sense returned.
        result: LookResult,
        /// What the shell measured; the core only records it (rule 9).
        spent_ms: f64,
    },
    /// It was decided not to look, with a reason.
    Skipped {
        /// Why not.
        reason: SkipReason,
    },
}

// The published form groups the rejections; the internal ledger keeps each reason.
#[derive(Serialize, Deserialize, JsonSchema)]
struct Rejected {
    /// Why they did not count.
    reason: RejectReason,
    /// How many, for that reason.
    count: u32,
}

mod rejections {
    use super::{RejectReason, Rejected};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub(super) fn serialize<S: Serializer>(
        reasons: &[RejectReason],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut groups: Vec<Rejected> = Vec::new();
        for reason in reasons {
            if let Some(group) = groups.iter_mut().find(|g| g.reason == *reason) {
                group.count = group.count.saturating_add(1);
            } else {
                groups.push(Rejected {
                    reason: *reason,
                    count: 1,
                });
            }
        }
        groups.serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<RejectReason>, D::Error> {
        let groups = Vec::<Rejected>::deserialize(deserializer)?;
        Ok(groups
            .into_iter()
            .flat_map(|g| {
                std::iter::repeat_n(g.reason, usize::try_from(g.count).unwrap_or(usize::MAX))
            })
            .collect())
    }
}

impl LookResult {
    /// The sense read `n` things and none were discarded.
    pub fn read(n: usize) -> Self {
        LookResult::Read {
            candidates: u32::try_from(n).unwrap_or(u32::MAX),
            rejected: Vec::new(),
        }
    }

    /// A look whose sense failed.
    pub fn failed(failure: Failure) -> Self {
        LookResult::Failed { failure }
    }
}

impl Look {
    /// The result, if a look was requested: a projection of the outcome, not a
    /// decision.
    pub fn result(&self) -> Option<&LookResult> {
        match &self.outcome {
            LookOutcome::Asked { result, .. } => Some(result),
            LookOutcome::Skipped { .. } => None,
        }
    }
}

/// What was looked at during the turn, in order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Attention {
    /// One per sense considered, in the order of the turn.
    pub looks: Vec<Look>,
}

/// The cell of a transition: `before:seen:after`, the natural key of the row.
/// Each part is written as decimal digits if all its counts are below
/// 10; otherwise as a tuple (`(12,1,1)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Cell(
    #[schemars(regex(
        pattern = r"^(\d{3}|\(\d+,\d+,\d+\)):(\d{2}|\(\d+,\d+\)):(\d{3}|\(\d+,\d+,\d+\))$"
    ))]
    pub String,
);

impl Cell {
    /// As published: `before:seen:after`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// --- moved from scene-core/src/contract/common.rs (FacesEvidence's trivial Default)
impl Default for FacesEvidence {
    fn default() -> Self {
        Self {
            status: FacesStatus::Skipped,
            reason: Some(SkipReason::NotRequested),
            items: Vec::new(),
        }
    }
}

// --- moved from scene-core/src/common.rs
/// The occupancy of the subject's parts: the second trama of occupancy, the
/// complement of layer 0 with its shape. In its notation: the positions
/// `cfhsaip` against the bed, `fhs` against the headboard and the flags `BFK`
/// (whole boxes inside), two bits each (IN, OUT; both: split); `-` where the
/// sense did not look; HELD where not seen now, kept from where it was last
/// seen (white book, volume I, ch. 09).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Parts {
    /// The places believed: `bed IN.OUT.HELD : headboard IN.OUT.HELD :
    /// BFK.HELD`. E.g. `1011111.0000000.0011111:010.001.011:100.000`.
    pub state: String,
    /// What this sample saw: `bed IN.OUT : headboard IN.OUT : BFK`. E.g.
    /// `1-11111.0-00000:-10.-01:1--`.
    pub seen: String,
    /// Since when the stable places hold: the sample of their last change;
    /// `None` before the first.
    pub since: Option<SampleRef>,
    /// New places that have not held long enough yet.
    pub pending: Option<PartsPending>,
    /// The cell of the last change: `before | seen | after`, the places
    /// without HELD.
    pub last_cell: Option<String>,
    /// What we believe of the resident from the places: the semantic reading
    /// of the trama (white book, volume I, ch. 10).
    pub reading: PartsReading,
    /// Since when the reading holds.
    pub reading_since: SampleRef,
}

/// Is someone in the bed?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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

/// What happened with faces in this sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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

// --- moved from scene-core/src/shadow.rs
/// The plane a point was measured on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ShadowPlane {
    /// The bed's footprint on the floor.
    Floor,
    /// The mattress, when there is no footprint.
    Mattress,
}

/// The shadow of one body. It decides nothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyShadow {
    /// The box's centre on the mattress: what decides the zone today.
    pub centroid: PlanePoint,
    /// The centre of the box's lower edge, where a standing body rests.
    pub support: PlanePoint,
    /// Where the support was measured.
    pub support_plane: ShadowPlane,
    /// How far the support is from the bed, in metres; zero inside. Absent
    /// without the bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_distance_m: Option<f32>,
    /// The share of the box over the mattress, on an 8 × 8 grid.
    pub share: f32,
    /// The zone the support says: in bed within the shadow's margin, on the
    /// floor past it. Absent without the bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<Zone>,
    /// On the floor for the shadow: whether the support is within the
    /// shadow's distance of the bed. Absent in bed or without the bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near_bed: Option<bool>,
}

// --- moved from scene-core/src/common.rs
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
#[non_exhaustive]
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
    /// Where it falls for the geometry's shadow: absent unless the profile
    /// asks for it. It decides nothing (white book, volume VII, ch. 11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<BodyShadow>,
    /// The hip–knee–ankle angle, in degrees, when the body was doubtful and
    /// its pose was asked before believing: bent, the room put it in bed
    /// (white book, volume VII, ch. 15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knee_deg: Option<f32>,
}

// --- moved from scene-core/src/belief/bed_posture/mod.rs
/// How the resident in bed is, as layer 1 believes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedPostureState {
    /// In bed, not yet believed lying: they just came in, or were there at
    /// startup. From here only lying is reported.
    InBed,
    /// Lying: held in bed, where upright would be seen; or seen with the head
    /// at the headboard and the disparity of lying.
    Lying,
    /// Sat up, after lying: the head seen away from the headboard. Sliding
    /// down the bed is sitting up too, on purpose: it deserves a look (G-13).
    SittingUp,
    /// Getting out, after lying: the head or the feet seen past the bed's
    /// sides.
    GettingOut,
}

impl BedPostureState {
    /// The state as the contract names it: a cell is written with it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InBed => "in_bed",
            Self::Lying => "lying",
            Self::SittingUp => "sitting_up",
            Self::GettingOut => "getting_out",
        }
    }
}

/// What a sample brought to layer 1, as published: held in bed when layer 0
/// holds the resident (there is nothing of theirs to see), otherwise what
/// layer 1 perceived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BedPostureSeen {
    /// Held in bed, alone: unseen without leaving.
    Held,
    /// The head seen away from the headboard, but not past the bed's sides:
    /// by definition, not lying.
    HeadAway,
    /// The head seen past the bed's sides.
    HeadSide,
    /// A foot seen past the bed's sides: getting up from below.
    LegsOut,
    /// The head at the headboard and the disparity of lying.
    HeadboardLying,
    /// The head at the headboard and the disparity of sitting up: against the
    /// headboard.
    HeadboardUpright,
    /// Nothing that says how they are: the state holds.
    Unknown,
}

impl BedPostureSeen {
    /// What was seen as the contract names it: a cell is written with it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::HeadAway => "head_away",
            Self::HeadSide => "head_side",
            Self::LegsOut => "legs_out",
            Self::HeadboardLying => "headboard_lying",
            Self::HeadboardUpright => "headboard_upright",
            Self::Unknown => "unknown",
        }
    }
}

// --- moved from scene-core/src/contract/v4.rs (the event's accessor)
impl Event {
    /// The fact's sample: the `observed_at` of its envelope.
    pub fn at(&self) -> SampleRef {
        match self {
            Self::Transition(e) => e.at,
            Self::Parts(e) => e.at,
            Self::Derived(e) => e.at,
            Self::Sensor(e) => e.at,
            Self::Map(e) => e.at,
            Self::BedPosture(e) => e.at,
        }
    }
}

// --- moved from scene-core/src/v4.rs
/// The occupancy: a single shape for all of v4 (before, after, each fact).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Occupancy {
    /// Occupants of the room, of the bed and retained in it.
    pub rbh: Rbh,
    /// The machine's state those counts name.
    pub mode: Mode,
    /// `unknown` if the geometry does not apply, even with counts.
    pub bed: BedState,
    /// Bodies in the bed that are not occupants (rule 6).
    pub unexplained: u32,
}

/// What we believed, since when and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Before {
    /// The occupancy we believed.
    #[serde(flatten)]
    pub occupancy: Occupancy,
    /// The sample of the previous transition; `None` on the first.
    pub since: Option<SampleRef>,
    /// Source seconds since `since`; `None` without `since` or across sessions.
    pub lasted_secs: Option<f64>,
}

/// The span that held: from which sample and for how many source seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    /// The first sample that saw the new count.
    pub first: SampleRef,
    /// Source seconds it held before counting.
    pub secs: f64,
}

/// What happened during the span and decided the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Context {
    /// How many appeared far from the bed.
    pub far: u32,
    /// How many appeared in passing.
    pub passing: u32,
}

/// What we saw: the count that held, the span and its context.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Seen {
    /// The bodies seen on the floor and in bed, as they held.
    pub fv: Fv,
    /// How long they held before counting.
    pub span: Span,
    /// What happened meanwhile, which decided the cell.
    pub context: Context,
}

/// A layer-0 fact: what happened to someone, in the order it happened.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactBody {
    /// Someone appeared on the floor: they entered the room.
    Entered,
    /// Someone stopped being seen on the floor: they left the room.
    LeftRoom,
    /// Someone appeared in bed coming from the floor; with cause `baseline`,
    /// they were found in bed at startup.
    LayDown,
    /// Someone was seen moving from the bed to the floor.
    LeftBed,
    /// Someone stopped being seen in bed without leaving: they are retained.
    HeldInBed {
        /// The last sample they were seen in.
        last_seen: SampleRef,
    },
    /// Someone retained in bed was seen again.
    Reappeared {
        /// The sample since which they were retained.
        held_since: SampleRef,
    },
}

/// A fact within its transition, with the occupancy it leaves.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fact {
    /// What happened.
    #[serde(flatten)]
    pub body: FactBody,
    /// Why it happened.
    pub cause: Cause,
    /// The occupancy this fact leaves, before the next one.
    pub after: Occupancy,
}

/// A layer-0 change: what we believed, what we saw, what we believe and the
/// facts, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transition {
    /// The sample in which the new count held.
    pub at: SampleRef,
    /// The row of the layer-0 table this change crossed.
    pub cell: Cell,
    /// The phase it was decided with.
    pub phase: Phase,
    /// What we believed, and since when.
    pub before: Before,
    /// What we saw hold.
    pub seen: Seen,
    /// The occupancy we believe at the end of the turn. Its occupants
    /// (`rbh`, `mode`, `bed`) are those of the last fact; `unexplained` is the
    /// scene's and may differ, because a body that is nobody comes and goes
    /// without a fact (rule 6). A transition with no facts still has it.
    pub after: Occupancy,
    /// What happened to each person, in order.
    pub facts: Vec<Fact>,
}

/// A change of the parts' places: the second trama of occupancy, the
/// complement of a layer-0 transition with its shape (white book, volume I,
/// ch. 09). The places are written `bed IN.OUT : headboard IN.OUT : BFK`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsChange {
    /// The sample in which the new places held.
    pub at: SampleRef,
    /// The row this change crossed: `before | seen | after`.
    pub cell: String,
    /// The places we believed, and since when.
    pub before: PartsBefore,
    /// What this sample saw, and how long the new places held.
    pub seen: PartsSeen,
    /// The places we believe.
    pub after: PartsAfter,
}

/// The parts' places before a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsBefore {
    /// The places, without HELD.
    pub places: String,
    /// The sample of the previous change; `None` on the first.
    pub since: Option<SampleRef>,
    /// Source seconds since `since`; `None` without `since` or across sessions.
    pub lasted_secs: Option<f64>,
}

/// What the sample of a parts' change saw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsSeen {
    /// The trama of the sample: `-` where the sense did not look.
    pub parts: String,
    /// The span the new places held.
    pub span: Span,
}

/// The parts' places after a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsAfter {
    /// The places, without HELD.
    pub places: String,
}

/// A layer-1 change: the bed posture of the resident left lying, with the
/// shape of a layer-0 transition. Only a change while it is followed is a
/// row of `contract/bed-posture-table.txt`; getting into and out of bed are
/// layer 0's events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPostureChange {
    /// The sample in which the new state held.
    pub at: SampleRef,
    /// The row this change crossed: `before | seen | after`.
    pub cell: String,
    /// The state we believed, and since when.
    pub before: BedPostureBefore,
    /// What this sample saw.
    pub seen: BedPostureSeen,
    /// The state we believe.
    pub after: BedPostureState,
}

/// The bed posture before a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPostureBefore {
    /// The state.
    pub state: BedPostureState,
    /// Its first sample; `None` if it was never dated.
    pub since: Option<SampleRef>,
    /// Source seconds since `since`; `None` without `since` or across sessions.
    pub lasted_secs: Option<f64>,
}

/// A change in what was published, computed by comparing turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivedBody {
    /// The mode changed.
    RoomChanged {
        /// The previous mode.
        from: Mode,
        /// The new mode.
        to: Mode,
    },
    /// The bed changed; it goes to `unknown` without a transition.
    BedChanged {
        /// The previous state.
        from: BedState,
        /// The new state.
        to: BedState,
    },
}

/// What changed in the reception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SensorBody {
    /// The reception changed.
    SensorChanged {
        /// The previous state.
        from: SensorState,
        /// The new state.
        to: SensorState,
    },
}

/// What the map learned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapBody {
    /// The map learned the depth of the empty bed.
    BedCalibrated {
        /// The empty bed's depth, on the depth model's scale.
        reference_m: f32,
    },
}

/// A change in what was published, with the occupancy at the end of the turn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DerivedEvent {
    /// Why it changed.
    pub cause: Cause,
    /// The sample in which it changed.
    pub at: SampleRef,
    /// What changed.
    #[serde(flatten)]
    pub body: DerivedBody,
    /// The occupancy at the end of the turn.
    pub after: Occupancy,
}

/// A reception change, reported by the shell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SensorEvent {
    /// Always `sensor_reported`.
    pub cause: Cause,
    /// The sample the shell reported it with.
    pub at: SampleRef,
    /// What changed.
    #[serde(flatten)]
    pub body: SensorBody,
    /// The occupancy, which a reception change does not touch.
    pub after: Occupancy,
}

/// What the map learned: not an occupancy fact.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapEvent {
    /// Always `map_learned`.
    pub cause: Cause,
    /// The sample it was learned in.
    pub at: SampleRef,
    /// What was learned.
    #[serde(flatten)]
    pub body: MapBody,
}

/// A message of `events.v4`. Within a turn they come out in order: `sensor`,
/// `transition`, `map`, `derived`, `parts`, `bed_posture`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A layer-0 change, with its facts.
    Transition(Transition),
    /// A change of the parts' places: the second trama of occupancy.
    Parts(PartsChange),
    /// A change in what was published, by comparing turns.
    Derived(DerivedEvent),
    /// A reception change.
    Sensor(SensorEvent),
    /// What the map learned.
    Map(MapEvent),
    /// A layer-1 change: the bed posture.
    BedPosture(BedPostureChange),
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

// --- moved from scene-core/src/map.rs
/// The geometry's shadow over the site (white book, volume VII, ch. 11): it
/// decides nothing; it is published to choose with data. Temporary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapShadow {
    /// Where the bed stands on the floor, if delimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_floor: Option<Quad>,
    /// The bed's width and length in metres, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_size_m: Option<[f32; 2]>,
    /// Near the bed in metres: the footprint (the mattress, without one) grown
    /// by the shadow's distance in its own coordinates. Absent without the
    /// bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near: Option<Quad>,
    /// Near the bed as decided today: the bed's bounding box grown by the
    /// admission's margin.
    pub near_today: Rect,
    /// The headboard as a quadrilateral, if it was drawn as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headboard_area: Option<Quad>,
}

/// What is published of the static world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapState {
    /// Whether the delimited bed applies to this image.
    pub geometry: Geometry,
    /// The bed's four corners on the image, clockwise.
    pub bed: Quad,
    /// Where faces are looked for when nobody is in sight.
    pub headboard: Option<Rect>,
    /// Where the main detector looks while the resident is alone in bed: the
    /// delimited focus or the bed grown by the focus policy's pad. Absent, and
    /// not published, when the main detector does not focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Rect>,
    /// The geometry's shadow: what a geometry that does not decide yet sees
    /// of the site. Absent, and not published, unless the profile asks for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<MapShadow>,
    /// What the map learned about the empty bed.
    pub calibration: Calibration,
}

/// The learned bed depth and the sample it came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Calibration {
    /// The empty bed has not been learned.
    #[default]
    Unavailable,
    /// The empty bed was learned.
    Complete {
        /// Its depth, on the depth model's scale (the name says metres, the
        /// value is relative: see `BodyPoint`).
        reference_m: f32,
        /// The sample it was learned in; `None` if the profile brought it.
        since: Option<SampleRef>,
    },
}

// --- moved from scene-core/src/posture.rs
/// The published posture, with its evidence on the depth model's (relative)
/// scale and the pose's provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Posture {
    /// Lying, upright or unknown.
    pub state: PostureState,
    /// Median of the measured head and shoulders.
    pub upper: Option<f32>,
    /// Median of the measured hips: the feet do not vote.
    pub lower: Option<f32>,
    /// `(upper − lower) / mean`: relative, without scale.
    pub disparity: Option<f32>,
    /// That of the pose it was concluded from.
    pub provenance: Provenance,
    /// Head, shoulders and pelvis apart. Absent in scenes recorded before R16.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<BodyGroups>,
    /// Each disparity against the pelvis; `upper_pelvis` is `disparity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disparities: Option<Disparities>,
    /// Above it, lying: the profile's `posture_tolerance`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f32>,
    /// The bed's points, read in the same depth map as the keypoints: covered,
    /// the bed's centre is the blanket over the hips (R17). Absent in scenes
    /// recorded before them, and without the bed's geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bed_points: Vec<BedPoint>,
    /// Where the head was when the bed's points were decided: they are read
    /// only with it at the headboard (1707). Absent in scenes recorded before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_place: Option<HeadPlace>,
}

// --- moved from scene-core/src/verdict.rs
/// How the resident is, as the arbiter rules it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VerdictState {
    /// In bed, the head and shoulders farther from the camera than the pelvis.
    LyingInBed,
    /// In bed, the head and shoulders closer to the camera than the pelvis.
    SittingUpInBed,
    /// In bed, but no head knows how.
    InBed,
}

/// The published verdict: how the resident is, which heads support it and
/// since when it holds (a fold of the chronicle, P4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// How the resident is.
    pub state: VerdictState,
    /// The heads that support it; empty when none knows.
    pub by: Vec<Head>,
    /// The first sample of this verdict, without interruption.
    pub since: SampleRef,
}

// --- moved from scene-core/src/mod.rs
/// The published bed posture: the state, what was seen in this sample and
/// since when the state holds (a fold of the chronicle, P4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPosture {
    /// How the resident in bed is.
    pub state: BedPostureState,
    /// What was seen in this sample.
    pub seen: BedPostureSeen,
    /// The first sample of this state, without interruption.
    pub since: SampleRef,
}

// --- moved from scene-core/src/common.rs
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

// --- moved from scene-core/src/v4.rs
/// The scene of one sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    /// The image size the turn worked on; `None` before the first image.
    pub frame: Option<FrameInfo>,
    /// What was admitted in this sample.
    pub evidence: Evidence,
    /// How many are in the room and the machine's mode.
    pub room: Room,
    /// Who is believed in bed.
    pub bed: Bed,
    /// Who is seen off the bed.
    pub floor: Floor,
    /// The room's only occupant, when there is exactly one.
    pub subject: Option<Subject>,
    /// What the core looked at in this turn, and why not the rest.
    pub attention: Attention,
    /// The bed geometry and what the map learned.
    pub map: MapState,
    /// The reception, as the shell reported it.
    pub sensor: SensorState,
    /// Present only if this scene does not come from a new observation (rule 2).
    pub republished: Option<Republished>,
}
