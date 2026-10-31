//! Bed posture steps (`features/bed-posture`): layer 1, the resident they
//! left lying. The machine's table is checked by the core's golden table; here,
//! what the scene publishes.

use cucumber::{given, then};
use serde_json::Value;

use crate::attention::scene;
use crate::ward::Ward;
use scene_core::policy::BedPosturePolicy;

#[given("the bed posture is followed")]
fn followed(ward: &mut Ward) {
    ward.policy.bed_posture = Some(BedPosturePolicy::default());
}

fn bed_posture(ward: &Ward, n: u32) -> Value {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["bed_posture"].clone()
}

#[then(regex = r#"^in sample (\d+) the bed posture is "([a-z_]+)"(?:, seen "([a-z_]+)")?$"#)]
fn the_bed_posture(ward: &mut Ward, n: u32, state: String, seen: String) {
    let b = bed_posture(ward, n);
    assert_eq!(b["state"], state.as_str(), "{b}");
    if !seen.is_empty() {
        assert_eq!(b["seen"], seen.as_str(), "{b}");
    }
}

#[given(
    regex = r#"^the bed posture "(lying|sitting_up|getting_out)" must hold (\d+) seconds of source time$"#
)]
fn must_hold(ward: &mut Ward, state: String, secs: u32) {
    let policy = ward
        .policy
        .bed_posture
        .as_mut()
        .expect("\"the bed posture is followed\" first");
    let secs = f64::from(secs);
    match state.as_str() {
        "lying" => policy.dwell_lying_secs = secs,
        "sitting_up" => policy.dwell_sitting_up_secs = secs,
        _ => policy.dwell_getting_out_secs = secs,
    }
}

#[given("the bed posture is not followed")]
fn not_followed(ward: &mut Ward) {
    ward.policy.bed_posture = None;
}

#[then(expr = "in sample {int} there is no bed posture")]
fn no_bed_posture(ward: &mut Ward, n: u32) {
    let b = bed_posture(ward, n);
    assert!(b.is_null(), "{b}");
}

// --- Published like layer 0: each change of the stable state is an event

/// The bed posture changes published in sample `n`, as the wire carries them.
fn changes(ward: &Ward, n: u32) -> Vec<Value> {
    let at = ward.sample(n);
    ward.turns
        .iter()
        .filter(|t| t.output.at == at)
        .flat_map(|t| &t.output.events)
        .map(|e| serde_json::to_value(e).expect("an event serializes"))
        .filter(|e| e["kind"] == "bed_posture")
        .collect()
}

#[then(
    regex = r#"^in sample (\d+) the bed posture changed from "([a-z_]+)" to "([a-z_]+)", seen "([a-z_]+)"$"#
)]
fn changed(ward: &mut Ward, n: u32, before: String, after: String, seen: String) {
    let changes = changes(ward, n);
    let [change] = changes.as_slice() else {
        panic!("one bed posture change in sample {n}, not {changes:?}");
    };
    assert_eq!(change["before"]["state"], before.as_str(), "{change}");
    assert_eq!(change["seen"], seen.as_str(), "{change}");
    assert_eq!(change["after"], after.as_str(), "{change}");
    assert_eq!(
        change["cell"],
        format!("{before} | {seen} | {after}"),
        "{change}"
    );
    // Since when what we believed held: the date the previous scene gave it.
    let previous = bed_posture(ward, n - 1);
    assert_eq!(
        change["before"]["since"], previous["since"],
        "{change} after {previous}"
    );
}

#[then(regex = r"^the bed posture changes published in samples (\d+) to (\d+) are none$")]
fn no_changes(ward: &mut Ward, first: u32, last: u32) {
    for n in first..=last {
        let changes = changes(ward, n);
        assert!(changes.is_empty(), "sample {n}: {changes:?}");
    }
}

#[then("on every turn, each bed posture change agrees with its scene")]
fn changes_agree(ward: &mut Ward) {
    let mut published = 0;
    for turn in &ward.turns {
        let scene = serde_json::to_value(&*turn.output.scene).expect("the scene serializes");
        let at = serde_json::to_value(turn.output.at).expect("a sample serializes");
        for event in &turn.output.events {
            let event = serde_json::to_value(event).expect("an event serializes");
            if event["kind"] != "bed_posture" {
                continue;
            }
            published += 1;
            let now = &scene["subject"]["bed_posture"];
            assert_eq!(event["at"], at, "{event}");
            assert_eq!(event["after"], now["state"], "{event} in {now}");
            assert_eq!(event["seen"], now["seen"], "{event} in {now}");
            assert_eq!(now["since"], at, "the change dates the state: {now}");
        }
    }
    assert!(
        published > 0,
        "the scenario published no bed posture change"
    );
}
