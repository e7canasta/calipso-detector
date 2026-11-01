//! The slow verifier: whether a doubt is worth asking, and the burst of crops
//! that goes with the question (white book, volume III).

use super::{Consideration, Decision, Glimpse, SkipReason};
use crate::belief::bed_posture::BedPostureState;
use crate::body::HeadPlaceState;
use crate::geom::Rect;
use crate::look::{Question, Target};
use crate::policy::Policy;
use crate::policy::VerifyPolicy;
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::SampleRef;
use crate::verification::{self, Collecting, HOUR_SECS, HeldCheckState, Verification};

/// The crop the verifier looks at: the bed grown by this fraction of its width
/// and height on each side (white book, volume III, ch. 05).
const VERIFY_PAD: f32 = 0.15;

/// What to ask the verifier: about what, over which crop, with which question,
/// and, for a doubtful digit, the places the answer fills.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VerifyLook {
    pub(crate) target: Target,
    pub(crate) region: Rect,
    /// The bed grown by the crop's margin: the shell rectifies to it.
    pub(crate) bed: crate::geom::Quad,
    pub(crate) question: Question,
    pub(crate) places: Option<BodyPlaces>,
}

/// Ask the verifier? Only what the policy verifies, one request at a time,
/// within the hour's budget. First a doubtful digit (v = 3): the disparity in
/// the camera's grey band, with the head at the headboard and layer 1
/// following the resident. Then the body held in bed (v = 2), after the cheap
/// senses had their chance: unverified, `after_secs` after it started being
/// held; or checked, once its check is older than `valid_secs` (D5). Without
/// `valid_secs` a held body is asked about once, so a «no» is never asked
/// about in a loop.
pub(crate) fn verify(
    g: &Glimpse<'_>,
    verification: &Verification,
    layer_1: Option<BedPostureState>,
    places: Option<BodyPlaces>,
    policy: &Policy,
) -> Consideration<VerifyLook> {
    let Some(verify) = policy.verify else {
        return Consideration::Disabled;
    };
    // The feet first (v = 4): getting out is the call that matters most.
    let feet = places.filter(|p| p.feet == FeetPlace::Doubtful);
    if let (Some(places), Some(stable)) = (feet, layer_1) {
        return ask(
            g,
            verification,
            &verify,
            Target::Feet { stable },
            Question::LeavingBed,
            Some(places),
        );
    }
    let doubtful = places.filter(|p| {
        p.head == HeadPlaceState::AtHeadboard && p.disparity == DisparitySide::Doubtful
    });
    if let (Some(places), Some(stable)) = (doubtful, layer_1) {
        return ask(
            g,
            verification,
            &verify,
            Target::Disparity { stable },
            Question::SittingInBed,
            Some(places),
        );
    }
    let Some(held_policy) = verify.held else {
        return Consideration::Inapplicable;
    };
    let Some(held) = verification.held() else {
        return Consideration::Inapplicable;
    };
    let due = match held.check.state {
        HeldCheckState::Unverified => {
            g.at.secs_since(held.held_at)
                .is_some_and(|age| age >= held_policy.after_secs)
        }
        HeldCheckState::Confirmed | HeldCheckState::NotConfirmed => {
            held_policy.valid_secs.is_some_and(|valid| {
                held.check
                    .looked
                    .and_then(|looked| g.at.secs_since(looked))
                    .is_none_or(|age| age >= valid)
            })
        }
    };
    if !due {
        return Consideration::Inapplicable;
    }
    ask(
        g,
        verification,
        &verify,
        Target::Held { since: held.since },
        Question::HeldInBed,
        None,
    )
}

/// What a burst being collected does at this sample (white book, volume III,
/// ch. 12, V4), in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BurstStep {
    /// What it asks about no longer holds: dropped unasked.
    Stale,
    /// The doubtful digit cleared by itself: dropped unasked (V17).
    Resolved,
    /// `every_secs` has not passed since the last kept crop.
    Wait,
    /// Keep this sample's crop: the turn sees how sharp it is and whether the
    /// burst is ready ([`burst_ready`]).
    Keep,
}

