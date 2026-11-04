//! Source time and the shell's measured milliseconds: the domain's only
//! clock (rule 9), and the wall-clock numbers that never decide (rule 9).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// --- moved from scene-core/src/time.rs
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

/// How long the shell took with a sample, in wall-clock milliseconds. It is
/// measured by the shell and never decides anything (rule 9).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Timing {
    /// From the sample's arrival to the start of its turn.
    pub wait_ms: f64,
    /// The whole turn: decoding, every look and the projection.
    pub processing_ms: f64,
}

/// A quantity reached against the one required.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Progress<T> {
    /// What has been reached.
    pub have: T,
    /// What is required.
    pub need: T,
}

/// The span that held: from which sample and for how many source seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    /// The first sample that saw the new count.
    pub first: SampleRef,
    /// Source seconds it held before counting.
    pub secs: f64,
}
