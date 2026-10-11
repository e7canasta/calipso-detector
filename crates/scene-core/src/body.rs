//! The subject's body: the COCO-17 pose and its depth. It is the
//! vocabulary shared by the shell (which reads it from the model), the belief
//! (which remembers it), the second level (which interprets it) and the
//! contract (which publishes it).

use crate::provenance::Provenance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
// The COCO-17 names document themselves.
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

    /// The joint at position `index` of a pose; beyond 17, none.
    pub fn coco(index: usize) -> Option<Joint> {
        Joint::COCO.get(index).copied()
    }
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
