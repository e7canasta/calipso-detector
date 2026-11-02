//! Head 2 of the second level: posture by depth disparity.
//! Stateless: each pose gives its posture.

use crate::belief::SubjectBody;
use crate::body::point_is_seen;
use crate::body::{BedSite, BodyPoint, Joint};
use crate::estimation;
use crate::policy::EnrichPolicy;

/// What depth says about the posture, without provenance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Reading {
    pub state: PostureState,
    pub upper: Option<f32>,
    pub lower: Option<f32>,
    pub disparity: Option<f32>,
    pub groups: BodyGroups,
    pub disparities: Disparities,
}

const HEAD: [Joint; 5] = [
    Joint::Nose,
    Joint::LeftEye,
    Joint::RightEye,
    Joint::LeftEar,
    Joint::RightEar,
];
const SHOULDERS: [Joint; 2] = [Joint::LeftShoulder, Joint::RightShoulder];
const PELVIS: [Joint; 2] = [Joint::LeftHip, Joint::RightHip];

/// Upper (head and shoulders) against lower (the hips), with the median of
/// each group's visible measured keypoints. The disparity is relative to the
/// body's mean depth: above `tolerance`, lying; otherwise upright. Head,
/// shoulders and pelvis are also measured apart, to show what was decided
/// with, and upper against the bed's centre, the stand-in for the hips read in
/// the same look: the baseline, there even when the hips are not. With no
/// measured keypoint, nothing.
pub(super) fn classify(body: &SubjectBody, policy: &EnrichPolicy) -> Option<Reading> {
    let tolerance = policy.posture_tolerance;
    let seen: Vec<&BodyPoint> = body
        .pose
        .keypoints
        .iter()
        .filter(|p| point_is_seen(p, policy.min_keypoint_score))
        .collect();
    if !seen.iter().any(|p| p.depth.is_some()) {
        return None;
    }
    let of = |joints: &[Joint]| -> Vec<&BodyPoint> {
        seen.iter()
            .copied()
            .filter(|p| p.name.is_some_and(|n| joints.contains(&n)))
            .collect()
    };
    let depth = |group: &[&BodyPoint]| -> Option<f32> {
        estimation::median(&group.iter().map(|p| p.depth).collect::<Vec<_>>())
    };
    let group = |joints: &[Joint]| -> BodyGroup {
        let found = of(joints);
        BodyGroup {
            depth: depth(&found),
            seen: found.len() as u32,
            of: joints.len() as u32,
        }
    };
    // Head and shoulders together: lying, the head is the farthest, and it adds
    // lever to the torso. Measured on the sample1 video, the shoulders alone
    // separate worse (gap 0.011 against 0.087; R16, 1604).
    let upper = depth(&[of(&HEAD), of(&SHOULDERS)].concat());
    let groups = BodyGroups {
        head: group(&HEAD),
        shoulders: group(&SHOULDERS),
        // The pelvis alone: the feet hang off the bed or rise, and their depth
        // says nothing of the torso (R16, 1604).
        pelvis: group(&PELVIS),
    };
    let lower = groups.pelvis.depth;
    let against = |a: Option<f32>| relative_disparity(a, lower);
    let centre = body
        .bed_points
        .iter()
        .find(|p| p.name == BedSite::Centre)
        .and_then(|p| p.depth);
    let disparity = against(upper);
    let state = match disparity {
        None => PostureState::Unknown,
        Some(d) if d > tolerance => PostureState::Lying,
        Some(_) => PostureState::Upright,
    };
    Some(Reading {
        state,
        upper,
        lower,
        disparity,
        groups,
        disparities: Disparities {
            upper_pelvis: disparity,
            shoulders_pelvis: against(groups.shoulders.depth),
            head_pelvis: against(groups.head.depth),
            upper_bed_centre: relative_disparity(upper, centre),
        },
    })
}

/// How much farther `a` is than `b`, relative to their mean: `(a − b) / mean`.
/// Without scale, because the depth model's is relative and changes from frame
/// to frame. Positive, `a` is farther from the camera. With either missing, or
/// not finite, none.
pub(super) fn relative_disparity(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    a.zip(b)
        .map(|(a, b)| (a - b) / ((a + b) / 2.0))
        .filter(|d| d.is_finite())
}

pub use scene_contract::Posture;
pub use scene_contract::{BodyGroup, BodyGroups, Disparities, PostureState};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::HeadPlace;

    fn body(upper: Option<f32>, lower: Option<f32>) -> SubjectBody {
        let keypoints = (0..17)
            .map(|i| BodyPoint {
                name: crate::body::coco_joint(i),
                x: 0.0,
                y: 0.0,
                score: 0.9,
                depth: match i {
                    0..=6 => upper,
                    11 | 12 | 15 | 16 => lower,
                    _ => None,
                },
            })
            .collect();
        SubjectBody {
            pose: crate::body::Pose {
                keypoints,
                provenance: crate::provenance::Provenance::Current,
            },
            depth: None,
            bed_points: Vec::new(),
            head_place: HeadPlace::UNKNOWN,
        }
    }

    const POLICY: EnrichPolicy = EnrichPolicy {
        move_threshold: 0.1,
        depth: true,
        min_keypoint_score: 0.3,
        posture_tolerance: 0.05,
        max_pose_age_secs: None,
        crop_pad: 0.0,
    };

    #[test]
    fn the_relative_disparity_is_signed_and_without_scale() {
        let d = |a, b| relative_disparity(Some(a), Some(b)).unwrap();
        assert!(d(2.2, 2.0) > 0.0 && d(1.8, 2.0) < 0.0);
        assert!((d(2.2, 2.0) - d(22.0, 20.0)).abs() < 1e-6);
        assert_eq!(relative_disparity(None, Some(2.0)), None);
        assert_eq!(relative_disparity(Some(0.0), Some(0.0)), None);
    }

    #[test]
    fn lying_only_above_the_tolerance() {
        let state = |a, b| classify(&body(Some(a), Some(b)), &POLICY).map(|r| r.state);
        assert_eq!(state(2.2, 2.0), Some(PostureState::Lying));
        assert_eq!(state(1.8, 2.0), Some(PostureState::Upright));
        assert_eq!(state(2.02, 2.0), Some(PostureState::Upright));
    }

    #[test]
    fn the_disparity_does_not_depend_on_scale() {
        let d =
            |a: f32, b: f32| classify(&body(Some(a), Some(b)), &POLICY).and_then(|r| r.disparity);
        let (x, y) = (d(2.2, 2.0).unwrap(), d(22.0, 20.0).unwrap());
        assert!((x - y).abs() < 1e-6);
    }

    #[test]
    fn an_unmeasured_group_is_unknown_and_no_measure_gives_no_reading() {
        let r = classify(&body(Some(2.0), None), &POLICY).unwrap();
        assert_eq!(
            (r.state, r.lower, r.disparity),
            (PostureState::Unknown, None, None)
        );
        assert_eq!(classify(&body(None, None), &POLICY), None);
    }

    #[test]
    fn nights_recorded_with_sitting_or_standing_read_as_upright() {
        for old in ["\"sitting\"", "\"standing\""] {
            let state: PostureState = serde_json::from_str(old).unwrap();
            assert_eq!(state, PostureState::Upright);
        }
        assert_eq!(
            serde_json::to_string(&PostureState::Upright).unwrap(),
            "\"upright\""
        );
    }
}
