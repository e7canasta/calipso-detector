//! Snapshot of the contract schema (rule 13): a change in the types without
//! updating the snapshot fails. To update it, on purpose:
//! `CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-core --test contract_schema`.

// Tests are shell: they read and write files. Rule 8 governs the core
// library, not its tests.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use scene_contract::{Envelope, Event, Scene};
use std::path::PathBuf;

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
fn scene_v4() {
    snapshot("calipso.scene.v4", schemars::schema_for!(Envelope<Scene>));
}

#[test]
fn events_v4() {
    snapshot("calipso.events.v4", schemars::schema_for!(Envelope<Event>));
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

/// A v4 transition, as is: it reads and writes back the same.
#[test]
fn the_v4_transition_is_the_specs() {
    use scene_contract::Event;
    let at = |sequence: u64, secs: f64| serde_json::json!({ "session": 1, "sequence": sequence, "source_secs": secs });
    let json = serde_json::json!({
        "kind": "transition",
        "at": at(812, 2034.0),
        "cell": "210:00:111",
        "phase": "running",
        "before": { "rbh": [2, 1, 0], "mode": "accompanied", "bed": "seen", "unexplained": 0,
                    "since": at(640, 1862.0), "lasted_secs": 172.0 },
        "seen": { "fv": [0, 0], "span": { "first": at(810, 2032.0), "secs": 2.0 },
                  "context": { "far": 0, "passing": 0 } },
        "after": { "rbh": [1, 1, 1], "mode": "alone_in_bed_held", "bed": "held", "unexplained": 0 },
        "facts": [
            { "type": "held_in_bed", "last_seen": at(809, 2031.0), "cause": "stabilized",
              "after": { "rbh": [2, 1, 1], "mode": "accompanied", "bed": "held", "unexplained": 0 } },
            { "type": "left_room", "cause": "stabilized",
              "after": { "rbh": [1, 1, 1], "mode": "alone_in_bed_held", "bed": "held", "unexplained": 0 } }
        ]
    });
    let event: Event = serde_json::from_value(json.clone()).unwrap();
    let Event::Transition(transition) = &event else {
        panic!("not a transition: {event:?}");
    };
    assert_eq!(
        transition.cell,
        scene_core::cell_of(
            transition.before.occupancy.rbh,
            transition.seen.fv,
            transition.after.rbh
        )
    );
    assert_eq!(serde_json::to_value(&event).unwrap(), json);
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

/// The guardian of v5 (rule 13, C5): every type v4 and v5 share keeps every
/// property, in the scene and in the events, except the ones the five changes
/// take out on purpose. A property that v5 loses without being in this list fails
/// the test: that is how a removed field (as `map.shadow` was) cannot pass in
/// silence.
#[test]
fn v5_keeps_every_property_of_v4_except_what_it_changes_on_purpose() {
    // (type, property) that v5 takes out on purpose (ADR "Lo que espera el contrato v5").
    const CHANGED_ON_PURPOSE: &[(&str, &str)] = &[
        // change 3: the subject has no verdict.
        ("Subject", "verdict"),
        // change 4: the depth is relative.
        ("SubjectDepth", "meters"),
    ];
    // Types that v5 takes out or renames on purpose: the verdict and its state
    // (change 3), and the occupancy's rbh and the seen's fv, which v4 writes as
    // tuples and v5 names (change 5).
    const TYPES_CHANGED_ON_PURPOSE: &[&str] = &["Head", "Verdict", "VerdictState", "Rbh", "Fv"];

    let mut lost = Vec::new();
    for snapshot in ["scene", "events"] {
        let v4 = schema_defs(&format!("calipso.{snapshot}.v4"));
        let v5 = schema_defs(&format!("calipso.{snapshot}.v5"));
        for (name, def) in &v4 {
            if TYPES_CHANGED_ON_PURPOSE.contains(&name.as_str()) {
                continue;
            }
            let Some(other) = v5.get(name) else {
                lost.push(format!("{snapshot}: type {name} is gone from v5"));
                continue;
            };
            for prop in properties(def) {
                if !properties(other).contains(&prop)
                    && !CHANGED_ON_PURPOSE.contains(&(name.as_str(), prop.as_str()))
                {
                    lost.push(format!("{snapshot}: {name}.{prop} is lost in v5"));
                }
            }
        }
    }
    assert!(
        lost.is_empty(),
        "v5 loses properties of v4 that are not in the list of changes on purpose: {lost:?}"
    );
}

/// The `$defs` of a snapshot, as name → definition.
fn schema_defs(name: &str) -> std::collections::BTreeMap<String, serde_json::Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contract/schema")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path).expect("the snapshot is readable");
    let json: serde_json::Value = serde_json::from_str(&text).expect("the snapshot is JSON");
    json["$defs"]
        .as_object()
        .expect("a scene schema has $defs")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// The property names of a definition.
fn properties(def: &serde_json::Value) -> Vec<String> {
    def["properties"]
        .as_object()
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default()
}
