//! Subject steps (`features/attention`): the pose, depth and calibration
//! policy. When the core decides to take the subject's pose, measure their
//! depth or learn the empty bed, with scripted senses. The real models are
//! shell (`features/shell`); the decision is checked here.

use crate::published::Body;
use cucumber::{given, then};
use scene_core::contract::Calibration;
use scene_core::contract::{
    LookKind, LookOutcome, LookResult, Provenance, Scene, SkipReason, Zone,
};
use scene_core::geom::Rect;
use scene_core::policy::{CalibrationPolicy, EnrichPolicy};

use crate::attention::{scene, skipped};
use crate::ward::{BED, Ward, later};

// --- Background

#[given("there is a pose model")]
fn pose_model(ward: &mut Ward) {
    ward.policy.enrich = Some(EnrichPolicy {
        move_threshold: 0.1,
        depth: false,
        min_keypoint_score: 0.3,
        posture_tolerance: 0.05,
        max_pose_age_secs: None,
        crop_pad: 0.0,
    });
}

#[given("there is no pose model")]
fn no_pose_model(ward: &mut Ward) {
    ward.policy.enrich = None;
}

#[given(expr = "the motion threshold is {float} of the subject's diagonal")]
fn threshold(ward: &mut Ward, move_threshold: f32) {
    ward.policy
        .enrich
        .as_mut()
        .expect("\"there is a pose model\" first")
        .move_threshold = move_threshold;
}

#[given(expr = "the pose crop grows the subject's box by {float}")]
fn pose_crop_pad(ward: &mut Ward, crop_pad: f32) {
    ward.policy
        .enrich
        .as_mut()
        .expect("\"there is a pose model\" first")
        .crop_pad = crop_pad;
}

#[given("there is a depth model")]
fn depth_model(ward: &mut Ward) {
    // Without a pose there is no subject depth; calibration turns on the
    // bed's.
    if let Some(enrich) = ward.policy.enrich.as_mut() {
        enrich.depth = true;
    }
}

#[given("there is no depth model")]
fn no_depth_model(ward: &mut Ward) {
    if let Some(enrich) = ward.policy.enrich.as_mut() {
        enrich.depth = false;
    }
}

#[given(expr = "the pose has {int} keypoints with enough score out of {int}")]
fn keypoints(ward: &mut Ward, good: usize, total: usize) {
    ward.keypoints = Some((good, total));
}

#[given(expr = "the bed is calibrated with a retry of {int} seconds of source time")]
fn calibrates(ward: &mut Ward, retry_secs: u32) {
    ward.policy.calibration = Some(CalibrationPolicy {
        retry_secs: f64::from(retry_secs),
    });
}

#[given("the bed has no depth reference")]
fn no_reference(ward: &mut Ward) {
    let map = ward.map.as_mut().expect("the bed first");
    map.bed_reference = None;
    map.calibration_attempt = None;
}

// --- What was decided

/// The scenes in which the pose was requested, with the requested region.
fn poses_requested(ward: &Ward) -> impl Iterator<Item = (&Scene, Rect)> {
    ward.turns.iter().filter_map(|t| {
        let scene: &Scene = &t.output.scene;
        scene.attention.looks.iter().find_map(|l| match l.outcome {
            LookOutcome::Asked {
                region: Some(region),
                ..
            } if l.kind == LookKind::Pose => Some((scene, region)),
            _ => None,
        })
    })
}

#[then("the first pose was requested without a reference, over the subject's box")]
fn first_pose(ward: &mut Ward) {
    // The first turn that noted something about the pose: requested, never
    // skipped for stillness, because there was nothing to compare with yet.
    let first = ward
        .turns
        .iter()
        .map(|t| -> &Scene { &t.output.scene })
        .find_map(|s| {
            s.attention
                .looks
                .iter()
                .find(|l| l.kind == LookKind::Pose)
                .map(|l| (s, l))
        })
        .expect("the pose was never considered");
    let (scene, look) = first;
    let LookOutcome::Asked { region, .. } = look.outcome else {
        panic!("the first consideration of the pose was not a request: {look:?}");
    };
    let subject = scene
        .subject
        .as_ref()
        .expect("the pose was requested without a subject");
    assert_eq!(region, Some(subject.bbox));
}

