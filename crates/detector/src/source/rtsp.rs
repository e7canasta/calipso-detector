//! The live source: the `ingest` mailbox, its events and on-demand decoding.
//! It takes the freshest sample every `interval`: a property of RTSP sampling,
//! not of the driver.
use super::{Arrival, SampleSource};
use crate::decoder::SessionDecoder;
use crate::driver::stamp_to_ref;
use image::DynamicImage;
use ingest::{Mailbox, Sample, StreamEvent, Take};
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
    pub fn arrival(&mut self, sample: Sample) -> Arrival<DynamicImage> {
        let started = Instant::now();
        Arrival::Sample {
            at: stamp_to_ref(sample.stamp),
            received: sample.received_at,
            started,
            frame: self.decoder.decode(sample.stamp.session, sample.payload),
        }
    }
}

#[derive(Debug)]
pub struct RtspSource {
    mailbox: Arc<Mailbox<Sample>>,
    events: Receiver<StreamEvent>,
    decoder: SampleDecoder,
    interval: Duration,
    next_due: Instant,
    /// A taken sample waiting for earlier events to be delivered.
    pending: Option<Sample>,
}

impl RtspSource {
    pub fn new(
        mailbox: Arc<Mailbox<Sample>>,
        events: Receiver<StreamEvent>,
        interval: Duration,
    ) -> Self {
        Self {
            mailbox,
            events,
            decoder: SampleDecoder::default(),
            interval,
            next_due: Instant::now(),
            pending: None,
        }
    }
}

impl SampleSource for RtspSource {
    type Frame = DynamicImage;

    /// Transport events go before the sample: that way a reconnection is always
    /// reported before the first sample of the new session.
    fn next(&mut self, wait: Duration) -> Arrival<DynamicImage> {
        if let Ok(event) = self.events.try_recv() {
            return Arrival::Transport(event);
        }
        if let Some(sample) = self.pending.take() {
            return self.decoder.arrival(sample);
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
    use ingest::{Payload, Stamp};
    use std::sync::mpsc;

    fn sample(n: u64) -> Sample {
        Sample {
            stamp: Stamp {
                session: 1,
                sequence: n,
                source_secs: n as f64,
            },
            received_at: Instant::now(),
            payload: Payload::Keyframe {
                data: Vec::new(),
                pts_ticks: 0,
                clock_rate: 90_000,
            },
        }
    }

    fn source(interval: Duration) -> (RtspSource, Arc<Mailbox<Sample>>, mpsc::Sender<StreamEvent>) {
        let mailbox = Arc::new(Mailbox::default());
        let (tx, rx) = mpsc::channel();
        (
            RtspSource::new(Arc::clone(&mailbox), rx, interval),
            mailbox,
            tx,
        )
    }

    #[test]
    fn transport_events_arrive_before_the_sample() {
        let (mut s, mailbox, tx) = source(Duration::ZERO);
        mailbox.put(sample(7));
        tx.send(StreamEvent::Streaming { session: 1 }).unwrap();
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
        let (mut s, mailbox, _tx) = source(Duration::from_secs(3600));
        mailbox.put(sample(1));
        assert!(matches!(
            s.next(Duration::from_millis(10)),
            Arrival::Sample { .. }
        ));
        mailbox.put(sample(2));
        assert!(matches!(s.next(Duration::from_millis(1)), Arrival::Nothing));
    }

    #[test]
    fn a_closed_mailbox_ends_the_source() {
        let (mut s, mailbox, _tx) = source(Duration::ZERO);
        mailbox.close();
        assert!(matches!(s.next(Duration::from_millis(10)), Arrival::End));
    }
}
