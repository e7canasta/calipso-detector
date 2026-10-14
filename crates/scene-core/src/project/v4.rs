//! The translation to v4: `calipso.scene.v4` and
//! `calipso.events.v4`. The room by machine mode, with its phase and last cell;
//! each layer-0 change as a transition with its before, what was seen, the
//! after and the facts, each with the occupancy it leaves.

use super::{
    bed, bed_state, empty_bed, pending, project_evidence, publish_bed, publish_subject,
    republished_at, retained_subject, unobserved,
};
use crate::belief::{self, Alone, BedPhase, Fact, InBed, Occupants, Tally};
use crate::chronicle::{Chronicled, RoomName, TransitionKey};
use crate::conclusion::{Observation, RepublishReason};
use crate::contract::v4::{
    Attention, Before, Cause, Cell, Context, DerivedBody, DerivedEvent, Evidence,
    Fact as Published, FactBody, Floor, Fv, MapBody, MapEvent, MapState, Mode, Occupancy, Phase,
    Rbh, Republished, Room, SampleRef, Seen, SensorBody, SensorEvent, SensorState, Span,
    Transition,
};
pub(crate) use crate::contract::v4::{Event, Scene};
use crate::map::BedCalibrated;
use crate::sensor::SensorChanged;

/// The machine's mode, said in v4.
pub(super) fn mode(occupants: Occupants) -> Mode {
    publish_room(RoomName::of(occupants))
}

pub(super) fn publish_room(name: RoomName) -> Mode {
    match name {
        RoomName::Empty => Mode::Empty,
        RoomName::Alone(Alone::Out) => Mode::AloneOut,
        RoomName::Alone(Alone::InBed(InBed::Seen)) => Mode::AloneInBedSeen,
        RoomName::Alone(Alone::InBed(InBed::Held)) => Mode::AloneInBedHeld,
        RoomName::Accompanied => Mode::Accompanied,
    }
}

fn phase(phase: BedPhase) -> Phase {
    match phase {
        BedPhase::Adopting => Phase::Booting,
        BedPhase::Tracking => Phase::Running,
    }
}

/// The `[R, B, H]` triple.
fn rbh(occupants: Occupants) -> Rbh {
    Rbh(
        occupants.total(),
        occupants.bed + occupants.held,
        occupants.held,
    )
}

fn occupancy(tally: Tally, bed_known: bool) -> Occupancy {
    Occupancy {
        rbh: rbh(tally.occupants),
        mode: mode(tally.occupants),
        bed: bed_state(tally.occupants, bed_known),
        unexplained: tally.unexplained,
    }
}

fn believed_occupancy(occupancy_now: &belief::Occupancy) -> Occupancy {
    occupancy(
        Tally {
            occupants: occupancy_now.occupants(),
            unexplained: occupancy_now.unexplained,
        },
        occupancy_now.bed_known,
    )
}

/// The cell of a transition: `before:seen:after`.
fn cell(transition: &belief::Transition) -> Cell {
    publish_cell(&TransitionKey::of(transition))
}

fn publish_cell(key: &TransitionKey) -> Cell {
    Cell::new(
        rbh(key.before),
        Fv(key.seen.floor, key.seen.bed),
        rbh(key.after),
    )
}

/// The v4 scene of an observed turn and its events in order: the
/// transition, the calibration and, last, the derived ones.
pub(crate) fn observed(observation: Observation, chronicled: &Chronicled) -> (Scene, Vec<Event>) {
    let Observation {
        at,
        believed,
        refined,
        bed_posture,
        parts,
        held_check,
        evidence,
        frame,
        calibrated,
        attention,
        map,
        sensor,
        dwell_secs,
    } = observation;
    let republished = unobserved(&evidence.bodies.bodies, chronicled.observed);
    let evidence = project_evidence(&evidence, believed.subject.as_ref(), &attention);
    let subject = publish_subject(&believed, refined, bed_posture, parts, chronicled);
    let belief::Believed {
        occupancy: occupancy_now,
        transition: changed,
        ..
    } = believed;
    let after = believed_occupancy(&occupancy_now);
    let scene = Scene {
        frame,
        evidence,
        room: Room {
            mode: after.mode,
            occupants: occupancy_now.occupants().total(),
            since: Some(chronicled.room.since),
            pending: pending(&occupancy_now, dwell_secs),
            phase: phase(occupancy_now.phase),
            last_cell: chronicled.last_transition.as_ref().map(publish_cell),
        },
        bed: bed(&occupancy_now, chronicled, held_check),
        floor: Floor {
            seen: occupancy_now.counts.floor,
        },
        subject,
        attention,
        map,
        sensor,
        republished,
    };
    let events = events(at, changed.as_ref(), calibrated, chronicled, after, &scene);
    (scene, events)
}

