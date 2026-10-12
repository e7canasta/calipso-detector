//! The site map: the static world the subject does not occlude.

use crate::geom::{Point, Quad, Rect};
use crate::policy::FocusPolicy;
use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Internal fact: the projection adds the context of the published event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedCalibrated {
    pub at: SampleRef,
    pub reference_m: f32,
}

/// Initial site configuration: data, not behaviour. `Monitor` splits geometry
/// and memory on receiving it, and the loop's questions (does the geometry
/// apply? where is the bed?) are answered by the running map.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteMap {
    /// The bed on the image plane.
    pub bed: Quad,
    /// The resolution the bed was delimited for; `None`: it always applies.
    pub resolution: Option<FrameInfo>,
    /// Where the headboard is: where to look for faces with no subject in sight.
    pub headboard: Option<Rect>,
    /// Where the main detector looks while the resident is alone in bed;
    /// `None`: the bed grown by the focus policy's pad.
    pub focus: Option<Rect>,
    /// The learned depth of the empty bed, in the depth model's relative scale.
    pub bed_reference: Option<f32>,
    /// The last calibration attempt, for the retry delay.
    pub calibration_attempt: Option<SampleRef>,
    pub(crate) calibrated_at: Option<SampleRef>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SiteGeometry {
    pub bed: Quad,
    pub resolution: Option<FrameInfo>,
    pub headboard: Option<Rect>,
    pub focus: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedReference {
    pub depth: f32,
    pub since: Option<SampleRef>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct CalibrationMemory {
    reference: Option<BedReference>,
    last_attempt: Option<SampleRef>,
}

/// The running map: configured geometry and learned memory, with different owners.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Map {
    pub site: SiteGeometry,
    calibration: CalibrationMemory,
}

impl From<SiteMap> for Map {
    fn from(initial: SiteMap) -> Self {
        Self {
            site: SiteGeometry {
                bed: initial.bed,
                resolution: initial.resolution,
                headboard: initial.headboard,
                focus: initial.focus,
            },
            calibration: CalibrationMemory {
                reference: initial.bed_reference.map(|depth| BedReference {
                    depth,
                    since: initial.calibrated_at,
                }),
                last_attempt: initial.calibration_attempt,
            },
        }
    }
}

impl SiteMap {
    /// A site with only its bed: no resolution, headboard or calibration.
    pub fn new(bed: Quad) -> Self {
        Self {
            bed,
            resolution: None,
            headboard: None,
            focus: None,
            bed_reference: None,
            calibration_attempt: None,
            calibrated_at: None,
        }
    }

    /// Where to measure the bed's depth: its corners and its centre. For
    /// measuring outside the loop; inside the loop, ask the running map.
    pub fn bed_points(&self) -> Vec<Point> {
        bed_points(&self.bed)
    }
}

impl Map {
    pub(crate) fn reference(&self) -> Option<BedReference> {
        self.calibration.reference
    }

    pub(crate) fn last_attempt(&self) -> Option<SampleRef> {
        self.calibration.last_attempt
    }

    pub(crate) fn bed_points(&self) -> Vec<Point> {
        bed_points(&self.site.bed)
    }

    /// Calibration was requested: the retry delay runs from here.
    pub(crate) fn attempted(&mut self, at: SampleRef) {
        self.calibration.last_attempt = Some(at);
    }

    /// Learns the depth of the empty bed and says so with a domain event;
    /// without a valid measurement it learns nothing.
    pub(crate) fn learn(&mut self, at: SampleRef, depth: Option<f32>) -> Option<BedCalibrated> {
        let depth = depth?;
        self.calibration.reference = Some(BedReference {
            depth,
            since: Some(at),
        });
        Some(BedCalibrated {
            at,
            reference_m: depth,
        })
    }

    /// Does the bed geometry apply to this frame? (F-09)
    pub(crate) fn gate(&self, frame: FrameInfo) -> Geometry {
        gate(self.site.resolution, frame)
    }

    /// The headboard, if the geometry applies.
    pub(crate) fn headboard_for(&self, geometry: Geometry) -> Option<Rect> {
        self.site.headboard.filter(|_| geometry == Geometry::Valid)
    }

    /// Where the main detector looks while focusing, if the geometry applies:
    /// the delimited focus, or else the bed grown by `pad`. The look and the
    /// published map read it here: they cannot differ.
    pub(crate) fn focus_region(&self, geometry: Geometry, pad: f32) -> Option<Rect> {
        let bed = self.bed_for(geometry)?;
        Some(self.site.focus.unwrap_or_else(|| bed.bounds().expand(pad)))
    }

    /// Where depth is measured with a pad: the bed's box grown by it, centred
    /// on the bed. Without the geometry, nowhere.
    pub(crate) fn depth_region(&self, geometry: Geometry, pad: f32) -> Option<Rect> {
        self.bed_for(geometry).map(|bed| bed.bounds().expand(pad))
    }

    /// The bed, if the geometry applies.
    pub(crate) fn bed_for(&self, geometry: Geometry) -> Option<&Quad> {
        (geometry == Geometry::Valid).then_some(&self.site.bed)
    }

    /// The static world as this turn sees it: the configured geometry and
    /// what was learned of the empty bed.
    /// With the focus the policy configured, if any.
    pub(crate) fn state(&self, geometry: Geometry, focus: Option<&FocusPolicy>) -> MapState {
        MapState {
            focus: focus.and_then(|f| self.focus_region(geometry, f.pad)),
            geometry,
            bed: self.site.bed,
            headboard: self.site.headboard,
            calibration: self
                .reference()
                .map_or(Calibration::Unavailable, |reference| {
                    Calibration::Complete {
                        reference_m: reference.depth,
                        since: reference.since,
                    }
                }),
        }
    }
}

fn bed_points(bed: &Quad) -> Vec<Point> {
    let mut points = bed.0.to_vec();
    points.push(bed.bounds().centroid());
    points
}

fn gate(resolution: Option<FrameInfo>, frame: FrameInfo) -> Geometry {
    match resolution {
        Some(declared) if declared != frame => Geometry::Invalid {
            declared,
            actual: frame,
        },
        _ => Geometry::Valid,
    }
}

/// Size of a decoded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FrameInfo {
    /// In pixels.
    pub width: u32,
    /// In pixels.
    pub height: u32,
}

/// Whether the delimited bed matches the incoming image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
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

/// What is published of the static world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapState {
    /// Whether the delimited bed applies to this image.
    pub geometry: Geometry,
    /// The bed's four corners on the image, clockwise.
    pub bed: Quad,
    /// Where faces are looked for when nobody is in sight.
    pub headboard: Option<Rect>,
    /// Where the main detector looks while the resident is alone in bed: the
    /// delimited focus or the bed grown by the focus policy's pad. Absent, and
    /// not published, when the main detector does not focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Rect>,
    /// What the map learned about the empty bed.
    pub calibration: Calibration,
}

/// The learned bed depth and the sample it came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Calibration {
    /// The empty bed has not been learned.
    #[default]
    Unavailable,
    /// The empty bed was learned.
    Complete {
        /// Its depth, on the depth model's scale (the name says metres, the
        /// value is relative: see `BodyPoint`).
        reference_m: f32,
        /// The sample it was learned in; `None` if the profile brought it.
        since: Option<SampleRef>,
    },
}
