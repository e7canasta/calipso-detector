use crate::IngestError;

type Result<T> = std::result::Result<T, IngestError>;
use futures::StreamExt;
use retina::{client::SetupOptions, codec::CodecItem};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::{
    Clock, Mailbox, Payload, Sample, StreamEvent,
    decode::{FrameDecoder, Picture},
};
use crate::{IngestConfig, MediaTime, Sampling, Sequence, Session, SourceSecs, Stamp};

const STATS_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Default)]
struct IngestStats {
    received: u64,
    keyframes: u64,
    published: u64,
    decode_errors: u64,
    packet_errors: u64,
    last_report: Option<Instant>,
}

impl IngestStats {
    /// Logs the counters if a stats interval has passed since the last report;
    /// whether it did.
    fn report_if_due<C: Clock>(&mut self, clock: &C, overwritten: u64) -> bool {
        let now = clock.now();
        if self
            .last_report
            .is_some_and(|last| now.duration_since(last) < STATS_INTERVAL)
        {
            return false;
        }
        self.last_report = Some(now);
        info!(
            "ingest: received={} keyframes={} published={} overwritten={} decode_errors={} packet_errors={}",
            self.received,
            self.keyframes,
            self.published,
            overwritten,
            self.decode_errors,
            self.packet_errors,
        );
        true
    }
}

/// Runs reception on a dedicated thread with its own current-thread runtime,
/// so retina and FFmpeg objects never cross threads.
pub fn spawn<C: Clock + Send + 'static>(
    config: IngestConfig,
    mailbox: Arc<Mailbox<Sample>>,
    events: mpsc::Sender<StreamEvent>,
    shutdown: CancellationToken,
    clock: C,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create stream runtime");
        runtime.block_on(run(config, mailbox, events, shutdown, clock));
    })
}

async fn run<C: Clock>(
    config: IngestConfig,
    mailbox: Arc<Mailbox<Sample>>,
    events: mpsc::Sender<StreamEvent>,
    shutdown: CancellationToken,
    clock: C,
) {
    let creds = config
        .credentials
        .as_ref()
        .map(|credentials| retina::client::Credentials {
            username: credentials.username.clone(),
            password: credentials.password.clone(),
        });
    let mut stats = IngestStats::default();
    let mut session = Session(0);
    let mut backoff = ReconnectBackoff::new(config.reconnect_delay);

    loop {
        session.0 += 1;
        let _ = events.send(StreamEvent::Connecting);
        info!("Connecting to {} (session {session})", config.url);
        let result = tokio::select! {
            result = stream_session(&config, creds.clone(), session, &mailbox, &events, &mut stats, &clock) => result,
            _ = shutdown.cancelled() => return,
        };
        let reason = match result {
            Ok(()) => IngestError::Unreachable("end of stream".to_owned()),
            Err(error) => error,
        };
        let wait = backoff.next(clock.now());
        error!(
            "Stream session {session} ended: {reason}; reconnecting in {}s",
            wait.as_secs_f64()
        );
        let _ = events.send(StreamEvent::Disconnected { session, reason });
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = shutdown.cancelled() => return,
        }
    }
}

async fn stream_session<C: Clock>(
    config: &IngestConfig,
    creds: Option<retina::client::Credentials>,
    session_id: Session,
    mailbox: &Mailbox<Sample>,
    events: &mpsc::Sender<StreamEvent>,
    stats: &mut IngestStats,
    clock: &C,
) -> Result<()> {
    let demuxed = handshake(config, creds).await?;
    info!("Streaming (session {session_id})");
    let _ = events.send(StreamEvent::Streaming {
        session: session_id,
    });
    demux(
        demuxed,
        SampleGate::new(config.sampling),
        session_id,
        mailbox,
        stats,
        clock,
    )
    .await
}

