//! The room in two planes (`features/occupancy/two-planes.feature`; white
//! book, volume VII, ch. 14): the profile chooses the geometry that answers
//! where a body is.

use cucumber::{given, then, when};
use scene_contract::{BodySeen, LookKind, LookOutcome, LookResult, Zone};
use scene_core::policy::TwoPlanesPolicy;
use scene_geom::Rect;
use scene_look::{Detection, Failure, Label};

use crate::script::Script;
use crate::ward::Ward;
use scene_core::units::{Degrees, Metres, PositiveMetres, Ratio};

#[given(
    regex = r"^the room is measured in two planes: in bed with (\d+(?:\.\d+)?) of the box over the mattress and its support within (\d+(?:\.\d+)?) metres, near the bed within (\d+(?:\.\d+)?) metres$"
)]
fn two_planes(ward: &mut Ward, share: f32, support_m: f32, near_m: f32) {
    ward.policy.two_planes = Some(TwoPlanesPolicy {
        share: Ratio::new(share).expect("a share in (0, 1]"),
        support_m: Metres::new(support_m).expect("a distance"),
        near_m: PositiveMetres::new(near_m).expect("a positive distance"),
        knee_bent_deg: None,
    });
}

#[given(
    regex = r"^a doubtful body is in bed with the knee bent to (\d+(?:\.\d+)?) degrees or less$"
)]
fn knee(ward: &mut Ward, bent_deg: f32) {
    ward.policy
        .two_planes
        .as_mut()
        .expect("\"the room is measured in two planes\" first")
        .knee_bent_deg = Some(Degrees::new(bent_deg).expect("an angle in (0, 180)"));
}

fn person(x0: f32, y0: f32, x1: f32, y1: f32) -> Script {
    Script::bodies(vec![Detection {
        bbox: Rect::new(x0, y0, x1, y1),
        confidence: 0.9,
        label: Label::Person,
    }])
}

#[when(
    regex = r#"^in sample (\d+) the camera sees a person at (\d+),(\d+) to (\d+),(\d+) and the pose sees them "([^"]+)"$"#
)]
fn sees_with_pose(ward: &mut Ward, n: u32, x0: f32, y0: f32, x1: f32, y1: f32, figure: String) {
    let mut script = person(x0, y0, x1, y1);
    script.figure = Some(crate::zones::figure(&figure));
    ward.turn_with(ward.sample(n), script);
}

#[when(
    regex = r#"^in samples (\d+) to (\d+) the camera sees a person at (\d+),(\d+) to (\d+),(\d+) and the pose sees them "([^"]+)"$"#
)]
#[allow(clippy::too_many_arguments)]
fn sees_with_pose_in_samples(
    ward: &mut Ward,
    from: u32,
    to: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    figure: String,
) {
    for n in from..=to {
        sees_with_pose(ward, n, x0, y0, x1, y1, figure.clone());
    }
}

#[when(
    regex = r"^in sample (\d+) the camera sees a person at (\d+),(\d+) to (\d+),(\d+) and the pose fails$"
)]
fn sees_and_the_pose_fails(ward: &mut Ward, n: u32, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut script = person(x0, y0, x1, y1);
    script.pose = Err(Failure::Model);
    ward.turn_with(ward.sample(n), script);
}

/// What the ledger says of the doubtful body's pose in the last turn.
fn pose_body(ward: &Ward) -> Option<LookOutcome> {
    ward.last()
        .output
        .v5
        .scene
        .attention
        .looks
        .iter()
        .find(|l| l.kind == LookKind::PoseBody)
        .map(|l| l.outcome.clone())
}

#[then("the pose was asked for a doubtful body")]
fn pose_asked(ward: &mut Ward) {
    let outcome = pose_body(ward);
    assert!(
        matches!(
            outcome,
            Some(LookOutcome::Asked {
                result: LookResult::Read { .. },
                ..
            })
        ),
        "{outcome:?}"
    );
}

#[then("the pose for a doubtful body failed")]
fn pose_failed(ward: &mut Ward) {
    let outcome = pose_body(ward);
    assert!(
        matches!(
            outcome,
            Some(LookOutcome::Asked {
                result: LookResult::Failed { .. },
                ..
            })
        ),
        "{outcome:?}"
    );
}

#[then("the pose was not asked for any doubtful body")]
fn pose_not_asked(ward: &mut Ward) {
    assert_eq!(pose_body(ward), None);
}

#[then(regex = r"^the body's knee measured (at most|more than) (\d+(?:\.\d+)?) degrees$")]
fn knee_measured(ward: &mut Ward, how: String, deg: f32) {
    let knee = body(ward).knee_deg.expect("a knee measured");
    match how.as_str() {
        "at most" => assert!(knee <= deg, "{knee}"),
        _ => assert!(knee > deg, "{knee}"),
    }
}

#[then("the body's knee was not measured")]
fn knee_not_measured(ward: &mut Ward) {
    assert_eq!(body(ward).knee_deg, None);
}

fn body(ward: &Ward) -> BodySeen {
    let items = &ward.last().output.v5.scene.evidence.bodies.items;
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
