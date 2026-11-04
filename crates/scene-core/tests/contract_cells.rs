//! The cell a transition claims, as `cell_of` computes it. The wire's tests
//! (`scene-contract/tests/contract_schema.rs`) build their cells by hand and
//! depend on no core; the computation is the core's, and is checked here
//! against the examples' transition and one written by hand.

use scene_contract::v5::Event;

/// The cell of the examples' transition is the one the core computes.
#[test]
fn cell_of_computes_the_v5_transition_cell() {
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
}

/// The cell of the transition written by hand in the wire's tests is the one
/// the core computes: a path of its own, against a value nobody computed.
#[test]
fn cell_of_computes_the_v5_transition_written_by_hand_cell() {
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
    let event: Event = serde_json::from_value(json).unwrap();
    let Event::Transition(transition) = &event else {
        panic!("not a transition: {event:?}");
    };
    assert_eq!(
        transition.cell,
        scene_core::cell_of(
            transition.before.occupancy.rbh,
            transition.seen.fv,
            transition.after.rbh,
        )
    );
}
