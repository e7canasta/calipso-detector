//! The subject's body: the COCO-17 pose and its depth. The types are the wire's
//! (`scene-contract`); the decisions about them live here: whether a keypoint is
//! seen under the profile's minimum, where a joint is, where a bed site falls,
//! and whether the head is at the headboard.

pub(crate) use scene_contract::{
    BedPoint, BedSite, BodyPoint, HeadEvidence, HeadPlace, HeadPlaceState, Joint, Keypoint, Pose,
    SubjectDepth,
};

/// Seen: its score reaches the profile's minimum. Below it a keypoint is not
/// seen: no depth, no reasoning, not drawn.
pub(crate) fn is_seen(keypoint: &Keypoint, min_score: f32) -> bool {
    keypoint.score >= min_score
}

/// Seen: its score reaches the profile's minimum, as a [`Keypoint`]'s.
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
