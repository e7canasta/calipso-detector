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

use crate::body::{Joint, Keypoint};
use crate::geom::{Point, Quad, Rect, Zone};
use crate::map::{BedArea, BedPlane, Geometry, Map};
use crate::policy::{AdmissionPolicy, Policy, TwoPlanesPolicy};

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
/// and do not know which one they got.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Room {
    frontal: Frontal,
    two_planes: Option<TwoPlanes>,
}

impl Room {
    /// The room of this turn, if the bed's geometry applies to its frame: in
    /// two planes if the profile asks for it and the planes can be built,
    /// frontal otherwise.
    pub(crate) fn of(map: &Map, geometry: Geometry, policy: &Policy) -> Option<Self> {
        let frontal = Frontal::of(map, geometry, &policy.admission)?;
        let two_planes = policy
            .two_planes
            .and_then(|two| TwoPlanes::of(&frontal, map, two));
        Some(Self {
            frontal,
            two_planes,
        })
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

    pub(crate) fn past_the_sides(&self, p: Point) -> bool {
        self.frontal.past_the_sides(p)
    }

    pub(crate) fn beyond_side(&self, p: Point) -> Option<f64> {
        self.frontal.beyond_side(p)
    }
}

#[cfg(test)]
impl Room {
    /// A frontal room, for the layers' tests.
    pub(crate) fn new(bed: &Quad, headboard: Option<Zone>, admission: &AdmissionPolicy) -> Self {
        Self {
            frontal: Frontal::new(bed, headboard, admission),
            two_planes: None,
        }
    }
}

/// The camera that sees the bed on the diagonal (volume VII, ch. 14): where a
/// body is by the share of its box over the mattress and by its support (the
/// centre of the box's lower edge) against the bed's footprint on the floor,
/// in metres. Every other question, frontal answers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TwoPlanes {
    mattress: BedPlane,
    floor: BedPlane,
    size: [f32; 2],
    policy: TwoPlanesPolicy,
}

impl TwoPlanes {
    /// With the mattress, the footprint and the bed's size; without any of
    /// them, or with degenerate corners, there are no two planes.
    fn of(frontal: &Frontal, map: &Map, policy: TwoPlanesPolicy) -> Option<Self> {
        Some(Self {
            mattress: frontal.plane?,
            floor: BedPlane::of(map.site.bed_floor.as_ref()?)?,
            size: map.site.bed_size_m?,
            policy,
        })
    }

    /// P1, P2: in bed with enough of the box over the mattress and the
    /// support close to the footprint; on the floor otherwise, near the bed
    /// with the support within the policy's distance.
    fn body_zone(&self, bbox: &Rect) -> BodyZone {
        let distance = self.support_m(bbox);
        if self.mattress.share(bbox) >= self.policy.share && distance <= self.policy.support_m {
            return BodyZone::Bed;
        }
        BodyZone::Floor {
            near_bed: distance <= self.policy.near_m,
        }
    }

    /// Doubtful: with the knee in the profile, on the floor by its support
    /// alone, with enough of the box over the mattress.
    fn doubtful(&self, bbox: &Rect) -> Option<Doubt> {
        self.policy.knee_bent_deg?;
        let share = self.mattress.share(bbox);
        (share >= self.policy.share && self.support_m(bbox) > self.policy.support_m)
            .then_some(Doubt { share })
    }

    /// A doubtful body with its knee bent to the policy's angle or less rests
    /// its pelvis on the mattress: in bed. Straight, or without a knee in
    /// sight, it stays where its box puts it.
    fn posed(&self, bbox: &Rect, pose: &[Keypoint], min_score: f32) -> Posed {
        let zone = self.body_zone(bbox);
        let knee_deg = knee_deg(pose, min_score);
        let bent = matches!(
            (self.policy.knee_bent_deg, knee_deg),
            (Some(bent), Some(knee)) if knee <= bent
        );
        Posed {
            zone: if bent && self.doubtful(bbox).is_some() {
                BodyZone::Bed
            } else {
                zone
            },
            knee_deg,
        }
    }

    /// How far the box's support is from the footprint, in metres.
    fn support_m(&self, bbox: &Rect) -> f32 {
        let support = Point {
            x: bbox.centroid().x,
            y: bbox.y1,
        };
        let (u, v) = self.floor.project(support);
        distance_m(u as f32, v as f32, self.size)
    }
}

