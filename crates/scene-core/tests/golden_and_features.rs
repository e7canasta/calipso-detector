//! The golden scenes S1–S8 and their scenarios in the features say the same.
//! The table is written twice (the feature is the specification, the
//! scene is the data that runs all the way to Rerun): if one changes without
//! the other, this turns red.
#![allow(clippy::disallowed_methods)]

// The three binaries that compile this module (here, contract_examples and the
// BDD harness) use different subsets of it, so some items look dead in each.
#[allow(dead_code)]
mod script;

use golden::language::Scene;

const GOLDEN: [(u32, &str); 8] = [
    (1, "s1-staff-put-to-bed"),
    (2, "s2-sleeps-covered"),
    (3, "s3-gets-up-alone"),
    (4, "s4-comes-back-and-covers-up"),
    (5, "s5-they-come-for-them"),
    (6, "s6-taken-away"),
    (7, "s7-enters-while-covered"),
    (8, "s8-the-carer-covers-them"),
];

fn scene(name: &str) -> Scene {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../calipso-golden/scenes")
        .join(format!("{name}.md"));
    Scene::parse(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn each_golden_scene_sees_the_same_as_its_scenario_sample_by_sample() {
    for (n, name) in GOLDEN {
        let golden = scene(name).samples();
        let feature = script::episode(n);
        assert_eq!(golden.len(), feature.len(), "{name}: number of samples");
        for (m, (k, s)) in golden.iter().zip(&feature) {
            assert_eq!(m.n, *k, "{name}");
            assert_eq!(
                Ok(m.detect.clone()),
                s.detect,
                "{name}, sample {k}: main detector"
            );
            assert_eq!(
                Ok(m.reinforce.clone()),
                s.reinforce,
                "{name}, sample {k}: reinforcement"
            );
            assert_eq!(Ok(m.faces.clone()), s.faces, "{name}, sample {k}: faces");
        }
    }
}

#[test]
fn each_scenarios_expectations_are_in_its_golden_scene() {
    for (n, name) in GOLDEN {
        let golden = scene(name);
        let (_, table) = script::episode_scenario(n);
        for span in table {
            let in_golden = golden
                .spans
                .iter()
                .find(|s| s.samples == span.samples)
                .unwrap_or_else(|| panic!("{name}: span {:?} is missing", span.samples));
            assert_eq!(
                in_golden.expected, span.expected,
                "{name}, {:?}",
                span.samples
            );
        }
    }
}
