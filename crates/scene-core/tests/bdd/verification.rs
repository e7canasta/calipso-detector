//! Verification steps (`features/verification`): the slow verifier, scripted.
//! It takes each request and answers after a latency in source time; the
//! ward delivers the answer before the first sample at or past it.

use cucumber::{given, then};
use scene_contract::{HeldCheckState, LookKind, LookOutcome, LookResult, SkipReason, Verifier};
use scene_core::look::Answer;
use scene_core::policy::{Band, Burst, HeldVerifyPolicy, SharpnessPolicy, VerifyPolicy};

use crate::attention::scene;
use crate::ward::Ward;
use scene_core::units::Seconds;

#[given(expr = "held occupants are verified after {int} seconds")]
fn held_verified_after(ward: &mut Ward, secs: u32) {
    ward.policy.verify = Some(VerifyPolicy {
        held: Some(HeldVerifyPolicy {
            after_secs: Seconds::new(f64::from(secs)).expect("whole seconds"),
            valid_secs: None,
        }),
        ..VerifyPolicy::default()
    });
}

#[given(expr = "disparities between {float} and {float} are doubtful")]
fn doubtful_disparity(ward: &mut Ward, lo: f32, hi: f32) {
    ward.policy
        .verify
        .get_or_insert_with(VerifyPolicy::default)
        .doubtful_disparity = Some(Band { lo, hi });
}

#[given(expr = "feet within {float} of the bed's sides are doubtful")]
fn doubtful_feet(ward: &mut Ward, margin: f32) {
    ward.policy
        .verify
        .get_or_insert_with(VerifyPolicy::default)
        .doubtful_feet_margin = Some(margin);
}

fn burst(frames: u8, every_secs: f64) -> Burst {
    Burst {
        frames,
        every_secs: Seconds::new(every_secs).expect("a time in source seconds"),
    }
}

#[given(expr = "the disparity is asked with {int} frames {int} seconds apart")]
fn disparity_burst(ward: &mut Ward, frames: u8, secs: u32) {
    ward.policy
        .verify
        .get_or_insert_with(VerifyPolicy::default)
        .frames
        .sitting = burst(frames, f64::from(secs));
}

#[given(expr = "the feet are asked with {int} frames {int} seconds apart")]
fn feet_burst(ward: &mut Ward, frames: u8, secs: u32) {
    ward.policy
        .verify
        .get_or_insert_with(VerifyPolicy::default)
        .frames
        .leaving = burst(frames, f64::from(secs));
}

#[then(expr = "in sample {int} a crop is kept for the verifier")]
fn crop_kept(ward: &mut Ward, n: u32) {
    let scene = scene(ward, n);
    assert!(
        scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Keep
                && matches!(l.result(), Some(LookResult::Kept { counted: true, .. }))),
        "no crop kept in sample {n}: {:?}",
        scene.attention.looks
    );
}

#[given(
    expr = "a crop counts if it is {float} of the median of the camera's last {int}, waiting at most {int} seconds"
)]
fn sharpness_policy(ward: &mut Ward, ratio: f32, window: u16, wait: u32) {
    ward.policy
        .verify
        .get_or_insert_with(VerifyPolicy::default)
        .sharpness = Some(SharpnessPolicy {
        min_ratio: ratio,
        window,
        max_wait_secs: Seconds::new(f64::from(wait)).expect("whole seconds"),
    });
}

#[given(expr = "the crops of samples {int} to {int} are blurred")]
fn blurred(ward: &mut Ward, from: u32, to: u32) {
    ward.verifier.blurred.extend(from..=to);
}

#[then(expr = "in sample {int} a blurred crop is kept, not counted")]
fn blurred_kept(ward: &mut Ward, n: u32) {
    let scene = scene(ward, n);
    assert!(
        scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Keep
                && matches!(l.result(), Some(LookResult::Kept { counted: false, .. }))),
        "no blurred crop kept in sample {n}: {:?}",
        scene.attention.looks
    );
}

#[then(expr = "in sample {int} nothing is kept for the verifier")]
fn nothing_kept(ward: &mut Ward, n: u32) {
    let scene = scene(ward, n);
    assert!(
        !scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Keep),
        "sample {n} kept a crop: {:?}",
        scene.attention.looks
    );
}

#[then(expr = "in sample {int} the burst is dropped, resolved")]
fn burst_resolved(ward: &mut Ward, n: u32) {
    skipped(ward, n, SkipReason::BurstResolved);
}

#[then(expr = "in sample {int} the burst is dropped, stale")]
fn burst_stale(ward: &mut Ward, n: u32) {
    skipped(ward, n, SkipReason::BurstStale);
}

#[then(expr = "in sample {int} a verification of the feet is asked")]
fn feet_asked(ward: &mut Ward, n: u32) {
    verification_asked(ward, n);
}

#[then(expr = "in sample {int} a verification of the disparity is asked")]
fn disparity_asked(ward: &mut Ward, n: u32) {
    verification_asked(ward, n);
}

#[then(expr = "in sample {int} nothing is asked to the verifier")]
fn nothing_asked(ward: &mut Ward, n: u32) {
    let scene = scene(ward, n);
    assert!(
        !scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Verify && l.result().is_some()),
        "sample {n} asked the verifier: {:?}",
        scene.attention.looks
    );
}

#[given("the verifier refuses to take its requests")]
fn refuses_requests(ward: &mut Ward) {
    ward.verifier.refuses_requests = true;
}

#[given("the verifier refuses to keep the crops")]
fn refuses_crops(ward: &mut Ward) {
    ward.verifier.refuses_crops = true;
}

/// Sample `n` has a look of `kind` that failed: the shell's port refused it.
fn failed(ward: &Ward, n: u32, kind: LookKind) {
    let scene = scene(ward, n);
    assert!(
        scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == kind && matches!(l.result(), Some(LookResult::Failed { .. }))),
        "sample {n} has no failed {kind:?}: {:?}",
        scene.attention.looks
    );
}

#[then(expr = "in sample {int} a verification of the disparity fails")]
fn disparity_fails(ward: &mut Ward, n: u32) {
    failed(ward, n, LookKind::Verify);
}

#[then(expr = "in sample {int} a verification of the feet fails")]
fn feet_fails(ward: &mut Ward, n: u32) {
    failed(ward, n, LookKind::Verify);
}

#[then(expr = "in sample {int} a crop fails")]
fn crop_fails(ward: &mut Ward, n: u32) {
    failed(ward, n, LookKind::Keep);
}

fn verify(ward: &mut Ward) -> &mut VerifyPolicy {
    ward.policy
        .verify
        .as_mut()
        .expect("\"held occupants are verified after …\" first")
}

#[given(expr = "verification requests expire after {int} seconds")]
fn requests_expire(ward: &mut Ward, secs: u32) {
    verify(ward).timeout_secs = Some(Seconds::new(f64::from(secs)).expect("whole seconds"));
}

#[given(expr = "the verifier may be asked {int} times per hour")]
fn budget(ward: &mut Ward, times: u32) {
    verify(ward).max_per_hour = Some(times);
}

#[given(expr = "a held occupant's check holds {int} seconds")]
fn check_holds(ward: &mut Ward, secs: u32) {
    if let Some(held) = verify(ward).held.as_mut() {
        held.valid_secs = Some(Seconds::new(f64::from(secs)).expect("whole seconds"));
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
