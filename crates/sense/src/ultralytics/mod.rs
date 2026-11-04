//! Ultralytics implementation of the detection, pose and depth ports.
mod box_detector;
mod depth;
mod detector;
mod pose;
pub use depth::DepthEstimator;
pub use detector::Detector;
pub use pose::PoseEstimator;

use crate::ModelError;

/// The task each model runs; the backend's name for it lives here, once (F3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Task {
    Detect,
    Pose,
    Depth,
}

impl Task {
    /// The name the backend knows this task by.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Pose => "pose",
            Self::Depth => "depth",
        }
    }
}

impl From<Task> for String {
    fn from(task: Task) -> Self {
        task.as_str().to_owned()
    }
}

/// The backend's failure at inference time, translated once (F3).
impl From<ultralytics_inference::InferenceError> for ModelError {
    fn from(error: ultralytics_inference::InferenceError) -> Self {
        Self::Inference(error.to_string())
    }
}
