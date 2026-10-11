//! Occupancy steps (`features/occupancy`): the bed only rises from the floor,
//! stabilization by time, failing reinforcement and the scene that declares
//! when it did not observe.

use crate::published::Body;
use cucumber::{given, then, when};
use scene_core::contract::{Cause, LookKind, LookOutcome, RepublishReason, SampleRef};
use scene_core::look::Failure;
use scene_core::policy::ReinforcePolicy;

use crate::script::Script;
use crate::ward::Ward;
use golden::language::Location;
use golden::world::on_floor;

// --- 501: the bed only rises from the floor

#[then(regex = r"^the bed has (\d+) unexplained bod(?:y|ies)$")]
fn unexplained(ward: &mut Ward, n: u32) {
    let bed = &ward.last().output.scene.bed;
    assert_eq!(bed.unexplained, n, "{bed:#?}");
}

#[then("nobody is held in bed")]
fn nobody_held(ward: &mut Ward) {
    let bed = &ward.last().output.scene.bed;
    assert!(bed.held.is_empty(), "{bed:#?}");
}

#[then(expr = "someone was observed lying down after sample {int}")]
fn lay_down_after(ward: &mut Ward, n: u32) {
    let limit = ward.sample(n);
    assert!(
        ward.events()
            .any(|e| matches!(e.body, Body::LayDown) && crate::ward::later(e.at, limit)),
        "nobody lay down after sample {n}: {:?}",
        ward.events().collect::<Vec<_>>()
    );
}

#[then("the bed was adopted at startup")]
fn adopted(ward: &mut Ward) {
    assert!(
        ward.events()
            .any(|e| matches!(e.body, Body::LayDown) && e.cause == Cause::Baseline),
        "the bed was not adopted: {:?}",
        ward.events().collect::<Vec<_>>()
    );
}

// --- 502: stabilize by time

#[when(regex = r"^a person appears at the door and stays for (\d+) samples$")]
fn appears_and_stays(ward: &mut Ward, samples: u32) {
    for n in 1..=samples {
        let at = ward.sample(n);
        ward.turn_with(at, Script::bodies(vec![on_floor(0, Location::Door)]));
    }
}

#[then(
    regex = r"^an entry into the room was observed (\d+(?:\.\d+)?) seconds of source time after first seeing them$"
)]
fn entry_after(ward: &mut Ward, secs: f64) {
    let first = ward.sample(1).source_secs.0;
    let entry = ward
        .events()
        .find(|e| matches!(e.body, Body::Entered))
        .expect("an entry was observed");
    assert_eq!(entry.at.source_secs.0 - first, secs);
}

#[then("no entry into the room was observed")]
fn no_entry(ward: &mut Ward) {
    let entries: Vec<_> = ward
        .events()
        .filter(|e| matches!(e.body, Body::Entered))
        .collect();
    assert!(entries.is_empty(), "observed: {entries:?}");
}

// --- 503: failing reinforcement does not spend the loss

#[given("there is a reinforcement detector")]
fn reinforcement(ward: &mut Ward) {
    ward.policy.reinforce = Some(ReinforcePolicy { bed_padding: 0.1 });
}

#[given("reinforcement fails")]
fn reinforcement_fails(ward: &mut Ward) {
    ward.reinforcement_fails = true;
}

/// The samples in which reinforcement was requested.
fn reinforcements(ward: &Ward) -> Vec<SampleRef> {
    ward.turns
        .iter()
        .filter(|t| {
            t.output.scene.attention.looks.iter().any(|l| {
                l.kind == LookKind::Reinforce && matches!(l.outcome, LookOutcome::Asked { .. })
            })
        })
        .map(|t| t.output.at)
        .collect()
}

#[then(regex = r"^reinforcement was requested in samples (\d+) and (\d+)$")]
fn reinforcement_in(ward: &mut Ward, a: u32, b: u32) {
    let requested = reinforcements(ward);
    for n in [a, b] {
        assert!(
            requested.contains(&ward.sample(n)),
            "reinforcement was not requested in sample {n}: {requested:?}"
        );
    }
}

#[then(expr = "reinforcement was requested only in sample {int}")]
fn reinforcement_only_in(ward: &mut Ward, n: u32) {
    assert_eq!(reinforcements(ward), vec![ward.sample(n)]);
}

#[then("reinforcement was not requested")]
fn no_reinforcement(ward: &mut Ward) {
    assert_eq!(reinforcements(ward), Vec::<SampleRef>::new());
}

// --- 504: the scene declares when it did not observe

#[given(expr = "the detector fails in samples {int} to {int}")]
fn detector_fails(ward: &mut Ward, from: u32, to: u32) {
    for n in from..=to {
        let at = ward.sample(n);
        let script = Script {
            detect: Err(Failure::Model),
            ..Script::default()
        };
        ward.turn_with(at, script);
    }
}

#[then(expr = "the scene declares no new observation since sample {int}")]
fn declared(ward: &mut Ward, n: u32) {
    let republished = ward
        .last()
        .output
        .scene
        .republished
        .expect("the scene is declared republished");
    assert_eq!(republished.reason, RepublishReason::DetectFailed);
    assert_eq!(republished.observed_at, Some(ward.sample(n)));
}

#[then("the scene is an observation of the last sample")]
fn observed(ward: &mut Ward) {
    assert_eq!(ward.last().output.scene.republished, None);
}

#[when(expr = "sample {int} cannot be decoded")]
fn decode_fails(ward: &mut Ward, n: u32) {
    ward.drive(
        scene_core::Input::NoFrame {
            at: ward.sample(n),
            why: scene_core::NoFrame::DecodeFailed,
        },
        &Script::default(),
    );
}

#[when(expr = "the sensor stalls at sample {int}")]
fn sensor_stalls(ward: &mut Ward, n: u32) {
    ward.drive(
        scene_core::Input::Sensor {
            at: ward.sample(n),
            state: scene_core::contract::SensorState::Stalled,
        },
        &Script::default(),
    );
}

#[then(expr = "the republished scene keeps the last observation at sample {int}")]
fn republished_origin(ward: &mut Ward, n: u32) {
    assert_eq!(
        ward.last().output.scene.republished.unwrap().observed_at,
        Some(ward.sample(n)),
    );
}

#[then("the republished scene declares there were no observations yet")]
fn no_earlier_observation(ward: &mut Ward) {
    assert_eq!(
        ward.last().output.scene.republished.unwrap().observed_at,
        None
    );
}
