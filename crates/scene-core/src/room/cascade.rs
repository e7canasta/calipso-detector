//! The cascade by support (white book, volume IX, ch. 05): the zone of one body,
//! with the step that decided it. A pure function of the frame's planes, the
//! body's box, its pose if it was looked at, the turn's face, and the
//! cascade's thresholds. It remembers nothing (the memory is `belief`'s, P7)
//! and counts nothing; the shadow publishes its measures (F2.3).
//!
//! Steps, in order: the box (1a past the near edge, 1b the cut at the far
//! edge, 1c no touch), the face at the headboard (2), the feet (3: proven on
//! the mattress by (a) the far strip and (b) the score; on the floor by E2),
//! and, without a proof, floor (4). Each step that decides names itself, and
//! «the pose was not looked at» is a step of its own.

use super::BodyZone;
use super::three_planes::{ThreePlanes, support_m};
use crate::body::{Keypoint, is_seen};
use crate::geom::{Point, Rect};
use crate::policy::{PartsPolicy, TwoPlanesPolicy};
use crate::units::Confidence;

/// COCO-17 indices of the joints the cascade reads.
pub(super) const HIPS: [usize; 2] = [11, 12];
pub(super) const ANKLES: [usize; 2] = [15, 16];

/// The cascade's thresholds, as its caller gives them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascadeRules {
    /// The share of the box over the mattress: below it, a past-the-edge box
    /// (1a) or a cut one (1b, E5S) decides floor.
    pub(crate) share: f32,
    /// E5: a box whose foot is within this fraction of the far edge's length
    /// from the far edge, with no ankle seen, is cut.
    pub(crate) e5_tol: f64,
    /// (a): the far strip, in metres from the far side.
    pub(crate) strip_m: f64,
    /// (b): the mean score of the visible ankles that proves the feet.
    pub(crate) score_test: f32,
    /// E2: the band of the pelvis's height, in mattress heights.
    pub(crate) e2_band: [f64; 2],
    /// The score from which a keypoint counts as seen.
    pub(crate) kp_min: f32,
    /// «Near the bed» of a body on the floor: its support within this many
    /// metres of the footprint (P2, `TwoPlanesPolicy::near_m`). Not the box's
    /// touch of the bed.
    pub(crate) near_m: f32,
    /// E6: the head's `w` on the headboard band, from a little under the
    /// mattress (`-0.15`) to the top (`1`).
    pub(crate) headboard_w: [f64; 2],
}

impl CascadeRules {
    /// The cascade's thresholds from the space's rules: the parts' own, the
    /// two planes' share and near distance (the same concepts as the room's),
    /// and the pose's keypoint score. `None` without the parts, the planes or
    /// the pose: without a pose the cascade has nothing to read the feet with.
    pub(crate) fn of(parts: &PartsPolicy, two: &TwoPlanesPolicy, kp_min: Confidence) -> Self {
        Self {
            share: two.share.get(),
            e5_tol: f64::from(parts.cut_tolerance.get()),
            strip_m: f64::from(parts.far_strip_m.get()),
            score_test: parts.ankle_proof_score.get(),
            e2_band: [
                f64::from(parts.pelvis_band.lo().get()),
                f64::from(parts.pelvis_band.hi().get()),
            ],
            kp_min: kp_min.get(),
            near_m: two.near_m.get(),
            headboard_w: [
                f64::from(parts.headboard_w.lo().get()),
                f64::from(parts.headboard_w.hi().get()),
            ],
        }
    }
}

/// What the cascade reads of a body.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Body<'a> {
    /// The person's box, in pixels.
    pub(crate) bbox: Rect,
    /// The pose, in COCO order, if it was looked at this turn.
    pub(crate) pose: Option<&'a [Keypoint]>,
    /// The head of the turn, where its face was seen (E6). The caller brings
    /// it: the face sense (F2.3) or the pose's face.
    pub(crate) face: Option<Point>,
}

/// The step that decided the zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// 1a: past the near edge, with little of the box on the mattress.
    PastNear,
    /// 1b: the box cut at the far edge, with little of it on the mattress (E5, E5S).
    Cut,
    /// 1c: the box does not touch the bed.
    NoTouch,
    /// 2: the face at the headboard (E6).
    FaceAtHeadboard,
    /// 3a: the feet on the mattress, proven by (a) and (b).
    FeetOnMattress,
    /// 3b: the feet on the floor, the pelvis at the mattress height in the band (E2).
    PelvisAtEdge,
    /// 3c: the feet on the floor, the pelvis out of the band.
    Standing,
    /// 4: the pose was looked at and does not prove the zone.
    Unproven,
    /// The pose was not looked at, and the face did not decide.
    PoseNotLooked,
}

