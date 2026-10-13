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
///   sitting up.
/// - What says nothing holds the state.
pub(super) fn decide(before: BedPostureState, seen: BedPostureSeen) -> BedPostureState {
    use BedPostureState::{InBed, Lying, SittingUp};
    match (before, seen) {
        (_, BedPostureSeen::Held) => Lying,
        (_, BedPostureSeen::Unknown) | (InBed, BedPostureSeen::HeadAway) => before,
        (Lying | SittingUp, BedPostureSeen::HeadAway) => SittingUp,
    }
}

#[cfg(test)]
mod table;
