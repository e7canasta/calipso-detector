//! Snapshot of the contract schema (rule 13): a change in the types without
//! updating the snapshot fails. To update it, on purpose:
//! `CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-core --test contract_schema`.

// Tests are shell: they read and write files. Rule 8 governs the core
// library, not its tests.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::path::PathBuf;

use scene_contract::v5::Event;

fn snapshot(name: &str, schema: schemars::Schema) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contract/schema")
        .join(format!("{name}.json"));
    let actual = serde_json::to_string_pretty(&schema).unwrap() + "\n";
    if std::env::var_os("CALIPSO_UPDATE_SCHEMA").is_some() {
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        actual == expected,
        "rule 13: the schema of {name} changed and the snapshot did not.\n\
         If the change is on purpose: CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-core --test contract_schema"
    );
}

#[test]
fn events_v5() {
    snapshot(
        "calipso.events.v5",
        schemars::schema_for!(scene_contract::v5::Envelope<scene_contract::v5::Event>),
    );
}

#[test]
fn scene_v5() {
    snapshot(
        "calipso.scene.v5",
        schemars::schema_for!(scene_contract::v5::Envelope<scene_contract::v5::Scene>),
    );
}

/// A v5 transition, as the examples publish it: it reads and writes back the
/// same, and its cell is the one the core computes.
#[test]
fn the_v5_transition_is_the_specs() {
    let line = include_str!("../../../contract/examples/s1.events.v5.jsonl")
        .lines()
        .find(|l| l.contains("\"kind\":\"transition\""))
        .expect("the examples have a transition");
    let envelope: scene_contract::v5::Envelope<Event> =
        serde_json::from_str(line).expect("a v5 event");
    let Event::Transition(transition) = &envelope.body else {
        panic!("not a transition: {:?}", envelope.body);
    };
    assert_eq!(
        transition.cell,
        scene_core::cell_of(
            transition.before.occupancy.rbh,
            transition.seen.fv,
            transition.after.rbh,
        )
    );
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(serde_json::to_value(&envelope).unwrap(), json);
}

#[test]
fn rejections_are_published_grouped_by_reason() {
    use scene_contract::{LookResult, RejectReason};
    let result = LookResult::Read {
        candidates: 4,
        rejected: vec![
            RejectReason::LowConfidence,
            RejectReason::LowConfidence,
            RejectReason::NotPerson,
        ],
    };
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "type": "read", "candidates": 4,
        "rejected": [{ "reason": "low_confidence", "count": 2 }, { "reason": "not_person", "count": 1 }] })
    );
    assert_eq!(serde_json::from_value::<LookResult>(json).unwrap(), result);
}
