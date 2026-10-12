//! Profile steps for pose and depth: the pose, depth and calibration sections are
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

// --- The focus (spec 1511): the main detector's own section and the map's box.

/// The focus section, appended to the baseline profile.
fn focus_section(pad: f32, wide_every_secs: u32, walk_secs: u32) -> String {
    format!(
        "\n[senses.detect.focus]\npad = {pad}\nwide_every_secs = {wide_every_secs}\nwalk_secs = {walk_secs}\n"
    )
}

#[given(
    regex = r"^a profile whose main detector focuses on the bed grown by (-?[\d.]+), looking at the whole frame every (\d+) seconds, and staff walk (\d+) seconds from the door to the bed$"
)]
fn focusing_profile(w: &mut Shell, pad: f32, wide_every_secs: u32, walk_secs: u32) {
    w.parse_config(&focus_section(pad, wide_every_secs, walk_secs));
}

#[given(regex = r"^a focusing profile whose map delimits the focus as (\d+),(\d+) to (\d+),(\d+)$")]
fn map_focus(w: &mut Shell, x0: u32, y0: u32, x1: u32, y1: u32) {
    let base = include_str!("../support/profile.toml").replace(
        "[map]\n",
        &format!("[map]\nfocus = [{x0}, {y0}, {x1}, {y1}]\n"),
    );
    w.config = Some(
        detector::config::Config::parse(&format!("{base}{}", focus_section(0.25, 3, 3)))
            .map_err(|e| e.to_string()),
    );
}

#[then("the core policy does not focus")]
fn no_focus(w: &mut Shell) {
    assert!(w.config().policy().focus.is_none());
}

#[then(
    expr = "the core policy focuses on the bed grown by {float} and looks at the whole frame every {int} seconds"
)]
fn focus_policy(w: &mut Shell, pad: f32, wide_every_secs: u32) {
    let focus = w.config().policy().focus.expect("focus policy");
    assert_eq!(focus.pad, pad);
    assert_eq!(focus.wide_every_secs, f64::from(wide_every_secs));
}

#[then(regex = r"^the core's site map focuses on (\d+),(\d+) to (\d+),(\d+)$")]
fn site_focus(w: &mut Shell, x0: f32, y0: f32, x1: f32, y1: f32) {
    assert_eq!(
        w.config().site_map().focus,
        Some(scene_core::geom::Rect::new(x0, y0, x1, y1))
    );
}
