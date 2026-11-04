//! Translates and delivers: every scene decision belongs to Monitor. The
//! driver does not know where samples come from or who looks: a
//! [`SampleSource`] and a [`SensesPort`] are injected into it.
use crate::clock::{Clock, SystemClock};
use crate::config::Config;
use crate::router;
use crate::senses::{Frame, SensesPort};
use crate::source::{Arrival, SampleSource};
use crate::stream_watch::{self, StreamWatch};
use crate::verifier::{NoVerifier, VerifierPort};
use anyhow::Result;
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

/// The driver of one camera from its profile: the models load and the driver
/// composes over them. Shared by the camera binary and the frames one.
/// `stall_timeout`: the stall watchdog's deadline; `None`, no watchdog —
/// there is no camera to stall (rule 12: what is not looked at has a reason).
pub fn build_driver(config: &Config, stall_timeout: Option<Duration>) -> Result<Driver> {
    let senses = crate::senses::load(config)?;
    Ok(Driver::new(
        config.policy()?,
        config.site_map(),
        senses,
        stall_timeout,
        SystemClock,
    ))
}

/// Both types belong to other crates: Rust does not allow impl From here.
pub(crate) fn stamp_to_ref(stamp: Stamp) -> SampleRef {
    SampleRef {
        session: SessionId(stamp.session.0),
        sequence: stamp.sequence.0,
        source_secs: SourceSecs(stamp.source_secs.0),
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
    /// The stall watchdog's deadline; `None`: no watchdog at all.
    stall_timeout: Option<Duration>,
    clock: C,
}
impl<S: SensesPort, C: Clock> Driver<S, NoVerifier<S::Frame>, C> {
    /// `stall_timeout`: the deadline of the stall watchdog; `None`, no
    /// watchdog — over frames there is no camera to stall.
    pub fn new(
        policy: Policy,
        map: SiteMap,
        senses: S,
        stall_timeout: Option<Duration>,
        clock: C,
    ) -> Self {
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
            self.session = session.0;
        }
        let (session, last) = (self.session, self.last_at);
        stream_watch::translate(&event).map(|state| self.report(state, session, last))
    }
    /// The watchdog's verdict: `None` without a timeout (nothing to stall) or
    /// when nothing stalled.
    fn poll_stall(&mut self, now: Instant) -> Option<Processed> {
        let timeout = self.stall_timeout?;
        if self.watch.poll(now, timeout) {
            Some(self.report(SensorState::Stalled, self.session, self.last_at))
        } else {
            None
        }
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
            Some(Duration::from_secs(3)),
            FixedClock::new(),
        )
    }
    fn stamp() -> Stamp {
        Stamp {
            session: ingest::Session(1),
            sequence: ingest::Sequence(42),
            source_secs: ingest::SourceSecs(4.2),
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
            assert_eq!(p.output.v5.scene.republished.unwrap().reason, reason);
            assert!(
                !p.output
                    .v5
                    .scene
                    .attention
                    .looks
                    .iter()
                    .any(|look| matches!(look.outcome, LookOutcome::Asked { .. }))
            );
            assert!(p.output.v5.events.is_empty());
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
                    time: ingest::MediaTime {
                        ticks: 378000,
                        clock_rate: std::num::NonZeroU32::new(90_000).expect("a clock rate"),
                    },
                },
            },
            &clock,
        ));
        let output = &outputs.last().unwrap().output;
        assert_eq!(output.at, Some(stamp_to_ref(stamp())));
        assert_eq!(
            output.v5.scene.frame,
            Some(FrameInfo {
                width: 160,
                height: 96
            })
        );
        assert!(output.v5.scene.republished.is_none());
        // Both clocks are fixed: the wait and the turn measure zero, exactly.
        assert_eq!(outputs.last().unwrap().timing.wait_ms, 0.0);
        assert_eq!(outputs.last().unwrap().timing.processing_ms, 0.0);
    }
    #[test]
    fn the_sensor_republishes_and_the_core_deduplicates() {
        let mut d = driver();
        let t = Instant::now();
        let first = d
            .on_stream(
                StreamEvent::Streaming {
                    session: ingest::Session(1),
                },
                t,
            )
            .unwrap();
        assert_eq!(first.output.v5.scene.sensor, SensorState::Streaming);
        assert_eq!(first.output.v5.events.len(), 1);
        assert!(
            d.on_stream(
                StreamEvent::Streaming {
                    session: ingest::Session(1)
                },
                t
            )
            .unwrap()
            .output
            .v5
            .events
            .is_empty()
        );
        let stalled = d.poll_stall(t + Duration::from_secs(3)).unwrap();
        assert_eq!(stalled.output.v5.scene.sensor, SensorState::Stalled);
        assert!(d.poll_stall(t + Duration::from_secs(4)).is_none());
        let recovered = d.frame(
            stamp_to_ref(stamp()),
            Ok(Some(DynamicImage::new_rgb8(160, 96))),
            t,
            t + Duration::from_secs(4),
        );
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].output.v5.scene.sensor, SensorState::Streaming);
        let disconnected = d
            .on_stream(
                StreamEvent::Disconnected {
                    session: ingest::Session(1),
                    reason: IngestError::Protocol("test".into()),
                },
                t,
            )
            .unwrap();
        assert_eq!(
            disconnected.output.v5.scene.sensor,
            SensorState::Reconnecting
        );
        assert_eq!(
            disconnected.output.v5.scene.room,
            recovered[1].output.v5.scene.room
        );
    }
}
