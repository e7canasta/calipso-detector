//! Evidence: which observation counts and how much it is worth. It never
//! decides what is believed or what is looked at. It receives readings already
//! typed: one of another kind than its probe arrives as a failing sense.

use crate::attention::LookResult;
use crate::body::Keypoint;
use crate::geom::Rect;
use crate::look::{Detection, Failure};
use crate::room::shadow::BodyShadow;
use crate::zone::Counts;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod bodies;
mod depth;
mod faces;
mod pose;

pub(crate) use bodies::{admit, consolidate};
pub(crate) use depth::{SubjectDepths, depth, depths};
pub(crate) use faces::{FaceEvidence, faces};
pub(crate) use pose::{pose, pose_body};

const NONE: Counts = Counts { floor: 0, bed: 0 };

/// What a sense read, or why it could not.
pub(crate) type Sensed<T> = Result<T, Failure>;

/// An admitted box, keeping which instrument produced it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Body {
    pub detection: Detection,
    pub source: BodySource,
    /// Where it falls for the geometry's shadow, if asked for: it decides
    /// nothing (white book, volume VII, ch. 11).
    pub shadow: Option<BodyShadow>,
    /// The knee measured on its pose, if it was doubtful and asked (white
    /// book, volume VII, ch. 15).
    pub knee_deg: Option<f32>,
}

/// A body off the bed, and whether it is near it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloorBody {
    pub detection: Detection,
    pub near_bed: bool,
    pub source: BodySource,
    /// Where it falls for the geometry's shadow, if asked for.
    pub shadow: Option<BodyShadow>,
    /// The knee measured on its pose, if it was doubtful and asked.
    pub knee_deg: Option<f32>,
}

/// The accepted bodies of a sample, per zone, from most to least confident.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Bodies {
    pub bed: Vec<Body>,
    pub floor: Vec<FloorBody>,
}

impl Bodies {
    /// How many are in each zone.
    pub(crate) fn counts(&self) -> Counts {
        Counts {
            floor: count(self.floor.len()),
            bed: count(self.bed.len()),
        }
    }

    /// How many are on the floor far from the bed: they come from the door, not from the bed.
    pub(crate) fn far(&self) -> u32 {
        count(self.floor.iter().filter(|b| !b.near_bed).count())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bed.is_empty() && self.floor.is_empty()
    }

    /// The most confident body, those in bed first.
    pub(crate) fn best(&self) -> Option<&Detection> {
        self.bed
            .first()
            .map(|b| &b.detection)
            .or_else(|| self.floor.first().map(|b| &b.detection))
    }
}

/// What the body detector proves in this sample (rule 1). The belief's `match`
/// must handle every case.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BodyEvidence {
    /// The instrument failed: it proves nothing.
    Unavailable(Failure),
    /// It ran, but without bed geometry the zone of each body is unknown: it
    /// proves nothing about the zones.
    Unzoned(Vec<Body>),
    /// The accepted bodies, per zone (there may be none).
    Seen(Bodies),
}

impl BodyEvidence {
    /// The bodies seen per zone, if the detector proved them.
    pub(crate) fn seen(&self) -> Option<&Bodies> {
        match self {
            BodyEvidence::Seen(b) => Some(b),
            _ => None,
        }
    }
}

/// The body evidence and what is recorded in the ledger.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Admission {
    pub bodies: BodyEvidence,
    /// What the main detector saw, per zone. Reinforcement does not change it:
    /// it does not re-arm reinforcement and is not a motion reference.
    pub primary: Counts,
    pub result: LookResult,
}

/// All the evidence of a turn, available from before believing until projecting.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Perceived {
    pub bodies: Admission,
    pub faces: FaceEvidence,
    /// The pose a doubtful body was seen with in this turn, before believing.
    pub posed: Option<PosedBody>,
}

/// A doubtful body's pose and its box (white book, volume VII, ch. 15): if
/// the body turns out to be the subject, the second level reads this pose
/// instead of looking again.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PosedBody {
    pub bbox: Rect,
    pub pose: Vec<Keypoint>,
}

impl Perceived {
    /// Each body zoned in this sample, with where it falls for the geometry's
    /// shadow. Without the shadow (`None`), nothing changes; with it, nothing
    /// that is believed changes either.
    pub(crate) fn cast_shadow(&mut self, site: Option<&crate::room::shadow::Site>) {
        let (Some(site), BodyEvidence::Seen(bodies)) = (site, &mut self.bodies.bodies) else {
            return;
        };
        for body in &mut bodies.bed {
            body.shadow = Some(site.body(&body.detection.bbox));
        }
        for body in &mut bodies.floor {
            body.shadow = Some(site.body(&body.detection.bbox));
        }
    }
}

impl Admission {
    /// Did the main detector see anyone?
    pub(crate) fn primary_seen(&self) -> bool {
        self.primary.bed + self.primary.floor > 0
    }
}

/// Which detector the admitted body came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodySource {
    /// The main detector, over the whole frame.
    Primary,
    /// The finer detector, over the bed.
    Reinforcement,
}

/// Why a candidate did not count as evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// Another class than a person.
    NotPerson,
    /// Below the policy's minimum confidence.
    LowConfidence,
    /// Outside the region where the look was requested.
    OutsideRegion,
}

/// Ledger quantities are `u32`; no sense returns that many.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
