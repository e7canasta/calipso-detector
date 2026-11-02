//! The handle numbers before sending; the network has its own owner. Each
//! contract version the profile lists is numbered on its own (its events and
//! its scenes, contiguous from 1), and goes out on its own subjects.
use scene_contract::{Envelope, Event, Profile, Scene, SchemaId, Source, Timing, v5};
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
pub(crate) type ScenePublication = Envelope<Arc<Scene>>;
pub(crate) type EventPublication = Envelope<Event>;
pub(crate) type ScenePublicationV5 = v5::Envelope<Arc<v5::Scene>>;
pub(crate) type EventPublicationV5 = v5::Envelope<v5::Event>;

/// A contract version the shell publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V4,
    V5,
}

impl Version {
    /// The name the profile gives it: `v4`, `v5`.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "v4" => Some(Self::V4),
            "v5" => Some(Self::V5),
            _ => None,
        }
    }

    /// The last token of each subject.
    pub fn token(self) -> &'static str {
        match self {
            Self::V4 => "v4",
            Self::V5 => "v5",
        }
    }
}

/// A numbered turn in v4: its events and, after them, its scene; and in v5, when
/// the profile lists it, the same turn in v5, numbered on its own.
#[derive(Debug)]
pub struct Publication {
    pub events: Vec<EventPublication>,
    pub scene: ScenePublication,
    pub v5: Option<Publication5>,
}

/// The same turn in v5: its events, then its scene.
#[derive(Debug)]
pub struct Publication5 {
    pub events: Vec<EventPublicationV5>,
    pub scene: ScenePublicationV5,
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

    /// `{prefix}.scene.v4`, `{prefix}.events.v5`, and so on.
    pub fn subject(self, prefix: &str, version: Version) -> String {
        format!("{prefix}.{}.{}", self.as_str(), version.token())
    }
}

/// An envelope that knows which stream it goes to and in which version: the
/// in-memory sink and the network actor convert it the same way.
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
    const VERSION: Version = Version::V4;
    fn msg_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }
}

impl Wired for ScenePublication {
    const KIND: Kind = Kind::Scene;
    const VERSION: Version = Version::V4;
    fn msg_id(&self) -> Option<&str> {
        None
    }
}

impl Wired for EventPublicationV5 {
    const KIND: Kind = Kind::Events;
    const VERSION: Version = Version::V5;
    fn msg_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }
}

impl Wired for ScenePublicationV5 {
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
    /// In-memory sink: exactly the bytes and headers NATS consumes, each version
    /// its events before its scene; v4 first.
    pub fn messages(&self) -> serde_json::Result<Vec<WireMessage>> {
        let mut messages = Vec::with_capacity(self.events.len() + 1);
        for event in &self.events {
            messages.push(event.wire()?);
        }
        messages.push(self.scene.wire()?);
        if let Some(v5) = &self.v5 {
            for event in &v5.events {
                messages.push(event.wire()?);
            }
            messages.push(v5.scene.wire()?);
        }
        Ok(messages)
    }
}

/// The numbering of one version's stream: its events and its scenes, each
/// contiguous from 1.
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

/// v5's `observed_at` of an event: the sample it happened in, or none for a
/// reception change, which observes none (change 2).
fn v5_event_at(event: &v5::Event) -> Option<scene_contract::SampleRef> {
    match event {
        v5::Event::Sensor(e) => e.at,
        v5::Event::Transition(e) => Some(e.at),
        v5::Event::Parts(e) => Some(e.at),
        v5::Event::Derived(e) => Some(e.at),
        v5::Event::Map(e) => Some(e.at),
        v5::Event::BedPosture(e) => Some(e.at),
    }
}

#[derive(Debug)]
pub struct PublisherHandle {
    source: Source,
    profile: Profile,
    numbering: Numbering,
    numbering5: Numbering,
    publish_v5: bool,
    tx: mpsc::Sender<Publication>,
    dropped_events: u64,
}

impl PublisherHandle {
    /// v4 only: what the profile publishes by default.
    pub fn channel(source: Source, profile: Profile) -> (Self, mpsc::Receiver<Publication>) {
        Self::channel_with(source, profile, &[Version::V4])
    }

    /// The versions the profile lists; the caller says which.
    pub fn channel_with(
        source: Source,
        profile: Profile,
        versions: &[Version],
    ) -> (Self, mpsc::Receiver<Publication>) {
        let (tx, rx) = mpsc::channel(CHANNEL_BUFFER);
        (
            Self {
                source,
                profile,
                numbering: Numbering::default(),
                numbering5: Numbering::default(),
                publish_v5: versions.contains(&Version::V5),
                tx,
                dropped_events: 0,
            },
            rx,
        )
    }

    /// Numbers a turn in each version it publishes, and wraps it in envelopes.
    pub fn prepare(&mut self, output: Output, timing: Timing) -> Publication {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let Self {
            source,
            profile,
            numbering,
            numbering5,
            publish_v5,
            ..
        } = self;
        let Output {
            at,
            scene,
            events,
            v5: projected,
        } = output;
        // The event_id names the version: if a stream captures several,
        // deduplication by Nats-Msg-Id does not confuse one event with another.
        let v4_events = events
            .into_iter()
            .map(|event| {
                let seq = numbering.next_event();
                Envelope {
                    schema: SchemaId::EventsV4,
                    source: source.clone(),
                    profile: profile.clone(),
                    seq,
                    observed_at: event.at(),
                    published_at_ms: now,
                    event_id: Some(format!("{}-{}-{seq}", source.run_id, Version::V4.token())),
                    timing: None,
                    last_event_seq: None,
                    body: event,
                }
            })
            .collect();
        let v4_scene = Envelope {
            schema: SchemaId::SceneV4,
            source: source.clone(),
            profile: profile.clone(),
            seq: numbering.next_scene(),
            observed_at: at,
            published_at_ms: now,
            event_id: None,
            timing: Some(timing),
            last_event_seq: Some(numbering.last_event_seq()),
            body: scene,
        };
        let v5 = (*publish_v5).then(|| {
            let v5_events = projected.events.into_iter().map(|event| {
                let seq = numbering5.next_event();
                v5::Envelope {
                    schema: v5::SchemaId::Events,
                    source: source.clone(),
                    profile: profile.clone(),
                    seq,
                    observed_at: v5_event_at(&event),
                    published_at_ms: now,
                    event_id: Some(format!("{}-{}-{seq}", source.run_id, Version::V5.token())),
                    timing: None,
                    last_event_seq: None,
                    body: event,
                }
            });
            let events = v5_events.collect();
            // The scene names the last v5 event numbered before it: the
            // numbering's, not the transport's (change 1).
            let scene = v5::Envelope {
                schema: v5::SchemaId::Scene,
                source: source.clone(),
                profile: profile.clone(),
                seq: numbering5.next_scene(),
                observed_at: projected.observed_at,
                published_at_ms: now,
                event_id: None,
                timing: Some(timing),
                last_event_seq: Some(numbering5.last_event_seq()),
                body: projected.scene,
            };
            Publication5 { events, scene }
        });
        Publication {
            events: v4_events,
            scene: v4_scene,
            v5,
        }
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
