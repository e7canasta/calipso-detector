//! Verification steps (`features/verification`): the slow verifier, scripted.
//! It takes each request and answers after a latency in source time; the
//! ward delivers the answer before the first sample at or past it.

use cucumber::{given, then};
use scene_core::contract::{HeldCheckState, LookKind, LookResult, Verifier};
use scene_core::look::Answer;
use scene_core::policy::{HeldVerifyPolicy, VerifyPolicy};

use crate::attention::scene;
use crate::ward::Ward;

#[given(expr = "held occupants are verified after {int} seconds")]
fn held_verified_after(ward: &mut Ward, secs: u32) {
    ward.policy.verify = Some(VerifyPolicy {
        held: Some(HeldVerifyPolicy {
            after_secs: f64::from(secs),
        }),
    });
}

#[given(expr = "the verifier answers after {int} seconds of source time")]
fn verifier_latency(ward: &mut Ward, secs: u32) {
    ward.verifier.latency_secs = f64::from(secs);
}

#[given(expr = "the verifier answers {string}")]
fn verifier_answers(ward: &mut Ward, answer: String) {
    ward.verifier.answer = Some(match answer.as_str() {
        "yes" => Answer::Yes,
        "no" => Answer::No,
        "unsure" => Answer::Unsure,
        other => panic!("unknown answer: {other}"),
    });
}

fn state(name: &str) -> HeldCheckState {
    match name {
        "unverified" => HeldCheckState::Unverified,
        "confirmed" => HeldCheckState::Confirmed,
        "not_confirmed" => HeldCheckState::NotConfirmed,
        other => panic!("unknown held check: {other}"),
    }
}

#[then(expr = "in sample {int} a verification of the held occupant is asked")]
fn verification_asked(ward: &mut Ward, n: u32) {
    let scene = scene(ward, n);
    assert!(
        scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Verify
                && matches!(l.result(), Some(LookResult::Dispatched))),
        "no verification asked in sample {n}: {:?}",
        scene.attention.looks
    );
}

#[then(regex = r#"^in sample (\d+) the held occupant is "([a-z_]+)"$"#)]
fn held_check(ward: &mut Ward, n: u32, expected: String) {
    let held = &scene(ward, n).bed.held;
    let check = held
        .first()
        .unwrap_or_else(|| panic!("nobody held in sample {n}"))
        .check
        .unwrap_or_else(|| panic!("the held occupant has no check in sample {n}"));
    assert_eq!(check.state, state(&expected), "{check:?}");
}

#[then(
    regex = r#"^in sample (\d+) the held occupant is "([a-z_]+)" by "vlm", looked in sample (\d+)$"#
)]
fn held_check_by(ward: &mut Ward, n: u32, expected: String, looked: u32) {
    let check = scene(ward, n)
        .bed
        .held
        .first()
        .and_then(|h| h.check)
        .unwrap_or_else(|| panic!("no held check in sample {n}"));
    assert_eq!(check.state, state(&expected), "{check:?}");
    assert_eq!(check.by, Some(Verifier::Vlm), "{check:?}");
    assert_eq!(check.looked, Some(ward.sample(looked)), "{check:?}");
}
