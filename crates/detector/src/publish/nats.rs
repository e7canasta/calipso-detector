//! An actor owns the client, the event queue and the last scene. The turns go
//! out in order: their events first, then their scene. The scene keeps the
//! numbering's `last_event_seq`, not the transport's (change 1): a lost event
//! shows up as a gap in `seq`.
use super::{EVENT_BUFFER, EventPublication, Publication, ScenePublication, WireMessage, Wired};
use crate::config::NatsConfig;
use std::{collections::VecDeque, time::Duration};
use tokio::{
    sync::mpsc,
    time::{Instant, timeout},
};
use tokio_util::sync::CancellationToken;

const NETWORK_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// An event that carries its number in the stream.
trait Numbered {
    fn number(&self) -> u64;
}

impl Numbered for EventPublication {
    fn number(&self) -> u64 {
        self.seq
    }
}

/// The queue: its events, and the last scene (a new one replaces the previous).
#[derive(Debug, Default)]
struct Queue {
    events: VecDeque<EventPublication>,
    scene: Option<ScenePublication>,
    dropped_events: u64,
}

impl Queue {
    fn ingest(&mut self, events: Vec<EventPublication>, scene: ScenePublication) {
        for event in events {
            if self.events.len() == EVENT_BUFFER {
                self.dropped_events += 1;
                tracing::warn!(
                    seq = event.number(),
                    dropped_events = self.dropped_events,
                    "FIFO full; the newest event is dropped"
                );
            } else {
                self.events.push_back(event);
            }
        }
        self.scene = Some(scene);
    }

    fn is_empty(&self) -> bool {
        self.events.is_empty() && self.scene.is_none()
    }

    /// The front: events first; with the queue empty, the scene. The scene
    /// goes out as the numbering wrote it (change 1): the transport never
    /// stamps `last_event_seq`.
    fn front(&self) -> Option<serde_json::Result<WireMessage>> {
        if let Some(event) = self.events.front() {
            Some(event.wire())
        } else {
            self.scene.as_ref().map(|scene| scene.wire())
        }
    }

    /// Removes the front that went out.
    fn pop(&mut self) {
        if self.events.pop_front().is_none() {
            self.scene = None;
        }
    }
}

