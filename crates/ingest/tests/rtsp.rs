//! CALIPSO_RTSP_TEST=1 cargo test -p ingest --test rtsp -- --ignored --nocapture
//! Requires mediamtx and ffmpeg. Each test owns its server on a free local port.
use ingest::{IngestConfig, Mailbox, Payload, Sample, StreamEvent, Take};
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    process::{Child, Command, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

struct Server(Child);
impl Server {
    fn start(port: u16) -> Self {
        let mut child = Command::new("bash")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/test-rtsp.sh"
            ))
            .arg(port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start local RTSP server");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let server = Self(child);
        assert_eq!(
            line.trim(),
            format!("rtsp://127.0.0.1:{port}/test"),
            "server ready"
        );
        server
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        // SIGTERM runs the script's cleanup trap, including its publisher.
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .status();
        let _ = self.0.wait();
    }
}

struct Receiver {
    mailbox: Arc<Mailbox<Sample>>,
    events: mpsc::Receiver<StreamEvent>,
    shutdown: CancellationToken,
    thread: Option<thread::JoinHandle<()>>,
}
impl Receiver {
    fn start(port: u16, keyframe_only: bool) -> Self {
        let mailbox = Arc::new(Mailbox::default());
        let (send, events) = mpsc::channel();
        let shutdown = CancellationToken::new();
        let handle = ingest::spawn(
            IngestConfig {
                url: format!("rtsp://127.0.0.1:{port}/test"),
                keyframe_only,
                reconnect_delay: Duration::from_secs(3),
                ..IngestConfig::default()
            },
            Arc::clone(&mailbox),
            send,
            shutdown.clone(),
        );
        Self {
            mailbox,
            events,
            shutdown,
            thread: Some(handle),
        }
    }
    fn sample(&self, session: u64) -> Sample {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            match self.mailbox.take(Duration::from_millis(200)) {
                Take::Value(sample) if sample.stamp.session == session => return sample,
                Take::Closed => panic!("ingest mailbox closed"),
                _ => {}
            }
        }
        panic!("no sample for session {session}")
    }
    fn event(&self) -> StreamEvent {
        self.events
            .recv_timeout(Duration::from_secs(15))
            .expect("stream event")
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.shutdown.cancel();
        if let Some(handle) = self.thread.take() {
            handle.join().unwrap();
        }
    }
}
fn enabled() -> bool {
    std::env::var("CALIPSO_RTSP_TEST").as_deref() == Ok("1")
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
#[ignore = "requires CALIPSO_RTSP_TEST=1, mediamtx and ffmpeg"]
fn rtsp_samples_and_reconnection() {
    if !enabled() {
        eprintln!("Set CALIPSO_RTSP_TEST=1 to run local RTSP acceptance");
        return;
    }
    let port = free_port();
    let server = Server::start(port);
    let receiver = Receiver::start(port, true);
    assert!(matches!(receiver.event(), StreamEvent::Connecting));
    assert!(matches!(
        receiver.event(),
        StreamEvent::Streaming { session: 1 }
    ));
    let first = receiver.sample(1);
    let second = receiver.sample(1);
    let third = receiver.sample(1);
    assert!(
        first.stamp.sequence < second.stamp.sequence
            && second.stamp.sequence < third.stamp.sequence
    );
    assert!(
        first.stamp.source_secs < second.stamp.source_secs
            && second.stamp.source_secs < third.stamp.source_secs
    );
    for sample in [&first, &second, &third] {
        assert!(matches!(sample.payload, Payload::Keyframe { .. }));
        assert!(sample.received_at <= Instant::now());
    }
    println!(
        "session=1 sequences={},{},{} source_secs={:.3},{:.3},{:.3}",
        first.stamp.sequence,
        second.stamp.sequence,
        third.stamp.sequence,
        first.stamp.source_secs,
        second.stamp.source_secs,
        third.stamp.source_secs
    );
    drop(server);
    let disconnected = receiver.event();
    assert!(matches!(
        disconnected,
        StreamEvent::Disconnected { session: 1, .. }
    ));
    let _server = Server::start(port);
    assert!(matches!(receiver.event(), StreamEvent::Connecting));
    assert!(matches!(
        receiver.event(),
        StreamEvent::Streaming { session: 2 }
    ));
    let resumed = receiver.sample(2);
    assert!(resumed.stamp.sequence > third.stamp.sequence);
    println!(
        "{disconnected:?} -> Connecting -> Streaming {{ session: 2 }}; sequence={} source_secs={:.3}",
        resumed.stamp.sequence, resumed.stamp.source_secs
    );
}

#[test]
#[ignore = "requires CALIPSO_RTSP_TEST=1, mediamtx and ffmpeg"]
fn continuous_decode_delivers_rgb_when_consumer_waits() {
    if !enabled() {
        return;
    }
    let port = free_port();
    let _server = Server::start(port);
    let receiver = Receiver::start(port, false);
    let sample = receiver.sample(1);
    let Payload::Picture(picture) = sample.payload else {
        panic!("continuous RGB sample")
    };
    assert_eq!((picture.width(), picture.height()), (160, 96));
    assert_eq!(sample.stamp.source_secs, picture.source_time_secs.unwrap());
    println!("Continuous decode OK: 160x96 RGB, session=1");
}
