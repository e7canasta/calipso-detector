//! Properties of the room and bed rules, only through the public API.
// `type_name` is for the BDD steps.
#[allow(dead_code)]
mod published;

use proptest::prelude::*;
use published::{Body, Fact, Kind};
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

use scene_contract::{BedPostureState, Mode};
use scene_contract::{BedState, FrameInfo, SampleRef, Scene, SessionId, SourceSecs};
use scene_core::geom::Quad;
use scene_core::look::{Reading, Spent};
use scene_core::map::SiteMap;
use scene_core::policy::{BedPosturePolicy, Policy};
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

/// What the turn published about occupancy: the layer-0 facts and the
/// derived changes, without the sensor or the map.
fn occupation_events(output: &Output) -> Vec<Fact> {
    published::facts(output)
        .into_iter()
        .filter(|fact| {
            !matches!(
                fact.body,
                Body::SensorChanged { .. } | Body::BedCalibrated { .. }
            )
        })
        .collect()
}

/// A monitor that also follows layer 1, the bed posture, each state counting
/// at once: its invariants against layer 0 hold on every turn.
fn monitor_with_bed_posture() -> Monitor {
    let policy = Policy {
        bed_posture: Some(BedPosturePolicy::default()),
        ..Policy::default()
    };
    Monitor::new(
        policy,
        SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))),
    )
}

fn alone_in_bed(scene: &Scene) -> bool {
    matches!(scene.room.mode, Mode::AloneInBedSeen | Mode::AloneInBedHeld)
}

fn bed_posture(scene: &Scene) -> Option<BedPostureState> {
    scene
        .subject
        .as_ref()
        .and_then(|s| s.bed_posture.as_ref())
        .map(|b| b.state)
}

fn run(
    sequence: &[Step],
    check: impl FnMut(&Scene, &Output, &Step) -> Result<(), TestCaseError>,
) -> Result<(), TestCaseError> {
    run_on(monitor(), sequence, check)
}

fn run_on(
    mut monitor: Monitor,
    sequence: &[Step],
    mut check: impl FnMut(&Scene, &Output, &Step) -> Result<(), TestCaseError>,
) -> Result<(), TestCaseError> {
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
            published::check(&output);
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
                prop_assert!(published::facts(output).iter().any(|fact| matches!(fact.body,Body::LeftBed)), "bed emptied without an exit: before={:?}, after={:?}",before.bed,output.scene.bed);
            }
            Ok(())
        })?;
    }

    #[test]
    fn re3_a_held_occupant_is_only_released_on_reappearing(sequence in sequences()) {
        run(&sequence,|before,output,_| {
            let after = &output.scene.bed.held;
            let released = before.bed.held.iter().filter(|held| !after.contains(held)).count();
            let reappeared = published::facts(output).iter().filter(|fact| matches!(fact.body,Body::Reappeared { .. })).count();
            prop_assert!(released <= reappeared,"held released={released}, reappeared={reappeared}; before={:?}, after={:?}",before.bed.held,after);
            Ok(())
        })?;
    }

    #[test]
    fn re4_a_failure_changes_neither_counts_nor_facts(sequence in sequences()) {
        run(&sequence,|before,output,step| {
            if step.failure.is_some() {
                prop_assert_eq!(counts(before),counts(&output.scene));
                prop_assert!(occupation_events(output).is_empty(),"occupancy facts during a failure: {:?}",output.events);
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
                    prop_assert_eq!(before.room.mode,output.scene.room.mode);
                    prop_assert_eq!(held(before),held(&output.scene));
                    prop_assert_eq!(occupation_events(&output).len(),0);
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
            if let Some(last) = published::facts(output).into_iter().rev().find(|fact|fact.kind == Kind::Domain) {
                let scene = &output.scene;
                prop_assert_eq!((last.after.bed_seen,last.after.floor_seen,last.after.bed_held as usize,last.after.occupants),counts(scene),"last fact={:?}",last);
            }
            Ok(())
        })?;
    }

    /// Layer 1 lives exactly while layer 0 has the resident alone in bed:
    /// `c = 1 ⇔ rbh ∈ {110, 111}` (white book, volume II, ch. 05).
    #[test]
    fn re7_layer_1_lives_exactly_while_the_resident_is_alone_in_bed(sequence in sequences()) {
        run_on(monitor_with_bed_posture(), &sequence, |_, output, _| {
            let scene = &output.scene;
            if bed_posture(scene).is_some() {
                prop_assert!(alone_in_bed(scene), "bed posture without the resident alone in bed: {:?}", scene.room.mode);
            }
            if alone_in_bed(scene) && scene.subject.is_some() {
                prop_assert!(bed_posture(scene).is_some(), "the resident alone in bed without a bed posture");
            }
            Ok(())
        })?;
    }

    /// Held in bed is lying: `rbh = 111 → cyz = 110`.
    #[test]
    fn re8_held_in_bed_is_lying(sequence in sequences()) {
        run_on(monitor_with_bed_posture(), &sequence, |_, output, _| {
            let scene = &output.scene;
            if scene.room.mode == Mode::AloneInBedHeld {
                prop_assert_eq!(bed_posture(scene), Some(BedPostureState::Lying));
            }
            Ok(())
        })?;
    }

    /// Entering the bed is not reported: from nothing or `in_bed`, layer 1
    /// only goes to lying (`y = 0` never jumps to risk 1 or 2).
    #[test]
    fn re9_entering_the_bed_is_not_reported(sequence in sequences()) {
        run_on(monitor_with_bed_posture(), &sequence, |before, output, _| {
            if matches!(bed_posture(before), None | Some(BedPostureState::InBed)) {
                prop_assert!(!matches!(bed_posture(&output.scene), Some(BedPostureState::SittingUp | BedPostureState::GettingOut)),
                    "from {:?} to {:?}", bed_posture(before), bed_posture(&output.scene));
            }
            Ok(())
        })?;
    }
}
