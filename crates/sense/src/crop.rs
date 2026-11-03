use image::DynamicImage;
use scene_contract::Keypoint;
use scene_geom::{Point, Rect};
use scene_look::Detection;
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

/// The crop of a box turned so that a direction of the image points up (F2.4).
/// The canvas holds the whole rotated box, so no body point falls outside it;
/// the keypoints come back through the inverse rotation.
#[derive(Debug)]
pub(crate) struct Turned {
    pub(crate) image: DynamicImage,
    centre: (f32, f32),
    canvas: (f32, f32),
    cos: f32,
    sin: f32,
}

impl Turned {
    /// The rotation R(θ) with θ = −π/2 − atan2(uy, ux) carries `up` to (0, −1).
    pub(crate) fn of(frame: &DynamicImage, bbox: Rect, up: scene_look::Up) -> Self {
        let theta = -std::f32::consts::FRAC_PI_2 - up.dy().atan2(up.dx());
        // Quarter turns come out of sin_cos with float noise (cos(-π/2) ≈ -4e-8); that noise
        // would round the canvas up by a pixel and shift every keypoint by half of it.
        let snap = |v: f32| if v.abs() < 1e-6 { 0.0 } else { v };
        let (sin, cos) = theta.sin_cos();
        let (sin, cos) = (snap(sin), snap(cos));
        let centre = ((bbox.x0 + bbox.x1) / 2.0, (bbox.y0 + bbox.y1) / 2.0);
        let (half_w, half_h) = ((bbox.x1 - bbox.x0) / 2.0, (bbox.y1 - bbox.y0) / 2.0);
        let w = (cos.abs() * 2.0 * half_w + sin.abs() * 2.0 * half_h)
            .ceil()
            .max(1.0) as u32;
        let h = (sin.abs() * 2.0 * half_w + cos.abs() * 2.0 * half_h)
            .ceil()
            .max(1.0) as u32;
        let canvas = (w as f32 / 2.0, h as f32 / 2.0);
        let source = frame.to_rgb8();
        let (fw, fh) = (source.width() as f32, source.height() as f32);
        let mut image = image::RgbImage::new(w, h);
        for qy in 0..h {
            for qx in 0..w {
                let (dx, dy) = (qx as f32 - canvas.0, qy as f32 - canvas.1);
                let x = cos * dx + sin * dy + centre.0;
                let y = -sin * dx + cos * dy + centre.1;
                if x >= 0.0 && y >= 0.0 && x < fw && y < fh {
                    image.put_pixel(qx, qy, *source.get_pixel(x as u32, y as u32));
                }
            }
        }
        Self {
            image: DynamicImage::ImageRgb8(image),
            centre,
            canvas,
            cos,
            sin,
        }
    }

    /// The keypoints of a pose, from the turned canvas back to the frame; the score does not change.
    pub(crate) fn keypoints_to_frame(&self, keypoints: Vec<Keypoint>) -> Vec<Keypoint> {
        keypoints
            .into_iter()
            .map(|k| {
                let (dx, dy) = (k.x - self.canvas.0, k.y - self.canvas.1);
                Keypoint {
                    x: self.cos * dx + self.sin * dy + self.centre.0,
                    y: -self.sin * dx + self.cos * dy + self.centre.1,
                    score: k.score,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene_geom::Rect;
    use scene_look::Label;
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

    /// The forward map (frame to canvas), the inverse of `keypoints_to_frame`, to check the turn.
    fn to_canvas(turned: &Turned, p: (f32, f32)) -> (f32, f32) {
        let (a, b) = (p.0 - turned.centre.0, p.1 - turned.centre.1);
        (
            turned.cos * a - turned.sin * b + turned.canvas.0,
            turned.sin * a + turned.cos * b + turned.canvas.1,
        )
    }

    fn near(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() <= 0.5 && (a.1 - b.1).abs() <= 0.5
    }

    #[test]
    fn a_turned_direction_points_up_on_the_canvas() {
        let frame = DynamicImage::new_rgb8(400, 400);
        let bbox = Rect::new(100.0, 100.0, 200.0, 300.0);
        let centre = (150.0, 200.0);
        for (dx, dy) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (-1.0, 0.5)] {
            let up = scene_look::Up::new(dx, dy).expect("a direction");
            let turned = Turned::of(&frame, bbox, up);
            let canvas = (turned.canvas.0, turned.canvas.1);
            // 20 px along the requested direction, from the box's centre, is 20 px above the canvas centre.
            let on_frame = (centre.0 + 20.0 * up.dx(), centre.1 + 20.0 * up.dy());
            assert!(
                near(to_canvas(&turned, on_frame), (canvas.0, canvas.1 - 20.0)),
                "up {up:?}"
            );
        }
    }

    #[test]
    fn a_keypoint_comes_back_to_the_frame_where_it_was() {
        let frame = DynamicImage::new_rgb8(400, 400);
        let bbox = Rect::new(100.0, 100.0, 200.0, 300.0);
        for (dx, dy) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let turned = Turned::of(
                &frame,
                bbox,
                scene_look::Up::new(dx, dy).expect("a direction"),
            );
            let on_canvas = Keypoint {
                x: 1.0,
                y: 1.0,
                score: 0.9,
            };
            let back = turned.keypoints_to_frame(vec![on_canvas])[0];
            assert!(
                near(to_canvas(&turned, (back.x, back.y)), (1.0, 1.0)),
                "up ({dx}, {dy})"
            );
        }
    }

    #[test]
    fn the_canvas_holds_every_corner_of_the_box() {
        let frame = DynamicImage::new_rgb8(400, 400);
        let bbox = Rect::new(100.0, 100.0, 200.0, 300.0);
        let turned = Turned::of(
            &frame,
            bbox,
            scene_look::Up::new(1.0, 1.0).expect("a direction"),
        );
        let (w, h) = (turned.image.width() as f32, turned.image.height() as f32);
        for corner in [
            (100.0, 100.0),
            (200.0, 100.0),
            (200.0, 300.0),
            (100.0, 300.0),
        ] {
            let c = to_canvas(&turned, corner);
            assert!(
                c.0 >= -0.5 && c.0 <= w + 0.5 && c.1 >= -0.5 && c.1 <= h + 0.5,
                "corner {corner:?} -> {c:?}"
            );
        }
    }
}
