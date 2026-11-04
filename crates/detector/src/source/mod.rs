//! The source port: where samples come from and what happens to the transport.
//! Live, RTSP with decode; for the loop test, frames from a directory.
mod frames;
mod rotate;
mod rtsp;

pub use frames::FramesSource;
pub use rotate::{Rotation, Turned};
pub use rtsp::{RtspSource, SampleDecoder};

use ingest::{IngestError, StreamEvent};
use scene_contract::SampleRef;
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
