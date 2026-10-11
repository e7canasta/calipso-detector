//! Head 2 of the second level: posture by depth disparity (904; book 2, ch.
//! 03). Stateless: each pose gives its posture.

use crate::body::{BodyPoint, Joint};
use crate::estimation;
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
}

/// Upper (head and shoulders) against lower (hips and ankles), with the median
/// of each group's visible measured keypoints. The disparity is relative to the
/// body's mean depth (904-2): above `tolerance`, lying; otherwise upright
/// (904-5: with this depth the sign does not separate sitting from standing).
/// With no measured keypoint, nothing.
pub(super) fn classify(points: &[BodyPoint], min_score: f32, tolerance: f32) -> Option<Reading> {
    let measured: Vec<&BodyPoint> = points
        .iter()
        .filter(|p| p.score >= min_score && p.depth.is_some())
        .collect();
    if measured.is_empty() {
        return None;
    }
    let group = |is: fn(Joint) -> bool| -> Option<f32> {
        let values: Vec<Option<f32>> = measured
            .iter()
            .filter(|p| p.name.is_some_and(is))
            .map(|p| p.depth)
            .collect();
        estimation::median(&values)
    };
    let upper = group(|j| {
        matches!(
            j,
            Joint::Nose
                | Joint::LeftEye
                | Joint::RightEye
                | Joint::LeftEar
                | Joint::RightEar
                | Joint::LeftShoulder
                | Joint::RightShoulder
        )
    });
    let lower = group(|j| {
        matches!(
            j,
            Joint::LeftHip | Joint::RightHip | Joint::LeftAnkle | Joint::RightAnkle
        )
    });
    let disparity = upper
        .zip(lower)
        .map(|(a, b)| (a - b) / ((a + b) / 2.0))
        .filter(|d| d.is_finite());
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
    })
}

/// The posture by how head, torso and hips are ordered in depth (904).
/// Monocular depth separates lying from upright by the magnitude of the
/// disparity, not by its sign (912's measurement): sitting and standing are not
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Posture {
    pub state: PostureState,
    /// Median of the measured head and shoulders.
    pub upper: Option<f32>,
    /// Median of the measured hips and ankles.
    pub lower: Option<f32>,
    /// `(upper − lower) / mean`: relative, without scale.
    pub disparity: Option<f32>,
    pub provenance: Provenance,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(upper: Option<f32>, lower: Option<f32>) -> Vec<BodyPoint> {
        (0..17)
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
            .collect()
    }

    #[test]
    fn lying_only_above_the_tolerance() {
        let state = |a, b| classify(&body(Some(a), Some(b)), 0.3, 0.05).map(|r| r.state);
        assert_eq!(state(2.2, 2.0), Some(PostureState::Lying));
        assert_eq!(state(1.8, 2.0), Some(PostureState::Upright));
        assert_eq!(state(2.02, 2.0), Some(PostureState::Upright));
    }

    #[test]
    fn the_disparity_does_not_depend_on_scale() {
        let d =
            |a: f32, b: f32| classify(&body(Some(a), Some(b)), 0.3, 0.05).and_then(|r| r.disparity);
        let (x, y) = (d(2.2, 2.0).unwrap(), d(22.0, 20.0).unwrap());
        assert!((x - y).abs() < 1e-6);
    }

    #[test]
    fn an_unmeasured_group_is_unknown_and_no_measure_gives_no_reading() {
        let r = classify(&body(Some(2.0), None), 0.3, 0.05).unwrap();
        assert_eq!(
            (r.state, r.lower, r.disparity),
            (PostureState::Unknown, None, None)
        );
        assert_eq!(classify(&body(None, None), 0.3, 0.05), None);
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
