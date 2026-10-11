//! The second level of the loop: how the resident is, on top of what
//! the first level already believes about where they are. It reads the
//! concluded pose and returns its own result; it never writes the belief or
//! changes a count (second-level rule 1). `project` builds the published scene.

mod lying;
mod plane;
mod posture;

use crate::belief::SubjectBody;
use crate::geom::Quad;
use crate::policy::EnrichPolicy;
pub use lying::{Lying, LyingState};
pub use posture::{Posture, PostureState};

pub(crate) use plane::BedPlane;

/// What the second level derives from the subject's pose.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Refinement {
    pub lying: Option<Lying>,
    pub posture: Option<Posture>,
}

/// What the second level derives from the subject's pose: whether they are
/// truly lying down (with the bed geometry) and their posture by depth
/// (with measured keypoints). What cannot be derived is not published.
pub(crate) fn refine(
    body: Option<&SubjectBody>,
    bed: Option<&Quad>,
    enrich: Option<&EnrichPolicy>,
) -> Refinement {
    let (Some(body), Some(enrich)) = (body, enrich) else {
        return Refinement::default();
    };
    let pose = &body.pose;
    let provenance = pose.provenance;
    let lying = bed.and_then(BedPlane::of).map(|plane| Lying {
        state: lying::classify(&pose.keypoints, &plane, enrich.min_keypoint_score),
        provenance,
    });
    let posture = posture::classify(
        &pose.keypoints,
        enrich.min_keypoint_score,
        enrich.posture_tolerance,
    )
    .map(|r| Posture {
        state: r.state,
        upper: r.upper,
        lower: r.lower,
        disparity: r.disparity,
        provenance,
    });
    Refinement { lying, posture }
}
