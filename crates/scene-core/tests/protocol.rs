//! Properties of the loop protocol, using only the public API.
use proptest::prelude::*;
use scene_core::contract::{FrameInfo, Keypoint, SampleRef, SensorState, SessionId, SourceSecs};
use scene_core::geom::Rect;
use scene_core::look::{Detection, Failure, Label, Reading, Spent};
use scene_core::{Input, NoFrame};

fn inputs() -> impl Strategy<Value = Input> {
    (
        0u8..8,
        0u64..5,
        0u32..500,
        prop_oneof![Just((640, 480)), Just((320, 240)), Just((0, 0))],
    )
        .prop_map(|(kind, session, seconds, (width, height))| {
            let at = SampleRef {
                session: SessionId(session),
                sequence: u64::from(seconds),
                source_secs: SourceSecs(f64::from(seconds)),
            };
            match kind {
                0 | 1 => Input::Sample {
                    at,
                    frame: FrameInfo { width, height },
                },
                2 => Input::NoFrame {
                    at,
                    why: NoFrame::DecodeFailed,
                },
                3 => Input::NoFrame {
                    at,
                    why: NoFrame::Empty,
                },
                state => Input::Sensor {
                    at,
                    state: match state {
                        4 => SensorState::Connecting,
                        5 => SensorState::Streaming,
                        6 => SensorState::Stalled,
                        _ => SensorState::Reconnecting,
                    },
                },
            }
        })
}

fn number() -> impl Strategy<Value = f32> {
    prop_oneof![
        (-1000i32..2000).prop_map(|n| n as f32),
        Just(f32::NAN),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY)
    ]
}

fn arbitrary_detections() -> impl Strategy<Value = Vec<Detection>> {
    prop::collection::vec(
        (
            number(),
            number(),
            number(),
            number(),
            number(),
            any::<bool>(),
        )
            .prop_map(|(x0, y0, x1, y1, confidence, person)| Detection {
                bbox: Rect::new(x0, y0, x1, y1),
                confidence,
                label: if person { Label::Person } else { Label::Other },
            }),
        0..8,
    )
}

fn detections() -> impl Strategy<Value = Vec<Detection>> {
    // Balance useful readings and adversarial tensors: Current provenance must
    // be exercised, besides the rejection/failure paths.
    let valid = prop::collection::vec(
        (0u8..3, 0u8..4).prop_map(|(zone, index)| {
            let x = match zone {
                0 => 150.0,
                1 => 505.0,
                _ => 550.0,
            };
            let y = 150.0 + f32::from(index) * 40.0;
            Detection {
                bbox: Rect::new(x, y, x + 20.0, y + 30.0),
                confidence: 0.9,
                label: Label::Person,
            }
        }),
        0..8,
    );
    prop_oneof![valid, arbitrary_detections()]
}

type Answer = (Result<Reading, Failure>, Spent);
type ScriptStep = (Input, Vec<Answer>);

fn responses() -> impl Strategy<Value = Answer> {
    let reading = prop_oneof![
        detections().prop_map(Reading::Bodies),
        detections().prop_map(Reading::Faces),
        prop::option::of(prop::collection::vec(
            (number(), number(), number()).prop_map(|(x, y, score)| Keypoint { x, y, score }),
            0..6
        ))
        .prop_map(Reading::Pose),
        prop::collection::vec(prop::option::of(number()), 0..6).prop_map(Reading::Depth),
    ];
    (
        prop_oneof![
            reading.prop_map(Ok),
            Just(Err(Failure::Model)),
            Just(Err(Failure::Cancelled))
        ],
        0u32..1000,
    )
        .prop_map(|(result, ms)| (result, Spent(f64::from(ms))))
}

fn sequences() -> impl Strategy<Value = Vec<ScriptStep>> {
    prop::collection::vec((inputs(), prop::collection::vec(responses(), 1..7)), 0..200)
}

use scene_core::contract::{BedState, Event, LookKind, LookResult, Provenance};
use scene_core::geom::Quad;
use scene_core::look::{LookId, Outcome, Probe, Request};
use scene_core::map::SiteMap;
use scene_core::policy::{CalibrationPolicy, EnrichPolicy, FacePolicy, Policy, ReinforcePolicy};
use scene_core::{FeedError, Monitor, Output, Step};

fn monitor() -> Monitor {
    let mut policy = Policy::default();
    policy.persistence.dwell_secs = 0.0;
    policy.faces = Some(FacePolicy {
        min_confidence: 0.5,
    });
    policy.reinforce = Some(ReinforcePolicy { bed_padding: 0.1 });
    policy.enrich = Some(EnrichPolicy {
        move_threshold: 0.05,
        depth: true,
        min_keypoint_score: 0.3,
        posture_tolerance: 0.05,
        max_pose_age_secs: None,
    });
    policy.calibration = Some(CalibrationPolicy { retry_secs: 2.0 });
    let mut map = SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0)));
    map.resolution = Some(FrameInfo {
        width: 640,
        height: 480,
    });
    map.headboard = Some(Rect::new(100.0, 100.0, 500.0, 180.0));
    Monitor::new(policy, map)
}

fn matching_empty(probe: &Probe) -> Reading {
    match probe {
        Probe::Detect { .. } | Probe::Reinforce { .. } => Reading::Bodies(vec![]),
        Probe::Faces { .. } => Reading::Faces(vec![]),
        Probe::Pose { .. } => Reading::Pose(None),
        Probe::Depth { .. } => Reading::Depth(vec![]),
    }
}

