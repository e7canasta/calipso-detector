#![cfg(feature = "onnx")]

use image::DynamicImage;
use scene_look::{Failure, Label, PosePurpose, Probe, Reading, Spent};
use sense::{Senses, ultralytics::Detector};
use std::time::Instant;

/// The answer and its time, measured here the way the shell's router does: the
/// answerer returns no time (ADR 2026-11-02, quien-responde-cada-probe).
fn timed(
    senses: &mut Senses,
    image: &DynamicImage,
    probe: &Probe,
) -> (Result<Reading, Failure>, Spent) {
    let started = Instant::now();
    let result = senses.look(image, &probe.sense().expect("a sense probe"));
    (result, Spent(started.elapsed().as_secs_f64() * 1000.0))
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s.onnx and ONNX Runtime"]
fn real_onnx_detects_a_person() {
    let models = std::env::var_os("CALIPSO_MODELS").expect("set CALIPSO_MODELS");
    let detector = Detector::load(&std::path::PathBuf::from(models).join("yolo26s.onnx"), 0.25)
        .expect("load yolo26s.onnx");
    let image = image::load_from_memory(include_bytes!("fixtures/astronaut.jpg")).unwrap();
    let request = Probe::Detect { region: None };
    let (result, spent) = timed(&mut Senses::new(detector), &image, &request);
    let Ok(Reading::Bodies(boxes)) = result else {
        panic!("model failed: {result:?}")
    };
    println!("boxes={boxes:?}; spent_ms={:.3}", spent.0);
    assert!(
        boxes
            .iter()
            .any(|d| d.label == Label::Person && d.confidence > 0.5)
    );
}

struct NeverCalled;
impl sense::DetectorPort for NeverCalled {
    fn detect(
        &mut self,
        _: &image::DynamicImage,
    ) -> Result<Vec<scene_look::Detection>, sense::ModelError> {
        panic!("primary must not run for reinforcement or faces")
    }
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26l.onnx and ONNX Runtime"]
fn real_onnx_reinforces_a_region() {
    use scene_geom::Rect;
    let models =
        std::path::PathBuf::from(std::env::var_os("CALIPSO_MODELS").expect("set CALIPSO_MODELS"));
    let model = Detector::load(&models.join("yolo26l.onnx"), 0.25).expect("load reinforcement");
    let image = image::load_from_memory(include_bytes!("fixtures/astronaut.jpg")).unwrap();
    let region = Rect::new(20.0, 0.0, image.width() as f32, image.height() as f32);
    let request = Probe::Reinforce { region };
    let mut senses = Senses::new(NeverCalled).with_reinforce(model);
    assert!(timed(&mut senses, &image, &request).0.is_ok()); // warm-up, outside the five measurements
    let mut times = Vec::new();
    for _ in 0..5 {
        let (reading, spent) = timed(&mut senses, &image, &request);
        let Ok(Reading::Bodies(boxes)) = reading else {
            panic!("reinforcement failed: {reading:?}")
        };
        assert!(
            boxes
                .iter()
                .any(|d| d.label == Label::Person && d.confidence > 0.5)
        );
        assert!(boxes.iter().all(|d| d.bbox.x0 >= region.x0
            && d.bbox.y0 >= region.y0
            && d.bbox.x1 <= region.x1
            && d.bbox.y1 <= region.y1));
        times.push(spent.0);
        println!("611 M-1: boxes={boxes:?}; spent_ms={:.3}", spent.0);
    }
    println!("611 M-1: five runs ms={times:?}");
}

#[test]
#[ignore = "requires CALIPSO_MODELS/face/yolov12l-face.onnx and ONNX Runtime"]
fn real_onnx_faces_on_full_image_and_upper_crop() {
    use scene_geom::Rect;
    use scene_look::FaceRegion;
    let models =
        std::path::PathBuf::from(std::env::var_os("CALIPSO_MODELS").expect("set CALIPSO_MODELS"));
    let name =
        std::env::var("CALIPSO_FACE_MODEL").unwrap_or_else(|_| "face/yolov12l-face.onnx".into());
    let model = Detector::load(&models.join(&name), 0.25).expect("load faces");
    let image = image::load_from_memory(include_bytes!("fixtures/astronaut.jpg")).unwrap();
    let mut senses = Senses::new(NeverCalled).with_faces(model);
    for (case, height) in [("full", image.height()), ("upper", image.height() / 2)] {
        let rect = Rect::new(0.0, 0.0, image.width() as f32, height as f32);
        let request = Probe::Faces {
            region: FaceRegion::Headboard(rect),
        };
        assert!(timed(&mut senses, &image, &request).0.is_ok());
        let mut times = Vec::new();
        for _ in 0..5 {
            let (reading, spent) = timed(&mut senses, &image, &request);
            let Ok(Reading::Faces(boxes)) = reading else {
                panic!("faces failed: {reading:?}")
            };
            assert!(
                boxes
                    .iter()
                    .any(|d| d.confidence >= 0.30 && d.bbox.centroid().y <= height as f32),
                "{name} {case}: {boxes:?}"
            );
            assert!(boxes.iter().all(|d| d.label == Label::Other));
            times.push(spent.0);
            println!(
                "612 M-1 {name} {case}: boxes={boxes:?}; spent_ms={:.3}",
                spent.0
            );
        }
        println!("612 M-1 {name} {case}: five runs ms={times:?}");
    }
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s.onnx, yolo26s-pose.onnx and ONNX Runtime"]
fn real_onnx_pose_on_the_person_box() {
    use sense::ultralytics::PoseEstimator;
    let models =
        std::path::PathBuf::from(std::env::var_os("CALIPSO_MODELS").expect("set CALIPSO_MODELS"));
    let image = image::load_from_memory(include_bytes!("fixtures/astronaut.jpg")).unwrap();
    let detector = Detector::load(&models.join("yolo26s.onnx"), 0.25).expect("load yolo26s");
    let pose = PoseEstimator::load(&models.join("yolo26s-pose.onnx"), 0.25).expect("load pose");
    let mut senses = Senses::new(detector).with_pose(pose);
    let detect = Probe::Detect { region: None };
    let Ok(Reading::Bodies(boxes)) = timed(&mut senses, &image, &detect).0 else {
        panic!("detector failed")
    };
    let bbox = boxes
        .iter()
        .filter(|d| d.label == Label::Person)
        .max_by(|a, b| a.confidence.total_cmp(&b.confidence))
        .expect("a person")
        .bbox;
    let request = Probe::Pose {
        bbox,
        purpose: PosePurpose::Subject,
    };
    assert!(timed(&mut senses, &image, &request).0.is_ok()); // calentamiento
    let mut times = Vec::new();
    for _ in 0..5 {
        let (reading, spent) = timed(&mut senses, &image, &request);
        let Ok(Reading::Pose(Some(keypoints))) = reading else {
            panic!("pose failed: {reading:?}")
        };
        assert_eq!(keypoints.len(), 17);
        let inside = keypoints
            .iter()
            .filter(|k| {
                k.score >= 0.5
                    && (bbox.x0..=bbox.x1).contains(&k.x)
                    && (bbox.y0..=bbox.y1).contains(&k.y)
            })
            .count();
        assert!(inside >= 5, "{keypoints:?}");
        times.push(spent.0);
        println!(
            "811 M-1: bbox={bbox:?}; inside={inside}; keypoints={keypoints:?}; spent_ms={:.3}",
            spent.0
        );
    }
    println!("811 M-1: five runs ms={times:?}");
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s-depth.onnx and ONNX Runtime"]
fn real_onnx_depth_at_points_and_its_scale() {
    use scene_geom::Point;
    use scene_look::DepthPurpose;
    use sense::{DepthPort, ultralytics::DepthEstimator};
    let models =
        std::path::PathBuf::from(std::env::var_os("CALIPSO_MODELS").expect("set CALIPSO_MODELS"));
    let image = image::load_from_memory(include_bytes!("fixtures/astronaut.jpg")).unwrap();
    let path = models.join("yolo26s-depth.onnx");
    let mut raw = DepthEstimator::load(&path).expect("load depth");
    let map = raw.estimate(&image).expect("depth map");
    let mut values: Vec<f32> = map
        .values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    values.sort_by(f32::total_cmp);
    println!(
        "812 M-1: image={}x{}; map={}x{}; finite={}/{}; min={:.4}; p50={:.4}; max={:.4}",
        image.width(),
        image.height(),
        map.width,
        map.height,
        values.len(),
        map.values.len(),
        values[0],
        values[values.len() / 2],
        values[values.len() - 1]
    );
    let points = vec![
        Point { x: 190.0, y: 260.0 }, // the astronaut's chest
        Point { x: 224.0, y: 124.0 }, // the nose (pose keypoint 0)
        Point { x: 5.0, y: 5.0 },     // a corner, the background
        Point {
            x: image.width() as f32,
            y: image.height() as f32,
        }, // the last edge
    ];
    let request = Probe::Depth {
        purpose: DepthPurpose::Bed,
        points,
        region: None,
    };
    let mut senses = Senses::new(NeverCalled).with_depth(DepthEstimator::load(&path).unwrap());
    assert!(timed(&mut senses, &image, &request).0.is_ok()); // calentamiento
    let mut times = Vec::new();
    for _ in 0..5 {
        let (reading, spent) = timed(&mut senses, &image, &request);
        let Ok(Reading::Depth(depths)) = reading else {
            panic!("depth failed: {reading:?}")
        };
        assert_eq!(depths.len(), 4);
        assert!(depths.iter().all(Option::is_some), "{depths:?}");
        times.push(spent.0);
        println!("812 M-1: depths={depths:?}; spent_ms={:.3}", spent.0);
    }
    println!("812 M-1: five runs ms={times:?}");
}
