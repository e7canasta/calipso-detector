//! Steps of the parts (`features/parts`): where each part of the subject is
//! against the bed and the headboard, and what is kept unseen, in the trama's
//! notation as the scene publishes it.

use cucumber::then;
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
