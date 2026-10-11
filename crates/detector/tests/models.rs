#[path = "support/models.rs"]
mod models;
use image::DynamicImage;
use models::Model;
use scene_core::{
    geom::Rect,
    look::{Detection, Label},
};
use sense::DetectorPort;

#[test]
fn role_models_keep_local_boxes_failures_and_call_records_independent() {
    let principal = Model::default();
    let reinforce = Model::default();
    let boxes = vec![Detection {
        bbox: Rect::new(10.0, 20.0, 110.0, 220.0),
        confidence: 0.1,
        label: Label::Other,
    }];
    reinforce.sees(boxes.clone());
    reinforce.enqueue(Err("primera falla".into()));
    reinforce.at(11);
    let mut port = reinforce.clone();
    let frame = DynamicImage::new_rgb8(480, 360);
    assert!(port.detect(&frame).is_err());
    assert_eq!(port.detect(&frame).unwrap(), boxes);
    assert_eq!(reinforce.calls().len(), 2);
    assert_eq!(reinforce.calls()[0].size, (480, 360));
    assert_eq!(reinforce.calls()[0].sample, 11);
    assert!(principal.calls().is_empty());
    reinforce.fails();
    assert!(port.detect(&frame).is_err());
}
