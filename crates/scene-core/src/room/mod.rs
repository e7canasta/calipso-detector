//! The room's geometry (white book, volume VII, ch. 01): the only module that
//! answers the questions of space. Layer 0 asks where a body is, in bed or on
//! the floor, and whether it is near the bed (P1, P2); layer 1 and the second
//! level ask where the head, a part, a box or a foot is against the bed and
//! its headboard (P3 to P6, P8). They get named facts back, never a plane or
//! a polygon. It answers from the map and the policy; it remembers nothing
//! and counts nothing.
//!
//! The profile chooses the piece (volume VII, ch. 14): `Frontal`, the engine
//! as it was, or `TwoPlanes`, for a camera that sees the bed on the diagonal.
//!
//! The crops (P7) are the map's: `Map::focus_region` and its siblings.

use crate::body::Keypoint;
use crate::geom::{Point, Rect};
#[cfg(test)]
use crate::geom::{Quad, Zone};
use crate::map::{Geometry, Map};
use crate::policy::{AdmissionPolicy, PartsPolicy, ShadowPolicy, TwoPlanesPolicy};
use crate::units::{Confidence, PositiveMetres};

mod cascade;
mod frontal;
pub(crate) mod shadow;
mod three_planes;
mod turn;
mod two_planes;

pub(crate) use cascade::{Placed, Step};
use frontal::Frontal;
use three_planes::ThreePlanes;
pub(crate) use turn::Turn;
use two_planes::TwoPlanes;

/// Where a point of the head falls against the bed and its headboard: the
/// geometry's answer, which `head` turns into the fact it publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadArea {
    /// In the headboard.
    Headboard,
    /// In bed, away from the headboard.
    Bed,
    /// Past the bed's sides (`u < 0` or `u > 1`).
    PastTheSides,
    /// Anywhere else, or without the bed's plane.
    Elsewhere,
}

/// Where a body falls in the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyZone {
    Bed,
    Floor { near_bed: bool },
}

/// A body the box alone cannot place (white book, volume VII, ch. 15): its
/// pose is asked before believing. When there are several, the one with the
/// most of its box over the mattress is asked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Doubt {
    /// The share of its box over the mattress.
    pub(crate) share: f32,
}

/// Where a doubtful body is with its pose, and the knee measured on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Posed {
    pub(crate) zone: BodyZone,
    pub(crate) knee_deg: Option<f32>,
}

/// The room's geometry of a turn: the piece the profile chose. Frontal
/// answers every question; in two planes, the mattress and the footprint
/// answer where a body is (P1, P2) and frontal the rest. The layers ask it
/// and do not know which one they got. Next to them, the shadow of a
/// geometry that does not decide yet, if the profile asks for it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Room {
    frontal: Frontal,
    two_planes: Option<TwoPlanes>,
    shadow: Option<shadow::Site>,
    /// The frame's planes, for the parts (F2.3b) and the turn; `None` without the bed's footprint.
    planes: Option<ThreePlanes>,
    /// The cascade's thresholds, when the profile asks for the parts and the pose is on.
    cascade: Option<cascade::CascadeRules>,
}

/// The space's rules of the policy, one slice (ADR 2026-11-02, politica-en-rebanadas,
/// *Introduce Parameter Object*): which body counts and where, the room in two
/// planes, and the shadow. The room and the map's state read it together.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpaceRules {
    pub(crate) admission: AdmissionPolicy,
    pub(crate) two_planes: Option<TwoPlanesPolicy>,
    pub(crate) shadow: Option<ShadowPolicy>,
    /// `[occupancy.parts]`: the cascade's thresholds (F2.3b). Read only with the planes and the pose.
    pub(crate) parts: Option<PartsPolicy>,
    /// The pose's keypoint score, from `[senses.pose]`: without a pose there are no parts.
    pub(crate) min_keypoint: Option<Confidence>,
}

impl Room {
    /// The room of this turn, if the bed's geometry applies to its frame: in
    /// two planes if the space's rules ask for it and the planes can be built,
    /// frontal otherwise.
    pub(crate) fn of(map: &Map, geometry: Geometry, space: &SpaceRules) -> Option<Self> {
        let frontal = Frontal::of(map, geometry, &space.admission)?;
        // The one source of the bed's planes: the room in two planes and the
        // shadow both read them from here.
        let planes = ThreePlanes::of(map, geometry);
        let two_planes = space
            .two_planes
            .and_then(|two| TwoPlanes::of(planes.as_ref()?, two));
        let cascade = match (
            space.parts.as_ref(),
            space.two_planes.as_ref(),
            space.min_keypoint,
        ) {
            (Some(parts), Some(two), Some(kp)) => {
                planes.map(|_| cascade::CascadeRules::of(parts, two, kp))
            }
            _ => None,
        };
        Some(Self {
            frontal,
            two_planes,
            shadow: planes
                .as_ref()
                .and_then(|planes| shadow::Site::of(planes, space.shadow.as_ref())),
            planes,
            cascade,
        })
    }

    /// The parts of one body (F2.3b): its zone by the cascade and the step that
    /// decided it. `pose` is the body's keypoints, if its pose was looked at.
    /// `None` without the parts in the profile.
    pub(crate) fn parts(
        &self,
        bbox: Rect,
        pose: Option<&[Keypoint]>,
        face: Option<Point>,
    ) -> Option<cascade::Verdict> {
        let body = cascade::Body { bbox, pose, face };
        cascade::decide(self.planes.as_ref()?, &body, self.cascade.as_ref()?)
    }