/// The next step of a burst being collected. It reads, never writes: the turn
/// applies it to the verification plane. Stale: another session, layer 1 left
/// the stable state it was asked under, or the held body is no longer held.
/// Resolved: this sample saw the doubtful digit clearly (a fresh pose).
pub(crate) fn burst(
    at: SampleRef,
    collecting: &Collecting,
    layer_1: Option<BedPostureState>,
    places: Option<BodyPlaces>,
    held: Option<SampleRef>,
) -> BurstStep {
    let Some(age) = at.secs_since(collecting.last) else {
        return BurstStep::Stale;
    };
    if !verification::holds(collecting.ticket.target, layer_1, held) {
        return BurstStep::Stale;
    }
    let resolved = match collecting.ticket.target {
        Target::Feet { .. } => {
            places.is_some_and(|p| matches!(p.feet, FeetPlace::Seen | FeetPlace::PastTheSides))
        }
        Target::Disparity { .. } => places
            .is_some_and(|p| matches!(p.disparity, DisparitySide::Lying | DisparitySide::Upright)),
        Target::Held { .. } => false,
    };
    if resolved {
        return BurstStep::Resolved;
    }
    if age < collecting.burst.every_secs {
        return BurstStep::Wait;
    }
    BurstStep::Keep
}

/// After a crop is kept: which kept crops go, if the burst is ready. Ready when
/// `frames` crops counted (were sharp), and those go; or when the longest wait
/// passed since the first crop, and then the sharpest it has go, in the order
/// they were kept. `None`: keep collecting.
pub(crate) fn burst_ready(
    at: SampleRef,
    collecting: &Collecting,
    policy: Option<&crate::policy::SharpnessPolicy>,
) -> Option<Vec<u8>> {
    let frames = usize::from(collecting.burst.frames.max(1));
    let counted: Vec<u8> = collecting
        .crops
        .iter()
        .enumerate()
        .filter(|(_, c)| c.counted)
        .filter_map(|(i, _)| u8::try_from(i).ok())
        .collect();
    if counted.len() >= frames {
        return Some(counted.into_iter().take(frames).collect());
    }
    let waited = policy.is_some_and(|p| {
        at.secs_since(collecting.opened)
            .is_none_or(|age| age >= p.max_wait_secs)
    });
    if !waited || collecting.crops.is_empty() {
        return None;
    }
    let mut sharpest: Vec<(usize, f32)> = collecting
        .crops
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.sharpness))
        .collect();
    sharpest.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut chosen: Vec<u8> = sharpest
        .into_iter()
        .take(frames)
        .filter_map(|(i, _)| u8::try_from(i).ok())
        .collect();
    chosen.sort_unstable();
    Some(chosen)
}

/// A doubt worth asking: unless a request is in flight, the hour's budget is
/// spent or there is no bed to crop.
fn ask(
    g: &Glimpse<'_>,
    verification: &Verification,
    verify: &VerifyPolicy,
    target: Target,
    question: Question,
    places: Option<BodyPlaces>,
) -> Consideration<VerifyLook> {
    if verification.busy() {
        return Consideration::Decided(Decision::Skip(SkipReason::VerifyBusy));
    }
    let spent = verify.max_per_hour.is_some_and(|max| {
        verification.asked_within(g.at, HOUR_SECS) >= usize::try_from(max).unwrap_or(usize::MAX)
    });
    if spent {
        return Consideration::Decided(Decision::Skip(SkipReason::BudgetSpent));
    }
    let Some(bed) = g
        .map
        .bed_for(g.geometry)
        .and_then(|bed| crate::map::grown_bed(bed, f64::from(VERIFY_PAD)))
    else {
        return Consideration::Decided(Decision::Skip(SkipReason::NoBedGeometry));
    };
    Consideration::Decided(Decision::Ask(VerifyLook {
        target,
        region: bed.bounds(),
        bed,
        question,
        places,
    }))
}
