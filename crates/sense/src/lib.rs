//! Senses: runs each core `Probe` on a frame and returns a `Reading`.
//! It returns everything it sees; it knows no policies (rule 6).
// What is `pub` is reachable from outside; the rest says `pub(crate)`.
#![warn(unreachable_pub)]
// A `_` over the probes would hide a sense the compiler should point at (ADR
// 2026-11-02, quien-responde-cada-probe): every match here names each variant.
#![deny(clippy::wildcard_enum_match_arm)]

mod crop;

mod error;
pub mod ports;
pub use error::ModelError;
pub use ports::{DepthMap, DepthPort, DetectorPort, PersonPose, PosePort};
#[cfg(feature = "onnx")]
pub mod ultralytics;

#[cfg(any(feature = "onnx", test))]
mod detection;

use image::DynamicImage;
use scene_contract::Keypoint;
use scene_geom::{Point, Rect};
use scene_look::{Detection, Failure, Reading, SenseProbe};

/// Runs the probes with one detector per role; accepting and deciding belong to the core.
pub struct Senses {
    detect: Box<dyn DetectorPort>,
    reinforce: Option<Box<dyn DetectorPort>>,
    faces: Option<Box<dyn DetectorPort>>,
    pose: Option<Box<dyn PosePort>>,
    depth: Option<Box<dyn DepthPort>>,
}

impl std::fmt::Debug for Senses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Senses").finish_non_exhaustive()
    }
}

impl Senses {
    pub fn new(detect: impl DetectorPort + 'static) -> Self {
        Self {
            detect: Box::new(detect),
            reinforce: None,
            faces: None,
            pose: None,
            depth: None,
        }
    }

    pub fn with_reinforce(mut self, detector: impl DetectorPort + 'static) -> Self {
        self.reinforce = Some(Box::new(detector));
        self
    }

    pub fn with_faces(mut self, detector: impl DetectorPort + 'static) -> Self {
        self.faces = Some(Box::new(detector));
        self
    }

    pub fn with_pose(mut self, estimator: impl PosePort + 'static) -> Self {
        self.pose = Some(Box::new(estimator));
        self
    }

    pub fn with_depth(mut self, estimator: impl DepthPort + 'static) -> Self {
        self.depth = Some(Box::new(estimator));
        self
    }

    /// Answers in whole-frame coordinates. The time is not measured here: the
    /// shell's router measures each answer once, whichever answers it.
    pub fn look(&mut self, frame: &DynamicImage, probe: &SenseProbe) -> Result<Reading, Failure> {
        let result = match *probe {
            SenseProbe::Detect { region } => {
                boxes(self.detect.as_mut(), frame, region).map(Reading::Bodies)
            }
            SenseProbe::Reinforce { region } => match self.reinforce.as_mut() {
                Some(port) => boxes(port.as_mut(), frame, Some(region)).map(Reading::Bodies),
                None => Err(Failure::Model),
            },
            SenseProbe::Faces { region } => match self.faces.as_mut() {
                Some(port) => boxes(port.as_mut(), frame, Some(region.rect())).map(Reading::Faces),
                None => Err(Failure::Model),
            },
            SenseProbe::Pose { bbox, .. } => match self.pose.as_mut() {
                Some(port) => pose(port.as_mut(), frame, bbox).map(Reading::Pose),
                None => Err(Failure::Model),
            },
            SenseProbe::Depth { points, region, .. } => match self.depth.as_mut() {
                Some(port) => depth(port.as_mut(), frame, points, region).map(Reading::Depth),
                None => Err(Failure::Model),
            },
        };
        if result.is_err() {
            tracing::warn!(probe = ?probe.kind(), "sense failed or unavailable");
        }
        result
    }
}

fn boxes(
    port: &mut dyn DetectorPort,
    frame: &DynamicImage,
    region: Option<Rect>,
) -> Result<Vec<Detection>, Failure> {
    let crop = crop::Crop::of(frame, region);
    if crop.image.width() == 0 || crop.image.height() == 0 {
        return Err(Failure::Model);
    }
    port.detect(&crop.image)
        .map(|boxes| crop.to_frame(boxes))
        .map_err(|error| {
            tracing::warn!(%error, "model failed");
            Failure::Model
        })
}

/// The pose of the most confident person in the crop (ties: the first). The
/// seam carries a single pose; choosing is not filtering: nothing is dropped by
/// score and every keypoint goes (rule 6). Nobody in sight: `None`.
fn pose(
    port: &mut dyn PosePort,
    frame: &DynamicImage,
    bbox: Rect,
) -> Result<Option<Vec<Keypoint>>, Failure> {
    let crop = crop::Crop::of(frame, Some(bbox));
    if crop.image.width() == 0 || crop.image.height() == 0 {
        return Err(Failure::Model);
    }
    let people = port.estimate(&crop.image).map_err(|error| {
        tracing::warn!(%error, "pose model failed");
        Failure::Model
    })?;
    Ok(people
        .into_iter()
        .reduce(|best, p| {
            if p.confidence > best.confidence {
                p
            } else {
                best
            }
        })
        .map(|p| crop.keypoints_to_frame(p.keypoints)))
}

/// The depth at each point, aligned with the requested points. The model
/// runs on the requested region (the whole frame without one), the same for
/// both purposes (subject and bed); the points come and go in whole-frame
/// coordinates, and one outside the region has no value.
fn depth(
    port: &mut dyn DepthPort,
    frame: &DynamicImage,
    points: &[Point],
    region: Option<Rect>,
) -> Result<Vec<Option<f32>>, Failure> {
    let crop = crop::Crop::of(frame, region);
    if crop.image.width() == 0 || crop.image.height() == 0 {
        return Err(Failure::Model);
    }
    let map = port.estimate(&crop.image).map_err(|error| {
        tracing::warn!(%error, "depth model failed");
        Failure::Model
    })?;
    let size = (crop.image.width() as f32, crop.image.height() as f32);
    Ok(points
        .iter()
        .map(|p| map.sample(size, crop.point_to_crop(*p)))
        .collect())
}
