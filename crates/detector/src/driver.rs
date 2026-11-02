//! Translates and delivers: every scene decision belongs to Monitor. The
//! driver does not know where samples come from or who looks: a
//! [`SampleSource`] and a [`SensesPort`] are injected into it.
use crate::clock::{Clock, SystemClock};
use crate::router;
use crate::senses::{Frame, SensesPort};
use crate::source::{Arrival, SampleSource};
use crate::stream_watch::{self, StreamWatch};
use crate::verifier::{NoVerifier, VerifierPort};
use ingest::{IngestError, Stamp, StreamEvent};
use scene_contract::{SampleRef, SensorState, SessionId, SourceSecs, Timing};
use scene_core::{Input, Monitor, NoFrame, Output, map::SiteMap, policy::Policy};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct Processed {
    pub output: Output,
    pub timing: Timing,
}

/// Both types belong to other crates: Rust does not allow impl From here.
pub(crate) fn stamp_to_ref(stamp: Stamp) -> SampleRef {
    SampleRef {
        session: SessionId(stamp.session),
        sequence: stamp.sequence,
        source_secs: SourceSecs(stamp.source_secs),
    }
}
/// The driver of one camera. By default, with the live senses, no verifier and
/// the system's clock.
#[derive(Debug)]
pub struct Driver<
    S: SensesPort = sense::Senses,
    V: VerifierPort<Frame = <S as SensesPort>::Frame> = NoVerifier<<S as SensesPort>::Frame>,
    C: Clock = SystemClock,
> {
    monitor: Monitor,
    senses: S,
    verifier: V,
    watch: StreamWatch,
    last_at: Option<SampleRef>,
    session: u64,
    stall_timeout: Duration,
    clock: C,
}
impl<S: SensesPort, C: Clock> Driver<S, NoVerifier<S::Frame>, C> {
    pub fn new(policy: Policy, map: SiteMap, senses: S, stall_timeout: Duration, clock: C) -> Self {
        Self {
            monitor: Monitor::new(policy, map),
            senses,
            verifier: NoVerifier::default(),
            watch: StreamWatch::new(clock.now()),
            last_at: None,
            session: 0,
            stall_timeout,
            clock,
        }
    }
}

