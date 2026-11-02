//! Capability steps (`features/architecture/capability-slices.feature`): each
//! decider receives its own policy slice, resolved when the monitor is built.
//! The deciders are private; the steps observe what they publish: the count,
//! the bed state, and whether layer 1 publishes a bed posture.

use cucumber::{given, then, when};
use scene_core::policy::BedPosturePolicy;
use serde_json::Value;

use crate::attention::scene;
use crate::script::{Script, came_in_alone};
use crate::ward::Ward;
use scene_core::units::Seconds;

/// The resident came in alone and lay down in bed, samples 1 to 10, in a room
/// with the bed delimited.
fn lay_down(ward: &mut Ward) {
    if ward.map.is_none() {
        ward.bed();
    }
    for (n, script) in came_in_alone(1, 10) {
        ward.turn_with(ward.sample(n), script);
    }
}

/// The resident lay down in samples 1 to `lay_down_to`; then, with a dwell of
/// `dwell` seconds of source time, nobody is seen for `unseen` samples.
fn held_resident(ward: &mut Ward, lay_down_to: u32, unseen: u32, dwell: f64) {
    if ward.map.is_none() {
        ward.bed();
    }
    for (n, script) in came_in_alone(1, lay_down_to) {
        ward.turn_with(ward.sample(n), script);
    }
    ward.policy.persistence.dwell_secs = Seconds::new(dwell).expect("a time in source seconds");
    for n in lay_down_to + 1..=lay_down_to + unseen {
        ward.turn_with(ward.sample(n), Script::default());
    }
}

/// What the scene publishes at sample `n`: the room's occupants and the bed's
/// state.
fn published_at(ward: &Ward, n: u32) -> (u32, Value) {
    let scene = scene(ward, n);
    let bed = serde_json::to_value(&scene.bed).expect("the bed serializes");
    (scene.room.occupants, bed["state"].clone())
}

#[given(regex = r#"^a profile without "\[verify\]"$"#)]
fn profile_without_verify(ward: &mut Ward) {
    ward.policy.verify = None;
}

/// Two runs from the same steps: the one the scenario answers, and its control,
/// which gets no answer. Each is the resident held in bed, the count stabilized
/// for the dwell (as `held.feature`).
#[given(
    regex = r"^the bed holds a resident who lay down in samples 1 to (\d+) and is not seen for (\d+) samples with a dwell of (\d+) seconds$"
)]
fn bed_holds_a_resident(ward: &mut Ward, lay_down_to: u32, unseen: u32, dwell: f64) {
    let mut control = Box::new(Ward::default());
    control.policy = ward.policy.clone();
    held_resident(ward, lay_down_to, unseen, dwell);
    held_resident(&mut control, lay_down_to, unseen, dwell);
    ward.control = Some(control);
}

/// The answer goes to the scenario's run only; the control keeps no answer.
/// Both are driven with the next sample.
#[when(regex = r#"^a verifier answers "(yes|no|unsure)" to a question about that body$"#)]
fn verifier_answers_about_body(ward: &mut Ward, answer: String) {
    let n = ward.last().output.at.sequence as u32 + 1;
    ward.verifier.answer = Some(match answer.as_str() {
        "yes" => scene_core::look::Answer::Yes,
        "no" => scene_core::look::Answer::No,
        _ => scene_core::look::Answer::Unsure,
    });
    ward.turn_with(ward.sample(n), Script::default());
    let control = ward.control.as_mut().expect("the control run is built");
    control.turn_with(control.sample(n), Script::default());
}

/// The real comparison: the occupants and the bed state published at the same
/// sample are equal with the answer and without it.
#[then("the room occupants are the same as without the answer")]
fn occupants_same_as_without(ward: &mut Ward) {
    let n = ward.last().output.at.sequence as u32;
    let control = ward.control.as_ref().expect("the control run is built");
    assert_eq!(
        published_at(ward, n),
        published_at(control, n),
        "(occupants, bed state) with the answer, and without it"
    );
}

/// In the scenario's run and in its control.
#[then(regex = r"^the published bed state is ([a-z_]+)$")]
fn published_bed_state(ward: &mut Ward, state: String) {
    let n = ward.last().output.at.sequence as u32;
    assert_eq!(published_at(ward, n).1, state.as_str(), "with the answer");
    if let Some(control) = &ward.control {
        assert_eq!(
            published_at(control, n).1,
            state.as_str(),
            "without the answer"
        );
    }
}

#[given(regex = r#"^a profile with "\[bed_posture\]"$"#)]
fn profile_with_bed_posture(ward: &mut Ward) {
    ward.policy.bed_posture = Some(BedPosturePolicy::default());
}

#[when("the monitor is built")]
fn the_monitor_is_built(ward: &mut Ward) {
    lay_down(ward);
}

/// The subject's bed posture as the scene publishes it: `null` when layer 1 is
/// not built, its state when it is.
fn bed_posture_of(ward: &Ward, n: u32) -> Value {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["bed_posture"].clone()
}

#[then("the bed posture decider exists")]
fn the_bed_posture_decider_exists(ward: &mut Ward) {
    let posture = bed_posture_of(ward, 10);
    assert!(!posture.is_null(), "the scene publishes no bed posture");
}

#[then(regex = r#"^a profile without "\[bed_posture\]" builds no bed posture decider$"#)]
fn no_bed_posture_decider_without_section(_: &mut Ward) {
    // The same resident, in the same room, with the section absent.
    let mut plain = Ward::default();
    plain.bed();
    plain.policy.bed_posture = None;
    for (n, script) in came_in_alone(1, 10) {
        plain.turn_with(plain.sample(n), script);
    }
    let posture = bed_posture_of(&plain, 10);
    assert!(posture.is_null(), "the scene publishes {posture}");
}
