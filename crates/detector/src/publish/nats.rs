//! An actor owns the client, the event queues and the last scene of each
//! version. Each version goes out in its own order: its events first, then its
//! scene. v4 writes its scene's `last_event_seq` with the last event sent (gaps
//! included), as it always has; v5 keeps the numbering's (change 1).
use super::{
    EVENT_BUFFER, EventPublication, EventPublicationV5, Publication, ScenePublication,
    ScenePublicationV5, WireMessage, Wired,
};
use crate::config::NatsConfig;
use std::{collections::VecDeque, time::Duration};
use tokio::{
    sync::mpsc,
    time::{Instant, timeout},
};
use tokio_util::sync::CancellationToken;

const NETWORK_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// The last event that actually went out, for the `last_event_seq` of the
/// next v4 scene (gaps included).
#[derive(Debug, Default, Clone, Copy)]
struct Sent(u64);

/// An event that carries its number in its version's stream.
trait Numbered {
    fn number(&self) -> u64;
}

impl Numbered for EventPublication {
    fn number(&self) -> u64 {
        self.seq
    }
}

impl Numbered for EventPublicationV5 {
    fn number(&self) -> u64 {
        self.seq
    }
}

/// A scene that may restate the last event sent before it: v4 does, as the
/// transport sends it; v5 does not, and keeps the numbering's (change 1).
trait Restamp {
    fn restamp(&mut self, sent: Sent);
}

impl Restamp for ScenePublication {
    fn restamp(&mut self, sent: Sent) {
        self.last_event_seq = Some(sent.0);
    }
}

impl Restamp for ScenePublicationV5 {
    fn restamp(&mut self, _sent: Sent) {}
}

/// One version's queue: its events, and the last scene (a new one replaces the
/// previous one).
#[derive(Debug)]
struct Pending<E, S> {
    events: VecDeque<E>,
    scene: Option<S>,
    dropped_events: u64,
}

impl<E, S> Default for Pending<E, S> {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            scene: None,
            dropped_events: 0,
        }
    }
}

