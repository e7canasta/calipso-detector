use image::DynamicImage;
use scene_geom::Rect;
use scene_look::{Detection, Failure, Label, Probe, Reading};
use sense::{DetectorPort, ModelError, Senses};

/// Answers a sense probe through its view. The answer carries no time: the
/// shell's router measures it (ADR 2026-11-02, quien-responde-cada-probe).
fn look(senses: &mut Senses, frame: &DynamicImage, probe: &Probe) -> Result<Reading, Failure> {
    senses.look(frame, &probe.sense().expect("a sense probe"))
}

struct FakeDetector;
impl DetectorPort for FakeDetector {
    fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
        assert_eq!((image.width(), image.height()), (100, 100));
        Ok(vec![Detection {
            bbox: Rect::new(10.0, 10.0, 30.0, 40.0),
            confidence: 0.9,
            label: Label::Other,
        }])
    }
}

struct NeverCalled;
impl DetectorPort for NeverCalled {
    fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
        panic!("wrong detector called")
    }
}

#[test]
fn reinforce_uses_its_own_port_and_maps_every_box() {
    let request = Probe::Reinforce {
        region: Rect::new(100.0, 50.0, 200.0, 150.0),
    };
    let mut senses = Senses::new(NeverCalled).with_reinforce(FakeDetector);
    let result = look(&mut senses, &DynamicImage::new_rgb8(320, 240), &request);
    let Ok(Reading::Bodies(boxes)) = result else {
        panic!("expected reinforcement bodies")
    };
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].bbox, Rect::new(110.0, 60.0, 130.0, 90.0));
    assert_eq!(boxes[0].label, Label::Other);
    assert_eq!(boxes[0].confidence, 0.9);
}

#[test]
fn reinforce_without_port_never_calls_primary() {
    let request = Probe::Reinforce {
        region: Rect::new(0.0, 0.0, 10.0, 10.0),
    };
    let result = look(
        &mut Senses::new(NeverCalled),
        &DynamicImage::new_rgb8(100, 100),
        &request,
    );
    assert_eq!(result, Err(Failure::Model));
}

#[test]
fn detect_returns_every_box_in_frame_coordinates() {
    let frame = DynamicImage::new_rgb8(320, 240);
    let request = Probe::Detect {
        region: Some(Rect::new(100.0, 50.0, 200.0, 150.0)),
    };
    let result = look(&mut Senses::new(FakeDetector), &frame, &request);
    let Ok(Reading::Bodies(boxes)) = result else {
        panic!("expected bodies")
    };
    assert_eq!(boxes[0].bbox, Rect::new(110.0, 60.0, 130.0, 90.0));
    assert_eq!(boxes[0].label, Label::Other);
}

#[test]
fn empty_regions_fail_without_calling_any_model() {
    let frame = DynamicImage::new_rgb8(100, 100);
    for region in [
        Rect::new(200.0, 0.0, 300.0, 20.0),
        Rect::new(10.0, 10.0, 10.0, 20.0),
    ] {
        for probe in [
            Probe::Detect {
                region: Some(region),
            },
            Probe::Reinforce { region },
        ] {
            let mut senses = Senses::new(NeverCalled).with_reinforce(NeverCalled);
            assert_eq!(look(&mut senses, &frame, &probe), Err(Failure::Model));
        }
    }
}

#[test]
fn faces_preserve_local_boxes_scores_and_labels_in_both_regions() {
    use scene_look::FaceRegion;
    struct Faces;
    impl DetectorPort for Faces {
        fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            assert_eq!((image.width(), image.height()), (100, 100));
            Ok(vec![Detection {
                bbox: Rect::new(10.0, 10.0, 20.0, 20.0),
                confidence: 0.05,
                label: Label::Other,
            }])
        }
    }
    let rect = Rect::new(100.0, 50.0, 200.0, 150.0);
    for region in [FaceRegion::Subject(rect), FaceRegion::Headboard(rect)] {
        let reading = look(
            &mut Senses::new(NeverCalled)
                .with_faces(Faces)
                .with_reinforce(NeverCalled),
            &DynamicImage::new_rgb8(320, 240),
            &Probe::Faces { region },
        );
        assert_eq!(
            reading,
            Ok(Reading::Faces(vec![Detection {
                bbox: Rect::new(110.0, 60.0, 120.0, 70.0),
                confidence: 0.05,
                label: Label::Other
            }]))
        );
    }
}

