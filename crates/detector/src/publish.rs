//! The handle numbers before sending; the network has its own owner. The
//! profile's turns go out as contract v5: its events, then its scene, numbered
//! on their own from 1, on their own subjects.
use scene_contract::{Profile, Source, Timing, v5};
use scene_core::Output;
use serde::Serialize;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

pub(crate) const EVENT_BUFFER: usize = 1024;
/// The channel is bounded too; the actor's fact queue has 1024 slots.
pub const CHANNEL_BUFFER: usize = 64;
pub(crate) type ScenePublication = v5::Envelope<Arc<v5::Scene>>;
pub(crate) type EventPublication = v5::Envelope<v5::Event>;

/// The contract version the shell publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V5,
}

impl Version {
    /// The last token of each subject.
    pub fn token(self) -> &'static str {
        match self {
            Self::V5 => "v5",
        }
    }
}

/// As `token` writes it: the subject's token and the display's are one.
impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.token())
    }
}

/// A numbered turn: its events and, after them, its scene.
#[derive(Debug)]
pub struct Publication {
    pub events: Vec<EventPublication>,
    pub scene: ScenePublication,
}

/// The class of a message: the scene (the last one counts) or an event (all of
/// them, each with its ID for deduplication).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Scene,
    Events,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Events => "events",
        }
    }

    /// `{prefix}.scene.v5`, `{prefix}.events.v5`.
    pub fn subject(self, prefix: &str, version: Version) -> String {
        format!("{prefix}.{}.{}", self.as_str(), version.token())
    }
}

/// As `as_str` writes it: one name for the subject and for display.
impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An envelope that knows which stream it goes to: the in-memory sink and the
/// network actor convert it the same way.
pub(crate) trait Wired: Serialize {
    const KIND: Kind;
    const VERSION: Version;
    /// The deduplication ID; only events carry it.
    fn msg_id(&self) -> Option<&str>;

    fn wire(&self) -> serde_json::Result<WireMessage> {
        Ok(WireMessage {
            kind: Self::KIND,
            version: Self::VERSION,
            bytes: serde_json::to_vec(self)?,
            msg_id: self.msg_id().map(str::to_owned),
        })
    }
}

impl Wired for EventPublication {
    const KIND: Kind = Kind::Events;
    const VERSION: Version = Version::V5;
    fn msg_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }
}

impl Wired for ScenePublication {
    const KIND: Kind = Kind::Scene;
    const VERSION: Version = Version::V5;
    fn msg_id(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug)]
pub struct WireMessage {
    pub kind: Kind,
    pub version: Version,
    pub bytes: Vec<u8>,
    pub msg_id: Option<String>,
}

impl WireMessage {
    /// The subject this message goes to, under the prefix.
    pub fn subject(&self, prefix: &str) -> String {
        self.kind.subject(prefix, self.version)
    }

    /// The header is the same in the in-memory sink and in the real transport.
    pub(crate) fn headers(&self) -> Option<async_nats::HeaderMap> {
        match self.kind {
            Kind::Events => {
                let mut headers = async_nats::HeaderMap::new();
                headers.insert(
                    async_nats::header::NATS_MESSAGE_ID,
                    self.msg_id.as_deref().unwrap_or(""),
                );
                Some(headers)
            }
            Kind::Scene => None,
        }
    }
}

impl Publication {
    /// In-memory sink: exactly the bytes and headers NATS consumes, the events
    /// before the scene.
    pub fn messages(&self) -> serde_json::Result<Vec<WireMessage>> {
        let mut messages = Vec::with_capacity(self.events.len() + 1);
        for event in &self.events {
            messages.push(event.wire()?);
        }
        messages.push(self.scene.wire()?);
        Ok(messages)
    }
}

/// The numbering of the stream: its events and its scenes, each contiguous
/// from 1.
#[derive(Debug, Default)]
struct Numbering {
    events: u64,
    scenes: u64,
}

impl Numbering {
    fn next_event(&mut self) -> u64 {
        self.events += 1;
        self.events
    }

    fn next_scene(&mut self) -> u64 {
        self.scenes += 1;
        self.scenes
    }

    /// The last event numbered before this scene.
    fn last_event_seq(&self) -> u64 {
        self.events
    }
}

#[derive(Debug)]
pub struct PublisherHandle {
    source: Source,
    profile: Profile,
    numbering: Numbering,
    tx: mpsc::Sender<Publication>,
    dropped_events: u64,
}

impl PublisherHandle {
    /// The channel to the network actor. The profile's identity goes in every
    /// envelope; the turns are numbered here.
    pub fn channel(source: Source, profile: Profile) -> (Self, mpsc::Receiver<Publication>) {
        let (tx, rx) = mpsc::channel(CHANNEL_BUFFER);
        (
            Self {
                source,
                profile,
                numbering: Numbering::default(),
                tx,
                dropped_events: 0,
            },
            rx,
        )
    }

    /// Numbers a turn and wraps it in v5 envelopes.
    pub fn prepare(&mut self, output: Output, timing: Timing) -> Publication {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let Self {
            source,
            profile,
            numbering,
            ..
        } = self;
        let projected = output.v5;
        // The event_id names the version: if a stream captures several,
        // deduplication by Nats-Msg-Id does not confuse one event with another.
        let events = projected
            .events
            .into_iter()
            .map(|event| {
                let seq = numbering.next_event();
                v5::Envelope::event(
                    source.clone(),
                    profile.clone(),
                    seq,
                    event.at(),
                    now,
                    format!("{}-{}-{seq}", source.run_id, Version::V5.token()),
                    event,
                )
            })
            .collect();
        // The scene names the last event numbered before it: the numbering's,
        // not the transport's (change 1).
        let scene = v5::Envelope::scene(
            source.clone(),
            profile.clone(),
            numbering.next_scene(),
            projected.observed_at,
            now,
            timing,
            numbering.last_event_seq(),
            projected.scene,
        );
        Publication { events, scene }
    }

    pub fn send(&mut self, output: Output, timing: Timing) -> bool {
        let publication = self.prepare(output, timing);
        self.publish(publication)
    }

    /// Hands an already numbered publication to the network actor, without waiting.
    pub fn publish(&mut self, publication: Publication) -> bool {
        match self.tx.try_send(publication) {
            Ok(()) => true,
            Err(error) => {
                self.dropped_events += error.into_inner().events.len() as u64;
                tracing::warn!(
                    dropped_events = self.dropped_events,
                    "publication channel full or closed; the new publication is dropped"
                );
                false
            }
        }
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

pub mod nats;
