//! Faces at the headboard: whether to look for them, and where.

use super::{Consideration, Decision, Glimpse, SkipReason};
use crate::look::FaceRegion;
use crate::policy::{FacePolicy, Policy};

/// Where to look for faces: in the subject's box if seen; otherwise at the
/// headboard grown by the policy's margin (also when the detector failed).
/// With neither, nothing is searched.
/// The face policy goes with the look: evidence admits the faces with it.
pub(crate) fn faces(g: &Glimpse<'_>, policy: &Policy) -> Consideration<(FaceRegion, FacePolicy)> {
    let Some(policy) = policy.faces else {
        return Consideration::Disabled;
    };
    Consideration::Decided(
        match (
            g.bodies.seen().and_then(|b| b.best()),
            g.map.faces_region(g.geometry, policy.headboard_pad),
        ) {
            (Some(best), _) => Decision::Ask((FaceRegion::Subject(best.bbox), policy)),
            (_, Some(headboard)) => Decision::Ask((FaceRegion::Headboard(headboard), policy)),
            (_, None) => Decision::Skip(SkipReason::NoFaceRegion),
        },
    )
}
