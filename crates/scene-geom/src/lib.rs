//! Geometry in whole-frame coordinates (pixels): points, boxes and
//! quadrilaterals. It is the leaf of the language (ADR 2026-11-02,
//! crates-del-lenguaje): no domain, no IO, and nothing above it depends on
//! the core for these shapes.
// What is `pub` is reachable from outside; the rest says `pub(crate)`.
#![warn(missing_docs)]
#![warn(unreachable_pub)]
// A cast that loses says so at its site, with its reason: the lints are on.
#![warn(clippy::cast_precision_loss)]
#![warn(clippy::cast_possible_truncation)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A point on the image plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    /// Pixels from the left edge.
    pub x: f32,
    /// Pixels from the top edge.
    pub y: f32,
}

/// An axis-aligned rectangle: top-left and bottom-right corners.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rect {
    /// Left edge, in pixels.
    pub x0: f32,
    /// Top edge, in pixels.
    pub y0: f32,
    /// Right edge, in pixels.
    pub x1: f32,
    /// Bottom edge, in pixels.
    pub y1: f32,
}

impl Rect {
    /// From its corners: top-left `(x0, y0)` and bottom-right `(x1, y1)`.
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// Its centre.
    pub fn centroid(&self) -> Point {
        Point {
            x: (self.x0 + self.x1) / 2.0,
            y: (self.y0 + self.y1) / 2.0,
        }
    }

    /// Does it contain the point? The edges count as inside; a `NaN`
    /// coordinate is never inside (`NaN` compares false to everything).
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }

    /// In square pixels; zero if it is inverted (its right edge left of its
    /// left one, or its bottom above its top) and zero when a coordinate is
    /// not a number.
    pub fn area(&self) -> f32 {
        (self.x1 - self.x0).max(0.0) * (self.y1 - self.y0).max(0.0)
    }

    /// Does `other` fall entirely inside this box?
    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x0 >= self.x0 && other.y0 >= self.y0 && other.x1 <= self.x1 && other.y1 <= self.y1
    }

    /// Intersection over union: how much two boxes overlap (0 to 1). Boxes
    /// that do not overlap score 0, and so do inverted or not-a-number ones
    /// (their union is not positive).
    pub fn iou(&self, other: &Rect) -> f32 {
        let inter = Rect {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
        .area();
        let union = self.area() + other.area() - inter;
        if union > 0.0 { inter / union } else { 0.0 }
    }

    /// The box grown by a fraction of its width and height on each side.
    pub fn expand(&self, fraction: f32) -> Rect {
        let dx = (self.x1 - self.x0) * fraction;
        let dy = (self.y1 - self.y0) * fraction;
        Rect {
            x0: self.x0 - dx,
            y0: self.y0 - dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
        }
    }
}

/// A quadrilateral: the bed seen in perspective.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Quad(pub [Point; 4]);

impl Quad {
    /// The quadrilateral of a rectangle, clockwise.
    pub fn from_rect(r: Rect) -> Self {
        Self([
            Point { x: r.x0, y: r.y0 },
            Point { x: r.x1, y: r.y0 },
            Point { x: r.x1, y: r.y1 },
            Point { x: r.x0, y: r.y1 },
        ])
    }

    /// The smallest rectangle that contains it.
    pub fn bounds(&self) -> Rect {
        let xs = self.0.map(|p| p.x);
        let ys = self.0.map(|p| p.y);
        Rect {
            x0: xs.iter().copied().fold(f32::INFINITY, f32::min),
            y0: ys.iter().copied().fold(f32::INFINITY, f32::min),
            x1: xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            y1: ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        }
    }