/// The events of an observed turn, in order: the layer-0 transition, the
/// calibration and, last, the derived ones, each with the occupancy after.
fn events(
    at: SampleRef,
    changed: Option<&belief::Transition>,
    calibrated: Option<BedCalibrated>,
    chronicled: &Chronicled,
    after: Occupancy,
    scene: &Scene,
) -> Vec<Event> {
    let mut events: Vec<_> = changed
        .map(|t| Event::Transition(transition(t, chronicled.transition_since)))
        .into_iter()
        .collect();
    events.extend(calibrated.map(|fact| {
        Event::Map(MapEvent {
            cause: Cause::MapLearned,
            at: fact.at,
            body: MapBody::BedCalibrated {
                reference_m: fact.reference_m,
            },
        })
    }));
    let derived = |body| {
        Event::Derived(DerivedEvent {
            cause: Cause::Stabilized,
            at,
            body,
            after,
        })
    };
    events.extend(chronicled.room.changed_from.map(|from| {
        derived(DerivedBody::RoomChanged {
            from: publish_room(from),
            to: scene.room.mode,
        })
    }));
    events.extend(chronicled.bed.changed_from.map(|from| {
        derived(DerivedBody::BedChanged {
            from: publish_bed(from),
            to: scene.bed.state,
        })
    }));
    events
}

/// A layer-0 transition, with since when what we believed held. A transition
/// always has the zones: the bed is known.
fn transition(t: &belief::Transition, since: Option<SampleRef>) -> Transition {
    Transition {
        at: t.at,
        cell: cell(t),
        phase: phase(t.phase),
        before: Before {
            occupancy: occupancy(t.before, true),
            since,
            lasted_secs: since.and_then(|since| t.at.secs_since(since)),
        },
        seen: Seen {
            fv: Fv(t.seen.counts.floor, t.seen.counts.bed),
            span: Span {
                first: t.first_seen,
                secs: t.at.secs_since(t.first_seen).unwrap_or(0.0),
            },
            context: Context {
                far: t.seen.from_door,
                passing: t.seen.passing,
            },
        },
        after: occupancy(t.after, true),
        facts: t
            .facts
            .iter()
            .map(|happened| {
                let (body, cause) = match happened.fact {
                    Fact::Entered => (FactBody::Entered, Cause::Stabilized),
                    Fact::Adopted => (FactBody::LayDown, Cause::Baseline),
                    Fact::LeftRoom => (FactBody::LeftRoom, Cause::Stabilized),
                    Fact::LayDown => (FactBody::LayDown, Cause::Stabilized),
                    Fact::LeftBed => (FactBody::LeftBed, Cause::Stabilized),
                    Fact::HeldInBed { last_seen } => {
                        (FactBody::HeldInBed { last_seen }, Cause::Stabilized)
                    }
                    Fact::Reappeared { held_since } => {
                        (FactBody::Reappeared { held_since }, Cause::Stabilized)
                    }
                };
                Published {
                    body,
                    cause,
                    after: occupancy(happened.after, true),
                }
            })
            .collect(),
    }
}

/// The occupancy a published scene states.
fn published(scene: &Scene) -> Occupancy {
    let held = u32::try_from(scene.bed.held.len()).unwrap_or(u32::MAX);
    Occupancy {
        rbh: Rbh(scene.room.occupants, scene.bed.seen + held, held),
        mode: scene.room.mode,
        bed: scene.bed.state,
        unexplained: scene.bed.unexplained,
    }
}

fn empty() -> Occupancy {
    occupancy(
        Tally {
            occupants: Occupants {
                floor: 0,
                bed: 0,
                held: 0,
            },
            unexplained: 0,
        },
        true,
    )
}

pub(crate) fn sensor_events(
    changed: Option<SensorChanged>,
    previous: Option<&Scene>,
) -> Vec<Event> {
    let after = previous.map_or_else(empty, published);
    changed
        .into_iter()
        .map(|fact| {
            Event::Sensor(SensorEvent {
                cause: Cause::SensorReported,
                at: fact.at,
                body: SensorBody::SensorChanged {
                    from: fact.from,
                    to: fact.to,
                },
                after,
            })
        })
        .collect()
}

/// The last scene, without a new observation and declared as such (rule 2).
pub(crate) fn republish(
    last: Option<(&Scene, SampleRef)>,
    reason: RepublishReason,
    sensor: SensorState,
    map: MapState,
) -> Scene {
    let Some((scene, projected_at)) = last else {
        return Scene {
            frame: None,
            evidence: Evidence::default(),
            room: Room {
                mode: Mode::Empty,
                occupants: 0,
                since: None,
                pending: None,
                phase: Phase::Booting,
                last_cell: None,
            },
            bed: empty_bed(),
            floor: Floor { seen: 0 },
            subject: None,
            attention: Attention::default(),
            map,
            sensor,
            republished: Some(Republished {
                reason,
                observed_at: None,
            }),
        };
    };
    Scene {
        frame: scene.frame,
        evidence: Evidence::default(),
        room: scene.room.clone(),
        bed: scene.bed.clone(),
        floor: scene.floor,
        subject: retained_subject(scene.subject.as_ref(), projected_at),
        attention: Attention::default(),
        map: scene.map,
        sensor,
        republished: Some(Republished {
            reason,
            observed_at: republished_at(scene.republished, projected_at),
        }),
    }
}
