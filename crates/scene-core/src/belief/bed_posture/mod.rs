//! Layer 1 of the loop: the bed posture. Whether the resident they left lying
//! is still lying, sat up or is getting out. It is the sister of layer 0 and
//! has its shape: what was seen in a sample is reduced to one symbol (by the
//! second level, `refine::seen`), the rules decide the next state from the
//! previous one (`rules`, a golden table) and the machine keeps it (`machine`).
//!
//! It exists only while layer 0 believes the resident alone in bed, and never
//! changes a count (second-level rule 1). It only reports leaving lying:
//! entering the bed, nothing.
//!
//! See also: `calipso-docs/specs/r18/00-memoria-de-diseno.md` §5 and spec 1806.

mod machine;
mod rules;

pub(crate) use machine::Machine;

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
    /// Lying: held in bed, where upright would be seen.
    Lying,
    /// Sat up, after lying: the head seen away from the headboard. Sliding
    /// down the bed is sitting up too, on purpose: it deserves a look (G-13).
    SittingUp,
    /// Getting out, after lying: the head or the feet seen past the bed's
    /// sides.
    GettingOut,
}

/// What layer 1 saw in a sample: the column of its table.
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
    /// Nothing that says how they are: the state holds.
    Unknown,
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
