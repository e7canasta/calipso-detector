//! F2.4 criterion 2: a fake pose port that knows where the body is returns its
//! keypoints in the turned crop; the shell must bring them back to the frame
//! within half a pixel, and the canvas must hold every body point.

use image::DynamicImage;
use scene_contract::Keypoint;
use scene_geom::Rect;
use scene_look::{Failure, PosePurpose, Probe, Reading, Up};
use sense::{DetectorPort, ModelError, PersonPose, PosePort, Senses};
use std::cell::Cell;
use std::rc::Rc;

/// Body points in frame coordinates; all inside the box 100,100 to 200,300.
const BODY: [(f32, f32); 6] = [
    (100.0, 100.0),
    (200.0, 100.0),
    (200.0, 300.0),
    (100.0, 300.0),
    (150.0, 200.0),
    (120.0, 250.0),
];
const CENTRE: (f32, f32) = (150.0, 200.0);

struct NoDetector;
impl DetectorPort for NoDetector {
    fn detect(&mut self, _: &DynamicImage) -> Result<Vec<scene_look::Detection>, ModelError> {
        Ok(Vec::new())
    }
}

/// Knows where the body is and where the turned crop puts it: it returns the
/// body points in the crop it was sent, and records that crop's size.
struct KnownBody {
    up: (f32, f32),
    size: Rc<Cell<(u32, u32)>>,
}

impl PosePort for KnownBody {
    fn estimate(&mut self, image: &DynamicImage) -> Result<Vec<PersonPose>, ModelError> {
        let (w, h) = (image.width(), image.height());
        self.size.set((w, h));
        // The rotation that takes the requested direction to up (0, -1).
        let theta = -std::f32::consts::FRAC_PI_2 - self.up.1.atan2(self.up.0);
        let (sin, cos) = theta.sin_cos();
        let keypoints = BODY
            .iter()
            .map(|&(x, y)| {
                let (a, b) = (x - CENTRE.0, y - CENTRE.1);
                Keypoint {
                    x: cos * a - sin * b + w as f32 / 2.0,
                    y: sin * a + cos * b + h as f32 / 2.0,
                    score: 0.9,
                }
            })
            .collect();
        Ok(vec![PersonPose {
            confidence: 0.9,
            keypoints,
        }])
    }
}

/// The keypoints the shell returns for a turn, and the size of the crop the port saw.
fn turned_pose(up: (f32, f32)) -> (Vec<Keypoint>, (u32, u32)) {
    let frame = DynamicImage::new_rgb8(400, 400);
    let size = Rc::new(Cell::new((0, 0)));
    let port = KnownBody {
        up,
        size: Rc::clone(&size),
    };
    let mut senses = Senses::new(NoDetector).with_pose(port);
    let probe = Probe::Pose {
        bbox: Rect::new(100.0, 100.0, 200.0, 300.0),
        purpose: PosePurpose::Subject,
        up: Up::new(up.0, up.1),
    };
    let reading = senses.look(&frame, &probe.sense().expect("a sense probe"));
    let Ok(Reading::Pose(Some(keypoints))) = reading else {
        panic!("no pose: {reading:?}")
    };
    (keypoints, size.get())
}

fn assert_round_trip(up: (f32, f32)) {
    let (keypoints, (w, h)) = turned_pose(up);
    assert_eq!(keypoints.len(), BODY.len());
    for (back, &(x, y)) in keypoints.iter().zip(BODY.iter()) {
        assert!(
            (back.x - x).abs() <= 0.5 && (back.y - y).abs() <= 0.5,
            "up {up:?} (crop {w}x{h}): got ({}, {}), want ({x}, {y})",
            back.x,
            back.y,
        );
    }
}

#[test]
fn zero_degrees_comes_back_exact() {
    assert_round_trip((0.0, -1.0));
}

#[test]
fn ninety_degrees_comes_back_exact() {
    assert_round_trip((1.0, 0.0));
    assert_round_trip((-1.0, 0.0));
}

#[test]
fn one_eighty_degrees_comes_back_exact() {
    assert_round_trip((0.0, 1.0));
}

#[test]
fn oblique_comes_back_exact() {
    assert_round_trip((1.0, 1.0));
    assert_round_trip((-1.0, 0.5));
}

#[test]
fn the_canvas_holds_every_body_point() {
    for up in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let (_, (w, h)) = turned_pose(up);
        // Each body point, moved into the canvas by the same rotation, lies inside it.
        let theta = -std::f32::consts::FRAC_PI_2 - up.1.atan2(up.0);
        let (sin, cos) = theta.sin_cos();
        for &(x, y) in &BODY {
            let (a, b) = (x - CENTRE.0, y - CENTRE.1);
            let qx = cos * a - sin * b + w as f32 / 2.0;
            let qy = sin * a + cos * b + h as f32 / 2.0;
            // A corner of the box lies on the canvas edge; allow the same half pixel as the round trip.
            assert!(
                qx >= -0.5 && qx <= w as f32 + 0.5 && qy >= -0.5 && qy <= h as f32 + 0.5,
                "up {up:?}: ({x}, {y}) falls outside the {w}x{h} canvas at ({qx}, {qy})"
            );
        }
    }
}

#[test]
fn a_failure_of_the_port_is_data() {
    struct Broken;
    impl PosePort for Broken {
        fn estimate(&mut self, _: &DynamicImage) -> Result<Vec<PersonPose>, ModelError> {
            Err(ModelError::Inference("boom".into()))
        }
    }
    let frame = DynamicImage::new_rgb8(400, 400);
    let mut senses = Senses::new(NoDetector).with_pose(Broken);
    let probe = Probe::Pose {
        bbox: Rect::new(100.0, 100.0, 200.0, 300.0),
        purpose: PosePurpose::Subject,
        up: Up::new(1.0, 0.0),
    };
    let reading = senses.look(&frame, &probe.sense().expect("a sense probe"));
    assert!(matches!(reading, Err(Failure::Model)), "{reading:?}");
}
