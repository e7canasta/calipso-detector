//! The body: the pose of a doubtful body before believing, and the subject's
//! pose after it.

use super::{Consideration, Decision, Glimpse, SkipReason};
use crate::belief::Believed;
use crate::estimation;
use crate::geom::{Point, Rect};
use crate::policy::EnrichPolicy;
use crate::provenance::Provenance;
use crate::time::SampleRef;
use crate::time::SourceTime;

/// The pose and its memory are decided together; the reason only explains the decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EnrichPlan {
    Disabled,
    /// No subject: there is no decision to record.
    Inapplicable,
    /// Look for the pose over `region`; the subject's `bbox` stays the
    /// motion reference.
    Ask {
        bbox: Rect,
        region: Rect,
    },
    Keep(SkipReason),
    Skip(SkipReason),
    /// The subject was posed in this turn as a doubtful body, before
    /// believing: that pose is read, fresh, without looking again.
    Posed {
        bbox: Rect,
    },
}

/// Which body a pose is asked for: an index into the sample's bed bodies or
/// its floor bodies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PoseTarget {
    Bed(usize),
    Floor(usize),
}

/// A body's pose to ask for: which body, where to look, and the direction that
/// is up for it (F2.4, the turn of the parts; `None`: no turn).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyPoseLook {
    pub(crate) target: PoseTarget,
    pub(crate) region: Rect,
    pub(crate) up: Option<crate::look::Up>,
}

/// Ask for a body's pose, before believing (white book, volume VII, ch. 15, and
/// volume IX, ch. 05)? One look per turn. First the doubtful floor body with
/// most of its box over the mattress (as before); then, with the parts, the
/// bed and floor bodies the cascade cannot place without their pose, by their
/// share over the mattress. Without a pose model, nothing to decide.
pub(crate) fn pose_bodies(
    g: &Glimpse<'_>,
    room: Option<&crate::room::Room>,
    enrich: Option<&EnrichPolicy>,
) -> Consideration<BodyPoseLook> {
    let Some(enrich) = enrich else {
        return Consideration::Disabled;
    };
    let (Some(room), Some(bodies)) = (room, g.bodies.seen()) else {
        return Consideration::Inapplicable;
    };
    let doubtful = bodies
        .floor
        .iter()
        .enumerate()
        .filter_map(|(i, b)| Some((i, b, room.doubtful(&b.detection.bbox)?)))
        .max_by(|(_, _, a), (_, _, b)| a.share.total_cmp(&b.share));
    if let Some((floor, b, _)) = doubtful {
        return Consideration::Decided(Decision::Ask(BodyPoseLook {
            target: PoseTarget::Floor(floor),
            region: b.detection.bbox.expand(enrich.crop_pad.get()),
            up: None,
        }));
    }
    // The turn's bodies, in the order the turn gives them: beds first, then the floor.
    let boxes: Vec<Rect> = bodies
        .bed
        .iter()
        .map(|b| b.detection.bbox)
        .chain(bodies.floor.iter().map(|b| b.detection.bbox))
        .collect();
    let faces: Vec<Point> = match g.faces {
        crate::evidence::FaceEvidence::Seen(found) => {
            found.iter().map(|f| f.bbox.centroid()).collect()
        }
        _ => Vec::new(),
    };
    let bed = bodies.bed.len();
    let candidates = bodies
        .bed
        .iter()
        .enumerate()
        .map(|(i, b)| (PoseTarget::Bed(i), b.detection.bbox, i))
        .chain(
            bodies
                .floor
                .iter()
                .enumerate()
                .map(|(i, b)| (PoseTarget::Floor(i), b.detection.bbox, bed + i)),
        );
    candidates
        .filter_map(|(target, bbox, index)| {
            let face = crate::room::Room::face_of(&boxes, &faces, index);
            let share = room.pose_needed(bbox, face)?;
            Some((target, bbox, index, share))
        })
        .max_by(|(_, _, _, a), (_, _, _, b)| a.total_cmp(b))
        .map_or(
            Consideration::Inapplicable,
            |(target, bbox, index, share)| {
                let up = room.turn(&boxes, &faces, index, share).and_then(up_of);
                Consideration::Decided(Decision::Ask(BodyPoseLook {
                    target,
                    region: bbox.expand(enrich.crop_pad.get()),
                    up,
                }))
            },
        )
}

/// The turn as the look's direction: from the arrow's start to its end (F2.4).
fn up_of(turn: crate::room::Turn) -> Option<crate::look::Up> {
    use crate::room::Turn;
    match turn {
        Turn::FaceInBox { from, to } | Turn::BedAxis { from, to } => {
            crate::look::Up::new(to.x - from.x, to.y - from.y)
        }
        Turn::Still => None,
    }
}

/// Ask for the pose? Second attention point, after believing: only with a
/// subject (a single occupant) seen in this sample, and only if it moved since
/// the last pose or, with a maximum age, if the last one expired. Without
/// a subject there is nothing to decide. It is looked for over the subject's
/// box grown by the policy's margin.
pub(crate) fn enrich(
    believed: &Believed,
    posed: Option<Rect>,
    reference: Option<Rect>,
    since: Option<SampleRef>,
    now: SampleRef,
    enrich: Option<&EnrichPolicy>,
) -> EnrichPlan {
    let Some(policy) = enrich else {
        return EnrichPlan::Disabled;
    };
    let subject = match &believed.subject {
        None => return EnrichPlan::Inapplicable,
        Some(s) if s.provenance == Provenance::Current => s,
        Some(_) => return EnrichPlan::Skip(SkipReason::NoCurrentSubject),
    };
    // Posed in this turn as a doubtful body: the same body, already looked at.
    if posed == Some(subject.bbox) {
        return EnrichPlan::Posed { bbox: subject.bbox };
    }
    // A pose from another session, or without a sample, has expired.
    let expired = policy
        .max_pose_age_secs
        .is_some_and(|max| since.is_none_or(|s| !now.since(s).within(max)));
    if let Some(r) = reference
        && !estimation::moved(&r, &subject.bbox, policy.move_threshold)
        && !expired
    {
        return EnrichPlan::Keep(SkipReason::SubjectStill);
    }
    EnrichPlan::Ask {
        bbox: subject.bbox,
        region: subject.bbox.expand(policy.crop_pad.get()),
    }
}
