//! The source port: where samples come from and what happens to the transport.
//! Live, RTSP with decode; in a scene, the spans.
pub mod rtsp;
pub mod scene;

use ingest::{IngestError, StreamEvent};
use scene_core::contract::SampleRef;
use std::time::{Duration, Instant};

/// What arrives from a source.
#[derive(Debug)]
pub enum Arrival<F> {
    /// A sample with its frame, or why there is none.
    Sample {
        at: SampleRef,
        /// When the transport received it.
        received: Instant,
        /// When preparing the frame started (before decoding).
        started: Instant,
        frame: Result<Option<F>, IngestError>,
    },
    /// A transport change.
    Transport(StreamEvent),
    /// Nothing arrived while waiting.
    Nothing,
    /// The source ended.
    End,
}

/// Where samples come from.
pub trait SampleSource {
    type Frame;
    /// The next thing that arrives, waiting at most `wait`.
    fn next(&mut self, wait: Duration) -> Arrival<Self::Frame>;
}
