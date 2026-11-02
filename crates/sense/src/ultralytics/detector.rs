use super::box_detector::BoxDetector;
use crate::{ModelError, ports::DetectorPort};
use image::DynamicImage;
use scene_look::Detection;
use std::path::Path;

/// Body detector adapter: no acceptance policy.
#[derive(Debug)]
pub struct Detector(BoxDetector);

impl Detector {
    pub fn load(path: &Path, confidence: f32) -> Result<Self, ModelError> {
        Ok(Self(BoxDetector::load(path, confidence)?))
    }
}

impl DetectorPort for Detector {
    fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
        self.0.predict(image)
    }
}
