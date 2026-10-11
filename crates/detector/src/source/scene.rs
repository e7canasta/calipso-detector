//! The source of a golden scene (R7): it emits each sample of the scene with
//! its source time, at real pace (like a camera) or as fast as possible. Each
//! sample's frame is what each sense sees: [`Scripted`] looks at it.
//!
//! [`Scripted`]: crate::senses::Scripted
use super::{Arrival, SampleSource};
use golden::language::Scene;
use golden::world::Sample;
use ingest::StreamEvent;
use scene_core::contract::{SampleRef, SessionId, SourceSecs};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// The session of a scene's source: a single one, without reconnections (for now).
const SESSION: u64 = 1;

/// At what pace samples are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// Like a camera: sample `n` arrives at `n × cadence` seconds.
    Real,
    /// Without waiting: for CI and for writing JSONL.
    Fast,
}

#[derive(Debug)]
pub struct SceneSource {
    samples: VecDeque<Sample>,
    pace: Pace,
    start: Instant,
    /// The source time of the first sample: the real clock starts there.
    origin_s: f64,
    connected: bool,
}

impl SceneSource {
    pub fn new(scene: &Scene, pace: Pace) -> Self {
        let samples: VecDeque<Sample> = scene.samples().into();
        let origin_s = samples.front().map_or(0.0, |m| m.source_secs);
        Self {
            samples,
            pace,
            start: Instant::now(),
            origin_s,
            connected: false,
        }
    }
}

impl SampleSource for SceneSource {
    type Frame = Sample;

    /// First the transport connects, like a camera; then, the samples.
    fn next(&mut self, wait: Duration) -> Arrival<Sample> {
        if !self.connected {
            self.connected = true;
            return Arrival::Transport(StreamEvent::Streaming { session: SESSION });
        }
        let Some(upcoming) = self.samples.front() else {
            return Arrival::End;
        };
        if self.pace == Pace::Real {
            let when = self.start
                + Duration::from_secs_f64((upcoming.source_secs - self.origin_s).max(0.0));
            if let Some(rest) = when.checked_duration_since(Instant::now()) {
                std::thread::sleep(rest.min(wait));
                return Arrival::Nothing;
            }
        }
        let Some(sample) = self.samples.pop_front() else {
            return Arrival::End;
        };
        let now = Instant::now();
        Arrival::Sample {
            at: SampleRef {
                session: SessionId(SESSION),
                sequence: u64::from(sample.n),
                source_secs: SourceSecs(sample.source_secs),
            },
            received: now,
            started: now,
            frame: Ok(Some(sample)),
        }
    }
}
