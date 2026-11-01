//! Depth: over the subject's points, and over the empty bed to calibrate it.

use super::{Consideration, Decision, Glimpse, Memory, SkipReason};
use crate::belief::Believed;
use crate::body::HeadPlace;
use crate::body::Keypoint;
use crate::geom::Point;
use crate::geom::Rect;
use crate::look::DepthTarget;
use crate::map::CalibrationMemory;
use crate::policy::Policy;

/// A depth look: through the lens (`region`; `None`, the whole frame), at
/// these points. The subject's carry each keypoint's index; the bed's, just
/// the point.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DepthLook<P> {
    pub(crate) region: Option<Rect>,
    pub(crate) points: Vec<P>,
}

/// Where depth is measured: the whole frame (`None`) or, with the policy's
/// pad, the bed's box grown by it. The subject and the empty bed measure
/// through the same lens; without the bed's geometry there is no crop to use.
fn depth_lens(g: &Glimpse<'_>, policy: &Policy) -> Result<Option<Rect>, SkipReason> {
    match policy.depth_pad {
        None => Ok(None),
        Some(pad) => g
            .map
            .depth_region(g.geometry, pad)
            .map(Some)
            .ok_or(SkipReason::NoBedGeometry),
    }
}

/// Measure the subject's depth? At the pose keypoints with enough score, each
/// with its index in the pose so the reading returns to its keypoint, and at
/// the bed's sites, in the same look: covered, the bed stands in for what the
/// pose does not see, read in the same depth map (R17). Through the depth
/// lens; without a depth model there is nothing to decide.
pub(crate) fn deepen(
    g: &Glimpse<'_>,
    pose: Option<&[Keypoint]>,
    head: HeadPlace,
    policy: &Policy,
) -> Consideration<SubjectDepthLook> {
    let lens = depth_lens(g, policy);
    let Some(policy) = policy.enrich.as_ref().filter(|p| p.depth) else {
        return Consideration::Disabled;
    };
    let mut points: Vec<(DepthTarget, Point)> = pose
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter(|(_, k)| k.is_seen(policy.min_keypoint_score))
        .map(|(i, k)| (DepthTarget::Keypoint(i), Point { x: k.x, y: k.y }))
        .collect();
    if points.is_empty() {
        return Consideration::Decided(Decision::Skip(SkipReason::NoPose));
    }
    // The stand-in is situational: the bed's centre is the hips only with the
    // head at the headboard (1707). Away from it, the bed is not looked at.
    if let Some(sites) = g.map.bed_sites(g.geometry).filter(|_| head.at_headboard()) {
        points.extend(sites.map(|(site, p)| (DepthTarget::Bed(site), p)));
    }
    Consideration::Decided(match lens {
        Err(reason) => Decision::Skip(reason),
        Ok(region) => Decision::Ask(SubjectDepthLook {
            look: DepthLook { region, points },
            head,
        }),
    })
}

/// The subject's depth look, with where the head was concluded to be when it
/// was decided: what was measured depends on it, so it travels with it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SubjectDepthLook {
    pub(crate) look: DepthLook<(DepthTarget, Point)>,
    pub(crate) head: HeadPlace,
}

/// Measure the empty bed to learn it? Only with an incomplete map and a truly
/// empty room: no faces, no bodies, no occupants and nothing pending. The
/// retry is measured in source time (rule 9).
pub(crate) fn calibrate(
    g: &Glimpse<'_>,
    believed: &Believed,
    calibration: &CalibrationMemory,
    memory: &Memory,
    policy: &Policy,
) -> Consideration<DepthLook<Point>> {
    let lens = depth_lens(g, policy);
    let Some(policy) = &policy.calibration else {
        return Consideration::Disabled;
    };
    let (map, at) = (g.map, g.at);
    if calibration.reference().is_some() {
        return Consideration::Inapplicable;
    }
    let skip = |r| Consideration::Decided(Decision::Skip(r));
    if map.bed_for(g.geometry).is_none() {
        return skip(SkipReason::NoBedGeometry);
    }
    if g.faces.seen() {
        return skip(SkipReason::FacesPresent);
    }
    let occupied = believed.occupancy.occupants().total() > 0
        || believed.occupancy.pending.is_some()
        || !g.bodies.seen().is_some_and(|b| b.is_empty());
    if occupied {
        return skip(SkipReason::RoomOccupied);
    }
    if memory
        .calibration_attempt
        .and_then(|last| at.secs_since(last))
        .is_some_and(|age| age < policy.retry_secs)
    {
        return skip(SkipReason::RetryWait);
    }
    Consideration::Decided(match lens {
        Ok(region) => Decision::Ask(DepthLook {
            region,
            points: map.calibration_points(),
        }),
        Err(reason) => Decision::Skip(reason),
    })
}
