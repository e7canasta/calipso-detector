mod support;
use detector::{
    build,
    config::NatsConfig,
    publish::{PublisherHandle, nats},
};
use futures::StreamExt;
use scene_core::contract::{Profile, ProfileHash, Source, Timing};
use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn certificate_error(error: &async_nats::ConnectError) -> &async_nats::rustls::CertificateError {
    // The handshake returns an IO error; its cause keeps the TLS error.
    let io = std::error::Error::source(error)
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .expect("the handshake rejection keeps its cause");
    match io
        .get_ref()
        .and_then(|source| source.downcast_ref::<async_nats::rustls::Error>())
    {
        Some(async_nats::rustls::Error::InvalidCertificate(reason)) => reason,
        other => panic!("a certificate rejection was expected: {other:?}; {error}"),
    }
}
fn handle() -> (
    PublisherHandle,
    tokio::sync::mpsc::Receiver<detector::publish::Publication>,
) {
    PublisherHandle::channel(
        Source {
            site: "test".into(),
            camera: "room-local".into(),
            run_id: "nats-test".into(),
            build: build::current(),
        },
        Profile {
            camera: None,
            resident: None,
            hash: ProfileHash("hash".into()),
            senses: Vec::new(),
        },
    )
}
#[tokio::test]
#[ignore = "requires nats-server and local sockets"]
async fn real_nats_delivers_headers_and_events_before_the_scene() {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let _server = Server(
        Command::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let url = format!("nats://127.0.0.1:{port}");
    let client = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(c) = async_nats::connect(&url).await {
                break c;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let mut sub = client
        .subscribe("vision.test.room-local.>".to_owned())
        .await
        .unwrap();
    client.flush().await.unwrap();
    let (mut h, rx) = handle();
    let token = CancellationToken::new();
    let actor = tokio::spawn(nats::run(
        NatsConfig { url, prefix: None },
        "vision.test.room-local".into(),
        rx,
        token.clone(),
    ));
    let mut driver = support::driver();
    for n in 1..=3 {
        let p = driver.arrive(support::sample(n, 1)).pop().unwrap();
        assert!(h.send(p.output, p.timing));
    }
    drop(h);
    let mut last_event = 0;
    let mut final_scene = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(message) = sub.next().await {
            let value: serde_json::Value = serde_json::from_slice(&message.payload).unwrap();
            if message.subject.ends_with("events.v4") {
                last_event += 1;
                assert_eq!(value["seq"], last_event);
                assert_eq!(
                    message
                        .headers
                        .as_ref()
                        .unwrap()
                        .get(async_nats::header::NATS_MESSAGE_ID)
                        .unwrap()
                        .as_str(),
                    value["event_id"].as_str().unwrap()
                );
            } else {
                assert_eq!(value["last_event_seq"], last_event);
                if value["seq"] == 3 {
                    final_scene = true;
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    assert!(final_scene && last_event > 0);
    tokio::time::timeout(Duration::from_secs(5), actor)
        .await
        .unwrap()
        .unwrap();
}
#[tokio::test]
#[ignore = "requires local sockets"]
async fn cierre_sin_broker_termina_con_plazo_acotado() {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let (mut h, rx) = handle();
    let mut d = support::driver();
    let p = d.arrive(support::sample(1, 1)).pop().unwrap();
    assert!(h.send(
        p.output,
        Timing {
            wait_ms: 0.0,
            processing_ms: 0.0
        }
    ));
    drop(h);
    let token = CancellationToken::new();
    token.cancel();
    tokio::time::timeout(
        Duration::from_secs(5),
        nats::run(
            NatsConfig {
                url: format!("nats://127.0.0.1:{port}"),
                prefix: None,
            },
            "vision.test.room-local".into(),
            rx,
            token,
        ),
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires nats-server and local sockets"]
async fn retries_the_connection_without_losing_pending_facts() {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let _server = Server(
        Command::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let url = format!("nats://127.0.0.1:{port}");
    let client = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = async_nats::connect(&url).await {
                break client;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let mut subscriber = client
        .subscribe("vision.test.room-local.>".to_owned())
        .await
        .unwrap();
    client.flush().await.unwrap();
    // A proxy holds INFO back: it forces a real actor timeout and then lets it through.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let actor_url = format!("nats://{}", listener.local_addr().unwrap());
    let (gate, closed_gate) = tokio::sync::watch::channel(false);
    let stop_proxy = CancellationToken::new();
    let stop = stop_proxy.clone();
    let proxy = tokio::spawn(async move {
        let mut copies = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _=stop.cancelled()=>break,
                stream=listener.accept()=>{
                    let (mut incoming,_)=stream.unwrap();let mut gate=closed_gate.clone();
                    copies.spawn(async move {
                        if gate.wait_for(|open| *open).await.is_err() {return;}
                        if let Ok(mut outgoing)=tokio::net::TcpStream::connect(("127.0.0.1",port)).await {
                            let _=tokio::io::copy_bidirectional(&mut incoming,&mut outgoing).await;
                        }
                    });
                }
            }
        }
        copies.abort_all();
        while copies.join_next().await.is_some() {}
    });
    let (mut h, rx) = handle();
    let token = CancellationToken::new();
    let actor = tokio::spawn(nats::run(
        NatsConfig {
            url: actor_url,
            prefix: None,
        },
        "vision.test.room-local".into(),
        rx,
        token,
    ));
    let mut driver = support::driver();
    for n in 1..=3 {
        let p = driver.arrive(support::sample(n, 1)).pop().unwrap();
        assert!(h.send(p.output, p.timing));
    }
    drop(h);
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(!actor.is_finished());
    gate.send_replace(true);
    let mut last_event = 0;
    let mut got_scene = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(message) = subscriber.next().await {
            let v: serde_json::Value = serde_json::from_slice(&message.payload).unwrap();
            if message.subject.ends_with("events.v4") {
                last_event += 1;
                assert_eq!(v["seq"], last_event);
            } else {
                assert_eq!(v["last_event_seq"], last_event);
                got_scene = v["seq"] == 3;
                if got_scene {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    assert!(got_scene && last_event > 0);
    tokio::time::timeout(Duration::from_secs(5), actor)
        .await
        .unwrap()
        .unwrap();
    stop_proxy.cancel();
    proxy.await.unwrap();
}

#[tokio::test]
#[ignore = "requires nats-server, openssl and local sockets"]
async fn tls_checks_trust_and_server_name() {
    let directory =
        Directory(std::env::temp_dir().join(format!("calipso-nats-tls-{}", ulid::Ulid::new())));
    std::fs::create_dir(&directory.0).unwrap();
    let certificate = directory.0.join("server.pem");
    let key = directory.0.join("server.key");
    let generated = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "extendedKeyUsage=serverAuth",
        ])
        .arg("-keyout")
        .arg(&key)
        .arg("-out")
        .arg(&certificate)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "openssl: {}",
        String::from_utf8_lossy(&generated.stderr)
    );

    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let _server = Server(
        Command::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &port.to_string(), "--tls"])
            .arg("--tlscert")
            .arg(&certificate)
            .arg("--tlskey")
            .arg(&key)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    let url = format!("tls://localhost:{port}");
    let untrusted = async_nats::connect(&url).await.unwrap_err();
    assert_eq!(
        certificate_error(&untrusted),
        &async_nats::rustls::CertificateError::UnknownIssuer
    );

    let options = async_nats::ConnectOptions::new().add_root_certificates(certificate);
    let wrong_name = options
        .clone()
        .connect(format!("tls://127.0.0.1:{port}"))
        .await
        .unwrap_err();
    assert!(matches!(
        certificate_error(&wrong_name),
        async_nats::rustls::CertificateError::NotValidForName
            | async_nats::rustls::CertificateError::NotValidForNameContext { .. }
    ));

    let client = options.connect(url).await.unwrap();
    let mut subscriber = client.subscribe("vision.test.tls").await.unwrap();
    client.flush().await.unwrap();
    let mut headers = async_nats::HeaderMap::new();
    headers.insert(async_nats::header::NATS_MESSAGE_ID, "tls-test");
    client
        .publish_with_headers("vision.test.tls", headers, "TLS message".into())
        .await
        .unwrap();
    client.flush().await.unwrap();
    let message = tokio::time::timeout(Duration::from_secs(5), subscriber.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(message.payload.as_ref(), "mensaje TLS".as_bytes());
    assert_eq!(
        message
            .headers
            .as_ref()
            .unwrap()
            .get(async_nats::header::NATS_MESSAGE_ID)
            .unwrap()
            .as_str(),
        "tls-test"
    );
}
