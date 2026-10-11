//! Spec 209 properties of the room/bed rules, only through the public API.
use proptest::prelude::*;
use scene_core::geom::Rect;
use scene_core::look::{Detection, Failure, Label};

#[derive(Debug, Clone)]
struct Step {
    bed: u8,
    near: u8,
    far: u8,
    failure: Option<Failure>,
    repeat: u8,
    reconnection: bool,
}

impl Step {
    fn bodies(&self) -> Result<Vec<Detection>, Failure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let mut detections = Vec::new();
        for (count, x, y) in [
            (self.bed, 150.0, 150.0),
            (self.near, 505.0, 150.0),
            (self.far, 550.0, 20.0),
        ] {
            for index in 0..count {
                let offset = f32::from(index) * 45.0;
                detections.push(Detection {
                    bbox: Rect::new(x, y + offset, x + 20.0, y + offset + 30.0),
                    confidence: 0.9,
                    label: Label::Person,
                });
            }
        }
        Ok(detections)
    }
}

fn steps() -> impl Strategy<Value = Step> {
    (0u8..4,0u8..4,0u8..4,prop_oneof![3 => Just(None),1 => Just(Some(Failure::Model)),1 => Just(Some(Failure::Cancelled))],1u8..5,any::<bool>())
        .prop_map(|(bed,near,far,failure,repeat,reconnection)| Step { bed,near,far,failure,repeat,reconnection })
}

fn sequences() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec(steps(), 0..200)
}

use scene_core::contract::{
    BedState, EventBody, EventKind, FrameInfo, SampleRef, Scene, SessionId, SourceSecs,
};
use scene_core::geom::Quad;
use scene_core::look::{Reading, Spent};
use scene_core::map::SiteMap;
use scene_core::policy::Policy;
use scene_core::{Input, Monitor, NoFrame, Output};

fn monitor() -> Monitor {
    Monitor::new(
        Policy::default(),
        SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))),
    )
}

fn sample(session: u64, sequence: u64, seconds: u32) -> Input {
    Input::Sample {
        at: SampleRef {
            session: SessionId(session),
            sequence,
            source_secs: SourceSecs(f64::from(seconds)),
        },
        frame: FrameInfo {
            width: 640,
            height: 480,
        },
    }
}

fn counts(scene: &Scene) -> (u32, u32, usize, u32) {
    (
        scene.bed.seen,
        scene.floor.seen,
        scene.bed.held.len(),
        scene.room.occupants,
    )
}

fn occupation_events(output: &Output) -> impl Iterator<Item = &scene_core::contract::Event> {
    output.events.iter().filter(|event| {
        matches!(
            event.body,
            EventBody::Entered
                | EventBody::LeftRoom
                | EventBody::LayDown
                | EventBody::LeftBed
                | EventBody::HeldInBed { .. }
                | EventBody::Reappeared { .. }
                | EventBody::RoomChanged { .. }
                | EventBody::BedChanged { .. }
        )
    })
}

