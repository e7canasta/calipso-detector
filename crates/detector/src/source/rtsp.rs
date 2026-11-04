//! The live source: the `ingest` mailbox, its events and on-demand decoding.
//! It takes the freshest sample every `interval`: a property of RTSP sampling,
//! not of the driver.
use super::{Arrival, SampleSource};
use crate::decoder::SessionDecoder;
use crate::driver::stamp_to_ref;
use image::DynamicImage;
use ingest::{Clock, Mailbox, Sample, StreamEvent, Take};
use std::{
    sync::{Arc, mpsc::Receiver},
    time::{Duration, Instant},
};

/// Decodes a received sample into its frame.
#[derive(Debug, Default)]
pub struct SampleDecoder {
    decoder: SessionDecoder,
}

impl SampleDecoder {
    /// The arrival of a sample: its frame, and the instant decoding starts, on
    /// the same clock that stamped its arrival (`wait_ms` is their difference).
    pub fn arrival(&mut self, sample: Sample, clock: &impl Clock) -> Arrival<DynamicImage> {
        let started = clock.now();
        Arrival::Sample {
            at: stamp_to_ref(sample.stamp),
            received: sample.received_at,
            started,
            frame: self.decoder.decode(sample.stamp.session, sample.payload),
        }
    }
}

#[derive(Debug)]
pub struct RtspSource<C: Clock> {
    mailbox: Arc<Mailbox<Sample>>,
    events: Receiver<StreamEvent>,
    decoder: SampleDecoder,
    interval: Duration,
    /// Pacing: the camera's cadence is a schedule, not a measurement.
    next_due: Instant,
    /// A taken sample waiting for earlier events to be delivered.
    pending: Option<Sample>,
    /// The clock of the arrivals: the same one the stream stage stamps with.
    clock: C,
}

impl<C: Clock> RtspSource<C> {
    pub fn new(
        mailbox: Arc<Mailbox<Sample>>,
        events: Receiver<StreamEvent>,
        interval: Duration,
        clock: C,
    ) -> Self {
        Self {
            mailbox,
            events,
            decoder: SampleDecoder::default(),
            interval,
            next_due: Instant::now(),
            pending: None,
            clock,
        }
    }
}

impl<C: Clock> SampleSource for RtspSource<C> {
    type Frame = DynamicImage;

    /// Transport events go before the sample: that way a reconnection is always
    /// reported before the first sample of the new session.
    fn next(&mut self, wait: Duration) -> Arrival<DynamicImage> {
        if let Ok(event) = self.events.try_recv() {
            return Arrival::Transport(event);
        }
        if let Some(sample) = self.pending.take() {
            return self.decoder.arrival(sample, &self.clock);
        }
        if let Some(rest) = self.next_due.checked_duration_since(Instant::now()) {
            std::thread::sleep(rest.min(wait));
            return Arrival::Nothing;
        }
        match self.mailbox.take(wait) {
            Take::Value(sample) => {
                self.next_due = Instant::now() + self.interval;
                self.pending = Some(sample);
                self.next(Duration::ZERO)
            }
            Take::Closed => Arrival::End,
            Take::Timeout => Arrival::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use ingest::{MediaTime, Payload, Session, Stamp};
    use std::sync::mpsc;

    fn sample(n: u64, received_at: Instant) -> Sample {
        Sample {
            stamp: Stamp {
                session: Session(1),
                sequence: ingest::Sequence(n),
                source_secs: ingest::SourceSecs(n as f64),
            },
            received_at,
            payload: Payload::Keyframe {
                data: Vec::new(),
                time: MediaTime {
                    ticks: 0,
                    clock_rate: 90_000.try_into().expect("a clock rate"),
                },
            },
        }
    }

    fn source<C: Clock>(
        interval: Duration,
        clock: C,
    ) -> (
        RtspSource<C>,
        Arc<Mailbox<Sample>>,
        mpsc::Sender<StreamEvent>,
    ) {
        let mailbox = Arc::new(Mailbox::default());
        let (tx, rx) = mpsc::channel();
        (
            RtspSource::new(Arc::clone(&mailbox), rx, interval, clock),
            mailbox,
            tx,
        )
    }

    /// A clock the test moves by hand, to measure a wait that is not zero.
    #[derive(Debug)]
    struct ManualClock(std::cell::Cell<Instant>);

    impl Clock for ManualClock {
        fn now(&self) -> Instant {
            self.0.get()
        }
    }

    #[test]
    fn transport_events_arrive_before_the_sample() {
        let clock = FixedClock::new();
        let (mut s, mailbox, tx) = source(Duration::ZERO, clock);
        mailbox.put(sample(7, clock.now()));
        tx.send(StreamEvent::Streaming {
            session: Session(1),
        })
        .unwrap();
        assert!(matches!(
            s.next(Duration::from_millis(10)),
            Arrival::Transport(_)
        ));
        let Arrival::Sample { at, .. } = s.next(Duration::from_millis(10)) else {
            panic!("the sample was expected");
        };
        assert_eq!(at.sequence, 7);
    }

    #[test]
    fn after_a_sample_it_waits_for_the_interval() {
        let clock = FixedClock::new();
        let (mut s, mailbox, _tx) = source(Duration::from_secs(3600), clock);
        mailbox.put(sample(1, clock.now()));
        assert!(matches!(
            s.next(Duration::from_millis(10)),
            Arrival::Sample { .. }
        ));
        mailbox.put(sample(2, clock.now()));
        assert!(matches!(s.next(Duration::from_millis(1)), Arrival::Nothing));
    }

    #[test]
    fn a_closed_mailbox_ends_the_source() {
        let (mut s, mailbox, _tx) = source(Duration::ZERO, FixedClock::new());
        mailbox.close();
        assert!(matches!(s.next(Duration::from_millis(10)), Arrival::End));
    }

    #[test]
    fn the_wait_is_the_time_on_the_clock_between_arrival_and_decoding() {
        let clock = ManualClock(std::cell::Cell::new(Instant::now()));
        let received = clock.now();
        clock.0.set(received + Duration::from_millis(250));
        let Arrival::Sample {
            received: got,
            started,
            ..
        } = SampleDecoder::default().arrival(sample(9, received), &clock)
        else {
            panic!("the sample was expected");
        };
        assert_eq!(
            got, received,
            "the arrival keeps the instant it was stamped"
        );
        assert_eq!(
            started.saturating_duration_since(got),
            Duration::from_millis(250),
            "the wait is measured on the injected clock"
        );
    }
}
