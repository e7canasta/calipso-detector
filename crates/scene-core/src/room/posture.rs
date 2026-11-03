//! The body's posture in the three-plane frame (white book, volume IX, ch. 04
//! and 08; F5.1): three pure questions the layer 1 will ask, the torso's
//! orientation (E3), the head against the headboard (E6) and the way the legs
//! leave the bed (E7). They read the frame (`three_planes`) and the joints the
//! cascade reads (`cascade`), and they decide nothing. Not connected to layer 1
//! until F5.2: the items are unused in production until then.
#![allow(dead_code)]

use super::cascade::{ANKLES, HIPS, mean_seen};
use super::three_planes::ThreePlanes;
use crate::body::Keypoint;
use crate::geom::Point;

/// COCO indices of the shoulders and of the eyes (the head's centre).
const SHOULDERS: [usize; 2] = [5, 6];
const EYES: [usize; 2] = [1, 2];

/// The thresholds of the posture, named and passed in: the prototype's values
/// (sprint 1) are the caller's, not the code's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PostureRules {
    /// Upright: the torso's axis against «up», in cosine, from this on.
    pub(crate) upright_cos: f64,
    /// Along the bed or across it: the torso's axis against `v` or `u`, in cosine.
    pub(crate) along_cos: f64,
    /// The head on the pillow: `|w|` from this down.
    pub(crate) pillow_w: f64,
    /// Minimum score of a joint to be read.
    pub(crate) kp_min: f32,
}

/// The torso's orientation (E3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Torso {
    /// Parallel to «up»: standing, or sitting up.
    Upright,
    /// Parallel to `v`: lying along the bed.
    AlongBed,
    /// Parallel to `u`: across the bed.
    Crosswise,
    /// Between them: no clear orientation.
    Unclear,
    /// «Up» and `v` are nearly parallel from this camera: E3 does not separate
    /// upright from lying here (cap. 04, límites).
    NotSeparated,
}

/// The head against the headboard, in `(u, w)` (E6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadAt {
    /// On the pillow: `w ≈ 0`.
    Pillow,
    /// Propped against the headboard: `0 < w ≤ 1`.
    Propped,
    /// Above the headboard's top: `w > 1`.
    AboveStop,
    /// Beside the headboard: `u` outside the bed.
    BesideStop,
}

/// Where the legs leave the bed (E7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegsOut {
    /// Past the near side.
    Near,
    /// Past the far side: the feet are hidden by the mattress.
    Far,
    /// Past the foot of the bed (`v < 0`).
    Foot,
    /// Still within the bed's sides.
    Inside,
}

/// The frame's directions in the image, unit vectors: «up» (the room's, from
/// the vertical rule), `v` and `u` on the mattress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Axes {
    up: [f64; 2],
    v: [f64; 2],
    u: [f64; 2],
}

fn unit(v: [f64; 2]) -> Option<[f64; 2]> {
    let length = v[0].hypot(v[1]);
    (length > 0.0).then(|| [v[0] / length, v[1] / length])
}

/// The absolute cosine of two directions: the axis is in pixels, so it is normalized here.
fn cosine(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (a, b) = (unit(a).unwrap_or([0.0, 0.0]), unit(b).unwrap_or([0.0, 0.0]));
    (a[0] * b[0] + a[1] * b[1]).abs()
}

impl Axes {
    fn of(planes: &ThreePlanes) -> Option<Self> {
        let mattress = planes.mattress();
        let vector = |from: Point, to: Point| [f64::from(to.x - from.x), f64::from(to.y - from.y)];
        Some(Self {
            up: planes.vertical_rule(0.5, 0.5)?.up(),
            v: unit(vector(mattress.point(0.5, 0.0), mattress.point(0.5, 1.0)))?,
            u: unit(vector(mattress.point(0.0, 0.5), mattress.point(1.0, 0.5)))?,
        })
    }

    /// The torso's orientation from its axis (the pelvis to the shoulders).
    fn torso(&self, axis: [f64; 2], rules: &PostureRules) -> Torso {
        if cosine(axis, self.up) >= rules.upright_cos {
            Torso::Upright
        } else if cosine(axis, self.v) >= rules.along_cos {
            Torso::AlongBed
        } else if cosine(axis, self.u) >= rules.along_cos {
            Torso::Crosswise
        } else {
            Torso::Unclear
        }
    }
}

