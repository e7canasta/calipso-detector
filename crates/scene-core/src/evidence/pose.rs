//! The pose: the subject's, and a doubtful body's, handed to the room.

use super::{Body, BodyEvidence, BodySource, Perceived, PosedBody, Sensed};
use crate::attention::LookResult;
use crate::body::Keypoint;
use crate::policy::Policy;
use crate::room::{BodyZone, Room};

/// The pose the sense brought: `None` if it failed or brought no keypoints.
pub(crate) fn pose(reading: Sensed<Option<Vec<Keypoint>>>) -> (Option<Vec<Keypoint>>, LookResult) {
    match reading {
        Err(failure) => (None, LookResult::failed(failure)),
        Ok(Some(kp)) if !kp.is_empty() => {
            let result = LookResult::read(kp.len());
            (Some(kp), result)
        }
        Ok(_) => (None, LookResult::read(0)),
    }
}

/// A doubtful body's pose (white book, volume VII, ch. 15): the room places
/// the floor body `floor` with it, and if it says bed the body moves there,
/// counted as the instrument that saw it counts. A failing or empty pose
/// leaves it where it was: not seeing the knee proves nothing (rule 1).
pub(crate) fn pose_body(
    perceived: &mut Perceived,
    floor: usize,
    reading: Sensed<Option<Vec<Keypoint>>>,
    room: Option<&Room>,
    policy: &Policy,
) -> LookResult {
    let (pose, result) = pose(reading);
    let admission = &mut perceived.bodies;
    let (Some(pose), Some(room), Some(enrich), BodyEvidence::Seen(bodies)) =
        (pose, room, policy.enrich, &mut admission.bodies)
    else {
        return result;
    };
    let Some(body) = bodies.floor.get_mut(floor) else {
        return result;
    };
    let posed = room.posed(&body.detection.bbox, &pose, enrich.min_keypoint_score.get());
    perceived.posed = Some(PosedBody {
        bbox: body.detection.bbox,
        pose,
    });
    body.knee_deg = posed.knee_deg;
    if posed.zone != BodyZone::Bed || floor >= bodies.floor.len() {
        return result;
    }
    let moved = bodies.floor.remove(floor);
    if moved.source == BodySource::Primary {
        admission.primary.floor = admission.primary.floor.saturating_sub(1);
        admission.primary.bed += 1;
    }
    bodies.bed.push(Body {
        detection: moved.detection,
        source: moved.source,
        shadow: moved.shadow,
        knee_deg: moved.knee_deg,
    });
    bodies
        .bed
        .sort_by(|a, b| b.detection.confidence.total_cmp(&a.detection.confidence));
    result
}
