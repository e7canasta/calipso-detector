//! The fake senses (R7-1c): they answer each probe with what the scene says
//! that sense sees, cropped to the requested region, like the real sense.
#![allow(clippy::disallowed_methods)]

use golden::language::{Expected, Faces, InBed, Location, Samples, Span};
use golden::world::{self, BED, HEADBOARD, Sample};
use scene_core::geom::Rect;
use scene_core::look::{FaceRegion, Failure, Probe, Reading};

/// A sample with the resident in bed, someone at the door, reinforcement
/// seeing the resident and a face.
fn sample(faces: Faces) -> Sample {
    let span = Span {
        samples: Samples { from: 1, to: 1 },
        floor: 1,
        location: Location::Door,
        bed: InBed::Fixed(1),
        reinforcement: 1,
        faces,
        expected: Expected::default(),
    };
    world::samples(&[span], 1.0).remove(0)
}

fn boxes(r: Result<Reading, Failure>) -> Vec<Rect> {
    match r {
        Ok(Reading::Bodies(d) | Reading::Faces(d)) => d.iter().map(|d| d.bbox).collect(),
        other => panic!("boxes were expected: {other:?}"),
    }
}

#[test]
fn the_main_detector_over_the_whole_frame_sees_everything() {
    let m = sample(Faces::None);
    let seen = boxes(world::respond(&m, &Probe::Detect { region: None }));
    assert_eq!(seen, m.detect.iter().map(|d| d.bbox).collect::<Vec<_>>());
}

#[test]
fn reinforcement_over_the_grown_bed_sees_its_own_and_nothing_at_the_door() {
    let m = sample(Faces::None);
    let seen = boxes(world::respond(
        &m,
        &Probe::Reinforce {
            region: BED.expand(0.1),
        },
    ));
    assert_eq!(seen, vec![world::in_bed(0).bbox]);
}

#[test]
fn a_sense_only_sees_what_falls_in_its_region() {
    let m = sample(Faces::None);
    let far = Rect::new(0.0, 0.0, 90.0, 90.0);
    assert!(boxes(world::respond(&m, &Probe::Detect { region: Some(far) })).is_empty());
    assert!(boxes(world::respond(&m, &Probe::Reinforce { region: far })).is_empty());
}

#[test]
fn headboard_faces_are_not_seen_when_looking_at_the_body() {
    let m = sample(Faces::AtHeadboard);
    let at_headboard = Probe::Faces {
        region: FaceRegion::Headboard(HEADBOARD),
    };
    let on_body = Probe::Faces {
        region: FaceRegion::Subject(world::in_bed(0).bbox),
    };
    assert_eq!(boxes(world::respond(&m, &at_headboard)).len(), 1);
    assert!(boxes(world::respond(&m, &on_body)).is_empty());
    let m = sample(Faces::OnSubject);
    assert_eq!(boxes(world::respond(&m, &on_body)).len(), 1);
}

#[test]
fn a_region_without_pixels_fails_like_the_real_sense() {
    let m = sample(Faces::None);
    let outside_the_frame = Rect::new(700.0, 500.0, 800.0, 600.0);
    assert_eq!(
        world::respond(
            &m,
            &Probe::Reinforce {
                region: outside_the_frame
            }
        ),
        Err(Failure::Model)
    );
}

#[test]
fn what_the_scene_does_not_model_is_not_seen() {
    let m = sample(Faces::None);
    let pose = Probe::Pose {
        bbox: world::in_bed(0).bbox,
    };
    assert_eq!(world::respond(&m, &pose), Err(Failure::Model));
}
