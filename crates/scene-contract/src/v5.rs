//! The wire of `scene.v5` and `events.v5`: only the types that the five changes of
//! ADR "Lo que espera el contrato v5" touch are defined here. Everything else is
//! re-exported from the crate root, where the shared vocabulary lives.
//!
//! The changes, and where each one lands:
//! 1. `last_event_seq` is written by the numbering, not by the transport: the
//!    field keeps its type; the shell writes it differently (project 41, V2).
//! 2. `observed_at` is optional in the envelope ([`Envelope`]): a reception
//!    change observes no sample.
//! 3. `subject.verdict` is not in v5 ([`Subject`] has no `verdict`).
//! 4. The depth is named for what it is: [`SubjectDepth::relative`] and
//!    [`Calibration::reference`], on the depth model's relative scale.
//! 5. `rbh` and `fv` have named fields: [`Rbh`] and [`Fv`].

use crate::{
    Attention, Bed, BedPostureChange, BedState, Cause, Cell, Counts, Evidence, Floor, FrameInfo,
    Geometry, Mode, PartsChange, Phase, Profile, Provenance, Republished, Room, SampleRef,
    SensorState, Source, Timing,
};
use scene_geom::{Quad, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The span that held: from which sample and for how many source seconds.
/// Declared once, in the crate's root: this is that [`Span`](crate::Span).
pub use crate::Span;

/// The triple of occupants: `room` is the total in the room (the bed's and the
/// floor's), `bed` the seen and retained in bed, `held` the retained in bed.
/// Counts: `0 ≤ held ≤ bed ≤ room` (book 3: RBH, total, bed, held).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rbh {
    /// Occupants of the room in total.
    pub room: u32,
    /// Occupants in the bed: seen and retained.
    pub bed: u32,
    /// Occupants retained in the bed.
    pub held: u32,
}

/// The pair of bodies seen: on the `floor` and in the `bed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Fv {
    /// Admitted bodies seen on the floor.
    pub floor: u32,
    /// Admitted bodies seen in the bed.
    pub bed: u32,
}

/// `Fv` is the pair of seen bodies as the layer-0 table names it (FV);
/// [`Counts`] is the same pair as the zones' count. One concept, two names:
/// the fields on the wire (`"fv"`, `"counts"`) keep each name.
impl From<Fv> for Counts {
    fn from(fv: Fv) -> Self {
        Self {
            floor: fv.floor,
            bed: fv.bed,
        }
    }
}

/// [`Counts`] is the same pair as `Fv`: the zones' count, as the layer-0 table
/// names it (FV).
impl From<Counts> for Fv {
    fn from(counts: Counts) -> Self {
        Self {
            floor: counts.floor,
            bed: counts.bed,
        }
    }
}

/// The subject's depth, on the depth model's relative scale (the unit is
/// `RelativeDepth`): the name says what the value is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubjectDepth {
    /// The median of the subject's measured points, on the depth model's
    /// relative scale.
    pub relative: f32,
    /// Measured in this sample, or stable with the pose.
    pub provenance: Provenance,
}

/// The subject of the scene. It has no `verdict` (change 3), and its `depth`
/// is the relative one (change 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Subject {
    /// Where the subject is: in bed or on the floor.
    pub zone: crate::Zone,
    /// Their box, in image pixels.
    pub bbox: Rect,
    /// The detector's score for that box.
    pub confidence: f32,
    /// Whether the box was seen in this turn or is held from before.
    pub provenance: Provenance,
    /// The subject's pose, when the policy takes it.
    pub pose: Option<crate::Pose>,
    /// The subject's depth, when the policy measures it: relative (change 4).
    pub depth: Option<SubjectDepth>,
    /// Second level: absent without a pose or without bed geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lying: Option<crate::Lying>,
    /// Second level: absent without a pose or without per-point depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<crate::Posture>,
    /// Layer 1: the bed posture of the resident left lying.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_posture: Option<crate::BedPosture>,
    /// The occupancy of the subject's parts against the bed and the headboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<crate::Parts>,
}

/// The empty bed's calibration, with the depth named for its scale (change 4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Calibration {
    /// The empty bed has not been learned.
    Unavailable,
    /// The empty bed was learned.
    Complete {
        /// Its depth, on the depth model's relative scale.
        reference: f32,
        /// The sample it was learned in; `None` if the profile brought it.
        since: Option<SampleRef>,
    },
}