/// The hip–knee–ankle angle of a pose in COCO order, in degrees: 180 is a
/// straight leg. Each joint is the midpoint of the sides seen; without a hip,
/// a knee or an ankle in sight, none.
pub(crate) fn knee_deg(pose: &[Keypoint], min_score: f32) -> Option<f32> {
    let joint = |sides: [Joint; 2]| {
        let seen: Vec<&Keypoint> = sides
            .iter()
            .filter_map(|&j| {
                let i = Joint::COCO.iter().position(|&c| c == j)?;
                pose.get(i).filter(|k| k.is_seen(min_score))
            })
            .collect();
        let n = seen.len() as f32;
        (!seen.is_empty()).then(|| Point {
            x: seen.iter().map(|k| k.x).sum::<f32>() / n,
            y: seen.iter().map(|k| k.y).sum::<f32>() / n,
        })
    };
    let hip = joint([Joint::LeftHip, Joint::RightHip])?;
    let knee = joint([Joint::LeftKnee, Joint::RightKnee])?;
    let ankle = joint([Joint::LeftAnkle, Joint::RightAnkle])?;
    let (a, b) = (
        (hip.x - knee.x, hip.y - knee.y),
        (ankle.x - knee.x, ankle.y - knee.y),
    );
    let lengths = a.0.hypot(a.1) * b.0.hypot(b.1);
    if lengths <= 0.0 {
        return None;
    }
    let cos = ((a.0 * b.0 + a.1 * b.1) / lengths).clamp(-1.0, 1.0);
    Some(cos.acos().to_degrees())
}

/// How far a point in bed coordinates is from the bed, in metres: past a side
/// by `u`, past an end by `v`, both when past a corner. Zero inside.
pub(crate) fn distance_m(u: f32, v: f32, [width, length]: [f32; 2]) -> f32 {
    let past = |c: f32| (-c).max(c - 1.0).max(0.0);
    (past(u) * width).hypot(past(v) * length)
}

/// The camera that sees the bed along the image (volume VII, ch. 02): the
/// engine's answers as they were, moved.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frontal {
    bed: Quad,
    /// The bed's own coordinates; `None` if its corners are degenerate.
    plane: Option<BedPlane>,
    headboard: Option<Zone>,
    near: Near,
    /// `Some`: a box is in bed when at least this share of it is
    /// (`AdmissionPolicy::bed_frame`).
    box_share: Option<f32>,
}

/// What "near the bed" is measured against.
#[derive(Debug, Clone, Copy)]
enum Near {
    /// The bed's bounding box grown by the margin: a box overlapping it.
    Bounds(Rect),
    /// In the bed frame, the bed grown in its own coordinates: a box touching it.
    Grown(Quad),
}

impl Frontal {
    /// The room of this turn, if the bed's geometry applies to its frame.
    pub(crate) fn of(map: &Map, geometry: Geometry, admission: &AdmissionPolicy) -> Option<Self> {
        let bed = map.bed_for(geometry)?;
        Some(Self::new(bed, map.headboard_for(geometry), admission))
    }

    pub(crate) fn new(bed: &Quad, headboard: Option<Zone>, admission: &AdmissionPolicy) -> Self {
        let margin = admission.near_bed_margin;
        let near = match admission
            .bed_frame
            .and_then(|_| crate::map::grown_bed(bed, f64::from(margin)))
        {
            Some(grown) => Near::Grown(grown),
            None => Near::Bounds(bed.bounds().expand(margin)),
        };
        Self {
            bed: *bed,
            plane: BedPlane::of(bed),
            headboard,
            near,
            box_share: admission.bed_frame,
        }
    }

    // --- Layer 0.

    /// P1, P2: in bed if the box's centre is in the bed; on the floor
    /// otherwise, near the bed if the box reaches it.
    pub(crate) fn body_zone(&self, bbox: &Rect) -> BodyZone {
        if self.bed.contains(bbox.centroid()) {
            return BodyZone::Bed;
        }
        let near_bed = match &self.near {
            Near::Grown(grown) => grown.touches(bbox),
            Near::Bounds(near) => bbox.iou(near) > 0.0,
        };
        BodyZone::Floor { near_bed }
    }

    // --- Layer 1 and the second level.

    /// Does the bed have a plane to be in? Without one, no part is placed.
    pub(crate) fn has_plane(&self) -> bool {
        self.plane.is_some()
    }

    /// Is the headboard delimited? Without it, where the head is is unknown.
    pub(crate) fn has_headboard(&self) -> bool {
        self.headboard.is_some()
    }

    /// P3: where a point of the head falls: the headboard first; away from
    /// it, in the bed's `u/v`, past its sides or inside it.
    pub(crate) fn head_area(&self, p: Point) -> HeadArea {
        if self.in_headboard(p) {
            return HeadArea::Headboard;
        }
        match self.plane.map(|plane| plane.locate(p)) {
            None | Some(BedArea::PastTheEnds) => HeadArea::Elsewhere,
            Some(BedArea::Inside) => HeadArea::Bed,
            Some(BedArea::PastTheSides) => HeadArea::PastTheSides,
        }
    }

    /// P4: is the point in the bed, `[0, 1]²` of its plane?
    pub(crate) fn in_bed(&self, p: Point) -> bool {
        self.plane.is_some_and(|plane| plane.inside(p))
    }

    /// P4: is the point in the headboard?
    pub(crate) fn in_headboard(&self, p: Point) -> bool {
        self.headboard.is_some_and(|h| h.contains(p))
    }

    /// P4: does the box fall entirely in the headboard?
    pub(crate) fn box_in_headboard(&self, r: &Rect) -> bool {
        self.headboard.is_some_and(|h| h.contains_rect(r))
    }

