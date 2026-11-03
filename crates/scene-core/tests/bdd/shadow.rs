//! The geometry's shadow (`features/contract/shadow.feature`; white book,
//! volume VII, ch. 11): what a geometry that does not decide yet sees,
//! published next to what is decided.

use cucumber::{given, then, when};
use scene_contract::{BodySeen, ShadowPlane, Zone};
use scene_core::map::MapShadow;
use scene_core::policy::ShadowPolicy;
use scene_core::{Monitor, Output};
use scene_geom::{Quad, Rect};
use scene_look::{Detection, Label};

use crate::script::Script;
use crate::ward::Ward;
use scene_core::units::{Metres, PositiveMetres};

/// Within a pixel: the expectations are written rounded.
const PIXEL: f32 = 1.0;

#[given(regex = r"^the bed's footprint on the floor is delimited as (\d+),(\d+) to (\d+),(\d+)$")]
fn footprint(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32) {
    ward.map.as_mut().expect("the bed first").bed_floor =
        Some(Quad::from_rect(Rect::new(x0, y0, x1, y1)));
}

#[given(expr = "the bed measures {float} by {float} metres")]
fn size(ward: &mut Ward, width: f32, length: f32) {
    ward.map.as_mut().expect("the bed first").bed_size_m = Some([width, length]);
}

#[given(expr = "the geometry's shadow measures near the bed within {float} metres")]
fn shadow_on(ward: &mut Ward, near_m: f32) {
    ward.policy.shadow = Some(ShadowPolicy {
        near_m: PositiveMetres::new(near_m).expect("a positive distance"),
        support_margin_m: Metres::new(0.0).expect("a constant distance"),
    });
}

fn shadow(ward: &Ward) -> MapShadow {
    ward.last()
        .output
        .v5
        .scene
        .map
        .shadow
        .expect("the scene's map carries a shadow")
}

fn close(a: Rect, b: Rect) -> bool {
    [(a.x0, b.x0), (a.y0, b.y0), (a.x1, b.x1), (a.y1, b.y1)]
        .iter()
        .all(|(p, q)| (p - q).abs() <= PIXEL)
}

#[then("the scene's map carries no shadow")]
fn no_shadow(ward: &mut Ward) {
    assert_eq!(ward.last().output.v5.scene.map.shadow, None);
}

#[then(
    regex = r"^the scene's map shadow carries the bed's footprint as (\d+),(\d+) to (\d+),(\d+)$"
)]
fn carries_footprint(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32) {
    assert_eq!(
        shadow(ward).bed_floor,
        Some(Quad::from_rect(Rect::new(x0, y0, x1, y1)))
    );
}

#[then("the scene's map shadow carries no footprint")]
fn carries_no_footprint(ward: &mut Ward) {
    assert_eq!(shadow(ward).bed_floor, None);
}

#[then(expr = "the scene's map shadow carries a bed of {float} by {float} metres")]
fn carries_size(ward: &mut Ward, width: f32, length: f32) {
    assert_eq!(shadow(ward).bed_size_m, Some([width, length]));
}

#[then(
    regex = r"^the scene's map shadow carries near the bed as (-?\d+),(-?\d+) to (-?\d+),(-?\d+)$"
)]
fn carries_near(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32) {
    let near = shadow(ward).near.expect("near in metres");
    let expected = Rect::new(x0, y0, x1, y1);
    assert!(
        close(near.bounds(), expected),
        "{near:?} is not {expected:?}"
    );
}

#[then("the scene's map shadow carries no near in metres")]
fn carries_no_near(ward: &mut Ward) {
    assert_eq!(shadow(ward).near, None);
}

#[then(
    regex = r"^the scene's map shadow carries near the bed today as (\d+),(\d+) to (\d+),(\d+)$"
)]
fn carries_near_today(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32) {
    let today = shadow(ward).near_today;
    let expected = Rect::new(x0, y0, x1, y1);
    assert!(close(today, expected), "{today:?} is not {expected:?}");
}