    /// Does the point fall inside? Ray casting: works for non-convex
    /// quadrilaterals.
    pub fn contains(&self, p: Point) -> bool {
        let v = &self.0;
        let mut inside = false;
        // Each vertex with the previous one, closing the polygon.
        let previous = v.iter().cycle().skip(v.len() - 1);
        for (&a, &b) in v.iter().zip(previous) {
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    }
}

impl Quad {
    /// Does it overlap the box, with some area? The quadrilateral convex:
    /// a corner of one inside the other, or two edges that cross. A graze is
    /// not an overlap: segments that merely touch — at a point or along a
    /// line — do not cross, and a corner exactly on an edge is inside only
    /// when that shape's `contains` says so (`Quad` is ray casting, which
    /// makes no promise about its own boundary).
    pub fn touches(&self, r: &Rect) -> bool {
        let boxed = Quad::from_rect(*r).0;
        let edges = |v: [Point; 4]| v.into_iter().zip(v.into_iter().cycle().skip(1));
        boxed.iter().any(|&p| self.contains(p))
            || self.0.iter().any(|&p| r.contains(p))
            || edges(self.0).any(|a| edges(boxed).any(|b| cross(a, b)))
    }
}

/// Do the two segments cross at a single point inside both? Strict: the
/// products of the sides must change sign, so segments that meet at an end or
/// overlap along a line do not cross.
fn cross((p, q): (Point, Point), (r, s): (Point, Point)) -> bool {
    let side = |a: Point, b: Point, c: Point| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let (d1, d2) = (side(r, s, p), side(r, s, q));
    let (d3, d4) = (side(p, q, r), side(p, q, s));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// A delimited region of the image: a box, or a quadrilateral for one seen on
/// the diagonal, whose box would take in what falls beside it. Only asked what
/// falls inside it; what is cropped around it is its bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Region {
    /// An axis-aligned box.
    Box(Rect),
    /// Convex, in any order around it.
    Quad(Quad),
}

impl Region {
    /// Does it contain the point? A box counts its edges as inside.
    pub fn contains(&self, p: Point) -> bool {
        match self {
            Region::Box(r) => r.contains(p),
            Region::Quad(q) => q.contains(p),
        }
    }

    /// Does `other` fall entirely inside it?
    pub fn contains_rect(&self, other: &Rect) -> bool {
        match self {
            Region::Box(r) => r.contains_rect(other),
            Region::Quad(q) => Quad::from_rect(*other).0.iter().all(|&p| q.contains(p)),
        }
    }

    /// The smallest box that contains it.
    pub fn bounds(&self) -> Rect {
        match self {
            Region::Box(r) => *r,
            Region::Quad(q) => q.bounds(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Quad {
        Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))
    }

    #[test]
    fn one_point_inside_and_one_outside() {
        assert!(square().contains(Point { x: 300.0, y: 250.0 }));
        assert!(!square().contains(Point { x: 50.0, y: 250.0 }));
        assert!(!square().contains(Point { x: 300.0, y: 450.0 }));
    }

    #[test]
    fn the_bounds_of_a_quadrilateral_in_perspective() {
        let q = Quad([
            Point { x: 150.0, y: 100.0 },
            Point { x: 450.0, y: 100.0 },
            Point { x: 500.0, y: 400.0 },
            Point { x: 100.0, y: 400.0 },
        ]);
        assert_eq!(q.bounds(), Rect::new(100.0, 100.0, 500.0, 400.0));
        assert!(!q.contains(Point { x: 110.0, y: 110.0 }));
    }

    #[test]
    fn overlap_and_containment() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(a.iou(&a), 1.0);
        assert_eq!(a.iou(&Rect::new(20.0, 20.0, 30.0, 30.0)), 0.0);
        assert!((a.iou(&Rect::new(5.0, 0.0, 15.0, 10.0)) - 1.0 / 3.0).abs() < 1e-6);
        assert!(a.contains_rect(&Rect::new(2.0, 2.0, 8.0, 8.0)));
        assert_eq!(a.expand(0.1), Rect::new(-1.0, -1.0, 11.0, 11.0));
    }

    /// A band on the diagonal, as a corner view sees it.
    fn band() -> Quad {
        Quad([
            Point { x: 300.0, y: 100.0 },
            Point { x: 400.0, y: 200.0 },
            Point { x: 350.0, y: 250.0 },
            Point { x: 250.0, y: 150.0 },
        ])
    }

