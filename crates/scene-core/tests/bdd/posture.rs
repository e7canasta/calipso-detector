//! Posture steps (`features/posture`): the loop's second level. What the
//! scene publishes is checked as its consumers read it: the contract's JSON.

use cucumber::{given, then};
use serde_json::Value;

use crate::attention::scene;
use crate::script::depth_at;
use crate::ward::Ward;

/// The 17 COCO keypoints, in order.
const COCO: [&str; 17] = [
    "nose",
    "left_eye",
    "right_eye",
    "left_ear",
    "right_ear",
    "left_shoulder",
    "right_shoulder",
    "left_elbow",
    "right_elbow",
    "left_wrist",
    "right_wrist",
    "left_hip",
    "right_hip",
    "left_knee",
    "right_knee",
    "left_ankle",
    "right_ankle",
];

/// The keypoints of the pose published in sample `n`, as JSON.
fn keypoints(ward: &Ward, n: u32) -> Vec<Value> {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["pose"]["keypoints"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "in sample {n} there is no published pose: {}",
                scene["subject"]
            )
        })
        .clone()
}

#[then(expr = "in sample {int} the pose keypoints are named, in order, as the 17 of COCO")]
fn coco_names(ward: &mut Ward, n: u32) {
    let names: Vec<Option<String>> = keypoints(ward, n)
        .iter()
        .map(|p| p["name"].as_str().map(String::from))
        .collect();
    let expected: Vec<Option<String>> = COCO.iter().map(|c| Some((*c).to_owned())).collect();
    assert_eq!(names, expected);
}

#[then(expr = "in sample {int} exactly the keypoints with enough score carry depth")]
fn the_good_ones_measured(ward: &mut Ward, n: u32) {
    let minimum = f64::from(
        ward.policy
            .enrich
            .expect("there is a pose model")
            .min_keypoint_score,
    );
    for (i, p) in keypoints(ward, n).iter().enumerate() {
        let good = p["score"].as_f64().expect("score") >= minimum;
        let measured = p["depth"].is_number();
        assert_eq!(measured, good, "keypoint {i}: {p}");
    }
}

#[then(expr = "in sample {int} each measured keypoint carries the depth read at it")]
fn each_its_own(ward: &mut Ward, n: u32) {
    let measured: Vec<Value> = keypoints(ward, n)
        .into_iter()
        .filter(|p| p["depth"].is_number())
        .collect();
    assert!(!measured.is_empty(), "no keypoint measured");
    for p in measured {
        let point = scene_core::geom::Point {
            x: p["x"].as_f64().expect("x") as f32,
            y: p["y"].as_f64().expect("y") as f32,
        };
        let expected = f64::from(depth_at(point));
        let depth = p["depth"].as_f64().expect("depth");
        assert!((depth - expected).abs() < 1e-5, "{p}: {expected} was read");
    }
}

#[then(expr = "in sample {int} no keypoint carries depth")]
fn none_measured(ward: &mut Ward, n: u32) {
    let measured: Vec<Value> = keypoints(ward, n)
        .into_iter()
        .filter(|p| !p["depth"].is_null())
        .collect();
    assert!(measured.is_empty(), "{measured:?}");
}

#[then(expr = "in sample {int} the pose keypoints are those of sample {int}, with their depth")]
fn stable_keypoints(ward: &mut Ward, n: u32, m: u32) {
    let before = keypoints(ward, m);
    assert!(
        before.iter().any(|p| p["depth"].is_number()),
        "nothing was measured in {m}"
    );
    assert_eq!(keypoints(ward, n), before);
}

#[then(expr = "in sample {int} the subject's depth is the median of their keypoints")]
fn the_median(ward: &mut Ward, n: u32) {
    let mut values: Vec<f64> = keypoints(ward, n)
        .iter()
        .filter_map(|p| p["depth"].as_f64())
        .collect();
    assert!(!values.is_empty(), "no keypoint measured");
    values.sort_by(f64::total_cmp);
    let half = values.len() / 2;
    let median = if values.len().is_multiple_of(2) {
        (values[half - 1] + values[half]) / 2.0
    } else {
        values[half]
    };
    let subject = scene(ward, n)
        .subject
        .as_ref()
        .and_then(|s| s.depth)
        .expect("the subject has depth");
    assert!(
        (f64::from(subject.meters) - median).abs() < 1e-5,
        "{} and {median}",
        subject.meters
    );
}

// --- Truly lying

/// `subject.lying` published in sample `n`, as JSON.
fn lying(ward: &Ward, n: u32) -> Value {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["lying"].clone()
}

