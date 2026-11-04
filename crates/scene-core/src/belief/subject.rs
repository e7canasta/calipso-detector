//! The subject's enrichment: its last pose and depth, and the box they were
//! taken with (the motion reference). The subject itself derives from the
//! occupancy: it is the only occupant, if there is exactly one.

use super::Subject;
use crate::body::HeadPlace;
use crate::body::coco_joint;
use crate::body::{BedPoint, BodyPoint, Keypoint, Pose, SubjectDepth};
use crate::estimation;
use crate::evidence::SubjectDepths;
use crate::geom::Rect;
use crate::provenance::Provenance;
use crate::time::SampleRef;

/// What to do with the motion reference on receiving a new pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReferenceUpdate {
    Refresh,
    Preserve,
}

/// What was computed of the subject the last time it was enriched.
#[derive(Debug, Clone)]
struct Enriched {
    pose: Vec<Keypoint>,
    /// What the depth look read: the keypoints, aligned, and the bed's points.
    depths: SubjectDepths,
    /// The median of the measured keypoints.
    depth: Option<f32>,
    /// Where the head was when the look was decided.
    head: HeadPlace,
    since: SampleRef,
    /// The box it was taken with: the motion reference.
    reference: Rect,
}

/// What happened to the enrichment in this turn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Enrichment {
    /// There was nothing to enrich.
    None,
    /// The subject did not move: the previous one holds.
    Kept,
    /// A new pose (and depth, if measured), with the decision about the reference.
    Fresh {
        pose: Vec<Keypoint>,
        /// What the depth look read, whole; empty if not measured.
        depths: SubjectDepths,
        /// Where the head was when the look was decided.
        head: HeadPlace,
        bbox: Rect,
        reference: ReferenceUpdate,
    },
    /// The pose failed or brought no keypoints: the previous one no longer holds.
    Failed,
}

impl ReferenceUpdate {
    /// Only the main detector renews an existing reference; reinforcement
    /// keeps it.
    pub(crate) fn of(primary_seen: bool) -> Self {
        if primary_seen {
            Self::Refresh
        } else {
            Self::Preserve
        }
    }
}

impl Enrichment {
    /// A new pose (and depth, if measured): what to do with the motion
    /// reference arrives as the rule it is, not as a flag.
    pub(crate) fn fresh(
        pose: Option<Vec<Keypoint>>,
        depths: Option<SubjectDepths>,
        head: HeadPlace,
        bbox: Rect,
        reference: ReferenceUpdate,
    ) -> Self {
        match pose {
            Some(pose) => Self::Fresh {
                pose,
                depths: depths.unwrap_or_default(),
                head,
                bbox,
                reference,
            },
            None => Self::Failed,
        }
    }
}

/// The subject's pose and depth in this turn, with their provenance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SubjectBody {
    pub pose: Pose,
    pub depth: Option<SubjectDepth>,
    /// The bed's points, read with the pose's depth: they hold with it.
    pub bed_points: Vec<BedPoint>,
    /// Where the head was when they were read.
    pub head_place: HeadPlace,
}

/// What is known of the subject beyond its box: its last pose and depth.
#[derive(Debug, Clone, Default)]
pub(super) struct SubjectMemory {
    enriched: Option<Enriched>,
}

impl SubjectMemory {
    /// There is no longer a single occupant: the enrichment belongs to nobody.
    pub(super) fn forget(&mut self) {
        self.enriched = None;
    }

    /// The box of the last pose: the reference to tell whether it moved.
    pub(super) fn reference(&self) -> Option<Rect> {
        self.enriched.as_ref().map(|e| e.reference)
    }

    /// The sample of the last pose: since when it holds.
    pub(super) fn since(&self) -> Option<SampleRef> {
        self.enriched.as_ref().map(|e| e.since)
    }

    /// What was computed (or kept) of the subject in this turn. Without a
    /// subject there is no body to publish, though the memory still updates.
    pub(super) fn observe(
        &mut self,
        at: SampleRef,
        subject: Option<&Subject>,
        e: Enrichment,
    ) -> Option<SubjectBody> {
        let body = match e {
            Enrichment::None => None,
            Enrichment::Failed => {
                self.enriched = None;
                None
            }
            Enrichment::Fresh {
                pose,
                depths,
                head,
                bbox,
                reference,
            } => {
                let reference = match (&self.enriched, reference) {
                    (Some(old), ReferenceUpdate::Preserve) => old.reference,
                    (None, ReferenceUpdate::Preserve)
                    | (None, ReferenceUpdate::Refresh)
                    | (Some(_), ReferenceUpdate::Refresh) => bbox,
                };
                let enriched = self.enriched.insert(Enriched {
                    pose,
                    depth: estimation::median(&depths.keypoints),
                    depths,
                    head,
                    since: at,
                    reference,
                });
                Some(body(enriched, Provenance::Current))
            }
            Enrichment::Kept => self
                .enriched
                .as_ref()
                .map(|e| body(e, Provenance::Stable { since: e.since })),
        };
        body.filter(|_| subject.is_some())
    }
}

fn body(enriched: &Enriched, provenance: Provenance) -> SubjectBody {
    SubjectBody {
        pose: Pose {
            keypoints: enriched
                .pose
                .iter()
                .enumerate()
                .map(|(i, k)| BodyPoint {
                    name: coco_joint(i),
                    x: k.x,
                    y: k.y,
                    score: k.score,
                    depth: enriched.depths.keypoints.get(i).copied().flatten(),
                })
                .collect(),
            provenance,
        },
        depth: enriched.depth.map(|depth| SubjectDepth {
            relative: depth,
            provenance,
        }),
        bed_points: enriched.depths.bed.clone(),
        head_place: enriched.head,
    }
}
