//! Snapshot of the contract schema (rule 13): a change in the types without
//! updating the snapshot fails. To update it, on purpose:
//! `CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-core --test contract_schema`.

// Tests are shell: they read and write files. Rule 8 governs the core
// library, not its tests.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use scene_core::contract::{Envelope, Event, Scene};
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

/// A v4 transition, as is: it reads and writes back the same.
#[test]
fn the_v4_transition_is_the_specs() {
    use scene_core::contract::v4::{Cell, Event};
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
        Cell::new(
            transition.before.occupancy.rbh,
            transition.seen.fv,
            transition.after.rbh
        )
    );
    assert_eq!(serde_json::to_value(&event).unwrap(), json);
}

#[test]
fn rejections_are_published_grouped_by_reason() {
    use scene_core::contract::{LookResult, RejectReason};
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
