use image::DynamicImage;
use scene_core::{
    geom::Rect,
    look::{Detection, Failure, Label, LookId, Probe, Reading, Request},
};
use sense::{DetectorPort, ModelError, Senses};

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
    let request = Request {
        id: LookId(0),
        probe: Probe::Reinforce {
            region: Rect::new(100.0, 50.0, 200.0, 150.0),
        },
    };
    let (result, spent) = Senses::new(NeverCalled)
        .with_reinforce(FakeDetector)
        .look(&DynamicImage::new_rgb8(320, 240), &request);
    let Ok(Reading::Bodies(boxes)) = result else {
        panic!("expected reinforcement bodies")
    };
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].bbox, Rect::new(110.0, 60.0, 130.0, 90.0));
    assert_eq!(boxes[0].label, Label::Other);
    assert_eq!(boxes[0].confidence, 0.9);
    assert!(spent.0 > 0.0);
}

#[test]
fn reinforce_without_port_never_calls_primary() {
    let request = Request {
        id: LookId(0),
        probe: Probe::Reinforce {
            region: Rect::new(0.0, 0.0, 10.0, 10.0),
        },
    };
    let (result, _) = Senses::new(NeverCalled).look(&DynamicImage::new_rgb8(100, 100), &request);
    assert_eq!(result, Err(Failure::Model));
}

#[test]
fn detect_returns_every_box_in_frame_coordinates() {
    let frame = DynamicImage::new_rgb8(320, 240);
    let request = Request {
        id: LookId(7),
        probe: Probe::Detect {
            region: Some(Rect::new(100.0, 50.0, 200.0, 150.0)),
        },
    };
    let (result, spent) = Senses::new(FakeDetector).look(&frame, &request);
    let Ok(Reading::Bodies(boxes)) = result else {
        panic!("expected bodies")
    };
    assert_eq!(boxes[0].bbox, Rect::new(110.0, 60.0, 130.0, 90.0));
    assert_eq!(boxes[0].label, Label::Other);
    assert!(spent.0 > 0.0);
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
            let (result, spent) = senses.look(
                &frame,
                &Request {
                    id: LookId(0),
                    probe,
                },
            );
            assert_eq!(result, Err(Failure::Model));
            assert!(spent.0 > 0.0);
        }
    }
}

#[test]
fn faces_preserve_local_boxes_scores_and_labels_in_both_regions() {
    use scene_core::look::FaceRegion;
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
        let (reading, _) = Senses::new(NeverCalled)
            .with_faces(Faces)
            .with_reinforce(NeverCalled)
            .look(
                &DynamicImage::new_rgb8(320, 240),
                &Request {
                    id: LookId(0),
                    probe: Probe::Faces { region },
                },
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
    use scene_core::look::FaceRegion;
    let frame = DynamicImage::new_rgb8(100, 100);
    let request = Request {
        id: LookId(0),
        probe: Probe::Faces {
            region: FaceRegion::Subject(Rect::new(200.0, 0.0, 300.0, 10.0)),
        },
    };
    assert_eq!(
        Senses::new(NeverCalled)
            .with_faces(NeverCalled)
            .look(&frame, &request)
            .0,
        Err(Failure::Model)
    );
    let request = Request {
        id: LookId(0),
        probe: Probe::Faces {
            region: FaceRegion::Headboard(Rect::new(0.0, 0.0, 100.0, 100.0)),
        },
    };
    assert_eq!(
        Senses::new(NeverCalled).look(&frame, &request).0,
        Err(Failure::Model)
    );
    struct Broken;
    impl DetectorPort for Broken {
        fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            Err(ModelError::Inference("fallo guionado".into()))
        }
    }
    assert_eq!(
        Senses::new(NeverCalled)
            .with_faces(Broken)
            .look(&frame, &request)
            .0,
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
    let request = Request {
        id: LookId(0),
        probe: Probe::Detect { region: None },
    };
    let (result, _) = Senses::new(Empty).look(&DynamicImage::new_rgb8(320, 240), &request);
    assert_eq!(result, Ok(Reading::Bodies(vec![])));
}

#[test]
fn model_failure_is_data_and_records_spent_time() {
    struct Broken;
    impl DetectorPort for Broken {
        fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
            Err(ModelError::Inference("boom".into()))
        }
    }
    let request = Request {
        id: LookId(0),
        probe: Probe::Detect { region: None },
    };
    let (result, spent) = Senses::new(Broken).look(&DynamicImage::new_rgb8(100, 100), &request);
    assert_eq!(result, Err(Failure::Model));
    assert!(spent.0 > 0.0);
}

#[test]
fn unsupported_requests_fail_and_warn_with_the_requested_kind() {
    use scene_core::look::{DepthPurpose, FaceRegion};
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
        Probe::Pose { bbox: rect },
        Probe::Depth {
            purpose: DepthPurpose::Subject,
            points: vec![],
        },
        Probe::Depth {
            purpose: DepthPurpose::Bed,
            points: vec![],
        },
    ] {
        let expected_kind = format!("probe={:?}", probe.kind());
        let request = Request {
            id: LookId(0),
            probe,
        };
        let (result, spent) = senses.look(&DynamicImage::new_rgb8(100, 100), &request);
        assert_eq!(result, Err(Failure::Model));
        assert!(spent.0 > 0.0);
        let mut captured = warnings.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(captured.pop().unwrap().contains(&expected_kind));
    }
}
