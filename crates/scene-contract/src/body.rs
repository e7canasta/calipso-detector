//! The body and its claims: the pose's keypoints, where the head is and where
//! each claim comes from (rule 2).

use crate::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
