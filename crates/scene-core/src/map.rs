//! The site map: the static world the subject does not occlude, and the bed's
//! own coordinates (`plane`): `u` across, `v` from the foot to the headboard.

mod plane;

pub(crate) use plane::{BedArea, BedPlane};

/// The image points of some bed coordinates (`u` across the bed, `v` from
/// the foot, 0, to the headboard, 1). `None` if the bed is degenerate. The
/// shell uses it to rectify the verifier's crop to the bed.
pub fn bed_points(bed: &crate::geom::Quad, uv: &[(f64, f64)]) -> Option<Vec<crate::geom::Point>> {
    let plane = BedPlane::of(bed)?;
    Some(uv.iter().map(|&(u, v)| plane.point(u, v)).collect())
}

/// The bed grown by `margin` in its own coordinates, as a quadrilateral in the
/// image, in `Quad`'s order: headboard left, headboard right, foot right, foot
/// left. `None` if the bed is degenerate.
pub(crate) fn grown_bed(bed: &crate::geom::Quad, margin: f64) -> Option<crate::geom::Quad> {
    grown(bed, margin, margin)
}

/// The bed grown by `across` on each side and by `along` at each end, in its
/// own coordinates (fractions of its width and length), in `Quad`'s order.
fn grown(bed: &crate::geom::Quad, across: f64, along: f64) -> Option<crate::geom::Quad> {
    let (u0, u1, v0, v1) = (-across, 1.0 + across, -along, 1.0 + along);
    let plane = BedPlane::of(bed)?;
    Some(crate::geom::Quad(
        [(u0, v1), (u1, v1), (u1, v0), (u0, v0)].map(|(u, v)| plane.point(u, v)),
    ))
}

use crate::body::BedSite;
use crate::body::bed_site_at;
use crate::geom::{Point, Quad, Rect, Zone};
use crate::policy::{AdmissionPolicy, FocusPolicy, ShadowPolicy};
use crate::time::SampleRef;

/// Internal fact: the projection adds the context of the published event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedCalibrated {
    pub at: SampleRef,
    pub reference_m: f32,
}

/// Initial site configuration: data, not behaviour. `Monitor` splits geometry
/// and memory on receiving it, and the loop's questions (does the geometry
/// apply? where is the bed?) are answered by the running map.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteMap {
    /// The bed on the image plane.
    pub bed: Quad,
    /// The resolution the bed was delimited for; `None`: it always applies.
    pub resolution: Option<FrameInfo>,
    /// Where the headboard is: around it, faces are looked for with no subject
    /// in sight.
    pub headboard: Option<Rect>,
    /// The headboard drawn as a quadrilateral, for a camera that sees it at an
    /// angle. It rules over `headboard`; faces are looked for, and the map
    /// published, with its bounds.
    pub headboard_area: Option<Quad>,
    /// Where the main detector looks while the resident is alone in bed;
    /// `None`: the bed grown by the focus policy's pad.
    pub focus: Option<Rect>,
    /// Where the bed stands on the floor, drawn like the bed: the shadow
    /// measures points on the floor against it. `None`: not delimited.
    pub bed_floor: Option<Quad>,
    /// The bed's width and length in metres. `None`: unknown.
    pub bed_size_m: Option<[f32; 2]>,
    /// The learned depth of the empty bed, in the depth model's relative scale.
    pub bed_reference: Option<f32>,
    /// The last calibration attempt, for the retry delay.
    pub calibration_attempt: Option<SampleRef>,
    pub(crate) calibrated_at: Option<SampleRef>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SiteGeometry {
    pub bed: Quad,
    pub resolution: Option<FrameInfo>,
    pub headboard: Option<Rect>,
    pub headboard_area: Option<Quad>,
    pub focus: Option<Rect>,
    pub bed_floor: Option<Quad>,
    pub bed_size_m: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedReference {
    pub depth: f32,
    pub since: Option<SampleRef>,
}

/// What was learned of the empty bed. It is not the map: the installer draws
/// the map, the loop learns this. When it was last tried is attention's
/// memory.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CalibrationMemory {
    reference: Option<BedReference>,
}

/// The running map: the geometry the installer drew. It does not change while
/// the loop runs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Map {
    pub site: SiteGeometry,
}

