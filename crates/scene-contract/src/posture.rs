//! How the body is, by depth: the published posture and the second level's
//! lying, with the pose's provenance.

use crate::{BedPoint, BodyGroups, Disparities, HeadPlace, Provenance};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
