//! The room's geometry (white book, volume VII, ch. 01): where a body is, in
//! bed or on the floor, and whether it is near the bed. It answers layer 0's
//! questions of space; it remembers nothing and counts nothing.

use crate::geom::{Quad, Rect};
use crate::policy::AdmissionPolicy;

/// Where a body falls in the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyZone {
    Bed,
    Floor { near_bed: bool },
}

/// The camera that sees the bed along the image (volume VII, ch. 02): the
/// engine's answers as they were, moved.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frontal<'a> {
    bed: &'a Quad,
    near: Near,
}

/// What "near the bed" is measured against.
#[derive(Debug, Clone, Copy)]
enum Near {
    /// The bed's bounding box grown by the margin: a box overlapping it.
    Bounds(Rect),
    /// In the bed frame, the bed grown in its own coordinates: a box touching it.
    Grown(Quad),
}

impl<'a> Frontal<'a> {
    pub(crate) fn of(bed: &'a Quad, policy: &AdmissionPolicy) -> Self {
        let margin = policy.near_bed_margin;
        let near = match policy
            .bed_frame
            .and_then(|_| crate::map::grown_bed(bed, f64::from(margin)))
        {
            Some(grown) => Near::Grown(grown),
            None => Near::Bounds(bed.bounds().expand(margin)),
        };
        Self { bed, near }
    }

    /// In bed if the box's centre is in the bed; on the floor otherwise, near
    /// the bed if the box reaches it.
    pub(crate) fn body_zone(&self, bbox: &Rect) -> BodyZone {
        if self.bed.contains(bbox.centroid()) {
            return BodyZone::Bed;
        }
        let near_bed = match &self.near {
            Near::Grown(grown) => grown.touches(bbox),
            Near::Bounds(near) => bbox.iou(near) > 0.0,
        };
        BodyZone::Floor { near_bed }
    }
}