/// The RTSP handshake: DESCRIBE, then SETUP and PLAY on the video stream.
/// The session is streaming when this returns.
async fn handshake(
    config: &IngestConfig,
    creds: Option<retina::client::Credentials>,
) -> Result<retina::client::Demuxed> {
    let mut session = retina::client::Session::describe(
        config.url.clone(),
        retina::client::SessionOptions::default()
            .creds(creds)
            .session_group(Arc::new(retina::client::SessionGroup::default()))
            .user_agent("RTSP Person Detector".to_owned())
            .teardown(retina::client::TeardownPolicy::Auto),
    )
    .await
    .map_err(IngestError::connecting)?;

    let video_stream_i = session
        .streams()
        .iter()
        .position(|stream| stream.media() == "video" && stream.encoding_name() == "h264")
        .ok_or_else(|| IngestError::Protocol("No supported H264 video stream found".to_owned()))?;
    session
        .setup(
            video_stream_i,
            SetupOptions::default()
                .transport(retina::client::Transport::Tcp(
                    retina::client::TcpTransportOptions::default(),
                ))
                .frame_format(retina::codec::FrameFormat::SIMPLE),
        )
        .await
        .map_err(IngestError::streaming)?;
    session
        .play(retina::client::PlayOptions::default())
        .await
        .map_err(IngestError::streaming)?
        .demuxed()
        .map_err(IngestError::streaming)
}

/// Consumes the demuxed stream until it ends: every good item resets the
/// error budget, a bad packet costs its access unit, and each frame goes
/// through the sampling gate.
async fn demux<C: Clock>(
    mut demuxed: retina::client::Demuxed,
    mut gate: SampleGate,
    session_id: Session,
    mailbox: &Mailbox<Sample>,
    stats: &mut IngestStats,
    clock: &C,
) -> Result<()> {
    // Continuous decoding only when every frame is a candidate sample.
    let mut decoder: Option<FrameDecoder> = None;
    let mut errors = ErrorBudget::default();

    while let Some(item) = demuxed.next().await {
        let item = match item {
            Ok(item) => {
                errors.reset();
                item
            }
            // Retina restores its depacketizer after a bad packet, so a bad
            // packet costs its access unit, not the session. Joining mid-NAL
            // yields one error per remaining fragment; only errors without
            // any good item for a while mean the connection is gone.
            Err(error) if errors.tolerate(clock) => {
                stats.packet_errors += 1;
                let summary = error.to_string();
                warn!(
                    "Packet dropped (session {session_id}): {}",
                    summary.lines().next().unwrap_or_default()
                );
                // Later frames reference the lost one.
                decoder = None;
                gate.lost();
                continue;
            }
            Err(error) => return Err(IngestError::streaming(error)),
        };
        let CodecItem::VideoFrame(frame) = item else {
            continue;
        };
        stats.received += 1;
        let is_keyframe = frame.is_random_access_point();
        stats.keyframes += u64::from(is_keyframe);
        let timestamp = frame.timestamp();
        let time = MediaTime {
            ticks: timestamp.elapsed(),
            clock_rate: timestamp.clock_rate(),
        };
        let sample = Stamp {
            session: session_id,
            sequence: Sequence(stats.received),
            source_secs: SourceSecs(timestamp.elapsed_secs()),
        };

        match gate.act(is_keyframe) {
            FrameAction::Publish => {
                publish(
                    mailbox,
                    stats,
                    sample,
                    Payload::Keyframe {
                        data: frame.into_data(),
                        time,
                    },
                    clock,
                )?;
            }
            FrameAction::Decode => {
                let decoder_ref = match decoder.as_mut() {
                    Some(decoder) => decoder,
                    None => decoder.insert(FrameDecoder::h264(time.clock_rate)?),
                };
                match decode_unit(decoder_ref, frame.data(), time, mailbox.is_waiting()) {
                    Ok(Some(picture)) => {
                        let sample = Stamp {
                            source_secs: SourceSecs(
                                picture.source_time_secs.unwrap_or(sample.source_secs.0),
                            ),
                            ..sample
                        };
                        publish(mailbox, stats, sample, Payload::Picture(picture), clock)?;
                    }
                    Ok(None) => {}
                    // A corrupt unit costs the frames up to the next keyframe,
                    // not the RTSP session.
                    Err(error) => {
                        stats.decode_errors += 1;
                        warn!(
                            "Decode failed (session {session_id}): {error}; resyncing at the next keyframe"
                        );
                        decoder = None;
                        gate.lost();
                    }
                }
            }
            FrameAction::Skip => {}
        }
        stats.report_if_due(clock, mailbox.overwritten());
    }
    Ok(())
}

