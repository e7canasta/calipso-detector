//! The subject's body: the COCO-17 pose and its depth (901). It is the
//! vocabulary shared by the shell (which reads it from the model), the belief
//! (which remembers it), the second level (which interprets it) and the
//! contract (which publishes it).

use crate::provenance::Provenance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A pose keypoint, in whole-frame coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Keypoint {
    pub x: f32,
    pub y: f32,
    pub score: f32,
}

/// A body joint per COCO-17: the keypoint's index in the pose gives its
/// name (901-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
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
/// metres (901-2). An unmeasured point carries `depth: null` (901-3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyPoint {
    pub name: Option<Joint>,
    pub x: f32,
    pub y: f32,
    pub score: f32,
    pub depth: Option<f32>,
}

/// The subject's pose, with its own provenance (rule 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pose {
    pub keypoints: Vec<BodyPoint>,
    pub provenance: Provenance,
}

/// The subject's depth (the median of its measured points), with its own
/// provenance. Kept for compatibility (901-4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubjectDepth {
    pub meters: f32,
    pub provenance: Provenance,
}
