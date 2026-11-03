//! The camera that sees the bed on the diagonal: the mattress and the
//! footprint answer where a body is.

use super::three_planes::ThreePlanes;
use super::{BodyZone, Doubt, Posed};
use crate::body::is_seen;
use crate::body::{Joint, Keypoint};
use crate::geom::{Point, Rect};
use crate::map::BedPlane;
use crate::policy::TwoPlanesPolicy;
use crate::units::PositiveMetres;

/// The camera that sees the bed on the diagonal (volume VII, ch. 14): where a
/// body is by the share of its box over the mattress and by its support (the
/// centre of the box's lower edge) against the bed's footprint on the floor,
/// in metres. Every other question, frontal answers.
#[derive(Debug, Clone, Copy)]
pub(super) struct TwoPlanes {
    mattress: BedPlane,
    floor: BedPlane,
    size: [PositiveMetres; 2],
    policy: TwoPlanesPolicy,
}

impl TwoPlanes {
    /// With the mattress, the footprint and the bed's size; without any of
    /// them, or with degenerate corners, there are no two planes.
    pub(super) fn of(planes: &ThreePlanes, policy: TwoPlanesPolicy) -> Option<Self> {
        Some(Self {
            mattress: planes.mattress(),
            floor: planes.floor()?,
            size: planes.size()?,
            policy,
        })
    }

    /// P1, P2: in bed with enough of the box over the mattress and the
    /// support close to the footprint; on the floor otherwise, near the bed
    /// with the support within the policy's distance.
    pub(super) fn body_zone(&self, bbox: &Rect) -> BodyZone {
        let distance = self.support_m(bbox);
        if self.mattress.share(bbox) >= self.policy.share.get()
            && distance <= self.policy.support_m.get()
        {
            return BodyZone::Bed;
        }
        BodyZone::Floor {
            near_bed: distance <= self.policy.near_m.get(),
        }
    }

    /// Doubtful: with the knee in the profile, on the floor by its support
    /// alone, with enough of the box over the mattress.
    pub(super) fn doubtful(&self, bbox: &Rect) -> Option<Doubt> {
        self.policy.knee_bent_deg?;
        let share = self.mattress.share(bbox);
        (share >= self.policy.share.get() && self.support_m(bbox) > self.policy.support_m.get())
            .then_some(Doubt { share })
    }

    /// A doubtful body with its knee bent to the policy's angle or less rests
    /// its pelvis on the mattress: in bed. Straight, or without a knee in
    /// sight, it stays where its box puts it.
    pub(super) fn posed(&self, bbox: &Rect, pose: &[Keypoint], min_score: f32) -> Posed {
        let zone = self.body_zone(bbox);
        let knee_deg = knee_deg(pose, min_score);
        let bent = matches!(
            (self.policy.knee_bent_deg.map(|d| d.get()), knee_deg),
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

    /// How far a foot is out of the footprint, past any of its edges, in bed
    /// widths: negative inside. A foot sitting on the edge rests past the
    /// foot end as well as past the sides; a foot in bed, raised by the
    /// mattress, falls farther from the camera, inside the footprint.
    pub(super) fn foot_beyond(&self, p: Point) -> f64 {
        let (u, v) = self.floor.project(p);
        let [width, length] = self.size;
        let along = f64::from(length.get() / width.get());
        (-u).max(u - 1.0).max(-v * along).max((v - 1.0) * along)
    }

    /// How far the box's support is from the footprint, in metres.
    fn support_m(&self, bbox: &Rect) -> f32 {
        super::three_planes::support_m(&self.floor, self.size, bbox)
    }
}

/// The hip–knee–ankle angle of a pose in COCO order, in degrees: 180 is a
/// straight leg. Each joint is the midpoint of the sides seen; without a hip,
/// a knee or an ankle in sight, none.
fn knee_deg(pose: &[Keypoint], min_score: f32) -> Option<f32> {
    let joint = |sides: [Joint; 2]| {
        let seen: Vec<&Keypoint> = sides
            .iter()
            .filter_map(|&j| {
                let i = Joint::COCO.iter().position(|&c| c == j)?;
                pose.get(i).filter(|k| is_seen(k, min_score))
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
