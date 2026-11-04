//! The geometry's shadow over the site (white book, volume VII, ch. 11): it
//! decides nothing; it is published to choose with data.

use crate::Zone;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// The zone by parts, with the step that decided it and the measures it
    /// leaned on. Absent unless the profile asks for `[occupancy.parts]`
    /// (ADR 2026-11-03, las partes en la sombra). It decides nothing: the
    /// zone that counts is `zone` of the body, as today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<PartsShadow>,
}

/// The zone by parts of one body, for the shadow (ADR 2026-11-03). It decides
/// nothing; the parts' measures are the ones the cascade leaned on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsShadow {
    /// The zone the parts say.
    pub zone: Zone,
    /// The step of the cascade that decided it, or that the pose was not looked at.
    pub step: PartsStep,
    /// The rule that chose the direction that is up for the pose, if any.
    pub turn: TurnRule,
    /// The turn's face, where it is in the headboard: `u` across, and `v` is
    /// the headboard's `w` (0 at the mattress, 1 at its top), not the bed's `v`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<PlanePoint>,
    /// The hips' mean, where they were seen, on the mattress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hips: Option<PlanePoint>,
    /// The ankles' mean, where they were seen, on the footprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ankles: Option<PlanePoint>,
    /// The pelvis's height over the feet, in mattress heights, if the hips were seen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pelvis_mattress_heights: Option<f32>,
    /// The seen ankles' `u` on the footprint, if any: past 1 or below 0 is out of the bed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ankle_u: Option<f32>,
    /// The mean score of the seen ankles, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ankle_score: Option<f32>,
    /// The torso as the frame reads it (F5.2). Absent without `posture`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torso: Option<TorsoShadow>,
    /// The side the legs leave the bed by (F5.2); absent while they are inside.
    /// Absent without `posture`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs_out: Option<LegsSide>,
}

/// The torso's orientation in the frame (F5.2, white book, volume IX, ch. 04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TorsoShadow {
    /// Parallel to «up»: standing, or sitting up.
    Upright,
    /// Parallel to the bed's length: lying along it.
    AlongBed,
    /// Parallel to the bed's width: across it.
    Crosswise,
    /// Between them: no clear orientation.
    Unclear,
    /// «Up» and the bed's length are nearly parallel from this camera: not separated.
    NotSeparated,
}

/// The side the legs leave the bed by (F5.2, E7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LegsSide {
    /// Past the near side.
    Near,
    /// Past the far side.
    Far,
    /// Past the foot of the bed.
    Foot,
}

/// The step of the parts' cascade that decided a body's zone (cap. 05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PartsStep {
    /// Past the near edge, with little of the box over the mattress.
    PastNear,
    /// The box cut at the far edge, with little of it over the mattress.
    Cut,
    /// The box does not touch the bed.
    NoTouch,
    /// The face at the headboard.
    FaceAtHeadboard,
    /// The feet on the mattress, proven by the far strip and the score.
    FeetOnMattress,
    /// The feet on the floor, the pelvis at the mattress height at the edge.
    PelvisAtEdge,
    /// The feet on the floor, the pelvis out of the band: standing.
    Standing,
    /// The pose was looked at and does not prove the zone.
    Unproven,
    /// The pose was not looked at: the zone has no proof.
    PoseNotLooked,
}

/// The rule that chose the direction that is up for the pose of a body (F2.3b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnRule {
    /// A face of the turn inside this body's box and in no other: the arrow
    /// from the box's centre to the face.
    FaceInBox,
    /// The bed's `v` axis towards the headboard, for a body mostly over the mattress.
    BedAxis,
    /// No turn: the standing body stays as it is.
    NoTurn,
}
