//! How the turn concludes where the resident's head is (R17, 1707). The
//! fact, `body::HeadPlace`, is of its own, as the occupancy is where the body
//! is: what depends on it reads only the fact, never how it was concluded: the face or the pose's head inside the
//! headboard region and, away from it, its place in the bed's u/v (R18,
//! 1807): below the headboard inside the bed, or past its sides.

use crate::body::{HeadEvidence, HeadPlace, HeadPlaceState, Joint, Keypoint};
use crate::body::{coco_joint, is_seen, mean_seen};
use crate::evidence::FaceEvidence;
use crate::policy::EnrichPolicy;
use crate::room::{HeadArea, Room};

impl From<HeadArea> for HeadPlaceState {
    /// The room's answer, as the fact of the head.
    fn from(area: HeadArea) -> Self {
        match area {
            HeadArea::Headboard => Self::AtHeadboard,
            HeadArea::Bed => Self::BelowHeadboard,
            HeadArea::PastTheSides => Self::AtSide,
            HeadArea::Elsewhere => Self::Elsewhere,
        }
    }
}

const HEAD: [Joint; 5] = [
    Joint::Nose,
    Joint::LeftEye,
    Joint::RightEye,
    Joint::LeftEar,
    Joint::RightEar,
];

/// Where the head is in a turn with a pose, with what it saw (the faces and
/// the pose) against the room's headboard and bed (R17, R18). A fact of the
/// turn, with or without depth: the depth look and layer 1 read it.
pub(crate) fn of_turn(
    faces: &FaceEvidence,
    pose: Option<&[Keypoint]>,
    room: Option<&Room>,
    enrich: Option<&EnrichPolicy>,
) -> HeadPlace {
    let min_score = enrich.map_or(f32::INFINITY, |p| p.min_keypoint_score.get());
    place(faces, pose, room, min_score)
}

/// Where the head is, from what this turn saw: a face accepted inside the
/// headboard says so first; otherwise the centroid of the pose's seen head
/// keypoints; otherwise the first face seen. Without the headboard delimited,
/// unknown: without knowing where the head is, the hips are not guessed.
pub(crate) fn place(
    faces: &FaceEvidence,
    pose: Option<&[Keypoint]>,
    room: Option<&Room>,
    min_score: f32,
) -> HeadPlace {
    let Some(room) = room.filter(|room| room.has_headboard()) else {
        return HeadPlace::UNKNOWN;
    };
    let faces: &[_] = match faces {
        FaceEvidence::Seen(faces) => faces,
        FaceEvidence::NotLooked | FaceEvidence::Unavailable(_) | FaceEvidence::None => &[],
    };
    if faces.iter().any(|f| room.in_headboard(f.bbox.centroid())) {
        return HeadPlace::of(HeadPlaceState::AtHeadboard, HeadEvidence::Face);
    }
    let head = pose
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter(|&(i, k)| is_seen(k, min_score) && coco_joint(i).is_some_and(|j| HEAD.contains(&j)))
        .map(|(_, k)| (k.x, k.y, k.score));
    if let Some(centroid) = mean_seen(head, min_score) {
        return HeadPlace::of(room.head_area(centroid).into(), HeadEvidence::PoseHead);
    }
    match faces.first() {
        None => HeadPlace::UNKNOWN,
        Some(face) => HeadPlace::of(
            room.head_area(face.bbox.centroid()).into(),
            HeadEvidence::Face,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Quad, Rect, Region};
    use crate::look::{Detection, Label};
    use crate::policy::AdmissionPolicy;
    use crate::units::Confidence;
    use crate::units::{Extension, Proportion};

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

    fn room(bed: Quad) -> Room {
        Room::new(
            &bed,
            Some(Region::Box(HEADBOARD)),
            &AdmissionPolicy {
                min_confidence: Confidence::HALF,
                same_body_iou: Proportion::new(0.5).expect("a constant proportion"),
                near_bed_margin: Extension::new(0.1).expect("a constant extension"),
                bed_frame: None,
            },
        )
    }

    /// Against the headboard alone: a degenerate bed has no plane.
    fn by(faces: &FaceEvidence, pose: Option<&[Keypoint]>) -> HeadPlace {
        let point = crate::geom::Point { x: 0.0, y: 0.0 };
        place(faces, pose, Some(&room(Quad([point; 4]))), 0.5)
    }

    /// With the test world's bed, (100,100)–(500,400), under the headboard.
    fn in_bed(faces: &FaceEvidence, pose: Option<&[Keypoint]>) -> HeadPlace {
        let bed = Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        place(faces, pose, Some(&room(bed)), 0.5)
    }

    #[test]
    fn away_from_the_headboard_the_beds_plane_says_where() {
        let state = |x, y| in_bed(&FaceEvidence::NotLooked, Some(&head_at(x, y))).state;
        assert_eq!(state(300.0, 130.0), HeadPlaceState::AtHeadboard);
        assert_eq!(state(300.0, 300.0), HeadPlaceState::BelowHeadboard);
        assert_eq!(state(50.0, 300.0), HeadPlaceState::AtSide);
        assert_eq!(state(550.0, 130.0), HeadPlaceState::AtSide);
        assert_eq!(state(300.0, 10.0), HeadPlaceState::Elsewhere);
        assert_eq!(state(300.0, 450.0), HeadPlaceState::Elsewhere);
        assert_eq!(
            in_bed(&face(50.0, 300.0), None),
            HeadPlace::of(HeadPlaceState::AtSide, HeadEvidence::Face)
        );
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
