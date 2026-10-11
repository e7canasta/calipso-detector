//! The loop protocol, through the public API: `poll`, `feed`, `finish`,
//! `drive` and `cancel`.
use scene_core::contract::{
    BedState, LookResult, Mode, RepublishReason, SampleRef, SessionId, SourceSecs,
};
use scene_core::geom::{Quad, Rect};
use scene_core::look::{Detection, Failure, Label, LookId, Outcome, Reading, Request, Spent};
use scene_core::map::SiteMap;
use scene_core::policy::Policy;
use scene_core::{FeedError, FrameInfo, Incomplete, Input, Monitor, Step, Turn};

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

fn empty(id: LookId) -> Outcome {
    Outcome {
        id,
        result: Ok(Reading::Bodies(vec![])),
        spent: Spent(1.0),
    }
}

fn first_request(turn: &mut Turn<'_>) -> Request {
    match turn.poll() {
        Step::Look(request) => request.clone(),
        Step::Done => panic!("a probe was expected"),
    }
}

#[test]
fn poll_before_the_answer_returns_the_same_look() {
    let mut m = monitor();
    let mut turn = m.handle(sample(1.0));
    let a = first_request(&mut turn);
    let b = first_request(&mut turn);
    assert_eq!(a, b);
    turn.feed(empty(a.id)).unwrap();
    assert_eq!(turn.poll(), Step::Done);
    turn.finish().unwrap();
}

#[test]
fn an_id_foreign_to_the_pending_look_changes_nothing() {
    let mut m = monitor();
    let mut turn = m.handle(sample(1.0));
    let r = first_request(&mut turn);
    assert_eq!(
        turn.feed(empty(LookId(999))),
        Err(FeedError::UnknownId(LookId(999)))
    );
    assert_eq!(first_request(&mut turn), r);
    turn.feed(empty(r.id)).unwrap();
    let _ = turn.poll();
    turn.finish().unwrap();
}

#[test]
fn a_repeated_result_is_rejected_and_the_first_is_kept() {
    let mut m = monitor();
    let mut turn = m.handle(sample(1.0));
    let r = first_request(&mut turn);
    turn.feed(empty(r.id)).unwrap();
    let repeated = Outcome {
        result: Err(Failure::Cancelled),
        ..empty(r.id)
    };
    assert_eq!(turn.feed(repeated), Err(FeedError::Duplicate(r.id)));
    let _ = turn.poll();
    let out = turn.finish().unwrap();
    assert!(matches!(
        out.scene.attention.looks[0].result(),
        Some(LookResult::Read { .. })
    ));
}

#[test]
fn a_reading_of_another_kind_is_rejected() {
    let mut m = monitor();
    let mut turn = m.handle(sample(1.0));
    let r = first_request(&mut turn);
    let faces = Outcome {
        result: Ok(Reading::Faces(vec![])),
        ..empty(r.id)
    };
    assert!(matches!(turn.feed(faces), Err(FeedError::KindMismatch(_))));
    turn.feed(empty(r.id)).unwrap();
    let _ = turn.poll();
    turn.finish().unwrap();
}

#[test]
fn early_finish_returns_the_turn() {
    let mut m = monitor();
    let turn = m.handle(sample(1.0));
    let Err(Incomplete { mut turn }) = turn.finish() else {
        panic!("Incomplete was expected");
    };
    let r = first_request(&mut turn);
    turn.feed(empty(r.id)).unwrap();
    let _ = turn.poll();
    (*turn).finish().unwrap();
}

#[test]
fn a_shutdown_closes_the_turn_without_moving_the_belief() {
    let mut m = monitor();
    for s in 1..=3 {
        let mut turn = m.handle(sample(f64::from(s)));
        let r = first_request(&mut turn);
        turn.feed(Outcome {
            result: Ok(Reading::Bodies(vec![Detection {
                bbox: Rect::new(200.0, 150.0, 300.0, 350.0),
                confidence: 0.8,
                label: Label::Person,
            }])),
            ..empty(r.id)
        })
        .unwrap();
        let _ = turn.poll();
        turn.finish().unwrap();
    }
    let mut turn = m.handle(sample(4.0));
    let r = first_request(&mut turn);
    turn.feed(Outcome {
        result: Err(Failure::Cancelled),
        ..empty(r.id)
    })
    .unwrap();
    let _ = turn.poll();
    let out = turn.finish().unwrap();
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
