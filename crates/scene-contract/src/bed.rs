//! The bed: who is believed in it, layer 1's bed posture, and the
//! verification bookkeeping of a held body (white book, volume III).

use crate::{Cell, SampleRef};
use scene_geom::Rect;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- moved from scene-core/src/verification.rs
/// Who looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Verifier {
    /// The slow verifier: a vision-language model over a crop of the bed.
    Vlm,
}

// --- moved from scene-core/src/verdict.rs
// --- moved from scene-core/src/verification.rs
/// How much observation backs a body held in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeldCheckState {
    /// Nobody looked since it stopped being seen.
    Unverified,
    /// A verifier saw it: the belief has an observation behind it.
    Confirmed,
    /// A verifier looked and did not see it. It is not released: the blanket
    /// hides it from every verifier (book 4, frontier 3).
    NotConfirmed,
}

// --- moved from scene-core/src/verification.rs
/// The provenance of a held body: whether a verifier looked, and when.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HeldCheck {
    /// Unverified, confirmed or not confirmed.
    pub state: HeldCheckState,
    /// Who looked; `None` while unverified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Verifier>,
    /// The sample it looked at; `None` while unverified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looked: Option<SampleRef>,
}

/// A fixed place on the bed where depth is read in the same look as the body:
/// covered, the bed stands in for what the pose does not see (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedSite {
    /// The middle of the bed: lying, the hips, under the blanket or not.
    Centre,
}

impl BedSite {
    /// Every site, in the order they are measured and published.
    pub const ALL: [BedSite; 1] = [BedSite::Centre];
}

/// A published bed point: which site, where it falls in the image and its
/// depth, read in the same depth map as the body's keypoints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPoint {
    /// The site.
    pub name: BedSite,
    /// Pixels from the left edge.
    pub x: f32,
    /// Pixels from the top edge.
    pub y: f32,
    /// Its depth on the depth model's scale, if it was read.
    pub depth: Option<f32>,
}

/// Is someone in the bed?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BedState {
    /// Nobody in the bed.
    Empty,
    /// At least one visible occupant in the bed's stable count.
    Seen,
    /// Nobody seen, with retained occupants.
    Held,
    /// The bed geometry does not apply to the image.
    Unknown,
}

/// Someone who stopped being seen in bed without an observed exit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Held {
    /// Where they were last seen, in image pixels.
    pub bbox: Rect,
    /// The last sample they were seen in.
    pub last_seen: SampleRef,
    /// Whether a verifier looked since: only with a verifier configured
    /// (white book, volume III). It never moves the count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<HeldCheck>,
}

/// The bed: who is believed in it, seen or held.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Bed {
    /// Empty, seen, held or unknown.
    pub state: BedState,
    /// Occupants seen in the bed, in the stable count. It is belief: if the
    /// scene did not come from an observation, `republished` says so.
    pub seen: u32,
    /// Since when `state` holds; `None` before the first turn.
    pub since: Option<SampleRef>,
    /// Who is retained in the bed without being seen, oldest first.
    pub held: Vec<Held>,
    /// Stable bodies in the bed that are not occupants: they appeared without
    /// anyone coming from the floor. They do not count in the room.
    pub unexplained: u32,
}

/// Off the bed, no longer being seen is leaving the room: nobody is retained.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Floor {
    /// Occupants seen off the bed, in the stable count.
    pub seen: u32,
}

// --- moved from scene-core/src/belief/bed_posture/mod.rs
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

impl BedPostureState {
    /// The state as the contract names it: a cell is written with it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InBed => "in_bed",
            Self::Lying => "lying",
            Self::SittingUp => "sitting_up",
            Self::GettingOut => "getting_out",
        }
    }
}

/// As `as_str` writes it: one name for the cell and for display.
impl std::fmt::Display for BedPostureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
    /// What was seen as the contract names it: a cell is written with it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::HeadAway => "head_away",
            Self::HeadSide => "head_side",
            Self::LegsOut => "legs_out",
            Self::HeadboardLying => "headboard_lying",
            Self::HeadboardUpright => "headboard_upright",
            Self::Unknown => "unknown",
        }
    }
}

/// As `as_str` writes it: one name for the cell and for display.
impl std::fmt::Display for BedPostureSeen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A layer-1 change: the bed posture of the resident left lying, with the
/// shape of a layer-0 transition. Only a change while it is followed is a
/// row of `contract/bed-posture-table.txt`; getting into and out of bed are
/// layer 0's events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPostureChange {
    /// The sample in which the new state held.
    pub at: SampleRef,
    /// The row this change crossed: `before | seen | after`.
    pub cell: Cell,
    /// The state we believed, and since when.
    pub before: BedPostureBefore,
    /// What this sample saw.
    pub seen: BedPostureSeen,
    /// The state we believe.
    pub after: BedPostureState,
}

/// The bed posture before a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BedPostureBefore {
    /// The state.
    pub state: BedPostureState,
    /// Its first sample; `None` if it was never dated.
    pub since: Option<SampleRef>,
    /// Source seconds since `since`; `None` without `since` or across sessions.
    pub lasted_secs: Option<f64>,
}

// --- moved from scene-core/src/verdict.rs
// --- moved from scene-core/src/mod.rs
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

#[cfg(test)]
mod tests {
    use super::{BedPostureSeen, BedPostureState};

    /// `Display` delegates in `as_str`: the name a cell is written with and
    /// the name displayed are one (F1c).
    #[test]
    fn display_writes_what_as_str_writes() {
        for state in [
            BedPostureState::InBed,
            BedPostureState::Lying,
            BedPostureState::SittingUp,
            BedPostureState::GettingOut,
        ] {
            assert_eq!(state.to_string(), state.as_str());
        }
        for seen in [
            BedPostureSeen::Held,
            BedPostureSeen::HeadAway,
            BedPostureSeen::HeadSide,
            BedPostureSeen::LegsOut,
            BedPostureSeen::HeadboardLying,
            BedPostureSeen::HeadboardUpright,
            BedPostureSeen::Unknown,
        ] {
            assert_eq!(seen.to_string(), seen.as_str());
        }
    }
}