#[then(expr = "in sample {int} the pose was requested over the subject's box grown by {float}")]
fn pose_with_margin(ward: &mut Ward, n: u32, pad: f32) {
    let scene = scene(ward, n);
    let region = scene
        .attention
        .looks
        .iter()
        .find_map(|l| match l.outcome {
            LookOutcome::Asked { region, .. } if l.kind == LookKind::Pose => region,
            _ => None,
        })
        .expect("the pose was not requested");
    let subject = scene.subject.as_ref().expect("no subject");
    assert_eq!(region, subject.bbox.expand(pad));
}

#[then("the pose was requested again on moving from the door to the bed")]
fn pose_in_bed(ward: &mut Ward) {
    let requested: Vec<_> = poses_requested(ward).collect();
    assert!(
        requested.len() >= 2,
        "the pose was requested {} times",
        requested.len()
    );
    let (first, _) = requested[0];
    assert_eq!(first.subject.as_ref().map(|s| s.zone), Some(Zone::Floor));
    assert!(
        requested[1..]
            .iter()
            .any(|(s, _)| s.subject.as_ref().is_some_and(|s| s.zone == Zone::Bed)),
        "no pose request with the subject in bed"
    );
}

/// Was the pose requested in sample `n`?
fn asked_pose(ward: &Ward, n: u32) -> bool {
    scene(ward, n)
        .attention
        .looks
        .iter()
        .any(|l| l.kind == LookKind::Pose && matches!(l.outcome, LookOutcome::Asked { .. }))
}

#[then(expr = "the pose was not requested in samples {int} to {int}")]
fn no_pose_in(ward: &mut Ward, from: u32, to: u32) {
    let requested: Vec<u32> = (from..=to).filter(|&n| asked_pose(ward, n)).collect();
    assert!(
        requested.is_empty(),
        "the pose was requested in samples {requested:?}"
    );
}

#[then(regex = r"^in sample (\d+) the pose is stable since (before )?sample (\d+)$")]
fn stable_pose(ward: &mut Ward, n: u32, before: String, m: u32) {
    let pose = scene(ward, n)
        .subject
        .as_ref()
        .and_then(|s| s.pose.as_ref())
        .unwrap_or_else(|| panic!("in sample {n} there is no pose"));
    let Provenance::Stable { since } = pose.provenance else {
        panic!(
            "the pose of sample {n} is not stable: {:?}",
            pose.provenance
        );
    };
    if before.is_empty() {
        assert_eq!(since, ward.sample(m), "stable since another sample");
    } else {
        assert!(
            later(ward.sample(m), since),
            "the pose is stable since {since:?}, not since before sample {m}"
        );
    }
}

#[then(expr = "between samples {int} and {int} the pose was requested only in {int}")]
fn pose_only_in(ward: &mut Ward, from: u32, to: u32, m: u32) {
    let requested: Vec<u32> = (from..=to).filter(|&n| asked_pose(ward, n)).collect();
    assert_eq!(requested, vec![m]);
}

#[then(
    regex = r"^in sample (\d+) the pose was not requested because (the subject did not move|the subject was not seen in this sample|it was already seen as a doubtful body)$"
)]
fn pose_skipped(ward: &mut Ward, n: u32, reason: String) {
    let expected = match reason.as_str() {
        "the subject did not move" => SkipReason::SubjectStill,
        "it was already seen as a doubtful body" => SkipReason::PosedBefore,
        _ => SkipReason::NoCurrentSubject,
    };
    assert_eq!(skipped(ward, n, LookKind::Pose), Some(expected));
}

#[then(expr = "in sample {int} there is no subject")]
fn no_subject_in(ward: &mut Ward, n: u32) {
    let subject = &scene(ward, n).subject;
    assert!(subject.is_none(), "there is a subject: {subject:?}");
}

#[then(expr = "in sample {int} there is no pose")]
fn there_is_no_pose(ward: &mut Ward, n: u32) {
    let pose = scene(ward, n)
        .subject
        .as_ref()
        .and_then(|s| s.pose.as_ref());
    assert!(pose.is_none(), "there is a pose: {pose:?}");
}

