//! The loop's vocabulary: what the core asks to look at and what the shell
//! returns. Never pixels (rule 8).

use crate::body::{BedSite, Keypoint};
use crate::geom::{Point, Rect};
use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the core asks to look at. Whole-frame coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    /// The main person detector over a region (`None`: the whole frame).
    Detect {
        /// Where; `None`: the whole frame.
        region: Option<Rect>,
    },
    /// The face detector over the subject's box or the headboard.
    Faces {
        /// Where faces are looked for.
        region: FaceRegion,
    },
    /// The finer detector over the expanded bed.
    Reinforce {
        /// The bed grown by the policy's padding.
        region: Rect,
    },
    /// The pose over a body's box: the subject's, or a doubtful body's before
    /// believing (white book, volume VII, ch. 15). The sense does the same.
    Pose {
        /// Where to look: the body's box grown by the pose policy's margin.
        bbox: Rect,
        /// Whose pose it is.
        purpose: PosePurpose,
    },
    /// The depth at these points.
    Depth {
        /// The subject or the empty bed.
        purpose: DepthPurpose,
        /// Where to measure; the reading keeps this order.
        points: Vec<Point>,
        /// What the model sees; `None`: the whole frame. The points stay in
        /// whole-frame coordinates.
        region: Option<Rect>,
    },
    /// Keep this sample's crop for a burst (white book, volume III, ch. 12): the
    /// shell crops it and holds it under the ticket, to send it with the
    /// `Verify` that completes the burst. Nothing is asked yet.
    Keep {
        /// The burst it belongs to.
        ticket: Ticket,
        /// The crop's bounding box, in whole-frame pixels.
        region: Rect,
    },
    /// The slow verifier: a closed question about a crop. With `crops`, the
    /// crops kept for its ticket that go, by the order they were kept; empty,
    /// this sample's own crop. The shell only takes
    /// the request (`Reading::Dispatched`); the answer comes later, through
    /// [`Monitor::verified`](crate::Monitor::verified), as evidence of the
    /// sample it looked at (book 4).
    Verify {
        /// What the answer will be about.
        ticket: Ticket,
        /// The crop's bounding box, in whole-frame pixels (the ledger's region).
        region: Rect,
        /// The bed grown by the crop's margin, in `Quad`'s order (headboard
        /// left, headboard right, foot right, foot left): the shell rectifies
        /// the crop to it, the headboard always up.
        bed: crate::geom::Quad,
        /// The closed question.
        question: Question,
        /// Which kept crops go, by the order they were kept; empty: this
        /// sample's crop.
        crops: Vec<u8>,
    },
}

/// A request to the verifier: which, the sample it looks at and what it asks
/// about. A late answer is applied only if that same thing still holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ticket {
    /// Unique per monitor, in the order they were issued.
    pub id: u64,
    /// The sample whose crop is asked about: the answer is evidence of it.
    pub looked: SampleRef,
    /// What is asked about.
    pub target: Target,
}

/// What a verification asks about, by its identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// A body held in bed, by the last sample it was seen in.
    Held {
        /// The held body's `last_seen`.
        since: SampleRef,
    },
    /// The doubtful feet of the looked sample, while layer 1 holds the same
    /// stable state.
    Feet {
        /// Layer 1's stable state when it was asked.
        stable: crate::belief::bed_posture::BedPostureState,
    },
    /// The doubtful disparity of the looked sample, while layer 1 holds the
    /// same stable state.
    Disparity {
        /// Layer 1's stable state when it was asked.
        stable: crate::belief::bed_posture::BedPostureState,
    },
}

/// A closed question to the verifier: yes, no or unsure. Questions are
/// versioned, like a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Is there a person lying in this bed, even if covered?
    HeldInBed,
    /// Is the person in this bed sitting up rather than lying down?
    SittingInBed,
    /// Is the person getting out of the bed, with a leg over the side?
    LeavingBed,
}

impl Question {
    /// Its versioned id, which the verifier and the ledger use.
    pub fn id(self) -> &'static str {
        match self {
            Self::HeldInBed => "held_in_bed@1",
            Self::SittingInBed => "sitting_in_bed@1",
            Self::LeavingBed => "leaving_bed@1",
        }
    }
}

/// The verifier's answer to a closed question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Yes.
    Yes,
    /// No.
    No,
    /// It could not tell.
    Unsure,
}

/// An answer to a ticket, as the shell delivers it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verified {
    /// The ticket it answers.
    pub ticket: Ticket,
    /// What the verifier said.
    pub answer: Answer,
}

/// Whose pose is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosePurpose {
    /// The subject's, after believing: the second level and layer 1.
    Subject,
    /// A doubtful body's, before believing: the room places it with it.
    Body,
}

/// What the depth is measured for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthPurpose {
    /// The subject's pose keypoints.
    Subject,
    /// The bed's points, to calibrate it.
    Bed,
}

impl Probe {
    /// The look this probe is recorded as in the ledger.
    pub fn kind(&self) -> LookKind {
        match self {
            Probe::Detect { .. } => LookKind::Detect,
            Probe::Faces { .. } => LookKind::Faces,
            Probe::Reinforce { .. } => LookKind::Reinforce,
            Probe::Pose {
                purpose: PosePurpose::Subject,
                ..
            } => LookKind::Pose,
            Probe::Pose {
                purpose: PosePurpose::Body,
                ..
            } => LookKind::PoseBody,
            Probe::Depth {
                purpose: DepthPurpose::Subject,
                ..
            } => LookKind::Depth,
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                ..
            } => LookKind::Calibrate,
            Probe::Verify { .. } => LookKind::Verify,
            Probe::Keep { .. } => LookKind::Keep,
        }
    }

    /// The region recorded in the ledger.
    pub fn region(&self) -> Option<Rect> {
        match self {
            Probe::Detect { region } => *region,
            Probe::Faces { region } => Some(region.rect()),
            Probe::Reinforce { region } => Some(*region),
            Probe::Pose { bbox, .. } => Some(*bbox),
            Probe::Depth { region, .. } => *region,
            Probe::Verify { region, .. } | Probe::Keep { region, .. } => Some(*region),
        }
    }
}

