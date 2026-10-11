//! Geometry in whole-frame coordinates (pixels).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A point on the image plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// An axis-aligned rectangle: top-left and bottom-right corners.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn centroid(&self) -> Point {
        Point {
            x: (self.x0 + self.x1) / 2.0,
            y: (self.y0 + self.y1) / 2.0,
        }
    }

    pub fn area(&self) -> f32 {
        (self.x1 - self.x0).max(0.0) * (self.y1 - self.y0).max(0.0)
    }

    /// Does `other` fall entirely inside this box?
    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x0 >= self.x0 && other.y0 >= self.y0 && other.x1 <= self.x1 && other.y1 <= self.y1
    }

    /// Intersection over union: how much two boxes overlap (0 to 1).
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

#[cfg(test)]
mod tests {
    use super::*;

    fn bed() -> Quad {
        Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))
    }

    #[test]
    fn one_point_inside_and_one_outside() {
        assert!(bed().contains(Point { x: 300.0, y: 250.0 }));
        assert!(!bed().contains(Point { x: 50.0, y: 250.0 }));
        assert!(!bed().contains(Point { x: 300.0, y: 450.0 }));
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

    #[test]
    fn the_centroid_of_a_box() {
        assert_eq!(
            Rect::new(200.0, 150.0, 300.0, 350.0).centroid(),
            Point { x: 250.0, y: 250.0 }
        );
    }
}