fn run(
    sequence: &[Step],
    mut check: impl FnMut(&Scene, &Output, &Step) -> Result<(), TestCaseError>,
) -> Result<(), TestCaseError> {
    let mut monitor = monitor();
    let mut previous = monitor
        .handle(Input::NoFrame {
            at: SampleRef {
                session: SessionId(1),
                sequence: 0,
                source_secs: SourceSecs(0.0),
            },
            why: NoFrame::Empty,
        })
        .cancel();
    let mut session = 1;
    let mut seconds = 0;
    let mut index = 0;
    for step in sequence {
        if step.reconnection {
            session += 1;
            seconds = 0;
        }
        for _ in 0..step.repeat {
            index += 1;
            seconds += 1;
            let output = monitor
                .handle(sample(session, index, seconds))
                .drive(|_| (step.bodies().map(Reading::Bodies), Spent(1.0)));
            check(&previous.scene, &output, step)?;
            previous = output;
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn re1_occupants_are_the_sum_of_seen_and_held(sequence in sequences()) {
        run(&sequence,|_,output,_| {
            let scene = &output.scene;
            prop_assert_eq!(scene.room.occupants,scene.floor.seen+scene.bed.seen+scene.bed.held.len() as u32);
            Ok(())
        })?;
    }

    #[test]
    fn re2_the_bed_does_not_empty_without_an_exit(sequence in sequences()) {
        run(&sequence,|before,output,_| {
            if matches!(before.bed.state,BedState::Seen | BedState::Held) && output.scene.bed.state == BedState::Empty {
                prop_assert!(output.events.iter().any(|event| matches!(event.body,EventBody::LeftBed)), "bed emptied without an exit: before={:?}, after={:?}",before.bed,output.scene.bed);
            }
            Ok(())
        })?;
    }

    #[test]
    fn re3_a_held_occupant_is_only_released_on_reappearing(sequence in sequences()) {
        run(&sequence,|before,output,_| {
            let after = &output.scene.bed.held;
            let released = before.bed.held.iter().filter(|held| !after.contains(held)).count();
            let reappeared = output.events.iter().filter(|event| matches!(event.body,EventBody::Reappeared { .. })).count();
            prop_assert!(released <= reappeared,"held released={released}, reappeared={reappeared}; before={:?}, after={:?}",before.bed.held,after);
            Ok(())
        })?;
    }

    #[test]
    fn re4_a_failure_changes_neither_counts_nor_facts(sequence in sequences()) {
        run(&sequence,|before,output,step| {
            if step.failure.is_some() {
                prop_assert_eq!(counts(before),counts(&output.scene));
                prop_assert!(occupation_events(output).next().is_none(),"occupancy facts during a failure: {:?}",output.events);
            }
            Ok(())
        })?;
    }

    #[test]
    fn re5_changing_session_neither_alters_counts_nor_creates_facts(sequence in sequences()) {
        // A reconnection does not change the belief: the first reading of the new
        // session keeps what was confirmed (counts, states, held) and creates no facts.
        // A pending change does not cross sessions: it restarts from one, so
        // confirmation may arrive later than without a reconnection.
        let mut monitor = monitor();
        let mut previous: Option<std::sync::Arc<Scene>> = None;
        let mut session = 1;
        let mut seconds = 0;
        let mut index = 0;
        let mut session_without_readings = false;
        for step in &sequence {
            if step.reconnection { session += 1; seconds = 0; session_without_readings = true; }
            for _ in 0..step.repeat {
                index += 1;
                seconds += 1;
                let output = monitor.handle(sample(session,index,seconds)).drive(|_| (step.bodies().map(Reading::Bodies),Spent(1.0)));
                let first_reading = session_without_readings && step.failure.is_none();
                if first_reading { session_without_readings = false; }
                if let (Some(before), true) = (&previous, first_reading) {
                    let held = |scene: &Scene| scene.bed.held.iter().map(|held| held.bbox).collect::<Vec<_>>();
                    prop_assert_eq!(counts(before),counts(&output.scene));
                    prop_assert_eq!(before.bed.state,output.scene.bed.state);
                    prop_assert_eq!(before.room.state,output.scene.room.state);
                    prop_assert_eq!(held(before),held(&output.scene));
                    prop_assert_eq!(occupation_events(&output).count(),0);
                    if let Some(pending) = &output.scene.room.pending {
                        prop_assert_eq!(pending.duration.have,0.0);
                    }
                }
                previous = Some(output.scene);
            }
        }
    }

    #[test]
    fn re6_the_last_domain_after_matches_the_scene(sequence in sequences()) {
        run(&sequence,|_,output,_| {
            if let Some(last) = output.events.iter().rev().find(|event|event.kind == EventKind::Domain) {
                let scene = &output.scene;
                prop_assert_eq!((last.after.bed_seen,last.after.floor_seen,last.after.bed_held as usize,last.after.occupants),counts(scene),"last fact={:?}",last);
            }
            Ok(())
        })?;
    }
}
