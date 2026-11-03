//! The frame of three planes (white book, volume IX, ch. 02): the mattress M,
//! the footprint F on the floor and the headboard C. Each is a homography of
//! its quadrilateral, and the three answer, as pure functions of the map, the
//! questions the cascade asks: the vertical rule at a point, the near side
//! and the far one, the far strip, a foot's height in mattress heights, and a
//! point of the headboard as `(u, w)`. Nothing here decides; the cascade (F2.2)
//! asks.

use crate::geom::{Point, Quad};
use crate::map::{BedPlane, Map};
use crate::units::PositiveMetres;

/// The three planes of one camera. `u` crosses the bed, `v` goes from the foot
/// (0) to the headboard (1) on M and F, and `w` goes from the mattress (0) to
/// the top (1) on C.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ThreePlanes {
    mattress: BedPlane,
    floor: BedPlane,
    headboard: Option<BedPlane>,
    /// The bed's width in metres: the scale of the far strip.
    width_m: f64,
}

/// The vertical rule at a `(u, v)`: the image segment from the floor point to
/// the mattress point above it, one mattress height long in the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rule {
    /// From the floor to the mattress, in pixels.
    pub(crate) vector: [f64; 2],
    /// Its length in pixels: the mattress height at that place.
    pub(crate) length: f64,
}

impl Rule {
    /// The rule's direction, which is the room's «up».
    pub(crate) fn up(&self) -> [f64; 2] {
        [self.vector[0] / self.length, self.vector[1] / self.length]
    }
}

/// The long sides of the bed as `u`: the near one, the camera's, and the far one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sides {
    pub(crate) near: f64,
    pub(crate) far: f64,
}

/// A foot read in the image, the level it rests on, and the rule there: what
/// the heights of its body are counted from (E1, E2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Foot {
    img: Point,
    /// 1 on the mattress, 0 on the floor.
    level: f64,
    rule: Rule,
}

impl Foot {
    /// The height of a point of the image above this foot, in mattress heights
    /// from its level.
    pub(crate) fn height(&self, p: Point) -> f64 {
        let up = self.rule.up();
        let d = [f64::from(p.x - self.img.x), f64::from(p.y - self.img.y)];
        self.level + (d[0] * up[0] + d[1] * up[1]) / self.rule.length
    }
}

impl ThreePlanes {
    /// The planes of this map: the bed and its footprint, with the bed's width.
    /// `None` without the footprint or the bed's size, or with degenerate corners.
    pub(crate) fn of(map: &Map) -> Option<Self> {
        let [width, _] = crate::room::bed_sizes(map.site.bed_size_m?)?;
        Self::new(
            &map.site.bed,
            map.site.bed_floor.as_ref()?,
            map.site.headboard_area.as_ref(),
            width,
        )
    }

    /// From the bed, its footprint and, if the camera sees it, the headboard's
    /// four corners, in `Quad`'s order. `None` for degenerate corners.
    pub(crate) fn new(
        bed: &Quad,
        floor: &Quad,
        headboard: Option<&Quad>,
        width: PositiveMetres,
    ) -> Option<Self> {
        Some(Self {
            mattress: BedPlane::of(bed)?,
            floor: BedPlane::of(floor)?,
            headboard: headboard.and_then(BedPlane::of),
            width_m: f64::from(width.get()),
        })
    }

    /// The point of the image on the footprint, as `(u, v)`.
    pub(crate) fn read_floor(&self, p: Point) -> (f64, f64) {
        self.floor.project(p)
    }

    /// The point of the image on the mattress, as `(u, v)`.
    pub(crate) fn read_mattress(&self, p: Point) -> (f64, f64) {
        self.mattress.project(p)
    }

    /// The point of the image on the headboard, as `(u, w)`. `None` without a
    /// headboard drawn as four points.
    pub(crate) fn read_headboard(&self, p: Point) -> Option<(f64, f64)> {
        self.headboard.map(|c| c.project(p))
    }

    /// The vertical rule at `(u, v)`: the mattress point above the footprint
    /// point, both in the image.
    pub(crate) fn vertical_rule(&self, u: f64, v: f64) -> Rule {
        let floor = self.floor.point(u, v);
        let mattress = self.mattress.point(u, v);
        let vector = [
            f64::from(mattress.x - floor.x),
            f64::from(mattress.y - floor.y),
        ];
        Rule {
            vector,
            length: vector[0].hypot(vector[1]),
        }
    }