impl From<SiteMap> for Map {
    fn from(initial: SiteMap) -> Self {
        Self {
            site: SiteGeometry {
                bed: initial.bed,
                resolution: initial.resolution,
                headboard: initial.headboard,
                headboard_area: initial.headboard_area,
                focus: initial.focus,
                bed_floor: initial.bed_floor,
                bed_size_m: initial.bed_size_m,
            },
        }
    }
}

impl From<&SiteMap> for CalibrationMemory {
    fn from(initial: &SiteMap) -> Self {
        Self {
            reference: initial.bed_reference.map(|depth| BedReference {
                depth,
                since: initial.calibrated_at,
            }),
        }
    }
}

impl SiteMap {
    /// A site with only its bed: no resolution, headboard or calibration.
    pub fn new(bed: Quad) -> Self {
        Self {
            bed,
            resolution: None,
            headboard: None,
            headboard_area: None,
            focus: None,
            bed_floor: None,
            bed_size_m: None,
            bed_reference: None,
            calibration_attempt: None,
            calibrated_at: None,
        }
    }

    /// Where to measure the empty bed's depth: its corners and its centre. For
    /// measuring outside the loop; inside the loop, ask the running map.
    pub fn calibration_points(&self) -> Vec<Point> {
        calibration_points(&self.bed)
    }
}

impl CalibrationMemory {
    pub(crate) fn reference(&self) -> Option<BedReference> {
        self.reference
    }

    /// Learns the depth of the empty bed and says so with a domain event;
    /// without a valid measurement it learns nothing.
    pub(crate) fn learn(&mut self, at: SampleRef, depth: Option<f32>) -> Option<BedCalibrated> {
        let depth = depth?;
        self.reference = Some(BedReference {
            depth,
            since: Some(at),
        });
        Some(BedCalibrated {
            at,
            reference_m: depth,
        })
    }

    /// What is published of it.
    fn state(&self) -> Calibration {
        self.reference
            .map_or(Calibration::Unavailable, |reference| {
                Calibration::Complete {
                    reference_m: reference.depth,
                    since: reference.since,
                }
            })
    }
}

impl Map {
    pub(crate) fn calibration_points(&self) -> Vec<Point> {
        calibration_points(&self.site.bed)
    }

    /// Does the bed geometry apply to this frame? (F-09)
    pub(crate) fn gate(&self, frame: FrameInfo) -> Geometry {
        gate(self.site.resolution, frame)
    }

    /// The headboard, if the geometry applies.
    pub(crate) fn headboard_for(&self, geometry: Geometry) -> Option<Zone> {
        self.headboard().filter(|_| geometry == Geometry::Valid)
    }

    /// The headboard delimited: the quadrilateral if drawn, else the box.
    fn headboard(&self) -> Option<Zone> {
        self.site
            .headboard_area
            .map(Zone::Quad)
            .or(self.site.headboard.map(Zone::Box))
    }

    /// Where faces are looked for with nobody in sight, if the geometry
    /// applies: the headboard grown by `pad`. The search grows, the headboard
    /// does not: where the head is is still decided against it.
    pub(crate) fn faces_region(&self, geometry: Geometry, pad: f32) -> Option<Rect> {
        self.headboard_for(geometry)
            .map(|headboard| headboard.bounds().expand(pad))
    }

    /// Where the main detector looks while focusing, if the geometry applies:
    /// the delimited focus, or else the bed grown by `pad`. The look and the
    /// published map read it here: they cannot differ.
    pub(crate) fn focus_region(&self, geometry: Geometry, pad: f32) -> Option<Rect> {
        let bed = self.bed_for(geometry)?;
        Some(self.site.focus.unwrap_or_else(|| bed.bounds().expand(pad)))
    }

    /// Where depth is measured with a pad: the bed's box grown by it, centred
    /// on the bed. Without the geometry, nowhere.
    pub(crate) fn depth_region(&self, geometry: Geometry, pad: f32) -> Option<Rect> {
        self.bed_for(geometry).map(|bed| bed.bounds().expand(pad))
    }

    /// The geometry's shadow over the site, which decides nothing (white
    /// book, volume VII, ch. 11): near the bed in metres, on the footprint if
    /// it is delimited and on the mattress if not, next to near as decided
    /// today.
    fn shadow(&self, shadow: ShadowPolicy, near_bed_margin: f32) -> MapShadow {
        let site = &self.site;
        let near = site.bed_size_m.and_then(|[width, length]| {
            let plane = site.bed_floor.unwrap_or(site.bed);
            let metres = f64::from(shadow.near_m);
            grown(
                &plane,
                metres / f64::from(width),
                metres / f64::from(length),
            )
        });
        MapShadow {
            bed_floor: site.bed_floor,
            bed_size_m: site.bed_size_m,
            near,
            near_today: site.bed.bounds().expand(near_bed_margin),
            headboard_area: site.headboard_area,
        }
    }

