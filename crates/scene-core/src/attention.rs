//! Attention: what to look at, where, and why not. Pure policies: they mutate
//! nothing (rule 12). Each question reads its own part of the `Policy`:
//! without it there is nothing to decide, and that is decided here too, not
//! in `cycle` (rule 11).

use crate::belief::{Believed, LatchState};
use crate::body::Keypoint;
use crate::estimation;
use crate::evidence::BodyEvidence;
use crate::evidence::FaceEvidence;
use crate::evidence::RejectReason;
use crate::geom::Point;
use crate::geom::Rect;
use crate::look::FaceRegion;
use crate::look::{Failure, LookKind};
use crate::map::{Geometry, Map};
use crate::policy::{FacePolicy, Policy};
use crate::provenance::Provenance;
use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What attention can see of the current turn. Read-only: deciding what to
/// look at never changes what is seen (rule 12).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Glimpse<'a> {
    pub at: SampleRef,
    pub geometry: Geometry,
    pub map: &'a Map,
    pub bodies: &'a BodyEvidence,
    pub faces: &'a FaceEvidence,
}

/// Ask or do not ask, with a reason. Everything goes to the ledger.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Decision<T> {
    Ask(T),
    Skip(SkipReason),
}

/// Without a policy, or out of context, nothing is decided and no ledger entry is added.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Consideration<T> {
    Disabled,
    Inapplicable,
    Decided(Decision<T>),
}

/// The pose and its memory are decided together; the reason only explains the decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EnrichPlan {
    Disabled,
    /// No subject: there is no decision to record.
    Inapplicable,
    Ask(Rect),
    Keep(SkipReason),
    Skip(SkipReason),
}

/// Where the main detector runs, which always runs: over the whole room
/// (`None`), to count everyone, in bed or not. When the map has a room
/// region, it is decided here.
pub(crate) fn primary_region() -> Option<Rect> {
    None
}

/// Look at the bed again with the finer detector? There is only something to
/// decide if the main detector sees fewer in bed than were seen: a possible
/// loss. While it sees someone in bed and no fewer than before, nobody was
/// lost.
pub(crate) fn reinforce(
    g: &Glimpse<'_>,
    bed_seen: u32,
    latch: LatchState,
    policy: &Policy,
) -> Consideration<Rect> {
    let Some(policy) = &policy.reinforce else {
        return Consideration::Disabled;
    };
    let now = g.bodies.bodies().map_or(0, |b| b.counts().bed);
    if now > 0 && now >= bed_seen {
        return Consideration::Inapplicable;
    }
    let Some(bed) = g.map.bed_for(g.geometry) else {
        return Consideration::Decided(Decision::Skip(SkipReason::NoBedGeometry));
    };
    Consideration::Decided(match latch {
        LatchState::Disarmed => Decision::Skip(SkipReason::NoSubjectInBed),
        LatchState::Spent => Decision::Skip(SkipReason::LatchSpent),
        LatchState::Armed => Decision::Ask(bed.bounds().expand(policy.bed_padding)),
    })
}

/// Where to look for faces: in the subject's box if seen; otherwise at the
/// headboard (also when the detector failed). With neither, nothing is searched.
/// The face policy goes with the look: evidence admits the faces with it.
pub(crate) fn faces(g: &Glimpse<'_>, policy: &Policy) -> Consideration<(FaceRegion, FacePolicy)> {
    let Some(policy) = policy.faces else {
        return Consideration::Disabled;
    };
    Consideration::Decided(
        match (
            g.bodies.bodies().and_then(|b| b.best()),
            g.map.headboard_for(g.geometry),
        ) {
            (Some(best), _) => Decision::Ask((FaceRegion::Subject(best.bbox), policy)),
            (_, Some(headboard)) => Decision::Ask((FaceRegion::Headboard(headboard), policy)),
            (_, None) => Decision::Skip(SkipReason::NoFaceRegion),
        },
    )
}

/// Ask for the pose? Second attention point, after believing: only with a
/// subject (a single occupant) seen in this sample, and only if it moved since
/// the last pose or, with a maximum age, if the last one expired. Without
/// a subject there is nothing to decide.
pub(crate) fn enrich(
    believed: &Believed,
    reference: Option<Rect>,
    since: Option<SampleRef>,
    now: SampleRef,
    policy: &Policy,
) -> EnrichPlan {
    let Some(policy) = &policy.enrich else {
        return EnrichPlan::Disabled;
    };
    let subject = match &believed.subject {
        None => return EnrichPlan::Inapplicable,
        Some(s) if s.provenance == Provenance::Current => s,
        Some(_) => return EnrichPlan::Skip(SkipReason::NoCurrentSubject),
    };
    // A pose from another session, or without a sample, has expired.
    let expired = policy.max_pose_age_secs.is_some_and(|max| {
        since
            .and_then(|s| now.secs_since(s))
            .is_none_or(|age| age >= max)
    });
    if let Some(r) = reference
        && !estimation::moved(&r, &subject.bbox, policy.move_threshold)
        && !expired
    {
        return EnrichPlan::Keep(SkipReason::SubjectStill);
    }
    EnrichPlan::Ask(subject.bbox)
}

/// Measure the subject's depth? At the pose keypoints with enough score, each
/// with its index in the pose so the reading returns to its keypoint;
/// without a depth model there is nothing to decide.
pub(crate) fn deepen(
    pose: Option<&[Keypoint]>,
    policy: &Policy,
) -> Consideration<Vec<(usize, Point)>> {
    let Some(policy) = policy.enrich.as_ref().filter(|p| p.depth) else {
        return Consideration::Disabled;
    };
    let points: Vec<(usize, Point)> = pose
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter(|(_, k)| k.score >= policy.min_keypoint_score)
        .map(|(i, k)| (i, Point { x: k.x, y: k.y }))
        .collect();
    Consideration::Decided(if points.is_empty() {
        Decision::Skip(SkipReason::NoPose)
    } else {
        Decision::Ask(points)
    })
}