/// The decode pipeline of the continuous mode: one push per access unit and
/// a picture only for the frames the engine is waiting for.
fn decode_unit(
    decoder: &mut FrameDecoder,
    data: &[u8],
    time: MediaTime,
    waiting: bool,
) -> Result<Option<Picture>> {
    // Convert to RGB only when the engine is waiting for a sample.
    let decoded = decoder.push(data, time)?;
    match decoded.filter(|_| waiting) {
        Some(decoded) => decoder.picture(&decoded).map(Some),
        None => Ok(None),
    }
}

/// What a demuxed frame becomes: a self-contained sample to publish, a frame
/// for the decode pipeline, or nothing yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameAction {
    Publish,
    Decode,
    Skip,
}

/// Which frames become samples: the keyframe mode publishes access units for
/// the consumer to decode; the continuous mode decodes every frame the sync
/// admits.
#[derive(Debug)]
struct SampleGate {
    sampling: Sampling,
    sync: KeyframeSync,
}

impl SampleGate {
    fn new(sampling: Sampling) -> Self {
        Self {
            sampling,
            sync: KeyframeSync::default(),
        }
    }

    /// What this frame becomes; in continuous mode the sync starts at the
    /// first keyframe.
    fn act(&mut self, is_keyframe: bool) -> FrameAction {
        match self.sampling {
            Sampling::KeyframesOnly => {
                if is_keyframe {
                    FrameAction::Publish
                } else {
                    FrameAction::Skip
                }
            }
            Sampling::Continuous => {
                if self.sync.admit(is_keyframe) {
                    FrameAction::Decode
                } else {
                    FrameAction::Skip
                }
            }
        }
    }

    /// The decoder state was dropped; wait for the next keyframe.
    fn lost(&mut self) {
        self.sync.lost();
    }
}

/// Continuous decoding starts at a keyframe and, after a decode error,
/// restarts at the next one: frames in between reference lost state.
#[derive(Debug, Default)]
struct KeyframeSync {
    synced: bool,
}

impl KeyframeSync {
    /// Whether this frame can be decoded.
    fn admit(&mut self, is_keyframe: bool) -> bool {
        self.synced |= is_keyframe;
        self.synced
    }

    /// The decoder state was dropped; wait for the next keyframe.
    fn lost(&mut self) {
        self.synced = false;
    }
}

/// How long errors may go on without a good item before the session ends.
const ERROR_WINDOW: Duration = Duration::from_secs(2);
/// Errors in a row that end the session even within the window (a stream
/// that only returns errors would otherwise spin).
const MAX_ERRORS_IN_A_ROW: u32 = 1000;

/// Tolerates stream errors while good items keep arriving.
#[derive(Default)]
struct ErrorBudget {
    in_a_row: u32,
    since: Option<Instant>,
}

impl ErrorBudget {
    /// Records an error at the clock's instant; whether the session may go on.
    fn tolerate<C: Clock>(&mut self, clock: &C) -> bool {
        let now = clock.now();
        self.in_a_row += 1;
        let since = *self.since.get_or_insert(now);
        self.in_a_row < MAX_ERRORS_IN_A_ROW && now.duration_since(since) < ERROR_WINDOW
    }

    /// A good item arrived.
    fn reset(&mut self) {
        (self.in_a_row, self.since) = (0, None);
    }
}

/// Eight times the configured delay: where the reconnect wait stops growing.
const MAX_RECONNECT_MULTIPLIER: u32 = 8;
/// Twice that, as the quiet between two failures that counts a session as
/// healthy again and starts the growth over.
const HEALTHY_GAP_MULTIPLIER: u32 = 2 * MAX_RECONNECT_MULTIPLIER;
/// The waits carry jitter in their top half: several cameras that lost the
/// same server do not retry in lockstep.
const JITTER: f64 = 0.5;

/// How long to wait before the next connection attempt: the configured delay,
/// doubling when one failure follows another closely, up to a cap. This is
/// the declared change of degradation mode: a server that is down is retried
/// less and less often instead of every `reconnect_delay`.
#[derive(Debug)]
struct ReconnectBackoff {
    base: Duration,
    jitter: f64,
    rng: u64,
    failures: u32,
    last: Option<Instant>,
}

