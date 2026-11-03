//! Pose capability steps (`features/shell/pose.feature`), with a scripted pose
//! model.
use crate::driver::{RESIDENT, detector_profile, scene};
use crate::world::{Shell, models::PoseModel};
use cucumber::{given, then, when};
use scene_contract::Keypoint;
use scene_geom::Rect;
use scene_look::{PosePurpose, Probe, Reading};
use sense::PersonPose;

fn person(confidence: f32, keypoints: Vec<Keypoint>) -> PersonPose {
    PersonPose {
        confidence,
        keypoints,
    }
}

fn point(x: f32, y: f32, score: f32) -> Keypoint {
    Keypoint { x, y, score }
}

impl Shell {
    pub fn pose_model(&self) -> &PoseModel {
        self.pose.as_ref().expect("scripted pose model")
    }
    fn keypoints(&self) -> &[Keypoint] {
        match self.reading.as_ref().unwrap() {
            Ok(Reading::Pose(Some(keypoints))) => keypoints,
            other => panic!("there is no pose: {other:?}"),
        }
    }
}

#[given("a scripted pose model")]
fn model(w: &mut Shell) {
    w.pose = Some(PoseModel::default());
}

#[given("there is no pose model")]
fn absent(w: &mut Shell) {
    w.pose = None;
}

#[given("the pose model fails")]
fn broken(w: &mut Shell) {
    w.pose_model().fails();
}

#[given("the pose model sees nobody")]
fn nobody(w: &mut Shell) {
    w.pose_model().sees(vec![]);
}

#[given(regex = r"^the pose model sees a person with a keypoint at (\d+),(\d+) of the crop$")]
fn one_point(w: &mut Shell, x: f32, y: f32) {
    w.pose_model()
        .sees(vec![person(0.9, vec![point(x, y, 0.9)])]);
}

#[given("the pose model sees a person with 17 keypoints, one with score 0.05")]
fn seventeen(w: &mut Shell) {
    let keypoints = (0..17)
        .map(|i| point(i as f32, i as f32, if i == 5 { 0.05 } else { 0.9 }))
        .collect();
    w.pose_model().sees(vec![person(0.9, keypoints)]);
}

#[given(
    regex = r"^the pose model sees a person of confidence ([\d.]+) and another of confidence ([\d.]+)$"
)]
fn two_people(w: &mut Shell, first: f32, second: f32) {
    // Each person marks their only keypoint with their confidence, to recognize them.
    w.pose_model().sees(vec![
        person(first, vec![point(1.0, 1.0, first)]),
        person(second, vec![point(2.0, 2.0, second)]),
    ]);
}

#[when(regex = r"^the core asks for the pose in the box (\d+),(\d+) to (\d+),(\d+)$")]
fn look(w: &mut Shell, x0: f32, y0: f32, x1: f32, y1: f32) {
    let reading = w.look_sense(&Probe::Pose {
        bbox: Rect::new(x0, y0, x1, y1),
        purpose: PosePurpose::Subject,
        up: None,
    });
    w.reading = Some(reading);
}

#[when(
    regex = r"^the core asks for the pose in the box (\d+),(\d+) to (\d+),(\d+) turned so that (\d+),(\d+) points up$"
)]
fn look_turned(w: &mut Shell, x0: f32, y0: f32, x1: f32, y1: f32, dx: f32, dy: f32) {
    let reading = w.look_sense(&Probe::Pose {
        bbox: Rect::new(x0, y0, x1, y1),
        purpose: PosePurpose::Subject,
        up: scene_look::Up::new(dx, dy),
    });
    w.reading = Some(reading);
}

#[then(regex = r"^the reading carries a pose with a keypoint at (\d+),(\d+)$")]
fn mapped(w: &mut Shell, x: f32, y: f32) {
    assert_eq!(w.keypoints(), [point(x, y, 0.9)]);
}

#[then(expr = "the reading carries a pose of {int} keypoints")]
fn count(w: &mut Shell, n: usize) {
    assert_eq!(w.keypoints().len(), n);
}

#[then(expr = "the reading carries a keypoint with score {float}")]
fn low_score(w: &mut Shell, score: f32) {
    assert_eq!(w.keypoints().iter().filter(|k| k.score == score).count(), 1);
}

#[then(expr = "the reading carries the pose of the person of confidence {float}")]
fn chosen(w: &mut Shell, confidence: f32) {
    assert_eq!(w.keypoints().len(), 1);
    assert_eq!(w.keypoints()[0].score, confidence);
}

#[then("the reading says there is no pose")]
fn no_pose(w: &mut Shell) {
    assert_eq!(w.reading, Some(Ok(Reading::Pose(None))));
}

#[given("a detector with pose and scripted models")]
fn pose_driver(w: &mut Shell) {
    detector_profile(w, "[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n");
}

#[then(regex = r"^the pose model was called (\d+) time, in sample (\d+), with the resident's box$")]
fn pose_once(w: &mut Shell, count: usize, sample: u64) {
    let calls = w.pose_model().calls();
    assert_eq!(calls.len(), count, "{calls:?}");
    assert_eq!(calls[0].sample, sample);
    assert_eq!(
        calls[0].size,
        (
            (RESIDENT.x1 - RESIDENT.x0) as u32,
            (RESIDENT.y1 - RESIDENT.y0) as u32
        )
    );
}

#[then(
    regex = r"^the scene of sample (\d+) carries the current pose with a keypoint at (\d+),(\d+)$"
)]
fn current_pose(w: &mut Shell, sample: u64, x: f32, y: f32) {
    let pose = &scene(w, sample)["body"]["subject"]["pose"];
    assert_eq!(pose["provenance"]["type"], "current", "{pose}");
    // What the capability proves: where the keypoint ended up in the frame. The
    // name and the per-keypoint depth belong to the core.
    let keypoints = pose["keypoints"].as_array().expect("there are keypoints");
    let places: Vec<_> = keypoints
        .iter()
        .map(|k| (k["x"].clone(), k["y"].clone(), k["score"].clone()))
        .collect();
    assert_eq!(
        places,
        vec![(
            serde_json::json!(x),
            serde_json::json!(y),
            serde_json::json!(0.9)
        )],
        "{pose}"
    );
}

#[then(regex = r"^the scene of sample (\d+) carries the pose stable since sample (\d+)$")]
fn stable_pose(w: &mut Shell, sample: u64, since: u64) {
    let pose = &scene(w, sample)["body"]["subject"]["pose"];
    assert_eq!(pose["provenance"]["type"], "stable", "{pose}");
    assert_eq!(pose["provenance"]["since"]["sequence"], since, "{pose}");
}
