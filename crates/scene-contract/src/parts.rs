//! The occupancy of the subject's parts: the second trama of occupancy and
//! its changes, with the row notation of its table.

use crate::{Cell, Progress, SampleRef, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// New places of the parts, waiting to hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsPending {
    /// The places waiting: `bed IN.OUT : headboard IN.OUT : BFK`.
    #[schemars(regex(pattern = PLACES_RE))]
    pub places: String,
    /// Source seconds they have held, against the required dwell.
    pub duration: Progress<f64>,
}

// --- moved from scene-core/src/body.rs
// --- moved from scene-core/src/reading.rs
/// What we believe of the resident from the places of their parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PartsReading {
    /// Neither the box nor the groin in the bed.
    OutOfBed,
    /// The groin in the bed, the box out: getting in or out.
    OnTheEdge,
    /// The shoulders and the face or the head at the headboard, nothing out.
    Lying,
    /// Something out of the bed while some of the torso is still at the
    /// headboard: leaving from lying.
    TryingToLeave,
    /// Everything in the bed, the whole boxes in it, nothing at the headboard.
    SittingUp,
    /// Not everything in the bed, nothing at the headboard.
    SittingInBed,
    /// The places match no rule.
    Undecided,
}

// --- the trama's DSL and the rows' notation, one pattern per concept
// (`scene-core`'s `project/parts.rs` writes them; that grammar is the
// reference). The patterns are part of the published schema (rule 13).
/// The places of the trama, without HELD: `bed IN.OUT : headboard IN.OUT :
/// BFK`, the positions `cfhsaip` · `fhs` · `BFK`, two bits each.
const PLACES_RE: &str = r"^[01]{7}\.[01]{7}:[01]{3}\.[01]{3}:[01]{3}$";

/// What a sample saw of the trama: the places, with `-` where the sense did
/// not look.
const TRAMA_RE: &str = r"^[01-]{7}\.[01-]{7}:[01-]{3}\.[01-]{3}:[01-]{3}$";

/// The places believed, with HELD: `bed IN.OUT.HELD : headboard IN.OUT.HELD :
/// BFK.HELD`.
const STATE_RE: &str = r"^[01]{7}\.[01]{7}\.[01]{7}:[01]{3}\.[01]{3}\.[01]{3}:[01]{3}\.[01]{3}$";

// --- moved from scene-core/src/common.rs
/// The occupancy of the subject's parts: the second trama of occupancy, the
/// complement of layer 0 with its shape. In its notation: the positions
/// `cfhsaip` against the bed, `fhs` against the headboard and the flags `BFK`
/// (whole boxes inside), two bits each (IN, OUT; both: split); `-` where the
/// sense did not look; HELD where not seen now, kept from where it was last
/// seen (white book, volume I, ch. 09).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Parts {
    /// The places believed: `bed IN.OUT.HELD : headboard IN.OUT.HELD :
    /// BFK.HELD`. E.g. `1011111.0000000.0011111:010.001.011:100.000`.
    #[schemars(regex(pattern = STATE_RE))]
    pub state: String,
    /// What this sample saw: `bed IN.OUT : headboard IN.OUT : BFK`. E.g.
    /// `1-11111.0-00000:-10.-01:1--`.
    #[schemars(regex(pattern = TRAMA_RE))]
    pub seen: String,
    /// Since when the stable places hold: the sample of their last change;
    /// `None` before the first.
    pub since: Option<SampleRef>,
    /// New places that have not held long enough yet.
    pub pending: Option<PartsPending>,
    /// The cell of the last change: `before | seen | after`, the places
    /// without HELD.
    pub last_cell: Option<Cell>,
    /// What we believe of the resident from the places: the semantic reading
    /// of the trama (white book, volume I, ch. 10).
    pub reading: PartsReading,
    /// Since when the reading holds.
    pub reading_since: SampleRef,
}

// --- shared by every version of the contract
/// A change of the parts' places: the second trama of occupancy, the
/// complement of a layer-0 transition with its shape (white book, volume I,
/// ch. 09). The places are written `bed IN.OUT : headboard IN.OUT : BFK`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsChange {
    /// The sample in which the new places held.
    pub at: SampleRef,
    /// The row this change crossed: `before | seen | after`.
    pub cell: Cell,
    /// The places we believed, and since when.
    pub before: PartsBefore,
    /// What this sample saw, and how long the new places held.
    pub seen: PartsSeen,
    /// The places we believe.
    pub after: PartsAfter,
}

/// The parts' places before a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsBefore {
    /// The places, without HELD.
    #[schemars(regex(pattern = PLACES_RE))]
    pub places: String,
    /// The sample of the previous change; `None` on the first.
    pub since: Option<SampleRef>,
    /// Source seconds since `since`; `None` without `since` or across sessions.
    pub lasted_secs: Option<f64>,
}

/// What the sample of a parts' change saw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsSeen {
    /// The trama of the sample: `-` where the sense did not look.
    #[schemars(regex(pattern = TRAMA_RE))]
    pub parts: String,
    /// The span the new places held.
    pub span: Span,
}

/// The parts' places after a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartsAfter {
    /// The places, without HELD.
    #[schemars(regex(pattern = PLACES_RE))]
    pub places: String,
}