    /// The face of body `index` among `boxes` (F2.3b): see `turn::face_of`.
    pub(crate) fn face_of(boxes: &[Rect], faces: &[Point], index: usize) -> Option<Point> {
        turn::face_of(boxes, faces, index)
    }

    /// The share over the mattress of a floor body that the cascade cannot
    /// place without its pose (F2.3b, the pose's choice): `Some` only when the
    /// parts are on and the step is «the pose was not looked at».
    pub(crate) fn pose_needed(&self, bbox: Rect, face: Option<Point>) -> Option<f32> {
        let verdict = self.parts(bbox, None, face)?;
        (verdict.step == cascade::Step::PoseNotLooked).then_some(verdict.measures.share)
    }

    /// The turn of body `index` among `boxes` (F2.3b): which direction is up
    /// for its pose. `None` without the parts.
    pub(crate) fn turn(
        &self,
        boxes: &[Rect],
        faces: &[Point],
        index: usize,
        share: f32,
    ) -> Option<turn::Turn> {
        let planes = self.planes.as_ref()?;
        let rules = self.cascade.as_ref()?;
        Some(turn::choose(
            planes,
            boxes,
            faces,
            index,
            share,
            rules.share,
        ))
    }

    /// What the shadow measures bodies against, if the profile asks for it
    /// and the bed has a plane.
    pub(crate) fn shadow(&self) -> Option<&shadow::Site> {
        self.shadow.as_ref()
    }

    /// P1, P2: where a body is, by the piece.
    pub(crate) fn body_zone(&self, bbox: &Rect) -> BodyZone {
        match &self.two_planes {
            Some(two) => two.body_zone(bbox),
            None => self.frontal.body_zone(bbox),
        }
    }

    /// P1 in doubt: can the box alone not place this body? Frontal always
    /// can; in two planes, only with the knee in the profile.
    pub(crate) fn doubtful(&self, bbox: &Rect) -> Option<Doubt> {
        self.two_planes.as_ref()?.doubtful(bbox)
    }

    /// P1 with the pose: where a body is when its pose was seen. Only a
    /// doubtful body can change zone; any other is where its box puts it.
    pub(crate) fn posed(&self, bbox: &Rect, pose: &[Keypoint], min_score: f32) -> Posed {
        match &self.two_planes {
            Some(two) => two.posed(bbox, pose, min_score),
            None => Posed {
                zone: self.frontal.body_zone(bbox),
                knee_deg: None,
            },
        }
    }

    pub(crate) fn has_plane(&self) -> bool {
        self.frontal.has_plane()
    }

    pub(crate) fn has_headboard(&self) -> bool {
        self.frontal.has_headboard()
    }

    pub(crate) fn head_area(&self, p: Point) -> HeadArea {
        self.frontal.head_area(p)
    }

    pub(crate) fn in_bed(&self, p: Point) -> bool {
        self.frontal.in_bed(p)
    }

    pub(crate) fn in_headboard(&self, p: Point) -> bool {
        self.frontal.in_headboard(p)
    }

    pub(crate) fn box_in_headboard(&self, r: &Rect) -> bool {
        self.frontal.box_in_headboard(r)
    }

    pub(crate) fn box_in_bed(&self, r: &Rect) -> bool {
        self.frontal.box_in_bed(r)
    }

    /// P6 for a foot: is it out of the bed? Frontal, past the mattress's
    /// sides; in two planes, out of the footprint on the floor, where a foot
    /// rests (white book, volume VII, ch. 04).
    pub(crate) fn foot_out(&self, p: Point) -> bool {
        match &self.two_planes {
            Some(two) => two.foot_beyond(p) > 0.0,
            None => self.frontal.past_the_sides(p),
        }
    }

    /// P6 for a foot: how far out of the bed it is, in bed widths; negative
    /// inside. `None` without the bed's plane.
    pub(crate) fn foot_beyond(&self, p: Point) -> Option<f64> {
        match &self.two_planes {
            Some(two) => Some(two.foot_beyond(p)),
            None => self.frontal.beyond_side(p),
        }
    }
}

#[cfg(test)]
impl Room {
    /// A frontal room, for the layers' tests.
    pub(crate) fn new(bed: &Quad, headboard: Option<Zone>, admission: &AdmissionPolicy) -> Self {
        Self {
            frontal: Frontal::new(bed, headboard, admission),
            two_planes: None,
            shadow: None,
            planes: None,
            cascade: None,
        }
    }
}

/// How far a point in bed coordinates is from the bed, in metres: past a side
/// by `u`, past an end by `v`, both when past a corner. Zero inside.
pub(crate) fn distance_m(u: f32, v: f32, [width, length]: [PositiveMetres; 2]) -> f32 {
    let past = |c: f32| (-c).max(c - 1.0).max(0.0);
    (past(u) * width.get()).hypot(past(v) * length.get())
}

/// The bed's size in metres, as the map publishes it, if both sides are
/// positive: the profile validates them, so `None` only for a map built by hand.
pub(crate) fn bed_sizes([width, length]: [f32; 2]) -> Option<[PositiveMetres; 2]> {
    Some([
        PositiveMetres::new(width).ok()?,
        PositiveMetres::new(length).ok()?,
    ])
}
