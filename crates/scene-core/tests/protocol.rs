//! The loop protocol, through the public API: `handle` opens a turn and
//! `drive` or `cancel` closes it. There is no other way through.
use scene_contract::{
    BedState, LookResult, Mode, RepublishReason, SampleRef, SessionId, SourceSecs,
};
use scene_core::map::{FrameInfo, SiteMap};
use scene_core::policy::Policy;
use scene_core::{Input, Monitor};
use scene_geom::{Quad, Rect};
use scene_look::{Detection, Failure, Label, Reading, Spent};

fn monitor() -> Monitor {
    Monitor::new(
        Policy::default(),
        SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))),
    )
}

fn sample(secs: f64) -> Input {
    Input::Sample {
        at: SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(secs),
        },
        frame: FrameInfo {
            width: 640,
            height: 480,
        },
    }
}

#[test]
fn a_shutdown_closes_the_turn_without_moving_the_belief() {
    let mut m = monitor();
    for s in 1..=3 {
        let _ = m.handle(sample(f64::from(s))).drive(|_| {
            let body = Detection {
                bbox: Rect::new(200.0, 150.0, 300.0, 350.0),
                confidence: 0.8,
                label: Label::Person,
            };
            (Ok(Reading::Bodies(vec![body])), Spent(1.0))
        });
    }
    let out = m
        .handle(sample(4.0))
        .drive(|_| (Err(Failure::Cancelled), Spent(1.0)));
    assert_eq!(out.scene.room.occupants, 1);
    assert_eq!(out.scene.bed.state, BedState::Seen);
    assert!(out.events.is_empty());
}

#[test]
fn drive_runs_the_whole_turn() {
    let mut m = monitor();
    let mut probes = 0;
    let out = m.handle(sample(1.0)).drive(|_| {
        probes += 1;
        (Ok(Reading::Bodies(vec![])), Spent(1.0))
    });
    assert_eq!(probes, 1);
    assert_eq!(out.scene.room.mode, Mode::Empty);
}

#[test]
fn a_reading_of_another_kind_in_drive_is_a_failing_sense() {
    let mut m = monitor();
    let out = m
        .handle(sample(1.0))
        .drive(|_| (Ok(Reading::Faces(vec![])), Spent(1.0)));
    assert!(matches!(
        out.scene.attention.looks[0].result(),
        Some(LookResult::Failed { .. })
    ));
}

#[test]
fn a_detector_failure_declares_the_last_observation() {
    let mut m = monitor();
    let _ = m
        .handle(sample(1.0))
        .drive(|_| (Ok(Reading::Bodies(vec![])), Spent(1.0)));
    let out = m
        .handle(sample(2.0))
        .drive(|_| (Ok(Reading::Faces(vec![])), Spent(1.0)));
    let republished = out
        .scene
        .republished
        .expect("the turn is declared republished");
    assert_eq!(republished.reason, RepublishReason::DetectFailed);
    assert_eq!(
        republished.observed_at.map(|at| at.source_secs),
        Some(SourceSecs(1.0))
    );
}

#[test]
fn cancel_closes_the_turn_like_a_shutdown() {
    let mut m = monitor();
    let out = m.handle(sample(1.0)).cancel();
    assert!(matches!(
        out.scene.attention.looks[0].result(),
        Some(LookResult::Failed { .. })
    ));
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "rule 10")]
fn abandoning_a_turn_is_an_error() {
    let mut m = monitor();
    let turn = m.handle(sample(1.0));
    drop(turn);
}
