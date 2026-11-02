//! An actor owns the client, the event queue and the last scene.
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

/// The last event that actually went out, for the `last_event_seq` of the
/// next scene (gaps included).
#[derive(Debug, Default, Clone, Copy)]
struct Sent(u64);

#[derive(Debug, Default)]
struct Pending {
    events: VecDeque<EventPublication>,
    /// The last scene: a new one replaces the previous one.
    scene: Option<ScenePublication>,
    dropped_events: u64,
}

impl Pending {
    fn ingest(&mut self, publication: Publication) {
        let Publication { events, scene } = publication;
        for event in events {
            if self.events.len() == EVENT_BUFFER {
                self.dropped_events += 1;
                tracing::warn!(
                    seq = event.seq,
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

    /// The front: events first; with the queue empty, the scene, with the
    /// last event that went out.
    fn front(&mut self, sent: Sent) -> Option<serde_json::Result<WireMessage>> {
        if let Some(event) = self.events.front() {
            Some(event.wire())
        } else if let Some(scene) = self.scene.as_mut() {
            scene.last_event_seq = Some(sent.0);
            Some(scene.wire())
        } else {
            None
        }
    }

    /// Removes the front that went out and notes its event.
    fn pop(&mut self, sent: &mut Sent) {
        match self.events.pop_front() {
            Some(event) => sent.0 = event.seq,
            None => self.scene = None,
        }
    }
}

pub async fn run(
    config: NatsConfig,
    prefix: String,
    mut rx: mpsc::Receiver<Publication>,
    token: CancellationToken,
) {
    tracing::info!(%prefix, version = super::VERSION, "NATS subjects: {{prefix}}.{{scene,events}}.{{version}}");
    let mut pending = Pending::default();
    let mut client: Option<async_nats::Client> = None;
    let mut closed = false;
    let mut stopping = None;
    let mut sent = Sent::default();
    loop {
        // Bounded by capacity: receiving cannot stop serving the network.
        for _ in 0..super::CHANNEL_BUFFER {
            match rx.try_recv() {
                Ok(p) => pending.ingest(p),
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
                publication=rx.recv() => match publication { Some(p) => pending.ingest(p), None => closed=true },
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
        let Some(message) = pending.front(sent) else {
            continue;
        };
        let result = match message {
            Ok(message) => timeout(NETWORK_TIMEOUT, publish(connected, &prefix, message)).await,
            Err(error) => Ok(Err(error.into())),
        };
        match result {
            Ok(Ok(())) => pending.pop(&mut sent),
            result => {
                // The front is not removed until flush: retry with the same ID.
                tracing::warn!(?result, "NATS publication failed; retrying the front");
                client = None;
            }
        }
    }
}

async fn publish(
    client: &async_nats::Client,
    prefix: &str,
    message: WireMessage,
) -> anyhow::Result<()> {
    let subject = message.kind.subject(prefix);
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
    use scene_core::{
        Input, Monitor,
        geom::{Quad, Rect},
        map::SiteMap,
        policy::Policy,
    };
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
        let mut queue = super::Pending::default();
        for n in 0..EVENT_BUFFER + 1 {
            let output = monitor
                .handle(Input::Sensor {
                    at,
                    state: if n % 2 == 0 {
                        SensorState::Streaming
                    } else {
                        SensorState::Connecting
                    },
                })
                .cancel();
            queue.ingest(handle.prepare(
                output,
                Timing {
                    wait_ms: 0.0,
                    processing_ms: 0.0,
                },
            ));
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
        let mut queue = super::Pending::default();
        for state in [SensorState::Streaming, SensorState::Connecting] {
            let output = monitor.handle(Input::Sensor { at, state }).cancel();
            queue.ingest(handle.prepare(
                output,
                Timing {
                    wait_ms: 0.0,
                    processing_ms: 0.0,
                },
            ));
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
}
