//! Reception steps (`features/architecture/reception.feature`): the reception
//! the shell reports reaches the published scene under the field `sensor`, and
//! the contract schema is still the snapshot it was before the rename.

use crate::script::Script;
use crate::ward::Ward;
use cucumber::{given, then, when};
use scene_contract::{Envelope, Scene, SensorState};
use scene_core::Input;
use serde_json::Value;
use std::path::PathBuf;

/// A stream that is delivering samples, in a room with the bed delimited. The
/// stall timeout is the shell's (`StreamWatch`): the core only receives what
/// the shell reports, so this step does not check the timeout itself.
fn streaming(ward: &mut Ward) {
    ward.bed();
    ward.drive(
        Input::Sensor {
            session: ward.sample(1).session,
            last: Some(ward.sample(1)),
            state: SensorState::Streaming,
        },
        &Script::default(),
    );
}

#[given("a stream that reconnects")]
fn a_stream_that_reconnects(ward: &mut Ward) {
    streaming(ward);
}

#[given("a stream with no samples for longer than its stall timeout")]
fn a_stream_with_no_samples(ward: &mut Ward) {
    streaming(ward);
}

/// The Given names the change: the stream dropped, so the reception is
/// reconnecting.
#[when("the shell reports the reception change")]
fn the_shell_reports_the_change(ward: &mut Ward) {
    ward.drive(
        Input::Sensor {
            session: ward.sample(2).session,
            last: Some(ward.sample(2)),
            state: SensorState::Reconnecting,
        },
        &Script::default(),
    );
}

#[when("the shell reports the stall")]
fn the_shell_reports_the_stall(ward: &mut Ward) {
    ward.drive(
        Input::Sensor {
            session: ward.sample(2).session,
            last: Some(ward.sample(2)),
            state: SensorState::Stalled,
        },
        &Script::default(),
    );
}

#[then(expr = "the published scene carries the field {string} with the value {string}")]
fn the_published_field(ward: &mut Ward, field: String, value: String) {
    let scene = serde_json::to_value(&*ward.last().output.v5.scene).expect("the scene serializes");
    assert_eq!(
        scene[field.as_str()],
        Value::from(value),
        "the published scene: {scene}"
    );
}

#[then("the contract schema is the same as before the rename")]
fn the_contract_schema_is_the_same(_ward: &mut Ward) {
    let schema = schemars::schema_for!(Envelope<Scene>);
    let actual = serde_json::to_string_pretty(&schema).expect("the schema serializes") + "\n";
    let snapshot = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contract/schema/calipso.scene.v4.json");
    let expected = std::fs::read_to_string(&snapshot).expect("the schema snapshot is there");
    assert!(
        actual == expected,
        "rule 13: the scene schema is not the snapshot {}",
        snapshot.display()
    );
}
