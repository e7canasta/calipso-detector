use super::box_detector::load;
use crate::{
    ModelError,
    ports::{DepthMap, DepthPort},
};
use image::DynamicImage;
use std::path::Path;
use ultralytics_inference::YOLOModel;

/// Depth model adapter (from v2's `DepthEstimator`): runs on the whole
/// image and hands over the backend map without touching its values.
#[derive(Debug)]
pub struct DepthEstimator {
    model: YOLOModel,
}

impl DepthEstimator {
    pub fn load(path: &Path) -> Result<Self, ModelError> {
        // Depth has no candidates the backend could discard.
        Ok(Self {
            model: load(path, 0.0)?,
        })
    }
}

impl DepthPort for DepthEstimator {
    fn estimate(&mut self, image: &DynamicImage) -> Result<DepthMap, ModelError> {
        let results = self
            .model
            .predict_image(image, "depth".into())
            .map_err(|error| ModelError::Inference(error.to_string()))?;
        // Without a map the backend ran and measured nothing: an empty map, which
        // yields no value at any point (as in v2).
        let Some(map) = results.iter().find_map(|r| r.depth.as_ref()) else {
            return Ok(DepthMap::default());
        };
        let (height, width) = (map.data.shape()[0], map.data.shape()[1]);
        Ok(DepthMap {
            width: width as u32,
            height: height as u32,
            values: map.data.iter().copied().collect(),
        })
    }
}
