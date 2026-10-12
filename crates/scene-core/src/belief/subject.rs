//! The subject's enrichment: its last pose and depth, and the box they were
//! taken with (the motion reference). The subject itself derives from the
//! occupancy: it is the only occupant, if there is exactly one.

use super::Subject;
use crate::body::{BedPoint, BodyPoint, Joint, Keypoint, Pose, SubjectDepth};
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
    /// The depth of each pose keypoint, aligned.
    depths: Vec<Option<f32>>,
    /// The median of the measured keypoints.
    depth: Option<f32>,
    /// The bed's points, read in the same look as the keypoints.
    bed_points: Vec<BedPoint>,
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
        /// The depth of each keypoint, aligned with the pose; empty if not
        /// measured.
        depths: Vec<Option<f32>>,
        /// The bed's points, read in the same look; empty if not measured.
        bed_points: Vec<BedPoint>,
        bbox: Rect,
        reference: ReferenceUpdate,
    },
    /// The pose failed or brought no keypoints: the previous one no longer holds.
    Failed,
}

impl Enrichment {
    /// Only the main detector renews an existing reference; reinforcement keeps it.
    pub(crate) fn fresh(
        pose: Option<Vec<Keypoint>>,
        depths: Option<SubjectDepths>,
        bbox: Rect,
        primary_seen: bool,
    ) -> Self {
        let SubjectDepths { keypoints, bed } = depths.unwrap_or(SubjectDepths {
            keypoints: Vec::new(),
            bed: Vec::new(),
        });
        match pose {
            Some(pose) => Self::Fresh {
                pose,
                depths: keypoints,
                bed_points: bed,
                bbox,
                reference: if primary_seen {
                    ReferenceUpdate::Refresh
                } else {
                    ReferenceUpdate::Preserve
                },
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
                bed_points,
                bbox,
                reference,
            } => {
                let reference = match (&self.enriched, reference) {
                    (Some(old), ReferenceUpdate::Preserve) => old.reference,
                    _ => bbox,
                };
                let enriched = self.enriched.insert(Enriched {
                    pose,
                    depth: estimation::median(&depths),
                    depths,
                    bed_points,
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
                    name: Joint::coco(i),
                    x: k.x,
                    y: k.y,
                    score: k.score,
                    depth: enriched.depths.get(i).copied().flatten(),
                })
                .collect(),
            provenance,
        },
        depth: enriched.depth.map(|depth| SubjectDepth {
            meters: depth,
            provenance,
        }),
        bed_points: enriched.bed_points.clone(),
    }
}
