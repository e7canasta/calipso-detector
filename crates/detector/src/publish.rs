//! The handle numbers before sending; the network has its own owner.
use scene_core::{
    Output,
    contract::{Envelope, Event, Profile, Scene, SchemaId, Source, Timing, v4},
};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

pub const EVENT_BUFFER: usize = 1024;
/// The channel is bounded too; the actor's fact queue has 1024 slots.
pub const CHANNEL_BUFFER: usize = 64;
pub type ScenePublication = Envelope<Arc<Scene>>;
pub type EventPublication = Envelope<Event>;
pub type SceneV4Publication = v4::Envelope<Arc<v4::Scene>>;
pub type EventV4Publication = v4::Envelope<v4::Event>;

/// What a version publishes of a turn, already numbered: its events and,
/// after them, its scene.
#[derive(Debug)]
pub struct Part<E, S> {
    pub events: Vec<E>,
    pub scene: S,
}

/// A numbered turn, with one part per version the profile asks for.
#[derive(Debug)]
pub struct Publication {
    pub v3: Option<Part<EventPublication, ScenePublication>>,
    pub v4: Option<Part<EventV4Publication, SceneV4Publication>>,
}

/// A contract version the shell knows how to publish; the profile picks which
/// (D9-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Contract {
    V3,
    V4,
}

impl Contract {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V3 => "v3",
            Self::V4 => "v4",
        }
    }
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
}

/// Where a message goes: the subject and headers come from here, for any
/// version. Each version's `schema` travels inside the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stream {
    pub contract: Contract,
    pub kind: Kind,
}

impl Stream {
    pub const fn scene(contract: Contract) -> Self {
        Self {
            contract,
            kind: Kind::Scene,
        }
    }

    pub const fn events(contract: Contract) -> Self {
        Self {
            contract,
            kind: Kind::Events,
        }
    }

    /// `{prefix}.scene.v3`, `{prefix}.events.v4`, …
    pub fn subject(self, prefix: &str) -> String {
        format!("{prefix}.{}.{}", self.kind.as_str(), self.contract.as_str())
    }
}

/// An envelope that knows which stream it goes to: the in-memory sink and the
/// network actor convert it the same way, whatever its version.
pub(crate) trait Wired: Serialize {
    const STREAM: Stream;
    /// The deduplication ID; only events carry it.
    fn msg_id(&self) -> Option<&str>;

    fn wire(&self) -> serde_json::Result<WireMessage> {
        Ok(WireMessage {
            stream: Self::STREAM,
            bytes: serde_json::to_vec(self)?,
            msg_id: self.msg_id().map(str::to_owned),
        })
    }
}

impl Wired for EventPublication {
    const STREAM: Stream = Stream::events(Contract::V3);
    fn msg_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }
}

impl Wired for ScenePublication {
    const STREAM: Stream = Stream::scene(Contract::V3);
    fn msg_id(&self) -> Option<&str> {
        None
    }
}

impl Wired for EventV4Publication {
    const STREAM: Stream = Stream::events(Contract::V4);
    fn msg_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }
}

impl Wired for SceneV4Publication {
    const STREAM: Stream = Stream::scene(Contract::V4);
    fn msg_id(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug)]
pub struct WireMessage {
    pub stream: Stream,
    pub bytes: Vec<u8>,
    pub msg_id: Option<String>,
}

impl WireMessage {
    /// The header is the same in the in-memory sink and in the real transport.
    pub(crate) fn headers(&self) -> Option<async_nats::HeaderMap> {
        match self.stream.kind {
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

/// The messages of a part: its events and then its scene.
fn wire_part<E: Wired, S: Wired>(
    part: &Part<E, S>,
    messages: &mut Vec<WireMessage>,
) -> serde_json::Result<()> {
    for event in &part.events {
        messages.push(event.wire()?);
    }
    messages.push(part.scene.wire()?);
    Ok(())
}

impl Publication {
    /// In-memory sink: exactly the bytes and headers NATS consumes, version by
    /// version (v3 and then v4), each with its events before its scene.
    pub fn messages(&self) -> serde_json::Result<Vec<WireMessage>> {
        let mut messages = Vec::with_capacity(self.events() + 2);
        if let Some(part) = &self.v3 {
            wire_part(part, &mut messages)?;
        }
        if let Some(part) = &self.v4 {
            wire_part(part, &mut messages)?;
        }
        Ok(messages)
    }

    /// How many events it carries, across versions.
    pub fn events(&self) -> usize {
        self.v3.as_ref().map_or(0, |p| p.events.len())
            + self.v4.as_ref().map_or(0, |p| p.events.len())
    }
}

/// The numbering of a version: its events and its scenes, each contiguous
/// desde 1.
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
    /// One numbering per version the profile asks for.
    v3: Option<Numbering>,
    v4: Option<Numbering>,
    tx: mpsc::Sender<Publication>,
    dropped_events: u64,
}
impl PublisherHandle {
    pub fn channel(
        source: Source,
        profile: Profile,
        contracts: &[Contract],
    ) -> (Self, mpsc::Receiver<Publication>) {
        let (tx, rx) = mpsc::channel(CHANNEL_BUFFER);
        let numbering = |c| contracts.contains(&c).then(Numbering::default);
        (
            Self {
                source,
                profile,
                v3: numbering(Contract::V3),
                v4: numbering(Contract::V4),
                tx,
                dropped_events: 0,
            },
            rx,
        )
    }
    pub fn prepare(&mut self, output: Output, timing: Timing) -> Publication {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let Self {
            source,
            profile,
            v3,
            v4,
            ..
        } = self;
        let v3 = v3.as_mut().map(|numbering| Part {
            events: output
                .events
                .into_iter()
                .map(|event| {
                    let seq = numbering.next_event();
                    Envelope {
                        schema: SchemaId::EventsV3,
                        source: source.clone(),
                        profile: profile.clone(),
                        seq,
                        observed_at: event.at,
                        published_at_ms: now,
                        event_id: Some(format!("{}-{seq}", source.run_id)),
                        timing: None,
                        last_event_seq: None,
                        body: event,
                    }
                })
                .collect(),
            scene: Envelope {
                schema: SchemaId::SceneV3,
                source: source.clone(),
                profile: profile.clone(),
                seq: numbering.next_scene(),
                observed_at: output.at,
                published_at_ms: now,
                event_id: None,
                timing: Some(timing),
                last_event_seq: Some(numbering.last_event_seq()),
                body: output.scene,
            },
        });
        // v4's event_id names the version: if a stream captures both, deduplication
        // by Nats-Msg-Id does not confuse one event with another.
        let v4 = v4.as_mut().map(|numbering| Part {
            events: output
                .v4
                .events
                .into_iter()
                .map(|event| {
                    let seq = numbering.next_event();
                    v4::Envelope {
                        schema: v4::SchemaId::EventsV4,
                        source: source.clone(),
                        profile: profile.clone(),
                        seq,
                        observed_at: event.at(),
                        published_at_ms: now,
                        event_id: Some(format!("{}-v4-{seq}", source.run_id)),
                        timing: None,
                        last_event_seq: None,
                        body: event,
                    }
                })
                .collect(),
            scene: v4::Envelope {
                schema: v4::SchemaId::SceneV4,
                source: source.clone(),
                profile: profile.clone(),
                seq: numbering.next_scene(),
                observed_at: output.at,
                published_at_ms: now,
                event_id: None,
                timing: Some(timing),
                last_event_seq: Some(numbering.last_event_seq()),
                body: output.v4.scene,
            },
        });
        Publication { v3, v4 }
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
                self.dropped_events += error.into_inner().events() as u64;
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