#[then(
    "the same night without the shadow gives the same scenes, but for the shadow, and the same events"
)]
fn shadow_decides_nothing(ward: &mut Ward) {
    let mut policy = ward.policy.clone();
    policy.shadow = None;
    let mut monitor = Monitor::new(policy, ward.map.clone().expect("there is a map"));
    let without: Vec<Output> = ward
        .recording
        .iter()
        .map(|(input, script)| {
            monitor
                .handle(input.clone())
                .drive(|request| (script.respond(request), scene_look::Spent(1.0)))
        })
        .collect();
    assert_eq!(without.len(), ward.turns.len());
    for (bare, turn) in without.iter().zip(&ward.turns) {
        let mut with = turn.output.v5.scene.as_ref().clone();
        assert!(with.map.shadow.is_some(), "the shadow was asked for");
        with.map.shadow = None;
        for body in &mut with.evidence.bodies.items {
            body.shadow = None;
        }
        assert_eq!(*bare.v5.scene, with, "the scene at {:?} differs", bare.at);
        assert_eq!(
            bare.v5.events, turn.output.v5.events,
            "the events at {:?} differ",
            bare.at
        );
    }
}

// --- Stage 2: each body

#[when(regex = r"^in sample (\d+) the camera sees a person at (\d+),(\d+) to (\d+),(\d+)$")]
fn sees_a_person(ward: &mut Ward, n: u32, x0: f32, y0: f32, x1: f32, y1: f32) {
    let person = Detection {
        bbox: Rect::new(x0, y0, x1, y1),
        confidence: 0.9,
        label: Label::Person,
    };
    ward.turn_with(ward.sample(n), Script::bodies(vec![person]));
}

fn body(ward: &Ward) -> BodySeen {
    let items = &ward.last().output.v5.scene.evidence.bodies.items;
    assert_eq!(items.len(), 1, "a single body: {items:?}");
    items.first().cloned().expect("a body")
}

fn zone(name: &str) -> Zone {
    match name {
        "bed" => Zone::Bed,
        "floor" => Zone::Floor,
        other => panic!("unknown zone {other}"),
    }
}

#[then(
    regex = r#"^the body is in the "(bed|floor)" today and (?:in|on) the "(bed|floor)" for the shadow$"#
)]
fn zones(ward: &mut Ward, today: String, shadow: String) {
    let body = body(ward);
    assert_eq!(body.zone, Some(zone(&today)));
    assert_eq!(body.shadow.expect("a shadow").zone, Some(zone(&shadow)));
}

#[then(
    regex = r#"^the body's support falls (\d+(?:\.\d+)?) metres from the bed, on the "(floor|mattress)"$"#
)]
fn support(ward: &mut Ward, metres: f32, plane: String) {
    let shadow = body(ward).shadow.expect("a shadow");
    let distance = shadow.support_distance_m.expect("a distance");
    assert!(
        (distance - metres).abs() <= 0.01,
        "{distance} is not {metres}"
    );
    let expected = match plane.as_str() {
        "floor" => ShadowPlane::Floor,
        _ => ShadowPlane::Mattress,
    };
    assert_eq!(shadow.support_plane, expected);
}

#[then("the body's support is near the bed for the shadow")]
fn support_near(ward: &mut Ward) {
    assert_eq!(body(ward).shadow.expect("a shadow").near_bed, Some(true));
}

#[then(expr = "the body's share over the mattress is {float}")]
fn share_over_the_mattress(ward: &mut Ward, share: f32) {
    let got = body(ward).shadow.expect("a shadow").share;
    assert!((got - share).abs() < 1e-6, "{got} is not {share}");
}

#[then("the body carries no shadow")]
fn no_body_shadow(ward: &mut Ward) {
    assert_eq!(body(ward).shadow, None);
}