pub async fn run(
    config: NatsConfig,
    prefix: String,
    mut rx: mpsc::Receiver<Publication>,
    token: CancellationToken,
) {
    tracing::info!(%prefix, "NATS subjects: {{prefix}}.{{scene,events}}.v5");
    let mut pending = Queue::default();
    let mut client: Option<async_nats::Client> = None;
    let mut closed = false;
    let mut stopping = None;
    loop {
        // Bounded by capacity: receiving cannot stop serving the network.
        for _ in 0..super::CHANNEL_BUFFER {
            match rx.try_recv() {
                Ok(p) => ingest(&mut pending, p),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if token.is_cancelled() {
            stopping.get_or_insert_with(|| Instant::now() + SHUTDOWN_GRACE);
        }
        if stopping.is_some_and(|deadline| Instant::now() >= deadline) {
            tracing::warn!(
                pending_events = pending.events.len(),
                "NATS shutdown deadline reached"
            );
            return;
        }
        if pending.is_empty() {
            if closed || stopping.is_some() {
                return;
            }
            tokio::select! {
                publication=rx.recv() => match publication {
                    Some(p) => ingest(&mut pending, p),
                    None => closed=true,
                },
                _=token.cancelled() => { stopping=Some(Instant::now()+SHUTDOWN_GRACE); }
            }
            continue;
        }
        if client.is_none() {
            match timeout(NETWORK_TIMEOUT, async_nats::connect(&config.url)).await {
                Ok(Ok(connected)) => {
                    tracing::info!("NATS conectado");
                    client = Some(connected);
                }
                result => {
                    tracing::warn!(?result, "NATS does not connect; pending events are kept");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }
        }
        let Some(connected) = client.as_ref() else {
            continue;
        };
        let Some(message) = pending.front() else {
            continue;
        };
        let result = match message {
            Ok(message) => timeout(NETWORK_TIMEOUT, publish(connected, &prefix, message)).await,
            Err(error) => Ok(Err(error.into())),
        };
        match result {
            Ok(Ok(())) => pending.pop(),
            result => {
                // The front is not removed until flush: retry with the same ID.
                tracing::warn!(?result, "NATS publication failed; retrying the front");
                client = None;
            }
        }
    }
}

/// Hands a publication to the queue.
fn ingest(pending: &mut Queue, publication: Publication) {
    pending.ingest(publication.events, publication.scene);
}

async fn publish(
    client: &async_nats::Client,
    prefix: &str,
    message: WireMessage,
) -> anyhow::Result<()> {
    let subject = message.subject(prefix);
    match message.headers() {
        Some(headers) => {
            client
                .publish_with_headers(subject, headers, message.bytes.into())
                .await?
        }
        None => client.publish(subject, message.bytes.into()).await?,
    }
    // Confirms delivery to the Core NATS server; durability belongs to the deployment.
    client.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build, publish::PublisherHandle};
    use scene_contract::*;
    use scene_core::{Input, Monitor, map::SiteMap, policy::Policy};
    use scene_geom::{Quad, Rect};

    fn source() -> Source {
        Source {
            site: "s".into(),
            camera: "c".into(),
            run_id: "r".into(),
            build: build::current(),
        }
    }

    fn profile() -> Profile {
        Profile {
            camera: None,
            resident: None,
            hash: ProfileHash("h".into()),
            senses: Vec::new(),
        }
    }

    fn monitor() -> Monitor {
        Monitor::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(1.0, 1.0, 2.0, 2.0))),
        )
    }

    fn at() -> SampleRef {
        SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(0.0),
        }
    }

    fn timing() -> Timing {
        Timing {
            wait_ms: 0.0,
            processing_ms: 0.0,
        }
    }

    #[test]
    fn fifo_1024_keeps_the_front_and_drops_the_newest() {
        let (mut handle, _) = PublisherHandle::channel(source(), profile());
        let mut monitor = monitor();
        let at = at();
        let mut queue = Queue::default();
        for n in 0..EVENT_BUFFER + 1 {
            let output = monitor
                .handle(Input::Sensor {
                    session: at.session,
                    last: Some(at),
                    state: if n % 2 == 0 {
                        SensorState::Streaming
                    } else {
                        SensorState::Connecting
                    },
                })
                .cancel();
            ingest(&mut queue, handle.prepare(output, timing()));
        }
        assert_eq!(queue.events.len(), 1024);
        assert_eq!(queue.events.front().unwrap().seq, 1);
        assert_eq!(queue.events.back().unwrap().seq, 1024);
        assert_eq!(queue.dropped_events, 1);
        assert_eq!(queue.scene.as_ref().unwrap().seq, 1025);
    }

    #[test]
    fn events_go_out_first_and_the_last_scene_says_the_last_event_numbered() {
        use crate::publish::Kind;
        let (mut handle, _) = PublisherHandle::channel(source(), profile());
        let mut monitor = monitor();
        let at = at();
        let mut queue = Queue::default();
        for state in [SensorState::Streaming, SensorState::Connecting] {
            let output = monitor
                .handle(Input::Sensor {
                    state,
                    session: at.session,
                    last: Some(at),
                })
                .cancel();
            ingest(&mut queue, handle.prepare(output, timing()));
        }
        let mut wire = Vec::new();
        while let Some(message) = queue.front() {
            let message = message.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&message.bytes).unwrap();
            wire.push((message.kind, json));
            queue.pop();
        }
        let kinds: Vec<Kind> = wire.iter().map(|(kind, _)| *kind).collect();
        let events = kinds.iter().filter(|k| **k == Kind::Events).count();
        assert!(events >= 2, "{kinds:?}");
        assert!(kinds[..events].iter().all(|k| *k == Kind::Events));
        assert_eq!(kinds[events..], [Kind::Scene]);
        let last = wire[..events].last().unwrap().1["seq"].clone();
        assert_eq!(wire[events].1["last_event_seq"], last);
    }

    /// The property the removed no-op trait protected (change 1): the scene
    /// keeps the numbering's `last_event_seq`, never the transport's count —
    /// what went out does not stamp what is published. What stands is that the
    /// numbering's number reaches the wire untouched through the queue — and
    /// `a_scene_that_follows_a_dropped_event_keeps_the_numbering_through_the_queue`
    /// pins the case where the transport's count and the numbering's differ.
    #[test]
    fn v5_alone_keeps_its_last_event_seq_whatever_the_transport_sent() {
        let (mut handle, _) = PublisherHandle::channel(source(), profile());
        let mut monitor = monitor();
        let at = at();
        let output = monitor
            .handle(Input::Sensor {
                state: SensorState::Streaming,
                session: at.session,
                last: Some(at),
            })
            .cancel();
        let publication = handle.prepare(output, timing());
        let numbered = publication.events.len() as u64;
        assert_eq!(publication.scene.last_event_seq, Some(numbered));
        // Through the transport's queue: the scene keeps the numbering's.
        let mut queue = Queue::default();
        ingest(&mut queue, publication);
        while !queue.events.is_empty() {
            queue.pop();
        }
        let scene = queue.front().unwrap().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&scene.bytes).unwrap();
        assert_eq!(json["last_event_seq"], numbered, "{json}");
    }

    /// Through the actor's queue: the FIFO drops the newest event, so the
    /// numbering runs ahead of what went out, and the scene must keep the
    /// numbering's.
    #[test]
    fn a_scene_that_follows_a_dropped_event_keeps_the_numbering_through_the_queue() {
        let (mut handle, _) = PublisherHandle::channel(source(), profile());
        let mut monitor = monitor();
        let at = at();
        let mut queue = Queue::default();
        for n in 0..EVENT_BUFFER + 1 {
            let output = monitor
                .handle(Input::Sensor {
                    session: at.session,
                    last: Some(at),
                    state: if n % 2 == 0 {
                        SensorState::Streaming
                    } else {
                        SensorState::Connecting
                    },
                })
                .cancel();
            ingest(&mut queue, handle.prepare(output, timing()));
        }
        assert_eq!(queue.dropped_events, 1);
        // The transport delivers the 1024 queued events: its last one is 1024.
        let mut delivered = 0;
        while !queue.events.is_empty() {
            queue.pop();
            delivered += 1;
        }
        assert_eq!(delivered, 1024);
        let scene = queue.front().unwrap().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&scene.bytes).unwrap();
        assert_eq!(json["last_event_seq"], 1025, "{json}");
    }
}
