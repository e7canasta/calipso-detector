//! What layer 1 perceives by itself in a sample: the second level reduces
//! the turn to one sighting, half of the row's input; layer 0's state is the
//! other half, and `Belief` wires it. It reads, never writes the belief.

use super::{BedPlane, Posture, PostureState};
use crate::belief::bed_posture::Sighting;
use crate::belief::{Believed, SubjectBody};
use crate::body::Joint;
use crate::geom::{Point, Quad};
use crate::head::HeadPlaceState;
use crate::policy::EnrichPolicy;
use crate::provenance::Provenance;

/// From a fresh pose and in this order: a foot past the bed's sides; the head
/// past them; the head away from the headboard (below it in bed, or anywhere
/// but past the sides), which is not lying by definition; and, only with the
/// head at the headboard, the disparity: lying or upright. Without a body, or
/// with a pose kept from before, nothing new: retained evidence holds, it does
/// not change (SPEC-MULTIHEAD, rule 4).
pub(crate) fn seen(
    believed: &Believed,
    posture: Option<&Posture>,
    bed: Option<&Quad>,
    enrich: Option<&EnrichPolicy>,
) -> Sighting {
    believed
        .body
        .as_ref()
        .filter(|b| b.pose.provenance == Provenance::Current)
        .map_or(Sighting::Unknown, |body| {
            by_body(body, posture, bed.and_then(BedPlane::of), enrich)
        })
}

fn by_body(
    body: &SubjectBody,
    posture: Option<&Posture>,
    plane: Option<BedPlane>,
    enrich: Option<&EnrichPolicy>,
) -> Sighting {
    let min_score = enrich.map_or(f32::INFINITY, |p| p.min_keypoint_score);
    let past_the_sides =
        |p: Point| plane.is_some_and(|bed| !(0.0..=1.0).contains(&bed.project(p).0));
    let legs_out = body.pose.keypoints.iter().any(|k| {
        matches!(k.name, Some(Joint::LeftAnkle | Joint::RightAnkle))
            && k.score >= min_score
            && past_the_sides(Point { x: k.x, y: k.y })
    });
    if legs_out {
        return Sighting::LegsOut;
    }
    match body.head_place.state {
        HeadPlaceState::AtSide => Sighting::HeadSide,
        HeadPlaceState::BelowHeadboard | HeadPlaceState::Elsewhere => Sighting::HeadAway,
        HeadPlaceState::AtHeadboard => match posture.map(|p| p.state) {
            Some(PostureState::Lying) => Sighting::HeadboardLying,
            Some(PostureState::Upright) => Sighting::HeadboardUpright,
            Some(PostureState::Unknown) | None => Sighting::Unknown,
        },
        HeadPlaceState::Unknown => Sighting::Unknown,
    }
}
