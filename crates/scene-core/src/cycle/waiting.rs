//! Builds each probe with its continuation.

use super::{Observed, Refined};
use crate::attention::{BodyPoseLook, DepthLook, SubjectDepthLook, VerifyLook};
use crate::body::Keypoint;
use crate::evidence::{Admission, Perceived};
use crate::geom::{Point, Rect};
use crate::head::HeadPlace;
use crate::look::{DepthPurpose, DepthTarget, FaceRegion, PosePurpose, Probe, Ticket};
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
    /// A doubtful body's pose, before believing: the floor body it is.
    PoseBody {
        perceived: Perceived,
        floor: usize,
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
        /// Whose each requested point is: a pose keypoint or a bed site.
        at: Vec<(DepthTarget, Point)>,
        /// Where the head was when the look was decided.
        head: HeadPlace,
    },
    /// The verifier takes the request; the turn concludes either way.
    Verify {
        refined: Refined,
    },
    /// The shell keeps a crop for a burst; the turn concludes either way.
    Keep {
        refined: Refined,
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

    /// A doubtful body's pose over `look.region`, before believing.
    pub(super) fn pose_body(perceived: Perceived, look: BodyPoseLook) -> Self {
        Self::new(
            Probe::Pose {
                bbox: look.region,
                purpose: PosePurpose::Body,
            },
            Continuation::PoseBody {
                perceived,
                floor: look.floor,
            },
        )
    }

    pub(super) fn calibrate(observed: Observed, look: DepthLook<Point>) -> Self {
        Self::new(
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                points: look.points,
                region: look.region,
            },
            Continuation::Calibrate { observed },
        )
    }

    /// The pose over `region`; the subject's `bbox` goes on as the motion reference.
    pub(super) fn pose(observed: Observed, bbox: Rect, region: Rect) -> Self {
        Self::new(
            Probe::Pose {
                bbox: region,
                purpose: PosePurpose::Subject,
            },
            Continuation::Pose { observed, bbox },
        )
    }

    pub(super) fn depth(
        observed: Observed,
        pose: Vec<Keypoint>,
        bbox: Rect,
        look: SubjectDepthLook,
    ) -> Self {
        let SubjectDepthLook { look, head } = look;
        let region = look.region;
        let at = look.points;
        let points = at.iter().map(|&(_, p)| p).collect();
        Self::new(
            Probe::Depth {
                purpose: DepthPurpose::Subject,
                points,
                region,
            },
            Continuation::Depth {
                observed,
                pose,
                bbox,
                at,
                head,
            },
        )
    }

    /// The verifier, over its crop or the kept crops `crops`, with the ticket
    /// its answer will carry.
    pub(super) fn verify(
        refined: Refined,
        ticket: Ticket,
        look: VerifyLook,
        crops: Vec<u8>,
    ) -> Self {
        Self::new(
            Probe::Verify {
                ticket,
                region: look.region,
                bed: look.bed,
                question: look.question,
                crops,
            },
            Continuation::Verify { refined },
        )
    }

    /// A crop kept for the burst of `ticket` (white book, volume III, ch. 12).
    pub(super) fn keep(refined: Refined, ticket: Ticket, region: Rect) -> Self {
        Self::new(
            Probe::Keep { ticket, region },
            Continuation::Keep { refined },
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
