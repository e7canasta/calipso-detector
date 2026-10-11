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

use super::{Mailbox, Payload, Sample, StreamEvent, decode::FrameDecoder};
use crate::{IngestConfig, Stamp};

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
    fn report_if_due(&mut self, overwritten: u64) {
        let now = Instant::now();
        if self
            .last_report
            .is_some_and(|last| now.duration_since(last) < STATS_INTERVAL)
        {
            return;
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
    }
}

/// Runs reception on a dedicated thread with its own current-thread runtime,
/// so retina and FFmpeg objects never cross threads.
pub fn spawn(
    config: IngestConfig,
    mailbox: Arc<Mailbox<Sample>>,
    events: mpsc::Sender<StreamEvent>,
    shutdown: CancellationToken,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create stream runtime");
        runtime.block_on(run(config, mailbox, events, shutdown));
    })
}

async fn run(
    config: IngestConfig,
    mailbox: Arc<Mailbox<Sample>>,
    events: mpsc::Sender<StreamEvent>,
    shutdown: CancellationToken,
) {
    let creds = config
        .username
        .as_ref()
        .map(|username| retina::client::Credentials {
            username: username.clone(),
            password: config.password.clone().unwrap_or_default(),
        });
    let mut stats = IngestStats::default();
    let mut session = 0;

    loop {
        session += 1;
        let _ = events.send(StreamEvent::Connecting);
        info!("Connecting to {} (session {session})", config.url);
        let result = tokio::select! {
            result = stream_session(&config, creds.clone(), session, &mailbox, &events, &mut stats) => result,
            _ = shutdown.cancelled() => return,
        };
        let reason = match result {
            Ok(()) => IngestError::Unreachable("end of stream".to_owned()),
            Err(error) => error,
        };
        error!(
            "Stream session {session} ended: {reason}; reconnecting in {}s",
            config.reconnect_delay.as_secs_f64()
        );
        let _ = events.send(StreamEvent::Disconnected { session, reason });
        tokio::select! {
            _ = tokio::time::sleep(config.reconnect_delay) => {}
            _ = shutdown.cancelled() => return,
        }
    }
}

