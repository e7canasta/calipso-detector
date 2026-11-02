//! The second level of the loop: how the resident is, on top of what
//! the first level already believes about where they are. It reads the
//! concluded pose and returns its own result; it never writes the belief or
//! changes a count (second-level rule 1). `project` builds the published scene.

pub(crate) mod head;
mod lying;
pub(crate) mod parts;
mod posture;
mod sighting;
mod verdict;

use crate::belief::SubjectBody;
use crate::belief::bed_posture::Sighting;
use crate::policy::{EnrichPolicy, VerifyPolicy};
use crate::room::Room;
use crate::zone::Zone;
pub(crate) use lying::{Lying, LyingState};
pub(crate) use posture::{BodyGroup, BodyGroups, Disparities, Posture, PostureState};
pub(crate) use sighting::{BodyPlaces, DisparitySide, FeetPlace};
pub(crate) use verdict::Ruling;
pub(crate) use verdict::{Head, Verdict, VerdictState};

/// What the second level derives from the subject's pose.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Refinement {
    pub lying: Option<Lying>,
    pub posture: Option<Posture>,
    /// The arbiter's ruling, for the chronicle to date and `project` to publish.
    pub verdict: Option<Ruling>,
    /// What layer 1 perceives by itself in this turn.
    pub sighting: Sighting,
    /// Where the body's parts are, from a fresh pose: the verifier reads its
    /// doubtful digits.
    pub places: Option<BodyPlaces>,
}

/// What the second level derives from the subject's pose: whether they are
/// truly lying down (with the bed geometry), their posture by depth (with
/// measured keypoints) and what layer 1 perceives. What cannot be derived is
/// not published; without a body, layer 1 perceives nothing new.
pub(crate) fn refine(
    zone: Option<Zone>,
    body: Option<&SubjectBody>,
    room: Option<&Room>,
    enrich: Option<&EnrichPolicy>,
    verify: Option<&VerifyPolicy>,
    filled: Option<BodyPlaces>,
) -> Refinement {
    let (Some(body), Some(enrich)) = (body, enrich) else {
        return Refinement {
            sighting: sighting::of(None, filled),
            ..Refinement::default()
        };
    };
    let pose = &body.pose;
    let provenance = pose.provenance;
    let lying = room.filter(|room| room.has_plane()).map(|room| Lying {
        state: lying::classify(&pose.keypoints, room, enrich.min_keypoint_score.get()),
        provenance,
    });
    let posture = posture::classify(body, enrich).map(|r| Posture {
        state: r.state,
        upper: r.upper,
        lower: r.lower,
        disparity: r.disparity,
        provenance,
        groups: Some(r.groups),
        disparities: Some(r.disparities),
        tolerance: Some(enrich.posture_tolerance.get()),
        bed_points: body.bed_points.clone(),
        head_place: Some(body.head_place),
    });
    let verdict = zone.and_then(|zone| verdict::arbitrate(zone, posture.as_ref().map(|p| p.state)));
    let places = sighting::places_of(body, posture.as_ref(), room, enrich, verify);
    Refinement {
        lying,
        posture,
        verdict,
        sighting: sighting::of(places, filled),
        places,
    }
}
