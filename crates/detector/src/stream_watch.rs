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
        // `ingest::StreamEvent` is `#[non_exhaustive]`: no match outside
        // `ingest` may close, so this wildcard stays. It is watched, not
        // silent: `every_known_variant_of_stream_event_is_translated` below
        // fails when a variant arrives that `translate` has not been taught.
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
        watch.on_stream(
            &ingest::StreamEvent::Streaming {
                session: ingest::Session(1),
            },
            t,
        );
        assert!(!watch.poll(t + limit - Duration::from_nanos(1), limit));
        assert!(watch.poll(t + limit, limit));
        assert!(!watch.poll(t + limit, limit));
        assert!(watch.on_sample(t + limit));
        assert!(!watch.on_sample(t + limit));
        watch.on_stream(
            &ingest::StreamEvent::Disconnected {
                session: ingest::Session(1),
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
                translate(&ingest::StreamEvent::Streaming {
                    session: ingest::Session(1)
                }),
                Some(SensorState::Streaming)
            );
        }
        assert_eq!(
            translate(&ingest::StreamEvent::Disconnected {
                session: ingest::Session(1),
                reason: ingest::IngestError::Protocol("test".into())
            }),
            Some(SensorState::Reconnecting)
        );
    }

    /// The wildcard in `translate` is `#[non_exhaustive]`'s (ingest may grow
    /// a variant), so this test is the one that points at a new one: it reads
    /// the enum's own source and fails until `translate` — and this list —
    /// know it.
    #[test]
    fn every_known_variant_of_stream_event_is_translated() {
        let known = ["Connecting", "Streaming", "Disconnected"];
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ingest/src/lib.rs"),
        )
        .expect("ingest's source is readable");
        let variants = stream_event_variants(&source);
        assert_eq!(
            variants, known,
            "ingest::StreamEvent grew or lost a variant: teach `translate` and this list"
        );
    }

    /// The variant names of `pub enum StreamEvent` in `source`, in order.
    fn stream_event_variants(source: &str) -> Vec<String> {
        let mut lines = source.lines();
        for line in &mut lines {
            if line.trim_start().starts_with("pub enum StreamEvent") {
                break;
            }
        }
        let mut variants = Vec::new();
        let mut depth: i64 = 1;
        for line in lines {
            if depth == 1 {
                let name: String = line
                    .trim()
                    .chars()
                    .skip_while(|c| c.is_whitespace() || *c == '#' || *c == '/')
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() && !line.trim().starts_with("//") {
                    variants.push(name);
                }
            }
            depth += line.matches('{').count() as i64 - line.matches('}').count() as i64;
            if depth <= 0 {
                break;
            }
        }
        variants
    }
}
