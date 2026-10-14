//! What a sample saw of the subject's parts: the second level reduces the
//! turn to the parts' half of the row, `seen[t]`; layer 0's state is the
//! other, and `Belief` wires it. It reads, never writes the belief.
//!
//! Each part is a group of points, and two bits say where it is against a
//! region: some point inside, some point outside. Both: split. A part counts
//! only from a sense that looked in this sample (`-` otherwise): the box from
//! the main detector, the faces from theirs, the rest from a fresh pose.

use crate::belief::Subject;
use crate::belief::SubjectBody;
use crate::belief::parts::{ByPart, Look};
use crate::body::{BodyPoint, Joint};
use crate::evidence::FaceEvidence;
use crate::geom::{Point, Quad, Rect};
use crate::look::Detection;
use crate::map::BedPlane;
use crate::provenance::Provenance;

/// What the sample offers to see the parts with.
pub(crate) struct Sample<'a> {
    /// Did the main detector look? A failure looked at nothing.
    pub looked: bool,
    /// The subject, as layer 0 believes it: its box counts if seen now.
    pub subject: Option<&'a Subject>,
    /// The subject's pose: it counts if taken in this sample.
    pub body: Option<&'a SubjectBody>,
    /// The faces of the sample.
    pub faces: &'a FaceEvidence,
    /// The bed, if its geometry applies.
    pub bed: Option<&'a Quad>,
    /// The headboard, if delimited and its geometry applies.
    pub headboard: Option<Rect>,
    /// Below this score a pose point is not seen; without a pose policy, none.
    pub min_score: Option<f32>,
}

const HEAD: [Joint; 5] = [
    Joint::Nose,
    Joint::LeftEye,
    Joint::RightEye,
    Joint::LeftEar,
    Joint::RightEar,
];
const SHOULDERS: [Joint; 2] = [Joint::LeftShoulder, Joint::RightShoulder];
const ARMS: [Joint; 4] = [
    Joint::LeftElbow,
    Joint::RightElbow,
    Joint::LeftWrist,
    Joint::RightWrist,
];
const GROIN: [Joint; 2] = [Joint::LeftHip, Joint::RightHip];
const LEGS: [Joint; 4] = [
    Joint::LeftKnee,
    Joint::RightKnee,
    Joint::LeftAnkle,
    Joint::RightAnkle,
];

/// What the sample saw of each part. Without the bed's plane, nothing: there
/// is no place to be in.
pub(crate) fn seen(s: &Sample<'_>) -> Option<ByPart<Look>> {
    let bed = BedPlane::of(s.bed?)?;
    let in_bed = |p: Point| bed.inside(p);
    let in_headboard = |p: Point| s.headboard.is_some_and(|h| h.contains(p));
    let box_in_bed = |r: &Rect| corners(r).iter().all(|p| bed.inside(*p));

    // The person's box: seen now, or looked for and not seen.
    let person = s
        .subject
        .filter(|subject| subject.provenance == Provenance::Current)
        .map(|subject| subject.bbox);
    let centre = match (s.looked, person) {
        (false, _) => Look::NotLooked,
        (true, Some(b)) => Look::one_side(in_bed(b.centroid())),
        (true, None) => Look::UNSEEN,
    };
    let person_in_bed = person.map_or(Look::NotLooked, |b| Look::one_side(box_in_bed(&b)));

    // The faces: their centres, and their whole boxes.
    let faces: Option<&[Detection]> = match s.faces {
        FaceEvidence::Seen(faces) => Some(faces),
        FaceEvidence::None => Some(&[]),
        FaceEvidence::NotLooked | FaceEvidence::Unavailable(_) => None,
    };
    let centres = |inside: &dyn Fn(Point) -> bool| {
        faces.map_or(Look::NotLooked, |faces| {
            group(faces.iter().map(|f| f.bbox.centroid()), inside)
        })
    };
    let whole_face = |inside: &dyn Fn(&Rect) -> bool| match faces {
        Some(faces) if !faces.is_empty() => Look::one_side(faces.iter().any(|f| inside(&f.bbox))),
        _ => Look::NotLooked,
    };
    let face_bed = centres(&in_bed);
    let (face_headboard, face_in_headboard) = match s.headboard {
        Some(headboard) => (
            centres(&in_headboard),
            whole_face(&|r: &Rect| headboard.contains_rect(r)),
        ),
        None => (Look::NotLooked, Look::NotLooked),
    };

    // The pose: only a fresh one says where the parts are now.
    let points: Option<Vec<&BodyPoint>> = s
        .body
        .filter(|body| body.pose.provenance == Provenance::Current)
        .zip(s.min_score)
        .map(|(body, min)| {
            body.pose
                .keypoints
                .iter()
                .filter(|k| k.is_seen(min))
                .collect()
        });
    let part = |joints: &[Joint], inside: &dyn Fn(Point) -> bool| {
        points.as_ref().map_or(Look::NotLooked, |points| {
            group(
                points
                    .iter()
                    .filter(|k| k.name.is_some_and(|n| joints.contains(&n)))
                    .map(|k| Point { x: k.x, y: k.y }),
                inside,
            )
        })
    };
    let on_headboard = |joints: &[Joint]| match s.headboard {
        Some(_) => part(joints, &in_headboard),
        None => Look::NotLooked,
    };

    Some(ByPart {
        bed: [
            centre,
            face_bed,
            part(&HEAD, &in_bed),
            part(&SHOULDERS, &in_bed),
            part(&ARMS, &in_bed),
            part(&GROIN, &in_bed),
            part(&LEGS, &in_bed),
        ],
        headboard: [
            face_headboard,
            on_headboard(&HEAD),
            on_headboard(&SHOULDERS),
        ],
        whole: [
            person_in_bed,
            whole_face(&|r: &Rect| box_in_bed(r)),
            face_in_headboard,
        ],
    })
}

/// Where a group of points is: some inside, some outside. None: unseen.
fn group(points: impl Iterator<Item = Point>, inside: &dyn Fn(Point) -> bool) -> Look {
    points.fold(Look::UNSEEN, |look, p| match look {
        Look::Looked {
            inside: i,
            outside: o,
        } => {
            let now = inside(p);
            Look::Looked {
                inside: i || now,
                outside: o || !now,
            }
        }
        Look::NotLooked => look,
    })
}

fn corners(r: &Rect) -> [Point; 4] {
    [
        Point { x: r.x0, y: r.y0 },
        Point { x: r.x1, y: r.y0 },
        Point { x: r.x1, y: r.y1 },
        Point { x: r.x0, y: r.y1 },
    ]
}
