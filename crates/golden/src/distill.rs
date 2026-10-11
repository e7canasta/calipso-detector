//! Distill: a published night (`scene.v4` as JSONL, as the detector,
//! the meter or the bridge write it) becomes a scene. It is the
//! anti-corruption layer in reverse: from the core's published language to
//! the golden scenes' language. No pixels (rule 7): only what the scene
//! published as seen.
//!
//! What each sample saw comes from `evidence`: how many in bed and on the floor
//! (and how many near the bed) according to the main detector, how many
//! reinforcement saw in bed when asked, and the accepted faces. Each span's
//! expectation is what the engine believed live at the end of the span: a
//! distilled night is a regression test of that moment's behaviour.
//!
//! What the language cannot represent yet is rejected with the sample and the
//! reason, without inventing: detector or decode failures, unzoned bodies,
//! reconnections, failures of the optional senses, readings lost when boxes
//! were consolidated, reinforcement seeing someone on the floor and an
//! irregular cadence.

use crate::language::{
    Error, Expected, ExpectedBed, FORMAT, Faces, InBed, Location, Samples, Scene, Sense, Span,
};
use scene_core::contract::{
    BedState, BodiesStatus, BodySource, Envelope, LookKind, LookOutcome, LookResult,
    RepublishReason, Scene as PublishedScene, Zone,
};

/// A distilled night.
#[derive(Debug, Clone, PartialEq)]
pub struct Distilled {
    pub scene: Scene,
    /// The run it came from.
    pub run_id: String,
    /// The camera it came from.
    pub camera: String,
}

/// What was seen in a sample, in the scene's language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    floor: u32,
    location: Location,
    bed: u32,
    reinforcement: u32,
    faces: Faces,
}

fn error(line: usize, reason: impl Into<String>) -> Error {
    Error {
        line: Some(line),
        reason: reason.into(),
    }
}

/// Distills a night. `run` picks the run if the file has several; `dwell_s` is
/// that of the profile it ran with (the envelope does not publish it).
pub fn distill(jsonl: &str, run: Option<&str>, dwell_s: Option<f64>) -> Result<Distilled, Error> {
    let mut chosen: Option<(String, String)> = None;
    let mut samples: Vec<(usize, f64, Seen, Expected)> = Vec::new();
    let mut session = None;
    let mut reinforcement = false;
    let mut faces = false;
    for (i, line) in jsonl.lines().enumerate() {
        let n = i + 1;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| error(n, format!("not JSON: {e}")))?;
        if value.get("schema").and_then(|s| s.as_str()) != Some("calipso.scene.v4") {
            continue;
        }
        let envelope: Envelope<PublishedScene> =
            serde_json::from_value(value).map_err(|e| error(n, format!("not scene.v4: {e}")))?;
        let run_id = &envelope.source.run_id;
        match (&chosen, run) {
            (_, Some(r)) if r != run_id => continue,
            (Some((r, _)), None) if r != run_id => {
                return Err(error(
                    n,
                    format!("the file has more than one run ({r} and {run_id}): pick one"),
                ));
            }
            _ => {}
        }
        chosen.get_or_insert_with(|| (run_id.clone(), envelope.source.camera.clone()));
        let scene = &envelope.body;
        let at = envelope.observed_at;
        match scene.republished.map(|r| r.reason) {
            Some(RepublishReason::Sensor) => continue,
            Some(reason) => {
                return Err(error(
                    n,
                    format!(
                        "sample {} was not observed ({reason:?}): the language cannot represent failures yet",
                        at.sequence
                    ),
                ));
            }
            None => {}
        }
        if *session.get_or_insert(at.session) != at.session {
            return Err(error(
                n,
                "the night has a reconnection: the language cannot represent it yet",
            ));
        }
        check_readings(n, at.sequence, scene)?;
        reinforcement |= scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Reinforce);
        faces |= scene
            .attention
            .looks
            .iter()
            .any(|l| l.kind == LookKind::Faces);
        let seen = seen(n, at.sequence, scene)?;
        samples.push((n, at.source_secs.0, seen, expected(scene)));
    }
    let Some((run_id, camera)) = chosen else {
        return Err(Error {
            line: None,
            reason: "there are no observed scenes to distill".into(),
        });
    };
    let cadence_s = cadence(&samples)?;
    let mut spans: Vec<Span> = Vec::new();
    for (k, (_, _, seen, expected)) in samples.iter().enumerate() {
        let m = u32::try_from(k + 1).unwrap_or(u32::MAX);
        match spans.last_mut() {
            Some(s) if same(s, seen) => {
                s.samples.to = m;
                s.expected = *expected;
            }
            _ => spans.push(Span {
                samples: Samples { from: m, to: m },
                floor: seen.floor,
                location: seen.location,
                bed: InBed::Fixed(seen.bed),
                reinforcement: seen.reinforcement,
                faces: seen.faces,
                expected: *expected,
            }),
        }
    }
    let mut senses = vec![Sense::Detect];
    if reinforcement || spans.iter().any(|s| s.reinforcement > 0) {
        senses.push(Sense::Reinforce);
    }
    if faces || spans.iter().any(|s| s.faces != Faces::None) {
        senses.push(Sense::Faces);
    }
    Ok(Distilled {
        scene: Scene {
            format: FORMAT,
            title: format!("Distilled night — {camera}"),
            cadence_s,
            dwell_s,
            senses,
            spans,
        },
        run_id,
        camera,
    })
}

