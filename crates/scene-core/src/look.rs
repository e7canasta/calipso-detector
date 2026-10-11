//! The loop's vocabulary: what the core asks to look at and what the shell
//! returns. Never pixels (rule 8).

use crate::body::Keypoint;
use crate::geom::{Point, Rect};
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
    /// The pose over the subject's box.
    Pose {
        /// The subject's box.
        bbox: Rect,
    },
    /// The depth at these points.
    Depth {
        /// The subject or the empty bed.
        purpose: DepthPurpose,
        /// Where to measure; the reading keeps this order.
        points: Vec<Point>,
    },
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
            Probe::Pose { .. } => LookKind::Pose,
            Probe::Depth {
                purpose: DepthPurpose::Subject,
                ..
            } => LookKind::Depth,
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                ..
            } => LookKind::Calibrate,
        }
    }

    /// Does this reading answer this probe?
    pub fn accepts(&self, reading: &Reading) -> bool {
        matches!(
            (self, reading),
            (
                Probe::Detect { .. } | Probe::Reinforce { .. },
                Reading::Bodies(_)
            ) | (Probe::Faces { .. }, Reading::Faces(_))
                | (Probe::Pose { .. }, Reading::Pose(_))
                | (Probe::Depth { .. }, Reading::Depth(_))
        )
    }

    /// The region recorded in the ledger.
    pub fn region(&self) -> Option<Rect> {
        match self {
            Probe::Detect { region } => *region,
            Probe::Faces { region } => Some(region.rect()),
            Probe::Reinforce { region } => Some(*region),
            Probe::Pose { bbox } => Some(*bbox),
            Probe::Depth { .. } => None,
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
}

impl Reading {
    /// The kind of probe this reading answers.
    pub fn kind(&self) -> LookKind {
        match self {
            Reading::Bodies(_) => LookKind::Detect,
            Reading::Faces(_) => LookKind::Faces,
            Reading::Pose(_) => LookKind::Pose,
            Reading::Depth(_) => LookKind::Depth,
        }
    }
}

/// Why a sense could not read. A failure is data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The shell could not complete the probe (shutdown).
    Cancelled,
    /// The sense ran and failed.
    Model,
}

impl Failure {
    /// Its published form.
    pub fn kind(&self) -> FailureKind {
        match self {
            Failure::Cancelled => FailureKind::Cancelled,
            Failure::Model => FailureKind::Model,
        }
    }
}

/// Identifies a probe within a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LookId(pub u32);

/// Milliseconds the shell measured. The core only records them (rule 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spent(pub f64);

/// The look the turn waits for.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Its id within the turn: the answer must carry it.
    pub id: LookId,
    /// What to look at, and where.
    pub probe: Probe,
}

/// The shell's answer to a probe.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The id of the request it answers.
    pub id: LookId,
    /// What the sense read, or why it could not.
    pub result: Result<Reading, Failure>,
    /// How long looking took.
    pub spent: Spent,
}

// The typed readings. `feed` already rejected those of another kind
// (KindMismatch), so the last arm does not happen; if it did, it is a failing
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
    /// The depth at image points.
    Depth,
    /// The depth of the empty bed, to learn it.
    Calibrate,
}

/// A reading that does not match the probe (protocol misuse).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeKindMismatch {
    /// The probe's kind.
    pub expected: LookKind,
    /// The reading's kind.
    pub got: LookKind,
}

impl LookKind {
    /// Its published name (`detect`, `faces`…).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Faces => "faces",
            Self::Reinforce => "reinforce",
            Self::Pose => "pose",
            Self::Depth => "depth",
            Self::Calibrate => "calibrate",
        }
    }
}

impl std::fmt::Display for LookKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Display for LookId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "look {}", self.0)
    }
}

impl std::fmt::Display for ProbeKindMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a {} reading for a {} probe", self.got, self.expected)
    }
}

impl std::error::Error for ProbeKindMismatch {}

/// How a look failed, as published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// The shell could not complete the probe (shutdown).
    Cancelled,
    /// The sense ran and failed (model or backend error).
    Model,
}
