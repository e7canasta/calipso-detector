//! Domain time: the sample, its session and its source time. It is the core's
//! only clock (rule 9). Shared vocabulary: the domain decides with it and
//! `contract` publishes it as is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Source session. Source time is only comparable within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SessionId(pub u64);

/// Seconds of source time (video PTS) within a session.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
pub struct SourceSecs(pub f64);

/// Which sample and when, in source time. It is the domain's only clock
/// (rule 9).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleRef {
    /// The stream session: a reconnection starts a new one.
    pub session: SessionId,
    /// The sample's number, increasing within the run.
    pub sequence: u64,
    /// The sample's presentation time, comparable only within its session.
    pub source_secs: SourceSecs,
}

impl SampleRef {
    /// Source seconds since an earlier sample. `None` across sessions: source
    /// time is not compared between sessions (rule 9).
    pub(crate) fn secs_since(self, earlier: SampleRef) -> Option<f64> {
        (self.session == earlier.session).then_some(self.source_secs.0 - earlier.source_secs.0)
    }
}