/// Measure the empty bed to learn it? Only with an incomplete map and a truly
/// empty room: no faces, no bodies, no occupants and nothing pending. The
/// retry is measured in source time (rule 9).
pub(crate) fn calibrate(
    g: &Glimpse<'_>,
    believed: &Believed,
    policy: &Policy,
) -> Consideration<Vec<Point>> {
    let Some(policy) = &policy.calibration else {
        return Consideration::Disabled;
    };
    let (map, at) = (g.map, g.at);
    if map.reference().is_some() {
        return Consideration::Inapplicable;
    }
    let skip = |r| Consideration::Decided(Decision::Skip(r));
    if map.bed_for(g.geometry).is_none() {
        return skip(SkipReason::NoBedGeometry);
    }
    if g.faces.seen() {
        return skip(SkipReason::FacesPresent);
    }
    let occupied = believed.occupancy.occupants() > 0
        || believed.occupancy.pending.is_some()
        || !g.bodies.bodies().is_some_and(|b| b.is_empty());
    if occupied {
        return skip(SkipReason::RoomOccupied);
    }
    if map
        .last_attempt()
        .and_then(|last| at.secs_since(last))
        .is_some_and(|age| age < policy.retry_secs)
    {
        return skip(SkipReason::RetryWait);
    }
    Consideration::Decided(Decision::Ask(map.bed_points()))
}

/// Why nothing was looked at (rule 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// This sense was not requested.
    NotRequested,
    /// No body and no delimited headboard: nowhere to look for faces.
    NoFaceRegion,
    /// This loss was already reinforced: once per loss.
    LatchSpent,
    /// Nobody was seen in bed: there is no loss to reinforce.
    NoSubjectInBed,
    /// The geometry does not apply: where the bed is, is unknown.
    NoBedGeometry,
    /// The subject did not move since the last pose: the previous one holds.
    SubjectStill,
    /// The subject is retained, not seen in this sample.
    NoCurrentSubject,
    /// Without a pose there are no points to measure depth at.
    NoPose,
    /// Someone is in the room: a person is not learned as the bed.
    RoomOccupied,
    /// Faces are in sight: someone may be covering the bed.
    FacesPresent,
    /// The last attempt failed and the retry delay has not passed yet.
    RetryWait,
}

/// What a look returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookResult {
    /// The sense read: how many candidates, and why those that did not count
    /// were discarded.
    Read {
        /// How many the sense returned, before admission.
        candidates: u32,
        /// The discarded ones, grouped by reason.
        #[serde(with = "rejections")]
        #[schemars(with = "Vec<Rejected>")]
        rejected: Vec<RejectReason>,
    },
    /// The sense failed: it proves nothing (rule 1).
    Failed {
        /// How it failed.
        failure: Failure,
    },
}

/// An entry of attention's ledger (rule 12): what was decided to look at or
/// not, and what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Look {
    /// Which sense.
    pub kind: LookKind,
    /// Asked, with what came back, or skipped, with why.
    pub outcome: LookOutcome,
}

/// What came of a look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookOutcome {
    /// A look was requested.
    Asked {
        /// `None`: the whole frame.
        region: Option<Rect>,
        /// What the sense returned.
        result: LookResult,
        /// What the shell measured; the core only records it (rule 9).
        spent_ms: f64,
    },
    /// It was decided not to look, with a reason.
    Skipped {
        /// Why not.
        reason: SkipReason,
    },
}

// The published form groups the rejections; the internal ledger keeps each reason.
#[derive(Serialize, Deserialize, JsonSchema)]
struct Rejected {
    /// Why they did not count.
    reason: RejectReason,
    /// How many, for that reason.
    count: u32,
}

mod rejections {
    use super::{RejectReason, Rejected};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub(super) fn serialize<S: Serializer>(
        reasons: &[RejectReason],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut groups: Vec<Rejected> = Vec::new();
        for reason in reasons {
            if let Some(group) = groups.iter_mut().find(|g| g.reason == *reason) {
                group.count = group.count.saturating_add(1);
            } else {
                groups.push(Rejected {
                    reason: *reason,
                    count: 1,
                });
            }
        }
        groups.serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<RejectReason>, D::Error> {
        let groups = Vec::<Rejected>::deserialize(deserializer)?;
        Ok(groups
            .into_iter()
            .flat_map(|g| {
                std::iter::repeat_n(g.reason, usize::try_from(g.count).unwrap_or(usize::MAX))
            })
            .collect())
    }
}

impl LookResult {
    /// The sense read `n` things and none were discarded.
    pub fn read(n: usize) -> Self {
        LookResult::Read {
            candidates: u32::try_from(n).unwrap_or(u32::MAX),
            rejected: Vec::new(),
        }
    }

    /// A look whose sense failed.
    pub fn failed(failure: Failure) -> Self {
        LookResult::Failed { failure }
    }
}

impl Look {
    /// The result, if a look was requested.
    pub fn result(&self) -> Option<&LookResult> {
        match &self.outcome {
            LookOutcome::Asked { result, .. } => Some(result),
            LookOutcome::Skipped { .. } => None,
        }
    }
}

/// What was looked at during the turn, in order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Attention {
    /// One per sense considered, in the order of the turn.
    pub looks: Vec<Look>,
}
