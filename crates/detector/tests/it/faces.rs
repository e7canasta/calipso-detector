use crate::{models, support};
use detector::clock::FixedClock;
use detector::driver::Driver;
use models::Model;
use scene_core::units::Confidence;
use scene_core::{
    geom::{Quad, Rect},
    look::{Detection, Label},
    map::SiteMap,
    policy::{FacePolicy, Policy},
};
use std::time::Duration;

#[test]
fn faces_on_current_resident_are_published_in_frame_coordinates_on_subject() {
    let principal = Model::default();
    principal.sees(vec![Detection {
        bbox: Rect::new(30.0, 30.0, 50.0, 60.0),
        confidence: 0.9,
        label: Label::Person,
    }]);
    let faces = Model::default();
    faces.sees(vec![Detection {
        bbox: Rect::new(2.0, 2.0, 8.0, 8.0),
        confidence: 0.9,
        label: Label::Other,
    }]);
    let policy = Policy {
        faces: Some(FacePolicy {
            min_confidence: Confidence::new(0.3).expect("a score"),
            headboard_pad: 0.0,
        }),
        ..Policy::default()
    };
    let map = SiteMap::new(Quad::from_rect(Rect::new(20.0, 20.0, 100.0, 80.0)));
    let mut driver = Driver::new(
        policy,
        map,
        sense::Senses::new(principal).with_faces(faces.clone()),
        Duration::from_secs(10),
        FixedClock::new(),
    );
    for n in 1..=5 {
        faces.at(n);
        let output = driver.arrive(support::sample(n, 1)).pop().unwrap().output;
        let items = &output.scene.evidence.faces.items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].bbox, Rect::new(32.0, 32.0, 38.0, 38.0));
        if n >= 3 {
            assert!(items[0].on_subject);
        }
    }
    assert_eq!(faces.calls().len(), 5);
    assert!(faces.calls().iter().all(|c| c.size == (20, 30)));
}