/// Where to look for faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaceRegion {
    /// The subject's box as the detector saw it.
    Subject(Rect),
    /// The bed's headboard, when no subject is in sight.
    Headboard(Rect),
}

impl FaceRegion {
    /// The rectangle, whichever it is.
    pub fn rect(&self) -> Rect {
        match self {
            FaceRegion::Subject(r) | FaceRegion::Headboard(r) => *r,
        }
    }
}

/// Which class the model detected. The sense returns all of them (rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// A person.
    Person,
    /// Any other class: the policy discards it, with its reason.
    Other,
}

/// A box a model returned, unfiltered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    /// The box, in whole-frame pixels.
    pub bbox: Rect,
    /// The model's score.
    pub confidence: f32,
    /// The class it detected.
    pub label: Label,
}

/// What the shell read. Mirrors `Probe` one to one.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    /// What a person detector saw (`Detect`, `Reinforce`).
    Bodies(Vec<Detection>),
    /// What the face detector saw.
    Faces(Vec<Detection>),
    /// `None`: the model found no pose.
    Pose(Option<Vec<Keypoint>>),
    /// One depth per requested point; `None` where there was no reading.
    Depth(Vec<Option<f32>>),
    /// The verifier took the request (`Verify`): the answer comes later.
    Dispatched,
    /// The shell kept the crop for its burst (`Keep`), and how sharp it is
    /// (any scale, the same for every crop of the camera).
    Kept {
        /// The crop's sharpness.
        sharpness: f32,
    },
}

/// Milliseconds the shell measured. The core only records them (rule 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spent(pub f64);

/// The shell's answer to a probe, as the turn records it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) result: Result<Reading, Failure>,
    pub(crate) spent: Spent,
}

// The typed readings. A reading of another kind than its probe is a failing
// sense, not a core panic.
impl Outcome {
    pub(crate) fn bodies(&self) -> Result<&[Detection], Failure> {
        match &self.result {
            Ok(Reading::Bodies(d)) => Ok(d),
            Err(f) => Err(*f),
            Ok(_) => Err(Failure::Model),
        }
    }

    pub(crate) fn faces(&self) -> Result<&[Detection], Failure> {
        match &self.result {
            Ok(Reading::Faces(d)) => Ok(d),
            Err(f) => Err(*f),
            Ok(_) => Err(Failure::Model),
        }
    }

    pub(crate) fn into_pose(self) -> Result<Option<Vec<Keypoint>>, Failure> {
        match self.result {
            Ok(Reading::Pose(p)) => Ok(p),
            Err(f) => Err(f),
            Ok(_) => Err(Failure::Model),
        }
    }

    pub(crate) fn depths(&self) -> Result<&[Option<f32>], Failure> {
        match &self.result {
            Ok(Reading::Depth(d)) => Ok(d),
            Err(f) => Err(*f),
            Ok(_) => Err(Failure::Model),
        }
    }

    /// Did the shell keep the crop, and how sharp is it?
    pub(crate) fn kept(&self) -> Result<f32, Failure> {
        match &self.result {
            Ok(Reading::Kept { sharpness }) => Ok(*sharpness),
            Err(f) => Err(*f),
            Ok(_) => Err(Failure::Model),
        }
    }

    /// Did the verifier take the request?
    pub(crate) fn dispatched(&self) -> Result<(), Failure> {
        match &self.result {
            Ok(Reading::Dispatched) => Ok(()),
            Err(f) => Err(*f),
            Ok(_) => Err(Failure::Model),
        }
    }
}

/// Whose depth a point of the subject's depth look reads: a pose keypoint's,
/// by its index in the pose, or a bed site's (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepthTarget {
    Keypoint(usize),
    Bed(BedSite),
}

/// What was asked to be looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LookKind {
    /// The main person detector.
    Detect,
    /// The face detector.
    Faces,
    /// The finer detector over the bed, when the subject is lost.
    Reinforce,
    /// The subject's pose.
    Pose,
    /// A doubtful body's pose, before believing.
    PoseBody,
    /// The depth at image points.
    Depth,
    /// The depth of the empty bed, to learn it.
    Calibrate,
    /// The slow verifier, asked about a crop.
    Verify,
    /// A crop kept for a burst to the verifier.
    Keep,
}

impl LookKind {
    /// Its published name (`detect`, `faces`…).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Faces => "faces",
            Self::Reinforce => "reinforce",
            Self::Pose => "pose",
            Self::PoseBody => "pose_body",
            Self::Depth => "depth",
            Self::Calibrate => "calibrate",
            Self::Verify => "verify",
            Self::Keep => "keep",
        }
    }
}

impl std::fmt::Display for LookKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// A failure is data: the shell returns it and the ledger publishes it as is.
// The published schema names it `FailureKind`: renaming it changes the contract.
/// How a look failed, as published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "FailureKind")]
pub enum Failure {
    /// The shell could not complete the probe (shutdown).
    Cancelled,
    /// The sense ran and failed (model or backend error).
    Model,
}
