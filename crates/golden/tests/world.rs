//! The world of a scene: what each sense sees in each sample. It must be
//! exactly what the BDD harness used to script (R7-1b).
#![allow(clippy::disallowed_methods)]

use golden::language::{Location, Scene};
use golden::world::{in_bed, on_floor};

const S1: &str = include_str!("../../../golden/s1-staff-put-to-bed.md");

/// Up to R7-1d it was compared with the BDD harness sample by sample; now the
/// harness uses this world, and the 56 scenarios and the S1–S8 examples are the
/// equivalence proof. What S1 says stays here, written from outside.
#[test]
fn s1_sees_what_its_table_says() {
    let samples = Scene::parse(S1).unwrap().samples();
    assert_eq!(samples.len(), 25);
    let n = |n: u32| &samples[usize::try_from(n - 1).unwrap()];
    assert_eq!(
        n(1).detect,
        vec![on_floor(0, Location::Door), on_floor(1, Location::Door)]
    );
    assert_eq!(n(6).detect, vec![in_bed(0), on_floor(0, Location::NearBed)]);
    assert_eq!(n(11).detect, vec![on_floor(0, Location::NearBed)]);
    assert!(n(25).detect.is_empty());
    assert!(
        samples
            .iter()
            .all(|m| m.reinforce.is_empty() && m.faces.is_empty())
    );
}

#[test]
fn source_time_comes_from_the_cadence() {
    let scene = Scene::parse(&S1.replace("cadence_s: 1", "cadence_s: 2.5")).unwrap();
    let samples = scene.samples();
    assert_eq!(samples[0].source_secs, 2.5);
    assert_eq!(samples[9].source_secs, 25.0);
}
