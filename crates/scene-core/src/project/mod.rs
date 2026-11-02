//! The projection: how the belief looks from outside. It does not decide and
//! is not read back (rule 3). It is the only place that translates the domain
//! into the contract, one version per module (today `v4`), from the
//! conclusion and the chronicle. What no version changes lives here, as
//! `contract::common` does: the evidence, the subject, the bed and how what
//! is published degrades on republishing (rule 2). It names only
//! `contract::common`, so a next version shares it without depending on v4
//! (C2).

mod parts;
pub(crate) mod v4;

use crate::attention::Attention;
use crate::belief::BedPosture;
use crate::belief::bed_posture::BedPostureSeen;
use crate::belief::bed_posture::Concluded;
use crate::belief::{Believed, Occupancy, Occupants, Subject as BelievedSubject};
use crate::body::{
    BedPoint, BodyPoint, HeadEvidence, HeadPlace, HeadPlaceState, Pose, SubjectDepth,
};
use crate::chronicle::{BedKind, Chronicled};
use crate::conclusion::RepublishReason;
use crate::evidence::{BodyEvidence, FaceEvidence, Perceived};
use crate::look::Detection;
use crate::refine::Refinement;
use crate::refine::{
    BodyGroup, BodyGroups, Disparities, Head, Lying, LyingState, Posture, PostureState, Verdict,
    VerdictState,
};
use crate::units::Seconds;
use scene_contract::{
    Bed, BedState, BodiesEvidence, BodiesStatus, BodySeen, Evidence, FaceSeen, FacesEvidence,
    FacesStatus, Held, LookKind, LookOutcome, Pending, Progress, Provenance, Republished,
    SampleRef, SkipReason, Subject, Zone,
};
use scene_contract::{Cell, Fv, Rbh};

impl From<BedPostureSeen> for scene_contract::BedPostureSeen {
    fn from(seen: BedPostureSeen) -> Self {
        match seen {
            BedPostureSeen::Held => Self::Held,
            BedPostureSeen::HeadAway => Self::HeadAway,
            BedPostureSeen::HeadSide => Self::HeadSide,
            BedPostureSeen::LegsOut => Self::LegsOut,
            BedPostureSeen::HeadboardLying => Self::HeadboardLying,
            BedPostureSeen::HeadboardUpright => Self::HeadboardUpright,
            BedPostureSeen::Unknown => Self::Unknown,
        }
    }
}

impl From<BedPosture> for scene_contract::BedPosture {
    fn from(b: BedPosture) -> Self {
        Self {
            state: b.state,
            seen: b.seen.into(),
            since: b.since,
        }
    }
}

impl From<LyingState> for scene_contract::LyingState {
    fn from(state: LyingState) -> Self {
        match state {
            LyingState::Confirmed => Self::Confirmed,
            LyingState::Assumed => Self::Assumed,
            LyingState::No => Self::No,
            LyingState::Unknown => Self::Unknown,
        }
    }
}

impl From<Lying> for scene_contract::Lying {
    fn from(l: Lying) -> Self {
        Self {
            state: l.state.into(),
            provenance: l.provenance,
        }
    }
}

impl From<PostureState> for scene_contract::PostureState {
    fn from(state: PostureState) -> Self {
        match state {
            PostureState::Lying => Self::Lying,
            PostureState::Upright => Self::Upright,
            PostureState::Unknown => Self::Unknown,
        }
    }
}

impl From<BodyGroup> for scene_contract::BodyGroup {
    fn from(g: BodyGroup) -> Self {
        Self {
            depth: g.depth,
            seen: g.seen,
            of: g.of,
        }
    }
}

impl From<BodyGroups> for scene_contract::BodyGroups {
    fn from(g: BodyGroups) -> Self {
        Self {
            head: g.head.into(),
            shoulders: g.shoulders.into(),
            pelvis: g.pelvis.into(),
        }
    }
}

impl From<Disparities> for scene_contract::Disparities {
    fn from(d: Disparities) -> Self {
        Self {
            upper_pelvis: d.upper_pelvis,
            shoulders_pelvis: d.shoulders_pelvis,
            head_pelvis: d.head_pelvis,
            upper_bed_centre: d.upper_bed_centre,
        }
    }
}

impl From<HeadEvidence> for scene_contract::HeadEvidence {
    fn from(e: HeadEvidence) -> Self {
        match e {
            HeadEvidence::Face => Self::Face,
            HeadEvidence::PoseHead => Self::PoseHead,
        }
    }
}

impl From<HeadPlaceState> for scene_contract::HeadPlaceState {
    fn from(s: HeadPlaceState) -> Self {
        match s {
            HeadPlaceState::AtHeadboard => Self::AtHeadboard,
            HeadPlaceState::BelowHeadboard => Self::BelowHeadboard,
            HeadPlaceState::AtSide => Self::AtSide,
            HeadPlaceState::Elsewhere => Self::Elsewhere,
            HeadPlaceState::Unknown => Self::Unknown,
        }
    }
}

impl From<HeadPlace> for scene_contract::HeadPlace {
    fn from(h: HeadPlace) -> Self {
        Self {
            state: h.state.into(),
            by: h.by.map(Into::into),
        }
    }
}

impl From<BodyPoint> for scene_contract::BodyPoint {
    fn from(p: BodyPoint) -> Self {
        Self {
            name: p.name,
            x: p.x,
            y: p.y,
            score: p.score,
            depth: p.depth,
        }
    }
}

impl From<Pose> for scene_contract::Pose {
    fn from(p: Pose) -> Self {
        Self {
            keypoints: p.keypoints.into_iter().map(Into::into).collect(),
            provenance: p.provenance,
        }
    }
}

