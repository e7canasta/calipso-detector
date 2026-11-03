//! The turn of the pose (white book, volume IX, ch. 11): which direction is up
//! for the pose of a body, in the order the spec fixes. A pure function of the
//! frame's planes, this turn's boxes and faces, and the body's share over the
//! mattress. The shadow publishes which rule chose; nothing else reads it.
//!
//! 1. A face of the turn inside this body's box and in no other box: the arrow
//!    from the box's centre to the face.
//! 2. Otherwise, a body mostly over the mattress (its share at least the doubt's
//!    share, `TwoPlanesPolicy::share`): the bed's `v` axis, from the foot to
//!    the headboard.
//! 3. Otherwise, no turn: the standing body stays as it is.

use super::three_planes::ThreePlanes;
use crate::geom::{Point, Rect};

/// The direction that is up for the pose: from a point to another, in the image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Turn {
    /// The arrow from the box's centre to a face of the turn in it.
    FaceInBox { from: Point, to: Point },
    /// The bed's `v` axis, from the foot (`v = 0`) to the headboard (`v = 1`).
    BedAxis { from: Point, to: Point },
    /// No direction: the body stays as it is.
    Still,
}

/// The turn of body `body` among `boxes`, the boxes of this turn in the order
/// the sample gives them. `faces` are the turn's faces; `share` is this body's
/// share over the mattress, and `doubt_share` the share from which a box is
/// mostly over it.
pub(crate) fn choose(
    planes: &ThreePlanes,
    boxes: &[Rect],
    faces: &[Point],
    body: usize,
    share: f32,
    doubt_share: f32,
) -> Turn {
    let Some(&own) = boxes.get(body) else {
        return Turn::Still;
    };
    let from = own.centroid();
    let alone = |f: &Point| {
        own.contains(*f)
            && boxes
                .iter()
                .enumerate()
                .all(|(i, b)| i == body || !b.contains(*f))
    };
    if let Some(&to) = faces.iter().find(|f| alone(f)) {
        return Turn::FaceInBox { from, to };
    }
    if share >= doubt_share {
        let mattress = planes.mattress();
        return Turn::BedAxis {
            from: mattress.point(0.5, 0.0),
            to: mattress.point(0.5, 1.0),
        };
    }
    Turn::Still
}

/// The face of body `index` among `boxes`: the first of the turn's faces that
/// is inside its box and in no other (the turn's rule 1). Shared by the shadow
/// and the pose's choice, so both read the same face.
pub(crate) fn face_of(boxes: &[Rect], faces: &[Point], index: usize) -> Option<Point> {
    let own = *boxes.get(index)?;
    faces.iter().copied().find(|f| {
        own.contains(*f)
            && boxes
                .iter()
                .enumerate()
                .all(|(j, b)| j == index || !b.contains(*f))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Quad;

    fn quad(c: [[f32; 2]; 4]) -> Quad {
        Quad(c.map(|[x, y]| Point { x, y }))
    }

    /// The dig1 camera (`perfiles/dig1.toml`).
    fn dig1() -> ThreePlanes {
        let bed = quad([
            [940.0, 465.0],
            [1270.0, 425.0],
            [1635.0, 745.0],
            [1380.0, 1010.0],
        ]);
        let floor = quad([
            [950.0, 610.0],
            [1268.0, 485.0],
            [1627.0, 805.0],
            [1335.0, 1030.0],
        ]);
        let size = crate::room::bed_sizes([0.9, 2.0]);
        ThreePlanes::new(&bed, Some(&floor), None, size).expect("dig1's planes")
    }

    const DOUBT: f32 = 0.45;

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn a_face_alone_in_its_box_turns_the_body_to_it() {
        let boxes = [
            Rect::new(100.0, 100.0, 200.0, 300.0),
            Rect::new(400.0, 100.0, 500.0, 300.0),
        ];
        let face = p(150.0, 150.0);
        let turn = choose(&dig1(), &boxes, &[face], 0, 0.0, DOUBT);
        assert_eq!(
            turn,
            Turn::FaceInBox {
                from: boxes[0].centroid(),
                to: face
            }
        );
    }

    #[test]
    fn a_face_in_two_boxes_is_no_face_of_either() {
        // The two boxes overlap around the face: it is in both, so it does not decide.
        let boxes = [
            Rect::new(100.0, 100.0, 200.0, 300.0),
            Rect::new(150.0, 100.0, 250.0, 300.0),
        ];
        let shared = p(170.0, 150.0);
        let turn = choose(&dig1(), &boxes, &[shared], 0, 0.1, DOUBT);
        assert_eq!(turn, Turn::Still);
    }

    #[test]
    fn a_face_outside_its_box_is_no_face_of_it() {
        let boxes = [Rect::new(100.0, 100.0, 200.0, 300.0)];
        let turn = choose(&dig1(), &boxes, &[p(600.0, 150.0)], 0, 0.1, DOUBT);
        assert_eq!(turn, Turn::Still);
    }

    #[test]
    fn a_body_mostly_over_the_mattress_turns_along_the_bed_to_the_headboard() {
        let planes = dig1();
        let boxes = [Rect::new(1000.0, 250.0, 1200.0, 500.0)];
        let turn = choose(&planes, &boxes, &[], 0, 0.6, DOUBT);
        let Turn::BedAxis { from, to } = turn else {
            panic!("the bed's axis: {turn:?}");
        };
        let (u0, v0) = planes.read_mattress(from);
        let (u1, v1) = planes.read_mattress(to);
        assert!((u0 - 0.5).abs() < 1e-3 && v0.abs() < 1e-3, "{u0} {v0}");
        assert!(
            (u1 - 0.5).abs() < 1e-3 && (v1 - 1.0).abs() < 1e-3,
            "{u1} {v1}"
        );
    }

    #[test]
    fn the_share_decides_at_the_doubt_share_and_not_below() {
        let planes = dig1();
        let boxes = [Rect::new(1000.0, 250.0, 1200.0, 500.0)];
        assert!(matches!(
            choose(&planes, &boxes, &[], 0, DOUBT, DOUBT),
            Turn::BedAxis { .. }
        ));
        assert_eq!(choose(&planes, &boxes, &[], 0, 0.44, DOUBT), Turn::Still);
    }

    #[test]
    fn a_standing_body_without_a_face_stays_as_it_is() {
        let boxes = [Rect::new(0.0, 0.0, 60.0, 120.0)];
        let turn = choose(&dig1(), &boxes, &[], 0, 0.0, DOUBT);
        assert_eq!(turn, Turn::Still);
    }
}
