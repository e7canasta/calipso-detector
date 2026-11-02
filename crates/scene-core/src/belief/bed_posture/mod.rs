//! Layer 1 of the loop: the bed posture. Whether the resident they left lying
//! is still lying, sat up or is getting out. It is the sister of layer 0 and
//! has its shape: the rules decide the next state (`rules`, a golden table)
//! and the machine keeps it (`machine`).
//!
//! It sits on layer 0 and never breaks it: layer 0's state is one of its
//! inputs, as the user wrote the row, `l1(t−1) : l0(t) + seen(t) : l1(t)`
//! (`Belief` wires it). What layer 1 perceives by itself is a [`Sighting`]
//! (the second level reduces the turn to one: `Refinement::sighting`). It never
//! changes a count (second-level rule 1) and it only reports leaving lying:
//! entering the bed, nothing.
//!
//! See also: `calipso-docs/specs/r18/00-memoria-de-diseno.md` §5 and spec 1806.

mod machine;
mod rules;

pub(crate) use machine::Machine;
pub(crate) use scene_contract::BedPostureState;

use crate::belief::InBed;
use crate::time::SampleRef;

/// What was seen in a sample of layer 1, against the headboard and the sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BedPostureSeen {
    /// Held in bed, alone: unseen without leaving.
    Held,
    /// The head seen away from the headboard, but not past the bed's sides:
    /// by definition, not lying.
    HeadAway,
    /// The head seen past the bed's sides.
    HeadSide,
    /// A foot seen past the bed's sides: getting up from below.
    LegsOut,
    /// The head at the headboard and the disparity of lying.
    HeadboardLying,
    /// The head at the headboard and the disparity of sitting up: against the
    /// headboard.
    HeadboardUpright,
    /// Nothing that says how they are: the state holds.
    Unknown,
}

/// The bed posture as layer 1 concludes it: the state, what was seen in this
/// sample and since when the state holds (a fold of the chronicle, P4).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BedPosture {
    /// How the resident in bed is.
    pub state: BedPostureState,
    /// What was seen in this sample.
    pub seen: BedPostureSeen,
    /// The first sample of this state, without interruption.
    pub since: SampleRef,
}

/// What layer 1 perceives by itself in a sample, from a fresh pose: the
/// second level's half of the row's input; layer 0's state is the other.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Sighting {
    /// The head at the headboard and the disparity of lying.
    HeadboardLying,
    /// The head at the headboard and the disparity of sitting up.
    HeadboardUpright,
    /// A foot past the bed's sides.
    LegsOut,
    /// The head past the bed's sides.
    HeadSide,
    /// The head away from the headboard, not past the sides.
    HeadAway,
    /// Nothing new: no fresh pose, or nothing that says how they are.
    #[default]
    Unknown,
}

/// What layer 1 concluded in a turn: the state and what was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Concluded {
    pub state: BedPostureState,
    pub seen: BedPostureSeen,
}

/// The row's input, as published: layer 0's state and what layer 1
/// perceived.
pub(crate) fn bed_posture_seen_of(l0: InBed, sighting: Sighting) -> BedPostureSeen {
    match (l0, sighting) {
        (InBed::Held, _) => BedPostureSeen::Held,
        (InBed::Seen, Sighting::HeadboardLying) => BedPostureSeen::HeadboardLying,
        (InBed::Seen, Sighting::HeadboardUpright) => BedPostureSeen::HeadboardUpright,
        (InBed::Seen, Sighting::LegsOut) => BedPostureSeen::LegsOut,
        (InBed::Seen, Sighting::HeadSide) => BedPostureSeen::HeadSide,
        (InBed::Seen, Sighting::HeadAway) => BedPostureSeen::HeadAway,
        (InBed::Seen, Sighting::Unknown) => BedPostureSeen::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{BedPostureSeen, BedPostureState};
    use crate::golden;

    #[test]
    fn the_names_of_a_cell_are_the_wire_names() {
        use BedPostureState::*;
        for state in [InBed, Lying, SittingUp, GettingOut] {
            assert_eq!(golden::wire(state), state.as_str());
        }
        use BedPostureSeen::*;
        for seen in [
            Held,
            HeadAway,
            HeadSide,
            LegsOut,
            HeadboardLying,
            HeadboardUpright,
            Unknown,
        ] {
            let on_the_wire = scene_contract::BedPostureSeen::from(seen);
            assert_eq!(golden::wire(on_the_wire), on_the_wire.as_str());
        }
    }
}
