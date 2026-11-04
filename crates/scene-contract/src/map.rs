//! The site map: whether the delimited bed matches the incoming image, and
//! the geometry's shadow over the site.

use crate::FrameInfo;
use scene_geom::{Quad, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- moved from scene-core/src/map.rs
/// Whether the delimited bed matches the incoming image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Geometry {
    /// The image has the resolution the bed was delimited for.
    Valid,
    /// The resolution is not the declared one: "in bed" cannot be claimed.
    Invalid {
        /// The resolution the bed was delimited for.
        declared: FrameInfo,
        /// The resolution of the incoming image.
        actual: FrameInfo,
    },
}

// --- moved from scene-core/src/map.rs
/// The geometry's shadow over the site (white book, volume VII, ch. 11): it
/// decides nothing; it is published to choose with data. Temporary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapShadow {
    /// Where the bed stands on the floor, if delimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_floor: Option<Quad>,
    /// The bed's width and length in metres, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_size_m: Option<[f32; 2]>,
    /// Near the bed in metres: the footprint (the mattress, without one) grown
    /// by the shadow's distance in its own coordinates. Absent without the
    /// bed's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near: Option<Quad>,
    /// Near the bed as decided today: the bed's bounding box grown by the
    /// admission's margin.
    pub near_today: Rect,
    /// The headboard as a quadrilateral, if it was drawn as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headboard_area: Option<Quad>,
}
