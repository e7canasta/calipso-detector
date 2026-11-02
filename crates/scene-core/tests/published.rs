//! What a turn published, read as a flat list of facts: one row per layer-0
//! fact, calibration, derived change or sensor change, in the order they were
//! published, each with the occupancy it leaves. It is how the steps and the
//! properties ask "was an exit from the bed observed?" without walking the
//! shape of `events.v4`.
//!
//! It also checks, on every turn, what each transition promises: its cell is
//! its before, seen and after; its after is its last fact's; the scene links
//! its cell; and a layer-0 fact is never the sensor's.

use scene_contract::{
    BedState, Cause, DerivedBody, Event, FactBody, MapBody, Mode, Occupancy, Rbh, SampleRef, Scene,
    SensorBody, SensorState,
};
use scene_core::Output;
use serde_json::Value;

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
        let Rbh(r, b, h) = o.rbh;
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
    pub at: SampleRef,
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
fn stated(scene: &Scene) -> Occupancy {
    let held = u32::try_from(scene.bed.held.len()).unwrap();
    Occupancy {
        rbh: Rbh(scene.room.occupants, scene.bed.seen + held, held),
        mode: scene.room.mode,
        bed: scene.bed.state,
        unexplained: scene.bed.unexplained,
    }
}

/// A turn's events as rows, in order.
pub fn facts(output: &Output) -> Vec<Fact> {
    let mut rows = Vec::new();
    for event in &output.events {
        match event {
            Event::Transition(t) => rows.extend(t.facts.iter().map(|f| Fact {
                kind: Kind::Domain,
                cause: f.cause,
                at: t.at,
                body: fact_body(f.body),
                after: f.after.into(),
            })),
            Event::Map(m) => {
                let MapBody::BedCalibrated { reference_m } = m.body;
                rows.push(Fact {
                    kind: Kind::Domain,
                    cause: m.cause,
                    at: m.at,
                    body: Body::BedCalibrated { reference_m },
                    after: stated(&output.scene).into(),
                });
            }
            Event::Derived(d) => rows.push(Fact {
                kind: Kind::Derived,
                cause: d.cause,
                at: d.at,
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
    check_v5_reads_back_to_v4(output);
    let at = output.at;
    for event in &output.events {
        if let Event::Transition(t) = event {
            assert_eq!(
                t.cell,
                scene_core::cell_of(t.before.occupancy.rbh, t.seen.fv, t.after.rbh),
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
                t.after.unexplained, output.scene.bed.unexplained,
                "{at:?}: the after's unexplained is the scene's"
            );
            assert_eq!(
                output.scene.room.last_cell.as_ref(),
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

/// C5, on every turn of every scenario: the v5 scene and events, read back to
/// v4, are the v4 ones, except what v5 changes on purpose (ADR "Lo que espera el
/// contrato v5"). Every capability a scenario configures is covered, since
/// every scenario reaches here (shadow, two planes, pose, depth, calibration).
pub fn check_v5_reads_back_to_v4(output: &Output) {
    let v4 = serde_json::to_value(&*output.scene).expect("the v4 scene serializes");
    let v5 =
        read_back_to_v4(serde_json::to_value(&*output.v5.scene).expect("the v5 scene serializes"));
    let mut diffs = Vec::new();
    differences(&v5, &v4, "", &mut diffs);
    assert!(
        diffs.is_empty(),
        "sample {}: v5 reads back differently from v4: {diffs:?}",
        output.at.sequence
    );
    let v4_events = output.events.len();
    assert_eq!(
        output.v5.events.len(),
        v4_events,
        "sample {}: v5 publishes a different number of events",
        output.at.sequence
    );
}

/// What v5 changes on purpose, by the JSON path it changes. Only the five
/// changes of the ADR: `room.occupants` is not one of them (it is the same count
/// in both versions), so a difference there is a failure.
const CHANGED_ON_PURPOSE: &[&str] = &[
    // change 3: the subject has no verdict.
    "subject.verdict",
    // change 4: the depth and the calibration reference, relative.
    "subject.depth.meters",
    "map.calibration.reference_m",
    // change 5: rbh and fv with names.
    "rbh",
    "fv",
];

/// Reads a v5 JSON back to v4 names: the inverse of the five changes.
fn read_back_to_v4(mut value: Value) -> Value {
    fn rename(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if let Some(depth) = map.get_mut("depth")
                    && let Some(relative) = depth.get("relative").cloned()
                    && let Some(d) = depth.as_object_mut()
                {
                    d.remove("relative");
                    d.insert("meters".into(), relative);
                }
                if let Some(cal) = map.get_mut("calibration")
                    && let Some(reference) = cal.get("reference").cloned()
                    && let Some(c) = cal.as_object_mut()
                {
                    c.remove("reference");
                    c.insert("reference_m".into(), reference);
                }
                if let Some(rbh) = map.get("rbh").cloned()
                    && rbh.is_object()
                {
                    map.insert(
                        "rbh".into(),
                        serde_json::json!([rbh["room"], rbh["bed"], rbh["held"]]),
                    );
                }
                if let Some(fv) = map.get("fv").cloned()
                    && fv.is_object()
                {
                    map.insert("fv".into(), serde_json::json!([fv["floor"], fv["bed"]]));
                }
                for v in map.values_mut() {
                    rename(v);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(rename),
            _ => {}
        }
    }
    rename(&mut value);
    value
}

/// The paths where two JSON values differ, ignoring what v5 changes on purpose.
fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<_> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                differences(
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    &p,
                    out,
                );
            }
        }
        _ => {
            if !CHANGED_ON_PURPOSE.iter().any(|c| path.ends_with(c)) {
                out.push(format!("{path}: v5 read back {a} != v4 {b}"));
            }
        }
    }
}
