//! The subject's body: the COCO-17 pose and its depth. The decisions about them
//! live here: whether a keypoint is seen under the profile's minimum, where a
//! joint is, where a bed site falls, and whether the head is at the headboard.
//! The joints, the keypoints and the bed sites are the sense's and the
//! protocol's vocabulary (`scene-contract`); the body the core builds from them
//! is its own, and `project` publishes it.

use crate::provenance::Provenance;
pub(crate) use scene_contract::{BedSite, Joint, Keypoint};

/// A pose keypoint with its place in the pose's joints and its depth, as the
/// core reads them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyPoint {
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pose {
    /// In COCO-17 order.
    pub keypoints: Vec<BodyPoint>,
    /// Taken in this sample, or stable since a sample in which the subject
    /// did not move.
    pub provenance: Provenance,
}

/// The subject's depth (the median of its measured points), with its own
/// provenance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SubjectDepth {
    /// On the depth model's scale (relative, not metres).
    pub meters: f32,
    /// Measured in this sample, or stable with the pose.
    pub provenance: Provenance,
}

/// A bed point as the core read it: which site, where it falls in the image and
/// its depth, read in the same depth map as the body's keypoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedPoint {
    /// The site.
    pub name: BedSite,
    /// Pixels from the left edge.
    pub x: f32,
    /// Pixels from the top edge.
    pub y: f32,
    /// Its depth on the depth model's scale, if it was read.
    pub depth: Option<f32>,
}

/// What the head's place was concluded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadEvidence {
    /// An accepted face.
    Face,
    /// The pose's head keypoints (nose, eyes, ears) seen.
    PoseHead,
}

/// Where the head is, and what said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeadPlace {
    /// At the headboard, elsewhere, or unknown.
    pub state: HeadPlaceState,
    /// What it was concluded from; `None` when unknown.
    pub by: Option<HeadEvidence>,
}

/// Where the head is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadPlaceState {
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
    pub(crate) const UNKNOWN: HeadPlace = HeadPlace {
        state: HeadPlaceState::Unknown,
        by: None,
    };

    /// The head's place, concluded from what it was seen in.
    pub(crate) fn of(state: HeadPlaceState, by: HeadEvidence) -> Self {
        Self {
            state,
            by: Some(by),
        }
    }
}

/// Seen: its score reaches the profile's minimum. Below it a keypoint is not
/// seen: no depth, no reasoning, not drawn.
pub(crate) fn is_seen(keypoint: &Keypoint, min_score: f32) -> bool {
    keypoint.score >= min_score
}

/// Seen: its score reaches the profile's minimum, as a [`BodyPoint`]'s.
pub(crate) fn point_is_seen(point: &BodyPoint, min_score: f32) -> bool {
    point.score >= min_score
}

/// The joint at position `index` of a pose; beyond 17, none.
pub(crate) fn coco_joint(index: usize) -> Option<Joint> {
    Joint::COCO.get(index).copied()
}

/// Where a bed site is in bed coordinates: `u` across, `v` from the foot (0) to
/// the headboard (1).
pub(crate) fn bed_site_at(site: BedSite) -> (f64, f64) {
    match site {
        BedSite::Centre => (0.5, 0.5),
    }
}

/// Is the head at the headboard?
pub(crate) fn at_headboard(head: &HeadPlace) -> bool {
    head.state == HeadPlaceState::AtHeadboard
}
