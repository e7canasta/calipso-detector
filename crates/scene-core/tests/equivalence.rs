//! The v4 → v3 equivalence (project 24, V5; spec 1301): for each turn, what v4
//! says can be brought down to what v3 says. If it holds on every turn of the
//! suites, the two versions cannot tell two different stories.
//!
//! It is called on every turn of the drivers (the BDD harness and the S1–S8
//! examples).

use scene_core::Output;
use scene_core::contract::v4;
use scene_core::contract::{After, Cause, Event, EventBody, EventKind, RoomState, SampleRef};

/// The v3 state that matches a v4 mode.
pub fn state(mode: v4::Mode) -> RoomState {
    match mode {
        v4::Mode::Empty => RoomState::Empty,
        v4::Mode::AloneOut | v4::Mode::AloneInBedSeen | v4::Mode::AloneInBedHeld => RoomState::One,
        v4::Mode::Accompanied => RoomState::Several,
    }
}

/// The v3 `after` that matches a v4 occupancy.
fn after(o: v4::Occupancy) -> After {
    let v4::Rbh(r, b, h) = o.rbh;
    After {
        room: state(o.mode),
        occupants: r,
        bed: o.bed,
        bed_seen: b - h,
        bed_held: h,
        floor_seen: r - b,
    }
}

fn fact(body: v4::FactBody) -> EventBody {
    match body {
        v4::FactBody::Entered => EventBody::Entered,
        v4::FactBody::LeftRoom => EventBody::LeftRoom,
        v4::FactBody::LayDown => EventBody::LayDown,
        v4::FactBody::LeftBed => EventBody::LeftBed,
        v4::FactBody::HeldInBed { last_seen } => EventBody::HeldInBed { last_seen },
        v4::FactBody::Reappeared { held_since } => EventBody::Reappeared { held_since },
    }
}

/// The v3 events deduced from the v4 ones, in v3's order: the domain ones (the
/// facts and the calibration), the derived ones and the sensor.
fn bring_down(events: &[v4::Event], scene: &v4::Scene) -> Vec<Event> {
    let mut v3 = Vec::new();
    for event in events {
        match event {
            v4::Event::Transition(t) => {
                for f in &t.facts {
                    v3.push(Event {
                        kind: EventKind::Domain,
                        cause: f.cause,
                        at: t.at,
                        body: fact(f.body),
                        after: after(f.after),
                    });
                }
            }
            v4::Event::Map(m) => {
                let v4::MapBody::BedCalibrated { reference_m } = m.body;
                v3.push(Event {
                    kind: EventKind::Domain,
                    cause: m.cause,
                    at: m.at,
                    body: EventBody::BedCalibrated { reference_m },
                    // The map is not occupancy: its after is the turn's.
                    after: after(published(scene)),
                });
            }
            v4::Event::Derived(d) => {
                let body = match d.body {
                    v4::DerivedBody::RoomChanged { from, to } => {
                        if state(from) == state(to) {
                            continue;
                        }
                        EventBody::RoomChanged {
                            from: state(from),
                            to: state(to),
                        }
                    }
                    v4::DerivedBody::BedChanged { from, to } => EventBody::BedChanged { from, to },
                };
                v3.push(Event {
                    kind: EventKind::Derived,
                    cause: d.cause,
                    at: d.at,
                    body,
                    after: after(d.after),
                });
            }
            v4::Event::Sensor(s) => {
                let v4::SensorBody::SensorChanged { from, to } = s.body;
                v3.push(Event {
                    kind: EventKind::Sensor,
                    cause: s.cause,
                    at: s.at,
                    body: EventBody::SensorChanged { from, to },
                    after: after(s.after),
                });
            }
        }
    }
    v3
}

/// The occupancy a v4 scene states.
fn published(scene: &v4::Scene) -> v4::Occupancy {
    let held = u32::try_from(scene.bed.held.len()).unwrap();
    v4::Occupancy {
        rbh: v4::Rbh(scene.room.occupants, scene.bed.seen + held, held),
        mode: scene.room.mode,
        bed: scene.bed.state,
        unexplained: scene.bed.unexplained,
    }
}

/// Checks that the turn in v4 brings down to the turn in v3.
pub fn check(output: &Output) {
    let (v3, v4) = (&output.scene, &output.v4.scene);
    let at = output.at;

    // The scene: the mode gives the state; what does not change across versions is equal.
    assert_eq!(
        state(v4.room.mode),
        v3.room.state,
        "{at:?}: room.mode → room.state"
    );
    assert_eq!(
        v4.room.occupants, v3.room.occupants,
        "{at:?}: room.occupants"
    );
    assert_eq!(v4.room.pending, v3.room.pending, "{at:?}: room.pending");
    assert!(
        since(v4.room.since) >= since(v3.room.since),
        "{at:?}: the mode cannot hold since before the state"
    );
    assert_eq!(
        (&v4.bed, &v4.floor, &v4.subject, &v4.evidence),
        (&v3.bed, &v3.floor, &v3.subject, &v3.evidence),
        "{at:?}: bed, floor, subject and evidence"
    );
    assert_eq!(
        (
            &v4.attention,
            &v4.map,
            &v4.sensor,
            &v4.republished,
            &v4.frame
        ),
        (
            &v3.attention,
            &v3.map,
            &v3.sensor,
            &v3.republished,
            &v3.frame
        ),
        "{at:?}: attention, map, sensor, republication and frame"
    );

    // The events: v4 brought down is v3, in the same order.
    assert_eq!(
        bring_down(&output.v4.events, v4),
        output.events,
        "{at:?}: events.v4 brought down to v3"
    );

    // Each transition: its cell and its after are those of its facts.
    for event in output.v4.events.iter() {
        if let v4::Event::Transition(t) = event {
            assert_eq!(
                t.cell,
                v4::Cell::new(t.before.occupancy.rbh, t.seen.fv, t.after.rbh),
                "{at:?}: the cell"
            );
            if let Some(last) = t.facts.last() {
                assert_eq!(last.after, t.after, "{at:?}: the after is the last fact's");
            }
            assert_eq!(
                v4.room.last_cell.as_ref(),
                Some(&t.cell),
                "{at:?}: the scene links the last cell"
            );
            assert!(
                t.facts.iter().all(|f| f.cause != Cause::SensorReported),
                "{at:?}: a layer-0 fact is not the sensor's"
            );
        }
    }
}

/// A total order to compare `since`: `None` before everything.
fn since(since: Option<SampleRef>) -> (u64, u64) {
    since.map_or((0, 0), |s| (s.session.0, s.sequence))
}