fn invariants(output: &Output) -> Result<(), TestCaseError> {
    let scene = &output.scene;
    if scene.bed.state == BedState::Held {
        prop_assert_eq!(scene.bed.seen, 0);
        prop_assert!(!scene.bed.held.is_empty());
    }
    if scene.bed.state == BedState::Seen {
        prop_assert!(scene.bed.seen > 0);
    }
    let read = |kinds: &[LookKind]| {
        scene.attention.looks.iter().any(|look| {
            kinds.contains(&look.kind) && matches!(look.result(), Some(LookResult::Read { .. }))
        })
    };
    if let Some(subject) = &scene.subject {
        if subject.provenance == Provenance::Current {
            prop_assert!(scene.republished.is_none());
            prop_assert!(read(&[LookKind::Detect, LookKind::Reinforce]));
        }
        if subject
            .pose
            .as_ref()
            .is_some_and(|p| p.provenance == Provenance::Current)
        {
            prop_assert!(read(&[LookKind::Pose]));
        }
        if subject
            .depth
            .is_some_and(|d| d.provenance == Provenance::Current)
        {
            prop_assert!(read(&[LookKind::Depth]));
        }
    }
    // Rule 4: sensor → layer 0 → map → derived.
    let rank = |event: &Event| match event {
        Event::Sensor(_) => 0,
        Event::Transition(_) => 1,
        Event::Map(_) => 2,
        Event::Derived(_) => 3,
    };
    prop_assert!(
        output
            .events
            .windows(2)
            .all(|pair| rank(&pair[0]) <= rank(&pair[1])),
        "events out of order: {:?}",
        output.events
    );
    Ok(())
}

proptest! {
    #[test]
    fn pr1_the_core_does_not_panic(sequence in sequences()) {
        let result = std::panic::catch_unwind(|| {
            let mut monitor = monitor();
            for (input,answers) in sequence {
                let mut answers = answers.iter().cycle();
                monitor.handle(input).drive(|_| answers.next().unwrap().clone());
            }
        });
        prop_assert!(result.is_ok(), "the core panicked");
    }

    #[test]
    fn pr2_every_turn_closes(sequence in sequences()) {
        let mut monitor = monitor();
        for (input,answers) in sequence {
            let mut answers = answers.iter().cycle();
            let at = match &input { Input::Sample { at,.. } | Input::NoFrame { at,.. } | Input::Sensor { at,.. } => *at };
            let output = monitor.handle(input).drive(|_| answers.next().unwrap().clone());
            prop_assert_eq!(output.at,at);
        }
    }

    #[test]
    fn pr3_a_wrong_feed_is_rejected_and_the_turn_stays_usable(sequence in sequences(), bogus in any::<u32>()) {
        let mut monitor = monitor();
        // At least one look is tested even if the generated sequence has no samples.
        let first = Input::Sample { at: SampleRef { session: SessionId(1),sequence: 0,source_secs: SourceSecs(0.0) },frame: FrameInfo { width: 640,height: 480 } };
        for input in std::iter::once(first).chain(sequence.into_iter().map(|(input,_)|input)) {
            let mut turn = monitor.handle(input);
            let mut checks = Vec::new();
            while let Step::Look(request) = turn.poll() {
                let request: Request = request.clone();
                {
                    let foreign = LookId(if request.id == LookId(bogus) { request.id.0 ^ u32::MAX } else { bogus });
                    let rejected_id = turn.feed(Outcome { id: foreign,result: Err(Failure::Model),spent: Spent(0.0) });
                    let wrong = if request.probe.accepts(&Reading::Bodies(vec![])) { Reading::Faces(vec![]) } else { Reading::Bodies(vec![]) };
                    let expected = scene_core::contract::ProbeKindMismatch { expected: request.probe.kind(),got: wrong.kind() };
                    let rejected_kind = turn.feed(Outcome { id: request.id,result: Ok(wrong),spent: Spent(0.0) });
                    let same_look = matches!(turn.poll(), Step::Look(pending) if *pending == request);
                    let accepted = turn.feed(Outcome { id: request.id,result: Ok(matching_empty(&request.probe)),spent: Spent(1.0) });
                    let duplicate = turn.feed(Outcome { id: request.id,result: Err(Failure::Cancelled),spent: Spent(0.0) });
                    checks.push((rejected_id == Err(FeedError::UnknownId(foreign)),rejected_kind == Err(FeedError::KindMismatch(expected)),same_look,accepted.is_ok(),duplicate == Err(FeedError::Duplicate(request.id))));
                }
            }
            // Close before the assertions: no test path abandons Turn.
            let output = turn.finish().unwrap();
            for check in checks { prop_assert_eq!(check,(true,true,true,true,true)); }
            prop_assert!(output.scene.attention.looks.iter().filter_map(|look|look.result()).all(|result| matches!(result,LookResult::Read { .. })), "an invalid or duplicate answer was kept");
        }
    }

    #[test]
    fn pr4_scenes_respect_provenance_and_order(sequence in sequences()) {
        let mut monitor = monitor();
        for (input,answers) in sequence {
            let mut answers = answers.iter().cycle();
            let output = monitor.handle(input).drive(|_| answers.next().unwrap().clone());
            invariants(&output)?;
        }
    }
}
