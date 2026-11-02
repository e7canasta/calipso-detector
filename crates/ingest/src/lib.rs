//! Stream stage: RTSP reception, sample filtering and on-demand decoding.
//!
//! Policy: the freshest sample wins. Nothing is queued; whatever the engine
//! does not take in time is replaced, and pixels are produced only for
//! samples the engine is going to analyse.
// What is `pub` is reachable from outside; the rest says `pub(crate)`.
#![warn(unreachable_pub)]

mod clock;
mod decode;
mod error;
mod mailbox;
mod rtsp;

use std::time::{Duration, Instant};

// The facade: each item by one path from this root.
pub use clock::Clock;
pub use decode::{FrameDecoder, Picture};
pub use error::IngestError;
pub use mailbox::{Mailbox, Take};
pub use rtsp::spawn;

#[derive(Debug)]
pub struct Sample {
    pub stamp: Stamp,
    pub received_at: Instant,
    pub payload: Payload,
}

#[derive(Debug)]
pub enum Payload {
    /// Self-contained keyframe access unit, decoded by the consumer.
    Keyframe {
        data: Vec<u8>,
        pts_ticks: i64,
        clock_rate: u32,
    },
    /// Picture already decoded by the ingest thread.
    Picture(Picture),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum StreamEvent {
    Connecting,
    Streaming { session: u64 },
    Disconnected { session: u64, reason: IngestError },
}

/// Configuration of the RTSP receiver, independent of the detector.
#[derive(Debug, Clone)]
pub struct IngestConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keyframe_only: bool,
    pub reconnect_delay: Duration,
}

#[derive(Debug, Clone, Copy)]
/// Source clock and access-unit number; sequence never resets on reconnection.
pub struct Stamp {
    pub session: u64,
    pub sequence: u64,
    pub source_secs: f64,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: None,
            password: None,
            keyframe_only: true,
            reconnect_delay: Duration::from_secs(5),
        }
    }
}