/// `[occupancy.parts]` with the sprint 1 values (ADR 2026-11-03; reglas.md, F1.1d),
/// on the two planes it needs, with the pose's keypoints on.
#[given("the profile asks for the parts")]
fn parts_profile(ward: &mut Ward) {
    use scene_core::policy::{EnrichPolicy, PartsPolicy, TwoPlanesPolicy};
    use scene_core::units::{
        Confidence, DiagonalFraction, Extension, HeadboardW, MattressHeights, Proportion, Ratio,
        RelativeDepth, Span,
    };
    ward.policy.two_planes = Some(TwoPlanesPolicy {
        share: Ratio::new(0.45).expect("a constant share"),
        support_m: Metres::new(0.10).expect("a constant distance"),
        near_m: PositiveMetres::new(0.6).expect("a constant distance"),
        knee_bent_deg: None,
    });
    ward.policy.enrich = Some(EnrichPolicy {
        move_threshold: DiagonalFraction::new(0.1).expect("a constant fraction"),
        depth: false,
        min_keypoint_score: Confidence::new(0.5).expect("a constant score"),
        posture_tolerance: RelativeDepth::new(0.05).expect("a constant depth"),
        max_pose_age_secs: None,
        crop_pad: Extension::new(0.2).expect("a constant extension"),
    });
    ward.policy.parts = Some(PartsPolicy {
        far_strip_m: Metres::new(0.18).expect("a constant distance"),
        ankle_proof_score: Confidence::new(0.75).expect("a constant score"),
        pelvis_band: Span::new(
            MattressHeights::new(0.6).expect("a finite height"),
            MattressHeights::new(1.4).expect("a finite height"),
        )
        .expect("an ordered band"),
        cut_tolerance: Proportion::new(0.12).expect("a constant proportion"),
        headboard_w: Span::new(
            HeadboardW::new(-0.15).expect("a finite w"),
            HeadboardW::new(1.0).expect("a finite w"),
        )
        .expect("an ordered band"),
        decide: false,
    });
}

/// The parts decide the zone (F4.1): the cascade and its memory say where a
/// body is, instead of the box.
#[given("the parts decide the zone")]
fn parts_decide(ward: &mut Ward) {
    ward.policy.parts.as_mut().expect("the parts first").decide = true;
}

#[then(regex = r#"^the body's parts are on the "(bed|floor)", by the step "(\w+)"$"#)]
fn parts_zone_and_step(ward: &mut Ward, zone_name: String, want: String) {
    let parts = body(ward)
        .shadow
        .expect("a shadow")
        .parts
        .expect("the body's parts");
    assert_eq!(parts.zone, zone(&zone_name));
    let got = serde_json::to_value(parts.step).expect("a step name");
    assert_eq!(got, serde_json::Value::String(want));
}

#[then(regex = r#"^the body's parts turn by "(\w+)"$"#)]
fn parts_turn(ward: &mut Ward, turn: String) {
    let parts = body(ward)
        .shadow
        .expect("a shadow")
        .parts
        .expect("the body's parts");
    let got = serde_json::to_value(parts.turn).expect("a turn name");
    assert_eq!(got, serde_json::Value::String(turn));
}

#[then("the body's shadow carries no parts")]
fn no_parts(ward: &mut Ward) {
    assert_eq!(body(ward).shadow.expect("a shadow").parts, None);
}

/// In sample N, a person and a face of the turn (F2.3b: the face is the turn's
/// rule 1 and the head of the parts).
#[when(
    regex = r"^in sample (\d+) the camera sees a person at ([\d,]+) to ([\d,]+) and a face at ([\d,]+)$"
)]
fn sees_a_person_and_a_face(ward: &mut Ward, n: u32, from: String, to: String, face: String) {
    let (x0, y0) = point(&from);
    let (x1, y1) = point(&to);
    let (fx, fy) = point(&face);
    let mut script = Script::bodies(vec![Detection {
        bbox: Rect::new(x0, y0, x1, y1),
        confidence: 0.9,
        label: Label::Person,
    }]);
    script.faces = Ok(vec![Detection {
        bbox: Rect::new(fx - 10.0, fy - 10.0, fx + 10.0, fy + 10.0),
        confidence: 0.9,
        label: Label::Person,
    }]);
    ward.turn_with(ward.sample(n), script);
}

/// A point written `x,y`.
fn point(text: &str) -> (f32, f32) {
    let (x, y) = text.split_once(',').expect("a point written x,y");
    (x.parse().expect("an x"), y.parse().expect("a y"))
}

