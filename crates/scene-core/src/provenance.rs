//! Provenance: where a claim comes from (rule 2). Nothing is published as
//! current without an observation in this sample; what is retained says since
//! when.

use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a claim comes from (rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Provenance {
    /// Observed in this sample.
    Current,
    /// Observed earlier and retained without a current observation.
    Retained { since: SampleRef },
    /// Computed earlier and still valid because the subject did not move.
    Stable { since: SampleRef },
}
