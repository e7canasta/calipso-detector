use crate::support;
use scene_core::contract::{BedState, Envelope, Event, FactBody, Mode};

/// The layer-0 facts of some events, in order.
fn facts(events: &[Event]) -> Vec<FactBody> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Transition(t) => Some(t.facts.iter().map(|f| f.body)),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn s1_end_to_end_matches_the_approved_events() {
    let mut driver = support::driver();
    let mut events = Vec::new();
    let mut scene = None;
    for n in 1..=25 {
        let output = driver.arrive(support::sample(n, 1)).pop().unwrap().output;
        events.extend(output.events);
        scene = Some(output.scene);
    }
    let golden: Vec<Event> = include_str!("../../../../contract/examples/s1.events.v4.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Envelope<Event>>(l).unwrap().body)
        .collect();
    assert_eq!(events, golden);
    let facts = facts(&events);
    assert!(
        facts
            .iter()
            .any(|f| matches!(f, FactBody::HeldInBed { .. }))
    );
    assert!(!facts.contains(&FactBody::LeftBed));
    let scene = scene.unwrap();
    assert_eq!(scene.bed.state, BedState::Held);
    assert_eq!(scene.room.mode, Mode::AloneInBedHeld);
    assert_eq!(scene.room.occupants, 1);
}

#[test]
fn a_reconnection_in_s1_only_reports_the_sensor_and_keeps_the_held() {
    use ingest::{IngestError, StreamEvent};
    use scene_core::contract::SensorState;
    use std::time::Instant;
    let mut driver = support::driver();
    driver.on_stream(StreamEvent::Streaming { session: 1 }, Instant::now());
    let mut previous = None;
    for n in 1..=15 {
        previous = Some(
            driver
                .arrive(support::sample(n, 1))
                .pop()
                .unwrap()
                .output
                .scene,
        );
    }
    let previous = previous.unwrap();
    assert_eq!(previous.bed.state, BedState::Held);
    for event in [
        StreamEvent::Disconnected {
            session: 1,
            reason: IngestError::Protocol("test".into()),
        },
        StreamEvent::Connecting,
        StreamEvent::Streaming { session: 2 },
    ] {
        let output = driver.on_stream(event, Instant::now()).unwrap().output;
        assert_eq!(output.scene.bed, previous.bed);
        assert_eq!(output.scene.room, previous.room);
        assert!(output.events.iter().all(|e| matches!(e, Event::Sensor(_))));
        assert_eq!(output.events.len(), 1);
        assert!(output.scene.republished.is_some());
    }
    let first = driver.arrive(support::sample(16, 2)).pop().unwrap().output;
    assert!(
        first.events.is_empty(),
        "the first sample after a reconnection does not stabilize a new occupancy"
    );
    assert_eq!(first.scene.sensor, SensorState::Streaming);
    assert_eq!(first.scene.bed.held, previous.bed.held);
    let mut last = first;
    for n in 17..=25 {
        last = driver.arrive(support::sample(n, 2)).pop().unwrap().output;
    }
    assert_eq!(last.scene.bed.state, BedState::Held);
    assert_eq!(last.scene.room.occupants, 1);
    assert_eq!(last.scene.bed.held, previous.bed.held);
}
