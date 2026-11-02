//! Head 1 of the second level: is the subject truly lying down?

use crate::body::point_is_seen;
use crate::body::{BodyPoint, Joint};
use crate::geom::Point;
use crate::room::Room;

/// Torso and hips inside the bed plane, and more than half of the visible
/// keypoints: lying. Visible feet confirm it if they are inside and deny it if
/// any is outside; with no feet in sight, it is assumed. Without a visible
/// torso or hips, it is unknown. The head is not required.
pub(super) fn classify(points: &[BodyPoint], room: &Room, min_score: f32) -> LyingState {
    let visible: Vec<&BodyPoint> = points
        .iter()
        .filter(|p| point_is_seen(p, min_score))
        .collect();
    let part = |joints: [Joint; 2]| -> Option<Point> {
        let found: Vec<&&BodyPoint> = visible
            .iter()
            .filter(|p| p.name.is_some_and(|n| joints.contains(&n)))
            .collect();
        (!found.is_empty()).then(|| {
            let n = found.len() as f32;
            Point {
                x: found.iter().map(|p| p.x).sum::<f32>() / n,
                y: found.iter().map(|p| p.y).sum::<f32>() / n,
            }
        })
    };
    let (Some(torso), Some(hips)) = (
        part([Joint::LeftShoulder, Joint::RightShoulder]),
        part([Joint::LeftHip, Joint::RightHip]),
    ) else {
        return LyingState::Unknown;
    };
    let is_inside = |p: &BodyPoint| room.in_bed(Point { x: p.x, y: p.y });
    let inside = visible.iter().filter(|p| is_inside(p)).count();
    if !room.in_bed(torso) || !room.in_bed(hips) || 2 * inside <= visible.len() {
        return LyingState::No;
    }
    let feet: Vec<&&BodyPoint> = visible
        .iter()
        .filter(|p| matches!(p.name, Some(Joint::LeftAnkle | Joint::RightAnkle)))
        .collect();
    match (feet.is_empty(), feet.iter().all(|p| is_inside(p))) {
        (true, _) => LyingState::Assumed,
        (false, true) => LyingState::Confirmed,
        (false, false) => LyingState::No,
    }
}

pub(crate) use scene_contract::{Lying, LyingState};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Quad, Rect};

    fn bed() -> Room {
        let bed = Quad::from_rect(Rect::new(0.0, 0.0, 100.0, 200.0));
        Room::new(&bed, None, &crate::policy::Policy::default().admission)
    }

    fn point(i: usize, x: f32, y: f32, score: f32) -> BodyPoint {
        BodyPoint {
            name: crate::body::coco_joint(i),
            x,
            y,
            score,
            depth: None,
        }
    }

    /// A body lying in the 100×200 bed, with its ankles at `feet`.
    fn lying_body(feet: Option<(f32, f32)>) -> Vec<BodyPoint> {
        (0..17)
            .map(|i| match (i, feet) {
                (15 | 16, None) => point(i, 50.0, 180.0, 0.1),
                (15 | 16, Some((x, y))) => point(i, x, y, 0.9),
                _ => point(i, 50.0, 20.0 + 9.0 * i as f32, 0.9),
            })
            .collect()
    }

    #[test]
    fn feet_inside_confirm_and_no_feet_assume() {
        let p = bed();
        assert_eq!(
            classify(&lying_body(Some((50.0, 190.0))), &p, 0.3),
            LyingState::Confirmed
        );
        assert_eq!(classify(&lying_body(None), &p, 0.3), LyingState::Assumed);
    }

    #[test]
    fn a_foot_outside_denies_it() {
        assert_eq!(
            classify(&lying_body(Some((150.0, 190.0))), &bed(), 0.3),
            LyingState::No
        );
    }

    #[test]
    fn without_a_visible_torso_it_is_unknown() {
        let mut body = lying_body(None);
        body[5].score = 0.1;
        body[6].score = 0.1;
        assert_eq!(classify(&body, &bed(), 0.3), LyingState::Unknown);
    }

    #[test]
    fn most_keypoints_outside_deny_it_even_with_torso_and_hips_inside() {
        let body: Vec<BodyPoint> = lying_body(None)
            .into_iter()
            .enumerate()
            .map(|(i, mut p)| {
                if !matches!(i, 5 | 6 | 11 | 12 | 15 | 16) {
                    p.x = 300.0;
                }
                p
            })
            .collect();
        assert_eq!(classify(&body, &bed(), 0.3), LyingState::No);
    }
}
