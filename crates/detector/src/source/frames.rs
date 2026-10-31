//! Frames from a directory, without a camera and without waiting: each image
//! is a sample, in order, and none is dropped. The detector over a clip at
//! its own pace, so that two builds over the same frames are compared frame
//! by frame, whatever the machine's load (white book, volume VII, ch. 12).
//!
//! Each file is named by its source second (`12.jpg`, `12.500.png`); files
//! whose name is not a number are ignored.
use super::{Arrival, SampleSource};
use image::DynamicImage;
use ingest::{IngestError, StreamEvent};
use scene_core::contract::{SampleRef, SessionId, SourceSecs};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A single session: the clip, from its first frame to its last.
const SESSION: u64 = 1;

/// The frames of a directory, in source-time order.
#[derive(Debug)]
pub struct FramesSource {
    frames: VecDeque<(f64, PathBuf)>,
    sequence: u64,
    connected: bool,
}

impl FramesSource {
    /// The directory's images named by their source second.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        let mut frames: Vec<(f64, PathBuf)> = std::fs::read_dir(dir)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter_map(|path| {
                let secs = path.file_stem()?.to_str()?.parse::<f64>().ok()?;
                Some((secs, path))
            })
            .collect();
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(Self {
            frames: frames.into(),
            sequence: 0,
            connected: false,
        })
    }

    /// How many frames are left.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

impl SampleSource for FramesSource {
    type Frame = DynamicImage;

    /// First the transport connects, like a camera; then, each frame.
    fn next(&mut self, _wait: Duration) -> Arrival<DynamicImage> {
        if !self.connected {
            self.connected = true;
            return Arrival::Transport(StreamEvent::Streaming { session: SESSION });
        }
        let Some((secs, path)) = self.frames.pop_front() else {
            return Arrival::End;
        };
        self.sequence += 1;
        let now = Instant::now();
        let frame = image::open(&path)
            .map(Some)
            .map_err(|e| IngestError::Decode(format!("{}: {e}", path.display())));
        Arrival::Sample {
            at: SampleRef {
                session: SessionId(SESSION),
                sequence: self.sequence,
                source_secs: SourceSecs(secs),
            },
            received: now,
            started: now,
            frame,
        }
    }
}
