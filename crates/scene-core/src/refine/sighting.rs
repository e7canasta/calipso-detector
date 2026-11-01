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
use crate::policy::EnrichPolicy;
use crate::policy::{Band, VerifyPolicy};
use crate::provenance::Provenance;
use crate::room::Room;
use crate::room::head::HeadPlaceState;

/// Where each part of the body layer 1 reads is, in one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BodyPlaces {
    pub head: HeadPlaceState,
    pub feet: FeetPlace,
    pub disparity: DisparitySide,
}

/// Which side of the posture threshold the disparity falls on: the trama's
/// `d`. Doubtful is the camera's grey band (`?`): neither side is believed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisparitySide {
    /// Not measured.
    Unknown,
    /// Lying: above the tolerance.
    Lying,
    /// Upright: at or below the tolerance.
    Upright,
    /// In the grey band: the verifier may fill it.
    Doubtful,
}

impl DisparitySide {
    fn of(state: PostureState) -> Self {
        match state {
            PostureState::Lying => Self::Lying,
            PostureState::Upright => Self::Upright,
            PostureState::Unknown => Self::Unknown,
        }
    }
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
    /// One seen in the grey band of a side, none clearly past it: the verifier
    /// may fill it.
    Doubtful,
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
                DisparitySide::Lying => Sighting::HeadboardLying,
                DisparitySide::Upright => Sighting::HeadboardUpright,
                DisparitySide::Unknown | DisparitySide::Doubtful => Sighting::Unknown,
            },
            HeadPlaceState::Unknown => Sighting::Unknown,
        }
    }
}

impl BodyPlaces {
    /// Is a digit in the grey band? Only the disparity can be, for now.
    pub(crate) fn doubtful(self) -> bool {
        self.disparity == DisparitySide::Doubtful || self.feet == FeetPlace::Doubtful
    }
}

/// The places of a fresh pose: a pose kept from before places nothing, retained
/// evidence holds, it does not change (SPEC-MULTIHEAD, rule 4).
pub(super) fn places_of(
    body: &SubjectBody,
    posture: Option<&Posture>,
    room: Option<&Room>,
    enrich: &EnrichPolicy,
    verify: Option<&VerifyPolicy>,
) -> Option<BodyPlaces> {
    (body.pose.provenance == Provenance::Current).then(|| {
        places(
            body,
            posture,
            room,
            enrich,
            verify.and_then(|v| v.doubtful_disparity),
            verify.and_then(|v| v.doubtful_feet_margin),
        )
    })
}

/// What layer 1 perceives in this turn: its own places if they are clear (the
/// freshest wins, rule 5); otherwise the places the verifier filled, if an
/// answer arrived; otherwise its own, where a doubtful digit says nothing new.
pub(super) fn of(own: Option<BodyPlaces>, filled: Option<BodyPlaces>) -> Sighting {
    match (own, filled) {
        (Some(places), _) if !places.doubtful() => places.sighting(),
        (_, Some(places)) => places.sighting(),
        (Some(places), None) => places.sighting(),
        (None, None) => Sighting::Unknown,
    }
}

/// Where the head, the feet and the disparity are in this pose. A disparity in
/// the grey band is doubtful.
fn places(
    body: &SubjectBody,
    posture: Option<&Posture>,
    room: Option<&Room>,
    enrich: &EnrichPolicy,
    doubtful: Option<Band>,
    feet_margin: Option<f32>,
) -> BodyPlaces {
    let past_the_sides = |p: Point| room.is_some_and(|room| room.foot_out(p));
    // How far out of the bed, in bed widths: negative inside.
    let beyond_side = |p: Point| room.and_then(|room| room.foot_beyond(p));
    let near_a_side = |p: Point| {
        feet_margin.is_some_and(|m| beyond_side(p).is_some_and(|b| b.abs() <= f64::from(m)))
    };
    let clearly_past = |p: Point| match feet_margin {
        Some(m) => beyond_side(p).is_some_and(|b| b > f64::from(m)),
        None => past_the_sides(p),
    };
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
    let feet = if ankles.iter().any(|p| clearly_past(*p)) {
        FeetPlace::PastTheSides
    } else if ankles.iter().any(|p| near_a_side(*p)) {
        FeetPlace::Doubtful
    } else if ankles.is_empty() {
        FeetPlace::Unseen
    } else {
        FeetPlace::Seen
    };
    BodyPlaces {
        head: body.head_place.state,
        feet,
        disparity: posture.map_or(DisparitySide::Unknown, |p| match (p.disparity, doubtful) {
            (Some(d), Some(band)) if band.contains(d) => DisparitySide::Doubtful,
            _ => DisparitySide::of(p.state),
        }),
    }
}
