//! What the turn records of a look: the shell's answer to a probe, and which
//! body a depth point reads. The names of the language are `scene-look`'s; these
//! two belong to the core's turn and never leave it.

use scene_contract::{BedSite, Keypoint};
use scene_look::{Detection, Failure, Reading, Spent};

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
