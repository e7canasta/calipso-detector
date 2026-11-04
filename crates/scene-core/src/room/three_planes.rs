//! The frame of three planes (white book, volume IX, ch. 02): the mattress M,
//! the footprint F on the floor and the headboard C. Each is a homography of
//! its quadrilateral, and this is the only place where the bed's planes are
//! built: `TwoPlanes` and the shadow take theirs from here. As pure functions
//! of the map they answer the questions the cascade asks: the vertical rule at
//! a point, the near side and the far one, the far strip, a foot's height in
//! mattress heights, and a point of the headboard as `(u, w)`. Nothing here
//! decides; the cascade (F2.2) and the layers ask.

use crate::geom::{Point, Quad, Rect};
use crate::map::{BedPlane, Geometry, Map};
use crate::units::PositiveMetres;

/// The planes of one camera. `u` crosses the bed, `v` goes from the foot (0)
/// to the headboard (1) on M and F, and `w` goes from the mattress (0) to the
/// top (1) on C. Only the mattress is required; the footprint, the bed's size
/// and the headboard are there when the map delimits them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ThreePlanes {
    mattress: BedPlane,
    floor: Option<BedPlane>,
    // Read by the geometry the cascade asks (F2.2): no production reader until F2.3.
    headboard: Option<BedPlane>,
    size: Option<[PositiveMetres; 2]>,
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
    /// Where the foot is, `(u, v)`, on the plane it rests on.
    uv: (f64, f64),
    rule: Rule,
}

impl Foot {
    /// The level the foot rests on: 1 on the mattress, 0 on the floor.
    pub(crate) fn level(&self) -> f64 {
        self.level
    }

    /// The foot's `(u, v)` on the plane it rests on.
    pub(crate) fn uv(&self) -> (f64, f64) {
        self.uv
    }

    /// The height of a point of the image above this foot, in mattress heights
    /// from its level.
    pub(crate) fn height(&self, p: Point) -> f64 {
        let up = self.rule.up();
        let d = [f64::from(p.x - self.img.x), f64::from(p.y - self.img.y)];
        self.level + (d[0] * up[0] + d[1] * up[1]) / self.rule.length
    }
}

impl ThreePlanes {
    /// The planes of this map for this geometry: none unless the geometry
    /// applies to the frame (`Map::bed_for`), and none for degenerate corners
    /// of the bed. The footprint, the size and the headboard are optional.
    pub(crate) fn of(map: &Map, geometry: Geometry) -> Option<Self> {
        Self::new(
            map.bed_for(geometry)?,
            map.site.bed_floor.as_ref(),
            map.site.headboard_area.as_ref(),
            map.site.bed_size_m.and_then(crate::room::bed_sizes),
        )
    }

    /// From the bed and, if given, its footprint and its headboard's four
    /// corners, in `Quad`'s order. A degenerate optional plane is absent.
    /// `None` only for a degenerate bed.
    pub(crate) fn new(
        bed: &Quad,
        floor: Option<&Quad>,
        headboard: Option<&Quad>,
        size: Option<[PositiveMetres; 2]>,
    ) -> Option<Self> {
        Some(Self {
            mattress: BedPlane::of(bed)?,
            floor: floor.and_then(BedPlane::of),
            headboard: headboard.and_then(BedPlane::of),
            size,
        })
    }

    /// The mattress, M.
    pub(crate) fn mattress(&self) -> BedPlane {
        self.mattress
    }

    /// The footprint on the floor, F, if the map delimits it.
    pub(crate) fn floor(&self) -> Option<BedPlane> {
        self.floor
    }

    /// The bed's size in metres (width, length), if the map has it.
    pub(crate) fn size(&self) -> Option<[PositiveMetres; 2]> {
        self.size
    }
}

/// The questions of the frame the cascade asks (F2.2); the layers and the
/// shadow read the planes above. No production reader until F2.3 connects it.
impl ThreePlanes {
    /// The share of a box over the mattress (white book, volume IX, ch. 05).
    pub(crate) fn share(&self, r: &Rect) -> f32 {
        self.mattress.share(r)
    }

    /// The point of the image on the mattress, as `(u, v)`.
    pub(crate) fn read_mattress(&self, p: Point) -> (f64, f64) {
        self.mattress.project(p)
    }

