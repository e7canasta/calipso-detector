//! The zones of the room and the body count per zone. Shared vocabulary: the
//! domain decides with it and `contract` publishes it as is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The zones of the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Zone {
    /// The delimited bed.
    Bed,
    /// The rest of the room.
    Floor,
}

/// What is seen in each zone, stabilized or yet to stabilize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Counts {
    /// Seen off the bed.
    pub floor: u32,
    /// Seen in the bed.
    pub bed: u32,
}
