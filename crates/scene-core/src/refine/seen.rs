//! What layer 1 saw in a sample: the second level reduces the turn to one
//! symbol, the column of the bed-posture table. It reads, never writes the
//! belief.

use super::{BedPlane, Posture, PostureState};
use crate::belief::{Alone, BedPostureSeen, Believed, InBed, Room, SubjectBody};
use crate::body::Joint;
use crate::geom::{Point, Quad};
use crate::head::HeadPlaceState;
use crate::policy::EnrichPolicy;
use crate::provenance::Provenance;

/// Nothing while layer 0 does not believe the resident alone in bed. Held in
/// bed is held. Seen, from a fresh pose and in this order: a foot past the
/// bed's sides; the head past them; the head away from the headboard (below
/// it in bed, or anywhere but past the sides), which is not lying by
/// definition; and, only with the head at the headboard, the disparity: lying
/// or upright. A pose kept from before says nothing new: retained evidence
/// holds, it does not change (SPEC-MULTIHEAD, rule 4).
pub(crate) fn seen(
    believed: &Believed,
    posture: Option<&Posture>,
    bed: Option<&Quad>,
    enrich: Option<&EnrichPolicy>,
) -> Option<BedPostureSeen> {
    match Room::of(believed.occupancy.occupants()) {
        Room::Alone(Alone::InBed(InBed::Held)) => Some(BedPostureSeen::Held),
        Room::Alone(Alone::InBed(InBed::Seen)) => {
            let fresh = believed
                .body
                .as_ref()
                .filter(|b| b.pose.provenance == Provenance::Current);
            Some(fresh.map_or(BedPostureSeen::Unknown, |body| {
                by_body(body, posture, bed.and_then(BedPlane::of), enrich)
            }))
        }
        Room::Empty | Room::Alone(Alone::Out) | Room::Accompanied(_) => None,
    }
}

fn by_body(
    body: &SubjectBody,
    posture: Option<&Posture>,
    plane: Option<BedPlane>,
    enrich: Option<&EnrichPolicy>,
) -> BedPostureSeen {
    let min_score = enrich.map_or(f32::INFINITY, |p| p.min_keypoint_score);
    let past_the_sides =
        |p: Point| plane.is_some_and(|bed| !(0.0..=1.0).contains(&bed.project(p).0));
    let legs_out = body.pose.keypoints.iter().any(|k| {
        matches!(k.name, Some(Joint::LeftAnkle | Joint::RightAnkle))
            && k.score >= min_score
            && past_the_sides(Point { x: k.x, y: k.y })
    });
    if legs_out {
        return BedPostureSeen::LegsOut;
    }
    match body.head_place.state {
        HeadPlaceState::AtSide => BedPostureSeen::HeadSide,
        HeadPlaceState::BelowHeadboard | HeadPlaceState::Elsewhere => BedPostureSeen::HeadAway,
        HeadPlaceState::AtHeadboard => match posture.map(|p| p.state) {
            Some(PostureState::Lying) => BedPostureSeen::HeadboardLying,
            Some(PostureState::Upright) => BedPostureSeen::HeadboardUpright,
            Some(PostureState::Unknown) | None => BedPostureSeen::Unknown,
        },
        HeadPlaceState::Unknown => BedPostureSeen::Unknown,
    }
}
