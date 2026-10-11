//! The bed plane: the homography that takes an image point to bed
//! coordinates, `u` across and `v` from the foot (0) to the headboard (1). It
//! is scene-analytics' convention (`spatial`), so the values are comparable.

use crate::geom::{Point, Quad};

/// The bed homography, from the image to the `(u, v)` plane: the nine
/// coefficients of `u = (a·x + b·y + c) / (g·x + h·y + i)` and
/// `v = (d·x + e·y + f) / (g·x + h·y + i)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedPlane([f64; 9]);

impl BedPlane {
    /// From the four corners in top-left, top-right, bottom-right, bottom-left
    /// order (`Quad`'s), which go to `(0,1) (1,1) (1,0) (0,0)`. Degenerate
    /// corners: none.
    ///
    /// The projection from the unit square to the bed is built (Heckbert,
    /// *Fundamentals of Texture Mapping*, 1989) and inverted by the adjugate.
    pub(crate) fn of(bed: &Quad) -> Option<Self> {
        let [top_left, top_right, bottom_right, bottom_left] =
            bed.0.map(|p| (f64::from(p.x), f64::from(p.y)));
        // From the square to the image: (0,0) bottom-left, (1,0) bottom-right,
        // (1,1) top-right, (0,1) top-left.
        let ((x0, y0), (x1, y1), (x2, y2), (x3, y3)) =
            (bottom_left, bottom_right, top_right, top_left);
        let (dx1, dy1) = (x1 - x2, y1 - y2);
        let (dx2, dy2) = (x3 - x2, y3 - y2);
        let (dx3, dy3) = (x0 - x1 + x2 - x3, y0 - y1 + y2 - y3);
        let den = dx1 * dy2 - dx2 * dy1;
        if den.abs() < 1e-10 {
            return None;
        }
        let g = (dx3 * dy2 - dx2 * dy3) / den;
        let h = (dx1 * dy3 - dx3 * dy1) / den;
        let (a, b, c) = (x1 - x0 + g * x1, x3 - x0 + h * x3, x0);
        let (d, e, f) = (y1 - y0 + g * y1, y3 - y0 + h * y3, y0);
        // The inverse, up to scale: the adjugate of [[a b c] [d e f] [g h 1]].
        let adj = [
            e - f * h,
            c * h - b,
            b * f - c * e,
            f * g - d,
            a - c * g,
            c * d - a * f,
            d * h - e * g,
            b * g - a * h,
            a * e - b * d,
        ];
        let [aa, _, _, dd, _, _, gg, _, _] = adj;
        let det = a * aa + b * dd + c * gg;
        (det.abs() >= 1e-10 && adj.iter().all(|v| v.is_finite())).then_some(Self(adj))
    }

    /// The point in bed coordinates.
    pub(crate) fn project(&self, p: Point) -> (f64, f64) {
        let [a, b, c, d, e, f, g, h, i] = self.0;
        let (x, y) = (f64::from(p.x), f64::from(p.y));
        let w = g * x + h * y + i;
        ((a * x + b * y + c) / w, (d * x + e * y + f) / w)
    }

    /// Does the point fall inside the bed, `[0, 1]²`, edges included? A rounding
    /// error is allowed at the edges.
    pub(crate) fn inside(&self, p: Point) -> bool {
        const TOL: f64 = 1e-9;
        let (u, v) = self.project(p);
        (-TOL..=1.0 + TOL).contains(&u) && (-TOL..=1.0 + TOL).contains(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;

    fn bed() -> BedPlane {
        BedPlane::of(&Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))).unwrap()
    }

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn the_corners_follow_the_scene_analytics_convention() {
        let p = bed();
        assert!(close(p.project(Point { x: 100.0, y: 100.0 }), (0.0, 1.0)));
        assert!(close(p.project(Point { x: 500.0, y: 400.0 }), (1.0, 0.0)));
        assert!(close(p.project(Point { x: 300.0, y: 250.0 }), (0.5, 0.5)));
    }

    #[test]
    fn a_trapezoid_in_perspective_too() {
        // The one from the scene-analytics scenarios (spatial).
        let q = Quad([
            Point { x: 0.0, y: 0.0 },
            Point { x: 100.0, y: 0.0 },
            Point { x: 110.0, y: 100.0 },
            Point { x: -10.0, y: 100.0 },
        ]);
        let p = BedPlane::of(&q).unwrap();
        assert!(close(p.project(Point { x: 110.0, y: 100.0 }), (1.0, 0.0)));
        assert!(p.inside(Point { x: 50.0, y: 50.0 }));
        assert!(!p.inside(Point { x: -20.0, y: 50.0 }));
    }

    #[test]
    fn degenerate_corners_give_no_plane() {
        let p = Point { x: 1.0, y: 1.0 };
        assert_eq!(BedPlane::of(&Quad([p; 4])), None);
    }
}