/// A point of the image and where it falls on the plane it was read on: `u`
/// across the bed, `v` along it (on the headboard, `v` is `w`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) u: f64,
    pub(crate) v: f64,
}

/// The measures the shadow publishes next to the decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Measures {
    /// The share of the box over the mattress.
    pub(crate) share: f32,
    /// The foot is past the bed's near side.
    pub(crate) past_near: bool,
    /// The box is cut at the far edge.
    pub(crate) cut: bool,
    /// The box touches the bed.
    pub(crate) touches: bool,
    /// At least one ankle was seen.
    pub(crate) foot_seen: bool,
    /// The level the foot rests on: 1 on the mattress, 0 on the floor.
    pub(crate) foot_level: f64,
    /// The seen ankles' `u` on the footprint, if any.
    pub(crate) foot_u: Option<f64>,
    /// The mean score of the seen ankles, if any.
    pub(crate) foot_score: Option<f32>,
    /// The pelvis's height over the foot, in mattress heights, if the hips were seen.
    pub(crate) pelvis: Option<f64>,
    /// The head's `(u, w)` on the headboard, if the face is given and the headboard drawn.
    pub(crate) head_uw: Option<(f64, f64)>,
    /// The head, on the headboard: `u` across, `v` the headboard's `w`; where the
    /// turn's face was seen.
    pub(crate) head: Option<Placed>,
    /// The hips, on the mattress, where they were seen.
    pub(crate) hips: Option<Placed>,
    /// The ankles, on the footprint, where they were seen.
    pub(crate) ankles: Option<Placed>,
    /// The box's support's distance from the footprint, in metres, if the bed has its size.
    pub(crate) support_m: Option<f32>,
}

/// The zone of a body, and the step that decided it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Verdict {
    pub(crate) zone: BodyZone,
    pub(crate) step: Step,
    pub(crate) measures: Measures,
}

/// The zone of a body by the cascade. `None` without the bed's footprint and
/// size, which the cascade needs to read the feet.
pub(crate) fn decide(planes: &ThreePlanes, body: &Body, rules: &CascadeRules) -> Option<Verdict> {
    let mattress = planes.mattress();
    let sides = planes.sides()?;
    let b = body.bbox;
    let box_foot = Point {
        x: b.centroid().x,
        y: b.y1,
    };
    let ankles = mean_seen(body.pose, &ANKLES, rules.kp_min);
    let foot = planes.foot(ankles.unwrap_or(box_foot))?;
    let hips = mean_seen(body.pose, &HIPS, rules.kp_min);

    let share = planes.share(&b);
    let past_near =
        (sides.near == 0.0 && foot.uv().0 < 0.0) || (sides.near == 1.0 && foot.uv().0 > 1.0);
    let far_a = mattress.point(sides.far, 0.0);
    let far_b = mattress.point(sides.far, 1.0);
    let (dist, edge_len) = seg_dist(box_foot, far_a, far_b);
    let cut = dist <= rules.e5_tol * edge_len && ankles.is_none();
    let touches = [
        b.centroid(),
        box_foot,
        Point { x: b.x0, y: b.y0 },
        Point { x: b.x1, y: b.y1 },
    ]
    .into_iter()
    .any(|p| mattress.inside(p));

    let measures = Measures {
        share,
        past_near,
        cut,
        touches,
        foot_seen: ankles.is_some(),
        foot_level: foot.level(),
        foot_u: ankles.and_then(|a| planes.read_floor(a)).map(|(u, _)| u),
        foot_score: mean_score(body.pose, &ANKLES, rules.kp_min),
        pelvis: hips.map(|h| foot.height(h)),
        head_uw: body.face.and_then(|f| planes.read_headboard(f)),
        head: body.face.and_then(|f| {
            let (u, w) = planes.read_headboard(f)?;
            Some(Placed {
                x: f.x,
                y: f.y,
                u,
                v: w,
            })
        }),
        hips: hips.map(|h| {
            let (u, v) = planes.read_mattress(h);
            Placed {
                x: h.x,
                y: h.y,
                u,
                v,
            }
        }),
        ankles: ankles.and_then(|a| {
            let (u, v) = planes.read_floor(a)?;
            Some(Placed {
                x: a.x,
                y: a.y,
                u,
                v,
            })
        }),
        support_m: planes
            .floor()
            .zip(planes.size())
            .map(|(floor, size)| support_m(&floor, size, &b)),
    };
    let near_bed = measures.support_m.is_some_and(|d| d <= rules.near_m);
    let decided = |zone: BodyZone, step: Step| Verdict {
        zone,
        step,
        measures,
    };
    let floor = |step: Step| decided(BodyZone::Floor { near_bed }, step);
    let bed = |step: Step| decided(BodyZone::Bed, step);

    // 1: the box.
    if past_near && share < rules.share {
        return Some(floor(Step::PastNear));
    }
    if cut && share < rules.share {
        return Some(floor(Step::Cut));
    }
    if !touches {
        return Some(floor(Step::NoTouch));
    }
    // 2: the face at the headboard.
    if let Some((u, w)) = measures.head_uw
        && (0.0..=1.0).contains(&u)
        && (rules.headboard_w[0]..=rules.headboard_w[1]).contains(&w)
    {
        return Some(bed(Step::FaceAtHeadboard));
    }
    // The feet need the pose: without it, the cascade stops here.
    if body.pose.is_none() {
        return Some(floor(Step::PoseNotLooked));
    }
    // 3: the feet.
    let Some(ankle) = ankles else {
        return Some(floor(Step::Unproven));
    };
    if mattress.inside(ankle) {
        let score = measures.foot_score.unwrap_or(0.0);
        let proves = !planes.in_far_strip(ankle, rules.strip_m)? && score >= rules.score_test;
        return Some(if proves {
            bed(Step::FeetOnMattress)
        } else {
            floor(Step::Unproven)
        });
    }
    if let (Some(h), Some(hip)) = (measures.pelvis, hips) {
        let [lo, hi] = rules.e2_band;
        if (lo..=hi).contains(&h) && in_unit(mattress.project(hip), 0.1) {
            return Some(bed(Step::PelvisAtEdge));
        }
    }
    Some(floor(Step::Standing))
}