impl ReconnectBackoff {
    /// Grows from `base`, with the process's own jitter stream.
    fn new(base: Duration) -> Self {
        Self {
            base,
            jitter: JITTER,
            rng: jitter_seed(),
            failures: 0,
            last: None,
        }
    }

    /// The wait before the next attempt, after the failure at `now`. Two
    /// failures separated by a healthy gap are not consecutive: a session
    /// that ran starts the growth over.
    fn next(&mut self, now: Instant) -> Duration {
        let window = self.base.saturating_mul(HEALTHY_GAP_MULTIPLIER);
        let consecutive = self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) <= window);
        self.failures = if consecutive {
            self.failures.saturating_add(1)
        } else {
            0
        };
        self.last = Some(now);
        let doubled = self.base.saturating_mul(1 << self.failures.min(16));
        let delay = doubled.min(self.base.saturating_mul(MAX_RECONNECT_MULTIPLIER));
        self.jittered(delay)
    }

    /// The delay with jitter uniformly in its top half: `[d/2, d]`.
    fn jittered(&mut self, delay: Duration) -> Duration {
        if self.jitter <= 0.0 {
            return delay;
        }
        // xorshift64: a pseudo-random stream of this backoff, no dependency.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        let fraction = (x >> 11) as f64 / (1u64 << 53) as f64;
        delay.mul_f64(1.0 - self.jitter * fraction)
    }
}

/// A non-zero seed from the process's randomized hash state.
fn jitter_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0x9E37_79B9_7F4A_7C15);
    hasher.finish() | 1
}

