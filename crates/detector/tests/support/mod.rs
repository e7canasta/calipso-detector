#![allow(dead_code)]
use detector::clock::FixedClock;
use detector::driver::Driver;
use detector::source::{Arrival, rtsp::SampleDecoder};
use image::DynamicImage;
use ingest::{Clock, Payload, Sample, Stamp};
use scene_core::{
    geom::{Quad, Rect},
    look::Detection,
    map::SiteMap,
    policy::Policy,
};
use std::{collections::VecDeque, time::Duration};

pub struct Script(pub VecDeque<Vec<Detection>>);
impl sense::DetectorPort for Script {
    fn detect(&mut self, _: &DynamicImage) -> Result<Vec<Detection>, sense::ModelError> {
        Ok(self.0.pop_front().expect("one reading per sample"))
    }
}
pub fn driver() -> Driver<sense::Senses, detector::verifier::NoVerifier<DynamicImage>, FixedClock> {
    const S1: &str = include_str!("../../../../../calipso-golden/scenes/s1-staff-put-to-bed.md");
    let script = golden::language::Scene::parse(S1)
        .unwrap()
        .samples()
        .into_iter()
        .map(|m| {
            m.detect
                .into_iter()
                .map(|mut d| {
                    let b = d.bbox;
                    d.bbox = Rect::new(b.x0 / 5.0, b.y0 / 5.0, b.x1 / 5.0, b.y1 / 5.0);
                    d
                })
                .collect()
        })
        .collect();
    Driver::new(
        Policy::default(),
        SiteMap::new(Quad::from_rect(Rect::new(20.0, 20.0, 100.0, 80.0))),
        sense::Senses::new(Script(script)),
        Duration::from_secs(3),
        FixedClock::new(),
    )
}
/// A real video sample (testsrc keyframe), already decoded as it arrives from
/// the RTSP source.
pub fn sample(n: u64, session: u64) -> Arrival<DynamicImage> {
    const VIDEO: &[u8] = include_bytes!("../../../ingest/tests/fixtures/testsrc.h264");
    let end = VIDEO
        .windows(5)
        .enumerate()
        .filter(|(_, b)| *b == [0, 0, 0, 1, 9])
        .nth(1)
        .unwrap()
        .0;
    // One clock stamps the arrival and starts the decoding: `wait_ms` is exact.
    let clock = FixedClock::new();
    SampleDecoder::default().arrival(
        Sample {
            stamp: Stamp {
                session,
                sequence: n,
                source_secs: n as f64,
            },
            received_at: clock.now(),
            payload: Payload::Keyframe {
                data: VIDEO[..end].to_vec(),
                pts_ticks: n as i64 * 90000,
                clock_rate: 90000,
            },
        },
        &clock,
    )
}
