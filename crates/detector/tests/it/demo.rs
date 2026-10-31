//! Reproducible external demo: uses only testsrc and the public NASA fixture from 303.
use futures::StreamExt;
use serde_json::Value;
use std::io::Write;
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

struct Process(Child);
impl Process {
    fn stop(&mut self, signal: &str) {
        let _ = Command::new("kill")
            .args([signal, &self.0.id().to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "process failed: {status}");
                return;
            }
            if Instant::now() > deadline {
                let _ = self.0.kill();
                panic!("the process did not close with {signal}");
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = Command::new("kill")
                .args(["-TERM", &self.0.id().to_string()])
                .status();
            let _ = self.0.wait();
        }
    }
}
// The demo uses async-nats, already a detector dependency: it does not need the nats CLI.
struct Subscription {
    stop: CancellationToken,
    thread: Option<thread::JoinHandle<()>>,
}
impl Subscription {
    fn start(url: String, path: &Path) -> Self {
        let stop = CancellationToken::new();
        let cancel = stop.clone();
        let path = path.to_owned();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let client = async_nats::connect(url).await.unwrap();
                    let mut subscriber = client.subscribe("vision.test.>").await.unwrap();
                    client.flush().await.unwrap();
                    let mut file = fs::File::create(path).unwrap();
                    ready_tx.send(()).unwrap();
                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => break,
                            message = subscriber.next() => {
                                let message = message.expect("active subscription");
                                file.write_all(&message.payload).unwrap();
                                file.write_all(b"\n").unwrap();
                            }
                        }
                    }
                });
        });
        ready_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("NATS subscription ready");
        Self {
            stop,
            thread: Some(worker),
        }
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop.cancel();
        self.thread.take().unwrap().join().unwrap();
    }
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn port() -> u16 {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    socket.local_addr().unwrap().port()
}
fn wait_file(path: &Path, predicate: impl Fn(&str) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if predicate(&fs::read_to_string(path).unwrap_or_default()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{} was not ready: {}",
            path.display(),
            fs::read_to_string(path).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(50));
    }
}
#[test]
#[ignore = "requires CALIPSO_MODELS, nats-server, mediamtx, ffmpeg and sockets"]
fn local_video_onnx_nats_and_shutdown_by_both_signals() {
    run_demo(1);
}

#[test]
#[ignore = "requires the three models in CALIPSO_MODELS, nats-server, mediamtx, ffmpeg and sockets"]
fn three_models_rtsp_nats_validate_and_publish_reinforcement_and_faces() {
    run_demo(3);
}

#[test]
#[ignore = "requires the five models in CALIPSO_MODELS, nats-server, mediamtx, ffmpeg and sockets"]
fn five_senses_rtsp_nats_validate_and_publish_pose_depth_and_calibration() {
    run_demo(5);
}