/// The torso's orientation (E3): the pelvis to the shoulders, against «up»,
/// `v` and `u`. `None` without the frame or without both joints.
pub(crate) fn torso(
    planes: &ThreePlanes,
    pose: Option<&[Keypoint]>,
    rules: &PostureRules,
) -> Option<Torso> {
    torso_on(&Axes::of(planes)?, pose, rules)
}

/// The torso of a pose on the given axes: the separated test runs before the
/// joints are read.
fn torso_on(axes: &Axes, pose: Option<&[Keypoint]>, rules: &PostureRules) -> Option<Torso> {
    if cosine(axes.up, axes.v) >= rules.upright_cos {
        return Some(Torso::NotSeparated);
    }
    let shoulders = mean_seen(pose, &SHOULDERS, rules.kp_min)?;
    let hips = mean_seen(pose, &HIPS, rules.kp_min)?;
    let axis = [
        f64::from(shoulders.x - hips.x),
        f64::from(shoulders.y - hips.y),
    ];
    Some(axes.torso(axis, rules))
}

/// The head against the headboard (E6). `None` without a headboard or without
/// the eyes.
pub(crate) fn head_at(
    planes: &ThreePlanes,
    pose: Option<&[Keypoint]>,
    rules: &PostureRules,
) -> Option<HeadAt> {
    let head = mean_seen(pose, &EYES, rules.kp_min)?;
    let (u, w) = planes.read_headboard(head)?;
    if !(0.0..=1.0).contains(&u) {
        return Some(HeadAt::BesideStop);
    }
    if w.abs() <= rules.pillow_w {
        Some(HeadAt::Pillow)
    } else if w > 0.0 && w <= 1.0 {
        Some(HeadAt::Propped)
    } else if w > 1.0 {
        Some(HeadAt::AboveStop)
    } else {
        None
    }
}

