//! Contract v3: `calipso.scene.v3` and `calipso.events.v3`.
//!
//! It is defined once, here, and the JSON Schema is generated from these types
//! (rule 13). An incompatible change is a major version, with an ADR. What it
//! shares with other versions, and the domain vocabulary, come from `common`:
//! no version depends on another.

pub use super::common::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which schema a message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SchemaId {
    #[serde(rename = "calipso.scene.v3")]
    SceneV3,
    #[serde(rename = "calipso.events.v3")]
    EventsV3,
}

/// The envelope of every published message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope<T> {
    pub schema: SchemaId,
    pub source: Source,
    pub seq: u64,
    /// Source time of what is claimed: without a new sample, that of the last
    /// observed one.
    pub observed_at: SampleRef,
    /// Wall clock; set by publishing, never by the core.
    pub published_at_ms: u64,
    pub profile: Profile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<Timing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_seq: Option<u64>,
    pub body: T,
}

/// How many people are in the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomState {
    Empty,
    One,
    Several,
}

/// The room: seen and retained occupants of both zones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Room {
    pub state: RoomState,
    pub occupants: u32,
    pub since: Option<SampleRef>,
    pub pending: Option<Pending>,
}

/// The scene of one sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    pub frame: Option<FrameInfo>,
    pub evidence: Evidence,
    pub room: Room,
    pub bed: Bed,
    pub floor: Floor,
    pub subject: Option<Subject>,
    pub attention: Attention,
    pub map: MapState,
    pub sensor: SensorState,
    /// Present only if this scene does not come from a new observation (P-5).
    pub republished: Option<Republished>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Emitted by the aggregate that changed, with its cause.
    Domain,
    /// Computed by comparing projections.
    Derived,
    /// Reported by the shell (the reception state).
    Sensor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventBody {
    RoomChanged {
        from: RoomState,
        to: RoomState,
    },
    BedChanged {
        from: BedState,
        to: BedState,
    },
    /// Someone appeared on the floor: they entered the room.
    Entered,
    /// Someone stopped being seen on the floor: they left the room.
    LeftRoom,
    /// Someone appeared in bed coming from the floor; with cause `baseline`,
    /// they were found in bed at startup.
    LayDown,
    /// Someone was seen moving from the bed to the floor.
    LeftBed,
    /// Someone stopped being seen in bed without leaving: they are retained.
    HeldInBed {
        last_seen: SampleRef,
    },
    /// Someone retained in bed was seen again.
    Reappeared {
        held_since: SampleRef,
    },
    SensorChanged {
        from: SensorState,
        to: SensorState,
    },
    /// The map learned the depth of the empty bed.
    BedCalibrated {
        reference_m: f32,
    },
}

/// Occupancy after this fact, not after the whole turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct After {
    pub room: RoomState,
    pub occupants: u32,
    pub bed: BedState,
    pub bed_seen: u32,
    pub bed_held: u32,
    pub floor_seen: u32,
}

/// A published fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Event {
    pub kind: EventKind,
    pub cause: Cause,
    pub at: SampleRef,
    #[serde(flatten)]
    pub body: EventBody,
    pub after: After,
}

impl Event {
    /// An event emitted by the aggregate that changed, with its cause.
    pub fn domain(at: SampleRef, cause: Cause, body: EventBody, after: After) -> Self {
        Self {
            kind: EventKind::Domain,
            cause,
            at,
            body,
            after,
        }
    }

    /// A reception change, reported by the shell.
    pub fn sensor(at: SampleRef, from: SensorState, to: SensorState, after: After) -> Self {
        Self {
            kind: EventKind::Sensor,
            cause: Cause::SensorReported,
            at,
            body: EventBody::SensorChanged { from, to },
            after,
        }
    }
}
