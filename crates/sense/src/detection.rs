use scene_geom::Rect;
use scene_look::{Detection, Label};

/// Translates backend coordinates and labels, with no acceptance policy.
pub(crate) fn from_backend(xyxy: [f32; 4], confidence: f32, class: Option<&str>) -> Detection {
    let [x0, y0, x1, y1] = xyxy;
    Detection {
        bbox: Rect::new(x0, y0, x1, y1),
        confidence,
        label: if class == Some("person") {
            Label::Person
        } else {
            Label::Other
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene_geom::Rect;
    use scene_look::Label;

    #[test]
    fn chair_and_unknown_classes_are_kept_with_their_boxes_and_confidence() {
        for name in [Some("chair"), None] {
            let detection = from_backend([10.0, 20.0, 50.0, 70.0], 0.1, name);
            assert_eq!(detection.label, Label::Other);
            assert_eq!(detection.bbox, Rect::new(10.0, 20.0, 50.0, 70.0));
            assert_eq!(detection.confidence, 0.1);
        }
        assert_eq!(
            from_backend([0.0; 4], 0.9, Some("person")).label,
            Label::Person
        );
    }
}
