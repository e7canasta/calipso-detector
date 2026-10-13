//! Verification steps (`features/verification`): the slow verifier, scripted.
//! It takes each request and answers after a latency in source time; the
//! ward delivers the answer before the first sample at or past it.

use cucumber::{given, then};
use scene_core::contract::{
    HeldCheckState, LookKind, LookOutcome, LookResult, SkipReason, Verifier,
};
use scene_core::look::Answer;
use scene_core::policy::{HeldVerifyPolicy, VerifyPolicy};

use crate::attention::scene;
use crate::ward::Ward;

#[given(expr = "held occupants are verified after {int} seconds")]
fn held_verified_after(ward: &mut Ward, secs: u32) {
    ward.policy.verify = Some(VerifyPolicy {
        held: Some(HeldVerifyPolicy {
            after_secs: f64::from(secs),
            valid_secs: None,
        }),
        timeout_secs: None,
        max_per_hour: None,
    });
}

fn verify(ward: &mut Ward) -> &mut VerifyPolicy {
    ward.policy
        .verify
        .as_mut()
        .expect("\"held occupants are verified after …\" first")
}

#[given(expr = "verification requests expire after {int} seconds")]
fn requests_expire(ward: &mut Ward, secs: u32) {
    verify(ward).timeout_secs = Some(f64::from(secs));
}

#[given(expr = "the verifier may be asked {int} times per hour")]
fn budget(ward: &mut Ward, times: u32) {
    verify(ward).max_per_hour = Some(times);
}

#[given(expr = "a held occupant's check holds {int} seconds")]
fn check_holds(ward: &mut Ward, secs: u32) {
    if let Some(held) = verify(ward).held.as_mut() {
        held.valid_secs = Some(f64::from(secs));
    }
}

#[given("the verifier never answers")]
fn never_answers(ward: &mut Ward) {
    ward.verifier.answer = None;
}

fn skipped(ward: &Ward, n: u32, reason: SkipReason) {
    let scene = scene(ward, n);
    assert!(
        scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Verify
                && matches!(l.outcome, LookOutcome::Skipped { reason: r } if r == reason)),
        "sample {n} did not skip the verifier with {reason:?}: {:?}",
        scene.attention.looks
    );
}

#[then(expr = "in sample {int} the verifier is busy")]
fn busy(ward: &mut Ward, n: u32) {
    skipped(ward, n, SkipReason::VerifyBusy);
}

#[then(expr = "in sample {int} the verifier's budget is spent")]
fn budget_spent(ward: &mut Ward, n: u32) {
    skipped(ward, n, SkipReason::BudgetSpent);
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
