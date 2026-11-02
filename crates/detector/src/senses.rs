//! The senses port: who answers the core's probes over a frame. Live,
//! the `sense` models over an image. The core cannot tell: it only sees
//! `Probe → Reading`.
use crate::config::Config;
use anyhow::{Context, Result};
use image::DynamicImage;
use scene_core::{
    look::{Failure, Reading, SenseProbe},
    map::FrameInfo,
};

/// What can be looked at in a sample. It knows its size: the core needs it to
/// know whether the bed geometry applies.
pub trait Frame {
    fn info(&self) -> FrameInfo;
}

impl Frame for DynamicImage {
    fn info(&self) -> FrameInfo {
        FrameInfo {
            width: self.width(),
            height: self.height(),
        }
    }
}

/// Who looks: it answers each sense probe over the sample's frame. It does not
/// measure time: the router does (`crate::router`).
pub trait SensesPort {
    type Frame: Frame;
    fn look(&mut self, frame: &Self::Frame, probe: &SenseProbe<'_>) -> Result<Reading, Failure>;
}

/// Live: the models over the image.
impl SensesPort for sense::Senses {
    type Frame = DynamicImage;
    fn look(&mut self, frame: &DynamicImage, probe: &SenseProbe<'_>) -> Result<Reading, Failure> {
        sense::Senses::look(self, frame, probe)
    }
}

/// Loads the profile's models: the main detector always, and each optional
/// sense whose section is present.
pub fn load(config: &Config) -> Result<sense::Senses> {
    use sense::ultralytics::{DepthEstimator, Detector, PoseEstimator};
    let senses = &config.senses;
    let path = |model| {
        let path = config.model_path(model);
        tracing::info!(model = %path.display(), "loads ONNX model");
        path
    };
    let detect = Detector::load(
        &path(&senses.detect.model),
        senses.detect.backend_confidence,
    )
    .context("load ONNX detector")?;
    let mut loaded = sense::Senses::new(detect);
    if let Some(reinforce) = &senses.reinforce {
        let path = path(&reinforce.model);
        let model = Detector::load(&path, reinforce.backend_confidence)
            .with_context(|| format!("load ONNX reinforcement: {}", path.display()))?;
        loaded = loaded.with_reinforce(model);
    }
    if let Some(faces) = &senses.faces {
        let path = path(&faces.model);
        let model = Detector::load(&path, faces.backend_confidence)
            .with_context(|| format!("load ONNX faces: {}", path.display()))?;
        loaded = loaded.with_faces(model);
    }
    if let Some(pose) = &senses.pose {
        let path = path(&pose.model);
        let model = PoseEstimator::load(&path, pose.backend_confidence)
            .with_context(|| format!("load ONNX pose: {}", path.display()))?;
        loaded = loaded.with_pose(model);
    }
    if let Some(depth) = &senses.depth {
        let path = path(&depth.model);
        let model = DepthEstimator::load(&path)
            .with_context(|| format!("load ONNX depth: {}", path.display()))?;
        loaded = loaded.with_depth(model);
    }
    Ok(loaded)
}