#[then(
    regex = r"^in sample (\d+) (they are truly lying, confirmed|they are truly lying, assumed|they are not lying|it is unknown whether they are lying)$"
)]
fn is_lying(ward: &mut Ward, n: u32, how: String) {
    let expected = match how.as_str() {
        "they are truly lying, confirmed" => "confirmed",
        "they are truly lying, assumed" => "assumed",
        "they are not lying" => "no",
        _ => "unknown",
    };
    let lying = lying(ward, n);
    assert_eq!(lying["state"], expected, "{lying}");
}

#[then(expr = "in sample {int} lying is stable since sample {int}")]
fn lying_stable(ward: &mut Ward, n: u32, m: u32) {
    let lying = lying(ward, n);
    assert_eq!(lying["provenance"]["type"], "stable", "{lying}");
    assert_eq!(
        lying["provenance"]["since"]["sequence"],
        ward.sample(m).sequence,
        "{lying}"
    );
}

// --- Posture by disparity

/// `subject.posture` published in sample `n`, as JSON.
fn posture(ward: &Ward, n: u32) -> Value {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["posture"].clone()
}

#[then(regex = r"^in sample (\d+) the posture is (lying|upright)$")]
fn the_posture(ward: &mut Ward, n: u32, which: String) {
    let posture = posture(ward, n);
    assert_eq!(posture["state"], which.as_str(), "{posture}");
}

#[then(regex = r"^in sample (\d+) the posture disparity is (positive|negative)$")]
fn the_disparity(ward: &mut Ward, n: u32, sign: String) {
    let posture = posture(ward, n);
    let disparity = posture["disparity"]
        .as_f64()
        .unwrap_or_else(|| panic!("{posture}"));
    let strict = if sign == "positive" {
        disparity > 0.0
    } else {
        disparity < 0.0
    };
    assert!(strict, "the disparity is not {sign}: {posture}");
}

#[then(expr = "in sample {int} the posture is stable since sample {int}")]
fn stable_posture(ward: &mut Ward, n: u32, m: u32) {
    let posture = posture(ward, n);
    assert_eq!(posture["provenance"]["type"], "stable", "{posture}");
    assert_eq!(
        posture["provenance"]["since"]["sequence"],
        ward.sample(m).sequence,
        "{posture}"
    );
}

// --- The pose's maximum age

#[given(expr = "the pose expires after {int} seconds of source time")]
fn expires(ward: &mut Ward, secs: u32) {
    ward.policy
        .enrich
        .as_mut()
        .expect("\"there is a pose model\" first")
        .max_pose_age_secs = Some(f64::from(secs));
}

#[then(
    expr = "between samples {int} and {int} the published pose is never {int} seconds old or more"
)]
fn never_expired(ward: &mut Ward, from: u32, to: u32, secs: u32) {
    for n in from..=to {
        let now = ward.sample(n);
        let pose = scene(ward, n)
            .subject
            .as_ref()
            .and_then(|s| s.pose.as_ref())
            .unwrap_or_else(|| panic!("in sample {n} there is no pose"));
        let age = match pose.provenance {
            scene_core::contract::Provenance::Stable { since } => {
                now.source_secs.0 - since.source_secs.0
            }
            _ => 0.0,
        };
        assert!(
            age < f64::from(secs),
            "in sample {n} the pose is {age} s old"
        );
    }
}

#[then(expr = "between samples {int} and {int} the pose was requested {int} times")]
fn poses_requested_in(ward: &mut Ward, from: u32, to: u32, times: usize) {
    use scene_core::contract::{LookKind, LookOutcome};
    let requested: Vec<u32> =
        (from..=to)
            .filter(|&n| {
                scene(ward, n).attention.looks.iter().any(|l| {
                    l.kind == LookKind::Pose && matches!(l.outcome, LookOutcome::Asked { .. })
                })
            })
            .collect();
    assert_eq!(requested.len(), times, "requested in {requested:?}");
}

// --- The body groups in the scene (R16, 1604 and 1606)

#[then(
    regex = r#"^in sample (\d+) the group "(head|shoulders|pelvis)" saw (\d+) of (\d+) keypoints$"#
)]
fn group_saw(ward: &mut Ward, n: u32, group: String, seen: u64, of: u64) {
    let posture = posture(ward, n);
    let g = &posture["groups"][group.as_str()];
    assert_eq!(
        (g["seen"].as_u64(), g["of"].as_u64()),
        (Some(seen), Some(of)),
        "{posture}"
    );
}

#[then(regex = r#"^in sample (\d+) the group "(head|shoulders|pelvis)" has no depth$"#)]
fn group_without_depth(ward: &mut Ward, n: u32, group: String) {
    let posture = posture(ward, n);
    assert!(
        posture["groups"][group.as_str()]["depth"].is_null(),
        "{posture}"
    );
}

