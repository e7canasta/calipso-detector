//! Steps of the parts (`features/parts`): where each part of the subject is
//! against the bed and the headboard, and what is kept unseen, in the trama's
//! notation as the scene publishes it.

use cucumber::{given, then};
use serde_json::Value;

use crate::attention::scene;
use crate::ward::Ward;

fn parts(ward: &Ward, n: u32) -> Value {
    let scene = serde_json::to_value(scene(ward, n)).expect("the scene serializes");
    scene["subject"]["parts"].clone()
}

#[then(expr = "in sample {int} the parts saw {string}")]
fn parts_saw(ward: &mut Ward, n: u32, seen: String) {
    let parts = parts(ward, n);
    assert_eq!(parts["seen"], seen.as_str(), "{parts}");
}

#[then(expr = "in sample {int} the parts are {string}")]
fn parts_are(ward: &mut Ward, n: u32, state: String) {
    let parts = parts(ward, n);
    assert_eq!(parts["state"], state.as_str(), "{parts}");
}

#[then(expr = "in sample {int} there are no parts")]
fn no_parts(ward: &mut Ward, n: u32) {
    let parts = parts(ward, n);
    assert!(parts.is_null(), "{parts}");
}

// --- Published like layer 0: pending, since, the change and its cell

fn turn(ward: &Ward, n: u32) -> &scene_core::Output {
    let at = ward.sample(n);
    &ward
        .turns
        .iter()
        .rev()
        .find(|t| t.output.at == Some(at))
        .unwrap_or_else(|| panic!("there was no sample {n}"))
        .output
}

fn change(ward: &Ward, n: u32) -> Option<&scene_contract::PartsChange> {
    turn(ward, n).v5.events.iter().find_map(|e| match e {
        scene_contract::v5::Event::Parts(p) => Some(p),
        _ => None,
    })
}

#[then(
    expr = "in sample {int} the parts' new places {string} are pending for {float} of {float} seconds"
)]
fn parts_pending(ward: &mut Ward, n: u32, places: String, have: f64, need: f64) {
    let parts = parts(ward, n);
    let pending = &parts["pending"];
    assert_eq!(pending["places"], places.as_str(), "{parts}");
    assert_eq!(pending["duration"]["have"], have, "{parts}");
    assert_eq!(pending["duration"]["need"], need, "{parts}");
}

#[then(expr = "in sample {int} the parts changed from {string} to {string}")]
fn parts_changed(ward: &mut Ward, n: u32, before: String, after: String) {
    let change = change(ward, n).unwrap_or_else(|| panic!("no parts change in sample {n}"));
    assert_eq!(change.before.places, before, "{change:?}");
    assert_eq!(change.after.places, after, "{change:?}");
    assert_eq!(
        change.cell,
        format!("{before} | {} | {after}", change.seen.parts),
        "{change:?}"
    );
    let parts = parts(ward, n);
    assert_eq!(parts["last_cell"], change.cell.as_str(), "{parts}");
}

#[then(expr = "in sample {int} no parts change was published")]
fn no_parts_change(ward: &mut Ward, n: u32) {
    assert!(change(ward, n).is_none(), "{:?}", change(ward, n));
}

#[then(expr = "in sample {int} the parts hold since sample {int}")]
fn parts_since(ward: &mut Ward, n: u32, since: u32) {
    let parts = parts(ward, n);
    let at = serde_json::to_value(ward.sample(since)).expect("a sample serializes");
    assert_eq!(parts["since"], at, "{parts}");
}

// --- The reading: what we believe from the places

#[given("the bed's headboard delimited down to the shoulders")]
fn headboard_to_the_shoulders(ward: &mut Ward) {
    ward.map.as_mut().expect("the bed first").headboard =
        Some(scene_geom::Rect::new(100.0, 100.0, 500.0, 210.0));
}

#[then(expr = "in sample {int} the parts read {string}")]
fn parts_read(ward: &mut Ward, n: u32, reading: String) {
    let parts = parts(ward, n);
    assert_eq!(parts["reading"], reading.as_str(), "{parts}");
}
