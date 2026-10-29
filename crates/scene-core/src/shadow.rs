//! The geometry's shadow over each body (white book, volume VII, ch. 11): where
//! a body falls for a geometry that does not decide yet, next to what decides
//! today. Pure functions over the bed's planes; nothing here changes a count.
//!
//! - the **centroid** of the box on the mattress: what decides today;
//! - the **support**, the centre of the box's lower edge, where a standing body
//!   rests: measured on the bed's footprint on the floor, or on the mattress
//!   without one, and its distance to the bed in metres;
//! - the **share** of the box over the mattress.

use crate::geom::{Point, Rect};
use crate::map::{BedPlane, Geometry, Map};
use crate::policy::{Policy, ShadowPolicy};
use crate::zone::Zone;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A point of the image and where it falls in the bed's coordinates on a
/// plane: `u` across, `v` from the foot (0) to the headboard (1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlanePoint {
    /// In image pixels.
    pub x: f32,
    /// In image pixels.
    pub y: f32,
    /// Across the bed: 0 and 1 are its sides.
    pub u: f32,
    /// Along the bed: 0 the foot, 1 the headboard.
    pub v: f32,
}

/// The plane a point was measured on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShadowPlane {
    /// The bed's footprint on the floor.
    Floor,
    /// The mattress, when there is no footprint.
    Mattress,
}

/// The shadow of one body. It decides nothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyShadow {
    /// The box's centre on the mattress: what decides the zone today.
    pub centroid: PlanePoint,
    /// The centre of the box's lower edge, where a standing body rests.
    pub support: PlanePoint,
    /// Where the support was measured.
    pub support_plane: ShadowPlane,
    /// How far the support is from the bed, in metres; zero inside. Absent
    /// without the bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_distance_m: Option<f32>,
    /// The share of the box over the mattress, on an 8 × 8 grid.
    pub share: f32,
    /// The zone the support says: in bed within the shadow's margin, on the
    /// floor past it. Absent without the bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<Zone>,
    /// On the floor for the shadow: whether the support is within the
    /// shadow's distance of the bed. Absent in bed or without the bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near_bed: Option<bool>,
}

/// What the shadow measures bodies against: the mattress, the footprint if
/// delimited, the bed's size if known, and the policy.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Site {
    mattress: BedPlane,
    floor: Option<BedPlane>,
    size: Option<[f32; 2]>,
    policy: ShadowPolicy,
}

impl Site {
    /// The site's shadow, if the profile asks for it and the bed's geometry
    /// applies to this frame.
    pub(crate) fn of(map: &Map, geometry: Geometry, policy: &Policy) -> Option<Self> {
        let shadow = policy.shadow?;
        let mattress = BedPlane::of(map.bed_for(geometry)?)?;
        Some(Self {
            mattress,
            floor: map.site.bed_floor.as_ref().and_then(BedPlane::of),
            size: map.site.bed_size_m,
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
            if d <= self.policy.support_margin_m {
                Zone::Bed
            } else {
                Zone::Floor
            }
        });
        let near_bed = match (zone, distance) {
            (Some(Zone::Floor), Some(d)) => Some(d <= self.policy.near_m),
            _ => None,
        };
        BodyShadow {
            centroid: point_on(&self.mattress, centroid),
            support: support_at,
            support_plane,
            support_distance_m: distance,
            share: self.mattress.share(bbox),
            zone,
            near_bed,
        }
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

/// How far a point in bed coordinates is from the bed, in metres: past a side
/// by `u`, past an end by `v`, both when past a corner. Zero inside.
fn distance_m(p: PlanePoint, [width, length]: [f32; 2]) -> f32 {
    let past = |c: f32| (-c).max(c - 1.0).max(0.0);
    (past(p.u) * width).hypot(past(p.v) * length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Quad;

    /// The scenes' bed, 100..500 x 100..400, and its footprint 40 px lower.
    fn site(floor: bool) -> Site {
        let bed = Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        let footprint = Quad::from_rect(Rect::new(100.0, 140.0, 500.0, 440.0));
        Site {
            mattress: BedPlane::of(&bed).unwrap(),
            floor: floor.then(|| BedPlane::of(&footprint).unwrap()),
            size: Some([0.9, 2.0]),
            policy: ShadowPolicy {
                near_m: 0.6,
                support_margin_m: 0.0,
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
        assert!((distance_m(p, [0.9, 2.0]) - 0.45_f32.hypot(0.3)).abs() < 1e-6);
    }
}
