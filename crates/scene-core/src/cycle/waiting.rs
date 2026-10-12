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
    stage: Continuation,
}

/// Data needed to interpret the probe of a waiting stage.
#[derive(Debug)]
pub(super) enum Continuation {
    Primary,
    /// The main detector over the focus: it may have to widen.
    Focused,
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
    /// The main detector: over the focus, or over the whole frame (`None`).
    pub(super) fn primary(region: Option<Rect>) -> Self {
        let stage = match region {
            Some(_) => Continuation::Focused,
            None => Continuation::Primary,
        };
        Self::new(Probe::Detect { region }, stage)
    }

    /// The focus did not confirm: the main detector again, over the whole
    /// frame. Its reading replaces the focus's.
    pub(super) fn widen() -> Self {
        Self::primary(None)
    }

    pub(super) fn reinforce(region: Rect, primary: Admission) -> Self {
        Self::new(
            Probe::Reinforce { region },
            Continuation::Reinforce { primary },
        )
    }

    pub(super) fn faces(bodies: Admission, region: FaceRegion, policy: FacePolicy) -> Self {
        Self::new(
            Probe::Faces { region },
            Continuation::Faces {
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
            Continuation::Calibrate { observed },
        )
    }

    pub(super) fn pose(observed: Observed, bbox: Rect) -> Self {
        Self::new(Probe::Pose { bbox }, Continuation::Pose { observed, bbox })
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
            Continuation::Depth {
                observed,
                pose,
                bbox,
                at,
            },
        )
    }

    fn new(probe: Probe, stage: Continuation) -> Self {
        Self { probe, stage }
    }

    /// The probe to look at and what interprets its reading.
    pub(super) fn into_parts(self) -> (Probe, Continuation) {
        (self.probe, self.stage)
    }
}