    #[test]
    fn a_diagonal_region_leaves_out_what_its_box_takes_in() {
        let quad = Region::Quad(band());
        let boxed = Region::Box(band().bounds());
        let off_the_band = Point { x: 380.0, y: 120.0 };
        assert!(boxed.contains(off_the_band));
        assert!(!quad.contains(off_the_band));
        assert!(quad.contains(Point { x: 325.0, y: 175.0 }));
        assert!(quad.contains_rect(&Rect::new(315.0, 165.0, 335.0, 185.0)));
        assert!(!quad.contains_rect(&Rect::new(300.0, 120.0, 340.0, 185.0)));
        assert_eq!(quad.bounds(), Rect::new(250.0, 100.0, 400.0, 250.0));
    }

    #[test]
    fn a_box_touches_a_quadrilateral_by_a_corner_or_by_crossing_edges() {
        let q = band();
        assert!(q.touches(&Rect::new(320.0, 160.0, 330.0, 170.0)), "inside");
        assert!(
            q.touches(&Rect::new(200.0, 50.0, 450.0, 300.0)),
            "around it"
        );
        assert!(
            q.touches(&Rect::new(290.0, 0.0, 310.0, 400.0)),
            "across, no corner inside"
        );
        assert!(
            !q.touches(&Rect::new(360.0, 100.0, 400.0, 140.0)),
            "in its box, off the band"
        );
    }

    #[test]
    fn the_centroid_of_a_box() {
        assert_eq!(
            Rect::new(200.0, 150.0, 300.0, 350.0).centroid(),
            Point { x: 250.0, y: 250.0 }
        );
    }

    #[test]
    fn the_edges_of_a_box_count_as_inside() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        for p in [
            Point { x: 10.0, y: 20.0 },
            Point { x: 30.0, y: 40.0 },
            Point { x: 10.0, y: 30.0 },
            Point { x: 30.0, y: 30.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 40.0 },
        ] {
            assert!(r.contains(p), "on the edge: {p:?}");
        }
        for p in [
            Point { x: 9.9, y: 30.0 },
            Point { x: 30.1, y: 30.0 },
            Point { x: 20.0, y: 19.9 },
            Point { x: 20.0, y: 40.1 },
        ] {
            assert!(!r.contains(p), "just outside: {p:?}");
        }
    }

    #[test]
    fn an_inverted_box_measures_nothing() {
        let inverted = Rect::new(30.0, 40.0, 10.0, 20.0);
        assert_eq!(inverted.area(), 0.0);
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(inverted.iou(&a), 0.0);
        assert_eq!(a.iou(&inverted), 0.0);
        assert_eq!(inverted.iou(&Rect::new(40.0, 50.0, 60.0, 70.0)), 0.0);
    }

    #[test]
    fn a_nan_is_never_inside_and_measures_nothing() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(!r.contains(Point {
            x: f32::NAN,
            y: 30.0
        }));
        assert!(!Rect::new(f32::NAN, 20.0, 30.0, 40.0).contains(Point { x: 15.0, y: 30.0 }));
        let nan = Rect::new(10.0, 20.0, f32::NAN, 40.0);
        assert_eq!(nan.area(), 0.0);
        assert_eq!(nan.iou(&r), 0.0);
        assert_eq!(r.iou(&nan), 0.0);
        assert!(!band().touches(&nan));
    }

    #[test]
    fn a_box_that_grazes_the_quadrilateral_does_not_touch_it() {
        // The band's edge runs from (300, 100) to (400, 200); the box's corner
        // sits exactly on its middle, outside: a graze, not an overlap.
        let grazed = Rect::new(350.0, 120.0, 390.0, 150.0);
        assert!(!band().touches(&grazed));
        // Crossing is strict: segments that meet at an end do not cross.
        let along = (Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 });
        let meeting = (Point { x: 10.0, y: 0.0 }, Point { x: 10.0, y: 10.0 });
        let across = (Point { x: 5.0, y: -5.0 }, Point { x: 5.0, y: 5.0 });
        assert!(!cross(along, meeting));
        assert!(cross(along, across));
    }
}
