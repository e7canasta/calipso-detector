//! Head 2 of the second level: posture by depth disparity.
//! Stateless: each pose gives its posture.

use super::head::HeadPlace;
use crate::belief::SubjectBody;
use crate::body::{BedPoint, BedSite, BodyPoint, Joint};
use crate::estimation;
use crate::policy::EnrichPolicy;
use crate::provenance::Provenance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
        .filter(|p| p.is_seen(policy.min_keypoint_score))
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

/// A group of the body's keypoints: the median depth of those seen and
/// measured, and how many of its keypoints were seen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyGroup {
    /// Median depth of its seen, measured keypoints; `None` if none.
    pub depth: Option<f32>,
    /// How many of its keypoints were seen (score at least the minimum).
    pub seen: u32,
    /// How many keypoints the group has.
    pub of: u32,
}

/// The groups the posture is measured with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyGroups {
    /// Nose, eyes and ears.
    pub head: BodyGroup,
    /// Both shoulders.
    pub shoulders: BodyGroup,
    /// Both hips: the reference.
    pub pelvis: BodyGroup,
}

/// Each disparity against the pelvis, `(a − pelvis) / mean`: relative,
/// without scale. `upper_pelvis` (head and shoulders together) decides.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Disparities {
    /// Head and shoulders against the pelvis: the one that decides.
    pub upper_pelvis: Option<f32>,
    /// The shoulders alone against the pelvis.
    pub shoulders_pelvis: Option<f32>,
    /// The head alone against the pelvis.
    pub head_pelvis: Option<f32>,
    /// Head and shoulders against the bed's centre: the stand-in for the
    /// hips, the baseline when the blanket hides them (R17). Absent in scenes
    /// recorded before it, and without the bed's points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_bed_centre: Option<f32>,
}

/// The posture by how head, torso and hips are ordered in depth.
/// Monocular depth separates lying from upright by the magnitude of the
/// disparity, not by its sign (measured on real nights): sitting and standing are not
/// distinguished and are published together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PostureState {
    /// The hips clearly closer to the camera than the head and shoulders.
    Lying,
    /// Sitting, getting up or standing. Also reads `sitting` and `standing`,
    /// the values published before 904-5, so nights recorded with them can be
    /// replayed; publishes only `upright`.
    #[serde(alias = "sitting", alias = "standing")]
    Upright,
    /// One of the two groups has no measured keypoints.
    Unknown,
}

/// The published posture, with its evidence on the depth model's (relative)
/// scale and the pose's provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Posture {
    /// Lying, upright or unknown.
    pub state: PostureState,
    /// Median of the measured head and shoulders.
    pub upper: Option<f32>,
    /// Median of the measured hips: the feet do not vote.
    pub lower: Option<f32>,
    /// `(upper − lower) / mean`: relative, without scale.
    pub disparity: Option<f32>,
    /// That of the pose it was concluded from.
    pub provenance: Provenance,
    /// Head, shoulders and pelvis apart. Absent in scenes recorded before R16.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<BodyGroups>,
    /// Each disparity against the pelvis; `upper_pelvis` is `disparity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disparities: Option<Disparities>,
    /// Above it, lying: the profile's `posture_tolerance`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f32>,
    /// The bed's points, read in the same depth map as the keypoints: covered,
    /// the bed's centre is the blanket over the hips (R17). Absent in scenes
    /// recorded before them, and without the bed's geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bed_points: Vec<BedPoint>,
    /// Where the head was when the bed's points were decided: they are read
    /// only with it at the headboard (1707). Absent in scenes recorded before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_place: Option<HeadPlace>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(upper: Option<f32>, lower: Option<f32>) -> SubjectBody {
        let keypoints = (0..17)
            .map(|i| BodyPoint {
                name: Joint::coco(i),
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
