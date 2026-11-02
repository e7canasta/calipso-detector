use crate::support;
use detector::build_info;
use detector::{CHANNEL_BUFFER, Kind, PublisherHandle};
use scene_contract::{Profile, ProfileHash, Source, Timing};

fn handle() -> (
    PublisherHandle,
    tokio::sync::mpsc::Receiver<detector::Publication>,
) {
    PublisherHandle::channel(
        Source {
            site: "test".into(),
            camera: "room-local".into(),
            run_id: "run-test".into(),
            build: build_info(),
        },
        Profile {
            camera: None,
            resident: None,
            hash: ProfileHash("sha256:0123456789abcdef".into()),
            senses: Vec::new(),
        },
    )
}
const TIMING: Timing = Timing {
    wait_ms: 1.0,
    processing_ms: 2.0,
};
#[test]
fn s1_valid_envelopes_events_first_and_contiguous_sequences() {
    let scene_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../../../../contract/schema/calipso.scene.v5.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let event_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../../../../contract/schema/calipso.events.v5.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut driver = support::driver();
    let (mut h, mut rx) = handle();
    let mut event_seq = 0;
    for n in 1..=25 {
        let processed = driver.arrive(support::sample(n, 1)).pop().unwrap();
        let scene_arc = std::sync::Arc::clone(&processed.output.v5.scene);
        assert!(h.send(processed.output, processed.timing));
        let publication = rx.try_recv().unwrap();
        assert!(std::sync::Arc::ptr_eq(&scene_arc, &publication.scene.body));
        let messages = publication.messages().unwrap();
        for (index, m) in messages.iter().enumerate() {
            let v: serde_json::Value = serde_json::from_slice(&m.bytes).unwrap();
            match m.kind {
                Kind::Events => {
                    event_seq += 1;
                    assert!(event_schema.is_valid(&v), "{v}");
                    assert_eq!(v["seq"], event_seq);
                    assert_eq!(v["event_id"], format!("run-test-v5-{event_seq}"));
                    assert_eq!(m.msg_id.as_deref(), v["event_id"].as_str());
                    assert!(index + 1 < messages.len());
                }
                Kind::Scene => {
                    assert_eq!(index + 1, messages.len());
                    assert!(scene_schema.is_valid(&v), "{v}");
                    assert_eq!(v["seq"], n);
                    assert_eq!(v["last_event_seq"], event_seq);
                    assert!(m.msg_id.is_none());
                    assert!(v.get("timing").is_some());
                }
            }
        }
    }
    assert!(event_seq > 0);
}
#[test]
fn a_full_channel_drops_without_waiting_and_leaves_a_seq_gap() {
    use ingest::StreamEvent;
    use std::time::Instant;
    let mut d = support::driver();
    let (mut h, mut rx) = handle();
    for i in 0..CHANNEL_BUFFER {
        let p = d
            .on_stream(
                if i % 2 == 0 {
                    StreamEvent::Streaming { session: 1 }
                } else {
                    StreamEvent::Connecting
                },
                Instant::now(),
            )
            .unwrap();
        assert!(h.send(p.output, TIMING));
    }
    let p = d
        .on_stream(StreamEvent::Streaming { session: 1 }, Instant::now())
        .unwrap();
    assert!(!h.send(p.output, TIMING));
    assert_eq!(h.dropped_events(), 1);
    while rx.try_recv().is_ok() {}
    let p = d
        .on_stream(StreamEvent::Connecting, Instant::now())
        .unwrap();
    assert!(h.send(p.output, TIMING));
    let p = rx.try_recv().unwrap();
    assert_eq!(p.events[0].seq, CHANNEL_BUFFER as u64 + 2);
    assert_eq!(p.scene.seq, CHANNEL_BUFFER as u64 + 2);
}
