//! The rules of layer 1: a pure decider, like layer 0's `conservation`. The
//! row is the user's, `l1(t−1) : l0(t) + seen(t) : l1(t)`: what layer 1
//! believed, what layer 0 believes now and what layer 1 perceived. Every cell
//! is in the golden table (`contract/bed-posture-table.txt`): a change of rule
//! is a diff of the table.

use super::{BedPostureState, Sighting};
use crate::belief::InBed;

/// The cell of the table. `None`: layer 1 does not exist.
///
/// - Without the resident alone in bed (layer 0), there is no layer 1.
/// - Layer 1 is born in bed.
/// - Held in bed (layer 0) is lying: upright is big and would be seen. So is
///   the head at the headboard with the disparity of lying.
/// - Entering the bed nothing is reported: in bed, only lying moves it.
/// - Once lying, what is seen now rules: back at the headboard lying, lying;
///   the head away from the headboard, or upright against it, sitting up; the
///   head or a foot past the bed's sides, getting out.
/// - What says nothing holds the state.
pub(super) fn decide(
    before: Option<BedPostureState>,
    l0: Option<InBed>,
    sighting: Sighting,
) -> Option<BedPostureState> {
    use BedPostureState::{GettingOut, InBed as Entering, Lying, SittingUp};
    use Sighting::{HeadAway, HeadSide, HeadboardLying, HeadboardUpright, LegsOut, Unknown};
    let before = before.unwrap_or(Entering);
    Some(match (l0?, before, sighting) {
        (InBed::Held, _, _) | (InBed::Seen, _, HeadboardLying) => Lying,
        (InBed::Seen, _, Unknown) | (InBed::Seen, Entering, _) => before,
        (InBed::Seen, Lying | SittingUp | GettingOut, HeadAway | HeadboardUpright) => SittingUp,
        (InBed::Seen, Lying | SittingUp | GettingOut, HeadSide | LegsOut) => GettingOut,
    })
}

#[cfg(test)]
mod frame;
#[cfg(test)]
mod table;