#[then(expr = "in sample {int} the pose is new")]
fn new_pose(ward: &mut Ward, n: u32) {
    let pose = scene(ward, n)
        .subject
        .as_ref()
        .and_then(|s| s.pose.as_ref())
        .unwrap_or_else(|| panic!("in sample {n} there is no pose"));
    assert_eq!(pose.provenance, Provenance::Current);
    assert!(asked_pose(ward, n), "a new pose without requesting it");
}

#[then(expr = "the first pose after sample {int} was requested without a reference")]
fn first_pose_after(ward: &mut Ward, m: u32) {
    let since = ward.sample(m);
    let (scene, look) = ward
        .turns
        .iter()
        .filter(|t| later(t.output.at, since))
        .map(|t| -> &Scene { &t.output.scene })
        .find_map(|s| {
            s.attention
                .looks
                .iter()
                .find(|l| l.kind == LookKind::Pose)
                .map(|l| (s, l))
        })
        .unwrap_or_else(|| panic!("after sample {m} the pose was never considered"));
    let LookOutcome::Asked { region, .. } = look.outcome else {
        panic!("the first consideration of the pose after {m} was not a request: {look:?}");
    };
    assert_eq!(region, scene.subject.as_ref().map(|s| s.bbox));
}

#[then(expr = "in sample {int}, seen again by the main detector, the pose was requested")]
fn pose_with_the_main_detector(ward: &mut Ward, n: u32) {
    assert!(
        asked_pose(ward, n),
        "the pose was not requested: the motion reference was taken from reinforcement's box"
    );
}

/// The subject's depth in sample `n`.
fn depth(ward: &Ward, n: u32) -> Option<scene_core::contract::SubjectDepth> {
    scene(ward, n).subject.as_ref().and_then(|s| s.depth)
}

#[then(regex = r"^in sample (\d+) the subject's depth is (new|stable since before sample (\d+))$")]
fn depth_is(ward: &mut Ward, n: u32, which: String, before: String) {
    let depth = depth(ward, n).unwrap_or_else(|| panic!("in sample {n} there is no depth"));
    if which == "new" {
        assert_eq!(depth.provenance, Provenance::Current);
        return;
    }
    let m: u32 = before.parse().expect("sample");
    let Provenance::Stable { since } = depth.provenance else {
        panic!(
            "the depth of sample {n} is not stable: {:?}",
            depth.provenance
        );
    };
    assert!(later(ward.sample(m), since), "stable since {since:?}");
}

#[then(expr = "in sample {int} the subject has no depth")]
fn no_depth_in(ward: &mut Ward, n: u32) {
    assert_eq!(depth(ward, n), None);
}

#[given(expr = "depth is measured over the bed grown by {float}")]
fn depth_crop(ward: &mut Ward, pad: f32) {
    ward.policy.depth_pad = Some(pad);
}

/// The region of the depth look (`depth`, the subject's; `calibration`, the
/// bed's) asked in sample `n`; `None`, the whole frame.
fn depth_region(ward: &Ward, n: u32, which: &str) -> Option<Rect> {
    let kind = match which {
        "depth" => LookKind::Depth,
        _ => LookKind::Calibrate,
    };
    scene(ward, n)
        .attention
        .looks
        .iter()
        .find_map(|l| match l.outcome {
            LookOutcome::Asked { region, .. } if l.kind == kind => Some(region),
            _ => None,
        })
        .unwrap_or_else(|| panic!("in sample {n} {which} was not requested"))
}

#[then(
    regex = r"^in sample (\d+) (depth|calibration) was requested over the bed grown by ([\d.]+)$"
)]
fn depth_over_the_bed(ward: &mut Ward, n: u32, which: String, pad: f32) {
    assert_eq!(depth_region(ward, n, &which), Some(BED.expand(pad)));
}

#[then(regex = r"^in sample (\d+) (depth|calibration) was requested over the whole frame$")]
fn depth_over_the_frame(ward: &mut Ward, n: u32, which: String) {
    assert_eq!(depth_region(ward, n, &which), None);
}