    /// The point of the image on the footprint, as `(u, v)`.
    pub(crate) fn read_floor(&self, p: Point) -> Option<(f64, f64)> {
        self.floor.map(|f| f.project(p))
    }

    /// The point of the image on the headboard, as `(u, w)`. `None` without a
    /// headboard drawn as four points.
    pub(crate) fn read_headboard(&self, p: Point) -> Option<(f64, f64)> {
        self.headboard.map(|c| c.project(p))
    }

    /// The vertical rule at `(u, v)`: the mattress point above the footprint
    /// point, both in the image. `None` without the footprint.
    pub(crate) fn vertical_rule(&self, u: f64, v: f64) -> Option<Rule> {
        let floor = self.floor?.point(u, v);
        let mattress = self.mattress.point(u, v);
        let vector = [
            f64::from(mattress.x - floor.x),
            f64::from(mattress.y - floor.y),
        ];
        Some(Rule {
            vector,
            length: vector[0].hypot(vector[1]),
        })
    }

    /// The near side and the far one: the long side (`u = 0` or `u = 1`) lower
    /// along the vertical rule, the nearer to the camera (cap. 02).
    pub(crate) fn sides(&self) -> Option<Sides> {
        let up = self.vertical_rule(0.5, 0.5)?.up();
        let along = |u: f64| {
            let p = self.mattress.point(u, 0.5);
            f64::from(p.x) * up[0] + f64::from(p.y) * up[1]
        };
        let near = if along(0.0) < along(1.0) { 0.0 } else { 1.0 };
        Some(Sides {
            near,
            far: 1.0 - near,
        })
    }

    /// Is the point in the far strip, `width_m` metres across the bed from the
    /// far side, read on the footprint? Behind the far edge counts, as E5 does:
    /// a foot there is hidden by the mattress. `None` without the footprint or
    /// the bed's size.
    pub(crate) fn in_far_strip(&self, p: Point, width_m: f64) -> Option<bool> {
        let (u, _) = self.read_floor(p)?;
        let width = f64::from(self.size?[0].get());
        Some((u - self.sides()?.far).abs() <= width_m / width)
    }

    /// A foot in the image: on the mattress if it lies inside the bed, on the
    /// floor otherwise. Its heights are counted from that level (E1). `None`
    /// without the footprint.
    pub(crate) fn foot(&self, img: Point) -> Option<Foot> {
        let on_bed = self.mattress.inside(img);
        let (u, v) = if on_bed {
            self.mattress.project(img)
        } else {
            self.floor?.project(img)
        };
        Some(Foot {
            img,
            level: if on_bed { 1.0 } else { 0.0 },
            uv: (u, v),
            rule: self.vertical_rule(u, v)?,
        })
    }
}