    /// The near side and the far one: the long side (`u = 0` or `u = 1`) lower
    /// along the vertical rule, the nearer to the camera (cap. 02).
    pub(crate) fn sides(&self) -> Sides {
        let up = self.vertical_rule(0.5, 0.5).up();
        let along = |u: f64| {
            let p = self.mattress.point(u, 0.5);
            f64::from(p.x) * up[0] + f64::from(p.y) * up[1]
        };
        let near = if along(0.0) < along(1.0) { 0.0 } else { 1.0 };
        Sides {
            near,
            far: 1.0 - near,
        }
    }

    /// Is the point in the far strip, `width_m` metres across the bed from the
    /// far side, read on the footprint? Behind the far edge counts, as E5 does:
    /// a foot there is hidden by the mattress.
    pub(crate) fn in_far_strip(&self, p: Point, width_m: f64) -> bool {
        let (u, _) = self.read_floor(p);
        (u - self.sides().far).abs() <= width_m / self.width_m
    }

    /// A foot in the image: on the mattress if it lies inside the bed, on the
    /// floor otherwise. Its heights are counted from that level (E1).
    pub(crate) fn foot(&self, img: Point) -> Foot {
        let on_bed = self.mattress.inside(img);
        let (u, v) = if on_bed {
            self.mattress.project(img)
        } else {
            self.floor.project(img)
        };
        Foot {
            img,
            level: if on_bed { 1.0 } else { 0.0 },
            rule: self.vertical_rule(u, v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::SiteMap;
    use crate::room::bed_sizes;

    /// Tolerances declared against the prototype `herramientas/partes.py`,
    /// which rounds to four decimals and runs its homographies in float32.
    const PIXELS: f64 = 0.05;
    const UNITS: f64 = 1e-3;
    const HEIGHTS: f64 = 0.01;

    fn quad(c: [[f32; 2]; 4]) -> Quad {
        Quad(c.map(|[x, y]| Point { x, y }))
    }

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    /// The dig1 camera (`workshop/43-ocupacion-por-partes/perfiles/dig1.toml`).
    fn dig1() -> ThreePlanes {
        let bed = quad([
            [940.0, 465.0],
            [1270.0, 425.0],
            [1635.0, 745.0],
            [1380.0, 1010.0],
        ]);
        let floor = quad([
            [950.0, 610.0],
            [1268.0, 485.0],
            [1627.0, 805.0],
            [1335.0, 1030.0],
        ]);
        let headboard = quad([
            [951.0, 365.0],
            [1281.0, 325.0],
            [1270.0, 425.0],
            [940.0, 465.0],
        ]);
        let width = bed_sizes([0.9, 2.0]).expect("a valid bed")[0];
        ThreePlanes::new(&bed, &floor, Some(&headboard), width).expect("dig1's planes")
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn the_far_side_of_dig1_is_u_one() {
        let s = dig1().sides();
        assert_eq!(
            s,
            Sides {
                near: 0.0,
                far: 1.0
            }
        );
    }

    #[test]
    fn the_vertical_rule_is_the_mattress_height_where_it_stands() {
        let planes = dig1();
        let rule = planes.vertical_rule(0.5, 0.5);
        assert!(close(rule.vector[0], 9.5812, PIXELS), "{:?}", rule.vector);
        assert!(close(rule.vector[1], -97.4212, PIXELS), "{:?}", rule.vector);
        assert!(close(rule.length, 97.8912, PIXELS), "{}", rule.length);
        let far = planes.vertical_rule(0.2, 0.8);
        assert!(close(far.length, 124.6244, PIXELS), "{}", far.length);
    }

    #[test]
    fn a_point_reads_on_the_footprint_and_on_the_mattress() {
        let planes = dig1();
        let (u, v) = planes.read_mattress(p(1300.0, 700.0));
        assert!(
            close(u, 0.3028, UNITS) && close(v, 0.3621, UNITS),
            "{u} {v}"
        );
        let (u, v) = planes.read_floor(p(1300.0, 700.0));
        assert!(
            close(u, 0.5300, UNITS) && close(v, 0.5128, UNITS),
            "{u} {v}"
        );
    }

    #[test]
    fn a_point_reads_on_the_headboard_as_u_and_w() {
        let (u, w) = dig1()
            .read_headboard(p(1483.45, 315.02))
            .expect("a headboard of four points");
        assert!(
            close(u, 1.6184, UNITS) && close(w, 0.8524, UNITS),
            "{u} {w}"
        );
        let (u, w) = dig1()
            .read_headboard(p(1100.0, 380.0))
            .expect("the headboard");
        assert!(
            close(u, 0.4627, UNITS) && close(w, 0.6649, UNITS),
            "{u} {w}"
        );
    }

    #[test]
    fn the_height_of_a_point_over_a_foot_in_mattress_heights() {
        // Pose of dig1 at 113 s: the ankle on the mattress (level 1), the hip,
        // the shoulders and the head above it.
        let foot = dig1().foot(p(1459.7, 641.3));
        assert_eq!(foot.level, 1.0);
        assert!(close(foot.height(p(1434.95, 460.8)), 3.3201, HEIGHTS));
        assert!(close(foot.height(p(1456.6, 316.2)), 5.2132, HEIGHTS));
        assert!(close(foot.height(p(1483.45, 315.02)), 5.2508, HEIGHTS));
        // The foot itself is at its own level.
        assert!(close(foot.height(p(1459.7, 641.3)), 1.0, HEIGHTS));
    }

    #[test]
    fn a_foot_off_the_bed_stands_on_the_floor() {
        // Dig1 at 6 s: the ankles lie on the mattress (the prototype's on_bed).
        assert_eq!(dig1().foot(p(1362.9, 906.2)).level, 1.0);
        // A point beside the bed, on the floor: its heights start there.
        assert_eq!(dig1().foot(p(700.0, 900.0)).level, 0.0);
    }

    #[test]
    fn the_far_strip_is_read_on_the_footprint_and_is_two_sided() {
        let planes = dig1();
        // The ankles of dig1 at 113 s and 185 s: behind the far edge, in the
        // strip of 0.12 m. At 6 s and 76 s: out of it.
        assert!(planes.in_far_strip(p(1459.7, 641.3), 0.12), "113 s");
        assert!(planes.in_far_strip(p(1381.4, 601.5), 0.12), "185 s");
        assert!(!planes.in_far_strip(p(1362.9, 906.2), 0.12), "6 s");
        assert!(!planes.in_far_strip(p(1293.0, 629.6), 0.12), "76 s");
        // The near side is never in the far strip; the strip is 0.12 m wide.
        assert!(!planes.in_far_strip(planes.floor.point(0.5, 0.5), 0.12));
        assert!(!planes.in_far_strip(planes.floor.point(0.8, 0.5), 0.12));
        assert!(planes.in_far_strip(planes.floor.point(1.05, 0.5), 0.12));
    }

    #[test]
    fn without_a_headboard_of_four_points_there_is_no_w() {
        let bed = quad([
            [940.0, 465.0],
            [1270.0, 425.0],
            [1635.0, 745.0],
            [1380.0, 1010.0],
        ]);
        let floor = quad([
            [950.0, 610.0],
            [1268.0, 485.0],
            [1627.0, 805.0],
            [1335.0, 1030.0],
        ]);
        let width = bed_sizes([0.9, 2.0]).expect("a valid bed")[0];
        let planes = ThreePlanes::new(&bed, &floor, None, width).expect("two planes");
        assert_eq!(planes.read_headboard(p(1100.0, 380.0)), None);
    }

    #[test]
    fn the_planes_of_a_map_need_its_footprint_and_the_bed_size() {
        let bed = quad([
            [940.0, 465.0],
            [1270.0, 425.0],
            [1635.0, 745.0],
            [1380.0, 1010.0],
        ]);
        let floor = quad([
            [950.0, 610.0],
            [1268.0, 485.0],
            [1627.0, 805.0],
            [1335.0, 1030.0],
        ]);
        let headboard = quad([
            [951.0, 365.0],
            [1281.0, 325.0],
            [1270.0, 425.0],
            [940.0, 465.0],
        ]);
        assert!(ThreePlanes::of(&Map::from(SiteMap::new(bed))).is_none());
        let mut site = SiteMap::new(bed);
        site.bed_floor = Some(floor);
        assert!(
            ThreePlanes::of(&Map::from(site.clone())).is_none(),
            "no bed size"
        );
        site.bed_size_m = Some([0.9, 2.0]);
        site.headboard_area = Some(headboard);
        let planes = ThreePlanes::of(&Map::from(site)).expect("the planes of dig1");
        let (u, v) = planes.read_floor(p(1300.0, 700.0));
        assert!(
            close(u, 0.5300, UNITS) && close(v, 0.5128, UNITS),
            "{u} {v}"
        );
        assert!(planes.read_headboard(p(1100.0, 380.0)).is_some());
    }

    #[test]
    fn degenerate_corners_give_no_frame() {
        let bed = quad([
            [940.0, 465.0],
            [1270.0, 425.0],
            [1635.0, 745.0],
            [1380.0, 1010.0],
        ]);
        let flat = Quad([p(1.0, 1.0); 4]);
        let width = bed_sizes([0.9, 2.0]).expect("a valid bed")[0];
        assert!(ThreePlanes::new(&bed, &flat, None, width).is_none());
        assert!(ThreePlanes::new(&flat, &bed, None, width).is_none());
    }
}
