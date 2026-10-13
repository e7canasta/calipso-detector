//! Where the resident's head is (R17, 1707): a fact of its own, as the
//! occupancy is where the body is. What depends on it reads only the fact,
//! never how it was concluded: the face or the pose's head inside the
//! headboard region and, away from it, its place in the bed's u/v (R18,
//! 1807): below the headboard inside the bed, or past its sides.

use crate::body::{Joint, Keypoint};
use crate::evidence::FaceEvidence;
use crate::geom::{Point, Rect};
use crate::map::{BedArea, BedPlane, Geometry, Map};
use crate::policy::Policy;
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
    /// Seen in bed, below the headboard: lying here is not lying as a bed is
    /// used (R18, 1807).
    BelowHeadboard,
    /// Seen past the bed's sides (`u < 0` or `u > 1`): on the way out.
    AtSide,
    /// Seen, but neither at the headboard, in bed nor past its sides; or seen
    /// without the bed's plane.
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

/// Where a head can be: the headboard region and, if known, the bed's plane.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bounds {
    headboard: Rect,
    bed: Option<BedPlane>,
}

/// Where the head is in a turn with a pose, with what it saw (the faces and
/// the pose) against the site's headboard and bed (R17, R18). A fact of the
/// turn, with or without depth: the depth look and layer 1 read it.
pub(crate) fn of_turn(
    faces: &FaceEvidence,
    pose: Option<&[Keypoint]>,
    map: &Map,
    geometry: Geometry,
    policy: &Policy,
) -> HeadPlace {
    let min_score = policy
        .enrich
        .as_ref()
        .map_or(f32::INFINITY, |p| p.min_keypoint_score);
    place(faces, pose, Bounds::of_site(map, geometry), min_score)
}

impl Bounds {
    /// The site's: its headboard and its bed's plane, when the geometry
    /// applies. Without the headboard delimited, none.
    fn of_site(map: &Map, geometry: Geometry) -> Option<Self> {
        map.headboard_for(geometry).map(|headboard| Self {
            headboard,
            bed: map.bed_for(geometry).and_then(BedPlane::of),
        })
    }

    /// Where a point of the head falls: the headboard region first; away from
    /// it, in the bed's u/v, past its sides or inside it.
    fn of(&self, p: Point) -> HeadPlaceState {
        if self.headboard.contains(p) {
            return HeadPlaceState::AtHeadboard;
        }
        let Some(bed) = self.bed else {
            return HeadPlaceState::Elsewhere;
        };
        match bed.locate(p) {
            BedArea::Inside => HeadPlaceState::BelowHeadboard,
            BedArea::PastTheSides => HeadPlaceState::AtSide,
            BedArea::PastTheEnds => HeadPlaceState::Elsewhere,
        }
    }
}

/// Where the head is, from what this turn saw: a face accepted inside the
/// headboard says so first; otherwise the centroid of the pose's seen head
/// keypoints; otherwise the first face seen. Without the headboard delimited,
/// unknown: without knowing where the head is, the hips are not guessed.
pub(crate) fn place(
    faces: &FaceEvidence,
    pose: Option<&[Keypoint]>,
    bounds: Option<Bounds>,
    min_score: f32,
) -> HeadPlace {
    let Some(bounds) = bounds else {
        return HeadPlace::UNKNOWN;
    };
    let headboard = bounds.headboard;
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
        return HeadPlace::of(bounds.of(centroid), HeadEvidence::PoseHead);
    }
    match faces.first() {
        None => HeadPlace::UNKNOWN,
        Some(face) => HeadPlace::of(bounds.of(face.bbox.centroid()), HeadEvidence::Face),
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
        let bounds = Bounds {
            headboard: HEADBOARD,
            bed: None,
        };
        place(faces, pose, Some(bounds), 0.5)
    }

    /// With the test world's bed, (100,100)–(500,400), under the headboard.
    fn in_bed(faces: &FaceEvidence, pose: Option<&[Keypoint]>) -> HeadPlace {
        let bed = crate::geom::Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        let bounds = Bounds {
            headboard: HEADBOARD,
            bed: BedPlane::of(&bed),
        };
        place(faces, pose, Some(bounds), 0.5)
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
