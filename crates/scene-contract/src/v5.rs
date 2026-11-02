//! The wire of `scene.v5` and `events.v5`: only the types that the five changes of
//! ADR "Lo que espera el contrato v5" touch are defined here. Everything else is
//! re-exported from the crate root, so v5 shares it with v4 and does not copy it.
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
    Attention, Bed, Evidence, Floor, FrameInfo, Geometry, Profile, Provenance, Republished, Room,
    SampleRef, SensorState, Source, Timing,
};
use scene_geom::{Quad, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// The subject: as in v4, without `verdict` (change 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Subject {
    /// The subject's box, in image pixels.
    pub bbox: Rect,
    /// The subject's depth, when measured.
    pub depth: Option<SubjectDepth>,
}

/// The empty bed's calibration: as in v4, with the depth named for its scale
/// (change 4).
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

/// The envelope of every v5 message: as v4's, with `observed_at` optional
/// (change 2) and v5's `SchemaId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope<T> {
    /// What the body is: a scene or an event.
    pub schema: SchemaId,
    /// Who published it.
    pub source: Source,
    /// Contiguous from 1 per run and per schema; a gap is a lost message.
    pub seq: u64,
    /// Source time of what is claimed. `None` for a message that observes no
    /// sample (a reception change).
    #[serde(skip_serializing_if = "Option::is_none")]
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

/// Which schema a v5 message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub enum SchemaId {
    /// One scene per sample.
    #[serde(rename = "calipso.scene.v5")]
    Scene,
    /// Every event, in order.
    #[serde(rename = "calipso.events.v5")]
    Events,
}

/// The map's state, as v5 publishes it: as in v4, with `calibration` named for
/// its scale (change 4).
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
    /// What the map learned about the empty bed.
    pub calibration: Calibration,
}

/// The scene of a sample, as v5 publishes it: as in v4 except for the subject
/// and the map, which name the changed types (changes 3 and 4).
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
