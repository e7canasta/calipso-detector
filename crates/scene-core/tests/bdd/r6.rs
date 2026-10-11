//! R6 steps (specs 601–602): the acquisition policy. What the core decides to
//! look at, where, in what order and why not, with scripted senses. The real
//! models are shell (611–613); the decision is checked here.

use cucumber::{given, then};
use scene_core::contract::{
    FrameInfo, LookKind, LookOutcome, LookResult, Provenance, RejectReason, Scene, SkipReason,
};
use scene_core::policy::FacePolicy;
use scene_core::{Monitor, Output};

use crate::ward::{BED, FRAME, Ward};
use golden::world::{HEADBOARD, in_bed};

// --- Background

#[given("the bed's headboard delimited")]
fn headboard(ward: &mut Ward) {
    ward.map.as_mut().expect("the bed first").headboard = Some(HEADBOARD);
}

#[given("the bed's headboard is not delimited")]
fn no_headboard(ward: &mut Ward) {
    ward.map.as_mut().expect("the bed first").headboard = None;
}

#[given(expr = "there is a face detector with minimum confidence {float}")]
fn faces(ward: &mut Ward, min_confidence: f32) {
    ward.policy.faces = Some(FacePolicy { min_confidence });
}

#[given("there is no reinforcement detector")]
fn no_reinforcement(ward: &mut Ward) {
    ward.policy.reinforce = None;
}

#[given("the camera delivers images of another size than the delimited bed's")]
fn other_size(ward: &mut Ward) {
    ward.map.as_mut().expect("the bed first").resolution = Some(FRAME);
    ward.frame = Some(FrameInfo {
        width: FRAME.width * 2,
        height: FRAME.height * 2,
    });
}

// --- What was looked at

/// The scene of sample `n`.
pub(crate) fn scene(ward: &Ward, n: u32) -> &Scene {
    let at = ward.sample(n);
    &ward
        .turns
        .iter()
        .rev()
        .find(|t| t.output.at == at)
        .unwrap_or_else(|| panic!("there was no sample {n}"))
        .output
        .scene
}

/// The region looked at with `kind` in sample `n`.
fn region(ward: &Ward, n: u32, kind: LookKind) -> scene_core::geom::Rect {
    let scene = scene(ward, n);
    scene
        .attention
        .looks
        .iter()
        .find_map(|l| match (&l.kind, &l.outcome) {
            (k, LookOutcome::Asked { region, .. }) if *k == kind => *region,
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "in sample {n} nothing was looked at with {kind:?}: {:?}",
                scene.attention
            )
        })
}

pub(crate) fn skipped(ward: &Ward, n: u32, kind: LookKind) -> Option<SkipReason> {
    scene(ward, n)
        .attention
        .looks
        .iter()
        .find_map(|l| match (&l.kind, &l.outcome) {
            (k, LookOutcome::Skipped { reason }) if *k == kind => Some(*reason),
            _ => None,
        })
}

#[then(expr = "in sample {int} the looks were {string}")]
fn looks(ward: &mut Ward, n: u32, list: String) {
    let expected: Vec<String> = list.split(", ").map(String::from).collect();
    let asked: Vec<String> = scene(ward, n)
        .attention
        .looks
        .iter()
        .filter(|l| matches!(l.outcome, LookOutcome::Asked { .. }))
        .map(|l| {
            serde_json::to_value(l.kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(asked, expected);
}

#[then(expr = "in sample {int} reinforcement was requested over the grown bed")]
fn reinforcement_on_the_bed(ward: &mut Ward, n: u32) {
    let padding = ward
        .policy
        .reinforce
        .expect("there is reinforcement")
        .bed_padding;
    assert_eq!(region(ward, n, LookKind::Reinforce), BED.expand(padding));
}

#[then(
    regex = r"^in sample (\d+) reinforcement was not requested because (nobody was in bed|this loss was already reinforced|there is no bed geometry)$"
)]
fn reinforcement_skipped(ward: &mut Ward, n: u32, reason: String) {
    let expected = match reason.as_str() {
        "nobody was in bed" => SkipReason::NoSubjectInBed,
        "this loss was already reinforced" => SkipReason::LatchSpent,
        _ => SkipReason::NoBedGeometry,
    };
    assert_eq!(skipped(ward, n, LookKind::Reinforce), Some(expected));
}

#[then(expr = "in sample {int} the subject was seen in that sample")]
fn current_subject(ward: &mut Ward, n: u32) {
    let subject = scene(ward, n).subject.as_ref().expect("there is a subject");
    assert_eq!(subject.provenance, Provenance::Current);
}

#[then(expr = "in sample {int} faces were looked for on the body seen in bed")]
fn faces_on_the_body(ward: &mut Ward, n: u32) {
    assert_eq!(region(ward, n, LookKind::Faces), in_bed(0).bbox);
}

#[then(expr = "in sample {int} faces were looked for at the headboard")]
fn faces_at_the_headboard(ward: &mut Ward, n: u32) {
    assert_eq!(region(ward, n, LookKind::Faces), HEADBOARD);
}

#[then(expr = "in sample {int} faces were not looked for because there is nowhere to look")]
fn faces_skipped(ward: &mut Ward, n: u32) {
    assert_eq!(
        skipped(ward, n, LookKind::Faces),
        Some(SkipReason::NoFaceRegion)
    );
}

// --- What was accepted

#[then(regex = r"^the last sample accepted (\d+) faces?$")]
fn faces_accepted(ward: &mut Ward, n: usize) {
    let faces = &ward.last().output.scene.evidence.faces;
    assert_eq!(faces.items.len(), n, "{faces:?}");
}

#[then("the face of the last sample is on the subject")]
fn face_on_the_subject(ward: &mut Ward) {
    let faces = &ward.last().output.scene.evidence.faces;
    assert!(
        faces.items.iter().any(|f| f.on_subject),
        "no face on the subject: {faces:?}"
    );
}

#[then("the last sample discarded a face for low confidence")]
fn face_discarded(ward: &mut Ward) {
    let scene = &ward.last().output.scene;
    let rejections = scene
        .attention
        .looks
        .iter()
        .find_map(|l| match (&l.kind, l.result()) {
            (LookKind::Faces, Some(LookResult::Read { rejected, .. })) => Some(rejected.clone()),
            _ => None,
        })
        .expect("faces were looked for");
    assert!(
        rejections.contains(&RejectReason::LowConfidence),
        "{rejections:?}"
    );
}

// --- The night replays without a camera

#[then("the same night, replayed with the same readings, gives the same scenes and events")]
fn replays(ward: &mut Ward) {
    let mut monitor = Monitor::new(
        ward.policy.clone(),
        ward.map.clone().expect("there is a map"),
    );
    let replayed: Vec<Output> = ward
        .recording
        .iter()
        .map(|(input, script)| {
            monitor
                .handle(input.clone())
                .drive(|request| (script.respond(&request.probe), scene_core::look::Spent(1.0)))
        })
        .collect();
    let original: Vec<&Output> = ward.turns.iter().map(|t| &t.output).collect();
    assert_eq!(replayed.len(), original.len());
    for (a, b) in replayed.iter().zip(original) {
        assert_eq!(a, b, "the replayed night differs at {:?}", a.at);
    }
}
