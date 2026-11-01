//! The ledger: what was looked at, what was not and why. It is published
//! language (rule 12): the contract re-exports these types.

use crate::evidence::RejectReason;
use crate::geom::Rect;
use crate::look::{Failure, LookKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Why nothing was looked at (rule 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// This sense was not requested.
    NotRequested,
    /// No body and no delimited headboard: nowhere to look for faces.
    NoFaceRegion,
    /// This loss was already reinforced: once per loss.
    LatchSpent,
    /// Nobody was seen in bed: there is no loss to reinforce.
    NoSubjectInBed,
    /// The geometry does not apply: where the bed is, is unknown.
    NoBedGeometry,
    /// The subject did not move since the last pose: the previous one holds.
    SubjectStill,
    /// The subject was posed in this turn as a doubtful body, before
    /// believing: that pose is read without looking again.
    PosedBefore,
    /// The subject is retained, not seen in this sample.
    NoCurrentSubject,
    /// Without a pose there are no points to measure depth at.
    NoPose,
    /// Someone is in the room: a person is not learned as the bed.
    RoomOccupied,
    /// Faces are in sight: someone may be covering the bed.
    FacesPresent,
    /// The last attempt failed and the retry delay has not passed yet.
    RetryWait,
    /// A verification is already in flight: one at a time per camera.
    VerifyBusy,
    /// The camera's verification budget for the hour is spent.
    BudgetSpent,
    /// The doubt cleared while the burst was being collected: nothing is asked
    /// (white book, volume III, ch. 12, V17).
    BurstResolved,
    /// What the burst asked about no longer holds: it is dropped unasked.
    BurstStale,
}

/// What a look returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookResult {
    /// The sense read: how many candidates, and why those that did not count
    /// were discarded.
    Read {
        /// How many the sense returned, before admission.
        candidates: u32,
        /// The discarded ones, grouped by reason.
        #[serde(with = "rejections")]
        #[schemars(with = "Vec<Rejected>")]
        rejected: Vec<RejectReason>,
    },
    /// The sense failed: it proves nothing (rule 1).
    Failed {
        /// How it failed.
        failure: Failure,
    },
    /// The verifier took the request: the answer comes in a later turn.
    Dispatched,
    /// The shell kept the crop for a burst.
    Kept {
        /// How sharp the shell measured it.
        sharpness: f32,
        /// Whether it counts for the burst: sharp enough for this camera.
        counted: bool,
    },
}

/// An entry of attention's ledger (rule 12): what was decided to look at or
/// not, and what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Look {
    /// Which sense.
    pub kind: LookKind,
    /// Asked, with what came back, or skipped, with why.
    pub outcome: LookOutcome,
}

/// What came of a look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookOutcome {
    /// A look was requested.
    Asked {
        /// `None`: the whole frame.
        region: Option<Rect>,
        /// What the sense returned.
        result: LookResult,
        /// What the shell measured; the core only records it (rule 9).
        spent_ms: f64,
    },
    /// It was decided not to look, with a reason.
    Skipped {
        /// Why not.
        reason: SkipReason,
    },
}

// The published form groups the rejections; the internal ledger keeps each reason.
#[derive(Serialize, Deserialize, JsonSchema)]
struct Rejected {
    /// Why they did not count.
    reason: RejectReason,
    /// How many, for that reason.
    count: u32,
}

mod rejections {
    use super::{RejectReason, Rejected};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub(super) fn serialize<S: Serializer>(
        reasons: &[RejectReason],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut groups: Vec<Rejected> = Vec::new();
        for reason in reasons {
            if let Some(group) = groups.iter_mut().find(|g| g.reason == *reason) {
                group.count = group.count.saturating_add(1);
            } else {
                groups.push(Rejected {
                    reason: *reason,
                    count: 1,
                });
            }
        }
        groups.serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<RejectReason>, D::Error> {
        let groups = Vec::<Rejected>::deserialize(deserializer)?;
        Ok(groups
            .into_iter()
            .flat_map(|g| {
                std::iter::repeat_n(g.reason, usize::try_from(g.count).unwrap_or(usize::MAX))
            })
            .collect())
    }
}

impl LookResult {
    /// The sense read `n` things and none were discarded.
    pub fn read(n: usize) -> Self {
        LookResult::Read {
            candidates: u32::try_from(n).unwrap_or(u32::MAX),
            rejected: Vec::new(),
        }
    }

    /// A look whose sense failed.
    pub fn failed(failure: Failure) -> Self {
        LookResult::Failed { failure }
    }
}

impl Look {
    /// The result, if a look was requested.
    pub fn result(&self) -> Option<&LookResult> {
        match &self.outcome {
            LookOutcome::Asked { result, .. } => Some(result),
            LookOutcome::Skipped { .. } => None,
        }
    }
}

/// What was looked at during the turn, in order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Attention {
    /// One per sense considered, in the order of the turn.
    pub looks: Vec<Look>,
}
