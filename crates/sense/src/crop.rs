use image::DynamicImage;
use scene_core::{
    contract::Keypoint,
    geom::{Point, Rect},
    look::Detection,
};
use std::borrow::Cow;

/// A crop and its pixel origin inside the frame. Without a region, the original is borrowed.
#[derive(Debug)]
pub(crate) struct Crop<'a> {
    pub(crate) image: Cow<'a, DynamicImage>,
    origin: Point,
}

impl<'a> Crop<'a> {
    /// Clips to the frame edges, rounding outwards to cover the requested
    /// region. An empty region stays empty: it never falls back to the whole frame.
    pub(crate) fn of(frame: &'a DynamicImage, region: Option<Rect>) -> Self {
        match region {
            Some(region) => {
                let x = (region.x0.floor() as u32).min(frame.width());
                let y = (region.y0.floor() as u32).min(frame.height());
                let right = (region.x1.ceil() as u32).min(frame.width());
                let bottom = (region.y1.ceil() as u32).min(frame.height());
                Self {
                    image: Cow::Owned(DynamicImage::ImageRgb8(
                        frame
                            .crop_imm(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
                            .into_rgb8(),
                    )),
                    origin: Point {
                        x: x as f32,
                        y: y as f32,
                    },
                }
            }
            None => Self {
                image: Cow::Borrowed(frame),
                origin: Point { x: 0.0, y: 0.0 },
            },
        }
    }

    /// A point of the frame in the crop's coordinates.
    pub(crate) fn point_to_crop(&self, point: Point) -> Point {
        Point {
            x: point.x - self.origin.x,
            y: point.y - self.origin.y,
        }
    }

    /// The keypoints of a pose, from the crop to the frame; the score does not change.
    pub(crate) fn keypoints_to_frame(&self, keypoints: Vec<Keypoint>) -> Vec<Keypoint> {
        keypoints
            .into_iter()
            .map(|k| Keypoint {
                x: k.x + self.origin.x,
                y: k.y + self.origin.y,
                score: k.score,
            })
            .collect()
    }

    pub(crate) fn to_frame(&self, detections: Vec<Detection>) -> Vec<Detection> {
        detections
            .into_iter()
            .map(|mut detection| {
                detection.bbox.x0 += self.origin.x;
                detection.bbox.x1 += self.origin.x;
                detection.bbox.y0 += self.origin.y;
                detection.bbox.y1 += self.origin.y;
                detection
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene_core::{geom::Rect, look::Label};
    use std::borrow::Cow;

    #[test]
    fn whole_frame_is_borrowed() {
        let frame = image::DynamicImage::new_rgb8(320, 240);
        let crop = Crop::of(&frame, None);
        assert!(matches!(crop.image, Cow::Borrowed(image) if std::ptr::eq(image, &frame)));
        let keypoint = Keypoint {
            x: 10.0,
            y: 10.0,
            score: 0.5,
        };
        assert_eq!(crop.keypoints_to_frame(vec![keypoint]), [keypoint]);
    }

    #[test]
    fn crop_maps_boxes_to_frame() {
        let frame = image::DynamicImage::new_rgb8(320, 240);
        let crop = Crop::of(&frame, Some(Rect::new(100.0, 50.0, 200.0, 150.0)));
        assert_eq!((crop.image.width(), crop.image.height()), (100, 100));
        let boxes = crop.to_frame(vec![Detection {
            bbox: Rect::new(10.0, 10.0, 30.0, 50.0),
            confidence: 0.9,
            label: Label::Other,
        }]);
        assert_eq!(boxes[0].bbox, Rect::new(110.0, 60.0, 130.0, 100.0));
        assert_eq!(boxes[0].label, Label::Other);
        assert_eq!(boxes[0].confidence, 0.9);
    }
}