/// The one body pose the last turn asked for, and its direction.
fn asked_pose(ward: &Ward) -> Option<scene_look::Up> {
    assert_eq!(
        ward.body_poses.len(),
        1,
        "one body pose in the turn: {:?}",
        ward.body_poses
    );
    ward.body_poses[0]
}

/// The direction the pose is asked to turn to, as a unit vector.
fn direction(dx: f32, dy: f32) -> scene_look::Up {
    scene_look::Up::new(dx, dy).expect("a direction")
}

#[then(regex = r"^the body's pose is asked pointing from (\d+),(\d+) to (\d+),(\d+)$")]
fn pose_points_from_to(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32) {
    let got = asked_pose(ward).expect("the pose is asked with a direction");
    let want = direction(x1 - x0, y1 - y0);
    assert!(
        (got.dx() - want.dx()).abs() < 1e-3 && (got.dy() - want.dy()).abs() < 1e-3,
        "{got:?} is not {want:?}"
    );
}

#[then("the body's pose is asked pointing up the bed")]
fn pose_points_up_the_bed(ward: &mut Ward) {
    // The scenes' bed is 100..500 x 100..400: its foot is the bottom edge, so
    // the way up the bed is straight up the image.
    let got = asked_pose(ward).expect("the pose is asked with a direction");
    assert!(
        got.dx().abs() < 1e-3 && (got.dy() + 1.0).abs() < 1e-3,
        "{got:?} is not up the bed"
    );
}

#[then("the body's pose is asked with no direction")]
fn pose_no_direction(ward: &mut Ward) {
    assert_eq!(asked_pose(ward), None);
}

/// The body whose box is at these corners, in the last turn.
fn body_at(ward: &Ward, x0: f32, y0: f32, x1: f32, y1: f32) -> BodySeen {
    let want = Rect::new(x0, y0, x1, y1);
    let items = &ward.last().output.v5.scene.evidence.bodies.items;
    items
        .iter()
        .find(|b| close(b.bbox, want))
        .cloned()
        .unwrap_or_else(|| panic!("no body at {want:?} in {items:?}"))
}

/// In sample N, a person and a second one, and the pose looks at the first only
/// (one pose per turn: the second is not looked at).
#[when(
    regex = r#"^in sample (\d+) the camera sees a person at ([\d,]+) to ([\d,]+) and a second person at ([\d,]+) to ([\d,]+), the pose sees the first "([^"]+)" and does not look at the second$"#
)]
fn sees_two_and_the_pose_looks_at_the_first(
    ward: &mut Ward,
    n: u32,
    first_from: String,
    first_to: String,
    second_from: String,
    second_to: String,
    figure: String,
) {
    let (ax0, ay0) = point(&first_from);
    let (ax1, ay1) = point(&first_to);
    let (bx0, by0) = point(&second_from);
    let (bx1, by1) = point(&second_to);
    let mut script = Script::bodies(vec![
        Detection {
            bbox: Rect::new(ax0, ay0, ax1, ay1),
            confidence: 0.9,
            label: Label::Person,
        },
        Detection {
            bbox: Rect::new(bx0, by0, bx1, by1),
            confidence: 0.9,
            label: Label::Person,
        },
    ]);
    script.figure = Some(crate::zones::figure(&figure));
    ward.turn_with(ward.sample(n), script);
}

#[then(regex = r#"^the body at (\d+),(\d+) to (\d+),(\d+) is in the "(bed|floor)"$"#)]
fn body_at_is_in(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32, name: String) {
    let body = body_at(ward, x0, y0, x1, y1);
    assert_eq!(body.zone, Some(zone(&name)), "{body:?}");
}

#[then(regex = r#"^the body at (\d+),(\d+) to (\d+),(\d+) has its parts by the step "(\w+)"$"#)]
fn body_at_has_parts_step(ward: &mut Ward, x0: f32, y0: f32, x1: f32, y1: f32, want: String) {
    let parts = body_at(ward, x0, y0, x1, y1)
        .shadow
        .expect("a shadow")
        .parts
        .expect("the body's parts");
    let got = serde_json::to_value(parts.step).expect("a step name");
    assert_eq!(got, serde_json::Value::String(want));
}