/// Where the legs leave the bed (E7), from the ankles on the footprint. `None`
/// without the ankles, or without the footprint.
pub(crate) fn legs_out(
    planes: &ThreePlanes,
    pose: Option<&[Keypoint]>,
    rules: &PostureRules,
) -> Option<LegsOut> {
    let ankles = mean_seen(pose, &ANKLES, rules.kp_min)?;
    let (u, v) = planes.foot(ankles)?.uv();
    if v < 0.0 {
        return Some(LegsOut::Foot);
    }
    let sides = planes.sides()?;
    // The side's own end: `u < 0` past the side at 0, `u > 1` past the side at 1.
    let past = |side: f64| if side == 0.0 { u < 0.0 } else { u > 1.0 };
    Some(if past(sides.near) {
        LegsOut::Near
    } else if past(sides.far) {
        LegsOut::Far
    } else {
        LegsOut::Inside
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Quad;

    /// The sprint 1 proposal's thresholds (`workshop/43-ocupacion-por-partes/resultados/reglas.md`).
    const RULES: PostureRules = PostureRules {
        upright_cos: 0.93,
        along_cos: 0.80,
        pillow_w: 0.15,
        kp_min: 0.5,
    };

    fn quad(c: [[f32; 2]; 4]) -> Quad {
        Quad(c.map(|[x, y]| Point { x, y }))
    }

    /// The dig1 camera (`perfiles/dig1-partes.toml`): the frame of three planes.
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

    /// The pose of a dig1 second, from `tests/fixtures/posture-dig1.json`.
    fn pose(second: &str) -> (String, Vec<Keypoint>) {
        let all: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/posture-dig1.json")).unwrap();
        let s = &all[second];
        let label = s["label"].as_str().unwrap().to_owned();
        let kp = s["kp"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| Keypoint {
                x: k[0].as_f64().unwrap() as f32,
                y: k[1].as_f64().unwrap() as f32,
                score: k[2].as_f64().unwrap() as f32,
            })
            .collect();
        (label, kp)
    }

    #[test]
    fn the_axes_of_dig1_are_unit_and_up_is_not_v() {
        let axes = Axes::of(&dig1()).expect("the axes");
        for v in [axes.up, axes.v, axes.u] {
            assert!((v[0].hypot(v[1]) - 1.0).abs() < 1e-9);
        }
        assert!(cosine(axes.up, axes.v) < RULES.upright_cos);
    }

    #[test]
    fn a_vertical_torso_is_upright_a_torso_along_v_lies_and_across_u_is_crosswise() {
        // Unit frame: «up» and `u` are perpendicular, `v` at 45° to both.
        let axes = Axes {
            up: [0.0, -1.0],
            v: [0.707, 0.707],
            u: [1.0, 0.0],
        };
        assert_eq!(axes.torso([0.0, -1.0], &RULES), Torso::Upright);
        assert_eq!(axes.torso([0.707, 0.707], &RULES), Torso::AlongBed);
        assert_eq!(axes.torso([1.0, 0.0], &RULES), Torso::Crosswise);
        assert_eq!(axes.torso([1.0, -1.0], &RULES), Torso::Unclear);
        // Pixels, not unit vectors: the axis of the dig1 lying second is 91 px long.
        assert_eq!(axes.torso([0.0, -91.0], &RULES), Torso::Upright);
    }

    /// A pose with the shoulders at `shoulder` and the hips at `hip`, the rest unseen.
    fn torso_pose(shoulder: (f32, f32), hip: (f32, f32)) -> Vec<Keypoint> {
        let mut kp = vec![
            Keypoint {
                x: 0.0,
                y: 0.0,
                score: 0.0
            };
            17
        ];
        for i in SHOULDERS {
            kp[i] = Keypoint {
                x: shoulder.0,
                y: shoulder.1,
                score: 0.9,
            };
        }
        for i in HIPS {
            kp[i] = Keypoint {
                x: hip.0,
                y: hip.1,
                score: 0.9,
            };
        }
        kp
    }

    #[test]
    fn with_up_and_v_nearly_parallel_the_torso_is_not_separated() {
        // The camera at the feet: «up» and `v` coincide, and E3 says so instead of guessing.
        let axes = Axes {
            up: [0.0, 1.0],
            v: [0.0, 1.0],
            u: [1.0, 0.0],
        };
        let kp = torso_pose((0.0, 10.0), (0.0, 0.0));
        assert_eq!(
            torso_on(&axes, Some(&kp), &RULES),
            Some(Torso::NotSeparated)
        );
    }

    #[test]
    fn dig1_lying_is_along_the_bed() {
        let (label, kp) = pose("5");
        assert_eq!(label, "lying");
        assert_eq!(torso(&dig1(), Some(&kp), &RULES), Some(Torso::AlongBed));
    }

    #[test]
    fn dig1_sitting_up_and_edge_are_upright() {
        for second in ["21", "28", "42"] {
            let (label, kp) = pose(second);
            assert!(
                label == "edge" || label == "sitting_up",
                "{second}: {label}"
            );
            assert_eq!(
                torso(&dig1(), Some(&kp), &RULES),
                Some(Torso::Upright),
                "{second}"
            );
        }
    }

    #[test]
    fn dig1_standing_is_upright() {
        let (label, kp) = pose("150");
        assert_eq!(label, "standing");
        assert_eq!(torso(&dig1(), Some(&kp), &RULES), Some(Torso::Upright));
    }

    /// What `head_at` says today on dig1's sitting seconds. `BesideStop` here is
    /// not "beside the headboard": a head that is not propped on the headboard
    /// (sitting further up the bed) projects onto the headboard's plane out of
    /// `u ∈ [0, 1]`, the drift of E1. `head_at` is only good for a head propped
    /// against the headboard or near it; F5.2 must not read `BesideStop` or
    /// `AboveStop` as a sighting. `head_side` keeps reading the head on the
    /// mattress, as it does today.
    #[test]
    fn a_head_not_propped_on_the_headboard_projects_out_of_its_range() {
        for second in ["21", "28", "42"] {
            let (label, kp) = pose(second);
            assert!(
                label == "edge" || label == "sitting_up",
                "{second}: {label}"
            );
            assert_eq!(
                head_at(&dig1(), Some(&kp), &RULES),
                Some(HeadAt::BesideStop),
                "{second}: the head is projected, not beside the headboard"
            );
        }
    }

    #[test]
    fn without_joints_the_orientation_says_nothing() {
        assert_eq!(torso(&dig1(), None, &RULES), None);
        assert_eq!(head_at(&dig1(), None, &RULES), None);
        assert_eq!(legs_out(&dig1(), None, &RULES), None);
    }
}