    /// P5: is the box in bed? Its four corners inside it or, in the bed
    /// frame, at least its share of the box over it.
    pub(crate) fn box_in_bed(&self, r: &Rect) -> bool {
        let Some(plane) = self.plane else {
            return false;
        };
        match self.box_share {
            Some(share) => plane.share(r) >= share,
            None => corners(r).iter().all(|&p| plane.inside(p)),
        }
    }

    /// P6: is the point past the bed's sides (`u < 0` or `u > 1`)?
    pub(crate) fn past_the_sides(&self, p: Point) -> bool {
        self.plane
            .is_some_and(|plane| plane.locate(p) == BedArea::PastTheSides)
    }

    /// P6: how far past the nearest side the point is, in bed widths:
    /// negative inside. `None` without the bed's plane.
    pub(crate) fn beyond_side(&self, p: Point) -> Option<f64> {
        self.plane.map(|plane| {
            let (u, _) = plane.project(p);
            (-u).max(u - 1.0)
        })
    }
}

fn corners(r: &Rect) -> [Point; 4] {
    [
        Point { x: r.x0, y: r.y0 },
        Point { x: r.x1, y: r.y0 },
        Point { x: r.x1, y: r.y1 },
        Point { x: r.x0, y: r.y1 },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADMISSION: AdmissionPolicy = AdmissionPolicy {
        min_confidence: 0.5,
        same_body_iou: 0.5,
        near_bed_margin: 0.10,
        bed_frame: None,
    };

    /// A bed twice as long as wide seen from the corner, at 45°: headboard
    /// up and to the right, foot down and to the left.
    fn diagonal(bed_frame: Option<f32>) -> Frontal {
        let bed = Quad([
            Point { x: 370.0, y: 88.0 },
            Point { x: 512.0, y: 230.0 },
            Point { x: 230.0, y: 512.0 },
            Point { x: 88.0, y: 370.0 },
        ]);
        Frontal::new(
            &bed,
            None,
            &AdmissionPolicy {
                bed_frame,
                ..ADMISSION
            },
        )
    }

    #[test]
    fn a_body_lying_on_the_diagonal_spills_the_corners_of_its_box() {
        // Lying along the bed, head to foot: its box's corners fall far past
        // the rails, most of the box is in bed.
        let lying = Rect::new(180.0, 180.0, 420.0, 420.0);
        assert!(!diagonal(None).box_in_bed(&lying));
        assert!(diagonal(Some(0.7)).box_in_bed(&lying));
        // Standing beside the bed, at its side: little of the box is in it.
        let beside = Rect::new(400.0, 300.0, 470.0, 500.0);
        assert!(!diagonal(Some(0.7)).box_in_bed(&beside));
    }

    /// A pose with only the hips, knees and ankles at these points, both
    /// sides at the same place.
    fn legs(hip: (f32, f32), knee: (f32, f32), ankle: (f32, f32), score: f32) -> Vec<Keypoint> {
        let at = |(x, y): (f32, f32), score| Keypoint { x, y, score };
        let mut pose = vec![at((0.0, 0.0), 0.0); 11];
        for (p, s) in [(hip, 0.9), (knee, 0.9), (ankle, score)] {
            pose.extend([at(p, s), at(p, s)]);
        }
        pose
    }

    #[test]
    fn the_knee_is_straight_standing_and_bent_sitting() {
        // White book, volume VII, annex A, A2: dig1 at 145 s and at 90 s.
        let standing = legs((1267.0, 810.0), (1237.0, 923.0), (1221.0, 1024.0), 0.9);
        let sitting = legs((1450.0, 767.0), (1527.0, 833.0), (1512.0, 941.0), 0.9);
        let knee = |pose: &[Keypoint]| knee_deg(pose, 0.5).map(f32::round);
        assert_eq!(knee(&standing), Some(174.0));
        assert_eq!(knee(&sitting), Some(123.0));
        // Without the ankles in sight there is no knee.
        let covered = legs((1450.0, 767.0), (1527.0, 833.0), (1512.0, 941.0), 0.1);
        assert_eq!(knee_deg(&covered, 0.5), None);
        assert_eq!(knee_deg(&[], 0.5), None);
    }

    #[test]
    fn without_the_beds_plane_nothing_is_in_bed_and_the_head_is_elsewhere() {
        let p = Point { x: 1.0, y: 1.0 };
        let headboard = Zone::Box(Rect::new(0.0, 0.0, 10.0, 10.0));
        let room = Frontal::new(&Quad([p; 4]), Some(headboard), &ADMISSION);
        assert!(!room.has_plane());
        assert!(!room.in_bed(p) && !room.past_the_sides(p));
        assert!(!room.box_in_bed(&Rect::new(0.0, 0.0, 2.0, 2.0)));
        assert_eq!(room.beyond_side(p), None);
        assert_eq!(room.head_area(p), HeadArea::Headboard);
        let away = Point { x: 50.0, y: 50.0 };
        assert_eq!(room.head_area(away), HeadArea::Elsewhere);
    }
}
