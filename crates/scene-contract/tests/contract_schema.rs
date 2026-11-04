//! The wire tested from its own crate (rule 13): the snapshot of the schema
//! against `contract/schema/*.json`, the round-trip of a v5 transition and the
//! published rejections. The cell a transition claims is the core's
//! computation (`scene-core/tests/contract_cells.rs`); here it is the value
//! written by hand, so the wire depends on no core, not even in tests.
//! To update the snapshot, on purpose:
//! `CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-contract --test contract_schema`.

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
         If the change is on purpose: CALIPSO_UPDATE_SCHEMA=1 cargo test -p scene-contract --test contract_schema"
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
/// same. Its cell is the one the core computes, checked where that computation
/// lives (`scene-core/tests/contract_cells.rs`).
#[test]
fn the_v5_transition_is_the_specs() {
    let line = include_str!("../../../contract/examples/s1.events.v5.jsonl")
        .lines()
        .find(|l| l.contains("\"kind\":\"transition\""))
        .expect("the examples have a transition");
    let envelope: scene_contract::v5::Envelope<Event> =
        serde_json::from_str(line).expect("a v5 event");
    let Event::Transition(_transition) = &envelope.body else {
        panic!("not a transition: {:?}", envelope.body);
    };
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(serde_json::to_value(&envelope).unwrap(), json);
}

/// The same transition written by hand, not read from the examples: a path of
/// its own, so the cell is checked against a value nobody computed. The core
/// checks its computation against this same transition in
/// `scene-core/tests/contract_cells.rs`.
#[test]
fn the_v5_transition_written_by_hand_is_the_specs() {
    let at = |sequence: u64, secs: f64| serde_json::json!({ "session": 1, "sequence": sequence, "source_secs": secs });
    let json = serde_json::json!({
        "kind": "transition",
        "at": at(812, 2034.0),
        "cell": "210:00:111",
        "phase": "running",
        "before": { "rbh": { "room": 2, "bed": 1, "held": 0 }, "mode": "accompanied", "bed": "seen",
                    "unexplained": 0, "since": at(640, 1862.0), "lasted_secs": 172.0 },
        "seen": { "fv": { "floor": 0, "bed": 0 }, "span": { "first": at(810, 2032.0), "secs": 2.0 },
                  "context": { "far": 0, "passing": 0 } },
        "after": { "rbh": { "room": 1, "bed": 1, "held": 1 }, "mode": "alone_in_bed_held", "bed": "held",
                   "unexplained": 0 },
        "facts": [
            { "type": "held_in_bed", "last_seen": at(809, 2031.0), "cause": "stabilized",
              "after": { "rbh": { "room": 2, "bed": 1, "held": 1 }, "mode": "accompanied", "bed": "held",
                         "unexplained": 0 } },
            { "type": "left_room", "cause": "stabilized",
              "after": { "rbh": { "room": 1, "bed": 1, "held": 1 }, "mode": "alone_in_bed_held", "bed": "held",
                         "unexplained": 0 } }
        ]
    });
    let event: Event = serde_json::from_value(json.clone()).unwrap();
    let Event::Transition(transition) = &event else {
        panic!("not a transition: {event:?}");
    };
    assert_eq!(transition.cell.as_str(), "210:00:111");
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

/// `as_str()` is what writes a cell with, and the wire publishes each variant
/// by serde's name: two sources of truth for one name. This fails when a
/// `rename_all` changes without `as_str` (or the other way round): the name
/// serde produces is taken from the variant itself, never written twice.
#[test]
fn as_str_matches_the_wire() {
    use scene_contract::{BedPostureSeen, BedPostureState, LookKind};

    fn wire_name<T: serde::Serialize>(variant: T) -> String {
        let json = serde_json::to_value(variant).expect("the variant serializes");
        json.as_str()
            .expect("a unit variant publishes as its name alone")
            .to_owned()
    }

    let looks = [
        LookKind::Detect,
        LookKind::Faces,
        LookKind::Reinforce,
        LookKind::Pose,
        LookKind::PoseBody,
        LookKind::Depth,
        LookKind::Calibrate,
        LookKind::Verify,
        LookKind::Keep,
    ];
    for kind in looks {
        assert_eq!(
            kind.as_str(),
            wire_name(kind),
            "LookKind writes cells with a name that is not its wire name"
        );
    }

    let states = [
        BedPostureState::InBed,
        BedPostureState::Lying,
        BedPostureState::SittingUp,
        BedPostureState::GettingOut,
    ];
    for state in states {
        assert_eq!(
            state.as_str(),
            wire_name(state),
            "BedPostureState writes cells with a name that is not its wire name"
        );
    }

    let sightings = [
        BedPostureSeen::Held,
        BedPostureSeen::HeadAway,
        BedPostureSeen::HeadSide,
        BedPostureSeen::LegsOut,
        BedPostureSeen::HeadboardLying,
        BedPostureSeen::HeadboardUpright,
        BedPostureSeen::Unknown,
    ];
    for seen in sightings {
        assert_eq!(
            seen.as_str(),
            wire_name(seen),
            "BedPostureSeen writes cells with a name that is not its wire name"
        );
    }
}
