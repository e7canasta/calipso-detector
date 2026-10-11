//! The translation to v4 (spec 1301): `calipso.scene.v4` and
//! `calipso.events.v4`. The room by machine mode, with its phase and last cell;
//! each layer-0 change as a transition with its before, what was seen, the
//! after and the facts, each with the occupancy it leaves.

use super::{Layer0, Shared, bed_state, empty_bed, republished_at, retained_subject};
use crate::belief::{Alone, BedPhase, Fact, InBed, Occupancy as Believed, Tally, ZoneCounts};
use crate::chronicle::Chronicled;
use crate::conclusion::RepublishReason;
use crate::contract::v4::{
    Attention, Before, Cause, Cell, Context, DerivedBody, DerivedEvent, Evidence,
    Fact as Published, FactBody, Floor, Fv, MapBody, MapEvent, MapState, Mode, Occupancy, Phase,
    Rbh, Republished, Room, SampleRef, Seen, SensorBody, SensorEvent, SensorState, Span,
    Transition,
};
pub(crate) use crate::contract::v4::{Event, Scene};

/// The machine's mode, said in v4.
pub(super) fn mode(counts: ZoneCounts) -> Mode {
    match crate::belief::Room::of(counts) {
        crate::belief::Room::Empty => Mode::Empty,
        crate::belief::Room::Alone(Alone::Out) => Mode::AloneOut,
        crate::belief::Room::Alone(Alone::InBed(InBed::Seen)) => Mode::AloneInBedSeen,
        crate::belief::Room::Alone(Alone::InBed(InBed::Held)) => Mode::AloneInBedHeld,
        crate::belief::Room::Accompanied(_) => Mode::Accompanied,
    }
}

fn phase(phase: BedPhase) -> Phase {
    match phase {
        BedPhase::Adopting => Phase::Booting,
        BedPhase::Tracking => Phase::Running,
    }
}

/// The `[R, B, H]` triple.
fn rbh(counts: ZoneCounts) -> Rbh {
    Rbh(counts.occupants(), counts.bed + counts.held, counts.held)
}

fn occupancy(tally: Tally, bed_known: bool) -> Occupancy {
    Occupancy {
        rbh: rbh(tally.counts),
        mode: mode(tally.counts),
        bed: bed_state(tally.counts, bed_known),
        unexplained: tally.unexplained,
    }
}

fn believed(occupancy_now: &Believed) -> Occupancy {
    occupancy(
        Tally {
            counts: occupancy_now.totals(),
            unexplained: occupancy_now.unexplained,
        },
        occupancy_now.bed_known,
    )
}

/// The cell of a transition: `before:seen:after`.
pub(super) fn cell(transition: &crate::belief::Transition) -> Cell {
    let seen = transition.seen.counts;
    Cell::new(
        rbh(transition.before.counts),
        Fv(seen.floor, seen.bed),
        rbh(transition.after.counts),
    )
}

/// The v4 scene of an observed turn and its events in order: the
/// transition, the calibration and, last, the derived ones.
pub(super) fn scene(
    shared: Shared,
    layer0: &Layer0,
    chronicled: &Chronicled,
) -> (Scene, Vec<Event>) {
    let occupancy_now = &layer0.occupancy;
    let after = believed(occupancy_now);
    let scene = Scene {
        frame: shared.frame,
        evidence: shared.evidence,
        room: Room {
            mode: after.mode,
            occupants: occupancy_now.occupants(),
            since: Some(chronicled.mode.since),
            pending: shared.pending,
            phase: phase(occupancy_now.phase),
            last_cell: chronicled.last_cell.clone(),
        },
        bed: shared.bed,
        floor: shared.floor,
        subject: shared.subject,
        attention: shared.attention,
        map: shared.map,
        sensor: shared.sensor,
        republished: shared.republished,
    };
    let mut events: Vec<_> = layer0
        .transition
        .as_ref()
        .map(|t| Event::Transition(transition(t, chronicled.transition_since)))
        .into_iter()
        .collect();
    events.extend(layer0.calibrated.map(|fact| {
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
            at: layer0.at,
            body,
            after,
        })
    };
    events.extend(chronicled.mode.changed_from.map(|from| {
        derived(DerivedBody::RoomChanged {
            from,
            to: scene.room.mode,
        })
    }));
    events.extend(chronicled.bed.changed_from.map(|from| {
        derived(DerivedBody::BedChanged {
            from,
            to: scene.bed.state,
        })
    }));
    (scene, events)
}

/// A layer-0 transition, with since when what we believed held. A transition
/// always has the zones: the bed is known.
fn transition(t: &crate::belief::Transition, since: Option<SampleRef>) -> Transition {
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
            counts: ZoneCounts {
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
    changed: Option<crate::sensor::SensorChanged>,
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
