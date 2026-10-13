//! The rules of layer 1: from what we believed and what we saw to what we
//! believe. A pure decider, like layer 0's `conservation`: no state, no
//! events. Every cell is in the golden table (`contract/bed-posture-table.txt`):
//! a change of rule is a diff of the table.

use super::{BedPostureSeen, BedPostureState};

/// The cell of the table: `before : seen → after`.
///
/// - Held in bed is lying: upright is big and would be seen.
/// - Entering the bed nothing is reported: in bed, only lying moves it.
/// - Once lying, what is seen now rules: the head away from the headboard is
///   sitting up; the head or a foot past the bed's sides, getting out.
/// - What says nothing holds the state.
pub(super) fn decide(before: BedPostureState, seen: BedPostureSeen) -> BedPostureState {
    use BedPostureSeen::{HeadAway, HeadSide, Held, LegsOut, Unknown};
    use BedPostureState::{GettingOut, InBed, Lying, SittingUp};
    match (before, seen) {
        (_, Held) => Lying,
        (_, Unknown) | (InBed, _) => before,
        (Lying | SittingUp | GettingOut, HeadAway) => SittingUp,
        (Lying | SittingUp | GettingOut, HeadSide | LegsOut) => GettingOut,
    }
}

#[cfg(test)]
mod table;
