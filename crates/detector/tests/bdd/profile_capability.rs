//! Profile steps by capability (`features/architecture/profile-by-capability.feature`):
//! a wrong section is reported by its own name, and a profile with every
//! capability keeps the hash it had before the reorganization.

use crate::world::Shell;
use cucumber::{given, then, when};
use detector::Config;
use std::path::Path;

/// The hash the test profile has: pinned here, as a constant, from its
/// `--check` (the command of the repository's profiles). The profile is a
/// test fixture with generic values, not a site's profile, so the scenario
/// pins its identity and not a value read from the same run.
fn recorded_hash(path: &str) -> &'static str {
    match path {
        "crates/detector/tests/support/every-capability.toml" => "sha256:a1526f30b1c77795",
        other => panic!("no hash pinned for {other}"),
    }
}

/// The capabilities a profile leaves out, by their name.
fn missing_capabilities(config: &Config) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if config.senses.reinforce.is_none() {
        missing.push("reinforce");
    }
    if config.senses.faces.is_none() {
        missing.push("faces");
    }
    if config.senses.pose.is_none() {
        missing.push("pose");
    }
    if config.senses.depth.is_none() {
        missing.push("depth");
    }
    if config.calibration.is_none() {
        missing.push("calibration");
    }
    if config.bed_posture.is_none() {
        missing.push("bed_posture");
    }
    if config.verify.is_none() || config.senses.verify.is_none() {
        missing.push("verify");
    }
    if config.senses.detect.focus.is_none() {
        missing.push("focus");
    }
    if config.shadow.is_none() {
        missing.push("shadow");
    }
    if config.occupancy.is_none() {
        missing.push("occupancy");
    }
    missing
}

#[given(regex = r#"^a profile with "\[faces\]" and a minimum confidence of (-?[\d.]+)$"#)]
fn faces_with_minimum(w: &mut Shell, minimum: String) {
    w.parse_config(&format!(
        "\n[senses.faces]\nmodel = \"face/yolov12l-face.onnx\"\nmin_confidence = {minimum}\n"
    ));
}

/// The profile was parsed by the Given: checking it is what the parse did.
#[when("the profile is checked")]
fn the_profile_is_checked(w: &mut Shell) {
    assert!(w.config.is_some(), "there is a profile to check");
}

#[then(expr = "the error names the section {string}")]
fn the_error_names_the_section(w: &mut Shell, section: String) {
    let error = w
        .config
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .expect("the profile is rejected");
    assert!(error.contains(&section), "{error}");
}

#[then(expr = "the error names the field {string}")]
fn the_error_names_the_field(w: &mut Shell, field: String) {
    let error = w
        .config
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .expect("the profile is rejected");
    assert!(error.contains(&field), "{error}");
}

#[given(expr = "the profile {string} with every capability enabled")]
fn the_profile_with_every_capability(w: &mut Shell, path: String) {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(&path);
    let config = Config::load(&full).unwrap_or_else(|e| panic!("{path}: {e:#}"));
    let missing = missing_capabilities(&config);
    assert!(missing.is_empty(), "{path} does not enable: {missing:?}");
    w.config = Some(Ok(config));
    w.loaded_from = Some(path);
}

#[then("it loads")]
fn it_loads(w: &mut Shell) {
    assert!(matches!(w.config, Some(Ok(_))), "the profile loads");
}

#[then("its hash is the one recorded before the reorganization")]
fn its_hash_is_recorded(w: &mut Shell) {
    let path = w.loaded_from.clone().expect("a profile was loaded");
    let hash = w.config().profile().expect("the profile builds").hash.0;
    assert_eq!(hash, recorded_hash(&path), "{path}");
}
