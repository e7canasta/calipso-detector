//! Posture steps (`features/posture`): the loop's second level. What the
//! scene publishes is checked as its consumers read it: the contract's JSON.

use cucumber::{given, then};
use serde_json::Value;

use crate::attention::scene;
use crate::script::depth_at;
use crate::ward::Ward;

/// The 17 COCO keypoints, in order (901-1).
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

// --- Truly lying (902)

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

// --- Posture by disparity (904)

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
    assert_eq!(disparity > 0.0, sign == "positive", "{posture}");
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

// --- The pose's maximum age (903)

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
