//! Metamorphic tests: with no per-sample oracle, the night
//! is compared with itself under transformations that must not change what the
//! engine believes. They work the same for written scenes and for distilled
//! nights, which bring no expectations.
#![allow(clippy::disallowed_methods)]

use golden::language::{InBed, Scene, Sense};
use golden::world::{self, Sample};
use scene_core::contract::{
    BedState, Cause, EventBody, RoomState, SampleRef, SessionId, SourceSecs,
};
use scene_core::geom::Quad;
use scene_core::look::{Failure, Probe, Spent};
use scene_core::map::SiteMap;
use scene_core::policy::Policy;
use scene_core::{Input, Monitor};
use std::mem::Discriminant;

/// What is compared: the sequence of facts (without their sample) and the final belief.
#[derive(Debug, PartialEq)]
struct Night {
    facts: Vec<(Discriminant<EventBody>, Cause)>,
    end: (RoomState, u32, BedState, usize, u32),
}

fn site() -> SiteMap {
    let mut map = SiteMap::new(Quad::from_rect(world::BED));
    map.resolution = Some(world::FRAME);
    map.headboard = Some(world::HEADBOARD);
    map
}

/// Drives the night; `failure` says in which sample the main detector fails.
fn run(samples: &[Sample], failure: Option<u32>) -> Night {
    let mut monitor = Monitor::new(Policy::default(), site());
    let mut facts = Vec::new();
    let mut last = None;
    for m in samples {
        let at = SampleRef {
            session: SessionId(1),
            sequence: u64::from(m.n),
            source_secs: SourceSecs(m.source_secs),
        };
        let out = monitor
            .handle(Input::Sample {
                at,
                frame: world::FRAME,
            })
            .drive(|r| {
                let answer = match (&r.probe, failure) {
                    (Probe::Detect { .. }, Some(n)) if n == m.n => Err(Failure::Model),
                    _ => world::respond(m, &r.probe),
                };
                (answer, Spent(0.0))
            });
        facts.extend(
            out.events
                .iter()
                .map(|e| (std::mem::discriminant(&e.body), e.cause)),
        );
        last = Some(out.scene);
    }
    let s = last.expect("the night has samples");
    Night {
        facts,
        end: (
            s.room.state,
            s.room.occupants,
            s.bed.state,
            s.bed.held.len(),
            s.bed.unexplained,
        ),
    }
}

/// The golden scenes that assume only the main detector.
fn scenes() -> Vec<(String, Scene)> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden");
    let mut scenes: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md") && !p.ends_with("README.md"))
        .map(|p| {
            let scene = Scene::parse(&std::fs::read_to_string(&p).unwrap()).unwrap();
            (p.file_name().unwrap().to_string_lossy().into_owned(), scene)
        })
        .filter(|(_, s)| s.senses == [Sense::Detect])
        .collect();
    scenes.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(scenes.len() >= 9);
    scenes
}

/// The samples to perturb without touching a transition: the second to last of
/// each fixed span of at least 5 samples (the transition stabilized earlier).
fn still(scene: &Scene) -> Vec<u32> {
    scene
        .spans
        .iter()
        .filter(|s| matches!(s.bed, InBed::Fixed(_)) && s.samples.to - s.samples.from >= 4)
        .map(|s| s.samples.to - 1)
        .collect()
}

#[test]
fn at_another_cadence_the_night_reaches_the_same() {
    for (name, scene) in scenes() {
        let base = run(&scene.samples(), None);
        let slow = Scene {
            cadence_s: 2.0,
            ..scene.clone()
        };
        assert_eq!(run(&slow.samples(), None), base, "{name} at 2 s");
    }
}

#[test]
fn a_detector_flicker_does_not_change_the_night() {
    for (name, scene) in scenes() {
        let base = run(&scene.samples(), None);
        for n in still(&scene) {
            let mut samples = scene.samples();
            if let Some(m) = samples.iter_mut().find(|m| m.n == n) {
                m.detect.clear();
            }
            assert_eq!(run(&samples, None), base, "{name}, flicker at {n}");
        }
    }
}

#[test]
fn a_detector_failure_does_not_change_the_night() {
    for (name, scene) in scenes() {
        let base = run(&scene.samples(), None);
        for n in still(&scene) {
            assert_eq!(
                run(&scene.samples(), Some(n)),
                base,
                "{name}, failure at {n}"
            );
        }
    }
}
