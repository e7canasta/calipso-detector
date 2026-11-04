use image::DynamicImage;
use std::path::Path;
use ultralytics_inference::{InferenceConfig, YOLOModel};

use super::Task;
use crate::{ModelError, detection::from_backend};
use scene_look::Detection;

/// Loads a model of any task with the backend confidence floor.
pub(super) fn load(path: &Path, confidence: f32) -> Result<YOLOModel, ModelError> {
    // The backend prints one line per image; the time of each look is already
    // in the ledger (`spent_ms`). As in v2, it is silenced.
    ultralytics_inference::logging::set_verbose(false);
    let config = InferenceConfig::new().with_confidence(confidence);
    YOLOModel::load_with_config(path, config).map_err(|error| ModelError::Load(error.to_string()))
}

/// Shared model execution and box decoding; no acceptance policy.
#[derive(Debug)]
pub(super) struct BoxDetector {
    model: YOLOModel,
}

impl BoxDetector {
    /// `confidence` is the backend threshold; the core applies the acceptance
    /// policy afterwards.
    pub(super) fn load(path: &Path, confidence: f32) -> Result<Self, ModelError> {
        Ok(Self {
            model: load(path, confidence)?,
        })
    }

    pub(super) fn predict(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
        let results = self.model.predict_image(image, Task::Detect.into())?;

        let mut detections = Vec::new();
        for result in &results {
            let Some(boxes) = result.boxes.as_ref() else {
                continue;
            };
            let xyxy = boxes.xyxy();
            for i in 0..boxes.len() {
                let class = boxes.cls()[i] as usize;
                let b = xyxy.row(i);
                detections.push(from_backend(
                    [b[0], b[1], b[2], b[3]],
                    boxes.conf()[i],
                    result.names.get(&class).map(String::as_str),
                ));
            }
        }
        Ok(detections)
    }
}
