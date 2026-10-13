//! The rules of layer 1: from what we believed and what we saw to what we
//! believe. A pure decider, like layer 0's `conservation`: no state, no
//! events. Every cell is in the golden table (`contract/bed-posture-table.txt`):
//! a change of rule is a diff of the table.

use super::{BedPostureSeen, BedPostureState};

/// The cell of the table: `before : seen → after`.
///
/// - Held in bed is lying: upright is big and would be seen.
/// - What says nothing holds the state.
pub(super) fn decide(before: BedPostureState, seen: BedPostureSeen) -> BedPostureState {
    match seen {
        BedPostureSeen::Held => BedPostureState::Lying,
        BedPostureSeen::Unknown => before,
    }
}

#[cfg(test)]
mod table;
