//! Builds each probe with its continuation.

use super::Observed;
use crate::body::Keypoint;
use crate::evidence::Admission;
use crate::geom::{Point, Rect};
use crate::look::{DepthPurpose, FaceRegion, LookId, Probe, Request};
use crate::policy::FacePolicy;

/// Probe and continuation are created together; their fields cannot be
/// replaced from the sequencer.
#[derive(Debug)]
pub(super) struct Waiting {
    request: Request,
    stage: Awaiting,
}

/// Data needed to interpret the probe of a waiting stage.
#[derive(Debug)]
pub(super) enum Awaiting {
    Primary,
    Reinforce {
        primary: Admission,
    },
    Faces {
        bodies: Admission,
        region: FaceRegion,
        policy: FacePolicy,
    },
    Calibrate {
        observed: Observed,
    },
    Pose {
        observed: Observed,
        bbox: Rect,
    },
    Depth {
        observed: Observed,
        pose: Vec<Keypoint>,
        bbox: Rect,
        /// The pose index of each requested point.
        at: Vec<usize>,
    },
}

impl Waiting {
    pub(super) fn primary(id: LookId, region: Option<Rect>) -> Self {
        Self::new(id, Probe::Detect { region }, Awaiting::Primary)
    }

    pub(super) fn reinforce(id: LookId, region: Rect, primary: Admission) -> Self {
        Self::new(
            id,
            Probe::Reinforce { region },
            Awaiting::Reinforce { primary },
        )
    }

    pub(super) fn faces(
        id: LookId,
        bodies: Admission,
        region: FaceRegion,
        policy: FacePolicy,
    ) -> Self {
        Self::new(
            id,
            Probe::Faces { region },
            Awaiting::Faces {
                bodies,
                region,
                policy,
            },
        )
    }

    pub(super) fn calibrate(id: LookId, observed: Observed, points: Vec<Point>) -> Self {
        Self::new(
            id,
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                points,
            },
            Awaiting::Calibrate { observed },
        )
    }

    pub(super) fn pose(id: LookId, observed: Observed, bbox: Rect) -> Self {
        Self::new(id, Probe::Pose { bbox }, Awaiting::Pose { observed, bbox })
    }

    pub(super) fn depth(
        id: LookId,
        observed: Observed,
        pose: Vec<Keypoint>,
        bbox: Rect,
        asked: Vec<(usize, Point)>,
    ) -> Self {
        let (at, points) = asked.into_iter().unzip();
        Self::new(
            id,
            Probe::Depth {
                purpose: DepthPurpose::Subject,
                points,
            },
            Awaiting::Depth {
                observed,
                pose,
                bbox,
                at,
            },
        )
    }

    fn new(id: LookId, probe: Probe, stage: Awaiting) -> Self {
        Self {
            request: Request { id, probe },
            stage,
        }
    }

    /// The probe to look at and what interprets its reading.
    pub(super) fn into_parts(self) -> (Request, Awaiting) {
        (self.request, self.stage)
    }
}