/// The mean image point of the seen joints among these COCO indices, if any.
pub(super) fn mean_seen(pose: Option<&[Keypoint]>, joints: &[usize; 2], min: f32) -> Option<Point> {
    let seen = seen(pose?, joints, min);
    if seen.is_empty() {
        return None;
    }
    let n = seen.len() as f32;
    Some(Point {
        x: seen.iter().map(|k| k.x).sum::<f32>() / n,
        y: seen.iter().map(|k| k.y).sum::<f32>() / n,
    })
}

/// The mean score of the seen joints among these COCO indices, if any.
pub(super) fn mean_score(pose: Option<&[Keypoint]>, joints: &[usize; 2], min: f32) -> Option<f32> {
    let seen = seen(pose?, joints, min);
    if seen.is_empty() {
        return None;
    }
    Some(seen.iter().map(|k| k.score).sum::<f32>() / seen.len() as f32)
}

pub(super) fn seen<'a>(pose: &'a [Keypoint], joints: &[usize; 2], min: f32) -> Vec<&'a Keypoint> {
    joints
        .iter()
        .filter_map(|&i| pose.get(i))
        .filter(|k| is_seen(k, min))
        .collect()
}

/// The distance from a point to a segment, and the segment's length, in pixels.
fn seg_dist(p: Point, a: Point, b: Point) -> (f64, f64) {
    let (p, a, b) = (xy(p), xy(a), xy(b));
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0);
    let q = [a[0] + t * ab[0], a[1] + t * ab[1]];
    ((p[0] - q[0]).hypot(p[1] - q[1]), len2.sqrt())
}

fn xy(p: Point) -> [f64; 2] {
    [f64::from(p.x), f64::from(p.y)]
}

