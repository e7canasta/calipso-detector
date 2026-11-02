//! The v5 projection of an observed turn: `scene.v5` and `events.v5`, from the
//! same `Observation` and chronicle that v4 reads (C4). It depends on the domain
//! and on the common projection (`project/mod.rs`), never on v4: the day v4 is
//! retired, v5 stands on its own (DESIGN, the versions do not depend on each
//! other). The equivalence test (C5) compares two independent projections.

use super::parts;
use super::{
    bed, bed_state, cell_of, empty_bed, pending, project_evidence, publish_bed, publish_room,
    publish_subject, republished_at, unobserved,
};
use crate::belief::total;
use crate::belief::{self, Occupancy as BeliefOccupancy, Occupants};
use crate::chronicle::{Chronicled, RoomName, TransitionKey};
use crate::conclusion::{Observation, RepublishReason};
use crate::reception::SensorChanged;
use crate::time::SampleRef;
use crate::time::SourceTime;
use scene_contract::v5::{
    self, Before, Context, DerivedBody, Event, Fact, FactBody, Fv, Occupancy, Rbh, Scene, Seen,
    Span, Transition,
};
use scene_contract::{Cause, Mode, Phase, Republished, SensorState};
use scene_contract::{PartsAfter, PartsBefore, PartsSeen};
use std::sync::Arc;

/// The v5 scene and events of one turn, as the shell publishes them.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    /// The scene of the sample, in v5.
    pub scene: Arc<Scene>,
    /// The events the turn explains, in v5, in order.
    pub events: Vec<Event>,
}

/// The v5 projection of an observed turn: the scene and its events, from the
/// domain only.
pub(crate) fn observed(observation: &Observation, chronicled: &Chronicled) -> Projection {
    let (scene, events) = scene_and_events(observation, chronicled);
    Projection {
        scene: Arc::new(scene),
        events,
    }
}

fn scene_and_events(observation: &Observation, chronicled: &Chronicled) -> (Scene, Vec<Event>) {
    let at = observation.at;
    let believed = &observation.believed;
    let occupancy_now = &believed.occupancy;
    let dwell_secs = observation.dwell_secs;
    let republished = unobserved(&observation.evidence.bodies.bodies, chronicled.observed);
    let evidence = project_evidence(
        &observation.evidence,
        believed.subject.as_ref(),
        &observation.attention,
    );
    let subject = publish_subject_v5(observation, chronicled, at);
    let after = believed_occupancy(occupancy_now);
    let scene = Scene {
        frame: observation.frame,
        evidence,
        room: scene_contract::Room {
            mode: after.mode,
            occupants: total(occupancy_now.occupants()),
            since: Some(chronicled.room.since),
            pending: pending(occupancy_now, dwell_secs),
            phase: phase(occupancy_now.phase),
            last_cell: chronicled.last_transition.as_ref().map(publish_cell),
        },
        bed: bed(occupancy_now, chronicled, observation.held_check),
        floor: scene_contract::Floor {
            seen: occupancy_now.counts.floor,
        },
        subject,
        attention: observation.attention.clone(),
        map: map_v5(&observation.map),
        sensor: observation.sensor,
        republished,
    };
    let events = events(at, observation, chronicled, after, &scene);
    (scene, events)
}

/// The occupancy after the turn: the domain's count, named for v5 (change 5).
fn believed_occupancy(occupancy_now: &BeliefOccupancy) -> Occupancy {
    occupancy(
        occupancy_now.occupants(),
        occupancy_now.unexplained,
        occupancy_now.bed_known,
    )
}

/// The occupancy in v5: the domain's occupants, named as `Rbh` (change 5).
fn occupancy(occupants: Occupants, unexplained: u32, bed_known: bool) -> Occupancy {
    Occupancy {
        rbh: rbh(occupants),
        mode: mode(occupants),
        bed: bed_state(occupants, bed_known),
        unexplained,
    }
}

/// The `Rbh` of the domain's occupants: the room in total, the bed (the seen
/// and the held) and the held in bed.
fn rbh(occupants: Occupants) -> Rbh {
    Rbh {
        room: total(occupants),
        bed: occupants.bed + occupants.held,
        held: occupants.held,
    }
}

/// The room's mode, from the domain's occupants.
fn mode(occupants: Occupants) -> Mode {
    publish_room(RoomName::of(occupants))
}

/// The phase of the bed, as v5 names it: the same states as v4.
fn phase(phase: crate::belief::BedPhase) -> Phase {
    match phase {
        crate::belief::BedPhase::Adopting => Phase::Booting,
        crate::belief::BedPhase::Tracking => Phase::Running,
    }
}

/// The cell of a transition in v5: `before:seen:after`, the same cell as v4.
fn publish_cell(key: &TransitionKey) -> scene_contract::Cell {
    cell_of(
        rbh_v4(key.before),
        scene_contract::Fv(key.seen.floor, key.seen.bed),
        rbh_v4(key.after),
    )
}