async fn stream_session(
    config: &IngestConfig,
    creds: Option<retina::client::Credentials>,
    session_id: u64,
    mailbox: &Mailbox<Sample>,
    events: &mpsc::Sender<StreamEvent>,
    stats: &mut IngestStats,
) -> Result<()> {
    let mut session = retina::client::Session::describe(
        config
            .url
            .parse()
            .map_err(|error| IngestError::Protocol(format!("invalid RTSP URL: {error}")))?,
        retina::client::SessionOptions::default()
            .creds(creds)
            .session_group(Arc::new(retina::client::SessionGroup::default()))
            .user_agent("RTSP Person Detector".to_owned())
            .teardown(retina::client::TeardownPolicy::Auto),
    )
    .await
    .map_err(|error| IngestError::rtsp(error, true))?;

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
        .map_err(|error| IngestError::rtsp(error, false))?;
    let mut demuxed = session
        .play(retina::client::PlayOptions::default())
        .await
        .map_err(|error| IngestError::rtsp(error, false))?
        .demuxed()
        .map_err(|error| IngestError::rtsp(error, false))?;
    info!("Streaming (session {session_id})");
    let _ = events.send(StreamEvent::Streaming {
        session: session_id,
    });

    // Continuous decoding only when every frame is a candidate sample.
    let mut decoder: Option<FrameDecoder> = None;
    let mut sync = KeyframeSync::default();
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
            Err(error) if errors.tolerate(Instant::now()) => {
                stats.packet_errors += 1;
                let summary = error.to_string();
                warn!(
                    "Packet dropped (session {session_id}): {}",
                    summary.lines().next().unwrap_or_default()
                );
                // Later frames reference the lost one.
                decoder = None;
                sync.lost();
                continue;
            }
            Err(error) => return Err(IngestError::rtsp(error, false)),
        };
        let CodecItem::VideoFrame(frame) = item else {
            continue;
        };
        stats.received += 1;
        let is_keyframe = frame.is_random_access_point();
        stats.keyframes += u64::from(is_keyframe);
        let timestamp = frame.timestamp();
        let sample = Stamp {
            session: session_id,
            sequence: stats.received,
            source_secs: timestamp.elapsed_secs(),
        };

        if config.keyframe_only {
            if is_keyframe {
                publish(
                    mailbox,
                    stats,
                    sample,
                    Payload::Keyframe {
                        data: frame.into_data(),
                        pts_ticks: timestamp.elapsed(),
                        clock_rate: timestamp.clock_rate().get(),
                    },
                )?;
            }
        } else if sync.admit(is_keyframe) {
            let decoder_ref = match decoder.as_mut() {
                Some(decoder) => decoder,
                None => decoder.insert(FrameDecoder::h264(timestamp.clock_rate().get())?),
            };
            // Convert to RGB only when the engine is waiting for a sample.
            let picture = decoder_ref
                .push(frame.data(), timestamp.elapsed())
                .and_then(|decoded| match decoded.filter(|_| mailbox.is_waiting()) {
                    Some(decoded) => decoder_ref.picture(&decoded).map(Some),
                    None => Ok(None),
                });
            match picture {
                Ok(Some(picture)) => {
                    let sample = Stamp {
                        source_secs: picture.source_time_secs.unwrap_or(sample.source_secs),
                        ..sample
                    };
                    publish(mailbox, stats, sample, Payload::Picture(picture))?;
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
                    sync.lost();
                }
            }
        }
        stats.report_if_due(mailbox.overwritten());
    }
    Ok(())
}

/// Continuous decoding starts at a keyframe and, after a decode error,
/// restarts at the next one: frames in between reference lost state.
#[derive(Default)]
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
    /// Records an error at `now`; whether the session may go on.
    fn tolerate(&mut self, now: Instant) -> bool {
        self.in_a_row += 1;
        let since = *self.since.get_or_insert(now);
        self.in_a_row < MAX_ERRORS_IN_A_ROW && now.duration_since(since) < ERROR_WINDOW
    }

    /// A good item arrived.
    fn reset(&mut self) {
        (self.in_a_row, self.since) = (0, None);
    }
}

fn publish(
    mailbox: &Mailbox<Sample>,
    stats: &mut IngestStats,
    sample: Stamp,
    payload: Payload,
) -> Result<()> {
    let delivered = mailbox.put(Sample {
        stamp: sample,
        received_at: Instant::now(),
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
    use super::{ERROR_WINDOW, ErrorBudget, KeyframeSync, MAX_ERRORS_IN_A_ROW};
    use std::time::{Duration, Instant};

    #[test]
    fn a_burst_of_fragment_errors_is_tolerated() {
        // Joining mid-NAL: one error per remaining fragment, all at once.
        let mut budget = ErrorBudget::default();
        let now = Instant::now();
        assert!((0..200).all(|_| budget.tolerate(now)));
        budget.reset();
        assert!(
            budget.tolerate(now + ERROR_WINDOW * 10),
            "a good item resets the window"
        );
    }

    #[test]
    fn errors_without_good_items_end_the_session() {
        let mut budget = ErrorBudget::default();
        let start = Instant::now();
        assert!(budget.tolerate(start));
        assert!(budget.tolerate(start + Duration::from_millis(1900)));
        assert!(!budget.tolerate(start + ERROR_WINDOW));

        let mut spinning = ErrorBudget::default();
        let tolerated = (0..MAX_ERRORS_IN_A_ROW * 2)
            .take_while(|_| spinning.tolerate(start))
            .count();
        assert_eq!(tolerated as u32, MAX_ERRORS_IN_A_ROW - 1);
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
}
