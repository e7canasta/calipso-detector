//! What a turn published, read as a flat list of facts: one row per layer-0
//! fact, calibration, derived change or sensor change, in the order they were
//! published, each with the occupancy it leaves. It is how the steps and the
//! properties ask "was an exit from the bed observed?" without walking the
//! shape of `events.v5`.
//!
//! It also checks, on every turn, what each transition promises: its cell is
//! its before, seen and after; its after is its last fact's; the scene links
//! its cell; and a layer-0 fact is never the sensor's.

use scene_contract::v5::{DerivedBody, Event, FactBody, MapBody, Occupancy, Rbh, SensorBody};
use scene_contract::{BedState, Cause, Mode, SampleRef, SensorState};
use scene_core::Output;

/// Which kind of row: a fact of the domain (layer 0 or the map), a change
/// derived by comparing turns, or the sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Domain,
    Derived,
    Sensor,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Body {
    Entered,
    LeftRoom,
    LayDown,
    LeftBed,
    HeldInBed { last_seen: SampleRef },
    Reappeared { held_since: SampleRef },
    BedCalibrated { reference_m: f32 },
    RoomChanged { from: Mode, to: Mode },
    BedChanged { from: BedState, to: BedState },
    SensorChanged { from: SensorState, to: SensorState },
}

/// The occupancy a row leaves, by zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct After {
    pub mode: Mode,
    pub occupants: u32,
    pub bed: BedState,
    pub bed_seen: u32,
    pub bed_held: u32,
    pub floor_seen: u32,
}

impl From<Occupancy> for After {
    fn from(o: Occupancy) -> Self {
        let Rbh {
            room: r,
            bed: b,
            held: h,
        } = o.rbh;
        Self {
            mode: o.mode,
            occupants: r,
            bed: o.bed,
            bed_seen: b - h,
            bed_held: h,
            floor_seen: r - b,
        }
    }
}

/// One published row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fact {
    pub kind: Kind,
    pub cause: Cause,
    /// The sample of the row; `None` for a reception change that observes none.
    pub at: Option<SampleRef>,
    pub body: Body,
    pub after: After,
}

impl Fact {
    /// Its `type`, as published (`entered`, `bed_changed`…).
    pub fn type_name(&self) -> &'static str {
        match self.body {
            Body::Entered => "entered",
            Body::LeftRoom => "left_room",
            Body::LayDown => "lay_down",
            Body::LeftBed => "left_bed",
            Body::HeldInBed { .. } => "held_in_bed",
            Body::Reappeared { .. } => "reappeared",
            Body::BedCalibrated { .. } => "bed_calibrated",
            Body::RoomChanged { .. } => "room_changed",
            Body::BedChanged { .. } => "bed_changed",
            Body::SensorChanged { .. } => "sensor_changed",
        }
    }
}

fn fact_body(body: FactBody) -> Body {
    match body {
        FactBody::Entered => Body::Entered,
        FactBody::LeftRoom => Body::LeftRoom,
        FactBody::LayDown => Body::LayDown,
        FactBody::LeftBed => Body::LeftBed,
        FactBody::HeldInBed { last_seen } => Body::HeldInBed { last_seen },
        FactBody::Reappeared { held_since } => Body::Reappeared { held_since },
    }
}

/// The occupancy a scene states: what a map row leaves, since the map is not
/// occupancy.
fn stated(scene: &scene_contract::v5::Scene) -> Occupancy {
    let held = u32::try_from(scene.bed.held.len()).unwrap();
    Occupancy {
        rbh: Rbh {
            room: scene.room.occupants,
            bed: scene.bed.seen + held,
            held,
        },
        mode: scene.room.mode,
        bed: scene.bed.state,
        unexplained: scene.bed.unexplained,
    }
}

/// A turn's events as rows, in order.
pub fn facts(output: &Output) -> Vec<Fact> {
    let mut rows = Vec::new();
    for event in &output.v5.events {
        match event {
            Event::Transition(t) => rows.extend(t.facts.iter().map(|f| Fact {
                kind: Kind::Domain,
                cause: f.cause,
                at: Some(t.at),
                body: fact_body(f.body),
                after: f.after.into(),
            })),
            Event::Map(m) => {
                let MapBody::BedCalibrated { reference } = m.body;
                rows.push(Fact {
                    kind: Kind::Domain,
                    cause: m.cause,
                    at: Some(m.at),
                    body: Body::BedCalibrated {
                        reference_m: reference,
                    },
                    after: stated(&output.v5.scene).into(),
                });
            }
            Event::Derived(d) => rows.push(Fact {
                kind: Kind::Derived,
                cause: d.cause,
                at: Some(d.at),
                body: match d.body {
                    DerivedBody::RoomChanged { from, to } => Body::RoomChanged { from, to },
                    DerivedBody::BedChanged { from, to } => Body::BedChanged { from, to },
                },
                after: d.after.into(),
            }),
            // A change of the parts' places carries no facts: it is not on
            // layer 0's tape.
            Event::Parts(_) => {}
            // Nor does a change of layer 1: its row is in its own table.
            Event::BedPosture(_) => {}
            Event::Sensor(s) => {
                let SensorBody::SensorChanged { from, to } = s.body;
                rows.push(Fact {
                    kind: Kind::Sensor,
                    cause: s.cause,
                    at: s.at,
                    body: Body::SensorChanged { from, to },
                    after: s.after.into(),
                });
            }
        }
    }
    rows
}

/// What each transition of the turn promises.
pub fn check(output: &Output) {
    let at = output.at;
    for event in &output.v5.events {
        if let Event::Transition(t) = event {
            assert_eq!(
                t.cell,
                scene_core::cell_of(t.before.occupancy.rbh, t.seen.fv, t.after.rbh,),
                "{at:?}: the cell"
            );
            // The occupants are those of the last fact; the bodies that are
            // nobody move without a fact (rule 6), so `unexplained` may differ.
            if let Some(last) = t.facts.last() {
                assert_eq!(
                    (last.after.rbh, last.after.mode, last.after.bed),
                    (t.after.rbh, t.after.mode, t.after.bed),
                    "{at:?}: the after's occupants are the last fact's"
                );
            }
            assert_eq!(
                t.after.unexplained, output.v5.scene.bed.unexplained,
                "{at:?}: the after's unexplained is the scene's"
            );
            assert_eq!(
                output.v5.scene.room.last_cell.as_ref(),
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
