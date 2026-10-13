//! What layer 1 perceives by itself in a sample: the second level reduces
//! the turn to one sighting, half of the row's input; layer 0's state is the
//! other half, and `Belief` wires it. It reads, never writes the belief.
//!
//! Two steps, as the trama writes them: first where each part of the body is
//! ([`BodyPlaces`], the `hfd` of `rbh.cyz:fv.hfd:rbh.cyz`), then what layer 1
//! reads in those places ([`BodyPlaces::sighting`]). The places are facts; the
//! reading is a decision list over them.

use super::{Posture, PostureState};
use crate::belief::SubjectBody;
use crate::belief::bed_posture::Sighting;
use crate::body::Joint;
use crate::geom::Point;
use crate::head::HeadPlaceState;
use crate::map::{BedArea, BedPlane};
use crate::policy::EnrichPolicy;
use crate::provenance::Provenance;

/// Where each part of the body layer 1 reads is, in one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BodyPlaces {
    pub head: HeadPlaceState,
    pub feet: FeetPlace,
    pub disparity: PostureState,
}

/// Where the feet are, as a set: one past the bed's sides is past the sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeetPlace {
    /// No ankle seen.
    Unseen,
    /// Seen, none past the bed's sides.
    Seen,
    /// At least one seen past the bed's sides.
    PastTheSides,
}

impl BodyPlaces {
    /// What layer 1 reads in these places, in this order: a foot past the
    /// bed's sides; the head past them; the head away from the headboard
    /// (below it in bed, or anywhere but past the sides), which is not lying
    /// by definition; and, only with the head at the headboard, the
    /// disparity: lying or upright.
    pub(crate) fn sighting(self) -> Sighting {
        if self.feet == FeetPlace::PastTheSides {
            return Sighting::LegsOut;
        }
        match self.head {
            HeadPlaceState::AtSide => Sighting::HeadSide,
            HeadPlaceState::BelowHeadboard | HeadPlaceState::Elsewhere => Sighting::HeadAway,
            HeadPlaceState::AtHeadboard => match self.disparity {
                PostureState::Lying => Sighting::HeadboardLying,
                PostureState::Upright => Sighting::HeadboardUpright,
                PostureState::Unknown => Sighting::Unknown,
            },
            HeadPlaceState::Unknown => Sighting::Unknown,
        }
    }
}

/// From a fresh pose only: a pose kept from before says nothing new, retained
/// evidence holds, it does not change (SPEC-MULTIHEAD, rule 4).
pub(super) fn of(
    body: &SubjectBody,
    posture: Option<&Posture>,
    plane: Option<BedPlane>,
    enrich: &EnrichPolicy,
) -> Sighting {
    if body.pose.provenance != Provenance::Current {
        return Sighting::Unknown;
    }
    places(body, posture, plane, enrich).sighting()
}

/// Where the head, the feet and the disparity are in this pose.
fn places(
    body: &SubjectBody,
    posture: Option<&Posture>,
    plane: Option<BedPlane>,
    enrich: &EnrichPolicy,
) -> BodyPlaces {
    let past_the_sides = |p: Point| plane.is_some_and(|bed| bed.locate(p) == BedArea::PastTheSides);
    let ankles: Vec<Point> = body
        .pose
        .keypoints
        .iter()
        .filter(|k| {
            matches!(k.name, Some(Joint::LeftAnkle | Joint::RightAnkle))
                && k.is_seen(enrich.min_keypoint_score)
        })
        .map(|k| Point { x: k.x, y: k.y })
        .collect();
    let feet = if ankles.iter().any(|p| past_the_sides(*p)) {
        FeetPlace::PastTheSides
    } else if ankles.is_empty() {
        FeetPlace::Unseen
    } else {
        FeetPlace::Seen
    };
    BodyPlaces {
        head: body.head_place.state,
        feet,
        disparity: posture.map_or(PostureState::Unknown, |p| p.state),
    }
}
