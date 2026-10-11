//! 811–812 steps on the profile: the pose, depth and calibration sections are
//! the only switches of `Policy.enrich` and `Policy.calibration`.
use crate::world::Shell;
use cucumber::{given, then};

#[given(
    regex = r#"^a profile with "\[senses.pose\]" of model "([^"]+)", motion threshold (-?[\d.]+) and minimum score (-?[\d.]+)$"#
)]
fn pose_profile(w: &mut Shell, model: String, threshold: f32, score: f32) {
    w.parse_config(&format!(
        "\n[senses.pose]\nmodel = \"{model}\"\nmove_threshold = {threshold}\nmin_keypoint_score = {score}\n"
    ));
}

#[then(expr = "the core policy requests the pose when the subject moves {float} of their diagonal")]
fn enrich_policy(w: &mut Shell, threshold: f32) {
    let enrich = w.config().policy().enrich.expect("pose policy");
    assert_eq!(enrich.move_threshold, threshold);
}

#[then("the core policy does not measure the subject's depth")]
fn no_subject_depth(w: &mut Shell) {
    assert!(!w.config().policy().enrich.expect("pose policy").depth);
}

#[then("the core neither requests the pose nor calibrates the bed")]
fn neither(w: &mut Shell) {
    let policy = w.config().policy();
    assert!(policy.enrich.is_none());
    assert!(policy.calibration.is_none());
}

const POSE: &str = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n";

#[given(regex = r#"^a profile with pose and "\[senses.depth\]" of model "([^"]+)"$"#)]
fn pose_and_depth(w: &mut Shell, model: String) {
    w.parse_config(&format!("{POSE}[senses.depth]\nmodel = \"{model}\"\n"));
}

#[given(
    regex = r#"^a profile with "\[senses.depth\]" of model "([^"]+)" and "\[calibration\]" retrying every (-?[\d.]+) seconds$"#
)]
fn depth_and_calibration(w: &mut Shell, model: String, retry: f64) {
    w.parse_config(&format!(
        "\n[senses.depth]\nmodel = \"{model}\"\n[calibration]\nretry_secs = {retry}\n"
    ));
}

#[given(r#"a profile with "[calibration]" without "[senses.depth]""#)]
fn calibration_alone(w: &mut Shell) {
    w.parse_config("\n[calibration]\nretry_secs = 2.0\n");
}

#[given(r#"a profile with "[senses.depth]" without pose or calibration"#)]
fn depth_alone(w: &mut Shell) {
    w.parse_config("\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n");
}

#[then(
    expr = "the core policy measures the subject's depth at keypoints with score {float} or more"
)]
fn subject_depth(w: &mut Shell, score: f32) {
    let enrich = w.config().policy().enrich.expect("pose policy");
    assert!(enrich.depth);
    assert_eq!(enrich.min_keypoint_score, score);
}

#[then("the core does not calibrate the bed")]
fn no_calibration(w: &mut Shell) {
    assert!(w.config().policy().calibration.is_none());
}

#[then("the core does not request the pose")]
fn no_pose(w: &mut Shell) {
    assert!(w.config().policy().enrich.is_none());
}

#[then(
    expr = "the core policy calibrates the bed and retries after {float} seconds of source time"
)]
fn calibration(w: &mut Shell, retry: f64) {
    let calibration = w.config().policy().calibration.expect("calibration");
    assert_eq!(calibration.retry_secs, retry);
}