impl From<SubjectDepth> for scene_contract::SubjectDepth {
    fn from(d: SubjectDepth) -> Self {
        Self {
            meters: d.meters,
            provenance: d.provenance,
        }
    }
}

impl From<BedPoint> for scene_contract::BedPoint {
    fn from(b: BedPoint) -> Self {
        Self {
            name: b.name,
            x: b.x,
            y: b.y,
            depth: b.depth,
        }
    }
}

impl From<Posture> for scene_contract::Posture {
    fn from(p: Posture) -> Self {
        Self {
            state: p.state.into(),
            upper: p.upper,
            lower: p.lower,
            disparity: p.disparity,
            provenance: p.provenance,
            groups: p.groups.map(Into::into),
            disparities: p.disparities.map(Into::into),
            tolerance: p.tolerance,
            bed_points: p.bed_points.into_iter().map(Into::into).collect(),
            head_place: p.head_place.map(Into::into),
        }
    }
}

impl From<Head> for scene_contract::Head {
    fn from(h: Head) -> Self {
        match h {
            Head::Disparity => Self::Disparity,
        }
    }
}

impl From<VerdictState> for scene_contract::VerdictState {
    fn from(state: VerdictState) -> Self {
        match state {
            VerdictState::LyingInBed => Self::LyingInBed,
            VerdictState::SittingUpInBed => Self::SittingUpInBed,
            VerdictState::InBed => Self::InBed,
        }
    }
}

impl From<Verdict> for scene_contract::Verdict {
    fn from(v: Verdict) -> Self {
        Self {
            state: v.state.into(),
            by: v.by.into_iter().map(Into::into).collect(),
            since: v.since,
        }
    }
}

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
fn pending(occupancy: &Occupancy, dwell_secs: Seconds) -> Option<Pending> {
    occupancy.pending.map(|pending| Pending {
        counts: pending.counts,
        duration: Progress {
            have: pending.source_secs,
            need: dwell_secs.get(),
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
    body_parts: Option<crate::belief::parts::Concluded>,
    chronicled: &Chronicled,
    at: SampleRef,
    dwell_secs: Seconds,
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
        pose: pose.map(Into::into),
        depth: depth.map(Into::into),
        lying: refined.lying.map(Into::into),
        posture: refined.posture.map(Into::into),
        verdict: refined.verdict.map(|r| {
            Verdict {
                state: r.state,
                by: r.by,
                since: chronicled.verdict.since,
            }
            .into()
        }),
        bed_posture: bed_posture.map(|b| {
            BedPosture {
                state: b.state,
                seen: b.seen,
                since: chronicled.bed_posture.since,
            }
            .into()
        }),
        parts: body_parts.map(|p| parts::parts(&p, chronicled, at, dwell_secs)),
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
    let seen = |d: &Detection, source, zone, near_bed, shadow, knee_deg| BodySeen {
        zone,
        near_bed,
        bbox: d.bbox,
        confidence: d.confidence,
        source,
        shadow,
        knee_deg,
    };
    match evidence {
        BodyEvidence::Unavailable(_) => BodiesEvidence::default(),
        BodyEvidence::Unzoned(unzoned) => BodiesEvidence {
            status: BodiesStatus::Unzoned,
            items: unzoned
                .iter()
                .map(|b| seen(&b.detection, b.source, None, None, b.shadow, b.knee_deg))
                .collect(),
        },
        BodyEvidence::Seen(bodies) => BodiesEvidence {
            status: BodiesStatus::Seen,
            items: bodies
                .bed
                .iter()
                .map(|b| {
                    seen(
                        &b.detection,
                        b.source,
                        Some(Zone::Bed),
                        None,
                        b.shadow,
                        b.knee_deg,
                    )
                })
                .chain(bodies.floor.iter().map(|b| {
                    seen(
                        &b.detection,
                        b.source,
                        Some(Zone::Floor),
                        Some(b.near_bed),
                        b.shadow,
                        b.knee_deg,
                    )
                }))
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

/// The cell of a change from `before`, seeing `seen`, to `after`: the natural
/// key of the row, `before:seen:after`. The same in every version; the
/// integration tests use it as their oracle.
pub fn cell_of(before: Rbh, seen: Fv, after: Rbh) -> Cell {
    let Rbh(r0, b0, h0) = before;
    let Fv(f, v) = seen;
    let Rbh(r1, b1, h1) = after;
    Cell(format!(
        "{}:{}:{}",
        part(&[r0, b0, h0]),
        part(&[f, v]),
        part(&[r1, b1, h1])
    ))
}

fn part(counts: &[u32]) -> String {
    let digits: Vec<String> = counts.iter().map(u32::to_string).collect();
    if counts.iter().all(|&count| count < 10) {
        digits.concat()
    } else {
        format!("({})", digits.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::cell_of;
    use scene_contract::{Fv, Rbh};

    #[test]
    fn a_cell_is_written_in_digits_if_all_counts_are_below_ten() {
        let cell = cell_of(Rbh(2, 1, 0), Fv(0, 0), Rbh(1, 1, 1));
        assert_eq!(cell.as_str(), "210:00:111");
    }

    #[test]
    fn a_part_with_two_digit_counts_is_written_as_a_tuple() {
        let cell = cell_of(Rbh(12, 0, 0), Fv(13, 0), Rbh(13, 0, 0));
        assert_eq!(cell.as_str(), "(12,0,0):(13,0):(13,0,0)");
        let mixed = cell_of(Rbh(9, 1, 1), Fv(10, 0), Rbh(10, 1, 1));
        assert_eq!(mixed.as_str(), "911:(10,0):(10,1,1)");
    }
}
