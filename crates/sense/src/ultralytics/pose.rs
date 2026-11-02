use super::box_detector::load;
use crate::{
    ModelError,
    ports::{PersonPose, PosePort},
};
use image::DynamicImage;
use scene_contract::Keypoint;
use std::path::Path;
use ultralytics_inference::YOLOModel;

/// Pose model adapter (from v2's `PoseEstimator`): every person in the
/// crop, with the confidence of their box and their COCO keypoints.
#[derive(Debug)]
pub struct PoseEstimator {
    model: YOLOModel,
}

impl PoseEstimator {
    /// `confidence` is the backend floor for each person's box.
    pub fn load(path: &Path, confidence: f32) -> Result<Self, ModelError> {
        Ok(Self {
            model: load(path, confidence)?,
        })
    }
}

impl PosePort for PoseEstimator {
    fn estimate(&mut self, image: &DynamicImage) -> Result<Vec<PersonPose>, ModelError> {
        let results = self
            .model
            .predict_image(image, "pose".into())
            .map_err(|error| ModelError::Inference(error.to_string()))?;
        let mut people = Vec::new();
        for result in &results {
            let Some(keypoints) = result.keypoints.as_ref() else {
                continue;
            };
            let data = &keypoints.data;
            let (n, k, channels) = (data.shape()[0], data.shape()[1], data.shape()[2]);
            let confidences = result.boxes.as_ref().map(|b| b.conf());
            for person in 0..n {
                people.push(PersonPose {
                    // Without boxes the backend gave no confidence: it counts as full.
                    confidence: confidences
                        .as_ref()
                        .and_then(|c| c.get(person).copied())
                        .unwrap_or(1.0),
                    keypoints: (0..k)
                        .map(|j| Keypoint {
                            x: data[[person, j, 0]],
                            y: data[[person, j, 1]],
                            // Without a visibility channel, as in v2: full score.
                            score: if channels > 2 {
                                data[[person, j, 2]]
                            } else {
                                1.0
                            },
                        })
                        .collect(),
                });
            }
        }
        Ok(people)
    }
}