/// `senses`: 1 (main), 3 (with reinforcement and faces) or 5 (and pose and depth,
/// with calibration).
fn run_demo(senses: u8) {
    let three_models = senses >= 3;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = Directory(std::env::temp_dir().join(format!("calipso-demo-{}", ulid::Ulid::new())));
    fs::create_dir(&dir.0).unwrap();
    let video = dir.0.join("input.h264");
    // Ephemeral test artefact, with no observed pixels: black → NASA → black.
    let result = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=512x512:r=10:d=15",
            "-loop",
            "1",
            "-i",
        ])
        .arg(root.join("crates/sense/tests/fixtures/astronaut.jpg"))
        .args([
            "-filter_complex",
            "[0:v][1:v]overlay=enable='between(t,4,9)':shortest=1",
            "-t",
            "15",
            "-r",
            "10",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "10",
            "-keyint_min",
            "10",
            "-sc_threshold",
            "0",
            "-x264-params",
            "repeat-headers=1:aud=1",
            "-f",
            "h264",
        ])
        .arg(&video)
        .status()
        .unwrap();
    assert!(result.success());
    let nats_port = port();
    let rtsp_port = port();
    let url = format!("nats://127.0.0.1:{nats_port}");
    let server = Process(
        Command::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &nats_port.to_string()])
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::net::TcpStream::connect(("127.0.0.1", nats_port)).is_err() {
        assert!(Instant::now() < deadline, "NATS is not listening");
        thread::sleep(Duration::from_millis(20));
    }
    let wire = dir.0.join("wire.jsonl");
    let subscriber = Subscription::start(url.clone(), &wire);
    let config = dir.0.join("test.toml");
    let mut text = include_str!("../../../../config/test.toml")
        .replace("18554", &rtsp_port.to_string())
        .replace("nats://127.0.0.1:4222", &url)
        .replace("[160, 96]", "[512, 512]");
    if three_models {
        text = text.replace(
            "bed = [[10,10], [50,10], [50,50], [10,50]]",
            "bed = [[0,0], [512,0], [512,512], [0,512]]\nheadboard = [0,0,512,256]",
        );
        if senses == 5 {
            text.push_str("\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n[calibration]\nretry_secs = 2.0\n");
        }
        text.push_str("\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbackend_confidence = 0.25\nbed_padding = 0.15\n[senses.faces]\nmodel = \"face/yolov12l-face.onnx\"\nbackend_confidence = 0.25\nmin_confidence = 0.30\n");
    }
    fs::write(&config, text).unwrap();
    let motor_log = dir.0.join("motor.log");
    let mut detector = Process(
        Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
            .args(["--config"])
            .arg(&config)
            .stdout(Stdio::null())
            .stderr(fs::File::create(&motor_log).unwrap())
            .spawn()
            .unwrap(),
    );
    let rtsp_log = dir.0.join("rtsp.log");
    let rtsp = Process(
        Command::new("bash")
            .arg(root.join("scripts/test-rtsp.sh"))
            .arg(rtsp_port.to_string())
            .arg(&video)
            .stdout(fs::File::create(&rtsp_log).unwrap())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_file(&rtsp_log, |s| s.contains("rtsp://"));
    let until = Instant::now() + Duration::from_secs(14);
    while Instant::now() < until {
        assert!(
            detector.0.try_wait().unwrap().is_none(),
            "{}",
            fs::read_to_string(&motor_log).unwrap()
        );
        thread::sleep(Duration::from_millis(50));
    }
    detector.stop("-TERM");
    let raw = fs::read_to_string(&wire).unwrap();
    let messages: Vec<Value> = raw
        .lines()
        .filter_map(|l| l.find('{').and_then(|i| serde_json::from_str(&l[i..]).ok()))
        .collect();
    let scenes: Vec<_> = messages
        .iter()
        .filter(|m| m["schema"] == "calipso.scene.v4" && m["body"]["republished"].is_null())
        .collect();
    let events: Vec<_> = messages
        .iter()
        .filter(|m| m["schema"] == "calipso.events.v4")
        .collect();
    assert!(
        scenes.len() >= 10,
        "{} scenes; {}",
        scenes.len(),
        fs::read_to_string(&motor_log).unwrap()
    );
    if three_models {
        for kind in ["reinforce", "faces"] {
            assert!(
                scenes
                    .iter()
                    .any(|scene| scene["body"]["attention"]["looks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|look| look["kind"] == kind && look["outcome"]["type"] == "asked")),
                "no asked {kind}: {raw}"
            );
        }
        if senses == 5 {
            // Black at the start: the empty room calibrates; then the astronaut
            // in bed gives the pose and its depth.
            for kind in ["calibrate", "pose", "depth"] {
                assert!(
                    scenes.iter().any(|scene| {
                        scene["body"]["attention"]["looks"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|look| look["kind"] == kind && look["outcome"]["type"] == "asked")
                    }),
                    "no asked {kind}: {raw}"
                );
            }
            assert!(
                events.iter().any(|m| m["body"]["type"] == "bed_calibrated"),
                "no bed_calibrated: {raw}"
            );
            assert!(
                scenes
                    .iter()
                    .any(|scene| scene["body"]["subject"]["pose"]["keypoints"]
                        .as_array()
                        .is_some_and(|k| k.len() == 17)
                        && scene["body"]["subject"]["depth"]["meters"].is_number()),
                "no subject pose with depth: {raw}"
            );
        }
        assert!(
            scenes
                .iter()
                .any(|scene| scene["body"]["evidence"]["faces"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|face| face["on_subject"] == true)),
            "no face on subject: {raw}"
        );
    } else {
        assert!(
            events.iter().any(|m| m["body"]["type"] == "entered"),
            "the fixture's person did not enter: {raw}"
        );
        assert!(
            events.iter().any(|m| m["body"]["type"] == "left_room"),
            "the fixture's person did not leave: {raw}"
        );
    }
    let scene_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../../../../contract/schema/calipso.scene.v4.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let event_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../../../../contract/schema/calipso.events.v4.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for m in &messages {
        assert!(
            if m["schema"] == "calipso.scene.v4" {
                scene_schema.is_valid(m)
            } else {
                event_schema.is_valid(m)
            },
            "{m}"
        );
    }
    let spacing: Vec<_> = scenes
        .windows(2)
        .map(|p| {
            (p[1]["published_at_ms"].as_u64().unwrap() - p[0]["published_at_ms"].as_u64().unwrap())
                as f64
        })
        .collect();
    let mean = spacing.iter().sum::<f64>() / spacing.len() as f64;
    assert!((800.0..=1500.0).contains(&mean), "mean interval {mean}");
    println!(
        "NATS demo vision.test.>: {} observed scenes, {} events, mean interval {:.0} ms; all validate v4",
        scenes.len(),
        events.len(),
        mean
    );
    println!(
        "senses={senses}; reinforcement, faces, pose, depth and calibration checked when switched on"
    );
    for m in events {
        println!("{}", serde_json::to_string(m).unwrap());
    }
    // Second composition: SIGINT with the transport available.
    let mut second = Process(
        Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
            .args(["--config"])
            .arg(&config)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    thread::sleep(Duration::from_secs(2));
    second.stop("-INT");
    println!("SIGTERM and SIGINT closed with code 0; no processes of our own left");
    drop(rtsp);
    drop(subscriber);
    drop(server);
}