impl<S: SensesPort, V: VerifierPort<Frame = S::Frame>, C: Clock> Driver<S, V, C> {
    /// The same driver, with a slow verifier behind its own port.
    pub fn with_verifier<W: VerifierPort<Frame = S::Frame>>(self, verifier: W) -> Driver<S, W, C> {
        Driver {
            monitor: self.monitor,
            senses: self.senses,
            verifier,
            watch: self.watch,
            last_at: self.last_at,
            session: self.session,
            stall_timeout: self.stall_timeout,
            clock: self.clock,
        }
    }
    /// The timing of one sample: its wait in the source, and the time its turn
    /// took. Its turn is read on the driver's clock.
    fn timing(&self, received: Instant, started: Instant) -> Timing {
        let now = self.clock.now();
        Timing {
            wait_ms: started.saturating_duration_since(received).as_secs_f64() * 1000.0,
            processing_ms: now.saturating_duration_since(started).as_secs_f64() * 1000.0,
        }
    }
    /// A reception change, with what the shell knows: the session it reports
    /// and the last sample it saw. The core names no sample it did not see.
    fn report(&mut self, state: SensorState, session: u64, last: Option<SampleRef>) -> Processed {
        let started = self.clock.now();
        let output = self
            .monitor
            .handle(Input::Sensor {
                state,
                session: SessionId(session),
                last,
            })
            .cancel();
        let now = self.clock.now();
        Processed {
            output,
            timing: Timing {
                wait_ms: 0.0,
                processing_ms: now.saturating_duration_since(started).as_secs_f64() * 1000.0,
            },
        }
    }
    pub fn on_stream(&mut self, event: StreamEvent, now: Instant) -> Option<Processed> {
        self.watch.on_stream(&event, now);
        if let StreamEvent::Streaming { session } | StreamEvent::Disconnected { session, .. } =
            &event
        {
            self.session = *session;
        }
        let (session, last) = (self.session, self.last_at);
        stream_watch::translate(&event).map(|state| self.report(state, session, last))
    }
    fn poll_stall(&mut self, now: Instant) -> Option<Processed> {
        self.watch
            .poll(now, self.stall_timeout)
            .then(|| self.report(SensorState::Stalled, self.session, self.last_at))
    }
    /// What arrives from the source: a sample or a transport change.
    pub fn arrive(&mut self, arrival: Arrival<S::Frame>) -> Vec<Processed> {
        match arrival {
            Arrival::Sample {
                at,
                received,
                started,
                frame,
            } => self.frame(at, frame, received, started),
            Arrival::Transport(event) => {
                let now = self.clock.now();
                self.on_stream(event, now).into_iter().collect()
            }
            Arrival::Nothing | Arrival::End => Vec::new(),
        }
    }
    fn frame(
        &mut self,
        at: SampleRef,
        frame: Result<Option<S::Frame>, IngestError>,
        received: Instant,
        started: Instant,
    ) -> Vec<Processed> {
        let _span =
            tracing::info_span!("turn", session = at.session.0, sequence = at.sequence).entered();
        let mut outputs = Vec::with_capacity(2);
        if self.watch.on_sample(started) {
            // The shell sees this sample arrive, so it is the last one it saw.
            outputs.push(self.report(SensorState::Streaming, at.session.0, Some(at)));
        }
        self.settle_verified();
        let output = self.turn_for(at, frame);
        self.last_at = Some(at);
        self.session = at.session.0;
        outputs.push(Processed {
            output,
            timing: self.timing(received, started),
        });
        outputs
    }
    /// The verifier's answers that arrived: evidence of the samples they looked
    /// at, settled by the core at this sample's turn.
    fn settle_verified(&mut self) {
        for verified in self.verifier.answers() {
            self.monitor.verified(verified);
        }
    }
    /// The turn of one sample: the frame is looked at through the router, or
    /// the turn closes without one (empty or failed decode).
    fn turn_for(&mut self, at: SampleRef, frame: Result<Option<S::Frame>, IngestError>) -> Output {
        match frame {
            Ok(Some(frame)) => self
                .monitor
                .handle(Input::Sample {
                    at,
                    frame: frame.info(),
                })
                .drive(|probe| {
                    let _look = tracing::debug_span!("look", kind=?probe.kind()).entered();
                    router::answer(
                        &self.clock,
                        &mut self.senses,
                        &mut self.verifier,
                        &frame,
                        probe,
                    )
                }),
            Ok(None) => self.no_frame(at, NoFrame::Empty),
            Err(error) => {
                tracing::warn!(%error, "decode failed");
                self.no_frame(at, NoFrame::DecodeFailed)
            }
        }
    }
    fn no_frame(&mut self, at: SampleRef, why: NoFrame) -> Output {
        self.monitor.handle(Input::NoFrame { at, why }).cancel()
    }
    /// Drives what arrives from the source until it ends or is cancelled. The
    /// wait is short: the no-sample deadline is checked between arrivals.
    pub fn run(
        &mut self,
        source: &mut impl SampleSource<Frame = S::Frame>,
        token: &CancellationToken,
        mut emit: impl FnMut(Processed),
    ) {
        let poll = Duration::from_millis(50);
        while !token.is_cancelled() {
            let now = self.clock.now();
            if let Some(output) = self.poll_stall(now) {
                emit(output);
            }
            match source.next(poll) {
                Arrival::End => break,
                arrival => self.arrive(arrival).into_iter().for_each(&mut emit),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use crate::source::SampleDecoder;
    use image::DynamicImage;
    use ingest::{Payload, Sample};
    use scene_contract::{LookOutcome, RepublishReason};
    use scene_core::map::FrameInfo;
    use scene_geom::{Quad, Rect};
    use sense::Senses;
    struct Empty;
    impl sense::DetectorPort for Empty {
        fn detect(
            &mut self,
            _: &DynamicImage,
        ) -> Result<Vec<scene_look::Detection>, sense::ModelError> {
            Ok(vec![])
        }
    }
    fn driver() -> Driver<Senses, NoVerifier<DynamicImage>, FixedClock> {
        Driver::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(10.0, 10.0, 50.0, 50.0))),
            Senses::new(Empty),
            Duration::from_secs(3),
            FixedClock::new(),
        )
    }
    fn stamp() -> Stamp {
        Stamp {
            session: 1,
            sequence: 42,
            source_secs: 4.2,
        }
    }
    #[test]
    fn failed_and_empty_decodes_do_not_look() {
        let mut d = driver();
        let t = Instant::now();
        let at = stamp_to_ref(stamp());
        for (frame, reason) in [
            (
                Err(IngestError::Decode("test".into())),
                RepublishReason::DecodeFailed,
            ),
            (Ok(None), RepublishReason::DecodeEmpty),
        ] {
            let p = d.frame(at, frame, t, t).pop().unwrap();
            assert_eq!(p.output.scene.republished.unwrap().reason, reason);
            assert!(
                !p.output
                    .scene
                    .attention
                    .looks
                    .iter()
                    .any(|look| matches!(look.outcome, LookOutcome::Asked { .. }))
            );
            assert!(p.output.events.is_empty());
        }
    }
    #[test]
    fn real_keyframe_and_stamp_translation() {
        let mut d = driver();
        let clock = FixedClock::new();
        let outputs = d.arrive(SampleDecoder::default().arrival(
            Sample {
                stamp: stamp(),
                received_at: clock.now(),
                payload: Payload::Keyframe {
                    data: {
                        let video = include_bytes!("../../ingest/tests/fixtures/testsrc.h264");
                        let end = video
                            .windows(5)
                            .enumerate()
                            .filter(|(_, b)| *b == [0, 0, 0, 1, 9])
                            .nth(1)
                            .unwrap()
                            .0;
                        video[..end].to_vec()
                    },
                    pts_ticks: 378000,
                    clock_rate: 90000,
                },
            },
            &clock,
        ));
        let output = &outputs.last().unwrap().output;
        assert_eq!(output.at, stamp_to_ref(stamp()));
        assert_eq!(
            output.scene.frame,
            Some(FrameInfo {
                width: 160,
                height: 96
            })
        );
        assert!(output.scene.republished.is_none());
        // Both clocks are fixed: the wait and the turn measure zero, exactly.
        assert_eq!(outputs.last().unwrap().timing.wait_ms, 0.0);
        assert_eq!(outputs.last().unwrap().timing.processing_ms, 0.0);
    }
    #[test]
    fn the_sensor_republishes_and_the_core_deduplicates() {
        let mut d = driver();
        let t = Instant::now();
        let first = d
            .on_stream(StreamEvent::Streaming { session: 1 }, t)
            .unwrap();
        assert_eq!(first.output.scene.sensor, SensorState::Streaming);
        assert_eq!(first.output.events.len(), 1);
        assert!(
            d.on_stream(StreamEvent::Streaming { session: 1 }, t)
                .unwrap()
                .output
                .events
                .is_empty()
        );
        let stalled = d.poll_stall(t + Duration::from_secs(3)).unwrap();
        assert_eq!(stalled.output.scene.sensor, SensorState::Stalled);
        assert!(d.poll_stall(t + Duration::from_secs(4)).is_none());
        let recovered = d.frame(
            stamp_to_ref(stamp()),
            Ok(Some(DynamicImage::new_rgb8(160, 96))),
            t,
            t + Duration::from_secs(4),
        );
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].output.scene.sensor, SensorState::Streaming);
        let disconnected = d
            .on_stream(
                StreamEvent::Disconnected {
                    session: 1,
                    reason: IngestError::Protocol("test".into()),
                },
                t,
            )
            .unwrap();
        assert_eq!(disconnected.output.scene.sensor, SensorState::Reconnecting);
        assert_eq!(
            disconnected.output.scene.room,
            recovered[1].output.scene.room
        );
    }
}
