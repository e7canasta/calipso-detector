//! The slow verifier: whether a doubt is worth asking, and the burst of crops
//! that goes with the question (white book, volume III).

use super::{Consideration, Decision, Glimpse, SkipReason};
use crate::belief::bed_posture::BedPostureState;
use crate::body::HeadPlaceState;
use crate::geom::Rect;
use crate::look::{Question, Target};
use crate::policy::VerifyPolicy;
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::secs_since;
use crate::units::Extension;
use crate::verification::{HOUR_SECS, HeldCheckState, Verification};

/// The crop the verifier looks at: the bed grown by this fraction of its width
/// and height on each side (white book, volume III, ch. 05).
const VERIFY_PAD: Extension = Extension::VERIFY_CROP;

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
    verify: Option<&VerifyPolicy>,
) -> Consideration<VerifyLook> {
    let Some(verify) = verify.copied() else {
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
            secs_since(g.at, held.held_at).is_some_and(|age| age >= held_policy.after_secs.get())
        }
        HeldCheckState::Confirmed | HeldCheckState::NotConfirmed => {
            held_policy.valid_secs.is_some_and(|valid| {
                held.check
                    .looked
                    .and_then(|looked| secs_since(g.at, looked))
                    .is_none_or(|age| age >= valid.get())
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
    // The question being collected is not a new doubt: the ledger has its burst.
    if verification.collecting_for(target) {
        return Consideration::Inapplicable;
    }
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
        .and_then(|bed| crate::map::grown_bed(bed, VERIFY_PAD))
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
