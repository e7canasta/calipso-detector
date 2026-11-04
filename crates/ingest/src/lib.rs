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

use std::num::NonZeroU32;
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
    Keyframe { data: Vec<u8>, time: MediaTime },
    /// Picture already decoded by the ingest thread.
    Picture(Picture),
}

/// The source time of an access unit: `ticks` of a clock at `clock_rate` Hz.
/// The rate cannot be zero: the type refuses the invalid clock that used to
/// be silently read as 1 Hz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaTime {
    pub ticks: i64,
    pub clock_rate: NonZeroU32,
}

impl MediaTime {
    /// The time in seconds of the source clock.
    pub fn as_secs(self) -> f64 {
        self.ticks as f64 / f64::from(self.clock_rate.get())
    }
}

/// The connection a sample came from. A new session is numbered per
/// reconnection; the sequence never resets across them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Session(pub u64);

impl std::fmt::Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Access-unit number within the whole run of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(pub u64);

impl std::fmt::Display for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The timestamp of a frame on the source clock, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SourceSecs(pub f64);

impl std::fmt::Display for SourceSecs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match f.precision() {
            Some(precision) => write!(f, "{:.*}", precision, self.0),
            None => self.0.fmt(f),
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Source clock and access-unit number; sequence never resets on reconnection.
pub struct Stamp {
    pub session: Session,
    pub sequence: Sequence,
    pub source_secs: SourceSecs,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum StreamEvent {
    Connecting,
    Streaming {
        session: Session,
    },
    Disconnected {
        session: Session,
        reason: IngestError,
    },
}

/// The credentials of an RTSP server that asks for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Which access units become samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    /// Only self-contained keyframes, decoded by the consumer.
    KeyframesOnly,
    /// Every frame, decoded by the ingest thread.
    Continuous,
}

/// Configuration of the RTSP receiver, independent of the detector.
/// It is built with [`IngestConfig::new`], which validates the URL once:
/// there is no invalid configuration to fall back from.
#[derive(Debug, Clone)]
pub struct IngestConfig {
    pub url: url::Url,
    pub credentials: Option<Credentials>,
    pub sampling: Sampling,
    /// The first wait between attempts; see the reconnect backoff.
    pub reconnect_delay: Duration,
}

impl IngestConfig {
    /// Builds a configuration around an RTSP URL, validating it here instead
    /// of per session inside the pipeline.
    pub fn new(
        url: &str,
        credentials: Option<Credentials>,
        sampling: Sampling,
        reconnect_delay: Duration,
    ) -> Result<Self, IngestError> {
        let url = url
            .parse()
            .map_err(|error| IngestError::Protocol(format!("invalid RTSP URL: {error}")))?;
        Ok(Self {
            url,
            credentials,
            sampling,
            reconnect_delay,
        })
    }
}