/// Inside the unit square, with a margin on each side.
fn in_unit((u, v): (f64, f64), margin: f64) -> bool {
    (-margin..=1.0 + margin).contains(&u) && (-margin..=1.0 + margin).contains(&v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::BedPlane;

    fn quad(c: [[f32; 2]; 4]) -> crate::geom::Quad {
        crate::geom::Quad(c.map(|[x, y]| Point { x, y }))
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
        let headboard = quad([
            [951.0, 365.0],
            [1281.0, 325.0],
            [1270.0, 425.0],
            [940.0, 465.0],
        ]);
        let size = crate::room::bed_sizes([0.9, 2.0]);
        ThreePlanes::new(&bed, Some(&floor), Some(&headboard), size).expect("dig1's planes")
    }

    /// The sprint 1 proposal (`workshop/43-ocupacion-por-partes/resultados/reglas.md`, F1.1d).
    const PROPOSAL: CascadeRules = CascadeRules {
        share: 0.45,
        e5_tol: 0.12,
        strip_m: 0.18,
        score_test: 0.75,
        e2_band: [0.6, 1.4],
        kp_min: 0.5,
        // dig1's profile: [occupancy.two_planes] near_m = 0.60; headboard band as in reglas.md.
        near_m: 0.6,
        headboard_w: [-0.15, 1.0],
    };

    #[test]
    fn a_box_past_the_near_side_with_no_share_is_on_the_floor() {
        // Left of the bed, its foot past the near side (u < 0), nothing over the mattress.
        let body = Body {
            bbox: Rect::new(0.0, 0.0, 60.0, 120.0),
            pose: None,
            face: None,
        };
        let v = decide(&dig1(), &body, &PROPOSAL).expect("the footprint");
        assert_eq!(v.step, Step::PastNear);
        assert_eq!(v.zone, BodyZone::Floor { near_bed: false });
        assert_eq!(v.measures.share, 0.0);
    }

    #[test]
    fn without_a_pose_and_without_a_face_the_cascade_says_so() {
        // Dig1 at 113 s: the box of a body lying in the bed, with no pose looked at.
        let body = Body {
            bbox: Rect::new(1300.0, 280.0, 1620.0, 700.0),
            pose: None,
            face: None,
        };
        let v = decide(&dig1(), &body, &PROPOSAL).expect("the footprint");
        assert_eq!(v.step, Step::PoseNotLooked);
        assert_eq!(v.zone, BodyZone::Floor { near_bed: true });
        assert!(!v.measures.foot_seen);
    }

    #[test]
    fn the_measures_place_the_head_the_hips_and_the_ankles_on_their_planes() {
        // A body lying on the mattress: hips at (0.5, 0.5) of the mattress, ankles
        // at (0.5, 0.2) of the footprint, the face at (0.5, 0.5) of the headboard.
        let planes = dig1();
        let headboard = BedPlane::of(&quad([
            [951.0, 365.0],
            [1281.0, 325.0],
            [1270.0, 425.0],
            [940.0, 465.0],
        ]))
        .expect("the headboard");
        let at = |plane: BedPlane, u: f64, v: f64| {
            let p = plane.point(u, v);
            Keypoint {
                x: p.x,
                y: p.y,
                score: 0.9,
            }
        };
        let mut pose = vec![
            Keypoint {
                x: 0.0,
                y: 0.0,
                score: 0.0
            };
            17
        ];
        let hip = at(planes.mattress(), 0.5, 0.5);
        pose[11] = hip;
        pose[12] = hip;
        let ankle = at(planes.floor().expect("the footprint"), 0.5, 0.2);
        pose[15] = ankle;
        pose[16] = ankle;
        let face = headboard.point(0.5, 0.5);
        let body = Body {
            bbox: Rect::new(1000.0, 250.0, 1200.0, 500.0),
            pose: Some(&pose),
            face: Some(face),
        };
        let m = decide(&planes, &body, &PROPOSAL)
            .expect("the footprint")
            .measures;
        let head = m.head.expect("the face on the headboard");
        assert!(
            close(head.u, 0.5, 1e-3) && close(head.v, 0.5, 1e-3),
            "{head:?}"
        );
        let hips = m.hips.expect("the hips seen");
        assert!(
            close(hips.u, 0.5, 1e-3) && close(hips.v, 0.5, 1e-3),
            "{hips:?}"
        );
        let ankles = m.ankles.expect("the ankles seen");
        assert!(
            close(ankles.u, 0.5, 1e-3) && close(ankles.v, 0.2, 1e-3),
            "{ankles:?}"
        );
        assert_eq!(ankles.x, ankle.x);
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn near_bed_is_the_support_within_near_m_not_the_touch() {
        let planes = dig1();
        // Left of the bed and not touching it, its support about two metres away:
        // near only with a wide near_m.
        let left = Body {
            bbox: Rect::new(0.0, 0.0, 60.0, 120.0),
            pose: None,
            face: None,
        };
        let v = decide(
            &planes,
            &left,
            &CascadeRules {
                near_m: 10.0,
                ..PROPOSAL
            },
        )
        .expect("the footprint");
        assert!(!v.measures.touches);
        assert_eq!(v.zone, BodyZone::Floor { near_bed: true });
        // Touching the bed with its support off the footprint: not near with
        // near_m 0, near once near_m reaches its support.
        let lying = Body {
            bbox: Rect::new(1000.0, 300.0, 1200.0, 480.0),
            pose: None,
            face: None,
        };
        let base = decide(&planes, &lying, &PROPOSAL).expect("the footprint");
        let d = base.measures.support_m.expect("the bed's size");
        assert!(base.measures.touches);
        assert!(d > 0.0, "the support is off the footprint: {d}");
        let at_zero = decide(
            &planes,
            &lying,
            &CascadeRules {
                near_m: 0.0,
                ..PROPOSAL
            },
        )
        .expect("the footprint");
        assert_eq!(at_zero.zone, BodyZone::Floor { near_bed: false });
        let at_d = decide(
            &planes,
            &lying,
            &CascadeRules {
                near_m: d,
                ..PROPOSAL
            },
        )
        .expect("the footprint");
        assert_eq!(at_d.zone, BodyZone::Floor { near_bed: true });
    }

    #[test]
    fn a_cut_box_with_most_of_it_on_the_mattress_is_not_cut() {
        // A square bed (100..500 × 100..400) whose footprint lies 40 px lower:
        // its far side is the left edge, x = 100, vertical in the image, so a
        // box centred on the foot at that edge has half of it over the mattress.
        let bed = crate::geom::Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        let floor = crate::geom::Quad::from_rect(Rect::new(100.0, 140.0, 500.0, 440.0));
        let planes = ThreePlanes::new(&bed, Some(&floor), None, crate::room::bed_sizes([0.9, 2.0]))
            .expect("the square bed");
        let body = |bbox: Rect| Body {
            bbox,
            pose: None,
            face: None,
        };
        // Most of the box over the mattress, its foot on the far edge, no ankles.
        let on_bed = decide(
            &planes,
            &body(Rect::new(50.0, 150.0, 150.0, 300.0)),
            &PROPOSAL,
        )
        .expect("the footprint");
        assert!(on_bed.measures.cut, "the foot is on the far edge");
        assert!(
            on_bed.measures.share >= PROPOSAL.share,
            "share {}",
            on_bed.measures.share
        );
        assert_ne!(on_bed.step, Step::Cut);
        // The same foot with little of the box over the mattress: cut.
        let off_bed = decide(
            &planes,
            &body(Rect::new(50.0, 20.0, 150.0, 110.0)),
            &PROPOSAL,
        )
        .expect("the footprint");
        assert!(off_bed.measures.cut);
        assert!(
            off_bed.measures.share < PROPOSAL.share,
            "share {}",
            off_bed.measures.share
        );
        assert_eq!(off_bed.step, Step::Cut);
    }

    #[test]
    fn the_face_at_the_headboard_decides_without_the_pose() {
        // The head of dig1 at the headboard (u, w) = (0.46, 0.66): in bed.
        let body = Body {
            bbox: Rect::new(1000.0, 250.0, 1200.0, 500.0),
            pose: None,
            face: Some(Point {
                x: 1100.0,
                y: 380.0,
            }),
        };
        let v = decide(&dig1(), &body, &PROPOSAL).expect("the footprint");
        assert_eq!(v.step, Step::FaceAtHeadboard);
        assert_eq!(v.zone, BodyZone::Bed);
        assert!(v.measures.head_uw.is_some());
    }

    /// The fixture of the prototype: `classify_s1` with E5S and no memory, over
    /// dig1's body-seconds (`tests/fixtures/cascade-dig1.json`). Generated by
    /// `herramientas/partes.py` (see its README); the command is the fixture's source.
    const FIXTURE: &str = include_str!("../../tests/fixtures/cascade-dig1.json");

    #[derive(serde::Deserialize)]
    struct Fixture {
        bodies: Vec<Row>,
    }

    #[derive(serde::Deserialize)]
    struct Row {
        s: u32,
        i: u32,
        #[serde(rename = "box")]
        bx: [f32; 4],
        kp: Vec<[f32; 3]>,
        face: Option<[f32; 2]>,
        zone: String,
        step: String,
        share_px: f64,
        cut: bool,
        past_near: bool,
        touch: bool,
        pie_visto: bool,
        foot_level: f64,
        #[serde(rename = "uF")]
        u_f: Option<f64>,
        #[serde(rename = "score_F")]
        score_f: Option<f64>,
        #[serde(rename = "h_G")]
        h_g: Option<f64>,
    }

    /// The prototype's step text, as the cascade's step.
    fn proto_step(text: &str) -> Step {
        let table = [
            ("1a past the near edge", Step::PastNear),
            ("1b cut E5", Step::Cut),
            ("1c box does not touch", Step::NoTouch),
            ("2 face at the headboard", Step::FaceAtHeadboard),
            ("3a feet on the mattress", Step::FeetOnMattress),
            (
                "3b feet on the floor, pelvis in the E2 band",
                Step::PelvisAtEdge,
            ),
            ("3c feet on the floor, pelvis outside", Step::Standing),
            ("4 feet on the mattress without proof", Step::Unproven),
            ("4 no feet seen", Step::Unproven),
        ];
        table
            .iter()
            .find(|(t, _)| text.starts_with(t))
            .map(|(_, s)| *s)
            .unwrap_or_else(|| {
                panic!("a step of the prototype that the cascade does not name: {text}")
            })
    }

    /// Body-seconds where the cascade and the prototype differ in zone or step,
    /// all explained by the rounding of the box's share over the mattress (the
    /// cascade's grid of 8 × 8 points against the prototype's pixels): their
    /// share is within 0.01 of the threshold, 0.45. Empty: no difference.
    const EXPLAINED: &[(u32, u32)] = &[];

    #[test]
    fn the_cascade_agrees_with_the_prototype_on_dig1() {
        let fixture: Fixture = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
        let planes = dig1();
        let mut differ = Vec::new();
        for row in &fixture.bodies {
            let pose: Vec<Keypoint> = row
                .kp
                .iter()
                .map(|&[x, y, score]| Keypoint { x, y, score })
                .collect();
            let body = Body {
                bbox: Rect::new(row.bx[0], row.bx[1], row.bx[2], row.bx[3]),
                pose: Some(&pose),
                face: row.face.map(|[x, y]| Point { x, y }),
            };
            let v = decide(&planes, &body, &PROPOSAL).expect("the footprint");
            let zone = match v.zone {
                BodyZone::Bed => "bed",
                BodyZone::Floor { .. } => "floor",
            };
            let want = proto_step(&row.step);
            if zone != row.zone || v.step != want {
                differ.push((row.s, row.i, row.step.clone(), v.step, row.share_px));
                continue;
            }
            // The measures the shadow will publish agree with the prototype's.
            assert_eq!(v.measures.touches, row.touch, "{}s {}", row.s, row.i);
            assert_eq!(v.measures.cut, row.cut, "{}s {}", row.s, row.i);
            assert_eq!(v.measures.past_near, row.past_near, "{}s {}", row.s, row.i);
            assert_eq!(v.measures.foot_seen, row.pie_visto, "{}s {}", row.s, row.i);
            assert_eq!(
                v.measures.foot_level, row.foot_level,
                "{}s {}",
                row.s, row.i
            );
            if let (Some(a), Some(b)) = (v.measures.foot_u, row.u_f) {
                assert!((a - b).abs() < 1e-3, "uF {}s {}: {a} vs {b}", row.s, row.i);
            }
            if let (Some(a), Some(b)) = (v.measures.pelvis, row.h_g) {
                assert!((a - b).abs() < 0.01, "h_G {}s {}: {a} vs {b}", row.s, row.i);
            }
            if let (Some(a), Some(b)) = (v.measures.foot_score, row.score_f) {
                assert!(
                    (f64::from(a) - b).abs() < 1e-3,
                    "score {}s {}",
                    row.s,
                    row.i
                );
            }
        }
        let explained: Vec<(u32, u32)> = differ.iter().map(|(s, i, ..)| (*s, *i)).collect();
        assert_eq!(
            explained,
            EXPLAINED.to_vec(),
            "the cascade differs from the prototype on {} body-seconds: {differ:?}",
            differ.len()
        );
        // Every difference is a share at the threshold, as explained.
        for (s, i, _, _, share) in &differ {
            assert!(
                (share - 0.45).abs() < 0.01,
                "{s}s {i}: share {share} is not at the threshold"
            );
        }
    }

    #[test]
    fn the_fixture_covers_every_step_the_prototype_decides_with() {
        let fixture: Fixture = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
        for want in [
            Step::PastNear,
            Step::Cut,
            Step::NoTouch,
            Step::FaceAtHeadboard,
            Step::FeetOnMattress,
            Step::PelvisAtEdge,
            Step::Standing,
            Step::Unproven,
        ] {
            assert!(
                fixture.bodies.iter().any(|r| proto_step(&r.step) == want),
                "no body-second decided by {want:?}"
            );
        }
    }
}
