//! The projection: how the belief looks from outside. It does not decide and
//! is not read back (rule 3). It is the only place that translates the domain
//! into the contract, one version per module (`v3`, `v4`), from the same
//! conclusion and the same chronicle: the versions cannot tell different
//! things (project 24, V4). What both translate the same lives here: the
//! evidence, the subject, the map, the bed and how what is published degrades
//! on republishing (rule 2).

pub(crate) mod v3;
pub(crate) mod v4;

use crate::attention::Attention;
use crate::belief::{Believed, Occupancy, Subject as BelievedSubject, Transition, ZoneCounts};
use crate::chronicle::{Chronicled, Entry};
use crate::conclusion::{Observation, RepublishReason};
use crate::contract::v3::{
    Bed, BedState, Evidence, Floor, Held, Pending, Progress, Provenance, Republished, SampleRef,
    Subject,
};
use crate::map::{FrameInfo, Geometry, MapState};
use crate::sensor::SensorState;

/// What every version publishes the same of an observed turn: computed once
/// and used as is by each version (V4).
#[derive(Debug, Clone)]
pub(super) struct Shared {
    pub frame: Option<FrameInfo>,
    pub evidence: Evidence,
    pub bed: Bed,
    pub floor: Floor,
    pub subject: Option<Subject>,
    pub attention: Attention,
    pub map: MapState,
    pub sensor: SensorState,
    pub republished: Option<Republished>,
    /// The count being stabilized: the room's, the same in every version.
    pub pending: Option<Pending>,
}

/// What of layer 0 each version translates its own way: the room, the
/// transition and the calibration.
#[derive(Debug)]
pub(super) struct Layer0 {
    pub at: SampleRef,
    pub occupancy: Occupancy,
    pub transition: Option<Transition>,
    pub calibrated: Option<crate::map::BedCalibrated>,
}

/// The scene and events of an observed turn, in each version.
#[derive(Debug)]
pub(crate) struct Projection {
    pub v3: (v3::Scene, Vec<v3::Event>),
    pub v4: (v4::Scene, Vec<v4::Event>),
}

/// Projects an observed turn: the shared part once, and each version builds
/// its room and events with the same chronicle.
pub(crate) fn observed(observation: Observation, chronicled: &Chronicled) -> Projection {
    let (shared, layer0) = split(observation, chronicled);
    Projection {
        v4: v4::scene(shared.clone(), &layer0, chronicled),
        v3: v3::scene(shared, &layer0, chronicled),
    }
}

fn split(observation: Observation, chronicled: &Chronicled) -> (Shared, Layer0) {
    let Observation {
        at,
        believed,
        refined,
        evidence,
        frame,
        calibrated,
        attention,
        map,
        sensor,
        dwell_secs,
    } = observation;
    let evidence = project_evidence(&evidence, believed.subject.as_ref(), &attention);
    let republished = unobserved(&evidence, chronicled.observed);
    let subject = publish_subject(&believed, refined);
    let Believed {
        occupancy,
        transition,
        ..
    } = believed;
    let shared = Shared {
        frame,
        evidence,
        bed: Bed {
            state: bed_state(occupancy.totals(), occupancy.bed_known),
            seen: occupancy.counts.bed,
            since: Some(chronicled.bed.since),
            held: occupancy
                .held
                .iter()
                .map(|held| Held {
                    bbox: held.detection.bbox,
                    last_seen: held.since,
                })
                .collect(),
            unexplained: occupancy.unexplained,
        },
        floor: Floor {
            seen: occupancy.counts.floor,
        },
        subject,
        attention,
        map,
        sensor,
        republished,
        pending: occupancy.pending.map(|pending| Pending {
            counts: pending.counts,
            duration: Progress {
                have: pending.source_secs,
                need: dwell_secs,
            },
        }),
    };
    let layer0 = Layer0 {
        at,
        occupancy,
        transition,
        calibrated,
    };
    (shared, layer0)
}

/// What was current becomes retained on republishing: nothing is `Current`
/// without an observation (rule 2).
fn retained_subject(subject: Option<&Subject>, projected_at: SampleRef) -> Option<Subject> {
    let mut subject = subject.cloned();
    if let Some(s) = subject.as_mut() {
        let retained = |p: Provenance| match p {
            Provenance::Current => Provenance::Retained {
                since: projected_at,
            },
            other => other,
        };
        s.provenance = retained(s.provenance);
        if let Some(pose) = s.pose.as_mut() {
            pose.provenance = retained(pose.provenance);
        }
        if let Some(depth) = s.depth.as_mut() {
            depth.provenance = retained(depth.provenance);
        }
        if let Some(lying) = s.lying.as_mut() {
            lying.provenance = retained(lying.provenance);
        }
        if let Some(posture) = s.posture.as_mut() {
            posture.provenance = retained(posture.provenance);
        }
    }
    subject
}

/// When what a republished scene claims is from. The last projection may have
/// been a detector failure, with evidence from an earlier sample (or none):
/// republishing it does not renew that origin.
fn republished_at(republished: Option<Republished>, projected_at: SampleRef) -> Option<SampleRef> {
    republished.map_or(Some(projected_at), |r| r.observed_at)
}

/// The bed before the first scene.
fn empty_bed() -> Bed {
    Bed {
        state: BedState::Empty,
        seen: 0,
        since: None,
        held: Vec::new(),
        unexplained: 0,
    }
}

