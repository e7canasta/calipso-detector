//! Layer 1 of the loop: the bed posture. Whether the resident they left lying
//! is still lying, sat up or is getting out. It is the sister of layer 0 and
//! has its shape: the rules decide the next state (`rules`, a golden table)
//! and the machine keeps it (`machine`).
//!
//! It sits on layer 0 and never breaks it: layer 0's state is one of its
//! inputs, as the user wrote the row, `l1(t−1) : l0(t) + seen(t) : l1(t)`
//! (`Belief` wires it). What layer 1 perceives by itself is a [`Sighting`]
//! (the second level reduces the turn to one, `refine::seen`). It never
//! changes a count (second-level rule 1) and it only reports leaving lying:
//! entering the bed, nothing.
//!
//! See also: `calipso-docs/specs/r18/00-memoria-de-diseno.md` §5 and spec 1806.

mod machine;
mod rules;

pub(crate) use machine::Machine;

use crate::belief::InBed;
use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the resident in bed is, as layer 1 believes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedPostureState {
    /// In bed, not yet believed lying: they just came in, or were there at
    /// startup. From here only lying is reported.
    InBed,
    /// Lying: held in bed, where upright would be seen; or seen with the head
    /// at the headboard and the disparity of lying.
    Lying,
    /// Sat up, after lying: the head seen away from the headboard. Sliding
    /// down the bed is sitting up too, on purpose: it deserves a look (G-13).
    SittingUp,
    /// Getting out, after lying: the head or the feet seen past the bed's
    /// sides.
    GettingOut,
}

/// What layer 1 perceives by itself in a sample, from a fresh pose: the
/// second level's half of the row's input; layer 0's state is the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Unknown,
}

/// What a sample brought to layer 1, as published: held in bed when layer 0
/// holds the resident (there is nothing of theirs to see), otherwise what
/// layer 1 perceived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedPostureSeen {
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

impl BedPostureSeen {
    /// The row's input, as published: layer 0's state and what layer 1
    /// perceived.
    pub(crate) fn of(l0: InBed, sighting: Sighting) -> Self {
        match (l0, sighting) {
            (InBed::Held, _) => Self::Held,
            (InBed::Seen, Sighting::HeadboardLying) => Self::HeadboardLying,
            (InBed::Seen, Sighting::HeadboardUpright) => Self::HeadboardUpright,
            (InBed::Seen, Sighting::LegsOut) => Self::LegsOut,
            (InBed::Seen, Sighting::HeadSide) => Self::HeadSide,
            (InBed::Seen, Sighting::HeadAway) => Self::HeadAway,
            (InBed::Seen, Sighting::Unknown) => Self::Unknown,
        }
    }
}

/// What layer 1 concluded in a turn: the state and what was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Concluded {
    pub state: BedPostureState,
    pub seen: BedPostureSeen,
}

/// The published bed posture: the state, what was seen in this sample and
/// since when the state holds (a fold of the chronicle, P4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPosture {
    /// How the resident in bed is.
    pub state: BedPostureState,
    /// What was seen in this sample.
    pub seen: BedPostureSeen,
    /// The first sample of this state, without interruption.
    pub since: SampleRef,
}
