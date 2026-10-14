//! Translates and delivers: every scene decision belongs to Monitor. The
//! driver does not know where samples come from or who looks: a
//! [`SampleSource`] and a [`SensesPort`] are injected into it.
use crate::senses::{Frame, SensesPort};
use crate::sensor::{self, Sensor};
use crate::source::{Arrival, SampleSource};
use crate::verifier::{NoVerifier, VerifierPort};
use ingest::{IngestError, Stamp, StreamEvent};
use scene_core::{
    Input, Monitor, NoFrame, Output,
    contract::{SampleRef, SensorState, SessionId, SourceSecs, Timing},
    look::{Probe, Reading, Spent},
    map::SiteMap,
    policy::Policy,
};
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
/// The driver of one camera. By default, with the live senses and no
/// verifier.
#[derive(Debug)]
pub struct Driver<
    S: SensesPort = sense::Senses,
    V: VerifierPort<Frame = <S as SensesPort>::Frame> = NoVerifier<<S as SensesPort>::Frame>,
> {
    monitor: Monitor,
    senses: S,
    verifier: V,
    sensor: Sensor,
    last_at: Option<SampleRef>,
    session: u64,
    stall_timeout: Duration,
}
impl<S: SensesPort> Driver<S> {
    pub fn new(policy: Policy, map: SiteMap, senses: S, stall_timeout: Duration) -> Self {
        Self {
            monitor: Monitor::new(policy, map),
            senses,
            verifier: NoVerifier::default(),
            sensor: Sensor::new(Instant::now()),
            last_at: None,
            session: 0,
            stall_timeout,
        }
    }
}

impl<S: SensesPort, V: VerifierPort<Frame = S::Frame>> Driver<S, V> {
    /// The same driver, with a slow verifier behind its own port.
    pub fn with_verifier<W: VerifierPort<Frame = S::Frame>>(self, verifier: W) -> Driver<S, W> {
        Driver {
            monitor: self.monitor,
            senses: self.senses,
            verifier,
            sensor: self.sensor,
            last_at: self.last_at,
            session: self.session,
            stall_timeout: self.stall_timeout,
        }
    }
    fn sensor_at(&self) -> SampleRef {
        self.last_at.unwrap_or(SampleRef {
            session: SessionId(self.session),
            sequence: 0,
            source_secs: SourceSecs(0.0),
        })
    }
    fn report(&mut self, at: SampleRef, state: SensorState) -> Processed {
        let started = Instant::now();
        let output = self.monitor.handle(Input::Sensor { at, state }).cancel();
        Processed {
            output,
            timing: Timing {
                wait_ms: 0.0,
                processing_ms: started.elapsed().as_secs_f64() * 1000.0,
            },
        }
    }
    pub fn on_stream(&mut self, event: StreamEvent, now: Instant) -> Option<Processed> {
        self.sensor.on_stream(&event, now);
        if let StreamEvent::Streaming { session } | StreamEvent::Disconnected { session, .. } =
            &event
        {
            self.session = *session;
        }
        sensor::translate(&event).map(|state| self.report(self.sensor_at(), state))
    }
    fn poll_stall(&mut self, now: Instant) -> Option<Processed> {
        self.sensor
            .poll(now, self.stall_timeout)
            .then(|| self.report(self.sensor_at(), SensorState::Stalled))
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
                self.on_stream(event, Instant::now()).into_iter().collect()
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
        if self.sensor.on_sample(started) {
            outputs.push(self.report(at, SensorState::Streaming));
        }
        // The verifier's answers that arrived: evidence of the samples they
        // looked at, settled by the core at this sample's turn.
        for verified in self.verifier.answers() {
            self.monitor.verified(verified);
        }
        let (senses, verifier) = (&mut self.senses, &mut self.verifier);
        let output = match frame {
            Ok(Some(frame)) => self
                .monitor
                .handle(Input::Sample {
                    at,
                    frame: frame.info(),
                })
                .drive(|probe| {
                    let _look = tracing::debug_span!("look", kind=?probe.kind()).entered();
                    match probe {
                        Probe::Verify {
                            ticket,
                            region,
                            bed,
                            question,
                            crops,
                        } => {
                            let started = Instant::now();
                            let taken =
                                verifier.ask(&frame, *ticket, *region, bed, *question, crops);
                            let spent = Spent(started.elapsed().as_secs_f64() * 1000.0);
                            (taken.map(|()| Reading::Dispatched), spent)
                        }
                        Probe::Keep { ticket, region } => {
                            let started = Instant::now();
                            let kept = verifier.keep(&frame, *ticket, *region);
                            let spent = Spent(started.elapsed().as_secs_f64() * 1000.0);
                            (kept.map(|sharpness| Reading::Kept { sharpness }), spent)
                        }
                        _ => senses.look(&frame, probe),
                    }
                }),
            Ok(None) => self
                .monitor
                .handle(Input::NoFrame {
                    at,
                    why: NoFrame::Empty,
                })
                .cancel(),
            Err(error) => {
                tracing::warn!(%error,"decode failed");
                self.monitor
                    .handle(Input::NoFrame {
                        at,
                        why: NoFrame::DecodeFailed,
                    })
                    .cancel()
            }
        };
        self.last_at = Some(at);
        self.session = at.session.0;
        outputs.push(Processed {
            output,
            timing: Timing {
                wait_ms: started.saturating_duration_since(received).as_secs_f64() * 1000.0,
                processing_ms: started.elapsed().as_secs_f64() * 1000.0,
            },
        });
        outputs
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
            if let Some(output) = self.poll_stall(Instant::now()) {
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
    use crate::source::rtsp::SampleDecoder;
    use image::DynamicImage;
    use ingest::{Payload, Sample};
    use scene_core::map::FrameInfo;
    use scene_core::{
        contract::{LookOutcome, RepublishReason},
        geom::{Quad, Rect},
    };
    use sense::Senses;
    struct Empty;
    impl sense::DetectorPort for Empty {
        fn detect(
            &mut self,
            _: &DynamicImage,
        ) -> Result<Vec<scene_core::look::Detection>, sense::ModelError> {
            Ok(vec![])
        }
    }
    fn driver() -> Driver {
        Driver::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(10.0, 10.0, 50.0, 50.0))),
            Senses::new(Empty),
            Duration::from_secs(3),
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
        let outputs = d.arrive(SampleDecoder::default().arrival(Sample {
            stamp: stamp(),
            received_at: Instant::now(),
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
        }));
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
        assert!(outputs.last().unwrap().timing.processing_ms >= 0.0);
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
