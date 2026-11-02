//! The body: the pose of a doubtful body before believing, and the subject's
//! pose after it.

use super::{Consideration, Decision, Glimpse, SkipReason};
use crate::belief::Believed;
use crate::estimation;
use crate::geom::Rect;
use crate::policy::EnrichPolicy;
use crate::provenance::Provenance;
use crate::time::SampleRef;
use crate::time::secs_since;

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

/// A doubtful body's pose: which floor body (its index among the sample's
/// floor bodies) and where to look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyPoseLook {
    pub(crate) floor: usize,
    pub(crate) region: Rect,
}

/// Ask for a doubtful body's pose, before believing (white book, volume VII,
/// ch. 15)? The room says which bodies its box cannot place; one look per
/// turn, for the one with the most of its box over the mattress, over its box
/// grown by the pose's margin. Without a pose model, nothing to decide;
/// without a doubtful body, nothing to ask.
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
    bodies
        .floor
        .iter()
        .enumerate()
        .filter_map(|(i, b)| Some((i, b, room.doubtful(&b.detection.bbox)?)))
        .max_by(|(_, _, a), (_, _, b)| a.share.total_cmp(&b.share))
        .map_or(Consideration::Inapplicable, |(floor, b, _)| {
            Consideration::Decided(Decision::Ask(BodyPoseLook {
                floor,
                region: b.detection.bbox.expand(enrich.crop_pad.get()),
            }))
        })
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
    let expired = policy.max_pose_age_secs.is_some_and(|max| {
        since
            .and_then(|s| secs_since(now, s))
            .is_none_or(|age| age >= max.get())
    });
    if let Some(r) = reference
        && !estimation::moved(&r, &subject.bbox, policy.move_threshold.get())
        && !expired
    {
        return EnrichPlan::Keep(SkipReason::SubjectStill);
    }
    EnrichPlan::Ask {
        bbox: subject.bbox,
        region: subject.bbox.expand(policy.crop_pad.get()),
    }
}
