//! Builds each probe with its continuation and guards the delivery protocol.

use super::{FeedError, Observed};
use crate::body::Keypoint;
use crate::evidence::Admission;
use crate::geom::{Point, Rect};
use crate::look::{DepthPurpose, FaceRegion, LookId, Outcome, Probe, ProbeKindMismatch, Request};
use crate::policy::FacePolicy;

/// Probe and continuation are created together; their fields cannot be
/// replaced from the sequencer.
#[derive(Debug)]
pub(super) struct Waiting {
    request: Request,
    outcome: Option<Outcome>,
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

pub(super) enum Resolution {
    Pending(Waiting),
    Ready {
        request: Request,
        outcome: Outcome,
        stage: Awaiting,
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
            outcome: None,
            stage,
        }
    }

    pub(super) fn request(&self) -> &Request {
        &self.request
    }

    /// A rejection keeps both the probe and the first answer received.
    pub(super) fn feed(&mut self, outcome: Outcome) -> Result<(), FeedError> {
        if self.request.id != outcome.id {
            return Err(FeedError::UnknownId(outcome.id));
        }
        if self.outcome.is_some() {
            return Err(FeedError::Duplicate(outcome.id));
        }
        if let Ok(reading) = &outcome.result
            && !self.request.probe.accepts(reading)
        {
            return Err(FeedError::KindMismatch(ProbeKindMismatch {
                expected: self.request.probe.kind(),
                got: reading.kind(),
            }));
        }
        self.outcome = Some(outcome);
        Ok(())
    }

    pub(super) fn resolve(self) -> Resolution {
        match self {
            Self {
                request,
                outcome: Some(outcome),
                stage,
            } => Resolution::Ready {
                request,
                outcome,
                stage,
            },
            pending => Resolution::Pending(pending),
        }
    }
}
