//! What the turn waits for, one variant per question. Each variant carries what
//! the turn has gathered so far and what defines the question; `probe` builds the
//! `Probe` the shell looks at, and `advance` reads the answer back into the turn
//! with the same variant. The pair cannot come apart: there is no probe without
//! its continuation, nor a continuation without the probe that asked for it.

use super::{Observed, Refined};
use crate::attention::VerifyLook;
use crate::body::HeadPlace;
use crate::body::Keypoint;
use crate::evidence::{Admission, Perceived};
use crate::geom::{Point, Rect};
use crate::look::{DepthPurpose, FaceRegion, PosePurpose, Probe, Ticket};
use crate::outcome::DepthTarget;
use crate::policy::FacePolicy;

/// A question the turn is waiting on, with what its answer is read into.
#[derive(Debug)]
pub(super) enum Pending {
    /// The main detector: over the focus (`Some`), or over the whole frame
    /// (`None`). A focus that does not confirm widens to the whole frame.
    Detect { region: Option<Rect> },
    /// The finer detector over the bed, to reinforce a loss; `primary` is the
    /// main detector's reading it joins.
    Reinforce { region: Rect, primary: Admission },
    /// The face detector over the subject's box or the headboard.
    Faces {
        region: FaceRegion,
        policy: FacePolicy,
        bodies: Admission,
    },
    /// A doubtful body's pose, before believing: the floor body it is.
    PoseBody {
        region: Rect,
        floor: usize,
        perceived: Perceived,
    },
    /// The depth of the empty bed, to learn it.
    Calibrate {
        region: Option<Rect>,
        points: Vec<Point>,
        observed: Observed,
    },
    /// The subject's pose; its box `bbox` goes on as the motion reference.
    Pose {
        region: Rect,
        bbox: Rect,
        observed: Observed,
    },
    /// The subject's depth: at the pose's keypoints and the bed's sites, each
    /// point with whose it is.
    Depth {
        region: Option<Rect>,
        at: Vec<(DepthTarget, Point)>,
        pose: Vec<Keypoint>,
        bbox: Rect,
        observed: Observed,
        /// Where the head was when the look was decided.
        head: HeadPlace,
    },
    /// Keep this sample's crop for the burst of `ticket` (white book, volume III,
    /// ch. 12). The turn concludes either way.
    Keep {
        ticket: Ticket,
        region: Rect,
        refined: Refined,
    },
    /// The verifier, over its crop or the kept crops, with the ticket its answer
    /// carries. The turn concludes either way.
    Verify {
        ticket: Ticket,
        look: VerifyLook,
        crops: Vec<u8>,
        refined: Refined,
    },
}

impl Pending {
    /// The probe the shell looks at for this question.
    pub(super) fn probe(&self) -> Probe {
        match self {
            Pending::Detect { region } => Probe::Detect { region: *region },
            Pending::Reinforce { region, .. } => Probe::Reinforce { region: *region },
            Pending::Faces { region, .. } => Probe::Faces { region: *region },
            Pending::PoseBody { region, .. } => Probe::Pose {
                bbox: *region,
                purpose: PosePurpose::Body,
            },
            Pending::Calibrate { region, points, .. } => Probe::Depth {
                purpose: DepthPurpose::Bed,
                points: points.clone(),
                region: *region,
            },
            Pending::Pose { region, .. } => Probe::Pose {
                bbox: *region,
                purpose: PosePurpose::Subject,
            },
            Pending::Depth { region, at, .. } => Probe::Depth {
                purpose: DepthPurpose::Subject,
                points: at.iter().map(|&(_, p)| p).collect(),
                region: *region,
            },
            Pending::Keep { ticket, region, .. } => Probe::Keep {
                ticket: *ticket,
                region: *region,
            },
            Pending::Verify {
                ticket,
                look,
                crops,
                ..
            } => Probe::Verify {
                ticket: *ticket,
                region: look.region,
                bed: look.bed,
                question: look.question,
                crops: crops.clone(),
            },
        }
    }
}