/// How far a box's support (the centre of its lower edge) is from the bed's
/// footprint, in metres: the room's own measure, shared by the room in two
/// planes (P2) and the cascade's «near the bed».
#[allow(
    clippy::cast_possible_truncation,
    reason = "`distance_m` measures in f32 metres: the u/v of the plane stay f64 until the measure"
)]
pub(crate) fn support_m(floor: &BedPlane, size: [PositiveMetres; 2], bbox: &Rect) -> f32 {
    let support = Point {
        x: bbox.centroid().x,
        y: bbox.y1,
    };
    let (u, v) = floor.project(support);
    super::distance_m(u as f32, v as f32, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::SiteMap;
    use crate::room::fixtures::{bed_quad, close, dig1, floor_quad, headboard_quad, p, size};

    /// Tolerances declared against the prototype `herramientas/partes.py`,
    /// which rounds to four decimals and runs its homographies in float32.
    const PIXELS: f64 = 0.05;
    const UNITS: f64 = 1e-3;
    const HEIGHTS: f64 = 0.01;

    #[test]
    fn the_far_side_of_dig1_is_u_one() {
        let s = dig1().sides().expect("the footprint");
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
        let rule = planes.vertical_rule(0.5, 0.5).expect("the footprint");
        assert!(close(rule.vector[0], 9.5812, PIXELS), "{:?}", rule.vector);
        assert!(close(rule.vector[1], -97.4212, PIXELS), "{:?}", rule.vector);
        assert!(close(rule.length, 97.8912, PIXELS), "{}", rule.length);
        let far = planes.vertical_rule(0.2, 0.8).expect("the footprint");
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
        let (u, v) = planes.read_floor(p(1300.0, 700.0)).expect("the footprint");
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
        let foot = dig1().foot(p(1459.7, 641.3)).expect("the footprint");
        assert_eq!(foot.level(), 1.0);
        assert!(close(foot.height(p(1434.95, 460.8)), 3.3201, HEIGHTS));
        assert!(close(foot.height(p(1456.6, 316.2)), 5.2132, HEIGHTS));
        assert!(close(foot.height(p(1483.45, 315.02)), 5.2508, HEIGHTS));
        // The foot itself is at its own level.
        assert!(close(foot.height(p(1459.7, 641.3)), 1.0, HEIGHTS));
    }

    #[test]
    fn a_foot_off_the_bed_stands_on_the_floor() {
        // Dig1 at 6 s: the ankles lie on the mattress (the prototype's on_bed).
        assert_eq!(
            dig1()
                .foot(p(1362.9, 906.2))
                .expect("the footprint")
                .level(),
            1.0
        );
        // A point beside the bed, on the floor: its heights start there.
        assert_eq!(
            dig1().foot(p(700.0, 900.0)).expect("the footprint").level(),
            0.0
        );
    }

    #[test]
    fn the_far_strip_is_read_on_the_footprint_and_is_two_sided() {
        let planes = dig1();
        let strip = |q: Point| planes.in_far_strip(q, 0.12).expect("the footprint");
        // The ankles of dig1 at 113 s and 185 s: behind the far edge, in the
        // strip of 0.12 m. At 6 s and 76 s: out of it.
        assert!(strip(p(1459.7, 641.3)), "113 s");
        assert!(strip(p(1381.4, 601.5)), "185 s");
        assert!(!strip(p(1362.9, 906.2)), "6 s");
        assert!(!strip(p(1293.0, 629.6)), "76 s");
        // The near side is never in the far strip; the strip is 0.12 m wide.
        let floor = planes.floor().expect("the footprint");
        assert!(!strip(floor.point(0.5, 0.5)));
        assert!(!strip(floor.point(0.8, 0.5)));
        assert!(strip(floor.point(1.05, 0.5)));
    }

    #[test]
    fn the_planes_without_their_optional_parts_answer_what_they_can() {
        let planes = ThreePlanes::new(&bed_quad(), None, None, None).expect("the bed");
        assert!(planes.floor().is_none() && planes.size().is_none());
        assert_eq!(planes.read_headboard(p(1100.0, 380.0)), None);
        assert_eq!(planes.sides(), None);
        assert_eq!(planes.foot(p(1459.7, 641.3)), None);
        // The share is the mattress's, whatever the other planes say.
        let r = Rect::new(1100.0, 300.0, 1300.0, 600.0);
        assert_eq!(planes.share(&r), dig1().share(&r));
    }

    #[test]
    fn the_planes_of_a_map_need_the_geometry_and_the_bed() {
        let site = SiteMap::new(bed_quad());
        let valid = ThreePlanes::of(&Map::from(site.clone()), Geometry::Valid).expect("the bed");
        assert!(valid.floor().is_none() && valid.size().is_none());
        let mut full = site;
        full.bed_floor = Some(floor_quad());
        full.bed_size_m = Some([0.9, 2.0]);
        full.headboard_area = Some(headboard_quad());
        let map = Map::from(full);
        let planes = ThreePlanes::of(&map, Geometry::Valid).expect("the planes of dig1");
        let (u, v) = planes.read_floor(p(1300.0, 700.0)).expect("the footprint");
        assert!(
            close(u, 0.5300, UNITS) && close(v, 0.5128, UNITS),
            "{u} {v}"
        );
        assert!(planes.read_headboard(p(1100.0, 380.0)).is_some());
    }

    #[test]
    fn degenerate_corners_give_no_frame() {
        let flat = Quad([p(1.0, 1.0); 4]);
        assert!(ThreePlanes::new(&flat, Some(&floor_quad()), None, size()).is_none());
        // A degenerate footprint is absent, not a failure of the mattress.
        let planes = ThreePlanes::new(&bed_quad(), Some(&flat), None, size()).expect("the bed");
        assert!(planes.floor().is_none());
    }
}