    /// Where the bed's sites are in the image, if the geometry applies and the
    /// bed has a plane: the subject's depth look measures the bed there.
    pub(crate) fn bed_sites(
        &self,
        geometry: Geometry,
    ) -> Option<[(BedSite, Point); BedSite::ALL.len()]> {
        let plane = BedPlane::of(self.bed_for(geometry)?)?;
        Some(BedSite::ALL.map(|site| {
            let (u, v) = bed_site_at(site);
            (site, plane.point(u, v))
        }))
    }

    /// The bed, if the geometry applies.
    pub(crate) fn bed_for(&self, geometry: Geometry) -> Option<&Quad> {
        (geometry == Geometry::Valid).then_some(&self.site.bed)
    }

    /// The static world as this turn sees it: the configured geometry and
    /// what was learned of the empty bed.
    /// With the focus the policy configured, if any, and the shadow if asked.
    pub(crate) fn state(
        &self,
        geometry: Geometry,
        focus: Option<&FocusPolicy>,
        shadow: Option<&ShadowPolicy>,
        admission: &AdmissionPolicy,
        calibration: &CalibrationMemory,
    ) -> MapState {
        MapState {
            focus: focus.and_then(|f| self.focus_region(geometry, f.pad)),
            shadow: shadow.map(|shadow| self.shadow(*shadow, admission.near_bed_margin)),
            geometry,
            bed: self.site.bed,
            headboard: self.headboard().map(|headboard| headboard.bounds()),
            calibration: calibration.state(),
        }
    }
}

/// The corners and the centre: the same centre the subject's look reads
/// (`BedSite::Centre`, in perspective through the bed plane), so "the bed's
/// centre" means one thing. Degenerate corners have no plane: the centre of
/// their box.
fn calibration_points(bed: &Quad) -> Vec<Point> {
    let (u, v) = bed_site_at(BedSite::Centre);
    let centre = BedPlane::of(bed).map_or_else(|| bed.bounds().centroid(), |p| p.point(u, v));
    let mut points = bed.0.to_vec();
    points.push(centre);
    points
}

fn gate(resolution: Option<FrameInfo>, frame: FrameInfo) -> Geometry {
    match resolution {
        Some(declared) if declared != frame => Geometry::Invalid {
            declared,
            actual: frame,
        },
        _ => Geometry::Valid,
    }
}

pub use scene_contract::FrameInfo;
pub use scene_contract::Geometry;
pub use scene_contract::{Calibration, MapShadow, MapState};
#[cfg(test)]
mod tests {
    #[test]
    fn the_grown_bed_of_a_rectangle_is_the_grown_rectangle() {
        use crate::geom::{Quad, Rect};
        let bed = Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        let grown = super::grown_bed(&bed, 0.15).unwrap();
        let b = grown.bounds();
        let near = |a: f32, e: f32| (a - e).abs() < 0.01;
        assert!(near(b.x0, 40.0) && near(b.x1, 560.0), "{b:?}");
        assert!(near(b.y0, 55.0) && near(b.y1, 445.0), "{b:?}");
        // Quad's order, as the shell rectifies it: the headboard (top) first.
        assert!(
            near(grown.0[0].y, 55.0) && near(grown.0[0].x, 40.0),
            "{:?}",
            grown.0
        );
    }

    use super::*;

    #[test]
    fn the_calibration_centre_is_the_subjects_bed_centre() {
        // A bed in perspective (sample1's): the centre of its box is not the
        // centre of the bed.
        let bed = Quad([
            Point { x: 785.0, y: 355.0 },
            Point {
                x: 1160.0,
                y: 370.0,
            },
            Point {
                x: 1240.0,
                y: 1075.0,
            },
            Point {
                x: 705.0,
                y: 1075.0,
            },
        ]);
        let points = calibration_points(&bed);
        assert_eq!(&points[..4], &bed.0);
        let (u, v) = BedPlane::of(&bed).unwrap().project(points[4]);
        assert!((u - 0.5).abs() < 1e-4 && (v - 0.5).abs() < 1e-4, "{u}, {v}");
    }
}