impl<E: Wired + Numbered, S: Wired + Restamp> Pending<E, S> {
    fn ingest(&mut self, events: Vec<E>, scene: S) {
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

    /// The front: events first; with the queue empty, the scene.
    fn front(&mut self, sent: Sent) -> Option<serde_json::Result<WireMessage>> {
        if let Some(event) = self.events.front() {
            Some(event.wire())
        } else if let Some(scene) = self.scene.as_mut() {
            scene.restamp(sent);
            Some(scene.wire())
        } else {
            None
        }
    }

    /// Removes the front that went out and notes its event.
    fn pop(&mut self, sent: &mut Sent) {
        match self.events.pop_front() {
            Some(event) => sent.0 = event.number(),
            None => self.scene = None,
        }
    }
}

type PendingV4 = Pending<EventPublication, ScenePublication>;
type PendingV5 = Pending<EventPublicationV5, ScenePublicationV5>;

/// Which version's queue the front came from, so that it is popped from there.
#[derive(Debug, Clone, Copy)]
enum Lane {
    V4,
    V5,
}

pub async fn run(
    config: NatsConfig,
    prefix: String,
    mut rx: mpsc::Receiver<Publication>,
    token: CancellationToken,
) {
    tracing::info!(%prefix, "NATS subjects: {{prefix}}.{{scene,events}}.{{version}}");
    let mut pending4 = PendingV4::default();
    let mut pending5 = PendingV5::default();
    let mut client: Option<async_nats::Client> = None;
    let mut closed = false;
    let mut stopping = None;
    let mut sent = Sent::default();
    let mut sent5 = Sent::default();
    loop {
        // Bounded by capacity: receiving cannot stop serving the network.
        for _ in 0..super::CHANNEL_BUFFER {
            match rx.try_recv() {
                Ok(p) => ingest(&mut pending4, &mut pending5, p),
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
                pending_events = pending4.events.len() + pending5.events.len(),
                "NATS shutdown deadline reached"
            );
            return;
        }
        if pending4.is_empty() && pending5.is_empty() {
            if closed || stopping.is_some() {
                return;
            }
            tokio::select! {
                publication=rx.recv() => match publication {
                    Some(p) => ingest(&mut pending4, &mut pending5, p),
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
        let connected = client.as_ref().expect("client just connected");
        // v4 goes first; v5 follows, each in its own order.
        let next = match pending4.front(sent) {
            Some(message) => Some((Lane::V4, message)),
            None => pending5.front(sent5).map(|message| (Lane::V5, message)),
        };
        let Some((lane, message)) = next else {
            continue;
        };
        let result = match message {
            Ok(message) => timeout(NETWORK_TIMEOUT, publish(connected, &prefix, message)).await,
            Err(error) => Ok(Err(error.into())),
        };
        match result {
            Ok(Ok(())) => match lane {
                Lane::V4 => pending4.pop(&mut sent),
                Lane::V5 => pending5.pop(&mut sent5),
            },
            result => {
                // The front is not removed until flush: retry with the same ID.
                tracing::warn!(?result, "NATS publication failed; retrying the front");
                client = None;
            }
        }
    }
}

/// Hands a publication to each version's queue.
fn ingest(pending4: &mut PendingV4, pending5: &mut PendingV5, publication: Publication) {
    let Publication {
        events,
        scene,
        v5,
        publish_v4,
    } = publication;
    if publish_v4 {
        pending4.ingest(events, scene);
    }
    if let Some(v5) = v5 {
        pending5.ingest(v5.events, v5.scene);
    }
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
    use crate::{
        build,
        publish::{Publication, PublisherHandle},
    };
    use scene_contract::*;

    fn ingest_v4(queue: &mut PendingV4, publication: Publication) {
        queue.ingest(publication.events, publication.scene);
    }
    use scene_core::{Input, Monitor, map::SiteMap, policy::Policy};
    use scene_geom::{Quad, Rect};
    #[test]
    fn fifo_1024_keeps_the_front_and_drops_the_newest() {
        let (mut handle, _) = PublisherHandle::channel(
            Source {
                site: "s".into(),
                camera: "c".into(),
                run_id: "r".into(),
                build: build::current(),
            },
            Profile {
                camera: None,
                resident: None,
                hash: ProfileHash("h".into()),
                senses: Vec::new(),
            },
        );
        let mut monitor = Monitor::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(1.0, 1.0, 2.0, 2.0))),
        );
        let at = SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(0.0),
        };
        let mut queue = PendingV4::default();
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
            ingest_v4(
                &mut queue,
                handle.prepare(
                    output,
                    Timing {
                        wait_ms: 0.0,
                        processing_ms: 0.0,
                    },
                ),
            );
        }
        assert_eq!(queue.events.len(), 1024);
        assert_eq!(queue.events.front().unwrap().seq, 1);
        assert_eq!(queue.events.back().unwrap().seq, 1024);
        assert_eq!(queue.dropped_events, 1);
        assert_eq!(queue.scene.as_ref().unwrap().seq, 1025);
    }

    #[test]
    fn events_go_out_first_and_the_last_scene_says_the_last_event_sent() {
        use crate::publish::Kind;
        let (mut handle, _) = PublisherHandle::channel(
            Source {
                site: "s".into(),
                camera: "c".into(),
                run_id: "r".into(),
                build: build::current(),
            },
            Profile {
                camera: None,
                resident: None,
                hash: ProfileHash("h".into()),
                senses: Vec::new(),
            },
        );
        let mut monitor = Monitor::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(1.0, 1.0, 2.0, 2.0))),
        );
        let at = SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(0.0),
        };
        let mut queue = PendingV4::default();
        for state in [SensorState::Streaming, SensorState::Connecting] {
            let output = monitor
                .handle(Input::Sensor {
                    state,
                    session: at.session,
                    last: Some(at),
                })
                .cancel();
            ingest_v4(
                &mut queue,
                handle.prepare(
                    output,
                    Timing {
                        wait_ms: 0.0,
                        processing_ms: 0.0,
                    },
                ),
            );
        }
        // Simulates a network that delivers everything: the order and the last_event_seq.
        let mut sent = super::Sent::default();
        let mut wire = Vec::new();
        while let Some(message) = queue.front(sent) {
            let message = message.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&message.bytes).unwrap();
            wire.push((message.kind, json));
            queue.pop(&mut sent);
        }
        let kinds: Vec<Kind> = wire.iter().map(|(kind, _)| *kind).collect();
        let events = kinds.iter().filter(|k| **k == Kind::Events).count();
        assert!(events >= 2, "{kinds:?}");
        // First all events in numbered order; then only the last scene.
        assert!(kinds[..events].iter().all(|k| *k == Kind::Events));
        assert_eq!(kinds[events..], [Kind::Scene]);
        let last = wire[..events]
            .iter()
            .map(|(_, json)| json["seq"].as_u64().unwrap())
            .max()
            .unwrap();
        let scene = &wire[events].1;
        assert_eq!(scene["seq"], 2, "{scene}");
        assert_eq!(scene["last_event_seq"], last);
        assert!(queue.is_empty());
    }

    #[test]
    fn v5_keeps_the_numbering_where_v4_restates_the_last_event_sent() {
        use crate::publish::Version;
        let (mut handle, _) = PublisherHandle::channel_with(
            Source {
                site: "s".into(),
                camera: "c".into(),
                run_id: "r".into(),
                build: build::current(),
            },
            Profile {
                camera: None,
                resident: None,
                hash: ProfileHash("h".into()),
                senses: Vec::new(),
            },
            &[Version::V4, Version::V5],
        );
        let mut monitor = Monitor::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(1.0, 1.0, 2.0, 2.0))),
        );
        let at = SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(0.0),
        };
        let output = monitor
            .handle(Input::Sensor {
                state: SensorState::Streaming,
                session: at.session,
                last: Some(at),
            })
            .cancel();
        let publication = handle.prepare(
            output,
            Timing {
                wait_ms: 0.0,
                processing_ms: 0.0,
            },
        );
        let v5 = publication.v5.expect("v5 is listed");
        assert_eq!(v5.scene.last_event_seq, Some(v5.events.len() as u64));
        // The transport says it last sent event 1: v4 restates that, v5 keeps
        // the numbering's last event.
        let mut v4_scene = publication.scene;
        let mut v5_scene = v5.scene;
        super::Restamp::restamp(&mut v4_scene, super::Sent(1));
        super::Restamp::restamp(&mut v5_scene, super::Sent(1));
        assert_eq!(v4_scene.last_event_seq, Some(1));
        assert_eq!(v5_scene.last_event_seq, Some(v5.events.len() as u64));
    }

    /// Retirar v4 (ADR 2026-11-02-retirar-v4): sin v4 en el perfil, v5 sigue
    /// sin reenviar el último evento enviado. El transporte no reescribe la
    /// escena de v5: su `last_event_seq` es el último evento numerado.
    #[test]
    fn v5_alone_keeps_its_last_event_seq_when_the_transport_restates_one_sent() {
        use crate::publish::Version;
        let (mut handle, _) = PublisherHandle::channel_with(
            Source {
                site: "s".into(),
                camera: "c".into(),
                run_id: "r".into(),
                build: build::current(),
            },
            Profile {
                camera: None,
                resident: None,
                hash: ProfileHash("h".into()),
                senses: Vec::new(),
            },
            &[Version::V5],
        );
        let mut monitor = Monitor::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(1.0, 1.0, 2.0, 2.0))),
        );
        let at = SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(0.0),
        };
        let output = monitor
            .handle(Input::Sensor {
                state: SensorState::Streaming,
                session: at.session,
                last: Some(at),
            })
            .cancel();
        let publication = handle.prepare(
            output,
            Timing {
                wait_ms: 0.0,
                processing_ms: 0.0,
            },
        );
        let v5 = publication.v5.expect("v5 is listed");
        let numbered = v5.events.len() as u64;
        assert_eq!(v5.scene.last_event_seq, Some(numbered));
        // The transport's number must differ from the numbered one, or a
        // restamp that copies it would pass.
        let mut scene = v5.scene;
        super::Restamp::restamp(&mut scene, super::Sent(numbered + 7));
        assert_eq!(scene.last_event_seq, Some(numbered));
    }
}
