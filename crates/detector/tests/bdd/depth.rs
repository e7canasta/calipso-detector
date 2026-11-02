//! Depth capability steps (`features/shell/depth.feature`): the depth capability, with a scripted model that sees a map the
//! scenario builds.
use crate::driver::{detector_profile, scene};
use crate::world::{Shell, models::DepthModel};
use cucumber::{given, then, when};
use scene_geom::{Point, Rect};
use scene_look::{DepthPurpose, Probe, Reading};
use sense::DepthMap;

impl Shell {
    pub fn depth_model(&self) -> &DepthModel {
        self.depth.as_ref().expect("scripted depth model")
    }
    /// Puts `value` in cell `x,y` of the map and shows it to the model.
    fn set_cell(&mut self, x: u32, y: u32, value: f32) {
        let map = &mut self.depth_map;
        let i = (y * map.width + x) as usize;
        map.values[i] = value;
        let map = map.clone();
        self.depth_model().sees(map);
    }
}

fn value(text: &str) -> f32 {
    match text {
        "NaN" => f32::NAN,
        "infinity" => f32::INFINITY,
        v => v.parse().unwrap(),
    }
}

fn points(text: &str) -> Vec<Point> {
    text.split(';')
        .map(|p| {
            let (x, y) = p.trim().split_once(',').unwrap();
            Point {
                x: x.parse().unwrap(),
                y: y.parse().unwrap(),
            }
        })
        .collect()
}

#[given("a scripted depth model")]
fn model(w: &mut Shell) {
    w.depth = Some(DepthModel::default());
}

#[given("there is no depth model")]
fn absent(w: &mut Shell) {
    w.depth = None;
}

#[given("the depth model fails")]
fn broken(w: &mut Shell) {
    w.depth_model().fails();
}

#[given(regex = r"^the depth model sees a (\d+)×(\d+) map worth ([\d.]+) everywhere$")]
fn flat(w: &mut Shell, width: u32, height: u32, v: f32) {
    w.depth_map = DepthMap {
        width,
        height,
        values: vec![v; (width * height) as usize],
    };
    w.depth_model().sees(w.depth_map.clone());
}

#[given(regex = r"^the map is (NaN|infinity|-?[\d.]+) at (\d+),(\d+)$")]
fn one_cell(w: &mut Shell, v: String, x: u32, y: u32) {
    w.set_cell(x, y, value(&v));
}

#[given(
    regex = r"^the map is (NaN|infinity|-?[\d.]+) at (\d+),(\d+) and (NaN|infinity|-?[\d.]+) at (\d+),(\d+)$"
)]
fn two_cells(w: &mut Shell, a: String, xa: u32, ya: u32, b: String, xb: u32, yb: u32) {
    w.set_cell(xa, ya, value(&a));
    w.set_cell(xb, yb, value(&b));
}

#[when(
    regex = r"^the core asks for the subject's depth at points (.+) over (\d+),(\d+) to (\d+),(\d+)$"
)]
fn look_over(w: &mut Shell, list: String, x0: f32, y0: f32, x1: f32, y1: f32) {
    let reading = w.look_sense(&Probe::Depth {
        purpose: DepthPurpose::Subject,
        points: points(&list),
        region: Some(Rect::new(x0, y0, x1, y1)),
    });
    w.reading = Some(reading);
}

#[when(regex = r"^the core asks for the (subject's|bed's) depth at points ([^o]+)$")]
fn look(w: &mut Shell, purpose: String, list: String) {
    let purpose = if purpose == "subject's" {
        DepthPurpose::Subject
    } else {
        DepthPurpose::Bed
    };
    let reading = w.look_sense(&Probe::Depth {
        purpose,
        points: points(&list),
        region: None,
    });
    w.reading = Some(reading);
}

#[then(regex = r"^the reading carries the depths (.+)$")]
fn depths(w: &mut Shell, list: String) {
    let expected: Vec<Option<f32>> = list
        .split(';')
        .map(|v| match v.trim() {
            "none" => None,
            v => Some(v.parse().unwrap()),
        })
        .collect();
    assert_eq!(w.reading, Some(Ok(Reading::Depth(expected))));
}

const DEPTH: &str = "[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n";

#[given("a detector with calibration and scripted models")]
fn calibration_driver(w: &mut Shell) {
    detector_profile(w, &format!("{DEPTH}[calibration]\nretry_secs = 2.0\n"));
}

#[given("a detector with pose, depth and scripted models")]
fn pose_depth_driver(w: &mut Shell) {
    detector_profile(
        w,
        &format!("[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n{DEPTH}"),
    );
}

#[then(regex = r"^the depth model was called (\d+) time, in sample (\d+), with the whole image$")]
fn depth_once(w: &mut Shell, count: usize, sample: u64) {
    let calls = w.depth_model().calls();
    assert_eq!(calls.len(), count, "{calls:?}");
    assert_eq!(calls[0].sample, sample);
    assert_eq!(calls[0].size, (160, 96));
}

#[then(regex = r"^in sample (\d+) it was published that the bed was calibrated at ([\d.]+)$")]
fn calibrated(w: &mut Shell, sample: u64, reference: f32) {
    let events: Vec<_> = w
        .messages
        .iter()
        .filter(|m| m["body"]["type"] == "bed_calibrated")
        .collect();
    assert_eq!(events.len(), 1, "one calibration: {events:?}");
    for event in &events {
        assert_eq!(event["observed_at"]["sequence"], sample);
        assert_eq!(event["body"]["reference_m"], reference);
    }
    let calibration = &scene(w, sample)["body"]["map"]["calibration"];
    assert_eq!(calibration["status"], "complete", "{calibration}");
    assert_eq!(calibration["reference_m"], reference);
}

#[then(regex = r"^the scene of sample (\d+) carries the subject at depth ([\d.]+)$")]
fn subject_depth(w: &mut Shell, sample: u64, meters: f32) {
    let depth = &scene(w, sample)["body"]["subject"]["depth"];
    assert_eq!(depth["meters"], meters, "{depth}");
    assert_eq!(depth["provenance"]["type"], "current", "{depth}");
}
