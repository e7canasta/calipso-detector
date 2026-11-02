//! The verifier's port through the real driver (white book, volume III, I2):
//! a `Probe::Verify` reaches the port with the looked frame, the crop is a
//! JPEG of the bed, and the answer, delivered before a later sample, confirms
//! the held occupant in that sample's scene.
use crate::support;

use detector::Driver;
use detector::FixedClock;
use detector::{VerifierPort, jpeg_crop};
use image::DynamicImage;
use scene_core::map::SiteMap;
use scene_core::policy::{HeldVerifyPolicy, Policy, VerifyPolicy};
use scene_core::units::Seconds;
use scene_geom::{Quad, Rect};
use scene_look::{Answer, Detection, Failure, Label, Question, Ticket, Verified};
use std::time::Duration;
use support::Script;

/// Answers yes, `after` samples after each request, and keeps what it was asked.
struct Scripted {
    after: usize,
    calls: usize,
    owed: Vec<(usize, Ticket)>,
    asked: Vec<(Ticket, Question, Vec<u8>)>,
}

impl VerifierPort for Scripted {
    type Frame = DynamicImage;
    fn ask(
        &mut self,
        frame: &DynamicImage,
        ticket: Ticket,
        region: Rect,
        _bed: &scene_geom::Quad,
        question: Question,
        _crops: &[u8],
    ) -> Result<(), Failure> {
        let jpeg = jpeg_crop(frame, region)?;
        self.asked.push((ticket, question, jpeg));
        self.owed.push((self.calls, ticket));
        Ok(())
    }
    fn answers(&mut self) -> Vec<Verified> {
        self.calls += 1;
        let (due, owed): (Vec<_>, Vec<_>) = self
            .owed
            .drain(..)
            .partition(|(asked, _)| self.calls - asked >= self.after);
        self.owed = owed;
        due.into_iter()
            .map(|(_, ticket)| Verified {
                ticket,
                answer: Some(Answer::Yes),
            })
            .collect()
    }
    fn keep(&mut self, _: &DynamicImage, _: Ticket, _: Rect) -> Result<f32, Failure> {
        Ok(100.0)
    }
}

#[test]
fn a_held_occupant_is_verified_through_the_port_and_confirmed() {
    let in_bed = Detection {
        bbox: Rect::new(40.0, 30.0, 60.0, 70.0),
        confidence: 0.9,
        label: Label::Person,
    };
    let at_door = Detection {
        bbox: Rect::new(110.0, 10.0, 125.0, 40.0),
        ..in_bed
    };
    // At the door, in bed, then covered: nobody seen.
    let script = (1..=60)
        .map(|n| match n {
            1..=5 => vec![at_door],
            6..=10 => vec![in_bed],
            _ => vec![],
        })
        .collect();
    let policy = Policy {
        verify: Some(VerifyPolicy {
            held: Some(HeldVerifyPolicy {
                after_secs: Seconds::new(30.0).expect("a time"),
                valid_secs: None,
            }),
            ..VerifyPolicy::default()
        }),
        ..Policy::default()
    };
    let mut driver = Driver::new(
        policy,
        SiteMap::new(Quad::from_rect(Rect::new(20.0, 20.0, 100.0, 80.0))),
        sense::Senses::new(Script(script)),
        Duration::from_secs(10),
        FixedClock::new(),
    )
    .with_verifier(Scripted {
        after: 3,
        calls: 0,
        owed: Vec::new(),
        asked: Vec::new(),
    });
    let mut scenes = std::collections::BTreeMap::new();
    for n in 1..=60 {
        for processed in driver.arrive(support::sample(n, 1)) {
            scenes.insert(processed.output.at.sequence, processed.output.scene);
        }
    }
    let held = |n: u64| scenes[&n].bed.held.first().and_then(|h| h.check);
    assert_eq!(
        held(42).map(|c| c.state),
        Some(scene_contract::HeldCheckState::Unverified)
    );
    let check = held(46).expect("a check on the held occupant");
    assert_eq!(check.state, scene_contract::HeldCheckState::Confirmed);
    assert_eq!(check.looked.map(|l| l.sequence), Some(43));
    assert_eq!(scenes[&46].room.occupants, 1);
}

#[test]
fn the_port_receives_a_jpeg_of_the_bed_and_the_question() {
    let frame = DynamicImage::new_rgb8(128, 96);
    let mut scripted = Scripted {
        after: 0,
        calls: 0,
        owed: Vec::new(),
        asked: Vec::new(),
    };
    let ticket = Ticket {
        id: 1,
        looked: scene_contract::SampleRef {
            session: scene_contract::SessionId(1),
            sequence: 43,
            source_secs: scene_contract::SourceSecs(43.0),
        },
        target: scene_look::Target::Held {
            since: scene_contract::SampleRef {
                session: scene_contract::SessionId(1),
                sequence: 10,
                source_secs: scene_contract::SourceSecs(10.0),
            },
        },
    };
    scripted
        .ask(
            &frame,
            ticket,
            Rect::new(8.0, 11.0, 112.0, 89.0),
            &scene_geom::Quad::from_rect(Rect::new(8.0, 11.0, 112.0, 89.0)),
            Question::HeldInBed,
            &[],
        )
        .unwrap();
    let (asked, question, jpeg) = &scripted.asked[0];
    assert_eq!(asked.id, 1);
    assert_eq!(question.id(), "held_in_bed@1");
    let decoded = image::load_from_memory(jpeg).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (104, 78));
}

/// The whole way, live: the detector's NatsVerifier asks the calipso-verifier
/// service over NATS and the answer comes back. Needs `nats-server` and the
/// service running (`CALIPSO_NATS_URL`, default `nats://127.0.0.1:4222`).
#[test]
#[ignore = "needs nats-server and calipso-verifier running"]
fn the_live_verifier_answers_over_nats() {
    use detector::NatsVerifier;
    let url = std::env::var("CALIPSO_NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut verifier = NatsVerifier::spawn(
        runtime.handle(),
        url,
        "calipso.verify.v1".into(),
        "vision.test.room-1".into(),
        "room-1".into(),
    );
    let frame = DynamicImage::new_rgb8(128, 96);
    let looked = scene_contract::SampleRef {
        session: scene_contract::SessionId(1),
        sequence: 43,
        source_secs: scene_contract::SourceSecs(43.0),
    };
    let ticket = Ticket {
        id: 7,
        looked,
        target: scene_look::Target::Held { since: looked },
    };
    std::thread::sleep(Duration::from_millis(500));
    verifier
        .ask(
            &frame,
            ticket,
            Rect::new(8.0, 11.0, 112.0, 89.0),
            &scene_geom::Quad::from_rect(Rect::new(8.0, 11.0, 112.0, 89.0)),
            Question::HeldInBed,
            &[],
        )
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let answers = loop {
        let answers = verifier.answers();
        if !answers.is_empty() || std::time::Instant::now() > deadline {
            break answers;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(answers.len(), 1, "no answer in time");
    assert_eq!(answers[0].ticket.id, 7);
    println!("answer: {:?}", answers[0].answer);
}