fn same(s: &Span, v: &Seen) -> bool {
    s.floor == v.floor
        && s.location == v.location
        && s.bed == InBed::Fixed(v.bed)
        && s.reinforcement == v.reinforcement
        && s.faces == v.faces
}

/// The consolidated evidence keeps only the winning box per body: it is not a
/// recording of each detector. The ledger shows readings that were lost, but
/// cannot rebuild their zones and overlaps: they are rejected.
fn check_readings(n: usize, sample: u64, scene: &PublishedScene) -> Result<(), Error> {
    for look in &scene.attention.looks {
        let LookOutcome::Asked { result, .. } = &look.outcome else {
            continue;
        };
        let name = match look.kind {
            LookKind::Detect => "detect",
            LookKind::Reinforce => "reinforce",
            LookKind::Faces => "faces",
            LookKind::Pose => "pose",
            LookKind::Depth => "depth",
            LookKind::Calibrate => "calibrate",
        };
        let LookResult::Read {
            candidates,
            rejected,
        } = result
        else {
            return Err(error(
                n,
                format!(
                    "in sample {sample} {name} failed: the language cannot represent sense failures yet"
                ),
            ));
        };
        let source = match look.kind {
            LookKind::Detect => BodySource::Primary,
            LookKind::Reinforce => BodySource::Reinforcement,
            _ => continue,
        };
        let kept = scene
            .evidence
            .bodies
            .items
            .iter()
            .filter(|b| b.source == source)
            .count();
        let admitted = u32::try_from(rejected.len())
            .ok()
            .and_then(|r| candidates.checked_sub(r));
        if admitted != u32::try_from(kept).ok() {
            return Err(error(
                n,
                format!(
                    "in sample {sample} the consolidated evidence does not keep every {name} reading: the bodies per sense cannot be rebuilt"
                ),
            ));
        }
    }
    Ok(())
}

fn seen(n: usize, sample: u64, scene: &PublishedScene) -> Result<Seen, Error> {
    let bodies = &scene.evidence.bodies;
    match bodies.status {
        BodiesStatus::Seen => {}
        BodiesStatus::Unzoned => {
            return Err(error(n, format!("sample {sample} has no bed geometry")));
        }
        BodiesStatus::Unavailable => {
            return Err(error(n, format!("sample {sample} has no observed bodies")));
        }
    }
    let count = |zone: Zone, source: BodySource| {
        u32::try_from(
            bodies
                .items
                .iter()
                .filter(|b| b.zone == Some(zone) && b.source == source)
                .count(),
        )
        .unwrap_or(u32::MAX)
    };
    if count(Zone::Floor, BodySource::Reinforcement) > 0 {
        return Err(error(
            n,
            format!(
                "in sample {sample} reinforcement saw someone on the floor: the language cannot represent it yet"
            ),
        ));
    }
    let floor = count(Zone::Floor, BodySource::Primary);
    let near = u32::try_from(
        bodies
            .items
            .iter()
            .filter(|b| {
                b.zone == Some(Zone::Floor)
                    && b.source == BodySource::Primary
                    && b.near_bed == Some(true)
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    let location = match near {
        0 => Location::Door,
        j if j == floor => Location::NearBed,
        j => Location::Mixed { near: j },
    };
    let faces = &scene.evidence.faces.items;
    Ok(Seen {
        floor,
        location,
        bed: count(Zone::Bed, BodySource::Primary),
        reinforcement: count(Zone::Bed, BodySource::Reinforcement),
        faces: if faces.iter().any(|c| c.on_subject) {
            Faces::OnSubject
        } else if faces.is_empty() {
            Faces::None
        } else {
            Faces::AtHeadboard
        },
    })
}

/// What the engine believed live.
fn expected(scene: &PublishedScene) -> Expected {
    Expected {
        occupants: Some(scene.room.occupants),
        bed: Some(match scene.bed.state {
            BedState::Empty => ExpectedBed::Empty,
            BedState::Seen => ExpectedBed::Seen,
            BedState::Held => ExpectedBed::Held,
            BedState::Unknown => ExpectedBed::Unknown,
        }),
    }
}

/// The cadence: the median between samples, if all are within ±10 % of it.
fn cadence(samples: &[(usize, f64, Seen, Expected)]) -> Result<f64, Error> {
    let steps: Vec<(usize, f64)> = samples
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((b.0, b.1 - a.1)),
            _ => None,
        })
        .collect();
    if steps.is_empty() {
        return Ok(1.0);
    }
    let mut sorted: Vec<f64> = steps.iter().map(|p| p.1).collect();
    sorted.sort_by(f64::total_cmp);
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(1.0);
    if median <= 0.0 {
        return Err(Error {
            line: None,
            reason: "source time does not advance".into(),
        });
    }
    if let Some((n, step)) = steps
        .iter()
        .find(|(_, p)| (p - median).abs() > median * 0.1)
    {
        return Err(error(
            *n,
            format!(
                "the cadence is irregular ({step:.3} s against {median:.3} s): the language assumes a fixed cadence"
            ),
        ));
    }
    Ok((median * 1000.0).round() / 1000.0)
}
