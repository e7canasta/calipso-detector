//! Attention: what to look at, where, and why not. Pure policies: they mutate
//! nothing (rule 12). Each question reads its own part of the `Policy`:
//! without it there is nothing to decide, and that is decided here too, not
//! in `cycle` (rule 11).

use crate::belief::{Alone, Belief, Believed, InBed, LatchState, Room};
use crate::body::{BedSite, Keypoint};
use crate::estimation;
use crate::evidence::BodyEvidence;
use crate::evidence::FaceEvidence;
use crate::evidence::RejectReason;
use crate::geom::Point;
use crate::geom::Rect;
use crate::head::HeadPlace;
use crate::look::FaceRegion;
use crate::look::{DepthTarget, Failure, LookKind};
use crate::map::BedPlane;
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
    /// Look for the pose over `region`; the subject's `bbox` stays the
    /// motion reference.
    Ask {
        bbox: Rect,
        region: Rect,
    },
    Keep(SkipReason),
    Skip(SkipReason),
}

/// What the main detector's lens remembers: the last sample it read the whole
/// frame. Attention reads it; `cycle` notes each whole-frame look that read.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Lens {
    last_wide: Option<SampleRef>,
}

impl Lens {
    /// The main detector read the whole frame in this sample.
    pub(crate) fn looked_wide(&mut self, at: SampleRef) {
        self.last_wide = Some(at);
    }

    /// Is the whole frame due? Never looked, another session or the period
    /// passed (rule 9).
    fn wide_due(&self, now: SampleRef, every_secs: f64) -> bool {
        self.last_wide
            .and_then(|last| now.secs_since(last))
            .is_none_or(|age| age >= every_secs)
    }
}

/// Where the main detector runs, which always runs. While the resident is
/// alone in bed, nothing is stabilizing and the sentinel is not due, at the
/// focus, with more pixels per body: the site map's, or else the bed grown by
/// the focus policy's pad. Otherwise over the whole room (`None`), to count
/// everyone, in bed or not. Without a focus policy or bed geometry there is
/// nothing to decide. It is decided before the sample is seen, with what
/// layer 0 believes.
pub(crate) fn primary_region(
    belief: &Belief,
    lens: &Lens,
    at: SampleRef,
    map: &Map,
    geometry: Geometry,
    policy: &Policy,
) -> Option<Rect> {
    let focus = policy.focus.as_ref()?;
    let region = map.focus_region(geometry, focus.pad)?;
    let alone_in_bed = matches!(belief.room(), Room::Alone(Alone::InBed(_)));
    let sentinel = lens.wide_due(at, focus.wide_every_secs);
    (alone_in_bed && belief.settled() && !sentinel).then_some(region)
}

/// The focus saw something other than what is believed: look at the whole
/// frame before believing? The focus only confirms, never refutes: alone in
/// bed and seen, it sees one in bed; held, it sees nobody or the one who
/// reappears; and nobody on the floor. Anything else is asked again over the
/// whole frame, so layer 0 never believes a reading of the focus that did not
/// confirm. A focus that failed proves nothing (rule 1): there is nothing to
/// decide, as with the main detector.
pub(crate) fn widen(g: &Glimpse<'_>, belief: &Belief) -> Consideration<()> {
    let Some(seen) = g.bodies.seen() else {
        return Consideration::Inapplicable;
    };
    let counts = seen.counts();
    let confirms = counts.floor == 0
        && match belief.room() {
            Room::Alone(Alone::InBed(InBed::Seen)) => counts.bed == 1,
            Room::Alone(Alone::InBed(InBed::Held)) => counts.bed <= 1,
            _ => false,
        };
    if confirms {
        Consideration::Inapplicable
    } else {
        Consideration::Decided(Decision::Ask(()))
    }
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
    let now = g.bodies.seen().map_or(0, |b| b.counts().bed);
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
            g.bodies.seen().and_then(|b| b.best()),
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
/// a subject there is nothing to decide. It is looked for over the subject's
/// box grown by the policy's margin.
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
    EnrichPlan::Ask {
        bbox: subject.bbox,
        region: subject.bbox.expand(policy.crop_pad),
    }
}

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
    let plane = g.map.bed_for(g.geometry).and_then(BedPlane::of);
    if let Some(plane) = plane.filter(|_| head.at_headboard()) {
        points.extend(BedSite::ALL.map(|site| {
            let (u, v) = site.at();
            (DepthTarget::Bed(site), plane.point(u, v))
        }));
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
    policy: &Policy,
) -> Consideration<DepthLook<Point>> {
    let lens = depth_lens(g, policy);
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
    let occupied = believed.occupancy.occupants().total() > 0
        || believed.occupancy.pending.is_some()
        || !g.bodies.seen().is_some_and(|b| b.is_empty());
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
    Consideration::Decided(match lens {
        Ok(region) => Decision::Ask(DepthLook {
            region,
            points: map.calibration_points(),
        }),
        Err(reason) => Decision::Skip(reason),
    })
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
