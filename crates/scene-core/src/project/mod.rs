//! The projection: how the belief looks from outside. It does not decide and
//! is not read back (rule 3). It is the only place that translates the domain
//! into the contract, one version per module (today `v4`), from the
//! conclusion and the chronicle. What no version changes lives here, as
//! `contract::common` does: the evidence, the subject, the bed and how what
//! is published degrades on republishing (rule 2).

pub(crate) mod v4;

use crate::attention::Attention;
use crate::belief::{Believed, Occupancy, Occupants, Subject as BelievedSubject};
use crate::chronicle::{BedKind, Chronicled};
use crate::conclusion::RepublishReason;
use crate::contract::v4::{
    Bed, BedState, Held, Pending, Progress, Provenance, Republished, SampleRef, Subject,
};

/// The bed as layer 0 believes it, dated by the chronicle.
fn bed(occupancy: &Occupancy, chronicled: &Chronicled) -> Bed {
    Bed {
        state: bed_state(occupancy.occupants(), occupancy.bed_known),
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
    }
}

/// The count being stabilized, against the dwell it needs.
fn pending(occupancy: &Occupancy, dwell_secs: f64) -> Option<Pending> {
    occupancy.pending.map(|pending| Pending {
        counts: pending.counts,
        duration: Progress {
            have: pending.source_secs,
            need: dwell_secs,
        },
    })
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
fn bed_state(occupants: Occupants, bed_known: bool) -> BedState {
    v4::publish_bed(BedKind::of(occupants, bed_known))
}

/// A turn in which the main detector failed observed nothing: the belief is
/// that of the last observation, and the scene declares it like a
/// republication (rule 2).
fn unobserved(
    evidence: &crate::contract::v4::Evidence,
    observed: Option<SampleRef>,
) -> Option<Republished> {
    (evidence.bodies.status == crate::contract::v4::BodiesStatus::Unavailable).then_some(
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
) -> Option<crate::contract::v4::Subject> {
    let s = believed.subject?;
    let (pose, depth) = believed
        .body
        .clone()
        .map_or((None, None), |b| (Some(b.pose), b.depth));
    Some(crate::contract::v4::Subject {
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
) -> crate::contract::v4::Evidence {
    use crate::contract::v4::*;
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
