//! The natural key of a row: `before | seen | after`, in the notation of the
//! table it belongs to.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The pattern of [`Cell`]: its three notations. The parts' row restates the
/// DSL's shapes because JSON Schema patterns do not compose.
const CELL_RE: &str = r"^(\d{3}|\(\d+,\d+,\d+\)):(\d{2}|\(\d+,\d+\)):(\d{3}|\(\d+,\d+,\d+\))$|^[01]{7}\.[01]{7}:[01]{3}\.[01]{3}:[01]{3} \| [01-]{7}\.[01-]{7}:[01-]{3}\.[01-]{3}:[01-]{3} \| [01]{7}\.[01]{7}:[01]{3}\.[01]{3}:[01]{3}$|^(in_bed|lying|sitting_up|getting_out) \| (held|head_away|head_side|legs_out|headboard_lying|headboard_upright|unknown) \| (in_bed|lying|sitting_up|getting_out)$";

/// The natural key of a row: `before | seen | after`, in the notation of the
/// table it belongs to. Three notations, disjoint, so the value names its
/// table: the layer-0 transitions write counts, `before:seen:after`
/// (`210:00:111`), each part in decimal digits when all its counts are below
/// 10 and as a tuple otherwise (`(12,0,0):(13,0):(13,0,0)`); the parts write
/// their places with the sample's trama between them
/// (`0000000.0000000:000.000:000 | 0------.1------:---.---:0-- |
/// 0000000.1000000:000.000:000`); the bed posture writes the names of what it
/// believed and of what was seen (`lying | head_away | sitting_up`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Cell(#[schemars(regex(pattern = CELL_RE))] pub String);

impl Cell {
    /// As published.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A cell compares with the string it is written as.
impl PartialEq<String> for Cell {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}