#[then(expr = "in sample {int} depth was requested at {int} keypoints and {int} bed point(s)")]
fn depth_at_points(ward: &mut Ward, n: u32, keypoints: u32, bed: u32) {
    let points = keypoints + bed;
    let read = scene(ward, n)
        .attention
        .looks
        .iter()
        .find_map(|l| match &l.outcome {
            LookOutcome::Asked {
                result: LookResult::Read { candidates, .. },
                ..
            } if l.kind == LookKind::Depth => Some(*candidates),
            _ => None,
        });
    assert_eq!(read, Some(points), "{:?}", scene(ward, n).attention);
}

#[then(expr = "in sample {int} depth was not requested because there is no pose")]
fn depth_without_pose(ward: &mut Ward, n: u32) {
    assert_eq!(skipped(ward, n, LookKind::Depth), Some(SkipReason::NoPose));
}

// --- Calibration

/// The samples in which calibration was requested, across the scenario.
fn calibrations(ward: &Ward) -> Vec<u64> {
    ward.turns
        .iter()
        .filter(|t| {
            t.output.scene.attention.looks.iter().any(|l| {
                l.kind == LookKind::Calibrate && matches!(l.outcome, LookOutcome::Asked { .. })
            })
        })
        .map(|t| t.output.at.sequence)
        .collect()
}

/// The samples in which the bed was learned (`bed_calibrated`).
fn learned(ward: &Ward) -> Vec<u64> {
    ward.events()
        .filter(|e| matches!(e.body, Body::BedCalibrated { .. }))
        .map(|e| e.at.sequence)
        .collect()
}

/// "1", "1 and 11", "1, 5 and 11".
fn list(text: &str) -> Vec<u64> {
    text.split([',', ' '])
        .filter(|p| !p.is_empty() && *p != "and")
        .map(|p| p.parse().unwrap_or_else(|_| panic!("sample: {p}")))
        .collect()
}

#[then(expr = "the bed was calibrated in sample {int}")]
fn calibrated_in(ward: &mut Ward, n: u64) {
    assert!(learned(ward).contains(&n), "learned in {:?}", learned(ward));
}

#[then("the bed was calibrated only once")]
fn calibrated_once(ward: &mut Ward) {
    assert_eq!(learned(ward).len(), 1, "learned in {:?}", learned(ward));
}

#[then(regex = r"^calibration was requested in samples (.+)$")]
fn calibrate_in(ward: &mut Ward, samples: String) {
    assert_eq!(calibrations(ward), list(&samples));
}

#[then("calibration was not requested")]
fn no_calibration(ward: &mut Ward) {
    assert_eq!(calibrations(ward), Vec::<u64>::new());
}

#[then("calibration was not requested before the room was left with 0 occupants")]
fn calibrate_with_the_room_empty(ward: &mut Ward) {
    for t in &ward.turns {
        let scene = &t.output.scene;
        let asked = scene.attention.looks.iter().any(|l| {
            l.kind == LookKind::Calibrate && matches!(l.outcome, LookOutcome::Asked { .. })
        });
        assert!(
            !asked || scene.room.occupants == 0,
            "calibration was requested in sample {} with {} occupants",
            t.output.at.sequence,
            scene.room.occupants
        );
    }
}

#[then(
    regex = r"^in sample (\d+) the bed was not calibrated because (someone is in the room|faces are in sight|there is no bed geometry|the retry delay has not passed yet)$"
)]
fn not_calibrated(ward: &mut Ward, n: u32, reason: String) {
    let expected = match reason.as_str() {
        "someone is in the room" => SkipReason::RoomOccupied,
        "faces are in sight" => SkipReason::FacesPresent,
        "there is no bed geometry" => SkipReason::NoBedGeometry,
        _ => SkipReason::RetryWait,
    };
    assert_eq!(skipped(ward, n, LookKind::Calibrate), Some(expected));
}

#[then("the scene publishes the calibrated bed")]
fn publishes_calibrated(ward: &mut Ward) {
    let calibration = ward.last().output.scene.map.calibration;
    assert!(
        matches!(calibration, Calibration::Complete { .. }),
        "{calibration:?}"
    );
}
