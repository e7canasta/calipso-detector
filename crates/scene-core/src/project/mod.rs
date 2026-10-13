//! The projection: how the belief looks from outside. It does not decide and
//! is not read back (rule 3). It is the only place that translates the domain
//! into the contract, one version per module (today `v4`), from the
//! conclusion and the chronicle. What no version changes lives here, as
//! `contract::common` does: the evidence, the subject, the bed and how what
//! is published degrades on republishing (rule 2). It names only
//! `contract::common`, so a next version shares it without depending on v4
//! (C2).

pub(crate) mod v4;

use crate::attention::Attention;
use crate::belief::BedPosture;
use crate::belief::bed_posture::Concluded;
use crate::belief::{Believed, Occupancy, Occupants, Subject as BelievedSubject};
use crate::chronicle::{BedKind, Chronicled};
use crate::conclusion::RepublishReason;
use crate::contract::common::{
    Bed, BedState, BodiesEvidence, BodiesStatus, BodySeen, Evidence, FaceSeen, FacesEvidence,
    FacesStatus, Held, LookKind, LookOutcome, Pending, Progress, Provenance, Republished,
    SampleRef, SkipReason, Subject, Zone,
};
use crate::evidence::{BodyEvidence, FaceEvidence, Perceived};
use crate::look::Detection;
use crate::refine::{Refinement, Verdict};

/// The bed as layer 0 believes it, dated by the chronicle.
fn bed(
    occupancy: &Occupancy,
    chronicled: &Chronicled,
    held_check: Option<crate::verification::HeldEntry>,
) -> Bed {
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
                check: held_check
                    .filter(|entry| entry.since == held.since)
                    .map(|entry| entry.check),
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
    publish_bed(BedKind::of(occupants, bed_known))
}

/// The bed's kind, as published.
fn publish_bed(kind: BedKind) -> BedState {
    match kind {
        BedKind::Unknown => BedState::Unknown,
        BedKind::Empty => BedState::Empty,
        BedKind::Held => BedState::Held,
        BedKind::Seen => BedState::Seen,
    }
}

/// A turn in which the main detector failed observed nothing: the belief is
/// that of the last observation, and the scene declares it like a
/// republication (rule 2).
fn unobserved(bodies: &BodyEvidence, observed: Option<SampleRef>) -> Option<Republished> {
    matches!(bodies, BodyEvidence::Unavailable(_)).then_some(Republished {
        reason: RepublishReason::DetectFailed,
        observed_at: observed,
    })
}

/// The published subject: what layer 0 believes, with its pose and depth and
/// what the second level derived.
fn publish_subject(
    believed: &Believed,
    refined: Refinement,
    bed_posture: Option<Concluded>,
    chronicled: &Chronicled,
) -> Option<Subject> {
    let s = believed.subject?;
    let (pose, depth) = believed
        .body
        .clone()
        .map_or((None, None), |b| (Some(b.pose), b.depth));
    Some(Subject {
        zone: s.zone,
        bbox: s.bbox,
        confidence: s.confidence,
        provenance: s.provenance,
        pose,
        depth,
        lying: refined.lying,
        posture: refined.posture,
        verdict: refined.verdict.map(|r| Verdict {
            state: r.state,
            by: r.by,
            since: chronicled.verdict.since,
        }),
        bed_posture: bed_posture.map(|b| BedPosture {
            state: b.state,
            seen: b.seen,
            since: chronicled.bed_posture.since,
        }),
    })
}

/// The evidence of the turn, as published: the bodies and the faces.
fn project_evidence(
    evidence: &Perceived,
    subject: Option<&BelievedSubject>,
    attention: &Attention,
) -> Evidence {
    Evidence {
        bodies: bodies(&evidence.bodies.bodies),
        faces: faces(&evidence.faces, subject, attention),
    }
}

/// The admitted bodies, with their zone when the bed geometry applied.
fn bodies(evidence: &BodyEvidence) -> BodiesEvidence {
    let seen = |d: &Detection, source, zone, near_bed| BodySeen {
        zone,
        near_bed,
        bbox: d.bbox,
        confidence: d.confidence,
        source,
    };
    match evidence {
        BodyEvidence::Unavailable(_) => BodiesEvidence::default(),
        BodyEvidence::Unzoned(unzoned) => BodiesEvidence {
            status: BodiesStatus::Unzoned,
            items: unzoned
                .iter()
                .map(|b| seen(&b.detection, b.source, None, None))
                .collect(),
        },
        BodyEvidence::Seen(bodies) => BodiesEvidence {
            status: BodiesStatus::Seen,
            items: bodies
                .bed
                .iter()
                .map(|b| seen(&b.detection, b.source, Some(Zone::Bed), None))
                .chain(
                    bodies
                        .floor
                        .iter()
                        .map(|b| seen(&b.detection, b.source, Some(Zone::Floor), Some(b.near_bed))),
                )
                .collect(),
        },
    }
}

/// The faces, and which fall on the subject seen in this turn. Not looked
/// for: the ledger says why.
fn faces(
    evidence: &FaceEvidence,
    subject: Option<&BelievedSubject>,
    attention: &Attention,
) -> FacesEvidence {
    match evidence {
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
                .map(|face| FaceSeen {
                    bbox: face.bbox,
                    confidence: face.confidence,
                    on_subject: subject.is_some_and(|s| {
                        s.provenance == Provenance::Current && s.bbox.contains(face.bbox.centroid())
                    }),
                })
                .collect(),
        },
    }
}