/// The v4 triple that `cell_of` writes: the cell's text is the same in both
/// versions, so the key is written with the same function.
fn rbh_v4(occupants: Occupants) -> scene_contract::Rbh {
    scene_contract::Rbh(
        total(occupants),
        occupants.bed + occupants.held,
        occupants.held,
    )
}

fn publish_subject_v5(
    observation: &Observation,
    chronicled: &Chronicled,
    at: SampleRef,
) -> Option<v5::Subject> {
    // The same subject as v4 (`publish_subject`), with the depth relative and
    // without verdict (changes 3 and 4): the common projection decides its
    // values, this only names them.
    let subject = publish_subject(
        &observation.believed,
        observation.refined.clone(),
        observation.bed_posture,
        observation.parts,
        chronicled,
        at,
        observation.dwell_secs,
    )?;
    Some(v5::Subject {
        zone: subject.zone,
        bbox: subject.bbox,
        confidence: subject.confidence,
        provenance: subject.provenance,
        pose: subject.pose,
        depth: subject.depth.map(|d| v5::SubjectDepth {
            relative: d.meters,
            provenance: d.provenance,
        }),
        lying: subject.lying,
        posture: subject.posture,
        bed_posture: subject.bed_posture,
        parts: subject.parts,
    })
}

fn map_v5(map: &crate::map::MapState) -> v5::MapState {
    v5::MapState {
        geometry: map.geometry,
        bed: map.bed,
        headboard: map.headboard,
        focus: map.focus,
        shadow: map.shadow,
        calibration: calibration_v5(map.calibration),
    }
}

fn calibration_v5(calibration: scene_contract::Calibration) -> v5::Calibration {
    match calibration {
        scene_contract::Calibration::Complete { reference_m, since } => v5::Calibration::Complete {
            reference: reference_m,
            since,
        },
        _ => v5::Calibration::Unavailable,
    }
}

