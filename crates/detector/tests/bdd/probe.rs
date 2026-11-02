//! Probe steps (`features/architecture/probe-answerers.feature`): the router
//! sends each probe to the answerer its kind names, and measures the answer
//! once. The steps observe the scripted models (senses) and the counting
//! verifier, and the answer the router returns.
use crate::driver::publish_samples;
use crate::world::{Shell, models::Model};
use cucumber::{given, then, when};
use detector::SystemClock;
use detector::answer as route;
use image::DynamicImage;
use scene_contract::{SampleRef, SessionId, SourceSecs};
use scene_geom::{Quad, Rect};
use scene_look::{Failure, PosePurpose, Probe, Question, Reading, Target, Ticket};

fn sample(n: u64) -> SampleRef {
    SampleRef {
        session: SessionId(1),
        sequence: n,
        source_secs: SourceSecs(n as f64),
    }
}

/// The turn's image and its main detector, when the scenario did not script them.
fn a_turn(w: &mut Shell) {
    if w.image.is_none() {
        w.image = Some(DynamicImage::new_rgb8(160, 96));
    }
    if w.main.is_none() {
        w.main = Some(Model::default());
    }
}

fn held_ticket() -> Ticket {
    Ticket {
        id: 1,
        looked: sample(1),
        target: Target::Held { since: sample(1) },
    }
}

const CROP: Rect = Rect {
    x0: 30.0,
    y0: 30.0,
    x1: 50.0,
    y1: 60.0,
};

#[given("a turn that asks for the main detection over the whole frame")]
fn main_detection(w: &mut Shell) {
    a_turn(w);
    w.probe = Some(Probe::Detect { region: None });
}

#[given("a turn that asks to verify the held body with a question")]
fn verify_held(w: &mut Shell) {
    a_turn(w);
    w.probe = Some(Probe::Verify {
        ticket: held_ticket(),
        region: CROP,
        bed: Quad::from_rect(Rect::new(20.0, 20.0, 100.0, 80.0)),
        question: Question::HeldInBed,
        crops: Vec::new(),
    });
}

#[given("a burst that keeps the crop of this sample")]
fn burst_keeps(w: &mut Shell) {
    a_turn(w);
    w.probe = Some(Probe::Keep {
        ticket: held_ticket(),
        region: CROP,
    });
}

/// Any probe, with the main detector slowed down so that the time can be seen.
#[given("any probe")]
fn any_probe(w: &mut Shell) {
    a_turn(w);
    w.probe = Some(Probe::Detect { region: None });
    w.main_delay_ms = 5;
}

#[given("a profile without a pose model")]
fn without_pose_model(w: &mut Shell) {
    a_turn(w);
    w.pose = None;
    if w.config.is_none() {
        w.parse_config("");
    }
}

#[given("a turn that asks for a pose")]
fn asks_for_pose(w: &mut Shell) {
    a_turn(w);
    w.probe = Some(Probe::Pose {
        bbox: CROP,
        purpose: PosePurpose::Subject,
    });
}

#[when(regex = r"^the shell routes (?:the probe|it)$")]
fn routes(w: &mut Shell) {
    let probe = w.probe.clone().expect("a turn that asks a probe");
    let frame = w.image.clone().expect("an image");
    let mut senses = w.senses();
    let mut verifier = std::mem::take(&mut w.verifier);
    // The one test that measures real time: "the time is measured by the
    // router" sleeps inside the answerer and needs the system's clock. Every
    // other driver test uses FixedClock (ADR 2026-11-02-reloj-inyectado).
    let answer = route(&SystemClock, &mut senses, &mut verifier, &frame, &probe);
    w.verifier = verifier;
    w.answer = Some(answer);
}

#[then("the senses answer it")]
fn senses_answer(w: &mut Shell) {
    assert_eq!(w.sense_calls(), 1, "the senses are asked once");
}

#[then("the senses are not asked anything")]
fn senses_silent(w: &mut Shell) {
    assert_eq!(w.sense_calls(), 0, "no scripted model was called");
}

#[then("the verifier is not asked anything")]
fn verifier_silent(w: &mut Shell) {
    assert_eq!(
        (w.verifier.asks, w.verifier.keeps),
        (0, 0),
        "the verifier took nothing"
    );
}

#[then("the verifier takes the request")]
fn verifier_takes(w: &mut Shell) {
    assert_eq!((w.verifier.asks, w.verifier.keeps), (1, 0));
    assert_eq!(answer(w).0, Ok(Reading::Dispatched));
}

#[then("the verifier keeps the crop")]
fn verifier_keeps(w: &mut Shell) {
    assert_eq!((w.verifier.asks, w.verifier.keeps), (0, 1));
    assert!(matches!(answer(w).0, Ok(Reading::Kept { .. })));
}

/// The answer the router returned, as the last "When" left it.
fn answer(w: &Shell) -> &(Result<Reading, Failure>, scene_look::Spent) {
    w.answer.as_ref().expect("the router answered")
}

#[then("the answer carries one measured time")]
fn one_measured_time(w: &mut Shell) {
    let spent = answer(w).1.0;
    assert!(spent.is_finite() && spent >= 0.0, "measured: {spent} ms");
}

#[then("the time is measured by the router, not by the answerer")]
fn measured_by_router(w: &mut Shell) {
    // The answerer sleeps `main_delay_ms` inside the call: a clock around the
    // call sees it; the answerer returns no time of its own to report.
    let spent = answer(w).1.0;
    let delay = w.main_delay_ms as f64;
    assert!(
        spent >= delay,
        "the router measured {spent} ms, the answerer slept {delay} ms"
    );
}

#[then("the answer is a model failure")]
fn model_failure(w: &mut Shell) {
    assert_eq!(answer(w).0, Err(Failure::Model));
}

#[then("the turn still concludes with a scene")]
fn turn_concludes(w: &mut Shell) {
    let config = w.config().clone();
    let concluded = publish_samples(w, &config, 1, 1)
        .iter()
        .any(|(_, json)| json["schema"] == "calipso.scene.v4");
    assert!(concluded, "sample 1 published no scene");
}
