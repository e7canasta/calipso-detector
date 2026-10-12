//! Where the resident's head is (R17, 1707): a fact of its own, as the
//! occupancy is where the body is. What depends on it reads only the fact,
//! never how it was concluded: today the face or the pose's head inside the
//! headboard region; tomorrow, its last known place or its place in the bed's
//! u/v (1703), without the readers noticing.

use crate::body::{Joint, Keypoint};
use crate::evidence::FaceEvidence;
use crate::geom::{Point, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where the head is, and what said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HeadPlace {
    /// At the headboard, elsewhere, or unknown.
    pub state: HeadPlaceState,
    /// What it was concluded from; `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<HeadEvidence>,
}

/// Where the head is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeadPlaceState {
    /// At the headboard: lying in bed as a bed is used, the hips are at its
    /// centre.
    AtHeadboard,
    /// Seen, but not at the headboard.
    Elsewhere,
    /// Without the headboard delimited, or without a face or a head in sight.
    Unknown,
}

/// What the head's place was concluded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeadEvidence {
    /// An accepted face.
    Face,
    /// The pose's head keypoints (nose, eyes, ears) seen.
    PoseHead,
}

impl HeadPlace {
    /// Nothing to say where the head is.
    pub const UNKNOWN: HeadPlace = HeadPlace {
        state: HeadPlaceState::Unknown,
        by: None,
    };

    fn of(state: HeadPlaceState, by: HeadEvidence) -> Self {
        Self {
            state,
            by: Some(by),
        }
    }

    /// Is the head at the headboard?
    pub fn at_headboard(&self) -> bool {
        self.state == HeadPlaceState::AtHeadboard
    }
}

const HEAD: [Joint; 5] = [
    Joint::Nose,
    Joint::LeftEye,
    Joint::RightEye,
    Joint::LeftEar,
    Joint::RightEar,
];

/// Where the head is, from what this turn saw: a face accepted inside the
/// headboard says so first; otherwise the centroid of the pose's seen head
/// keypoints, inside or outside it; otherwise a face seen elsewhere. Without
/// the headboard delimited, unknown: without knowing where the head is, the
/// hips are not guessed.
pub(crate) fn place(
    faces: &FaceEvidence,
    pose: Option<&[Keypoint]>,
    headboard: Option<Rect>,
    min_score: f32,
) -> HeadPlace {
    let Some(headboard) = headboard else {
        return HeadPlace::UNKNOWN;
    };
    let faces: &[_] = match faces {
        FaceEvidence::Seen(faces) => faces,
        _ => &[],
    };
    if faces.iter().any(|f| headboard.contains(f.bbox.centroid())) {
        return HeadPlace::of(HeadPlaceState::AtHeadboard, HeadEvidence::Face);
    }
    let head: Vec<&Keypoint> = pose
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter(|&(i, k)| k.score >= min_score && Joint::coco(i).is_some_and(|j| HEAD.contains(&j)))
        .map(|(_, k)| k)
        .collect();
    if !head.is_empty() {
        let n = head.len() as f32;
        let centroid = Point {
            x: head.iter().map(|k| k.x).sum::<f32>() / n,
            y: head.iter().map(|k| k.y).sum::<f32>() / n,
        };
        let state = if headboard.contains(centroid) {
            HeadPlaceState::AtHeadboard
        } else {
            HeadPlaceState::Elsewhere
        };
        return HeadPlace::of(state, HeadEvidence::PoseHead);
    }
    if faces.is_empty() {
        HeadPlace::UNKNOWN
    } else {
        HeadPlace::of(HeadPlaceState::Elsewhere, HeadEvidence::Face)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{Detection, Label};

    const HEADBOARD: Rect = Rect {
        x0: 100.0,
        y0: 100.0,
        x1: 500.0,
        y1: 170.0,
    };

    fn face(x: f32, y: f32) -> FaceEvidence {
        FaceEvidence::Seen(vec![Detection {
            bbox: Rect::new(x - 10.0, y - 10.0, x + 10.0, y + 10.0),
            confidence: 0.9,
            label: Label::Other,
        }])
    }

    /// A pose with only its head seen, at (x, y).
    fn head_at(x: f32, y: f32) -> Vec<Keypoint> {
        (0..17)
            .map(|i| Keypoint {
                x,
                y,
                score: if i < 5 { 0.9 } else { 0.1 },
            })
            .collect()
    }

    fn by(faces: &FaceEvidence, pose: Option<&[Keypoint]>) -> HeadPlace {
        place(faces, pose, Some(HEADBOARD), 0.5)
    }

    #[test]
    fn a_face_at_the_headboard_says_so_first() {
        let pose = head_at(300.0, 300.0);
        let p = by(&face(300.0, 130.0), Some(&pose));
        assert_eq!(
            p,
            HeadPlace::of(HeadPlaceState::AtHeadboard, HeadEvidence::Face)
        );
    }

    #[test]
    fn otherwise_the_poses_head_inside_or_outside() {
        let inside = head_at(300.0, 130.0);
        let outside = head_at(300.0, 300.0);
        assert_eq!(
            by(&FaceEvidence::NotLooked, Some(&inside)),
            HeadPlace::of(HeadPlaceState::AtHeadboard, HeadEvidence::PoseHead)
        );
        assert_eq!(
            by(&face(300.0, 300.0), Some(&outside)),
            HeadPlace::of(HeadPlaceState::Elsewhere, HeadEvidence::PoseHead)
        );
    }

    #[test]
    fn a_face_elsewhere_without_a_head_says_elsewhere() {
        assert_eq!(
            by(&face(300.0, 300.0), None),
            HeadPlace::of(HeadPlaceState::Elsewhere, HeadEvidence::Face)
        );
        assert_eq!(by(&FaceEvidence::None, None), HeadPlace::UNKNOWN);
    }

    #[test]
    fn without_the_headboard_nothing_is_guessed() {
        let pose = head_at(300.0, 130.0);
        assert_eq!(
            place(&face(300.0, 130.0), Some(&pose), None, 0.5),
            HeadPlace::UNKNOWN
        );
    }
}
