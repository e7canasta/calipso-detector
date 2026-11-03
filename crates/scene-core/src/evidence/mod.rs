//! Evidence: which observation counts and how much it is worth. It never
//! decides what is believed or what is looked at. It receives readings already
//! typed: one of another kind than its probe arrives as a failing sense.

use crate::attention::LookResult;
use crate::body::Keypoint;
use crate::geom::Rect;
use crate::look::{Detection, Failure};
use crate::room::shadow::BodyShadow;
use crate::zone::Counts;

mod bodies;
mod depth;
mod faces;
mod parts;
mod pose;

pub(crate) use bodies::{admit, consolidate};
pub(crate) use depth::{SubjectDepths, depth, depths};
pub(crate) use faces::{FaceEvidence, faces};
pub(crate) use parts::{PartsMemory, decide as decide_parts};
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
    /// The zone by parts, if the profile asks for them (F2.3b).
    pub parts: Option<crate::room::shadow::PartsRead>,
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
    /// The zone by parts, if the profile asks for them (F2.3b).
    pub parts: Option<crate::room::shadow::PartsRead>,
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
            BodyEvidence::Unavailable(_) | BodyEvidence::Unzoned(_) => None,
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
    pub(crate) fn cast_shadow(&mut self, room: Option<&crate::room::Room>) {
        let (Some(room), Some(site), BodyEvidence::Seen(bodies)) =
            (room, room.and_then(|r| r.shadow()), &mut self.bodies.bodies)
        else {
            return;
        };
        let posed = self.posed.as_ref();
        let faces: Vec<crate::geom::Point> = match &self.faces {
            FaceEvidence::Seen(found) => found.iter().map(|f| f.bbox.centroid()).collect(),
            _ => Vec::new(),
        };
        let boxes: Vec<Rect> = bodies
            .bed
            .iter()
            .map(|b| b.detection.bbox)
            .chain(bodies.floor.iter().map(|b| b.detection.bbox))
            .collect();
        let bed = bodies.bed.len();
        for (i, body) in bodies.bed.iter_mut().enumerate() {
            body.shadow = Some(site.body(&body.detection.bbox));
            body.parts = cast_parts(room, posed, &boxes, &faces, i, body.shadow.as_ref());
        }
        for (i, body) in bodies.floor.iter_mut().enumerate() {
            body.shadow = Some(site.body(&body.detection.bbox));
            body.parts = cast_parts(room, posed, &boxes, &faces, bed + i, body.shadow.as_ref());
        }
    }
}

/// The parts of body `index` among `boxes` (F2.3b). The pose is the one read
/// this turn, if it was of this body; its face is the turn's face alone in its
/// box. `None` without the parts in the profile, or without a shadow to say them in.
fn cast_parts(
    room: &crate::room::Room,
    posed: Option<&PosedBody>,
    boxes: &[Rect],
    faces: &[crate::geom::Point],
    index: usize,
    shadow: Option<&BodyShadow>,
) -> Option<crate::room::shadow::PartsRead> {
    let bbox = *boxes.get(index)?;
    let shadow = shadow?;
    let pose = posed.filter(|p| p.bbox == bbox).map(|p| p.pose.as_slice());
    let face = crate::room::Room::face_of(boxes, faces, index);
    let verdict = room.parts(bbox, pose, face)?;
    let turn = room.turn(boxes, faces, index, shadow.share)?;
    let frame = pose.and_then(|pose| room.frame_posture(pose));
    Some(crate::room::shadow::parts_read(&verdict, turn, frame))
}

impl Admission {
    /// Did the main detector see anyone?
    pub(crate) fn primary_seen(&self) -> bool {
        self.primary.bed + self.primary.floor > 0
    }
}

pub(crate) use scene_contract::{BodySource, RejectReason};
/// Ledger quantities are `u32`; no sense returns that many.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