/// The events of an observed turn in v5, in order: the layer-0 transition, the
/// calibration, the derived ones, the parts and, last, layer 1 (as in v4).
fn events(
    at: SampleRef,
    observation: &Observation,
    chronicled: &Chronicled,
    after: Occupancy,
    scene: &Scene,
) -> Vec<Event> {
    let mut events: Vec<Event> = observation
        .believed
        .transition
        .as_ref()
        .map(|t| Event::Transition(transition(t, chronicled.transition_since)))
        .into_iter()
        .collect();
    events.extend(observation.calibrated.map(|fact| {
        Event::Map(v5::MapEvent {
            cause: Cause::MapLearned,
            at: fact.at,
            body: v5::MapBody::BedCalibrated {
                reference: fact.reference_m,
            },
        })
    }));
    let derived = |body| {
        Event::Derived(v5::DerivedEvent {
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
    if let Some(t) = observation
        .parts
        .as_ref()
        .and_then(|p| p.transition.as_ref())
    {
        events.push(Event::Parts(parts_change(
            t,
            chronicled.parts_transition_since,
        )));
    }
    // Layer 1 last: it reads layer 0, as its row does (as in v4).
    events.extend(
        bed_posture_change(at, observation.bed_posture.as_ref(), chronicled).map(Event::BedPosture),
    );
    events
}

/// A change of layer 1 while it is followed: the same row as v4's, with the
/// states it believed and the sample it perceived.
fn bed_posture_change(
    at: SampleRef,
    now: Option<&crate::belief::bed_posture::Concluded>,
    chronicled: &Chronicled,
) -> Option<scene_contract::BedPostureChange> {
    let now = now?;
    let before = chronicled.bed_posture.changed_from.flatten()?;
    let since = chronicled.bed_posture_transition_since;
    let seen = scene_contract::BedPostureSeen::from(now.seen);
    Some(scene_contract::BedPostureChange {
        at,
        cell: format!(
            "{} | {} | {}",
            before.as_str(),
            seen.as_str(),
            now.state.as_str()
        ),
        before: scene_contract::BedPostureBefore {
            state: before,
            since,
            lasted_secs: since.and_then(|since| at.since(since).published()),
        },
        seen,
        after: now.state,
    })
}

/// A layer-0 transition in v5, with since when what we believed held.
fn transition(t: &belief::Transition, since: Option<SampleRef>) -> Transition {
    Transition {
        at: t.at,
        cell: cell_v5(t),
        phase: phase(t.phase),
        before: Before {
            occupancy: occupancy(t.before.occupants, t.before.unexplained, true),
            since,
            lasted_secs: since.and_then(|since| t.at.since(since).published()),
        },
        seen: Seen {
            fv: Fv {
                floor: t.seen.counts.floor,
                bed: t.seen.counts.bed,
            },
            span: Span {
                first: t.first_seen,
                secs: t.at.since(t.first_seen).published().unwrap_or(0.0),
            },
            context: Context {
                far: t.seen.from_door,
                passing: t.seen.passing,
            },
        },
        after: occupancy(t.after.occupants, t.after.unexplained, true),
        facts: t
            .facts
            .iter()
            .map(|happened| {
                let (body, cause) = match happened.fact {
                    belief::Fact::Entered => (FactBody::Entered, Cause::Stabilized),
                    belief::Fact::Adopted => (FactBody::LayDown, Cause::Baseline),
                    belief::Fact::LeftRoom => (FactBody::LeftRoom, Cause::Stabilized),
                    belief::Fact::LayDown => (FactBody::LayDown, Cause::Stabilized),
                    belief::Fact::LeftBed => (FactBody::LeftBed, Cause::Stabilized),
                    belief::Fact::HeldInBed { last_seen } => {
                        (FactBody::HeldInBed { last_seen }, Cause::Stabilized)
                    }
                    belief::Fact::Reappeared { held_since } => {
                        (FactBody::Reappeared { held_since }, Cause::Stabilized)
                    }
                };
                Fact {
                    body,
                    cause,
                    after: occupancy(happened.after.occupants, happened.after.unexplained, true),
                }
            })
            .collect(),
    }
}

/// The cell of a layer-0 transition in v5: the same text as v4's.
fn cell_v5(t: &belief::Transition) -> scene_contract::Cell {
    publish_cell(&TransitionKey::of(t))
}

/// The last scene, without a new observation and declared as such (rule 2), in
/// v5: it republishes the last v5 scene, never a v4 one.
fn republish_scene(
    last: Option<(&Scene, SampleRef)>,
    reason: RepublishReason,
    sensor: SensorState,
    map: v5::MapState,
) -> Scene {
    let Some((scene, projected_at)) = last else {
        return Scene {
            frame: None,
            evidence: scene_contract::Evidence::default(),
            room: scene_contract::Room {
                mode: Mode::Empty,
                occupants: 0,
                since: None,
                pending: None,
                phase: Phase::Booting,
                last_cell: None,
            },
            bed: empty_bed(),
            floor: scene_contract::Floor { seen: 0 },
            subject: None,
            attention: scene_contract::Attention::default(),
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
        evidence: scene_contract::Evidence::default(),
        room: scene.room.clone(),
        bed: scene.bed.clone(),
        floor: scene.floor,
        subject: retained_subject_v5(scene.subject.as_ref(), projected_at),
        attention: scene_contract::Attention::default(),
        map,
        sensor,
        republished: Some(Republished {
            reason,
            observed_at: republished_at(scene.republished, projected_at),
        }),
    }
}

/// The sensor events of a reception change, with the occupancy of the last
/// scene: v5's.
fn sensor_events(changed: Option<SensorChanged>, previous: Option<&Scene>) -> Vec<Event> {
    let after = previous.map_or_else(empty_occupancy, published);
    changed
        .into_iter()
        .map(|fact| {
            Event::Sensor(v5::SensorEvent {
                cause: Cause::SensorReported,
                at: fact.at,
                body: v5::SensorBody::SensorChanged {
                    from: fact.from,
                    to: fact.to,
                },
                after,
            })
        })
        .collect()
}

/// The occupancy a published v5 scene states.
fn published(scene: &Scene) -> Occupancy {
    let held = u32::try_from(scene.bed.held.len()).unwrap_or(u32::MAX);
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

fn empty_occupancy() -> Occupancy {
    Occupancy {
        rbh: Rbh {
            room: 0,
            bed: 0,
            held: 0,
        },
        mode: Mode::Empty,
        bed: scene_contract::BedState::Empty,
        unexplained: 0,
    }
}

/// The subject of the last scene, kept while the turn carries no observation
/// of it: the same rule as v4's, on the v5 subject.
fn retained_subject_v5(
    subject: Option<&v5::Subject>,
    _projected_at: SampleRef,
) -> Option<v5::Subject> {
    subject.cloned()
}

/// The v5 projection of a republished turn: the republished scene and the
/// sensor events, from the last v5 scene.
pub(crate) fn republished(
    reason: RepublishReason,
    changed: Option<SensorChanged>,
    sensor: SensorState,
    map: crate::map::MapState,
    last: Option<(&Scene, SampleRef)>,
) -> Projection {
    let map = map_v5(&map);
    let previous = last.map(|(scene, _)| scene);
    Projection {
        scene: Arc::new(republish_scene(last, reason, sensor, map)),
        events: sensor_events(changed, previous),
    }
}

/// The parts' change in v5: the same cell, places and seen as v4, from the
/// common parts projection (`project::parts`), with the same shape.
fn parts_change(
    t: &crate::belief::parts::PartsTransition,
    since: Option<SampleRef>,
) -> scene_contract::PartsChange {
    scene_contract::PartsChange {
        at: t.at,
        cell: parts::cell(t),
        before: PartsBefore {
            places: parts::places(&t.before),
            since,
            lasted_secs: since.and_then(|since| t.at.since(since).published()),
        },
        seen: PartsSeen {
            parts: parts::seen(&t.seen),
            span: scene_contract::Span {
                first: t.first_seen,
                secs: t.at.since(t.first_seen).published().unwrap_or(0.0),
            },
        },
        after: PartsAfter {
            places: parts::places(&t.after),
        },
    }
}