fn publish<C: Clock>(
    mailbox: &Mailbox<Sample>,
    stats: &mut IngestStats,
    sample: Stamp,
    payload: Payload,
    clock: &C,
) -> Result<()> {
    let delivered = mailbox.put(Sample {
        stamp: sample,
        received_at: clock.now(),
        payload,
    });
    if !delivered {
        return Err(IngestError::Protocol("engine mailbox closed".to_owned()));
    }
    stats.published += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Clock, ERROR_WINDOW, ErrorBudget, FrameAction, IngestStats, KeyframeSync,
        MAX_ERRORS_IN_A_ROW, MAX_RECONNECT_MULTIPLIER, ReconnectBackoff, STATS_INTERVAL,
        SampleGate,
    };
    use crate::Sampling;
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    /// A clock that moves only when a test says so: no sleeps, no system time.
    /// The origin is only a starting point; every measured time is `elapsed`.
    struct ManualClock {
        origin: Instant,
        elapsed: Cell<Duration>,
    }

    impl ManualClock {
        fn new() -> Self {
            Self {
                origin: Instant::now(),
                elapsed: Cell::new(Duration::ZERO),
            }
        }

        fn at(&self, elapsed: Duration) {
            self.elapsed.set(elapsed);
        }

        fn elapsed(&self) -> Duration {
            self.elapsed.get()
        }
    }

    impl super::Clock for ManualClock {
        fn now(&self) -> Instant {
            self.origin + self.elapsed.get()
        }
    }

    #[test]
    fn a_burst_of_fragment_errors_is_tolerated() {
        // Joining mid-NAL: one error per remaining fragment, all at once.
        let clock = ManualClock::new();
        let mut budget = ErrorBudget::default();
        assert!((0..200).all(|_| budget.tolerate(&clock)));
        budget.reset();
        clock.at(ERROR_WINDOW * 10);
        assert!(budget.tolerate(&clock), "a good item resets the window");
    }

    #[test]
    fn errors_without_good_items_end_the_session() {
        let clock = ManualClock::new();
        let mut budget = ErrorBudget::default();
        assert!(budget.tolerate(&clock));
        clock.at(Duration::from_millis(1900));
        assert!(budget.tolerate(&clock));
        clock.at(ERROR_WINDOW);
        assert!(!budget.tolerate(&clock));

        let mut spinning = ErrorBudget::default();
        let tolerated = (0..MAX_ERRORS_IN_A_ROW * 2)
            .take_while(|_| spinning.tolerate(&clock))
            .count();
        assert_eq!(tolerated as u32, MAX_ERRORS_IN_A_ROW - 1);
    }

    #[test]
    fn stats_report_at_most_once_per_interval() {
        let clock = ManualClock::new();
        let mut stats = IngestStats::default();
        assert!(stats.report_if_due(&clock, 0), "the first report is due");
        clock.at(STATS_INTERVAL - Duration::from_millis(1));
        assert!(!stats.report_if_due(&clock, 0), "not a whole interval yet");
        clock.at(STATS_INTERVAL);
        assert!(
            stats.report_if_due(&clock, 0),
            "an interval after the last report"
        );
        clock.at(STATS_INTERVAL * 2 - Duration::from_millis(1));
        assert!(!stats.report_if_due(&clock, 0));
        clock.at(STATS_INTERVAL * 2);
        assert!(stats.report_if_due(&clock, 0));
    }

    #[test]
    fn decoding_starts_at_a_keyframe_and_resyncs_at_the_next_one() {
        let mut sync = KeyframeSync::default();
        assert!(
            !sync.admit(false),
            "nothing decodes before the first keyframe"
        );
        assert!(sync.admit(true));
        assert!(sync.admit(false));
        sync.lost();
        assert!(
            !sync.admit(false),
            "frames after a decode error are skipped"
        );
        assert!(sync.admit(true));
        assert!(sync.admit(false));
    }

    #[test]
    fn keyframes_only_publishes_only_keyframes() {
        let mut gate = SampleGate::new(Sampling::KeyframesOnly);
        assert_eq!(gate.act(false), FrameAction::Skip);
        assert_eq!(gate.act(true), FrameAction::Publish);
        assert_eq!(gate.act(false), FrameAction::Skip);
    }

    #[test]
    fn continuous_decodes_from_the_first_keyframe_on() {
        let mut gate = SampleGate::new(Sampling::Continuous);
        assert_eq!(gate.act(false), FrameAction::Skip);
        assert_eq!(gate.act(true), FrameAction::Decode);
        assert_eq!(gate.act(false), FrameAction::Decode);
        gate.lost();
        assert_eq!(gate.act(false), FrameAction::Skip, "wait for a keyframe");
        assert_eq!(gate.act(true), FrameAction::Decode);
    }

    /// The backoff the tests measure exactly: no jitter.
    fn unjittered(base: Duration) -> ReconnectBackoff {
        ReconnectBackoff {
            jitter: 0.0,
            rng: 1,
            ..ReconnectBackoff::new(base)
        }
    }

    #[test]
    fn reconnection_waits_grow_and_stop_at_the_cap() {
        let clock = ManualClock::new();
        let mut backoff = unjittered(Duration::from_secs(1));
        // Handshake failures, one right after the other.
        let mut waits = Vec::new();
        for _ in 0..6 {
            let wait = backoff.next(clock.now());
            waits.push(wait);
            // The next attempt fails as soon as its wait is over.
            clock.at(clock.elapsed() + wait);
        }
        assert_eq!(
            waits,
            [
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(8),
                Duration::from_secs(8),
            ],
            "the growth stops at {}x the base",
            MAX_RECONNECT_MULTIPLIER
        );
    }

    #[test]
    fn a_session_that_lived_starts_the_growth_over() {
        let clock = ManualClock::new();
        let mut backoff = unjittered(Duration::from_secs(1));
        for _ in 0..3 {
            let wait = backoff.next(clock.now());
            clock.at(clock.elapsed() + wait);
        }
        // The next failure comes after a healthy gap: not a consecutive one.
        clock.at(clock.elapsed() + Duration::from_secs(100));
        assert_eq!(backoff.next(clock.now()), Duration::from_secs(1));
    }

    #[test]
    fn the_wait_carries_jitter_in_its_top_half() {
        let mut backoff = ReconnectBackoff::new(Duration::from_secs(4));
        let mut expected = Duration::from_secs(4);
        for _ in 0..8 {
            let wait = backoff.next(Instant::now());
            assert!(
                wait >= expected / 2 && wait <= expected,
                "wait {wait:?} outside the top half of {expected:?}"
            );
            expected *= 2;
            if expected > Duration::from_secs(4) * MAX_RECONNECT_MULTIPLIER {
                expected = Duration::from_secs(4) * MAX_RECONNECT_MULTIPLIER;
            }
        }
    }
}
