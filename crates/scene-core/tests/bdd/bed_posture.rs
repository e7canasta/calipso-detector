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
