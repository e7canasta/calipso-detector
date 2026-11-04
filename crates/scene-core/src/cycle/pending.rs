//! What the turn waits for, one variant per question. Each variant carries what
//! the turn has gathered so far and what defines the question; `split` consumes
//! it into the pair that cannot come apart: the `Probe` the shell looks at
//! (built once, its points and its crops moved into it, never cloned per look)
//! and the [`Continuation`] its answer is read back into.

use super::{Observed, Refined};
use crate::attention::{PoseTarget, VerifyLook};
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
        target: PoseTarget,
        up: Option<crate::look::Up>,
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

/// What a probe's answer is read back into: what the question carried, minus
/// what went into the probe itself. One variant per question, narrower than
/// [`Pending`]'s: the shell's crop, the points and the boxes the probe asked
/// with are gone, and only what the answer moves the turn on with remains.
#[derive(Debug)]
pub(super) enum Continuation {
    /// The main detector over the focus: its answer is looked at again over
    /// the whole frame if the focus does not confirm.
    Focused,
    /// The main detector over the whole frame.
    Primary,
    /// The finer detector over the bed, to reinforce a loss; `primary` is the
    /// main detector's reading it joins.
    Reinforce { primary: Admission },
    /// The face detector's answer, read in the region it was asked in.
    Faces {
        region: FaceRegion,
        policy: FacePolicy,
        bodies: Admission,
    },
    /// A doubtful body's pose, before believing: the floor body it is.
    PoseBody {
        target: PoseTarget,
        perceived: Perceived,
    },
    /// The depth of the empty bed, to learn it.
    Calibrate { observed: Observed },
    /// The subject's pose; its box `bbox` goes on as the motion reference.
    Pose { bbox: Rect, observed: Observed },
    /// The subject's depth, read at `at` and on the pose's keypoints.
    Depth {
        at: Vec<(DepthTarget, Point)>,
        pose: Vec<Keypoint>,
        bbox: Rect,
        observed: Observed,
        /// Where the head was when the look was decided.
        head: HeadPlace,
    },
    /// This sample's crop is kept for the burst of `ticket`: the turn concludes
    /// either way.
    Keep { refined: Refined },
    /// The verifier, over its crop or the kept crops: the turn concludes either
    /// way.
    Verify { refined: Refined },
}

impl Pending {
    /// The question, opened: the probe the shell looks at and the continuation
    /// its answer is read into. The probe takes what it asked with — the
    /// points of the depth and the crops of the verifier are moved, never
    /// cloned — and the continuation keeps what the answer is read with.
    pub(super) fn split(self) -> (Probe, Continuation) {
        match self {
            Pending::Detect {
                region: Some(region),
            } => (
                Probe::Detect {
                    region: Some(region),
                },
                Continuation::Focused,
            ),
            Pending::Detect { region: None } => {
                (Probe::Detect { region: None }, Continuation::Primary)
            }
            Pending::Reinforce { region, primary } => (
                Probe::Reinforce { region },
                Continuation::Reinforce { primary },
            ),
            Pending::Faces {
                region,
                policy,
                bodies,
            } => (
                Probe::Faces { region },
                Continuation::Faces {
                    region,
                    policy,
                    bodies,
                },
            ),
            Pending::PoseBody {
                region,
                target,
                up,
                perceived,
            } => (
                Probe::Pose {
                    bbox: region,
                    purpose: PosePurpose::Body,
                    up,
                },
                Continuation::PoseBody { target, perceived },
            ),
            Pending::Calibrate {
                region,
                points,
                observed,
            } => (
                Probe::Depth {
                    purpose: DepthPurpose::Bed,
                    points,
                    region,
                },
                Continuation::Calibrate { observed },
            ),
            Pending::Pose {
                region,
                bbox,
                observed,
            } => (
                Probe::Pose {
                    bbox: region,
                    purpose: PosePurpose::Subject,
                    up: None,
                },
                Continuation::Pose { bbox, observed },
            ),
            Pending::Depth {
                region,
                at,
                pose,
                bbox,
                observed,
                head,
            } => (
                Probe::Depth {
                    purpose: DepthPurpose::Subject,
                    points: at.iter().map(|&(_, p)| p).collect(),
                    region,
                },
                Continuation::Depth {
                    at,
                    pose,
                    bbox,
                    observed,
                    head,
                },
            ),
            Pending::Keep {
                ticket,
                region,
                refined,
            } => (
                Probe::Keep { ticket, region },
                Continuation::Keep { refined },
            ),
            Pending::Verify {
                ticket,
                look,
                crops,
                refined,
            } => (
                Probe::Verify {
                    ticket,
                    region: look.region,
                    bed: look.bed,
                    question: look.question,
                    crops,
                },
                Continuation::Verify { refined },
            ),
        }
    }
}
