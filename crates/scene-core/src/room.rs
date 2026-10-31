//! The room's geometry (white book, volume VII, ch. 01): the only module that
//! answers the questions of space. Layer 0 asks where a body is, in bed or on
//! the floor, and whether it is near the bed (P1, P2); layer 1 and the second
//! level ask where the head, a part, a box or a foot is against the bed and
//! its headboard (P3 to P6, P8). They get named facts back, never a plane or
//! a polygon. It answers from the map and the policy; it remembers nothing
//! and counts nothing.
//!
//! The crops (P7) are the map's: `Map::focus_region` and its siblings.

use crate::geom::{Point, Quad, Rect, Zone};
use crate::map::{BedArea, BedPlane, Geometry, Map};
use crate::policy::AdmissionPolicy;

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