/// The envelope of every v5 message. `observed_at` is optional: a reception
/// change observes no sample (change 2).
///
/// The convention lives in [`Envelope::scene`] and [`Envelope::event`]: an
/// event carries only its `event_id`, a scene only its `timing` and its
/// `last_event_seq`, and each names its `schema`. The struct is
/// `#[non_exhaustive]`: outside this crate those constructors are the only way
/// to build one, so an `Envelope<Event>` with a `timing` cannot be built
/// without forcing it (forging JSON and deserializing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Envelope<T> {
    /// What the body is: a scene or an event.
    pub schema: SchemaId,
    /// Who published it.
    pub source: Source,
    /// Contiguous from 1 per run and per schema; a gap is a lost message.
    pub seq: u64,
    /// Source time of what is claimed. `null` for a message that observes no
    /// sample (a reception change).
    pub observed_at: Option<SampleRef>,
    /// Wall clock; set by publishing, never by the core.
    pub published_at_ms: u64,
    /// The configuration the engine ran with.
    pub profile: Profile,
    /// Events only: unique per event, for deduplication on redelivery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// Scenes only: how long the shell took with the sample.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<Timing>,
    /// Scenes only: the `seq` of the last event published before this scene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_seq: Option<u64>,
    /// The scene or the event.
    pub body: T,
}

impl<T> Envelope<T> {
    /// An event envelope, as the convention demands: `schema` is
    /// `calipso.events.v5`, the `event_id` names it for deduplication and the
    /// scene's fields are absent (`timing`, `last_event_seq`).
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the envelope's fields one to one"
    )]
    pub fn event(
        source: Source,
        profile: Profile,
        seq: u64,
        observed_at: Option<SampleRef>,
        published_at_ms: u64,
        event_id: String,
        body: T,
    ) -> Self {
        Self {
            schema: SchemaId::Events,
            source,
            profile,
            seq,
            observed_at,
            published_at_ms,
            event_id: Some(event_id),
            timing: None,
            last_event_seq: None,
            body,
        }
    }

    /// A scene envelope, as the convention demands: `schema` is
    /// `calipso.scene.v5`, it carries the shell's `timing` and the numbering's
    /// `last_event_seq`, and no `event_id`.
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the envelope's fields one to one"
    )]
    pub fn scene(
        source: Source,
        profile: Profile,
        seq: u64,
        observed_at: Option<SampleRef>,
        published_at_ms: u64,
        timing: Timing,
        last_event_seq: u64,
        body: T,
    ) -> Self {
        Self {
            schema: SchemaId::Scene,
            source,
            profile,
            seq,
            observed_at,
            published_at_ms,
            event_id: None,
            timing: Some(timing),
            last_event_seq: Some(last_event_seq),
            body,
        }
    }
}

/// Which schema a v5 message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SchemaId {
    /// One scene per sample.
    #[serde(rename = "calipso.scene.v5")]
    Scene,
    /// Every event, in order.
    #[serde(rename = "calipso.events.v5")]
    Events,
}

/// The map's state. Its `calibration` is named for its scale (change 4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapState {
    /// Whether the delimited bed applies to this image.
    pub geometry: Geometry,
    /// The bed's four corners on the image, clockwise.
    pub bed: Quad,
    /// Where faces are looked for when nobody is in sight.
    pub headboard: Option<Rect>,
    /// The main detector's focus, when the policy has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Rect>,
    /// The geometry's shadow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<crate::MapShadow>,
    /// What the map learned about the empty bed.
    pub calibration: Calibration,
}

/// The scene of a sample. Its subject and its map name the changed types
/// (changes 3 and 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    /// The image size the turn worked on; `None` before the first image.
    pub frame: Option<FrameInfo>,
    /// What was admitted in this sample.
    pub evidence: Evidence,
    /// How many are in the room and the machine's mode.
    pub room: Room,
    /// Who is believed in bed.
    pub bed: Bed,
    /// Who is seen off the bed.
    pub floor: Floor,
    /// The room's only occupant, when there is exactly one.
    pub subject: Option<Subject>,
    /// What the core looked at in this turn, and why not the rest.
    pub attention: Attention,
    /// The bed geometry and what the map learned.
    pub map: MapState,
    /// The reception, as the shell reported it.
    pub sensor: SensorState,
    /// Present only if this scene does not come from a new observation (rule 2).
    pub republished: Option<Republished>,
}

/// The occupancy: `rbh` is split into named fields (change 5: the room in
/// total, the bed, the held in bed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Occupancy {
    /// Occupants of the room, of the bed and held in it.
    pub rbh: Rbh,
    /// The machine's state those counts name.
    pub mode: Mode,
    /// `unknown` if the geometry does not apply, even with counts.
    pub bed: BedState,
    /// Bodies in the bed that are not occupants (rule 6).
    pub unexplained: u32,
}

