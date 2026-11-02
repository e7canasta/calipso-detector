//! The health of the stream, with a clock: the missing-samples timer, extracted
//! from v2's main.rs @4c74723. It neither decides reception changes nor emits
//! domain facts.
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct Stall {
    last_sample: Instant,
}

impl Stall {
    pub(crate) fn new(now: Instant) -> Self {
        Self { last_sample: now }
    }

    pub(crate) fn on_sample(&mut self, now: Instant) {
        self.last_sample = now;
    }

    pub(crate) fn expired(&self, now: Instant, timeout: Duration) -> bool {
        now.saturating_duration_since(self.last_sample) >= timeout
    }
}

/// Translates transport states; the core decides whether the reception changed.
pub(crate) fn translate(event: &ingest::StreamEvent) -> Option<scene_contract::SensorState> {
    use scene_contract::SensorState;
    match event {
        ingest::StreamEvent::Connecting => Some(SensorState::Connecting),
        ingest::StreamEvent::Streaming { .. } => Some(SensorState::Streaming),
        ingest::StreamEvent::Disconnected { .. } => Some(SensorState::Reconnecting),
        _ => {
            tracing::warn!("unknown ingest event");
            None
        }
    }
}

/// One stall deadline per session; armed on streaming and fired once.
#[derive(Debug)]
pub(crate) struct StreamWatch {
    stall: Stall,
    armed: bool,
    fired: bool,
}
impl StreamWatch {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            stall: Stall::new(now),
            armed: false,
            fired: false,
        }
    }
    pub(crate) fn on_stream(&mut self, event: &ingest::StreamEvent, now: Instant) {
        self.armed = matches!(event, ingest::StreamEvent::Streaming { .. });
        self.fired = false;
        self.stall.on_sample(now);
    }
    /// A sample restarts the deadline; if it recovers from a timeout, that must be reported.
    pub(crate) fn on_sample(&mut self, now: Instant) -> bool {
        let recovered = self.fired;
        self.armed = true;
        self.fired = false;
        self.stall.on_sample(now);
        recovered
    }
    pub(crate) fn poll(&mut self, now: Instant, timeout: Duration) -> bool {
        if self.armed && !self.fired && self.stall.expired(now, timeout) {
            self.fired = true;
            true
        } else {
            false
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stall_only_while_streaming_and_a_sample_recovers() {
        let t = Instant::now();
        let limit = Duration::from_secs(3);
        let mut watch = StreamWatch::new(t);
        assert!(!watch.poll(t + limit, limit));
        watch.on_stream(&ingest::StreamEvent::Streaming { session: 1 }, t);
        assert!(!watch.poll(t + limit - Duration::from_nanos(1), limit));
        assert!(watch.poll(t + limit, limit));
        assert!(!watch.poll(t + limit, limit));
        assert!(watch.on_sample(t + limit));
        assert!(!watch.on_sample(t + limit));
        watch.on_stream(
            &ingest::StreamEvent::Disconnected {
                session: 1,
                reason: ingest::IngestError::Protocol("test".into()),
            },
            t,
        );
        assert!(!watch.poll(t + limit, limit));
    }
    #[test]
    fn stream_is_translated_without_deduplicating() {
        use scene_contract::SensorState;
        assert_eq!(
            translate(&ingest::StreamEvent::Connecting),
            Some(SensorState::Connecting)
        );
        for _ in 0..2 {
            assert_eq!(
                translate(&ingest::StreamEvent::Streaming { session: 1 }),
                Some(SensorState::Streaming)
            );
        }
        assert_eq!(
            translate(&ingest::StreamEvent::Disconnected {
                session: 1,
                reason: ingest::IngestError::Protocol("test".into())
            }),
            Some(SensorState::Reconnecting)
        );
    }
}
