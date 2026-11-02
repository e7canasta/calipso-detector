//! Model boundaries. Each implementation translates the backend tensors into
//! plain values in the coordinates of the image it received; `Crop` takes them to
//! the whole frame.

use crate::ModelError;
use image::DynamicImage;
use scene_contract::Keypoint;
use scene_geom::Point;
use scene_look::Detection;

pub trait DetectorPort {
    /// Every detection the backend returns, in `image` coordinates.
    fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError>;
}

/// A person the pose model saw: the confidence of their box and their keypoints,
/// in the coordinates of the image it received, in model order (COCO).
#[derive(Debug, Clone, PartialEq)]
pub struct PersonPose {
    pub confidence: f32,
    pub keypoints: Vec<Keypoint>,
}

pub trait PosePort {
    /// Every person with a pose the backend returns in `image`.
    fn estimate(&mut self, image: &DynamicImage) -> Result<Vec<PersonPose>, ModelError>;
}

/// The depth map of an image: `width × height` values, row by row, in the
/// proportions of the image the model received (it may have another
/// resolution). Values are as the backend gives them, with no scale of our own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DepthMap {
    pub width: u32,
    pub height: u32,
    pub values: Vec<f32>,
}

impl DepthMap {
    /// The value at `point`, in the coordinates of a frame of `size` (width, height),
    /// proportional to the map. Outside the frame (edges count), not finite, or
    /// a value that is not finite and positive (the backend's mark for "no
    /// value"): `None`.
    pub fn sample(&self, (width, height): (f32, f32), point: Point) -> Option<f32> {
        let Point { x, y } = point;
        let inside = x.is_finite()
            && y.is_finite()
            && (0.0..=width).contains(&x)
            && (0.0..=height).contains(&y);
        if !inside || self.width == 0 || self.height == 0 {
            return None;
        }
        // A point on the last edge falls in the last cell.
        let column = ((x / width * self.width as f32) as u32).min(self.width - 1);
        let row = ((y / height * self.height as f32) as u32).min(self.height - 1);
        self.values
            .get((row * self.width + column) as usize)
            .copied()
            .filter(|v| v.is_finite() && *v > 0.0)
    }
}

pub trait DepthPort {
    /// El mapa de profundidad de toda `image`.
    fn estimate(&mut self, image: &DynamicImage) -> Result<DepthMap, ModelError>;
}
