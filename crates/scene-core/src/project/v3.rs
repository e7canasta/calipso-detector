//! The translation to v3: `calipso.scene.v3` and `calipso.events.v3`. The
//! room by state (`empty`, `one`, `several`), the facts one by one as domain
//! events and v3's `after`.

use super::{Layer0, Shared, bed_state};
use crate::belief::{Occupancy, ZoneCounts};
use crate::chronicle::Chronicled;
use crate::contract::v3::{
    Attention, Bed, BedState, Floor, MapState, RepublishReason, Republished, Room, RoomState,
    SampleRef, SensorState,
};
pub(crate) use crate::contract::v3::{Event, Scene};

/// The room's published state: the machine's mode said in the contract's
/// vocabulary. The translation lives here and not in layer 0.
pub(super) fn room_state(counts: ZoneCounts) -> RoomState {
    match crate::belief::Room::of(counts) {
        crate::belief::Room::Empty => RoomState::Empty,
        crate::belief::Room::Alone(_) => RoomState::One,
        crate::belief::Room::Accompanied(_) => RoomState::Several,
    }
}

/// The v3 scene of an observed turn and its events in order: the domain
/// ones, the calibration and, last, those derived from the states that
/// changed.
pub(super) fn scene(
    shared: Shared,
    layer0: &Layer0,
    chronicled: &Chronicled,
) -> (Scene, Vec<Event>) {
    let occupancy = &layer0.occupancy;
    let after = after_occupancy(occupancy);
    let scene = Scene {
        frame: shared.frame,
        evidence: shared.evidence,
        room: Room {
            state: room_state(occupancy.totals()),
            occupants: occupancy.occupants(),
            since: Some(chronicled.room.since),
            pending: shared.pending,
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
        .map(publish_transition)
        .unwrap_or_default();
    events.extend(layer0.calibrated.map(|fact| {
        Event::domain(
            fact.at,
            crate::contract::v3::Cause::MapLearned,
            crate::contract::v3::EventBody::BedCalibrated {
                reference_m: fact.reference_m,
            },
            after,
        )
    }));
    events.extend(changes(
        layer0.at,
        &scene,
        chronicled.room.changed_from,
        chronicled.bed.changed_from,
    ));
    (scene, events)
}

pub(crate) fn sensor_events(
    changed: Option<crate::sensor::SensorChanged>,
    previous: Option<&Scene>,
) -> Vec<Event> {
    let after = previous.map_or_else(empty_after, after_scene);
    changed
        .into_iter()
        .map(|fact| Event::sensor(fact.at, fact.from, fact.to, after))
        .collect()
}

/// The last scene, without a new observation and declared as such. What was
/// current becomes retained: nothing is `Current` without an observation
/// (rule 2).
pub(crate) fn republish(
    last: Option<(&Scene, SampleRef)>,
    reason: RepublishReason,
    sensor: SensorState,
    map: MapState,
) -> Scene {
    let Some((scene, projected_at)) = last else {
        return Scene {
            frame: None,
            evidence: crate::contract::v3::Evidence::default(),
            room: Room {
                state: RoomState::Empty,
                occupants: 0,
                since: None,
                pending: None,
            },
            bed: super::empty_bed(),
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
    let observed_at = super::republished_at(scene.republished, projected_at);
    let subject = super::retained_subject(scene.subject.as_ref(), projected_at);
    Scene {
        frame: scene.frame,
        evidence: crate::contract::v3::Evidence::default(),
        room: scene.room.clone(),
        bed: scene.bed.clone(),
        floor: scene.floor,
        subject,
        attention: Attention::default(),
        map: scene.map,
        sensor,
        republished: Some(Republished {
            reason,
            observed_at,
        }),
    }
}

/// The facts of a layer-0 transition, as domain events. The cause belongs to
/// publishing: adoption at startup is `baseline`; the rest, what stabilized.
fn publish_transition(transition: &crate::belief::Transition) -> Vec<Event> {
    use crate::belief::Fact;
    use crate::contract::v3::{Cause, EventBody};
    let at = transition.at;
    transition
        .facts
        .iter()
        .map(|happened| {
            let (body, cause) = match happened.fact {
                Fact::Entered => (EventBody::Entered, Cause::Stabilized),
                Fact::Adopted => (EventBody::LayDown, Cause::Baseline),
                Fact::LeftRoom => (EventBody::LeftRoom, Cause::Stabilized),
                Fact::LayDown => (EventBody::LayDown, Cause::Stabilized),
                Fact::LeftBed => (EventBody::LeftBed, Cause::Stabilized),
                Fact::HeldInBed { last_seen } => {
                    (EventBody::HeldInBed { last_seen }, Cause::Stabilized)
                }
                Fact::Reappeared { held_since } => {
                    (EventBody::Reappeared { held_since }, Cause::Stabilized)
                }
            };
            Event::domain(at, cause, body, after_counts(happened.after.counts))
        })
        .collect()
}

pub(crate) fn after_scene(scene: &Scene) -> crate::contract::v3::After {
    after_parts(&scene.room, &scene.bed, scene.floor)
}

fn after_occupancy(occupancy: &Occupancy) -> crate::contract::v3::After {
    crate::contract::v3::After {
        room: room_state(occupancy.totals()),
        occupants: occupancy.occupants(),
        bed: bed_state(occupancy.totals(), occupancy.bed_known),
        bed_seen: occupancy.counts.bed,
        bed_held: u32::try_from(occupancy.held.len()).unwrap_or(u32::MAX),
        floor_seen: occupancy.counts.floor,
    }
}

fn after_parts(room: &Room, bed: &Bed, floor: Floor) -> crate::contract::v3::After {
    crate::contract::v3::After {
        room: room.state,
        occupants: room.occupants,
        bed: bed.state,
        bed_seen: bed.seen,
        bed_held: u32::try_from(bed.held.len()).unwrap_or(u32::MAX),
        floor_seen: floor.seen,
    }
}

pub(crate) fn empty_after() -> crate::contract::v3::After {
    after_counts(crate::belief::ZoneCounts {
        bed: 0,
        floor: 0,
        held: 0,
    })
}

fn after_counts(counts: crate::belief::ZoneCounts) -> crate::contract::v3::After {
    crate::contract::v3::After {
        room: room_state(counts),
        occupants: counts.occupants(),
        bed: bed_state(counts, true),
        bed_seen: counts.bed,
        bed_held: counts.held,
        floor_seen: counts.floor,
    }
}

/// The events derived from the states that changed: always after the domain
/// and the calibration.
fn changes(
    at: SampleRef,
    scene: &Scene,
    room_from: Option<RoomState>,
    bed_from: Option<BedState>,
) -> Vec<Event> {
    use crate::contract::v3::{Cause, EventBody, EventKind};
    let after = after_scene(scene);
    let room = room_from.map(|from| EventBody::RoomChanged {
        from,
        to: scene.room.state,
    });
    let bed = bed_from.map(|from| EventBody::BedChanged {
        from,
        to: scene.bed.state,
    });
    room.into_iter()
        .chain(bed)
        .map(|body| Event {
            kind: EventKind::Derived,
            cause: Cause::Stabilized,
            at,
            body,
            after,
        })
        .collect()
}
