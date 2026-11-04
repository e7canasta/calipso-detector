//! Who published, and with what: the run's identity and the configuration
//! the consumer needs to compare runs.

use crate::LookKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- moved from scene-core/src/common.rs
/// Identity of the publishing shell; the core does not build these values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    /// The site the camera belongs to (a subject token).
    pub site: String,
    /// The camera within the site (a subject token).
    pub camera: String,
    /// One per process start: a new run restarts every numbering.
    pub run_id: String,
    /// The binary that published.
    pub build: Build,
}

/// The publishing binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Build {
    /// The crate version.
    pub version: String,
    /// The commit it was built from.
    pub git: String,
}

/// The configuration the engine ran with, as the consumer needs to compare runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    /// The camera layer of the profile.
    pub camera: Option<LayerRef>,
    /// The resident layer of the profile; `None` while there is none.
    pub resident: Option<LayerRef>,
    /// Two runs with the same hash believed with the same policy and map.
    pub hash: ProfileHash,
    /// What each configured sense looks with. `hash` already covers them; this
    /// names them for whoever watches, as in v2. Empty if the shell does not
    /// report them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub senses: Vec<SenseProfile>,
}

/// The model and thresholds of a sense.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SenseProfile {
    /// The look this sense answers.
    pub kind: LookKind,
    /// The model file, without its path.
    pub model: String,
    /// Below this score the backend does not return the candidate.
    pub backend_confidence: f32,
    /// Below this score the policy does not admit the candidate.
    pub min_confidence: Option<f32>,
    /// The pose only: below this score a keypoint is not seen, and the core
    /// neither measures nor reasons with it. Absent in every other sense.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_keypoint_score: Option<f32>,
}

/// A layer of the profile, by name and revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayerRef {
    /// The layer's name.
    pub id: String,
    /// Its revision; `0` while layers are not versioned.
    pub version: u32,
}

/// Hash of the effective policy the engine ran with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileHash(pub String);
