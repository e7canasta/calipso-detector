//! Contract v4: `calipso.scene.v4` and `calipso.events.v4`.
//!
//! The layer-0 transition is its occupancy event and the scene states the
//! machine's mode. What no version changes comes from `common`. Like every
//! version, it is defined once and its JSON Schema comes from these types
//! (rule 13).

pub use super::common::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which schema a v4 message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SchemaId {
    /// One scene per sample.
    #[serde(rename = "calipso.scene.v4")]
    SceneV4,
    /// Every event, in order.
    #[serde(rename = "calipso.events.v4")]
    EventsV4,
}

/// The envelope of every v4 message: the usual one, with v4's `SchemaId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope<T> {
    /// What the body is: a scene or an event.
    pub schema: SchemaId,
    /// Who published it.
    pub source: Source,
    /// Contiguous from 1 per run and per schema; a gap is a lost message.
    pub seq: u64,
    /// Source time of what is claimed: without a new sample, that of the last
    /// observed one.
    pub observed_at: SampleRef,
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
    /// Scenes only: the `seq` of the last event published before this scene,
    /// so a consumer knows which events the scene already reflects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_seq: Option<u64>,
    /// The scene or the event.
    pub body: T,
}

/// The mode of the layer-0 machine: the name of its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Nobody in the room.
    Empty,
    /// One occupant, off the bed.
    AloneOut,
    /// One occupant in bed, seen in the stable count.
    AloneInBedSeen,
    /// One occupant in bed, retained.
    AloneInBedHeld,
    /// More than one occupant.
    Accompanied,
}

/// The phase of rule 6: until the first climb into bed, whatever appears in
/// it is adopted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// The bed has no history yet: whoever is clearly seen in it is adopted.
    Booting,
    /// Someone climbed into bed from the floor: the bed only rises from it.
    Running,
}

/// The triple `[R, B, H]`: occupants of the room, of the bed (seen and
/// retained) and retained. They are counts: `0 ≤ H ≤ B ≤ R`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rbh(pub u32, pub u32, pub u32);

/// The pair `[F, V]`: admitted bodies seen on the floor and in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Fv(pub u32, pub u32);

/// The occupancy: a single shape for all of v4 (before, after, each fact).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Occupancy {
    /// Occupants of the room, of the bed and retained in it.
    pub rbh: Rbh,
    /// The machine's state those counts name.
    pub mode: Mode,
    /// `unknown` if the geometry does not apply, even with counts.
    pub bed: BedState,
    /// Bodies in the bed that are not occupants (rule 6).
    pub unexplained: u32,
}

/// What we believed, since when and for how long.
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

/// The span that held: from which sample and for how many source seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    /// The first sample that saw the new count.
    pub first: SampleRef,
    /// Source seconds it held before counting.
    pub secs: f64,
}

/// What happened during the span and decided the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Context {
    /// How many appeared far from the bed.
    pub far: u32,
    /// How many appeared in passing.
    pub passing: u32,
}

/// What we saw: the count that held, the span and its context.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Seen {
    /// The bodies seen on the floor and in bed, as they held.
    pub fv: Fv,
    /// How long they held before counting.
    pub span: Span,
    /// What happened meanwhile, which decided the cell.
    pub context: Context,
}

/// A layer-0 fact: what happened to someone, in the order it happened.
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

/// The cell of a transition: `before:seen:after`, the natural key of the row.
/// Each part is written as decimal digits if all its counts are below
/// 10; otherwise as a tuple (`(12,1,1)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Cell(
    #[schemars(regex(
        pattern = r"^(\d{3}|\(\d+,\d+,\d+\)):(\d{2}|\(\d+,\d+\)):(\d{3}|\(\d+,\d+,\d+\))$"
    ))]
    String,
);

impl Cell {
    /// The cell of a change from `before`, seeing `seen`, to `after`.
    pub fn new(before: Rbh, seen: Fv, after: Rbh) -> Self {
        let Rbh(r0, b0, h0) = before;
        let Fv(f, v) = seen;
        let Rbh(r1, b1, h1) = after;
        Self(format!(
            "{}:{}:{}",
            part(&[r0, b0, h0]),
            part(&[f, v]),
            part(&[r1, b1, h1])
        ))
    }

    /// As published: `before:seen:after`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn part(counts: &[u32]) -> String {
    let digits: Vec<String> = counts.iter().map(u32::to_string).collect();
    if counts.iter().all(|&count| count < 10) {
        digits.concat()
    } else {
        format!("({})", digits.join(","))
    }
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
    /// The occupancy we believe at the end of the turn. Its occupants
    /// (`rbh`, `mode`, `bed`) are those of the last fact; `unexplained` is the
    /// scene's and may differ, because a body that is nobody comes and goes
    /// without a fact (rule 6). A transition with no facts still has it.
    pub after: Occupancy,
    /// What happened to each person, in order.
    pub facts: Vec<Fact>,
}

/// A change in what was published, computed by comparing turns.
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

/// What the map learned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapBody {
    /// The map learned the depth of the empty bed.
    BedCalibrated {
        /// The empty bed's depth, on the depth model's scale.
        reference_m: f32,
    },
}

/// A change in what was published, with the occupancy at the end of the turn.
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

/// A reception change, reported by the shell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SensorEvent {
    /// Always `sensor_reported`.
    pub cause: Cause,
    /// The sample the shell reported it with.
    pub at: SampleRef,
    /// What changed.
    #[serde(flatten)]
    pub body: SensorBody,
    /// The occupancy, which a reception change does not touch.
    pub after: Occupancy,
}

/// What the map learned: not an occupancy fact.
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

/// A message of `events.v4`. Within a turn they come out in order: `sensor`,
/// `transition`, `map`, `derived`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A layer-0 change, with its facts.
    Transition(Transition),
    /// A change in what was published, by comparing turns.
    Derived(DerivedEvent),
    /// A reception change.
    Sensor(SensorEvent),
    /// What the map learned.
    Map(MapEvent),
}

impl Event {
    /// The fact's sample: the `observed_at` of its envelope.
    pub fn at(&self) -> SampleRef {
        match self {
            Self::Transition(e) => e.at,
            Self::Derived(e) => e.at,
            Self::Sensor(e) => e.at,
            Self::Map(e) => e.at,
        }
    }
}

/// The room: the machine's mode, the phase and the last cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Room {
    /// The machine's state.
    pub mode: Mode,
    /// How many people are in the room, seen or retained.
    pub occupants: u32,
    /// Since when the mode holds.
    pub since: Option<SampleRef>,
    /// A new count that has not held long enough yet.
    pub pending: Option<Pending>,
    /// Whether the bed has history yet (rule 6).
    pub phase: Phase,
    /// The cell of the last transition; `None` before the first.
    pub last_cell: Option<Cell>,
}

/// The scene of one sample.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_is_written_in_digits_if_all_counts_are_below_ten() {
        let cell = Cell::new(Rbh(2, 1, 0), Fv(0, 0), Rbh(1, 1, 1));
        assert_eq!(cell.as_str(), "210:00:111");
    }

    #[test]
    fn a_part_with_two_digit_counts_is_written_as_a_tuple() {
        let cell = Cell::new(Rbh(12, 0, 0), Fv(13, 0), Rbh(13, 0, 0));
        assert_eq!(cell.as_str(), "(12,0,0):(13,0):(13,0,0)");
        let mixed = Cell::new(Rbh(9, 1, 1), Fv(10, 0), Rbh(10, 1, 1));
        assert_eq!(mixed.as_str(), "911:(10,0):(10,1,1)");
    }
}
