//! What was admitted in this sample, apart from the stabilized belief: the
//! bodies and the faces, and why a candidate did not count.

use crate::{BodyShadow, SkipReason, Zone};
use scene_geom::Rect;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
