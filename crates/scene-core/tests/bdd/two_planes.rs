//! The room in two planes (`features/occupancy/two-planes.feature`; white
//! book, volume VII, ch. 14): the profile chooses the geometry that answers
//! where a body is.

use cucumber::{given, then};
use scene_core::contract::{BodySeen, Zone};
use scene_core::policy::TwoPlanesPolicy;

use crate::ward::Ward;

#[given(
    regex = r"^the room is measured in two planes: in bed with (\d+(?:\.\d+)?) of the box over the mattress and its support within (\d+(?:\.\d+)?) metres, near the bed within (\d+(?:\.\d+)?) metres$"
)]
fn two_planes(ward: &mut Ward, share: f32, support_m: f32, near_m: f32) {
    ward.policy.two_planes = Some(TwoPlanesPolicy {
        share,
        support_m,
        near_m,
    });
}

fn body(ward: &Ward) -> BodySeen {
    let items = &ward.last().output.scene.evidence.bodies.items;
    assert_eq!(items.len(), 1, "a single body: {items:?}");
    items.first().cloned().expect("a body")
}

#[then(regex = r#"^the body is (?:in|on) the "(bed|floor)"$"#)]
fn body_zone(ward: &mut Ward, zone: String) {
    let expected = match zone.as_str() {
        "bed" => Zone::Bed,
        _ => Zone::Floor,
    };
    assert_eq!(body(ward).zone, Some(expected));
}

#[then("the body is near the bed")]
fn near(ward: &mut Ward) {
    assert_eq!(body(ward).near_bed, Some(true));
}
