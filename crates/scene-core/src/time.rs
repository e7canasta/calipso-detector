//! Domain time: the sample, its session and its source time. It is the core's
//! only clock (rule 9). Shared vocabulary: the domain decides with it and
//! `contract` publishes it as is.

pub use scene_contract::{SampleRef, SessionId, SourceSecs};

/// Source seconds since an earlier sample. `None` across sessions: source time
/// is not compared between sessions (rule 9).
pub(crate) fn secs_since(at: SampleRef, earlier: SampleRef) -> Option<f64> {
    (at.session == earlier.session).then_some(at.source_secs.0 - earlier.source_secs.0)
}
