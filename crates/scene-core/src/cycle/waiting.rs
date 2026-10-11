//! Builds each probe with its continuation.

use super::Observed;
use crate::body::Keypoint;
use crate::evidence::Admission;
use crate::geom::{Point, Rect};
use crate::look::{DepthPurpose, FaceRegion, Probe};
use crate::policy::FacePolicy;

/// Probe and continuation are created together; their fields cannot be
/// replaced from the sequencer.
#[derive(Debug)]
pub(super) struct Waiting {
    probe: Probe,
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
    pub(super) fn primary(region: Option<Rect>) -> Self {
        Self::new(Probe::Detect { region }, Awaiting::Primary)
    }

    pub(super) fn reinforce(region: Rect, primary: Admission) -> Self {
        Self::new(Probe::Reinforce { region }, Awaiting::Reinforce { primary })
    }

    pub(super) fn faces(bodies: Admission, region: FaceRegion, policy: FacePolicy) -> Self {
        Self::new(
            Probe::Faces { region },
            Awaiting::Faces {
                bodies,
                region,
                policy,
            },
        )
    }

    pub(super) fn calibrate(observed: Observed, points: Vec<Point>) -> Self {
        Self::new(
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                points,
            },
            Awaiting::Calibrate { observed },
        )
    }

    pub(super) fn pose(observed: Observed, bbox: Rect) -> Self {
        Self::new(Probe::Pose { bbox }, Awaiting::Pose { observed, bbox })
    }

    pub(super) fn depth(
        observed: Observed,
        pose: Vec<Keypoint>,
        bbox: Rect,
        asked: Vec<(usize, Point)>,
    ) -> Self {
        let (at, points) = asked.into_iter().unzip();
        Self::new(
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

    fn new(probe: Probe, stage: Awaiting) -> Self {
        Self { probe, stage }
    }

    /// The probe to look at and what interprets its reading.
    pub(super) fn into_parts(self) -> (Probe, Awaiting) {
        (self.probe, self.stage)
    }
}
