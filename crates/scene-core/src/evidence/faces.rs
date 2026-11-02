//! Faces at the headboard: which faces count, and where.

use super::{RejectReason, Sensed, count};
use crate::attention::LookResult;
use crate::geom::Rect;
use crate::look::{Detection, Failure};
use crate::policy::FacePolicy;

/// What the faces of this sample prove.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum FaceEvidence {
    /// Faces were not looked for.
    #[default]
    NotLooked,
    Unavailable(Failure),
    None,
    /// Faces accepted in the requested region.
    Seen(Vec<Detection>),
}

impl FaceEvidence {
    pub(crate) fn seen(&self) -> bool {
        matches!(self, FaceEvidence::Seen(_))
    }
}

/// The face evidence and what is recorded in the ledger.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FaceAdmission {
    pub faces: FaceEvidence,
    pub result: LookResult,
}

/// Accepts the faces with enough confidence inside the requested region.
pub(crate) fn faces(
    reading: Sensed<&[Detection]>,
    region: Rect,
    policy: &FacePolicy,
) -> FaceAdmission {
    let detections = match reading {
        Err(failure) => {
            return FaceAdmission {
                faces: FaceEvidence::Unavailable(failure),
                result: LookResult::failed(failure),
            };
        }
        Ok(d) => d,
    };
    let inside = |d: &Detection| {
        let c = d.bbox.centroid();
        c.x >= region.x0 && c.x <= region.x1 && c.y >= region.y0 && c.y <= region.y1
    };
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for d in detections {
        if d.confidence < policy.min_confidence.get() {
            rejected.push(RejectReason::LowConfidence);
        } else if !inside(d) {
            rejected.push(RejectReason::OutsideRegion);
        } else {
            accepted.push(*d);
        }
    }
    FaceAdmission {
        faces: if accepted.is_empty() {
            FaceEvidence::None
        } else {
            FaceEvidence::Seen(accepted)
        },
        result: LookResult::Read {
            candidates: count(detections.len()),
            rejected,
        },
    }
}