/// The bed's published state: `unknown` if this turn does not know the zones;
/// otherwise empty, held or seen.
fn bed_state(counts: ZoneCounts, bed_known: bool) -> BedState {
    match (bed_known, counts.bed, counts.held) {
        (false, _, _) => BedState::Unknown,
        (true, 0, 0) => BedState::Empty,
        (true, 0, _) => BedState::Held,
        (true, _, _) => BedState::Seen,
    }
}

/// What the chronicle notes of a concluded turn: the published states and
/// whether it saw bodies per zone.
pub(crate) fn entry(observation: &Observation) -> Entry {
    let occupancy = &observation.believed.occupancy;
    Entry {
        at: observation.at,
        room: v3::room_state(occupancy.totals()),
        mode: v4::mode(occupancy.totals()),
        bed: bed_state(occupancy.totals(), occupancy.bed_known),
        seen: matches!(
            observation.evidence.bodies.bodies,
            crate::evidence::BodyEvidence::Seen(_)
        ),
        transition: observation.believed.transition.as_ref().map(v4::cell),
    }
}

/// A turn in which the main detector failed observed nothing: the belief is
/// that of the last observation, and the scene declares it like a
/// republication (rule 2).
fn unobserved(
    evidence: &crate::contract::v3::Evidence,
    observed: Option<SampleRef>,
) -> Option<Republished> {
    (evidence.bodies.status == crate::contract::v3::BodiesStatus::Unavailable).then_some(
        Republished {
            reason: RepublishReason::DetectFailed,
            observed_at: observed,
        },
    )
}

/// The published subject: what layer 0 believes, with its pose and depth and
/// what the second level derived.
fn publish_subject(
    believed: &Believed,
    refined: crate::refine::Refinement,
) -> Option<crate::contract::v3::Subject> {
    let s = believed.subject?;
    let (pose, depth) = believed
        .body
        .clone()
        .map_or((None, None), |b| (Some(b.pose), b.depth));
    Some(crate::contract::v3::Subject {
        zone: s.zone,
        bbox: s.bbox,
        confidence: s.confidence,
        provenance: s.provenance,
        pose,
        depth,
        lying: refined.lying,
        posture: refined.posture,
    })
}

fn project_evidence(
    evidence: &crate::evidence::Perceived,
    subject: Option<&BelievedSubject>,
    attention: &Attention,
) -> crate::contract::v3::Evidence {
    use crate::contract::v3::*;
    use crate::evidence::{BodyEvidence, FaceEvidence};
    let crate::evidence::Perceived {
        bodies: admission,
        faces,
    } = evidence;
    let body = |d: &crate::look::Detection, source, zone, near_bed| BodySeen {
        zone,
        near_bed,
        bbox: d.bbox,
        confidence: d.confidence,
        source,
    };
    let bodies = match &admission.bodies {
        BodyEvidence::Unavailable(_) => BodiesEvidence::default(),
        BodyEvidence::Unzoned(b) => BodiesEvidence {
            status: BodiesStatus::Unzoned,
            items: b
                .iter()
                .map(|b| body(&b.detection, b.source, None, None))
                .collect(),
        },
        BodyEvidence::Seen(b) => BodiesEvidence {
            status: BodiesStatus::Seen,
            items: b
                .bed
                .iter()
                .map(|b| body(&b.detection, b.source, Some(Zone::Bed), None))
                .chain(
                    b.floor
                        .iter()
                        .map(|b| body(&b.detection, b.source, Some(Zone::Floor), Some(b.near_bed))),
                )
                .collect(),
        },
    };
    let faces = match faces {
        FaceEvidence::NotLooked => FacesEvidence {
            reason: Some(
                attention
                    .looks
                    .iter()
                    .find_map(|l| match (&l.kind, &l.outcome) {
                        (LookKind::Faces, LookOutcome::Skipped { reason }) => Some(*reason),
                        _ => None,
                    })
                    .unwrap_or(SkipReason::NotRequested),
            ),
            ..FacesEvidence::default()
        },
        FaceEvidence::Unavailable(_) => FacesEvidence {
            status: FacesStatus::Failed,
            reason: None,
            items: Vec::new(),
        },
        FaceEvidence::None => FacesEvidence {
            status: FacesStatus::Empty,
            reason: None,
            items: Vec::new(),
        },
        FaceEvidence::Seen(faces) => FacesEvidence {
            status: FacesStatus::Ok,
            reason: None,
            items: faces
                .iter()
                .map(|d| {
                    let c = d.bbox.centroid();
                    let on_subject = subject.is_some_and(|s| {
                        s.provenance == Provenance::Current
                            && c.x >= s.bbox.x0
                            && c.x <= s.bbox.x1
                            && c.y >= s.bbox.y0
                            && c.y <= s.bbox.y1
                    });
                    FaceSeen {
                        bbox: d.bbox,
                        confidence: d.confidence,
                        on_subject,
                    }
                })
                .collect(),
        },
    };
    Evidence { bodies, faces }
}

pub(crate) fn map_state(map: &crate::map::Map, geometry: Geometry) -> MapState {
    MapState {
        geometry,
        bed: map.site.bed,
        headboard: map.site.headboard,
        calibration: map.reference().map_or(
            crate::contract::v3::Calibration::Unavailable,
            |reference| crate::contract::v3::Calibration::Complete {
                reference_m: reference.meters,
                since: reference.since,
            },
        ),
    }
}