/// What we believed, since when and for how long. Its `occupancy` names `rbh`
/// (change 5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Before {
    /// The occupancy we believed.
    #[serde(flatten)]
    pub occupancy: Occupancy,
    /// The sample of the previous transition; `None` on the first.
    pub since: Option<SampleRef>,
    /// Source seconds since `since`; `None` without `since` or across sessions.
    pub lasted_secs: Option<f64>,
}

/// What happened during the span and decided the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Context {
    /// How many appeared far from the bed.
    pub far: u32,
    /// How many appeared in passing.
    pub passing: u32,
}

/// What we saw: the count that held, the span and its context, with v5's `Fv`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Seen {
    /// The bodies seen on the floor and in bed, as they held.
    pub fv: Fv,
    /// How long they held before counting.
    pub span: Span,
    /// What happened meanwhile, which decided the cell.
    pub context: Context,
}

/// What happened to someone.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactBody {
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
        /// The last sample they were seen in.
        last_seen: SampleRef,
    },
    /// Someone retained in bed was seen again.
    Reappeared {
        /// The sample since which they were retained.
        held_since: SampleRef,
    },
}

/// A fact within its transition, with the occupancy it leaves.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fact {
    /// What happened.
    #[serde(flatten)]
    pub body: FactBody,
    /// Why it happened.
    pub cause: Cause,
    /// The occupancy this fact leaves, before the next one.
    pub after: Occupancy,
}

/// A layer-0 change: what we believed, what we saw, what we believe and the
/// facts, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transition {
    /// The sample in which the new count held.
    pub at: SampleRef,
    /// The row of the layer-0 table this change crossed.
    pub cell: Cell,
    /// The phase it was decided with.
    pub phase: Phase,
    /// What we believed, and since when.
    pub before: Before,
    /// What we saw hold.
    pub seen: Seen,
    /// The occupancy we believe at the end of the turn.
    pub after: Occupancy,
    /// What happened to each person, in order.
    pub facts: Vec<Fact>,
}

/// A change in what was published, by comparing turns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DerivedEvent {
    /// Why it changed.
    pub cause: Cause,
    /// The sample in which it changed.
    pub at: SampleRef,
    /// What changed.
    #[serde(flatten)]
    pub body: DerivedBody,
    /// The occupancy at the end of the turn.
    pub after: Occupancy,
}

/// What changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivedBody {
    /// The mode changed.
    RoomChanged {
        /// The previous mode.
        from: Mode,
        /// The new mode.
        to: Mode,
    },
    /// The bed changed; it goes to `unknown` without a transition.
    BedChanged {
        /// The previous state.
        from: BedState,
        /// The new state.
        to: BedState,
    },
}

/// A reception change, reported by the shell, with its `after`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SensorEvent {
    /// Always `sensor_reported`.
    pub cause: Cause,
    /// The last sample the shell saw, `null` before the first one: v5 names no
    /// sample it did not see (change 2).
    pub at: Option<SampleRef>,
    /// What changed.
    #[serde(flatten)]
    pub body: SensorBody,
    /// The occupancy, which a reception change does not touch.
    pub after: Occupancy,
}

/// What changed in the reception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SensorBody {
    /// The reception changed.
    SensorChanged {
        /// The previous state.
        from: SensorState,
        /// The new state.
        to: SensorState,
    },
}

/// What the map learned: the bed's depth reference, named for its scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapEvent {
    /// Always `map_learned`.
    pub cause: Cause,
    /// The sample it was learned in.
    pub at: SampleRef,
    /// What was learned.
    #[serde(flatten)]
    pub body: MapBody,
}

/// What the map learned: the empty bed's depth, on the relative scale (change
/// 4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapBody {
    /// The map learned the depth of the empty bed.
    BedCalibrated {
        /// The empty bed's depth, on the depth model's relative scale.
        reference: f32,
    },
}

/// A message of `events.v5`, in the order the turn publishes them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A layer-0 change, with its facts.
    Transition(Transition),
    /// A change of the parts' places.
    Parts(PartsChange),
    /// A change in what was published, by comparing turns.
    Derived(DerivedEvent),
    /// A reception change.
    Sensor(SensorEvent),
    /// What the map learned.
    Map(MapEvent),
    /// A layer-1 change.
    BedPosture(BedPostureChange),
}

impl Event {
    /// The sample this event happened in; `None` for a reception change, which
    /// observes no sample (change 2). It is the only answer to «in which sample
    /// did this event happen»: the envelope's `observed_at` carries it.
    pub fn at(&self) -> Option<SampleRef> {
        match self {
            Self::Transition(event) => Some(event.at),
            Self::Parts(event) => Some(event.at),
            Self::Derived(event) => Some(event.at),
            Self::Sensor(event) => event.at,
            Self::Map(event) => Some(event.at),
            Self::BedPosture(event) => Some(event.at),
        }
    }
}
