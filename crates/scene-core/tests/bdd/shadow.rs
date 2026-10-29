//! The geometry's shadow (`features/contract/shadow.feature`; white book,
//! volume VII, ch. 11): what a geometry that does not decide yet sees,
//! published next to what is decided.

use cucumber::{given, then};
use scene_core::geom::{Quad, Rect};
use scene_core::map::MapShadow;
use scene_core::policy::ShadowPolicy;
use scene_core::{Monitor, Output};

use crate::ward::Ward;

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
    ward.policy.shadow = Some(ShadowPolicy { near_m });
}

fn shadow(ward: &Ward) -> MapShadow {
    ward.last()
        .output
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
    assert_eq!(ward.last().output.scene.map.shadow, None);
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
                .drive(|request| (script.respond(request), scene_core::look::Spent(1.0)))
        })
        .collect();
    assert_eq!(without.len(), ward.turns.len());
    for (bare, turn) in without.iter().zip(&ward.turns) {
        let mut with = turn.output.scene.as_ref().clone();
        assert!(with.map.shadow.is_some(), "the shadow was asked for");
        with.map.shadow = None;
        assert_eq!(*bare.scene, with, "the scene at {:?} differs", bare.at);
        assert_eq!(
            bare.events, turn.output.events,
            "the events at {:?} differ",
            bare.at
        );
    }
}