#[then(regex = r#"^in sample (\d+) the disparities "([^"]+)" are (positive|negative)$"#)]
fn disparities_sign(ward: &mut Ward, n: u32, names: String, sign: String) {
    let posture = posture(ward, n);
    for name in names.split(',').map(str::trim) {
        let d = posture["disparities"][name]
            .as_f64()
            .unwrap_or_else(|| panic!("no {name}: {posture}"));
        assert!(
            if sign == "positive" { d > 0.0 } else { d < 0.0 },
            "{name} is not {sign}: {posture}"
        );
    }
}

#[then(expr = "in sample {int} the posture tolerance is {float}")]
fn posture_tolerance(ward: &mut Ward, n: u32, tolerance: f64) {
    let posture = posture(ward, n);
    let published = posture["tolerance"]
        .as_f64()
        .unwrap_or_else(|| panic!("{posture}"));
    assert!((published - tolerance).abs() < 1e-6, "{posture}");
}

#[then(expr = "in sample {int} the disparity {string} is the posture disparity")]
fn deciding_disparity(ward: &mut Ward, n: u32, name: String) {
    let posture = posture(ward, n);
    assert_eq!(
        posture["disparities"][name.as_str()],
        posture["disparity"],
        "{posture}"
    );
}

#[then(expr = "in sample {int} the posture still publishes upper, lower and disparity")]
fn todays_fields(ward: &mut Ward, n: u32) {
    let posture = posture(ward, n);
    for key in ["upper", "lower", "disparity", "state", "provenance"] {
        assert!(
            posture.get(key).is_some_and(|v| !v.is_null()),
            "no {key}: {posture}"
        );
    }
}

#[then(expr = "in sample {int} the subject publishes no {string} key")]
fn subject_without_key(ward: &mut Ward, n: u32, key: String) {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    assert!(
        scene["subject"].get(key.as_str()).is_none(),
        "{}",
        scene["subject"]
    );
}

// --- The verdict (R16, 1605)

fn verdict(ward: &Ward, n: u32) -> Value {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["verdict"].clone()
}

#[then(expr = "in sample {int} the verdict is {string}, by {string}")]
fn the_verdict(ward: &mut Ward, n: u32, state: String, by: String) {
    let v = verdict(ward, n);
    assert_eq!(v["state"], state.as_str(), "{v}");
    let heads: Vec<&str> = v["by"]
        .as_array()
        .unwrap_or_else(|| panic!("{v}"))
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(heads.join(", "), by, "{v}");
}

#[then(expr = "in sample {int} the verdict is {string} since sample {int}")]
fn verdict_since(ward: &mut Ward, n: u32, state: String, since: u32) {
    let v = verdict(ward, n);
    assert_eq!(v["state"], state.as_str(), "{v}");
    let expected = serde_json::to_value(ward.sample(since)).expect("the sample serializes");
    assert_eq!(v["since"], expected, "{v}");
}

#[then(expr = "in sample {int} there is no verdict")]
fn no_verdict(ward: &mut Ward, n: u32) {
    let v = verdict(ward, n);
    assert!(v.is_null(), "{v}");
}

#[then(expr = "in sample {int} the subject is on the floor")]
fn subject_on_the_floor(ward: &mut Ward, n: u32) {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    assert_eq!(scene["subject"]["zone"], "floor", "{}", scene["subject"]);
}

// --- The bed's points (R17): measured in the same look as the body

/// The bed point `name` the posture of sample `n` published.
fn bed_point(ward: &Ward, n: u32, name: &str) -> Value {
    let posture = posture(ward, n);
    posture["bed_points"]
        .as_array()
        .and_then(|points| points.iter().find(|p| p["name"] == name))
        .unwrap_or_else(|| panic!("in sample {n} there is no bed point {name}: {posture}"))
        .clone()
}

#[then(expr = "in sample {int} the bed point {string} was measured at the bed's centre")]
fn bed_point_at_the_centre(ward: &mut Ward, n: u32, name: String) {
    let p = bed_point(ward, n, &name);
    let centre = crate::ward::BED.centroid();
    let at = |key: &str| p[key].as_f64().unwrap_or_else(|| panic!("{p}"));
    assert!((at("x") - f64::from(centre.x)).abs() < 1e-3, "{p}");
    assert!((at("y") - f64::from(centre.y)).abs() < 1e-3, "{p}");
    assert!(p["depth"].is_number(), "{p}");
}

#[then(expr = "in sample {int} the head is farther from the camera than the bed point {string}")]
fn head_beyond_the_bed_point(ward: &mut Ward, n: u32, name: String) {
    let head = posture(ward, n)["groups"]["head"]["depth"].as_f64();
    let point = bed_point(ward, n, &name)["depth"].as_f64();
    assert!(
        head.zip(point).is_some_and(|(head, point)| head > point),
        "head {head:?}, {name} {point:?}"
    );
}