#[test]
fn faces_fail_without_model_for_empty_crop_or_model_error() {
    use scene_look::FaceRegion;
    let frame = DynamicImage::new_rgb8(100, 100);
    let request = Probe::Faces {
        region: FaceRegion::Subject(Rect::new(200.0, 0.0, 300.0, 10.0)),
    };
    assert_eq!(
        look(
            &mut Senses::new(NeverCalled).with_faces(NeverCalled),
            &frame,
            &request
        ),
        Err(Failure::Model)
    );
    let request = Probe::Faces {
        region: FaceRegion::Headboard(Rect::new(0.0, 0.0, 100.0, 100.0)),
    };
    assert_eq!(
        look(&mut Senses::new(NeverCalled), &frame, &request),
        Err(Failure::Model)
    );
    struct Broken;
    impl DetectorPort for Broken {
        fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            Err(ModelError::Inference("fallo guionado".into()))
        }
    }
    assert_eq!(
        look(
            &mut Senses::new(NeverCalled).with_faces(Broken),
            &frame,
            &request
        ),
        Err(Failure::Model)
    );
}

#[test]
fn full_frame_and_empty_result_are_successful() {
    struct Empty;
    impl DetectorPort for Empty {
        fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            assert_eq!((image.width(), image.height()), (320, 240));
            Ok(vec![])
        }
    }
    let request = Probe::Detect { region: None };
    let result = look(
        &mut Senses::new(Empty),
        &DynamicImage::new_rgb8(320, 240),
        &request,
    );
    assert_eq!(result, Ok(Reading::Bodies(vec![])));
}

#[test]
fn model_failure_is_data() {
    struct Broken;
    impl DetectorPort for Broken {
        fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            Err(ModelError::Inference("boom".into()))
        }
    }
    let request = Probe::Detect { region: None };
    let result = look(
        &mut Senses::new(Broken),
        &DynamicImage::new_rgb8(100, 100),
        &request,
    );
    assert_eq!(result, Err(Failure::Model));
}

#[test]
fn unsupported_requests_fail_and_warn_with_the_requested_kind() {
    use scene_look::{DepthPurpose, FaceRegion, PosePurpose};
    use std::sync::{Arc, Mutex};
    use tracing::{
        Event, Metadata, Subscriber,
        field::{Field, Visit},
        span::{Attributes, Id, Record},
    };

    struct Warnings {
        thread: std::thread::ThreadId,
        captured: Arc<Mutex<Vec<String>>>,
    }
    impl Subscriber for Warnings {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
            Some(tracing::level_filters::LevelFilter::WARN)
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            if std::thread::current().id() != self.thread {
                return;
            }
            assert_eq!(*event.metadata().level(), tracing::Level::WARN);
            struct Fields(String);
            impl Visit for Fields {
                fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                    self.0.push_str(&format!("{field}={value:?};"));
                }
            }
            let mut fields = Fields(String::new());
            event.record(&mut fields);
            self.captured.lock().unwrap().push(fields.0);
        }
    }
    struct NeverCalled;
    impl DetectorPort for NeverCalled {
        fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            panic!("unsupported request called primary detector")
        }
    }
    let warnings = Arc::new(Mutex::new(Vec::new()));
    // The global dispatch keeps the shared callsite interest; only this thread's events are captured.
    tracing::subscriber::set_global_default(Warnings {
        thread: std::thread::current().id(),
        captured: warnings.clone(),
    })
    .unwrap();
    let mut senses = Senses::new(NeverCalled);
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    for probe in [
        Probe::Faces {
            region: FaceRegion::Subject(rect),
        },
        Probe::Reinforce { region: rect },
        Probe::Pose {
            bbox: rect,
            purpose: PosePurpose::Subject,
        },
        Probe::Depth {
            purpose: DepthPurpose::Subject,
            points: vec![],
            region: None,
        },
        Probe::Depth {
            purpose: DepthPurpose::Bed,
            points: vec![],
            region: None,
        },
    ] {
        let expected_kind = format!("probe={:?}", probe.kind());
        let result = look(&mut senses, &DynamicImage::new_rgb8(100, 100), &probe);
        assert_eq!(result, Err(Failure::Model));
        let mut captured = warnings.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(captured.pop().unwrap().contains(&expected_kind));
    }
}
