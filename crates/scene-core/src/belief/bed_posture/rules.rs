//! The rules of layer 1: from what we believed and what we saw to what we
//! believe. A pure decider, like layer 0's `conservation`: no state, no
//! events. Every cell is in the golden table (`contract/bed-posture-table.txt`):
//! a change of rule is a diff of the table.

use super::{BedPostureSeen, BedPostureState};

/// The cell of the table: `before : seen → after`.
///
/// - Held in bed is lying: upright is big and would be seen. So is the head
///   at the headboard with the disparity of lying.
/// - Entering the bed nothing is reported: in bed, only lying moves it.
/// - Once lying, what is seen now rules: back at the headboard lying, lying;
///   the head away from the headboard, or upright against it, sitting up; the
///   head or a foot past the bed's sides, getting out.
/// - What says nothing holds the state.
pub(super) fn decide(before: BedPostureState, seen: BedPostureSeen) -> BedPostureState {
    use BedPostureSeen::{
        HeadAway, HeadSide, HeadboardLying, HeadboardUpright, Held, LegsOut, Unknown,
    };
    use BedPostureState::{GettingOut, InBed, Lying, SittingUp};
    match (before, seen) {
        (_, Held | HeadboardLying) => Lying,
        (_, Unknown) | (InBed, _) => before,
        (Lying | SittingUp | GettingOut, HeadAway | HeadboardUpright) => SittingUp,
        (Lying | SittingUp | GettingOut, HeadSide | LegsOut) => GettingOut,
    }
}

#[cfg(test)]
mod table;
