//! Contract v4: `calipso.scene.v4` and `calipso.events.v4` (spec 1301).
//!
//! It is published next to v3, from the same turn (the contract v4 ADR). The
//! layer-0 transition is its occupancy event and the scene states the
//! machine's mode. What no version changes comes from `common`: v4 does not
//! depend on v3. Like every version, it is defined once and its JSON Schema
//! comes from these types (rule 13).

pub use super::common::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which schema a v4 message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SchemaId {
    #[serde(rename = "calipso.scene.v4")]
    SceneV4,
    #[serde(rename = "calipso.events.v4")]
    EventsV4,
}

/// The envelope of every v4 message: the usual one, with v4's `SchemaId` (D9-3).
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

/// The mode of the layer-0 machine: the name of its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
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
    Booting,
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
    pub rbh: Rbh,
    pub mode: Mode,
    /// `unknown` if the geometry does not apply, even with counts.
    pub bed: BedState,
    /// Bodies in the bed that are not occupants (rule 6).
    pub unexplained: u32,
}

/// What we believed, since when and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Before {
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
    pub first: SampleRef,
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
    pub fv: Fv,
    pub span: Span,
    pub context: Context,
}

/// A layer-0 fact. The names and the data are those of v3.
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
    HeldInBed { last_seen: SampleRef },
    /// Someone retained in bed was seen again.
    Reappeared { held_since: SampleRef },
}

/// A fact within its transition, with the occupancy it leaves.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fact {
    #[serde(flatten)]
    pub body: FactBody,
    pub cause: Cause,
    pub after: Occupancy,
}

/// The cell of a transition: `before:seen:after`, the natural key of the row
/// (D2). Each part is written as decimal digits if all its counts are below
/// 10; otherwise as a tuple (`(12,1,1)`), as in book 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Cell(
    #[schemars(regex(
        pattern = r"^(\d{3}|\(\d+,\d+,\d+\)):(\d{2}|\(\d+,\d+\)):(\d{3}|\(\d+,\d+,\d+\))$"
    ))]
    String,
);

impl Cell {
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
/// facts, in order (D9-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transition {
    pub at: SampleRef,
    pub cell: Cell,
    /// The phase it was decided with.
    pub phase: Phase,
    pub before: Before,
    pub seen: Seen,
    /// The occupancy we believe: that of the last fact.
    pub after: Occupancy,
    pub facts: Vec<Fact>,
}

/// A change in what was published, computed by comparing turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivedBody {
    /// The mode changed (in v3, the state).
    RoomChanged { from: Mode, to: Mode },
    /// The bed changed; it goes to `unknown` without a transition.
    BedChanged { from: BedState, to: BedState },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SensorBody {
    SensorChanged { from: SensorState, to: SensorState },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapBody {
    /// The map learned the depth of the empty bed.
    BedCalibrated { reference_m: f32 },
}

/// A change in what was published, with the occupancy at the end of the turn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DerivedEvent {
    pub cause: Cause,
    pub at: SampleRef,
    #[serde(flatten)]
    pub body: DerivedBody,
    pub after: Occupancy,
}

/// A reception change, reported by the shell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SensorEvent {
    pub cause: Cause,
    pub at: SampleRef,
    #[serde(flatten)]
    pub body: SensorBody,
    pub after: Occupancy,
}

/// What the map learned: not an occupancy fact.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapEvent {
    pub cause: Cause,
    pub at: SampleRef,
    #[serde(flatten)]
    pub body: MapBody,
}

/// A message of `events.v4`. Within a turn they come out in order: `sensor`,
/// `transition`, `map`, `derived`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Transition(Transition),
    Derived(DerivedEvent),
    Sensor(SensorEvent),
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
    pub mode: Mode,
    pub occupants: u32,
    /// Since when the mode holds.
    pub since: Option<SampleRef>,
    pub pending: Option<Pending>,
    pub phase: Phase,
    /// The cell of the last transition; `None` before the first.
    pub last_cell: Option<Cell>,
}

/// The scene of one sample: v3's, with v4's room.
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
