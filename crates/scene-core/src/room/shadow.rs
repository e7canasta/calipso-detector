//! The geometry's shadow over each body (white book, volume VII, ch. 11): where
//! a body falls for a geometry that does not decide yet, next to what decides
//! today. Pure functions over the bed's planes; nothing here changes a count.
//!
//! - the **centroid** of the box on the mattress: what decides today;
//! - the **support**, the centre of the box's lower edge, where a standing body
//!   rests: measured on the bed's footprint on the floor, or on the mattress
//!   without one, and its distance to the bed in metres;
//! - the **share** of the box over the mattress.

use super::BodyZone;
use super::cascade::{Placed, Step, Verdict};
use super::three_planes::ThreePlanes;
use super::turn::Turn;
use crate::geom::{Point, Rect};
use crate::map::BedPlane;
use crate::policy::ShadowPolicy;
use crate::units::PositiveMetres;
use crate::zone::Zone;

pub(crate) use scene_contract::PlanePoint;
pub(crate) use scene_contract::{BodyShadow, ShadowPlane};
/// What the shadow measures bodies against: the mattress, the footprint if
/// delimited, the bed's size if known, and the policy. The planes are the
/// frame's (`ThreePlanes`), the same ones the room in two planes reads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Site {
    mattress: BedPlane,
    floor: Option<BedPlane>,
    size: Option<[PositiveMetres; 2]>,
    policy: ShadowPolicy,
}

impl Site {
    /// The site's shadow, if the profile asks for it and the bed's geometry
    /// applies to this frame (the planes exist).
    pub(crate) fn of(planes: &ThreePlanes, shadow: Option<&ShadowPolicy>) -> Option<Self> {
        let shadow = *shadow?;
        Some(Self {
            mattress: planes.mattress(),
            floor: planes.floor(),
            size: planes.size(),
            policy: shadow,
        })
    }

    /// Where a body's box falls for the shadow.
    pub(crate) fn body(&self, bbox: &Rect) -> BodyShadow {
        let centroid = bbox.centroid();
        let support = Point {
            x: centroid.x,
            y: bbox.y1,
        };
        let (plane, support_plane) = match self.floor {
            Some(floor) => (floor, ShadowPlane::Floor),
            None => (self.mattress, ShadowPlane::Mattress),
        };
        let support_at = point_on(&plane, support);
        let distance = self.size.map(|size| distance_m(support_at, size));
        let zone = distance.map(|d| {
            if d <= self.policy.support_margin_m.get() {
                Zone::Bed
            } else {
                Zone::Floor
            }
        });
        let near_bed = match (zone, distance) {
            (Some(Zone::Floor), Some(d)) => Some(d <= self.policy.near_m.get()),
            (Some(Zone::Floor), None) | (Some(Zone::Bed), _) | (None, _) => None,
        };
        BodyShadow {
            centroid: point_on(&self.mattress, centroid),
            support: support_at,
            support_plane,
            support_distance_m: distance,
            share: self.mattress.share(bbox),
            zone,
            near_bed,
            parts: None,
        }
    }
}

/// The parts of one body, as the room reads them (ADR 2026-11-03): the
/// cascade's zone, step and measures, and the turn. The projection puts them on
/// the wire; the room names no wire type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PartsRead {
    pub(crate) zone: BodyZone,
    pub(crate) step: Step,
    pub(crate) turn: Turn,
    pub(crate) pelvis: Option<f64>,
    pub(crate) ankle_u: Option<f64>,
    pub(crate) ankle_score: Option<f32>,
    /// The torso and the legs' side in the frame (F5.2), with `posture` on.
    pub(crate) torso: Option<super::Torso>,
    pub(crate) legs: Option<super::LegsOut>,
    pub(crate) head: Option<Placed>,
    pub(crate) hips: Option<Placed>,
    pub(crate) ankles: Option<Placed>,
}

/// The parts of a body, from its cascade's verdict and its turn.
pub(crate) fn parts_read(
    verdict: &Verdict,
    turn: Turn,
    frame: Option<super::FramePosture>,
) -> PartsRead {
    let m = verdict.measures;
    PartsRead {
        zone: verdict.zone,
        step: verdict.step,
        turn,
        pelvis: m.pelvis,
        ankle_u: m.foot_u,
        ankle_score: m.foot_score,
        torso: frame.and_then(|f| f.torso),
        legs: frame.and_then(|f| f.legs),
        head: m.head,
        hips: m.hips,
        ankles: m.ankles,
    }
}

/// A point and its `(u, v)` on a plane.
fn point_on(plane: &BedPlane, p: Point) -> PlanePoint {
    let (u, v) = plane.project(p);
    PlanePoint {
        x: p.x,
        y: p.y,
        u: u as f32,
        v: v as f32,
    }
}

/// How far a point in bed coordinates is from the bed, in metres: the room's
/// own measure, so the shadow and the room in two planes agree.
fn distance_m(p: PlanePoint, size: [PositiveMetres; 2]) -> f32 {
    crate::room::distance_m(p.u, p.v, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Quad;
    use crate::units::{Metres, PositiveMetres};

    /// The scenes' bed, 100..500 x 100..400, and its footprint 40 px lower.
    fn site(floor: bool) -> Site {
        let bed = Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        let footprint = Quad::from_rect(Rect::new(100.0, 140.0, 500.0, 440.0));
        Site {
            mattress: BedPlane::of(&bed).unwrap(),
            floor: floor.then(|| BedPlane::of(&footprint).unwrap()),
            size: crate::room::bed_sizes([0.9, 2.0]),
            policy: ShadowPolicy {
                near_m: PositiveMetres::new(0.6).expect("a constant distance"),
                support_margin_m: Metres::new(0.0).expect("a constant distance"),
            },
        }
    }

    #[test]
    fn standing_by_the_near_side_rests_on_the_floor_past_the_foot() {
        let shadow = site(true).body(&Rect::new(80.0, 200.0, 160.0, 470.0));
        assert!((shadow.centroid.u - 0.05).abs() < 1e-6, "{shadow:?}");
        assert!((shadow.support_distance_m.unwrap() - 0.2).abs() < 1e-4);
        assert_eq!(
            (shadow.zone, shadow.near_bed),
            (Some(Zone::Floor), Some(true))
        );
    }

    #[test]
    fn lying_in_bed_rests_on_the_footprint_and_is_all_over_the_mattress() {
        let shadow = site(true).body(&Rect::new(150.0, 150.0, 450.0, 380.0));
        assert_eq!(shadow.support_distance_m, Some(0.0));
        assert_eq!((shadow.zone, shadow.near_bed), (Some(Zone::Bed), None));
        assert_eq!(shadow.share, 1.0);
    }

    #[test]
    fn past_a_corner_both_distances_count() {
        let p = PlanePoint {
            x: 0.0,
            y: 0.0,
            u: -0.5,
            v: 1.15,
        };
        assert!(
            (distance_m(p, crate::room::bed_sizes([0.9, 2.0]).expect("a bed size"))
                - 0.45_f32.hypot(0.3))
            .abs()
                < 1e-6
        );
    }
}
