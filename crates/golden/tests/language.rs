//! The scene language: a golden scene and a Gherkin table read the same, and
//! an error says which line it is on.
#![allow(clippy::disallowed_methods)]

use golden::language::{
    Expected, ExpectedBed, Faces, InBed, Location, Samples, Scene, Span, spans,
};

const S1: &str = include_str!("../../../golden/s1-staff-put-to-bed.md");
const FEATURE_203: &str = include_str!("../../../features/r4/203-accompanied/accompanied.feature");

/// The table rows of a scenario of a feature.
fn scenario_rows<'a>(feature: &'a str, name: &str) -> Vec<&'a str> {
    let (_, rest) = feature
        .split_once(&format!("Scenario: {name}\n"))
        .expect("the scenario exists");
    let body = rest.split("Scenario:").next().unwrap_or_default();
    body.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|'))
        .collect()
}

#[test]
fn s1_written_as_a_scene_is_the_same_table_as_in_the_feature() {
    let scene = Scene::parse(S1).unwrap();
    assert_eq!(scene.format, 1);
    assert_eq!(
        scene.title,
        "S1 — The staff put the resident to bed and leave"
    );
    assert_eq!(scene.cadence_s, 1.0);
    assert_eq!(scene.dwell_s, Some(2.0));
    let feature = spans(&scenario_rows(
        FEATURE_203,
        "The staff put the resident to bed and leave",
    ))
    .unwrap();
    assert_eq!(scene.spans.len(), 4);
    assert_eq!(scene.spans, feature);
    // What row 6-10 says, read from outside the reader.
    assert_eq!(
        scene.spans[1],
        Span {
            samples: Samples { from: 6, to: 10 },
            floor: 1,
            location: Location::NearBed,
            bed: InBed::Fixed(1),
            reinforcement: 0,
            faces: Faces::None,
            expected: Expected {
                occupants: Some(2),
                bed: Some(ExpectedBed::Seen),
            },
        }
    );
}

#[test]
fn without_format_it_is_not_read() {
    let text = S1.replace("format: 1\n", "");
    let error = Scene::parse(&text).unwrap_err();
    assert!(error.to_string().contains("format"), "{error}");
}

#[test]
fn an_unknown_format_is_not_read() {
    let error = Scene::parse(&S1.replace("format: 1", "format: 2")).unwrap_err();
    assert!(error.to_string().contains("format"), "{error}");
}

#[test]
fn an_unknown_column_says_its_line() {
    let text = S1.replace(
        "| bed            |\n|---",
        "| bed            | colour |\n|---",
    );
    let error = Scene::parse(&text).unwrap_err();
    assert!(error.to_string().contains("colour"), "{error}");
    assert!(error.to_string().contains("line 12"), "{error}");
}

#[test]
fn a_scenes_samples_are_contiguous_from_the_first() {
    let text = S1.replace("| 11-15 ", "| 12-15 ");
    let error = Scene::parse(&text).unwrap_err();
    assert!(error.to_string().contains("12"), "{error}");
}

#[test]
fn a_scene_says_which_senses_it_assumes() {
    use golden::language::Sense;
    assert_eq!(Scene::parse(S1).unwrap().senses, vec![Sense::Detect]);
    let r6 = S1.replace("dwell_s: 2\n", "dwell_s: 2\nsenses: detect, faces\n");
    assert_eq!(
        Scene::parse(&r6).unwrap().senses,
        vec![Sense::Detect, Sense::Faces]
    );
    let without_main = S1.replace("dwell_s: 2\n", "dwell_s: 2\nsenses: faces\n");
    assert!(Scene::parse(&without_main).is_err());
}

#[test]
fn every_golden_scene_written_and_read_back_is_the_same() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "md") || path.ends_with("README.md") {
            continue;
        }
        let scene = Scene::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let written = golden::language::write(&scene, "A description.");
        assert_eq!(
            Scene::parse(&written).unwrap(),
            scene,
            "{}\n{written}",
            path.display()
        );
    }
}

#[test]
fn the_floor_can_mix_the_door_and_near_bed() {
    let text = S1.replace(
        "| 1-5     | 2        | door     |",
        "| 1-5     | 2        | 1 near bed |",
    );
    let scene = Scene::parse(&text).unwrap();
    assert_eq!(scene.spans[0].location, Location::Mixed { near: 1 });
    let first = &scene.samples()[0];
    assert_eq!(
        first.detect,
        vec![
            golden::world::on_floor(0, Location::NearBed),
            golden::world::on_floor(1, Location::Door)
        ]
    );
    let all = S1.replace(
        "| 1-5     | 2        | door     |",
        "| 1-5     | 2        | 2 near bed |",
    );
    let error = Scene::parse(&all).unwrap_err();
    assert!(error.to_string().contains("use \"door\""), "{error}");
}
